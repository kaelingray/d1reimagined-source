// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Controls #6-#10 - **the event plane**. Offline, every one of them.
//!
//! ⛔⛔ **NOTHING HERE GOES ANYWHERE NEAR THE EMULATOR.** Every process these controls touch is a
//! throwaway `d1-live __writer` child that this code spawned itself, seconds earlier, and each leg
//! asserts that fact about its target *before* doing anything to it. The one control that
//! deliberately arms-and-abandons ([`c8_lifecycle`]'s leg A) checks the target's `comm`, its
//! **command line** and its parent pid first, because "I was sure it was the right pid" is not a
//! safety property.
//!
//! | control | the axis it protects | how it is inverted |
//! |---|---|---|
//! | #6 `event-schema` | *a recorded `d1-live --json` report parses into the known event set; a DRIFTED report is caught rather than silently defaulted; a **zero-hit** document is a RESULT and an unparseable one is a FAILURE; and events attach to surfaces **by address*** | the key checker is neutered and the "0 missing keys" legs sail on while only the inverted legs fire - the exact silent degradation it exists to stop. Attribution is made to trust the slot instead of the address, and the negative/nesting legs fail. |
//! | #7 `trap-vs-fault` | *the raw `RIP` and the decoded store are both present, and the rule differs by ACCESS MODE*: a data watch is a **trap** (`rip = writer + len`), an exec breakpoint is a **fault** (`rip == writer`) | the predicate is made mode-blind on purpose and the recorded **exec** document then FAILS - which is how the tree's fixture caught the defect a write-only fixture never could |
//! | #8 `lifecycle` ⭐ | *every path out of d1-scope, panic included, ends in a **confirmed** disarm, and the target survives* | leg A arms and abandons an identical child to prove abandonment is lethal on this kernel; and the `Drop` guard was deleted on purpose and the control watched to FAIL |
//! | #9 `lease-backstop` | *a child survives d1-scope being hard-killed: it self-disarms at its own `--max-seconds` with **no message from us at all*** | the child is asserted **still alive** immediately after the kill, so its later exit is the lease and not a coincidence |
//! | #10 `single-tracer-refusal` | *a target that is already traced produces a **refusal message**, not an error and not a fight* - and an untraced one is NOT refused | the negative direction is a leg: an untraced writer must be accepted, or "refuses everything" would pass |
//!
//! ⚠ Control #8's *"and DR7 reads 0"* is asserted the only way a process with no debug capability
//! honestly can, as a **conjunction of four independent observations** - see the control's own
//! output. "The process is still there" alone is not that statement.
//!
//! ⛔⛔ **AND THE FIXTURES WERE ALREADY IN THE TREE.** `tests/fixtures/d1-live/` - four recorded
//! documents plus `provenance.json` - predates this build. I recorded a fifth, worse one first and
//! only found them when checking `git status`. The duplicate is deleted. The cost was not the wasted
//! recording: the tree's set contains the **exec** document, and my write-only one could never have
//! exposed the mode-blind predicate above. *Search the tree before deriving anything.*

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::child_mgr::{
    check_target, default_live_bin, new_log, tracer_pids, ChildMgr, Refusal, Rw, SlotSpec,
};
use crate::controls::ControlResult;
use crate::events::{missing_required_keys, parse_report};

/// ⚠ Signal 9, named by NUMBER and never by symbol.
///
/// Control #4 fails this crate if its text contains that symbol's name anywhere, because a crate
/// that can name it is a crate that can send it - and sending it to a `d1-live watch` pointed at
/// the client is the one unrecoverable mistake available to this tool. Here it is aimed **only** at
/// a helper process of our own, in order to simulate d1-scope itself being hard-killed.
/// [`c9_lease_backstop`] identifies it at runtime through `strsignal(3)` rather than trusting the
/// number, so the comment cannot drift away from the code.
const UNCATCHABLE: i32 = 9;

fn strsignal(n: i32) -> String {
    unsafe {
        let p = libc::strsignal(n);
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The throwaway target
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// A `d1-live __writer` child: it stores to a known cell millions of times a second and reports its
/// own store instruction's address. **Ours, spawned here, and nothing else is ever watched.**
pub struct Writer {
    child: Child,
    pub pid: i32,
    pub target: u64,
    pub decoy: u64,
    pub store: u64,
    pub next: u64,
}

impl Drop for Writer {
    /// ⚠ `SIGTERM`, never the un-catchable signal, and never `Child::kill()` - the same rule the
    /// whole crate lives by. The writer has no watchpoint of its own to lose, but a helper that
    /// reaches for the hard kill in *one* place is a helper someone copies.
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.pid, libc::SIGTERM);
        }
        let _ = self.child.wait();
    }
}

