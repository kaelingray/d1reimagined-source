// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The Activity-Host application protocol: the 49 `user_defined_type` values.
//!
//! # The id space, and why it is the same thing as `user_defined_type`
//!
//! BAP messages 8 (`client_to_activity_host_notification`) and 100
//! (`activity_host_to_client_notification`) each carry a `u32be user_defined_type` and an opaque
//! payload. `d1-bap` correctly says the legal values are "not a BAP question". **They are this
//! table.**
//!
//! ⭐ **The control that ties them together, and it is a two-sided one:**
//!
//! | independent fact | from |
//! |---|---|
//! | `udt 8` never reaches the wire - it is intercepted client-side and rebuilt as BAP message 6 | `historicaldocs/design/SERVER-ROLES-PLAN.md` §3.4 |
//! | AH id **8** is `request_activity_host`, and BAP 6 is how an AH is started | this table |
//! | `udt 10` is what the BAP message-7 handler injects into the activity client | §3.3, `0x594C60` |
//! | AH id **10** is `start_activity_host_response` | this table |
//! | BAP message 7's body is `{u8 2, u64 host_token, char[128] host_text}` = **0x88 bytes of payload** | `d1_bap::activity_host::ActivityHostManagerResponse` |
//! | AH message 10 is **0x88 bytes** = `{u64 ah_id, char session_id[]}` | `historicaldocs/design/activity-host-contract.md` §4 |
//!
//! Two independently-derived facts on each row, and the sizes agree exactly. ⇒
//! **`ActivityHostManagerResponse::host_token` IS the `ah_id`, and `host_text` IS the
//! `session_id`.** See [`crate::registry`] - this closes the plan's `[U]` on what `host_text`
//! should contain, and it says the answer is *not* `"ip:port"`.
//!
//! # Provenance of this table
//!
//! **`[C]`, re-derived in this crate's session** from the pinned 01.29 eboot
//! (`d1-eboot controls` 22/22 PASS), not transcribed:
//!
//! ```text
//! d1-eboot reloc --slot 0x18F8780 --span 400 --json     # 49 string pointers, then NULL at index 49
//! d1-eboot reloc --slot 0x19220B0 --span 48  --json     # OUT PRIMARY / OUT FAH / OUT GAH0 / OUT GAH1
//! ```
//!
//! The NULL terminator at index 49 is the positive control for "the table ends where we say it
//! does"; `index == wire type id` is corroborated by the receive dispatch table (below).
//!
//! ⚠ **Two corrections to `historicaldocs/design/activity-host-contract.md` §2 fall out of the re-derivation:**
//!
//! 1. Ids **30** and **31** are **swapped** there. The eboot says
//!    `30 = query_entity_slot_authority_mask_per_bubble_response`,
//!    `31 = query_entity_slot_authority_mask_response`. The doc has the names the other way round,
//!    and pairs sizes `0x608`/`0x604` with them - so those two sizes are `[U]` here, not `[R]`.
//! 2. The doc's two tables enumerate 44 distinct ids. Five more exist: **9**
//!    `start_activity_host`, **16** `activity_client_keepalive_response`, **21**
//!    `client_authoritative_data_update`, **34** `debug_request_migration`, **40**
//!    `request_script_updates`. None appears in the client's receive dispatch table and none is in
//!    the doc's send list ⇒ [`AhDirection::NoHandler`]. ⭐ **Never send udt 16**: the client has
//!    no handler for a keepalive *response*, so a stub that "acks" keepalives is talking to itself.

use crate::AhError;

/// The table at `0x18F8780` holds 49 entries and a NULL. `[C]`
pub const AH_MESSAGE_COUNT: usize = 49;

