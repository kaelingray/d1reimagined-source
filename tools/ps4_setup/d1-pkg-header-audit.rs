// SPDX-License-Identifier: GPL-2.0-or-later
//! Install as a bin in the separate PS4 d1-pkg crate. Reuses its header parser,
//! selection and manifest codec; clips never become an accepted installed estate.
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use d1_pkg::{
    manifest::{self, Policy, Row, Select},
    Header,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Controls,
    /// Verify tables and every stored block for one already recovered package.
    Probe {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        package: String,
    },
    Audit {
        #[arg(long)]
        dir: PathBuf,
        /// Complete or partial independently extracted packages, for prefix agreement.
        #[arg(long)]
        independent: PathBuf,
        #[arg(long, default_value_t = 52553)]
        max_build: u32,
    },
}
#[derive(Deserialize)]
struct Inventory {
    #[serde(rename = "packageCount")]
    count: usize,
    packages: Vec<Item>,
}
#[derive(Deserialize)]
struct Item {
    name: String,
    header_file: String,
    size: u64,
}

fn single_name(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\', ':'])
}
fn checked_header(path: &Path, size: u64) -> Result<Header> {
    let h = Header::read(path)?;
    if h.magic != 0x18 || h.version != 7 {
        bail!("unsupported Bungie header")
    }
    if u64::from(h.declared_size) != size {
        bail!("header size differs from source inventory")
    }
    Ok(h)
}
fn select_era(headers: &[(Header, Row)], max_build: u32) -> Vec<Row> {
    let rows: Vec<_> = headers
        .iter()
        .filter(|(h, _)| h.build <= max_build)
        .map(|(_, r)| r.clone())
        .collect();
    manifest::select(
        &rows,
        &Select {
            policy: Policy::AllGenerations,
            only: None,
            lang: Some(1),
        },
    )
}
fn control_rows() -> Result<Vec<(&'static str, bool)>> {
    let mut raw = [0u8; 0x140];
    raw[..2].copy_from_slice(&0x18u16.to_le_bytes());
    raw[2..4].copy_from_slice(&7u16.to_le_bytes());
    raw[0x18..0x1c].copy_from_slice(&52553u32.to_le_bytes());
    let h = Header::parse(&raw);
    let row = Row {
        path: PathBuf::from("synthetic-control"),
        stem: "control_0001_0".into(),
        id: 1,
        guid: 111,
        timestamp: 1,
        gen: 0,
        locale: 0,
    };
    let mut later_h = h.clone();
    later_h.build = 56578;
    let mut later_r = row.clone();
    later_r.stem = "later_0001_1".into();
    later_r.gen = 1;
    let mut french_r = row.clone();
    french_r.stem = "french_0001_0".into();
    french_r.locale = 2;
    let selected = select_era(
        &[
            (h.clone(), row.clone()),
            (later_h, later_r),
            (h.clone(), french_r),
        ],
        52553,
    );
    let body = manifest::encode(&selected, "")?;
    let decoded = manifest::decode(&body)?;
    let mut mutated = row.clone();
    mutated.guid ^= 1;
    Ok(vec![
        (
            "existing parser reads known build",
            h.build == 52553 && h.magic == 0x18 && h.version == 7,
        ),
        (
            "later build and other locale excluded",
            selected.len() == 1 && selected[0].stem == row.stem,
        ),
        (
            "codec preserves observed GUID",
            decoded.packages.len() == 1 && decoded.packages[0].guid == 111,
        ),
        (
            "inverted GUID changes manifest",
            manifest::encode(&[row], "")? != manifest::encode(&[mutated], "")?,
        ),
        (
            "inventory traversal names rejected",
            !single_name("../game.pkg")
                && !single_name("C:\\game.pkg")
                && single_name("game.pkg.hdr"),
        ),
    ])
}
fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let controls = control_rows()?;
    for (name, pass) in &controls {
        eprintln!("{} {name}", if *pass { "PASS" } else { "FAIL" });
    }
    if controls.iter().any(|(_, p)| !p) {
        return Ok(ExitCode::from(2));
    }
    if let Cmd::Probe { dir, package } = &cli.cmd {
        let estate = d1_pkg::Estate::load(dir)?;
        let pkg = estate.find(package).context("probe package missing")?;
        let tables = pkg.verify_tables();
        if !pkg.layout_ok() || !tables.all_ok() {
            bail!(
                "probe integrity failed: layout_ok={}, tables={tables:?}",
                pkg.layout_ok()
            );
        }
        let mut blocks = 0usize;
        for block in pkg.blocks() {
            estate.read_block_stored(pkg, &block)?;
            blocks += 1;
        }
        if blocks == 0 {
            bail!("probe had no payload blocks")
        }
        println!(
            "{}",
            json!({"package": package, "tables_verified": true,
            "stored_blocks_verified": blocks, "scoped_controls_passed": controls.len()})
        );
        return Ok(ExitCode::SUCCESS);
    }
    let Cmd::Audit {
        dir,
        independent,
        max_build,
    } = cli.cmd
    else {
        return Ok(ExitCode::SUCCESS);
    };
    let bytes = std::fs::read(dir.join("inventory.json"))?;
    // PowerShell UTF-8 BOM is metadata, not part of the JSON object.
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let inventory: Inventory = serde_json::from_slice(bytes)?;
    if inventory.count == 0 || inventory.count != inventory.packages.len() {
        bail!("inventory count gate failed")
    }
    let mut names = BTreeSet::new();
    let mut headers = Vec::new();
    let mut builds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut compared = 0usize;
    for item in inventory.packages {
        if !single_name(&item.name)
            || !single_name(&item.header_file)
            || !names.insert(item.name.clone())
        {
            bail!("unsafe or duplicate inventory name")
        }
        let path = dir.join(&item.header_file);
        let h = checked_header(&path, item.size).with_context(|| item.name.clone())?;
        let clip = std::fs::read(&path)?;
        if clip.len() != 0x140 {
            bail!("not a header-only clip: {}", item.name)
        }
        let reference = independent.join(&item.name);
        if reference.is_file() {
            use std::io::Read;
            let mut other = [0u8; 0x140];
            std::fs::File::open(reference)?.read_exact(&mut other)?;
            if clip != other {
                bail!("independent extractor disagrees: {}", item.name)
            }
            compared += 1;
        }
        *builds.entry(h.build).or_default() += 1;
        let stem = item
            .name
            .strip_suffix(".pkg")
            .context("non-pkg inventory name")?
            .to_owned();
        let row = Row {
            path,
            stem,
            id: h.id,
            guid: h.guid,
            timestamp: h.timestamp,
            gen: h.gen,
            locale: h.locale,
        };
        headers.push((h, row));
    }
    if compared < 2 {
        bail!("need two independent real-package prefix controls in this audit window")
    }
    let selected = select_era(&headers, max_build);
    let body = manifest::encode(&selected, "")?;
    let sha = manifest::sha256_hex(&body);
    let eligible = headers.iter().filter(|(h, _)| h.build <= max_build).count();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "header catalog and hypothetical manifest only",
            "controls_passed": controls.len(), "independent_header_matches": compared,
            "headers": headers.len(), "build_counts": builds, "max_build": max_build,
            "eligible_headers": eligible, "selected_english_headers": selected.len(),
            "hypothetical_manifest_bytes": body.len(), "hypothetical_manifest_sha256": sha,
            "reference_manifest_matches": body.len() == manifest::GOLDEN_FULL_EN.0 && sha == manifest::GOLDEN_FULL_EN.1,
            "reference_estate_count": manifest::PINNED_ESTATE_FILES,
            "descriptor_cap_ok": selected.len() <= manifest::PACKAGE_CAP,
            "full_payloads_verified": false, "executable_rollback": false,
            "eligible": headers.iter().filter(|(h, _)| h.build <= max_build).map(|(_, r)| format!("{}.pkg", r.stem)).collect::<Vec<_>>(),
            "selected": selected.iter().map(|r| &r.stem).collect::<Vec<_>>()
        }))?
    );
    Ok(ExitCode::SUCCESS)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_controls_pass() {
        for (n, p) in control_rows().unwrap() {
            assert!(p, "{n}")
        }
    }
    #[test]
    fn truncated_prefix_refused() {
        let p = std::env::temp_dir().join(format!("d1-header-short-{}.hdr", std::process::id()));
        std::fs::write(&p, [0u8; 3]).unwrap();
        let result = checked_header(&p, 8192);
        std::fs::remove_file(p).unwrap();
        assert!(result.is_err());
    }
}
