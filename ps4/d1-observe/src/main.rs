// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use d1_observe::{first_divergence, load_run, reduce, snapshot_at, LoadOptions, RunData};

#[derive(Debug, Parser)]
#[command(
    name = "d1-observe",
    about = "Replay and explain Destiny 1 protocol conversations without mutating a run"
)]
struct Cli {
    /// Existing Python migration adapter for legacy responder/emulator logs.
    #[arg(long, global = true)]
    adapter: Option<PathBuf>,

    /// Emit stable machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate and summarize an event file or preserved run directory.
    Validate { source: PathBuf },
    /// Print the semantic event timeline.
    Timeline {
        source: PathBuf,
        /// Show only failed/confounded events.
        #[arg(long)]
        failures: bool,
    },
    /// Show the modeled in-flight queue and semantic state at one event.
    Queue {
        source: PathBuf,
        /// Event index or stable event id. Defaults to the final event.
        #[arg(long)]
        at: Option<String>,
    },
    /// Explain one event using the state immediately after it.
    Explain {
        source: PathBuf,
        /// Event index or stable event id. Defaults to the final event.
        #[arg(long)]
        at: Option<String>,
    },
    /// List all modeled requests still in flight at the final event.
    Unanswered { source: PathBuf },
    /// Report the earliest proven broken semantic edge.
    WhyStuck { source: PathBuf },
    /// Find the first semantic divergence between two runs.
    FirstDivergence { a: PathBuf, b: PathBuf },
    /// Produce a compact Markdown incident block.
    ExportIncident {
        source: PathBuf,
        #[arg(long)]
        at: Option<String>,
    },
    /// Open the read-only native replay UI.
    #[cfg(feature = "gui")]
    Gui {
        source: PathBuf,
        /// Optional comparison run displayed in the divergence pane.
        #[arg(long)]
        compare: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let options = LoadOptions { adapter: cli.adapter.clone() };
    match cli.command {
        Command::Validate { source } => cmd_validate(&source, &options, cli.json),
        Command::Timeline { source, failures } => {
            cmd_timeline(&source, &options, failures, cli.json)
        }
        Command::Queue { source, at } => cmd_queue(&source, &options, at.as_deref(), cli.json),
        Command::Explain { source, at } => {
            cmd_explain(&source, &options, at.as_deref(), cli.json)
        }
        Command::Unanswered { source } => cmd_unanswered(&source, &options, cli.json),
        Command::WhyStuck { source } => cmd_why_stuck(&source, &options, cli.json),
        Command::FirstDivergence { a, b } => cmd_diff(&a, &b, &options, cli.json),
        Command::ExportIncident { source, at } => {
            cmd_export_incident(&source, &options, at.as_deref())
        }
        #[cfg(feature = "gui")]
        Command::Gui { source, compare } => {
            let run = load_run(&source, &options)?;
            let compare = compare
                .as_deref()
                .map(|path| load_run(path, &options))
                .transpose()?;
            d1_observe::gui::run(run, compare)
        }
    }
}

fn load(path: &Path, options: &LoadOptions) -> Result<RunData> {
    load_run(path, options).with_context(|| format!("load {}", path.display()))
}

fn cmd_validate(path: &Path, options: &LoadOptions, json: bool) -> Result<()> {
    let run = load(path, options)?;
    let reduced = reduce(&run);
    if json {
        println!("{}", serde_json::to_string_pretty(&reduced)?);
    } else {
        println!("run: {}", reduced.run_id);
        println!("events: {}", reduced.event_count);
        println!("ordering: {:?}", reduced.ordering);
        println!("diagnoses: {}", reduced.diagnoses.len());
        println!("source: {}", run.source_description);
        for warning in &reduced.warnings {
            println!("warning: {warning}");
        }
    }
    Ok(())
}

fn cmd_timeline(
    path: &Path,
    options: &LoadOptions,
    failures: bool,
    json: bool,
) -> Result<()> {
    let run = load(path, options)?;
    let reduced = reduce(&run);
    let rows: Vec<_> = reduced
        .timeline
        .iter()
        .filter(|row| !failures || row.status == "failed" || format!("{:?}", row.evidence).contains("Confounded"))
        .cloned()
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("idx    source:seq            dir   type/message             sequence  stage                              status");
        for row in rows {
            println!(
                "{:<6} {:<21} {:<5} {:<24} {:<9} {:<34} {}",
                row.index,
                format!("{}:{}", row.source, row.source_sequence),
                row.direction,
                row.message,
                row.inner_sequence
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                row.stage,
                row.status
            );
        }
    }
    Ok(())
}

fn cmd_queue(
    path: &Path,
    options: &LoadOptions,
    at: Option<&str>,
    json: bool,
) -> Result<()> {
    let run = load(path, options)?;
    let index = resolve_index(&run, at)?;
    let explanation = snapshot_at(&run, index)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&explanation.snapshot)?);
    } else {
        print_queue(&explanation.snapshot);
    }
    Ok(())
}

