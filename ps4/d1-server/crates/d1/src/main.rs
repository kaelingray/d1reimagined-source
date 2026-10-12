// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1` - inspect, summarise and diff experiment runs.
//!
//! The project's scientific instrument is run-A-vs-run-B comparison. Until now that was done by hand
//! with `grep`, and because `runs/` is gitignored, evidence only survived if someone copied it into a
//! document. This makes the comparison a command and the promotion a command.
//!
//! Design rules it follows deliberately:
//!   * **CLI first.** Everything has `--json`, so it is scriptable and CI-able. A TUI, if it ever
//!     exists, is a view over this - never the only path.
//!   * **Deterministic output.** Sorted collections throughout, so two runs of one experiment produce
//!     byte-identical summaries and a diff means something really changed.
//!   * **It does not run anything.** Running is the launcher's job
//!     (`capture-rig/run-contained-emulator.sh`); this only reads what a run left behind.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use d1_run::{diff, list_runs, summarize_run, Quiet, QuietThresholds, RunSummary};

mod crash;
mod serve;

#[derive(Parser)]
#[command(name = "d1", about = "Inspect, summarise and diff Destiny 1 preservation runs")]
struct Cli {
    /// Directory holding run directories.
    #[arg(long, default_value = "runs", global = true)]
    runs_dir: PathBuf,

    /// Machine-readable output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List runs with a one-line headline each.
    Runs,
    /// Summarise a run: hostnames attempted, gates, guest diagnostics, faults, provenance.
    Show {
        /// Run name or path. Omit for the most recent.
        run: Option<String>,
    },
    /// Diff two runs. With one argument, diffs it against the previous run.
    Diff { a: String, b: Option<String> },
    /// Serve the run inspector as a web page on loopback.
    ///
    /// This is the GUI. Served HTML rather than a native toolkit, deliberately: zero extra build
    /// targets, no framework, works from a phone over `ssh -L`, screenshots into docs, and it renders
    /// the same summaries the CLI prints so the two cannot drift.
    Serve {
        /// Address to bind. Keep it on loopback; use `ssh -L` for remote access.
        #[arg(long, default_value = "127.0.0.1:8787")]
        bind: std::net::SocketAddr,
    },
    /// Print a markdown block suitable for pasting into a status or design doc.
    ///
    /// `runs/` is gitignored, so this is how evidence survives.
    Promote {
        run: Option<String>,
    },
    /// Assert that a run still passes every gate the project has ALREADY passed.
    ///
    /// Run it BEFORE reading anything else out of a run: a finding from a run that fails this is
    /// not a finding, it is a confound. Exit status is nonzero if any previously-passed gate
    /// regressed, so it can guard an experiment. Port of `capture-rig/gate-check.py`.
    Gate {
        /// Run name or path. Omit for the most recent run directory.
        run: Option<String>,
    },
    /// Harvest and SYMBOLIZE the guest's own crash report into `<run>/guest-crash-report`.
    ///
    /// The client writes its richest diagnostic - backtrace, halt reason, bootflow state, the
    /// frame at the halt - into a folder the NEXT BOOT DESTROYS. Run this immediately after any
    /// halt. Symbolization anchors every frame on a real function prologue (never a guessed
    /// va-N) and names functions from the diagnostic strings they reference. Port of
    /// `capture-rig/harvest-crash.py`.
    Crash {
        /// Run directory to harvest into (path or name suffix). Omit for the most recent run.
        run: Option<String>,
        /// Comma-separated guest vaddrs to symbolize instead of harvesting,
        /// e.g. `--stack 0x94702F,0x798400`.
        #[arg(long)]
        stack: Option<String>,
    },
    /// Run this tool's own controls; exits 2 if any fails.
    Controls,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Runs => cmd_runs(&cli),
        Command::Show { run } => cmd_show(&cli, run.as_deref()),
        Command::Diff { a, b } => cmd_diff(&cli, a, b.as_deref()),
        Command::Promote { run } => cmd_promote(&cli, run.as_deref()),
        Command::Serve { bind } => serve::run(cli.runs_dir.clone(), *bind),
        Command::Gate { run } => cmd_gate(&cli, run.as_deref()),
        Command::Crash { run, stack } => cmd_crash(&cli, run.as_deref(), stack.as_deref()),
        Command::Controls => cmd_controls(),
    }
}

