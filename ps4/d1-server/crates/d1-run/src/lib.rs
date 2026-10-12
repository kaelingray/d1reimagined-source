// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Run artifacts: parse a run directory into **structured, deterministic** events.
//!
//! ## Why this exists
//!
//! The project's scientific instrument is run-A-vs-run-B comparison, and until now that comparison was
//! done by hand with `grep`. Worse, `runs/` is gitignored, so evidence only survived by someone copying
//! it into a document. This turns a run directory into a structured summary that can be diffed,
//! promoted into a doc, and checked into git.
//!
//! ## The framing that drives the design
//!
//! Our instrument is an **attempt ledger**, not a traffic ledger: the gates we care about are defined
//! by requests that *fail*. So a resolver call with no answer, a refused connect and an `Unsupported`
//! reply are all first-class events here - the absence is the finding.
//!
//! ## Determinism is a requirement, not a nicety
//!
//! Every collection is a `BTreeMap`/`BTreeSet`, never a hash container, so two runs of the same
//! experiment produce **byte-identical** summaries and a diff means something really changed. A
//! summary that varied run-to-run would make the diff useless, which is the whole point of the tool.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The boot-path regression assert (`d1 gate`), ported from `capture-rig/gate-check.py`.
pub mod gate;

/// One structured observation extracted from a run's logs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Event {
    /// Game identity as the emulator resolved it.
    GameIdentity { title_id: String, app_version: String },
    /// A milestone the emulator reached (video-out opened, audio opened, pipeline compiled…).
    Milestone(String),
    /// `sceNetResolverStartNtoa` - a name the client wanted. The heart of the attempt ledger.
    NameResolution { hostname: String },
    /// An HTTP(S) call the client made or prepared.
    Http { call: String },
    /// NP identity state as reported by `sceNpGetOnlineId`.
    NpIdentity { signed_out: bool },
    /// A ShadNet command our own server saw, and whether we served it.
    ShadnetCommand { command: String, served: bool },
    /// A line the *game itself* printed - the richest source we have.
    GuestDiagnostic { subsystem: String, message: String },
    /// A guest fault: address plus the thread that took it.
    Fault { address: String, thread: String },
    /// `world_controller:state_manager: Entering state 'X' for reason 'Y'` - the **gate ladder**.
    ///
    /// This is the project's core metric: progress is measured in how many bootflow states fall, and
    /// a run that reaches one state deeper is a win even if it still ends in an error. Without it
    /// `diff` could only compare counts, which cannot answer "did we get further?".
    BootState { state: String, reason: String },
    /// The game's own `RELEASE_VHALT` halt reason, read from its crash report. Prose, from the engine.
    GuestHalt { halt: String, information: String },
}

impl Event {
    /// Short stable kind name, used for grouping and for diff output.
    pub fn kind(&self) -> &'static str {
        match self {
            Event::GameIdentity { .. } => "game-identity",
            Event::Milestone(_) => "milestone",
            Event::NameResolution { .. } => "name-resolution",
            Event::Http { .. } => "http",
            Event::NpIdentity { .. } => "np-identity",
            Event::ShadnetCommand { .. } => "shadnet-command",
            Event::GuestDiagnostic { .. } => "guest-diagnostic",
            Event::Fault { .. } => "fault",
            Event::BootState { .. } => "boot-state",
            Event::GuestHalt { .. } => "guest-halt",
        }
    }
}

/// Provenance copied out of `RUN-INFO.txt` / `containment.txt`, so a summary is self-describing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Key/value lines from `RUN-INFO.txt`, ordered.
    pub info: BTreeMap<String, String>,
    /// Whether containment was asserted and passed. `None` if no proof was recorded.
    pub containment_ok: Option<bool>,
    /// Raw containment proof lines.
    pub containment: Vec<String>,
}

