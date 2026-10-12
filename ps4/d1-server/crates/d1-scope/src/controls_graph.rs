// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! BUILD 4's controls - **#16 and #17**, the flow graph.
//!
//! ⭐ Both were **inverted once**: broken on purpose, watched to fail, restored. #17's inversion is
//! *inline* - it pans the canvas off its own world box and asserts the clip detector FIRES, because
//! a counter that can only ever read zero is decoration.
//!
//! | control | the axis it protects | how it is inverted |
//! |---|---|---|
//! | **#16** | *the model: `graph.toml` and `registry.toml` cannot drift; the five colours are derived by ONE ordered rule; `grey` beats every downstream question; `retired` is never red; a `guard` may only SUPPRESS; a POKED witness is never a colour; and the same edge reads differently across two snapshots FOR A STATED REASON* | a node is deleted from a clone of the spec and the consistency check must complain; the derivation is driven over **every** state pair and each rule has a case that must FAIL it |
//! | **#17** | *the drawing: everything the model declares is inside the world box, nothing is silently clipped at the default fit, and every node/edge/badge has its OWN hit target* | the canvas is panned 4000 px and `nodes_clipped` must go **non-zero** - proving the detector is not stuck at 0 |
//!
//! ⛔ #17's render half needs `--features gui`. Without it the control runs its data half and
//! **says so, loudly, in its own name** - it is not skipped into a pass.

use crate::controls::{ControlResult, Snaps};
use crate::frame::EdgeState;
use crate::graph::{derive, GraphModel, GraphParams, GraphSpec, Hit, NodeState, P2, RectF};
use crate::registry::Registry;
use crate::sampler::Sampler;

/// ⭐ The two snapshots that make the poke visible: one PRISTINE, one POKED, same edge.
const PRISTINE: &str = "rung34-task8-cleared-registry";
const POKED: &str = "rung36-biped-chain";

