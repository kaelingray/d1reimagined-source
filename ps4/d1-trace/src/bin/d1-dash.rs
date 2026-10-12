// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-dash` - the live debug/status dashboard. See `historicaldocs/design/d1-dashboard.md`.
//!
//! A **second binary over the same library** as `d1-trace`: shared ingest, shared controls,
//! separate event loop. `d1-trace` stays a finished-run viewer that requests no repaints;
//! `d1-dash` follows a running one.
//!
//! ```text
//! d1-dash                              # newest run under ./runs, GUI
//! d1-dash runs/20260808-234535-CODEC-LIVE2
//! d1-dash --json board <run>           # the exact struct the GUI renders
//! d1-dash --json board --live <run>    # …plus one poll of the live client's descriptor table
//! d1-dash headline <run>               # one line
//! ```
//!
//! ⛔ It writes nothing, launches nothing and pokes nothing.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use d1_trace::dash;

#[derive(Parser)]
#[command(
    name = "d1-dash",
    about = "Live debug/status dashboard for a Destiny 1 preservation run (read-only)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run directory to watch. Defaults to the newest directory under `./runs` that has a
    /// RUN-INFO.txt.
    run: Option<PathBuf>,

    /// Print the presented board instead of opening a window: `--json board`.
    ///
    /// `board` is the only shape, and it is the exact struct the GUI renders - **the GUI has no
    /// fact the CLI cannot print** (`historicaldocs/design/interface-architecture.md` rule 1).
    #[arg(long, value_name = "WHAT", value_parser = ["board"])]
    json: Option<String>,

    /// With `--json board`: also poll the live client's descriptor table once.
    #[arg(long)]
    live: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Open the live window on a run directory.
    #[cfg(feature = "gui")]
    Gui { run: Option<PathBuf> },
    /// Print the presented board. `board` is the only shape; the GUI has no fact this cannot print.
    Json {
        #[arg(value_parser = ["board"])]
        what: String,
        run: Option<PathBuf>,
        /// Also poll the live client's descriptor table once (`capture-rig/d1guest.py descriptors`).
        ///
        /// Off by default so `--json board` on a closed run is deterministic - the live lane is a
        /// property of whatever client happens to be running, not of the run directory.
        #[arg(long)]
        live: bool,
    },
    /// One line: furthest rung, current rung, error count.
    Headline { run: Option<PathBuf> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // `--json board` is a flag rather than only a subcommand because that is how every control and
    // every other tool in this tree is invoked.
    if cli.json.is_some() {
        return json_board(resolve(cli.run)?, cli.live);
    }
    match cli.command {
        #[cfg(feature = "gui")]
        Some(Command::Gui { run }) => open(resolve(run)?),
        Some(Command::Json { what: _, run, live }) => json_board(resolve(run)?, live),
        Some(Command::Headline { run }) => {
            let dir = resolve(run)?;
            let session = dash::Session::open(&dir);
            println!("{}", session.board(None).headline());
            Ok(())
        }
        #[cfg(feature = "gui")]
        None => open(resolve(cli.run)?),
        #[cfg(not(feature = "gui"))]
        None => json_board(resolve(cli.run)?, false),
    }
}

fn resolve(run: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = run {
        if !p.is_dir() {
            bail!("{} is not a directory", p.display());
        }
        return Ok(p);
    }
    let roots = ["runs", "../runs", "../../runs"];
    for r in roots {
        let root = PathBuf::from(r);
        if root.is_dir() {
            if let Some(p) = dash::newest_run(&root) {
                eprintln!("d1-dash: no run given; watching the newest - {}", p.display());
                return Ok(p);
            }
        }
    }
    bail!(
        "no run directory given and none found under ./runs - pass one explicitly. \
         (A default that silently picks the wrong run is worse than no default.)"
    )
}

fn json_board(dir: PathBuf, live: bool) -> Result<()> {
    let session = dash::Session::open(&dir);
    let descriptors = if live {
        Some(dash::probe::poll(&dir))
    } else {
        None
    };
    let board = session.board(descriptors.as_ref());
    println!("{}", serde_json::to_string_pretty(&board)?);
    Ok(())
}

#[cfg(feature = "gui")]
fn open(dir: PathBuf) -> Result<()> {
    dash::gui::run(dir)
}
