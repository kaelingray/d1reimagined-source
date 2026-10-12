// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Load a `runs/<stamp>/` directory into a unified [`Trace`].
//!
//! Sources and how they are read (historicaldocs/design/trace-viewer.md §1, §2):
//!   - probe records  → decoded natively by [`crate::probe_decode`] (the Rust port of
//!     `capture-rig/decode-account-trace.py`; no Python at runtime)
//!   - blob sidecar   → decoded natively by [`crate::blob_decode`] (the Rust port of
//!     `capture-rig/world-launch-command.py`; no Python at runtime)
//!   - HTTP + BAP     → scrape `responder.log` (the responder ALREADY decoded these; we read its text)
//!   - guest diag     → scrape `emulator.log` `[stdout]` lines
//!
//! Ordering: the probe `sequence` is the spine. BAP frames anchor onto it by (msg_type, sequence)
//! against the guest's own rx probes; responder events *between two anchors* are bracketed; anything
//! outside the outermost anchor is extrapolated and labelled approximate. Every event records its
//! [`Confidence`].
//!
//! ⚠ Three failure modes this module is now explicitly built to refuse (see §7 of the design doc):
//!   - **never claim an order we did not observe** - no anchors ⇒ no cross-stream timeline at all;
//!   - **never lose a lane silently** - every read error becomes a note, not an empty vector;
//!   - **never let a cosmetic reword zero the anchors** - `d1-trace controls` pins that invariant.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::model::{BapDir, BapInfo, Confidence, Event, ProbeRecord, RunSummary, Stream, Trace};
use crate::probe_decode;

/// Strip ANSI SGR escapes (`\x1b[…m`). `responder.log`/`emulator.log` are colourised.
///
/// ⚠ F5: this used to walk `line.as_bytes()` and `push(b as char)`, which is a Latin-1 decode of
/// UTF-8 - it turned `⭐` into three mojibake chars, so `is_detail_line`'s `"⭐"` arm could never
/// fire and the responder's highest-signal annotations were unreachable. Iterate `chars()`.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // Consume up to and including the final byte (an ASCII letter, typically 'm').
                while let Some(c2) = chars.next() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            // A bare ESC (not CSI) is dropped; the next char is processed normally.
        } else {
            out.push(c);
        }
    }
    out
}

