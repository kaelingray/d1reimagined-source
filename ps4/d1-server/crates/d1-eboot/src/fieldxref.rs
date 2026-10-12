// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Field-level cross-references - **who READS and who WRITES `object + offset`**.
//!
//! ## The motivating failure (2026-08-09, rung 33)
//!
//! *"Who writes the activity-name field at `+8` of the `c_activity_loader` singleton
//! (`DAT_0267AF08`)?"* was asked with [`crate::xref`] against the absolute address, and the answer
//! came back nearly empty **with all four controls passing**. The tool was working; the technique
//! was wrong. The field is read and written **register-relative** -
//!
//! ```text
//! 0x0033ff30   lea rax,[rip+…]   ; -> 0x267af08     the accessor: returns the singleton
//! 0x0034cb84   mov rdi, r13                          … handed to a helper …
//! 0x00340240   mov rax,[rdi+8]                       … which dereferences the FIELD
//! ```
//!
//! - and an absolute-address scan is blind to `[rdi+8]` **by construction**: the bytes
//! `48 8b 47 08` contain no address at all. `xref` says so itself
//! ([`crate::xref`] module docs, ATLAS §4.2c) and does not pretend otherwise. This module is the
//! complementary technique: enumerate the functions that plausibly *hold* the object, then look at
//! every memory operand whose **displacement** is the field offset.
//!
//! ## Candidate resolution - a union of four mechanisms
//!
//! | source | [`Origin`] | how |
//! |---|---|---|
//! | RTTI class vtable slots | [`Origin::ClassVfunc`] | [`crate::rtti`] class → vtables → slots |
//! | ctor/dtor | [`Origin::ClassVptr`] | functions that RIP-relatively reference the class vptr (and `vptr-0x10`, ⚠ CLAUDE.md trap 7) |
//! | direct users of the global | [`Origin::GlobalRef`] | confirmed `riprel` xrefs to the singleton address |
//! | callers of an accessor | [`Origin::AccessorCaller`] | `call`/`jmp` xrefs to any [`Origin::GlobalRef`] function |
//! | one hop of argument forwarding | [`Origin::AccessorForwarded`] | the tracker saw the object in **RDI** at a `call` |
//!
//! ## The honest limit - and why every row carries `tracked`
//!
//! You cannot prove `[rdi+8]` is *this* object without real dataflow. This module runs a **linear,
//! intraprocedural, branch-oblivious** tracker (the same shape as [`crate::functions`]'s `this`
//! propagation, which is already load-bearing for `d1-eboot thiscalls`): a set of registers and
//! `[rbp-N]` spill slots believed to hold the object, seeded from RDI at entry, from a
//! `lea reg,[rip+global]`, and from RAX after a call to an accessor; killed on any other write and,
//! for the volatile registers, across every call.
//!
//! So each hit says which it is:
//!
//! * `tracked: true`  - the base register provably held the object at that instruction. `[C]`.
//! * `tracked: false` - the **displacement matched and nothing else did**. `[U]`: the base may be
//!   any unrelated object with a field at the same offset. Reported, never counted as evidence.
//! * `absolute: true` - a RIP-relative operand landing exactly on `global + offset`. Unambiguous,
//!   and the only class [`crate::xref`] could already see.
//!
//! Branches are not followed, so a value that becomes the object only on one path is missed
//! (**false negatives are possible; the `truncated` list names the functions where the decoder also
//! stopped early**). This is stated rather than papered over, per ATLAS §4.
//!
//! ## Example
//!
//! ```text
//! d1-eboot field-xref --class c_activity_loader --global 0x267af08 --offset 8
//! d1-eboot field-xref --global 0x267af08 --offset 8 --all      # include untracked candidates
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use iced_x86::{
    Decoder, DecoderOptions, Formatter, Instruction, InstructionInfoFactory, IntelFormatter,
    Mnemonic, OpAccess, OpKind, Register,
};
use serde::Serialize;

use crate::functions::{self, Index};
use crate::image::{Image, Result};
use crate::rtti::Rtti;
use crate::xref::{self, RefKind};

/// How a function got into the candidate set. A function can arrive by several routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A slot of one of the RTTI class's vtables.
    ClassVfunc,
    /// RIP-relatively references the class's own vptr - a constructor or destructor.
    ClassVptr,
    /// RIP-relatively references the singleton global itself (the accessor, or a direct user).
    GlobalRef,
    /// Calls (or tail-calls) a [`Origin::GlobalRef`] function - the accessor's callers.
    AccessorCaller,
    /// The tracker saw the object in RDI at a call site in a candidate: this is the callee.
    AccessorForwarded,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClassVfunc => "class-vfunc",
            Self::ClassVptr => "class-vptr",
            Self::GlobalRef => "global-ref",
            Self::AccessorCaller => "accessor-caller",
            Self::AccessorForwarded => "accessor-forwarded",
        }
    }
}

/// What the instruction does to the field. Taken from iced's own instruction-info, not guessed:
/// this is what makes `lea rax,[rdi+8]` an [`Access::AddrOf`] rather than a bogus read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Read,
    Write,
    ReadWrite,
    /// `lea` - the *address* of the field is produced. Whoever receives it may write through it,
    /// which is exactly how `c_activity_loader::vfunc[1]` initialises `+8`.
    AddrOf,
}

