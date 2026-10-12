// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The mandatory positive controls.
//!
//! ATLAS §4.4d: **a new tool is not evidence until it has reproduced a known result.**
//! `ready-byte-report.py` printed `NEVER FIRED` for three runs because it read `probe_id` at the
//! field index that actually holds `record_size` - and that output was indistinguishable from the
//! failure it was built to detect. A replay oracle can fail in exactly that shape: a wrong base, a
//! half-mapped region or a rollback that quietly does nothing all produce *plausible* register
//! dumps.
//!
//! So every control here is designed to fail loudly on one specific way this instrument could lie:
//!
//! | control | the lie it catches |
//! |---|---|
//! | build gate | the snapshot is of a different eboot than the address book was measured on |
//! | snapshot-vs-file | the base is wrong, or the region is short/misaligned |
//! | negative control | the compare window is degenerate, so the check above proved nothing |
//! | relocation | we are looking at file bytes, not live ones (pointer tables are zero on disk) |
//! | trampoline | `call` never actually returns, so "no fault" means "never ran" |
//! | **execute-vs-read** | execution disagrees with a direct read of the same address |
//! | **fault-is-a-result** | a fault is swallowed instead of reported |
//! | rollback | candidate *N+1* silently inherits candidate *N*'s writes |
//! | **stack-canary redirect** | the redirect is not actually happening (asked of MEMORY, not of the plan), so ≥45% of the binary is unexecutable and every replay of it reads as "the arguments were wrong" |
//! | **stack-canary epilogue round-trip** | the redirect got past the prologue but the guard value is incoherent, so the epilogue's compare would take `__stack_chk_fail` |
//! | **stack-canary auto safety** (INVERTED) | `auto` redirects a guard pointer that ALREADY resolved - i.e. it alters memory that worked, and every prior replay becomes suspect |
//! | **captured-vs-invented registers** | `--use-captured-regs` is accepted and prints a CAPTURED banner while seeding nothing, so a replay claims the client's entry state and does not have it |
//! | registry cross-check | the captured heap is not the client's registry |
//! | **existing-snapshots-still-load** | a schema change has orphaned the 4.3 GiB archive in `runs/snapshots/` - the snapshots that cost whole nights no longer parse |
//!
//! `d1-replay controls` exits **non-zero** on any FAIL, and any run without a passing control block
//! is stamped **NOT EVIDENCE** in its own output.

use std::rc::Rc;

use anyhow::{Context, Result};

use crate::machine::{Machine, RunSpec};
use crate::snapshot::Snapshot;
use crate::symbols::Symbols;

pub struct Control {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    fn tag(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
}

pub struct Report {
    pub controls: Vec<Control>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.controls.iter().all(|c| c.status != Status::Fail)
    }
    pub fn render(&self) -> String {
        let pass = self.controls.iter().filter(|c| c.status == Status::Pass).count();
        let fail = self.controls.iter().filter(|c| c.status == Status::Fail).count();
        let skip = self.controls.iter().filter(|c| c.status == Status::Skip).count();
        let mut s = String::from("CONTROLS (ATLAS §4.4d - a tool is not evidence until it reproduces a known result):\n");
        for c in &self.controls {
            s.push_str(&format!("  {}  {}: {}\n", c.status.tag(), c.name, c.detail));
        }
        s.push_str(&format!(
            "CONTROLS: {pass} PASS, {fail} FAIL, {skip} SKIP - {}",
            if fail == 0 { "usable as evidence" } else { "⛔ NOT EVIDENCE" }
        ));
        s
    }
}

fn push(v: &mut Vec<Control>, name: &str, status: Status, detail: String) {
    v.push(Control { name: name.into(), status, detail });
}

