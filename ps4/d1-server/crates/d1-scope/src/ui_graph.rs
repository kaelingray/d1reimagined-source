// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐⭐ **THE FLOW GRAPH, DRAWN.** A hand-made node/edge canvas on [`egui::Painter`], pinned
//! `=0.36.1`, glow only. ⛔ **No graph crate.** None of `egui_plot`/`egui_tiles`/`egui_dock`/any
//! layout crate is in a lock file in this tree or in this box's offline registry cache, so "just
//! add the dep" is a build failure here, not a convenience.
//!
//! # ⛔ WHAT THIS MODULE MAY NOT DO - the same boundary [`crate::ui`] has, asserted the same way
//!
//! ⚠ **AND NOTE WHAT THIS PROSE MAY NOT SAY.** Control #14 scans this file for the *names* of the
//! memory-source and child-manager types with no exclusion list, so the paragraphs below describe
//! those names rather than spelling them.
//!
//! 1. **It never reads memory.** No read-source type, no `/proc`, no snapshot, and - unlike its
//!    sibling - not a single filesystem read of any kind. The sampler runs on its own thread and
//!    hands over an `Arc<Frame>`.
//! 2. **It never spawns a child and never signals one.** The canvas arms nothing. The one
//!    unrecoverable mistake available on this box is on the operator's live client, and a button is
//!    exactly how it gets made by accident.
//! 3. **It computes no value, and no COLOUR DECISION.** Every edge's colour, every node's reading,
//!    every rectangle's coordinates and every string come from [`crate::graph::GraphModel`]. This
//!    file maps `EdgeState` → `Color32` and draws; that mapping is the whole of its authority.
//!
//! # ⭐ THE TRAP THIS VIEW IS BUILT AROUND
//!
//! [`egui::Ui::allocate_painter`] returns a painter **pre-clipped to its response rect**, so
//! anything drawn outside is silently dropped - and *"my node failed to render"* is then
//! indistinguishable from *"my node is off-canvas"*. That is the UNMAPPED-is-not-zero rail in
//! pixels. ⇒ this canvas **always** draws its own border and **always** prints
//! `N/M nodes · N/M edges on screen`, computed by comparing each transformed rect against
//! [`egui::Painter::clip_rect`] - see [`PaintReceipt`]. Control #17 drives [`paint_graph`]
//! headlessly and asserts `clipped == 0` at the default fit, then **shrinks the canvas on purpose**
//! and asserts the detector fires. A counter that can only ever read zero is decoration.
//!
//! # ⚠ API notes for the pinned build, verified against the vendored sources
//!
//! * [`eframe::App`] here is **`logic` + `ui`**; there is no `update(&mut self, ctx, frame)`.
//! * Panels are `egui::Panel::{top,bottom,right}(id).show(ui, …)` shown *inside* the `Ui` handed to
//!   `ui()`, before the `CentralPanel`. `SidePanel`/`TopBottomPanel` do not exist.
//! * `CornerRadius::same(4)` takes an **integer**; `rect`/`rect_stroke` take a trailing
//!   [`egui::StrokeKind`]. The pre-0.31 three-argument form does not compile.
//! * `zoom_delta()` is a **method** on the input state; `smooth_scroll_delta` is a **field**.
//! * ⭐ **The `Shape::Noop` reservation trick is deliberately NOT used.** It exists for layouts
//!   computed *during* drawing, where edge shapes must be enqueued before node rects are known.
//!   Ours is computed in [`crate::graph`], so every rect is known before the first shape is
//!   emitted and a plain two-pass draw gives the identical z-order with no reserved slots.
//! * ⭐ **Edges are sampled polylines, not `CubicBezierShape`.** One code path then serves the solid
//!   and the dashed (asserted-upstream) case, and - the reason that matters - the samples come from
//!   [`crate::graph::cubic_at`], the same evaluation that placed the label. The label therefore sits
//!   *on* the wire by construction rather than by an eyeballed offset that drifts when the layout
//!   changes. (⚠ If you do reach for `CubicBezierShape::from_points_stroke`, it needs
//!   `closed = false` and `fill = TRANSPARENT`: a closed one fills a lens instead of drawing a wire.)

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::frame::{EdgeState, Frame};
use crate::graph::{
    cubic_at, GraphEdge, GraphModel, GraphNode, GraphParams, GraphSpec, Hit, NodeState, P2, RectF,
};
use crate::registry::Registry;
use crate::ui::Palette;

