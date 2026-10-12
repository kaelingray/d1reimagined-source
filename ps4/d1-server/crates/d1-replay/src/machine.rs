// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Snapshot → Unicorn, and the observation apparatus.
//!
//! # Why this does not reuse `d1-oracle::Harness`
//!
//! Boundary agreed with the `d1-oracle` session on 2026-08-07: **it owns eboot→Unicorn mapping,
//! codec invocation and schema-tag entry points; this crate owns snapshot capture/format, replay of
//! arbitrary functions, register/step/watchpoint observation and candidate search.** Two concrete
//! reasons the *mapping* cannot be shared even though `Harness::map_region` now exists:
//!
//! 1. **Different address space.** `Harness` maps the eboot into a file-address window,
//!    `0 … 0x300_0000`. A snapshot is in the client's own space - the eboot at `base` (measured,
//!    `0x8_0000_0000` here) and every captured pointer, vtable slot and heap object expressed
//!    relative to that. Rebasing a captured object graph down into the oracle's window means
//!    rewriting pointers, which is precisely the silent-corruption class this project keeps paying
//!    for.
//! 2. **One code hook, ever.** `Harness` deliberately owns its single code hook and closes the
//!    double-`_ret` trap *by construction*; a replay needs its own return trampoline, i.e. its own
//!    dispatch. Adding a second dispatching hook to somebody else's harness would re-open exactly
//!    the trap they closed.
//!
//! (Also flagged by that session and worth repeating: `d1-oracle`'s `registry::build` overwrites
//! `0x1A0B420` with a **synthetic** registry. Nothing in this crate goes near it - a replay against
//! a fabricated registry that *looks* like the captured one is the worst possible outcome.)
//!
//! # ⚠ What a replay is, and is not - AND IT NOW DEPENDS ON THE SNAPSHOT
//!
//! ⛔ **Do not quote a blanket answer here. Read the run's own `REGISTERS` block**, which every
//! rendering prints, in both modes, on every run ([`RegisterOrigin::render`]).
//!
//! * **INVENTED** (every snapshot without a trap, and any run that does not ask): *your* call, with
//!   *your* register values, against the client's *real* memory, fully observed. `/proc/<pid>/mem`
//!   has no registers in it, so the entry state was supplied, not recovered. Say "these inputs make
//!   this function hit", never "this is what the client did".
//! * **CAPTURED** (`--use-captured-regs`, on a snapshot taken by `capture --snapshot-on-hit`): the
//!   entry state is the trapping thread's own integer register file, read by `PTRACE_GETREGS` at a
//!   hardware-watchpoint stop. The honest sentence becomes *"this function, with the integer
//!   arguments the client actually held, against memory read over the following milliseconds while
//!   the other threads ran"* - and `snapshot::Coherence` carries the numbers behind that clause.
//!   ⛔ Still not "the state at the moment": `rip` is not seeded (the entry is `--func`), `rsp`/`rbp`
//!   are this run's own frame, and there is **no `xmm`/FP state at all**.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use unicorn_engine::{Arch, HookType, Mode, Prot, RegisterX86, Unicorn};

use crate::snapshot::{Snapshot, TrapProvenance, PAGE};

// ── ⭐ THE STACK CANARY - why a replay of almost any function died eight instructions in ─────────
//
// Nearly every function in this binary is stack-protected. The prologue is
//
//     mov rax, [rip+d32]        ; -> [0x1A06C78]      the guard POINTER
//     mov rax, [rax]                                  ; the guard VALUE
//     mov [rbp-0x18], rax
//
// and the epilogue compares. In the decompiled corpus this is the ubiquitous
// `local_X = *DAT_01a06c78;`.
//
// `0x1A06C78` is inside the captured eboot window, so we hold the client's own bytes - and the
// client's own bytes are a pointer into **host thread-local storage** (`0x55ed6ed63c80` in every
// snapshot taken so far). Unicorn has no `gs:`-backed TLS mapping behind it, so the *second* load
// is `READ_UNMAPPED` at a host-shaped address and the function dies at step 8, before it has done
// anything at all. The fault is real, but it is a **harness** limit, not a client fact - and at a
// glance it is indistinguishable from "the arguments were wrong".
//
// Measured 2026-08-09 (`d1-eboot xref 0x1a06c78`, its four controls PASS): **25,848 RIP-relative
// reference sites across 16,117 distinct functions** - **≥45%** of the binary's 35,834.
//
// ⚠ That is a FLOOR, not a count. `d1-eboot`'s riprel scanner has a known open defect: it drops the
// disp32 when the instruction carries a trailing immediate (`status/TODO.md`, "Instrument defects
// found" - `xref 0x266deac` reports 0 while `disasm` resolves 3), and all four xref controls pass
// anyway. Every miss is a reference it failed to find, so the true share can only be HIGHER.
// Quoting it as "45%" is safe **in this direction only**: it under-states how ubiquitous the guard
// is, and no argument here depends on the figure being tight. Do not reuse the same number for any
// claim that needs an upper bound.
//
// So this is not a corner case; without the redirect it is the default experience of the instrument.
//
// The fix is one pointer: make the guard pointer point at mapped, readable, *stable* guest memory,
// so the prologue's read and the epilogue's compare agree. ⚠ It is an ALTERATION of the client's
// captured memory, so it is reported on every run and recorded in every `--json` - see
// [`CanaryReport`]. ⛔ It does not make a function "work": `FUN_0010C500` still faults later, on
// content pages this snapshot does not hold. The claim it supports is exactly **"it executes past
// the canary"**, and nothing more.
//
// ⚠ The address is a constant *because the build gate makes it safe to be one*: controls 1-2 refuse
// any snapshot whose eboot is not the pinned build. The canary control additionally re-derives it
// from the instruction stream at a pinned call site, so a drifted constant FAILS loudly instead of
// silently redirecting some other global.
/// Eboot vaddr of the stack-guard **pointer** (the `__stack_chk_guard` indirection cell).
pub const GUARD_PTR_VADDR: u64 = 0x1A0_6C78;

/// The value planted in the machine's own canary slot. Distinctive on purpose: if this turns up in
/// a trace, a register dump or a fault address, it came from here and from nowhere else.
pub const CANARY_MAGIC: u64 = 0xD1D1_CA5A_D1D1_CA5A;

/// Where the stack-guard pointer should point for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackCanary {
    /// Point it at this machine's own canary slot - but **only** if the captured pointer is
    /// unmapped. A captured pointer that already resolves is left exactly as the client had it, so
    /// `auto` never alters memory that already worked.
    #[default]
    Auto,
    /// Leave the captured pointer alone. Stack-protected functions then fault in their prologue,
    /// which is the correct thing to want when the prologue fault is the thing being studied.
    Off,
    /// Point it at this absolute address.
    At(u64),
}

impl StackCanary {
    /// `auto` · `off`/`none`/`no` · anything else is resolved as an address by the caller, so the
    /// resolution is printed like every other one.
    pub fn parse(s: &str, resolve: &dyn Fn(&str) -> Result<u64>) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(StackCanary::Auto),
            "off" | "none" | "no" => Ok(StackCanary::Off),
            _ => Ok(StackCanary::At(resolve(s.trim())?)),
        }
    }
}

/// What was actually done to the guard pointer, and why. Carried on every [`RunReport`] so the
/// alteration cannot be invisible in either the human output or the JSON.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CanaryReport {
    /// `auto` | `off` | `at` | `unavailable`
    pub mode: String,
    /// Absolute address of the guard-pointer global, and its eboot vaddr.
    pub guard_ptr: u64,
    pub guard_ptr_vaddr: u64,
    /// The pointer value the CLIENT had there, read from the snapshot at machine construction - 
    /// never re-read afterwards, so a run that leaves the redirect in place cannot launder it.
    pub captured: Option<u64>,
    /// Whether that captured pointer resolves inside this machine's mapped memory.
    pub captured_mapped: bool,
    /// ⚠ The pointer value this run **intends** to write. This is a PLAN, not an observation:
    /// [`canary_plan`](Machine::canary_plan) fills it in before anything has happened, and `run()`
    /// carries the same value through. A control that asks "did the redirect happen?" by reading
    /// this field is asking the planner whether it planned - measured 2026-08-10 by deleting the
    /// `mem_write` and watching the FAIL row still print *"guard repointed at 0x700000051000"*.
    /// **Ask [`effective`](Self::effective).**
    pub applied: Option<u64>,
    /// ⭐ The pointer value **actually in memory at the instant execution started**, read back out
    /// of the machine after the redirect and after `spec.writes`. `None` on a plan (nothing has run
    /// yet) or when the guard global is not in the snapshot. This is the only field that is an
    /// observation, and it is what [`altered`](Self::altered) and
    /// [`redirect_landed`](Self::redirect_landed) are computed from.
    pub effective: Option<u64>,
    /// Whether the guard pointer, as it stands for this run, points at readable memory.
    pub resolves: bool,
    pub note: String,
}

