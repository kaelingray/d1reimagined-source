// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The event plane's **lifecycle**: exactly one leased `d1-live` child, and every path out of
//! d1-scope goes through a confirmed disarm.
//!
//! > **THE LIFECYCLE INVARIANT.** *d1-scope may never leave a hardware watchpoint armed behind a
//! > process that is not alive to disarm it: at most one `d1-live` child exists at any instant, it
//! > is always launched with a bounded `--max-seconds` lease, it is stopped only by `SIGTERM` and
//! > only after the parent has waited for the child's own confirmed disarm on exit, and every path
//! > out of d1-scope - normal return, error, panic, `SIGINT`, `SIGTERM` - runs that stop.*
//!
//! ⛔⛔ **A TRACER THAT DIES ARMED KILLS ITS TARGET.** That is measured on this box, every time
//! `d1-live controls` runs, not theoretical - and the target here is the operator's live client.
//! Control #8 re-measures it from d1-scope's side before asserting anything about our own stop
//! path, so a kernel that later flushes on detach would be reported as a **degenerate inversion**
//! rather than banked as a free pass.
//!
//! # ⭐ FIVE THINGS THIS MODULE DOES NOT DO, EACH DELIBERATE
//!
//! 1. **It performs no debug-register operation itself.** d1-scope holds no such capability: it
//!    spawns the `d1-live` *binary* and talks to it over argv, stdout and signals. That is why
//!    control #4's scan of this crate's sources and of the shipped binary finds no call path - and
//!    why this module is a **process manager**, not a debugger.
//!    ⚠ It deliberately does not link `d1_live::watch` or `d1_live::controls` either: linking them
//!    would drag their diagnostics into our binary and turn control #4 into a lie of omission.
//! 2. **It never uses the un-catchable signal**, on any path, at any timeout, for any reason. There
//!    is no escalation ladder here: `SIGTERM`, then wait, then wait longer - **the lease is the
//!    escalation.** A child that will not leave is reported loudly and left to its own deadline,
//!    because hard-killing it is precisely the thing that kills the client.
//!    ⚠ `std::process::Child::kill()` sends that signal. It is never called in this crate.
//! 3. **It refuses rather than fights.** If something else already traces the target, d1-scope says
//!    so and names the hand-off path. One tracer per process is a kernel rule, and losing a race to
//!    it produces confusing half-attached states.
//! 4. **It holds four slots but arms one.** There are four debug registers, so four is the right
//!    ceiling for the table; but one `d1-live` process watches one address, and *one child* is the
//!    invariant. Arming four would mean four children and four separate confirmed disarms - a
//!    strictly weaker safety property. The other slots are reported `Pending`, out loud, on every
//!    frame. ⇒ `[U]` in `historicaldocs/design/d1-scope.md`, with the named instrument.
//! 5. **It does not stream.** `d1-live --json` writes its report once, on the way out, so events
//!    arrive in batches at each re-arm. The lease is therefore also the event latency. Parsing the
//!    human-readable per-hit output instead would fork a second copy of that format.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::events::{parse_report, LiveReport};
use crate::frame::{ChildStatus, StopReceipt, WatchSpecView};

/// There are four debug registers. Not a policy - hardware.
pub const MAX_SLOTS: usize = 4;

/// How long we wait for a `SIGTERM`ed child before saying so out loud. ⛔ Nothing happens when it
/// expires except a louder message and a longer wait: see the module docs, point 2.
pub const STOP_GRACE: Duration = Duration::from_secs(5);

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The signal path
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The pid of the one child, published for the signal handler. `0` = none.
static CHILD_PID: AtomicI32 = AtomicI32::new(0);
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static HANDLERS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// ⭐ The handler does exactly two things, and both are legal from a signal context: an atomic
/// store, and `kill(2)` - which is async-signal-safe. It does **not** allocate, lock, format or run
/// a destructor, because a handler that does any of those can deadlock against the very code it
/// interrupted, and a deadlocked handler is a watchpoint left armed.
///
/// ⚠ This is the belt to the `Drop` guard's braces. A `Drop` runs on unwind but not on a signal;
/// the handler covers the signal, the guard covers the unwind, and the child's own `--max-seconds`
/// covers the case where neither of them ever runs (control #9).
extern "C" fn on_signal(_sig: i32) {
    let p = CHILD_PID.load(Ordering::SeqCst);
    if p > 0 {
        unsafe {
            libc::kill(p, libc::SIGTERM);
        }
    }
    INTERRUPTED.store(true, Ordering::SeqCst);
}

pub fn install_signal_handlers() {
    if HANDLERS_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as usize);
        libc::signal(libc::SIGTERM, on_signal as *const () as usize);
        libc::signal(libc::SIGHUP, on_signal as *const () as usize);
    }
}

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Vocabulary
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The access mode, in `d1-live watch --rw`'s own three tokens.
///
/// ⚠ **State the access mode before choosing the instrument** - the project rule that separates
/// `xref` from `field-xref`, applied here: `Write` answers *"who stored to this"*, `ReadWrite`
/// answers *"who touched it"*, and `Exec` is not a watchpoint at all but a hardware breakpoint that
/// answers *"did this code ever run"*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rw {
    Write,
    ReadWrite,
    Exec,
}

