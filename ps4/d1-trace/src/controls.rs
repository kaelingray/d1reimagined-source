// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-trace controls` - pinned invariants, exit 2 when one breaks.
//!
//! Mirrors `d1-eboot controls` and `capture-rig/diagnostic-catalogue.py --controls`. **Run this
//! before citing anything `d1-trace` printed.**
//!
//! ## Why this exists (F4)
//!
//! `d1-trace` anchors BAP frames onto the probe spine by scraping the *human prose* of the probe
//! `explain` lines - `received_type=` / `received_sequence=`. When that prose came from
//! `capture-rig/decode-account-trace.py`, its format strings were edited twice in one day, and a
//! rename (display labels only, no semantic change) was measured to take `bap anchored: 1061/2342`
//! → **`0/2342`**, with an 8× position error on the first BAP frame, **exit 0, no warning, and
//! 12/12 unit tests still green**. The producer is now `probe_decode::explain` IN THIS CRATE and it
//! emits the labels from the same constants the scraper reads, so a reword moves both sides at
//! once; these controls remain as belt and braces over the whole pipeline.
//!
//! Two of the controls below are the ones that specifically catch that:
//!   - **C-ANCHOR-WITNESS** - if any probe record is named `secured_rx_dispatch_entry`, the anchor
//!     map MUST be non-empty. (A rename leaves the probe present and the anchors zero.)
//!   - **C-ANCHOR-RATE** - the anchor rate over *id-carrying* BAP rows must be ≥ 80%.
//!
//! ## Discipline
//!
//! - A control that cannot be *evaluated* is a control that FAILED (ATLAS §5). If the pinned run
//!   directory is missing, this command exits 2 rather than reporting nothing.
//! - Every scan control carries a **positive control** and, where it could degrade into "matches
//!   everything", a **negative** one too.
//! - Self-controls build their fixtures in a fresh temp directory and delete only that directory.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::ingest::{strip_ansi_for_controls, ANCHOR_WITNESS_PROBE};
use crate::model::{BapDir, Confidence, Stream, Trace};
use crate::{load_run, Query};

/// The pinned control artifact. Chosen because it is the run every §4 example in
/// `historicaldocs/design/trace-viewer.md` is quoted from, it is closed (no live process appends to it), and it
/// exercises all four lanes.
pub const PINNED_RUN: &str = "runs/20260808-110410-AVAIL-ITEM-TRACE";
/// Pinned decoded-probe count for `PINNED_RUN`.
const PINNED_PROBES: usize = 17862;
/// Pinned guest-diagnostic count for `PINNED_RUN`.
const PINNED_GUEST: usize = 3648;
/// Minimum acceptable anchor rate over id-carrying BAP rows.
const MIN_ANCHOR_RATE: f64 = 0.80;

/// One evaluated invariant.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

struct Ledger {
    passed: usize,
    failed: usize,
}

impl Ledger {
    fn check(&mut self, name: &'static str, ok: bool, detail: String) {
        self.record(Check {
            name,
            ok,
            detail,
        });
    }

    fn record(&mut self, c: Check) {
        if c.ok {
            self.passed += 1;
            println!("  PASS  {:<22} {}", c.name, c.detail);
        } else {
            self.failed += 1;
            println!("  FAIL  {:<22} {}", c.name, c.detail);
        }
    }
}

/// Ascend from the cwd (and from the binary's own directory) for the repository root - the
/// directory that holds both `capture-rig/` and `runs/`.
///
/// ⚠ This used to key on the exact file `capture-rig/decode-account-trace.py`; that decoder now
/// lives inside this binary, so root discovery keys on the two directories instead and survives
/// the script's eventual archival.
fn find_repo_root() -> Option<PathBuf> {
    let is_root = |p: &Path| p.join("capture-rig").is_dir() && p.join("runs").is_dir();
    if let Ok(repo) = std::env::var("D1_REPO") {
        let p = PathBuf::from(repo);
        if is_root(&p) {
            return Some(p);
        }
    }
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    for start in starts {
        let mut cur: Option<&Path> = Some(&start);
        while let Some(p) = cur {
            if is_root(p) {
                return Some(p.to_path_buf());
            }
            cur = p.parent();
        }
    }
    None
}

