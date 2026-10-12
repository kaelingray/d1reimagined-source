// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The compiled `{index, name_hash}` enum tables** - ~172 of them, harvested.
//!
//! `historicaldocs/design/THE-CHAIN.md` §4.1 found the mechanism and decoded one table by hand, then said the
//! obvious thing: *"a systematic harvester is **unbuilt** - my quick regex pass conflated adjacent
//! code and is not trustworthy `[I]`."* This is that harvester, and it does not use a regex.
//!
//! ## The shape
//!
//! The compiler emits an enum's name table as a run of stores into a BSS array, then registers it:
//!
//! ```asm
//! 0x009cd4a7  lea      rsi, [rip+0x1d805d2]        ; [0x274da80]  <- the table, in BSS
//! 0x009cd4ae  mov      dword ptr [rsi],      0            ; entry 0: index
//! 0x009cd4b4  mov      dword ptr [rsi+4],    0x2ca33bdb    ; entry 0: fnv1("none")
//! 0x009cd4bb  mov      dword ptr [rsi+8],    1
//! 0x009cd4c2  mov      dword ptr [rsi+0xc],  0x4247521a
//! …
//! 0x009cd527  mov      edx, 8                             ; <- the ENTRY COUNT
//! 0x009cd52c  call     0xa8c920                           ; <- the registrar
//! ```
//!
//! The table lives in **BSS**, so it is zero in the file and cannot be read (ATLAS §4.1 / the
//! `Image::file_offset` BSS refusal). *The stores are the only copy of the data.* That is why this
//! is a disassembly problem rather than a table read.
//!
//! ## What it recovers, and what it cannot
//!
//! `index → name_hash`, always. `index → name` only when the spelling is in the image's own string
//! corpus - and THE-CHAIN §4.2 shows that is often false by design: `patrol`, `arena`,
//! `trials_of_osiris`, `salvage`, `rift` all have their **hash** compiled in and their **string**
//! nowhere in the binary. An unresolved hash here is a fact about the corpus, not a failure.
//!
//! ⚠ Composite (`A_B`) meet-in-the-middle cracking has a **42% false-positive rate per target**
//! (THE-CHAIN §4.4, measured over 300 random targets). This module does single-token lookup only,
//! which scored 0/300. It does not guess.

use std::collections::HashMap;

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};
use serde::Serialize;

use crate::image::{Image, Result};
use crate::strings::Corpus;
use crate::xref::{self, RefKind};

/// `0xA8C920(this, table, count)` - the enum name-table registrar. 172 `call rel32` sites in the
/// pinned build (THE-CHAIN §4.1, 4/4 xref controls passing).
pub const REGISTRAR: u64 = 0x00A8_C920;

/// The one table decoded by hand in THE-CHAIN §4.1 - this module's positive control (ATLAS §4.4d:
/// *a new tool is not evidence until it reproduces a KNOWN result*).
pub const CONTROL_TABLE: u64 = 0x0274_DA80;
pub const CONTROL_SITE: u64 = 0x009C_D52C;

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub index: u32,
    pub hash: u32,
    /// Corpus spellings whose FNV-1 is `hash`. Usually 0 or 1; more than 1 is a real collision and
    /// is reported rather than resolved.
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Table {
    /// The `call 0xA8C920` instruction address.
    pub site: u64,
    /// The function that **contains** `site` - frame-pointer-omitted ones included.
    pub function: u64,
    /// The BSS array the stores target, when it is reached by `lea r,[rip+d]`.
    pub table: Option<u64>,
    /// The `mov edx, N` immediately before the call.
    pub declared_count: Option<u32>,
    pub entries: Vec<Entry>,
    /// `true` when `entries.len() == declared_count` - the table was recovered whole.
    pub complete: bool,
}

/// Harvest every enum name table registered through `registrar`.
pub fn harvest(image: &Image, corpus: &Corpus, registrar: u64) -> Result<Vec<Table>> {
    let mut sites: Vec<u64> = xref::scan_kind(image, registrar, RefKind::Call)?
        .into_iter()
        .filter(|h| h.confirmed)
        .map(|h| h.site)
        .collect();
    sites.sort_unstable();
    sites.dedup();

    let mut out = Vec::new();
    for site in sites {
        // ⛔ Was `nearest_prologue`, the same defect `xref`'s `fn` column had: for a call site
        // inside a frame-pointer-omitted registrar caller that starts the abstract interpretation
        // in the PREVIOUS function, so every `lea`/`mov` between the two is attributed to this
        // table. Anchor on the function that actually contains the site; the prologue walk is only
        // the fallback for a site the index does not cover.
        let f = image
            .containing_function(site)
            .proven_start()
            .or_else(|| image.nearest_prologue(site, 0x8000));
        let Some(f) = f else { continue };
        out.push(decode_one(image, corpus, f, site));
    }
    Ok(out)
}

