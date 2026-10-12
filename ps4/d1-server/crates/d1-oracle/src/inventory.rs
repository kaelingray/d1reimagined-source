// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The **family-4 / type-3 inventory element** (`0x80801732`, 184 B) - built so the client can
//! actually resolve, render and *equip* the item.
//!
//! # Why this module exists
//!
//! `d1-responder::stage_loadout()` hand-rolled the 184-byte struct and wrote, for **every item in
//! every run this project has ever made**:
//!
//! | offset | it wrote | what the client reads there |
//! |---|---|---|
//! | `+0x18` | the item's content **hash** (~10⁹) | ⭐ the item's **LEVEL** (`FUN_0026F480`, `i32`) |
//! | `+0x1C` | the item's **bucketTypeHash** (~10⁹) | ⭐ the **equip MINIMUM STAT** (`0x269971`) |
//! | `+0x24` | the literal **`1`** | ⭐ the **content index** → the item DEFINITION |
//! | `+0x26` | `0` × 8 | eight extra content indices; `0` is a *valid* index |
//! | `+0x44` | `0` | the progression ordinal; `0` names progression **0** |
//! | `+0x58` | `0` | the talent-grid index; `0` names grid **0** and leaks its stats |
//!
//! Content index **1** is *'Bronze Tier'* - `stats:{}`, `equippable:false`, `hasGeometry:false` -
//! so the item has no definition worth reading, no model, no stats, and the equip check compares
//! the character's stat against a **nine-digit minimum** and refuses. That is the padlock.
//!
//! Full attribution, field by field, with the instruction that proves each one:
//! `historicaldocs/design/inventory-instance-fields.md`. This module is that document made executable.
//!
//! # The rule this module enforces in code rather than in prose
//!
//! ⭐ **A hash never lands in a field the client reads as a small number.** [`ItemSpec::to_object`]
//! *refuses* a level above [`MAX_PLAUSIBLE_LEVEL`] and an equip minimum above
//! [`MAX_PLAUSIBLE_STAT`]. Both bounds exist for exactly one reason: the values we shipped were
//! `3_564_229_425` and `953_998_645`, and **nothing in the tree could fail because of it.**
//! A guard that can only fail in the direction we care about is the whole point.
//!
//! # ⚠ Reach
//!
//! `static-client-inference` throughout: every offset here is read out of the client's descriptor
//! tree (and re-derived by [`derive_layout`] on every `controls` run), and every *meaning* is read
//! out of the client's own disassembly. Nothing here is `observed-live` - the discriminator is a
//! run, and the on-screen result is the only observable, because
//! `c_investment_globals_interface` and the inventory iterators emit **no diagnostics at all**.

use std::path::Path;

use d1_eboot::descriptors::{self, Descriptor, Field, Index, LEAF};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::item_index::IndexTable;
use crate::object::{Decoded, ObjectOracle};

/// Schema tag of the family-4/type-3 element. 666 slots of 184 bytes; instance type `0xEF38431D`.
pub const TAG: u32 = 0x8080_1732;
pub const STRUCT_SIZE: usize = 184;

// ── offsets, every one re-derived by `derive_layout` ─────────────────────────────────────────────

/// `+0x00` - the registry lookup key. `0x56CA70` compares `element[0]` against the key it is given.
pub const OFF_SOID: usize = 0x00;
/// `+0x10` - a 2-bit field inside `0x80801920`. `[U]`; we write `1`, as every fixture has.
pub const OFF_STATE_BITS: usize = 0x10;
/// ⭐ `+0x18` - **the item's LEVEL**, `i32`. `FUN_0026F480`'s selector is the constant thunk
/// `0x14D0` (`30 c0 c3` = `xor al,al; ret`) ⇒ 0, so it returns `element + 0x18 + 0*4`. Four callers
/// gate on it (`rec[0x10] <= v <= rec[0x14]`), and `FUN_002816F0` adds it into the stat curve.
pub const OFF_LEVEL: usize = 0x18;
/// ⭐ `+0x1C` - **the equip MINIMUM STAT**, entry 1 of the same `0x80801711` array.
/// `c_investment_character_equipment_const_iterator::ctor` at `0x269971`:
/// `cmp eax,[rcx+rbx*4+0x1c]` / `jge` / else `or byte [rax],8` - **bit 3 is NOT-EQUIPPABLE**.
pub const OFF_EQUIP_MIN_STAT: usize = 0x1C;
/// `+0x20` - the stat-curve index offset, `i16`. `FUN_002816F0` samples the curve at `+0x20 + level`.
pub const OFF_CURVE_OFFSET: usize = 0x20;
/// ⭐⭐ `+0x24` - **the CONTENT INDEX**, the argument of
/// `c_investment_globals_interface::vfunc[130]` (`0x25ADE0`, `vtbl+0x410`). 20 distinct functions
/// read it. ⛔ Out-of-range fails loudly; a *wrong* index is completely silent.
pub const OFF_CONTENT_INDEX: usize = 0x24;
/// ⭐ `+0x26` - eight more content indices whose world tags this item also wants loaded
/// (`0x80801018`, 16 B / stride 2). ⛔ **`0x555730`'s tail is a fixed 8-iteration loop** in which
/// `0xFFFF` is a per-entry SKIP, so a sentinel in slot 0 does **not** terminate slots 1-7.
pub const OFF_EXTRA_INDICES: usize = 0x26;
pub const EXTRA_INDEX_SLOTS: usize = 8;
/// `+0x38` - progression POINTS (XP). `FUN_00284630` subtracts per-level costs until it underflows.
pub const OFF_PROGRESS_POINTS: usize = 0x38;
/// ⭐ `+0x44` - the **progression ordinal**, `u16` → `vtbl[0x478]` (`0x25B200`, stride `0x38`).
/// `0xFFFF` = none: `FUN_00284630` returns 0 immediately and
/// `c_investment_item_progression_iterator<0,340>::vfunc[1]` (`0x10559F0`) filters the item out of
/// the *item-progression* list - which is the correct answer for an item with no XP bar, and is
/// **not** a rejection from the equip or render paths. ⛔ `0` names progression **0**.
pub const OFF_PROGRESSION_ORDINAL: usize = 0x44;
/// ⭐ `+0x58` - the **talent-grid index**, `u16` → `vtbl[0x4E0]` (`0x25B960`). ⛔ `0xFFFF` is the
/// only "none": `FUN_00283A30` returns 0 *only* for `-1`, so `0` silently selects grid #0 and leaks
/// its stats into this item.
pub const OFF_TALENT_GRID: usize = 0x58;
/// `+0x5A` - the 36-byte talent-grid node-state array for the `+0x58` grid.
pub const OFF_NODE_STATES: usize = 0x5A;
pub const NODE_STATE_SLOTS: usize = 36;
/// `+0x7E` - 8 node STEP selections, consumed as `byte / 255.0` interpolants.
pub const OFF_NODE_STEPS: usize = 0x7E;
/// `+0x86` - `[U]`; we write `1`, as every fixture has.
pub const OFF_FLAG_86: usize = 0x86;
/// ⭐ `+0x88` - per-instance stat **indices**, 8 × u16, the key array paired with `+0x98`.
pub const OFF_STAT_KEYS: usize = 0x88;
/// ⭐ `+0x98` - per-instance stat **values**, 8 × u32, indexed by *position in the definition's stat
/// list* at `definition+0x68`, clamped to `definition+0x20`.
pub const OFF_STAT_VALUES: usize = 0x98;
pub const STAT_SLOTS: usize = 8;

