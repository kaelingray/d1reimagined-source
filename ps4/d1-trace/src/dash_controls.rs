// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `C-DASH-*` - the dashboard's pinned invariants, evaluated inside `d1-trace controls`.
//! **Exit 2 = a control FAILED and the output may not be cited.**
//!
//! ## The meta-rules, which outrank the list
//!
//! 1. **State the axis in words before writing the control.** ⚠ A control that only ever passes is
//!    decoration. `capture-rig/eboot-callgraph.tsv` shipped 7/7 and re-tests at **4/11 FAIL**
//!    because all seven originals were direct calls - a control that did not test the axis it
//!    protected. Every entry below names its axis in its `detail` string.
//! 2. **Every control is itself unit-tested for its RED** (see the `tests` module at the bottom:
//!    each mutation breaks the thing on purpose and asserts the control goes red).
//! 3. **A control that cannot be evaluated is a control that FAILED** - never "skipped".
//! 4. Controls run against the same `Board` the CLI prints, not against the GUI, so they are cheap
//!    and headless.
//!
//! ## ⚠ Deviation from `historicaldocs/design/d1-dashboard.md` §7, stated rather than hidden
//!
//! The spec pins `runs/20260808-234535-CODEC-LIVE2`. **That run is still live** - the client is
//! running and `emulator.log` is still being appended to (measured 2026-08-09: 869,352 raw lines
//! and a *second* `error.*`, against the spec's 206,783-line snapshot and one error). A control
//! artifact must be **closed**, exactly as `controls::PINNED_RUN`'s own doc comment says.
//!
//! `runs/20260808-210910-AH-RAWCODEC` is pinned instead. It is closed (three hours old) and carries
//! **byte-for-byte the properties the spec measured**: 25 `Entering` lines, 22 distinct states in
//! the same order, furthest rung **30 `setup:activity_host_setup`**, last state `setup:orbit`
//! (rung 27), **exactly one** `error.*`, 15 KTOBUCKET lines, and `Provenance::quiet == No`.
//! `C-DASH-LADDER-PIN` is still red against the pre-existing code: `d1 show
//! 20260808-210910-AH-RAWCODEC` prints **`gate=prepare_for_orbit`** - rung 22 of the obsolete
//! 14-state spine - for this exact run.

use std::path::{Path, PathBuf};

use crate::controls::Check;
use crate::dash::ladder::{self, Ladder, Rung};
use crate::dash::present::{self, Board, Cell};
use crate::dash::probe::Descriptors;
use crate::dash::reduce::Reducer;
use crate::dash::{tail, Session};

/// The pinned dashboard artifact: **closed**, and deep enough to expose the 14-vs-37 rung bug.
pub const PINNED_DASH_RUN: &str = "runs/20260808-210910-AH-RAWCODEC";
/// A run whose only halt evidence is `uploads/ticket_drop-*.bin` - the path `d1_run` does not read.
pub const PINNED_UPLOAD_RUN: &str = "runs/20260808-172032-ACTIVITY-LADDER";
/// ⛔ The R1/R2 review artifact: a run that died TWICE at `bootflow:content_check` (rung 13) on
/// `error.content_manifest_not_provided`, printed ONLY on the entering form. The reviewed code read
/// it as "furthest rung 25 - cleanup · errors 0", in green. Both fix controls pin against it.
pub const PINNED_SIGNON_RUN: &str = "runs/20260806-140313-signon-f2";

/// The signon run's true depth: `bootflow:content_check`, rung 13. NOT 25.
const SIGNON_FURTHEST: u32 = 13;
const SIGNON_FURTHEST_STATE: &str = "bootflow:content_check";
/// Its two errors - both entering-form only, both `termite`.
const SIGNON_ERROR: &str = "error.content_manifest_not_provided";
const SIGNON_ERRORS: usize = 2;

/// `Entering state` lines in the pinned run.
const PIN_ENTERING: usize = 25;
/// Furthest rung in the pinned run.
const PIN_FURTHEST: u32 = 30;
const PIN_FURTHEST_STATE: &str = "setup:activity_host_setup";
/// The *last* state entered - deliberately shallower than the furthest, which is what makes
/// `C-DASH-FURTHEST-NOT-LATEST` a real test rather than a tautology.
const PIN_LAST_STATE: &str = "setup:orbit";
const PIN_ERROR: &str = "error.starting_private_activity_instance_timed_out_waiting_for_join_response";
/// The 22 distinct states, in first-entry order.
const PIN_STATES: [&str; 22] = [
    "bootflow:media_check",
    "bootflow:bootload",
    "bootflow:intro_screen_display",
    "bootflow:start",
    "bootflow:platform_signin",
    "bootflow:platform_account_warnings",
    "bootflow:enumerate_content_rights",
    "bootflow:account_signin",
    "bootflow:content_check",
    "bootflow:package_registration",
    "bootflow:bap_signin",
    "bootflow:investment_signin",
    "bootflow:profile_setup",
    "bootflow:prepare_for_orbit",
    "character:signin",
    "character:prologue",
    "cleanup",
    "setup:orbit_loading",
    "setup:orbit",
    "setup:activity_session_creation",
    "setup:matchmaking",
    "setup:activity_host_setup",
];
/// Ladder shape pinned from `d1-eboot ladder --json` (was state-machine.py before 2026-08-11).
const PIN_LADDER_ENTRIES: usize = 35;
const PIN_LADDER_MAX: u32 = 37;
/// Rung ids the client's own table does NOT carry. A property of the client, not a parse bug.
const PIN_LADDER_ABSENT: [u32; 2] = [15, 16];

fn check(name: &'static str, ok: bool, detail: String) -> Check {
    Check { name, ok, detail }
}

