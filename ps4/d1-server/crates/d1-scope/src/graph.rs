// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐⭐ **THE FLOW GRAPH, AS DATA.** Nodes, edges, the fixed layout, the derived colour and the
//! event badge - all of it computed here, from a [`Frame`], the surface registry and
//! `capture-rig/surfaces/graph.toml`, in a module that knows **no egui type**.
//!
//! ⛔ This is [`crate::frame`]'s chokepoint extended, exactly as [`crate::table`] is: the window
//! and `d1-scope graph --headless [--json]` render the **same** [`GraphModel`]. The window picks
//! colours and draws rectangles whose coordinates were computed *here*; it computes nothing.
//!
//! # ⛔⛔ FIVE COLOURS, AND COLLAPSING ANY TWO MIS-ACCUSES AN EDGE
//!
//! | colour | meaning | ⛔ what it is NOT |
//! |---|---|---|
//! | **green** | upstream populated, downstream populated | - |
//! | **red** | upstream populated, downstream **not** - ⭐ the only colour that accuses | - |
//! | **grey** | upstream **not** populated ⇒ never exercised - **untested, not broken** | *"it failed"* |
//! | **unknown** | a witness could not be read: uncaptured region, `[U]` address, poked field, or no witness at all | *"it is fine"*, and *"it is broken"* |
//! | **retired** | dead **by construction** - must never be drawn red again | a bug |
//!
//! ⭐ **GREY IS TESTED BEFORE EVERYTHING EXCEPT `retired` AND `poked`, AND THAT ORDER IS THE
//! POINT.** Downstream of a break, the next edges' upstream arm is *false* - they have never run.
//! Painting them red charges three edges with a failure exactly one of them committed, and E-18 in
//! particular is a documented dead end that two still-current documents call "the break".
//!
//! # ⚠ THE DERIVED COLOUR IS NOT THE REGISTRY'S `state`
//!
//! `registry.toml`'s `state` is **declared**, as of a date, by a human pass; the colour here is
//! **derived from the state plane, this tick**. Both travel on every edge, and a disagreement is
//! reported rather than resolved. Two instruments disagreeing is a finding - and the derived one is
//! not automatically right, it is the one whose reasoning you can read on screen.
//!
//! # ⛔ A POKED WITNESS IS NEVER GREEN
//!
//! A field **we wrote** is not a measurement of the client. `registry.toml`'s
//! `meta.snapshot.poked_fields` names them; a node whose witness is one of them renders `POKED`,
//! every edge touching it is `unknown`, and the model emits a loud note naming the pristine
//! snapshots where the same edge can be read honestly. *A poke identifies a gate; it never
//! satisfies one, and a graph that went green on one would be satisfying our own mock.*

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::frame::{EdgeState, EventView, Frame, Resolved, SurfaceView, Value};
use crate::registry::Registry;
use crate::table::{badge_for, EventBadge, TableParams};

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// graph.toml
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpecMeta {
    pub version: String,
    pub generated: String,
    pub generator: String,
    pub registry: String,
    pub source: String,
    pub columns: Vec<String>,
    pub witness_rules: Vec<String>,
}

/// What kind of thing a node is - and every one of these is a different answer to *"can the state
/// plane read it?"*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// A registry surface: readable, in principle.
    Surface,
    /// ⛔ One of OURS. d1-scope reads the client, not `d1-responder`/`d1-activity-host`.
    Server,
    /// A `[U]:…` endpoint: nobody knows where it lives.
    Unknown,
    /// A FUNCTION. There is no state to read; proving it ran is the EVENT plane's job.
    Code,
}

