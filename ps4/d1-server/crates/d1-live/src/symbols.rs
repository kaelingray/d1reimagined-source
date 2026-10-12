// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `capture-rig/guest-symbols.json`, read rather than re-implemented.
//!
//! ⚠ This crate **consumes** the project's one address book; it does not fork it. Every vaddr in
//! the file carries a `source` and a `confidence`, and the file is gated on the eboot's SHA-256 - 
//! a vaddr measured against a different build is precisely the plausible-looking-but-wrong input
//! `historicaldocs/design/runtime-state-access.md` §2 exists to stop. `d1guest.py` owns the file; if a symbol is
//! missing, add it *there*.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

/// Number, or a string like `"0x26A2040"` - the file uses both.
pub fn num(v: &Value) -> Result<u64> {
    match v {
        Value::Number(n) => n.as_u64().ok_or_else(|| anyhow!("not a u64: {n}")),
        Value::String(s) => {
            let s = s.trim();
            let (radix, body) = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                (16, h)
            } else {
                (10, s)
            };
            u64::from_str_radix(body, radix).with_context(|| format!("not a number: {s:?}"))
        }
        other => Err(anyhow!("not a number: {other}")),
    }
}

#[derive(Debug, Clone)]
pub struct Anchor {
    pub name: String,
    pub vaddr: u64,
    pub len: usize,
}

/// A `symbols[]` row, with the provenance the file carries for it.
///
/// ⚠ The provenance is not decoration. `d1guest.py`'s third refusal is *"say where an address came
/// from"*: `0x263D0F8` is `profile_setup_config_block`, not a magic number in a script, and every
/// command that resolves a name prints the `source` and `confidence` beside it.
#[derive(Debug, Clone)]
pub struct Sym {
    pub name: String,
    pub vaddr: u64,
    /// `ptr` / `u32` / `bytes` - advisory only; the reader chooses the view.
    pub kind: String,
    /// `bss` / `data` / `rodata` - what the FILE says, cross-checked against the image at read time.
    pub region: String,
    pub confidence: String,
    pub source: String,
    pub note: String,
    /// Default read length for this symbol, when the caller gives none.
    pub len: Option<usize>,
    /// Default `structs[]` name to decode with, when the caller gives none.
    pub strukt: Option<String>,
}

/// One field of a `structs[]` entry.
#[derive(Debug, Clone)]
pub struct Field {
    pub off: usize,
    pub name: String,
    /// One of `u8 u16 u32 u64 i32 i64 ptr` - the `FMT` table of `d1guest.py`.
    pub ty: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub size: usize,
    pub confidence: String,
    pub source: String,
    pub fields: Vec<Field>,
}

/// What a name resolved to, and how.
#[derive(Debug, Clone)]
pub enum Resolved {
    /// A `symbols[]` row.
    Named(Sym),
    /// A bare number, parsed as an eboot vaddr.
    Number(u64),
}

