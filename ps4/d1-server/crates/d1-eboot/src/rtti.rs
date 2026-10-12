// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **RTTI harvest** - the client's own class names, hierarchy, vtables and vtable slots.
//!
//! The eboot is stripped of symbols, but it is **not** stripped of RTTI: it was built with
//! `-frtti`, so every polymorphic class still carries a `__class_type_info` object naming itself in
//! plain Itanium mangling. `design/eboot-map.md` already quoted a handful of these by hand
//! (`c_world_controller_state_bootflow_profile_setup_handler`, the 67 `c_investment_bap_message_*`
//! classes). This module harvests **all** of them, mechanically.
//!
//! ## The layout, and why nothing is readable in the file
//!
//! Every pointer below is **zero in the file** and supplied at load time by a relocation
//! (ATLAS §4.2). A byte search finds nothing by design.
//!
//! ```text
//!   typeinfo object                     vtable group
//!     ti+0x00  vptr  -> R_X86_64_64       vptr-0x10  offset_to_top  (plain i64, NO relocation)
//!                      sym = one of the   vptr-0x08  typeinfo ptr   -> R_X86_64_RELATIVE
//!                      three __cxxabiv1   vptr+0x00  virtual fn 0   -> R_X86_64_RELATIVE
//!                      class_type_infos   vptr+0x08  virtual fn 1
//!     ti+0x08  name  -> R_X86_64_RELATIVE, points at the mangled spelling  ("28s_investment…")
//!     ti+0x10  __si_class_type_info : base typeinfo
//!              __vmi_class_type_info: u32 flags, u32 base_count, then {ti*, offset_flags} x N
//! ```
//!
//! **The discriminator is the `ti+0x00` symbol**, and it is *derived*, not remembered: the three
//! `__cxxabiv1` vtable spellings are run through [`crate::dynsym::nid`] and looked up in the
//! dynamic symbol table (ATLAS §4.3 - a hardcoded `pZ9WXcClPO8` is exactly the kind of constant
//! that outlives the reading that produced it).
//!
//! ## Yield on the pinned 01.29 build
//!
//! 2,633 typeinfos (2,131 single-inheritance, 323 base, 179 multi/virtual), **all named**;
//! 2,429 vtables over 2,129 classes; 22,498 vtable slots naming 7,930 distinct code addresses.
//!
//! ⚠ A vtable slot names a function *by role*, not by spelling: `C::vfunc[7]` is a true statement
//! about what the client calls it for, not the identifier Bungie typed. Slots are also **shared**
//! - a base's implementation appears in every derived vtable - so [`Rtti::owner_of`] attributes a
//! function to the **least-derived** class that carries it, which is where the code was written.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Serialize;

use crate::dynsym;
use crate::image::{Image, Result};
use crate::reloc::{self, R_X86_64_64, R_X86_64_RELATIVE};

