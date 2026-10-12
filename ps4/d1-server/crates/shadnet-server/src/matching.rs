// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The wire shapes and the client-side requirements encoded here are read out of shadPS4 (GPL-2.0).
// See ATTRIBUTION.md. No code is copied.

//! Translation between the vendored protobuf schema and [`crate::rooms`], plus the **emitter
//! guard**.
//!
//! The guard is the point of this module. Three of the client's contracts fail *silently* when a
//! server gets them wrong, and all three are checkable before the bytes leave:
//!
//! | contract | what the client does when we break it | guard |
//! |---|---|---|
//! | `details` must be present | callback fires with a **null data pointer and `errorCode 0`** | [`EmitError::MissingDetails`] |
//! | a member needs `addr` + `port` | reader thread **blocks 5 s** in `RequestSignalingInfos` | [`EmitError::MemberEndpointUnset`] |
//! | a notification needs the recipient's `ctx_id` | `ContextManager::Get` misses; **no log line at all** | [`EmitError::CtxIdUnset`] |
//!
//! Every reply and every notification goes through [`guard_create_join_reply`] /
//! [`guard_room_event`] before it is framed. A server that emitted nothing at all would pass the
//! negative half of each of those checks, so the controls pair every refusal with a positive arm.

use prost::Message;
use shadnet_proto as pb;
use shadnet_wire::ErrorType;

use crate::rooms::{
    BinAttr, Endpoint, IntAttr, Member, Room, RoomGroup, RoomNotification, SearchFilter,
};

/// A reply or notification we refuse to send because the client would mis-handle it in silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// `CreateRoomReply`/`JoinRoomReply` without `details`.
    MissingDetails,
    /// A `MatchingRoomMemberData` whose `addr` is not a dotted quad or whose `port` is 0.
    MemberEndpointUnset(String),
    /// `member_id == 0`, the client's "not a member" sentinel.
    MemberIdZero,
    /// `room_id == 0`, the context's "no room" sentinel.
    RoomIdZero,
    /// A notification with no `ctx_id` - the recipient never called `ContextStart`.
    CtxIdUnset,
    /// A room event shadPS4's `HandleRoomEvent` drops on its `default:` arm.
    EventNotHandledByClient(u16),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDetails => write!(
                f,
                "reply has no `details` - the title's callback would fire with a null data \
                 pointer and errorCode 0"
            ),
            Self::MemberEndpointUnset(npid) => write!(
                f,
                "member '{npid}' has no usable addr/port - the client would block its ShadNet \
                 reader thread for 5 s in RequestSignalingInfos(105)"
            ),
            Self::MemberIdZero => write!(f, "member_id 0 is the client's `not a member` sentinel"),
            Self::RoomIdZero => write!(f, "room_id 0 is the context's `no room` sentinel"),
            Self::CtxIdUnset => write!(
                f,
                "no ctx_id for this recipient - ContextManager::Get would miss and the \
                 notification would vanish with no log line"
            ),
            Self::EventNotHandledByClient(e) => write!(
                f,
                "room event {e:#06x} falls to HandleRoomEvent's default arm and is dropped \
                 without a callback"
            ),
        }
    }
}

impl std::error::Error for EmitError {}

// ── request → room model ─────────────────────────────────────────────────────────────────────

pub fn bin_attrs_from(src: &[pb::MatchingBinAttr]) -> Vec<BinAttr> {
    src.iter()
        .map(|a| BinAttr {
            attr_id: a.attr_id,
            data: a.data.clone(),
            update_date: a.update_date,
            update_member_id: a.update_member_id,
        })
        .collect()
}

pub fn int_attrs_from(src: &[pb::MatchingIntAttr]) -> Vec<IntAttr> {
    src.iter().map(|a| IntAttr { attr_id: a.attr_id, attr_value: a.attr_value }).collect()
}

/// `MatchingRoomGroupConfig` → `MatchingRoomGroup`.
///
/// Group ids are **1-based and positional** - the config list carries no id, and Sony's API numbers
/// groups from 1. `[I]`; the client stores whatever we send and never re-derives it.
pub fn groups_from_configs(configs: &[pb::MatchingRoomGroupConfig]) -> Vec<RoomGroup> {
    configs
        .iter()
        .enumerate()
        .map(|(i, c)| RoomGroup {
            group_id: (i + 1) as u32,
            has_passwd: c.has_passwd,
            has_label: c.has_label,
            label: c.label.clone(),
            slot_count: c.slot_count,
            num_members: 0,
        })
        .collect()
}

