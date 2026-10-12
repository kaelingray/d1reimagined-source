// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ **THE SURFACE TABLE, DRAWN.** eframe/egui, pinned `=0.36.1`, glow only.
//!
//! # ⛔ WHAT THIS MODULE MAY NOT DO, AND WHY IT IS A MODULE BOUNDARY RATHER THAN A HABIT
//!
//! ⚠ **AND NOTE WHAT THIS PROSE MAY NOT SAY.** Control #14 scans this file for the *names* of the
//! memory-source and child-manager types, with no exclusion list - so the paragraphs below have to
//! **describe** those names rather than spell them. That is the right trade, and it is the same
//! one control #4 makes about the debug API: an exclusion is a hole someone later widens. (This
//! comment used to name the read trait, and the control caught it on the first run. Working as
//! intended.)
//!
//! 1. **It never reads memory.** No read-source type, no `/proc`, no snapshot. The sampler runs on
//!    its own thread and hands over an `Arc<Frame>`; a UI thread that read process memory would put a
//!    multi-hundred-millisecond read inside a 60 Hz repaint and - far worse - would make the
//!    *perturbation budget* (`frame.tick_ms`) a lie, because the reads would no longer be the ones
//!    being measured.
//! 2. **It never spawns a child and never signals one.** BUILD 3's window arms nothing at all. The
//!    one unrecoverable mistake available on this box is on the operator's live client, and a
//!    button is exactly how it gets made by accident. Arming stays on the command line, where it
//!    takes a deliberate `--pid`.
//! 3. **It computes no value.** Every cell string comes from [`TableRow::cell`]. What lives here is
//!    view state and nothing else: sort column, filter text, grouping, and how bright a flash is.
//!
//! Control **#14** asserts 1-3 over this file's own text, with a positive control so that
//! *"no forbidden token"* cannot mean *"the scan was broken"*.
//!
//! # ⚠ THE API TRAPS OF THE PINNED BUILD, verified against the vendored sources, not from memory
//!
//! * [`eframe::App`] here is **`logic` + `ui`** - there is no `update(&mut self, ctx, frame)`.
//! * Panels are **`egui::Panel::{top,bottom,left,right}(id).show(ui, …)`**, shown *inside* the `Ui`
//!   handed to `ui()`, before the `CentralPanel`. `TopBottomPanel`/`SidePanel` do not exist.
//! * `Context::style()` / `set_style()` are gone: [`egui::Context::all_styles_mut`].
//! * `egui_extras::TableBuilder` has **one** `sense` for header and body, so a click-sortable
//!   header forces click-sensing body cells; the click target is the `Response` returned by
//!   `col()`, not `row.response()`.
//! * `Column::auto()` under a virtualized body measures only the *visible* rows and jitters as you
//!   scroll - every column here is `initial(..).at_least(..)`.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use egui::{Color32, CornerRadius, RichText};

use crate::frame::{ChildStatus, Frame};
use crate::table::{Column, RenderState, RowKind, Signatures, TableModel, TableParams};

/// Colours, lifted from `d1-forge/src/theme.rs` so the two tools look like one project.
pub struct Palette;
impl Palette {
    pub const ACCENT: Color32 = Color32::from_rgb(0x4A, 0x8F, 0xE7);
    pub const ERROR: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);
    pub const WARN: Color32 = Color32::from_rgb(0xE5, 0xC0, 0x7B);
    pub const OK: Color32 = Color32::from_rgb(0x8C, 0xC2, 0x65);
    pub const MUTED: Color32 = Color32::from_rgb(0x93, 0x9A, 0xA6);
    pub const VIOLET: Color32 = Color32::from_rgb(0xC0, 0x8C, 0xE0);
}

/// ⭐ ONE COLOUR PER RENDER STATE, and `UNMAPPED` is deliberately **not** the same grey as
/// `NOT-READ`. Two facts that a reader must be able to tell apart at a glance do not get to share a
/// swatch - that is the whole UNMAPPED-is-not-zero rail, in pixels.
fn state_colour(s: RenderState) -> Color32 {
    match s {
        RenderState::Value => Palette::OK,
        RenderState::Unmapped => Palette::WARN,
        RenderState::NotLive => Palette::VIOLET,
        RenderState::NotRead => Palette::MUTED,
        RenderState::Unresolved => Palette::ERROR,
        RenderState::NotExpressible => Palette::ACCENT,
        RenderState::UnknownAddr => Palette::MUTED,
    }
}