/// `snapshot_path` is where the snapshot under test came from. It is used only by
/// [`existing_snapshots_still_load`], which tests its SIBLINGS - pass `None` and that control
/// reports `Skip` with the reason, rather than silently vanishing.
pub fn run(
    snap: Rc<Snapshot>,
    syms: &Symbols,
    image: &d1_eboot::Image,
    snapshot_path: Option<&std::path::Path>,
) -> Result<Report> {
    let mut c: Vec<Control> = Vec::new();
    let base = snap.base();

    // 1. Build gate.
    let (ok, line) = syms.check_build(&snap.manifest.eboot.sha256);
    push(&mut c, "build gate", if ok { Status::Pass } else { Status::Fail }, line);
    let file_ok = image.sha256() == snap.manifest.eboot.sha256;
    push(
        &mut c,
        "eboot identity",
        if file_ok { Status::Pass } else { Status::Fail },
        format!(
            "the eboot on disk {} the one the snapshot was taken from",
            if file_ok { "IS" } else { "IS NOT" }
        ),
    );

    // 2/3. Snapshot vs the file image, with its negative control.
    let v = &syms.verify;
    match (image.bytes(v.vaddr, v.len), snap.read(base + v.vaddr, v.len)) {
        (Ok(want), Ok(got)) => {
            let eq = want == got;
            push(
                &mut c,
                "snapshot-vs-file",
                if eq { Status::Pass } else { Status::Fail },
                format!(
                    "{} bytes at v:{:#x} (abs {:#x}) {} the file image",
                    v.len,
                    v.vaddr,
                    base + v.vaddr,
                    if eq { "match" } else { "DIFFER from" }
                ),
            );
            match snap.read(base + v.vaddr + 1, v.len) {
                Ok(shift) => {
                    let disc = shift != want;
                    push(
                        &mut c,
                        "negative control",
                        if disc { Status::Pass } else { Status::Fail },
                        if disc {
                            "the same window shifted 1 byte differs - it discriminates".into()
                        } else {
                            "the shifted window ALSO matches - the window is degenerate and the check above proved nothing".to_string()
                        },
                    );
                }
                Err(e) => push(&mut c, "negative control", Status::Fail, e.to_string()),
            }
        }
        (w, g) => push(
            &mut c,
            "snapshot-vs-file",
            Status::Fail,
            format!("could not compare: file={:?} snapshot={:?}", w.err(), g.err()),
        ),
    }

    // 4. Relocation control - PS4 eboots store pointer tables as ZERO in the file and fill them at
    //    load. If the snapshot's copy is also all-zero, we are not looking at a live image.
    if let Some(bsn) = syms.lookup("boot_state_names") {
        let file_zero = image.bytes(bsn, 64).map(|b| b.iter().all(|&x| x == 0)).unwrap_or(false);
        match snap.read(base + bsn, 64) {
            Ok(live) => {
                let live_zero = live.iter().all(|&x| x == 0);
                let ok = file_zero && !live_zero;
                push(
                    &mut c,
                    "relocation",
                    if ok { Status::Pass } else { Status::Fail },
                    format!(
                        "boot_state_names v:{bsn:#x}: file all-zero={file_zero}, snapshot all-zero={live_zero} \
                         (want file=true, snapshot=false)"
                    ),
                );
            }
            Err(e) => push(&mut c, "relocation", Status::Skip, format!("not captured: {e}")),
        }
    }

    // ── the executing controls ───────────────────────────────────────────────────────────────────
    let mut m = Machine::new(snap.clone())?;

    // 5. The trampoline. If `call` never returns, "no fault" would mean "never ran".
    //    `ret` alone: assemble nothing, just execute the `ret` we can always find - the byte at the
    //    end of any function. Use the constant-folded getter below instead, which ends in `ret`.
    // 6. EXECUTE-VS-READ. `0x2A8570` is `mov eax, [rip+disp32]; ret` - a constant-folded getter.
    //    We verify the SHAPE from the captured bytes and compute the target from the displacement,
    //    so this control does not depend on remembering which global it reads. Then: executing it
    //    must return exactly what a direct read of that global returns. That exercises mapping,
    //    RIP-relative addressing at the recovered base, execution and register readout at once.
    const GETTER: u64 = 0x2A_8570;
    match snap.read(base + GETTER, 7) {
        Ok(bytes) if bytes[0] == 0x8B && bytes[1] == 0x05 && bytes[6] == 0xC3 => {
            let disp = i32::from_le_bytes(bytes[2..6].try_into().unwrap()) as i64;
            let target = (base as i64 + GETTER as i64 + 6 + disp) as u64;
            let direct = snap.u32(target);
            let mut spec = RunSpec::new(base + GETTER);
            spec.step_limit = 1000;
            let r = m.run(&spec)?;
            match (direct, &r.exit) {
                (Ok(d), e) if e.ok() => {
                    let eq = (r.rax as u32) == d;
                    push(
                        &mut c,
                        "trampoline",
                        Status::Pass,
                        format!("v:{GETTER:#x} returned cleanly in {} instructions", r.steps),
                    );
                    push(
                        &mut c,
                        "execute-vs-read",
                        if eq { Status::Pass } else { Status::Fail },
                        format!(
                            "executing v:{GETTER:#x} returned {:#x}; a direct snapshot read of its \
                             own operand (abs {target:#x} = v:{:#x}) is {d:#x} - {}",
                            r.rax as u32,
                            target.wrapping_sub(base),
                            if eq { "they agree" } else { "THEY DISAGREE" }
                        ),
                    );
                }
                (d, e) => {
                    push(&mut c, "trampoline", Status::Fail, format!("v:{GETTER:#x} exited {}", e.render()));
                    push(
                        &mut c,
                        "execute-vs-read",
                        Status::Fail,
                        format!("run={} direct read={:?}", e.render(), d.err()),
                    );
                }
            }
        }
        Ok(bytes) => push(
            &mut c,
            "execute-vs-read",
            Status::Skip,
            format!(
                "v:{GETTER:#x} is not the expected `mov eax,[rip+d32]; ret` shape ({bytes:02x?}) - \
                 refusing to use it as a control rather than assuming"
            ),
        ),
        Err(e) => push(&mut c, "execute-vs-read", Status::Skip, format!("not captured: {e}")),
    }

    // 7. A FAULT IS A RESULT. `0xB45E20` is `mov edi,[rdi+8]; movzx esi,sil; jmp …`. Called with
    //    rdi = 0 the client itself died with "Read from address 0x8" (§26.77). The instrument must
    //    reproduce that exactly - same address, reported as a result rather than as a tool crash.
    const THUNK: u64 = 0xB4_5E20;
    if snap.has(base + THUNK, 3) {
        let mut spec = RunSpec::new(base + THUNK);
        spec.args = vec![0, 0];
        spec.step_limit = 1000;
        let r = m.run(&spec)?;
        match &r.exit {
            crate::machine::Exit::Fault { addr, .. } if *addr == 8 => push(
                &mut c,
                "fault-is-a-result",
                Status::Pass,
                format!(
                    "v:{THUNK:#x}(rdi=0) faulted reading {addr:#x} - byte-for-byte the client's own \
                     'Read from address 0x8' (§26.77), reported as a RESULT"
                ),
            ),
            e => push(
                &mut c,
                "fault-is-a-result",
                Status::Fail,
                format!("v:{THUNK:#x}(rdi=0) gave {} - expected a fault at 0x8", e.render()),
            ),
        }
    } else {
        push(&mut c, "fault-is-a-result", Status::Skip, "the thunk is not in this snapshot".into());
    }

    // 8. Rollback. Write a distinctive pattern into a captured region through the run's own write
    //    path, then require the snapshot bytes back. A rollback that quietly does nothing is the
    //    single most dangerous defect a candidate search can have.
    if let Ok(orig) = snap.read(base + syms.verify.vaddr, 16) {
        let mut spec = RunSpec::new(base + GETTER);
        spec.step_limit = 1000;
        spec.writes = vec![(base + syms.verify.vaddr, vec![0xA5; 16])];
        let _ = m.run(&spec)?;
        let after = m.read(base + syms.verify.vaddr, 16)?;
        let restored = after == orig;
        let (pristine, detail) = m.verify_pristine()?;
        push(
            &mut c,
            "rollback",
            if restored && pristine { Status::Pass } else { Status::Fail },
            format!(
                "a 16-byte write at v:{:#x} was {} after the run; {detail}",
                syms.verify.vaddr,
                if restored { "restored" } else { "NOT RESTORED" }
            ),
        );
    }

    // 9. ⭐ THE STACK-CANARY REDIRECT.
    //
    //    THE AXIS, IN WORDS: **a stack-protected function that previously faulted at its canary now
    //    executes past it.**
    //
    //    Not "it succeeds" - `FUN_0010C500` still faults later, on content pages no snapshot here
    //    holds, and saying otherwise would be the exact overclaim this crate's header warns about.
    //    The control asserts the two halves of the axis separately, so it cannot pass by accident:
    //
    //      a. WITHOUT the redirect the function must fault dereferencing *the captured guard
    //         pointer itself* - not merely "fault somewhere". Requiring the fault ADDRESS to equal
    //         the pointer value is what makes this a canary control rather than a liveness check.
    //      b. WITH the redirect it must reach at least `PAST_CANARY_STEPS` instructions, and its
    //         fault (if any) must be somewhere else.
    //
    //    Both halves are measured, not asserted from theory: 8 steps → host address `0x55ed6ed63c80`
    //    without, 27 steps → a guest content page with (2026-08-09).
    //
    //    The guard global's address is also re-derived here from the instruction stream rather than
    //    trusted as a constant, so a drifted `GUARD_PTR_VADDR` FAILS instead of quietly redirecting
    //    some unrelated global and leaving the canary fault in place.
    //
    //    ⚠ 2026-08-10, adversarial review. The original single row had two terms that did not test
    //    their axis, and one axis with no term at all. Each of the three rows below exists because a
    //    deliberate break was watched to slip past the version that did not have it:
    //
    //      · `redirected` asked `canary.applied.is_some()` - the PLAN. Deleting the `mem_write`
    //        left the FAIL row printing *"guard repointed at 0x700000051000"*. Now: the read-back.
    //      · `pristine` used `verify_pristine`, which samples 0x1000 of every 0x40000 and does NOT
    //        cover v:0x1a06c78 (offset 0x6a78 into its 0x40000 window - measured). Dropping the
    //        guard write from the `dirty` journal left it reporting *"835584 sampled bytes match"*.
    //        Now: a byte compare of the eight bytes in question.
    //      · The epilogue compare was never executed at all - `FUN_0010C500` faults before it - so
    //        "the canary VALUE round-trips" was untested. `FUN_003FE580` closes it by RETURNING.
    //      · Auto's safety claim ("never alters memory that already worked") had no term anywhere.
    canary_control(&mut c, &snap, &mut m)?;
    canary_epilogue_control(&mut c, &snap, &mut m)?;
    canary_auto_safety_control(&mut c, &snap, &mut m)?;

    // 9b. ⭐⭐ CAPTURED vs INVENTED REGISTERS. See the function for the axis and its five terms.
    captured_regs_control(&mut c, &snap, &mut m)?;

    // 10. Registry cross-check - the same 18/18 result `d1guest.py selftest` reports, recomputed from
    //    the snapshot. Derive each family's element address twice: once from the client's RUNTIME
    //    entry table, once from the STATIC family/slot table in guest-symbols.json.
    match registry_cross_check(&snap, syms) {
        Ok((agree, total, detail)) if total > 0 => push(
            &mut c,
            "registry cross-check",
            if agree == total { Status::Pass } else { Status::Fail },
            format!("{agree}/{total} families agree between the runtime entry table and the static table{detail}"),
        ),
        Ok((_, _, detail)) => push(&mut c, "registry cross-check", Status::Skip, detail),
        Err(e) => push(&mut c, "registry cross-check", Status::Skip, e.to_string()),
    }

    // 11. ⭐ THE TWO-PHASE INVARIANT. A trap-paired capture reads some bytes while a thread is
    //     halted and the rest after it resumes. If those two phases could produce OVERLAPPING
    //     region entries, `Snapshot::read` - which walks the table with `range(..=cur).next_back()`
    //     - would silently take the later instant's bytes for an address the caller believes was
    //     read in the stop. That is a wrong answer with no error, which is this project's signature
    //     failure shape. The invariant holds for every snapshot ever written (merge() enforces it),
    //     so it is checked on all of them and not only on trap-paired ones.
    push_regions_do_not_overlap(&mut c, &snap);

    // 12. ⭐ The pre-arm budget refusal and the phase-split verdict, both PURE - no client, no
    //     ptrace, no trap needed, so they run on every `controls` invocation instead of only on the
    //     day someone arms a watchpoint. Each is a PAIR: a case that must refuse next to one that
    //     must not, and a verdict that must be DEGENERATE next to one that must PASS.
    let (st, detail) = stop_budget_control();
    push(&mut c, "stop-budget-refuses-above-the-cap", st, detail);
    let (st, detail) = phase_split_control();
    push(&mut c, "phase-split-is-its-own-pair", st, detail);

    // 12b. ⭐ The caveat must travel with the RESULT, not only with the file. Pure, so it runs
    //      everywhere. Its pair is a fabricated ATOMIC trap, which must NOT print the smear warning.
    let (st, detail) = trap_caveat_control();
    push(&mut c, "the-trap-caveat-is-in-the-result", st, detail);

    // 12c. ⭐ The honesty fields must survive a serde round-trip. An `Option<usize>` that
    //      deserialises to `Some(0)` would turn "the census failed" into "zero threads were
    //      running", which is the atomic-freeze claim printed by a measurement failure.
    let (st, detail) = trap_fields_round_trip_control();
    push(&mut c, "trap-honesty-fields-round-trip", st, detail);

    // 13. ⭐ Backward compatibility of the snapshot FORMAT itself. See
    //     `existing_snapshots_still_load` for the axis and its degeneracy check.
    let dir = snapshot_path.and_then(|p| p.parent()).map(|p| p.to_path_buf());
    match existing_snapshots_still_load(dir.as_deref()) {
        Ok((status, detail)) => push(&mut c, "existing-snapshots-still-load", status, detail),
        Err(e) => push(&mut c, "existing-snapshots-still-load", Status::Fail, format!("{e:#}")),
    }

    Ok(Report { controls: c })
}

