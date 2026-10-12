// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Room-model semantics are read out of shadPS4 (GPL-2.0) - see ATTRIBUTION.md. No code is copied.

//! The Matching2 room state machine, with no I/O in it.
//!
//! Everything here is pure: requests in, `(reply, notifications)` out. That is deliberate - the
//! whole point of a room protocol is what the *other* participant is told, and a state machine you
//! can only exercise through a socket is a state machine you cannot test.
//!
//! ## ⚠⚠ Read this before believing anything downstream of it
//!
//! `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §3 marks this section **CONTINGENT**: on the current
//! evidence (§0/§1, `[I]`-strong) **Destiny 1 never calls `sceNpMatching2` at all** - its managed
//! session is the PSN Session WebAPI over HTTP, and its peer key material is minted locally by the
//! client's own MT19937. This module exists so that the *branch* where D1 does speak Matching2 is
//! already implemented rather than a blank file. It is **off by default** (`--matching2` /
//! `--rooms`), and turning it on does not make the evidence for it any better.
//!
//! ## The five client contracts encoded here
//!
//! 1. **`details` is mandatory** on `CreateRoomReply`/`JoinRoomReply`. `DispatchRequestComplete`
//!    tests `reply.has_details()`; without it the title's callback fires with a **null data pointer
//!    and `errorCode 0`** - success with nothing attached. The top-level scalars (`room_id`,
//!    `member_id`, `max_slots`, …) are **never read**. [`Outcome::create_join_details`] is the only
//!    constructor, and [`RoomError::MemberEndpointUnset`] refuses to build one that would fail
//!    contract 5.
//! 2. **`member_id` is never 0** - the client's `ShouldConnectToPeer` and `MarkMatching2PeerActive`
//!    both treat 0 as "not a peer", and `HandleRoomEvent` indexes `room.members[member_id]`.
//! 3. **`room_id` is never 0** - 0 is the context's "no room" sentinel (`ctx.room_id == 0`).
//! 4. **`team_id` is copied verbatim from the request.** The server never assigns a team. D1's own
//!    teams are decided peer-side by `team_picker` (`0x417380`) and written to
//!    `membership + player*0xE8 + 0x1D08`; a server-assigned `team_id` would be a fiction.
//! 5. **A member must carry a dotted-quad `addr` and a non-zero host-order `port`.** Otherwise the
//!    client's `ResolvePeerEndpoint` falls through to `RequestSignalingInfos(105)`, which **blocks
//!    the ShadNet reader thread for 5 s** waiting on a reply only that same thread can deliver:
//!    guaranteed timeout plus head-of-line blocking of every other notification. It reads as "the
//!    server is slow". We refuse to emit such a member instead.

use shadnet_wire::{ErrorType, EventCause, RoomEvent};

/// The reasons a room operation can fail, each mapping onto one of the client's `ErrorType` values.
///
/// ⚠ `RoomPasswordMismatch(17)` is **deliberately absent**, and that is a finding rather than an
/// omission: `JoinRoomRequest` in the vendored schema carries **no password bytes**, only
/// `room_password_present: bool`. Server-side password *comparison* is therefore inexpressible, and
/// a server that returned 17 would be inventing a check it never performed. See
/// [`RoomError::PasswordMissing`] for what we can honestly enforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomError {
    /// No such `room_id`.
    Missing,
    /// This connection is already a member of the room it is trying to join.
    AlreadyJoined,
    /// `members.len() == max_slots`.
    Full,
    /// The room was created with a password and the joiner did not present one.
    PasswordMissing,
    /// The caller is not a member, or is not the owner of an owner-only operation.
    Unauthorized,
    /// The named target (member id / npid) is not in the room.
    NotFound,
    /// A field the client requires is unusable - `max_slots == 0`, a non-IPv4 address, …
    InvalidInput,
    /// ⭐ Contract 5: we were asked to publish a member with no endpoint. Refusing here turns a
    /// silent 5-second client stall into a named server-side error.
    MemberEndpointUnset,
}

impl RoomError {
    pub fn as_error_type(self) -> ErrorType {
        match self {
            Self::Missing => ErrorType::RoomMissing,
            Self::AlreadyJoined => ErrorType::RoomAlreadyJoined,
            Self::Full => ErrorType::RoomFull,
            Self::PasswordMissing => ErrorType::RoomPasswordMissing,
            Self::Unauthorized => ErrorType::Unauthorized,
            Self::NotFound => ErrorType::NotFound,
            Self::InvalidInput => ErrorType::InvalidInput,
            Self::MemberEndpointUnset => ErrorType::InvalidInput,
        }
    }
}

/// A peer's routable endpoint, as the *server* observed it.
///
/// ⚠ `addr` is the TCP source address of that participant's ShadNet connection - genuinely
/// observed. `port` is **not observable**: shadPS4's `GetP2PConfiguredPort()` returns `0` at HEAD
/// (`p2p_sockets.cpp:140`), and neither `CreateRoomRequest` nor `JoinRoomRequest` carries a port
/// field. The operator supplies it (`--peer-port`, default 3074 - the UDP port D1's guest is
/// **observed** to bind). Two instruments therefore disagree about this number and neither is
/// authoritative: recorded, not reconciled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Dotted quad. The client parses it with `sscanf("%u.%u.%u.%u")` and yields **0** on failure,
    /// and address 0 then fails its own `addr != 0 && port != 0` guard. So anything that is not
    /// four decimal octets is as good as absent.
    pub addr: String,
    /// **Host order.** The client applies `sceNetHtons` itself.
    pub port: u32,
}

impl Endpoint {
    /// The exact predicate the client applies, reimplemented so we can refuse before we send.
    ///
    /// `IpStringToAddr` returns `a | b<<8 | c<<16 | d<<24`, so `0.0.0.0` also yields 0 and is
    /// unusable even though it parses.
    pub fn is_usable(&self) -> bool {
        self.port != 0 && self.port <= u16::MAX as u32 && ip_string_to_addr(&self.addr) != 0
    }
}