pub fn run() -> Result<()> {
    let started = std::time::Instant::now();
    let mut l = Ledger {
        passed: 0,
        failed: 0,
    };

    println!("d1-trace controls");
    let Some(repo) = find_repo_root() else {
        bail!(
            "could not locate the repository root (a directory holding both capture-rig/ and \
             runs/); set D1_REPO. A control that cannot be evaluated is a FAILED control."
        );
    };
    // Make the remaining capture-rig helpers (the dash lane's live scripts) discoverable from the
    // temp-dir fixtures too. The probe and blob-sidecar decoders themselves are native.
    std::env::set_var("D1_REPO", &repo);
    println!("  repo: {}", repo.display());

    // ---- Self-controls: no external artifact, so they run everywhere ------------------------
    println!("\n  self-controls (fixtures built in a temp dir)");
    self_controls(&mut l)?;

    // ---- Pinned-run controls ---------------------------------------------------------------
    let run_dir = repo.join(PINNED_RUN);
    println!("\n  pinned run: {}", run_dir.display());
    if !run_dir.is_dir() {
        l.check(
            "C-PINNED-RUN",
            false,
            format!("{} is missing - controls could not be evaluated", run_dir.display()),
        );
    } else {
        let trace = load_run(&run_dir)?;
        for c in pinned_controls(&trace) {
            l.record(c);
        }
    }

    // ---- blob-sidecar decoder over real artifacts (the absorbed world-launch-command.py) -----
    println!("\n  blob sidecar (C-BLOB-*)");
    for c in blob_artifact_controls(&repo) {
        l.record(c);
    }

    // ---- the dashboard's own invariants (historicaldocs/design/d1-dashboard.md §7) -------------------------
    println!("\n  d1-dash (C-DASH-*)");
    for c in crate::dash_controls::run_all(&repo) {
        l.record(c);
    }

    // ---- the ghost write-door invariants (design/d1-ghost.md §10) ----------------------------
    println!("\n  d1-ghost (C-GHOST-*)");
    for c in crate::ghost_controls::run_all(&repo) {
        l.record(c);
    }

    println!(
        "\n  {} passed, {} failed in {:.2}s",
        l.passed,
        l.failed,
        started.elapsed().as_secs_f64()
    );
    if l.failed > 0 {
        eprintln!("d1-trace controls FAILED - its output may not be cited.");
        std::process::exit(2);
    }
    println!("d1-trace controls OK");
    Ok(())
}