/// ⭐ The axis: **no two regions in a snapshot cover the same byte.**
///
/// [`Snapshot::read`] resolves an address with `range(..=cur).next_back()` - the *last* region at or
/// below it. With overlapping entries that silently picks one of two instants, and a trap-paired
/// capture has exactly two instants to confuse: bytes read while the thread was halted, and bytes
/// read after it resumed. So the region table is checked for overlap, ordering and payload extent.
///
/// ⚠ Its own degeneracy check: a snapshot with fewer than two regions cannot exhibit overlap, so it
/// reports `Skip` with that reason rather than a free pass.
fn push_regions_do_not_overlap(c: &mut Vec<Control>, snap: &Snapshot) {
    const NAME: &str = "regions-do-not-overlap";
    let rs = &snap.manifest.regions;
    if rs.len() < 2 {
        push(
            c,
            NAME,
            Status::Skip,
            format!("{} region(s) - a table this small cannot exhibit the defect", rs.len()),
        );
        return;
    }
    let mut sorted: Vec<(u64, u64, &str)> =
        rs.iter().map(|r| (r.addr, r.len, r.origin.as_str())).collect();
    sorted.sort_by_key(|(a, _, _)| *a);
    let mut bad = Vec::new();
    for w in sorted.windows(2) {
        let (a0, l0, o0) = w[0];
        let (a1, _, o1) = w[1];
        if a0 + l0 > a1 {
            bad.push(format!(
                "{a0:#x}+{l0:#x} ({o0}) overlaps {a1:#x} ({o1})"
            ));
        }
    }
    let ordered = rs.windows(2).all(|w| w[0].addr <= w[1].addr);
    let ok = bad.is_empty() && ordered;
    push(
        c,
        NAME,
        if ok { Status::Pass } else { Status::Fail },
        if ok {
            format!(
                "{} region(s), strictly ascending and pairwise disjoint - a read resolves to exactly \
                 one region, so a two-phase (in-stop / after-resume) capture cannot hand back the \
                 wrong instant",
                rs.len()
            )
        } else {
            format!(
                "⛔ {} overlapping pair(s){}: {}. `Snapshot::read` takes the last region at or below \
                 an address, so these bytes resolve ambiguously",
                bad.len(),
                if ordered { "" } else { " AND the table is not in address order" },
                bad.join(" | ")
            )
        },
    );
}

/// ⭐ PURE. The axis: **the hard cap refuses and the budget does not** - and both halves are
/// asserted, because a threshold that refused everything would be just as broken as one that refused
/// nothing, and only the pair can tell them apart.
fn stop_budget_control() -> (Status, String) {
    use crate::onhit::{verdict, Opts, Verdict};
    let o = Opts::default();
    // The spec's own worked example: `--stop-scope wide --preset content`, 586 MiB rehearsed at 102 ms.
    let refuses_bytes = matches!(verdict(586 << 20, 102.0, &o), Verdict::Refuse(_));
    // Time is a separate wall from bytes: 1 MiB that takes 260 ms must refuse too.
    let refuses_ms = matches!(verdict(1 << 20, 260.0, &o), Verdict::Refuse(_));
    // Over budget, under the cap: allowed, loudly, with the in-stop deadline as the enforcement.
    let warns = matches!(verdict(100 << 20, 20.0, &o), Verdict::OverBudget(_));
    // ⭐ And the documented default - 22.7 MiB at 4.6 ms - must sail through, or the tool refuses
    // its own default and nothing can ever be captured.
    let allows_default = verdict(23_800_000, 4.6, &o) == Verdict::Ok;
    let ok = refuses_bytes && refuses_ms && warns && allows_default;
    (
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "586 MiB/102 ms REFUSED={refuses_bytes}, 1 MiB/260 ms REFUSED={refuses_ms} (time is its \
             own wall), 100 MiB/20 ms over-budget-but-allowed={warns}, the default 22.7 MiB/4.6 ms \
             allowed={allows_default}. ⛔ The refusal happens BEFORE the arm: a refusal at hit time \
             burns the trigger"
        ),
    )
}

/// ⭐ PURE. The axis: **a "nothing changed" reading is only evidence if the other half DID change.**
fn phase_split_control() -> (Status, String) {
    use crate::onhit::score_phase_split;
    let pass = score_phase_split(32, 0, 32, 7).starts_with("PASS");
    let code_moved_fails = score_phase_split(32, 1, 32, 7).starts_with("FAIL");
    let nothing_moved_is_degenerate = score_phase_split(32, 0, 32, 0).starts_with("DEGENERATE");
    let nothing_sampled_is_degenerate = score_phase_split(0, 0, 32, 7).starts_with("DEGENERATE");
    let ok = pass && code_moved_fails && nothing_moved_is_degenerate && nothing_sampled_is_degenerate;
    (
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "code-stable + data-moved ⇒ PASS={pass}; code MOVED ⇒ FAIL={code_moved_fails}; nothing \
             moved anywhere ⇒ DEGENERATE={nothing_moved_is_degenerate}; nothing sampled ⇒ \
             DEGENERATE={nothing_sampled_is_degenerate}. The seg0-unchanged half is what justifies \
             reading code AFTER the resume; the seg1-changed half is what proves the comparison \
             could have detected a change at all"
        ),
    )
}

