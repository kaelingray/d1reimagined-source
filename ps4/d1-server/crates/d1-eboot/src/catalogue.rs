// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The COMPLETE BAP / investment message catalogue, read statically from the eboot.
//!
//! Port of `capture-rig/message-catalogue.py` (735 lines), which is now superseded by this
//! module. The port removes a duplicate description of the descriptor anchor/sentinel layout:
//! those constants are consumed from [`crate::descriptors`], never re-declared.
//!
//! # WHY THIS EXISTS
//!
//! Every BAP message type this project met was discovered *reactively*: send something, watch the
//! client stall, name the thing that stalled it. This module converts "what is this new message?"
//! from an investigation into a lookup, and it answers the one question that actually costs runs:
//!
//! > **DOES THE CLIENT WANT A RESPONSE AT ALL?**
//!
//! In this protocol **answering wrongly is worse than not answering** (`status/LESSONS.md`). The
//! answer is not a naming convention and it is not `N -> N+1`; it is a literal constant in a
//! vtable, and this module reads it:
//!
//! ```text
//! c_investment_bap_message_queue::expected_reply_type   0x567EB0
//!     entry   = find_in_flight_entry(this, request_id)
//!     policy  = 0x55F400(entry->policy_family)      ; the 33-entry policy table
//!     rsp     = policy->vtable[0x30](policy)        ; <-- THE EXPECTED RESPONSE TYPE
//!     if (rsp == 0) return 0                        ; <-- ZERO MEANS "NO RESPONSE EXISTS"
//!     return g_bap_message_singletons[rsp]->vtable[0x00]()
//!
//! update_receiving_header  0xB2E116   compares that return value against the received type.
//! ```
//!
//! A policy whose slot `+0x30` is `0` can therefore **never** match an inbound frame, because
//! message type 0 is `none` and is never on the wire. Replying to such a message is guaranteed to
//! land in the "failed to find recipient for header" path. That is exactly the measured behaviour
//! of `0x00AB` (XETROV), and it is derivable for every message at once.
//!
//! # THE THREE ID SPACES (do not conflate them)
//!
//! 1. **BAP envelope type**, `u16be`, `< 0x130`, in the 6-byte inner base header. 63 of the 304
//!    possible ids have a message class in this build. Enumerated from RTTI, NOT from the log-name
//!    table at `0xB32410` (which is coarser and disagrees).
//! 2. **Policy family index**, `0..32`, the argument to `0x55F400`/`0x55F410`. This is where the
//!    request->response pairing lives. Plus 13 *push* policies for server-initiated frames that
//!    are dispatched by type alone with no in-flight entry.
//! 3. **Investment message id** (104..1901), riding inside envelope type `0x000A`. A different
//!    table, and it is **not in the eboot**: it is shipped content (pkg 303 entry 890), extracted
//!    to `capture-rig/schemas/investment-message-definitions.bin`. `0x27FDD0` is a LINEAR SEARCH
//!    over that content table, not a static id->index map.
//!
//! # PYTHON-PARITY ADDRESSING
//!
//! The Python prototype addresses the whole 29 MB file through the single global bias `0x4000`
//! and scans the whole file for byte patterns, whereas [`crate::image`] is per-segment for good
//! reason (ATLAS 4.1). For this image every LOAD segment happens to carry the same bias, which is
//! precisely why the Python was right; [`Reader::new`] asserts that coincidence and refuses any
//! image where it does not hold, so the parity reads below can never silently corrupt on a
//! different build. The rendered output is pinned byte-for-byte against the Python's own output
//! (`tests/fixtures/message-catalogue-full.txt`, produced by running the prototype), so the port
//! is verified by an executed golden diff, not by an asserted one.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Serialize;

use crate::descriptors::{self, ANCHOR, LEAF};
use crate::image::{Image, ImageError, Result};
use crate::reloc::{self, R_X86_64_RELATIVE};

/// Anything above this is data, not a function pointer.
pub const TEXT_MAX: u64 = 0x130_0000;

/// `0x55F410` -> display-name string, indexed by policy family.
pub const POLICY_NAMES: u64 = 0x1923AA0;
/// `0x55F400` -> policy singleton, indexed by policy family.
pub const POLICY_PTRS: u64 = 0x1923CB0;
pub const POLICY_COUNT: usize = 33;
/// `0x55C1E0` -> header-class singleton, indexed by class id.
pub const HEADER_CLASS_TABLE: u64 = 0x19234A0;
pub const HEADER_CLASS_COUNT: usize = 8;
/// protocol_source: request kind (1-based) -> reply type.
pub const KIND_TO_REPLY: u64 = 0x15D1B40;
pub const KIND_COUNT: usize = 5;
/// `0xB32410`'s jump table, types 0x00..0x2F.
pub const TYPE_NAME_JT: u64 = 0xB32714;
/// `0xB32410`'s second jump table, types 0x12C..0x12F.
pub const TYPE_NAME_JT_HI: u64 = 0xB32465;
/// Every message class has this at vtable+0x20 - the coverage control's other instrument.
pub const GENERIC_TYPE_NAME_FN: u64 = 0x559D30;

// ---- pinned controls -----------------------------------------------------------------------
// historicaldocs/design/rsat-character-object.md section 4: checksum, descriptor tag, field
// count, span in bits, and (character only) the five child tags in order.
pub const PINNED_SCHEMAS: [(&str, u32, u32, u32, u32, Option<[u32; 6]>); 4] = [
    ("character", 0x3431_F1BA, 0x8080_1954, 6, 0x2C0,
     Some([0xFFFF_FFFF, 0x8080_17FC, 0x8080_181A, 0x8080_1755, 0x8080_1855, 0x8080_1923])),
    ("roster", 0xA2AC_2C74, 0x8080_1951, 4, 0x004, None),
    ("account", 0x64AE_75BF, 0x8080_1803, 1, 0x1E00, None),
    ("profile", 0x8127_F15A, 0x8080_1743, 1, 0x181D, None),
];
/// bap-protocol.md section A.2.1.
pub const PINNED_HEADER_SIZES: [u32; 8] = [6, 6, 8, 14, 6, 24, 32, 8];
/// `capture-rig/schemas/investment-503-schema.txt` came from a LIVE registry walk on a running
/// client - the one control whose reference came from a different instrument entirely.
pub const PINNED_503_RSP: (u32, u32, u32, [u32; 5], [u32; 5]) = (
    0x8080_1825, 5, 968,
    [0x1, 0x6, 0x9, 0x9, 0x1],
    [0x8080_1796, LEAF, LEAF, LEAF, 0x8080_1941],
);
/// xetrov one-way; subscription paired. Order matters: the Python prints 19 first.
pub const PINNED_POLICIES: [(usize, u32, u32); 2] = [(19, 0xAB, 0x0000), (13, 0x0C, 0x000D)];
pub const PINNED_DEFS: [(usize, u32, u32, u32); 2] =
    [(59, 206, 0x8080_1774, 0x8080_1869), (76, 503, 0x8080_1726, 0x8080_1825)];
