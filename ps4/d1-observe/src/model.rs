// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_schema_version() -> u32 {
    1
}

fn default_status() -> String {
    "ok".to_string()
}

fn default_direction() -> String {
    "local".to_string()
}

/// One normalized observation. This intentionally accepts the existing
/// `send-receive-event.schema.json` envelope without owning protocol bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub run_id: String,
    pub source: String,
    #[serde(default)]
    pub source_sequence: u64,
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub stage: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub inner_type: Option<u16>,
    #[serde(default)]
    pub inner_sequence: Option<u32>,
    #[serde(default)]
    pub investment_id: Option<u16>,
    #[serde(default)]
    pub family: Option<u8>,
    #[serde(default)]
    pub body_length: Option<u64>,
    #[serde(default)]
    pub frame_length: Option<u64>,
    #[serde(default)]
    pub monotonic_ns: Option<u64>,
    #[serde(default)]
    pub socket_key: Option<String>,
    #[serde(default)]
    pub wire_key: Option<String>,
    #[serde(default)]
    pub details: BTreeMap<String, Value>,
}

impl Event {
    pub fn stable_id(&self, input_index: usize) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.run_id, self.source, self.source_sequence, self.stage, input_index
        )
    }

    pub fn detail_u64(&self, key: &str) -> Option<u64> {
        self.details.get(key).and_then(Value::as_u64)
    }

    pub fn detail_str(&self, key: &str) -> Option<&str> {
        self.details.get(key).and_then(Value::as_str)
    }

    pub fn message_label(&self) -> String {
        if let Some(id) = self.investment_id {
            format!("investment {id}")
        } else if let Some(family) = self.family {
            format!("QueueZ family {family}")
        } else if let Some(kind) = self.inner_type {
            format!("inner 0x{kind:04x}")
        } else {
            self.stage.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Observed,
    Inferred,
    Missing,
    NotObserved,
    Confounded,
}

impl Default for EvidenceStatus {
    fn default() -> Self {
        Self::Observed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRequest {
    pub connection_id: String,
    pub request_type: u16,
    pub sequence: u32,
    pub investment_id: Option<u16>,
    pub expected_reply_type: Option<u16>,
    pub queued_at: usize,
    pub evidence: EvidenceStatus,
}

impl PendingRequest {
    pub fn label(&self) -> String {
        match self.investment_id {
            Some(id) => format!("investment {id}"),
            None => format!("inner 0x{:04x}", self.request_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnosis {
    HeadOfLine {
        event_index: usize,
        connection_id: String,
        head: PendingRequest,
        candidate: PendingRequest,
        received_type: Option<u16>,
        received_sequence: Option<u32>,
        explanation: String,
    },
    QueueHeadTimeout {
        event_index: usize,
        connection_id: String,
        head: PendingRequest,
        explanation: String,
    },
    ReplyTypeMismatch {
        event_index: usize,
        connection_id: String,
        head: PendingRequest,
        received_type: Option<u16>,
        explanation: String,
    },
    NoRecipientAtHead {
        event_index: usize,
        connection_id: String,
        head: Option<PendingRequest>,
        received_sequence: Option<u32>,
        explanation: String,
    },
    ConsumerFailure {
        event_index: usize,
        correlation_id: Option<String>,
        explanation: String,
    },
    PredicateFailure {
        event_index: usize,
        predicate: String,
        observed: String,
        expected: String,
        explanation: String,
    },
    ManualMutationThenFault {
        event_index: usize,
        target: String,
        explanation: String,
    },
}

impl Diagnosis {
    pub fn event_index(&self) -> usize {
        match self {
            Self::HeadOfLine { event_index, .. }
            | Self::QueueHeadTimeout { event_index, .. }
            | Self::ReplyTypeMismatch { event_index, .. }
            | Self::NoRecipientAtHead { event_index, .. }
            | Self::ConsumerFailure { event_index, .. }
            | Self::PredicateFailure { event_index, .. }
            | Self::ManualMutationThenFault { event_index, .. } => *event_index,
        }
    }

    pub fn explanation(&self) -> &str {
        match self {
            Self::HeadOfLine { explanation, .. }
            | Self::QueueHeadTimeout { explanation, .. }
            | Self::ReplyTypeMismatch { explanation, .. }
            | Self::NoRecipientAtHead { explanation, .. }
            | Self::ConsumerFailure { explanation, .. }
            | Self::PredicateFailure { explanation, .. }
            | Self::ManualMutationThenFault { explanation, .. } => explanation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuezVerdict {
    ObjectInserted,
    FamilyAccepted,
    FamilyRefused,
    PartialFailure,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub event_index: Option<usize>,
    pub queues: BTreeMap<String, Vec<PendingRequest>>,
    pub boot_state: Option<String>,
    pub world: Option<String>,
    pub queuez: BTreeMap<String, QueuezVerdict>,
    pub last_failure: Option<Diagnosis>,
    pub last_manual_write: Option<(usize, String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineRow {
    pub index: usize,
    pub event_id: String,
    pub source: String,
    pub source_sequence: u64,
    pub monotonic_ns: Option<u64>,
    pub direction: String,
    pub stage: String,
    pub status: String,
    pub connection_id: Option<String>,
    pub message: String,
    pub inner_sequence: Option<u32>,
    pub evidence: EvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunData {
    pub run_id: String,
    pub events: Vec<Event>,
    #[serde(default)]
    pub coverage: BTreeMap<String, Value>,
    #[serde(default)]
    pub provenance: BTreeMap<String, String>,
    pub source_description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReducedRun {
    pub run_id: String,
    pub event_count: usize,
    pub ordering: EvidenceStatus,
    pub timeline: Vec<TimelineRow>,
    pub final_snapshot: Snapshot,
    pub diagnoses: Vec<Diagnosis>,
    pub warnings: Vec<String>,
    pub coverage: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    pub event: TimelineRow,
    pub snapshot: Snapshot,
    pub diagnosis: Option<Diagnosis>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Divergence {
    pub index: usize,
    pub a_event: Option<TimelineRow>,
    pub b_event: Option<TimelineRow>,
    pub a_snapshot: Snapshot,
    pub b_snapshot: Snapshot,
    pub changed_provenance: BTreeMap<String, (Option<String>, Option<String>)>,
    pub explanation: String,
}

