// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Prologue-anchored disassembly.
//!
//! Consolidates four Python disassemblers that had drifted apart:
//! `capture-rig/disassemble-elf-range.py` (range + truncation warning),
//! `capture-rig/oodle/dis-eboot.py` (vaddr convenience),
//! `capture-rig/oodle/dis-annot.py` (resolves RIP-relative operands to absolute vaddrs), and
//! `capture-rig/oodle/rsat-encoder-disasm.py` (refuses a start that is not a real prologue).
//!
//! ⚠ **Two rules, both paid for** (`historicaldocs/design/ATLAS.md` §4.5, and every one of those docstrings):
//!
//! 1. **Never sweep the whole image linearly.** One data island desyncs the decoder and every
//!    later instruction is authoritative-looking garbage - and the sweep *silently truncates*.
//!    Everything here is bounded to a range the caller named.
//! 2. **Never start from a guessed `va - N`.** [`disasm_range`] refuses a start that is not
//!    `55 48 89 e5` unless the caller passes [`Anchor::Forced`], and [`Anchor::Snap`] walks back
//!    to the real prologue first.
//!
//! Truncation is reported, never swallowed: [`Disassembly::truncated_at`] is `Some` when the
//! decoder stopped short of the requested end, which is the signal that a data island was hit.

use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter, Mnemonic, OpKind};
use serde::Serialize;

use crate::functions::Containing;
use crate::image::{Image, ImageError, Result, PROLOGUE};

/// How a disassembly start address is justified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The caller asserts `start` IS a prologue; verified, and refused if it is not.
    Prologue,
    /// Walk back to the nearest real prologue at or before `start`.
    Snap,
    /// Start exactly here regardless. Only for ranges that are known not to be function starts
    /// (a jump-table body, a data blob being examined as code).
    Forced,
}

