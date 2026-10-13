// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The executable-build registry: `capture-rig/builds.json`.
//!
//! One place that says which owned executable a tool targets. `$D1_BUILD` selects an entry;
//! otherwise the registry's `default`. The registry's `reference` build is the one every fixed
//! address in `design/`, the probe-site catalogues and the BAP tables was measured against.
//!
//! ⚠ A build whose `addresses_valid` is false has no re-derived addresses. Anything that uses a
//! fixed game address must check [`Build::addresses_valid`] (or compare the image hash with the
//! catalogue's own derivation hash) and refuse, never fall back to the reference's numbers.
//!
//! `$D1_BUILDS` overrides the registry path (tests, alternate checkouts). Relative paths inside
//! the registry are relative to the repository root, which is the registry's grandparent.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

/// The registry beside the source tree.
pub const DEFAULT_REGISTRY: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../capture-rig/builds.json");

#[derive(Debug, Clone, Deserialize)]
pub struct Build {
    #[serde(skip)]
    pub id: String,
    #[serde(skip)]
    pub root: PathBuf,
    /// The game's own `APP_VER`, once observed. `None` until the build reports it.
    pub app_version: Option<String>,
    pub build_label: String,
    pub title_id: String,
    pub game_dir: String,
    pub packages: String,
    /// The plain ELF static tools read.
    pub eboot: String,
    pub eboot_sha256: String,
    pub eboot_bytes: u64,
    /// The file the emulator is launched with (may be the SELF wrapper of `eboot`).
    pub launch: String,
    pub launch_sha256: String,
    pub launch_bytes: u64,
    pub manifest: String,
    /// `None` until the manifest for this build has been generated and recorded.
    pub manifest_sha256: Option<String>,
    pub manifest_bytes: Option<u64>,
    /// A client patch written for this build, if any. Patches are address-specific.
    pub diag_patch: Option<String>,
    pub addresses_valid: bool,
}

#[derive(Debug, Deserialize)]
struct Registry {
    schema: u32,
    default: String,
    reference: String,
    builds: BTreeMap<String, Build>,
}

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, serde_json::Error),
    Schema(u32),
    Unknown { id: String, known: Vec<String> },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "build registry {}: {e}", p.display()),
            Self::Parse(p, e) => write!(f, "build registry {}: {e}", p.display()),
            Self::Schema(s) => write!(f, "build registry schema {s} is not 1"),
            Self::Unknown { id, known } => {
                write!(f, "unknown build `{id}` (D1_BUILD); registry has: {}", known.join(", "))
            }
        }
    }
}

impl std::error::Error for Error {}

impl Build {
    /// Resolve a registry-relative path against the repository root.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    pub fn eboot_path(&self) -> PathBuf {
        self.path(&self.eboot)
    }
    pub fn launch_path(&self) -> PathBuf {
        self.path(&self.launch)
    }
    pub fn packages_path(&self) -> PathBuf {
        self.path(&self.packages)
    }
    pub fn manifest_path(&self) -> PathBuf {
        self.path(&self.manifest)
    }
}

/// A loaded registry: every build, plus which one is selected and which is the reference.
#[derive(Debug)]
pub struct Builds {
    pub selected: Build,
    pub reference: Build,
    pub all: Vec<Build>,
}

/// Load `registry`, selecting `want` (or the registry default).
pub fn load(registry: &Path, want: Option<&str>) -> Result<Builds, Error> {
    let text = std::fs::read_to_string(registry).map_err(|e| Error::Io(registry.into(), e))?;
    let reg: Registry =
        serde_json::from_str(&text).map_err(|e| Error::Parse(registry.into(), e))?;
    if reg.schema != 1 {
        return Err(Error::Schema(reg.schema));
    }
    let root = registry
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let all: Vec<Build> = reg
        .builds
        .into_iter()
        .map(|(id, mut b)| {
            b.id = id;
            b.root = root.clone();
            b
        })
        .collect();
    let find = |id: &str| {
        all.iter().find(|b| b.id == id).cloned().ok_or_else(|| Error::Unknown {
            id: id.to_string(),
            known: all.iter().map(|b| b.id.clone()).collect(),
        })
    };
    let selected = find(want.unwrap_or(&reg.default))?;
    let reference = find(&reg.reference)?;
    Ok(Builds { selected, reference, all })
}

