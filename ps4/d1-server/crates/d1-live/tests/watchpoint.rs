// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The live controls, as `#[test]`s.
//!
//! ⛔ **These never touch the emulator.** Every one of them spawns a throwaway child of this
//! crate's own binary (`__writer`), watches that, and kills it. `cargo test -p d1-live` is safe to
//! run while the operator is driving shadPS4.
//!
//! They are integration tests rather than unit tests for one reason: `CARGO_BIN_EXE_d1-live` is the
//! only supported way to get the path of the crate's own binary, and the control *needs* a separate
//! process - a `fork()` inside a threaded test harness is exactly the kind of cleverness that makes
//! a control fail for reasons unrelated to its axis.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use d1_live::controls;
use d1_live::ptrace;

fn helper() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_d1-live"))
}

/// ⚠ `cargo test` runs a binary's tests as THREADS OF ONE PROCESS, and `Watcher::run` polls
/// `waitpid(-1, __WALL)` - which is process-wide. Two tests running at once therefore steal each
/// other's `SIGTRAP` notifications (a false "no hit") **and each other's child exit statuses** (a
/// `Command::output()` that never gets its own result). Both were observed here, at roughly one run
/// in six. This is a property of ptrace, not a flaw in the controls, and it is exactly the "silent
/// false negative" shape this project keeps paying for - so every test that spawns a child
/// serialises, rather than being marked flaky. (`Watcher::run` counts stray stops as
/// `foreign_stops` for the same reason.)
static TRACER: Mutex<()> = Mutex::new(());

fn one_tracer_at_a_time() -> MutexGuard<'static, ()> {
    TRACER.lock().unwrap_or_else(|e| e.into_inner())
}

/// The axis: `offsetof(struct user, u_debugreg)` is measured against a real stopped task, and the
/// measurement is bounded on BOTH sides, so a kernel that accepted every offset would fail.
#[test]
fn debugreg_offset_is_measured_and_bounded() {
    let _tracer = one_tracer_at_a_time();
    let (off, detail) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    assert_eq!(off, ptrace::DEBUGREG_OFFSET, "{detail}");
    assert!(detail.contains("rejects"), "the derivation must state both rejections: {detail}");
}

/// ⭐ The axis: **a write watchpoint catches the exact instruction that wrote a chosen address.**
///
/// The child reports its own `mov qword ptr [reg], reg` address out of inline-asm labels, so the
/// expected answer is not transcribed from a disassembly. A data breakpoint is a *trap*, so the
/// caught `RIP` must be the byte after the store - an instrument that reported the store address
/// itself, or one instruction too far, fails here.
#[test]
fn watchpoint_catches_the_exact_writer_instruction() {
    let _tracer = one_tracer_at_a_time();
    let (dbg, _) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    let detail = controls::catches_the_writer(helper(), dbg).expect("positive control");
    assert!(detail.contains("=="), "{detail}");
}

/// ⭐ The INVERSION. The axis: the watchpoint is about the **address**, not "traps always fire".
///
/// A decoy 16 bytes from the target, in the same allocation, on a child that is storing thousands
/// of times per millisecond, must produce **no** trap - and then the very same child, still
/// running, must trip the real target. Without that second leg the silence would be explained just
/// as well by a child that had stopped working.
#[test]
fn watchpoint_is_address_specific_and_the_silence_is_meaningful() {
    let _tracer = one_tracer_at_a_time();
    let (dbg, _) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    let detail = controls::address_specific(helper(), dbg).expect("inverted control");
    assert!(detail.contains("0 hits"), "{detail}");
}

/// The axis: `Watcher`'s disarm path really removes the breakpoint - and whether that matters is
/// itself measured, by abandoning an armed target and seeing whether it dies.
#[test]
fn dropping_the_watcher_disarms_and_the_target_survives() {
    let _tracer = one_tracer_at_a_time();
    let (dbg, _) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    let detail = controls::disarm_is_load_bearing(helper(), dbg).expect("disarm control");
    // Either world is acceptable; an ambiguous one is not.
    assert!(
        detail.contains("SAFETY FINDING") || detail.contains("DEGENERATE INVERSION"),
        "{detail}"
    );
}

/// ⭐ The axis: **the capture window runs with nothing armed**, and the zero that says so is paired
/// with a leg that reads the armed value through the same instrument.
#[test]
fn the_capture_window_runs_with_no_debug_register_set() {
    let _tracer = one_tracer_at_a_time();
    let (dbg, _) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    let detail = controls::capture_runs_unarmed(helper(), dbg).expect("capture-runs-unarmed");
    assert!(detail.contains("CLEARED, not never-armed"), "{detail}");
}

/// ⭐⭐ The axis this whole build is arranged around: **a crash during the capture cannot strand an
/// armed tracer.** The helper panics on purpose, mid-capture, with the debug registers already
/// cleared; the child must be alive, unarmed and still storing.
#[test]
fn a_panic_during_the_capture_leaves_the_child_alive_and_unarmed() {
    let _tracer = one_tracer_at_a_time();
    let (dbg, _) = controls::debugreg_offset(helper()).expect("debugreg offset control");
    let detail =
        controls::panic_during_capture_is_survivable(helper(), dbg).expect("panic-during-capture");
    // Either world is acceptable; an ambiguous one is not.
    assert!(
        detail.contains("PANIC DURING THE CAPTURE DOES NOT")
            || detail.contains("DEGENERATE INVERSION"),
        "{detail}"
    );
    assert!(detail.contains("DR7 read") || detail.contains("DR7 = 0"), "{detail}");
}

