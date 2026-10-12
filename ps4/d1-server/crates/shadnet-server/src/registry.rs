// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Cross-connection state: who is logged in, who is in which room, and where peers were seen.
//!
//! Everything before this module was per-connection. A room protocol is not: the room *is* the
//! shared object, and `MEMBER_JOINED` has to reach a socket that some other task owns. So this
//! holds three maps behind one lock each, and hands out an `mpsc` sender per connection.
//!
//! ## Two simultaneous identities
//!
//! `PLAN-B-SHADNET-MATCHING2.md` §4 is blunt about the failure mode: two participants that collapse
//! into one identity produce something "that would look like a networking bug rather than a fixture
//! bug". Three rules follow, and all three are enforced here rather than documented:
//!
//! 1. **Rooms are keyed by `conn_id`, never by npid.** Two connections claiming the same handle are
//!    two members (see `rooms::tests::two_connections_sharing_an_npid_are_two_distinct_members`).
//! 2. **A second `Login` for a live npid is refused** with the client's own
//!    `LoginAlreadyLoggedIn(6)` - a variant that has existed in the wire crate since day one and
//!    has never been used. Silence here is what conflation looks like.
//! 3. **`user_id` is allocated per npid**, not a constant `1` for everybody, so the identity the
//!    client is handed differs between participants.
//!
//! The first participant still receives the configured identity **verbatim**, so a single-instance
//! run is byte-identical to what this server did before rooms existed. That is deliberate: the
//! operator is driving live runs against this binary and a new identity would be a silent variable.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use shadnet_wire::ErrorType;

use crate::identity::{Identity, IdentityMode, ServerConfig};
use crate::rooms::{Endpoint, RoomTable};

/// A connection's outbox. Replies and notifications both go through it, in order, so a
/// `MEMBER_JOINED` can never overtake the `JoinRoomReply` that has to precede it.
pub type Outbox = tokio::sync::mpsc::UnboundedSender<Vec<u8>>;

/// What the server knows about one live connection.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub conn_id: u64,
    /// The npid **the client reported in `LoginRequest`**, not our `--npid`. That asymmetry is the
    /// point: Matching2 keys on the client's own profile handle.
    pub npid: String,
    pub peer: SocketAddr,
    /// The Matching2 context id from `ContextStartRequest`. `None` until the client starts one.
    /// Every notification we push must carry **this** value, per recipient.
    pub ctx_id: Option<u32>,
    pub outbox: Outbox,
}

/// The server's shared state. One instance, shared by every connection.
#[derive(Debug)]
pub struct ServerState {
    pub config: ServerConfig,
    sessions: Mutex<HashMap<u64, SessionHandle>>,
    /// npid → conn_id, for the login-collision check and for `RequestSignalingInfos` lookups.
    by_npid: Mutex<HashMap<String, u64>>,
    /// npid → identity handed out. Stable for the lifetime of the process so a reconnect gets the
    /// same `user_id` and a trace stays comparable.
    identities: Mutex<HashMap<String, Identity>>,
    /// npid → endpoint observed on the **UDP registration channel** (`bind.port + 1`).
    ///
    /// ⚠ Empty in practice today: shadPS4's `SendMatching2StunPing` bails at
    /// `EnsureP2PTransport()`, which returns `false` at HEAD, so the ping never leaves the
    /// emulator. Implemented anyway because the alternative - inferring a port - is fabrication.
    signaling: Mutex<HashMap<String, SocketAddr>>,
    pub rooms: Mutex<RoomTable>,
    next_conn_id: AtomicU64,
    next_user_id: AtomicU64,
}

/// Why a login was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRefusal {
    /// This npid already has a live connection. **Not** an error to paper over: it is the
    /// two-instance fixture telling you both instances were configured the same.
    AlreadyLoggedIn,
}

impl LoginRefusal {
    pub fn as_error_type(self) -> ErrorType {
        match self {
            Self::AlreadyLoggedIn => ErrorType::LoginAlreadyLoggedIn,
        }
    }
}

