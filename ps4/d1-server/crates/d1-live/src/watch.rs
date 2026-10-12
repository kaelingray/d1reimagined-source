// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Arm a CPU debug register on every thread of a live process, wait for the trap, name the
//! instruction, disarm.
//!
//! # Why the answer is exact
//!
//! There is no candidate list and no search space, so there is no premise to have chosen wrongly
//! (CLAUDE.md lesson 3). The CPU stops the thread that touched the address. What the tool then has
//! to be careful about is *reporting*, and there are exactly two traps:
//!
//! * ⚠ **A data breakpoint is a trap: `RIP` is the NEXT instruction.** Quoting it as "the writer"
//!   names the wrong instruction - often the wrong *basic block*, if the store is the last
//!   instruction before a jump. [`Hit::writer`] is the decoded store; `Hit::rip` is the raw
//!   register. Both are always reported, labelled.
//! * ⚠ **Decoding x86 backwards is a heuristic, and this tool says so.** Every candidate that ends
//!   exactly at `RIP` is reported, the chosen one is marked, and `writer_confidence` is `unique`,
//!   `preferred` (several decode, one writes memory) or `ambiguous`. An `ambiguous` writer is a
//!   `[U]` to be closed with `d1-eboot disasm`, not a finding.
//!
//! # Threads
//!
//! ⚠ Debug registers live in `task_struct`. Every thread in `/proc/<pid>/task` is armed at start;
//! threads created later are armed only under `--follow-clone`. **An unarmed thread is a silent
//! miss**, which is why the report always states how many threads were armed and whether clone
//! following was on - an empty result is only evidence about the threads named in the report.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::proc::Proc;
use crate::ptrace::{self, Rw};

/// Set by every catchable signal whose default action would TERMINATE this process, so the loop can
/// leave through [`Watcher::disarm`] instead of dying armed. ⛔ `SIGKILL` cannot be caught - see the
/// crate docs.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// How many graceful signals have arrived. ⭐ The **second** one stops being polite: it runs
/// [`emergency_disarm`] and `_exit`s. An operator who signals twice is about to reach for `SIGKILL`,
/// and `SIGKILL` is the one signal that cannot be intercepted and therefore the one that kills the
/// client. Giving impatience a safe landing is cheaper than a warning nobody reads.
static SIGNAL_COUNT: AtomicUsize = AtomicUsize::new(0);

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The emergency registry - the only thing a dying process can still act on
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐⭐ **Every tid that MAY carry a live debug register, in a form a signal handler can walk.**
///
/// ⛔ The rule this crate exists under is *a tracer that dies armed kills its target*
/// ([`crate::controls::disarm_is_load_bearing`] measures it on every run). `Watcher`'s `Drop` covers
/// the normal exits and the panic; it does **not** cover the process simply ceasing to exist -
/// `SIGHUP` from a closed terminal, `SIGSEGV`, or the `abort()` Rust takes on allocation failure.
/// None of those unwind, so no `Drop` runs, the kernel auto-detaches with `DR7` still set, and the
/// client dies of `SIGTRAP` on its next write to an address it writes **by construction**.
///
/// So the tid list is duplicated here, in fixed storage, with no allocation and no lock, and the
/// fatal-signal handler walks it with raw syscalls. Entries are written **before the first poke**
/// and removed only after `DR7` has been **read back as zero** - so an entry can be stale-present
/// (harmless: a redundant clear) but never stale-absent (lethal).
const REGISTRY_CAP: usize = 4096;
static REGISTRY: [AtomicI32; REGISTRY_CAP] = [const { AtomicI32::new(0) }; REGISTRY_CAP];
static REGISTRY_DBG: AtomicU64 = AtomicU64::new(0);
/// Threads that could not be recorded because the table was full. ⚠ Non-zero means the emergency
/// path is INCOMPLETE, and it is reported rather than swallowed.
static REGISTRY_OVERFLOW: AtomicUsize = AtomicUsize::new(0);

/// Record `tid` as possibly-armed. Call this **before** the first `POKEUSER`, never after.
pub fn register_armed(tid: i32, dbg: u64) {
    REGISTRY_DBG.store(dbg, Ordering::SeqCst);
    for cell in REGISTRY.iter() {
        if cell.load(Ordering::SeqCst) == tid {
            return;
        }
    }
    for cell in REGISTRY.iter() {
        if cell.compare_exchange(0, tid, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            return;
        }
    }
    REGISTRY_OVERFLOW.fetch_add(1, Ordering::SeqCst);
}

