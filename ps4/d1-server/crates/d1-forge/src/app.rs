// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The editor.
//!
//! ## Discipline this file is holding to
//!
//! * **All state lives in [`Forge`]; the UI is a pure function of it.** `ctx.memory()` is used for
//!   nothing (`historicaldocs/design/egui-practices.md`). Immediate mode makes smuggling domain state into memory
//!   trivially easy and it is unrecoverable once it starts.
//! * **The item table is virtualized** - `TableBuilder … body.rows(h, n, …)`, never iterating the
//!   data. 5,293 rows are then free. This is not an optimisation, it is egui's performance model:
//!   layout and tessellation are all-CPU and there is no retained scene.
//! * **The filtered result is cached, not recomputed per frame.** Same reason the practices doc
//!   bans rebuilding `PlotPoints` inside the closure. `dirty` is set by the widgets that change the
//!   query, and nothing else.
//! * **No `request_repaint()` per frame.** The only wake comes from the HTTP thread, producer-side.
//!   Idle CPU is zero.
//! * **`on_hover_text` on every control.** No dead knobs.
//!
//! ## What is deliberately absent
//!
//! **Item icons.** `CatalogueEntry::icon` is a path on `bungie.net`, and this box must never fetch
//! from it - containment is the project's first non-negotiable. The path is shown as text so it can
//! be looked up by hand; nothing renders it.

use std::path::PathBuf;

use d1_loadout::{
    catalogue::Query, era, ApplyRequest, ApplyTarget, Catalogue, Class, Damage,
    Gender, Loadout, Race, Severity, Slot, Tier,
};

use crate::control::{self, InFlight, Outcome};
use crate::theme::Palette;

/// Everything the app knows.
pub struct Forge {
    // --- loaded data ---------------------------------------------------------------------------
    pub catalogue: Catalogue,
    /// Why the catalogue is empty, when it is. Rendered instead of a blank table.
    pub catalogue_error: Option<String>,
    pub fonts_installed: Vec<String>,

    // --- the document --------------------------------------------------------------------------
    pub loadout: Loadout,
    pub loadout_path: Option<PathBuf>,
    pub loadouts_dir: PathBuf,
    /// Files found in `loadouts_dir`, refreshed on demand rather than every frame.
    pub loadout_files: Vec<PathBuf>,

    // --- browser state -------------------------------------------------------------------------
    pub query: Query,
    /// Cached result of `query`. See the module docs - never recomputed in the draw closure.
    results: Vec<usize>,
    dirty: bool,
    pub selected: Option<u32>,
    /// Which slot the next "equip" lands in. `None` = the item's own slot, which is what you want
    /// almost always; the override exists because the *validator* is what should catch a mismatch,
    /// not the UI silently preventing you from expressing one.
    pub target_slot: Option<Slot>,

    // --- control plane -------------------------------------------------------------------------
    pub control_url: String,
    pub apply_target: ApplyTarget,
    pub dry_run: bool,
    in_flight: Option<InFlight>,
    pub last_outcome: Option<Outcome>,
    pub status: String,
}

impl Forge {
    pub fn new(
        catalogue: Result<Catalogue, String>,
        loadouts_dir: PathBuf,
        control_url: String,
        fonts_installed: Vec<String>,
    ) -> Self {
        let (catalogue, catalogue_error) = match catalogue {
            Ok(c) => (c, None),
            // An empty catalogue plus a rendered reason, rather than a panic or a silently blank
            // list. "The table is empty" and "the catalogue never loaded" must not look alike.
            Err(e) => (Catalogue::default(), Some(e)),
        };
        let mut f = Forge {
            catalogue,
            catalogue_error,
            fonts_installed,
            loadout: Loadout::default(),
            loadout_path: None,
            loadouts_dir,
            loadout_files: Vec::new(),
            query: Query { hide_post_era: true, ..Default::default() },
            results: Vec::new(),
            dirty: true,
            selected: None,
            target_slot: None,
            control_url,
            apply_target: ApplyTarget::default(),
            dry_run: true,
            in_flight: None,
            last_outcome: None,
            status: String::new(),
        };
        f.rescan_loadouts();
        f
    }