/// The probe decoder the gate runs under: the native port in `d1-trace/src/probe_decode.rs`, the
/// ONE Rust description of the 232-byte tracer record. `d1-run` cannot name it (`d1-trace`
/// depends on `d1-run`), so the CLI - above both - wires them together. Nothing here describes
/// the record layout, and nothing in the shipped binary spawns Python.
fn native_probe_decode(blob: &[u8]) -> Result<Vec<d1_run::gate::ProbeSample>, String> {
    let records = d1_trace::probe_decode::read_records(blob, "account-trace.bin")?;
    let rax = d1_trace::probe_decode::REGISTER_NAMES
        .iter()
        .position(|n| *n == "rax")
        .expect("rax is in the register catalogue");
    Ok(records
        .into_iter()
        .map(|r| d1_run::gate::ProbeSample { probe_id: r.probe_id, rax: r.regs[rax] })
        .collect())
}

fn cmd_gate(cli: &Cli, run: Option<&str>) -> Result<()> {
    let dir = resolve(cli, run)?;
    let report = d1_run::gate::gate_check(&dir, native_probe_decode);
    if let Some(note) = &report.probe_note {
        // Loud, but on stderr: stdout stays byte-diffable against the prototype.
        eprintln!("warning: probe lane empty because of an error: {note}");
    }
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", d1_run::gate::render_text(&report));
    }
    if report.regressed {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_crash(cli: &Cli, run: Option<&str>, stack: Option<&str>) -> Result<()> {
    if let Some(stack) = stack {
        let addrs = crash::parse_stack_list(stack).map_err(anyhow::Error::msg)?;
        let eboot = crash::load_eboot_if_needed(&addrs).map_err(anyhow::Error::msg)?;
        for a in addrs {
            println!("{}", crash::render(&crash::symbolize(&eboot, a)));
        }
        return Ok(());
    }
    // The prototype's order, kept: look for a crash first, so "nothing to harvest" needs neither
    // a run directory nor the eboot.
    let root = crash::crash_root().map_err(anyhow::Error::msg)?;
    let Some(cf) = crash::newest_crash_folder(&root) else {
        println!("{}", crash::NO_CRASH_MSG);
        return Ok(());
    };
    let run_dir = resolve(cli, run)?;
    let text = crash::harvest(&run_dir, &cf).map_err(anyhow::Error::msg)?;
    print!("{text}");
    Ok(())
}

fn cmd_controls() -> Result<()> {
    let mut failed = 0usize;
    let mut score = |name: &str, result: Result<(), String>| match result {
        Ok(()) => println!("[ PASS ] {name}"),
        Err(e) => {
            failed += 1;
            println!("[ FAIL ] {name}");
            println!("           {e}");
        }
    };
    // The gate controls run with the REAL decoder injected - this is the end-to-end pipeline the
    // shipped `d1 gate` uses (d1-run's own tests run the same list under a pinned-answer double).
    for (name, control) in d1_run::gate::controls::all() {
        score(name, control(native_probe_decode));
    }
    for (name, control) in crash::controls::all() {
        score(name, control());
    }
    if failed > 0 {
        println!("\n{failed} control(s) FAILED - this tool's output may not be cited");
        std::process::exit(2);
    }
    println!("\nall controls passed");
    Ok(())
}

/// Resolve a run reference: an explicit path, a directory name, or the newest run.
fn resolve(cli: &Cli, run: Option<&str>) -> Result<PathBuf> {
    let runs = list_runs(&cli.runs_dir)
        .with_context(|| format!("no runs directory at {}", cli.runs_dir.display()))?;
    match run {
        Some(r) => {
            let direct = Path::new(r);
            if direct.is_dir() {
                return Ok(direct.to_path_buf());
            }
            // Suffix match so a partial name works.
            let hits: Vec<_> = runs
                .iter()
                .filter(|p| p.file_name().map(|n| n.to_string_lossy().contains(r)).unwrap_or(false))
                .collect();
            match hits.len() {
                0 => bail!("no run matching '{r}' in {}", cli.runs_dir.display()),
                1 => Ok(hits[0].clone()),
                _ => bail!(
                    "'{r}' matches {} runs; be more specific:\n  {}",
                    hits.len(),
                    hits.iter()
                        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("\n  ")
                ),
            }
        }
        None => {
            // "Newest" means newest by timestamp-shaped name. `runs/` also holds scratch
            // directories (sweep-*, snapshots, measurements) that sort after the digits and
            // would otherwise win the lexical sort forever.
            runs.iter()
                .rev()
                .find(|p| {
                    p.file_name()
                        .map(|n| is_timestamp_named(&n.to_string_lossy()))
                        .unwrap_or(false)
                })
                .or_else(|| runs.last())
                .cloned()
                .context("no runs found")
        }
    }
}

/// `YYYYMMDD-HHMMSS-...`: eight digits, a hyphen, six digits.
fn is_timestamp_named(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 15
        && b[..8].iter().all(|c| c.is_ascii_digit())
        && b[8] == b'-'
        && b[9..15].iter().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod resolve_tests {
    use super::is_timestamp_named;

    #[test]
    fn timestamp_names_beat_scratch_names_for_newest() {
        assert!(is_timestamp_named("20260811-050950-GEARED-385-VISUALS-X11"));
        assert!(!is_timestamp_named("sweep-sub0-5"));
        assert!(!is_timestamp_named("snapshots"));
        assert!(!is_timestamp_named("measurements"));
        assert!(!is_timestamp_named("2026081-050950-short"));
    }
}

#[cfg(test)]
mod native_decoder_tests {
    /// Every gate control, under the REAL `d1-trace` decoder - the exact wiring `d1 gate` and
    /// `d1 controls` ship with. d1-run's own tests run the same list under its pinned-answer
    /// double; this is the half the dependency direction keeps out of that crate.
    #[test]
    fn gate_controls_pass_with_the_native_decoder() {
        for (name, control) in d1_run::gate::controls::all() {
            if let Err(e) = control(super::native_probe_decode) {
                panic!("{name}: {e}");
            }
        }
    }

    /// The native decoder and d1-run's pinned-answer double must agree on the excerpt - the pin
    /// is only honest while the real decoder reproduces it.
    #[test]
    fn pinned_double_matches_the_native_decoder() {
        let excerpt = include!("../../d1-run/src/gate_synthetic.rs");
        let native = super::native_probe_decode(excerpt).expect("native decoder accepts excerpt");
        let pinned = d1_run::gate::controls::pinned_excerpt_decode(excerpt).unwrap();
        assert_eq!(native, pinned, "the pinned answer drifted from the canonical decode");
    }
}

fn cmd_runs(cli: &Cli) -> Result<()> {
    let runs = list_runs(&cli.runs_dir)?;
    if cli.json {
        let all: Vec<RunSummary> = runs.iter().filter_map(|d| summarize_run(d).ok()).collect();
        println!("{}", serde_json::to_string_pretty(&all)?);
        return Ok(());
    }
    if runs.is_empty() {
        println!("no runs in {}", cli.runs_dir.display());
        return Ok(());
    }
    for dir in runs {
        match summarize_run(&dir) {
            Ok(s) => println!("{}", s.headline()),
            Err(e) => println!("{} · unreadable: {e}", dir.display()),
        }
    }
    Ok(())
}

fn cmd_show(cli: &Cli, run: Option<&str>) -> Result<()> {
    let dir = resolve(cli, run)?;
    let s = summarize_run(&dir)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&s)?);
        return Ok(());
    }

    println!("{}\n", s.headline());

    // Provenance first - including whether this run can support a determinism claim at all.
    if !s.provenance.info.is_empty() {
        println!("provenance:");
        for (k, v) in &s.provenance.info {
            println!("  {k:<18} {v}");
        }
        match s.provenance.containment_ok {
            Some(true) => println!("  containment        asserted and PASSED"),
            Some(false) => println!("  containment        ** ASSERTION FAILED **"),
            None => println!("  containment        (no proof recorded)"),
        }
        match s.provenance.quiet(&QuietThresholds::default()) {
            Quiet::Yes => println!("  host quiet         YES (within pre-declared thresholds)"),
            Quiet::No(reasons) => {
                println!("  host quiet         ** NO **");
                for r in &reasons {
                    println!("                       {r}");
                }
                println!("                       a determinism claim from this run is [O], not [C]");
            }
            Quiet::Unknown => println!(
                "  host quiet         UNKNOWN (no samples) - determinism claims are [O], not [C]"
            ),
        }
        println!();
    }

    section_set("hostnames attempted", &s.hostnames);
    section_set("http calls", &s.http_calls);
    section_set("milestones", &s.milestones);

    if !s.shadnet.is_empty() {
        println!("shadnet commands:");
        for (cmd, served) in &s.shadnet {
            println!("  {cmd:<32} {}", if *served { "served" } else { "UNIMPLEMENTED" });
        }
        println!();
    }

    if !s.guest_subsystems.is_empty() {
        println!("guest diagnostics by subsystem:");
        for (sub, n) in &s.guest_subsystems {
            println!("  {sub:<36} {n}");
        }
        println!();
    }

    if !s.faults.is_empty() {
        println!("faults:");
        for f in &s.faults {
            println!("  {f}");
        }
        println!();
    }

    println!("np identity: {} SIGNED_OUT polls{}", s.signed_out_polls,
        if s.signed_out_polls == 0 { "  (identity gate OPEN)" } else { "" });

    // The gate ladder last, because it is the conclusion: how far this run got.
    if !s.boot_ladder.is_empty() {
        println!("\nboot-flow ladder ({} transitions):", s.boot_ladder.len());
        for (state, reason) in &s.boot_ladder {
            let mark = if reason.starts_with("error.") { "✗" } else { " " };
            println!("  {mark} {state}   ({reason})");
        }
        if let Some(d) = &s.deepest_state {
            println!("\ndeepest gate: {d}");
        }
        if let Some(r) = &s.terminal_reason {
            println!("terminal reason: {r}");
        }
        if let Some(h) = &s.guest_halt {
            println!("GUEST HALT (the engine's own words): {h}");
        }
    }
    Ok(())
}

