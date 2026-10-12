// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-ghost` - the run companion (`design/d1-ghost.md`). The `d1-dash` read panels plus a control
//! console whose every write is registered, gated by observed client state, and ledgered into the
//! run directory.
//!
//! ```text
//! d1-ghost                      # newest run under ./runs, GUI
//! d1-ghost runs/<stamp>         # a named run
//! d1-ghost --pid <PID>          # freeze this emulator pid around multi-byte code patches
//! d1-ghost registry             # print the poke registry (no client needed)
//! d1-ghost ledger <run>         # print a run's pokes.jsonl, with any dangling intents flagged
//! ```
//!
//! ⛔ Ghost is NEVER a launcher: it attaches to an existing run and spawns only the two whitelisted
//! tools (`d1guest.py`, `session-params.py`). Containment stays in `run-boot.sh`.

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use d1_trace::dash;
use d1_trace::ghost::{ledger, registry};

#[derive(Parser)]
#[command(name = "d1-ghost", about = "Run companion: d1-dash panels + a gated, ledgered poke console")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run directory to attach to. Defaults to the newest under `./runs` with a RUN-INFO.txt.
    run: Option<PathBuf>,

    /// The emulator pid to freeze (SIGSTOP/SIGCONT) around a multi-byte code patch. Auto-detected
    /// from `pgrep -f "[s]hadps4 -p "` if omitted; if none is found, code patches REFUSE rather
    /// than tear the client.
    #[arg(long)]
    pid: Option<u32>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the window on a run directory.
    #[cfg(feature = "gui")]
    Gui { run: Option<PathBuf> },
    /// Print the poke registry - name, class, gate, citation. No client needed. This is the honest
    /// list of what is still synthetic; it shrinks as pokes are earned onto the wire.
    Registry,
    /// Print a run's `pokes.jsonl`, flagging any dangling intents (a write with no result).
    Ledger { run: Option<PathBuf> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        #[cfg(feature = "gui")]
        Some(Command::Gui { run }) => open(resolve(run)?, cli.pid),
        Some(Command::Registry) => print_registry(),
        Some(Command::Ledger { run }) => print_ledger(resolve(run)?),
        #[cfg(feature = "gui")]
        None => open(resolve(cli.run)?, cli.pid),
        #[cfg(not(feature = "gui"))]
        None => print_registry(),
    }
}

fn print_registry() -> Result<()> {
    println!(
        "d1-ghost registry - {} entries still synthetic, in RECIPE ORDER (each shrinks when served)\n\
         the sequence to reach in-world: launch at orbit -> suppress cine -> force the ladder ->\n\
         the two code hooks -> spawn -> clear the fade. Each poke's GATE is its valid window.\n",
        registry::registry().len()
    );
    for p in registry::registry() {
        let window = match (p.gate.min_rung, p.gate.max_rung) {
            (Some(a), Some(b)) if a == b => format!("rung {a}"),
            (Some(a), Some(b)) => format!("rung {a}-{b}"),
            (Some(a), None) => format!("rung>={a}"),
            (None, Some(b)) => format!("rung<={b}"),
            (None, None) => "any rung".to_string(),
        };
        println!(
            "  {}. {:<15} [{:<7}] window: {:<10} L{}",
            p.step, p.name, p.class.as_str(), window, p.gate.min_liveness
        );
        println!("     {}", p.summary);
        println!("     cite: {}", p.citation);
    }
    Ok(())
}

fn print_ledger(dir: PathBuf) -> Result<()> {
    let led = ledger::Ledger::for_run(&dir);
    let lines: Vec<ledger::Line> = led.read_all().into_iter().flatten().collect();
    if lines.is_empty() {
        println!("no ledgered pokes in {} (a shell poke bypassing ghost would not appear here)", dir.display());
        return Ok(());
    }
    for l in &lines {
        println!("{}", serde_json::to_string(l)?);
    }
    let dangling = ledger::dangling(&lines);
    if !dangling.is_empty() {
        eprintln!("\n⛔ {} DANGLING INTENT(S) - a write started and left no result:", dangling.len());
        for d in dangling {
            eprintln!("   seq {} '{}' [{}] @ {}", d.seq, d.name, d.target, d.ts);
        }
    }
    Ok(())
}

fn resolve(run: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = run {
        if !p.is_dir() {
            bail!("{} is not a directory", p.display());
        }
        return Ok(p);
    }
    for r in ["runs", "../runs", "../../runs"] {
        let root = PathBuf::from(r);
        if root.is_dir() {
            if let Some(p) = dash::newest_run(&root) {
                eprintln!("d1-ghost: no run given; attaching to the newest - {}", p.display());
                return Ok(p);
            }
        }
    }
    bail!("no run directory given and none found under ./runs - pass one explicitly")
}

/// Ascend for the repository root (the directory holding both `capture-rig/` and `runs/`), so the
/// executor's tool paths resolve. `$D1_REPO` overrides.
fn find_repo_root(run_dir: &Path) -> PathBuf {
    let is_root = |p: &Path| p.join("capture-rig").is_dir() && p.join("runs").is_dir();
    if let Ok(repo) = std::env::var("D1_REPO") {
        let p = PathBuf::from(repo);
        if is_root(&p) {
            return p;
        }
    }
    let mut starts = vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
    if let Ok(abs) = std::fs::canonicalize(run_dir) {
        starts.push(abs);
    }
    for start in starts {
        let mut cur: Option<&Path> = Some(&start);
        while let Some(p) = cur {
            if is_root(p) {
                return p.to_path_buf();
            }
            cur = p.parent();
        }
    }
    PathBuf::from(".")
}

/// Best-effort emulator pid via the project's documented incantation. The `[s]` class stops the
/// pattern matching the pgrep shell itself.
fn detect_emu_pid() -> Option<u32> {
    let out = ProcessCommand::new("pgrep").arg("-f").arg("[s]hadps4 -p ").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().next()?.trim().parse::<u32>().ok()
}

#[cfg(feature = "gui")]
fn open(dir: PathBuf, pid: Option<u32>) -> Result<()> {
    let repo_root = find_repo_root(&dir);
    let emu_pid = pid.or_else(detect_emu_pid);
    if emu_pid.is_none() {
        eprintln!(
            "d1-ghost: no emulator pid (pass --pid or start the emulator); multi-byte code patches \
             will REFUSE rather than risk a torn write."
        );
    }
    d1_trace::ghost::gui::run(dir, repo_root, emu_pid)
}