fn self_controls(l: &mut Ledger) -> Result<()> {
    // ---- native probe decoder (the absorbed decode-account-trace.py) ------------------------
    // C-DECODE-REFUSE: every one of the Python tool's four refusals must refuse here too. This
    // control CAN fail - delete a check in `probe_decode::read_records` and it does.
    {
        use crate::probe_decode::{make_record_bytes, read_records, RECORD_SIZE};
        let mut sample_a = [0u8; 48];
        sample_a[0..2].copy_from_slice(&0x001fu16.to_le_bytes());
        sample_a[2..6].copy_from_slice(&7u32.to_le_bytes());
        let good = make_record_bytes(46, 9, &sample_a);
        let decodes = read_records(&good, "ctl").is_ok();
        let mut bad_magic = good;
        bad_magic[0] ^= 0xff;
        let mut bad_version = good;
        bad_version[4] = 9;
        let mut bad_size = good;
        bad_size[8] = 216;
        let refusals = [
            read_records(&bad_magic, "ctl").err().map_or(false, |e| e.contains("bad magic")),
            read_records(&bad_version, "ctl")
                .err()
                .map_or(false, |e| e.contains("unsupported version")),
            read_records(&bad_size, "ctl").err().map_or(false, |e| e.contains("encoded size")),
            read_records(&good[..RECORD_SIZE - 1], "ctl")
                .err()
                .map_or(false, |e| e.contains("not a multiple")),
        ];
        l.check(
            "C-DECODE-REFUSE",
            decodes && refusals.iter().all(|&r| r),
            format!("valid record decodes={decodes}; refusals magic/version/size/length={refusals:?}"),
        );

        // C-DECODE-ANCHOR-LABELS (F4, producer side): the witness probe's explain text must carry
        // the exact labels `build_bap_anchors` scrapes, and an INVERTED control - perturbing the
        // sample bytes must change the output.
        let recs = read_records(&good, "ctl").unwrap_or_default();
        let text = recs
            .first()
            .map(|r| crate::probe_decode::explain(r).join("\n"))
            .unwrap_or_default();
        let labels_present =
            text.contains("received_type=0x001f") && text.contains("received_sequence=7");
        let mut perturbed = good;
        perturbed[136] ^= 0x01; // sample_a[0]
        let text2 = read_records(&perturbed, "ctl")
            .ok()
            .and_then(|r| r.first().map(crate::probe_decode::explain))
            .map(|v| v.join("\n"))
            .unwrap_or_default();
        l.check(
            "C-DECODE-ANCHOR-LABELS",
            labels_present && text2 != text && !text2.is_empty(),
            format!(
                "labels present={labels_present} (witness explain: {:?}), perturbed input changes \
                 output={}",
                text.lines().nth(1).unwrap_or(""),
                text2 != text
            ),
        );
    }

    // ---- native blob-sidecar decoder (the absorbed world-launch-command.py) ------------------
    // C-BLOB-REFUSE: every refusal must refuse - the Python's three (bad magic, truncated payload,
    // empty file) byte-identically, plus the trailing-partial-header refusal the Python LACKED (it
    // silently summarised a sidecar truncated mid-header as complete). This control CAN fail -
    // delete a check in `blob_decode::read_blobs` and it does.
    {
        use crate::blob_decode::{diff, make_blob_bytes, read_blobs, summarise, HEADER_SIZE};
        let payload: Vec<u8> = (0u8..48).collect();
        let mut good = make_blob_bytes(88, 52, 0x2_0503_b81c, &payload);
        good.extend_from_slice(&make_blob_bytes(86, 99, 0x7_efcf_1f48, &payload));
        let decodes = read_blobs(&good).map(|b| b.len() == 2).unwrap_or(false);
        let mut bad_magic = good.clone();
        bad_magic[0] ^= 0xff;
        let mut tail = good.clone();
        tail.extend_from_slice(&[0u8; 7]);
        let refusals = [
            read_blobs(&bad_magic).err().is_some_and(|e| e.contains("bad magic")),
            read_blobs(&good[..good.len() - 5])
                .err()
                .is_some_and(|e| e.contains("truncated blob")),
            read_blobs(&[]).err().is_some_and(|e| e.contains("no blobs in file")),
            read_blobs(&tail).err().is_some_and(|e| e.contains("truncated mid-header")),
            read_blobs(&good)
                .ok()
                .and_then(|b| diff(&b, 0, 9).err())
                .is_some_and(|e| e.contains("no blob #9 (have 2)")),
        ];
        l.check(
            "C-BLOB-REFUSE",
            decodes && refusals.iter().all(|&r| r),
            format!(
                "valid sidecar decodes={decodes}; refusals magic/truncated/empty/mid-header/index=\
                 {refusals:?}"
            ),
        );

        // C-BLOB-INVERT: perturb one sidecar payload byte, prove BOTH outputs change; and the
        // negative arm - two identical commands must say IDENTICAL, not invent a difference.
        let clean = read_blobs(&good).unwrap_or_default();
        let mut perturbed_bytes = good.clone();
        perturbed_bytes[HEADER_SIZE + 8] ^= 0x01;
        let perturbed = read_blobs(&perturbed_bytes).unwrap_or_default();
        let summary_changes = !clean.is_empty()
            && !perturbed.is_empty()
            && summarise(&clean) != summarise(&perturbed);
        let diff_changes = matches!(
            (diff(&clean, 0, 1), diff(&perturbed, 0, 1)),
            (Ok(a), Ok(b)) if a != b
        );
        let identical_says_so = diff(&clean, 0, 1)
            .map(|t| t.contains("IDENTICAL - these two launches carry the same command"))
            .unwrap_or(false);
        l.check(
            "C-BLOB-INVERT",
            summary_changes && diff_changes && identical_says_so,
            format!(
                "one-bit perturbation changes summary={summary_changes} diff={diff_changes}; \
                 identical pair says IDENTICAL={identical_says_so}"
            ),
        );
    }

    // C-UTF8-STRIP (F5): ANSI stripping must not Latin-1-decode UTF-8.
    let src = "\u{1b}[33m      ⭐ FAMILY 4 ANSWER seq 0 - answering the SUBSCRIPTION\u{1b}[0m";
    let got = strip_ansi_for_controls(src);
    l.check(
        "C-UTF8-STRIP",
        got == "      ⭐ FAMILY 4 ANSWER seq 0 - answering the SUBSCRIPTION",
        format!("⭐ and - survive ANSI stripping: {got:?}"),
    );

    // C-SEARCH-DETAIL (F7): a bare word reaches detail - with a negative control so this cannot be
    // satisfied by "matches everything".
    let mut ev = crate::model::Event {
        id: 0,
        stream: Stream::Probe,
        order: 53.0,
        confidence: Confidence::Exact,
        ordinal: 53,
        summary: "#53 item_tag_resolved    guest=0x00555775".into(),
        detail: vec!["     world_tag=0x80a6035a resolved".into()],
        probe: None,
        bap: None,
    };
    let pos = Query::parse("world_tag").matches(&ev);
    let neg = Query::parse("zzz_absent_token").matches(&ev);
    l.check(
        "C-SEARCH-DETAIL",
        pos && !neg,
        format!("world_tag in detail matches={pos} (positive), nonsense matches={neg} (negative)"),
    );
    ev.detail.clear();

    // C-QUERY-TYPO (F9b): `rax=2` is an error, not a silent substring search.
    let q = Query::parse("rax=2");
    l.check(
        "C-QUERY-TYPO",
        q.errors().len() == 1,
        format!("rax=2 → errors {:?}", q.errors()),
    );

    // C-QUERY-UNSAT (F9c): a mask/value mismatch warns.
    let q = Query::parse("rax&0xffffffff==0xdead00000002");
    let flagship = Query::parse("probe:item_load_state_primary rax&0xffffffff==2");
    l.check(
        "C-QUERY-UNSAT",
        q.warnings().len() == 1 && flagship.warnings().is_empty(),
        format!(
            "unsatisfiable warns ({}), flagship does not ({})",
            q.warnings().len(),
            flagship.warnings().len()
        ),
    );

    // ---- Fixture-backed self-controls -------------------------------------------------------
    let tmp = std::env::temp_dir().join(format!("d1-trace-controls-{}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    let result = fixture_controls(l, &tmp);
    // Only ever remove the directory this function created.
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// A tiny but realistic `responder.log`: one HTTP exchange, one BAP frame with its outer header,
/// raw hex, inner decode, reply identity and server reply.
const FIXTURE_RESPONDER: &str = "\
 INFO listening http://0.0.0.0:80
 INFO --> POST www4.signon.deadorbit.net/SignOn  797B
 INFO     authorization: Bearer d1-local-token
 INFO     success response: type=0 silo=12345 bap=10.77.0.2:3075 manifest_url=http://x/m.bin
 INFO <-- 200 SignOn, 201B placeholder (synthetic-generic)
 WARN     BAP TCP msg#1 28B [type 0101 len 22 (+6 = 28)]
 INFO       01 01 00 00 00 16 b8 d4 0f 98 99 bc b6 62 57 08
 WARN       SECURED (6B): 00 79 00 00 00 02 -> Ok(Inner { msg_type: 121, sequence: 2 })
 INFO       -> register subscriber rsp (0x007a), seq 2
 WARN     --> BAP SECURED REPLY 30B: 01 01 00 00 00 18 fe fa 14 40
 WARN       ⭐ FAMILY 4 ANSWER seq 0 root 0xd100000000000001 - answering the SUBSCRIPTION
";

fn write_fixture_run(dir: &Path, guest_bytes: Option<&[u8]>, trace_bytes: Option<&[u8]>) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("responder.log"), FIXTURE_RESPONDER)?;
    if let Some(g) = guest_bytes {
        let mut f = std::fs::File::create(dir.join("emulator.log"))?;
        f.write_all(g)?;
    }
    if let Some(t) = trace_bytes {
        std::fs::write(dir.join("account-trace.bin"), t)?;
    }
    Ok(())
}

fn fixture_controls(l: &mut Ledger, tmp: &Path) -> Result<()> {
    // C-NO-SPINE (F2): a run with no probe trace must refuse to synthesise cross-stream order.
    let d = tmp.join("no-spine");
    write_fixture_run(&d, Some(b"[Tty] <Info> log_flush: [stdout] bootflow:orbit\n"), None)?;
    let t = load_run(&d)?;
    let all_approx = t.events.iter().all(|e| e.confidence == Confidence::Approx);
    let noted = t
        .summary
        .notes
        .iter()
        .any(|n| n.contains("NO CROSS-STREAM SPINE"));
    let lanes_present = t.summary.http > 0 && t.summary.guest > 0 && t.summary.bap > 0;
    l.check(
        "C-NO-SPINE",
        !t.summary.spine && all_approx && noted && lanes_present,
        format!(
            "spine={} all-approx={all_approx} loud-note={noted} lanes(h{},b{},g{})",
            t.summary.spine, t.summary.http, t.summary.bap, t.summary.guest
        ),
    );

    // C-LOSSY-UTF8 (F3): one bad byte must not zero a lane.
    let d = tmp.join("bad-utf8");
    let mut guest = Vec::new();
    guest.extend_from_slice(b"[Tty] <Info> log_flush: [stdout] first line\n");
    guest.extend_from_slice(b"[Tty] <Info> log_flush: [stdout] tainted \xff line\n");
    guest.extend_from_slice(b"[Tty] <Info> log_flush: [stdout] bootflow:orbit\n");
    write_fixture_run(&d, Some(&guest), None)?;
    let t = load_run(&d)?;
    let noted = t.summary.notes.iter().any(|n| n.contains("not valid UTF-8"));
    l.check(
        "C-LOSSY-UTF8",
        t.summary.guest == 3 && noted,
        format!(
            "guest lines kept = {} (expect 3), lossy-note={noted}",
            t.summary.guest
        ),
    );

    // C-BAD-TRACE (F8): an undecodable account-trace.bin must not refuse the whole run directory.
    // 100 bytes is not a multiple of the 232-byte record, exactly like the 18 pre-232-era runs.
    let d = tmp.join("bad-trace");
    write_fixture_run(
        &d,
        Some(b"[Tty] <Info> log_flush: [stdout] bootflow:orbit\n"),
        Some(&[0u8; 100]),
    )?;
    let loaded = load_run(&d);
    let ok = match &loaded {
        Ok(t) => {
            let noted = t
                .summary
                .notes
                .iter()
                .any(|n| n.contains("EMPTY BECAUSE OF AN ERROR"));
            let lanes = t.summary.http > 0 && t.summary.guest > 0;
            noted && lanes
        }
        Err(_) => false,
    };
    l.check(
        "C-BAD-TRACE",
        ok,
        match &loaded {
            Ok(t) => format!(
                "loaded anyway: h{} b{} g{} p{}, note carried",
                t.summary.http, t.summary.bap, t.summary.guest, t.summary.probe
            ),
            Err(e) => format!("REFUSED the directory: {e:#}"),
        },
    );

    // C-FRAME-FOLD (F6/F10): the outer `BAP TCP msg#` header is folded into the inner decode (one
    // BAP recv row, not two) and the server's reply is its own `send` row.
    let d = tmp.join("frame");
    write_fixture_run(&d, None, None)?;
    let t = load_run(&d)?;
    let recv = t
        .events
        .iter()
        .filter(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Recv))
        .count();
    let send = t
        .events
        .iter()
        .filter(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Send))
        .count();
    let header_rows = t
        .events
        .iter()
        .filter(|e| e.summary.contains("BAP TCP msg#"))
        .count();
    let folded = t
        .events
        .iter()
        .any(|e| e.detail.iter().any(|d| d.contains("BAP TCP msg#1")));
    let send_id = t
        .events
        .iter()
        .find(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Send))
        .and_then(|e| e.bap.as_ref())
        .map(|b| (b.msg_type, b.sequence));
    l.check(
        "C-FRAME-FOLD",
        recv == 1 && send == 1 && header_rows == 0 && folded && send_id == Some((Some(0x7a), Some(2))),
        format!(
            "recv={recv} send={send} standalone-header-rows={header_rows} folded={folded} \
             send-id={send_id:?} (expect recv 1, send 1, headers 0, send id (0x7a, 2))"
        ),
    );

    // C-RECOVERED-LINES (F7): responder lines that used to be dropped entirely are on the timeline.
    let recovered = ["success response", "⭐ FAMILY 4 ANSWER", "listening http"];
    let missing: Vec<&str> = recovered
        .iter()
        .copied()
        .filter(|needle| {
            !t.events
                .iter()
                .any(|e| e.summary.contains(needle) || e.detail.iter().any(|d| d.contains(needle)))
        })
        .collect();
    l.check(
        "C-RECOVERED-LINES",
        missing.is_empty(),
        format!("previously-dropped lines present; missing={missing:?}"),
    );

    Ok(())
}


