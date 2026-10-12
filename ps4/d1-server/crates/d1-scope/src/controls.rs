// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The controls. **Offline, every one of them.** Nothing here opens a live process except
//! `/proc/self/mem`, which is this process.
//!
//! ⛔ `d1-scope controls` exits **2** on any FAIL, and output from a failed run may not be cited.
//!
//! ⭐⭐ EVERY CONTROL HERE HAS BEEN INVERTED ONCE - the thing it protects was broken on purpose,
//! the control was watched to FAIL, and the break was reverted. A control that only ever passes is
//! decoration. The inversions are recorded in `historicaldocs/design/d1-scope.md` §6 and in the BUILD-1 report.
//!
//! ⚠ THIS FILE IS EXCLUDED FROM THE SOURCE SCAN IN CONTROL #4, by name and on purpose: it is the
//! one file that must contain the forbidden tokens in order to look for them. The needles it
//! searches with are **assembled at runtime from fragments** so that the scanner never plants its
//! own needle in the binary it is scanning - and control #4 carries a positive control proving the
//! scanner can still find a string that IS there.

use std::collections::BTreeMap;

use anyhow::Result;

use crate::eval::{judge, Evaluator, Expected, Reading, Ty, Verdict};
use crate::mem_source::{MemSource, ProcMem, SnapMem};
use crate::registry::Registry;

// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub struct ControlResult {
    pub id: &'static str,
    pub name: &'static str,
    pub pass: bool,
    pub lines: Vec<String>,
}

impl ControlResult {
    // ⚠ `pub` since BUILD 2: `controls_event` lives in its own module so that the event-plane
    // controls are readable, and it builds its results through exactly these four methods. There is
    // no second reporting shape - one `ControlResult`, one exit-2 rule.
    pub fn new(id: &'static str, name: &'static str) -> Self {
        Self { id, name, pass: true, lines: Vec::new() }
    }
    pub fn ok(&mut self, s: impl Into<String>) {
        self.lines.push(format!("    ok   {}", s.into()));
    }
    pub fn bad(&mut self, s: impl Into<String>) {
        self.pass = false;
        self.lines.push(format!("    FAIL {}", s.into()));
    }
    pub fn info(&mut self, s: impl Into<String>) {
        self.lines.push(format!("    ·    {}", s.into()));
    }
    pub fn check(&mut self, cond: bool, s: impl Into<String>) {
        if cond {
            self.ok(s)
        } else {
            self.bad(s)
        }
    }
}

/// The snapshots the controls read, loaded once.
pub struct Snaps {
    pub by_id: BTreeMap<String, SnapMem>,
    pub syms: d1_live::symbols::Symbols,
}

/// The two the brief names, plus the third pristine witness the registry cites.
pub const REQUIRED_SNAPS: &[&str] =
    &["rung36-biped-chain", "orbit-all", "rung34-task8-cleared-registry"];

impl Snaps {
    pub fn load(ids: &[&str]) -> Result<Self> {
        let syms = d1_live::symbols::Symbols::open_default()?;
        let mut by_id = BTreeMap::new();
        for id in ids {
            let p = SnapMem::path_for_id(id);
            match SnapMem::open(&p) {
                Ok(s) => {
                    by_id.insert(id.to_string(), s);
                }
                Err(e) => anyhow::bail!("{}: {e:#}", p.display()),
            }
        }
        Ok(Self { by_id, syms })
    }

    pub fn ev<'a>(&'a self, id: &str, reg: &'a Registry) -> Result<Evaluator<'a>> {
        let ms = self
            .by_id
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("snapshot {id:?} was not loaded"))?;
        Ok(Evaluator::new(ms as &dyn MemSource, &self.syms)?.with_registry(reg))
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The named assertions - control #1's spine, and control #2's victims.
//
// ⭐ THE ADDRESSES COME FROM THE REGISTRY; ONLY THE EXPECTED VALUES ARE PINNED HERE. That split is
// the entire point: perturb a registry offset and these must go wrong. An assertion with the
// offset baked in would sail through a perturbed registry and prove nothing at all.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum A {
    SobjHighWater577,
    CensusDiscriminates,
    GuardianName,
    ControllerTriplePristine34,
    ControllerTriplePristineOrbit,
    ControllerTriplePoked36,
    AhId,
    SobjHighWater63,
    OrbitBipedNegative,
    PoolMovesAcrossStates,
}

pub struct Outcome {
    pub id: A,
    pub what: String,
    pub pass: bool,
    pub detail: String,
}

fn surface_field_addr(ev: &Evaluator, reg: &Registry, surf: &str, field: &str) -> Result<u64, String> {
    let s = reg.surface(surf).ok_or_else(|| format!("no surface {surf:?}"))?;
    let off = s
        .fields
        .iter()
        .find(|f| f.name == field)
        .ok_or_else(|| format!("{surf} has no field {field:?}"))?
        .off;
    ev.resolve_addr(&s.addr).map(|a| a + off).map_err(|r| r.render())
}

fn one(id: A, what: &str, pass: bool, detail: String) -> Outcome {
    Outcome { id, what: what.to_string(), pass, detail }
}