/// `0xFFFF` is *absent* on every `u16` index field of this object. ⛔ `0x0000` is a **real** index
/// everywhere (`0x1056640: cmp word[rsi],0xFFFF; setne`), and all five table accessors
/// (`vtbl[0x410]/[0x430]/[0x478]/[0x4E0]/[0x538]`) treat `0` as valid.
pub const EMPTY_INDEX: u16 = 0xFFFF;

/// The level cap this client's `character_level` progression carries (`0x276220` → 40, replayed).
/// A level above this is not *wrong* by itself; a level above [`MAX_PLAUSIBLE_LEVEL`] is a hash.
pub const LEVEL_CAP: i32 = 40;

/// ⭐ The guard that would have caught the shipped bug. Any `+0x18` above this is not a level, it
/// is a content hash - the value we shipped for Gjallarhorn was `3_564_229_425`, which as `i32` is
/// **-730,737,871** and floors every stat curve it feeds.
pub const MAX_PLAUSIBLE_LEVEL: i32 = 1_000;
/// ⭐ The same guard for `+0x1C`. We shipped `953_998_645` - the Heavy Weapons *bucket hash* - as a
/// **minimum required stat**, so every item this project ever served was un-equippable by
/// construction and the screen drew a padlock.
pub const MAX_PLAUSIBLE_STAT: u32 = 100_000;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The bucket ⇄ slot table
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One row of the client's bucket table, as far as slot placement needs it.
///
/// ⭐⭐ **The inventory tab is POSITIONAL.** `cui::c_investment_backpack_item::ctor` (`0x9F2CC0`) at
/// `0x9F2D60` does `mov edx,[r12+0x24]` (the **slot index**), `mov esi,[r12+0x34]` (the scope) and
/// calls `FUN_0026DB60` - **the item is not an input.** That leaf scans 39 records of `0x14` bytes
/// `{u8 id @+0x00, i32 first_slot @+0x04, i32 slot_count @+0x08, u8 scope @+0x10}` and returns the
/// first whose scope matches and whose `[first, first+count)` contains the slot.
///
/// ⇒ writing item *n* into slot *n* puts **everything in Materials** (slots 0-19), which is what
/// every fixture before 2026-08-10 did and exactly what the screen showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BucketRange {
    /// The `Slot::display_name()` `d1-loadout` uses, which is also the staged filename's suffix.
    pub name: &'static str,
    pub first_slot: u16,
    pub slot_count: u16,
}

impl BucketRange {
    pub fn contains(&self, slot: u16) -> bool {
        slot >= self.first_slot && slot < self.first_slot + self.slot_count
    }
}

/// ⭐⭐ **THE COMPLETE 39-RECORD TABLE, MEASURED TWICE.** `[C]`
///
/// Read out of the live client on 2026-08-10 -
/// `[v:0x2639AE0]` → interface → vptr `0x18F4F70`, slot 19 = `0x25D250`; `vfunc[1]`'s cache
/// `[v:0x2639AF8]` → globals; `globals+0x108` = tag `0x80A5FFB6`; `vfunc[19]` returns the resolved
/// object `+8` - and then **re-derived independently** by executing the client's own
/// `FUN_0026DB60` on `runs/snapshots/orbit-all.snap` (a *different boot*, 2026-08-08, pid 147351).
/// Both instruments return the same table object address.
///
/// ⭐ The **inverted control**: one slot past the last range returns `0xFF` in *each* scope
/// (scope 0 slot 322, scope 1 slot 318), so "found" means found. And both scopes tile with **zero
/// gaps and zero overlaps** - scope 0 covers slots 0-321 over 30 ranges, scope 1 covers 0-317 over
/// 9. Provenance and every control: `capture-rig/inventory-buckets.tsv`.
///
/// ⛔ **This retracts `historicaldocs/design/inventory-instance-fields.md` §3.2's claim** that ids `0x11`
/// (163-167), `0x12` (168) and `0x13` (209) have "no bucket-definition row". They are records 15,
/// 16 and 23 here, and the client's own leaf returns `0x13` for slot 209. They are *non-equippable*
/// buckets, not unreachable ones.
///
/// `scope` 0 = profile/character, 1 = account (the Vault).
pub const BUCKET_TABLE: &[BucketRecord] = &[
    BucketRecord { id: 0x05, first_slot: 0, slot_count: 20, scope: 0 },
    BucketRecord { id: 0x01, first_slot: 20, slot_count: 20, scope: 0 },
    BucketRecord { id: 0x06, first_slot: 40, slot_count: 3, scope: 0 },
    BucketRecord { id: 0x07, first_slot: 43, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x08, first_slot: 53, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x09, first_slot: 63, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0A, first_slot: 73, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0B, first_slot: 83, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x02, first_slot: 93, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x03, first_slot: 103, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x04, first_slot: 113, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0C, first_slot: 123, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0F, first_slot: 133, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0D, first_slot: 143, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x0E, first_slot: 153, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x11, first_slot: 163, slot_count: 5, scope: 0 },
    BucketRecord { id: 0x12, first_slot: 168, slot_count: 1, scope: 0 },
    BucketRecord { id: 0x10, first_slot: 169, slot_count: 20, scope: 0 },
    BucketRecord { id: 0x14, first_slot: 189, slot_count: 20, scope: 0 },
    BucketRecord { id: 0x15, first_slot: 0, slot_count: 20, scope: 1 },
    BucketRecord { id: 0x16, first_slot: 20, slot_count: 108, scope: 1 },
    BucketRecord { id: 0x17, first_slot: 128, slot_count: 72, scope: 1 },
    BucketRecord { id: 0x18, first_slot: 200, slot_count: 108, scope: 1 },
    BucketRecord { id: 0x13, first_slot: 209, slot_count: 1, scope: 0 },
    BucketRecord { id: 0x19, first_slot: 210, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x1B, first_slot: 220, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x1C, first_slot: 230, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x1E, first_slot: 240, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x1D, first_slot: 250, slot_count: 12, scope: 0 },
    BucketRecord { id: 0x1A, first_slot: 262, slot_count: 16, scope: 0 },
    BucketRecord { id: 0x1F, first_slot: 308, slot_count: 1, scope: 1 },
    BucketRecord { id: 0x20, first_slot: 278, slot_count: 1, scope: 0 },
    BucketRecord { id: 0x21, first_slot: 279, slot_count: 1, scope: 0 },
    BucketRecord { id: 0x22, first_slot: 309, slot_count: 1, scope: 1 },
    BucketRecord { id: 0x23, first_slot: 310, slot_count: 1, scope: 1 },
    BucketRecord { id: 0x24, first_slot: 280, slot_count: 10, scope: 0 },
    BucketRecord { id: 0x25, first_slot: 290, slot_count: 32, scope: 0 },
    BucketRecord { id: 0x26, first_slot: 311, slot_count: 1, scope: 1 },
    BucketRecord { id: 0x27, first_slot: 312, slot_count: 6, scope: 1 },
];