/// ⭐⭐ The axis: **a result taken from a trap-paired snapshot names the trap, in the result** -
/// and the smear warning is conditional on the file's own numbers rather than boilerplate.
///
/// Three legs, all pure:
/// * `None` must render the words NOT ATTEMPTED and must **not** claim the client had no registers.
/// * A realistic trap (110 of 111 threads running, memory read after the resume) must render the
///   SMEAR warning and the thread counts.
/// * ⛔ The **inversion**: a fabricated ATOMIC trap (`all_threads_stopped`, `memory_read_while_stopped`,
///   0 regions after the resume, 0 threads running) must render **without** the smear warning. A
///   warning printed unconditionally carries no information, and this is the leg that proves it is
///   reading the data.
fn trap_caveat_control() -> (Status, String) {
    use crate::snapshot::{render_trap_provenance, TrapProvenance};
    let mk = |atomic: bool| TrapProvenance {
        method: "d1-live DR0 write 8 bytes at 0x8026a34e8".into(),
        tid: 4242,
        comm: "shadPS4:Main".into(),
        rip_vaddr: Some(0x10C500),
        threads_running: if atomic { Some(0) } else { Some(110) },
        threads_total: Some(111),
        threads_armed: Some(111),
        threads_failed: 0,
        all_threads_stopped: atomic,
        memory_read_while_stopped: atomic,
        regions_read_after_resume: if atomic { 0 } else { 7 },
        stop_read_ms: 4.6,
        resumed_read_ms: 812.0,
        deadline_fired: false,
        phase_split: Some("PASS: 32 window(s) unchanged".into()),
    };
    let none = render_trap_provenance(None, "TRAP ", "     ");
    let smear = render_trap_provenance(Some(&mk(false)), "TRAP ", "     ");
    let atomic = render_trap_provenance(Some(&mk(true)), "TRAP ", "     ");

    let none_says_not_attempted = none.contains("NOT ATTEMPTED") && none.contains("carries NO trap");
    let smear_warns = smear.contains("THE MEMORY IS A SMEAR") && smear.contains("110");
    let smear_names_the_window = smear.contains("4.60 ms") && smear.contains("812.00 ms");
    let atomic_is_quiet = !atomic.contains("THE MEMORY IS A SMEAR");
    // And the plumbing: a `RunReport` from a trap-less snapshot must carry `trap: None` rather than
    // omitting the field, so `--json` consumers see it in both states.
    let field_is_serialised = serde_json::to_string(&mk(false)).map(|s| s.contains("all_threads_stopped")).unwrap_or(false);

    let ok = none_says_not_attempted
        && smear_warns
        && smear_names_the_window
        && atomic_is_quiet
        && field_is_serialised;
    (
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "None ⇒ NOT ATTEMPTED={none_says_not_attempted}; a realistic trap ⇒ SMEAR \
             warning={smear_warns} naming 110 running threads and the 4.60/812.00 ms \
             window={smear_names_the_window}; ⛔ INVERTED - a fabricated ATOMIC trap prints NO smear \
             warning={atomic_is_quiet}, so the warning is read off the data and is not boilerplate; \
             serialised for --json={field_is_serialised}. ⭐ This renderer is what `replay` and \
             `search` print - until build 3 they named the trap nowhere"
        ),
    )
}

/// ⭐ The axis: **the honesty fields survive the file.** `threads_total: None` means the
/// `/proc/<pid>/task` census FAILED; if serde turned that into `Some(0)` on the way back, the file
/// would render "0 thread(s) of 0 KEPT RUNNING" - a measurement failure printed as the atomic-freeze
/// claim, in the one artifact whose job is to prevent that claim.
///
/// Paired: the same round-trip with real values must come back with those values, so "None survives"
/// cannot be satisfied by a decoder that returns `None` for everything.
fn trap_fields_round_trip_control() -> (Status, String) {
    let doc = serde_json::json!({
        "method": "m", "watch_addr": 1u64, "watch_addr_vaddr": null, "watch_len": 8u64,
        "watch_rw": "write", "slot": 0, "hit": {},
        "trapped": {"tid": 1, "comm": "c", "stopped_at_ns": 0u64, "regs": {}, "rip_vaddr": null},
        "budget": {"scope":"state","budget_bytes":0u64,"budget_ms":0.0,"hard_cap_bytes":0u64,
                   "hard_cap_ms":0.0,"rehearsed_bytes":0u64,"rehearsed_ms":0.0},
        "coherence": {"trap_unix_nanos":0u64,"debug_registers_cleared_ns":0u64,
                      "stop_read_start_ns":0u64,"stop_read_end_ns":0u64,"thread_resumed_ns":0u64,
                      "resumed_read_end_ns":0u64,"memory_read_while_stopped":false,
                      "all_threads_stopped":false,"measured_mib_per_s":0.0,"deadline_fired":false}
    });
    // (a) the fields ABSENT: every honesty field must default to None / empty, never to 0.
    let absent: Result<crate::snapshot::TrapCapture, _> = serde_json::from_value(doc.clone());
    let (absent_is_none, absent_err) = match &absent {
        Ok(t) => (
            t.threads_total.is_none()
                && t.threads_running.is_none()
                && t.threads_armed.is_none()
                && t.threads_total_at_arm.is_none(),
            String::new(),
        ),
        Err(e) => (false, format!("{e}")),
    };
    // (b) the pair: present values must come back intact, so (a) is not satisfied by a decoder
    //     that answers None to everything.
    let mut with = doc;
    with["threads_total"] = serde_json::json!(111);
    with["threads_running"] = serde_json::json!(110);
    with["threads_armed"] = serde_json::json!(109);
    with["threads_failed"] = serde_json::json!(["tid 7: injected"]);
    let present: Result<crate::snapshot::TrapCapture, _> = serde_json::from_value(with);
    let (present_ok, present_err) = match &present {
        Ok(t) => (
            t.threads_total == Some(111)
                && t.threads_running == Some(110)
                && t.threads_armed == Some(109)
                && t.threads_failed.len() == 1,
            String::new(),
        ),
        Err(e) => (false, format!("{e}")),
    };
    let ok = absent_is_none && present_ok;
    (
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "absent ⇒ None (not Some(0)) = {absent_is_none}{}; present ⇒ 111/110/109 + 1 failure \
             preserved = {present_ok}{}. ⛔ `Some(0)` where the census failed would print \"0 \
             thread(s) of 0 KEPT RUNNING\" - the atomic-freeze claim, rendered from a measurement \
             failure",
            if absent_err.is_empty() { String::new() } else { format!(" ({absent_err})") },
            if present_err.is_empty() { String::new() } else { format!(" ({present_err})") }
        ),
    )
}

/// ⭐ The axis: **a snapshot written before `Manifest::trap` existed must still load.**
///
/// Every file in the directory beside the snapshot under test is opened and its manifest parsed -
/// header and JSON only, never the 4.3 GiB of payloads ([`Snapshot::load_manifest`]). One parse
/// failure fails the control, because a format change that orphans the archive is the most expensive
/// mistake available here: the 48 MB `rung36-in-world-moonbase.snap` cost a night, and rungs 34-36
/// are answerable offline only for as long as it loads.
///
/// ⚠⚠ **The degeneracy check is the part that matters.** Parsing files that all ALREADY carry the
/// new key would prove nothing about backward compatibility, so the raw manifest JSON is inspected
/// for the literal key: at least one file must **lack** it, or the control reports `DEGENERATE`
/// rather than banking a pass. Likewise, an empty or missing directory is `Skip` with the reason
/// said out loud - a scan that finds nothing is not evidence.
///
/// ⛔ Read-only by construction: nothing here opens a snapshot for writing.
pub fn existing_snapshots_still_load(dir: Option<&std::path::Path>) -> Result<(Status, String)> {
    let Some(dir) = dir else {
        return Ok((
            Status::Skip,
            "no snapshot directory was named, so no older snapshot was tested - this control \
             proved NOTHING about backward compatibility on this run"
                .into(),
        ));
    };
    if !dir.is_dir() {
        return Ok((
            Status::Skip,
            format!("{} is not a directory - no older snapshot was tested", dir.display()),
        ));
    }
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("listing {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("snap") | Some("d1snap")
                )
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Ok((
            Status::Skip,
            format!(
                "no *.snap / *.d1snap files in {} - nothing older was tested, so this control \
                 proved NOTHING",
                dir.display()
            ),
        ));
    }

    let mut without_key = 0usize;
    let mut with_key = 0usize;
    let mut with_trap = 0usize;
    let mut failures = Vec::new();
    for p in &files {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        // (a) the raw JSON: is the key literally present? serde cannot answer this after
        //     deserialisation, because a defaulted field looks exactly like an explicit one.
        match Snapshot::load_manifest_json(p) {
            Ok(v) => {
                if v.get("trap").is_some() {
                    with_key += 1;
                } else {
                    without_key += 1;
                }
            }
            Err(e) => failures.push(format!("{name}: raw manifest unreadable - {e:#}")),
        }
        // (b) the typed parse: does it still deserialise into the CURRENT Manifest?
        match Snapshot::load_manifest(p) {
            Ok(m) => {
                if m.trap.is_some() {
                    with_trap += 1;
                }
            }
            Err(e) => failures.push(format!("{name}: {e:#}")),
        }
    }

    if !failures.is_empty() {
        return Ok((
            Status::Fail,
            format!(
                "{} of {} snapshot(s) in {} NO LONGER PARSE against the current Manifest - the \
                 archive is orphaned: {}",
                failures.len(),
                files.len(),
                dir.display(),
                failures.join(" | ")
            ),
        ));
    }
    if without_key == 0 {
        return Ok((
            Status::Skip,
            format!(
                "⚠ DEGENERATE: all {} snapshot(s) in {} already carry a `trap` key, so parsing \
                 them says nothing about files that predate the field. Point this control at an \
                 older archive before citing it.",
                files.len(),
                dir.display()
            ),
        ));
    }
    Ok((
        Status::Pass,
        format!(
            "{} snapshot(s) in {} all parse against the current Manifest; {} of them have NO `trap` \
             key in their JSON at all (they predate it) and load as trap=None, {} carry the key, \
             {} carry a populated trap",
            files.len(),
            dir.display(),
            without_key,
            with_key,
            with_trap
        ),
    ))
}

