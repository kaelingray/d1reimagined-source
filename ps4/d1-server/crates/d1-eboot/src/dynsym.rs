// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The PS4 dynamic symbol table, and the **NID oracle** that makes it readable.
//!
//! A PS4 dynamic symbol is not spelled out. It is stored as `NID#lib#mod`, where the NID is
//!
//! ```text
//! base64_std( be64( le64( sha1(name || SALT)[0..8] ) ) )      SALT = 518D64A635DED8C1E6B039B1C3E55230
//! ```
//!
//! That is a one-way function, so this module cannot *name* an arbitrary import. What it can do -
//! and what the RTTI harvest depends on - is **confirm a guess**: hash a candidate spelling and
//! look for the resulting NID in the table.
//!
//! ⚠ This matters because it is the difference between a constant and a derivation
//! (`historicaldocs/design/ATLAS.md` §4.3). The three Itanium-ABI class-type-info vtable symbols that
//! [`crate::rtti`] keys off could have been hardcoded as `pZ9WXcClPO8` and friends; instead they
//! are re-derived from their C++ spellings on every run, so a wrong one cannot silently survive.
//!
//! Derived and verified against the pinned 01.29 eboot:
//!
//! | sym | NID | spelling | R_X86_64_64 uses |
//! |---:|---|---|---:|
//! | 458 | `byV+FWlAnB4` | `_ZTVN10__cxxabiv117__class_type_infoE` | 323 |
//! | 459 | `pZ9WXcClPO8` | `_ZTVN10__cxxabiv120__si_class_type_infoE` | 2,131 |
//! | 501 | `9ByRMdo7ywg` | `_ZTVN10__cxxabiv121__vmi_class_type_infoE` | 179 |
//! | 511 | `zr094EQ39Ww` | `__cxa_pure_virtual` | 484 |

use std::collections::HashMap;

use serde::Serialize;

use crate::image::{Image, ImageError, Result, PT_DYNAMIC, PT_SCE_DYNLIBDATA};

pub const DT_SCE_STRTAB: u64 = 0x6100_0035;
pub const DT_SCE_STRSZ: u64 = 0x6100_0037;
pub const DT_SCE_SYMTAB: u64 = 0x6100_0039;
pub const DT_SCE_SYMTABSZ: u64 = 0x6100_003F;

/// Sony's fixed NID salt. Public and long documented; not derived from any excluded prior art.
const NID_SALT: [u8; 16] = [
    0x51, 0x8D, 0x64, 0xA6, 0x35, 0xDE, 0xD8, 0xC1, 0xE6, 0xB0, 0x39, 0xB1, 0xC3, 0xE5, 0x52, 0x30,
];

