// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The state plane: **one tick = re-resolve every chain from its root**.
//!
//! ⛔⛔ NOTHING IS CACHED ACROSS TICKS, and that is not an optimisation left undone - it is the
//! correctness property. Every surface here is reached through a pointer chain rooted in BSS
//! (`[v:0x27E7F18]+0x12A78+0x10` is three hops), and the client reallocates, re-publishes and tears
//! those objects down underneath us. A cached leaf address that is re-read next tick is a read of
//! *whatever now lives there* - which returns plausible numbers and is how a whole causal theory
//! gets published on a stale pointer. Re-resolving costs microseconds against a snapshot.
//!
//! ⚠ The liveness predicate is evaluated **before** the fields, and a surface whose liveness says
//! `NotLive` reports [`Value::NotMeasurable`] rather than the bytes at that address. Registry
//! objects are cleared on teardown while descriptors survive, so a read after a disconnect shows
//! zeros and is indistinguishable from "delivery never worked". That mistake has been made twice.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::eval::{Evaluator, Reading, Ty};
use crate::frame::*;
// ⚠ `MemSource` is deliberately NOT imported: `ev.ms` is a `dyn MemSource`, whose methods are
// inherent to the trait object. Importing the trait here would be an unused import, and silencing
// that warning with an `#[allow]` would hide the next real one.
use crate::registry::{Registry, Surface};

pub struct Sampler<'a> {
    pub reg: &'a Registry,
    pub ev: Evaluator<'a>,
    /// ⭐ THE EVENT PLANE'S INBOX. Reports come from the `d1-live` child in **batches at its exit**
    /// (see [`crate::child_mgr`]), so a tick either has new ones or does not; it never blocks on
    /// them. Drained into the frame and cleared, so a frame's `events` are the ones that arrived
    /// since the last frame and nothing else.
    pub inbox: Vec<crate::events::LiveReport>,
    /// What the armed slot claimed - the second derivation attribution is checked against.
    pub armed_surface: Option<String>,
    pub child: ChildStatus,
    pub events_dropped: u64,
    event_seq: u64,
    seq: u64,
    t0: Instant,
}

impl<'a> Sampler<'a> {
    pub fn new(reg: &'a Registry, ev: Evaluator<'a>) -> Self {
        Self {
            reg,
            ev,
            inbox: Vec::new(),
            armed_surface: None,
            child: ChildStatus::None,
            events_dropped: 0,
            event_seq: 0,
            seq: 0,
            t0: Instant::now(),
        }
    }

