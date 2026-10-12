// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Shutdown safety: **signal resolved process ids recorded for this run, and nothing else.**
//!
//! `historicaldocs/design/two-instance-harness-design.md`: *"Shutdown targets only resolved process IDs recorded
//! for this run. No broad `pkill` pattern or unresolved variable is permitted."*
//!
//! ## Why a recorded pid is not enough on its own
//!
//! A pid is not an identity - it is an index the kernel reuses. Between recording a pid and killing
//! it the process can exit and the number be handed to something else, and the something else is
//! quite likely to be a sibling in the same run (or the operator's shell). So every record also
//! carries the process's **start time** from `/proc/<pid>/stat` field 22, which together with the
//! pid *is* unique for the life of a boot, plus a substring of the command line. A record whose
//! start time no longer matches is refused as `PidReused` and never signalled.
//!
//! ## What is banned here, and why
//!
//! * **No patterns.** `pkill -f "shadps4 -g"` once matched the shell that was evaluating it and
//!   swallowed its own output; `pgrep -x shadps4` matched nothing at all, because the comm is
//!   `shadPS4:Main`. Neither failure is possible against an integer.
//! * **No pid ≤ 1, ever.** `kill(0, …)` signals the whole process group and `kill(-1, …)` signals
//!   everything the user owns. An unresolved variable in a shell becomes exactly those. Here they
//!   are rejected before the record is even written.
//! * **Dry run by default.** `plan()` is pure; only [`execute`] signals, and only what `plan()`
//!   marked signallable.