/// Evaluate every `C-DASH-*` control. `repo` is the repository root.
pub fn run_all(repo: &Path) -> Vec<Check> {
    let mut out = Vec::new();

    // ---- the ladder denominator, loaded once and shared -------------------------------------
    let l = ladder::load(repo);
    out.push(c_ladder_denom(&l));

    // ---- pinned-run controls ----------------------------------------------------------------
    let run = repo.join(PINNED_DASH_RUN);
    if !run.is_dir() {
        // Rule 3: a control that cannot be evaluated FAILED.
        out.push(check(
            "C-DASH-PINNED-RUN",
            false,
            format!(
                "{} is missing - every pinned dash control could NOT BE EVALUATED, which is a \
                 FAILURE, not a skip",
                run.display()
            ),
        ));
        return out;
    }
    let session = Session::open(&run);
    let board = session.board(None);
    out.extend(pinned_controls(&board, &l));
    out.push(c_determinism(&session));

    // ---- the R1/R2 review run -----------------------------------------------------------------
    let signon = repo.join(PINNED_SIGNON_RUN);
    if signon.is_dir() {
        let b = Session::open(&signon).board(None);
        out.extend(signon_controls(&b));
    } else {
        out.push(check(
            "C-DASH-SIGNON-RUN",
            false,
            format!(
                "{} is missing - C-DASH-CLEANUP-FLOOR and C-DASH-ERROR-BOTH-FORMS could NOT BE \
                 EVALUATED, which is a FAILURE, not a skip",
                signon.display()
            ),
        ));
    }

    // ---- fixture-backed controls ------------------------------------------------------------
    let tmp = std::env::temp_dir().join(format!("d1-dash-controls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    if std::fs::create_dir_all(&tmp).is_err() {
        out.push(check(
            "C-DASH-FIXTURES",
            false,
            format!("could not create {} - fixture controls NOT EVALUATED", tmp.display()),
        ));
        return out;
    }
    out.extend(fixture_controls(&tmp));
    out.push(c_halt_both_paths(repo, &tmp));
    let _ = std::fs::remove_dir_all(&tmp);

    out
}

// -------------------------------------------------------------------------------------------
// Pinned-run controls
// -------------------------------------------------------------------------------------------

/// Everything that can be asserted from the pinned board, so tests can drive it without I/O.
pub fn pinned_controls(b: &Board, l: &Ladder) -> Vec<Check> {
    let mut out = Vec::new();

    // ⭐ C-DASH-LADDER-PIN
    // AXIS: does the headline rung reflect the ACTIVITY ladder (37 rungs, from the client's own
    // RTTI), or the obsolete 14-state `d1_run::BOOTFLOW_SPINE`? `d1 show` prints
    // `gate=prepare_for_orbit` for this run - rung 22 of the old spine, 8 rungs short.
    let states: Vec<&str> = b.states_in_order.iter().map(|s| s.as_str()).collect();
    let entering = b.transitions.iter().filter(|t| t.entering).count();
    let furthest_ok = b
        .furthest
        .as_ref()
        .is_some_and(|f| f.rung == Some(PIN_FURTHEST) && f.state == PIN_FURTHEST_STATE);
    out.push(check(
        "C-DASH-LADDER-PIN",
        furthest_ok && states == PIN_STATES && entering == PIN_ENTERING,
        format!(
            "AXIS: activity ladder vs the obsolete 14-state bootflow spine. headline={} \
             (pinned rung {PIN_FURTHEST} {PIN_FURTHEST_STATE}); {} distinct states (pinned 22, \
             order-sensitive); {entering} Entering lines (pinned {PIN_ENTERING}). `d1 show` on this \
             same run prints gate=prepare_for_orbit.",
            b.furthest
                .as_ref()
                .map(|f| format!("{:?}/{}", f.rung, f.state))
                .unwrap_or_else(|| "NONE".into()),
            states.len()
        ),
    ));

    // C-DASH-FURTHEST-NOT-LATEST
    // AXIS: is depth `max()` or `last()`? A wrong-but-stable reading that a race control cannot
    // detect. The pinned run's LAST state is rung 27 while its furthest is 30.
    let last = b.current.as_ref().map(|c| c.state.clone()).unwrap_or_default();
    let last_rung = b.current.as_ref().and_then(|c| c.rung);
    out.push(check(
        "C-DASH-FURTHEST-NOT-LATEST",
        furthest_ok && last == PIN_LAST_STATE && last_rung == Some(27),
        format!(
            "AXIS: max() vs last(). furthest={:?} last={last} ({:?}) - the two DIFFER on this run, \
             so a last() implementation reads 27 and fails here",
            b.furthest.as_ref().and_then(|f| f.rung),
            last_rung
        ),
    ));

    // C-DASH-CLEANUP-TRANSIT
    // AXIS: is `cleanup` (rung 25) treated as failure, and does it cap depth? The pinned run enters
    // cleanup twice - once for normal_flow, once for the error - and still reports rung 30.
    let cleanup_normal = b
        .transitions
        .iter()
        .any(|t| t.entering && t.state == "cleanup" && t.reason == "normal_flow");
    let cleanup_not_error = b
        .transitions
        .iter()
        .filter(|t| t.state == "cleanup" && t.reason == "normal_flow")
        .all(|t| !t.is_error());
    out.push(check(
        "C-DASH-CLEANUP-TRANSIT",
        cleanup_normal && cleanup_not_error && furthest_ok,
        format!(
            "AXIS: cleanup is a NORMAL TRANSIT STATE at rung 25. normal_flow-cleanup present={} \
             rendered-as-failure={} depth-still-{:?}",
            cleanup_normal,
            !cleanup_not_error,
            b.furthest.as_ref().and_then(|f| f.rung)
        ),
    ));

    // C-DASH-ERROR-PIN
    // AXIS: does the error tape find real errors AND ONLY real errors? The negative half is what
    // stops a matcher that "matches everything" from passing.
    let n = b.errors.len();
    let exact = n == 1 && b.errors[0].reason == PIN_ERROR;
    let left_right = n == 1
        && b.errors[0].left_state == PIN_FURTHEST_STATE
        && b.errors[0].entered_state.as_deref() == Some("cleanup");
    let nonsense = b
        .errors
        .iter()
        .filter(|e| e.reason == "error.zzz_not_a_real_reason")
        .count();
    out.push(check(
        "C-DASH-ERROR-PIN",
        exact && left_right && nonsense == 0,
        format!(
            "AXIS: real errors, and only real errors. {n} error.* (pinned 1, string-equal={exact}, \
             left/entered correct={left_right}); nonsense reason matches {nonsense} (negative \
             control - a matcher that matches everything fails here)"
        ),
    ));

    // C-DASH-INSTRUMENT-FAULT (positive half; the fixture half is in `fixture_controls`)
    // AXIS: does a healthy run report `instrument_fault=false`? Without this the fixture half could
    // be satisfied by a function that always returns true.
    out.push(check(
        "C-DASH-INSTRUMENT-FAULT-NEG",
        !b.instrument_fault && b.guest_lines > 0,
        format!(
            "AXIS: positive control for the instrument-fault detector. pinned run \
             instrument_fault={} guest_lines={} (must be false / > 0, or the fixture half is \
             vacuous)",
            b.instrument_fault, b.guest_lines
        ),
    ));

    // C-DASH-GREY-NOT-ZERO
    // AXIS: does an unreadable instrument render DIFFERENTLY from a real zero? This project's
    // signature false negative is d1guest returning zeros, not an error, for a wrong address class.
    let all_grey = b
        .families
        .iter()
        .all(|r| r.state == Cell::NotObserved);
    let any_literal_zero = b
        .families
        .iter()
        .any(|r| r.value.trim() == "0" || r.value.trim() == "0.0");
    out.push(check(
        "C-DASH-GREY-NOT-ZERO",
        all_grey && !any_literal_zero && !b.families.is_empty(),
        format!(
            "AXIS: unreadable instrument vs a real zero. {} memory tiles with no live poll, \
             all NOT_OBSERVED={all_grey}, any rendering the literal 0={any_literal_zero}",
            b.families.len()
        ),
    ));

    // C-DASH-NO-MERGE
    // AXIS: does the dashboard ever assert a cross-lane order? (F1/F2.) Every ordinal on the board
    // is a within-lane line number, and no row carries a cross-source order key.
    let mono = |v: &[u64]| v.windows(2).all(|w| w[0] <= w[1]);
    let t_ord: Vec<u64> = b.transitions.iter().map(|t| t.line_no).collect();
    let e_ord: Vec<u64> = b.errors.iter().map(|e| e.line_no).collect();
    let k_ord: Vec<u64> = b.ktobucket.iter().map(|k| k.line_no).collect();
    let shuffled_is_rejected = !mono(&[9, 3, 7]); // positive control that the check CAN fail
    out.push(check(
        "C-DASH-NO-MERGE",
        mono(&t_ord) && mono(&e_ord) && mono(&k_ord) && shuffled_is_rejected,
        format!(
            "AXIS: no cross-lane order is asserted. within-lane ordinals monotonic: transitions={} \
             errors={} ktobucket={}; the monotonicity check itself rejects a shuffled fixture={}",
            mono(&t_ord),
            mono(&e_ord),
            mono(&k_ord),
            shuffled_is_rejected
        ),
    ));

    // C-DASH-STALE
    // AXIS: is a FROZEN source distinguishable from a QUIET one? sockets.log is
    // `for _ in $(seq 1 60); do ss -tuanp; sleep 5; done` - it stops after exactly 300 s and then
    // shows five-minute-old data forever, with no in-band marker.
    let sockets = b.lanes.iter().find(|l| l.file == "sockets.log");
    let emu = b.lanes.iter().find(|l| l.file == "emulator.log");
    let sock_stale = sockets.is_some_and(|l| l.stale);
    let emu_fresh = emu.is_some_and(|l| !l.stale);
    out.push(check(
        "C-DASH-STALE",
        sock_stale && emu_fresh,
        format!(
            "AXIS: frozen vs quiet. sockets.log behind={:?}s stale={sock_stale}; emulator.log \
             behind={:?}s stale={} (the newest lane in the run is the reference, so a CLOSED run \
             can still answer this)",
            sockets.and_then(|l| l.behind_secs),
            emu.and_then(|l| l.behind_secs),
            !emu_fresh
        ),
    ));

    // C-DASH-PROVENANCE-COMPLETE
    // AXIS: does EVERY displayed number carry a source? No exemptions, no Option::None.
    let rows = b.all_rows();
    let bad: Vec<&str> = rows
        .iter()
        .filter(|r| r.src.file.trim().is_empty() || r.src.line_shape.trim().is_empty())
        .map(|r| r.label.as_str())
        .collect();
    out.push(check(
        "C-DASH-PROVENANCE-COMPLETE",
        bad.is_empty() && !rows.is_empty(),
        format!(
            "AXIS: every number names its source. {} rows checked; {} without a file+line-shape \
             {bad:?}",
            rows.len(),
            bad.len()
        ),
    ));

    // C-DASH-QUIET
    // AXIS: is `Unknown` rendered as `Yes`? A determinism claim from a run with no host-quiet
    // evidence is [O], never [C].
    let quiet_row = b.header.iter().find(|r| r.label == "host quiet");
    let says_no = quiet_row.is_some_and(|r| r.value.contains("NO") && r.value.contains("loadavg-1"));
    let not_ok = quiet_row.is_some_and(|r| r.state != Cell::Ok);
    out.push(check(
        "C-DASH-QUIET",
        says_no && not_ok,
        format!(
            "AXIS: Unknown ≠ Yes, and No is loud. pinned run reads {:?} (state {:?})",
            quiet_row.map(|r| r.value.as_str()),
            quiet_row.map(|r| r.state)
        ),
    ));

    // C-DASH-KTOBUCKET-VERBATIM
    // AXIS: is the KTOBUCKET interior ever re-rendered as a typed field? It must not be - the
    // client's own printf emits visible garbage there ([qlen r/t 8/-273202064][type -48]).
    let kto_n = b.ktobucket.len();
    let carries_garbage = b
        .ktobucket
        .iter()
        .any(|k| k.text.contains("qlen r/t") && k.text.contains("-273202064"));
    let stamped = b
        .ktobucket_note
        .contains("AUTOPSY, NOT A HEARTBEAT");
    out.push(check(
        "C-DASH-KTOBUCKET",
        kto_n == 15 && carries_garbage && stamped,
        format!(
            "AXIS: verbatim, and stamped as an autopsy. {kto_n} lines (pinned 15); the client's own \
             printf garbage survives unparsed={carries_garbage}; stamped as an autopsy={stamped}"
        ),
    ));

    // C-DASH-AC-LAMPS
    // AXIS: is `CON-Y EST-N` - connected but never established - rendered as two different lamps?
    // A panel showing only "connected" reads the 2026-08-08 failure as success.
    let con = b.ac_rows.iter().find(|r| r.label.starts_with("CON"));
    let est = b.ac_rows.iter().find(|r| r.label.starts_with("EST"));
    let ah = b.ac_rows.iter().find(|r| r.label.starts_with("AH"));
    let split = con.map(|r| r.state) == Some(Cell::Ok) && est.map(|r| r.state) == Some(Cell::Empty);
    let ah_verbatim = ah.is_some_and(|r| r.value == "--------:--------");
    out.push(check(
        "C-DASH-AC-LAMPS",
        split && ah_verbatim,
        format!(
            "AXIS: CON and EST are separate lamps; AH is verbatim. CON={:?} EST={:?} (must differ) \
             AH={:?} (must NOT be collapsed to 0)",
            con.map(|r| r.state),
            est.map(|r| r.state),
            ah.map(|r| r.value.as_str())
        ),
    ));

    // C-DASH-THREE-STATE
    // AXIS: can the board express PASSED-EMPTILY, and does NOT-OBSERVED ever collapse to OK?
    let a = present::family_cell(Some(3), Some(4));
    let b2 = present::family_cell(Some(3), Some(0));
    let c = present::family_cell(None, None);
    let toks = [a.token(), b2.token(), c.token()];
    let distinct = toks[0] != toks[1] && toks[1] != toks[2] && toks[0] != toks[2];
    let never_ok = c != Cell::Ok && b2 != Cell::Ok;
    out.push(check(
        "C-DASH-THREE-STATE",
        distinct && a == Cell::Ok && b2 == Cell::Empty && c == Cell::NotObserved && never_ok,
        format!(
            "AXIS: three states, three tokens, and NOT_OBSERVED never becomes OK. \
             (state=3,count>0)={} (state=3,count=0)={} (nothing)={} distinct={distinct}",
            toks[0], toks[1], toks[2]
        ),
    ));

    // C-DASH-LADDER-SOURCE
    // AXIS: is the rung table shelled out to, or has someone inlined a copy?
    out.push(check(
        "C-DASH-LADDER-SOURCE",
        b.ladder_source.contains("d1-eboot") && b.rung_table.len() == l.rungs.len(),
        format!(
            "AXIS: shelled out, not transcribed. board source = {:?}; {} rungs on the board vs {} \
             from a fresh invocation",
            b.ladder_source,
            b.rung_table.len(),
            l.rungs.len()
        ),
    ));

    out
}

/// The R1/R2 fix controls, over the board of `PINNED_SIGNON_RUN`. Public so the RED tests can
/// drive them against a mutated board.
pub fn signon_controls(b: &Board) -> Vec<Check> {
    let mut out = Vec::new();

    // ⛔ C-DASH-CLEANUP-FLOOR (R1)
    // AXIS: does `cleanup` - ladder position 25 AND the universal failure sink - put a floor of 25
    // under exactly the runs that failed? This run died twice at rung 13; the reviewed code read
    // "furthest rung 25 - cleanup", in green. The premise half asserts the inflating transitions
    // are really present, so a run without them cannot satisfy this vacuously.
    let premise = b
        .transitions
        .iter()
        .filter(|t| t.entering && t.state == "cleanup" && t.is_error())
        .count();
    let floor_ok = b
        .furthest
        .as_ref()
        .is_some_and(|f| f.rung == Some(SIGNON_FURTHEST) && f.state == SIGNON_FURTHEST_STATE);
    out.push(check(
        "C-DASH-CLEANUP-FLOOR",
        floor_ok && premise == 2,
        format!(
            "AXIS: cleanup must never RAISE the headline. furthest={} (pinned rung \
             {SIGNON_FURTHEST} {SIGNON_FURTHEST_STATE}, NOT 25-cleanup); {premise} error-reason \
             cleanup entries present (pinned 2 - the premise that would have inflated it)",
            b.furthest
                .as_ref()
                .map(|f| format!("{:?}/{}", f.rung, f.state))
                .unwrap_or_else(|| "NONE".into()),
        ),
    ));

    // ⛔ C-DASH-ERROR-BOTH-FORMS (R2)
    // AXIS: is an error printed ONLY on the entering form visible? Eight measured runs rendered
    // "0 errors" over a named gate failure. This run has exactly two, both entering-form only, both
    // codename `termite` - and exactly two, so a fix that double-counts a Leaving/Entering pair
    // elsewhere still has to de-duplicate here.
    let n = b.errors.len();
    let all_pinned = b.errors.iter().all(|e| {
        e.reason == SIGNON_ERROR
            && e.codename == Some("termite")
            && e.left_state == SIGNON_FURTHEST_STATE
            && e.entered_state.as_deref() == Some("cleanup")
    });
    out.push(check(
        "C-DASH-ERROR-BOTH-FORMS",
        n == SIGNON_ERRORS && all_pinned,
        format!(
            "AXIS: errors on the entering form land on the tape, once each. {n} errors (pinned \
             {SIGNON_ERRORS}); all match reason={SIGNON_ERROR} codename=termite \
             left={SIGNON_FURTHEST_STATE} entered=cleanup: {all_pinned}"
        ),
    ));

    out
}

/// The R3 fix's assertion, factored so the RED test can drive it against a wrongly-stamped batch:
/// within-lane ordinals strictly increase and never approach the u64 wrap.
pub fn tail_ordinals_sane(transitions: &[crate::dash::reduce::Transition]) -> (bool, String) {
    let ords: Vec<u64> = transitions.iter().map(|t| t.line_no).collect();
    let strictly = ords.windows(2).all(|w| w[0] < w[1]);
    // A wrapped subtraction lands within `lines.len()` of u64::MAX - nowhere near any real log.
    let bounded = ords.iter().all(|o| *o < u64::MAX / 2);
    (
        strictly && bounded,
        format!(
            "{} ordinals, strictly-increasing={strictly}, none-wrapped={bounded}, last={:?}",
            ords.len(),
            ords.last()
        ),
    )
}

/// C-DASH-LADDER-DENOM
/// AXIS: has the 37-rung ladder drifted from `d1-eboot ladder`? Fails the day either
/// side changes alone.
fn c_ladder_denom(l: &Ladder) -> Check {
    if !l.source.is_available() {
        return check(
            "C-DASH-LADDER-DENOM",
            false,
            format!(
                "AXIS: ladder drift. The rung table could NOT BE LOADED ({}) - a control that \
                 cannot be evaluated is a FAILED control",
                l.source.describe()
            ),
        );
    }
    let n = l.rungs.len();
    let max = l.max_rung();
    let absent: Vec<u32> = (1..=PIN_LADDER_MAX)
        .filter(|r| !l.rungs.iter().any(|x| x.rung == *r))
        .collect();
    let ids_in_range = l.rungs.iter().all(|r| (1..=PIN_LADDER_MAX).contains(&r.rung));
    check(
        "C-DASH-LADDER-DENOM",
        n == PIN_LADDER_ENTRIES && max == PIN_LADDER_MAX && absent == PIN_LADDER_ABSENT && ids_in_range,
        format!(
            "AXIS: ladder drift. {n} entries (pinned {PIN_LADDER_ENTRIES}), max rung {max} (pinned \
             {PIN_LADDER_MAX}), absent {absent:?} (pinned {PIN_LADDER_ABSENT:?}), all ids in \
             range={ids_in_range}"
        ),
    )
}

/// C-DASH-DETERMINISM
/// AXIS: is `--json board` on a closed run byte-identical across two builds? A board that carries a
/// wall clock would fail here - which is why it does not.
fn c_determinism(session: &Session) -> Check {
    let a = serde_json::to_string(&session.board(None)).unwrap_or_default();
    let b = serde_json::to_string(&session.board(None)).unwrap_or_default();
    check(
        "C-DASH-DETERMINISM",
        !a.is_empty() && a == b,
        format!(
            "AXIS: no wall clock leaks into the board. two presentations of the same closed run \
             are byte-identical={} ({} bytes)",
            a == b,
            a.len()
        ),
    )
}

// -------------------------------------------------------------------------------------------
// Fixture-backed controls
// -------------------------------------------------------------------------------------------

fn fixture_controls(tmp: &Path) -> Vec<Check> {
    let mut out = Vec::new();

    // ⭐ C-DASH-INSTRUMENT-FAULT
    // AXIS: does zero guest narration render as an instrument fault, or as rung 0? The two-gate
    // diagnostics trap is silent by design: 626 of 1,614 sites pass the default filter and STILL
    // emit nothing, so a naive reading says "the game never got there".
    let d = tmp.join("no-stdout");
    let _ = std::fs::create_dir_all(&d);
    let mut spam = String::new();
    for i in 0..500 {
        spam.push_str(&format!(
            "[Render.Vulkan] <Info> renderer.cpp:{i} Skipped 3 duplicated messages\n"
        ));
    }
    let _ = std::fs::write(d.join("emulator.log"), &spam);
    let _ = std::fs::write(d.join("RUN-INFO.txt"), "run: no-stdout\n");
    let s = Session::open(&d);
    let b = s.board(None);
    out.push(check(
        "C-DASH-INSTRUMENT-FAULT",
        b.instrument_fault && b.furthest.is_none() && b.instrument_note.is_some(),
        format!(
            "AXIS: instrument fault vs rung 0. Vulkan spam with zero [stdout] lines ⇒ \
             instrument_fault={} furthest={:?} banner={}",
            b.instrument_fault,
            b.furthest.as_ref().map(|f| f.rung),
            b.instrument_note.is_some()
        ),
    ));

    // C-DASH-TAIL-SPLIT
    // AXIS: does the byte-offset tail lose or duplicate a line split across two polls? The oracle
    // is a one-shot read of the final file.
    let p = tmp.join("split.log");
    let _ = std::fs::write(&p, b"alpha\nbravo\ncharl");
    let mut t = tail::Tailer::new(&p);
    let mut got: Vec<String> = t.poll().lines;
    append(&p, b"ie\ndel");
    got.extend(t.poll().lines);
    append(&p, b"ta\n");
    got.extend(t.poll().lines);
    let oracle: Vec<String> = std::fs::read_to_string(&p)
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect();
    out.push(check(
        "C-DASH-TAIL-SPLIT",
        got == oracle && !oracle.is_empty(),
        format!(
            "AXIS: a line split across polls arrives exactly once. tail={got:?} vs one-shot \
             read={oracle:?}"
        ),
    ));

    // C-DASH-LOSSY-UTF8
    // AXIS: does a mid-multibyte snapshot silently zero a lane? (F3.) Assert the LINE COUNT, not
    // `> 0` - `> 0` is exactly the assertion that let F3 ship.
    let p = tmp.join("lossy.log");
    let _ = std::fs::write(&p, b"first\ntainted \xff line\nthird\n");
    let mut t = tail::Tailer::new(&p);
    let batch = t.poll();
    // …and the multibyte half: a `⭐` cut in two by the poll boundary must rejoin, not corrupt.
    let p2 = tmp.join("mb.log");
    let star = "⭐".as_bytes();
    let _ = std::fs::write(&p2, [b"pre ".as_slice(), &star[..1]].concat());
    let mut t2 = tail::Tailer::new(&p2);
    let first = t2.poll();
    append(&p2, &star[1..]);
    append(&p2, b" post\n");
    let second = t2.poll();
    out.push(check(
        "C-DASH-LOSSY-UTF8",
        batch.lines.len() == 3
            && batch.lossy
            && first.lines.is_empty()
            && second.lines == vec!["pre ⭐ post".to_string()]
            && !second.lossy,
        format!(
            "AXIS: one bad byte must not zero a lane, and a split character must not be corrupted. \
             kept {} lines (expect exactly 3), lossy-note={}; split ⭐ rejoined as {:?} lossy={}",
            batch.lines.len(),
            batch.lossy,
            second.lines,
            second.lossy
        ),
    ));

    // C-DASH-UTF8-STRIP
    // AXIS: do `⭐` and `-` survive ANSI stripping through the DASH ingest path? (F5 - a Latin-1
    // decode made the responder's highest-signal annotations permanently unmatchable.)
    let src = "\u{1b}[33m      ⭐ FAMILY 4 ANSWER seq 0 - answering the SUBSCRIPTION\u{1b}[0m";
    let got = crate::dash::reduce::strip_ansi(src);
    out.push(check(
        "C-DASH-UTF8-STRIP",
        got == "      ⭐ FAMILY 4 ANSWER seq 0 - answering the SUBSCRIPTION",
        format!("AXIS: UTF-8 survives ANSI stripping in the dash path. {got:?}"),
    ));

    // C-DASH-FURTHEST-SYNTHETIC
    // AXIS: the NEGATIVE fixture for max()-vs-last(): a ladder that reaches rung 30 and then
    // re-enters `bootflow:start` must still read 30, not 4.
    let l = fixture_ladder();
    let mut r = Reducer::new(&l);
    for (i, line) in [
        "world_controller:state_manager: Entering state 'setup:activity_host_setup' for reason 'normal_flow'.",
        "world_controller:state_manager: Entering state 'cleanup' for reason 'error.boom'.",
        "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
    ]
    .iter()
    .enumerate()
    {
        r.feed_guest_body(line, i as u64);
    }
    let f = r.furthest().map(|(n, _)| n);
    let last = r.current().and_then(|(n, _, _)| n);
    out.push(check(
        "C-DASH-FURTHEST-SYNTHETIC",
        f == Some(30) && last == Some(4),
        format!(
            "AXIS: max() survives a fallback to a SHALLOWER state. synthetic ladder reads \
             furthest={f:?} (expect 30) with last={last:?} (expect 4)"
        ),
    ));

    // C-DASH-CLEANUP-SYNTHETIC
    // AXIS: `cleanup` entered for `normal_flow` renders no failure and does not lower depth.
    let mut r = Reducer::new(&l);
    for (i, line) in [
        "world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.",
        "world_controller:state_manager: Entering state 'cleanup' for reason 'normal_flow'.",
    ]
    .iter()
    .enumerate()
    {
        r.feed_guest_body(line, i as u64);
    }
    out.push(check(
        "C-DASH-CLEANUP-SYNTHETIC",
        r.errors.is_empty() && r.furthest().map(|(n, _)| n) == Some(27),
        format!(
            "AXIS: cleanup for normal_flow is transit, not failure. errors={} furthest={:?} \
             (expect 0 / 27, NOT 25)",
            r.errors.len(),
            r.furthest().map(|(n, _)| n)
        ),
    ));

    // C-DASH-DESCRIPTORS-PARSE
    // AXIS: does the live-lane parser read the client's own table - and does it refuse prose that
    // merely mentions the same words?
    let d = crate::dash::probe::parse(DESCRIPTOR_SAMPLE);
    let neg = crate::dash::probe::parse("  the class=0 key=0x1 state=3 line with no slot bracket\n");
    out.push(check(
        "C-DASH-DESCRIPTORS-PARSE",
        d.rows.len() == 2
            && d.liveness_level == Some(3)
            && d.rows[1].ver == -1
            && neg.rows.is_empty(),
        format!(
            "AXIS: reads the real shape, and is not 'matches everything'. {} rows / L{:?} / ver={} \
             from the pinned sample; {} rows from prose (negative control)",
            d.rows.len(),
            d.liveness_level,
            d.rows.last().map(|r| r.ver).unwrap_or(0),
            neg.rows.len()
        ),
    ));

    // ⛔ C-DASH-TAIL-ORDINALS (R3)
    // AXIS: do live-tailed ordinals stay strictly increasing across TWO batches, absorbed the way
    // the GUI absorbs them (its own tailer, resumed from the session's load point)? The reviewed
    // code froze the counter at load time: tailed lines collided, went backwards, and on a SHORT
    // log - this fixture starts empty, the worst case - the subtraction wrapped u64 to ~1.8e19.
    let d = tmp.join("tail-ordinals");
    let _ = std::fs::create_dir_all(&d);
    let logp = d.join("emulator.log");
    let _ = std::fs::write(&logp, b"");
    let _ = std::fs::write(d.join("RUN-INFO.txt"), "run: tail-ordinals\n");
    let mut s = Session::open(&d);
    // The GUI's exact resume: a second tailer, from the session's load offset and count.
    let mut gui_tailer =
        tail::Tailer::resuming_at(&logp, s.emulator.offset(), s.emulator.next_line_no());
    append(
        &logp,
        b"x log_flush: [stdout] world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.\nspam\n",
    );
    s.absorb(&gui_tailer.poll());
    append(
        &logp,
        b"x log_flush: [stdout] world_controller:state_manager: Entering state 'setup:activity_host_setup' for reason 'normal_flow'.\n",
    );
    s.absorb(&gui_tailer.poll());
    let (sane, detail) = tail_ordinals_sane(&s.reducer.transitions);
    out.push(check(
        "C-DASH-TAIL-ORDINALS",
        sane && s.reducer.transitions.len() == 2,
        format!(
            "AXIS: two-batch monotonicity on the LIVE-TAILED path, from an empty log. {} \
             transitions (expect 2); {detail}",
            s.reducer.transitions.len()
        ),
    ));

    // C-DASH-GREY-VS-LIVE
    // AXIS: does a SUCCESSFUL live poll produce green tiles, so C-DASH-GREY-NOT-ZERO is not
    // vacuously satisfied by "the family table is always grey"?
    let live = Descriptors {
        rows: crate::dash::probe::parse(DESCRIPTOR_SAMPLE).rows,
        liveness: Vec::new(),
        liveness_level: Some(3),
        stamp: "00:10:58".into(),
        error: String::new(),
    };
    let d = tmp.join("grey-vs-live");
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("emulator.log"), "x [Tty] log_flush: [stdout] a:b: hi\n");
    let _ = std::fs::write(d.join("RUN-INFO.txt"), "run: grey\n");
    let s = Session::open(&d);
    let with = s.board(Some(&live));
    let without = s.board(None);
    let green = with.families.iter().filter(|r| r.state == Cell::Ok).count();
    let grey = without.families.iter().filter(|r| r.state == Cell::NotObserved).count();
    out.push(check(
        "C-DASH-GREY-VS-LIVE",
        green == 2 && grey == present::EXPECTED_FAMILY_CLASSES.len() && green > 0,
        format!(
            "AXIS: grey is not the only outcome. same run + a successful poll ⇒ {green} OK tiles; \
             without a poll ⇒ {grey} NOT_OBSERVED tiles"
        ),
    ));

    out
}