/// One record of [`BUCKET_TABLE`], exactly as `FUN_0026DB60` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BucketRecord {
    pub id: u8,
    pub first_slot: u16,
    pub slot_count: u16,
    pub scope: u8,
}

impl BucketRecord {
    pub fn contains(&self, slot: u16) -> bool {
        slot >= self.first_slot && slot < self.first_slot + self.slot_count
    }
}

/// The **bucket id** for each name `d1-loadout`'s `Slot::display_name()` produces.
///
/// ⚠ The *ranges* are `[C]`; these **names** are `[I]`, matched by position against the fourteen
/// rows `historicaldocs/design/inventory-instance-fields.md` §3.2 read off a snapshot and by item-count
/// cross-partition against the 2020 Bungie manifest - a **sibling of this client, not the wire**.
///
/// ⛔ Five equippable count-10 buckets - `0x19` (210-219), `0x1B` (220-229), `0x1C` (230-239),
/// `0x1E` (240-249), `0x24` (280-289) - are exactly `{Artifacts, Sparrow Horn, Shaders, Emblems,
/// Emotes}` **in an order nothing has determined**: item counts cannot discriminate five buckets
/// that all hold ten. They are therefore **absent from this map on purpose**, and
/// [`slot_for_bucket`] returns `None` for them. Guessing one in five is a 20% chance of a silent
/// wrong answer, and a wrong bucket is invisible - it just puts the item in the wrong tab.
pub const BUCKET_IDS: &[(&str, u8)] = &[
    ("Materials", 0x05),
    ("Consumables", 0x01),
    ("Subclass", 0x06),
    ("Helmet", 0x07),
    ("Gauntlets", 0x08),
    ("Chest Armor", 0x09),
    ("Leg Armor", 0x0A),
    ("Class Armor", 0x0B),
    ("Primary Weapons", 0x02),
    ("Special Weapons", 0x03),
    ("Heavy Weapons", 0x04),
    ("Ships", 0x0C),
    ("Vehicle", 0x0D),
    ("Ghost", 0x0E),
];

/// ⛔ **Materials** - bucket `0x05`, slots 0-19. Every item this project has ever shipped landed
/// here, because the builder wrote item *n* at slot *n*. Kept named so a control can assert we are
/// **out** of it, which is the only assertion that can fail in the direction we care about.
pub const MATERIALS_ID: u8 = 0x05;

/// The record for a bucket id in the given scope.
pub fn bucket_record(id: u8, scope: u8) -> Option<&'static BucketRecord> {
    BUCKET_TABLE.iter().find(|r| r.id == id && r.scope == scope)
}

/// The bucket id a name maps to, or `None` when nothing has determined it.
pub fn bucket_id(name: &str) -> Option<u8> {
    BUCKET_IDS.iter().find(|(n, _)| *n == name).map(|(_, id)| *id)
}

/// The `nth` slot of the named bucket in the character scope, or `None` when the name has no
/// determined bucket id.
///
/// ⭐ Returning `None` is deliberate. The alternative - falling back to slot *n* - is precisely the
/// bug: it is silent, it looks like placement, and it puts the item in Materials.
pub fn slot_for_bucket(name: &str, nth: u16) -> Option<u16> {
    let r = bucket_record(bucket_id(name)?, 0)?;
    if nth >= r.slot_count {
        return None;
    }
    Some(r.first_slot + nth)
}

/// Which bucket id a character-scope slot falls in - the Rust twin of `FUN_0026DB60(0, slot)`,
/// first match wins. `None` is the leaf's `0xFF`.
pub fn bucket_for_slot(slot: u16) -> Option<u8> {
    BUCKET_TABLE.iter().find(|r| r.scope == 0 && r.contains(slot)).map(|r| r.id)
}