pub fn search_filter_from(req: &pb::SearchRoomRequest) -> SearchFilter {
    SearchFilter {
        world_id: req.world_id,
        lobby_id: req.lobby_id,
        flag_filter: req.flag_filter,
        flag_attr: req.flag_attr,
        int_filters: req.int_filters.iter().map(|f| (f.op, f.attr_id, f.attr_value)).collect(),
        bin_filters: req
            .bin_filters
            .iter()
            .map(|f| (f.op, f.attr_id, f.data.clone()))
            .collect(),
        range_start: req.range_filter_start,
        range_max: req.range_filter_max,
    }
}

// ── room model → reply ───────────────────────────────────────────────────────────────────────

fn bin_attrs_to(src: &[BinAttr]) -> Vec<pb::MatchingBinAttr> {
    src.iter()
        .map(|a| pb::MatchingBinAttr {
            attr_id: a.attr_id,
            data: a.data.clone(),
            update_date: a.update_date,
            update_member_id: a.update_member_id,
        })
        .collect()
}

fn int_attrs_to(src: &[IntAttr]) -> Vec<pb::MatchingIntAttr> {
    src.iter()
        .map(|a| pb::MatchingIntAttr { attr_id: a.attr_id, attr_value: a.attr_value })
        .collect()
}

fn groups_to(src: &[RoomGroup]) -> Vec<pb::MatchingRoomGroup> {
    src.iter()
        .map(|g| pb::MatchingRoomGroup {
            group_id: g.group_id,
            has_passwd: g.has_passwd,
            has_label: g.has_label,
            label: g.label.clone(),
            slot_count: g.slot_count,
            num_members: g.num_members,
        })
        .collect()
}

/// Build the per-member submessage, **refusing** anything the client would silently mis-handle.
pub fn member_to_proto(m: &Member) -> Result<pb::MatchingRoomMemberData, EmitError> {
    if m.member_id == 0 {
        return Err(EmitError::MemberIdZero);
    }
    if !m.endpoint.is_usable() {
        return Err(EmitError::MemberEndpointUnset(m.npid.clone()));
    }
    Ok(pb::MatchingRoomMemberData {
        npid: m.npid.clone(),
        member_id: m.member_id,
        team_id: m.team_id,
        is_owner: m.is_owner,
        group: None,
        bin_attrs_internal: bin_attrs_to(&m.bin_attrs_internal),
        join_date: m.join_date,
        nat_type: m.nat_type,
        flag_attr: m.flag_attr,
        addr: m.endpoint.addr.clone(),
        port: m.endpoint.port,
        group_id: m.group_id,
        account_id: m.account_id,
        platform: m.platform,
    })
}

pub fn room_data_internal(room: &Room) -> pb::MatchingRoomDataInternal {
    let cur = room.members.len() as u32;
    pb::MatchingRoomDataInternal {
        public_slots: room.max_slots,
        private_slots: 0,
        open_public_slots: room.max_slots.saturating_sub(cur),
        open_private_slots: 0,
        max_slot: room.max_slots,
        server_id: room.server_id,
        world_id: room.world_id,
        lobby_id: room.lobby_id,
        room_id: room.room_id,
        passwd_slot_mask: room.passwd_slot_mask,
        // ⚠ `joined_slot_mask` is uint32 here and uint64 in `MatchingRoomDataExternal`. One Rust
        // type does not fit both; the widening is explicit at each site rather than hidden in a
        // shared alias.
        joined_slot_mask: room.joined_slot_mask(),
        groups: groups_to(&room.groups),
        flags: room.flags,
        bin_attrs_internal: bin_attrs_to(&room.bin_attrs_internal),
    }
}

pub fn room_data_external(room: &Room) -> pb::MatchingRoomDataExternal {
    let cur = room.members.len() as u32;
    let owner = room.member_by_id(room.owner_member_id);
    pb::MatchingRoomDataExternal {
        max_slot: room.max_slots,
        cur_members: cur,
        flags: room.flags,
        server_id: room.server_id,
        world_id: room.world_id,
        lobby_id: room.lobby_id as u64,
        room_id: room.room_id,
        passwd_slot_mask: room.passwd_slot_mask,
        joined_slot_mask: room.joined_slot_mask() as u64,
        public_slots: room.max_slots,
        private_slots: 0,
        open_public_slots: room.max_slots.saturating_sub(cur),
        open_private_slots: 0,
        groups: groups_to(&room.groups),
        external_search_int_attrs: int_attrs_to(&room.search_int_attrs),
        external_search_bin_attrs: bin_attrs_to(&room.search_bin_attrs),
        external_bin_attrs: bin_attrs_to(&room.ext_bin_attrs),
        owner_npid: owner.map(|m| m.npid.clone()).unwrap_or_default(),
        owner_account_id: owner.map(|m| m.account_id).unwrap_or_default(),
        owner_platform: owner.map(|m| m.platform).unwrap_or_default(),
    }
}

