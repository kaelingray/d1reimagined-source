// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The 21-byte `Matching2StunPing` layout is read out of shadPS4's
// `src/core/libraries/np/np_matching2/np_matching2_signaling.cpp` (GPL-2.0). See ATTRIBUTION.md.

//! The UDP registration channel - how the server learns a peer's *port*.
//!
//! Each started Matching2 context sends this every 5 s to `<shadnet-host>:tcp_port + 1`:
//!
//! ```text
//! #pragma pack(1)
//! struct Matching2StunPing {   // sizeof == 21, static_assert'd upstream
//!     u8  cmd = 0x01;
//!     u8  online_id[16];       // ORBIS_NP_ONLINEID_MAX_LENGTH, NUL-padded
//!     u32 local_ip;            // the client's own advertised address, same encoding as
//!                              // IpStringToAddr: a | b<<8 | c<<16 | d<<24
//! }
//! ```
//!
//! It is **one-way** - nothing reads a reply. Its only purpose is to let the server observe
//! `(npid → public addr:port)` from the datagram's source address, which is the honest source for
//! `RequestSignalingInfos`.
//!
//! ## ⛔ It will receive nothing today, and that is not a bug in this file
//!
//! `SendMatching2StunPing` bails at its `EnsureP2PTransport()` guard, and `EnsureP2PTransport()`
//! `return false;` at `p2p_sockets.cpp:140` in the pinned fork. So the ping never leaves the
//! emulator. The listener is implemented anyway because the alternative is inferring a port, and an
//! inferred port is a fabricated one. **A zero observation count is the expected result** - see the
//! `signaling_observations` line in the shutdown summary. Do not read it as "the listener is
//! broken"; that is exactly the silent-false-negative shape this project keeps paying for.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

use crate::registry::ServerState;

/// `sizeof(Matching2StunPing)`, `static_assert`ed upstream.
pub const STUN_PING_LEN: usize = 21;
/// `ORBIS_NP_ONLINEID_MAX_LENGTH`.
pub const ONLINE_ID_LEN: usize = 16;
/// The only `cmd` value the struct is ever built with.
pub const STUN_PING_CMD: u8 = 0x01;

/// What one registration ping said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StunPing {
    pub npid: String,
    /// The address the client *believes* it has. ⚠ Advisory only: shadPS4's `RetrieveIp()` probes
    /// by `connect()`ing to `1.1.1.1:80`, which cannot succeed inside our contained namespace, so
    /// this field is expected to be `0.0.0.0`. The datagram's **source** address is the real datum.
    pub local_ip: [u8; 4],
}

/// Parse a datagram. `None` for anything that is not this struct - a length that is not exactly 21
/// or a `cmd` that is not 1 means we are looking at somebody else's traffic, and guessing at it
/// would put a wrong endpoint into the signaling map.
pub fn parse_stun_ping(buf: &[u8]) -> Option<StunPing> {
    if buf.len() != STUN_PING_LEN || buf[0] != STUN_PING_CMD {
        return None;
    }
    let id = &buf[1..1 + ONLINE_ID_LEN];
    let end = id.iter().position(|&b| b == 0).unwrap_or(ONLINE_ID_LEN);
    let npid = std::str::from_utf8(&id[..end]).ok()?.to_string();
    if npid.is_empty() {
        return None;
    }
    Some(StunPing {
        npid,
        local_ip: [buf[17], buf[18], buf[19], buf[20]],
    })
}

/// Bind `bind` and record every well-formed ping. Never answers: the client reads no reply.
pub async fn serve(bind: SocketAddr, state: Arc<ServerState>) {
    let socket = match UdpSocket::bind(bind).await {
        Ok(s) => s,
        Err(e) => {
            // Non-fatal: the TCP service is the product, and the client does not send these today.
            warn!(bind = %bind, "signaling UDP listener failed to bind: {e} - RequestSignalingInfos \
                  will answer from room membership only");
            return;
        }
    };
    info!(
        bind = %bind,
        "signaling UDP listener up (21-byte Matching2StunPing). ⚠ expect ZERO packets: shadPS4's \
         EnsureP2PTransport() returns false at HEAD, so the client never sends one"
    );

    let mut buf = [0u8; 64];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, from)) => match parse_stun_ping(&buf[..n]) {
                Some(ping) => {
                    let first = state.signaling_observation(&ping.npid).is_none();
                    state.observe_signaling(&ping.npid, from);
                    if first {
                        info!(
                            npid = %ping.npid,
                            observed = %from,
                            claimed_local_ip = %format!(
                                "{}.{}.{}.{}",
                                ping.local_ip[0], ping.local_ip[1], ping.local_ip[2], ping.local_ip[3]
                            ),
                            "⭐ FIRST signaling registration - the client's P2P transport is live"
                        );
                    } else {
                        debug!(npid = %ping.npid, observed = %from, "signaling registration");
                    }
                }
                None => debug!(from = %from, bytes = n, "ignored non-StunPing datagram"),
            },
            Err(e) => {
                warn!("signaling recv failed: {e}");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ping_bytes(npid: &str, ip: [u8; 4]) -> Vec<u8> {
        let mut v = vec![0u8; STUN_PING_LEN];
        v[0] = STUN_PING_CMD;
        v[1..1 + npid.len()].copy_from_slice(npid.as_bytes());
        v[17..21].copy_from_slice(&ip);
        v
    }

    #[test]
    fn the_struct_is_twenty_one_bytes_and_the_id_is_sixteen() {
        // shadPS4 static_asserts both. If either ever changes, the parse below reads garbage.
        assert_eq!(STUN_PING_LEN, 1 + ONLINE_ID_LEN + 4);
        assert_eq!(STUN_PING_LEN, 21);
        assert_eq!(ONLINE_ID_LEN, 16);
    }

    #[test]
    fn a_well_formed_ping_parses() {
        let p = parse_stun_ping(&ping_bytes("PlayerA", [10, 77, 0, 2])).unwrap();
        assert_eq!(p.npid, "PlayerA");
        assert_eq!(p.local_ip, [10, 77, 0, 2]);
    }

    #[test]
    fn a_sixteen_character_id_uses_the_whole_field_with_no_terminator() {
        let full = "ABCDEFGHIJKLMNOP"; // exactly ORBIS_NP_ONLINEID_MAX_LENGTH
        assert_eq!(full.len(), ONLINE_ID_LEN);
        assert_eq!(parse_stun_ping(&ping_bytes(full, [0; 4])).unwrap().npid, full);
    }

    /// The negatives, each with the positive control implied by the test above.
    #[test]
    fn anything_that_is_not_this_struct_is_ignored_rather_than_guessed_at() {
        let good = ping_bytes("PlayerA", [1, 2, 3, 4]);
        assert!(parse_stun_ping(&good).is_some());

        let mut wrong_cmd = good.clone();
        wrong_cmd[0] = 0x02;
        assert!(parse_stun_ping(&wrong_cmd).is_none());

        assert!(parse_stun_ping(&good[..20]).is_none(), "short");
        let mut long = good.clone();
        long.push(0);
        assert!(parse_stun_ping(&long).is_none(), "long");

        let mut anon = good.clone();
        anon[1..17].fill(0);
        assert!(parse_stun_ping(&anon).is_none(), "an empty online id names nobody");

        let mut not_utf8 = good.clone();
        not_utf8[1] = 0xFF;
        assert!(parse_stun_ping(&not_utf8).is_none());

        // A `SHAD` peer-handshake packet is 0x32 bytes and must not be mistaken for a ping.
        assert!(parse_stun_ping(&vec![b'S'; 0x32]).is_none());
    }
}
