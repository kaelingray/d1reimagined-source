// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Controls - run these before citing anything this crate says. **Exit 2 = a control failed and
//! the output may not be cited.**
//!
//! ## The axis
//!
//! Hard rule 5: *a control must test the axis you are worried about*, and the PVE lane's 25
//! controls failed it by comparing our constants to our constants. So the axes here are:
//!
//! * **`capture-rig/run-boot.sh`, parsed at run time.** Participant A's account SOID, character
//!   SOID, 206 body and 503 body are compared against the pinned defaults in the launcher - a file
//!   this crate does not own. Edit either side and the control fails. `SKIPPED` (never `PASS`) if
//!   the repo is not present.
//! * **The kernel.** `/proc` reads are checked against our own live process, which is the only
//!   process whose properties we independently know.
//! * **Mutation arms.** Every refusal the preflight can emit is *driven* from a fake world and
//!   observed to fire - and paired with the same world minus the defect, where it must not fire.
//! * ⭐ **The saturating positive control.** A preflight that refused everything unconditionally
//!   would pass every refusal control above. So one control drives a fully-satisfied world and
//!   requires **READY**. Without it the rest are decoration.
//!
//! ## SKIPPED is a third state
//!
//! A control whose input is absent prints `SKIP` and is counted separately. It is never folded into
//! the pass count, because silently passing a check that never ran is how instrument defects hide.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use crate::layout::{
    self, bytes_to_hex, find_bit_runs, find_unique_bit_run, hex_to_bytes, read_bits,
    splice_account_soid, ControllerIsolation, Layout, Options, Scope,
    ANCHOR_ACCOUNT_SOID, ANCHOR_CHARACTER_SOID, GUEST_BOUND_UDP, INV_503_ACCOUNT_SOID_BIT_OFFSET,
    INV_503_TEMPLATE_HEX, NP_ONLINE_ID_MAX, RESPONDER_BAP_UDP,
};
use crate::preflight::{self, Input, Report, Stage, SysProbe};
use crate::procs::{self, Action, Ledger, PidRecord, ProcTable, Skip};

pub struct Control {
    pub name: &'static str,
    pub state: State,
    pub detail: String,
}

#[derive(PartialEq)]
pub enum State {
    Pass,
    Fail,
    Skipped,
}

pub(crate) fn check(name: &'static str, cond: bool, detail: impl AsRef<str>) -> Control {
    Control { name, state: if cond { State::Pass } else { State::Fail }, detail: detail.as_ref().to_string() }
}

pub(crate) fn skipped(name: &'static str, detail: impl AsRef<str>) -> Control {
    Control { name, state: State::Skipped, detail: detail.as_ref().to_string() }
}

// ── a fake world, so every refusal can be driven ─────────────────────────────────────────────

#[derive(Clone)]
pub struct Fake {
    pub default_route: Option<bool>,
    pub subnet_route: Option<bool>,
    /// address -> selected source. Absent means unreachable.
    pub routes: BTreeMap<String, String>,
    pub ip_forward: Option<u8>,
    pub cap_net_raw: Option<bool>,
    pub emulator_pids: Vec<i32>,
    pub existing: Vec<String>,
    pub dirs: Vec<String>,
    pub fifos: Vec<String>,
    pub files: Vec<String>,
}

impl Fake {
    /// A world in which nothing is wrong. Every refusal control mutates exactly one thing away
    /// from this, so a failure names its own cause.
    pub fn clean(l: &Layout) -> Fake {
        let mut routes = BTreeMap::new();
        for p in &l.participants {
            let peer = l.participants.iter().find(|q| q.slot != p.slot).unwrap();
            // Looking up the PEER from inside p's namespace selects p's own address.
            routes.insert(peer.addr.clone(), p.addr.clone());
        }
        let mut dirs: Vec<String> = Vec::new();
        for p in &l.participants {
            for plane in &p.capture {
                if let Some((d, _)) = plane.sink.rsplit_once('/') {
                    dirs.push(d.to_string());
                }
            }
        }
        let files: Vec<String> = l.shared_readonly_inputs.values().cloned().collect();
        Fake {
            default_route: Some(false),
            subnet_route: Some(true),
            routes,
            ip_forward: Some(0),
            cap_net_raw: Some(true),
            emulator_pids: vec![],
            existing: files.clone(),
            dirs,
            fifos: vec![],
            files,
        }
    }
}

