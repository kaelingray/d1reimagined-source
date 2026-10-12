// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The boot-path regression assert: does a run still pass every gate the project already won?
//!
//! Port of `capture-rig/gate-check.py`. The reason it exists is unchanged: three times on
//! 2026-08-07 a run silently un-passed a gate that was already won, because the input that won it
//! lived only in a shell variable (`D1_MANIFEST_FILE`, `D1_PUSH_CHARACTER`, `D1_503_SOID`/
//! `D1_503_RAW`). Each time the failure surfaced three gates earlier than the thing under test and
//! read exactly like a regression in it; two client-behaviour hypotheses were written on that
//! confound and retracted. A gate is only "passed" if a fresh run can be shown to pass it, and a
//! finding from a run that fails this check is not a finding - it is a confound.
//!
//! ## Format ownership (the no-format-twice rule)
//!
//! The Python prototype re-described the 232-byte tracer record (`gate-check.py:33`), which was the
//! named violation this port closes. Probe records enter here through exactly one door: the native
//! decoder `d1-trace/src/probe_decode.rs` (the byte-faithful Rust port of
//! `capture-rig/decode-account-trace.py`), injected by the caller as a [`ProbeDecoder`]. No Rust
//! code in this crate describes the record layout, and nothing here spawns Python at runtime -
//! the shell-out seam named by the previous revision of this comment has been replaced.
//!
//! The decoder arrives by injection because the dependency points the other way: `d1-trace`
//! depends on `d1-run`, so this crate cannot name `d1_trace::probe_decode` without a cycle. The
//! `d1` CLI, which sits above both, wires them together (`native_probe_decode` in `d1/src/main.rs`).
//!
//! ## Named divergences from the prototype, all deliberate
//!
//! * Default run selection is the newest *timestamp-named* run directory (via
//!   [`crate::list_runs`] plus the CLI's timestamp filter), not the newest `runs/` entry by
//!   mtime. The prototype's mtime pick could select a stray file such as `guest-writes.jsonl`
//!   and crash; a bare name sort would let scratch directories (`sweep-*`, `snapshots`) win
//!   "newest" forever because they sort after the digits.
//! * An explicit run path that does not exist is refused loudly. The prototype printed a
//!   confident all-FAIL gate ledger for a typo'd path, exit 1, indistinguishable from a real
//!   fully-regressed run.
//! * A corrupt or pre-232-byte-era `account-trace.bin` fails the whole probe gate here (the
//!   decoder refuses the file, and a note goes to the caller); the prototype silently skipped
//!   records whose magic did not match and kept counting the rest. Stricter, and loud about it.
//! * Bootflow reach is measured on the CLI's existing gate ladder (`Entering state` transitions
//!   from [`crate::parse_emulator_log`]), not on a `bootflow:` mention anywhere in the log. A
//!   truncated log that lost the Entering line but kept a later mention would flip; verified
//!   equivalent on real runs (see the golden diff in the port record).
//! * The crash gate's detail names `d1 crash`, where the prototype said `run harvest-crash.py`.
//!   Asked for by the operator: the crash harvester is a `d1` subcommand now, and a shipped
//!   binary must not direct a human at an archived Python file. This is the one deliberate break
//!   from byte parity with the prototype's render.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{parse_emulator_log, read_lossy, strip_ansi, Event};

/// One gate: the label, the verdict, a short detail, and what originally won it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub label: String,
    pub ok: bool,
    pub detail: String,
    /// How the gate was won the first time - the thing to re-check when it regresses.
    pub won_by: String,
}

/// The whole report for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub run: String,
    pub gates: Vec<GateResult>,
    /// The check's own control: the tracer must have armed, and records must exist.
    pub tracer_armed: String,
    pub trace_records: u64,
    /// True when any previously-passed gate is now failing.
    pub regressed: bool,
    /// Set when the probe lane was empty because of an error (decoder missing or refused the
    /// trace). The prototype had no such state; here it is loud instead of silent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_note: Option<String>,
}