#[derive(Debug, Clone, Serialize)]
pub struct Insn {
    pub address: u64,
    pub len: usize,
    pub bytes: String,
    pub mnemonic: String,
    pub op_str: String,
    /// Absolute target of a near branch/call, when there is one.
    pub branch_target: Option<u64>,
    /// Absolute address a RIP-relative memory operand resolves to (`dis-annot.py`'s whole point).
    pub rip_target: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Disassembly {
    pub start: u64,
    pub end: u64,
    /// The prologue this run was anchored on, when one was used.
    pub anchor: Option<u64>,
    pub insns: Vec<Insn>,
    /// `Some(addr)` when decoding stopped before `end` - i.e. a data island or an undecodable byte.
    pub truncated_at: Option<u64>,
}

pub fn is_prologue(image: &Image, va: u64) -> bool {
    image.bytes(va, PROLOGUE.len()).map(|b| b == PROLOGUE).unwrap_or(false)
}

/// The anchor mode for a start that is already known to be a real function start: verified when it
/// is a `55 48 89 e5`, forced when it is frame-pointer-omitted. Never call this on a guess.
pub fn anchor_for_known_start(image: &Image, start: u64) -> Anchor {
    if is_prologue(image, start) { Anchor::Prologue } else { Anchor::Forced }
}

/// Disassemble `[start, end)`.
///
/// `end` is exclusive; the last instruction may extend past it, matching the Python range tool.
pub fn disasm_range(image: &Image, start: u64, end: u64, anchor: Anchor) -> Result<Disassembly> {
    if end <= start {
        return Err(ImageError::Unmapped { vaddr: end });
    }
    let (start, anchor_va) = match anchor {
        Anchor::Prologue => {
            if !is_prologue(image, start) {
                return Err(ImageError::NotAPrologue { vaddr: start });
            }
            (start, Some(start))
        }
        Anchor::Snap => {
            let p = image
                .nearest_prologue(start, 0x4000)
                .ok_or(ImageError::NotAPrologue { vaddr: start })?;
            (p, Some(p))
        }
        Anchor::Forced => (start, None),
    };

    let len = (end.saturating_sub(start)) as usize;
    // +32 so the instruction straddling `end` decodes whole, as the Python tools do.
    let avail = image.bytes(start, len + 32).or_else(|_| image.bytes(start, len))?;
    let mut decoder = Decoder::with_ip(64, avail, start, DecoderOptions::NONE);
    let mut fmt = IntelFormatter::new();
    fmt.options_mut().set_uppercase_hex(false);
    fmt.options_mut().set_hex_prefix("0x");
    fmt.options_mut().set_hex_suffix("");
    fmt.options_mut().set_space_after_operand_separator(true);
    fmt.options_mut().set_branch_leading_zeros(false);
    // Print `[rip + disp]` in the operand, as capstone (and every Python tool here) does, and put
    // the RESOLVED absolute address in `rip_target` - `oodle/dis-annot.py`'s reason for existing.
    // Printing only the resolved form silently changes what a diff against the old tools means.
    fmt.options_mut().set_rip_relative_addresses(true);

    let mut insns = Vec::new();
    let mut instr = Instruction::default();
    let mut consumed = 0u64;
    let mut truncated_at = None;
    while decoder.can_decode() {
        let ip = decoder.ip();
        if ip >= end {
            break;
        }
        decoder.decode_out(&mut instr);
        if instr.is_invalid() {
            truncated_at = Some(ip);
            break;
        }
        let mut text = String::new();
        fmt.format(&instr, &mut text);
        let (mnemonic, op_str) = match text.find(' ') {
            Some(i) => (text[..i].to_string(), text[i + 1..].trim_start().to_string()),
            None => (text.clone(), String::new()),
        };
        let branch_target = match instr.op0_kind() {
            OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64 => {
                Some(instr.near_branch_target())
            }
            _ => None,
        };
        let rip_target = if instr.is_ip_rel_memory_operand() {
            Some(instr.ip_rel_memory_address())
        } else {
            None
        };
        let raw = image.bytes(ip, instr.len()).map(|b| {
            b.iter().map(|x| format!("{x:02x}")).collect::<String>()
        }).unwrap_or_default();
        insns.push(Insn {
            address: ip,
            len: instr.len(),
            bytes: raw,
            mnemonic,
            op_str,
            branch_target,
            rip_target,
        });
        consumed = ip + instr.len() as u64 - start;
    }
    if truncated_at.is_none() && start + consumed < end {
        truncated_at = Some(start + consumed);
    }
    Ok(Disassembly { start, end, anchor: anchor_va, insns, truncated_at })
}

/// The instruction that covers `site`, plus **the function that actually contains it**.
///
/// This is the confirmation step every xref scan runs: a candidate byte window is only a real
/// reference if a *decoded instruction* covers it.
///
/// ⛔ **This used to return `nearest_prologue(site)` as the function, and that was a defect with a
/// cost.** 11,446 of this image's functions are frame-pointer-omitted, so for a site inside one the
/// prologue walk names the *preceding* function. `xref 0x25c0c58` printed its site `0x1D20A1` as
/// `fn 0x1D1E80` when the containing function is `0x1D2030` - and because `0x1D1E80` writes the
/// pool that `0x1D2030` reads, a session concluded the pool was write-only and the code below it
/// dead. Attribution now goes through [`crate::functions::Index::containing`], the same index the
/// atlas uses, and says [`crate::functions::Containment::None`] rather than guess.
///
/// The prologue walk survives as what it always was - a **decode anchor**. The containing
/// function's start is tried first (it is an instruction boundary by construction, prologue or
/// not); the nearest prologue is the fallback for a site no known function covers and the second
/// chance for one whose own body decode truncated on a data island.
pub fn instruction_at(image: &Image, site: u64) -> Result<Option<(Containing, Insn)>> {
    let containing = image.containing_function(site);

    let mut anchors: Vec<(u64, Anchor)> = Vec::new();
    if let Some(s) = containing.start {
        // An FPO start is a confirmed direct `call rel32` target: a justified start, not a guess,
        // so `Forced` here is not the ATLAS §4.5 failure mode it looks like.
        anchors.push((s, anchor_for_known_start(image, s)));
    }
    if let Some(p) = image.nearest_prologue(site, 0x4000) {
        if anchors.first().map(|a| a.0) != Some(p) {
            anchors.push((p, Anchor::Prologue));
        }
    }
    if anchors.is_empty() {
        return Ok(None);
    }

    for (anchor, mode) in anchors {
        let dis = disasm_range(image, anchor, site + 16, mode)?;
        for i in dis.insns {
            if i.address <= site && site < i.address + i.len as u64 {
                return Ok(Some((containing, i)));
            }
            if i.address > site {
                break;
            }
        }
    }
    Ok(Some((containing, Insn {
        address: site,
        len: 0,
        bytes: String::new(),
        mnemonic: "<unconfirmed>".into(),
        op_str: String::new(),
        branch_target: None,
        rip_target: None,
    })))
}

/// Is this instruction a near `call`/`jmp` to `target`?
pub fn is_branch_to(insn: &Insn, target: u64, want_call: bool) -> bool {
    let m = insn.mnemonic.as_str();
    let matches_kind = if want_call { m == "call" } else { m == "jmp" };
    matches_kind && insn.branch_target == Some(target)
}

/// Mnemonic classification helper for callers that want iced's enum rather than text.
pub fn mnemonic_of(image: &Image, va: u64) -> Result<Option<Mnemonic>> {
    let b = image.bytes(va, 16)?;
    let mut d = Decoder::with_ip(64, b, va, DecoderOptions::NONE);
    let i = d.decode();
    Ok(if i.is_invalid() { None } else { Some(i.mnemonic()) })
}
