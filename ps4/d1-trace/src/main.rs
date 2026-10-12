// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-trace` - open a `runs/<stamp>/` directory and view its four evidence streams on one
//! searchable, cross-correlated timeline. Read-only; see `historicaldocs/design/trace-viewer.md`.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use d1_trace::load_run;
use d1_trace::query::Query;

#[derive(Parser)]
#[command(name = "d1-trace", about = "Read-only unified four-stream trace viewer for D1 runs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run directory to open in the GUI (shorthand for `d1-trace gui <dir>`).
    run: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the native viewer on a run directory.
    #[cfg(feature = "gui")]
    Gui {
        run: PathBuf,
        /// Pre-populate the search box (same grammar as `search`).
        #[arg(long, default_value = "")]
        query: String,
        /// Start with the timeline narrowed to matches.
        #[arg(long)]
        matches_only: bool,
    },
    /// Print a summary of the loaded run (headless; used to verify ingest without a display).
    Summary { run: PathBuf },
    /// Run a search query headless and print matching events (verifies the query engine).
    Search {
        run: PathBuf,
        /// The query string, e.g. `probe:item_load_state_primary rax&0xffffffff==2`.
        query: String,
        /// Max rows to print.
        #[arg(long, default_value_t = 40)]
        limit: usize,
    },
    /// Decode a raw `account-trace.bin` (the native port of `capture-rig/decode-account-trace.py`;
    /// same output, same refusals, exit 2 on an undecodable trace).
    Decode {
        trace: PathBuf,
        /// Emit the machine document instead of the human explanation.
        #[arg(long)]
        json: bool,
    },
    /// Read captured `world_launch` commands from a `D1BL` blob sidecar
    /// (`account-trace.bin.blob`), and DIFF them to bound the activity-specific fields (the native
    /// port of `capture-rig/world-launch-command.py`; same output, same refusals, exit 2 on a
    /// refused sidecar).
    Blob {
        /// The `runs/<stamp>/account-trace.bin.blob` sidecar.
        blob: PathBuf,
        /// Hexdump blob N.
        #[arg(long, value_name = "N")]
        dump: Option<usize>,
        /// Diff two blobs; the differing regions bracket the world/activity fields.
        #[arg(long, num_args = 2, value_names = ["A", "B"])]
        diff: Option<Vec<usize>>,
        /// Write blob N's raw command bytes to --out.
        #[arg(long, value_name = "N")]
        extract: Option<usize>,
        /// Output file for --extract.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check the pinned invariants. **Exit 2 = a control FAILED and the output may not be cited.**
    Controls,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        #[cfg(feature = "gui")]
        Some(Command::Gui {
            run,
            query,
            matches_only,
        }) => open_gui(run, query, matches_only),
        Some(Command::Summary { run }) => cmd_summary(run),
        Some(Command::Search { run, query, limit }) => cmd_search(run, query, limit),
        Some(Command::Decode { trace, json }) => cmd_decode(trace, json),
        Some(Command::Blob {
            blob,
            dump,
            diff,
            extract,
            out,
        }) => cmd_blob(blob, dump, diff, extract, out),
        Some(Command::Controls) => d1_trace::controls::run(),
        None => match cli.run {
            #[cfg(feature = "gui")]
            Some(run) => open_gui(run, String::new(), false),
            #[cfg(not(feature = "gui"))]
            Some(run) => cmd_summary(run),
            None => {
                eprintln!("usage: d1-trace <run-dir>   |   d1-trace summary|search <run-dir> …");
                std::process::exit(2);
            }
        },
    }
}

#[cfg(feature = "gui")]
fn open_gui(run: PathBuf, query: String, matches_only: bool) -> Result<()> {
    let trace = load_run(&run)?;
    d1_trace::gui::run_with(trace, d1_trace::gui::Startup { query, matches_only })
}

/// The Python tool's contract: decode errors are `error: …` on stderr with exit 2; success prints
/// the document on stdout and exits 0.
fn cmd_decode(trace: PathBuf, json: bool) -> Result<()> {
    use std::io::Write as _;
    let blob = match std::fs::read(&trace) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", trace.display());
            std::process::exit(2);
        }
    };
    let records = match d1_trace::probe_decode::read_records(&blob, &trace.display().to_string()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let out = if json {
        let mut s = d1_trace::probe_decode::to_json(&records);
        s.push('\n');
        s
    } else {
        d1_trace::probe_decode::to_human(&records)
    };
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(out.as_bytes())?;
    Ok(())
}

