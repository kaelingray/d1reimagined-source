// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The generator for [`crate::tables`]** - the derivation that used to live in
//! `capture-rig/bap-message-idspace.py` and `capture-rig/bap-message-table.py`, landed beside the
//! constants it generates so the id space is described exactly once.
//!
//! Until this module existed, the derivation lived in Python and the constants lived in
//! `tables.rs` with nothing tying them together - precisely the "no format described twice"
//! violation `design/capture-rig-audit.md` item 9 names. The tie is now a set of `#[test]`s
//! (and the `d1-bap controls` subcommand): every const table in `tables.rs` is compared against a
//! fresh walk of the pinned eboot. If one of those tests fails, **that is a drift finding to
//! report, not a test to silence** - do not edit either side to make it pass without
//! understanding which side is wrong.
//!
//! # The four tables this module re-derives
//!
//! | const in `tables.rs` | derivation |
//! |---|---|
//! | [`crate::tables::MESSAGE_CLASS`] | 58-entry vtable registry at `0x1922110`: slot 0 (`+0x00`) is a `mov eax, <id>; ret` thunk (the transport message id), slot 1 (`+0x08`) the same for the **header class** |
//! | [`crate::tables::POLICIES`] (the `PolicyVtable` rows) | 33 policy singletons at `0x1923CB0`: vtable slot 2 (`+0x10`) is the request id thunk, slot 6 (`+0x30`) the response id, `xor eax,eax` meaning fire-and-forget |
//! | [`crate::tables::PUSH_ACCEPT_SET`] | 13 non-null singleton pointers at `0x1925A90`, each singleton's vtable slot 0 an id thunk |
//! | [`crate::tables::HEADER_LEN`] | 8 codec singletons at `0x19234A0`, each vtable slot 1 a `mov eax, <len>; ret` thunk |
//!
//! plus the **investment message definition table**
//! (`capture-rig/schemas/investment-message-definitions.bin`, 101 rows of 0x40): row index vs
//! investment message id vs request/response schema tag - the three id spaces
//! `bap-message-idspace.py` existed to stop people conflating. That table has no const twin in
//! `tables.rs` (nothing in the server indexes it yet); its controls (rows 59 and 76 are messages
//! 206 and 503 with independently recorded schema tags) are carried here so the file's stride and
//! meaning stay pinned.
//!
//! # Facts the Python encoded that must not be lost
//!
//! * **Relocated words read as zero in the file.** Every pointer in `.data.rel.ro` must come from
//!   the `R_X86_64_RELATIVE` addend map ([`d1_eboot::reloc::by_slot`]), never from the raw bytes.
//! * **A thunk that is not `mov eax, imm; ret` is not a constant.** The Python's regex returned
//!   `None` for anything else, which is why three notification rows have a blank `category` cell
//!   in `capture-rig/bap-message-table.tsv`. Those three thunks are in fact `xor eax, eax; ret`
//!   (constant 0, verified byte-for-byte here), which is how `tables.rs` carries them as header
//!   class 0. [`Thunk`] keeps the distinction so the Python-compatible TSV can still be emitted
//!   byte-identically while the drift tests use the truth.
//! * **`+0x70` is beyond the vtable.** Message vtables are 8 slots (0x40); the Python's `pair_id`
//!   column reads slot 0 of whatever the linker placed next and is right only by declaration-order
//!   coincidence (`tables.rs` retracts it in full). It is re-derived here purely as a **layout
//!   pin** and for TSV parity - never exposed as a pairing API. The pairing API is
//!   [`crate::tables::response_for`].
//! * **The definition file's stride is load-bearing.** `0x40 + count*0x40 == file size` is a
//!   refusal, not a warning: a file that fails it is not this format.
//! * **The two name tables disagree on rows 0/1.** The policy *name-string* table at `0x1923AA0`
//!   lists `nondeterministic` before `deterministic`; the *singleton* table at `0x1923CB0` has
//!   them the other way round (RTTI proves it). Comparisons here therefore key on RTTI class
//!   names, which are attached to the object actually returned, never on the parallel name table.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use d1_eboot::descriptors::{self, Index as DescIndex, LEAF};
use d1_eboot::image::{Image, PINNED_EBOOT_SHA256};
use d1_eboot::reloc;

use crate::tables;

// ── the fixed addresses (pinned 01.29 eboot; the sha256 control guards them) ──────────────────

/// First entry of the 58-slot transport message vtable registry.
pub const REGISTRY: u64 = 0x192_2110;
/// Number of registry slots. `[C]` - slot 58 onward is other data; the count control pins it.
pub const REGISTRY_COUNT: usize = 58;
/// Policy family name-string pointer table (33 entries). Display only - see the rows 0/1 caveat.
pub const POLICY_NAMES: u64 = 0x192_3AA0;
/// Policy family singleton pointer table (33 entries) - what `0x55F400` indexes.
pub const POLICY_PTRS: u64 = 0x192_3CB0;
/// Exactly 33: `[33]` and `[34]` are null, asserted as a control.
pub const POLICY_COUNT: usize = 33;
/// The push accept-set singleton table; 15 slots, 13 non-null.
pub const PUSH_TABLE: u64 = 0x192_5A90;
pub const PUSH_SLOTS: usize = 15;
/// The 8 inner-header codec singletons behind `0x55C1E0`.
pub const HEADER_CODECS: u64 = 0x192_34A0;
pub const HEADER_CODEC_COUNT: usize = 8;

/// The pinned investment message definition table, beside the pinned eboot.
/// Like [`d1_eboot::image::PINNED_EBOOT`] this resolves relative to the source tree; both files
/// are local-only artifacts (owned media) and everything that needs them skips or refuses when
/// they are absent.
pub const PINNED_DEFS: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../capture-rig/schemas/investment-message-definitions.bin");

/// `kind & 0x100` marks a repeated field; `kind & 0xFF` is the element kind.
pub const ARRAY_BIT: u32 = 0x100;

// ── constant thunks ───────────────────────────────────────────────────────────────────────────

