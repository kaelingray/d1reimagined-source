// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The `d1-dash` window. **egui/eframe 0.36.1**, API re-verified against the vendored sources on
//! this box and against `d1-trace`'s own shipping viewer - no egui API is invented here.
//!
//! Pinned API facts, because two of them contradict `historicaldocs/design/egui-practices.md` (that doc is
//! corrected in the same turn as this file):
//!
//! - ⛔ `ctx.style()` / `ctx.set_style()` **do not exist at 0.36.1**. Style is per-theme;
//!   [`egui::Context::all_styles_mut`] applies one set of tweaks to light *and* dark at once.
//!   The `global_style` path silently gives a one-theme-only app.
//! - ⛔ `SidePanel` / `TopBottomPanel` **do not exist at 0.36.1**. Use
//!   `egui::Panel::{top,left,right,bottom}(id).show(ui, …)`, panels before `CentralPanel`.
//! - ⭐ `eframe::App::{logic, ui}` **is** the current API (`logic` has a default impl, `ui` is
//!   required). `interface-architecture.md`'s warning that this is "0.34-era and likely stale" is
//!   itself the stale thing.
//!
//! ## The repaint discipline, which no other crate in this tree exercises
//!
//! Producers `tx.send()` **then** `ctx.request_repaint()` from the **producer side** (`Context` is
//! `Send + Sync + Clone` for exactly this). The UI never polls, and `request_repaint()` is never
//! called per frame - that pegs a core forever. Channel drains happen in [`DashApp::tick`], called
//! from `logic()`, so the trait impl stays a two-line shim.
//!
//! ## Virtualization
//!
//! The tape is a `ScrollArea::show_rows` over a bounded `VecDeque` with a uniform line height. That
//! is what keeps 20,000 lines at 60 fps; iterating the data instead is the standard way an egui log
//! view dies.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, TextStyle};

use super::present::{Board, Cell, Lane, Row};
use super::probe::Descriptors;
use super::{probe, tail, Session};

/// How often the tail threads look for appended bytes. Bursts are coalesced; the UI never repaints
/// per log line.
const TAIL_PERIOD: Duration = Duration::from_millis(250);
/// How often the live client is polled. **2-5 s. Never faster.**
const PROBE_PERIOD: Duration = Duration::from_secs(3);

/// Messages from the background threads.
enum Msg {
    Tail(tail::Batch),
    Live(Box<Descriptors>),
    Controls(Box<ControlsVerdict>),
    /// A producer thread ended. Closure is a **domain signal**, never a silently-ready branch.
    TailDied(String),
}

/// The verdict from running `d1-trace controls` once at startup, in a separate process.
struct ControlsVerdict {
    ok: bool,
    summary: String,
}

pub fn run(run_dir: std::path::PathBuf) -> Result<()> {
    let session = Session::open(&run_dir);
    let title = format!("d1-dash - {}", session.run_dir.display());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1720.0, 1000.0])
            .with_min_inner_size([1200.0, 700.0])
            .with_title(title.clone()),
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| Ok(Box::new(DashApp::new(cc, session)))),
    )
    .map_err(|e| anyhow!(e.to_string()))
}

/// The dashboard app. `pub(crate)` so `d1-ghost` (`crate::ghost::gui`) can embed the whole read
/// side by composition - the console panel registers first, then [`DashApp::draw`] fills the rest.
pub(crate) struct DashApp {
    session: Session,
    board: Board,
    live: Option<Descriptors>,
    rx: Receiver<Msg>,
    /// Kept so the channel stays open for the life of the app.
    _tx: Sender<Msg>,
    dead_producers: Vec<String>,
    controls: Option<ControlsVerdict>,
    /// UI-only state (egui-practices: domain state lives in the struct; this is neither).
    filter: String,
    selected_error: Option<usize>,
    last_data: Instant,
    fonts_installed: Vec<String>,
}