pub const ABSENT_TAG: u32 = 0x8080_33F4;
pub const PRESENT_TAG: u32 = 0x8080_0050;

// ---- typed rows ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct EnvelopeClass {
    #[serde(rename = "type")]
    pub ty: u32,
    pub class: String,
    pub vtable: u64,
    pub header_class: Option<u32>,
    pub encode_payload: Option<u64>,
    pub decode_payload: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeaderClass {
    pub index: usize,
    pub singleton: Option<u64>,
    pub vtable: Option<u64>,
    pub class: Option<String>,
    pub size: Option<u32>,
    pub flags: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyFamily {
    pub family: usize,
    pub display_name: Option<String>,
    pub class: Option<String>,
    pub singleton: Option<u64>,
    pub vtable: Option<u64>,
    /// Vtable slot `+0x10` - the request type this family is keyed on.
    pub request_type: Option<u32>,
    /// Vtable slot `+0x30` - THE ANSWER. `Some(0)` means "no response exists; do not answer".
    pub response_type: Option<u32>,
    pub slot0: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PushPolicy {
    pub class: String,
    pub vtable: u64,
    #[serde(rename = "type")]
    pub ty: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaRecord {
    pub record: u64,
    pub anchor: u64,
    pub descriptor: u64,
    /// The QueueZ `instance_type` on the wire.
    pub checksum: u32,
    pub tag: u32,
    pub decoded_size: u32,
    pub fields: u32,
    pub span_bits: u32,
    pub record_len: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvestmentDef {
    pub row: usize,
    pub message_id: u32,
    pub request_tag: u32,
    pub response_tag: u32,
    pub name_hash: u32,
    /// The record is 0x40 bytes and EVERY other word is a real zero (content carries no
    /// relocations). There is no flags field - which is exactly why "does this investment
    /// message need a reply" is NOT answerable from this table; the envelope layer answers it.
    pub unexpected_nonzero_offsets: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DescSummary {
    pub tag: u32,
    pub anchor: u64,
    pub descriptor: u64,
    pub fields: u32,
    pub decoded_size: u32,
    pub span_bits: u32,
}

/// Everything the catalogue derives, in one pass over the image.
pub struct Data {
    /// Layer 1: envelope type id -> message class, built from RTTI.
    pub cat: BTreeMap<u32, EnvelopeClass>,
    /// The same id set reached with a different instrument (the generic type-name thunk at
    /// vtable+0x20) - the COVERAGE control for `cat`.
    pub shape: BTreeMap<u32, u64>,
    pub hdrs: Vec<HeaderClass>,
    /// Layer 2: the 33 policy families - where "response required" actually lives.
    pub pols: Vec<PolicyFamily>,
    pub pushes: Vec<PushPolicy>,
    /// protocol_source request kind (1-based) -> reply type.
    pub kinds: Vec<(u32, u32)>,
    /// `0xB32410`'s type -> log string. COARSER than the class table and it disagrees; kept only
    /// so the disagreements can be printed.
    pub lognames: BTreeMap<u32, Option<String>>,
    /// Layer 3: tag -> descriptor anchor (Python-parity: first anchor wins, no count filter).
    pub idx: HashMap<u32, u64>,
    pub anchors: Vec<u64>,
    pub recs: Vec<SchemaRecord>,
    /// Layer 4: the investment sub-id space (CONTENT, not eboot). `None` when the definitions
    /// file was not supplied - the eboot-only layers still work.
    pub defs: Option<Vec<InvestmentDef>>,
    /// Layer 5: our own `d1-bap` TYPE_* constants, for the cross-check.
    pub d1bap: Vec<(String, u16)>,
}

// ---- the Python-parity reader --------------------------------------------------------------

/// Raw reads through the uniform LOAD bias, plus the relocation map and its reverse index.
///
/// Relocated words read as ZERO in the file image. Never read a pointer with a raw u64; always go
/// through [`Reader::ptr`]. The reverse index is what makes "who points at this typeinfo?" a hash
/// lookup instead of a scan.
pub struct Reader<'a> {
    img: &'a Image,
    bias: u64,
    /// slot -> (r_type, addend), all kinds, RELA then JMPREL (the Python's `self.rel`).
    rel: HashMap<u64, (u32, u64)>,
    /// addend -> slots, RELATIVE only, in table order (the Python's `self.rev`).
    rev: HashMap<u64, Vec<u64>>,
}

impl<'a> Reader<'a> {
    pub fn new(img: &'a Image) -> Result<Self> {
        let mut loads = img.load_segments();
        let bias = loads.next().ok_or(ImageError::NotElf64Le)?.bias();
        // The parity guard: the Python's single global bias is only correct because every LOAD
        // segment of this image shares it. Refuse rather than silently mis-address (ATLAS 4.1).
        if bias < 0 || loads.any(|s| s.bias() != bias) {
            return Err(ImageError::NotInLoad { offset: 0 });
        }
        let mut rel = HashMap::new();
        let mut rev: HashMap<u64, Vec<u64>> = HashMap::new();
        let mut all = reloc::all(img)?;
        all.extend(reloc::plt(img)?);
        for r in all {
            rel.insert(r.slot, (r.r_type, r.addend));
            if r.r_type == R_X86_64_RELATIVE {
                rev.entry(r.addend).or_default().push(r.slot);
            }
        }
        Ok(Self { img, bias: bias as u64, rel, rev })
    }

    fn data(&self) -> &[u8] {
        self.img.data()
    }
    fn inside(&self, va: u64) -> bool {
        (va + self.bias) as usize + 8 <= self.data().len()
    }
    fn raw(&self, va: u64, len: usize) -> Option<&[u8]> {
        let o = va.checked_add(self.bias)? as usize;
        self.data().get(o..o.checked_add(len)?)
    }
    pub fn u16(&self, va: u64) -> Option<u16> {
        Some(u16::from_le_bytes(self.raw(va, 2)?.try_into().unwrap()))
    }
    pub fn u32(&self, va: u64) -> Option<u32> {
        Some(u32::from_le_bytes(self.raw(va, 4)?.try_into().unwrap()))
    }
    pub fn i32(&self, va: u64) -> Option<i32> {
        Some(i32::from_le_bytes(self.raw(va, 4)?.try_into().unwrap()))
    }
    pub fn u64(&self, va: u64) -> Option<u64> {
        Some(u64::from_le_bytes(self.raw(va, 8)?.try_into().unwrap()))
    }

    /// Relocation-aware pointer read: the RELATIVE addend when the slot is relocated, else the
    /// raw word, with 0 mapping to `None` (zero is never a valid pointer here).
    pub fn ptr(&self, va: u64) -> Option<u64> {
        if let Some(&(kind, addend)) = self.rel.get(&va) {
            if kind == R_X86_64_RELATIVE {
                return Some(addend);
            }
        }
        if !self.inside(va) {
            return None;
        }
        match self.u64(va) {
            Some(0) | None => None,
            v => v,
        }
    }

    /// NUL-terminated ASCII at `va` (bytes past 0x7F become U+FFFD, as Python's `ascii/replace`).
    pub fn cstr(&self, va: u64, limit: usize) -> Option<String> {
        let o = va.checked_add(self.bias)? as usize;
        if o >= self.data().len() {
            return None;
        }
        let end = (o + limit).min(self.data().len());
        let slice = &self.data()[o..end];
        let n = slice.iter().position(|&b| b == 0)?;
        Some(slice[..n].iter().map(|&b| if b < 0x80 { b as char } else { '\u{FFFD}' }).collect())
    }

    /// vtable -> typeinfo (vt-8) -> mangled name (typeinfo+8), stripped of its length prefix.
    pub fn rtti_name(&self, vt: u64) -> Option<String> {
        let ti = self.ptr(vt.checked_sub(8)?)?;
        if !self.inside(ti) {
            return None;
        }
        let s = self.cstr(self.ptr(ti + 8)?, 400)?;
        Some(strip_digits(&s))
    }

    /// `mov eax, imm32; ret` / `xor eax,eax; ret` / `mov al, imm8; ret` -> the constant.
    pub fn const_fn(&self, va: u64) -> Option<u32> {
        if !self.inside(va) {
            return None;
        }
        let o = (va + self.bias) as usize;
        let b = &self.data()[o..(o + 8).min(self.data().len())];
        if b.len() >= 6 && b[0] == 0xB8 && b[5] == 0xC3 {
            return Some(u32::from_le_bytes(b[1..5].try_into().unwrap()));
        }
        if b.len() >= 3 && (b[0] == 0x31 || b[0] == 0x33) && b[1] == 0xC0 && b[2] == 0xC3 {
            return Some(0);
        }
        if b.len() >= 3 && b[0] == 0xB0 && b[2] == 0xC3 {
            return Some(b[1] as u32);
        }
        None
    }

    /// Occurrences of a u32 anywhere in the file, overlapping, whole image (the Python counts the
    /// raw file, and this is a presence/absence control, not an address conversion).
    pub fn count_u32(&self, value: u32) -> usize {
        let pat = value.to_le_bytes();
        let d = self.data();
        let mut n = 0usize;
        let mut i = 0usize;
        while i + 4 <= d.len() {
            match find(&d[i..], &pat) {
                Some(k) => {
                    n += 1;
                    i += k + 1;
                }
                None => break,
            }
        }
        n
    }

    /// Every real vtable whose class name contains `needle`.
    ///
    /// Three hops, all through the reverse relocation index: name string <- typeinfo+8 <-
    /// vtable-8. A "real vtable" is filtered by two independent facts: offset-to-top
    /// (vtable-0x10) is 0, and slot 0 resolves into .text. Without that filter the hop also
    /// returns `__si_class_type_info` base pointers, which are typeinfo interiors, not vtables.
    pub fn vtables_by_rtti(&self, needle: &str) -> Vec<(String, u64)> {
        let d = self.data();
        let nb = needle.as_bytes();
        let mut out = Vec::new();
        let mut starts = HashSet::new();
        let mut i = 0usize;
        while let Some(k) = find(&d[i..], nb) {
            let p = i + k;
            i = p + 1;
            // Expand to the word-character run around the needle; require a terminating NUL.
            let mut l = p;
            while l > 0 && is_word(d[l - 1]) {
                l -= 1;
            }
            let mut r = p + nb.len();
            while r < d.len() && is_word(d[r]) {
                r += 1;
            }
            if r >= d.len() || d[r] != 0 {
                continue;
            }
            // The regex is `[0-9]{1,3}c_[A-Za-z0-9_]*needle...`: 1..=3 digits then `c_`. With a
            // longer digit run the leftmost match starts three digits before the `c`.
            let mut k_dig = 0usize;
            while l + k_dig < r && d[l + k_dig].is_ascii_digit() {
                k_dig += 1;
            }
            if k_dig == 0 || l + k_dig + 1 >= r || d[l + k_dig] != b'c' || d[l + k_dig + 1] != b'_'
            {
                continue;
            }
            let mstart = if k_dig > 3 { l + (k_dig - 3) } else { l };
            if !starts.insert(mstart) {
                continue; // two needle hits inside one string are one regex match
            }
            let name = strip_digits(&String::from_utf8_lossy(&d[mstart..r]));
            let Some(nva) = (mstart as u64).checked_sub(self.bias) else { continue };
            for &site in self.rev.get(&nva).map(|v| v.as_slice()).unwrap_or(&[]) {
                let ti = site - 8; // typeinfo: {vptr, name, ...}
                for &s2 in self.rev.get(&ti).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let vt = s2 + 8;
                    if self.u64(s2.wrapping_sub(8)).unwrap_or(1) != 0 {
                        continue; // offset-to-top must be 0
                    }
                    match self.ptr(vt) {
                        Some(f0) if f0 <= TEXT_MAX => out.push((name.clone(), vt)),
                        _ => {}
                    }
                }
            }
        }
        out
    }
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn strip_digits(s: &str) -> String {
    let t = s.trim_start_matches(|c: char| c.is_ascii_digit());
    if t.len() == s.len() { s.to_string() } else { t.to_string() }
}

// ---- layer builders ------------------------------------------------------------------------

/// Layer 1: type id -> {name, vtable, header_class, encode, decode}. Built from RTTI.
pub fn envelope_classes(r: &Reader) -> BTreeMap<u32, EnvelopeClass> {
    let mut out = BTreeMap::new();
    for (name, vt) in r.vtables_by_rtti("bap_message") {
        if name.contains("_policy_") || name.ends_with("_header") || name.ends_with("_header_base")
        {
            continue;
        }
        let Some(t) = r.const_fn(r.ptr(vt).unwrap_or(u64::MAX)) else { continue };
        if t >= 0x130 {
            continue;
        }
        out.insert(t, EnvelopeClass {
            ty: t,
            class: name,
            vtable: vt,
            header_class: r.ptr(vt + 0x08).and_then(|f| r.const_fn(f)),
            encode_payload: r.ptr(vt + 0x10),
            decode_payload: r.ptr(vt + 0x18),
        });
    }
    out
}

/// The same id set, reached with a completely different instrument: every vtable that carries the
/// generic type-name thunk at slot +0x20. This is the COVERAGE control for [`envelope_classes`].
pub fn envelope_types_by_shape(r: &Reader) -> BTreeMap<u32, u64> {
    let mut out = BTreeMap::new();
    for &s in r.rev.get(&GENERIC_TYPE_NAME_FN).map(|v| v.as_slice()).unwrap_or(&[]) {
        let vt = s - 0x20;
        if let Some(t) = r.ptr(vt).and_then(|f| r.const_fn(f)) {
            if t < 0x130 {
                out.insert(t, vt);
            }
        }
    }
    out
}

pub fn header_classes(r: &Reader) -> Vec<HeaderClass> {
    (0..HEADER_CLASS_COUNT)
        .map(|i| {
            let s = r.ptr(HEADER_CLASS_TABLE + i as u64 * 8);
            let vt = s.and_then(|s| r.ptr(s));
            HeaderClass {
                index: i,
                singleton: s,
                vtable: vt,
                class: vt.and_then(|vt| r.rtti_name(vt)),
                size: vt.and_then(|vt| r.ptr(vt + 0x08)).and_then(|f| r.const_fn(f)),
                flags: vt.and_then(|vt| r.ptr(vt)).and_then(|f| r.const_fn(f)),
            }
        })
        .collect()
}

fn log_name_at(r: &Reader, jt: u64, off: i32) -> Option<Option<String>> {
    let tgt = jt.wrapping_add(off as i64 as u64);
    let b = r.raw(tgt, 8)?;
    // `lea rax, [rip+disp]; ret`
    if b[0] == 0x48 && b[1] == 0x8D && b[2] == 0x05 && b[7] == 0xC3 {
        let disp = i32::from_le_bytes(b[3..7].try_into().unwrap());
        Some(r.cstr(tgt.wrapping_add(7).wrapping_add(disp as i64 as u64), 400))
    } else {
        None
    }
}

/// `0xB32410`'s type -> log string. COARSER than the class table and it disagrees.
pub fn log_names(r: &Reader) -> BTreeMap<u32, Option<String>> {
    let mut out = BTreeMap::new();
    for t in 0..0x30u32 {
        if let Some(off) = r.i32(TYPE_NAME_JT + t as u64 * 4) {
            if let Some(s) = log_name_at(r, TYPE_NAME_JT, off) {
                out.insert(t, s);
            }
        }
    }
    for i in 0..4u32 {
        if let Some(off) = r.i32(TYPE_NAME_JT_HI + i as u64 * 4) {
            if let Some(s) = log_name_at(r, TYPE_NAME_JT_HI, off) {
                out.insert(0x12C + i, s);
            }
        }
    }
    out
}

/// Layer 2: the 33-entry policy table. Slot `+0x10` = request type, slot `+0x30` = RESPONSE TYPE
/// (0 = none). Semantics proven by `0x567EB0` (`expected_reply_type`), which calls
/// `0x55F400(family)` and then `policy->vtable[0x30]()`, returning 0 when that is 0.
pub fn policy_families(r: &Reader) -> Vec<PolicyFamily> {
    (0..POLICY_COUNT)
        .map(|i| {
            let obj = r.ptr(POLICY_PTRS + i as u64 * 8);
            let vt = obj.and_then(|o| r.ptr(o));
            PolicyFamily {
                family: i,
                display_name: r.ptr(POLICY_NAMES + i as u64 * 8).and_then(|p| r.cstr(p, 400)),
                class: vt.and_then(|vt| r.rtti_name(vt)),
                singleton: obj,
                vtable: vt,
                request_type: vt.and_then(|vt| r.ptr(vt + 0x10)).and_then(|f| r.const_fn(f)),
                response_type: vt.and_then(|vt| r.ptr(vt + 0x30)).and_then(|f| r.const_fn(f)),
                slot0: vt.and_then(|vt| r.ptr(vt)).and_then(|f| r.const_fn(f)),
            }
        })
        .collect()
}

/// Server-initiated frames: dispatched by TYPE ALONE through `conn[0x1138]`, no in-flight entry
/// and no sequence match.
pub fn push_policies(r: &Reader) -> Vec<PushPolicy> {
    let mut rows: Vec<PushPolicy> = r
        .vtables_by_rtti("bap_message_push_policy")
        .into_iter()
        .map(|(name, vt)| PushPolicy { class: name, ty: r.ptr(vt).and_then(|f| r.const_fn(f)), vtable: vt })
        .collect();
    rows.sort_by_key(|p| (p.ty.is_none(), p.ty));
    rows
}

pub fn kind_to_reply(r: &Reader) -> Vec<(u32, u32)> {
    (0..KIND_COUNT as u32).map(|i| (i + 1, r.u32(KIND_TO_REPLY + i as u64 * 4).unwrap_or(0))).collect()
}

/// Layer 3: tag -> anchor vaddr, plus every anchor, Python-parity (whole-file scan through the
/// uniform bias, non-overlapping stride 8, first anchor wins per tag, no field-count filter).
/// The anchor bytes come from [`crate::descriptors::ANCHOR`] - described once, consumed here.
pub fn descriptor_index(r: &Reader) -> (HashMap<u32, u64>, Vec<u64>) {
    let d = r.data();
    let mut idx = HashMap::new();
    let mut anchors = Vec::new();
    let mut start = 0usize;
    while let Some(k) = find(&d[start..], &ANCHOR) {
        let i = start + k;
        start = i + 8;
        let Some(a) = (i as u64).checked_sub(r.bias) else { continue };
        let Some(tag) = r.u32(a + 0x10) else { continue };
        anchors.push(a);
        if tag & 0xFFFF_0000 == 0x8080_0000 {
            idx.entry(tag).or_insert(a);
        }
    }
    (idx, anchors)
}

/// One descriptor's fields, stopping at the first missing sentinel exactly as the Python breaks.
/// The field layout itself lives in [`crate::descriptors::fields`] - described once.
pub fn desc_fields(image: &Image, anchor: u64, count: u32) -> Vec<descriptors::Field> {
    descriptors::fields(image, anchor, count).into_iter().map_while(|(_, f)| f).collect()
}

pub fn desc_summary(r: &Reader, idx: &HashMap<u32, u64>, tag: u32) -> Option<DescSummary> {
    let &a = idx.get(&tag)?;
    Some(DescSummary {
        tag,
        anchor: a,
        descriptor: a + 8,
        fields: r.u32(a + 0x08)?,
        decoded_size: r.u32(a + 0x14)?,
        span_bits: r.u32(a + 0x1C)?,
    })
}

/// Descriptors preceded by a 0x50-byte schema record carrying an instance checksum.
///
/// RECORD BASE = anchor - 0x50, on direct code evidence: `0x56B6F8` resolves a runtime schema tag
/// to an object and reads `object + 0x08` to compare against the wire `instance_type`. Layout:
///
/// ```text
/// +0x00 u64 length of the rest of the record      (== end_of_fields - (base+8))
/// +0x08 u32 INSTANCE CHECKSUM   <-- the QueueZ `instance_type` on the wire
/// +0x10 u32 a class tag (shared between objects; NOT this object's schema tag)
/// +0x14 u32 decoded object size                   (== anchor+0x14 - the discriminator)
/// +0x48 i64 self-relative pointer to +0x58
/// +0x50 the descriptor ANCHOR
/// ```
pub fn schema_records(r: &Reader, anchors: &[u64]) -> Vec<SchemaRecord> {
    let mut out = Vec::new();
    for &a in anchors {
        let Some(b) = a.checked_sub(0x50) else { continue };
        let (Some(cnt), Some(bsz), Some(asz), Some(blen)) =
            (r.u32(a + 0x08), r.u32(b + 0x14), r.u32(a + 0x14), r.u64(b))
        else {
            continue;
        };
        let end = a + 0x30 + cnt as u64 * 0x28;
        if bsz != asz || blen != end.wrapping_sub(b + 8) {
            continue;
        }
        out.push(SchemaRecord {
            record: b,
            anchor: a,
            descriptor: a + 8,
            checksum: r.u32(b + 0x08).unwrap_or(0),
            tag: r.u32(a + 0x10).unwrap_or(0),
            decoded_size: asz,
            fields: cnt,
            span_bits: r.u32(a + 0x1C).unwrap_or(0),
            record_len: blen,
        });
    }
    out
}

/// Layer 4: the investment message-definition table (CONTENT, not eboot - pkg 303 entry 890).
/// Refuses a file whose stride check fails, exactly as the prototype does.
pub fn investment_defs(d: &[u8]) -> std::result::Result<Vec<InvestmentDef>, String> {
    if d.len() < 0x1C {
        return Err(format!("stride check failed: 0x40 + ?*0x40 != {}", d.len()));
    }
    let count = u32::from_le_bytes(d[0x18..0x1C].try_into().unwrap()) as usize;
    if 0x40 + count * 0x40 != d.len() {
        return Err(format!("stride check failed: 0x40 + {count}*0x40 != {}", d.len()));
    }
    let u32at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    Ok((0..count)
        .map(|i| {
            let rb = 0x40 + i * 0x40;
            let extra = (0..0x40usize)
                .step_by(4)
                .filter(|&o| ![0x00, 0x10, 0x14, 0x18].contains(&o) && u32at(rb + o) != 0)
                .map(|o| o as u32)
                .collect();
            InvestmentDef {
                row: i,
                message_id: u32at(rb + 0x10),
                request_tag: u32at(rb + 0x14),
                response_tag: u32at(rb + 0x18),
                name_hash: u32at(rb),
                unexpected_nonzero_offsets: extra,
            }
        })
        .collect())
}

/// Layer 5: `pub const TYPE_*: u16 = 0x...;` constants out of `d1-bap/src/lib.rs`.
/// Aliases defined in terms of other constants carry no hex literal and are skipped, as in the
/// prototype's regex.
pub fn d1bap_constants(src: &str) -> Vec<(String, u16)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.trim_start().strip_prefix("pub const TYPE_") else { continue };
        let Some((name_tail, rest)) = rest.split_once(':') else { continue };
        if !name_tail.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix("u16") else { continue };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let Some(hex) = rest.strip_prefix("0x") else { continue };
        let Some(end) = hex.find(';') else { continue };
        let Ok(v) = u16::from_str_radix(&hex[..end], 16) else { continue };
        out.push((format!("TYPE_{name_tail}"), v));
    }
    out
}

impl Data {
    /// Build every layer. `defs` is the already-parsed definition table from [`investment_defs`]
    /// (`None` skips layer 4 and its controls); `d1bap_src` is `d1-bap/src/lib.rs` if present.
    pub fn build(image: &Image, defs: Option<Vec<InvestmentDef>>, d1bap_src: Option<&str>) -> Result<Self> {
        let r = Reader::new(image)?;
        let (idx, anchors) = descriptor_index(&r);
        Ok(Self {
            cat: envelope_classes(&r),
            shape: envelope_types_by_shape(&r),
            hdrs: header_classes(&r),
            pols: policy_families(&r),
            pushes: push_policies(&r),
            kinds: kind_to_reply(&r),
            lognames: log_names(&r),
            recs: schema_records(&r, &anchors),
            idx,
            anchors,
            defs,
            d1bap: d1bap_src.map(d1bap_constants).unwrap_or_default(),
        })
    }
}

// ---- Python formatting helpers -------------------------------------------------------------

/// Python `repr()` of an ASCII string: single quotes, switching to double quotes when the string
/// contains a single quote and no double quote.
fn py_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        if c == '\\' || c == q {
            out.push('\\');
        }
        out.push(c);
    }
    out.push(q);
    out
}