/// Read a log file as text WITHOUT ever discarding it for encoding reasons.
///
/// ⚠ F3: `read_to_string(...)` with `Err(_) => Ok(Vec::new())` meant that a single stray `0xff`
/// anywhere in a 231 MB `emulator.log` zeroed the guest lane and exited 0 - a silent lane loss that
/// reads exactly like "the game never got there". A byte-range snapshot of a live run truncating
/// mid-multibyte-char reaches this directly. Lossy-decode instead, and report any real read error.
fn read_log_lossy(path: &Path, notes: &mut Vec<String>) -> Option<String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            if matches!(text, std::borrow::Cow::Owned(_)) {
                notes.push(format!(
                    "⚠ {} is not valid UTF-8 - decoded lossily (invalid bytes shown as U+FFFD); \
                     no lines were dropped",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Some(text.into_owned())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            notes.push(format!(
                "no {} in this run - that lane is empty",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            None
        }
        Err(e) => {
            notes.push(format!(
                "⚠ could not read {}: {e} - that lane is EMPTY BECAUSE OF AN ERROR, not because \
                 the run produced nothing",
                path.display()
            ));
            None
        }
    }
}

/// The ANSI stripper, exposed so `d1-trace controls` can pin the F5 invariant at its defect site.
pub fn strip_ansi_for_controls(line: &str) -> String {
    strip_ansi(line)
}

/// Decode the probe records natively via [`crate::probe_decode`] - the Rust port of
/// `capture-rig/decode-account-trace.py`, so the shipped binary needs no Python at runtime.
///
/// ⚠ F8: a decoder *failure* is the same graceful path as a *missing* trace. 18 of 133 run dirs
/// carry a pre-232-byte-era `account-trace.bin`; refusing here made `load_run` refuse the whole
/// directory, hiding a perfectly readable `responder.log` and `emulator.log` behind an unrelated
/// lane's error. The decoder's refusals (bad magic/version/size/length) become a note instead.
fn load_probes(run_dir: &Path) -> (Vec<ProbeRecord>, Vec<String>) {
    let mut notes = Vec::new();
    let trace = run_dir.join("account-trace.bin");
    if !trace.is_file() {
        notes.push("no account-trace.bin - probe lane empty".to_string());
        return (Vec::new(), notes);
    }
    let blob = match std::fs::read(&trace) {
        Ok(b) => b,
        Err(e) => {
            notes.push(format!(
                "⚠ probe lane EMPTY BECAUSE OF AN ERROR: reading {}: {e}",
                trace.display()
            ));
            return (Vec::new(), notes);
        }
    };
    let records = match probe_decode::read_records(&blob, &trace.display().to_string()) {
        Ok(r) => r,
        Err(e) => {
            notes.push(format!(
                "⚠ probe lane EMPTY BECAUSE OF AN ERROR: probe decoder refused the trace: {e}"
            ));
            return (Vec::new(), notes);
        }
    };
    notes.push(format!("probe diagnosis: {}", probe_decode::diagnosis(&records)));
    let probes = records
        .iter()
        .map(|r| {
            let mut registers = std::collections::BTreeMap::new();
            for (name, value) in probe_decode::REGISTER_NAMES.iter().zip(r.regs.iter()) {
                registers.insert(name.to_string(), *value);
            }
            ProbeRecord {
                sequence: r.sequence as u64,
                probe_id: r.probe_id as u32,
                probe: r.probe.clone(),
                registers,
                sample_a: hex::encode_plain(&r.sample_a),
                sample_b: hex::encode_plain(&r.sample_b),
                explain: probe_decode::explain(r),
            }
        })
        .collect();
    (probes, notes)
}

/// Tiny local hex helper (no external crate; `probe_decode` keeps its own for the JSON path).
mod hex {
    pub fn encode_plain(d: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut s = String::with_capacity(d.len() * 2);
        for b in d {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

/// Summarise the oversized-capture blob (world_launch commands), if present and non-empty.
///
/// Decoded natively by [`crate::blob_decode`] - the Rust port of
/// `capture-rig/world-launch-command.py`, which this function used to SPAWN at runtime (the last
/// Python subprocess in the ingest path). A refused sidecar becomes a loud note, not a silently
/// missing one: the old shell-out turned any script failure into `None`.
fn load_blob_summary(run_dir: &Path) -> Option<String> {
    let path = run_dir.join("account-trace.bin.blob");
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() == 0 {
        return Some("account-trace.bin.blob is empty (no world_launch commands captured)".into());
    }
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            return Some(format!(
                "⚠ blob lane EMPTY BECAUSE OF AN ERROR: reading {}: {e}",
                path.display()
            ))
        }
    };
    match crate::blob_decode::read_blobs(&data) {
        Ok(blobs) => {
            let text = crate::blob_decode::summarise(&blobs);
            Some(text.lines().take(24).collect::<Vec<_>>().join("\n"))
        }
        Err(e) => Some(format!(
            "⚠ blob sidecar refused (its lane is empty because of an ERROR, not because the run \
             captured nothing): {e}"
        )),
    }
}

/// Where an event's timeline position came from. Distinguishing *between two anchors* from
/// *extrapolated past the outermost anchor* is the whole of F1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// Pinned to the spine by a real join key.
    Anchored,
    /// Strictly between two anchored neighbours in the same file - true bracketing.
    Bracketed,
    /// Before the first anchor, after the last, or no anchors at all: EXTRAPOLATED.
    Outside,
}

/// A responder-log event before global ordering (BAP or HTTP), with its own file order.
struct RespEvent {
    stream: Stream,
    line_no: u64,
    summary: String,
    detail: Vec<String>,
    bap: Option<BapInfo>,
    /// Exact order if anchored; filled during ordering.
    order: Option<f64>,
    placement: Placement,
}

impl RespEvent {
    fn new(stream: Stream, line_no: u64, summary: String, detail: Vec<String>) -> Self {
        RespEvent {
            stream,
            line_no,
            summary,
            detail,
            bap: None,
            order: None,
            placement: Placement::Outside,
        }
    }
}

/// The BAP outer header line, buffered until the inner-decode line it belongs to arrives (F6).
struct PendingFrame {
    line_no: u64,
    lines: Vec<String>,
}

fn flush_pending(pending: &mut Option<PendingFrame>, events: &mut Vec<RespEvent>) {
    if let Some(p) = pending.take() {
        // An outer header with no inner decode after it: keep it, but as its own (unanchorable) row
        // rather than losing it.
        let summary = p.lines.first().cloned().unwrap_or_default();
        let mut ev = RespEvent::new(Stream::Bap, p.line_no, summary, p.lines);
        ev.bap = Some(BapInfo {
            direction: BapDir::Recv,
            ..Default::default()
        });
        events.push(ev);
    }
}

/// Parse `responder.log` into HTTP and BAP events, attaching indented header/hex lines as detail.
fn load_responder(run_dir: &Path, notes: &mut Vec<String>) -> Vec<RespEvent> {
    let path = run_dir.join("responder.log");
    let Some(raw) = read_log_lossy(&path, notes) else {
        return Vec::new();
    };
    let mut events: Vec<RespEvent> = Vec::new();
    // The outer `BAP TCP msg#N` header + its raw hex, waiting for the inner decode (F6).
    let mut pending: Option<PendingFrame> = None;
    // Index of the BAP inner-decode event for the frame currently being described. Reply-identity
    // lines and the `--> BAP …` send row attach HERE, not to `events.last()`, because generic
    // annotation rows (`INVESTMENT message id 503`, `503 RAW BODY`, …) can sit in between.
    let mut cur_frame: Option<usize> = None;
    let mut dropped = 0usize;

    for (idx, raw_line) in raw.lines().enumerate() {
        let line_no = idx as u64;
        let clean = strip_ansi(raw_line);
        // Drop the leading level token (INFO/WARN/…) and note its indentation.
        let body = strip_level(&clean);
        let trimmed = body.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let text = clean.trim().to_string();

        if trimmed.starts_with("BAP TCP msg#") {
            // F6: NOT an event of its own - it is the outer header of the inner decode that follows.
            flush_pending(&mut pending, &mut events);
            pending = Some(PendingFrame {
                line_no,
                lines: vec![text],
            });
            cur_frame = None;
        } else if trimmed.starts_with("--> BAP") {
            // F10: the server→client half of the conversation. It was swallowed as detail, so every
            // BAP row on the timeline was labelled `recv` and `BapDir::Send` was never constructed.
            // The frame's reply identity (from its `-> … (0x00NN), seq N` line) is this frame's id.
            flush_pending(&mut pending, &mut events);
            let (rt, rs) = cur_frame
                .and_then(|i| events[i].bap.as_ref())
                .map(|b| (b.reply_type, b.reply_seq))
                .unwrap_or((None, None));
            let info = BapInfo {
                msg_type: rt,
                sequence: rs,
                reply_type: None,
                reply_seq: None,
                direction: BapDir::Send,
            };
            let mut ev = RespEvent::new(Stream::Bap, line_no, bap_summary(&info, trimmed), vec![text]);
            ev.bap = Some(info);
            events.push(ev);
        } else if trimmed.starts_with("--> ") {
            // HTTP request. e.g. "--> POST host/path 187B"
            flush_pending(&mut pending, &mut events);
            cur_frame = None;
            events.push(RespEvent::new(
                Stream::Http,
                line_no,
                format!("HTTP {}", trimmed.trim_start_matches("--> ")),
                vec![text],
            ));
        } else if trimmed.starts_with("<-- ") {
            // HTTP response.
            flush_pending(&mut pending, &mut events);
            cur_frame = None;
            events.push(RespEvent::new(
                Stream::Http,
                line_no,
                format!("HTTP {}", trimmed.trim_start_matches("<-- ")),
                vec![text],
            ));
        } else if let Some(info) = parse_bap_line(trimmed) {
            // The inner decode. Absorb the buffered outer header + raw hex as its detail (F6) and
            // inherit the header's line number so the frame sits at its ARRIVAL position.
            let (ordinal, mut detail) = match pending.take() {
                Some(p) => (p.line_no, p.lines),
                None => (line_no, Vec::new()),
            };
            detail.push(text);
            let mut ev = RespEvent::new(Stream::Bap, ordinal, bap_summary(&info, trimmed), detail);
            ev.bap = Some(info);
            events.push(ev);
            cur_frame = Some(events.len() - 1);
        } else if trimmed.starts_with("-> ") {
            // The responder's reply identity for the frame in flight.
            if let Some(i) = cur_frame {
                events[i].detail.push(text);
                if let Some((t, s)) = parse_reply_ids(trimmed) {
                    if let Some(bap) = events[i].bap.as_mut() {
                        bap.reply_type = Some(t);
                        bap.reply_seq = s;
                    }
                }
            } else if let Some(last) = events.last_mut() {
                last.detail.push(text);
            } else {
                dropped += 1;
            }
        } else if is_detail_line(trimmed) {
            // Indented continuation (header, hex body, envelope). Attach to the last event.
            if let Some(last) = events.last_mut() {
                last.detail.push(text);
            } else {
                dropped += 1;
            }
        } else if is_hex_dump_line(trimmed) {
            // Raw frame bytes: detail of the frame they belong to, never a timeline row.
            if let Some(p) = pending.as_mut() {
                p.lines.push(text);
            } else if let Some(i) = cur_frame {
                events[i].detail.push(text);
            } else if let Some(last) = events.last_mut() {
                last.detail.push(text);
            } else {
                dropped += 1;
            }
        } else {
            // F7: everything else used to be DROPPED ENTIRELY - 126 lines in the reference run,
            // including `INVESTMENT message id 503`, `503 RAW BODY`, `QUEUEZ …`, `push plaintext`,
            // the ⭐ annotations, and the `success response: … bap=… manifest_url=…` line that names
            // the BAP endpoint. They are annotations about the surrounding traffic, so they become
            // generic rows in the lane they annotate.
            let stream = if pending.is_some() {
                Stream::Bap
            } else {
                events.last().map(|e| e.stream).unwrap_or(Stream::Http)
            };
            let mut summary: String = trimmed.chars().take(400).collect();
            if trimmed.chars().count() > 400 {
                summary.push('…');
            }
            events.push(RespEvent::new(stream, line_no, summary, vec![text]));
        }
    }
    flush_pending(&mut pending, &mut events);
    if dropped > 0 {
        notes.push(format!(
            "⚠ {dropped} responder.log continuation line(s) had no event to attach to and were \
             dropped"
        ));
    }
    events
}

fn strip_level(line: &str) -> String {
    let t = line.trim_start();
    for lvl in ["INFO", "WARN", "ERROR", "DEBUG", "TRACE"] {
        if let Some(rest) = t.strip_prefix(lvl) {
            // Preserve the following indentation (it distinguishes events from continuations).
            return rest.strip_prefix(' ').unwrap_or(rest).to_string();
        }
    }
    line.to_string()
}

fn is_detail_line(trimmed: &str) -> bool {
    trimmed.starts_with("authorization")
        || trimmed.starts_with("user-agent")
        || trimmed.starts_with("content-type")
        || trimmed.starts_with("accept")
        || trimmed.starts_with("host:")
        || trimmed.starts_with("body:")
        || trimmed.starts_with("hex:")
        || trimmed.starts_with("envelope")
        || trimmed.starts_with("-> ")
        || trimmed.starts_with("--> BAP")
}

/// A raw frame dump: nothing but 2-digit hex bytes (optionally truncated with `… (+N more)`).
fn is_hex_dump_line(trimmed: &str) -> bool {
    let head = trimmed.split('…').next().unwrap_or(trimmed).trim();
    if head.is_empty() {
        return false;
    }
    let mut n = 0usize;
    for tok in head.split_whitespace() {
        if tok.len() == 2 && tok.bytes().all(|b| b.is_ascii_hexdigit()) {
            n += 1;
        } else {
            return false;
        }
    }
    n >= 8
}

/// Recognise a BAP inner-decode line and pull `(msg_type, sequence)`.
///   "SECURED (6B): 00 79 … -> Ok(Inner { msg_type: 121, sequence: 2 })"
///   "envelope v1 mode 2 payload 134 | inner type 0x001e seq 0"
///
/// ⚠ `BAP TCP msg#N` is deliberately NOT handled here (F6) - it is an outer header with no
/// type/seq, folded into the inner decode by the caller.
fn parse_bap_line(trimmed: &str) -> Option<BapInfo> {
    if trimmed.starts_with("SECURED") {
        let mut info = BapInfo {
            direction: BapDir::Recv,
            ..Default::default()
        };
        if let Some(t) = extract_after(trimmed, "msg_type:") {
            info.msg_type = parse_num(t).map(|v| v as u16);
        }
        if let Some(s) = extract_after(trimmed, "sequence:") {
            info.sequence = parse_num(s).map(|v| v as u32);
        }
        return Some(info);
    }
    if trimmed.starts_with("envelope") {
        let mut info = BapInfo {
            direction: BapDir::Recv,
            ..Default::default()
        };
        if let Some(t) = extract_after(trimmed, "inner type") {
            info.msg_type = parse_num(t).map(|v| v as u16);
        }
        if let Some(s) = extract_after(trimmed, "seq") {
            info.sequence = parse_num(s).map(|v| v as u32);
        }
        return Some(info);
    }
    None
}

fn parse_reply_ids(trimmed: &str) -> Option<(u16, Option<u32>)> {
    // "-> start response (0x001f), seq 0"  or "-> FAMILY SUBSCRIBE ACK 0x000D () … seq 7"
    let ty = extract_after(trimmed, "(0x")
        .and_then(|s| u16::from_str_radix(s.split(')').next()?.trim(), 16).ok())
        .or_else(|| {
            extract_after(trimmed, "ACK 0x")
                .and_then(|s| u16::from_str_radix(s.split_whitespace().next()?, 16).ok())
        })?;
    let seq = extract_after(trimmed, "seq").and_then(parse_num).map(|v| v as u32);
    Some((ty, seq))
}

fn bap_summary(info: &BapInfo, trimmed: &str) -> String {
    let dir = match info.direction {
        BapDir::Recv => "recv",
        BapDir::Send => "send",
    };
    match (info.msg_type, info.sequence) {
        (Some(t), Some(s)) => format!("BAP {dir} type=0x{t:04x} seq={s}"),
        (Some(t), None) => format!("BAP {dir} type=0x{t:04x}"),
        // F6: do not re-prefix a line that already begins with "BAP" ("BAP BAP TCP…").
        _ => {
            let head: String = trimmed.chars().take(70).collect();
            if head.starts_with("BAP") {
                format!("{head} [{dir}]")
            } else {
                format!("BAP {dir} {head}")
            }
        }
    }
}

fn extract_after<'a>(hay: &'a str, needle: &str) -> Option<&'a str> {
    let pos = hay.find(needle)? + needle.len();
    Some(hay[pos..].trim_start())
}