/// ⭐ Two more swatches than [`crate::ui::Palette`] carries, because a node has **six** readings and
/// two facts a reader must tell apart at a glance do not get to share a colour.
pub struct GPal;
impl GPal {
    /// `EMPTY` - measured, and the answer is no. ⛔ Not the same as UNREADABLE.
    pub const EMPTY: Color32 = Color32::from_rgb(0xD1, 0x9A, 0x66);
    /// `NO-WITNESS` - a deliberate abstention.
    pub const NO_WITNESS: Color32 = Color32::from_rgb(0x61, 0xAF, 0xEF);
    pub const CANVAS: Color32 = Color32::from_rgb(0x1B, 0x1E, 0x24);
    pub const NODE_BG: Color32 = Color32::from_rgb(0x24, 0x28, 0x30);
    pub const GRID: Color32 = Color32::from_rgb(0x2E, 0x33, 0x3C);
}

/// ⛔ FIVE EDGE COLOURS. Collapsing any two mis-accuses an edge - `grey` (never exercised) and
/// `unknown` (could not be read) in particular are opposite claims about whose fault the silence is.
pub fn edge_colour(s: EdgeState) -> Color32 {
    match s {
        EdgeState::Green => Palette::OK,
        EdgeState::Red => Palette::ERROR,
        EdgeState::Grey => Palette::MUTED,
        EdgeState::Unknown => Palette::WARN,
        EdgeState::Retired => Palette::VIOLET,
    }
}

