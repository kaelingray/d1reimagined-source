// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Cross-reference scanning - **all four reference kinds**, with a mandatory positive control.
//!
//! Consolidates `capture-rig/find-direct-call-xrefs.py` (`call rel32` only) and
//! `capture-rig/oodle/global-xref.py` (RIP-relative only) and `oodle/reloc-xref.py`
//! (RELA addends only). Keeping them apart is what caused the bug they were written to prevent.
//!
//! ⚠ **`historicaldocs/design/ATLAS.md` §4.2 - cost: two sessions.** §26.9 concluded "the producer cannot be
//! found by reading the binary" because the scan checked only `call rel32`; the function was
//! entered by a **tail call** (`jmp` at `0x560D27`). Every reference sweep must cover:
//!
//! | kind | encoding | who needs it |
//! |---|---|---|
//! | [`RefKind::Call`] | `e8 rel32` | ordinary calls |
//! | [`RefKind::Jmp`] | `e9 rel32` | **tail calls** - the two-session bug |
//! | [`RefKind::RipRel`] | REX.W `8d`/`8b` + ModRM mod=00 rm=101, disp32 | globals, string literals |
//! | [`RefKind::Rela`] | `R_X86_64_RELATIVE` addend | vtable slots, which are **zero in the file** |
//!
//! ⚠ **A scan without a positive control is not evidence of absence** (§4.2), and
//! ⚠ **a passing control does not mean you asked about the right address** (§4.2b): the control
//! proves the *instrument*, never the *aim*. For a string target, run the target through
//! [`crate::image::Image::string_start_at_or_before`] first - `0x162E2C0` was the interior of a
//! literal that starts at `0x162E2B5`, and "0 references" was correct and useless.
//!
//! ⛔ **The `fn` column was wrong until 2026-08-11, and it cost a wrong published verdict.** It was
//! `nearest_prologue(site)`, so every site inside one of this image's 11,446 frame-pointer-omitted
//! functions was credited to the *preceding* prologue-anchored one. `xref 0x25c0c58` printed
//! `fn 0x1d1e80` for a site at `0x1d20a1`; `FUN_001D1E80` ends at `0x1D2020` and the site belongs
//! to `0x1D2030`. `0x1D1E80` **writes** that pool and `0x1D2030` **reads** it, so the pool looked
//! write-only and a session concluded the dispatch below it was unreachable dead code. It was
//! live. Attribution is now [`crate::functions::Index::containing`] - the index the atlas already
//! used - and it answers `fn -` rather than name a neighbour. ⇒ [`crate::functions::Containment`].
//!
//! ⚠ **A displacement scan is structurally blind to `disp == 0`** (§4.2c). `0x56A8A7
//! mov byte [r14], 2` was the whole answer to a question three passes missed, because `r14` was
//! `lea r14,[r12+rbx+0x88]` and the field's `+0x28` had been folded into the base. Pair any
//! displacement search with a per-function instruction dump over an independently enumerated
//! candidate set. Nothing in this module can see that case, and it does not pretend to.

use std::fmt;

use serde::Serialize;

use crate::disasm;
use crate::functions::Containing;
use crate::image::{Image, Result};
use crate::reloc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    Call,
    Jmp,
    RipRel,
    Rela,
}

impl RefKind {
    pub const ALL: [RefKind; 4] = [RefKind::Call, RefKind::Jmp, RefKind::RipRel, RefKind::Rela];
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "call" => Some(Self::Call),
            "jmp" | "tailcall" | "tail" => Some(Self::Jmp),
            "riprel" | "rip" | "lea" | "global" => Some(Self::RipRel),
            "rela" | "reloc" | "relocation" => Some(Self::Rela),
            _ => None,
        }
    }
}

impl fmt::Display for RefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Call => "call",
            Self::Jmp => "jmp",
            Self::RipRel => "riprel",
            Self::Rela => "rela",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub kind: RefKind,
    /// Instruction address for code refs; the relocated **slot** vaddr for `Rela`.
    pub site: u64,
    /// ⭐ The function that **contains** `site` - `Some` only when containment is proven, so a
    /// consumer that reads this field alone gets a right answer or none, never a neighbour's.
    ///
    /// ⛔ This was `nearest_prologue(site)` until 2026-08-11 and named the previous function for
    /// every site inside a frame-pointer-omitted one. See [`crate::functions::Containment`] for
    /// what that cost.
    pub function: Option<u64>,
    /// The full attribution, including the two honest failure answers (`Unproven`, `None`).
    pub containing: Containing,
    pub mnemonic: String,
    pub op_str: String,
    /// False when a candidate byte window did not turn out to be a real decoded reference.
    pub confirmed: bool,
}

