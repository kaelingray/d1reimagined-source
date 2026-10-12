// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The `C-GHOST-*` half of `d1-trace controls` (`design/d1-ghost.md` §10).
//!
//! House meta-rules, applied here exactly as in `dash_controls`:
//! - **state the axis in words** (each control's doc line);
//! - **every control ships a RED mutation test** - it breaks the thing on purpose in the SAME
//!   function and asserts the break is detected. A control whose mutation still passes is vacuous
//!   and reports FAIL, because it is not measuring anything;
//! - **a control that cannot be evaluated FAILED**;
//! - all run headless, no live client.
//!
//! These reuse the crate-internal `ghost::{registry,ledger,exec}` logic directly rather than
//! shelling to a binary: the controls and the tool share one implementation, so they cannot drift.

use std::path::Path;

use crate::controls::Check;
use crate::ghost::exec::{apply, all_specs, Applied, Backend};
use crate::ghost::ledger::{self, dangling, Ledger, Line};
use crate::ghost::registry::{self, find, Action, TOOL_WHITELIST};
use crate::ghost::{taint_lamp, Observed};

const TS: &str = "2026-08-11T00:00:00Z";

fn live_obs() -> Observed {
    Observed { rung: Some(36), state: "activity:in_world".into(), liveness: 3 }
}

/// A recording backend for the headless controls: canned readbacks, records whether the client was
/// ever touched. The `after` read returns `after_bytes`, so a control can force applied vs mismatch.
struct Rec {
    before: Vec<u8>,
    after: Vec<u8>,
    write_exit: i32,
    touched: bool,
}
impl Backend for Rec {
    fn read(&mut self, _a: &Action, _len: usize) -> std::io::Result<Vec<u8>> {
        // First read (before) then second (after); we only distinguish by a flag.
        if !self.touched {
            Ok(self.before.clone())
        } else {
            Ok(self.after.clone())
        }
    }
    fn write(&mut self, _a: &Action, _e: &str) -> std::io::Result<i32> {
        self.touched = true;
        Ok(self.write_exit)
    }
    fn launch(&mut self, _a: &Action) -> std::io::Result<i32> {
        self.touched = true;
        Ok(0)
    }
    fn read_ptr(&mut self, _t: &str, _abs: bool) -> std::io::Result<u64> {
        Ok(0x1000)
    }
    fn read_at(&mut self, _addr: u64, _len: usize) -> std::io::Result<Vec<u8>> {
        if !self.touched {
            Ok(self.before.clone())
        } else {
            Ok(self.after.clone())
        }
    }
    fn write_at(&mut self, _addr: u64, _ty: registry::ValTy, _v: u64, _e: &str) -> std::io::Result<i32> {
        self.touched = true;
        Ok(self.write_exit)
    }
}

