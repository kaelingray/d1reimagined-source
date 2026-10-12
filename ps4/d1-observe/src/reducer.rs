// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::{BTreeMap, VecDeque};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::model::{
    Diagnosis, Divergence, Event, EvidenceStatus, Explanation, PendingRequest, QueuezVerdict,
    ReducedRun, RunData, Snapshot, TimelineRow,
};

#[derive(Debug, Default)]
struct ReplayState {
    queues: BTreeMap<String, VecDeque<PendingRequest>>,
    boot_state: Option<String>,
    world: Option<String>,
    queuez: BTreeMap<String, QueuezVerdict>,
    diagnoses: Vec<Diagnosis>,
    last_manual_write: Option<(usize, String, String)>,
}

impl ReplayState {
    fn snapshot(&self, event_index: Option<usize>) -> Snapshot {
        Snapshot {
            event_index,
            queues: self
                .queues
                .iter()
                .map(|(key, queue)| (key.clone(), queue.iter().cloned().collect()))
                .collect(),
            boot_state: self.boot_state.clone(),
            world: self.world.clone(),
            queuez: self.queuez.clone(),
            last_failure: self.diagnoses.last().cloned(),
            last_manual_write: self.last_manual_write.clone(),
        }
    }
}

pub fn reduce(run: &RunData) -> ReducedRun {
    let mut state = ReplayState::default();
    let mut timeline = Vec::with_capacity(run.events.len());
    for (index, event) in run.events.iter().enumerate() {
        timeline.push(timeline_row(event, index));
        apply_event(&mut state, event, index);
    }

    let monotonic_complete = !run.events.is_empty()
        && run.events.iter().all(|event| event.monotonic_ns.is_some())
        && run
            .events
            .windows(2)
            .all(|pair| pair[0].monotonic_ns <= pair[1].monotonic_ns);
    let ordering = if monotonic_complete {
        EvidenceStatus::Observed
    } else {
        EvidenceStatus::NotObserved
    };
    let mut warnings = Vec::new();
    if !monotonic_complete {
        warnings.push(
            "global cross-source order is not proven; artifact order is displayed without claiming synchronized time"
                .to_string(),
        );
    }
    if !run
        .events
        .iter()
        .any(|event| event.stage == "client_request_queued")
    {
        warnings.push(
            "client queue-insertion events are absent; a server-received request is not promoted into client queue state"
                .to_string(),
        );
    }

    ReducedRun {
        run_id: run.run_id.clone(),
        event_count: run.events.len(),
        ordering,
        timeline,
        final_snapshot: state.snapshot(run.events.len().checked_sub(1)),
        diagnoses: state.diagnoses,
        warnings,
        coverage: run.coverage.clone(),
        provenance: run.provenance.clone(),
    }
}

pub fn snapshot_at(run: &RunData, index: usize) -> Result<Explanation> {
    if index >= run.events.len() {
        bail!("event index {index} is outside 0..{}", run.events.len());
    }
    let mut state = ReplayState::default();
    for (event_index, event) in run.events.iter().enumerate().take(index + 1) {
        apply_event(&mut state, event, event_index);
    }
    let diagnosis = state
        .diagnoses
        .iter()
        .rev()
        .find(|diagnosis| diagnosis.event_index() == index)
        .cloned();
    Ok(Explanation {
        event: timeline_row(&run.events[index], index),
        snapshot: state.snapshot(Some(index)),
        diagnosis,
    })
}

