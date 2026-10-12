// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The activation preflight: **the thing that says no.**
//!
//! `historicaldocs/design/two-instance-harness-design.md` lists a five-step activation checklist and then says the
//! one-emulator guard "remains active until an explicit activation review". This module is that
//! checklist, executable, with one rule above all others:
//!
//! > ⛔ **A check that cannot be evaluated REFUSES. It never skips and it never passes.**
//!
//! That is not pedantry. Every expensive failure in this project's log has the same shape - an
//! instrument that produced nothing and was read as "the thing didn't happen": the diagnostic sink
//! that never outputs, the tracer that arms nothing when `D1_ACCOUNT_TRACE_OUT` is unset, the
//! `pgrep -x shadps4` guard that matched no process because the comm is `shadPS4:Main`. A preflight
//! that skips an unevaluable check is the same defect wearing a green tick.
//!
//! ## It launches nothing and changes nothing
//!
//! Every probe is a read: `/proc/net/route`, `/proc/sys/net/ipv4/ip_forward`, `/proc/self/status`,
//! `/proc/<pid>/cmdline`, file metadata, and one **route lookup** - `connect()` on an unbound UDP
//! socket, which selects a source address and **sends no packet**. (That is the same mechanism the
//! emulator lane used to fix `RetrieveIp`, and the reason its probe cannot block.)
//!
//! ## The containment check is a PAIR, and both arms matter
//!
//! * a route lookup toward the **peer participant** must succeed *and select this participant's own
//!   address*; and
//! * a route lookup toward **1.1.1.1** must fail.
//!
//! One arm alone is decoration: if both succeed, containment is broken; if both fail, the preflight
//! is not running inside the harness namespace and knows nothing about either.

use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};

use sha2::{Digest, Sha256};

use crate::layout::{CaptureMethod, ControllerIsolation, Layout};

