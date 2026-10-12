// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The live-memory lane: `capture-rig/d1guest.py descriptors`, as a subprocess.
//!
//! ## Why a subprocess and not an mmap
//!
//! `d1guest.py` re-runs its own build gate, base control and **liveness ladder L0-L3 on every
//! invocation**, and prints them. Reimplementing the read here would throw all of that away and
//! give us a number with no controls attached. Measured cost: 0.10-0.11 s warm, ordinary
//! `/proc/<pid>/mem` reads - **no ptrace stop, no SIGSTOP, no observable perturbation of the
//! guest**. Poll at 2-5 s, never faster.
//!
//! ## What the caller must not do with the result
//!
//! - ⚠ **Every read is a torn read.** There is no lock between `/proc/pid/mem` and the guest, so
//!   each value is stamped *"as of T, single sample"* and is **never smoothed or averaged**. A
//!   smoothed torn read is indistinguishable from a real transition.
//! - ⚠ **Below L3 the whole table greys.** A failed read returns **zeros, not an error**, for a
//!   wrong address class - this project's signature false negative.
//! - ⚠ Never read `/tmp/d1guest-base.json` directly. It is a shared mutable cache and other agents'
//!   tools write it.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

/// One row of the client's own descriptor table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FamilyRow {
    pub slot: u32,
    pub class: u32,
    pub key: String,
    pub state: u32,
    pub state_text: String,
    pub err: String,
    pub ver: i64,
    /// The line exactly as `d1guest.py` printed it.
    pub raw: String,
}

/// One rung of `d1guest.py`'s own liveness ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivenessRung {
    pub rung: String,
    pub pass: bool,
    pub text: String,
    pub raw: String,
}

/// A single poll of the live client.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Descriptors {
    pub rows: Vec<FamilyRow>,
    pub liveness: Vec<LivenessRung>,
    /// `LIVENESS: L3` → `Some(3)`.
    pub liveness_level: Option<u32>,
    /// The tool's own timestamp line, verbatim. Local time - **not** the same clock as
    /// `activity-host.log`'s RFC3339 UTC, which is why no lane here shares a time axis.
    pub stamp: String,
    /// Non-empty when the poll failed. An empty table with an empty error would be a lie.
    pub error: String,
}

impl Descriptors {
    pub fn failed(why: impl Into<String>) -> Descriptors {
        Descriptors {
            error: why.into(),
            ..Default::default()
        }
    }
}

fn find_script(start: &Path) -> Option<PathBuf> {
    if let Ok(repo) = std::env::var("D1_REPO") {
        let p = Path::new(&repo).join("capture-rig").join("d1guest.py");
        if p.is_file() {
            return Some(p);
        }
    }
    for base in [Some(start.to_path_buf()), std::env::current_dir().ok()]
        .into_iter()
        .flatten()
    {
        let mut cur: Option<&Path> = Some(&base);
        while let Some(p) = cur {
            let cand = p.join("capture-rig").join("d1guest.py");
            if cand.is_file() {
                return Some(cand);
            }
            cur = p.parent();
        }
    }
    None
}

/// Poll the live client. Never panics; a failure is a [`Descriptors::failed`] with the reason.
pub fn poll(near: &Path) -> Descriptors {
    let Some(script) = find_script(near) else {
        return Descriptors::failed(
            "could not find capture-rig/d1guest.py by ascending from the run directory or the cwd; \
             set D1_REPO",
        );
    };
    let out = match Command::new("python3")
        .arg("-B")
        .arg(&script)
        .arg("descriptors")
        .output()
    {
        Ok(o) => o,
        Err(e) => return Descriptors::failed(format!("running {}: {e}", script.display())),
    };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut d = parse(&text);
    if !out.status.success() {
        // The tool exits non-zero when a control fails or no client is running. Its own words are
        // the reason - do not paraphrase them into "not running".
        d.error = format!(
            "d1guest.py descriptors exited {}: {}",
            out.status,
            first_meaningful_line(&err).unwrap_or_else(|| first_meaningful_line(&text)
                .unwrap_or_else(|| "no output".into()))
        );
        d.rows.clear();
    } else if d.rows.is_empty() && d.error.is_empty() {
        d.error = "d1guest.py descriptors exited 0 but printed no descriptor rows".into();
    }
    d
}

fn first_meaningful_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.to_string())
}