/// ⛔ SIX NODE COLOURS, all distinct. Control #17 asserts the distinctness rather than trusting it.
pub fn node_colour(s: &NodeState) -> Color32 {
    match s {
        NodeState::Populated => Palette::OK,
        NodeState::Empty => GPal::EMPTY,
        NodeState::Poked { .. } => Palette::VIOLET,
        NodeState::Unreadable { .. } => Palette::WARN,
        NodeState::NoWitness { .. } => GPal::NO_WITNESS,
        NodeState::NotMeasurable { .. } => Palette::MUTED,
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The receipt
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐⭐ **WHAT THE CANVAS ACTUALLY PUT ON SCREEN**, counted against the painter's own clip rect.
///
/// This is the answer to the pre-clipped-painter trap: it is printed on the status bar every frame
/// and asserted by control #17. `*_clipped > 0` at the default fit is a **layout bug**; after the
/// operator pans it is simply the truth about what is visible, which is why the counts are shown as
/// `drawn/total` rather than as a pass/fail.
#[derive(Clone, Copy, Debug, Default)]
pub struct PaintReceipt {
    pub nodes_total: usize,
    pub nodes_clipped: usize,
    pub edges_total: usize,
    pub edges_clipped: usize,
    pub badges_drawn: usize,
    pub hit_targets: usize,
    pub scale: f32,
    pub canvas_w: f32,
    pub canvas_h: f32,
}

impl PaintReceipt {
    pub fn line(&self) -> String {
        format!(
            "{}/{} nodes · {}/{} edges on screen · {} badge(s) · {} hit target(s) · scale {:.2} · canvas {:.0}×{:.0}",
            self.nodes_total - self.nodes_clipped,
            self.nodes_total,
            self.edges_total - self.edges_clipped,
            self.edges_total,
            self.badges_drawn,
            self.hit_targets,
            self.scale,
            self.canvas_w,
            self.canvas_h
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// View state
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Pan, zoom, what is selected, and a galley cache. ⭐ Nothing here is a *value*: it is where the
/// operator is looking, and how big.
pub struct ViewState {
    pub pan: Vec2,
    pub zoom: f32,
    pub selection: Option<Hit>,
    /// ⚠ [`egui::Painter::text`] re-lays out the string on **every call**. At 27 nodes × 3 lines
    /// plus 21 edges × 2 that is ~120 layouts per repaint at 60 Hz, for text that never changes.
    /// Cached by `(text, size)` and painted with `galley`.
    galleys: HashMap<(String, u32), Arc<egui::Galley>>,
    pub galley_hits: u64,
    pub galley_misses: u64,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
            selection: None,
            galleys: HashMap::new(),
            galley_hits: 0,
            galley_misses: 0,
        }
    }
}

impl ViewState {
    fn galley(&mut self, p: &egui::Painter, text: &str, size: f32, colour: Color32) -> Arc<egui::Galley> {
        // ⚠ The colour is NOT part of the key: a galley is laid out once and painted with an
        //   override colour, so keying on colour would multiply the cache by the palette.
        let key = (text.to_string(), size.to_bits());
        if let Some(g) = self.galleys.get(&key) {
            self.galley_hits += 1;
            return g.clone();
        }
        self.galley_misses += 1;
        let g = p.layout_no_wrap(text.to_string(), FontId::proportional(size), colour);
        self.galleys.insert(key, g.clone());
        g
    }
}

/// World → screen. ⭐ One transform, built once per repaint, applied to everything - including the
/// hit rectangles, so what you click is what you see.
#[derive(Clone, Copy)]
struct Xform {
    scale: f32,
    dx: f32,
    dy: f32,
}

impl Xform {
    fn p(&self, p: P2) -> Pos2 {
        Pos2::new(p.x * self.scale + self.dx, p.y * self.scale + self.dy)
    }
    fn r(&self, r: &RectF) -> Rect {
        Rect::from_min_max(self.p(P2 { x: r.x0, y: r.y0 }), self.p(P2 { x: r.x1, y: r.y1 }))
    }
    fn inv(&self, s: Pos2) -> P2 {
        P2 { x: (s.x - self.dx) / self.scale, y: (s.y - self.dy) / self.scale }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The canvas
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐⭐ **THE WHOLE DRAWING, IN ONE FUNCTION THAT RETURNS A RECEIPT.**
///
/// It is `pub` and takes only `(&mut Ui, &GraphModel, &mut ViewState)` precisely so that control
/// #17 can drive it under `egui_kittest` with no window, no eframe and no sampler - the same shape
/// as control #11's render half. A drawing nobody can run headlessly is a drawing nobody audits.
pub fn paint_graph(ui: &mut egui::Ui, m: &GraphModel, view: &mut ViewState) -> PaintReceipt {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let canvas = resp.rect;

    // ── pan / zoom ──────────────────────────────────────────────────────────────────────────────
    if resp.dragged() {
        view.pan += resp.drag_delta();
    }
    if resp.hovered() {
        // ⚠ `zoom_delta()` is a METHOD on the input state (`smooth_scroll_delta` is a field).
        let z = ui.input(|i| i.zoom_delta());
        if (z - 1.0).abs() > f32::EPSILON {
            view.zoom = (view.zoom * z).clamp(0.25, 6.0);
        }
    }

    let ww = (m.world.x1 - m.world.x0).max(1.0);
    let wh = (m.world.y1 - m.world.y0).max(1.0);
    let fit = (canvas.width() / ww).min(canvas.height() / wh);
    let scale = (fit * view.zoom).max(0.02);
    let x = Xform {
        scale,
        dx: canvas.min.x - m.world.x0 * scale + view.pan.x,
        dy: canvas.min.y - m.world.y0 * scale + view.pan.y,
    };

    painter.rect_filled(canvas, CornerRadius::same(4), GPal::CANVAS);

    let mut r = PaintReceipt {
        nodes_total: m.nodes.len(),
        edges_total: m.edges.len(),
        scale,
        canvas_w: canvas.width(),
        canvas_h: canvas.height(),
        ..Default::default()
    };
    let clip = painter.clip_rect();

    // ── column bands and headers ────────────────────────────────────────────────────────────────
    for c in &m.columns {
        let band = Rect::from_min_max(
            x.p(P2 { x: c.x0, y: m.world.y0 }),
            x.p(P2 { x: c.x1, y: m.world.y1 }),
        );
        if c.index % 2 == 1 {
            painter.rect_filled(band, CornerRadius::same(0), GPal::GRID);
        }
        let colour = if c.empty { Palette::MUTED } else { Palette::ACCENT };
        let title = if c.empty {
            // ⚠ An empty column SAYS SO. Omitting it would hide a coverage gap in the recon.
            format!("{}  (no edge endpoint)", c.name)
        } else {
            format!("{}  ({})", c.name, c.node_count)
        };
        let g = view.galley(&painter, &title, 13.0, colour);
        painter.galley(x.p(P2 { x: c.x0 + 14.0, y: m.world.y0 + 46.0 }), g, colour);
    }

    // ── edges, UNDER the nodes ──────────────────────────────────────────────────────────────────
    for (i, e) in m.edges.iter().enumerate() {
        let selected = matches!(view.selection, Some(Hit::Edge(k) | Hit::Badge(k)) if k == i);
        let col = edge_colour(e.color);
        let w = if e.color == EdgeState::Red { 3.4 } else { 1.8 } * if selected { 1.8 } else { 1.0 };
        draw_wire(&painter, &x, e, col, w);

        // the arrow head, from the curve's own tail so it points along the wire
        let near = cubic_at(e.from, e.c1, e.c2, e.to, 0.90);
        let tip = x.p(e.to);
        let from = x.p(near);
        painter.arrow(from, tip - from, Stroke::new(w, col));

        let lr = x.r(&e.label_rect);
        if !clip.contains_rect(lr) {
            r.edges_clipped += 1;
        }
    }

    // ── nodes ───────────────────────────────────────────────────────────────────────────────────
    for (i, n) in m.nodes.iter().enumerate() {
        let nr = x.r(&n.rect);
        if !clip.contains_rect(nr) {
            r.nodes_clipped += 1;
        }
        let col = node_colour(&n.state);
        let selected = matches!(view.selection, Some(Hit::Node(k)) if k == i);
        painter.rect(
            nr,
            CornerRadius::same(4),
            GPal::NODE_BG,
            Stroke::new(if selected { 2.6 } else { 1.4 }, col),
            StrokeKind::Inside,
        );
        // ⚠ Cull against the painter's clip rect ourselves - nothing culls text for us, and a
        //   layout for an off-screen node is a layout paid for nothing.
        if nr.intersects(clip) {
            let g = view.galley(&painter, &n.label, 12.5, Palette::MUTED);
            painter.galley(nr.min + Vec2::new(8.0, 5.0) * scale.min(1.0), g, Color32::from_rgb(0xD8, 0xDD, 0xE4));
            let g = view.galley(&painter, n.state.label(), 11.0, col);
            painter.galley(nr.min + Vec2::new(8.0, 23.0) * scale.min(1.0), g, col);
            let wt: String = n.witness_text.chars().take(46).collect();
            let g = view.galley(&painter, &wt, 10.0, Palette::MUTED);
            painter.galley(nr.min + Vec2::new(8.0, 39.0) * scale.min(1.0), g, Palette::MUTED);
        }
    }

    // ── edge labels and event badges, ON TOP ────────────────────────────────────────────────────
    for e in m.edges.iter() {
        let col = edge_colour(e.color);
        let lr = x.r(&e.label_rect);
        if lr.intersects(clip) {
            painter.rect(
                lr,
                CornerRadius::same(3),
                GPal::CANVAS,
                Stroke::new(1.0, col),
                StrokeKind::Inside,
            );
            let t = format!(
                "{} {}{}",
                e.id,
                format!("{:?}", e.color).to_uppercase(),
                if e.agrees { "" } else { " ⚠" }
            );
            let g = view.galley(&painter, &t, 10.5, col);
            painter.galley(lr.min + Vec2::new(5.0, 1.5), g, col);
        }
        if let Some(br) = &e.badge_rect {
            let brr = x.r(br);
            r.badges_drawn += 1;
            if brr.intersects(clip) {
                // ⭐ THE BADGE SHOWS THE LAST WRITER AS `insn @ addr`, or the FINDING that nothing
                //   ever fired. ⛔ Those are different facts and never share a text.
                let (t, c) = match (&e.last_writer, e.badge_text.as_str()) {
                    (Some(w), _) => (w.badge_line(), Palette::OK),
                    (None, s) if s.contains("NEVER FIRED") => {
                        // ⭐ A real negative result, in violet - not a warning colour. Painting it
                        //   red would teach the reader to read the evidence as a fault.
                        ("NEVER FIRED since armed".to_string(), Palette::VIOLET)
                    }
                    (None, s) => (s.to_string(), Palette::MUTED),
                };
                let t: String = t.chars().take(38).collect();
                painter.rect(brr, CornerRadius::same(3), GPal::CANVAS, Stroke::new(1.0, c), StrokeKind::Inside);
                let g = view.galley(&painter, &t, 10.0, c);
                painter.galley(brr.min + Vec2::new(5.0, 1.5), g, c);
            }
        }
    }

    // ── the border, ALWAYS ──────────────────────────────────────────────────────────────────────
    painter.rect_stroke(canvas, CornerRadius::same(4), Stroke::new(1.0, GPal::GRID), StrokeKind::Inside);

    // ── hit targets ─────────────────────────────────────────────────────────────────────────────
    // ⚠ Per-node/per-edge `ui.interact` with an EXPLICIT id: the single canvas `Response` can never
    //   say WHICH node was clicked, and auto-ids shift when the visible set changes.
    let mut clicked: Option<Hit> = None;
    for (i, e) in m.edges.iter().enumerate() {
        if let Some(br) = &e.badge_rect {
            let id = ui.make_persistent_id(("gbadge", &e.id));
            r.hit_targets += 1;
            if ui.interact(x.r(br), id, Sense::click()).clicked() {
                clicked = Some(Hit::Badge(i));
            }
        }
    }
    for (i, e) in m.edges.iter().enumerate() {
        let id = ui.make_persistent_id(("gedge", &e.id));
        r.hit_targets += 1;
        if ui.interact(x.r(&e.label_rect), id, Sense::click()).clicked() {
            clicked = Some(Hit::Edge(i));
        }
    }
    for (i, n) in m.nodes.iter().enumerate() {
        let id = ui.make_persistent_id(("gnode", &n.id));
        r.hit_targets += 1;
        if ui.interact(x.r(&n.rect), id, Sense::click()).clicked() {
            clicked = Some(Hit::Node(i));
        }
    }
    // ⭐ A click on empty canvas falls through to the model's own hit test, so the two agree by
    //   construction: the canvas cannot select something `GraphModel::hit` would not.
    if clicked.is_none() && resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            clicked = m.hit(x.inv(p));
        }
    }
    if let Some(h) = clicked {
        view.selection = Some(h);
    }

    r
}

/// Sample the curve and draw it, dashing the half whose upstream is ASSERTED rather than measured.
fn draw_wire(painter: &egui::Painter, x: &Xform, e: &GraphEdge, col: Color32, w: f32) {
    const N: usize = 28;
    let mut prev = x.p(e.from);
    for i in 1..=N {
        let t = i as f32 / N as f32;
        let p = x.p(cubic_at(e.from, e.c1, e.c2, e.to, t));
        // ⭐ The dashed half says "we ASSERT we sent this; we did not measure it". An edge out of
        //   our own servers is half-evidence, and the wire says so without a legend.
        let dashed = !e.upstream_measured && t <= 0.5;
        let retired = e.color == EdgeState::Retired;
        let skip = (dashed || retired) && i % 2 == 0;
        if !skip {
            painter.line_segment([prev, p], Stroke::new(w, col));
        }
        prev = p;
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The app
// ─────────────────────────────────────────────────────────────────────────────────────────────────

const IDLE_REPAINT_MS: u64 = 250;

pub struct GraphApp {
    rx: Receiver<Arc<Frame>>,
    latest: Option<Arc<Frame>>,
    arrived: Instant,
    frames_seen: u64,
    frames_dropped: u64,
    reg: Registry,
    spec: GraphSpec,
    params: GraphParams,
    provenance: String,
    view: ViewState,
    receipt: PaintReceipt,
}

impl GraphApp {
    pub fn new(
        rx: Receiver<Arc<Frame>>,
        reg: Registry,
        spec: GraphSpec,
        params: GraphParams,
        provenance: String,
    ) -> Self {
        Self {
            rx,
            latest: None,
            arrived: Instant::now(),
            frames_seen: 0,
            frames_dropped: 0,
            reg,
            spec,
            params,
            provenance,
            view: ViewState::default(),
            receipt: PaintReceipt::default(),
        }
    }

    fn model(&self) -> Option<GraphModel> {
        let f = self.latest.as_ref()?;
        let mut p = self.params.clone();
        p.age_ms = self.arrived.elapsed().as_millis() as u64;
        Some(GraphModel::build(f, &self.reg, &self.spec, &p))
    }
}

impl eframe::App for GraphApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ⭐ DRAIN TO THE LATEST, and COUNT what was dropped. An event badge belongs to the frame it
        //   arrived on; rendering a backlog would show stale badges beside current values.
        let mut newest = None;
        while let Ok(f) = self.rx.try_recv() {
            if newest.is_some() {
                self.frames_dropped += 1;
            }
            self.frames_seen += 1;
            newest = Some(f);
        }
        if let Some(f) = newest {
            self.latest = Some(f);
            self.arrived = Instant::now();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ⭐ Repaint on our own clock so STALE can appear without anyone feeding us.
        ctx.request_repaint_after(std::time::Duration::from_millis(IDLE_REPAINT_MS));

        let model = self.model();
        egui::Panel::top("graph-top").show(ui, |ui| self.top_bar(ui, model.as_ref()));
        egui::Panel::bottom("graph-status").show(ui, |ui| self.status_bar(ui, model.as_ref()));
        if let Some(m) = &model {
            egui::Panel::right("graph-inspector").default_size(460.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui, m));
            });
        }
        egui::CentralPanel::default().show(ui, |ui| match &model {
            None => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("waiting for the first frame from the sampler thread…").color(Palette::MUTED));
                    ui.label(
                        RichText::new("⚠ if this never changes, the sampler failed to open its source - check stderr.")
                            .small()
                            .color(Palette::WARN),
                    );
                });
            }
            Some(m) => {
                self.receipt = paint_graph(ui, m, &mut self.view);
            }
        });
    }
}

