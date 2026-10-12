// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use anyhow::{anyhow, Result};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, TextStyle};

use crate::model::{Divergence, EvidenceStatus, Explanation, ReducedRun, RunData};
use crate::reducer::{first_divergence, reduce, snapshot_at};

pub fn run(run: RunData, compare: Option<RunData>) -> Result<()> {
    let title = format!("D1 Conversation Debugger - {}", run.run_id);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1050.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(move |creation| Ok(Box::new(InspectorApp::new(creation, run, compare)))),
    )
    .map_err(|error| anyhow!(error.to_string()))
}

struct InspectorApp {
    run: RunData,
    reduced: ReducedRun,
    compare: Option<RunData>,
    divergence: Option<Divergence>,
    selected: usize,
    explanation: Explanation,
    event_details: String,
    filter: String,
    failed_only: bool,
    visible: Vec<usize>,
    filter_key: (String, bool),
}

impl InspectorApp {
    fn new(
        creation: &eframe::CreationContext<'_>,
        run: RunData,
        compare: Option<RunData>,
    ) -> Self {
        configure_style(&creation.egui_ctx);
        let reduced = reduce(&run);
        let selected = reduced
            .diagnoses
            .first()
            .map(|diagnosis| diagnosis.event_index())
            .unwrap_or_else(|| run.events.len().saturating_sub(1));
        let explanation = if run.events.is_empty() {
            Explanation {
                event: crate::model::TimelineRow {
                    index: 0,
                    event_id: "empty".to_string(),
                    source: "none".to_string(),
                    source_sequence: 0,
                    monotonic_ns: None,
                    direction: "local".to_string(),
                    stage: "empty_run".to_string(),
                    status: "not_observed".to_string(),
                    connection_id: None,
                    message: "no events".to_string(),
                    inner_sequence: None,
                    evidence: EvidenceStatus::NotObserved,
                },
                snapshot: Default::default(),
                diagnosis: None,
            }
        } else {
            snapshot_at(&run, selected).expect("selected event is in range")
        };
        let event_details = run
            .events
            .get(selected)
            .and_then(|event| serde_json::to_string_pretty(&event.details).ok())
            .unwrap_or_else(|| "{}".to_string());
        let divergence = compare.as_ref().map(|other| first_divergence(&run, other));
        let visible = (0..run.events.len()).collect();
        Self {
            run,
            reduced,
            compare,
            divergence,
            selected,
            explanation,
            event_details,
            filter: String::new(),
            failed_only: false,
            visible,
            filter_key: (String::new(), false),
        }
    }

    fn select(&mut self, index: usize) {
        if index >= self.run.events.len() || index == self.selected {
            return;
        }
        self.selected = index;
        if let Ok(explanation) = snapshot_at(&self.run, index) {
            self.explanation = explanation;
        }
        self.event_details = serde_json::to_string_pretty(&self.run.events[index].details)
            .unwrap_or_else(|_| "{}".to_string());
    }

    fn refresh_filter(&mut self) {
        let key = (self.filter.to_ascii_lowercase(), self.failed_only);
        if key == self.filter_key {
            return;
        }
        self.filter_key = key.clone();
        self.visible = self
            .reduced
            .timeline
            .iter()
            .filter(|row| {
                let failed = row.status == "failed"
                    || matches!(row.evidence, EvidenceStatus::Confounded | EvidenceStatus::Missing);
                let text_match = key.0.is_empty()
                    || format!(
                        "{} {} {} {} {}",
                        row.source, row.stage, row.status, row.message, row.event_id
                    )
                    .to_ascii_lowercase()
                    .contains(&key.0);
                (!key.1 || failed) && text_match
            })
            .map(|row| row.index)
            .collect();
    }
}