    pub fn tick(&mut self) -> Frame {
        let started = Instant::now();
        let generation = self.ev.ms.generation();
        let mut notes = Vec::new();

        // ⚠ POKE PROVENANCE, surfaced on every frame rather than buried in the registry: a value
        // read out of a field WE WROTE is not the client's value and must never be cited as one.
        if let crate::mem_source::SourceId::Snapshot { id, .. } = self.ev.ms.id() {
            if let Some(m) = self.reg.snapshot_meta(&id) {
                if m.poked {
                    notes.push(Note {
                        level: "loud".into(),
                        text: format!(
                            "⚠⚠ snapshot {id:?} is POKE-CONTAMINATED in {} field(s): {}. \
                             Values read there are OURS, not the client's.",
                            m.poked_fields.len(),
                            m.poked_fields.join(", ")
                        ),
                    });
                }
            }
        }

        let surfaces: Vec<SurfaceView> = self.reg.surfaces.iter().map(|s| self.sample(s)).collect();
        let edges = self.reg.edges.iter().map(edge_view).collect();

        // ⭐ EVENTS ARE ATTACHED AGAINST **THIS TICK'S** RESOLUTIONS, not against whatever the
        // address meant when the child armed. If the chain moved in between, the attribution says
        // so (`disagreement`) rather than silently pinning the event to a stale surface.
        let mut events = Vec::new();
        for rep in std::mem::take(&mut self.inbox) {
            let v = crate::events::to_event_views(
                &rep,
                &surfaces,
                self.armed_surface.as_deref(),
                self.event_seq,
            );
            self.event_seq += rep.hits.len() as u64;
            if rep.threads_failed.len() > 0 {
                notes.push(Note {
                    level: "warn".into(),
                    text: format!(
                        "⚠ the watch armed {} thread(s) and FAILED on {}: {:?}. An unarmed thread \
                         is a SILENT miss - absence is only evidence about the threads named here.",
                        rep.threads_armed,
                        rep.threads_failed.len(),
                        rep.threads_failed
                    ),
                });
            }
            if rep.hits.is_empty() {
                notes.push(Note {
                    level: "warn".into(),
                    text: format!(
                        "⚠ NO HIT in {:.1}s ({}). This is evidence ONLY about the {} thread(s) \
                         armed at start{}. It is NOT evidence that nothing writes {:#x}.",
                        rep.seconds,
                        rep.stop_reason,
                        rep.threads_armed,
                        if rep.follow_clone {
                            format!(" plus {} armed on clone", rep.clones_armed)
                        } else {
                            " (threads created later were NOT armed)".into()
                        },
                        rep.addr
                    ),
                });
            }
            if rep.unattributed_sigtraps > 0 || rep.foreign_stops > 0 {
                notes.push(Note {
                    level: "loud".into(),
                    text: format!(
                        "⚠ {} trap(s) were not ours and {} stop(s) belonged to tasks the child did \
                         not arm - something else is tracing this process, and notifications may \
                         have been stolen.",
                        rep.unattributed_sigtraps, rep.foreign_stops
                    ),
                });
            }
            for e in &v {
                if let Some(d) = &e.disagreement {
                    notes.push(Note { level: "loud".into(), text: d.clone() });
                }
            }
            events.extend(v);
        }

        if self.ev.ms.generation() != generation {
            // ⭐ The source moved beneath us mid-tick. The whole frame is discarded upstream; say so.
            notes.push(Note {
                level: "loud".into(),
                text: "the memory source's generation changed DURING this tick - discard it".into(),
            });
        }

        let f = Frame {
            seq: self.seq,
            ts_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            ts_mono_ms: self.t0.elapsed().as_millis() as u64,
            tick_ms: started.elapsed().as_secs_f32() * 1000.0,
            source: self.ev.ms.id(),
            generation,
            surfaces,
            edges,
            events,
            events_dropped: self.events_dropped,
            child: self.child.clone(),
            notes,
        };
        self.seq += 1;
        f
    }

    fn sample(&self, s: &Surface) -> SurfaceView {
        // 1. THE ROOT, EVERY TICK.
        let (resolved, trace) = self.resolve_surface(s);

        // 2. Liveness before values.
        let liveness = self.liveness(s);

        // 3. Fields.
        let fields = match (&resolved, liveness.verdict) {
            (Resolved::Addr(base), Verdict::Live) | (Resolved::Addr(base), Verdict::Unknown) => {
                s.fields.iter().map(|f| self.field(*base, f)).collect()
            }
            (Resolved::Addr(_), Verdict::NotLive) => s
                .fields
                .iter()
                .map(|f| FieldView {
                    name: f.name.clone(),
                    off: f.off,
                    ty: f.ty.clone(),
                    value: Value::NotMeasurable { liveness: liveness.expr.clone() },
                    note: f.note.clone(),
                })
                .collect(),
            _ => s
                .fields
                .iter()
                .map(|f| FieldView {
                    name: f.name.clone(),
                    off: f.off,
                    ty: f.ty.clone(),
                    value: Value::NotRead { why: "the surface address did not resolve".into() },
                    note: f.note.clone(),
                })
                .collect(),
        };

        SurfaceView {
            name: s.name.clone(),
            group: s.group.clone(),
            addr_expr: s.addr.clone(),
            resolved,
            resolve_trace: trace,
            liveness,
            fields,
            confidence: s.confidence.clone(),
            reach: s.reach.clone(),
        }
    }