impl GraphApp {
    fn top_bar(&mut self, ui: &mut egui::Ui, m: Option<&GraphModel>) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("d1-scope · flow graph").strong());
            ui.label(RichText::new(&self.provenance).small().color(Palette::MUTED));
            if let Some(m) = m {
                ui.separator();
                ui.label(RichText::new(format!("seq {}", m.frame_seq)).small().monospace());
                ui.label(RichText::new(format!("gen {}", m.generation)).small().monospace().color(Palette::MUTED));
                ui.label(RichText::new(format!("tick {:.1} ms", m.tick_ms)).small().monospace().color(Palette::MUTED));
                if m.stale {
                    ui.label(
                        RichText::new(format!("⛔ STALE - {} ms since the last frame", m.age_ms))
                            .strong()
                            .color(Palette::ERROR),
                    );
                }
            }
            ui.separator();
            if ui.button("reset view").clicked() {
                self.view.pan = Vec2::ZERO;
                self.view.zoom = 1.0;
            }
            if ui.button("clear selection").clicked() {
                self.view.selection = None;
            }
            ui.label(RichText::new(format!("zoom {:.2}×", self.view.zoom)).small().color(Palette::MUTED));
        });
        // ⭐⭐ THE HEADLINE. The one thing this view exists to make obvious, at the top, in red,
        //     before anything else - and EVERY red, because two on one path are independent.
        if let Some(m) = m {
            ui.add_space(2.0);
            match &m.headline {
                Some(h) => {
                    for line in h.lines() {
                        ui.label(RichText::new(line).strong().color(Palette::ERROR));
                    }
                }
                None => {
                    ui.label(
                        RichText::new(
                            "no RED edge this tick - ⚠ which is NOT 'nothing is broken': read the grey and unknown counts below",
                        )
                        .small()
                        .color(Palette::WARN),
                    );
                }
            }
        }
        ui.add_space(4.0);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, m: Option<&GraphModel>) {
        ui.add_space(2.0);
        let Some(m) = m else {
            ui.label(RichText::new("no frame").small().color(Palette::MUTED));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            // ⛔ ALWAYS A DENOMINATOR, and always the CLIPPED count: a node that is off-canvas and a
            //    node that failed to draw are otherwise the same picture.
            ui.label(RichText::new(self.receipt.line()).small().monospace());
            ui.separator();
            for s in [EdgeState::Green, EdgeState::Red, EdgeState::Grey, EdgeState::Unknown, EdgeState::Retired] {
                let k = format!("{s:?}").to_lowercase();
                if let Some(n) = m.color_counts.get(&k) {
                    ui.label(RichText::new(format!("{n} {k}")).small().monospace().color(edge_colour(s)));
                }
            }
            ui.separator();
            ui.label(
                RichText::new(format!("frames {} (dropped {})", self.frames_seen, self.frames_dropped))
                    .small()
                    .color(Palette::MUTED),
            );
            ui.label(
                RichText::new(format!("galleys {}/{}", self.view.galley_hits, self.view.galley_hits + self.view.galley_misses))
                    .small()
                    .color(Palette::MUTED),
            );
        });
        for n in &m.notes {
            let colour = if n.contains("⛔") { Palette::ERROR } else if n.contains('⚠') { Palette::WARN } else { Palette::MUTED };
            ui.label(RichText::new(n).small().color(colour));
        }
        ui.add_space(2.0);
    }

    /// ⭐ Clicking an EDGE opens its two surfaces **side by side**; clicking a BADGE opens the
    /// writer-event log with the backward-decode caveat **inline**, never as a footnote.
    fn inspector(&mut self, ui: &mut egui::Ui, m: &GraphModel) {
        match self.view.selection {
            None => {
                ui.label(RichText::new("nothing selected").strong());
                ui.label(
                    RichText::new(
                        "click an EDGE label to open its two surfaces side by side · click an event \
                         BADGE to open the writer log · click a NODE for its witness and its reason \
                         · drag to pan · ctrl-scroll to zoom",
                    )
                    .small()
                    .color(Palette::MUTED),
                );
            }
            Some(Hit::Node(i)) => {
                if let Some(n) = m.nodes.get(i) {
                    node_panel(ui, n);
                }
            }
            Some(Hit::Edge(i)) => {
                if let Some(e) = m.edges.get(i) {
                    edge_panel(ui, m, e, false);
                }
            }
            Some(Hit::Badge(i)) => {
                if let Some(e) = m.edges.get(i) {
                    edge_panel(ui, m, e, true);
                }
            }
        }
    }
}