fn parse_num(s: &str) -> Option<u64> {
    let s = s.trim();
    let tok: String = s
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == 'x' || *c == 'X')
        .collect();
    if let Some(hex) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if tok.chars().all(|c| c.is_ascii_digit()) && !tok.is_empty() {
        tok.parse().ok()
    } else {
        // token like "001e" from "inner type 0x001e" already stripped of 0x by the loop above?
        u64::from_str_radix(&tok, 16).ok()
    }
}

/// A guest-diagnostic line before ordering.
struct GuestEvent {
    line_no: u64,
    summary: String,
    is_error: bool,
    bootflow: Option<String>,
}

/// Parse `emulator.log` `[stdout]` guest lines. Renderer/loader spam is skipped.
fn load_guest(run_dir: &Path, notes: &mut Vec<String>) -> Vec<GuestEvent> {
    let path = run_dir.join("emulator.log");
    let Some(raw) = read_log_lossy(&path, notes) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for (idx, raw_line) in raw.lines().enumerate() {
        let clean = strip_ansi(raw_line);
        let Some(pos) = clean.find("log_flush: [stdout] ") else {
            continue;
        };
        let body = clean[pos + "log_flush: [stdout] ".len()..].trim().to_string();
        if body.is_empty() {
            continue;
        }
        let is_error = body.contains("error.");
        let bootflow = body
            .find("bootflow:")
            .map(|p| body[p..].split('\'').next().unwrap_or("").to_string());
        events.push(GuestEvent {
            line_no: idx as u64,
            summary: body,
            is_error,
            bootflow,
        });
    }
    events
}

