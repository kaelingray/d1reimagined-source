// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-ghost` - the run companion's window (`design/d1-ghost.md` §1).
//!
//! One window: the whole `d1-dash` read side (embedded by composition - `GhostApp` owns a
//! `DashApp` and calls its panels) plus a **control console** on the right whose every write goes
//! through the gated, ledgered [`crate::ghost::exec`] path. The read side stays read-only; the
//! console is the project's one write door.
//!
//! ⚠ **Runtime-unverified in CI.** This is a glow/eframe app; its LOGIC (registry gates, the
//! ledger, the executor, the destination parse) is pinned by `C-GHOST-*` and unit tests, but the
//! rendering itself is exercised only when a human runs it against a live client. The egui 0.36.1
//! API rules from `dash/gui.rs` (Panel::{top,left,right,bottom} before CentralPanel;
//! `eframe::App::{logic, ui}`) are followed here.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use eframe::egui::{self, RichText, ScrollArea};

use super::dests;
use super::dests::Destinations;
use super::exec::{self, SubprocessBackend};
use super::ledger::{self, Ledger, Line};
use super::registry::{self, Class, Poke};
use super::Observed;
use crate::dash::gui::{DashApp, Palette};
use crate::dash::Session;

/// Open a run directory in the ghost window.
pub fn run(run_dir: PathBuf, repo_root: PathBuf, emu_pid: Option<u32>) -> Result<()> {
    let session = Session::open(&run_dir);
    let title = format!("d1-ghost - {}", session.run_dir.display());
    let ghost = GhostApp::new(run_dir, repo_root, emu_pid);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1040.0])
            .with_min_inner_size([1320.0, 760.0])
            .with_title(title.clone()),
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| Ok(Box::new(ghost.into_running(cc, session)))),
    )
    .map_err(|e| anyhow!(e.to_string()))
}

/// The pre-creation-context half - what `run` builds before eframe hands us a `CreationContext`.
struct GhostApp {
    run_dir: PathBuf,
    repo_root: PathBuf,
    emu_pid: Option<u32>,
}

impl GhostApp {
    fn new(run_dir: PathBuf, repo_root: PathBuf, emu_pid: Option<u32>) -> GhostApp {
        GhostApp { run_dir, repo_root, emu_pid }
    }

    fn into_running(self, cc: &eframe::CreationContext<'_>, session: Session) -> RunningGhost {
        let dests = Destinations::load(&self.repo_root);
        let mut r = RunningGhost {
            dash: DashApp::new(cc, session),
            run_dir: self.run_dir,
            repo_root: self.repo_root,
            emu_pid: self.emu_pid,
            dests,
            ledger_lines: Vec::new(),
            last_ledger_read: Instant::now() - Duration::from_secs(60),
            last_outcome: None,
            operator_note: String::new(),
            show_unlaunchable: false,
            selected_map: super::dests::MAP_OBSERVED_LIVE.to_string(),
        };
        r.reload_ledger();
        r
    }
}

/// The live app: the embedded dash plus the console state.
struct RunningGhost {
    dash: DashApp,
    run_dir: PathBuf,
    repo_root: PathBuf,
    emu_pid: Option<u32>,
    dests: Destinations,
    ledger_lines: Vec<Line>,
    last_ledger_read: Instant,
    /// The last poke's human-readable outcome line, shown under the buttons.
    last_outcome: Option<String>,
    operator_note: String,
    /// Destination selector: show the greyed non-launchable rows (the reachability map) or just the
    /// launchable ones. Off by default - 519 of 540 activities cannot be launched, so the actionable
    /// list is short; the toggle reveals the rest with their refusal reasons.
    show_unlaunchable: bool,
    /// The MAP half of a launch (the MODE is the activity index the buttons fire). The stem is the
    /// world-name string the Activity Host serves at `blob+0x688`. Defaults to the one proven live.
    selected_map: String,
}