/// Probe-15 (`acct_ready`) tallies pulled out of the decoder's JSON. Only the fields the gate
/// predicate needs are deserialized; the record layout itself stays described in Python only.
#[derive(Debug, Default)]
struct ProbeTally {
    total_records: u64,
    p15_total: u64,
    p15_true: u64,
    note: Option<String>,
}

/// One decoded probe sample - just the two fields the gate predicate reads. This is a
/// decoded-value contract, NOT a record-format description: parsing the 232-byte record stays in
/// exactly one Rust place, `d1-trace/src/probe_decode.rs`, and reaches this module only through
/// the injected [`ProbeDecoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeSample {
    pub probe_id: u16,
    pub rax: u64,
}

/// The decoder the caller injects: whole trace-file bytes in, samples out, or the decoder's own
/// refusal message. The `d1` CLI passes the native `d1_trace::probe_decode` port; this crate's
/// own tests pass [`controls::pinned_excerpt_decode`], a bytes-keyed pinned-answer double.
pub type ProbeDecoder = fn(&[u8]) -> Result<Vec<ProbeSample>, String>;

/// Tally probe 15 (`acct_ready`) out of `account-trace.bin` through the injected decoder.
///
/// Failure shape, deliberately preserved from the shell-out era: a MISSING trace is the
/// prototype's quiet empty state (the `tracer armed 0` control line is the loud half of that);
/// an unreadable or REFUSED trace is a loud note. What changed is the reason a refusal can
/// happen - it is now always about the trace bytes, never about Python or a script path being
/// absent. Repo-root discovery is gone from this path entirely: nothing here needs a script.
fn load_probe_tally(run_dir: &Path, decode: ProbeDecoder) -> ProbeTally {
    let trace = run_dir.join("account-trace.bin");
    if !trace.is_file() {
        return ProbeTally::default();
    }
    let blob = match std::fs::read(&trace) {
        Ok(b) => b,
        Err(e) => {
            return ProbeTally {
                note: Some(format!("reading {}: {e}", trace.display())),
                ..Default::default()
            }
        }
    };
    let samples = match decode(&blob) {
        Ok(s) => s,
        Err(e) => {
            return ProbeTally {
                note: Some(format!("the probe decoder refused {}: {e}", trace.display())),
                ..Default::default()
            }
        }
    };
    let mut t = ProbeTally { total_records: samples.len() as u64, ..Default::default() };
    for s in &samples {
        if s.probe_id == 15 {
            t.p15_total += 1;
            // The predicate the prototype checked: AL (rax & 0xFF) nonzero means account_ready.
            if s.rax & 0xFF != 0 {
                t.p15_true += 1;
            }
        }
    }
    t
}

/// All numbers matching `<prefix><digits>B` (at least one digit), in order of appearance.
fn numbers_after(text: &str, prefix: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        let after = &rest[i + prefix.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && after[digits.len()..].starts_with('B') {
            if let Ok(n) = digits.parse() {
                out.push(n);
            }
        }
        rest = &rest[i + prefix.len()..];
    }
    out
}

