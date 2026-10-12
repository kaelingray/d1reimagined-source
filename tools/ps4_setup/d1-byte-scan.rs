// SPDX-License-Identifier: GPL-2.0-or-later
//! Original bounded byte-signature recovery checks. No Sony/PFS/ELF parser.
//! The platform adapter obtains payload extents from the existing PFS reader.
use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
const LATER_BYTES: u64 = 29_276_248;
const LATER_SHA: &str = "ebee5df27aca00057f223b96edab461917e1a7e837b4017b7d6d84b0ae7a2a02";
const MARKERS: [(&str, &[u8]); 5] = [
    ("ELF", b"\x7fELF"),
    ("SELF", b"\x4f\x15\x3d\x1d"),
    ("target-build", b"52553.16.09.28.1551.evil"),
    ("later-build", b"56578.17.04.12.1251.evil"),
    ("build-suffix", b".evil"),
];
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Controls,
    Plan {
        #[arg(long)]
        layout: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1_073_741_824)]
        max_gap_bytes: u64,
    },
    Scan {
        #[arg(long)]
        directory: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    File {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layout {
    logical_bytes: u64,
    files: Vec<Payload>,
    control: Control,
}
#[derive(Deserialize)]
struct Payload {
    start: u64,
    length: u64,
}
#[derive(Deserialize, Serialize)]
struct Control {
    start: u64,
    length: u64,
    sha256: String,
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
struct Region {
    file: String,
    start: u64,
    length: u64,
    role: String,
    sha256: Option<String>,
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
struct Plan {
    logical_bytes: u64,
    payload_bytes_union: u64,
    gap_bytes: u64,
    regions: Vec<Region>,
}
fn gaps(total: u64, files: &[Payload]) -> Result<Vec<(u64, u64)>> {
    ensure!(total > 0 && total < 1 << 46, "invalid logical extent");
    let mut ranges = Vec::new();
    for f in files {
        let end = f
            .start
            .checked_add(f.length)
            .ok_or_else(|| anyhow::anyhow!("extent overflow"))?;
        ensure!(end <= total, "payload outside logical image");
        if f.length > 0 {
            ranges.push((f.start, end));
        }
    }
    ranges.sort_unstable();
    let mut next = 0;
    let mut result = Vec::new();
    for (start, end) in ranges {
        if start > next {
            result.push((next, start - next));
        }
        next = next.max(end);
    }
    if next < total {
        result.push((next, total - next));
    }
    Ok(result)
}
fn create_json(path: &Path, v: &impl Serialize) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(serde_json::to_string_pretty(v)?.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    Ok(())
}
fn make_plan(l: Layout, budget: u64) -> Result<Plan> {
    ensure!(
        l.control.length == LATER_BYTES && l.control.sha256 == LATER_SHA,
        "known executable control is not pinned"
    );
    ensure!(
        l.files
            .iter()
            .any(|f| f.start == l.control.start && f.length == l.control.length),
        "control not a mapped file payload"
    );
    let gaps = gaps(l.logical_bytes, &l.files)?;
    let gap_bytes: u64 = gaps.iter().map(|r| r.1).sum();
    ensure!(
        gap_bytes <= budget && budget <= 4_294_967_296,
        "gap scan exceeds bounded budget"
    );
    let mut regions = vec![Region {
        file: "control.bin".into(),
        start: l.control.start,
        length: l.control.length,
        role: "known-executable-control".into(),
        sha256: Some(l.control.sha256),
    }];
    // Split large gaps for bounded files, preserving actual absolute addresses.
    for (mut start, mut remaining) in gaps {
        while remaining > 0 {
            let length = remaining.min(8_388_608);
            regions.push(Region {
                file: format!("gap-{:06}.bin", regions.len() - 1),
                start,
                length,
                role: "outside-file-payloads".into(),
                sha256: None,
            });
            start += length;
            remaining -= length;
        }
    }
    Ok(Plan {
        logical_bytes: l.logical_bytes,
        payload_bytes_union: l.logical_bytes - gap_bytes,
        gap_bytes,
        regions,
    })
}
fn plan(layout: &Path, out: &Path, budget: u64) -> Result<()> {
    create_json(
        out,
        &make_plan(serde_json::from_slice(&fs::read(layout)?)?, budget)?,
    )
}
#[derive(Debug, Serialize, PartialEq, Eq)]
struct Hit {
    marker: String,
    offset: u64,
}
fn scan_reader(mut reader: impl Read, chunk: usize) -> Result<(u64, String, Vec<Hit>)> {
    use sha2::{Digest, Sha256};
    ensure!(chunk > 0, "zero scan chunk");
    let mut hash = Sha256::new();
    let mut hits = Vec::new();
    let mut tail = Vec::new();
    let mut bytes = vec![0u8; chunk];
    let mut total = 0;
    loop {
        let n = reader.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
        let base = total - tail.len() as u64;
        let mut window = std::mem::take(&mut tail);
        let prior = window.len();
        window.extend_from_slice(&bytes[..n]);
        for (label, marker) in MARKERS {
            for (i, span) in window.windows(marker.len()).enumerate() {
                if span == marker && i + marker.len() > prior {
                    ensure!(hits.len() < 4096, "too many signatures; scan is incomplete");
                    hits.push(Hit {
                        marker: label.into(),
                        offset: base + i as u64,
                    });
                }
            }
        }
        total += n as u64;
        tail = window[window.len().saturating_sub(31)..].to_vec();
    }
    hits.sort_by_key(|h| h.offset);
    Ok((total, format!("{:x}", hash.finalize()), hits))
}
fn controls() -> Result<Vec<(&'static str, bool)>> {
    let bytes = b"x\x7fELF\x4f\x15\x3d\x1d52553.16.09.28.1551.evil!56578.17.04.12.1251.evil!";
    let full = scan_reader(&bytes[..], 128)?;
    let split = scan_reader(&bytes[..], 3)?;
    let empty = scan_reader(&b"no executable here"[..], 2)?;
    let labels = build_labels(std::io::Cursor::new(&bytes[..]), &full.2)?;
    Ok(vec![
        (
            "empty payload map covers complete logical image",
            gaps(10, &[])?.as_slice() == [(0, 10)],
        ),
        (
            "overlapping extents are merged with end padding retained",
            gaps(
                30,
                &[
                    Payload {
                        start: 5,
                        length: 10,
                    },
                    Payload {
                        start: 10,
                        length: 10,
                    },
                ],
            )?
            .as_slice()
                == [(0, 5), (20, 10)],
        ),
        (
            "out-of-bounds and overflowing extents refused",
            gaps(
                10,
                &[Payload {
                    start: 9,
                    length: 2,
                }],
            )
            .is_err()
                && gaps(
                    10,
                    &[Payload {
                        start: u64::MAX,
                        length: 2,
                    }],
                )
                .is_err(),
        ),
        (
            "header/build signatures and both suffixes found at real byte offsets",
            full.2.len() == 6
                && full.2[0].offset == 1
                && full.2[1].offset == 5
                && full.2[2].offset == 9,
        ),
        (
            "chunk-boundary signatures and digest match contiguous control",
            split == full,
        ),
        ("negative region has no signatures", empty.2.is_empty()),
        (
            "truncated marker does not become a match",
            scan_reader(&b"52553.16.09.28.1551.evi"[..], 3)?
                .2
                .is_empty(),
        ),
        (
            "SHA-256 independent known-answer control",
            scan_reader(&b"abc"[..], 1)?.1
                == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "bounded build-label text matches both synthetic identities",
            labels.len() == 2
                && labels[0]["label"] == "52553.16.09.28.1551.evil"
                && labels[1]["label"] == "56578.17.04.12.1251.evil",
        ),
    ])
}

fn build_labels(mut reader: impl Read + Seek, hits: &[Hit]) -> Result<Vec<serde_json::Value>> {
    let mut labels = Vec::new();
    for hit in hits.iter().filter(|h| h.marker == "build-suffix") {
        let Some(start) = hit.offset.checked_sub(19) else {
            continue;
        };
        let mut bytes = [0u8; 24];
        reader.seek(SeekFrom::Start(start))?;
        reader.read_exact(&mut bytes)?;
        let valid = bytes[19..] == *b".evil"
            && (0..19).all(|i| {
                if [5, 8, 11, 14].contains(&i) {
                    bytes[i] == b'.'
                } else {
                    bytes[i].is_ascii_digit()
                }
            });
        if valid {
            labels.push(json!({"offset":start,"label":std::str::from_utf8(&bytes)?}));
        }
    }
    Ok(labels)
}
fn scan_file(input: &Path, out: &Path) -> Result<()> {
    let metadata = fs::metadata(input)?;
    ensure!(
        !input.is_symlink()
            && metadata.is_file()
            && metadata.len() > 0
            && metadata.len() <= 67_108_864,
        "raw input outside bounded file scope"
    );
    let (bytes, sha, hits) = scan_reader(fs::File::open(input)?, 1_048_576)?;
    let labels = build_labels(fs::File::open(input)?, &hits)?;
    create_json(
        out,
        &json!({"scope":"bounded raw bytes and build-label text; no SELF/ELF format or boot acceptance",
        "bytes":bytes,"sha256":sha,"hits":hits,"build_labels":labels,
        "known_later_wrapper_hash":bytes==LATER_BYTES&&sha==LATER_SHA,
        "exact_target_elf_hash":bytes==29_148_264&&sha=="7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954",
        "native_boot_accepted":false}),
    )
}
fn scan(directory: &Path, out: &Path) -> Result<()> {
    let p: Plan = serde_json::from_slice(&fs::read(directory.join("plan.json"))?)?;
    let recomputed = make_plan(
        serde_json::from_slice(&fs::read(directory.join("layout.json"))?)?,
        4_294_967_296,
    )?;
    ensure!(
        p == recomputed,
        "region plan differs from the full extent complement"
    );
    ensure!(
        p.regions.len() > 1 && p.regions[0].role == "known-executable-control",
        "missing same-pass control"
    );
    let mut rows = Vec::new();
    let mut scanned_gap_bytes = 0u64;
    let mut last_gap_end: Option<u64> = None;
    let mut last_gap_tail = Vec::new();
    let mut boundary_hits = Vec::new();
    for r in &p.regions {
        ensure!(
            !r.file.is_empty() && !r.file.contains(['/', '\\', ':']) && r.file.ends_with(".bin"),
            "unsafe region filename"
        );
        let path = directory.join(&r.file);
        ensure!(!path.is_symlink(), "symlink region");
        ensure!(
            fs::metadata(&path)?.len() == r.length && r.length > 0 && r.length <= LATER_BYTES,
            "region length mismatch"
        );
        let (bytes, sha, hits) = scan_reader(fs::File::open(&path)?, 1_048_576)?;
        ensure!(bytes == r.length, "short region");
        if r.role == "known-executable-control" {
            ensure!(
                bytes == LATER_BYTES && sha == LATER_SHA && r.sha256.as_deref() == Some(LATER_SHA),
                "known raw executable digest failed"
            );
            ensure!(
                hits.iter().any(|h| h.marker == "SELF")
                    && hits.iter().any(|h| h.marker == "ELF")
                    && hits.iter().any(|h| h.marker == "later-build"),
                "same-pass signatures failed"
            );
        } else {
            ensure!(r.role == "outside-file-payloads", "unknown region role");
            // Adjacent split gap files need the same cross-boundary proof as read chunks.
            let prefix = fs::read(&path)?;
            if last_gap_end == Some(r.start) {
                let prior = last_gap_tail.len();
                last_gap_tail.extend_from_slice(&prefix[..prefix.len().min(31)]);
                for (label, marker) in MARKERS {
                    for (i, span) in last_gap_tail.windows(marker.len()).enumerate() {
                        if span == marker && i < prior && i + marker.len() > prior {
                            boundary_hits.push(json!({"marker":label,"logical_offset":r.start-prior as u64+i as u64}));
                        }
                    }
                }
            }
            last_gap_tail = prefix[prefix.len().saturating_sub(31)..].to_vec();
            last_gap_end = Some(r.start + r.length);
            scanned_gap_bytes += bytes;
        }
        rows.push(json!({"file":r.file,"start":r.start,"bytes":bytes,"sha256":sha,"role":r.role,"hits":hits}));
    }
    ensure!(scanned_gap_bytes == p.gap_bytes, "gap coverage incomplete");
    create_json(
        out,
        &json!({"scope":"all logical inner-PFS bytes outside listed file payloads, with same-pass known executable control",
        "logical_bytes":p.logical_bytes,"payload_bytes_union":p.payload_bytes_union,"gap_bytes_scanned":scanned_gap_bytes,
        "known_executable_control_passed":true,"regions":rows,"cross_region_hits":boundary_hits,
        "native_boot_accepted":false,"executable_recovered":false}),
    )
}
fn run() -> Result<ExitCode> {
    let rows = controls()?;
    if rows.iter().any(|(_, pass)| !pass) {
        return Ok(ExitCode::from(2));
    }
    match Cli::parse().cmd {
        Cmd::Controls => println!(
            "{}",
            serde_json::to_string_pretty(&json!({"passed":true,"controls":rows}))?
        ),
        Cmd::Plan {
            layout,
            out,
            max_gap_bytes,
        } => plan(&layout, &out, max_gap_bytes)?,
        Cmd::Scan { directory, out } => scan(&directory, &out)?,
        Cmd::File { input, out } => scan_file(&input, &out)?,
    }
    Ok(ExitCode::SUCCESS)
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(2)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_positive_and_negative_controls() {
        for (name, pass) in controls().unwrap() {
            assert!(pass, "{name}")
        }
    }
    #[test]
    fn zero_chunk_rejected() {
        assert!(scan_reader(&b"x"[..], 0).is_err())
    }
    #[test]
    fn build_label_text_is_checked_and_not_a_boot_claim() {
        let bytes = b".evil x 52553.16.09.28.1551.evil xxBAD.16.09.28.1551.evil";
        let (_, _, hits) = scan_reader(&bytes[..], 3).unwrap();
        let labels = build_labels(std::io::Cursor::new(&bytes[..]), &hits).unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0]["label"], "52553.16.09.28.1551.evil");
    }
}