fn node_panel(ui: &mut egui::Ui, n: &GraphNode) {
    ui.label(RichText::new(&n.id).strong().monospace());
    ui.label(RichText::new(format!("{} · column {}", n.label, n.column)).small().color(Palette::MUTED));
    ui.separator();
    ui.label(RichText::new(n.state.label()).strong().color(node_colour(&n.state)));
    ui.label(RichText::new(n.state.detail()).small());
    ui.add_space(4.0);
    ui.label(RichText::new(format!("witness   {}", n.witness_text)).small().monospace());
    ui.label(RichText::new(format!("address   {}", n.addr)).small().monospace().color(Palette::MUTED));
    ui.label(RichText::new(format!("edges     {}", n.degree)).small().monospace().color(Palette::MUTED));
    ui.separator();
    // ⭐ The WHY, verbatim from graph.toml - the reason this witness and not another one.
    ui.label(RichText::new(&n.why).small().color(Palette::MUTED));
}

/// ⛔ Both surfaces, ALWAYS - even when the click was on the badge. An edge is a claim about a PAIR,
/// and showing one end is how "the downstream is empty" gets read without "the upstream is full".
fn edge_panel(ui: &mut egui::Ui, m: &GraphModel, e: &GraphEdge, writers_first: bool) {
    ui.label(RichText::new(format!("{}  {:?}", e.id, e.color)).strong().color(edge_colour(e.color)));
    ui.label(RichText::new(&e.mech).small().color(Palette::MUTED));
    ui.separator();
    ui.label(RichText::new(&e.why).small());
    for c in &e.caveats {
        ui.label(RichText::new(c).small().color(Palette::WARN));
    }
    if let Some(g) = &e.accusation_suppressed {
        ui.label(RichText::new(format!("⛔ GUARD (from registry.toml): {g}")).small().color(Palette::WARN));
    }
    // ⚠ DERIVED next to DECLARED, always. Two instruments disagreeing is a finding.
    ui.label(
        RichText::new(format!(
            "derived {:?}  ·  declared {:?} (as_of {}){}",
            e.color,
            e.declared,
            e.as_of,
            if e.agrees { "" } else { "   ⚠ THEY DISAGREE" }
        ))
        .small()
        .monospace()
        .color(if e.agrees { Palette::MUTED } else { Palette::WARN }),
    );

    if writers_first {
        writer_log(ui, e);
        ui.separator();
    }

    // ── the two surfaces, SIDE BY SIDE ──────────────────────────────────────────────────────────
    ui.separator();
    ui.columns(2, |cols| {
        for (k, id) in [(0usize, &e.up), (1usize, &e.down)] {
            let ui = &mut cols[k];
            ui.label(RichText::new(if k == 0 { "UPSTREAM" } else { "DOWNSTREAM" }).small().strong().color(Palette::ACCENT));
            match m.nodes.iter().find(|n| n.id == *id) {
                Some(n) => node_panel(ui, n),
                None => {
                    ui.label(RichText::new(format!("{id}  - NO NODE")).small().color(Palette::ERROR));
                }
            }
        }
    });

    if !writers_first {
        ui.separator();
        writer_log(ui, e);
    }
}