/// `index == wire type id`. `[C]` - dumped from `0x18F8780` in this session.
pub const AH_MESSAGE_NAMES: [&str; AH_MESSAGE_COUNT] = [
    "entity_slots_allocated",                              // 0
    "global_activity_state",                               // 1
    "world_globals_state",                                 // 2
    "join_request",                                        // 3
    "join_result",                                         // 4
    "sensor_auth_update",                                  // 5
    "sensor_sense_update",                                 // 6
    "sensor_message",                                      // 7
    "request_activity_host",                               // 8
    "start_activity_host",                                 // 9
    "start_activity_host_response",                        // 10
    "replicate_membership",                                // 11
    "request_peer_reservation",                            // 12
    "release_peer_reservation",                            // 13
    "peer_leave_request",                                  // 14
    "activity_client_keepalive_request",                   // 15
    "activity_client_keepalive_response",                  // 16
    "activity_client_request_state_refresh",               // 17
    "incident",                                            // 18
    "allocate_entity_indices",                             // 19
    "free_entity_indices",                                 // 20
    "client_authoritative_data_update",                    // 21
    "client_identity_update",                              // 22
    "claim_authority_over_abandoned_entity_slots",         // 23
    "purge_abandoned_entity_slots",                        // 24
    "abandon_entity_slots",                                // 25
    "request_purge_entity_slots",                          // 26
    "reset_entity_slot_authority_mask",                    // 27
    "reset_entity_slot_authority_mask_acknowledgement",    // 28
    "query_entity_slot_authority_mask",                    // 29
    "query_entity_slot_authority_mask_per_bubble_response", // 30
    "query_entity_slot_authority_mask_response",           // 31
    "abdicate_authority",                                  // 32
    "process_debug_command",                               // 33
    "debug_request_migration",                             // 34
    "connectivity_failure",                                // 35
    "membership_acknowledgement",                          // 36
    "send_client_heartbeat",                               // 37
    "script_state",                                        // 38
    "script_event",                                        // 39
    "request_script_updates",                              // 40
    "bug_claw",                                            // 41
    "advance_replication_epoch",                           // 42
    "reservations_failed",                                 // 43
    "report_lag_switch",                                   // 44
    "connection_quality_report",                           // 45
    "speculative_migration",                               // 46
    "high_water",                                          // 47
    "refresh_inspirations",                                // 48
];

/// The ids the **client has a receive handler for** - i.e. the only ids an Activity Host may send.
///
/// > ### RETRACTED IN PLACE, 2026-08-11 - there is NO terminator entry
/// >
/// > This doc claimed *"17 entries then a `{0, 0, NULL}` terminator - the terminator is the
/// > positive control."* Two adversarial verifiers refuted it independently: the 16 bytes at
/// > `0x18F85B0` are `c_activity_client`'s `__vmi_class_type_info` (both quadwords are relocation
/// > slots, so they read zero in the FILE only; live they read a vtable pointer and a name
/// > pointer). That "control" would have passed for any object following any table. The real
/// > bound is the hardcoded loop limit in `vfunc[9]` itself: `cmp eax, 0x10; ja` at `0x299472`.
/// > Locate handlers by scanning for the type VALUE, never by ordinal - the table is not sorted
/// > (index 4 holds type 11, index 5 type 10, index 6 type 5).
///
/// `[C]` re-derived: `c_activity_client::vfunc[9]` (`0x299430`) linear-scans a table at
/// `0x18F84A0`, stride `0x10`, `{i32 type; i32 pad; fnptr}`, bounded by the `cmp eax, 0x10`
/// at `0x299472`. (The `fnptr` column reads 0 statically because it is an `R_X86_64_RELATIVE`
/// relocation; the *type* column is in the file.)
pub const AH_TO_CLIENT: [u8; 17] = [0, 1, 2, 4, 5, 7, 10, 11, 18, 23, 24, 27, 29, 38, 39, 42, 43];

/// The ids the client **sends**.
///
/// `[R]` - from `historicaldocs/design/activity-host-contract.md` §2, which derived it from the send sites rather
/// than from a single table. Not re-derived here. It is disjoint from [`AH_TO_CLIENT`] except for
/// **18 `incident`**, which has an identical schema in both directions.
/// ⭐ **21 was added on 2026-08-09** - see [`NO_HANDLER`]'s retraction.
pub const CLIENT_TO_AH: [u8; 29] = [
    3, 6, 8, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 25, 26, 28, 30, 31, 32, 33, 35, 36, 37, 41,
    44, 45, 46, 47, 48,
];