fn py_str_list<I: IntoIterator<Item = String>>(items: I) -> String {
    let inner: Vec<String> = items.into_iter().map(|s| py_repr(&s)).collect();
    format!("[{}]", inner.join(", "))
}

fn py_int_list<T: std::fmt::Display, I: IntoIterator<Item = T>>(items: I) -> String {
    let inner: Vec<String> = items.into_iter().map(|v| v.to_string()).collect();
    format!("[{}]", inner.join(", "))
}

/// Python `str()` of an optional value: the value, or the literal `None`.
fn py_opt<T: std::fmt::Display>(o: &Option<T>) -> String {
    match o {
        Some(v) => v.to_string(),
        None => "None".to_string(),
    }
}

// ---- reporting -----------------------------------------------------------------------------

struct ResponseMap<'a> {
    by_policy: HashMap<u32, Vec<&'a PolicyFamily>>,
    push_types: HashSet<u32>,
    reply_types: HashSet<u32>,
}

fn response_map<'a>(pols: &'a [PolicyFamily], pushes: &[PushPolicy], kinds: &[(u32, u32)]) -> ResponseMap<'a> {
    let mut by_policy: HashMap<u32, Vec<&PolicyFamily>> = HashMap::new();
    for p in pols {
        if let Some(rt) = p.request_type {
            by_policy.entry(rt).or_default().push(p);
        }
    }
    ResponseMap {
        by_policy,
        push_types: pushes.iter().filter_map(|p| p.ty).collect(),
        reply_types: kinds.iter().map(|&(_, v)| v).filter(|&v| v != 0).collect(),
    }
}