/// Pre-declared quietness thresholds.
///
/// There is **no universal "quiet machine" standard**, so the project declares its own *in advance*
/// and makes quietness a **recorded predicate** rather than a judgement made after the fact. Chosen
/// from the incident that motivated all of this: a run during 16 GB of core-dump compression looked
/// "deterministic" and was not.
pub struct QuietThresholds {
    /// `some avg10` for CPU pressure, percent.
    pub psi_cpu_avg10: f64,
    /// `some avg10` for I/O pressure, percent.
    pub psi_io_avg10: f64,
    /// `some avg10` for memory pressure, percent.
    pub psi_mem_avg10: f64,
    /// 1-minute load average.
    pub loadavg_1: f64,
}

impl Default for QuietThresholds {
    fn default() -> Self {
        // CPU pressure is the one that actually flipped an outcome here, so it is strict.
        // I/O is deliberately loose: this box sustains ~80% io pressure at rest, and a run at that
        // level completed cleanly, so treating it as disqualifying would reject every run.
        Self { psi_cpu_avg10: 5.0, psi_io_avg10: 90.0, psi_mem_avg10: 5.0, loadavg_1: 8.0 }
    }
}

/// Verdict on whether a run's host was quiet enough for its results to carry weight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quiet {
    /// Host-condition samples present and all within threshold.
    Yes,
    /// Samples present, at least one threshold exceeded. Carries the reasons.
    No(Vec<String>),
    /// No host-condition samples recorded, so the question is unanswerable for this run.
    Unknown,
}

impl Provenance {
    /// True when the provenance carries host-condition samples at all.
    pub fn has_host_conditions(&self) -> bool {
        self.info.keys().any(|k| k.starts_with("psi-"))
    }

    /// Evaluate the quietness predicate against pre-declared thresholds.
    ///
    /// This is the mechanism behind the project rule that **a determinism claim from a run with no
    /// host-quiet evidence is `[O]`, not `[C]`** - and now also: a claim from a run that was
    /// *measurably not quiet* is `[O]` regardless of how clean its output looked.
    pub fn quiet(&self, t: &QuietThresholds) -> Quiet {
        if !self.has_host_conditions() {
            return Quiet::Unknown;
        }
        let mut reasons = Vec::new();
        let mut check = |key: &str, field: &str, limit: f64| {
            if let Some(v) = self.info.get(key).and_then(|s| parse_psi_avg10(s)) {
                if v > limit {
                    reasons.push(format!("{field} avg10={v:.2} exceeds {limit:.2}"));
                }
            }
        };
        check("psi-cpu-pre", "cpu", t.psi_cpu_avg10);
        check("psi-io-pre", "io", t.psi_io_avg10);
        check("psi-mem-pre", "mem", t.psi_mem_avg10);
        if let Some(l) = self
            .info
            .get("loadavg-pre")
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<f64>().ok())
        {
            if l > t.loadavg_1 {
                reasons.push(format!("loadavg-1={l:.2} exceeds {:.2}", t.loadavg_1));
            }
        }
        if reasons.is_empty() {
            Quiet::Yes
        } else {
            Quiet::No(reasons)
        }
    }
}

/// Pull `avg10=N` out of a PSI line such as `avg10=82.58 avg60=82.99 avg300=82.98`.
fn parse_psi_avg10(s: &str) -> Option<f64> {
    s.split_whitespace()
        .find_map(|tok| tok.strip_prefix("avg10=")) 
        .and_then(|v| v.parse().ok())
}

/// A whole run, reduced to something diffable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub name: String,
    pub provenance: Provenance,
    /// Count per event kind.
    pub counts: BTreeMap<String, u64>,
    /// Every distinct hostname the client tried to resolve. Sorted.
    pub hostnames: BTreeSet<String>,
    /// Distinct HTTP calls observed.
    pub http_calls: BTreeSet<String>,
    /// Guest diagnostic subsystems seen, with counts - how far the game narrated.
    pub guest_subsystems: BTreeMap<String, u64>,
    /// Distinct guest diagnostic lines, sorted. The primary source when present.
    pub guest_lines: BTreeSet<String>,
    /// ShadNet commands, and whether each was served.
    pub shadnet: BTreeMap<String, bool>,
    /// Faults, sorted.
    pub faults: BTreeSet<String>,
    /// Milestones reached.
    pub milestones: BTreeSet<String>,
    /// The bootflow ladder in the order the game entered the states. **Ordered, not a set** - the
    /// sequence is the evidence, and duplicates matter (a state re-entered after `cleanup` means the
    /// flow looped). Deterministic for a given log.
    pub boot_ladder: Vec<(String, String)>,
    /// The deepest bootflow state reached, and the reason it was entered.
    pub deepest_state: Option<String>,
    /// The reason attached to a terminal `cleanup`/halt, if the run ended in one.
    pub terminal_reason: Option<String>,
    /// The game's own halt information, when it wrote a crash report.
    pub guest_halt: Option<String>,
    /// `sceNpGetOnlineId` SIGNED_OUT count - 0 means the identity gate is open.
    pub signed_out_polls: u64,
    pub game_identity: Option<String>,
}

