// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `capture-rig/surfaces/registry.toml`, **parsed and never rewritten**.
//!
//! ⛔ A tool that regenerates its own input eats its own output (`status/LESSONS.md` #4). The
//! `toml` dependency is pinned `default-features = false, features = ["parse","serde"]` precisely
//! so that the *serialiser* is not linked: d1-scope cannot write this file even by accident.
//!
//! ⭐ This module owns **no address parser**. Every `addr` string here is handed to
//! [`crate::eval`], which hands it to [`d1_live::addr::addr`] - the same function
//! `d1-live watch --addr` calls.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The reserved `addr` value meaning *"this surface needs a Rust resolver"* - its input expression
/// and the resolver's name are in `addr_input` / `resolver`.
pub const NOT_EXPRESSIBLE: &str = "NOT-EXPRESSIBLE";
/// The reserved `addr` value meaning *"nobody knows where this lives"* - see the matching
/// `[[open]]` entry. ⛔ Renders as `Unknown`, never as an address and never as zero.
pub const UNKNOWN_ADDR: &str = "[U]";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Meta {
    pub version: String,
    pub generated: String,
    pub generator: String,
    pub eboot_sha256: String,
    pub eboot: String,
    pub grammar: String,
    pub symbols: String,
    pub groups: Vec<String>,
    pub edge_states: Vec<String>,
    #[serde(default)]
    pub snapshot: Vec<SnapshotMeta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub path: String,
    pub state: String,
    pub run: String,
    pub size: String,
    pub coverage: String,
    /// ⚠ `true` ⇒ at least one field in this snapshot was WRITTEN BY US. `poked_fields` names them.
    /// A value read out of a poked field is not the client's value and must never be cited as one.
    #[serde(default)]
    pub poked: bool,
    #[serde(default)]
    pub poked_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Field {
    pub name: String,
    pub off: u64,
    pub ty: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Control {
    pub snap: String,
    pub expr: String,
    pub value: String,
    pub verified: bool,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Surface {
    pub name: String,
    pub group: String,
    pub addr: String,
    /// Present only when `addr == NOT-EXPRESSIBLE`: the expression whose **value** feeds `resolver`.
    #[serde(default)]
    pub addr_input: Option<String>,
    #[serde(default)]
    pub addr_input_width: Option<String>,
    #[serde(default)]
    pub resolver: Option<String>,
    pub rw: String,
    pub confidence: String,
    pub reach: String,
    pub liveness: String,
    pub source: String,
    pub note: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub controls: Vec<Control>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Edge {
    pub id: String,
    pub up: String,
    pub down: String,
    pub mech: String,
    pub green: String,
    pub red: String,
    pub state: String,
    pub as_of: String,
    pub evaluable_offline: bool,
    pub source: String,
    pub note: String,
    #[serde(default)]
    pub guard: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Open {
    pub id: String,
    pub surface: String,
    pub question: String,
    pub instrument: String,
    pub why_it_matters: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Registry {
    pub meta: Meta,
    #[serde(default, rename = "surface")]
    pub surfaces: Vec<Surface>,
    #[serde(default, rename = "edge")]
    pub edges: Vec<Edge>,
    #[serde(default, rename = "open")]
    pub opens: Vec<Open>,
    #[serde(skip)]
    pub path: PathBuf,
}

impl Registry {
    pub fn open_default() -> Result<Self> {
        Self::open(crate::default_registry_path())
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading the surface registry {}", path.display()))?;
        let mut r: Registry = toml::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))?;
        r.path = path;
        Ok(r)
    }

    pub fn surface(&self, name: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.name == name)
    }

    pub fn snapshot_meta(&self, id: &str) -> Option<&SnapshotMeta> {
        self.meta.snapshot.iter().find(|s| s.id == id)
    }

    /// Every distinct snapshot id named by a control, minus the `n/a` sentinel.
    pub fn control_snapshot_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .surfaces
            .iter()
            .flat_map(|s| s.controls.iter())
            .map(|c| c.snap.clone())
            .filter(|s| s != "n/a")
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// ⛔ Dangling-endpoint check: every edge endpoint is either a surface name or a namespaced
    /// external (`server:…`, `client:…`, `log:…`). A dangling endpoint means the graph lies.
    pub fn dangling_endpoints(&self) -> Vec<String> {
        let mut out = Vec::new();
        for e in &self.edges {
            for (which, ep) in [("up", &e.up), ("down", &e.down)] {
                if ep.contains(':') {
                    continue; // an external endpoint, namespaced on purpose
                }
                if self.surface(ep).is_none() {
                    out.push(format!("{} {which} = {ep:?}", e.id));
                }
            }
        }
        out
    }
}

impl Surface {
    pub fn is_not_expressible(&self) -> bool {
        self.addr == NOT_EXPRESSIBLE
    }
    pub fn is_unknown_addr(&self) -> bool {
        self.addr == UNKNOWN_ADDR
    }
    /// Does this surface's `addr` go straight to the shared grammar?
    pub fn addr_is_expression(&self) -> bool {
        !self.is_not_expressible() && !self.is_unknown_addr()
    }

    /// ⭐ Control #5's other half: a surface must name the evidence it came from, and that
    /// evidence must be a path that EXISTS. A citation nobody can open is a citation nobody
    /// checked. Returns the tree paths this surface's `source` names, and which of them resolve.
    pub fn cited_paths(&self, root: &Path) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        for tok in self
            .source
            .split(|c: char| c.is_whitespace() || c == ';' || c == ',' || c == '(' || c == ')')
        {
            let t = tok.trim_end_matches(|c: char| c == '.' || c == ':');
            let looks_like_path = t.contains('/')
                && (t.ends_with(".md")
                    || t.ends_with(".py")
                    || t.ends_with(".json")
                    || t.ends_with(".tsv")
                    || t.ends_with(".rs")
                    || t.ends_with(".toml"));
            if looks_like_path {
                out.push((t.to_string(), root.join(t).exists()));
            }
        }
        out
    }
}
