// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The `Loadout` document: a character and what it has equipped.
//!
//! Everything here is plain data with a versioned on-disk form. See the crate docs for why there is
//! no encoding in this crate at all.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::era::Era;

/// On-disk schema version for a [`Loadout`] document.
///
/// Bump on **every** breaking change, no exceptions - the same rule `Envelope` follows in
/// `historicaldocs/design/interface-architecture.md` §2. A reader refuses a document from the future rather than
/// half-understanding it; a document from the past is migrated or rejected explicitly.
pub const SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------------------------
// Character identity
// ---------------------------------------------------------------------------------------------

/// Guardian class.
///
/// The discriminants are Bungie's `classType`, and [`Class::hash`] is the manifest `classHash`.
/// Both read out of `DestinyClassDefinition` in the preserved world manifest [C, 3 rows].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Titan = 0,
    Hunter = 1,
    Warlock = 2,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Titan, Class::Hunter, Class::Warlock];

    /// `DestinyClassDefinition.classHash`.
    pub fn hash(self) -> u32 {
        match self {
            Class::Titan => 3_655_393_761,
            Class::Hunter => 671_679_327,
            Class::Warlock => 2_271_682_572,
        }
    }

    /// `DestinyInventoryItemDefinition.classType`, which is the same enum.
    pub fn class_type(self) -> u32 {
        self as u32
    }

    pub fn from_class_type(v: u32) -> Option<Class> {
        match v {
            0 => Some(Class::Titan),
            1 => Some(Class::Hunter),
            2 => Some(Class::Warlock),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Class::Titan => "Titan",
            Class::Hunter => "Hunter",
            Class::Warlock => "Warlock",
        }
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `classType == 3` in the manifest means "any class" - an unrestricted item.
///
/// It is *not* a fourth class, and treating it as one is the obvious way to write a validator that
/// rejects every weapon in the game.
pub const CLASS_TYPE_ANY: u32 = 3;

/// Guardian race. Discriminants are `raceType`; [`Race::hash`] is `raceHash` [C, 3 rows].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Race {
    Human = 0,
    Awoken = 1,
    Exo = 2,
}

impl Race {
    pub const ALL: [Race; 3] = [Race::Human, Race::Awoken, Race::Exo];

    pub fn hash(self) -> u32 {
        match self {
            Race::Human => 3_887_404_748,
            Race::Awoken => 2_803_282_938,
            Race::Exo => 898_834_093,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Awoken => "Awoken",
            Race::Exo => "Exo",
        }
    }
}

impl fmt::Display for Race {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Guardian gender. Discriminants are `genderType`; [`Gender::hash`] is `genderHash` [C, 2 rows].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male = 0,
    Female = 1,
}

impl Gender {
    pub const ALL: [Gender; 2] = [Gender::Male, Gender::Female];

    pub fn hash(self) -> u32 {
        match self {
            Gender::Male => 3_111_576_190,
            Gender::Female => 2_204_441_813,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Gender::Male => "Male",
            Gender::Female => "Female",
        }
    }
}

impl fmt::Display for Gender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------------------------

/// One equip slot on a character.
///
/// These are exactly the `DestinyInventoryBucketDefinition` rows with `category == 3` (Equippable)
/// and `enabled == true` [C, 17 rows in the preserved manifest]. The `category` field is the reason
/// this list is not guesswork: vault and consumable buckets are categories 1 and 2 and are not
/// equip destinations, so a slot enum built from item `bucketTypeHash` values alone would silently
/// admit Materials.
///
/// Ordering follows the manifest's own `bucketOrder`, so a UI that iterates [`Slot::ALL`] lays the
/// character sheet out the way the game does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Subclass,
    Primary,
    Special,
    Heavy,
    Ghost,
    Head,
    Arms,
    Chest,
    Legs,
    ClassItem,
    Artifact,
    Vehicle,
    Horn,
    Ship,
    Shader,
    Emblem,
    Emote,
}

impl Slot {
    /// In `bucketOrder`, which is also the order the character sheet reads top to bottom.
    pub const ALL: [Slot; 17] = [
        Slot::Subclass,
        Slot::Primary,
        Slot::Special,
        Slot::Heavy,
        Slot::Ghost,
        Slot::Head,
        Slot::Arms,
        Slot::Chest,
        Slot::Legs,
        Slot::ClassItem,
        Slot::Artifact,
        Slot::Vehicle,
        Slot::Horn,
        Slot::Ship,
        Slot::Shader,
        Slot::Emblem,
        Slot::Emote,
    ];