/// The `explain()` labels this module scrapes to build the anchor map.
///
/// ⚠ F4, structurally closed: anchoring used to be scraped from human prose printed by
/// `capture-rig/decode-account-trace.py`, where a cosmetic reword (it happened twice in one day)
/// took 1061 anchors → 0 **silently**, with every unit test still green. The prose producer is now
/// [`crate::probe_decode::explain`] in this same crate, and it emits these labels FROM the shared
/// constants below - producer and consumer can no longer drift. The `C-ANCHOR-*` controls remain
/// as belt and braces.
pub use crate::probe_decode::{ANCHOR_SEQ_LABELS, ANCHOR_TYPE_LABELS, ANCHOR_WITNESS_PROBE};

/// Build the anchor map from probe records: (msg_type, sequence) → probe `sequence` on the spine.
/// Uses the guest's own decoded rx/decrypt probes, read from the tool's `explain` text so nothing
/// is re-interpreted here.
fn build_bap_anchors(probes: &[ProbeRecord]) -> HashMap<(u16, u32), f64> {
    let mut map = HashMap::new();
    for p in probes {
        for line in &p.explain {
            // "received_type=0x001f received_sequence=0" or "inner_type=0x007a inner_sequence=2"
            let ty = ANCHOR_TYPE_LABELS
                .iter()
                .find_map(|l| extract_hex_after(line, l));
            let seq = ANCHOR_SEQ_LABELS
                .iter()
                .find_map(|l| extract_dec_after(line, l));
            if let (Some(t), Some(s)) = (ty, seq) {
                // First writer wins: the earliest probe sequence that saw this (type, seq).
                map.entry((t as u16, s as u32)).or_insert(p.sequence as f64);
            }
        }
    }
    map
}

