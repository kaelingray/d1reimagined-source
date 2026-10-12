// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The ledger - `runs/<run>/pokes.jsonl` (`design/d1-ghost.md` §5).
//!
//! Append-only JSONL, **two lines per poke**, written by ghost itself:
//!
//! 1. **intent** - appended BEFORE execution. If this cannot be appended, the poke refuses: the
//!    property is "a poke that cannot be ledgered must not execute", so the intent write IS the
//!    gate, not a log of a gate already passed.
//! 2. **result** - appended after: readback before, readback after, outcome, tool exit status.
//!
//! A crash between the two leaves a visible **dangling intent** - a fact worth having, not a bug.
//! `C-GHOST-DANGLING-INTENT` asserts it renders as a loud banner, never silence.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The ledger filename inside a run directory.
pub const LEDGER_FILE: &str = "pokes.jsonl";

/// One line of the ledger. `kind` discriminates intent from result so a reader can pair them by
/// `seq` and flag any intent with no matching result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Line {
    /// Written before the poke executes.
    Intent {
        /// RFC3339 UTC. Passed in by the caller (the crate takes no wall-clock of its own, so the
        /// controls are deterministic); the GUI stamps it from `SystemTime` at call time.
        ts: String,
        /// Monotonic within this run - the pairing key.
        seq: u64,
        /// Registry entry name (the ledger key), or `"freeform"`.
        name: String,
        /// `Probe` / `Satisfy` / `Launch` / `Freeform`.
        class: String,
        /// Human-readable target, e.g. `0x26B5730 u32` or `session-params --activity-id 407`.
        target: String,
        /// The rung the client was sitting in when the intent was recorded (gate evidence).
        rung: Option<u32>,
        /// Optional free-text operator note.
        note: Option<String>,
    },
    /// Written after the poke executes (or fails).
    Result {
        ts: String,
        /// Matches the intent's `seq`.
        seq: u64,
        /// Bytes read back at the target BEFORE the write, hex.
        before: String,
        /// Bytes read back AFTER, hex. Empty on a launch or a failed spawn.
        after: String,
        /// `applied` | `mismatch` | `refused` | `failed`.
        outcome: String,
        /// The subprocess exit status, if one ran.
        exit: Option<i32>,
    },
}

/// Terse constructor for an intent line - keeps the controls and any caller from restating the
/// whole struct. Not used by the executor (which builds the struct inline with real values).
#[allow(clippy::too_many_arguments)]
pub fn mk_intent(
    ts: &str,
    seq: u64,
    name: &str,
    class: &str,
    target: &str,
    rung: Option<u32>,
    note: Option<String>,
) -> Line {
    Line::Intent {
        ts: ts.into(),
        seq,
        name: name.into(),
        class: class.into(),
        target: target.into(),
        rung,
        note,
    }
}

/// Terse constructor for a result line.
pub fn mk_result(ts: &str, seq: u64, before: &str, after: &str, outcome: &str, exit: Option<i32>) -> Line {
    Line::Result {
        ts: ts.into(),
        seq,
        before: before.into(),
        after: after.into(),
        outcome: outcome.into(),
        exit,
    }
}

/// The outcome tokens. Kept as consts so the executor and the dangling-intent scanner cannot
/// disagree about spelling.
pub const OUTCOME_APPLIED: &str = "applied";
pub const OUTCOME_MISMATCH: &str = "mismatch";
pub const OUTCOME_REFUSED: &str = "refused";
pub const OUTCOME_FAILED: &str = "failed";

/// A handle to one run's ledger.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Open (do not create) the ledger for a run directory. The run dir must already exist - ghost
    /// attaches to a run, it never makes one.
    pub fn for_run(run_dir: &Path) -> Ledger {
        Ledger {
            path: run_dir.join(LEDGER_FILE),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one line, flushing to disk before returning. **Fail closed:** any IO error is
    /// returned to the caller, which must refuse the poke rather than execute-then-warn
    /// (`design/d1-ghost.md` §5, `C-GHOST-FAIL-CLOSED`).
    ///
    /// The run directory is NOT created: a missing run dir is exactly the "cannot be ledgered"
    /// condition the fail-closed property is about, so it surfaces as an error here.
    pub fn append(&self, line: &Line) -> std::io::Result<()> {
        let mut json = serde_json::to_string(line).map_err(std::io::Error::other)?;
        json.push('\n');
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(json.as_bytes())?;
        f.flush()?;
        // fsync: the intent must survive a crash between the two lines, or the dangling-intent
        // evidence the property promises would be lost to the page cache. `[U]` in the design doc
        // asked whether this is needed; it is cheap and it is what makes the crash window real.
        f.sync_all()?;
        Ok(())
    }

    /// Read every line back, in order. Malformed lines are returned as errors alongside the good
    /// ones so a corrupt tail is loud, not silently truncated.
    pub fn read_all(&self) -> Vec<Result<Line, String>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(e) => return vec![Err(format!("{}: {e}", self.path.display()))],
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<Line>(l).map_err(|e| format!("{e}: {l}")))
            .collect()
    }

    /// Does this ledger have at least one line? Drives the header taint lamp: a nonempty ledger
    /// means this run has ledgered pokes (`C-GHOST-TAINT-LAMP`).
    pub fn is_nonempty(&self) -> bool {
        std::fs::metadata(&self.path).map(|m| m.len() > 0).unwrap_or(false)
    }
}