fn registry_path() -> PathBuf {
    std::env::var_os("D1_BUILDS").map(PathBuf::from).unwrap_or_else(|| DEFAULT_REGISTRY.into())
}

static LOADED: OnceLock<Result<Builds, String>> = OnceLock::new();

/// The process-wide registry, honouring `$D1_BUILDS` and `$D1_BUILD`.
pub fn builds() -> Result<&'static Builds, String> {
    LOADED
        .get_or_init(|| {
            let want = std::env::var("D1_BUILD").ok().filter(|s| !s.is_empty());
            load(&registry_path(), want.as_deref()).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// The selected build. Panics with the registry error: a tool with no build identity must not run.
pub fn selected() -> &'static Build {
    &builds().unwrap_or_else(|e| panic!("{e}")).selected
}

/// The address reference build (01.29).
pub fn reference() -> &'static Build {
    &builds().unwrap_or_else(|e| panic!("{e}")).reference
}

/// Whether the selected build is the reference. Tests whose expectations are 01.29 facts reached
/// through default-opening library functions gate on this (run them with `D1_BUILD=01.29`).
pub fn reference_selected() -> bool {
    builds().map(|b| b.selected.id == b.reference.id).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(dir: &Path) -> PathBuf {
        let rig = dir.join("capture-rig");
        std::fs::create_dir_all(&rig).unwrap();
        let entry = |label: &str, valid: bool| {
            format!(
                r#"{{"app_version":null,"build_label":"{label}","title_id":"T","game_dir":"g",
                "packages":"g/packages","eboot":"g/e.elf","eboot_sha256":"aa","eboot_bytes":1,
                "launch":"g/eboot.bin","launch_sha256":"bb","launch_bytes":2,"manifest":"m.bin",
                "manifest_sha256":null,"manifest_bytes":null,"diag_patch":null,
                "addresses_valid":{valid}}}"#
            )
        };
        let text = format!(
            r#"{{"schema":1,"default":"new","reference":"old","builds":{{"old":{},"new":{}}}}}"#,
            entry("1.old", true),
            entry("2.new", false)
        );
        let p = rig.join("builds.json");
        std::fs::write(&p, text).unwrap();
        p
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d1-build-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn default_selection_reference_and_root_relative_paths() {
        let d = scratch("default");
        let b = load(&fixture(&d), None).unwrap();
        assert_eq!(b.selected.id, "new");
        assert!(!b.selected.addresses_valid);
        assert_eq!(b.reference.id, "old");
        assert!(b.reference.addresses_valid);
        assert_eq!(b.selected.eboot_path(), d.join("g/e.elf"));
        assert_eq!(b.selected.manifest_sha256, None);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn explicit_selection_and_unknown_build_is_refused() {
        let d = scratch("explicit");
        let p = fixture(&d);
        assert_eq!(load(&p, Some("old")).unwrap().selected.build_label, "1.old");
        let e = load(&p, Some("01.33")).unwrap_err().to_string();
        assert!(e.contains("unknown build `01.33`") && e.contains("new"), "{e}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_checked_in_registry_parses_and_keeps_the_reference_pin() {
        let b = load(Path::new(DEFAULT_REGISTRY), Some("01.29")).unwrap();
        assert_eq!(
            b.reference.eboot_sha256,
            "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954"
        );
        assert!(b.reference.addresses_valid);
        assert!(b.all.iter().all(|x| x.eboot_sha256.len() == 64 && x.launch_sha256.len() == 64));
    }
}