/// Named, but wired to nothing in either direction.
///
/// > ## ⛔ RETRACTED IN PLACE, 2026-08-09 - **udt 21 IS sent, by the client, every frame it can**
/// >
/// > This list used to contain `21 client_authoritative_data_update`, on the reading *"it appears
/// > in neither the receive dispatch table nor the contract doc's send list"*. The premise was that
/// > every client send goes through `activity_client_send_message` (`FUN_00299310`). **It does
/// > not.** `FUN_0029CC20` builds its own bitstream, encodes with `FUN_002C9290`, and calls
/// > `messaging->vfunc[6]` (`0x594550`) **directly**, so a scan of `FUN_00299310`'s call sites was
/// > structurally unable to see it.
/// >
/// > Run `20260809-054417-RUNG33-BYPASS` then measured it twice, at 171 and 3 bytes, and the
/// > listener called it a named gate. It is not a gate: `FUN_002987E0` drives it, and on success
/// > does nothing but `memcpy(ac+0x6748, ac+0x6968, 0x220)` and clear the dirty mask. **It is
/// > fire-and-forget telemetry - the client telling us its own authoritative state.** Nothing waits
/// > for a reply, and there is no reply message in the id space.
/// >
/// > ⇒ Sibling of `CLAUDE.md`'s trap #3: *a negative result is only as good as the premise that
/// > chose its search space.*
pub const NO_HANDLER: [u8; 4] = [9, 16, 34, 40];

/// Which way an id travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhDirection {
    /// The Activity Host sends it; the client has a handler at `0x18F84A0`.
    AhToClient,
    /// The client sends it; the Activity Host must be able to receive it.
    ClientToAh,
    /// `18 incident` - one schema, both ways.
    Both,
    /// Named in the table but wired to nothing. Sending it is a silent drop.
    NoHandler,
}

/// The client's four BAP connection slots, `[C]` from `0x19220B0` (`d1-eboot reloc`).
///
/// ⭐ All four are built by the same function - `FUN_00559650(index)` - which installs the push
/// handler at `conn+0x1138` from **one pool** (`DAT_026A1CE8`, stride `0xA8`, factory
/// `FUN_00563E50`/`FUN_00563ED0`) with no dependence on the index. `[C]`
///
/// ⇒ `[I]` **one class ⇒ one push accept-set.** The 13-entry whitelist at `0x1925A90` that
/// `d1_bap::tables::PUSH_ACCEPT_SET` records for `OUT PRIMARY` governs `OUT FAH` and the `GAH`
/// slots identically. Two consequences, both load-bearing:
///
/// * **100 is pushable on the AH connection** - which is the whole point of [`crate::push`].
/// * **102 `activity_host_to_client_log_notification` is pushable NOWHERE.** It is absent from the
///   accept-set, and there is no fourth connection with a different handler to hope for. This
///   closes the plan §9 Tier-2 `[U]` "where message 102 is legally delivered" with *nowhere on any
///   BAP connection*.
///
/// ⚠ The remaining gap in that argument: the pool's vtable is a runtime vptr, so "same pool ⇒ same
/// vtable ⇒ same `vfunc[0]`" is structural reasoning, not a read. It is falsifiable for free - if
/// a push of 100 on `OUT FAH` is rejected, the reasoning is wrong.
pub const CONNECTION_SLOT_NAMES: [&str; 4] =
    ["OUT PRIMARY", "OUT FAH", "OUT GAH0", "OUT GAH1"];

// ── the ids this crate actually reasons about ─────────────────────────────────────────────────

