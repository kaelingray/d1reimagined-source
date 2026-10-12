// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐⭐ `capture --snapshot-on-hit` - **the capture at the stop.**
//!
//! A watchpoint hands back registers with no memory; a snapshot hands back memory with no
//! registers. This module welds them: at a hardware-watchpoint trap, with the trapping thread
//! halted, read a deliberately small **pre-rehearsed** set of memory, then resume and read the rest.
//! The spec is `historicaldocs/design/live-watch.md` §6a and every number below comes from it.
//!
//! # ⛔ The safety argument, because it is the reason this is allowed to exist at all
//!
//! `d1-live`'s governing rule is *a tracer that dies armed kills its target*, measured `[C]` and
//! re-measured by its own controls on every run. This module adds a **multi-millisecond** operation
//! to a tool built around that rule - so the operation was moved **outside** the armed window rather
//! than made safe inside it:
//!
//! 1. the trap fires; `d1-live` builds the [`Hit`](d1_live::watch::Hit) (~10 us, armed);
//! 2. ⭐ [`Watcher::catch_one`](d1_live::watch::Watcher::catch_one) clears `DR0..DR3`/`DR6`/`DR7` on
//!    **every** armed thread and **reads `DR7` back as zero** (0.68 ms for 111 threads, measured);
//! 3. *everything in this module runs here*, with **nothing armed anywhere**;
//! 4. the thread is resumed and detached by a `Drop` guard - on success, on `?`, and on panic.
//!
//! ⇒ a crash during step 3 leaves a tracee the kernel auto-detaches, which resumes.
//! `d1-live controls`' `panic-during-capture-is-survivable` panics on purpose in the middle of step 3
//! and requires the child to be alive, **unarmed** and still storing; its inversion (panic while
//! armed) kills the child, which is how we know the axis is real.
//!
//! # ⚠⚠ What the artifact does NOT claim
//!
//! **This is not an atomic freeze.** One thread stops; the other ~110 keep running. The guarantee is
//! narrow and worth stating exactly: *the trapping thread executed no instructions between its
//! registers being read and the in-stop bytes being read.* Nothing else. Every number a reader needs
//! to see that for themselves - which threads were stopped, when each phase started and ended, which
//! regions are the trap's instant and which are later - is written into
//! [`Coherence`](crate::snapshot::Coherence), in the file.

use std::time::Instant;

use anyhow::{bail, Context, Result};

use crate::live::Proc;
use crate::snapshot::PAGE;

/// Chunk size for every read in this module. ⭐ Measured: 47.3 MiB in 1 MiB chunks is 8.15 ms
/// (5,886 MiB/s); the same bytes in one `pread` into a fresh `Vec` is 16.48 ms - **2.0x slower**,
/// because the allocation faults and zeroes inside the timed window. The buffer here is allocated
/// once, before anything is armed.
pub const CHUNK: usize = 1 << 20;

pub const DEFAULT_BUDGET_BYTES: u64 = 0x400_0000; // 64 MiB
pub const DEFAULT_BUDGET_MS: f64 = 16.0; // one 60 fps frame
pub const DEFAULT_HARD_CAP_BYTES: u64 = 0x2000_0000; // 512 MiB
pub const DEFAULT_HARD_CAP_MS: f64 = 250.0; // 1.25% of the client's 20,000 ms block detector

/// The System V integer argument registers plus the two callee-saved ones that most often carry
/// `this`. ⭐ These are the arguments the client actually passed, and they can ONLY be resolved at
/// the trap - which is the entire reason this pairing exists.
pub const DEFAULT_REG_ROOTS: [&str; 8] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "rbx", "rbp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopScope {
    /// Watched page + register roots + stack. ~0.1 MiB, ~0.75 ms - almost all of it the disarm.
    Minimal,
    /// `minimal` + the eboot's writable segments (data + BSS): every singleton, the handler ladder,
    /// the state machine. **This IS the state.** ~22.7 MiB, ~4.6 ms.
    State,
    /// `state` + whatever `--region` / `--follow` / `--preset` resolved. The operator's problem,
    /// bounded below by the hard cap.
    Wide,
}