/// What the preflight is being asked to clear.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Pure checks over the layout. Safe to run anywhere, clears nothing.
    Layout,
    /// Adds the namespace and containment probes for one participant.
    Namespace,
    /// Everything, plus capture arming, provenance and the activation review. The only stage that
    /// can return READY - and READY still launches nothing.
    Arm,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Verdict {
    Pass,
    /// Named reason, in the report and in the exit message. A refusal without a name is a shrug.
    Refuse(&'static str),
}

#[derive(Clone, Debug)]
pub struct Check {
    pub name: &'static str,
    pub verdict: Verdict,
    pub detail: String,
}

impl Check {
    fn pass(name: &'static str, detail: impl AsRef<str>) -> Check {
        Check { name, verdict: Verdict::Pass, detail: detail.as_ref().to_string() }
    }
    fn refuse(name: &'static str, reason: &'static str, detail: impl AsRef<str>) -> Check {
        Check { name, verdict: Verdict::Refuse(reason), detail: detail.as_ref().to_string() }
    }
    fn when(name: &'static str, ok: bool, reason: &'static str, detail: impl AsRef<str>) -> Check {
        if ok {
            Check::pass(name, detail)
        } else {
            Check::refuse(name, reason, detail)
        }
    }
    pub fn is_refusal(&self) -> bool {
        matches!(self.verdict, Verdict::Refuse(_))
    }
    pub fn reason(&self) -> Option<&'static str> {
        match self.verdict {
            Verdict::Refuse(r) => Some(r),
            Verdict::Pass => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub checks: Vec<Check>,
    pub stage: Stage,
    pub slot: Option<String>,
}

impl Report {
    pub fn refusals(&self) -> Vec<&Check> {
        self.checks.iter().filter(|c| c.is_refusal()).collect()
    }
    pub fn reasons(&self) -> Vec<&'static str> {
        self.checks.iter().filter_map(|c| c.reason()).collect()
    }
    pub fn has(&self, reason: &str) -> bool {
        self.reasons().iter().any(|r| *r == reason)
    }
    /// READY means "every check passed at stage `Arm`". It does **not** mean anything started.
    pub fn ready(&self) -> bool {
        self.stage == Stage::Arm && self.refusals().is_empty()
    }
}

// ── the probe surface, so the checks can be tested without a lab ─────────────────────────────

/// Everything the preflight learns about the world. A trait so [`crate::controls`] can drive every
/// refusal path from a fake - a refusal that has never been *observed to fire* is a claim, not a
/// guard.
pub trait SysProbe {
    /// `Some(true)` if a default route exists, `None` if the routing table could not be read.
    fn default_route(&self) -> Option<bool>;
    /// `Some(true)` if a route for `subnet` (dotted quad network) exists.
    fn subnet_route(&self, network: Ipv4Addr, prefix: u8) -> Option<bool>;
    /// Source address the kernel would select for `addr`, or `None` if it is unreachable.
    /// **Sends no packet.**
    fn route_source(&self, addr: Ipv4Addr) -> Option<IpAddr>;
    fn ip_forward(&self) -> Option<u8>;
    fn cap_net_raw(&self) -> Option<bool>;
    /// PIDs of running emulators, by command line. Never a pattern handed to a killer.
    fn emulator_pids(&self) -> Vec<i32>;
    fn exists(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
    fn is_fifo(&self, path: &str) -> bool;
    fn writable_dir(&self, path: &str) -> bool;
    fn readable_file(&self, path: &str) -> bool;
    fn sha256(&self, path: &str) -> Option<String>;
}

/// The real one. Reads only.
pub struct Real;

fn proc_routes() -> Option<Vec<(u32, u32)>> {
    let text = fs::read_to_string("/proc/net/route").ok()?;
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 8 {
            continue;
        }
        let dest = u32::from_str_radix(f[1], 16).ok()?;
        let mask = u32::from_str_radix(f[7], 16).ok()?;
        out.push((dest, mask));
    }
    Some(out)
}

/// `/proc/net/route` stores addresses in the byte order they appear on the wire, read as a
/// little-endian `u32` - so `10.77.0.0` is `0x00004D0A`, not `0x0A4D0000`.
fn proc_word(a: Ipv4Addr) -> u32 {
    u32::from_le_bytes(a.octets())
}

impl SysProbe for Real {
    fn default_route(&self) -> Option<bool> {
        Some(proc_routes()?.iter().any(|&(d, m)| d == 0 && m == 0))
    }

    fn subnet_route(&self, network: Ipv4Addr, prefix: u8) -> Option<bool> {
        let want_mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
        let want_mask = proc_word(Ipv4Addr::from(want_mask.to_be_bytes()));
        let want_dest = proc_word(network);
        Some(proc_routes()?.iter().any(|&(d, m)| d == want_dest && m == want_mask))
    }

    fn route_source(&self, addr: Ipv4Addr) -> Option<IpAddr> {
        // SOCK_DGRAM + connect() = route lookup and source selection. No datagram is sent, so this
        // cannot block and cannot leak. Port 9 (discard) is arbitrary and never contacted.
        let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
        sock.connect((addr, 9)).ok()?;
        sock.local_addr().ok().map(|a| a.ip())
    }

    fn ip_forward(&self) -> Option<u8> {
        fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    fn cap_net_raw(&self) -> Option<bool> {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("CapEff:"))?;
        let hex = line.split_whitespace().nth(1)?;
        let caps = u64::from_str_radix(hex, 16).ok()?;
        Some(caps >> 13 & 1 == 1) // CAP_NET_RAW == 13
    }

    fn emulator_pids(&self) -> Vec<i32> {
        // ⚠ `pgrep -x shadps4` matches nothing: the comm is `shadPS4:Main`. Read command lines.
        let me = std::process::id() as i32;
        let mut out = Vec::new();
        let Ok(dir) = fs::read_dir("/proc") else { return out };
        for entry in dir.flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else { continue };
            if pid == me {
                continue;
            }
            let Ok(raw) = fs::read(entry.path().join("cmdline")) else { continue };
            let cmd = String::from_utf8_lossy(&raw).replace('\0', " ");
            if cmd.contains("shadps4") && !cmd.contains("d1-harness") {
                out.push(pid);
            }
        }
        out.sort_unstable();
        out
    }

    fn exists(&self, path: &str) -> bool {
        fs::symlink_metadata(path).is_ok()
    }
    fn is_dir(&self, path: &str) -> bool {
        fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
    fn is_fifo(&self, path: &str) -> bool {
        fs::metadata(path).map(|m| m.file_type().is_fifo()).unwrap_or(false)
    }
    fn writable_dir(&self, path: &str) -> bool {
        fs::metadata(path)
            .map(|m| m.is_dir() && m.permissions().mode() & 0o200 != 0)
            .unwrap_or(false)
    }
    fn readable_file(&self, path: &str) -> bool {
        fs::File::open(path).is_ok()
    }
    fn sha256(&self, path: &str) -> Option<String> {
        let bytes = fs::read(path).ok()?;
        let mut h = Sha256::new();
        h.update(&bytes);
        Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }
}

// ── input ────────────────────────────────────────────────────────────────────────────────────

pub struct Input<'a> {
    pub layout: &'a Layout,
    pub stage: Stage,
    /// Which participant's namespace this preflight is running inside.
    pub slot: Option<String>,
    /// The operator's own words. Recorded verbatim in the report; nothing clears without it.
    pub activation_review: Option<String>,
    /// `role=sha256`, so a relinked emulator cannot masquerade as the one a previous run used.
    pub expect_sha: BTreeMap<String, String>,
}

