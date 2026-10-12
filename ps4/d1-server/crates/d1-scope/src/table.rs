// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐⭐ **THE SURFACE TABLE, AS DATA.** Rows, cells, sort keys, filter, grouping, render-state and
//! event badge - all of it computed here, from a [`Frame`] and nothing else, in a module that knows
//! **no egui type**.
//!
//! ⛔ **This is an extension of the `present` chokepoint, not a view layer.** The egui window and
//! `d1-scope gui --headless --json` are two renderings of the *same* [`TableModel`]: the window
//! iterates [`Column::ALL`] and prints [`TableRow::cell`], and the JSON dump serialises the very
//! same strings. A column the UI could draw that the JSON does not carry is exactly the drift this
//! split exists to prevent, and control **#11** fails on it - it compares the strings the AccessKit
//! tree actually contains against the strings the model actually holds.
//!
//! # ⛔ FOUR FACTS THAT LOOK ALIKE ON A SCREEN AND ARE NOT ALIKE
//!
//! | render state | what it means | ⛔ what it is NOT |
//! |---|---|---|
//! | `VALUE` | bytes were read at a mapped address | - (a pointer that reads `0` is a **VALUE**) |
//! | `UNMAPPED` | the region was never captured / is not mapped | *"it was zero"* |
//! | `NOT-LIVE` | the liveness predicate said no, so the bytes were **not read** | *"it was zero"* |
//! | `NOT-READ` | the surface address never resolved, so no field could be read | `UNMAPPED` |
//!
//! ⭐ And a fifth, which is about the *instrument* rather than the client:
//! **`STALE`** - the frame on screen is older than the staleness threshold, i.e. **the sampler has
//! stopped feeding us**. A frozen window showing month-old numbers is pixel-identical to a live
//! window showing stable numbers, and that is the same failure shape as UNMAPPED-vs-zero, moved
//! into the time axis. ⛔ Staleness therefore **wraps** the underlying state rather than replacing
//! it (`STALE ‹UNMAPPED›`): collapsing the two would destroy the reading, and the reading is
//! evidence about the client while staleness is evidence about us.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::frame::{ChildStatus, EventView, Frame, Resolved, SurfaceView, Value, Verdict};

/// ⛔ SEVEN STATES, and collapsing any two mis-reports a surface. The brief names four; the other
/// three exist because [`Resolved`] and [`Value`] really do distinguish them, and folding
/// `NOT-READ` into `UNMAPPED` would claim *"the region was not captured"* about a surface whose
/// **address never resolved** - a statement about the wrong thing entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum RenderState {
    /// Bytes were read at a mapped address. ⭐ Includes a pointer that read `0`: that is a
    /// measurement whose answer is zero, and it is evidence.
    Value,
    /// The region was not captured (snapshot) or is not mapped (live). ⛔ Never `0`.
    Unmapped,
    /// The liveness predicate was false, so the field was deliberately **not read**.
    NotLive,
    /// The surface address did not resolve, so no field under it could be read.
    NotRead,
    /// A deref hop failed. `why` is on the row.
    Unresolved,
    /// The registry says this needs a Rust resolver and none produced an address.
    NotExpressible,
    /// The registry `addr` is `[U]` - nobody knows where this lives. See its `[[open]]` entry.
    UnknownAddr,
}

impl RenderState {
    pub fn label(self) -> &'static str {
        match self {
            RenderState::Value => "VALUE",
            RenderState::Unmapped => "UNMAPPED",
            RenderState::NotLive => "NOT-LIVE",
            RenderState::NotRead => "NOT-READ",
            RenderState::Unresolved => "UNRESOLVED",
            RenderState::NotExpressible => "NOT-EXPRESSIBLE",
            RenderState::UnknownAddr => "[U]",
        }
    }

    /// ⭐ `true` only for a state that is a **measurement of the client**. Everything else is a
    /// statement about our reach, and a reader who sorts by this column is asking exactly that.
    pub fn is_measurement(self) -> bool {
        self == RenderState::Value
    }

    pub const ALL: [RenderState; 7] = [
        RenderState::Value,
        RenderState::Unmapped,
        RenderState::NotLive,
        RenderState::NotRead,
        RenderState::Unresolved,
        RenderState::NotExpressible,
        RenderState::UnknownAddr,
    ];
}