/// What sits at the target of a vtable slot, byte-decoded.
///
/// The Python's `constant_thunk` only recognised `mov eax, imm; ret` and returned `None` for
/// everything else; [`Thunk::Xor`] is the case it missed (constant 0). [`Thunk::mov_value`]
/// reproduces the Python behaviour for the byte-compatible TSV; [`Thunk::value`] is the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thunk {
    /// `b8 xx xx xx xx c3` - `mov eax, imm32; ret`.
    Mov(u32),
    /// `31 c0 c3` - `xor eax, eax; ret`, i.e. the constant 0.
    Xor,
    /// Anything else: a real function, or an unresolvable slot. Not a constant - on purpose.
    Other,
}

impl Thunk {
    /// The constant the thunk returns, if it is one.
    pub fn value(self) -> Option<u32> {
        match self {
            Thunk::Mov(v) => Some(v),
            Thunk::Xor => Some(0),
            Thunk::Other => None,
        }
    }
    /// Python-compatible reading: only the `mov` form counts (blank TSV cell otherwise).
    pub fn mov_value(self) -> Option<u32> {
        match self {
            Thunk::Mov(v) => Some(v),
            _ => None,
        }
    }
}

// ── the reader: image + relocation map ────────────────────────────────────────────────────────

/// The pinned eboot plus its `R_X86_64_RELATIVE` slot map - everything a table walk needs.
pub struct Reader {
    image: Image,
    slots: HashMap<u64, u64>,
}

impl Reader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let image = Image::open(path).map_err(|e| e.to_string())?;
        Self::from_image(image)
    }

    pub fn open_default() -> Result<Self, String> {
        let image = Image::open_default().map_err(|e| e.to_string())?;
        Self::from_image(image)
    }

    pub fn from_image(image: Image) -> Result<Self, String> {
        let slots = reloc::by_slot(&image).map_err(|e| e.to_string())?;
        Ok(Reader { image, slots })
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The resolved pointer at `va`. Relocated words read as **zero** in the file, so the
    /// relocation addend always wins; a raw zero (or an unmapped address) is `None`.
    pub fn ptr(&self, va: u64) -> Option<u64> {
        if let Some(addend) = self.slots.get(&va) {
            return Some(*addend);
        }
        match self.image.u64(va) {
            Ok(0) | Err(_) => None,
            Ok(v) => Some(v),
        }
    }

    /// NUL-terminated ASCII at `va`, up to 250 bytes (the Python's `cstr`).
    pub fn cstr(&self, va: u64) -> Option<String> {
        let bytes = self.image.bytes(va, 250).ok()?;
        let end = bytes.iter().position(|b| *b == 0)?;
        Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }

    /// Byte-decode the constant thunk at function address `va`. See [`Thunk`].
    pub fn thunk_at(&self, va: u64) -> Thunk {
        let Ok(b) = self.image.bytes(va, 6) else { return Thunk::Other };
        if b[0] == 0xB8 && b[5] == 0xC3 {
            return Thunk::Mov(u32::from_le_bytes([b[1], b[2], b[3], b[4]]));
        }
        if b[0] == 0x31 && b[1] == 0xC0 && b[2] == 0xC3 {
            return Thunk::Xor;
        }
        Thunk::Other
    }

    /// Thunk at the *target of the slot* at `va` (slot unresolved -> [`Thunk::Other`]).
    pub fn slot_thunk(&self, slot_va: u64) -> Thunk {
        match self.ptr(slot_va) {
            Some(f) => self.thunk_at(f),
            None => Thunk::Other,
        }
    }

    /// `object -> vptr -> vtable; vtable-8 -> typeinfo; typeinfo+8 -> mangled name`.
    /// Returns `(vtable, class_name)` with the Itanium length prefix stripped
    /// (`"51c_investment_bap_message_..."` -> `"c_investment_bap_message_..."`).
    pub fn rtti(&self, obj: u64) -> (Option<u64>, Option<String>) {
        let Some(vt) = self.ptr(obj) else { return (None, None) };
        let Some(ti) = vt.checked_sub(8).and_then(|s| self.ptr(s)) else { return (Some(vt), None) };
        let name = self
            .ptr(ti + 8)
            .and_then(|p| self.cstr(p))
            .map(|m| m.trim_start_matches(|c: char| c.is_ascii_digit()).to_string());
        (Some(vt), name)
    }

    /// Overlapping count of a little-endian u32 pattern in the raw file - the presence/absence
    /// control (`0x808033F4` must be absent while `0x80800050` occurs thousands of times).
    pub fn count_u32(&self, value: u32) -> usize {
        count_u32(self.image.data(), value)
    }
}

/// Overlapping occurrences of `value` (LE) anywhere in `data`, at any byte offset.
pub fn count_u32(data: &[u8], value: u32) -> usize {
    let pat = value.to_le_bytes();
    data.windows(4).filter(|w| *w == pat).count()
}

// ── (1) the transport message registry (bap-message-table.py) ─────────────────────────────────

/// One row of the 58-entry registry at [`REGISTRY`].
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// Registry position, 0..58. **Not** id order (echo is entry 4, id 250).
    pub entry: usize,
    pub vtable: u64,
    /// Slot 0 target - where the id thunk lives (kept so a test can perturb the exact byte).
    pub id_thunk_va: Option<u64>,
    /// Slot 0 (`+0x00`): the transport message id.
    pub id: Thunk,
    /// Slot 1 (`+0x08`): the **header class** (0..=7). ⛔ Not "category" - that gloss is
    /// retracted in [`crate::tables::MESSAGE_CLASS`]'s doc.
    pub header_class: Thunk,
    /// `+0x70` - **beyond the 8-slot vtable**; layout pin and TSV parity only. See module doc.
    pub beyond_pair_id: Thunk,
    /// `+0x78` - same caveat.
    pub beyond_pair_class: Thunk,
    /// RTTI class name, e.g. `c_investment_bap_message_client_to_bap_echo_request`.
    pub class_name: String,
    /// [`TransportMessage::class_name`] with the `c_investment_bap_message_` prefix stripped.
    pub short: String,
    /// The C string the slot at `+0x48` points to. This is the RTTI **mangled** name (the slot is
    /// past the vtable end); carried only because the Python TSV has the column.
    pub name_string: String,
}

