// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The forward index** - every function, decoded once, with everything it refers to.
//!
//! [`crate::xref`] answers *"who points at X?"* and costs a whole-image sweep per question. This
//! module answers the inverse - *"what does each function point at?"* - for every function in one
//! pass, which is what makes a whole-image atlas possible at all.
//!
//! ## Function identity
//!
//! A function is a `55 48 89 e5` prologue (`push rbp; mov rbp,rsp`), the only start this project
//! allows (ATLAS §4.5). Its extent runs to the **next** prologue - never further, which is the
//! `--count` overrun trap stated in one place instead of remembered in several.
//!
//! ⚠ **Not every prologue is a function, and not every function is a prologue.** Those four bytes also occur inside immediates and inside
//! data islands, and this module does not pretend otherwise: [`Function::clean`] records whether
//! the decode reached the next prologue **without hitting an undecodable byte**, and
//! [`Function::padded`] records whether the byte before the prologue is `cc`/`90`/`00` (a compiler
//! pad, i.e. a real function boundary). Both are reported rather than used to silently filter,
//! because 35,834 is the denominator every coverage claim in `design/eboot-map.md` is stated
//! against and quietly shrinking it would inflate every percentage.
//!
//! ⚠⚠ **AND THE PROLOGUE COUNT IS NOT THE FUNCTION COUNT** (ATLAS §4.2d, found the hard way during
//! this module's own bring-up). 11,446 direct `call rel32` targets have **no** `55 48 89 e5` -
//! frame-pointer-omitted functions - and a prologue-only index answers "which function is
//! `0x56CA70`?" with *the previous function*. [`build`] therefore runs **two passes**: the second
//! folds pass 1's call targets into the boundary set. See [`Anchored`].
//!
//! ## What is deliberately NOT here
//!
//! No linear whole-image sweep (ATLAS §4.5 rule 1) - decoding is bounded per function and restarts
//! at every prologue, so one data island costs one function, not the rest of the image.

use std::collections::{HashMap, HashSet};

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};
use serde::Serialize;

use crate::image::{Image, Result, PROLOGUE};

/// How a function's start address is justified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchored {
    /// `55 48 89 e5`. The 35,834 that every coverage number in this project is stated against.
    Prologue,
    /// ⭐ **Not a prologue, but the target of a direct `call rel32`** - a function the compiler
    /// built without a frame pointer.
    ///
    /// ⚠ **The prologue count is NOT the function count, and this project has been quietly wrong
    /// about that.** `0x56CA70` (the registry `find` - the hot leaf of ATLAS §4.4c), `0xA8C920`
    /// (the enum name-table registrar of THE-CHAIN §4.1), `0x260C10` (the accessor whose 40
    /// referrers killed dead-list theory #1) and `0x1DF0` (the `xor eax,eax; ret` constant thunk
    /// of eboot-map §3.1) are **all** frame-pointer-omitted, so a prologue-only index answers
    /// "which function is `0x56CA70`?" with *the previous function*, confidently and wrongly.
    CallTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub anchored: Anchored,
    pub start: u64,
    /// Exclusive end of the *decoded* body - never past the next prologue.
    pub end: u64,
    /// The next prologue (the hard bound). `end < bound` means the decode stopped early.
    pub bound: u64,
    pub insns: u32,
    /// The decode reached `bound` with no invalid instruction.
    pub clean: bool,
    /// Preceded by compiler padding (`cc`/`90`/`00`) - evidence this really is a function start.
    pub padded: bool,
    /// 16-byte aligned.
    pub aligned: bool,
    /// Makes no `call` at all.
    pub leaf: bool,
    /// Ends in `jmp rel32` out of its own extent - a tail call, invisible to a `call`-only scan.
    pub tail: bool,
    /// Switch-table entries skipped inside this body (§2.2 of `eboot-atlas-roadmap.md`).
    pub jt_entries: u32,
}

