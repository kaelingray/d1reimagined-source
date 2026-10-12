// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-replay` - capture the live client's memory, then execute its own functions on it with full
//! observation.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use d1_replay::addr::{self, Ctx, Width};
use d1_replay::live::{self, Proc};
use d1_replay::machine::{self, reg_by_name, CapturedRegs, Machine, RunSpec, StackCanary};
use d1_replay::preset;
use d1_replay::search::{self, Candidates, Knob, Predicate};
use d1_replay::snapshot::{self, CaptureSpec, Selection, Snapshot};
use d1_replay::symbols::Symbols;

const LIMITATION: &str = "\
⚠ WHAT THIS CANNOT DO - say it out loud before quoting any result:
   • REGISTERS DEPEND ON THE SNAPSHOT KIND, and there are two. /proc/<pid>/mem is memory only,
     so an ORDINARY capture has no registers: a replay from one is YOUR call with YOUR register
     values against the client's REAL memory - report 'input X makes this function hit', never
     'the client's input was X'. A snapshot taken with `capture --snapshot-on-hit` DOES carry
     them (PTRACE_GETREGS at a hardware-watchpoint trap, not memory), and `replay
     --use-captured-regs` seeds the integer file from it - report 'this function, with the
     integer arguments the client actually held, against memory read over the following N ms
     while ~110 other threads ran'. Never 'the state at the moment of the write'.
     ⚠ Which one you got is printed in the run's own TRAP and REGISTERS blocks. Quote those,
     not this banner. `trap: none` means NOT ATTEMPTED - it never means the client had no
     registers - and --use-captured-regs REFUSES on such a snapshot rather than inventing.
     ⛔ rip, rsp, rbp and ALL FP/SSE state are invented in BOTH kinds.
   • A snapshot is ONE instant. It says nothing about T±ε.
   • It replays a fragment: no threads, no syscalls, no allocator. Anything that needs those faults,
     and a fault is reported as a RESULT.
   • Reads outside the captured regions FAIL. They are never zero-filled - 'not captured' and
     'was zero' are different facts and only one is evidence.
   • Reach tags propagate: a snapshot of a client driven by our own synthetic server stays
     synthetic_ancestor no matter how faithfully it re-executes.
   • ⭐ IT ALTERS ONE POINTER BY DEFAULT - the stack canary at v:0x1A06C78, which ≥45% of this
     binary's functions dereference in their prologue and which the client filled with a HOST
     thread-local address Unicorn cannot map. --stack-canary auto repoints it so execution gets
     past the check; that is OUR value, reported on every run and in --json, and it buys exactly
     'it executed past the canary' and nothing more. --stack-canary off leaves it untouched.";

#[derive(Parser)]
#[command(
    name = "d1-replay",
    about = "The live-state replay oracle: snapshot the client's memory, then execute its own code on it",
    long_about = None,
    after_long_help = LIMITATION
)]
struct Cli {
    /// The eboot to read provenance and file-image bytes from. Defaults to $D1_EBOOT, else pinned.
    #[arg(long, global = true)]
    eboot: Option<PathBuf>,
    /// The guest symbol file. Defaults to $D1_GUEST_SYMBOLS, else capture-rig/guest-symbols.json.
    #[arg(long, global = true)]
    symbols: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Snapshot a live client's memory to a portable file.
    Capture(CaptureArgs),
    /// Describe a snapshot: provenance, controls, regions, and what was skipped.
    Info { snapshot: PathBuf },
    /// Read from a snapshot without executing anything.
    Read(ReadArgs),
    /// Execute one function on a snapshot, with observation.
    Replay(ReplayArgs),
    /// ⭐ Run a function over a set of candidate inputs and report which ones hit.
    Search(SearchArgs),
    /// Run every positive control against a snapshot. Exits non-zero on any FAIL.
    Controls { snapshot: PathBuf },
    /// ⭐ Re-derive the activity-lifetime record facts from a snapshot - the control that lets a
    /// later session re-check the lifetime-phase evidence instead of trusting a transcript.
    Rederive(RederiveArgs),
    /// ⭐ The A/B control for a capture PRESET: does it capture what the function actually reads?
    PresetControl(PresetControlArgs),
    /// (internal) a throwaway child to measure `offsetof(struct user, u_debugreg)` against.
    ///
    /// ⛔ That measurement pokes zero into every debug register of its tracee, so it must never be
    /// the emulator. This subcommand exists so `--snapshot-on-hit` can run the control against a
    /// process of OUR OWN before it arms anything at the client.
    #[command(hide = true, name = "__stopped-child")]
    StoppedChild {
        #[arg(long, default_value_t = 20.0)]
        seconds: f64,
    },
}

#[derive(Args)]
struct CaptureArgs {
    /// Where to write the snapshot.
    #[arg(short, long)]
    out: PathBuf,
    /// Which process, if more than one emulator is running (there should never be).
    #[arg(long)]
    pid: Option<i32>,
    /// The run directory, so the manifest records the state the client had reached.
    #[arg(long)]
    run_dir: Option<PathBuf>,
    /// Free-text label for this instant ("at the fireteam crash", "parked in orbit").
    #[arg(long)]
    state: Option<String>,
    /// Reach tag for everything downstream of this snapshot.
    #[arg(long, default_value = "synthetic_ancestor")]
    reach: String,
    /// Capture the eboot's LOAD segments (code, rodata, data, BSS) as the client has them.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    eboot_window: bool,
    /// Extra range: `EXPR:LEN`, e.g. `v:0x26A2040:0x40` or `a:0x205015e70:0x400`.
    #[arg(long = "region", value_name = "EXPR:LEN")]
    regions: Vec<String>,
    /// Capture the whole mapping containing this address.
    #[arg(long = "map", value_name = "EXPR")]
    maps: Vec<String>,
    /// ⭐ Apply a named preset, deriving its regions from the client's own tables.
    /// `content` = the package/content registry working set (arena table, the content DB blob's
    /// arena, the registry slot table, every populated slot's entry array, and the objects those
    /// entries point back at) - so a content-DB lookup replays offline in ONE capture instead of N.
    #[arg(long = "preset", value_name = "NAME")]
    presets: Vec<String>,
    /// Bytes taken at each object a preset resolves, from the object's own start.
    #[arg(long, value_parser = parse_u64, default_value = "0x1000")]
    preset_object_window: u64,
    /// A preset takes an enclosing named arena WHOLE only up to this size; above it, a window.
    #[arg(long, value_parser = parse_u64, default_value = "0x2000000")]
    preset_arena_cap: u64,
    /// A preset REFUSES (loudly) rather than silently truncating above this many bytes.
    #[arg(long, value_parser = parse_u64, default_value = "0x10000000")]
    preset_max: u64,
    /// Dereference the u64 at EXPR and capture LEN bytes at the target (default --follow-len).
    #[arg(long = "follow", value_name = "EXPR[:LEN]")]
    follows: Vec<String>,
    #[arg(long, default_value_t = 1)]
    follow_depth: u32,
    #[arg(long, value_parser = parse_u64, default_value = "0x400")]
    follow_len: u64,
    #[arg(long, default_value_t = 512)]
    follow_max_regions: usize,
    /// Capture every readable mapping with resident pages, subject to the caps below.
    #[arg(long)]
    all_resident: bool,
    #[arg(long, value_parser = parse_u64, default_value = "0x10000000")]
    max_mapping: u64,
    #[arg(long, value_parser = parse_u64, default_value = "0x80000000")]
    max_total: u64,
    /// Scan every readable mapping for the base anchors, not just executable ones.
    #[arg(long)]
    full_scan: bool,
    /// Refuse to save below this liveness level (see historicaldocs/design/runtime-state-access.md §3).
    #[arg(long, default_value_t = 1)]
    require_liveness: u8,
    /// The account SOID the L3 predicate expects.
    #[arg(long)]
    account_soid: Option<String>,
    /// Wait until this u32/u64 expression is non-zero before capturing (a liveness predicate, never
    /// a sleep). Form: `u32@EXPR` or `u64@EXPR`.
    #[arg(long)]
    wait_nonzero: Option<String>,
    /// Seconds to wait for --wait-nonzero.
    #[arg(long, default_value_t = 120.0)]
    wait_timeout: f64,

