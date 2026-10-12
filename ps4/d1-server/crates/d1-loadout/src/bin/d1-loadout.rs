// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-loadout` - build the item catalogue, search it, and check loadout files.
//!
//! This exists *before* the GUI on purpose. `historicaldocs/design/interface-architecture.md` §9.1: **every feature
//! exists in the CLI/JSON contract before or with any UI surface.** Everything `d1-forge` can do to a
//! loadout, this can do from a script, with `--json`.
//!
//! It talks to no network and runs nothing. Building the catalogue reads one file off disk.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use d1_loadout::{
    catalogue::Query,
    era,
    manifest::{default_scratch_dir, find_world_manifest, WorldManifest},
    Catalogue, Class, Damage, Loadout, Severity, Slot, Tier,
};

#[derive(Parser)]
#[command(
    name = "d1-loadout",
    about = "Destiny 1 loadout data model and world-manifest catalogue"
)]
struct Cli {
    /// Machine-readable output.
    #[arg(long, global = true)]
    json: bool,

    /// Cached catalogue. Built by `d1-loadout build`.
    #[arg(long, global = true, default_value = "catalogue/d1-catalogue.json")]
    catalogue: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract the equippable-item catalogue from the preserved world manifest.
    Build {
        /// The `.content` file. Discovered under `manifest/data/` if omitted.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Language of the world manifest to discover.
        #[arg(long, default_value = "en")]
        lang: String,
    },
    /// Report what the manifest on disk contains - the control run.
    ///
    /// Run this before citing anything the catalogue says. It is the same discipline `d1-eboot
    /// controls` enforces for static analysis: a scan with no positive control is not evidence.
    Controls {
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long, default_value = "en")]
        lang: String,
    },
    /// Search the cached catalogue.
    Search {
        /// Name substring, or an exact hash (`1274330687` or `0x4bf4be3f`).
        text: Option<String>,
        #[arg(long, value_parser = parse_slot)]
        slot: Option<Slot>,
        #[arg(long, value_parser = parse_tier)]
        tier: Option<Tier>,
        #[arg(long, value_parser = parse_class)]
        class: Option<Class>,
        #[arg(long, value_parser = parse_damage)]
        damage: Option<Damage>,
        /// Only Year-1 copies (`qualityLevel > 0`).
        #[arg(long)]
        year_one: bool,
        /// Drop items whose reward sources are all post-01.29.
        #[arg(long)]
        hide_post_era: bool,
        #[arg(long, default_value_t = 40)]
        limit: usize,
    },
    /// Print one loadout file, resolved and checked.
    Show { path: PathBuf },
    /// Validate loadout files. Exits 2 if any has an error.
    Validate {
        /// Files or directories. Defaults to `loadouts/`.
        paths: Vec<PathBuf>,
        /// Also re-resolve every item against the catalogue and report stale cached fields.
        #[arg(long)]
        refresh: bool,
    },
    /// List the equip slots and their bucket hashes.
    Slots,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Build { manifest, lang } => build(&cli, manifest.as_deref(), lang),
        Command::Controls { manifest, lang } => controls(&cli, manifest.as_deref(), lang),
        Command::Search { .. } => search(&cli),
        Command::Show { path } => show(&cli, path),
        Command::Validate { paths, refresh } => validate(&cli, paths, *refresh),
        Command::Slots => slots(&cli),
    }
}

fn open_manifest(explicit: Option<&std::path::Path>, lang: &str) -> Result<WorldManifest> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let cwd = std::env::current_dir()?;
            find_world_manifest(&cwd, lang).with_context(|| {
                format!("no manifest/data/world_{lang}_*.content found at or above {}", cwd.display())
            })?
        }
    };
    WorldManifest::open(&path, &default_scratch_dir())
}