/// ⭐ The **only** way to build `details`. There is no `Default` path, on purpose.
pub fn create_join_details(
    room: &Room,
    me_member_id: u32,
) -> Result<pb::CreateJoinRoomResponse, EmitError> {
    if room.room_id == 0 {
        return Err(EmitError::RoomIdZero);
    }
    if me_member_id == 0 {
        return Err(EmitError::MemberIdZero);
    }
    let mut members = Vec::with_capacity(room.members.len());
    for m in &room.members {
        members.push(member_to_proto(m)?);
    }
    Ok(pb::CreateJoinRoomResponse {
        room_data: Some(room_data_internal(room)),
        members,
        me_member_id,
        owner_member_id: room.owner_member_id,
    })
}

pub fn create_room_reply(room: &Room, me: u32) -> Result<pb::CreateRoomReply, EmitError> {
    let details = create_join_details(room, me)?;
    Ok(pb::CreateRoomReply {
        // These eight scalars are NEVER READ by the client - `DispatchRequestComplete` goes
        // straight to `reply.details()`. They are populated anyway so a packet capture is
        // self-describing, but nothing may depend on them.
        room_id: room.room_id,
        server_id: room.server_id,
        world_id: room.world_id,
        lobby_id: room.lobby_id,
        member_id: me,
        max_slots: room.max_slots,
        flags: room.flags,
        cur_members: room.members.len() as u32,
        details: Some(details),
    })
}

pub fn join_room_reply(room: &Room, me: u32) -> Result<pb::JoinRoomReply, EmitError> {
    let details = create_join_details(room, me)?;
    Ok(pb::JoinRoomReply {
        room_id: room.room_id,
        member_id: me,
        max_slots: room.max_slots,
        flags: room.flags,
        cur_members: room.members.len() as u32,
        details: Some(details),
    })
}

pub fn search_room_reply(
    rooms: &[&Room],
    range_start: u32,
    range_total: u32,
) -> pb::SearchRoomReply {
    pb::SearchRoomReply {
        rooms: rooms.iter().map(|r| room_data_external(r)).collect(),
        range_start,
        range_total,
        range_result: rooms.len() as u32,
    }
}

// ── notifications ────────────────────────────────────────────────────────────────────────────

/// Build one recipient's `NotifyRoomEvent`.
///
/// `ctx_id` is the **recipient's own**, never the actor's. That is the whole reason this takes a
/// parameter instead of reading it off the notification.
pub fn room_event_proto(
    n: &RoomNotification,
    ctx_id: Option<u32>,
) -> Result<pb::NotifyRoomEvent, EmitError> {
    let ctx_id = ctx_id.ok_or(EmitError::CtxIdUnset)?;
    if ctx_id == 0 {
        // `ContextManager::Get(0)` misses: valid ids are 1..10 and 0 is not a wildcard.
        return Err(EmitError::CtxIdUnset);
    }
    if n.room_id == 0 {
        return Err(EmitError::RoomIdZero);
    }
    if !n.event.handled_by_shadps4() {
        return Err(EmitError::EventNotHandledByClient(n.event as u16));
    }
    let member = match &n.member {
        Some(m) => Some(member_to_proto(m)?),
        None => None,
    };
    Ok(pb::NotifyRoomEvent {
        ctx_id,
        room_id: n.room_id,
        event: n.event as u32,
        event_cause: n.cause as u32,
        error_code: n.error_code,
        member,
        bin_attrs: bin_attrs_to(&n.bin_attrs),
        flags: n.flags,
        has_passwd_mask: n.passwd_slot_mask.is_some(),
        passwd_slot_mask: n.passwd_slot_mask.unwrap_or(0),
    })
}

// ── the emitter guards ───────────────────────────────────────────────────────────────────────