impl RunSummary {
    /// One-line human headline.
    pub fn headline(&self) -> String {
        let id = self.game_identity.clone().unwrap_or_else(|| "unknown".into());
        // Gate depth leads the headline: it is the metric that says whether a run made progress.
        let gate = match (&self.deepest_state, &self.terminal_reason) {
            (Some(d), Some(r)) => format!(" · gate={} ✗{}", d.trim_start_matches("bootflow:"), r.trim_start_matches("error.")),
            (Some(d), None) => format!(" · gate={}", d.trim_start_matches("bootflow:")),
            _ => String::new(),
        };
        let halt = match &self.guest_halt {
            Some(h) => format!(" · HALT[{h}]"),
            None => String::new(),
        };
        format!(
            "{} · {} · faults={} · signed_out={} · names={} · guest_lines={}{gate}{halt}",
            self.name,
            id,
            self.faults.len(),
            self.signed_out_polls,
            self.hostnames.len(),
            self.guest_lines.len()
        )
    }
}

/// Build a summary from a run directory (as produced by `run-contained-emulator.sh`).
pub fn summarize_run(dir: &Path) -> std::io::Result<RunSummary> {
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());

    let provenance = read_provenance(dir);
    let mut events = Vec::new();
    if let Ok(text) = read_lossy(&dir.join("emulator.log")) {
        events.extend(parse_emulator_log(&text));
    }
    if let Ok(text) = read_lossy(&dir.join("shadnet.log")) {
        events.extend(parse_shadnet_log(&text));
    }
    // Destiny's own crash report, if the harness preserved one. It names the halt cause in prose.
    if let Ok(text) = read_lossy(&dir.join("guest-crash-report").join("crash_info.txt")) {
        events.extend(parse_guest_crash_report(&text));
    }
    Ok(summarize(name, provenance, &events))
}

fn read_lossy(path: &Path) -> std::io::Result<String> {
    // Emulator logs carry ANSI colour codes and occasional non-UTF8; be forgiving.
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_provenance(dir: &Path) -> Provenance {
    let mut p = Provenance::default();
    if let Ok(text) = read_lossy(&dir.join("RUN-INFO.txt")) {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim();
                let v = v.trim();
                if !k.is_empty() && !v.is_empty() {
                    p.info.insert(k.to_string(), v.to_string());
                }
            }
        }
    }
    if let Ok(text) = read_lossy(&dir.join("containment.txt")) {
        p.containment = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        // The launcher writes "ABORT" into the proof if either assertion fails.
        p.containment_ok = Some(!p.containment.iter().any(|l| l.contains("ABORT")));
    }
    p
}

/// Strip ANSI escape sequences without pulling in a regex dependency.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip until the terminating byte of the sequence.
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn after<'a>(hay: &'a str, needle: &str) -> Option<&'a str> {
    hay.find(needle).map(|i| &hay[i + needle.len()..])
}

fn up_to_comma(s: &str) -> &str {
    s.split([',', ' ']).next().unwrap_or(s).trim()
}