/// A known-true reference of the same shape, run through the identical code path.
///
/// ⚠ Prefer an **independent** control (a different target whose site you already know) over one
/// that depends on the answer being sought - `global-xref.py` makes exactly this distinction and
/// it is the right one. A control that can only pass if the search succeeds is not a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Control {
    pub kind: RefKind,
    pub target: u64,
    pub site: u64,
}

impl Control {
    /// Pinned, re-derived controls for the 01.29 build. Each was verified against the image, not
    /// recalled from a document (ATLAS §4.3: *re-derive a constant rather than trusting a copy*).
    pub fn pinned(kind: RefKind) -> Option<Control> {
        Some(match kind {
            // `0x5719f2  call 0x5858c0` - read off the image.
            // ⚠ find-direct-call-xrefs.py's own docstring advertises `0x571940:0x5719f2`, which
            // is WRONG: the call at 0x5719f2 targets 0x5858c0, not 0x571940. That example control
            // FAILS, and a "fix" by editing the constant is ATLAS §6.1's failure mode exactly.
            RefKind::Call => Control { kind, target: 0x5858C0, site: 0x5719F2 },
            // ⭐ THE MANDATORY TAIL-CALL CONTROL (ATLAS §4.2): 0x560D27 -> 0x56FFB0.
            // 0x56FFB0 has ZERO `call rel32` sites; a call-only scanner reports "unreachable".
            RefKind::Jmp => Control { kind, target: 0x56FFB0, site: 0x560D27 },
            // global-xref.py's own independent control pair.
            RefKind::RipRel => Control { kind, target: 0x164CE3E, site: 0x7982DA },
            // `0x18f5260` is a slot whose R_X86_64_RELATIVE addend is `0x25a330`.
            RefKind::Rela => Control { kind, target: 0x25A330, site: 0x18F5260 },
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlResult {
    pub control: Control,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub target: u64,
    pub kinds: Vec<RefKind>,
    pub hits: Vec<Hit>,
    pub controls: Vec<ControlResult>,
    /// ⚠ `false` means this run may not be cited as evidence of absence.
    pub is_evidence: bool,
    /// Set when `target` is not the start of a NUL-terminated string it sits inside (ATLAS §4.2b).
    pub string_start_warning: Option<u64>,
}

impl ScanReport {
    pub fn confirmed(&self) -> impl Iterator<Item = &Hit> {
        self.hits.iter().filter(|h| h.confirmed)
    }
}

/// Every `E8`/`E9 rel32` byte window in an executable segment whose displacement resolves to
/// `target`. Candidates only - confirmation is by prologue-anchored disassembly.
fn branch_candidates(image: &Image, target: u64, opcode: u8) -> Vec<u64> {
    let mut out = Vec::new();
    for seg in image.executable_segments() {
        let r = seg.file_range();
        let blob = &image.data()[r.start..r.end.min(image.data().len())];
        if blob.len() < 5 {
            continue;
        }
        for i in 0..blob.len() - 5 {
            if blob[i] != opcode {
                continue;
            }
            let disp = i32::from_le_bytes(blob[i + 1..i + 5].try_into().unwrap()) as i64;
            let site = seg.vaddr + i as u64;
            if (site as i64 + 5 + disp) as u64 == target {
                out.push(site);
            }
        }
    }
    out
}

/// Every 4-byte window that, read as a `disp32` ending an instruction, resolves to `target`.
///
/// Instruction length is deliberately **not** guessed here (ATLAS §4.5) - the window is a
/// candidate, and the caller confirms by anchoring on the enclosing prologue.
fn riprel_candidates(image: &Image, target: u64) -> Vec<u64> {
    let mut out = Vec::new();
    for seg in image.executable_segments() {
        let r = seg.file_range();
        let blob = &image.data()[r.start..r.end.min(image.data().len())];
        if blob.len() < 4 {
            continue;
        }
        for i in 0..blob.len() - 4 {
            let disp = i32::from_le_bytes(blob[i..i + 4].try_into().unwrap()) as i64;
            let end_va = seg.vaddr + i as u64 + 4;
            if (end_va as i64 + disp) as u64 == target {
                out.push(seg.vaddr + i as u64);
            }
        }
    }
    out
}

fn confirm_branch(image: &Image, site: u64, target: u64, want_call: bool) -> Result<Hit> {
    let kind = if want_call { RefKind::Call } else { RefKind::Jmp };
    match disasm::instruction_at(image, site)? {
        Some((containing, insn)) => {
            let ok = insn.address == site && disasm::is_branch_to(&insn, target, want_call);
            Ok(Hit {
                kind,
                site,
                function: containing.proven_start(),
                containing,
                mnemonic: insn.mnemonic,
                op_str: insn.op_str,
                confirmed: ok,
            })
        }
        None => Ok(Hit {
            kind,
            site,
            function: None,
            containing: Containing::NONE,
            mnemonic: "<no prologue>".into(),
            op_str: String::new(),
            confirmed: false,
        }),
    }
}

fn confirm_riprel(image: &Image, disp_site: u64, target: u64) -> Result<Hit> {
    match disasm::instruction_at(image, disp_site)? {
        Some((containing, insn)) => {
            let ok = insn.rip_target == Some(target);
            Ok(Hit {
                kind: RefKind::RipRel,
                // report the INSTRUCTION address, as global-xref.py does
                site: if ok { insn.address } else { disp_site },
                function: containing.proven_start(),
                containing,
                mnemonic: insn.mnemonic,
                op_str: insn.op_str,
                confirmed: ok,
            })
        }
        None => Ok(Hit {
            kind: RefKind::RipRel,
            site: disp_site,
            function: None,
            containing: Containing::NONE,
            mnemonic: "<no prologue>".into(),
            op_str: String::new(),
            confirmed: false,
        }),
    }
}

/// Scan one kind. Used both for the real target and, unchanged, for the control.
pub fn scan_kind(image: &Image, target: u64, kind: RefKind) -> Result<Vec<Hit>> {
    Ok(match kind {
        RefKind::Call => branch_candidates(image, target, 0xE8)
            .into_iter()
            .map(|s| confirm_branch(image, s, target, true))
            .collect::<Result<Vec<_>>>()?,
        RefKind::Jmp => branch_candidates(image, target, 0xE9)
            .into_iter()
            .map(|s| confirm_branch(image, s, target, false))
            .collect::<Result<Vec<_>>>()?,
        RefKind::RipRel => riprel_candidates(image, target)
            .into_iter()
            .map(|s| confirm_riprel(image, s, target))
            .collect::<Result<Vec<_>>>()?,
        RefKind::Rela => reloc::by_addend(image)?
            .get(&target)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|slot| Hit {
                kind: RefKind::Rela,
                site: slot,
                function: None,
                // A relocation slot is DATA, not code: no function contains it, and saying so is
                // the point of `Containment::None`.
                containing: Containing::NONE,
                mnemonic: "R_X86_64_RELATIVE".into(),
                op_str: format!("slot {slot:#x} -> {target:#x}"),
                confirmed: true,
            })
            .collect(),
    })
}

/// ATLAS §4.2b: verify the **aim**, not only the instrument.
///
/// On 2026-08-07 a scan for a diagnostic format string at `0x162E2C0` returned 0 references with
/// an independent control **passing**, and the conclusion drawn was wrong: the literal starts at
/// `0x162E2B5` and `0x162E2C0` pointed into its interior. Nothing references a string interior, so
/// the scanner was right and the question was not. If `target` lands inside printable text whose
/// true start is earlier, say so before the caller reads "0 references" as absence.
pub fn string_interior_warning(image: &Image, target: u64) -> Option<u64> {
    let start = image.string_start_at_or_before(target, 512).ok()?;
    if start == target || target - start > 512 {
        return None;
    }
    let run = image.bytes(start, (target - start) as usize + 1).ok()?;
    if run.len() < 4 || !run.iter().all(|&b| (0x20..0x7F).contains(&b)) {
        return None;
    }
    Some(start)
}

/// Full scan with controls. `controls` may be empty, in which case the report is explicitly
/// marked `is_evidence: false` - a caller may still print it, but must not cite it as absence.
pub fn scan(
    image: &Image,
    target: u64,
    kinds: &[RefKind],
    controls: &[Control],
) -> Result<ScanReport> {
    let mut hits = Vec::new();
    for &k in kinds {
        hits.extend(scan_kind(image, target, k)?);
    }
    hits.sort_by_key(|h| (h.site, h.kind as u8));

    let mut results = Vec::new();
    for c in controls {
        let found = scan_kind(image, c.target, c.kind)?
            .into_iter()
            .any(|h| h.confirmed && h.site == c.site);
        results.push(ControlResult { control: *c, passed: found });
    }
    let is_evidence = !results.is_empty() && results.iter().all(|r| r.passed);

    let string_start_warning = string_interior_warning(image, target);

    Ok(ScanReport { target, kinds: kinds.to_vec(), hits, controls: results, is_evidence, string_start_warning })
}
