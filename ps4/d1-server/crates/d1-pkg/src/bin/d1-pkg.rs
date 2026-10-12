// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-pkg` - read and verify Destiny 1 PS4 `.pkg` containers. **Read-only on `dumps/`.**
//!
//! The subcommand that matters most is `verify-plain`: it is how a decompressed block is checked
//! against evidence the decompressor never saw. See `capture-rig/oodle/README.md` for the pipeline.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use d1_pkg::manifest::{self, Policy, Select};
use d1_pkg::*;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

// Defaults come from the build registry (`capture-rig/builds.json`, `$D1_BUILD`). The pinned
// controls are goldens of the 01.29 reference estate: run them with `D1_BUILD=01.29`.

#[derive(Parser)]
#[command(name = "d1-pkg", about = "Destiny 1 PS4 .pkg container reader (read-only)")]
struct Cli {
    /// Directory holding the package estate (default: the selected build's packages).
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Header fields and table integrity for one package.
    Info { pkg: String },
    /// Block table, with the stored-bytes SHA-1 checked against each record.
    Blocks {
        pkg: String,
        /// Only blocks whose stored bytes are Oodle-compressed.
        #[arg(long)]
        compressed_only: bool,
        #[arg(long)]
        json: bool,
    },
    /// Entry table, optionally restricted to one block.
    Entries {
        pkg: String,
        #[arg(long)]
        block: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    /// Write a block's stored (possibly still-compressed) bytes to a file.
    ///
    /// This is the input to `capture-rig/oodle/d1-oodle-harness.py`.
    ExtractBlock {
        pkg: String,
        index: u32,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Cross-check a *decompressed* block against the entry table's declared sizes.
    ///
    /// The block record's SHA-1 covers the stored bytes, so it says nothing about decompressed
    /// output. This does: most payloads open with a u64 length that the entry table independently
    /// declares. Exit status is non-zero if agreement falls below `--min-agree`.
    VerifyPlain {
        pkg: String,
        index: u32,
        /// The decompressed block, as produced by the Oodle harness.
        #[arg(long)]
        plain: PathBuf,
        /// Fail below this agreement rate among length-headered entries.
        #[arg(long, default_value = "1.0")]
        min_agree: f64,
        #[arg(long)]
        json: bool,
    },
    /// Table SHA-1s and layout arithmetic across the whole estate.
    Verify {
        /// Also hash every block's stored bytes. Slow: reads the entire corpus.
        #[arg(long)]
        blocks: bool,
    },
    /// Corpus-wide statistics, including the compressed-block share.
    Stats,

    /// Build the CDN content manifest from the package headers on disk.
    ///
    /// Every emitted value (id, GUID) is read out of a header we own; nothing is invented. The
    /// default policy reproduces the served `capture-rig/manifests/full_en.bin`: neutral plus one
    /// language, all generations, engine-registered ids dropped.
    ///
    /// ⚠ Serving the result needs the lab manifest patch, which clears the RSA-envelope check.
    /// Anything the client does past that point is `synthetic_ancestor`.
    Manifest {
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, value_enum, default_value = "all-generations")]
        policy: Policy,
        /// Keep locale 0 (neutral) plus this language: en fr it de sp jpn pt, or `all` for no
        /// filter. Defaults to `en` for `all-generations` and to `all` for the legacy policies,
        /// which must stay unfiltered to match the prototype generator byte for byte.
        #[arg(long)]
        lang: Option<String>,
        /// Restrict to package ids, hex, e.g. 0172. Repeatable. Applied before the policy.
        #[arg(long)]
        only: Vec<String>,
        /// Top-level manifest guid (field 5). Must strncmp-match engine config key 101.
        #[arg(long, default_value = "")]
        guid: String,
        /// What engine configuration key 101 is set to. Both empty is the default and matches.
        #[arg(long, default_value = "")]
        config_guid: String,
        /// Emit even above the descriptor cap. The client rejects such a body outright with
        /// `array overflow`; this exists only to byte-compare the `all` policy.
        #[arg(long)]
        allow_overflow: bool,
        /// Select and report, write nothing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        verbose: bool,
    },

