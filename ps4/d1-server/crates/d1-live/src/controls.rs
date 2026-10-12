// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The mandatory controls. **Offline** - they never touch the emulator; every one of them drives a
//! throwaway child this crate spawns itself.
//!
//! # The axis each control protects, stated in words
//!
//! | control | the axis | how it is inverted |
//! |---|---|---|
//! | `debugreg-offset` | *`u_debugreg` is at the offset we think.* A wrong offset makes `POKEUSER` fail, or worse, land in `u_comm`. | The measurement is a **boundary**: the word below and the word above must be REJECTED. A kernel that accepted everything fails. |
//! | `catches-the-writer` | *A write watchpoint catches the EXACT instruction that wrote the chosen address.* | The child reports its own store's address from inline asm; the caught `RIP` must equal the byte **after** it, and the decoded writer must equal it. An off-by-one instrument fails. |
//! | `address-specific` | *The watchpoint is about the ADDRESS, not "traps always fire".* | Arm 16 bytes away, on a live child that is storing millions of times a second, and require **no trap**. Then, **without restarting the child**, re-arm on the real target and require a trap - so "no trap" can never be explained by a dead or idle child. |
//! | `registers-at-the-hit` | *The register map on a `Hit` is the trapping thread's LIVE values*, not a zeroed struct and not a fabrication. | Completeness against `ptrace::REG_NAMES`; `regs["rip"]` anchored to the address the **child itself** printed; the store's pointer operand must still be live in a general register, with a spread check so a constant fill fails. The decoy leg is scored **DEGENERATE**, never as a free pass. |
//! | `disarm-is-load-bearing` | *`Watcher`'s disarm really removes the breakpoint.* | A helper arms and then **abandons** the target. If the abandoned child dies, disarming matters - and the real `Watcher` must then take a hit and still leave its child alive. If the abandoned child *lives*, the control says the inversion was degenerate instead of banking a free pass. |
//!
//! ⛔ The one thing these controls cannot prove is that shadPS4's guest threads are watchable in
//! the same way - that is `[U]`, and the instrument that closes it is a single `d1-live watch` run
//! against the live client (`historicaldocs/design/live-watch.md` §6).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::proc::Proc;
use crate::ptrace::{self, Rw};
use crate::watch::{WatchSpec, Watcher};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Control {
    pub name: String,
    pub pass: bool,
    pub axis: String,
    pub detail: String,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Controls {
    pub lines: Vec<Control>,
}

impl Controls {
    fn push(&mut self, name: &str, axis: &str, r: Result<String>) {
        let (pass, detail) = match r {
            Ok(d) => (true, d),
            Err(e) => (false, format!("{e:#}")),
        };
        self.lines.push(Control { name: name.into(), pass, axis: axis.into(), detail });
    }
    pub fn ok(&self) -> bool {
        !self.lines.is_empty() && self.lines.iter().all(|l| l.pass)
    }
    pub fn report(&self) -> String {
        let mut s = format!(
            "d1-live controls: {} ({}/{} pass)",
            if self.ok() { "PASS" } else { "FAIL" },
            self.lines.iter().filter(|l| l.pass).count(),
            self.lines.len()
        );
        for l in &self.lines {
            s.push_str(&format!(
                "\n  {}  {}\n        axis: {}\n        {}",
                if l.pass { "PASS" } else { "FAIL" },
                l.name,
                l.axis,
                l.detail
            ));
        }
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The throwaway child
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// What `__writer` prints on its first line: two 8-byte cells 16 bytes apart, and the address of
/// the single `mov [reg], reg` that writes the first one, plus the address of the instruction after
/// it. **The child measures its own writer**; nothing here is transcribed from a disassembly.
#[derive(Debug, Clone, Copy)]
pub struct WriterChild {
    pub pid: i32,
    pub target: u64,
    pub decoy: u64,
    pub store: u64,
    pub next: u64,
}

pub struct Spawned {
    pub info: WriterChild,
    child: Child,
}

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `helper` is the path to this crate's own binary - `current_exe()` at runtime,
/// `env!("CARGO_BIN_EXE_d1-live")` under the test harness.
pub fn spawn_writer(helper: &Path, seconds: f64) -> Result<Spawned> {
    spawn_writer_threads(helper, seconds, 0)
}

/// [`spawn_writer`] with `extra` additional spinner threads. ⭐ The one-thread child is not the
/// shape this tool meets: shadPS4 has ~111 threads, and until this existed **no control exercised a
/// multi-thread disarm at all** - which is exactly how a 7/7-green tool turns out to be 4/11.
pub fn spawn_writer_threads(helper: &Path, seconds: f64, extra: usize) -> Result<Spawned> {
    let mut child = Command::new(helper)
        .arg("__writer")
        .arg("--seconds")
        .arg(seconds.to_string())
        .arg("--threads")
        .arg(extra.to_string())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning the control child {}", helper.display()))?;
    let stdout = child.stdout.take().expect("piped");
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).context("reading the control child's banner")?;
    let mut f = std::collections::HashMap::new();
    let toks: Vec<&str> = line.split_whitespace().collect();
    for pair in toks.chunks(2) {
        if pair.len() == 2 {
            if let Ok(v) = u64::from_str_radix(pair[1].trim_start_matches("0x"), 16) {
                f.insert(pair[0].to_string(), v);
            }
        }
    }
    let get = |k: &str| -> Result<u64> {
        f.get(k).copied().ok_or_else(|| anyhow::anyhow!("control child banner had no {k}: {line:?}"))
    };
    let info = WriterChild {
        pid: child.id() as i32,
        target: get("TARGET")?,
        decoy: get("DECOY")?,
        store: get("STORE")?,
        next: get("NEXT")?,
    };
    Ok(Spawned { info, child })
}

fn watch_once(info: &WriterChild, addr: u64, dbg: u64, max: Duration) -> Result<crate::watch::WatchReport> {
    let proc = Proc::open(info.pid)?;
    let spec =
        WatchSpec { addr, len: 8, rw: Rw::Write, slot: 0, follow_clone: false, base: None, tids: None };
    let mut w = Watcher::attach(info.pid, spec, dbg)?;
    let report = w.run(&proc, 1, max, |_| {})?;
    let failed = w.disarm();
    if !failed.is_empty() {
        bail!("disarm did not verify: {}", failed.join("; "));
    }
    Ok(report)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Measure `offsetof(struct user, u_debugreg)` against our own stopped child and require it to
/// agree with the compiled-in constant.
pub fn debugreg_offset(helper: &Path) -> Result<(u64, String)> {
    let s = spawn_writer(helper, 10.0)?;
    debugreg_offset_against(s.info.pid)
}

/// The same measurement against a stopped child **the caller owns**.
///
/// ⭐ Split out for `d1-replay`, which must know the offset before it arms anything at the client
/// but has no `__writer` subcommand of its own. ⛔ The tracee must be a throwaway process of ours:
/// this pokes zero into every debug register while probing, and the docs on
/// [`ptrace::derive_debugreg_offset`] say why that must never be the emulator.
pub fn debugreg_offset_against(pid: i32) -> Result<(u64, String)> {
    ptrace::seize(pid, 0)?;
    ptrace::interrupt(pid)?;
    ptrace::wait_for(pid, Duration::from_secs(2))?;
    let derived = ptrace::derive_debugreg_offset(pid);
    // Leave the child clean whatever happened.
    for i in 0..8 {
        let _ = ptrace::pokeuser(pid, ptrace::dr_offset(ptrace::DEBUGREG_OFFSET, i), 0);
    }
    let _ = ptrace::detach(pid);
    let (off, detail) = derived?;
    if off != ptrace::DEBUGREG_OFFSET {
        bail!(
            "measured u_debugreg at {off:#x} but the crate is compiled with {:#x} - {detail}",
            ptrace::DEBUGREG_OFFSET
        );
    }
    Ok((off, detail))
}

/// ⭐ The positive control. Catch the exact instruction that writes a chosen address.
pub fn catches_the_writer(helper: &Path, dbg: u64) -> Result<String> {
    let s = spawn_writer(helper, 20.0)?;
    let info = s.info;
    let report = watch_once(&info, info.target, dbg, Duration::from_secs(5))?;
    let hit = report
        .hits
        .first()
        .ok_or_else(|| anyhow::anyhow!("no trap in {:.1}s - {}", report.seconds, report.stop_reason))?;
    if hit.rip != info.next {
        bail!(
            "caught RIP {:#x}, but the child reports its store at {:#x} and the next instruction at \
             {:#x}. A data breakpoint is a TRAP: RIP must be the instruction AFTER the store.",
            hit.rip,
            info.store,
            info.next
        );
    }
    if hit.writer != Some(info.store) {
        bail!(
            "backward decode named {:?} as the writer; the child's own asm says {:#x}",
            hit.writer.map(|w| format!("{w:#x}")),
            info.store
        );
    }
    Ok(format!(
        "child pid {} store {:#x} `{}` (confidence {}), trap RIP {:#x} == store+{} ✓, {} thread(s) armed, hit in {} ms",
        info.pid,
        info.store,
        hit.writer_text.clone().unwrap_or_default(),
        hit.writer_confidence,
        hit.rip,
        info.next - info.store,
        report.threads_armed,
        hit.elapsed_ms
    ))
}

/// ⭐ The inversion. Same child, same second: a decoy 16 bytes away must NOT trap, and then the real
/// target must. Without the second leg, "no trap" would be explained just as well by a dead child.
pub fn address_specific(helper: &Path, dbg: u64) -> Result<String> {
    let s = spawn_writer(helper, 20.0)?;
    let info = s.info;
    if info.decoy == info.target {
        bail!("the control child handed back the same address twice");
    }

    let neg = watch_once(&info, info.decoy, dbg, Duration::from_millis(500))?;
    if !neg.hits.is_empty() {
        bail!(
            "the watchpoint on the DECOY {:#x} fired {} time(s) - it is not address-specific, and \
             every positive result from this tool would be meaningless",
            info.decoy,
            neg.hits.len()
        );
    }
    if neg.threads_armed == 0 {
        bail!("the decoy leg armed no thread, so its silence proves nothing");
    }

    // The child is STILL RUNNING and still storing. If it were not, the silence above would be
    // trivial - so prove it, now, with the same mechanism.
    let pos = watch_once(&info, info.target, dbg, Duration::from_secs(5))?;
    if pos.hits.is_empty() {
        bail!(
            "the decoy was silent AND the target was silent - the child had stopped storing, so \
             the negative leg proved nothing ({})",
            pos.stop_reason
        );
    }
    Ok(format!(
        "decoy {:#x}: 0 hits in {:.2}s with {} thread(s) armed; target {:#x} (16 bytes away, same \
         child, immediately after): {} hit in {} ms",
        info.decoy,
        neg.seconds,
        neg.threads_armed,
        info.target,
        pos.hits.len(),
        pos.hits[0].elapsed_ms
    ))
}

/// ⭐ The axis: **the register map on a `Hit` is the trapping thread's LIVE values**, not a zeroed
/// struct, not a plausible-looking fabrication, and not the wrong thread's.
///
/// Three legs, each of which fails on a different way the capture could lie:
///
/// 1. **Complete and keyed.** Every name in [`ptrace::REG_NAMES`] is present. A missing key reads
///    downstream as "the register was not set", which is a different fact from its value.
/// 2. **Anchored to a value the CHILD reported.** `regs["rip"]` must equal `hit.rip`, and `hit.rip`
///    must equal the child's own `NEXT` label - the same anchor [`catches_the_writer`] uses. So the
///    map is tied to an address measured by the target itself, not to anything this tool invented.
/// 3. **Live, not synthetic.** A general-purpose register must still hold the store's pointer
///    operand (`mov qword ptr [reg], reg` - the pointer is in a register by construction, and
///    nothing between the store and the trap clobbers it). A zeroed or fabricated map fails here.
///    Paired with a spread check: 16 identical values is a fill, not a register file.
///
/// ⚠ The built-in negative - the decoy address must appear in NO register - is reported as
/// **DEGENERATE** rather than FAIL if it does fire, because a dead value can legitimately linger in
/// a callee-saved register. Banking it as a pass would be decoration; saying so is not.
pub fn registers_at_the_hit(helper: &Path, dbg: u64) -> Result<String> {
    let s = spawn_writer(helper, 20.0)?;
    let info = s.info;
    let report = watch_once(&info, info.target, dbg, Duration::from_secs(5))?;
    let hit = report
        .hits
        .first()
        .ok_or_else(|| anyhow::anyhow!("no trap in {:.1}s - {}", report.seconds, report.stop_reason))?;

    // Leg 1 - completeness.
    let missing: Vec<&str> =
        ptrace::REG_NAMES.iter().copied().filter(|n| !hit.regs.contains_key(*n)).collect();
    if !missing.is_empty() {
        bail!(
            "the hit's register map is missing {} name(s): {}. A missing key reads as 'not set', \
             which is NOT the same fact as a value.",
            missing.len(),
            missing.join(", ")
        );
    }
    if hit.regs.len() != ptrace::REG_NAMES.len() {
        bail!(
            "the register map has {} entries but {} names were captured - a duplicated key has \
             silently eaten a field",
            hit.regs.len(),
            ptrace::REG_NAMES.len()
        );
    }

    // Leg 2 - anchored to the child's own measurement.
    if hit.rip != info.next {
        bail!(
            "caught RIP {:#x} but the child reports the instruction after its store at {:#x} - the \
             anchor for this control does not hold",
            hit.rip,
            info.next
        );
    }
    if hit.regs.get("rip").copied() != Some(hit.rip) {
        bail!(
            "regs[\"rip\"] is {:?} but the hit's own RIP is {:#x} - the map is not the register \
             file this trap reported",
            hit.regs.get("rip").map(|v| format!("{v:#x}")),
            hit.rip
        );
    }

    // Leg 3 - live values.
    let holding: Vec<&str> = ptrace::GP_REG_NAMES
        .iter()
        .copied()
        .filter(|n| hit.regs.get(*n).copied() == Some(info.target))
        .collect();
    if holding.is_empty() {
        bail!(
            "no general-purpose register held the store's pointer operand {:#x}. The child's store \
             is `mov qword ptr [reg], reg`, so at the trap that pointer IS in a register - a map \
             without it is not the thread's live state. Values seen: {}",
            info.target,
            ptrace::GP_REG_NAMES
                .iter()
                .map(|n| format!("{n}={:#x}", hit.regs[*n]))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    let distinct: std::collections::BTreeSet<u64> =
        ptrace::GP_REG_NAMES.iter().map(|n| hit.regs[*n]).collect();
    if distinct.len() < 4 {
        bail!(
            "only {} distinct value(s) across the 16 general registers - that is a fill, not a \
             register file",
            distinct.len()
        );
    }

    // The built-in negative, honestly scored.
    let decoy_in: Vec<&str> = ptrace::GP_REG_NAMES
        .iter()
        .copied()
        .filter(|n| hit.regs.get(*n).copied() == Some(info.decoy))
        .collect();
    let neg = if decoy_in.is_empty() {
        format!("the never-stored decoy {:#x} appears in NO register ✓", info.decoy)
    } else {
        format!(
            "⚠ DEGENERATE NEGATIVE: the never-stored decoy {:#x} is still sitting in {} - a dead \
             value lingering in a callee-saved register. The positive legs stand; this leg proved \
             nothing this time",
            info.decoy,
            decoy_in.join(",")
        )
    };

    Ok(format!(
        "child pid {}: {} registers captured, regs[\"rip\"] == RIP {:#x} == the child's own NEXT \
         label ✓, the store's pointer {:#x} is live in {} ({} distinct GP values), fs_base={:#x}; {}",
        info.pid,
        hit.regs.len(),
        hit.rip,
        info.target,
        holding.join(","),
        distinct.len(),
        hit.regs["fs_base"],
        neg
    ))
}

/// ⭐ The axis: **the capture runs with NOTHING armed.**
///
/// This is a find-nothing claim, so it ships with the find-something leg that gives it meaning
/// (CLAUDE.md: *a scan that finds nothing must be paired with one that finds something*):
///
/// * **leg 1 - cleared.** Catch a trap the normal way. Inside the closure, with the thread still
///   stopped, read `DR7` **out of the child** and require **0**, and require
///   [`DisarmProof::all_clear`] over every armed thread.
/// * **leg 2 - the pair.** The same catch with the disarm step skipped. The same read must return
///   the **armed** `DR7` - the exact value [`ptrace::dr7_for`] computed. Without this, leg 1's zero
///   would be equally well explained by "the watchpoint was never armed", which is precisely the
///   false negative this crate exists to avoid.
///
/// Both children must survive: leg 2 leaves through a verified disarm as well, so skipping the
/// safety step is a *measurement*, never a hazard.
pub fn capture_runs_unarmed(helper: &Path, dbg: u64) -> Result<String> {
    let expected_dr7 = ptrace::dr7_for(0, Rw::Write, 8)?;

    let leg = |disarm: bool| -> Result<(u64, usize, String)> {
        let s = spawn_writer(helper, 20.0)?;
        let info = s.info;
        let proc = Proc::open(info.pid)?;
        let spec = WatchSpec {
            addr: info.target,
            len: 8,
            rw: Rw::Write,
            slot: 0,
            follow_clone: false,
            base: None,
            tids: None,
        };
        let mut w = Watcher::attach(info.pid, spec, dbg)?;
        let caught = w.catch_one_with(&proc, Duration::from_secs(5), disarm, |hit, stop| {
            // The read is taken from the CHILD's own task, in the middle of the capture window -
            // not from anything this process remembers writing.
            let dr7 = ptrace::peekuser(hit.tid, ptrace::dr_offset(dbg, 7))?;
            Ok((dr7, stop.disarm.clone()))
        })?;
        let Some((dr7, proof)) = caught.value else {
            bail!("no trap in {:.1}s - {}", caught.report.seconds, caught.report.stop_reason);
        };
        let (alive, how) = survives_abandonment(&info);
        if !alive {
            bail!("the child {how} - a control leg must not damage its own subject");
        }
        drop(s);
        Ok((dr7, proof.threads.len(), format!("{:?}", proof.all_clear())))
    };

    let (cleared, n_cleared, all_clear) = leg(true)?;
    if cleared != 0 {
        bail!(
            "with the disarm step in place, the child's own DR7 still reads {cleared:#x} INSIDE the \
             capture window - the capture would run armed, and a crash during it would kill the \
             client"
        );
    }
    if all_clear != "true" || n_cleared == 0 {
        bail!(
            "the disarm proof covered {n_cleared} thread(s) and all_clear={all_clear} - an empty or \
             partial proof is 'we measured nothing', not 'nothing is armed'"
        );
    }

    let (armed, _, _) = leg(false)?;
    if armed == 0 {
        bail!(
            "the leg that SKIPS the disarm also read DR7 == 0, so the zero above proves nothing: \
             either the watchpoint was never armed or this read cannot see the debug registers. \
             Expected {expected_dr7:#x}"
        );
    }
    if armed != expected_dr7 {
        bail!(
            "the skip-disarm leg read DR7 {armed:#x} but arming asked for {expected_dr7:#x} - the \
             read is not seeing the register we set"
        );
    }
    Ok(format!(
        "inside the capture window the child's own DR7 reads {cleared:#x} with the disarm step \
         ({n_cleared} thread(s) proven clear) and {armed:#x} without it - the same read, the same \
         instrument, one step apart. The zero means CLEARED, not never-armed."
    ))
}

/// ⭐⭐ The axis: **a crash DURING the capture cannot strand an armed tracer.**
///
/// This is the risk the whole `--snapshot-on-hit` design is arranged around. The capture is the
/// long, unbounded, panicking-capable step, and lengthening the armed window is the one thing this
/// crate must never do - so the design moves the work *outside* the window rather than making it
/// safe inside it, and this control is what proves the window moved.
///
/// * **Leg A - the inversion**, and it is the same `__arm-and-die` helper
///   [`disarm_is_load_bearing`] uses: a tracer that exits while `DR0` is armed. On this kernel the
///   abandoned child **dies of `SIGTRAP`**. If it survives, the kernel flushes on detach, the axis
///   does not exist on this box, and the control says **DEGENERATE** rather than banking a pass.
/// * **Leg B - the positive**: `__capture-panic` arms, traps, clears every debug register, and then
///   **panics in the middle of the capture** with the thread still stopped. Three things must hold
///   afterwards: the child is **alive**, its `DR7` reads **0**, and it is **still storing** (two
///   reads of its target cell differ) - so "alive" cannot mean "alive but wedged in a ptrace-stop".
///
/// ⚠ The two legs are each other's control: same helper binary, same child shape, same kernel,
/// one step different.
pub fn panic_during_capture_is_survivable(helper: &Path, dbg: u64) -> Result<String> {
    // ── Leg A: armed abandon ────────────────────────────────────────────────────────────────────
    let a = spawn_writer(helper, 10.0)?;
    let st = Command::new(helper)
        .arg("__arm-and-die")
        .arg("--pid")
        .arg(a.info.pid.to_string())
        .arg("--addr")
        .arg(format!("{:#x}", a.info.target))
        .arg("--dbg")
        .arg(dbg.to_string())
        .stdout(Stdio::null())
        .status()
        .context("running the arm-and-die helper")?;
    if !st.success() {
        bail!("the arm-and-die helper failed ({st}) - the inversion was never asked");
    }
    let (armed_survived, armed_how) = survives_abandonment(&a.info);

    // ── Leg B: panic in the middle of the capture, with nothing armed ───────────────────────────
    let b = spawn_writer(helper, 20.0)?;
    let out = Command::new(helper)
        .arg("__capture-panic")
        .arg("--pid")
        .arg(b.info.pid.to_string())
        .arg("--addr")
        .arg(format!("{:#x}", b.info.target))
        .arg("--dbg")
        .arg(dbg.to_string())
        .output()
        .context("running the capture-panic helper")?;
    let helper_err = String::from_utf8_lossy(&out.stderr).to_string();
    // A panic exits 101. Anything else means the helper never reached the capture, so the question
    // was not asked - that is an error, not a pass.
    if out.status.code() != Some(101) {
        bail!(
            "the capture-panic helper exited {:?}, not 101 - it did not panic inside the capture, \
             so nothing was tested. stderr: {}",
            out.status.code(),
            helper_err.trim()
        );
    }
    if !helper_err.contains("SIMULATED CRASH DURING CAPTURE") {
        bail!("the capture-panic helper did not report the simulated crash: {}", helper_err.trim());
    }
    let (survived, how) = survives_abandonment(&b.info);
    if !survived {
        bail!(
            "⛔ the child {how} after a panic during the capture - the capture window is still \
             lethal, and `--snapshot-on-hit` must not ship. Helper said: {}",
            helper_err.trim()
        );
    }

    // Alive is not enough: it must be UNARMED and RUNNING.
    let proc = Proc::open(b.info.pid)?;
    let first = proc
        .try_read(b.info.target, 8)
        .ok_or_else(|| anyhow::anyhow!("the surviving child's target cell is unreadable"))?;
    std::thread::sleep(Duration::from_millis(60));
    let second = proc
        .try_read(b.info.target, 8)
        .ok_or_else(|| anyhow::anyhow!("the surviving child's target cell is unreadable"))?;
    if first == second {
        bail!(
            "the child survived the panic but its target cell did not change in 60 ms - it is alive \
             and NOT RUNNING, which is a stranded tracee, not a success"
        );
    }
    let dr7 = read_dr7_of(b.info.pid, dbg)?;
    if dr7 != 0 {
        bail!(
            "the child survived and is running, but its DR7 reads {dr7:#x} - a live debug register \
             on a process with no tracer is a SIGTRAP waiting to happen"
        );
    }

    if armed_survived {
        return Ok(format!(
            "⚠ DEGENERATE INVERSION: an ARMED abandon left child {} alive too, so this kernel \
             flushes ptrace hardware breakpoints on implicit detach and leg B's survival proves \
             nothing here. Leg B still measured what it could: after the panic, child {} is alive, \
             DR7 = 0, and still storing. Do not carry this reading to another kernel.",
            a.info.pid, b.info.pid
        ));
    }
    Ok(format!(
        "⛔ ARMED abandon KILLS: child {} was {}. ⭐ PANIC DURING THE CAPTURE DOES NOT: the helper \
         armed, trapped, cleared every debug register and then panicked with the thread still \
         stopped (exit 101) - child {} is alive, DR7 reads {dr7:#x}, and its target cell is still \
         advancing. ⇒ the unbounded work is OUTSIDE the armed window, which is the whole safety \
         argument for --snapshot-on-hit.",
        a.info.pid, armed_how, b.info.pid
    ))
}

/// Read one task's `DR7` without disturbing it: seize, interrupt, peek, detach.
fn read_dr7_of(pid: i32, dbg: u64) -> Result<u64> {
    ptrace::seize(pid, 0)?;
    ptrace::interrupt(pid)?;
    ptrace::wait_for(pid, Duration::from_secs(2))?;
    let v = ptrace::peekuser(pid, ptrace::dr_offset(dbg, 7));
    let _ = ptrace::detach(pid);
    v
}

/// Did a child that was armed and then abandoned survive? `(alive, how it died)`.
fn survives_abandonment(info: &WriterChild) -> (bool, String) {
    let deadline = Instant::now() + Duration::from_millis(750);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut st: i32 = 0;
    let r = unsafe { libc::waitpid(info.pid, &mut st, libc::WNOHANG) };
    if r != info.pid {
        return (true, String::new());
    }
    let how = if st & 0x7f != 0 {
        format!("killed by signal {} (status {st:#x})", st & 0x7f)
    } else {
        format!("exited with code {} (status {st:#x})", (st >> 8) & 0xff)
    };
    (false, how)
}

/// ⭐ The axis: **[`Watcher`]'s disarm path actually removes the breakpoint**, and that mattering is
/// itself measured rather than assumed.
///
/// * Leg A - the **inversion**: a helper process arms `DR0` and exits *without* disarming. On a
///   kernel that leaves a ptrace hardware breakpoint installed after an implicit detach, the child
///   takes a `SIGTRAP` with no tracer and dies. Leg A tells us which world we are in.
/// * Leg B - the **positive**: the real [`Watcher`] arms and is dropped. The child must survive.
///
/// If leg A shows the child *surviving*, this kernel flushes on detach, leg B's survival is free,
/// and the control says so out loud rather than banking a degenerate pass - the same discipline as
/// the shifted-window negative control in [`crate::proc::recover_base`].
///
/// ⛔ Leg A's result on this box is the reason `watch` has a `SIGKILL` warning at all.
pub fn disarm_is_load_bearing(helper: &Path, dbg: u64) -> Result<String> {
    // ── Leg A: arm, abandon, observe ────────────────────────────────────────────────────────────
    let a = spawn_writer(helper, 10.0)?;
    let status = Command::new(helper)
        .arg("__arm-and-die")
        .arg("--pid")
        .arg(a.info.pid.to_string())
        .arg("--addr")
        .arg(format!("{:#x}", a.info.target))
        .arg("--dbg")
        .arg(dbg.to_string())
        .stdout(Stdio::null())
        .status()
        .context("running the arm-and-die helper")?;
    if !status.success() {
        bail!("the arm-and-die helper failed ({status}) - the question was never asked");
    }
    let (abandoned_survived, abandoned_how) = survives_abandonment(&a.info);

    // ── Leg B: arm through the real Watcher, drop it, observe ───────────────────────────────────
    let b = spawn_writer(helper, 10.0)?;
    {
        let proc = Proc::open(b.info.pid)?;
        let spec = WatchSpec {
            addr: b.info.target,
            len: 8,
            rw: Rw::Write,
            slot: 0,
            follow_clone: false,
            base: None,
            tids: None,
        };
        let mut w = Watcher::attach(b.info.pid, spec, dbg)?;
        // Take one real hit first, so the slot is provably live before we tear it down.
        let r = w.run(&proc, 1, Duration::from_secs(5), |_| {})?;
        if r.hits.is_empty() {
            bail!("leg B never trapped, so its disarm proves nothing ({})", r.stop_reason);
        }
    } // <- Drop runs disarm() here, and shouts on stderr if it could not verify.
    let (disarmed_survived, disarmed_how) = survives_abandonment(&b.info);

    if !disarmed_survived {
        bail!(
            "⛔ the child {} after `Watcher` was dropped - the disarm path does NOT remove the \
             breakpoint, and a watch against the emulator would kill it on exit.",
            disarmed_how
        );
    }
    if abandoned_survived {
        return Ok(format!(
            "⚠ DEGENERATE INVERSION: an abandoned tracer left child {} alive too, so this kernel \
             flushes ptrace hardware breakpoints on implicit detach and leg B proves nothing. \
             Nothing to guard against here - but do not carry this reading to another kernel.",
            a.info.pid
        ));
    }
    Ok(format!(
        "⛔ SAFETY FINDING [C]: a tracer that exits while DR0 is armed KILLS its target - child {} was \
         {}. Our disarm path is therefore load-bearing, and it works: \
         child {} took a real hit, then survived `Watcher::drop`. ⇒ NEVER `SIGKILL` `d1-live \
         watch` against shadPS4. ⚠ Everything that is CAUGHT is safe - Ctrl-C, SIGTERM, --max-seconds, \
         and since build 3 the other 15 signals in GRACEFUL_SIGNALS/FATAL_SIGNALS (SIGHUP was the \
         measured hole). What is safe is the CAUGHT set, not \"anything but SIGKILL\": see \
         `fatal-signals-disarm`.",
        a.info.pid, abandoned_how, b.info.pid
    ))
}

/// Run every control. `helper` is this crate's own binary.
pub fn run_all(helper: &Path) -> Controls {
    let mut c = Controls::default();

    let dbg = match debugreg_offset(helper) {
        Ok((off, detail)) => {
            c.push(
                "debugreg-offset",
                "offsetof(struct user, u_debugreg) is measured, not transcribed; the words on \
                 either side of the array must be REJECTED",
                Ok(format!("u_debugreg[0] at {off:#x} - {detail}")),
            );
            Some(off)
        }
        Err(e) => {
            c.push(
                "debugreg-offset",
                "offsetof(struct user, u_debugreg) is measured, not transcribed",
                Err(e),
            );
            None
        }
    };

    let Some(dbg) = dbg else {
        c.push(
            "catches-the-writer",
            "a write watchpoint catches the exact instruction that wrote the chosen address",
            Err(anyhow::anyhow!("skipped: the debug-register offset could not be measured")),
        );
        return c;
    };

    c.push(
        "catches-the-writer",
        "a write watchpoint catches the EXACT instruction that wrote the chosen address (the \
         child reports its own store address from inline asm; RIP must be store + insn_len)",
        catches_the_writer(helper, dbg),
    );
    c.push(
        "address-specific",
        "the watchpoint is about the ADDRESS, not 'traps always fire': a decoy 16 bytes away must \
         stay silent while the same, still-running child trips the real target",
        address_specific(helper, dbg),
    );
    c.push(
        "registers-at-the-hit",
        "the register map on a Hit is the trapping thread's LIVE integer register file: complete, \
         keyed by name, anchored to an address the child itself reported, and holding the store's \
         own pointer operand - a zeroed or fabricated map fails",
        registers_at_the_hit(helper, dbg),
    );
    c.push(
        "disarm-is-load-bearing",
        "Watcher's disarm path really removes the breakpoint - inverted by a helper that arms and \
         abandons the target, which is what tells us whether disarming matters at all",
        disarm_is_load_bearing(helper, dbg),
    );
    c.push(
        "capture-runs-unarmed",
        "the capture window runs with NOTHING armed: the child's own DR7, read inside the window, \
         is 0 with the disarm step and the ARMED value without it - a find-nothing paired with a \
         find-something, so the zero cannot mean 'never armed'",
        capture_runs_unarmed(helper, dbg),
    );
    c.push(
        "panic-during-capture-is-survivable",
        "a crash DURING the capture cannot strand an armed tracer: a helper panics mid-capture with \
         the debug registers already cleared and the child must be alive, unarmed and still \
         storing - inverted by the ARMED abandon, which kills",
        panic_during_capture_is_survivable(helper, dbg),
    );
    c.push(
        "fatal-signals-disarm",
        "ANY catchable terminating signal leaves through a verified disarm, not just SIGINT and \
         SIGTERM: SIGHUP to a handler-less tracer KILLS the child (the inversion), and the same \
         signal to a handled one leaves it alive with DR7 == 0",
        fatal_signals_disarm(helper, dbg),
    );
    c.push(
        "a-failed-arm-is-still-disarmed",
        "an arm that fails AFTER its pokes landed stays on the disarm list: an injected read-back \
         failure must make attach REFUSE and must still leave the child alive with DR7 == 0 - \
         inverted by the arm-and-abandon, which kills",
        a_failed_arm_is_still_disarmed(helper, dbg),
    );
    read_side(&mut c, helper);
    c.push(
        "disarm-is-bounded-and-multithreaded",
        "the capture-time disarm is BOUNDED and is exercised across FOUR threads, not one: inside \
         the bound every thread is proven clear and the capture runs; with the bound set to ZERO \
         catch_one REFUSES the capture and the child still ends up alive with DR7 == 0",
        disarm_is_bounded_and_multithreaded(helper, dbg),
    );
    c
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The child itself, and the arm-and-die helper
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One `mov qword ptr [target], v`, whose own address and successor are handed back from inline asm
/// labels. `#[inline(never)]` keeps it a single instruction at a single address across the loop - 
/// which is what lets the control assert an exact `RIP`.
#[inline(never)]
fn poke_cell(target: *mut u64, v: u64) -> (u64, u64) {
    let store: u64;
    let next: u64;
    unsafe {
        std::arch::asm!(
            "lea {store}, [rip + 2f]",
            "lea {next}, [rip + 3f]",
            "2:",
            "mov qword ptr [{t}], {v}",
            "3:",
            t = in(reg) target,
            v = in(reg) v,
            store = out(reg) store,
            next = out(reg) next,
            options(nostack, preserves_flags),
        );
    }
    (store, next)
}

/// The `__writer` subcommand: print the banner, then store to `target` until the deadline.
/// `decoy` is 16 bytes away in the same allocation and is **never written**.
///
/// `extra` spinner threads are started **before** the banner is printed, so a tracer that attaches
/// on the banner sees all of them in `/proc/<pid>/task` and arms every one. They never touch
/// `target`, so they cannot supply a trap - their only job is to make the disarm visit N threads.
pub fn writer_main(seconds: f64, extra: usize) -> ! {
    use std::io::Write;
    let cells = Box::leak(Box::new([0u64; 4])).as_mut_ptr();
    let target = unsafe { cells.add(0) };
    let decoy = unsafe { cells.add(2) };
    for _ in 0..extra {
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs_f64(seconds + 5.0);
            let mut own = 0u64;
            while Instant::now() < deadline {
                own = own.wrapping_add(1);
                std::hint::black_box(&own);
                std::thread::sleep(Duration::from_micros(200));
            }
        });
    }
    // Give every spinner time to appear in /proc/<pid>/task before anyone reads it.
    if extra > 0 {
        std::thread::sleep(Duration::from_millis(60));
    }
    let (store, next) = poke_cell(target, 1);
    println!(
        "TARGET {:#x} DECOY {:#x} STORE {:#x} NEXT {:#x}",
        target as u64, decoy as u64, store, next
    );
    std::io::stdout().flush().ok();

    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    let mut i = 0u64;
    while Instant::now() < deadline {
        for _ in 0..20_000 {
            poke_cell(target, i);
            i = i.wrapping_add(1);
        }
        // Yield a sliver so the control does not need a whole core; still thousands of stores per
        // millisecond, so "no trap" can never be blamed on an idle child.
        std::thread::sleep(Duration::from_micros(50));
    }
    std::process::exit(0);
}

/// The `__capture-panic` subcommand: arm, catch one trap, and then **panic in the middle of the
/// capture** - the crash [`panic_during_capture_is_survivable`] is built around.
///
/// ⭐ The panic happens inside [`Watcher::catch_one`]'s closure, i.e. at step 3 of §6a.2: the
/// trapping thread is stopped and **every debug register has already been cleared and read back as
/// zero**. So the process dies with a tracee that the kernel auto-detaches, and the tracee resumes.
///
/// ⛔ `armed = true` inverts it: seize, arm, continue, panic - no catch, no disarm, no guard. That
/// is the arm-and-abandon shape, and on this kernel it KILLS the child. It exists so the positive
/// leg can be broken on purpose and watched to fail, which is the only thing that makes it a
/// control rather than decoration.
pub fn capture_panic_main(pid: i32, addr: u64, dbg: u64, armed: bool) -> Result<()> {
    if armed {
        arm_and_die_main(pid, addr, dbg)?;
        panic!("SIMULATED CRASH DURING CAPTURE (armed: the debug registers were NOT cleared)");
    }
    let proc = Proc::open(pid)?;
    let spec = WatchSpec {
        addr,
        len: 8,
        rw: Rw::Write,
        slot: 0,
        follow_clone: false,
        base: None,
        tids: None,
    };
    let mut w = Watcher::attach(pid, spec, dbg)?;
    let _ = w.catch_one(&proc, Duration::from_secs(5), |_hit, stop| -> Result<()> {
        if !stop.disarm.all_clear() {
            // Panicking here too would still be safe, but it would test the wrong thing: the axis
            // is "a crash AFTER the disarm is survivable", so an unproven disarm is an error.
            bail!(
                "the disarm did not verify before the capture ({} thread(s), {} failure(s)) - \
                 refusing to panic, because the result would not measure the axis",
                stop.disarm.threads.len(),
                stop.disarm.failures.len()
            );
        }
        panic!("SIMULATED CRASH DURING CAPTURE (unarmed: {} thread(s) proven clear)", stop.disarm.threads.len());
    })?;
    bail!("the capture-panic helper returned without panicking - the question was never asked");
}

/// The `__hold-armed` subcommand: arm through a **real** [`Watcher`] and then sit in its wait loop
/// until a signal arrives.
///
/// ⭐ This is the process a control signals. With `handlers = true` the signal is caught,
/// `INTERRUPTED` is set, `Watcher::run` returns and `Drop` disarms - the child must survive. With
/// `handlers = false` the signal's **default action** applies: for `SIGHUP` that is terminate
/// without unwinding, so nothing disarms and the child dies of `SIGTRAP`. That pair is the axis.
pub fn hold_armed_main(pid: i32, addr: u64, dbg: u64, handlers: bool) -> Result<()> {
    if handlers {
        crate::watch::install_signal_handlers();
    }
    let proc = Proc::open(pid)?;
    let spec =
        WatchSpec { addr, len: 8, rw: Rw::Write, slot: 0, follow_clone: false, base: None, tids: None };
    let mut w = Watcher::attach(pid, spec, dbg)?;
    println!("HELD {}", w.armed_threads());
    use std::io::Write;
    std::io::stdout().flush().ok();
    // A huge count so the loop only ever leaves through the signal or the deadline. Hits are
    // continued and ignored; the point is to be sitting here, armed, when the signal lands.
    let _ = w.run(&proc, usize::MAX, Duration::from_secs(30), |_| {})?;
    Ok(())
}

/// ⭐⭐ The axis: **any catchable signal that terminates the tracer must still leave through a
/// verified disarm** - not just `SIGINT` and `SIGTERM`.
///
/// * Leg A - the **inversion**: `__hold-armed --no-handlers`, then `SIGHUP`. The default action
///   terminates without unwinding, so no `Drop` runs and the abandoned child must DIE. If it lives,
///   the control reports DEGENERATE rather than banking a free pass.
/// * Leg B - the **positive**: `__hold-armed` with the handlers installed, then the same `SIGHUP`.
///   The child must be alive and its `DR7` must read zero.
///
/// ⛔ `SIGHUP` specifically, because that is the measured defect: `install_signal_handlers` covered
/// `SIGINT`/`SIGTERM` only, the documented workflow is *arm in one shell, poke in a second*, and the
/// default `--hit-timeout` is 120 s - so a dropped ssh session or a closed terminal inside that
/// window killed the client, while the banner said "NEVER SIGKILL this" and implied everything else
/// was survivable.
pub fn fatal_signals_disarm(helper: &Path, dbg: u64) -> Result<String> {
    let hup = |handlers: bool| -> Result<(i32, bool, String, bool)> {
        let w = spawn_writer(helper, 20.0)?;
        let mut tracer = Command::new(helper)
            .arg("__hold-armed")
            .arg("--pid")
            .arg(w.info.pid.to_string())
            .arg("--addr")
            .arg(format!("{:#x}", w.info.target))
            .arg("--dbg")
            .arg(dbg.to_string())
            .args(if handlers { Vec::new() } else { vec!["--no-handlers"] })
            .stdout(Stdio::piped())
            .spawn()
            .context("spawning the hold-armed tracer")?;
        // Wait for its own receipt that the watchpoint is live before signalling it.
        let out = tracer.stdout.take().expect("piped");
        let mut line = String::new();
        BufReader::new(out).read_line(&mut line).context("reading the hold-armed banner")?;
        if !line.starts_with("HELD") {
            let _ = tracer.kill();
            bail!("the hold-armed tracer never armed (banner {line:?}) - the question was not asked");
        }
        unsafe { libc::kill(tracer.id() as i32, libc::SIGHUP) };
        let st = tracer.wait().context("waiting for the hold-armed tracer")?;
        let (alive, how) = survives_abandonment(&w.info);
        let dr7_zero = alive && read_dr7_of(w.info.pid, dbg).map(|v| v == 0).unwrap_or(false);
        Ok((st.code().unwrap_or(-1), alive, how, dr7_zero))
    };

    let (_, a_alive, a_how, _) = hup(false)?;
    let (b_code, b_alive, b_how, b_clear) = hup(true)?;

    if !b_alive {
        bail!(
            "⛔ SIGHUP with the handlers installed still KILLED the child ({b_how}) - the signal \
             set is incomplete or the disarm did not run"
        );
    }
    if !b_clear {
        bail!("the child survived SIGHUP but its DR7 does not read zero - it is still armed");
    }
    if a_alive {
        return Ok(
            "⚠ DEGENERATE INVERSION: SIGHUP to a tracer with NO handlers left the child alive too, \
             so this kernel/libc combination is not one where an unhandled terminating signal \
             strands a debug register. Leg B proves nothing here - do not carry this reading to \
             another box."
                .to_string(),
        );
    }
    Ok(format!(
        "⛔ SIGHUP to a tracer WITHOUT handlers KILLS its target - child was {a_how}. ⭐ With the \
         handlers installed the same signal is survivable: the tracer exited {b_code}, the child is \
         alive and its DR7 reads 0. GRACEFUL_SIGNALS covers {} signals and FATAL_SIGNALS {} more, \
         so a dropped terminal, a SIGQUIT, a SIGXCPU or an abort() now all leave through a disarm.",
        crate::watch::GRACEFUL_SIGNALS.len(),
        crate::watch::FATAL_SIGNALS.len()
    ))
}

/// ⭐⭐ The axis: **an arm that FAILS after its pokes have landed is still on the disarm list.**
///
/// [`Watcher::attach_injecting_arm_failure`] makes `arm_stopped` fail *after* `DR0`/`DR7` have been
/// written and read back, which is the real shape (a read-back mismatch, or a `PTRACE_CONT` that
/// fails once the registers are set). Build 1 pushed such a thread to `threads_failed` and **never**
/// to `armed`, so no disarm path in the program would ever visit it again.
///
/// The pair: leg A is the arm-and-abandon shape from [`disarm_is_load_bearing`], re-measured here so
/// this control carries its own proof that a stranded register is lethal on this kernel; leg B
/// requires `attach` to refuse AND the child to be alive with `DR7 == 0` afterwards.
pub fn a_failed_arm_is_still_disarmed(helper: &Path, dbg: u64) -> Result<String> {
    // ── Leg A: the axis. A register stranded by an abandoning tracer kills. ─────────────────────
    let a = spawn_writer(helper, 10.0)?;
    let st = Command::new(helper)
        .arg("__arm-and-die")
        .arg("--pid")
        .arg(a.info.pid.to_string())
        .arg("--addr")
        .arg(format!("{:#x}", a.info.target))
        .arg("--dbg")
        .arg(dbg.to_string())
        .stdout(Stdio::null())
        .status()
        .context("running the arm-and-die helper")?;
    if !st.success() {
        bail!("the arm-and-die helper failed ({st}) - the axis was never established");
    }
    let (a_alive, a_how) = survives_abandonment(&a.info);

    // ── Leg B: inject the failure and require the retained thread to be cleared anyway. ─────────
    let b = spawn_writer(helper, 10.0)?;
    let spec = WatchSpec {
        addr: b.info.target,
        len: 8,
        rw: Rw::Write,
        slot: 0,
        follow_clone: false,
        base: None,
        tids: None,
    };
    let r = Watcher::attach_injecting_arm_failure(b.info.pid, spec, dbg);
    let refusal = match r {
        Ok(_) => bail!(
            "attach ACCEPTED a watcher with zero verified arms - a run from there would report a \
             false 'nothing wrote it'"
        ),
        Err(e) => format!("{e:#}"),
    };
    if !refusal.contains("armed 0 of") {
        bail!("attach refused for the wrong reason: {refusal}");
    }
    // ⭐ THE DIRECT ASSERTION, and the one the inversion trips first: the registry entry is removed
    // ONLY by `clear_and_verify`, i.e. only against a `DR7` that read back zero. A tid still on the
    // list after `attach` returned means nothing ever proved its debug registers clear.
    if crate::watch::registry_holds(b.info.pid) {
        bail!(
            "⛔ tid {} is STILL on the emergency registry after attach refused - no disarm path \
             visited it, so a debug register may be live on a thread nothing in this program will \
             ever revisit (and this process is still its tracer, so it will not even die loudly)",
            b.info.pid
        );
    }
    let (b_alive, b_how) = survives_abandonment(&b.info);
    if !b_alive {
        bail!(
            "⛔ the child {b_how} after an INJECTED arm failure - a thread whose arm failed after \
             the poke landed was left armed, which is the leak this control exists for"
        );
    }
    let dr7 = read_dr7_of(b.info.pid, dbg)?;
    if dr7 != 0 {
        bail!("the child survived but its DR7 reads {dr7:#x} - the retained thread was not cleared");
    }
    let (registered, overflow) = crate::watch::registry_state();
    if overflow != 0 {
        bail!("the emergency registry OVERFLOWED {overflow} time(s) - its coverage is incomplete");
    }
    if a_alive {
        return Ok(format!(
            "⚠ DEGENERATE INVERSION: the abandoned child survived too, so a stranded debug register \
             is not lethal on this kernel and leg B proves less than it looks. attach still refused \
             ({}) and DR7 reads 0.",
            refusal.lines().next().unwrap_or("")
        ));
    }
    Ok(format!(
        "⛔ a stranded register KILLS on this kernel - the abandoned child was {a_how}. ⭐ An \
         INJECTED arm failure (pokes landed, read-back made to fail) does NOT: attach REFUSED \
         (\"armed 0 of 1\"), the retained thread went through the disarm path anyway, child {} is \
         alive and its DR7 reads 0. The emergency registry holds {} tid(s) with 0 overflow(s).",
        b.info.pid,
        registered.len()
    ))
}

/// ⭐⭐ The axis: **the disarm that brackets a capture is BOUNDED, and it is bounded across many
/// threads, not one.**
///
/// Review measured the build-1 disarm at up to `6 attempts x 500 ms x 47 threads = ~141 s` with the
/// trapping thread held stopped and every unvisited thread still armed - against a design that
/// claims "about 0.7 ms" and a client whose own block detector fires at 20,000 ms. And **no control
/// exercised a multi-thread disarm at all**: every existing leg drove a single-threaded child.
///
/// * Leg A - the **positive**: a 4-thread child, the default deadline. Every thread must be proven
///   clear, the capture closure must run, and the measured elapsed must be inside the bound.
/// * Leg B - the **inversion**: the same shape with the deadline set to ZERO. `catch_one` must
///   **REFUSE** the capture rather than run it with threads unproven, and the child must still be
///   alive with `DR7 == 0` - i.e. the refusal is safe, not just loud.
pub fn disarm_is_bounded_and_multithreaded(helper: &Path, dbg: u64) -> Result<String> {
    let spec_for = |addr: u64| WatchSpec {
        addr,
        len: 8,
        rw: Rw::Write,
        slot: 0,
        follow_clone: false,
        base: None,
        tids: None,
    };

    // ── Leg A ───────────────────────────────────────────────────────────────────────────────────
    let a = spawn_writer_threads(helper, 10.0, 3)?;
    let (armed_a, cleared_a, elapsed_ms, deadline_ms, ran) = {
        let proc = Proc::open(a.info.pid)?;
        let mut w = Watcher::attach(a.info.pid, spec_for(a.info.target), dbg)?;
        let armed = w.armed_threads();
        if armed < 4 {
            bail!(
                "only {armed} thread(s) were armed on a 4-thread child - the multi-thread axis was \
                 not exercised"
            );
        }
        let caught = w.catch_one(&proc, Duration::from_secs(5), |_h, stop| {
            Ok((stop.disarm.threads.len(), stop.disarm.elapsed_ns, stop.disarm.deadline_ms, stop.disarm.all_clear()))
        })?;
        let Some((n, ns, dl, clear)) = caught.value else {
            bail!("leg A never trapped ({}) - its disarm proves nothing", caught.report.stop_reason);
        };
        if !clear {
            bail!("leg A's disarm was not all-clear across {n} thread(s)");
        }
        (armed, n, ns as f64 / 1e6, dl, true)
    };
    if elapsed_ms > deadline_ms {
        bail!("leg A's disarm took {elapsed_ms:.3} ms, past its own {deadline_ms:.0} ms bound");
    }

    // ── Leg B: the inversion ────────────────────────────────────────────────────────────────────
    let b = spawn_writer_threads(helper, 10.0, 3)?;
    let refusal = {
        let proc = Proc::open(b.info.pid)?;
        let mut w = Watcher::attach(b.info.pid, spec_for(b.info.target), dbg)?;
        w.set_disarm_deadline(Duration::ZERO);
        match w.catch_one(&proc, Duration::from_secs(5), |_h, _s| Ok(())) {
            Ok(c) if c.hit.is_none() => {
                bail!("leg B never trapped ({}) - the inversion was not exercised", c.report.stop_reason)
            }
            Ok(_) => bail!(
                "⛔ with a ZERO disarm deadline the capture RAN anyway - the refusal is not wired, \
                 so an unbounded disarm can still be followed by a capture on threads nobody proved \
                 clear"
            ),
            Err(e) => format!("{e:#}"),
        }
    };
    if !refusal.contains("REFUSING THE CAPTURE") {
        bail!("leg B failed for the wrong reason: {refusal}");
    }
    let (b_alive, b_how) = survives_abandonment(&b.info);
    if !b_alive {
        bail!("⛔ the refusal itself killed the child ({b_how}) - a safe refusal is the whole point");
    }
    let dr7 = read_dr7_of(b.info.pid, dbg)?;
    if dr7 != 0 {
        bail!("after the refusal the child's DR7 reads {dr7:#x} - the fallback disarm did not run");
    }

    Ok(format!(
        "⭐ MULTI-THREAD: {armed_a} thread(s) armed on one child, all {cleared_a} proven clear in \
         {elapsed_ms:.3} ms against a {deadline_ms:.0} ms bound, and the capture closure ran \
         ({ran}). ⛔ INVERTED with the deadline at ZERO: catch_one REFUSED the capture instead of \
         running it with threads unproven, and the refusal is SAFE - child {} is alive and its DR7 \
         reads 0 after the fallback unbounded disarm.",
        b.info.pid
    ))
}

/// The `__arm-and-die` subcommand: seize, arm, continue, and exit **without disarming**. Its only
/// purpose is to ask the kernel the safety question in [`tracer_death_safety`].
pub fn arm_and_die_main(pid: i32, addr: u64, dbg: u64) -> Result<()> {
    ptrace::check_alignment(addr, 8)?;
    let dr7 = ptrace::dr7_for(0, Rw::Write, 8)?;
    ptrace::seize(pid, 0)?;
    ptrace::interrupt(pid)?;
    ptrace::wait_for(pid, Duration::from_secs(2))?;
    ptrace::pokeuser(pid, ptrace::dr_offset(dbg, 0), addr)?;
    ptrace::pokeuser(pid, ptrace::dr_offset(dbg, 7), dr7)?;
    let back = ptrace::peekuser(pid, ptrace::dr_offset(dbg, 0))?;
    if back != addr {
        bail!("DR0 read back {back:#x}, expected {addr:#x} - the safety question was not asked");
    }
    ptrace::cont(pid, 0)?;
    // Deliberately no disarm, no detach.
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The READ SIDE - `capture-rig/d1guest.py` parity, measured against a fixture we can make wrong
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Append the read-side controls. See [`crate::readcontrols`] for the axis table.
///
/// ⛔ If shadPS4 is up these are recorded as **FAILURES with a reason**, not skipped: the fixture
/// cannot start beside a real client, so the read side is genuinely uncertified in that session and
/// the report must say so rather than quietly shrinking.
pub fn read_side(c: &mut Controls, helper: &Path) {
    use crate::readcontrols as rc;

    if let Some(why) = rc::blocked_by_a_live_client() {
        c.push(
            "read-side (8 controls)",
            "the read side is measured against a fixture process that cannot coexist with a real \
             client",
            Err(anyhow::anyhow!("{why}")),
        );
        return;
    }

    c.push(
        "read-side-base-and-read",
        "the base is recovered against the REAL eboot and reads land where the arithmetic says - \
         inverted by a one-byte shift that must differ and by an UNMAPPED address that must ERROR \
         rather than return zeros",
        rc::base_and_read(helper),
    );
    c.push(
        "wrong-image-fails-the-verify",
        "base control 2 compares a third, independent address against the file BY DIRECT READ - \
         inverted by flipping one byte inside that window, which must fail it even though the \
         anchors still agree, and must make Session::open refuse with exit 2",
        rc::wrong_image_fails_the_verify(helper),
    );
    c.push(
        "two-anchors-need-exactly-one-base",
        "the anchors must agree on EXACTLY ONE base - inverted by a second copy of both anchors at \
         another base, which must produce a refusal rather than a choice",
        rc::exactly_one_base(helper),
    );
    c.push(
        "registry-cross-check-refuses",
        "an element address is derived TWICE, from the runtime entry table and from the static \
         family/slot mapper - inverted by one bit of stride, which must WITHDRAW the address and \
         drop the liveness ladder, not annotate it",
        rc::registry_cross_check_refuses(helper),
    );
    c.push(
        "tag-resolves-through-the-back-offset",
        "a 0x8080xxxx tag resolves through the package registry's SIGNED `entry - (mask & loc)` \
         subtraction - inverted by the neighbouring tag, which must not land on the same object, \
         and by an empty slot, which must refuse with a reason",
        rc::tag_back_offset(helper),
    );
    c.push(
        "descriptors-decode",
        "class/key/state/err/version decode at their offsets and the accepts-versus-REFUSES \
         distinction survives - inverted by the 255 free rows, which must be filtered out",
        rc::descriptors_decode(helper),
    );
    c.push(
        "read-after-death-is-an-error",
        "a read after the client dies is an ERROR that asks for exit 3, never a buffer of zeros - \
         paired with the SAME read against the SAME address while it was alive, so the failure \
         cannot be explained by a bad address",
        rc::read_after_death(helper),
    );
    c.push(
        "garbage-in-refuses-loudly",
        "an input the golden diff cannot generate must be REFUSED, never answered - a family index \
         that wraps onto another family's entry, a deleted registry_layout key, and a package field \
         offset past the end of its record; each paired with the positive read it perturbs",
        rc::garbage_in_refuses(helper),
    );
}