impl eframe::App for InspectorApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh_filter();

        egui::Panel::top("toolbar").show(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("D1 Conversation Debugger");
                ui.separator();
                ui.monospace(&self.run.run_id);
                ui.separator();
                if ui
                    .add_enabled(self.selected > 0, egui::Button::new("◀ Previous"))
                    .on_hover_text("Step to the previous semantic event; this does not pause the guest")
                    .clicked()
                {
                    self.select(self.selected - 1);
                }
                if ui
                    .add_enabled(
                        self.selected + 1 < self.run.events.len(),
                        egui::Button::new("Next ▶"),
                    )
                    .on_hover_text("Step to the next semantic event; this is replay, not live execution control")
                    .clicked()
                {
                    self.select(self.selected + 1);
                }
                if !self.run.events.is_empty() {
                    let mut selected = self.selected;
                    let slider = egui::Slider::new(&mut selected, 0..=self.run.events.len() - 1)
                        .show_value(true)
                        .text("event");
                    if ui.add(slider).changed() {
                        self.select(selected);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.text_edit_singleline(&mut self.filter)
                    .on_hover_text("Filter source, stage, status, message label or stable event id");
                ui.checkbox(&mut self.failed_only, "failed/confounded only")
                    .on_hover_text("Not-observed coverage remains visible only when it is an event verdict");
                ui.separator();
                let ordering = match self.reduced.ordering {
                    EvidenceStatus::Observed => "global monotonic order observed",
                    _ => "cross-source order not proven",
                };
                ui.label(RichText::new(ordering).color(if self.reduced.ordering == EvidenceStatus::Observed {
                    Color32::from_rgb(137, 220, 165)
                } else {
                    Color32::from_rgb(240, 192, 110)
                }));
            });
        });

        egui::Panel::left("queue")
            .resizable(true)
            .default_size(390.0)
            .show(root_ui, |ui| {
                ui.heading("In-flight queue");
                ui.label(RichText::new(format!("after event {}", self.selected)).weak());
                if self.explanation.snapshot.queues.values().all(Vec::is_empty) {
                    ui.colored_label(
                        Color32::from_rgb(160, 170, 185),
                        "Empty or not observed. Server receipt alone is not promoted into client queue state.",
                    );
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (connection, queue) in &self.explanation.snapshot.queues {
                        ui.separator();
                        ui.strong(connection);
                        egui::Grid::new(format!("queue-{connection}"))
                            .striped(true)
                            .num_columns(5)
                            .show(ui, |ui| {
                                ui.strong("pos");
                                ui.strong("request");
                                ui.strong("seq");
                                ui.strong("expected");
                                ui.strong("evidence");
                                ui.end_row();
                                for (position, request) in queue.iter().enumerate() {
                                    ui.label(RichText::new(if position == 0 { "HEAD" } else { "" }).strong());
                                    ui.monospace(request.label());
                                    ui.monospace(request.sequence.to_string());
                                    ui.monospace(
                                        request
                                            .expected_reply_type
                                            .map(|value| format!("0x{value:04x}"))
                                            .unwrap_or_else(|| "unknown".to_string()),
                                    );
                                    ui.label(format!("{:?}", request.evidence));
                                    ui.end_row();
                                }
                            });
                    }
                });
            });

        egui::Panel::right("details")
            .resizable(true)
            .default_size(430.0)
            .show(root_ui, |ui| {
                ui.heading("Selected event");
                let row = &self.explanation.event;
                egui::Grid::new("selected-event").striped(true).show(ui, |ui| {
                    field(ui, "id", &row.event_id);
                    field(ui, "source", &format!("{}:{}", row.source, row.source_sequence));
                    field(ui, "stage", &row.stage);
                    field(ui, "status", &row.status);
                    field(ui, "message", &row.message);
                    field(
                        ui,
                        "sequence",
                        &row.inner_sequence.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
                    );
                    field(ui, "evidence", &format!("{:?}", row.evidence));
                });
                ui.separator();
                ui.heading("Why this event matters");
                if let Some(diagnosis) = &self.explanation.diagnosis {
                    ui.colored_label(Color32::from_rgb(245, 145, 135), diagnosis.explanation());
                } else {
                    ui.label("No causal diagnosis is proven at this event.");
                }
                ui.separator();
                ui.heading("State");
                egui::Grid::new("state-grid").striped(true).show(ui, |ui| {
                    field(
                        ui,
                        "boot state",
                        self.explanation.snapshot.boot_state.as_deref().unwrap_or("not observed"),
                    );
                    field(
                        ui,
                        "world",
                        self.explanation.snapshot.world.as_deref().unwrap_or("not observed"),
                    );
                    for (key, verdict) in &self.explanation.snapshot.queuez {
                        field(ui, key, &format!("{:?}", verdict));
                    }
                });
                ui.separator();
                ui.collapsing("Typed/redacted event details", |ui| {
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&self.event_details).monospace())
                                .wrap(),
                        );
                    });
                });
            });

        egui::Panel::bottom("verdict")
            .resizable(true)
            .default_size(105.0)
            .show(root_ui, |ui| {
                if let Some(divergence) = &self.divergence {
                    let compare_name = self.compare.as_ref().map(|run| run.run_id.as_str()).unwrap_or("?");
                    ui.strong(format!("A/B first divergence vs {compare_name}"));
                    ui.label(&divergence.explanation);
                    if !divergence.changed_provenance.is_empty() {
                        ui.label(format!("{} changed run knobs", divergence.changed_provenance.len()));
                    }
                } else {
                    ui.strong("Coverage / instrument warnings");
                    if self.reduced.warnings.is_empty() {
                        ui.colored_label(
                            Color32::from_rgb(137, 220, 165),
                            "No coverage warnings for this structured event stream.",
                        );
                    } else {
                        for warning in &self.reduced.warnings {
                            ui.label(RichText::new(warning).color(Color32::from_rgb(240, 192, 110)));
                        }
                    }
                }
            });

        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.heading("Semantic timeline");
            ui.horizontal(|ui| {
                ui.strong("idx");
                ui.add_space(28.0);
                ui.strong("source:seq");
                ui.add_space(85.0);
                ui.strong("dir");
                ui.add_space(20.0);
                ui.strong("message");
                ui.add_space(105.0);
                ui.strong("stage / status");
            });
            ui.separator();
            let timeline = &self.reduced.timeline;
            let visible = &self.visible;
            let selected = self.selected;
            let mut clicked = None;
            egui::ScrollArea::vertical().show_rows(ui, 22.0, visible.len(), |ui, range| {
                for visible_index in range {
                    let row = &timeline[visible[visible_index]];
                    let color = row_color(row.status.as_str(), row.evidence);
                    let label = format!(
                        "{:<5} {:<20} {:<4} {:<24} {:<31} {}",
                        row.index,
                        format!("{}:{}", row.source, row.source_sequence),
                        row.direction,
                        row.message,
                        row.stage,
                        row.status
                    );
                    let row_button = egui::Button::selectable(
                        row.index == selected,
                        RichText::new(label).font(FontId::monospace(12.5)).color(color),
                    )
                    .truncate();
                    let row_width = ui.available_width();
                    if ui
                        .add_sized([row_width, 22.0], row_button)
                        .on_hover_text(&row.event_id)
                        .clicked()
                    {
                        clicked = Some(row.index);
                    }
                }
            });
            if let Some(index) = clicked {
                self.select(index);
            }
        });

        // Static replay intentionally goes idle here. Input events request their
        // own repaint; there is no polling loop to peg a core while reading evidence.
    }
}

