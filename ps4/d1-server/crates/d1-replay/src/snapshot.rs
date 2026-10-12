// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The snapshot: the live client's memory, made portable and self-describing.
//!
//! ```text
//! offset 0   "D1SNAP\x01\x00"          8 bytes, magic + format version
//! offset 8   u32 LE manifest_len
//! offset 12  manifest, JSON            provenance + region table + every skip, with its reason
//! then       region payloads, concatenated, each at its recorded absolute `file_off`
//! ```
//!
//! Everything a later reader needs to decide whether to trust a result is in the manifest: the
//! eboot's SHA-256, the recovered base **and the three controls that justified it**, the liveness
//! level with its ladder, the run directory and the client state reached, the selection policy, the
//! region table and - the part people skip - **the list of mappings that were NOT captured, and
//! why**. What you did not capture is part of the evidence.
//!
//! ⚠ **`/proc/<pid>/mem` is memory, and memory has no registers.** That has not changed.
//!
//! ⭐ **CORRECTED 2026-08-10 (in place, per CLAUDE.md):** this file used to end *"there are no
//! registers in here, and there cannot be."* The second half was wrong. A snapshot taken **at a
//! hardware-watchpoint trap** can carry the trapping thread's integer register file, because
//! `d1-live` already has the thread stopped and has already called `PTRACE_GETREGS` - the registers
//! come from ptrace, not from `/proc/<pid>/mem`. That is [`Manifest::trap`], and
//! `historicaldocs/design/live-watch.md` §6a is its spec.
//!
//! ⚠⚠ Read [`Coherence`] before quoting a trap-paired snapshot as "the state at the moment of the
//! write". The registers are **one thread at one instant**; the memory is a smear over the
//! milliseconds that follow, during which the other ~110 threads kept running. The artifact records
//! that, in numbers, so nobody has to take it on trust - and `trap: None` means **NOT ATTEMPTED**,
//! never "the client had no registers".

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::live::{BaseControls, Liveness, Proc};