/// How long a changed row stays lit, in seconds.
const FLASH_S: f64 = 1.2;
/// The window repaints at least this often even when nothing arrives - ⭐ so that **staleness
/// appears on its own**. A window that only repaints when fed can never show that it stopped being
/// fed, which is the one thing a frozen window most needs to say.
const IDLE_REPAINT_MS: u64 = 250;

pub struct ScopeApp {
    rx: Receiver<Arc<Frame>>,
    latest: Option<Arc<Frame>>,
    /// When the frame on screen ARRIVED. Staleness is measured from here, not from the frame's own
    /// timestamps: the question is whether the producer is still running.
    arrived: Instant,
    frames_seen: u64,
    /// ⭐ Frames received and DROPPED because a newer one was in the queue. Shown, never hidden:
    /// this window renders the latest frame only, and a reader who thinks they are seeing every
    /// tick would mis-count events.
    frames_dropped: u64,
    prev_sig: Signatures,
    /// row key → the `ctx.input().time` at which it last changed.
    flash: HashMap<String, f64>,
    params: TableParams,
    provenance: String,
    fonts: Vec<String>,
    /// ⭐ The selected row's `TableRow::key` - **the key, never the index.** The index moves on
    /// every re-sort and every filter change, so an index-based selection silently follows whatever
    /// row slid into that slot. Same reasoning as the flash animation, which keys off `key` for
    /// exactly this reason.
    ///
    /// ⚠ A key that no longer exists in the model (filtered out, or the surface stopped resolving)
    /// renders as an explicit "no longer in this frame" panel rather than vanishing - a detail pane
    /// that empties itself when a row disappears looks identical to one that was never populated.
    selected: Option<String>,
}

impl ScopeApp {
    pub fn new(rx: Receiver<Arc<Frame>>, params: TableParams, provenance: String, fonts: Vec<String>) -> Self {
        Self {
            rx,
            latest: None,
            arrived: Instant::now(),
            frames_seen: 0,
            frames_dropped: 0,
            prev_sig: Signatures::new(),
            flash: HashMap::new(),
            params,
            provenance,
            fonts,
            selected: None,
        }
    }

    fn age_ms(&self) -> u64 {
        self.arrived.elapsed().as_millis() as u64
    }

    /// Rebuild the model for THIS repaint: params and age both move without a new frame arriving
    /// (a filter keystroke, or the sampler dying), and both must change what is on screen.
    fn model(&self, now: f64) -> Option<TableModel> {
        let f = self.latest.as_ref()?;
        let mut p = self.params.clone();
        p.age_ms = self.age_ms();
        let mut m = TableModel::build(f, &p);
        // ⚠ `changed` here means *"is currently flashing"*, derived from the recorded change
        // times - NOT recomputed against the previous model. Recomputing it during a repaint would
        // diff a frame against itself and nothing would ever flash.
        for r in &mut m.rows {
            r.changed = self.flash.get(&r.key).is_some_and(|t| now - t < FLASH_S);
        }
        Some(m)
    }
}

impl eframe::App for ScopeApp {
    /// ⭐ DRAIN TO THE LATEST. Everything older is counted and dropped - an event-plane badge is
    /// attached to the frame it arrived on, so rendering a backlog would show stale badges next to
    /// current values.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut newest: Option<Arc<Frame>> = None;
        while let Ok(f) = self.rx.try_recv() {
            if newest.is_some() {
                self.frames_dropped += 1;
            }
            self.frames_seen += 1;
            newest = Some(f);
        }
        let Some(f) = newest else { return };

        // Change detection happens exactly once per arriving frame.
        let mut p = self.params.clone();
        p.age_ms = 0;
        // ⚠ Diff over the UNFILTERED, UNGROUPED row set: a row hidden by the current filter still
        //   changed, and it must be lit when the filter is cleared rather than silently missed.
        p.filter.clear();
        p.group_by_group = false;
        p.show_fields = true;
        let mut m = TableModel::build(&f, &p);
        m.mark_changes(&self.prev_sig);
        let now = ctx.input(|i| i.time);
        for r in m.rows.iter().filter(|r| r.changed) {
            self.flash.insert(r.key.clone(), now);
        }
        self.prev_sig = m.signatures();
        self.latest = Some(f);
        self.arrived = Instant::now();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ⭐ Repaint on our own clock so STALE can appear without anyone feeding us.
        ctx.request_repaint_after(std::time::Duration::from_millis(IDLE_REPAINT_MS));
        let now = ctx.input(|i| i.time);