/// One row of a control run, for `d1-eboot controls` and the tests.
pub struct ControlRow {
    pub name: String,
    pub passed: bool,
}

/// The positive controls, rendered byte-for-byte as the Python prints them, plus structured rows.
///
/// A control proves the INSTRUMENT, never the aim and never the coverage, so this carries controls
/// of three different shapes: instrument (pinned RSAT schemas, header sizes, pinned policies and
/// definition rows), coverage (the envelope catalogue built TWICE by two scans with different
/// failure modes), and absence (a tag known to be absent must be absent WHILE a tag known to be
/// abundant is abundant, in the same scan).
pub fn render_controls(image: &Image, r: &Reader, d: &Data) -> (String, bool, Vec<ControlRow>) {
    let mut s = String::new();
    let mut rows: Vec<ControlRow> = Vec::new();
    let mut ok = true;
    let push = |s: &mut String, rows: &mut Vec<ControlRow>, ok: &mut bool, name: &str, good: bool, line: String| {
        s.push_str(&line);
        s.push('\n');
        rows.push(ControlRow { name: name.to_string(), passed: good });
        *ok &= good;
    };

    s.push_str(&format!("=== POSITIVE CONTROLS {}\n", "=".repeat(56)));

    // instrument: the four pinned RSAT schemas
    let mut by_ck: HashMap<u32, &SchemaRecord> = HashMap::new();
    for rec in &d.recs {
        by_ck.entry(rec.checksum).or_insert(rec);
    }
    for (name, ck, tag, nf, span, children) in PINNED_SCHEMAS.iter() {
        let rec = by_ck.get(ck).copied();
        let mut good = rec.is_some_and(|r| r.tag == *tag && r.fields == *nf && r.span_bits == *span);
        if good {
            if let Some(children) = children {
                let got: Vec<u32> =
                    desc_fields(image, rec.unwrap().anchor, rec.unwrap().fields).iter().map(|f| f.child_tag).collect();
                good = got == children.to_vec();
            }
        }
        let body = match rec {
            Some(r) => format!("tag={:#010x} fields={} span={}", r.tag, r.fields, r.span_bits),
            None => "NOT FOUND".to_string(),
        };
        let line = format!(
            "  schema {name:<10} ck={ck:#010x} -> {body}{}",
            if good { "   OK" } else { "   *** MISMATCH ***" }
        );
        push(&mut s, &mut rows, &mut ok, &format!("catalogue schema {name}"), good, line);
    }

    // cross-instrument: the 503 response schema, referenced from a LIVE registry walk
    let (tag503, nf, size, kinds_, children) = PINNED_503_RSP;
    let a503 = d.idx.get(&tag503).copied();
    let fl: Vec<descriptors::Field> = match a503 {
        Some(a) => desc_fields(image, a, r.u32(a + 0x08).unwrap_or(0)),
        None => Vec::new(),
    };
    let good503 = a503.is_some_and(|a| {
        r.u32(a + 0x08) == Some(nf)
            && r.u32(a + 0x14) == Some(size)
            && fl.iter().map(|f| f.kind).collect::<Vec<_>>() == kinds_.to_vec()
            && fl.iter().map(|f| f.child_tag).collect::<Vec<_>>() == children.to_vec()
    });
    let body = match a503 {
        Some(a) => format!(
            "fields={} size={} kinds={}",
            r.u32(a + 0x08).unwrap_or(0),
            r.u32(a + 0x14).unwrap_or(0),
            py_str_list(fl.iter().map(|f| format!("{:#x}", f.kind)))
        ),
        None => "NOT FOUND".to_string(),
    };
    push(&mut s, &mut rows, &mut ok, "catalogue 503 response schema", good503, format!(
        "  503 response {tag503:#010x}   : {body}{}",
        if good503 { "   OK" } else { "   *** MISMATCH ***" }
    ));

    // coverage: two independent enumerations of the envelope id space
    let a: BTreeSet<u32> = d.cat.keys().copied().collect();
    let b: BTreeSet<u32> = d.shape.keys().copied().collect();
    let sym: Vec<String> = {
        let mut v: Vec<String> = a.symmetric_difference(&b).map(|x| format!("{x:#x}")).collect();
        v.sort();
        v
    };
    let good_cov = a == b && a.len() > 50;
    push(&mut s, &mut rows, &mut ok, "catalogue envelope coverage (two scans agree)", good_cov, format!(
        "  envelope ids: RTTI scan {} \u{b7} shape scan {} \u{b7} symmetric difference {}{}",
        a.len(),
        b.len(),
        if sym.is_empty() { "none".to_string() } else { py_str_list(sym) },
        if good_cov { "   OK" } else { "   *** MISMATCH ***" }
    ));

    // instrument: header class sizes
    let sizes: Vec<Option<u32>> = d.hdrs.iter().map(|h| h.size).collect();
    let good_sizes = sizes.iter().map(|s| s.unwrap_or(u32::MAX)).collect::<Vec<_>>() == PINNED_HEADER_SIZES.to_vec();
    push(&mut s, &mut rows, &mut ok, "catalogue header class sizes", good_sizes, format!(
        "  header class sizes          : {}   (expect {}){}",
        py_int_list(sizes.iter().map(py_opt)),
        py_int_list(PINNED_HEADER_SIZES.iter()),
        if good_sizes { "   OK" } else { "   *** MISMATCH ***" }
    ));

    // instrument: one one-way policy and one paired policy
    for (fam, req, rsp) in PINNED_POLICIES.iter() {
        let p = &d.pols[*fam];
        let good = p.request_type == Some(*req) && p.response_type == Some(*rsp);
        push(&mut s, &mut rows, &mut ok, &format!("catalogue pinned policy family {fam}"), good, format!(
            "  policy family {fam:>2}            : req={:#x} rsp={:#x}   (expect {req:#x}/{rsp:#x}){}",
            p.request_type.unwrap_or(u32::MAX),
            p.response_type.unwrap_or(u32::MAX),
            if good { "   OK" } else { "   *** MISMATCH ***" }
        ));
    }

    // instrument: two pinned investment definition rows (skipped when the defs file is absent)
    if let Some(defs) = &d.defs {
        for (row, mid, req, rsp) in PINNED_DEFS.iter() {
            let rr = defs.get(*row);
            let good = rr.is_some_and(|rr| {
                (rr.message_id, rr.request_tag, rr.response_tag) == (*mid, *req, *rsp)
            });
            let rr = rr.expect("pinned definition row out of range");
            push(&mut s, &mut rows, &mut ok, &format!("catalogue pinned definition row {row}"), good, format!(
                "  definition row {row:>3}          : id={} req={:#x} rsp={:#x}{}",
                rr.message_id,
                rr.request_tag,
                rr.response_tag,
                if good { "   OK" } else { "   *** MISMATCH ***" }
            ));
        }
    }

    // absence, always paired with a known-present control in the same scan
    let na = r.count_u32(ABSENT_TAG);
    let np = r.count_u32(PRESENT_TAG);
    s.push_str(&format!("  raw u32 {ABSENT_TAG:#010x} occurrences : {na:>6}   (expect 0)\n"));
    let good_abs = na == 0 && np > 1000;
    push(&mut s, &mut rows, &mut ok, "catalogue absence control (absent tag 0, present tag abundant)", good_abs, format!(
        "  raw u32 {PRESENT_TAG:#010x} occurrences : {np:>6}   <- CONTROL, must be large{}",
        if good_abs { "   OK" } else { "   *** MISMATCH ***" }
    ));

    if let Some(defs) = &d.defs {
        let tags: BTreeSet<u32> =
            defs.iter().flat_map(|r| [r.request_tag, r.response_tag]).collect();
        let miss: Vec<u32> = tags.iter().copied().filter(|t| !d.idx.contains_key(t)).collect();
        let good_tags = miss.is_empty();
        push(&mut s, &mut rows, &mut ok, "catalogue investment schema tags all resolve", good_tags, format!(
            "  investment schema tags      : {} distinct, {} unresolved {}{}",
            tags.len(),
            miss.len(),
            py_str_list(miss.iter().take(5).map(|t| format!("{t:#x}"))),
            if good_tags { "   OK" } else { "   *** MISMATCH ***" }
        ));
    }

    // structural: the record-base rule
    let bad = d
        .recs
        .iter()
        .filter(|rr| rr.record_len != (rr.anchor + 0x30 + rr.fields as u64 * 0x28) - (rr.record + 8))
        .count();
    let good_rec = bad == 0;
    push(&mut s, &mut rows, &mut ok, "catalogue schema record length rule", good_rec, format!(
        "  schema records found        : {}, length-rule violations {}{}",
        d.recs.len(),
        bad,
        if good_rec { "   OK" } else { "   *** MISMATCH ***" }
    ));

    s.push_str(&format!(
        "  ==> controls {}\n\n",
        if ok { "PASS" } else { "FAIL - do not trust anything printed below" }
    ));
    (s, ok, rows)
}