/// `BAP TCP msg#<n> <bytes>B` frame sizes, in order.
fn bap_frame_sizes(text: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("BAP TCP msg#") {
        let after = &rest[i + "BAP TCP msg#".len()..];
        let msg_digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        let tail = &after[msg_digits.len()..];
        if !msg_digits.is_empty() && tail.starts_with(' ') {
            let sz: String = tail[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if !sz.is_empty() && tail[1 + sz.len()..].starts_with('B') {
                if let Ok(n) = sz.parse() {
                    out.push(n);
                }
            }
        }
        rest = &rest[i + "BAP TCP msg#".len()..];
    }
    out
}

/// First `MANIFEST from <peer>, <n>B` size, if any.
fn manifest_size(text: &str) -> Option<u64> {
    let mut rest = text;
    while let Some(i) = rest.find("MANIFEST from ") {
        let after = &rest[i + "MANIFEST from ".len()..];
        // The peer token is non-whitespace up to ", "; then digits then 'B'.
        if let Some(comma) = after.find(", ") {
            let peer = &after[..comma];
            if !peer.is_empty() && !peer.contains(char::is_whitespace) {
                let sz: String =
                    after[comma + 2..].chars().take_while(|c| c.is_ascii_digit()).collect();
                if !sz.is_empty() && after[comma + 2 + sz.len()..].starts_with('B') {
                    if let Ok(n) = sz.parse() {
                        return Some(n);
                    }
                }
            }
        }
        rest = &rest[i + "MANIFEST from ".len()..];
    }
    None
}

/// First `account tracer armed: <n>` count, as a string; "0" when never armed.
fn tracer_armed(text: &str) -> String {
    if let Some(i) = text.find("account tracer armed: ") {
        let after = &text[i + "account tracer armed: ".len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            return digits;
        }
    }
    "0".to_string()
}

/// Run the gate ledger over one run directory. `decode` is the probe-record decoder to use for
/// `account-trace.bin` - see [`ProbeDecoder`] for why it is injected rather than named.
pub fn gate_check(run_dir: &Path, decode: ProbeDecoder) -> GateReport {
    let emu = strip_ansi(&read_lossy(&run_dir.join("emulator.log")).unwrap_or_default());
    let resp = strip_ansi(&read_lossy(&run_dir.join("responder.log")).unwrap_or_default());
    let containment = read_lossy(&run_dir.join("containment.txt")).unwrap_or_default();
    let tally = load_probe_tally(run_dir, decode);

    // Bootflow reach comes from the CLI's one existing ladder parser, not a second regex.
    let boot_states: Vec<String> = parse_emulator_log(&emu)
        .into_iter()
        .filter_map(|e| match e {
            Event::BootState { state, .. } => Some(state),
            _ => None,
        })
        .collect();
    let reached = |s: &str| boot_states.iter().any(|st| st == s);

    let close_sizes = numbers_after(&resp, "peer closed after ");
    // `peer closed after NB` only exists when the client HUNG UP. Measuring BAP progress solely
    // from it means a connection that is still ALIVE scores zero and the gate reports "no close
    // seen" -> FAIL -> "A PREVIOUSLY-PASSED GATE REGRESSED". That is the cure being reported as
    // the disease, and it fired the first time the connection survived (run
    // 20260807-162724-subackhdr-1, 130 messages, zero errors, after 0x000C started being
    // answered). Fall back to the per-message frame sizes, which exist either way. This is the
    // ATLAS 4.2b trap in a report rather than a scan: the instrument's control passed, its
    // predicate was simply blind to the outcome we were trying to produce.
    let frame_sizes = bap_frame_sizes(&resp);
    let bap_alive = !frame_sizes.is_empty() && close_sizes.is_empty();
    let bap_bytes: Vec<u64> = if close_sizes.is_empty() && !frame_sizes.is_empty() {
        vec![frame_sizes.iter().sum()]
    } else {
        close_sizes
    };

    let manifest = manifest_size(&resp);
    let count_503 = resp.matches("503 RAW BODY").count();
    let gcm_failures = resp.matches("SECURED DECRYPT FAILED").count();
    let crashed_emu = emu.contains("crash_folder");

    let mut gates = Vec::new();
    let mut push = |label: &str, ok: bool, detail: String, won_by: &str| {
        gates.push(GateResult { label: label.into(), ok, detail, won_by: won_by.into() });
    };

    push(
        "containment: no route off the box",
        containment.contains("no route") && !containment.contains("REACHABLE"),
        containment.lines().next().unwrap_or("").trim().to_string(),
        "non-negotiable",
    );
    push(
        "content manifest served (real, not a stub)",
        manifest.map_or(false, |n| n > 1000),
        manifest.map_or("NOT SERVED".to_string(), |n| format!("{n}B")),
        "D1_MANIFEST_FILE + the unsigned-manifest lab patch",
    );
    push(
        "guest did not halt",
        !crashed_emu && !resp.contains("GUEST CRASH UPLOAD"),
        if crashed_emu { "CRASH - run d1 crash" } else { "clean" }.to_string(),
        "-",
    );
    push(
        "bootflow reached bap_signin",
        reached("bootflow:bap_signin"),
        String::new(),
        "package registration",
    );
    push(
        "bootflow reached investment_signin",
        reached("bootflow:investment_signin"),
        String::new(),
        "BAP handshake",
    );
    push(
        "signon 503 answered WITH A BODY",
        count_503 >= 1,
        format!("{count_503} emitted"),
        "D1_503_SOID=1 AND D1_503_RAW (the raw body is INERT without the SOID flag)",
    );
    push(
        "BAP conversation ran to the keepalive loop",
        bap_bytes.iter().max().map_or(false, |m| *m >= 600),
        match bap_bytes.iter().max() {
            Some(m) => format!(
                "max {m}B{}",
                if bap_alive { " - STILL OPEN, never closed" } else { "" }
            ),
            None => "no BAP frames at all".to_string(),
        },
        "reference run 20260807-130633 reached 678B / 13 messages",
    );
    push(
        "BAP channel crypto stayed in sync",
        gcm_failures == 0,
        format!("{gcm_failures} GCM failures"),
        "TCP frame reassembly: split on the declared length, never one-read-one-frame",
    );
    push(
        "investment message-206 answered with a span",
        resp.contains("message-206"),
        String::new(),
        "D1_206_BODY = the client's own object_f4 encoding",
    );
    push(
        "substate B predicate 1 account_ready TRUE",
        tally.p15_true > 0,
        format!("{}/{} true", tally.p15_true, tally.p15_total),
        "the QueueZ span landing",
    );

    let regressed = gates.iter().any(|g| !g.ok);
    GateReport {
        run: run_dir.display().to_string(),
        gates,
        tracer_armed: tracer_armed(&emu),
        trace_records: tally.total_records,
        regressed,
        probe_note: tally.note,
    }
}

/// Render the report exactly as the prototype printed it, so the two can be diffed byte for byte
/// over the same run. Byte parity is the port's verification instrument; do not restyle.
pub fn render_text(r: &GateReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("run: {}\n\n", r.run));
    for g in &r.gates {
        let mark = if g.ok { " PASS " } else { " FAIL " };
        out.push_str(&format!("[{mark}] {}", g.label));
        if !g.detail.is_empty() {
            out.push_str(&format!("   ({})", g.detail));
        }
        out.push('\n');
        if !g.ok {
            out.push_str(&format!("          won by: {}\n", g.won_by));
        }
    }
    out.push_str(&format!(
        "\ncontrol: tracer armed {} probes; {} trace records\n",
        r.tracer_armed, r.trace_records
    ));
    if r.regressed {
        out.push_str(
            "\n*** A PREVIOUSLY-PASSED GATE REGRESSED. Findings from this run are CONFOUNDED. ***\n",
        );
        // The section marker is kept byte-identical to the prototype for golden-diff parity.
        out.push_str("    Diff the inputs before theorising about the outputs (ATLAS \u{a7}4.4e).\n");
    }
    out
}

