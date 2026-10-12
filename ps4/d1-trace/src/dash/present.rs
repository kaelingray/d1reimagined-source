// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The **present chokepoint** - one struct producing label / value / provenance / state rows, which
//! the panels render and nothing else.
//!
//! `historicaldocs/design/interface-architecture.md`'s rule: a new fact reaches every surface by editing one
//! function. `d1-dash --json board` prints exactly this struct, so **the GUI has no fact the CLI
//! cannot print**, and every control runs against the JSON rather than against pixels.
//!
//! ## The three-state rule lives here
//!
//! [`Cell`] is `Ok` / `Empty` / `NotObserved`, and they serialise to three distinct tokens. No code
//! path maps `NotObserved` to `Ok`. This is the whole point: four of the 2026-08-08 session's most
//! expensive failures produced **no error line at all**, so an error-only board shows a clean green
//! screen through every one of them. **Absence of red is not green.**
//!
//! ## And so does the provenance rule
//!
//! Every [`Row`] carries a [`Src`]: the file it was scraped from, the exact line-shape it matched,
//! and whether the value is `Verbatim` (printed by the source) or `Derived` (a count, delta or
//! lookup we computed). *A log line is a rendering, not a value* - on 2026-08-08 a byte-reversed
//! printf caused a **correct** encoder to be "fixed", breaking a channel that had reached
//! `_connected`. A dashboard is all renderings, so it says where each one came from.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::ladder::{Ladder, Rung};
use super::probe::Descriptors;
use super::reduce;
use super::reduce::{AcFlags, ErrorEvent, KtoLine, Reducer, Transition};

/// R1: three states, and grey must never render as green.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Cell {
    /// Observed, and observed healthy.
    #[serde(rename = "OK")]
    Ok,
    /// The mechanism ran and carried nothing - a family replicated with `object_count 0`, a message
    /// answered with a zero-length body. **Not green.**
    #[serde(rename = "PASSED_EMPTILY")]
    Empty,
    /// We never read it, or the instrument could not read it. **Not green, and not zero.**
    #[serde(rename = "NOT_OBSERVED")]
    NotObserved,
}

impl Cell {
    /// The glyph carries the shape distinction the colour alone must not carry.
    pub fn glyph(self) -> &'static str {
        match self {
            Cell::Ok => "✅",
            Cell::Empty => "⚠",
            Cell::NotObserved => "⬜",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Cell::Ok => "OK",
            Cell::Empty => "PASSED_EMPTILY",
            Cell::NotObserved => "NOT_OBSERVED",
        }
    }
}

/// Is the number printed by the source, or computed by us?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Prov {
    Verbatim,
    Derived,
}

/// R2: where a number came from. Both strings are non-empty by construction - the control asserts it
/// over the whole board with no exemptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Src {
    pub file: String,
    pub line_shape: String,
    pub prov: Prov,
}

impl Src {
    pub fn verbatim(file: &str, line_shape: &str) -> Src {
        Src {
            file: file.into(),
            line_shape: line_shape.into(),
            prov: Prov::Verbatim,
        }
    }
    pub fn derived(file: &str, line_shape: &str) -> Src {
        Src {
            file: file.into(),
            line_shape: line_shape.into(),
            prov: Prov::Derived,
        }
    }
    pub fn hover(&self) -> String {
        format!(
            "{}  ·  matched: {}  ·  {}",
            self.file,
            self.line_shape,
            match self.prov {
                Prov::Verbatim => "VERBATIM (printed by the source)",
                Prov::Derived => "DERIVED (computed by d1-dash)",
            }
        )
    }
}

/// One presented fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub state: Cell,
    pub src: Src,
    /// The raw source line, unmodified - no reformatting, no byte-swapping, no normalising.
    pub raw: Option<String>,
    /// What this row must not be misread as.
    pub note: Option<String>,
}

impl Row {
    pub fn new(label: &str, value: impl Into<String>, state: Cell, src: Src) -> Row {
        Row {
            label: label.into(),
            value: value.into(),
            state,
            src,
            raw: None,
            note: None,
        }
    }
    pub fn raw(mut self, raw: impl Into<String>) -> Row {
        self.raw = Some(raw.into());
        self
    }
    pub fn note(mut self, note: &str) -> Row {
        self.note = Some(note.into());
        self
    }
}

/// A rung the client is on or has been past.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RungRef {
    pub rung: Option<u32>,
    pub state: String,
    pub reason: Option<String>,
}