    // ── ⭐⭐ --snapshot-on-hit: the capture at a hardware-watchpoint trap ─────────────────────────
    /// ⭐⭐ Arm a hardware watchpoint at EXPR and take this snapshot **at the trap**, with the
    /// trapping thread halted - so the file carries the client's own registers as well as its
    /// memory. Spec: `historicaldocs/design/live-watch.md` §6a. ⚠ ONE arm, ONE trap, ONE artifact.
    #[arg(long = "snapshot-on-hit", value_name = "EXPR")]
    snapshot_on_hit: Option<String>,
    /// Watchpoint width: 1, 2, 4 or 8. The address must be naturally aligned to it.
    #[arg(long, default_value_t = 8)]
    hit_len: u64,
    /// `write` | `rw` | `exec` (a hardware breakpoint; needs --hit-len 1).
    #[arg(long, default_value = "write")]
    hit_rw: String,
    /// Which debug register, 0..3.
    #[arg(long, default_value_t = 0)]
    hit_slot: usize,
    /// Give up (and write nothing) after this long with no trap.
    #[arg(long, default_value_t = 120.0)]
    hit_timeout: f64,
    /// Arm only these threads. ⚠ Then "no hit" is evidence about these threads only.
    #[arg(long = "hit-tid")]
    hit_tids: Vec<i32>,
    /// Also arm threads created after the arm.
    #[arg(long)]
    follow_clone: bool,
    /// What to read INSIDE the stop. `minimal` = the watched page, the register roots and the
    /// stack; `state` (default) adds the eboot's data+BSS - the singletons and the state machine;
    /// `wide` adds whatever --region/--follow/--preset resolved, bounded by the hard cap.
    #[arg(long, default_value = "state")]
    stop_scope: String,
    #[arg(long, value_parser = parse_u64, default_value = "0x4000000")]
    stop_budget_bytes: u64,
    /// The in-stop deadline, enforced between 1 MiB chunks. One 60 fps frame by default.
    #[arg(long, default_value_t = 16.0)]
    stop_budget_ms: f64,
    /// ⛔ Above this the tool REFUSES - before arming, so no trigger is spent.
    #[arg(long, value_parser = parse_u64, default_value = "0x20000000")]
    stop_hard_cap_bytes: u64,
    #[arg(long, default_value_t = 250.0)]
    stop_hard_cap_ms: f64,
    /// Registers whose pointees get a window in the stop. Default: the System V integer arguments
    /// plus rbx/rbp. ⭐ These are the arguments the client actually passed.
    #[arg(long = "reg-root")]
    reg_roots: Vec<String>,
    #[arg(long, value_parser = parse_u64, default_value = "0x1000")]
    reg_window: u64,
    #[arg(long, value_parser = parse_u64, default_value = "0x10000")]
    stack_window: u64,
    /// `stopped` (default) or `resumed`. ⚠ `resumed` reads ALL memory after the thread restarts:
    /// wider, and its failure mode is silent - a freed object under an exact pointer. The file
    /// always records which it got.
    #[arg(long, default_value = "stopped")]
    snapshot_when: String,
    /// `none` (default). `all` is `[U]` and unimplemented - see historicaldocs/design/live-watch.md §6a.5.
    #[arg(long, default_value = "none")]
    stop_threads: String,
    /// ⭐ The wall-clock bound on the pre-capture disarm - the step that brackets the stop.
    ///
    /// Measured at 0.68 ms for 111 threads, so the 1500 ms default is ~2,200x headroom. If it
    /// expires the capture is **REFUSED** rather than run with threads nobody proved clear: the
    /// trapping thread is released, an unbounded disarm follows, and nothing is written. ⚠ Raising
    /// this raises the maximum time the client's trapping thread can be held stopped.
    #[arg(long, default_value_t = 1500.0)]
    disarm_deadline_ms: f64,
}

#[derive(Args)]
struct ReadArgs {
    snapshot: PathBuf,
    /// Address expression, e.g. `v:0x2815860`, `[v:0x2815860]+0xd8`, `object_registry_ptr`.
    expr: String,
    #[arg(long, value_parser = parse_u64, default_value = "0x40")]
    len: u64,
    /// Render as `hex` (default), `u32`, `u64`, `ptr`.
    #[arg(long, default_value = "hex")]
    r#as: String,
}

#[derive(Args)]
struct RunCommon {
    /// The function to call, as an address expression (bare numbers are eboot vaddrs).
    #[arg(long)]
    func: String,
    /// Integer arguments, in System V order. Each is a value expression: a number, an address
    /// expression, `scratch`, or a load like `u32@[v:0x2815860]+0xf4`.
    #[arg(long = "arg")]
    args: Vec<String>,
    /// Set a register directly: `--reg r13=0x…`.
    #[arg(long = "reg")]
    regs: Vec<String>,
    /// ⭐ Start from the register values the CLIENT actually had, instead of from zeros.
    ///
    /// Only a snapshot taken by `capture --snapshot-on-hit` carries them (`d1-replay info` prints
    /// `trap` in both states); without one this flag REFUSES rather than falling back, because a
    /// flag whose purpose is to stop inventing registers must never quietly invent them.
    ///
    /// What it seeds: the 14 general-purpose registers other than `rsp`/`rbp`, plus `eflags` with
    /// `TF`/`RF` cleared. What it does NOT: `rip` (the entry is `--func`), the frame (`rsp`/`rbp`
    /// are this run's own, so the call can stop by returning), and anything floating point - there
    /// is no `xmm` in a capture. `--arg` and `--reg` still win, per register.
    ///
    /// ⚠ Whichever mode a run uses is printed on EVERY run in EVERY mode and carried in `--json`.
    #[arg(long = "use-captured-regs")]
    use_captured_regs: bool,
    /// Pre-run memory write: `--set-mem EXPR=HEXBYTES`.
    #[arg(long = "set-mem")]
    set_mem: Vec<String>,
    /// Short-circuit a call: `--stub EXPR=RETVAL`. Use sparingly and say so in the finding.
    #[arg(long = "stub")]
    stubs: Vec<String>,
    /// Record register state the first time each address is reached.
    #[arg(long = "probe")]
    probes: Vec<String>,
    #[arg(long, value_parser = parse_u64, default_value = "2000000")]
    step_limit: u64,
    /// ⭐ Where the stack-guard pointer at v:0x1A06C78 points for this run.
    ///
    /// At least 45% of this binary's functions (16,117 of 35,834 - a FLOOR: the scanner that
    /// measured it can only under-count) load it in their prologue and dereference
    /// it. The client's own value is a pointer into HOST thread-local storage, which Unicorn cannot
    /// map, so without this every stack-protected function faults at step 8 having done nothing.
    ///
    /// `auto` (default) redirects it at a mapped guest slot - but only when the captured pointer is
    /// unmapped, so it never alters memory that already worked. `off` leaves the client's bytes
    /// alone, which is what you want when the prologue fault IS the subject. Or give an address
    /// expression to point it wherever you like. ⚠ Whatever it does is printed on every run and
    /// recorded in --json: this flag alters captured memory and never does so silently.
    #[arg(long = "stack-canary", value_name = "auto|off|ADDR", default_value = "auto")]
    stack_canary: String,
}

#[derive(Args)]
struct ReplayArgs {
    snapshot: PathBuf,
    #[command(flatten)]
    common: RunCommon,
    /// Single-step and print up to N instructions.
    #[arg(long)]
    trace: Option<usize>,
    /// Watchpoint: `--watch-read EXPR:LEN`.
    #[arg(long = "watch-read")]
    watch_reads: Vec<String>,
    #[arg(long = "watch-write")]
    watch_writes: Vec<String>,
    /// Dump this range after the run (post-state observation): `--dump EXPR:LEN`.
    #[arg(long = "dump")]
    dumps: Vec<String>,
    /// Skip the control block. The output is then stamped NOT EVIDENCE.
    #[arg(long)]
    no_controls: bool,
}

#[derive(Args)]
struct SearchArgs {
    snapshot: PathBuf,
    #[command(flatten)]
    common: RunCommon,
    /// A knob to turn. `reg:RDI=0,1,2` or `mem:EXPR:u32=0..255` or `mem:EXPR:u32=table` where
    /// `table` is replaced by the distinct values already present (see --from-table).
    #[arg(long = "vary", required = true)]
    vary: Vec<String>,
    /// The outcome that counts as a hit: `ret!=0` | `nofault` | `reached:EXPR` |
    /// `EXPR:REG!=0` (a register at a probe address).
    #[arg(long)]
    hit: Option<String>,
    /// Run the rollback control after EVERY candidate, not just the first and last.
    #[arg(long)]
    verify_every: bool,
    /// Print every candidate, not just the hits and a summary.
    #[arg(long)]
    all: bool,
    #[arg(long)]
    no_controls: bool,
}

#[derive(Args)]
struct PresetControlArgs {
    /// The snapshot captured WITHOUT the preset.
    #[arg(long)]
    without: PathBuf,
    /// The snapshot captured WITH it - same client, same instant, one flag different.
    #[arg(long = "with")]
    with_: PathBuf,
    /// The preset the `--with` snapshot must declare.
    #[arg(long, default_value = "content")]
    preset: String,
    /// The function whose reads are the axis. Default: the rung-34 content lookup.
    #[arg(long, default_value = "v:0x10c500")]
    func: String,
    #[arg(long = "arg")]
    args: Vec<String>,
    #[arg(long = "set-mem")]
    set_mem: Vec<String>,
    /// ⭐ Same meaning as on `replay`. ⚠ `off` makes BOTH arms fault in the prologue at 8 steps, so
    /// neither can retire more than the other and this control can never pass - which is exactly
    /// what it did before `--stack-canary` existed.
    #[arg(long = "stack-canary", value_name = "auto|off|ADDR", default_value = "auto")]
    stack_canary: String,
    /// ⚠ INVERT the control: assert the preset does NOT help. A control that cannot fail is
    /// decoration, so this exists to demonstrate that it can.
    #[arg(long)]
    invert: bool,
    #[arg(long, value_parser = parse_u64, default_value = "2000000")]
    step_limit: u64,
}

#[derive(Args)]
struct RederiveArgs {
    snapshot: PathBuf,
    /// The content-record datum to resolve. Default: the activity-lifetime record
    /// (class 0x8080039D, globals record 3 - design/activity-lifetime-state.md).
    #[arg(long, value_parser = parse_u64, default_value = "0x27FA2003")]
    datum: u64,
    /// Write the full record+0x1AC value image (0x504 bytes) as bare hex to this path.
    /// REFUSES if the image range is not captured - a partial image would read as a whole one.
    #[arg(long)]
    dump_image: Option<PathBuf>,
    /// Run this subcommand's own controls against the snapshot instead of deriving. Exit 2 on
    /// any FAIL. ⚠ Controls always use the canonical datum 0x27FA2003 - the known answers are
    /// facts about THAT record - regardless of --datum.
    #[arg(long)]
    controls: bool,
}

fn parse_u64(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|e| e.to_string())
    } else {
        s.parse().map_err(|e: std::num::ParseIntError| e.to_string())
    }
}

fn main() {
    match real_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("d1-replay: {e:#}");
            std::process::exit(1);
        }
    }
}