    fn resolve_surface(&self, s: &Surface) -> (Resolved, Vec<String>) {
        if s.is_unknown_addr() {
            return (Resolved::Unknown, vec![]);
        }
        if s.is_not_expressible() {
            let Some(resolver) = s.resolver.clone() else {
                return (
                    Resolved::Unresolvable {
                        why: "addr is NOT-EXPRESSIBLE but no `resolver` is named".into(),
                    },
                    vec![],
                );
            };
            // Rust in, `a:0x…` out.
            let Some(input) = s.addr_input.clone() else {
                return (Resolved::NotExpressible { resolver }, vec![]);
            };
            let width = s.addr_input_width.as_deref().unwrap_or("u32");
            let r = self.ev.eval(&format!("{width}@{input}"));
            let trace = self.ev.last_trace.borrow().clone();
            return match (&resolver[..], r) {
                ("package_tag_registry", Reading::Ints(v)) if v.len() == 1 => {
                    match crate::resolvers::tag_resolve(&self.ev, v[0] as u64) {
                        Ok(a) => (Resolved::Addr(a), trace),
                        Err(Reading::Unmapped { addr, .. }) => (Resolved::Unmapped { addr }, trace),
                        Err(other) => {
                            (Resolved::Unresolvable { why: other.render() }, trace)
                        }
                    }
                }
                (_, Reading::Unmapped { addr, .. }) => (Resolved::Unmapped { addr }, trace),
                (r, other) => (
                    Resolved::Unresolvable {
                        why: format!("resolver {r:?} could not consume {}", other.render()),
                    },
                    trace,
                ),
            };
        }
        match self.ev.resolve_addr(&s.addr) {
            Ok(a) => (Resolved::Addr(a), self.ev.last_trace.borrow().clone()),
            Err(Reading::Unmapped { addr, .. }) => {
                (Resolved::Unmapped { addr }, self.ev.last_trace.borrow().clone())
            }
            Err(other) => (
                Resolved::Unresolvable { why: other.render() },
                self.ev.last_trace.borrow().clone(),
            ),
        }
    }

    fn field(&self, base: u64, f: &crate::registry::Field) -> FieldView {
        let at = base + f.off;
        let value = match parse_field_ty(&f.ty) {
            FieldTy::Scalar(Ty::Ptr) => match self.ev.read_typed(Ty::U64, 1, at) {
                Reading::Ints(v) => Value::Ptr(v[0] as u64),
                Reading::Unmapped { .. } => Value::Unmapped,
                other => Value::NotRead { why: other.render() },
            },
            FieldTy::Scalar(t) => match self.ev.read_typed(t, 1, at) {
                Reading::Ints(v) => Value::Int(v[0]),
                Reading::Floats(v) => Value::Floats(v),
                Reading::Unmapped { .. } => Value::Unmapped,
                other => Value::NotRead { why: other.render() },
            },
            FieldTy::Array(t, n) => match self.ev.read_typed(t, n, at) {
                Reading::Ints(v) => Value::Ints(v),
                Reading::Floats(v) => Value::Floats(v),
                Reading::Unmapped { .. } => Value::Unmapped,
                other => Value::NotRead { why: other.render() },
            },
            FieldTy::Ascii(max) => match self.ev.read_cstr(at, max) {
                Reading::Str(s) => Value::Ascii(s),
                Reading::Unmapped { .. } => Value::Unmapped,
                other => Value::NotRead { why: other.render() },
            },
            // ⚠ Aggregates are shown as a bounded WINDOW, explicitly labelled. Pretending to print
            // a 0xD8-stride 40-record array in a table row is how a reader stops noticing what is
            // and is not evidence.
            FieldTy::Aggregate(n) => match self.ev.ms.read_vec(at, n) {
                Ok(b) => Value::Bytes(b),
                Err(crate::mem_source::ReadErr::Unmapped { .. }) => Value::Unmapped,
                Err(e) => Value::NotRead { why: e.to_string() },
            },
            FieldTy::Opaque => Value::NotRead { why: format!("field type {:?} is not decodable", f.ty) },
        };
        FieldView { name: f.name.clone(), off: f.off, ty: f.ty.clone(), value, note: f.note.clone() }
    }

    // ── liveness ─────────────────────────────────────────────────────────────────────────────