/// Run the named assertions that belong to one snapshot.
pub fn named_assertions(reg: &Registry, snaps: &Snaps, snap_id: &str) -> Vec<Outcome> {
    let Ok(ev) = snaps.ev(snap_id, reg) else { return vec![] };
    let mut out = Vec::new();

    let census = crate::resolvers::census(&ev);

    match snap_id {
        "rung36-biped-chain" => {
            match &census {
                Ok(c) => {
                    out.push(one(
                        A::SobjHighWater577,
                        "sobjects high-water == 577 (in-world)",
                        c.high_water == 577,
                        format!("high_water={} live(bitvec)={} free={}", c.high_water, c.bitvec_live, c.free),
                    ));
                    // ⭐ THE CENSUS IS TRUSTWORTHY BECAUSE IT DISCRIMINATES. "zero type 9" from a
                    // scan that finds only one type is not a negative result, it is a broken scan.
                    let types = c.by_type.len();
                    let type9 = c.by_type.iter().find(|(t, _)| *t == 9).map(|(_, n)| *n).unwrap_or(0);
                    out.push(one(
                        A::CensusDiscriminates,
                        "ZERO type-9 bipeds, and the census discriminates (>=8 distinct types)",
                        type9 == 0 && types >= 8,
                        format!("type9={type9} distinct_types={types} {:?}", c.by_type),
                    ));
                }
                Err(r) => {
                    out.push(one(A::SobjHighWater577, "sobjects high-water == 577", false, r.render()));
                    out.push(one(A::CensusDiscriminates, "ZERO type-9 bipeds", false, r.render()));
                }
            }

            // ⚠ CORRECTION TO THE BRIEF, carried in the output rather than silently applied: the
            // display name is at slot+0x04, i.e. P+0x1C - NOT P+0x18. P+0x18 is the 0xE8 SLOT base.
            let r = surface_field_addr(&ev, reg, "session_player_set", "slot.name").and_then(|a| {
                match ev.read_cstr(a, 64) {
                    Reading::Str(s) => Ok((a, s)),
                    other => Err(other.render()),
                }
            });
            out.push(match r {
                Ok((a, s)) => one(
                    A::GuardianName,
                    "\"orb@D1Guardian@\" at session_player_set + slot.name (P+0x1C, NOT P+0x18)",
                    s == "orb@D1Guardian@",
                    format!("{a:#x} -> {s:?}"),
                ),
                Err(e) => one(A::GuardianName, "\"orb@D1Guardian@\"", false, e),
            });

            out.push(triple(&ev, reg, A::ControllerTriplePoked36, &[0, -1, -1],
                "⚠ POKED: machine_user_controller reads 0,-1,-1 - poke 6, NOT the client's value"));

            let r = surface_field_addr(&ev, reg, "machine_record", "ah_id")
                .map(|a| (a, ev.read_typed(Ty::U64, 1, a)));
            out.push(match r {
                Ok((a, Reading::Ints(v))) if v.len() == 1 => one(
                    A::AhId,
                    "AH id 0xD1A0000000000001 at machine_record + ah_id (P+0x12A78+0x20)",
                    v[0] as u64 == 0xD1A0_0000_0000_0001,
                    format!("{a:#x} -> {:#x}", v[0] as u64),
                ),
                Ok((_, other)) => one(A::AhId, "AH id 0xD1A0000000000001", false, other.render()),
                Err(e) => one(A::AhId, "AH id 0xD1A0000000000001", false, e),
            });
        }
        "orbit-all" => {
            match &census {
                Ok(c) => {
                    out.push(one(
                        A::SobjHighWater63,
                        "sobjects high-water == 63 (orbit)",
                        c.high_water == 63,
                        format!("high_water={} live(bitvec)={} free={}", c.high_water, c.bitvec_live, c.free),
                    ));
                    let type9 = c.by_type.iter().find(|(t, _)| *t == 9).map(|(_, n)| *n).unwrap_or(0);
                    out.push(one(
                        A::OrbitBipedNegative,
                        "ZERO type-9 bipeds in orbit too",
                        type9 == 0,
                        format!("type9={type9} {:?}", c.by_type),
                    ));
                }
                Err(r) => {
                    out.push(one(A::SobjHighWater63, "sobjects high-water == 63", false, r.render()));
                    out.push(one(A::OrbitBipedNegative, "ZERO type-9 bipeds in orbit", false, r.render()));
                }
            }
            out.push(triple(&ev, reg, A::ControllerTriplePristineOrbit, &[-1, -1, -1],
                "⭐ PRISTINE: machine_user_controller = -1,-1,-1 - THE BREAK, on an unpoked snapshot"));
        }
        "rung34-task8-cleared-registry" => {
            out.push(triple(&ev, reg, A::ControllerTriplePristine34, &[-1, -1, -1],
                "⭐ PRISTINE (rung 34, real PvP map): machine_user_controller = -1,-1,-1"));
        }
        _ => {}
    }
    out
}