fn section_set(title: &str, set: &std::collections::BTreeSet<String>) {
    if set.is_empty() {
        return;
    }
    println!("{title} ({}):", set.len());
    for item in set {
        println!("  {item}");
    }
    println!();
}

fn cmd_diff(cli: &Cli, a: &str, b: Option<&str>) -> Result<()> {
    // One argument means "this run vs the one before it" - the common case.
    let (dir_a, dir_b) = match b {
        Some(b) => (resolve(cli, Some(a))?, resolve(cli, Some(b))?),
        None => {
            let target = resolve(cli, Some(a))?;
            let runs = list_runs(&cli.runs_dir)?;
            let idx = runs.iter().position(|p| *p == target).context("run not in runs dir")?;
            if idx == 0 {
                bail!("{} is the earliest run; nothing to diff against", target.display());
            }
            (runs[idx - 1].clone(), target)
        }
    };

    let sa = summarize_run(&dir_a)?;
    let sb = summarize_run(&dir_b)?;
    let entries = diff(&sa, &sb);

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!("--- {}\n+++ {}\n", sa.name, sb.name);
    if entries.is_empty() {
        println!("no differences - the runs are equivalent by every measure this tool extracts");
        return Ok(());
    }
    let mut current = String::new();
    for e in &entries {
        if e.category != current {
            println!("[{}]", e.category);
            current = e.category.clone();
        }
        match e.change.as_str() {
            "added" => println!("  + {}", e.item),
            "removed" => println!("  - {}", e.item),
            _ => println!(
                "  ~ {}: {} -> {}",
                e.item,
                e.before.as_deref().unwrap_or("?"),
                e.after.as_deref().unwrap_or("?")
            ),
        }
    }
    Ok(())
}