pub const MAGIC: &[u8; 8] = b"D1SNAP\x01\x00";
pub const FORMAT: &str = "d1-replay-snapshot/1";
pub const PAGE: u64 = 0x1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EbootProvenance {
    pub path: String,
    pub sha256: String,
    pub len: u64,
    /// PT_LOAD spans, as `[vaddr, vaddr+memsz)` - the window the replay maps.
    pub load_segments: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcInfo {
    pub pid: i32,
    pub starttime: u64,
    pub cmdline: String,
    pub emulator_exe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientState {
    /// Last `bootflow:` state seen in the run log, if a run directory was given.
    pub bootflow: Option<String>,
    /// Last ui substage line, if any.
    pub ui_substage: Option<String>,
    /// Free-text label the operator supplied (`--state`).
    pub label: Option<String>,
    pub run_dir: Option<String>,
    pub log_tail: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub eboot_window: bool,
    /// Named presets applied (`--preset content`). ⚠ `serde(default)` on purpose: snapshots taken
    /// before presets existed must still load, and an absent field must read as "no preset", never
    /// as a parse error.
    #[serde(default)]
    pub presets: Vec<String>,
    /// Bytes taken at each object a preset resolved - the bound that bites first.
    #[serde(default)]
    pub preset_object_window: u64,
    #[serde(default)]
    pub preset_arena_cap: u64,
    pub explicit_regions: Vec<String>,
    pub whole_mappings: Vec<String>,
    pub follows: Vec<String>,
    pub follow_depth: u32,
    pub follow_len: u64,
    pub follow_max_regions: usize,
    pub all_resident: bool,
    pub max_mapping_bytes: u64,
    pub max_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub addr: u64,
    pub len: u64,
    pub perms: String,
    /// Absolute offset of this region's bytes within the snapshot file.
    pub file_off: u64,
    /// Why this range is in the snapshot - "eboot-window seg0", "explicit", "mapping",
    /// "follow depth 1 from 0x…".
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skip {
    pub addr: u64,
    pub len: u64,
    pub perms: String,
    pub rss: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub tool: String,
    pub created_unix: u64,
    pub created_utc: String,
    pub eboot: EbootProvenance,
    pub process: ProcInfo,
    /// Host address at which eboot vaddr 0 is mapped. Guest pointers in shadPS4 are usable as host
    /// addresses, so this is the only translation the replay needs.
    pub base: u64,
    pub base_controls: BaseControls,
    pub liveness: Liveness,
    pub client_state: ClientState,
    pub selection: Selection,
    pub regions: Vec<Region>,
    pub skipped: Vec<Skip>,
    pub total_bytes: u64,
    /// Free-text reach tag, carried forward so a finding cannot lose its provenance.
    pub reach: String,

    /// ⭐ Present ONLY when this snapshot was taken at a hardware-watchpoint trap
    /// (`d1-replay capture --snapshot-on-hit`, `historicaldocs/design/live-watch.md` §6a).
    ///
    /// ⚠ `None` means **NOT ATTEMPTED**. It never means "the client had no registers", and it never
    /// means "we tried and could not" - that is [`TrapCapture::threads_failed`]. The distinction is
    /// the same one [`Snapshot::read`] enforces for bytes: *not captured* and *was zero* are
    /// different facts and only one of them is evidence.
    ///
    /// `serde(default)` states the intent, exactly as [`Selection::presets`] does: every snapshot
    /// already in `runs/snapshots/` predates this field and must still load, and an absent field
    /// must read as "no trap", never as a parse error.
    ///
    /// ⚠ **MEASURED 2026-08-10, and it matters for anyone auditing this:** removing that attribute
    /// changes NOTHING. `serde` already routes a missing `Option<T>` field through `missing_field`,
    /// which yields `None` - verified by deleting the attribute, rebuilding and watching all 18
    /// archived snapshots still parse. So **the attribute is documentation, not mechanism**, and the
    /// thing that actually protects the 4.3 GiB archive is the `existing-snapshots-still-load`
    /// control. Its inversion had to make the field genuinely non-`Option` before a single snapshot
    /// failed to load - and then all 18 did, at once. Do not mistake the annotation for the guard.
    #[serde(default)]
    pub trap: Option<TrapCapture>,

    pub notes: Vec<String>,
}

/// The trap this snapshot is paired with: the registers the client actually had, and a **measured**
/// account of how far the memory image is from that instant.
///
/// ⛔ Filled in **only** by [`capture_at_trap`], which is reached only from
/// `d1-replay capture --snapshot-on-hit`; the plain [`capture`] path hard-codes `None`.
/// ⚠ **No trap-paired snapshot has ever been taken from shadPS4.** Every file in `runs/snapshots/`
/// has no `trap` key at all, and `None` means NOT ATTEMPTED - never "the client had none".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapCapture {
    /// How the thread came to be stopped, in words:
    /// `"d1-live DR0 write 8 bytes at 0x8026a34e8 (v:0x26a34e8)"`.
    pub method: String,
    pub watch_addr: u64,
    pub watch_addr_vaddr: Option<u64>,
    pub watch_len: u64,
    /// `"write"` | `"rw"` | `"exec"`.
    pub watch_rw: String,
    pub slot: usize,

    /// ⭐ The `d1_live::watch::Hit` **verbatim** - the raw `RIP`, the backward-decoded `writer`,
    /// every candidate, the confidence label, `value_after`, the whole register map. Carried as a
    /// `serde_json::Value` so this pairing NEVER re-describes what `d1-live` already describes: one
    /// producer, one schema (the no-format-twice non-negotiable). A reader who wants the writer
    /// reads `hit.writer` - and gets `d1-live`'s own caveats with it, including that a data
    /// breakpoint is a TRAP and `rip` is the instruction AFTER the store.
    pub hit: serde_json::Value,

    /// The thread that trapped. ⚠ The ONLY thread this artifact can speak for.
    pub trapped: ThreadRegs,
    /// Other threads whose registers were also taken. Empty by default, and expected to stay empty:
    /// stopping the rest re-enters the process-wide `waitpid` ownership `Watcher::run` holds.
    #[serde(default)]
    pub also_stopped: Vec<ThreadRegs>,
    /// ⚠⚠ Threads that KEPT RUNNING throughout the capture. The approximation, as a number, in the
    /// file. On shadPS4 this is normally ~110.
    ///
    /// ⛔ `None` means the `/proc/<pid>/task` census FAILED, which is **not zero**. Build 2 used
    /// `unwrap_or(0)` here, so a failed census rendered as *"0 thread(s) of 0 KEPT RUNNING during
    /// the capture"* - a measurement failure printed as the atomic-freeze claim, in the one artifact
    /// whose entire job is to stop that claim being made.
    #[serde(default)]
    pub threads_running: Option<usize>,
    /// `/proc/<pid>/task` counted **inside the stop, after the disarm** - the same instant
    /// `threads_running` describes. ⚠ Build 2 counted this at wait-loop entry, up to
    /// `--hit-timeout` (120 s) before the trap, and then described it as the capture's instant.
    #[serde(default)]
    pub threads_total: Option<usize>,
    /// The same count taken when the watchpoint was armed. Kept so the drift between arming and
    /// trapping is visible rather than hidden inside one number.
    #[serde(default)]
    pub threads_total_at_arm: Option<usize>,
    /// ⭐ Threads whose debug register was **verified armed**. A snapshot from a run that armed 3 of
    /// 47 threads was byte-identical to one that armed all 47 until this field existed - the exact
    /// false-negative shape `WatchReport::summary` exists to prevent, reproduced inside the file.
    #[serde(default)]
    pub threads_armed: Option<usize>,
    /// Threads we tried to stop and could not, each named with its failure - **and** threads whose
    /// arm failed, which build 2 printed to stdout and never wrote to the file at all. An empty
    /// `also_stopped` **and** an empty `threads_failed` means we never asked; the two together are
    /// what separate "did not attempt" from "attempted and could not".
    #[serde(default)]
    pub threads_failed: Vec<String>,
    /// ⭐ What the pre-capture disarm proved, verbatim from `d1_live::watch::DisarmProof`: the
    /// per-thread `DR7` read-back, its wall-clock bound, whether that bound fired, and any thread
    /// left tracked. A capture only exists at all if this was `all_clear()`.
    #[serde(default)]
    pub disarm: Option<serde_json::Value>,

    pub budget: StopBudget,
    pub coherence: Coherence,
}

/// The x86-64 integer register file of one thread, at one instant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRegs {
    pub tid: i32,
    pub comm: String,
    /// Nanoseconds after [`Coherence::trap_unix_nanos`] at which THIS thread was stopped and read.
    /// ⚠ N threads interrupted serially are N instants, not one - so each carries its own stamp and
    /// nobody can read `also_stopped` as a simultaneous set.
    pub stopped_at_ns: u64,
    /// Lowercase register name → value: the whole `PTRACE_GETREGS` set (`rax`..`r15`, `rip`, `rsp`,
    /// `rbp`, `eflags`, `orig_rax`, `cs`/`ss`/`ds`/`es`/`fs`/`gs`) plus `fs_base`/`gs_base`.
    ///
    /// ⚠ A `BTreeMap`, not a mirrored `libc::user_regs_struct`: a C struct layout is not a wire
    /// format. And [`crate::machine::RegDump`] is **already exactly this shape**, with
    /// [`crate::machine::reg_by_name`] already mapping these names - so the replay side needs no new
    /// type, no conversion and no second rendering. `d1_live::ptrace::regs_map` is the one producer.
    ///
    /// ⛔ **No FP/SSE/AVX state.** `PTRACE_GETFPREGS`/`NT_X86_XSTATE` are not called, so a function
    /// whose arguments are floats, or whose behaviour depends on `xmm`, CANNOT be replayed from
    /// this. The hole is real and cheap to close later; until it is, the honest phrase is *"the
    /// integer register file"*, never *"the registers"*.
    pub regs: BTreeMap<String, u64>,
    /// `rip - base`, when `RIP` is inside the eboot window. Precomputed so no consumer re-derives
    /// it - and so nobody meets CLAUDE.md trap 1 (heap addresses are absolute; only eboot vaddrs get
    /// the base).
    pub rip_vaddr: Option<u64>,
}

/// The scoping decision, recorded so a reader sees what was deliberately left OUT of the stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopBudget {
    /// `"minimal"` | `"state"` | `"wide"`.
    pub scope: String,
    pub budget_bytes: u64,
    pub budget_ms: f64,
    pub hard_cap_bytes: u64,
    pub hard_cap_ms: f64,
    /// ⭐ What the PRE-ARM rehearsal measured, reading exactly these regions from exactly this
    /// client. The budget is enforced against a MEASURED rate, never an assumed one.
    ///
    /// ⚠ It rehearses the **fixed** set only. The register-root windows and the stack are resolved
    /// at the hit and are neither proven readable nor timed here, so `rehearsed_ms` is a floor for
    /// the realised in-stop cost, not a bound on it.
    pub rehearsed_bytes: u64,
    pub rehearsed_ms: f64,
}