    /// The slots that carry combat consequence - the ones a 3v3 private match actually depends on.
    /// Cosmetics are excluded, not because they do not exist but because they are not the gate.
    pub const COMBAT: [Slot; 11] = [
        Slot::Subclass,
        Slot::Primary,
        Slot::Special,
        Slot::Heavy,
        Slot::Ghost,
        Slot::Head,
        Slot::Arms,
        Slot::Chest,
        Slot::Legs,
        Slot::ClassItem,
        Slot::Artifact,
    ];

    /// `DestinyInventoryBucketDefinition.bucketHash`, which is also an item's `bucketTypeHash`.
    pub fn bucket_hash(self) -> u32 {
        match self {
            Slot::Subclass => 3_284_755_031,
            Slot::Primary => 1_498_876_634,
            Slot::Special => 2_465_295_065,
            Slot::Heavy => 953_998_645,
            Slot::Ghost => 4_023_194_814,
            Slot::Head => 3_448_274_439,
            Slot::Arms => 3_551_918_588,
            Slot::Chest => 14_239_492,
            Slot::Legs => 20_886_954,
            Slot::ClassItem => 1_585_787_867,
            Slot::Artifact => 434_908_299,
            Slot::Vehicle => 2_025_709_351,
            Slot::Horn => 3_796_357_825,
            Slot::Ship => 284_967_655,
            Slot::Shader => 2_973_005_342,
            Slot::Emblem => 4_274_335_291,
            Slot::Emote => 3_054_419_239,
        }
    }

    pub fn from_bucket_hash(h: u32) -> Option<Slot> {
        Slot::ALL.into_iter().find(|s| s.bucket_hash() == h)
    }

    /// The manifest's own `bucketName`, so labels in a UI match what the game shows.
    pub fn display_name(self) -> &'static str {
        match self {
            Slot::Subclass => "Subclass",
            Slot::Primary => "Primary Weapons",
            Slot::Special => "Special Weapons",
            Slot::Heavy => "Heavy Weapons",
            Slot::Ghost => "Ghost",
            Slot::Head => "Helmet",
            Slot::Arms => "Gauntlets",
            Slot::Chest => "Chest Armor",
            Slot::Legs => "Leg Armor",
            Slot::ClassItem => "Class Armor",
            Slot::Artifact => "Artifacts",
            Slot::Vehicle => "Vehicle",
            Slot::Horn => "Sparrow Horn",
            Slot::Ship => "Ships",
            Slot::Shader => "Shaders",
            Slot::Emblem => "Emblems",
            Slot::Emote => "Emotes",
        }
    }

    /// Armour slots are the ones where an item may be class-restricted.
    pub fn is_armor(self) -> bool {
        matches!(
            self,
            Slot::Head | Slot::Arms | Slot::Chest | Slot::Legs | Slot::ClassItem
        )
    }

    pub fn is_weapon(self) -> bool {
        matches!(self, Slot::Primary | Slot::Special | Slot::Heavy)
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Only for `#[serde(default)]`; a `Slot` is never meaningfully absent, so the choice is arbitrary
/// and any code relying on it is wrong.
impl Default for Slot {
    fn default() -> Self {
        Slot::Primary
    }
}

// ---------------------------------------------------------------------------------------------
// Item reference
// ---------------------------------------------------------------------------------------------

/// Rarity. Discriminants are Bungie's `tierType` [C, observed values 0..=6].
///
/// `tierType == 1` also renders as "Basic" in the manifest; it is kept distinct rather than folded
/// into 0 because the manifest distinguishes them and this crate does not get to decide it knows
/// better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Unknown = 0,
    Currency = 1,
    Common = 2,
    Uncommon = 3,
    Rare = 4,
    Legendary = 5,
    Exotic = 6,
}

impl Tier {
    pub const ALL: [Tier; 7] = [
        Tier::Unknown,
        Tier::Currency,
        Tier::Common,
        Tier::Uncommon,
        Tier::Rare,
        Tier::Legendary,
        Tier::Exotic,
    ];