    pub fn rescan_loadouts(&mut self) {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&self.loadouts_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        self.loadout_files = v;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn recompute(&mut self) {
        if !self.dirty {
            return;
        }
        // Indices, not clones: the table redraws from this every frame and cloning ~5k structs on
        // every keystroke is exactly the per-frame allocation the practices doc warns about.
        self.results = self.catalogue.search_indices(&self.query);
        self.dirty = false;
    }

    pub fn results(&self) -> &[usize] {
        &self.results
    }

    pub fn equip(&mut self, idx: usize) {
        let e = &self.catalogue.entries[idx];
        let slot = self.target_slot.unwrap_or(e.slot);
        let item = e.to_ref();
        self.loadout.set(slot, item);
        self.status = format!("equipped {} into {}", e.name, slot);
    }

    pub fn load_file(&mut self, p: &std::path::Path) {
        match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| {
            Loadout::from_json(&s).map_err(|e| e.to_string())
        }) {
            Ok(l) => {
                self.loadout = l;
                self.loadout_path = Some(p.to_path_buf());
                self.status = format!("loaded {}", p.display());
            }
            Err(e) => self.status = format!("could not load {}: {e}", p.display()),
        }
    }

    pub fn save(&mut self, p: PathBuf) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&p, self.loadout.to_json()) {
            Ok(()) => {
                self.status = format!("saved {}", p.display());
                self.loadout_path = Some(p);
                self.rescan_loadouts();
            }
            Err(e) => self.status = format!("could not save: {e}"),
        }
    }

    /// A filename derived from the loadout's own name, so "Save as new" needs no dialog.
    pub fn suggested_filename(&self) -> PathBuf {
        let slug: String = self
            .loadout
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect();
        let slug = slug.trim_matches('-').replace("--", "-");
        self.loadouts_dir.join(format!(
            "{}.json",
            if slug.is_empty() { "untitled" } else { &slug }
        ))
    }

    pub fn apply(&mut self, ctx: &egui::Context) {
        if self.in_flight.is_some() {
            self.status = "an apply is already in flight".into();
            return;
        }
        let req = ApplyRequest::new(
            self.loadout.clone(),
            self.apply_target.clone(),
            self.dry_run,
            control::request_id(),
        );
        self.status = format!(
            "POST {}{} …",
            self.control_url.trim_end_matches('/'),
            d1_loadout::APPLY_PATH
        );
        self.last_outcome = None;
        self.in_flight = Some(control::spawn_apply(self.control_url.clone(), req, ctx.clone()));
    }

    /// Drain the HTTP channel. Per-frame non-UI work belongs in `App::logic`, which is where this is
    /// called from - not in a widget closure.
    fn drain(&mut self) {
        let Some(f) = &self.in_flight else { return };
        match f.rx.try_recv() {
            Ok(o) => {
                self.status = o.headline();
                self.last_outcome = Some(o);
                self.in_flight = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            // The worker thread ended without sending. It always sends before it exits, so this is
            // a panic in the worker - a domain signal, not noise. Standing rule from
            // interface-architecture.md §9.7: every channel source gets an explicit terminal
            // condition, and closure is never silently ignored.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.status = "the apply worker exited without answering".into();
                self.last_outcome = Some(Outcome::Unreachable {
                    error: "worker thread ended without sending a result".into(),
                });
                self.in_flight = None;
            }
        }
    }

    pub fn is_applying(&self) -> bool {
        self.in_flight.is_some()
    }
}

// The `eframe::App` impl is a two-line shim over [`Forge::tick`] and [`Forge::draw`]. Keeping the
// real bodies off the trait is what lets `egui_kittest` drive the whole app headlessly: constructing
// an `eframe::Frame` in a test is not practical, and a GUI that can only be exercised by opening a
// window is a GUI that never gets exercised.
impl eframe::App for Forge {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl Forge {
    /// Per-frame non-UI work. `App::logic`'s body - channel drains and the filter recompute, never
    /// widgets.
    pub fn tick(&mut self) {
        self.drain();
        self.recompute();
    }

