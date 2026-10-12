// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `shadnet-server` - serve the local ShadNet surface, or run its controls.
//!
//! ## Safety
//!
//! Binds **loopback by default** and never makes an outbound connection. It speaks only shadPS4's
//! own protocol to a local client. It mints nothing derived from Sony or Bungie material: the
//! identity is a local fixture, deliberately fixed so runs are comparable.
//!
//! ## ⚠ The CLI stayed backwards-compatible on purpose
//!
//! `capture-rig/run-signon-experiment.sh` and `run-contained-emulator.sh` invoke this binary with
//! bare flags and no subcommand, and the operator is driving live runs against it. So `serve` is
//! the **default** subcommand: `shadnet-server --bind 127.0.0.1:31313` still works unchanged, and
//! `shadnet-server controls` is new. Nothing in `capture-rig/` had to be edited, which is the point
//! - changing a launcher mid-run is how a run gets attributed to the wrong variable.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use shadnet_server::identity::{Identity, IdentityMode, ServerConfig};
use shadnet_server::registry::ServerState;
use shadnet_server::session::Session;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "shadnet-server",
    about = "Local ShadNet server (Layer A) for Destiny 1 preservation",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    #[command(flatten)]
    serve: ServeArgs,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Listen for the client. This is the default when no subcommand is given.
    Serve(ServeArgs),
    /// Run every control. **Exit 2 means a control failed and the output may not be cited.**
    Controls,
}

#[derive(clap::Args, Debug, Clone)]
struct ServeArgs {
    /// Address to listen on. Keep this on loopback unless you have a reason not to.
    ///
    /// ⚠ For a two-instance run this **must** be `10.77.0.1` or similar: loopback is
    /// per-namespace, so a loopback-bound server is invisible to both guests.
    #[arg(long, default_value_t = default_bind())]
    bind: SocketAddr,

    /// NP online ID handed to the client (real hardware caps this at 16 characters).
    #[arg(long, default_value = "D1Guardian")]
    npid: String,

    /// ShadNet user id returned by Login. The first participant gets exactly this; later distinct
    /// npids get successive values, so two participants are never handed one identity.
    #[arg(long, default_value_t = 1)]
    user_id: u64,

    /// Opaque session token returned by GetToken.
    #[arg(long, default_value = "d1-local-token")]
    token: String,

    /// Avatar URL. Must be non-empty - an empty LoginReply proto is rejected as malformed.
    #[arg(long, default_value = "http://127.0.0.1/avatar.png")]
    avatar_url: String,

    /// Advertise NP Matching2 support in `ServerFeaturesReply`, and serve commands 100-115.
    ///
    /// ⭐ **Off by default and it should stay off.** Nothing in Destiny has asked: no game-side
    /// code reaches the `sceNpMatching2` import library. Turning it on also arms shadPS4's
    /// 31314/UDP expectation and its UPnP path. See `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §7.
    #[arg(long)]
    matching2: bool,

    /// Serve the room commands independently of what `ServerFeaturesReply` advertises.
    ///
    /// `auto` (default) follows `--matching2`. Use `on` to serve rooms while still advertising
    /// `matching2_enabled=false` - which is the only configuration in which a room command reaching
    /// us is *evidence*, because the emulator short-circuits every one of them except
    /// `ContextStart`/`ContextStop` while the flag is false.
    #[arg(long, value_enum, default_value_t = RoomsFlag::Auto)]
    rooms: RoomsFlag,

    /// How to answer Login/GetToken when more than one participant connects.
    #[arg(long, value_enum, default_value_t = IdentityModeArg::Fixed)]
    identity_mode: IdentityModeArg,

    /// Host-order UDP port published for every room member.
    ///
    /// ⚠ The one number here that is neither observed on the wire nor derivable: no request field
    /// carries it and shadPS4's `GetP2PConfiguredPort()` returns 0 at HEAD. 3074 is the port D1's
    /// guest is **observed** to bind. Set it to 0 only if you want members refused rather than
    /// published (which is what a `MemberEndpointUnset` refusal in the log means).
    #[arg(long, default_value_t = 3074)]
    peer_port: u16,