/// Walk the registry. Errors if any of the 58 slots fails to resolve to a vtable - a partial
/// registry is not this table.
pub fn transport_registry(r: &Reader) -> Result<Vec<TransportMessage>, String> {
    let mut out = Vec::with_capacity(REGISTRY_COUNT);
    for entry in 0..REGISTRY_COUNT {
        let slot = REGISTRY + entry as u64 * 8;
        let vtable =
            r.ptr(slot).ok_or_else(|| format!("registry slot {entry} at {slot:#x} unresolved"))?;
        let (_, class) = {
            // typeinfo hangs at vtable-8 exactly as for a singleton's vptr; reuse the walk by
            // treating the registry slot itself as the "object" whose vptr we already have.
            let ti = vtable.checked_sub(8).and_then(|s| r.ptr(s));
            let name = ti
                .and_then(|ti| r.ptr(ti + 8))
                .and_then(|p| r.cstr(p))
                .map(|m| m.trim_start_matches(|c: char| c.is_ascii_digit()).to_string());
            (ti, name)
        };
        let class_name = class.unwrap_or_default();
        let short = class_name.strip_prefix("c_investment_bap_message_").unwrap_or(&class_name).to_string();
        let name_string = r
            .ptr(vtable + 0x48)
            .and_then(|p| read_cstring_compat(r.image(), p))
            .unwrap_or_default();
        out.push(TransportMessage {
            entry,
            vtable,
            id_thunk_va: r.ptr(vtable),
            id: r.slot_thunk(vtable),
            header_class: r.slot_thunk(vtable + 0x08),
            beyond_pair_id: r.slot_thunk(vtable + 0x70),
            beyond_pair_class: r.slot_thunk(vtable + 0x78),
            class_name,
            short,
            name_string,
        });
    }
    Ok(out)
}

/// The Python's `read_cstring`, kept bug-for-bug for TSV parity: flat `+0x4000` bias, address
/// must be below `0x18C0000`, at most 120 printable-ASCII bytes. Compat only - real string reads
/// go through [`Reader::cstr`], which maps per segment.
fn read_cstring_compat(image: &Image, vaddr: u64) -> Option<String> {
    const TEXT_LIMIT: u64 = 0x18C_0000;
    const TEXT_DELTA: u64 = 0x4000;
    if vaddr >= TEXT_LIMIT {
        return None;
    }
    let data = image.data();
    let start = (vaddr + TEXT_DELTA) as usize;
    if start >= data.len() {
        return None;
    }
    let end = data[start..].iter().position(|b| *b == 0)? + start;
    if end - start > 120 {
        return None;
    }
    let raw = &data[start..end];
    if raw.is_empty() || raw.iter().any(|c| !(32..127).contains(c)) {
        return None;
    }
    Some(String::from_utf8_lossy(raw).into_owned())
}

// ── (2) the policy families (bap-message-idspace.py + the POLICIES derivation) ────────────────

/// One of the 33 `c_investment_bap_message_policy_*` families.
#[derive(Debug, Clone)]
pub struct PolicyFamily {
    pub index: usize,
    /// From the parallel name table at [`POLICY_NAMES`]. ⚠ Display only: rows 0 and 1 of this
    /// table are swapped relative to the singletons. Comparisons use [`PolicyFamily::rtti_class`].
    pub name_string: Option<String>,
    pub singleton: Option<u64>,
    pub vtable: Option<u64>,
    /// RTTI class name of the singleton actually returned by `0x55F400(index)`.
    pub rtti_class: Option<String>,
    /// Vtable slot 2 (`+0x10`): the request id this policy governs.
    pub request: Thunk,
    /// Vtable slot 6 (`+0x30`): the response id, `xor eax,eax` (0) = fire-and-forget.
    pub response: Thunk,
}

/// Walk the 33 policy families.
pub fn policy_families(r: &Reader) -> Vec<PolicyFamily> {
    (0..POLICY_COUNT)
        .map(|index| {
            let name_string = r.ptr(POLICY_NAMES + index as u64 * 8).and_then(|p| r.cstr(p));
            let singleton = r.ptr(POLICY_PTRS + index as u64 * 8);
            let (vtable, rtti_class) = match singleton {
                Some(obj) => r.rtti(obj),
                None => (None, None),
            };
            let (request, response) = match vtable {
                Some(vt) => (r.slot_thunk(vt + 0x10), r.slot_thunk(vt + 0x30)),
                None => (Thunk::Other, Thunk::Other),
            };
            PolicyFamily { index, name_string, singleton, vtable, rtti_class, request, response }
        })
        .collect()
}

/// The two words after the policy table (`[33]`, `[34]`) - the "table really ends" control reads
/// them and demands `(None, None)`.
pub fn policy_table_end(r: &Reader) -> (Option<u64>, Option<u64>) {
    (r.ptr(POLICY_PTRS + POLICY_COUNT as u64 * 8), r.ptr(POLICY_PTRS + (POLICY_COUNT as u64 + 1) * 8))
}

// ── (3) the push accept-set ───────────────────────────────────────────────────────────────────

/// The ids of the non-null singletons at [`PUSH_TABLE`], in slot order.
pub fn push_accept_ids(r: &Reader) -> Vec<u16> {
    (0..PUSH_SLOTS)
        .filter_map(|i| r.ptr(PUSH_TABLE + i as u64 * 8))
        .filter_map(|obj| r.ptr(obj))
        .filter_map(|vt| r.thunk_at(r.ptr(vt)?).value())
        .map(|v| v as u16)
        .collect()
}

// ── (4) the header codec lengths ──────────────────────────────────────────────────────────────