/// AH → client. `0x600` struct = a 12,288-bit entity-index mask.
pub const UDT_ENTITY_SLOTS_ALLOCATED: u32 = 0;
/// AH → client. The authoritative Activity State: `0x6B0` bytes at payload `+8`.
pub const UDT_GLOBAL_ACTIVITY_STATE: u32 = 1;
/// ⭐⭐ AH → client. `sensor_auth_update` - the bespoke, non-single-descriptor frame that writes
/// the activity **lifetime phase** (`record+0x1AC`) the player-spawn gate reads. Not a
/// `CALL_SITES` row: its handler `FUN_0029D3F0` forwards raw bytes into a simulation-queue element
/// whose applier `FUN_002A1210` decodes a replicated frame over schema `0x80800CF7`. Built by
/// [`d1_oracle::sensor`]; see [`crate::messages::AhMessage::SensorAuthUpdate`].
pub const UDT_SENSOR_AUTH_UPDATE: u32 = 5;
/// client → AH. `0x100` struct.
pub const UDT_JOIN_REQUEST: u32 = 3;
/// AH → client. `0x1A8` struct - the richest message in the protocol.
pub const UDT_JOIN_RESULT: u32 = 4;
/// client → AH, but **never on the wire**: intercepted and rebuilt as BAP message 6.
pub const UDT_REQUEST_ACTIVITY_HOST: u32 = 8;
/// AH → client, but **never on this wire either**: injected by the BAP message-7 handler.
pub const UDT_START_ACTIVITY_HOST_RESPONSE: u32 = 10;
/// AH → client. Client acks with [`UDT_MEMBERSHIP_ACKNOWLEDGEMENT`].
pub const UDT_REPLICATE_MEMBERSHIP: u32 = 11;
/// client → AH. `0x330` struct = `{u32; u32; i32 count; peer[20]}`, peer `0x28`.
pub const UDT_REQUEST_PEER_RESERVATION: u32 = 12;
/// client → AH. ⭐ **A consequence, not a gate.** `FUN_0029A610`, `0x14` struct, tag `0x80800E84`.
/// The client sends it when it has decided to leave - in run `20260809-054417-RUNG33-BYPASS` it
/// arrived 0.3 s *after* the host-crash timeout had already fired. An Activity Host that "handles"
/// it is treating the funeral as the illness.
pub const UDT_PEER_LEAVE_REQUEST: u32 = 14;
/// client → AH. The lease. 1 byte. Cadence is **ours to set**, via `join_result+0x198`.
///
/// ⚠⚠ It is **not** what holds the session open - see [`crate::state::HOST_SILENCE_LIMIT_MS`].
/// This is the client telling *us* it is alive; the client's own liveness test looks the other way
/// and is a receive-side idle timer.
pub const UDT_KEEPALIVE_REQUEST: u32 = 15;
/// ⛔ Named but unhandled. Never send it.
pub const UDT_KEEPALIVE_RESPONSE: u32 = 16;
/// client → AH. ⭐ `c_activity_client::vfunc[5]` (`0x29AB80`), `0x008` struct, tag `0x808009E7`:
/// `{u32 membership_counter, u32}` - the client asking the host to re-publish state. `0x29AB80` is
/// called from the accept branch of the `join_result` handler `0x29BD10`, so a successful join
/// produces one.
pub const UDT_REQUEST_STATE_REFRESH: u32 = 17;
/// client → AH, **fire-and-forget**. `FUN_0029CC20`, `0x220` struct, tag `0x80800C56`, encoded
/// through the *replicated* wrapper `FUN_002C9290` with the client's own dirty mask `ac+0x6B88`.
/// See [`NO_HANDLER`]'s retraction.
pub const UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE: u32 = 21;
/// client → AH. 4 bytes = `u32 count`.
pub const UDT_ALLOCATE_ENTITY_INDICES: u32 = 19;
/// client → AH. `0x600` mask.
pub const UDT_FREE_ENTITY_INDICES: u32 = 20;
/// client → AH.
pub const UDT_CLIENT_IDENTITY_UPDATE: u32 = 22;
/// client → AH.
pub const UDT_CONNECTIVITY_FAILURE: u32 = 35;
/// client → AH. 4 bytes.
pub const UDT_MEMBERSHIP_ACKNOWLEDGEMENT: u32 = 36;
/// client → AH, **fire-and-forget**, and ⛔ **not decodable here**: its descriptor `0x80800D06` is
/// a single field of kind `0x22`, a *dynamic flags* bitset whose backing store comes from a runtime
/// pool that a statically mapped image does not have. See
/// [`d1_oracle::activity_host::FIELD_KIND_DYNAMIC_FLAGS`]. udt 33 is the same shape.
pub const UDT_SEND_CLIENT_HEARTBEAT: u32 = 37;
/// AH → client. 2 bytes = `{u8 epoch, u8 suppress}`.
pub const UDT_ADVANCE_REPLICATION_EPOCH: u32 = 42;
/// AH → client. `{i32 count, u64 machine_ids[]}`.
pub const UDT_RESERVATIONS_FAILED: u32 = 43;

/// `[C]` `0x600` - the mask both `entity_slots_allocated` and `free_entity_indices` carry.
pub const ENTITY_MASK_STRUCT_SIZE: usize = 0x600;
/// `[C]` `0x100` - the size `FUN_0029A170` passes to `activity_client_send_message`, and the
/// `struct_size` of schema tag `0x80800E98`. Two instruments; see `d1_oracle::activity_host`.
pub const JOIN_REQUEST_STRUCT_SIZE: usize = 0x100;
/// `[C]` `0x1A8` - `local_1e0[424]` in `FUN_01058020`, and `memcpy(..., 0x1a8)` in `0x29BD10`.
pub const JOIN_RESULT_STRUCT_SIZE: usize = 0x1A8;
/// `[C]` `0x6B0` - the `struct_size` of schema tag `0x80800E76`. ⚠ This is the WHOLE struct, not a
/// blob behind an 8-byte prefix; `0xBAFB50`'s `memcpy(world+0x50B0, payload+8, 0x6b0)` is unpacking
/// a simulation-queue element, which is a different object. See `messages::GlobalActivityState`.
pub const GLOBAL_ACTIVITY_STATE_BLOB_SIZE: usize = 0x6B0;
/// `[C]` `0x88` - `{u64 ah_id, char session_id[0x80]}`, and exactly BAP message 7's payload.
pub const START_ACTIVITY_HOST_RESPONSE_SIZE: usize = 0x88;