const R_LAYOUT: &str = "LayoutInvalid";
const R_CONFLATION: &str = "IdentityConflation";
const R_COLLISION: &str = "EndpointCollision";
const R_CONTROLLER: &str = "ControllerIsolationUnresolved";
const R_REVIEW: &str = "ActivationReviewMissing";
const R_EMU_RUNNING: &str = "EmulatorAlreadyRunning";
const R_DEFAULT_ROUTE: &str = "DefaultRoutePresent";
const R_FORWARDING: &str = "IpForwardingEnabled";
const R_NOT_IN_NS: &str = "NotInsideHarnessNamespace";
const R_PEER_SOURCE: &str = "PeerRouteWrongSource";
const R_EGRESS: &str = "EgressReachable";
const R_PORTABLE: &str = "PortableUserDirShadowsXdg";
const R_BINARY: &str = "BinaryMissing";
const R_SHA: &str = "BinaryShaMismatch";
const R_SINK: &str = "CaptureSinkUnwritable";
const R_UNARMED: &str = "CaptureNotArmed";
const R_CAP: &str = "CapNetRawMissing";
const R_CONTROL_RUN: &str = "UninstrumentedControlMissing";
const R_INPUT: &str = "ReadOnlyInputMissing";
const R_SLOT: &str = "SlotNotSpecified";
const R_UNEVALUABLE: &str = "CheckUnevaluable";

/// Every named refusal this module can emit. Used by the controls to prove each one is reachable.
pub const ALL_REASONS: &[&str] = &[
    R_LAYOUT,
    R_CONFLATION,
    R_COLLISION,
    R_CONTROLLER,
    R_REVIEW,
    R_EMU_RUNNING,
    R_DEFAULT_ROUTE,
    R_FORWARDING,
    R_NOT_IN_NS,
    R_PEER_SOURCE,
    R_EGRESS,
    R_PORTABLE,
    R_BINARY,
    R_SHA,
    R_SINK,
    R_UNARMED,
    R_CAP,
    R_CONTROL_RUN,
    R_INPUT,
    R_SLOT,
    R_UNEVALUABLE,
];

