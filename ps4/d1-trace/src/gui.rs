// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The egui viewer. API shape verified against d1-observe's shipping 0.36.1 GUI (the `ui()` split,
//! `egui::Panel::*`, `ScrollArea::show_rows`, `ctx.global_style()`). No new egui API is invented.
//!
//! egui-practices discipline: all domain state lives in the app struct; the visible-row list is
//! cached and rebuilt only when the query/filter inputs change; timeline rows are virtualized; a
//! restrained style + host fonts are installed once; and being post-hoc, the app makes no periodic
//! repaint request - it goes idle between inputs.

use anyhow::{anyhow, Result};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, TextStyle};

use crate::model::{Stream, Trace};
use crate::query::Query;

/// Optional startup filter - lets a run be opened pre-queried (also used for deterministic tests).
#[derive(Default, Clone)]
pub struct Startup {
    pub query: String,
    pub matches_only: bool,
}

pub fn run(trace: Trace) -> Result<()> {
    run_with(trace, Startup::default())
}

pub fn run_with(trace: Trace, startup: Startup) -> Result<()> {
    let title = format!("d1-trace - {}", trace.summary.run_id);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1560.0, 940.0])
            .with_min_inner_size([1080.0, 640.0])
            .with_title(title.clone()),
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| Ok(Box::new(ViewerApp::new(cc, trace, startup)))),
    )
    .map_err(|e| anyhow!(e.to_string()))
}

struct StreamToggles {
    http: bool,
    bap: bool,
    probe: bool,
    guest: bool,
}

impl Default for StreamToggles {
    fn default() -> Self {
        Self {
            http: true,
            bap: true,
            probe: true,
            guest: true,
        }
    }
}

impl StreamToggles {
    fn allows(&self, s: Stream) -> bool {
        match s {
            Stream::Http => self.http,
            Stream::Bap => self.bap,
            Stream::Probe => self.probe,
            Stream::Guest => self.guest,
        }
    }
}

struct ViewerApp {
    trace: Trace,
    query_text: String,
    query: Query,
    streams: StreamToggles,
    matches_only: bool,
    /// Cached visible row indices (into `trace.events`) and the inputs that produced them.
    visible: Vec<usize>,
    /// Set of matching ids, for highlighting when not in matches-only mode.
    matched: Vec<bool>,
    filter_key: (String, bool, bool, bool, bool, bool),
    selected: Option<usize>,
}

impl ViewerApp {
    fn new(cc: &eframe::CreationContext<'_>, trace: Trace, startup: Startup) -> Self {
        configure_style(&cc.egui_ctx);
        let matched = vec![true; trace.events.len()];
        let mut app = Self {
            visible: Vec::new(),
            matched,
            trace,
            query_text: startup.query,
            query: Query::default(),
            streams: StreamToggles::default(),
            matches_only: startup.matches_only,
            // A sentinel key that cannot equal a real one, so the first refresh always runs.
            filter_key: ("\u{0}unset".to_string(), false, false, false, false, false),
            selected: None,
        };
        app.refresh();
        app.selected = app.visible.first().copied();
        app
    }

    /// Rebuild the visible list only when an input actually changed (egui-practices: cache filters).
    fn refresh(&mut self) {
        let key = (
            self.query_text.clone(),
            self.matches_only,
            self.streams.http,
            self.streams.bap,
            self.streams.probe,
            self.streams.guest,
        );
        if key == self.filter_key && !self.visible.is_empty() {
            return;
        }
        self.filter_key = key;
        self.query = Query::parse(&self.query_text);
        let has_query = !self.query.is_empty();

        self.visible.clear();
        for (i, ev) in self.trace.events.iter().enumerate() {
            let is_match = !has_query || self.query.matches(ev);
            self.matched[i] = is_match;
            if !self.streams.allows(ev.stream) {
                continue;
            }
            if self.matches_only && has_query && !is_match {
                continue;
            }
            self.visible.push(i);
        }
    }

    fn match_count(&self) -> usize {
        self.matched.iter().filter(|m| **m).count()
    }
}