fn extract_hex_after(hay: &str, needle: &str) -> Option<u64> {
    let rest = extract_after(hay, needle)?;
    let tok: String = rest
        .trim_start_matches("0x")
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    u64::from_str_radix(&tok, 16).ok()
}

fn extract_dec_after(hay: &str, needle: &str) -> Option<u64> {
    let rest = extract_after(hay, needle)?;
    let tok: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    tok.parse().ok()
}

/// Does this BAP row carry an id at all? Rows that do not can never anchor, so they must not sit in
/// the anchor-rate denominator (F6).
fn is_anchorable(info: &BapInfo) -> bool {
    (info.reply_type.is_some() && info.reply_seq.is_some())
        || (info.msg_type.is_some() && info.sequence.is_some())
}

/// Load an entire run directory into an ordered [`Trace`].
pub fn load_run(run_dir: &Path) -> Result<Trace> {
    if !run_dir.is_dir() {
        return Err(anyhow!("{} is not a directory", run_dir.display()));
    }
    let (probes, mut notes) = load_probes(run_dir);
    let mut resp = load_responder(run_dir, &mut notes);
    let guests = load_guest(run_dir, &mut notes);

    if let Ok(head) = std::fs::read_to_string(run_dir.join("RUN-INFO.txt")) {
        notes.push(format!(
            "RUN-INFO: {}",
            head.lines().take(6).collect::<Vec<_>>().join(" · ")
        ));
    }
    if let Some(blob) = load_blob_summary(run_dir) {
        notes.push(blob);
    }

    // ---- Ordering ---------------------------------------------------------------------------
    let spine_max = probes.last().map(|p| p.sequence as f64).unwrap_or(0.0);
    let spine_min = 0.0;
    let anchors = build_bap_anchors(&probes);
    let mut bap_anchored = 0usize;

    // ⚠ F2: a unified cross-stream timeline requires BOTH a spine and at least one join onto it.
    // 57 of 133 run dirs have a `responder.log` and NO `account-trace.bin`; those collapsed to a
    // single order value (0.0) and then fell through to `stream_priority`, so every guest line
    // sorted before every HTTP line before every BAP line - pure fiction, presented as `~`/`≈`.
    let spine = !probes.is_empty() && !anchors.is_empty();
    if !spine {
        let reason = if probes.is_empty() {
            "no probe records (account-trace.bin absent or undecodable)"
        } else {
            "probe records exist but none carried a (type, seq) anchor"
        };
        notes.push(format!(
            "⚠⚠ NO CROSS-STREAM SPINE - {reason}. Cross-stream order is NOT OBSERVED and has not \
             been invented: rows are grouped by lane and ordered within lane by their own native \
             key only. Every row is marked ≈; do not cite any HTTP↔BAP↔guest ordering from this run."
        ));
    }

    // Pass 1: anchor BAP frames onto the spine where a probe saw the same (type, seq). The guest's
    // rx probes record the RESPONSE, so try the reply key first, then the request key.
    if spine {
        for ev in resp.iter_mut() {
            if ev.stream == Stream::Bap {
                if let Some(info) = &ev.bap {
                    if let Some(&order) = anchor_lookup(&anchors, info) {
                        ev.order = Some(order);
                        ev.placement = Placement::Anchored;
                        bap_anchored += 1;
                    }
                }
            }
        }

        // Pass 2: place the responder events that are still unplaced, in file order. Only those
        // strictly BETWEEN two anchors are `Bracketed`; the head and tail are EXTRAPOLATED (F1).
        interpolate(&mut resp, spine_min, spine_max);
    }

    let bap_anchorable = resp
        .iter()
        .filter(|e| e.stream == Stream::Bap)
        .filter(|e| e.bap.as_ref().is_some_and(is_anchorable))
        .count();

    // ---- Assemble unified events ------------------------------------------------------------
    let mut events: Vec<Event> = Vec::new();

    for p in &probes {
        let summary = p
            .explain
            .first()
            .cloned()
            .unwrap_or_else(|| format!("#{} {}", p.sequence, p.probe));
        events.push(Event {
            id: 0,
            stream: Stream::Probe,
            order: p.sequence as f64,
            confidence: Confidence::Exact,
            ordinal: p.sequence,
            summary,
            detail: p.explain.clone(),
            probe: Some(p.clone()),
            bap: None,
        });
    }

    for ev in resp {
        let confidence = match ev.placement {
            Placement::Anchored => Confidence::Exact,
            Placement::Bracketed => Confidence::Bracketed,
            Placement::Outside => Confidence::Approx,
        };
        events.push(Event {
            id: 0,
            stream: ev.stream,
            order: ev.order.unwrap_or(spine_min),
            confidence,
            ordinal: ev.line_no,
            summary: ev.summary,
            detail: ev.detail,
            probe: None,
            bap: ev.bap,
        });
    }

    // Guest diagnostics: linear map of line position onto the spine range (no numeric join).
    let (gmin, gmax) = guests
        .iter()
        .fold((u64::MAX, 0u64), |(lo, hi), g| (lo.min(g.line_no), hi.max(g.line_no)));
    let gspan = (gmax.saturating_sub(gmin)).max(1) as f64;
    let mut bootflow_reached = None;
    let mut error_count = 0usize;
    for g in &guests {
        if g.is_error {
            error_count += 1;
        }
        if let Some(bf) = &g.bootflow {
            bootflow_reached = Some(bf.clone());
        }
        let frac = (g.line_no.saturating_sub(gmin)) as f64 / gspan;
        events.push(Event {
            id: 0,
            stream: Stream::Guest,
            order: spine_min + frac * (spine_max - spine_min),
            confidence: Confidence::Approx,
            ordinal: g.line_no,
            summary: g.summary.clone(),
            detail: vec![g.summary.clone()],
            probe: None,
            bap: None,
        });
    }

    if spine {
        // ---- Global sort: by order, then a stable stream priority so ties read sensibly --------
        events.sort_by(|a, b| {
            a.order
                .partial_cmp(&b.order)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(stream_priority(a.stream).cmp(&stream_priority(b.stream)))
                .then(a.ordinal.cmp(&b.ordinal))
        });
    } else {
        // F2: lane mode. Order is the lane's own native key and nothing else; lanes are presented
        // one after another so no cross-lane adjacency can be read off the screen as evidence.
        for ev in events.iter_mut() {
            ev.order = ev.ordinal as f64;
            ev.confidence = Confidence::Approx;
        }
        events.sort_by(|a, b| {
            stream_priority(a.stream)
                .cmp(&stream_priority(b.stream))
                .then(a.ordinal.cmp(&b.ordinal))
        });
    }
    for (i, ev) in events.iter_mut().enumerate() {
        ev.id = i;
    }

    let mut probe_names: Vec<String> = probes.iter().map(|p| p.probe.clone()).collect();
    probe_names.sort();
    probe_names.dedup();

    let summary = RunSummary {
        run_id: run_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        http: events.iter().filter(|e| e.stream == Stream::Http).count(),
        bap: events.iter().filter(|e| e.stream == Stream::Bap).count(),
        probe: probes.len(),
        guest: guests.len(),
        probe_names,
        bootflow_reached,
        error_count,
        bap_anchored,
        bap_anchorable,
        spine,
        notes,
    };

    Ok(Trace { events, summary })
}