/// Inner-header length per header class: 8 codec singletons, vtable slot 1 is the length thunk.
pub fn header_lengths(r: &Reader) -> Result<[usize; 8], String> {
    let mut out = [0usize; 8];
    for (class, slot) in out.iter_mut().enumerate() {
        let obj = r
            .ptr(HEADER_CODECS + class as u64 * 8)
            .ok_or_else(|| format!("header codec {class} unresolved"))?;
        let vt = r.ptr(obj).ok_or_else(|| format!("header codec {class} has no vptr"))?;
        *slot = r
            .slot_thunk(vt + 0x08)
            .value()
            .ok_or_else(|| format!("header codec {class} slot 1 is not a constant thunk"))?
            as usize;
    }
    Ok(out)
}

// ── (5) the investment message definition table ───────────────────────────────────────────────

/// One row of `investment-message-definitions.bin`: 0x40-byte record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefRow {
    /// Row index - what the envelope's `+0x00` and `0x550A40` carry. Not the message id.
    pub index: usize,
    /// `+0x00`: FNV name hash.
    pub name_hash: u32,
    /// `+0x10`: the investment message id that goes on the wire (206, 503, ...).
    pub message_id: u32,
    /// `+0x14`: request schema tag.
    pub request_tag: u32,
    /// `+0x18`: response schema tag - exactly what `0x550A40(index)` returns `[rax+0x18]` of.
    pub response_tag: u32,
}

/// Parse the definition file. **Refuses** (does not warn) if `0x40 + count*0x40 != len` - the
/// stride check is what pins the record layout, and a file that fails it is not this format.
pub fn read_definitions(data: &[u8]) -> Result<Vec<DefRow>, String> {
    if data.len() < 0x40 {
        return Err(format!("definition file too short: {} bytes", data.len()));
    }
    let count = u32::from_le_bytes(data[0x18..0x1C].try_into().unwrap()) as usize;
    if 0x40 + count * 0x40 != data.len() {
        return Err(format!(
            "stride check failed: 0x40 + {count}*0x40 != {} - this is not the definition table",
            data.len()
        ));
    }
    let u32_at = |off: usize| u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
    Ok((0..count)
        .map(|index| {
            let r = 0x40 + index * 0x40;
            DefRow {
                index,
                name_hash: u32_at(r),
                message_id: u32_at(r + 0x10),
                request_tag: u32_at(r + 0x14),
                response_tag: u32_at(r + 0x18),
            }
        })
        .collect())
}

// ── (6) repeated-field walk over the schema descriptors ───────────────────────────────────────

/// Count of fields anywhere in `tag`'s schema tree whose kind carries [`ARRAY_BIT`].
///
/// Semantics are the Python's `array_fields`, preserved exactly because the TSV is golden-diffed
/// against it: one **shared** visited set across the whole recursion (a tag expanded in one
/// branch is skipped in a sibling branch), depth cap 6, and the field walk stops at the first
/// missing sentinel.
pub fn array_field_count(image: &Image, idx: &DescIndex, tag: u32) -> usize {
    fn rec(image: &Image, idx: &DescIndex, tag: u32, depth: u32, seen: &mut HashSet<u32>) -> usize {
        if depth > 6 || !seen.insert(tag) {
            return 0;
        }
        let Some(d) = idx.get(tag) else { return 0 };
        let mut n = 0;
        for (_, field) in descriptors::fields(image, d.anchor, d.field_count) {
            let Some(f) = field else { break };
            if f.kind & ARRAY_BIT != 0 {
                n += 1;
            }
            if f.child_tag != LEAF {
                n += rec(image, idx, f.child_tag, depth + 1, seen);
            }
        }
        n
    }
    rec(image, idx, tag, 0, &mut HashSet::new())
}

// ── drift checks: derived vs the consts in tables.rs ──────────────────────────────────────────

/// Derived registry vs [`tables::MESSAGE_CLASS`]. `Err` is a **drift finding**.
pub fn diff_message_class(rows: &[TransportMessage]) -> Result<(), String> {
    let mut derived: Vec<(u16, u8, &str)> = rows
        .iter()
        .map(|m| {
            let id = m.id.value().ok_or_else(|| format!("{}: id is not a constant", m.short))?;
            let class = m
                .header_class
                .value()
                .ok_or_else(|| format!("{}: header class is not a constant", m.short))?;
            Ok((id as u16, class as u8, m.short.as_str()))
        })
        .collect::<Result<_, String>>()?;
    derived.sort_unstable();
    let mut consts: Vec<(u16, u8, &str)> = tables::MESSAGE_CLASS.to_vec();
    consts.sort_unstable();
    if derived == consts {
        return Ok(());
    }
    let drift: Vec<String> = derived
        .iter()
        .filter(|d| !consts.contains(d))
        .map(|d| format!("derived-only {d:?}"))
        .chain(consts.iter().filter(|c| !derived.contains(c)).map(|c| format!("const-only {c:?}")))
        .collect();
    Err(format!("MESSAGE_CLASS drift: {}", drift.join("; ")))
}

/// Derived policy families vs the `PolicyVtable` rows of [`tables::POLICIES`], compared as
/// multisets of `(request, response, class)` - the singleton table's order is not the const
/// table's order, and must not need to be.
pub fn diff_policies(families: &[PolicyFamily]) -> Result<(), String> {
    const PREFIX: &str = "c_investment_bap_message_policy_";
    let mut derived: Vec<(u16, Option<u16>, String)> = families
        .iter()
        .map(|f| {
            let class = f
                .rtti_class
                .as_deref()
                .and_then(|c| c.strip_prefix(PREFIX))
                .ok_or_else(|| format!("policy {} has no {PREFIX}* RTTI name", f.index))?;
            let req = f
                .request
                .value()
                .ok_or_else(|| format!("policy {class}: slot 2 is not a constant"))?;
            let rsp = f
                .response
                .value()
                .ok_or_else(|| format!("policy {class}: slot 6 is not a constant"))?;
            Ok((req as u16, (rsp != 0).then_some(rsp as u16), class.to_string()))
        })
        .collect::<Result<_, String>>()?;
    derived.sort();
    let mut consts: Vec<(u16, Option<u16>, String)> = tables::POLICIES
        .iter()
        .filter(|p| p.source == tables::PolicySource::PolicyVtable)
        .map(|p| (p.request, p.response, p.policy_class.to_string()))
        .collect();
    consts.sort();
    if derived == consts {
        return Ok(());
    }
    let drift: Vec<String> = derived
        .iter()
        .filter(|d| !consts.contains(d))
        .map(|d| format!("derived-only {d:?}"))
        .chain(consts.iter().filter(|c| !derived.contains(c)).map(|c| format!("const-only {c:?}")))
        .collect();
    Err(format!("POLICIES drift: {}", drift.join("; ")))
}