/// The documented world-launch specimen (`historicaldocs/design/world-launch-command.md`) and the
/// two-worlds sidecar, decoded natively and checked against facts pinned from the Python tool's
/// output on 2026-08-11 (the porting golden diff).
///
/// Both are REAL artifacts: a missing file is a FAILED control (a control that cannot be evaluated
/// failed), same as `C-PINNED-RUN`.
fn blob_artifact_controls(repo: &Path) -> Vec<Check> {
    use crate::blob_decode::{diff, read_blobs, summarise};
    let mut out: Vec<Check> = Vec::new();
    let mut push = |name: &'static str, ok: bool, detail: String| {
        out.push(Check { name, ok, detail });
    };

    // C-BLOB-SPECIMEN: the probe-88 command capture every §-reference cites.
    let specimen = repo.join("runs/20260808-142443-WORLD-LAUNCH-1080-C/account-trace.bin.blob");
    match std::fs::read(&specimen).map_err(|e| e.to_string()).and_then(|d| read_blobs(&d)) {
        Ok(blobs) => {
            let b = &blobs[0];
            let shape = blobs.len() == 1
                && b.probe_id == 88
                && b.sequence == 52
                && b.address == 0x2_0503_b81c
                && b.bytes.len() == 0x18F8;
            let text = summarise(&blobs);
            let facts = text.contains("'ambient_bootstrap'@0x630")
                && text.contains("530 non-zero bytes (8%)");
            push(
                "C-BLOB-SPECIMEN",
                shape && facts,
                format!(
                    "1 blob, probe 88 seq 52 len 0x18f8 @0x20503b81c = {shape}; pinned summary \
                     facts (ambient_bootstrap@0x630, 530 non-zero) = {facts}"
                ),
            );
        }
        Err(e) => push(
            "C-BLOB-SPECIMEN",
            false,
            format!("{} could not be decoded: {e}", specimen.display()),
        ),
    }

    // C-BLOB-TWO-WORLDS: the flagship two-command diff - ambient_bootstrap vs orbit captured in
    // ONE run must differ exactly where the Python said (116 bytes, 21 regions, the world-name
    // text), and two same-world commands must be IDENTICAL (the negative arm).
    let sliceset = repo.join("runs/20260809-143929-ORBIT-SLICESET/account-trace.bin.blob");
    match std::fs::read(&sliceset).map_err(|e| e.to_string()).and_then(|d| read_blobs(&d)) {
        Ok(blobs) => {
            let five = blobs.len() == 5;
            let d01 = diff(&blobs, 0, 1).unwrap_or_default();
            let differs = d01.contains("116 of 6392 bytes differ (1%)")
                && d01.contains("21 differing region(s)")
                && d01.contains("'ent_bootstrap' -> 't'");
            let d12 = diff(&blobs, 1, 2).unwrap_or_default();
            let identical = d12.contains("IDENTICAL - these two launches carry the same command");
            push(
                "C-BLOB-TWO-WORLDS",
                five && differs && identical,
                format!(
                    "5 blobs={five}; ambient_bootstrap vs orbit pinned diff (116B/21 regions/world \
                     text)={differs}; orbit vs orbit IDENTICAL={identical}"
                ),
            );
        }
        Err(e) => push(
            "C-BLOB-TWO-WORLDS",
            false,
            format!("{} could not be decoded: {e}", sliceset.display()),
        ),
    }

    out
}