    pub fn from_tier_type(v: u32) -> Tier {
        match v {
            1 => Tier::Currency,
            2 => Tier::Common,
            3 => Tier::Uncommon,
            4 => Tier::Rare,
            5 => Tier::Legendary,
            6 => Tier::Exotic,
            _ => Tier::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tier::Unknown => "Basic",
            Tier::Currency => "Currency",
            Tier::Common => "Common",
            Tier::Uncommon => "Uncommon",
            Tier::Rare => "Rare",
            Tier::Legendary => "Legendary",
            Tier::Exotic => "Exotic",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Needed by `#[serde(default)]` on the structs that hold a `Tier`. The default is the *absence* of
/// information, matching `tierType == 0`.
impl Default for Tier {
    fn default() -> Self {
        Tier::Unknown
    }
}

/// Damage type, from `DestinyDamageTypeDefinition.enumValue` [C, 4 rows].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Damage {
    None = 0,
    Kinetic = 1,
    Arc = 2,
    Solar = 3,
    Void = 4,
}

impl Damage {
    pub fn from_enum_value(v: u32) -> Damage {
        match v {
            1 => Damage::Kinetic,
            2 => Damage::Arc,
            3 => Damage::Solar,
            4 => Damage::Void,
            _ => Damage::None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Damage::None => "-",
            Damage::Kinetic => "Kinetic",
            Damage::Arc => "Arc",
            Damage::Solar => "Solar",
            Damage::Void => "Void",
        }
    }
}

impl fmt::Display for Damage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Default for Damage {
    fn default() -> Self {
        Damage::None
    }
}

/// A reference to a manifest item, with just enough display state cached that the UI never needs the
/// database open.
///
/// **`hash` is the only load-bearing field.** Everything else is a copy of what the 2020 manifest
/// said at extraction time, kept so a saved loadout still reads sensibly on a box with no manifest,
/// and so the file is reviewable as text. If a cached field ever disagrees with the manifest, the
/// manifest wins and the cache is stale - [`crate::Catalogue::refresh_ref`] is how you resolve it.
///
/// It is `[U]` whether `hash` is even the identifier the client's inventory objects carry. See the
/// crate docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemRef {
    /// `DestinyInventoryItemDefinition.itemHash`.
    pub hash: u32,
    /// `itemName` at extraction time. Display only.
    pub name: String,
    pub tier: Tier,
    /// The item's own `bucketTypeHash`. Compared against the slot it is placed in.
    pub bucket_hash: u32,
    /// `itemTypeName`, e.g. "Hand Cannon", "Chest Armor".
    pub item_type_name: String,
    /// `icon`, a path *relative to `https://www.bungie.net`*. Nothing here fetches it - the lab is
    /// sealed and the emulator box must never reach the internet.
    pub icon: String,
    /// `classType`: 0/1/2 restrict to a class, [`CLASS_TYPE_ANY`] (3) means unrestricted.
    pub class_type: u32,
    pub damage: Damage,
    /// The era verdict recorded when this ref was created. See [`crate::era`] - the honest default
    /// is [`Era::Unknown`], and it is recorded rather than recomputed so a hand-written loadout can
    /// state a human's judgement and keep it.
    pub era: Era,
    /// Free text: *why* this item carries the era verdict it does. Written by hand in the shipped
    /// loadouts; the extractor fills it from the reward sources it matched.
    pub era_note: String,
}

impl Default for ItemRef {
    fn default() -> Self {
        ItemRef {
            hash: 0,
            name: String::new(),
            tier: Tier::Unknown,
            bucket_hash: 0,
            item_type_name: String::new(),
            icon: String::new(),
            class_type: CLASS_TYPE_ANY,
            damage: Damage::None,
            era: Era::Unknown,
            era_note: String::new(),
        }
    }
}

impl ItemRef {
    /// True when this item may be equipped by `class`.
    pub fn allows_class(&self, class: Class) -> bool {
        self.class_type == CLASS_TYPE_ANY || self.class_type == class.class_type()
    }
}

// ---------------------------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------------------------

/// One item placed in one slot.
///
/// This is a `Vec` element rather than a `BTreeMap` entry on purpose: a map would make "two items in
/// one bucket" *unrepresentable*, which sounds like a win until you remember these files are
/// hand-edited and the validator's job is to catch exactly that. A structurally impossible error is
/// an error the validator can never report - it becomes a silent key overwrite instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipped {
    pub slot: Slot,
    pub item: ItemRef,
}

