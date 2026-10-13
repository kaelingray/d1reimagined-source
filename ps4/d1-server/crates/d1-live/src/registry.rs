// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The read side of `capture-rig/d1guest.py`**: a live client with a proven base, the *object*
//! registry, the *package* registry, and the QueueZ descriptor table.
//!
//! # Why this is a crate and not a script
//!
//! The answer is in BSS or on the heap, so static analysis stops. Before `d1guest.py` there were
//! five scripts that each re-derived the guest base by signature match and each re-implemented
//! `/proc/<pid>/mem`; five copies of one mechanism is five places for a superseded constant to
//! survive a retraction. `d1guest.py` collapsed them to one. This module is that one mechanism in
//! the workspace, so the server and the harness can ask the same questions the operator asks.
//!
//! # ⚠ Two registries, and they are not the same registry
//!
//! | | described by | rooted at | indexed by |
//! |---|---|---|---|
//! | **object registry** | `registry_layout` | `object_registry_ptr` | `(family, type, index)` |
//! | **package registry** | `package_registry_layout` | `package_registry_global` | a `0x8080xxxx` tag |
//!
//! Confusing them produces a clean, confident answer to a different question. Every function here
//! says which one it walks.
//!
//! # The controls that are not optional
//!
//! * [`Guest::family_slot`] derives an element address **twice** - once from the client's runtime
//!   entry table, once from the static family/slot mapper in `guest-symbols.json` - and refuses if
//!   they disagree on address, capacity or stride. Two independent derivations landing on one
//!   address is the thing a recovered base plus a registry pointer cannot otherwise give you.
//! * A family with **no** static row is reported as **cross-check SKIPPED**, never as passed.
//! * An entry offset of `-1` means the region is not allocated in this process. That is a refusal,
//!   not an address.

use anyhow::{anyhow, bail, Result};
use d1_eboot::Image;

use crate::proc::Proc;
use crate::symbols::Symbols;

/// Which part of the eboot image a vaddr falls in.
///
/// ⚠ The distinction is load-bearing for the read side: a `file` range can be diffed against the
/// shipped bytes for free (and `boot_state_names` is *zero in the file* and filled by relocation at
/// load, ATLAS §4.2), while a `bss` range is precisely what static analysis cannot answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    File,
    Bss,
    Outside,
}

impl Region {
    pub fn as_str(self) -> &'static str {
        match self {
            Region::File => "file",
            Region::Bss => "bss",
            Region::Outside => "outside",
        }
    }
}

/// `d1guest.py`'s `Image.region_of`, over `d1-eboot`'s program-header table.
///
/// ⚠ ATLAS §4.1: the file-to-vaddr bias is **per segment**. `d1-eboot` parses the program headers,
/// so nothing here assumes the `0x4000` that every pre-`d1guest` script hardcoded.
pub fn region_of(image: &Image, vaddr: u64) -> Region {
    for s in image.load_segments() {
        if s.vaddr <= vaddr && vaddr < s.vaddr + s.filesz {
            return Region::File;
        }
    }
    for s in image.load_segments() {
        if s.vaddr <= vaddr && vaddr < s.vaddr + s.memsz {
            return Region::Bss;
        }
    }
    Region::Outside
}

/// A `registry_layout` value, **required rather than defaulted**.
///
/// ⛔ This function exists because the first cut of this port wrote
/// `syms.layout.get(k).copied().unwrap_or(0x60)` and friends. That silently re-introduced the exact
/// thing `d1guest.py` was written to abolish: its own docstring says five bespoke scripts meant
/// "five places for a superseded constant to survive a retraction". A hardcoded fallback is a sixth.
/// Deleting `entry_stride_family` from the symbol file made the Python raise `KeyError` and refuse,
/// and made this crate print a confident, correct-looking dump from a constant compiled in months
/// earlier - invisible today because the default happens to match, wrong the day the value moves.
///
/// The symbol file is the single source; a missing key is a refusal.
pub fn layout_req(syms: &Symbols, key: &str) -> Result<u64> {
    syms.layout.get(key).copied().ok_or_else(|| {
        anyhow!(
            "{}: registry_layout has no {key:?}. This value is NOT defaulted - a compiled-in \
             fallback is how a superseded constant survives a retraction. Add it to the symbol \
             file, with a source and a confidence.",
            syms.path.display()
        )
    })
}