impl CanaryReport {
    /// Did this run execute against memory that differs from the client's own captured bytes?
    /// Measured from the read-back, so it is false when a write was planned and did not land, and
    /// true when a *previous* un-rolled-back run left a redirect in place under `--stack-canary
    /// off` - which the plan alone would have reported as "left exactly as the client had it".
    pub fn altered(&self) -> bool {
        match (self.effective, self.captured) {
            (Some(e), Some(c)) => e != c,
            (Some(_), None) => true,
            _ => false,
        }
    }
    /// The redirect was planned **and the bytes are there**. This is the control-grade question.
    pub fn redirect_landed(&self) -> bool {
        self.applied.is_some() && self.effective == self.applied
    }
    pub fn render(&self, indent: &str) -> String {
        let cap = match self.captured {
            Some(v) => format!(
                "{v:#x} ({})",
                if self.captured_mapped { "mapped" } else { "UNMAPPED - host TLS" }
            ),
            None => "<not in this snapshot>".into(),
        };
        let mut s = format!(
            "{indent}STACK CANARY  mode={}  guard pointer v:{:#x} (abs {:#x}) held {cap}\n",
            self.mode, self.guard_ptr_vaddr, self.guard_ptr
        );
        match (self.applied, self.effective) {
            // Planned and observed to have landed - the ordinary redirect.
            (Some(p), Some(e)) if e == p => s.push_str(&format!(
                "{indent}              ⚠ ALTERED - redirected to {p:#x} for this run so \
                 stack-protected prologues resolve (read back from memory, not assumed).\n\
                 {indent}                That pointer is OURS, not the client's{}. It says nothing \
                 about what the guard\n{indent}                value was; it only lets execution \
                 continue past the check.\n",
                if self.resolves { "" } else { ", and ⛔ IT IS NOT MAPPED" }
            )),
            // Planned, ran, and the bytes are something else. Never say "redirected" here.
            (Some(p), Some(e)) => s.push_str(&format!(
                "{indent}              ⛔ THE REDIRECT DID NOT LAND - {p:#x} was planned, memory \
                 holds {e:#x} at execution start.\n{indent}                Treat this run as \
                 UNREDIRECTED; a stack-protected prologue will fault.\n"
            )),
            // A plan that has not been executed yet (`search`'s up-front preview).
            (Some(p), None) => s.push_str(&format!(
                "{indent}              ⚠ WILL BE ALTERED - every candidate redirects it to {p:#x}. \
                 Not yet written; each run reports what actually landed.\n"
            )),
            (None, Some(e)) if Some(e) != self.captured => s.push_str(&format!(
                "{indent}              ⛔ not redirected by this run, but memory holds {e:#x}, not \
                 the client's captured value - \n{indent}                something else wrote this \
                 guard pointer (a --set-mem, or an earlier run that was not rolled back).\n"
            )),
            (None, _) => s.push_str(&format!("{indent}              unaltered - {}\n", self.note)),
        }
        s
    }
}

/// Registers dumped and settable by name.
pub const REGS: &[(&str, RegisterX86)] = &[
    ("rax", RegisterX86::RAX),
    ("rbx", RegisterX86::RBX),
    ("rcx", RegisterX86::RCX),
    ("rdx", RegisterX86::RDX),
    ("rsi", RegisterX86::RSI),
    ("rdi", RegisterX86::RDI),
    ("rbp", RegisterX86::RBP),
    ("rsp", RegisterX86::RSP),
    ("r8", RegisterX86::R8),
    ("r9", RegisterX86::R9),
    ("r10", RegisterX86::R10),
    ("r11", RegisterX86::R11),
    ("r12", RegisterX86::R12),
    ("r13", RegisterX86::R13),
    ("r14", RegisterX86::R14),
    ("r15", RegisterX86::R15),
    ("rip", RegisterX86::RIP),
    ("eflags", RegisterX86::EFLAGS),
];

pub fn reg_by_name(name: &str) -> Option<RegisterX86> {
    let n = name.trim().to_ascii_lowercase();
    REGS.iter().find(|(k, _)| *k == n).map(|(_, r)| *r)
}

pub fn reg_name(r: RegisterX86) -> Option<&'static str> {
    REGS.iter().find(|(_, k)| *k == r).map(|(n, _)| *n)
}

/// System V AMD64 integer argument order.
pub const ARG_REGS: [RegisterX86; 6] = [
    RegisterX86::RDI,
    RegisterX86::RSI,
    RegisterX86::RDX,
    RegisterX86::RCX,
    RegisterX86::R8,
    RegisterX86::R9,
];

/// The same six, by name, so a report can say *which* register an `--arg` claimed.
pub const ARG_REG_NAMES: [&str; 6] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];

// ── ⭐⭐ CAPTURED vs INVENTED REGISTERS ───────────────────────────────────────────────────────────
//
// For its whole life this instrument had exactly one entry state: the one the caller typed.  That
// is what limitation 1 in `lib.rs` is about, and it is why every finding here has to be phrased
// *"these inputs make this function hit"* rather than *"this is what the client did"*.
//
// A trap-paired snapshot (`capture --snapshot-on-hit`) changes the input side of that sentence and
// nothing else: `PTRACE_GETREGS` hands back the trapping thread's whole integer register file, so a
// replay can start from the values the client ACTUALLY had.  Two rules keep that from becoming a new
// way to lie:
//
//   1. ⭐ **The mode is in every rendering, every time, in both modes.** A run that silently used
//      one or the other is worse than a run that used the wrong one, because nobody can audit it.
//      `RegisterOrigin` is a non-optional field of `RunReport` for exactly the reason `CanaryReport`
//      is: this instrument alters/supplies state, and a report that omits what it supplied reads as
//      a measurement.
//   2. ⭐ **Invented means ZEROED, not "whatever the last run left".** Before this existed, `run()`
//      never reset the register file, so candidate N+1 inherited candidate N's `r13` - the exact
//      silent-inheritance shape `rollback` and `verify_pristine` exist to catch on the memory side,
//      live on the register side and uncontrolled.  Now the baseline is defined in both modes, and
//      "invented" is a statement about a known state rather than about an unknown one.
//
// ⛔ Still integer-only.  No `xmm`, no FP: a float argument is not recoverable from a trap and a
// function that depends on `xmm` state cannot be replayed from one.  `RegisterOrigin` says so in
// words on every captured run, because the caveat that only lives in a doc is the caveat that gets
// quoted past.

/// A register baseline taken from a snapshot's trap capture.
///
/// ⚠ There is exactly one legitimate producer: `snapshot::ThreadRegs::regs` of a `Manifest::trap`
/// (see `main.rs`'s `prepare`).  `label` is printed verbatim in every rendering, so a stand-in built
/// by a control has to announce itself as one.
#[derive(Debug, Clone)]
pub struct CapturedRegs {
    /// Provenance, printed as-is: the method, the thread, the instant.
    pub label: String,
    /// Lowercase register name -> value, exactly [`crate::snapshot::ThreadRegs::regs`]' shape.
    pub regs: BTreeMap<String, u64>,
}

/// ⭐ Where THIS run's entry registers came from - always present, always printed.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RegisterOrigin {
    /// `"captured"` | `"invented"`. Never empty in a real report.
    pub mode: String,
    /// The trap's own provenance line, when a baseline was available (in either mode).
    pub source: Option<String>,
    /// TRUE when the snapshot carries a trap, **whatever this run chose to do with it**. An
    /// invented run against a trap-paired snapshot is a legitimate thing to want; it is not a
    /// legitimate thing to do by accident, so the report names the road not taken.
    pub captured_available: bool,
    /// Registers seeded from the capture, in the order applied.
    pub seeded: Vec<(String, u64)>,
    /// Registers deliberately NOT taken from the capture, each with its reason. ⭐ This is the half
    /// that stops "captured" being read as "all of them".
    pub not_seeded: Vec<(String, String)>,
    /// Registers whose captured value an explicit flag overrode, and which flag did it.
    pub overridden: Vec<(String, String)>,
    /// The frame: `true` = this run's own trampoline frame, `false` = `RSP` came from `--reg`.
    pub synthetic_frame: bool,
    /// Filled by [`Machine::run`] once the frame address is known.
    pub frame_rsp: Option<u64>,
    pub notes: Vec<String>,
}