impl DashApp {
    /// The current presented board - read access for an embedding app's console (gate state, the
    /// GPU/taint lamps, the current rung).
    pub(crate) fn board(&self) -> &Board {
        &self.board
    }

    /// True once the live probe has returned at least one descriptor snapshot - a coarse proxy the
    /// ghost console uses for "is the client answering". NOT the d1guest L0-L3 ladder.
    pub(crate) fn live_ok(&self) -> bool {
        self.live.is_some()
    }

    pub(crate) fn new(cc: &eframe::CreationContext<'_>, session: Session) -> DashApp {
        apply_style(&cc.egui_ctx);
        let fonts_installed = install_fonts(&cc.egui_ctx);
        let board = session.board(None);

        let (tx, rx) = std::sync::mpsc::channel::<Msg>();

        // ---- the tail thread ----------------------------------------------------------------
        {
            let ctx = cc.egui_ctx.clone();
            let tx = tx.clone();
            let path = session.emulator.path.clone();
            let offset = session.emulator.offset();
            let line_no = session.emulator.next_line_no();
            std::thread::Builder::new()
                .name("d1-dash-tail".into())
                .spawn(move || {
                    let mut t = tail::Tailer::resuming_at(path, offset, line_no);
                    let mut consecutive_errors = 0u32;
                    loop {
                        let batch = t.poll();
                        if batch.error.is_empty() {
                            consecutive_errors = 0;
                        } else {
                            consecutive_errors += 1;
                            if consecutive_errors >= 5 {
                                // A producer that gives up says so. Silence would render as a
                                // quiet run, which is the failure this whole page exists to stop.
                                let _ = tx.send(Msg::TailDied(format!(
                                    "the emulator.log tailer gave up after 5 consecutive read \
                                     errors ({}). Every ladder/tape fact below is FROZEN at the \
                                     moment it stopped.",
                                    batch.error
                                )));
                                ctx.request_repaint();
                                return;
                            }
                        }
                        let had = !batch.lines.is_empty()
                            || !batch.error.is_empty()
                            || batch.truncated;
                        if had {
                            // Send BEFORE the repaint poke: a wake that arrives before the data is
                            // a wasted frame.
                            if tx.send(Msg::Tail(batch)).is_err() {
                                return;
                            }
                            ctx.request_repaint();
                        }
                        std::thread::sleep(TAIL_PERIOD);
                    }
                })
                .ok();
        }

        // ---- the live-memory poll thread ------------------------------------------------------
        {
            let ctx = cc.egui_ctx.clone();
            let tx = tx.clone();
            let near = session.run_dir.clone();
            std::thread::Builder::new()
                .name("d1-dash-live".into())
                .spawn(move || loop {
                    let d = probe::poll(&near);
                    if tx.send(Msg::Live(Box::new(d))).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                    std::thread::sleep(PROBE_PERIOD);
                })
                .ok();
        }

        // ---- the controls verdict, once ------------------------------------------------------
        {
            let ctx = cc.egui_ctx.clone();
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("d1-dash-controls".into())
                .spawn(move || {
                    let v = run_controls();
                    let _ = tx.send(Msg::Controls(Box::new(v)));
                    ctx.request_repaint();
                })
                .ok();
        }

        DashApp {
            session,
            board,
            live: None,
            rx,
            _tx: tx,
            dead_producers: Vec::new(),
            controls: None,
            filter: String::new(),
            selected_error: None,
            last_data: Instant::now(),
            fonts_installed,
        }
    }