    fn liveness(&self, s: &Surface) -> LivenessView {
        let expr = s.liveness.clone();
        let (verdict, why) = self.eval_predicate(&expr);
        LivenessView { expr, verdict, why }
    }

    /// A deliberately small predicate language: `A AND B`, and each term is
    /// `LHS [& MASK] (== | !=) RHS` · `LHS in A..=B` · `LHS reads "S"`.
    ///
    /// ⭐ Anything it does not understand is **`Unknown`**, never `Live` and never `NotLive`. A
    /// predicate evaluator that guesses is worse than one that abstains, because a wrong `Live`
    /// licenses reading a torn-down object as evidence.
    fn eval_predicate(&self, raw: &str) -> (Verdict, String) {
        let text = raw.split('#').next().unwrap_or("").trim();
        if text.is_empty() || text.starts_with("n/a") || text == "[U]" {
            return (Verdict::Unknown, format!("no evaluable predicate: {raw:?}"));
        }
        let mut whys = Vec::new();
        let mut any_unknown = false;
        for term in text.split(" AND ") {
            match self.eval_term(term.trim()) {
                (Verdict::NotLive, w) => return (Verdict::NotLive, w),
                (Verdict::Unknown, w) => {
                    any_unknown = true;
                    whys.push(w);
                }
                (Verdict::Live, w) => whys.push(w),
            }
        }
        (
            if any_unknown { Verdict::Unknown } else { Verdict::Live },
            whys.join(" AND "),
        )
    }

    fn eval_term(&self, term: &str) -> (Verdict, String) {
        let unknown = |t: &str| (Verdict::Unknown, format!("not evaluable: {t}"));

        if let Some((lhs, rhs)) = term.split_once(" reads ") {
            let want = rhs.trim().trim_matches('"');
            let (l, _) = crate::eval::unwrap_parens(lhs);
            let a = match self.ev.resolve_addr(l) {
                Ok(a) => a,
                Err(r) => return (Verdict::NotLive, r.render()),
            };
            return match self.ev.read_cstr(a, 64) {
                Reading::Str(got) if got == want => (Verdict::Live, format!("{l} reads {got:?}")),
                Reading::Str(got) => (Verdict::NotLive, format!("{l} reads {got:?}, wanted {want:?}")),
                other => (Verdict::NotLive, other.render()),
            };
        }

        if let Some((lhs, range)) = term.split_once(" in ") {
            let Some((lo, hi)) = range.trim().split_once("..=") else { return unknown(term) };
            let (Some(lo), Some(hi)) = (num(lo), num(hi)) else { return unknown(term) };
            let Some(v) = self.term_value(lhs) else { return unknown(term) };
            return if v >= lo && v <= hi {
                (Verdict::Live, format!("{lhs} = {v} in {lo}..={hi}"))
            } else {
                (Verdict::NotLive, format!("{lhs} = {v} outside {lo}..={hi}"))
            };
        }

        for (op, eq) in [("==", true), ("!=", false)] {
            if let Some((lhs, rhs)) = term.split_once(op) {
                // Optional `& MASK` on the left.
                let (lexpr, mask) = match lhs.rsplit_once('&') {
                    Some((l, m)) if num(m).is_some() => (l.to_string(), num(m)),
                    _ => (lhs.to_string(), None),
                };
                let Some(mut l) = self.term_value(&lexpr) else { return unknown(term) };
                if let Some(m) = mask {
                    l &= m;
                }
                let Some(r) = self.term_value(rhs) else { return unknown(term) };
                let ok = if eq { l == r } else { l != r };
                let sym = if eq { "==" } else { "!=" };
                return if ok {
                    (Verdict::Live, format!("{l:#x} {sym} {r:#x}"))
                } else {
                    (Verdict::NotLive, format!("{l:#x} {sym} {r:#x} is FALSE"))
                };
            }
        }
        unknown(term)
    }