    /// Listen on `<bind port> + 1`/UDP for the client's 21-byte Matching2 registration ping.
    ///
    /// ⚠ Expect **zero** packets: `SendMatching2StunPing` bails at `EnsureP2PTransport()`, which
    /// returns false at HEAD. Enabled with `--matching2` unless `--no-signaling-udp`.
    #[arg(long)]
    signaling_udp: bool,

    /// Never bind the signaling UDP port, even with `--matching2`.
    #[arg(long)]
    no_signaling_udp: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum RoomsFlag {
    Auto,
    On,
    Off,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum IdentityModeArg {
    /// Everyone gets the configured npid/token; only `user_id` differs. Byte-identical to the
    /// pre-rooms server for a single participant.
    Fixed,
    /// Each participant is handed its own npid and a derived token.
    PerClient,
}

fn default_bind() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], shadnet_wire::DEFAULT_PORT))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Cmd::Controls) => std::process::exit(shadnet_server::controls::main()),
        Some(Cmd::Serve(args)) => serve(args).await,
        None => serve(cli.serve).await,
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .init();

    if args.npid.chars().count() > 16 {
        warn!(
            npid = %args.npid,
            "npid is longer than the 16-character NP online-ID limit on real hardware"
        );
    }
    if !args.bind.ip().is_loopback() {
        warn!(
            bind = %args.bind,
            "binding a non-loopback address - make sure this is deliberate and the segment is contained"
        );
    }

    let rooms_enabled = match args.rooms {
        RoomsFlag::Auto => args.matching2,
        RoomsFlag::On => true,
        RoomsFlag::Off => false,
    };
    let signaling_udp = !args.no_signaling_udp && (args.signaling_udp || args.matching2);

    let config = ServerConfig {
        identity: Identity {
            npid: args.npid,
            user_id: args.user_id,
            token: args.token,
            avatar_url: args.avatar_url,
        },
        matching2_enabled: args.matching2,
        rooms_enabled,
        identity_mode: match args.identity_mode {
            IdentityModeArg::Fixed => IdentityMode::Fixed,
            IdentityModeArg::PerClient => IdentityMode::PerClient,
        },
        peer_port: args.peer_port,
        signaling_udp,
    };

    let state = Arc::new(ServerState::new(config));

    let listener = TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("failed to bind {}", args.bind))?;

    info!(
        bind = %args.bind,
        npid = %state.config.identity.npid,
        user_id = state.config.identity.user_id,
        matching2 = state.config.matching2_enabled,
        rooms = state.config.rooms_enabled,
        identity_mode = ?state.config.identity_mode,
        peer_port = state.config.peer_port,
        protocol_version = shadnet_wire::PROTOCOL_VERSION,
        "ShadNet server listening"
    );
    if state.config.rooms_enabled {
        warn!(
            "⚠ ROOM COMMANDS (100-115) ARE BEING SERVED. On current evidence D1 never sends one - \
             historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md §0/§3 marks this contingent. If a CreateRoom or \
             JoinRoom appears in the ledger below, that is a finding: record it and run §9.1."
        );
    }
    if state.config.matching2_enabled {
        warn!(
            "⚠ matching2_enabled=true is advertised. Note §3.5: with it FALSE the emulator answers \
             GetWorldInfoList itself, so the two configurations are not comparable."
        );
    }

    if state.config.signaling_udp {
        let udp_bind = SocketAddr::new(args.bind.ip(), args.bind.port().wrapping_add(1));
        let st = state.clone();
        tokio::spawn(async move { shadnet_server::signaling::serve(udp_bind, st).await });
    }

    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let st = state.clone();
                    tokio::spawn(async move { Session::run(stream, peer, st).await });
                }
                Err(e) => warn!("accept failed: {e}"),
            },
            _ = tokio::signal::ctrl_c() => {
                info!(
                    rooms = state.rooms.lock().unwrap().len(),
                    sessions = state.session_count(),
                    signaling_observations = state.signaling_count(),
                    "shutting down"
                );
                return Ok(());
            }
        }
    }
}