fn states() -> Vec<(&'static str, NodeState)> {
    vec![
        ("POPULATED", NodeState::Populated),
        ("EMPTY", NodeState::Empty),
        ("POKED", NodeState::Poked { field: "q+0x10".into(), snapshot: "s".into() }),
        ("UNREADABLE", NodeState::Unreadable { why: "UNMAPPED".into() }),
        ("NO-WITNESS", NodeState::NoWitness { why: "declined".into() }),
        ("NOT-MEASURABLE", NodeState::NotMeasurable { why: "our server".into() }),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #16 - the model
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c16_graph_model(reg: &Registry, snaps: &Snaps) -> ControlResult {
    let mut r = ControlResult::new("#16", "the flow graph: one ordered derivation, and two files that cannot drift");

    let spec = match GraphSpec::open_default() {
        Ok(s) => s,
        Err(e) => {
            r.bad(format!("opening graph.toml: {e:#}"));
            return r;
        }
    };

    // ── (a) the two files agree, and the checker can find a real fault ──────────────────────────
    let complaints = spec.consistency(reg);
    r.check(
        complaints.is_empty(),
        format!("graph.toml ⟷ registry.toml: {} node(s), {} edge(s), 0 complaints; got {complaints:?}", spec.nodes.len(), reg.edges.len()),
    );
    // ⭐ THE CHECKER'S OWN POSITIVE CONTROL. "No complaints" from a broken checker reads exactly
    //   like two files that agree - the failure mode this project has shipped twice.
    let mut broken = spec.clone();
    let dropped = broken.nodes.iter().position(|n| n.id == "sim_player_set");
    if let Some(i) = dropped {
        broken.nodes.remove(i);
    }
    let c2 = broken.consistency(reg);
    r.check(
        c2.iter().any(|c| c.contains("sim_player_set") && c.contains("NO node")),
        format!("consistency positive control: deleting a node IS caught ({} complaint(s))", c2.len()),
    );
    // …and a witness that names a field nobody declares.
    let mut broken2 = spec.clone();
    if let Some(n) = broken2.nodes.iter_mut().find(|n| n.id == "session_player_set") {
        n.witness = Some("no_such_field".into());
    }
    r.check(
        broken2.consistency(reg).iter().any(|c| c.contains("no_such_field")),
        "consistency positive control: a witness naming a non-existent FIELD is caught - otherwise it would read UNKNOWN forever",
    );

    // ── (b) THE DERIVATION, over every pair ────────────────────────────────────────────────────
    let ss = states();
    let mut seen: Vec<EdgeState> = Vec::new();
    let mut accusations = 0usize;
    for (un, us) in &ss {
        for (dn, ds) in &ss {
            let d = derive(EdgeState::Green, us, ds, None);
            seen.push(d.color);
            if d.color == EdgeState::Red {
                accusations += 1;
                // ⛔ EXACTLY TWO PAIRS MAY ACCUSE, and both require the DOWNSTREAM to have been
                //    READ and found empty:
                //      • POPULATED → EMPTY      - both ends measured; the strong form;
                //      • NOT-MEASURABLE → EMPTY - our own server upstream, ASSERTED not measured,
                //        with the client's end read and empty. The caveat travels on the edge and
                //        the wire is drawn dashed, but the accusation is real: we say we sent it
                //        and the client's end is zero. (E-09 is exactly this.)
                //    If any OTHER pair can go red, one break is being charged to edges that never
                //    ran - the failure this whole ordering exists to prevent.
                r.check(
                    *dn == "EMPTY" && (*un == "POPULATED" || *un == "NOT-MEASURABLE"),
                    format!("RED only into a downstream READ AS EMPTY: got {un} -> {dn}"),
                );
                r.check(
                    d.upstream_measured == (*un == "POPULATED"),
                    format!("…and {un} -> {dn} declares upstream_measured = {}", d.upstream_measured),
                );
            }
            if *un == "EMPTY" && *dn != "POKED" {
                r.check(
                    d.color == EdgeState::Grey,
                    format!("⛔ an unpopulated UPSTREAM is GREY whatever the downstream reads: {un} -> {dn} = {:?}", d.color),
                );
            }
            if *un == "POKED" || *dn == "POKED" {
                r.check(
                    d.color == EdgeState::Unknown,
                    format!("a POKED end is never a colour: {un} -> {dn} = {:?}", d.color),
                );
            }
        }
    }
    r.check(
        derive(EdgeState::Green, &NodeState::Populated, &NodeState::Empty, None).color == EdgeState::Red,
        format!("populated -> empty accuses ({accusations} accusing pair(s) in the whole table)"),
    );
    // ⭐ Every colour but `retired` must be REACHABLE from the state pairs - an unreachable arm is
    //   a colour the view can never show, i.e. a state the reader will never learn to recognise.
    for want in [EdgeState::Green, EdgeState::Red, EdgeState::Grey, EdgeState::Unknown] {
        r.check(seen.contains(&want), format!("{want:?} is reachable from some pair of node states"));
    }
    r.check(
        !seen.contains(&EdgeState::Retired),
        "…and RETIRED is NOT reachable from a reading - it comes only from the registry (the negative direction)",
    );
    // retired, and the guard
    r.check(
        derive(EdgeState::Retired, &NodeState::Populated, &NodeState::Empty, None).color == EdgeState::Retired,
        "⛔ a RETIRED edge stays retired even when its ends would accuse",
    );
    let g = derive(EdgeState::Red, &NodeState::Populated, &NodeState::Empty, Some("orbit gives a FALSE RED"));
    r.check(
        g.color == EdgeState::Unknown && g.accusation_suppressed.is_some(),
        "a registry guard SUPPRESSES an accusation and names itself",
    );
    r.check(
        derive(EdgeState::Green, &NodeState::Populated, &NodeState::Populated, Some("x")).color == EdgeState::Green,
        "…and a guard touches NOTHING else (the negative direction - otherwise 'guarded' means 'ignored')",
    );

    // ── (c) THE SAME EDGE, TWO SNAPSHOTS, ONE STATED REASON ─────────────────────────────────────
    let p = GraphParams::default();
    let mut models = Vec::new();
    for id in [PRISTINE, POKED] {
        match snaps.ev(id, reg) {
            Ok(ev) => {
                let f = Sampler::new(reg, ev).tick();
                models.push((id, GraphModel::build(&f, reg, &spec, &p)));
            }
            Err(e) => r.bad(format!("{id}: {e:#}")),
        }
    }
    if models.len() == 2 {
        let (_, mp) = &models[0];
        let (_, mk) = &models[1];
        let e15p = mp.edges.iter().find(|e| e.id == "E-15");
        let e15k = mk.edges.iter().find(|e| e.id == "E-15");
        match (e15p, e15k) {
            (Some(a), Some(b)) => {
                // ⭐⭐ THE ONE THE VIEW EXISTS FOR.
                r.check(
                    a.color == EdgeState::Red,
                    format!("⭐⭐ on the PRISTINE snapshot {PRISTINE}, E-15 session_player_set -> sim_player_set is {:?}", a.color),
                );
                let w = mp.nodes.iter().find(|n| n.id == "sim_player_set").map(|n| n.witness_text.clone()).unwrap_or_default();
                r.check(
                    w.contains("+0x10") && w.contains("sweep_mask"),
                    format!("…and it names the field that decided it: {w:?}"),
                );
                r.check(
                    mp.headline.as_deref().unwrap_or("").contains("E-15"),
                    "…and E-15 is in the HEADLINE - the one line the view exists to make obvious",
                );
                // ⛔ THE OTHER DIRECTION, and it is the honest one: on the POKED snapshot the same
                //    edge must NOT be a colour, and must SAY why.
                r.check(
                    b.color == EdgeState::Unknown && b.why.contains("POKED"),
                    format!("⛔ on the POKED snapshot {POKED}, the same edge is {:?} and names the poke", b.color),
                );
                r.check(
                    mk.notes.iter().any(|n| n.contains("PRISTINE") && n.contains(PRISTINE)),
                    "…and the model names a PRISTINE snapshot to re-read it on - a dead end with a route out",
                );
                r.check(
                    !mk.notes.iter().any(|n| n.contains("--source snap:rung36")),
                    "…and it does NOT offer the poked snapshot as the route (the negative direction)",
                );
            }
            _ => r.bad("E-15 is missing from one of the two models"),
        }
        // ⚠ TWO SIMULTANEOUS REDS must be showable - E-09 and E-15 are independent and both open.
        r.check(
            mp.accusations.len() >= 2,
            format!("{} simultaneous accusation(s) on {PRISTINE}: {:?}", mp.accusations.len(), mp.accusations),
        );
        r.check(
            mp.headline.as_deref().unwrap_or("").contains("SIMULTANEOUS"),
            "…and the headline says they are INDEPENDENT - fixing either alone yields nothing visible",
        );

        // ── (d) nothing is silently dropped, and the world box contains the drawing ─────────────
        for (id, m) in &models {
            r.check(
                m.edges.len() == reg.edges.len(),
                format!("{id}: all {} registry edges are in the model ({} drawn)", reg.edges.len(), m.edges.len()),
            );
            r.check(
                m.nodes.len() == spec.nodes.len(),
                format!("{id}: all {} spec nodes are in the model", spec.nodes.len()),
            );
            let escapes = geometry_escapes(m);
            r.check(
                escapes.is_empty(),
                format!(
                    "{id}: ⛔ every node, label, badge and CURVE SAMPLE lies inside the world box - \
                     anything outside would be silently clipped by the painter and read as 'it \
                     failed to render'. Escapes: {escapes:?}"
                ),
            );
        }
        // ⭐ The escape detector's own inversion: shrink the world and it must fire.
        let mut shrunk = models[0].1.clone();
        shrunk.world = RectF { x0: 0.0, y0: 0.0, x1: 10.0, y1: 10.0 };
        r.check(
            !geometry_escapes(&shrunk).is_empty(),
            "escape-detector positive control: a world box that does not contain the drawing IS caught",
        );

        // ── (e) purity, layout stability, and real dependence on the frame ─────────────────────
        let a = serde_json::to_string(&models[0].1).unwrap_or_default();
        // PURITY: `build` is a function of `(Frame, Registry, GraphSpec, GraphParams)` and nothing
        // else. Two builds of ONE frame are byte-identical - a build that reached for a clock or
        // the filesystem mid-render would break this, quietly.
        match snaps.ev(PRISTINE, reg) {
            Ok(ev) => {
                let f = Sampler::new(reg, ev).tick();
                let x = serde_json::to_string(&GraphModel::build(&f, reg, &spec, &p)).unwrap_or_default();
                let y = serde_json::to_string(&GraphModel::build(&f, reg, &spec, &p)).unwrap_or_default();
                r.check(!x.is_empty() && x == y, "GraphModel::build is PURE: two builds of one frame are byte-identical");
                // ⚠ AND THE LAYOUT IS STABLE ACROSS TICKS, which purity alone does NOT give: a
                //   fresh tick carries a new `tick_ms`, so the whole documents differ by
                //   construction. What must not move is the GEOMETRY - a layout that shifted
                //   between two runs could not be compared between two runs, and comparing two
                //   runs is the job.
                let m2 = GraphModel::build(&f, reg, &spec, &p);
                r.check(
                    geometry_signature(&models[0].1) == geometry_signature(&m2),
                    "…and the LAYOUT is identical across two independent ticks (in-column order is by edge count, then id - nothing that moves)",
                );
                r.check(
                    models[0].1.tick_ms != m2.tick_ms || models[0].1.tick_ms == 0.0,
                    "…while the documents themselves DO differ tick to tick (so the geometry check is not comparing two identical strings)",
                );
            }
            Err(e) => r.bad(format!("{PRISTINE}: {e:#}")),
        }
        let c = serde_json::to_string(&models[1].1).unwrap_or_default();
        r.check(a != c, "…and it really depends on the frame (a different snapshot gives a different model)");
        r.check(
            geometry_signature(&models[0].1) == geometry_signature(&models[1].1),
            "⭐ …and the layout is the SAME on both snapshots - the FIXED layout the view promises, not a force-directed one that redraws itself when the data moves",
        );

        // ── (f) present parity: every string the canvas can draw is in --json ───────────────────
        let m = &models[0].1;
        let mut missing = Vec::new();
        for n in &m.nodes {
            for s in [&n.label, &n.witness_text, &n.addr] {
                if !s.is_empty() && !a.contains(&serde_json::to_string(s).unwrap_or_default().trim_matches('"').to_string()) {
                    missing.push(format!("node {}: {s:?}", n.id));
                }
            }
        }
        for e in &m.edges {
            if !e.badge_text.is_empty() && !a.contains(&e.badge_text) {
                missing.push(format!("edge {}: badge {:?}", e.id, e.badge_text));
            }
        }
        r.check(missing.is_empty(), format!("every drawable string is carried by --json; missing {missing:?}"));
        r.check(!a.contains("CANVAS-ONLY-STRING"), "parity scanner positive control: a string no node holds is absent");
        let text = m.to_text();
        r.check(text.contains("E-15") && text.contains("RED"), "…and `--headless` (no --json) leads with the same accusation");

        // ── (g) hit testing agrees with the geometry ────────────────────────────────────────────
        let i15 = m.edges.iter().position(|e| e.id == "E-15").unwrap_or(0);
        let lc = m.edges[i15].label_rect.center();
        r.check(
            m.hit(lc) == Some(Hit::Edge(i15)),
            format!("clicking E-15's label selects E-15 (got {:?})", m.hit(lc)),
        );
        let nc = m.nodes.iter().position(|n| n.id == "sim_player_set").unwrap_or(0);
        r.check(
            m.hit(m.nodes[nc].rect.center()) == Some(Hit::Node(nc)),
            "clicking a node's box selects that node",
        );
        // ⭐ THE FINDS-NOTHING HALF, next to the finds-something half.
        r.check(
            m.hit(P2 { x: -500.0, y: -500.0 }).is_none(),
            "a point outside every target selects NOTHING (the negative direction)",
        );
    }

    // ── (h) THE EVENT BADGE ON AN EDGE - `insn @ addr`, or the FINDING that it never fired ──────
    match model_with_events(reg, &spec, snaps, crate::controls_event::fixture_path()) {
        Ok(m) => {
            let e = m.edges.iter().find(|e| e.id == "E-15");
            match e {
                Some(e) => {
                    r.check(
                        e.writers.len() == 2,
                        format!("the recorded two-hit document lands on E-15's DOWNSTREAM by address ({} writer(s))", e.writers.len()),
                    );
                    match &e.last_writer {
                        Some(w) => {
                            // ⭐ `insn @ addr` - the badge line.
                            r.check(
                                w.badge_line().contains('@') && w.badge_line().contains("mov"),
                                format!("the badge shows the LAST WRITER as `insn @ addr`: {:?}", w.badge_line()),
                            );
                            // ⛔ rip and writer are TWO FIELDS and are never collapsed.
                            r.check(
                                w.rip != w.addr,
                                format!("⛔ the raw RIP ({:#x}) and the decoded store ({:#x}) are DISTINCT", w.rip, w.addr),
                            );
                            r.check(
                                w.caveat().contains("AFTER the store") && w.caveat().contains(&format!("{:#x}", w.rip)),
                                "⚠⚠ …and the backward-decode caveat is INLINE, carrying the raw RIP",
                            );
                            r.check(
                                !w.quotable && w.caveat().contains("NOT QUOTABLE"),
                                format!("an AMBIGUOUS decode says so rather than being quoted: {:?}", w.confidence),
                            );
                        }
                        None => r.bad("E-15 carries no last_writer after a two-hit report"),
                    }
                    r.check(e.badge_rect.is_some(), "…and the badge has a hit rectangle of its own, so it can be clicked");
                    // The negative direction, in the same model: an untouched edge has no badge.
                    let clean = m.edges.iter().find(|x| x.id == "E-19");
                    r.check(
                        clean.map(|x| x.badge_text.is_empty() && x.writers.is_empty()).unwrap_or(false),
                        "an edge no write landed in carries NO badge (the finds-nothing half)",
                    );
                }
                None => r.bad("E-15 is missing from the event model"),
            }
        }
        Err(e) => r.bad(format!("building the event model: {e}")),
    }
    // ⛔ AND THE OTHER ZERO: a completed watch that saw nothing is a REAL NEGATIVE, and it must not
    //   share a statement with "a writer fired".
    match model_with_events(reg, &spec, snaps, crate::controls_event::fixture_nohit()) {
        Ok(m) => match m.edges.iter().find(|e| e.id == "E-15") {
            Some(e) => {
                r.check(
                    e.last_writer.is_none(),
                    "a zero-hit document leaves E-15 with NO writer to name",
                );
                r.check(
                    e.badge_text.contains("NEVER FIRED"),
                    format!("…and the badge is the FINDING, not a spinner: {:?}", e.badge_text),
                );
                r.check(
                    e.badge.detail().contains("thread"),
                    "…and its detail carries the BOUND of the negative - which threads, how long",
                );
            }
            None => r.bad("E-15 is missing from the no-hit model"),
        },
        Err(e) => r.bad(format!("building the no-hit model: {e}")),
    }

    r
}

/// ⭐ A model whose EVENT plane is real: a recorded `d1-live --json` document, replayed, with the
/// downstream surface of E-15 moved onto the report's own address so attribution lands **by
/// address** through the real pipeline. ⛔ Not a hand-made `EventView` - a control that built its
/// own event would keep passing after the attribution changed shape.
fn model_with_events(
    reg: &Registry,
    spec: &GraphSpec,
    snaps: &Snaps,
    fixture: std::path::PathBuf,
) -> Result<GraphModel, String> {
    let text = std::fs::read_to_string(&fixture).map_err(|e| format!("{}: {e}", fixture.display()))?;
    let (rep, missing) = crate::events::parse_report(&text)?;
    if !missing.is_empty() {
        return Err(format!("the recorded report is missing key(s): {missing:?}"));
    }
    let ev = snaps.ev(PRISTINE, reg).map_err(|e| format!("{e:#}"))?;
    let mut f = Sampler::new(reg, ev).tick();
    // ⚠ Only the ADDRESS moves. Every field VALUE stays exactly as the snapshot read it, so the
    //   node's reading (and therefore the edge's colour) is untouched by this rehoming.
    if let Some(s) = f.surfaces.iter_mut().find(|s| s.name == "sim_player_set") {
        s.resolved = crate::frame::Resolved::Addr(rep.addr);
    }
    f.events = crate::events::to_event_views(&rep, &f.surfaces, Some("sim_player_set"), 0);
    f.child = crate::events::replayed_child_status(&rep, Some("sim_player_set".into()));
    Ok(GraphModel::build(&f, reg, spec, &GraphParams::default()))
}

/// ⭐ Just the GEOMETRY - every node id, its rect, and every edge's four control points. This is
/// what "the layout is FIXED" means, and it is deliberately separate from the whole document: the
/// document carries `tick_ms`, which differs between two ticks by construction.
fn geometry_signature(m: &GraphModel) -> String {
    let mut s = String::new();
    for n in &m.nodes {
        s.push_str(&format!(
            "{}|{}|{}|{:.2},{:.2},{:.2},{:.2}\n",
            n.id, n.col_index, n.row_index, n.rect.x0, n.rect.y0, n.rect.x1, n.rect.y1
        ));
    }
    for e in &m.edges {
        s.push_str(&format!(
            "{}|{:.2},{:.2}|{:.2},{:.2}|{:.2},{:.2}|{:.2},{:.2}\n",
            e.id, e.from.x, e.from.y, e.c1.x, e.c1.y, e.c2.x, e.c2.y, e.to.x, e.to.y
        ));
    }
    s
}

/// Every rectangle and every sampled curve point the canvas will emit, checked against the model's
/// own world box. ⛔ The painter is **pre-clipped**: anything outside is dropped in silence.
fn geometry_escapes(m: &GraphModel) -> Vec<String> {
    let mut out = Vec::new();
    for n in &m.nodes {
        if !m.world.contains_rect(&n.rect) {
            out.push(format!("node {}", n.id));
        }
    }
    for e in &m.edges {
        if !m.world.contains_rect(&e.label_rect) {
            out.push(format!("{} label", e.id));
        }
        if let Some(b) = &e.badge_rect {
            if !m.world.contains_rect(b) {
                out.push(format!("{} badge", e.id));
            }
        }
        for i in 0..=28 {
            let p = crate::graph::cubic_at(e.from, e.c1, e.c2, e.to, i as f32 / 28.0);
            if !m.world.contains(p) {
                out.push(format!("{} curve@t={:.2}", e.id, i as f32 / 28.0));
                break;
            }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #17 - the drawing
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c17_graph_render(reg: &Registry, snaps: &Snaps) -> ControlResult {
    let name: &'static str = if cfg!(feature = "gui") {
        "the canvas draws every node, clips none, and gives each one its own hit target"
    } else {
        "the canvas - DATA HALF ONLY (the render half needs --features gui)"
    };
    let mut r = ControlResult::new("#17", name);
    if !cfg!(feature = "gui") {
        r.info(
            "    · ⚠ THE RENDER HALF DID NOT RUN. This binary has no window, so nothing here has \
             emitted a shape. Rebuild with `--features gui` to run the painter leg - that is the \
             leg that catches a silently-clipped node, and it is NOT skipped into a pass.",
        );
    }

    let spec = match GraphSpec::open_default() {
        Ok(s) => s,
        Err(e) => {
            r.bad(format!("opening graph.toml: {e:#}"));
            return r;
        }
    };
    let p = GraphParams::default();
    let m = match snaps.ev(PRISTINE, reg) {
        Ok(ev) => {
            let f = Sampler::new(reg, ev).tick();
            GraphModel::build(&f, reg, &spec, &p)
        }
        Err(e) => {
            r.bad(format!("{PRISTINE}: {e:#}"));
            return r;
        }
    };

    // The data half: the colour tables. ⛔ Two facts a reader must tell apart do not share a swatch.
    #[cfg(feature = "gui")]
    {
        use std::collections::BTreeSet;
        let ec: BTreeSet<[u8; 4]> = [EdgeState::Green, EdgeState::Red, EdgeState::Grey, EdgeState::Unknown, EdgeState::Retired]
            .into_iter()
            .map(|s| crate::ui_graph::edge_colour(s).to_array())
            .collect();
        r.check(ec.len() == 5, format!("five EDGE colours, {} distinct", ec.len()));
        let nc: BTreeSet<[u8; 4]> = states()
            .iter()
            .map(|(_, s)| crate::ui_graph::node_colour(s).to_array())
            .collect();
        r.check(nc.len() == 6, format!("six NODE colours, {} distinct", nc.len()));
    }

    // ⭐ A model whose BADGES are real, so the badge-drawing path is exercised rather than assumed.
    let badged = model_with_events(reg, &spec, snaps, crate::controls_event::fixture_path()).ok();

    #[cfg(feature = "gui")]
    render_half(&mut r, &m, badged.as_ref());
    #[cfg(not(feature = "gui"))]
    let _ = &badged;
    #[cfg(not(feature = "gui"))]
    {
        // Without a window the model half still asserts what the canvas is asked to draw.
        r.check(!m.nodes.is_empty() && !m.edges.is_empty(), "the model to be drawn is non-empty");
    }
    r
}

#[cfg(feature = "gui")]
fn render_half(r: &mut ControlResult, m: &GraphModel, badged: Option<&GraphModel>) {
    use crate::ui_graph::{paint_graph, PaintReceipt, ViewState};

    fn run(m: &GraphModel, w: f32, h: f32, pan: egui::Vec2) -> PaintReceipt {
        let model = m.clone();
        let out = std::sync::Arc::new(std::sync::Mutex::new(PaintReceipt::default()));
        let sink = out.clone();
        let mut view = ViewState::default();
        view.pan = pan;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(w, h))
            .build_ui(move |ui| {
                let rec = paint_graph(ui, &model, &mut view);
                if let Ok(mut g) = sink.lock() {
                    *g = rec;
                }
            });
        harness.run();
        let v = out.lock().map(|g| *g).unwrap_or_default();
        v
    }

    // ── the default fit ─────────────────────────────────────────────────────────────────────────
    let a = run(m, 1800.0, 1000.0, egui::Vec2::ZERO);
    r.check(a.nodes_total == m.nodes.len(), format!("the canvas was handed all {} nodes", a.nodes_total));
    r.check(
        a.nodes_clipped == 0 && a.edges_clipped == 0,
        format!("⭐ at the default fit NOTHING is clipped: {}", a.line()),
    );
    // ⭐ Every node, every edge label and every badge has its OWN hit target: the single canvas
    //   Response can never say WHICH one was clicked.
    let want = m.nodes.len() + m.edges.len() + m.edges.iter().filter(|e| e.badge_rect.is_some()).count();
    r.check(
        a.hit_targets == want,
        format!("{} hit target(s) = {} nodes + {} edges + {} badges", a.hit_targets, m.nodes.len(), m.edges.len(), want - m.nodes.len() - m.edges.len()),
    );
    r.check(a.scale > 0.0 && a.canvas_w > 100.0, format!("render positive control: a real canvas was allocated ({:.0}×{:.0}, scale {:.2})", a.canvas_w, a.canvas_h, a.scale));

    // ── ⭐ THE DISCRIMINATING LEG: a SMALL pan clips some nodes only PARTIALLY ──────────────────
    //    ⛔ THIS LEG EXISTS BECAUSE THE INVERSION EXPOSED THE OTHER TWO AS TOO WEAK. Swapping the
    //    detector from `contains_rect` to `intersects` - i.e. "count a node as visible if ANY pixel
    //    of it is on screen", which silently hides every half-drawn node - left both the
    //    default-fit leg and the 4000 px leg PASSING. At 300 px the two predicates give different
    //    answers: several nodes straddle the right edge, so `contains_rect` reports them and
    //    `intersects` reports zero. *A control that only ever passes is decoration, and so is an
    //    inversion that the control survives.*
    let c = run(m, 1800.0, 1000.0, egui::vec2(300.0, 0.0));
    r.check(
        c.nodes_clipped > 0 && c.nodes_clipped < c.nodes_total,
        format!(
            "⭐ a 300 px pan clips {} of {} nodes PARTIALLY - the leg that tells 'fully on screen' \
             from 'merely touching the edge'",
            c.nodes_clipped, c.nodes_total
        ),
    );

    // ── ⛔ THE INVERSION, INLINE: pan the world off the canvas and the detector MUST fire ───────
    //    Without this, `nodes_clipped == 0` above could simply mean the counter never increments -
    //    which is exactly what a broken scan looks like from the outside.
    let b = run(m, 1800.0, 1000.0, egui::vec2(4000.0, 4000.0));
    r.check(
        b.nodes_clipped > 0,
        format!("⛔ clip-detector positive control: panning 4000 px reports {} node(s) off-canvas", b.nodes_clipped),
    );
    r.check(
        b.nodes_total == a.nodes_total,
        "…and the TOTAL is unchanged - the denominator is what makes 'off-canvas' distinguishable from 'never drawn'",
    );

    // ── the badge path really draws ─────────────────────────────────────────────────────────────
    // ⚠ Without this the badge branch is dead code in every control run: the pristine snapshot has
    //   no event plane at all, so `badges_drawn` would be 0 forever and nobody would notice.
    match badged {
        Some(bm) => {
            let want = bm.edges.iter().filter(|e| e.badge_rect.is_some()).count();
            let d = run(bm, 1800.0, 1000.0, egui::Vec2::ZERO);
            r.check(
                want > 0 && d.badges_drawn == want,
                format!("the canvas drew {} event badge(s) - the model declares {want}", d.badges_drawn),
            );
            r.check(
                d.hit_targets == bm.nodes.len() + bm.edges.len() + want,
                format!("…and each badge has its OWN hit target ({} total)", d.hit_targets),
            );
            r.check(
                d.badges_drawn > a.badges_drawn,
                format!("…and the un-armed model really drew FEWER ({}) - the counter is not a constant", a.badges_drawn),
            );
        }
        None => r.bad("no event model was available - the badge-drawing path was never exercised"),
    }
}

pub fn run_graph_controls(reg: &Registry, snaps: &Snaps) -> Vec<ControlResult> {
    vec![c16_graph_model(reg, snaps), c17_graph_render(reg, snaps)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ The project's rule: every control is a `#[test]` **and** a `controls` subcommand that
    /// exits 2. This is the first half; `d1-scope controls` is the second.
    #[test]
    fn build_4_controls_pass() {
        let reg = Registry::open_default().expect("registry");
        let snaps = Snaps::load(crate::controls::REQUIRED_SNAPS).expect("snapshots");
        for c in run_graph_controls(&reg, &snaps) {
            assert!(c.pass, "{} {}\n{}", c.id, c.name, c.lines.join("\n"));
        }
    }
}