    /// One side of a comparison: a literal, `v:`/`a:` address, or an expression evaluated through
    /// the shared grammar.
    fn term_value(&self, text: &str) -> Option<i128> {
        let (t, _) = crate::eval::unwrap_parens(text);
        let t = t.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(n) = num(t) {
            return Some(n);
        }
        if let Some(rest) = t.strip_prefix("v:") {
            return num(rest).map(|n| n + self.ev.base as i128);
        }
        if t.contains('*') || t.contains("$") {
            // `27*8`, `$D1_ACCOUNT_SOID` - the shared grammar has no multiplication and no env
            // interpolation. ⭐ ABSTAIN rather than approximate.
            return None;
        }
        match self.ev.eval(t) {
            Reading::Ints(v) if v.len() == 1 => Some(v[0]),
            Reading::Addr(a) => Some(a as i128),
            _ => None,
        }
    }
}

fn num(t: &str) -> Option<i128> {
    let t = t.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i128::from_str_radix(h, 16).ok()?
    } else {
        t.parse::<i128>().ok()?
    };
    Some(if neg { -v } else { v })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⛔ BUILD 1 renders an edge's state **as declared in the registry**, and says so in `why`.
/// The predicates in `green`/`red` name server-side facts, `forall`/`exists` quantifiers and
/// symbolic operands (`blob`, `ac`, `gg`); evaluating them is BUILD 3. Rendering a *declared*
/// state as if it had been *measured this tick* would be the exact failure this project keeps
/// paying for, so the provenance is in the string.
fn edge_view(e: &crate::registry::Edge) -> EdgeView {
    EdgeView {
        id: e.id.clone(),
        up: e.up.clone(),
        down: e.down.clone(),
        mech: e.mech.clone(),
        state: EdgeState::parse(&e.state).unwrap_or(EdgeState::Unknown),
        why: format!(
            "DECLARED in registry.toml (as_of {}), not evaluated this tick - BUILD 1 has no edge \
             predicate evaluator",
            e.as_of
        ),
        guard: e.guard.clone(),
        guard_active: e.guard.is_some(),
        evaluable: e.evaluable_offline,
        as_of: e.as_of.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub enum FieldTy {
    Scalar(Ty),
    Array(Ty, usize),
    Ascii(usize),
    Aggregate(usize),
    Opaque,
}

/// Registry field types are prose-ish (`u8[40*0xD8]`, `i64 selfrel`, `struct[]`). Decode the ones
/// with an unambiguous machine meaning and mark the rest **Opaque** rather than guessing.
pub fn parse_field_ty(ty: &str) -> FieldTy {
    let t = ty.trim();
    if t == "[U]" {
        return FieldTy::Opaque;
    }
    if let Some(rest) = t.strip_prefix("char[") {
        let n = parse_len(rest.trim_end_matches(']')).unwrap_or(64);
        return FieldTy::Ascii(n.min(256));
    }
    if t == "char[]" {
        return FieldTy::Ascii(64);
    }
    if let Some(base) = Ty::parse(t) {
        return FieldTy::Scalar(base);
    }
    if let Some((base, rest)) = t.split_once('[') {
        let inner = rest.trim_end_matches(']');
        if let Some(bt) = Ty::parse(base) {
            match parse_len(inner) {
                Some(n) if bt == Ty::U8 => return FieldTy::Aggregate(n.min(64)),
                Some(n) => return FieldTy::Array(bt, n.min(16)),
                None => return FieldTy::Opaque, // `u32[]`, `u8[]` - unknown extent
            }
        }
        if base == "ptr" {
            return match parse_len(inner) {
                Some(n) => FieldTy::Array(Ty::U64, n.min(16)),
                None => FieldTy::Opaque,
            };
        }
    }
    // `struct`, `struct[]`, `struct[257]`, `i64 selfrel`, …
    FieldTy::Opaque
}

/// ⚠ `pub` since BUILD 2 so that [`crate::events::declared_extent`] can compute an **unclamped**
/// byte extent for event attribution. [`parse_field_ty`] deliberately clamps array lengths for
/// *display*; attribution must not inherit that clamp, or an event landing at field byte 300 of a
/// 512-byte array would be reported as belonging to no surface at all.
pub fn parse_len(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains('*') {
        // `40*0xD8` - a real extent, but not one to splat into a table cell.
        return None;
    }
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        usize::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}
