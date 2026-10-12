// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Gate ledger - the actual product of this server.
//!
//! The governing question of the project is *"what is the next observable condition Destiny refuses
//! to proceed without?"* Every command we don't implement is a candidate answer. This tracks them so

//! a run ends with a ranked, deterministic list instead of a wall of scrollback.

use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct GateLedger {
    /// Command name → times requested. `BTreeMap` so the summary is byte-identical across runs.
    counts: BTreeMap<String, u64>,
    /// Commands we answered with `Unsupported` - the backlog.
    unimplemented: BTreeMap<String, u64>,
}

impl GateLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a request. Returns true the first time this command is seen.
    pub fn record(&mut self, name: &str) -> bool {
        let n = self.counts.entry(name.to_string()).or_insert(0);
        *n += 1;
        *n == 1
    }

    pub fn record_unimplemented(&mut self, name: &str) {
        *self.unimplemented.entry(name.to_string()).or_insert(0) += 1;
    }

    pub fn count(&self, name: &str) -> u64 {
        self.counts.get(name).copied().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Deterministic one-line-per-command summary, implemented commands first.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        for (name, n) in &self.counts {
            let mark = if self.unimplemented.contains_key(name) {
                "UNIMPLEMENTED"
            } else {
                "served"
            };
            out.push_str(&format!("\n    {name:<32} x{n:<6} {mark}"));
        }
        if !self.unimplemented.is_empty() {
            out.push_str("\n  next gates to design for: ");
            out.push_str(
                &self
                    .unimplemented
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sight_is_reported_once() {
        let mut l = GateLedger::new();
        assert!(l.record("Login"));
        assert!(!l.record("Login"));
        assert_eq!(l.count("Login"), 2);
    }

    #[test]
    fn summary_is_deterministic() {
        let mut a = GateLedger::new();
        let mut b = GateLedger::new();
        for name in ["CreateRoom", "Login", "GetToken"] {
            a.record(name);
        }
        // same set, inserted in a different order
        for name in ["GetToken", "CreateRoom", "Login"] {
            b.record(name);
        }
        a.record_unimplemented("CreateRoom");
        b.record_unimplemented("CreateRoom");
        assert_eq!(a.summary(), b.summary());
    }
}
