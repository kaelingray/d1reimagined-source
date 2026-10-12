// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The item catalogue: an extracted, cached, searchable view of the world manifest.
//!
//! The GUI must not hold a 59 MB SQLite database open to draw a list, and it must not re-inflate one
//! at startup. [`Catalogue`] is the extract - a few MB of JSON, written once by
//! `d1-loadout catalogue build` and loaded in milliseconds.
//!
//! Search is a linear scan over ~4,500 entries. That is not a placeholder for an index: at this size
//! a scan is faster than any structure that has to be kept coherent, and it means a filter can be an
//! arbitrary predicate rather than something the index anticipated.

use serde::{Deserialize, Serialize};

use crate::era::Era;
use crate::model::{Class, Damage, ItemRef, Slot, Tier, CLASS_TYPE_ANY};

/// Where the catalogue came from and what is wrong with it. Rendered by the GUI, verbatim, in a
/// place the user cannot miss - `CLAUDE.md`'s reach-tag discipline applied to a data file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CatalogueHeader {
    pub schema_version: u32,
    /// From `GetDestinyManifest.json`, e.g. `88857.20.11.05.1145-2`.
    pub manifest_version: String,
    /// Basename of the `.content` file the entries were read from.
    pub manifest_file: String,
    /// The build this project targets, so the mismatch is legible without cross-referencing a doc.
    pub target_build: String,
    pub items_scanned: u64,
    pub items_kept: u64,
    pub provenance_warning: String,
}

/// One equippable item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CatalogueEntry {
    pub hash: u32,
    pub name: String,
    pub description: String,
    pub tier: Tier,
    pub slot: Slot,
    pub bucket_hash: u32,
    pub item_type_name: String,
    /// Path relative to `https://www.bungie.net`. **Nothing in this project fetches it.**
    pub icon: String,
    pub class_type: u32,
    pub damage: Damage,
    /// `DestinyInventoryItemDefinition.qualityLevel`. Kept raw as well as interpreted, so a later
    /// session can re-derive [`CatalogueEntry::year_one`] without re-extracting.
    pub quality_level: u32,
    /// `quality_level > 0` - the Year-1 copy of an item. See [`crate::era`] for the controls.
    pub year_one: bool,
    pub era: Era,
    pub era_note: String,
}

impl CatalogueEntry {
    pub fn allows_class(&self, class: Class) -> bool {
        self.class_type == CLASS_TYPE_ANY || self.class_type == class.class_type()
    }