/// Derived push ids vs [`tables::PUSH_ACCEPT_SET`].
pub fn diff_push_set(mut ids: Vec<u16>) -> Result<(), String> {
    ids.sort_unstable();
    let mut consts = tables::PUSH_ACCEPT_SET;
    consts.sort_unstable();
    if ids == consts {
        Ok(())
    } else {
        Err(format!("PUSH_ACCEPT_SET drift: derived {ids:?} vs const {consts:?}"))
    }
}

/// Derived header lengths vs [`tables::HEADER_LEN`].
pub fn diff_header_lens(lens: [usize; 8]) -> Result<(), String> {
    if lens == tables::HEADER_LEN {
        Ok(())
    } else {
        Err(format!("HEADER_LEN drift: derived {lens:?} vs const {:?}", tables::HEADER_LEN))
    }
}

// ── controls ──────────────────────────────────────────────────────────────────────────────────

/// One control's outcome. A `false` anywhere means the derivation may not be cited.
#[derive(Debug, Clone)]
pub struct Control {
    pub ok: bool,
    pub label: &'static str,
    pub detail: String,
}

fn ctl(ok: bool, label: &'static str, detail: String) -> Control {
    Control { ok, label, detail }
}

/// The 18 transport ids pinned from live wire traffic (`d1-bap/src/lib.rs` constants) - the
/// control that makes this a static reading checked against `observed-live` bytes rather than
/// against itself.
pub const LIVE_CONFIRMED: &[(&str, u16)] = &[
    ("client_to_bap_echo_request", 0x00FA),
    ("client_to_bap_echo_response", 0x00FB),
    ("client_to_bap_secure_hello_response", 0x001A),
    ("client_to_bap_queuez_register_request", 0x0079),
    ("client_to_bap_queuez_register_response", 0x007A),
    ("queuez_to_client_subscription_lost_notification", 0x007C),
    ("client_to_bap_client_config_request", 0x0012),
    ("client_to_bap_client_config_response", 0x0013),
    ("client_to_bap_account_id_translation_request", 0x0017),
    ("client_to_bap_account_id_translation_response", 0x0018),
    ("client_to_xetrov_request", 0x00AB),
    ("client_to_bap_register_relay_client_request", 0x012E),
    ("client_to_bap_register_relay_client_response", 0x012F),
    ("client_to_bap_channel_startup_request", 0x001E),
    ("client_to_bap_channel_startup_response", 0x001F),
    ("bap_to_client_user_message_request", 0x0020),
    ("bap_to_client_user_message_response", 0x0021),
    ("client_to_bap_update_purchased_offers_request", 0x0015),
];

/// Definition rows whose id and schema tags `d1-bap` records independently (message 206 and 503).
pub const DEF_ROW_CONTROLS: &[(usize, u32, u32, u32)] =
    &[(59, 206, 0x8080_1774, 0x8080_1869), (76, 503, 0x8080_1726, 0x8080_1825)];

/// The seven activity-host ids - the deliverable - asserted by name so a regression is loud.
pub const ACTIVITY_HOST_IDS: &[(&str, u16)] = &[
    ("client_to_activity_host_manager_request", 6),
    ("client_to_activity_host_manager_response", 7),
    ("client_to_activity_host_notification", 8),
    ("client_to_bap_get_activity_host_proxy_request", 0x10),
    ("client_to_bap_get_activity_host_proxy_response", 0x11),
    ("activity_host_to_client_notification", 100),
    ("activity_host_to_client_log_notification", 102),
];

/// Every control both Python tools carried, plus the four drift checks that are the point of the
/// port. `defs` may be absent (`None`) - its controls then FAIL rather than silently skip.
pub fn run_controls(r: &Reader, defs: Option<&[DefRow]>) -> Vec<Control> {
    let mut out = run_eboot_controls(r);

    // The definition-table controls.
    match defs {
        None => out.push(ctl(
            false,
            "definition rows 59/76 are messages 206/503 with the recorded schema tags",
            "definition file absent - control cannot run".into(),
        )),
        Some(defs) => {
            let by_index: HashMap<usize, &DefRow> = defs.iter().map(|d| (d.index, d)).collect();
            let bad: Vec<String> = DEF_ROW_CONTROLS
                .iter()
                .filter(|(idx, mid, req, rsp)| {
                    by_index.get(idx).map(|d| (d.message_id, d.request_tag, d.response_tag))
                        != Some((*mid, *req, *rsp))
                })
                .map(|(idx, ..)| format!("row {idx} mismatch"))
                .collect();
            out.push(ctl(
                bad.is_empty(),
                "definition rows 59/76 are messages 206/503 with the recorded schema tags",
                if bad.is_empty() { "2/2".into() } else { bad.join("; ") },
            ));
        }
    }

    out
}