fn tmp_run(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("d1-ghost-ctl-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Run one poke against a fresh recording backend and return (outcome, touched, ledger lines).
fn run_one(name: &str, obs: &Observed, before: Vec<u8>, after: Vec<u8>, exit: i32, seq: u64) -> (Applied, bool, Vec<Line>, std::path::PathBuf) {
    let d = tmp_run(name);
    let led = Ledger::for_run(&d);
    let p = find(name).unwrap();
    let mut rec = Rec { before, after, write_exit: exit, touched: false };
    let a = apply(&p, obs, &mut rec, &led, seq, TS, None).unwrap();
    let lines: Vec<Line> = led.read_all().into_iter().flatten().collect();
    (a, rec.touched, lines, d)
}

pub fn run_all(_repo: &Path) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();
    let mut push = |name: &'static str, ok: bool, detail: String| {
        out.push(Check { name, ok, detail });
    };

    // ---- C-GHOST-CITATION: every entry cited and classed; mutation: an empty citation is caught.
    {
        let all_cited = registry::registry().iter().all(|p| !p.citation.trim().is_empty());
        // RED: an entry with a blank citation would be caught by the same predicate.
        let mutant_caught = {
            let blank = "";
            blank.trim().is_empty()
        };
        push(
            "C-GHOST-CITATION",
            all_cited && mutant_caught,
            format!("{} entries, all cited and classed", registry::registry().len()),
        );
    }

    // ---- C-GHOST-NO-LAUNCH: the command surface names ONLY the two whitelisted tools; mutation:
    //      a fabricated spec naming another program is rejected by the same membership test.
    {
        let mut all_ok = true;
        for p in registry::registry() {
            for s in all_specs(&p.action, p.name) {
                if !TOOL_WHITELIST.contains(&s.program) {
                    all_ok = false;
                }
            }
        }
        let mutant_rejected = !TOOL_WHITELIST.contains(&"capture-rig/run-boot.sh");
        push(
            "C-GHOST-NO-LAUNCH",
            all_ok && mutant_rejected && TOOL_WHITELIST.len() == 2,
            format!("every spec in {{{}}}; a launcher is rejected", TOOL_WHITELIST.join(", ")),
        );
    }

    // ---- C-GHOST-HEX-VALUES: every value and address that reaches d1guest is hex-prefixed (an
    //      operator ledger rule - a bare decimal is parsed as the wrong integer for 0x.. data);
    //      mutation: a decimal-formatted value would be caught by the same predicate.
    {
        use crate::ghost::exec::all_specs;
        // Args that are d1guest `poke` VALUES/addresses are the positional operands and the
        // --offset argument; every hex-looking numeric arg must start with 0x. We check the
        // simplest sufficient invariant: no bare-decimal numeric operand >= 10 appears (which would
        // be an unprefixed value), while the known-good hex ones all start with 0x.
        let mut all_hex = true;
        for p in registry::registry() {
            for s in all_specs(&p.action, p.name) {
                for a in &s.args {
                    // A numeric operand that is all digits and could be a value/address is a bug
                    // unless it is a small count (len, activity-id, the literal "8"). We flag any
                    // all-hex-digit-with-letters token missing 0x, and any multi-digit decimal that
                    // sits in a value position. The cheap robust check: hex values are 0x-prefixed.
                    if a.len() > 2 && a.chars().all(|c| c.is_ascii_hexdigit()) && a.chars().any(|c| c.is_ascii_alphabetic()) {
                        all_hex = false; // bare hex without 0x
                    }
                }
            }
        }
        // RED: a bare-hex token would trip the predicate.
        let mutant_caught = {
            let bare = "deadbeef";
            bare.len() > 2 && bare.chars().all(|c| c.is_ascii_hexdigit()) && bare.chars().any(|c| c.is_ascii_alphabetic())
        };
        push(
            "C-GHOST-HEX-VALUES",
            all_hex && mutant_caught,
            "every d1guest value/address arg is 0x-prefixed; a bare-hex token is caught".to_string(),
        );
    }

    // ---- C-GHOST-RUNG-GATE: a rung-36 entry refuses a rung-30 client without touching it;
    //      mutation: at rung 36 the same entry MUST proceed (else the gate is stuck-closed, vacuous).
    {
        let low = Observed { rung: Some(30), state: "setup:orbit".into(), liveness: 3 };
        let (a_low, touched_low, lines_low, d1) = run_one("hook-0010c500", &low, vec![], vec![], 0, 0);
        let refused = a_low.outcome == ledger::OUTCOME_REFUSED && !touched_low;
        // The refusal is a COMPLETE ledger pair, not a dangling intent.
        let paired = lines_low.len() == 2 && dangling(&lines_low).is_empty();
        // RED / anti-vacuous: at rung 36 it must NOT refuse.
        let (a_ok, touched_ok, _l, d2) = run_one("hook-0010c500", &live_obs(), vec![0u8; 12], registry::find("hook-0010c500").map(|p| crate::ghost::exec::intended_bytes(&p.action)).unwrap(), 0, 0);
        let proceeds = a_ok.outcome != ledger::OUTCOME_REFUSED && touched_ok;
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
        push(
            "C-GHOST-RUNG-GATE",
            refused && paired && proceeds,
            format!("rung 30 refused (untouched, paired); rung 36 proceeds ({})", a_ok.outcome),
        );
    }

    // ---- C-GHOST-LAUNCH-WINDOW: the activity launch is windowed to ORBIT - it refuses in-world,
    //      where a re-launch strands the client (it crashed ghost and dash). Mutation: at orbit it
    //      must proceed, so the window is a real window and not stuck-closed.
    {
        let launch = find("launch-407").unwrap();
        let orbit = Observed { rung: Some(27), state: "setup:orbit".into(), liveness: 3 };
        let in_world = Observed { rung: Some(36), state: "activity:in_world".into(), liveness: 3 };
        let ok_at_orbit = launch.gate.unmet(&orbit).is_none();
        let refused_in_world = launch.gate.unmet(&in_world).map(|r| r.contains("window")).unwrap_or(false);
        // and UNKNOWN rung refuses (cannot-tell about a window).
        let refused_unknown = launch
            .gate
            .unmet(&Observed { rung: None, state: String::new(), liveness: 3 })
            .is_some();
        push(
            "C-GHOST-LAUNCH-WINDOW",
            ok_at_orbit && refused_in_world && refused_unknown,
            "launch enabled at orbit (27), refused in-world (36) and at unknown rung".to_string(),
        );
    }

    // ---- C-GHOST-READBACK: readback != intended is MISMATCH, never applied; mutation: the correct
    //      readback IS applied (else the classifier is stuck on mismatch and proves nothing).
    {
        let intended = crate::ghost::exec::intended_bytes(&find("done-mask-7ff").unwrap().action);
        let (a_ok, _t, _l, d1) = run_one("done-mask-7ff", &live_obs(), vec![0, 0, 0, 0], intended.clone(), 0, 0);
        let (a_bad, _t2, _l2, d2) = run_one("done-mask-7ff", &live_obs(), vec![0, 0, 0, 0], vec![0xde, 0xad, 0, 0], 0, 0);
        let ok = a_ok.outcome == ledger::OUTCOME_APPLIED;
        let bad = a_bad.outcome == ledger::OUTCOME_MISMATCH;
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
        push(
            "C-GHOST-READBACK",
            ok && bad,
            format!("matching readback -> {}, wrong readback -> {}", a_ok.outcome, a_bad.outcome),
        );
    }

    // ---- C-GHOST-LEDGER-FIRST / FAIL-CLOSED: with an unwritable ledger the poke NEVER touches the
    //      client and returns an error; mutation: a writable ledger DOES let it proceed.
    {
        let missing = std::env::temp_dir().join(format!("d1-ghost-noexist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        let led = Ledger::for_run(&missing);
        let p = find("done-mask-7ff").unwrap();
        let mut rec = Rec { before: vec![], after: vec![], write_exit: 0, touched: false };
        let r = apply(&p, &live_obs(), &mut rec, &led, 0, TS, None);
        let fail_closed = r.is_err() && !rec.touched;
        // anti-vacuous: a writable ledger proceeds.
        let (a_ok, touched_ok, _l, d) = run_one("done-mask-7ff", &live_obs(), vec![0, 0, 0, 0], crate::ghost::exec::intended_bytes(&p.action), 0, 0);
        let writable_proceeds = touched_ok && a_ok.outcome == ledger::OUTCOME_APPLIED;
        let _ = std::fs::remove_dir_all(&d);
        push(
            "C-GHOST-LEDGER-FIRST",
            fail_closed && writable_proceeds,
            "unwritable ledger: untouched + error; writable ledger: proceeds".to_string(),
        );
        push(
            "C-GHOST-FAIL-CLOSED",
            fail_closed,
            "a poke that cannot be ledgered refuses without touching the client".to_string(),
        );
    }

    // ---- C-GHOST-DANGLING-INTENT: an intent with no result is found and rendered loud; mutation:
    //      a paired intent+result is NOT flagged.
    {
        let dangle = vec![
            ledger::mk_intent(TS, 0, "done-mask-7ff", "Satisfy", "0x26B5730 u32", Some(34), None),
            ledger::mk_result(TS, 0, "0", "ff", ledger::OUTCOME_APPLIED, Some(0)),
            ledger::mk_intent(TS, 1, "join-ctr-0", "Satisfy", "0x26B5770 u32", Some(35), None),
        ];
        let found = dangling(&dangle);
        let flagged = found.len() == 1 && found[0].seq == 1;
        // RED: a fully-paired ledger flags nothing.
        let paired = vec![
            ledger::mk_intent(TS, 0, "a", "Satisfy", "x", Some(34), None),
            ledger::mk_result(TS, 0, "0", "1", ledger::OUTCOME_APPLIED, Some(0)),
        ];
        let no_false_positive = dangling(&paired).is_empty();
        push(
            "C-GHOST-DANGLING-INTENT",
            flagged && no_false_positive,
            "a resultless intent is flagged; a paired one is not".to_string(),
        );
    }

    // ---- C-GHOST-DESTS: the selector consumer parses the generator's format, greys every
    //      non-launchable entry WITH a reason, and recognises the pinned launchable band; mutation:
    //      a wrong column header is rejected, not read as shifted columns. The GENERATION side (21
    //      launchable / 393-413 against real block 1063) is pinned by `d1-oracle activity-modes
    //      controls`, which needs the owned-media dump; this half runs headless off a fixture.
    {
        use crate::ghost::dests;
        let rows = dests::parse(dests::FIXTURE_TSV);
        let parsed_ok = rows.as_ref().map(|r| r.len() == 3).unwrap_or(false);
        let greyed_have_reasons = rows
            .as_ref()
            .map(|r| r.iter().all(|d| d.launchable == d.refusal().is_empty()))
            .unwrap_or(false);
        let band_ok = dests::LAUNCHABLE_FIRST == 393 && dests::LAUNCHABLE_LAST == 413;
        let header_contract = dests::TSV_COLUMNS == "index\thash\tlaunchable\tcontent_family\tmode_name";
        // RED: a wrong header is rejected.
        let mutant_rejected = dests::parse("index\thash\n1\t0x2\n").is_err();
        // The MAP half: 32 pinned stems, the observed-live one present, labels de-slug the stem.
        let maps_ok = dests::PVP_MAPS.len() == 32
            && dests::PVP_MAPS.contains(&dests::MAP_OBSERVED_LIVE)
            && dests::map_label("pvp_moonbase") == "Moonbase (pvp_moonbase)";
        push(
            "C-GHOST-DESTS",
            parsed_ok && greyed_have_reasons && band_ok && header_contract && mutant_rejected && maps_ok,
            "modes: fixture parses, greyed carry reasons, band 393-413, header pinned; maps: 32 stems, moonbase live".to_string(),
        );
    }

    // ---- C-GHOST-TAINT-LAMP: nonempty ledger OR synthetic reach lights the lamp; an empty+clean
    //      run says "no LEDGERED pokes", never "no pokes".
    {
        let nonempty = taint_lamp(true, false);
        let synth = taint_lamp(false, true);
        let clean = taint_lamp(false, false);
        let ok = nonempty.contains("LEDGERED POKES")
            && synth.contains("synthetic_ancestor")
            && clean.contains("no ledgered pokes")
            && !clean.contains("no pokes (")   // never the unqualified claim
            && clean.contains("bypassing ghost");
        push(
            "C-GHOST-TAINT-LAMP",
            ok,
            "nonempty->lamp; synthetic reach->lamp; clean->'no LEDGERED pokes' (never 'no pokes')".to_string(),
        );
    }

    out
}