/// The display name for a bucket id, where one has been determined.
pub fn bucket_name(id: u8) -> Option<&'static str> {
    BUCKET_IDS.iter().find(|(_, i)| *i == id).map(|(n, _)| *n)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The spec
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One inventory element, with every field the client is *known* to read stated explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSpec {
    /// `+0x00`. Must be unique, and must match the SOID the profile names in its slot and in its
    /// equipped list - [`crate::profile`] derives both from the staged files so they cannot drift.
    pub soid: u64,
    /// ⭐ `+0x24`. Resolve it with [`IndexTable::require`]; never write a literal.
    pub content_index: u16,
    /// ⭐ `+0x18`, the item's level. Bounded by [`MAX_PLAUSIBLE_LEVEL`].
    pub level: i32,
    /// ⭐ `+0x1C`, the equip minimum. **`0` is the value that clears the padlock unconditionally**,
    /// because the test is `character_stat >= minimum` and every stat is `>= 0`.
    pub equip_min_stat: u32,
    /// `+0x20`, the stat-curve index offset.
    pub curve_offset: i16,
    /// `+0x26` × 8. Default [`EMPTY_INDEX`] in all eight.
    pub extra_content_indices: [u16; EXTRA_INDEX_SLOTS],
    /// `+0x38`, progression XP.
    pub progress_points: u32,
    /// `+0x44`. Default [`EMPTY_INDEX`].
    pub progression_ordinal: u16,
    /// `+0x58`. Default [`EMPTY_INDEX`].
    pub talent_grid: u16,
    /// `+0x88` / `+0x98`, paired. `[U]` - the key array's empty marker has no identified reader, so
    /// the default is *empty* (all zero, exactly what every fixture has had) rather than a guessed
    /// sentinel. Set it only with a discriminator in mind.
    pub stats: Vec<(u16, u32)>,
    /// `+0x10`, the 2-bit field. `[U]`; `1` reproduces every fixture.
    pub state_bits: u8,
    /// `+0x86`. `[U]`; `1` reproduces every fixture.
    pub flag_86: u8,
    /// The bucket this item belongs in, by `d1-loadout`'s `Slot::display_name()`. Carried so the
    /// profile can place it, and so the staged filename and the placement cannot disagree.
    pub bucket: String,
    /// The item's content hash - **our** naming convention, not a field the client reads on the
    /// render path. Written to nothing; carried so `content_index` can be re-derived and checked.
    pub item_hash: u32,
}

impl ItemSpec {
    /// A well-formed element for `hash`, with the content index resolved out of the measured table.
    ///
    /// ⭐ This is the constructor that makes the old bug unrepresentable: there is no way to get an
    /// `ItemSpec` with a literal content index without saying `content_index` out loud.
    pub fn resolved(soid: u64, hash: u32, bucket: &str, table: &IndexTable) -> Result<Self> {
        let content_index = table.require(hash)?;
        Ok(ItemSpec {
            soid,
            content_index,
            level: LEVEL_CAP,
            equip_min_stat: 0,
            curve_offset: 0,
            extra_content_indices: [EMPTY_INDEX; EXTRA_INDEX_SLOTS],
            progress_points: 0,
            progression_ordinal: EMPTY_INDEX,
            talent_grid: EMPTY_INDEX,
            stats: Vec::new(),
            state_bits: 1,
            flag_86: 1,
            bucket: bucket.to_string(),
            item_hash: hash,
        })
    }

    /// ⛔ The **pre-2026-08-10 element**, exactly as `stage_loadout()` wrote it. Only for bisecting,
    /// and it is what the negative controls assert must be **refused** by [`Self::to_object`].
    pub fn legacy_broken(soid: u64, hash: u32, bucket_hash: u32) -> Self {
        ItemSpec {
            soid,
            content_index: 1,
            level: hash as i32,
            equip_min_stat: bucket_hash,
            curve_offset: 1,
            extra_content_indices: [0; EXTRA_INDEX_SLOTS],
            progress_points: 0,
            progression_ordinal: 0,
            talent_grid: 0,
            stats: Vec::new(),
            state_bits: 1,
            flag_86: 1,
            bucket: String::new(),
            item_hash: hash,
        }
    }