        let model = self.model(now);

        egui::Panel::top("provenance").show(ui, |ui| self.provenance_bar(ui, model.as_ref()));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui, model.as_ref()));
        if self.selected.is_some() {
            egui::Panel::right("detail").show(ui, |ui| self.detail_pane(ui, model.as_ref()));
        }
        egui::CentralPanel::default().show(ui, |ui| match &model {
            // ⛔ NOT an empty table. "No frame yet" and "a frame with no rows" are different facts
            //    and an empty grid says the wrong one.
            None => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("waiting for the first frame from the sampler thread…").color(Palette::MUTED));
                    ui.label(RichText::new("⚠ if this never changes, the sampler failed to open its source - check stderr.").small().color(Palette::WARN));
                });
            }
            Some(m) => self.table(ui, m, now),
        });
    }
}

impl ScopeApp {
    /// ⭐⭐ THE DETAIL PANE - everything [`TableRow`] already carried and the six columns had no
    /// room for. **It renders no new facts**: every line here is a field of the same `TableRow` the
    /// headless dump serialises, so a claim made in this panel is reproducible with
    /// `d1-scope gui --headless --json` and never exists only on screen.
    ///
    /// ⭐ The one that earns the pane is [`TableRow::resolve_trace`] - *how the address was reached*,
    /// step by step. That is the difference between "this row reads UNMAPPED" and "this row reads
    /// UNMAPPED **because the second deref landed outside the captured extents**", and until now it
    /// was computed on every tick and shown nowhere.
    fn detail_pane(&mut self, ui: &mut egui::Ui, m: Option<&TableModel>) {
        let key = match self.selected.clone() {
            Some(k) => k,
            None => return,
        };
        ui.set_min_width(430.0);
        ui.add_space(6.0);

        let row = m.and_then(|m| m.rows.iter().find(|r| r.key == key));
        let Some(r) = row else {
            // ⛔ NOT a blank pane. "Filtered out / stopped resolving" and "nothing selected" are
            //    different facts, and an empty panel says the wrong one.
            ui.label(RichText::new("⚠ this row is not in the current frame").color(Palette::WARN));
            ui.label(RichText::new(&key).small().monospace().color(Palette::MUTED));
            ui.label(RichText::new("it was filtered out, or the surface stopped producing a row.").small());
            if ui.button("clear selection").clicked() {
                self.selected = None;
            }
            return;
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new(&r.name).strong());
            if ui.small_button("✕").on_hover_text("clear selection").clicked() {
                self.selected = None;
            }
        });
        ui.label(RichText::new(&r.key).small().monospace().color(Palette::MUTED));
        ui.separator();

        let kv = |ui: &mut egui::Ui, k: &str, v: &str| {
            if v.is_empty() {
                return;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{k:<12}")).small().monospace().color(Palette::MUTED));
                ui.label(RichText::new(v).monospace());
            });
        };
        kv(ui, "group", &r.group);
        kv(ui, "addr", &r.addr);
        kv(ui, "value", &r.value);
        kv(ui, "state", &r.state_text);
        if !r.badge_text.is_empty() {
            kv(ui, "event", &r.badge_text);
        }
        // ⚠ Carried ALONGSIDE state, never folded into it - see the table module header.
        if r.stale {
            ui.label(RichText::new("⚠ STALE - the frame stopped advancing; this is about the SAMPLER, not the client").small().color(Palette::WARN));
        }
        ui.separator();
        kv(ui, "confidence", &r.confidence);
        kv(ui, "reach", &r.reach);
        kv(ui, "liveness", &r.liveness);

        if let Some(n) = &r.note {
            ui.add_space(4.0);
            ui.label(RichText::new("note").small().monospace().color(Palette::MUTED));
            ui.label(RichText::new(n).small());
        }

        if !r.resolve_trace.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("resolve trace").small().monospace().color(Palette::MUTED));
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for (i, step) in r.resolve_trace.iter().enumerate() {
                    ui.label(RichText::new(format!("{i}. {step}")).small().monospace());
                }
            });
        } else {
            ui.add_space(6.0);
            ui.label(RichText::new("no resolve trace - this row is a group header or a literal").small().color(Palette::MUTED));
        }
    }

    fn provenance_bar(&mut self, ui: &mut egui::Ui, m: Option<&TableModel>) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("d1-scope").strong());
            ui.label(RichText::new(&self.provenance).small().color(Palette::MUTED));
            if let Some(m) = m {
                ui.separator();
                ui.label(RichText::new(format!("seq {}", m.frame_seq)).small().monospace());
                ui.label(RichText::new(format!("gen {}", m.generation)).small().monospace().color(Palette::MUTED));
                ui.label(RichText::new(format!("tick {:.1} ms", m.tick_ms)).small().monospace().color(Palette::MUTED));
                // ⛔ THE LOUDEST THING ON THE BAR, because a frozen window is otherwise identical
                //    to a quiet one.
                if m.stale {
                    ui.label(
                        RichText::new(format!("⛔ STALE - {} ms since the last frame", m.age_ms))
                            .strong()
                            .color(Palette::ERROR),
                    );
                } else {
                    ui.label(RichText::new(format!("+{} ms", m.age_ms)).small().color(Palette::MUTED));
                }
            }
        });
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("filter");
            ui.add(egui::TextEdit::singleline(&mut self.params.filter).desired_width(240.0).hint_text("group, name, addr, value, state, event…"));
            if ui.button("clear").clicked() {
                self.params.filter.clear();
            }
            ui.separator();
            ui.checkbox(&mut self.params.group_by_group, "group");
            ui.checkbox(&mut self.params.show_fields, "fields");
            ui.separator();
            ui.label(RichText::new("sort").small().color(Palette::MUTED));
            egui::ComboBox::from_id_salt("sortcol")
                .selected_text(self.params.sort.title())
                .show_ui(ui, |ui| {
                    for c in Column::ALL {
                        ui.selectable_value(&mut self.params.sort, c, c.title());
                    }
                });
            ui.checkbox(&mut self.params.descending, "desc");
        });
        ui.add_space(4.0);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, m: Option<&TableModel>) {
        ui.add_space(2.0);
        let Some(m) = m else {
            ui.label(RichText::new("no frame").small().color(Palette::MUTED));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            // ⭐ ALWAYS WITH A DENOMINATOR. "12 rows" alone cannot be told apart from a broken model.
            ui.label(
                RichText::new(format!(
                    "{} of {} rows shown ({} filtered out)",
                    m.rows.iter().filter(|r| r.kind != RowKind::GroupHeader).count(),
                    m.rows_total,
                    m.rows_filtered_out
                ))
                .small()
                .monospace(),
            );
            ui.separator();
            for s in RenderState::ALL {
                if let Some(n) = m.state_counts.get(s.label()) {
                    ui.label(RichText::new(format!("{n} {}", s.label())).small().monospace().color(state_colour(s)));
                }
            }
            ui.separator();
            ui.label(
                RichText::new(format!("frames {} (dropped {})", self.frames_seen, self.frames_dropped))
                    .small()
                    .color(Palette::MUTED),
            );
            if !self.fonts.is_empty() {
                ui.label(RichText::new(format!("fonts {}", self.fonts.len())).small().color(Palette::MUTED));
            }
        });
        // The child banner - one line, and it says which of the four zero-hit meanings applies.
        if let Some(f) = &self.latest {
            let (text, colour) = child_line(&f.child);
            if !text.is_empty() {
                ui.label(RichText::new(text).small().color(colour));
            }
        }
        for n in &m.notes {
            let colour = if n.contains("⛔") || n.starts_with("[loud]") {
                Palette::ERROR
            } else if n.contains('⚠') || n.starts_with("[warn]") {
                Palette::WARN
            } else {
                Palette::MUTED
            };
            ui.label(RichText::new(n).small().color(colour));
        }
        ui.add_space(2.0);
    }

    fn table(&mut self, ui: &mut egui::Ui, m: &TableModel, now: f64) {
        use egui_extras::{Column as TCol, TableBuilder};

        // ⚠ `initial(..).at_least(..)`, never `auto()`: under a virtualized body `auto()` measures
        //    only the visible rows, so the columns visibly resize as you scroll.
        let widths = [
            TCol::initial(96.0).at_least(70.0),
            TCol::initial(300.0).at_least(160.0),
            TCol::initial(220.0).at_least(140.0),
            TCol::initial(280.0).at_least(160.0),
            TCol::initial(150.0).at_least(110.0),
            TCol::remainder().at_least(150.0),
        ];
        let mut clicked: Option<Column> = None;
        let mut picked: Option<String> = None;
        let mut builder = TableBuilder::new(ui)
            .striped(true)
            // ⚠ ONE sense for header AND body: click-sortable headers force click-sensing cells.
            .sense(egui::Sense::click())
            .resizable(true)
            .min_scrolled_height(0.0);
        for w in widths {
            builder = builder.column(w);
        }

        let sort = self.params.sort;
        let desc = self.params.descending;
        builder
            .header(22.0, |mut h| {
                for c in Column::ALL {
                    // ⚠ The click target is the Response returned by `col()`. `row.response()` is
                    //   the UNION of the cells added so far and would fire for every column.
                    let (_, resp) = h.col(|ui| {
                        let mark = if c == sort {
                            if desc { " ▼" } else { " ▲" }
                        } else {
                            ""
                        };
                        ui.label(RichText::new(format!("{}{mark}", c.title())).small().strong());
                    });
                    if resp.clicked() {
                        clicked = Some(c);
                    }
                }
            })
            .body(|body| {
                let rows = &m.rows;
                body.rows(19.0, rows.len(), |mut row| {
                    // ⚠ Borrow the FIELD, not `self`: a `self.foo()` accessor here would borrow the
                    //   whole struct and the click handler below could not write back.
                    let r = &rows[row.index()];
                    let flashing = self
                        .flash
                        .get(&r.key)
                        .map(|t| (1.0 - (now - t) / FLASH_S).clamp(0.0, 1.0))
                        .unwrap_or(0.0);
                    if r.kind == RowKind::GroupHeader {
                        row.set_overline(true);
                    }
                    // ⭐ Selection wins over the flash tint: a row you clicked must stay visibly
                    //    picked even while a value change is animating on it.
                    let is_sel = self.selected.as_deref() == Some(r.key.as_str());
                    row.set_selected(is_sel || flashing > 0.0);

                    for c in Column::ALL {
                        row.col(|ui| {
                            cell_ui(ui, r, c, flashing as f32);
                        });
                    }
                    // ⚠ `row.response()` is the union of the cells added so far, which is what we
                    //    want HERE (click anywhere on the row) and is exactly what the header loop
                    //    must NOT use. Both idioms live in this file on purpose.
                    if row.response().clicked() {
                        picked = Some(r.key.clone());
                    }
                });
            });

        if let Some(k) = picked {
            // click the selected row again to dismiss - no close button to hunt for
            self.selected = if self.selected.as_deref() == Some(k.as_str()) { None } else { Some(k) };
        }

        if let Some(c) = clicked {
            if self.params.sort == c {
                self.params.descending = !self.params.descending;
            } else {
                self.params.sort = c;
                self.params.descending = false;
            }
        }
    }
}