    /// Drain every channel and rebuild the board when something actually changed.
    fn tick(&mut self, ctx: &egui::Context) {
        let mut dirty = false;
        loop {
            match self.rx.try_recv() {
                Ok(Msg::Tail(batch)) => {
                    self.session.absorb(&batch);
                    dirty = true;
                }
                Ok(Msg::Live(d)) => {
                    self.live = Some(*d);
                    dirty = true;
                }
                Ok(Msg::Controls(v)) => {
                    self.controls = Some(*v);
                    dirty = true;
                }
                Ok(Msg::TailDied(why)) => {
                    self.dead_producers.push(why);
                    dirty = true;
                }
                Err(TryRecvError::Empty) => break,
                // §9.7: closure is a domain signal, not a silently-ready branch.
                Err(TryRecvError::Disconnected) => {
                    if self.dead_producers.is_empty() {
                        self.dead_producers.push(
                            "THE TAILER DIED - every push lane below is frozen at the moment it \
                             stopped. This is not a quiet run."
                                .into(),
                        );
                        dirty = true;
                    }
                    break;
                }
            }
        }
        if dirty {
            self.board = self.session.board(self.live.as_ref());
            self.last_data = Instant::now();
        }
        // Scoped to when data is flowing; idle otherwise. NOT a per-frame repaint.
        if self.last_data.elapsed() < Duration::from_secs(10) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl eframe::App for DashApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(root);
    }
}

impl DashApp {
    /// `pub(crate)` so `d1-ghost` can call the read side after registering its console panel. It
    /// still owns `root`'s remaining space via its own `CentralPanel`, so a panel the caller
    /// registers first sits outside it.
    pub(crate) fn tick_public(&mut self, ctx: &egui::Context) {
        self.tick(ctx);
    }

    pub(crate) fn draw(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("d3-header").show(root, |ui| self.header(ui));
        egui::Panel::left("d1-ladder")
            .resizable(true)
            .default_size(360.0)
            .show(root, |ui| self.ladder_panel(ui));
        egui::Panel::right("d4-d5")
            .resizable(true)
            .default_size(560.0)
            .show(root, |ui| self.right_panel(ui));
        egui::Panel::bottom("d2-errors")
            .resizable(true)
            .default_size(190.0)
            .show(root, |ui| self.error_panel(ui));
        egui::CentralPanel::default().show(root, |ui| self.tape_panel(ui));
    }

    // ---- D3 ---------------------------------------------------------------------------------
    fn header(&mut self, ui: &mut egui::Ui) {
        let b = &self.board;
        ui.horizontal_wrapped(|ui| {
            ui.heading("d1-dash");
            ui.label(RichText::new(&b.run_id).monospace().color(Palette::ACCENT));
            ui.label(
                RichText::new("READ-ONLY · synthetic_ancestor")
                    .small()
                    .color(Palette::MUTED),
            )
            .on_hover_text(
                "d1-dash never writes to the guest, never POSTs and never launches a run. \
                 Everything it shows is downstream of answers our own server invented.",
            );
        });

        if b.instrument_fault {
            banner(
                ui,
                "⛔ INSTRUMENT FAULT - no guest narration in emulator.log. NO RUNG IS REPORTED.",
                b.instrument_note.as_deref().unwrap_or(""),
            );
        }
        if let Some(c) = &self.controls {
            if !c.ok {
                banner(
                    ui,
                    "⛔ d1-trace controls FAILED - every derived tile on this page is greyed and \
                     may not be cited.",
                    &c.summary,
                );
            }
        }
        for d in &self.dead_producers {
            banner(ui, "⛔ A PRODUCER DIED", d);
        }

        ui.horizontal_wrapped(|ui| {
            for row in &b.header {
                cell_chip(ui, row);
            }
            match &self.controls {
                Some(c) => {
                    ui.label(
                        RichText::new(format!(
                            "{} d1-trace controls",
                            if c.ok { "✅" } else { "⛔" }
                        ))
                        .color(if c.ok { Palette::OK } else { Palette::ERR }),
                    )
                    .on_hover_text(&c.summary);
                }
                None => {
                    ui.label(RichText::new("⬜ d1-trace controls - running…").color(Palette::MUTED));
                }
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("lanes:").color(Palette::MUTED));
            for l in &b.lanes {
                lane_chip(ui, l);
            }
        });
    }