/// A `package_registry_layout` field offset, required **and** bounded against the record it indexes.
///
/// ⚠ The bound is the second half: a slot record is `0x40` bytes, so an offset of `0x3C` for an
/// 8-byte field indexes past the end. Before this check that was a Rust slice panic (exit 101 with a
/// backtrace), which is loud but is not a refusal - and `d1guest.py`, which hardcodes these three
/// offsets, cannot reach the state at all.
fn package_field(syms: &Symbols, key: &str, width: usize, record: usize) -> Result<usize> {
    let off = syms.package_layout.get(key).copied().ok_or_else(|| {
        anyhow!("{}: package_registry_layout has no {key:?}", syms.path.display())
    })? as usize;
    if off + width > record {
        bail!(
            "{}: package_registry_layout {key} = {off:#x} puts a {width}-byte field past the end \
             of a {record:#x}-byte slot record",
            syms.path.display()
        );
    }
    Ok(off)
}

/// `a + b*c`, refusing rather than wrapping.
///
/// ⛔ Release builds wrap on overflow, and a wrapped registry address is the worst output this tool
/// can produce: `d1-live obj 576460752303423492:0` (that is `4 + 2^59`, and `0x60 * 2^59 == 0` mod
/// 2^64) aliased exactly onto the family-4/0 entry and printed the ACCOUNT object's bytes, exit 0,
/// under the heading "family 576460752303423492/0 ... cross-check SKIPPED". `d1guest.py` refuses the
/// same input, because Python integers do not wrap and the seek overflows.
fn addr_of(base: u64, index: u64, stride: u64, what: &str) -> Result<u64> {
    index
        .checked_mul(stride)
        .and_then(|o| base.checked_add(o))
        .ok_or_else(|| anyhow!("{what}: {base:#x} + {index} * {stride:#x} overflows a 64-bit address"))
}

/// One row of the runtime-versus-static cross-check, or of a refusal.
#[derive(Debug, Clone)]
pub struct FamilyResult {
    /// The element-array address, present **only** when every cross-check that could run, passed.
    pub addr: Option<u64>,
    pub cap: u64,
    pub stride: u64,
    pub lines: Vec<String>,
}

/// One claimed QueueZ replication descriptor.
#[derive(Debug, Clone)]
pub struct Descriptor {
    pub index: usize,
    pub class: u8,
    pub key: u64,
    pub state: u8,
    pub err: u8,
    pub version: i32,
}

/// ⚠ The state names are not cosmetic: **1 and 4 REFUSE updates** while 2 and 3 accept them, so a
/// delivery that "arrived" into a state-1 descriptor did nothing at all.
pub fn state_name(s: u8) -> String {
    match s {
        0 => "free".into(),
        1 => "1 CLAIMED (update REFUSED)".into(),
        2 => "2 SUBSCRIBED (update accepted)".into(),
        3 => "3 REPLICATING (update accepted)".into(),
        4 => "4 LOST/TIMED-OUT (update REFUSED)".into(),
        other => other.to_string(),
    }
}

pub fn err_name(e: u8) -> String {
    match e {
        0 => "-".into(),
        1 => "unknown".into(),
        2 => "no_content".into(),
        3 => "moved".into(),
        4 => "not_found".into(),
        5 => "service_unavailable".into(),
        6 => "unauthorized".into(),
        7 => "TIMEOUT".into(),
        other => other.to_string(),
    }
}

/// A live client with a **proven** base. Nothing here may be constructed from an unverified base:
/// every constructor path in `main.rs` runs the three base controls first and exits 2 on any FAIL.
pub struct Guest<'a> {
    pub proc: &'a Proc,
    pub image: &'a Image,
    pub syms: &'a Symbols,
    pub base: u64,
    registry: std::cell::Cell<Option<u64>>,
}

impl<'a> Guest<'a> {
    pub fn new(proc: &'a Proc, image: &'a Image, syms: &'a Symbols, base: u64) -> Self {
        Self { proc, image, syms, base, registry: std::cell::Cell::new(None) }
    }

