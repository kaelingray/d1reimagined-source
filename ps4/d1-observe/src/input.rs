// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::model::{Event, RunData};

#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    pub adapter: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct LedgerEnvelope {
    run_id: String,
    events: Vec<Event>,
    #[serde(default)]
    coverage: BTreeMap<String, Value>,
}

pub fn load_run(path: &Path, options: &LoadOptions) -> Result<RunData> {
    if path.is_dir() {
        return load_run_directory(path, options);
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    load_bytes(&bytes, path.display().to_string())
}

fn load_run_directory(path: &Path, options: &LoadOptions) -> Result<RunData> {
    for name in ["conversation-events.jsonl", "send-receive-events.jsonl", "events.jsonl"] {
        let candidate = path.join(name);
        if candidate.is_file() {
            let mut run = load_run(&candidate, options)?;
            run.provenance = read_provenance(path);
            return Ok(run);
        }
    }

    let adapter = options
        .adapter
        .clone()
        .or_else(|| discover_adapter(path))
        .ok_or_else(|| {
            anyhow!(
                "{} contains no normalized event JSONL and no send-receive adapter was found; pass --adapter",
                path.display()
            )
        })?;
    let output = Command::new("python3")
        .arg(&adapter)
        .arg(path)
        .arg("--json")
        .output()
        .with_context(|| format!("launch migration adapter {}", adapter.display()))?;
    if !output.status.success() {
        bail!(
            "migration adapter failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut run = load_bytes(
        &output.stdout,
        format!("{} via {}", path.display(), adapter.display()),
    )?;
    run.provenance = read_provenance(path);
    Ok(run)
}

fn discover_adapter(run_dir: &Path) -> Option<PathBuf> {
    let cwd = PathBuf::from("capture-rig/send-receive-ledger.py");
    if cwd.is_file() {
        return Some(cwd);
    }
    let repo = run_dir.parent()?.parent()?;
    let candidate = repo.join("capture-rig/send-receive-ledger.py");
    candidate.is_file().then_some(candidate)
}

fn read_provenance(run_dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Ok(text) = fs::read_to_string(run_dir.join("RUN-INFO.txt")) {
        for line in text.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !value.is_empty() {
                    out.insert(key.to_string(), value.to_string());
                }
            }
        }
    }
    out
}

fn load_bytes(bytes: &[u8], source_description: String) -> Result<RunData> {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        bail!("empty event input: {source_description}");
    }

    if trimmed.starts_with('{') {
        if let Ok(envelope) = serde_json::from_str::<LedgerEnvelope>(&text) {
            return Ok(RunData {
                run_id: envelope.run_id,
                events: envelope.events,
                coverage: envelope.coverage,
                provenance: BTreeMap::new(),
                source_description,
            });
        }
        if let Ok(event) = serde_json::from_str::<Event>(&text) {
            return Ok(RunData {
                run_id: event.run_id.clone(),
                events: vec![event],
                coverage: BTreeMap::new(),
                provenance: BTreeMap::new(),
                source_description,
            });
        }
        // A JSONL stream also begins with `{`. If parsing the whole input as
        // one object reports trailing characters, continue with line parsing.
    }

    if trimmed.starts_with('[') {
        let events: Vec<Event> = serde_json::from_str(&text)
            .with_context(|| format!("parse event array from {source_description}"))?;
        let run_id = common_run_id(&events)?;
        return Ok(RunData {
            run_id,
            events,
            coverage: BTreeMap::new(),
            provenance: BTreeMap::new(),
            source_description,
        });
    }

    let mut events = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event: Event = serde_json::from_str(line).with_context(|| {
            format!("parse JSONL line {} from {source_description}", line_index + 1)
        })?;
        events.push(event);
    }
    let run_id = common_run_id(&events)?;
    Ok(RunData {
        run_id,
        events,
        coverage: BTreeMap::new(),
        provenance: BTreeMap::new(),
        source_description,
    })
}

fn common_run_id(events: &[Event]) -> Result<String> {
    let Some(first) = events.first() else {
        bail!("event stream contains no events");
    };
    if events.iter().any(|event| event.run_id != first.run_id) {
        bail!("event stream mixes multiple run_id values");
    }
    Ok(first.run_id.clone())
}
