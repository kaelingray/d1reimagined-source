// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Named capture presets - **one shot instead of N rounds**.
//!
//! # Why this exists
//!
//! `capture --region EXPR:LEN` is explicit, and explicit is right: what you did not capture is part
//! of the evidence. But an *offline* replay of a content lookup turns that virtue into a grind.
//! 2026-08-09, replaying `FUN_0010C500` (`v:0x10c500`) against an in-world snapshot: each attempt
//! faulted `READ_UNMAPPED` on a page nobody had thought to name, we added a `--region`, re-captured,
//! and bought about four more instructions. Three rounds, three fault addresses - 
//! `0x2044126d0`, `0x38031ca38`, `0x22db821de` - and no reason to believe the fourth would be the
//! last. **A capture is a live-client-exclusive resource; a replay is not.** So the fix belongs at
//! capture time: derive the whole working set from the client's own tables, once.
//!
//! # What a content lookup actually reads - read out of `v:0x10c500` itself
//!
//! ```text
//!   call 0x1072c0                    ; the resolver: reads the ARENA TABLE, then a content DB blob
//!   esi = tag >> 13
//!   edi = ((esi >>u 18) | 0x3ff) & esi          ; the slot index
//!   rcx = **[0x1a0b420]                          ; THE PACKAGE REGISTRY slot table (double deref)
//!   rsi = [rcx + slot*0x40 + 0x10]               ; slot.data_base
//!   rbx = movsxd [rcx + slot*0x40 + 0x34]        ; slot.mask  (SIGNED)
//!   edx = (tag & 0x1fff) * [rcx + slot*0x40 + 0x30]  ; index * slot.entry_size
//!   rbx &= [rsi + rdx + 8]                       ; THE ENTRY ARRAY
//!   rsi = (rsi + rdx) - rbx                      ; THE OBJECT, at a back-offset from its own entry
//!   movsx ecx, byte [rsi + r8 + 0x1ac]
//! ```
//!
//! Four memory populations, and the three observed faults were one from each of the first three:
//!
//! | population | reached via | 2026-08-09 fault |
//! |---|---|---|
//! | the arena table's content DB blob | `FUN_001072C0` | `0x22db821de` (`+0x2bd5e` of *client reference manager*) |
//! | the package registry slot table | `**[0x1a0b420]` | `0x2044126d0` (slot `0x3d1`, field `+0x10`) |
//! | a slot's entry array | `slot.data_base` | `0x38031ca38` (slot `0x3d1`'s entries) |
//! | an object | `entry - (mask & loc)` | - never reached |
//!
//! # The two tables this walks, and why neither is guesswork
//!
//! **The arena table** (`v:0x1abcb80`) is a self-relative array of `0x140`-byte records with which
//! the client describes its own named heaps. Each record is `{ name[0x30], base @ +0x30,
//! size @ +0x50 }`, and the names read straight out of the live process: *"Datum Array Table
//! Manager"*, *"Tag Memory"*, *"client reference manager"*, *"Resourcerer entry array cache"*. So an
//! enclosing arena is an **exact extent**, not a guessed window - measured: the content DB blob for
//! live index 182 is `0x22db56480 + 0x2be80`, and the observed fault at `+0x2bd5e` is inside it.
//!
//! **The package registry** (`**[0x1a0b420]`) is `{ table_base, table_end, slot_stride }` followed
//! by `(table_end - table_base) / 0x40` slot records, each `{ data_base @ +0x10, entries_end @
//! +0x20, entry_size @ +0x30, mask @ +0x34 }`. The entry array is `[data_base, entries_end)` - **the
//! client states its own bound**; we do not count entries and hope.
//!
//! # ⚠ What this preset does NOT do, stated plainly
//!
//! * It captures the arenas the *roots* land in, not all 199 (345 MiB at the measured instant).
//!   A lookup that reaches a fifth population still faults - and that fault is still a **result**.
//! * Objects are captured as a bounded window from each object's own start
//!   (`--preset-object-window`, default one page). An object longer than the window, read past it,
//!   faults. The window is recorded in the manifest so a later reader can see the bound that bit.
//! * An entry whose back-offset lands outside any mapping is **skipped and counted**, never
//!   "captured as zero".
//! * It is a *selection* policy. It changes what is in the file, never what the bytes mean, and it
//!   inherits the snapshot's reach tag like everything else.