    /// Lay the spec out as the decoded 184-byte C struct. **Refuses a hash in a small-number field.**
    pub fn to_object(&self) -> Result<Vec<u8>> {
        if self.soid == 0 {
            return Err(OracleError::Shape(
                "element SOID is 0 - `0x56CA70` keys the registry on it and nothing can be equipped \
                 to a zero key"
                    .into(),
            ));
        }
        if self.level > MAX_PLAUSIBLE_LEVEL || self.level < 0 {
            return Err(OracleError::Shape(format!(
                "+{OFF_LEVEL:#04x} level is {} - that is not a level, it is a content hash. \
                 `FUN_0026F480` returns this field as an i32 item LEVEL and `FUN_002816F0` adds it \
                 into the stat curve; the value this project shipped was 3564229425, i.e. \
                 -730737871, which floors every stat term. Bound is {MAX_PLAUSIBLE_LEVEL}",
                self.level
            )));
        }
        if self.equip_min_stat > MAX_PLAUSIBLE_STAT {
            return Err(OracleError::Shape(format!(
                "+{OFF_EQUIP_MIN_STAT:#04x} equip minimum is {} - that is a bucketTypeHash, not a \
                 stat. `0x269971` does `cmp eax,[rcx+rbx*4+0x1c]; jge; or byte [rax],8`, so this \
                 value is compared against the character's stat and anything this large marks the \
                 item NOT EQUIPPABLE - the padlock. Bound is {MAX_PLAUSIBLE_STAT}",
                self.equip_min_stat
            )));
        }
        if self.stats.len() > STAT_SLOTS {
            return Err(OracleError::Shape(format!(
                "{} per-instance stats exceeds the schema's {STAT_SLOTS} slots - the extra entries \
                 would land in the NEXT field and the codec would silently reshape them",
                self.stats.len()
            )));
        }

        let mut o = vec![0u8; STRUCT_SIZE];
        o[OFF_SOID..OFF_SOID + 8].copy_from_slice(&self.soid.to_le_bytes());
        o[OFF_STATE_BITS] = self.state_bits;
        o[OFF_LEVEL..OFF_LEVEL + 4].copy_from_slice(&self.level.to_le_bytes());
        o[OFF_EQUIP_MIN_STAT..OFF_EQUIP_MIN_STAT + 4]
            .copy_from_slice(&self.equip_min_stat.to_le_bytes());
        o[OFF_CURVE_OFFSET..OFF_CURVE_OFFSET + 2].copy_from_slice(&self.curve_offset.to_le_bytes());
        o[OFF_CONTENT_INDEX..OFF_CONTENT_INDEX + 2]
            .copy_from_slice(&self.content_index.to_le_bytes());
        for (i, v) in self.extra_content_indices.iter().enumerate() {
            let at = OFF_EXTRA_INDICES + i * 2;
            o[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
        o[OFF_PROGRESS_POINTS..OFF_PROGRESS_POINTS + 4]
            .copy_from_slice(&self.progress_points.to_le_bytes());
        o[OFF_PROGRESSION_ORDINAL..OFF_PROGRESSION_ORDINAL + 2]
            .copy_from_slice(&self.progression_ordinal.to_le_bytes());
        o[OFF_TALENT_GRID..OFF_TALENT_GRID + 2].copy_from_slice(&self.talent_grid.to_le_bytes());
        o[OFF_FLAG_86] = self.flag_86;
        for (i, (key, value)) in self.stats.iter().enumerate() {
            let k = OFF_STAT_KEYS + i * 2;
            let v = OFF_STAT_VALUES + i * 4;
            o[k..k + 2].copy_from_slice(&key.to_le_bytes());
            o[v..v + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(o)
    }
}

/// A built element: the wire payload plus the decode that proves it.
#[derive(Debug, Clone)]
pub struct BuiltItem {
    pub payload: Vec<u8>,
    pub bits: u32,
    pub decoded: Decoded,
}

/// Encode through the **client's own encoder**, decode it back, and require the C object identical.
pub fn build(oracle: &mut ObjectOracle, spec: &ItemSpec) -> Result<BuiltItem> {
    if oracle.tag() != TAG {
        return Err(OracleError::Shape(format!(
            "oracle is for tag {:#010x}, not the inventory-element tag {TAG:#010x}",
            oracle.tag()
        )));
    }
    let object = spec.to_object()?;
    let (enc, dec) = oracle.round_trip(&object, 0x4000)?;
    verify(spec, &dec)?;
    Ok(BuiltItem { payload: enc.payload, bits: enc.bits, decoded: dec })
}

/// Assert every value came back **at the offset it was written to**, out of the client's decoder.
pub fn verify(spec: &ItemSpec, dec: &Decoded) -> Result<()> {
    let u16_at = |o: usize| u16::from_le_bytes(dec.object[o..o + 2].try_into().unwrap_or([0; 2]));
    let u32_at = |o: usize| u32::from_le_bytes(dec.object[o..o + 4].try_into().unwrap_or([0; 4]));
    let bad = |m: String| Err(OracleError::Shape(m));

    if dec.object.len() != STRUCT_SIZE {
        return bad(format!("decoded {} bytes, expected {STRUCT_SIZE}", dec.object.len()));
    }
    if dec.qword(OFF_SOID) != spec.soid {
        return bad(format!("+{OFF_SOID:#04x} SOID is {:#x}, expected {:#x}", dec.qword(OFF_SOID), spec.soid));
    }
    if u16_at(OFF_CONTENT_INDEX) != spec.content_index {
        return bad(format!(
            "+{OFF_CONTENT_INDEX:#04x} content index is {:#06x}, expected {:#06x} - this is the \
             argument of the definition lookup and a wrong one is SILENT",
            u16_at(OFF_CONTENT_INDEX),
            spec.content_index
        ));
    }
    if u32_at(OFF_LEVEL) as i32 != spec.level {
        return bad(format!("+{OFF_LEVEL:#04x} level is {}, expected {}", u32_at(OFF_LEVEL) as i32, spec.level));
    }
    if u32_at(OFF_EQUIP_MIN_STAT) != spec.equip_min_stat {
        return bad(format!(
            "+{OFF_EQUIP_MIN_STAT:#04x} equip minimum is {}, expected {}",
            u32_at(OFF_EQUIP_MIN_STAT),
            spec.equip_min_stat
        ));
    }
    for (i, v) in spec.extra_content_indices.iter().enumerate() {
        let at = OFF_EXTRA_INDICES + i * 2;
        if u16_at(at) != *v {
            return bad(format!(
                "+{at:#04x} extra content index [{i}] is {:#06x}, expected {v:#06x} - `0x555730`'s \
                 tail is a FIXED 8-iteration loop, so every one of the eight matters",
                u16_at(at)
            ));
        }
    }
    if u16_at(OFF_PROGRESSION_ORDINAL) != spec.progression_ordinal {
        return bad(format!(
            "+{OFF_PROGRESSION_ORDINAL:#04x} progression ordinal is {:#06x}, expected {:#06x}",
            u16_at(OFF_PROGRESSION_ORDINAL),
            spec.progression_ordinal
        ));
    }
    if u16_at(OFF_TALENT_GRID) != spec.talent_grid {
        return bad(format!(
            "+{OFF_TALENT_GRID:#04x} talent grid is {:#06x}, expected {:#06x} - `FUN_00283A30` \
             returns 'none' only for -1, so 0 selects grid #0",
            u16_at(OFF_TALENT_GRID),
            spec.talent_grid
        ));
    }
    for (i, (key, value)) in spec.stats.iter().enumerate() {
        let k = OFF_STAT_KEYS + i * 2;
        let v = OFF_STAT_VALUES + i * 4;
        if u16_at(k) != *key || u32_at(v) != *value {
            return bad(format!(
                "stat[{i}] is {{{}, {}}}, expected {{{key}, {value}}}",
                u16_at(k),
                u32_at(v)
            ));
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Reading the staged directory back
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// A staged element on disk, decoded through the client's own decoder, with the bucket its filename
/// claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedElement {
    pub file: std::path::PathBuf,
    /// The filename's `NN-<bucket>.bin` suffix - the only record of which bucket the item is for.
    pub bucket: String,
    pub soid: u64,
    pub item_hash: u32,
    pub level: i32,
    pub equip_min_stat: u32,
    pub content_index: u16,
    pub progression_ordinal: u16,
    pub talent_grid: u16,
}

/// `NN-Heavy Weapons.bin` → `"Heavy Weapons"`. A name with no leading `NN-` is used verbatim.
pub fn bucket_from_filename(p: &Path) -> String {
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    match stem.split_once('-') {
        Some((n, rest)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
        _ => stem,
    }
}

/// Decode every `*.bin` in `dir`, in filename order.
pub fn read_staged(dir: &Path) -> Result<Vec<StagedElement>> {
    let mut files: Vec<std::path::PathBuf> = if dir.is_dir() {
        std::fs::read_dir(dir)
            .map_err(|e| OracleError::Eboot(format!("{}: {e}", dir.display())))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "bin").unwrap_or(false))
            .collect()
    } else {
        vec![dir.to_path_buf()]
    };
    files.sort();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut o = ObjectOracle::new(TAG)?;
    let mut out = Vec::new();
    for f in &files {
        let raw = std::fs::read(f).map_err(|e| OracleError::Eboot(format!("{}: {e}", f.display())))?;
        let d = o.decode(&raw)?;
        let u16_at = |x: usize| u16::from_le_bytes(d.object[x..x + 2].try_into().unwrap());
        let u32_at = |x: usize| u32::from_le_bytes(d.object[x..x + 4].try_into().unwrap());
        out.push(StagedElement {
            file: f.clone(),
            bucket: bucket_from_filename(f),
            soid: d.qword(OFF_SOID),
            // ⚠ Only meaningful on a legacy element, where +0x18 held the hash. A rebuilt element
            // carries a LEVEL here; `rebuild` therefore reads the hash before it overwrites it.
            item_hash: u32_at(OFF_LEVEL),
            level: u32_at(OFF_LEVEL) as i32,
            equip_min_stat: u32_at(OFF_EQUIP_MIN_STAT),
            content_index: u16_at(OFF_CONTENT_INDEX),
            progression_ordinal: u16_at(OFF_PROGRESSION_ORDINAL),
            talent_grid: u16_at(OFF_TALENT_GRID),
        });
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub use crate::profile::Control;

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn no(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: false, detail: detail.into() }
}

/// Every control this module ships. **Exit 2 if any `pass` is false.**
///
/// Four of them are **negative** - they assert that a value this project actually shipped is now
/// *refused*. Each was watched failing (by deleting the guard) before it was allowed to ship;
/// a control nobody has seen fail is decoration.
pub fn controls(table: Option<&IndexTable>) -> Vec<Control> {
    let mut v = Vec::new();

    // ── the layout is re-derived, never transcribed ──────────────────────────────────────────────
    match (Image::open_default(), Image::open_default().and_then(|i| descriptors::index(&i))) {
        (Ok(image), Ok(idx)) => match derive_layout(&image, &idx) {
            Ok(d) => {
                let want = declared_layout();
                if d == want {
                    v.push(ok(
                        "layout-from-eboot",
                        format!(
                            "{TAG:#010x} {} B: level +{:#04x}, equip-min +{:#04x}, content index \
                             +{:#04x}, extra {}×u16 @+{:#04x}, prog ordinal +{:#04x}, grid +{:#04x}, \
                             stats {}×(u16,u32) @+{:#04x}/+{:#04x}",
                            d.struct_size, d.level, d.equip_min_stat, d.content_index,
                            EXTRA_INDEX_SLOTS, d.extra_indices, d.progression_ordinal,
                            d.talent_grid, STAT_SLOTS, d.stat_keys, d.stat_values
                        ),
                    ));
                } else {
                    v.push(no("layout-from-eboot", format!("eboot says {d:?}, code says {want:?}")));
                }
            }
            Err(e) => v.push(no("layout-from-eboot", e.to_string())),
        },
        _ => v.push(no("layout-from-eboot", "could not open the pinned eboot")),
    }

    // ── the round trip, through the client's own codec ───────────────────────────────────────────
    let demo = ItemSpec {
        soid: 0xD100_0000_0000_0301,
        content_index: 2085,
        level: 40,
        equip_min_stat: 0,
        curve_offset: 0,
        extra_content_indices: [EMPTY_INDEX; EXTRA_INDEX_SLOTS],
        progress_points: 0,
        progression_ordinal: EMPTY_INDEX,
        talent_grid: EMPTY_INDEX,
        stats: Vec::new(),
        state_bits: 1,
        flag_86: 1,
        bucket: "Heavy Weapons".into(),
        item_hash: 0x4BF4_BE3F,
    };
    match ObjectOracle::new(TAG).and_then(|mut o| build(&mut o, &demo)) {
        Ok(b) => v.push(ok(
            "round-trip",
            format!("{} B / {} bits, object IDENTICAL through 0x2C9000/0x2C8FF0", b.payload.len(), b.bits),
        )),
        Err(e) => v.push(no("round-trip", e.to_string())),
    }

    // ── ⭐ NEGATIVE: the two hash-in-a-small-field values we actually shipped must be REFUSED ─────
    let broken = ItemSpec::legacy_broken(0xD100_0000_0000_0304, 3_564_229_425, 953_998_645);
    match broken.to_object() {
        Err(e) => v.push(ok(
            "NEGATIVE-legacy-element-refused",
            format!("the element `stage_loadout` shipped is refused: {e}"),
        )),
        Ok(_) => v.push(no(
            "NEGATIVE-legacy-element-refused",
            "the pre-2026-08-10 element was ACCEPTED - the guard has stopped testing anything",
        )),
    }
    let mut hash_level = demo.clone();
    hash_level.level = 3_564_229_425u32 as i32;
    match hash_level.to_object() {
        Err(_) => v.push(ok("NEGATIVE-hash-as-level-refused", "+0x18 = 3564229425 is refused")),
        Ok(_) => v.push(no("NEGATIVE-hash-as-level-refused", "a content hash was accepted as a LEVEL")),
    }
    let mut hash_min = demo.clone();
    hash_min.equip_min_stat = 953_998_645;
    match hash_min.to_object() {
        Err(_) => v.push(ok(
            "NEGATIVE-bucket-hash-as-equip-min-refused",
            "+0x1C = 953998645 (Heavy Weapons bucketTypeHash) is refused - this is the padlock",
        )),
        Ok(_) => v.push(no(
            "NEGATIVE-bucket-hash-as-equip-min-refused",
            "a bucketTypeHash was accepted as a minimum required stat",
        )),
    }

    // ── ⭐ NEGATIVE: an unknown item hash must not silently become an index ──────────────────────
    match table {
        Some(t) => match t.require(0xDEAD_BEEF) {
            Err(_) => v.push(ok(
                "NEGATIVE-unknown-hash-refused",
                "IndexTable::require(0xDEADBEEF) refuses - a hash the content does not know cannot \
                 become a content index",
            )),
            Ok(i) => v.push(no(
                "NEGATIVE-unknown-hash-refused",
                format!("0xDEADBEEF resolved to index {i} - require() is not discriminating"),
            )),
        },
        None => v.push(no("NEGATIVE-unknown-hash-refused", "no content-index table loaded")),
    }

    // ── the bucket table, and it now carries its own tiling proof ───────────────────────────────
    let mut overlap = None;
    for (i, a) in BUCKET_TABLE.iter().enumerate() {
        for b in BUCKET_TABLE.iter().skip(i + 1) {
            if a.scope == b.scope
                && a.first_slot < b.first_slot + b.slot_count
                && b.first_slot < a.first_slot + a.slot_count
            {
                overlap = Some((a.id, b.id));
            }
        }
    }
    let mut gaps: Vec<(u8, u16, u16)> = Vec::new();
    for scope in [0u8, 1] {
        let mut rows: Vec<&BucketRecord> =
            BUCKET_TABLE.iter().filter(|r| r.scope == scope).collect();
        rows.sort_by_key(|r| r.first_slot);
        let mut cur = 0u16;
        for r in rows {
            if r.first_slot != cur {
                gaps.push((scope, cur, r.first_slot));
            }
            cur = r.first_slot + r.slot_count;
        }
    }
    let ids: std::collections::BTreeSet<u8> = BUCKET_TABLE.iter().map(|r| r.id).collect();
    match (overlap, gaps.is_empty(), ids.len() == BUCKET_TABLE.len()) {
        (None, true, true) => v.push(ok(
            "bucket-table-tiles",
            format!(
                "{} records, {} distinct ids ({:#04x}..={:#04x}); both scopes tile with ZERO gaps \
                 and ZERO overlaps - scope 0 to slot {}, scope 1 to slot {}",
                BUCKET_TABLE.len(),
                ids.len(),
                ids.iter().next().copied().unwrap_or(0),
                ids.iter().next_back().copied().unwrap_or(0),
                BUCKET_TABLE.iter().filter(|r| r.scope == 0)
                    .map(|r| r.first_slot + r.slot_count).max().unwrap_or(0),
                BUCKET_TABLE.iter().filter(|r| r.scope == 1)
                    .map(|r| r.first_slot + r.slot_count).max().unwrap_or(0),
            ),
        )),
        (o, g, d) => v.push(no(
            "bucket-table-tiles",
            format!("overlap {o:?}, gaps {gaps:?} (none expected: {g}), ids distinct: {d}"),
        )),
    }

    // ⭐ NEGATIVE: the placement we shipped must now be visible as wrong.
    match (slot_for_bucket("Heavy Weapons", 0), bucket_for_slot(3), bucket_for_slot(322)) {
        (Some(s), Some(MATERIALS_ID), None) if (113..123).contains(&s) => v.push(ok(
            "NEGATIVE-old-placement-is-materials",
            format!(
                "Heavy Weapons[0] is slot {s} (113-122); the old builder's slot 3 is bucket \
                 {MATERIALS_ID:#04x} MATERIALS; and slot 322 - one past the last range - is the \
                 leaf's 0xFF NOT-FOUND, so this lookup discriminates"
            ),
        )),
        (s, b, past) => v.push(no(
            "NEGATIVE-old-placement-is-materials",
            format!("Heavy Weapons[0] = {s:?}, slot 3 = {b:?}, slot 322 = {past:?}"),
        )),
    }
    match (slot_for_bucket("Artifacts", 0), slot_for_bucket("Sparrow Horn", 0)) {
        (None, None) => v.push(ok(
            "undetermined-bucket-returns-none",
            "Artifacts and Sparrow Horn have no determined bucket id - they are two of the five \
             equippable count-10 buckets 0x19/0x1B/0x1C/0x1E/0x24 and item counts cannot \
             discriminate five buckets that all hold ten. slot_for_bucket refuses rather than \
             guessing one in five",
        )),
        (a, b) => v.push(no(
            "undetermined-bucket-returns-none",
            format!("Artifacts -> {a:?}, Sparrow Horn -> {b:?} - a bucket id was invented"),
        )),
    }

    v
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Layout, re-derived from the eboot rather than transcribed
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub struct_size: u32,
    pub soid: usize,
    pub state_bits: usize,
    pub level: usize,
    pub equip_min_stat: usize,
    pub curve_offset: usize,
    pub content_index: usize,
    pub extra_indices: usize,
    pub extra_index_count: u32,
    pub progress_points: usize,
    pub progression_ordinal: usize,
    pub talent_grid: usize,
    pub node_states: usize,
    pub node_steps: usize,
    pub flag_86: usize,
    pub stat_keys: usize,
    pub stat_key_count: u32,
    pub stat_values: usize,
    pub stat_value_count: u32,
}

fn desc<'a>(idx: &'a Index, tag: u32) -> Result<&'a Descriptor> {
    idx.get(tag).ok_or(OracleError::UnknownTag(tag))
}

fn field(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<Field> {
    let d = desc(idx, tag)?;
    descriptors::fields(image, d.anchor, d.field_count)
        .get(i)
        .and_then(|(_, f)| *f)
        .ok_or_else(|| OracleError::Shape(format!("{tag:#010x} has no field [{i}]")))
}

fn child(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<(Field, u32)> {
    let f = field(image, idx, tag, i)?;
    if f.child_tag == LEAF {
        return Err(OracleError::Shape(format!("{tag:#010x} field [{i}] is a leaf, expected a struct")));
    }
    Ok((f, f.child_tag))
}

/// ⭐ The **array-count rule**, applied rather than assumed: a descriptor is an inline array iff it
/// has exactly one field whose `offset == the element size` and `container % offset == 0`; then
/// `count = container / offset` and element 0 sits at `+0`.
/// (`historicaldocs/design/inventory-instance-fields.md` §1.1 - the `kind`-bit version of this rule was refuted.)
fn inline_array(image: &Image, idx: &Index, tag: u32) -> Result<u32> {
    let d = desc(idx, tag)?;
    if d.field_count != 1 {
        return Err(OracleError::Shape(format!(
            "{tag:#010x} has {} fields; an inline array has exactly one",
            d.field_count
        )));
    }
    let f = field(image, idx, tag, 0)?;
    let stride = f.offset as u32;
    if stride == 0 || d.struct_size % stride != 0 {
        return Err(OracleError::Shape(format!(
            "{tag:#010x}: stride {stride} does not tile {} bytes",
            d.struct_size
        )));
    }
    let _ = image;
    Ok(d.struct_size / stride)
}

/// Walk `0x80801732`'s tree and produce the offsets this module writes to.
///
/// **This is the control on every constant above.** Nothing is transcribed; if the eboot disagrees,
/// [`controls`] fails and the tool exits 2.
pub fn derive_layout(image: &Image, idx: &Index) -> Result<Layout> {
    let root = desc(idx, TAG)?.clone();
    if root.struct_size as usize != STRUCT_SIZE {
        return Err(OracleError::Shape(format!(
            "{TAG:#010x} is {} bytes, code says {STRUCT_SIZE}",
            root.struct_size
        )));
    }

    let soid = field(image, idx, TAG, 0)?;
    if soid.kind != 0x106 {
        return Err(OracleError::Shape(format!(
            "{TAG:#010x} field [0] is kind {:#x}, expected the 0x106 u64 registry key",
            soid.kind
        )));
    }

    // [1] +0x08 -> 0x80801920 {u64 @+0, 2-bit @+8}
    let (bits_f, bits_tag) = child(image, idx, TAG, 1)?;
    let state_bits = bits_f.offset as usize + field(image, idx, bits_tag, 1)?.offset as usize;

    // [2] +0x18 -> 0x80801711 {u32, u32, u16}
    let (lvl_f, lvl_tag) = child(image, idx, TAG, 2)?;
    let l0 = field(image, idx, lvl_tag, 0)?;
    let l1 = field(image, idx, lvl_tag, 1)?;
    let l2 = field(image, idx, lvl_tag, 2)?;
    if l0.kind != 0x105 || l1.kind != 0x105 || l2.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{lvl_tag:#010x} fields are kinds {:#x}/{:#x}/{:#x}, expected u32/u32/u16 - \
             `0x26F492` reads element+0x18 as an i32 and `0x269971` compares element+0x1C",
            l0.kind, l1.kind, l2.kind
        )));
    }

    // [3] +0x24 leaf u16 - the content index.
    let ci = field(image, idx, TAG, 3)?;
    if ci.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{TAG:#010x} field [3] is kind {:#x}, expected the 0x104 u16 content index",
            ci.kind
        )));
    }