/// ⚠⚠ **THE BACKWARD-DECODE CAVEAT IS INLINE, ON EVERY ENTRY.** A data breakpoint is a TRAP: the
/// reported `rip` is the instruction AFTER the store. `rip` and `writer` are two fields and are
/// never collapsed - a view that carried one "address" would name the wrong instruction, and when
/// the store ends a basic block, the wrong basic block, while looking entirely correct.
fn writer_log(ui: &mut egui::Ui, e: &GraphEdge) {
    ui.label(RichText::new("EVENT PLANE").small().strong().color(Palette::ACCENT));
    if e.badge_text.is_empty() && e.writers.is_empty() {
        ui.label(
            RichText::new(
                "nothing in the event plane concerns this edge's downstream. ⛔ That is not a \
                 negative result - no watch was armed here.",
            )
            .small()
            .color(Palette::MUTED),
        );
        return;
    }
    if !e.badge_text.is_empty() {
        ui.label(RichText::new(&e.badge_text).small().color(Palette::VIOLET));
        ui.label(RichText::new(e.badge.detail()).small().color(Palette::MUTED));
    }
    if e.writers.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!("{} write(s), attributed by ADDRESS - newest last", e.writers.len()))
            .small()
            .color(Palette::MUTED),
    );
    for w in &e.writers {
        ui.add_space(3.0);
        ui.label(RichText::new(format!("t+{} ms  tid {}", w.elapsed_ms, w.tid)).small().monospace().color(Palette::MUTED));
        ui.label(RichText::new(w.badge_line()).monospace().small().color(if w.quotable { Palette::OK } else { Palette::WARN }));
        // ⛔ INLINE. Not a tooltip, not a footnote - the caveat travels with the reading.
        ui.label(RichText::new(w.caveat()).small().color(Palette::WARN));
        if let Some(v) = &w.value_after {
            ui.label(
                RichText::new(format!("value after the trap: {v}"))
                    .small()
                    .monospace()
                    .color(Palette::MUTED),
            );
        }
    }
}