    /// Pinned controls. **Exit 2 means the output of this tool may not be cited.**
    Controls {
        /// The served manifest to byte-compare against (default: the selected build's).
        /// Untracked (it is derived from retail content), so its absence is expected on a clone
        /// and the control says so rather than passing quietly.
        #[arg(long)]
        served: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let dir = cli.dir.clone().unwrap_or_else(|| d1_build::selected().packages_path());
    match cli.cmd {
        Cmd::Info { pkg } => {
            let est = Estate::load(&dir)?;
            let p = find(&est, &pkg)?;
            let h = &p.header;
            println!("{}", p.name);
            println!("  magic {:#06x}  version {}  id {}  gen {}  locale {}", h.magic, h.version, h.id, h.gen, h.locale);
            println!("  guid {:#018x}  timestamp {}  build {}  rev {}", h.guid, h.timestamp, h.build, h.revision);
            println!("  build string: {}", h.build_string);
            println!("  entries {:<6} @ {:<10}  blocks {:<6} @ {:<10}  symbols {:<6} @ {}", h.n_ent, h.off_ent, h.n_blk, h.off_blk, h.n_sym, h.off_sym);
            println!("  declared size {} / actual {}", h.declared_size, p.file_size);
            let c = p.verify_tables();
            println!("  tables: entry={:?} block={:?} symbol={:?} size={}  layout_ok={}", c.entry, c.block, c.symbol, c.size, p.layout_ok());
            let comp = p.blocks().filter(|b| b.compressed()).count();
            println!("  blocks compressed: {}/{}", comp, h.n_blk);
        }

        Cmd::Blocks { pkg, compressed_only, json } => {
            let est = Estate::load(&dir)?;
            let p = find(&est, &pkg)?;
            let mut rows = Vec::new();
            for b in p.blocks() {
                if compressed_only && !b.compressed() {
                    continue;
                }
                let ok = est.read_block_stored(p, &b).is_ok();
                rows.push((b, ok));
            }
            if json {
                let v: Vec<_> = rows.iter().map(|(b, ok)| serde_json::json!({"block": b, "stored_sha1_ok": ok})).collect();
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                for (b, ok) in &rows {
                    println!("blk {:<5} off {:<10} stored {:<8} gen {:<3} flags {:#06x} {:<12} sha1 {} {}",
                        b.index, b.offset, b.stored_size, b.gen, b.flags,
                        if b.compressed() { "compressed" } else { "raw" },
                        hex(&b.sha1), if *ok { "OK" } else { "MISMATCH" });
                }
                let bad = rows.iter().filter(|(_, ok)| !ok).count();
                println!("{} blocks, {} stored-SHA-1 mismatches", rows.len(), bad);
            }
        }

        Cmd::Entries { pkg, block, json } => {
            let est = Estate::load(&dir)?;
            let p = find(&est, &pkg)?;
            let list: Vec<Entry> = p
                .entries()
                .filter(|e| e.size() != 0 && block.is_none_or(|b| e.block() == b))
                .collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for e in &list {
                    println!("idx {:<6} tag {:08x} class {:08x} u04 {:08x} blk {:<5} off {:<8} size {:<9} end {}",
                        e.index, make_tag(p.id(), e.index), e.class_tag, e.u04,
                        e.block(), e.block_off(), e.size(), e.block_off() + e.size());
                }
                println!("{} entries with data", list.len());
            }
        }

        Cmd::ExtractBlock { pkg, index, out } => {
            let est = Estate::load(&dir)?;
            let p = find(&est, &pkg)?;
            let b = p.block(index).context("block index out of range")?;
            let data = est.read_block_stored(p, &b)?;
            std::fs::File::create(&out)?.write_all(&data)?;
            let owner = est.owner(p, &b).map(|o| o.name.clone()).unwrap_or_default();
            println!("block {} of {}: {} bytes, {}, stored in {} at offset {}",
                index, p.name, data.len(),
                if b.compressed() { "Oodle-compressed" } else { "raw" }, owner, b.offset);
            println!("stored SHA-1 verified against the block record: {}", hex(&b.sha1));
            println!("wrote {}", out.display());
        }

        Cmd::VerifyPlain { pkg, index, plain, min_agree, json } => {
            let est = Estate::load(&dir)?;
            let p = find(&est, &pkg)?;
            let data = std::fs::read(&plain).with_context(|| format!("reading {}", plain.display()))?;
            let checks = check_self_lengths(p, index, &data);

            // Only headered entries are scorable; the other kinds carry no length to compare.
            let n = |k: EntryKind| checks.iter().filter(|c| c.kind == k).count();
            let headered = n(EntryKind::Headered);
            let agree = checks.iter().filter(|c| c.agrees).count();
            let failed: Vec<_> = checks
                .iter()
                .filter(|c| c.kind == EntryKind::Headered && !c.agrees)
                .collect();
            let rate = if headered == 0 { 1.0 } else { agree as f64 / headered as f64 };

            if json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "package": p.name, "block": index, "plain_bytes": data.len(),
                    "entries": checks.len(), "headered": headered, "agree": agree,
                    "failed": failed.len(), "rate": rate,
                    "raw_blob": n(EntryKind::RawBlob),
                    "self_classed": n(EntryKind::SelfClassed),
                    "too_small": n(EntryKind::TooSmall),
                    "checks": checks,
                }))?);
            } else {
                println!("{} block {} vs {} ({} bytes)", p.name, index, plain.display(), data.len());
                if data.len() != BLOCK_PLAIN_SIZE {
                    println!("  note: expected {} bytes for a full block", BLOCK_PLAIN_SIZE);
                }
                println!("  entries mapping into this block: {}", checks.len());
                println!("    headered (scored) {headered}   raw blobs {}   self-classed {}   too small {}",
                    n(EntryKind::RawBlob), n(EntryKind::SelfClassed), n(EntryKind::TooSmall));
                println!("  self-length agreement: {}/{} ({:.2}%)", agree, headered, rate * 100.0);
                for c in failed.iter().take(20) {
                    println!("    FAIL idx {:<6} class {:08x} off {:<8} declared {:<9} found {:#x}{}",
                        c.entry, c.class_tag, c.offset, c.declared, c.found,
                        if c.spills { "  [spills into next block]" } else { "" });
                }
            }
            if rate < min_agree {
                anyhow::bail!("self-length agreement {:.4} below --min-agree {:.4}", rate, min_agree);
            }
        }

        Cmd::Verify { blocks } => {
            let est = Estate::load(&dir)?;
            let (mut ok, mut bad) = (0usize, 0usize);
            let (mut bok, mut bbad) = (0usize, 0usize);
            for p in &est.packages {
                let c = p.verify_tables();
                if c.all_ok() && p.layout_ok() {
                    ok += 1;
                } else {
                    bad += 1;
                    println!("FAIL {} {:?} layout_ok={}", p.name, c, p.layout_ok());
                }
                if blocks {
                    for b in p.blocks() {
                        match est.read_block_stored(p, &b) {
                            Ok(_) => bok += 1,
                            Err(e) => {
                                bbad += 1;
                                if bbad <= 20 {
                                    println!("FAIL {} {}", p.name, e);
                                }
                            }
                        }
                    }
                }
            }
            println!("packages: {ok} ok, {bad} failed");
            if blocks {
                println!("blocks: {bok} stored-SHA-1 ok, {bbad} failed");
            }
            if bad > 0 || bbad > 0 {
                anyhow::bail!("{} package and {} block failures", bad, bbad);
            }
        }

        Cmd::Stats => {
            let est = Estate::load(&dir)?;
            let (mut nb, mut nc, mut ne, mut ns) = (0usize, 0usize, 0usize, 0usize);
            let (mut raw_max, mut comp_max) = (0u32, 0u32);
            for p in &est.packages {
                ne += p.entries().filter(|e| e.size() != 0).count();
                ns += p.header.n_sym as usize;
                for b in p.blocks() {
                    nb += 1;
                    if b.compressed() {
                        nc += 1;
                        comp_max = comp_max.max(b.stored_size);
                    } else {
                        raw_max = raw_max.max(b.stored_size);
                    }
                }
            }
            println!("packages {}  blocks {}  entries-with-data {}  symbols {}", est.packages.len(), nb, ne, ns);
            println!("compressed blocks {}/{} ({:.1}%)", nc, nb, nc as f64 * 100.0 / nb as f64);
            println!("largest raw block {raw_max} (BLOCK_PLAIN_SIZE = {BLOCK_PLAIN_SIZE})");
            println!("largest compressed block {comp_max}");
        }

        Cmd::Manifest {
            out,
            policy,
            lang,
            only,
            guid,
            config_guid,
            allow_overflow,
            dry_run,
            verbose,
        } => {
            let lang = match lang.as_deref().or(policy.default_lang()) {
                None | Some("all") => None,
                Some(t) => Some(
                    manifest::locale_code(t)
                        .with_context(|| format!("unknown language {t:?}; known: en fr it de sp jpn pt, or `all`"))?,
                ),
            };
            let only = if only.is_empty() {
                None
            } else {
                let mut s = BTreeSet::new();
                for x in &only {
                    s.insert(
                        u16::from_str_radix(x.trim_start_matches("0x"), 16)
                            .with_context(|| format!("--only {x:?} is not a hex package id"))?,
                    );
                }
                Some(s)
            };
            let sel = Select { policy, only, lang };
            // dry-run reports the selection without the cap, because nothing is written and so
            // nothing can be handed to a client.
            let built = manifest::build(&dir, &sel, &guid, &config_guid, allow_overflow || dry_run)?;

            if verbose {
                for r in &built.rows {
                    println!(
                        "   id=0x{:04x} gen={} locale={} guid={:#018x}  {}",
                        r.id,
                        r.gen,
                        manifest::locale_tag(r.locale).unwrap_or("?"),
                        r.guid,
                        r.stem
                    );
                }
            }
            let n = built.rows.len();
            if dry_run {
                println!(
                    "{n} packages, {} bytes, policy={}, lang={} (dry run, nothing written){}",
                    built.body.len(),
                    policy.tag(),
                    lang.and_then(manifest::locale_tag).unwrap_or("all"),
                    if n > manifest::PACKAGE_CAP {
                        format!(" [OVER the {} cap: the client would reject the whole body]", manifest::PACKAGE_CAP)
                    } else {
                        String::new()
                    }
                );
            } else {
                std::fs::write(&out, &built.body)
                    .with_context(|| format!("writing {}", out.display()))?;
                println!(
                    "{}: {n} packages, {} bytes, policy={}, lang={}, sha256 {}",
                    out.display(),
                    built.body.len(),
                    policy.tag(),
                    lang.and_then(manifest::locale_tag).unwrap_or("all"),
                    manifest::sha256_hex(&built.body)
                );
            }
        }

        Cmd::Controls { served, json } => {
            // An unreadable estate is not an error to bubble up: it means the controls could not
            // run, which is exactly what exit 2 is for. Bubbling it out as exit 1 would let a
            // caller that only checks `!= 2` treat an unverified tool as verified.
            let served = served.unwrap_or_else(|| d1_build::selected().manifest_path());
            let mut rows = manifest::controls(&dir, Some(&served)).unwrap_or_else(|e| {
                vec![manifest::ControlRow {
                    name: "estate readable".into(),
                    passed: false,
                    skipped: false,
                    detail: format!("{e}"),
                }]
            });
            rows.extend(block_owner_controls().unwrap_or_else(|e| {
                vec![manifest::ControlRow {
                    name: "stored-block ownership controls runnable".into(),
                    passed: false, skipped: false, detail: format!("{e}"),
                }]
            }));
            let pass = rows.iter().all(|r| r.passed);
            if json {
                let v: Vec<_> = rows
                    .iter()
                    .map(|r| serde_json::json!({"control": r.name, "passed": r.passed, "skipped": r.skipped, "detail": r.detail}))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({"passed": pass, "controls": v}))?
                );
            } else {
                for r in &rows {
                    println!(
                        "{}  {}{}",
                        if r.skipped { "SKIP" } else if r.passed { "PASS" } else { "FAIL" },
                        r.name,
                        if r.detail.is_empty() { String::new() } else { format!("  [{}]", r.detail) }
                    );
                }
                println!(
                    "\n{}",
                    if pass {
                        "ALL CONTROLS PASS"
                    } else {
                        "*** CONTROL FAILURE - the manifest generator's output may not be cited ***"
                    }
                );
            }
            return Ok(if pass { ExitCode::SUCCESS } else { ExitCode::from(2) });
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn find<'a>(est: &'a Estate, name: &str) -> Result<&'a Package> {
    est.find(name)
        .or_else(|| est.find(&format!("{name}.pkg")))
        .with_context(|| format!("no package named {name}"))
}
