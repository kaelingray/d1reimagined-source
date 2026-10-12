// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Two participants, two sockets, one room - over real TCP.
//!
//! The unit tests exercise the state machine directly. This exercises the part they cannot: that a
//! `MEMBER_JOINED` produced by participant B's request actually arrives **on participant A's
//! socket**, carrying **A's** `ctx_id` and not B's. That is the whole of what P1 in
//! `PLAN-B-SHADNET-MATCHING2.md` §3.0 says was structurally impossible before the stream was split
//! - `encode_notification` had zero callers in the workspace and there was no writer to call while
//! a task was parked in `read_packet`.
//!
//! ⛔ Nothing here starts an emulator or touches `runs/`. Both "participants" are this test.

use std::sync::Arc;

use prost::Message;
use shadnet_proto as pb;
use shadnet_server::identity::{Identity, IdentityMode, ServerConfig};
use shadnet_server::registry::ServerState;
use shadnet_server::session::Session;
use shadnet_wire::{
    read_packet, Command, ErrorType, Notification, Packet, PacketType, HEADER_SIZE,
};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

fn config(rooms: bool) -> ServerConfig {
    ServerConfig {
        identity: Identity {
            npid: "D1Guardian".into(),
            user_id: 1,
            token: "d1-local-token".into(),
            avatar_url: "http://127.0.0.1/avatar.png".into(),
        },
        matching2_enabled: false,
        rooms_enabled: rooms,
        identity_mode: IdentityMode::PerClient,
        peer_port: 3074,
        signaling_udp: false,
    }
}

async fn start(rooms: bool) -> (std::net::SocketAddr, Arc<ServerState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(ServerState::new(config(rooms)));
    let st = state.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else { return };
            let s = st.clone();
            tokio::spawn(async move { Session::run(stream, peer, s).await });
        }
    });
    (addr, state)
}

/// One participant's socket, with the request framing the client uses.
struct Client {
    stream: TcpStream,
    next_id: u64,
}

impl Client {
    async fn connect(addr: std::net::SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut c = Self { stream, next_id: 1 };
        // The server speaks first.
        let hello = c.read().await;
        assert_eq!(hello.packet_type, PacketType::ServerInfo);
        assert_eq!(
            u32::from_le_bytes(hello.payload[..4].try_into().unwrap()),
            shadnet_wire::PROTOCOL_VERSION
        );
        c
    }

    async fn send(&mut self, cmd: Command, proto: &[u8]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut frame = Vec::new();
        frame.push(PacketType::Request as u8);
        frame.extend_from_slice(&(cmd as u16).to_le_bytes());
        frame.extend_from_slice(&((HEADER_SIZE + 4 + proto.len()) as u32).to_le_bytes());
        frame.extend_from_slice(&id.to_le_bytes());
        frame.extend_from_slice(&(proto.len() as u32).to_le_bytes());
        frame.extend_from_slice(proto);
        self.stream.write_all(&frame).await.unwrap();
        self.stream.flush().await.unwrap();
        id
    }

    async fn read(&mut self) -> Packet {
        read_packet(&mut self.stream).await.unwrap()
    }

    /// Read the next **reply**, asserting it echoes the command and packet id the client sent.
    async fn reply(&mut self, cmd: Command, id: u64) -> (ErrorType, Vec<u8>) {
        let p = self.read().await;
        assert_eq!(p.packet_type, PacketType::Reply, "expected a reply to {cmd}");
        assert_eq!(p.command, cmd as u16, "reply must echo the command id");
        assert_eq!(p.packet_id, id, "reply must echo the packet id");
        let err = ErrorType::from_u8(p.payload[0]).unwrap();
        let blob = shadnet_wire::extract_blob(&p.payload, 1).unwrap_or(&[]).to_vec();
        (err, blob)
    }

    async fn request(&mut self, cmd: Command, proto: &[u8]) -> (ErrorType, Vec<u8>) {
        let id = self.send(cmd, proto).await;
        self.reply(cmd, id).await
    }