impl Default for NodeKind {
    fn default() -> Self {
        NodeKind::Surface
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeSpec {
    pub id: String,
    #[serde(default)]
    pub kind: NodeKind,
    /// Optional for a surface - it defaults to that surface's registry `group`.
    #[serde(default)]
    pub column: Option<String>,
    pub label: String,
    /// ⭐ The ONE field that decides *populated*. Absent ⇒ `NO-WITNESS`, on purpose.
    #[serde(default)]
    pub witness: Option<String>,
    #[serde(default)]
    pub witness_rule: Option<String>,
    pub why: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GraphSpec {
    pub meta: SpecMeta,
    #[serde(default, rename = "node")]
    pub nodes: Vec<NodeSpec>,
    #[serde(skip)]
    pub path: PathBuf,
}

pub fn default_spec_path() -> PathBuf {
    if let Ok(p) = std::env::var("D1_SCOPE_GRAPH") {
        return PathBuf::from(p);
    }
    crate::repo_root().join("capture-rig/surfaces/graph.toml")
}

impl GraphSpec {
    pub fn open_default() -> Result<Self> {
        Self::open(default_spec_path())
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading the graph spec {}", path.display()))?;
        let mut s: GraphSpec =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        s.path = path;
        Ok(s)
    }

    pub fn node(&self, id: &str) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// ⛔ **THE TWO FILES MAY NOT DRIFT.** Every complaint here is a hard failure of control #16,
    /// because each one renders as a permanent, plausible UNKNOWN that nobody would investigate:
    /// a dropped edge simply is not drawn, and a witness naming a renamed field simply never reads.
    pub fn consistency(&self, reg: &Registry) -> Vec<String> {
        let mut out = Vec::new();

        // 1. every edge endpoint has a node
        for e in &reg.edges {
            for (which, ep) in [("up", &e.up), ("down", &e.down)] {
                if self.node(ep).is_none() {
                    out.push(format!(
                        "{} {which} = {ep:?} has NO node in graph.toml - the edge would be silently \
                         DROPPED from the drawing",
                        e.id
                    ));
                }
            }
        }
        // 2. every node is an endpoint of some edge (an orphan node is a lie about the flow)
        for n in &self.nodes {
            let used = reg.edges.iter().any(|e| e.up == n.id || e.down == n.id);
            if !used {
                out.push(format!("node {:?} is not an endpoint of any edge - orphan", n.id));
            }
        }
        // 3. a `surface` node names a real surface; its witness names a real FIELD of it
        for n in &self.nodes {
            match n.kind {
                NodeKind::Surface | NodeKind::Code => {
                    let Some(s) = reg.surface(&n.id) else {
                        out.push(format!("node {:?} is kind={:?} but registry.toml has no such surface", n.id, n.kind));
                        continue;
                    };
                    if let Some(w) = &n.witness {
                        if !s.fields.iter().any(|f| f.name == *w) {
                            out.push(format!(
                                "node {:?} witness {w:?} is not a field of that surface - it would \
                                 read UNKNOWN forever. Fields: {:?}",
                                n.id,
                                s.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>()
                            ));
                        }
                        match &n.witness_rule {
                            None => out.push(format!("node {:?} has a witness but no witness_rule", n.id)),
                            Some(r) if !self.meta.witness_rules.contains(r) => out.push(format!(
                                "node {:?} witness_rule {r:?} is not in the closed vocabulary {:?}",
                                n.id, self.meta.witness_rules
                            )),
                            Some(_) => {}
                        }
                    }
                    if let Some(c) = &n.column {
                        if !self.meta.columns.contains(c) {
                            out.push(format!("node {:?} column {c:?} is not one of {:?}", n.id, self.meta.columns));
                        }
                    }
                }
                NodeKind::Server | NodeKind::Unknown => {
                    if n.witness.is_some() {
                        out.push(format!(
                            "node {:?} is kind={:?} - NOT readable by the state plane - yet declares \
                             a witness. That would claim a measurement nobody made.",
                            n.id, n.kind
                        ));
                    }
                    let want = if n.kind == NodeKind::Server { "server:" } else { "[U]:" };
                    if !n.id.starts_with(want) {
                        out.push(format!("node {:?} is kind={:?} but its id does not start with {want:?}", n.id, n.kind));
                    }
                }
            }
        }
        // 4. ids are unique
        let mut ids: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        ids.sort_unstable();
        let n0 = ids.len();
        ids.dedup();
        if ids.len() != n0 {
            out.push(format!("graph.toml declares {} nodes but only {} distinct ids", n0, ids.len()));
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The witness
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// What a witness field READ. ⛔ `Unmapped`, `NotLive` and `NotRead` are three different facts and
/// none of them is `Num(0)`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum WitnessRead {
    Num(i128),
    Unmapped,
    NotLive { why: String },
    NotRead { why: String },
    /// The field decodes to something no rule in the closed vocabulary can compare.
    NotNumeric { ty: String },
}

/// ⭐ ONE PLACE a `Value` becomes a number, and it declines rather than guesses.
pub fn witness_read(v: &Value) -> WitnessRead {
    match v {
        Value::Int(n) => WitnessRead::Num(*n),
        Value::Ptr(p) => WitnessRead::Num(*p as i128),
        // ⚠ An array/blob witness answers "is ANY of it non-zero" - the same question `all(… == 0)`
        //   asks in three of the registry's own red predicates. It reports the first non-zero unit
        //   so the rendered number is a real byte of the object, not a summary nobody can check.
        Value::Ints(a) => WitnessRead::Num(a.iter().copied().find(|x| *x != 0).unwrap_or(0)),
        Value::Bytes(b) => WitnessRead::Num(b.iter().copied().find(|x| *x != 0).unwrap_or(0) as i128),
        Value::Ascii(s) => WitnessRead::Num(if s.is_empty() { 0 } else { 1 }),
        Value::Floats(_) => WitnessRead::NotNumeric { ty: "float".into() },
        Value::Unmapped => WitnessRead::Unmapped,
        Value::NotMeasurable { liveness } => WitnessRead::NotLive { why: liveness.clone() },
        Value::NotRead { why } => WitnessRead::NotRead { why: why.clone() },
    }
}

/// The closed rule vocabulary. `None` ⇒ the rule is not one we know, and the caller abstains.
pub fn rule_holds(rule: &str, v: i128) -> Option<bool> {
    Some(match rule.trim() {
        "!= 0" => v != 0,
        "== 0" => v == 0,
        "!= -1" => v != -1,
        "== 1" => v == 1,
        _ => return None,
    })
}

/// ⛔ SIX STATES. `Empty` is the only one that means *"measured, and the answer is no"*; the other
/// four non-`Populated` ones are statements about our REACH, not about the client.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum NodeState {
    Populated,
    Empty,
    /// ⛔ The witness is a field WE WROTE. Never green, never red - it is not the client's value.
    Poked { field: String, snapshot: String },
    /// UNMAPPED / NOT-LIVE / NOT-READ / `[U]` address.
    Unreadable { why: String },
    /// No witness is declared, or the declared one cannot be compared. A deliberate abstention.
    NoWitness { why: String },
    /// A server, a `[U]:` endpoint, or code: the state plane structurally cannot read it.
    NotMeasurable { why: String },
}

impl NodeState {
    pub fn label(&self) -> &'static str {
        match self {
            NodeState::Populated => "POPULATED",
            NodeState::Empty => "EMPTY",
            NodeState::Poked { .. } => "POKED",
            NodeState::Unreadable { .. } => "UNREADABLE",
            NodeState::NoWitness { .. } => "NO-WITNESS",
            NodeState::NotMeasurable { .. } => "NOT-MEASURABLE",
        }
    }
    pub fn detail(&self) -> String {
        match self {
            NodeState::Populated => "the witness field read non-trivially - a measurement of the client".into(),
            NodeState::Empty => "the witness field was READ and the answer is no. ⭐ This is evidence.".into(),
            NodeState::Poked { field, snapshot } => format!(
                "⛔ {field} is POKED in snapshot {snapshot:?} (registry meta.snapshot.poked_fields). \
                 The value there is OURS. It is not evidence about the client in either direction."
            ),
            NodeState::Unreadable { why } | NodeState::NoWitness { why } | NodeState::NotMeasurable { why } => why.clone(),
        }
    }
    pub const ALL_LABELS: [&'static str; 6] =
        ["POPULATED", "EMPTY", "POKED", "UNREADABLE", "NO-WITNESS", "NOT-MEASURABLE"];
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The derivation
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct Derivation {
    pub color: EdgeState,
    pub why: String,
    /// Things that qualify the colour without changing it. ⚠ Never folded into `why`: a caveat that
    /// is only in prose is a caveat nobody reads.
    pub caveats: Vec<String>,
    /// ⭐ `false` when the upstream arm was ASSERTED rather than measured (a server, a `[U]:`
    /// endpoint). The window draws that half of the arrow dashed.
    pub upstream_measured: bool,
    /// ⛔ Set when a RED was suppressed by the edge's own registry `guard`.
    pub accusation_suppressed: Option<String>,
}

/// ⛔⛔ **THE ONE DERIVATION, AND THE ORDER IS THE ARGUMENT.**
///
/// 1. `retired` wins over everything. Dead by construction is not a reading.
/// 2. a **poked** end is never green and never red - the value is ours.
/// 3. **upstream not populated ⇒ GREY**, before any downstream question is asked. An edge whose
///    upstream never fired was never exercised; whatever its downstream reads is irrelevant, and
///    this is what stops one break from being charged to three edges.
/// 4. an end we could not READ (uncaptured, `[U]`, torn down) ⇒ unknown, with the reason.
/// 5. a downstream we cannot MEASURE (a `[U]:` endpoint, or code) ⇒ the upstream decides, and the
///    edge says so: *"the preconditions hold"* is not *"the callee ran"*.
/// 6. an upstream we cannot MEASURE (our own server) ⇒ the downstream decides, with the caveat.
/// 7. both populated ⇒ green. 8. populated → empty ⇒ **RED**, the only accusation.
///
/// Finally: a RED on an edge carrying a registry `guard` is degraded to `unknown` and the guard
/// text is printed. The guard lives in the DATA; the renderer only obeys it.
pub fn derive(
    declared: EdgeState,
    up: &NodeState,
    down: &NodeState,
    guard: Option<&str>,
) -> Derivation {
    let mut caveats = Vec::new();
    let upstream_measured = !matches!(up, NodeState::NotMeasurable { .. });

    let mk = |color: EdgeState, why: String, caveats: Vec<String>, upstream_measured: bool| Derivation {
        color,
        why,
        caveats,
        upstream_measured,
        accusation_suppressed: None,
    };

    if declared == EdgeState::Retired {
        return mk(
            EdgeState::Retired,
            "RETIRED by construction in registry.toml - this edge must never be drawn red again"
                .into(),
            caveats,
            upstream_measured,
        );
    }
    for (which, s) in [("upstream", up), ("downstream", down)] {
        if let NodeState::Poked { field, snapshot } = s {
            return mk(
                EdgeState::Unknown,
                format!(
                    "⛔ the {which} witness {field} is POKED in {snapshot:?} - the value is OURS. \
                     Re-read this edge against a pristine snapshot."
                ),
                caveats,
                upstream_measured,
            );
        }
    }
    // ⭐ GREY FIRST. See the doc comment: this is the rule that stops one break from accusing three.
    if *up == NodeState::Empty {
        return mk(
            EdgeState::Grey,
            "the UPSTREAM was read and is not populated ⇒ this edge has never been exercised. \
             ⛔ UNTESTED, not broken."
                .into(),
            caveats,
            upstream_measured,
        );
    }
    for (which, s) in [("upstream", up), ("downstream", down)] {
        match s {
            NodeState::Unreadable { why } => {
                return mk(
                    EdgeState::Unknown,
                    format!("the {which} could not be READ: {why}"),
                    caveats,
                    upstream_measured,
                )
            }
            NodeState::NoWitness { why } => {
                return mk(
                    EdgeState::Unknown,
                    format!("the {which} declares NO WITNESS: {why}"),
                    caveats,
                    upstream_measured,
                )
            }
            _ => {}
        }
    }
    // downstream not measurable ⇒ the upstream decides
    if let NodeState::NotMeasurable { why } = down {
        caveats.push(format!(
            "⚠ the DOWNSTREAM is not measurable by the state plane ({why}) - this edge states that \
             the PRECONDITIONS hold, never that the far end ran. Proving that is the EVENT plane's \
             job (`--rw exec`)."
        ));
        return match up {
            NodeState::Populated => mk(EdgeState::Green, "the upstream is populated".into(), caveats, upstream_measured),
            _ => mk(EdgeState::Unknown, "neither end is measurable".into(), caveats, upstream_measured),
        };
    }
    // upstream not measurable ⇒ the downstream decides
    if let NodeState::NotMeasurable { why } = up {
        caveats.push(format!(
            "⚠ the UPSTREAM is ASSERTED, not measured ({why}). A green here says the CLIENT's end \
             is populated; it does NOT say our server is correct."
        ));
        let d = match down {
            NodeState::Populated => mk(EdgeState::Green, "the client's end is populated".into(), caveats.clone(), false),
            NodeState::Empty => mk(
                EdgeState::Red,
                "⛔ the client's end was read and is EMPTY while we assert we sent it".into(),
                caveats.clone(),
                false,
            ),
            _ => mk(EdgeState::Unknown, "neither end is measurable".into(), caveats.clone(), false),
        };
        return guarded(d, guard);
    }
    match (up, down) {
        (NodeState::Populated, NodeState::Populated) => {
            mk(EdgeState::Green, "both ends are populated".into(), caveats, upstream_measured)
        }
        (NodeState::Populated, NodeState::Empty) => guarded(
            mk(
                EdgeState::Red,
                "⛔ the UPSTREAM is populated and the DOWNSTREAM was read and is EMPTY - THE BREAK"
                    .into(),
                caveats,
                upstream_measured,
            ),
            guard,
        ),
        // Unreachable given the arms above, but stated rather than defaulted.
        (u, d) => mk(
            EdgeState::Unknown,
            format!("no rule covers upstream {} / downstream {}", u.label(), d.label()),
            caveats,
            upstream_measured,
        ),
    }
}

/// ⛔ A registry `guard` may only ever SUPPRESS an accusation - never manufacture one, and never
/// turn a grey green. E-13's guard exists because evaluating it in orbit yields a **false red**;
/// the rule is therefore narrow enough to state in one line, which is what makes it checkable.
fn guarded(mut d: Derivation, guard: Option<&str>) -> Derivation {
    if d.color == EdgeState::Red {
        if let Some(g) = guard {
            d.accusation_suppressed = Some(g.to_string());
            d.color = EdgeState::Unknown;
            d.why = format!(
                "⚠ a RED was SUPPRESSED by this edge's registry guard - an accusation this view is \
                 not entitled to make here. Guard: {g}"
            );
        }
    }
    d
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Geometry - computed HERE, so a control can check it without a window
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct P2 {
    pub x: f32,
    pub y: f32,
}
pub fn p2(x: f32, y: f32) -> P2 {
    P2 { x, y }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RectF {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl RectF {
    pub fn new(x0: f32, y0: f32, w: f32, h: f32) -> Self {
        RectF { x0, y0, x1: x0 + w, y1: y0 + h }
    }
    pub fn center(&self) -> P2 {
        p2((self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5)
    }
    pub fn left_mid(&self) -> P2 {
        p2(self.x0, (self.y0 + self.y1) * 0.5)
    }
    pub fn right_mid(&self) -> P2 {
        p2(self.x1, (self.y0 + self.y1) * 0.5)
    }
    pub fn top_mid(&self) -> P2 {
        p2((self.x0 + self.x1) * 0.5, self.y0)
    }
    pub fn bottom_mid(&self) -> P2 {
        p2((self.x0 + self.x1) * 0.5, self.y1)
    }
    pub fn contains(&self, p: P2) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }
    pub fn contains_rect(&self, r: &RectF) -> bool {
        r.x0 >= self.x0 && r.y0 >= self.y0 && r.x1 <= self.x1 && r.y1 <= self.y1
    }
    pub fn union(&self, r: &RectF) -> RectF {
        RectF {
            x0: self.x0.min(r.x0),
            y0: self.y0.min(r.y0),
            x1: self.x1.max(r.x1),
            y1: self.y1.max(r.y1),
        }
    }
}

/// The layout constants. ⭐ Fixed, and in one place: a layout that moves when the data moves cannot
/// be compared between two runs, and comparing two runs is the job.
pub const COL_W: f32 = 330.0;
pub const NODE_W: f32 = 274.0;
pub const NODE_H: f32 = 60.0;
pub const V_GAP: f32 = 30.0;
pub const TOP: f32 = 104.0;
pub const LEFT: f32 = 26.0;
pub const LABEL_W: f32 = 168.0;
pub const LABEL_H: f32 = 17.0;
pub const BADGE_W: f32 = 210.0;
pub const BADGE_H: f32 = 17.0;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The model
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub column: String,
    pub col_index: usize,
    pub row_index: usize,
    pub kind: NodeKind,
    pub group: String,
    pub rect: RectF,
    pub state: NodeState,
    /// ⭐ Built HERE, so the window and the JSON cannot disagree about what a node reads as.
    pub state_text: String,
    /// `occupancy_mask +0x10 = 16409937 (0xfa6551)` - the witness, its offset and its value.
    pub witness_text: String,
    pub addr: String,
    pub degree: usize,
    pub why: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WriterView {
    /// ⭐ The DECODED STORE - what the badge shows. `insn @ addr`.
    pub text: String,
    pub addr: u64,
    pub vaddr: Option<u64>,
    /// ⚠ The RAW register: the instruction AFTER the store. Carried, never collapsed into `text`.
    pub rip: u64,
    pub rip_vaddr: Option<u64>,
    pub confidence: String,
    pub quotable: bool,
    pub tid: i32,
    pub elapsed_ms: u64,
    pub value_after: Option<String>,
}

impl WriterView {
    /// `mov [rbx+0x10],eax @ v:0x341798` - the edge badge's one line.
    pub fn badge_line(&self) -> String {
        let at = match self.vaddr {
            Some(v) => format!("v:{v:#x}"),
            None => format!("a:{:#x}", self.addr),
        };
        format!("{} @ {at}", self.text)
    }
    /// ⚠⚠ THE CAVEAT TRAVELS WITH THE READING, INLINE - never in a footnote and never omitted.
    pub fn caveat(&self) -> String {
        let rip = match self.rip_vaddr {
            Some(v) => format!("{:#x} = v:{v:#x}", self.rip),
            None => format!("{:#x}", self.rip),
        };
        format!(
            "⚠ A DATA BREAKPOINT IS A TRAP: the raw RIP is the instruction AFTER the store. \
             RIP {rip}; the store above was recovered by decoding BACKWARDS from it. \
             confidence: {}{}",
            self.confidence,
            if self.quotable {
                ""
            } else {
                " ⛔ NOT QUOTABLE - the decode is ambiguous. Close it with `d1-eboot disasm`, do not cite it."
            }
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub up: String,
    pub down: String,
    pub mech: String,
    /// ⭐ DERIVED from the state plane, this tick.
    pub color: EdgeState,
    /// ⚠ DECLARED in registry.toml, as of `as_of`.
    pub declared: EdgeState,
    pub as_of: String,
    pub agrees: bool,
    pub why: String,
    pub caveats: Vec<String>,
    pub upstream_measured: bool,
    pub guard: Option<String>,
    pub accusation_suppressed: Option<String>,
    pub up_state: NodeState,
    pub down_state: NodeState,
    pub badge: EventBadge,
    pub badge_text: String,
    /// The last write attributed **by address** to the downstream surface, if any.
    pub last_writer: Option<WriterView>,
    /// Every event attributed to the downstream surface, newest last.
    pub writers: Vec<WriterView>,

    // geometry
    pub from: P2,
    pub to: P2,
    pub c1: P2,
    pub c2: P2,
    pub label_rect: RectF,
    pub badge_rect: Option<RectF>,
    /// ⭐ `true` when the two endpoints share a column - drawn as a vertical hop, not a curve.
    pub intra_column: bool,
}

impl GraphEdge {
    /// The one-line summary used by the headline and by `to_text`.
    pub fn line(&self) -> String {
        format!(
            "{:<5} {:<8} {:<34} -> {:<24} {}",
            self.id,
            format!("{:?}", self.color).to_lowercase(),
            self.up,
            self.down,
            self.why.lines().next().unwrap_or("")
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ColumnView {
    pub name: String,
    pub index: usize,
    pub x0: f32,
    pub x1: f32,
    pub node_count: usize,
    /// ⚠ Drawn even when empty - see graph.toml. An omitted column silently hides a coverage gap.
    pub empty: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphParams {
    pub age_ms: u64,
    pub stale_after_ms: u64,
    pub held_surfaces: Vec<String>,
    pub refusals: Vec<(String, String)>,
}

impl Default for GraphParams {
    fn default() -> Self {
        Self { age_ms: 0, stale_after_ms: 2_000, held_surfaces: Vec::new(), refusals: Vec::new() }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphModel {
    pub frame_seq: u64,
    pub source: String,
    pub generation: u64,
    pub tick_ms: f32,
    pub stale: bool,
    pub age_ms: u64,
    pub columns: Vec<ColumnView>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// The bounding box of everything drawn. ⭐ The window fits THIS; a node outside it would be
    /// silently clipped, and *"my node failed to render"* and *"my node is off-canvas"* are
    /// otherwise indistinguishable.
    pub world: RectF,
    pub color_counts: BTreeMap<String, usize>,
    pub node_state_counts: BTreeMap<String, usize>,
    /// ⭐⭐ THE LINES THE VIEW EXISTS TO MAKE OBVIOUS: **every** accusing edge, each naming the
    /// witness field that decided it.
    ///
    /// ⚠ A `Vec`, never an `Option`, and that is not a convenience. Two reds on one path are
    /// **independent** and both open - E-15 (no player record) and E-21 (activity lifetime = NONE)
    /// are the worked example - so fixing either alone yields nothing visible. A headline that
    /// showed only the first would teach the reader that the graph has one problem.
    pub accusations: Vec<String>,
    pub headline: Option<String>,
    pub disagreements: Vec<String>,
    pub notes: Vec<String>,
}

impl GraphModel {
    pub fn build(f: &Frame, reg: &Registry, spec: &GraphSpec, p: &GraphParams) -> GraphModel {
        let stale = p.age_ms > p.stale_after_ms;
        let mut notes: Vec<String> =
            f.notes.iter().map(|n| format!("[{}] {}", n.level, n.text)).collect();

        // ── the poked set, straight from the registry's own metadata ────────────────────────────
        let snap_id = match &f.source {
            crate::mem_source::SourceId::Snapshot { id, .. } => id.clone(),
            crate::mem_source::SourceId::Live { pid, .. } => format!("live:{pid}"),
        };
        let poked: Vec<(String, u64)> = reg
            .snapshot_meta(&snap_id)
            .map(|m| m.poked_fields.iter().filter_map(|s| parse_poked(s)).collect())
            .unwrap_or_default();
        let pristine: Vec<String> = reg
            .meta
            .snapshot
            .iter()
            .filter(|s| !s.poked)
            .map(|s| s.id.clone())
            .collect();

        let by_surface: BTreeMap<&str, &SurfaceView> =
            f.surfaces.iter().map(|s| (s.name.as_str(), s)).collect();

        // ── degrees, for in-column ordering ──────────────────────────────────────────────────────
        let mut degree: BTreeMap<&str, usize> = BTreeMap::new();
        for e in &reg.edges {
            *degree.entry(e.up.as_str()).or_default() += 1;
            *degree.entry(e.down.as_str()).or_default() += 1;
        }

        // ── nodes, laid out ─────────────────────────────────────────────────────────────────────
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut columns: Vec<ColumnView> = Vec::new();
        for (ci, col) in spec.meta.columns.iter().enumerate() {
            let x0 = LEFT + ci as f32 * COL_W;
            let mut members: Vec<&NodeSpec> = spec
                .nodes
                .iter()
                .filter(|n| column_of(n, reg) == *col)
                .collect();
            // ⭐ Deterministic: by edge count descending, then by id. Not by anything that moves.
            members.sort_by(|a, b| {
                let da = degree.get(a.id.as_str()).copied().unwrap_or(0);
                let db = degree.get(b.id.as_str()).copied().unwrap_or(0);
                db.cmp(&da).then_with(|| a.id.cmp(&b.id))
            });
            columns.push(ColumnView {
                name: col.clone(),
                index: ci,
                x0,
                x1: x0 + COL_W,
                node_count: members.len(),
                empty: members.is_empty(),
            });
            for (ri, ns) in members.iter().enumerate() {
                let rect = RectF::new(
                    x0 + (COL_W - NODE_W) * 0.5,
                    TOP + ri as f32 * (NODE_H + V_GAP),
                    NODE_W,
                    NODE_H,
                );
                let sv = by_surface.get(ns.id.as_str()).copied();
                let (state, witness_text) = node_state(ns, sv, &poked, &snap_id);
                nodes.push(GraphNode {
                    id: ns.id.clone(),
                    label: ns.label.clone(),
                    column: col.clone(),
                    col_index: ci,
                    row_index: ri,
                    kind: ns.kind,
                    group: sv.map(|s| s.group.clone()).unwrap_or_default(),
                    rect,
                    state_text: state.label().to_string(),
                    state,
                    witness_text,
                    addr: sv.map(|s| s.resolved.render()).unwrap_or_else(|| "-".into()),
                    degree: degree.get(ns.id.as_str()).copied().unwrap_or(0),
                    why: ns.why.clone(),
                });
            }
        }

        // ── events, grouped by the surface the ADDRESS landed in ────────────────────────────────
        let mut writers: BTreeMap<String, Vec<WriterView>> = BTreeMap::new();
        for e in &f.events {
            writers.entry(e.surface.clone()).or_default().push(writer_view(e));
        }
        let tp = TableParams {
            held_surfaces: p.held_surfaces.clone(),
            refusals: p.refusals.clone(),
            ..Default::default()
        };
        let ev_counts: BTreeMap<String, usize> = writers.iter().map(|(k, v)| (k.clone(), v.len())).collect();
        let armed = armed_surface_of(&f.child);

        // ── edges ───────────────────────────────────────────────────────────────────────────────
        let idx: BTreeMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut disagreements = Vec::new();
        for (ei, e) in reg.edges.iter().enumerate() {
            let (Some(&iu), Some(&id_)) = (idx.get(e.up.as_str()), idx.get(e.down.as_str())) else {
                // ⛔ NOT silently dropped. Control #16 fails on this, and the note says so anyway,
                //    because an edge that vanishes from a drawing looks exactly like an edge that
                //    was never declared.
                notes.push(format!(
                    "⛔ edge {} has an endpoint with no node in graph.toml ({:?} -> {:?}) and is NOT \
                     DRAWN - the picture is incomplete, not clean.",
                    e.id, e.up, e.down
                ));
                continue;
            };
            let un = &nodes[iu];
            let dn = &nodes[id_];
            let declared = EdgeState::parse(&e.state).unwrap_or(EdgeState::Unknown);
            let d = derive(declared, &un.state, &dn.state, e.guard.as_deref());

            let badge = badge_for(&dn.id, None, &ev_counts, &f.child, armed.as_deref(), &tp);
            let ws = writers.get(&dn.id).cloned().unwrap_or_default();
            let last_writer = ws.last().cloned();

            let g = edge_geometry(un, dn, ei);
            let agrees = d.color == declared;
            if !agrees {
                disagreements.push(format!(
                    "{}: DERIVED {:?} vs DECLARED {:?} (as_of {}) - {}",
                    e.id,
                    d.color,
                    declared,
                    e.as_of,
                    d.why.lines().next().unwrap_or("")
                ));
            }
            edges.push(GraphEdge {
                id: e.id.clone(),
                up: e.up.clone(),
                down: e.down.clone(),
                mech: e.mech.clone(),
                color: d.color,
                declared,
                as_of: e.as_of.clone(),
                agrees,
                why: d.why,
                caveats: d.caveats,
                upstream_measured: d.upstream_measured,
                guard: e.guard.clone(),
                accusation_suppressed: d.accusation_suppressed,
                up_state: un.state.clone(),
                down_state: dn.state.clone(),
                badge_text: badge.label(),
                badge_rect: if badge == EventBadge::None { None } else { Some(g.badge_rect) },
                badge,
                last_writer,
                writers: ws,
                from: g.from,
                to: g.to,
                c1: g.c1,
                c2: g.c2,
                label_rect: g.label_rect,
                intra_column: g.intra_column,
            });
        }

        // ── counts, world box, headline ─────────────────────────────────────────────────────────
        let mut color_counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in &edges {
            *color_counts.entry(format!("{:?}", e.color).to_lowercase()).or_default() += 1;
        }
        let mut node_state_counts: BTreeMap<String, usize> = BTreeMap::new();
        for n in &nodes {
            *node_state_counts.entry(n.state.label().to_string()).or_default() += 1;
        }
        let mut world = RectF::new(0.0, 0.0, LEFT + spec.meta.columns.len() as f32 * COL_W, TOP);
        for n in &nodes {
            world = world.union(&n.rect);
        }
        for e in &edges {
            world = world.union(&e.label_rect);
            if let Some(b) = &e.badge_rect {
                world = world.union(b);
            }
        }
        world.x1 += 24.0;
        world.y1 += 40.0;

        let accusations: Vec<String> = edges
            .iter()
            .filter(|e| e.color == EdgeState::Red)
            .map(|e| {
                let w = nodes
                    .iter()
                    .find(|n| n.id == e.down)
                    .map(|n| n.witness_text.clone())
                    .unwrap_or_default();
                format!("⛔ {} - {} → {} is RED at {}", e.id, e.up, e.down, w)
            })
            .collect();
        let headline = if accusations.is_empty() {
            None
        } else {
            let mut h = accusations.join("\n");
            if accusations.len() > 1 {
                h.push_str(&format!(
                    "\n⚠⚠ {} SIMULTANEOUS REDS. They are INDEPENDENT accusations, not one problem \
                     seen twice - fixing either alone yields nothing visible.",
                    accusations.len()
                ));
            }
            Some(h)
        };

        // ⛔ A poked witness gets its own loud line naming the pristine snapshots. Without it, the
        //   most important edge in the graph reads UNKNOWN with no route out.
        for n in &nodes {
            if let NodeState::Poked { field, snapshot } = &n.state {
                notes.push(format!(
                    "⛔⛔ node {:?}'s witness {field} is POKED in snapshot {snapshot:?}, so every edge \
                     touching it renders UNKNOWN rather than a colour we would not be entitled to. \
                     ⇒ re-read it on a PRISTINE snapshot: {}",
                    n.id,
                    pristine
                        .iter()
                        .map(|s| format!("--source snap:{s}"))
                        .collect::<Vec<_>>()
                        .join("  or  ")
                ));
            }
        }
        if headline.is_none() {
            notes.push(
                "⚠ NO RED EDGE THIS TICK. That is not the same as 'nothing is broken' - check the \
                 grey and unknown counts below, and read the disagreements: an edge whose witness \
                 could not be read cannot accuse anything."
                    .into(),
            );
        }
        if stale {
            notes.push(format!(
                "⛔ STALE - this frame arrived {} ms ago (threshold {} ms). A frozen canvas and a \
                 quiet client are pixel-identical without this line.",
                p.age_ms, p.stale_after_ms
            ));
        }
        for c in columns.iter().filter(|c| c.empty) {
            notes.push(format!(
                "⚠ column {:?} is EMPTY - no edge in the registry has an endpoint in that group. A \
                 coverage gap in the recon, drawn rather than omitted.",
                c.name
            ));
        }

        GraphModel {
            frame_seq: f.seq,
            source: f.source.label(),
            generation: f.generation,
            tick_ms: f.tick_ms,
            stale,
            age_ms: p.age_ms,
            columns,
            nodes,
            edges,
            world,
            color_counts,
            node_state_counts,
            accusations,
            headline,
            disagreements,
            notes,
        }
    }

    /// Which hit target contains a point, in WORLD coordinates. ⭐ Edge and badge outrank node so a
    /// label drawn over a box is clickable; the order is fixed here rather than in the window, so
    /// the JSON consumer and the window agree about what a click means.
    pub fn hit(&self, p: P2) -> Option<Hit> {
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(b) = &e.badge_rect {
                if b.contains(p) {
                    return Some(Hit::Badge(i));
                }
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if e.label_rect.contains(p) {
                return Some(Hit::Edge(i));
            }
        }
        for (i, n) in self.nodes.iter().enumerate() {
            if n.rect.contains(p) {
                return Some(Hit::Node(i));
            }
        }
        None
    }

    /// The plain-text rendering, for `--headless` without `--json`. Same facts, same order.
    pub fn to_text(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!(
            "{} · seq {} · gen {} · tick {:.1} ms · age {} ms{}\n",
            self.source,
            self.frame_seq,
            self.generation,
            self.tick_ms,
            self.age_ms,
            if self.stale { " · ⛔ STALE" } else { "" }
        ));
        // ⭐ THE HEADLINE FIRST. It is the whole point of the view.
        match &self.headline {
            Some(h) => o.push_str(&format!("\n{h}\n")),
            None => o.push_str("\n(no RED edge this tick)\n"),
        }
        o.push_str(&format!(
            "\n{} node(s) in {} column(s) · {} edge(s) · world {:.0}x{:.0}\n",
            self.nodes.len(),
            self.columns.len(),
            self.edges.len(),
            self.world.x1 - self.world.x0,
            self.world.y1 - self.world.y0
        ));
        o.push_str(&format!(
            "edges   {}\nnodes   {}\n\n",
            render_counts(&self.color_counts),
            render_counts(&self.node_state_counts)
        ));
        for c in &self.columns {
            o.push_str(&format!("── {} ({} node(s)) ──\n", c.name, c.node_count));
            for n in self.nodes.iter().filter(|n| n.col_index == c.index) {
                o.push_str(&format!(
                    "   {:<32} {:<15} {:<26} {}\n",
                    n.id,
                    n.state.label(),
                    n.witness_text,
                    n.addr
                ));
            }
        }
        o.push('\n');
        for e in &self.edges {
            o.push_str(&e.line());
            if !e.agrees {
                o.push_str(&format!("   ⚠ declared {:?}", e.declared));
            }
            o.push('\n');
            if !e.badge_text.is_empty() {
                o.push_str(&format!("      event  {}\n", e.badge_text));
            }
            if let Some(w) = &e.last_writer {
                o.push_str(&format!("      writer {}\n", w.badge_line()));
                o.push_str(&format!("      {}\n", w.caveat()));
            }
        }
        if !self.disagreements.is_empty() {
            o.push_str("\n⚠ DERIVED vs DECLARED - two instruments disagreeing is a finding:\n");
            for d in &self.disagreements {
                o.push_str(&format!("   {d}\n"));
            }
        }
        for n in &self.notes {
            o.push_str(&format!("\n{n}\n"));
        }
        o
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Hit {
    Node(usize),
    Edge(usize),
    Badge(usize),
}

fn render_counts(m: &BTreeMap<String, usize>) -> String {
    m.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(" · ")
}

/// A `graph.toml` node's column: declared, or the registry surface's own group.
fn column_of(n: &NodeSpec, reg: &Registry) -> String {
    if let Some(c) = &n.column {
        return c.clone();
    }
    reg.surface(&n.id).map(|s| s.group.clone()).unwrap_or_default()
}

/// `"sim_player_set+0x10"` / `"sim_player_set+0x18(slot 0)"` → `("sim_player_set", 0x10)`.
///
/// ⚠ The trailing parenthetical is a HUMAN note in the registry, not part of the address; dropping
/// it here rather than failing to parse keeps a poked field from silently becoming un-poked.
fn parse_poked(s: &str) -> Option<(String, u64)> {
    let (name, rest) = s.split_once('+')?;
    let tok: String = rest
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == 'x' || *c == 'X')
        .collect();
    let v = if let Some(h) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()?
    } else {
        tok.parse().ok()?
    };
    Some((name.trim().to_string(), v))
}

/// ⭐ The node's reading, and the string that renders it. **Every non-`Populated`/`Empty` answer
/// names itself and its reason** - there is no arm here that turns a non-reading into a colour.
fn node_state(
    ns: &NodeSpec,
    sv: Option<&SurfaceView>,
    poked: &[(String, u64)],
    snap_id: &str,
) -> (NodeState, String) {
    match ns.kind {
        NodeKind::Server => (
            NodeState::NotMeasurable {
                why: "OUR SERVER - d1-scope reads the CLIENT's memory. This end is asserted, never measured."
                    .into(),
            },
            "-  (not in the state plane)".into(),
        ),
        NodeKind::Unknown => (
            NodeState::NotMeasurable {
                why: "[U] - nobody knows where this lives; see the registry's matching [[open]] entry"
                    .into(),
            },
            "[U]  (no address)".into(),
        ),
        NodeKind::Code => (
            NodeState::NotMeasurable {
                why: "CODE - there is no state to read. Proving it RAN is the event plane's job (--rw exec)."
                    .into(),
            },
            "-  (code)".into(),
        ),
        NodeKind::Surface => {
            let Some(s) = sv else {
                return (
                    NodeState::Unreadable {
                        why: format!("this frame carries no surface named {:?}", ns.id),
                    },
                    "-".into(),
                );
            };
            let Some(w) = &ns.witness else {
                return (NodeState::NoWitness { why: ns.why.clone() }, "-  (no witness declared)".into());
            };
            // ⛔ THE ADDRESS FIRST. `[U]`, UNMAPPED and UNRESOLVED are statements about our reach.
            if !matches!(s.resolved, Resolved::Addr(_)) {
                return (
                    NodeState::Unreadable { why: s.resolved.render() },
                    format!("{w}  - {}", s.resolved.render()),
                );
            }
            let Some(fv) = s.fields.iter().find(|f| f.name == *w) else {
                return (
                    NodeState::NoWitness {
                        why: format!("registry.toml declares no field {w:?} on {:?}", ns.id),
                    },
                    format!("{w}  - NO SUCH FIELD"),
                );
            };
            if poked.iter().any(|(n, o)| *n == ns.id && *o == fv.off) {
                return (
                    NodeState::Poked {
                        field: format!("{}+{:#x}", ns.id, fv.off),
                        snapshot: snap_id.to_string(),
                    },
                    format!("{w} +{:#x} = {}  ⛔ POKED", fv.off, fv.value.render()),
                );
            }
            let text = format!("{w} +{:#x} = {}", fv.off, fv.value.render());
            let rule = ns.witness_rule.as_deref().unwrap_or("!= 0");
            match witness_read(&fv.value) {
                WitnessRead::Num(v) => match rule_holds(rule, v) {
                    Some(true) => (NodeState::Populated, format!("{text}  [{rule}]")),
                    Some(false) => (NodeState::Empty, format!("{text}  [{rule} is FALSE]")),
                    None => (
                        NodeState::NoWitness {
                            why: format!("witness_rule {rule:?} is not in the closed vocabulary"),
                        },
                        text,
                    ),
                },
                WitnessRead::Unmapped => (
                    NodeState::Unreadable {
                        why: format!("{w} +{:#x} is UNMAPPED - the region was never captured. ⛔ NOT 'it was zero'.", fv.off),
                    },
                    text,
                ),
                WitnessRead::NotLive { why } => (
                    NodeState::Unreadable {
                        why: format!("NOT-LIVE: the liveness predicate is false, so the bytes were deliberately not read ({why})"),
                    },
                    text,
                ),
                WitnessRead::NotRead { why } => (
                    NodeState::Unreadable { why: format!("NOT-READ: {why}") },
                    text,
                ),
                WitnessRead::NotNumeric { ty } => (
                    NodeState::NoWitness { why: format!("the witness decodes to {ty}, which no rule compares") },
                    text,
                ),
            }
        }
    }
}

fn writer_view(e: &EventView) -> WriterView {
    WriterView {
        text: if e.writer.is_some() { e.writer_text.clone() } else { "UNDECODABLE".into() },
        addr: e.writer.unwrap_or(e.rip),
        vaddr: e.writer_vaddr,
        rip: e.rip,
        rip_vaddr: e.rip_vaddr,
        confidence: e.writer_confidence.clone(),
        quotable: e.writer_quotable,
        tid: e.tid,
        elapsed_ms: e.elapsed_ms,
        value_after: e.value_after.clone(),
    }
}

fn armed_surface_of(c: &crate::frame::ChildStatus) -> Option<String> {
    use crate::frame::ChildStatus as C;
    match c {
        C::Running { spec, .. } | C::Finished { spec, .. } | C::NoHit { spec, .. } | C::Failed { spec, .. } => {
            spec.surface.clone()
        }
        _ => None,
    }
}

struct Geom {
    from: P2,
    to: P2,
    c1: P2,
    c2: P2,
    label_rect: RectF,
    badge_rect: RectF,
    intra_column: bool,
}

/// ⭐ THREE ROUTES, and each is a different shape so a reader can tell them apart without reading a
/// label: forward (a curve rightwards), **intra-column** (a vertical hop down the column's right
/// edge - E-15 lives here), and backward (a curve that dips below both boxes).
fn edge_geometry(up: &GraphNode, dn: &GraphNode, seq: usize) -> Geom {
    // ⚠ A small per-edge stagger so two edges between the same pair of columns do not overprint.
    let jitter = ((seq % 5) as f32 - 2.0) * 9.0;
    if up.col_index == dn.col_index {
        let (a, b) = if up.row_index <= dn.row_index {
            (up.rect.bottom_mid(), dn.rect.top_mid())
        } else {
            (up.rect.top_mid(), dn.rect.bottom_mid())
        };
        let side = up.rect.x1 + 26.0;
        let c1 = p2(side, a.y);
        let c2 = p2(side, b.y);
        let mid = p2((a.x + b.x) * 0.5 + 34.0, (a.y + b.y) * 0.5);
        return Geom {
            from: a,
            to: b,
            c1,
            c2,
            label_rect: RectF::new(mid.x, mid.y - LABEL_H, LABEL_W, LABEL_H),
            badge_rect: RectF::new(mid.x, mid.y + 2.0, BADGE_W, BADGE_H),
            intra_column: true,
        };
    }
    let forward = dn.col_index > up.col_index;
    let (a, b) = if forward {
        (up.rect.right_mid(), dn.rect.left_mid())
    } else {
        (up.rect.left_mid(), dn.rect.right_mid())
    };
    let dx = (b.x - a.x) * 0.45;
    let c1 = p2(a.x + dx, a.y + jitter);
    let c2 = p2(b.x - dx, b.y + jitter);
    let mid = cubic_at(a, c1, c2, b, 0.5);
    Geom {
        from: a,
        to: b,
        c1,
        c2,
        label_rect: RectF::new(mid.x - LABEL_W * 0.5, mid.y - LABEL_H - 2.0, LABEL_W, LABEL_H),
        badge_rect: RectF::new(mid.x - BADGE_W * 0.5, mid.y + 2.0, BADGE_W, BADGE_H),
        intra_column: false,
    }
}

/// The point on a cubic Bézier at `t`. ⭐ The label sits exactly on the wire because this is the
/// same evaluation the painter uses - not an eyeballed offset that drifts when the layout changes.
pub fn cubic_at(a: P2, c1: P2, c2: P2, b: P2, t: f32) -> P2 {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    p2(
        w0 * a.x + w1 * c1.x + w2 * c2.x + w3 * b.x,
        w0 * a.y + w1 * c1.y + w2 * c2.y + w3 * b.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(l: &str) -> NodeState {
        match l {
            "pop" => NodeState::Populated,
            "empty" => NodeState::Empty,
            "unread" => NodeState::Unreadable { why: "x".into() },
            "nowit" => NodeState::NoWitness { why: "x".into() },
            _ => NodeState::NotMeasurable { why: "x".into() },
        }
    }

    #[test]
    fn populated_to_empty_is_the_only_accusation() {
        assert_eq!(derive(EdgeState::Green, &st("pop"), &st("empty"), None).color, EdgeState::Red);
        assert_eq!(derive(EdgeState::Green, &st("pop"), &st("pop"), None).color, EdgeState::Green);
    }

    #[test]
    fn an_unpopulated_upstream_is_grey_not_red() {
        // ⛔ The rule that stops one break from being charged to three edges.
        assert_eq!(derive(EdgeState::Red, &st("empty"), &st("empty"), None).color, EdgeState::Grey);
        assert_eq!(derive(EdgeState::Red, &st("empty"), &st("unread"), None).color, EdgeState::Grey);
    }

    #[test]
    fn retired_is_never_red() {
        let d = derive(EdgeState::Retired, &st("pop"), &st("empty"), None);
        assert_eq!(d.color, EdgeState::Retired);
    }

    #[test]
    fn a_guard_suppresses_an_accusation_and_nothing_else() {
        let red = derive(EdgeState::Red, &st("pop"), &st("empty"), Some("only in an activity"));
        assert_eq!(red.color, EdgeState::Unknown);
        assert!(red.accusation_suppressed.is_some());
        let green = derive(EdgeState::Green, &st("pop"), &st("pop"), Some("only in an activity"));
        assert_eq!(green.color, EdgeState::Green, "a guard must not touch a green");
    }

    #[test]
    fn a_poked_end_is_never_a_colour() {
        let p = NodeState::Poked { field: "q+0x10".into(), snapshot: "s".into() };
        assert_eq!(derive(EdgeState::Red, &st("pop"), &p, None).color, EdgeState::Unknown);
        assert_eq!(derive(EdgeState::Green, &p, &st("pop"), None).color, EdgeState::Unknown);
    }

    #[test]
    fn poked_field_parsing_survives_the_human_note() {
        assert_eq!(parse_poked("sim_player_set+0x10"), Some(("sim_player_set".into(), 0x10)));
        assert_eq!(parse_poked("sim_player_set+0x18(slot 0)"), Some(("sim_player_set".into(), 0x18)));
        assert_eq!(parse_poked("nonsense"), None);
    }

    #[test]
    fn the_witness_declines_rather_than_guessing() {
        assert_eq!(witness_read(&Value::Unmapped), WitnessRead::Unmapped);
        assert_eq!(witness_read(&Value::Ptr(0)), WitnessRead::Num(0));
        assert!(matches!(witness_read(&Value::Floats(vec![1.0])), WitnessRead::NotNumeric { .. }));
        assert_eq!(rule_holds("!= 0", 1), Some(true));
        assert_eq!(rule_holds("nonsense", 1), None);
    }
}