fn cmd_explain(
    path: &Path,
    options: &LoadOptions,
    at: Option<&str>,
    json: bool,
) -> Result<()> {
    let run = load(path, options)?;
    let index = resolve_index(&run, at)?;
    let explanation = snapshot_at(&run, index)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&explanation)?);
    } else {
        println!(
            "event {} · {}:{} · {} · {}",
            explanation.event.index,
            explanation.event.source,
            explanation.event.source_sequence,
            explanation.event.stage,
            explanation.event.status
        );
        if let Some(diagnosis) = &explanation.diagnosis {
            println!("cause: {}", diagnosis.explanation());
        } else {
            println!("cause: no causal diagnosis is proven at this event");
        }
        print_queue(&explanation.snapshot);
    }
    Ok(())
}

fn cmd_unanswered(path: &Path, options: &LoadOptions, json: bool) -> Result<()> {
    let run = load(path, options)?;
    let reduced = reduce(&run);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&reduced.final_snapshot.queues)?
        );
    } else {
        print_queue(&reduced.final_snapshot);
    }
    Ok(())
}

fn cmd_why_stuck(path: &Path, options: &LoadOptions, json: bool) -> Result<()> {
    let run = load(path, options)?;
    let reduced = reduce(&run);
    let diagnosis = reduced.diagnoses.first();
    let queue_instrumentation_absent = reduced
        .warnings
        .iter()
        .any(|warning| warning.contains("queue-insertion"));
    let reason = if let Some(diagnosis) = diagnosis {
        diagnosis.explanation().to_string()
    } else if queue_instrumentation_absent {
        "no proven blocker: client queue instrumentation is absent, so the last visible message cannot be promoted into a cause".to_string()
    } else {
        "no proven blocker in the captured semantic edges".to_string()
    };
    if json {
        let report = serde_json::json!({
            "proven": diagnosis.is_some(),
            "diagnosis": diagnosis,
            "reason": reason,
            "evidence_event": diagnosis.map(|value| value.event_index()),
            "warnings": reduced.warnings,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if let Some(diagnosis) = diagnosis {
        println!("{reason}");
        println!("evidence event: {}", diagnosis.event_index());
    } else {
        println!("{reason}");
    }
    Ok(())
}

fn cmd_diff(
    a_path: &Path,
    b_path: &Path,
    options: &LoadOptions,
    json: bool,
) -> Result<()> {
    let a = load(a_path, options)?;
    let b = load(b_path, options)?;
    let divergence = first_divergence(&a, &b);
    if json {
        println!("{}", serde_json::to_string_pretty(&divergence)?);
    } else {
        println!("{}", divergence.explanation);
        for (key, (before, after)) in divergence.changed_provenance {
            println!(
                "knob {key}: {} -> {}",
                before.as_deref().unwrap_or("<unset>"),
                after.as_deref().unwrap_or("<unset>")
            );
        }
    }
    Ok(())
}

fn cmd_export_incident(path: &Path, options: &LoadOptions, at: Option<&str>) -> Result<()> {
    let run = load(path, options)?;
    let index = resolve_index(&run, at)?;
    let explanation = snapshot_at(&run, index)?;
    println!("### Protocol incident `{}` event {}\n", run.run_id, index);
    println!("- **Event:** `{}` / `{}`", explanation.event.stage, explanation.event.status);
    println!("- **Evidence id:** `{}`", explanation.event.event_id);
    if let Some(diagnosis) = explanation.diagnosis {
        println!("- **Earliest causal edge at this event:** {}", diagnosis.explanation());
    } else {
        println!("- **Causal verdict:** not proven at this event");
    }
    println!("- **Boot state:** `{}`", explanation.snapshot.boot_state.as_deref().unwrap_or("not observed"));
    println!("- **World:** `{}`", explanation.snapshot.world.as_deref().unwrap_or("not observed"));
    let pending: usize = explanation.snapshot.queues.values().map(Vec::len).sum();
    println!("- **Modeled in-flight requests:** {pending}");
    Ok(())
}

fn resolve_index(run: &RunData, at: Option<&str>) -> Result<usize> {
    let Some(at) = at else {
        return run.events.len().checked_sub(1).context("run contains no events");
    };
    if let Ok(index) = at.parse::<usize>() {
        if index < run.events.len() {
            return Ok(index);
        }
        bail!("event index {index} is outside 0..{}", run.events.len());
    }
    run.events
        .iter()
        .enumerate()
        .find(|(index, event)| event.stable_id(*index) == at)
        .map(|(index, _)| index)
        .with_context(|| format!("no event id '{at}'"))
}

fn print_queue(snapshot: &d1_observe::Snapshot) {
    if snapshot.queues.values().all(Vec::is_empty) {
        println!("in-flight queue: empty or not observed");
        return;
    }
    for (connection, queue) in &snapshot.queues {
        println!("in-flight queue {connection}:");
        println!("  pos   request                  seq   expected reply   evidence");
        for (position, request) in queue.iter().enumerate() {
            println!(
                "  {:<5} {:<24} {:<5} {:<16} {:?}",
                if position == 0 { "HEAD" } else { "" },
                request.label(),
                request.sequence,
                request
                    .expected_reply_type
                    .map(|value| format!("0x{value:04x}"))
                    .unwrap_or_else(|| "unknown".to_string()),
                request.evidence
            );
        }
    }
}