fn real_main() -> Result<i32> {
    let cli = Cli::parse();
    // ⚠ Before the symbol/eboot gate: this child must be startable with no address book at all,
    // because it is what proves we may poke debug registers in the first place.
    if let Cmd::StoppedChild { seconds } = &cli.cmd {
        std::thread::sleep(std::time::Duration::from_secs_f64(*seconds));
        return Ok(0);
    }
    let syms = match &cli.symbols {
        Some(p) => Symbols::open(p)?,
        None => Symbols::open_default()?,
    };
    let image = live::open_image(cli.eboot.clone())?;

    match cli.cmd {
        Cmd::Capture(a) => cmd_capture(&a, &syms, &image),
        Cmd::Info { snapshot } => {
            let s = Snapshot::load(&snapshot)?;
            println!("{}", s.summary());
            Ok(0)
        }
        Cmd::Read(a) => cmd_read(&a, &syms),
        Cmd::Replay(a) => cmd_replay(&a, &syms, &image, cli.json),
        Cmd::Search(a) => cmd_search(&a, &syms, &image),
        Cmd::PresetControl(a) => cmd_preset_control(&a, &syms, &image),
        Cmd::StoppedChild { .. } => unreachable!("handled above, before the build gate"),
        Cmd::Controls { snapshot } => {
            let s = Rc::new(Snapshot::load(&snapshot)?);
            let rep = d1_replay::controls::run(s, &syms, &image, Some(snapshot.as_path()))?;
            println!("{}", rep.render());
            Ok(if rep.ok() { 0 } else { 2 })
        }
        Cmd::Rederive(a) => cmd_rederive(&a),
    }
}

// ── capture ──────────────────────────────────────────────────────────────────────────────────────

fn split_len(s: &str, default: u64) -> (String, u64) {
    // Split on the LAST ':' only when what follows parses as a number - address expressions
    // legitimately contain `v:` and `a:` prefixes.
    if let Some(i) = s.rfind(':') {
        let head = &s[..i];
        // `v:`/`a:` are address-space prefixes, not a length separator; anything else before the
        // last `:` is a real expression (a symbol name, a number, a bracketed deref).
        if head != "v" && head != "a" && !head.is_empty() {
            if let Ok(n) = parse_u64(&s[i + 1..]) {
                return (head.to_string(), n);
            }
        }
    }
    (s.to_string(), default)
}

