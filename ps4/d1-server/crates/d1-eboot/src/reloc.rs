// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Relocation reading, and the addend→slot index that makes "not found" sayable.
//!
//! Ported from `capture-rig/oodle/reloc-xref.py`, with one correction: that tool **hardcodes**
//! `RELA_OFF = 0x1A0D100 + 0xBDD0` and `RELA_SZ = 0x1B0918`. Those numbers are right for this
//! image, but a hardcoded constant is exactly how a superseded reading survives into new code
//! (ATLAS §4.3). Here they are **parsed out of `PT_DYNAMIC`** using the SCE dynamic tags, and the
//! historical constants survive only as a pinned control in `tests/controls.rs`.
//!
//! Why this module exists at all (`reloc-xref.py` docstring, §26.10):
//!
//! > Vtable slots in `.data.rel.ro` hold **ZERO in the file**; the target lives in the
//! > `R_X86_64_RELATIVE` addend. A scan that only greps for literal qwords therefore finds nothing
//! > and reports a false absence.
//!
//! PS4 eboots store pointer tables as zero in the file and fill them at load, so a raw byte search
//! for an address finds nothing **by design** (ATLAS §4.2).

use std::collections::HashMap;

use serde::Serialize;

use crate::image::{Image, ImageError, Result, PT_DYNAMIC, PT_SCE_DYNLIBDATA};

// SCE d_tag values, read from PT_DYNAMIC. Offsets are relative to the PT_SCE_DYNLIBDATA
// segment's *file* offset - that segment has p_vaddr == 0 (ATLAS §4.1).
pub const DT_SCE_JMPREL: u64 = 0x6100_0029;
pub const DT_SCE_PLTRELSZ: u64 = 0x6100_002D;
pub const DT_SCE_RELA: u64 = 0x6100_002F;
pub const DT_SCE_RELASZ: u64 = 0x6100_0031;
pub const DT_SCE_RELAENT: u64 = 0x6100_0033;

pub const R_X86_64_RELATIVE: u32 = 8;
pub const R_X86_64_JUMP_SLOT: u32 = 7;
pub const R_X86_64_64: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rela {
    /// `r_offset` - for this image, the guest vaddr of the slot being relocated.
    pub slot: u64,
    pub r_type: u32,
    pub sym: u32,
    pub addend: u64,
}

/// Where the relocation tables actually are, derived rather than remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RelaTables {
    pub dynlibdata_file_offset: u64,
    pub rela_file_offset: u64,
    pub rela_size: u64,
    pub rela_entsize: u64,
    pub jmprel_file_offset: u64,
    pub jmprel_size: u64,
}

fn dynamic_tags(image: &Image) -> Result<HashMap<u64, u64>> {
    let dyn_seg = image
        .segment_of_type(PT_DYNAMIC)
        .ok_or(ImageError::NotInLoad { offset: 0 })?;
    let mut out = HashMap::new();
    let base = dyn_seg.offset as usize;
    let mut p = base;
    while p + 16 <= base + dyn_seg.filesz as usize {
        let tag = u64::from_le_bytes(image.raw(p, 8)?.try_into().unwrap());
        let val = u64::from_le_bytes(image.raw(p + 8, 8)?.try_into().unwrap());
        if tag == 0 {
            break;
        }
        out.insert(tag, val);
        p += 16;
    }
    Ok(out)
}

pub fn tables(image: &Image) -> Result<RelaTables> {
    let tags = dynamic_tags(image)?;
    let dynlib = image
        .segment_of_type(PT_SCE_DYNLIBDATA)
        .ok_or(ImageError::NotInLoad { offset: 0 })?;
    let get = |t: u64| tags.get(&t).copied().unwrap_or(0);
    Ok(RelaTables {
        dynlibdata_file_offset: dynlib.offset,
        rela_file_offset: dynlib.offset + get(DT_SCE_RELA),
        rela_size: get(DT_SCE_RELASZ),
        rela_entsize: if get(DT_SCE_RELAENT) == 0 { 24 } else { get(DT_SCE_RELAENT) },
        jmprel_file_offset: dynlib.offset + get(DT_SCE_JMPREL),
        jmprel_size: get(DT_SCE_PLTRELSZ),
    })
}

/// Every entry of `DT_SCE_RELA`, in table order.
pub fn all(image: &Image) -> Result<Vec<Rela>> {
    let t = tables(image)?;
    read_table(image, t.rela_file_offset, t.rela_size, t.rela_entsize)
}

/// Every entry of `DT_SCE_JMPREL` (the PLT relocations).
pub fn plt(image: &Image) -> Result<Vec<Rela>> {
    let t = tables(image)?;
    read_table(image, t.jmprel_file_offset, t.jmprel_size, t.rela_entsize)
}

fn read_table(image: &Image, off: u64, size: u64, ent: u64) -> Result<Vec<Rela>> {
    let mut out = Vec::with_capacity((size / ent) as usize);
    let mut p = off as usize;
    let end = (off + size) as usize;
    while p + 24 <= end {
        let r_offset = u64::from_le_bytes(image.raw(p, 8)?.try_into().unwrap());
        let r_info = u64::from_le_bytes(image.raw(p + 8, 8)?.try_into().unwrap());
        let r_addend = u64::from_le_bytes(image.raw(p + 16, 8)?.try_into().unwrap());
        out.push(Rela {
            slot: r_offset,
            r_type: (r_info & 0xFFFF_FFFF) as u32,
            sym: (r_info >> 32) as u32,
            addend: r_addend,
        });
        p += ent as usize;
    }
    Ok(out)
}

/// Only the `R_X86_64_RELATIVE` entries - the ones that carry a code/data pointer in the addend.
/// This is the set `reloc-xref.py` reports (70,682 in the pinned build).
pub fn relative(image: &Image) -> Result<Vec<Rela>> {
    Ok(all(image)?
        .into_iter()
        .filter(|r| r.r_type == R_X86_64_RELATIVE)
        .collect())
}

/// addend → slots pointing at it. The map that lets a "nothing points here" be evidence.
pub fn by_addend(image: &Image) -> Result<HashMap<u64, Vec<u64>>> {
    let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
    for r in relative(image)? {
        m.entry(r.addend).or_default().push(r.slot);
    }
    Ok(m)
}

/// slot vaddr → addend. `reloc-xref.py --slot` walks a vtable through this.
pub fn by_slot(image: &Image) -> Result<HashMap<u64, u64>> {
    let mut m = HashMap::new();
    for r in relative(image)? {
        m.insert(r.slot, r.addend);
    }
    Ok(m)
}