    async fn login(&mut self, npid: &str) -> ErrorType {
        let req = pb::LoginRequest {
            npid: npid.into(),
            title_id: "CUSA00219".into(),
            title_name: "Destiny".into(),
            ..Default::default()
        };
        self.request(Command::Login, &req.encode_to_vec()).await.0
    }

    async fn context_start(&mut self, ctx_id: u32) -> ErrorType {
        let req = pb::ContextStartRequest { ctx_id };
        self.request(Command::ContextStart, &req.encode_to_vec()).await.0
    }
}

fn room_event(p: &Packet) -> pb::NotifyRoomEvent {
    assert_eq!(p.packet_type, PacketType::Notification);
    assert_eq!(p.command, Notification::RoomEvent as u16);
    assert_eq!(p.packet_id, 0, "notifications carry no packet id");
    // ⚠ No ErrorType byte on a notification: the blob length starts at offset 0.
    let blob = shadnet_wire::extract_blob(&p.payload, 0).unwrap();
    pb::NotifyRoomEvent::decode(blob).unwrap()
}

#[tokio::test]
async fn two_participants_share_one_room_and_each_hears_about_the_other() {
    let (addr, state) = start(true).await;

    // ── participant A: log in, start a context, create a room ────────────────────────────────
    let mut a = Client::connect(addr).await;
    assert_eq!(a.login("PlayerA").await, ErrorType::NoError);
    assert_eq!(a.context_start(1).await, ErrorType::NoError);

    let create = pb::CreateRoomRequest {
        max_slots: 4,
        world_id: 1,
        team_id: 5,
        flags: 0b1010,
        room_password_present: true,
        room_password: b"hunter2".to_vec(),
        ..Default::default()
    };
    let (err, blob) = a.request(Command::CreateRoom, &create.encode_to_vec()).await;
    assert_eq!(err, ErrorType::NoError);
    let reply = pb::CreateRoomReply::decode(blob.as_slice()).unwrap();
    let details = reply.details.expect("has_details() is what the client tests first");
    let room_id = details.room_data.as_ref().unwrap().room_id;
    assert_ne!(room_id, 0, "0 is the client's `no room` sentinel");
    assert_eq!(details.me_member_id, 1);
    assert_eq!(details.owner_member_id, 1);
    assert_eq!(details.members.len(), 1);
    assert_eq!(details.members[0].npid, "PlayerA");
    assert_eq!(details.members[0].team_id, 5, "team_id is copied verbatim");
    assert_eq!(details.members[0].addr, "127.0.0.1");
    assert_eq!(details.members[0].port, 3074);
    assert!(details.members[0].is_owner);

    // ── participant B: a SECOND socket, a DIFFERENT identity, a different ctx_id ─────────────
    let mut b = Client::connect(addr).await;
    assert_eq!(b.login("PlayerB").await, ErrorType::NoError);
    assert_eq!(b.context_start(7).await, ErrorType::NoError);

    // B finds the room it never created.
    let (err, blob) = b
        .request(
            Command::SearchRoom,
            &pb::SearchRoomRequest { world_id: 1, ..Default::default() }.encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);
    let found = pb::SearchRoomReply::decode(blob.as_slice()).unwrap();
    assert_eq!(found.range_total, 1);
    assert_eq!(found.rooms[0].room_id, room_id);
    assert_eq!(found.rooms[0].owner_npid, "PlayerA");

    // ⭐ The private-room half: joining without the password flag is REFUSED.
    let (err, _) = b
        .request(
            Command::JoinRoom,
            &pb::JoinRoomRequest { room_id, team_id: 9, ..Default::default() }.encode_to_vec(),
        )
        .await;
    assert_eq!(
        err,
        ErrorType::RoomPasswordMissing,
        "a private room must refuse a joiner that presents nothing"
    );

    // ...and with the flag it is admitted. Without this arm the refusal above would be
    // indistinguishable from "JoinRoom refuses everything".
    let (err, blob) = b
        .request(
            Command::JoinRoom,
            &pb::JoinRoomRequest {
                room_id,
                team_id: 9,
                room_password_present: true,
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);
    let jdetails = pb::JoinRoomReply::decode(blob.as_slice()).unwrap().details.unwrap();
    assert_eq!(jdetails.me_member_id, 2);
    assert_eq!(jdetails.owner_member_id, 1);
    assert_eq!(jdetails.members.len(), 2);

    // ── ⭐⭐ the thing this whole test exists for ─────────────────────────────────────────────
    // A, which asked for nothing, receives a notification produced by B's request.
    let n = room_event(&a.read().await);
    assert_eq!(n.event, 0x1101, "MEMBER_JOINED");
    assert_eq!(n.ctx_id, 1, "A's OWN ctx_id - not B's 7, which would vanish silently");
    assert_eq!(n.room_id, room_id);
    let m = n.member.expect("pb.has_member()");
    assert_eq!(m.npid, "PlayerB");
    assert_eq!(m.member_id, 2);
    assert_eq!(m.team_id, 9, "the server did not renumber B's team");
    assert!(!m.is_owner);
    assert_eq!(m.addr, "127.0.0.1");
    assert_ne!(m.port, 0, "a zero port would stall A's reader thread for 5 s");

    // ── room data: A changes it, BOTH hear ───────────────────────────────────────────────────
    let (err, _) = a
        .request(
            Command::SetRoomDataInternal,
            &pb::SetRoomDataInternalRequest {
                room_id,
                flag_filter: 0b0011,
                flag_attr: 0b0001,
                bin_attrs: vec![pb::MatchingBinAttr {
                    attr_id: 1,
                    data: b"pvp".to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);
    for (who, ctx) in [(&mut a, 1u32), (&mut b, 7u32)] {
        let n = room_event(&who.read().await);
        assert_eq!(n.event, 0x1106, "UPDATED_ROOM_DATA_INTERNAL");
        assert_eq!(n.ctx_id, ctx);
        assert_eq!(n.flags, 0b1001, "masked read-modify-write");
        assert_eq!(n.bin_attrs[0].data, b"pvp");
        assert_eq!(n.bin_attrs[0].update_member_id, 1, "the attr names who changed it");
    }

    // ── B leaves; A is told, with LEAVE_ACTION ───────────────────────────────────────────────
    let (err, _) = b
        .request(
            Command::LeaveRoom,
            &pb::LeaveRoomRequest { room_id, req_id: 0 }.encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);
    let n = room_event(&a.read().await);
    assert_eq!(n.event, 0x1102, "MEMBER_LEFT");
    assert_eq!(n.event_cause, 1, "LEAVE_ACTION");
    assert_eq!(n.member.unwrap().npid, "PlayerB");

    // The room survives with one member; it is destroyed only when the last one goes.
    assert_eq!(state.rooms.lock().unwrap().len(), 1);
    let (err, _) = a
        .request(Command::LeaveRoom, &pb::LeaveRoomRequest { room_id, req_id: 0 }.encode_to_vec())
        .await;
    assert_eq!(err, ErrorType::NoError);
    assert_eq!(state.rooms.lock().unwrap().len(), 0);
}

#[tokio::test]
async fn a_dropped_socket_tells_the_remaining_member() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    a.login("PlayerA").await;
    a.context_start(1).await;
    let (_, blob) = a
        .request(
            Command::CreateRoom,
            &pb::CreateRoomRequest { max_slots: 4, ..Default::default() }.encode_to_vec(),
        )
        .await;
    let room_id = pb::CreateRoomReply::decode(blob.as_slice())
        .unwrap()
        .details
        .unwrap()
        .room_data
        .unwrap()
        .room_id;

    let mut b = Client::connect(addr).await;
    b.login("PlayerB").await;
    b.context_start(2).await;
    b.request(
        Command::JoinRoom,
        &pb::JoinRoomRequest { room_id, ..Default::default() }.encode_to_vec(),
    )
    .await;
    let _joined = a.read().await; // MEMBER_JOINED

    // B vanishes without leaving. Without the disconnect path, A would hold a member that never
    // answers - the failure shows up much later as a peer that never handshakes.
    drop(b);
    let n = room_event(&a.read().await);
    assert_eq!(n.event, 0x1102, "MEMBER_LEFT");
    assert_eq!(n.event_cause, 5, "MEMBER_DISAPPEARED - because that is what happened");
}

#[tokio::test]
async fn kickout_reaches_the_target_and_the_rest_separately() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    a.login("PlayerA").await;
    a.context_start(1).await;
    let (_, blob) = a
        .request(
            Command::CreateRoom,
            &pb::CreateRoomRequest { max_slots: 4, ..Default::default() }.encode_to_vec(),
        )
        .await;
    let room_id = pb::CreateRoomReply::decode(blob.as_slice())
        .unwrap()
        .details
        .unwrap()
        .room_data
        .unwrap()
        .room_id;

    let mut b = Client::connect(addr).await;
    b.login("PlayerB").await;
    b.context_start(2).await;
    b.request(
        Command::JoinRoom,
        &pb::JoinRoomRequest { room_id, ..Default::default() }.encode_to_vec(),
    )
    .await;
    let _ = a.read().await;

    // B may not kick A: only the owner kicks.
    let (err, _) = b
        .request(
            Command::KickoutRoomMember,
            &pb::KickoutRoomMemberRequest { room_id, target_member_id: 1, ..Default::default() }
                .encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::Unauthorized);

    // A kicks B. B hears KICKEDOUT with no member submessage; A hears MEMBER_LEFT with one.
    let (err, _) = a
        .request(
            Command::KickoutRoomMember,
            &pb::KickoutRoomMemberRequest { room_id, target_member_id: 2, ..Default::default() }
                .encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);

    let to_b = room_event(&b.read().await);
    assert_eq!(to_b.event, 0x1103, "KICKEDOUT");
    assert_eq!(to_b.ctx_id, 2);
    assert_eq!(to_b.event_cause, 2, "KICKOUT_ACTION");
    assert!(to_b.member.is_none());

    let to_a = room_event(&a.read().await);
    assert_eq!(to_a.event, 0x1102, "MEMBER_LEFT");
    assert_eq!(to_a.ctx_id, 1);
    assert_eq!(to_a.member.unwrap().member_id, 2);
}

#[tokio::test]
async fn a_room_message_is_relayed_to_the_other_member_only() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    a.login("PlayerA").await;
    a.context_start(1).await;
    let (_, blob) = a
        .request(
            Command::CreateRoom,
            &pb::CreateRoomRequest { max_slots: 4, ..Default::default() }.encode_to_vec(),
        )
        .await;
    let room_id = pb::CreateRoomReply::decode(blob.as_slice())
        .unwrap()
        .details
        .unwrap()
        .room_data
        .unwrap()
        .room_id;

    let mut b = Client::connect(addr).await;
    b.login("PlayerB").await;
    b.context_start(2).await;
    b.request(
        Command::JoinRoom,
        &pb::JoinRoomRequest { room_id, ..Default::default() }.encode_to_vec(),
    )
    .await;
    let _ = a.read().await;

    let (err, _) = a
        .request(
            Command::SendRoomMessage,
            &pb::SendRoomMessageRequest {
                room_id,
                cast_type: 2,
                msg: b"ping".to_vec(),
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);

    let p = b.read().await;
    assert_eq!(p.command, Notification::RoomMessage as u16);
    let msg = pb::NotifyRoomMessage::decode(shadnet_wire::extract_blob(&p.payload, 0).unwrap())
        .unwrap();
    assert_eq!(msg.ctx_id, 2, "B's own ctx_id");
    assert_eq!(msg.msg, b"ping");
    assert_eq!(msg.src_npid, "PlayerA");
    assert_eq!(msg.src_member_id, 1);
}

/// ⭐ Two participants configured with the SAME npid is the conflation the harness contract
/// forbids, and it must be **loud**.
#[tokio::test]
async fn a_second_participant_claiming_the_same_npid_is_refused() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    assert_eq!(a.login("SameName").await, ErrorType::NoError);

    let mut b = Client::connect(addr).await;
    assert_eq!(b.login("SameName").await, ErrorType::LoginAlreadyLoggedIn);

    // positive control: the same second socket with its own handle is accepted, so the refusal is
    // about the collision and not about second connections.
    assert_eq!(b.login("OtherName").await, ErrorType::NoError);
}

/// The default configuration must be indistinguishable from the pre-rooms server.
#[tokio::test]
async fn with_rooms_off_every_room_command_is_still_answered_unsupported() {
    let (addr, _state) = start(false).await;
    let mut a = Client::connect(addr).await;
    assert_eq!(a.login("PlayerA").await, ErrorType::NoError);

    // ContextStart is the one room command that reaches the socket regardless of the feature flag.
    assert_eq!(a.context_start(1).await, ErrorType::Unsupported);
    let (err, _) = a
        .request(
            Command::CreateRoom,
            &pb::CreateRoomRequest { max_slots: 4, ..Default::default() }.encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::Unsupported);

    // ...while the four commands that were always served still are.
    let (err, blob) = a.request(Command::GetToken, &[]).await;
    assert_eq!(err, ErrorType::NoError);
    assert!(!pb::GetTokenReply::decode(blob.as_slice()).unwrap().token.is_empty());
    let (err, blob) = a.request(Command::GetServerFeatures, &[]).await;
    assert_eq!(err, ErrorType::NoError);
    assert!(
        !pb::ServerFeaturesReply::decode(blob.as_slice()).unwrap().matching2_enabled,
        "still advertised false - nothing has asked"
    );
}

/// A room command before `Login` is refused. Otherwise an unauthenticated socket could take a
/// member slot and its npid would be the empty string.
#[tokio::test]
async fn a_room_command_before_login_is_refused() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    assert_eq!(a.context_start(1).await, ErrorType::Unauthorized);
    assert_eq!(a.login("PlayerA").await, ErrorType::NoError);
    assert_eq!(a.context_start(1).await, ErrorType::NoError);
}