    // ── primitive reads ──────────────────────────────────────────────────────────────────────────

    /// Read at an **eboot vaddr** (the base is added).
    pub fn read(&self, vaddr: u64, n: usize) -> Result<Vec<u8>> {
        self.proc.read(self.base + vaddr, n)
    }

    /// Read at an **absolute host address**. ⚠ Anything read *out of* memory is already absolute -
    /// adding the base to a heap pointer is CLAUDE.md trap 1, paid for with a retracted theory.
    pub fn read_abs(&self, addr: u64, n: usize) -> Result<Vec<u8>> {
        self.proc.read(addr, n)
    }

    pub fn u32_at(&self, vaddr: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(vaddr, 4)?.try_into().unwrap()))
    }

    pub fn u64_at(&self, vaddr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(vaddr, 8)?.try_into().unwrap()))
    }

    fn u64_abs(&self, addr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.proc.read(addr, 8)?.try_into().unwrap()))
    }

    // ── the OBJECT registry ──────────────────────────────────────────────────────────────────────

    /// `*object_registry_ptr`. Zero means the client has not built the registry yet - which is an
    /// L2 failure, not an address.
    pub fn registry(&self) -> Result<u64> {
        if let Some(r) = self.registry.get() {
            return Ok(r);
        }
        let va = self
            .syms
            .lookup("object_registry_ptr")
            .ok_or_else(|| anyhow!("guest-symbols.json has no object_registry_ptr"))?;
        let r = self.u64_at(va)?;
        self.registry.set(Some(r));
        Ok(r)
    }

    fn layout(&self, key: &str) -> Result<u64> {
        layout_req(self.syms, key)
    }

    /// The element array for `(family, type)`, **derived twice and required to agree**.
    ///
    /// Once from the client's runtime entry table at `registry + entry_table_base + f*0x60 + t*0x10`
    /// (a `u32` offset, a `u16` capacity, a `u32` stride), and once from the static family/slot
    /// mapper table read out of `0x56C550` and recorded in `guest-symbols.json`. They must agree on
    /// **address, capacity and stride**; a family with no static row is `SKIPPED`, never passed.
    pub fn family_slot(&self, family: u64, typ: u64) -> Result<FamilyResult> {
        family_slot_at(self.proc, self.syms, self.registry()?, family, typ)
    }
}

/// [`Guest::family_slot`] without a `Guest`, so the liveness ladder can run the SAME cross-check.
///
/// ⚠ It has to be the same one. `d1guest.py`'s L3 rung *is* `family_slot`, so a ladder that read the
/// entry table directly would report L3 PASS on a registry whose runtime and static derivations
/// disagree - the exact condition the cross-check exists to catch, reported as health.
pub fn family_slot_at(
    proc: &Proc,
    syms: &Symbols,
    reg: u64,
    family: u64,
    typ: u64,
) -> Result<FamilyResult> {
    {
        let entry_base = layout_req(syms, "entry_table_base")?;
        let sf = layout_req(syms, "entry_stride_family")?;
        let st = layout_req(syms, "entry_stride_type")?;
        let eb = layout_req(syms, "element_base")?;

        // ⛔ Checked, not wrapping - see `addr_of`. A wrapped family index aliases onto a real
        // entry and prints another object's bytes at exit 0.
        let ent = addr_of(reg, family, sf, "entry table (family)")?;
        let ent = addr_of(ent, typ, st, "entry table (type)")?;
        let ent = ent
            .checked_add(entry_base)
            .ok_or_else(|| anyhow!("entry table: adding entry_table_base overflows"))?;
        let raw = proc.read(ent, 12)?;
        // `<IHxxI`: u32 offset, u16 capacity, 2 pad, u32 stride.
        let off = u32::from_le_bytes(raw[0..4].try_into().unwrap());
        let cap = u16::from_le_bytes(raw[4..6].try_into().unwrap()) as u64;
        let stride = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as u64;

        if off == 0xFFFF_FFFF {
            return Ok(FamilyResult {
                addr: None,
                cap,
                stride,
                lines: vec![format!(
                    "  FAIL  family {family}/{typ}: entry offset is -1 (region not allocated in \
                     this process)"
                )],
            });
        }

        let addr_rt = reg
            .checked_add(eb)
            .and_then(|a| a.checked_add(off as u64))
            .ok_or_else(|| anyhow!("element base: {reg:#x} + {eb:#x} + {off:#x} overflows"))?;
        let mut lines = Vec::new();
        match syms.family(family, typ) {
            None => {
                lines.push(format!(
                    "  ----  family {family}/{typ}: no static entry in guest-symbols.json, \
                     cross-check SKIPPED"
                ));
                Ok(FamilyResult { addr: Some(addr_rt), cap, stride, lines })
            }
            Some(stat) => {
                let addr_st = reg
                    .checked_add(stat.storage)
                    .ok_or_else(|| anyhow!("static storage offset {:#x} overflows", stat.storage))?;
                let ok = addr_rt == addr_st && cap == stat.cap && stride == stat.size;
                lines.push(format!(
                    "  {}  family {family}/{typ} cross-check: runtime {addr_rt:#x} cap={cap} \
                     stride={stride:#x} vs static {addr_st:#x} cap={} stride={:#x}  [{}]",
                    if ok { "PASS" } else { "FAIL" },
                    stat.cap,
                    stat.size,
                    if stat.identity.is_empty() { "-" } else { &stat.identity },
                ));
                Ok(FamilyResult {
                    addr: if ok { Some(addr_rt) } else { None },
                    cap,
                    stride,
                    lines,
                })
            }
        }
    }
}