impl Resolved {
    pub fn vaddr(&self) -> u64 {
        match self {
            Resolved::Named(s) => s.vaddr,
            Resolved::Number(v) => *v,
        }
    }
    pub fn sym(&self) -> Option<&Sym> {
        match self {
            Resolved::Named(s) => Some(s),
            Resolved::Number(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FamilySlot {
    pub family: u64,
    pub typ: u64,
    pub storage: u64,
    pub cap: u64,
    pub size: u64,
    pub identity: String,
}

#[derive(Debug, Clone)]
pub struct Symbols {
    pub path: PathBuf,
    pub doc: Value,
    pub by_name: HashMap<String, u64>,
    pub anchors: Vec<Anchor>,
    pub verify: Anchor,
    pub eboot_rel: String,
    pub eboot_sha256: String,
    pub families: Vec<FamilySlot>,
    pub layout: HashMap<String, u64>,
    pub account_soid: u64,
    pub account_family: u64,
    pub account_type: u64,
    /// Every `symbols[]` row, in file order, with its provenance.
    pub symbols: Vec<Sym>,
    /// `structs[]`, by name.
    pub structs: HashMap<String, StructDef>,
    /// Struct names in **file order**.
    ///
    /// ⚠ `serde_json` stores an object as a `BTreeMap` unless the `preserve_order` feature is on,
    /// so iterating `structs` yields them ALPHABETICALLY. Python's `dict` keeps insertion order, so
    /// `d1guest.py syms` prints them in the order the file lists them. The difference is one line of
    /// output and it is the only thing that stopped the two tools being byte-identical - recorded
    /// here rather than "fixed" by sorting the Python's side of the diff.
    pub struct_order: Vec<String>,
    /// `package_registry_layout` - ⚠ a DIFFERENT registry from `registry_layout`.
    pub package_layout: HashMap<String, u64>,
}

/// The default location, relative to the repository root that contains `d1-server/`.
pub fn default_path() -> PathBuf {
    if let Ok(p) = std::env::var("D1_GUEST_SYMBOLS") {
        return PathBuf::from(p);
    }
    // …/d1-server/crates/d1-live/src/symbols.rs → repo root is three levels above `d1-server`.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")); // …/d1-server/crates/d1-live
    let repo = manifest
        .parent() // crates
        .and_then(|p| p.parent()) // d1-server
        .and_then(|p| p.parent()) // repo root
        .unwrap_or(manifest);
    repo.join("capture-rig/guest-symbols.json")
}

impl Symbols {
    pub fn open_default() -> Result<Self> {
        Self::open(default_path())
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading the guest symbol file {}", path.display()))?;
        let doc: Value = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))?;

        let mut by_name = HashMap::new();
        let mut symbols = Vec::new();
        for s in doc["symbols"].as_array().unwrap_or(&vec![]) {
            let n = s["name"].as_str().unwrap_or_default().to_string();
            if let Ok(v) = num(&s["vaddr"]) {
                by_name.insert(n.clone(), v);
                symbols.push(Sym {
                    name: n,
                    vaddr: v,
                    kind: s["type"].as_str().unwrap_or("").to_string(),
                    region: s["region"].as_str().unwrap_or("").to_string(),
                    confidence: s["confidence"].as_str().unwrap_or("").to_string(),
                    source: s["source"].as_str().unwrap_or("").to_string(),
                    note: s["note"].as_str().unwrap_or("").to_string(),
                    len: num(&s["len"]).ok().map(|n| n as usize),
                    strukt: s["struct"].as_str().map(|x| x.to_string()),
                });
            }
        }

        let mut structs = HashMap::new();
        let mut struct_order: Vec<String> = Vec::new();
        if let Some(o) = doc["structs"].as_object() {
            // File order, recovered from the raw text: the position of each `"name":` key inside the
            // `"structs"` object.
            let anchor = text.find("\"structs\"").unwrap_or(0);
            let mut with_pos: Vec<(usize, String)> = o
                .keys()
                .map(|k| (text[anchor..].find(&format!("\"{k}\"")).unwrap_or(usize::MAX), k.clone()))
                .collect();
            with_pos.sort();
            struct_order = with_pos.into_iter().map(|(_, k)| k).collect();
            for (name, sd) in o {
                let mut fields = Vec::new();
                for f in sd["fields"].as_array().unwrap_or(&vec![]) {
                    fields.push(Field {
                        off: num(&f["off"]).unwrap_or(0) as usize,
                        name: f["name"].as_str().unwrap_or("?").to_string(),
                        ty: f["type"].as_str().unwrap_or("u64").to_string(),
                        note: f["note"].as_str().unwrap_or("").to_string(),
                    });
                }
                structs.insert(
                    name.clone(),
                    StructDef {
                        name: name.clone(),
                        size: num(&sd["size"]).unwrap_or(0) as usize,
                        confidence: sd["confidence"].as_str().unwrap_or("").to_string(),
                        source: sd["source"].as_str().unwrap_or("").to_string(),
                        fields,
                    },
                );
            }
        }

        let mut package_layout = HashMap::new();
        if let Some(o) = doc["package_registry_layout"].as_object() {
            for (k, v) in o {
                if let Ok(n) = num(v) {
                    package_layout.insert(k.clone(), n);
                }
            }
        }

        let mut anchors = Vec::new();
        for a in doc["anchors"].as_array().unwrap_or(&vec![]) {
            anchors.push(Anchor {
                name: a["name"].as_str().unwrap_or("?").to_string(),
                vaddr: num(&a["vaddr"])?,
                len: num(&a["len"])? as usize,
            });
        }
        if anchors.len() < 2 {
            return Err(anyhow!(
                "{}: needs at least two independent anchors - one signature that happens to occur \
                 twice is exactly the failure the two-anchor rule exists to catch",
                path.display()
            ));
        }

        let verify = Anchor {
            name: "verify".into(),
            vaddr: num(&doc["verify"]["vaddr"])?,
            len: num(&doc["verify"]["len"])? as usize,
        };

        let mut families = Vec::new();
        for f in doc["families"].as_array().unwrap_or(&vec![]) {
            families.push(FamilySlot {
                family: num(&f["family"])?,
                typ: num(&f["type"])?,
                storage: num(&f["storage"])?,
                cap: num(&f["cap"])?,
                size: num(&f["size"])?,
                identity: f["identity"].as_str().unwrap_or("").to_string(),
            });
        }

        let mut layout = HashMap::new();
        if let Some(o) = doc["registry_layout"].as_object() {
            for (k, v) in o {
                if let Ok(n) = num(v) {
                    layout.insert(k.clone(), n);
                }
            }
        }

        Ok(Self {
            eboot_rel: doc["build"]["eboot"].as_str().unwrap_or_default().to_string(),
            eboot_sha256: doc["build"]["sha256"].as_str().unwrap_or_default().to_string(),
            account_soid: num(&doc["liveness"]["account_soid"]).unwrap_or(0),
            account_family: num(&doc["liveness"]["account_family"]).unwrap_or(4),
            account_type: num(&doc["liveness"]["account_type"]).unwrap_or(0),
            path,
            doc,
            by_name,
            anchors,
            verify,
            families,
            layout,
            symbols,
            structs,
            struct_order,
            package_layout,
        })
    }

    pub fn lookup(&self, name: &str) -> Option<u64> {
        self.by_name.get(name).copied()
    }

    pub fn sym(&self, name: &str) -> Option<&Sym> {
        self.symbols.iter().find(|s| s.name == name)
    }

    /// The static family/slot row for `(family, type)`, or `None` when the file has no row - which
    /// means the runtime-versus-static cross-check is **SKIPPED**, not passed.
    pub fn family(&self, family: u64, typ: u64) -> Option<&FamilySlot> {
        self.families.iter().find(|f| f.family == family && f.typ == typ)
    }

    /// `d1guest.py`'s `Symbols.resolve`: a `symbols[]` name, or a number in any base.
    ///
    /// ⚠ The refusal is the point. An unknown token is **not** silently treated as an address; a
    /// typo that resolved to a plausible number would read the wrong place and print bytes.
    pub fn resolve(&self, token: &str) -> Result<Resolved> {
        if let Some(s) = self.sym(token) {
            return Ok(Resolved::Named(s.clone()));
        }
        let t = token.trim();
        let parsed = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u64::from_str_radix(h, 16).ok()
        } else if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            u64::from_str_radix(b, 2).ok()
        } else if let Some(o) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            u64::from_str_radix(o, 8).ok()
        } else {
            t.parse::<u64>().ok()
        };
        parsed.map(Resolved::Number).ok_or_else(|| {
            anyhow!(
                "unknown symbol and not a number: {token:?}  (try `d1-live syms`; add new addresses \
                 to {} with a `source` and a `confidence`, never to a script)",
                self.path.display()
            )
        })
    }

    /// The build gate. A vaddr measured against a different eboot is not a vaddr.
    pub fn check_build(&self, sha256: &str) -> (bool, String) {
        let ok = self.eboot_sha256 == sha256;
        (
            ok,
            format!(
                "BUILD GATE: {}  eboot sha256 {}… {} symbol-file {}…",
                if ok { "PASS" } else { "FAIL" },
                &sha256[..16.min(sha256.len())],
                if ok { "==" } else { "!=" },
                &self.eboot_sha256[..16.min(self.eboot_sha256.len())],
            ),
        )
    }
}