/// C-DASH-HALT-BOTH-PATHS
/// AXIS: does a crash whose ONLY evidence is `uploads/ticket_drop-*.bin` render as "no halt"?
/// `d1_run` reads only `guest-crash-report/crash_info.txt`, so `d1 show` reports
/// `guest_halt: None` over a real crash. **A green "no halt" over a crash is the worst cell on the
/// page.**
fn c_halt_both_paths(repo: &Path, tmp: &Path) -> Check {
    let run = repo.join(PINNED_UPLOAD_RUN);
    if !run.is_dir() {
        return check(
            "C-DASH-HALT-BOTH-PATHS",
            false,
            format!(
                "AXIS: both halt paths. {} is missing - NOT EVALUATED, which is a FAILURE",
                run.display()
            ),
        );
    }
    let has_uploads = run.join("uploads").is_dir();
    let has_legacy = run.join("guest-crash-report").join("crash_info.txt").is_file();
    let h = present::read_halt(&run);
    // Negative: a directory with neither must be NOT_OBSERVED, never Ok.
    let empty = tmp.join("no-halt");
    let _ = std::fs::create_dir_all(&empty);
    let h2 = present::read_halt(&empty);
    check(
        "C-DASH-HALT-BOTH-PATHS",
        h.state == Cell::Empty
            && has_uploads
            && !has_legacy
            && h.entries.iter().any(|e| e == "crash_info.txt")
            && h2.state == Cell::NotObserved,
        format!(
            "AXIS: the uploads/ path, which d1_run does not read. {} has uploads={has_uploads} \
             legacy-crash-report={has_legacy} ⇒ halt {:?} carrying {:?}; a run with neither ⇒ {:?} \
             (must be NOT_OBSERVED, not OK)",
            PINNED_UPLOAD_RUN,
            h.state,
            h.entries,
            h2.state
        ),
    )
}

