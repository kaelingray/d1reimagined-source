// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Deterministic replay core shared by the `d1-observe` CLI and GUI.
//!
//! The immutable input event stream is the authority. Frontends receive only
//! reduced snapshots and explanations from this crate so human and agent views
//! cannot silently develop different protocol models.

pub mod framing;
#[cfg(feature = "gui")]
pub mod gui;
pub mod input;
pub mod model;
pub mod reducer;

pub use input::{load_run, LoadOptions};
pub use model::*;
pub use reducer::{first_divergence, reduce, snapshot_at};