impl RunningGhost {
    /// What the console has observed about the client - drives every gate. Rung comes from the
    /// dash board's `current`; liveness is the coarse live-probe proxy (see [`DashApp::live_ok`]).
    ///
    /// ⚠ The liveness proxy is deliberately conservative: `live_ok()` false ⇒ L0, which refuses
    /// every write. A false "the client is live" would be far worse than a false "it is not".
    fn observed(&self) -> Observed {
        let board = self.dash.board();
        let (rung, state) = board
            .current
            .as_ref()
            .map(|c| (c.rung, c.state.clone()))
            .unwrap_or((None, String::new()));
        Observed {
            rung,
            state,
            liveness: if self.dash.live_ok() { 3 } else { 0 },
        }
    }

    fn reload_ledger(&mut self) {
        let led = Ledger::for_run(&self.run_dir);
        self.ledger_lines = led.read_all().into_iter().flatten().collect();
        self.last_ledger_read = Instant::now();
    }

    /// Run one registered poke through the executor, then reload the ledger so the tape and the
    /// dangling-intent banner reflect it.
    fn fire(&mut self, poke: &Poke) {
        let obs = self.observed();
        let led = Ledger::for_run(&self.run_dir);
        let seq = ledger::next_seq(&self.ledger_lines);
        let ts = rfc3339_now();
        let note = if self.operator_note.trim().is_empty() {
            None
        } else {
            Some(self.operator_note.trim().to_string())
        };
        let mut backend = SubprocessBackend {
            repo_root: self.repo_root.clone(),
            emu_pid: self.emu_pid,
            guest_common: Vec::new(),
        };
        match exec::apply(poke, &obs, &mut backend, &led, seq, &ts, note) {
            Ok(a) => {
                self.last_outcome = Some(match a.refused_reason {
                    Some(r) => format!("{} REFUSED: {r}", poke.name),
                    None => format!("{} -> {} (exit {:?})", poke.name, a.outcome, a.exit),
                });
            }
            Err(e) => {
                self.last_outcome = Some(format!("{} could not be ledgered, NOT executed: {e}", poke.name));
            }
        }
        self.reload_ledger();
    }