/// A character and its equipment.
///
/// `#[serde(default)]` on the whole struct plus `deny_unknown_fields` deliberately **absent**: the
/// wire/inventory representation is `[U]` and this format will grow fields. A file written by a
/// later revision of the same `schema_version` must still load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loadout {
    /// Always written as [`SCHEMA_VERSION`]. Refused on load if greater.
    pub schema_version: u32,
    pub name: String,
    /// Free text: what this loadout is for, and what is known/unknown about it.
    pub notes: String,
    pub class: Class,
    pub race: Race,
    pub gender: Gender,
    /// Character level. 40 is the 01.29 (Rise of Iron) cap [I - from public patch history, not
    /// measured against our client].
    pub level: u32,
    /// Light. 400 is the Rise of Iron maximum [I, same caveat].
    pub light: u32,
    pub equipped: Vec<Equipped>,
}

impl Default for Loadout {
    fn default() -> Self {
        Loadout {
            schema_version: SCHEMA_VERSION,
            name: "untitled".to_string(),
            notes: String::new(),
            class: Class::Titan,
            race: Race::Human,
            gender: Gender::Male,
            level: 40,
            light: 400,
            equipped: Vec::new(),
        }
    }
}

/// Level cap at build 01.29 [I]. `ERA-TARGETING.md` pins 01.29 to Bungie 2.4.1 (Rise of Iron).
pub const LEVEL_CAP_01_29: u32 = 40;
/// Light cap at build 01.29 [I]. Warning-only: we have never observed the client reject a value.
pub const LIGHT_CAP_01_29: u32 = 400;

impl Loadout {
    /// Parse a document, refusing one written by a schema we do not understand.
    ///
    /// A *newer* version is a hard error - silently ignoring fields we do not know about is how a
    /// loadout quietly loses its subclass. An *older* version is accepted only if a migration exists;
    /// today `1` is the first version, so there is nothing to migrate and nothing to pretend about.
    pub fn from_json(s: &str) -> anyhow::Result<Loadout> {
        // Peek at the version before committing to the full shape, so the error message is about
        // the version rather than about whichever field happened to move.
        #[derive(Deserialize)]
        struct VersionPeek {
            #[serde(default)]
            schema_version: u32,
        }
        let peek: VersionPeek = serde_json::from_str(s)?;
        if peek.schema_version > SCHEMA_VERSION {
            anyhow::bail!(
                "loadout schema_version {} is newer than this build understands ({SCHEMA_VERSION})",
                peek.schema_version
            );
        }
        let mut l: Loadout = serde_json::from_str(s)?;
        l.schema_version = SCHEMA_VERSION;
        Ok(l)
    }

    /// Serialise deterministically: slots in [`Slot::ALL`] order, pretty-printed.
    ///
    /// Determinism is the same requirement `d1-run` has - these files are checked into git and
    /// reviewed as diffs, so a save that reorders equipment would make every diff meaningless.
    pub fn to_json(&self) -> String {
        let mut out = self.clone();
        out.schema_version = SCHEMA_VERSION;
        out.equipped.sort_by_key(|e| {
            (
                Slot::ALL.iter().position(|s| *s == e.slot).unwrap_or(usize::MAX),
                e.item.hash,
            )
        });
        serde_json::to_string_pretty(&out).expect("Loadout is always serialisable")
    }

    pub fn get(&self, slot: Slot) -> Option<&ItemRef> {
        self.equipped.iter().find(|e| e.slot == slot).map(|e| &e.item)
    }

    /// Place `item` in `slot`, replacing whatever was there.
    pub fn set(&mut self, slot: Slot, item: ItemRef) {
        self.equipped.retain(|e| e.slot != slot);
        self.equipped.push(Equipped { slot, item });
    }

    pub fn clear(&mut self, slot: Slot) {
        self.equipped.retain(|e| e.slot != slot);
    }

    /// Slot → item, keeping only the **first** entry for a duplicated slot.
    ///
    /// Callers that care about correctness should run [`Loadout::validate`] first; this exists for
    /// display, where showing the first of two is better than showing neither.
    pub fn by_slot(&self) -> BTreeMap<Slot, &ItemRef> {
        let mut m = BTreeMap::new();
        for e in &self.equipped {
            m.entry(e.slot).or_insert(&e.item);
        }
        m
    }

