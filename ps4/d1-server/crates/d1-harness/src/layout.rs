// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The two-instance layout: **one run id, two instances, two participants**, every mutable root
//! separate, every endpoint deterministic and non-colliding.
//!
//! ## What this module is allowed to do
//!
//! Nothing but arithmetic and string formatting. It **creates no directory, opens no socket, reads
//! no configuration and launches nothing** - `historicaldocs/design/two-instance-harness-design.md` calls that out
//! explicitly and [`controls`](crate::controls) asserts it by snapshotting a scratch tree around a
//! generation.
//!
//! ## Where the numbers come from (and where they do not)
//!
//! * `10.77.0.0/24`, `d1net`, "no default route" - `capture-rig/run-signon-experiment.sh`, which
//!   already builds exactly this shape for one participant. The dummy interface becomes a veth pair
//!   onto a bridge because two participants need L2 between them; nothing else changes.
//! * **3074/UDP is not ours to allocate.** D1's guest binds it itself, which is why
//!   `d1-responder --bap-port` documents "MUST NOT be 3074". Both participants therefore carry the
//!   *same port number* on *different addresses*, and that is only legitimate because each guest
//!   lives in its own network namespace - see [`Scope::NamespaceLocal`].
//! * The account/character SOIDs and the message-503 body for **participant A** are the pinned
//!   known-good values from `capture-rig/run-boot.sh`. A two-instance run that cannot reproduce the
//!   single-instance boot is not a two-instance run, it is a new experiment.
//! * Participant **B**'s 503 body is *derived by bit-splicing* its account SOID into A's pinned
//!   template at the one bit offset where A's SOID occurs. See [`splice_account_soid`]. It is not
//!   invented and it is not a second encoder.
//!
//! ⚠ **Reach tag.** Everything the harness produces is `synthetic-minimal`; every finding made
//! through it inherits `synthetic_ancestor`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── pinned constants, each with the file it was taken from ───────────────────────────────────

pub const SCHEMA_VERSION: u32 = 2;

/// The sealed lab subnet. `run-signon-experiment.sh` puts the single participant on `10.77.0.2/24`.
pub const SUBNET: &str = "10.77.0.0/24";
pub const SUBNET_PREFIX: u8 = 24;
/// Where the *shared* services live: reachable from both participant namespaces, owned by neither.
pub const SERVICES_ADDR: &str = "10.77.0.1";
/// Participant A is `.11`, B is `.12`. Deliberately **not** `.2`: a two-instance layout that reuses
/// the single-instance address makes a stale one-participant run look like participant A.
pub const PARTICIPANT_ADDR_BASE: u8 = 11;

/// UDP port D1's **guest** binds for itself. We do not choose it and cannot move it.
pub const GUEST_BOUND_UDP: u16 = 3074;
/// The BAP port `d1-responder` advertises. `--bap-port` defaults to this and its own help says it
/// must never be 3074.
pub const RESPONDER_BAP_UDP: u16 = 3075;
/// ShadNet socket protocol (shadPS4's own backend), TCP.
pub const SHADNET_TCP: u16 = 31313;
/// shadNet's *HTTP* WebAPI - a different service on a different port from the one above.
pub const WEBAPI_TCP: u16 = 31315;
/// PSN OAuth exchange (TLS) and Demonware's `ticket_drop` crash upload (TLS).
pub const OAUTH_TLS: u16 = 443;
pub const TICKET_DROP_TLS: u16 = 32556;
/// `d1-forge`'s control plane. Participant A keeps the default; B gets the next port so a host-side
/// tool can address either without a flag collision.
pub const FORGE_TCP_BASE: u16 = 8788;

/// The account SOID every single-participant run has used. `six-player-fixtures.py` **rejects** it
/// as fixture material; here participant A keeps it on purpose, and participant B must never get it.
pub const ANCHOR_ACCOUNT_SOID: u64 = 0xD100_0000_0000_0001;
pub const ANCHOR_CHARACTER_SOID: u64 = 0xD100_0000_0000_0101;
/// The high byte both SOIDs carry. Preserved for B so the registry keying stays in the same space.
pub const SOID_PREFIX: u64 = 0xD1 << 56;

/// The `D1_503_RAW` template a participant is handed, carrying [`ANCHOR_ACCOUNT_SOID`].
///
/// ⭐⭐ **2026-08-10: this is now the TEN-FLAG body** (`d1_bap::INVESTMENT_503_UNLOCKS_RAW`,
/// `{98,308,419,440,834,2667,3389,5015,5987,6018}` at value 2). It used to be the 45-byte flagless
/// baseline `run-boot.sh` pinned, and that mattered more than it looks: the harness *exports*
/// `D1_503_RAW`, and **env beats default**, so a multi-participant run would have kept shipping
/// `count = 0` to every participant while single-participant runs got the flags - a divergence that
/// shows up only as "rung 33 refuses, but only in six-player runs".
///
/// ⚠ Kept as a literal because this crate does not depend on the wire codec at runtime. The
/// `#[test] template_is_the_body_the_responder_actually_serves` below takes `d1-bap` as a
/// **dev-dependency** and asserts the two are byte-identical - without it this is a third
/// hand-maintained copy of a wire body, which is how the first two drifted.
pub const INV_503_TEMPLATE_HEX: &str = "180000000d1000000000000010000000000000000bfffffffffff\
    ffffc00000000000000020958062f8134f81a3f81b8f8342f8a6bf8d3df9397f9763f9782f0220000000100";