fn cmd_capture(a: &CaptureArgs, syms: &Symbols, image: &d1_eboot::Image) -> Result<i32> {
    let (build_ok, build_line) = syms.check_build(&image.sha256());
    println!("{build_line}");
    if !build_ok {
        bail!(
            "refusing: the symbol file was measured against a different eboot. Every vaddr in it is \
             suspect."
        );
    }

    // ⭐ Validate every --snapshot-on-hit flag BEFORE looking for a client. A refusal that only
    // appears once an emulator is running is a refusal the operator meets with a live session in
    // front of them, and `--stop-threads all` is unimplemented for reasons no run can change.
    let on_hit = a.snapshot_on_hit.as_ref().map(|_| validate_on_hit_flags(a)).transpose()?;

    let pid = match a.pid {
        Some(p) => p,
        None => {
            let pids = live::emulator_pids()?;
            match pids.len() {
                0 => bail!(
                    "no running shadps4 (pgrep -f '[s]hadps4 -p ') - a runtime capture with no \
                     runtime is an ERROR, not an empty snapshot"
                ),
                1 => pids[0],
                n => bail!("{n} emulators running ({pids:?}); refusing to guess. One emulator, one owner."),
            }
        }
    };
    let proc = Proc::open(pid)?;

    let ctl = live::recover_base(&proc, image, syms, a.full_scan);
    println!("{}", ctl.report());
    let Some(base) = ctl.base.filter(|_| ctl.ok()) else {
        bail!("base controls FAILED - refusing to capture. A wrong base yields plausible-looking bytes, not an error.");
    };

    let soid = match &a.account_soid {
        Some(s) => parse_u64(s).map_err(|e| anyhow!("{e}"))?,
        None => syms.account_soid,
    };

    // Optional liveness predicate - never a sleep (ATLAS §4.6b).
    if let Some(pred) = &a.wait_nonzero {
        let ctx = mk_ctx(base, syms, &proc);
        let t0 = std::time::Instant::now();
        let mut last;
        loop {
            let load = |ad: u64, w: Width| -> Result<u64> {
                Ok(w.decode(&proc.read(ad, w.bytes())?))
            };
            let v = addr::value(pred, &ctx, &load).unwrap_or(0);
            ctx.take_trace();
            last = v;
            if v != 0 {
                println!("WAIT: {pred} became {v:#x} after {:.1}s", t0.elapsed().as_secs_f64());
                break;
            }
            if t0.elapsed().as_secs_f64() > a.wait_timeout {
                bail!(
                    "WAIT TIMEOUT: {pred} was still {last:#x} after {:.0}s. This is a RESULT - the \
                     client never reached that state - not a reason to capture anyway.",
                    a.wait_timeout
                );
            }
            if !proc.alive() {
                bail!("the client died while waiting for {pred} - an ERROR, not a zero");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    let live_state = live::liveness(&proc, syms, base, soid);
    println!("{}", live_state.report());
    if live_state.level < a.require_liveness {
        bail!(
            "refusing to save: this capture needs L{} and the client is at L{}. A snapshot taken \
             from a dead or unverified process is refused, not saved (pass --require-liveness 0 and \
             own the label).",
            a.require_liveness,
            live_state.level
        );
    }

    let ctx = mk_ctx(base, syms, &proc);
    let mut regions = Vec::new();
    for r in &a.regions {
        let (e, len) = split_len(r, 0x100);
        let ad = addr::addr(&e, &ctx)?;
        regions.push((ad, len, format!("--region {r}")));
    }
    let mut maps = Vec::new();
    for m in &a.maps {
        maps.push(addr::addr(m, &ctx)?);
    }
    let mut follows = Vec::new();
    for f in &a.follows {
        let (e, len) = split_len(f, a.follow_len);
        follows.push((addr::addr(&e, &ctx)?, len));
    }
    for l in ctx.take_trace() {
        println!("RESOLVE{l}");
    }

    // ── PRESETS ──────────────────────────────────────────────────────────────────────────────────
    // Derived from the client's own tables, at THIS instant, and every derived fact is printed and
    // then written into the manifest. A preset that silently produced nothing would be
    // indistinguishable from one that worked, so a failing walk is an ERROR and aborts the capture.
    let mut preset_notes: Vec<String> = Vec::new();
    for name in &a.presets {
        let opts = preset::Opts {
            object_window: a.preset_object_window,
            arena_cap: a.preset_arena_cap,
            max_bytes: a.preset_max,
        };
        let p = preset::by_name(name, &proc, base, syms, opts)
            .with_context(|| format!("--preset {name}"))?;
        println!("\nPRESET {name}:");
        for n in &p.notes {
            println!("  {n}");
        }
        for s in &p.skips {
            println!("  ⚠ {s}");
        }
        println!("  -> {} region(s) added to the selection", p.regions.len());
        preset_notes.extend(p.notes.iter().cloned());
        preset_notes.extend(p.skips.iter().map(|s| format!("⚠ {s}")));
        for (lo, hi, why) in p.regions {
            regions.push((lo, hi - lo, why));
        }
    }

    let spec = CaptureSpec {
        eboot_window: a.eboot_window,
        regions,
        whole_mappings: maps,
        follows,
        follow_depth: a.follow_depth,
        follow_len: a.follow_len,
        follow_max_regions: a.follow_max_regions,
        all_resident: a.all_resident,
        max_mapping_bytes: a.max_mapping,
        max_total_bytes: a.max_total,
        selection_text: Selection {
            eboot_window: a.eboot_window,
            presets: a.presets.clone(),
            preset_object_window: a.preset_object_window,
            preset_arena_cap: a.preset_arena_cap,
            explicit_regions: a.regions.clone(),
            whole_mappings: a.maps.clone(),
            follows: a.follows.clone(),
            follow_depth: a.follow_depth,
            follow_len: a.follow_len,
            follow_max_regions: a.follow_max_regions,
            all_resident: a.all_resident,
            max_mapping_bytes: a.max_mapping,
            max_total_bytes: a.max_total,
        },
    };

    let cs = snapshot::client_state_from_run(a.run_dir.as_deref(), a.state.clone());

    // ── ⭐⭐ the capture at a trap ────────────────────────────────────────────────────────────────
    if let (Some(expr), Some(opts)) = (a.snapshot_on_hit.clone(), on_hit) {
        return cmd_capture_on_hit(
            a,
            &expr,
            opts,
            &proc,
            image,
            base,
            ctl,
            live_state,
            cs,
            &spec,
            syms,
            preset_notes,
        );
    }

    let snap = snapshot::capture(
        &proc,
        image,
        base,
        ctl,
        live_state,
        cs,
        &spec,
        a.reach.clone(),
        {
            let mut n = vec![
                "no registers are captured BY THIS PATH - `trap: none` here means NOT ATTEMPTED, \
                 never \"the client had none\". `capture --snapshot-on-hit EXPR` is the path that \
                 does capture them, and `replay --use-captured-regs` is what consumes them; see \
                 `d1-replay --help` for what a replay can and cannot claim."
                    .to_string(),
            ];
            n.extend(preset_notes);
            n
        },
    )?;
    let bytes = snap.save(&a.out)?;
    println!(
        "\nSNAPSHOT {} - {} region(s), {:.1} MiB captured, {:.1} MiB on disk",
        a.out.display(),
        snap.manifest.regions.len(),
        snap.manifest.total_bytes as f64 / (1u64 << 20) as f64,
        bytes as f64 / (1u64 << 20) as f64
    );
    println!("{}", snap.summary());
    Ok(0)
}

// ── ⭐⭐ capture --snapshot-on-hit ────────────────────────────────────────────────────────────────
//
// THE ORDER IS THE SAFETY ARGUMENT, and it is worth restating at the call site because a future
// edit that reorders it would look harmless:
//
//   resolve → plan → REHEARSE against the live client → refuse above the hard cap → allocate the
//   buffer → measure the debug-register offset against a child of OUR OWN → arm → trap → DISARM AND
//   VERIFY → read → resume → read the rest → write.
//
// The refusal is BEFORE the arm because a refusal at hit time burns the trigger; this project has
// already paid for that once (historicaldocs/design/live-watch.md §2.2a). The allocation is before the arm because
// an allocation failure calls abort(), which does not unwind, so no Drop would run. And the disarm
// is before the read because the read is the long, panicking-capable part.
/// Parse and refuse every `--snapshot-on-hit` flag, with no client anywhere near it.
///
/// ⛔ `--stop-threads all` is `[U]` and refused here rather than half-built: it re-enters the
/// process-wide `waitpid` ownership `Watcher::run` holds, and every interrupted thread would then
/// need the queued-`SIGTRAP` drain or the intermittently-lethal disarm bug comes back in a new
/// place (historicaldocs/design/live-watch.md §6a.5).
fn validate_on_hit_flags(a: &CaptureArgs) -> Result<d1_replay::onhit::Opts> {
    use d1_live::ptrace::Rw;
    use d1_replay::onhit::{self, Opts, StopScope};

    if a.stop_threads != "none" {
        bail!(
            "--stop-threads {:?} is [U] and NOT implemented. Stopping every thread would make the \
             image genuinely atomic, but it re-enters the process-wide waitpid ownership \
             `Watcher::run` holds, and every interrupted thread would then need the queued-SIGTRAP \
             drain or the intermittently-lethal disarm bug comes back in a new place. A half-built \
             version of this is worse than not having it - historicaldocs/design/live-watch.md §6a.5.",
            a.stop_threads
        );
    }
    let when_stopped = match a.snapshot_when.as_str() {
        "stopped" => true,
        "resumed" => false,
        other => bail!("unknown --snapshot-when {other:?} - use stopped | resumed"),
    };
    // Parsed here so an unusable watchpoint (`--hit-rw exec --hit-len 8`, an unaligned address)
    // fails before anything is armed rather than after.
    let rw = Rw::parse(&a.hit_rw)?;
    d1_live::ptrace::dr7_for(a.hit_slot, rw, a.hit_len)?;
    Ok(Opts {
        scope: StopScope::parse(&a.stop_scope)?,
        budget_bytes: a.stop_budget_bytes,
        budget_ms: a.stop_budget_ms,
        hard_cap_bytes: a.stop_hard_cap_bytes,
        hard_cap_ms: a.stop_hard_cap_ms,
        reg_roots: if a.reg_roots.is_empty() {
            onhit::DEFAULT_REG_ROOTS.iter().map(|s| s.to_string()).collect()
        } else {
            a.reg_roots.clone()
        },
        reg_window: a.reg_window,
        stack_window: a.stack_window,
        when_stopped,
        ..Opts::default()
    })
}

#[allow(clippy::too_many_arguments)]
fn cmd_capture_on_hit(
    a: &CaptureArgs,
    expr: &str,
    opts: d1_replay::onhit::Opts,
    proc: &Proc,
    image: &d1_eboot::Image,
    base: u64,
    base_controls: d1_replay::live::BaseControls,
    live_state: d1_replay::live::Liveness,
    cs: snapshot::ClientState,
    spec: &CaptureSpec,
    syms: &Symbols,
    preset_notes: Vec<String>,
) -> Result<i32> {
    use d1_live::ptrace::Rw;
    use d1_live::watch::{WatchSpec, Watcher};
    use d1_replay::onhit::{self, Verdict};
    use d1_replay::snapshot::{
        Coherence, PhaseSplitControl, Skip, StopBudget, ThreadRegs, TrapCapture, TrapPhase,
    };

    let when_stopped = opts.when_stopped;

    // ── resolve the address, and PRINT how it resolved ──────────────────────────────────────────
    let ctx = mk_ctx(base, syms, proc);
    let target = addr::addr(expr, &ctx)?;
    for l in ctx.take_trace() {
        println!("RESOLVE{l}");
    }
    let rw = Rw::parse(&a.hit_rw)?;
    println!(
        "\nON-HIT   DR{} {:?} {} byte(s) at {target:#x} (v:{:#x})",
        a.hit_slot,
        rw,
        a.hit_len,
        target.wrapping_sub(base)
    );

    // ── plan and REHEARSE, against this client, before anything is armed ────────────────────────
    let plan = onhit::plan(proc, image, base, target, &opts, &spec.regions)?;
    println!(
        "PLAN     --stop-scope {}: {} in-stop region(s), {:#x} bytes ({:.2} MiB) fixed + {:#x} \
         allowance for the register roots and the stack",
        opts.scope.name(),
        plan.fixed.len(),
        plan.fixed_bytes,
        plan.fixed_bytes as f64 / (1u64 << 20) as f64,
        plan.dynamic_allowance
    );
    for n in &plan.notes {
        println!("  ⚠ {n}");
    }
    println!(
        "REHEARSE {:.2} MiB read from the live client in {:.2} ms ({:.0} MiB/s) - every in-stop \
         region is proven readable, and the budget below is checked against THIS rate, not an \
         assumed one",
        plan.rehearsed_bytes as f64 / (1u64 << 20) as f64,
        plan.rehearsed_ms,
        (plan.rehearsed_bytes as f64 / (1u64 << 20) as f64) / (plan.rehearsed_ms / 1000.0).max(1e-9)
    );
    println!(
        "PREDICT  {:#x} bytes ({:.2} MiB) at {:.2} ms in the stop",
        plan.predicted_bytes,
        plan.predicted_bytes as f64 / (1u64 << 20) as f64,
        plan.predicted_ms
    );

    let mut notes: Vec<String> = Vec::new();
    match onhit::verdict(plan.predicted_bytes, plan.predicted_ms, &opts) {
        Verdict::Ok => println!(
            "BUDGET   OK - under --stop-budget-bytes ({:#x}) and --stop-budget-ms ({:.0})",
            opts.budget_bytes, opts.budget_ms
        ),
        Verdict::OverBudget(m) => {
            println!("BUDGET   ⚠ OVER BUDGET: {m}");
            notes.push(format!("⚠ the in-stop set was over budget before the arm: {m}"));
        }
        // ⛔ Nothing has been armed and no trigger has been spent.
        Verdict::Refuse(m) => bail!("REFUSING to arm. {m}"),
    }

    // ── allocate BEFORE the arm: no allocation inside the stop, ever ────────────────────────────
    let mut buf = vec![0u8; (plan.fixed_bytes + plan.dynamic_allowance) as usize];
    let mut scratch = vec![0u8; snapshot::PAGE as usize];

    // ── the debug-register offset, measured against a child of OUR OWN ──────────────────────────
    let me = std::env::current_exe().context("locating this binary")?;
    let mut child = std::process::Command::new(&me)
        .arg("__stopped-child")
        .stdout(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("spawning the debug-register control child {}", me.display()))?;
    let dbg = d1_live::controls::debugreg_offset_against(child.id() as i32);
    let _ = child.kill();
    let _ = child.wait();
    let (dbg, dbg_detail) = dbg.context(
        "the debug-register offset control FAILED - refusing to poke the client's debug registers",
    )?;
    println!("DEBUGREG CONTROL: PASS  u_debugreg[0] at {dbg:#x} - {dbg_detail}");

    // ── arm ─────────────────────────────────────────────────────────────────────────────────────
    // ⛔ BEFORE the arm, and its receipt is printed: from here to the trap - up to `--hit-timeout`,
    // 120 s by default - this process holds a live debug register on the client. Build 2 handled
    // SIGINT and SIGTERM only, so a dropped terminal (SIGHUP) inside that window killed the client;
    // measured 2 of 2 and now covered by d1-live's `fatal-signals-disarm` control.
    let sigs = d1_live::watch::install_signal_handlers();
    println!("{}", sigs.summary());
    let wspec = WatchSpec {
        addr: target,
        len: a.hit_len,
        rw,
        slot: a.hit_slot,
        follow_clone: a.follow_clone,
        base: Some(base),
        tids: if a.hit_tids.is_empty() { None } else { Some(a.hit_tids.clone()) },
    };
    let mut w = Watcher::attach(proc.pid, wspec, dbg)?;
    w.set_disarm_deadline(std::time::Duration::from_secs_f64(a.disarm_deadline_ms / 1000.0));
    println!(
        "ARMED    {} thread(s){} (disarm bound {:.0} ms). ⛔ Ctrl-C to stop - NEVER SIGKILL this \
         while it is armed; everything CAUGHT (the 17 signals above) is safe.\n\
         WAITING  up to {:.0}s for ONE trap. --snapshot-on-hit is one arm, one trap, one artifact.",
        w.armed_threads(),
        if a.follow_clone { ", following clones" } else { "" },
        a.disarm_deadline_ms,
        a.hit_timeout
    );
    for f in &w.threads_failed {
        println!("  ⚠ not armed: {f}");
    }
    if w.tracked_threads() != w.armed_threads() {
        println!(
            "  ⛔ {} thread(s) are on the DISARM list without a verified arm - a register may be \
             live on them. They are named in the file's `trap.threads_failed`.",
            w.tracked_threads() - w.armed_threads()
        );
    }

    let plan_ref = &plan;
    let opts_ref = &opts;
    let buf_ref = &mut buf;
    let scratch_ref = &mut scratch;
    let caught = w.catch_one(proc, std::time::Duration::from_secs_f64(a.hit_timeout), move |hit, stop| {
        // ⭐ EVERYTHING IN HERE RUNS WITH NOTHING ARMED. `catch_one` cleared every debug register on
        // every armed thread and read DR7 back as zero before this closure was entered, so a panic
        // in here leaves a tracee the kernel auto-detaches - measured by d1-live's
        // `panic-during-capture-is-survivable` control.
        if !opts_ref.when_stopped {
            return Ok(None);
        }
        Ok(Some(onhit::read_in_stop(
            proc,
            plan_ref,
            opts_ref,
            &hit.regs,
            buf_ref,
            stop.trap_unix_nanos,
            scratch_ref,
        )))
    })?;

    let (Some(hit), Some(stop)) = (caught.hit.clone(), caught.stop.clone()) else {
        println!("\n{}", caught.report.summary());
        println!(
            "⛔ NO SNAPSHOT WAS WRITTEN. A capture with no trap is not an empty artifact - it is a \
             run that did not happen, and writing one would put a `trap: None` file in the archive \
             that looks like a plain capture."
        );
        return Ok(1);
    };
    let in_stop = caught.value.flatten();
    let resumed_ns = caught
        .resumed_unix_nanos
        .unwrap_or(stop.trap_unix_nanos)
        .saturating_sub(stop.trap_unix_nanos);

    println!("\nHIT      tid {} ({}) t+{} ms", hit.tid, hit.comm, hit.elapsed_ms);
    match (hit.writer, &hit.writer_text) {
        (Some(wr), Some(t)) => println!(
            "  writer {wr:#x}{}  {t}   [{}]",
            hit.writer_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default(),
            hit.writer_confidence
        ),
        _ => println!("  writer UNREADABLE - could not decode backwards from RIP"),
    }
    println!(
        "  RIP    {:#x}{}   ⚠ a data breakpoint is a TRAP: this is the instruction AFTER the store",
        hit.rip,
        hit.rip_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default()
    );
    println!(
        "  DISARM {} thread(s) proven clear in {:.3} ms{}",
        stop.disarm.threads.len(),
        stop.disarm.elapsed_ns as f64 / 1e6,
        if stop.disarm.failures.is_empty() {
            String::new()
        } else {
            format!("  ⛔ {} FAILURE(S)", stop.disarm.failures.len())
        }
    );
    for f in &stop.disarm.failures {
        println!("    ⛔ {f}");
        notes.push(format!("⛔ disarm could not be proven on a thread: {f}"));
    }

    // ── the phase split, scored across the resume boundary ──────────────────────────────────────
    let mut phase_split = None;
    if let Some(r) = &in_stop {
        let (ew, ec, dw, dc) = onhit::recheck_phase(proc, &r.phase, &mut scratch);
        let verdict = onhit::score_phase_split(ew, ec, dw, dc);
        println!("PHASE    {verdict}");
        if !verdict.starts_with("PASS") {
            notes.push(format!("⚠ phase-split control: {verdict}"));
        }
        phase_split = Some(PhaseSplitControl {
            seg0_windows: ew,
            seg0_changed: ec,
            seg1_windows: dw,
            seg1_changed: dc,
            verdict,
        });
    }

    // ── assemble the trap record ────────────────────────────────────────────────────────────────
    let (preread, dropped, stop_read_start_ns, stop_read_end_ns, deadline_fired, in_stop_bytes) =
        match &in_stop {
            Some(r) => (
                r.filled
                    .iter()
                    .map(|(ad, off, len, origin)| {
                        (*ad, buf[*off..*off + *len as usize].to_vec(), origin.clone())
                    })
                    .collect::<Vec<_>>(),
                r.dropped
                    .iter()
                    .map(|(ad, len, reason)| Skip {
                        addr: *ad,
                        len: *len,
                        perms: proc.mapping_of(*ad).map(|m| m.perms.clone()).unwrap_or_else(|| "----".into()),
                        rss: 0,
                        // ⚠ A range dropped from the STOP may still be captured after the resume,
                        // because the ordinary selection runs over it in phase 2. That is not a
                        // contradiction: the bytes exist, they are simply a LATER instant. Say so,
                        // or a reader meets a `skipped` row and a region at the same address and
                        // has to guess which one is true.
                        reason: format!(
                            "{reason}. ⚠ NOT the trap's instant - if this range appears in the \
                             region table it was read AFTER the resume (see \
                             `coherence.regions_read_after_resume`)"
                        ),
                    })
                    .collect::<Vec<_>>(),
                r.start_ns,
                r.end_ns,
                r.deadline_fired,
                r.bytes,
            ),
            None => (Vec::new(), Vec::new(), 0, 0, false, 0),
        };
    if deadline_fired {
        notes.push(format!(
            "⛔ THE IN-STOP DEADLINE FIRED at {:.0} ms - {} region(s) were dropped and are in \
             `skipped` with that reason. Those bytes are NOT in this file; they are not zero.",
            opts.budget_ms,
            dropped.len()
        ));
    }
    if !when_stopped {
        notes.push(
            "⚠ --snapshot-when resumed: EVERY byte in this file was read AFTER the thread \
             restarted. The registers are exact; the object a register points at may already have \
             been freed and its memory reused. Do not pair an exact pointer with these bytes and \
             call it 'the moment'."
                .to_string(),
        );
    }
    println!(
        "IN-STOP  {:.2} MiB read in {:.2} ms while the thread was halted; {} region(s) dropped; \
         thread released at t+{:.2} ms",
        in_stop_bytes as f64 / (1u64 << 20) as f64,
        (stop_read_end_ns.saturating_sub(stop_read_start_ns)) as f64 / 1e6,
        dropped.len(),
        resumed_ns as f64 / 1e6
    );

    let trap = TrapCapture {
        method: format!(
            "d1-live DR{} {} {} bytes at {:#x} (v:{:#x}), caught by `d1-replay capture \
             --snapshot-on-hit`",
            a.hit_slot,
            a.hit_rw,
            a.hit_len,
            target,
            target.wrapping_sub(base)
        ),
        watch_addr: target,
        watch_addr_vaddr: Some(target.wrapping_sub(base)),
        watch_len: a.hit_len,
        watch_rw: a.hit_rw.clone(),
        slot: a.hit_slot,
        hit: serde_json::to_value(&hit)?,
        trapped: ThreadRegs {
            tid: hit.tid,
            comm: hit.comm.clone(),
            stopped_at_ns: 0, // it was already stopped: the trap IS its instant
            regs: hit.regs.clone(),
            rip_vaddr: hit.rip_vaddr,
        },
        // ⛔ Empty on purpose: we never ASKED to stop another thread. ⚠ `threads_failed` is NOT
        // empty by construction any more - build 2 hard-coded it to `Vec::new()` with a comment
        // asserting we never asked, which was false: `arm_one` interrupts EVERY thread and
        // `disarm_for_capture` interrupts every thread again, and both kinds of failure went to
        // stdout only. A snapshot from a run that armed 3 of 47 threads was byte-identical to one
        // that armed all 47.
        also_stopped: Vec::new(),
        threads_running: stop.threads_running(),
        threads_total: stop.threads_total,
        threads_total_at_arm: stop.threads_total_at_arm,
        threads_armed: Some(stop.threads_armed_at_hit),
        threads_failed: {
            let mut v = w.threads_failed.clone();
            v.extend(stop.disarm.failures.iter().cloned());
            v
        },
        disarm: serde_json::to_value(&stop.disarm).ok(),
        budget: StopBudget {
            scope: opts.scope.name().to_string(),
            budget_bytes: opts.budget_bytes,
            budget_ms: opts.budget_ms,
            hard_cap_bytes: opts.hard_cap_bytes,
            hard_cap_ms: opts.hard_cap_ms,
            rehearsed_bytes: plan.rehearsed_bytes,
            rehearsed_ms: plan.rehearsed_ms,
        },
        coherence: Coherence {
            trap_unix_nanos: stop.trap_unix_nanos,
            debug_registers_cleared_ns: stop.debug_registers_cleared_ns,
            stop_read_start_ns,
            stop_read_end_ns,
            thread_resumed_ns: resumed_ns,
            resumed_read_end_ns: 0,       // filled by capture_at_trap
            memory_read_while_stopped: false, // ditto - it depends on what lands after the resume
            // ⛔ FALSE, in the file. Anyone who wants to say "atomic" has to find this TRUE, and by
            // default it never is: one thread stopped, the other N kept running.
            all_threads_stopped: false,
            regions_read_after_resume: Vec::new(),
            measured_mib_per_s: if stop_read_end_ns > stop_read_start_ns {
                (in_stop_bytes as f64 / (1u64 << 20) as f64)
                    / ((stop_read_end_ns - stop_read_start_ns) as f64 / 1e9)
            } else {
                0.0
            },
            deadline_fired,
            phase_split,
        },
    };

    let mut all_notes = vec![
        format!(
            "⭐ TRAP-PAIRED CAPTURE. The registers are ONE thread ({} of {}) at ONE instant; the \
             memory is a smear over the milliseconds after it, during which {} other thread(s) kept \
             running. Do not write \"the state at the moment of the write\".",
            hit.tid,
            stop.threads_total.map(|n| n.to_string()).unwrap_or_else(|| "UNKNOWN".into()),
            stop.threads_running().map(|n| n.to_string()).unwrap_or_else(|| "an UNKNOWN number of".into())
        ),
        "⛔ integer registers only - no xmm/FP state was captured, so a float argument is NOT \
         recoverable from this file."
            .to_string(),
    ];
    all_notes.extend(notes);
    all_notes.extend(preset_notes);

    let snap = snapshot::capture_at_trap(
        proc,
        image,
        base,
        base_controls,
        live_state,
        cs,
        spec,
        a.reach.clone(),
        all_notes,
        TrapPhase { preread, dropped, trap },
    )?;
    let bytes = snap.save(&a.out)?;
    println!(
        "\nSNAPSHOT {} - {} region(s), {:.1} MiB captured, {:.1} MiB on disk",
        a.out.display(),
        snap.manifest.regions.len(),
        snap.manifest.total_bytes as f64 / (1u64 << 20) as f64,
        bytes as f64 / (1u64 << 20) as f64
    );
    println!("{}", snap.summary());
    Ok(0)
}

fn mk_ctx<'a>(base: u64, syms: &'a Symbols, proc: &'a Proc) -> Ctx<'a> {
    let mut ctx = Ctx::new(base, syms);
    let f: &'a dyn Fn(u64) -> Result<u64> =
        Box::leak(Box::new(move |a: u64| -> Result<u64> {
            Ok(u64::from_le_bytes(proc.read(a, 8)?.try_into().unwrap()))
        }));
    ctx.mem = Some(f);
    ctx
}

fn snap_ctx<'a>(snap: &'a Snapshot, syms: &'a Symbols) -> Ctx<'a> {
    let mut ctx = Ctx::new(snap.base(), syms);
    let f: &'a dyn Fn(u64) -> Result<u64> = Box::leak(Box::new(move |a: u64| snap.u64(a)));
    ctx.mem = Some(f);
    ctx
}

// ── read ─────────────────────────────────────────────────────────────────────────────────────────

fn cmd_read(a: &ReadArgs, syms: &Symbols) -> Result<i32> {
    let snap = Snapshot::load(&a.snapshot)?;
    let ctx = snap_ctx(&snap, syms);
    let ad = addr::addr(&a.expr, &ctx)?;
    for l in ctx.take_trace() {
        println!("RESOLVE{l}");
    }
    let data = snap.read(ad, a.len as usize)?;
    println!("{} at abs {ad:#x} (v:{:#x})", a.expr, ad.wrapping_sub(snap.base()));
    print!("{}", render(&data, ad, &a.r#as, snap.base()));
    Ok(0)
}

fn render(data: &[u8], addr: u64, how: &str, base: u64) -> String {
    let mut s = String::new();
    match how {
        "u32" => {
            for (i, c) in data.chunks(4).enumerate() {
                if c.len() == 4 {
                    let v = u32::from_le_bytes(c.try_into().unwrap());
                    s.push_str(&format!("  +{:#06x}  {v:#010x}  {v}\n", i * 4));
                }
            }
        }
        "u64" | "ptr" => {
            for (i, c) in data.chunks(8).enumerate() {
                if c.len() == 8 {
                    let v = u64::from_le_bytes(c.try_into().unwrap());
                    let ann = if how == "ptr" && v >= base && v < base + 0x300_0000 {
                        format!("   -> v:{:#x}", v - base)
                    } else {
                        String::new()
                    };
                    s.push_str(&format!("  +{:#06x}  {v:#018x}{ann}\n", i * 8));
                }
            }
        }
        _ => {
            for (i, c) in data.chunks(16).enumerate() {
                let hex: Vec<String> = c.iter().map(|b| format!("{b:02x}")).collect();
                let txt: String =
                    c.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                s.push_str(&format!(
                    "  {:#014x}  {:<47}  {txt}\n",
                    addr + (i * 16) as u64,
                    hex.join(" ")
                ));
            }
        }
    }
    s
}

// ── rederive ─────────────────────────────────────────────────────────────────────────────────────

fn cmd_rederive(a: &RederiveArgs) -> Result<i32> {
    use d1_replay::rederive;
    let snap = Snapshot::load(&a.snapshot)?;
    if a.controls {
        let rep = rederive::run_controls(&snap);
        println!("{}", rep.render());
        return Ok(if rep.ok() { 0 } else { 2 });
    }
    let datum = u32::try_from(a.datum)
        .map_err(|_| anyhow!("--datum {:#x} does not fit a u32 datum id", a.datum))?;
    let read = |ad: u64, len: usize| snap.read(ad, len);
    if datum != rederive::DEFAULT_DATUM {
        eprintln!(
            "⚠ --datum {datum:#010x} is not the canonical record: the known-answer controls \
             cover only 0x27fa2003, so these facts carry inversion-only assurance. The registry \
             walk will resolve ANY 32-bit value to an address; verify the +0xA8 quad names a \
             plausible class and echoes the datum before citing."
        );
    }
    let d = rederive::derive(&read, snap.base(), datum);
    // The facts go to STDOUT, byte-identical to the prototype so the two golden-diff; the
    // provenance goes to STDERR so it is seen without breaking the diff.
    print!("{}", rederive::render(&d));
    eprintln!(
        "\nreach {} - taint propagates; run `d1-replay controls {}` (exit 2 = the snapshot may \
         not be cited) and `d1-replay rederive {} --controls` before citing these facts.",
        snap.manifest.reach,
        a.snapshot.display(),
        a.snapshot.display()
    );
    if let Some(p) = &a.dump_image {
        match &d.image {
            Ok(img) => {
                std::fs::write(p, rederive::render_image_hex(img))
                    .with_context(|| format!("writing {}", p.display()))?;
                eprintln!("image    {} bytes written as hex to {}", img.len(), p.display());
            }
            Err(e) => bail!(
                "--dump-image: the value image is not derivable from this snapshot - {e}. \
                 A partial or zero-filled image would read as a whole one, so nothing was written."
            ),
        }
    }
    if d.complete() {
        Ok(0)
    } else {
        eprintln!(
            "INCOMPLETE: {} fact(s) NOT CAPTURED - a missing range is not a zero. This snapshot \
             cannot source those facts; re-capture with the datum-registry heap in the selection.",
            d.missing()
        );
        Ok(1)
    }
}

// ── replay / search shared setup ─────────────────────────────────────────────────────────────────

struct Prepared {
    spec: RunSpec,
    resolutions: Vec<String>,
}

fn prepare(m: &mut Machine, syms: &Symbols, c: &RunCommon) -> Result<Prepared> {
    let snap = m.snapshot();
    let base = snap.base();
    let mut ctx = Ctx::new(base, syms);
    let f: &dyn Fn(u64) -> Result<u64> = &|a: u64| snap.u64(a);
    // SAFETY-free approach: resolve everything up front while `snap` is borrowed.
    ctx.mem = Some(f);
    ctx.scratch = Some(m.scratch);
    ctx.stop = Some(m.stop);

    let load = |ad: u64, w: Width| -> Result<u64> { Ok(w.decode(&snap.read(ad, w.bytes())?)) };

    let entry = addr::addr(&c.func, &ctx)?;
    let mut spec = RunSpec::new(entry);
    spec.step_limit = c.step_limit;
    for a in &c.args {
        spec.args.push(addr::value(a, &ctx, &load)?);
    }
    for r in &c.regs {
        let (n, v) = r.split_once('=').ok_or_else(|| anyhow!("--reg wants NAME=VALUE, got {r:?}"))?;
        let reg = reg_by_name(n).ok_or_else(|| anyhow!("unknown register {n:?}"))?;
        spec.regs.push((reg, addr::value(v, &ctx, &load)?));
    }
    for w in &c.set_mem {
        let (e, hex) = w.split_once('=').ok_or_else(|| anyhow!("--set-mem wants EXPR=HEX, got {w:?}"))?;
        let ad = addr::addr(e, &ctx)?;
        let bytes = hex_bytes(hex)?;
        spec.writes.push((ad, bytes));
    }
    for s in &c.stubs {
        let (e, v) = s.split_once('=').ok_or_else(|| anyhow!("--stub wants EXPR=RETVAL, got {s:?}"))?;
        spec.stubs.push((addr::addr(e, &ctx)?, addr::value(v, &ctx, &load)?));
    }
    for p in &c.probes {
        spec.probes.push(addr::addr(p, &ctx)?);
    }
    spec.canary = StackCanary::parse(&c.stack_canary, &|e| addr::addr(e, &ctx))?;

    // ⭐ The captured baseline is attached whether or not it is asked for, so a run that did NOT use
    // it can still say that it existed. A caveat you only learn about by passing the flag is a
    // caveat nobody reads.
    spec.captured = snap.manifest.trap.as_ref().map(|t| CapturedRegs {
        label: format!(
            "{} - tid {} ({}), rip {:#x}{}, captured {} ({} of {} thread(s) kept running; \
             memory_read_while_stopped={})",
            t.method,
            t.trapped.tid,
            t.trapped.comm,
            t.trapped.regs.get("rip").copied().unwrap_or_default(),
            t.trapped.rip_vaddr.map(|v| format!(" = v:{v:#x}")).unwrap_or_default(),
            snap.manifest.created_utc,
            t.threads_running.map(|n| n.to_string()).unwrap_or_else(|| "UNKNOWN".into()),
            t.threads_total.map(|n| n.to_string()).unwrap_or_else(|| "UNKNOWN".into()),
            t.coherence.memory_read_while_stopped,
        ),
        regs: t.trapped.regs.clone(),
    });
    spec.use_captured = c.use_captured_regs;
    if spec.use_captured && spec.captured.is_none() {
        bail!(
            "--use-captured-regs: this snapshot carries NO trap, so there are no captured registers \
             to start from.\n  ⚠ `trap: none` means NOT ATTEMPTED - it never means \"the client had \
             no registers\". Every snapshot taken before 2026-08-10, and every snapshot taken \
             without `--snapshot-on-hit`, reads this way.\n  Run `d1-replay info <snapshot>` to see \
             it, or take one with `d1-replay capture --snapshot-on-hit EXPR`.\n  Refusing rather \
             than falling back to invented registers: a flag whose purpose is to stop inventing \
             them must not quietly invent them."
        );
    }

    Ok(Prepared { spec, resolutions: ctx.take_trace() })
}

fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace() && *c != '_').collect();
    if clean.len() % 2 != 0 {
        bail!("hex byte string must have an even number of digits: {s:?}");
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).context("hex"))
        .collect()
}

fn control_block(
    snap: Rc<Snapshot>,
    syms: &Symbols,
    image: &d1_eboot::Image,
    skip: bool,
    path: &Path,
) -> Result<bool> {
    control_block_to(snap, syms, image, skip, false, path)
}

/// `to_stderr` keeps stdout a clean JSON stream without ever making the control block optional - 
/// controls that vanish in one output mode are controls nobody reads in that mode.
fn control_block_to(
    snap: Rc<Snapshot>,
    syms: &Symbols,
    image: &d1_eboot::Image,
    skip: bool,
    to_stderr: bool,
    path: &Path,
) -> Result<bool> {
    if skip {
        let msg = "⛔ CONTROLS SKIPPED (--no-controls) - this output is NOT EVIDENCE.\n";
        if to_stderr {
            eprintln!("{msg}");
        } else {
            println!("{msg}");
        }
        return Ok(false);
    }
    let rep = d1_replay::controls::run(snap, syms, image, Some(path))?;
    if to_stderr {
        eprintln!("{}\n", rep.render());
    } else {
        println!("{}\n", rep.render());
    }
    Ok(rep.ok())
}

// ── replay ───────────────────────────────────────────────────────────────────────────────────────

fn cmd_replay(a: &ReplayArgs, syms: &Symbols, image: &d1_eboot::Image, json: bool) -> Result<i32> {
    let snap = Rc::new(Snapshot::load(&a.snapshot)?);
    let controls_ok = control_block_to(snap.clone(), syms, image, a.no_controls, json, &a.snapshot)?;

    let mut m = Machine::new(snap.clone())?;
    let mut prep = prepare(&mut m, syms, &a.common)?;
    prep.spec.trace = a.trace;
    prep.spec.rollback = false; // a replay's post-state is observable; `--dump` reads it

    {
        let ctx = snap_ctx(&snap, syms);
        for w in &a.watch_reads {
            let (e, len) = split_len(w, 8);
            prep.spec.watch_reads.push((addr::addr(&e, &ctx)?, len));
        }
        for w in &a.watch_writes {
            let (e, len) = split_len(w, 8);
            prep.spec.watch_writes.push((addr::addr(&e, &ctx)?, len));
        }
        for l in ctx.take_trace() {
            prep.resolutions.push(l);
        }
    }

    // In --json mode every human line goes to stderr so stdout is a single parseable document.
    // Nothing is dropped: the provenance a reader needs is not optional just because a machine is
    // reading the other stream.
    let say = |s: String| {
        if json {
            eprintln!("{s}")
        } else {
            println!("{s}")
        }
    };

    say("RESOLUTIONS (every address, in both spaces - check them):".into());
    for l in &prep.resolutions {
        say(l.clone());
    }
    say(format!(
        "  entry  abs {:#x} = v:{:#x}   args {:x?}",
        prep.spec.entry,
        prep.spec.entry.wrapping_sub(snap.base()),
        prep.spec.args
    ));
    say(format!(
        "  scratch {:#x}  stack {:#x}  trampoline {:#x}  canary slot {:#x}\n",
        m.scratch, m.stack, m.stop, m.canary
    ));

    let r = m.run(&prep.spec)?;

    // ⭐ Printed BEFORE the exit, always, in every mode. A run whose memory we altered - or whose
    // entry state came from somewhere other than the caller - must say so before it shows a result,
    // or the reader forms a conclusion first and learns the caveat second.
    say(r.regs_origin.render("").trim_end().to_string());
    say(r.canary.render("").trim_end().to_string());
    // ⭐ And where the MEMORY came from. Printed in both states and in both renderings: `RunReport`
    // carries `trap`, so `--json` has it whether or not anyone read this stream. Until build 3 this
    // subcommand named the trap NOWHERE, and a `replay` transcript from a trap-paired snapshot was
    // silent about the smear it was taken across.
    say(snapshot::render_trap_provenance(r.trap.as_ref(), "TRAP          ", "              ")
        .trim_end()
        .to_string());
    say(String::new());

    if json {
        // `RunReport` carries `canary`, so the alteration is in the JSON whether or not anyone read
        // the stderr block.
        println!("{}", serde_json::to_string_pretty(&r)?);
        if !controls_ok {
            eprintln!("⛔ CONTROLS DID NOT PASS - this output is NOT EVIDENCE.");
            return Ok(2);
        }
        return Ok(0);
    }

    println!("EXIT   {}", r.exit.render());
    println!("RAX    {:#018x}   eax {:#010x}   al {:#04x}", r.rax, r.eax, r.al);
    println!("       ⚠ a bool-returning function answers in AL; RAX's upper bytes are the callee's leftovers");
    println!("STEPS  {}", r.steps);
    for n in &r.notes {
        println!("NOTE   {n}");
    }
    println!("\nREGISTERS ON EXIT:");
    print!("{}", r.final_regs.render("  "));

    if !r.probes.is_empty() {
        println!("\nPROBES:");
        for (ad, d) in &r.probes {
            println!(
                "  {ad:#x} (v:{:#x})  first of {} arrival(s):",
                ad.wrapping_sub(snap.base()),
                r.probe_counts.get(ad).copied().unwrap_or(0)
            );
            print!("{}", d.render("    "));
        }
    }
    if !r.watch_events.is_empty() {
        println!("\nWATCHPOINTS ({} event(s)):", r.watch_events.len());
        for e in &r.watch_events {
            println!(
                "  {:>18} {} {:#x} size {} value {:#x}   from RIP {:#x} (v:{:#x})",
                e.kind,
                if e.kind.contains("WRITE") { "->" } else { "<-" },
                e.addr,
                e.size,
                e.value,
                e.rip,
                e.rip.wrapping_sub(snap.base())
            );
        }
    }
    if !r.trace.is_empty() {
        println!("\nTRACE ({} instruction(s){}):", r.trace.len(), if r.trace_truncated { ", TRUNCATED" } else { "" });
        for t in &r.trace {
            match t.vaddr {
                Some(v) => println!("  {:>6}  v:{v:#010x}  {:<20}  {}", t.index, t.bytes, t.text),
                None => println!("  {:>6}  a:{:#012x}  {:<20}  {}", t.index, t.rip, t.bytes, t.text),
            }
        }
    }
    if !a.dumps.is_empty() {
        println!("\nPOST-STATE:");
        let ctx = snap_ctx(&snap, syms);
        for d in &a.dumps {
            let (e, len) = split_len(d, 0x40);
            let ad = addr::addr(&e, &ctx)?;
            println!("  {e} -> abs {ad:#x}:");
            let bytes = m.read(ad, len as usize)?;
            print!("{}", render(&bytes, ad, "hex", snap.base()));
        }
    }

    if !controls_ok {
        println!("\n⛔ CONTROLS DID NOT PASS - this output is NOT EVIDENCE.");
        return Ok(2);
    }
    Ok(0)
}

// ── search ───────────────────────────────────────────────────────────────────────────────────────

fn cmd_search(a: &SearchArgs, syms: &Symbols, image: &d1_eboot::Image) -> Result<i32> {
    let snap = Rc::new(Snapshot::load(&a.snapshot)?);
    let controls_ok = control_block(snap.clone(), syms, image, a.no_controls, &a.snapshot)?;

    let mut m = Machine::new(snap.clone())?;
    let prep = prepare(&mut m, syms, &a.common)?;

    let ctx = snap_ctx(&snap, syms);
    let mut knobs: Vec<Candidates> = Vec::new();
    for v in &a.vary {
        let (spec, values) = v
            .split_once('=')
            .ok_or_else(|| anyhow!("--vary wants `reg:NAME=VALUES` or `mem:EXPR:WIDTH=VALUES`, got {v:?}"))?;
        let values = search::parse_values(values)?;
        if let Some(name) = spec.strip_prefix("reg:") {
            knobs.push(Candidates { knob: search::reg_knob(name)?, values });
        } else if let Some(rest) = spec.strip_prefix("mem:") {
            let (e, w) = rest
                .rsplit_once(':')
                .ok_or_else(|| anyhow!("--vary mem: wants EXPR:WIDTH, got {rest:?}"))?;
            let width = Width::parse(w).ok_or_else(|| anyhow!("unknown width {w:?}"))?;
            let ad = addr::addr(e, &ctx)?;
            knobs.push(Candidates {
                knob: Knob::Mem { addr: ad, width, label: format!("{w}@{e}") },
                values,
            });
        } else {
            bail!("--vary must start with `reg:` or `mem:`, got {spec:?}");
        }
    }
    let hit = match &a.hit {
        Some(h) => Some(Predicate::parse(h, &|e| addr::addr(e, &ctx))?),
        None => None,
    };

    println!("RESOLUTIONS:");
    for l in &prep.resolutions {
        println!("{l}");
    }
    for l in ctx.take_trace() {
        println!("RESOLVE{l}");
    }
    println!(
        "  entry  abs {:#x} = v:{:#x}   args {:x?}",
        prep.spec.entry,
        prep.spec.entry.wrapping_sub(snap.base()),
        prep.spec.args
    );
    // ⭐ The same plan `run()` will apply to every candidate, printed once from the one function
    // that computes it - a preview that could disagree with the action would be worse than none.
    // Both of them: what this search does to memory, and where its entry registers come from.
    // ⚠ A `reg:` knob is applied per candidate, INSIDE `search::search`, so the planner cannot see
    // it - and without `claimed_by` the preview would print the captured value for a register every
    // candidate is about to overwrite.
    let mut ro = machine::plan_register_origin(&prep.spec, m.base);
    for k in &knobs {
        if let Knob::Reg { name, .. } = &k.knob {
            ro.claimed_by(name, &format!("--vary reg:{name} ({} candidate value(s))", k.values.len()));
        }
    }
    print!("{}", ro.render(""));
    print!("{}", m.canary_plan(prep.spec.canary).render(""));
    // ⭐ And the memory's provenance, in both states - `search` was silent about the trap for the
    // same reason `replay` was, and a search transcript is quoted as evidence just as often.
    print!(
        "{}",
        snapshot::render_trap_provenance(
            snapshot::TrapProvenance::of(snap.manifest.trap.as_ref()).as_ref(),
            "TRAP          ",
            "              "
        )
    );
    let total: usize = knobs.iter().map(|k| k.values.len()).product();
    println!(
        "\nSEARCH {} candidate(s) over {}; hit = {}\n",
        total,
        knobs
            .iter()
            .map(|k| format!("{} ({} values)", k.knob.label(), k.values.len()))
            .collect::<Vec<_>>()
            .join(" × "),
        hit.as_ref().map(|h| h.describe()).unwrap_or_else(|| "<none given - reporting all>".into())
    );

    let res = search::search(&mut m, &prep.spec, &knobs, hit.as_ref(), a.verify_every)?;

    println!("{:<24} {:>18} {:>4} {:>8}  {}", res.knobs.join(" / "), "rax", "al", "steps", "exit");
    for row in &res.rows {
        if !a.all && hit.is_some() && !row.hit {
            continue;
        }
        println!(
            "{:<24} {:>#18x} {:>#4x} {:>8}  {} {}",
            row.values.iter().map(|v| format!("{v:#x}")).collect::<Vec<_>>().join(","),
            row.rax,
            row.al,
            row.steps,
            row.exit,
            if row.hit { "  ⭐ HIT" } else { "" }
        );
        if row.hit {
            for (ad, d) in &row.probes {
                println!("    probe {ad:#x} (v:{:#x}):", ad.wrapping_sub(snap.base()));
                print!("{}", d.render("      "));
            }
        }
    }

    println!(
        "\nSEARCH RESULT: {}/{} candidate(s) hit{}",
        res.hits.len(),
        res.rows.len(),
        if res.hits.is_empty() && hit.is_some() {
            ".  ⚠ An EMPTY result over an exhaustive candidate set is the STRONGER finding: no \
             input would have made this hit, so the state, not the key, is what is wrong."
        } else {
            ""
        }
    );
    println!("ROLLBACK CONTROL:");
    for l in &res.pristine {
        println!("  {l}");
    }
    // ⚠ CONDITIONAL, because the blanket sentence became false the day a snapshot could carry a
    // trap. A caveat that is wrong for one kind of artifact teaches readers to skip it on both.
    if prep.spec.use_captured {
        println!(
            "\n⚠ This says: given the memory the client actually had at the snapshot instant, and \
             the client's OWN integer registers as the baseline, these inputs make the function \
             behave this way. The varied knobs are still OURS: a hit names an input that works, not \
             the input the client used - only the UNVARIED registers above are the client's. And \
             the baseline is integer-only; no xmm was captured."
        );
    } else {
        println!(
            "\n⚠ This says: given the memory the client actually had at the snapshot instant, these \
             inputs make the function behave this way. It does NOT say the client's input was any \
             of them - the entry registers are INVENTED (this snapshot carries {}). "
        , if prep.spec.captured.is_some() {
            "a trap, so --use-captured-regs would start from the client's own registers instead"
        } else {
            "no trap: registers were NOT ATTEMPTED, which is not \"the client had none\""
        });
    }

    if !controls_ok {
        println!("\n⛔ CONTROLS DID NOT PASS - this output is NOT EVIDENCE.");
        return Ok(2);
    }
    Ok(0)
}

// ── preset-control ───────────────────────────────────────────────────────────────────────────────
//
// ⭐ THE AXIS, IN WORDS: **the content preset captures the regions a content lookup actually reads.**
//
// It is measured, not asserted. The same function is replayed against two snapshots of the same
// client that differ by one flag, and the WITH run must get strictly further - more instructions
// retired, or a clean return where the WITHOUT run faulted. That is the only claim a snapshot can
// support here: a fault is a RESULT (a page we did not capture), and "further" is the observable.
//
// Three ways it refuses to be decoration:
//   * both snapshots must pass the full control block, or the comparison is INDETERMINATE and FAILs;
//   * the `--with` snapshot must DECLARE the preset and the `--without` snapshot must not - a pair
//     that differs by nothing would otherwise "pass" by tying;
//   * `--invert` asserts the opposite, and swapping `--with`/`--without` is a second, independent
//     inversion. Both must FAIL on a good pair.

struct OneRun {
    steps: u64,
    exit: String,
    ok: bool,
    controls_ok: bool,
    declares: bool,
}

fn preset_run(
    path: &PathBuf,
    a: &PresetControlArgs,
    syms: &Symbols,
    image: &d1_eboot::Image,
) -> Result<OneRun> {
    let snap = Rc::new(Snapshot::load(path)?);
    let declares = snap.manifest.selection.presets.iter().any(|p| p == &a.preset);
    let rep = d1_replay::controls::run(snap.clone(), syms, image, Some(path.as_path()))?;
    let controls_ok = rep.ok();

    let mut m = Machine::new(snap.clone())?;
    let common = RunCommon {
        func: a.func.clone(),
        args: if a.args.is_empty() { vec!["scratch".into()] } else { a.args.clone() },
        regs: Vec::new(),
        // ⚠ INVENTED, deliberately. The preset control's axis is "the WITH snapshot captured the
        // pages this lookup reads", and it compares two snapshots taken at different instants. A
        // captured baseline would differ between them, so the register file would become a second
        // uncontrolled variable in a two-arm comparison that has exactly one.
        use_captured_regs: false,
        // ⭐ MERGE NOTE (2026-08-10). This used to default to
        // `--set-mem v:0x1a06c78=0000000008000000` - a hand-written stack-canary redirect, written
        // before `--stack-canary` existed. It is now the tool-wide `auto` default instead, for two
        // reasons: `auto` refuses to touch a pointer that is already mapped (the `--set-mem` form
        // could not tell), and it reports what it did. Keeping both would have made the explicit
        // write override the redirect and fire the "redirect DID NOT LAND" note on every run.
        // ⚠ Without one or the other, BOTH arms die in the prologue at 8 steps, `improved` is
        // always false, and this control can never pass. That coupling is why the canary landed
        // first. `--set-mem` still overrides, deliberately.
        set_mem: a.set_mem.clone(),
        stubs: Vec::new(),
        probes: Vec::new(),
        step_limit: a.step_limit,
        stack_canary: a.stack_canary.clone(),
    };
    let mut prep = prepare(&mut m, syms, &common)?;
    prep.spec.rollback = false;
    let r = m.run(&prep.spec)?;
    Ok(OneRun {
        steps: r.steps,
        exit: r.exit.render(),
        ok: r.exit.ok(),
        controls_ok,
        declares,
    })
}

fn cmd_preset_control(
    a: &PresetControlArgs,
    syms: &Symbols,
    image: &d1_eboot::Image,
) -> Result<i32> {
    println!("CONTROL  content-preset-coverage");
    println!(
        "  AXIS: the `{}` preset captures the regions a content lookup actually reads.",
        a.preset
    );
    println!("  METHOD: replay {} against two snapshots that differ by one flag; the WITH run must", a.func);
    println!("          retire strictly more instructions, or return cleanly where WITHOUT faulted.");
    if a.invert {
        println!("  ⚠ INVERTED (--invert): this run asserts the preset does NOT help. On a good pair it MUST FAIL.");
    }
    println!();

    let w0 = preset_run(&a.without, a, syms, image)?;
    let w1 = preset_run(&a.with_, a, syms, image)?;

    println!("  WITHOUT  {}", a.without.display());
    println!("           declares preset {:<5}  controls {}", w0.declares, if w0.controls_ok { "PASS" } else { "FAIL" });
    println!("           steps {:<8} exit {}", w0.steps, w0.exit);
    println!("  WITH     {}", a.with_.display());
    println!("           declares preset {:<5}  controls {}", w1.declares, if w1.controls_ok { "PASS" } else { "FAIL" });
    println!("           steps {:<8} exit {}", w1.steps, w1.exit);
    println!();

    let mut refusals: Vec<String> = Vec::new();
    if !w0.controls_ok || !w1.controls_ok {
        refusals.push(
            "a snapshot failed its own control block - the comparison is INDETERMINATE, not a result"
                .into(),
        );
    }
    if !w1.declares {
        refusals.push(format!(
            "the --with snapshot does not declare `--preset {}`; the axis would be untested and a \
             tie would read as a pass",
            a.preset
        ));
    }
    if w0.declares {
        refusals.push(format!(
            "the --without snapshot ALSO declares `--preset {}` - this pair does not differ by the \
             thing being measured",
            a.preset
        ));
    }

    let improved = w1.steps > w0.steps || (w1.ok && !w0.ok);
    let expected = !a.invert;
    let pass = refusals.is_empty() && improved == expected;

    println!(
        "  RESULT: WITH retired {} instruction(s) {} WITHOUT ({} vs {}){}",
        w1.steps.abs_diff(w0.steps),
        if w1.steps > w0.steps { "MORE than" } else if w1.steps < w0.steps { "FEWER than" } else { " - the same as" },
        w1.steps,
        w0.steps,
        if w1.ok && !w0.ok { ", and returned cleanly where WITHOUT faulted" } else { "" }
    );
    for r in &refusals {
        println!("  ⛔ REFUSED: {r}");
    }
    println!(
        "\n  {}  content-preset-coverage{}",
        if pass { "PASS" } else { "FAIL" },
        if a.invert { " (inverted)" } else { "" }
    );
    if !pass {
        println!("  ⛔ NOT EVIDENCE - do not cite a preset capture whose coverage control failed.");
    }
    Ok(if pass { 0 } else { 2 })
}
