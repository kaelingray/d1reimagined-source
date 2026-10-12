// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ **THE `present` CHOKEPOINT.** One tick of the world, immutable, and the only thing any
//! frontend is ever handed. `d1-scope frame --json` serialises exactly this; BUILD 2's egui window
//! will draw exactly this. **If it is not in here, no frontend may know it.**
//!
//! ⛔ This module depends on nothing else in the crate and knows no egui type. That is the property
//! that keeps the CLI and the UI from drifting into two different tools.

use serde::Serialize;

use crate::mem_source::SourceId;

#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub seq: u64,
    pub ts_unix_ms: u64,
    pub ts_mono_ms: u64,
    pub tick_ms: f32,
    pub source: SourceId,
    /// [`crate::mem_source::MemSource::generation`] when this frame was built. A resolution stamped
    /// with an older one is **discarded**, never reused.
    pub generation: u64,
    pub surfaces: Vec<SurfaceView>,
    pub edges: Vec<EdgeView>,
    /// ⚠ Events arrive in **batches at child exit**, not streamed - `d1-live --json` writes its
    /// report once, on the way out. The lease is therefore also the event latency, and that is a
    /// property to state rather than to hide behind a spinner.
    pub events: Vec<EventView>,
    pub events_dropped: u64,
    pub child: ChildStatus,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Note {
    pub level: String, // "info" | "warn" | "loud"
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SurfaceView {
    pub name: String,
    pub group: String,
    pub addr_expr: String,
    pub resolved: Resolved,
    /// Every hop, from `d1_live::addr`. A resolution nobody saw is a resolution nobody checked.
    pub resolve_trace: Vec<String>,
    pub liveness: LivenessView,
    pub fields: Vec<FieldView>,
    pub confidence: String,
    pub reach: String,
}

#[derive(Clone, Debug, Serialize)]
pub enum Resolved {
    Addr(u64),
    /// ⛔ NOT zero. The region was not captured / is not mapped.
    Unmapped { addr: u64 },
    Unresolvable { why: String },
    NotExpressible { resolver: String },
    /// registry `addr == "[U]"` - see the matching `[[open]]` entry.
    Unknown,
}

impl Resolved {
    /// ⭐ The display string is built HERE, in the `present` chokepoint, not in a frontend - and it
    /// prints addresses in **hex**. A decimal address in a tool whose whole subject is an address
    /// language is a rendering nobody can check against a doc, a log line or `d1-live --addr`.
    pub fn render(&self) -> String {
        match self {
            Resolved::Addr(a) => format!("a:{a:#x}"),
            Resolved::Unmapped { addr } => format!("UNMAPPED @ {addr:#x}"),
            Resolved::Unresolvable { why } => format!("UNRESOLVABLE ({why})"),
            Resolved::NotExpressible { resolver } => format!("NOT-EXPRESSIBLE (resolver {resolver})"),
            Resolved::Unknown => "[U] - see the [[open]] entry".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldView {
    pub name: String,
    pub off: u64,
    pub ty: String,
    pub value: Value,
    pub note: Option<String>,
}

/// ⛔ `Unmapped`, `NotMeasurable` and `NotRead` are NOT `U32(0)`. *"Not captured"*, *"liveness said
/// no"* and *"was zero"* are three different facts and only one of them is evidence.
#[derive(Clone, Debug, Serialize)]
pub enum Value {
    Int(i128),
    Ints(Vec<i128>),
    Floats(Vec<f64>),
    Ptr(u64),
    Ascii(String),
    Bytes(Vec<u8>),
    Unmapped,
    NotMeasurable { liveness: String },
    NotRead { why: String },
}

impl Value {
    /// ⭐ The display string, built HERE in the `present` chokepoint - so the table, the JSON dump
    /// and any future frontend cannot disagree about what a value *reads as*.
    ///
    /// Three rules that are not cosmetic:
    /// * **A null pointer renders `NULL (0x0)`, not blank.** It was read, and the answer was zero.
    ///   Blank is what a cell looks like when nothing happened, and that is a different fact.
    /// * **A string renders quoted.** An empty string and an unread cell are otherwise identical.
    /// * **Every non-reading names itself and its reason.** `UNMAPPED`, `NOT-MEASURABLE (…)`,
    ///   `NOT-READ (…)` - never `0`, and never the same word for two different reasons.
    pub fn render(&self) -> String {
        fn n(v: i128) -> String {
            if (0..=9).contains(&v) {
                v.to_string()
            } else if v < 0 {
                format!("{v} ({:#x})", v as i64)
            } else {
                format!("{v} ({v:#x})")
            }
        }
        match self {
            Value::Int(v) => n(*v),
            Value::Ints(v) => {
                let head: Vec<String> = v.iter().take(8).map(|x| n(*x)).collect();
                format!("[{}{}]", head.join(", "), if v.len() > 8 { format!(", … ({} total)", v.len()) } else { String::new() })
            }
            Value::Floats(v) => {
                let head: Vec<String> = v.iter().take(8).map(|x| format!("{x}")).collect();
                format!("[{}{}]", head.join(", "), if v.len() > 8 { format!(", … ({} total)", v.len()) } else { String::new() })
            }
            Value::Ptr(0) => "NULL (0x0)".to_string(),
            Value::Ptr(p) => format!("a:{p:#x}"),
            Value::Ascii(s) => format!("{s:?}"),
            Value::Bytes(b) => {
                let head: Vec<String> = b.iter().take(16).map(|x| format!("{x:02x}")).collect();
                format!("{}{}", head.join(" "), if b.len() > 16 { format!(" … ({} bytes)", b.len()) } else { String::new() })
            }
            Value::Unmapped => "UNMAPPED".to_string(),
            Value::NotMeasurable { liveness } => format!("NOT-MEASURABLE (liveness: {liveness})"),
            Value::NotRead { why } => format!("NOT-READ ({why})"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LivenessView {
    pub expr: String,
    pub verdict: Verdict,
    pub why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Live,
    NotLive,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct EdgeView {
    pub id: String,
    pub up: String,
    pub down: String,
    pub mech: String,
    pub state: EdgeState,
    pub why: String,
    pub guard: Option<String>,
    pub guard_active: bool,
    pub evaluable: bool,
    pub as_of: String,
}

/// ⛔ FIVE STATES. Collapsing any two mis-accuses an edge. `Grey` = upstream not populated ⇒ never
/// exercised (untested, **not** broken). `Retired` = dead by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum EdgeState {
    Green,
    Red,
    Grey,
    Unknown,
    Retired,
}

impl EdgeState {
    pub fn parse(s: &str) -> Option<EdgeState> {
        Some(match s.trim() {
            "green" => EdgeState::Green,
            "red" => EdgeState::Red,
            "grey" => EdgeState::Grey,
            "unknown" => EdgeState::Unknown,
            "retired" => EdgeState::Retired,
            _ => return None,
        })
    }
}

/// ⛔ `rip` and `writer` are TWO FIELDS and are never collapsed into one.
///
/// A data breakpoint is a **trap**: the reported `rip` is the instruction *after* the store. A view
/// that carried a single "address" would name the wrong instruction - and, when the store ends a
/// basic block, the wrong basic block - while looking entirely correct. Control #7 asserts both are
/// present and **distinct** on a recorded fixture.
#[derive(Clone, Debug, Serialize)]
pub struct EventView {
    pub seq: u64,
    pub elapsed_ms: u64,
    /// Derived from the ADDRESS against this tick's resolved surfaces - *not* from what the slot
    /// claimed. See `slot_surface` / `disagreement`.
    pub surface: String,
    pub field: Option<String>,
    pub how: String,
    /// What the armed slot said it was watching. Present so the two derivations can disagree.
    pub slot_surface: Option<String>,
    /// ⭐ Set when the two derivations disagree. Two instruments disagreeing is a finding.
    pub disagreement: Option<String>,
    pub addr: u64,
    pub tid: i32,
    /// ⚠ The RAW register: the instruction AFTER the store. Never quote this as the writer.
    pub rip: u64,
    pub rip_vaddr: Option<u64>,
    /// The decoded store. `None` ⇒ the backward decode failed; there is no writer to name.
    pub writer: Option<u64>,
    pub writer_vaddr: Option<u64>,
    pub writer_text: String,
    pub writer_confidence: String,
    /// ⭐ `false` ⇒ `AMBIGUOUS`/`no-decode`: a `[U]` for `d1-eboot disasm`, not a finding.
    pub writer_quotable: bool,
    pub value_after: Option<String>,
}

/// ⭐ **The receipt for a stop.** The one fact that matters is `confirmed_disarm`, and it is a
/// *positive* statement assembled from three independent things: the child exited on its own clean
/// path (not by a signal), it wrote a complete report on the way out, and it did not complain about
/// a thread it could not prove disarmed. "The process is gone" is NOT that statement - a child that
/// died of a signal is gone too, and it takes its target with it.
#[derive(Clone, Debug, Serialize)]
pub struct StopReceipt {
    pub child_pid: i32,
    pub target_pid: i32,
    /// Why we stopped it: `"re-arm"`, `"drop-guard"`, `"lease-renewal"`, `"release-tracer"`, …
    pub how: String,
    pub exit_code: Option<i32>,
    /// ⛔ `Some(_)` means the child died of a signal - its disarm path did NOT run to completion.
    pub exit_signal: Option<i32>,
    pub waited_ms: u128,
    pub confirmed_disarm: bool,
    /// The child's own `stop_reason` - `"interrupted"` when it left through the term handler.
    pub stop_reason: Option<String>,
    /// Anything the child said about a thread it could not prove disarmed. Non-empty is loud.
    pub complaints: Vec<String>,
    pub hits: usize,
}

/// The armed watch, as the frame shows it.
#[derive(Clone, Debug, Serialize)]
pub struct WatchSpecView {
    pub surface: Option<String>,
    pub addr_expr: String,
    /// The absolute the CHILD reported on its own `ARMED` line. `0` = it has not announced one.
    pub resolved: u64,
    pub len: u64,
    pub rw: String,
    pub slot: usize,
    pub count: usize,
    pub lease_s: f64,
}

/// ⛔⛔ **`NoHit` IS A RESULT, NOT A FAILURE, AND THAT DISTINCTION IS THE WHOLE POINT.**
///
/// A parseable document with `hits: []` is a real negative - *"nothing wrote this address, on the
/// threads that were armed, for the window that was watched."* Filing it under `Failed` would throw
/// away the one shape of evidence this instrument exists to produce, and would make a working watch
/// look broken. `Failed` means the document did not parse; the two exit `1` alike, which is exactly
/// why classification is on **stdout parseability and never on the exit code**.
#[derive(Clone, Debug, Serialize)]
pub enum ChildStatus {
    /// No child has ever been launched this session.
    None,
    Running {
        pid: i32,
        target_pid: i32,
        spec: WatchSpecView,
        lease_s: f64,
        lease_remaining_s: f64,
        armed_threads: u32,
        rearms: u64,
        hits: usize,
        /// ⚠ Slots we hold and are NOT arming. One child arms one address.
        pending: Vec<String>,
    },
    Finished { spec: WatchSpecView, hits: u32, stop_reason: String, receipt: StopReceipt },
    /// ⭐ A parseable document with zero hits. **A real negative result**, and `why` carries the
    /// honest reading of it verbatim.
    NoHit { spec: WatchSpecView, watched_s: f64, stop_reason: String, why: String, receipt: StopReceipt },
    /// The child's stdout did not parse. `stderr` verbatim - never summarised.
    Failed { spec: WatchSpecView, stderr: String, receipt: StopReceipt },
    /// ⛔ LOUD AND PERSISTENT: `SIGTERM` was sent and the child never confirmed exit. No further
    /// spawn is permitted until it is reaped. ⚠ We do **not** escalate: the child's own lease is
    /// the escalation, because the hard kill is what would take the target down.
    StopTimedOut { pid: i32, waited_s: f64, receipt: StopReceipt },
    /// ⭐ We declined, and said why. A refusal is a MESSAGE, not a fight and not an error code.
    Refused { why: String },
}