impl Access {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::Write => "WRITE",
            Self::ReadWrite => "RW",
            Self::AddrOf => "ADDR",
        }
    }
    /// Does this access modify the field?
    pub fn writes(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub func: u64,
    pub origins: Vec<Origin>,
    /// The accessor / caller / vptr that pulled this function in.
    pub via: Vec<u64>,
    pub class: Option<String>,
    pub vslot: Option<usize>,
    /// The object is believed to arrive in RDI at function entry.
    pub entry_this: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldHit {
    pub func: u64,
    pub site: u64,
    pub access: Access,
    /// Base register text (`rdi`, `r13`, …), or `rip` for an absolute hit.
    pub base: String,
    pub disp: i64,
    /// An indexed operand (`[rdi+rax*4+8]`) is an ARRAY step, not a plain field - flagged, not hidden.
    pub indexed: bool,
    /// Operand size in bytes, when iced reports one.
    pub size: u32,
    /// ⭐ The base register provably held the object here. `false` = displacement matched only `[U]`.
    pub tracked: bool,
    /// RIP-relative straight onto `global + offset` - the only class an absolute xref could see.
    pub absolute: bool,
    pub origins: Vec<Origin>,
    pub text: String,
    pub bytes: String,
}

/// What a pinned control expects the sweep to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Expect {
    /// A **tracked** field access at `site`, whose bytes are still `bytes`.
    Access { site: u64, bytes: &'static [u8] },
    /// The field's ADDRESS handed to `callee` at `site` - see [`Escape`].
    Escape { site: u64, callee: u64, bytes: &'static [u8] },
    /// **ZERO** tracked accesses - while still producing at least `min_matches` raw displacement
    /// candidates, which is what proves the sweep RAN rather than quietly doing nothing.
    Empty { min_matches: usize },
}

/// A pinned, known-true (or known-false) field query, run through the identical code path.
///
/// ⭐⭐ **THE AXIS THESE CONTROLS PROTECT, IN WORDS:** *field-xref must find register-relative
/// field access - `mov rax,[rdi+8]`, and `[rbx]` where `rbx` was `add`ed to the field - that an
/// absolute-address xref is blind to **by construction**, and it must NOT simply return every
/// `[reg+N]` it walks past.*
///
/// A single "the sweep returned rows" control would be decoration: it passes just as well for a
/// tool that reports everything. So the pinned set is a **discriminating pair**. The positives name
/// specific sites reachable only through the tracker; the inverted one runs the *same* sweep over
/// the *same* candidate set at an offset the object does not have, and demands **nothing tracked
/// while 22 displacement candidates are still seen**. Break the tracker so it trusts every base
/// register and the inverted control fails; break it so it trusts none and the positives fail.
/// Neither half can be satisfied by a degenerate implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FieldControl {
    pub class: &'static str,
    pub global: u64,
    pub offset: i64,
    pub expect: Expect,
    pub what: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlResult {
    pub control: FieldControl,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub class: Option<String>,
    pub global: Option<u64>,
    pub offset: i64,
    /// Functions that materialise the global RIP-relatively.
    pub accessors: Vec<u64>,
    pub candidates: Vec<Candidate>,
    pub hits: Vec<FieldHit>,
    /// ⭐ Where the field's ADDRESS was handed to another function. See [`Escape`] - for a field
    /// that is filled in by an allocator, this is the only place a "writer" appears at all.
    pub escapes: Vec<Escape>,
    /// Where a candidate's decode stopped on an undecodable byte. Read
    /// [`Truncation::after_terminal`] before believing the scan was incomplete.
    pub truncated: Vec<Truncation>,
    pub controls: Vec<ControlResult>,
    /// ⚠ `false` means this run may not be cited as evidence of absence.
    pub is_evidence: bool,
}

impl Report {
    /// The rows that carry evidential weight: base register proven, or RIP-absolute.
    pub fn tracked(&self) -> impl Iterator<Item = &FieldHit> {
        self.hits.iter().filter(|h| h.tracked || h.absolute)
    }
    /// Rows that could change the field's value: a store, or the address being produced.
    /// ⚠ [`Report::escapes`] is the other half - an address handed to a callee that writes it.
    pub fn writers(&self) -> impl Iterator<Item = &FieldHit> {
        self.tracked().filter(|h| h.access.writes() || h.access == Access::AddrOf)
    }
}

// ───────────────────────────── candidate resolution ─────────────────────────────

/// Volatile (caller-saved) registers under System V: destroyed across every call.
const VOLATILE: [Register; 9] = [
    Register::RAX,
    Register::RCX,
    Register::RDX,
    Register::RSI,
    Register::RDI,
    Register::R8,
    Register::R9,
    Register::R10,
    Register::R11,
];

fn reg64(r: Register) -> Register {
    if r.is_gpr() {
        r.full_register()
    } else {
        r
    }
}

/// Merge one resolution route into the candidate set. A function may arrive by several routes and
/// keeps all of them - which route found it is part of the answer, not an implementation detail.
fn add(
    cands: &mut BTreeMap<u64, Candidate>,
    f: u64,
    o: Origin,
    via: Option<u64>,
    cls: Option<String>,
    slot: Option<usize>,
) {
    let c = cands.entry(f).or_insert_with(|| Candidate {
        func: f,
        origins: Vec::new(),
        via: Vec::new(),
        class: None,
        vslot: None,
        entry_this: false,
    });
    if !c.origins.contains(&o) {
        c.origins.push(o);
    }
    if let Some(v) = via {
        if !c.via.contains(&v) {
            c.via.push(v);
        }
    }
    if c.class.is_none() {
        c.class = cls;
    }
    if c.vslot.is_none() {
        c.vslot = slot;
    }
    // A vfunc, a ctor and a forwarded callee all receive the object in RDI.
    if matches!(o, Origin::ClassVfunc | Origin::ClassVptr | Origin::AccessorForwarded) {
        c.entry_this = true;
    }
}

/// Functions that RIP-relatively reference `global`, confirmed by decode.
///
/// These are the accessors *and* any function that reaches the singleton directly. Both hold the
/// object, so both are candidates.
///
/// ⛔⛔ **THE PROLOGUE TRAP, AND IT BIT THIS MODULE ON ITS FIRST RUN** (CLAUDE.md: *"the prologue
/// count is not the function count… a prologue-only lookup names the PREVIOUS function"*).
/// [`crate::xref`]'s `Hit::function` **used to be** `nearest_prologue`, and the accessor we are
/// looking for - `FUN_0033FF30`, two instructions long, `lea rax,[rip+…]; ret` - **has no
/// `55 48 89 e5`**. Using `Hit::function` reported the accessor as `0x33fb20`, the unrelated
/// function before it, so `callers()` found the wrong 8 functions and the whole sweep silently
/// missed its own control.
///
/// ⭐ `Hit::function` was fixed on 2026-08-11 (it is now [`Index::containing`], the same index this
/// module reaches for) after the identical defect published a wrong "this code is dead" verdict
/// from the `fn` column of `xref`. The explicit [`Index::function_of`] below is kept anyway: the
/// index is already in hand, and a local answer cannot silently drift with someone else's
/// scanner - which is exactly what happened the first time.
fn global_refs(image: &Image, index: &Index, global: u64) -> Result<BTreeSet<u64>> {
    let starts = index.starts();
    Ok(xref::scan_kind(image, global, RefKind::RipRel)?
        .into_iter()
        .filter(|h| h.confirmed)
        .filter_map(|h| Index::function_of(&starts, h.site))
        .collect())
}

/// Assemble the candidate set. `depth` is the number of argument-forwarding hops (0 = none).
#[allow(clippy::too_many_arguments)]
fn resolve(
    image: &Image,
    index: &Index,
    rtti: &Rtti,
    class: Option<&str>,
    global: Option<u64>,
    depth: usize,
) -> Result<(Vec<u64>, BTreeMap<u64, Candidate>)> {
    let starts = index.starts();
    let mut cands: BTreeMap<u64, Candidate> = BTreeMap::new();

    // (a) the RTTI class: every vtable slot, plus every function referencing its vptr.
    if let Some(name) = class {
        let want: Vec<&crate::rtti::TypeInfo> =
            rtti.types.iter().filter(|t| t.name == name).collect();
        let tis: HashSet<u64> = want.iter().map(|t| t.ti).collect();
        // ⚠ CLAUDE.md trap 7: a ctor loads `vptr + 0x10`, NOT the RTTI-reported vtable address.
        // Both spellings must be admitted or every constructor goes missing.
        let mut vptrs: HashSet<u64> = HashSet::new();
        for vt in rtti.vtables.iter().filter(|v| tis.contains(&v.ti)) {
            vptrs.insert(vt.vptr);
            vptrs.insert(vt.vptr.wrapping_sub(0x10));
            for (i, &f) in vt.slots.iter().enumerate() {
                if f != 0 {
                    add(&mut cands, f, Origin::ClassVfunc, Some(vt.vptr), Some(name.to_string()), Some(i));
                }
            }
        }
        for &(f, t) in &index.riprefs {
            if vptrs.contains(&t) {
                add(&mut cands, f, Origin::ClassVptr, Some(t), Some(name.to_string()), None);
            }
        }
    }

    // (b) the singleton global: who materialises it, and who calls them.
    let mut accessors: Vec<u64> = Vec::new();
    if let Some(g) = global {
        let refs = global_refs(image, index, g)?;
        accessors = refs.iter().copied().collect();
        let callers = index.callers();
        for &a in &refs {
            add(&mut cands, a, Origin::GlobalRef, None, None, None);
            for &c in callers.get(&a).map(|v| v.as_slice()).unwrap_or(&[]) {
                add(&mut cands, c, Origin::AccessorCaller, Some(a), None, None);
            }
        }
    }

    // (c) one (or more) hops of argument forwarding: the tracker names the callees that received
    //     the object in RDI. Each hop re-scans only the functions the previous hop added.
    let accessor_set: HashSet<u64> = accessors.iter().copied().collect();
    let bounds = bounds_map(index);
    let mut frontier: Vec<u64> = cands.keys().copied().collect();
    for _ in 0..depth {
        let mut next: BTreeSet<u64> = BTreeSet::new();
        for f in &frontier {
            let entry_this = cands.get(f).map(|c| c.entry_this).unwrap_or(false);
            let Some(&(start, bound)) = bounds.get(f) else { continue };
            let mut sink = FnScan::new(start, bound, entry_this, &accessor_set, global, None);
            sink.run(image)?;
            for t in sink.forwarded {
                if !cands.contains_key(&t) {
                    next.insert(t);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        for t in &next {
            // Resolve to a known function start; a forwarded target is always a call target, so
            // `functions::build` already admitted it.
            let f = Index::function_of(&starts, *t).unwrap_or(*t);
            add(&mut cands, f, Origin::AccessorForwarded, None, None, None);
        }
        frontier = next.into_iter().collect();
    }

    Ok((accessors, cands))
}

fn bounds_map(index: &Index) -> HashMap<u64, (u64, u64)> {
    index.funcs.iter().map(|f| (f.start, (f.start, f.bound))).collect()
}

// ───────────────────────────── the per-function scan ─────────────────────────────

/// Linear, branch-oblivious tracking of "which register holds the object", plus the field sweep.
///
/// Mirrors [`crate::functions`]'s `this` propagation deliberately: that tracker already ships as
/// `d1-eboot thiscalls`, so the two agree by construction rather than by coincidence.
struct FnScan<'a> {
    start: u64,
    bound: u64,
    accessors: &'a HashSet<u64>,
    global: Option<u64>,
    /// `Some(offset)` to record field hits; `None` to only collect forwarding edges.
    offset: Option<i64>,
    /// register → **byte offset from the object base** it currently holds.
    ///
    /// ⭐⭐ Not a set. The offset is the whole point: `0x33FF4C add rbx,8` makes `rbx` point at the
    /// field, and every later access spells it `[rbx]` with **displacement 0**. ATLAS §4.2c is
    /// precisely this - *"a displacement scan is structurally blind to `disp == 0`"* - and it cost
    /// three passes once already. Carrying the offset in the tracker closes it: `[rbx]` with
    /// `rbx -> +8` is recognised as an access to field `+8`.
    held: HashMap<Register, i64>,
    /// `[rbp-N]` spill slot → offset from the object base. `this` is spilled constantly, because
    /// RDI is volatile and dies at the first call.
    slots: HashMap<i64, i64>,
    pub hits: Vec<FieldHit>,
    pub escapes: Vec<Escape>,
    pub forwarded: Vec<u64>,
    /// `Some(site)` when decoding stopped on an undecodable byte.
    pub truncated: Option<u64>,
    /// A terminal instruction (`ret`/`jmp`) was decoded before that byte.
    pub after_terminal: bool,
}

/// Where a candidate's decode stopped, and whether that is a real gap or just padding.
///
/// ⚠ A function's scan bound is the **next function's start**, so the bytes after its final `ret`
/// are alignment filler that decodes into nonsense. Reporting that as "INCOMPLETE" cries wolf on
/// every leaf accessor in the image (`0x78970` = `mov rax,[rdi+8]; ret` was the first one). But
/// silently suppressing it would hide a genuinely dirty decode, so both are reported and
/// distinguished rather than one being chosen for the reader.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Truncation {
    pub func: u64,
    pub site: u64,
    /// `true` = the function had already returned; the stop is almost certainly padding.
    pub after_terminal: bool,
}

/// The field's **address** left the function in an argument register.
///
/// This is how `c_activity_loader::vfunc[1]` "writes" `+8`: it never stores to it, it hands
/// `object+8` to an allocator (`FUN_0023DE00`) which writes through the pointer. A field-access
/// scan that only reported `mov [reg+8], …` would report **no writer at all** and be believed.
#[derive(Debug, Clone, Serialize)]
pub struct Escape {
    pub func: u64,
    pub site: u64,
    /// The callee that receives the field address.
    pub callee: u64,
    /// Which argument register carried it (`rdi` = arg0).
    pub reg: String,
    pub arg_index: usize,
    pub text: String,
}

/// System V integer argument registers, in order.
const ARGS: [Register; 6] = [
    Register::RDI,
    Register::RSI,
    Register::RDX,
    Register::RCX,
    Register::R8,
    Register::R9,
];

/// The immediate operand `i`, sign-extended, when operand `i` is one.
fn imm_of(ins: &Instruction, i: u32) -> Option<i64> {
    match ins.op_kind(i) {
        OpKind::Immediate8
        | OpKind::Immediate8_2nd
        | OpKind::Immediate16
        | OpKind::Immediate32
        | OpKind::Immediate64 => Some(ins.immediate(i) as i64),
        OpKind::Immediate8to16 | OpKind::Immediate8to32 | OpKind::Immediate8to64 => {
            Some(ins.immediate(i) as i64)
        }
        OpKind::Immediate32to64 => Some(ins.immediate32to64()),
        _ => None,
    }
}

/// Does this instruction end a basic path such that undecodable bytes after it are PADDING, not a
/// truncated scan? A function's `bound` is the *next* function's start, so the gap after `ret` is
/// alignment filler and decoding it produces garbage that must not be reported as an incomplete
/// scan (`0x78970` - `mov rax,[rdi+8]; ret` - tripped exactly this on the first run).
fn is_terminal(ins: &Instruction) -> bool {
    matches!(
        ins.mnemonic(),
        Mnemonic::Ret | Mnemonic::Retf | Mnemonic::Jmp | Mnemonic::Ud2 | Mnemonic::Int3 | Mnemonic::Hlt
    )
}

impl<'a> FnScan<'a> {
    fn new(
        start: u64,
        bound: u64,
        entry_this: bool,
        accessors: &'a HashSet<u64>,
        global: Option<u64>,
        offset: Option<i64>,
    ) -> Self {
        let mut held = HashMap::new();
        if entry_this {
            held.insert(Register::RDI, 0);
        }
        Self {
            start,
            bound,
            accessors,
            global,
            offset,
            held,
            slots: HashMap::new(),
            hits: Vec::new(),
            escapes: Vec::new(),
            forwarded: Vec::new(),
            truncated: None,
            after_terminal: false,
        }
    }

    fn run(&mut self, image: &Image) -> Result<()> {
        let len = (self.bound.saturating_sub(self.start)) as usize;
        if len == 0 {
            return Ok(());
        }
        let blob = match image.bytes(self.start, len + 15).or_else(|_| image.bytes(self.start, len))
        {
            Ok(b) => b,
            Err(_) => return Ok(()),
        };
        let mut dec = Decoder::with_ip(64, blob, self.start, DecoderOptions::NONE);
        let mut ins = Instruction::default();
        let mut info = InstructionInfoFactory::new();
        let mut fmt = IntelFormatter::new();
        fmt.options_mut().set_uppercase_hex(false);
        fmt.options_mut().set_hex_prefix("0x");
        fmt.options_mut().set_hex_suffix("");
        fmt.options_mut().set_space_after_operand_separator(true);
        fmt.options_mut().set_rip_relative_addresses(true);
        // Jump tables are DATA in the middle of code. Not skipping them truncates the scan, and a
        // truncated scan is a silent false negative - the exact failure this crate exists to avoid.
        let mut tables: HashSet<u64> = HashSet::new();
        let mut seen_terminal = false;

        while dec.can_decode() {
            let ip = dec.ip();
            if ip >= self.bound {
                break;
            }
            if tables.contains(&ip) {
                if let Some(n) = functions::jump_table_len(image, ip, self.start, self.bound) {
                    let after = ip + 4 * n as u64;
                    let Ok(rest) = image.bytes(after, (self.bound - after) as usize + 15) else {
                        break;
                    };
                    dec = Decoder::with_ip(64, rest, after, DecoderOptions::NONE);
                    continue;
                }
            }
            dec.decode_out(&mut ins);
            if ins.is_invalid() {
                self.truncated = Some(ip);
                self.after_terminal = seen_terminal;
                break;
            }
            seen_terminal |= is_terminal(&ins);
            if ins.is_ip_rel_memory_operand() {
                let t = ins.ip_rel_memory_address();
                if t > ip && t < self.bound {
                    tables.insert(t);
                }
            }

            self.record(image, &ins, &mut info, &mut fmt)?;
            self.propagate(&ins, &mut fmt);
        }
        Ok(())
    }

    /// Record every memory operand that resolves to the requested field.
    ///
    /// Two ways in: the base register is **tracked**, in which case the field offset is
    /// `held[base] + disp` and the row is `[C]`; or the base is unknown and the raw displacement
    /// happens to equal the offset, in which case the row is `[U]` and says so.
    fn record(
        &mut self,
        image: &Image,
        ins: &Instruction,
        info: &mut InstructionInfoFactory,
        fmt: &mut IntelFormatter,
    ) -> Result<()> {
        let Some(want) = self.offset else { return Ok(()) };
        let ip = ins.ip();
        let mut text: Option<String> = None;

        let used: Vec<_> = info.info(ins).used_memory().to_vec();
        for m in used {
            let base = reg64(m.base());
            let disp = m.displacement() as i64;
            let abs = self.global.is_some_and(|g| {
                base == Register::RIP
                    && ins.is_ip_rel_memory_operand()
                    && ins.ip_rel_memory_address() == (g as i64 + want) as u64
            });
            let tracked_off = self.held.get(&base).copied();
            if !abs {
                if base == Register::RIP || base == Register::None {
                    continue;
                }
                match tracked_off {
                    // A tracked base is authoritative: `[rbx+8]` where `rbx -> +8` is field +0x10,
                    // NOT field +8, and reporting it would be worse than not reporting it.
                    Some(k) => {
                        if k + disp != want {
                            continue;
                        }
                    }
                    None => {
                        if disp != want {
                            continue;
                        }
                    }
                }
            }
            let access = match m.access() {
                OpAccess::Write | OpAccess::CondWrite => Access::Write,
                OpAccess::ReadWrite | OpAccess::ReadCondWrite => Access::ReadWrite,
                OpAccess::NoMemAccess => Access::AddrOf,
                _ => Access::Read,
            };
            if text.is_none() {
                let mut s = String::new();
                fmt.format(ins, &mut s);
                text = Some(s);
            }
            let bytes = image
                .bytes(ip, ins.len())
                .map(|b| b.iter().map(|x| format!("{x:02x}")).collect::<String>())
                .unwrap_or_default();
            self.hits.push(FieldHit {
                func: self.start,
                site: ip,
                access,
                base: format!("{base:?}").to_lowercase(),
                disp,
                indexed: m.index() != Register::None,
                size: m.memory_size().size() as u32,
                tracked: abs || tracked_off.is_some(),
                absolute: abs,
                origins: Vec::new(),
                text: text.clone().unwrap_or_default(),
                bytes,
            });
        }
        Ok(())
    }

    /// Update the register → object-offset map, and note forwarding and escapes.
    fn propagate(&mut self, ins: &Instruction, fmt: &mut IntelFormatter) {
        // Seed: a RIP-relative `lea` onto the global materialises the object into a register.
        if let Some(g) = self.global {
            if ins.mnemonic() == Mnemonic::Lea
                && ins.is_ip_rel_memory_operand()
                && ins.op0_kind() == OpKind::Register
            {
                let t = ins.ip_rel_memory_address();
                // `lea reg,[rip+X]` where X is inside the object gives `reg -> X - g`. Only the
                // exact base is admitted: a "nearby global" is a different object, not a field.
                if t == g {
                    self.held.insert(reg64(ins.op0_register()), 0);
                    return;
                }
            }
        }

        let m = ins.mnemonic();
        match m {
            Mnemonic::Mov => match (ins.op0_kind(), ins.op1_kind()) {
                (OpKind::Register, OpKind::Register) => {
                    let d = reg64(ins.op0_register());
                    match self.held.get(&reg64(ins.op1_register())).copied() {
                        Some(k) => {
                            self.held.insert(d, k);
                        }
                        None => {
                            self.held.remove(&d);
                        }
                    }
                }
                (OpKind::Memory, OpKind::Register) => {
                    if ins.memory_base() == Register::RBP && ins.memory_index() == Register::None {
                        let d = ins.memory_displacement64() as i64;
                        match self.held.get(&reg64(ins.op1_register())).copied() {
                            Some(k) => {
                                self.slots.insert(d, k);
                            }
                            None => {
                                self.slots.remove(&d);
                            }
                        }
                    }
                }
                (OpKind::Register, OpKind::Memory) => {
                    let from_slot = if ins.memory_base() == Register::RBP
                        && ins.memory_index() == Register::None
                    {
                        self.slots.get(&(ins.memory_displacement64() as i64)).copied()
                    } else {
                        None
                    };
                    let d = reg64(ins.op0_register());
                    match from_slot {
                        Some(k) => {
                            self.held.insert(d, k);
                        }
                        None => {
                            self.held.remove(&d);
                        }
                    }
                }
                _ => {}
            },
            // ⭐ POINTER ARITHMETIC - the ATLAS §4.2c case. `lea rD,[rS+disp]` and `add rD,imm`
            // move the tracked offset instead of destroying it.
            Mnemonic::Lea => {
                if ins.op0_kind() == OpKind::Register
                    && ins.memory_index() == Register::None
                    && !ins.is_ip_rel_memory_operand()
                {
                    let d = reg64(ins.op0_register());
                    let src = reg64(ins.memory_base());
                    match self.held.get(&src).copied() {
                        Some(k) => {
                            let off = k + ins.memory_displacement64() as i64;
                            self.held.insert(d, off);
                        }
                        None => {
                            self.held.remove(&d);
                        }
                    }
                } else if ins.op0_kind() == OpKind::Register {
                    let d = reg64(ins.op0_register());
                    self.held.remove(&d);
                }
            }
            Mnemonic::Add | Mnemonic::Sub => {
                if ins.op0_kind() == OpKind::Register {
                    let d = reg64(ins.op0_register());
                    match (self.held.get(&d).copied(), imm_of(ins, 1)) {
                        (Some(k), Some(i)) => {
                            let off = if m == Mnemonic::Add { k + i } else { k - i };
                            self.held.insert(d, off);
                        }
                        _ => {
                            self.held.remove(&d);
                        }
                    }
                }
            }
            _ => {
                if ins.op_count() > 0 && ins.op0_kind() == OpKind::Register {
                    self.held.remove(&reg64(ins.op0_register()));
                }
            }
        }

        if m == Mnemonic::Call && ins.op0_kind() == OpKind::NearBranch64 {
            let t = ins.near_branch_target();
            // The object itself in arg0 → the callee is a candidate (one forwarding hop).
            if self.held.get(&Register::RDI) == Some(&0) {
                self.forwarded.push(t);
            }
            // The FIELD ADDRESS in any argument register → the callee may write through it.
            if let Some(want) = self.offset {
                for (i, r) in ARGS.iter().enumerate() {
                    if self.held.get(r) == Some(&want) {
                        let mut s = String::new();
                        fmt.format(ins, &mut s);
                        self.escapes.push(Escape {
                            func: self.start,
                            site: ins.ip(),
                            callee: t,
                            reg: format!("{r:?}").to_lowercase(),
                            arg_index: i,
                            text: s,
                        });
                    }
                }
            }
            for r in VOLATILE {
                self.held.remove(&r);
            }
            // An accessor RETURNS the object in RAX. This is the edge `xref` cannot follow and the
            // reason a caller of `FUN_0033FF30` is a candidate at all.
            if self.accessors.contains(&t) {
                self.held.insert(Register::RAX, 0);
            }
        }
        if m == Mnemonic::Jmp && ins.op0_kind() == OpKind::NearBranch64 {
            let t = ins.near_branch_target();
            if (t < self.start || t >= self.bound) && self.held.get(&Register::RDI) == Some(&0) {
                self.forwarded.push(t);
            }
        }
    }
}

// ───────────────────────────── the driver ─────────────────────────────

/// Full field sweep. Pass `controls` empty only when the caller has explicitly opted out -
/// [`Report::is_evidence`] is then `false` and the result may not be cited as absence.
pub fn scan(
    image: &Image,
    index: &Index,
    rtti: &Rtti,
    class: Option<&str>,
    global: Option<u64>,
    offset: i64,
    depth: usize,
) -> Result<Report> {
    let (accessors, cands) = resolve(image, index, rtti, class, global, depth)?;
    let accessor_set: HashSet<u64> = accessors.iter().copied().collect();
    let bounds = bounds_map(index);

    let mut hits: Vec<FieldHit> = Vec::new();
    let mut escapes: Vec<Escape> = Vec::new();
    let mut truncated: Vec<Truncation> = Vec::new();
    for (f, c) in &cands {
        let Some(&(start, bound)) = bounds.get(f) else { continue };
        let mut s =
            FnScan::new(start, bound, c.entry_this, &accessor_set, global, Some(offset));
        s.run(image)?;
        if let Some(site) = s.truncated {
            truncated.push(Truncation { func: *f, site, after_terminal: s.after_terminal });
        }
        for mut h in s.hits {
            h.origins = c.origins.clone();
            hits.push(h);
        }
        escapes.extend(s.escapes);
    }
    escapes.sort_by_key(|e| e.site);
    escapes.dedup_by_key(|e| e.site);

    // ⭐ The absolute class, image-wide. `object + offset` of a STATIC singleton is a real address,
    // so some accesses ARE RIP-relative - these are the ones `xref` could already see, and leaving
    // them out would make field-xref's answer narrower than the tool it complements.
    if let Some(g) = global {
        let field_addr = (g as i64 + offset) as u64;
        let starts = index.starts();
        for h in xref::scan_kind(image, field_addr, RefKind::RipRel)?
            .into_iter()
            .filter(|h| h.confirmed)
        {
            // Same prologue trap as `global_refs`: attribute by the function index, not
            // `Hit::function`, or an accessor without a `55 48 89 e5` is credited to its neighbour.
            let func = Index::function_of(&starts, h.site).or(h.function).unwrap_or(0);
            if hits.iter().any(|x| x.site == h.site && x.absolute) {
                continue;
            }
            let access = if h.mnemonic == "lea" {
                Access::AddrOf
            } else if h.op_str.starts_with('[') {
                Access::Write
            } else {
                Access::Read
            };
            hits.push(FieldHit {
                func,
                site: h.site,
                access,
                base: "rip".into(),
                disp: offset,
                indexed: false,
                size: 0,
                tracked: true,
                absolute: true,
                origins: cands.get(&func).map(|c| c.origins.clone()).unwrap_or_default(),
                text: format!("{} {}", h.mnemonic, h.op_str),
                bytes: String::new(),
            });
        }
    }

    hits.sort_by_key(|h| (h.site, h.disp));
    hits.dedup_by_key(|h| (h.site, h.disp, h.access as u8));
    truncated.sort_by_key(|t| t.func);

    Ok(Report {
        class: class.map(|s| s.to_string()),
        global,
        offset,
        accessors,
        candidates: cands.into_values().collect(),
        hits,
        escapes,
        truncated,
        controls: Vec::new(),
        is_evidence: false,
    })
}

// ───────────────────────────── the pinned controls ─────────────────────────────

/// ⭐⭐ **POSITIVE 1 - the argument-forwarded reader.** `FUN_00340240` is `mov rax,[rdi+8]; ret`.
/// It is **not** in `c_activity_loader`'s vtable, it is **not** the accessor, and its four bytes
/// `48 8b 47 08` contain **no address at all**. The only route to it is: accessor → caller →
/// object in RDI at a call → callee. `d1-eboot xref 0x267af10` - the absolute address of this very
/// field - does not report `0x340240` and cannot. If this control passes while the forwarding hop
/// is broken, the tool has stopped doing the one thing it was built for.
pub const CONTROL_FORWARDED_READ: FieldControl = FieldControl {
    class: "c_activity_loader",
    global: 0x267_AF08,
    offset: 8,
    expect: Expect::Access { site: 0x340_240, bytes: &[0x48, 0x8B, 0x47, 0x08] },
    what: "FUN_00340240 `mov rax,[rdi+8]` - reachable ONLY by argument forwarding",
};

/// ⭐⭐ **POSITIVE 2 - the folded displacement.** `c_activity_loader::vfunc[1]` does
/// `0x33FF49 mov rbx,rdi` / `0x33FF4C add rbx,8`, and then spells the field **`[rbx]`** -
/// displacement **zero**. `historicaldocs/design/ATLAS.md` §4.2c is exactly this case (*"a displacement scan is
/// structurally blind to `disp == 0`"*, cost: three passes), so a scanner that matched only on the
/// literal displacement byte would miss the class's own initialiser. Bytes `48 8b 03`.
pub const CONTROL_FOLDED_READ: FieldControl = FieldControl {
    class: "c_activity_loader",
    global: 0x267_AF08,
    offset: 8,
    expect: Expect::Access { site: 0x33F_FA2, bytes: &[0x48, 0x8B, 0x03] },
    what: "c_activity_loader::vfunc[1] `mov rax,[rbx]` with rbx = object+8 (ATLAS §4.2c, disp==0)",
};

/// ⭐⭐ **POSITIVE 3 - the escape, which is the only \"writer\" that exists.** Nothing in the image
/// stores to `c_activity_loader+8` with a `mov [reg+8], …`. `vfunc[1]` hands the field's ADDRESS to
/// `FUN_0023DE00` in RDI and that function writes through it. A field scan that reported only
/// stores would answer *"nobody writes this field"* - confidently, and wrongly.
pub const CONTROL_ESCAPE: FieldControl = FieldControl {
    class: "c_activity_loader",
    global: 0x267_AF08,
    offset: 8,
    expect: Expect::Escape { site: 0x33F_F9D, callee: 0x23D_E00, bytes: &[0xE8, 0x5E, 0xDE, 0xEF, 0xFF] },
    what: "the field ADDRESS escapes into FUN_0023DE00 - the actual initialiser",
};

/// ⭐⭐ **THE INVERTED CONTROL.** Same class, same global, same 67 candidate functions, same code
/// path - an offset the object does not have. It must return **zero tracked accesses**.
///
/// **Hand-verified that `+0x20` is untouched** (2026-08-09): `c_activity_loader`'s object at
/// `0x267AF08` is `{vptr@+0, pointer@+8}`; `0x267AF18` and `0x267AF20` are *separate globals* with
/// their own RIP-relative references (1 and 3 respectively), so `+0x20` is past the end of the
/// object. An offset histogram over `+0x00..+0x40` in steps of 4 puts the object's real traffic at
/// `+0` (2) and `+8` (24) and nothing at `+0x20`.
///
/// ⭐ `min_matches` is the half that makes this a control instead of a tautology: the sweep must
/// still surface **22 raw displacement candidates** at `+0x20`. A tool that had simply stopped
/// working would return zero of those too, and pass a bare "is it empty?" assertion.
pub const CONTROL_INVERTED: FieldControl = FieldControl {
    class: "c_activity_loader",
    global: 0x267_AF08,
    offset: 0x20,
    expect: Expect::Empty { min_matches: 20 },
    what: "offset +0x20 is past the end of the object - tracked MUST be 0, candidates MUST NOT be",
};

pub const PINNED: [FieldControl; 4] =
    [CONTROL_FORWARDED_READ, CONTROL_FOLDED_READ, CONTROL_ESCAPE, CONTROL_INVERTED];

/// Run one pinned control through the *identical* code path a user's query takes.
pub fn run_control(
    image: &Image,
    index: &Index,
    rtti: &Rtti,
    c: &FieldControl,
) -> Result<ControlResult> {
    // ── the raw byte-level half ────────────────────────────────────────────────────────────────
    // Pinned the way `xref`'s controls are: the named instruction must still BE those bytes at that
    // address. This is what catches a different build, or a per-segment bias regression, before any
    // conclusion is drawn from the sweep (ATLAS §4.3 - re-derive, never trust a transcription).
    let byte_check = match c.expect {
        Expect::Access { site, bytes } | Expect::Escape { site, bytes, .. } => {
            let got = image.bytes(site, bytes.len())?;
            if got != bytes {
                Some(format!(
                    "BYTES at {site:#x} are {got:02x?}, pinned {bytes:02x?} - wrong image or wrong address"
                ))
            } else {
                None
            }
        }
        Expect::Empty { .. } => None,
    };
    if let Some(detail) = byte_check {
        return Ok(ControlResult { control: *c, passed: false, detail });
    }

    let r = scan(image, index, rtti, Some(c.class), Some(c.global), c.offset, 1)?;
    let tracked: Vec<&FieldHit> = r.tracked().collect();
    let (passed, detail) = match c.expect {
        Expect::Access { site, .. } => {
            let found = tracked.iter().any(|h| h.site == site);
            (
                found,
                format!(
                    "{} tracked access(es); site {site:#x} {}",
                    tracked.len(),
                    if found { "FOUND" } else { "MISSING" }
                ),
            )
        }
        Expect::Escape { site, callee, .. } => {
            let found = r.escapes.iter().any(|e| e.site == site && e.callee == callee);
            (
                found,
                format!(
                    "{} escape(s); {site:#x} -> {callee:#x} {}",
                    r.escapes.len(),
                    if found { "FOUND" } else { "MISSING" }
                ),
            )
        }
        Expect::Empty { min_matches } => {
            let ran = r.hits.len() >= min_matches;
            (
                tracked.is_empty() && ran,
                format!(
                    "{} tracked (must be 0), {} raw displacement candidate(s) (must be >= {min_matches}){}",
                    tracked.len(),
                    r.hits.len(),
                    tracked
                        .first()
                        .map(|h| format!(" - leaked {:#x} {}", h.site, h.text))
                        .unwrap_or_default()
                ),
            )
        }
    };
    Ok(ControlResult { control: *c, passed, detail })
}

/// Every pinned control, positives and the inverted one alike.
pub fn controls(image: &Image, index: &Index, rtti: &Rtti) -> Result<Vec<ControlResult>> {
    PINNED.iter().map(|c| run_control(image, index, rtti, c)).collect()
}

/// Attach the pinned controls to a report and set [`Report::is_evidence`].
pub fn with_controls(
    image: &Image,
    index: &Index,
    rtti: &Rtti,
    mut r: Report,
) -> Result<Report> {
    r.controls = controls(image, index, rtti)?;
    r.is_evidence = !r.controls.is_empty() && r.controls.iter().all(|c| c.passed);
    Ok(r)
}