/// Decode the table registered by the `call` at `site`, inside function `f`.
///
/// Abstract-interprets exactly two things - `lea r64,[rip+d]` and `mov r32, imm32` - which is
/// enough for this pattern and small enough that it cannot quietly be wrong about a third.
fn decode_one(image: &Image, corpus: &Corpus, f: u64, site: u64) -> Table {
    let mut regbase: HashMap<Register, u64> = HashMap::new();
    let mut regimm: HashMap<Register, u32> = HashMap::new();
    // absolute store address → value
    let mut stores: HashMap<u64, u32> = HashMap::new();
    let mut table = None;
    let mut declared_count = None;

    let len = (site - f) as usize + 16;
    let Ok(blob) = image.bytes(f, len) else {
        return Table { site, function: f, table, declared_count, entries: vec![], complete: false };
    };
    let mut dec = Decoder::with_ip(64, blob, f, DecoderOptions::NONE);
    let mut ins = Instruction::default();
    while dec.can_decode() {
        let ip = dec.ip();
        if ip > site {
            break;
        }
        dec.decode_out(&mut ins);
        if ins.is_invalid() {
            break;
        }
        match ins.mnemonic() {
            Mnemonic::Lea if ins.is_ip_rel_memory_operand() => {
                regbase.insert(ins.op0_register(), ins.ip_rel_memory_address());
                regimm.remove(&ins.op0_register());
            }
            Mnemonic::Mov if ins.op0_kind() == OpKind::Register => {
                if ins.op1_kind() == OpKind::Immediate32 {
                    regimm.insert(ins.op0_register(), ins.immediate32());
                } else {
                    regimm.remove(&ins.op0_register());
                    regbase.remove(&ins.op0_register());
                }
            }
            Mnemonic::Mov
                if ins.op0_kind() == OpKind::Memory
                    && ins.op1_kind() == OpKind::Immediate32
                    && ins.memory_size().size() == 4 =>
            {
                let addr = if ins.is_ip_rel_memory_operand() {
                    Some(ins.ip_rel_memory_address())
                } else if ins.memory_index() == Register::None {
                    regbase
                        .get(&ins.memory_base())
                        .map(|&b| b.wrapping_add(ins.memory_displacement64()))
                } else {
                    None
                };
                if let Some(a) = addr {
                    stores.insert(a, ins.immediate32());
                }
            }
            Mnemonic::Call if ins.op0_kind() == OpKind::NearBranch64 && ip == site => {
                table = regbase.get(&Register::RSI).copied();
                declared_count = regimm.get(&Register::EDX).copied();
            }
            _ => {}
        }
    }

    let mut entries = Vec::new();
    if let Some(t) = table {
        let n = declared_count.unwrap_or(0).min(4096);
        for k in 0..n as u64 {
            let (Some(&index), Some(&hash)) = (stores.get(&(t + 8 * k)), stores.get(&(t + 8 * k + 4)))
            else {
                continue;
            };
            entries.push(Entry { index, hash, names: corpus.resolve(hash).to_vec() });
        }
    }
    let complete = declared_count.is_some_and(|n| entries.len() as u32 == n) && !entries.is_empty();
    Table { site, function: f, table, declared_count, entries, complete }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    /// ATLAS §4.4d - reproduce a KNOWN result before trusting the tool. THE-CHAIN §4.1 decoded the
    /// bootflow table at `0x274DA80` by hand; this asserts the harvester agrees, name for name.
    #[test]
    fn reproduces_the_hand_decoded_bootflow_table() {
        let Ok(image) = Image::open_default() else { return };
        let mut corpus = Corpus::build(&image, 3).unwrap();
        corpus.add_names(crate::rtti::harvest(&image).unwrap().types.into_iter().map(|t| t.name));
        let tables = harvest(&image, &corpus, REGISTRAR).unwrap();
        let t = tables
            .iter()
            .find(|t| t.site == CONTROL_SITE)
            .expect("the control call site must be found");
        assert_eq!(t.table, Some(CONTROL_TABLE));
        assert_eq!(t.declared_count, Some(8));
        assert_eq!(t.entries.len(), 8);
        assert_eq!(t.entries[0].hash, 0x2CA3_3BDB); // fnv1("none")
        assert_eq!(t.entries[2].hash, 0xE9D4_6191); // fnv1("platform_signin")
        assert_eq!(t.entries[3].hash, 0xEFA4_CF17); // fnv1("account_signin")
        assert_eq!(t.entries[5].hash, 0x783B_F8AD); // fnv1("content_check")
        assert!(t.entries[2].names.iter().any(|n| n == "platform_signin"));
    }
}
