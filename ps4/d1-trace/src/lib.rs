// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-trace` - a read-only unified four-stream trace viewer for `runs/<stamp>/` directories.
//!
//! See `historicaldocs/design/trace-viewer.md`. It runs nothing and writes nothing into any client, so opening a
//! run adds no `synthetic_ancestor` taint.

pub mod blob_decode;
pub mod controls;
/// The **live** lane: `d1-dash`. Separate event loop, same ingest. See `historicaldocs/design/d1-dashboard.md`.
pub mod dash;
/// The `C-DASH-*` half of `d1-trace controls`.
pub mod dash_controls;
/// The **write** door: `d1-ghost`. Registry, ledger, gated executor. See `design/d1-ghost.md`.
pub mod ghost;
/// The `C-GHOST-*` half of `d1-trace controls`.
pub mod ghost_controls;
pub mod ingest;
pub mod model;
pub mod probe_decode;
pub mod query;

#[cfg(feature = "gui")]
pub mod gui;

pub use ingest::load_run;
pub use model::{Confidence, Event, Stream, Trace};
pub use query::Query;