impl<'a> Guest<'a> {
    /// Find the element whose **key qword** (offset 0) equals `key`.
    ///
    /// ⭐ This exists because an index is a guess and a key is an identity: the client reorders
    /// elements, so `obj 3:1:0` and `obj 3:1 --key …` are different questions.
    pub fn find_by_key(&self, family: u64, typ: u64, key: u64) -> Result<(Option<(u64, usize)>, Vec<String>)> {
        let fr = self.family_slot(family, typ)?;
        let Some(addr) = fr.addr else { return Ok((None, fr.lines)) };
        for i in 0..fr.cap {
            let at = addr_of(addr, i, fr.stride, "registry element")?;
            let k = self.u64_abs(at)?;
            if k == key {
                return Ok((Some((at, i as usize)), fr.lines));
            }
        }
        let mut lines = fr.lines;
        lines.push(format!("  ----  no element in {} slot(s) carries key {key:#018x}", fr.cap));
        Ok((None, lines))
    }

    // ── the PACKAGE registry (schema / content tags) ─────────────────────────────────────────────

    /// `*(*package_registry_global)` - the slot table base. `None` when the client has not built it.
    pub fn package_registry(&self) -> Result<Option<u64>> {
        let va = self
            .syms
            .lookup("package_registry_global")
            .ok_or_else(|| anyhow!("guest-symbols.json has no package_registry_global"))?;
        let header = self.u64_at(va)?;
        if header == 0 {
            return Ok(None);
        }
        Ok(Some(self.u64_abs(header)?))
    }

    /// The slot stride the CLIENT states, read from `header + header_slot_stride`.
    ///
    /// ⚠ `d1guest.py` hardcodes `0x40` here. The client does not: the lookup inlined in
    /// `FUN_0010C500` loads the stride out of the header, and `package_registry_layout` in
    /// `guest-symbols.json` records that (`header_slot_stride = 16`, value `0x40` live). This port
    /// **reads it** and prints a loud line if it is not `0x40`, rather than quietly walking a table
    /// with the wrong pitch on some future build. On the pinned client the two are identical, which
    /// is why the golden diff against the Python is byte-for-byte.
    pub fn package_slot_stride(&self) -> Result<(u64, Option<String>)> {
        let va = self
            .syms
            .lookup("package_registry_global")
            .ok_or_else(|| anyhow!("guest-symbols.json has no package_registry_global"))?;
        let header = self.u64_at(va)?;
        if header == 0 {
            return Ok((0x40, None));
        }
        let off = self.syms.package_layout.get("header_slot_stride").copied().unwrap_or(16);
        let stride = self.u64_abs(header + off)?;
        if stride == 0x40 {
            Ok((0x40, None))
        } else if stride == 0 || stride > 0x1000 {
            Ok((
                0x40,
                Some(format!(
                    "  ⚠ header+{off:#x} states slot stride {stride:#x}, which is not a usable \
                     pitch; falling back to the 0x40 that d1guest.py hardcodes. Treat every tag \
                     below as [U]."
                )),
            ))
        } else {
            Ok((
                stride,
                Some(format!(
                    "  ⚠ header+{off:#x} states slot stride {stride:#x}, NOT the 0x40 that \
                     d1guest.py hardcodes - using the client's value. Results below are not \
                     comparable with that script's."
                )),
            ))
        }
    }