pub fn first_divergence(a: &RunData, b: &RunData) -> Divergence {
    let common = a.events.len().min(b.events.len());
    let index = (0..common)
        .find(|&index| semantic_fingerprint(&a.events[index]) != semantic_fingerprint(&b.events[index]))
        .unwrap_or(common);

    let a_event = a.events.get(index).map(|event| timeline_row(event, index));
    let b_event = b.events.get(index).map(|event| timeline_row(event, index));
    let a_snapshot = if a.events.is_empty() {
        Snapshot::default()
    } else {
        snapshot_at(a, index.min(a.events.len() - 1))
            .map(|explanation| explanation.snapshot)
            .unwrap_or_default()
    };
    let b_snapshot = if b.events.is_empty() {
        Snapshot::default()
    } else {
        snapshot_at(b, index.min(b.events.len() - 1))
            .map(|explanation| explanation.snapshot)
            .unwrap_or_default()
    };

    let mut keys: Vec<String> = a
        .provenance
        .keys()
        .chain(b.provenance.keys())
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    let changed_provenance = keys
        .into_iter()
        .filter_map(|key| {
            let before = a.provenance.get(&key).cloned();
            let after = b.provenance.get(&key).cloned();
            (before != after).then_some((key, (before, after)))
        })
        .collect();

    let explanation = match (&a_event, &b_event) {
        (Some(a), Some(b)) => format!(
            "first semantic divergence at event {index}: A={} {} / B={} {}",
            a.stage, a.status, b.stage, b.status
        ),
        (None, Some(_)) => format!("run A ends before run B event {index}"),
        (Some(_), None) => format!("run B ends before run A event {index}"),
        (None, None) => "no semantic event divergence".to_string(),
    };

    Divergence {
        index,
        a_event,
        b_event,
        a_snapshot,
        b_snapshot,
        changed_provenance,
        explanation,
    }
}

fn timeline_row(event: &Event, index: usize) -> TimelineRow {
    TimelineRow {
        index,
        event_id: event.stable_id(index),
        source: event.source.clone(),
        source_sequence: event.source_sequence,
        monotonic_ns: event.monotonic_ns,
        direction: event.direction.clone(),
        stage: event.stage.clone(),
        status: event.status.clone(),
        connection_id: event.connection_id.clone(),
        message: event.message_label(),
        inner_sequence: event.inner_sequence,
        evidence: evidence_from_event(event),
    }
}

fn evidence_from_event(event: &Event) -> EvidenceStatus {
    match event.detail_str("evidence") {
        Some("inferred") => EvidenceStatus::Inferred,
        Some("missing") => EvidenceStatus::Missing,
        Some("not_observed") => EvidenceStatus::NotObserved,
        Some("confounded") => EvidenceStatus::Confounded,
        _ => EvidenceStatus::Observed,
    }
}

fn connection(event: &Event) -> String {
    event
        .connection_id
        .clone()
        .unwrap_or_else(|| "bap-unknown".to_string())
}