/// Controls: each returns Err with a reason on failure. Run as `#[test]`s and as `d1 controls`.
pub mod controls {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// An original synthetic fixture (6 records, all probe 15, 4 with AL set),
    /// generated through d1-trace make_record_bytes; no native input is included. It is data, not a format description:
    /// the layout is described in Rust only by `d1-trace/src/probe_decode.rs` (and historically by
    /// `capture-rig/decode-account-trace.py`, which pinned this fixture's known answer of 4/6
    /// true; the native decoder reproduces that answer, re-verified 2026-08-11).
    const TRACE_EXCERPT: &[u8] = include!("gate_synthetic.rs");

    /// The excerpt's decode, pinned as DATA, for this crate's own tests - which cannot call the
    /// real decoder, because `d1-trace` depends on `d1-run` and a dev-dependency would still be a
    /// cycle. It is a bytes-keyed double: it answers ONLY for the compiled-in excerpt and refuses
    /// every other input, so it can never silently accept a perturbed trace. Pinned from the
    /// canonical decoder over the excerpt on 2026-08-11: 6 records, all probe 15, sequences
    /// 53/59/93/102/107/123 after the decoder's sequence sort, AL set on the last four.
    ///
    /// The same controls also run with the REAL decoder injected - `d1 controls` and the `d1`
    /// crate's tests pass `native_probe_decode`, so the true end-to-end pipeline is exercised
    /// where the dependency direction allows it.
    pub fn pinned_excerpt_decode(blob: &[u8]) -> Result<Vec<ProbeSample>, String> {
        if blob != TRACE_EXCERPT {
            return Err("pinned-answer decoder: these bytes are not the compiled-in excerpt".into());
        }
        Ok(vec![
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a00 },
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a00 },
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a01 },
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a01 },
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a01 },
            ProbeSample { probe_id: 15, rax: 0xdeadbeef54321a01 },
        ])
    }

    /// Emulator log shaped like a real one, ANSI codes included so stripping is exercised.
    const EMU_OK: &str = concat!(
        "\u{1b}[0;37m[Tty] <Info> (job #0/4) logger.cpp:62 log_flush: [stdout] world_controller:state_manager: Entering state 'bootflow:bap_signin' for reason 'normal_flow'.\u{1b}[m\n",
        "[Tty] <Info> (job #0/4) log_flush: [stdout] world_controller:state_manager: Entering state 'bootflow:investment_signin' for reason 'normal_flow'.\n",
        "\u{1b}[0;37m[Debug] <Info> (shadPS4:Main) OnGameLoaded: D1 account tracer armed: 82 one-shot probes, output account-trace.bin\u{1b}[m\n",
    );

    const RESP_OK: &str = concat!(
        "MANIFEST from 10.0.0.2:53464, 52797B\n",
        "503 RAW BODY 123B\n",
        "BAP TCP msg#1 400B\n",
        "BAP TCP msg#2 278B\n",
        "peer closed after 678B\n",
        "message-206 span served\n",
    );

    const CONTAINMENT_OK: &str = "tcp-out:      no route\ndns:          no route\n";

    fn scratch(tag: &str, files: &[(&str, &[u8])]) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join(format!("d1-gate-ctl-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        for (name, bytes) in files {
            fs::write(dir.join(name), bytes).map_err(|e| format!("write {name}: {e}"))?;
        }
        Ok(dir)
    }

    fn gate<'a>(r: &'a GateReport, label: &str) -> Result<&'a GateResult, String> {
        r.gates
            .iter()
            .find(|g| g.label == label)
            .ok_or_else(|| format!("gate '{label}' missing from report"))
    }

    /// C-GATE-1: a run with every winning artifact present passes every gate, with the pinned
    /// details, including the synthetic fixture decoding to its known 4/6 answer.
    pub fn c_gate_all_pass(decode: ProbeDecoder) -> Result<(), String> {
        let dir = scratch(
            "pass",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", RESP_OK.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
                ("account-trace.bin", TRACE_EXCERPT),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        if let Some(n) = &r.probe_note {
            return Err(format!("probe lane errored: {n}"));
        }
        if r.regressed {
            return Err(format!("expected all gates PASS, got:\n{}", render_text(&r)));
        }
        for (label, want) in [
            ("content manifest served (real, not a stub)", "52797B"),
            ("BAP conversation ran to the keepalive loop", "max 678B"),
            ("substate B predicate 1 account_ready TRUE", "4/6 true"),
            ("signon 503 answered WITH A BODY", "1 emitted"),
            ("BAP channel crypto stayed in sync", "0 GCM failures"),
            ("containment: no route off the box", "tcp-out:      no route"),
        ] {
            let g = gate(&r, label)?;
            if g.detail != want {
                return Err(format!("{label}: detail '{}', wanted '{want}'", g.detail));
            }
        }
        if r.tracer_armed != "82" || r.trace_records != 6 {
            return Err(format!(
                "control line wrong: armed {} records {}",
                r.tracer_armed, r.trace_records
            ));
        }
        Ok(())
    }

    /// C-GATE-2 (inverted): remove the one 503 RAW BODY line and only that gate must flip, taking
    /// the whole report to regressed. A control that cannot fail is decoration.
    pub fn c_gate_503_inversion(decode: ProbeDecoder) -> Result<(), String> {
        let resp = RESP_OK.replace("503 RAW BODY 123B\n", "");
        let dir = scratch(
            "no503",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", resp.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
                ("account-trace.bin", TRACE_EXCERPT),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "signon 503 answered WITH A BODY")?;
        if g.ok || g.detail != "0 emitted" {
            return Err(format!("503 gate did not flip: ok={} detail='{}'", g.ok, g.detail));
        }
        if !r.regressed {
            return Err("report not marked regressed".into());
        }
        let failed: Vec<_> = r.gates.iter().filter(|g| !g.ok).collect();
        if failed.len() != 1 {
            return Err(format!("expected exactly one failing gate, got {}", failed.len()));
        }
        if !render_text(&r).contains("won by: D1_503_SOID=1 AND D1_503_RAW") {
            return Err("render lost the won-by line".into());
        }
        Ok(())
    }

    /// C-GATE-3: `REACHABLE` anywhere in the containment proof poisons it even when `no route` is
    /// also present - the prototype's AND, carried across.
    pub fn c_gate_containment_and(decode: ProbeDecoder) -> Result<(), String> {
        let dir = scratch(
            "reach",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", RESP_OK.as_bytes()),
                ("containment.txt", b"tcp-out:      no route\nudp-out:      REACHABLE\n"),
                ("account-trace.bin", TRACE_EXCERPT),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "containment: no route off the box")?;
        if g.ok {
            return Err("REACHABLE did not poison the containment gate".into());
        }
        Ok(())
    }

    /// C-GATE-4: the 2-byte stub manifest that cost a day fails the gate; the served real one is
    /// covered by C-GATE-1.
    pub fn c_gate_stub_manifest(decode: ProbeDecoder) -> Result<(), String> {
        let resp = RESP_OK.replace(", 52797B", ", 2B");
        let dir = scratch(
            "stub",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", resp.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "content manifest served (real, not a stub)")?;
        if g.ok || g.detail != "2B" {
            return Err(format!("stub manifest passed: ok={} detail='{}'", g.ok, g.detail));
        }
        Ok(())
    }

    /// C-GATE-5: the alive-connection trap. A BAP conversation that never closed must still PASS
    /// when the frame sizes sum past the threshold, and must say so; under the threshold it fails.
    /// This is the regression that fired on run 20260807-162724-subackhdr-1.
    pub fn c_gate_alive_connection(decode: ProbeDecoder) -> Result<(), String> {
        let alive = "BAP TCP msg#1 400B\nBAP TCP msg#2 278B\nMANIFEST from x, 52797B\n503 RAW BODY\nmessage-206\n";
        let dir = scratch(
            "alive",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", alive.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
                ("account-trace.bin", TRACE_EXCERPT),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "BAP conversation ran to the keepalive loop")?;
        if !g.ok || g.detail != "max 678B - STILL OPEN, never closed" {
            return Err(format!("alive connection scored wrong: ok={} detail='{}'", g.ok, g.detail));
        }

        // Inverted: same shape, too few bytes -> FAIL.
        let short = "BAP TCP msg#1 300B\nBAP TCP msg#2 200B\n";
        let dir = scratch("aliveshort", &[("responder.log", short.as_bytes())])?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "BAP conversation ran to the keepalive loop")?;
        if g.ok || g.detail != "max 500B - STILL OPEN, never closed" {
            return Err(format!("short alive connection passed: ok={} detail='{}'", g.ok, g.detail));
        }

        // And the true-empty case keeps its distinct wording.
        let dir = scratch("nobap", &[("responder.log", b"nothing here\n")])?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "BAP conversation ran to the keepalive loop")?;
        if g.ok || g.detail != "no BAP frames at all" {
            return Err(format!("empty BAP lane scored wrong: ok={} detail='{}'", g.ok, g.detail));
        }
        Ok(())
    }

    /// C-GATE-6 (inverted, trace half): truncate the real excerpt by one byte. The decoder must
    /// refuse it, the probe gate must fail loudly with a note, and the report must be regressed.
    /// Proves the output tracks the input bytes, not the file's presence. The second half pins
    /// the OTHER failure shape: a MISSING trace stays quiet (no note) and simply fails the gate
    /// 0/0 - the prototype's quiet empty state, preserved across the de-Python-ing.
    pub fn c_gate_truncated_trace(decode: ProbeDecoder) -> Result<(), String> {
        let truncated = &TRACE_EXCERPT[..TRACE_EXCERPT.len() - 1];
        let dir = scratch(
            "trunc",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", RESP_OK.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
                ("account-trace.bin", truncated),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "substate B predicate 1 account_ready TRUE")?;
        if g.ok || g.detail != "0/0 true" {
            return Err(format!("truncated trace passed: ok={} detail='{}'", g.ok, g.detail));
        }
        if r.probe_note.is_none() {
            return Err("decoder refusal was silent (no probe_note)".into());
        }
        if !r.regressed {
            return Err("report not marked regressed".into());
        }

        // Missing trace: quiet fail, NO note - a run whose tracer never armed is not an error in
        // the decoder, and the `tracer armed 0` control line is what makes it loud.
        let dir = scratch(
            "notrace",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", RESP_OK.as_bytes()),
                ("containment.txt", CONTAINMENT_OK.as_bytes()),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "substate B predicate 1 account_ready TRUE")?;
        if g.ok || g.detail != "0/0 true" {
            return Err(format!("missing trace scored wrong: ok={} detail='{}'", g.ok, g.detail));
        }
        if let Some(n) = &r.probe_note {
            return Err(format!("missing trace produced a spurious note: {n}"));
        }
        Ok(())
    }

    /// C-GATE-7: guest crash is fatal whichever side reported it - the emulator's crash_folder
    /// line or our responder's upload record. The detail keys off the emulator side only, exactly
    /// as the prototype did.
    pub fn c_gate_crash_either_side(decode: ProbeDecoder) -> Result<(), String> {
        let emu = format!("{EMU_OK}crash_folder written\n");
        let dir = scratch("crashemu", &[("emulator.log", emu.as_bytes())])?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "guest did not halt")?;
        if g.ok || g.detail != "CRASH - run d1 crash" {
            return Err(format!("emulator crash missed: ok={} detail='{}'", g.ok, g.detail));
        }

        let dir = scratch(
            "crashresp",
            &[
                ("emulator.log", EMU_OK.as_bytes()),
                ("responder.log", b"GUEST CRASH UPLOAD ticket_drop-1.bin\n"),
            ],
        )?;
        let r = gate_check(&dir, decode);
        let _ = fs::remove_dir_all(&dir);
        let g = gate(&r, "guest did not halt")?;
        // Prototype behaviour, kept: the responder-side upload fails the gate but the detail still
        // reads "clean" because the detail expression only consulted the emulator log.
        if g.ok || g.detail != "clean" {
            return Err(format!("responder-side crash missed: ok={} detail='{}'", g.ok, g.detail));
        }
        Ok(())
    }

    /// Every control, in one list, for the `d1 controls` subcommand and the test harness. Each
    /// takes the [`ProbeDecoder`] to run under: the CLI passes the native `d1-trace` decoder (the
    /// real pipeline), this crate's tests pass [`pinned_excerpt_decode`].
    pub fn all() -> Vec<(&'static str, fn(ProbeDecoder) -> Result<(), String>)> {
        vec![
            ("C-GATE-1 all winning artifacts pass, pinned details", c_gate_all_pass),
            ("C-GATE-2 removing the 503 body flips exactly that gate", c_gate_503_inversion),
            ("C-GATE-3 REACHABLE poisons containment despite no route", c_gate_containment_and),
            ("C-GATE-4 the 2-byte stub manifest fails", c_gate_stub_manifest),
            ("C-GATE-5 an alive BAP connection is not a regression", c_gate_alive_connection),
            ("C-GATE-6 a truncated trace is loud; a missing one is quiet", c_gate_truncated_trace),
            ("C-GATE-7 a crash on either side fails the run", c_gate_crash_either_side),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every control runs here under the pinned-answer double (this crate cannot name the real
    // decoder - see [`controls::pinned_excerpt_decode`]); the `d1` crate runs the same list again
    // with the native `d1-trace` decoder injected.
    #[test]
    fn c_gate_all_pass() {
        controls::c_gate_all_pass(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_503_inversion() {
        controls::c_gate_503_inversion(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_containment_and() {
        controls::c_gate_containment_and(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_stub_manifest() {
        controls::c_gate_stub_manifest(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_alive_connection() {
        controls::c_gate_alive_connection(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_truncated_trace() {
        controls::c_gate_truncated_trace(controls::pinned_excerpt_decode).unwrap();
    }
    #[test]
    fn c_gate_crash_either_side() {
        controls::c_gate_crash_either_side(controls::pinned_excerpt_decode).unwrap();
    }

    /// The pinned-answer double must refuse anything that is not the exact excerpt - otherwise the
    /// truncation control above would be testing nothing.
    #[test]
    fn pinned_decoder_refuses_perturbed_input() {
        assert!(controls::pinned_excerpt_decode(&[0u8; 232]).is_err());
    }

    /// Render parity spot check: the exact prototype line shapes, byte for byte.
    #[test]
    fn render_matches_prototype_shapes() {
        let r = GateReport {
            run: "runs/x".into(),
            gates: vec![
                GateResult {
                    label: "bootflow reached bap_signin".into(),
                    ok: true,
                    detail: String::new(),
                    won_by: "package registration".into(),
                },
                GateResult {
                    label: "signon 503 answered WITH A BODY".into(),
                    ok: false,
                    detail: "0 emitted".into(),
                    won_by: "D1_503_SOID=1 AND D1_503_RAW (the raw body is INERT without the SOID flag)".into(),
                },
            ],
            tracer_armed: "82".into(),
            trace_records: 6,
            regressed: true,
            probe_note: None,
        };
        let text = render_text(&r);
        assert!(text.starts_with("run: runs/x\n\n"));
        assert!(text.contains("[ PASS ] bootflow reached bap_signin\n"));
        assert!(text.contains("[ FAIL ] signon 503 answered WITH A BODY   (0 emitted)\n"));
        assert!(text.contains("          won by: D1_503_SOID=1"));
        assert!(text.contains("\ncontrol: tracer armed 82 probes; 6 trace records\n"));
        assert!(text.contains("*** A PREVIOUSLY-PASSED GATE REGRESSED."));
    }
}