/// R3: a lane's own freshness. `sockets.log` stops after exactly 300 s and then shows five-minute-old
/// data forever, with no in-band marker that it went quiet.
///
/// ⚠ **Deviation from `historicaldocs/design/d1-dashboard.md` §7's C-DASH-STALE, stated rather than hidden.** The
/// spec words that control against wall-clock ("`emulator.log` on a live run is not stale"), which
/// cannot be evaluated on a closed run and so would be a control that can only ever be run by hand.
/// Staleness here is measured **relative to the newest lane in the same run** - is this source
/// frozen while its siblings kept moving? - which is the axis that actually distinguishes *frozen*
/// from *quiet*, and which a closed run can answer. Absolute wall-clock age is carried alongside so
/// nothing is lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lane {
    pub file: String,
    pub present: bool,
    pub bytes: u64,
    pub mtime_epoch: Option<u64>,
    /// Seconds this lane is behind the newest lane in the run.
    pub behind_secs: Option<u64>,
    pub stale: bool,
    pub state: Cell,
    pub note: String,
}

/// Seconds a lane may lag the newest lane in the run before it is called frozen.
pub const STALE_BEHIND_SECS: u64 = 60;

/// The guest's own halt evidence - read from **both** paths.
///
/// ⛔ `d1_run` looks only for `guest-crash-report/crash_info.txt`, but the responder now writes
/// `uploads/ticket_drop-*.bin`. `runs/20260808-172032-ACTIVITY-LADDER/` has only the latter, so
/// `d1 show` reports `guest_halt: None` over a real crash. **A green "no halt" over a crash is the
/// worst cell on the page.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Halt {
    pub state: Cell,
    pub path: Option<String>,
    pub detail: String,
    /// Entry names read out of the ZIP's central directory. The bodies are DEFLATEd and are not
    /// decompressed here - `d1-dash` adds no compression dependency to read a filename.
    pub entries: Vec<String>,
}

/// The whole presented board. `d1-dash --json board` prints this and nothing else.
#[derive(Debug, Clone, Serialize)]
pub struct Board {
    pub run_id: String,
    pub run_dir: String,
    /// Permanent chrome, not a field: everything past `/SignOn` is downstream of our own answers.
    pub reach: String,

    // ---- D1: the ladder -----------------------------------------------------------------------
    pub ladder_source: String,
    pub rung_table: Vec<Rung>,
    pub max_rung: u32,
    pub furthest: Option<RungRef>,
    pub current: Option<RungRef>,
    pub states_in_order: Vec<String>,
    pub state_counts: BTreeMap<String, u64>,
    pub transitions: Vec<Transition>,
    /// ⛔ Zero state lines is an INSTRUMENT FAULT, not rung 0.
    pub instrument_fault: bool,
    pub instrument_note: Option<String>,

    // ---- D2: the error tape -------------------------------------------------------------------
    pub errors: Vec<ErrorEvent>,
    /// ⛔ "errors: 0" is not success. This string never reads `✅ 0 errors`.
    pub error_note: String,

    // ---- D3: header / provenance / health -----------------------------------------------------
    pub header: Vec<Row>,
    pub lanes: Vec<Lane>,
    pub halt: Halt,

    // ---- D4: the activity-client flag strip + the KTOBUCKET autopsy ---------------------------
    pub ac: Option<AcFlags>,
    pub ac_rows: Vec<Row>,
    pub ktobucket: Vec<KtoLine>,
    pub ktobucket_stamp: Option<String>,
    pub ktobucket_total: u64,
    pub ktobucket_note: String,

    // ---- D5: the QueueZ family table ----------------------------------------------------------
    /// Authoritative column: the client's own descriptor table via `d1guest.py descriptors`.
    pub families: Vec<Row>,
    pub family_source: String,
    pub liveness: Vec<Row>,
    /// Corroborating column: the guest's own `push:queuez` narration. **Never merged** with the one
    /// above - state 2 is written locally at send time, so a send log proves nothing about
    /// replication.
    pub queuez_log: Vec<Row>,

    // ---- D6: the tape -------------------------------------------------------------------------
    pub subsystems: BTreeMap<String, u64>,
    pub guest_lines: u64,
    pub raw_lines: u64,
    pub tape_len: usize,
    pub tape_dropped: u64,

    pub notes: Vec<String>,
}

impl Board {
    /// Every row on the board, for `C-DASH-PROVENANCE-COMPLETE`. No exemptions.
    pub fn all_rows(&self) -> Vec<&Row> {
        self.header
            .iter()
            .chain(self.ac_rows.iter())
            .chain(self.families.iter())
            .chain(self.liveness.iter())
            .chain(self.queuez_log.iter())
            .collect()
    }