/// Drop `tid` from the registry. ⛔ Only ever call this after `DR7` has been **read back as zero**
/// on that thread. Removing it on "the poke returned Ok" would reintroduce the exact
/// silently-lethal disarm this crate already paid for once.
pub fn unregister_armed(tid: i32) {
    for cell in REGISTRY.iter() {
        let _ = cell.compare_exchange(tid, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

/// Is `tid` still on the possibly-armed list? Used by [`Watcher::attach`] to tell an arm that failed
/// *before* touching a register from one that failed *after*.
pub fn registry_holds(tid: i32) -> bool {
    REGISTRY.iter().any(|c| c.load(Ordering::SeqCst) == tid)
}

/// `(tids currently registered, threads that overflowed the table)`.
pub fn registry_state() -> (Vec<i32>, usize) {
    let v: Vec<i32> =
        REGISTRY.iter().map(|c| c.load(Ordering::SeqCst)).filter(|t| *t != 0).collect();
    (v, REGISTRY_OVERFLOW.load(Ordering::SeqCst))
}

/// Clear `DR0..DR3`, `DR6`, `DR7` on an already-stopped task with raw syscalls. `true` = every poke
/// returned success. ⛔ Signal-handler safe: no allocation, no formatting, no locks.
fn clear_dr_raw(tid: i32, dbg: u64) -> bool {
    let mut ok = true;
    for i in [0usize, 1, 2, 3, 6, 7] {
        let off = ptrace::dr_offset(dbg, i) as usize;
        let r = unsafe { libc::ptrace(ptrace::PTRACE_POKEUSER, tid, off, 0usize) };
        if r == -1 {
            ok = false;
        }
    }
    ok
}

/// ⭐⭐ **The last-ditch disarm, callable from a signal handler.** Walks [`REGISTRY`] and clears the
/// debug registers of every thread on it, then detaches.
///
/// The order is deliberate: **poke first**, interrupt only if the poke failed. A thread that is
/// already in a ptrace-stop (the trapping thread during a capture) takes the poke straight away, and
/// asking the kernel to stop it again first would only widen the window. A thread that is running
/// returns `ESRCH`, so it gets `PTRACE_INTERRUPT` and a **bounded** `waitpid` spin - bounded because
/// a handler that hangs leaves the operator with a wedged tool *and* a stopped client thread, which
/// is worse than the failure it was trying to prevent.
///
/// Returns how many threads were cleared. ⚠ Best-effort by construction: it is the path taken when
/// the process is already dying, and it is a mitigation for the crash, not a substitute for
/// [`Watcher::disarm`].
pub fn emergency_disarm() -> usize {
    let dbg = REGISTRY_DBG.load(Ordering::SeqCst);
    if dbg == 0 {
        return 0;
    }
    let mut cleared = 0usize;
    for cell in REGISTRY.iter() {
        let tid = cell.swap(0, Ordering::SeqCst);
        if tid == 0 {
            continue;
        }
        if clear_dr_raw(tid, dbg) {
            cleared += 1;
            unsafe { libc::ptrace(ptrace::PTRACE_DETACH, tid, 0usize, 0usize) };
            continue;
        }
        unsafe { libc::ptrace(ptrace::PTRACE_INTERRUPT, tid, 0usize, 0usize) };
        let mut st: i32 = 0;
        for _ in 0..50_000 {
            let r = unsafe { libc::waitpid(tid, &mut st, libc::WNOHANG | ptrace::WALL) };
            if r != 0 {
                break;
            }
            unsafe { libc::sched_yield() };
        }
        if clear_dr_raw(tid, dbg) {
            cleared += 1;
        }
        unsafe { libc::ptrace(ptrace::PTRACE_DETACH, tid, 0usize, 0usize) };
    }
    cleared
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The handlers
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐ **Catchable signals whose DEFAULT action TERMINATES this process** - i.e. every ordinary way
/// to die with `DR7` still set on the client.
///
/// ⛔ `SIGHUP` is the one that matters and the one that was missing: `historicaldocs/design/live-watch.md` §2.2a's
/// own workflow is *arm in one shell, poke the trigger in a second*, and the default `--hit-timeout`
/// is 120 s. A dropped ssh session or a closed terminal in that window delivered `SIGHUP`, whose
/// default action is terminate-without-unwinding, so neither `ResumeGuard::drop` nor
/// `Watcher::drop` ran and the client died of `SIGTRAP`. Measured 2 of 2, and inverted with
/// `SIGTERM` (child survives) - [`crate::controls::fatal_signals_disarm`].
pub const GRACEFUL_SIGNALS: [i32; 11] = [
    libc::SIGHUP,
    libc::SIGINT,
    libc::SIGQUIT,
    libc::SIGTERM,
    libc::SIGUSR1,
    libc::SIGUSR2,
    libc::SIGALRM,
    libc::SIGVTALRM,
    libc::SIGPROF,
    libc::SIGXCPU,
    libc::SIGXFSZ,
];

/// Signals that mean **this process is already broken**. Setting a flag would be useless - there is
/// nobody left to read it - so the handler runs [`emergency_disarm`] with raw syscalls, restores the
/// default disposition and re-raises, which preserves the core dump and the exit status.
///
/// ⭐ `SIGABRT` is on this list for a specific reason named in review: Rust's allocation-failure
/// path calls `abort()`, which does **not** unwind, and the `--snapshot-on-hit` capture allocates
/// its buffers before the arm but then waits up to `--hit-timeout` with everything armed.
pub const FATAL_SIGNALS: [i32; 6] =
    [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGABRT, libc::SIGSYS];

extern "C" fn on_graceful(_sig: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
    if SIGNAL_COUNT.fetch_add(1, Ordering::SeqCst) >= 1 {
        emergency_disarm();
        unsafe { libc::_exit(130) };
    }
}

extern "C" fn on_fatal(sig: i32) {
    emergency_disarm();
    unsafe {
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// What [`install_signal_handlers`] managed to install. ⚠ Returned, and printed by both binaries: a
/// handler that silently failed to install is a lethal window nobody was told about.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SignalHandlers {
    pub graceful: Vec<i32>,
    pub fatal: Vec<i32>,
    pub failed: Vec<i32>,
}

impl SignalHandlers {
    pub fn summary(&self) -> String {
        format!(
            "SIGNALS  {} graceful + {} fatal handler(s) installed{} - a dropped terminal (SIGHUP), \
             a SIGQUIT or an abort() now leaves through a verified disarm instead of stranding an \
             armed debug register. ⛔ SIGKILL still cannot be caught.",
            self.graceful.len(),
            self.fatal.len(),
            if self.failed.is_empty() {
                String::new()
            } else {
                format!(", ⛔ {} FAILED: {:?}", self.failed.len(), self.failed)
            }
        )
    }
}

/// Install the handlers. Idempotent; safe to call more than once.
///
/// ⚠ Deliberately **not** handled: `SIGPIPE`, which the Rust runtime sets to `SIG_IGN`, so a write
/// to a closed pipe makes `println!` panic - and a panic *unwinds*, which runs `Watcher::drop` and
/// disarms. Installing a handler there would not improve on that and would change the semantics of
/// every write in the process. `SIGSTOP`/`SIGKILL` cannot be caught at all.
pub fn install_signal_handlers() -> SignalHandlers {
    let mut out = SignalHandlers { graceful: Vec::new(), fatal: Vec::new(), failed: Vec::new() };
    for sig in GRACEFUL_SIGNALS {
        let r = unsafe { libc::signal(sig, on_graceful as *const () as usize) };
        if r == libc::SIG_ERR {
            out.failed.push(sig);
        } else {
            out.graceful.push(sig);
        }
    }
    for sig in FATAL_SIGNALS {
        let r = unsafe { libc::signal(sig, on_fatal as *const () as usize) };
        if r == libc::SIG_ERR {
            out.failed.push(sig);
        } else {
            out.fatal.push(sig);
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct WatchSpec {
    /// **Absolute** host address. Translation from an eboot vaddr happens in the CLI, once, and is
    /// printed - see [`crate::addr`].
    pub addr: u64,
    pub len: u64,
    pub rw: Rw,
    pub slot: usize,
    pub follow_clone: bool,
    /// Translate a caught `RIP` into an eboot vaddr when it lands in the eboot's mapping.
    pub base: Option<u64>,
    /// Arm only these threads. `None` = **every** thread in `/proc/<pid>/task`, which is the only
    /// setting for which "no hit" is evidence about the process rather than about one thread.
    pub tids: Option<Vec<i32>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InsnCandidate {
    pub addr: u64,
    pub len: usize,
    pub text: String,
    pub bytes: String,
    pub writes_memory: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Hit {
    pub seq: usize,
    pub elapsed_ms: u128,
    pub tid: i32,
    pub comm: String,
    pub dr6: u64,
    /// The raw register at the trap. For a **write/rw** watch this is the instruction AFTER the
    /// store; for an **exec** breakpoint it is the instruction itself.
    pub rip: u64,
    pub rip_vaddr: Option<u64>,
    /// The instruction we believe performed the access.
    pub writer: Option<u64>,
    pub writer_vaddr: Option<u64>,
    pub writer_text: Option<String>,
    pub writer_confidence: &'static str,
    pub candidates: Vec<InsnCandidate>,
    /// The bytes at the watched address, read immediately after the trap.
    pub value_after: Option<String>,
    pub rsp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbx: u64,
    /// ⭐ **The trapping thread's whole integer register file at the trap** - every field of
    /// `PTRACE_GETREGS`, keyed by lowercase name ([`crate::ptrace::regs_map`]).
    ///
    /// The four fields above are the §2.2a argument idiom and stay where readers expect them; this
    /// is the same instant, complete, and in the shape `d1_replay::machine::reg_by_name` already
    /// consumes - so a later `--snapshot-on-hit` pairs a memory image with these values without
    /// anyone re-describing them.
    ///
    /// ⚠ These are the registers of **one** thread, at **one** instant. They say nothing about the
    /// other ~110 threads of the client, and nothing about memory: a register holding a pointer is
    /// evidence about the pointer, not about the object it names.
    /// ⛔ **Integer only** - no `xmm`/FP state. See [`crate::ptrace::regs_map`].
    pub regs: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchReport {
    pub pid: i32,
    pub addr: u64,
    pub addr_vaddr: Option<u64>,
    pub len: u64,
    pub rw: String,
    pub slot: usize,
    pub threads_armed: usize,
    pub threads_failed: Vec<String>,
    pub follow_clone: bool,
    pub clones_armed: usize,
    pub unattributed_sigtraps: usize,
    /// Stops seen for tasks this `Watcher` did not arm - see [`Watcher::run`]'s waitpid caveat.
    pub foreign_stops: usize,
    pub signals_forwarded: BTreeMap<i32, usize>,
    pub seconds: f64,
    pub stop_reason: String,
    pub hits: Vec<Hit>,
}

impl WatchReport {
    /// ⚠ The honest reading of zero hits. Absence is only evidence about the threads that were
    /// armed, for the window that was watched.
    pub fn summary(&self) -> String {
        if self.hits.is_empty() {
            format!(
                "NO HIT in {:.1}s ({}).  ⚠ This is evidence ONLY about the {} thread(s) armed at \
                 start{}. It is NOT evidence that nothing writes {:#x}.",
                self.seconds,
                self.stop_reason,
                self.threads_armed,
                if self.follow_clone {
                    format!(" plus {} armed on clone", self.clones_armed)
                } else {
                    " (threads created later were NOT armed - pass --follow-clone)".into()
                },
                self.addr
            )
        } else {
            format!("{} hit(s) in {:.1}s ({})", self.hits.len(), self.seconds, self.stop_reason)
        }
    }
}

/// Wall-clock nanoseconds since the Unix epoch - the trap instant, and the anchor every other
/// timestamp in a paired snapshot is measured *after*.
pub fn unix_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// The state the wait loop carries between turns. Split out so [`Watcher::run`] and
/// [`Watcher::catch_one_with`] share **one** event handler rather than two that can drift.
struct Pump {
    alive: Vec<i32>,
    unattributed: usize,
    foreign: usize,
    forwarded: BTreeMap<i32, usize>,
    all_exited: bool,
}

impl Pump {
    fn new(alive: Vec<i32>) -> Self {
        Self { alive, unattributed: 0, foreign: 0, forwarded: BTreeMap::new(), all_exited: false }
    }
}

/// ⭐ **What `DR7` actually read back, per thread, after the clear.** The evidence that a capture
/// ran with nothing armed - and it is a read-back, never "the poke returned Ok".
#[derive(Debug, Clone, serde::Serialize)]
pub struct DisarmProof {
    /// FALSE means this is a *census* (read-only), not a disarm - see [`Watcher::catch_one_with`].
    pub attempted: bool,
    /// `(tid, DR7 as read back)`. Every armed thread appears here or in `failures`; neither list is
    /// allowed to be silently short.
    pub threads: Vec<(i32, u64)>,
    pub failures: Vec<String>,
    pub elapsed_ns: u64,
    /// ⭐ The wall-clock bound this disarm ran under, in milliseconds.
    pub deadline_ms: f64,
    /// ⛔ TRUE if the bound expired with threads left to visit. The capture is then REFUSED - see
    /// [`Watcher::catch_one_with`] - because a stop we cannot bound is a stop we cannot spend.
    pub deadline_fired: bool,
    /// Threads still on the disarm list afterwards: those that could not be proven clear. **Not
    /// detached** - we stay their tracer, so a debug exception arrives as a ptrace-stop we absorb
    /// rather than as a fatal `SIGTRAP`.
    pub still_tracked: Vec<i32>,
}

impl DisarmProof {
    /// ⚠ Requires at least one thread. An **empty** proof is not "all clear" - it is "we measured
    /// nothing", which is the failure shape this project keeps paying for.
    pub fn all_clear(&self) -> bool {
        self.failures.is_empty()
            && !self.threads.is_empty()
            && self.still_tracked.is_empty()
            && !self.deadline_fired
            && self.threads.iter().all(|(_, d)| *d == 0)
    }
    pub fn armed_threads(&self) -> Vec<(i32, u64)> {
        self.threads.iter().copied().filter(|(_, d)| *d != 0).collect()
    }
}

/// The trapping thread, held stopped, with the safety step already done and measured.
///
/// ⚠⚠ **One thread of N is stopped.** `threads_total - 1` threads kept running for the whole of the
/// caller's work, so anything read during it is a smear, not an instant. That number is here so a
/// consumer can put it in its own artifact instead of taking it on trust.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrapStop {
    pub tid: i32,
    pub comm: String,
    /// `CLOCK_REALTIME` at the trap. Every other stamp a consumer records is relative to this.
    pub trap_unix_nanos: u64,
    /// How long step 2 took, in nanoseconds - the width of the window in which a crash is lethal.
    pub debug_registers_cleared_ns: u64,
    pub disarm: DisarmProof,
    /// ⭐ `/proc/<pid>/task` counted **inside the stop, after the disarm**. `None` means the census
    /// FAILED, which is not zero: a `0` here once rendered as "0 thread(s) of 0 KEPT RUNNING", i.e.
    /// a measurement failure printed as the atomic-freeze claim.
    pub threads_total: Option<usize>,
    /// The same count taken when the wait loop started - up to `--hit-timeout` earlier. Kept so the
    /// two instants can be compared, never used as `threads_total`.
    pub threads_total_at_arm: Option<usize>,
    pub threads_armed_at_hit: usize,
}

impl TrapStop {
    /// Threads that kept running throughout: `total - 1`, or `None` if the census failed.
    pub fn threads_running(&self) -> Option<usize> {
        self.threads_total.map(|t| t.saturating_sub(1))
    }
}

/// The result of [`Watcher::catch_one`]: the usual report, plus the hit, the stop and whatever the
/// caller's closure produced. `hit: None` is a **timeout**, not a finding - read `report.summary()`.
pub struct Caught<T> {
    pub report: WatchReport,
    pub hit: Option<Hit>,
    pub stop: Option<TrapStop>,
    /// `CLOCK_REALTIME` at the instant the thread was released.
    pub resumed_unix_nanos: Option<u64>,
    pub value: Option<T>,
}

/// Resume-and-detach the held thread, **whatever happens** - normal return, `?`, or a panic during
/// the caller's capture. `Drop` is the mechanism precisely because the interesting failure is the
/// one nobody wrote a code path for.
struct ResumeGuard {
    tid: i32,
    dbg: u64,
    done: bool,
    at: u64,
}

impl ResumeGuard {
    fn release(&mut self) -> u64 {
        if self.done {
            return self.at;
        }
        self.done = true;
        // ⛔ Belt and braces: clear the debug registers again before detaching. `disarm_for_capture`
        // has already verified them zero on this thread, but a detach with a live DR is the one
        // action measured to KILL a target ([`crate::controls::disarm_is_load_bearing`] leg A), and
        // six pokes on an already-stopped thread cost microseconds. This is also what makes the
        // control leg that deliberately SKIPS the disarm safe to run.
        for slot in 0..4 {
            let _ = ptrace::pokeuser(self.tid, ptrace::dr_offset(self.dbg, slot), 0);
        }
        let _ = ptrace::pokeuser(self.tid, ptrace::dr_offset(self.dbg, 6), 0);
        let _ = ptrace::pokeuser(self.tid, ptrace::dr_offset(self.dbg, 7), 0);
        // Let any queued SIGTRAP be delivered into a signal-delivery-stop, where DETACH suppresses
        // it. See `sigtrap_pending` - this is the fix for the intermittently-lethal disarm.
        let mut drained = 0;
        while sigtrap_pending(self.tid) && drained < 16 {
            if ptrace::cont(self.tid, 0).is_err() {
                break;
            }
            match ptrace::wait_for(self.tid, Duration::from_millis(500)) {
                Ok(st) if st.stopped().is_some() => drained += 1,
                _ => break,
            }
        }
        let _ = ptrace::detach(self.tid);
        self.at = unix_nanos();
        self.at
    }
}

impl Drop for ResumeGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// An armed watchpoint. **Disarms in `Drop`** - including on panic and on the `?` early return of
/// any caller - because a debug register left set on a process whose tracer has gone is a `SIGTRAP`
/// with no handler.
pub struct Watcher {
    pid: i32,
    dbg: u64,
    spec: WatchSpec,
    dr7: u64,
    /// Every tid we are the tracer of that **may** carry a live debug register - the disarm list.
    ///
    /// ⭐ This deliberately includes threads whose arm FAILED after the first poke landed. Review
    /// found the opposite: a thread that failed its read-back was pushed to `threads_failed` and
    /// never to `armed`, so no disarm path in the program would ever visit it again, and it carried
    /// a live `DR7` for the life of the process. "It failed" is not "it is clear".
    armed: Vec<i32>,
    /// The subset of `armed` whose `DR0`/`DR7` **read back** the requested values. This is what
    /// "armed N thread(s)" means in every report: a count that included unverified threads would
    /// turn a broken arm into a confident false negative.
    verified: Vec<i32>,
    disarmed: bool,
    pub threads_failed: Vec<String>,
    pub clones_armed: usize,
    /// ⛔ **Controls only.** Makes `arm_stopped` fail *after* its pokes have landed, which is the
    /// only way to exercise the armed-but-unverified path deliberately. Set exclusively by
    /// [`Watcher::attach_injecting_arm_failure`].
    inject_arm_failure: bool,
    /// ⭐ The wall-clock bound on [`Watcher::disarm_for_capture`] - the step that brackets the
    /// capture and holds the trapping thread stopped. Review measured the unbounded version at up
    /// to `6 attempts x 500 ms x 47 threads = ~141 s`, against a design that claims "about 0.7 ms".
    /// See that method for what happens when this expires (it refuses; it does not abandon).
    disarm_deadline: Duration,
}

/// The default bound on the capture-time disarm. ⭐ Measured: 0.68 ms for 111 threads, so this is
/// ~2,200x headroom - wide enough that it never fires on a healthy client, narrow enough that a
/// thread wedged in a blocking syscall cannot silently turn a 4.6 ms stop into a two-minute one.
pub const DEFAULT_DISARM_DEADLINE: Duration = Duration::from_millis(1500);

/// Is a `SIGTRAP` sitting **queued but not yet delivered** for this task?
///
/// ⛔ This is the fix for a bug that made `disarm` intermittently lethal, and it is worth stating
/// in full because the symptom was maximally misleading: `DR7` read back **zero**, the disarm
/// verified, and the child still died of `SIGTRAP` about a third of the time.
///
/// The cause: after the last hit we `PTRACE_CONT` the thread, it immediately stores again and the
/// CPU raises another `SIGTRAP`. If our `PTRACE_INTERRUPT` wins the race, the thread stops in a
/// *group-stop* with that `SIGTRAP` still **queued** - never reported to us, so nothing to suppress
/// with `PTRACE_DETACH`'s signal argument. `PTRACE_DETACH` then leaves it pending, the tracee
/// resumes untraced, and the default action for `SIGTRAP` is *terminate with core*.
///
/// A signal in a *signal-delivery-stop* has already been dequeued and does not appear here - which
/// is exactly right, because that one **is** suppressible by detaching with signal 0. So this
/// predicate names precisely the dangerous case.
fn sigtrap_pending(tid: i32) -> bool {
    const SIGTRAP_BIT: u64 = 1 << (libc::SIGTRAP as u64 - 1);
    let Ok(status) = std::fs::read_to_string(format!("/proc/{tid}/status")) else { return false };
    status
        .lines()
        .filter(|l| l.starts_with("SigPnd:") || l.starts_with("ShdPnd:"))
        .filter_map(|l| u64::from_str_radix(l.split_whitespace().nth(1)?, 16).ok())
        .any(|m| m & SIGTRAP_BIT != 0)
}

fn comm_of(tid: i32) -> String {
    std::fs::read_to_string(format!("/proc/{tid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "?".into())
}

pub fn task_ids(pid: i32) -> Result<Vec<i32>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(format!("/proc/{pid}/task"))
        .with_context(|| format!("listing /proc/{pid}/task"))?
    {
        if let Ok(t) = e?.file_name().to_string_lossy().parse::<i32>() {
            out.push(t);
        }
    }
    out.sort_unstable();
    if out.is_empty() {
        bail!("/proc/{pid}/task is empty - the process is gone");
    }
    Ok(out)
}

impl Watcher {
    /// Seize every thread, arm the slot, and let them all run again.
    ///
    /// `dbg` is the `u_debugreg` offset - pass the value [`crate::ptrace::derive_debugreg_offset`]
    /// measured, never a literal.
    pub fn attach(pid: i32, spec: WatchSpec, dbg: u64) -> Result<Self> {
        Self::attach_inner(pid, spec, dbg, false)
    }

    /// ⛔ **Controls only** - [`Watcher::attach`] with every arm made to fail *after* its pokes have
    /// landed. It exists so [`crate::controls::a_failed_arm_is_still_disarmed`] can measure the
    /// armed-but-unverified path instead of asserting it, and it is not an operational mode.
    pub fn attach_injecting_arm_failure(pid: i32, spec: WatchSpec, dbg: u64) -> Result<Self> {
        Self::attach_inner(pid, spec, dbg, true)
    }

    fn attach_inner(pid: i32, spec: WatchSpec, dbg: u64, inject: bool) -> Result<Self> {
        ptrace::check_alignment(spec.addr, spec.len)?;
        let dr7 = ptrace::dr7_for(spec.slot, spec.rw, spec.len)?;
        let mut w = Self {
            pid,
            dbg,
            spec,
            dr7,
            armed: Vec::new(),
            verified: Vec::new(),
            disarmed: false,
            threads_failed: Vec::new(),
            clones_armed: 0,
            inject_arm_failure: inject,
            disarm_deadline: DEFAULT_DISARM_DEADLINE,
        };
        let options = if w.spec.follow_clone { ptrace::PTRACE_O_TRACECLONE } else { 0 };
        let tids = match w.spec.tids.clone() {
            Some(t) if !t.is_empty() => t,
            _ => task_ids(pid)?,
        };
        for tid in tids {
            match w.arm_one(tid, options) {
                Ok(()) => {
                    w.armed.push(tid);
                    w.verified.push(tid);
                }
                Err(e) => {
                    // ⭐ The distinction the registry exists to make: did this arm fail BEFORE or
                    // AFTER a debug register was written? `arm_stopped` registers the tid before its
                    // first poke and only `clear_and_verify` removes it, so a tid still on the list
                    // may be armed and MUST stay on the disarm list.
                    if registry_holds(tid) {
                        w.armed.push(tid);
                        w.threads_failed.push(format!(
                            "tid {tid} ({}): {e} - ⛔ ARMED-BUT-UNVERIFIED: a debug register may be \
                             live on it, so it is retained for the disarm path rather than dropped",
                            comm_of(tid)
                        ));
                    } else {
                        w.threads_failed.push(format!(
                            "tid {tid} ({}): {e} (no register was written - nothing to disarm)",
                            comm_of(tid)
                        ));
                    }
                }
            }
        }
        if w.verified.is_empty() {
            bail!(
                "armed 0 of {} thread(s) - the watchpoint is NOT set, and a run from here would \
                 report a false 'nothing wrote it'. First failure: {}",
                w.threads_failed.len(),
                w.threads_failed.first().map(String::as_str).unwrap_or("(none recorded)")
            );
            // ⭐ `w` is dropped here, and its `Drop` disarms every tid on `armed` - including any
            // retained above. A refusal must not itself strand a register.
        }
        Ok(w)
    }

    /// Override the bound on the capture-time disarm. ⛔ `Duration::ZERO` is the inversion leg of
    /// [`crate::controls::disarm_is_bounded_and_multithreaded`]; it is not an operational setting.
    pub fn set_disarm_deadline(&mut self, d: Duration) {
        self.disarm_deadline = d;
    }

    /// Seize (does **not** stop), interrupt (one brief stop), poke, verify by read-back, continue.
    fn arm_one(&self, tid: i32, options: u64) -> Result<()> {
        ptrace::seize(tid, options)?;
        ptrace::interrupt(tid)?;
        let st = ptrace::wait_for(tid, Duration::from_secs(2))?;
        if st.stopped().is_none() {
            bail!("task did not stop for arming (status {:#x})", st.raw);
        }
        self.arm_stopped(tid)?;
        ptrace::cont(tid, 0)
    }

    /// The poke itself, on an already-stopped task. Split out because a clone-stop arrives already
    /// stopped and must not be interrupted again.
    fn arm_stopped(&self, tid: i32) -> Result<()> {
        // ⭐⭐ BEFORE the first poke, never after. From here until a verified clear, this tid is
        // reachable from a signal handler ([`emergency_disarm`]) and from the retention branch in
        // `attach_inner`. Registering after a successful verify would leave the interesting window -
        // poke landed, verify failed - invisible to both.
        register_armed(tid, self.dbg);
        ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, self.spec.slot), self.spec.addr)?;
        ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, 6), 0)?;
        ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, 7), self.dr7)?;

        // ⭐ The read-back is the control that separates "armed and nothing wrote it" from "never
        // armed". Without it a silently-rejected poke is indistinguishable from a true negative - 
        // the exact shape of failure the UD2 tracer's guard check exists to prevent.
        let a = ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, self.spec.slot))?;
        let d = ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, 7))?;
        if a != self.spec.addr {
            bail!("DR{} read back {a:#x}, expected {:#x}", self.spec.slot, self.spec.addr);
        }
        if d & self.dr7 != self.dr7 {
            bail!("DR7 read back {d:#x}, which does not contain the requested {:#x}", self.dr7);
        }
        if self.inject_arm_failure {
            bail!(
                "INJECTED arm failure (controls only): the pokes LANDED and the read-back was made \
                 to fail, so this tid is armed-but-unverified"
            );
        }
        Ok(())
    }

    /// How many threads are armed **and verified**. ⚠ Not the length of the disarm list: a thread
    /// whose arm failed after the poke landed is on that list and is deliberately NOT counted here.
    pub fn armed_threads(&self) -> usize {
        self.verified.len()
    }

    /// Threads on the disarm list - verified or armed-but-unverified.
    pub fn tracked_threads(&self) -> usize {
        self.armed.len()
    }

    /// Wait for traps. Returns when `count` hits have been seen, the deadline expires, the target
    /// exits, or `SIGINT`/`SIGTERM` arrives.
    ///
    /// ⚠ **A `Watcher` owns `waitpid(-1)` for the whole process while it runs.** ptrace stops are
    /// delivered through the tracer's child-wait queue and there is no per-tracee wait, so this
    /// loop also **reaps the exit status of any unrelated child** of the same process. Two
    /// `Watcher`s running concurrently in one process steal each other's notifications (a false
    /// "no hit"), and a concurrent `Command::output()` can lose its own result. The `d1-live`
    /// binary has no other children while watching; an embedder must serialise. Stops for tasks
    /// this `Watcher` did not arm are counted as `foreign_stops` and left strictly alone - that
    /// counter being non-zero is the symptom.
    pub fn run(
        &mut self,
        proc: &Proc,
        count: usize,
        max: Duration,
        mut on_hit: impl FnMut(&Hit),
    ) -> Result<WatchReport> {
        let started = Instant::now();
        let deadline = started + max;
        let mut hits: Vec<Hit> = Vec::new();
        let mut p = Pump::new(self.armed.clone());
        let stop_reason;

        loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                stop_reason = "interrupted".to_string();
                break;
            }
            if Instant::now() >= deadline {
                stop_reason = format!("--max-seconds {:.0} reached", max.as_secs_f64());
                break;
            }
            if hits.len() >= count {
                stop_reason = format!("--count {count} reached");
                break;
            }
            if p.all_exited {
                stop_reason = "every armed thread exited".to_string();
                break;
            }
            if let Some(hit) = self.pump(proc, &mut p, started, hits.len() + 1)? {
                // ⚠ `pump` leaves the trapping thread STOPPED - that is what makes
                // `catch_one_with` possible at all. This path continues it immediately, which is
                // the historical behaviour and the one every existing control measures.
                let tid = hit.tid;
                on_hit(&hit);
                hits.push(hit);
                ptrace::cont(tid, 0)?;
            }
        }

        Ok(self.report(&p, started, stop_reason, self.verified.len(), hits))
    }

    /// ⭐⭐ Catch ONE trap and **hold the thread stopped** while a caller does work - with every
    /// debug register on every armed thread cleared and read back as zero FIRST.
    ///
    /// This is the primitive `d1-replay capture --snapshot-on-hit` is built on, and the order of
    /// its steps is the whole safety argument (`historicaldocs/design/live-watch.md` §6a.2):
    ///
    /// | # | step | armed? | trapping thread |
    /// |---|---|---|---|
    /// | 1 | trap, `PTRACE_GETREGS`, build the [`Hit`] | yes | stopped |
    /// | 2 | ⭐ clear `DR0..DR3`/`DR6`/`DR7` on **every** armed thread, verify by read-back, detach all but the trapping one | **no** | stopped |
    /// | 3 | run the caller's closure - arbitrarily long | no | stopped |
    /// | 4 | drain queued `SIGTRAP`, `PTRACE_DETACH` (which resumes) | no | released |
    ///
    /// ⛔ **The lethal window is steps 1-2, and nothing the caller writes can lengthen it.** The
    /// crate's governing rule is *a tracer that dies armed kills its target*
    /// ([`crate::controls::disarm_is_load_bearing`] leg A measures it, every run). Step 3 is the new,
    /// unbounded, panicking-capable work - and it runs with **nothing armed anywhere**, so a panic,
    /// an `abort()` or even a `SIGKILL` during it leaves a tracee that the kernel auto-detaches and
    /// that resumes. That is measured too, by
    /// [`crate::controls::panic_during_capture_is_survivable`], which panics on purpose in the middle
    /// of step 3 and requires the child to be alive, unarmed and still storing afterwards.
    ///
    /// Step 4 happens through a `Drop` guard, so it runs on the closure's `Err`, on `?`, and on a
    /// panic. The guard also re-clears the debug registers before detaching: belt and braces, six
    /// pokes on an already-stopped thread, and the one thing that must never be skipped.
    pub fn catch_one<T>(
        &mut self,
        proc: &Proc,
        max: Duration,
        f: impl FnOnce(&Hit, &TrapStop) -> Result<T>,
    ) -> Result<Caught<T>> {
        self.catch_one_with(proc, max, true, f)
    }

    /// [`Watcher::catch_one`], with the disarm step switchable.
    ///
    /// ⛔ `disarm_before_capture = false` exists for **one** reason: it is the other half of
    /// [`crate::controls::capture_runs_unarmed`]. A control that only ever reads `DR7 == 0` has no
    /// axis; this leg makes the same read return the armed value, so "zero" means *cleared* rather
    /// than *never set*. It is not an operational mode, and the registers are cleared anyway before
    /// the thread is resumed.
    pub fn catch_one_with<T>(
        &mut self,
        proc: &Proc,
        max: Duration,
        disarm_before_capture: bool,
        f: impl FnOnce(&Hit, &TrapStop) -> Result<T>,
    ) -> Result<Caught<T>> {
        let started = Instant::now();
        let deadline = started + max;
        let mut p = Pump::new(self.armed.clone());
        let threads_total = task_ids(self.pid).map(|v| v.len()).ok();
        let mut caught: Option<Hit> = None;
        let stop_reason;

        loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                stop_reason = "interrupted".to_string();
                break;
            }
            if Instant::now() >= deadline {
                stop_reason = format!("timeout {:.0}s reached with no hit", max.as_secs_f64());
                break;
            }
            if p.all_exited {
                stop_reason = "every armed thread exited".to_string();
                break;
            }
            if let Some(hit) = self.pump(proc, &mut p, started, 1)? {
                caught = Some(hit);
                stop_reason = "hit".to_string();
                break;
            }
        }

        let armed_at_hit = self.verified.len();
        let Some(hit) = caught else {
            let report = self.report(&p, started, stop_reason, armed_at_hit, Vec::new());
            return Ok(Caught { report, hit: None, stop: None, resumed_unix_nanos: None, value: None });
        };

        // ── step 2: the safety spine ────────────────────────────────────────────────────────────
        let trap_unix_nanos = unix_nanos();
        let t0 = Instant::now();
        let disarm = if disarm_before_capture {
            self.disarm_for_capture(hit.tid)
        } else {
            self.dr7_census(hit.tid)
        };
        // ⭐ Re-counted HERE, inside the stop and after the disarm, not at loop entry up to
        // `--hit-timeout` (120 s) earlier. Review named the build-1 version as the same
        // instant-vs-set confusion the design forbids elsewhere: `threads_running` was derived from
        // a census taken at a different instant than the one it describes.
        let threads_total_at_stop = task_ids(self.pid).map(|v| v.len()).ok();
        let stop = TrapStop {
            tid: hit.tid,
            comm: hit.comm.clone(),
            trap_unix_nanos,
            debug_registers_cleared_ns: t0.elapsed().as_nanos() as u64,
            disarm,
            threads_total: threads_total_at_stop,
            threads_total_at_arm: threads_total,
            threads_armed_at_hit: armed_at_hit,
        };

        // ⛔⛔ THE REFUSAL. If we cannot prove the whole process is unarmed inside the bound, we do
        // not spend the stop on the caller's work: release the thread, run the UNBOUNDED disarm, and
        // return an error naming what happened. Build 1 ran the capture regardless and recorded the
        // failure as a string in `notes` - i.e. a run that wrote a snapshot, exited 0, and left a
        // live debug register on a thread nothing would ever revisit.
        if disarm_before_capture && !stop.disarm.all_clear() {
            let mut guard = ResumeGuard { tid: hit.tid, dbg: self.dbg, done: false, at: 0 };
            guard.release();
            drop(guard);
            let leftover = self.disarm();
            bail!(
                "⛔ REFUSING THE CAPTURE: the pre-capture disarm could not be proven on every \
                 thread within {:.0} ms ({} proven clear, {} failure(s){}). The trapping thread has \
                 been released and a full unbounded disarm has run{}. Nothing was written. \
                 First failure: {}",
                stop.disarm.deadline_ms,
                stop.disarm.threads.len(),
                stop.disarm.failures.len(),
                if stop.disarm.deadline_fired { ", DEADLINE FIRED" } else { "" },
                if leftover.is_empty() {
                    " and cleared everything".to_string()
                } else {
                    format!(" and STILL could not clear {} thread(s): {}", leftover.len(), leftover.join("; "))
                },
                stop.disarm.failures.first().map(String::as_str).unwrap_or("(none recorded)")
            );
        }

        // ── step 3: the caller's work, with NOTHING armed ───────────────────────────────────────
        let mut guard = ResumeGuard { tid: hit.tid, dbg: self.dbg, done: false, at: 0 };
        let value = f(&hit, &stop);
        if !disarm_before_capture {
            // The census leg still leaves through a verified disarm: a detach with a live debug
            // register is what kills a target, and a control is not a licence to skip that.
            let _ = self.disarm_for_capture(hit.tid);
        }
        // ── step 4 ──────────────────────────────────────────────────────────────────────────────
        let resumed = guard.release();
        let value = value?;

        let report = self.report(&p, started, "hit".to_string(), armed_at_hit, vec![hit.clone()]);
        Ok(Caught {
            report,
            hit: Some(hit),
            stop: Some(stop),
            resumed_unix_nanos: Some(resumed),
            value: Some(value),
        })
    }

    fn report(
        &self,
        p: &Pump,
        started: Instant,
        stop_reason: String,
        threads_armed: usize,
        hits: Vec<Hit>,
    ) -> WatchReport {
        WatchReport {
            pid: self.pid,
            addr: self.spec.addr,
            addr_vaddr: self.spec.base.and_then(|b| self.spec.addr.checked_sub(b)),
            len: self.spec.len,
            rw: format!("{:?}", self.spec.rw).to_lowercase(),
            slot: self.spec.slot,
            threads_armed,
            threads_failed: self.threads_failed.clone(),
            follow_clone: self.spec.follow_clone,
            clones_armed: self.clones_armed,
            unattributed_sigtraps: p.unattributed,
            foreign_stops: p.foreign,
            signals_forwarded: p.forwarded.clone(),
            seconds: started.elapsed().as_secs_f64(),
            stop_reason,
            hits,
        }
    }

    /// One turn of the wait loop.
    ///
    /// ⚠ **On a hit this returns with the trapping thread still STOPPED.** Every caller must either
    /// `PTRACE_CONT` it ([`Watcher::run`]) or hand it to a [`ResumeGuard`]
    /// ([`Watcher::catch_one_with`]). Everything that is *not* our hit - clone events, group stops,
    /// foreign tasks, the target's own signals - is handled here and the thread is left running, so
    /// the two callers cannot disagree about any of it.
    fn pump(
        &mut self,
        proc: &Proc,
        p: &mut Pump,
        started: Instant,
        seq: usize,
    ) -> Result<Option<Hit>> {
        let Some(st) = ptrace::wait_any_nohang()? else {
            if p.alive.is_empty() {
                p.all_exited = true;
            } else {
                std::thread::sleep(Duration::from_micros(200));
            }
            return Ok(None);
        };
        let tid = st.pid;
        if st.exited() {
            p.alive.retain(|t| *t != tid);
            self.armed.retain(|t| *t != tid);
            self.verified.retain(|t| *t != tid);
            // A thread that has exited cannot carry anything; leaving it registered would make
            // `emergency_disarm` poke a dead tid, which is harmless but reads as a failure.
            unregister_armed(tid);
            return Ok(None);
        }
        let Some(sig) = st.stopped() else { return Ok(None) };
        let event = st.event();

        // Not one of ours. Do not continue it, do not read its registers - just record that it
        // happened, because a non-zero count means something else in this process is tracing.
        if !self.armed.contains(&tid) && event != ptrace::PTRACE_EVENT_CLONE {
            p.foreign += 1;
            return Ok(None);
        }

        if event == ptrace::PTRACE_EVENT_CLONE {
            // The new task is auto-seized and already stopped. Arm it before letting it run.
            let mut newtid: u64 = 0;
            let ok = unsafe {
                libc::ptrace(0x4201, tid, 0usize, &mut newtid as *mut u64) // PTRACE_GETEVENTMSG
            } != -1;
            if ok && newtid != 0 {
                let nt = newtid as i32;
                let _ = ptrace::wait_for(nt, Duration::from_millis(500));
                if self.arm_stopped(nt).is_ok() {
                    self.clones_armed += 1;
                    self.armed.push(nt);
                    self.verified.push(nt);
                    p.alive.push(nt);
                } else if registry_holds(nt) {
                    // Same rule as `attach_inner`: the pokes may have landed, so it goes on the
                    // disarm list. The build-1 code `cont`ed it and forgot it existed.
                    self.armed.push(nt);
                    p.alive.push(nt);
                    self.threads_failed.push(format!(
                        "clone tid {nt}: arming failed - ⛔ ARMED-BUT-UNVERIFIED, retained for the \
                         disarm path"
                    ));
                } else {
                    self.threads_failed
                        .push(format!("clone tid {nt}: arming failed (no register was written)"));
                }
                let _ = ptrace::cont(nt, 0);
            }
            ptrace::cont(tid, 0)?;
            return Ok(None);
        }

        if event == ptrace::PTRACE_EVENT_STOP {
            // Our own PTRACE_INTERRUPT stop reports SIGTRAP here; a genuine group-stop reports
            // one of the four stop signals and must stay stopped - PTRACE_LISTEN preserves
            // that, where PTRACE_CONT would resume a process the operator had suspended.
            if matches!(sig, libc::SIGSTOP | libc::SIGTSTP | libc::SIGTTIN | libc::SIGTTOU) {
                let _ = ptrace::listen(tid);
            } else {
                ptrace::cont(tid, 0)?;
            }
            return Ok(None);
        }

        if sig == libc::SIGTRAP {
            let dr6 = ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, 6)).unwrap_or(0);
            if dr6 & (1 << self.spec.slot) != 0 {
                let hit = self.build_hit(proc, tid, dr6, seq, started)?;
                let _ = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, 6), 0);
                return Ok(Some(hit)); // ⚠ LEFT STOPPED - see the doc comment.
            }
            p.unattributed += 1;
            ptrace::cont(tid, 0)?;
            return Ok(None);
        }

        // ⚠ Every other signal is the TARGET's, not ours. Swallowing it would change the
        // program we are trying to observe - shadPS4 handles SIGSEGV itself, and the UD2 tracer
        // raises SIGILL on purpose.
        *p.forwarded.entry(sig).or_default() += 1;
        ptrace::cont(tid, sig)?;
        Ok(None)
    }

    /// Clear every debug register on every armed thread, **verify by read-back**, and detach them
    /// all except `keep` (which stays in its ptrace-stop for the caller).
    ///
    /// After this returns, `self.armed` is empty and `self.disarmed` is set: the registers are
    /// provably clear, so [`Watcher::drop`] has nothing left to do and `keep`'s resume belongs to
    /// the [`ResumeGuard`]. The returned [`DisarmProof`] carries the DR7 value read back from every
    /// thread, which is the evidence - not the fact that the pokes returned `Ok`.
    fn disarm_for_capture(&mut self, keep: i32) -> DisarmProof {
        let t0 = Instant::now();
        let deadline = t0 + self.disarm_deadline;
        let mut threads = Vec::new();
        let mut failures = Vec::new();
        let mut unproven: Vec<i32> = Vec::new();
        let mut deadline_fired = false;

        for tid in std::mem::take(&mut self.armed) {
            if tid == keep {
                // Already in a ptrace-stop: poke straight through, no interrupt handshake. ⛔ NOT
                // subject to the deadline. This is the one thread that is certainly stopped, the
                // clear costs six pokes, and leaving IT armed is the single most dangerous outcome
                // available - it is the thread the trap already proved writes the watched address.
                match self.clear_and_verify(tid) {
                    Ok(v) => threads.push((tid, v)),
                    Err(e) => {
                        failures.push(format!("tid {tid} (the trapping thread): {e}"));
                        unproven.push(tid);
                    }
                }
                continue;
            }
            if Instant::now() >= deadline {
                deadline_fired = true;
                unproven.push(tid);
                failures.push(format!(
                    "⛔ tid {tid} was NOT VISITED: the {:.0} ms disarm deadline expired first. It \
                     is retained on the disarm list, still seized, and NOT detached",
                    self.disarm_deadline.as_secs_f64() * 1000.0
                ));
                continue;
            }
            let mut ok = false;
            let mut last = String::new();
            for attempt in 0..6 {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    deadline_fired = true;
                    last = format!(
                        "attempt {attempt}: the {:.0} ms disarm deadline expired mid-thread",
                        self.disarm_deadline.as_secs_f64() * 1000.0
                    );
                    break;
                }
                let _ = ptrace::interrupt(tid);
                // ⭐ The per-attempt wait is clipped to what is LEFT of the deadline, so six
                // attempts x 500 ms x N threads can no longer add up to minutes with the trapping
                // thread held stopped and the other threads still armed.
                if let Err(e) = ptrace::wait_for(tid, left.min(Duration::from_millis(500))) {
                    last = format!("attempt {attempt}: no stop to poke through ({e})");
                    continue;
                }
                match self.clear_and_verify(tid) {
                    Ok(v) => {
                        threads.push((tid, v));
                        // The queued-SIGTRAP drain, for exactly the reason `disarm` documents: a
                        // trap raised between the last CONT and our INTERRUPT is still pending, and
                        // detaching would leave it on an untraced process - which kills it.
                        let mut drained = 0;
                        while sigtrap_pending(tid) && drained < 16 {
                            if ptrace::cont(tid, 0).is_err() {
                                break;
                            }
                            match ptrace::wait_for(tid, Duration::from_millis(500)) {
                                Ok(st) if st.stopped().is_some() => drained += 1,
                                _ => break,
                            }
                        }
                        let _ = ptrace::detach(tid);
                        ok = true;
                        break;
                    }
                    Err(e) => last = format!("attempt {attempt}: {e}"),
                }
            }
            if !ok && std::path::Path::new(&format!("/proc/{tid}")).exists() {
                failures.push(format!(
                    "⛔ tid {tid} could NOT be proven disarmed ({last}) - it may still carry a live \
                     debug register, so it is RETAINED on the disarm list and NOT detached"
                ));
                // ⛔ NO `detach` here. Build 1 detached a thread it could not prove disarmed, which
                // is precisely leg A of `disarm_is_load_bearing` - the action measured to kill a
                // target. While we remain its tracer a debug exception arrives as a ptrace-stop we
                // absorb; the instant we detach it becomes a fatal SIGTRAP. Staying attached is the
                // strictly safer of the two, and `Watcher::disarm` gets another attempt at it.
                unproven.push(tid);
            }
        }

        // ⭐ Whatever could not be proven clear goes BACK on the disarm list, and `disarmed` stays
        // false, so `Watcher::disarm` (unbounded, and reached from `Drop`) tries again. Build 1 did
        // `mem::take` + `disarmed = true` unconditionally, so a thread that failed here was never
        // retried by anything, anywhere.
        self.armed = unproven;
        self.verified.retain(|t| self.armed.contains(t));
        self.disarmed = self.armed.is_empty();

        DisarmProof {
            attempted: true,
            threads,
            failures,
            elapsed_ns: t0.elapsed().as_nanos() as u64,
            deadline_ms: self.disarm_deadline.as_secs_f64() * 1000.0,
            deadline_fired,
            still_tracked: self.armed.clone(),
        }
    }

    /// Poke `DR0..DR3`, `DR6`, `DR7` to zero on an already-stopped task and return the value `DR7`
    /// **reads back**. A poke that returned `Ok` can still have been a no-op; the read-back is the
    /// control, exactly as in [`Watcher::disarm`].
    fn clear_and_verify(&self, tid: i32) -> Result<u64> {
        let mut err = None;
        for slot in 0..4 {
            if let Err(e) = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, slot), 0) {
                err = Some(e);
            }
        }
        for reg in [6usize, 7] {
            if let Err(e) = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, reg), 0) {
                err = Some(e);
            }
        }
        let back = ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, 7))
            .map_err(|e| anyhow::anyhow!("DR7 unreadable ({e}){}", err.map(|e| format!("; poke said {e}")).unwrap_or_default()))?;
        if back != 0 {
            bail!("DR7 still reads {back:#x} after being poked to zero");
        }
        // ⭐ ONLY here. The registry entry is removed on the strength of a read-back of zero, never
        // on "the poke returned Ok" - a stale-present entry costs a redundant clear, a stale-absent
        // one costs the client.
        unregister_armed(tid);
        Ok(back)
    }

    /// Read `DR7` without changing it. ⚠ Only the trapping thread is in a ptrace-stop, so every
    /// other thread reports a failure here - which is the honest answer, not a zero.
    fn dr7_census(&self, keep: i32) -> DisarmProof {
        let mut threads = Vec::new();
        let mut failures = Vec::new();
        for tid in self.armed.clone() {
            match ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, 7)) {
                Ok(v) => threads.push((tid, v)),
                Err(e) => failures.push(format!(
                    "tid {tid}{}: DR7 unreadable ({e}) - a running thread cannot be peeked",
                    if tid == keep { " (the trapping thread)" } else { "" }
                )),
            }
        }
        DisarmProof {
            attempted: false,
            threads,
            failures,
            elapsed_ns: 0,
            deadline_ms: self.disarm_deadline.as_secs_f64() * 1000.0,
            deadline_fired: false,
            still_tracked: self.armed.clone(),
        }
    }

    fn build_hit(&self, proc: &Proc, tid: i32, dr6: u64, seq: usize, t0: Instant) -> Result<Hit> {
        let regs = ptrace::getregs(tid)?;
        let rip = regs.rip;
        let vaddr = |a: u64| self.spec.base.and_then(|b| a.checked_sub(b)).filter(|v| *v < 0x1000_0000);

        let (writer, text, conf, candidates) = if self.spec.rw.rip_is_the_instruction() {
            let cands = decode_forward(proc, rip);
            let first = cands.first().cloned();
            (
                first.as_ref().map(|c| c.addr),
                first.as_ref().map(|c| c.text.clone()),
                if first.is_some() { "exact" } else { "unreadable" },
                cands,
            )
        } else {
            let cands = decode_backwards(proc, rip);
            let chosen = choose_writer(&cands);
            // ⚠ x86 is not uniquely decodable backwards: a shorter decode ending at the same byte
            // is usually a SUFFIX of the real instruction (`89 37` inside `48 89 37`), which is why
            // the longest memory-writing candidate wins.  The label says how much room for doubt
            // is left, and `d1-eboot disasm` - which is prologue-anchored - settles it for good.
            let writers = cands.iter().filter(|c| c.writes_memory).count();
            let conf = match (cands.len(), chosen.is_some(), writers) {
                (0, _, _) => "unreadable",
                (_, false, _) => "no-decode",
                (1, true, _) => "unique",
                (_, true, 1) => "preferred: the only candidate that writes memory",
                (_, true, 0) => "AMBIGUOUS: no candidate writes memory - check --rw and the target",
                (_, true, _) => "longest of several memory-writing decodes - confirm with d1-eboot disasm",
            };
            (
                chosen.map(|c| c.addr),
                chosen.map(|c| c.text.clone()),
                conf,
                cands,
            )
        };

        let value_after = proc
            .try_read(self.spec.addr, self.spec.len as usize)
            .map(|b| b.iter().map(|x| format!("{x:02x}")).collect::<String>());

        Ok(Hit {
            seq,
            elapsed_ms: t0.elapsed().as_millis(),
            tid,
            comm: comm_of(tid),
            dr6,
            rip,
            rip_vaddr: vaddr(rip),
            writer,
            writer_vaddr: writer.and_then(vaddr),
            writer_text: text,
            writer_confidence: conf,
            candidates,
            value_after,
            rsp: regs.rsp,
            rdi: regs.rdi,
            rsi: regs.rsi,
            rbx: regs.rbx,
            // ⭐ Free: `PTRACE_GETREGS` already ran at the top of this function, so the whole
            // register file costs one struct copy and NOT a second stop of the target. Widening the
            // armed window is the one thing this crate must never do casually.
            regs: ptrace::regs_map(&regs),
        })
    }

    /// Clear `DR7` (which disables every slot at once), clear `DR0..DR3`, then detach.
    ///
    /// ⛔ **This is verified, not fired-and-forgotten.** `PTRACE_POKEUSER` requires the tracee to be
    /// in a ptrace-stop, so a missed stop handshake makes every poke fail with `ESRCH` - and on this
    /// kernel a breakpoint that survives the detach kills the target with `SIGTRAP`
    /// ([`crate::controls::disarm_is_load_bearing`] measures exactly that). An early version
    /// swallowed those errors with `let _ =` and produced an intermittently lethal disarm. So each
    /// thread is retried until `DR7` **reads back zero**, and anything left over is reported loudly
    /// rather than silently.
    ///
    /// Returns one message per thread that could not be proven disarmed. Idempotent.
    pub fn disarm(&mut self) -> Vec<String> {
        if self.disarmed {
            return Vec::new();
        }
        self.disarmed = true;
        let mut failures = Vec::new();
        for tid in std::mem::take(&mut self.armed) {
            let mut last = String::new();
            let mut ok = false;
            for attempt in 0..6 {
                let _ = ptrace::interrupt(tid);
                match ptrace::wait_for(tid, Duration::from_millis(500)) {
                    Ok(_) => {}
                    Err(e) => {
                        last = format!("attempt {attempt}: no stop to poke through ({e})");
                        continue;
                    }
                }
                let mut err = None;
                for slot in 0..4 {
                    if let Err(e) = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, slot), 0) {
                        err = Some(e);
                    }
                }
                if let Err(e) = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, 6), 0) {
                    err = Some(e);
                }
                if let Err(e) = ptrace::pokeuser(tid, ptrace::dr_offset(self.dbg, 7), 0) {
                    err = Some(e);
                }
                // The read-back IS the control: a poke that returned Ok can still have been a
                // no-op, and "the register is clear" is the property we actually need.
                match ptrace::peekuser(tid, ptrace::dr_offset(self.dbg, 7)) {
                    Ok(0) => {
                        // ⛔ Clearing DR7 is not enough: a SIGTRAP raised between the last CONT and
                        // our INTERRUPT can still be QUEUED, and PTRACE_DETACH would leave it
                        // pending on an untraced process - which kills it. Let each queued one be
                        // delivered into a signal-delivery-stop, where detaching with signal 0
                        // suppresses it. See `sigtrap_pending`.
                        let mut drained = 0;
                        while sigtrap_pending(tid) && drained < 16 {
                            if ptrace::cont(tid, 0).is_err() {
                                break;
                            }
                            match ptrace::wait_for(tid, Duration::from_millis(500)) {
                                Ok(st) if st.stopped().is_some() => drained += 1,
                                _ => break,
                            }
                        }
                        unregister_armed(tid);
                        ok = true;
                        break;
                    }
                    Ok(v) => last = format!("attempt {attempt}: DR7 still reads {v:#x}"),
                    Err(e) => {
                        last = format!("attempt {attempt}: DR7 unreadable ({e}){}",
                                       err.map(|e| format!("; poke said {e}")).unwrap_or_default())
                    }
                }
            }
            if ok {
                let _ = ptrace::detach(tid);
            } else if std::path::Path::new(&format!("/proc/{tid}")).exists() {
                // ⛔ Last ditch, with raw syscalls and no stop handshake: if the thread happens to
                // be in a ptrace-stop right now this lands, and it costs six failed pokes if not.
                let raw = clear_dr_raw(tid, self.dbg);
                if raw {
                    unregister_armed(tid);
                }
                failures.push(format!(
                    "⛔ tid {tid} could NOT be proven disarmed ({last}) - it may still carry a live \
                     debug register. A raw last-ditch clear {}. Detaching anyway (this is the \
                     terminal path: the kernel would auto-detach at process exit regardless); if it \
                     takes SIGTRAP it will die.",
                    if raw { "REPORTED SUCCESS but was not read back" } else { "also failed" }
                ));
                let _ = ptrace::detach(tid);
            } else {
                // A tid that has exited needs no disarm and is not a failure.
                unregister_armed(tid);
            }
        }
        self.verified.clear();
        failures
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for f in self.disarm() {
            eprintln!("d1-live: {f}");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Naming the instruction
// ─────────────────────────────────────────────────────────────────────────────────────────────────

fn decode_at(bytes: &[u8], ip: u64) -> Option<(usize, String, bool)> {
    use iced_x86::{Decoder, DecoderOptions, Formatter, IntelFormatter, OpKind};
    let mut dec = Decoder::with_ip(64, bytes, ip, DecoderOptions::NONE);
    let insn = dec.decode();
    if insn.is_invalid() {
        return None;
    }
    let mut f = IntelFormatter::new();
    let mut s = String::new();
    f.format(&insn, &mut s);
    let writes = (0..insn.op_count()).any(|i| insn.op_kind(i) == OpKind::Memory)
        && insn.op0_kind() == OpKind::Memory;
    Some((insn.len(), s, writes))
}

/// Every instruction start in the 15 bytes before `rip` that decodes to end **exactly** at `rip`.
/// x86 is not uniquely decodable backwards; the caller is told so.
fn decode_backwards(proc: &Proc, rip: u64) -> Vec<InsnCandidate> {
    let window = 15usize;
    let Some(lo) = rip.checked_sub(window as u64) else { return Vec::new() };
    let Some(buf) = proc.try_read(lo, window) else { return Vec::new() };
    let mut out = Vec::new();
    for back in 1..=window {
        let start = rip - back as u64;
        let off = window - back;
        if let Some((len, text, writes)) = decode_at(&buf[off..], start) {
            if len == back {
                out.push(InsnCandidate {
                    addr: start,
                    len,
                    text,
                    bytes: buf[off..off + len].iter().map(|b| format!("{b:02x}")).collect(),
                    writes_memory: writes,
                });
            }
        }
    }
    out
}

fn decode_forward(proc: &Proc, rip: u64) -> Vec<InsnCandidate> {
    let Some(buf) = proc.try_read(rip, 16) else { return Vec::new() };
    match decode_at(&buf, rip) {
        Some((len, text, writes)) => vec![InsnCandidate {
            addr: rip,
            len,
            text,
            bytes: buf[..len].iter().map(|b| format!("{b:02x}")).collect(),
            writes_memory: writes,
        }],
        None => Vec::new(),
    }
}

/// Prefer the candidate that actually writes memory; among equals prefer the **longest** encoding,
/// which is the conventional backward-decode heuristic (a shorter decode ending at the same byte is
/// usually a suffix of the real instruction).
fn choose_writer(cands: &[InsnCandidate]) -> Option<&InsnCandidate> {
    cands
        .iter()
        .filter(|c| c.writes_memory)
        .max_by_key(|c| c.len)
        .or_else(|| cands.iter().max_by_key(|c| c.len))
}

#[cfg(test)]
mod tests {
    use crate::ptrace::{
        check_alignment, dr7_for, len_bits, regs_map, Rw, GP_REG_NAMES, REG_NAMES,
    };

    /// The axis: **the register file is copied field-by-name, and no name is spelled twice.**
    /// A duplicated key would make one field silently vanish and the map still look plausible -
    /// exactly the shape of failure this project keeps paying for. Inverted below: the map's length
    /// must equal the name list's, so a collision FAILS instead of shrinking quietly.
    #[test]
    fn regs_map_is_complete_and_correctly_keyed() {
        let mut r: libc::user_regs_struct = unsafe { std::mem::zeroed() };
        let m = regs_map(&r);
        assert_eq!(m.len(), REG_NAMES.len(), "a duplicated key would shrink this map");
        for n in REG_NAMES {
            assert!(m.contains_key(n), "{n} is missing from regs_map");
        }
        // Every GP name must be one of the captured names - otherwise a consumer asking for a
        // general register gets `None` and reads it as "the register was not set".
        for n in GP_REG_NAMES {
            assert!(REG_NAMES.contains(&n), "{n} is not a captured register name");
        }
        // Field → name pairing, for the four a caller is most likely to quote. A copy-paste swap
        // (the `rsi`/`rdi` pair is adjacent in the struct) fails here.
        r.rdi = 0xD1;
        r.rsi = 0x51;
        r.rip = 0x1234_5678;
        r.fs_base = 0xFB;
        let m = regs_map(&r);
        assert_eq!(m["rdi"], 0xD1);
        assert_eq!(m["rsi"], 0x51);
        assert_eq!(m["rip"], 0x1234_5678);
        assert_eq!(m["fs_base"], 0xFB);
        assert_eq!(m["rax"], 0, "an untouched field must stay zero, not inherit a neighbour");
    }

    /// The axis: the LEN field is not monotonic, and a hand-written table gets 4 and 8 backwards.
    /// Inverted below by asserting the values that would result from the "obvious" ordering FAIL.
    #[test]
    fn len_encoding_is_the_sdm_one_not_the_intuitive_one() {
        assert_eq!(len_bits(1).unwrap(), 0b00);
        assert_eq!(len_bits(2).unwrap(), 0b01);
        assert_eq!(len_bits(4).unwrap(), 0b11);
        assert_eq!(len_bits(8).unwrap(), 0b10);
        // The inversion: an intuitive 0b10 == 4 / 0b11 == 8 table would make these equal.
        assert_ne!(len_bits(4).unwrap(), 0b10);
        assert_ne!(len_bits(8).unwrap(), 0b11);
        // And a length the hardware cannot express is refused rather than rounded.
        assert!(len_bits(3).is_err());
        assert!(len_bits(16).is_err());
    }

    #[test]
    fn dr7_encodes_slot_rw_and_len() {
        // slot 0, write, 8 bytes: L0|G0 (0b11) | LE|GE (0x300) | RW0=01 | LEN0=10.
        // ⚠ BOTH enable bits, not just L0: Linux's decode_dr7() arms the slot on `(dr7 >> 2*i) & 3`,
        // so either bit alone works - but a debug register that is only *locally* enabled is not
        // guaranteed to survive a context switch on every CPU, and shadPS4's threads are scheduled
        // constantly.  A watch that silently stops being armed is the worst failure this tool has.
        assert_eq!(dr7_for(0, Rw::Write, 8).unwrap(), 0x0009_0303);
        assert_eq!(dr7_for(0, Rw::Write, 8).unwrap() & 0b11, 0b11);
        // slot 1 shifts the enables by 2 and the control field by 4.
        let d = dr7_for(1, Rw::ReadWrite, 4).unwrap();
        assert_eq!(d & 0b1100, 0b1100); // L1|G1
        assert_eq!(d & 0b0011, 0b0000); // …and slot 0 is untouched
        assert_eq!((d >> 20) & 0b11, 0b11); // RW1 = read/write
        assert_eq!((d >> 22) & 0b11, 0b11); // LEN1 = 4 bytes
        // Execute breakpoints must be one byte - refused, not narrowed.
        assert!(dr7_for(0, Rw::Exec, 8).is_err());
        assert!(dr7_for(0, Rw::Exec, 1).is_ok());
        assert!(dr7_for(4, Rw::Write, 8).is_err());
    }

    /// The axis: an unaligned watch would silently cover different bytes than the caller named.
    #[test]
    fn unaligned_data_watch_is_refused() {
        assert!(check_alignment(0x1000, 8).is_ok());
        assert!(check_alignment(0x1008, 8).is_ok());
        assert!(check_alignment(0x1004, 8).is_err()); // the failing half of the axis
        assert!(check_alignment(0x1004, 4).is_ok());
        assert!(check_alignment(0x1002, 4).is_err());
        assert!(check_alignment(0x1003, 1).is_ok()); // 1 byte is always aligned
    }

    /// The axis: the trap-vs-fault distinction decides whether `RIP` names the writer.
    #[test]
    fn only_exec_breakpoints_report_the_instruction_in_rip() {
        assert!(Rw::Exec.rip_is_the_instruction());
        assert!(!Rw::Write.rip_is_the_instruction());
        assert!(!Rw::ReadWrite.rip_is_the_instruction());
    }
}