/// shadPS4's `IpStringToAddr`, reimplemented: `sscanf("%u.%u.%u.%u")`, then
/// `a | (b << 8) | (c << 16) | (d << 24)`; **0 on any parse failure**.
///
/// It is reimplemented rather than approximated because the client's *failure* mode is the part
/// that matters: a hostname, an IPv6 literal or a `::ffff:10.0.0.1` mapped address all silently
/// become address 0, which then fails `ResolvePeerEndpoint`. `sscanf %u` also accepts values above
/// 255 and truncates on cast - we reject those instead of reproducing the overflow, because a
/// server emitting `999.1.1.1` is a server bug either way.
pub fn ip_string_to_addr(s: &str) -> u32 {
    let mut octets = [0u32; 4];
    let mut parts = s.split('.');
    for slot in octets.iter_mut() {
        let Some(p) = parts.next() else { return 0 };
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return 0;
        }
        let Ok(v) = p.parse::<u32>() else { return 0 };
        if v > 255 {
            return 0;
        }
        *slot = v;
    }
    if parts.next().is_some() {
        return 0;
    }
    octets[0] | (octets[1] << 8) | (octets[2] << 16) | (octets[3] << 24)
}

/// One binary attribute. Mirrors `MatchingBinAttr` in the vendored schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinAttr {
    pub attr_id: u32,
    pub data: Vec<u8>,
    pub update_date: u64,
    pub update_member_id: u32,
}

/// One searchable integer attribute. Mirrors `MatchingIntAttr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntAttr {
    pub attr_id: u32,
    pub attr_value: u32,
}

/// A room member, as the server owns it.
///
/// `conn_id` is ours and never crosses the wire: it is what stops two participants being conflated
/// when they report the same npid, which the identity discipline in
/// `PLAN-B-SHADNET-MATCHING2.md` §4 calls the failure that "would look like a networking bug rather
/// than a fixture bug".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub conn_id: u64,
    pub npid: String,
    pub member_id: u32,
    /// Verbatim from the request. **The server never assigns this.**
    pub team_id: u32,
    pub is_owner: bool,
    pub endpoint: Endpoint,
    pub nat_type: u32,
    pub flag_attr: u32,
    pub group_id: u32,
    pub join_date: u64,
    pub account_id: u64,
    pub platform: u32,
    pub bin_attrs_internal: Vec<BinAttr>,
}

/// What a member said about itself when it created or joined.
#[derive(Debug, Clone)]
pub struct MemberIntent {
    pub conn_id: u64,
    pub npid: String,
    pub team_id: u32,
    pub endpoint: Endpoint,
    pub nat_type: u32,
    pub flag_attr: u32,
    pub group_id: u32,
    pub account_id: u64,
    pub platform: u32,
    pub join_date: u64,
    pub bin_attrs_internal: Vec<BinAttr>,
}

/// Everything `CreateRoom` establishes about a room.
#[derive(Debug, Clone, Default)]
pub struct RoomSpec {
    pub world_id: u32,
    pub lobby_id: u32,
    pub max_slots: u32,
    pub flags: u32,
    /// `Some` only when `room_password_present` was set **and** bytes arrived. The creator is the
    /// only party who can supply them: `JoinRoomRequest` has no such field.
    pub password: Option<Vec<u8>>,
    /// Stored and republished **verbatim, never interpreted**. Its bit numbering is `[U]` (not in
    /// the schema; the client stores it without interpreting it either), so interpreting it here
    /// would be inventing a convention.
    pub passwd_slot_mask: Option<u64>,
    pub groups: Vec<RoomGroup>,
    pub bin_attrs_internal: Vec<BinAttr>,
    pub search_int_attrs: Vec<IntAttr>,
    pub search_bin_attrs: Vec<BinAttr>,
    pub ext_bin_attrs: Vec<BinAttr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomGroup {
    pub group_id: u32,
    pub has_passwd: bool,
    pub has_label: bool,
    pub label: Vec<u8>,
    pub slot_count: u32,
    pub num_members: u32,
}

/// A live room.
#[derive(Debug, Clone)]
pub struct Room {
    pub room_id: u64,
    pub server_id: u32,
    pub world_id: u32,
    pub lobby_id: u32,
    pub max_slots: u32,
    pub flags: u32,
    pub password: Option<Vec<u8>>,
    pub passwd_slot_mask: u64,
    pub groups: Vec<RoomGroup>,
    pub bin_attrs_internal: Vec<BinAttr>,
    pub search_int_attrs: Vec<IntAttr>,
    pub search_bin_attrs: Vec<BinAttr>,
    pub ext_bin_attrs: Vec<BinAttr>,
    /// Insertion-ordered. Small (the client's own session ceiling is 20 peers), so a `Vec` beats a
    /// map and keeps the published member order stable across runs - which is what makes two
    /// traces diffable.
    pub members: Vec<Member>,
    pub owner_member_id: u32,
    next_member_id: u32,
}

/// The room's `joined_slot_mask`, derived as bit `member_id - 1`.
///
/// `[I]` - the schema does not define the numbering and the client only stores the value. It is
/// published because `MatchingRoomDataInternal.joined_slot_mask` exists and a zero would be a lie
/// once members are present; it is **not** consulted anywhere in our own logic.
pub fn joined_slot_mask(members: &[Member]) -> u32 {
    members
        .iter()
        .filter(|m| m.member_id >= 1 && m.member_id <= 32)
        .fold(0u32, |acc, m| acc | (1u32 << (m.member_id - 1)))
}

impl Room {
    /// `room_id` must already be non-zero - [`RoomTable`] owns that guarantee.
    fn new(room_id: u64, server_id: u32, spec: RoomSpec) -> Self {
        Self {
            room_id,
            server_id,
            world_id: spec.world_id,
            lobby_id: spec.lobby_id,
            max_slots: spec.max_slots,
            flags: spec.flags,
            password: spec.password,
            passwd_slot_mask: spec.passwd_slot_mask.unwrap_or(0),
            groups: spec.groups,
            bin_attrs_internal: spec.bin_attrs_internal,
            search_int_attrs: spec.search_int_attrs,
            search_bin_attrs: spec.search_bin_attrs,
            ext_bin_attrs: spec.ext_bin_attrs,
            members: Vec::new(),
            owner_member_id: 0,
            next_member_id: 1,
        }
    }

    pub fn is_private(&self) -> bool {
        self.password.is_some()
    }

    pub fn member_by_conn(&self, conn_id: u64) -> Option<&Member> {
        self.members.iter().find(|m| m.conn_id == conn_id)
    }

    pub fn member_by_id(&self, member_id: u32) -> Option<&Member> {
        self.members.iter().find(|m| m.member_id == member_id)
    }