/// The eboot-side controls only (everything in [`run_controls`] except the definition-row
/// control). This is what gates a dump that does not consume the definition file - both Python
/// tools ran their controls before EVERY dump and refused to emit on a failure, and that refusal
/// must survive the port: a table derived from an image that fails its controls is a confident
/// wrong answer, not a table.
pub fn run_eboot_controls(r: &Reader) -> Vec<Control> {
    let mut out = Vec::new();

    // The addresses in this module are for the pinned 01.29 build and no other.
    let sha = r.image().sha256();
    out.push(ctl(
        sha == PINNED_EBOOT_SHA256,
        "image is the pinned 01.29 build (fixed table addresses apply)",
        format!("sha256 {}...", &sha[..12]),
    ));

    // Presence/absence pair: the absent tag, and the control that proves the counter counts.
    let absent = r.count_u32(0x8080_33F4);
    let present = r.count_u32(0x8080_0050);
    out.push(ctl(
        absent == 0 && present > 1000,
        "tag 0x808033F4 absent while control tag 0x80800050 is everywhere",
        format!("absent={absent} present={present}"),
    ));

    // The policy table really is 33 long.
    let end = policy_table_end(r);
    out.push(ctl(
        end == (None, None),
        "policy singleton table ends at [33] ([33] and [34] are null)",
        format!("{end:?}"),
    ));
    let families = policy_families(r);
    let named = families
        .iter()
        .filter(|f| {
            f.rtti_class.as_deref().is_some_and(|c| c.starts_with("c_investment_bap_message_policy_"))
        })
        .count();
    out.push(ctl(
        named == POLICY_COUNT,
        "all 33 policy singletons RTTI-name themselves c_investment_bap_message_policy_*",
        format!("{named}/{POLICY_COUNT}"),
    ));

    // The registry and its per-row invariants.
    match transport_registry(r) {
        Err(e) => out.push(ctl(false, "the 58-entry registry resolves", e)),
        Ok(rows) => {
            out.push(ctl(
                rows.len() == REGISTRY_COUNT,
                "the registry at 0x1922110 holds 58 message classes",
                format!("{}", rows.len()),
            ));
            let unnamed = rows.iter().filter(|m| m.class_name.is_empty()).count();
            out.push(ctl(
                unnamed == 0,
                "every registry entry resolves to an RTTI class",
                format!("{unnamed} unnamed"),
            ));
            let no_id = rows.iter().filter(|m| m.id.mov_value().is_none()).count();
            out.push(ctl(
                no_id == 0,
                "every message's vtable slot 0 is a mov-eax constant thunk",
                format!("{no_id} without"),
            ));
            let ids: Vec<u32> = rows.iter().filter_map(|m| m.id.value()).collect();
            let unique: HashSet<u32> = ids.iter().copied().collect();
            out.push(ctl(
                unique.len() == ids.len(),
                "message ids are unique",
                format!("{}/{}", unique.len(), ids.len()),
            ));

            let by_short: HashMap<&str, &TransportMessage> =
                rows.iter().map(|m| (m.short.as_str(), m)).collect();
            let mismatched: Vec<String> = LIVE_CONFIRMED
                .iter()
                .filter(|(short, want)| {
                    by_short.get(short).and_then(|m| m.id.value()) != Some(*want as u32)
                })
                .map(|(short, want)| {
                    format!(
                        "{short} expected {want:#x} got {:?}",
                        by_short.get(short).map(|m| m.id)
                    )
                })
                .collect();
            out.push(ctl(
                mismatched.is_empty(),
                "all 18 live-confirmed wire ids reproduce from the vtables",
                if mismatched.is_empty() {
                    format!("{}/18", LIVE_CONFIRMED.len())
                } else {
                    mismatched.join("; ")
                },
            ));

            // Layout pins on the beyond-vtable read (see module doc - these pin the linker
            // layout the Python's pair_id column depended on, not a semantic pairing).
            let echo_req = by_short.get("client_to_bap_echo_request").map(|m| m.beyond_pair_id);
            out.push(ctl(
                echo_req.and_then(|t| t.mov_value()) == Some(0xFB),
                "layout pin: echo_request +0x70 still reads 0xFB (declaration-order coincidence)",
                format!("{echo_req:?}"),
            ));
            let echo_rsp = by_short.get("client_to_bap_echo_response").map(|m| m.beyond_pair_id);
            let echo_rsp_val = echo_rsp.and_then(|t| t.mov_value());
            out.push(ctl(
                echo_rsp_val.is_some() && echo_rsp_val != Some(0xFA),
                "NEGATIVE: echo_response +0x70 is NOT its request id (never read it as a pair)",
                format!("{echo_rsp:?}"),
            ));

            let client_requests: Vec<&&TransportMessage> = by_short
                .values()
                .filter(|m| m.short.starts_with("client_to_") && m.short.ends_with("_request"))
                .collect();
            let odd = client_requests
                .iter()
                .filter(|m| m.header_class.value() != Some(1))
                .count();
            out.push(ctl(
                odd == 0 && client_requests.len() >= 18,
                "every client_to_*_request has header class 1",
                format!("{} checked, {odd} odd", client_requests.len()),
            ));

            let wrong: Vec<String> = ACTIVITY_HOST_IDS
                .iter()
                .filter(|(short, want)| {
                    by_short.get(short).and_then(|m| m.id.value()) != Some(*want as u32)
                })
                .map(|(short, want)| format!("{short} != {want}"))
                .collect();
            out.push(ctl(
                wrong.is_empty(),
                "the seven activity-host message ids are stable",
                if wrong.is_empty() { "7/7".into() } else { wrong.join("; ") },
            ));

            // ⭐ The drift checks - the reason this module exists.
            out.push(match diff_message_class(&rows) {
                Ok(()) => ctl(true, "DRIFT: derived registry == tables::MESSAGE_CLASS", "58/58".into()),
                Err(e) => ctl(false, "DRIFT: derived registry == tables::MESSAGE_CLASS", e),
            });
        }
    }

    out.push(match diff_policies(&families) {
        Ok(()) => ctl(true, "DRIFT: derived policies == tables::POLICIES (vtable rows)", "33/33".into()),
        Err(e) => ctl(false, "DRIFT: derived policies == tables::POLICIES (vtable rows)", e),
    });
    out.push(match diff_push_set(push_accept_ids(r)) {
        Ok(()) => ctl(true, "DRIFT: derived push set == tables::PUSH_ACCEPT_SET", "13/13".into()),
        Err(e) => ctl(false, "DRIFT: derived push set == tables::PUSH_ACCEPT_SET", e),
    });
    out.push(match header_lengths(r).and_then(diff_header_lens) {
        Ok(()) => ctl(true, "DRIFT: derived header lengths == tables::HEADER_LEN", "8/8".into()),
        Err(e) => ctl(false, "DRIFT: derived header lengths == tables::HEADER_LEN", e),
    });

    out
}