/// The name for an id, if it is one of the 49.
pub fn name(user_defined_type: u32) -> Option<&'static str> {
    usize::try_from(user_defined_type).ok().and_then(|i| AH_MESSAGE_NAMES.get(i).copied())
}

/// [`name`] with a printable fallback, for diagnostics.
pub fn name_or_unknown(user_defined_type: u32) -> &'static str {
    name(user_defined_type).unwrap_or("<not in the 49-entry table>")
}

/// Which way an id travels, or `None` if it is not one of the 49.
pub fn direction(user_defined_type: u32) -> Option<AhDirection> {
    let id = u8::try_from(user_defined_type).ok()?;
    if usize::from(id) >= AH_MESSAGE_COUNT {
        return None;
    }
    let to_client = AH_TO_CLIENT.contains(&id);
    let to_ah = CLIENT_TO_AH.contains(&id);
    Some(match (to_client, to_ah) {
        (true, true) => AhDirection::Both,
        (true, false) => AhDirection::AhToClient,
        (false, true) => AhDirection::ClientToAh,
        (false, false) => AhDirection::NoHandler,
    })
}

/// Refuse, with a reason, anything an Activity Host must not put on the wire.
///
/// This is the check that stops a stub from "acking" a keepalive with udt 16, which the client
/// has no handler for and would drop in silence - the failure mode that looks exactly like the
/// game never getting there.
pub fn check_sendable(user_defined_type: u32) -> Result<(), AhError> {
    match direction(user_defined_type) {
        None => Err(AhError::UnknownUserDefinedType(user_defined_type)),
        Some(AhDirection::NoHandler) => Err(AhError::NoHandler {
            user_defined_type,
            name: name_or_unknown(user_defined_type),
        }),
        Some(AhDirection::ClientToAh) => Err(AhError::WrongDirection {
            user_defined_type,
            name: name_or_unknown(user_defined_type),
            expected: AhDirection::ClientToAh,
        }),
        Some(AhDirection::AhToClient | AhDirection::Both) => Ok(()),
    }
}