/// The envelope catalogue: type -> verdict about whether the CLIENT is waiting for a reply.
pub fn render_envelopes(d: &Data) -> String {
    let m = response_map(&d.pols, &d.pushes, &d.kinds);
    let responses: HashSet<u32> = d.pols.iter().filter_map(|p| p.response_type).filter(|&r| r != 0).collect();
    let handshake: [(u32, u32); 5] =
        [(0x19, 0x1A), (0x1E, 0x1F), (0xFA, 0xFB), (0x79, 0x7A), (0x12E, 0x12F)];
    let mut s = String::new();
    s.push_str("=== BAP ENVELOPE MESSAGE CATALOGUE - 63 classes, read from RTTI + vtable constants ===\n");
    s.push_str("  type   hdr sz  role            response   class / notes\n");
    for (t, c) in &d.cat {
        let hdr = c.header_class;
        let size = hdr.filter(|&h| (h as usize) < 8).map(|h| PINNED_HEADER_SIZES[h as usize]);
        let pl = m.by_policy.get(t).map(|v| v.as_slice()).unwrap_or(&[]);
        let (role, resp): (&str, String) = if !pl.is_empty() {
            let rsp: BTreeSet<u32> = pl.iter().map(|p| p.response_type.unwrap_or(0)).collect();
            if rsp.iter().all(|&r| r == 0) {
                ("policy-request", "NONE - do not answer".to_string())
            } else {
                ("policy-request",
                 rsp.iter().map(|r| format!("{r:#06x}")).collect::<Vec<_>>().join(" / "))
            }
        } else if m.push_types.contains(t) {
            ("server-push", "NONE (type-only dispatch)".to_string())
        } else if m.reply_types.contains(t) {
            ("handshake-reply", "(is itself a reply)".to_string())
        } else if responses.contains(t) {
            ("response", "(is itself a reply)".to_string())
        } else if let Some(&(_, reply)) = handshake.iter().find(|&&(q, _)| q == *t) {
            let k = d.kinds.iter().find(|&&(_, v)| d.cat.contains_key(&v) && v == reply).map(|&(k, _)| k);
            ("handshake-request", format!("{reply:#06x} (kind {})", py_opt(&k)))
        } else {
            ("unbound", "UNKNOWN - no policy, no push policy".to_string())
        };
        let note = c.class.replace("c_investment_bap_message_", "\u{2026}").replace("c_bap_message_", "\u{2026}");
        let extra = match d.lognames.get(t) {
            Some(Some(ln)) if !ln.is_empty() => format!("   [log name {}]", py_repr(ln)),
            _ => "   [no log name]".to_string(),
        };
        s.push_str(&format!(
            "  {t:#06x}  {:>3} {:>3}  {role:<15} {resp:<24} {note}{extra}\n",
            py_opt(&hdr),
            py_opt(&size),
        ));
    }
    let missing: Vec<String> = d
        .lognames
        .iter()
        .filter(|(t, _)| !d.cat.contains_key(t))
        .map(|(t, n)| format!("{t:#04x} {}", match n { Some(n) => py_repr(n), None => "None".to_string() }))
        .collect();
    s.push_str(&format!(
        "\n  \u{26a0} named in the log table but WITHOUT a message class (unreachable in this build): {}\n\n",
        py_str_list(missing)
    ));
    s
}