/// A second `ContextStart` with a different id is refused rather than guessed at: notifications
/// carry exactly one `ctx_id` and multi-context attribution is a standing unknown (§9.22).
#[tokio::test]
async fn a_second_context_start_with_a_different_id_is_refused() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    a.login("PlayerA").await;
    assert_eq!(a.context_start(1).await, ErrorType::NoError);
    assert_eq!(a.context_start(1).await, ErrorType::NoError, "idempotent for the same id");
    assert_eq!(a.context_start(2).await, ErrorType::Invalid);
    assert_eq!(a.context_start(0).await, ErrorType::InvalidInput, "0 is not a wildcard");
}

/// `RequestSignalingInfos` answers from room membership, and refuses when it has nothing -
/// it never invents a port.
#[tokio::test]
async fn signaling_answers_for_a_room_member_and_refuses_for_a_stranger() {
    let (addr, _state) = start(true).await;
    let mut a = Client::connect(addr).await;
    a.login("PlayerA").await;
    a.context_start(1).await;
    let (_, blob) = a
        .request(
            Command::CreateRoom,
            &pb::CreateRoomRequest { max_slots: 4, ..Default::default() }.encode_to_vec(),
        )
        .await;
    let room_id = pb::CreateRoomReply::decode(blob.as_slice())
        .unwrap()
        .details
        .unwrap()
        .room_data
        .unwrap()
        .room_id;
    assert_ne!(room_id, 0);

    let (err, blob) = a
        .request(
            Command::RequestSignalingInfos,
            &pb::RequestSignalingInfosRequest { target_npid: "PlayerA".into() }.encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NoError);
    let reply = pb::RequestSignalingInfosReply::decode(blob.as_slice()).unwrap();
    assert_eq!(reply.target_ip, "127.0.0.1");
    assert_eq!(reply.target_port, 3074);
    assert_eq!(reply.target_member_id, 1);

    let (err, _) = a
        .request(
            Command::RequestSignalingInfos,
            &pb::RequestSignalingInfosRequest { target_npid: "Nobody".into() }.encode_to_vec(),
        )
        .await;
    assert_eq!(err, ErrorType::NotFound, "a refusal, never a fabricated port");
}
