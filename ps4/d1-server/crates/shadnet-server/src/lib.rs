// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Layer A of the Destiny 1 private-server stack: a local ShadNet server.
//!
//! ## What this is for
//!
//! shadPS4 will not report a signed-in NP identity - `sceNpGetOnlineId` returns `SIGNED_OUT` -
//! unless its ShadNet client authenticates against a server. Destiny sits at its title screen
//! behind exactly that condition. This server satisfies it **locally**, then records everything the
//! game asks for next.
//!
//! ## What was added, and the honest status of it
//!
//! Commands **100-115**, the NP Matching2 room protocol: contexts, create/join/leave/search, member
//! tracking, room data internal and external, kickout, room messages, `RequestSignalingInfos`, and
//! the **notifications** that tell the other participant anything happened.
//!
//! ⚠⚠ **`historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §3 marks all of that CONTINGENT, and that has not
//! changed.** The evidence (`[I]`-strong, §0/§1 of that document) is that **D1 never calls
//! `sceNpMatching2` at all**: no game-side code reaches the import library, its managed session is
//! the PSN Session WebAPI over HTTP, and its peer key material is minted locally by the client's
//! own MT19937 PRNG rather than issued by anybody. So this is the implemented *branch*, not a
//! deliverable that something asked for. It is **off by default** (`--matching2` / `--rooms`), the
//! `ServerFeaturesReply` still advertises `matching2_enabled=false`, and the one-command experiment
//! that would retire it formally is §9.1: `grep Lib_NpMatching2 <run>.log`, with `Lib_NpWebApi` in
//! the same grep as the positive control that the log surface is on.
//!
//! ## What is deliberately NOT here
//!
//! **Session-key issuance.** The C7 verdict is that nobody issues it: `bdSecurityKeyMap::initKey`
//! (`0xF875A0`) draws 8 + 16 random bytes from Demonware's MT19937, takes no input, parses no
//! response and always returns true. A key scheme invented here would work only against itself and
//! would look exactly like success.

pub mod controls;
pub mod gates;
pub mod identity;
pub mod matching;
pub mod registry;
pub mod rooms;
pub mod session;
pub mod signaling;

/// The vendored schema, embedded at compile time so a control can re-derive its hash instead of
/// trusting a transcription. `shadnet-proto`'s `build.rs` compiles the same path.
pub const VENDORED_PROTO: &str = include_str!("../../../proto/shadnet.proto");

/// Verbatim enum excerpts from shadPS4's `client.h` and `np_types2.h`.
///
/// This exists so the controls can check our Rust enums against **the client's own text** rather
/// than against numbers we typed in ourselves - the difference between a control and a decoration.
pub const SHADPS4_CLIENT_CONSTANTS: &str = include_str!("../../../proto/shadps4-client-constants.h");

/// Where the shadPS4 source tree lives, for the controls that re-derive the excerpt against it.
pub fn shadps4_src_dir() -> std::path::PathBuf {
    std::env::var("SHADPS4_SRC")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../shadPS4-d1trace").to_string())
        .into()
}