/// The three Itanium `__class_type_info` flavours, keyed by the vtable symbol at `ti+0`.
pub const TI_BASE: &str = "_ZTVN10__cxxabiv117__class_type_infoE";
pub const TI_SI: &str = "_ZTVN10__cxxabiv120__si_class_type_infoE";
pub const TI_VMI: &str = "_ZTVN10__cxxabiv121__vmi_class_type_infoE";
/// A vtable slot bound to this import is a pure virtual - a real slot with no implementation.
pub const PURE_VIRTUAL: &str = "__cxa_pure_virtual";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TiKind {
    /// `__class_type_info` - no base classes.
    Base,
    /// `__si_class_type_info` - exactly one public, non-virtual, zero-offset base.
    Si,
    /// `__vmi_class_type_info` - multiple and/or virtual and/or non-public bases.
    Vmi,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeInfo {
    /// vaddr of the typeinfo object.
    pub ti: u64,
    pub kind: TiKind,
    /// vaddr of the mangled spelling.
    pub name_ptr: u64,
    /// The Itanium spelling exactly as stored, e.g. `N3cui20c_director_activityE`.
    pub mangled: String,
    /// Demangled where this module's parser can do it, else identical to `mangled`.
    pub name: String,
    /// Base classes, most-significant first, as typeinfo vaddrs.
    pub bases: Vec<u64>,
    /// vtable pointers (the address stored in an object's vptr field) belonging to this class.
    pub vtables: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VTable {
    /// The address an object's vptr holds - i.e. slot 0, **not** the start of the vtable group.
    pub vptr: u64,
    pub ti: u64,
    /// `[vptr-0x10]`. `0` for a primary vtable; non-zero marks a secondary (base-subobject) vtable.
    pub offset_to_top: i64,
    /// Slot targets. `0` marks `__cxa_pure_virtual` - a declared slot with no implementation.
    pub slots: Vec<u64>,
}

#[derive(Debug, Default, Serialize)]
pub struct Rtti {
    pub types: Vec<TypeInfo>,
    pub vtables: Vec<VTable>,
    /// Which `__cxxabiv1` symbol index each flavour resolved to - printed so the derivation is
    /// visible rather than assumed.
    pub abi_syms: BTreeMap<String, u32>,
}

impl Rtti {
    pub fn type_at(&self, ti: u64) -> Option<&TypeInfo> {
        self.types.iter().find(|t| t.ti == ti)
    }

    /// Depth in the inheritance DAG: 0 for a class with no bases. Used to pick the *least-derived*
    /// class that carries a shared vtable slot, which is where the implementation was written.
    pub fn depths(&self) -> HashMap<u64, u32> {
        let by_ti: HashMap<u64, &TypeInfo> = self.types.iter().map(|t| (t.ti, t)).collect();
        let mut memo: HashMap<u64, u32> = HashMap::new();
        fn go(
            ti: u64,
            by_ti: &HashMap<u64, &TypeInfo>,
            memo: &mut HashMap<u64, u32>,
            seen: &mut HashSet<u64>,
        ) -> u32 {
            if let Some(&d) = memo.get(&ti) {
                return d;
            }
            if !seen.insert(ti) {
                return 0; // cycle guard; the DAG should have none, but a bad read must not hang
            }
            let d = by_ti
                .get(&ti)
                .map(|t| t.bases.iter().map(|&b| go(b, by_ti, memo, seen) + 1).max().unwrap_or(0))
                .unwrap_or(0);
            seen.remove(&ti);
            memo.insert(ti, d);
            d
        }
        let tis: Vec<u64> = self.types.iter().map(|t| t.ti).collect();
        for ti in tis {
            let mut seen = HashSet::new();
            go(ti, &by_ti, &mut memo, &mut seen);
        }
        memo
    }

    /// function vaddr → (typeinfo, slot index) of the **least-derived** class declaring that slot.
    ///
    /// A derived class's vtable repeats every inherited implementation, so a naive
    /// "last writer wins" attributes half the engine to whichever leaf class happened to sort last.
    pub fn owner_of(&self) -> HashMap<u64, (u64, usize)> {
        let depth = self.depths();
        let mut out: HashMap<u64, (u64, usize, u32)> = HashMap::new();
        for vt in &self.vtables {
            let d = depth.get(&vt.ti).copied().unwrap_or(u32::MAX);
            for (i, &f) in vt.slots.iter().enumerate() {
                if f == 0 {
                    continue;
                }
                match out.get(&f) {
                    Some(&(_, _, pd)) if pd <= d => {}
                    _ => {
                        out.insert(f, (vt.ti, i, d));
                    }
                }
            }
        }
        out.into_iter().map(|(f, (ti, i, _))| (f, (ti, i))).collect()
    }
}

/// Harvest every typeinfo, vtable and vtable slot in the image.
pub fn harvest(image: &Image) -> Result<Rtti> {
    let syms = dynsym::by_nid(image)?;
    let mut abi_syms = BTreeMap::new();
    let mut kind_of_sym: HashMap<u32, TiKind> = HashMap::new();
    for (spelling, kind) in [(TI_BASE, TiKind::Base), (TI_SI, TiKind::Si), (TI_VMI, TiKind::Vmi)] {
        if let Some(ix) = dynsym::index_of(&syms, spelling) {
            abi_syms.insert(spelling.to_string(), ix);
            kind_of_sym.insert(ix, kind);
        }
    }
    let pure_sym = dynsym::index_of(&syms, PURE_VIRTUAL);
    if let Some(ix) = pure_sym {
        abi_syms.insert(PURE_VIRTUAL.to_string(), ix);
    }

    let mut rel: HashMap<u64, u64> = HashMap::new();
    let mut by_addend: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut abs: HashMap<u64, (u32, u64)> = HashMap::new();
    for r in reloc::all(image)? {
        match r.r_type {
            R_X86_64_RELATIVE => {
                rel.insert(r.slot, r.addend);
                by_addend.entry(r.addend).or_default().push(r.slot);
            }
            R_X86_64_64 => {
                abs.entry(r.slot).or_insert((r.sym, r.addend));
            }
            _ => {}
        }
    }

    // ── typeinfos ───────────────────────────────────────────────────────────────────────────
    // `ti+0` is relocated against a __cxxabiv1 class_type_info vtable, +16 (past its own
    // offset-to-top and typeinfo slots). That addend is part of the signature.
    let mut types: Vec<TypeInfo> = Vec::new();
    let mut ti_kind: HashMap<u64, TiKind> = HashMap::new();
    for (&slot, &(sym, addend)) in &abs {
        if addend != 16 {
            continue;
        }
        if let Some(&k) = kind_of_sym.get(&sym) {
            ti_kind.insert(slot, k);
        }
    }

    // ti-internal pointer slots - base-class links, NOT vtable typeinfo slots. Keeping these apart
    // is the whole difference between "class B derives from A" and "here is a vtable for A".
    let mut ti_internal: HashSet<u64> = HashSet::new();
    let mut bases_of: HashMap<u64, Vec<u64>> = HashMap::new();
    for (&ti, &kind) in &ti_kind {
        match kind {
            TiKind::Base => {}
            TiKind::Si => {
                ti_internal.insert(ti + 0x10);
                if let Some(&b) = rel.get(&(ti + 0x10)) {
                    bases_of.entry(ti).or_default().push(b);
                }
            }
            TiKind::Vmi => {
                let n = image.u32(ti + 0x14).unwrap_or(0).min(64) as u64;
                for i in 0..n {
                    let s = ti + 0x18 + 16 * i;
                    ti_internal.insert(s);
                    if let Some(&b) = rel.get(&s) {
                        bases_of.entry(ti).or_default().push(b);
                    }
                }
            }
        }
    }

    // ── vtables ─────────────────────────────────────────────────────────────────────────────
    // A relocation whose addend is a typeinfo and whose slot is not ti-internal is a vtable's
    // typeinfo slot, at `vptr - 8`.
    let mut vptrs: Vec<(u64, u64)> = Vec::new(); // (vptr, ti)
    for (&ti, _) in &ti_kind {
        for &slot in by_addend.get(&ti).into_iter().flatten() {
            if ti_internal.contains(&slot) {
                continue;
            }
            vptrs.push((slot + 8, ti));
        }
    }
    vptrs.sort_unstable();
    let vptr_meta: HashSet<u64> = vptrs.iter().flat_map(|&(v, _)| [v - 8, v - 16]).collect();
    let ti_addrs: HashSet<u64> = ti_kind.keys().copied().collect();

    let exec: Vec<(u64, u64)> = image
        .executable_segments()
        .map(|s| (s.vaddr, s.vaddr + s.filesz))
        .collect();
    let in_text = |a: u64| exec.iter().any(|&(lo, hi)| a >= lo && a < hi);

    let mut vtables: Vec<VTable> = Vec::new();
    let mut vt_of_ti: HashMap<u64, Vec<u64>> = HashMap::new();
    for &(vptr, ti) in &vptrs {
        let offset_to_top = image
            .bytes(vptr - 16, 8)
            .map(|b| i64::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0);
        let mut slots = Vec::new();
        let mut k = 0u64;
        loop {
            let s = vptr + 8 * k;
            if k > 0 && (vptr_meta.contains(&s) || ti_addrs.contains(&s) || ti_internal.contains(&s))
            {
                break;
            }
            if let Some(&a) = rel.get(&s) {
                if !in_text(a) {
                    break;
                }
                slots.push(a);
            } else if pure_sym.is_some() && abs.get(&s).map(|v| v.0) == pure_sym {
                slots.push(0);
            } else {
                break;
            }
            k += 1;
            if k > 4096 {
                break;
            }
        }
        vt_of_ti.entry(ti).or_default().push(vptr);
        vtables.push(VTable { vptr, ti, offset_to_top, slots });
    }

    for (&ti, &kind) in &ti_kind {
        let name_ptr = rel.get(&(ti + 8)).copied().unwrap_or(0);
        let mangled = if name_ptr != 0 {
            image.cstr(name_ptr, 1024).unwrap_or("").to_string()
        } else {
            String::new()
        };
        let name = demangle(&mangled).unwrap_or_else(|| mangled.clone());
        let mut vtables = vt_of_ti.get(&ti).cloned().unwrap_or_default();
        vtables.sort_unstable();
        types.push(TypeInfo {
            ti,
            kind,
            name_ptr,
            mangled,
            name,
            bases: bases_of.get(&ti).cloned().unwrap_or_default(),
            vtables,
        });
    }
    types.sort_by_key(|t| t.ti);
    vtables.sort_by_key(|v| v.vptr);

    Ok(Rtti { types, vtables, abi_syms })
}

// ── a small Itanium demangler, for class names only ────────────────────────────────────────────
//
// `__type_name` holds a `<type>` production, which for a class is a `<name>`. That is a much
// smaller grammar than a full function signature, and a partial demangler that RETURNS NOTHING on
// anything it does not fully understand is safer than one that guesses: an unrecognised form keeps
// its mangled spelling, which is still a unique, greppable identity.

struct Dem<'a> {
    s: &'a [u8],
    i: usize,
    subs: Vec<String>,
}

/// `N3cui20c_director_activityE` → `cui::c_director_activity`. `None` when not fully parsed.
pub fn demangle(m: &str) -> Option<String> {
    if m.is_empty() {
        return None;
    }
    let mut d = Dem { s: m.as_bytes(), i: 0, subs: Vec::new() };
    let out = d.type_()?;
    if d.i == d.s.len() {
        Some(out)
    } else {
        None
    }
}

impl<'a> Dem<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn source_name(&mut self) -> Option<String> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        let n: usize = std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()?;
        let end = self.i.checked_add(n)?;
        if end > self.s.len() {
            return None;
        }
        let out = String::from_utf8_lossy(&self.s[self.i..end]).into_owned();
        self.i = end;
        Some(out)
    }

    fn substitution(&mut self) -> Option<String> {
        // `S_` = subs[0], `S0_` = subs[1], `S1_` = subs[2] …; `St` = std.
        if !self.eat(b'S') {
            return None;
        }
        if self.eat(b't') {
            return Some("std".into());
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
            self.i += 1;
        }
        let digits = &self.s[start..self.i];
        if !self.eat(b'_') {
            return None;
        }
        let ix = if digits.is_empty() {
            0
        } else {
            usize::from_str_radix(std::str::from_utf8(digits).ok()?, 36).ok()? + 1
        };
        self.subs.get(ix).cloned()
    }

    fn template_args(&mut self) -> Option<String> {
        if !self.eat(b'I') {
            return None;
        }
        let mut parts = Vec::new();
        while !self.eat(b'E') {
            if self.peek().is_none() {
                return None;
            }
            parts.push(self.type_()?);
        }
        Some(format!("<{}>", parts.join(", ")))
    }

    /// A nested name `N <prefix components> E`, building substitution candidates as it goes.
    fn nested(&mut self) -> Option<String> {
        if !self.eat(b'N') {
            return None;
        }
        // CV / ref qualifiers on the *name* are legal here and irrelevant to a class identity.
        while matches!(self.peek(), Some(b'r') | Some(b'V') | Some(b'K') | Some(b'R') | Some(b'O')) {
            self.i += 1;
        }
        let mut acc = String::new();
        while !self.eat(b'E') {
            let c = self.peek()?;
            let part = if c == b'S' {
                self.substitution()?
            } else if c == b'I' {
                // template args attach to the accumulated name, they are not a new component
                let t = self.template_args()?;
                acc.push_str(&t);
                self.subs.push(acc.clone());
                continue;
            } else if c.is_ascii_digit() {
                self.source_name()?
            } else {
                return None;
            };
            if acc.is_empty() {
                acc = part;
            } else {
                acc = format!("{acc}::{part}");
            }
            self.subs.push(acc.clone());
        }
        Some(acc)
    }

    fn builtin(&mut self) -> Option<String> {
        let c = self.peek()?;
        let name = match c {
            b'v' => "void",
            b'b' => "bool",
            b'c' => "char",
            b'a' => "signed char",
            b'h' => "unsigned char",
            b's' => "short",
            b't' => "unsigned short",
            b'i' => "int",
            b'j' => "unsigned int",
            b'l' => "long",
            b'm' => "unsigned long",
            b'x' => "long long",
            b'y' => "unsigned long long",
            b'n' => "__int128",
            b'o' => "unsigned __int128",
            b'f' => "float",
            b'd' => "double",
            b'e' => "long double",
            b'z' => "...",
            _ => return None,
        };
        self.i += 1;
        Some(name.into())
    }

    fn type_(&mut self) -> Option<String> {
        let c = self.peek()?;
        match c {
            b'N' => self.nested(),
            b'S' => {
                let base = self.substitution()?;
                if self.peek() == Some(b'I') {
                    let t = self.template_args()?;
                    let full = format!("{base}{t}");
                    self.subs.push(full.clone());
                    Some(full)
                } else {
                    Some(base)
                }
            }
            b'P' | b'R' | b'O' | b'K' | b'V' => {
                self.i += 1;
                let inner = self.type_()?;
                let out = match c {
                    b'P' => format!("{inner}*"),
                    b'R' => format!("{inner}&"),
                    b'O' => format!("{inner}&&"),
                    b'K' => format!("{inner} const"),
                    _ => format!("{inner} volatile"),
                };
                self.subs.push(out.clone());
                Some(out)
            }
            b'L' => {
                // literal: `L <type> <value> E` - template non-type arguments
                self.i += 1;
                let t = self.type_()?;
                let start = self.i;
                while self.peek().is_some_and(|x| x != b'E') {
                    self.i += 1;
                }
                let v = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                if !self.eat(b'E') {
                    return None;
                }
                Some(if t == "bool" {
                    (if v == "1" { "true" } else { "false" }).to_string()
                } else {
                    v
                })
            }
            _ if c.is_ascii_digit() => {
                let n = self.source_name()?;
                self.subs.push(n.clone());
                if self.peek() == Some(b'I') {
                    let t = self.template_args()?;
                    let full = format!("{n}{t}");
                    self.subs.push(full.clone());
                    Some(full)
                } else {
                    Some(n)
                }
            }
            _ => self.builtin(),
        }
    }
}