    // ---- D1 ---------------------------------------------------------------------------------
    fn ladder_panel(&mut self, ui: &mut egui::Ui) {
        let b = &self.board;
        ui.strong("THE LADDER");
        ui.label(
            RichText::new(&b.ladder_source)
                .small()
                .color(Palette::MUTED),
        )
        .on_hover_text(
            "The rung table is shelled out to d1-eboot ladder --json - the client's \
             own RTTI - never transcribed into a Rust array.",
        );

        match (&b.furthest, b.instrument_fault) {
            (_, true) => {
                ui.label(
                    RichText::new("NO RUNG")
                        .size(34.0)
                        .color(Palette::ERR)
                        .strong(),
                );
            }
            (Some(f), false) => {
                ui.label(
                    RichText::new(format!(
                        "rung {} of {}",
                        f.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                        b.max_rung
                    ))
                    .size(34.0)
                    .color(Palette::ACCENT)
                    .strong(),
                )
                .on_hover_text("FURTHEST rung entered - max(), not last().");
                ui.label(RichText::new(&f.state).monospace().color(Palette::ACCENT));
            }
            (None, false) => {
                ui.label(RichText::new("- no transitions -").size(22.0).color(Palette::MUTED));
            }
        }
        if let Some(c) = &b.current {
            ui.label(
                RichText::new(format!(
                    "sitting in: {} ({})",
                    c.state,
                    c.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into())
                ))
                .monospace()
                .color(if c.reason.as_deref().unwrap_or("").starts_with("error.") {
                    Palette::ERR
                } else {
                    Palette::OK
                }),
            )
            .on_hover_text(
                "A rung the client is SITTING IN is a different fact from one it passed through.",
            );
        }
        ui.separator();