/// ⛔ THE ONLY PLACE A CELL IS DRAWN, and it draws [`crate::table::TableRow::cell`] verbatim. It
/// picks a colour and an indent; it does not compute, truncate or re-format a value.
fn cell_ui(ui: &mut egui::Ui, r: &crate::table::TableRow, c: Column, flash: f32) {
    let text = r.cell(c);
    if r.kind == RowKind::GroupHeader {
        let t = RichText::new(text).small().strong().color(Palette::ACCENT);
        ui.label(t);
        return;
    }
    if r.depth > 0 && c == Column::Name {
        ui.add_space(12.0);
    }
    let mut t = match c {
        Column::Addr | Column::Value => RichText::new(text).small().monospace(),
        Column::State => RichText::new(text).small().monospace().color(state_colour(r.state)),
        Column::Badge => RichText::new(text).small().color(badge_colour(&r.badge)),
        _ => RichText::new(text).small(),
    };
    if r.stale && c == Column::State {
        t = t.strikethrough();
    }
    if flash > 0.0 && matches!(c, Column::Value | Column::State) {
        t = t.background_color(Palette::ACCENT.gamma_multiply(0.35 * flash));
    }
    let resp = ui.label(t);
    // The evidence behind the cell, on hover - not in a cell, because it is prose.
    if !r.liveness.is_empty() || r.note.is_some() || !r.resolve_trace.is_empty() {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(560.0);
            ui.label(RichText::new(&r.key).monospace().strong());
            ui.label(RichText::new(format!("{} · {}", r.confidence, r.reach)).small().color(Palette::MUTED));
            if !r.liveness.is_empty() {
                ui.label(RichText::new(format!("liveness  {}", r.liveness)).small());
            }
            if let Some(n) = &r.note {
                ui.label(RichText::new(n).small().color(Palette::MUTED));
            }
            if !r.resolve_trace.is_empty() {
                ui.separator();
                // ⭐ Every hop. A resolution nobody saw is a resolution nobody checked.
                for hop in &r.resolve_trace {
                    ui.label(RichText::new(hop).small().monospace().color(Palette::MUTED));
                }
            }
            let d = r.badge.detail();
            if !d.is_empty() {
                ui.separator();
                ui.label(RichText::new(d).small().color(badge_colour(&r.badge)));
            }
        });
    }
}