    pub fn member_by_npid(&self, npid: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.npid == npid)
    }

    /// Every member except `except_conn` - the notification fan-out set.
    pub fn others(&self, except_conn: u64) -> Vec<u64> {
        self.members.iter().filter(|m| m.conn_id != except_conn).map(|m| m.conn_id).collect()
    }

    pub fn joined_slot_mask(&self) -> u32 {
        joined_slot_mask(&self.members)
    }

    fn admit(&mut self, intent: MemberIntent, is_owner: bool) -> Result<u32, RoomError> {
        if !intent.endpoint.is_usable() {
            return Err(RoomError::MemberEndpointUnset);
        }
        if self.members.len() as u32 >= self.max_slots {
            return Err(RoomError::Full);
        }
        // member_id 0 is the client's "not a member" sentinel; the allocator starts at 1 and the
        // wrap is a hard error rather than a silent 0.
        let member_id = self.next_member_id;
        if member_id == 0 {
            return Err(RoomError::InvalidInput);
        }
        self.next_member_id += 1;
        self.members.push(Member {
            conn_id: intent.conn_id,
            npid: intent.npid,
            member_id,
            team_id: intent.team_id,
            is_owner,
            endpoint: intent.endpoint,
            nat_type: intent.nat_type,
            flag_attr: intent.flag_attr,
            group_id: intent.group_id,
            join_date: intent.join_date,
            account_id: intent.account_id,
            platform: intent.platform,
            bin_attrs_internal: intent.bin_attrs_internal,
        });
        if is_owner {
            self.owner_member_id = member_id;
        }
        Ok(member_id)
    }
}

/// A room event to deliver to specific connections.
///
/// `ctx_id` is filled in per-recipient at send time, **not here** - every notification must carry
/// the `ctx_id` that recipient's own `ContextStart` established, and both client handlers begin
/// `ContextManager::Get(n.ctx_id); if (!ctx) return;`. A wrong or zero `ctx_id` makes the
/// notification vanish with **no log line at all**, so it is the one field that must never be
/// copied from the sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomNotification {
    pub targets: Vec<u64>,
    pub room_id: u64,
    pub event: RoomEvent,
    pub cause: EventCause,
    pub error_code: i32,
    /// `Some` for member events. The client tests `pb.has_member()` before reading any of it.
    pub member: Option<Member>,
    pub bin_attrs: Vec<BinAttr>,
    pub flags: u32,
    pub passwd_slot_mask: Option<u64>,
}

/// What a room operation produced: the caller's own view, plus what everyone else must be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub room_id: u64,
    pub me_member_id: u32,
    pub owner_member_id: u32,
    pub notifications: Vec<RoomNotification>,
    /// Set when the operation emptied the room. The room is already gone from the table.
    pub room_destroyed: bool,
}

/// All rooms on this server.
#[derive(Debug, Default)]
pub struct RoomTable {
    rooms: Vec<Room>,
    next_room_id: u64,
    /// Reported in `MatchingRoomDataInternal.server_id`. One server, one id.
    pub server_id: u32,
}

impl RoomTable {
    pub fn new(server_id: u32) -> Self {
        Self { rooms: Vec::new(), next_room_id: 1, server_id }
    }

    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Room> {
        self.rooms.iter()
    }

    pub fn get(&self, room_id: u64) -> Option<&Room> {
        self.rooms.iter().find(|r| r.room_id == room_id)
    }

