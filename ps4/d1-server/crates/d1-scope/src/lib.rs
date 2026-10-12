// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-scope` - **the live client as two planes**, of which BUILD 1 implements exactly one.
//!
//! > **THE LIFECYCLE INVARIANT.** *d1-scope may never leave a hardware watchpoint armed behind a
//! > process that is not alive to disarm it: at most one `d1-live` child exists at any instant, it
//! > is always launched with a bounded `--max-seconds` lease shorter than the parent's heartbeat
//! > gap, it is stopped only by `SIGTERM` and only after the parent has waited for the child's own
//! > confirmed disarm on exit, and every path out of d1-scope - normal return, error, panic,
//! > `SIGINT`, `SIGTERM` - runs that stop. **A tracer that dies armed kills its target.** That is
//! > measured, every time, not theoretical, and the target is the operator's live client.*
//!
//! # ⛔ WHAT BUILDS 1-2 ARE, AND ARE NOT
//!
//! **BUILD 1 - the state plane:** [`mem_source`], [`registry`], [`eval`], [`resolvers`], [`frame`],
//! [`sampler`]. Everything is a **read**, and every read goes through [`mem_source::MemSource`], a
//! trait with **no `write` method at all**. The poke ban is a property of the type here, not of
//! anyone's discipline.
//!
//! **BUILD 2 - the event plane:** [`child_mgr`] (exactly one leased `d1-live` child, a `SIGTERM`-only
//! stop path, a `Drop` guard, a signal handler, and refusal-by-message when something else holds
//! the target) and [`events`] (the child's `--json` report, parsed and attached to registry
//! surfaces **by address**).
//!
//! ⛔ **d1-scope performs no debug-register operation itself.** It spawns the `d1-live` *binary*
//! and speaks to it over argv, stdout and signals. Control #4 asserts there is no call path - over
//! this crate's whole source tree and over the shipped binary's dynamic symbols - and BUILD 2 is
//! written to keep that true: [`child_mgr`] links neither `d1_live::watch` nor `d1_live::controls`.
//!
//! **BUILD 3 - the surface table:** [`table`] (rows, cells, sort keys, filter, grouping, the seven
//! render states and the event badge - **egui-free**, and the only thing that computes a cell
//! string) and [`ui`] (eframe/egui, behind `--features gui`).
//!
//! ⭐ **The window and `d1-scope gui --headless --json` render the SAME [`table::TableModel`].**
//! The window iterates [`table::Column::ALL`] and prints [`table::TableRow::cell`]; the dump
//! serialises those very strings. Control #11 compares what the AccessKit tree actually contains
//! against what the model actually holds, so a UI-only column fails the build rather than shipping
//! a table nobody can reproduce from the JSON.
//!
//! ⛔ **BUILD 3's window arms nothing.** It reads no memory (the sampler runs on its own thread and
//! hands over an `Arc<Frame>`) and it spawns no child. Arming stays on the command line, where it
//! costs a deliberate `--pid`.
//!
//! **BUILD 4 - the flow graph:** [`graph`] (nodes, edges, the fixed layout, the derived colour and
//! the event badge - **egui-free**, and the only thing that decides a colour) and [`ui_graph`] (a
//! hand-drawn `egui::Painter` canvas, behind `--features gui`). ⛔ **No graph crate**: nothing of
//! that shape is in a lock file in this tree or in this box's offline registry cache, so "just add
//! the dep" is a build failure here.
//!
//! ⭐⭐ **THE GRAPH'S COLOUR IS DERIVED, NOT DECLARED.** `registry.toml`'s `state` is a human pass's
//! verdict as of a date; [`graph::derive`] computes one from the state plane **this tick**, through
//! a single ordered rule, using the per-node **witness** field declared in
//! `capture-rig/surfaces/graph.toml`. Both travel on every edge and a disagreement is *reported*,
//! never resolved - the derived one is not automatically right, it is the one whose reasoning the
//! reader can check. ⛔ And a witness that appears in the snapshot's `poked_fields` is never a
//! colour at all: a value we wrote is not evidence about the client.
//!
//! ⛔ NOT implemented: `--source live` wiring for the state plane. `ProcMem` exists because
//! control #4 has to *prove* the read-only property on a real file descriptor, and it refuses the
//! emulator outright unless `D1_SCOPE_ALLOW_LIVE=1` is set. [`child_mgr::check_target`] refuses it
//! the same way.
//!
//! # The address grammar is NOT forked
//!
//! Every address in `capture-rig/surfaces/registry.toml` is resolved by
//! [`d1_live::addr::addr`] - the same function `d1-live watch --addr` calls. d1-scope contains no
//! address parser. It owns exactly two things *above* that grammar, both in [`eval`]:
//!
//! 1. a **type prefix** (`u32@…`, `i32[3]@…`, `f32[3]@…`) that is a strict superset of
//!    [`d1_live::addr::Width`], because the registry needs `i16`/`i64`/`f32` and fixed-size arrays;
//! 2. **one** textual transform - unwrapping a single matched paren pair that encloses the whole
//!    address text - because the registry's control receipts were written as `u32@(EXPR)` and the
//!    shared grammar has no parentheses. The transform is in one function, it is counted and
//!    reported by control #5, and **the surface `addr` strings need it zero times**.
//!
//! # The snapshot format is NOT described twice
//!
//! [`mem_source::SnapMem`] wraps [`d1_replay::snapshot::Snapshot`]. d1-scope knows nothing about
//! `D1SNAP\x01\x00`, the manifest length prefix, or `file_off`.