use std::collections::BTreeSet;
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PidRecord {
    /// `emulator-a`, `responder-b`, `shadnet-server`, … Unique within a ledger.
    pub role: String,
    pub pid: i32,
    /// `/proc/<pid>/stat` field 22. The half of the identity that makes pid reuse detectable.
    pub starttime: u64,
    /// A substring that must still be present in the command line at shutdown.
    pub marker: String,
    pub recorded_unix: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Ledger {
    pub run_id: String,
    pub records: Vec<PidRecord>,
}

/// Why a recorded process will not be signalled. Every one of these is a *refusal to act*.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Skip {
    /// `/proc/<pid>` is gone. Nothing to do.
    Exited,
    /// The pid exists but is a different process than the one recorded.
    PidReused { recorded: u64, now: u64 },
    /// Same start time, but the command line no longer contains the recorded marker. Two
    /// instruments disagreeing - report it, do not reconcile it, do not signal.
    MarkerMismatch { marker: String, cmdline: String },
    /// The record itself is not signallable (pid ≤ 1, empty marker).
    Unsignallable(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Signal { role: String, pid: i32 },
    Skip { role: String, pid: i32, reason: Skip },
}

/// Everything shutdown needs to know about a live process. A trait so the controls can drive
/// `PidReused` and `MarkerMismatch` without waiting for a real collision.
pub trait ProcTable {
    fn starttime(&self, pid: i32) -> Option<u64>;
    fn cmdline(&self, pid: i32) -> Option<String>;
    fn now_unix(&self) -> u64;
}

pub struct RealProcs;

/// `/proc/<pid>/stat` field 22, counting from 1.
///
/// ⚠ Field 2 is the comm **in parentheses and may itself contain spaces and parentheses**, so a
/// naive `split_whitespace().nth(21)` reads the wrong number for a process called `shadPS4:Main`
/// only by luck. Parse from the last `)`.
fn parse_starttime(stat: &str) -> Option<u64> {
    let tail = &stat[stat.rfind(')')? + 1..];
    // After the closing paren the fields are 3.. , so starttime (22) is index 19 here.
    tail.split_whitespace().nth(19)?.parse().ok()
}

impl ProcTable for RealProcs {
    fn starttime(&self, pid: i32) -> Option<u64> {
        parse_starttime(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
    }
    fn cmdline(&self, pid: i32) -> Option<String> {
        let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        Some(String::from_utf8_lossy(&raw).replace('\0', " ").trim().to_string())
    }
    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Build a record for a **live** process, refusing anything that cannot be identified later.
pub fn record(
    role: &str,
    pid: i32,
    marker: &str,
    procs: &dyn ProcTable,
) -> Result<PidRecord, String> {
    if pid <= 1 {
        return Err(format!(
            "refusing to record pid {pid}: 0 means the whole process group and -1 means every \
             process this user owns - those are what an unresolved variable becomes"
        ));
    }
    if marker.trim().is_empty() {
        return Err("refusing to record an empty marker: it would match every command line".into());
    }
    let starttime = procs
        .starttime(pid)
        .ok_or_else(|| format!("pid {pid} has no /proc entry; it is not running now"))?;
    let cmdline = procs.cmdline(pid).unwrap_or_default();
    if !cmdline.contains(marker) {
        return Err(format!(
            "pid {pid} command line {cmdline:?} does not contain the marker {marker:?} - recording \
             it would make the shutdown verification vacuous"
        ));
    }
    Ok(PidRecord {
        role: role.to_string(),
        pid,
        starttime,
        marker: marker.to_string(),
        recorded_unix: procs.now_unix(),
    })
}

pub fn add(ledger: &mut Ledger, rec: PidRecord) -> Result<(), String> {
    if ledger.records.iter().any(|r| r.role == rec.role) {
        return Err(format!("role {:?} is already in the ledger", rec.role));
    }
    if let Some(other) = ledger.records.iter().find(|r| r.pid == rec.pid) {
        return Err(format!(
            "pid {} is already recorded as {:?}; one process cannot be two roles",
            rec.pid, other.role
        ));
    }
    ledger.records.push(rec);
    Ok(())
}

/// Pure. Decides what *would* be signalled, in the ledger's reverse order - the stop order is the
/// start order backwards, so the last thing started is the first thing stopped.
pub fn plan(ledger: &Ledger, procs: &dyn ProcTable) -> Vec<Action> {
    let mut out = Vec::new();
    for r in ledger.records.iter().rev() {
        if r.pid <= 1 || r.marker.trim().is_empty() {
            out.push(Action::Skip {
                role: r.role.clone(),
                pid: r.pid,
                reason: Skip::Unsignallable(format!("pid={} marker={:?}", r.pid, r.marker)),
            });
            continue;
        }
        let Some(now) = procs.starttime(r.pid) else {
            out.push(Action::Skip { role: r.role.clone(), pid: r.pid, reason: Skip::Exited });
            continue;
        };
        if now != r.starttime {
            out.push(Action::Skip {
                role: r.role.clone(),
                pid: r.pid,
                reason: Skip::PidReused { recorded: r.starttime, now },
            });
            continue;
        }
        let cmdline = procs.cmdline(r.pid).unwrap_or_default();
        if !cmdline.contains(&r.marker) {
            out.push(Action::Skip {
                role: r.role.clone(),
                pid: r.pid,
                reason: Skip::MarkerMismatch { marker: r.marker.clone(), cmdline },
            });
            continue;
        }
        out.push(Action::Signal { role: r.role.clone(), pid: r.pid });
    }
    out
}

/// The only place a signal is sent. Takes the *plan*, not the ledger, so nothing can be signalled
/// that `plan` did not clear.
pub fn execute(plan: &[Action], signal: i32) -> Vec<String> {
    let mut log = Vec::new();
    let mut seen: BTreeSet<i32> = BTreeSet::new();
    for a in plan {
        if let Action::Signal { role, pid } = a {
            // Belt and braces: `plan` already refuses these, and it stays refused here too.
            if *pid <= 1 || !seen.insert(*pid) {
                log.push(format!("refused {role} pid {pid}"));
                continue;
            }
            // SAFETY: `pid` is a positive integer resolved from /proc and re-verified by start time
            // in `plan`. No pattern, no group, no wildcard.
            let rc = unsafe { libc::kill(*pid, signal) };
            log.push(format!("kill({pid}, {signal}) -> {rc}   [{role}]"));
        }
    }
    log
}

pub fn load(path: &str) -> Result<Ledger, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Writes only into an **existing** directory. The harness never creates a run tree as a side
/// effect of recording something.
pub fn save(path: &str, ledger: &Ledger) -> Result<(), String> {
    let parent = path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".");
    if !std::path::Path::new(parent).is_dir() {
        return Err(format!(
            "{parent} does not exist. The pid ledger lives in the run directory; create the run \
             directory deliberately, not as a side effect of recording a pid."
        ));
    }
    fs::write(path, serde_json::to_string_pretty(ledger).unwrap())
        .map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starttime_survives_a_comm_containing_spaces_and_parens() {
        // A real `shadPS4:Main` has no spaces, but `(sd-pam)` and `(a b) c` do exist, and the
        // failure mode is silent: a wrong field parses fine and compares unequal forever.
        let stat = "1234 (weird (comm) name) S 1 1 1 0 -1 4194560 100 0 0 0 5 6 0 0 20 0 1 0 987654 \
                    12345 0 0 0 0";
        assert_eq!(parse_starttime(stat), Some(987654));
    }

    #[test]
    fn our_own_starttime_reads_and_is_stable() {
        // Positive control that the /proc reader works at all - without it, "no starttime" would be
        // indistinguishable from "process gone" for every record.
        let p = RealProcs;
        let me = std::process::id() as i32;
        let a = p.starttime(me).expect("own starttime");
        let b = p.starttime(me).expect("own starttime again");
        assert_eq!(a, b);
        assert!(p.cmdline(me).unwrap().contains("d1"));
        // and a pid that cannot exist reads as absent
        assert_eq!(p.starttime(i32::MAX), None);
    }
}