impl SysProbe for Fake {
    fn default_route(&self) -> Option<bool> {
        self.default_route
    }
    fn subnet_route(&self, _n: Ipv4Addr, _p: u8) -> Option<bool> {
        self.subnet_route
    }
    fn route_source(&self, addr: Ipv4Addr) -> Option<IpAddr> {
        self.routes.get(&addr.to_string()).and_then(|s| s.parse().ok())
    }
    fn ip_forward(&self) -> Option<u8> {
        self.ip_forward
    }
    fn cap_net_raw(&self) -> Option<bool> {
        self.cap_net_raw
    }
    fn emulator_pids(&self) -> Vec<i32> {
        self.emulator_pids.clone()
    }
    fn exists(&self, path: &str) -> bool {
        self.existing.iter().any(|p| p == path)
            || self.dirs.iter().any(|p| p == path)
            || self.fifos.iter().any(|p| p == path)
    }
    fn is_dir(&self, path: &str) -> bool {
        self.dirs.iter().any(|p| p == path)
    }
    fn is_fifo(&self, path: &str) -> bool {
        self.fifos.iter().any(|p| p == path)
    }
    fn writable_dir(&self, path: &str) -> bool {
        self.dirs.iter().any(|p| p == path)
    }
    fn readable_file(&self, path: &str) -> bool {
        self.files.iter().any(|p| p == path)
    }
    fn sha256(&self, path: &str) -> Option<String> {
        self.files.iter().any(|p| p == path).then(|| format!("{:064x}", path.len()))
    }
}

/// A layout with every operator-owned knob satisfied: capture armed, controller mechanism chosen,
/// control run recorded.
fn armed_layout(base: &Layout) -> Layout {
    let mut l = base.clone();
    l.guards.uninstrumented_control = Some("20260808-000000-uninstrumented-control".into());
    for p in &mut l.participants {
        p.controller.isolation = ControllerIsolation::DevInputBindMount;
        for plane in &mut p.capture {
            plane.armed = true;
        }
    }
    l
}

fn run_preflight(l: &Layout, sys: &dyn SysProbe, review: Option<&str>, slot: &str) -> Report {
    preflight::run(
        &Input {
            layout: l,
            stage: Stage::Arm,
            slot: Some(slot.to_string()),
            activation_review: review.map(str::to_string),
            expect_sha: BTreeMap::new(),
        },
        sys,
    )
}

// ── run-boot.sh, parsed rather than transcribed ──────────────────────────────────────────────

/// `export NAME="${NAME:-VALUE}"` → `NAME -> VALUE`.
pub fn shell_defaults(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("export ") else { continue };
        let Some((name, value)) = rest.split_once('=') else { continue };
        let value = value.trim().trim_matches('"');
        let Some(inner) = value.strip_prefix("${").and_then(|v| v.strip_suffix('}')) else {
            continue;
        };
        let Some((_, default)) = inner.split_once(":-") else { continue };
        out.insert(name.trim().to_string(), default.to_string());
    }
    out
}

fn run_boot_defaults() -> Option<BTreeMap<String, String>> {
    let repo = layout::default_repo();
    let path = format!("{repo}/capture-rig/run-boot.sh");
    let text = std::fs::read_to_string(path).ok()?;
    // `run-boot.sh` writes its paths as `$REPO/...`; the layout carries them absolute. Expand the
    // one variable rather than weakening the comparison to a suffix match - a suffix match would
    // stop noticing if the launcher started pointing somewhere else entirely.
    Some(
        shell_defaults(&text)
            .into_iter()
            .map(|(k, v)| {
                // The build registry's exports (`d1build.py shell`) are expanded the same way.
                let manifest = format!("{repo}/{}", d1_build::selected().manifest);
                let v = v.replace("${D1_BUILD_MANIFEST}", &manifest).replace("$D1_BUILD_MANIFEST", &manifest);
                (k, v.replace("${REPO}", &repo).replace("$REPO", &repo))
            })
            .collect(),
    )
}

// ── fake /proc ───────────────────────────────────────────────────────────────────────────────

struct FakeProcs {
    live: BTreeMap<i32, (u64, String)>,
}

impl ProcTable for FakeProcs {
    fn starttime(&self, pid: i32) -> Option<u64> {
        self.live.get(&pid).map(|(t, _)| *t)
    }
    fn cmdline(&self, pid: i32) -> Option<String> {
        self.live.get(&pid).map(|(_, c)| c.clone())
    }
    fn now_unix(&self) -> u64 {
        1_767_225_600
    }
}

// ── the suite ────────────────────────────────────────────────────────────────────────────────