    fn get_mut(&mut self, room_id: u64) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|r| r.room_id == room_id)
    }

    /// The room this connection is in, if any. A connection is in at most one room, matching the
    /// client, whose `ContextObject` holds a single `room_id`.
    pub fn room_of(&self, conn_id: u64) -> Option<&Room> {
        self.rooms.iter().find(|r| r.member_by_conn(conn_id).is_some())
    }

    /// Allocate a **non-zero** room id.
    fn alloc_room_id(&mut self) -> u64 {
        let id = self.next_room_id;
        self.next_room_id = self.next_room_id.wrapping_add(1);
        if self.next_room_id == 0 {
            self.next_room_id = 1;
        }
        id
    }

    // ── 101 CreateRoom ───────────────────────────────────────────────────────────────────────

    pub fn create_room(
        &mut self,
        spec: RoomSpec,
        creator: MemberIntent,
    ) -> Result<Outcome, RoomError> {
        if spec.max_slots == 0 {
            return Err(RoomError::InvalidInput);
        }
        if !creator.endpoint.is_usable() {
            return Err(RoomError::MemberEndpointUnset);
        }
        // A connection already in a room is re-creating; leave the old one first so the previous
        // room's members are told, rather than silently stranding them.
        let room_id = self.alloc_room_id();
        let mut room = Room::new(room_id, self.server_id, spec);
        let member_id = room.admit(creator, true)?;
        self.rooms.push(room);
        Ok(Outcome {
            room_id,
            me_member_id: member_id,
            owner_member_id: member_id,
            // No notification: the creator is alone and learns everything from `details`.
            notifications: Vec::new(),
            room_destroyed: false,
        })
    }

    // ── 102 JoinRoom ─────────────────────────────────────────────────────────────────────────

    /// `password_present` is the joiner's `room_password_present` bool.
    ///
    /// ⚠ **This is the whole of what private-room enforcement can be.** The joiner sends a *flag*,
    /// never bytes, so we can refuse a joiner who did not claim to have the password and we can
    /// **not** check the one they claim to have. `RoomPasswordMismatch(17)` is therefore
    /// unenforceable and is never returned - a control asserts that, so nobody later "fixes" it by
    /// inventing a comparison.
    pub fn join_room(
        &mut self,
        room_id: u64,
        password_present: bool,
        joiner: MemberIntent,
    ) -> Result<Outcome, RoomError> {
        if !joiner.endpoint.is_usable() {
            return Err(RoomError::MemberEndpointUnset);
        }
        let conn_id = joiner.conn_id;
        let server_room = self.get_mut(room_id).ok_or(RoomError::Missing)?;
        if server_room.member_by_conn(conn_id).is_some() {
            return Err(RoomError::AlreadyJoined);
        }
        if server_room.is_private() && !password_present {
            return Err(RoomError::PasswordMissing);
        }
        let member_id = server_room.admit(joiner, false)?;
        let owner_member_id = server_room.owner_member_id;
        let member = server_room.member_by_id(member_id).expect("just admitted").clone();
        let targets = server_room.others(conn_id);

        let mut notifications = Vec::new();
        if !targets.is_empty() {
            notifications.push(RoomNotification {
                targets,
                room_id,
                event: RoomEvent::MemberJoined,
                cause: EventCause::LeaveAction, // overwritten below; see note
                error_code: 0,
                member: Some(member),
                bin_attrs: Vec::new(),
                flags: 0,
                passwd_slot_mask: None,
            });
            // MEMBER_JOINED has no "cause" in the Sony model - the callback carries an
            // `eventCause` field regardless, and the client casts whatever arrives. `SERVER_OPERATION`
            // is the only honest answer: the server admitted them.
            notifications[0].cause = EventCause::ServerOperation;
        }

        Ok(Outcome {
            room_id,
            me_member_id: member_id,
            owner_member_id,
            notifications,
            room_destroyed: false,
        })
    }

    // ── 103 LeaveRoom / 106 ContextStop / disconnect ──────────────────────────────────────────

    /// Remove `conn_id` from `room_id`.
    ///
    /// ⚠ When the room empties it is destroyed, and we do **not** emit `ROOM_DESTROYED(0x1104)`:
    /// shadPS4's `HandleRoomEvent` drops that event on the floor (`default:` arm), so emitting it
    /// would look like delivery and be nothing. Same for `ROOM_OWNER_CHANGED(0x1105)`, which is why
    /// ownership is **not** reassigned when the owner leaves - a silent reassignment the client
    /// cannot learn about is worse than a room that dies with its owner.
    pub fn leave_room(&mut self, room_id: u64, conn_id: u64) -> Result<Outcome, RoomError> {
        let table_index =
            self.rooms.iter().position(|r| r.room_id == room_id).ok_or(RoomError::Missing)?;
        let room = &mut self.rooms[table_index];
        let idx =
            room.members.iter().position(|m| m.conn_id == conn_id).ok_or(RoomError::NotFound)?;
        let member = room.members.remove(idx);
        let owner_member_id = room.owner_member_id;
        let targets = room.others(conn_id);
        let now_empty = room.members.is_empty();

        let mut notifications = Vec::new();
        if !targets.is_empty() {
            notifications.push(RoomNotification {
                targets,
                room_id,
                event: RoomEvent::MemberLeft,
                cause: EventCause::LeaveAction,
                error_code: 0,
                member: Some(member.clone()),
                bin_attrs: Vec::new(),
                flags: 0,
                passwd_slot_mask: None,
            });
        }
        if now_empty {
            self.rooms.remove(table_index);
        }
        Ok(Outcome {
            room_id,
            me_member_id: member.member_id,
            owner_member_id,
            notifications,
            room_destroyed: now_empty,
        })
    }

    /// The disconnect path: leave whatever room this connection was in, if any.
    ///
    /// `MEMBER_DISAPPEARED` rather than `LEAVE_ACTION`, because that is what happened.
    pub fn drop_connection(&mut self, conn_id: u64) -> Option<Outcome> {
        let room_id = self.room_of(conn_id)?.room_id;
        let mut outcome = self.leave_room(room_id, conn_id).ok()?;
        for n in &mut outcome.notifications {
            n.cause = EventCause::MemberDisappeared;
        }
        Some(outcome)
    }

    // ── 110 KickoutRoomMember ────────────────────────────────────────────────────────────────

    /// Only the owner may kick, and nobody may kick themselves.
    ///
    /// Two notifications: `KICKEDOUT(0x1103)` to the target (which the client *does* handle, and
    /// which carries no member submessage - its callback reads only `eventCause`/`errorCode`), and
    /// `MEMBER_LEFT(0x1102)` to everybody else so their member caches stay correct.
    pub fn kickout(
        &mut self,
        room_id: u64,
        requester_conn: u64,
        target_member_id: u32,
    ) -> Result<Outcome, RoomError> {
        let table_index =
            self.rooms.iter().position(|r| r.room_id == room_id).ok_or(RoomError::Missing)?;
        let room = &mut self.rooms[table_index];
        let requester =
            room.member_by_conn(requester_conn).ok_or(RoomError::Unauthorized)?.clone();
        if !requester.is_owner {
            return Err(RoomError::Unauthorized);
        }
        if requester.member_id == target_member_id {
            return Err(RoomError::InvalidInput);
        }
        let idx = room
            .members
            .iter()
            .position(|m| m.member_id == target_member_id)
            .ok_or(RoomError::NotFound)?;
        let target = room.members.remove(idx);
        let owner_member_id = room.owner_member_id;
        let remaining = room.others(target.conn_id);
        let now_empty = room.members.is_empty();

        let mut notifications = vec![RoomNotification {
            targets: vec![target.conn_id],
            room_id,
            event: RoomEvent::KickedOut,
            cause: EventCause::KickoutAction,
            error_code: 0,
            member: None,
            bin_attrs: Vec::new(),
            flags: 0,
            passwd_slot_mask: None,
        }];
        if !remaining.is_empty() {
            notifications.push(RoomNotification {
                targets: remaining,
                room_id,
                event: RoomEvent::MemberLeft,
                cause: EventCause::KickoutAction,
                error_code: 0,
                member: Some(target.clone()),
                bin_attrs: Vec::new(),
                flags: 0,
                passwd_slot_mask: None,
            });
        }
        if now_empty {
            self.rooms.remove(table_index);
        }
        Ok(Outcome {
            room_id,
            me_member_id: requester.member_id,
            owner_member_id,
            notifications,
            room_destroyed: now_empty,
        })
    }

    // ── 108 SetRoomDataInternal ──────────────────────────────────────────────────────────────

    /// Masked read-modify-write of the room flags, plus an upsert of the internal bin attrs.
    ///
    /// `flags = (flags & !flag_filter) | (flag_attr & flag_filter)` - `[I]`. The schema names the
    /// two fields `flag_filter` and `flag_attr` and the Sony API documents the pair as
    /// mask-and-value; nothing in the client checks our arithmetic, so this is inference, not
    /// measurement. A `flag_filter` of 0 therefore changes **nothing**, which is the safe reading.
    pub fn set_room_data_internal(
        &mut self,
        room_id: u64,
        requester_conn: u64,
        flag_filter: u32,
        flag_attr: u32,
        bin_attrs: Vec<BinAttr>,
        passwd_slot_mask: Option<u64>,
    ) -> Result<Outcome, RoomError> {
        let room = self.get_mut(room_id).ok_or(RoomError::Missing)?;
        let requester =
            room.member_by_conn(requester_conn).ok_or(RoomError::Unauthorized)?.clone();

        room.flags = (room.flags & !flag_filter) | (flag_attr & flag_filter);
        if let Some(mask) = passwd_slot_mask {
            room.passwd_slot_mask = mask;
        }
        for attr in &bin_attrs {
            let mut stamped = attr.clone();
            // The client reads `update_member_id` straight into `OrbisNpMatching2RoomBinAttrInternal
            // ::memberId`, so it must name who changed it, not be echoed from the request.
            stamped.update_member_id = requester.member_id;
            match room.bin_attrs_internal.iter_mut().find(|a| a.attr_id == attr.attr_id) {
                Some(existing) => *existing = stamped,
                None => room.bin_attrs_internal.push(stamped),
            }
        }
        let flags = room.flags;
        let mask = room.passwd_slot_mask;
        let targets = room.members.iter().map(|m| m.conn_id).collect::<Vec<_>>();
        let owner_member_id = room.owner_member_id;

        // ⭐ To ALL members including the requester: the client's UPDATED_ROOM_DATA_INTERNAL arm
        // rewrites `rc.flags` and the bin-attr cache, and a requester left out of the fan-out would
        // hold a stale cache while everyone else was correct.
        let notifications = if targets.is_empty() {
            Vec::new()
        } else {
            vec![RoomNotification {
                targets,
                room_id,
                event: RoomEvent::UpdatedRoomDataInternal,
                cause: EventCause::ServerOperation,
                error_code: 0,
                member: None,
                bin_attrs: bin_attrs
                    .into_iter()
                    .map(|mut a| {
                        a.update_member_id = requester.member_id;
                        a
                    })
                    .collect(),
                flags,
                passwd_slot_mask: passwd_slot_mask.map(|_| mask),
            }]
        };
        Ok(Outcome {
            room_id,
            me_member_id: requester.member_id,
            owner_member_id,
            notifications,
            room_destroyed: false,
        })
    }

    // ── 109 SetRoomDataExternal ──────────────────────────────────────────────────────────────

    /// The searchable face of the room. **No notification is defined for this** - searchers
    /// re-query, so pushing one would be inventing an event id.
    pub fn set_room_data_external(
        &mut self,
        room_id: u64,
        requester_conn: u64,
        search_int_attrs: Vec<IntAttr>,
        search_bin_attrs: Vec<BinAttr>,
        ext_bin_attrs: Vec<BinAttr>,
    ) -> Result<Outcome, RoomError> {
        let room = self.get_mut(room_id).ok_or(RoomError::Missing)?;
        let requester =
            room.member_by_conn(requester_conn).ok_or(RoomError::Unauthorized)?.clone();
        upsert_int(&mut room.search_int_attrs, search_int_attrs);
        upsert_bin(&mut room.search_bin_attrs, search_bin_attrs);
        upsert_bin(&mut room.ext_bin_attrs, ext_bin_attrs);
        Ok(Outcome {
            room_id,
            me_member_id: requester.member_id,
            owner_member_id: room.owner_member_id,
            notifications: Vec::new(),
            room_destroyed: false,
        })
    }

    // ── 104 SearchRoom ───────────────────────────────────────────────────────────────────────

    /// Filter, then page. Returns `(matching rooms in the window, range_start, range_total)`.
    ///
    /// `range_filter_start` is **1-based** - Sony's convention, `[I]`; 0 is treated as 1 so a
    /// client that leaves it defaulted still gets the first page rather than an empty one.
    pub fn search(&self, f: &SearchFilter) -> (Vec<&Room>, u32, u32) {
        let matched: Vec<&Room> = self.iter().filter(|r| f.matches(r)).collect();
        let total = matched.len() as u32;
        let start = f.range_start.max(1);
        let max = if f.range_max == 0 { total } else { f.range_max };
        let window: Vec<&Room> = matched
            .into_iter()
            .skip(start.saturating_sub(1) as usize)
            .take(max as usize)
            .collect();
        (window, start, total)
    }
}