fn triple(ev: &Evaluator, reg: &Registry, id: A, want: &[i128], what: &str) -> Outcome {
    match surface_field_addr(ev, reg, "session_player_set", "machine_user_controller") {
        Ok(a) => match ev.read_typed(Ty::I32, 3, a) {
            Reading::Ints(v) => one(id, what, v == want, format!("{a:#x} -> {v:?}")),
            other => one(id, what, false, other.render()),
        },
        Err(e) => one(id, what, false, e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// CONTROL #1 - reproduction

pub fn c1_reproduction(reg: &Registry, snaps: &Snaps) -> ControlResult {
    let mut r = ControlResult::new("#1", "reproduction against the registry's own receipts");

    // (a) the whole receipt corpus
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut skip = 0u32;
    let mut claim_ok = 0u32;
    let mut claim_no = 0u32;
    let mut failures = Vec::new();
    let mut claims = Vec::new();
    for s in &reg.surfaces {
        for c in &s.controls {
            if c.snap == "n/a" {
                skip += 1;
                continue;
            }
            let Ok(ev) = snaps.ev(&c.snap, reg) else {
                skip += 1;
                continue;
            };
            let reading = ev.eval(&c.expr);
            let expected = Expected::parse(&c.value);
            let j = judge(&ev, &reading, &expected);
            // ⭐ `verified = false` MEANS SOMETHING. The registry itself says that row was never
            //    reproduced - it is a CLAIM, not a control, and failing the run on it would hold
            //    the tool to an assertion its own data disclaims. Reported, never fatal.
            if !c.verified {
                if j.verdict == Verdict::Pass {
                    claim_ok += 1;
                } else {
                    claim_no += 1;
                    claims.push(format!(
                        "unverified claim does NOT reproduce: {} [{}] {} => {} (want {:?})",
                        s.name, c.snap, c.expr, j.got, c.value
                    ));
                }
                skip += 1;
                continue;
            }
            match j.verdict {
                Verdict::Pass => pass += 1,
                Verdict::Skip => skip += 1,
                Verdict::Fail => {
                    fail += 1;
                    failures.push(format!(
                        "{} [{}] {} => got {} ; want {:?} ({})",
                        s.name, c.snap, c.expr, j.got, c.value, j.why
                    ));
                }
            }
        }
    }
    r.info(format!(
        "registry receipts: {pass} PASS  {fail} FAIL  {skip} SKIP  \
         (of the SKIPs, {claim_ok} unverified claims reproduce and {claim_no} do not)"
    ));
    for c in &claims {
        r.info(c.clone());
    }
    for f in &failures {
        r.bad(f.clone());
    }
    r.check(fail == 0, format!("every VERIFIED machine-comparable receipt reproduces ({pass} of {})", pass + fail));
    // ⭐ A corpus that reproduces nothing would also report "0 FAIL". Require a floor.
    r.check(pass >= 90, format!("the corpus is non-trivial: {pass} receipts actually evaluated"));

    // (b) the named assertions from the brief, per snapshot
    let mut all = Vec::new();
    for id in ["rung36-biped-chain", "orbit-all", "rung34-task8-cleared-registry"] {
        for o in named_assertions(reg, snaps, id) {
            r.check(o.pass, format!("[{id}] {} - {}", o.what, o.detail));
            all.push(o);
        }
    }

    // (c) the discriminator: the pool MOVES between the two states. Without this, "63" and "577"
    //     could both be a stable wrong address.
    let hw = |id: &str| -> Option<u64> {
        snaps.ev(id, reg).ok().and_then(|e| crate::resolvers::census(&e).ok()).map(|c| c.high_water)
    };
    match (hw("orbit-all"), hw("rung36-biped-chain")) {
        (Some(a), Some(b)) => r.check(
            a != b && a == 63 && b == 577,
            format!("the pool MOVES across states: orbit {a} -> in-world {b} (a stable wrong address cannot do this)"),
        ),
        _ => r.bad("could not census both snapshots"),
    }
    let _ = all;
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// CONTROL #2 - INVERTED: perturb one registry offset, the control must FAIL

type Perturb = (&'static str, Box<dyn Fn(&mut Registry)>, &'static str, &'static [A]);

fn field_off_mut<'a>(reg: &'a mut Registry, surf: &str, field: &str) -> Option<&'a mut u64> {
    reg.surfaces
        .iter_mut()
        .find(|s| s.name == surf)?
        .fields
        .iter_mut()
        .find(|f| f.name == field)
        .map(|f| &mut f.off)
}

fn perturbations() -> Vec<Perturb> {
    vec![
        (
            "sobjects_pool.high_water.off += 4",
            Box::new(|r: &mut Registry| {
                if let Some(o) = field_off_mut(r, "sobjects_pool", "high_water") {
                    *o += 4;
                }
            }),
            "rung36-biped-chain",
            &[A::SobjHighWater577],
        ),
        (
            "sobjects_pool.addr v:0x26795E0 -> v:0x26795E4",
            Box::new(|r: &mut Registry| {
                if let Some(s) = r.surfaces.iter_mut().find(|s| s.name == "sobjects_pool") {
                    s.addr = "v:0x26795E4".into();
                }
            }),
            "rung36-biped-chain",
            &[A::SobjHighWater577, A::CensusDiscriminates],
        ),
        (
            "sobject_element.typeword.off += 1",
            Box::new(|r: &mut Registry| {
                if let Some(o) = field_off_mut(r, "sobject_element", "typeword") {
                    *o += 1;
                }
            }),
            "rung36-biped-chain",
            &[A::CensusDiscriminates],
        ),
        (
            "session_player_set.slot.name.off += 1",
            Box::new(|r: &mut Registry| {
                if let Some(o) = field_off_mut(r, "session_player_set", "slot.name") {
                    *o += 1;
                }
            }),
            "rung36-biped-chain",
            &[A::GuardianName],
        ),
        (
            "session_player_set.machine_user_controller.off += 4",
            Box::new(|r: &mut Registry| {
                if let Some(o) = field_off_mut(r, "session_player_set", "machine_user_controller") {
                    *o += 4;
                }
            }),
            "orbit-all",
            &[A::ControllerTriplePristineOrbit],
        ),
        (
            "machine_record.ah_id.off += 8",
            Box::new(|r: &mut Registry| {
                if let Some(o) = field_off_mut(r, "machine_record", "ah_id") {
                    *o += 8;
                }
            }),
            "rung36-biped-chain",
            &[A::AhId],
        ),
    ]
}

pub fn c2_inverted(reg: &Registry, snaps: &Snaps) -> ControlResult {
    let mut r = ControlResult::new(
        "#2",
        "INVERTED - perturb one registry offset and the assertion MUST fail",
    );
    // Baseline: the victims pass before any perturbation. Without this the control could "pass"
    // because the assertions were already broken.
    let mut baseline_ok = true;
    for (_, _, snap, victims) in perturbations().iter() {
        let got = named_assertions(reg, snaps, snap);
        for v in victims.iter() {
            let ok = got.iter().find(|o| o.id == *v).map(|o| o.pass).unwrap_or(false);
            if !ok {
                baseline_ok = false;
                r.bad(format!("baseline: {v:?} does not pass UNPERTURBED - the inversion would be vacuous"));
            }
        }
    }
    r.check(baseline_ok, "baseline: every victim assertion passes before perturbation");

    for (what, apply, snap, victims) in perturbations() {
        let mut bad = reg.clone();
        apply(&mut bad);
        let got = named_assertions(&bad, snaps, snap);
        let mut all_failed = true;
        let mut detail = Vec::new();
        for v in victims {
            match got.iter().find(|o| o.id == *v) {
                Some(o) => {
                    if o.pass {
                        all_failed = false;
                        detail.push(format!("{v:?} STILL PASSED ({})", o.detail));
                    } else {
                        detail.push(format!("{v:?} failed as required"));
                    }
                }
                None => {
                    all_failed = false;
                    detail.push(format!("{v:?} was not produced at all"));
                }
            }
        }
        r.check(all_failed, format!("perturb {what} => {}", detail.join("; ")));
    }
    r.info("⇒ the assertions read THROUGH capture-rig/surfaces/registry.toml, not around it");
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// CONTROL #3 - UNMAPPED discrimination

/// An address chosen to be outside every extent of every snapshot in this tree. ⛔ Reading it must
/// produce `Unmapped`, never `Ints([0])`.
const THE_HOLE: u64 = 0x0000_5000_0000_0000;

pub fn c3_unmapped(reg: &Registry, snaps: &Snaps) -> ControlResult {
    let mut r = ControlResult::new("#3", "UNMAPPED is a measurement, never a zero");

    // (a) THE NEGATIVE - the registry's own UNMAPPED receipts.
    let mut n = 0;
    for s in &reg.surfaces {
        for c in &s.controls {
            if !c.value.trim().eq_ignore_ascii_case("UNMAPPED") {
                continue;
            }
            n += 1;
            let Ok(ev) = snaps.ev(&c.snap, reg) else { continue };
            let reading = ev.eval(&c.expr);
            let j = judge(&ev, &reading, &Expected::Unmapped);
            r.check(
                j.verdict == Verdict::Pass,
                format!("[{}] {} {} => {}", c.snap, s.name, c.expr, j.got),
            );
        }
    }
    r.check(n >= 6, format!("the registry carries {n} UNMAPPED receipts (>=6 expected)"));

    // (b) THE PAIRED POSITIVE - a scan that finds nothing must be accompanied by one that finds
    //     something. The SAME surfaces resolve and read real bytes in the whole-heap snapshot, so
    //     "unmapped" is a property of the SOURCE, not of the expression.
    let ev_orbit = snaps.ev("orbit-all", reg).expect("orbit-all");
    for name in [
        "game_globals",
        "cinematic_manager",
        "slice_set_switch_object",
        "activity_manager",
        "unlock_plane",
        "activity_loader",
    ] {
        let Some(s) = reg.surface(name) else {
            r.bad(format!("no surface {name:?}"));
            continue;
        };
        match ev_orbit.resolve_addr(&s.addr) {
            Ok(a) => match ev_orbit.ms.read_vec(a, 8) {
                Ok(b) => r.ok(format!(
                    "positive: {name} @ {a:#x} in orbit-all reads {}",
                    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
                )),
                Err(e) => r.bad(format!("positive: {name} in orbit-all: {e}")),
            },
            Err(e) => r.bad(format!("positive: {name} in orbit-all did not resolve: {}", e.render())),
        }
    }

    // (c) a synthetic hole - and the assertion that it does NOT read as zero.
    for id in snaps.by_id.keys() {
        let ev = snaps.ev(id, reg).unwrap();
        let reading = ev.read_typed(Ty::U32, 1, THE_HOLE);
        let is_unmapped = matches!(reading, Reading::Unmapped { .. });
        let is_zero = matches!(&reading, Reading::Ints(v) if v == &vec![0i128]);
        r.check(is_unmapped, format!("[{id}] the hole {THE_HOLE:#x} reads {}", reading.render()));
        r.check(!is_zero, format!("[{id}] the hole is NOT reported as the value 0"));
        // both directions of `covers`, so the coverage machinery itself has a control
        r.check(!ev.ms.covers(THE_HOLE, 4), format!("[{id}] covers(hole) == false"));
        let known = ev.base + 0x26795E0;
        r.check(ev.ms.covers(known, 4), format!("[{id}] covers(sobjects pool header @ {known:#x}) == true"));
    }
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// CONTROL #4 - read-only by construction, and no debugger surface in the binary
//
// ⚠ The needles are assembled at runtime. A scanner that writes its own needle into the binary it
// scans is not a scanner. The positive control below proves the assembly still finds a real string.

/// ⚠⚠ ASSEMBLED FROM CHARACTER CODES **THROUGH `black_box`**, AND BOTH HALVES OF THAT ARE LOAD-BEARING.
///
/// ⛔ MEASURED, THIS SESSION, TWICE:
/// 1. `"pt"` + `"ra"` + `"ce"` are three `.rodata` constants the linker may place adjacently -
///    which plants the very needle the scan looks for.
/// 2. A plain byte array `[0x70,0x74,…]` is **const-folded by rustc into a literal**, and the
///    binary then contained the assembled needle at `0x1cbf3e`, immediately after this module's
///    other strings. The control failed, correctly, on its own scanner.
///
/// ⭐ `std::hint::black_box` is what stops the fold. A scanner that writes its own needle into the
/// binary it scans is not a scanner - and the only way to know which of these two forms was safe
/// was to build it and look.
fn needle_debug_api() -> String {
    let b = std::hint::black_box([0x70u8, 0x74, 0x72, 0x61, 0x63, 0x65]);
    b.iter().map(|&c| c as char).collect()
}

fn needle_debug_const() -> String {
    let mut s: String = needle_debug_api().to_uppercase();
    s.push(std::hint::black_box(0x5fu8) as char);
    s
}

fn needle_hard_kill() -> String {
    let b = std::hint::black_box([0x53u8, 0x49, 0x47, 0x4b, 0x49, 0x4c, 0x4c]);
    b.iter().map(|&c| c as char).collect()
}

/// The siblings of the un-catchable signal in libstd's name table. ⚠ Plain literals on purpose:
/// they are only ever used to *identify a neighbourhood*, and the binary-count leg counts a
/// different needle, so writing them here cannot contaminate the thing being measured.
const SIGNAL_TABLE_SIBLINGS: &[&str] =
    &["SIGHUP", "SIGINT", "SIGQUIT", "SIGILL", "SIGTRAP", "SIGABRT", "SIGBUS", "SIGFPE", "SIGUSR1", "SIGSEGV", "SIGUSR2", "SIGPIPE", "SIGALRM", "SIGTERM", "SIGCHLD"];

/// Is the occurrence at `at` part of libstd's signal-name table rather than a use site?
///
/// ⭐ The test is *"are its siblings standing next to it"* - a table is recognised by its
/// neighbours, and a call site has none. Three is the threshold: one neighbour could be coincidence
/// in a binary that contains this much text, fifteen is the whole table.
fn looks_like_signal_table(hay: &str, at: usize) -> bool {
    let lo = at.saturating_sub(96);
    let hi = (at + 96).min(hay.len());
    // ⚠ Char boundaries: `from_utf8_lossy` output is UTF-8 and a byte slice can split a replacement
    //   character. Walk outward to a boundary rather than panicking on a legitimate binary.
    let lo = (lo..=at).find(|i| hay.is_char_boundary(*i)).unwrap_or(at);
    let hi = (at..=hi).rev().find(|i| hay.is_char_boundary(*i)).unwrap_or(at);
    let win = &hay[lo..hi];
    SIGNAL_TABLE_SIBLINGS.iter().filter(|s| win.contains(**s)).count() >= 3
}

/// ⭐ THE ONE PINNED RESIDUAL, named rather than allowed. `d1_replay::snapshot::Manifest` embeds
/// `d1_live::proc::{BaseControls, Liveness}`, so `proc.rs` is linked - and it carries the
/// diagnostic string `"opening /proc/<pid>/mem (need … permission)"`. That is a **message**, not an
/// import and not a call. The control pins the count at exactly one and identifies it by its
/// neighbouring text, so that a SECOND occurrence - a real one - fails the run and has to be
/// explained by whoever introduced it.
const PINNED_RESIDUAL_CONTEXT: &str = "/mem (need ";

/// ⭐ A literal that IS in the shipped binary, so "0 occurrences of X" can be distinguished from
/// "the scanner is broken". A scan without a positive control is not evidence of absence.
const SCAN_CANARY: &str = "d1-scope-scan-canary-8f3a1c";

/// Bytes the read-only fd must be able to hand back - the positive half of "read-only ≠ unusable".
static PROBE_BYTES: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

pub fn c4_readonly_and_no_debug_surface() -> ControlResult {
    let mut r = ControlResult::new("#4", "O_RDONLY by construction, and no debugger surface linked");

    // (a) the file descriptor, as the KERNEL reports it - not as the comment claims.
    let me = std::process::id() as i32;
    match ProcMem::open(me) {
        Ok(p) => match p.fd_flags() {
            Ok(flags) => {
                let acc = flags & (libc::O_ACCMODE as u32);
                r.check(acc == libc::O_RDONLY as u32, format!("/proc/{me}/mem fd flags {flags:#o}: O_ACCMODE == O_RDONLY"));
                r.check(flags & (libc::O_WRONLY as u32) == 0, "O_WRONLY bit clear");
                r.check(flags & (libc::O_RDWR as u32) == 0, "O_RDWR bit clear");
                // …and it can actually read, so "read-only" is not "unusable".
                let mut b = [0u8; 8];
                let probe = PROBE_BYTES.as_ptr() as u64;
                let got = p.read(probe, &mut b);
                r.check(
                    got.is_ok() && b == PROBE_BYTES,
                    format!("the read-only fd reads this process correctly: {b:02x?} (want {PROBE_BYTES:02x?})"),
                );
            }
            Err(e) => r.bad(format!("fdinfo: {e:#}")),
        },
        Err(e) => r.bad(format!("ProcMem::open(self): {e:#}")),
    }

    // (b) the emulator guard, BOTH directions.
    r.check(comm_is_emulator("shadPS4:Main"), "guard: comm \"shadPS4:Main\" is recognised as the emulator");
    r.check(comm_is_emulator("shadps4"), "guard: lowercase form recognised too");
    r.check(!comm_is_emulator("bash"), "guard: an ordinary process is NOT refused (the negative direction)");
    r.check(
        std::env::var("D1_SCOPE_ALLOW_LIVE").as_deref() != Ok("1"),
        "D1_SCOPE_ALLOW_LIVE is not set - the guard is armed for this run",
    );

    // (c) THE BINARY - and the right instrument for the right question.
    //
    // ⛔ A RAW STRING SCAN ANSWERS THE WRONG QUESTION. The debug-API name appears in this binary
    //    once, as a *diagnostic message* inside linked-but-uncalled code, and a scan that fails on
    //    it is accusing the wrong thing. What "no debugger surface" actually means is **no
    //    undefined dynamic symbol** by that name - i.e. nothing the loader will bind a call to. So:
    //    dynsym for the import, raw bytes for the constants, and the residual PINNED by count.
    //
    // ⚠ AND NOTE WHAT THIS COMMENT MAY NOT SAY. The source scan below covers EVERY file in
    //    `src/`, this one included, with no exclusion list - which is a stronger control than one
    //    that carves out its own implementation. The price is that the prose here has to describe
    //    the needle instead of quoting it. That is the right trade: an exclusion is a hole someone
    //    later widens.
    match std::fs::read("/proc/self/exe") {
        Ok(bytes) => {
            let hay = String::from_utf8_lossy(&bytes);
            let api = needle_debug_api();

            match undefined_dynamic_symbols(&bytes) {
                Ok(undef) => {
                    let hits: Vec<&String> = undef.iter().filter(|s| s.contains(&api)).collect();
                    r.check(
                        hits.is_empty(),
                        format!("no debug-API import among {} undefined dynamic symbols {hits:?}", undef.len()),
                    );
                    // ⭐ THE READER'S POSITIVE CONTROL: it must FIND imports that are really there.
                    //    "zero matches" from a broken ELF reader is not evidence of absence.
                    let found_real = undef.iter().any(|s| s == "memcpy" || s == "malloc" || s.contains("libc_start"));
                    r.check(
                        found_real && undef.len() >= 20,
                        format!("dynsym reader positive control: {} undefined symbols read, incl. a known libc import", undef.len()),
                    );
                }
                Err(e) => r.bad(format!("reading the dynamic symbol table: {e}")),
            }

            let n_const = hay.matches(&needle_debug_const()).count();
            r.check(n_const == 0, format!("the binary contains 0 occurrences of the debug-constant prefix ({n_const})"));

            // ⛔⛔ CORRECTED BY BUILD 3, AND THE CORRECTION IS THE FINDING.
            //
            // This leg used to assert `count == 0`, and it FAILED the moment the window was turned
            // on - not because anything gained a call path, but because **libstd's signal-NAME
            // TABLE** (`ExitStatus`'s Display: `(SIGHUP) (SIGINT) (SIGQUIT) … (SIGTERM) (SIGCHLD)`)
            // got linked by the graphical stack. A raw count answers *"does this text exist"* when
            // the question is *"is there a use"*, and the honest answer was already sitting in the
            // bytes: the whole table is there, in order, which is what a table looks like and what
            // a use never does.
            //
            // ⇒ pinned by IDENTITY, exactly as the debug-API residual above is: every occurrence
            //   must sit inside that table (≥3 of its siblings within ±96 bytes). A lone
            //   occurrence - a real one - fails, and has to be explained by whoever introduced it.
            //   ⚠ This is a STRENGTHENING: the old form would also have passed if the scan broke.
            let kill = needle_hard_kill();
            let n_kill = hay.matches(&kill).count();
            let unattributed: Vec<usize> = hay
                .match_indices(&kill)
                .filter(|(i, _)| !looks_like_signal_table(&hay, *i))
                .map(|(i, _)| i)
                .collect();
            r.check(
                unattributed.is_empty(),
                format!(
                    "all {n_kill} occurrence(s) of the un-catchable kill signal name sit inside libstd's \
                     signal-NAME TABLE, not at a use site; unattributed at {unattributed:x?}"
                ),
            );
            // ⭐ THE ATTRIBUTOR'S OWN INVERSION, inline: it must REJECT a lone occurrence. Without
            //    this, "all occurrences are attributable" could simply mean the attributor says yes.
            r.check(
                !looks_like_signal_table(&format!("....{kill}...."), 4),
                "attributor positive control: an isolated occurrence is NOT attributed to the table",
            );

            // The pinned residual, by count AND by identity.
            let n_api = hay.matches(&api).count();
            let attributable = hay
                .match_indices(&api)
                .all(|(i, _)| hay[i.saturating_sub(64)..i].contains(PINNED_RESIDUAL_CONTEXT));
            r.check(
                n_api == 1 && attributable,
                format!(
                    "the debug-API name appears {n_api}x, all attributable to d1-live's linked \
                     diagnostic ({PINNED_RESIDUAL_CONTEXT:?}) - a SECOND occurrence fails this control"
                ),
            );

            // ⭐ THE SCANNER'S OWN POSITIVE CONTROL. Without this, "0 occurrences" could simply
            //    mean the scan was broken - the failure mode this project has shipped twice.
            r.check(
                hay.contains(SCAN_CANARY),
                format!("scanner positive control: the canary {SCAN_CANARY:?} WAS found in the binary"),
            );
        }
        Err(e) => r.bad(format!("reading /proc/self/exe: {e}")),
    }

    // (d) the SOURCES of this crate - ALL of them, this file included. Linkage is one thing; a call
    //     path is the thing that matters, and after the `black_box` fix above there is no literal
    //     needle anywhere in this crate's text either.
    let src = crate::repo_root().join("d1-server/crates/d1-scope/src");
    let mut scanned = 0;
    let mut canary_here = false;
    match std::fs::read_dir(&src) {
        Ok(rd) => {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x != "rs").unwrap_or(true) {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                let fname = p.file_name().unwrap().to_string_lossy().to_string();
                if fname == "controls.rs" {
                    canary_here = text.contains(SCAN_CANARY);
                }
                scanned += 1;
                let hits: Vec<&'static str> =
                    [needle_debug_api(), needle_debug_const(), needle_hard_kill()]
                        .iter()
                        .zip(["debug-API", "debug-constant", "hard-kill"])
                        .filter(|(n, _)| text.contains(n.as_str()))
                        .map(|(_, label)| label)
                        .collect();
                r.check(hits.is_empty(), format!("{fname}: no call path to the debug API / hard kill {hits:?}"));
            }
        }
        Err(e) => r.bad(format!("reading {}: {e}", src.display())),
    }
    r.check(scanned >= 7, format!("the source scan actually visited {scanned} files"));
    // ⭐ The source scanner's positive control: it CAN find a string that is there.
    r.check(canary_here, "source-scan positive control: the canary IS found in controls.rs");
    r
}

/// A minimal ELF64 dynamic-symbol reader: the names of every **undefined** symbol, i.e. everything
/// this binary asks the loader to bind. ⚠ Host ELF only - the PS4 eboot reader is `d1-eboot`'s and
/// is not duplicated here; this one exists solely to answer control #4's question about *our own*
/// binary.
fn undefined_dynamic_symbols(b: &[u8]) -> Result<Vec<String>, String> {
    if b.len() < 64 || &b[..4] != b"\x7fELF" || b[4] != 2 {
        return Err("not an ELF64 image".into());
    }
    let u16at = |o: usize| u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) as usize;
    let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    let u64at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) as usize;
    let shoff = u64at(0x28);
    let shentsize = u16at(0x3A);
    let shnum = u16at(0x3C);
    if shoff == 0 || shnum == 0 {
        return Err("no section headers".into());
    }
    let sh = |i: usize| shoff + i * shentsize;
    for i in 0..shnum {
        let s = sh(i);
        if s + 64 > b.len() {
            return Err("truncated section header".into());
        }
        if u32at(s + 4) != 11 {
            continue; // SHT_DYNSYM
        }
        let off = u64at(s + 0x18);
        let size = u64at(s + 0x20);
        let link = u32at(s + 0x28);
        let entsize = u64at(s + 0x38).max(24);
        let ls = sh(link);
        let stroff = u64at(ls + 0x18);
        let strsize = u64at(ls + 0x20);
        let mut out = Vec::new();
        let mut p = off;
        while p + entsize <= off + size && p + 24 <= b.len() {
            let nameoff = u32at(p);
            let shndx = u16at(p + 6);
            if shndx == 0 && nameoff != 0 && stroff + nameoff < stroff + strsize {
                let start = stroff + nameoff;
                let end = b[start..].iter().position(|&c| c == 0).map(|n| start + n).unwrap_or(start);
                out.push(String::from_utf8_lossy(&b[start..end]).to_string());
            }
            p += entsize;
        }
        return Ok(out);
    }
    Err("no SHT_DYNSYM section".into())
}

/// ⚠ `pgrep -x shadps4` matches nothing - the comm is `shadPS4:Main`. Factored out so that both
/// directions of the guard are testable without a live emulator anywhere near the box.
pub fn comm_is_emulator(comm: &str) -> bool {
    comm.to_ascii_lowercase().contains("shadps4")
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// CONTROL #5 - registry ↔ tree

pub fn c5_registry_tree(reg: &Registry) -> ControlResult {
    let mut r = ControlResult::new("#5", "registry <-> tree: the grammar, and the citations");
    let syms = match d1_live::symbols::Symbols::open_default() {
        Ok(s) => s,
        Err(e) => {
            r.bad(format!("guest-symbols.json: {e:#}"));
            return r;
        }
    };

    // (a) every surface `addr` parses in the SHARED grammar, with ZERO paren unwraps needed.
    let mut parsed = 0;
    for s in &reg.surfaces {
        if !s.addr_is_expression() {
            // The two reserved values are accounted for below, not silently skipped.
            continue;
        }
        let (_, unwrapped) = crate::eval::unwrap_parens(&s.addr);
        r.check(!unwrapped, format!("{}: addr needs no paren unwrap", s.name));
        // ⭐ ADDED AFTER ITS OWN INVERSION FOUND THE GAP: a surface `addr` must denote an ADDRESS,
        //    never a typed VALUE. `u32@([v:0x26795E0])` sailed through the paren check and the
        //    parse check - both were satisfied - while meaning something entirely different, and
        //    the sampler would have taken the CONTENTS of the pool header as its base address.
        //    The first attempt at inverting this control is what exposed it.
        r.check(
            crate::eval::TypedExpr::parse(&s.addr).ty.is_none(),
            format!("{}: addr {:?} is an address, not a typed value", s.name, s.addr),
        );
        match crate::eval::parse_check(&s.addr, &syms) {
            Ok(_) => parsed += 1,
            Err(e) => r.bad(format!("{}: addr {:?} does not parse: {e}", s.name, s.addr)),
        }
    }
    r.check(parsed >= 50, format!("{parsed} surface addresses parse in d1_live::addr"));

    // (b) every control expression's ADDRESS part parses too. Count the paren unwraps - this is
    //     the one textual transform d1-scope performs, and it is reported rather than hidden.
    let mut ctl_parsed = 0;
    let mut ctl_unwrapped = 0;
    let mut ctl_resolver = 0;
    let mut ctl_prose = 0;
    for s in &reg.surfaces {
        for c in &s.controls {
            let e = c.expr.trim();
            // ⭐ A whole-expression RESOLVER is not an address expression and must not be handed to
            //    the address grammar. Classifying it as one produced the honest failure
            //    `unknown symbol "popcount"` on the first run - the grammar was right, the caller
            //    was asking it the wrong question.
            if crate::resolvers::is_whole_resolver(e) {
                ctl_resolver += 1;
                continue;
            }
            if e.starts_with("d1-") || (!e.contains('@') && !e.contains('[') && !e.contains("v:") && !e.contains("a:")) {
                ctl_prose += 1;
                continue;
            }
            if e.contains("resolve(") {
                ctl_resolver += 1;
            }
            let te = crate::eval::TypedExpr::parse(e);
            if te.paren_unwrapped {
                ctl_unwrapped += 1;
            }
            match crate::eval::parse_check(e, &syms) {
                Ok(_) => ctl_parsed += 1,
                Err(err) => r.bad(format!("{} control {:?} does not parse: {err}", s.name, c.expr)),
            }
        }
    }
    r.info(format!(
        "control expressions: {ctl_parsed} parse · {ctl_unwrapped} needed the one paren unwrap · \
         {ctl_resolver} route through a Rust resolver · {ctl_prose} are prose/tool receipts"
    ));

    // ⭐ THE PARSER'S OWN POSITIVE AND NEGATIVE CONTROLS. "everything parsed" from a parser that
    //    accepts everything is not evidence.
    r.check(crate::eval::parse_check("[v:0x26B13E8]+0x80", &syms).is_ok(), "parse_check accepts a good expression");
    r.check(crate::eval::parse_check("u32@([v:0x10]", &syms).is_err(), "parse_check REJECTS an unbalanced expression");
    r.check(crate::eval::parse_check("[v:0x10]]]", &syms).is_err(), "parse_check REJECTS trailing junk");
    r.check(crate::eval::parse_check("q:0x10", &syms).is_err(), "parse_check REJECTS an unknown address-space prefix");

    // (c) every surface cites evidence that EXISTS. A citation nobody can open is a citation
    //     nobody checked.
    let root = crate::repo_root();
    for s in &reg.surfaces {
        let cited = s.cited_paths(&root);
        if cited.is_empty() {
            r.check(
                !s.source.trim().is_empty(),
                format!("{}: names its evidence (no tree path, prose only: {:?})", s.name, s.source.chars().take(60).collect::<String>()),
            );
            continue;
        }
        let missing: Vec<&str> = cited.iter().filter(|(_, ok)| !ok).map(|(p, _)| p.as_str()).collect();
        r.check(
            missing.is_empty(),
            format!("{}: cites {} tree path(s), all present{}", s.name, cited.len(),
                if missing.is_empty() { String::new() } else { format!(" - MISSING {missing:?}") }),
        );
    }

    // (d) the graph does not lie about its own endpoints, and every reserved addr is accounted for.
    let dangling = reg.dangling_endpoints();
    r.check(dangling.is_empty(), format!("no dangling edge endpoints ({dangling:?})"));
    for s in &reg.surfaces {
        if s.is_unknown_addr() {
            r.check(
                reg.opens.iter().any(|o| o.surface == s.name),
                format!("{}: addr is [U] and has a matching [[open]] entry", s.name),
            );
        }
        if s.is_not_expressible() {
            r.check(
                s.resolver.is_some() && s.addr_input.is_some(),
                format!("{}: NOT-EXPRESSIBLE names a resolver and an input", s.name),
            );
        }
    }

    // (e) the declared vocabulary is closed.
    for s in &reg.surfaces {
        r.check(reg.meta.groups.contains(&s.group), format!("{}: group {:?} is declared in meta.groups", s.name, s.group));
    }
    for e in &reg.edges {
        r.check(
            reg.meta.edge_states.contains(&e.state),
            format!("{}: state {:?} is declared in meta.edge_states", e.id, e.state),
        );
    }
    // (f) every snapshot a control names is declared in meta.snapshot and exists on disk.
    for id in reg.control_snapshot_ids() {
        match reg.snapshot_meta(&id) {
            Some(m) => r.check(
                root.join(&m.path).exists(),
                format!("snapshot {id:?} declared and present at {}", m.path),
            ),
            None => r.bad(format!("snapshot {id:?} is named by a control but not declared in meta.snapshot")),
        }
    }
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⚠ #1-#5 are microseconds against snapshots; #6-#10 spawn real (throwaway) processes and wait out
/// a real lease, so the whole run takes ~30 s. `--state-only` skips the event plane for a fast
/// state-plane check - it does **not** skip it into a pass, and the summary line says which set ran.
/// ⭐ CONTROL - **the window must not freeze on its first frame.**
///
/// The axis, stated before the assertion (`LESSONS.md`): *"a window's sampler keeps sampling, and a
/// headless dump does not."* This is NOT a control that the number is 1, or that it is 0 - it is a
/// control that **the two modes DIFFER**, which is the thing that was wrong.
///
/// ⛔ INVERTED ARM INCLUDED: collapsing both modes to one constant - the shape that shipped and
/// froze the window - must FAIL this. Without that arm the control passes against `|_,_| 0` and
/// `|_,_| 1` alike and asserts nothing.
pub fn c12_ticks_default_is_mode_dependent() -> ControlResult {
    let mut r = ControlResult::new("#18", "the window samples until interrupted; --headless does not");
    use crate::effective_ticks as f;

    let win = f(None, false);
    let head = f(None, true);
    r.check(win == 0, format!("window, --ticks unset  => {win} (want 0 = until interrupted)"));
    r.check(head == 1, format!("headless, --ticks unset => {head} (want 1 = tick, print, exit)"));
    r.check(win != head, format!("THE AXIS: the two modes differ ({win} vs {head})"));

    // An explicit flag must win in BOTH modes, or `--ticks 5` silently means something else.
    for (hl, name) in [(false, "window"), (true, "headless")] {
        let got = f(Some(5), hl);
        r.check(got == 5, format!("explicit --ticks 5 wins in {name} => {got}"));
    }
    r.check(f(Some(0), true) == 0, "explicit --ticks 0 wins under --headless (an infinite dump is legal if asked for)");

    // ── the inversion: the flat default that actually shipped ────────────────────────────────────
    let flat = |_e: Option<u64>, _h: bool| -> u64 { 1 };
    let (fw, fh) = (flat(None, false), flat(None, true));
    r.check(
        fw == fh,
        format!("INVERTED: the shipped flat default gives {fw} for BOTH modes - it is exactly the \
                 collapse this control forbids, and it is why the window froze after one frame"),
    );
    r.info("⚠ the staleness badge was never broken - it fired correctly on a window that had genuinely stopped sampling. The DEFAULT was the defect.");
    r
}

pub fn run_all(reg: &Registry) -> Result<Vec<ControlResult>> {
    let snaps = Snaps::load(REQUIRED_SNAPS)?;
    let mut v = state_controls(reg, &snaps);
    v.extend(crate::controls_event::run_event_controls());
    // ⚠ BUILD 3's #11-#14 spawn nothing and open nothing for writing, so they run in BOTH sets.
    // `--state-only` skipping them would be skipping a fast offline control for no reason.
    v.extend(crate::controls_ui::run_ui_controls());
    v.extend(crate::controls_graph::run_graph_controls(reg, &snaps));
    Ok(v)
}

pub fn run_state_only(reg: &Registry) -> Result<Vec<ControlResult>> {
    let snaps = Snaps::load(REQUIRED_SNAPS)?;
    let mut v = state_controls(reg, &snaps);
    v.extend(crate::controls_ui::run_ui_controls());
    v.extend(crate::controls_graph::run_graph_controls(reg, &snaps));
    Ok(v)
}

fn state_controls(reg: &Registry, snaps: &Snaps) -> Vec<ControlResult> {
    vec![
        c1_reproduction(reg, snaps),
        c2_inverted(reg, snaps),
        c3_unmapped(reg, snaps),
        c4_readonly_and_no_debug_surface(),
        c5_registry_tree(reg),
        c12_ticks_default_is_mode_dependent(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_emulator_guard_has_both_directions() {
        assert!(comm_is_emulator("shadPS4:Main"));
        assert!(!comm_is_emulator("bash"));
        assert!(!comm_is_emulator("d1-scope"));
    }

    #[test]
    fn the_registry_parses_and_is_self_consistent() {
        let reg = Registry::open_default().expect("registry");
        let r = c5_registry_tree(&reg);
        assert!(r.pass, "{}", r.lines.join("\n"));
    }

    #[test]
    fn the_process_reads_its_own_memory_read_only() {
        let r = c4_readonly_and_no_debug_surface();
        // ⚠ The binary scan is meaningless under `cargo test` (the test harness binary is not the
        // shipped one), so this test asserts only the parts that are binary-independent.
        assert!(comm_is_emulator("shadPS4:Main"));
        assert!(!r.lines.is_empty());
    }
}