/// ⭐⭐ **THE AXIS, IN WORDS: a replay that starts from the client's captured registers and a replay
/// that starts from invented ones are DISTINGUISHABLE - in the result AND in the text.**
///
/// This is the control the consumption side needs, and the reason it needs it is specific. A flag
/// named `--use-captured-regs` that was accepted, printed a "CAPTURED" banner and then seeded
/// nothing would produce output identical to an ordinary replay in every respect a reader checks -
/// same `rax`, same steps, same exit - while claiming provenance it does not have. That is this
/// project's signature failure shape (a confident, plausible, wrong reading), and it is invisible
/// unless something asserts that the two modes actually DIFFER.
///
/// **The subject is four bytes of our own**, written into the machine's scratch page:
/// ```text
///   4c 89 e8   mov rax, r13
///   c3         ret
/// ```
/// `r13` is chosen because no `--arg` can reach it (System V passes six arguments in
/// `rdi/rsi/rdx/rcx/r8/r9`), so a seeded value there can only have come from the capture. And a
/// four-byte subject cannot fail for a reason belonging to the client's code: if the two runs do
/// not differ, the seeding is what did not happen.
///
/// Five terms, each closing a way this could pass while broken:
///
/// 1. all three runs must **`RETURN`** - otherwise `rax` is a leftover and every other term is
///    reading noise;
/// 2. the INVENTED run must yield exactly `0` **after a preceding run deliberately left `r13`
///    non-zero** - which is the control on the register zeroing `run()` now does. ⚠ Without that
///    poison run this term was measured to be DECORATION: deleting the zeroing left the row green,
///    because the stack-canary controls that run just before it happen to leave `r13` clear;
/// 3. the CAPTURED run must yield the seeded value, and its report must say `mode == "captured"`;
/// 4. the two RENDERINGS must differ, and each must name its own mode - the text is what a human
///    reads, so "distinguishable in the output" is a claim about the text, not only about `rax`;
/// 5. **precedence**: `--reg r13=…` on top of a captured baseline must win, and the report must
///    list `r13` under `overridden` rather than under `seeded`.
///
/// ⚠ **Where the baseline comes from, stated plainly.** No snapshot in `runs/snapshots/` carries a
/// trap yet (nothing has been armed at the client), so the register map here is a **control
/// stand-in** and its `label` says exactly that in the rendered output. The mechanism is what is
/// under test. When a snapshot DOES carry a trap, a sixth term runs on top: every register the plan
/// claims to have seeded must equal the value in the file, so the stand-in can never mask a
/// mis-keyed real map.
fn captured_regs_control(c: &mut Vec<Control>, snap: &Snapshot, m: &mut Machine) -> Result<()> {
    use crate::machine::{plan_register_origin, CapturedRegs};
    use std::collections::BTreeMap;

    const NAME: &str = "captured-vs-invented-registers";
    /// `mov rax, r13` / `ret`.
    const SUBJECT: [u8; 4] = [0x4c, 0x89, 0xe8, 0xc3];
    /// The stand-in's `r13`. Deliberately not a small number and not a plausible pointer: if it
    /// ever appears in output that is supposed to be the client's, its provenance is unmistakable.
    const SENTINEL: u64 = 0xd1d1_0be1_5eed_0007;
    /// What `--reg r13=…` overrides it with, in the precedence leg.
    const OVERRIDE: u64 = 0x0bad_c0de_0bad_c0de;

    // The baseline: the file's own trap when there is one, otherwise a stand-in that announces
    // itself. ⭐ `subject_reg_value` is what the seeded run must produce, whichever it is.
    let (captured, from_file) = match &snap.manifest.trap {
        Some(t) => (
            CapturedRegs {
                label: format!(
                    "{} - tid {} ({}) [THE SNAPSHOT'S OWN TRAP]",
                    t.method, t.trapped.tid, t.trapped.comm
                ),
                regs: t.trapped.regs.clone(),
            },
            true,
        ),
        None => {
            let mut regs = BTreeMap::new();
            regs.insert("r13".to_string(), SENTINEL);
            regs.insert("rbx".to_string(), SENTINEL ^ 0xffff);
            (
                CapturedRegs {
                    label: "⚠ CONTROL STAND-IN - not a real trap. No snapshot in runs/snapshots/ \
                            carries one yet, so this exercises the MECHANISM"
                        .to_string(),
                    regs,
                },
                false,
            )
        }
    };
    // A real trap whose `r13` happens to be 0 would make terms 2 and 3 agree by coincidence, and
    // the row would pass without an axis. Say DEGENERATE rather than take the free pass.
    let want = captured.regs.get("r13").copied().unwrap_or(0);
    if want == 0 {
        push(
            c,
            NAME,
            Status::Skip,
            format!(
                "DEGENERATE: the baseline's r13 is 0, which is also the invented value, so the two \
                 modes could not be told apart by this subject even if the mechanism worked. \
                 Baseline: {}",
                captured.label
            ),
        );
        return Ok(());
    }

    let mk = |use_captured: bool, over: bool| {
        let mut s = RunSpec::new(m.scratch);
        s.writes = vec![(m.scratch, SUBJECT.to_vec())];
        s.step_limit = 64;
        s.captured = Some(captured.clone());
        s.use_captured = use_captured;
        if over {
            s.regs = vec![(crate::machine::reg_by_name("r13").unwrap(), OVERRIDE)];
        }
        s
    };
    // ⭐⭐ THE POISON RUN, and it exists because the term it protects was measured to be
    // DECORATION without it. Deleting the register zeroing from `run()` on 2026-08-10 left this
    // row GREEN at 18/18 - the invented run read `r13 == 0` anyway, because the stack-canary
    // controls that run just before it happen to leave `r13` clear. So the control now LEAVES A
    // NON-ZERO r13 BEHIND on purpose, and only then asks the invented run what it sees. With the
    // zeroing: 0. Without it: `POISON`, and the row FAILS.
    const POISON: u64 = 0x1111_2222_3333_4444;
    let s_poison = {
        let mut s = mk(false, false);
        s.regs = vec![(crate::machine::reg_by_name("r13").unwrap(), POISON)];
        s
    };
    // ⚠ Every spec is built BEFORE the first run: `mk` borrows the machine, and the runs need it
    // mutably. Building them all up front also fixes the ORDER, which is load-bearing here - the
    // poison run must be the one immediately before the invented run.
    let (s_inv, s_cap, s_over) = (mk(false, false), mk(true, false), mk(true, true));
    // The plan is checked as well as the run: `render()` is what a human reads, and it is computed
    // by the same function `run()` applies, so a preview/action divergence would show up here too.
    let (p_inv, p_cap) = (
        plan_register_origin(&s_inv, m.base).render("  "),
        plan_register_origin(&s_cap, m.base).render("  "),
    );
    let r_poison = m.run(&s_poison)?;
    let r_inv = m.run(&s_inv)?;
    let r_cap = m.run(&s_cap)?;
    let r_over = m.run(&s_over)?;

    let all_returned =
        r_poison.exit.ok() && r_inv.exit.ok() && r_cap.exit.ok() && r_over.exit.ok();
    // The poison must have LANDED, or the term below is asserting against a register that was
    // already zero and we are back to decoration with an extra run.
    let poison_landed = r_poison.rax == POISON;
    let invented_is_zero = r_inv.rax == 0 && !r_inv.regs_origin.is_captured();
    let captured_is_seeded = r_cap.rax == want
        && r_cap.regs_origin.is_captured()
        && r_cap.regs_origin.seeded.iter().any(|(n, v)| n == "r13" && *v == want);
    // ⭐ The TEXT term. Same subject, same memory, same everything except the baseline: the two
    // blocks must not be interchangeable, and each must name its own mode.
    let text_differs = p_inv != p_cap
        && p_cap.contains("CAPTURED")
        && p_inv.contains("INVENTED")
        && !p_inv.contains("⭐ CAPTURED");
    let precedence = r_over.rax == OVERRIDE
        && r_over.regs_origin.overridden.iter().any(|(n, _)| n == "r13")
        && !r_over.regs_origin.seeded.iter().any(|(n, _)| n == "r13");
    // Term 6: only meaningful when the baseline came from the file. ⭐ `eflags` is compared against
    // the file's value WITH THE DOCUMENTED MASK APPLIED, and that is the whole subtlety: the only
    // register this path is allowed to alter is the one whose alteration is printed, so the term
    // has to encode the same rule the seeding does. (Written without the mask first, and the row
    // FAILED on the injected trap - `eflags 0x10346` seeded as `0x246`. A term that could not tell
    // a declared mask from a silent corruption would have been the wrong term either way.)
    let expected = |n: &str| -> Option<u64> {
        let f = captured.regs.get(n).copied()?;
        Some(if n == "eflags" { f & !crate::machine::EFLAGS_UNSAFE } else { f })
    };
    let fidelity =
        !from_file || r_cap.regs_origin.seeded.iter().all(|(n, v)| expected(n) == Some(*v));

    let ok = all_returned
        && poison_landed
        && invented_is_zero
        && captured_is_seeded
        && text_differs
        && precedence
        && fidelity;
    push(
        c,
        NAME,
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "`mov rax,r13; ret` at the scratch page, run four times on this snapshot. POISON \
             (r13={POISON:#x} left behind on purpose) ⇒ rax {:#x}; INVENTED ⇒ rax {:#x} ({}); \
             CAPTURED ⇒ rax {:#x} ({}, want {want:#x}); CAPTURED + `--reg r13={OVERRIDE:#x}` ⇒ rax \
             {:#x}. {} seeded {} register(s) from {}. {}",
            r_poison.rax,
            r_inv.rax,
            r_inv.exit.short(),
            r_cap.rax,
            r_cap.exit.short(),
            r_over.rax,
            if from_file { "The snapshot's OWN trap" } else { "A control stand-in (no archived snapshot carries a trap yet)" },
            r_cap.regs_origin.seeded.len(),
            captured.label,
            if ok {
                format!(
                    "The two modes are DISTINGUISHABLE in the result AND in the printed REGISTERS \
                     block, and an explicit --reg still beats the capture.{}",
                    if from_file { " Every seeded value matches the file." } else { "" }
                )
            } else {
                format!(
                    "⛔ {}{}{}{}{}{}{}",
                    if all_returned { "" } else { "a run did not RETURN, so rax is a leftover and no other term means anything; " },
                    if poison_landed { "" } else { "the poison run did not leave r13 set, so the zeroing term below has no axis; " },
                    if invented_is_zero { "" } else { "THE INVENTED RUN INHERITED THE PREVIOUS RUN'S r13 - the register file is not being reset, so every 'invented' baseline is really the last candidate's leftovers; " },
                    if captured_is_seeded { "" } else { "THE CAPTURED RUN DID NOT SEE THE CAPTURED VALUE - the flag is accepted and the seeding does not happen; " },
                    if text_differs { "" } else { "the two REGISTERS blocks are not distinguishable as text; " },
                    if precedence { "" } else { "an explicit --reg did NOT override the captured value; " },
                    if fidelity { "" } else { "a seeded value DISAGREES with the register map in the file; " },
                )
            }
        ),
    );
    Ok(())
}