/// Extract structured events from a shadPS4 emulator log.
/// The bootflow spine, in flow order - the ladder we measure depth against.
///
/// **Why a table and not "the last state seen":** on failure the flow runs `cleanup` and then
/// re-enters `bootflow:start`, so the chronologically-last state is `start` and a naive reading
/// reports a deep run as having gone nowhere. Depth must be the *furthest* state, not the latest.
///
/// Order is `[O]` from observed runs; the full set of 22 `bootflow:` states is `[C]` from string
/// literals in the eboot. States absent here are conditional branches (`eula`, `upsell`, `queuing`,
/// `content_install`, `confirm_account_transfer`, `email`, `illegal_dlc_media`,
/// `insufficient_content_space`) - they are still recorded in the ladder, they just do not define
/// depth. Extend this list rather than reordering it if a new state is observed in the spine.
pub const BOOTFLOW_SPINE: &[&str] = &[
    "bootflow:media_check",
    "bootflow:bootload",
    "bootflow:intro_screen_display",
    "bootflow:start",
    "bootflow:platform_signin",
    "bootflow:platform_account_warnings",
    "bootflow:enumerate_content_rights",
    "bootflow:account_signin",
    "bootflow:content_check",
    "bootflow:package_registration",
    "bootflow:bap_signin",
    "bootflow:investment_signin",
    "bootflow:profile_setup",
    "bootflow:prepare_for_orbit",
];

/// Position of a state on the spine, or `None` for conditional branches.
pub fn spine_rank(state: &str) -> Option<usize> {
    BOOTFLOW_SPINE.iter().position(|s| *s == state)
}

/// `Entering state 'X' for reason 'Y'.` -> (X, Y)
fn parse_boot_state(line: &str) -> Option<(String, String)> {
    let rest = line.split("Entering state ").nth(1)?;
    let mut it = rest.split('\'');
    it.next()?;
    let state = it.next()?.to_string();
    let rest2 = rest.split("for reason ").nth(1)?;
    let reason = rest2.split('\'').nth(1)?.to_string();
    Some((state, reason))
}

pub fn parse_emulator_log(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    // shadPS4 prints the title id and the app version on *separate* lines, so hold the title id
    // until the version shows up.
    let mut pending_title: Option<String> = None;
    for raw in text.lines() {
        let line = strip_ansi(raw);

        if line.contains("Entering state ") {
            if let Some((state, reason)) = parse_boot_state(&line) {
                events.push(Event::BootState { state, reason });
                continue;
            }
        }

        if let Some(rest) = after(&line, "Game id: ") {
            pending_title = Some(up_to_comma(rest).to_string());
            if let Some(v) = after(&line, "App Version: ") {
                events.push(Event::GameIdentity {
                    title_id: pending_title.take().unwrap_or_default(),
                    app_version: up_to_comma(v).to_string(),
                });
            }
            continue;
        }
        if let Some(v) = after(&line, "App Version: ") {
            events.push(Event::GameIdentity {
                title_id: pending_title.take().unwrap_or_default(),
                app_version: up_to_comma(v).to_string(),
            });
            continue;
        }

        if let Some(rest) = after(&line, "hostname = ") {
            events.push(Event::NameResolution { hostname: up_to_comma(rest).to_string() });
            continue;
        }

        if line.contains("sceNpGetOnlineId") {
            events.push(Event::NpIdentity { signed_out: line.contains("SIGNED_OUT") });
            continue;
        }

        if let Some(rest) = after(&line, "[stdout] ") {
            let msg = rest.trim().to_string();
            // Guest diagnostics are namespaced `subsystem:` or `networking:subsystem:`.
            let subsystem = guest_subsystem(&msg);
            events.push(Event::GuestDiagnostic { subsystem, message: msg });
            continue;
        }

        if let Some(rest) = after(&line, "Unhandled access violation at code address ") {
            let addr = up_to_comma(rest).trim_end_matches(':').to_string();
            let thread = line
                .split_once('(')
                .and_then(|(_, r)| r.split_once(')'))
                .map(|(t, _)| t.to_string())
                .unwrap_or_default();
            events.push(Event::Fault { address: addr, thread });
            continue;
        }

        for call in [
            "sceHttpsLoadCert",
            "sceHttpSendRequest",
            "sceHttpCreateRequest",
            "sceHttpCreateTemplate",
            "sceHttpInit",
        ] {
            if line.contains(call) {
                events.push(Event::Http { call: call.to_string() });
                break;
            }
        }

        for milestone in [
            "sceVideoOutOpen",
            "sceVideoOutSetBufferAttribute",
            "sceAudioOutOpen",
            "HideSplashScreen",
            "GetGraphicsPipeline",
            "sceNetInit",
            "sceSslInit",
        ] {
            if line.contains(milestone) {
                events.push(Event::Milestone(milestone.to_string()));
                break;
            }
        }
    }
    events
}

