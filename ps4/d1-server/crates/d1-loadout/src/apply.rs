// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The **control-plane request contract** for "apply this loadout".
//!
//! # What this is, and what it is emphatically not
//!
//! This is an HTTP/JSON message between *our own tools* - the editor and whatever process owns the
//! character state. It is **not** a game protocol message. It contains no BAP, no QueueZ, no RSAT,
//! no framing, no byte order, no field ordering that matters. `CLAUDE.md`'s "one protocol, one
//! implementer" rule is about the wire to the client; this never reaches the client.
//!
//! The transport choice is not free either: `historicaldocs/design/interface-architecture.md` §0 makes **HTTP/JSON
//! on a loopback socket** the single transport for every frontend, precisely so the local and remote
//! paths cannot drift. An editor that reached into the server in-process would be the second
//! transport that doc exists to prevent.
//!
//! # The route
//!
//! ```text
//! POST /control/loadout/apply     Content-Type: application/json
//! ```
//!
//! Loopback bind only; `ssh -L` is the auth layer, as everywhere else in this project. No auth code.
//!
//! # ⚠ The receiving end does not exist yet
//!
//! Nothing serves this route today. That is expected and is why the client distinguishes *four*
//! failure modes rather than printing "error":
//!
//! | what happened | how it looks | what it means |
//! |---|---|---|
//! | connect refused | transport error | no control plane is running |
//! | 404 / 405 | HTTP status | a server is there, this route is not implemented |
//! | 501 | HTTP status | the route exists and is explicitly a stub |
//! | 200 + `accepted: false` | body | the server understood and declined; `issues` says why |
//!
//! Collapsing those into one message is how you spend an afternoon debugging a server that was
//! never started.
//!
//! # ⚠ `[U]`: how a loadout becomes character state
//!
//! Everything past the HTTP boundary is unknown. The main session is still deriving the inventory
//! object schema (family 4 / type 3, capacity 666, element size `0xB8`). We do not know whether the
//! manifest item hash is the identifier those objects carry, how an equipped item differs from a
//! stored one, or what a character is addressed by. So:
//!
//! * [`ApplyTarget`] addresses a character by **index** with an optional opaque id, because the
//!   character identity model is `[U]` and an index is the least we can be wrong about.
//! * [`ApplyRequest::dry_run`] exists so the first implementation can answer honestly without
//!   touching state.
//! * [`ApplyResponse`] reuses [`crate::ValidationIssue`] rather than inventing an error enum the
//!   server would have to match on.

use serde::{Deserialize, Serialize};

use crate::model::{Loadout, ValidationIssue};

/// Version of *this contract*, independent of the loadout document's `schema_version`.
///
/// Two versions because they move for different reasons: the loadout format changes when we learn
/// something about items, the control-plane envelope changes when we learn something about how the
/// server wants to be asked.
pub const APPLY_CONTRACT_VERSION: u32 = 1;

/// The path the request is posted to. A constant so the client and any future server cannot
/// disagree about it in two string literals.
pub const APPLY_PATH: &str = "/control/loadout/apply";

/// Which character to apply to.
///
/// `[U]` - see the module docs. `index` is the only field a caller must supply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApplyTarget {
    /// 0-based character slot on the account. Destiny 1 allows three.
    pub index: u32,
    /// An opaque server-side character identifier, if the caller happens to know one. Empty means
    /// "use `index`". Kept as a string because we do not know its type.
    pub character_id: String,
}

impl Default for ApplyTarget {
    fn default() -> Self {
        ApplyTarget { index: 0, character_id: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApplyRequest {
    pub contract_version: u32,
    /// Client-generated, echoed in the response. Correlates the editor's action with the server's
    /// log line, and lets a retry be recognised as a retry rather than a second apply.
    pub request_id: String,
    pub target: ApplyTarget,
    /// Validate and report, change nothing. **The first server implementation should honour this
    /// before it honours anything else.**
    pub dry_run: bool,
    /// The whole document, not a delta. A delta would require both sides to agree on prior state,
    /// which is exactly the shared assumption that goes stale silently.
    pub loadout: Loadout,
}

impl Default for ApplyRequest {
    fn default() -> Self {
        ApplyRequest {
            contract_version: APPLY_CONTRACT_VERSION,
            request_id: String::new(),
            target: ApplyTarget::default(),
            dry_run: false,
            loadout: Loadout::default(),
        }
    }
}

impl ApplyRequest {
    pub fn new(loadout: Loadout, target: ApplyTarget, dry_run: bool, request_id: String) -> Self {
        ApplyRequest {
            contract_version: APPLY_CONTRACT_VERSION,
            request_id,
            target,
            dry_run,
            loadout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ApplyResponse {
    pub contract_version: u32,
    /// Echo of [`ApplyRequest::request_id`].
    pub request_id: String,
    /// The server changed (or, under `dry_run`, would have changed) state.
    pub accepted: bool,
    /// One line for a human. Shown verbatim in the UI.
    pub detail: String,
    /// Server-side findings. May repeat what the client already knew - the client is not the
    /// authority on what the server will accept.
    pub issues: Vec<ValidationIssue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Slot};

    #[test]
    fn request_round_trips_and_tolerates_extra_fields() {
        let mut l = Loadout { name: "apply me".into(), class: Class::Hunter, ..Default::default() };
        l.set(
            Slot::Heavy,
            crate::ItemRef { hash: 1_274_330_687, bucket_hash: Slot::Heavy.bucket_hash(), ..Default::default() },
        );
        let req = ApplyRequest::new(l, ApplyTarget { index: 1, ..Default::default() }, true, "r-1".into());
        let json = serde_json::to_string(&req).unwrap();
        let back: ApplyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);

        // A server that adds a field must not break an older client, and vice versa.
        let with_extra = format!("{{{}, \"a_field_from_the_future\": 1}}", &json[1..json.len() - 1]);
        let tolerated: ApplyRequest = serde_json::from_str(&with_extra).unwrap();
        assert_eq!(tolerated.loadout.name, "apply me");
    }

    #[test]
    fn response_defaults_are_a_refusal() {
        // An empty body must never read as "applied". Default is the safe direction.
        let r: ApplyResponse = serde_json::from_str("{}").unwrap();
        assert!(!r.accepted);
    }
}