        let furthest = b.furthest.as_ref().and_then(|f| f.rung).unwrap_or(0);
        let current = b.current.as_ref().and_then(|c| c.rung);
        let counts = &b.state_counts;
        let rungs = b.rung_table.clone();
        egui::ScrollArea::vertical()
            .id_salt("ladder")
            .auto_shrink([false, false])
            .show_rows(ui, 17.0, rungs.len(), |ui, range| {
                for i in range {
                    let r = &rungs[i];
                    let n = counts.get(&r.state).copied().unwrap_or(0);
                    let is_current = current == Some(r.rung);
                    let (glyph, color) = if is_current {
                        ("▶", Palette::ACCENT)
                    } else if n > 0 {
                        ("·", Palette::OK)
                    } else if r.rung <= furthest {
                        // Passed the rung number but never named this state: a conditional branch.
                        ("˙", Palette::MUTED)
                    } else {
                        (" ", Palette::MUTED.gamma_multiply(0.55))
                    };
                    let repeat = if n > 1 { format!("  ×{n}") } else { String::new() };
                    ui.label(
                        RichText::new(format!("{glyph} {:>2}  {}{repeat}", r.rung, r.state))
                            .font(FontId::monospace(12.0))
                            .color(color),
                    );
                }
            });
    }

    // ---- D2 ---------------------------------------------------------------------------------
    fn error_panel(&mut self, ui: &mut egui::Ui) {
        let b = &self.board;
        ui.horizontal(|ui| {
            ui.strong("THE ERROR TAPE");
            ui.label(
                RichText::new(format!("{} error.* observed", b.errors.len()))
                    .color(if b.errors.is_empty() {
                        Palette::MUTED
                    } else {
                        Palette::ERR
                    }),
            );
        });
        // ⛔ The empty state never reads "0 errors" in green.
        ui.label(
            RichText::new(&b.error_note)
                .italics()
                .color(Palette::WARN),
        );
        let errors = b.errors.clone();
        egui::ScrollArea::vertical()
            .id_salt("errors")
            .auto_shrink([false, false])
            .show_rows(ui, 19.0, errors.len(), |ui, range| {
                for i in range {
                    let e = &errors[i];
                    let code = e.codename.map(|c| format!("  [\"{c}\"]")).unwrap_or_default();
                    let text = format!(
                        "{:>6}  {}  →  {}{}   left rung {}",
                        e.line_no,
                        e.left_state,
                        e.entered_state.clone().unwrap_or_else(|| "?".into()),
                        code,
                        e.rung.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                    );
                    let selected = self.selected_error == Some(i);
                    if ui
                        .add_sized(
                            [ui.available_width(), 19.0],
                            egui::Button::selectable(
                                selected,
                                RichText::new(text)
                                    .font(FontId::monospace(12.0))
                                    .color(Palette::ERR),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&e.raw)
                        .clicked()
                    {
                        self.selected_error = Some(i);
                    }
                    if selected {
                        ui.label(
                            RichText::new(&e.reason)
                                .font(FontId::monospace(12.0))
                                .color(Palette::WARN),
                        );
                        ui.label(
                            RichText::new(&e.raw)
                                .font(FontId::monospace(11.0))
                                .color(Palette::MUTED),
                        );
                    }
                }
            });
    }

    // ---- D4 + D5 ----------------------------------------------------------------------------
    fn right_panel(&mut self, ui: &mut egui::Ui) {
        let b = &self.board;
        egui::ScrollArea::vertical()
            .id_salt("right")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.strong("ACTIVITY-CLIENT FLAG STRIP");
                if let Some(ac) = &b.ac {
                    ui.label(
                        RichText::new(&ac.raw_strip)
                            .font(FontId::monospace(12.0))
                            .color(Palette::MUTED),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    for row in &b.ac_rows {
                        cell_chip(ui, row);
                    }
                });

                ui.add_space(6.0);
                ui.strong("KTOBUCKET (verbatim)");
                ui.label(
                    RichText::new(&b.ktobucket_note)
                        .small()
                        .italics()
                        .color(Palette::WARN),
                );
                for k in &b.ktobucket {
                    ui.label(
                        RichText::new(&k.text)
                            .font(FontId::monospace(11.0))
                            .color(Palette::MUTED),
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.strong("QUEUEZ FAMILIES - client's own descriptor table (AUTHORITATIVE)");
                ui.label(
                    RichText::new(&b.family_source)
                        .small()
                        .color(Palette::MUTED),
                );
                for row in &b.liveness {
                    cell_row(ui, row);
                }
                for row in &b.families {
                    cell_row(ui, row);
                }

                ui.add_space(8.0);
                ui.strong("QUEUEZ log lane - CORROBORATING ONLY, never merged");
                ui.label(
                    RichText::new(
                        "state 2 is written LOCALLY at send time, so a send log proves nothing \
                         about replication. Two columns, never one.",
                    )
                    .small()
                    .italics()
                    .color(Palette::WARN),
                );
                for row in &b.queuez_log {
                    cell_row(ui, row);
                }

                if !b.notes.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.strong("notes");
                    for n in &b.notes {
                        ui.label(
                            RichText::new(n)
                                .font(FontId::monospace(11.0))
                                .color(if n.starts_with('⚠') || n.starts_with('⛔') {
                                    Palette::ERR
                                } else {
                                    Palette::MUTED
                                }),
                        );
                    }
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("fonts: {}", if self.fonts_installed.is_empty() {
                        "egui defaults (no host font found)".to_string()
                    } else {
                        self.fonts_installed.join(", ")
                    }))
                    .small()
                    .color(Palette::MUTED),
                );
            });
    }

    // ---- D6 ---------------------------------------------------------------------------------
    fn tape_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("TTY TAPE");
            ui.label(
                RichText::new(format!(
                    "{} guest lines of {} raw ({:.2}%)",
                    self.board.guest_lines,
                    self.board.raw_lines,
                    if self.board.raw_lines == 0 {
                        0.0
                    } else {
                        100.0 * self.board.guest_lines as f64 / self.board.raw_lines as f64
                    }
                ))
                .color(Palette::MUTED),
            )
            .on_hover_text(
                "89% of emulator.log is Render.Vulkan spam. Only `log_flush: [stdout] ` lines carry \
                 a guest fact, and only those are kept.",
            );
            ui.separator();
            ui.label("filter");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .desired_width(360.0)
                    .hint_text("substring, e.g. queuez / activity_client / KTOBUCKET"),
            );
        });
        ui.separator();

        let needle = self.filter.to_ascii_lowercase();
        let rows: Vec<&super::reduce::TtyLine> = self
            .session
            .reducer
            .tape
            .iter()
            .filter(|l| needle.is_empty() || l.text.to_ascii_lowercase().contains(&needle))
            .collect();

        if rows.is_empty() && self.board.guest_lines == 0 && self.board.raw_lines > 0 {
            ui.label(
                RichText::new(
                    "⛔ empty tape over a live, growing file = PARSER FAULT or the guest-diagnostic \
                     sink was never patched. It is NOT 'the game produced nothing'.",
                )
                .color(Palette::ERR),
            );
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("tape")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show_rows(ui, 15.0, rows.len(), |ui, range| {
                for i in range {
                    let l = rows[i];
                    ui.label(
                        RichText::new(format!("{:>7}  {}", l.line_no, l.text))
                            .font(FontId::monospace(11.5))
                            .color(if l.is_error {
                                Palette::ERR
                            } else {
                                subsystem_color(&l.subsystem)
                            }),
                    );
                }
            });
    }
}