/// `NID(name)` - the 11-character encoded form a PS4 dynamic symbol is stored under.
pub fn nid(name: &str) -> String {
    use base64::Engine as _;
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(name.as_bytes());
    h.update(NID_SALT);
    let d = h.finalize();
    let le = u64::from_le_bytes(d[..8].try_into().unwrap());
    let mut s = base64::engine::general_purpose::STANDARD.encode(le.to_be_bytes());
    while s.ends_with('=') {
        s.pop();
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DynSym {
    pub index: u32,
    /// The full stored spelling, e.g. `pZ9WXcClPO8#w#r`.
    pub raw: String,
    /// Everything before the first `#` - the NID proper.
    pub nid: String,
    pub info: u8,
    pub shndx: u16,
    pub value: u64,
}

impl DynSym {
    pub fn is_import(&self) -> bool {
        self.shndx == 0 && self.value == 0
    }
}

fn dynamic_tags(image: &Image) -> Result<HashMap<u64, u64>> {
    let seg = image
        .segment_of_type(PT_DYNAMIC)
        .ok_or(ImageError::NotInLoad { offset: 0 })?;
    let base = seg.offset as usize;
    let mut out = HashMap::new();
    let mut p = base;
    while p + 16 <= base + seg.filesz as usize {
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

/// Every `Elf64_Sym` in `DT_SCE_SYMTAB`, in table order (the index a relocation's `r_info>>32` uses).
pub fn all(image: &Image) -> Result<Vec<DynSym>> {
    let tags = dynamic_tags(image)?;
    let dynlib = image
        .segment_of_type(PT_SCE_DYNLIBDATA)
        .ok_or(ImageError::NotInLoad { offset: 0 })?;
    // ⚠ ATLAS §4.1: the metadata segments have p_vaddr == 0 and are addressed by RAW FILE OFFSET.
    let strtab = dynlib.offset as usize + *tags.get(&DT_SCE_STRTAB).unwrap_or(&0) as usize;
    let strsz = *tags.get(&DT_SCE_STRSZ).unwrap_or(&0) as usize;
    let symtab = dynlib.offset as usize + *tags.get(&DT_SCE_SYMTAB).unwrap_or(&0) as usize;
    let symsz = *tags.get(&DT_SCE_SYMTABSZ).unwrap_or(&0) as usize;

    let str_blob = image.raw(strtab, strsz)?;
    let read_str = |off: usize| -> String {
        if off >= str_blob.len() {
            return String::new();
        }
        let end = str_blob[off..]
            .iter()
            .position(|&b| b == 0)
            .map(|n| off + n)
            .unwrap_or(str_blob.len());
        String::from_utf8_lossy(&str_blob[off..end]).into_owned()
    };

    let mut out = Vec::with_capacity(symsz / 24);
    let mut p = symtab;
    let mut index = 0u32;
    while p + 24 <= symtab + symsz {
        let st_name = u32::from_le_bytes(image.raw(p, 4)?.try_into().unwrap()) as usize;
        let info = image.raw(p + 4, 1)?[0];
        let shndx = u16::from_le_bytes(image.raw(p + 6, 2)?.try_into().unwrap());
        let value = u64::from_le_bytes(image.raw(p + 8, 8)?.try_into().unwrap());
        let raw = read_str(st_name);
        let nid = raw.split('#').next().unwrap_or("").to_string();
        out.push(DynSym { index, raw, nid, info, shndx, value });
        p += 24;
        index += 1;
    }
    Ok(out)
}

/// NID → symbol index. The lookup the RTTI harvest uses to turn a C++ spelling into a `sym` field.
pub fn by_nid(image: &Image) -> Result<HashMap<String, u32>> {
    Ok(all(image)?.into_iter().map(|s| (s.nid, s.index)).collect())
}

/// Resolve a C++/C spelling to its dynamic-symbol index, or `None` if this image does not import it.
pub fn index_of(map: &HashMap<String, u32>, spelling: &str) -> Option<u32> {
    map.get(&nid(spelling)).copied()
}

// ── PLT imports ────────────────────────────────────────────────────────────────────────────────

/// One imported function: its symbol, its GOT slot, and the PLT stub the game actually calls.
#[derive(Debug, Clone, Serialize)]
pub struct Import {
    pub sym: u32,
    pub nid: String,
    pub raw: String,
    /// `r_offset` of the `R_X86_64_JUMP_SLOT` relocation.
    pub got_slot: u64,
    /// Address of the `jmp qword ptr [rip+got]` stub. **This is the call target in the call graph.**
    pub plt_stub: Option<u64>,
    /// Resolved spelling, when a dictionary confirmed the NID.
    pub name: Option<String>,
}

/// Every `DT_SCE_JMPREL` import, paired with its PLT stub.
///
/// ⚠ The stub is found by scanning executable bytes for `ff 25 disp32` (`jmp qword ptr [rip+d]`)
/// whose displacement lands on a GOT slot - **not** by assuming a stub stride. In the pinned build
/// the 638 stubs occupy `0x129D080..0x129F850`, and **19,510 functions call one directly**, which
/// is why naming these 638 symbols is worth more than any other single technique left.
pub fn imports(image: &Image) -> Result<Vec<Import>> {
    let syms = all(image)?;
    let plt = crate::reloc::plt(image)?;
    let mut got_to_stub: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
    let slots: std::collections::HashSet<u64> = plt.iter().map(|r| r.slot).collect();
    for seg in image.executable_segments() {
        let r = seg.file_range();
        let blob = &image.data()[r.start..r.end.min(image.data().len())];
        let mut i = 0usize;
        while i + 6 <= blob.len() {
            if blob[i] == 0xFF && blob[i + 1] == 0x25 {
                let d = i32::from_le_bytes(blob[i + 2..i + 6].try_into().unwrap()) as i64;
                let target = (seg.vaddr as i64 + i as i64 + 6 + d) as u64;
                if slots.contains(&target) {
                    got_to_stub.insert(target, seg.vaddr + i as u64);
                }
            }
            i += 1;
        }
    }
    Ok(plt
        .iter()
        .map(|r| {
            let s = syms.get(r.sym as usize);
            Import {
                sym: r.sym,
                nid: s.map(|x| x.nid.clone()).unwrap_or_default(),
                raw: s.map(|x| x.raw.clone()).unwrap_or_default(),
                got_slot: r.slot,
                plt_stub: got_to_stub.get(&r.slot).copied(),
                name: None,
            }
        })
        .collect())
}

/// Confirm import NIDs against a dictionary of candidate spellings.
///
/// The NID is one-way, so this can never *invent* a name - a hash either matches or it does not.
/// That makes a large, sloppy candidate list completely safe: false candidates cost nothing.
pub fn resolve(imports: &mut [Import], candidates: impl IntoIterator<Item = String>) -> usize {
    let mut by_nid: HashMap<String, String> = HashMap::new();
    for c in candidates {
        let c = c.trim().to_string();
        if c.starts_with('#') {
            continue;
        }
        if c.is_empty() {
            continue;
        }
        by_nid.entry(nid(&c)).or_insert(c);
    }
    let mut hit = 0;
    for i in imports.iter_mut() {
        if let Some(n) = by_nid.get(&i.nid) {
            i.name = Some(n.clone());
            hit += 1;
        }
    }
    hit
}