impl StopScope {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "minimal" => StopScope::Minimal,
            "state" => StopScope::State,
            "wide" => StopScope::Wide,
            other => bail!("unknown --stop-scope {other:?} - use minimal | state | wide"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            StopScope::Minimal => "minimal",
            StopScope::State => "state",
            StopScope::Wide => "wide",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Opts {
    pub scope: StopScope,
    pub budget_bytes: u64,
    pub budget_ms: f64,
    pub hard_cap_bytes: u64,
    pub hard_cap_ms: f64,
    pub reg_roots: Vec<String>,
    pub reg_window: u64,
    pub stack_window: u64,
    /// `false` = `--snapshot-when resumed`: take the registers, resume immediately, read everything
    /// afterwards. ⚠ Not the default, and the file always says which it got - see §6a.5.
    pub when_stopped: bool,
    pub phase_windows: usize,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            scope: StopScope::State,
            budget_bytes: DEFAULT_BUDGET_BYTES,
            budget_ms: DEFAULT_BUDGET_MS,
            hard_cap_bytes: DEFAULT_HARD_CAP_BYTES,
            hard_cap_ms: DEFAULT_HARD_CAP_MS,
            reg_roots: DEFAULT_REG_ROOTS.iter().map(|s| s.to_string()).collect(),
            reg_window: PAGE,
            stack_window: 0x1_0000,
            when_stopped: true,
            phase_windows: 32,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The plan, the rehearsal, and the refusal
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// What will be read inside the stop, what it is predicted to cost, and - the part that matters -
/// **what was actually measured reading it from this client, before anything was armed.**
#[derive(Debug, Clone)]
pub struct Plan {
    pub scope: StopScope,
    /// Page-aligned, mapping-clipped, non-overlapping. Known before the arm.
    pub fixed: Vec<(u64, u64, String)>,
    pub fixed_bytes: u64,
    /// The upper bound on regions that can only be resolved AT the hit (register roots, the stack).
    pub dynamic_allowance: u64,
    pub rehearsed_bytes: u64,
    pub rehearsed_ms: f64,
    pub predicted_bytes: u64,
    pub predicted_ms: f64,
    /// Absolute spans of the eboot's executable segments - read AFTER the resume, and spot-checked
    /// across the boundary by the phase-split control.
    pub seg_exec: Vec<(u64, u64)>,
    /// Absolute spans of the eboot's writable segments.
    pub seg_data: Vec<(u64, u64)>,
    pub notes: Vec<String>,
}

/// Walk `[addr, addr+len)` mapping by mapping, clipping to what is readable and resident. A range
/// that crosses a hole is split rather than dropped - shadPS4 maps text and data separately, and a
/// single span across the gap would lose the whole segment.
fn clip_to_mappings(proc: &Proc, addr: u64, len: u64, origin: &str, out: &mut Vec<(u64, u64, String)>, notes: &mut Vec<String>) {
    let lo = addr & !(PAGE - 1);
    let hi = (addr + len).div_ceil(PAGE) * PAGE;
    let mut cur = lo;
    while cur < hi {
        match proc.mapping_of(cur) {
            Some(m) if m.readable() => {
                let take = (hi - cur).min(m.end - cur);
                out.push((cur, take, origin.to_string()));
                cur += take;
            }
            Some(m) => {
                notes.push(format!("{origin}: {cur:#x} is in a non-readable mapping ({})", m.perms));
                cur = m.end.min(hi);
            }
            None => {
                let next = proc
                    .mappings()
                    .iter()
                    .filter(|m| m.start > cur)
                    .map(|m| m.start)
                    .min()
                    .unwrap_or(hi);
                notes.push(format!("{origin}: unmapped hole {cur:#x}..{:#x}", next.min(hi)));
                cur = next.min(hi);
            }
        }
    }
}

/// Merge overlapping/adjacent ranges so a byte is never read - or stored - twice.
fn merge(mut v: Vec<(u64, u64, String)>) -> Vec<(u64, u64, String)> {
    v.sort_by_key(|(a, _, _)| *a);
    let mut out: Vec<(u64, u64, String)> = Vec::new();
    for (a, l, o) in v {
        if let Some(last) = out.last_mut() {
            if a <= last.0 + last.1 {
                let end = (last.0 + last.1).max(a + l);
                last.1 = end - last.0;
                if !last.2.contains(&o) {
                    last.2 = format!("{}; {}", last.2, o);
                }
                continue;
            }
        }
        out.push((a, l, o));
    }
    out
}

/// Build the in-stop plan. `wide_extra` is the already-resolved `--region`/`--preset` set, used only
/// under `--stop-scope wide`.
pub fn plan(
    proc: &Proc,
    image: &d1_eboot::Image,
    base: u64,
    watch_addr: u64,
    opts: &Opts,
    wide_extra: &[(u64, u64, String)],
) -> Result<Plan> {
    let mut fixed = Vec::new();
    let mut notes = Vec::new();
    let mut seg_exec = Vec::new();
    let mut seg_data = Vec::new();

    for s in image.load_segments() {
        let span = s.memsz.max(s.filesz);
        if span == 0 {
            continue;
        }
        let abs = (base + s.vaddr, base + s.vaddr + span);
        if s.is_executable() {
            seg_exec.push(abs);
        } else {
            seg_data.push(abs);
        }
    }

    // The thing the trap is about.
    clip_to_mappings(proc, watch_addr, PAGE, "in-stop: the watched page", &mut fixed, &mut notes);

    if opts.scope != StopScope::Minimal {
        // ⭐ The eboot's writable segments: data + BSS. Every singleton, the handler ladder, the
        // state machine. The executable segment is deliberately NOT here - it does not change during
        // a run, so it is read after the resume, which is what keeps the default under one frame.
        // ⚠ That is an assumption (a `-p/--patch` XML rewrites code at load; the UD2 tracer writes
        // `0f 0b` over probe sites), so it is not asserted - the phase-split control CHECKS it,
        // every run.
        for (i, (lo, hi)) in seg_data.iter().enumerate() {
            clip_to_mappings(proc, *lo, hi - lo, &format!("in-stop: eboot data segment {i}"), &mut fixed, &mut notes);
        }
    }
    if opts.scope == StopScope::Wide {
        for (a, l, o) in wide_extra {
            clip_to_mappings(proc, *a, *l, &format!("in-stop (wide): {o}"), &mut fixed, &mut notes);
        }
    }

    let fixed = merge(fixed);
    let fixed_bytes: u64 = fixed.iter().map(|(_, l, _)| *l).sum();
    let dynamic_allowance = opts.reg_roots.len() as u64 * opts.reg_window + opts.stack_window;

    // ── THE REHEARSAL ───────────────────────────────────────────────────────────────────────────
    // ⭐ Read exactly these regions, from exactly this client, and time it. The budget is enforced
    // against a MEASURED rate and never an assumed one.
    // ⚠⚠ IT REHEARSES `fixed` ONLY. The register-root windows and the stack can only be resolved AT
    // the hit, so they are NOT proven readable here and their cost is NOT in `rehearsed_ms` - a
    // non-resident stack page faulting in from disk is an unmeasured in-stop cost. An earlier
    // version of this comment claimed the rehearsal proved "every in-stop region" readable, which
    // was false for precisely the regions that are the point of the pairing. `dynamic_allowance`
    // bounds their SIZE; nothing bounds their latency. ⇒ they are read with the deadline check and
    // every failure lands in `dropped`, which is the honest mitigation.
    let mut scratch = vec![0u8; CHUNK];
    let t0 = Instant::now();
    let mut rehearsed_bytes = 0u64;
    for (a, l, o) in &fixed {
        let mut off = 0u64;
        while off < *l {
            let n = ((*l - off) as usize).min(CHUNK);
            proc.read_into(a + off, &mut scratch[..n])
                .with_context(|| format!("rehearsing the in-stop read of {o} at {:#x}", a + off))?;
            off += n as u64;
            rehearsed_bytes += n as u64;
        }
    }
    let rehearsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let predicted_bytes = fixed_bytes + dynamic_allowance;
    // A floor on the rate keeps a degenerate rehearsal (0 bytes, or a clock that did not tick) from
    // predicting zero milliseconds and waving a huge capture through.
    let rate = if rehearsed_ms > 0.0 && rehearsed_bytes > 0 {
        rehearsed_bytes as f64 / rehearsed_ms
    } else {
        f64::MAX
    };
    let predicted_ms = if rate == f64::MAX { 0.0 } else { predicted_bytes as f64 / rate };

    Ok(Plan {
        scope: opts.scope,
        fixed,
        fixed_bytes,
        dynamic_allowance,
        rehearsed_bytes,
        rehearsed_ms,
        predicted_bytes,
        predicted_ms,
        seg_exec,
        seg_data,
        notes,
    })
}

/// The verdict of the pre-arm budget check. ⭐ **Pure** - no client, no ptrace - so it has a control
/// that runs offline, and so the refusal cannot depend on anything that only exists at a hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Inside the budget. Arm.
    Ok,
    /// Over `--stop-budget-*` but under the hard cap: allowed, said out loud, and the in-stop
    /// deadline will cut the read short rather than let it run long.
    OverBudget(String),
    /// ⛔ Over `--stop-hard-cap-*`. **Refuse, before arming** - a refusal at hit time burns the
    /// trigger, and this project has already paid for that exact mistake once (§2.2a).
    Refuse(String),
}

pub fn verdict(bytes: u64, ms: f64, opts: &Opts) -> Verdict {
    if bytes > opts.hard_cap_bytes || ms > opts.hard_cap_ms {
        return Verdict::Refuse(format!(
            "the in-stop set is {bytes:#x} bytes ({:.2} MiB), rehearsed at {ms:.1} ms, which exceeds \
             --stop-hard-cap-bytes ({:.0} MiB) or --stop-hard-cap-ms ({:.0}). Nothing has been armed \
             and no trigger has been spent. Either narrow --stop-scope, or pass \
             `--snapshot-when resumed` and accept a LATER memory image (historicaldocs/design/live-watch.md §6a.5)",
            bytes as f64 / (1u64 << 20) as f64,
            opts.hard_cap_bytes as f64 / (1u64 << 20) as f64,
            opts.hard_cap_ms
        ));
    }
    if bytes > opts.budget_bytes || ms > opts.budget_ms {
        return Verdict::OverBudget(format!(
            "the in-stop set is {bytes:#x} bytes ({:.2} MiB) at a rehearsed {ms:.1} ms, over \
             --stop-budget-bytes ({:.0} MiB) / --stop-budget-ms ({:.0}). Allowed, but the in-stop \
             deadline will CUT THE READ SHORT at {:.0} ms and every dropped region will be in \
             `skipped` with that reason",
            bytes as f64 / (1u64 << 20) as f64,
            opts.budget_bytes as f64 / (1u64 << 20) as f64,
            opts.budget_ms,
            opts.budget_ms
        ));
    }
    Verdict::Ok
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The in-stop read
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// FNV-1a, 64-bit. Cheap enough to run on 4 KiB windows inside the stop; it is a change detector,
/// not a cryptographic claim.
fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The spot windows the phase-split control compares across the resume boundary.
#[derive(Debug, Clone, Default)]
pub struct PhaseSample {
    pub exec: Vec<(u64, u64)>, // (addr, hash)
    pub data: Vec<(u64, u64)>,
}

fn sample_windows(proc: &Proc, spans: &[(u64, u64)], n: usize, buf: &mut [u8]) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    if spans.is_empty() || n == 0 {
        return out;
    }
    let total: u64 = spans.iter().map(|(a, b)| b - a).sum();
    if total == 0 {
        return out;
    }
    let step = (total / n as u64).max(PAGE);
    let mut want = 0u64;
    while want < total && out.len() < n {
        // Map the linear offset back into the span list.
        let mut rem = want;
        for (lo, hi) in spans {
            let len = hi - lo;
            if rem < len {
                let a = (lo + rem) & !(PAGE - 1);
                if proc.mapping_of(a).map(|m| m.readable()).unwrap_or(false)
                    && proc.read_into(a, &mut buf[..PAGE as usize]).is_ok()
                {
                    out.push((a, fnv1a(&buf[..PAGE as usize])));
                }
                break;
            }
            rem -= len;
        }
        want += step;
    }
    out
}

/// What the in-stop phase actually managed to read. ⚠ `dropped` is never silent: every entry also
/// becomes a `Manifest::skipped` row with the deadline as its reason.
pub struct InStopResult {
    /// `(addr, offset into the buffer, len, origin)`.
    pub filled: Vec<(u64, usize, u64, String)>,
    pub dropped: Vec<(u64, u64, String)>,
    pub bytes: u64,
    pub start_ns: u64,
    pub end_ns: u64,
    pub deadline_fired: bool,
    pub phase: PhaseSample,
    pub dynamic: Vec<(u64, u64, String)>,
}

/// Page-align a pointer-ish register value into a capture window, if it is mapped.
fn reg_window(proc: &Proc, v: u64, len: u64) -> Option<(u64, u64)> {
    if v < PAGE {
        return None;
    }
    let lo = v & !(PAGE - 1);
    let m = proc.mapping_of(lo)?;
    if !m.readable() {
        return None;
    }
    Some((lo, len.min(m.end - lo)))
}

/// ⭐ THE IN-STOP READ. Everything in here runs with the trapping thread halted and **nothing
/// armed** - see the module header for why that ordering is the whole safety argument.
///
/// `buf` is preallocated by the caller **before the arm**: no allocation happens inside the window,
/// which also keeps Rust's allocation-failure `abort()` - which does not unwind, so no `Drop` runs -
/// out of it entirely.
#[allow(clippy::too_many_arguments)]
pub fn read_in_stop(
    proc: &Proc,
    plan: &Plan,
    opts: &Opts,
    regs: &std::collections::BTreeMap<String, u64>,
    buf: &mut [u8],
    trap_unix_nanos: u64,
    scratch: &mut [u8],
) -> InStopResult {
    let start_ns = d1_live::watch::unix_nanos().saturating_sub(trap_unix_nanos);
    let t0 = Instant::now();
    let deadline_ms = opts.budget_ms;

    // ── the regions only a trap can resolve ─────────────────────────────────────────────────────
    let mut dynamic: Vec<(u64, u64, String)> = Vec::new();
    let mut spent = 0u64;
    for name in &opts.reg_roots {
        let Some(v) = regs.get(name).copied() else { continue };
        if spent + opts.reg_window > plan.dynamic_allowance {
            break;
        }
        if let Some((lo, len)) = reg_window(proc, v, opts.reg_window) {
            dynamic.push((lo, len, format!("in-stop: {name}={v:#x} (an argument the client passed)")));
            spent += len;
        }
    }
    if let Some(rsp) = regs.get("rsp").copied() {
        // ⚠ The frame's own memory is at and ABOVE rsp; the 128-byte red zone is just below it. One
        // page below and the rest above covers both, and it is clipped to the stack's own mapping.
        let lo = (rsp.saturating_sub(PAGE)) & !(PAGE - 1);
        let len = opts.stack_window.min(plan.dynamic_allowance.saturating_sub(spent));
        // len == 0 means the register roots ate the whole allowance; a zero-length region would be
        // a row in the table that holds nothing, which is worse than no row at all.
        if len > 0 {
            if let Some((lo, len)) = reg_window(proc, lo, len) {
                dynamic.push((lo, len, format!("in-stop: the stack around rsp={rsp:#x}")));
            }
        }
    }

    let mut all = plan.fixed.clone();
    all.extend(dynamic.iter().cloned());
    let all = merge(all);

    // ── the read ────────────────────────────────────────────────────────────────────────────────
    let mut filled = Vec::new();
    let mut dropped = Vec::new();
    let mut off = 0usize;
    let mut deadline_fired = false;
    for (a, l, o) in &all {
        if deadline_fired || off as u64 + *l > buf.len() as u64 {
            dropped.push((
                *a,
                *l,
                if deadline_fired {
                    format!("{o}: DROPPED - the in-stop deadline of {deadline_ms:.0} ms fired")
                } else {
                    format!("{o}: DROPPED - the preallocated in-stop buffer is full")
                },
            ));
            continue;
        }
        let region_off = off;
        let mut done = 0u64;
        let mut failed: Option<String> = None;
        while done < *l {
            if t0.elapsed().as_secs_f64() * 1000.0 > deadline_ms {
                deadline_fired = true;
                break;
            }
            let n = ((*l - done) as usize).min(CHUNK);
            if let Err(e) = proc.read_into(a + done, &mut buf[off..off + n]) {
                failed = Some(format!("{e:#}"));
                break;
            }
            done += n as u64;
            off += n;
        }
        if let Some(e) = failed {
            // The rehearsal already read every fixed region, so a failure here is a real change in
            // the client, not a bad selection. It is recorded, never swallowed.
            dropped.push((*a, *l, format!("{o}: unreadable inside the stop - {e}")));
            off = region_off;
            continue;
        }
        if done < *l {
            dropped.push((
                *a + done,
                *l - done,
                format!("{o}: DROPPED at +{done:#x} - the in-stop deadline of {deadline_ms:.0} ms fired"),
            ));
        }
        if done > 0 {
            filled.push((*a, region_off, done, o.clone()));
        }
    }

    // ── the phase-split samples, taken IN the stop ──────────────────────────────────────────────
    let phase = if deadline_fired {
        PhaseSample::default()
    } else {
        PhaseSample {
            exec: sample_windows(proc, &plan.seg_exec, opts.phase_windows, scratch),
            data: sample_windows(proc, &plan.seg_data, opts.phase_windows, scratch),
        }
    };

    let bytes = filled.iter().map(|(_, _, l, _)| *l).sum();
    InStopResult {
        filled,
        dropped,
        bytes,
        start_ns,
        end_ns: d1_live::watch::unix_nanos().saturating_sub(trap_unix_nanos),
        deadline_fired,
        phase,
        dynamic,
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The phase-split control
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐ Score the phase split. **It is its own pair**: the executable segment's windows must be
/// UNCHANGED across the resume (which is what justifies reading it after the resume at all) *and*
/// the data segment's windows must have CHANGED (which is what proves the comparison could have
/// detected a change). If the data side did not move, the verdict is **DEGENERATE**, not PASS - a
/// scan that finds nothing paired with one that must find something.
pub fn score_phase_split(
    exec_windows: usize,
    exec_changed: usize,
    data_windows: usize,
    data_changed: usize,
) -> String {
    if exec_windows == 0 {
        return "DEGENERATE: no executable-segment windows were sampled, so nothing was compared"
            .to_string();
    }
    if exec_changed > 0 {
        return format!(
            "FAIL: {exec_changed} of {exec_windows} executable-segment window(s) CHANGED between the \
             stop and the resume - code is not stable across the phase boundary here, so reading it \
             after the resume fabricates a mixed instant. Re-capture with --stop-scope wide, or treat \
             those bytes as a LATER instant"
        );
    }
    if data_changed == 0 {
        return format!(
            "DEGENERATE: {exec_windows} executable-segment window(s) were unchanged, but so were all \
             {data_windows} data-segment window(s) - nothing moved during the capture, so 'unchanged' \
             proves nothing. The comparison had no axis this time"
        );
    }
    format!(
        "PASS: {exec_windows} executable-segment window(s) unchanged across the resume while \
         {data_changed} of {data_windows} data-segment window(s) DID change - the code the snapshot \
         reads after the resume is the code it had in the stop, and the comparison demonstrably \
         detects change"
    )
}

/// Re-read the sampled windows after the resume and count how many moved.
pub fn recheck_phase(proc: &Proc, phase: &PhaseSample, buf: &mut [u8]) -> (usize, usize, usize, usize) {
    let count = |ws: &Vec<(u64, u64)>, buf: &mut [u8]| -> (usize, usize) {
        let mut changed = 0;
        let mut seen = 0;
        for (a, h) in ws {
            if proc.read_into(*a, &mut buf[..PAGE as usize]).is_ok() {
                seen += 1;
                if fnv1a(&buf[..PAGE as usize]) != *h {
                    changed += 1;
                }
            }
        }
        (seen, changed)
    };
    let (ew, ec) = count(&phase.exec, buf);
    let (dw, dc) = count(&phase.data, buf);
    (ew, ec, dw, dc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ The axis: **the hard cap REFUSES and the budget does not.** Inverted in the same test by
    /// asserting the small plan is `Ok` - a threshold that refused everything would fail here, and
    /// one that refused nothing would fail on the first assertion.
    #[test]
    fn the_hard_cap_refuses_and_the_budget_only_warns() {
        let o = Opts::default();
        // The spec's own worked example: `--stop-scope wide --preset content`, 586 MiB at 102 ms.
        let big = 586 * (1 << 20);
        assert!(matches!(verdict(big, 102.0, &o), Verdict::Refuse(_)));
        // Over the millisecond cap alone is enough - bytes and time are separate walls.
        assert!(matches!(verdict(1 << 20, 260.0, &o), Verdict::Refuse(_)));
        // Over budget but under the cap: allowed, loudly.
        assert!(matches!(verdict(100 << 20, 20.0, &o), Verdict::OverBudget(_)));
        // ⭐ The default in-stop set: 22.7 MiB at 4.6 ms must sail through, or the tool refuses its
        // own documented default and nobody can capture anything.
        assert_eq!(verdict(23_800_000, 4.6, &o), Verdict::Ok);
    }

    /// ⭐ The axis: a "nothing changed" reading is only evidence when the other half of the split
    /// DID change. Every branch is asserted, including the two that must never silently pass.
    #[test]
    fn the_phase_split_is_its_own_pair() {
        assert!(score_phase_split(32, 0, 32, 7).starts_with("PASS"));
        // Code moved: the whole after-the-resume read is a mixed instant.
        assert!(score_phase_split(32, 1, 32, 7).starts_with("FAIL"));
        // Nothing moved anywhere: the comparison had no axis.
        assert!(score_phase_split(32, 0, 32, 0).starts_with("DEGENERATE"));
        // Nothing sampled: not a pass.
        assert!(score_phase_split(0, 0, 32, 7).starts_with("DEGENERATE"));
    }

    /// The scope names round-trip, and an unknown one is refused rather than defaulted - a silent
    /// default here would capture 22.6 MiB inside a stop the operator asked to keep minimal.
    #[test]
    fn scopes_parse_or_refuse() {
        assert_eq!(StopScope::parse("minimal").unwrap(), StopScope::Minimal);
        assert_eq!(StopScope::parse("state").unwrap(), StopScope::State);
        assert_eq!(StopScope::parse("wide").unwrap(), StopScope::Wide);
        assert!(StopScope::parse("everything").is_err());
    }
}