// ---------------------------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------------------------

/// A three-state chip. ⬜ and ✅ differ in **shape as well as colour** - a green zero has already
/// published a whole retracted causal theory in this project.
fn cell_chip(ui: &mut egui::Ui, row: &Row) {
    let (color, prefix) = cell_style(row.state);
    let text = RichText::new(format!("{prefix} {}: {}", row.label, row.value))
        .font(FontId::monospace(12.0))
        .color(color);
    let text = if row.state == Cell::NotObserved {
        text.weak()
    } else {
        text
    };
    ui.label(text).on_hover_text(hover(row));
    ui.separator();
}

fn cell_row(ui: &mut egui::Ui, row: &Row) {
    let (color, prefix) = cell_style(row.state);
    ui.label(
        RichText::new(format!("{prefix} {:<26} {}", row.label, row.value))
            .font(FontId::monospace(11.5))
            .color(color),
    )
    .on_hover_text(hover(row));
}

fn hover(row: &Row) -> String {
    let mut s = format!("[{}]  {}", row.state.token(), row.src.hover());
    if let Some(n) = &row.note {
        s.push_str("\n\n");
        s.push_str(n);
    }
    if let Some(r) = &row.raw {
        s.push_str("\n\nRAW (unmodified):\n");
        s.push_str(r);
    }
    s
}

fn cell_style(c: Cell) -> (Color32, &'static str) {
    match c {
        Cell::Ok => (Palette::OK, "✅"),
        Cell::Empty => (Palette::WARN, "⚠"),
        Cell::NotObserved => (Palette::MUTED, "⬜"),
    }
}

fn lane_chip(ui: &mut egui::Ui, l: &Lane) {
    let (color, prefix) = cell_style(l.state);
    let text = match l.behind_secs {
        Some(0) => format!("{prefix} {}", l.file),
        Some(n) => format!("{prefix} {} (-{n}s)", l.file),
        None => format!("{prefix} {} (absent)", l.file),
    };
    ui.label(
        RichText::new(text)
            .font(FontId::monospace(11.5))
            .color(color),
    )
    .on_hover_text(format!(
        "{} bytes · {}",
        l.bytes,
        if l.note.is_empty() {
            "keeping pace with the newest lane in this run".to_string()
        } else {
            l.note.clone()
        }
    ));
}