impl eframe::App for ViewerApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh();

        egui::Panel::top("header").show(root_ui, |ui| {
            let s = &self.trace.summary;
            ui.horizontal(|ui| {
                ui.heading("d1-trace");
                ui.separator();
                ui.monospace(&s.run_id);
            });
            ui.horizontal_wrapped(|ui| {
                lane_chip(ui, Stream::Guest, s.guest);
                lane_chip(ui, Stream::Http, s.http);
                lane_chip(ui, Stream::Bap, s.bap);
                lane_chip(ui, Stream::Probe, s.probe);
                ui.separator();
                let rate = if s.bap_anchorable == 0 {
                    0.0
                } else {
                    100.0 * s.bap_anchored as f64 / s.bap_anchorable as f64
                };
                ui.label(
                    RichText::new(format!(
                        "BAP anchored {}/{} ({rate:.0}%)",
                        s.bap_anchored, s.bap_anchorable
                    ))
                    .color(Palette::MUTED),
                )
                .on_hover_text(
                    "BAP frames pinned to the guest probe spine by (msg_type, sequence). The \
                     denominator is rows that CARRY an id - outer `BAP TCP msg#N` headers have no \
                     type/seq and are folded into the inner decode, so they are not counted.",
                );
                if !s.spine {
                    ui.separator();
                    ui.label(
                        RichText::new("⚠ NO CROSS-STREAM SPINE - lane-grouped, every row ≈")
                            .color(Palette::ERR)
                            .strong(),
                    )
                    .on_hover_text(
                        "This run has no probe spine or no anchor onto it. Cross-stream order was \
                         NOT observed and has not been invented: lanes are shown one after another, \
                         ordered within lane only. Do not read any HTTP↔BAP↔guest ordering here.",
                    );
                }
                if let Some(bf) = &s.bootflow_reached {
                    ui.separator();
                    ui.label(RichText::new(format!("bootflow: {bf}")).color(Palette::OK));
                }
                if s.error_count > 0 {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("error.* × {}", s.error_count)).color(Palette::ERR),
                    );
                }
            });
        });

        egui::Panel::top("controls").show(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.query_text)
                        .desired_width(560.0)
                        .hint_text("probe:item_load_state_primary rax&0xffffffff==2"),
                );
                resp.on_hover_text(
                    "Terms ANDed. stream:<s> · probe:<substr> · <reg>[&mask]<op><val> \
                     (probe events only) · bare word = summary substring",
                );
                ui.checkbox(&mut self.matches_only, "matches only")
                    .on_hover_text("Narrow the timeline to matching rows");
                ui.separator();
                ui.label(RichText::new("lanes:").color(Palette::MUTED));
                ui.checkbox(&mut self.streams.guest, "guest");
                ui.checkbox(&mut self.streams.http, "http");
                ui.checkbox(&mut self.streams.bap, "bap");
                ui.checkbox(&mut self.streams.probe, "probe");
            });
            ui.horizontal_wrapped(|ui| {
                let errs = self.query.errors();
                let warns = self.query.warnings();
                if !errs.is_empty() {
                    ui.label(
                        RichText::new(format!("unparsed: {}", errs.join(", ")))
                            .color(Palette::ERR),
                    );
                } else if !warns.is_empty() {
                    // F9c: unsatisfiable-by-construction is a fact about the QUERY, not the run.
                    ui.label(RichText::new(format!("⚠ {}", warns.join("; "))).color(Palette::ERR));
                } else if !self.query.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "{} match(es) · {} shown",
                            self.match_count(),
                            self.visible.len()
                        ))
                        .color(Palette::ACCENT),
                    );
                } else {
                    ui.label(RichText::new(format!("{} events", self.trace.events.len())).color(Palette::MUTED));
                }
                // F9a: a `probe:` typo returns 0 rows with no complaint. Say which it is.
                for term in self.query.probe_terms() {
                    if !self
                        .trace
                        .summary
                        .probe_names
                        .iter()
                        .any(|n| n.contains(term.as_str()))
                    {
                        ui.label(
                            RichText::new(format!(
                                "⚠ probe:{term} matches none of this run's {} probe names",
                                self.trace.summary.probe_names.len()
                            ))
                            .color(Palette::ERR),
                        )
                        .on_hover_text("A property of the query, not of the run - see the probe list below.");
                    }
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "order confidence:  = exact (real join key)   ~ bracketed (BETWEEN two \
                         anchors)   ≈ approx / extrapolated - no shared clock",
                    )
                    .color(Palette::MUTED)
                    .italics(),
                );
            });
        });

        egui::Panel::right("detail")
            .resizable(true)
            .default_size(560.0)
            .show(root_ui, |ui| {
                self.detail_pane(ui);
            });

        egui::Panel::bottom("notes")
            .resizable(true)
            .default_size(96.0)
            .show(root_ui, |ui| {
                ui.strong("Run provenance / notes");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for note in &self.trace.summary.notes {
                        let loud = note.starts_with('⚠');
                        ui.label(
                            RichText::new(note)
                                .font(FontId::monospace(11.5))
                                .color(if loud { Palette::ERR } else { Palette::MUTED }),
                        );
                    }
                    // F9a: the names exist; show them so a typo is checkable.
                    ui.collapsing(
                        format!("{} distinct probe names", self.trace.summary.probe_names.len()),
                        |ui| {
                            for chunk in self.trace.summary.probe_names.chunks(4) {
                                ui.label(
                                    RichText::new(chunk.join("   "))
                                        .font(FontId::monospace(11.0))
                                        .color(Palette::MUTED),
                                );
                            }
                        },
                    );
                });
            });

        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Unified timeline");
                ui.label(RichText::new("(lane · order · summary)").color(Palette::MUTED));
            });
            ui.separator();
            let visible = std::mem::take(&mut self.visible);
            let mut clicked = None;
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(ui, 20.0, visible.len(), |ui, range| {
                    for vi in range {
                        let idx = visible[vi];
                        let ev = &self.trace.events[idx];
                        let selected = self.selected == Some(idx);
                        let dim = !self.query.is_empty() && !self.matched[idx];
                        let color = lane_color(ev.stream, dim);
                        let label = format!(
                            "{:<5} {} {:>9.1} {}  {}",
                            ev.stream.tag(),
                            ev.confidence.marker(),
                            ev.order,
                            marker_for(ev.stream),
                            truncate(&ev.summary, 150),
                        );
                        let btn = egui::Button::selectable(
                            selected,
                            RichText::new(label).font(FontId::monospace(12.0)).color(color),
                        )
                        .truncate();
                        let w = ui.available_width();
                        if ui.add_sized([w, 20.0], btn).clicked() {
                            clicked = Some(idx);
                        }
                    }
                });
            self.visible = visible;
            if let Some(idx) = clicked {
                self.selected = Some(idx);
            }
        });

        // Post-hoc: no periodic repaint. Input drives redraw; the app idles otherwise.
    }
}