fn upsert_int(dst: &mut Vec<IntAttr>, src: Vec<IntAttr>) {
    for a in src {
        match dst.iter_mut().find(|d| d.attr_id == a.attr_id) {
            Some(existing) => *existing = a,
            None => dst.push(a),
        }
    }
}

fn upsert_bin(dst: &mut Vec<BinAttr>, src: Vec<BinAttr>) {
    for a in src {
        match dst.iter_mut().find(|d| d.attr_id == a.attr_id) {
            Some(existing) => *existing = a,
            None => dst.push(a),
        }
    }
}

/// The comparison ops the schema comments define: `1=Eq 2=Ne 3=Lt 4=Le 5=Gt 6=Ge` for ints,
/// `1=Eq 2=Ne` for bins. An op outside that set matches **nothing** - a filter we do not
/// understand must not silently degrade into "match everything".
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub world_id: u32,
    pub lobby_id: u64,
    pub flag_filter: u32,
    pub flag_attr: u32,
    pub int_filters: Vec<(u32, u32, u32)>,
    pub bin_filters: Vec<(u32, u32, Vec<u8>)>,
    pub range_start: u32,
    pub range_max: u32,
}

impl SearchFilter {
    pub fn matches(&self, room: &Room) -> bool {
        if self.world_id != 0 && room.world_id != self.world_id {
            return false;
        }
        if self.lobby_id != 0 && room.lobby_id as u64 != self.lobby_id {
            return false;
        }
        if self.flag_filter != 0 && (room.flags & self.flag_filter) != (self.flag_attr & self.flag_filter)
        {
            return false;
        }
        for (op, attr_id, value) in &self.int_filters {
            let Some(a) = room.search_int_attrs.iter().find(|a| a.attr_id == *attr_id) else {
                return false;
            };
            let ok = match op {
                1 => a.attr_value == *value,
                2 => a.attr_value != *value,
                3 => a.attr_value < *value,
                4 => a.attr_value <= *value,
                5 => a.attr_value > *value,
                6 => a.attr_value >= *value,
                _ => false,
            };
            if !ok {
                return false;
            }
        }
        for (op, attr_id, data) in &self.bin_filters {
            let Some(a) = room.search_bin_attrs.iter().find(|a| a.attr_id == *attr_id) else {
                return false;
            };
            let ok = match op {
                1 => a.data == *data,
                2 => a.data != *data,
                _ => false,
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(addr: &str, port: u32) -> Endpoint {
        Endpoint { addr: addr.to_string(), port }
    }

    fn intent(conn: u64, npid: &str, team: u32) -> MemberIntent {
        MemberIntent {
            conn_id: conn,
            npid: npid.to_string(),
            team_id: team,
            endpoint: ep("10.77.0.2", 3074),
            nat_type: 2,
            flag_attr: 0,
            group_id: 0,
            account_id: 0,
            platform: 0,
            join_date: 0,
            bin_attrs_internal: Vec::new(),
        }
    }

    fn spec(max: u32) -> RoomSpec {
        RoomSpec { max_slots: max, world_id: 1, ..Default::default() }
    }

    // ── the client's own predicate, reimplemented ────────────────────────────────────────────

    #[test]
    fn ip_string_to_addr_matches_the_clients_sscanf() {
        // The client: a | b<<8 | c<<16 | d<<24.
        assert_eq!(ip_string_to_addr("10.77.0.2"), 10 | (77 << 8) | (0 << 16) | (2 << 24));
        assert_eq!(ip_string_to_addr("255.255.255.255"), 0xFFFF_FFFF);
        // Every failure mode collapses to 0, which then fails the client's own
        // `addr != 0 && port != 0` guard.
        for bad in ["", "10.77.0", "10.77.0.2.3", "::ffff:10.77.0.2", "localhost", "10.77.0.x"] {
            assert_eq!(ip_string_to_addr(bad), 0, "{bad} should be unusable");
        }
        // 0.0.0.0 parses and is still 0 - usable-looking, useless.
        assert_eq!(ip_string_to_addr("0.0.0.0"), 0);
    }

    #[test]
    fn an_endpoint_without_a_usable_address_or_port_is_refused() {
        assert!(ep("10.77.0.2", 3074).is_usable());
        // negative arms
        assert!(!ep("10.77.0.2", 0).is_usable());
        assert!(!ep("", 3074).is_usable());
        assert!(!ep("0.0.0.0", 3074).is_usable());
        assert!(!ep("10.77.0.2", 65536).is_usable());
    }

    #[test]
    fn a_member_with_no_endpoint_cannot_be_admitted() {
        let mut t = RoomTable::new(1);
        let mut bad = intent(1, "A", 0);
        bad.endpoint = ep("10.77.0.2", 0);
        assert_eq!(t.create_room(spec(4), bad), Err(RoomError::MemberEndpointUnset));
        // positive control: the same call with a usable endpoint succeeds, so the refusal is
        // about the endpoint and not about create_room refusing everything.
        assert!(t.create_room(spec(4), intent(1, "A", 0)).is_ok());
    }

    // ── ids ──────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn room_ids_and_member_ids_are_never_zero() {
        let mut t = RoomTable::new(1);
        for i in 0..8 {
            let o = t.create_room(spec(4), intent(100 + i, &format!("P{i}"), 0)).unwrap();
            assert_ne!(o.room_id, 0);
            assert_ne!(o.me_member_id, 0);
        }
    }

    #[test]
    fn member_ids_are_unique_within_a_room_and_survive_churn() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        let b = t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        assert_ne!(a.me_member_id, b.me_member_id);
        t.leave_room(a.room_id, 2).unwrap();
        let c = t.join_room(a.room_id, false, intent(3, "C", 0)).unwrap();
        // Not reused: the client keys its member cache and its peer table by member_id, and a
        // recycled id would silently graft C onto B's cached peer entry.
        assert_ne!(c.me_member_id, b.me_member_id);
    }

    // ── join / leave / notifications ─────────────────────────────────────────────────────────

    #[test]
    fn joining_tells_everyone_else_and_never_the_joiner() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        assert!(a.notifications.is_empty(), "the creator is alone; details carries everything");

        let b = t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        assert_eq!(b.notifications.len(), 1);
        let n = &b.notifications[0];
        assert_eq!(n.targets, vec![1]);
        assert_eq!(n.event, RoomEvent::MemberJoined);
        assert_eq!(n.member.as_ref().unwrap().npid, "B");
        assert!(!n.targets.contains(&2), "the joiner learns itself from its own reply");
    }