pub fn spawn_writer(live_bin: &Path, seconds: f64) -> Result<Writer, String> {
    let mut child = Command::new(live_bin)
        .arg("__writer")
        .arg("--seconds")
        .arg(seconds.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawning {} __writer: {e}", live_bin.display()))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|e| format!("reading the writer banner: {e}"))?;
    let toks: Vec<&str> = line.split_whitespace().collect();
    let get = |k: &str| -> Result<u64, String> {
        let i = toks.iter().position(|t| *t == k).ok_or(format!("banner has no {k}: {line:?}"))?;
        let v = toks.get(i + 1).ok_or(format!("banner truncated at {k}"))?;
        u64::from_str_radix(v.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
    };
    let pid = child.id() as i32;
    Ok(Writer {
        child,
        pid,
        target: get("TARGET")?,
        decoy: get("DECOY")?,
        store: get("STORE")?,
        next: get("NEXT")?,
    })
}

/// ⛔ The safety assertion that must precede anything destructive.
///
/// The primary discriminator is the **command line**, not the pid and not the parent: only a
/// `d1-live __writer` is ever a legal target here, and that is a property of the process itself
/// rather than of anyone's bookkeeping. `require_child` adds the parent check where it is
/// available - ⚠ it is **not** available inside the control-#9 helper, whose target was spawned by
/// its own parent, and inventing a parent relationship that does not hold would have been the
/// weaker guard dressed as the stronger one.
fn is_throwaway(pid: i32, require_child: bool) -> Result<(), String> {
    let comm = crate::child_mgr::comm_of(pid);
    if crate::child_mgr::comm_is_emulator(&comm) {
        return Err(format!("pid {pid} comm {comm:?} IS THE EMULATOR - refusing, loudly"));
    }
    if comm != "d1-live" {
        return Err(format!("pid {pid} comm is {comm:?}, not the throwaway writer"));
    }
    let cmd = crate::child_mgr::cmdline_of(pid);
    if !cmd.contains("__writer") {
        return Err(format!(
            "pid {pid} cmdline is {cmd:?} - only a `d1-live __writer` may be a control target"
        ));
    }
    if require_child {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_err(|e| format!("reading /proc/{pid}/stat: {e}"))?;
        // ppid is field 4, and the comm in field 2 may contain spaces - split after the last ')'.
        let tail = stat.rsplit_once(')').ok_or("malformed /proc stat")?.1;
        let ppid: i32 = tail
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or("no ppid in /proc stat")?;
        let me = std::process::id() as i32;
        if ppid != me {
            return Err(format!("pid {pid} has ppid {ppid}, not this process ({me}) - not ours"));
        }
    }
    Ok(())
}

fn wait_gone(pid: i32, max: Duration) -> bool {
    let deadline = Instant::now() + max;
    while Instant::now() < deadline {
        if !crate::child_mgr::alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !crate::child_mgr::alive(pid)
}

fn live_bin_or_fail(r: &mut ControlResult) -> Option<PathBuf> {
    let p = default_live_bin();
    if p.exists() {
        r.ok(format!("the d1-live binary is at {}", p.display()));
        Some(p)
    } else {
        // ⛔ NOT a skip. A control that cannot run has not passed.
        r.bad(format!(
            "the d1-live binary is not at {} - build it (`cargo build --release -p d1-live` from \
             d1-server/) or set $D1_LIVE_BIN. This control cannot be skipped into a pass.",
            p.display()
        ));
        None
    }
}

/// ⭐⭐ **THE FIXTURES WERE ALREADY IN THE TREE, WITH PROVENANCE, AND I DID NOT LOOK.**
///
/// `tests/fixtures/d1-live/` was recorded before this build started - four `--json` documents plus
/// a `provenance.json` naming the kernel, the d1-live binary, the measured `u_debugreg` offset and
/// **the emulator pids that were present and untouched**. I recorded a fifth, worse one first.
/// That is `status/LESSONS.md`'s standing failure - *search the tree before deriving anything* -
/// and the cost was not just duplication: the tree's set contains the **exec** document, whose
/// `rip == writer` is CORRECT, and my write-only fixture could never have caught the predicate that
/// got that wrong. ⇒ these are the fixtures. `fixtures/` is gone.
pub fn fixture_dir() -> PathBuf {
    crate::repo_root().join("d1-server/crates/d1-scope/tests/fixtures/d1-live")
}
/// The two-hit **write** document - the trap case, where `rip` is the instruction AFTER the store.
pub fn fixture_path() -> PathBuf {
    fixture_dir().join("watch-write-2hits.json")
}
/// The one-hit **exec** document - the FAULT case, where `rip` IS the instruction.
pub fn fixture_exec() -> PathBuf {
    fixture_dir().join("watch-exec-1hit.json")
}
/// ⭐ A parseable document with `hits: []` - a real negative result, not a failure.
pub fn fixture_nohit() -> PathBuf {
    fixture_dir().join("watch-write-nohit.json")
}
/// A run that produced no document at all: stderr only.
pub fn fixture_error_stderr() -> PathBuf {
    fixture_dir().join("watch-error.stderr.txt")
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #6 - the recorded report parses into the known event set
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c6_event_schema() -> ControlResult {
    let mut r = ControlResult::new("#6", "a d1-live --json report parses, and drift is CAUGHT");

    // ── (a) the checked-in fixture ───────────────────────────────────────────────────────────────
    let fp = fixture_path();
    let text = match std::fs::read_to_string(&fp) {
        Ok(t) => t,
        Err(e) => {
            r.bad(format!("reading the recorded fixture {}: {e}", fp.display()));
            return r;
        }
    };
    match parse_report(&text) {
        Ok((rep, missing)) => {
            r.check(missing.is_empty(), format!("fixture: 0 missing required keys ({missing:?})"));
            // ⭐ A parse that finds NOTHING proves nothing. The fixture must carry hits.
            r.check(
                !rep.hits.is_empty(),
                format!("fixture carries {} hit(s) - a zero-hit fixture cannot exercise the parser", rep.hits.len()),
            );
            r.check(
                rep.threads_armed >= 1 && rep.stop_reason.len() > 3,
                format!("fixture: {} thread(s) armed, stop_reason {:?}", rep.threads_armed, rep.stop_reason),
            );
            let quotable = rep.hits.iter().filter(|h| h.writer.is_some()).count();
            r.check(
                quotable == rep.hits.len(),
                format!("every one of the {} hit(s) decoded a writer ({quotable})", rep.hits.len()),
            );
            r.info(format!(
                "fixture rw={} len={} addr={:#x} confidence[0]={:?}",
                rep.rw, rep.len, rep.addr,
                rep.hits.first().map(|h| h.writer_confidence.clone()).unwrap_or_default()
            ));
        }
        Err(e) => r.bad(format!("fixture did not parse: {e}")),
    }

    // ── (b) THE INVERSION, in-line: drop a key we depend on and require it to be NAMED ───────────
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(mut v) => {
            v.as_object_mut().map(|o| o.remove("stop_reason"));
            let missing = missing_required_keys(&v);
            r.check(
                missing.contains(&"stop_reason".to_string()),
                format!("a report with `stop_reason` removed is REPORTED as drifted: {missing:?}"),
            );
            let mut v2: serde_json::Value = serde_json::from_str(&text).unwrap();
            if let Some(h) = v2.get_mut("hits").and_then(|h| h.as_array_mut()) {
                if let Some(first) = h.first_mut().and_then(|x| x.as_object_mut()) {
                    first.remove("writer_text");
                }
            }
            let m2 = missing_required_keys(&v2);
            r.check(
                m2.iter().any(|k| k.ends_with(".writer_text")),
                format!("a hit with `writer_text` removed is REPORTED too: {m2:?}"),
            );
        }
        Err(e) => r.bad(format!("re-reading the fixture as raw JSON: {e}")),
    }

    // ── (c) THE DRIFT DETECTOR: a report from TODAY'S binary, generated right now ─────────────────
    let Some(bin) = live_bin_or_fail(&mut r) else { return r };
    match record_report(&bin, 2, 4.0) {
        Ok((fresh, note)) => {
            r.info(note);
            match parse_report(&fresh) {
                Ok((rep, missing)) => {
                    r.check(
                        missing.is_empty(),
                        format!("FRESH report from today's d1-live: 0 missing keys ({missing:?})"),
                    );
                    r.check(
                        !rep.hits.is_empty(),
                        format!("FRESH report carries {} hit(s)", rep.hits.len()),
                    );
                }
                Err(e) => r.bad(format!("the FRESH report did not parse: {e} - d1-live's report shape has DRIFTED")),
            }
        }
        Err(e) => r.bad(format!("could not record a fresh report: {e}")),
    }

    // ── (d) ⭐ THE ZERO-HIT DOCUMENT IS A RESULT, NOT A FAILURE ──────────────────────────────────
    match std::fs::read_to_string(fixture_nohit()).map_err(|e| e.to_string()).and_then(|t| parse_report(&t)) {
        Ok((rep, missing)) => {
            r.check(missing.is_empty(), format!("the zero-hit document parses with 0 missing keys ({missing:?})"));
            r.check(
                rep.hits.is_empty() && rep.stop_reason.contains("max-seconds") && rep.threads_armed >= 1,
                format!(
                    "…and it is a REAL NEGATIVE: {} thread(s) armed, {:.1}s watched, stop_reason {:?}, 0 hits",
                    rep.threads_armed, rep.seconds, rep.stop_reason
                ),
            );
            r.info(
                "⛔ this document and the failure below BOTH make d1-live exit 1. Classifying on the \
                 exit code would file a real negative result as a broken run - which is why \
                 ChildStatus is decided on stdout PARSEABILITY.",
            );
        }
        Err(e) => r.bad(format!("the zero-hit fixture: {e}")),
    }
    // …and the failure: no document at all, stderr only.
    match std::fs::read_to_string(fixture_error_stderr()) {
        Ok(t) => {
            r.check(
                parse_report("").is_err() && !t.trim().is_empty(),
                format!("a run that produced NO document is Failed, with stderr verbatim: {:?}", t.trim()),
            );
        }
        Err(e) => r.bad(format!("the error-stderr fixture: {e}")),
    }

    // ── (e) ATTRIBUTION BY ADDRESS - the positive control, and its negative ──────────────────────
    attribution_legs(&mut r);
    r
}

/// ⭐ *A scan that finds nothing must be accompanied by one that finds something.*
///
/// The fixture's watched address is a control-writer heap cell, so it belongs to no registry
/// surface and every event from it renders `unattributed` - which is correct and proves nothing.
/// These legs put a synthetic event **on a real resolved surface** taken from a real snapshot and
/// require it to be named, field and all; then move it away and require the opposite.
fn attribution_legs(r: &mut ControlResult) {
    use crate::eval::Evaluator;
    use crate::events::{attach_by_address, declared_extent, surface_span};
    use crate::frame::Resolved;
    use crate::mem_source::{MemSource, SnapMem};
    use crate::sampler::Sampler;

    let reg = match crate::registry::Registry::open_default() {
        Ok(x) => x,
        Err(e) => return r.bad(format!("registry: {e:#}")),
    };
    let ms = match SnapMem::open(SnapMem::path_for_id("rung36-biped-chain")) {
        Ok(x) => x,
        Err(e) => return r.bad(format!("snapshot: {e:#}")),
    };
    let syms = match d1_live::symbols::Symbols::open_default() {
        Ok(x) => x,
        Err(e) => return r.bad(format!("symbols: {e:#}")),
    };
    let ev = match Evaluator::new(&ms as &dyn MemSource, &syms) {
        Ok(x) => x.with_registry(&reg),
        Err(e) => return r.bad(format!("evaluator: {e:#}")),
    };
    let f = Sampler::new(&reg, ev).tick();

    // ⭐⭐ SURFACES NEST, AND THE FIRST VERSION OF THIS LEG DID NOT KNOW IT. Asserting "the event
    //    lands in the surface I picked" FAILED against the real registry - `state_handler_table`
    //    IS `world_controller_state_manager + 0x80`, so an event at that offset legitimately lands
    //    in both and the tighter one is the right answer. The instrument was correct and the
    //    control's expectation was naive. ⇒ the identity leg now runs only where coverage is
    //    UNAMBIGUOUS, and nesting gets a leg of its own instead of being a surprise.
    let covering = |addr: u64| -> Vec<&crate::frame::SurfaceView> {
        f.surfaces
            .iter()
            .filter(|s| match s.resolved {
                Resolved::Addr(b) => addr >= b && addr < b + surface_span(s),
                _ => false,
            })
            .collect()
    };

    // ── the identity leg, on an UNAMBIGUOUSLY covered address ────────────────────────────────────
    let mut identity_done = false;
    for s in &f.surfaces {
        let Resolved::Addr(base) = s.resolved else { continue };
        for fld in &s.fields {
            if declared_extent(&fld.ty).is_none() {
                continue;
            }
            let a = base + fld.off;
            if covering(a).len() != 1 {
                continue;
            }
            let at = attach_by_address(&f.surfaces, a, 8, Some(&s.name));
            r.check(
                at.surface.as_deref() == Some(s.name.as_str())
                    && at.field.as_deref() == Some(fld.name.as_str()),
                format!(
                    "attribution POSITIVE: an event at {a:#x} is named {:?} field {:?} [{}] - \
                     derived from the ADDRESS, not from what the slot claimed",
                    at.surface, at.field, at.how
                ),
            );
            r.check(at.disagreement.is_none(), "…and the two derivations AGREE, so nothing is flagged");
            // …and a slot that claims the wrong surface is CALLED OUT rather than believed.
            let wrong = attach_by_address(&f.surfaces, a, 8, Some("a-surface-it-is-not"));
            r.check(
                wrong.disagreement.is_some(),
                format!("a slot claiming the wrong surface is FLAGGED: {:?}", wrong.disagreement),
            );
            // The negative: far outside every surface.
            let far = base.wrapping_add(surface_span(s)).wrapping_add(0x1000_0000);
            let out = attach_by_address(&f.surfaces, far, 8, None);
            r.check(
                out.surface.is_none() && out.how == "unattributed",
                format!("attribution NEGATIVE: an event at {far:#x} is UNATTRIBUTED, not attached to the nearest thing"),
            );
            identity_done = true;
            break;
        }
        if identity_done {
            break;
        }
    }
    if !identity_done {
        r.bad("no unambiguously-covered field in the whole registry - the identity leg never ran");
    }

    // ── the NESTING leg: where two surfaces cover one address, the TIGHTER one wins ──────────────
    let mut nested = 0usize;
    let mut nesting_done = false;
    for s in &f.surfaces {
        let Resolved::Addr(base) = s.resolved else { continue };
        for fld in &s.fields {
            if declared_extent(&fld.ty).is_none() {
                continue;
            }
            let a = base + fld.off;
            let c = covering(a);
            if c.len() < 2 {
                continue;
            }
            nested += 1;
            if nesting_done {
                continue;
            }
            let tightest = c.iter().min_by_key(|x| surface_span(x)).unwrap();
            let at = attach_by_address(&f.surfaces, a, 8, None);
            r.check(
                at.surface.as_deref() == Some(tightest.name.as_str()),
                format!(
                    "attribution NESTING: {a:#x} is covered by {} surfaces {:?}; the TIGHTEST \
                     ({}, span {}) is chosen, and the answer was {:?}",
                    c.len(),
                    c.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
                    tightest.name,
                    surface_span(tightest),
                    at.surface
                ),
            );
            nesting_done = true;
        }
    }
    r.info(format!(
        "{nested} field address(es) in this registry are covered by more than one surface - nesting \
         is normal here, not an anomaly"
    ));
    if !nesting_done {
        r.info("no nested coverage in this snapshot, so the nesting leg did not run");
    }
}

/// Drive one real, offline `d1-live --json watch` against a throwaway writer and hand back its
/// stdout. This is exactly how `fixtures/d1-live-watch-report.json` was produced.
pub fn record_report(live_bin: &Path, count: usize, max_seconds: f64) -> Result<(String, String), String> {
    let w = spawn_writer(live_bin, max_seconds + 6.0)?;
    is_throwaway(w.pid, true)?;
    let out = Command::new(live_bin)
        .arg("--json")
        .arg("watch")
        .arg("--pid")
        .arg(w.pid.to_string())
        .arg("--addr")
        .arg(format!("a:{:#x}", w.target))
        .arg("--no-base")
        .arg("--len")
        .arg("8")
        .arg("--rw")
        .arg("write")
        .arg("--count")
        .arg(count.to_string())
        .arg("--max-seconds")
        .arg(format!("{max_seconds:.1}"))
        .output()
        .map_err(|e| format!("running d1-live watch: {e}"))?;
    if out.status.code() == Some(2) {
        return Err("d1-live exited 2 - a CONTROL failed inside it; its output may not be cited".into());
    }
    let note = format!(
        "recorded against our own writer pid {} (store {:#x}, next {:#x}), exit {:?}",
        w.pid,
        w.store,
        w.next,
        out.status.code()
    );
    Ok((String::from_utf8_lossy(&out.stdout).to_string(), note))
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #7 - RIP is NOT the writer
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐⭐ **THE PREDICATE IS MODE-AWARE, AND THE TREE'S `exec` FIXTURE IS WHY.**
///
/// A **data** breakpoint is a *trap*: it fires once the store has retired, so `writer < rip` and
/// `rip == writer + len(writer)`. An **execute** breakpoint is a *fault*: it fires before the
/// instruction runs, so `rip` **IS** the instruction and `writer == rip` is the CORRECT answer.
///
/// ⛔ The first version of this control knew only the first rule. It would have failed the exec
/// document - i.e. it would have accused a correct reading of being the very bug it exists to
/// catch, and it would have done so confidently. A single-mode fixture could never have shown that;
/// `tests/fixtures/d1-live/watch-exec-1hit.json` showed it in one run.
fn trap_shape_is_right(
    rw: &str,
    rip: u64,
    writer: Option<u64>,
    chosen_len: Option<usize>,
) -> Result<(), String> {
    let Some(w) = writer else { return Err("no writer was decoded".into()) };
    if rw == "exec" {
        // A FAULT. RIP is the instruction, and anything else means the decode drifted.
        return if w == rip {
            Ok(())
        } else {
            Err(format!(
                "exec breakpoint: writer {w:#x} != rip {rip:#x}. An execute breakpoint is a FAULT: \
                 RIP IS the instruction, so decoding backwards from it names the previous one."
            ))
        };
    }
    // A TRAP.
    if w == rip {
        return Err(format!(
            "rip == writer == {rip:#x} on a {rw:?} watch. A data breakpoint is a TRAP: RIP is the \
             instruction AFTER the store. Reporting them equal names the wrong instruction."
        ));
    }
    if w > rip {
        return Err(format!("writer {w:#x} is AFTER rip {rip:#x}"));
    }
    match chosen_len {
        Some(l) if w + l as u64 == rip => Ok(()),
        Some(l) => Err(format!("writer {w:#x} + {l} != rip {rip:#x}")),
        None => Err("the chosen candidate has no length".into()),
    }
}

pub fn c7_backward_decode_labelling() -> ControlResult {
    let mut r = ControlResult::new(
        "#7",
        "raw RIP and the decoded store are both present, and the TRAP/FAULT distinction is honoured",
    );

    // ── the WRITE document: a TRAP. rip must be the instruction AFTER the store. ─────────────────
    let write = check_one_document(&mut r, &fixture_path(), "write");
    // ── the EXEC document: a FAULT. rip IS the instruction. ─────────────────────────────────────
    let exec = check_one_document(&mut r, &fixture_exec(), "exec");
    r.check(
        write && exec,
        "both recorded modes were checked - a single-mode fixture cannot exercise this axis",
    );

    // ⭐⭐ THE LEG THAT MAKES THE MODE-AWARENESS MEAN SOMETHING: the exec document's hits, judged
    //    by the WRITE rule, must FAIL. If they passed, the predicate would be mode-blind and the
    //    two documents would only look like two tests.
    if let Ok((rep, _)) = std::fs::read_to_string(fixture_exec()).map_err(|e| e.to_string()).and_then(|t| parse_report(&t)) {
        let mut all_rejected = !rep.hits.is_empty();
        for h in &rep.hits {
            let chosen = h.candidates.iter().find(|c| Some(c.addr) == h.writer);
            if trap_shape_is_right("write", h.rip, h.writer, chosen.map(|c| c.len)).is_ok() {
                all_rejected = false;
            }
        }
        r.check(
            all_rejected,
            "⭐ the exec document judged by the WRITE rule is REJECTED - the predicate is genuinely \
             mode-aware, not two names for one check",
        );
        // …and symmetrically, a write hit judged by the exec rule must fail too.
        if let Ok((w, _)) = std::fs::read_to_string(fixture_path()).map_err(|e| e.to_string()).and_then(|t| parse_report(&t)) {
            let h = &w.hits[0];
            r.check(
                trap_shape_is_right("exec", h.rip, h.writer, None).is_err(),
                "…and the write document judged by the EXEC rule is rejected as well",
            );
        }
    } else {
        r.bad("the exec fixture did not parse");
    }

    // ⭐ THE SYNTHETIC HALF: the same predicate must reject shapes no real hit should ever have.
    let bad = trap_shape_is_right("write", 0x1000, Some(0x1000), Some(3));
    r.check(
        bad.is_err(),
        format!("a synthetic WRITE hit with rip == writer is REJECTED: {}", bad.unwrap_err_or_default()),
    );
    let bad2 = trap_shape_is_right("write", 0x1000, Some(0x0ffd), Some(2));
    r.check(bad2.is_err(), "a synthetic hit whose writer length does not reach RIP is REJECTED");
    let bad3 = trap_shape_is_right("exec", 0x1000, Some(0x0ffd), None);
    r.check(bad3.is_err(), "a synthetic EXEC hit whose writer is NOT rip is REJECTED");
    let good = trap_shape_is_right("write", 0x1000, Some(0x0ffd), Some(3));
    r.check(good.is_ok(), "…and the same predicate ACCEPTS a correctly-shaped write hit");
    let good2 = trap_shape_is_right("exec", 0x1000, Some(0x1000), None);
    r.check(good2.is_ok(), "…and a correctly-shaped exec hit");
    r
}

/// Check every hit of one recorded document against the rule for its own access mode.
fn check_one_document(r: &mut ControlResult, path: &Path, want_rw: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        r.bad(format!("reading {}", path.display()));
        return false;
    };
    let (rep, _) = match parse_report(&text) {
        Ok(x) => x,
        Err(e) => {
            r.bad(format!("{}: {e}", path.display()));
            return false;
        }
    };
    r.check(rep.rw == want_rw, format!("{}: rw is {:?}", path.display(), rep.rw));
    r.check(!rep.hits.is_empty(), format!("{}: {} hit(s) to check", path.display(), rep.hits.len()));
    let mut ok = !rep.hits.is_empty();
    for h in &rep.hits {
        let chosen = h.candidates.iter().find(|c| Some(c.addr) == h.writer);
        match trap_shape_is_right(&rep.rw, h.rip, h.writer, chosen.map(|c| c.len)) {
            Ok(()) => r.ok(format!(
                "[{}] hit #{}: writer {:#x} `{}` [{}], RIP {:#x} - {}",
                rep.rw,
                h.seq,
                h.writer.unwrap_or(0),
                h.writer_text.clone().unwrap_or_default(),
                h.writer_confidence,
                h.rip,
                if rep.rw == "exec" {
                    "a FAULT: RIP IS the instruction".to_string()
                } else {
                    format!("a TRAP: RIP = writer + {} bytes", chosen.map(|c| c.len).unwrap_or(0))
                }
            )),
            Err(e) => {
                ok = false;
                r.bad(format!("[{}] hit #{}: {e}", rep.rw, h.seq));
            }
        }
        if h.candidates.len() > 1 {
            r.info(format!(
                "[{}] hit #{}: {} candidates end at RIP; confidence {:?} ⇒ quotable = {}",
                rep.rw,
                h.seq,
                h.candidates.len(),
                h.writer_confidence,
                h.writer_is_quotable()
            ));
        }
    }
    ok
}

trait ErrText {
    fn unwrap_err_or_default(self) -> String;
}
impl ErrText for Result<(), String> {
    fn unwrap_err_or_default(self) -> String {
        self.err().unwrap_or_else(|| "(it did not fail)".into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #8 ⭐ THE LOAD-BEARING ONE - a d1-scope panic ends in a CONFIRMED disarm
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c8_lifecycle() -> ControlResult {
    let mut r = ControlResult::new(
        "#8",
        "a d1-scope PANIC stops the child, the disarm is CONFIRMED, and the target survives",
    );
    let Some(bin) = live_bin_or_fail(&mut r) else { return r };

    // ── the debug-register offset, AS d1-LIVE MEASURED IT ────────────────────────────────────────
    let dbg = {
        let w = match spawn_writer(&bin, 8.0) {
            Ok(w) => w,
            Err(e) => {
                r.bad(e);
                return r;
            }
        };
        let out = Command::new(&bin)
            .arg("--json").arg("watch")
            .arg("--pid").arg(w.pid.to_string())
            .arg("--addr").arg(format!("a:{:#x}", w.target))
            .arg("--no-base").arg("--count").arg("1").arg("--max-seconds").arg("2.0")
            .output();
        match out {
            Ok(o) => crate::child_mgr::debugreg_offset_from(&String::from_utf8_lossy(&o.stderr)),
            Err(e) => {
                r.bad(format!("running d1-live to read its own debug-register receipt: {e}"));
                None
            }
        }
    };
    let Some(dbg) = dbg else {
        r.bad("d1-live did not print its debug-register receipt - refusing to run leg A blind");
        return r;
    };
    r.ok(format!("d1-live measured u_debugreg[0] at {dbg:#x}; d1-scope relays it, never transcribes it"));

    // ── LEG A: is abandonment lethal on THIS kernel? ─────────────────────────────────────────────
    //    Without this leg, leg B's survival is free and proves nothing.
    let (abandon_died, abandon_note) = {
        let mut wa = match spawn_writer(&bin, 8.0) {
            Ok(w) => w,
            Err(e) => {
                r.bad(e);
                return r;
            }
        };
        match is_throwaway(wa.pid, true) {
            Ok(()) => r.ok(format!(
                "leg A target pid {} is OUR OWN throwaway writer (comm d1-live, ppid = us) - checked \
                 before anything was armed on it",
                wa.pid
            )),
            Err(e) => {
                r.bad(format!("leg A refused: {e}"));
                return r;
            }
        }
        let st = Command::new(&bin)
            .arg("__arm-and-die")
            .arg("--pid").arg(wa.pid.to_string())
            .arg("--addr").arg(format!("{:#x}", wa.target))
            .arg("--dbg").arg(dbg.to_string())
            .stdout(Stdio::null())
            .status();
        match st {
            Ok(s) if s.success() => {}
            other => {
                r.bad(format!("the arm-and-abandon helper failed ({other:?}) - the question was never asked"));
                return r;
            }
        }
        // ⛔ ASK `waitpid`, NOT `/proc`. The abandoned writer is an unreaped child of this process,
        //    so `/proc/<pid>` survives its death as a ZOMBIE - and the first version of this leg
        //    read that as "the target survived an armed abandonment", i.e. it reported the kernel
        //    as harmless at the exact moment it had killed the target. The reassuring answer was
        //    the wrong one. `try_wait` also hands back HOW it died, which is the finding.
        let deadline = Instant::now() + Duration::from_millis(1500);
        let mut st = None;
        while Instant::now() < deadline && st.is_none() {
            st = wa.child.try_wait().ok().flatten();
            std::thread::sleep(Duration::from_millis(20));
        }
        let sig = st.as_ref().and_then(|s| {
            use std::os::unix::process::ExitStatusExt;
            s.signal()
        });
        let died = sig.is_some();
        (
            died,
            format!(
                "writer {} after an armed abandonment: {}",
                wa.pid,
                match (st.as_ref(), sig) {
                    (Some(_), Some(n)) => format!("KILLED by signal {n} ({})", strsignal(n)),
                    (Some(s), None) => format!("exited normally with {:?}", s.code()),
                    (None, _) => "still running".to_string(),
                }
            ),
        )
    };
    if abandon_died {
        r.ok(format!(
            "⛔ SAFETY FINDING re-measured from d1-scope's side: {abandon_note}. A tracer that dies \
             armed KILLS ITS TARGET on this kernel ⇒ leg B's survival below is NOT free."
        ));
    } else {
        r.info(format!(
            "⚠ DEGENERATE INVERSION: {abandon_note}. This kernel appears to flush the breakpoint on \
             implicit detach, so leg B's survival proves less than it does elsewhere. The receipt \
             assertions below still stand on their own. Do NOT carry this reading to another kernel."
        ));
    }

    // ── LEG B: arm through ChildMgr, then PANIC ──────────────────────────────────────────────────
    let wb = match spawn_writer(&bin, 20.0) {
        Ok(w) => w,
        Err(e) => {
            r.bad(e);
            return r;
        }
    };
    if let Err(e) = is_throwaway(wb.pid, true) {
        r.bad(format!("leg B refused: {e}"));
        return r;
    }
    let log = new_log();
    let armed_ok;
    let child_pid;
    {
        // ⭐⭐ `--count` IS DELIBERATELY UNREACHABLE, AND THE FIRST VERSION OF THIS CONTROL WAS
        //    WEAKER FOR NOT KNOWING IT. Measured: against a `__writer` the child reaches **1000
        //    hits in 0.30 s**. With `--count 1000` the child therefore exits and disarms ON ITS OWN
        //    before the survival check runs - so when the `Drop` guard was deleted for the
        //    inversion, "the target survived" and "TracerPid is 0" both still PASSED and only the
        //    receipt caught it. A control leg that passes with the guard deleted is decoration.
        //    ⇒ the child's ONLY way out of this control is our stop path.
        let mut mgr = match ChildMgr::new(wb.pid, Duration::from_secs(6), log.clone()) {
            Ok(m) => m.with_live_bin(&bin).with_count(1_000_000),
            Err(e) => {
                r.bad(format!("ChildMgr refused our own writer: {}", e.message()));
                return r;
            }
        };
        if let Err(e) = mgr.add_slot(SlotSpec {
            surface: Some("control-writer-cell".into()),
            addr_expr: format!("a:{:#x}", wb.target),
            len: 8,
            rw: Rw::Write,
        }) {
            r.bad(e);
            return r;
        }
        if let Err(e) = mgr.launch() {
            r.bad(format!("launch: {}", e.message()));
            return r;
        }
        armed_ok = mgr.wait_until_armed(Duration::from_secs(6));
        child_pid = mgr.child_pid().unwrap_or(0);

        // ⭐ THE SIMULATED PANIC. `catch_unwind` returning Err is itself the proof that this build
        //   unwinds - a `panic = "abort"` profile would run no destructor at all, and this control
        //   would be measuring a guard that cannot fire.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guarded = mgr; // moved in, so unwinding drops it here
            panic!("simulated d1-scope panic - control #8");
        }));
        std::panic::set_hook(hook);
        r.check(res.is_err(), "the simulated panic really unwound (so `Drop` runs; not a panic=abort build)");
    }

    r.check(armed_ok, format!("the child ({child_pid}) confirmed ARMED before the panic"));

    // ── the receipt the Drop guard left behind ───────────────────────────────────────────────────
    let rec = log.lock().ok().and_then(|l| l.receipts.last().cloned());
    match rec {
        None => r.bad("no stop receipt was recorded - the Drop guard did not run"),
        Some(rc) => {
            r.check(rc.how == "drop-guard", format!("the stop came from the Drop guard ({:?})", rc.how));
            r.check(
                rc.exit_signal.is_none(),
                format!("the child exited on its OWN path, not by a signal (exit_code {:?}, signal {:?})", rc.exit_code, rc.exit_signal),
            );
            r.check(
                rc.stop_reason.as_deref() == Some("interrupted"),
                format!("the child's own stop_reason is {:?} - it left through its term handler, which is the path that disarms", rc.stop_reason),
            );
            r.check(rc.complaints.is_empty(), format!("the child reported NO thread it could not prove disarmed ({:?})", rc.complaints));
            r.check(rc.confirmed_disarm, "⭐ confirmed_disarm - the conjunction of all four of the above");
            r.info(format!("waited {} ms for the child to leave; it took {} hit(s)", rc.waited_ms, rc.hits));
        }
    }

    // ── and the target ───────────────────────────────────────────────────────────────────────────
    std::thread::sleep(Duration::from_millis(400));
    let survived = crate::child_mgr::alive(wb.pid);
    r.check(survived, format!("⭐ the TARGET ({}) survived the panic", wb.pid));
    let tracers = tracer_pids(wb.pid).unwrap_or_default();
    r.check(tracers.is_empty(), format!("…and nothing traces it any more (TracerPid {tracers:?})"));
    r.info(
        "⚠ HOW \"DR7 reads 0\" IS ASSERTED BY A PROCESS THAT MAY NOT READ DEBUG REGISTERS: as a \
         conjunction, not a claim. (1) the child left through its interrupt path, whose disarm \
         re-reads DR7 until it reads back zero and shouts otherwise - and it shouted nothing; \
         (2) it wrote a complete report, so it reached the end of that path; (3) TracerPid is 0, so \
         it detached; (4) the target is alive where leg A's identical child DIED. Any one of these \
         alone would be weak.",
    );
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #9 - the lease backstop: survive d1-scope being hard-killed
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c9_lease_backstop() -> ControlResult {
    let mut r = ControlResult::new(
        "#9",
        "the child self-disarms at its own lease with NO message from d1-scope at all",
    );
    let Some(bin) = live_bin_or_fail(&mut r) else { return r };
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            r.bad(format!("locating this binary: {e}"));
            return r;
        }
    };
    // ⭐⭐ THE LEASE IS ASSERTED AGAINST THE ARGV BUILDER ITSELF, not against a copy of it. An
    //    assertion that reconstructs the command line separately cannot notice the real one losing
    //    its lease - the two would drift apart in silence.
    {
        use crate::child_mgr::{argv, LEASE_MAX};
        let spec = SlotSpec { surface: None, addr_expr: "a:0x1000".into(), len: 8, rw: Rw::Write };
        for lease in [0.5f64, 2.5, 20.0, 120.0] {
            let v = argv(4242, &spec, 32, Duration::from_secs_f64(lease), true, false);
            let i = v.iter().position(|a| a == "--max-seconds");
            let got: Option<f64> = i.and_then(|i| v.get(i + 1)).and_then(|x| x.parse().ok());
            r.check(
                got.map(|g| (g - lease).abs() < 0.05).unwrap_or(false),
                format!("argv() at lease {lease}s emits --max-seconds {got:?}"),
            );
        }
        // …and the bound is REFUSED, not clamped.
        let over = ChildMgr::new(std::process::id() as i32, LEASE_MAX + Duration::from_secs(1), new_log());
        r.check(
            matches!(over, Err(Refusal::LeaseOutOfBounds { .. })),
            format!("a lease past the {}s maximum is REFUSED, not clamped", LEASE_MAX.as_secs()),
        );
    }
    r.check(
        strsignal(UNCATCHABLE).to_lowercase().contains("kill"),
        format!("signal {UNCATCHABLE} is identified at runtime as {:?} - the un-catchable one, named by number", strsignal(UNCATCHABLE)),
    );

    let w = match spawn_writer(&bin, 40.0) {
        Ok(w) => w,
        Err(e) => {
            r.bad(e);
            return r;
        }
    };
    if let Err(e) = is_throwaway(w.pid, true) {
        r.bad(e);
        return r;
    }

    let dir = std::env::temp_dir().join(format!("d1-scope-c9-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let helper_out = dir.join("helper.out");
    let lease = 2.5f64;

    let fo = match std::fs::File::create(&helper_out) {
        Ok(f) => f,
        Err(e) => {
            r.bad(format!("creating {}: {e}", helper_out.display()));
            return r;
        }
    };
    let mut helper = match Command::new(&me)
        .arg("__abandon-child")
        .arg("--pid").arg(w.pid.to_string())
        // ⭐ THE **DECOY** CELL, 16 bytes from the one the writer stores to and never written.
        //   The axis of this control is the LEASE, so the child must have no other reason to exit:
        //   armed on the hot cell it would hit `--count` first and the measurement would be of the
        //   wrong thing. Armed here it sits there, doing exactly what a real watch that never fires
        //   does, until its own deadline lets go.
        .arg("--addr").arg(format!("a:{:#x}", w.decoy))
        .arg("--lease").arg(lease.to_string())
        .arg("--stdio-dir").arg(&dir)
        .arg("--live-bin").arg(&bin)
        .stdout(Stdio::from(fo))
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            r.bad(format!("spawning the abandon helper: {e} (this control needs the d1-scope BINARY, not the test harness)"));
            return r;
        }
    };

    // Wait for the helper to announce the grandchild's pid.
    let mut grandchild = 0i32;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && grandchild == 0 {
        if let Ok(t) = std::fs::read_to_string(&helper_out) {
            if let Some(l) = t.lines().find(|l| l.starts_with("CHILD ")) {
                grandchild = l[6..].trim().parse().unwrap_or(0);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if grandchild == 0 {
        r.bad("the helper never announced an armed d1-live child - nothing to abandon");
        unsafe { libc::kill(helper.id() as i32, libc::SIGTERM) };
        let _ = helper.wait();
        return r;
    }
    r.ok(format!("the helper armed d1-live child {grandchild} on our writer {} with a {lease}s lease", w.pid));

    // ── HARD-KILL d1-SCOPE'S STAND-IN. No Drop, no handler, no message. ─────────────────────────
    let size_at_kill = std::fs::metadata(&helper_out).map(|m| m.len()).unwrap_or(0);
    unsafe {
        libc::kill(helper.id() as i32, UNCATCHABLE);
    }
    let _ = helper.wait();
    r.check(!crate::child_mgr::alive(helper.id() as i32), "the helper is gone - it can no longer send anything");

    // ⭐ THE DISCRIMINATOR. If the child had already exited, its later absence would prove nothing.
    r.check(
        crate::child_mgr::alive(grandchild),
        format!("the orphaned d1-live child {grandchild} is STILL ALIVE immediately after the kill - so its exit below is the LEASE, not a coincidence"),
    );

    // ── wait out the lease ──────────────────────────────────────────────────────────────────────
    let gone = wait_gone(grandchild, Duration::from_secs_f64(lease + 6.0));
    r.check(gone, format!("the orphan exited by itself within its {lease}s lease + grace"));
    r.check(
        crate::child_mgr::alive(w.pid),
        format!("⭐ the TARGET ({}) survived being orphaned mid-watch", w.pid),
    );
    let tracers = tracer_pids(w.pid).unwrap_or_default();
    r.check(tracers.is_empty(), format!("nothing traces the target any more (TracerPid {tracers:?})"));

    // ⭐ The orphan's OWN report, on disk because the child's stdio went to files rather than to
    //   pipes owned by a process that no longer exists.
    match std::fs::read_to_string(dir.join("d1-live-0.out")) {
        Ok(t) => match parse_report(t.trim()) {
            Ok((rep, _)) => r.check(
                rep.stop_reason.contains("max-seconds"),
                format!("the orphan's own report says it stopped because of {:?}", rep.stop_reason),
            ),
            Err(e) => r.bad(format!("the orphan's report did not parse: {e}")),
        },
        Err(e) => r.bad(format!("the orphan wrote no report: {e}")),
    }

    // ── "no message from d1-scope AT ALL" ───────────────────────────────────────────────────────
    let size_now = std::fs::metadata(&helper_out).map(|m| m.len()).unwrap_or(0);
    r.check(
        size_now == size_at_kill,
        format!("d1-scope's stand-in wrote NOTHING after being killed ({size_at_kill} bytes then, {size_now} now) - the disarm was entirely the child's own"),
    );
    let _ = std::fs::remove_dir_all(&dir);
    r
}

/// The `__abandon-child` subcommand: launch the leased child, announce it, and then **stand there**
/// so that something else can hard-kill us. Never call this outside control #9.
pub fn abandon_child_main(
    target: i32,
    addr: String,
    lease: f64,
    stdio_dir: PathBuf,
    live_bin: PathBuf,
) -> Result<(), String> {
    if let Err(e) = is_throwaway(target, false) {
        // ⛔ Even the helper refuses a target that is not a throwaway of ours.
        return Err(format!("__abandon-child refuses pid {target}: {e}"));
    }
    let log = new_log();
    let mut mgr = ChildMgr::new(target, Duration::from_secs_f64(lease), log)
        .map_err(|e: Refusal| e.message())?
        .with_live_bin(live_bin)
        .with_stdio_dir(stdio_dir)
        .with_count(1000);
    let _ = mgr.add_slot(SlotSpec { surface: None, addr_expr: addr, len: 8, rw: Rw::Write })?;
    mgr.launch().map_err(|e| e.message())?;
    if !mgr.wait_until_armed(Duration::from_secs(8)) {
        return Err("the child never armed".into());
    }
    println!("CHILD {}", mgr.child_pid().unwrap_or(0));
    std::io::stdout().flush().ok();
    // ⭐ LEAK IT ON PURPOSE. Nothing here may ever stop the child: the whole point is that the only
    //   thing standing between the target and a permanently-armed debug register is the child's own
    //   `--max-seconds` lease.
    std::mem::forget(mgr);
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #10 - one tracer per process: refuse, do not fight
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn c10_single_tracer_refusal() -> ControlResult {
    let mut r = ControlResult::new("#10", "an already-traced target is REFUSED by message, and released on request");
    let Some(bin) = live_bin_or_fail(&mut r) else { return r };
    let w = match spawn_writer(&bin, 40.0) {
        Ok(w) => w,
        Err(e) => {
            r.bad(e);
            return r;
        }
    };
    if let Err(e) = is_throwaway(w.pid, true) {
        r.bad(e);
        return r;
    }

    // ── (a) THE NEGATIVE DIRECTION. An untraced target is NOT refused. ───────────────────────────
    //     Without this leg, a `check_target` that refused everything would sail through (b).
    let t0 = tracer_pids(w.pid).unwrap_or_default();
    r.check(t0.is_empty(), format!("a fresh writer has TracerPid 0 ({t0:?})"));
    r.check(check_target(w.pid, None).is_ok(), "…and check_target ACCEPTS it - the negative direction of the guard");

    // ── (b) our own child holds it ───────────────────────────────────────────────────────────────
    let log = new_log();
    let mut mgr = match ChildMgr::new(w.pid, Duration::from_secs(20), log) {
        Ok(m) => m.with_live_bin(&bin).with_count(1_000_000),
        Err(e) => {
            r.bad(format!("ChildMgr refused our own writer: {}", e.message()));
            return r;
        }
    };
    if let Err(e) = mgr.add_slot(SlotSpec {
        surface: None,
        addr_expr: format!("a:{:#x}", w.target),
        len: 8,
        rw: Rw::Write,
    }) {
        r.bad(e);
        return r;
    }
    if let Err(e) = mgr.launch() {
        r.bad(format!("launch: {}", e.message()));
        return r;
    }
    r.check(mgr.wait_until_armed(Duration::from_secs(6)), "our child armed the writer");
    let cp = mgr.child_pid().unwrap_or(0);
    let t1 = tracer_pids(w.pid).unwrap_or_default();
    r.check(t1 == vec![cp], format!("TracerPid is now our child {cp} ({t1:?})"));

    match check_target(w.pid, Some(cp)) {
        Ok(()) => r.bad("check_target ACCEPTED a target our own child already holds - a second child would race"),
        Err(e) => {
            let m = e.message();
            r.ok(format!("refused, with a message: {m}"));
            r.check(m.contains("Re-arm") || m.contains("re-arm"), "…and the message names the re-arm path");
            r.check(matches!(e, Refusal::HeldByOurChild { .. }), "…and it is the OUR-CHILD flavour, not the generic one");
        }
    }
    // Seen from a manager that does not know about our child, it is simply "held by another".
    match check_target(w.pid, None) {
        Ok(()) => r.bad("check_target ACCEPTED a traced target"),
        Err(e) => {
            let m = e.message();
            r.check(matches!(e, Refusal::HeldByAnother { .. }), "a foreign holder is reported as HeldByAnother");
            r.check(m.contains("release"), "…and the message names the RELEASE path so the operator can hand it over");
            r.check(
                m.contains("un-catchable"),
                "…and it warns against the un-catchable signal, which would kill the target",
            );
            r.check(m.contains(&cp.to_string()), format!("…and it names the holder ({cp})"));
        }
    }

    // ── (c) THE RELEASE PATH ─────────────────────────────────────────────────────────────────────
    let rec = mgr.release();
    match rec {
        None => r.bad("release() produced no receipt"),
        Some(rc) => {
            r.check(rc.confirmed_disarm, format!("release: confirmed_disarm (stop_reason {:?}, complaints {:?})", rc.stop_reason, rc.complaints));
            r.check(rc.exit_signal.is_none(), "release: the child left on its own path");
        }
    }
    let t2 = tracer_pids(w.pid).unwrap_or_default();
    r.check(t2.is_empty(), format!("after release the target is FREE again (TracerPid {t2:?})"));
    r.check(check_target(w.pid, None).is_ok(), "…and check_target accepts it again - another lane can take it");
    r.check(crate::child_mgr::alive(w.pid), "…and the target is still alive throughout");

    // ── (d) the emulator guard, on this path too ────────────────────────────────────────────────
    r.check(
        std::env::var("D1_SCOPE_ALLOW_LIVE").as_deref() != Ok("1"),
        "D1_SCOPE_ALLOW_LIVE is not set - ChildMgr would refuse the emulator outright",
    );
    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #15 - no stop path is silent
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⭐⭐ **THE REGRESSION CONTROL FOR THE ONE STOP PATH THAT WAS SILENT - AND FOR THE REASON IT
/// STAYED SILENT: NO CONTROL EVER CALLED IT.**
///
/// `ChildMgr::heartbeat()` is the **normal collection point**: it is what reaps a child that
/// finished on its own, which is every `--count`-bounded cycle and every self-expired lease. It
/// consumed `confirmed_disarm` only as a relaunch predicate and returned `Ok(())`, so a receipt
/// meaning *"the tracer may have died armed on the operator's client"* produced a clean terminal
/// and `$? == 0`. Controls #8/#9/#10 exercise `Drop`, the orphaned lease and `release()`; **none of
/// them calls `heartbeat()`**, so the suite read 100% while the most-travelled path in the shipped
/// binary carried no assertion at all. *A control that covers a path the product does not take is
/// decoration one level up.*
///
/// ⛔ **THIS CONTROL ARMS NOTHING.** The failure is provoked with an address `d1-live` cannot
/// resolve, so the child dies **before** it touches a debug register - it produces the exact
/// receipt the defect swallowed (empty stdout ⇒ `parse_report` fails ⇒ `confirmed_disarm == false`)
/// without ever putting a watchpoint on the target. The writer is asserted **alive afterwards**, so
/// "nothing was armed" is measured rather than argued.
///
/// ⚠ And the negative direction is a leg, not a footnote: a clean session must yield **zero**
/// unconfirmed receipts and exit code **0**. Without it, *"an unconfirmed disarm exits 2"* would
/// also pass for an implementation that exits 2 always.
pub fn c15_no_stop_path_is_silent() -> ControlResult {
    let mut r = ControlResult::new("#15", "no stop path is silent: an unconfirmed disarm exits 2 from EVERY route");
    let Some(bin) = live_bin_or_fail(&mut r) else { return r };

    // ── LEG A: the lethal reading, reached through `heartbeat()` ─────────────────────────────────
    let wa = match spawn_writer(&bin, 20.0) {
        Ok(w) => w,
        Err(e) => {
            r.bad(e);
            return r;
        }
    };
    match is_throwaway(wa.pid, true) {
        Ok(()) => r.ok(format!(
            "leg A target pid {} is OUR OWN throwaway writer (comm d1-live, ppid = us) - and \
             NOTHING is ever armed on it here",
            wa.pid
        )),
        Err(e) => {
            r.bad(format!("leg A refused: {e}"));
            return r;
        }
    }
    let log_a = new_log();
    let mut mgr = match ChildMgr::new(wa.pid, Duration::from_secs(6), log_a.clone()) {
        Ok(m) => m.with_live_bin(&bin).with_count(1_000_000),
        Err(e) => {
            r.bad(format!("ChildMgr refused our own writer: {}", e.message()));
            return r;
        }
    };
    // ⭐ The provocation: an address the shared grammar rejects. `SlotSpec::validate` accepts it
    //   (it is non-empty), d1-live rejects it, and the child exits 1 with EMPTY STDOUT - the
    //   `ChildStatus::Failed` shape, which is `confirmed_disarm == false` by construction.
    if let Err(e) = mgr.add_slot(SlotSpec {
        surface: Some("control-unresolvable".into()),
        addr_expr: "zzz-not-an-address".into(),
        len: 8,
        rw: Rw::Write,
    }) {
        r.bad(e);
        return r;
    }
    if let Err(e) = mgr.launch() {
        r.bad(format!("launch: {}", e.message()));
        return r;
    }
    let child_pid = mgr.child_pid().unwrap_or(0);

    // Poll exactly as `cmd_watch` does. ⚠ The first heartbeats legitimately return Ok(()) - the
    // child has not exited yet - so "it returned Err eventually" is the assertion, with a bound.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut refusal: Option<String> = None;
    let mut beats = 0u32;
    while Instant::now() < deadline {
        beats += 1;
        match mgr.heartbeat() {
            Ok(()) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                refusal = Some(e.message());
                break;
            }
        }
    }
    match &refusal {
        Some(m) => {
            r.ok(format!("⭐ heartbeat() REFUSED after {beats} beat(s) - the collection path is loud"));
            r.check(
                m.contains("could NOT be confirmed"),
                format!("…and the refusal names the condition verbatim: {:?}", m.chars().take(96).collect::<String>()),
            );
        }
        None => r.bad(format!(
            "⛔ heartbeat() returned Ok(()) for {beats} beat(s) while child {child_pid} died \
             unconfirmed - THIS IS THE DEFECT: the receipt is dropped and the session reads clean"
        )),
    }

    // The receipt really is in the recorder, and it is the one `heartbeat()` collected.
    let unconf = crate::child_mgr::unconfirmed_receipts(&log_a);
    r.check(
        unconf.iter().any(|x| x.how == "child-exited"),
        format!("the recorder holds the unconfirmed receipt, how={:?}", unconf.iter().map(|x| x.how.clone()).collect::<Vec<_>>()),
    );
    r.check(
        crate::child_mgr::session_exit_code(&log_a) == 2,
        "⛔ session_exit_code() == 2 - the shell's $? cannot disagree with the receipts",
    );

    // ⛔ AND NOTHING WAS ARMED: the target is alive. (Leg A of #8 measures that an armed
    //    abandonment KILLS it on this kernel, so this is not a free assertion.)
    drop(mgr);
    std::thread::sleep(Duration::from_millis(200));
    r.check(
        crate::child_mgr::alive(wa.pid),
        format!("the target ({}) is alive - this control never armed anything on it", wa.pid),
    );
    r.check(
        tracer_pids(wa.pid).unwrap_or_default().is_empty(),
        "…and nothing traces it",
    );
    drop(wa);

    // ── LEG B: THE NEGATIVE DIRECTION. A clean session must exit 0. ──────────────────────────────
    let wb = match spawn_writer(&bin, 20.0) {
        Ok(w) => w,
        Err(e) => {
            r.bad(e);
            return r;
        }
    };
    if let Err(e) = is_throwaway(wb.pid, true) {
        r.bad(format!("leg B refused: {e}"));
        return r;
    }
    let log_b = new_log();
    {
        let mut mgr = match ChildMgr::new(wb.pid, Duration::from_secs(8), log_b.clone()) {
            Ok(m) => m.with_live_bin(&bin).with_count(4),
            Err(e) => {
                r.bad(format!("ChildMgr refused our own writer: {}", e.message()));
                return r;
            }
        };
        if let Err(e) = mgr.add_slot(SlotSpec {
            surface: Some("control-writer-cell".into()),
            addr_expr: format!("a:{:#x}", wb.target),
            len: 8,
            rw: Rw::Write,
        }) {
            r.bad(e);
            return r;
        }
        if let Err(e) = mgr.launch() {
            r.bad(format!("launch: {}", e.message()));
            return r;
        }
        // `--count 4` against a writer that stores millions of times a second: the child reaches
        // its count and exits on its own, and `heartbeat()` collects it. ⭐ THIS IS THE PATH THE
        // PRODUCT TAKES, and it must be silent - because it is clean, not because it is unread.
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut ok_beats = 0u32;
        let mut leg_b_err: Option<String> = None;
        while Instant::now() < deadline {
            match mgr.heartbeat() {
                Ok(()) => ok_beats += 1,
                Err(e) => {
                    leg_b_err = Some(e.message());
                    break;
                }
            }
            if mgr.rearms() > 0 {
                break; // it was collected and relaunched - the confirmed path ran
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        r.check(
            leg_b_err.is_none(),
            format!("a CLEAN child is collected silently over {ok_beats} beat(s); refusal {leg_b_err:?}"),
        );
        r.check(
            mgr.rearms() > 0,
            format!("…and it really was collected and re-armed ({} re-arm(s)) - otherwise 'silent' is vacuous", mgr.rearms()),
        );
        // ⚠ The RELAUNCHED child needs to reach its own arm before it can write a report. Stopping
        //   it inside that window produces `confirmed_disarm == false` **correctly** - a child that
        //   wrote no report proves nothing - and the first version of this leg did exactly that and
        //   failed itself. That is the control working, not the product misbehaving: wait for the
        //   arm the same way `cmd_watch` does before demanding a receipt from it.
        let armed = mgr.wait_until_armed(Duration::from_secs(6));
        r.check(armed, "the re-armed child confirmed ARMED before we asked it for a receipt");
        std::thread::sleep(Duration::from_millis(300));
        let rec = mgr.stop("control-session-end");
        r.check(
            rec.as_ref().map(|x| x.confirmed_disarm).unwrap_or(false),
            format!("the session-end stop confirmed its disarm ({:?})", rec.as_ref().map(|x| x.stop_reason.clone())),
        );
    }
    let n_b = log_b.lock().map(|l| l.receipts.len()).unwrap_or(0);
    r.check(
        crate::child_mgr::unconfirmed_receipts(&log_b).is_empty(),
        format!("⭐ NEGATIVE DIRECTION: {n_b} receipt(s), ZERO unconfirmed"),
    );
    r.check(
        crate::child_mgr::session_exit_code(&log_b) == 0,
        "…so a clean session exits 0 - 'unconfirmed exits 2' is not a constant",
    );
    r.check(n_b >= 2, format!("…and there really were multiple stops to judge ({n_b})"));
    drop(wb);

    // ── LEG C: the SHIPPED command consults the receipts. ────────────────────────────────────────
    // ⛔ The evidence was never lost - it was UNREACHABLE from `cmd_watch`, which drained
    //    `reports` and never touched `receipts`. Assert the reader exists where it is needed.
    let mainp = crate::repo_root().join("d1-server/crates/d1-scope/src/main.rs");
    match std::fs::read_to_string(&mainp) {
        Ok(t) => {
            r.check(
                t.contains("unconfirmed_receipts(&log)"),
                "cmd_watch consults the session's receipts, not just the last one",
            );
            r.check(
                t.contains("session_exit_code(&log)"),
                "…and its exit code comes from the ONE decision function, not from a second copy of the rule",
            );
            r.check(
                !t.contains("None => println!(\"\\n(no child was running at session end)\")"),
                "⛔ the unconditional benign message is GONE (it was printed on the exact reading that means 'may have died armed')",
            );
        }
        Err(e) => r.bad(format!("reading {}: {e}", mainp.display())),
    }

    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub fn run_event_controls() -> Vec<ControlResult> {
    vec![
        c6_event_schema(),
        c7_backward_decode_labelling(),
        c8_lifecycle(),
        c9_lease_backstop(),
        c10_single_tracer_refusal(),
        c15_no_stop_path_is_silent(),
    ]
}