/// ⚠⚠ How far this snapshot is from "the moment" - measured, not claimed.
///
/// ⛔ **This is not an atomic freeze.** One thread stops; the other ~110 keep running. What a
/// trap-paired snapshot guarantees is narrow and worth stating exactly: *the trapping thread
/// executed no instructions between its registers being read and the in-stop bytes being read.*
/// Nothing else. Do not write "the state at the moment of the write"; write "the trapping thread's
/// registers, paired with memory read over the following N ms while N other threads ran", and let
/// these fields supply the numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coherence {
    /// `CLOCK_REALTIME` at the instant the trap was observed. Every other stamp is nanoseconds
    /// AFTER it, so the whole window reads at a glance.
    pub trap_unix_nanos: u64,
    pub debug_registers_cleared_ns: u64,
    pub stop_read_start_ns: u64,
    pub stop_read_end_ns: u64,
    pub thread_resumed_ns: u64,
    pub resumed_read_end_ns: u64,
    /// TRUE only if EVERY byte in this file was read while the trapping thread was in its
    /// ptrace-stop. FALSE the moment one region is read after the resume - which is the default,
    /// because eboot seg0 is.
    pub memory_read_while_stopped: bool,
    /// ⛔ FALSE by default and expected to stay FALSE. TRUE requires all ~111 threads stopped for
    /// the whole read. **A reader who wants to say "atomic" has to find this field TRUE.**
    pub all_threads_stopped: bool,
    /// Start addresses of regions read AFTER the resume. Those bytes are a LATER instant, and naming
    /// them is cheaper than making every reader infer it from [`Region::origin`].
    #[serde(default)]
    pub regions_read_after_resume: Vec<u64>,
    pub measured_mib_per_s: f64,
    /// ⛔ TRUE if the in-stop deadline fired and regions were dropped. Every dropped region is ALSO
    /// in [`Manifest::skipped`] with that reason, and [`Manifest::notes`] carries a loud line.
    pub deadline_fired: bool,
    /// ⭐ The phase-split control (§6a.6). `None` = it did not run.
    #[serde(default)]
    pub phase_split: Option<PhaseSplitControl>,
}

/// ⭐⭐ The trap caveat in a form that travels - small, owned, serialisable, and renderable
/// **wherever a result derived from this snapshot is shown**.
///
/// ⛔ It exists because of a measured gap: [`Snapshot::summary`] printed the trap block, so `capture`
/// and `info` carried it, while `replay` - the most-used subcommand, and the one whose transcripts
/// get pasted into `design/` docs as evidence - printed the word "trap" **zero** times in either its
/// human or its `--json` rendering, because [`crate::machine::RunReport`] had no field for it. A
/// reader then met a transcript whose only caveat was *"a replay is YOUR call with YOUR register
/// values"* - which, under `--use-captured-regs`, is wrong in the reader's favour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapProvenance {
    pub method: String,
    pub tid: i32,
    pub comm: String,
    pub rip_vaddr: Option<u64>,
    pub threads_running: Option<usize>,
    pub threads_total: Option<usize>,
    pub threads_armed: Option<usize>,
    pub threads_failed: usize,
    pub all_threads_stopped: bool,
    pub memory_read_while_stopped: bool,
    pub regions_read_after_resume: usize,
    pub stop_read_ms: f64,
    pub resumed_read_ms: f64,
    pub deadline_fired: bool,
    pub phase_split: Option<String>,
}

impl TrapProvenance {
    pub fn of(t: Option<&TrapCapture>) -> Option<Self> {
        let t = t?;
        let c = &t.coherence;
        Some(Self {
            method: t.method.clone(),
            tid: t.trapped.tid,
            comm: t.trapped.comm.clone(),
            rip_vaddr: t.trapped.rip_vaddr,
            threads_running: t.threads_running,
            threads_total: t.threads_total,
            threads_armed: t.threads_armed,
            threads_failed: t.threads_failed.len(),
            all_threads_stopped: c.all_threads_stopped,
            memory_read_while_stopped: c.memory_read_while_stopped,
            regions_read_after_resume: c.regions_read_after_resume.len(),
            stop_read_ms: (c.stop_read_end_ns.saturating_sub(c.stop_read_start_ns)) as f64 / 1e6,
            resumed_read_ms: (c.resumed_read_end_ns.saturating_sub(c.thread_resumed_ns)) as f64 / 1e6,
            deadline_fired: c.deadline_fired,
            phase_split: c.phase_split.as_ref().map(|p| p.verdict.clone()),
        })
    }

    /// TRUE when the file's own numbers say the memory is a smear rather than one instant. ⭐ The
    /// warning below is conditional on THIS, not boilerplate: a genuinely atomic capture must not
    /// print a smear warning, or the warning stops carrying information.
    pub fn is_a_smear(&self) -> bool {
        !self.all_threads_stopped
            || !self.memory_read_while_stopped
            || self.regions_read_after_resume > 0
            || self.threads_running.map(|n| n > 0).unwrap_or(true)
    }
}

/// Render the trap caveat **in both states**. `head` prefixes the first line, `indent` the rest.
///
/// ⚠ A field that only appears when it is set teaches readers that its absence means "no". Here
/// absence means NOT ATTEMPTED, so the `None` arm says so in words.
pub fn render_trap_provenance(p: Option<&TrapProvenance>, head: &str, indent: &str) -> String {
    let Some(t) = p else {
        return format!(
            "{head}none - this snapshot carries NO trap, so nothing here is paired with the \
             client's own registers (NOT ATTEMPTED, which is not the same as 'the client had \
             none').\n"
        );
    };
    let n = |v: Option<usize>| v.map(|x| x.to_string()).unwrap_or_else(|| "UNKNOWN".into());
    let mut s = format!(
        "{head}⭐ TRAP-PAIRED - {} \n{indent}tid {} ({}){}\n",
        t.method,
        t.tid,
        t.comm,
        t.rip_vaddr.map(|v| format!(", rip v:{v:#x}")).unwrap_or_default()
    );
    s.push_str(&format!(
        "{indent}threads  {} of {} KEPT RUNNING during the capture; {} verified armed; {} failure(s)\n",
        n(t.threads_running),
        n(t.threads_total),
        n(t.threads_armed),
        t.threads_failed
    ));
    s.push_str(&format!(
        "{indent}window   {:.2} ms read IN the stop, {:.2} ms read AFTER the resume; \
         {} region(s) are the LATER instant; deadline_fired={}\n",
        t.stop_read_ms, t.resumed_read_ms, t.regions_read_after_resume, t.deadline_fired
    ));
    if let Some(v) = &t.phase_split {
        s.push_str(&format!("{indent}phase    {v}\n"));
    }
    if t.is_a_smear() {
        s.push_str(&format!(
            "{indent}⚠⚠ THE MEMORY IS A SMEAR. The registers are ONE thread at ONE instant; the \
             bytes are a range of instants after it while {} other thread(s) ran. Any result below \
             is \"this function, with the arguments the client passed, against memory read over the \
             following milliseconds\" - NOT \"the state at the moment of the write\".\n",
            n(t.threads_running)
        ));
    } else {
        s.push_str(&format!(
            "{indent}⭐ all_threads_stopped AND memory_read_while_stopped are both TRUE with 0 \
             regions read after the resume - this really is one instant. ⚠ Nothing in \
             runs/snapshots/ has ever been captured that way; check the numbers above yourself.\n"
        ));
    }
    s
}