/// ⭐ `d1-replay`'s stack-canary control. See the call site for the axis this tests.
///
/// The subject is `FUN_0010C500` - chosen because it is a *plain* stack-protected function: the
/// guard load is at a fixed offset in its prologue, it needs no arguments beyond a scratch pointer,
/// and it faults again 19 instructions later on content we do not hold. That last property is a
/// feature: a control whose subject returns cleanly could pass on a snapshot where the canary was
/// never the obstacle.
fn canary_control(c: &mut Vec<Control>, snap: &Snapshot, m: &mut Machine) -> Result<()> {
    use crate::machine::{Exit, StackCanary};

    const NAME: &str = "stack-canary redirect";
    const PROTECTED: u64 = 0x10_C500;
    /// Offset of `mov rax,[rip+d32]` in that prologue, and the instruction's length.
    const GUARD_LOAD_OFF: u64 = 0x0E;
    /// Measured 2026-08-09: 8 steps without the redirect, 27 with it. The threshold is the "with"
    /// figure, so a partial redirect (guard pointer written but pointing at unmapped memory) lands
    /// below it and FAILS.
    const PAST_CANARY_STEPS: u64 = 27;

    let base = snap.base();
    let captured = match guard_load_target(c, NAME, snap, PROTECTED, GUARD_LOAD_OFF)? {
        Some(v) => v,
        None => return Ok(()),
    };

    // (a) Without the redirect: it must fault dereferencing the captured pointer itself.
    let mut off = RunSpec::new(base + PROTECTED);
    off.args = vec![m.scratch];
    off.step_limit = 100_000;
    off.canary = StackCanary::Off;
    let r_off = m.run(&off)?;

    // (b) With it: past the canary, and no longer stuck on that pointer.
    let mut auto = RunSpec::new(base + PROTECTED);
    auto.args = vec![m.scratch];
    auto.step_limit = 100_000;
    auto.canary = StackCanary::Auto;
    let r_auto = m.run(&auto)?;

    let off_at_guard = matches!(&r_off.exit, Exit::Fault { addr, .. } if *addr == captured);
    let auto_past = r_auto.steps >= PAST_CANARY_STEPS;
    let auto_elsewhere = !matches!(&r_auto.exit, Exit::Fault { addr, .. } if *addr == captured);
    // ⭐ `redirect_landed` is the READ-BACK, not `applied` (the plan). Deleting the `mem_write`
    // used to leave this term true and the row still printing "guard repointed at …".
    let redirected = r_auto.canary.redirect_landed();
    // ⭐ …and it must be OUR dedicated page. `Machine::canary` sits on a page of its own, past the
    // trampoline, precisely so no callee's buffer and no `reset()` can reach it. Pointing `auto` at
    // the scratch page instead satisfies every *execution* term above - measured 2026-08-10, the
    // row went green - so the structural claim needs its own term.
    let own_page = r_auto.canary.applied == Some(m.canary)
        && !(m.canary >= m.scratch && m.canary < m.stack + m.stack_len)
        && m.canary != m.stop;

    // ⭐ THE TARGETED ROLLBACK CHECK. `verify_pristine` samples 0x1000 of every 0x40000 and the
    // guard pointer lands at offset 0x6a78 of its window - outside the sample. Dropping the guard
    // write from the `dirty` journal left it reporting "sampled bytes match" while the redirect was
    // still live in memory. These are the eight bytes the control is actually about, so compare
    // them, byte for byte, and keep `verify_pristine` as the wider net it is.
    let after = m.u64(m.guard_ptr)?;
    let restored = after == captured;
    let (pristine, pristine_detail) = m.verify_pristine()?;

    let ok = off_at_guard && auto_past && auto_elsewhere && redirected && own_page && restored && pristine;
    push(
        c,
        NAME,
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "v:{PROTECTED:#x} (a stack-protected function): --stack-canary OFF ⇒ {} in {} step(s) \
 - the captured guard pointer {captured:#x} is host TLS and unmappable; AUTO ⇒ {} in \
             {} step(s) (need ≥{PAST_CANARY_STEPS}), guard pointer read back as {}. {} Rollback: \
             the guard's own 8 bytes read {after:#x} after the run (want {captured:#x}); {}",
            r_off.exit.short(),
            r_off.steps,
            r_auto.exit.short(),
            r_auto.steps,
            match r_auto.canary.effective {
                Some(p) => format!("{p:#x}"),
                None => "UNREADABLE".into(),
            },
            // ⚠ The verdict sentence is CONDITIONAL. A detail line that asserts the finding whether
            // or not the control passed is decoration, and it reads as a success in the FAIL row.
            if ok {
                "It EXECUTES PAST THE CANARY - it does NOT succeed, and the later fault is real."
                    .to_string()
            } else {
                format!(
                    "⛔ NOT past the canary: {}{}{}{}{}{}",
                    if off_at_guard { "" } else { "the OFF run did not fault on the guard pointer, so this snapshot does not exhibit the defect; " },
                    if redirected { "" } else { "the redirect DID NOT LAND IN MEMORY; " },
                    if own_page { "" } else { "the redirect does not point at the machine's own dedicated canary page; " },
                    if auto_past { "" } else { "the AUTO run stopped short of the threshold; " },
                    if auto_elsewhere { "" } else { "the AUTO run still faults on the guard pointer; " },
                    if restored { "" } else { "ROLLBACK LEFT THE GUARD POINTER REWRITTEN; " },
                )
            },
            if pristine { pristine_detail } else { format!("verify_pristine FAILED - {pristine_detail}") }
        ),
    );
    Ok(())
}

