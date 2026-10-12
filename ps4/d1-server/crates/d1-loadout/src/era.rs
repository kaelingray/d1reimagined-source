// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Era: is this item plausibly *from* the build we target?
//!
//! # The problem, stated honestly
//!
//! The preserved world manifest is version `88857.20.11.05.1145-2` - captured in **2020**, four years
//! after the build we run. We target **01.29** = Bungie **2.4.1, 18 Oct 2016** (`ERA-TARGETING.md`
//! pins this from the PKG `c_date`). The manifest has **no "date added" field**: nothing in
//! `DestinyInventoryItemDefinition` says when an item entered the game.
//!
//! So era cannot be *read*. It can only be *inferred*, and the inference is weak. This module makes
//! that weakness explicit rather than hiding it behind a boolean.
//!
//! # The one signal that exists: reward sources
//!
//! `sourceHashes` points into `DestinyRewardSourceDefinition`, whose 64 rows are named activities and
//! vendors ("Vault of Glass", "Age of Triumph", "Lady Efrideet"). Those names have public release
//! dates. [`SOURCE_TIMING`] assigns each one a side of the 01.29 boundary - or [`Timing::Undated`]
//! when the name is generic ("Quest", "Unknown", "Evolved From Another Item") or when this session
//! was not confident, which is a vote of *nothing*, never a vote of "fine".
//!
//! **What this can and cannot prove:**
//!
//! * It **can** flag an item as [`Era::PostEra`]: if every dated source shipped after 18 Oct 2016,
//!   the item did not exist in our build. Positive control in the tests: `Fatebringer (Adept)`
//!   (`0x4CC5FE6D`), an Age of Triumph re-issue, must come back `PostEra`.
//! * It **cannot** prove an item is in era. An item added in Age of Triumph and dropped from the
//!   Crucible lists "Crucible" - a pre-boundary source - and looks clean. [`Era::SourcesInEra`]
//!   therefore means *"no evidence against"*, and is named to say so.
//! * 2,942 of 8,609 items (34%) carry **no** `sourceHashes` at all and are simply [`Era::Unknown`].
//!
//! Reach tag for every verdict here: `static-client-inference`, and even that is generous - the
//! source is Bungie's public API, not our client. Nothing in this module has been checked against
//! the 01.29 binary.
//!
//! # The other signal: `qualityLevel` separates Year-1 copies from their re-issues
//!
//! This one is *measured*, not assumed, and it matters more than it looks. D1 re-issued items across
//! years **under different item hashes**. Both copies are in the manifest, both are named
//! "Gjallarhorn", and they are different items:
//!
//! | hash | `qualityLevel` | reward sources |
//! |---|---|---|
//! | `1274330687` | 70 | Vault of Glass, Crota's End, Xûr, … (Year 1) |
//! | `3564229425` | 0 | Quest, **Rise of Iron** (the Year-3 quest re-issue) |
//!
//! Across every pair examined, the Year-1 copy has `qualityLevel > 0` and the later copy has `0`.
//! `Catalogue::year_one` uses that, and `catalogue::tests::year_one_marker_has_both_controls`
//! asserts it in both directions: Year-1-only items (Vex Mythoclast `346443849`, Thunderlord
//! `3191797831`) must be flagged, and items that **cannot** have a Year-1 copy because they were
//! introduced in The Taken King or later (Nemesis Star, Sleeper Simulant, Touch of Malice, Zhalo
//! Supercell, Outbreak Prime) must not be. A one-sided check here would have passed on a scanner
//! that flagged everything. [O - reproducible from the preserved manifest; not confirmed against
//! the client.]

use serde::{Deserialize, Serialize};

/// Which side of build 01.29 (Bungie 2.4.1, 18 Oct 2016) a reward source shipped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// Released on or before 18 Oct 2016 - could be in our build.
    AtOrBefore,
    /// Released after 18 Oct 2016 - definitely not in our build.
    After,
    /// A generic marker, or a date this session would not assert. **Votes for nothing.**
    Undated,
}

use Timing::{After, AtOrBefore, Undated};