/// Namespace of a guest diagnostic line: `networking:signon: …` → `networking:signon`.
fn guest_subsystem(msg: &str) -> String {
    let mut parts = msg.split(':');
    let first = parts.next().unwrap_or("").trim();
    if first == "networking" {
        if let Some(second) = parts.next() {
            return format!("networking:{}", second.trim());
        }
    }
    first.to_string()
}

/// Extract the halt reason from Destiny's own crash report (`crash_info.txt`).
///
/// The engine writes this itself on `RELEASE_VHALT`, and it states the cause in prose - e.g.
/// `failed to load bootstrap_patchable`. That is cheaper and more reliable than lowering diagnostic
/// thresholds and guessing, so it belongs in every run summary.
pub fn parse_guest_crash_report(text: &str) -> Vec<Event> {
    let mut halt = String::new();
    let mut information = String::new();
    let mut section = "";
    for raw in text.lines() {
        let line = raw.trim_end();
        match line.trim() {
            "halt:" => { section = "halt"; continue }
            "halt information:" => { section = "info"; continue }
            _ => {}
        }
        // Values are indented under their heading; an unindented line ends the section.
        if !line.starts_with(' ') && !line.starts_with('\t') { section = ""; continue }
        let v = line.trim();
        if v.is_empty() { continue }
        match section {
            "halt" if halt.is_empty() => halt = v.to_string(),
            "info" if information.is_empty() => information = v.to_string(),
            _ => {}
        }
    }
    if halt.is_empty() && information.is_empty() {
        return Vec::new();
    }
    vec![Event::GuestHalt { halt, information }]
}

/// Extract events from our own ShadNet server's log.
pub fn parse_shadnet_log(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for raw in text.lines() {
        let line = strip_ansi(raw);
        if let Some(rest) = after(&line, "cmd=") {
            let cmd = up_to_comma(rest).to_string();
            if cmd.is_empty() {
                continue;
            }
            // A NEW GATE line means we refused it.
            let served = !line.contains("NEW GATE") && !line.contains("Unsupported");
            events.push(Event::ShadnetCommand { command: cmd, served });
        }
    }
    events
}

fn summarize(name: String, provenance: Provenance, events: &[Event]) -> RunSummary {
    let mut s = RunSummary {
        name,
        provenance,
        counts: BTreeMap::new(),
        hostnames: BTreeSet::new(),
        http_calls: BTreeSet::new(),
        guest_subsystems: BTreeMap::new(),
        boot_ladder: Vec::new(),
        deepest_state: None,
        terminal_reason: None,
        guest_halt: None,
        guest_lines: BTreeSet::new(),
        shadnet: BTreeMap::new(),
        faults: BTreeSet::new(),
        milestones: BTreeSet::new(),
        signed_out_polls: 0,
        game_identity: None,
    };
    for e in events {
        *s.counts.entry(e.kind().to_string()).or_insert(0) += 1;
        match e {
            Event::GameIdentity { title_id, app_version } => {
                if s.game_identity.is_none() && !app_version.is_empty() {
                    s.game_identity = Some(if title_id.is_empty() {
                        app_version.clone()
                    } else {
                        format!("{title_id} {app_version}")
                    });
                }
            }
            Event::NameResolution { hostname } => {
                s.hostnames.insert(hostname.clone());
            }
            Event::Http { call } => {
                s.http_calls.insert(call.clone());
            }
            Event::NpIdentity { signed_out } => {
                if *signed_out {
                    s.signed_out_polls += 1;
                }
            }
            Event::ShadnetCommand { command, served } => {
                // A command counts as unserved if it was ever refused.
                let entry = s.shadnet.entry(command.clone()).or_insert(*served);
                *entry = *entry && *served;
            }
            Event::GuestDiagnostic { subsystem, message } => {
                *s.guest_subsystems.entry(subsystem.clone()).or_insert(0) += 1;
                s.guest_lines.insert(message.clone());
            }
            Event::Fault { address, thread } => {
                s.faults.insert(format!("{address} ({thread})"));
            }
            Event::Milestone(m) => {
                s.milestones.insert(m.clone());
            }
            Event::BootState { state, reason } => {
                s.boot_ladder.push((state.clone(), reason.clone()));
                // Depth = furthest along the spine, NOT most recent: a failed run loops back to
                // `bootflow:start`, which would otherwise erase how far it actually got.
                if let Some(r) = spine_rank(state) {
                    let better = match &s.deepest_state {
                        Some(cur) => spine_rank(cur).map_or(true, |c| r > c),
                        None => true,
                    };
                    if better {
                        s.deepest_state = Some(state.clone());
                    }
                }
                // Any error reason is terminal information, whichever state carries it.
                if reason.starts_with("error.") {
                    s.terminal_reason = Some(reason.clone());
                }
            }
            Event::GuestHalt { halt, information } => {
                s.guest_halt = Some(format!("{halt}: {information}"));
            }
        }
    }
    s
}