/// ⛔ **How an address was attributed to a function - and "nearest prologue at or before" is NOT
/// an attribution.**
///
/// This enum exists because that shortcut published a wrong finding. `d1-eboot xref 0x25c0c58`
/// reported its referring instruction (`0x1D20A1`) as `fn 0x1D1E80`, but `FUN_001D1E80` ends at
/// `0x1D2020`; the site is inside `0x1D2030`, a frame-pointer-omitted function. `0x1D1E80` is the
/// registrar that **writes** a pool and `0x1D2030` is the lookup that **reads** it, so the
/// mis-attribution made the pool look write-only and an investigating session concluded the
/// dispatch below it was dead code. It was live. ⇒ `status/LESSONS.md`,
/// `design/activity-lifetime-state.md`.
///
/// A wrong name is worse than an honest unknown, so [`Containment::None`] is a first-class answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Containment {
    /// Inside `[start, end)` of a `55 48 89 e5` function.
    Prologue,
    /// Inside `[start, end)` of a frame-pointer-omitted function (a direct `call rel32` target).
    Fpo,
    /// Inside the function's extent but past its decoded `end` - the body truncated on a data
    /// island, so containment is plausible and unproven. Printed `fn ?0x…`.
    Unproven,
    /// **No known function contains this address.** Printed `fn -`, never a neighbour's name.
    None,
}

impl Containment {
    /// Containment actually proven by a decode - the only case a name may be asserted from.
    pub fn proven(self) -> bool {
        matches!(self, Self::Prologue | Self::Fpo)
    }
}

/// The result of [`Index::containing`]: which function, and on what evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Containing {
    /// The function's start. `None` only when [`Containing::how`] is [`Containment::None`].
    pub start: Option<u64>,
    pub how: Containment,
}

impl Containing {
    pub const NONE: Containing = Containing { start: None, how: Containment::None };

    /// The start, but **only when containment is proven** - so a caller that ignores `how` still
    /// cannot print a neighbour's name.
    pub fn proven_start(&self) -> Option<u64> {
        self.start.filter(|_| self.how.proven())
    }