/// The mirror of [`check_sendable`] for the receive path.
pub fn check_receivable(user_defined_type: u32) -> Result<(), AhError> {
    match direction(user_defined_type) {
        None => Err(AhError::UnknownUserDefinedType(user_defined_type)),
        Some(AhDirection::NoHandler) => Err(AhError::NoHandler {
            user_defined_type,
            name: name_or_unknown(user_defined_type),
        }),
        Some(AhDirection::AhToClient) => Err(AhError::WrongDirection {
            user_defined_type,
            name: name_or_unknown(user_defined_type),
            expected: AhDirection::AhToClient,
        }),
        Some(AhDirection::ClientToAh | AhDirection::Both) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The re-derivation's own controls: the table's length and its terminator.
    #[test]
    fn the_id_space_is_forty_nine_entries() {
        assert_eq!(AH_MESSAGE_NAMES.len(), 49);
        // Every entry is named - a NULL inside the table would mean the walk over-ran.
        assert!(AH_MESSAGE_NAMES.iter().all(|n| !n.is_empty()));
    }

    /// ⭐ The two-sided control from the module doc. If either of these ever fails, the claim that
    /// `user_defined_type` and this table are the same id space is dead, and BAP messages 6/7 stop
    /// meaning what `d1-responder` thinks they mean.
    #[test]
    fn udt_eight_and_ten_tie_the_bap_manager_pair_to_this_table() {
        assert_eq!(name(UDT_REQUEST_ACTIVITY_HOST), Some("request_activity_host"));
        assert_eq!(name(UDT_START_ACTIVITY_HOST_RESPONSE), Some("start_activity_host_response"));
        // And the size agreement: BAP message 7 carries `{u64, char[128]}` = 0x88 of payload,
        // which is AH message 10 exactly.
        assert_eq!(START_ACTIVITY_HOST_RESPONSE_SIZE, 8 + d1_bap::activity_host::AH_HOST_TEXT_LEN);
        assert_eq!(
            d1_bap::activity_host::AH_MANAGER_RESPONSE_BODY_LEN,
            1 + START_ACTIVITY_HOST_RESPONSE_SIZE
        );
    }

    #[test]
    fn the_receive_dispatch_table_has_seventeen_entries_and_they_are_disjoint_but_for_incident() {
        assert_eq!(AH_TO_CLIENT.len(), 17);
        assert_eq!(CLIENT_TO_AH.len(), 29, "⭐ 28 until udt 21 was measured on the wire");
        let both: Vec<u8> =
            AH_TO_CLIENT.iter().copied().filter(|i| CLIENT_TO_AH.contains(i)).collect();
        assert_eq!(both, vec![18], "only `incident` may travel both ways");
    }

    /// Every one of the 49 gets exactly one direction, and the ones with none are named.
    ///
    /// ⭐⭐ **INVERTED 2026-08-09.** This used to assert that `client_authoritative_data_update`
    /// was unwired. It is not - the client sends it every frame it has a dirty bit, through an
    /// encoder that never touches `FUN_00299310`. The test now asserts the *opposite* for udt 21
    /// and keeps discriminating on the other four, so it can still fail if someone widens the list
    /// carelessly.
    #[test]
    fn every_id_is_classified_and_the_unwired_ones_are_exactly_the_expected_four() {
        let unwired: Vec<u8> = (0..49u8)
            .filter(|&i| direction(u32::from(i)) == Some(AhDirection::NoHandler))
            .collect();
        assert_eq!(unwired, NO_HANDLER.to_vec());
        assert_eq!(
            unwired.iter().map(|&i| AH_MESSAGE_NAMES[usize::from(i)]).collect::<Vec<_>>(),
            vec![
                "start_activity_host",
                "activity_client_keepalive_response",
                "debug_request_migration",
                "request_script_updates",
            ]
        );
        // ⭐ The inversion, asserted rather than only written down.
        assert_eq!(
            direction(UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE),
            Some(AhDirection::ClientToAh)
        );
        assert!(check_receivable(UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE).is_ok());
        // …and it is still not something an Activity Host may SEND: there is no handler for it on
        // the client and inventing a reply is the udt-16 mistake in a new hat.
        assert!(check_sendable(UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE).is_err());
    }

    /// ⭐ The named regression: a stub that "acks" the keepalive lease is talking to itself.
    #[test]
    fn udt_sixteen_the_keepalive_response_is_refused_as_unsendable() {
        assert_eq!(
            check_sendable(UDT_KEEPALIVE_RESPONSE),
            Err(AhError::NoHandler {
                user_defined_type: 16,
                name: "activity_client_keepalive_response"
            })
        );
        // Positive control on the same call: the message the lease actually replies to nothing
        // with, and a real AH→client id, are treated differently.
        assert!(check_sendable(UDT_JOIN_RESULT).is_ok());
        assert!(check_receivable(UDT_KEEPALIVE_REQUEST).is_ok());
    }

    #[test]
    fn direction_refuses_ids_outside_the_table() {
        assert_eq!(direction(49), None);
        assert_eq!(check_sendable(49), Err(AhError::UnknownUserDefinedType(49)));
        assert_eq!(check_sendable(1000), Err(AhError::UnknownUserDefinedType(1000)));
        // Positive control: 48 is the last real id.
        assert_eq!(name(48), Some("refresh_inspirations"));
    }

    /// The correction to `historicaldocs/design/activity-host-contract.md` §2, asserted so it cannot drift back.
    #[test]
    fn ids_thirty_and_thirty_one_are_not_what_the_contract_doc_says() {
        assert_eq!(name(30), Some("query_entity_slot_authority_mask_per_bubble_response"));
        assert_eq!(name(31), Some("query_entity_slot_authority_mask_response"));
    }

    #[test]
    fn the_four_bap_connection_slots_are_named_from_the_eboot() {
        assert_eq!(CONNECTION_SLOT_NAMES[0], "OUT PRIMARY");
        assert_eq!(CONNECTION_SLOT_NAMES[1], "OUT FAH");
    }
}