fn apply_event(state: &mut ReplayState, event: &Event, index: usize) {
    match event.stage.as_str() {
        "client_request_queued" => queue_request(state, event, index),
        "client_reply_matched" | "client_request_retired" => {
            match_and_retire(state, event, index)
        }
        "client_no_recipient" => diagnose_no_recipient(state, event, index),
        "connection_timeout" | "client_request_timeout" => diagnose_timeout(state, event, index),
        "client_consumer_result" if event.status == "failed" => {
            state.diagnoses.push(Diagnosis::ConsumerFailure {
                event_index: index,
                correlation_id: event.correlation_id.clone(),
                explanation: format!(
                    "consumer rejected {} after recipient matching",
                    event.message_label()
                ),
            });
        }
        "state_entered" => {
            state.boot_state = event.detail_str("state").map(str::to_string);
        }
        "world_changed" => {
            state.world = event.detail_str("world").map(str::to_string);
        }
        "queuez_object_inserted" => update_queuez(state, event, QueuezVerdict::ObjectInserted),
        "client_family_update_result" if event.status == "failed" => {
            update_queuez(state, event, QueuezVerdict::FamilyRefused)
        }
        "client_family_update_result" => {
            update_queuez(state, event, QueuezVerdict::FamilyAccepted)
        }
        "predicate_evaluated" if event.status == "failed" => {
            let predicate = event
                .detail_str("predicate")
                .unwrap_or("unnamed predicate")
                .to_string();
            let observed = event
                .details
                .get("observed")
                .map(value_text)
                .unwrap_or_else(|| "unknown".to_string());
            let expected = event
                .details
                .get("expected")
                .map(value_text)
                .unwrap_or_else(|| "unknown".to_string());
            state.diagnoses.push(Diagnosis::PredicateFailure {
                event_index: index,
                predicate: predicate.clone(),
                observed: observed.clone(),
                expected: expected.clone(),
                explanation: format!(
                    "predicate {predicate} failed: observed {observed}, expected {expected}"
                ),
            });
        }
        "manual_memory_write" => {
            let target = event.detail_str("target").unwrap_or("unknown address").to_string();
            let semantic_type = event
                .detail_str("semantic_type")
                .unwrap_or("unknown")
                .to_string();
            state.last_manual_write = Some((index, target, semantic_type));
        }
        "guest_fault" => {
            if let Some((_, target, semantic_type)) = &state.last_manual_write {
                state.diagnoses.push(Diagnosis::ManualMutationThenFault {
                    event_index: index,
                    target: target.clone(),
                    explanation: format!(
                        "guest fault followed a manual write to {target}; the slot semantic type remains {semantic_type}, so temporal association is observed but field identity is not proven"
                    ),
                });
            }
        }
        _ => {}
    }
}

fn queue_request(state: &mut ReplayState, event: &Event, index: usize) {
    let Some(request_type) = event.inner_type else {
        return;
    };
    let Some(sequence) = event.inner_sequence else {
        return;
    };
    let request = PendingRequest {
        connection_id: connection(event),
        request_type,
        sequence,
        investment_id: event.investment_id,
        expected_reply_type: event
            .detail_u64("expected_reply_type")
            .and_then(|value| u16::try_from(value).ok()),
        queued_at: index,
        evidence: evidence_from_event(event),
    };
    state
        .queues
        .entry(request.connection_id.clone())
        .or_default()
        .push_back(request);
}

fn match_and_retire(state: &mut ReplayState, event: &Event, index: usize) {
    let connection_id = connection(event);
    let queue = state.queues.entry(connection_id.clone()).or_default();
    let Some(head) = queue.front().cloned() else {
        state.diagnoses.push(Diagnosis::NoRecipientAtHead {
            event_index: index,
            connection_id,
            head: None,
            received_sequence: event.inner_sequence,
            explanation: "reply-match event arrived with an empty modeled in-flight queue".to_string(),
        });
        return;
    };
    let sequence_matches = event.inner_sequence == Some(head.sequence);
    let type_matches = head
        .expected_reply_type
        .map(|expected| event.inner_type == Some(expected))
        .unwrap_or(true);
    if sequence_matches && type_matches {
        queue.pop_front();
    } else if !sequence_matches {
        let candidate = queue
            .iter()
            .find(|request| Some(request.sequence) == event.inner_sequence)
            .cloned();
        if let Some(candidate) = candidate {
            state.diagnoses.push(head_of_line(
                index,
                connection_id,
                head,
                candidate,
                event,
            ));
        } else {
            state.diagnoses.push(Diagnosis::NoRecipientAtHead {
                event_index: index,
                connection_id,
                head: Some(head),
                received_sequence: event.inner_sequence,
                explanation: "reply sequence matches neither the queue head nor a later modeled request"
                    .to_string(),
            });
        }
    } else {
        state.diagnoses.push(Diagnosis::ReplyTypeMismatch {
            event_index: index,
            connection_id,
            head: head.clone(),
            received_type: event.inner_type,
            explanation: format!(
                "queue head sequence {} matched, but expected reply type {} and received {}",
                head.sequence,
                fmt_type(head.expected_reply_type),
                fmt_type(event.inner_type)
            ),
        });
    }
}