    /// One line for a terminal.
    pub fn headline(&self) -> String {
        if self.instrument_fault {
            return format!(
                "{} · ⛔ INSTRUMENT FAULT - no guest narration in emulator.log; NO RUNG REPORTED",
                self.run_id
            );
        }
        match &self.furthest {
            Some(f) => format!(
                "{} · furthest rung {} of {} - {} · current {} · errors {}",
                self.run_id,
                f.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                self.max_rung,
                f.state,
                self.current
                    .as_ref()
                    .map(|c| c.state.clone())
                    .unwrap_or_else(|| "?".into()),
                self.errors.len()
            ),
            None => format!("{} · no state transitions observed", self.run_id),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Building the board
// ---------------------------------------------------------------------------------------------

/// The lanes a run directory can carry, in the order the header shows them.
pub const LANE_FILES: &[&str] = &[
    "emulator.log",
    "responder.log",
    "activity-host.log",
    "shadnet.log",
    "sockets.log",
    "account-trace.bin",
];

/// Assemble the board from a reduced run plus the optional live-memory lane.
///
/// `descriptors` is `None` when the live lane was not polled at all - which is **not** the same as
/// polling it and getting nothing, and is rendered differently.
pub fn build(
    run_dir: &Path,
    ladder: &Ladder,
    r: &Reducer,
    descriptors: Option<&Descriptors>,
    mut notes: Vec<String>,
) -> Board {
    let run_id = run_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| run_dir.display().to_string());

    // ---- provenance -------------------------------------------------------------------------
    let prov = d1_run::summarize_run(run_dir)
        .map(|s| s.provenance)
        .unwrap_or_default();
    let quiet = prov.quiet(&d1_run::QuietThresholds::default());
    let reach = prov
        .info
        .get("reach")
        .cloned()
        .unwrap_or_else(|| "unrecorded".into());

    // ---- lanes ------------------------------------------------------------------------------
    let lanes = build_lanes(run_dir);

    // ---- header -----------------------------------------------------------------------------
    let mut header = Vec::new();
    header.push(
        Row::new(
            "run",
            run_id.clone(),
            Cell::Ok,
            Src::verbatim("RUN-INFO.txt", "run: <id>"),
        )
        .raw(format!("run:  {run_id}")),
    );
    header.push(
        Row::new(
            "reach",
            format!("{reach} · synthetic_ancestor"),
            if reach == "unrecorded" {
                Cell::NotObserved
            } else {
                Cell::Ok
            },
            Src::verbatim("RUN-INFO.txt", "reach: <tag>"),
        )
        .note(
            "synthetic_ancestor is permanent chrome, not a field: everything past /SignOn is \
             downstream of answers our own server invented.",
        ),
    );
    let (quiet_txt, quiet_cell) = match &quiet {
        d1_run::Quiet::Yes => ("Yes".to_string(), Cell::Ok),
        d1_run::Quiet::No(rs) => (format!("** NO ** - {}", rs.join("; ")), Cell::Empty),
        // ⚠ Unknown ≠ Yes. A determinism claim from a run with no host-quiet evidence is [O].
        d1_run::Quiet::Unknown => ("Unknown - no host-condition samples".into(), Cell::NotObserved),
    };
    header.push(
        Row::new(
            "host quiet",
            quiet_txt,
            quiet_cell,
            Src::derived("RUN-INFO.txt", "psi-*-pre: avg10=… / loadavg-pre: …"),
        )
        .note("Unknown is NOT Yes - it means the question is unanswerable for this run."),
    );
    let (cont_txt, cont_cell) = match prov.containment_ok {
        Some(true) => ("asserted and passed".to_string(), Cell::Ok),
        Some(false) => ("** ABORT recorded **".to_string(), Cell::Empty),
        None => ("no proof recorded".to_string(), Cell::NotObserved),
    };
    header.push(Row::new(
        "containment",
        cont_txt,
        cont_cell,
        Src::derived("containment.txt", "any line containing ABORT"),
    ));
    let (gpu_txt, gpu_cell) = match &r.gpu_model {
        Some(m) if m.contains(reduce::GPU_SOFTWARE) => {
            (format!("⛔ SOFTWARE - {m}"), Cell::Empty)
        }
        Some(m) => (m.clone(), Cell::Ok),
        None => ("not printed yet".to_string(), Cell::NotObserved),
    };
    header.push(
        Row::new(
            "GPU",
            gpu_txt,
            gpu_cell,
            Src::verbatim("emulator.log", "CollectDeviceParameters: GPU_Model: <name>"),
        )
        .note(
            "llvmpipe is CPU software rasterisation: 268 of this project's runs rendered on it \
             and the 6-7 fps in-game was that, not the game. Red means relaunch, not debug.",
        ),
    );

    // ---- halt (both paths) ------------------------------------------------------------------
    let halt = read_halt(run_dir);
    header.push(
        Row::new(
            "guest halt",
            halt.detail.clone(),
            halt.state,
            Src::derived(
                "guest-crash-report/crash_info.txt  +  uploads/ticket_drop-*.bin",
                "halt information: <reason>  |  ZIP central directory entry names",
            ),
        )
        .note(
            "Both paths are read. d1_run reads only the first; a run with only uploads/ reports \
             guest_halt: None over a real crash.",
        ),
    );

    // ---- ladder -----------------------------------------------------------------------------
    let instrument_fault = r.instrument_fault();
    let instrument_note = if instrument_fault {
        Some(
            "emulator.log has content but ZERO guest [stdout] lines. That is the guest-diagnostic \
             sink, not the game: 626 of 1,614 diagnostic sites already pass the default filter and \
             still emit nothing, so this looks exactly like 'the game never got there'. Apply \
             patches/CUSA00219-guest-diagnostics.xml (it patches the SINK, not just the filter). \
             NO RUNG IS REPORTED."
                .to_string(),
        )
    } else if r.transitions.is_empty() && r.guest_lines > 0 {
        Some(
            "guest narration is present but no world_controller:state_manager transition has been \
             printed yet - the ladder is genuinely unstarted, not missing."
                .to_string(),
        )
    } else {
        None
    };

    let furthest = if instrument_fault {
        None
    } else {
        r.furthest().map(|(rung, state)| RungRef {
            rung: Some(rung),
            state,
            reason: None,
        })
    };
    let current = if instrument_fault {
        None
    } else {
        r.current().map(|(rung, state, reason)| RungRef {
            rung,
            state,
            reason: Some(reason),
        })
    };

    header.push(
        Row::new(
            "furthest rung",
            match &furthest {
                Some(f) => format!(
                    "{} of {} - {}",
                    f.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                    ladder.max_rung(),
                    f.state
                ),
                None => "-".to_string(),
            },
            if furthest.is_some() {
                Cell::Ok
            } else {
                Cell::NotObserved
            },
            Src::derived(
                "emulator.log",
                "world_controller:state_manager: Entering state 'X' for reason 'Y'.",
            ),
        )
        .note(
            "FURTHEST, not latest: max() over entered rungs. A run that reached 30 and fell back \
             through cleanup to setup:orbit still reads 30.",
        ),
    );
    header.push(
        Row::new(
            "current rung",
            match &current {
                Some(c) => format!(
                    "{} - {} (entered for '{}')",
                    c.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                    c.state,
                    c.reason.clone().unwrap_or_default()
                ),
                None => "-".to_string(),
            },
            if current.is_some() {
                Cell::Ok
            } else {
                Cell::NotObserved
            },
            Src::derived(
                "emulator.log",
                "the LAST 'Entering state' line, whatever its rung",
            ),
        )
        .note("A rung the client is SITTING IN is a different fact from one it passed through."),
    );

    // ---- errors -----------------------------------------------------------------------------
    let error_note = build_error_note(&current, &r.errors);
    header.push(
        Row::new(
            "error.* observed",
            r.errors.len().to_string(),
            if r.errors.is_empty() {
                // ⛔ Zero errors is NOT green. Rung 35 activity:physics_join has no timeout and no
                // failure path at all, so quiet and hung are indistinguishable from this lane.
                Cell::NotObserved
            } else {
                Cell::Empty
            },
            Src::derived(
                "emulator.log",
                "Leaving state 'X' for reason 'error.…'",
            ),
        )
        .note(&error_note),
    );

    // ---- D4 ---------------------------------------------------------------------------------
    let ac_rows = build_ac_rows(r.ac.as_ref());
    let ktobucket_note = if r.ktobucket.is_empty() {
        "no KTOBUCKET block seen. KTOBUCKET fires only on a STATE ERROR - its absence means no \
         state error has been reported, not that the client is healthy."
            .to_string()
    } else {
        format!(
            "AUTOPSY, NOT A HEARTBEAT - this block was printed at the last state error{}. Its \
             numeric interior is rendered verbatim and is NOT parsed: the client's own printf emits \
             visible garbage there ([qlen r/t 8/-273202064][type -48]).",
            match &r.ktobucket_stamp {
                Some(s) => format!(" ({s})"),
                None => String::new(),
            }
        )
    };

    // ---- D5 ---------------------------------------------------------------------------------
    let (families, family_source, liveness) = build_families(descriptors, &run_id);
    let queuez_log = build_queuez_log(r);

    // ⚠ The live lane is NOT scoped to the run directory on screen. `d1guest.py` attaches to
    // whatever `shadPS4:Main` is running right now, so a closed run rendered beside a live client
    // shows two different processes on one page. Say so, loudly, and only when both are true -
    // gating on `descriptors.is_some()` keeps `--json board` (no live poll) free of a wall clock
    // and therefore deterministic.
    if descriptors.is_some() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let emu_age = lanes
            .iter()
            .find(|l| l.file == "emulator.log")
            .and_then(|l| l.mtime_epoch)
            .map(|t| now.saturating_sub(t));
        if emu_age.is_some_and(|a| a > 300) {
            notes.push(format!(
                "⚠⚠ TWO DIFFERENT PROCESSES ON ONE PAGE: this run's emulator.log last grew {}s \
                 ago, but the QUEUEZ FAMILIES table below was read from the client running RIGHT \
                 NOW. The live lane attaches to whatever shadPS4 process exists; it is not scoped \
                 to {run_id}.",
                emu_age.unwrap_or(0)
            ));
        }
    }

    if instrument_fault {
        notes.push(
            "⛔ INSTRUMENT FAULT: no guest [stdout] lines. Every ladder fact on this board is \
             NOT OBSERVED."
                .to_string(),
        );
    }
    if !ladder.source.is_available() {
        notes.push(format!(
            "⚠ rung numbers are NOT OBSERVED - {}",
            ladder.source.describe()
        ));
    }
    if r.lossy {
        notes.push(
            "⚠ a lane decoded lossily (invalid bytes shown as U+FFFD); no lines were dropped"
                .to_string(),
        );
    }
    if r.tape_dropped > 0 {
        notes.push(format!(
            "tape scrollback capped: {} older guest lines aged out of the ring buffer (the counts \
             and the ladder are over ALL lines, not just the tape)",
            r.tape_dropped
        ));
    }

    Board {
        run_id,
        run_dir: run_dir.display().to_string(),
        reach,
        ladder_source: ladder.source.describe(),
        rung_table: ladder.rungs.clone(),
        max_rung: ladder.max_rung(),
        furthest,
        current,
        states_in_order: r.states_in_order.clone(),
        state_counts: r.state_counts.clone(),
        transitions: r.transitions.clone(),
        instrument_fault,
        instrument_note,
        errors: r.errors.clone(),
        error_note,
        header,
        lanes,
        halt,
        ac: r.ac.clone(),
        ac_rows,
        ktobucket: r.ktobucket.clone(),
        ktobucket_stamp: r.ktobucket_stamp.clone(),
        ktobucket_total: r.ktobucket_total,
        ktobucket_note,
        families,
        family_source,
        liveness,
        queuez_log,
        subsystems: r.subsystems.clone(),
        guest_lines: r.guest_lines,
        raw_lines: r.raw_lines,
        tape_len: r.tape.len(),
        tape_dropped: r.tape_dropped,
        notes,
    }
}

/// ⛔ The empty state never reads `✅ 0 errors`. It names the current rung's failure signature and
/// says plainly that silence is not evidence.
fn build_error_note(current: &Option<RungRef>, errors: &[ErrorEvent]) -> String {
    if !errors.is_empty() {
        return format!(
            "{} error.* leaving-reason(s) observed, in order, each with the state it left and the \
             state it entered.",
            errors.len()
        );
    }
    let sig = match current.as_ref().and_then(|c| c.rung) {
        Some(28) => "rung 28 fails as 'timed_out_waiting_to_establish_client' - all AC lamps dark \
                     with no error is that shape",
        Some(30) => "rung 30 fails as \
                     'starting_private_activity_instance_timed_out_waiting_for_join_response'",
        Some(35) => "⛔ rung 35 activity:physics_join has NO TIMEOUT AND NO FAILURE PATH - a \
                     documented silent 150-tick hang. Quiet and hung are indistinguishable from \
                     this lane alone",
        _ => "this rung's failure signature is not catalogued",
    };
    format!("no error observed - {sig}; silence here is not evidence.")
}

fn build_ac_rows(ac: Option<&AcFlags>) -> Vec<Row> {
    let src = Src::verbatim(
        "emulator.log",
        "networking:activity_client*: [AC <privacy> <currency> CON-? EST-? AH->? MEM-?]",
    );
    let Some(a) = ac else {
        return vec![
            Row::new("AC strip", "-", Cell::NotObserved, src)
                .note("no [AC …] line has been printed in this run"),
        ];
    };
    let lamp = |v: Option<bool>| match v {
        Some(true) => Cell::Ok,
        Some(false) => Cell::Empty,
        None => Cell::NotObserved,
    };
    vec![
        Row::new("privacy", a.privacy.clone(), Cell::Ok, src.clone()).raw(a.raw_line.clone()),
        Row::new("currency", a.currency.clone(), Cell::Ok, src.clone()).raw(a.raw_line.clone()),
        Row::new("CON (channel connected)", yn_text(a.connected), lamp(a.connected), src.clone())
            .raw(a.raw_line.clone()),
        Row::new(
            "EST (client established)",
            yn_text(a.established),
            lamp(a.established),
            src.clone(),
        )
        .raw(a.raw_line.clone())
        .note(
            "⚠ CON-Y EST-N is exactly the 2026-08-08 failure: connected but never established. A \
             panel showing only 'connected' reads that as success.",
        ),
        Row::new(
            "AH→ (activity-host id)",
            a.ah.clone(),
            if a.ah == "0" || a.ah.starts_with("---") {
                Cell::Empty
            } else {
                Cell::Ok
            },
            src.clone(),
        )
        .raw(a.raw_line.clone())
        .note("⚠ '0' (never set) and '--------:--------' (cleared) are DIFFERENT and are not collapsed."),
        Row::new(
            "MEM (membership)",
            a.mem.clone(),
            if a.mem == "0" { Cell::Empty } else { Cell::Ok },
            src,
        )
        .raw(a.raw_line.clone()),
    ]
}

fn yn_text(v: Option<bool>) -> String {
    match v {
        Some(true) => "Y".into(),
        Some(false) => "N".into(),
        None => "-".into(),
    }
}

/// The families the client's descriptor table is expected to carry. Used to emit **NOT OBSERVED**
/// placeholders rather than a table of literal zeros when the live lane cannot be read.
pub const EXPECTED_FAMILY_CLASSES: &[u32] = &[0, 2, 3, 4, 5];

/// ⭐⭐ **The three-state rule for a family, in one function.**
///
/// - `state=3` and a positive object count ⇒ [`Cell::Ok`]
/// - `state=3` and `object_count 0` ⇒ [`Cell::Empty`] - **the third state, not green**. Family 2
///   replicated cleanly and carried nothing, and the only tells were one `object-count of zero`
///   line and `peer_name=<UNKNOWN PEER NAME>` **in a different subsystem's log**.
/// - no replication state at all ⇒ [`Cell::NotObserved`] - never `Ok`, never a literal `0`.
/// - anything else ⇒ [`Cell::Empty`]. ⭐ **`state=3` or it did not happen**; state 2 is written
///   LOCALLY at send time, so it proves nothing about replication.
///
/// ⚠ Callers pass only what their own column knows. The descriptor table carries a state and no
/// object count; the `push:queuez` log carries an object count and no trustworthy state. **They are
/// deliberately not joined** - that is the merge the spec forbids. One rule, two columns.
pub fn family_cell(state: Option<u32>, object_count: Option<u64>) -> Cell {
    match (state, object_count) {
        (None, None) => Cell::NotObserved,
        (Some(3), Some(0)) => Cell::Empty,
        (Some(3), _) => Cell::Ok,
        (Some(_), _) => Cell::Empty,
        (None, Some(0)) => Cell::Empty,
        (None, Some(_)) => Cell::Ok,
    }
}

fn build_families(d: Option<&Descriptors>, run_id: &str) -> (Vec<Row>, String, Vec<Row>) {
    let src = Src::verbatim(
        "python3 -B capture-rig/d1guest.py descriptors  (THE LIVE CLIENT, not this run directory)",
        "[NNN] class=C key=0x… state=S <text> err=E ver=V",
    );
    let live_src = Src::verbatim(
        "python3 -B capture-rig/d1guest.py descriptors",
        "LIVENESS: L<n>  /  L<n> PASS|FAIL <text>",
    );
    let Some(d) = d else {
        // ⚠ C-DASH-GREY-NOT-ZERO: not one tile may render the literal 0 here. A green zero has
        // already published a whole retracted causal theory in this project.
        let rows = EXPECTED_FAMILY_CLASSES
            .iter()
            .map(|c| {
                Row::new(
                    &format!("family class {c}"),
                    "-",
                    Cell::NotObserved,
                    src.clone(),
                )
                .note("the live-memory lane was not polled (pass --live), or no client is running")
            })
            .collect();
        return (
            rows,
            "NOT OBSERVED - live lane not polled".to_string(),
            vec![Row::new(
                "d1guest liveness",
                "-",
                Cell::NotObserved,
                live_src,
            )
            .note("below L3 the whole memory table greys out rather than rendering zeros")],
        );
    };

    let mut liveness: Vec<Row> = d
        .liveness
        .iter()
        .map(|l| {
            Row::new(
                &l.rung,
                l.text.clone(),
                if l.pass { Cell::Ok } else { Cell::Empty },
                live_src.clone(),
            )
            .raw(l.raw.clone())
        })
        .collect();
    liveness.insert(
        0,
        Row::new(
            "liveness ladder",
            d.liveness_level
                .map(|n| format!("L{n}"))
                .unwrap_or_else(|| "-".into()),
            match d.liveness_level {
                Some(3) => Cell::Ok,
                Some(_) => Cell::Empty,
                None => Cell::NotObserved,
            },
            live_src,
        )
        .note("below L3 the whole memory table greys out rather than rendering zeros"),
    );

    if d.rows.is_empty() {
        let why = if d.error.is_empty() {
            "descriptors returned no rows".to_string()
        } else {
            d.error.clone()
        };
        let rows = EXPECTED_FAMILY_CLASSES
            .iter()
            .map(|c| {
                Row::new(&format!("family class {c}"), "-", Cell::NotObserved, src.clone())
                    .note(&why)
            })
            .collect();
        return (rows, format!("NOT OBSERVED - {why}"), liveness);
    }

    let rows = d
        .rows
        .iter()
        .map(|f| {
            // ⭐⭐ state=3 or it did not happen. The descriptor column knows a state and no object
            // count, so that is exactly what it passes - the join is not performed.
            let cell = family_cell(Some(f.state), None);
            Row::new(
                &format!("family class {}", f.class),
                format!(
                    "slot [{}]  key={}  state={} {}  err={}  ver={}",
                    f.slot, f.key, f.state, f.state_text, f.err, f.ver
                ),
                cell,
                src.clone(),
            )
            .raw(f.raw.clone())
            .note(
                "⭐ state=3 or it did not happen. A green cell may come from `descriptors` ONLY, \
                 never from responder.log - state 2 is written locally at send time. Single \
                 sample, torn read: there is no lock between /proc/pid/mem and the guest, so this \
                 is never smoothed or averaged.",
            )
        })
        .collect();
    (
        rows,
        format!(
            "d1guest.py descriptors - {} row(s), single torn sample at {} · ⚠ read from THE \
             CURRENTLY RUNNING CLIENT, which may not be the client that produced {run_id}",
            d.rows.len(),
            d.stamp
        ),
        liveness,
    )
}

fn build_queuez_log(r: &Reducer) -> Vec<Row> {
    let src = Src::verbatim(
        "emulator.log",
        "networking:investment:*:queuez: … [family_type ENUM(N), family_root_soid 0x…, object_count N]",
    );
    if r.queuez.is_empty() {
        return vec![Row::new(
            "queuez log lane",
            "-",
            Cell::NotObserved,
            src,
        )
        .note(
            "⚠ a (type, root) miss returns 0 with NO STRING OF ANY KIND - 'no error lines for this \
             family' is not evidence.",
        )];
    }
    r.queuez
        .values()
        .map(|q| {
            // ⚠ object_count 0 is the THIRD state, not green: family 2 replicated cleanly and
            // carried nothing, and the only tells were one `object-count of zero` line and
            // `peer_name=<UNKNOWN PEER NAME>` in a different subsystem's log.
            // The log column knows an object count and no trustworthy replication state.
            let mut cell = family_cell(None, q.object_count.or(if q.saw_zero { Some(0) } else { None }));
            if q.saw_failure || q.saw_unhandled {
                cell = Cell::Empty;
            }
            let mut flags = Vec::new();
            if q.saw_zero {
                flags.push("object-count-of-zero");
            }
            if q.saw_failure {
                flags.push("failed-to-process");
            }
            if q.saw_unhandled {
                flags.push("unhandled-replication-state");
            }
            Row::new(
                &format!("family_type ENUM({})", q.family_type),
                format!(
                    "root={}  object_count={}  {}",
                    q.root_soid.clone().unwrap_or_else(|| "-".into()),
                    q.object_count
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into()),
                    flags.join(" ")
                ),
                cell,
                src.clone(),
            )
            .raw(q.raw.join("\n"))
            .note(
                "CORROBORATING ONLY - this column never establishes replication. Rendered beside \
                 the descriptor table, never merged into it.",
            )
        })
        .collect()
}