fn cmd_promote(cli: &Cli, run: Option<&str>) -> Result<()> {
    let dir = resolve(cli, run)?;
    let s = summarize_run(&dir)?;
    // Evidence class is decided by the *predicate*, not by whether samples merely exist.
    let quiet = s.provenance.quiet(&QuietThresholds::default());
    let evidence = if matches!(quiet, Quiet::Yes) { "[C]" } else { "[O]" };

    println!("### Run `{}` {evidence}\n", s.name);
    if let Some(id) = &s.game_identity {
        println!("- **Game:** {id}");
    }
    println!("- **Faults:** {}", s.faults.len());
    println!(
        "- **NP identity:** {} `SIGNED_OUT` polls{}",
        s.signed_out_polls,
        if s.signed_out_polls == 0 { " - gate open" } else { "" }
    );
    println!("- **Names attempted:** {}", s.hostnames.len());
    println!("- **Guest diagnostic lines:** {}", s.guest_lines.len());
    match s.provenance.containment_ok {
        Some(true) => println!("- **Containment:** asserted and passed"),
        Some(false) => println!("- **Containment:** ⚠ ASSERTION FAILED"),
        None => println!("- **Containment:** no proof recorded"),
    }
    match &quiet {
        Quiet::Yes => println!("- **Host quiet:** yes, within pre-declared thresholds"),
        Quiet::No(reasons) => {
            println!("- **Host quiet:** ⚠ NO - {}", reasons.join("; "));
            println!(
                "\n> The host was measurably not quiet during this run, so any determinism claim \
                 from it is `[O]`, not `[C]`, however clean the output looks."
            );
        }
        Quiet::Unknown => println!(
            "\n> No host-condition samples in this run's provenance, so any determinism claim from \
             it is `[O]`, not `[C]`."
        ),
    }
    if !s.hostnames.is_empty() {
        println!("\n```");
        for h in &s.hostnames {
            println!("{h}");
        }
        println!("```");
    }
    if !s.guest_lines.is_empty() {
        println!("\nGuest diagnostics:\n\n```");
        for l in &s.guest_lines {
            println!("{l}");
        }
        println!("```");
    }
    Ok(())
}