// ── TSV emitters (golden-diff parity with the Python) ─────────────────────────────────────────

/// `bap-message-table.py --tsv`, byte-for-byte: tab-separated, CRLF line endings (Python's `csv`
/// default), blank cells wherever the Python's mov-only thunk reader returned `None`.
pub fn message_table_tsv(rows: &[TransportMessage]) -> String {
    let cell = |t: Thunk| t.mov_value().map(|v| v.to_string()).unwrap_or_default();
    let mut out = String::from("entry\tid\tcategory\tpair_id\tpair_category\tvtable\tclass\tshort\tname_string\r\n");
    for m in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\r\n",
            m.entry,
            cell(m.id),
            cell(m.header_class),
            cell(m.beyond_pair_id),
            cell(m.beyond_pair_class),
            m.vtable,
            m.class_name,
            m.short,
            m.name_string,
        ));
    }
    out
}

/// `bap-message-idspace.py --tsv`, byte-for-byte (LF line endings - that script writes plainly).
pub fn idspace_tsv(image: &Image, idx: &DescIndex, defs: &[DefRow]) -> String {
    let mut out = String::from(
        "row_index\tmessage_id\trequest_tag\tresponse_tag\tname_hash\treq_array_fields\trsp_array_fields\n",
    );
    for d in defs {
        out.push_str(&format!(
            "{}\t{}\t{:#010x}\t{:#010x}\t{:#010x}\t{}\t{}\n",
            d.index,
            d.message_id,
            d.request_tag,
            d.response_tag,
            d.name_hash,
            array_field_count(image, idx, d.request_tag),
            array_field_count(image, idx, d.response_tag),
        ));
    }
    out
}