fn build_lanes(run_dir: &Path) -> Vec<Lane> {
    let mut lanes: Vec<Lane> = Vec::new();
    let mut newest = 0u64;
    for f in LANE_FILES {
        let p = run_dir.join(f);
        let (present, bytes, mtime) = match std::fs::metadata(&p) {
            Ok(m) => (
                true,
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            ),
            Err(_) => (false, 0, None),
        };
        if let Some(t) = mtime {
            newest = newest.max(t);
        }
        lanes.push(Lane {
            file: (*f).to_string(),
            present,
            bytes,
            mtime_epoch: mtime,
            behind_secs: None,
            stale: false,
            state: if present && bytes > 0 {
                Cell::Ok
            } else if present {
                Cell::Empty
            } else {
                Cell::NotObserved
            },
            note: String::new(),
        });
    }
    for l in lanes.iter_mut() {
        if let Some(t) = l.mtime_epoch {
            let behind = newest.saturating_sub(t);
            l.behind_secs = Some(behind);
            l.stale = behind > STALE_BEHIND_SECS;
            if l.stale {
                l.note = format!(
                    "FROZEN - {behind}s behind the newest lane in this run. Neither sockets.log \
                     (which stops after exactly 300 s) nor activity-host.log (which freezes when \
                     the AH connection closes) carries any in-band marker that it went quiet."
                );
                l.state = Cell::Empty;
            }
        } else {
            l.note = "absent from this run directory".into();
        }
    }
    lanes
}