/// x86 `EFLAGS` bits that must NOT be carried over from a live thread into an emulator.
///
/// `TF` (single-step) would make the guest trap on its own first instruction, and `RF` is a
/// resume-from-debug-exception flag that means nothing here. Both are cleared and the clearing is
/// reported - a masked bit that nobody is told about is a difference between the client's state and
/// the replay's state hiding inside a feature named "use the client's state".
pub const EFLAGS_UNSAFE: u64 = 0x100 /* TF */ | 0x1_0000 /* RF */;
/// What `eflags` is set to in invented mode: reserved bit 1, plus `IF`. Not zero, which is not a
/// legal `EFLAGS` value on real hardware.
pub const EFLAGS_INVENTED: u64 = 0x202;

impl RegisterOrigin {
    pub fn is_captured(&self) -> bool {
        self.mode == "captured"
    }

    /// Move a register out of `seeded` because something the spec cannot see will write it after
    /// the seeding - a **search knob**, which is applied per candidate inside
    /// [`crate::search::search`] and so is invisible to [`plan_register_origin`].
    ///
    /// ⭐ Without this, `d1-replay search --use-captured-regs --vary reg:R13=…` printed a preview
    /// claiming `r13` held the client's captured value while every candidate overwrote it. A
    /// preview that can disagree with the action is worse than no preview, and this is the second
    /// time that shape has come up in this file (see [`Machine::canary_plan`]).
    pub fn claimed_by(&mut self, name: &str, by: &str) {
        let lower = name.to_ascii_lowercase();
        if let Some(i) = self.seeded.iter().position(|(n, _)| *n == lower) {
            let (n, v) = self.seeded.remove(i);
            self.overridden.push((n, format!("captured {v:#x} replaced by {by}")));
        } else if !self.overridden.iter().any(|(n, _)| *n == lower) {
            self.overridden.push((lower, by.to_string()));
        }
    }

    /// ⭐ Printed on EVERY run, in both modes, in both renderings. Modelled on
    /// [`CanaryReport::render`] and for the same reason.
    pub fn render(&self, indent: &str) -> String {
        let mut s = String::new();
        if self.is_captured() {
            s.push_str(&format!(
                "{indent}REGISTERS     ⭐ CAPTURED - the entry state is the CLIENT's own, not this \
                 run's invention.\n{indent}              from {}\n",
                self.source.as_deref().unwrap_or("<no provenance - this is a bug>")
            ));
            let vals: Vec<String> =
                self.seeded.iter().map(|(n, v)| format!("{n}={v:#x}")).collect();
            s.push_str(&format!(
                "{indent}              seeded ({}): {}\n",
                self.seeded.len(),
                if vals.is_empty() { "NONE - ⛔ nothing was seeded".into() } else { vals.join(" ") }
            ));
            for (n, why) in &self.not_seeded {
                s.push_str(&format!("{indent}              not seeded  {n:<7} {why}\n"));
            }
            for (n, by) in &self.overridden {
                s.push_str(&format!("{indent}              OVERRIDDEN  {n:<7} {by}\n"));
            }
            s.push_str(&format!(
                "{indent}              ⛔ INTEGER REGISTERS ONLY - no xmm/FP was captured, so a \
                 float argument is still invented.\n"
            ));
        } else {
            s.push_str(&format!(
                "{indent}REGISTERS     INVENTED - every entry register is this run's own: all GPRs \
                 zeroed, eflags {EFLAGS_INVENTED:#x},\n{indent}              then --arg / --reg \
                 applied. This is NOT the client's entry state.\n"
            ));
            match (&self.source, self.captured_available) {
                (Some(src), true) => s.push_str(&format!(
                    "{indent}              ⭐ this snapshot DOES carry a trap ({src}) - \
                     --use-captured-regs would start from it.\n"
                )),
                _ => s.push_str(&format!(
                    "{indent}              this snapshot carries no trap, so there is no captured \
                     state to start from (NOT ATTEMPTED, not \"the client had none\").\n"
                )),
            }
        }
        match (self.synthetic_frame, self.frame_rsp) {
            (true, Some(r)) => s.push_str(&format!(
                "{indent}              frame  synthetic: rsp={r:#x} (return address = the \
                 trampoline), rbp=0\n"
            )),
            (true, None) => s.push_str(&format!("{indent}              frame  synthetic\n")),
            (false, Some(r)) => s.push_str(&format!(
                "{indent}              frame  ⚠ RSP SET BY --reg; the trampoline return address is \
                 at {r:#x}, NOT under your rsp,\n{indent}                     so this run will not \
                 stop by returning unless the callee returns into it.\n"
            )),
            (false, None) => s.push_str(&format!("{indent}              frame  ⚠ rsp set by --reg\n")),
        }
        for n in &self.notes {
            s.push_str(&format!("{indent}              {n}\n"));
        }
        s
    }
}