// ── tests ─────────────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use d1_eboot::image::PINNED_EBOOT;

    /// Local-artifact tests skip (never fail) when the pinned eboot or the definition file is
    /// absent, so the workspace tests still run on a machine without the dumps - the same
    /// convention as `d1-eboot/tests/controls.rs`.
    fn reader() -> Option<Reader> {
        if !Path::new(PINNED_EBOOT).exists() {
            eprintln!("SKIP: pinned eboot not present");
            return None;
        }
        Some(Reader::open(PINNED_EBOOT).expect("open pinned eboot"))
    }

    fn defs_bytes() -> Option<Vec<u8>> {
        if !Path::new(PINNED_DEFS).exists() {
            eprintln!("SKIP: definition file not present");
            return None;
        }
        Some(std::fs::read(PINNED_DEFS).expect("read defs"))
    }

    macro_rules! rdr {
        () => {
            match reader() {
                Some(r) => r,
                None => return,
            }
        };
    }

    // ── no local artifacts needed ─────────────────────────────────────────────────────────────

    /// The presence/absence counter counts overlapping and planted patterns - the instrument
    /// control for the tag-absence control.
    #[test]
    fn count_u32_finds_planted_and_overlapping_patterns() {
        let mut buf = vec![0u8; 64];
        assert_eq!(count_u32(&buf, 0xDEAD_BEEF), 0);
        buf[10..14].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(count_u32(&buf, 0xDEAD_BEEF), 1);
        // Overlap: 0x41414141 in five 0x41 bytes occurs twice, as Python's step-1 find counted.
        assert_eq!(count_u32(&[0x41; 5], 0x4141_4141), 2);
    }

    /// The stride check is a refusal. A truncated or padded file is not this format.
    #[test]
    fn definition_parse_refuses_a_bad_stride() {
        let mut good = vec![0u8; 0x40 + 2 * 0x40];
        good[0x18..0x1C].copy_from_slice(&2u32.to_le_bytes());
        assert!(read_definitions(&good).is_ok());
        // INVERTED: one byte shorter must refuse.
        assert!(read_definitions(&good[..good.len() - 1]).is_err());
        // INVERTED: count that disagrees with the length must refuse.
        good[0x18..0x1C].copy_from_slice(&3u32.to_le_bytes());
        assert!(read_definitions(&good).is_err());
    }

    #[test]
    fn definition_rows_read_the_documented_columns() {
        let mut data = vec![0u8; 0x40 + 0x40];
        data[0x18..0x1C].copy_from_slice(&1u32.to_le_bytes());
        let r = 0x40;
        data[r..r + 4].copy_from_slice(&0x1111_1111u32.to_le_bytes()); // name hash
        data[r + 0x10..r + 0x14].copy_from_slice(&206u32.to_le_bytes());
        data[r + 0x14..r + 0x18].copy_from_slice(&0x8080_1774u32.to_le_bytes());
        data[r + 0x18..r + 0x1C].copy_from_slice(&0x8080_1869u32.to_le_bytes());
        let rows = read_definitions(&data).unwrap();
        assert_eq!(
            rows[0],
            DefRow {
                index: 0,
                name_hash: 0x1111_1111,
                message_id: 206,
                request_tag: 0x8080_1774,
                response_tag: 0x8080_1869
            }
        );
    }

    // ── against the pinned eboot ──────────────────────────────────────────────────────────────

    /// ⭐ The reason this module exists: the const tables can never drift from their generator.
    /// A failure here is a drift FINDING - report it, do not edit either side to make it pass.
    #[test]
    fn derived_registry_matches_message_class_consts() {
        let r = rdr!();
        let rows = transport_registry(&r).expect("registry resolves");
        diff_message_class(&rows).expect("tables::MESSAGE_CLASS matches its generator");
    }

    #[test]
    fn derived_policies_match_policy_consts() {
        let r = rdr!();
        diff_policies(&policy_families(&r)).expect("tables::POLICIES matches its generator");
    }

    #[test]
    fn derived_push_set_matches_push_accept_consts() {
        let r = rdr!();
        diff_push_set(push_accept_ids(&r)).expect("tables::PUSH_ACCEPT_SET matches its generator");
    }

    #[test]
    fn derived_header_lengths_match_header_len_consts() {
        let r = rdr!();
        diff_header_lens(header_lengths(&r).expect("codec table resolves"))
            .expect("tables::HEADER_LEN matches its generator");
    }

    /// Every control the two Python tools carried passes against the real artifacts.
    #[test]
    fn all_controls_pass_against_the_pinned_artifacts() {
        let r = rdr!();
        let defs_data = defs_bytes();
        let defs = defs_data.as_deref().map(|d| read_definitions(d).expect("defs parse"));
        let controls = run_controls(&r, defs.as_deref());
        let failed: Vec<&Control> = controls.iter().filter(|c| !c.ok).collect();
        assert!(
            failed.is_empty(),
            "failed controls: {:?}",
            failed.iter().map(|c| (c.label, &c.detail)).collect::<Vec<_>>()
        );
    }

    /// INVERTED (input perturbed): flip the low immediate byte of echo_request's id thunk in an
    /// in-memory copy of the image and prove the drift check FAILS. A comparator that cannot fail
    /// is decoration.
    #[test]
    fn perturbed_image_fails_the_drift_check() {
        let r = rdr!();
        let rows = transport_registry(&r).expect("registry resolves");
        let echo = rows
            .iter()
            .find(|m| m.short == "client_to_bap_echo_request")
            .expect("echo row present");
        assert_eq!(echo.id, Thunk::Mov(0xFA));
        let thunk_va = echo.id_thunk_va.expect("echo id thunk resolved");
        let off = r.image().file_offset(thunk_va).expect("thunk maps to file") + 1; // imm low byte
        let mut data = r.image().data().to_vec();
        assert_eq!(data[off], 0xFA);
        data[off] = 0xFB; // echo_request now claims id 251 - a duplicate of echo_response
        let mutated = Reader::from_image(
            Image::from_bytes(std::path::PathBuf::from("<mutated>"), data).expect("image parses"),
        )
        .expect("reloc parses");
        let rows = transport_registry(&mutated).expect("registry still resolves");
        assert!(
            diff_message_class(&rows).is_err(),
            "a one-byte id change must be reported as drift"
        );
        // ... and the uniqueness control must fire too.
        let ids: Vec<u32> = rows.iter().filter_map(|m| m.id.value()).collect();
        let unique: HashSet<u32> = ids.iter().copied().collect();
        assert!(unique.len() < ids.len(), "the duplicate id must be visible");
    }

    /// INVERTED (input perturbed): corrupt row 59's message id in the definition bytes and prove
    /// the row control fails while the pristine bytes pass, in the same test.
    #[test]
    fn perturbed_definitions_fail_the_row_control() {
        let Some(mut data) = defs_bytes() else { return };
        let row_ok = |data: &[u8]| {
            let defs = read_definitions(data).expect("stride ok");
            DEF_ROW_CONTROLS.iter().all(|(idx, mid, req, rsp)| {
                defs.get(*idx).map(|d| (d.message_id, d.request_tag, d.response_tag))
                    == Some((*mid, *req, *rsp))
            })
        };
        assert!(row_ok(&data), "pristine file must pass (positive control)");
        let off = 0x40 + 59 * 0x40 + 0x10;
        data[off] ^= 0xFF;
        assert!(!row_ok(&data), "corrupted row 59 must fail");
    }

    /// The dump gate's two halves stay honest: an absent definition file is a FAILING control
    /// (never a silent skip), and [`run_eboot_controls`] is exactly [`run_controls`] minus that
    /// one control - so gating `message-table`/`policies` on it drops nothing else.
    #[test]
    fn defs_absent_is_a_failing_control_and_the_eboot_set_is_the_rest() {
        let r = rdr!();
        let all = run_controls(&r, None);
        let absent: Vec<&Control> = all.iter().filter(|c| !c.ok).collect();
        assert_eq!(absent.len(), 1, "exactly the defs control fails when defs are absent");
        assert!(absent[0].detail.contains("absent"));
        assert_eq!(run_eboot_controls(&r).len() + 1, all.len());
        assert!(run_eboot_controls(&r).iter().all(|c| c.ok));
    }

    /// The three xor-thunk rows the Python left blank: proven here to be `xor eax, eax; ret`
    /// (header class 0), not unreadable functions - the correction `tables.rs` carries.
    #[test]
    fn the_three_blank_category_rows_are_xor_zero_thunks() {
        let r = rdr!();
        let rows = transport_registry(&r).expect("registry resolves");
        let mut found = 0;
        for short in
            ["bap_to_client_notification", "bap_to_client_log_notification", "bap_to_client_server_shutdown_notification"]
        {
            let m = rows.iter().find(|m| m.short == short).expect("row present");
            assert_eq!(m.header_class, Thunk::Xor, "{short} slot 1 is xor eax,eax");
            found += 1;
        }
        assert_eq!(found, 3);
        // Positive control on the same reader: a mov thunk decodes as Mov, not Xor.
        let echo = rows.iter().find(|m| m.short == "client_to_bap_echo_request").unwrap();
        assert_eq!(echo.header_class, Thunk::Mov(1));
    }

    /// Golden diff, permanently executed: the Rust emitter reproduces the checked-in TSV the
    /// Python wrote, byte for byte (CRLF endings, blank mov-only cells and all).
    #[test]
    fn message_table_tsv_matches_the_python_artifact_byte_for_byte() {
        let tsv_path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../capture-rig/bap-message-table.tsv");
        if !Path::new(tsv_path).exists() {
            eprintln!("SKIP: python TSV artifact not present");
            return;
        }
        let r = rdr!();
        let rows = transport_registry(&r).expect("registry resolves");
        let expected = std::fs::read_to_string(tsv_path).expect("read tsv");
        assert_eq!(message_table_tsv(&rows), expected);
    }
}