    /// The `fn` column, one spelling for every reader: `fn 0x001d2030`, `fn ?0x001d2030` when the
    /// decode never reached the site, `fn -` when nothing contains it.
    pub fn label(&self) -> String {
        match (self.start, self.how) {
            (Some(s), Containment::Prologue | Containment::Fpo) => format!("fn {s:#09x}"),
            (Some(s), Containment::Unproven) => format!("fn ?{s:#09x}"),
            _ => "fn -".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Index {
    pub funcs: Vec<Function>,
    /// `(caller_start, callee_target)` for every direct `call rel32`.
    pub calls: Vec<(u64, u64)>,
    /// `(caller_start, target)` for every `jmp rel32` that leaves the caller's extent.
    pub tailcalls: Vec<(u64, u64)>,
    /// `(function_start, resolved_absolute_address)` for every RIP-relative memory operand.
    pub riprefs: Vec<(u64, u64)>,
    /// `(function_start, imm32)` for every 32-bit immediate ≥ `IMM_FLOOR`.
    pub imms: Vec<(u64, u32)>,
    /// ⭐ `(caller, callee)` for direct calls where **`RDI` still holds the caller's own `this`**.
    ///
    /// A C++ member function receives `this` in `RDI`. If a method of class `C` calls `G` with that
    /// same pointer still in `RDI`, then `G` is a member of `C` (or of a base subobject at offset
    /// 0). This is the only signal in the image that reaches **non-virtual** member functions,
    /// which are the bulk of what RTTI cannot name - a non-virtual method is in no vtable, has no
    /// `__FILE__`, and often has no string.
    ///
    /// ⚠ Deliberately narrow. `lea rdi,[rbx+0x20]` - a call on a *member* object - breaks the
    /// chain, which is correct: that callee belongs to the member's class, not this one.
    pub this_calls: Vec<(u64, u64)>,
}

/// Immediates below this are loop counters and struct offsets, not identities. A name hash or a
/// content tag is always far above it; including the small ones would swamp the FNV-1 oracle with
/// noise and destroy the base-rate argument in `historicaldocs/design/THE-CHAIN.md` §4.1.
pub const IMM_FLOOR: u32 = 0x1000;

impl Index {
    /// Every function start, prologue-anchored and call-target alike, sorted.
    pub fn starts(&self) -> Vec<u64> {
        self.funcs.iter().map(|f| f.start).collect()
    }

    /// Only the `55 48 89 e5` ones - the 35,834 denominator.
    pub fn prologue_starts(&self) -> Vec<u64> {
        self.funcs
            .iter()
            .filter(|f| f.anchored == Anchored::Prologue)
            .map(|f| f.start)
            .collect()
    }

    /// Sorted start list → the greatest start at or before `va`, by binary search.
    ///
    /// ⚠ **Pass [`Index::starts`], never [`Index::prologue_starts`]** - with the prologue-only list
    /// this is the very bug [`Containment`] exists to name. And even with the full list it answers
    /// *"which function begins nearest below this?"*, not *"which function contains this?"*: it
    /// never consults an end, so it cannot say "nothing". Prefer [`Index::containing`].
    pub fn function_of(starts: &[u64], va: u64) -> Option<u64> {
        match starts.binary_search(&va) {
            Ok(i) => Some(starts[i]),
            Err(0) => None,
            Err(i) => Some(starts[i - 1]),
        }
    }

    /// ⭐ **The containing function of `va`, honestly** - FPO functions included, and an explicit
    /// "nothing contains this" instead of the nearest preceding prologue.
    ///
    /// `funcs` is sorted by `start` and tiles each executable segment (`bound` is the next start),
    /// so the greatest start at or below `va` is the only candidate; whether it *contains* `va` is
    /// then a comparison against its decoded `end`.
    pub fn containing(&self, va: u64) -> Containing {
        let i = match self.funcs.binary_search_by_key(&va, |f| f.start) {
            Ok(i) => i,
            Err(0) => return Containing::NONE,
            Err(i) => i - 1,
        };
        let f = &self.funcs[i];
        let how = if va < f.end {
            match f.anchored {
                Anchored::Prologue => Containment::Prologue,
                Anchored::CallTarget => Containment::Fpo,
            }
        } else if va < f.bound {
            // Inside the extent but past where the decode stopped - a data island ate the tail.
            Containment::Unproven
        } else {
            return Containing::NONE;
        };
        Containing { start: Some(f.start), how }
    }

    pub fn callers(&self) -> HashMap<u64, Vec<u64>> {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for &(c, t) in self.calls.iter().chain(self.tailcalls.iter()) {
            m.entry(t).or_default().push(c);
        }
        for v in m.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        m
    }

    pub fn callees(&self) -> HashMap<u64, Vec<u64>> {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for &(c, t) in self.calls.iter().chain(self.tailcalls.iter()) {
            m.entry(c).or_default().push(t);
        }
        for v in m.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        m
    }

    /// function → the addresses it reaches RIP-relatively (globals, string literals, jump tables).
    pub fn refs_by_function(&self) -> HashMap<u64, Vec<u64>> {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for &(f, t) in &self.riprefs {
            m.entry(f).or_default().push(t);
        }
        for v in m.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        m
    }

    /// target → the functions that reach it RIP-relatively. The inverse index, built once.
    pub fn referrers(&self) -> HashMap<u64, Vec<u64>> {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for &(f, t) in &self.riprefs {
            m.entry(t).or_default().push(f);
        }
        for v in m.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        m
    }
}

/// Every `55 48 89 e5` in an executable segment, as guest vaddrs, sorted.
///
/// **This is the denominator.** `design/eboot-map.md` and every coverage number in this project are
/// stated against it, so it is a pinned control (`d1-eboot controls`).
pub fn prologues(image: &Image) -> Vec<u64> {
    let mut out = Vec::new();
    for seg in image.executable_segments() {
        let r = seg.file_range();
        let blob = &image.data()[r.start..r.end.min(image.data().len())];
        let mut i = 0usize;
        while i + PROLOGUE.len() <= blob.len() {
            match blob[i..].windows(PROLOGUE.len()).position(|w| w == PROLOGUE) {
                Some(k) => {
                    out.push(seg.vaddr + (i + k) as u64);
                    i += k + 1;
                }
                None => break,
            }
        }
    }
    out.sort_unstable();
    out
}

/// Decode every function once and record everything it refers to.
///
/// **Two passes, and the second one is the point.** Pass 1 decodes the prologue-anchored functions
/// and collects their direct `call` targets. Any target that is not itself a prologue is a
/// frame-pointer-omitted function, and pass 2 re-decodes with those folded into the boundary set -
/// otherwise their bodies are silently attributed to whatever function precedes them.
pub fn build(image: &Image) -> Result<Index> {
    let pro = prologues(image);
    let pro_set: HashSet<u64> = pro.iter().copied().collect();
    let first = scan(image, &pro, &pro_set)?;

    // Direct `call` targets only. A `jmp rel32` target may be a basic block inside the same
    // function (a switch arm, a shared epilogue), and admitting those would shred real functions
    // into fragments - the opposite failure to the one being fixed.
    let mut extra: Vec<u64> = first
        .calls
        .iter()
        .map(|&(_, t)| t)
        .filter(|t| !pro_set.contains(t))
        .collect();
    extra.sort_unstable();
    extra.dedup();
    if extra.is_empty() {
        return Ok(first);
    }
    let mut all = pro.clone();
    all.extend_from_slice(&extra);
    all.sort_unstable();
    all.dedup();
    scan(image, &all, &pro_set)
}

fn scan(image: &Image, starts: &[u64], pro_set: &HashSet<u64>) -> Result<Index> {
    let mut idx = Index::default();
    idx.funcs.reserve(starts.len());

    for (n, &start) in starts.iter().enumerate() {
        let bound = starts
            .get(n + 1)
            .copied()
            .filter(|b| image.segment_for_vaddr(*b) == image.segment_for_vaddr(start))
            .unwrap_or_else(|| {
                image
                    .segment_for_vaddr(start)
                    .map(|s| s.vaddr + s.filesz)
                    .unwrap_or(start + 16)
            });
        let len = (bound - start) as usize;
        // +15 so a final instruction straddling `bound` still decodes; it is not recorded past end.
        let blob = match image.bytes(start, len + 15).or_else(|_| image.bytes(start, len)) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let mut dec = Decoder::with_ip(64, blob, start, DecoderOptions::NONE);
        let mut ins = Instruction::default();
        let mut end = start;
        let mut count = 0u32;
        let mut clean = true;
        let mut leaf = true;
        let mut tail = false;
        // Candidate jump-table bases: a RIP-relative reference into the function's OWN body is
        // almost always `lea rcx,[rip+table]` for a switch. See `jump_table_len`.
        let mut tables: HashSet<u64> = HashSet::new();
        let mut jt_entries = 0u32;
        // `this`-tracking: which registers and stack slots still hold the incoming RDI.
        let mut this_regs: HashSet<Register> = HashSet::from([Register::RDI]);
        let mut this_slots: HashSet<i64> = HashSet::new();
        while dec.can_decode() {
            let ip = dec.ip();
            if ip >= bound {
                break;
            }
            // ⭐ A switch table is DATA sitting in the middle of the code. Decoding it produces
            // either an "invalid instruction" (which used to end the function early and cost every
            // call, string and immediate after it) or - worse - plausible garbage. Skip it.
            if tables.contains(&ip) {
                if let Some(n) = jump_table_len(image, ip, start, bound) {
                    jt_entries += n as u32;
                    let after = ip + 4 * n as u64;
                    let Ok(rest) = image.bytes(after, (bound - after) as usize + 15) else { break };
                    dec = Decoder::with_ip(64, rest, after, DecoderOptions::NONE);
                    end = after;
                    continue;
                }
            }
            dec.decode_out(&mut ins);
            if ins.is_invalid() {
                clean = false;
                break;
            }
            count += 1;
            end = ip + ins.len() as u64;
            if ins.is_ip_rel_memory_operand() {
                let t = ins.ip_rel_memory_address();
                if t > ip && t < bound {
                    tables.insert(t);
                }
            }

            // ── `this` propagation, before the call is recorded ─────────────────────────────
            match ins.mnemonic() {
                // mov reg, reg  /  mov [rbp-N], reg  /  mov reg, [rbp-N]
                Mnemonic::Mov => match (ins.op0_kind(), ins.op1_kind()) {
                    (OpKind::Register, OpKind::Register) => {
                        if this_regs.contains(&ins.op1_register()) {
                            this_regs.insert(ins.op0_register());
                        } else {
                            this_regs.remove(&ins.op0_register());
                        }
                    }
                    (OpKind::Memory, OpKind::Register) => {
                        if ins.memory_base() == Register::RBP && ins.memory_index() == Register::None
                        {
                            let d = ins.memory_displacement64() as i64;
                            if this_regs.contains(&ins.op1_register()) {
                                this_slots.insert(d);
                            } else {
                                this_slots.remove(&d);
                            }
                        }
                    }
                    (OpKind::Register, OpKind::Memory) => {
                        let from_slot = ins.memory_base() == Register::RBP
                            && ins.memory_index() == Register::None
                            && this_slots.contains(&(ins.memory_displacement64() as i64));
                        if from_slot {
                            this_regs.insert(ins.op0_register());
                        } else {
                            this_regs.remove(&ins.op0_register());
                        }
                    }
                    _ => {}
                },
                // Anything else writing a register kills its `this`-ness.
                _ => {
                    if ins.op_count() > 0 && ins.op0_kind() == OpKind::Register {
                        this_regs.remove(&ins.op0_register());
                    }
                }
            }

            match ins.mnemonic() {
                Mnemonic::Call if ins.op0_kind() == OpKind::NearBranch64 => {
                    leaf = false;
                    let t = ins.near_branch_target();
                    idx.calls.push((start, t));
                    if this_regs.contains(&Register::RDI) {
                        idx.this_calls.push((start, t));
                    }
                    // The System V caller-saved set is destroyed across a call. Callee-saved
                    // registers (RBX, RBP, R12-R15) and the RBP stack slots survive - which is
                    // exactly why `this` is usually spilled, and why tracking the spill matters.
                    for r in [
                        Register::RAX, Register::RCX, Register::RDX, Register::RSI,
                        Register::RDI, Register::R8, Register::R9, Register::R10, Register::R11,
                    ] {
                        this_regs.remove(&r);
                    }
                }
                Mnemonic::Jmp if ins.op0_kind() == OpKind::NearBranch64 => {
                    let t = ins.near_branch_target();
                    if t < start || t >= bound {
                        tail = true;
                        idx.tailcalls.push((start, t));
                    }
                }
                _ => {}
            }
            if ins.is_ip_rel_memory_operand() {
                idx.riprefs.push((start, ins.ip_rel_memory_address()));
            }
            for oi in 0..ins.op_count() {
                match ins.op_kind(oi) {
                    OpKind::Immediate32 => {
                        let v = ins.immediate32();
                        if v >= IMM_FLOOR {
                            idx.imms.push((start, v));
                        }
                    }
                    OpKind::Immediate32to64 => {
                        let v = ins.immediate32to64();
                        if (0..=u32::MAX as i64).contains(&v) && v as u32 >= IMM_FLOOR {
                            idx.imms.push((start, v as u32));
                        }
                    }
                    _ => {}
                }
            }
        }
        if end < bound {
            clean = false;
        }
        let padded = image
            .u8(start.wrapping_sub(1))
            .map(|b| matches!(b, 0xCC | 0x90 | 0x00))
            .unwrap_or(false);
        idx.funcs.push(Function {
            anchored: if pro_set.contains(&start) { Anchored::Prologue } else { Anchored::CallTarget },
            start,
            end,
            bound,
            insns: count,
            clean,
            padded,
            aligned: start % 16 == 0,
            leaf,
            tail,
            jt_entries,
        });
    }
    Ok(idx)
}

/// How many `i32` entries the switch table at `base` has, or `None` if it is not one.
///
/// The idiom is `lea rcx,[rip+base]; movsxd rax,[rcx+idx*4]; add rax,rcx; jmp rax`, so each entry
/// is **`target - base`** as a signed 32-bit delta. An entry is real only if it lands on an
/// instruction inside this function's own extent - which is also the termination condition, since
/// the byte after the table is code and reads as a wild delta.
///
/// ⚠ Requires **at least 2** plausible entries. A single accidental match is exactly how a
/// data-island heuristic starts eating real code, and eating code here is silent: the function
/// simply loses its tail.
pub(crate) fn jump_table_len(image: &Image, base: u64, start: u64, bound: u64) -> Option<usize> {
    let max = ((bound - base) / 4) as usize;
    let mut n = 0usize;
    while n < max {
        let Ok(d) = image.i32(base + 4 * n as u64) else { break };
        let target = (base as i64 + d as i64) as u64;
        if target < start || target >= bound {
            break;
        }
        n += 1;
    }
    if n >= 2 { Some(n) } else { None }
}