/// Did the bytes we read AFTER the resume hold still, and did the bytes we read IN the stop move?
///
/// ⭐ Its own pair: a "nothing changed" reading is only evidence if the other half of the split
/// **did** change. Both counts are in the file so the verdict can be re-derived, not just believed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSplitControl {
    pub seg0_windows: usize,
    /// Must be 0. Non-zero means the phase split fabricated a mixed instant.
    pub seg0_changed: usize,
    pub seg1_windows: usize,
    /// Must be > 0, or the control never had an axis and reports DEGENERATE rather than a free pass.
    pub seg1_changed: usize,
    /// `"PASS"` | `"FAIL: …"` | `"DEGENERATE: …"`.
    pub verdict: String,
}

pub struct Snapshot {
    pub manifest: Manifest,
    /// Region payloads, keyed by start address, sorted.
    data: BTreeMap<u64, Vec<u8>>,
}

impl Snapshot {
    pub fn regions(&self) -> impl Iterator<Item = (u64, &Vec<u8>)> {
        self.data.iter().map(|(a, b)| (*a, b))
    }

    pub fn base(&self) -> u64 {
        self.manifest.base
    }

    /// Read `len` bytes at an absolute address.
    ///
    /// ⚠ **Never zero-fills.** A range that is not in the snapshot is an error, because "we did not
    /// capture it" and "it was zero" are different facts and only one of them is evidence.
    pub fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(len);
        let mut cur = addr;
        let end = addr
            .checked_add(len as u64)
            .ok_or_else(|| anyhow!("read at {addr:#x} len {len} overflows"))?;
        while cur < end {
            let Some((&rstart, blob)) = self.data.range(..=cur).next_back() else {
                bail!(
                    "{cur:#x} is not in this snapshot (no region at or below it). \
                     A missing range is NOT a zero - re-capture with `--region a:{cur:#x}:0x100` \
                     or widen the selection."
                );
            };
            let rend = rstart + blob.len() as u64;
            if cur >= rend {
                bail!(
                    "{cur:#x} is not in this snapshot (nearest region {rstart:#x}..{rend:#x}). \
                     A missing range is NOT a zero."
                );
            }
            let take = ((rend.min(end)) - cur) as usize;
            let off = (cur - rstart) as usize;
            out.extend_from_slice(&blob[off..off + take]);
            cur += take as u64;
        }
        Ok(out)
    }

    pub fn has(&self, addr: u64, len: usize) -> bool {
        self.read(addr, len).is_ok()
    }

    pub fn u8(&self, addr: u64) -> Result<u8> {
        Ok(self.read(addr, 1)?[0])
    }
    pub fn u16(&self, addr: u64) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read(addr, 2)?.try_into().unwrap()))
    }
    pub fn u32(&self, addr: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(addr, 4)?.try_into().unwrap()))
    }
    pub fn u64(&self, addr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(addr, 8)?.try_into().unwrap()))
    }

    // ── file I/O ─────────────────────────────────────────────────────────────────────────────────

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if bytes.len() < 12 || &bytes[..8] != MAGIC {
            bail!("{}: not a d1-replay snapshot (bad magic)", path.display());
        }
        let mlen = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if 12 + mlen > bytes.len() {
            bail!("{}: truncated manifest", path.display());
        }
        let manifest: Manifest = serde_json::from_slice(&bytes[12..12 + mlen])
            .with_context(|| format!("parsing the manifest of {}", path.display()))?;
        let mut data = BTreeMap::new();
        for r in &manifest.regions {
            let lo = r.file_off as usize;
            let hi = lo + r.len as usize;
            if hi > bytes.len() {
                bail!(
                    "{}: region {:#x} len {:#x} claims file bytes {lo}..{hi} but the file is {} bytes",
                    path.display(),
                    r.addr,
                    r.len,
                    bytes.len()
                );
            }
            data.insert(r.addr, bytes[lo..hi].to_vec());
        }
        Ok(Self { manifest, data })
    }

    /// Read **only** the header and the manifest - never the payloads.
    ///
    /// ⭐ Why this exists: `runs/snapshots/` is 4.3 GiB across 18 files, and the question
    /// *"does every existing snapshot still parse?"* is a question about the **manifest**. Loading
    /// 4.3 GiB of region payloads to answer it would make the backward-compatibility control too
    /// slow to run, and a control nobody runs is not a control.
    ///
    /// ⚠ It does NOT validate the region table against the file length - [`Snapshot::load`] does
    /// that. This answers the JSON-schema question and says so.
    pub fn load_manifest(path: impl AsRef<Path>) -> Result<Manifest> {
        use std::io::{Read, Seek, SeekFrom};
        let path = path.as_ref();
        let mut f = std::fs::File::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        let mut head = [0u8; 12];
        f.read_exact(&mut head)
            .with_context(|| format!("{}: too short to be a snapshot", path.display()))?;
        if &head[..8] != MAGIC {
            bail!("{}: not a d1-replay snapshot (bad magic)", path.display());
        }
        let mlen = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
        let mut buf = vec![0u8; mlen];
        f.seek(SeekFrom::Start(12))?;
        f.read_exact(&mut buf)
            .with_context(|| format!("{}: truncated manifest", path.display()))?;
        serde_json::from_slice(&buf)
            .with_context(|| format!("parsing the manifest of {}", path.display()))
    }

    /// The manifest's raw JSON, for questions about which keys are literally PRESENT.
    ///
    /// ⚠ `serde` cannot answer that: a defaulted field is indistinguishable from an explicit one
    /// once it is deserialised, so a control that wants to prove *"this file predates the field"*
    /// has to look at the bytes.
    pub fn load_manifest_json(path: impl AsRef<Path>) -> Result<serde_json::Value> {
        use std::io::{Read, Seek, SeekFrom};
        let path = path.as_ref();
        let mut f = std::fs::File::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        let mut head = [0u8; 12];
        f.read_exact(&mut head)
            .with_context(|| format!("{}: too short to be a snapshot", path.display()))?;
        if &head[..8] != MAGIC {
            bail!("{}: not a d1-replay snapshot (bad magic)", path.display());
        }
        let mlen = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
        let mut buf = vec![0u8; mlen];
        f.seek(SeekFrom::Start(12))?;
        f.read_exact(&mut buf)
            .with_context(|| format!("{}: truncated manifest", path.display()))?;
        serde_json::from_slice(&buf)
            .with_context(|| format!("parsing the manifest of {} as raw JSON", path.display()))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).ok();
        }
        let json = serde_json::to_vec_pretty(&self.manifest)?;
        let mut f = std::fs::File::create(path)
            .with_context(|| format!("creating {}", path.display()))?;
        f.write_all(MAGIC)?;
        f.write_all(&(json.len() as u32).to_le_bytes())?;
        f.write_all(&json)?;
        for r in &self.manifest.regions {
            let blob = self
                .data
                .get(&r.addr)
                .ok_or_else(|| anyhow!("region {:#x} listed but has no payload", r.addr))?;
            f.write_all(blob)?;
        }
        f.flush()?;
        Ok(std::fs::metadata(path)?.len())
    }

    pub fn summary(&self) -> String {
        let m = &self.manifest;
        let mut s = String::new();
        s.push_str(&format!("format        {}\n", m.format));
        s.push_str(&format!("tool          {}\n", m.tool));
        s.push_str(&format!("captured      {} (unix {})\n", m.created_utc, m.created_unix));
        s.push_str(&format!("eboot         {}\n", m.eboot.path));
        s.push_str(&format!("  sha256      {}\n", m.eboot.sha256));
        s.push_str(&format!("process       pid {} start {}\n", m.process.pid, m.process.starttime));
        s.push_str(&format!("  exe         {}\n", m.process.emulator_exe));
        s.push_str(&format!("base          {:#x}\n", m.base));
        s.push_str(&format!("liveness      L{}\n", m.liveness.level));
        s.push_str(&format!("reach         {}\n", m.reach));
        // ⚠ Printed in BOTH states, always. A field that only appears when it is set teaches readers
        // to assume its absence means "no", and here absence means NOT ATTEMPTED.
        // ⭐ ONE renderer, shared with `d1-replay replay` and `search` - the no-format-twice rule.
        // A caveat that exists in two spellings drifts, and the one that drifts is the one nobody is
        // reading when it matters.
        s.push_str(&render_trap_provenance(
            TrapProvenance::of(m.trap.as_ref()).as_ref(),
            "trap          ",
            "              ",
        ));
        if let Some(b) = &m.client_state.bootflow {
            s.push_str(&format!("bootflow      {b}\n"));
        }
        if let Some(u) = &m.client_state.ui_substage {
            s.push_str(&format!("ui substage   {u}\n"));
        }
        if let Some(l) = &m.client_state.label {
            s.push_str(&format!("label         {l}\n"));
        }
        if let Some(r) = &m.client_state.run_dir {
            s.push_str(&format!("run dir       {r}\n"));
        }
        s.push_str(&format!(
            "regions       {} covering {} bytes ({:.1} MiB)\n",
            m.regions.len(),
            m.total_bytes,
            m.total_bytes as f64 / (1u64 << 20) as f64
        ));
        for r in &m.regions {
            s.push_str(&format!(
                "  {:#014x}..{:#014x} {} {:>10}  {}\n",
                r.addr,
                r.addr + r.len,
                r.perms,
                r.len,
                r.origin
            ));
        }
        s.push_str(&format!("skipped       {} mapping(s)\n", m.skipped.len()));
        for k in m.skipped.iter().take(24) {
            s.push_str(&format!(
                "  {:#014x}..{:#014x} {} rss={:>10}  {}\n",
                k.addr,
                k.addr + k.len,
                k.perms,
                k.rss,
                k.reason
            ));
        }
        if m.skipped.len() > 24 {
            s.push_str(&format!("  … and {} more (see the manifest)\n", m.skipped.len() - 24));
        }
        s.push_str(&m.base_controls.report());
        s.push('\n');
        s.push_str(&m.liveness.report());
        s.push('\n');
        for n in &m.notes {
            s.push_str(&format!("note: {n}\n"));
        }
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Capture
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// What to capture. Every field ends up verbatim in the manifest.
pub struct CaptureSpec {
    pub eboot_window: bool,
    /// `(addr, len, origin)` - already absolute.
    pub regions: Vec<(u64, u64, String)>,
    /// Whole mappings containing these absolute addresses.
    pub whole_mappings: Vec<u64>,
    /// Pointer-following roots: `(addr_of_pointer, capture_len)`.
    pub follows: Vec<(u64, u64)>,
    pub follow_depth: u32,
    pub follow_len: u64,
    pub follow_max_regions: usize,
    pub all_resident: bool,
    pub max_mapping_bytes: u64,
    pub max_total_bytes: u64,
    // provenance echoed into the manifest
    pub selection_text: Selection,
}

struct Builder<'a> {
    proc: &'a Proc,
    regions: Vec<(u64, u64, String)>, // page-aligned, non-overlapping after merge
    /// ⭐ Ranges already read in an EARLIER phase (the in-stop read of `--snapshot-on-hit`). They
    /// are subtracted from everything this builder adds, so no byte is captured twice and - the
    /// part that matters - **no byte read while the thread was stopped is silently overwritten by a
    /// later re-read of the same address**. Two phases, one instant each, never mixed inside a
    /// region.
    blocked: Vec<(u64, u64)>,
    skipped: Vec<Skip>,
    total: u64,
    max_total: u64,
}