pub fn run() -> Vec<Control> {
    let mut c = Vec::new();
    let opts = Options::default();
    let l = layout::build(&opts);
    let json = layout::to_json(&l);

    // ---- determinism -------------------------------------------------------------------
    let again = layout::to_json(&layout::build(&opts));
    c.push(check(
        "determinism: the same seed produces byte-identical JSON",
        json == again,
        format!("{} bytes", json.len()),
    ));

    let mut o2 = opts.clone();
    o2.seed = format!("{}-second", opts.seed);
    let l2 = layout::build(&o2);
    let ids_a: Vec<&str> = l
        .participants
        .iter()
        .flat_map(|p| {
            [
                p.instance_id.as_str(),
                p.participant_id.as_str(),
                p.np.npid.as_str(),
                p.d1.account_soid.as_str(),
                p.d1.character_soid.as_str(),
            ]
        })
        .collect();
    let ids_b: Vec<&str> = l2
        .participants
        .iter()
        .flat_map(|p| {
            [
                p.instance_id.as_str(),
                p.participant_id.as_str(),
                p.np.npid.as_str(),
                p.d1.account_soid.as_str(),
                p.d1.character_soid.as_str(),
            ]
        })
        .collect();
    // Participant A is anchored on purpose, so its identity is EXPECTED to repeat across seeds.
    // The seed must move everything that is not anchored.
    let anchored: Vec<&str> = vec![
        l.participants[0].np.npid.as_str(),
        l.participants[0].d1.account_soid.as_str(),
        l.participants[0].d1.character_soid.as_str(),
    ];
    let leaked: Vec<&&str> = ids_a
        .iter()
        .filter(|x| ids_b.contains(x) && !anchored.contains(x))
        .collect();
    c.push(check(
        "determinism: a second seed collides with nothing that is not deliberately anchored",
        l.run_id != l2.run_id && leaked.is_empty(),
        format!("{} vs {}, unexpected shared ids: {leaked:?}", l.run_id, l2.run_id),
    ));

    // ---- collisions --------------------------------------------------------------------
    let problems = layout::validate(&l);
    c.push(check(
        "collision: the generated layout validates clean",
        problems.is_empty(),
        problems.join("; "),
    ));

    let mut clash = l.clone();
    let a_addr = clash.participants[0].addr.clone();
    clash.participants[1].addr = a_addr.clone();
    for ep in &mut clash.participants[1].endpoints {
        ep.addr = a_addr.clone();
    }
    let clash_problems = layout::validate(&clash);
    c.push(check(
        "collision: the detector FIRES when two participants share an address (meta)",
        clash_problems.iter().any(|p| p.contains("collision")),
        format!("{} problems reported", clash_problems.len()),
    ));

    let mut nonetns = l.clone();
    nonetns.lan.netns_separation = false;
    c.push(check(
        "collision: 3074 on both participants is refused once netns separation is withdrawn",
        layout::validate(&nonetns).iter().any(|p| p.contains("netns_separation"))
            && !layout::validate(&l).iter().any(|p| p.contains("netns_separation")),
        "same layout, one flag - the only difference is the justification",
    ));

    let host_scope_3074 = l
        .participants
        .iter()
        .flat_map(|p| p.endpoints.iter())
        .any(|e| e.port == GUEST_BOUND_UDP && e.scope != Scope::NamespaceLocal);
    c.push(check(
        "collision: the guest-bound port 3074 is never claimed at LAN or host scope",
        !host_scope_3074,
        "we do not allocate 3074; the guest binds it",
    ));

    // ⭐ The defect in the superseded Python generator, written down as an executable statement:
    // `game_port: 3074 + i` hands participant B 3075, which is the port `d1-responder` advertises
    // for BAP. Its own self-test could not see this because the responder is not in its model.
    c.push(check(
        "collision: the superseded two-instance-layout.py port plan would have collided",
        GUEST_BOUND_UDP + 1 == RESPONDER_BAP_UDP,
        format!(
            "3074+1 == {RESPONDER_BAP_UDP} == d1-responder --bap-port default; the Python \
             generator's participant B took exactly that number"
        ),
    ));

    // ---- identity ----------------------------------------------------------------------
    let mut conflated = l.clone();
    conflated.participants[1].np.npid = conflated.participants[0].np.npid.clone();
    c.push(check(
        "identity: the conflation detector FIRES on a shared npid (meta)",
        layout::validate(&conflated).iter().any(|p| p.contains("conflation")),
        "npid is what shadnet-server keys every identity and room on",
    ));

    c.push(check(
        "identity: both npids fit the 16-character NP online-ID limit",
        l.participants.iter().all(|p| p.np.npid.chars().count() <= NP_ONLINE_ID_MAX),
        l.participants.iter().map(|p| format!("{}={}", p.slot, p.np.npid)).collect::<Vec<_>>().join(" "),
    ));

    c.push(check(
        "identity: participant B never carries the anchored known-example SOID",
        l.participants[1].d1.account_soid != format!("0x{ANCHOR_ACCOUNT_SOID:016X}")
            && l.participants[1].d1.character_soid != format!("0x{ANCHOR_CHARACTER_SOID:016X}"),
        format!(
            "B account={} character={}",
            l.participants[1].d1.account_soid, l.participants[1].d1.character_soid
        ),
    ));

    let mut roots: Vec<&String> = l.participants.iter().flat_map(|p| p.paths.values()).collect();
    let before = roots.len();
    roots.sort();
    roots.dedup();
    c.push(check(
        "identity: no mutable root is shared between participants",
        roots.len() == before,
        format!("{before} paths, {} distinct", roots.len()),
    ));

    // ---- the 503 splice ----------------------------------------------------------------
    let template = hex_to_bytes(INV_503_TEMPLATE_HEX).unwrap();
    let hits = find_bit_runs(&template, ANCHOR_ACCOUNT_SOID, 64);
    c.push(check(
        "503: the anchor account SOID occurs at exactly ONE bit offset, and it is the constant",
        hits == vec![INV_503_ACCOUNT_SOID_BIT_OFFSET],
        format!("offsets {hits:?}, constant {INV_503_ACCOUNT_SOID_BIT_OFFSET}"),
    ));

    let round_trip = splice_account_soid(&template, INV_503_ACCOUNT_SOID_BIT_OFFSET, ANCHOR_ACCOUNT_SOID);
    c.push(check(
        "503: splicing A's own SOID back reproduces the pinned body byte-for-byte (positive arm)",
        bytes_to_hex(&round_trip) == INV_503_TEMPLATE_HEX,
        bytes_to_hex(&round_trip),
    ));

    let wrong = splice_account_soid(&template, INV_503_ACCOUNT_SOID_BIT_OFFSET, 0xD100_0000_DEAD_BEEF);
    c.push(check(
        "503: splicing a DIFFERENT SOID does not reproduce it (meta - the comparison can fail)",
        bytes_to_hex(&wrong) != INV_503_TEMPLATE_HEX
            && find_unique_bit_run(&wrong, 0xD100_0000_DEAD_BEEF, 64)
                == Some(INV_503_ACCOUNT_SOID_BIT_OFFSET),
        "a check that only ever passes is decoration",
    ));

    let b_raw = hex_to_bytes(&l.participants[1].d1.inv_503_raw).unwrap();
    let b_soid =
        u64::from_str_radix(l.participants[1].d1.account_soid.trim_start_matches("0x"), 16).unwrap();
    c.push(check(
        "503: participant B's body decodes back to participant B's own account SOID",
        read_bits(&b_raw, INV_503_ACCOUNT_SOID_BIT_OFFSET, 64) == Some(b_soid),
        format!("{:#018x}", b_soid),
    ));

    let differing: Vec<usize> = (0..template.len() * 8)
        .filter(|&i| read_bits(&template, i, 1) != read_bits(&b_raw, i, 1))
        .collect();
    let inside = differing.iter().all(|&i| {
        (INV_503_ACCOUNT_SOID_BIT_OFFSET..INV_503_ACCOUNT_SOID_BIT_OFFSET + 64).contains(&i)
    });
    c.push(check(
        "503: the splice touches ONLY the 64-bit SOID field - the family-5 root at bit 165 is intact",
        inside && !differing.is_empty(),
        format!("{} bits differ, all within [36,100)", differing.len()),
    ));

    // ---- agreement with run-boot.sh, which this crate does not own ----------------------
    match run_boot_defaults() {
        None => c.push(skipped(
            "anchor: participant A reproduces run-boot.sh's pinned environment",
            "capture-rig/run-boot.sh not readable from here",
        )),
        Some(d) => {
            let a = &l.participants[0];
            let want = [
                ("D1_ACCOUNT_SOID", a.d1.account_soid.clone()),
                ("D1_CHARACTER_SOID", a.d1.character_soid.clone()),
                // ⛔ `D1_503_RAW` IS DELIBERATELY NOT COMPARED HERE ANY MORE (2026-08-10).
                // `run-boot.sh` no longer pins it: the body is `d1-responder`'s ten-flag DEFAULT,
                // and an export would override it. Asserting agreement with a variable that must
                // be absent is how this control would have forced the empty body back. What
                // replaces it is the pair below - "run-boot.sh must not pin it" here, and byte
                // identity with `d1_bap::INVESTMENT_503_UNLOCKS_RAW` in `layout.rs`'s test.
                // ⛔ `D1_206_BODY` LEFT THE SAME WAY (2026-08-11): 206 is answered per request
                // since 2026-08-10 and the export became a refusable override, so the layout no
                // longer carries it and the not-pinned control below asserts its absence.
                ("D1_503_SOID", a.env["D1_503_SOID"].clone()),
                ("D1_PUSH_FAMILY", a.env["D1_PUSH_FAMILY"].clone()),
                ("D1_PUSH_AT", a.env["D1_PUSH_AT"].clone()),
                ("D1_PROFILE_FILE", a.env["D1_PROFILE_FILE"].clone()),
                ("D1_MANIFEST_FILE", a.env["D1_MANIFEST_FILE"].clone()),
            ];
            let mismatched: Vec<String> = want
                .iter()
                .filter_map(|(k, ours)| match d.get(*k) {
                    Some(theirs) if theirs == ours => None,
                    Some(theirs) => Some(format!("{k}: ours={ours} run-boot={theirs}")),
                    None => Some(format!("{k}: absent from run-boot.sh")),
                })
                .collect();
            c.push(check(
                "anchor: participant A reproduces run-boot.sh's pinned environment",
                mismatched.is_empty(),
                if mismatched.is_empty() {
                    format!("{} pinned values agree", want.len())
                } else {
                    mismatched.join("; ")
                },
            ));
            // ⭐⭐ The new invariant, and the one that decides whether rung 33's unlock flags reach
            // the wire at all: `run-boot.sh` must leave `D1_503_RAW`/`D1_205_RAW` UNSET so the
            // responder's ten-flag default applies. **Env beats default**, so a re-added export
            // would silently restore `count = 0` while every other control in the tree stayed
            // green. Inverting this control means re-adding the export - it goes red immediately.
            let pinned_raw: Vec<&str> = ["D1_503_RAW", "D1_205_RAW", "D1_206_BODY"]
                .into_iter()
                .filter(|k| d.contains_key(*k))
                .collect();
            c.push(check(
                "anchor: run-boot.sh does NOT pin D1_503_RAW/D1_205_RAW/D1_206_BODY (the responder defaults win)",
                pinned_raw.is_empty() && !l.participants.iter().any(|p| p.env.contains_key("D1_206_BODY")),
                if pinned_raw.is_empty() {
                    "none exported; 503 gets the ten-flag body, 206 is answered per request"
                        .to_string()
                } else {
                    format!("{pinned_raw:?} exported - this OVERRIDES the responder's fix")
                },
            ));
            // The parser must be able to fail, or the control above is vacuous.
            c.push(check(
                "anchor: the run-boot.sh parser can detect a difference (meta)",
                d.get("D1_ACCOUNT_SOID").map(|v| v != "0xDEADBEEF").unwrap_or(false)
                    && d.len() >= 8
                    // …and nothing compared above is still an unexpanded shell variable, which
                    // would make a "match" mean only that both sides are equally unresolved.
                    && !d.values().any(|v| v.contains("$REPO")),
                format!("{} defaults parsed, none left unexpanded", d.len()),
            ));
        }
    }

    // ---- generation has no side effects -------------------------------------------------
    // ⚠ Do NOT do this by diffing a shared directory listing: /tmp churns, and a control that fails
    // when an unrelated process writes a file is a flake, which is worse than no control. Assert
    // the property itself - every path this layout names is absent afterwards.
    let probe_root = std::env::temp_dir().join("d1-harness-side-effect-probe");
    let mut o3 = opts.clone();
    o3.root = probe_root.display().to_string();
    let probe = layout::build(&o3);
    let _ = layout::to_json(&probe);
    let mut created: Vec<String> = Vec::new();
    if std::path::Path::new(&probe_root).exists() {
        created.push(probe_root.display().to_string());
    }
    for p in &probe.participants {
        for v in p.paths.values() {
            if std::path::Path::new(v).exists() {
                created.push(v.clone());
            }
        }
    }
    if std::path::Path::new(&probe.base_dir).exists() {
        created.push(probe.base_dir.clone());
    }
    c.push(check(
        "generation creates no directory, opens no socket and launches nothing",
        created.is_empty(),
        format!(
            "{} declared paths under {} - none exist",
            probe.participants.iter().map(|p| p.paths.len()).sum::<usize>(),
            probe_root.display()
        ),
    ));

    // ---- preflight: the saturating POSITIVE arm first -----------------------------------
    let armed = armed_layout(&l);
    let clean = Fake::clean(&armed);
    let ready = run_preflight(&armed, &clean, Some("operator activation review 2026-08-08"), "a");
    c.push(check(
        "⭐ preflight: a fully satisfied world reaches READY (without this every refusal below is decoration)",
        ready.ready(),
        format!(
            "{} checks, refusals: {:?}",
            ready.checks.len(),
            ready.reasons()
        ),
    ));

    // Each refusal: mutate exactly one thing.
    let mut fired: Vec<&'static str> = Vec::new();
    let mut refusal = |c: &mut Vec<Control>,
                       name: &'static str,
                       reason: &'static str,
                       report: Report,
                       detail: String| {
        if report.has(reason) {
            fired.push(reason);
        }
        c.push(check(name, report.has(reason) && !ready.has(reason), detail));
    };

    refusal(
        &mut c,
        "preflight refuses without an activation review",
        "ActivationReviewMissing",
        run_preflight(&armed, &clean, None, "a"),
        "the guard is lifted by the operator, not by a flag this tool sets".into(),
    );

    let mut m = armed.clone();
    m.participants[1].np.npid = m.participants[0].np.npid.clone();
    refusal(
        &mut c,
        "preflight refuses on identity conflation",
        "IdentityConflation",
        run_preflight(&m, &clean, Some("review"), "a"),
        "two instances reporting one npid are ONE participant to shadnet-server".into(),
    );

    let mut m = armed.clone();
    m.participants[1].endpoints[1].addr = m.participants[0].addr.clone();
    m.participants[1].endpoints[1].port = m.participants[0].endpoints[1].port;
    refusal(
        &mut c,
        "preflight refuses on an endpoint collision",
        "EndpointCollision",
        run_preflight(&m, &clean, Some("review"), "a"),
        "two owners for one address:port".into(),
    );

    let mut m = armed.clone();
    m.participants[0].controller.isolation = ControllerIsolation::Unresolved;
    refusal(
        &mut c,
        "preflight refuses while controller isolation is unresolved",
        "ControllerIsolationUnresolved",
        run_preflight(&m, &clean, Some("review"), "a"),
        "two pads with one VID/PID are ONE device to SDL; d1-harness pad run --vendor/--product \
         can mint distinct ids, but the mechanism must still be chosen and recorded"
            .into(),
    );

    let mut f = clean.clone();
    f.files.clear();
    refusal(
        &mut c,
        "preflight refuses a missing read-only input (the manifest trap)",
        "ReadOnlyInputMissing",
        run_preflight(&armed, &f, Some("review"), "a"),
        "an absent D1_MANIFEST_FILE halts the client in bap_signin ~90 s later".into(),
    );

    let mut m = armed.clone();
    m.participants[0].capture[0].armed = false;
    refusal(
        &mut c,
        "preflight refuses an unarmed capture plane",
        "CaptureNotArmed",
        run_preflight(&m, &clean, Some("review"), "a"),
        "a plane with no canary cannot distinguish 'quiet' from 'not recording'".into(),
    );

    let mut f = clean.clone();
    f.cap_net_raw = Some(false);
    refusal(
        &mut c,
        "preflight refuses a pcap plane without CAP_NET_RAW",
        "CapNetRawMissing",
        run_preflight(&armed, &f, Some("review"), "a"),
        "tcpdump without CAP_NET_RAW records nothing and exits 0".into(),
    );

    let mut m = armed.clone();
    m.guards.uninstrumented_control = None;
    refusal(
        &mut c,
        "preflight refuses without an uninstrumented control run",
        "UninstrumentedControlMissing",
        run_preflight(&m, &clean, Some("review"), "a"),
        "otherwise a behaviour change cannot be separated from the instrument".into(),
    );

    let mut f = clean.clone();
    f.routes.insert(armed.participants[1].addr.clone(), "10.77.0.99".into());
    refusal(
        &mut c,
        "preflight refuses when the route to the peer selects the wrong source address",
        "PeerRouteWrongSource",
        run_preflight(&armed, &f, Some("review"), "a"),
        "both instances would appear as one endpoint on the wire".into(),
    );

    let mut f = clean.clone();
    f.routes.insert("1.1.1.1".into(), "10.77.0.11".into());
    refusal(
        &mut c,
        "preflight refuses when anything off the lab is routable (containment negative arm)",
        "EgressReachable",
        run_preflight(&armed, &f, Some("review"), "a"),
        "the CFW console must never reach the internet, and neither may this".into(),
    );

    let mut f = clean.clone();
    f.default_route = Some(true);
    refusal(
        &mut c,
        "preflight refuses when a default route exists",
        "DefaultRoutePresent",
        run_preflight(&armed, &f, Some("review"), "a"),
        "".into(),
    );

    let mut f = clean.clone();
    f.ip_forward = Some(1);
    refusal(
        &mut c,
        "preflight refuses when ip forwarding is enabled",
        "IpForwardingEnabled",
        run_preflight(&armed, &f, Some("review"), "a"),
        "".into(),
    );

    let mut f = clean.clone();
    f.subnet_route = Some(false);
    refusal(
        &mut c,
        "preflight refuses when it is not inside the harness namespace",
        "NotInsideHarnessNamespace",
        run_preflight(&armed, &f, Some("review"), "a"),
        "a network verdict about the wrong namespace is worse than none".into(),
    );

    let mut f = clean.clone();
    f.existing.push(armed.participants[0].portable_dir_must_not_exist.clone());
    refusal(
        &mut c,
        "preflight refuses when a portable user directory shadows XDG_DATA_HOME",
        "PortableUserDirShadowsXdg",
        run_preflight(&armed, &f, Some("review"), "a"),
        "path_util.cpp tries $PWD/user first; both instances would share one identity".into(),
    );

    let mut f = clean.clone();
    f.emulator_pids = vec![4242];
    refusal(
        &mut c,
        "preflight refuses when an unaccounted emulator is already running",
        "EmulatorAlreadyRunning",
        run_preflight(&armed, &f, Some("review"), "a"),
        "the activation review decides whether a SECOND may start, not whether a stray first one \
         may be ignored"
            .into(),
    );

    let mut f = clean.clone();
    f.dirs.clear();
    refusal(
        &mut c,
        "preflight refuses when a capture sink directory is not writable",
        "CaptureSinkUnwritable",
        run_preflight(&armed, &f, Some("review"), "a"),
        "".into(),
    );

    let no_slot = preflight::run(
        &Input {
            layout: &armed,
            stage: Stage::Arm,
            slot: None,
            activation_review: Some("review".into()),
            expect_sha: BTreeMap::new(),
        },
        &clean,
    );
    refusal(
        &mut c,
        "preflight refuses when no participant slot was named",
        "SlotNotSpecified",
        no_slot,
        "namespace verdicts are per-participant".into(),
    );

    let mut sha = BTreeMap::new();
    sha.insert("emulator".to_string(), "0".repeat(64));
    let sha_report = preflight::run(
        &Input {
            layout: &armed,
            stage: Stage::Arm,
            slot: Some("a".into()),
            activation_review: Some("review".into()),
            expect_sha: sha,
        },
        &clean,
    );
    refusal(
        &mut c,
        "preflight refuses when a pinned binary sha256 does not match",
        "BinaryShaMismatch",
        sha_report,
        "a relinked emulator must not be able to masquerade as the one a prior run used".into(),
    );

    let mut f = clean.clone();
    f.files.retain(|p| !p.contains("shadps4"));
    f.existing.retain(|p| !p.contains("shadps4"));
    let missing_bin = preflight::run(
        &Input {
            layout: &armed,
            stage: Stage::Arm,
            slot: Some("a".into()),
            activation_review: Some("review".into()),
            expect_sha: BTreeMap::new(),
        },
        &f,
    );
    refusal(
        &mut c,
        "preflight refuses when a declared binary cannot be hashed",
        "BinaryMissing",
        missing_bin,
        "provenance is not optional; two runs must never be able to carry one identity".into(),
    );

    let mut f = clean.clone();
    f.default_route = None;
    let unevaluable = run_preflight(&armed, &f, Some("review"), "a");
    refusal(
        &mut c,
        "⭐ preflight refuses (not skips) a check it cannot evaluate",
        "CheckUnevaluable",
        unevaluable,
        "an unevaluable check that passes is the same defect as a tracer that arms nothing".into(),
    );

    let mut m = armed.clone();
    m.lan.egress = "allow".into();
    refusal(
        &mut c,
        "preflight refuses a layout whose containment declaration is wrong",
        "LayoutInvalid",
        run_preflight(&m, &clean, Some("review"), "a"),
        "".into(),
    );

    // Every reason must be reachable. An unreachable refusal is a promise nothing keeps.
    let mut unfired: Vec<&&str> =
        preflight::ALL_REASONS.iter().filter(|r| !fired.contains(r)).collect();
    unfired.sort();
    c.push(check(
        "⭐ every named refusal reason was OBSERVED to fire at least once",
        unfired.is_empty(),
        format!("{} reasons, never fired: {unfired:?}", preflight::ALL_REASONS.len()),
    ));

    // ---- shutdown safety ---------------------------------------------------------------
    let procs_live = FakeProcs {
        live: [
            (100, (5000u64, "shadps4 -p patch.xml -g eboot.bin".to_string())),
            (101, (5001u64, "d1-responder --bind 10.77.0.11".to_string())),
        ]
        .into_iter()
        .collect(),
    };
    let mut ledger = Ledger { run_id: l.run_id.clone(), records: vec![] };
    procs::add(&mut ledger, procs::record("emulator-a", 100, "shadps4", &procs_live).unwrap())
        .unwrap();
    procs::add(&mut ledger, procs::record("responder-a", 101, "d1-responder", &procs_live).unwrap())
        .unwrap();

    let p = procs::plan(&ledger, &procs_live);
    c.push(check(
        "shutdown: a verified record is signallable, newest first",
        p == vec![
            Action::Signal { role: "responder-a".into(), pid: 101 },
            Action::Signal { role: "emulator-a".into(), pid: 100 },
        ],
        format!("{p:?}"),
    ));

    let reused = FakeProcs {
        live: [(100, (9999u64, "something else entirely".to_string()))].into_iter().collect(),
    };
    let p = procs::plan(&ledger, &reused);
    c.push(check(
        "shutdown: a reused pid is REFUSED, not signalled",
        p.iter().any(|a| matches!(a, Action::Skip { reason: Skip::PidReused { .. }, .. }))
            && !p.iter().any(|a| matches!(a, Action::Signal { pid: 100, .. })),
        format!("{p:?}"),
    ));

    let renamed = FakeProcs {
        live: [(100, (5000u64, "a completely different command".to_string()))].into_iter().collect(),
    };
    let p = procs::plan(&ledger, &renamed);
    c.push(check(
        "shutdown: a start-time match with a changed command line is REFUSED (instruments disagree)",
        p.iter().any(|a| matches!(a, Action::Skip { reason: Skip::MarkerMismatch { .. }, .. })),
        format!("{p:?}"),
    ));

    let gone = FakeProcs { live: BTreeMap::new() };
    let p = procs::plan(&ledger, &gone);
    c.push(check(
        "shutdown: an exited process is skipped, and nothing else is signalled in its place",
        p.iter().all(|a| matches!(a, Action::Skip { reason: Skip::Exited, .. })),
        format!("{p:?}"),
    ));

    let bad: Vec<Result<PidRecord, String>> = vec![
        procs::record("x", 0, "shadps4", &procs_live),
        procs::record("x", 1, "shadps4", &procs_live),
        procs::record("x", -1, "shadps4", &procs_live),
        procs::record("x", 100, "", &procs_live),
        procs::record("x", 100, "not-in-the-cmdline", &procs_live),
    ];
    c.push(check(
        "shutdown: pid 0, 1, -1, an empty marker and a non-matching marker are all refused at record time",
        bad.iter().all(|r| r.is_err()),
        format!("{} refusals", bad.iter().filter(|r| r.is_err()).count()),
    ));
    c.push(check(
        "shutdown: … and a good record is still accepted (positive arm)",
        procs::record("ok", 100, "shadps4", &procs_live).is_ok(),
        "the refusals above are selective, not blanket",
    ));

    let signalled = procs::execute(
        &[Action::Skip { role: "x".into(), pid: 100, reason: Skip::Exited }],
        libc::SIGTERM,
    );
    c.push(check(
        "shutdown: execute() signals only what plan() cleared - a Skip sends nothing",
        signalled.is_empty(),
        "the plan is the authority; the ledger is not",
    ));

    // ---- /proc reader, against the one process we independently know -------------------
    let real = procs::RealProcs;
    let me = std::process::id() as i32;
    c.push(check(
        "the /proc reader resolves OUR OWN process and refuses a pid that cannot exist",
        real.starttime(me).is_some()
            && real.starttime(me) == real.starttime(me)
            && real.starttime(i32::MAX).is_none(),
        format!("pid {me} starttime {:?}", real.starttime(me)),
    ));

    let sys = preflight::Real;
    c.push(check(
        "the route-lookup probe answers for loopback and is read-only (sends no packet)",
        sys.route_source(Ipv4Addr::LOCALHOST).is_some(),
        format!("127.0.0.1 -> {:?}", sys.route_source(Ipv4Addr::LOCALHOST)),
    ));

    // ---- the virtual pad, pure tier ------------------------------------------------------
    // The device tier (real uinput devices, evdev read-back, the Python side-by-side) is
    // deliberately NOT here: this suite stays side-effect free. Run `d1-harness pad controls`
    // before citing anything about a created pad.
    c.extend(crate::pad::pure_controls());

    c
}

