// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The unified event model shared by ingest, query and the GUI.
//!
//! Every event carries a `stream`, a unified-timeline `order` position, and - crucially - the
//! `confidence` of that position, because the four streams do NOT share a wall clock (measured; see
//! `historicaldocs/design/trace-viewer.md` §1). The viewer never lets a bracketed/approximate placement look like a
//! proven one.

use std::collections::BTreeMap;

use serde::Serialize;

/// Which of the four evidence streams an event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Stream {
    /// HTTP request/response on the responder surface.
    Http,
    /// A BAP inner message (from the responder's already-decoded log lines).
    Bap,
    /// A UD2 probe record from `account-trace.bin`.
    Probe,
    /// A guest diagnostic line from `emulator.log`.
    Guest,
}

impl Stream {
    pub fn tag(self) -> &'static str {
        match self {
            Stream::Http => "HTTP",
            Stream::Bap => "BAP",
            Stream::Probe => "PROBE",
            Stream::Guest => "GUEST",
        }
    }

    pub fn parse(s: &str) -> Option<Stream> {
        match s.to_ascii_lowercase().as_str() {
            "http" => Some(Stream::Http),
            "bap" => Some(Stream::Bap),
            "probe" => Some(Stream::Probe),
            "guest" => Some(Stream::Guest),
            _ => None,
        }
    }
}

/// How trustworthy an event's position on the unified timeline is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Confidence {
    /// A real order key: probe `sequence`, or a BAP frame anchored to a guest rx probe by
    /// `(msg_type, sequence)`.
    Exact,
    /// Placed between two exactly-ordered neighbours in the SAME log file (true bracketing).
    ///
    /// ⚠ F1: this is asserted ONLY when the event genuinely sits between two anchors. An event
    /// before the first anchor or after the last one is EXTRAPOLATED, not bracketed, and is
    /// [`Confidence::Approx`] - asserting `Bracketed` there produced a causally inverted timeline
    /// (`HTTP ~ 4.90 POST /SignOn` placed before the BAP traffic the SignOn response bootstraps).
    Bracketed,
    /// No numeric join to the spine exists; placed by best-effort linear mapping or extrapolated
    /// past the outermost anchor. Labelled as such; never cite a cross-stream order from it.
    Approx,
}

impl Confidence {
    pub fn marker(self) -> &'static str {
        match self {
            Confidence::Exact => "=",
            Confidence::Bracketed => "~",
            Confidence::Approx => "≈",
        }
    }
}

/// A decoded UD2 probe record, in the shape `capture-rig/decode-account-trace.py --json` emitted.
/// Since the native port, the records are produced by [`crate::probe_decode`] in this crate - the
/// shipped binary needs no Python at runtime.
///
/// ⚠ Correction (historicaldocs/design/trace-viewer.md §2): the 232-byte layout is **not**
/// described once. Besides `probe_decode.rs`, the same `struct.Struct("<IHHII15Q48s48s")` lives in
/// `capture-rig/decode-account-trace.py:16`, `ready-byte-report.py:30` and `gate-check.py:33`, and
/// that duplication has already cost a wrong answer (`ready-byte-report.py:60-64`). The Rust
/// decoder is now the one a shipped binary depends on; the Python copies are prototyping/live-rig
/// tools pending archival.
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct ProbeRecord {
    pub sequence: u64,
    pub probe_id: u32,
    pub probe: String,
    pub registers: BTreeMap<String, u64>,
    pub sample_a: String,
    pub sample_b: String,
    #[serde(default)]
    pub explain: Vec<String>,
}

/// Parsed identity of a BAP inner message, used both for display and for anchoring.
///
/// `msg_type`/`sequence` are the REQUEST the responder received. `reply_type`/`reply_seq` are the
/// response it sent. The guest's rx probes record the RESPONSE they were handed, so the reply key is
/// the one that actually joins to the probe spine (historicaldocs/design/trace-viewer.md §1).
#[derive(Debug, Clone, Default)]
pub struct BapInfo {
    pub msg_type: Option<u16>,
    pub sequence: Option<u32>,
    pub reply_type: Option<u16>,
    pub reply_seq: Option<u32>,
    pub direction: BapDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BapDir {
    #[default]
    Recv,
    Send,
}

/// One event on the unified timeline.
#[derive(Debug, Clone)]
pub struct Event {
    /// Index into the fully-ordered event vector (assigned after the global sort).
    pub id: usize,
    pub stream: Stream,
    /// Unified-timeline position. The spine is the probe `sequence`.
    pub order: f64,
    pub confidence: Confidence,
    /// Native order key within the stream (probe sequence, or source-file line number).
    pub ordinal: u64,
    pub summary: String,
    pub detail: Vec<String>,
    pub probe: Option<ProbeRecord>,
    pub bap: Option<BapInfo>,
}

impl Event {
    /// Case-insensitive substring test over the summary AND every detail line.
    ///
    /// ⚠ F7: searching the summary alone silently missed `world_tag` (681 probe records - the
    /// design doc's own flagship cross-stream example), `Bearer`, `XETROV`, `manifest_url` and
    /// `key-only`, all of which live only in detail. `needle` must already be lowercase.
    pub fn contains_text(&self, needle: &str) -> bool {
        if self.summary.to_ascii_lowercase().contains(needle) {
            return true;
        }
        self.detail
            .iter()
            .any(|d| d.to_ascii_lowercase().contains(needle))
    }

    /// Field lookup for query predicates. Only probe events carry register/seq/probe_id fields.
    pub fn field(&self, name: &str) -> Option<u64> {
        let probe = self.probe.as_ref()?;
        match name {
            "seq" | "sequence" => Some(probe.sequence),
            "probe_id" => Some(probe.probe_id as u64),
            other => probe.registers.get(other).copied(),
        }
    }
}

/// Run-level facts surfaced in the header and the info panel.
#[derive(Debug, Clone, Default)]
pub struct RunSummary {
    pub run_id: String,
    pub http: usize,
    pub bap: usize,
    pub probe: usize,
    pub guest: usize,
    /// Distinct probe names seen (for the probe legend / autocomplete hint).
    pub probe_names: Vec<String>,
    /// Furthest `bootflow:` state entered, if any.
    pub bootflow_reached: Option<String>,
    /// Count of guest `error.*` lines.
    pub error_count: usize,
    /// Number of exactly-anchored BAP frames.
    pub bap_anchored: usize,
    /// BAP rows that CARRY an id `(msg_type, sequence)` and so could in principle anchor.
    ///
    /// ⚠ F6: the old denominator was every BAP row, but ~half of them are `BAP TCP msg#N` outer
    /// headers with no type/seq - unanchorable by construction, capping the reported rate at 50%.
    /// Those headers are now folded into the inner-decode row they belong to, and the rate is
    /// reported over anchorable rows.
    pub bap_anchorable: usize,
    /// True when a probe spine AND at least one cross-stream anchor exist, i.e. when a unified
    /// cross-stream order is *observed* rather than invented (F2).
    pub spine: bool,
    /// Human notes gathered during ingest (RUN-INFO.txt head, blob summary, warnings).
    pub notes: Vec<String>,
}

/// The whole loaded run: ordered events + summary.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub events: Vec<Event>,
    pub summary: RunSummary,
}