fn build(cli: &Cli, manifest: Option<&std::path::Path>, lang: &str) -> Result<()> {
    let m = open_manifest(manifest, lang)?;
    let cat = m.extract_catalogue()?;
    if let Some(dir) = cli.catalogue.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&cli.catalogue, cat.to_json())?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&cat.header)?);
    } else {
        println!("wrote {}", cli.catalogue.display());
        println!("  manifest      {}", cat.header.manifest_file);
        println!("  version       {}", cat.header.manifest_version);
        println!("  target build  {}", cat.header.target_build);
        println!("  scanned       {}", cat.header.items_scanned);
        println!("  equippable    {}", cat.header.items_kept);
    }
    Ok(())
}

/// The control run. Every number here is pinned in `catalogue::tests`; this is the operator-facing
/// version, and it exits 2 when a control fails so a script cannot cite a broken extraction.
fn controls(cli: &Cli, manifest: Option<&std::path::Path>, lang: &str) -> Result<()> {
    let m = open_manifest(manifest, lang)?;
    let mut checks: Vec<(String, bool, String)> = Vec::new();

    let tables = m.tables()?;
    checks.push(("tables == 39".into(), tables.len() == 39, format!("{}", tables.len())));
    for (t, want) in [
        ("DestinyInventoryItemDefinition", 8609u64),
        ("DestinyInventoryBucketDefinition", 40),
        ("DestinyClassDefinition", 3),
        ("DestinyRaceDefinition", 3),
        ("DestinyGenderDefinition", 2),
        ("DestinyStatDefinition", 36),
    ] {
        let got = m.row_count(t).unwrap_or(0);
        checks.push((format!("{t} == {want}"), got == want, format!("{got}")));
    }
    checks.push((
        "no such table errors".into(),
        m.row_count("DestinyNotARealTable").is_err(),
        "err".into(),
    ));

    let cat = m.extract_catalogue()?;
    let n = cat.entries.len();
    checks.push(("equippable items > 4000".into(), n > 4000, format!("{n}")));

    // Item-level positive controls, all by hash.
    for (hash, name, slot) in [
        (1_274_330_687u32, "Gjallarhorn", Slot::Heavy),
        (2_994_845_059, "Helm of Saint-14", Slot::Head),
        (2_962_927_168, "Bladedancer", Slot::Subclass),
    ] {
        let ok = cat.get(hash).is_some_and(|e| e.name == name && e.slot == slot);
        checks.push((format!("{name} {hash} present in {slot}"), ok, String::new()));
    }
    // Negative control on lookup.
    checks.push(("0xdeadbeef absent".into(), cat.get(0xDEAD_BEEF).is_none(), String::new()));

    // Year-1 marker, both directions.
    let y1_pos = cat.get(346_443_849).is_some_and(|e| e.year_one);
    let y1_neg = cat.get(3_012_398_149).is_some_and(|e| !e.year_one);
    checks.push(("Vex Mythoclast 346443849 is year_one".into(), y1_pos, String::new()));
    checks.push(("Sleeper Simulant 3012398149 is NOT year_one".into(), y1_neg, String::new()));

    // Era classifier, both directions.
    let era_pos = cat.get(1_288_233_773).is_some_and(|e| e.era == era::Era::PostEra);
    let era_neg = cat.get(3_892_023_023).is_some_and(|e| e.era == era::Era::SourcesInEra);
    checks.push(("Fatebringer (Adept) is post-era".into(), era_pos, String::new()));
    checks.push(("Fatebringer (Y1) is not post-era".into(), era_neg, String::new()));

    let failed = checks.iter().filter(|(_, ok, _)| !ok).count();
    if cli.json {
        let rows: Vec<_> = checks
            .iter()
            .map(|(name, ok, got)| serde_json::json!({"check": name, "pass": ok, "got": got}))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "manifest": m.source().display().to_string(),
                "failed": failed,
                "checks": rows,
            }))?
        );
    } else {
        println!("manifest {}", m.source().display());
        for (name, ok, got) in &checks {
            let mark = if *ok { "ok  " } else { "FAIL" };
            if got.is_empty() {
                println!("  {mark} {name}");
            } else {
                println!("  {mark} {name}  ({got})");
            }
        }
        println!("{} check(s), {failed} failed", checks.len());
    }
    if failed > 0 {
        std::process::exit(2);
    }
    Ok(())
}

