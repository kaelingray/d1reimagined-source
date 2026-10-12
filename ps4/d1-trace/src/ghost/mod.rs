// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-ghost` - the run companion's WRITE side. Design of record: `design/d1-ghost.md`.
//!
//! The read side is `d1-dash`, unchanged. This module is the project's **one write door**, with
//! its own outranking property (`design/d1-ghost.md` §2):
//!
//! > **No invisible writes.** Every write into a live client goes through the registry, is gated
//! > by observed client state, and is journaled to the run directory before and after execution.
//! > A poke that cannot be ledgered must not execute.
//!
//! ⚠ Honest bound: the ledger is a lower bound, not a lock. `/proc/<pid>/mem` cannot be reserved;
//! a shell poke bypassing ghost is still possible and still invisible. Anything rendering the
//! ledger must say "no *ledgered* pokes", never "no pokes".
//!
//! Three submodules, one job each:
//! - [`registry`] - the const table of known pokes: name, class, target, gate, citation. The
//!   registry is expected to SHRINK: a poke earned back onto the wire is deleted here, so the
//!   table doubles as the honest list of what is still synthetic.
//! - [`ledger`] - `runs/<run>/pokes.jsonl`, append-only, two lines per poke (intent before
//!   execute, result after), fail closed.
//! - [`exec`] - gate check, address resolution, the freeze-write-thaw discipline for multi-byte
//!   code patches, and the subprocess backend over the two whitelisted tools.

pub mod dests;
pub mod exec;
pub mod ledger;
pub mod registry;

#[cfg(feature = "gui")]
pub mod gui;

/// What the ghost has OBSERVED about the client, fed from the same dash panels the operator sees.
/// Gates read this, never recollection - that is the whole point of merging the tools
/// (`design/d1-ghost.md` §1.2).
#[derive(Debug, Clone, Default)]
pub struct Observed {
    /// The rung the client is sitting in (dash `current()`), if the ladder resolved it.
    pub rung: Option<u32>,
    /// The state name the client is sitting in, verbatim.
    pub state: String,
    /// `d1guest.py` liveness ladder level, 0-3. Only L3 means registry objects are live.
    pub liveness: u8,
}

/// The header taint lamp text (`design/d1-ghost.md` §2, `C-GHOST-TAINT-LAMP`).
///
/// ⚠ The honest bound is baked in: with an empty ledger the lamp says "no *ledgered* pokes",
/// NEVER "no pokes" - a shell poke bypassing ghost is still possible and still invisible. A
/// nonempty ledger, or a synthetic reach tag even over an empty ledger, lights the lamp.
///
/// - `ledger_nonempty`: this run's `pokes.jsonl` has at least one line.
/// - `reach_is_synthetic`: the run's reach tag is `synthetic*` (from `RUN-INFO.txt`), which taints
///   the run regardless of whether ghost itself wrote anything.
pub fn taint_lamp(ledger_nonempty: bool, reach_is_synthetic: bool) -> String {
    if ledger_nonempty {
        "⛔ LEDGERED POKES - this run has synthetic writes (see pokes.jsonl)".to_string()
    } else if reach_is_synthetic {
        "⛔ synthetic_ancestor - no ledgered pokes, but the reach tag is already synthetic".to_string()
    } else {
        // ⛔ Never "no pokes": the ledger is a lower bound, not a lock.
        "no ledgered pokes (a shell poke bypassing ghost would not appear here)".to_string()
    }
}