    /// The tag hash, exactly as `FUN_0010C500` computes it: `(slot, index)`.
    ///
    /// ⚠ Every step is width-specific and one of them is **signed**. Transcribed from the client:
    ///
    /// ```text
    /// eax = (i32) tag
    /// esi = eax >> 13                      arithmetic (signed) shift
    /// edx = ((u32) esi >> 0x12) | 0x3FF    logical shift, then a set floor
    /// slot  = (u32) ((i32) edx & esi)
    /// index = tag & 0x1FFF
    /// ```
    pub fn tag_slot(tag: u32) -> (u64, u64) {
        let eax = tag as i32;
        let esi = eax >> 13;
        let edx = ((esi as u32) >> 0x12) | 0x3FF;
        let slot = ((edx as i32) & esi) as u32;
        (slot as u64, (tag & 0x1FFF) as u64)
    }

    /// Resolve a `0x8080xxxx` schema/content tag to the object it names.
    ///
    /// `Ok(None)` with a reason is a **refusal**, not a failure: "the client has not built the
    /// package registry yet" and "that slot is empty in this process" are both real answers.
    pub fn resolve_tag(&self, tag: u32) -> Result<(Option<u64>, String)> {
        let Some(reg) = self.package_registry()? else {
            return Ok((None, "package registry pointer is NULL - the client has not built it yet".into()));
        };
        let (stride, _warn) = self.package_slot_stride()?;
        let (slot, index) = Self::tag_slot(tag);
        const REC: usize = 0x40;
        let rec = self.proc.read(addr_of(reg, slot, stride, "package slot")?, REC)?;
        // ⛔ Required and BOUNDED: an offset past the record used to be a slice panic, not a refusal.
        let dbo = package_field(self.syms, "slot_data_base", 8, REC)?;
        let eso = package_field(self.syms, "slot_entry_size", 4, REC)?;
        let mko = package_field(self.syms, "slot_mask", 4, REC)?;
        let data_base = u64::from_le_bytes(rec[dbo..dbo + 8].try_into().unwrap());
        let esize = i32::from_le_bytes(rec[eso..eso + 4].try_into().unwrap());
        let mask = i32::from_le_bytes(rec[mko..mko + 4].try_into().unwrap());
        if data_base == 0 || esize <= 0 {
            return Ok((None, format!("slot {slot:#x} is empty in this process")));
        }
        let ent = addr_of(data_base, index, esize as u64, "package entry")?;
        let loc = self.u64_abs(
            ent.checked_add(8).ok_or_else(|| anyhow!("package entry {ent:#x} + 8 overflows"))?,
        )?;
        // `and rbx,[rsi+rax+8]` then `sub rsi,rbx`: the mask is a SIGN-EXTENDED i32 and the
        // subtraction is SIGNED, so the masked `loc` is a signed 64-bit BACK-offset. A `mask` of 0
        // means the entry IS the object.
        let back = ((mask as i64 as u64) & loc) as i64;
        let obj = (ent as i64 - back) as u64;
        Ok((
            Some(obj),
            format!("slot {slot:#x} index {index} data_base={data_base:#x} entry_size={esize}"),
        ))
    }

    // ── the QueueZ replication descriptor table ──────────────────────────────────────────────────