fn field(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.label(RichText::new(key).weak());
    ui.label(value);
    ui.end_row();
}

fn row_color(status: &str, evidence: EvidenceStatus) -> Color32 {
    if status == "failed" {
        Color32::from_rgb(245, 145, 135)
    } else {
        match evidence {
            EvidenceStatus::Observed => Color32::from_rgb(214, 222, 235),
            EvidenceStatus::Inferred => Color32::from_rgb(155, 196, 245),
            EvidenceStatus::Missing => Color32::from_rgb(245, 145, 135),
            EvidenceStatus::NotObserved => Color32::from_rgb(145, 155, 170),
            EvidenceStatus::Confounded => Color32::from_rgb(240, 192, 110),
        }
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.visuals.window_shadow = egui::Shadow::NONE;
    style.visuals.selection.bg_fill = Color32::from_rgb(52, 93, 145);
    style.visuals.panel_fill = Color32::from_rgb(23, 27, 34);
    style.visuals.window_fill = Color32::from_rgb(28, 33, 41);
    style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
    ctx.set_global_style(style);

    install_system_fonts(ctx);
}

fn install_system_fonts(ctx: &egui::Context) {
    let proportional = [
        "/usr/share/fonts/google-noto-sans-fonts/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    ]
    .iter()
    .find_map(|path| std::fs::read(path).ok());
    let monospace = [
        "/usr/share/fonts/jetbrains-mono-fonts/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/google-noto-sans-mono-fonts/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
    ]
    .iter()
    .find_map(|path| std::fs::read(path).ok());

    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = proportional {
        fonts.font_data.insert(
            "d1-proportional".to_string(),
            egui::FontData::from_owned(bytes).into(),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "d1-proportional".to_string());
    }
    if let Some(bytes) = monospace {
        fonts.font_data.insert(
            "d1-monospace".to_string(),
            egui::FontData::from_owned(bytes).into(),
        );
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "d1-monospace".to_string());
    }
    ctx.set_fonts(fonts);
}