fn diagnose_no_recipient(state: &mut ReplayState, event: &Event, index: usize) {
    let connection_id = connection(event);
    let queue = state.queues.entry(connection_id.clone()).or_default();
    let head = queue.front().cloned();
    let candidate = event
        .inner_sequence
        .and_then(|sequence| queue.iter().find(|request| request.sequence == sequence).cloned());
    match (head, candidate) {
        (Some(head), Some(candidate)) if head.sequence != candidate.sequence => {
            state.diagnoses.push(head_of_line(
                index,
                connection_id,
                head,
                candidate,
                event,
            ));
        }
        (Some(head), _) => state.diagnoses.push(Diagnosis::NoRecipientAtHead {
            event_index: index,
            connection_id,
            received_sequence: event.inner_sequence,
            explanation: format!(
                "client found no recipient while queue head was {} sequence {}; exact handler/type failure remains unresolved",
                head.label(), head.sequence
            ),
            head: Some(head),
        }),
        (None, _) => state.diagnoses.push(Diagnosis::NoRecipientAtHead {
            event_index: index,
            connection_id,
            head: None,
            received_sequence: event.inner_sequence,
            explanation: "client found no recipient and no queue-head event was captured".to_string(),
        }),
    }
}

fn head_of_line(
    event_index: usize,
    connection_id: String,
    head: PendingRequest,
    candidate: PendingRequest,
    event: &Event,
) -> Diagnosis {
    Diagnosis::HeadOfLine {
        event_index,
        connection_id,
        received_type: event.inner_type,
        received_sequence: event.inner_sequence,
        explanation: format!(
            "reply sequence {} resembles later queue item {} ({}), but the client matcher examines head {} ({})",
            event
                .inner_sequence
                .map(|sequence| sequence.to_string())
                .unwrap_or_else(|| "?".to_string()),
            candidate.sequence,
            candidate.label(),
            head.sequence,
            head.label()
        ),
        head,
        candidate,
    }
}

fn diagnose_timeout(state: &mut ReplayState, event: &Event, index: usize) {
    let connection_id = connection(event);
    let Some(head) = state
        .queues
        .get(&connection_id)
        .and_then(|queue| queue.front())
        .cloned()
    else {
        return;
    };
    state.diagnoses.push(Diagnosis::QueueHeadTimeout {
        event_index: index,
        connection_id,
        explanation: format!(
            "oldest in-flight request {} sequence {} timed out and blocks every later reply",
            head.label(), head.sequence
        ),
        head,
    });
}

fn update_queuez(state: &mut ReplayState, event: &Event, next: QueuezVerdict) {
    let key = match event.family {
        Some(family) => format!("family:{family}"),
        None => "family:unknown".to_string(),
    };
    let previous = state.queuez.get(&key).copied();
    let combined = match (previous, next) {
        (Some(QueuezVerdict::ObjectInserted), QueuezVerdict::FamilyRefused)
        | (Some(QueuezVerdict::FamilyRefused), QueuezVerdict::ObjectInserted)
        | (Some(QueuezVerdict::PartialFailure), _)
        | (_, QueuezVerdict::PartialFailure) => QueuezVerdict::PartialFailure,
        (_, value) => value,
    };
    state.queuez.insert(key, combined);
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn fmt_type(value: Option<u16>) -> String {
    value
        .map(|value| format!("0x{value:04x}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn semantic_fingerprint(event: &Event) -> Value {
    json!({
        "source": event.source,
        "connection_id": event.connection_id,
        "direction": event.direction,
        "stage": event.stage,
        "status": event.status,
        "correlation_id": event.correlation_id,
        "inner_type": event.inner_type,
        "inner_sequence": event.inner_sequence,
        "investment_id": event.investment_id,
        "family": event.family,
        "body_length": event.body_length,
        "frame_length": event.frame_length,
        "details": event.details,
    })
}