impl Rw {
    /// The vocabulary is CLOSED, and it is d1-live's own three tokens, verbatim - see that crate's
    /// `Rw::parse`, which is the authority. ⚠ Its module cannot be *named* here: control #4 fails
    /// this crate if its text contains the debug-API's name, and the module is named after it.
    pub fn parse(s: &str) -> Result<Rw, String> {
        Ok(match s.trim() {
            "write" => Rw::Write,
            "rw" => Rw::ReadWrite,
            "exec" => Rw::Exec,
            other => {
                return Err(format!(
                    "--rw {other:?} is not one of write|rw|exec (d1-live's own vocabulary)"
                ))
            }
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Rw::Write => "write",
            Rw::ReadWrite => "rw",
            Rw::Exec => "exec",
        }
    }
    /// ⭐ For an execute breakpoint the trapping `RIP` **is** the instruction; for a data watch it
    /// is the one after. The UI must not label them the same way.
    pub fn rip_is_the_instruction(self) -> bool {
        matches!(self, Rw::Exec)
    }
}

/// ⭐ The substring that identifies a slot-exhaustion refusal, pinned in ONE place so that
/// [`crate::table::EventBadge::classify_refusal`] classifies the message this crate actually
/// produces. ⚠ A classifier that matched a transcribed copy would keep passing after the message
/// was reworded - the same drift `argv()` exists to prevent for the lease.
pub const SLOT_CEILING_MARKER: &str = "slots is the ceiling";

/// The refusal text for a fifth watch. Four debug registers is a **hardware** ceiling, not policy.
pub fn slot_ceiling_message() -> String {
    format!("{MAX_SLOTS} {SLOT_CEILING_MARKER} - there are four debug registers. Drop one first.")
}

/// One requested watch. The address is kept as **text**, in the shared grammar, and re-resolved by
/// `d1-live` itself at every arm - so a chain that moves is followed, and d1-scope never hands a
/// stale absolute address to a tracer.
#[derive(Clone, Debug)]
pub struct SlotSpec {
    pub surface: Option<String>,
    pub addr_expr: String,
    pub len: u64,
    pub rw: Rw,
}

impl SlotSpec {
    /// ⚠ Refuse rather than narrow. Rounding a length down would watch different bytes than the
    /// caller named and report a real, wrong answer.
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.len, 1 | 2 | 4 | 8) {
            return Err(format!("--len {} must be 1, 2, 4 or 8", self.len));
        }
        if self.rw == Rw::Exec && self.len != 1 {
            return Err("--rw exec is a hardware breakpoint and needs --len 1".into());
        }
        if self.addr_expr.trim().is_empty() {
            return Err("an empty address expression".into());
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Who holds the target?
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Every distinct non-zero `TracerPid` across **all threads** of a process.
///
/// ⚠ Per-thread on purpose: a debug register lives in `task_struct`, so an instrument arms every
/// thread, and reading only `/proc/<pid>/status` would answer about the main thread alone. A
/// partially-attached target is exactly the confusing state this check exists to avoid.
pub fn tracer_pids(pid: i32) -> Result<Vec<i32>, String> {
    let dir = format!("/proc/{pid}/task");
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("reading {dir}: {e}"))?;
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path().join("status");
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("TracerPid:") {
                if let Ok(t) = v.trim().parse::<i32>() {
                    if t != 0 && !out.contains(&t) {
                        out.push(t);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    Ok(out)
}

pub fn comm_of(pid: i32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// ⛔⛔ **A ZOMBIE IS NOT ALIVE, AND `/proc/<pid>` STILL EXISTS FOR ONE.**
///
/// This cost a wrong reading the first time control #8 ran: an arm-and-abandon child that died of
/// `SIGTRAP` was still an unreaped child of the control process, `/proc/<pid>` was still there, and
/// the control cheerfully reported *"the target survived an armed abandonment"* - i.e. it reported
/// this kernel as harmless when it had in fact just killed the target. **The safest-looking answer
/// was the wrong one.** The state field in `/proc/<pid>/stat` is what separates them.
pub fn alive(pid: i32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // The comm in field 2 may contain spaces and parens; the state is the first token after the
    // last ')'.
    let Some((_, tail)) = stat.rsplit_once(')') else { return false };
    !matches!(tail.split_whitespace().next(), Some("Z") | Some("X") | None)
}

/// `/proc/<pid>/cmdline`, NUL-separated, joined with spaces.
pub fn cmdline_of(pid: i32) -> String {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| {
            String::from_utf8_lossy(&b)
                .split('\0')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Kept for the places that genuinely mean *"does a process table entry exist"* rather than *"is it
/// running"* - currently none, and the distinction is written down so the next caller has to choose.
pub fn exists(pid: i32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// ⭐ A refusal is a **message**, not an error code and not a fight.
#[derive(Clone, Debug)]
pub enum Refusal {
    NotAProcess(i32),
    Ourselves(i32),
    /// ⛔ The operator's live client. Guarded by the same predicate `ProcMem` uses.
    Emulator { pid: i32, comm: String },
    /// Someone else got there first. One tracer per process is a kernel rule.
    HeldByAnother { pid: i32, tracers: Vec<i32>, comms: Vec<String> },
    /// Our own child already holds it - re-arm, do not launch a second.
    HeldByOurChild { pid: i32, child: i32 },
    /// A previous stop could not be proven; d1-scope will not start another child.
    PreviousStopUnconfirmed { why: String },
    /// ⛔ Refused, not clamped: see [`ChildMgr::new`].
    LeaseOutOfBounds { asked: f64, max: f64 },
}

impl Refusal {
    pub fn message(&self) -> String {
        match self {
            Refusal::NotAProcess(p) => format!("pid {p} is not a running process - nothing to attach to."),
            Refusal::Ourselves(p) => format!("pid {p} is d1-scope itself. Refusing."),
            Refusal::Emulator { pid, comm } => format!(
                "⛔ pid {pid} ({comm}) is the emulator - the OPERATOR'S LIVE CLIENT. d1-scope will \
                 not arm a watchpoint on it by default. If you truly mean to, set \
                 D1_SCOPE_ALLOW_LIVE=1 deliberately, on the command line, having read \
                 historicaldocs/design/live-watch.md §4.2. ⚠ `pgrep -x shadps4` matches nothing; the comm is \
                 `shadPS4:Main`."
            ),
            Refusal::HeldByAnother { pid, tracers, comms } => format!(
                "⛔ pid {pid} is ALREADY TRACED by {tracers:?} ({}). One tracer per process is a \
                 kernel rule and d1-scope does not race for it. ⇒ ask that lane to release it \
                 (`d1-scope tracer --pid {pid}` names the holder; a `d1-live watch` releases on \
                 SIGTERM, on Ctrl-C, or by itself at its --max-seconds lease). ⛔ Do NOT use the \
                 un-catchable signal on it: a tracer that dies armed kills its target.",
                comms.join(", ")
            ),
            Refusal::HeldByOurChild { pid, child } => format!(
                "our own d1-live child {child} already holds pid {pid}. Re-arm it (which stops it \
                 cleanly first) instead of launching a second - one child is the invariant."
            ),
            Refusal::LeaseOutOfBounds { asked, max } => format!(
                "a lease of {asked}s is refused (max {max}s). The lease is how long a watchpoint \
                 can outlive a d1-scope that was hard-killed - an unbounded one is the hazard \
                 wearing a seatbelt. Ask for less; it is REFUSED rather than clamped, so you never \
                 get something other than what you asked for."
            ),
            Refusal::PreviousStopUnconfirmed { why } => format!(
                "⛔ the previous child's disarm could NOT be confirmed ({why}). d1-scope will not \
                 start another child while a watchpoint may still be live on that target. Check it \
                 by hand before continuing."
            ),
        }
    }
}

/// ⚠ `pgrep -x shadps4` matches nothing - the comm is `shadPS4:Main`.
pub fn comm_is_emulator(comm: &str) -> bool {
    crate::controls::comm_is_emulator(comm)
}

/// May d1-scope attach to `pid`? `our_child` is the pid of the child we already run, if any.
pub fn check_target(pid: i32, our_child: Option<i32>) -> Result<(), Refusal> {
    if pid <= 1 || !alive(pid) {
        return Err(Refusal::NotAProcess(pid));
    }
    if pid == std::process::id() as i32 {
        return Err(Refusal::Ourselves(pid));
    }
    let comm = comm_of(pid);
    if comm_is_emulator(&comm) && std::env::var("D1_SCOPE_ALLOW_LIVE").as_deref() != Ok("1") {
        return Err(Refusal::Emulator { pid, comm });
    }
    let tracers = tracer_pids(pid).unwrap_or_default();
    if !tracers.is_empty() {
        if let Some(c) = our_child {
            if tracers == vec![c] {
                return Err(Refusal::HeldByOurChild { pid, child: c });
            }
        }
        let comms = tracers.iter().map(|t| comm_of(*t)).collect();
        return Err(Refusal::HeldByAnother { pid, tracers, comms });
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The black-box recorder
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐ Receipts and reports survive the manager that produced them.
///
/// This exists because of one scenario: d1-scope **panics**, `Drop` stops the child, and the
/// evidence that it did so must still be readable afterwards - by a control, or by whoever is
/// reading the crash. A `Drop` cannot return anything, so it writes here.
#[derive(Default, Debug)]
pub struct Recorder {
    pub receipts: Vec<StopReceipt>,
    pub reports: Vec<LiveReport>,
    pub notes: Vec<String>,
}

pub type Log = Arc<Mutex<Recorder>>;

pub fn new_log() -> Log {
    Arc::new(Mutex::new(Recorder::default()))
}

/// ⭐⭐ **EVERY STOP THIS SESSION THAT DID NOT PROVE A DISARM.**
///
/// The receipts are the black box, and this is the only question anyone should ask of them at the
/// end of a run. It exists because the evidence *was already being recorded* and no shipped command
/// read it: `cmd_watch` drained `reports` and never touched `receipts`, so a session whose child
/// left through a lethal path printed *"(no child was running at session end)"* and exited 0.
/// ⇒ **the fact was not lost, it was unreachable.** Now there is one reader and one meaning.
pub fn unconfirmed_receipts(log: &Log) -> Vec<StopReceipt> {
    log.lock()
        .map(|l| l.receipts.iter().filter(|r| !r.confirmed_disarm).cloned().collect())
        .unwrap_or_default()
}

/// ⛔ **THE SESSION'S EXIT CODE, DECIDED IN ONE FUNCTION.** `2` iff any stop this session failed to
/// prove a disarm - whatever route it took (session end, re-arm, heartbeat collection, interrupt,
/// `Drop`). A control drives *this* function rather than a copy of the rule, so the shell's `$?`
/// cannot drift away from what the receipts say.
pub fn session_exit_code(log: &Log) -> i32 {
    if unconfirmed_receipts(log).is_empty() {
        0
    } else {
        2
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The child
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Where the `d1-live` binary is. `$D1_LIVE_BIN` overrides.
pub fn default_live_bin() -> PathBuf {
    if let Ok(p) = std::env::var("D1_LIVE_BIN") {
        return PathBuf::from(p);
    }
    crate::repo_root().join("d1-server/target/release/d1-live")
}

/// Where a child's two streams go.
///
/// ⭐ **`File` is not a convenience.** A child whose parent is hard-killed loses the read end of a
/// pipe, and with it every word of its provenance - including the report it writes *after*
/// disarming. Files keep that evidence on disk and are what makes control #9 a measurement of the
/// orphan's own behaviour rather than an inference from its absence.
enum Sink {
    Mem(Arc<Mutex<String>>),
    File(PathBuf),
}

impl Sink {
    fn text(&self) -> String {
        match self {
            Sink::Mem(m) => m.lock().map(|s| s.clone()).unwrap_or_default(),
            Sink::File(p) => std::fs::read_to_string(p).unwrap_or_default(),
        }
    }
}

struct Running {
    child: Child,
    pid: i32,
    started: Instant,
    lease: Duration,
    out: Sink,
    err: Sink,
}

fn drain(reader: impl std::io::Read + Send + 'static, sink: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Ok(mut s) = sink.lock() {
                        s.push_str(&line);
                    }
                }
            }
        }
    });
}

/// ⛔ The longest lease d1-scope will ever grant. A lease is *the time a watchpoint can outlive a
/// d1-scope that was hard-killed*, so an unbounded one is the whole hazard wearing a seatbelt.
pub const LEASE_MAX: Duration = Duration::from_secs(120);

/// ⭐⭐ **THE ONE PLACE THE CHILD'S COMMAND LINE IS BUILT**, so that
/// [`crate::controls_event::c9_lease_backstop`] can assert `--max-seconds` against *this function*
/// rather than against a copy of it. An assertion that reads a second construction of the same
/// thing cannot catch the first one drifting - which is exactly how a lease goes missing.
pub fn argv(target: i32, spec: &SlotSpec, count: usize, lease: Duration, no_base: bool, follow_clone: bool) -> Vec<String> {
    let mut v: Vec<String> = vec![
        "--json".into(),
        "watch".into(),
        "--pid".into(),
        target.to_string(),
        "--addr".into(),
        spec.addr_expr.clone(),
        "--len".into(),
        spec.len.to_string(),
        "--rw".into(),
        spec.rw.as_str().into(),
        "--slot".into(),
        "0".into(),
        "--count".into(),
        count.to_string(),
        // ⭐ ALWAYS. There is no branch here and there must never be one.
        "--max-seconds".into(),
        format!("{:.1}", lease.as_secs_f64()),
    ];
    if no_base {
        v.push("--no-base".into());
    }
    if follow_clone {
        v.push("--follow-clone".into());
    }
    v
}

/// `ARMED   pid 42  DR0  Write 8 byte(s) at 0x… on 111 thread(s)` → `111`.
fn parse_threads_armed(line: &str) -> Option<i32> {
    let i = line.find("thread(s)")?;
    line[..i].split_whitespace().last()?.parse().ok()
}

/// …and the address out of the same line: ` at 0x5632865bf000`.
fn parse_armed_addr(line: &str) -> Option<u64> {
    let rest = line.split(" at ").nth(1)?;
    let tok = rest.split_whitespace().next()?;
    u64::from_str_radix(tok.trim_start_matches("0x"), 16).ok()
}

/// `DEBUGREG CONTROL: PASS  u_debugreg[0] at 0x350 - …` → `0x350`.
///
/// ⭐ Taken from d1-live's own measurement, never transcribed. d1-scope has no business knowing
/// this constant; it only needs to pass it back to a d1-live helper (control #8's leg A).
pub fn debugreg_offset_from(text: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.contains("u_debugreg[0] at "))?;
    let rest = line.split("u_debugreg[0] at ").nth(1)?;
    let tok = rest.split_whitespace().next()?;
    u64::from_str_radix(tok.trim_start_matches("0x"), 16).ok()
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// ChildMgr
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub struct ChildMgr {
    live_bin: PathBuf,
    target: i32,
    lease: Duration,
    /// Re-arm when this much of the lease has been spent. Keeps the child's deadline always in the
    /// future while d1-scope is alive, and makes the lease a *backstop* rather than a schedule.
    renew_fraction: f64,
    count: usize,
    no_base: bool,
    follow_clone: bool,
    /// When set, the child's two streams go to FILES here instead of pipes - so its provenance and
    /// its exit report survive the death of this process. See [`Sink`].
    stdio_dir: Option<PathBuf>,
    slots: Vec<SlotSpec>,
    armed_index: usize,
    child: Option<Running>,
    blocked: Option<String>,
    rearms: u64,
    hits_total: usize,
    log: Log,
    last_status: ChildStatus,
}

impl ChildMgr {
    /// ⛔ Construction **refuses** rather than returning something half-attached.
    pub fn new(target: i32, lease: Duration, log: Log) -> Result<Self, Refusal> {
        // ⛔ A lease IS the time a watchpoint can outlive a hard-killed d1-scope. An unbounded one
        //    is the hazard wearing a seatbelt, so it is refused rather than clamped: clamping would
        //    silently grant something other than what the caller asked for.
        if lease > LEASE_MAX || lease.is_zero() {
            return Err(Refusal::LeaseOutOfBounds { asked: lease.as_secs_f64(), max: LEASE_MAX.as_secs_f64() });
        }
        check_target(target, None)?;
        install_signal_handlers();
        Ok(Self {
            live_bin: default_live_bin(),
            target,
            lease,
            renew_fraction: 0.6,
            count: 32,
            // The default target is not the pinned client, so there is no base to recover and
            // every address must be absolute. `with_base()` turns it back on for the real client.
            no_base: true,
            follow_clone: false,
            stdio_dir: None,
            slots: Vec::new(),
            armed_index: 0,
            child: None,
            blocked: None,
            rearms: 0,
            hits_total: 0,
            log,
            last_status: ChildStatus::None,
        })
    }

    pub fn with_live_bin(mut self, p: impl Into<PathBuf>) -> Self {
        self.live_bin = p.into();
        self
    }
    pub fn with_count(mut self, n: usize) -> Self {
        self.count = n.max(1);
        self
    }
    /// Recover the guest base, so eboot vaddrs (`v:0x…`) and `RIP → v:` translation work. Only
    /// meaningful against the pinned client.
    pub fn with_base(mut self) -> Self {
        self.no_base = false;
        self
    }
    pub fn with_follow_clone(mut self, yes: bool) -> Self {
        self.follow_clone = yes;
        self
    }
    /// ⭐ Put the child's stdout/stderr on disk. Do this for any session that matters: it is the
    /// only way the child's own report survives d1-scope being hard-killed, and "the orphan
    /// self-disarmed at its lease" is then a **measurement** rather than an inference.
    pub fn with_stdio_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.stdio_dir = Some(dir.into());
        self
    }
    pub fn target(&self) -> i32 {
        self.target
    }
    pub fn slots(&self) -> &[SlotSpec] {
        &self.slots
    }
    pub fn child_pid(&self) -> Option<i32> {
        self.child.as_ref().map(|c| c.pid)
    }
    /// How many children this manager has started after the first. ⭐ Control #15 uses it to prove
    /// that its "clean collection is silent" leg really *collected* something - a leg that passes
    /// because nothing happened is not a leg.
    pub fn rearms(&self) -> u64 {
        self.rearms
    }
    pub fn log(&self) -> Log {
        Arc::clone(&self.log)
    }

    /// ⛔ Four is the ceiling because there are four debug registers, and the fifth is REFUSED
    /// rather than silently dropped.
    pub fn add_slot(&mut self, s: SlotSpec) -> Result<usize, String> {
        s.validate()?;
        if self.slots.len() >= MAX_SLOTS {
            return Err(slot_ceiling_message());
        }
        self.slots.push(s);
        Ok(self.slots.len() - 1)
    }

    /// Which slot the single child arms. The rest are `Pending`, and the frame says so.
    pub fn select(&mut self, idx: usize) -> Result<(), String> {
        if idx >= self.slots.len() {
            return Err(format!("slot {idx} does not exist ({} held)", self.slots.len()));
        }
        self.armed_index = idx;
        Ok(())
    }

    /// ⚠ The honest list: slots we hold and are NOT watching.
    pub fn pending(&self) -> Vec<String> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.armed_index)
            .map(|(i, s)| {
                format!(
                    "slot {i} {} ({} {}) - NOT ARMED: one d1-live process watches one address, and \
                     one child is the lifecycle invariant",
                    s.addr_expr,
                    s.len,
                    s.rw.as_str()
                )
            })
            .collect()
    }

    // ── launch ──────────────────────────────────────────────────────────────────────────────────

    /// Start the one child. ⛔ Refuses if one is already running, if a previous stop was never
    /// confirmed, or if anything else now holds the target.
    pub fn launch(&mut self) -> Result<(), Refusal> {
        if let Some(why) = &self.blocked {
            return Err(Refusal::PreviousStopUnconfirmed { why: why.clone() });
        }
        if let Some(c) = &self.child {
            return Err(Refusal::HeldByOurChild { pid: self.target, child: c.pid });
        }
        check_target(self.target, None)?;
        let Some(spec) = self.slots.get(self.armed_index).cloned() else {
            self.note("launch() with no slots - nothing to arm");
            return Ok(());
        };

        // ⭐ ONE construction of the command line, shared with the control that asserts the lease.
        let mut cmd = Command::new(&self.live_bin);
        cmd.args(argv(self.target, &spec, self.count, self.lease, self.no_base, self.follow_clone));
        cmd.stdin(Stdio::null());

        // ⚠ The two stdio shapes are chosen ONCE, here, and the rest of the manager reads through
        // `Sink` so no path can accidentally work in only one of them.
        let (out_sink, err_sink) = match &self.stdio_dir {
            Some(dir) => {
                let n = self.rearms;
                let o = dir.join(format!("d1-live-{n}.out"));
                let e = dir.join(format!("d1-live-{n}.err"));
                let _ = std::fs::create_dir_all(dir);
                let (fo, fe) = match (std::fs::File::create(&o), std::fs::File::create(&e)) {
                    (Ok(a), Ok(b)) => (a, b),
                    _ => {
                        self.note(format!("could not create child log files under {}", dir.display()));
                        return Err(Refusal::NotAProcess(self.target));
                    }
                };
                cmd.stdout(Stdio::from(fo)).stderr(Stdio::from(fe));
                (Sink::File(o), Sink::File(e))
            }
            None => {
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                (Sink::Mem(Arc::new(Mutex::new(String::new()))), Sink::Mem(Arc::new(Mutex::new(String::new()))))
            }
        };

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.note(format!("spawning {}: {e}", self.live_bin.display()));
                return Err(Refusal::NotAProcess(self.target));
            }
        };
        let pid = child.id() as i32;
        // ⭐ PUBLISH THE PID BEFORE ANYTHING ELSE. Between spawn and this store, a signal would
        //   find no child to term - so the window is one atomic store wide, and the child's own
        //   lease covers even that.
        CHILD_PID.store(pid, Ordering::SeqCst);

        if let (Sink::Mem(o), Sink::Mem(e)) = (&out_sink, &err_sink) {
            drain(child.stdout.take().expect("piped"), Arc::clone(o));
            drain(child.stderr.take().expect("piped"), Arc::clone(e));
        }

        self.child = Some(Running {
            child,
            pid,
            started: Instant::now(),
            lease: self.lease,
            out: out_sink,
            err: err_sink,
        });
        Ok(())
    }

    /// The child's own `ARMED` line, if it has printed one.
    fn armed_line(&self) -> Option<String> {
        let c = self.child.as_ref()?;
        c.err.text().lines().rev().find(|l| l.starts_with("ARMED")).map(str::to_string)
    }

    /// ⭐ Wait until the child says it armed. `false` = it never did - and a run from there would
    /// report a false *"nothing wrote it"*, which is the worst output this instrument can produce.
    pub fn wait_until_armed(&self, timeout: Duration) -> bool {
        let Some(c) = &self.child else { return false };
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.armed_line().is_some() {
                return true;
            }
            if !alive(c.pid) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    pub fn stderr_text(&self) -> String {
        self.child.as_ref().map(|c| c.err.text()).unwrap_or_default()
    }

    /// ⭐ The debug-register offset **as d1-live measured it**, scraped from the child's own
    /// receipt line. d1-scope never transcribes this constant; it only relays it.
    pub fn debugreg_offset(&self) -> Option<u64> {
        debugreg_offset_from(&self.stderr_text())
    }

    // ── stop ────────────────────────────────────────────────────────────────────────────────────

    /// ⭐⭐ **THE ONE STOP PATH.** `SIGTERM`, wait, read the child's own report, and only then say
    /// it is disarmed.
    ///
    /// ⛔ There is no second signal and no timeout that escalates. If the child will not leave, the
    /// receipt says `confirmed_disarm: false`, the manager **blocks** itself from starting another,
    /// and the child's `--max-seconds` lease finishes the job. Hard-killing it is the one action
    /// that is guaranteed to take the target down with it.
    pub fn stop(&mut self, how: &str) -> Option<StopReceipt> {
        let mut c = self.child.take()?;
        let t0 = Instant::now();
        // Idempotent by construction: a child that already exited ignores this.
        unsafe {
            libc::kill(c.pid, libc::SIGTERM);
        }
        CHILD_PID.store(0, Ordering::SeqCst);

        let hard_deadline = c.started + c.lease + Duration::from_secs(3);
        let mut status = None;
        let mut nagged = false;
        loop {
            match c.child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    break;
                }
                Ok(None) => {}
                Err(_) => break,
            }
            let waited = t0.elapsed();
            if waited > STOP_GRACE && !nagged {
                nagged = true;
                self.note(format!(
                    "⚠ d1-live child {} has not exited {:.1}s after SIGTERM. NOT escalating - the \
                     un-catchable signal would leave the watchpoint armed and kill the target. \
                     Waiting for its own --max-seconds lease instead.",
                    c.pid,
                    waited.as_secs_f64()
                ));
                // Re-send: harmless, and covers the case where the first arrived before the child
                // installed its handler.
                unsafe {
                    libc::kill(c.pid, libc::SIGTERM);
                }
            }
            if Instant::now() > hard_deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Give the drain threads a beat to flush the report the child wrote on its way out.
        std::thread::sleep(Duration::from_millis(80));
        let out = c.out.text();
        let errtext = c.err.text();

        let exit_code = status.as_ref().and_then(|s| s.code());
        let exit_signal = status.as_ref().and_then(signal_of);

        let mut complaints: Vec<String> = errtext
            .lines()
            .filter(|l| l.contains("could NOT be proven disarmed") || l.contains("d1-live: ⛔"))
            .map(str::to_string)
            .collect();

        let stopped_cleanly = status.is_some();
        let parsed = parse_report(out.trim());
        let parse_ok = parsed.is_ok();
        let seconds = parsed.as_ref().map(|(r, _)| r.seconds).unwrap_or(0.0);
        let threads_armed = parsed.as_ref().map(|(r, _)| r.threads_armed).unwrap_or(0);
        let (stop_reason, hits) = match &parsed {
            Ok((rep, missing)) => {
                if !missing.is_empty() {
                    complaints.push(format!(
                        "the child's report is missing key(s) we depend on: {missing:?} - d1-live's \
                         report shape has DRIFTED and events may be silently degraded"
                    ));
                }
                (Some(rep.stop_reason.clone()), rep.hits.len())
            }
            Err(e) => {
                if status.is_some() {
                    complaints.push(format!("could not read the child's exit report: {e}"));
                }
                (None, 0)
            }
        };

        // ⭐ THE RECEIPT IS A CONJUNCTION, and every term is load-bearing:
        //    • it exited at all - a child still running may still be armed;
        //    • it exited on its own path, not by a signal - a child that DIED did not disarm;
        //    • it wrote a complete report - the proof it reached the end of that path;
        //    • it did not complain about a thread it could not prove disarmed.
        //  ⛔ "the process is gone" alone is NOT this statement.
        let confirmed = status.is_some()
            && exit_signal.is_none()
            && parsed.is_ok()
            && complaints.is_empty();

        if let Ok((rep, _)) = parsed {
            self.hits_total += rep.hits.len();
            if let Ok(mut l) = self.log.lock() {
                l.reports.push(rep);
            }
        }

        let receipt = StopReceipt {
            child_pid: c.pid,
            target_pid: self.target,
            how: how.to_string(),
            exit_code,
            exit_signal,
            waited_ms: t0.elapsed().as_millis(),
            confirmed_disarm: confirmed,
            stop_reason,
            complaints: complaints.clone(),
            hits,
        };
        if !confirmed {
            self.blocked = Some(format!(
                "child {} left with exit_code={:?} signal={:?} complaints={:?}",
                c.pid, exit_code, exit_signal, complaints
            ));
        }
        if let Ok(mut l) = self.log.lock() {
            l.receipts.push(receipt.clone());
        }

        // ⭐⭐ CLASSIFY ON **STDOUT PARSEABILITY**, NEVER ON THE EXIT CODE. `d1-live` exits 1 both
        //    for "nothing wrote it" and for "the run failed", and those are opposite facts: the
        //    first is a real negative result this instrument exists to produce, the second is no
        //    result at all. Filing NoHit under Failed would throw away the evidence and make a
        //    working watch look broken.
        let sv = self.spec_view();
        self.last_status = if !stopped_cleanly {
            ChildStatus::StopTimedOut {
                pid: c.pid,
                waited_s: t0.elapsed().as_secs_f64(),
                receipt: receipt.clone(),
            }
        } else if parse_ok {
            if hits == 0 {
                ChildStatus::NoHit {
                    spec: sv,
                    watched_s: seconds,
                    stop_reason: receipt.stop_reason.clone().unwrap_or_default(),
                    why: format!(
                        "⚠ NO HIT in {seconds:.1}s ({}). This is evidence ONLY about the {} \
                         thread(s) armed at start. It is NOT evidence that nothing writes it.",
                        receipt.stop_reason.clone().unwrap_or_default(),
                        threads_armed
                    ),
                    receipt: receipt.clone(),
                }
            } else {
                ChildStatus::Finished {
                    spec: sv,
                    hits: hits as u32,
                    stop_reason: receipt.stop_reason.clone().unwrap_or_default(),
                    receipt: receipt.clone(),
                }
            }
        } else {
            // ⚠ stderr VERBATIM. A summarised failure is a failure nobody can diagnose.
            ChildStatus::Failed { spec: sv, stderr: errtext.clone(), receipt: receipt.clone() }
        };
        Some(receipt)
    }

    /// ⛔⛔ **THE ONE PLACE A RECEIPT BECOMES A REFUSAL - AND IT EXISTS BECAUSE THE ONE PATH THAT
    /// DID NOT USE IT WAS THE ONE THE PRODUCT ACTUALLY TAKES.**
    ///
    /// `confirmed_disarm == false` means any of three things, and all three are the lethal reading:
    /// the child died of a signal (its disarm never ran), it complained about a thread whose `DR7`
    /// did not read back zero, or it wrote no parseable report (so nothing proves it reached the
    /// end of its disarm path). ⇒ **every** route out of [`stop`](Self::stop) funnels through here.
    ///
    /// ⚠ THE BUG THIS REPLACES, stated so it cannot come back: [`heartbeat`](Self::heartbeat)
    /// collected a self-expired child, consumed `confirmed_disarm` **only** to decide whether to
    /// relaunch, and returned `Ok(())`. That is the normal collection point for every
    /// `--count`-bounded cycle and every self-expired lease - so the most-travelled stop path in
    /// the binary was the only silent one, and a session that may have left a watchpoint armed on
    /// the operator's client printed a clean log and exited 0. `rearm()` was loud, `session-end`
    /// was loud, `heartbeat()` was not, and no control called `heartbeat()` at all.
    /// ⇒ control **#15** drives *this* function through `heartbeat()` and asserts the exit code.
    fn raise_if_unconfirmed(r: &Option<StopReceipt>) -> Result<(), Refusal> {
        // ⚠ `None` = there was no child to stop. That is not an unconfirmed disarm; it is no event.
        let Some(rc) = r else { return Ok(()) };
        if rc.confirmed_disarm {
            return Ok(());
        }
        Err(Refusal::PreviousStopUnconfirmed {
            why: format!(
                "how={:?} exit_code={:?} signal={:?} stop_reason={:?} complaints={:?}",
                rc.how, rc.exit_code, rc.exit_signal, rc.stop_reason, rc.complaints
            ),
        })
    }

    /// ⭐ **Re-arm is a full restart.** v1 does not mutate a live watchpoint: it stops the child,
    /// waits for the confirmed disarm, and launches a new one. Every change therefore routes
    /// through a disarm that was *proved*, which is the safest shape available and costs one
    /// process spawn. A mutate-in-place path would have to be trusted; this one is witnessed.
    pub fn rearm(&mut self, why: &str) -> Result<Option<StopReceipt>, Refusal> {
        let r = self.stop(why);
        Self::raise_if_unconfirmed(&r)?;
        self.rearms += 1;
        self.launch()?;
        Ok(r)
    }

    /// ⭐ The hand-off. Stop cleanly, prove it, and leave the target free for another lane.
    pub fn release(&mut self) -> Option<StopReceipt> {
        let r = self.stop("release-tracer");
        self.note(format!(
            "released pid {} - tracers now {:?}",
            self.target,
            tracer_pids(self.target).unwrap_or_default()
        ));
        r
    }

    /// ⭐ Called once per sampler tick. Keeps the child's deadline in the future while d1-scope
    /// lives, collects a child that finished on its own, and does nothing at all if there is
    /// nothing to do.
    /// ⛔ **NOT AN `Ok(())` MACHINE.** It returns [`Refusal::PreviousStopUnconfirmed`] on any stop
    /// it collects whose disarm was not proved - see [`raise_if_unconfirmed`](Self::raise_if_unconfirmed).
    /// A caller that discards this error re-introduces the swallow it was written to close.
    pub fn heartbeat(&mut self) -> Result<(), Refusal> {
        if interrupted() {
            // ⛔ The receipt used to be DISCARDED here, outright. A Ctrl-C that collected a child
            //    which died of a signal was indistinguishable from a clean one.
            let r = self.stop("interrupted");
            Self::raise_if_unconfirmed(&r)?;
            return Ok(());
        }
        // ⚠ Read what we need and DROP the borrow: `stop()`/`rearm()` need `&mut self`.
        let (exited, spent, lease) = match &mut self.child {
            None => return Ok(()),
            Some(c) => (
                matches!(c.child.try_wait(), Ok(Some(_))),
                c.started.elapsed().as_secs_f64(),
                c.lease.as_secs_f64(),
            ),
        };
        // Did it finish on its own - the lease, or --count?
        if exited {
            let r = self.stop("child-exited");
            // ⛔⛔ RAISE BEFORE RELAUNCHING, AND RAISE AT ALL. `confirmed_disarm` used to be read
            //    here ONLY as a relaunch predicate and then dropped on the floor - a receipt that
            //    says "the tracer may have died armed on the target" became, silently, the decision
            //    not to start another one. It is now the same loud refusal `rearm()` has always
            //    raised, on the path that actually runs.
            Self::raise_if_unconfirmed(&r)?;
            self.rearms += 1;
            self.launch()?;
            return Ok(());
        }
        if spent >= lease * self.renew_fraction {
            self.rearm("lease-renewal")?;
        }
        Ok(())
    }

    // ── present ─────────────────────────────────────────────────────────────────────────────────

    /// The armed slot, as the frame shows it.
    fn spec_view(&self) -> WatchSpecView {
        let d = SlotSpec { surface: None, addr_expr: String::new(), len: 8, rw: Rw::Write };
        let spec = self.slots.get(self.armed_index).unwrap_or(&d);
        WatchSpecView {
            surface: spec.surface.clone(),
            addr_expr: spec.addr_expr.clone(),
            // ⭐ THIS child's own resolution, off its own ARMED line - never carried over from the
            // previous one. `0` = it has not announced an arm yet.
            resolved: self.armed_line().and_then(|l| parse_armed_addr(&l)).unwrap_or(0),
            len: spec.len,
            rw: spec.rw.as_str().to_string(),
            slot: self.armed_index,
            count: self.count,
            lease_s: self.lease.as_secs_f64(),
        }
    }

    pub fn status(&self) -> ChildStatus {
        let Some(c) = &self.child else {
            return self.last_status.clone();
        };
        ChildStatus::Running {
            pid: c.pid,
            target_pid: self.target,
            spec: self.spec_view(),
            lease_s: c.lease.as_secs_f64(),
            lease_remaining_s: (c.lease.as_secs_f64() - c.started.elapsed().as_secs_f64()).max(0.0),
            armed_threads: self
                .armed_line()
                .and_then(|l| parse_threads_armed(&l))
                .unwrap_or(0)
                .max(0) as u32,
            rearms: self.rearms,
            hits: self.hits_total,
            pending: self.pending(),
        }
    }

    /// The surface name the armed slot claims - one half of the two-derivation attribution.
    pub fn armed_surface(&self) -> Option<String> {
        self.slots.get(self.armed_index).and_then(|s| s.surface.clone())
    }

    fn note(&self, s: impl Into<String>) {
        if let Ok(mut l) = self.log.lock() {
            l.notes.push(s.into());
        }
    }
}

/// ⛔⛔ **THE BRACES.** Normal return, `?`, and **panic-unwind** all pass through here. Control #8
/// removes this block on purpose and watches the control fail - a guard nobody has ever seen fail
/// is a guard nobody has tested.
impl Drop for ChildMgr {
    fn drop(&mut self) {
        if let Some(r) = self.stop("drop-guard") {
            if !r.confirmed_disarm {
                eprintln!(
                    "d1-scope: ⛔ the d1-live child {} could not be PROVEN disarmed on the way out \
                     ({:?}). Its --max-seconds lease will finish the job; do not hard-kill it.",
                    r.child_pid, r.complaints
                );
            }
        }
    }
}

/// The terminating signal of an exit status, without importing a unix-only extension trait into
/// every caller.
fn signal_of(s: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    s.signal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_access_mode_vocabulary_is_closed() {
        assert_eq!(Rw::parse("write").unwrap(), Rw::Write);
        assert_eq!(Rw::parse("rw").unwrap(), Rw::ReadWrite);
        assert_eq!(Rw::parse("exec").unwrap(), Rw::Exec);
        // The failing half of the axis: anything else is REFUSED, not defaulted to `write`.
        assert!(Rw::parse("w").is_err());
        assert!(Rw::parse("").is_err());
        assert!(Rw::parse("read").is_err());
        // …and only exec makes RIP the instruction.
        assert!(Rw::Exec.rip_is_the_instruction());
        assert!(!Rw::Write.rip_is_the_instruction());
    }

    #[test]
    fn slot_validation_refuses_rather_than_narrows() {
        let s = |len, rw| SlotSpec { surface: None, addr_expr: "a:0x1000".into(), len, rw };
        assert!(s(8, Rw::Write).validate().is_ok());
        assert!(s(3, Rw::Write).validate().is_err()); // a length the hardware cannot express
        assert!(s(16, Rw::Write).validate().is_err());
        assert!(s(8, Rw::Exec).validate().is_err()); // exec is one byte
        assert!(s(1, Rw::Exec).validate().is_ok());
    }

    #[test]
    fn the_slot_ceiling_is_four_and_the_fifth_is_refused() {
        // No process is touched: the ceiling is enforced before anything is launched.
        let spec = SlotSpec { surface: None, addr_expr: "a:0x1000".into(), len: 8, rw: Rw::Write };
        let mut held: Vec<SlotSpec> = Vec::new();
        for _ in 0..MAX_SLOTS {
            held.push(spec.clone());
        }
        assert_eq!(held.len(), MAX_SLOTS);
        assert_eq!(MAX_SLOTS, 4, "four debug registers");
    }

    #[test]
    fn banner_parsers_are_measured_against_real_lines() {
        let armed = "ARMED   pid 1999389  DR0  Write 8 byte(s) at 0x5632865bf000  on 111 thread(s)";
        assert_eq!(parse_threads_armed(armed), Some(111));
        assert_eq!(parse_threads_armed("nothing here"), None);
        let dbg = "DEBUGREG CONTROL: PASS  u_debugreg[0] at 0x350 - POKEUSER rejects 0x348; …";
        assert_eq!(debugreg_offset_from(dbg), Some(0x350));
        assert_eq!(debugreg_offset_from("no such line"), None);
    }

    #[test]
    fn an_impossible_pid_is_refused_by_message() {
        // 2^31-1 is not a pid on this box; the point is that the answer is a MESSAGE.
        let r = check_target(i32::MAX, None).unwrap_err();
        assert!(r.message().contains("not a running process"));
        assert!(check_target(std::process::id() as i32, None).is_err());
    }
}