/// Refuse a create/join reply the client would mis-handle. Call this on the bytes' way out.
pub fn guard_create_join_reply(details: Option<&pb::CreateJoinRoomResponse>) -> Result<(), EmitError> {
    let d = details.ok_or(EmitError::MissingDetails)?;
    if d.me_member_id == 0 {
        return Err(EmitError::MemberIdZero);
    }
    let rd = d.room_data.as_ref().ok_or(EmitError::MissingDetails)?;
    if rd.room_id == 0 {
        return Err(EmitError::RoomIdZero);
    }
    for m in &d.members {
        if m.member_id == 0 {
            return Err(EmitError::MemberIdZero);
        }
        let ep = Endpoint { addr: m.addr.clone(), port: m.port };
        if !ep.is_usable() {
            return Err(EmitError::MemberEndpointUnset(m.npid.clone()));
        }
    }
    Ok(())
}

/// Refuse a notification the client would drop.
///
/// ⚠ Also refuses an **all-default** proto: `HandleNotification` bails when the blob is empty
/// (`if (blob.empty() && type != WebApiPushEvent) return;`), and a proto3 message whose every field
/// is its default serialises to **zero bytes**. So "the notification with no content" is not
/// delivered-and-ignored, it is never delivered at all.
pub fn guard_room_event(n: &pb::NotifyRoomEvent) -> Result<(), EmitError> {
    if n.ctx_id == 0 {
        return Err(EmitError::CtxIdUnset);
    }
    if n.room_id == 0 {
        return Err(EmitError::RoomIdZero);
    }
    if n.encoded_len() == 0 {
        return Err(EmitError::MissingDetails);
    }
    if let Some(m) = &n.member {
        if m.member_id == 0 {
            return Err(EmitError::MemberIdZero);
        }
        let ep = Endpoint { addr: m.addr.clone(), port: m.port };
        if !ep.is_usable() {
            return Err(EmitError::MemberEndpointUnset(m.npid.clone()));
        }
    }
    Ok(())
}

/// The reply for `RequestSignalingInfos(105)`.
///
/// The client reads **only** `target_ip` and `target_port`, and treats an empty ip or a zero port
/// as a failure - so both are checked here rather than trusted.
pub fn signaling_reply(
    target_npid: &str,
    ep: &Endpoint,
    member_id: u32,
) -> Result<pb::RequestSignalingInfosReply, EmitError> {
    if !ep.is_usable() {
        return Err(EmitError::MemberEndpointUnset(target_npid.to_string()));
    }
    Ok(pb::RequestSignalingInfosReply {
        target_npid: target_npid.to_string(),
        target_ip: ep.addr.clone(),
        target_port: ep.port,
        target_member_id: member_id,
    })
}