    /// The whole UI. `App::ui`'s body.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        // Panels must be added before the CentralPanel, and the `Ui` handed to `App::ui` has no
        // margin or background of its own - both are eframe facts, not preferences.
        //
        // ⚠ API drift, found by the compiler on 0.36.1: `SidePanel` and `TopBottomPanel` are gone,
        // unified into `egui::Panel::{left,right,top,bottom}` with `.default_size()` /
        // `.size_range()` instead of the per-axis `default_width`/`width_range`, and `show_inside`
        // is deprecated in favour of `show`. This is precisely the breakage
        // `historicaldocs/design/egui-practices.md` says to expect between minors - noted here so the next reader
        // does not "fix" it back to the old names from memory.
        egui::Panel::top("provenance").show(ui, |ui| self.provenance_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("loadout")
            .default_size(430.0)
            .size_range(360.0..=640.0)
            .show(ui, |ui| self.loadout_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.browser(ui));
    }
}

// -------------------------------------------------------------------------------------------------
// Views
// -------------------------------------------------------------------------------------------------

impl Forge {
    /// The provenance banner.
    ///
    /// The brief for this work said to make era visible rather than silently implied, and this is
    /// the load-bearing part of that: the catalogue is a **2020** manifest and the target is a
    /// **2016** build. Put somewhere it can be scrolled away from, that mismatch is forgotten within
    /// a session; pinned to the top of the window it is a standing condition.
    fn provenance_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.heading("d1-forge");
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("target {}", era::TARGET_BUILD))
                    .color(Palette::OK)
                    .strong(),
            )
            .on_hover_text("The build this project runs. ERA-TARGETING.md pins it from the PKG c_date.");
            ui.label(egui::RichText::new("·").color(Palette::MUTED));
            ui.label(
                egui::RichText::new(format!(
                    "catalogue from manifest {}",
                    if self.catalogue.header.manifest_version.is_empty() {
                        "(none loaded)"
                    } else {
                        &self.catalogue.header.manifest_version
                    }
                ))
                .color(Palette::WARN)
                .strong(),
            )
            .on_hover_text("The preserved Bungie world manifest.");
            ui.label(
                egui::RichText::new(format!(
                    "{} equippable of {} scanned",
                    self.catalogue.header.items_kept, self.catalogue.header.items_scanned
                ))
                .color(Palette::MUTED),
            )
            .on_hover_text(format!(
                "catalogue file: {}\nfonts: {}",
                if self.catalogue.header.manifest_file.is_empty() {
                    "(none)"
                } else {
                    &self.catalogue.header.manifest_file
                },
                if self.fonts_installed.is_empty() {
                    // Worth surfacing rather than shrugging at: the whole point of loading host
                    // fonts is that egui's defaults are the giveaway, so silently falling back
                    // should be visible somewhere.
                    "egui defaults (no host font found)".to_string()
                } else {
                    self.fonts_installed.join(", ")
                }
            ));
        });
        if !self.catalogue.header.provenance_warning.is_empty() {
            ui.label(
                egui::RichText::new(&self.catalogue.header.provenance_warning)
                    .small()
                    .color(Palette::MUTED),
            );
        }
        ui.add_space(4.0);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            let issues = self.loadout.validate();
            let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
            let warns = issues.len() - errors;
            let (text, colour) = if errors > 0 {
                (format!("{errors} error(s), {warns} warning(s)"), Palette::ERROR)
            } else if warns > 0 {
                (format!("valid · {warns} warning(s)"), Palette::WARN)
            } else {
                ("valid".to_string(), Palette::OK)
            };
            ui.label(egui::RichText::new(text).color(colour).strong())
                .on_hover_text("Manifest-consistency checks only. Nothing here has been confirmed against the 01.29 client.");
            ui.separator();
            ui.label(egui::RichText::new(&self.status).color(Palette::MUTED));
        });
        ui.add_space(3.0);
    }

    fn loadout_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("loadout-scroll")
            .show(ui, |ui| {
                self.character_section(ui);
                ui.separator();
                self.slots_section(ui);
                ui.separator();
                self.issues_section(ui);
                ui.separator();
                self.files_section(ui);
                ui.separator();
                self.apply_section(ui);
            });
    }

    fn character_section(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Character").heading());
        egui::Grid::new("character")
            .num_columns(2)
            .striped(true)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut self.loadout.name).desired_width(f32::INFINITY))
                    .on_hover_text("Free text. Also the suggested filename when saving.");
                ui.end_row();

                ui.label("Class");
                ui.horizontal(|ui| {
                    for c in Class::ALL {
                        if ui
                            .selectable_label(self.loadout.class == c, c.name())
                            .on_hover_text(format!("classHash {}", c.hash()))
                            .clicked()
                        {
                            self.loadout.class = c;
                        }
                    }
                });
                ui.end_row();

                ui.label("Race");
                ui.horizontal(|ui| {
                    for r in Race::ALL {
                        if ui
                            .selectable_label(self.loadout.race == r, r.name())
                            .on_hover_text(format!("raceHash {}", r.hash()))
                            .clicked()
                        {
                            self.loadout.race = r;
                        }
                    }
                });
                ui.end_row();

                ui.label("Gender");
                ui.horizontal(|ui| {
                    for g in Gender::ALL {
                        if ui
                            .selectable_label(self.loadout.gender == g, g.name())
                            .on_hover_text(format!("genderHash {}", g.hash()))
                            .clicked()
                        {
                            self.loadout.gender = g;
                        }
                    }
                });
                ui.end_row();

                ui.label("Level");
                ui.add(egui::DragValue::new(&mut self.loadout.level).range(1..=40))
                    .on_hover_text("01.29 is a Rise of Iron build; the cap was 40. [I] - from public patch history, not measured against our client.");
                ui.end_row();

                ui.label("Light");
                ui.add(egui::DragValue::new(&mut self.loadout.light).range(0..=400))
                    .on_hover_text("Rise of Iron maximum was 400. [I], same caveat. Year-1 loadouts use the Year-1 numbers instead.");
                ui.end_row();
            });

        ui.collapsing("Notes", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.loadout.notes)
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            )
            .on_hover_text("Travels with the file. Say what is verified and what is not.");
        });
    }

    fn slots_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Equipment").heading());
            ui.label(egui::RichText::new("(click a row to target it)").small().color(Palette::MUTED));
        });

        let mut clear: Option<Slot> = None;
        egui::Grid::new("slots")
            .num_columns(3)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                for slot in Slot::ALL {
                    let targeted = self.target_slot == Some(slot);
                    let combat = Slot::COMBAT.contains(&slot);
                    let label = egui::RichText::new(slot.display_name())
                        .color(if combat { ui.visuals().text_color() } else { Palette::MUTED });
                    if ui
                        .selectable_label(targeted, label)
                        .on_hover_text(format!(
                            "bucketHash {}{}",
                            slot.bucket_hash(),
                            if combat { "" } else { " · cosmetic" }
                        ))
                        .clicked()
                    {
                        self.target_slot = if targeted { None } else { Some(slot) };
                    }

                    match self.loadout.get(slot) {
                        Some(item) => {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&item.name).color(tier_colour(item.tier)),
                                )
                                .on_hover_text(format!(
                                    "hash {}\n{}\nicon {}\n{}: {}",
                                    item.hash,
                                    item.item_type_name,
                                    if item.icon.is_empty() { "(none)" } else { &item.icon },
                                    item.era.label(),
                                    item.era_note
                                ));
                                if item.era.is_suspect() {
                                    ui.label(
                                        egui::RichText::new("⚠")
                                            .color(Palette::ERROR)
                                            .small(),
                                    )
                                    .on_hover_text(item.era_note.clone());
                                }
                            });
                            if ui.small_button("clear").on_hover_text("Unequip this slot").clicked() {
                                clear = Some(slot);
                            }
                        }
                        None => {
                            ui.label(egui::RichText::new("-").color(Palette::MUTED));
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
        if let Some(s) = clear {
            self.loadout.clear(s);
            self.status = format!("cleared {s}");
        }
    }

    fn issues_section(&mut self, ui: &mut egui::Ui) {
        let issues = self.loadout.validate();
        ui.label(egui::RichText::new("Checks").heading());
        if issues.is_empty() {
            ui.label(egui::RichText::new("nothing to report").color(Palette::MUTED));
            return;
        }
        for i in &issues {
            let colour = match i.severity {
                Severity::Error => Palette::ERROR,
                Severity::Warning => Palette::WARN,
            };
            ui.label(egui::RichText::new(format!("{i}")).color(colour))
                .on_hover_text(
                    "Manifest-consistency and known-game-rule checks. [I] - the client has never \
                     been observed accepting or rejecting any of these.",
                );
        }
    }

    fn files_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Files").heading());
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("New")
                .on_hover_text("Discard the current loadout and start empty")
                .clicked()
            {
                self.loadout = Loadout::default();
                self.loadout_path = None;
                self.status = "new loadout".into();
            }
            let save_target = self.loadout_path.clone().unwrap_or_else(|| self.suggested_filename());
            if ui
                .button("Save")
                .on_hover_text(format!("Write {}", save_target.display()))
                .clicked()
            {
                self.save(save_target);
            }
            let as_new = self.suggested_filename();
            if ui
                .button("Save as new")
                .on_hover_text(format!("Write {}", as_new.display()))
                .clicked()
            {
                self.save(as_new);
            }
            if ui.button("Rescan").on_hover_text("Re-read the loadouts directory").clicked() {
                self.rescan_loadouts();
            }
        });

        let files = self.loadout_files.clone();
        if files.is_empty() {
            ui.label(
                egui::RichText::new(format!("no .json files in {}", self.loadouts_dir.display()))
                    .color(Palette::MUTED),
            );
        }
        for p in files {
            let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let current = self.loadout_path.as_deref() == Some(p.as_path());
            if ui
                .selectable_label(current, name)
                .on_hover_text(p.display().to_string())
                .clicked()
            {
                self.load_file(&p);
            }
        }
    }

    fn apply_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Apply").heading());
        ui.label(
            egui::RichText::new(format!(
                "POST {}  ·  contract v{}",
                d1_loadout::APPLY_PATH,
                d1_loadout::APPLY_CONTRACT_VERSION
            ))
            .small()
            .color(Palette::MUTED),
        );

        egui::Grid::new("apply")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Control plane");
                ui.add(egui::TextEdit::singleline(&mut self.control_url).desired_width(f32::INFINITY))
                    .on_hover_text("Loopback only. Use `ssh -L` for remote - there is no auth code anywhere in this project.");
                ui.end_row();

                ui.label("Character index");
                ui.add(egui::DragValue::new(&mut self.apply_target.index).range(0..=2))
                    .on_hover_text("[U] the character identity model is unknown; an index is the least we can be wrong about.");
                ui.end_row();

                ui.label("Character id");
                ui.add(
                    egui::TextEdit::singleline(&mut self.apply_target.character_id)
                        .hint_text("(optional, opaque)")
                        .desired_width(f32::INFINITY),
                )
                .on_hover_text("[U] type unknown, so it travels as a string. Empty means 'use the index'.");
                ui.end_row();
            });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.dry_run, "Dry run")
                .on_hover_text("Ask the server to validate and report without changing state.");
            let busy = self.is_applying();
            let btn = ui.add_enabled(!busy, egui::Button::new(if busy { "applying…" } else { "Apply" }));
            if btn
                .on_hover_text(
                    "Sends the whole loadout document as JSON over HTTP. This is a control-plane \
                     call between our own tools - it is NOT a game protocol message and invents no \
                     wire format.",
                )
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.apply(&ctx);
            }
        });

        if let Some(o) = &self.last_outcome {
            let colour = if o.is_success() {
                Palette::OK
            } else if o.is_expected_gap() {
                // Not an error. The receiving end genuinely does not exist yet, and colouring that
                // red trains the operator to ignore the one indicator that matters.
                Palette::MUTED
            } else {
                Palette::ERROR
            };
            ui.label(egui::RichText::new(o.headline()).color(colour));
            if o.is_expected_gap() {
                ui.label(
                    egui::RichText::new(
                        "Expected: no control plane implements this route yet. See \
                         historicaldocs/design/loadout-framework.md for the contract the server side must satisfy.",
                    )
                    .small()
                    .color(Palette::MUTED),
                );
            }
            let detail = o.detail();
            if !detail.trim().is_empty() {
                ui.collapsing("Response detail", |ui| {
                    ui.label(egui::RichText::new(detail).monospace().small());
                });
            }
        }
    }

    // --- the browser ---------------------------------------------------------------------------

    fn browser(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = self.catalogue_error.clone() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No catalogue loaded").heading().color(Palette::ERROR));
            ui.label(egui::RichText::new(err).color(Palette::MUTED));
            ui.add_space(6.0);
            ui.label("Build it with:");
            ui.label(
                egui::RichText::new("  cargo run -p d1-loadout --bin d1-loadout -- build")
                    .monospace(),
            );
            return;
        }

        self.filter_bar(ui);
        ui.separator();
        self.item_table(ui);
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            let r = ui
                .add(
                    egui::TextEdit::singleline(&mut self.query.text)
                        .hint_text("name, type, or a hash (1274330687 / 0x4bf4be3f)")
                        .desired_width(280.0),
                )
                .on_hover_text("Substring of the name or the item type. A bare number is matched as an exact hash - the question that actually gets asked.");
            changed |= r.changed();

            changed |= combo(ui, "slot", &mut self.query.slot, Slot::ALL.to_vec(), |s| {
                s.display_name().to_string()
            });
            changed |= combo(ui, "tier", &mut self.query.tier, Tier::ALL.to_vec(), |t| {
                t.name().to_string()
            });
            changed |= combo(ui, "class", &mut self.query.class, Class::ALL.to_vec(), |c| {
                c.name().to_string()
            });
            changed |= combo(
                ui,
                "damage",
                &mut self.query.damage,
                vec![Damage::Kinetic, Damage::Arc, Damage::Solar, Damage::Void],
                |d| d.name().to_string(),
            );
        });
        ui.horizontal_wrapped(|ui| {
            changed |= ui
                .checkbox(&mut self.query.year_one_only, "Year 1 only")
                .on_hover_text(
                    "qualityLevel > 0. Measured: the Year-1 copy of an item has a non-zero quality \
                     level and its later re-issue has 0. Controlled in both directions - see \
                     d1-loadout's era module.",
                )
                .changed();
            changed |= ui
                .checkbox(&mut self.query.hide_post_era, "Hide post-01.29")
                .on_hover_text(
                    "Drops items whose reward sources are all from The Dawning or Age of Triumph. \
                     This can only ever prove an item is OUT of era - it cannot prove one is in it.",
                )
                .changed();
            if ui.button("Reset filters").clicked() {
                self.query = Query { hide_post_era: true, ..Default::default() };
                changed = true;
            }
            ui.label(
                egui::RichText::new(format!("{} match(es)", self.results().len()))
                    .color(Palette::MUTED),
            );
            if let Some(s) = self.target_slot {
                ui.label(
                    egui::RichText::new(format!("→ equipping into {s}"))
                        .color(Palette::ACCENT)
                        .strong(),
                )
                .on_hover_text("Click the slot again in the left panel to clear the override.");
            }
        });
        if changed {
            self.mark_dirty();
        }
    }

    fn item_table(&mut self, ui: &mut egui::Ui) {
        use egui_extras::{Column, TableBuilder};

        let mut equip: Option<usize> = None;
        let row_h = 20.0;
        let n = self.results().len();

        TableBuilder::new(ui)
            .striped(true)
            .sense(egui::Sense::click())
            .column(Column::auto().at_least(78.0)) // hash, right-aligned monospace
            .column(Column::auto().at_least(56.0)) // tier
            .column(Column::auto().at_least(96.0)) // slot
            .column(Column::auto().at_least(118.0)) // type
            .column(Column::auto().at_least(52.0)) // damage
            .column(Column::auto().at_least(44.0)) // Y1
            .column(Column::remainder().at_least(180.0)) // name
            .min_scrolled_height(0.0)
            .header(22.0, |mut h| {
                for title in ["hash", "tier", "slot", "type", "dmg", "yr", "name"] {
                    h.col(|ui| {
                        ui.label(egui::RichText::new(title).small().strong());
                    });
                }
            })
            // Virtualized: only the visible rows are laid out. Iterating `self.results` here
            // instead would lay out all 5,293 every frame.
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let idx = self.results()[row.index()];
                    // Borrow the FIELD, not `self`: an accessor method would borrow the whole
                    // struct and the click handler below could not then assign `self.selected`.
                    let e = &self.catalogue.entries[idx];
                    let hash = e.hash;
                    row.set_selected(self.selected == Some(hash));

                    row.col(|ui| {
                        // Right-aligned monospace so magnitudes line up - the numbers are compared
                        // by eye far more often than they are read.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(e.hash.to_string()).monospace().small());
                        });
                    });
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(e.tier.name()).small().color(tier_colour(e.tier)),
                        );
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(e.slot.display_name()).small());
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&e.item_type_name).small());
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(e.damage.name()).small());
                    });
                    row.col(|ui| {
                        if e.year_one {
                            ui.label(egui::RichText::new("Y1").small().color(Palette::YEAR_ONE));
                        }
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(&e.name);
                            if e.era.is_suspect() {
                                ui.label(egui::RichText::new("⚠").small().color(Palette::ERROR));
                            }
                            if e.class_type != d1_loadout::CLASS_TYPE_ANY {
                                if let Some(c) = Class::from_class_type(e.class_type) {
                                    ui.label(
                                        egui::RichText::new(c.name()).small().color(Palette::MUTED),
                                    );
                                }
                            }
                        });
                    });

                    let r = row.response();
                    if r.clicked() {
                        self.selected = Some(hash);
                    }
                    if r.double_clicked() {
                        equip = Some(idx);
                    }
                    r.on_hover_text(format!(
                        "{}\n\nhash {}  ·  bucket {}  ·  qualityLevel {}\nicon {}\n\nera: {} - {}\n\n\
                         double-click to equip",
                        if e.description.is_empty() { "(no description)" } else { &e.description },
                        e.hash,
                        e.bucket_hash,
                        e.quality_level,
                        if e.icon.is_empty() { "(none)" } else { &e.icon },
                        e.era.label(),
                        e.era_note
                    ));
                });
            });

        if let Some(idx) = equip {
            self.equip(idx);
        }
    }
}