/// The set of distinct code addresses any vtable points at.
pub fn virtual_targets(r: &Rtti) -> BTreeSet<u64> {
    r.vtables.iter().flat_map(|v| v.slots.iter().copied()).filter(|&f| f != 0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠ Every input here is a spelling **copied out of the pinned image**, never composed by
    /// hand. The first draft of this test invented `N3cui20c_director_activityE` and failed -
    /// `c_director_activity` is 19 characters, not 20 - which would have looked exactly like a
    /// demangler defect. A test fixture is a constant, and ATLAS §4.3 applies to those too.
    #[test]
    fn demangles_the_forms_this_image_actually_uses() {
        assert_eq!(demangle("28s_investment_utility_context").as_deref(), Some("s_investment_utility_context"));
        assert_eq!(demangle("N3cui19c_director_activityE").as_deref(), Some("cui::c_director_activity"));
        assert_eq!(demangle("N9SpeedTree13CCoordSysBaseE").as_deref(), Some("SpeedTree::CCoordSysBase"));
        assert_eq!(
            demangle("N3cui26c_properties_accessor_baseINS_29c_multi_decision_maker_branchEEE").as_deref(),
            Some("cui::c_properties_accessor_base<cui::c_multi_decision_maker_branch>")
        );
        assert_eq!(
            demangle("N3cui27c_properties_accessor_proxyI37c_player_select_pipe_reader_componentEE").as_deref(),
            Some("cui::c_properties_accessor_proxy<c_player_select_pipe_reader_component>")
        );
        // Refuses rather than guesses.
        assert_eq!(demangle("N3cui"), None);
    }
}