impl ViewerApp {
    fn detail_pane(&self, ui: &mut egui::Ui) {
        let Some(idx) = self.selected else {
            ui.label("Select an event.");
            return;
        };
        let ev = &self.trace.events[idx];
        ui.heading("Selected event");
        egui::Grid::new("sel").striped(true).num_columns(2).show(ui, |ui| {
            kv(ui, "stream", ev.stream.tag());
            kv(ui, "order", &format!("{:.2}  ({:?})", ev.order, ev.confidence));
            kv(ui, "native ordinal", &ev.ordinal.to_string());
            if let Some(bap) = &ev.bap {
                if let Some(t) = bap.msg_type {
                    kv(ui, "bap type", &format!("0x{t:04x}"));
                }
                if let Some(s) = bap.sequence {
                    kv(ui, "bap seq", &s.to_string());
                }
            }
        });

        if let Some(p) = &ev.probe {
            ui.separator();
            ui.strong(RichText::new("probe interpretation").color(Palette::ACCENT));
            for line in &p.explain {
                ui.label(RichText::new(line).font(FontId::monospace(12.0)));
            }
            ui.separator();
            ui.collapsing("registers", |ui| {
                egui::Grid::new("regs").striped(true).num_columns(2).show(ui, |ui| {
                    for name in [
                        "guest_vaddr", "rip", "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp",
                        "r12", "r13", "r14", "r15", "rflags",
                    ] {
                        if let Some(v) = p.registers.get(name) {
                            ui.label(RichText::new(name).color(Palette::MUTED).monospace());
                            ui.monospace(format!("0x{v:016x}"));
                            ui.end_row();
                        }
                    }
                });
            });
            ui.collapsing("memory samples (48B each)", |ui| {
                sample(ui, "sample_a", &p.sample_a);
                sample(ui, "sample_b", &p.sample_b);
            });
        } else {
            ui.separator();
            ui.strong("detail");
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for line in &ev.detail {
                    ui.label(RichText::new(line).font(FontId::monospace(11.5)));
                }
            });
        }

        ui.separator();
        ui.strong(RichText::new("nearby events (all streams)").color(Palette::ACCENT))
            .on_hover_text(
                "Events near this one on the unified timeline - how the streams explain each other. \
                 Same-stream neighbours are included so a probe's own context (e.g. the tag resolved \
                 on this iteration) is reachable.",
            );
        // Window scales with the spine; ±40 order units is a few dozen probe records.
        let window = 40.0_f64;
        egui::ScrollArea::vertical().max_height(300.0).id_salt("nearby").show(ui, |ui| {
            let mut shown = 0usize;
            for other in &self.trace.events {
                if other.id == ev.id || (other.order - ev.order).abs() > window {
                    continue;
                }
                shown += 1;
                if shown > 80 {
                    ui.label(RichText::new("… (window truncated)").color(Palette::MUTED));
                    break;
                }
                let same = other.stream == ev.stream;
                let mut text = RichText::new(format!(
                    "{:<5} {:>8.1} {}",
                    other.stream.tag(),
                    other.order,
                    truncate(&other.summary, 90)
                ))
                .font(FontId::monospace(11.0))
                .color(lane_color(other.stream, false));
                if same {
                    text = text.weak();
                }
                ui.label(text);
            }
        });
    }
}