/// Read the halt evidence from **both** paths.
pub fn read_halt(run_dir: &Path) -> Halt {
    // Path 1: the harness-preserved crash report.
    let p = run_dir.join("guest-crash-report").join("crash_info.txt");
    if let Ok(bytes) = std::fs::read(&p) {
        let text = String::from_utf8_lossy(&bytes);
        let reason = text
            .lines()
            .find(|l| l.contains("halt information:"))
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| "crash_info.txt present".to_string());
        return Halt {
            state: Cell::Empty,
            path: Some(p.display().to_string()),
            detail: format!("HALT - {reason}"),
            entries: vec!["crash_info.txt".into()],
        };
    }
    // Path 2: the guest's own upload. ⛔ d1_run does not read this one.
    let uploads = run_dir.join("uploads");
    let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(&uploads)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("ticket_drop-"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    if let Some(p) = found.last() {
        let entries = std::fs::read(p).map(|b| zip_entry_names(&b)).unwrap_or_default();
        return Halt {
            state: Cell::Empty,
            path: Some(p.display().to_string()),
            detail: format!(
                "HALT - the guest uploaded its own crash report ({} bytes); ZIP carries {}",
                std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                if entries.is_empty() {
                    "no readable central directory".to_string()
                } else {
                    entries.join(", ")
                }
            ),
            entries,
        };
    }
    Halt {
        state: Cell::NotObserved,
        path: None,
        detail: "no crash report on either path - NOT OBSERVED, which is not 'no halt'".into(),
        entries: Vec::new(),
    }
}