fn badge_colour(b: &crate::table::EventBadge) -> Color32 {
    use crate::table::EventBadge as B;
    match b {
        B::None => Palette::MUTED,
        B::Armed { .. } => Palette::OK,
        B::ArmedNothingYet { .. } => Palette::MUTED,
        // ⭐ A real negative is a RESULT, not a warning colour - it is the evidence the event plane
        //    exists to produce, and painting it red would teach the reader to treat it as a fault.
        B::ArmedNeverFired { .. } => Palette::VIOLET,
        B::Held => Palette::MUTED,
        B::SlotFull { .. } | B::Refused { .. } => Palette::WARN,
        B::Failed { .. } => Palette::ERROR,
    }
}

fn child_line(c: &ChildStatus) -> (String, Color32) {
    match c {
        ChildStatus::None => (String::new(), Palette::MUTED),
        ChildStatus::Running { pid, target_pid, lease_remaining_s, hits, pending, .. } => (
            format!(
                "child {pid} armed on {target_pid} · lease {lease_remaining_s:.1}s · {hits} hit(s){}",
                if pending.is_empty() { String::new() } else { format!(" · {} slot(s) HELD not armed", pending.len()) }
            ),
            Palette::OK,
        ),
        ChildStatus::Finished { hits, stop_reason, .. } => {
            (format!("child finished ({stop_reason}) · {hits} hit(s)"), Palette::MUTED)
        }
        ChildStatus::NoHit { watched_s, why, .. } => {
            (format!("⭐ NO HIT in {watched_s:.1}s - a real negative. {why}"), Palette::VIOLET)
        }
        ChildStatus::Failed { stderr, .. } => {
            (format!("⛔ child FAILED: {}", stderr.lines().next().unwrap_or("")), Palette::ERROR)
        }
        ChildStatus::StopTimedOut { pid, waited_s, .. } => (
            format!("⛔⛔ child {pid} did not confirm its stop after {waited_s:.1}s - no further child may start"),
            Palette::ERROR,
        ),
        ChildStatus::Refused { why } => (format!("⛔ REFUSED: {why}"), Palette::WARN),
    }
}