pub fn render_policies(d: &Data) -> String {
    let mut s = String::new();
    s.push_str("=== POLICY FAMILIES - index space of 0x55F400/0x55F410; slot +0x30 is the answer ===\n");
    s.push_str("  fam  request  response  display name (0x1923AA0)                   RTTI class\n");
    for p in &d.pols {
        let mark = if p.response_type == Some(0) { "  <-- ONE-WAY, do not answer" } else { "" };
        let cls = p.class.clone().unwrap_or_default().replace("c_investment_bap_message_policy_", "\u{2026}");
        let dn: String = py_opt(&p.display_name).chars().take(42).collect();
        s.push_str(&format!(
            "  {:>3}  {:#07x}  {:#08x}  {dn:<42} {cls}{mark}\n",
            p.family,
            p.request_type.unwrap_or(u32::MAX),
            p.response_type.unwrap_or(u32::MAX),
        ));
    }
    let disagree: Vec<usize> = d
        .pols
        .iter()
        .filter(|p| {
            matches!((&p.class, &p.display_name), (Some(c), Some(n)) if !c.is_empty() && !n.is_empty() && !c.contains(n.trim_end_matches(',')))
        })
        .map(|p| p.family)
        .collect();
    s.push_str(&format!(
        "\n  display name vs RTTI class: {}/{} agree; differing families {}\n",
        POLICY_COUNT - disagree.len(),
        POLICY_COUNT,
        py_int_list(disagree.iter())
    ));
    s.push_str("\n=== PUSH POLICIES - server-initiated, dispatched by TYPE ALONE (no sequence) ===\n");
    for p in &d.pushes {
        s.push_str(&format!(
            "  {:#06x}  {}\n",
            p.ty.unwrap_or(u32::MAX),
            p.class.replace("c_investment_bap_message_push_policy_", "\u{2026}")
        ));
    }
    s.push_str("\n=== protocol_source request kind -> reply type (0x15D1B40) ===\n");
    for (k, v) in &d.kinds {
        s.push_str(&format!("  kind {k} -> {v:#06x}\n"));
    }
    s.push('\n');
    s
}