const DESCRIPTOR_SAMPLE: &str = "\
LIVENESS: L3
  L0 PASS  pid 3423549 alive
00:10:58
  [  0] class=0   key=0xd100000000000001  state=3 REPLICATING (update accepted)  err=-  ver=0
  [254] class=5   key=0x7fffffffffffffff  state=3 REPLICATING (update accepted)  err=-  ver=-1
";

fn fixture_ladder() -> Ladder {
    let json = r#"[
        {"rung":4,"state":"bootflow:start"},
        {"rung":25,"state":"cleanup"},
        {"rung":27,"state":"setup:orbit"},
        {"rung":30,"state":"setup:activity_host_setup"}
    ]"#;
    let rungs: Vec<Rung> = serde_json::from_str(json).expect("fixture ladder");
    Ladder::from_rungs_for_tests(rungs)
}

fn append(p: &PathBuf, bytes: &[u8]) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = f.write_all(bytes);
    }
}

// -------------------------------------------------------------------------------------------
// Meta-rule 2: every control is unit-tested for its RED.
// -------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dash::present::Row;

    fn named(checks: &[Check], name: &str) -> bool {
        checks.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} missing")).ok
    }

    /// Build a board from a synthetic guest log, so the RED tests need no run directory.
    fn board_from(lines: &[&str], ladder: &Ladder, descriptors: Option<&Descriptors>) -> Board {
        let mut r = Reducer::new(ladder);
        r.emulator_bytes = 4096;
        for (i, l) in lines.iter().enumerate() {
            r.feed_guest_body(l, i as u64);
        }
        let dir = std::env::temp_dir().join(format!("d1-dash-red-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        present::build(&dir, ladder, &r, descriptors, Vec::new())
    }

    /// ⭐ THE RED TEST FOR C-DASH-LADDER-PIN - the mutation is the actual bug it protects against:
    /// reading depth off `d1_run::BOOTFLOW_SPINE`, which ends at `bootflow:prepare_for_orbit`.
    /// The client reached rung 30; the old spine can only ever say 22.
    #[test]
    fn ladder_pin_goes_red_against_the_obsolete_14_state_spine() {
        let spine: Vec<Rung> = d1_run::BOOTFLOW_SPINE
            .iter()
            .enumerate()
            .map(|(i, s)| Rung {
                rung: i as u32 + 1,
                state: (*s).to_string(),
                short: String::new(),
                class: String::new(),
            })
            .collect();
        let obsolete = Ladder::from_rungs_for_tests(spine);
        let lines: Vec<String> = PIN_STATES
            .iter()
            .map(|s| {
                format!("world_controller:state_manager: Entering state '{s}' for reason 'normal_flow'.")
            })
            .collect();
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let b = board_from(&refs, &obsolete, None);
        let checks = pinned_controls(&b, &obsolete);
        assert!(
            !named(&checks, "C-DASH-LADDER-PIN"),
            "the 14-state spine cannot reach rung 30 - this control MUST be red against it"
        );
    }

    /// …and green when the ladder is the real one and the run reached rung 30. Without this the
    /// test above would be satisfied by a control that is always red.
    #[test]
    fn ladder_pin_is_green_on_a_correct_board() {
        let l = full_fixture_ladder();
        let mut lines: Vec<String> = PIN_STATES
            .iter()
            .map(|s| {
                format!("world_controller:state_manager: Entering state '{s}' for reason 'normal_flow'.")
            })
            .collect();
        // The pinned run enters 25 states over 22 distinct ones, ending back at setup:orbit.
        lines.push("world_controller:state_manager: Leaving state 'setup:activity_host_setup' for reason 'error.starting_private_activity_instance_timed_out_waiting_for_join_response'.".into());
        lines.push("world_controller:state_manager: Entering state 'cleanup' for reason 'error.starting_private_activity_instance_timed_out_waiting_for_join_response'.".into());
        lines.push("world_controller:state_manager: Entering state 'setup:orbit_loading' for reason 'normal_flow'.".into());
        lines.push("world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.".into());
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let b = board_from(&refs, &l, None);
        let checks = pinned_controls(&b, &l);
        assert!(named(&checks, "C-DASH-LADDER-PIN"), "{:?}", checks[0].detail);
        assert!(named(&checks, "C-DASH-FURTHEST-NOT-LATEST"));
        assert!(named(&checks, "C-DASH-CLEANUP-TRANSIT"));
        assert!(named(&checks, "C-DASH-ERROR-PIN"));
    }

    /// RED for C-DASH-FURTHEST-NOT-LATEST: a `last()` implementation reports the shallow state.
    #[test]
    fn furthest_not_latest_goes_red_for_a_last_implementation() {
        let l = full_fixture_ladder();
        let mut r = Reducer::new(&l);
        r.emulator_bytes = 1;
        for (i, s) in ["setup:activity_host_setup", "setup:orbit"].iter().enumerate() {
            r.feed_guest_body(
                &format!("world_controller:state_manager: Entering state '{s}' for reason 'normal_flow'."),
                i as u64,
            );
        }
        // A `last()` reading would be rung 27; the control demands 30 AND a last of 27.
        assert_eq!(r.furthest().map(|(n, _)| n), Some(30));
        assert_eq!(r.current().and_then(|(n, _, _)| n), Some(27));
        // Mutate: drop the deep entry, so max() == last() and the control's premise collapses.
        let mut r2 = Reducer::new(&l);
        r2.feed_guest_body(
            "world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.",
            0,
        );
        let dir = std::env::temp_dir().join(format!("d1-dash-red2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let b = present::build(&dir, &l, &r2, None, Vec::new());
        assert!(!named(&pinned_controls(&b, &l), "C-DASH-FURTHEST-NOT-LATEST"));
    }

    /// RED for C-DASH-ERROR-PIN's negative half: a matcher that treats every leaving-reason as an
    /// error would report far more than one and fail.
    #[test]
    fn error_pin_goes_red_when_the_matcher_matches_everything() {
        let l = full_fixture_ladder();
        let mut r = Reducer::new(&l);
        for (i, s) in PIN_STATES.iter().enumerate() {
            r.feed_guest_body(
                &format!("world_controller:state_manager: Entering state '{s}' for reason 'normal_flow'."),
                i as u64,
            );
        }
        // Two errors instead of one.
        for (i, s) in ["setup:matchmaking", "setup:activity_host_setup"].iter().enumerate() {
            r.feed_guest_body(
                &format!("world_controller:state_manager: Leaving state '{s}' for reason '{PIN_ERROR}'."),
                100 + i as u64,
            );
        }
        let dir = std::env::temp_dir().join(format!("d1-dash-red3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let b = present::build(&dir, &l, &r, None, Vec::new());
        assert!(!named(&pinned_controls(&b, &l), "C-DASH-ERROR-PIN"));
    }

    /// ⛔ RED for C-DASH-CLEANUP-FLOOR: the mutation is the reviewed bug itself - a `furthest` that
    /// counts `cleanup` reads rung 25 on the signon-f2 shape and must go red.
    #[test]
    fn cleanup_floor_goes_red_against_the_inflated_headline() {
        let l = full_fixture_ladder();
        let b = board_from(&signon_f2_shape(), &l, None);
        assert!(
            named(&signon_controls(&b), "C-DASH-CLEANUP-FLOOR"),
            "the fixed reducer must read rung 13 on this shape: {:?}",
            b.furthest
        );
        // Mutate: the old computation's answer.
        let mut bad = b.clone();
        bad.furthest = Some(crate::dash::present::RungRef {
            rung: Some(25),
            state: "cleanup".into(),
            reason: None,
        });
        assert!(!named(&signon_controls(&bad), "C-DASH-CLEANUP-FLOOR"));
    }

    /// ⛔ RED for C-DASH-ERROR-BOTH-FORMS: the reviewed code pushed errors only in the `!entering`
    /// branch, so this run's tape was empty; and a fix that forgets to de-dup would double-count.
    #[test]
    fn error_both_forms_goes_red_for_empty_and_for_double_counted() {
        let l = full_fixture_ladder();
        let b = board_from(&signon_f2_shape(), &l, None);
        assert!(
            named(&signon_controls(&b), "C-DASH-ERROR-BOTH-FORMS"),
            "the fixed tape must carry exactly 2 errors: got {}",
            b.errors.len()
        );
        // Mutation 1: the old behaviour - no errors at all.
        let mut empty = b.clone();
        empty.errors.clear();
        assert!(!named(&signon_controls(&empty), "C-DASH-ERROR-BOTH-FORMS"));
        // Mutation 2: a de-dup failure - every error counted twice.
        let mut doubled = b.clone();
        let dup = doubled.errors.clone();
        doubled.errors.extend(dup);
        assert!(!named(&signon_controls(&doubled), "C-DASH-ERROR-BOTH-FORMS"));
    }

    /// ⛔ RED for C-DASH-TAIL-ORDINALS: a batch stamped the frozen-counter way (underflowed near
    /// u64::MAX, and colliding) fails the sanity check the control runs.
    #[test]
    fn tail_ordinals_go_red_for_the_frozen_counter() {
        let l = full_fixture_ladder();
        let mut r = Reducer::new(&l);
        // The old caller on an empty-at-load log: next_line_no()=0, batch of 2 lines,
        // 0 - 2 wraps to u64::MAX - 1.
        let wrapped = 0u64.wrapping_sub(2);
        r.feed_guest_body(
            "world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.",
            wrapped,
        );
        r.feed_guest_body(
            "world_controller:state_manager: Entering state 'setup:activity_host_setup' for reason 'normal_flow'.",
            wrapped.wrapping_add(1),
        );
        let (sane, detail) = tail_ordinals_sane(&r.transitions);
        assert!(!sane, "a wrapped ordinal must fail the check: {detail}");
        // And plain collision (two batches given the same start) fails too.
        let mut r2 = Reducer::new(&l);
        for _ in 0..2 {
            r2.feed_guest_body(
                "world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.",
                7,
            );
        }
        let (sane2, _) = tail_ordinals_sane(&r2.transitions);
        assert!(!sane2, "colliding ordinals must fail the check");
    }

    /// The guest-line shape of `runs/20260806-140313-signon-f2`, reduced to its ladder skeleton:
    /// two content_check attempts, each dying into cleanup on the ENTERING form only.
    fn signon_f2_shape() -> Vec<&'static str> {
        vec![
            "world_controller:state_manager: Entering state 'bootflow:media_check' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'bootflow:account_signin' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'bootflow:content_check' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'cleanup' for reason 'error.content_manifest_not_provided'.",
            "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'bootflow:account_signin' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'bootflow:content_check' for reason 'normal_flow'.",
            "world_controller:state_manager: Entering state 'cleanup' for reason 'error.content_manifest_not_provided'.",
            "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
        ]
    }

    /// RED for C-DASH-GREY-NOT-ZERO: a family table that rendered a literal `0` for an unread
    /// instrument is the exact false negative this control exists to catch.
    #[test]
    fn grey_not_zero_goes_red_when_an_unread_tile_renders_zero() {
        let l = full_fixture_ladder();
        let mut b = board_from(&[], &l, None);
        assert!(named(&pinned_controls(&b, &l), "C-DASH-GREY-NOT-ZERO"));
        b.families = vec![Row::new(
            "family class 0",
            "0",
            Cell::NotObserved,
            crate::dash::present::Src::derived("x", "y"),
        )];
        assert!(!named(&pinned_controls(&b, &l), "C-DASH-GREY-NOT-ZERO"));
    }

    /// RED for C-DASH-PROVENANCE-COMPLETE: one row with an empty source fails the whole board.
    #[test]
    fn provenance_complete_goes_red_for_a_single_sourceless_row() {
        let l = full_fixture_ladder();
        let mut b = board_from(&[], &l, None);
        assert!(named(&pinned_controls(&b, &l), "C-DASH-PROVENANCE-COMPLETE"));
        b.header.push(Row::new(
            "mystery number",
            "42",
            Cell::Ok,
            crate::dash::present::Src::derived("", ""),
        ));
        assert!(!named(&pinned_controls(&b, &l), "C-DASH-PROVENANCE-COMPLETE"));
    }

    /// RED for C-DASH-LADDER-DENOM: an unavailable ladder is a FAILED control, not a skipped one.
    #[test]
    fn ladder_denom_fails_when_it_cannot_be_evaluated() {
        assert!(!c_ladder_denom(&Ladder::unavailable("no python")).ok);
        assert!(!c_ladder_denom(&full_fixture_ladder()).ok, "a 4-rung fixture is not the real table");
    }

    /// RED for C-DASH-THREE-STATE: collapsing PASSED_EMPTILY into OK.
    #[test]
    fn three_state_rule_never_maps_not_observed_to_ok() {
        assert_eq!(present::family_cell(None, None), Cell::NotObserved);
        assert_eq!(present::family_cell(Some(3), Some(0)), Cell::Empty);
        assert_eq!(present::family_cell(Some(2), None), Cell::Empty);
        assert_eq!(present::family_cell(Some(3), Some(1)), Cell::Ok);
    }

    /// The full 37-rung fixture, matching the real table's rung ids for the states the pinned run
    /// visits. Used only where shelling out would make a unit test depend on python.
    fn full_fixture_ladder() -> Ladder {
        let pairs: [(u32, &str); 22] = [
            (1, "bootflow:media_check"),
            (2, "bootflow:bootload"),
            (3, "bootflow:intro_screen_display"),
            (4, "bootflow:start"),
            (5, "bootflow:platform_signin"),
            (6, "bootflow:platform_account_warnings"),
            (7, "bootflow:enumerate_content_rights"),
            (8, "bootflow:account_signin"),
            (13, "bootflow:content_check"),
            (18, "bootflow:package_registration"),
            (19, "bootflow:bap_signin"),
            (20, "bootflow:investment_signin"),
            (21, "bootflow:profile_setup"),
            (22, "bootflow:prepare_for_orbit"),
            (23, "character:signin"),
            (24, "character:prologue"),
            (25, "cleanup"),
            (26, "setup:orbit_loading"),
            (27, "setup:orbit"),
            (28, "setup:activity_session_creation"),
            (29, "setup:matchmaking"),
            (30, "setup:activity_host_setup"),
        ];
        Ladder::from_rungs_for_tests(
            pairs
                .iter()
                .map(|(r, s)| Rung {
                    rung: *r,
                    state: (*s).to_string(),
                    short: String::new(),
                    class: String::new(),
                })
                .collect(),
        )
    }
}