    /// Freeze this entry into the form a loadout stores.
    pub fn to_ref(&self) -> ItemRef {
        ItemRef {
            hash: self.hash,
            name: self.name.clone(),
            tier: self.tier,
            bucket_hash: self.bucket_hash,
            item_type_name: self.item_type_name.clone(),
            icon: self.icon.clone(),
            class_type: self.class_type,
            damage: self.damage,
            era: self.era,
            era_note: self.era_note.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Catalogue {
    pub header: CatalogueHeader,
    pub entries: Vec<CatalogueEntry>,
}

/// A search. Every field is a narrowing filter; all default to "no opinion".
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Case-insensitive substring of the name. A bare `0x…` or decimal number also matches an exact
    /// hash, because "which item is 1274330687" is the question that actually gets asked.
    pub text: String,
    pub slot: Option<Slot>,
    pub tier: Option<Tier>,
    /// Restricts to items this class may equip - including unrestricted ones.
    pub class: Option<Class>,
    pub damage: Option<Damage>,
    /// Only the Year-1 copies.
    pub year_one_only: bool,
    /// Drop anything the era classifier calls [`Era::PostEra`].
    pub hide_post_era: bool,
}

impl Catalogue {
    pub fn from_json(s: &str) -> anyhow::Result<Catalogue> {
        let c: Catalogue = serde_json::from_str(s)?;
        if c.header.schema_version > crate::model::SCHEMA_VERSION {
            anyhow::bail!(
                "catalogue schema_version {} is newer than this build understands ({})",
                c.header.schema_version,
                crate::model::SCHEMA_VERSION
            );
        }
        Ok(c)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Catalogue is always serialisable")
    }

    pub fn get(&self, hash: u32) -> Option<&CatalogueEntry> {
        self.entries.iter().find(|e| e.hash == hash)
    }

    /// Every entry with this exact name. **Usually more than one** - see [`crate::manifest`] trap 3.
    pub fn by_name(&self, name: &str) -> Vec<&CatalogueEntry> {
        self.entries
            .iter()
            .filter(|e| e.name.eq_ignore_ascii_case(name))
            .collect()
    }

    pub fn search(&self, q: &Query) -> Vec<&CatalogueEntry> {
        self.search_indices(q).into_iter().map(|i| &self.entries[i]).collect()
    }

    /// Positions in [`Catalogue::entries`] that match.
    ///
    /// The GUI keeps *indices* rather than references or clones: a virtualized table redraws from
    /// the cached result set every frame, and cloning ~5,000 entries on each keystroke is exactly
    /// the per-frame allocation egui punishes you for.
    pub fn search_indices(&self, q: &Query) -> Vec<usize> {
        let needle = q.text.trim().to_ascii_lowercase();
        let hash_needle = parse_hash(&needle);

        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if let Some(s) = q.slot {
                    if e.slot != s {
                        return false;
                    }
                }
                if let Some(t) = q.tier {
                    if e.tier != t {
                        return false;
                    }
                }
                if let Some(c) = q.class {
                    if !e.allows_class(c) {
                        return false;
                    }
                }
                if let Some(d) = q.damage {
                    if e.damage != d {
                        return false;
                    }
                }
                if q.year_one_only && !e.year_one {
                    return false;
                }
                if q.hide_post_era && e.era == Era::PostEra {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                if hash_needle == Some(e.hash) {
                    return true;
                }
                e.name.to_ascii_lowercase().contains(&needle)
                    || e.item_type_name.to_ascii_lowercase().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Re-resolve a stored [`ItemRef`] against the current catalogue.
    ///
    /// Returns `None` when the hash is not in the catalogue at all - which is a finding, not a bug:
    /// it means the loadout names something this manifest does not have.
    pub fn refresh_ref(&self, r: &ItemRef) -> Option<ItemRef> {
        self.get(r.hash).map(|e| {
            let mut fresh = e.to_ref();
            // A hand-written `Pinned` verdict outranks the extractor's inference - a human checked
            // it, the classifier only guessed.
            if r.era == Era::Pinned {
                fresh.era = Era::Pinned;
                fresh.era_note = r.era_note.clone();
            }
            fresh
        })
    }
}

/// `"0x4c0f1abf"` / `"1274330687"` → a hash. Anything else → `None`.
fn parse_hash(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

// -------------------------------------------------------------------------------------------------
// Tests
//
// These parse the REAL preserved manifest, not synthetic rows. `CLAUDE.md`'s standing rule applies:
// a query that "finds nothing" is only evidence if a known-present control was found alongside it.
// Every assertion below that something is absent is paired with one that something is present.
//
// They skip (rather than fail) when the manifest is not on disk, so the workspace still builds on a
// box without it - but `manifest_is_present` reports loudly which way it went.
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{default_scratch_dir, find_world_manifest, WorldManifest};
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    /// Everything the tests need, read once.
    ///
    /// `rusqlite::Connection` is `!Sync`, so the open database cannot live in a `static` - and
    /// re-inflating 59 MB per test would make the suite crawl. So the connection is opened once,
    /// drained into plain data, and dropped. The trade is deliberate: the fixture is what the
    /// database *said*, captured at one instant, which is also what makes the assertions below
    /// reproducible.
    struct Fixture {
        tables: Vec<String>,
        counts: BTreeMap<&'static str, u64>,
        missing_table_errors: bool,
        classes: Vec<serde_json::Value>,
        races: Vec<serde_json::Value>,
        genders: Vec<serde_json::Value>,
        buckets: Vec<serde_json::Value>,
        cat: Catalogue,
    }

    /// Build the fixture, or explain exactly why not.
    ///
    /// ⚠ **The two outcomes must not look alike.** The first draft of this returned `Option` and
    /// used `.ok()?` throughout, so a *read failure* (the `json` column is TEXT, not the declared
    /// BLOB - see `manifest`'s trap 2) was indistinguishable from *the file is not on this box*.
    /// Result: eight manifest-backed tests reported `ok` while asserting nothing at all, and the bug
    /// was only found because the CLI hit it. That is `CLAUDE.md`'s silent-false-negative trap, in
    /// this crate, on the first run.
    ///
    /// So: file absent → `Err` and the test skips, loudly, once. Anything else → **panic**, with the
    /// underlying error.
    fn build_fixture() -> Result<Fixture, String> {
        let start = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(p) = find_world_manifest(start, "en") else {
            return Err(format!(
                "no manifest/data/world_en_*.content at or above {}",
                start.display()
            ));
        };
        let scratch = default_scratch_dir().join("test-scratch");
        let m = WorldManifest::open(&p, &scratch)
            .unwrap_or_else(|e| panic!("manifest {} is present but did not open: {e:#}", p.display()));
        let mut counts = BTreeMap::new();
        for t in [
            "DestinyInventoryItemDefinition",
            "DestinyInventoryBucketDefinition",
            "DestinyClassDefinition",
            "DestinyRaceDefinition",
            "DestinyGenderDefinition",
            "DestinyStatDefinition",
        ] {
            counts.insert(t, m.row_count(t).unwrap_or_else(|e| panic!("counting {t}: {e:#}")));
        }
        let defs = |t: &str| {
            m.definitions(t)
                .unwrap_or_else(|e| panic!("reading {t}: {e:#}"))
        };
        Ok(Fixture {
            tables: m.tables().expect("listing tables"),
            counts,
            missing_table_errors: m.row_count("DestinyNotARealTable").is_err(),
            classes: defs("DestinyClassDefinition"),
            races: defs("DestinyRaceDefinition"),
            genders: defs("DestinyGenderDefinition"),
            buckets: defs("DestinyInventoryBucketDefinition"),
            cat: m
                .extract_catalogue()
                .unwrap_or_else(|e| panic!("extracting the catalogue: {e:#}")),
        })
    }

    fn fixture() -> Result<&'static Fixture, &'static String> {
        static F: OnceLock<Result<Fixture, String>> = OnceLock::new();
        F.get_or_init(build_fixture).as_ref()
    }

    macro_rules! need_fixture {
        () => {
            match fixture() {
                Ok(f) => f,
                Err(why) => {
                    // Printed with the test name so a run that skips everything is obvious in the
                    // output rather than looking like a clean pass.
                    println!("SKIP {}: {why}", module_path!());
                    return;
                }
            }
        };
    }

    macro_rules! need_manifest {
        () => {
            &need_fixture!().cat
        };
    }

    #[test]
    fn manifest_is_present_and_has_the_expected_shape() {
        let m = need_fixture!();
        let tables = &m.tables;
        assert_eq!(tables.len(), 39, "table count changed: {tables:?}");
        // Pinned row counts. If these move, the manifest on disk is not the one this crate was
        // written against and every era claim below needs re-checking.
        assert_eq!(m.counts["DestinyInventoryItemDefinition"], 8609);
        assert_eq!(m.counts["DestinyInventoryBucketDefinition"], 40);
        assert_eq!(m.counts["DestinyClassDefinition"], 3);
        assert_eq!(m.counts["DestinyRaceDefinition"], 3);
        assert_eq!(m.counts["DestinyGenderDefinition"], 2);
        assert_eq!(m.counts["DestinyStatDefinition"], 36);
        // Negative control: a table that does not exist must error, not return 0.
        assert!(m.missing_table_errors);
    }

    #[test]
    fn identity_tables_match_the_constants() {
        let m = need_fixture!();
        let classes = &m.classes;
        for c in Class::ALL {
            let row = classes
                .iter()
                .find(|d| d["classType"].as_u64() == Some(c.class_type() as u64))
                .unwrap_or_else(|| panic!("no DestinyClassDefinition row for {c}"));
            assert_eq!(row["classHash"].as_u64().unwrap() as u32, c.hash(), "{c}");
            assert_eq!(row["className"].as_str().unwrap(), c.name(), "{c}");
        }
        let races = &m.races;
        for r in crate::model::Race::ALL {
            let row = races
                .iter()
                .find(|d| d["raceName"].as_str() == Some(r.name()))
                .unwrap_or_else(|| panic!("no DestinyRaceDefinition row for {r}"));
            assert_eq!(row["raceHash"].as_u64().unwrap() as u32, r.hash(), "{r}");
        }
        let genders = &m.genders;
        for g in crate::model::Gender::ALL {
            let row = genders
                .iter()
                .find(|d| d["genderName"].as_str() == Some(g.name()))
                .unwrap_or_else(|| panic!("no DestinyGenderDefinition row for {g}"));
            assert_eq!(row["genderHash"].as_u64().unwrap() as u32, g.hash(), "{g}");
        }
    }

    #[test]
    fn slot_constants_match_the_bucket_table() {
        let m = need_fixture!();
        let buckets = &m.buckets;
        for s in Slot::ALL {
            let row = buckets
                .iter()
                .find(|d| d["bucketHash"].as_u64() == Some(s.bucket_hash() as u64))
                .unwrap_or_else(|| panic!("no bucket row for {s} ({:#010x})", s.bucket_hash()));
            // category 3 == Equippable. This is the check that stops a vault bucket sneaking in.
            assert_eq!(row["category"].as_u64(), Some(3), "{s} is not an equip bucket");
            assert_eq!(row["bucketName"].as_str(), Some(s.display_name()), "{s}");
        }
        // And the converse: every enabled category-3 bucket must have a Slot, or the enum is
        // missing an equip destination and the GUI would silently be unable to show it.
        let missing: Vec<String> = buckets
            .iter()
            .filter(|d| d["category"].as_u64() == Some(3) && d["enabled"].as_bool() == Some(true))
            .filter(|d| {
                Slot::from_bucket_hash(d["bucketHash"].as_u64().unwrap_or(0) as u32).is_none()
            })
            .map(|d| d["bucketIdentifier"].as_str().unwrap_or("?").to_string())
            .collect();
        assert!(missing.is_empty(), "equip buckets with no Slot: {missing:?}");
    }

    #[test]
    fn catalogue_extraction_finds_known_items() {
        let c = need_manifest!();
        assert!(c.entries.len() > 4000, "only {} entries", c.entries.len());
        assert_eq!(c.header.items_scanned, 8609);

        // Positive controls, by hash - the identity that actually matters.
        let gjally = c.get(1_274_330_687).expect("Year-1 Gjallarhorn 1274330687");
        assert_eq!(gjally.name, "Gjallarhorn");
        assert_eq!(gjally.slot, Slot::Heavy);
        assert_eq!(gjally.tier, Tier::Exotic);
        assert_eq!(gjally.item_type_name, "Rocket Launcher");
        assert_eq!(gjally.damage, Damage::Solar);
        assert_eq!(gjally.class_type, CLASS_TYPE_ANY);

        let saint = c.get(2_994_845_059).expect("Helm of Saint-14 2994845059");
        assert_eq!(saint.slot, Slot::Head);
        assert_eq!(saint.class_type, Class::Titan.class_type());
        assert!(saint.allows_class(Class::Titan));
        assert!(!saint.allows_class(Class::Warlock));

        // Subclasses are items in BUCKET_BUILD, and they are class-locked the same way armour is.
        let bladedancer = c.get(2_962_927_168).expect("Bladedancer 2962927168");
        assert_eq!(bladedancer.slot, Slot::Subclass);
        assert_eq!(bladedancer.class_type, Class::Hunter.class_type());

        // Negative control on `get`: a hash that is not an item.
        assert!(c.get(0xDEAD_BEEF).is_none());
    }

    #[test]
    fn the_same_name_resolves_to_several_hashes() {
        // The re-issue trap, asserted so a later session cannot quietly "fix" `by_name` to return
        // one entry. Picking the first row for "Gjallarhorn" is a coin flip between Year 1 and
        // Rise of Iron.
        let c = need_manifest!();
        let all = c.by_name("Gjallarhorn");
        assert!(all.len() >= 2, "expected several Gjallarhorns, got {}", all.len());
        let hashes: Vec<u32> = all.iter().map(|e| e.hash).collect();
        assert!(hashes.contains(&1_274_330_687), "{hashes:?}");
        assert!(hashes.contains(&3_564_229_425), "{hashes:?}");
        // Control that `by_name` is not simply matching everything.
        assert!(c.by_name("a weapon that does not exist").is_empty());
    }

    #[test]
    fn year_one_marker_has_both_controls() {
        let c = need_manifest!();

        // POSITIVE: items that only ever existed in Year 1, or whose Year-1 copy is distinct.
        for (hash, name) in [
            (346_443_849u32, "Vex Mythoclast"),
            (3_191_797_831, "Thunderlord"),
            (1_274_330_687, "Gjallarhorn"),
        ] {
            let e = c.get(hash).unwrap_or_else(|| panic!("{name} {hash} missing"));
            assert!(e.year_one, "{name} {hash} should be flagged Year 1 (q={})", e.quality_level);
        }

        // NEGATIVE: exotics introduced in The Taken King or later cannot have a Year-1 copy. Without
        // this half, a marker that returned `true` for everything would pass.
        for (hash, name) in [
            (3_851_373_522u32, "Nemesis Star"),
            (3_012_398_149, "Sleeper Simulant"),
            (3_688_594_189, "Touch of Malice"),
            (255_654_879, "Zhalo Supercell"),
            (3_742_521_821, "Outbreak Prime"),
        ] {
            let e = c.get(hash).unwrap_or_else(|| panic!("{name} {hash} missing"));
            assert!(!e.year_one, "{name} {hash} must not be flagged Year 1 (q={})", e.quality_level);
        }

        // And the population is split, not degenerate.
        let y1 = c.entries.iter().filter(|e| e.year_one).count();
        assert!(y1 > 100 && y1 < c.entries.len(), "year_one split is degenerate: {y1}");
    }

    #[test]
    fn era_classifier_has_both_controls() {
        let c = need_manifest!();

        // POSITIVE: an Age of Triumph re-issue, sourced only from "Raid Reprise".
        let adept = c.get(1_288_233_773).expect("Fatebringer (Adept) 1288233773");
        assert_eq!(adept.name, "Fatebringer (Adept)");
        assert_eq!(adept.era, Era::PostEra, "note: {}", adept.era_note);

        // NEGATIVE: the Year-1 Fatebringer, from Vault of Glass, must NOT be post-era.
        let y1 = c.get(3_892_023_023).expect("Fatebringer 3892023023");
        assert_eq!(y1.era, Era::SourcesInEra, "note: {}", y1.era_note);

        // The classifier must produce a spread, not one verdict for everything.
        let post = c.entries.iter().filter(|e| e.era == Era::PostEra).count();
        let unknown = c.entries.iter().filter(|e| e.era == Era::Unknown).count();
        let in_era = c.entries.iter().filter(|e| e.era == Era::SourcesInEra).count();
        assert!(post > 0 && unknown > 0 && in_era > 0, "{post} post / {unknown} unknown / {in_era} in-era");
        // Post-era is a minority. If it ever becomes the bulk, the timing table is wrong.
        assert!(post * 4 < c.entries.len(), "{post} of {} flagged post-era", c.entries.len());
    }

    #[test]
    fn search_filters_narrow_and_still_find_something() {
        let c = need_manifest!();

        // Text search, with its own positive control.
        let by_text = c.search(&Query { text: "gjallarhorn".into(), ..Default::default() });
        assert!(!by_text.is_empty(), "text search found nothing");

        // Hash search: the exact question people actually ask.
        let by_hash = c.search(&Query { text: "1274330687".into(), ..Default::default() });
        assert_eq!(by_hash.len(), 1);
        assert_eq!(by_hash[0].name, "Gjallarhorn");
        let by_hex = c.search(&Query { text: "0x4bf4be3f".into(), ..Default::default() });
        assert_eq!(by_hex.len(), 1, "0x4bf4be3f == 1274330687");

        // Narrowing must actually narrow, and must not narrow to nothing.
        let all_heavy = c.search(&Query { slot: Some(Slot::Heavy), ..Default::default() });
        let exotic_heavy = c.search(&Query {
            slot: Some(Slot::Heavy),
            tier: Some(Tier::Exotic),
            ..Default::default()
        });
        assert!(!exotic_heavy.is_empty());
        assert!(exotic_heavy.len() < all_heavy.len());
        assert!(exotic_heavy.iter().all(|e| e.slot == Slot::Heavy && e.tier == Tier::Exotic));

        // A class filter must keep *unrestricted* items - the mistake that empties a weapon list.
        //
        // ⚠ It is **not** true that all weapons are `classType == 3`. This assertion originally read
        // `warlock_heavy.len() == all_heavy.len()` and failed by exactly one, which is how the
        // class-locked weapons were found: The Taken King's `Ace of Spades` (Hunter),
        // `Fabian Strategy` (Titan), `Tlaloc` (Warlock), the Year-3 faction specials
        // `Immobius`/`Stillpiercer`/`Susanoo`, and one redacted "Classified" heavy - seven weapons
        // in all carry a real class restriction.
        let primaries = c.search(&Query { slot: Some(Slot::Primary), ..Default::default() });
        let titan_primaries = c.search(&Query {
            slot: Some(Slot::Primary),
            class: Some(Class::Titan),
            ..Default::default()
        });
        assert!(titan_primaries.len() < primaries.len());
        // Positive: the Titan exotic survives a Titan filter.
        assert!(titan_primaries.iter().any(|e| e.hash == 2_748_609_458), "Fabian Strategy");
        // Negative: and is gone for a Warlock, while the Warlock's own exotic appears.
        let warlock_primaries = c.search(&Query {
            slot: Some(Slot::Primary),
            class: Some(Class::Warlock),
            ..Default::default()
        });
        assert!(!warlock_primaries.iter().any(|e| e.hash == 2_748_609_458), "Fabian Strategy");
        assert!(warlock_primaries.iter().any(|e| e.hash == 803_312_564), "Tlaloc");
        // But the overwhelming majority of weapons are unrestricted and must survive either filter.
        assert!(warlock_primaries.len() * 100 > primaries.len() * 99);

        // Class filter on armour does exclude.
        let all_head = c.search(&Query { slot: Some(Slot::Head), ..Default::default() });
        let titan_head = c.search(&Query {
            slot: Some(Slot::Head),
            class: Some(Class::Titan),
            ..Default::default()
        });
        assert!(!titan_head.is_empty());
        assert!(titan_head.len() < all_head.len());
        assert!(titan_head.iter().all(|e| e.allows_class(Class::Titan)));

        // Negative control: a query for something that is genuinely absent returns nothing, which
        // only means anything because the searches above found things.
        assert!(c
            .search(&Query { text: "zzzz-not-an-item".into(), ..Default::default() })
            .is_empty());
    }

    #[test]
    fn catalogue_json_round_trips() {
        let c = need_manifest!();
        let json = c.to_json();
        let back = Catalogue::from_json(&json).unwrap();
        assert_eq!(&back, c);
    }

    #[test]
    fn to_ref_produces_a_loadout_that_validates() {
        let c = need_manifest!();
        let mut l = crate::model::Loadout {
            name: "control".into(),
            class: Class::Titan,
            ..Default::default()
        };
        l.set(Slot::Heavy, c.get(1_274_330_687).unwrap().to_ref());
        l.set(Slot::Head, c.get(2_994_845_059).unwrap().to_ref());
        l.set(Slot::Subclass, c.get(2_455_559_914).unwrap().to_ref()); // Striker, Titan
        assert!(l.is_valid(), "{:?}", l.validate());

        // And the same equipment on a Hunter must fail - proving the validator is reading the
        // catalogue's class data, not defaulting to permissive.
        let hunter = crate::model::Loadout { class: Class::Hunter, ..l.clone() };
        let codes: Vec<_> = hunter.validate().into_iter().map(|i| i.code).collect();
        assert!(codes.contains(&"class_mismatch".to_string()), "{codes:?}");
    }
}