/// Parse `d1guest.py descriptors` stdout.
///
/// Shapes, all pinned by `C-DASH-DESCRIPTORS-PARSE`:
/// ```text
/// LIVENESS: L3
///   L0  PASS  pid 3423549 alive
///   L3  PASS  account (fam4/t0) key=0xd100000000000001 == expected - registry objects are LIVE
/// 00:10:58
///   [  0] class=0   key=0xd100000000000001  state=3 REPLICATING (update accepted)  err=-  ver=0
/// ```
pub fn parse(text: &str) -> Descriptors {
    let mut d = Descriptors::default();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("LIVENESS:") {
            d.liveness_level = rest
                .trim()
                .trim_start_matches('L')
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok());
            continue;
        }
        // `L0  PASS  pid … alive`
        if line.len() > 2
            && line.starts_with('L')
            && line[1..2].chars().all(|c| c.is_ascii_digit())
            && (line.contains("PASS") || line.contains("FAIL"))
        {
            let mut it = line.splitn(3, char::is_whitespace);
            let rung = it.next().unwrap_or("").to_string();
            let rest = line[rung.len()..].trim();
            let pass = rest.starts_with("PASS");
            let text = rest
                .trim_start_matches("PASS")
                .trim_start_matches("FAIL")
                .trim()
                .to_string();
            d.liveness.push(LivenessRung {
                rung,
                pass,
                text,
                raw: raw.to_string(),
            });
            continue;
        }
        if line.starts_with('[') {
            if let Some(row) = parse_family_row(line, raw) {
                d.rows.push(row);
            }
            continue;
        }
        // The bare `HH:MM:SS` stamp the tool prints before the table.
        if line.len() == 8 && line.chars().filter(|c| *c == ':').count() == 2 {
            d.stamp = line.to_string();
        }
    }
    if d.stamp.is_empty() {
        d.stamp = "(no stamp printed)".into();
    }
    d
}

fn parse_family_row(line: &str, raw: &str) -> Option<FamilyRow> {
    let close = line.find(']')?;
    let slot: u32 = line[1..close].trim().parse().ok()?;
    let rest = &line[close + 1..];
    let class: u32 = field(rest, "class=")?.parse().ok()?;
    let key = field(rest, "key=")?.to_string();
    let state_tok = field(rest, "state=")?;
    let state: u32 = state_tok.parse().ok()?;
    // Everything between the state number and `err=` is the tool's own prose for that state.
    let after_state = rest.split("state=").nth(1)?;
    let state_text = after_state
        .split("err=")
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches(state_tok)
        .trim()
        .to_string();
    let err = field(rest, "err=").unwrap_or("-").to_string();
    let ver: i64 = field(rest, "ver=").and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(FamilyRow {
        slot,
        class,
        key,
        state,
        state_text,
        err,
        ver,
        raw: raw.trim().to_string(),
    })
}

fn field<'a>(hay: &'a str, key: &str) -> Option<&'a str> {
    let i = hay.find(key)? + key.len();
    Some(hay[i..].split_whitespace().next().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
BUILD GATE: PASS  eboot sha256 7271fcb926401df8… == symbol-file 7271fcb926401df8…
BASE CONTROL: PASS  base=0x800000000
LIVENESS: L3
  L0 PASS  pid 3423549 alive
  L2 PASS  object registry 0x37f8dfe80
  L3 PASS  account (fam4/t0) key=0xd100000000000001 == expected - registry objects are LIVE
00:10:58
  [  0] class=0   key=0xd100000000000001  state=3 REPLICATING (update accepted)  err=-  ver=0
  [254] class=5   key=0x7fffffffffffffff  state=3 REPLICATING (update accepted)  err=-  ver=-1
";

    #[test]
    fn parses_the_real_shape() {
        let d = parse(SAMPLE);
        assert_eq!(d.liveness_level, Some(3));
        assert_eq!(d.liveness.len(), 3);
        assert_eq!(d.stamp, "00:10:58");
        assert_eq!(d.rows.len(), 2);
        assert_eq!(d.rows[0].slot, 0);
        assert_eq!(d.rows[0].class, 0);
        assert_eq!(d.rows[0].state, 3);
        assert_eq!(d.rows[0].key, "0xd100000000000001");
        assert_eq!(d.rows[0].ver, 0);
        assert_eq!(d.rows[1].slot, 254);
        assert_eq!(d.rows[1].ver, -1);
        assert!(d.rows[0].state_text.starts_with("REPLICATING"));
        // Verbatim: the row keeps the tool's own text.
        assert!(d.rows[0].raw.contains("update accepted"));
    }

    /// Negative control: the parser is not "matches everything". Prose that merely mentions the
    /// words must not become a family row.
    #[test]
    fn prose_is_not_a_family_row() {
        let d = parse("  the class=0 key=0x1 state=3 line without a slot bracket\n");
        assert!(d.rows.is_empty());
        assert!(d.liveness.is_empty());
    }

    #[test]
    fn a_failed_poll_carries_a_reason_not_an_empty_table() {
        let d = Descriptors::failed("no client");
        assert!(d.rows.is_empty());
        assert!(!d.error.is_empty());
    }
}