fn load_catalogue(cli: &Cli) -> Result<Catalogue> {
    let s = std::fs::read_to_string(&cli.catalogue).with_context(|| {
        format!(
            "reading {} - run `d1-loadout build` first",
            cli.catalogue.display()
        )
    })?;
    Catalogue::from_json(&s)
}

fn search(cli: &Cli) -> Result<()> {
    let Command::Search {
        text,
        slot,
        tier,
        class,
        damage,
        year_one,
        hide_post_era,
        limit,
    } = &cli.command
    else {
        unreachable!()
    };
    let cat = load_catalogue(cli)?;
    let q = Query {
        text: text.clone().unwrap_or_default(),
        slot: *slot,
        tier: *tier,
        class: *class,
        damage: *damage,
        year_one_only: *year_one,
        hide_post_era: *hide_post_era,
    };
    let hits = cat.search(&q);
    if cli.json {
        let shown: Vec<_> = hits.iter().take(*limit).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "matched": hits.len(),
                "shown": shown.len(),
                "provenance": cat.header,
                "items": shown,
            }))?
        );
    } else {
        println!("{} match(es)", hits.len());
        for e in hits.iter().take(*limit) {
            let y1 = if e.year_one { " [Y1]" } else { "" };
            let era_mark = if e.era.is_suspect() { format!(" [{}]", e.era.label()) } else { String::new() };
            println!(
                "{:>10}  {:<11} {:<9} {:<22} {}{}{}",
                e.hash,
                e.slot.display_name(),
                e.tier.name(),
                e.item_type_name,
                e.name,
                y1,
                era_mark
            );
        }
        if hits.len() > *limit {
            println!("… {} more (raise --limit)", hits.len() - *limit);
        }
    }
    Ok(())
}

fn show(cli: &Cli, path: &std::path::Path) -> Result<()> {
    let l = Loadout::from_json(&std::fs::read_to_string(path)?)?;
    let issues = l.validate();
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "loadout": l,
                "issues": issues,
            }))?
        );
        return Ok(());
    }
    println!("{} - {} {} {}", l.name, l.gender, l.race, l.class);
    println!("  level {}  light {}", l.level, l.light);
    if !l.notes.trim().is_empty() {
        println!("  {}", l.notes.trim());
    }
    for slot in Slot::ALL {
        if let Some(it) = l.get(slot) {
            let era_mark = if it.era.is_suspect() {
                format!("  <{}>", it.era.label())
            } else {
                String::new()
            };
            println!(
                "  {:<16} {:>10}  {:<9} {}{}",
                slot.display_name(),
                it.hash,
                it.tier.name(),
                it.name,
                era_mark
            );
        }
    }
    for i in &issues {
        println!("  {i}");
    }
    Ok(())
}