use anyhow::{anyhow, bail, Context, Result};

use crate::live::Proc;
use crate::symbols::Symbols;

pub const PAGE: u64 = 0x1000;

/// The names `--preset` accepts. One today; the list is here so an unknown name is an error with a
/// menu rather than a silent no-op.
pub const KNOWN: &[&str] = &["content"];

#[derive(Debug, Clone, Copy)]
pub struct Opts {
    /// Bytes to take at each resolved object, from the object's own start.
    pub object_window: u64,
    /// An enclosing arena larger than this is windowed instead of taken whole.
    pub arena_cap: u64,
    /// Refuse (loudly) rather than silently truncate above this many bytes.
    pub max_bytes: u64,
}

impl Default for Opts {
    fn default() -> Self {
        Self { object_window: PAGE, arena_cap: 32 << 20, max_bytes: 256 << 20 }
    }
}

/// One `{ name, base, size }` record of the client's named-heap table.
#[derive(Debug, Clone)]
pub struct Arena {
    pub index: usize,
    pub name: String,
    pub base: u64,
    pub size: u64,
}

impl Arena {
    pub fn contains(&self, a: u64) -> bool {
        self.size != 0 && a >= self.base && a < self.base + self.size
    }
}

/// The result of a preset: ranges to capture, plus everything a later reader needs to judge it.
#[derive(Debug, Default)]
pub struct Preset {
    pub regions: Vec<(u64, u64, String)>,
    /// Facts measured while walking - these go into the manifest's notes verbatim.
    pub notes: Vec<String>,
    /// Every bound that bit, and every candidate refused, with its reason.
    pub skips: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Pure helpers - unit-tested offline, because the arithmetic is where a wrong answer would look
// perfectly plausible.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// `object = entry - (mask & loc)`, with the client's own signedness.
///
/// `mask` is loaded with `movsxd` (sign-extended i32) and the subtraction is 64-bit, so a `mask` of
/// `-1` means "the whole qword is a back-offset" and a `mask` of `0` means "the entry *is* the
/// object". Getting this wrong yields addresses that are plausible and wrong.
pub fn object_addr(entry: u64, mask: i32, loc: i64) -> u64 {
    let off = (mask as i64) & loc;
    (entry as i64).wrapping_sub(off) as u64
}

/// Is a resolved object address worth capturing? A slot's entry array is mostly empty, and an empty
/// entry resolves to garbage - so this is the filter that keeps a 4096-slot walk from proposing the
/// whole address space.
///
/// `entry - object` must be a *back*-offset (the client subtracts), non-zero, plausible in size, and
/// the result must be a real address. Mapping residency is checked by the caller.
pub fn object_plausible(entry: u64, object: u64) -> bool {
    const MAX_BACK: u64 = 1 << 33; // 8 GiB: wider than the guest heap, narrower than nonsense
    object >= PAGE && object < entry && entry - object <= MAX_BACK
}

/// Sort and merge `(lo, hi, why)` ranges. Two adjacent ranges become one, and the reasons are kept.
pub fn merge(mut v: Vec<(u64, u64, String)>) -> Vec<(u64, u64, String)> {
    v.sort_by_key(|(a, _, _)| *a);
    let mut out: Vec<(u64, u64, String)> = Vec::new();
    for (lo, hi, why) in v {
        if let Some(last) = out.last_mut() {
            if lo <= last.1 {
                last.1 = last.1.max(hi);
                if !last.2.contains(&why) && last.2.len() < 400 {
                    last.2 = format!("{}; {}", last.2, why);
                }
                continue;
            }
        }
        out.push((lo, hi, why));
    }
    out
}

/// Round a `[lo, hi)` range out to page boundaries - the unit the capture actually works in.
pub fn page_out(lo: u64, hi: u64) -> (u64, u64) {
    (lo & !(PAGE - 1), hi.div_ceil(PAGE) * PAGE)
}

/// Parse one arena record. `None` means "the table ends here".
pub fn parse_arena(index: usize, rec: &[u8]) -> Option<Arena> {
    if rec.len() < 0x58 {
        return None;
    }
    let name_bytes: &[u8] = &rec[..0x30];
    let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(0x30);
    let name: String = name_bytes[..end].iter().map(|&b| b as char).collect();
    let base = u64::from_le_bytes(rec[0x30..0x38].try_into().ok()?);
    let size = u64::from_le_bytes(rec[0x50..0x58].try_into().ok()?);
    if name.is_empty() && base == 0 {
        return None;
    }
    Some(Arena { index, name, base, size })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The live walk
// ─────────────────────────────────────────────────────────────────────────────────────────────────

fn u64_at(proc: &Proc, a: u64) -> Result<u64> {
    Ok(u64::from_le_bytes(
        proc.read(a, 8).with_context(|| format!("reading a u64 at {a:#x}"))?.try_into().unwrap(),
    ))
}

fn sym(syms: &Symbols, name: &str) -> Result<u64> {
    syms.lookup(name).ok_or_else(|| {
        anyhow!(
            "guest-symbols.json has no `{name}`. This preset reads the project's one address book \
             rather than carrying its own copy of a vaddr (ATLAS §4.3) - add it THERE, with a \
             source and a confidence."
        )
    })
}

/// The client's named-heap table, read out of the live process.
pub fn arenas(proc: &Proc, base: u64, syms: &Symbols) -> Result<(u64, u64, Vec<Arena>)> {
    let root = base + sym(syms, "memory_arena_table")?;
    let self_rel = u64_at(proc, root)?;
    let stride = u64_at(proc, root + 0x10)?;
    if !(0x40..=0x1000).contains(&stride) || self_rel > 0x10000 {
        bail!(
            "the arena table at {root:#x} does not look like one (self_rel={self_rel:#x}, \
             stride={stride:#x}). Refusing to walk a structure I cannot recognise - a plausible \
             walk over the wrong bytes is the failure this check exists to stop."
        );
    }
    let arr = root + self_rel;
    let mut out = Vec::new();
    for i in 0..1024usize {
        let Some(rec) = proc.try_read(arr + i as u64 * stride, 0x58) else { break };
        match parse_arena(i, &rec) {
            Some(a) => out.push(a),
            None => break,
        }
    }
    if out.is_empty() {
        bail!("the arena table at {arr:#x} is empty - the client has not built its heaps yet");
    }
    Ok((arr, stride, out))
}

fn enclosing<'a>(list: &'a [Arena], a: u64) -> Option<&'a Arena> {
    list.iter().find(|x| x.contains(a))
}

