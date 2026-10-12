// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The GUI: one static page plus a JSON endpoint, served on loopback.
//!
//! Why served HTML rather than a native toolkit - this is the decision recorded in
//! `historicaldocs/design/interface-architecture.md` §5, and it holds:
//!   * zero extra build targets and no framework or asset pipeline (the page is `include_str!`);
//!   * works from a phone over the same `ssh -L` tunnel used for everything else;
//!   * screenshots straight into docs;
//!   * and it renders **the same `RunSummary` the CLI prints**, so the two views cannot drift.
//!
//! The egui trigger - timeline, plots, scrubbing - has not fired. When it does, it gets built then
//! rather than ported to.
//!
//! Polling rather than streaming is deliberate: every response is complete state, so a flaky tunnel
//! is self-healing and there is no reconnect logic to get wrong.

use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::{extract::State, response::Html, routing::get, Json, Router};
use d1_run::{list_runs, summarize_run, Quiet, QuietThresholds, RunSummary};

/// Summary plus the quietness verdict, which the page needs but `RunSummary` doesn't carry.
#[derive(serde::Serialize)]
struct RunView {
    #[serde(flatten)]
    summary: RunSummary,
    quiet: String,
}

fn collect(runs_dir: &PathBuf) -> Vec<RunView> {
    let t = QuietThresholds::default();
    list_runs(runs_dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|d| summarize_run(d).ok())
        .map(|summary| {
            let quiet = match summary.provenance.quiet(&t) {
                Quiet::Yes => "Yes".to_string(),
                Quiet::Unknown => "Unknown".to_string(),
                Quiet::No(_) => "No".to_string(),
            };
            RunView { summary, quiet }
        })
        .collect()
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn runs_json(State(dir): State<PathBuf>) -> Json<Vec<RunView>> {
    Json(collect(&dir))
}

pub fn run(runs_dir: PathBuf, bind: std::net::SocketAddr) -> Result<()> {
    if !bind.ip().is_loopback() {
        eprintln!(
            "warning: binding {bind}, which is not loopback. There is no auth here - \
             use `ssh -L` instead unless you mean this."
        );
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to start async runtime")?;
    rt.block_on(async move {
        let app = Router::new()
            .route("/", get(index))
            .route("/runs.json", get(runs_json))
            .with_state(runs_dir.clone());
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .with_context(|| format!("failed to bind {bind}"))?;
        println!("d1 inspector on http://{bind}  (runs: {})", runs_dir.display());
        axum::serve(listener, app).await.context("server error")
    })
}