/// `DestinyRewardSourceDefinition` rows, with a hand-assigned release side.
///
/// Sorted by hash so the table is binary-searchable and reviewable as a stable diff. The dates
/// behind the assignment are D1's public release history: launch 2014-09-09, The Dark Below
/// 2014-12-09, House of Wolves 2015-05-19, The Taken King 2015-09-15, Sparrow Racing 2015-12-08,
/// Crimson Days 2016-02-09, April Update 2016-04-12, Rise of Iron 2016-09-20, **01.29 2016-10-18**,
/// The Dawning 2016-12-13, Age of Triumph 2017-03-28.
///
/// `Raid Reprise` is the Age of Triumph raid re-issue line - [I], from the name plus the fact that
/// every `(Adept)` raid weapon in the manifest sources from it, and Adept raid gear is an Age of
/// Triumph feature.
pub const SOURCE_TIMING: &[(u32, &str, Timing)] = &[
    (39778960, "Bounty Tracker", Undated),
    (103311758, "Dead Orbit Vendor", AtOrBefore),
    (164690604, "Lady Efrideet", AtOrBefore),
    (321228980, "Warlock Vanguard", AtOrBefore),
    (353834582, "Gunsmith", AtOrBefore),
    (440710167, "Vault of Glass", AtOrBefore),
    (469744126, "Tyra Karn", AtOrBefore),
    (512830513, "Variks", AtOrBefore),
    (560942287, "Unknown", Undated),
    (615633921, "Cryptarch", AtOrBefore),
    (644349516, "Meridian Bay", AtOrBefore),
    (866590753, "Ocean of Storms", AtOrBefore),
    (995344558, "Future War Cult Vendor", AtOrBefore),
    (1111209135, "Crucible", AtOrBefore),
    (1234918199, "Sparrow Racing", AtOrBefore),
    (1322283879, "Iron Banner", AtOrBefore),
    (1362425043, "Crucible Quartermaster", AtOrBefore),
    (1374970038, "Crota's Bane", AtOrBefore),
    (1412538411, "Exotic Bounty", AtOrBefore),
    (1482793537, "Vanguard Quartermaster", AtOrBefore),
    (1488238664, "Oryx's Dreadnaught", AtOrBefore),
    (1537575125, "Challenge of the Elders", AtOrBefore),
    (1564673835, "Guardian Outfitter", AtOrBefore),
    (1662673928, "King's Fall", AtOrBefore),
    (1682133942, "Queen's Wrath", AtOrBefore),
    (1720938419, "Titan Vanguard", AtOrBefore),
    (1805138012, "Heroic", AtOrBefore),
    (1983234046, "Queen's Wrath Quests", AtOrBefore),
    (2179714245, "Xûr, Agent of the Nine", AtOrBefore),
    (2312726869, "Nightfall", AtOrBefore),
    (2480320429, "Hunter Vanguard", AtOrBefore),
    (2585003248, "Crota's End", AtOrBefore),
    (2650556703, "Trials of Osiris", AtOrBefore),
    (2659839637, "The Taken King", AtOrBefore),
    (2682516238, "Patrol", AtOrBefore),
    (2765678689, "Old Russia", AtOrBefore),
    (2775576620, "Crimson Doubles", AtOrBefore),
    (2784812137, "Prison of Elders", AtOrBefore),
    (2900459251, "Refer-a-Friend", Undated),
    (2964550958, "Rise of Iron", AtOrBefore),
    (2977848024, "Moments of Triumph, Year 2", AtOrBefore),
    (3068521220, "Age of Triumph", After),
    (3072854931, "New Monarchy Vendor", AtOrBefore),
    (3080570839, "Ishtar Sink", AtOrBefore),
    (3102749437, "Shiro-4", AtOrBefore),
    (3131490494, "The Dawning", After),
    (3341981194, "Quest", Undated),
    (3378481830, "Disciple of Osiris", AtOrBefore),
    (3475869915, "Festival of the Lost", AtOrBefore),
    (3480896747, "Public Events", AtOrBefore),
    (3498458434, "Silver Dust", Undated),
    (3559790162, "Eververse", AtOrBefore),
    (3602080346, "Strike", AtOrBefore),
    (3667653533, "Archon's Forge", AtOrBefore),
    (3721473564, "Shipwright", AtOrBefore),
    (3727034986, "Rise of Iron Record Book", AtOrBefore),
    (3846902203, "Lord Saladin", AtOrBefore),
    (4061111816, "Iron Banner Reputation", AtOrBefore),
    (4134961255, "Crucible Handler", AtOrBefore),
    (4160622434, "Wrath of the Machine", AtOrBefore),
    (4161861381, "Raid Reprise", After),
    (4233534433, "Character Creation", AtOrBefore),
    (4241664776, "The Speaker", AtOrBefore),
    (4260990541, "Evolved From Another Item", Undated),
];

/// The build this project targets, for display.
pub const TARGET_BUILD: &str = "01.29 (Bungie 2.4.1, 18 Oct 2016)";
/// The manifest the catalogue comes from, for display.
pub const MANIFEST_VERSION: &str = "88857.20.11.05.1145-2";

/// What we are willing to say about an item's era.
///
/// The variants are ordered from "no idea" to "certainly wrong", so `max()` over an equipment list
/// gives the loadout's worst verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Era {
    /// No dated reward source. **The default and the majority answer.**
    #[default]
    Unknown,
    /// A human pinned this after checking it. `era_note` records who and on what grounds. Never
    /// produced by the extractor - only by hand, in a checked-in loadout file.
    Pinned,
    /// Every dated source shipped on or before 01.29. Means *no evidence against*, **not** "in era":
    /// an Age of Triumph item that also drops from the Crucible lands here.
    SourcesInEra,
    /// Dated sources on both sides of the boundary. Usually a re-issue.
    SourcesMixed,
    /// Every dated source shipped after 01.29. The item was not in our build.
    PostEra,
}