    /// Every rule we can check without the client. Ordered deterministically.
    ///
    /// Nothing here is confirmed against the 01.29 client - these are *manifest-consistency* rules,
    /// not observed client rejections. When the client eventually rejects something we allowed, that
    /// is new evidence and belongs here with a `[C]` tag; until then this is `[I]`.
    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if self.name.trim().is_empty() {
            issues.push(ValidationIssue::error("empty_name", "loadout has no name"));
        }

        // One item per equip bucket. See `Equipped`'s docs for why this is even possible to violate.
        let mut seen: BTreeMap<Slot, usize> = BTreeMap::new();
        for e in &self.equipped {
            *seen.entry(e.slot).or_insert(0) += 1;
        }
        for (slot, n) in &seen {
            if *n > 1 {
                issues.push(ValidationIssue::error(
                    "duplicate_slot",
                    format!("{n} items in the {slot} slot; exactly one may be equipped"),
                ));
            }
        }

        for e in &self.equipped {
            let it = &e.item;
            let who = if it.name.is_empty() {
                format!("item {:#010x}", it.hash)
            } else {
                format!("\"{}\"", it.name)
            };

            if it.hash == 0 {
                issues.push(ValidationIssue::error(
                    "zero_hash",
                    format!("{} in {} has hash 0", who, e.slot),
                ));
            }

            // The item's own bucket must be the slot it sits in. This is the rule that catches a
            // Special weapon dropped into the Primary slot - the single most likely hand-edit error.
            if it.bucket_hash != 0 && it.bucket_hash != e.slot.bucket_hash() {
                let actual = Slot::from_bucket_hash(it.bucket_hash)
                    .map(|s| s.display_name().to_string())
                    .unwrap_or_else(|| format!("bucket {:#010x}", it.bucket_hash));
                issues.push(ValidationIssue::error(
                    "bucket_slot_mismatch",
                    format!("{} belongs in {} but is placed in {}", who, actual, e.slot),
                ));
            }

            // Class-restricted gear. Applies to armour *and* to subclasses, which are class-locked
            // in exactly the same way (`Bladedancer` is `classType == 1`).
            if !it.allows_class(self.class) {
                let owner = Class::from_class_type(it.class_type)
                    .map(|c| c.name().to_string())
                    .unwrap_or_else(|| format!("classType {}", it.class_type));
                issues.push(ValidationIssue::error(
                    "class_mismatch",
                    format!(
                        "{} is {}-only but this loadout is a {}",
                        who, owner, self.class
                    ),
                ));
            }

            if it.era == Era::PostEra {
                issues.push(ValidationIssue::warning(
                    "post_era_item",
                    format!(
                        "{} is sourced only from content released after build 01.29 ({})",
                        who, it.era_note
                    ),
                ));
            }
        }

        // The exotic limit: one exotic weapon and one exotic armour piece.
        //
        // [I] - this is the game's rule as it was played, not something we have observed the client
        // enforce, so it is a warning. If the client turns out to enforce it (or not to), that is
        // evidence and this comment should change with a `[C]`.
        let count_exotics = |slots: &[Slot]| -> usize {
            self.equipped
                .iter()
                .filter(|e| slots.contains(&e.slot) && e.item.tier == Tier::Exotic)
                .count()
        };
        let ex_weapons = count_exotics(&[Slot::Primary, Slot::Special, Slot::Heavy]);
        if ex_weapons > 1 {
            issues.push(ValidationIssue::warning(
                "too_many_exotic_weapons",
                format!("{ex_weapons} exotic weapons; the game allows one"),
            ));
        }
        let ex_armor = count_exotics(&[
            Slot::Head,
            Slot::Arms,
            Slot::Chest,
            Slot::Legs,
            Slot::ClassItem,
        ]);
        if ex_armor > 1 {
            issues.push(ValidationIssue::warning(
                "too_many_exotic_armor",
                format!("{ex_armor} exotic armour pieces; the game allows one"),
            ));
        }

        if self.level == 0 || self.level > LEVEL_CAP_01_29 {
            issues.push(ValidationIssue::warning(
                "level_out_of_range",
                format!("level {} is outside 1..={LEVEL_CAP_01_29}", self.level),
            ));
        }
        if self.light > LIGHT_CAP_01_29 {
            issues.push(ValidationIssue::warning(
                "light_out_of_range",
                format!("light {} is above {LIGHT_CAP_01_29}", self.light),
            ));
        }

