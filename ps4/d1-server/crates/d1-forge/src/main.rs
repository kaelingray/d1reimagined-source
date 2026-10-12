// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-forge` - the loadout / spawn editor.
//!
//! Browse the item catalogue, build a loadout, save it as `d1-loadout` JSON, and hand it to the
//! control plane over HTTP.
//!
//! # Why this is native egui and not another served HTML page
//!
//! `historicaldocs/design/interface-architecture.md` §5 held a **named bet**: served HTML for tables and diffs,
//! egui the day a "visualising structure" need lands. That trigger fired on 2026-08-07. This app is
//! on the egui side of the line for a narrower reason than the debugger was - it is a *stateful
//! editor* with a 5,000-row virtualized table, drag-values, live validation and a background HTTP
//! call, and a poll-every-300ms HTML page has no story for any of that.
//!
//! # Version pinning - verified, not recalled
//!
//! `egui` / `eframe` / `egui_extras` / `egui_kittest` are pinned **as a set** at **0.36.1**,
//! resolved by `cargo search` on **2026-08-08**. `historicaldocs/design/interface-architecture.md` records a
//! session asserting a version that did not exist, so the pin was checked by asking cargo and the
//! `App` trait shape was checked by *reading* `eframe-0.36.1/src/epi.rs` - where the
//! `logic()` + `ui()` split that doc calls "0.34-era and likely stale" turns out to still be the
//! current API. It is used here as the doc describes: channel drains in `logic`, widgets in `ui`.
//!
//! # It runs nothing
//!
//! No emulator, no server, no capture. It reads a JSON catalogue and POSTs to a URL you give it.

mod app;
mod control;
mod theme;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use d1_loadout::Catalogue;

#[derive(Parser)]
#[command(name = "d1-forge", about = "Destiny 1 loadout / spawn editor")]
struct Cli {
    /// Catalogue JSON, built by `d1-loadout build`.
    #[arg(long, default_value = "catalogue/d1-catalogue.json")]
    catalogue: PathBuf,

    /// Directory of saved loadouts.
    #[arg(long, default_value = "loadouts")]
    loadouts: PathBuf,

    /// Control-plane base URL for Apply. Loopback; `ssh -L` is the auth layer.
    #[arg(long, default_value = "http://127.0.0.1:8788")]
    control_url: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load before the window opens, and keep the *reason* on failure rather than an empty list -
    // "no items match" and "the catalogue never loaded" must never look alike on screen.
    let catalogue = std::fs::read_to_string(&cli.catalogue)
        .map_err(|e| format!("{}: {e}", cli.catalogue.display()))
        .and_then(|s| Catalogue::from_json(&s).map_err(|e| format!("{}: {e:#}", cli.catalogue.display())));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 600.0])
            .with_title("d1-forge - Destiny 1 loadout editor"),
        ..Default::default()
    };

    eframe::run_native(
        "d1-forge",
        options,
        Box::new(move |cc| {
            let fonts = theme::install_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::Forge::new(
                catalogue,
                cli.loadouts.clone(),
                cli.control_url.clone(),
                fonts,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}