pub fn render_schemas(d: &Data) -> String {
    let mut s = String::new();
    s.push_str(&format!("=== RSAT SCHEMA RECORDS - {} records with an instance checksum ===\n", d.recs.len()));
    s.push_str("  RECORD BASE = anchor - 0x50.  `0x56B6F8` reads `object+0x08` for the instance checksum,\n");
    s.push_str("  so the base is 8 BELOW the checksum, and the u64 at +0x00 is the record's own length.\n");
    s.push_str("  Only the pinned four are printed; use --json for all.\n\n");
    let want: HashSet<u32> = PINNED_SCHEMAS.iter().map(|p| p.1).collect();
    for r in &d.recs {
        if want.contains(&r.checksum) {
            s.push_str(&format!(
                "  record {:#09x} checksum {:#010x} tag {:#010x} fields {:>2} span {:>5} size {:#07x} len {:#06x}\n",
                r.record, r.checksum, r.tag, r.fields, r.span_bits, r.decoded_size, r.record_len
            ));
        }
    }
    s.push('\n');
    s
}

pub fn render_investment(r: &Reader, d: &Data) -> String {
    let defs = d.defs.as_deref().unwrap_or(&[]);
    let mut s = String::new();
    s.push_str("=== INVESTMENT SUB-ID SPACE - 101 ids inside envelope 0x000A (CONTENT, not eboot) ===\n");
    s.push_str("  \u{26a0} `span` is the raw word at anchor+0x1C.  It is CONFIRMED as the RSAT dirty-field span\n");
    s.push_str("    for the four pinned QueueZ objects, and it is NOT a serialized body length for\n");
    s.push_str("    message schemas: many response schemas read 0 while their child subtree plainly\n");
    s.push_str("    carries bits (e.g. 0x80801845 -> child 0x80801796 = 4+32 bits).  `[U]` - treat the\n");
    s.push_str("    field count and the child tree as the catalogue; treat span as a hint only.\n");
    s.push_str("  row   id    request tag   f  span   size   response tag  f  span   size\n");
    let col = |x: &Option<DescSummary>| match x {
        Some(x) => format!("{:#010x} {:>2} {:>5} {:>6}", x.tag, x.fields, x.span_bits, x.decoded_size),
        None => format!("{:>10}", "UNRESOLVED"),
    };
    for row in defs {
        let a = desc_summary(r, &d.idx, row.request_tag);
        let b = desc_summary(r, &d.idx, row.response_tag);
        s.push_str(&format!("  {:>3} {:>5}   {}   {}\n", row.row, row.message_id, col(&a), col(&b)));
    }
    let weird = defs.iter().filter(|r| !r.unexpected_nonzero_offsets.is_empty()).count();
    s.push_str(&format!(
        "\n  records with any field beyond {{name_hash, id, request_tag, response_tag}}: {weird}\n"
    ));
    s.push_str("  \u{21d2} the definition record carries NO direction flag, NO 'response optional' bit and NO\n");
    s.push_str("    handler binding.  Every one of the 101 ids has BOTH tags non-zero, so 'does this\n");
    s.push_str("    investment message need a reply' is NOT answerable from this table.  The envelope\n");
    s.push_str("    layer answers it instead: policy families 0 and 1 both map 0x000A -> 0x000B.\n\n");
    s
}