/// ⭐ The half the redirect control cannot reach: **the epilogue compare**.
///
/// `FUN_0010C500` faults 19 instructions after the canary, so it never executes
/// `cmp rcx,[rbp-8]` / `call __stack_chk_fail`. That leaves the whole *second* half of stack
/// protection untested - "the guard VALUE is stable enough that the function can return" is a
/// different claim from "the prologue load resolved", and only this one shows the redirect produced
/// a coherent canary rather than merely a mapped byte.
///
/// `FUN_003FE580` is the subject because it is the complete shape in fifteen instructions:
/// ```text
///   0x3fe588  mov  rcx,[rip+…]      ; -> [0x1a06c78]
///   0x3fe58f  mov  rax,[rcx]
///   0x3fe592  mov  [rbp-8],rax
///   0x3fe59e  cmp  qword [rdi],0    ; the entire body
///   0x3fe5a5  mov  rcx,[rcx]        ; re-read the guard
///   0x3fe5a8  cmp  rcx,[rbp-8]
///   0x3fe5ac  jne  0x3fe5b4         ; -> call __stack_chk_fail
///   0x3fe5b3  ret
/// ```
/// so OFF ⇒ fault at the captured pointer in 5 steps, AUTO ⇒ **RETURNED** in 15. A redirect that
/// pointed somewhere unstable would take the `jne` and never return.
fn canary_epilogue_control(c: &mut Vec<Control>, snap: &Snapshot, m: &mut Machine) -> Result<()> {
    use crate::machine::{Exit, StackCanary};

    const NAME: &str = "stack-canary epilogue round-trip";
    const PROTECTED: u64 = 0x3F_E580;
    const GUARD_LOAD_OFF: u64 = 0x08;
    /// The `call __stack_chk_fail` the epilogue takes when the compare disagrees. If the run ends
    /// up in here, the redirect produced an *incoherent* canary - the opposite of a pass.
    const STACK_CHK_FAIL: u64 = 0x129_D0B0;

    let base = snap.base();
    let captured = match guard_load_target(c, NAME, snap, PROTECTED, GUARD_LOAD_OFF)? {
        Some(v) => v,
        None => return Ok(()),
    };

    let mut off = RunSpec::new(base + PROTECTED);
    off.args = vec![m.scratch];
    off.step_limit = 100_000;
    off.canary = StackCanary::Off;
    let r_off = m.run(&off)?;

    let mut auto = RunSpec::new(base + PROTECTED);
    auto.args = vec![m.scratch];
    auto.step_limit = 100_000;
    auto.canary = StackCanary::Auto;
    auto.probes = vec![base + STACK_CHK_FAIL];
    let r_auto = m.run(&auto)?;

    let off_at_guard = matches!(&r_off.exit, Exit::Fault { addr, .. } if *addr == captured);
    let auto_returned = r_auto.exit.ok();
    let no_chk_fail = !r_auto.probes.contains_key(&(base + STACK_CHK_FAIL));
    let ok = off_at_guard && auto_returned && no_chk_fail;

    push(
        c,
        NAME,
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "v:{PROTECTED:#x} loads the guard, re-reads it and compares before `ret`: OFF ⇒ {} in \
             {} step(s); AUTO ⇒ {} in {} step(s). {}",
            r_off.exit.short(),
            r_off.steps,
            r_auto.exit.short(),
            r_auto.steps,
            if ok {
                format!(
                    "The prologue's value and the epilogue's re-read AGREE - the redirected guard \
                     is coherent across the call, and v:{STACK_CHK_FAIL:#x} (__stack_chk_fail) was \
                     never reached. ⚠ Still only 'it ran to completion'; the RESULT of a replay is \
                     evidence about inputs, never about the client's own moment."
                )
            } else {
                format!(
                    "⛔ {}{}{}",
                    if off_at_guard { "" } else { "the OFF run did not fault on the guard pointer, so this subject does not exhibit the defect; " },
                    if auto_returned { "" } else { "the AUTO run did not return, so the epilogue compare was not shown to agree; " },
                    if no_chk_fail { "" } else { "the run reached __stack_chk_fail - the redirected canary is NOT coherent across the call; " },
                )
            }
        ),
    );
    Ok(())
}