        issues.sort();
        issues
    }

    /// True when nothing at [`Severity::Error`] is outstanding.
    pub fn is_valid(&self) -> bool {
        !self.validate().iter().any(|i| i.severity == Severity::Error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The document is internally inconsistent. Do not hand it to anything.
    Error,
    /// Loadable, but a human should look. Era problems land here - being out of era is a research
    /// hazard, not a malformed file.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    /// Stable machine-readable code. Grep for these; do not match on the message.
    pub code: String,
    pub message: String,
}

impl ValidationIssue {
    fn error(code: &str, message: impl Into<String>) -> Self {
        ValidationIssue {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
        }
    }
    fn warning(code: &str, message: impl Into<String>) -> Self {
        ValidationIssue {
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warn",
        };
        write!(f, "{tag}[{}]: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gjallarhorn, Year-1 copy. Real manifest values - see `catalogue::tests` for the assertion
    /// that these are what the preserved database actually holds.
    fn gjallarhorn_y1() -> ItemRef {
        ItemRef {
            hash: 1_274_330_687,
            name: "Gjallarhorn".to_string(),
            tier: Tier::Exotic,
            bucket_hash: Slot::Heavy.bucket_hash(),
            item_type_name: "Rocket Launcher".to_string(),
            icon: "/common/destiny_content/icons/af80c15aed47fc28095a6e040979b27c.jpg".to_string(),
            class_type: CLASS_TYPE_ANY,
            damage: Damage::Solar,
            era: Era::Unknown,
            era_note: String::new(),
        }
    }

    fn titan_helm() -> ItemRef {
        ItemRef {
            hash: 2_994_845_059,
            name: "Helm of Saint-14".to_string(),
            tier: Tier::Exotic,
            bucket_hash: Slot::Head.bucket_hash(),
            item_type_name: "Helmet".to_string(),
            icon: String::new(),
            class_type: 0, // Titan
            damage: Damage::None,
            era: Era::Unknown,
            era_note: String::new(),
        }
    }

    #[test]
    fn slot_bucket_hashes_round_trip() {
        for s in Slot::ALL {
            assert_eq!(Slot::from_bucket_hash(s.bucket_hash()), Some(s), "{s}");
        }
        // Negative control: a real bucket hash that is NOT an equip slot must not resolve.
        // BUCKET_MATERIALS, category 1.
        assert_eq!(Slot::from_bucket_hash(3_865_314_626), None);
    }

    #[test]
    fn json_round_trip_is_stable() {
        let mut l = Loadout {
            name: "y1 titan".to_string(),
            class: Class::Titan,
            ..Default::default()
        };
        l.set(Slot::Heavy, gjallarhorn_y1());
        l.set(Slot::Head, titan_helm());

        let a = l.to_json();
        let back = Loadout::from_json(&a).unwrap();
        assert_eq!(back, {
            let mut expect = l.clone();
            expect.equipped.sort_by_key(|e| {
                Slot::ALL.iter().position(|s| *s == e.slot).unwrap_or(usize::MAX)
            });
            expect
        });
        // Second serialisation is byte-identical, which is what makes a git diff mean something.
        assert_eq!(a, back.to_json());
    }

    #[test]
    fn refuses_a_document_from_the_future() {
        let err = Loadout::from_json(r#"{"schema_version": 99, "name": "x"}"#).unwrap_err();
        assert!(err.to_string().contains("newer"), "{err}");
    }

    #[test]
    fn tolerates_unknown_fields_and_missing_fields() {
        // Both halves matter: the wire representation is [U], so files will gain fields, and
        // hand-written loadouts will omit them.
        let l = Loadout::from_json(
            r#"{"schema_version": 1, "name": "sparse", "class": "hunter",
                "invented_by_a_later_session": {"a": 1}}"#,
        )
        .unwrap();
        assert_eq!(l.class, Class::Hunter);
        assert_eq!(l.level, LEVEL_CAP_01_29);
        assert!(l.equipped.is_empty());
    }

    #[test]
    fn detects_wrong_bucket() {
        let mut l = Loadout::default();
        // Gjallarhorn is a Heavy weapon; put it in Primary.
        l.set(Slot::Primary, gjallarhorn_y1());
        let codes: Vec<_> = l.validate().into_iter().map(|i| i.code).collect();
        assert!(codes.contains(&"bucket_slot_mismatch".to_string()), "{codes:?}");
        assert!(!l.is_valid());

        // Positive control: the same item in the right slot is clean, so the rule is not simply
        // rejecting everything.
        let mut ok = Loadout::default();
        ok.set(Slot::Heavy, gjallarhorn_y1());
        assert!(ok.is_valid(), "{:?}", ok.validate());
    }

    #[test]
    fn detects_class_restricted_armor_on_the_wrong_class() {
        let mut l = Loadout {
            class: Class::Warlock,
            ..Default::default()
        };
        l.set(Slot::Head, titan_helm());
        let codes: Vec<_> = l.validate().into_iter().map(|i| i.code).collect();
        assert!(codes.contains(&"class_mismatch".to_string()), "{codes:?}");

        // Positive control: on a Titan it is fine.
        let mut ok = Loadout {
            class: Class::Titan,
            ..Default::default()
        };
        ok.set(Slot::Head, titan_helm());
        assert!(ok.is_valid(), "{:?}", ok.validate());

        // And an unrestricted item (classType 3) is fine on every class.
        for c in Class::ALL {
            let mut any = Loadout { class: c, ..Default::default() };
            any.set(Slot::Heavy, gjallarhorn_y1());
            assert!(any.is_valid(), "{c}: {:?}", any.validate());
        }
    }

    #[test]
    fn detects_two_items_in_one_bucket() {
        // Reached only by hand-editing, which is exactly why `equipped` is a Vec.
        let mut l = Loadout::default();
        l.equipped.push(Equipped { slot: Slot::Heavy, item: gjallarhorn_y1() });
        l.equipped.push(Equipped {
            slot: Slot::Heavy,
            item: ItemRef { hash: 1, bucket_hash: Slot::Heavy.bucket_hash(), ..Default::default() },
        });
        let codes: Vec<_> = l.validate().into_iter().map(|i| i.code).collect();
        assert!(codes.contains(&"duplicate_slot".to_string()), "{codes:?}");
    }

    #[test]
    fn exotic_limit_is_a_warning_not_an_error() {
        let mut l = Loadout::default();
        l.set(Slot::Heavy, gjallarhorn_y1());
        l.set(
            Slot::Primary,
            ItemRef {
                hash: 3_164_616_405,
                name: "The Last Word".into(),
                tier: Tier::Exotic,
                bucket_hash: Slot::Primary.bucket_hash(),
                ..Default::default()
            },
        );
        let issues = l.validate();
        let codes: Vec<_> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"too_many_exotic_weapons"), "{codes:?}");
        // A rule we have not seen the client enforce must not make the document invalid.
        assert!(l.is_valid(), "{issues:?}");

        // Positive control: one exotic weapon is silent.
        l.clear(Slot::Primary);
        assert!(!l
            .validate()
            .iter()
            .any(|i| i.code == "too_many_exotic_weapons"));
    }

    #[test]
    fn set_replaces_rather_than_appends() {
        let mut l = Loadout::default();
        l.set(Slot::Heavy, gjallarhorn_y1());
        l.set(Slot::Heavy, ItemRef { hash: 7, bucket_hash: Slot::Heavy.bucket_hash(), ..Default::default() });
        assert_eq!(l.equipped.len(), 1);
        assert_eq!(l.get(Slot::Heavy).unwrap().hash, 7);
        l.clear(Slot::Heavy);
        assert!(l.get(Slot::Heavy).is_none());
    }

    #[test]
    fn identity_hashes_match_the_manifest() {
        // Pinned from `DestinyClassDefinition` / `DestinyRaceDefinition` / `DestinyGenderDefinition`
        // in the preserved world manifest. `catalogue::tests::identity_tables_match_the_constants`
        // re-reads them from the database; this is the cheap version that runs without it.
        assert_eq!(Class::Titan.hash(), 3_655_393_761);
        assert_eq!(Class::Hunter.hash(), 671_679_327);
        assert_eq!(Class::Warlock.hash(), 2_271_682_572);
        assert_eq!(Race::Awoken.hash(), 2_803_282_938);
        assert_eq!(Gender::Female.hash(), 2_204_441_813);
    }
}