impl ServerState {
    pub fn new(config: ServerConfig) -> Self {
        let first_user_id = config.identity.user_id;
        Self {
            config,
            sessions: Mutex::new(HashMap::new()),
            by_npid: Mutex::new(HashMap::new()),
            identities: Mutex::new(HashMap::new()),
            signaling: Mutex::new(HashMap::new()),
            rooms: Mutex::new(RoomTable::new(1)),
            next_conn_id: AtomicU64::new(1),
            next_user_id: AtomicU64::new(first_user_id),
        }
    }

    /// Non-zero, monotonic, and never reused within a process.
    pub fn alloc_conn_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn register(&self, handle: SessionHandle) {
        self.sessions.lock().unwrap().insert(handle.conn_id, handle);
    }

    pub fn session(&self, conn_id: u64) -> Option<SessionHandle> {
        self.sessions.lock().unwrap().get(&conn_id).cloned()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    pub fn set_ctx_id(&self, conn_id: u64, ctx_id: u32) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&conn_id) {
            s.ctx_id = Some(ctx_id);
        }
    }

    pub fn clear_ctx_id(&self, conn_id: u64) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&conn_id) {
            s.ctx_id = None;
        }
    }

    pub fn ctx_id(&self, conn_id: u64) -> Option<u32> {
        self.sessions.lock().unwrap().get(&conn_id).and_then(|s| s.ctx_id)
    }

    /// Attach the npid this connection claimed at `Login`, so `disconnect` can release it and
    /// `RequestSignalingInfos` can find it.
    pub fn set_npid(&self, conn_id: u64, npid: &str) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&conn_id) {
            s.npid = npid.to_string();
        }
    }

    /// The identity minted for `npid`, or the configured one if that npid never logged in.
    pub fn identity_for(&self, npid: &str) -> Identity {
        self.identities
            .lock()
            .unwrap()
            .get(npid)
            .cloned()
            .unwrap_or_else(|| self.config.identity.clone())
    }

    /// Claim an npid for this connection and mint (or recall) its identity.
    ///
    /// The **first** npid to log in inherits the configured identity verbatim, so single-participant
    /// runs do not change. Later distinct npids get a distinct `user_id` and, in
    /// [`IdentityMode::PerClient`], their own npid and token.
    pub fn login(
        &self,
        conn_id: u64,
        reported_npid: &str,
    ) -> Result<(String, Identity), LoginRefusal> {
        // An empty npid means the client did not tell us who it is; fall back rather than key
        // every anonymous connection to the same empty string.
        let npid = if reported_npid.is_empty() {
            self.config.identity.npid.clone()
        } else {
            reported_npid.to_string()
        };

        // ⚠ Two locks, never nested: `disconnect` takes `sessions` then `by_npid`, so taking them
        // the other way round here would be a lock-order inversion - a deadlock that would appear
        // only when a login and a disconnect raced, i.e. only in the two-instance run.
        let existing = self.by_npid.lock().unwrap().get(&npid).copied();
        if let Some(other) = existing {
            if other != conn_id && self.sessions.lock().unwrap().contains_key(&other) {
                return Err(LoginRefusal::AlreadyLoggedIn);
            }
        }
        self.by_npid.lock().unwrap().insert(npid.clone(), conn_id);

        let mut identities = self.identities.lock().unwrap();
        let identity = identities
            .entry(npid.clone())
            .or_insert_with(|| {
                let user_id = self.next_user_id.fetch_add(1, Ordering::Relaxed);
                let base = &self.config.identity;
                match self.config.identity_mode {
                    IdentityMode::Fixed => Identity { user_id, ..base.clone() },
                    IdentityMode::PerClient => Identity {
                        npid: npid.clone(),
                        user_id,
                        token: if user_id == base.user_id {
                            base.token.clone()
                        } else {
                            format!("{}-{}", base.token, user_id)
                        },
                        avatar_url: base.avatar_url.clone(),
                    },
                }
            })
            .clone();
        Ok((npid, identity))
    }

    /// Tear a connection down: drop its session, release its npid, and leave its room.
    ///
    /// Returns whatever the remaining members must be told.
    pub fn disconnect(&self, conn_id: u64) -> Option<crate::rooms::Outcome> {
        let handle = self.sessions.lock().unwrap().remove(&conn_id);
        if let Some(h) = &handle {
            let mut by_npid = self.by_npid.lock().unwrap();
            if by_npid.get(&h.npid) == Some(&conn_id) {
                by_npid.remove(&h.npid);
            }
        }
        self.rooms.lock().unwrap().drop_connection(conn_id)
    }

    // ── signaling ────────────────────────────────────────────────────────────────────────────

    /// Record a `(npid → source addr:port)` observation from the UDP registration channel.
    pub fn observe_signaling(&self, npid: &str, from: SocketAddr) {
        self.signaling.lock().unwrap().insert(npid.to_string(), from);
    }

    pub fn signaling_observation(&self, npid: &str) -> Option<SocketAddr> {
        self.signaling.lock().unwrap().get(npid).copied()
    }

    pub fn signaling_count(&self) -> usize {
        self.signaling.lock().unwrap().len()
    }

    /// ⭐ Answer `RequestSignalingInfos(105)`.
    ///
    /// Two sources, in this order, and **no third**:
    ///
    /// 1. the endpoint that npid **published as a room member** - what every other peer already
    ///    holds, so answering anything else would put two peers on different addresses;
    /// 2. the UDP registration observation, if the emulator ever sends one.
    ///
    /// If neither exists we return [`ErrorType::NotFound`]. ⚠ The tempting third source - the
    /// npid's **TCP** source address - carries the *wrong port* by construction (it is the
    /// ephemeral port of the ShadNet control connection, not a P2P port). Returning it would hand
    /// the client a plausible endpoint that connects to nothing, and the failure would surface
    /// minutes later as a peer that never handshakes. A refusal is loud; a wrong port is silent.
    pub fn resolve_signaling(&self, target_npid: &str) -> Result<(Endpoint, u32), ErrorType> {
        if target_npid.is_empty() {
            return Err(ErrorType::InvalidInput);
        }
        if let Some(rooms) = self.rooms.lock().ok() {
            for room in rooms.iter() {
                if let Some(m) = room.member_by_npid(target_npid) {
                    if m.endpoint.is_usable() {
                        return Ok((m.endpoint.clone(), m.member_id));
                    }
                }
            }
        }
        match self.signaling_observation(target_npid) {
            Some(addr) if addr.port() != 0 => Ok((
                Endpoint { addr: addr.ip().to_string(), port: addr.port() as u32 },
                0,
            )),
            _ => Err(ErrorType::NotFound),
        }
    }
}

