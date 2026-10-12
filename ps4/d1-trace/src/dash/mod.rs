// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-dash` - the **live** debug/status surface. See `historicaldocs/design/d1-dashboard.md`.
//!
//! This is a second binary over the same library as the static `d1-trace` viewer, exactly as §1 of
//! that spec decided: shared ingest/model/query/controls, a **separate event loop**, so `d1-trace`'s
//! documented idleness (it calls `request_repaint` nowhere, on purpose) is not broken by a live tail.
//!
//! The one-line purpose:
//!
//! > Show, live and without stopping the client, which rung of the 37-rung launch ladder the game is
//! > on, what refused to happen, and which of our systems answered - with every number carrying the
//! > file and line-shape it was scraped from.
//!
//! Three rules run through every module here and are worth stating once:
//!
//! - **R1 - three states.** [`present::Cell`] is `Ok` / `Empty` / `NotObserved`. Grey must never
//!   render as green, and the two differ in *shape* as well as colour in the GUI. This project's
//!   signature false negative is an instrument returning **zeros, not an error**.
//! - **R2 - every number names its source.** Every row carries [`present::Src`]: the file, the exact
//!   line-shape it matched, and `Verbatim` vs `Derived`.
//! - **R3 - staleness is a value.** Every lane carries its own last-update stamp and a stale badge.
//!   `sockets.log` stops after exactly 300 s and then shows five-minute-old data forever, with no
//!   in-band marker that it went quiet.
//!
//! **Nothing here writes anything.** No poke, no POST, no launch. Read-only is the property that
//! keeps the dashboard out of the `synthetic_ancestor` chain.

pub mod ladder;
pub mod present;
pub mod probe;
pub mod reduce;
pub mod tail;

#[cfg(feature = "gui")]
pub mod gui;

pub use present::{Board, Cell, Prov, Src};

use std::path::{Path, PathBuf};

/// A run being watched: the reducer, the ladder, and one tailer per push lane.
///
/// The **same** parse path serves the initial whole-file load and every 250 ms tail poll, so a live
/// board and a post-hoc board cannot disagree about what a line means.
pub struct Session {
    pub run_dir: PathBuf,
    pub ladder: ladder::Ladder,
    pub reducer: reduce::Reducer,
    pub emulator: tail::Tailer,
    pub notes: Vec<String>,
}

impl Session {
    /// Open a run directory and consume everything already in it.
    pub fn open(run_dir: &Path) -> Session {
        let ladder = ladder::load(run_dir);
        let mut reducer = reduce::Reducer::new(&ladder);
        let mut notes = Vec::new();

        let emu_path = run_dir.join("emulator.log");
        let (text, consumed, lossy, err) = tail::load_and_offset(&emu_path);
        if let Some(e) = err {
            notes.push(e);
        }
        if lossy {
            reducer.lossy = true;
        }
        reducer.emulator_bytes = consumed;
        let mut line_no = 0u64;
        for line in text.lines() {
            reducer.feed_emulator_line(line, line_no);
            line_no += 1;
        }
        // Resume exactly where the load stopped - the offset is the length of what we actually
        // read, not a second metadata() call taken afterwards, so no appended line can slip through
        // the gap.
        let emulator = tail::Tailer::resuming_at(&emu_path, consumed, line_no);

        Session {
            run_dir: run_dir.to_path_buf(),
            ladder,
            reducer,
            emulator,
            notes,
        }
    }

    /// Fold one tail batch into the reducer.
    pub fn absorb(&mut self, batch: &tail::Batch) {
        if batch.lossy {
            self.reducer.lossy = true;
        }
        if batch.truncated {
            self.notes.push(format!(
                "⚠ {} SHRANK - the tail offset was reset. A run directory being reused underneath \
                 a live viewer is a domain fact, not a read error.",
                self.emulator.path.display()
            ));
        }
        if !batch.error.is_empty() {
            self.notes.push(format!("⚠ tail error: {}", batch.error));
        }
        // ⛔ R3: ordinals come from the BATCH, which the producing tailer stamped - never from
        // `self.emulator`, whose counter is frozen at load time when the GUI polls its own tailer.
        // The old `next_line_no() - len` subtraction collided, went backwards, and wrapped u64 on a
        // short log.
        let mut n = batch.first_line_no;
        for line in &batch.lines {
            self.reducer.emulator_bytes += line.len() as u64 + 1;
            self.reducer.feed_emulator_line(line, n);
            n += 1;
        }
    }

    /// Present the current state as a [`Board`].
    pub fn board(&self, descriptors: Option<&probe::Descriptors>) -> Board {
        present::build(
            &self.run_dir,
            &self.ladder,
            &self.reducer,
            descriptors,
            self.notes.clone(),
        )
    }
}

/// The newest directory under `runs/` that looks like a run (has a `RUN-INFO.txt`).
pub fn newest_run(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(root).ok()?.flatten() {
        let p = e.path();
        if !p.is_dir() || !p.join("RUN-INFO.txt").is_file() {
            continue;
        }
        let t = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
            best = Some((t, p));
        }
    }
    best.map(|(_, p)| p)
}