    // [4] +0x26 -> 0x80801018, an 8 × u16 inline array.
    let (extra_f, extra_tag) = child(image, idx, TAG, 4)?;
    let extra_count = inline_array(image, idx, extra_tag)?;

    // [5] +0x38 -> 0x80801846 (80 B) - the progression / talent block.
    let (blk_f, blk_tag) = child(image, idx, TAG, 5)?;
    let blk = blk_f.offset as usize;
    let (prog_f, prog_tag) = child(image, idx, blk_tag, 0)?;
    let p0 = field(image, idx, prog_tag, 0)?;
    let p3 = field(image, idx, prog_tag, 3)?;
    if p0.kind != 0x105 || p3.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{prog_tag:#010x} fields [0]/[3] are kinds {:#x}/{:#x}, expected u32 XP + u16 ordinal",
            p0.kind, p3.kind
        )));
    }
    let grid_f = field(image, idx, blk_tag, 2)?;
    if grid_f.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{blk_tag:#010x} field [2] is kind {:#x}, expected the 0x104 u16 talent-grid index",
            grid_f.kind
        )));
    }
    let (nodes_f, _) = child(image, idx, blk_tag, 3)?;
    let (steps_f, _) = child(image, idx, blk_tag, 4)?;
    let flag_f = field(image, idx, blk_tag, 5)?;

    // [6] +0x88 -> 0x80805A08, 8 × u16 stat keys.  [7] +0x98 -> 0x80804EA5, 8 × u32 stat values.
    let (keys_f, keys_tag) = child(image, idx, TAG, 6)?;
    let key_count = inline_array(image, idx, keys_tag)?;
    let (vals_f, vals_tag) = child(image, idx, TAG, 7)?;
    let val_count = inline_array(image, idx, vals_tag)?;

    Ok(Layout {
        struct_size: root.struct_size,
        soid: soid.offset as usize,
        state_bits,
        level: lvl_f.offset as usize + l0.offset as usize,
        equip_min_stat: lvl_f.offset as usize + l1.offset as usize,
        curve_offset: lvl_f.offset as usize + l2.offset as usize,
        content_index: ci.offset as usize,
        extra_indices: extra_f.offset as usize,
        extra_index_count: extra_count,
        progress_points: blk + prog_f.offset as usize + p0.offset as usize,
        progression_ordinal: blk + prog_f.offset as usize + p3.offset as usize,
        talent_grid: blk + grid_f.offset as usize,
        node_states: blk + nodes_f.offset as usize,
        node_steps: blk + steps_f.offset as usize,
        flag_86: blk + flag_f.offset as usize,
        stat_keys: keys_f.offset as usize,
        stat_key_count: key_count,
        stat_values: vals_f.offset as usize,
        stat_value_count: val_count,
    })
}