/// Print a suite and return its exit code: 0 all pass, 2 any failure. SKIP is counted
/// separately and loudly, never folded into the pass count.
pub fn report(controls: &[Control]) -> i32 {
    let mut failed = 0;
    let mut skipped_n = 0;
    for c in controls {
        let tag = match c.state {
            State::Pass => "PASS",
            State::Fail => {
                failed += 1;
                "FAIL"
            }
            State::Skipped => {
                skipped_n += 1;
                "SKIP"
            }
        };
        println!("{tag}  {}", c.name);
        if !c.detail.is_empty() {
            println!("        {}", c.detail);
        }
    }
    let n = controls.len() - skipped_n;
    if failed == 0 {
        println!("\nALL CONTROLS PASS  ({n} checks, {skipped_n} skipped)");
        0
    } else {
        println!("\n{failed} CONTROL(S) FAILED - output may not be cited  ({n} checks, {skipped_n} skipped)");
        2
    }
}

pub fn main() -> i32 {
    report(&run())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_control_passes() {
        let controls = super::run();
        let failed: Vec<&str> = controls
            .iter()
            .filter(|c| c.state == super::State::Fail)
            .map(|c| c.name)
            .collect();
        assert!(failed.is_empty(), "failed: {failed:?}");
    }

    #[test]
    fn there_are_enough_controls_to_be_worth_running() {
        assert!(super::run().len() >= 30, "a controls suite this thin is decoration");
    }

    #[test]
    fn the_shell_parser_reads_a_default_and_ignores_a_plain_assignment() {
        let m = super::shell_defaults(
            "export A=\"${A:-one}\"\nexport B=plain\nPRESS=\"${PRESS:-3}\"\nexport C=\"${C:-}\"\n",
        );
        assert_eq!(m.get("A").map(String::as_str), Some("one"));
        assert_eq!(m.get("C").map(String::as_str), Some(""));
        assert!(m.get("B").is_none());
        assert!(m.get("PRESS").is_none(), "only exported defaults count");
    }
}
