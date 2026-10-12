// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! End-to-end over the real binary, because the exit codes are part of the contract: an operator
//! and a script both read them, and a tool that prints REFUSED while exiting 0 is worse than one
//! that says nothing.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_d1-harness"))
}

#[test]
fn layout_is_valid_json_and_byte_identical_across_invocations() {
    let a = bin().args(["layout"]).output().unwrap();
    let b = bin().args(["layout"]).output().unwrap();
    assert!(a.status.success());
    assert_eq!(a.stdout, b.stdout, "two invocations of one seed disagreed");
    let v: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(v["participants"].as_array().unwrap().len(), 2);
    assert_eq!(v["activation"], "dry_run_only");
    assert_eq!(v["guards"]["uninstrumented_control"], serde_json::Value::Null);
}

#[test]
fn a_second_seed_moves_the_run_id_and_participant_b() {
    let a = bin().args(["layout"]).output().unwrap();
    let b = bin().args(["layout", "--seed", "something-else"]).output().unwrap();
    let va: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    let vb: serde_json::Value = serde_json::from_slice(&b.stdout).unwrap();
    assert_ne!(va["run_id"], vb["run_id"]);
    assert_ne!(va["participants"][1]["d1"]["account_soid"], vb["participants"][1]["d1"]["account_soid"]);
    // …and participant A is anchored, so it deliberately does NOT move.
    assert_eq!(va["participants"][0]["d1"]["account_soid"], vb["participants"][0]["d1"]["account_soid"]);
}

#[test]
fn layout_creates_nothing() {
    let tmp = std::env::temp_dir().join("d1-harness-should-never-exist");
    let _ = std::fs::remove_dir_all(&tmp);
    let out = bin().args(["layout", "--root", tmp.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success());
    assert!(!tmp.exists(), "layout generation created {}", tmp.display());
}

#[test]
fn env_emits_quoted_exports_for_one_participant_only() {
    let out = bin().args(["env", "--slot", "b"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("export D1_ACCOUNT_SOID="));
    assert!(text.contains("export XDG_DATA_HOME="));
    assert!(text.contains("export D1_MANIFEST_FILE="));
    // Every value is single-quoted, so a path with a space or a `$` cannot become two words.
    for line in text.lines().filter(|l| l.starts_with("export ")) {
        let (_, value) = line.split_once('=').unwrap();
        assert!(value.starts_with('\'') && value.ends_with('\''), "unquoted: {line}");
    }
    let a = bin().args(["env", "--slot", "a"]).output().unwrap();
    assert_ne!(String::from_utf8(a.stdout).unwrap(), text, "both slots emitted the same environment");
}

#[test]
fn env_refuses_a_slot_that_does_not_exist() {
    let out = bin().args(["env", "--slot", "c"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn preflight_refuses_by_default_and_exits_3() {
    let out = bin().args(["preflight"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(out.status.code(), Some(3), "{text}");
    assert!(text.contains("REFUSED"));
    assert!(text.contains("ActivationReviewMissing"));
    // The word appears in a refusal detail ("only --stage arm can return READY"), so match
    // the verdict line itself, not the word.
    assert!(!text.contains("\nREADY -"));
}

#[test]
fn preflight_at_arm_stage_still_refuses_on_this_machine() {
    // This box is not the harness namespace and the capture planes are not armed, so `arm` must
    // refuse here too. If this ever passes, something armed the harness - which is the operator's
    // decision, not a test's.
    let out = bin()
        .args(["preflight", "--stage", "arm", "--slot", "a", "--activation-review", "test"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn controls_exit_zero() {
    let out = bin().args(["controls"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn pad_print_ignore_exits_zero_and_emits_only_vid_pid_pairs() {
    let out = bin().args(["pad", "print-ignore"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    let line = text.trim_end();
    // Empty is legal (headless box); otherwise every comma-item is 0xVVVV/0xPPPP.
    for item in line.split(',').filter(|s| !s.is_empty()) {
        let (v, p) = item.split_once('/').expect("item has a slash");
        assert!(v.starts_with("0x") && p.starts_with("0x"), "malformed item: {item}");
        assert!(u16::from_str_radix(&v[2..], 16).is_ok(), "vendor not hex: {item}");
        assert!(u16::from_str_radix(&p[2..], 16).is_ok(), "product not hex: {item}");
    }
}

#[test]
fn pad_run_refuses_a_malformed_usb_id_before_touching_anything() {
    let fifo = std::env::temp_dir().join("d1-harness-pad-refuse-test.fifo");
    let out = bin()
        .args(["pad", "run", "--vendor", "zzzz", "--fifo", fifo.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!fifo.exists(), "a refused run must not leave a FIFO behind");
}

#[test]
fn pad_controls_never_fail_here() {
    // Where /dev/uinput is accessible this executes the full device tier; where it is not, the
    // device tier SKIPs loudly. Either way, a FAIL (exit 2) means the pad may not be cited.
    let out = bin().args(["pad", "controls"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn the_pid_ledger_refuses_a_directory_it_would_have_to_create() {
    let path = std::env::temp_dir().join("d1-harness-no-such-dir/pids.json");
    let out = bin()
        .args([
            "pid",
            "--ledger",
            path.to_str().unwrap(),
            "--role",
            "self",
            "--pid",
            &std::process::id().to_string(),
            "--marker",
            "cli",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn shutdown_is_a_dry_run_by_default_and_refuses_an_arbitrary_signal() {
    let dir = std::env::temp_dir().join(format!("d1-harness-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ledger = dir.join("pids.json");
    let me = std::process::id().to_string();

    // Record this very test process - the one process whose liveness we are certain of.
    let out = bin()
        .args([
            "pid",
            "--ledger",
            ledger.to_str().unwrap(),
            "--run-id",
            "test",
            "--role",
            "self",
            "--pid",
            &me,
            "--marker",
            "cli",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));

    // A dry run says what it would do and does nothing. We are still alive afterwards, which is
    // the assertion: if `--execute` were the default this test would kill itself.
    let out = bin().args(["shutdown", "--ledger", ledger.to_str().unwrap()]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(out.status.code(), Some(0));
    assert!(text.contains("SIGNAL"), "{text}");
    assert!(text.contains("dry run"), "{text}");

    let out = bin()
        .args(["shutdown", "--ledger", ledger.to_str().unwrap(), "--signal", "USR1", "--execute"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "an open-ended signal argument must be refused");

    // A ledger entry for a pid that cannot exist is skipped, never signalled.
    let text = std::fs::read_to_string(&ledger).unwrap().replace(&me, "2147483647");
    std::fs::write(&ledger, text).unwrap();
    let out = bin()
        .args(["shutdown", "--ledger", ledger.to_str().unwrap(), "--execute"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("SKIP"), "{text}");
    assert!(!text.contains("kill("), "{text}");

    let _ = std::fs::remove_dir_all(&dir);
}