/// The layout this module's constants declare - what [`derive_layout`] must reproduce.
pub fn declared_layout() -> Layout {
    Layout {
        struct_size: STRUCT_SIZE as u32,
        soid: OFF_SOID,
        state_bits: OFF_STATE_BITS,
        level: OFF_LEVEL,
        equip_min_stat: OFF_EQUIP_MIN_STAT,
        curve_offset: OFF_CURVE_OFFSET,
        content_index: OFF_CONTENT_INDEX,
        extra_indices: OFF_EXTRA_INDICES,
        extra_index_count: EXTRA_INDEX_SLOTS as u32,
        progress_points: OFF_PROGRESS_POINTS,
        progression_ordinal: OFF_PROGRESSION_ORDINAL,
        talent_grid: OFF_TALENT_GRID,
        node_states: OFF_NODE_STATES,
        node_steps: OFF_NODE_STEPS,
        flag_86: OFF_FLAG_86,
        stat_keys: OFF_STAT_KEYS,
        stat_key_count: STAT_SLOTS as u32,
        stat_values: OFF_STAT_VALUES,
        stat_value_count: STAT_SLOTS as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_in_a_level_field_is_refused() {
        let s = ItemSpec::legacy_broken(1, 3_564_229_425, 953_998_645);
        assert!(s.to_object().is_err(), "the element stage_loadout shipped must be refused");
    }

    #[test]
    fn bucket_table_tiles_and_placement_leaves_materials() {
        assert_eq!(BUCKET_TABLE.len(), 39, "the leaf scans exactly 39 records");
        assert_eq!(slot_for_bucket("Heavy Weapons", 0), Some(113));
        assert_eq!(slot_for_bucket("Helmet", 0), Some(43));
        assert_eq!(bucket_for_slot(3), Some(MATERIALS_ID));
        assert_eq!(bucket_for_slot(322), None, "one past the last range must be NOT FOUND");
        assert_eq!(bucket_for_slot(137), Some(0x0F), "slots 133-142 are bucket 0x0F");
        assert_eq!(slot_for_bucket("Artifacts", 0), None, "an undetermined bucket must not be invented");
        for (name, _) in BUCKET_IDS.iter().filter(|(n, _)| *n != "Materials" && *n != "Consumables") {
            let s = slot_for_bucket(name, 0).unwrap_or_else(|| panic!("{name} has no slot"));
            assert_ne!(bucket_for_slot(s), Some(MATERIALS_ID), "{name} landed in Materials");
        }
    }

    #[test]
    fn filename_gives_the_bucket() {
        assert_eq!(bucket_from_filename(Path::new("03-Heavy Weapons.bin")), "Heavy Weapons");
        assert_eq!(bucket_from_filename(Path::new("Ghost.bin")), "Ghost");
    }
}