/// Bit offset of the 64-bit account SOID inside [`INV_503_TEMPLATE_HEX`].
///
/// `[I]`-strong, and re-derived rather than trusted: [`find_unique_bit_run`] locates the anchor
/// SOID in the template and a control asserts the answer is **unique** and equals this constant.
/// A value that occurred twice would make the splice ambiguous, and ambiguity here silently hands
/// participant B participant A's account.
pub const INV_503_ACCOUNT_SOID_BIT_OFFSET: usize = 36;

/// The 53-byte family-4 message-206 body `run-boot.sh` used to pin as `D1_206_BODY`.
///
/// ⛔ **No longer exported (2026-08-11).** Since 2026-08-10 the responder answers 206 per REQUEST
/// (family decoded, span built by `build_family_body`), and an exported `D1_206_BODY` is an
/// OVERRIDE that the responder REFUSES whenever its span is not for the family the client asked
/// for - `run-boot.sh` prints a ⛔ warning if it is set. Exporting this to both participants
/// would have force-fed a family-4 span to every 206. Kept as provenance of the historical
/// pinned body only; the anchor control now asserts run-boot.sh does NOT pin it.
pub const INV_206_BODY_HEX: &str =
    "1801900000000800000026880000000000000800000000080000000b2573adfe88000000000000080000004f440000000000000400";

/// NP online-ID length limit on real hardware. `shadnet-server` warns above it; we refuse.
pub const NP_ONLINE_ID_MAX: usize = 16;

// ── scopes, planes, and the vocabulary the collision control speaks ──────────────────────────

/// Where a port number has to be unique.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Bound on the shared lab LAN. `(addr, port)` must be globally unique across the run.
    Lan,
    /// Bound on the host, outside any namespace. The **port number alone** must be unique.
    Host,
    /// Bound inside one participant's network namespace. May repeat across participants **only**
    /// while `lan.netns_separation` is true - that is the whole justification for two guests both
    /// binding 3074.
    NamespaceLocal,
}

/// How a plane is recorded. `Pcap` is the only one that needs `CAP_NET_RAW`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureMethod {
    Pcap,
    /// `ss -tuanp` polled into a log - what `run-signon-experiment.sh` falls back to today.
    SsPoll,
    /// A server-side log written by one of our own processes.
    File,
    /// The UD2 probe tracer's binary output.
    Tracer,
}

/// Who owns an address:port, for the report.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Endpoint {
    pub plane: String,
    pub proto: String,
    pub addr: String,
    pub port: u16,
    pub scope: Scope,
    pub owner: String,
    pub note: String,
}