/// ⭐ INVERTED CONTROL for `auto`'s safety claim: *"it redirects only when the captured pointer is
/// unmapped, so it never alters memory that already worked."*
///
/// Every snapshot taken so far holds a host-TLS pointer there, so the "already mapped" branch is
/// never exercised by an ordinary run and the claim would ship untested. This plants a **mapped**
/// pointer in the guard global, tells the machine that is what the client had, and requires `auto`
/// to keep its hands off: no write, and the run must behave exactly as the planted pointer dictates.
///
/// It restores the global itself - the plant is deliberately made outside the `dirty` journal so
/// that `rollback` cannot mask a failure to keep hands off, which means this function owns the
/// cleanup and `verify_pristine` is asserted afterwards to prove it did it.
fn canary_auto_safety_control(c: &mut Vec<Control>, snap: &Snapshot, m: &mut Machine) -> Result<()> {
    use crate::machine::{StackCanary, GUARD_PTR_VADDR};

    const NAME: &str = "stack-canary auto safety (INVERTED)";
    /// A stack-protected function that RETURNS, so "auto left it alone" is visible as behaviour and
    /// not only as a byte compare.
    const PROTECTED: u64 = 0x3F_E580;

    let base = snap.base();
    if !snap.has(base + GUARD_PTR_VADDR, 8) || !snap.has(base + PROTECTED, 0x20) {
        push(c, NAME, Status::Skip, "the guard global or v:0x3fe580 is not in this snapshot".into());
        return Ok(());
    }
    let captured = snap.u64(base + GUARD_PTR_VADDR)?;

    // Plant a pointer that DOES resolve, and tell the machine the client had it. `m.canary` holds
    // CANARY_MAGIC and is re-planted by every `run()`, so the guard value is stable and the subject
    // returns - the same outcome `auto` would have produced, which is the point: the *outcome* is
    // not what is being asserted, the *absence of a write* is.
    let planted = m.canary;
    let saved_captured = m.guard_captured;
    m.write(m.guard_ptr, &planted.to_le_bytes())?;
    m.guard_captured = Some(planted);

    let mut spec = RunSpec::new(base + PROTECTED);
    spec.args = vec![m.scratch];
    spec.step_limit = 100_000;
    spec.canary = StackCanary::Auto;
    let r = m.run(&spec)?;

    let left_alone = r.canary.applied.is_none();
    let says_so = !r.canary.altered() && r.canary.captured_mapped;
    let bytes_intact = m.u64(m.guard_ptr)? == planted;
    let ran = r.exit.ok();

    // Undo the plant ourselves, then prove the machine is back to the client's bytes.
    m.write(m.guard_ptr, &captured.to_le_bytes())?;
    m.guard_captured = saved_captured;
    let cleaned = m.u64(m.guard_ptr)? == captured;
    let (pristine, pristine_detail) = m.verify_pristine()?;

    let ok = left_alone && says_so && bytes_intact && ran && cleaned && pristine;
    push(
        c,
        NAME,
        if ok { Status::Pass } else { Status::Fail },
        format!(
            "with a MAPPED pointer ({planted:#x}) planted in the guard global, auto reports \
             applied={:#x?} effective={:#x?} and v:{PROTECTED:#x} {} in {} step(s). {}",
            r.canary.applied,
            r.canary.effective,
            r.exit.short(),
            r.steps,
            if ok {
                "auto changed NOTHING and the client's own pointer was used - the 'only when \
                 unmapped' claim holds on the branch no snapshot exercises. Guard global restored \
                 to the captured value afterwards."
                    .to_string()
            } else {
                format!(
                    "⛔ {}{}{}{}{}{}",
                    if left_alone { "" } else { "AUTO WROTE THE GUARD POINTER even though it already resolved - every prior replay result is suspect; " },
                    if says_so { "" } else { "the report does not say the captured pointer was mapped-and-untouched; " },
                    if bytes_intact { "" } else { "the planted pointer did not survive the run; " },
                    if ran { "" } else { "the subject did not return, so the planted pointer was not actually used; " },
                    if cleaned { "" } else { "⛔⛔ THE CONTROL FAILED TO RESTORE THE GUARD GLOBAL - later controls on this machine are contaminated; " },
                    if pristine { String::new() } else { format!("verify_pristine after cleanup: {pristine_detail}") },
                )
            }
        ),
    );
    Ok(())
}

/// Re-derive the guard-pointer global from a subject function's own prologue, and return the
/// pointer value the client had there. `Ok(None)` means a row was already pushed (Skip or Fail) and
/// the caller should return.
///
/// ⚠ The modrm test accepts any `mov r64,[rip+d32]`: `FUN_0010C500` loads into rax (`48 8b 05`) and
/// `FUN_003FE580` into rcx (`48 8b 0d`). Pinning `0x05` would have made the second subject SKIP - 
/// silently, and a skipped control is a control nobody reads.
fn guard_load_target(
    c: &mut Vec<Control>,
    name: &'static str,
    snap: &Snapshot,
    func: u64,
    load_off: u64,
) -> Result<Option<u64>> {
    use crate::machine::GUARD_PTR_VADDR;
    const LEN: u64 = 7;

    let base = snap.base();
    if !snap.has(base + func, 0x20) {
        push(c, name, Status::Skip, format!("v:{func:#x} is not in this snapshot"));
        return Ok(None);
    }
    let ins = snap.read(base + func + load_off, LEN as usize)?;
    // REX.W + 8B /r with mod=00, rm=101 - the RIP-relative form, any destination register.
    if ins[0] != 0x48 || ins[1] != 0x8B || (ins[2] & 0xC7) != 0x05 {
        push(
            c,
            name,
            Status::Skip,
            format!(
                "v:{:#x} is not the expected `mov r64,[rip+d32]` guard load ({ins:02x?}) - refusing \
                 to use this function as the control rather than assuming",
                func + load_off
            ),
        );
        return Ok(None);
    }
    let disp = i32::from_le_bytes(ins[3..7].try_into().unwrap()) as i64;
    let derived = (func as i64 + load_off as i64 + LEN as i64 + disp) as u64;
    if derived != GUARD_PTR_VADDR {
        push(
            c,
            name,
            Status::Fail,
            format!(
                "v:{func:#x}'s own prologue loads the guard pointer from v:{derived:#x}, but \
                 GUARD_PTR_VADDR is v:{GUARD_PTR_VADDR:#x} - the constant has DRIFTED and the \
                 redirect is writing to the wrong global"
            ),
        );
        return Ok(None);
    }
    if !snap.has(base + GUARD_PTR_VADDR, 8) {
        push(
            c,
            name,
            Status::Skip,
            format!("the guard global v:{GUARD_PTR_VADDR:#x} is not in this snapshot"),
        );
        return Ok(None);
    }
    Ok(Some(snap.u64(base + GUARD_PTR_VADDR)?))
}

fn registry_cross_check(snap: &Snapshot, syms: &Symbols) -> Result<(usize, usize, String)> {
    let base = snap.base();
    let Some(reg_ptr) = syms.lookup("object_registry_ptr") else {
        return Ok((0, 0, "guest-symbols.json has no object_registry_ptr".into()));
    };
    let reg = match snap.u64(base + reg_ptr) {
        Ok(0) => {
            return Ok((0, 0, "object_registry_ptr is NULL in this snapshot - the client had not built the registry".into()))
        }
        Ok(v) => v,
        Err(e) => return Ok((0, 0, format!("object_registry_ptr not captured: {e}"))),
    };
    if !snap.has(reg, 8) {
        return Ok((
            0,
            0,
            format!(
                "the registry heap at {reg:#x} was not captured - re-run capture with \
                 `--follow object_registry_ptr:0x60000` (this is a SELECTION gap, not a result)"
            ),
        ));
    }
    let eb = syms.layout.get("entry_table_base").copied().unwrap_or(0x4F800);
    let sf = syms.layout.get("entry_stride_family").copied().unwrap_or(0x60);
    let st = syms.layout.get("entry_stride_type").copied().unwrap_or(0x10);
    let elb = syms.layout.get("element_base").copied().unwrap_or(0x40A0);
    let (mut agree, mut total) = (0usize, 0usize);
    let mut bad = Vec::new();
    for f in &syms.families {
        let ent = reg + f.family * sf + f.typ * st + eb;
        let Ok(off) = snap.u32(ent) else { continue };
        if off == 0xFFFF_FFFF {
            continue;
        }
        let Ok(cap) = snap.u16(ent + 4) else { continue };
        let Ok(stride) = snap.u32(ent + 8) else { continue };
        total += 1;
        let rt = reg + elb + off as u64;
        let stt = reg + f.storage;
        if rt == stt && cap as u64 == f.cap && stride as u64 == f.size {
            agree += 1;
        } else {
            bad.push(format!(
                "fam{}/t{}: runtime {rt:#x} cap={cap} stride={stride:#x} vs static {stt:#x} cap={} stride={:#x}",
                f.family, f.typ, f.cap, f.size
            ));
        }
    }
    let detail = if bad.is_empty() { String::new() } else { format!("; mismatches: {}", bad.join(" | ")) };
    Ok((agree, total, detail))
}