/// An optional-valued combo box. Returns whether the selection changed.
fn combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<T>,
    options: Vec<T>,
    name: impl Fn(T) -> String,
) -> bool {
    let before = *value;
    let text = value.map(&name).unwrap_or_else(|| format!("any {label}"));
    egui::ComboBox::from_id_salt(label)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, format!("any {label}"));
            for o in options {
                ui.selectable_value(value, Some(o), name(o));
            }
        });
    before != *value
}

fn tier_colour(t: Tier) -> egui::Color32 {
    // The game's own rarity colours, near enough to be recognisable without being a reproduction.
    match t {
        Tier::Exotic => egui::Color32::from_rgb(0xCE, 0xAE, 0x33),
        Tier::Legendary => egui::Color32::from_rgb(0x91, 0x6F, 0xB8),
        Tier::Rare => egui::Color32::from_rgb(0x5B, 0x92, 0xC9),
        Tier::Uncommon => egui::Color32::from_rgb(0x62, 0x9C, 0x62),
        _ => Palette::MUTED,
    }
}

// -------------------------------------------------------------------------------------------------
// Tests
//
// `egui_kittest` drives the real app headlessly over the AccessKit tree - no window, no GPU, no
// pixel diffing (`historicaldocs/design/interface-architecture.md` §7.4). Nothing here launches anything.
//
// The assertions are deliberately about the things that would otherwise fail *silently*: that the
// provenance mismatch is on screen, that an unloadable catalogue says so instead of rendering an
// empty table, and that the filters actually filter.
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable as _;
    use egui_kittest::Harness;

    fn repo_root() -> PathBuf {
        // crates/d1-forge -> crates -> d1-server -> repo root
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repo root")
            .to_path_buf()
    }

    fn load_catalogue() -> Result<Catalogue, String> {
        let p = repo_root().join("catalogue/d1-catalogue.json");
        std::fs::read_to_string(&p)
            .map_err(|e| format!("{}: {e}", p.display()))
            .and_then(|s| Catalogue::from_json(&s).map_err(|e| e.to_string()))
    }

    fn harness(cat: Result<Catalogue, String>) -> Harness<'static, Forge> {
        let forge = Forge::new(
            cat,
            repo_root().join("loadouts"),
            "http://127.0.0.1:1".to_string(),
            vec![],
        );
        Harness::builder()
            .with_size(egui::vec2(1400.0, 900.0))
            .build_ui_state(
                |ui, state: &mut Forge| {
                    state.tick();
                    state.draw(ui);
                },
                forge,
            )
    }

    #[test]
    fn renders_and_shows_the_era_mismatch() {
        let Ok(cat) = load_catalogue() else {
            println!("SKIP: catalogue/d1-catalogue.json absent - run `d1-loadout build`");
            return;
        };
        let items = cat.entries.len();
        let mut h = harness(Ok(cat));
        h.run();

        // The brief's requirement, asserted rather than assumed: both the 2016 target and the 2020
        // manifest are on screen at once, so the mismatch cannot be silently implied.
        h.get_by_label_contains("target 01.29");
        h.get_by_label_contains("88857.20.11.05.1145-2");

        // And the browser actually populated. A control for the two assertions above: if the app
        // rendered a blank shell they would still have passed.
        let shown = h.state().results().len();
        assert!(shown > 4000, "{shown}");
        // `hide_post_era` is on by default, so the default view is a strict subset - the safe
        // direction, and asserted so a later change to the default is a test failure rather than a
        // silent change of what the operator sees.
        assert!(shown < items, "{shown} of {items}: nothing was hidden by the default filter");
    }

    #[test]
    fn a_missing_catalogue_says_so_instead_of_rendering_an_empty_table() {
        let mut h = harness(Err("catalogue/d1-catalogue.json: No such file".into()));
        h.run();
        h.get_by_label_contains("No catalogue loaded");
        h.get_by_label_contains("No such file");
        assert!(h.state().results().is_empty());
    }

    #[test]
    fn filters_narrow_the_result_set() {
        let Ok(cat) = load_catalogue() else {
            println!("SKIP: catalogue absent");
            return;
        };
        let mut h = harness(Ok(cat));
        h.run();
        let all = h.state().results().len();

        // Drive the state the way the widgets do, then let the app recompute in `tick`.
        h.state_mut().query.slot = Some(Slot::Heavy);
        h.state_mut().mark_dirty();
        h.run();
        let heavy = h.state().results().len();
        assert!(heavy > 0 && heavy < all, "{heavy} of {all}");
        assert!(h
            .state()
            .results()
            .iter()
            .all(|i| h.state().catalogue.entries[*i].slot == Slot::Heavy));

        // Hiding post-era items must remove some, and the default query already has it on - so
        // turning it OFF must ADD rows. Checking it in that direction proves the flag is wired,
        // where "turning it on removed nothing" could just mean nothing was flagged.
        h.state_mut().query.slot = None;
        h.state_mut().query.hide_post_era = false;
        h.state_mut().mark_dirty();
        h.run();
        assert!(h.state().results().len() > all, "hide_post_era hid nothing");
    }

    #[test]
    fn equipping_from_the_catalogue_produces_a_valid_loadout() {
        let Ok(cat) = load_catalogue() else {
            println!("SKIP: catalogue absent");
            return;
        };
        let mut h = harness(Ok(cat));
        h.run();

        // Year-1 Gjallarhorn, by hash - the same positive control the data crate uses.
        let idx = h
            .state()
            .catalogue
            .entries
            .iter()
            .position(|e| e.hash == 1_274_330_687)
            .expect("Gjallarhorn 1274330687");
        h.state_mut().equip(idx);
        h.run();

        let l = &h.state().loadout;
        assert_eq!(l.get(Slot::Heavy).map(|i| i.hash), Some(1_274_330_687));
        assert!(l.is_valid(), "{:?}", l.validate());

        // The status line is how the operator knows it landed.
        h.get_by_label_contains("equipped Gjallarhorn");
    }

    #[test]
    fn the_shipped_loadouts_load_and_validate() {
        let Ok(cat) = load_catalogue() else {
            println!("SKIP: catalogue absent");
            return;
        };
        let mut h = harness(Ok(cat));
        h.run();
        let files = h.state().loadout_files.clone();
        assert!(!files.is_empty(), "no loadouts found in {}", h.state().loadouts_dir.display());
        for f in files {
            h.state_mut().load_file(&f);
            h.run();
            let l = &h.state().loadout;
            assert!(l.is_valid(), "{}: {:?}", f.display(), l.validate());
            // Every stored item must still resolve against the catalogue, or the file names
            // something this manifest does not have.
            for e in &l.equipped {
                assert!(
                    h.state().catalogue.get(e.item.hash).is_some(),
                    "{}: {} ({}) is not in the catalogue",
                    f.display(),
                    e.item.name,
                    e.item.hash
                );
            }
        }
    }
}
