// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The 37-rung activity-launch ladder - **shelled out to, never transcribed**.
//!
//! `d1-eboot ladder --json` reads the 35 handler classes out of the eboot's own RTTI
//! and reports each one's rung id. Copying that table into a Rust array would be a second
//! description of the same fact (`design/format-description.md`), and it would drift the first time
//! a rung is renamed. So this module runs the tool and parses its JSON into a declared struct that
//! **fails loudly on mismatch**.
//!
//! ⚠ Two traps this module exists to keep out of the reducer:
//!
//! 1. **Depth is `max()`, not `last()`.** `d1_run::BOOTFLOW_SPINE` is 14 states ending at
//!    `bootflow:prepare_for_orbit`; a run that reached rung 30 and fell back through `cleanup` to
//!    `setup:orbit` has a *last* state of rung 27 and a *furthest* of 30. The headline is the
//!    furthest. (`C-DASH-FURTHEST-NOT-LATEST`.)
//! 2. **`cleanup` is rung 25 and is a NORMAL TRANSIT STATE.** It sits mid-ladder, between
//!    `character:prologue` (24) and `setup:orbit_loading` (26), and every clean boot passes through
//!    it. Rendering it as failure without reading its *reason* is a depth trap.
//!    (`C-DASH-CLEANUP-TRANSIT`.)
//!
//! Rungs 15 and 16 are absent from the tool's output. That is a property of the client, not a
//! parse bug, and the control pins it so a future change is noticed rather than absorbed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// One rung, exactly as `d1-eboot ladder --json` reports it. Extra fields in the JSON (vtable,
/// slots, overridden) are deliberately not modelled - the dashboard has no use for them and
/// modelling them would make an unrelated change to that tool break this parse.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Rung {
    pub rung: u32,
    pub state: String,
    #[serde(default)]
    pub short: String,
    #[serde(default)]
    pub class: String,
}

/// Where the rung table came from. A ladder we could not load is **not** an empty ladder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum LadderSource {
    /// `d1-eboot ladder --json`, with the resolved tool path.
    EbootLadder(String),
    /// The tool could not be run or its output could not be parsed. Carries the reason verbatim.
    Unavailable(String),
}

impl LadderSource {
    pub fn describe(&self) -> String {
        match self {
            LadderSource::EbootLadder(p) => format!("{p} ladder --json"),
            LadderSource::Unavailable(why) => format!("NOT OBSERVED - {why}"),
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, LadderSource::EbootLadder(_))
    }
}

/// The loaded ladder.
#[derive(Debug, Clone)]
pub struct Ladder {
    pub rungs: Vec<Rung>,
    pub source: LadderSource,
    by_state: BTreeMap<String, u32>,
}

impl Ladder {
    /// The highest rung id in the table - the denominator of "rung N of M".
    pub fn max_rung(&self) -> u32 {
        self.rungs.iter().map(|r| r.rung).max().unwrap_or(0)
    }

    /// Rung id for a state name, or `None` if the client named a state the table does not carry.
    /// `None` is a finding (the ladder drifted), never a silent 0.
    pub fn rung_of(&self, state: &str) -> Option<u32> {
        self.by_state.get(state).copied()
    }

    pub fn name_of(&self, rung: u32) -> Option<&str> {
        self.rungs
            .iter()
            .find(|r| r.rung == rung)
            .map(|r| r.state.as_str())
    }

    /// An empty ladder with a stated reason. Used when the tool is unreachable; every consumer
    /// must check [`LadderSource::is_available`] before reporting a rung number.
    pub fn unavailable(why: impl Into<String>) -> Ladder {
        Ladder {
            rungs: Vec::new(),
            source: LadderSource::Unavailable(why.into()),
            by_state: BTreeMap::new(),
        }
    }

    /// Build a ladder from a literal rung list, for fixtures and controls. Marked as coming from a
    /// fixture so nothing can mistake it for the client's own table.
    pub fn from_rungs_for_tests(rungs: Vec<Rung>) -> Ladder {
        Ladder::from_rungs(rungs, Path::new("<fixture>"))
    }

    fn from_rungs(rungs: Vec<Rung>, script: &Path) -> Ladder {
        let by_state = rungs
            .iter()
            .map(|r| (r.state.clone(), r.rung))
            .collect::<BTreeMap<_, _>>();
        Ladder {
            rungs,
            source: LadderSource::EbootLadder(script.display().to_string()),
            by_state,
        }
    }
}

/// Locate the `d1-eboot` binary: `D1_REPO` first, then by ascending from `start` and from the cwd.
///
/// It is invoked as a SUBPROCESS rather than linked: `d1-trace` deliberately sits outside the
/// `d1-server` workspace, and a library call would drag `iced-x86` in and require the pinned eboot
/// at dash runtime. `d1-eboot ladder --json` is byte-identical to what `state-machine.py --json`
/// produced (16,778 bytes, verified 2026-08-11), so the parse below is unchanged.
fn find_tool(start: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(repo) = std::env::var("D1_REPO") {
        let p = Path::new(&repo).join(TOOL_DIR).join(name);
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
            let cand = p.join(TOOL_DIR).join(name);
            if cand.is_file() {
                return Some(cand);
            }
            cur = p.parent();
        }
    }
    None
}

/// Where `find_tool` looks, relative to a repo root.
const TOOL_DIR: &str = "d1-server/target/release";

/// Run `d1-eboot ladder --json` and parse it. About 0.02 s, down from the 0.75 s the Python cost
/// (which spawned `d1-eboot` 36 times and loaded two 47k-row TSVs to answer this). Callers still
/// load it once.
pub fn load(near: &Path) -> Ladder {
    let Some(script) = find_tool(near, "d1-eboot") else {
        return Ladder::unavailable(
            "could not find d1-server/target/release/d1-eboot by ascending from the run directory \
             or the cwd; set D1_REPO, or `cd d1-server && cargo build --release`",
        );
    };
    let out = match Command::new(&script).arg("ladder").arg("--json").output() {
        Ok(o) => o,
        Err(e) => return Ladder::unavailable(format!("running {}: {e}", script.display())),
    };
    if !out.status.success() {
        return Ladder::unavailable(format!(
            "{} exited {}: {}",
            script.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // Declared struct, not `["field"]["sub"]` indexing: a shape change fails here, loudly.
    match serde_json::from_slice::<Vec<Rung>>(&out.stdout) {
        Ok(rungs) if !rungs.is_empty() => Ladder::from_rungs(rungs, &script),
        Ok(_) => Ladder::unavailable(format!("{} returned an empty table", script.display())),
        Err(e) => Ladder::unavailable(format!("parsing {} --json: {e}", script.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_is_not_an_empty_ladder() {
        let l = Ladder::unavailable("no python");
        assert!(!l.source.is_available());
        assert_eq!(l.max_rung(), 0);
        assert!(l.source.describe().contains("NOT OBSERVED"));
    }

    #[test]
    fn parses_the_declared_shape_and_ignores_extra_fields() {
        let json = r#"[{"class":"c_x","short":"x","rung":27,"state":"setup:orbit",
                        "vtable":123,"slots":{"enter":1},"overridden":{"enter":true}}]"#;
        let rungs: Vec<Rung> = serde_json::from_str(json).unwrap();
        assert_eq!(rungs[0].rung, 27);
        assert_eq!(rungs[0].state, "setup:orbit");
    }
}