/// A single difference between two runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub category: String,
    pub item: String,
    /// `"added"`, `"removed"`, or `"changed"`.
    pub change: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Diff two run summaries. Output order is deterministic.
pub fn diff(a: &RunSummary, b: &RunSummary) -> Vec<DiffEntry> {
    let mut out = Vec::new();

    fn set_diff(
        out: &mut Vec<DiffEntry>,
        category: &str,
        a: &BTreeSet<String>,
        b: &BTreeSet<String>,
    ) {
        for item in b.difference(a) {
            out.push(DiffEntry {
                category: category.into(),
                item: item.clone(),
                change: "added".into(),
                before: None,
                after: None,
            });
        }
        for item in a.difference(b) {
            out.push(DiffEntry {
                category: category.into(),
                item: item.clone(),
                change: "removed".into(),
                before: None,
                after: None,
            });
        }
    }

    set_diff(&mut out, "hostname", &a.hostnames, &b.hostnames);
    set_diff(&mut out, "http", &a.http_calls, &b.http_calls);
    set_diff(&mut out, "fault", &a.faults, &b.faults);
    set_diff(&mut out, "milestone", &a.milestones, &b.milestones);

    // Gate depth: the single most important diff line. Reported explicitly rather than left for the
    // reader to infer from a set difference.
    if a.deepest_state != b.deepest_state {
        out.push(DiffEntry {
            category: "gate-depth".into(),
            item: "deepest bootflow state".into(),
            change: "changed".into(),
            before: a.deepest_state.clone(),
            after: b.deepest_state.clone(),
        });
    }
    if a.terminal_reason != b.terminal_reason {
        out.push(DiffEntry {
            category: "gate-depth".into(),
            item: "terminal reason".into(),
            change: "changed".into(),
            before: a.terminal_reason.clone(),
            after: b.terminal_reason.clone(),
        });
    }
    if a.guest_halt != b.guest_halt {
        out.push(DiffEntry {
            category: "gate-depth".into(),
            item: "guest halt".into(),
            change: "changed".into(),
            before: a.guest_halt.clone(),
            after: b.guest_halt.clone(),
        });
    }
    {
        let av: BTreeSet<String> = a.boot_ladder.iter().map(|(s, _)| s.clone()).collect();
        let bv: BTreeSet<String> = b.boot_ladder.iter().map(|(s, _)| s.clone()).collect();
        set_diff(&mut out, "boot-state", &av, &bv);
    }
    set_diff(&mut out, "guest-line", &a.guest_lines, &b.guest_lines);

    // Counts that moved.
    let keys: BTreeSet<_> = a.counts.keys().chain(b.counts.keys()).cloned().collect();
    for k in keys {
        let x = a.counts.get(&k).copied().unwrap_or(0);
        let y = b.counts.get(&k).copied().unwrap_or(0);
        if x != y {
            out.push(DiffEntry {
                category: "count".into(),
                item: k,
                change: "changed".into(),
                before: Some(x.to_string()),
                after: Some(y.to_string()),
            });
        }
    }

    if a.signed_out_polls != b.signed_out_polls {
        out.push(DiffEntry {
            category: "np-identity".into(),
            item: "signed_out_polls".into(),
            change: "changed".into(),
            before: Some(a.signed_out_polls.to_string()),
            after: Some(b.signed_out_polls.to_string()),
        });
    }
    out
}