/// Look a BAP frame up in the anchor map: reply key (response the guest received) first, then the
/// request key as a fallback.
fn anchor_lookup<'a>(
    anchors: &'a HashMap<(u16, u32), f64>,
    info: &BapInfo,
) -> Option<&'a f64> {
    if let (Some(t), Some(s)) = (info.reply_type, info.reply_seq) {
        if let Some(order) = anchors.get(&(t, s)) {
            return Some(order);
        }
    }
    if let (Some(t), Some(s)) = (info.msg_type, info.sequence) {
        return anchors.get(&(t, s));
    }
    None
}

fn stream_priority(s: Stream) -> u8 {
    match s {
        Stream::Guest => 0,
        Stream::Http => 1,
        Stream::Bap => 2,
        Stream::Probe => 3,
    }
}

/// Fill `order` for responder events that lack it, by linear interpolation in file order between
/// the nearest events that DO have one.
///
/// ⚠ F1: the head (before the first anchor) and the tail (after the last) are **extrapolations**,
/// not brackets - nothing constrains them from one side. They are marked [`Placement::Outside`] so
/// the caller labels them `≈`, not `~`. Getting this wrong put `HTTP ~ 4.90 POST /SignOn` *before*
/// `PROBE = 5.00 bap_request_matched`, when the SignOn response is what delivers the BAP address.
fn interpolate(resp: &mut [RespEvent], lo: f64, hi: f64) {
    let n = resp.len();
    if n == 0 {
        return;
    }
    // indices of anchored events
    let anchored: Vec<usize> = (0..n).filter(|&i| resp[i].order.is_some()).collect();
    if anchored.is_empty() {
        // No anchors at all: spread uniformly across the spine. NOTHING here is bracketed.
        for (i, ev) in resp.iter_mut().enumerate() {
            ev.order = Some(lo + (hi - lo) * (i as f64 / (n.max(1) - 1).max(1) as f64));
            ev.placement = Placement::Outside;
        }
        return;
    }
    // Before the first anchor - EXTRAPOLATED.
    let first = anchored[0];
    let first_order = resp[first].order.unwrap();
    for i in 0..first {
        let frac = if first == 0 { 0.0 } else { i as f64 / first as f64 };
        resp[i].order = Some(lo + (first_order - lo) * frac);
        resp[i].placement = Placement::Outside;
    }
    // Between consecutive anchors - TRUE BRACKETING.
    for w in anchored.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (oa, ob) = (resp[a].order.unwrap(), resp[b].order.unwrap());
        for i in (a + 1)..b {
            let frac = (i - a) as f64 / (b - a) as f64;
            resp[i].order = Some(oa + (ob - oa) * frac);
            resp[i].placement = Placement::Bracketed;
        }
    }
    // After the last anchor - EXTRAPOLATED.
    let last = *anchored.last().unwrap();
    let last_order = resp[last].order.unwrap();
    let tail = n - 1 - last;
    for (k, i) in ((last + 1)..n).enumerate() {
        let frac = if tail == 0 { 0.0 } else { (k + 1) as f64 / (tail + 1) as f64 };
        resp[i].order = Some(last_order + (hi - last_order) * frac);
        resp[i].placement = Placement::Outside;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(stream: Stream, line_no: u64, order: Option<f64>) -> RespEvent {
        let mut e = RespEvent::new(stream, line_no, String::new(), vec![]);
        e.order = order;
        if order.is_some() {
            e.placement = Placement::Anchored;
        }
        e
    }

    #[test]
    fn ansi_stripping() {
        assert_eq!(strip_ansi("\x1b[32m INFO\x1b[0m hello"), " INFO hello");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    /// F5 regression: the old byte-wise `push(b as char)` Latin-1-decoded UTF-8, so `⭐` became
    /// mojibake and the responder's highest-signal annotations were unreachable - and every em-dash
    /// in the HTTP lane rendered as `â`.
    #[test]
    fn ansi_stripping_preserves_utf8() {
        let src = "\x1b[33m      ⭐ FAMILY 4 ANSWER seq 0 root 0xd1 - answering the SUBSCRIPTION\x1b[0m";
        let got = strip_ansi(src);
        assert_eq!(
            got,
            "      ⭐ FAMILY 4 ANSWER seq 0 root 0xd1 - answering the SUBSCRIPTION"
        );
        assert!(got.contains('⭐'), "star must survive: {got:?}");
        assert!(got.contains('-'), "em dash must survive: {got:?}");
    }

    #[test]
    fn level_and_detail() {
        assert_eq!(strip_level("INFO --> GET x"), "--> GET x");
        assert_eq!(strip_level("WARN   SECURED"), "  SECURED");
        assert!(is_detail_line("authorization: Bearer x"));
        assert!(is_detail_line("-> start response (0x001f), seq 0"));
    }

    #[test]
    fn hex_dump_recognition() {
        assert!(is_hex_dump_line("01 02 00 00 00 86 00 1e 00 00"));
        assert!(is_hex_dump_line("01 02 00 00 00 86 00 1e 00 00 … (+46 more)"));
        assert!(!is_hex_dump_line("SECURED (6B): 00 79 00 00 00 02"));
        assert!(!is_hex_dump_line("00 01 02"));
        assert!(!is_hex_dump_line("INVESTMENT message id 503 (47 body bytes)"));
    }

    #[test]
    fn bap_secured_ids() {
        let info =
            parse_bap_line("SECURED (6B): 00 79 -> Ok(Inner { msg_type: 121, sequence: 2 })")
                .unwrap();
        assert_eq!(info.msg_type, Some(121));
        assert_eq!(info.sequence, Some(2));
        assert_eq!(info.direction, BapDir::Recv);
    }

    #[test]
    fn bap_envelope_ids() {
        let info = parse_bap_line("envelope v1 mode 2 payload 134 | inner type 0x001e seq 0")
            .unwrap();
        assert_eq!(info.msg_type, Some(0x1e));
        assert_eq!(info.sequence, Some(0));
    }

    /// F6: the outer header is not an event on its own, and never gets a "BAP BAP …" summary.
    #[test]
    fn outer_header_is_not_an_inner_decode() {
        assert!(parse_bap_line("BAP TCP msg#1 140B [type 0102 len 134 (+6 = 140)]").is_none());
        let info = BapInfo::default();
        let s = bap_summary(&info, "BAP TCP msg#1 140B");
        assert!(!s.contains("BAP BAP"), "double prefix: {s}");
    }

    #[test]
    fn reply_ids() {
        assert_eq!(parse_reply_ids("-> start response (0x001f), seq 0"), Some((0x1f, Some(0))));
    }

    #[test]
    fn anchor_extraction() {
        let p = ProbeRecord {
            sequence: 9,
            probe_id: 32,
            probe: ANCHOR_WITNESS_PROBE.into(),
            registers: Default::default(),
            sample_a: String::new(),
            sample_b: String::new(),
            explain: vec!["     received_type=0x001f received_sequence=0".into()],
        };
        let m = build_bap_anchors(&[p]);
        assert_eq!(m.get(&(0x1f, 0)), Some(&9.0));
    }

    /// F4 regression: renaming the `explain()` labels takes the anchors to zero. The parser must
    /// keep working for the labels it claims to know, and `controls` (see `controls.rs`) is what
    /// catches a rename - this test pins the label constants the two share.
    #[test]
    fn anchor_labels_are_the_ones_the_control_pins() {
        assert!(ANCHOR_TYPE_LABELS.contains(&"received_type="));
        assert!(ANCHOR_SEQ_LABELS.contains(&"received_sequence="));
        let reworded = ProbeRecord {
            sequence: 9,
            probe_id: 32,
            probe: ANCHOR_WITNESS_PROBE.into(),
            registers: Default::default(),
            sample_a: String::new(),
            sample_b: String::new(),
            // A cosmetic reword of the SAME information - the exact 1061→0 failure.
            explain: vec!["     rx_type=0x001f rx_sequence=0".into()],
        };
        assert!(
            build_bap_anchors(&[reworded]).is_empty(),
            "sanity: a reword really does zero the anchors - that is why controls exist"
        );
    }

    #[test]
    fn interpolate_places_unanchored_between_anchors() {
        let mut resp = vec![
            ev(Stream::Bap, 0, Some(10.0)),
            ev(Stream::Http, 1, None),
            ev(Stream::Bap, 2, Some(20.0)),
        ];
        interpolate(&mut resp, 0.0, 100.0);
        assert_eq!(resp[1].order, Some(15.0)); // midpoint bracket
        assert_eq!(resp[1].placement, Placement::Bracketed);
    }

    /// F1 regression: rows outside the outermost anchor are EXTRAPOLATED, never bracketed.
    #[test]
    fn interpolate_marks_head_and_tail_as_outside() {
        let mut resp = vec![
            ev(Stream::Http, 0, None), // before the first anchor
            ev(Stream::Http, 1, None),
            ev(Stream::Bap, 2, Some(10.0)),
            ev(Stream::Http, 3, None), // truly bracketed
            ev(Stream::Bap, 4, Some(20.0)),
            ev(Stream::Http, 5, None), // after the last anchor
        ];
        interpolate(&mut resp, 0.0, 100.0);
        assert_eq!(resp[0].placement, Placement::Outside);
        assert_eq!(resp[1].placement, Placement::Outside);
        assert_eq!(resp[2].placement, Placement::Anchored);
        assert_eq!(resp[3].placement, Placement::Bracketed);
        assert_eq!(resp[4].placement, Placement::Anchored);
        assert_eq!(resp[5].placement, Placement::Outside);
    }

    #[test]
    fn interpolate_with_no_anchors_brackets_nothing() {
        let mut resp = vec![ev(Stream::Http, 0, None), ev(Stream::Bap, 1, None)];
        interpolate(&mut resp, 0.0, 100.0);
        assert!(resp.iter().all(|e| e.placement == Placement::Outside));
    }
}