/// `[lo, hi)` minus every blocked range. Returns the surviving pieces in address order.
fn subtract(lo: u64, hi: u64, blocked: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut pieces = vec![(lo, hi)];
    for (blo, blen) in blocked {
        let (bhi, blo) = (blo + blen, *blo);
        let mut next = Vec::with_capacity(pieces.len() + 1);
        for (plo, phi) in pieces {
            if bhi <= plo || blo >= phi {
                next.push((plo, phi));
                continue;
            }
            if plo < blo {
                next.push((plo, blo));
            }
            if bhi < phi {
                next.push((bhi, phi));
            }
        }
        pieces = next;
        if pieces.is_empty() {
            break;
        }
    }
    pieces
}

impl<'a> Builder<'a> {
    fn covered(&self, addr: u64, len: u64) -> bool {
        self.regions.iter().any(|(a, l, _)| *a <= addr && addr + len <= *a + *l)
            || self.blocked.iter().any(|(a, l)| *a <= addr && addr + len <= *a + *l)
    }

    fn add(&mut self, addr: u64, len: u64, origin: String) -> bool {
        if len == 0 {
            return false;
        }
        let lo = addr & !(PAGE - 1);
        let hi = (addr + len).div_ceil(PAGE) * PAGE;
        // Split around anything an earlier phase already holds, then add each surviving piece.
        if !self.blocked.is_empty() {
            let pieces = subtract(lo, hi, &self.blocked.clone());
            if pieces.len() != 1 || pieces[0] != (lo, hi) {
                let mut any = false;
                for (plo, phi) in pieces {
                    any |= self.add_one(plo, phi - plo, origin.clone());
                }
                return any;
            }
        }
        self.add_one(lo, hi - lo, origin)
    }