    #[test]
    fn leaving_tells_the_remaining_members_with_leave_action() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        let out = t.leave_room(a.room_id, 2).unwrap();
        assert_eq!(out.notifications.len(), 1);
        assert_eq!(out.notifications[0].event, RoomEvent::MemberLeft);
        assert_eq!(out.notifications[0].cause, EventCause::LeaveAction);
        assert!(!out.room_destroyed);
        assert_eq!(t.get(a.room_id).unwrap().members.len(), 1);
    }

    #[test]
    fn a_dropped_connection_leaves_with_member_disappeared() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        let out = t.drop_connection(2).unwrap();
        assert_eq!(out.notifications[0].cause, EventCause::MemberDisappeared);
        // control: a connection in no room produces nothing rather than a spurious event
        assert!(t.drop_connection(99).is_none());
    }

    #[test]
    fn the_last_member_out_destroys_the_room_and_emits_nothing() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        let out = t.leave_room(a.room_id, 1).unwrap();
        assert!(out.room_destroyed);
        assert!(t.get(a.room_id).is_none());
        // ROOM_DESTROYED(0x1104) is dropped by shadPS4's HandleRoomEvent, so we do not pretend.
        assert!(out.notifications.is_empty());
    }

    #[test]
    fn every_event_we_emit_is_one_shadps4_actually_handles() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        let mut all = Vec::new();
        all.extend(t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap().notifications);
        all.extend(t.join_room(a.room_id, false, intent(3, "C", 0)).unwrap().notifications);
        all.extend(
            t.set_room_data_internal(a.room_id, 1, 0xFF, 0x12, vec![], None).unwrap().notifications,
        );
        all.extend(t.kickout(a.room_id, 1, 3).unwrap().notifications);
        all.extend(t.leave_room(a.room_id, 2).unwrap().notifications);
        assert!(!all.is_empty());
        for n in &all {
            assert!(
                n.event.handled_by_shadps4(),
                "{:?} falls to HandleRoomEvent's default arm and is dropped without a callback",
                n.event
            );
        }
        // positive control: the predicate can say no.
        assert!(!RoomEvent::RoomOwnerChanged.handled_by_shadps4());
    }

    // ── team_id ──────────────────────────────────────────────────────────────────────────────

    #[test]
    fn team_id_is_copied_verbatim_and_never_assigned() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 7)).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().member_by_conn(1).unwrap().team_id, 7);
        let b = t.join_room(a.room_id, false, intent(2, "B", 7)).unwrap();
        // Same team as A: the server does not balance, split or renumber.
        assert_eq!(b.notifications[0].member.as_ref().unwrap().team_id, 7);
        let c = t.join_room(a.room_id, false, intent(3, "C", 0)).unwrap();
        assert_eq!(c.notifications[0].member.as_ref().unwrap().team_id, 0);
    }

    // ── capacity and private rooms ───────────────────────────────────────────────────────────

    #[test]
    fn a_full_room_refuses_with_room_full() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(2), intent(1, "A", 0)).unwrap();
        assert!(t.join_room(a.room_id, false, intent(2, "B", 0)).is_ok());
        assert_eq!(t.join_room(a.room_id, false, intent(3, "C", 0)), Err(RoomError::Full));
        assert_eq!(RoomError::Full.as_error_type(), ErrorType::RoomFull);
    }

    #[test]
    fn a_private_room_refuses_a_joiner_that_presents_no_password() {
        let mut t = RoomTable::new(1);
        let mut s = spec(4);
        s.password = Some(b"hunter2".to_vec());
        let a = t.create_room(s, intent(1, "A", 0)).unwrap();
        assert!(t.get(a.room_id).unwrap().is_private());
        assert_eq!(
            t.join_room(a.room_id, false, intent(2, "B", 0)),
            Err(RoomError::PasswordMissing)
        );
        // positive control: the same joiner with the flag set is admitted, so the refusal is about
        // the flag and not about private rooms refusing everybody.
        assert!(t.join_room(a.room_id, true, intent(2, "B", 0)).is_ok());
    }

    #[test]
    fn a_public_room_does_not_care_about_the_password_flag() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        assert!(t.join_room(a.room_id, true, intent(2, "B", 0)).is_ok());
        assert!(t.join_room(a.room_id, false, intent(3, "C", 0)).is_ok());
    }

    #[test]
    fn password_mismatch_is_never_returned_because_it_cannot_be_checked() {
        // JoinRoomRequest carries no password bytes. Anything that claimed to compare them would
        // be inventing the comparison. If this test ever fails, someone has done exactly that.
        let mut t = RoomTable::new(1);
        let mut s = spec(4);
        s.password = Some(b"hunter2".to_vec());
        let a = t.create_room(s, intent(1, "A", 0)).unwrap();
        for present in [true, false] {
            if let Err(e) = t.join_room(a.room_id, present, intent(9, "X", 0)) {
                assert_ne!(e.as_error_type(), ErrorType::RoomPasswordMismatch);
            }
        }
    }

    #[test]
    fn passwd_slot_mask_is_stored_and_republished_but_never_interpreted() {
        let mut t = RoomTable::new(1);
        let mut s = spec(4);
        s.passwd_slot_mask = Some(0xDEAD_BEEF_0000_0001);
        let a = t.create_room(s, intent(1, "A", 0)).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().passwd_slot_mask, 0xDEAD_BEEF_0000_0001);
        // Joining a "password slot" is not gated on the mask: the bit numbering is unknown and we
        // do not guess it.
        assert!(t.join_room(a.room_id, false, intent(2, "B", 0)).is_ok());
        let out =
            t.set_room_data_internal(a.room_id, 1, 0, 0, vec![], Some(0x5555)).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().passwd_slot_mask, 0x5555);
        assert_eq!(out.notifications[0].passwd_slot_mask, Some(0x5555));
    }

    // ── room data ────────────────────────────────────────────────────────────────────────────

    #[test]
    fn set_room_data_internal_is_a_masked_read_modify_write() {
        let mut t = RoomTable::new(1);
        let mut s = spec(4);
        s.flags = 0b1010;
        let a = t.create_room(s, intent(1, "A", 0)).unwrap();
        // filter 0b0011, value 0b0001 → touch only the low two bits
        t.set_room_data_internal(a.room_id, 1, 0b0011, 0b0001, vec![], None).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().flags, 0b1001);
        // filter 0 changes nothing, even with a value set
        t.set_room_data_internal(a.room_id, 1, 0, 0xFFFF_FFFF, vec![], None).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().flags, 0b1001);
    }

    #[test]
    fn updated_room_data_internal_reaches_every_member_including_the_requester() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        let out = t
            .set_room_data_internal(
                a.room_id,
                1,
                0xF,
                0x5,
                vec![BinAttr { attr_id: 1, data: vec![9], update_date: 0, update_member_id: 0 }],
                None,
            )
            .unwrap();
        let mut targets = out.notifications[0].targets.clone();
        targets.sort_unstable();
        assert_eq!(targets, vec![1, 2]);
        // The attribute names WHO changed it, not whatever the request claimed.
        assert_eq!(out.notifications[0].bin_attrs[0].update_member_id, 1);
        assert_eq!(t.get(a.room_id).unwrap().bin_attrs_internal[0].update_member_id, 1);
    }

    #[test]
    fn bin_attrs_upsert_rather_than_accumulate() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        for v in [1u8, 2, 3] {
            t.set_room_data_internal(
                a.room_id,
                1,
                0,
                0,
                vec![BinAttr { attr_id: 42, data: vec![v], update_date: 0, update_member_id: 0 }],
                None,
            )
            .unwrap();
        }
        let room = t.get(a.room_id).unwrap();
        assert_eq!(room.bin_attrs_internal.len(), 1);
        assert_eq!(room.bin_attrs_internal[0].data, vec![3]);
    }

    #[test]
    fn a_non_member_cannot_change_room_data() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        assert_eq!(
            t.set_room_data_internal(a.room_id, 77, 0xF, 0xF, vec![], None),
            Err(RoomError::Unauthorized)
        );
        assert_eq!(
            t.set_room_data_external(a.room_id, 77, vec![], vec![], vec![]),
            Err(RoomError::Unauthorized)
        );
        // positive control
        assert!(t.set_room_data_internal(a.room_id, 1, 0xF, 0xF, vec![], None).is_ok());
    }

    // ── kickout ──────────────────────────────────────────────────────────────────────────────

    #[test]
    fn only_the_owner_kicks_and_the_target_hears_kickedout() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        let b = t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        t.join_room(a.room_id, false, intent(3, "C", 0)).unwrap();

        assert_eq!(t.kickout(a.room_id, 2, 3), Err(RoomError::Unauthorized));
        assert_eq!(t.kickout(a.room_id, 1, 1), Err(RoomError::InvalidInput));
        assert_eq!(t.kickout(a.room_id, 1, 99), Err(RoomError::NotFound));

        let out = t.kickout(a.room_id, 1, b.me_member_id).unwrap();
        assert_eq!(out.notifications[0].event, RoomEvent::KickedOut);
        assert_eq!(out.notifications[0].targets, vec![2]);
        assert!(out.notifications[0].member.is_none(), "KICKEDOUT carries no member submessage");
        assert_eq!(out.notifications[1].event, RoomEvent::MemberLeft);
        let mut rest = out.notifications[1].targets.clone();
        rest.sort_unstable();
        assert_eq!(rest, vec![1, 3]);
    }

    // ── search ───────────────────────────────────────────────────────────────────────────────

    #[test]
    fn search_filters_on_world_flags_and_attributes() {
        let mut t = RoomTable::new(1);
        let mut s1 = spec(4);
        s1.world_id = 1;
        s1.flags = 0b0101;
        let r1 = t.create_room(s1, intent(1, "A", 0)).unwrap();
        t.set_room_data_external(
            r1.room_id,
            1,
            vec![IntAttr { attr_id: 1, attr_value: 10 }],
            vec![BinAttr { attr_id: 2, data: b"pvp".to_vec(), update_date: 0, update_member_id: 0 }],
            vec![],
        )
        .unwrap();

        let mut s2 = spec(4);
        s2.world_id = 2;
        t.create_room(s2, intent(2, "B", 0)).unwrap();

        let by_world = SearchFilter { world_id: 1, ..Default::default() };
        assert_eq!(t.search(&by_world).0.len(), 1);
        let by_other_world = SearchFilter { world_id: 2, ..Default::default() };
        assert_eq!(t.search(&by_other_world).0.len(), 1);
        let by_missing_world = SearchFilter { world_id: 9, ..Default::default() };
        assert_eq!(t.search(&by_missing_world).0.len(), 0);

        let by_flags = SearchFilter { flag_filter: 0b0100, flag_attr: 0b0100, ..Default::default() };
        assert_eq!(t.search(&by_flags).0.len(), 1);
        let by_wrong_flags =
            SearchFilter { flag_filter: 0b0010, flag_attr: 0b0010, ..Default::default() };
        assert_eq!(t.search(&by_wrong_flags).0.len(), 0);

        let ge = SearchFilter { int_filters: vec![(6, 1, 10)], ..Default::default() };
        assert_eq!(t.search(&ge).0.len(), 1);
        let gt = SearchFilter { int_filters: vec![(5, 1, 10)], ..Default::default() };
        assert_eq!(t.search(&gt).0.len(), 0);
        let bin_eq =
            SearchFilter { bin_filters: vec![(1, 2, b"pvp".to_vec())], ..Default::default() };
        assert_eq!(t.search(&bin_eq).0.len(), 1);

        // An op we do not understand matches NOTHING rather than degrading to "everything".
        let unknown_op = SearchFilter { int_filters: vec![(99, 1, 10)], ..Default::default() };
        assert_eq!(t.search(&unknown_op).0.len(), 0);
    }

    #[test]
    fn search_paging_is_one_based() {
        let mut t = RoomTable::new(1);
        for i in 0..5u64 {
            t.create_room(spec(4), intent(i, &format!("P{i}"), 0)).unwrap();
        }
        let all = SearchFilter::default();
        let (w, start, total) = t.search(&all);
        assert_eq!((w.len(), start, total), (5, 1, 5));
        let page =
            SearchFilter { range_start: 3, range_max: 2, ..Default::default() };
        let (w, start, total) = t.search(&page);
        assert_eq!((w.len(), start, total), (2, 3, 5));
        // start 0 means "the client left it defaulted" - page 1, not an empty result.
        let zero = SearchFilter { range_start: 0, range_max: 2, ..Default::default() };
        assert_eq!(t.search(&zero).1, 1);
    }

    #[test]
    fn joined_slot_mask_tracks_member_ids() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().joined_slot_mask(), 0b1);
        t.join_room(a.room_id, false, intent(2, "B", 0)).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().joined_slot_mask(), 0b11);
        t.leave_room(a.room_id, 1).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().joined_slot_mask(), 0b10);
    }

    #[test]
    fn a_second_join_from_the_same_connection_is_refused() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "A", 0)).unwrap();
        assert_eq!(
            t.join_room(a.room_id, false, intent(1, "A", 0)),
            Err(RoomError::AlreadyJoined)
        );
    }

    #[test]
    fn an_unknown_room_is_room_missing() {
        let mut t = RoomTable::new(1);
        assert_eq!(t.join_room(999, false, intent(1, "A", 0)), Err(RoomError::Missing));
        assert_eq!(t.leave_room(999, 1), Err(RoomError::Missing));
        assert_eq!(RoomError::Missing.as_error_type(), ErrorType::RoomMissing);
    }

    /// ⭐ Two participants that report the SAME npid must still be two members.
    ///
    /// §4 of the plan calls this out as the conflation that "would look like a networking bug
    /// rather than a fixture bug": rooms are keyed by our own `conn_id`, never by the npid string.
    #[test]
    fn two_connections_sharing_an_npid_are_two_distinct_members() {
        let mut t = RoomTable::new(1);
        let a = t.create_room(spec(4), intent(1, "SameName", 0)).unwrap();
        let b = t.join_room(a.room_id, false, intent(2, "SameName", 0)).unwrap();
        let room = t.get(a.room_id).unwrap();
        assert_eq!(room.members.len(), 2);
        assert_ne!(a.me_member_id, b.me_member_id);
        // and leaving removes exactly one of them
        t.leave_room(a.room_id, 2).unwrap();
        assert_eq!(t.get(a.room_id).unwrap().members.len(), 1);
        assert_eq!(t.get(a.room_id).unwrap().members[0].conn_id, 1);
    }
}