/// The Python tool's contract, preserved: refusals are `world-launch-command: …` on stderr with
/// exit 2; success prints the document on stdout and exits 0. Mode precedence is the Python's:
/// `--diff`, then `--dump`, then `--extract`, else summarise. (An unreadable file also refuses
/// with exit 2 where the Python died with an uncaught traceback.)
fn cmd_blob(
    blob: PathBuf,
    dump: Option<usize>,
    diff: Option<Vec<usize>>,
    extract: Option<usize>,
    out: Option<PathBuf>,
) -> Result<()> {
    fn refuse(error: String) -> ! {
        eprintln!("world-launch-command: {error}");
        std::process::exit(2);
    }
    let data = match std::fs::read(&blob) {
        Ok(d) => d,
        Err(e) => refuse(format!("could not read {}: {e}", blob.display())),
    };
    let blobs = d1_trace::blob_decode::read_blobs(&data).unwrap_or_else(|e| refuse(e));
    let text = if let Some(indices) = diff {
        d1_trace::blob_decode::diff(&blobs, indices[0], indices[1]).unwrap_or_else(|e| refuse(e))
    } else if let Some(n) = dump {
        d1_trace::blob_decode::dump(&blobs, n).unwrap_or_else(|e| refuse(e))
    } else if let Some(n) = extract {
        let Some(out) = out else {
            refuse("--extract needs --out".to_string());
        };
        let payload = d1_trace::blob_decode::extract(&blobs, n).unwrap_or_else(|e| refuse(e));
        if let Err(e) = std::fs::write(&out, payload) {
            refuse(format!("could not write {}: {e}", out.display()));
        }
        format!("wrote {} bytes to {}\n", payload.len(), out.display())
    } else {
        d1_trace::blob_decode::summarise(&blobs)
    };
    use std::io::Write as _;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())?;
    Ok(())
}

fn cmd_summary(run: PathBuf) -> Result<()> {
    let trace = load_run(&run)?;
    let s = &trace.summary;
    println!("run:            {}", s.run_id);
    println!("total events:   {}", trace.events.len());
    println!(
        "  guest {}   http {}   bap {}   probe {}",
        s.guest, s.http, s.bap, s.probe
    );
    // F6: the denominator is rows that CARRY an id; `BAP TCP msg#N` outer headers are folded into
    // the inner decode they belong to and can never anchor, so they are not in it.
    let rate = if s.bap_anchorable == 0 {
        0.0
    } else {
        100.0 * s.bap_anchored as f64 / s.bap_anchorable as f64
    };
    println!(
        "bap anchored:   {}/{} id-carrying rows ({rate:.1}%)   [{} BAP rows total]",
        s.bap_anchored, s.bap_anchorable, s.bap
    );
    // F2: say plainly whether a cross-stream order was observed at all.
    println!(
        "cross-stream:   {}",
        if s.spine {
            "spine present - order is joined where marked '=' , bracketed '~', extrapolated '≈'"
        } else {
            "⚠ NOT OBSERVED - lane-grouped only; every row is '≈' (see the note below)"
        }
    );
    println!(
        "bootflow:       {}",
        s.bootflow_reached.as_deref().unwrap_or("(none seen)")
    );
    println!("error.* lines:  {}", s.error_count);
    // F9a: the names were collected, deduped, sorted - and then only `.len()` was printed, so an
    // operator could not tell a typo'd `probe:` term from a genuinely absent probe.
    println!("distinct probes:{}", s.probe_names.len());
    for chunk in s.probe_names.chunks(4) {
        println!("  {}", chunk.join("  "));
    }
    for note in &s.notes {
        println!("note: {note}");
    }
    Ok(())
}

fn cmd_search(run: PathBuf, query: String, limit: usize) -> Result<()> {
    let trace = load_run(&run)?;
    let q = Query::parse(&query);
    let errs = q.errors();
    if !errs.is_empty() {
        eprintln!("warning: unparsed tokens: {}", errs.join(", "));
    }
    // F9c: a query that cannot match by construction must not read as a fact about the run.
    for w in q.warnings() {
        eprintln!("warning: {w}");
    }
    // F9a: a typo'd probe name silently returns 0. Check it against the names this run actually has.
    for term in q.probe_terms() {
        if !trace
            .summary
            .probe_names
            .iter()
            .any(|n| n.contains(term.as_str()))
        {
            eprintln!(
                "warning: probe:{term} matches NONE of this run's {} probe names - this is a \
                 property of the query, not of the run. `d1-trace summary <run>` lists them.",
                trace.summary.probe_names.len()
            );
        }
    }
    if !trace.summary.spine {
        eprintln!(
            "warning: this run has no cross-stream spine; every `order` below is lane-local and \
             no cross-stream ordering may be read off it."
        );
    }
    let mut shown = 0usize;
    let mut total = 0usize;
    for ev in &trace.events {
        if q.matches(ev) {
            total += 1;
            if shown < limit {
                println!(
                    "{:<5} {} order={:>9.1}  {}",
                    ev.stream.tag(),
                    ev.confidence.marker(),
                    ev.order,
                    ev.summary
                );
                shown += 1;
            }
        }
    }
    println!("--- {total} match(es); showing {shown} ---");
    Ok(())
}