fn push(want: &mut Vec<(u64, u64, String)>, lo: u64, hi: u64, why: String) {
    if hi > lo {
        let (lo, hi) = page_out(lo, hi);
        want.push((lo, hi, why));
    }
}

/// ⭐ `--preset content` - the package/content registry working set, in one shot.
pub fn content(proc: &Proc, base: u64, syms: &Symbols, o: Opts) -> Result<Preset> {
    let mut p = Preset::default();
    let mut want: Vec<(u64, u64, String)> = Vec::new();

    // ── 1. The arena table, and the arena the content DB blob lives in ───────────────────────────
    let (arr, stride, list) = arenas(proc, base, syms)?;
    let idx_addr = base + sym(syms, "memory_arena_index")?;
    let idx = u16::from_le_bytes(
        proc.read(idx_addr, 2).context("reading the live arena index")?.try_into().unwrap(),
    ) as usize;
    p.notes.push(format!(
        "preset content: arena table at {arr:#x} stride {stride:#x}, {} arena(s); live index {idx}",
        list.len()
    ));
    push(&mut want, arr, arr + (list.len() as u64 + 1) * stride, "preset content: arena table".into());

    if idx < list.len() {
        let blob = u64_at(proc, arr + idx as u64 * stride + 0x30)?;
        match enclosing(&list, blob) {
            Some(a) if a.size <= o.arena_cap => {
                p.notes.push(format!(
                    "preset content: content DB blob {blob:#x} is arena[{}] '{}' {:#x}+{:#x} - taken \
                     WHOLE (an exact extent, from the client's own record)",
                    a.index, a.name, a.base, a.size
                ));
                push(&mut want, a.base, a.base + a.size, format!("preset content: arena '{}'", a.name));
            }
            Some(a) => {
                p.skips.push(format!(
                    "preset content: arena[{}] '{}' is {:#x} bytes, over --preset-arena-cap {:#x}; \
                     taking a {:#x}-byte WINDOW from the blob instead - a read past it will FAULT, \
                     and that fault is a result",
                    a.index, a.name, a.size, o.arena_cap, o.arena_cap
                ));
                push(&mut want, blob, blob + o.arena_cap, "preset content: content DB window".into());
            }
            None => {
                let w = o.object_window.max(1 << 20);
                p.skips.push(format!(
                    "preset content: content DB blob {blob:#x} is in NO named arena; taking a \
                     {w:#x}-byte window - and it is a WINDOW, not an extent"
                ));
                push(&mut want, blob, blob + w, "preset content: content DB window".into());
            }
        }
    } else {
        p.skips.push(format!(
            "preset content: live arena index {idx} is past the {}-entry table - the content DB \
             root was NOT captured",
            list.len()
        ));
    }

    // ── 1b. The OBJECT registry heap - not on the lookup's path, and deliberately here anyway ────
    // `d1-replay controls` ends with a registry cross-check that reproduces `d1guest.py selftest`'s
    // 18/18 from the snapshot alone. Without this window that control SKIPs, and a snapshot that
    // cannot run its own control block is a snapshot nobody should cite. 0x60000 is the size the
    // control's own error message names - a generous WINDOW, not an extent, and recorded as one.
    // ⚠ It is not read by the content lookup, so it cannot flatter an A/B of this preset.
    if let Some(orp) = syms.lookup("object_registry_ptr") {
        match u64_at(proc, base + orp) {
            Ok(0) | Err(_) => p.skips.push(
                "preset content: object_registry_ptr is NULL or unreadable - this snapshot will \
                 SKIP the registry cross-check control"
                    .into(),
            ),
            Ok(reg) => {
                const OBJ_REGISTRY_WINDOW: u64 = 0x60000;
                push(
                    &mut want,
                    reg,
                    reg + OBJ_REGISTRY_WINDOW,
                    "preset content: object registry heap (for the snapshot's own control block)"
                        .into(),
                );
                p.notes.push(format!(
                    "preset content: object registry {reg:#x} + {OBJ_REGISTRY_WINDOW:#x} - a \
                     WINDOW, not an extent; it is here for the control block, not for the lookup"
                ));
            }
        }
    }

    // ── 2. The package registry: header and slot table ───────────────────────────────────────────
    let g = base + sym(syms, "package_registry_global")?;
    let p1 = u64_at(proc, g)?;
    if p1 == 0 {
        bail!(
            "package_registry_global (v:{:#x}) is NULL - the client has not built the package \
             registry. That is an ERROR, not an empty preset: capturing now would produce a \
             snapshot whose content lookups are unreplayable for a reason no later reader could see.",
            g - base
        );
    }
    let table_base = u64_at(proc, p1)?;
    let table_end = u64_at(proc, p1 + 8)?;
    let slot_stride = u64_at(proc, p1 + 0x10)?;
    if slot_stride < 0x40
        || table_end <= table_base
        || (table_end - table_base) % slot_stride != 0
        || (table_end - table_base) > 0x40_0000
    {
        bail!(
            "the package registry header at {p1:#x} is not a slot table \
             (base={table_base:#x} end={table_end:#x} stride={slot_stride:#x}). Refusing to walk it."
        );
    }
    let slots = (table_end - table_base) / slot_stride;
    push(&mut want, p1, p1 + 0x40, "preset content: registry header".into());
    match enclosing(&list, p1) {
        Some(a) if a.size <= o.arena_cap => {
            p.notes.push(format!(
                "preset content: the registry lives in arena[{}] '{}' {:#x}+{:#x} - taken WHOLE",
                a.index, a.name, a.base, a.size
            ));
            push(&mut want, a.base, a.base + a.size, format!("preset content: arena '{}'", a.name));
        }
        _ => p.skips.push(format!(
            "preset content: the registry header {p1:#x} is not inside a capturable named arena; \
             taking the slot table exactly and nothing around it"
        )),
    }
    push(&mut want, table_base, table_end, format!("preset content: slot table ({slots} slots)"));

    // ── 3. Every populated slot's entry array, and the objects it points back at ─────────────────
    let table = proc
        .read(table_base, (table_end - table_base) as usize)
        .context("reading the package registry slot table")?;
    let (mut populated, mut entry_bytes) = (0u64, 0u64);
    let (mut objects, mut unmapped, mut malformed) = (0u64, 0u64, 0u64);
    for slot in 0..slots {
        let rec = &table[(slot * slot_stride) as usize..][..0x40];
        let data_base = u64::from_le_bytes(rec[0x10..0x18].try_into().unwrap());
        let entries_end = u64::from_le_bytes(rec[0x20..0x28].try_into().unwrap());
        let entry_size = i32::from_le_bytes(rec[0x30..0x34].try_into().unwrap());
        let mask = i32::from_le_bytes(rec[0x34..0x38].try_into().unwrap());
        if data_base == 0 || entries_end <= data_base || entry_size <= 0 {
            continue;
        }
        let span = entries_end - data_base;
        if span > 0x400_0000 {
            malformed += 1;
            p.skips.push(format!(
                "preset content: slot {slot:#x} claims a {span:#x}-byte entry array - refused, \
                 rather than capturing 64 MiB on the strength of one qword"
            ));
            continue;
        }
        populated += 1;
        entry_bytes += span;
        let n = span / entry_size as u64;
        push(
            &mut want,
            data_base,
            entries_end,
            format!("preset content: slot {slot:#x} entries ({n} x {entry_size})"),
        );
        if mask == 0 {
            // `object == entry`: the entry IS the object, and the array above already covers it.
            continue;
        }
        let Some(blob) = proc.try_read(data_base, span as usize) else {
            p.skips.push(format!(
                "preset content: slot {slot:#x}'s entry array at {data_base:#x} is unreadable - its \
                 objects were NOT resolved"
            ));
            continue;
        };
        for i in 0..n {
            let at = (i * entry_size as u64) as usize + 8;
            let loc = i64::from_le_bytes(blob[at..at + 8].try_into().unwrap());
            let entry = data_base + i * entry_size as u64;
            let object = object_addr(entry, mask, loc);
            if !object_plausible(entry, object) {
                continue;
            }
            if proc.mapping_of(object).is_none() {
                unmapped += 1;
                continue;
            }
            objects += 1;
            push(
                &mut want,
                object,
                object + o.object_window,
                format!("preset content: slot {slot:#x} objects"),
            );
        }
    }

    p.notes.push(format!(
        "preset content: registry {p1:#x} -> slot table {table_base:#x}..{table_end:#x} \
         ({slots} slots, {populated} populated, {:.2} MiB of entry arrays)",
        entry_bytes as f64 / (1u64 << 20) as f64
    ));
    p.notes.push(format!(
        "preset content: {objects} object(s) resolved through the back-offset, window {:#x} each; \
         {unmapped} entry target(s) were in no mapping and were SKIPPED (never zero-filled); \
         {malformed} slot(s) refused as malformed",
        o.object_window
    ));

    let merged = merge(want);
    let total: u64 = merged.iter().map(|(l, h, _)| h - l).sum();
    if total > o.max_bytes {
        bail!(
            "preset content would capture {:.1} MiB, over --preset-max ({:.1} MiB). Refusing, \
             loudly: a silently truncated preset is exactly the 'we did not capture it' trap this \
             tool exists to avoid. Raise --preset-max if you mean it.",
            total as f64 / (1u64 << 20) as f64,
            o.max_bytes as f64 / (1u64 << 20) as f64
        );
    }
    p.notes.push(format!(
        "preset content: {} merged range(s), {:.2} MiB before mapping clipping",
        merged.len(),
        total as f64 / (1u64 << 20) as f64
    ));
    p.regions = merged;
    Ok(p)
}