    fn add_one(&mut self, addr: u64, len: u64, origin: String) -> bool {
        if len == 0 {
            return false;
        }
        let lo = addr;
        let hi = addr + len;
        // Clip to a readable, resident mapping - capturing across a hole would silently fabricate.
        let Some(m) = self.proc.mapping_of(lo) else {
            self.skipped.push(Skip {
                addr: lo,
                len: hi - lo,
                perms: "----".into(),
                rss: 0,
                reason: format!("{origin}: {lo:#x} is not inside any mapping"),
            });
            return false;
        };
        if !m.readable() {
            self.skipped.push(Skip {
                addr: lo,
                len: hi - lo,
                perms: m.perms.clone(),
                rss: m.rss,
                reason: format!("{origin}: mapping is not readable"),
            });
            return false;
        }
        let lo = lo.max(m.start);
        let hi = hi.min(m.end);
        if hi <= lo || self.covered(lo, hi - lo) {
            return false;
        }
        if self.total + (hi - lo) > self.max_total {
            self.skipped.push(Skip {
                addr: lo,
                len: hi - lo,
                perms: m.perms.clone(),
                rss: m.rss,
                reason: format!("{origin}: would exceed --max-total ({} bytes)", self.max_total),
            });
            return false;
        }
        self.total += hi - lo;
        self.regions.push((lo, hi - lo, origin));
        true
    }
}

/// Merge overlapping/adjacent regions, preserving the first origin and noting merges.
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

/// ⭐ The in-stop half of a `--snapshot-on-hit` capture, handed to [`capture_at_trap`] so the two
/// phases end up in ONE file with the boundary recorded rather than in two files nobody can pair.
pub struct TrapPhase {
    /// Regions read while the trapping thread was halted: `(addr, bytes, origin)`. These bytes are
    /// the trap's own instant; everything else in the file is later.
    pub preread: Vec<(u64, Vec<u8>, String)>,
    /// In-stop regions that were NOT read (the deadline fired, or the buffer filled), each carrying
    /// its reason. ⚠ They are recorded as skips, never dropped silently.
    pub dropped: Vec<Skip>,
    /// Everything the trap knows about itself. [`Coherence::regions_read_after_resume`],
    /// [`Coherence::resumed_read_end_ns`] and [`Coherence::memory_read_while_stopped`] are filled in
    /// here, because only the capture knows what ended up on the other side of the boundary.
    pub trap: TrapCapture,
}

pub fn capture(
    proc: &Proc,
    image: &d1_eboot::Image,
    base: u64,
    base_controls: BaseControls,
    live: Liveness,
    client_state: ClientState,
    spec: &CaptureSpec,
    reach: String,
    notes: Vec<String>,
) -> Result<Snapshot> {
    capture_inner(proc, image, base, base_controls, live, client_state, spec, reach, notes, None)
}

/// [`capture`], paired with a hardware-watchpoint trap. The in-stop bytes are already in hand; this
/// adds everything that is safe to read after the resume, and records exactly where the line is.
#[allow(clippy::too_many_arguments)]
pub fn capture_at_trap(
    proc: &Proc,
    image: &d1_eboot::Image,
    base: u64,
    base_controls: BaseControls,
    live: Liveness,
    client_state: ClientState,
    spec: &CaptureSpec,
    reach: String,
    notes: Vec<String>,
    phase: TrapPhase,
) -> Result<Snapshot> {
    capture_inner(
        proc,
        image,
        base,
        base_controls,
        live,
        client_state,
        spec,
        reach,
        notes,
        Some(phase),
    )
}