/// Cross-check our own `d1-bap` TYPE_* constants against the binary. A disagreement here is a bug
/// in OUR code.
pub fn render_d1bap(d: &Data) -> String {
    let mut s = String::new();
    if d.d1bap.is_empty() {
        s.push_str("=== d1-bap CROSS-CHECK: lib.rs not found, skipped ===\n\n");
        return s;
    }
    let m = response_map(&d.pols, &d.pushes, &[]);
    let responses: HashSet<u32> = d.pols.iter().filter_map(|p| p.response_type).filter(|&r| r != 0).collect();
    s.push_str("=== d1-bap CROSS-CHECK - a disagreement here is a bug in OUR code ===\n");
    let mut consts = d.d1bap.clone();
    consts.sort_by_key(|&(_, v)| v);
    for (name, v) in consts {
        let v = v as u32;
        let Some(c) = d.cat.get(&v) else {
            s.push_str(&format!("  {name:<38} {v:#06x}  *** NO MESSAGE CLASS IN THE BINARY ***\n"));
            continue;
        };
        let cls = c.class.replace("c_investment_bap_message_", "");
        let pl = m.by_policy.get(&v).map(|x| x.as_slice()).unwrap_or(&[]);
        let rsp = if !pl.is_empty() && pl.iter().all(|p| p.response_type == Some(0)) {
            "one-way".to_string()
        } else if m.push_types.contains(&v) {
            "push".to_string()
        } else if responses.contains(&v) {
            "response".to_string()
        } else if !pl.is_empty() {
            pl.iter().map(|p| format!("{:#x}", p.response_type.unwrap_or(0))).collect::<Vec<_>>().join(" / ")
        } else {
            "handshake".to_string()
        };
        let ours = name.replace("TYPE_", "").to_lowercase();
        let agree = ours.split('_').filter(|w| w.len() > 3).all(|w| cls.contains(w));
        s.push_str(&format!(
            "  {name:<38} {v:#06x}  {rsp:<9} {cls}{}\n",
            if agree { "" } else { "   <-- NAME DIFFERS" }
        ));
    }
    s.push('\n');
    s
}

/// The machine-readable everything-blob, shaped exactly as the Python's `--json` file (compare
/// with `jq -S`; the Python emits `indent=1` and string keys for int-keyed dicts).
pub fn json_blob(image: &Image, r: &Reader, d: &Data) -> serde_json::Value {
    let eboot = image
        .path()
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let inv: Vec<serde_json::Value> = d
        .defs
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|row| {
            let mut v = serde_json::to_value(row).unwrap();
            v["request_schema"] = serde_json::to_value(desc_summary(r, &d.idx, row.request_tag)).unwrap();
            v["response_schema"] = serde_json::to_value(desc_summary(r, &d.idx, row.response_tag)).unwrap();
            v
        })
        .collect();
    serde_json::json!({
        "eboot": eboot,
        "envelope_classes": d.cat.values().collect::<Vec<_>>(),
        "header_classes": d.hdrs,
        "policy_families": d.pols,
        "push_policies": d.pushes,
        "kind_to_reply": d.kinds.iter().map(|&(k, v)| (k.to_string(), v)).collect::<BTreeMap<_, _>>(),
        "log_names": d.lognames.iter().map(|(k, v)| (format!("{k:#x}"), v.clone())).collect::<BTreeMap<_, _>>(),
        "schema_records": d.recs,
        "investment_messages": inv,
    })
}

/// The full human-readable run: controls, then every section, in the prototype's order.
/// Returns the text and whether the controls passed.
pub fn render_full(image: &Image, r: &Reader, d: &Data) -> (String, bool) {
    let (mut s, ok, _) = render_controls(image, r, d);
    if ok {
        s.push_str(&render_envelopes(d));
        s.push_str(&render_policies(d));
        s.push_str(&render_schemas(d));
        s.push_str(&render_investment(r, d));
        s.push_str(&render_d1bap(d));
    }
    (s, ok)
}