/// Dispatch by name, so an unknown `--preset` is an error with a menu.
pub fn by_name(name: &str, proc: &Proc, base: u64, syms: &Symbols, o: Opts) -> Result<Preset> {
    match name {
        "content" => content(proc, base, syms, o),
        other => bail!("unknown --preset {other:?}; known presets: {}", KNOWN.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_addr_uses_the_clients_signedness() {
        // mask = -1: the whole qword is a back-offset.
        assert_eq!(object_addr(0x1000, -1, 0x100), 0xF00);
        // mask = 0: the entry IS the object. This must NOT produce a second region.
        assert_eq!(object_addr(0x1000, 0, 0x1234_5678_9ABC), 0x1000);
        // The mask is sign-extended from i32 before the AND. A u32 mask would drop the high half of
        // `loc` and give an address ~4 GiB away - plausible-looking, and wrong.
        assert_eq!(object_addr(0x2_0000_0000, -1, 0x1_0000_0000), 0x1_0000_0000);
        assert_eq!(object_addr(0x1000, 0x0000_00FF, 0x1234_5678), 0x1000 - 0x78);
    }

    #[test]
    fn object_plausible_rejects_what_an_empty_entry_resolves_to() {
        assert!(object_plausible(0x2_0000_0000, 0x1_FFFF_0000));
        // entry == object (mask 0, or loc 0) is not a separate object.
        assert!(!object_plausible(0x1000, 0x1000));
        // forward offsets: the client subtracts, so an object above its entry is a misread.
        assert!(!object_plausible(0x1000, 0x2000));
        // NULL and the low pages.
        assert!(!object_plausible(0x1000, 0));
        assert!(!object_plausible(0x1_0000_0000, 0x800));
        // absurd distance - the shape a garbage `loc` takes.
        assert!(!object_plausible(0x10_0000_0000, 0x1000));
    }

    /// ⚠ INVERTED: this asserts the filter can FAIL, not merely that it passes. A filter that
    /// accepted everything would satisfy every test above by accident.
    #[test]
    fn object_plausible_is_not_a_rubber_stamp() {
        let entry = 0x2_0000_0000u64;
        let accepted = (0..64u32)
            .map(|k| object_addr(entry, -1, 1i64 << k))
            .filter(|&o| object_plausible(entry, o))
            .count();
        assert!(accepted > 0, "the filter accepts nothing - it would capture no objects at all");
        assert!(accepted < 64, "the filter accepts every back-offset - it discriminates nothing");
    }

    #[test]
    fn merge_joins_and_keeps_reasons() {
        let m = merge(vec![
            (0x2000, 0x3000, "b".into()),
            (0x1000, 0x2000, "a".into()),
            (0x9000, 0xA000, "c".into()),
        ]);
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].0, m[0].1), (0x1000, 0x3000));
        assert!(m[0].2.contains('a') && m[0].2.contains('b'));
        assert_eq!((m[1].0, m[1].1), (0x9000, 0xA000));
    }

    #[test]
    fn page_out_rounds_outward_only() {
        assert_eq!(page_out(0x1234, 0x1235), (0x1000, 0x2000));
        assert_eq!(page_out(0x1000, 0x2000), (0x1000, 0x2000));
    }

    #[test]
    fn parse_arena_reads_name_base_and_size() {
        let mut rec = vec![0u8; 0x140];
        rec[..10].copy_from_slice(b"Tag Memory");
        rec[0x30..0x38].copy_from_slice(&0x3_8070_0000u64.to_le_bytes());
        rec[0x50..0x58].copy_from_slice(&0x6E0_0000u64.to_le_bytes());
        let a = parse_arena(2, &rec).expect("a named arena parses");
        assert_eq!(a.name, "Tag Memory");
        assert_eq!(a.base, 0x3_8070_0000);
        assert_eq!(a.size, 0x6E0_0000);
        assert!(a.contains(0x3_8070_0000 + 1));
        assert!(!a.contains(0x3_8070_0000 - 1));
        // A blank record ends the table - otherwise the walk runs off into whatever follows it.
        assert!(parse_arena(3, &vec![0u8; 0x140]).is_none());
    }
}
