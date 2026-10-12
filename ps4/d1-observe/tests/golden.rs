// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::path::{Path, PathBuf};
use std::process::Command;

use d1_observe::framing::{BapFrameAccumulator, FrameError};
use d1_observe::{
    first_divergence, load_run, reduce, snapshot_at, Diagnosis, LoadOptions, QueuezVerdict,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

#[test]
fn suback_retires_000c_and_promotes_0020() {
    let run = load_run(
        &fixture("head-of-line-operator-504.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let after_suback = snapshot_at(&run, 2).unwrap().snapshot;
    let queue = &after_suback.queues["bap-1"];
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].request_type, 0x0020);
    assert_eq!(queue[0].sequence, 10);
}

#[test]
fn operator_504_failure_names_0020_as_the_head_not_000c() {
    let run = load_run(
        &fixture("head-of-line-operator-504.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let reduced = reduce(&run);
    match reduced.diagnoses.first().unwrap() {
        Diagnosis::HeadOfLine { head, candidate, received_sequence, .. } => {
            assert_eq!(head.request_type, 0x0020);
            assert_eq!(head.sequence, 10);
            assert_eq!(candidate.investment_id, Some(504));
            assert_eq!(*received_sequence, Some(13));
        }
        other => panic!("expected head-of-line diagnosis, got {other:?}"),
    }
}

#[test]
fn pre_suback_timeout_names_000c_as_the_historical_head() {
    let run = load_run(
        &fixture("pre-suback-timeout.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let reduced = reduce(&run);
    match reduced.diagnoses.first().unwrap() {
        Diagnosis::QueueHeadTimeout { head, .. } => {
            assert_eq!(head.request_type, 0x000c);
            assert_eq!(head.expected_reply_type, Some(0x000d));
        }
        other => panic!("expected timeout diagnosis, got {other:?}"),
    }
}

#[test]
fn post_suback_positive_control_drains_queue_and_reaches_character_signin() {
    let run = load_run(
        &fixture("post-suback-clean.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let reduced = reduce(&run);
    assert!(reduced.diagnoses.is_empty());
    assert!(reduced.final_snapshot.queues["bap-1"].is_empty());
    assert_eq!(reduced.final_snapshot.world.as_deref(), Some("orbit"));
    assert_eq!(
        reduced.final_snapshot.boot_state.as_deref(),
        Some("character:signin")
    );

    let blocked = load_run(
        &fixture("pre-suback-timeout.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let divergence = first_divergence(&blocked, &run);
    assert_eq!(divergence.index, 1);
    assert_eq!(divergence.a_event.unwrap().stage, "client_request_queued");
    assert_eq!(divergence.b_event.unwrap().stage, "client_reply_matched");
}

#[test]
fn object_insert_plus_family_refusal_is_partial_failure() {
    let run = load_run(
        &fixture("queuez-partial-acceptance.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let reduced = reduce(&run);
    assert_eq!(
        reduced.final_snapshot.queuez.get("family:4"),
        Some(&QueuezVerdict::PartialFailure)
    );
}

#[test]
fn unknown_selection_slot_remains_unknown_after_fault() {
    let run = load_run(
        &fixture("selection-slot-fault.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let reduced = reduce(&run);
    assert!(matches!(reduced.diagnoses[0], Diagnosis::PredicateFailure { .. }));
    match reduced.diagnoses.last().unwrap() {
        Diagnosis::ManualMutationThenFault { explanation, .. } => {
            assert!(explanation.contains("concrete type unknown"));
            assert!(explanation.contains("not proven"));
        }
        other => panic!("expected mutation/fault diagnosis, got {other:?}"),
    }
}

#[test]
fn bap_accumulator_splits_coalesced_frames_and_retains_fragments() {
    let a = [0x01, 0x01, 0, 0, 0, 3, 0xaa, 0xbb, 0xcc];
    let b = [0x01, 0x02, 0, 0, 0, 2, 0xdd, 0xee];
    let mut decoder = BapFrameAccumulator::new(1024);
    assert!(decoder.feed(&a[..4]).unwrap().is_empty());
    assert_eq!(decoder.buffered_len(), 4);
    let mut rest = Vec::from(&a[4..]);
    rest.extend_from_slice(&b);
    let frames = decoder.feed(&rest).unwrap();
    assert_eq!(frames, vec![a.to_vec(), b.to_vec()]);
    assert_eq!(decoder.buffered_len(), 0);
}

#[test]
fn impossible_frame_length_is_rejected_without_drain() {
    let mut decoder = BapFrameAccumulator::new(16);
    let error = decoder.feed(&[1, 1, 0, 0, 0, 17]).unwrap_err();
    assert_eq!(
        error,
        FrameError::DeclaredLengthTooLarge { declared: 17, maximum: 16 }
    );
    assert_eq!(decoder.buffered_len(), 6);
}

#[test]
fn reduction_is_byte_deterministic() {
    let run = load_run(
        &fixture("head-of-line-operator-504.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let a = serde_json::to_vec(&reduce(&run)).unwrap();
    let b = serde_json::to_vec(&reduce(&run)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn first_divergence_is_semantic_not_run_id() {
    let a = load_run(
        &fixture("pre-suback-timeout.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let b = load_run(
        &fixture("head-of-line-operator-504.jsonl"),
        &LoadOptions::default(),
    )
    .unwrap();
    let divergence = first_divergence(&a, &b);
    assert_eq!(divergence.index, 1);
    assert_eq!(divergence.a_event.unwrap().stage, "client_request_queued");
    assert_eq!(divergence.b_event.unwrap().stage, "client_reply_matched");
}

#[test]
fn why_stuck_json_explains_missing_queue_instrumentation() {
    let output = Command::new(env!("CARGO_BIN_EXE_d1-observe"))
        .args([
            "--json",
            "why-stuck",
            fixture("queuez-partial-acceptance.jsonl")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["proven"], false);
    assert!(report["diagnosis"].is_null());
    assert!(report["reason"]
        .as_str()
        .unwrap()
        .contains("client queue instrumentation is absent"));
}