/// Find run directories under a root, newest name last (names are timestamp-prefixed).
pub fn list_runs(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMU: &str = "\
[Loader] <Info> (shadPS4:Main) emulator.cpp:338 Run: Game id: CUSA00219 Title: Destiny
[Loader] <Info> (shadPS4:Main) emulator.cpp:339 Run: Fw: 0x4010000 App Version: 01.29
[Lib.Net] <Info> (Thread26) net.cpp:1446 sceNetResolverStartNtoa: called, resolverid = 13, hostname = stun1.signon.deadorbit.net, timeout = 1000000
[Lib.NpManager] <Info> (job #0/4) np_manager.cpp:631 sceNpGetOnlineId: SIGNED_OUT (user_id=1000)
[Lib.VideoOut] <Info> (TIME_CONTROL) video_out.cpp:300 sceVideoOutOpen: called
[Tty] <Info> (job #0/4) logger.cpp:62 log_flush: [stdout] networking:signon: Entering signon queue
[Debug] <Critical> (Graphics Heartbeat) signals.cpp:114 SignalHandler: Unreachable code!
Unhandled access violation at code address 0x8000faaf4: Read from address 0x8
";

    #[test]
    fn extracts_the_attempt_ledger_from_an_emulator_log() {
        let s = summarize("t".into(), Provenance::default(), &parse_emulator_log(EMU));
        assert_eq!(s.game_identity.as_deref(), Some("CUSA00219 01.29"));
        assert!(s.hostnames.contains("stun1.signon.deadorbit.net"));
        assert_eq!(s.signed_out_polls, 1);
        assert!(s.milestones.contains("sceVideoOutOpen"));
        assert_eq!(s.guest_subsystems.get("networking:signon").copied(), Some(1));
        assert_eq!(s.faults.len(), 1);
        assert!(s.faults.iter().next().unwrap().starts_with("0x8000faaf4"));
    }

    #[test]
    fn summaries_are_deterministic() {
        // Same input, shuffled event order, must produce an identical summary - otherwise a diff
        // reports noise and the tool is worthless.
        let mut ev = parse_emulator_log(EMU);
        let a = summarize("t".into(), Provenance::default(), &ev);
        ev.reverse();
        let b = summarize("t".into(), Provenance::default(), &ev);
        assert_eq!(a, b);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn diff_reports_the_gate_opening() {
        let before = summarize(
            "before".into(),
            Provenance::default(),
            &parse_emulator_log(
                "[Lib.NpManager] sceNpGetOnlineId: SIGNED_OUT (user_id=1000)\n",
            ),
        );
        let after = summarize(
            "after".into(),
            Provenance::default(),
            &parse_emulator_log(
                "[Lib.Net] sceNetResolverStartNtoa: hostname = stun1.signon.deadorbit.net, x\n",
            ),
        );
        let d = diff(&before, &after);
        assert!(d.iter().any(|e| e.category == "hostname" && e.change == "added"));
        assert!(d
            .iter()
            .any(|e| e.item == "signed_out_polls" && e.before.as_deref() == Some("1")));
    }

    #[test]
    fn shadnet_command_is_unserved_if_ever_refused() {
        let ev = parse_shadnet_log(
            "INFO --> request cmd=Login packet_id=1\n\
             WARN ** NEW GATE ** cmd=CreateRoom unimplemented\n",
        );
        let s = summarize("t".into(), Provenance::default(), &ev);
        assert_eq!(s.shadnet.get("Login"), Some(&true));
        assert_eq!(s.shadnet.get("CreateRoom"), Some(&false));
    }

    #[test]
    fn host_conditions_gate_the_evidence_class() {
        let mut p = Provenance::default();
        assert!(!p.has_host_conditions());
        assert_eq!(p.quiet(&QuietThresholds::default()), Quiet::Unknown);
        p.info.insert("psi-cpu-pre".into(), "avg10=0.00".into());
        assert!(p.has_host_conditions());
    }

    #[test]
    fn quietness_is_a_predicate_against_predeclared_thresholds() {
        let t = QuietThresholds::default();

        // The real stun-clean run: cpu quiet, io high but within our deliberately loose io limit.
        let mut ok = Provenance::default();
        ok.info.insert("psi-cpu-pre".into(), "avg10=0.00 avg60=0.00 avg300=0.00".into());
        ok.info.insert("psi-io-pre".into(), "avg10=82.58 avg60=82.99 avg300=82.98".into());
        ok.info.insert("loadavg-pre".into(), "0.85 0.51 0.50".into());
        assert_eq!(ok.quiet(&t), Quiet::Yes);

        // The shape of the incident that motivated all of this: CPU pressure from a core dump being
        // compressed. Output can look perfectly clean and the run still must not carry a [C].
        let mut busy = Provenance::default();
        busy.info.insert("psi-cpu-pre".into(), "avg10=61.20 avg60=40.00 avg300=20.00".into());
        busy.info.insert("loadavg-pre".into(), "22.00 18.00 9.00".into());
        match busy.quiet(&t) {
            Quiet::No(reasons) => {
                assert!(reasons.iter().any(|r| r.starts_with("cpu")));
                assert!(reasons.iter().any(|r| r.starts_with("loadavg-1")));
            }
            other => panic!("expected No, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod ladder_tests {
    use super::*;

    /// The regression this guards: a failed run ends by re-entering `bootflow:start`, and reporting
    /// depth as "most recent state" made a run that reached `content_check` look like it reached
    /// `start`. Depth must be the furthest point on the spine.
    #[test]
    fn depth_is_furthest_not_latest() {
        let events = vec![
            Event::BootState { state: "bootflow:start".into(), reason: "normal_flow".into() },
            Event::BootState { state: "bootflow:account_signin".into(), reason: "normal_flow".into() },
            Event::BootState { state: "bootflow:content_check".into(), reason: "normal_flow".into() },
            Event::BootState { state: "cleanup".into(), reason: "error.content_manifest_not_provided".into() },
            Event::BootState { state: "bootflow:start".into(), reason: "normal_flow".into() },
        ];
        let s = summarize("t".into(), Provenance::default(), &events);
        assert_eq!(s.deepest_state.as_deref(), Some("bootflow:content_check"));
        assert_eq!(s.terminal_reason.as_deref(), Some("error.content_manifest_not_provided"));
        assert_eq!(s.boot_ladder.len(), 5, "the ladder keeps every transition, including the loop");
    }

    /// A conditional branch must not be mistaken for spine progress.
    #[test]
    fn conditional_states_do_not_define_depth() {
        let events = vec![
            Event::BootState { state: "bootflow:bap_signin".into(), reason: "normal_flow".into() },
            Event::BootState { state: "bootflow:eula".into(), reason: "normal_flow".into() },
        ];
        let s = summarize("t".into(), Provenance::default(), &events);
        assert_eq!(s.deepest_state.as_deref(), Some("bootflow:bap_signin"));
    }

    /// The engine's own halt prose is the cheapest cause-of-death signal we have; parse it exactly.
    #[test]
    fn guest_halt_is_parsed_from_crash_report() {
        let txt = "stack:\n000000000094702F <unknown>\nhalt:\n  RELEASE_VHALT\nhalt information:\n  failed to load bootstrap_patchable\nfatal error detected, current status was:\n    state:: bootflow:bap_signin\n";
        let ev = parse_guest_crash_report(txt);
        let s = summarize("t".into(), Provenance::default(), &ev);
        assert_eq!(s.guest_halt.as_deref(), Some("RELEASE_VHALT: failed to load bootstrap_patchable"));
    }
}
