// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The synthetic local identity this server hands out.
//!
//! Deliberately fixed and boring: a preservation lab wants the *same* identity every run so traces
//! are comparable. Nothing here is a PSN credential and nothing is minted from Sony material - it is
//! a local fixture, mirroring the fixed `1111111111111111` account the CFW console gets.

/// A synthetic NP identity. Stable across runs by design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// NP online ID (the "handle"). Max 16 chars on real hardware.
    pub npid: String,
    /// ShadNet user id handed back in `LoginReply`.
    pub user_id: u64,
    /// Opaque session token echoed by `GetToken`.
    pub token: String,
    /// Avatar URL. Must be non-empty: shadPS4 treats an empty `LoginReply` proto as malformed.
    pub avatar_url: String,
}

/// How the server answers `Login`/`GetToken` when more than one participant connects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityMode {
    /// Every participant is handed the **configured** npid and token; only `user_id` differs.
    ///
    /// The default, and byte-identical to what this server did before rooms existed. Room
    /// membership still keys on the npid the *client* reports, so two participants are never
    /// conflated even in this mode.
    Fixed,
    /// Each participant is handed **its own** npid and a derived token.
    ///
    /// For the two-instance harness, where handing both instances the same handle is precisely the
    /// conflation §4 of the plan warns about.
    PerClient,
}

/// Whether the room commands (100-115) are served at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub identity: Identity,
    /// Whether to advertise NP Matching2 support in `ServerFeaturesReply`.
    ///
    /// ⭐ **Still off by default, deliberately, and the recon that produced the room protocol is
    /// the reason.** `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §0/§7: D1's managed session is the PSN
    /// Session WebAPI over HTTP, no game-side code reaches `sceNpMatching2`, and **nothing has
    /// asked**. Turning it on also arms shadPS4's 31314/UDP expectation and its UPnP path for
    /// nothing, and §3.5 records a provenance trap: with the flag *off*, `GetWorldInfoList(111)` is
    /// answered by the **emulator itself**, so any advance seen past a world-list gate while it is
    /// off was caused by shadPS4 rather than by anything on the wire.
    pub matching2_enabled: bool,
    /// Serve commands 100-115. Defaults to whatever `matching2_enabled` says, so the one flag an
    /// operator reaches for does the expected thing; `--rooms` decouples them for experiments.
    pub rooms_enabled: bool,
    pub identity_mode: IdentityMode,
    /// The **host-order UDP port published for every room member**.
    ///
    /// ⚠ This is the one number in the room protocol that is neither observed nor derived. Nothing
    /// on the wire carries it: `CreateRoomRequest`/`JoinRoomRequest` have no port field, and
    /// shadPS4's `GetP2PConfiguredPort()` returns **0** at HEAD (`p2p_sockets.cpp:140`). The
    /// default 3074 is the UDP port D1's guest is **observed** to bind (`PLAN-B` §6.1, alongside
    /// 2001 and 2002). Those two instruments disagree and neither is authoritative - recorded as a
    /// finding, not reconciled. Publishing 0 instead is not the safe option: the client would then
    /// block its ShadNet reader thread for 5 s per peer inside `RequestSignalingInfos`.
    pub peer_port: u16,
    /// Listen on `bind.port + 1`/UDP for the client's 21-byte registration ping.
    pub signaling_udp: bool,
}

impl ServerConfig {
    /// The `user_id` the very first participant receives - the configured one, so a
    /// single-instance run does not change.
    pub fn base_user_id(&self) -> u64 {
        self.identity.user_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mode_default_is_the_one_that_does_not_change_live_runs() {
        // Not a tautology: it asserts which variant the CLI default maps to, which is the thing
        // an operator would be surprised by.
        let cfg = ServerConfig {
            identity: Identity {
                npid: "D1Guardian".into(),
                user_id: 1,
                token: "t".into(),
                avatar_url: "u".into(),
            },
            matching2_enabled: false,
            rooms_enabled: false,
            identity_mode: IdentityMode::Fixed,
            peer_port: 3074,
            signaling_udp: false,
        };
        assert_eq!(cfg.base_user_id(), 1);
        assert!(!cfg.matching2_enabled);
    }
}