/// The pinned-run invariants, evaluated (not printed) so tests can assert on them directly.
///
/// ⚠ This is the F4 catcher: `C-ANCHOR-WITNESS` and `C-ANCHOR-RATE` are what fail when a cosmetic
/// reword of `decode-account-trace.py`'s `explain()` labels takes 1061 anchors → 0.
pub fn pinned_controls(trace: &Trace) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();
    let mut push = |name: &'static str, ok: bool, detail: String| {
        out.push(Check { name, ok, detail });
    };
    let s = &trace.summary;

    // C-PROBE-COUNT / C-GUEST-COUNT: positive controls on the two shell-out / scrape lanes.
    push(
        "C-PROBE-COUNT",
        s.probe == PINNED_PROBES,
        format!("{} decoded probe records (pinned {PINNED_PROBES})", s.probe),
    );
    push(
        "C-GUEST-COUNT",
        s.guest == PINNED_GUEST,
        format!("{} guest diagnostic lines (pinned {PINNED_GUEST})", s.guest),
    );

    // C-ANCHOR-WITNESS (F4a): the witness probe fired, so the anchors must NOT be zero. A reword of
    // the `explain()` labels leaves the probe present and the anchor map empty - this is the check
    // that turns that silent 1061→0 into an exit 2.
    let witness = trace
        .events
        .iter()
        .filter(|e| {
            e.probe
                .as_ref()
                .is_some_and(|p| p.probe == ANCHOR_WITNESS_PROBE)
        })
        .count();
    push(
        "C-ANCHOR-WITNESS",
        witness == 0 || s.bap_anchored > 0,
        format!(
            "{witness} × {ANCHOR_WITNESS_PROBE} records ⇒ anchors must be > 0; anchored = {}",
            s.bap_anchored
        ),
    );

    // C-ANCHOR-RATE (F4b): rate over ID-CARRYING rows, not over every row (F6).
    let rate = if s.bap_anchorable == 0 {
        0.0
    } else {
        s.bap_anchored as f64 / s.bap_anchorable as f64
    };
    push(
        "C-ANCHOR-RATE",
        rate >= MIN_ANCHOR_RATE,
        format!(
            "{}/{} id-carrying BAP rows anchored = {:.1}% (floor {:.0}%)",
            s.bap_anchored,
            s.bap_anchorable,
            rate * 100.0,
            MIN_ANCHOR_RATE * 100.0
        ),
    );

    // C-SPINE: the pinned run does have a real cross-stream spine.
    push("C-SPINE", s.spine, format!("cross-stream spine = {}", s.spine));

    // C-NO-FALSE-BRACKET (F1): nothing outside the outermost anchor may claim `~`.
    let anchored_orders: Vec<f64> = trace
        .events
        .iter()
        .filter(|e| e.stream == Stream::Bap && e.confidence == Confidence::Exact)
        .map(|e| e.order)
        .collect();
    let lo = anchored_orders.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = anchored_orders
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let outside_bracketed = trace
        .events
        .iter()
        .filter(|e| e.confidence == Confidence::Bracketed)
        .filter(|e| e.order < lo || e.order > hi)
        .count();
    let bracketed_total = trace
        .events
        .iter()
        .filter(|e| e.confidence == Confidence::Bracketed)
        .count();
    push(
        "C-NO-FALSE-BRACKET",
        outside_bracketed == 0 && bracketed_total > 0,
        format!(
            "{outside_bracketed} rows claim ~ outside the anchor span [{lo:.1}, {hi:.1}]; \
             {bracketed_total} genuinely bracketed (positive control)"
        ),
    );

    // C-SIGNON-ORDER (F1, the measured inversion): the /SignOn POST precedes every anchor, so it
    // must be ≈, never ~ - its RESPONSE is what delivers `bap=…:3075`, so BAP cannot precede it.
    let signon = trace
        .events
        .iter()
        .find(|e| e.stream == Stream::Http && e.summary.contains("/SignOn"));
    push(
        "C-SIGNON-ORDER",
        signon.is_some_and(|e| e.confidence == Confidence::Approx),
        match signon {
            Some(e) => format!(
                "POST /SignOn order={:.2} confidence={:?} (must be Approx - nothing anchors before it)",
                e.order, e.confidence
            ),
            None => "no /SignOn row found - control could not be evaluated".to_string(),
        },
    );

    // C-FRAME-FOLD-RUN (F6): no standalone outer-header rows, no `BAP BAP` double prefix.
    let header_rows = trace
        .events
        .iter()
        .filter(|e| e.summary.contains("BAP TCP msg#"))
        .count();
    let double_prefix = trace
        .events
        .iter()
        .filter(|e| e.summary.contains("BAP BAP"))
        .count();
    push(
        "C-FRAME-FOLD-RUN",
        header_rows == 0 && double_prefix == 0,
        format!("standalone outer-header rows={header_rows}, 'BAP BAP' summaries={double_prefix}"),
    );

    // C-BAP-SEND (F10): the server→client half of the conversation is on the timeline.
    let send = trace
        .events
        .iter()
        .filter(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Send))
        .count();
    push(
        "C-BAP-SEND",
        send > 0,
        format!("{send} BAP rows labelled send (BapDir::Send constructed)"),
    );

    // C-WORLD-TAG (F7): the design doc's flagship cross-stream field, with a negative control.
    let q_pos = Query::parse("world_tag");
    let q_neg = Query::parse("zzz_absent_token");
    let pos = trace.events.iter().filter(|e| q_pos.matches(e)).count();
    let neg = trace.events.iter().filter(|e| q_neg.matches(e)).count();
    push(
        "C-WORLD-TAG",
        pos > 0 && neg == 0,
        format!("world_tag matches {pos} events (positive), nonsense matches {neg} (negative)"),
    );

    // C-STAR-ANNOTATION (F5 end to end): the ⭐ lines survive ANSI stripping, classification and
    // search - the whole pipeline, not just the stripper.
    let q_star = Query::parse("FAMILY 4 ANSWER");
    let star = trace.events.iter().filter(|e| q_star.matches(e)).count();
    push(
        "C-STAR-ANNOTATION",
        star > 0,
        format!("{star} rows carry the ⭐ FAMILY ANSWER annotation"),
    );

    // C-FLAGSHIP: historicaldocs/design/trace-viewer.md §4 - item_load_state_primary returned 2 at seq 2596/5066.
    let q = Query::parse("probe:item_load_state_primary rax&0xffffffff==2");
    let hits: Vec<u64> = trace
        .events
        .iter()
        .filter(|e| q.matches(e))
        .map(|e| e.ordinal)
        .collect();
    push(
        "C-FLAGSHIP",
        hits == vec![2596, 5066],
        format!("flagship query hits at probe sequences {hits:?} (pinned [2596, 5066])"),
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Event, ProbeRecord, RunSummary};

    fn probe_event(name: &str, seq: u64) -> Event {
        Event {
            id: 0,
            stream: Stream::Probe,
            order: seq as f64,
            confidence: Confidence::Exact,
            ordinal: seq,
            summary: format!("#{seq} {name}"),
            detail: vec![],
            probe: Some(ProbeRecord {
                sequence: seq,
                probe_id: 32,
                probe: name.to_string(),
                registers: Default::default(),
                sample_a: String::new(),
                sample_b: String::new(),
                explain: vec![],
            }),
            bap: None,
        }
    }

    fn named(checks: &[Check], name: &str) -> bool {
        checks.iter().find(|c| c.name == name).expect(name).ok
    }

    /// F4 regression - **the test that would have caught the silent 1061 → 0.**
    ///
    /// A cosmetic reword of `decode-account-trace.py`'s `explain()` labels leaves the witness probe
    /// firing and the anchor map empty. Every ingest unit test still passes (they feed the parser
    /// the OLD labels); only a control that asserts *witness present ⇒ anchors non-zero* notices.
    #[test]
    fn anchor_witness_control_fails_when_a_label_reword_zeroes_the_anchors() {
        let trace = Trace {
            events: vec![probe_event(ANCHOR_WITNESS_PROBE, 9)],
            summary: RunSummary {
                probe: 1,
                bap_anchored: 0,
                bap_anchorable: 2342,
                spine: false,
                ..Default::default()
            },
        };
        let checks = pinned_controls(&trace);
        assert!(
            !named(&checks, "C-ANCHOR-WITNESS"),
            "the witness probe fired and anchors are 0 - this MUST fail"
        );
        assert!(!named(&checks, "C-ANCHOR-RATE"), "0/2342 is below the 80% floor");
    }

    /// …and it passes when the anchors are healthy, so the control is not vacuously red.
    #[test]
    fn anchor_witness_control_passes_when_anchors_are_healthy() {
        let trace = Trace {
            events: vec![probe_event(ANCHOR_WITNESS_PROBE, 9)],
            summary: RunSummary {
                probe: 1,
                bap_anchored: 1061,
                bap_anchorable: 1171,
                ..Default::default()
            },
        };
        let checks = pinned_controls(&trace);
        assert!(named(&checks, "C-ANCHOR-WITNESS"));
        assert!(named(&checks, "C-ANCHOR-RATE"), "1061/1171 = 90.6% clears the 80% floor");
    }

    /// F6: the old denominator (every BAP row, half of them unanchorable outer headers) put the
    /// SAME healthy run below the floor. Proof the denominator change is load-bearing.
    #[test]
    fn old_denominator_would_have_failed_the_rate_control() {
        let trace = Trace {
            events: vec![probe_event(ANCHOR_WITNESS_PROBE, 9)],
            summary: RunSummary {
                probe: 1,
                bap_anchored: 1061,
                bap_anchorable: 2342, // the pre-F6 denominator: 50% ceiling by construction
                ..Default::default()
            },
        };
        assert!(!named(&pinned_controls(&trace), "C-ANCHOR-RATE"));
    }
}