    /// Every descriptor with a non-zero state **or** key.
    ///
    /// ⚠ Descriptors **survive** a connection teardown that clears registry objects (ATLAS §4.6b).
    /// That asymmetry is why `descriptors` does not require L3 and `obj` does.
    pub fn descriptors(&self) -> Result<Vec<Descriptor>> {
        let reg = self.registry()?;
        let table = self.layout("descriptor_table")?;
        let count = self.layout("descriptor_count")? as usize;
        let stride = self.layout("descriptor_stride")? as usize;
        if stride < 0x40 {
            bail!("descriptor_stride {stride:#x} is under 0x40 - a row would be truncated");
        }
        let span = count
            .checked_mul(stride)
            .ok_or_else(|| anyhow!("descriptor table {count} x {stride:#x} overflows"))?;
        let blob = self.proc.read(
            reg.checked_add(table)
                .ok_or_else(|| anyhow!("descriptor table {reg:#x} + {table:#x} overflows"))?,
            span,
        )?;
        let mut out = Vec::new();
        for i in 0..count {
            let d = &blob[i * stride..(i + 1) * stride];
            let key = u64::from_le_bytes(d[0x20..0x28].try_into().unwrap());
            let state = d[0x28];
            let err = d[0x29];
            if state != 0 || key != 0 {
                out.push(Descriptor {
                    index: i,
                    class: d[0],
                    key,
                    state,
                    err,
                    version: i32::from_le_bytes(d[0x3C..0x40].try_into().unwrap()),
                });
            }
        }
        Ok(out)
    }
}

impl Descriptor {
    /// The single line `dump-descriptors.py` and `d1guest.py descriptors` both print.
    pub fn line(&self) -> String {
        format!(
            "  [{:3}] class={:<3} key={:#018x}  state={}  err={}  ver={}",
            self.index,
            self.class,
            self.key,
            state_name(self.state),
            err_name(self.err),
            self.version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Goldens lifted from `d1guest.py`'s own `Guest.tag_slot`, evaluated by CPython over the same
    /// inputs. ⭐ The point is the **signedness**: `0xFFFFFFFF` lands in slot `0x3FFF` only if the
    /// `>> 13` is arithmetic and the `>> 0x12` is logical. Get either wrong and this table moves.
    #[test]
    fn tag_slot_matches_the_python_oracle() {
        let cases: &[(u32, u64, u64)] = &[
            (0x8080_1825, 0x400, 6181),
            (0x8080_1826, 0x400, 6182),
            (0x8080_18B7, 0x400, 6327),
            (0x0000_0000, 0x0, 0),
            (0x0000_2000, 0x1, 0),
            (0x7FFF_FFFF, 0x3FF, 8191),
            (0xFFFF_FFFF, 0x3FFF, 8191),
            (0x0000_0001, 0x0, 1),
        ];
        for &(tag, slot, index) in cases {
            assert_eq!(Guest::tag_slot(tag), (slot, index), "tag {tag:#x}");
        }
    }

    /// The INVERSION: the hash must actually discriminate. A formula that returned a constant would
    /// pass the table above for one row and silently resolve every tag to one object.
    #[test]
    fn tag_slot_discriminates() {
        assert_ne!(Guest::tag_slot(0x8080_1825), Guest::tag_slot(0x8080_1826));
        assert_ne!(Guest::tag_slot(0x0000_0000), Guest::tag_slot(0x0000_2000));
        // …and a naive UNSIGNED `>> 13` would put 0x80801825 in slot 0x40 rather than 0x400.
        let naive = ((0x8080_1825u32 >> 13) >> 0x12) | 0x3FF;
        assert_ne!(naive as u64 & (0x8080_1825u32 >> 13) as u64, Guest::tag_slot(0x8080_1825).0);
    }

    #[test]
    fn region_of_separates_file_from_bss() {
        let img = match Image::open_reference() {
            Ok(i) => i,
            Err(_) => return, // the pinned dump is not present in every checkout
        };
        // `object_registry_ptr` is BSS: inside a PT_LOAD memsz range and past its filesz. No file
        // read can answer it - which is the whole reason this instrument exists.
        assert_eq!(region_of(&img, 0x26A2040), Region::Bss);
        // The verify window is file-backed by construction (the base control reads it out of both).
        assert_eq!(region_of(&img, 0x1324060), Region::File);
        assert_eq!(region_of(&img, 0xFFFF_FFFF_0000), Region::Outside);
    }
}