/// The endpoint we publish for a member.
///
/// `addr` is genuinely observed - the TCP source address of that participant's own connection.
/// `port` is **not**: nothing on the wire carries it and shadPS4's `GetP2PConfiguredPort()` returns
/// 0 at HEAD, so the operator supplies it. Returns `None` for a non-IPv4 peer, because the client's
/// `IpStringToAddr` would turn one into address 0 anyway and a member we cannot address is a member
/// we must refuse rather than publish.
pub fn endpoint_for(peer: SocketAddr, peer_port: u16) -> Option<Endpoint> {
    let ip = match peer.ip() {
        IpAddr::V4(v4) => v4,
        // An IPv4-mapped IPv6 peer (`::ffff:10.0.0.1`) is still reachable; unmap it rather than
        // refuse, because that is what a dual-stack listener produces for an ordinary IPv4 client.
        IpAddr::V6(v6) => v6.to_ipv4_mapped()?,
    };
    Some(Endpoint { addr: ip.to_string(), port: peer_port as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{Identity, IdentityMode, ServerConfig};

    fn cfg(mode: IdentityMode) -> ServerConfig {
        ServerConfig {
            identity: Identity {
                npid: "D1Guardian".into(),
                user_id: 1,
                token: "d1-local-token".into(),
                avatar_url: "http://127.0.0.1/avatar.png".into(),
            },
            matching2_enabled: false,
            rooms_enabled: true,
            identity_mode: mode,
            peer_port: 3074,
            signaling_udp: false,
        }
    }

    fn addr(n: u8) -> SocketAddr {
        format!("10.77.0.{n}:40000").parse().unwrap()
    }

    fn handle(state: &ServerState, npid: &str, n: u8) -> u64 {
        let conn = state.alloc_conn_id();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        // keep the receiver alive for the length of the test
        std::mem::forget(rx);
        state.register(SessionHandle {
            conn_id: conn,
            npid: npid.into(),
            peer: addr(n),
            ctx_id: None,
            outbox: tx,
        });
        conn
    }

    #[test]
    fn the_first_participant_gets_the_configured_identity_verbatim() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let conn = handle(&state, "PlayerA", 2);
        let (npid, id) = state.login(conn, "PlayerA").unwrap();
        assert_eq!(npid, "PlayerA", "rooms key on the CLIENT's npid");
        // ...but the identity handed back is still the configured one, so a single-instance run
        // sees exactly what it saw before this module existed.
        assert_eq!(id.npid, "D1Guardian");
        assert_eq!(id.user_id, 1);
        assert_eq!(id.token, "d1-local-token");
    }

    #[test]
    fn a_second_distinct_participant_gets_a_distinct_user_id() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "PlayerA", 2);
        let b = handle(&state, "PlayerB", 3);
        let (_, ida) = state.login(a, "PlayerA").unwrap();
        let (_, idb) = state.login(b, "PlayerB").unwrap();
        assert_ne!(ida.user_id, idb.user_id);
    }

    #[test]
    fn per_client_mode_hands_each_participant_its_own_npid_and_token() {
        let state = ServerState::new(cfg(IdentityMode::PerClient));
        let a = handle(&state, "PlayerA", 2);
        let b = handle(&state, "PlayerB", 3);
        let (_, ida) = state.login(a, "PlayerA").unwrap();
        let (_, idb) = state.login(b, "PlayerB").unwrap();
        assert_eq!(ida.npid, "PlayerA");
        assert_eq!(idb.npid, "PlayerB");
        assert_ne!(ida.token, idb.token);
    }

    #[test]
    fn a_second_login_for_a_live_npid_is_refused() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "Same", 2);
        let b = handle(&state, "Same", 3);
        assert!(state.login(a, "Same").is_ok());
        assert_eq!(state.login(b, "Same"), Err(LoginRefusal::AlreadyLoggedIn));
        assert_eq!(
            LoginRefusal::AlreadyLoggedIn.as_error_type(),
            ErrorType::LoginAlreadyLoggedIn
        );
        // positive control: a DIFFERENT npid on the same second connection is accepted, so the
        // refusal is about the collision and not about the second connection.
        assert!(state.login(b, "Other").is_ok());
    }

    #[test]
    fn a_reconnect_after_disconnect_reclaims_the_npid_and_its_user_id() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "PlayerA", 2);
        let (_, first) = state.login(a, "PlayerA").unwrap();
        state.disconnect(a);
        let a2 = handle(&state, "PlayerA", 2);
        let (_, again) = state.login(a2, "PlayerA").unwrap();
        assert_eq!(first.user_id, again.user_id, "same handle, same identity, comparable traces");
    }

    #[test]
    fn login_with_an_empty_npid_falls_back_to_the_configured_one() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "", 2);
        let (npid, _) = state.login(a, "").unwrap();
        assert_eq!(npid, "D1Guardian");
    }

    #[test]
    fn ctx_id_is_per_connection() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "A", 2);
        let b = handle(&state, "B", 3);
        state.set_ctx_id(a, 1);
        state.set_ctx_id(b, 7);
        assert_eq!(state.ctx_id(a), Some(1));
        assert_eq!(state.ctx_id(b), Some(7));
        state.clear_ctx_id(a);
        assert_eq!(state.ctx_id(a), None);
        assert_eq!(state.ctx_id(b), Some(7));
    }

    // ── signaling ────────────────────────────────────────────────────────────────────────────

    #[test]
    fn signaling_answers_from_room_membership_first() {
        use crate::rooms::{MemberIntent, RoomSpec};
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "PlayerA", 2);
        state.login(a, "PlayerA").unwrap();
        state
            .rooms
            .lock()
            .unwrap()
            .create_room(
                RoomSpec { max_slots: 4, ..Default::default() },
                MemberIntent {
                    conn_id: a,
                    npid: "PlayerA".into(),
                    team_id: 0,
                    endpoint: Endpoint { addr: "10.77.0.2".into(), port: 3074 },
                    nat_type: 2,
                    flag_attr: 0,
                    group_id: 0,
                    account_id: 0,
                    platform: 0,
                    join_date: 0,
                    bin_attrs_internal: vec![],
                },
            )
            .unwrap();
        let (ep, member_id) = state.resolve_signaling("PlayerA").unwrap();
        assert_eq!(ep.addr, "10.77.0.2");
        assert_eq!(ep.port, 3074);
        assert_eq!(member_id, 1);
    }

    /// ⭐ The control that matters: an unknown peer gets a REFUSAL, not the TCP source port.
    #[test]
    fn signaling_refuses_rather_than_fabricating_a_port() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "PlayerA", 2);
        state.login(a, "PlayerA").unwrap();
        // PlayerA is logged in - so a naive implementation could answer from `peer` - and is in no
        // room and has sent no UDP registration.
        assert_eq!(state.resolve_signaling("PlayerA"), Err(ErrorType::NotFound));
        assert_eq!(state.resolve_signaling(""), Err(ErrorType::InvalidInput));

        // positive control: once an observation exists, the same call succeeds. Without this arm
        // "it refuses" would be indistinguishable from "it refuses everything".
        state.observe_signaling("PlayerA", "10.77.0.2:3074".parse().unwrap());
        let (ep, _) = state.resolve_signaling("PlayerA").unwrap();
        assert_eq!((ep.addr.as_str(), ep.port), ("10.77.0.2", 3074));
    }

    #[test]
    fn endpoint_for_unmaps_ipv4_and_refuses_real_ipv6() {
        let v4: SocketAddr = "10.77.0.3:51000".parse().unwrap();
        assert_eq!(endpoint_for(v4, 3074).unwrap().addr, "10.77.0.3");
        let mapped: SocketAddr = "[::ffff:10.77.0.3]:51000".parse().unwrap();
        assert_eq!(endpoint_for(mapped, 3074).unwrap().addr, "10.77.0.3");
        let v6: SocketAddr = "[2001:db8::1]:51000".parse().unwrap();
        assert!(endpoint_for(v6, 3074).is_none(), "IpStringToAddr would read this as address 0");
        // and the port is the operator's, not the peer's ephemeral one
        assert_eq!(endpoint_for(v4, 3074).unwrap().port, 3074);
        assert_ne!(endpoint_for(v4, 3074).unwrap().port, 51000);
    }

    #[test]
    fn disconnect_removes_the_session_and_releases_the_npid() {
        let state = ServerState::new(cfg(IdentityMode::Fixed));
        let a = handle(&state, "PlayerA", 2);
        state.login(a, "PlayerA").unwrap();
        assert_eq!(state.session_count(), 1);
        state.disconnect(a);
        assert_eq!(state.session_count(), 0);
        let b = handle(&state, "PlayerA", 3);
        assert!(state.login(b, "PlayerA").is_ok());
    }
}