impl Endpoint {
    pub fn key(&self) -> String {
        format!("{}/{}:{}", self.proto, self.addr, self.port)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct CapturePlane {
    pub name: String,
    pub method: CaptureMethod,
    pub sink: String,
    /// The observation that proves this plane's capture is actually armed. A plane whose canary
    /// never appears is **not armed**, however healthy the capture process looks.
    pub canary: String,
    /// Set by the operator's own arming step, never by the generator.
    pub armed: bool,
}

/// How the two participants' controllers are kept apart.
///
/// ⚠ `Unresolved` is the generated default **and the preflight refuses on it**. It is not an
/// oversight: two pads that present one USB VID/PID are *indistinguishable to SDL*, and
/// `SDL_GAMECONTROLLER_IGNORE_DEVICES` matches on VID/PID alone. Choosing a mechanism is an
/// operator decision, because the half of either mechanism that involves a live client is
/// unexercised.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerIsolation {
    /// No mechanism chosen. **Refused.**
    Unresolved,
    /// The two pads get different USB ids and each instance ignores the other's.
    /// `d1-harness pad run --vendor/--product` mints them (landed 2026-08-11, ported from
    /// `capture-rig/virtual-pad.py`), and `d1-harness pad controls` exercises the kernel half:
    /// two simultaneous pads with distinct ids enumerate as two distinct devices. ⚠ The SDL half
    /// is still open: changing the VID/PID changes the SDL joystick GUID, so the built-in x360
    /// mapping no longer applies - `SDL_GAMECONTROLLERCONFIG` would have to supply one. That a
    /// live client binds such a pad is `[U]`.
    DistinctUsbIds,
    /// Each participant's mount namespace bind-mounts only its own `/dev/input/eventN`. Needs no
    /// pad change at all and cannot be defeated by an SDL mapping. Untested: `[U]`.
    DevInputBindMount,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Controller {
    pub fifo: String,
    pub uinput_name: String,
    pub usb_vendor: String,
    pub usb_product: String,
    pub isolation: ControllerIsolation,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct NpIdentity {
    /// What this instance's `users.json` must report. shadPS4 sends `u->shadnet_npid` in
    /// `LoginRequest`, and `shadnet-server` keys every room and identity on it - so two instances
    /// reporting one npid are **one participant** to the server, whatever else differs.
    pub npid: String,
    /// The `user_id` `shadnet-server` is expected to mint. Informational: in `per-client` mode the
    /// server allocates these itself, first-come.
    pub expected_user_id: u64,
    /// `d1-responder --account-id`.
    pub responder_account_id: u64,
    pub users_json: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct D1Identity {
    pub account_soid: String,
    pub character_soid: String,
    /// `D1_503_RAW` for this participant. A's is the pinned template verbatim; B's is that template
    /// with its own SOID spliced in.
    pub inv_503_raw: String,
    pub inv_503_provenance: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Vehicle {
    pub kind: String,
    pub allowed: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Participant {
    pub slot: String,
    pub index: u8,
    pub instance_id: String,
    pub participant_id: String,
    pub vehicle: Vehicle,
    pub netns: String,
    pub veth_bridge_side: String,
    pub veth_guest_side: String,
    pub addr: String,
    pub prefix: u8,
    pub default_route: String,
    pub endpoints: Vec<Endpoint>,
    pub np: NpIdentity,
    pub d1: D1Identity,
    pub paths: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
    /// shadPS4 has **no `--user-dir` flag**: the user directory is `$XDG_DATA_HOME/shadPS4`, and
    /// only if `$PWD/user` does not exist - `path_util.cpp` tries the portable directory first.
    /// Both facts are checked by the preflight, because a silently shared user directory is a
    /// silently shared identity.
    pub xdg_data_home: String,
    pub emulator_cwd: String,
    pub portable_dir_must_not_exist: String,
    /// Written into `<xdg_data_home>/shadPS4/config.json` by the operator's arming step.
    pub shadps4_settings: BTreeMap<String, String>,
    pub controller: Controller,
    pub capture: Vec<CapturePlane>,
    pub argv: BTreeMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SharedService {
    pub role: String,
    pub bin: String,
    pub argv: Vec<String>,
    pub endpoint: Endpoint,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Lan {
    pub subnet: String,
    pub bridge: String,
    pub services_addr: String,
    pub egress: String,
    pub dns: String,
    pub default_route: String,
    pub ip_forward: u8,
    /// Each participant in its own network namespace. This is what makes two guests on 3074 legal.
    pub netns_separation: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Supervision {
    pub start_order: Vec<String>,
    pub stop_order: Vec<String>,
    pub kill_scope: String,
    pub pid_ledger: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Guards {
    pub one_emulator_guard: String,
    pub activation_review_required: bool,
    pub distinct_identity_required: bool,
    pub nonzero_trace_drops_fail: bool,
    /// A named control run with no instrumentation. Null until the operator records one, and the
    /// preflight refuses while it is null.
    pub uninstrumented_control: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Layout {
    pub schema_version: u32,
    pub generator: String,
    pub run_id: String,
    pub seed_sha256: String,
    pub activation: String,
    pub activation_review: Option<String>,
    pub reach: String,
    pub base_dir: String,
    pub lan: Lan,
    pub shared_readonly_inputs: BTreeMap<String, String>,
    pub shared_services: Vec<SharedService>,
    pub participants: Vec<Participant>,
    pub guards: Guards,
    pub supervision: Supervision,
}

// ── derivation ───────────────────────────────────────────────────────────────────────────────

fn token(seed: &str, label: &str, n: usize) -> String {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update([0u8]);
    h.update(label.as_bytes());
    let d = h.finalize();
    d.iter().map(|b| format!("{b:02x}")).collect::<String>()[..n].to_string()
}

fn derive_u64(seed: &str, label: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update([0u8]);
    h.update(label.as_bytes());
    let d = h.finalize();
    u64::from_be_bytes(d[..8].try_into().unwrap())
}

/// A SOID in the project's `0xD1…` space, never equal to any value in `avoid`.
fn derive_soid(seed: &str, label: &str, avoid: &[u64]) -> u64 {
    for attempt in 0..64u32 {
        let tail = derive_u64(seed, &format!("{label}:{attempt}")) & 0x00FF_FFFF_FFFF_FFFF;
        let soid = SOID_PREFIX | tail;
        if tail != 0 && !avoid.contains(&soid) {
            return soid;
        }
    }
    unreachable!("64 domain-separated draws all collided - the derivation is broken, not unlucky")
}

pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

pub fn bytes_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Read `width` bits, MSB-first, starting at `bit`.
pub fn read_bits(bytes: &[u8], bit: usize, width: usize) -> Option<u64> {
    if width > 64 || bit + width > bytes.len() * 8 {
        return None;
    }
    let mut v = 0u64;
    for i in 0..width {
        let b = bit + i;
        let set = bytes[b / 8] >> (7 - b % 8) & 1;
        v = v << 1 | set as u64;
    }
    Some(v)
}

/// Write `width` bits of `value`, MSB-first, starting at `bit`. Everything else is untouched.
pub fn write_bits(bytes: &mut [u8], bit: usize, width: usize, value: u64) {
    for i in 0..width {
        let b = bit + i;
        let set = (value >> (width - 1 - i)) & 1;
        let mask = 1u8 << (7 - b % 8);
        if set == 1 {
            bytes[b / 8] |= mask;
        } else {
            bytes[b / 8] &= !mask;
        }
    }
}

/// Every bit offset at which `value` occurs as a `width`-bit big-endian run.
pub fn find_bit_runs(bytes: &[u8], value: u64, width: usize) -> Vec<usize> {
    let total = bytes.len() * 8;
    if width > total {
        return Vec::new();
    }
    (0..=total - width)
        .filter(|&i| read_bits(bytes, i, width) == Some(value))
        .collect()
}

/// The single offset at which `value` occurs, or `None` if it occurs zero or more than once.
///
/// **Ambiguity is a failure, not a tie-break.** Two matches would mean the splice below could put
/// participant B's SOID in the wrong field and leave A's in the right one.
pub fn find_unique_bit_run(bytes: &[u8], value: u64, width: usize) -> Option<usize> {
    let hits = find_bit_runs(bytes, value, width);
    if hits.len() == 1 {
        Some(hits[0])
    } else {
        None
    }
}

/// Participant B's `D1_503_RAW`: A's pinned body with the account SOID field replaced.
///
/// Nothing else in the 45-byte body moves - including the family-5 root at bit 165, which is not
/// per-participant. The control that makes this trustworthy is in [`crate::controls`]: splicing
/// **A's own** SOID back must reproduce the pinned bytes byte-for-byte.
pub fn splice_account_soid(template: &[u8], bit_offset: usize, soid: u64) -> Vec<u8> {
    let mut out = template.to_vec();
    write_bits(&mut out, bit_offset, 64, soid);
    out
}

// ── generation ───────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Options {
    pub seed: String,
    /// Directory the run tree *would* live under. Not created here.
    pub root: String,
    pub repo: String,
    pub emulator_bin: String,
    pub emulator_cwd: String,
    pub game: String,
    pub patch: String,
    /// Participant A reuses `run-boot.sh`'s pinned identity so the known-good boot is reproducible.
    /// Turn it off only for an experiment that deliberately abandons that comparison.
    pub anchor_a: bool,
}

impl Default for Options {
    fn default() -> Self {
        let repo = default_repo();
        Options {
            seed: "d1-two-participant-v2".into(),
            root: format!("{repo}/runs"),
            emulator_bin: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../shadPS4-d1trace/build-d1trace/shadps4").into(),
            emulator_cwd: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../shadPS4-main").into(),
            game: format!("{repo}/dumps/patch-01.29-CUSA00219/eboot.bin"),
            patch: format!("{repo}/patches/CUSA00219-lab-manifest-T1.xml"),
            anchor_a: true,
            repo,
        }
    }
}

/// Repo root: `$D1_REPO` if set, else derived from this crate's compile-time location.
pub fn default_repo() -> String {
    if let Ok(p) = std::env::var("D1_REPO") {
        if !p.is_empty() {
            return p;
        }
    }
    // …/<repo>/d1-server/crates/d1-harness
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(3)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".into())
}

pub fn build(opts: &Options) -> Layout {
    let seed = &opts.seed;
    let run_id = format!("p2p-{}", token(seed, "run", 12));
    let base = format!("{}/{}", opts.root, run_id);

    let manifest = format!("{}/capture-rig/manifests/full_en.bin", opts.repo);
    let profile = format!("{}/capture-rig/profiles/profile-old-russia.bin", opts.repo);
    let responder = format!("{}/d1-server/target/release/d1-responder", opts.repo);
    let shadnet = format!("{}/d1-server/target/release/shadnet-server", opts.repo);

    let template = hex_to_bytes(INV_503_TEMPLATE_HEX).expect("pinned template is valid hex");

    let mut participants = Vec::new();
    for (index, slot) in ["a", "b"].into_iter().enumerate() {
        let index = index as u8;
        let addr = format!("10.77.0.{}", PARTICIPANT_ADDR_BASE + index);
        let proot = format!("{base}/{slot}");

        let anchored = opts.anchor_a && index == 0;
        let account_soid = if anchored {
            ANCHOR_ACCOUNT_SOID
        } else {
            derive_soid(seed, &format!("account:{slot}"), &[ANCHOR_ACCOUNT_SOID, ANCHOR_CHARACTER_SOID])
        };
        let character_soid = if anchored {
            ANCHOR_CHARACTER_SOID
        } else {
            derive_soid(
                seed,
                &format!("character:{slot}"),
                &[ANCHOR_ACCOUNT_SOID, ANCHOR_CHARACTER_SOID, account_soid],
            )
        };
        let npid = if anchored {
            "D1Guardian".to_string()
        } else {
            format!("D1G-{slot}-{}", token(seed, &format!("npid:{slot}"), 6))
        };
        let (raw_503, raw_prov) = if anchored {
            (
                INV_503_TEMPLATE_HEX.to_string(),
                "run-boot.sh pinned D1_503_RAW, verbatim".to_string(),
            )
        } else {
            (
                bytes_to_hex(&splice_account_soid(
                    &template,
                    INV_503_ACCOUNT_SOID_BIT_OFFSET,
                    account_soid,
                )),
                format!(
                    "run-boot.sh template with the 64-bit account SOID spliced at bit {INV_503_ACCOUNT_SOID_BIT_OFFSET}"
                ),
            )
        };

        let paths: BTreeMap<String, String> = [
            "user", "config", "save", "controller", "logs", "trace", "pcap", "provenance",
            "uploads", "capture",
        ]
        .into_iter()
        .map(|k| (k.to_string(), format!("{proot}/{k}")))
        .collect();

        // Per-participant USB product id: A keeps SDL's built-in x360 GUID, B takes the next id.
        // Distinct ids are what makes two pads two devices to SDL (the two-instance blocker);
        // whether a live client binds the non-default GUID is still [U], which is why
        // `controller.isolation` stays Unresolved until the operator chooses.
        let pad_name = format!("D1 Preservation Virtual Pad {}", slot.to_uppercase());
        let usb_vendor = "0x045E".to_string();
        let usb_product = format!("0x{:04X}", 0x028Eu16 + index as u16);

        let forge_port = FORGE_TCP_BASE + index as u16;
        let http_ports = format!("80,8080,{WEBAPI_TCP},{forge_port}");

        let endpoints = vec![
            Endpoint {
                plane: "peer".into(),
                proto: "udp".into(),
                addr: addr.clone(),
                port: GUEST_BOUND_UDP,
                scope: Scope::NamespaceLocal,
                owner: "destiny-guest".into(),
                note: "the guest binds this itself; identical port on both participants is legal \
                       ONLY because each is in its own netns"
                    .into(),
            },
            Endpoint {
                plane: "bap".into(),
                proto: "udp".into(),
                addr: addr.clone(),
                port: RESPONDER_BAP_UDP,
                scope: Scope::Lan,
                owner: "d1-responder".into(),
                note: "advertised BAP endpoint (D1_BAP_IP/--bap-port); never 3074".into(),
            },
            Endpoint {
                plane: "signon".into(),
                proto: "tcp".into(),
                addr: addr.clone(),
                port: OAUTH_TLS,
                scope: Scope::Lan,
                owner: "d1-responder".into(),
                note: "PSN OAuth exchange, TLS".into(),
            },
            Endpoint {
                plane: "signon".into(),
                proto: "tcp".into(),
                addr: addr.clone(),
                port: TICKET_DROP_TLS,
                scope: Scope::Lan,
                owner: "d1-responder".into(),
                note: "Demonware ticket_drop crash upload, TLS".into(),
            },
            Endpoint {
                plane: "webapi".into(),
                proto: "tcp".into(),
                addr: addr.clone(),
                port: WEBAPI_TCP,
                scope: Scope::Lan,
                owner: "d1-responder".into(),
                note: "shadNet HTTP WebAPI - where the 78-byte PSN session blob would be stored"
                    .into(),
            },
            Endpoint {
                plane: "control".into(),
                proto: "tcp".into(),
                addr: addr.clone(),
                port: forge_port,
                scope: Scope::Host,
                owner: "d1-forge".into(),
                note: "loadout control plane; unique by port number so a host-side tool can address \
                       either participant"
                    .into(),
            },
        ];

        let capture = vec![
            CapturePlane {
                name: "platform".into(),
                method: CaptureMethod::SsPoll,
                sink: format!("{proot}/capture/platform-sockets.log"),
                canary: "the ShadNet TCP connection to 10.77.0.1:31313 appears in the first poll"
                    .into(),
                armed: false,
            },
            CapturePlane {
                name: "signon".into(),
                method: CaptureMethod::File,
                sink: format!("{proot}/logs/responder.log"),
                canary: "one logged POST /2.0/oauth/token for THIS participant's npid".into(),
                armed: false,
            },
            CapturePlane {
                name: "bap".into(),
                method: CaptureMethod::File,
                sink: format!("{proot}/logs/responder.log"),
                canary: "the investment-503 reply carrying THIS participant's account SOID".into(),
                armed: false,
            },
            CapturePlane {
                name: "peer".into(),
                method: CaptureMethod::Pcap,
                sink: format!("{proot}/pcap/peer.pcap"),
                canary: "one datagram whose peer is the OTHER participant's address on 3074".into(),
                armed: false,
            },
            CapturePlane {
                name: "trace".into(),
                method: CaptureMethod::Tracer,
                sink: format!("{proot}/trace/account-trace.bin"),
                canary: "a non-empty trace file - an armed tracer that never fires is \
                         indistinguishable from a disarmed one"
                    .into(),
                armed: false,
            },
        ];

        let mut env: BTreeMap<String, String> = BTreeMap::new();
        // shared, read-only inputs
        env.insert("D1_MANIFEST_FILE".into(), manifest.clone());
        env.insert("D1_PROFILE_FILE".into(), profile.clone());
        // ⛔ D1_206_BODY is DELIBERATELY NOT EXPORTED (2026-08-11): the responder answers 206
        // per request since 2026-08-10 and refuses a family-mismatched override. See
        // INV_206_BODY_HEX's doc comment - env beats default, so exporting the old pinned
        // family-4 body here would have overridden the fix for every participant.
        // per-participant identity
        env.insert("D1_503_SOID".into(), "1".into());
        env.insert("D1_503_RAW".into(), raw_503.clone());
        env.insert("D1_ACCOUNT_SOID".into(), format!("0x{account_soid:016X}"));
        env.insert("D1_CHARACTER_SOID".into(), format!("0x{character_soid:016X}"));
        env.insert("D1_PUSH_CHARACTER".into(), "1".into());
        env.insert("D1_PUSH_AT".into(), "echo".into());
        env.insert("D1_PUSH_FAMILY".into(), "4,3".into());
        env.insert("D1_INV_SILENT".into(), String::new());
        // per-participant network
        env.insert("D1_IP".into(), addr.clone());
        env.insert("D1_BAP_IP".into(), addr.clone());
        env.insert("D1_BAP_PORT".into(), RESPONDER_BAP_UDP.to_string());
        // per-participant roots
        env.insert("RUN_OUT".into(), proot.clone());
        env.insert("XDG_DATA_HOME".into(), format!("{proot}/user"));
        env.insert("D1_UPLOAD_DIR".into(), format!("{proot}/uploads"));
        env.insert(
            "D1_ACCOUNT_TRACE_OUT".into(),
            format!("{proot}/trace/account-trace.bin"),
        );
        env.insert("PAD_FIFO".into(), format!("{proot}/controller/pad.fifo"));
        // the vehicle
        env.insert("SHADPS4_BIN".into(), opts.emulator_bin.clone());
        env.insert("DIAG_PATCH".into(), opts.patch.clone());
        env.insert("GAME".into(), opts.game.clone());

        let mut argv: BTreeMap<String, Vec<String>> = BTreeMap::new();
        argv.insert(
            "d1-responder".into(),
            vec![
                responder.clone(),
                "--bind".into(),
                addr.clone(),
                "--npid".into(),
                npid.clone(),
                "--account-id".into(),
                (index as u64 + 1).to_string(),
                "--http-ports".into(),
                http_ports,
                "--tls-ports".into(),
                format!("{OAUTH_TLS},{TICKET_DROP_TLS}"),
                "--bap-port".into(),
                RESPONDER_BAP_UDP.to_string(),
                "--cert".into(),
                format!("{proot}/config/tls.crt"),
                "--key".into(),
                format!("{proot}/config/tls.key"),
            ],
        );
        argv.insert(
            "emulator".into(),
            vec![
                opts.emulator_bin.clone(),
                "-p".into(),
                opts.patch.clone(),
                "-g".into(),
                opts.game.clone(),
            ],
        );
        // 2026-08-11: the pad is `d1-harness pad run` (the Rust port of
        // `capture-rig/virtual-pad.py`), which grew `--vendor/--product` precisely for this
        // seam - the Python hardcodes one identity, so two of it are one device to SDL.
        argv.insert(
            "virtual-pad".into(),
            vec![
                format!("{}/d1-server/target/release/d1-harness", opts.repo),
                "pad".into(),
                "run".into(),
                "--fifo".into(),
                format!("{proot}/controller/pad.fifo"),
                "--name".into(),
                pad_name.clone(),
                "--vendor".into(),
                usb_vendor.clone(),
                "--product".into(),
                usb_product.clone(),
            ],
        );

        let mut settings = BTreeMap::new();
        settings.insert(
            "shadnet_server".into(),
            format!("{SERVICES_ADDR}:{SHADNET_TCP}"),
        );
        settings.insert(
            "shadnet_webapi_server".into(),
            format!("http://{addr}:{WEBAPI_TCP}"),
        );

        participants.push(Participant {
            slot: slot.into(),
            index,
            instance_id: format!("instance-{slot}-{}", token(seed, &format!("instance:{slot}"), 8)),
            participant_id: format!(
                "participant-{slot}-{}",
                token(seed, &format!("participant:{slot}"), 8)
            ),
            vehicle: Vehicle {
                kind: "unspecified".into(),
                allowed: vec![
                    "shadps4".into(),
                    "contained_cfw_ps4".into(),
                    "retail_ps4_later".into(),
                ],
            },
            netns: format!("d1p-{slot}-{}", token(seed, "run", 12)),
            veth_bridge_side: format!("v{slot}br"),
            veth_guest_side: format!("v{slot}in"),
            addr: addr.clone(),
            prefix: SUBNET_PREFIX,
            default_route: "none".into(),
            endpoints,
            np: NpIdentity {
                npid: npid.clone(),
                expected_user_id: index as u64 + 1,
                responder_account_id: index as u64 + 1,
                users_json: format!("{proot}/user/shadPS4/users.json"),
            },
            d1: D1Identity {
                account_soid: format!("0x{account_soid:016X}"),
                character_soid: format!("0x{character_soid:016X}"),
                inv_503_raw: raw_503,
                inv_503_provenance: raw_prov,
            },
            paths,
            env,
            xdg_data_home: format!("{proot}/user"),
            emulator_cwd: opts.emulator_cwd.clone(),
            portable_dir_must_not_exist: format!("{}/user", opts.emulator_cwd),
            shadps4_settings: settings,
            controller: Controller {
                fifo: format!("{proot}/controller/pad.fifo"),
                uinput_name: pad_name,
                usb_vendor,
                usb_product,
                isolation: ControllerIsolation::Unresolved,
                note: "d1-harness pad run mints per-participant names and USB ids (A 0x028E, \
                       B 0x028F); kernel-level distinctness is exercised by `d1-harness pad \
                       controls`, the SDL-GUID mapping half is not. Choose a mechanism before \
                       arming."
                    .into(),
            },
            capture,
            argv,
        });
    }

    let mut shared_readonly_inputs = BTreeMap::new();
    shared_readonly_inputs.insert("content_manifest".into(), manifest);
    shared_readonly_inputs.insert("profile_object".into(), profile);
    shared_readonly_inputs.insert("game_eboot".into(), opts.game.clone());
    shared_readonly_inputs.insert("patch".into(), opts.patch.clone());
    shared_readonly_inputs.insert("emulator".into(), opts.emulator_bin.clone());

    let shared_services = vec![SharedService {
        role: "shadnet-server".into(),
        bin: shadnet,
        argv: vec![
            "serve".into(),
            "--bind".into(),
            format!("{SERVICES_ADDR}:{SHADNET_TCP}"),
            "--identity-mode".into(),
            "per-client".into(),
            "--rooms".into(),
            "off".into(),
        ],
        endpoint: Endpoint {
            plane: "platform".into(),
            proto: "tcp".into(),
            addr: SERVICES_ADDR.into(),
            port: SHADNET_TCP,
            scope: Scope::Lan,
            owner: "shadnet-server".into(),
            note: "ONE shared instance, deliberately: in per-client mode it refuses a second login \
                   for a live npid (LoginAlreadyLoggedIn=6), so identity conflation becomes an \
                   observed refusal instead of a declared invariant"
                .into(),
        },
        note: "rooms OFF and matching2 OFF - PLAN-B §0/§7: nothing in D1 reaches sceNpMatching2. \
               If a room command ever arrives here, that is a finding."
            .into(),
    }];

    Layout {
        schema_version: SCHEMA_VERSION,
        generator: "d1-harness layout".into(),
        run_id,
        seed_sha256: {
            let mut h = Sha256::new();
            h.update(seed.as_bytes());
            bytes_to_hex(&h.finalize())
        },
        activation: "dry_run_only".into(),
        activation_review: None,
        reach: "synthetic-minimal; everything downstream inherits synthetic_ancestor".into(),
        base_dir: base.clone(),
        lan: Lan {
            subnet: SUBNET.into(),
            bridge: "d1lan".into(),
            services_addr: SERVICES_ADDR.into(),
            egress: "deny".into(),
            dns: "deny".into(),
            default_route: "none".into(),
            ip_forward: 0,
            netns_separation: true,
        },
        shared_readonly_inputs,
        shared_services,
        participants,
        guards: Guards {
            one_emulator_guard: "ACTIVE - lifted only by an explicit operator activation review"
                .into(),
            activation_review_required: true,
            distinct_identity_required: true,
            nonzero_trace_drops_fail: true,
            uninstrumented_control: None,
        },
        supervision: Supervision {
            start_order: vec![
                "containment_assert".into(),
                "identity_assert".into(),
                "capture_start".into(),
                "shared_services_start".into(),
                "participant_a_start".into(),
                "participant_b_start".into(),
            ],
            stop_order: vec![
                "participant_b_stop".into(),
                "participant_a_stop".into(),
                "shared_services_stop".into(),
                "capture_stop".into(),
                "loss_assert".into(),
                "artifact_manifest".into(),
            ],
            kill_scope: "resolved process ids recorded in the pid ledger, each re-verified by \
                         /proc start-time before any signal; no pattern, ever"
                .into(),
            pid_ledger: format!("{base}/pids.json"),
        },
    }
}

// ── validation ───────────────────────────────────────────────────────────────────────────────

/// Everything the layout must be true of before anyone looks at the world. Returns every problem,
/// not the first: a caller that fixes one at a time learns less.
pub fn validate(l: &Layout) -> Vec<String> {
    let mut e = Vec::new();

    if l.participants.len() != 2 {
        e.push(format!("expected 2 participants, found {}", l.participants.len()));
        return e;
    }
    if l.activation != "dry_run_only" {
        e.push(format!("activation is '{}', not dry_run_only", l.activation));
    }
    if l.lan.egress != "deny" || l.lan.dns != "deny" {
        e.push("lan egress/dns must both be deny".into());
    }
    if l.lan.default_route != "none" || l.lan.ip_forward != 0 {
        e.push("lan must declare no default route and ip_forward=0".into());
    }

    // ── identity: nothing shared, at any layer ──
    let a = &l.participants[0];
    let b = &l.participants[1];
    let identity_axes: [(&str, &str, &str); 8] = [
        ("instance_id", &a.instance_id, &b.instance_id),
        ("participant_id", &a.participant_id, &b.participant_id),
        ("np.npid", &a.np.npid, &b.np.npid),
        ("d1.account_soid", &a.d1.account_soid, &b.d1.account_soid),
        ("d1.character_soid", &a.d1.character_soid, &b.d1.character_soid),
        ("d1.inv_503_raw", &a.d1.inv_503_raw, &b.d1.inv_503_raw),
        ("addr", &a.addr, &b.addr),
        ("netns", &a.netns, &b.netns),
    ];
    for (name, x, y) in identity_axes {
        if x == y {
            e.push(format!("identity conflation: both participants share {name} = {x}"));
        }
    }
    if a.np.responder_account_id == b.np.responder_account_id {
        e.push("identity conflation: both participants share np.responder_account_id".into());
    }
    for p in &l.participants {
        if p.np.npid.chars().count() > NP_ONLINE_ID_MAX {
            e.push(format!(
                "npid '{}' exceeds the {NP_ONLINE_ID_MAX}-character NP online-ID limit",
                p.np.npid
            ));
        }
        if p.d1.account_soid == p.d1.character_soid {
            e.push(format!(
                "participant {}: account and character SOID are the same value",
                p.slot
            ));
        }
    }
    // The known example SOID belongs to participant A alone (six-player-fixture rule, applied to
    // the one participant that is *not* the anchored one).
    if b.d1.account_soid == format!("0x{ANCHOR_ACCOUNT_SOID:016X}")
        || b.d1.character_soid == format!("0x{ANCHOR_CHARACTER_SOID:016X}")
    {
        e.push("participant B carries the anchored known-example SOID".into());
    }

    // ── endpoints ──
    let mut lan_seen: BTreeMap<String, String> = BTreeMap::new();
    let mut host_ports: BTreeMap<u16, String> = BTreeMap::new();
    let mut ns_local: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let all: Vec<(&str, &Endpoint)> = l
        .participants
        .iter()
        .flat_map(|p| p.endpoints.iter().map(move |x| (p.slot.as_str(), x)))
        .chain(l.shared_services.iter().map(|s| ("shared", &s.endpoint)))
        .collect();
    for (slot, ep) in all {
        match ep.scope {
            Scope::Lan => {
                if let Some(prev) = lan_seen.insert(ep.key(), slot.to_string()) {
                    e.push(format!(
                        "endpoint collision on the LAN: {} claimed by both {prev} and {slot}",
                        ep.key()
                    ));
                }
            }
            Scope::Host => {
                if let Some(prev) = host_ports.insert(ep.port, slot.to_string()) {
                    e.push(format!(
                        "host port collision: {} claimed by both {prev} and {slot}",
                        ep.port
                    ));
                }
            }
            Scope::NamespaceLocal => {
                ns_local
                    .entry(format!("{}/{}", ep.proto, ep.port))
                    .or_default()
                    .push(slot.to_string());
            }
        }
    }
    for (key, slots) in &ns_local {
        if slots.len() > 1 && !l.lan.netns_separation {
            e.push(format!(
                "namespace-local port {key} is claimed by {slots:?} but lan.netns_separation is false"
            ));
        }
    }

    // ── paths: nothing mutable shared ──
    let mut path_owner: BTreeMap<&str, &str> = BTreeMap::new();
    for p in &l.participants {
        for (k, v) in &p.paths {
            if let Some(prev) = path_owner.insert(v.as_str(), p.slot.as_str()) {
                e.push(format!("mutable path {v} ({k}) shared by {prev} and {}", p.slot));
            }
        }
        if !p.xdg_data_home.starts_with(&format!("{}/{}", l.base_dir, p.slot)) {
            e.push(format!(
                "participant {}: XDG_DATA_HOME {} is outside its own root",
                p.slot, p.xdg_data_home
            ));
        }
        if p.controller.fifo != p.env["PAD_FIFO"] {
            e.push(format!("participant {}: controller fifo disagrees with PAD_FIFO", p.slot));
        }
        for (k, v) in &p.env {
            if v.contains("${") || v.contains("$(") {
                e.push(format!(
                    "participant {}: env {k} carries an unresolved shell expansion",
                    p.slot
                ));
            }
        }
        if p.env.get("D1_MANIFEST_FILE").map(String::is_empty).unwrap_or(true) {
            e.push(format!(
                "participant {}: D1_MANIFEST_FILE is unset - a 2-byte manifest halts the client in \
                 bap_signin and reads as a regression in whatever else is under test",
                p.slot
            ));
        }
        // A 503 body that is inert is worse than an absent one: it looks configured.
        if p.env.get("D1_503_SOID").map(String::as_str) == Some("0") {
            e.push(format!(
                "participant {}: D1_503_SOID=0 makes D1_503_RAW inert (the raw block is nested \
                 inside the SOID check in d1-responder)",
                p.slot
            ));
        }
    }
    if a.controller.fifo == b.controller.fifo {
        e.push("both participants share one controller FIFO".into());
    }
    if a.controller.isolation == ControllerIsolation::DistinctUsbIds
        && a.controller.usb_vendor == b.controller.usb_vendor
        && a.controller.usb_product == b.controller.usb_product
    {
        e.push(
            "controller isolation claims distinct USB ids while both pads carry the same VID/PID"
                .into(),
        );
    }

    // ── the 503 splice must actually carry the SOID it claims ──
    for p in &l.participants {
        let Some(bytes) = hex_to_bytes(&p.d1.inv_503_raw) else {
            e.push(format!("participant {}: D1_503_RAW is not valid hex", p.slot));
            continue;
        };
        let want = u64::from_str_radix(p.d1.account_soid.trim_start_matches("0x"), 16).unwrap_or(0);
        match read_bits(&bytes, INV_503_ACCOUNT_SOID_BIT_OFFSET, 64) {
            Some(got) if got == want => {}
            other => e.push(format!(
                "participant {}: D1_503_RAW carries {other:?} at bit {INV_503_ACCOUNT_SOID_BIT_OFFSET}, \
                 not its own account SOID {want:#018x}",
                p.slot
            )),
        }
    }

    e
}

/// Canonical JSON. Byte-identical for a given seed - that is the determinism control's whole axis.
pub fn to_json(l: &Layout) -> String {
    serde_json::to_string_pretty(l).expect("layout serialises")
}

#[cfg(test)]
mod ten_flag_template_tests {
    use super::*;

    /// ⭐⭐ **The third copy of the 503 body, pinned to the one the responder serves.**
    ///
    /// `d1-harness` exports [`INV_503_TEMPLATE_HEX`] as `D1_503_RAW`, and **env beats default**. So
    /// a stale template here does not merely go unnoticed - it *overrides* the ten-flag body the
    /// responder now serves, and it does so only in multi-participant runs. That failure would read
    /// as "rung 33 refuses in six-player runs but not solo", which points at the session layer and
    /// not at a constant.
    ///
    /// ⚠ The control axis is byte identity with `d1_bap::INVESTMENT_503_UNLOCKS_RAW`, not "does it
    /// contain flags" - a template carrying the superseded union-8 set would pass the weaker test
    /// and still be missing the two indices (2667, 5987) that rung 33's permissions wall needs.
    #[test]
    fn template_is_the_body_the_responder_actually_serves() {
        let template = hex_to_bytes(INV_503_TEMPLATE_HEX).expect("template is valid hex");
        assert_eq!(
            template.as_slice(),
            d1_bap::INVESTMENT_503_UNLOCKS_RAW,
            "INV_503_TEMPLATE_HEX has drifted from the body d1-responder serves by default"
        );
        // The splice offset must still be the SOID's, and still unique, in the longer body.
        assert_eq!(
            find_unique_bit_run(&template, ANCHOR_ACCOUNT_SOID, 64),
            Some(INV_503_ACCOUNT_SOID_BIT_OFFSET),
            "the account SOID must occur exactly once - an ambiguous splice hands B the A account"
        );
        // …and the flags must survive a splice for a different participant.
        let spliced =
            splice_account_soid(&template, INV_503_ACCOUNT_SOID_BIT_OFFSET, 0xD100_0000_0000_0002);
        assert_eq!(
            d1_bap::unlock_flags_in_shared_tail(&spliced[21..]),
            Some(d1_bap::UNLOCK_FLAG_INDICES.iter().map(|&i| (i, 2u8)).collect::<Vec<_>>()),
            "participant B must get the same ten flags, not just the same SOID machinery"
        );
    }
}