/// Entry names from a ZIP central directory. Names are stored in plaintext regardless of the
/// compression method, so no decompressor is needed - and none is added.
fn zip_entry_names(bytes: &[u8]) -> Vec<String> {
    const SIG: &[u8] = b"PK\x01\x02";
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 46 <= bytes.len() {
        if &bytes[i..i + 4] != SIG {
            i += 1;
            continue;
        }
        let n = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
        let start = i + 46;
        if start + n <= bytes.len() {
            out.push(String::from_utf8_lossy(&bytes[start..start + n]).into_owned());
        }
        i = start + n.max(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_states_serialise_to_three_distinct_tokens() {
        let toks: Vec<&str> = [Cell::Ok, Cell::Empty, Cell::NotObserved]
            .iter()
            .map(|c| c.token())
            .collect();
        assert_eq!(toks, vec!["OK", "PASSED_EMPTILY", "NOT_OBSERVED"]);
        let json: Vec<String> = [Cell::Ok, Cell::Empty, Cell::NotObserved]
            .iter()
            .map(|c| serde_json::to_string(c).unwrap())
            .collect();
        assert_eq!(json.len(), 3);
        assert_ne!(json[0], json[1]);
        assert_ne!(json[1], json[2]);
        assert_ne!(json[0], json[2]);
        // …and they differ in SHAPE, not only colour.
        assert_ne!(Cell::Ok.glyph(), Cell::NotObserved.glyph());
    }

    #[test]
    fn zip_names_are_read_without_decompressing() {
        // A minimal central-directory record for "crash_info.txt", DEFLATEd body we never touch.
        let name = b"crash_info.txt";
        let mut z = vec![0u8; 0];
        z.extend_from_slice(b"junk-before-the-zip");
        z.extend_from_slice(b"PK\x01\x02");
        z.extend_from_slice(&[0u8; 24]);
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0u8; 16]);
        z.extend_from_slice(name);
        assert_eq!(zip_entry_names(&z), vec!["crash_info.txt".to_string()]);
    }

    #[test]
    fn an_absent_crash_report_is_not_observed_not_ok() {
        let d = std::env::temp_dir().join(format!("d1-dash-halt-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let h = read_halt(&d);
        assert_eq!(h.state, Cell::NotObserved);
        assert!(h.detail.contains("NOT OBSERVED"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_zero_error_count_never_reads_as_green() {
        let note = build_error_note(
            &Some(RungRef {
                rung: Some(35),
                state: "activity:physics_join".into(),
                reason: None,
            }),
            &[],
        );
        assert!(note.contains("silence here is not evidence"));
        assert!(note.contains("NO TIMEOUT"));
        assert!(!note.contains("✅"));
    }
}