struct Palette;
impl Palette {
    const ACCENT: Color32 = Color32::from_rgb(0x6C, 0xB6, 0xFF);
    const MUTED: Color32 = Color32::from_rgb(0x8A, 0x93, 0xA3);
    const OK: Color32 = Color32::from_rgb(0x8C, 0xC2, 0x65);
    const ERR: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);
    const GUEST: Color32 = Color32::from_rgb(0xC9, 0x9A, 0xF0);
    const HTTP: Color32 = Color32::from_rgb(0x7A, 0xC6, 0x9A);
    const BAP: Color32 = Color32::from_rgb(0xE5, 0xC0, 0x7B);
    const PROBE: Color32 = Color32::from_rgb(0xB8, 0xC2, 0xD0);
}

fn lane_color(stream: Stream, dim: bool) -> Color32 {
    let c = match stream {
        Stream::Guest => Palette::GUEST,
        Stream::Http => Palette::HTTP,
        Stream::Bap => Palette::BAP,
        Stream::Probe => Palette::PROBE,
    };
    if dim {
        c.gamma_multiply(0.35)
    } else {
        c
    }
}

/// A one-glyph lane marker; combined with colour it gives the visual lane separation without a 2D
/// canvas (v1). The glyph places the four lanes at four horizontal positions in the row text.
fn marker_for(stream: Stream) -> &'static str {
    match stream {
        Stream::Guest => "G···",
        Stream::Http => "·H··",
        Stream::Bap => "··B·",
        Stream::Probe => "···P",
    }
}

fn lane_chip(ui: &mut egui::Ui, stream: Stream, count: usize) {
    ui.label(
        RichText::new(format!("{} {}", stream.tag(), count))
            .color(lane_color(stream, false))
            .strong(),
    );
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.label(RichText::new(k).color(Palette::MUTED));
    ui.monospace(v);
    ui.end_row();
}

fn sample(ui: &mut egui::Ui, name: &str, hex: &str) {
    ui.label(RichText::new(name).color(Palette::MUTED).monospace());
    // Render hex in rows of 16 bytes with an ASCII gutter.
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect();
    for chunk in bytes.chunks(16) {
        let hexs: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let asc: String = chunk
            .iter()
            .map(|b| if (0x20..0x7f).contains(b) { *b as char } else { '.' })
            .collect();
        ui.label(
            RichText::new(format!("{:<47}  {}", hexs.join(" "), asc)).font(FontId::monospace(11.0)),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    style.visuals.window_shadow = egui::Shadow::NONE;
    style.visuals.popup_shadow = egui::Shadow::NONE;
    style.visuals.selection.bg_fill = Color32::from_rgb(52, 93, 145);
    style.visuals.panel_fill = Color32::from_rgb(21, 25, 31);
    style.visuals.window_fill = Color32::from_rgb(26, 31, 39);
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
    ctx.set_global_style(style);
    install_fonts(ctx);
}

fn install_fonts(ctx: &egui::Context) {
    let proportional = [
        "/usr/share/fonts/google-noto-sans-fonts/NotoSans-Regular.ttf",
        "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok());
    let monospace = [
        "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
        "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok());

    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = proportional {
        fonts
            .font_data
            .insert("prop".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "prop".into());
    }
    if let Some(bytes) = monospace {
        fonts
            .font_data
            .insert("mono".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "mono".into());
    }
    ctx.set_fonts(fonts);
}