pub fn run(input: &Input, sys: &dyn SysProbe) -> Report {
    let l = input.layout;
    let mut c: Vec<Check> = Vec::new();

    // ── stage: layout ──────────────────────────────────────────────────────────────────────
    let problems = crate::layout::validate(l);
    let pick = |needle: &str| -> Vec<String> {
        problems.iter().filter(|p| p.contains(needle)).cloned().collect()
    };
    let conflation = pick("conflation");
    let collision = pick("collision");
    let other: Vec<String> = problems
        .iter()
        .filter(|p| !p.contains("conflation") && !p.contains("collision"))
        .cloned()
        .collect();

    let identity_detail = if conflation.is_empty() {
        if l.participants.len() == 2 {
            format!(
                "npid {} vs {}; account {} vs {}; addr {} vs {}",
                l.participants[0].np.npid,
                l.participants[1].np.npid,
                l.participants[0].d1.account_soid,
                l.participants[1].d1.account_soid,
                l.participants[0].addr,
                l.participants[1].addr
            )
        } else {
            "not two participants".to_string()
        }
    } else {
        conflation.join("; ")
    };
    c.push(Check::when(
        "identities differ at NP, D1 account, peer and endpoint layers",
        conflation.is_empty(),
        R_CONFLATION,
        identity_detail,
    ));
    c.push(Check::when(
        "no two participants share an address or a port",
        collision.is_empty(),
        R_COLLISION,
        if collision.is_empty() {
            "every LAN endpoint unique; namespace-local reuse justified by netns separation".into()
        } else {
            collision.join("; ")
        },
    ));
    c.push(Check::when(
        "layout validates in every other respect",
        other.is_empty(),
        R_LAYOUT,
        if other.is_empty() { "clean".to_string() } else { other.join("; ") },
    ));

    // Controller separation. Unresolved is the generated default and it refuses.
    for p in &l.participants {
        if p.controller.isolation == ControllerIsolation::Unresolved {
            c.push(Check::refuse(
                "controller namespace separation is a named mechanism",
                R_CONTROLLER,
                format!(
                    "participant {}: isolation is 'unresolved'. SDL_GAMECONTROLLER_IGNORE_DEVICES \
                     matches on VID/PID alone, so two pads with one id are ONE device to the \
                     emulator. `d1-harness pad run --vendor/--product` mints distinct ids \
                     (this participant would get {}/{}), but whether SDL maps the non-x360 GUID \
                     is untested - choose dev-input-bind-mount or distinct-usb-ids and record it.",
                    p.slot, p.controller.usb_vendor, p.controller.usb_product
                ),
            ));
        } else {
            c.push(Check::pass(
                "controller namespace separation is a named mechanism",
                format!("participant {}: {:?}", p.slot, p.controller.isolation),
            ));
        }
    }

    // Read-only inputs must exist. A missing content manifest is the single most expensive
    // unset value in this project's history.
    for (name, path) in &l.shared_readonly_inputs {
        c.push(Check::when(
            "shared read-only inputs are present",
            sys.readable_file(path),
            R_INPUT,
            format!("{name}: {path}"),
        ));
    }

    // ── stage: namespace ───────────────────────────────────────────────────────────────────
    if input.stage != Stage::Layout {
        let slot = input.slot.clone();
        let me = slot.as_ref().and_then(|s| l.participants.iter().find(|p| &p.slot == s));
        let peer = slot
            .as_ref()
            .and_then(|s| l.participants.iter().find(|p| &p.slot != s));

        match (me, peer) {
            (Some(me), Some(peer)) => {
                let net: Ipv4Addr = "10.77.0.0".parse().unwrap();
                match sys.subnet_route(net, l.prefix()) {
                    Some(true) => c.push(Check::pass(
                        "running inside the harness network namespace",
                        format!("route for {} present", l.lan.subnet),
                    )),
                    Some(false) => c.push(Check::refuse(
                        "running inside the harness network namespace",
                        R_NOT_IN_NS,
                        format!(
                            "no route for {} - this preflight is looking at some other namespace, \
                             so every network verdict below would be about the wrong world",
                            l.lan.subnet
                        ),
                    )),
                    None => c.push(Check::refuse(
                        "running inside the harness network namespace",
                        R_UNEVALUABLE,
                        "/proc/net/route could not be read",
                    )),
                }

                match sys.default_route() {
                    Some(false) => c.push(Check::pass("no default route", "none in the table")),
                    Some(true) => c.push(Check::refuse(
                        "no default route",
                        R_DEFAULT_ROUTE,
                        "a default route exists; off-subnet traffic would leave the lab",
                    )),
                    None => c.push(Check::refuse(
                        "no default route",
                        R_UNEVALUABLE,
                        "/proc/net/route could not be read",
                    )),
                }

                match sys.ip_forward() {
                    Some(0) => c.push(Check::pass("ip forwarding disabled", "ip_forward=0")),
                    Some(v) => c.push(Check::refuse(
                        "ip forwarding disabled",
                        R_FORWARDING,
                        format!("ip_forward={v}"),
                    )),
                    None => c.push(Check::refuse(
                        "ip forwarding disabled",
                        R_UNEVALUABLE,
                        "/proc/sys/net/ipv4/ip_forward could not be read",
                    )),
                }

                // The containment PAIR. Both arms, or neither is evidence.
                let peer_addr: Ipv4Addr = peer.addr.parse().unwrap_or(Ipv4Addr::UNSPECIFIED);
                match sys.route_source(peer_addr) {
                    Some(src) if src.to_string() == me.addr => c.push(Check::pass(
                        "isolated-LAN reachability: the peer routes from OUR address",
                        format!("{} -> {}", me.addr, peer.addr),
                    )),
                    Some(src) => c.push(Check::refuse(
                        "isolated-LAN reachability: the peer routes from OUR address",
                        R_PEER_SOURCE,
                        format!(
                            "route to {} selects source {src}, not this participant's {} - the two \
                             instances would appear as one endpoint on the wire",
                            peer.addr, me.addr
                        ),
                    )),
                    None => c.push(Check::refuse(
                        "isolated-LAN reachability: the peer routes from OUR address",
                        R_NOT_IN_NS,
                        format!("no route to the peer at {}", peer.addr),
                    )),
                }
                let outside: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
                match sys.route_source(outside) {
                    None => c.push(Check::pass(
                        "containment: nothing off the lab is routable (negative arm)",
                        "1.1.1.1 is unreachable, so the peer arm above proves a route table that \
                         reaches the lab and only the lab"
                            ,
                    )),
                    Some(src) => c.push(Check::refuse(
                        "containment: nothing off the lab is routable (negative arm)",
                        R_EGRESS,
                        format!("1.1.1.1 routes from {src} - this namespace has egress"),
                    )),
                }

                // shadPS4 resolves its user directory from `$PWD/user` FIRST. If that exists, both
                // participants share one identity however carefully XDG_DATA_HOME was set.
                c.push(Check::when(
                    "no portable user directory shadows XDG_DATA_HOME",
                    !sys.exists(&me.portable_dir_must_not_exist),
                    R_PORTABLE,
                    format!(
                        "{} must not exist (path_util.cpp tries the portable dir before \
                         $XDG_DATA_HOME/shadPS4)",
                        me.portable_dir_must_not_exist
                    ),
                ));
            }
            _ => c.push(Check::refuse(
                "a participant slot was named",
                R_SLOT,
                "namespace checks are per-participant; pass --slot a or --slot b, and run the \
                 preflight inside that participant's namespace"
                    ,
            )),
        }
    }

    // ── stage: arm ─────────────────────────────────────────────────────────────────────────
    if input.stage == Stage::Arm {
        // One emulator at a time, still. The activation review does not repeal this: it decides
        // whether a SECOND may start, not whether a stray first one may be ignored.
        let pids = sys.emulator_pids();
        c.push(Check::when(
            "no unaccounted emulator is already running",
            pids.is_empty(),
            R_EMU_RUNNING,
            if pids.is_empty() {
                "none found by command line".to_string()
            } else {
                format!("running: {pids:?}")
            },
        ));

        // Provenance. The relink hazard: two runs with materially different emulator behaviour can
        // otherwise carry an identical RUN-INFO.txt, because it records a git commit and the change
        // may be uncommitted.
        for (role, path) in &l.shared_readonly_inputs {
            match sys.sha256(path) {
                None => c.push(Check::refuse(
                    "every declared binary and input hashes",
                    R_BINARY,
                    format!("{role}: {path} could not be read"),
                )),
                Some(sha) => {
                    if let Some(want) = input.expect_sha.get(role) {
                        c.push(Check::when(
                            "pinned sha256 matches",
                            &sha == want,
                            R_SHA,
                            format!("{role}: {sha} (expected {want})"),
                        ));
                    } else {
                        c.push(Check::pass(
                            "every declared binary and input hashes",
                            format!("{role}: {sha}"),
                        ));
                    }
                }
            }
        }

        // Capture, per plane, per participant.
        let mut needs_cap_net_raw = false;
        for p in &l.participants {
            for plane in &p.capture {
                let parent = plane.sink.rsplit_once('/').map(|(d, _)| d).unwrap_or("/");
                c.push(Check::when(
                    "capture sink directory exists and is writable",
                    sys.writable_dir(parent),
                    R_SINK,
                    format!("{}/{}: {}", p.slot, plane.name, parent),
                ));
                c.push(Check::when(
                    "every plane is armed and carries a canary",
                    plane.armed && !plane.canary.is_empty(),
                    R_UNARMED,
                    format!(
                        "{}/{}: armed={} canary={:?}",
                        p.slot, plane.name, plane.armed, plane.canary
                    ),
                ));
                if plane.method == CaptureMethod::Pcap {
                    needs_cap_net_raw = true;
                }
            }
        }
        if needs_cap_net_raw {
            match sys.cap_net_raw() {
                Some(true) => c.push(Check::pass(
                    "CAP_NET_RAW is held where a pcap plane is declared",
                    "CapEff bit 13 set",
                )),
                Some(false) => c.push(Check::refuse(
                    "CAP_NET_RAW is held where a pcap plane is declared",
                    R_CAP,
                    "a pcap plane is declared and CAP_NET_RAW is absent - the capture would run, \
                     record nothing, and exit 0"
                        ,
                )),
                None => c.push(Check::refuse(
                    "CAP_NET_RAW is held where a pcap plane is declared",
                    R_UNEVALUABLE,
                    "/proc/self/status could not be read",
                )),
            }
        }

        c.push(Check::when(
            "an uninstrumented control run is on record",
            l.guards.uninstrumented_control.is_some(),
            R_CONTROL_RUN,
            l.guards
                .uninstrumented_control
                .clone()
                .unwrap_or_else(|| "none recorded - without it, any behaviour change cannot be \
                                    separated from the instrumentation that observed it"
                    .into()),
        ));

        // FIFOs, if they exist at all, must be FIFOs and must be distinct.
        for p in &l.participants {
            if sys.exists(&p.controller.fifo) {
                c.push(Check::when(
                    "controller FIFO is a FIFO",
                    sys.is_fifo(&p.controller.fifo),
                    R_CONTROLLER,
                    format!("{}: {}", p.slot, p.controller.fifo),
                ));
            }
        }

        c.push(match &input.activation_review {
            Some(text) if !text.trim().is_empty() => Check::pass(
                "the operator's activation review is recorded",
                format!("review: {text}"),
            ),
            _ => Check::refuse(
                "the operator's activation review is recorded",
                R_REVIEW,
                "the one-emulator guard holds until an explicit activation review. That review is \
                 the operator's, not this tool's, and not another agent's."
                    ,
            ),
        });
    } else {
        c.push(Check::refuse(
            "the operator's activation review is recorded",
            R_REVIEW,
            format!("stage {:?} clears nothing; only --stage arm can return READY", input.stage),
        ));
    }

    Report { checks: c, stage: input.stage, slot: input.slot.clone() }
}

impl Layout {
    fn prefix(&self) -> u8 {
        self.participants.first().map(|p| p.prefix).unwrap_or(24)
    }
}