#[allow(clippy::too_many_arguments)]
fn capture_inner(
    proc: &Proc,
    image: &d1_eboot::Image,
    base: u64,
    base_controls: BaseControls,
    live: Liveness,
    client_state: ClientState,
    spec: &CaptureSpec,
    reach: String,
    notes: Vec<String>,
    phase: Option<TrapPhase>,
) -> Result<Snapshot> {
    let mut b = Builder {
        proc,
        regions: Vec::new(),
        blocked: phase
            .as_ref()
            .map(|p| p.preread.iter().map(|(a, d, _)| (*a, d.len() as u64)).collect())
            .unwrap_or_default(),
        skipped: phase.as_ref().map(|p| p.dropped.clone()).unwrap_or_default(),
        total: phase
            .as_ref()
            .map(|p| p.preread.iter().map(|(_, d, _)| d.len() as u64).sum())
            .unwrap_or(0),
        max_total: spec.max_total_bytes,
    };

    // 1. The eboot window - code, rodata, data and BSS at their live values, with relocations
    //    already applied and any `-p/--patch` edits already in place. This is strictly better than
    //    re-deriving them from the file, and it is why replay executes captured bytes, not ours.
    let mut load_segments = Vec::new();
    if spec.eboot_window {
        for (i, s) in image.load_segments().enumerate() {
            let span = s.memsz.max(s.filesz);
            if span == 0 {
                continue;
            }
            load_segments.push((s.vaddr, s.vaddr + span));
            let mut off = 0u64;
            // Walk the segment mapping-by-mapping: shadPS4 maps text and data separately, and a
            // single `add` across a hole would be refused (correctly) and lose the whole segment.
            while off < span {
                let a = base + s.vaddr + off;
                let remaining = span - off;
                match proc.mapping_of(a) {
                    Some(m) => {
                        let take = remaining.min(m.end - a);
                        b.add(a, take, format!("eboot-window seg{i}"));
                        off += take;
                    }
                    None => {
                        // Skip forward to the next mapping start, recording the hole.
                        let next = proc
                            .mappings()
                            .iter()
                            .filter(|m| m.start > a)
                            .map(|m| m.start)
                            .min()
                            .unwrap_or(base + s.vaddr + span);
                        let hole = (next - a).min(remaining);
                        b.skipped.push(Skip {
                            addr: a,
                            len: hole,
                            perms: "----".into(),
                            rss: 0,
                            reason: format!("eboot-window seg{i}: unmapped hole"),
                        });
                        off += hole;
                    }
                }
            }
        }
    } else {
        for s in image.load_segments() {
            load_segments.push((s.vaddr, s.vaddr + s.memsz.max(s.filesz)));
        }
    }

    // 2. Explicit ranges.
    for (a, l, o) in spec.regions.clone() {
        b.add(a, l, o);
    }

    // 3. Whole mappings.
    for a in &spec.whole_mappings {
        match proc.mapping_of(*a) {
            Some(m) => {
                let (start, end, perms, rss, len) = (m.start, m.end, m.perms.clone(), m.rss, m.len());
                if len > spec.max_mapping_bytes {
                    b.skipped.push(Skip {
                        addr: start,
                        len,
                        perms,
                        rss,
                        reason: format!(
                            "--map {a:#x}: mapping is {len} bytes, over --max-mapping ({}); \
                             name an explicit --region instead",
                            spec.max_mapping_bytes
                        ),
                    });
                } else {
                    b.add(start, end - start, format!("mapping containing {a:#x}"));
                }
            }
            None => b.skipped.push(Skip {
                addr: *a,
                len: 0,
                perms: "----".into(),
                rss: 0,
                reason: format!("--map {a:#x}: not inside any mapping"),
            }),
        }
    }

    // 4. Pointer following. Depth 1 = "deref this pointer and capture the target"; deeper walks the
    //    object graph. Bounded by --follow-max-regions, and every bound that bites is recorded.
    let mut frontier: Vec<(u64, u64, u32, String)> = spec
        .follows
        .iter()
        .map(|(a, l)| (*a, *l, 1u32, format!("follow from {a:#x}")))
        .collect();
    let mut added = 0usize;
    while let Some((ptr_at, len, depth, why)) = frontier.pop() {
        if depth > spec.follow_depth {
            continue;
        }
        let Some(raw) = proc.try_read(ptr_at, 8) else {
            b.skipped.push(Skip {
                addr: ptr_at,
                len: 8,
                perms: "----".into(),
                rss: 0,
                reason: format!("{why}: the pointer slot itself is not readable"),
            });
            continue;
        };
        let target = u64::from_le_bytes(raw.try_into().unwrap());
        if target == 0 || target < PAGE {
            b.skipped.push(Skip {
                addr: ptr_at,
                len: 8,
                perms: "----".into(),
                rss: 0,
                reason: format!("{why}: pointer is {target:#x} - NULL or not a pointer"),
            });
            continue;
        }
        if proc.mapping_of(target).is_none() {
            b.skipped.push(Skip {
                addr: target,
                len,
                perms: "----".into(),
                rss: 0,
                reason: format!("{why}: target {target:#x} is not mapped"),
            });
            continue;
        }
        if added >= spec.follow_max_regions {
            b.skipped.push(Skip {
                addr: target,
                len,
                perms: "----".into(),
                rss: 0,
                reason: format!("{why}: --follow-max-regions ({}) reached", spec.follow_max_regions),
            });
            continue;
        }
        if b.add(target, len, format!("{why} depth {depth} -> {target:#x}")) {
            added += 1;
        }
        if depth < spec.follow_depth {
            // Every 8-aligned qword in the captured object that lands in a mapping is a candidate.
            if let Some(blob) = proc.try_read(target, len as usize) {
                for i in (0..blob.len().saturating_sub(8)).step_by(8) {
                    let v = u64::from_le_bytes(blob[i..i + 8].try_into().unwrap());
                    if v >= PAGE && proc.mapping_of(v).is_some() {
                        frontier.push((
                            target + i as u64,
                            spec.follow_len,
                            depth + 1,
                            format!("follow from {:#x}", target + i as u64),
                        ));
                    }
                }
            }
        }
    }

    // 5. Everything resident, if asked. Explicitly opt-in: it is 17 GiB on this box.
    if spec.all_resident {
        let maps: Vec<_> = proc.mappings().to_vec();
        for m in maps {
            if m.rss == 0 || !m.readable() {
                continue;
            }
            if m.len() > spec.max_mapping_bytes {
                b.skipped.push(Skip {
                    addr: m.start,
                    len: m.len(),
                    perms: m.perms.clone(),
                    rss: m.rss,
                    reason: format!(
                        "--all-resident: {} bytes exceeds --max-mapping ({}). ⚠ shadPS4's sparse \
                         reservations hold the guest heap - a size filter is NOT safe to treat as \
                         'nothing there'; capture it explicitly if you need it",
                        m.len(),
                        spec.max_mapping_bytes
                    ),
                });
                continue;
            }
            b.add(m.start, m.len(), format!("all-resident {}", m.path));
        }
    }

    let regions = merge(std::mem::take(&mut b.regions));
    let skipped = std::mem::take(&mut b.skipped);

    // Read the bytes. A read that fails here fails the whole capture: a snapshot with a silently
    // short region is exactly the artefact that would make a later replay lie.
    let mut data = BTreeMap::new();
    let mut table = Vec::new();
    let file_off = 12u64; // patched after the manifest is serialised
    let mut total = 0u64;
    // Phase 1 (if any): the bytes read while the trapping thread was halted. They are NOT re-read.
    if let Some(p) = &phase {
        for (a, blob, o) in &p.preread {
            let perms = proc.mapping_of(*a).map(|m| m.perms.clone()).unwrap_or_else(|| "????".into());
            table.push(Region {
                addr: *a,
                len: blob.len() as u64,
                perms,
                file_off,
                origin: o.clone(),
            });
            total += blob.len() as u64;
            data.insert(*a, blob.clone());
        }
    }
    // Phase 2: everything else, read with the client running again.
    let mut after_resume: Vec<u64> = Vec::new();
    for (a, l, o) in &regions {
        let blob = proc
            .read(*a, *l as usize)
            .with_context(|| format!("capturing region {a:#x} len {l:#x} ({o})"))?;
        let perms = proc.mapping_of(*a).map(|m| m.perms.clone()).unwrap_or_else(|| "????".into());
        table.push(Region { addr: *a, len: *l, perms, file_off, origin: o.clone() });
        total += l;
        after_resume.push(*a);
        data.insert(*a, blob);
    }
    // ⚠ One region per address, in address order - `Snapshot::read` walks the table with
    // `range(..=cur).next_back()`, so an out-of-order or duplicated entry would silently truncate a
    // read. The two phases are interleaved by address here, and each keeps its own origin string.
    table.sort_by_key(|r| r.addr);

    if !proc.alive() {
        bail!("the client died during capture - refusing to save a snapshot that is half a corpse");
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let manifest = Manifest {
        format: FORMAT.into(),
        tool: format!("d1-replay {}", env!("CARGO_PKG_VERSION")),
        created_unix: now,
        created_utc: iso8601(now),
        eboot: EbootProvenance {
            path: image.path().display().to_string(),
            sha256: image.sha256(),
            len: image.data().len() as u64,
            load_segments,
        },
        process: ProcInfo {
            pid: proc.pid,
            starttime: proc.starttime,
            cmdline: std::fs::read_to_string(format!("/proc/{}/cmdline", proc.pid))
                .map(|s| s.replace('\0', " ").trim().to_string())
                .unwrap_or_default(),
            emulator_exe: std::fs::read_link(format!("/proc/{}/exe", proc.pid))
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        },
        base,
        base_controls,
        liveness: live,
        client_state,
        selection: spec.selection_text.clone(),
        regions: table,
        skipped,
        total_bytes: total,
        reach,
        // ⛔ `None` means NOT ATTEMPTED - this path captures memory from a RUNNING client, so no
        // thread is stopped and there is no instant to pair registers with. Synthesising a
        // `TrapCapture` here would be the exact lie `Coherence` exists to prevent. It is populated
        // ONLY when a real trap handed us its in-stop bytes.
        trap: phase.map(|p| {
            let mut t = p.trap;
            t.coherence.regions_read_after_resume = after_resume;
            t.coherence.resumed_read_end_ns =
                d1_live::watch::unix_nanos().saturating_sub(t.coherence.trap_unix_nanos);
            // TRUE only if EVERY byte in this file was read while the thread was halted. One region
            // on the far side of the resume makes it FALSE - which is the normal case, because the
            // eboot's code segment is deliberately over there.
            t.coherence.memory_read_while_stopped =
                t.coherence.regions_read_after_resume.is_empty() && !p.preread.is_empty();
            t
        }),
        notes,
    };

    // The manifest's length changes the payload offsets, so fix them up once it is final.
    let mut snap = Snapshot { manifest, data };
    let json_len = serde_json::to_vec_pretty(&snap.manifest)?.len() as u64;
    let mut off = 12 + json_len;
    // Re-serialising after mutation can change the length again only if a number's digit count
    // changes; offsets are fixed-width-free JSON, so iterate to a fixed point (converges in ≤3).
    for _ in 0..8 {
        let mut cur = off;
        for r in snap.manifest.regions.iter_mut() {
            r.file_off = cur;
            cur += r.len;
        }
        let n = 12 + serde_json::to_vec_pretty(&snap.manifest)?.len() as u64;
        if n == off {
            return Ok(snap);
        }
        off = n;
    }
    bail!("manifest offset fixup did not converge - this is a bug in d1-replay, not in the client");
}

/// Days-from-civil, so the manifest carries a human timestamp without a date dependency.
pub fn iso8601(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ The axis: **a range already read in the stop is never re-read after the resume.**
    ///
    /// If `subtract` were wrong, a post-resume region would cover addresses the in-stop phase
    /// already holds, and the later bytes would silently win - an artifact that pairs an exact
    /// register with memory from a different instant, which reads as a clean result. Every shape
    /// is asserted, including the two that must produce a HOLE and the one that must produce
    /// NOTHING.
    #[test]
    fn an_earlier_phase_is_subtracted_from_a_later_one() {
        // Disjoint: untouched.
        assert_eq!(subtract(0x1000, 0x2000, &[(0x8000, 0x1000)]), vec![(0x1000, 0x2000)]);
        // Fully covered: nothing survives - the whole range belongs to the earlier phase.
        assert_eq!(subtract(0x1000, 0x2000, &[(0x1000, 0x1000)]), Vec::new());
        assert_eq!(subtract(0x1400, 0x1800, &[(0x1000, 0x1000)]), Vec::new());
        // A bite out of the middle leaves TWO pieces - the case a naive "clip the ends"
        // implementation gets wrong, and the one that matters for eboot data inside a wider map.
        assert_eq!(
            subtract(0x1000, 0x4000, &[(0x2000, 0x1000)]),
            vec![(0x1000, 0x2000), (0x3000, 0x4000)]
        );
        // Overlapping at each end.
        assert_eq!(subtract(0x1000, 0x3000, &[(0x0800, 0x1000)]), vec![(0x1800, 0x3000)]);
        assert_eq!(subtract(0x1000, 0x3000, &[(0x2800, 0x1000)]), vec![(0x1000, 0x2800)]);
        // Several blocks at once, applied in sequence.
        assert_eq!(
            subtract(0x1000, 0x5000, &[(0x2000, 0x1000), (0x4000, 0x1000)]),
            vec![(0x1000, 0x2000), (0x3000, 0x4000)]
        );
        // No blocks at all: the identity, which is what every pre-trap capture takes.
        assert_eq!(subtract(0x1000, 0x2000, &[]), vec![(0x1000, 0x2000)]);
    }

    /// The axis: `merge` never leaves two entries covering the same byte - the invariant
    /// `controls::regions-do-not-overlap` asserts on the finished file.
    #[test]
    fn merge_leaves_a_disjoint_ascending_table() {
        let v = merge(vec![
            (0x3000, 0x1000, "c".into()),
            (0x1000, 0x1000, "a".into()),
            (0x1800, 0x1000, "b".into()),
        ]);
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].0, 0x1000);
        assert_eq!(v[0].1, 0x1800); // a and b merged, adjacency included
        assert!(v[0].2.contains('a') && v[0].2.contains('b'));
        for w in v.windows(2) {
            assert!(w[0].0 + w[0].1 <= w[1].0, "{v:?}");
        }
    }
}

/// Pull the client's reached state out of a run directory's `emulator.log`.
pub fn client_state_from_run(run_dir: Option<&Path>, label: Option<String>) -> ClientState {
    let mut cs = ClientState {
        bootflow: None,
        ui_substage: None,
        label,
        run_dir: run_dir.map(|p| p.display().to_string()),
        log_tail: Vec::new(),
    };
    let Some(dir) = run_dir else { return cs };
    let log = dir.join("emulator.log");
    let Ok(text) = std::fs::read_to_string(&log) else { return cs };
    for line in text.lines() {
        if line.contains("bootflow:") {
            cs.bootflow = Some(line.trim().to_string());
        }
        if line.contains("substage") || line.contains("ENUM(") {
            cs.ui_substage = Some(line.trim().to_string());
        }
    }
    cs.log_tail = text.lines().rev().take(20).map(|s| s.trim().to_string()).collect();
    cs.log_tail.reverse();
    cs
}