/// Map an emitter refusal onto the error byte the client will see. Everything here is our fault,
/// not the client's, so it is `Invalid` - the title cannot act on it, but the run log names it.
pub fn emit_error_type(_e: &EmitError) -> ErrorType {
    ErrorType::Invalid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rooms::{MemberIntent, RoomSpec, RoomTable};
    use shadnet_wire::{EventCause, RoomEvent};

    fn intent(conn: u64, npid: &str) -> MemberIntent {
        MemberIntent {
            conn_id: conn,
            npid: npid.into(),
            team_id: 0,
            endpoint: Endpoint { addr: format!("10.77.0.{conn}"), port: 3074 },
            nat_type: 2,
            flag_attr: 0,
            group_id: 0,
            account_id: 0x1122_3344_5566_7788,
            platform: 7,
            join_date: 42,
            bin_attrs_internal: vec![],
        }
    }

    fn two_member_room() -> (RoomTable, u64) {
        let mut t = RoomTable::new(1);
        let a = t
            .create_room(RoomSpec { max_slots: 4, world_id: 3, ..Default::default() }, intent(1, "A"))
            .unwrap();
        t.join_room(a.room_id, false, intent(2, "B")).unwrap();
        (t, a.room_id)
    }

    // ── details ──────────────────────────────────────────────────────────────────────────────

    /// ⭐ The negative half - and its positive control, because a server that emitted nothing
    /// would pass the negative half on its own.
    #[test]
    fn the_emitter_refuses_details_none_and_accepts_details_some() {
        assert_eq!(guard_create_join_reply(None), Err(EmitError::MissingDetails));

        let (t, room_id) = two_member_room();
        let room = t.get(room_id).unwrap();
        let details = create_join_details(room, 1).unwrap();
        assert!(guard_create_join_reply(Some(&details)).is_ok());

        // and a `details` that exists but is empty is still refused
        let empty = pb::CreateJoinRoomResponse::default();
        assert!(guard_create_join_reply(Some(&empty)).is_err());
    }

    #[test]
    fn every_reply_we_build_carries_details() {
        let (t, room_id) = two_member_room();
        let room = t.get(room_id).unwrap();
        let c = create_room_reply(room, 1).unwrap();
        let j = join_room_reply(room, 2).unwrap();
        assert!(c.details.is_some());
        assert!(j.details.is_some());
        assert!(guard_create_join_reply(c.details.as_ref()).is_ok());
        assert!(guard_create_join_reply(j.details.as_ref()).is_ok());
        // `details` survives a real encode/decode round trip - `has_details()` is what the client
        // actually calls, and prost's `Option` is what makes it true.
        let bytes = c.encode_to_vec();
        let back = pb::CreateRoomReply::decode(bytes.as_slice()).unwrap();
        assert!(back.details.is_some());
        assert_eq!(back.details.unwrap().members.len(), 2);
    }

    #[test]
    fn a_member_without_an_endpoint_is_refused_at_the_emitter_too() {
        let mut m = Member {
            conn_id: 1,
            npid: "A".into(),
            member_id: 1,
            team_id: 0,
            is_owner: true,
            endpoint: Endpoint { addr: "10.77.0.2".into(), port: 3074 },
            nat_type: 2,
            flag_attr: 0,
            group_id: 0,
            join_date: 0,
            account_id: 0,
            platform: 0,
            bin_attrs_internal: vec![],
        };
        assert!(member_to_proto(&m).is_ok()); // positive control
        m.endpoint.port = 0;
        assert_eq!(
            member_to_proto(&m),
            Err(EmitError::MemberEndpointUnset("A".into()))
        );
        m.endpoint = Endpoint { addr: "not-an-ip".into(), port: 3074 };
        assert!(member_to_proto(&m).is_err());
        m.endpoint = Endpoint { addr: "10.77.0.2".into(), port: 3074 };
        m.member_id = 0;
        assert_eq!(member_to_proto(&m), Err(EmitError::MemberIdZero));
    }

    // ── notifications ────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_notification_without_the_recipients_ctx_id_is_refused() {
        let n = RoomNotification {
            targets: vec![1],
            room_id: 5,
            event: RoomEvent::MemberJoined,
            cause: EventCause::ServerOperation,
            error_code: 0,
            member: None,
            bin_attrs: vec![],
            flags: 0,
            passwd_slot_mask: None,
        };
        assert_eq!(room_event_proto(&n, None), Err(EmitError::CtxIdUnset));
        assert_eq!(room_event_proto(&n, Some(0)), Err(EmitError::CtxIdUnset));
        // positive control: a real ctx_id builds
        assert!(room_event_proto(&n, Some(1)).is_ok());
    }

    #[test]
    fn a_room_event_the_client_drops_is_refused_before_it_is_sent() {
        for e in [
            RoomEvent::RoomDestroyed,
            RoomEvent::RoomOwnerChanged,
            RoomEvent::UpdatedRoomMemberDataInternal,
            RoomEvent::UpdatedSignalingOptParam,
        ] {
            let n = RoomNotification {
                targets: vec![1],
                room_id: 5,
                event: e,
                cause: EventCause::ServerOperation,
                error_code: 0,
                member: None,
                bin_attrs: vec![],
                flags: 0,
                passwd_slot_mask: None,
            };
            assert_eq!(
                room_event_proto(&n, Some(1)),
                Err(EmitError::EventNotHandledByClient(e as u16))
            );
        }
        // positive control: the four it does handle build fine
        for e in [
            RoomEvent::MemberJoined,
            RoomEvent::MemberLeft,
            RoomEvent::KickedOut,
            RoomEvent::UpdatedRoomDataInternal,
        ] {
            let n = RoomNotification {
                targets: vec![1],
                room_id: 5,
                event: e,
                cause: EventCause::ServerOperation,
                error_code: 0,
                member: None,
                bin_attrs: vec![],
                flags: 0,
                passwd_slot_mask: None,
            };
            assert!(room_event_proto(&n, Some(1)).is_ok(), "{e:?}");
        }
    }

    /// An all-default proto serialises to zero bytes and `HandleNotification` bails on an empty
    /// blob - so the guard must catch it rather than trusting "we sent something".
    #[test]
    fn an_all_default_notification_encodes_to_zero_bytes_and_is_refused() {
        let empty = pb::NotifyRoomEvent::default();
        assert_eq!(empty.encoded_len(), 0);
        assert!(guard_room_event(&empty).is_err());
        // positive control: a populated one is non-empty and passes
        let (t, room_id) = two_member_room();
        let room = t.get(room_id).unwrap();
        let n = RoomNotification {
            targets: vec![1],
            room_id,
            event: RoomEvent::MemberJoined,
            cause: EventCause::ServerOperation,
            error_code: 0,
            member: Some(room.member_by_id(2).unwrap().clone()),
            bin_attrs: vec![],
            flags: 0,
            passwd_slot_mask: None,
        };
        let proto = room_event_proto(&n, Some(1)).unwrap();
        assert!(proto.encoded_len() > 0);
        assert!(guard_room_event(&proto).is_ok());
    }

    #[test]
    fn member_joined_carries_the_fields_the_client_reads() {
        let (t, room_id) = two_member_room();
        let room = t.get(room_id).unwrap();
        let b = room.member_by_id(2).unwrap();
        let n = RoomNotification {
            targets: vec![1],
            room_id,
            event: RoomEvent::MemberJoined,
            cause: EventCause::ServerOperation,
            error_code: 0,
            member: Some(b.clone()),
            bin_attrs: vec![],
            flags: 0,
            passwd_slot_mask: None,
        };
        let proto = room_event_proto(&n, Some(4)).unwrap();
        // Round-trip, because the client parses bytes and not our struct.
        let back = pb::NotifyRoomEvent::decode(proto.encode_to_vec().as_slice()).unwrap();
        assert_eq!(back.ctx_id, 4);
        assert_eq!(back.event, 0x1101);
        let m = back.member.expect("pb.has_member() is what the client tests first");
        assert_eq!(m.npid, "B");
        assert_eq!(m.member_id, 2);
        assert_eq!(m.addr, "10.77.0.2");
        assert_eq!(m.port, 3074);
        assert_eq!(m.platform, 7);
        assert_eq!(m.account_id, 0x1122_3344_5566_7788);
    }

    // ── replies ──────────────────────────────────────────────────────────────────────────────

    #[test]
    fn room_data_internal_reports_slots_honestly() {
        let (t, room_id) = two_member_room();
        let rd = room_data_internal(t.get(room_id).unwrap());
        assert_eq!(rd.room_id, room_id);
        assert_eq!(rd.max_slot, 4);
        assert_eq!(rd.open_public_slots, 2);
        assert_eq!(rd.world_id, 3);
        assert_ne!(rd.room_id, 0);
    }

    #[test]
    fn search_reply_reports_the_window_and_the_total_separately() {
        let mut t = RoomTable::new(1);
        for i in 1..=5u64 {
            t.create_room(RoomSpec { max_slots: 4, ..Default::default() }, intent(i, "P")).unwrap();
        }
        let f = SearchFilter { range_start: 2, range_max: 2, ..Default::default() };
        let (window, start, total) = t.search(&f);
        let reply = search_room_reply(&window, start, total);
        assert_eq!(reply.rooms.len(), 2);
        assert_eq!(reply.range_result, 2);
        assert_eq!(reply.range_start, 2);
        assert_eq!(reply.range_total, 5);
    }

    #[test]
    fn signaling_reply_refuses_an_unusable_endpoint() {
        let good = Endpoint { addr: "10.77.0.3".into(), port: 3074 };
        assert!(signaling_reply("B", &good, 2).is_ok());
        let bad = Endpoint { addr: "10.77.0.3".into(), port: 0 };
        assert!(signaling_reply("B", &bad, 2).is_err());
    }

    #[test]
    fn group_ids_are_one_based_and_positional() {
        let cfgs = vec![
            pb::MatchingRoomGroupConfig {
                slot_count: 3,
                has_label: true,
                label: b"alpha".to_vec(),
                has_passwd: false,
            },
            pb::MatchingRoomGroupConfig {
                slot_count: 3,
                has_label: false,
                label: vec![],
                has_passwd: true,
            },
        ];
        let groups = groups_from_configs(&cfgs);
        assert_eq!(groups.iter().map(|g| g.group_id).collect::<Vec<_>>(), vec![1, 2]);
        assert!(groups.iter().all(|g| g.group_id != 0));
    }
}