fn validate(cli: &Cli, paths: &[PathBuf], refresh: bool) -> Result<()> {
    let mut files = Vec::new();
    let roots: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from("loadouts")]
    } else {
        paths.to_vec()
    };
    for r in roots {
        if r.is_dir() {
            let mut in_dir: Vec<PathBuf> = std::fs::read_dir(&r)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect();
            in_dir.sort();
            files.extend(in_dir);
        } else {
            files.push(r);
        }
    }

    let cat = if refresh { Some(load_catalogue(cli)?) } else { None };
    let mut report = Vec::new();
    let mut errors = 0usize;

    for f in &files {
        let text = std::fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?;
        match Loadout::from_json(&text) {
            Ok(l) => {
                let mut issues = l.validate();
                if let Some(c) = &cat {
                    for e in &l.equipped {
                        match c.refresh_ref(&e.item) {
                            None => issues.push(d1_loadout::ValidationIssue {
                                severity: Severity::Warning,
                                code: "not_in_catalogue".into(),
                                message: format!(
                                    "{} ({}) is not in the catalogue",
                                    e.item.name, e.item.hash
                                ),
                            }),
                            Some(fresh) if fresh != e.item => {
                                issues.push(d1_loadout::ValidationIssue {
                                    severity: Severity::Warning,
                                    code: "stale_cache".into(),
                                    message: format!(
                                        "{} ({}) cached fields differ from the catalogue",
                                        e.item.name, e.item.hash
                                    ),
                                })
                            }
                            Some(_) => {}
                        }
                    }
                    issues.sort();
                }
                errors += issues.iter().filter(|i| i.severity == Severity::Error).count();
                report.push(serde_json::json!({
                    "file": f.display().to_string(),
                    "name": l.name,
                    "ok": !issues.iter().any(|i| i.severity == Severity::Error),
                    "issues": issues,
                }));
                if !cli.json {
                    let mark = if issues.iter().any(|i| i.severity == Severity::Error) {
                        "FAIL"
                    } else {
                        "ok  "
                    };
                    println!("{mark} {} - {}", f.display(), l.name);
                    for i in &issues {
                        println!("       {i}");
                    }
                }
            }
            Err(e) => {
                errors += 1;
                report.push(serde_json::json!({
                    "file": f.display().to_string(),
                    "ok": false,
                    "parse_error": e.to_string(),
                }));
                if !cli.json {
                    println!("FAIL {} - {e}", f.display());
                }
            }
        }
    }

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "files": files.len(),
                "errors": errors,
                "results": report,
            }))?
        );
    } else {
        println!("{} file(s), {errors} error(s)", files.len());
    }
    if errors > 0 {
        std::process::exit(2);
    }
    Ok(())
}

fn slots(cli: &Cli) -> Result<()> {
    if cli.json {
        let rows: Vec<_> = Slot::ALL
            .iter()
            .map(|s| {
                serde_json::json!({
                    "slot": s,
                    "name": s.display_name(),
                    "bucket_hash": s.bucket_hash(),
                    "combat": Slot::COMBAT.contains(s),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for s in Slot::ALL {
            println!(
                "{:>10}  {:<16} {}",
                s.bucket_hash(),
                s.display_name(),
                if Slot::COMBAT.contains(&s) { "combat" } else { "cosmetic" }
            );
        }
    }
    Ok(())
}

// --- clap value parsers -------------------------------------------------------------------------

fn parse_slot(s: &str) -> Result<Slot, String> {
    Slot::ALL
        .into_iter()
        .find(|x| {
            format!("{x:?}").eq_ignore_ascii_case(s) || x.display_name().eq_ignore_ascii_case(s)
        })
        .ok_or_else(|| {
            format!(
                "unknown slot {s:?}; one of {}",
                Slot::ALL.map(|x| format!("{x:?}")).join(", ")
            )
        })
}

fn parse_tier(s: &str) -> Result<Tier, String> {
    Tier::ALL
        .into_iter()
        .find(|x| format!("{x:?}").eq_ignore_ascii_case(s) || x.name().eq_ignore_ascii_case(s))
        .ok_or_else(|| format!("unknown tier {s:?}"))
}

fn parse_class(s: &str) -> Result<Class, String> {
    Class::ALL
        .into_iter()
        .find(|x| x.name().eq_ignore_ascii_case(s))
        .ok_or_else(|| format!("unknown class {s:?}"))
}

fn parse_damage(s: &str) -> Result<Damage, String> {
    [Damage::None, Damage::Kinetic, Damage::Arc, Damage::Solar, Damage::Void]
        .into_iter()
        .find(|x| x.name().eq_ignore_ascii_case(s))
        .ok_or_else(|| format!("unknown damage type {s:?}"))
}