/// Style, applied once. ⚠ `Context::style()`/`set_style()` are gone at this pin.
pub fn apply_theme(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        let r = CornerRadius::same(4);
        for w in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.corner_radius = r;
        }
        style.visuals.window_corner_radius = r;
        style.visuals.window_shadow = egui::Shadow::NONE;
        style.visuals.popup_shadow = egui::Shadow::NONE;
        style.visuals.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.45);
        style.visuals.selection.stroke.color = Palette::ACCENT;
        style.visuals.hyperlink_color = Palette::ACCENT;
    });
}

/// Host fonts if any are present; egui's defaults otherwise. ⚠ No binary asset is vendored for
/// this, and the lab box has no internet - so this is a deliberate half-measure, named.
pub fn install_fonts(ctx: &egui::Context) -> Vec<String> {
    const PROPORTIONAL: &[&str] = &[
        "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    ];
    const MONOSPACE: &[&str] = &[
        "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
        "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
    ];
    let mut defs = egui::FontDefinitions::default();
    let mut installed = Vec::new();
    for (family, candidates, key) in [
        (egui::FontFamily::Proportional, PROPORTIONAL, "host-proportional"),
        (egui::FontFamily::Monospace, MONOSPACE, "host-monospace"),
    ] {
        let Some((path, bytes)) = candidates.iter().find_map(|p| std::fs::read(p).ok().map(|b| (*p, b))) else {
            continue;
        };
        defs.font_data.insert(key.to_string(), Arc::new(egui::FontData::from_owned(bytes)));
        // Keep egui's own fonts behind it: dropping them loses every glyph the host font lacks -
        // and this UI's whole vocabulary is ⭐ ⛔ ⚠ ‹›.
        defs.families.entry(family).or_default().insert(0, key.to_string());
        installed.push(path.to_string());
    }
    if !installed.is_empty() {
        ctx.set_fonts(defs);
    }
    installed
}