/// A `seq` whose intent has no matching result - a poke that started and did not visibly finish.
/// The GUI renders each of these as a loud dangling-write banner (`C-GHOST-DANGLING-INTENT`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dangling {
    pub seq: u64,
    pub name: String,
    pub target: String,
    pub ts: String,
}

/// Pair intents with results by `seq` and return the intents that never got a result. Parse errors
/// are ignored HERE (they surface via [`Ledger::read_all`]); this function answers exactly one
/// question, so a garbled line does not masquerade as a dangling intent.
pub fn dangling(lines: &[Line]) -> Vec<Dangling> {
    use std::collections::BTreeMap;
    let mut intents: BTreeMap<u64, Dangling> = BTreeMap::new();
    let mut resulted: std::collections::BTreeSet<u64> = Default::default();
    for l in lines {
        match l {
            Line::Intent { seq, name, target, ts, .. } => {
                intents.insert(
                    *seq,
                    Dangling {
                        seq: *seq,
                        name: name.clone(),
                        target: target.clone(),
                        ts: ts.clone(),
                    },
                );
            }
            Line::Result { seq, .. } => {
                resulted.insert(*seq);
            }
        }
    }
    intents
        .into_iter()
        .filter(|(seq, _)| !resulted.contains(seq))
        .map(|(_, d)| d)
        .collect()
}

/// The next `seq` for a run: one past the highest intent seq already on disk, so a ghost that
/// re-attaches to a run mid-session does not collide with pokes it made earlier.
pub fn next_seq(lines: &[Line]) -> u64 {
    lines
        .iter()
        .filter_map(|l| match l {
            Line::Intent { seq, .. } => Some(*seq),
            _ => None,
        })
        .max()
        .map(|m| m + 1)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_run(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d1-ghost-ledger-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn intent(seq: u64, name: &str) -> Line {
        Line::Intent {
            ts: "2026-08-11T00:00:00Z".into(),
            seq,
            name: name.into(),
            class: "Satisfy".into(),
            target: "0x26B5730 u32".into(),
            rung: Some(34),
            note: None,
        }
    }
    fn result(seq: u64, outcome: &str) -> Line {
        Line::Result {
            ts: "2026-08-11T00:00:01Z".into(),
            seq,
            before: "00000000".into(),
            after: "ff070000".into(),
            outcome: outcome.into(),
            exit: Some(0),
        }
    }

    /// The round-trip: what we append is exactly what we read back.
    #[test]
    fn append_then_read_roundtrips() {
        let d = tmp_run("roundtrip");
        let led = Ledger::for_run(&d);
        led.append(&intent(0, "done-mask-7ff")).unwrap();
        led.append(&result(0, OUTCOME_APPLIED)).unwrap();
        let got: Vec<Line> = led.read_all().into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(got, vec![intent(0, "done-mask-7ff"), result(0, OUTCOME_APPLIED)]);
        assert!(led.is_nonempty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ C-GHOST-FAIL-CLOSED half: a missing run dir makes append error, so the caller refuses.
    #[test]
    fn append_into_a_missing_run_dir_errors() {
        let missing = std::env::temp_dir().join(format!("d1-ghost-nope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        let led = Ledger::for_run(&missing);
        assert!(led.append(&intent(0, "x")).is_err(), "a poke into a missing run dir must not ledger");
        assert!(!led.is_nonempty());
    }

    /// C-GHOST-DANGLING-INTENT: an intent with no result is found; a paired one is not.
    #[test]
    fn a_dangling_intent_is_found() {
        let lines = vec![
            intent(0, "done-mask-7ff"),
            result(0, OUTCOME_APPLIED),
            intent(1, "join-ctr-0"), // crashed before result
        ];
        let d = dangling(&lines);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].seq, 1);
        assert_eq!(d[0].name, "join-ctr-0");
    }

    #[test]
    fn next_seq_is_one_past_the_highest_intent() {
        assert_eq!(next_seq(&[]), 0);
        assert_eq!(next_seq(&[intent(0, "a"), result(0, OUTCOME_APPLIED), intent(1, "b")]), 2);
    }

    /// A corrupt line is surfaced as an error, not silently dropped.
    #[test]
    fn a_corrupt_line_is_loud() {
        let d = tmp_run("corrupt");
        let led = Ledger::for_run(&d);
        led.append(&intent(0, "a")).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(led.path())
            .unwrap()
            .write_all(b"{not json}\n")
            .unwrap();
        let got = led.read_all();
        assert_eq!(got.len(), 2);
        assert!(got[0].is_ok());
        assert!(got[1].is_err(), "the garbled tail must be an error, not a dropped line");
        let _ = std::fs::remove_dir_all(&d);
    }
}