fn banner(ui: &mut egui::Ui, headline: &str, detail: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(0x3A, 0x18, 0x1C))
        .inner_margin(egui::Margin::same(6))
        .corner_radius(egui::CornerRadius::same(4))
        .show(ui, |ui| {
            ui.label(RichText::new(headline).color(Palette::ERR).strong());
            if !detail.is_empty() {
                ui.label(RichText::new(detail).small().color(Palette::WARN));
            }
        });
}

fn subsystem_color(sub: &str) -> Color32 {
    if sub.starts_with("world_controller:state_manager") {
        Palette::ACCENT
    } else if sub.starts_with("world_controller") {
        Palette::GUEST
    } else if sub.starts_with("networking") {
        Palette::HTTP
    } else {
        Palette::MUTED
    }
}

pub(crate) struct Palette;
impl Palette {
    pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x6C, 0xB6, 0xFF);
    pub(crate) const MUTED: Color32 = Color32::from_rgb(0x8A, 0x93, 0xA3);
    pub(crate) const OK: Color32 = Color32::from_rgb(0x8C, 0xC2, 0x65);
    pub(crate) const WARN: Color32 = Color32::from_rgb(0xE5, 0xC0, 0x7B);
    pub(crate) const ERR: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);
    const GUEST: Color32 = Color32::from_rgb(0xC9, 0x9A, 0xF0);
    const HTTP: Color32 = Color32::from_rgb(0x7A, 0xC6, 0x9A);
}

/// ⛔ `ctx.style()` / `set_style()` do not exist at 0.36.1. `all_styles_mut` applies to BOTH themes;
/// the `global_style` path silently gives a one-theme-only app.
fn apply_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        let r = egui::CornerRadius::same(4);
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
        style.visuals.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.55);
        style.visuals.selection.stroke.color = Palette::ACCENT;
        style.text_styles = std::collections::BTreeMap::from([
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]);
    });
}

const PROPORTIONAL: &[&str] = &[
    "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
    "/usr/share/fonts/google-noto-sans-fonts/NotoSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
];
const MONOSPACE: &[&str] = &[
    "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
    "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
];

/// Host fonts, inserted at the **front** of the family so egui's own remain the glyph fallback
/// chain - dropping them loses the glyphs the host font does not have (including ⭐ and ⬜).
fn install_fonts(ctx: &egui::Context) -> Vec<String> {
    let mut defs = egui::FontDefinitions::default();
    let mut installed = Vec::new();
    for (family, candidates, key) in [
        (FontFamily::Proportional, PROPORTIONAL, "host-prop"),
        (FontFamily::Monospace, MONOSPACE, "host-mono"),
    ] {
        let Some((path, bytes)) = candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok().map(|b| (*p, b)))
        else {
            continue;
        };
        defs.font_data
            .insert(key.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families
            .entry(family)
            .or_default()
            .insert(0, key.to_string());
        installed.push(path.to_string());
    }
    if !installed.is_empty() {
        ctx.set_fonts(defs);
    }
    installed
}

/// Run `d1-trace controls` in a separate process. **Not** an in-process call: the controls load a
/// pinned run and take ~1.2 s, and a viewer that blocks its own startup on them is a viewer nobody
/// opens.
fn run_controls() -> ControlsVerdict {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("d1-trace")));
    let Some(exe) = exe.filter(|p| p.is_file()) else {
        return ControlsVerdict {
            ok: false,
            summary: "d1-trace binary not found next to d1-dash - the controls verdict could NOT \
                      BE EVALUATED, which is a FAILED control, not a skipped one."
                .into(),
        };
    };
    match std::process::Command::new(&exe).arg("controls").output() {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let last = text
                .lines()
                .rev()
                .find(|l| l.contains("passed,"))
                .unwrap_or("(no summary line)")
                .trim()
                .to_string();
            ControlsVerdict {
                ok: o.status.success(),
                summary: format!("{} - {last}", exe.display()),
            }
        }
        Err(e) => ControlsVerdict {
            ok: false,
            summary: format!("running {}: {e}", exe.display()),
        },
    }
}