/// The render state of one [`Value`]. ⛔ Note what is NOT here: there is no arm that turns any
/// non-reading into `Value`.
pub fn state_of_value(v: &Value) -> RenderState {
    match v {
        Value::Int(_)
        | Value::Ints(_)
        | Value::Floats(_)
        | Value::Ptr(_)
        | Value::Ascii(_)
        | Value::Bytes(_) => RenderState::Value,
        Value::Unmapped => RenderState::Unmapped,
        Value::NotMeasurable { .. } => RenderState::NotLive,
        Value::NotRead { .. } => RenderState::NotRead,
    }
}

/// The render state of a surface row: its address, then its liveness. ⚠ A surface that resolved but
/// whose liveness is false is `NOT-LIVE`, not `VALUE` - the address is fine and the object is torn
/// down, and reading it anyway is the mistake this project has made twice.
pub fn state_of_surface(s: &SurfaceView) -> RenderState {
    match &s.resolved {
        Resolved::Addr(_) => match s.liveness.verdict {
            Verdict::NotLive => RenderState::NotLive,
            _ => RenderState::Value,
        },
        Resolved::Unmapped { .. } => RenderState::Unmapped,
        Resolved::Unresolvable { .. } => RenderState::Unresolved,
        Resolved::NotExpressible { .. } => RenderState::NotExpressible,
        Resolved::Unknown => RenderState::UnknownAddr,
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The event badge

/// ⭐ What the EVENT plane has to say about this row - and it says four different kinds of nothing.
///
/// ⛔ `ArmedNothingYet` and `ArmedNeverFired` are **not** the same fact and must never share a
/// badge. *"The window is still open"* is not a result; *"the watch completed and saw no write"* is
/// a **real negative** - and it is only a negative about the threads that were armed, for the
/// window that was watched, which is why the honest caveat travels with it.
// ⚠ `PartialEq` only - `lease_remaining_s` is an `f64` and there is no total order on it. That is
// fine here: badges are compared for equality in the controls, never sorted.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum EventBadge {
    /// Nothing in the event plane concerns this row.
    None,
    /// N writes were attributed to this row **by address**.
    Armed { writes: usize },
    /// A child is armed on this row and the lease has not expired. ⛔ NOT a negative result.
    ArmedNothingYet { lease_remaining_s: f64 },
    /// ⭐ A completed watch with zero hits: a real negative, with its bounds attached.
    ArmedNeverFired { watched_s: f64, caveat: String },
    /// We hold this slot and are **not** arming it - one `d1-live` process watches one address.
    Held,
    /// A fifth watch was asked for. Four debug registers is a hardware ceiling, not a policy.
    SlotFull { why: String },
    /// We declined, and said why. A refusal is a message, not an error code.
    Refused { why: String },
    /// The child's stdout did not parse. ⛔ Distinct from `ArmedNeverFired`, which parsed fine.
    Failed { stderr_head: String },
}

impl EventBadge {
    /// The cell string. ⭐ Short enough for a column, and never ambiguous between the two
    /// zero-hit meanings.
    pub fn label(&self) -> String {
        match self {
            EventBadge::None => String::new(),
            EventBadge::Armed { writes } => {
                format!("ARMED · {writes} write{}", if *writes == 1 { "" } else { "s" })
            }
            EventBadge::ArmedNothingYet { lease_remaining_s } => {
                format!("ARMED · nothing yet ({lease_remaining_s:.0}s left)")
            }
            EventBadge::ArmedNeverFired { watched_s, .. } => {
                format!("ARMED · NEVER FIRED ({watched_s:.1}s)")
            }
            EventBadge::Held => "SLOT HELD · NOT ARMED".to_string(),
            EventBadge::SlotFull { .. } => "SLOT FULL".to_string(),
            EventBadge::Refused { .. } => "REFUSED".to_string(),
            EventBadge::Failed { .. } => "CHILD FAILED".to_string(),
        }
    }

    /// ⭐⭐ **THE LABEL WITH ITS NUMBERS REMOVED** - everything up to the first `(`.
    ///
    /// ⛔ THIS EXISTS BECAUSE AN INVERSION CAUGHT THE CONTROL BEING WRONG. Control #13's "the two
    /// zero-hit badges do not share a label" leg used to compare the *full* labels. Collapsing
    /// `ARMED · nothing yet` into `ARMED · NEVER FIRED` - the exact defect the leg exists to catch -
    /// **still passed**, because the two rendered `…(0.0s)` and `…(2.0s)` and the leg was, without
    /// anyone noticing, comparing the SECONDS rather than the STATEMENT. Two badges that say the
    /// same thing about different windows are the same badge.
    pub fn label_stem(&self) -> String {
        let l = self.label();
        l.split('(').next().unwrap_or(&l).trim().to_string()
    }

    /// The long form, for a tooltip or a `--json` reader. Refusals and negatives carry their whole
    /// reason here; a badge that only ever showed `label()` would be a summary of a summary.
    pub fn detail(&self) -> String {
        match self {
            EventBadge::None => String::new(),
            EventBadge::Armed { writes } => format!("{writes} write(s) attributed to this row by ADDRESS"),
            EventBadge::ArmedNothingYet { lease_remaining_s } => format!(
                "the child is armed and its lease has {lease_remaining_s:.1}s left. ⛔ This is NOT \
                 a negative result - the window is still open."
            ),
            EventBadge::ArmedNeverFired { watched_s, caveat } => format!(
                "a completed watch of {watched_s:.1}s saw ZERO writes. ⭐ A real negative - and only \
                 about what it watched: {caveat}"
            ),
            EventBadge::Held => "we hold this slot and are NOT arming it: one d1-live process \
                                 watches one address, and one child is the lifecycle invariant"
                .to_string(),
            EventBadge::SlotFull { why } => why.clone(),
            EventBadge::Refused { why } => why.clone(),
            EventBadge::Failed { stderr_head } => {
                format!("the child's stdout did not parse. stderr: {stderr_head}")
            }
        }
    }

    /// ⭐ Classify a REAL [`crate::child_mgr::Refusal`] message - never a transcribed copy of one.
    /// The slot ceiling has a single spelling, owned by
    /// [`crate::child_mgr::slot_ceiling_message`], and control #13 feeds *that function's* output
    /// through here so the classifier cannot drift from the thing it classifies.
    pub fn classify_refusal(why: &str) -> EventBadge {
        if why.contains(crate::child_mgr::SLOT_CEILING_MARKER) {
            EventBadge::SlotFull { why: why.to_string() }
        } else {
            EventBadge::Refused { why: why.to_string() }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Columns

/// ⭐ **THE COLUMN SET IS DATA.** The UI does not name columns; it iterates this. That is what makes
/// *"the table and the JSON are the same rows"* a structural property rather than a promise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Column {
    Group,
    Name,
    Addr,
    Value,
    State,
    Badge,
}

impl Column {
    pub const ALL: [Column; 6] =
        [Column::Group, Column::Name, Column::Addr, Column::Value, Column::State, Column::Badge];

    pub fn title(self) -> &'static str {
        match self {
            Column::Group => "group",
            Column::Name => "name",
            Column::Addr => "addr",
            Column::Value => "value",
            Column::State => "state",
            Column::Badge => "event",
        }
    }

    pub fn parse(s: &str) -> Option<Column> {
        Column::ALL.into_iter().find(|c| c.title().eq_ignore_ascii_case(s.trim()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Rows

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RowKind {
    /// A group header. Present only when grouping is on.
    GroupHeader,
    Surface,
    Field,
}

#[derive(Clone, Debug, Serialize)]
pub struct TableRow {
    /// ⭐ Stable across ticks - `"surface"` or `"surface.field"`. Change detection and the flash
    /// animation key off THIS, never off the row index: the index moves when the sort changes and a
    /// re-sort would then flash the whole table.
    pub key: String,
    pub kind: RowKind,
    pub depth: u8,

    // ── the six cells, pre-rendered ──────────────────────────────────────────────────────────────
    pub group: String,
    pub name: String,
    pub addr: String,
    pub value: String,
    pub state_text: String,
    pub badge_text: String,

    // ── the facts behind the cells ───────────────────────────────────────────────────────────────
    pub state: RenderState,
    /// ⛔ Carried ALONGSIDE `state`, never folded into it. See the module header.
    pub stale: bool,
    pub badge: EventBadge,
    /// Numeric sort key for `addr`. `None` sorts last, always - an unresolved row must not
    /// masquerade as address zero.
    pub addr_num: Option<u64>,
    pub confidence: String,
    pub reach: String,
    pub liveness: String,
    pub note: Option<String>,
    pub resolve_trace: Vec<String>,

    /// ⭐ `(state, value)` - the change signature. A row that goes `0x40 → UNMAPPED` changed even
    /// though both are "not the same string", and a row that goes `UNMAPPED → 0` changed even
    /// though a value-only comparison would see `"UNMAPPED"` vs `"0"`. Comparing the STATE too is
    /// what makes a *reading-class* change visible, and control #12 inverts exactly that.
    pub sig: String,
    /// Set by [`TableModel::mark_changes`]. ⚠ A row that was not in the previous tick is **not**
    /// "changed" - otherwise the first frame flashes everything and the flash means nothing.
    pub changed: bool,
}

impl TableRow {
    /// ⭐ THE ONLY WAY A CELL STRING IS OBTAINED, by the UI and by `--json` alike.
    pub fn cell(&self, c: Column) -> &str {
        match c {
            Column::Group => &self.group,
            Column::Name => &self.name,
            Column::Addr => &self.addr,
            Column::Value => &self.value,
            Column::State => &self.state_text,
            Column::Badge => &self.badge_text,
        }
    }

    /// Sort key. Addresses sort **numerically** - a lexicographic sort of `a:0x…` puts
    /// `0x9…` before `0x10…` and reads as a scrambled table nobody trusts.
    fn sort_key(&self, c: Column) -> (u64, String) {
        match c {
            // `u64::MAX` for "no address": unresolved rows go to the end in ascending order rather
            // than pretending to live at 0.
            Column::Addr => (self.addr_num.unwrap_or(u64::MAX), self.key.clone()),
            _ => (0, format!("{}\u{0}{}", self.cell(c).to_lowercase(), self.key)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Parameters

#[derive(Clone, Debug, Serialize)]
pub struct TableParams {
    pub filter: String,
    pub sort: Column,
    pub descending: bool,
    pub group_by_group: bool,
    pub show_fields: bool,
    /// How long ago this frame ARRIVED, in ms. ⭐ Not `frame.ts_mono_ms` - the question is *"has the
    /// producer stopped?"*, and only the consumer can answer it.
    pub age_ms: u64,
    pub stale_after_ms: u64,
    /// Surfaces we hold a slot for and are not arming. Filled from `ChildMgr::pending()` by
    /// whichever front end owns a child; empty here, because BUILD 3's window arms nothing.
    pub held_surfaces: Vec<String>,
    /// `(surface, refusal message)` - verbatim [`crate::child_mgr::Refusal::message`] output.
    pub refusals: Vec<(String, String)>,
}

impl Default for TableParams {
    fn default() -> Self {
        Self {
            filter: String::new(),
            sort: Column::Group,
            descending: false,
            group_by_group: true,
            show_fields: true,
            age_ms: 0,
            // ⚠ Two seconds, deliberately shorter than the default `--interval-ms 1000` doubled:
            // one missed tick is a hiccup, two is a producer that has stopped.
            stale_after_ms: 2_000,
            held_surfaces: Vec::new(),
            refusals: Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The model

#[derive(Clone, Debug, Serialize)]
pub struct TableModel {
    pub frame_seq: u64,
    pub source: String,
    pub generation: u64,
    pub tick_ms: f32,
    /// ⭐ Frame-level: everything on screen is older than the threshold.
    pub stale: bool,
    pub age_ms: u64,
    pub rows: Vec<TableRow>,
    /// Rows before the filter ran. ⛔ Printed always: *"3 rows"* with no denominator is
    /// indistinguishable from a broken model, and a filter that silently matches nothing is the
    /// exact false negative this project keeps paying for.
    pub rows_total: usize,
    pub rows_filtered_out: usize,
    /// Counts by render state, over the VISIBLE rows.
    pub state_counts: BTreeMap<String, usize>,
    pub params: TableParams,
    /// Loud conditions, verbatim from the frame plus the ones this model adds.
    pub notes: Vec<String>,
}

/// The change signatures of a tick, for the next tick to diff against.
pub type Signatures = BTreeMap<String, String>;

impl TableModel {
    pub fn build(f: &Frame, p: &TableParams) -> TableModel {
        let stale = p.age_ms > p.stale_after_ms;
        let armed = armed_surface(&f.child);
        let by_surface = events_by_row(&f.events);

        let mut all: Vec<TableRow> = Vec::with_capacity(f.surfaces.len() * 4);
        for s in &f.surfaces {
            let sstate = state_of_surface(s);
            let base = match &s.resolved {
                Resolved::Addr(a) => Some(*a),
                _ => None,
            };
            let badge = badge_for(&s.name, None, &by_surface, &f.child, armed.as_deref(), p);
            all.push(TableRow {
                key: s.name.clone(),
                kind: RowKind::Surface,
                depth: 0,
                group: s.group.clone(),
                name: s.name.clone(),
                addr: s.resolved.render(),
                value: surface_value_text(s),
                state_text: state_text(sstate, stale),
                badge_text: badge.label(),
                state: sstate,
                stale,
                badge,
                addr_num: base,
                confidence: s.confidence.clone(),
                reach: s.reach.clone(),
                liveness: format!("{}  ⇒  {}", s.liveness.expr, s.liveness.why),
                note: None,
                resolve_trace: s.resolve_trace.clone(),
                sig: format!("{}|{}", sstate.label(), s.resolved.render()),
                changed: false,
            });

            if !p.show_fields {
                continue;
            }
            for fld in &s.fields {
                let fstate = state_of_value(&fld.value);
                let key = format!("{}.{}", s.name, fld.name);
                let badge = badge_for(
                    &s.name,
                    Some(&fld.name),
                    &by_surface,
                    &f.child,
                    armed.as_deref(),
                    p,
                );
                let vtext = fld.value.render();
                all.push(TableRow {
                    key: key.clone(),
                    kind: RowKind::Field,
                    depth: 1,
                    group: s.group.clone(),
                    name: format!("{}·{}", s.name, fld.name),
                    addr: match base {
                        Some(b) => format!("a:{:#x}  (+{:#x})", b + fld.off, fld.off),
                        // ⛔ NOT a blank cell and NOT `+0x10`: the row must say why there is no
                        // address, or a reader infers "offset zero".
                        None => format!("{}  (+{:#x})", s.resolved.render(), fld.off),
                    },
                    value: vtext.clone(),
                    state_text: state_text(fstate, stale),
                    badge_text: badge.label(),
                    state: fstate,
                    stale,
                    badge,
                    addr_num: base.map(|b| b + fld.off),
                    confidence: s.confidence.clone(),
                    reach: s.reach.clone(),
                    liveness: format!("{}  ⇒  {}", s.liveness.expr, s.liveness.why),
                    note: fld.note.clone(),
                    resolve_trace: Vec::new(),
                    sig: format!("{}|{}|{}", fstate.label(), fld.ty, vtext),
                    changed: false,
                });
            }
        }

        let rows_total = all.len();

        // ── filter ──────────────────────────────────────────────────────────────────────────────
        // ⭐ A matching FIELD keeps its SURFACE row. A field row shown without the surface it lives
        //   under is a floating address with no provenance, which is precisely what rail 10 of the
        //   NOT-LIST forbids.
        let needle = p.filter.trim().to_lowercase();
        let mut kept: Vec<TableRow> = if needle.is_empty() {
            all
        } else {
            let mut keep_surface: BTreeMap<String, bool> = BTreeMap::new();
            for r in &all {
                if row_matches(r, &needle) {
                    let owner = r.key.split('.').next().unwrap_or(&r.key).to_string();
                    keep_surface.insert(owner, true);
                }
            }
            all.into_iter()
                .filter(|r| {
                    if row_matches(r, &needle) {
                        return true;
                    }
                    // keep the parent of a matching field, and nothing else
                    r.kind == RowKind::Surface && keep_surface.contains_key(&r.key)
                })
                .collect()
        };
        let rows_filtered_out = rows_total - kept.len();

        // ── sort ────────────────────────────────────────────────────────────────────────────────
        // ⚠ FIELDS FOLLOW THEIR SURFACE. Sorting a flat list would scatter a surface's fields
        //   across the table under other surfaces' names - the cells would all be individually
        //   correct and the table would be a lie about structure. So: surfaces are ordered by the
        //   sort key; a surface's fields are ordered by the sort key WITHIN it.
        let mut order: BTreeMap<String, Vec<TableRow>> = BTreeMap::new();
        let mut surfaces: Vec<TableRow> = Vec::new();
        for r in kept.drain(..) {
            match r.kind {
                RowKind::Surface => surfaces.push(r),
                _ => order.entry(r.key.split('.').next().unwrap_or("").to_string()).or_default().push(r),
            }
        }
        let cmp = |a: &TableRow, b: &TableRow| a.sort_key(p.sort).cmp(&b.sort_key(p.sort));
        surfaces.sort_by(cmp);
        if p.descending {
            surfaces.reverse();
        }
        for v in order.values_mut() {
            v.sort_by(cmp);
            if p.descending {
                v.reverse();
            }
        }

        // ── group ───────────────────────────────────────────────────────────────────────────────
        let mut rows: Vec<TableRow> = Vec::with_capacity(rows_total + 12);
        if p.group_by_group {
            // Group order follows the sorted surface order, so the sort column drives the groups too.
            let mut seen: Vec<String> = Vec::new();
            for s in &surfaces {
                if !seen.contains(&s.group) {
                    seen.push(s.group.clone());
                }
            }
            for g in seen {
                let members: Vec<&TableRow> = surfaces.iter().filter(|s| s.group == g).collect();
                let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
                let mut n_fields = 0usize;
                for m in &members {
                    *counts.entry(m.state.label()).or_default() += 1;
                    n_fields += order.get(&m.key).map(|v| v.len()).unwrap_or(0);
                }
                let summary = counts
                    .iter()
                    .map(|(k, v)| format!("{v} {k}"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                rows.push(TableRow {
                    key: format!("::group::{g}"),
                    kind: RowKind::GroupHeader,
                    depth: 0,
                    group: g.clone(),
                    name: g.clone(),
                    addr: String::new(),
                    value: format!("{} surface(s), {n_fields} field(s)", members.len()),
                    state_text: summary,
                    badge_text: String::new(),
                    state: RenderState::Value,
                    stale,
                    badge: EventBadge::None,
                    addr_num: None,
                    confidence: String::new(),
                    reach: String::new(),
                    liveness: String::new(),
                    note: None,
                    resolve_trace: Vec::new(),
                    sig: String::new(),
                    changed: false,
                });
                for s in surfaces.iter().filter(|s| s.group == g) {
                    rows.push(s.clone());
                    if let Some(v) = order.get(&s.key) {
                        rows.extend(v.iter().cloned());
                    }
                }
            }
        } else {
            for s in &surfaces {
                rows.push(s.clone());
                if let Some(v) = order.get(&s.key) {
                    rows.extend(v.iter().cloned());
                }
            }
        }

        let mut state_counts: BTreeMap<String, usize> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.kind != RowKind::GroupHeader) {
            *state_counts.entry(r.state.label().to_string()).or_default() += 1;
        }

        let mut notes: Vec<String> = f.notes.iter().map(|n| format!("[{}] {}", n.level, n.text)).collect();
        if stale {
            notes.push(format!(
                "⛔ STALE - this frame arrived {} ms ago (threshold {} ms). The numbers on screen are \
                 not a current reading of anything. ⚠ A stopped sampler and a quiet client look \
                 IDENTICAL without this line.",
                p.age_ms, p.stale_after_ms
            ));
        }
        // ⚠ `SourceId::label()` renders `snap:<id>`, not `snapshot …`. Written the other way first,
        //   and the note therefore NEVER FIRED - a silent no-op that read exactly like "there was
        //   nothing to warn about". Caught by running it, not by reading it.
        if f.source.label().starts_with("snap:") {
            notes.push(
                "⚠ SNAPSHOT SOURCE - the bytes are FROZEN by construction. No row will ever flash, \
                 and that is not evidence that the client is idle."
                    .to_string(),
            );
        }
        if !needle.is_empty() && rows.is_empty() {
            notes.push(format!(
                "⚠ the filter {needle:?} matched 0 of {rows_total} rows - an empty table here means \
                 'nothing matched', NOT 'nothing was sampled'."
            ));
        }

        TableModel {
            frame_seq: f.seq,
            source: f.source.label(),
            generation: f.generation,
            tick_ms: f.tick_ms,
            stale,
            age_ms: p.age_ms,
            rows,
            rows_total,
            rows_filtered_out,
            state_counts,
            params: p.clone(),
            notes,
        }
    }

    /// The signatures of this tick, keyed by row key.
    pub fn signatures(&self) -> Signatures {
        self.rows
            .iter()
            .filter(|r| r.kind != RowKind::GroupHeader)
            .map(|r| (r.key.clone(), r.sig.clone()))
            .collect()
    }

    /// ⭐ Mark rows whose `(state, value)` differs from the previous tick's. A row absent from
    /// `prev` is **not** marked: "new" is not "changed", and flashing every row on the first frame
    /// teaches the operator to ignore the flash.
    pub fn mark_changes(&mut self, prev: &Signatures) -> usize {
        let mut n = 0;
        for r in &mut self.rows {
            if let Some(old) = prev.get(&r.key) {
                if *old != r.sig {
                    r.changed = true;
                    n += 1;
                }
            }
        }
        n
    }

    /// A plain-text rendering, for `--headless` without `--json`. Same cells, same order.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let widths = [11usize, 46, 34, 40, 22, 30];
        let head: Vec<String> = Column::ALL
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{:<w$}", c.title(), w = w))
            .collect();
        out.push_str(&head.join(" "));
        out.push('\n');
        for r in &self.rows {
            if r.kind == RowKind::GroupHeader {
                out.push_str(&format!("── {} ──\n", r.name));
                continue;
            }
            let cells: Vec<String> = Column::ALL
                .iter()
                .zip(widths)
                .map(|(c, w)| {
                    // ⚠ A truncation that does not announce itself is a rendering nobody can check
                    //   against the JSON. The ellipsis is the receipt.
                    let s = r.cell(*c);
                    let s: String = if s.chars().count() > w {
                        s.chars().take(w - 1).chain(std::iter::once('…')).collect()
                    } else {
                        s.to_string()
                    };
                    format!("{s:<w$}", w = w)
                })
                .collect();
            out.push_str(&format!("{}{}\n", if r.changed { "*" } else { " " }, cells.join(" ")));
        }
        out
    }
}

fn state_text(s: RenderState, stale: bool) -> String {
    if stale {
        // ⛔ WRAPS, never replaces: the reading is evidence about the client, staleness is evidence
        // about us, and a reader needs both.
        format!("STALE ‹{}›", s.label())
    } else {
        s.label().to_string()
    }
}

/// ⛔ **THE FILTER MATCHES ONLY WHAT IS VISIBLE**, i.e. the six cells - never the hover note, never
/// the resolve trace, never the liveness prose.
///
/// This was written the other way first and the first real run caught it: filtering `orbit-all` for
/// `"world_controller"` returned `game_globals` and `handler35_countdown`, because a registry
/// *note* mentioned the phrase. Every cell on those rows was correct and the operator had no way on
/// screen to answer *"why is this row here?"* - a row you cannot explain from the table is the same
/// failure as a value without provenance. ⇒ if a hidden field should be searchable, it has to
/// become a column first.
fn row_matches(r: &TableRow, needle: &str) -> bool {
    Column::ALL.iter().any(|c| r.cell(*c).to_lowercase().contains(needle))
}

fn surface_value_text(s: &SurfaceView) -> String {
    let v = match s.liveness.verdict {
        Verdict::Live => "LIVE",
        Verdict::NotLive => "NOT-LIVE",
        Verdict::Unknown => "liveness UNKNOWN",
    };
    format!("{v} · {} field(s) · {} {}", s.fields.len(), s.confidence, s.reach)
}

/// The surface the single child claims to be watching, if any.
fn armed_surface(c: &ChildStatus) -> Option<String> {
    match c {
        ChildStatus::Running { spec, .. }
        | ChildStatus::Finished { spec, .. }
        | ChildStatus::NoHit { spec, .. }
        | ChildStatus::Failed { spec, .. } => spec.surface.clone(),
        _ => None,
    }
}

/// Events keyed by `"surface"` and by `"surface.field"`.
///
/// ⭐ Attribution came from the ADDRESS ([`crate::events::attach_by_address`]), not from what the
/// slot claimed - so a badge on this row is a statement that a write LANDED here, not that someone
/// intended to watch here.
fn events_by_row(events: &[EventView]) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for e in events {
        *m.entry(e.surface.clone()).or_default() += 1;
        if let Some(f) = &e.field {
            *m.entry(format!("{}.{}", e.surface, f)).or_default() += 1;
        }
    }
    m
}

/// ⭐ `pub` since BUILD 4: the **graph** puts a badge on an EDGE (keyed by its downstream surface)
/// and the table puts one on a ROW. One classifier, one spelling, two renderings - the same reason
/// [`crate::table::TableRow::cell`] is the only way a cell string is obtained.
pub fn badge_for(
    surface: &str,
    field: Option<&str>,
    by_row: &BTreeMap<String, usize>,
    child: &ChildStatus,
    armed: Option<&str>,
    p: &TableParams,
) -> EventBadge {
    let key = match field {
        Some(f) => format!("{surface}.{f}"),
        None => surface.to_string(),
    };
    if let Some(n) = by_row.get(&key) {
        if *n > 0 {
            return EventBadge::Armed { writes: *n };
        }
    }
    // A refusal named for this surface outranks "held": we did not get as far as holding it.
    if let Some((_, why)) = p.refusals.iter().find(|(s, _)| s == surface) {
        return EventBadge::classify_refusal(why);
    }
    if armed == Some(surface) {
        // ⛔ Only a SURFACE row carries the armed badge when the child names no field: the watch is
        //    on one address, and painting it across every field of the surface would claim four
        //    watches where there is one.
        let owns = field.is_none();
        if owns {
            return match child {
                ChildStatus::Running { lease_remaining_s, .. } => {
                    EventBadge::ArmedNothingYet { lease_remaining_s: *lease_remaining_s }
                }
                ChildStatus::NoHit { watched_s, why, .. } => {
                    EventBadge::ArmedNeverFired { watched_s: *watched_s, caveat: why.clone() }
                }
                ChildStatus::Finished { hits: 0, stop_reason, .. } => EventBadge::ArmedNeverFired {
                    watched_s: 0.0,
                    caveat: format!("the child finished ({stop_reason}) with zero hits"),
                },
                ChildStatus::Failed { stderr, .. } => EventBadge::Failed {
                    stderr_head: stderr.lines().next().unwrap_or("").to_string(),
                },
                _ => EventBadge::None,
            };
        }
    }
    if p.held_surfaces.iter().any(|h| h == surface) && field.is_none() {
        return EventBadge::Held;
    }
    EventBadge::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seven_states_have_seven_distinct_labels() {
        let mut v: Vec<&str> = RenderState::ALL.iter().map(|s| s.label()).collect();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), RenderState::ALL.len(), "two states share a label");
    }

    #[test]
    fn stale_wraps_the_state_and_does_not_replace_it() {
        let s = state_text(RenderState::Unmapped, true);
        assert!(s.contains("STALE"), "{s}");
        assert!(s.contains("UNMAPPED"), "the underlying reading must survive: {s}");
    }

    #[test]
    fn a_null_pointer_is_a_value_not_an_unmapped() {
        assert_eq!(state_of_value(&Value::Ptr(0)), RenderState::Value);
        assert_eq!(state_of_value(&Value::Unmapped), RenderState::Unmapped);
    }

    #[test]
    fn every_column_is_reachable_by_name() {
        for c in Column::ALL {
            assert_eq!(Column::parse(c.title()), Some(c));
        }
        assert_eq!(Column::parse("nope"), None);
    }
}