/// ⭐⭐ THE `--ticks` DEFAULT, AND IT IS MODE-DEPENDENT ON PURPOSE.
///
/// `None` (the flag unset) means **1** under `--headless` and **0** (until interrupted) for a
/// window. An explicit `--ticks N` always wins.
///
/// ⛔ **Why this is a function and not a `default_value_t`.** A flat default of `1` shipped, and it
/// made both windows sample **exactly once**: the sampler thread returned after one frame, the
/// `Frame` stopped advancing, and `--stale-after-ms` (2 s) then painted every row `STALE ‹…›`.
///
/// That reads as *"the client went quiet"* and means *"the tool stopped asking"* - **the crate's own
/// thesis turned against it.** `table`'s module header says a frozen window and a quiet client are
/// otherwise pixel-identical, which is precisely why the staleness badge exists. **The badge worked.
/// The default made the frozen case normal**, so the badge fired on every run and taught the
/// operator to read `STALE` as background noise. ⚠ *A warning that fires on correctly-handled input
/// is worse than no warning* - `status/LESSONS.md`, and this is that entry with a UI on it.
pub fn effective_ticks(explicit: Option<u64>, headless: bool) -> u64 {
    match explicit {
        Some(n) => n,
        None if headless => 1, // tick, print, exit - the dump is a one-shot
        None => 0,             // a window samples until interrupted
    }
}

pub mod child_mgr;
pub mod controls;
pub mod controls_event;
pub mod controls_graph;
pub mod controls_ui;
pub mod eval;
pub mod events;
pub mod frame;
pub mod graph;
pub mod mem_source;
pub mod registry;
pub mod resolvers;
pub mod sampler;
pub mod table;
#[cfg(feature = "gui")]
pub mod ui;
#[cfg(feature = "gui")]
pub mod ui_graph;

use std::path::{Path, PathBuf};

/// The repository root that contains `d1-server/` and `capture-rig/`.
///
/// ⚠ Derived from `CARGO_MANIFEST_DIR` at compile time, not from the process's cwd: controls must
/// give the same answer whoever runs them and from wherever. `$D1_REPO_ROOT` overrides.
pub fn repo_root() -> PathBuf {
    if let Ok(p) = std::env::var("D1_REPO_ROOT") {
        return PathBuf::from(p);
    }
    // …/d1-server/crates/d1-scope → up three.
    let m = Path::new(env!("CARGO_MANIFEST_DIR"));
    m.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap_or(m)
        .to_path_buf()
}

/// `capture-rig/surfaces/registry.toml`.
pub fn default_registry_path() -> PathBuf {
    if let Ok(p) = std::env::var("D1_SCOPE_REGISTRY") {
        return PathBuf::from(p);
    }
    repo_root().join("capture-rig/surfaces/registry.toml")
}

/// `runs/snapshots/` - ⛔ read-only to this crate, forever. Nothing here opens a snapshot for
/// writing, and nothing here deletes one.
pub fn snapshots_dir() -> PathBuf {
    repo_root().join("runs/snapshots")
}