/// The whole suite, exactly as `d1-live controls` runs it - so a control that only passes when
/// called directly cannot hide.
#[test]
fn the_controls_subcommand_passes_as_a_whole() {
    let _tracer = one_tracer_at_a_time();
    let c = controls::run_all(helper());
    assert!(c.ok(), "{}", c.report());
    // ⭐ The count is PINNED on purpose: a control that is silently dropped from `run_all` fails
    // here rather than shrinking the suite unnoticed. 5 since 2026-08-10 (`registers-at-the-hit`);
    // 7 since the capture-at-the-stop build (`capture-runs-unarmed`,
    // `panic-during-capture-is-survivable`); 10 since the armed-window safety build
    // (`fatal-signals-disarm`, `a-failed-arm-is-still-disarmed`,
    // `disarm-is-bounded-and-multithreaded`); 17 since the READ-SIDE port of
    // `capture-rig/d1guest.py read|obj|tag|descriptors`, whose seven controls drive a fixture
    // process rather than the emulator; 18 since the adversarial pass added
    // `garbage-in-refuses-loudly`.
    //
    // ⚠ If a client is up, the read-side leg collapses to ONE failing row that says so, and the
    // count is 11 rather than 18. That is deliberate: the suite must not silently shrink, and a
    // read side that could not be certified must say it out loud.
    assert_eq!(c.lines.len(), 18, "{}", c.report());
    for name in [
        "registers-at-the-hit",
        "capture-runs-unarmed",
        "panic-during-capture-is-survivable",
        "fatal-signals-disarm",
        "a-failed-arm-is-still-disarmed",
        "disarm-is-bounded-and-multithreaded",
        "read-side-base-and-read",
        "wrong-image-fails-the-verify",
        "two-anchors-need-exactly-one-base",
        "registry-cross-check-refuses",
        "tag-resolves-through-the-back-offset",
        "descriptors-decode",
        "read-after-death-is-an-error",
    ] {
        assert!(
            c.lines.iter().any(|l| l.name == name),
            "{name} is not in the suite: {}",
            c.report()
        );
    }
}

/// ⭐ The axis: **a flag that exists elsewhere must be REFUSED with the command that owns it**, not
/// rejected as unknown. "unexpected argument" reads as *the feature does not exist*, and that is how
/// this project keeps re-implementing what it already has (CLAUDE.md's signature failure).
#[test]
fn snapshot_on_hit_is_refused_by_name_not_by_the_parser() {
    let out = std::process::Command::new(helper())
        .args([
            "watch",
            "--pid",
            "0",
            "--addr",
            "a:0x1000",
            "--no-base",
            "--snapshot-on-hit",
            "a:0x1000",
        ])
        .output()
        .expect("running d1-live watch --snapshot-on-hit");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("d1-replay capture"), "the refusal must name the real command: {err}");
    assert!(!err.contains("unexpected argument"), "{err}");
}

/// The axis this protects is the *exit code contract*: `controls` must exit **2**, not 1, when a
/// control fails - the project's "do not cite this output" signal. Asserted by running the real
/// binary, because the contract lives in `main`, not in the library.
#[test]
fn controls_subcommand_exits_zero_when_it_passes() {
    let _tracer = one_tracer_at_a_time();
    let out = std::process::Command::new(helper())
        .arg("controls")
        .output()
        .expect("running d1-live controls");
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
}

/// A watch on a process that does not exist must be an error, not a silent zero-hit "finding".
#[test]
fn a_missing_target_is_an_error_not_an_empty_result() {
    let _tracer = one_tracer_at_a_time();
    // pid 0 is never a watchable task; `--no-base` keeps the eboot out of it.
    let out = std::process::Command::new(helper())
        .args(["watch", "--pid", "0", "--addr", "a:0x1000", "--no-base", "--max-seconds", "1"])
        .output()
        .expect("running d1-live watch");
    assert_ne!(out.status.code(), Some(0));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("d1-live:"), "{err}");
}

/// The alignment refusal, end to end: the CLI must reject rather than quietly watch other bytes.
#[test]
fn an_unaligned_watch_is_refused_by_the_cli() {
    let _tracer = one_tracer_at_a_time();
    let s = controls::spawn_writer(helper(), 5.0).expect("control child");
    let unaligned = s.info.target + 4;
    let out = std::process::Command::new(helper())
        .args([
            "watch",
            "--pid",
            &s.info.pid.to_string(),
            "--addr",
            &format!("a:{:#x}", unaligned),
            "--no-base",
            "--len",
            "8",
            "--max-seconds",
            "1",
        ])
        .output()
        .expect("running d1-live watch");
    assert_ne!(out.status.code(), Some(0));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("aligned"), "{err}");
    drop(s);
    std::thread::sleep(Duration::from_millis(10));
}