    fn console(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ghost console");
        ui.label(
            RichText::new("the one write door - every poke is gated, ledgered, and readback-checked")
                .small()
                .color(Palette::MUTED),
        );
        ui.separator();

        let obs = self.observed();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("observed:").color(Palette::MUTED));
            ui.label(
                RichText::new(match obs.rung {
                    Some(r) => format!("rung {r} ({})", obs.state),
                    None => "rung UNKNOWN".to_string(),
                })
                .color(if obs.rung.is_some() { Palette::ACCENT } else { Palette::WARN }),
            );
            ui.label(
                RichText::new(format!("L{}", obs.liveness))
                    .color(if obs.liveness >= 3 { Palette::OK } else { Palette::ERR }),
            );
        });

        // The taint lamp - always visible, honest about the lower bound.
        let reach_synth = self
            .dash
            .board()
            .reach
            .to_lowercase()
            .contains("synthetic");
        let lamp = super::taint_lamp(!self.ledger_lines.is_empty(), reach_synth);
        let lamp_color = if self.ledger_lines.is_empty() && !reach_synth {
            Palette::MUTED
        } else {
            Palette::WARN
        };
        ui.label(RichText::new(lamp).color(lamp_color));
        ui.separator();

        // Dangling-intent banner: a poke that started and did not visibly finish.
        let dangling = ledger::dangling(&self.ledger_lines);
        if !dangling.is_empty() {
            for d in &dangling {
                ui.label(
                    RichText::new(format!(
                        "⛔ DANGLING WRITE seq {} '{}' ({}) - intent on disk, NO result. The \
                         client may hold a half-applied write.",
                        d.seq, d.name, d.target
                    ))
                    .color(Palette::ERR),
                );
            }
            ui.separator();
        }

        ui.label(RichText::new("operator note (optional, goes in the ledger):").small());
        ui.text_edit_singleline(&mut self.operator_note);
        ui.add_space(4.0);

        // The registry buttons, in RECIPE ORDER (registry() is sorted by step). Each button is a
        // numbered step to reach in-world; the gate greys it outside its window.
        ui.label(RichText::new("registered pokes - recipe order to reach in-world").strong());
        ui.label(
            RichText::new(format!(
                "{} entries still synthetic. Launch AT ORBIT, then force the ladder, then the two \
                 hooks, spawn, and clear the fade. A greyed button is not yet in its window.",
                registry::registry().len()
            ))
            .small()
            .color(Palette::MUTED),
        );
        let mut to_fire: Option<Poke> = None;
        for poke in registry::registry() {
            let unmet = poke.gate.unmet(&obs);
            let enabled = unmet.is_none();
            let color = match poke.class {
                Class::Launch => Palette::ACCENT,
                Class::Satisfy => Palette::WARN,
                Class::Probe => Palette::OK,
            };
            ui.horizontal_wrapped(|ui| {
                let btn = egui::Button::new(
                    RichText::new(format!("{}. {}", poke.step, poke.name)).color(color),
                );
                let resp = ui.add_enabled(enabled, btn);
                let resp = resp.on_hover_text(format!(
                    "{}\nclass {}\n{}\ncite: {}",
                    poke.summary,
                    poke.class.as_str(),
                    poke.class.warning(),
                    poke.citation
                ));
                if resp.clicked() {
                    to_fire = Some(poke.clone());
                }
                if let Some(reason) = &unmet {
                    ui.label(RichText::new(format!("greyed: {reason}")).small().color(Palette::MUTED));
                } else if poke.class == Class::Satisfy {
                    ui.label(RichText::new("SATISFY - deliverable is server code").small().color(Palette::WARN));
                }
            });
            // The one-line function, so the operator does not need the hover to know what it does.
            ui.label(RichText::new(poke.summary).small().color(Palette::MUTED));
        }
        if let Some(p) = to_fire {
            self.fire(&p);
        }

        if let Some(o) = &self.last_outcome {
            ui.add_space(4.0);
            ui.label(RichText::new(o).monospace().color(Palette::ACCENT));
        }

        ui.separator();
        self.destinations(ui);

        ui.separator();
        ui.label(RichText::new("ledger tape (pokes.jsonl)").strong());
        self.ledger_tape(ui);
    }

    fn destinations(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("destinations - MODE x MAP").strong());
        ui.label(
            RichText::new(
                "a launch is a MODE (one of 21 private-match Crucible types) crossed with a MAP. \
                 Pick the map, then click a mode to launch it there.",
            )
            .small()
            .color(Palette::MUTED),
        );

        // ---- the MAP half: a dropdown over the 32 pvp_* world stems. -------------------------
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("map:").color(Palette::MUTED));
            egui::ComboBox::from_id_salt("ghost-map")
                .selected_text(dests::map_label(&self.selected_map))
                .show_ui(ui, |ui| {
                    for stem in dests::PVP_MAPS {
                        ui.selectable_value(&mut self.selected_map, stem.to_string(), dests::map_label(stem));
                    }
                });
            if self.selected_map == dests::MAP_OBSERVED_LIVE {
                ui.label(RichText::new("observed-live").small().color(Palette::OK));
            } else {
                ui.label(RichText::new("[U] untried pair").small().color(Palette::WARN));
            }
        });
        // ⚠ Honest bound: the MAP is served by the Activity Host (`D1_AH_ACTIVITY=<stem>`), NOT by a
        // click-time poke ghost can safely make (the world-name lives at blob+0x688, published by
        // the AH; poking it needs a live-verified address). So selecting a map composes the AH
        // launch config below; the MODE button fires the client-side activity-index poke.
        ui.label(
            RichText::new(format!(
                "map is AH-served: set D1_AH_ACTIVITY={} at boot (or AH hot-swap). the mode button \
                 pokes the activity index client-side.",
                self.selected_map
            ))
            .small()
            .color(Palette::MUTED),
        );

        // The toggle: launchable-only by default; reveal the greyed reachability map on demand.
        ui.checkbox(&mut self.show_unlaunchable, "show unlaunchable (reachability map)");
        let show_unlaunchable = self.show_unlaunchable;
        let obs = self.observed();
        let launch_gate_ok = registry::find("launch-407")
            .map(|p| p.gate.unmet(&obs).is_none())
            .unwrap_or(false);
        // Collect the clicked MODE index first; firing borrows self mutably, so it happens after
        // the scroll's immutable borrow of self.dests ends.
        let mut to_launch: Option<usize> = None;
        ScrollArea::vertical().max_height(160.0).id_salt("ghost-dests").show(ui, |ui| {
            for d in &self.dests.rows {
                if d.launchable {
                    let resp = ui.add_enabled(
                        launch_gate_ok,
                        egui::Button::new(RichText::new(d.label()).color(Palette::ACCENT)),
                    );
                    if resp.clicked() {
                        to_launch = Some(d.index);
                    }
                } else if show_unlaunchable {
                    ui.label(
                        RichText::new(format!("{}  -  {}", d.label(), d.refusal()))
                            .small()
                            .color(Palette::MUTED),
                    );
                }
            }
        });
        // A launch is a registry Launch action; only 407 is wired as an entry today, so non-407
        // launchable modes are shown but not yet firable. Firing the mode also composes the AH map
        // config so the operator has the exact MODE x MAP launch line.
        if let Some(idx) = to_launch {
            let map = self.selected_map.clone();
            if let Some(p) = registry::find_launch(idx) {
                self.fire(&p);
                self.last_outcome = Some(format!(
                    "{} on {}: mode poked; map via D1_AH_ACTIVITY={} D1_AH_ACTIVITY_INDEX={}",
                    p.name, map, map, idx
                ));
            } else {
                self.last_outcome = Some(format!(
                    "mode {idx} on {map}: no Launch entry wired yet (only 407 is). Set \
                     D1_AH_ACTIVITY={map} D1_AH_ACTIVITY_INDEX={idx} and add a registry Launch entry."
                ));
            }
        }
    }

    fn ledger_tape(&mut self, ui: &mut egui::Ui) {
        if self.ledger_lines.is_empty() {
            ui.label(RichText::new("no ledgered pokes in this run").small().color(Palette::MUTED));
            return;
        }
        ScrollArea::vertical().max_height(220.0).id_salt("ghost-ledger").show(ui, |ui| {
            for l in &self.ledger_lines {
                let (txt, color) = match l {
                    Line::Intent { seq, name, target, rung, .. } => (
                        format!("#{seq} INTENT {name} [{target}] @rung {rung:?}"),
                        Palette::MUTED,
                    ),
                    Line::Result { seq, outcome, before, after, exit, .. } => {
                        let c = match outcome.as_str() {
                            "applied" => Palette::OK,
                            "mismatch" | "failed" => Palette::ERR,
                            "refused" => Palette::WARN,
                            _ => Palette::MUTED,
                        };
                        (
                            format!("#{seq} {} {before}->{after} (exit {exit:?})", outcome.to_uppercase()),
                            c,
                        )
                    }
                };
                ui.label(RichText::new(txt).monospace().small().color(color));
            }
        });
    }
}

impl eframe::App for RunningGhost {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.dash.tick_public(ctx);
        // Reload the ledger at most once a second - cheap, and keeps the tape fresh if another
        // process (a shell poke through the same file) appends.
        if self.last_ledger_read.elapsed() > Duration::from_secs(1) {
            self.reload_ledger();
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The console registers FIRST, so it owns the right edge; DashApp's own panels then fill
        // what remains (its CentralPanel is innermost).
        egui::Panel::right("ghost-console")
            .resizable(true)
            .default_size(520.0)
            .show(root, |ui| {
                ScrollArea::vertical().id_salt("ghost-console-scroll").show(ui, |ui| self.console(ui));
            });
        self.dash.draw(root);
    }
}

/// RFC3339 UTC from the wall clock, for the ledger timestamp. The crate takes no wall-clock inside
/// the testable logic (deterministic controls); it is read HERE, at the GUI edge, only.
fn rfc3339_now() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    // A minimal civil-time formatter (no chrono dep in this crate). Days since epoch -> y/m/d.
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (y, m, dd) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{dd:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard Hinnant's days-from-civil, inverted - integer only, no dependency.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