impl Era {
    pub fn label(self) -> &'static str {
        match self {
            Era::Unknown => "era unknown",
            Era::Pinned => "era pinned by hand",
            Era::SourcesInEra => "no evidence against",
            Era::SourcesMixed => "sources straddle 01.29",
            Era::PostEra => "post-01.29",
        }
    }

    /// True when a UI should draw attention to it.
    pub fn is_suspect(self) -> bool {
        matches!(self, Era::PostEra | Era::SourcesMixed)
    }
}

/// An era verdict plus the evidence it rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraVerdict {
    pub era: Era,
    /// Human-readable evidence, e.g. `"sources: Raid Reprise (after)"`. Goes straight into
    /// `ItemRef::era_note`, so a saved loadout carries its own justification.
    pub note: String,
}

/// Look up one reward source.
pub fn source_timing(hash: u32) -> Option<(&'static str, Timing)> {
    SOURCE_TIMING
        .binary_search_by_key(&hash, |(h, _, _)| *h)
        .ok()
        .map(|i| (SOURCE_TIMING[i].1, SOURCE_TIMING[i].2))
}

/// Classify an item from its `sourceHashes`.
pub fn classify(source_hashes: &[u32]) -> EraVerdict {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut undated = 0usize;
    let mut unrecognised = 0usize;

    for h in source_hashes {
        match source_timing(*h) {
            Some((name, Timing::AtOrBefore)) => before.push(name),
            Some((name, Timing::After)) => after.push(name),
            Some((_, Timing::Undated)) => undated += 1,
            // A source hash the 2020 manifest has and this table does not. Should be impossible
            // (the table was generated from that manifest) but counted rather than ignored, because
            // "the scan found nothing" is the project's most expensive failure mode.
            None => unrecognised += 1,
        }
    }

    let era = match (before.is_empty(), after.is_empty()) {
        (true, true) => Era::Unknown,
        (true, false) => Era::PostEra,
        (false, true) => Era::SourcesInEra,
        (false, false) => Era::SourcesMixed,
    };

    let mut note = String::new();
    if !after.is_empty() {
        note.push_str("after 01.29: ");
        note.push_str(&after.join(", "));
    }
    if !before.is_empty() {
        if !note.is_empty() {
            note.push_str("; ");
        }
        note.push_str("at/before 01.29: ");
        note.push_str(&before.join(", "));
    }
    if undated > 0 {
        if !note.is_empty() {
            note.push_str("; ");
        }
        note.push_str(&format!("{undated} undated source(s)"));
    }
    if unrecognised > 0 {
        note.push_str(&format!("; {unrecognised} source(s) not in the timing table"));
    }
    if note.is_empty() {
        note.push_str("no reward sources listed");
    }

    EraVerdict { era, note }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_table_is_sorted_and_unique() {
        // `source_timing` binary-searches. An unsorted table would return `None` for real hashes -
        // a lookup that silently finds nothing, which is the failure this project keeps paying for.
        for w in SOURCE_TIMING.windows(2) {
            assert!(w[0].0 < w[1].0, "unsorted or duplicated at {:?} / {:?}", w[0], w[1]);
        }
        // Positive control on the search itself.
        assert_eq!(source_timing(440710167).map(|(n, _)| n), Some("Vault of Glass"));
        assert_eq!(source_timing(4161861381).map(|(_, t)| t), Some(Timing::After));
        // Negative control: a hash that is not a reward source at all (BUCKET_HEAD).
        assert_eq!(source_timing(3448274439), None);
    }

    #[test]
    fn classify_covers_all_four_verdicts() {
        // Age of Triumph "Raid Reprise" only → post-era.
        let v = classify(&[4161861381]);
        assert_eq!(v.era, Era::PostEra, "{v:?}");
        assert!(v.note.contains("Raid Reprise"), "{v:?}");

        // Vault of Glass only → nothing against it.
        assert_eq!(classify(&[440710167]).era, Era::SourcesInEra);

        // Both → mixed. This is the shape of a re-issued raid weapon.
        assert_eq!(classify(&[440710167, 4161861381]).era, Era::SourcesMixed);

        // Nothing, and undated-only, are both Unknown - an undated source votes for nothing.
        assert_eq!(classify(&[]).era, Era::Unknown);
        assert_eq!(classify(&[3341981194]).era, Era::Unknown); // "Quest"
        assert!(classify(&[]).note.contains("no reward sources"));
    }

    #[test]
    fn unrecognised_sources_are_reported_not_swallowed() {
        let v = classify(&[0xDEAD_BEEF]);
        assert_eq!(v.era, Era::Unknown);
        assert!(v.note.contains("not in the timing table"), "{v:?}");
    }
}