/// Decide - without touching Unicorn - exactly which register gets which value and why.
///
/// ⭐ ONE function computes it, and [`Machine::run`] then *applies what it decided*, so the preview
/// a caller prints before a search cannot disagree with the action. That is the same rule
/// [`Machine::canary_plan`] follows, and it exists because a preview that could differ from the
/// action is worse than no preview at all.
pub fn plan_register_origin(spec: &RunSpec, base: u64) -> RegisterOrigin {
    let mut o = RegisterOrigin {
        mode: if spec.use_captured { "captured" } else { "invented" }.into(),
        source: spec.captured.as_ref().map(|c| c.label.clone()),
        captured_available: spec.captured.is_some(),
        synthetic_frame: true,
        ..Default::default()
    };

    // What the explicit flags will claim. `--reg` is applied after `--arg`, so it wins - and the
    // report names the LAST writer, never the first.
    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
    for (i, a) in spec.args.iter().take(6).enumerate() {
        claimed.insert(ARG_REG_NAMES[i].to_string(), format!("--arg {} ({a:#x})", i + 1));
    }
    for (r, v) in &spec.regs {
        if let Some(n) = reg_name(*r) {
            claimed.insert(n.to_string(), format!("--reg {n}={v:#x}"));
        }
    }
    o.synthetic_frame = !claimed.contains_key("rsp");

    let Some(cap) = spec.captured.as_ref().filter(|_| spec.use_captured) else {
        return o;
    };

    for (name, _) in REGS {
        let name = *name;
        // RIP is the one register a replay cannot take from the capture: `emu_start` is given the
        // entry, so seeding RIP would be overwritten and reading as though it had not been.
        if name == "rip" {
            let rip = cap.regs.get("rip").copied();
            o.not_seeded.push((
                "rip".into(),
                match rip {
                    Some(v) if v >= base && v < base + 0x300_0000 => format!(
                        "the entry comes from --func. The capture's rip was {v:#x} = v:{:#x} - \
                         pass `--func v:{:#x}` to start there (⚠ a data breakpoint traps AFTER the \
                         storing instruction)",
                        v - base,
                        v - base
                    ),
                    Some(v) => format!(
                        "the entry comes from --func. The capture's rip was {v:#x}, outside the \
                         eboot window - pass `--func a:{v:#x}` to start there"
                    ),
                    None => "the entry comes from --func, and the capture has no rip".into(),
                },
            ));
            continue;
        }
        if name == "rsp" || name == "rbp" {
            o.not_seeded.push((
                name.into(),
                format!(
                    "this run uses its own frame so it can stop by returning{}. The capture's \
                     {name} was {} - `--reg {name}=…` takes it, and then the run ends at the step \
                     limit instead",
                    if name == "rbp" { " (rbp=0)" } else { "" },
                    cap.regs.get(name).map(|v| format!("{v:#x}")).unwrap_or_else(|| "absent".into())
                ),
            ));
            continue;
        }
        let Some(&v) = cap.regs.get(name) else {
            o.not_seeded.push((name.into(), "absent from the capture's register map".into()));
            continue;
        };
        if let Some(by) = claimed.get(name) {
            o.overridden.push((name.into(), format!("captured {v:#x} replaced by {by}")));
            continue;
        }
        if name == "eflags" {
            let masked = v & !EFLAGS_UNSAFE;
            if masked != v {
                o.notes.push(format!(
                    "⚠ eflags {v:#x} carried TF/RF; seeded {masked:#x} instead (single-step and \
                     resume-from-debug flags do not cross into an emulator)"
                ));
            }
            o.seeded.push(("eflags".into(), masked));
            continue;
        }
        o.seeded.push((name.into(), v));
    }
    o
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RegDump(pub BTreeMap<String, u64>);

impl RegDump {
    pub fn get(&self, name: &str) -> Option<u64> {
        self.0.get(&name.to_ascii_lowercase()).copied()
    }
    pub fn render(&self, indent: &str) -> String {
        let order = [
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
            "r13", "r14", "r15", "rip", "eflags",
        ];
        let mut s = String::new();
        for chunk in order.chunks(4) {
            s.push_str(indent);
            for k in chunk {
                if let Some(v) = self.0.get(*k) {
                    s.push_str(&format!("{k:>6} {v:#018x}  "));
                }
            }
            s.push('\n');
        }
        s
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceLine {
    pub index: u64,
    pub rip: u64,
    /// The eboot vaddr, when RIP is inside the eboot window.
    pub vaddr: Option<u64>,
    pub bytes: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemEvent {
    pub rip: u64,
    pub kind: String,
    pub addr: u64,
    pub size: usize,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum Exit {
    /// RIP reached the return trampoline: a clean return.
    Returned,
    /// A memory access the snapshot does not cover, or a protection violation.
    Fault { kind: String, addr: u64, size: usize, rip: u64 },
    /// The instruction budget ran out - usually a loop, occasionally a missing stub.
    StepLimit { steps: u64 },
    /// Unicorn stopped somewhere unexpected.
    Stopped { rip: u64, reason: String },
    Interrupt { intno: u32, rip: u64 },
}

impl Exit {
    pub fn is_fault(&self) -> bool {
        matches!(self, Exit::Fault { .. })
    }
    pub fn ok(&self) -> bool {
        matches!(self, Exit::Returned)
    }
    /// One clause, no advice - for embedding inside a longer sentence (a control's detail line).
    /// [`render`](Self::render) is the standalone form and keeps the "a fault is a RESULT" reminder,
    /// which is exactly wrong halfway through someone else's paragraph.
    pub fn short(&self) -> String {
        match self {
            Exit::Returned => "returned".into(),
            Exit::Fault { kind, addr, size, .. } => format!("FAULT {kind} at {addr:#x} (size {size})"),
            Exit::StepLimit { steps } => format!("step limit after {steps}"),
            Exit::Stopped { rip, reason } => format!("stopped at {rip:#x}: {reason}"),
            Exit::Interrupt { intno, rip } => format!("interrupt {intno:#x} at {rip:#x}"),
        }
    }
    pub fn render(&self) -> String {
        match self {
            Exit::Returned => "RETURNED".into(),
            Exit::Fault { kind, addr, size, rip } => format!(
                "FAULT {kind} at {addr:#x} (size {size}) from RIP {rip:#x}   ← a fault is a RESULT, \
                 not a tool crash"
            ),
            Exit::StepLimit { steps } => format!("STEP LIMIT after {steps} instructions"),
            Exit::Stopped { rip, reason } => format!("STOPPED at {rip:#x}: {reason}"),
            Exit::Interrupt { intno, rip } => format!("INTERRUPT {intno:#x} at {rip:#x}"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub entry: u64,
    pub args: Vec<u64>,
    pub exit: Exit,
    pub rax: u64,
    /// ⚠ Many of these functions return a **bool in `al`**. `rax`'s upper bytes are whatever the
    /// callee left there and are NOT part of the return value - reporting them as one number is how
    /// a `true` reads as `0x800000001`. Both are printed; the caller must pick the right one.
    pub al: u8,
    pub eax: u32,
    pub steps: u64,
    pub final_regs: RegDump,
    pub trace: Vec<TraceLine>,
    pub trace_truncated: bool,
    /// Register state the first time each `--probe` address was reached.
    pub probes: BTreeMap<u64, RegDump>,
    pub probe_counts: BTreeMap<u64, u64>,
    pub watch_events: Vec<MemEvent>,
    /// ⭐ What was done to the stack-guard pointer for this run. Never `None`, never omitted: a run
    /// whose memory we altered must say so in its own report, in both renderings.
    pub canary: CanaryReport,
    /// ⭐ Where this run's ENTRY REGISTERS came from - the client's capture, or this run's own
    /// invention. Never optional, for the same reason `canary` is not: the one thing a reader must
    /// never have to guess is whether a result is the client's state or ours.
    pub regs_origin: RegisterOrigin,
    /// ⭐ Where this run's MEMORY came from, when the snapshot was paired with a hardware-watchpoint
    /// trap. `None` = the snapshot carries no trap, which is NOT ATTEMPTED rather than "the client
    /// had none" - and it is rendered in **both** states for exactly the reason `canary` and
    /// `regs_origin` are.
    ///
    /// ⛔ It exists because `replay` - the most-used subcommand - printed the word "trap" zero times
    /// in either rendering, so a transcript from a trap-paired snapshot pasted into a `design/` doc
    /// was silent about the coherence smear while `--use-captured-regs` invited the strongest
    /// possible reading of it.
    pub trap: Option<TrapProvenance>,
    pub notes: Vec<String>,
}

impl RunReport {
    pub fn reached(&self, addr: u64) -> bool {
        self.probe_counts.get(&addr).copied().unwrap_or(0) > 0
    }
}

/// What to run and what to observe.
#[derive(Debug, Clone, Default)]
pub struct RunSpec {
    pub entry: u64,
    pub args: Vec<u64>,
    /// Explicit register overrides, applied after `args`.
    pub regs: Vec<(RegisterX86, u64)>,
    /// Memory to write before the run (scratch setup, candidate values).
    pub writes: Vec<(u64, Vec<u8>)>,
    pub step_limit: u64,
    /// `Some(n)` records up to `n` single-step lines.
    pub trace: Option<usize>,
    pub probes: Vec<u64>,
    pub watch_reads: Vec<(u64, u64)>,
    pub watch_writes: Vec<(u64, u64)>,
    pub watch_limit: usize,
    /// Addresses to short-circuit: `(addr, return_value)` - the call returns immediately.
    pub stubs: Vec<(u64, u64)>,
    /// Roll memory back to the snapshot after the run so the next call starts pristine.
    pub rollback: bool,
    /// ⭐ Where the stack-guard pointer should point. Defaults to [`StackCanary::Auto`], which is
    /// why a stack-protected function now runs at all. Applied BEFORE `writes`, so an explicit
    /// `--set-mem` on the guard global still wins.
    pub canary: StackCanary,

    /// ⭐ The client's own register file, when the snapshot carries a trap. **Filled in whether or
    /// not it is used**, so the report can name the road not taken.
    pub captured: Option<CapturedRegs>,
    /// ⭐ Start from `captured` instead of from zeros (`--use-captured-regs`). ⚠ With
    /// `captured: None` this is a caller bug, not a fallback: [`Machine::run`] refuses rather than
    /// quietly inventing registers under a flag whose whole purpose is to stop inventing them.
    pub use_captured: bool,
}

impl RunSpec {
    pub fn new(entry: u64) -> Self {
        Self { entry, step_limit: 2_000_000, watch_limit: 256, rollback: true, ..Default::default() }
    }
}

struct Shared {
    steps: u64,
    step_limit: u64,
    stop_addr: u64,
    exit: Option<Exit>,
    trace_budget: usize,
    trace: Vec<TraceLine>,
    trace_truncated: bool,
    probes: HashSet<u64>,
    probe_hits: BTreeMap<u64, RegDump>,
    probe_counts: BTreeMap<u64, u64>,
    stubs: HashMap<u64, u64>,
    /// Ranges written during the run, for rollback.
    dirty: Vec<(u64, usize)>,
    watch_events: Vec<MemEvent>,
    watch_limit: usize,
    base: u64,
    eboot_hi: u64,
    notes: Vec<String>,
}

fn regdump(uc: &mut Unicorn<'_, ()>) -> RegDump {
    let mut m = BTreeMap::new();
    for (name, r) in REGS {
        if let Ok(v) = uc.reg_read(*r) {
            m.insert((*name).to_string(), v);
        }
    }
    RegDump(m)
}

pub struct Machine {
    uc: Unicorn<'static, ()>,
    sh: Rc<RefCell<Shared>>,
    snap: Rc<Snapshot>,
    pub base: u64,
    pub scratch: u64,
    pub scratch_len: u64,
    pub stack: u64,
    pub stack_len: u64,
    pub stop: u64,
    /// The 8-byte slot holding [`CANARY_MAGIC`]. Its own page, not scratch and not the stack: a
    /// canary that shares a page with a buffer the callee writes would be clobbered by the very
    /// function it is meant to let run, and the epilogue compare would fail for a reason that looks
    /// exactly like a real stack smash.
    pub canary: u64,
    /// Absolute address of the guard-pointer global, `base + `[`GUARD_PTR_VADDR`].
    pub guard_ptr: u64,
    /// The pointer the CLIENT had at `guard_ptr`, read once from the snapshot at construction.
    pub guard_captured: Option<u64>,
    pub mapped: Vec<(u64, u64)>,
}

fn uce<T>(r: std::result::Result<T, unicorn_engine::uc_error>) -> Result<T> {
    r.map_err(|e| anyhow!("unicorn: {e:?}"))
}

impl Machine {
    /// Rehydrate a snapshot into Unicorn **at the client's own addresses**.
    pub fn new(snap: Rc<Snapshot>) -> Result<Self> {
        let base = snap.base();
        let mut uc = uce(Unicorn::new(Arch::X86, Mode::MODE_64))?;

        // 1. Map and fill every captured region.
        let mut mapped: Vec<(u64, u64)> = Vec::new();
        for (addr, blob) in snap.regions() {
            let lo = addr & !(PAGE - 1);
            let hi = (addr + blob.len() as u64).div_ceil(PAGE) * PAGE;
            if mapped.iter().any(|(a, l)| *a < hi && lo < a + l) {
                bail!("snapshot regions overlap after page alignment at {lo:#x} - refusing to guess");
            }
            uce(uc.mem_map(lo, hi - lo, Prot::ALL))?;
            uce(uc.mem_write(addr, blob))?;
            mapped.push((lo, hi - lo));
        }
        mapped.sort();

        // 2. Scaffolding - stack, scratch and the return trampoline - placed in a hole that does
        //    not collide with anything captured. Never a fixed constant: a snapshot's address space
        //    is the client's, and it is not ours to assume anything about.
        let scratch_len = 0x10_000u64;
        let stack_len = 0x40_000u64;
        // Layout: [scratch][stack][trampoline page][canary page]. The canary gets a page of its own
        // so that neither the wholesale re-zeroing of scratch+stack nor a callee scribbling on the
        // buffer it was handed can touch it.
        let need = scratch_len + stack_len + 2 * PAGE;
        let hole = find_hole(&mapped, need)?;
        let scratch = hole;
        let stack = hole + scratch_len;
        let stop = hole + scratch_len + stack_len;
        let canary = stop + PAGE;
        uce(uc.mem_map(hole, need, Prot::ALL))?;
        uce(uc.mem_write(stop, &[0xF4]))?; // hlt - never executed; the code hook stops first.
        uce(uc.mem_write(canary, &CANARY_MAGIC.to_le_bytes()))?;
        mapped.push((hole, need));
        mapped.sort();

        let guard_ptr = base + GUARD_PTR_VADDR;
        let guard_captured = snap.u64(guard_ptr).ok();

        let eboot_hi = snap
            .manifest
            .eboot
            .load_segments
            .iter()
            .map(|(_, hi)| *hi)
            .max()
            .unwrap_or(0x300_0000);

        let sh = Rc::new(RefCell::new(Shared {
            steps: 0,
            step_limit: 2_000_000,
            stop_addr: stop,
            exit: None,
            trace_budget: 0,
            trace: Vec::new(),
            trace_truncated: false,
            probes: HashSet::new(),
            probe_hits: BTreeMap::new(),
            probe_counts: BTreeMap::new(),
            stubs: HashMap::new(),
            dirty: Vec::new(),
            watch_events: Vec::new(),
            watch_limit: 256,
            base,
            eboot_hi,
            notes: Vec::new(),
        }));

        // ── THE ONE CODE HOOK ────────────────────────────────────────────────────────────────────
        // `(1, 0)` is Unicorn's "every address" idiom. Exactly one, for the life of the machine;
        // everything per-address is DATA in `Shared`, never a replaced callback. (The alternative
        // cost `d1-oracle`'s Python ancestor a session: a second hook popped the stack twice.)
        let h = sh.clone();
        uce(uc.add_code_hook(1, 0, move |uc, address, size| {
            let mut s = h.borrow_mut();
            if s.exit.is_some() {
                let _ = uc.emu_stop();
                return;
            }
            s.steps += 1;
            if s.steps > s.step_limit {
                s.exit = Some(Exit::StepLimit { steps: s.steps });
                drop(s);
                let _ = uc.emu_stop();
                return;
            }
            if address == s.stop_addr {
                s.exit = Some(Exit::Returned);
                drop(s);
                let _ = uc.emu_stop();
                return;
            }
            if s.probes.contains(&address) {
                *s.probe_counts.entry(address).or_insert(0) += 1;
                if !s.probe_hits.contains_key(&address) {
                    drop(s);
                    let d = regdump(uc);
                    h.borrow_mut().probe_hits.insert(address, d);
                    s = h.borrow_mut();
                }
            }
            if s.trace.len() < s.trace_budget {
                let n = s.steps;
                let (base, hi) = (s.base, s.eboot_hi);
                drop(s);
                let bytes = uc.mem_read_as_vec(address, size.max(1).min(16) as usize).unwrap_or_default();
                let vaddr = (address >= base && address < base + hi).then(|| address - base);
                let text = disasm_one(&bytes, address);
                h.borrow_mut().trace.push(TraceLine {
                    index: n,
                    rip: address,
                    vaddr,
                    bytes: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                    text,
                });
                s = h.borrow_mut();
            } else if s.trace_budget > 0 && !s.trace_truncated && s.trace.len() >= s.trace_budget {
                s.trace_truncated = true;
            }
            if let Some(ret) = s.stubs.get(&address).copied() {
                drop(s);
                // `_ret(value)`: pop the return address, set rax, resume there.
                if let Ok(rsp) = uc.reg_read(RegisterX86::RSP) {
                    let mut b = [0u8; 8];
                    if uc.mem_read(rsp, &mut b).is_ok() {
                        let _ = uc.reg_write(RegisterX86::RAX, ret);
                        let _ = uc.reg_write(RegisterX86::RSP, rsp + 8);
                        let _ = uc.reg_write(RegisterX86::RIP, u64::from_le_bytes(b));
                    }
                }
            }
        }))?;

        // ── faults are RESULTS ───────────────────────────────────────────────────────────────────
        // Return `false` so Unicorn still raises: we record the detail, we never paper over it by
        // mapping a zero page. A range the snapshot does not cover must not become "it was zero".
        let h = sh.clone();
        uce(uc.add_mem_hook(HookType::MEM_INVALID, 1, 0, move |uc, t, addr, size, _v| {
            let rip = uc.pc_read().unwrap_or(0);
            let mut s = h.borrow_mut();
            if s.exit.is_none() {
                s.exit = Some(Exit::Fault { kind: format!("{t:?}"), addr, size, rip });
            }
            false
        }))?;

        // ── the write journal, for rollback ──────────────────────────────────────────────────────
        // We record only (addr, size). Restoring from the SNAPSHOT afterwards is correct whether
        // the hook fires before or after the store - an assumption about hook ordering is exactly
        // the kind of thing that would silently turn rollback into a no-op.
        let h = sh.clone();
        uce(uc.add_mem_hook(HookType::MEM_WRITE, 1, 0, move |_uc, _t, addr, size, _v| {
            h.borrow_mut().dirty.push((addr, size));
            true
        }))?;

        let h = sh.clone();
        uce(uc.add_intr_hook(move |uc, intno| {
            let rip = uc.pc_read().unwrap_or(0);
            let mut s = h.borrow_mut();
            if s.exit.is_none() {
                s.exit = Some(Exit::Interrupt { intno, rip });
            }
            drop(s);
            let _ = uc.emu_stop();
        }))?;

        // ⚠ AVX shim, FAIL CLOSED. `d1-oracle` documents the trap: Destiny's codecs are
        // AVX-compiled and `vxorps ymm,ymm,ymm` raises `INSN_INVALID`, which reads like a mapping
        // fault. We do not emulate anything here - we NAME it, so a puzzling invalid-instruction
        // exit is not mistaken for bad memory. Inventing register contents would be worse.
        let h = sh.clone();
        uce(uc.add_insn_invalid_hook(move |uc| {
            let rip = uc.pc_read().unwrap_or(0);
            let b = uc.mem_read_as_vec(rip, 8).unwrap_or_default();
            let mut s = h.borrow_mut();
            s.notes.push(format!(
                "invalid instruction at {rip:#x}: {b:02x?} - if this is a VEX prefix (0xC4/0xC5) \
                 the function is AVX-compiled and this Unicorn has no AVX; that is a HARNESS limit, \
                 not a client defect"
            ));
            if s.exit.is_none() {
                s.exit = Some(Exit::Stopped { rip, reason: "invalid instruction".into() });
            }
            false
        }))?;

        Ok(Self {
            uc,
            sh,
            snap,
            base,
            scratch,
            scratch_len,
            stack,
            stack_len,
            stop,
            canary,
            guard_ptr,
            guard_captured,
            mapped,
        })
    }

    /// What [`run`](Self::run) *will* do to the stack-guard pointer under `mode` - computed without
    /// running anything, so `search` can print the alteration once up front and `run` can apply the
    /// very same plan. One function, two callers: a preview that could disagree with the action
    /// would be worse than no preview at all.
    pub fn canary_plan(&self, mode: StackCanary) -> CanaryReport {
        let mapped_at = |a: u64| self.uc.mem_read_as_vec(a, 8).is_ok();
        let base_report = |mode: &str, applied: Option<u64>, note: String| CanaryReport {
            mode: mode.into(),
            guard_ptr: self.guard_ptr,
            guard_ptr_vaddr: GUARD_PTR_VADDR,
            captured: self.guard_captured,
            captured_mapped: self.guard_captured.map(mapped_at).unwrap_or(false),
            resolves: match applied {
                Some(p) => mapped_at(p),
                None => self.guard_captured.map(mapped_at).unwrap_or(false),
            },
            applied,
            // A plan has observed nothing. `run()` fills this in from memory.
            effective: None,
            note,
        };
        // The global itself must be there before anything can be written to it. "Not captured" is a
        // selection gap, and it is reported as one - never quietly treated as "nothing to do".
        if self.guard_captured.is_none() || !mapped_at(self.guard_ptr) {
            return base_report(
                "unavailable",
                None,
                format!(
                    "the guard-pointer global v:{GUARD_PTR_VADDR:#x} is not in this snapshot, so \
                     there is nothing to redirect - a stack-protected function will fault reading \
                     it. That is a SELECTION gap in the capture, not a result."
                ),
            );
        }
        match mode {
            StackCanary::Off => base_report(
                "off",
                None,
                "left exactly as the client had it (--stack-canary off). A stack-protected \
                 function faults ~8 instructions in, dereferencing this pointer."
                    .into(),
            ),
            StackCanary::At(p) => base_report("at", Some(p), String::new()),
            StackCanary::Auto => {
                let captured = self.guard_captured.unwrap();
                if mapped_at(captured) {
                    base_report(
                        "auto",
                        None,
                        format!(
                            "the captured guard pointer {captured:#x} already resolves inside the \
                             snapshot, so auto changed NOTHING - this run uses the client's own \
                             bytes."
                        ),
                    )
                } else {
                    base_report("auto", Some(self.canary), String::new())
                }
            }
        }
    }

    pub fn uc(&mut self) -> &mut Unicorn<'static, ()> {
        &mut self.uc
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snap
    }

    pub fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        uce(self.uc.mem_read_as_vec(addr, len))
    }
    pub fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<()> {
        uce(self.uc.mem_write(addr, bytes))
    }
    pub fn u32(&self, addr: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(addr, 4)?.try_into().unwrap()))
    }
    pub fn u64(&self, addr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(addr, 8)?.try_into().unwrap()))
    }

    /// Run one call. The machine is left with memory rolled back when `spec.rollback` is set, so
    /// candidate N+1 sees exactly the state candidate N saw.
    pub fn run(&mut self, spec: &RunSpec) -> Result<RunReport> {
        // Fresh scaffolding every call: a stale scratch byte from the previous candidate is
        // indistinguishable from a real result.
        let zeros = vec![0u8; (self.scratch_len + self.stack_len) as usize];
        uce(self.uc.mem_write(self.scratch, &zeros))?;
        // …and a fresh canary. It lives outside the snapshot, so `rollback` cannot restore it; if a
        // previous run wrote through it, candidate N+1 would inherit that and the epilogue compare
        // would fail for a reason that has nothing to do with the candidate.
        uce(self.uc.mem_write(self.canary, &CANARY_MAGIC.to_le_bytes()))?;

        {
            let mut s = self.sh.borrow_mut();
            s.steps = 0;
            s.exit = None;
            s.trace.clear();
            s.trace_truncated = false;
            s.trace_budget = spec.trace.unwrap_or(0);
            s.step_limit = spec.step_limit;
            s.probes = spec.probes.iter().copied().collect();
            s.probe_hits.clear();
            s.probe_counts.clear();
            s.stubs = spec.stubs.iter().copied().collect();
            s.dirty.clear();
            s.watch_events.clear();
            s.watch_limit = spec.watch_limit;
            s.notes.clear();
        }

        // Watchpoints are attached per-run, and they are pure observation: they never change
        // control flow, so a second mem hook here is safe (unlike a second *dispatching* code hook).
        let mut hooks = Vec::new();
        for (lo, len) in &spec.watch_reads {
            let h = self.sh.clone();
            hooks.push(uce(self.uc.add_mem_hook(
                HookType::MEM_READ,
                *lo,
                lo + len - 1,
                move |uc, t, addr, size, v| {
                    let rip = uc.pc_read().unwrap_or(0);
                    let mut s = h.borrow_mut();
                    if s.watch_events.len() < s.watch_limit {
                        s.watch_events.push(MemEvent { rip, kind: format!("{t:?}"), addr, size, value: v });
                    }
                    true
                },
            ))?);
        }
        for (lo, len) in &spec.watch_writes {
            let h = self.sh.clone();
            hooks.push(uce(self.uc.add_mem_hook(
                HookType::MEM_WRITE,
                *lo,
                lo + len - 1,
                move |uc, t, addr, size, v| {
                    let rip = uc.pc_read().unwrap_or(0);
                    let mut s = h.borrow_mut();
                    if s.watch_events.len() < s.watch_limit {
                        s.watch_events.push(MemEvent { rip, kind: format!("{t:?}"), addr, size, value: v });
                    }
                    true
                },
            ))?);
        }

        // ⭐ The stack-canary redirect, applied BEFORE `spec.writes` so an explicit
        // `--set-mem v:0x1a06c78=…` still overrides it - and journalled through exactly the same
        // `dirty` list as every other write, so `rollback` restores the client's own pointer and
        // `verify_pristine` still has something to catch us with.
        let mut canary = self.canary_plan(spec.canary);
        if let Some(p) = canary.applied {
            uce(self.uc.mem_write(self.guard_ptr, &p.to_le_bytes()))?;
            self.sh.borrow_mut().dirty.push((self.guard_ptr, 8));
        }

        for (addr, bytes) in &spec.writes {
            uce(self.uc.mem_write(*addr, bytes))?;
            self.sh.borrow_mut().dirty.push((*addr, bytes.len()));
        }

        // ⭐ READ IT BACK. Until this line every field of `canary` is a PLAN, and a report that
        // states a plan in the past tense is how a FAIL row comes to read like a success. Read
        // AFTER `spec.writes` so an explicit `--set-mem` on the guard global is reflected too:
        // this is the pointer value execution is about to start with, whoever wrote it.
        canary.effective = self.uc.mem_read_as_vec(self.guard_ptr, 8).ok().map(|b| {
            u64::from_le_bytes(b.try_into().expect("mem_read_as_vec(8) returns 8 bytes"))
        });
        if let Some(e) = canary.effective {
            canary.resolves = self.uc.mem_read_as_vec(e, 8).is_ok();
        }
        if canary.applied.is_some() && !canary.redirect_landed() {
            self.sh.borrow_mut().notes.push(format!(
                "⛔ the stack-canary redirect to {:?} DID NOT LAND - memory holds {:?}",
                canary.applied, canary.effective
            ));
        }

        // ── ⭐⭐ THE ENTRY REGISTERS, and where they came from ────────────────────────────────────
        //
        // Decided by ONE function, then applied here in the order the report states:
        //
        //   zero everything  ->  seed from the capture (if asked)  ->  --arg  ->  --reg  ->  frame
        //
        // ⭐ The zeroing is not cosmetic. Until it existed `run()` never reset the register file, so
        // candidate N+1 in a search started from candidate N's leftovers - the register-side twin of
        // the memory inheritance `rollback`/`verify_pristine` were built to catch, and it had no
        // control at all. "Invented" now names a DEFINED state.
        if spec.use_captured && spec.captured.is_none() {
            bail!(
                "--use-captured-regs was asked for but this run has no captured register file. \
                 Refusing: a flag whose entire purpose is to stop inventing registers must not \
                 silently invent them. (A snapshot with `trap: none` was NOT ATTEMPTED, which is \
                 not the same as \"the client had no registers\" - see `d1-replay info`.)"
            );
        }
        let mut origin = plan_register_origin(spec, self.base);
        for (name, r) in REGS {
            if *name == "rip" {
                continue; // emu_start owns it
            }
            uce(self.uc.reg_write(
                *r,
                if *name == "eflags" { EFLAGS_INVENTED } else { 0 },
            ))?;
        }
        for (name, v) in &origin.seeded {
            let r = reg_by_name(name)
                .ok_or_else(|| anyhow!("plan_register_origin produced unknown register {name:?}"))?;
            uce(self.uc.reg_write(r, *v))?;
        }

        // Frame: return address = the trampoline, 16-byte aligned below it, red zone left clear.
        let rsp = self.stack + self.stack_len - 0x1008;
        uce(self.uc.mem_write(rsp, &self.stop.to_le_bytes()))?;
        for (i, a) in spec.args.iter().take(6).enumerate() {
            uce(self.uc.reg_write(ARG_REGS[i], *a))?;
        }
        for (i, a) in spec.args.iter().skip(6).enumerate() {
            uce(self.uc.mem_write(rsp + 8 + i as u64 * 8, &a.to_le_bytes()))?;
        }
        for (r, v) in &spec.regs {
            uce(self.uc.reg_write(*r, *v))?;
        }
        // ⚠ `RSP`/`RBP` are the frame, so they are written LAST - except when `--reg` named them.
        // Before this they were written unconditionally, which made `--reg rsp=…` a silent no-op:
        // the flag was accepted, the value was discarded, and nothing said so. Now the choice is
        // honoured and `RegisterOrigin::render` warns that the trampoline is no longer under it.
        origin.frame_rsp = Some(rsp);
        if origin.synthetic_frame {
            uce(self.uc.reg_write(RegisterX86::RSP, rsp))?;
            uce(self.uc.reg_write(RegisterX86::RBP, 0))?;
        }

        let run = self.uc.emu_start(spec.entry, self.stop, 0, 0);

        let (mut exit, steps, trace, trace_truncated, probes, probe_counts, watch_events, notes) = {
            let s = self.sh.borrow();
            (
                s.exit.clone(),
                s.steps,
                s.trace.clone(),
                s.trace_truncated,
                s.probe_hits.clone(),
                s.probe_counts.clone(),
                s.watch_events.clone(),
                s.notes.clone(),
            )
        };
        if exit.is_none() {
            let rip = self.uc.pc_read().unwrap_or(0);
            exit = Some(match run {
                Ok(()) if rip == self.stop => Exit::Returned,
                Ok(()) => Exit::Stopped { rip, reason: "emulation ended off the trampoline".into() },
                Err(e) => Exit::Stopped { rip, reason: format!("{e:?}") },
            });
        }
        let exit = exit.unwrap();
        let rax = self.uc.reg_read(RegisterX86::RAX).unwrap_or(0);
        let (al, eax) = (rax as u8, rax as u32);
        let final_regs = regdump(&mut self.uc);

        for h in hooks {
            let _ = self.uc.remove_hook(h);
        }

        if spec.rollback {
            self.rollback()?;
        }

        Ok(RunReport {
            entry: spec.entry,
            args: spec.args.clone(),
            exit,
            rax,
            al,
            eax,
            steps,
            final_regs,
            trace,
            trace_truncated,
            probes,
            probe_counts,
            watch_events,
            canary,
            regs_origin: origin,
            trap: TrapProvenance::of(self.snap.manifest.trap.as_ref()),
            notes,
        })
    }

    /// Restore every byte written during the last run from the snapshot.
    ///
    /// Returns the number of bytes restored. Ranges outside the snapshot (scratch, stack) are not
    /// restored here - `run()` re-zeroes them wholesale before every call.
    pub fn rollback(&mut self) -> Result<usize> {
        let dirty = std::mem::take(&mut self.sh.borrow_mut().dirty);
        let mut restored = 0usize;
        for (addr, size) in dirty {
            if let Ok(orig) = self.snap.read(addr, size) {
                uce(self.uc.mem_write(addr, &orig))?;
                restored += size;
            }
        }
        Ok(restored)
    }

    /// Mandatory control on the rollback machinery: every captured region must byte-match the
    /// snapshot. Without this, a rollback that silently did nothing would make candidate *N+1*
    /// inherit candidate *N*'s writes - and the search would report a confident wrong answer.
    pub fn verify_pristine(&self) -> Result<(bool, String)> {
        let mut checked = 0u64;
        for (addr, blob) in self.snap.regions() {
            // Sample: the head, the tail and a stride through the middle - a full compare of 48 MiB
            // per candidate would dominate the run, and a targeted sample catches the writes we
            // actually make (they are all in small structures).
            let probes: Vec<(u64, usize)> = if blob.len() <= 0x4000 {
                vec![(addr, blob.len())]
            } else {
                let mut v = vec![(addr, 0x1000), (addr + blob.len() as u64 - 0x1000, 0x1000)];
                let mut o = 0x1000u64;
                while o + 0x1000 < blob.len() as u64 {
                    v.push((addr + o, 0x1000));
                    o += 0x40_000;
                }
                v
            };
            for (a, n) in probes {
                let live = uce(self.uc.mem_read_as_vec(a, n))?;
                let want = &blob[(a - addr) as usize..(a - addr) as usize + n];
                checked += n as u64;
                if live != want {
                    let off = live.iter().zip(want).position(|(x, y)| x != y).unwrap_or(0);
                    return Ok((
                        false,
                        format!(
                            "memory differs from the snapshot at {:#x} (live {:#04x} vs snapshot \
                             {:#04x}) - rollback did NOT restore it",
                            a + off as u64,
                            live[off],
                            want[off]
                        ),
                    ));
                }
            }
        }
        Ok((true, format!("{checked} sampled bytes match the snapshot after rollback")))
    }
}

/// Find `need` bytes of address space that collide with nothing captured.
fn find_hole(mapped: &[(u64, u64)], need: u64) -> Result<u64> {
    // Start well above anything shadPS4 reserves, then walk up past every captured region.
    let mut cand = 0x0000_7000_0000_0000u64;
    for _ in 0..64 {
        let clash = mapped.iter().find(|(a, l)| *a < cand + need && cand < a + l);
        match clash {
            None => return Ok(cand),
            Some((a, l)) => cand = (a + l).div_ceil(PAGE) * PAGE,
        }
    }
    bail!("could not find {need} bytes of free address space for the replay stack")
}

fn disasm_one(bytes: &[u8], ip: u64) -> String {
    use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter};
    if bytes.is_empty() {
        return "<unreadable>".into();
    }
    let mut d = Decoder::with_ip(64, bytes, ip, DecoderOptions::NONE);
    let mut insn = Instruction::default();
    d.decode_out(&mut insn);
    let mut out = String::new();
    let mut f = IntelFormatter::new();
    f.format(&insn, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_BASE: u64 = 0x8_0000_0000;

    fn cap() -> CapturedRegs {
        let mut regs = BTreeMap::new();
        for (n, v) in [
            ("rax", 0x11u64),
            ("rbx", 0x22),
            ("rcx", 0x33),
            ("rdx", 0x44),
            ("rsi", 0x55),
            ("rdi", 0x66),
            ("rbp", 0x7fff_0000_0100),
            ("rsp", 0x7fff_0000_00c0),
            ("r8", 0x88),
            ("r9", 0x99),
            ("r10", 0xaa),
            ("r11", 0xbb),
            ("r12", 0xcc),
            ("r13", 0xdd),
            ("r14", 0xee),
            ("r15", 0xff),
            ("rip", TEST_BASE + 0x2a_8572),
            ("eflags", 0x1_0346),
        ] {
            regs.insert(n.to_string(), v);
        }
        CapturedRegs { label: "TEST".into(), regs }
    }

    fn spec(use_captured: bool) -> RunSpec {
        let mut s = RunSpec::new(TEST_BASE + 0x1000);
        s.captured = Some(cap());
        s.use_captured = use_captured;
        s
    }

    /// ⭐ The claim the whole flag rests on: `--use-captured-regs` seeds the GPRs and NOT the three
    /// registers a replay owns. If `rip` ever appeared in `seeded` the run would still start at
    /// `--func` and the report would be lying about it.
    #[test]
    fn captured_mode_seeds_the_gprs_and_never_rip_rsp_or_rbp() {
        let o = plan_register_origin(&spec(true), TEST_BASE);
        assert!(o.is_captured());
        let names: Vec<&str> = o.seeded.iter().map(|(n, _)| n.as_str()).collect();
        for n in ["rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "eflags"] {
            assert!(names.contains(&n), "{n} should be seeded, got {names:?}");
        }
        for n in ["rip", "rsp", "rbp"] {
            assert!(!names.contains(&n), "{n} must NOT be seeded");
            assert!(o.not_seeded.iter().any(|(x, _)| x == n), "{n} must be listed with a reason");
        }
        // Every not-seeded entry carries a reason. A blank reason is how "not seeded" turns into
        // "seeded, we think" in a reader's head.
        assert!(o.not_seeded.iter().all(|(_, why)| why.len() > 20));
    }

    /// ⚠ `TF`/`RF` must not cross into the emulator, and the masking must be VISIBLE. A mask nobody
    /// is told about is a divergence from the client's state hiding inside "use the client's state".
    #[test]
    fn eflags_is_masked_and_the_mask_is_reported() {
        let o = plan_register_origin(&spec(true), TEST_BASE);
        let e = o.seeded.iter().find(|(n, _)| n == "eflags").expect("eflags seeded").1;
        // 0x1_0346 carries BOTH: RF (0x1_0000) and TF (0x100). Both go.
        assert_eq!(e, 0x246, "RF (0x10000) and TF (0x100) must both be cleared");
        assert_eq!(e & EFLAGS_UNSAFE, 0);
        assert!(o.notes.iter().any(|n| n.contains("eflags")), "the mask must be reported: {:?}", o.notes);
    }

    /// Precedence: explicit flags beat the capture, per register, and the report says which.
    #[test]
    fn explicit_flags_override_the_capture_and_are_named() {
        let mut s = spec(true);
        s.args = vec![0x1234]; // rdi
        s.regs = vec![(RegisterX86::R13, 0x9999)];
        let o = plan_register_origin(&s, TEST_BASE);
        let seeded: Vec<&str> = o.seeded.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!seeded.contains(&"rdi") && !seeded.contains(&"r13"));
        let over: BTreeMap<&str, &str> =
            o.overridden.iter().map(|(n, b)| (n.as_str(), b.as_str())).collect();
        assert!(over["rdi"].contains("--arg 1"), "{:?}", over);
        assert!(over["r13"].contains("--reg r13=0x9999"), "{:?}", over);
    }

    /// ⭐ THE DISTINGUISHABILITY AXIS, at the text level - the same assertion `controls`'
    /// `captured-vs-invented-registers` makes about a live run, made here about the rendering alone
    /// so it fails in `cargo test` too.
    #[test]
    fn the_two_modes_render_differently_and_each_names_itself() {
        let inv = plan_register_origin(&spec(false), TEST_BASE).render("");
        let capd = plan_register_origin(&spec(true), TEST_BASE).render("");
        assert_ne!(inv, capd);
        assert!(capd.contains("CAPTURED") && !capd.contains("INVENTED"));
        assert!(inv.contains("INVENTED") && !inv.contains("⭐ CAPTURED"));
        // ⭐ An invented run against a trap-paired snapshot must name the road not taken, or the
        // reader never learns the better artifact was in their hands.
        assert!(inv.contains("--use-captured-regs"), "{inv}");
        assert!(inv.contains("DOES carry a trap"), "{inv}");
    }

    /// A snapshot with no trap: invented mode must say NOT ATTEMPTED, never "the client had none".
    #[test]
    fn without_a_trap_the_invented_block_says_not_attempted() {
        let mut s = RunSpec::new(TEST_BASE + 0x1000);
        s.use_captured = false;
        let r = plan_register_origin(&s, TEST_BASE).render("");
        assert!(r.contains("NOT ATTEMPTED"), "{r}");
        assert!(!r.contains("DOES carry a trap"));
    }

    /// ⚠ A `--vary reg:` knob is applied per candidate, inside `search::search`, so the planner
    /// cannot see it. Without `claimed_by` the SEARCH PREVIEW printed the client's captured value
    /// for a register every candidate was about to overwrite - a preview that disagrees with the
    /// action, which is the exact defect `Machine::canary_plan` was shaped to avoid.
    #[test]
    fn a_search_knob_takes_a_register_out_of_the_seeded_list() {
        let mut o = plan_register_origin(&spec(true), TEST_BASE);
        assert!(o.seeded.iter().any(|(n, _)| n == "r13"));
        o.claimed_by("R13", "--vary reg:r13 (2 candidate value(s))");
        assert!(!o.seeded.iter().any(|(n, _)| n == "r13"), "r13 must leave `seeded`");
        let row = o.overridden.iter().find(|(n, _)| n == "r13").expect("listed as overridden");
        assert!(row.1.contains("--vary") && row.1.contains("0xdd"), "{row:?}");
        // Idempotent, and safe on a register that was never seeded.
        o.claimed_by("rsp", "--vary reg:rsp");
        assert_eq!(o.overridden.iter().filter(|(n, _)| n == "rsp").count(), 1);
    }

    /// `--reg rsp=…` used to be a silent no-op: accepted, then overwritten by the frame. Now the
    /// plan records that the frame is NOT synthetic, and the rendering warns what that costs.
    #[test]
    fn an_explicit_rsp_takes_the_frame_and_says_what_that_means() {
        let mut s = spec(false);
        assert!(plan_register_origin(&s, TEST_BASE).synthetic_frame);
        s.regs = vec![(RegisterX86::RSP, 0x7fff_0000_00c0)];
        let mut o = plan_register_origin(&s, TEST_BASE);
        assert!(!o.synthetic_frame);
        o.frame_rsp = Some(0x7000_0004_eff8);
        let r = o.render("");
        assert!(r.contains("RSP SET BY --reg"), "{r}");
        assert!(r.contains("will not \nstop by returning") || r.contains("stop by returning"), "{r}");
    }

    fn rep(captured: Option<u64>, applied: Option<u64>, effective: Option<u64>) -> CanaryReport {
        CanaryReport {
            mode: "auto".into(),
            guard_ptr: 0x8_01A0_6C78,
            guard_ptr_vaddr: GUARD_PTR_VADDR,
            captured,
            captured_mapped: false,
            applied,
            effective,
            resolves: true,
            note: String::new(),
        }
    }

    #[test]
    fn stack_canary_parse_covers_every_spelling() {
        let never = |_: &str| -> Result<u64> { panic!("resolver must not run for a keyword") };
        for s in ["auto", "AUTO", "  Auto  "] {
            assert_eq!(StackCanary::parse(s, &never).unwrap(), StackCanary::Auto, "{s:?}");
        }
        for s in ["off", "none", "NO", " Off "] {
            assert_eq!(StackCanary::parse(s, &never).unwrap(), StackCanary::Off, "{s:?}");
        }
        // Anything else is handed to the caller's resolver verbatim (already trimmed), so the
        // address is printed in the run's RESOLUTIONS block like every other one.
        let seen = std::cell::RefCell::new(String::new());
        let r = |e: &str| -> Result<u64> {
            *seen.borrow_mut() = e.to_string();
            Ok(0x1234)
        };
        assert_eq!(StackCanary::parse(" v:0x1a06c78 ", &r).unwrap(), StackCanary::At(0x1234));
        assert_eq!(&*seen.borrow(), "v:0x1a06c78", "the resolver gets the trimmed expression");
        // A bad expression must surface as an error, not as a silent `Off`.
        assert!(StackCanary::parse("nonsense", &|_| bail!("no")).is_err());
    }

    /// ⭐ The distinction this crate paid for on 2026-08-10: `applied` is a PLAN and `effective` is
    /// an OBSERVATION, and only the second can answer "did the redirect happen?".
    #[test]
    fn altered_and_landed_read_the_observation_not_the_plan() {
        let host_tls = 0x55ed_6ed6_3c80u64;
        let ours = 0x7000_0005_1000u64;

        // Planned and landed.
        let r = rep(Some(host_tls), Some(ours), Some(ours));
        assert!(r.altered() && r.redirect_landed());

        // Planned, ran, and the write did NOT land - the INV1 shape. A `redirected` term reading
        // `applied.is_some()` returns true here, which is exactly the decoration that shipped.
        let r = rep(Some(host_tls), Some(ours), Some(host_tls));
        assert!(!r.altered(), "memory still holds the client's bytes");
        assert!(!r.redirect_landed(), "the plan is not the action");
        assert!(r.applied.is_some(), "…and the plan alone would have said yes");

        // `off`, but a previous un-rolled-back run left a redirect in place. The plan says
        // "unaltered"; the read-back tells the truth.
        let mut r = rep(Some(host_tls), None, Some(ours));
        r.mode = "off".into();
        assert!(r.altered() && !r.redirect_landed());

        // A plan that has not run yet observes nothing and must not claim anything.
        let r = rep(Some(host_tls), Some(ours), None);
        assert!(!r.altered() && !r.redirect_landed());
    }

    /// The canary slot must be its own page: not inside scratch, not inside the stack, not the
    /// trampoline. `Machine::new` lays them out `[scratch][stack][trampoline][canary]`, which this
    /// re-derives from the same constants so a layout change that overlaps them fails here too.
    #[test]
    fn canary_slot_is_disjoint_from_scratch_stack_and_trampoline() {
        let (scratch_len, stack_len) = (0x10_000u64, 0x40_000u64);
        let hole = 0x7000_0000_0000u64;
        let (scratch, stack) = (hole, hole + scratch_len);
        let stop = hole + scratch_len + stack_len;
        let canary = stop + PAGE;
        assert!(canary >= stack + stack_len, "canary is past the stack");
        assert!(!(canary >= scratch && canary < stack + stack_len), "canary is outside scratch+stack");
        assert_ne!(canary / PAGE, stop / PAGE, "canary does not share the trampoline's page");
        assert!(canary + 8 <= hole + scratch_len + stack_len + 2 * PAGE, "canary is inside the mapping");
    }
}
