// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-live` - hardware watchpoints on the live client.
//!
//! ```text
//! d1-live controls                                   # 10 controls, offline, exit 2 on failure
//! d1-live base                                       # the three base controls against the client
//! d1-live watch --addr 0x26B1850 --len 8             # an eboot vaddr (base is added)
//! d1-live watch --addr 0x204eda2f8 --abs --len 8     # a heap address (used as-is)
//! d1-live watch --addr '[v:0x26B1850]+0x8' --len 8   # deref, then a plain offset
//! ```

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use d1_live::addr::{self, Ctx, Width};
use d1_live::format;
use d1_live::proc::{self as live, Proc};
use d1_live::ptrace::{self, Rw};
use d1_live::registry;
use d1_live::session::{self, Session};
use d1_live::symbols::{self, Symbols};
use d1_live::watch::{WatchSpec, Watcher};

#[derive(Parser)]
#[command(
    name = "d1-live",
    about = "The live client as a Linux process: hardware (debug-register) watchpoints that name \
             the instruction which wrote a field",
    long_about = None
)]
struct Cli {
    /// The eboot to translate RIPs against. Defaults to the pinned image / $D1_EBOOT.
    #[arg(long, global = true)]
    eboot: Option<PathBuf>,
    /// Emit JSON instead of the human report.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// ⭐ Watch an address and report the instruction that touched it.
    Watch(WatchArgs),
    /// Recover the guest base and print the three base controls.
    Base(BaseArgs),
    /// Print the liveness ladder, L0..L3.
    Live(ReadSide),
    /// List `guest-symbols.json` symbols, with source and confidence.
    Syms {
        pattern: Option<String>,
        #[arg(long)]
        symbols: Option<PathBuf>,
    },
    /// ⭐ Read a guest vaddr, a symbol, or an address expression.
    Read(ReadArgs),
    /// A server-object registry element, by `FAMILY:TYPE[:INDEX]` or by `--key`.
    Obj(ObjArgs),
    /// Resolve a `0x8080xxxx` schema/content tag through the PACKAGE registry.
    Tag(TagArgs),
    /// The QueueZ replication descriptor table.
    Descriptors(DescArgs),
    /// (internal) a synthetic guest address space, so the read side has controls that can FAIL.
    #[command(hide = true, name = "__mem-target")]
    MemTarget {
        #[arg(long, default_value_t = 20.0)]
        hold: f64,
        /// none | family-stride | verify-window | duplicate-anchor
        #[arg(long, default_value = "none")]
        perturb: String,
        #[arg(long)]
        eboot: Option<PathBuf>,
        /// ⚠ Ignored argv padding. The golden diff has to hand the SAME target to `d1guest.py`, and
        /// that script has no `--pid`: it finds its client with `pgrep -f "[s]hadps4 -p "`. Passing
        /// those words here is how a fixture becomes discoverable by the instrument under
        /// comparison. The fixture refuses to start while a real emulator is running, so this can
        /// never add a second candidate to a live run.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        decoy_argv: Vec<String>,
    },
    /// Run every positive control. **Offline** - never touches the emulator. Exits 2 on any FAIL.
    Controls,
    /// (internal) the control child: store to a known cell in a loop.
    #[command(hide = true, name = "__writer")]
    Writer {
        #[arg(long, default_value_t = 20.0)]
        seconds: f64,
        /// Extra spinner threads, so a control can exercise the MULTI-THREAD disarm rather than
        /// the one-thread case the tool will never actually meet on shadPS4.
        #[arg(long, default_value_t = 0)]
        threads: usize,
    },
    /// (internal) arm through a real `Watcher` and then WAIT to be signalled - the signal-safety
    /// probe. With `--no-handlers` it is the inversion: the signal kills the tracer outright.
    #[command(hide = true, name = "__hold-armed")]
    HoldArmed {
        #[arg(long)]
        pid: i32,
        #[arg(long, value_parser = parse_u64)]
        addr: u64,
        #[arg(long, value_parser = parse_u64)]
        dbg: u64,
        /// ⛔ The inversion: do NOT install the handlers, so the signal's default action applies.
        #[arg(long)]
        no_handlers: bool,
    },
    /// (internal) arm a watchpoint and exit without disarming - the tracer-death safety probe.
    #[command(hide = true, name = "__arm-and-die")]
    ArmAndDie {
        #[arg(long)]
        pid: i32,
        #[arg(long, value_parser = parse_u64)]
        addr: u64,
        #[arg(long, value_parser = parse_u64)]
        dbg: u64,
    },
    /// (internal) catch a trap, clear every debug register, then PANIC in the middle of the
    /// capture - the crash-during-capture safety probe.
    #[command(hide = true, name = "__capture-panic")]
    CapturePanic {
        #[arg(long)]
        pid: i32,
        #[arg(long, value_parser = parse_u64)]
        addr: u64,
        #[arg(long, value_parser = parse_u64)]
        dbg: u64,
        /// ⛔ The inversion: panic while STILL ARMED. On this kernel that kills the child.
        #[arg(long)]
        armed: bool,
    },
}

#[derive(Args)]
struct BaseArgs {
    #[arg(long)]
    pid: Option<i32>,
    /// Scan every readable mapping for the anchors, not just executable ones.
    #[arg(long)]
    full_scan: bool,
}

/// The flags `d1guest.py` puts on its top-level parser, carried verbatim.
#[derive(Args, Clone, Debug)]
struct ReadSide {
    /// Which process. Defaults to the single `pgrep -f "[s]hadps4 -p "` match; zero matches is an
    /// ERROR and two is a refusal to guess.
    #[arg(long)]
    pid: Option<i32>,
    #[arg(long)]
    symbols: Option<PathBuf>,
    /// ⚠ Read every vaddr in the symbol file as suspect if the eboot differs. This flag says you
    /// re-verified them yourself.
    #[arg(long)]
    ignore_build_gate: bool,
    /// Scan ALL resident readable mappings for the anchors, not just executable ones (slower; use
    /// if the anchor control fails).
    #[arg(long)]
    full_scan: bool,
    /// Re-scan for the base even if it is cached for this pid.
    #[arg(long)]
    no_cache: bool,
    /// ⚠ Read even though the liveness predicate failed. The result is then indistinguishable from
    /// "delivery never worked" - label it.
    #[arg(long)]
    allow_dead: bool,
    /// Override the liveness account SOID (default: $D1_ACCOUNT_SOID or the symbol file).
    #[arg(long)]
    account_soid: Option<String>,
}

impl ReadSide {
    fn opts(&self, eboot: Option<PathBuf>) -> d1_live::session::Opts {
        d1_live::session::Opts {
            pid: self.pid,
            eboot,
            symbols: self.symbols.clone(),
            ignore_build_gate: self.ignore_build_gate,
            full_scan: self.full_scan,
            no_cache: self.no_cache,
            allow_dead: self.allow_dead,
            // ⚠ The env fallback is `d1guest.py`'s, and it is deliberately BELOW the flag: a run
            // that exported $D1_ACCOUNT_SOID for one experiment must not silently override an
            // explicit --account-soid on the next command.
            account_soid: self
                .account_soid
                .clone()
                .or_else(|| std::env::var("D1_ACCOUNT_SOID").ok().filter(|v| !v.is_empty())),
            // ⛔ Never quiet on the CLI: the provenance IS the output.
            quiet: false,
        }
    }
}

#[derive(Args)]
struct ReadArgs {
    /// A `guest-symbols.json` name, an eboot vaddr, or an address expression (`[v:0x…]+0x8`).
    target: String,
    #[arg(long, value_parser = parse_u64)]
    len: Option<u64>,
    /// Read the qword there, then dump AT it.
    #[arg(long)]
    deref: bool,
    /// ⚠ The target is already a HOST address. A heap address needs this.
    #[arg(long)]
    abs: bool,
    /// Decode with a `structs[]` entry from the symbol file.
    #[arg(long = "struct", value_name = "NAME")]
    strukt: Option<String>,
    /// hex (default), u32, u64, ptr.
    ///
    /// ⛔ The value list is ENFORCED, as `argparse`'s `choices=` enforces it in `d1guest.py`.
    /// Without it `--as float` printed the provenance, the liveness ladder and the
    /// "guest … -> host …, 64 bytes" header, then **no data at all, at exit 0** - a successful read
    /// with an empty result, which is the shape this project keeps mistaking for a finding.
    #[arg(long = "as", value_name = "VIEW", value_parser = ["hex", "u32", "u64", "ptr"])]
    as_: Option<String>,
    /// Minimum liveness level for this read (default 2 = registry built).
    #[arg(long, default_value_t = 2)]
    require: u8,
    #[command(flatten)]
    common: ReadSide,
}

#[derive(Args)]
struct ObjArgs {
    /// `FAMILY:TYPE[:INDEX]`.
    spec: String,
    /// Resolve by KEY instead of index. ⭐ An index is a guess; a key is an identity.
    #[arg(long)]
    key: Option<String>,
    #[arg(long, value_parser = parse_u64)]
    len: Option<u64>,
    #[arg(long = "struct", value_name = "NAME")]
    strukt: Option<String>,
    #[command(flatten)]
    common: ReadSide,
}

#[derive(Args)]
struct TagArgs {
    #[arg(required = true)]
    tags: Vec<String>,
    #[arg(long, value_parser = parse_u64)]
    len: Option<u64>,
    #[command(flatten)]
    common: ReadSide,
}

#[derive(Args)]
struct DescArgs {
    /// Poll, printing only when the table changes.
    #[arg(long)]
    watch: bool,
    #[command(flatten)]
    common: ReadSide,
}

#[derive(Args)]
struct WatchArgs {
    /// Which process. Defaults to the single `pgrep -f "[s]hadps4 -p "` match.
    #[arg(long)]
    pid: Option<i32>,
    /// The address to watch. A bare number or a `guest-symbols.json` name is an **eboot vaddr**;
    /// `a:0x…` (or `--abs`) is absolute; `[…]` dereferences through the live process.
    #[arg(long)]
    addr: String,
    /// ⚠ d1guest.py parity: the address is already a host address. **A heap address needs this.**
    #[arg(long)]
    abs: bool,
    /// 1, 2, 4 or 8. The address must be naturally aligned to it.
    #[arg(long, default_value_t = 8)]
    len: u64,
    /// `write` (data writes), `rw` (reads or writes), `exec` (a hardware breakpoint; --len 1).
    #[arg(long, default_value = "write")]
    rw: String,
    /// Which debug register, 0..3. Use different slots to watch several addresses at once.
    #[arg(long, default_value_t = 0)]
    slot: usize,
    /// Stop after this many hits.
    #[arg(long, default_value_t = 1)]
    count: usize,
    /// Self-disarm after this long, whatever happened. A watch that is forgotten is a watch that
    /// eventually meets a `SIGKILL`.
    #[arg(long, default_value_t = 120.0)]
    max_seconds: f64,
    /// Also arm threads created *after* attach. Off by default: it is more perturbation, and it
    /// changes the target's clone path.
    #[arg(long)]
    follow_clone: bool,
    /// Arm only these threads (repeatable). ⚠ Then "no hit" is evidence about these threads only.
    #[arg(long = "tid")]
    tids: Vec<i32>,
    /// Print the trapping thread's WHOLE integer register file, not just the four argument
    /// registers. ⚠ Integer only - no xmm/FP state is captured. `--json` always carries all of it.
    #[arg(long)]
    regs: bool,
    /// ⛔ ACCEPTED AND REFUSED. The paired capture is `d1-replay capture --snapshot-on-hit`; this
    /// flag exists so the answer is a message that names it, not "unexpected argument" - which
    /// reads as *the feature does not exist* and is how this project keeps re-implementing things
    /// it already has.
    #[arg(long = "snapshot-on-hit", value_name = "EXPR")]
    snapshot_on_hit: Option<String>,
    /// Skip base recovery. Required when the target is not the pinned client; every address must
    /// then be absolute and no RIP is translated to a vaddr.
    #[arg(long)]
    no_base: bool,
    #[arg(long)]
    full_scan: bool,
}

fn parse_u64(s: &str) -> Result<u64, String> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|e| e.to_string())
    } else {
        t.parse().map_err(|e: std::num::ParseIntError| e.to_string())
    }
}

fn pick_pid(explicit: Option<i32>) -> Result<i32> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let pids = live::emulator_pids()?;
    match pids.len() {
        1 => Ok(pids[0]),
        0 => bail!(
            "no shadPS4 process found (`pgrep -f \"[s]hadps4 -p \"`). Pass --pid, or start the run \
             first. ⚠ `pgrep -x shadps4` matches nothing - the comm is `shadPS4:Main`."
        ),
        n => bail!("{n} shadPS4 processes are running ({pids:?}) - say which with --pid"),
    }
}

fn main() {
    let code = match real_main() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("d1-live: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn real_main() -> Result<i32> {
    let cli = Cli::parse();
    match &cli.cmd {
        Cmd::Writer { seconds, threads } => d1_live::controls::writer_main(*seconds, *threads),
        Cmd::HoldArmed { pid, addr, dbg, no_handlers } => {
            d1_live::controls::hold_armed_main(*pid, *addr, *dbg, !*no_handlers)?;
            Ok(0)
        }
        Cmd::ArmAndDie { pid, addr, dbg } => {
            d1_live::controls::arm_and_die_main(*pid, *addr, *dbg)?;
            Ok(0)
        }
        Cmd::CapturePanic { pid, addr, dbg, armed } => {
            d1_live::controls::capture_panic_main(*pid, *addr, *dbg, *armed)?;
            Ok(0)
        }
        Cmd::Controls => {
            let helper = std::env::current_exe().context("locating this binary")?;
            let c = d1_live::controls::run_all(&helper);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&c)?);
            } else {
                println!("{}", c.report());
            }
            // ⛔ 2, not 1: the project's convention for "a control failed, do not cite the output".
            Ok(if c.ok() { 0 } else { 2 })
        }
        Cmd::Base(a) => {
            let pid = pick_pid(a.pid)?;
            let proc = Proc::open(pid)?;
            let image = live::open_image(cli.eboot.clone())?;
            let syms = Symbols::open_default()?;
            let ctl = live::recover_base(&proc, &image, &syms, a.full_scan);
            println!("{}", ctl.report());
            Ok(if ctl.ok() { 0 } else { 2 })
        }
        Cmd::Watch(a) => cmd_watch(&cli, a),
        Cmd::MemTarget { hold, perturb, eboot, decoy_argv } => {
            let _ = decoy_argv;
            d1_live::fixture::mem_target_main(
                *hold,
                d1_live::fixture::Perturb::parse(perturb)?,
                eboot.clone().or_else(|| cli.eboot.clone()),
            )
        }
        Cmd::Syms { pattern, symbols } => cmd_syms(pattern.as_deref(), symbols.clone()),
        Cmd::Live(c) => read_side(|| {
            let s = Session::open(&c.opts(cli.eboot.clone()))?;
            let (_l, text, refusal) = s.liveness(0);
            println!("{text}");
            if let Some(e) = refusal {
                return Err(e);
            }
            Ok(0)
        }),
        Cmd::Read(a) => read_side(|| cmd_read(&cli, a)),
        Cmd::Obj(a) => read_side(|| cmd_obj(&cli, a)),
        Cmd::Tag(a) => read_side(|| cmd_tag(&cli, a)),
        Cmd::Descriptors(a) => read_side(|| cmd_descriptors(&cli, a)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The read side - `capture-rig/d1guest.py` parity
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Exit codes, kept identical to `d1guest.py` because scripts and skills branch on them:
///
/// * **0** the read happened · **1** an ordinary error · **2** a CONTROL or a gate refused, so the
///   output may not be cited · **3** the client DIED, which is an error and never a zero.
///
/// ⚠ 2 and 3 are different facts. Collapsing them is how "the field was zero" gets published when
/// what happened was "the client went away".
fn read_side(f: impl FnOnce() -> Result<i32>) -> Result<i32> {
    match f() {
        Ok(c) => Ok(c),
        Err(e) => {
            let code = session::exit_code(&e);
            eprintln!("{}{e:#}", if code == 3 { "DEAD: " } else { "d1-live: " });
            Ok(code)
        }
    }
}

fn cmd_syms(pattern: Option<&str>, path: Option<PathBuf>) -> Result<i32> {
    let syms = match path {
        Some(p) => Symbols::open(p)?,
        None => Symbols::open_default()?,
    };
    println!("{:<32} {:>12} {:<7} {:<7} conf  source", "name", "vaddr", "type", "reg");
    for s in &syms.symbols {
        if let Some(p) = pattern {
            if !s.name.contains(p) {
                continue;
            }
        }
        println!(
            "{:<32} {:>#12x} {:<7} {:<7} {}  {}",
            s.name, s.vaddr, s.kind, s.region, s.confidence, s.source
        );
        if !s.note.is_empty() {
            println!("{:<32} {}", "", s.note);
        }
    }
    if pattern.is_none() {
        // ⚠ FILE order for the structs (Python dicts keep insertion order) and NUMERIC order for
        // the families (`sorted()` over `(family, type)` tuples). Sorting either as strings makes
        // the two tools disagree - the families line silently, once a family number reaches 10.
        println!("\nstructs: {}", syms.struct_order.join(", "));
        let mut fams: Vec<&symbols::FamilySlot> = syms.families.iter().collect();
        fams.sort_by_key(|f| (f.family, f.typ));
        println!(
            "families: {}",
            fams.iter()
                .map(|f| format!(
                    "{}/{}={}",
                    f.family,
                    f.typ,
                    if f.identity.is_empty() { "-" } else { &f.identity }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(0)
}

fn cmd_read(cli: &Cli, a: &ReadArgs) -> Result<i32> {
    let s = Session::open(&a.common.opts(cli.eboot.clone()))?;

    // ── Resolve, and SAY how it resolved ────────────────────────────────────────────────────────
    //
    // Two languages meet here and both must keep working. `d1guest.py` accepts a symbol name or a
    // number; this crate's `addr` grammar adds `v:`/`a:`/`[…]`/arithmetic. A plain name or number
    // takes the Python path exactly, so a dump from either tool is the same dump; anything with an
    // expression character takes the grammar. Neither ever guesses which space a number is in.
    let uses_expr = a.target.contains(['[', '+', ':', ']']);
    let (vaddr, sym) = if uses_expr {
        (None, None)
    } else {
        let r = s.syms.resolve(&a.target)?;
        (Some(r.vaddr()), r.sym().cloned())
    };
    if let Some(sym) = &sym {
        println!(
            "symbol {} @ {:#x}  {}  region={}  {}",
            sym.name,
            sym.vaddr,
            sym.confidence,
            if sym.region.is_empty() { "?" } else { &sym.region },
            sym.source
        );
        if !sym.note.is_empty() {
            println!("  note: {}", sym.note);
        }
    }

    let (_lv, ltext, refusal) = s.liveness(a.require);
    println!("{ltext}");
    if let Some(e) = refusal {
        return Err(e);
    }

    // ⚠ `args.len or (sym len) or 64` in the Python: a `--len 0` falls THROUGH to the default
    // rather than reading zero bytes. Reproduced, so the two tools cannot disagree on a corner.
    let n = a
        .len
        .filter(|v| *v != 0)
        .map(|v| v as usize)
        .or_else(|| sym.as_ref().and_then(|s| s.len))
        .unwrap_or(64);
    let struct_name = a.strukt.clone().or_else(|| sym.as_ref().and_then(|s| s.strukt.clone()));
    let mut n = n;
    // ⚠ `--len 0` must be treated as ABSENT here too, exactly as it is three lines up. The Python's
    // test is `not args.len`, which 0 satisfies; matching on `None` alone made
    // `--len 0 --struct signon_manager_state` read 64 bytes where `d1guest.py` reads 592.
    if let (Some(sn), None) = (&struct_name, a.len.filter(|v| *v != 0)) {
        if let Some(sd) = s.syms.structs.get(sn) {
            n = n.max(sd.size);
        }
    }

    let mut addr = match vaddr {
        Some(v) if a.abs => v,
        Some(v) => s.base + v,
        None => {
            let mut ctx = Ctx::new(s.base, &s.syms);
            let mem = |ad: u64| -> Result<u64> {
                Ok(u64::from_le_bytes(s.proc.read(ad, 8)?.try_into().unwrap()))
            };
            ctx.mem = Some(&mem);
            let v = addr::addr(&a.target, &ctx)?;
            for l in ctx.take_trace() {
                println!("{l}");
            }
            v
        }
    };

    let mut label = None;
    if a.deref {
        let ptr = u64::from_le_bytes(s.proc.read(addr, 8)?.try_into().unwrap());
        println!("[{}] = {ptr:#018x}", a.target);
        if ptr == 0 {
            println!(
                "  NULL - nothing to dump. ⚠ For a heap object this may mean the client has not \
                 built it yet OR that it has been torn down; check LIVENESS above."
            );
            return Ok(0);
        }
        addr = ptr;
        label = Some("*");
    }

    let data = s.proc.read(addr, n)?;
    let where_ = vaddr.map(|v| registry::region_of(&s.image, v));
    match vaddr {
        Some(v) => println!(
            "guest {v:#x} -> host {addr:#x}, {n} bytes{}",
            match (where_, a.deref) {
                (Some(registry::Region::Outside), _) | (None, _) => String::new(),
                (Some(r), false) => format!("   [image region: {}]", r.as_str()),
                (_, true) => String::new(),
            }
        ),
        None => println!("host {addr:#x}, {n} bytes"),
    }

    let view = a.as_.as_deref();
    if view.is_none() || view == Some("hex") {
        println!("{}", format::hexdump(&data, addr, label));
    }
    if matches!(view, Some("u32") | Some("u64") | Some("ptr")) {
        println!("{}", format::as_words(&data, view.unwrap(), s.base));
    }
    if let Some(sn) = &struct_name {
        let sd = s
            .syms
            .structs
            .get(sn)
            .ok_or_else(|| anyhow::anyhow!("unknown struct {sn:?} - `d1-live syms` lists them"))?;
        println!("{}", format::show_struct(sd, &data));
    }

    // If the range is file-backed, diffing against the file is free and is often the whole point:
    // PS4 eboots keep pointer tables ZERO in the file and fill them at load (ATLAS §4.2), so
    // `boot_state_names` reads all-zero statically and full of pointers live.
    if where_ == Some(registry::Region::File) && !a.deref {
        if let (Some(v), Ok(fdata)) = (vaddr, s.image.bytes(vaddr.unwrap_or(0), n)) {
            let _ = v;
            let same = fdata == data.as_slice();
            let diff = fdata.iter().zip(data.iter()).filter(|(x, y)| x != y).count();
            println!(
                "  vs FILE image: {}{}",
                if same { "IDENTICAL" } else { "DIFFERS" },
                if same {
                    String::new()
                } else {
                    format!(" in {diff}/{n} bytes - relocations, patches or runtime writes")
                }
            );
        }
    }
    Ok(0)
}

fn cmd_obj(cli: &Cli, a: &ObjArgs) -> Result<i32> {
    let s = Session::open(&a.common.opts(cli.eboot.clone()))?;
    // ⚠ L3, not L2: this command reads registry object CONTENTS, and objects are cleared on
    // connection teardown while descriptors survive (ATLAS §4.6b).
    let (_lv, ltext, refusal) = s.liveness(3);
    println!("{ltext}");
    if let Some(e) = refusal {
        return Err(e);
    }
    let g = s.guest();

    let parts: Vec<&str> = a.spec.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        bail!("spec must be FAMILY:TYPE[:INDEX], not {:?}", a.spec);
    }
    let fam = session::parse_u64(parts[0])?;
    let typ = session::parse_u64(parts[1])?;
    let idx = if parts.len() > 2 { session::parse_u64(parts[2])? } else { 0 };

    let addr = match &a.key {
        Some(k) => {
            let key = session::parse_u64(k)?;
            let (found, lines) = g.find_by_key(fam, typ, key)?;
            println!("REGISTRY CONTROL:");
            for l in &lines {
                println!("{l}");
            }
            let Some((addr, at)) = found else {
                // Exit 2: the REGISTRY CONTROL lines above already said what was searched.
                return Err(session::control_refusal(format!(
                    "no element of family {fam}/{typ} carries key {key:#018x}"
                )));
            };
            println!("family {fam}/{typ} key {key:#018x} found at index {at}");
            addr
        }
        None => {
            let fr = g.family_slot(fam, typ)?;
            println!("REGISTRY CONTROL:");
            for l in &fr.lines {
                println!("{l}");
            }
            let Some(base_addr) = fr.addr else {
                // Exit 2: a CONTROL refused. Printing bytes from an address only one derivation
                // agrees with is precisely the plausible-looking-but-wrong output this exists for.
                return Err(session::control_refusal(format!(
                    "the two-derivation cross-check for family {fam}/{typ} did not pass - refusing \
                     to print bytes from an address only one of them agrees with"
                )));
            };
            if idx >= fr.cap {
                bail!("index {idx} >= capacity {}", fr.cap);
            }
            idx.checked_mul(fr.stride)
                .and_then(|o| base_addr.checked_add(o))
                .ok_or_else(|| anyhow::anyhow!("element {idx} x stride {:#x} overflows", fr.stride))?
        }
    };

    let n = a.len.filter(|v| *v != 0).map(|v| v as usize).unwrap_or(128);
    let data = s.proc.read(addr, n)?;
    println!("element @ {addr:#x}, {n} bytes");
    println!("{}", format::hexdump(&data, addr, None));
    if let Some(sn) = &a.strukt {
        let sd = s
            .syms
            .structs
            .get(sn)
            .ok_or_else(|| anyhow::anyhow!("unknown struct {sn:?} - `d1-live syms` lists them"))?;
        println!("{}", format::show_struct(sd, &data));
    }
    Ok(0)
}

fn cmd_tag(cli: &Cli, a: &TagArgs) -> Result<i32> {
    let s = Session::open(&a.common.opts(cli.eboot.clone()))?;
    let (_lv, ltext, refusal) = s.liveness(0);
    println!("{ltext}");
    if let Some(e) = refusal {
        return Err(e);
    }
    let g = s.guest();

    // The stride the CLIENT states, checked once rather than per tag - see `package_slot_stride`.
    if let (_, Some(warn)) = g.package_slot_stride()? {
        println!("{warn}");
    }
    for t in &a.tags {
        // ⚠ Refuse rather than narrow. `… as u32` silently turned `0xFFFFFFFFFF` into `0xFFFFFFFF`
        // and then printed the truncated value as though it were the tag asked for.
        let raw = session::parse_u64(t)?;
        let tag = u32::try_from(raw).map_err(|_| {
            anyhow::anyhow!("tag {raw:#x} does not fit in 32 bits - a content tag is a u32")
        })?;
        let (obj, why) = g.resolve_tag(tag)?;
        match obj {
            None => println!("tag {tag:#010x}: {why}"),
            Some(obj) => {
                println!("tag {tag:#010x}: {why}  -> object @ {obj:#x}");
                let n = a.len.filter(|v| *v != 0).map(|v| v as usize).unwrap_or(128);
                println!("{}", format::hexdump(&s.proc.read(obj, n)?, obj, None));
            }
        }
    }
    Ok(0)
}

fn cmd_descriptors(cli: &Cli, a: &DescArgs) -> Result<i32> {
    let s = Session::open(&a.common.opts(cli.eboot.clone()))?;
    let (_lv, ltext, refusal) = s.liveness(0);
    println!("{ltext}");
    if let Some(e) = refusal {
        return Err(e);
    }
    let g = s.guest();

    let mut last: Option<String> = None;
    loop {
        let rows = g.descriptors()?;
        let out = if rows.is_empty() {
            "  (no claimed descriptors)".to_string()
        } else {
            rows.iter().map(|d| d.line()).collect::<Vec<_>>().join("\n")
        };
        if last.as_deref() != Some(out.as_str()) {
            println!("{}", hhmmss());
            println!("{out}");
            use std::io::Write;
            std::io::stdout().flush().ok();
            last = Some(out);
        }
        if !a.watch {
            return Ok(0);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// `time.strftime("%H:%M:%S")` in local time, without pulling a date crate into this crate's
/// dependency set for one line.
fn hhmmss() -> String {
    let out = std::process::Command::new("date").arg("+%H:%M:%S").output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

/// ⚠ In `--json` mode every human line - provenance, controls, the address resolution - goes to
/// **stderr**, so stdout is a parseable document and nothing else. A tool that mixes them makes its
/// own output un-pipeable, and the provenance is the part a reader must not lose.
macro_rules! say {
    ($json:expr, $($t:tt)*) => {
        if $json { eprintln!($($t)*) } else { println!($($t)*) }
    };
}

fn cmd_watch(cli: &Cli, a: &WatchArgs) -> Result<i32> {
    let j = cli.json;
    // ⛔ Refuse BEFORE anything is resolved or armed, and name the command that does the job.
    // `d1-replay` depends on `d1-live`, so the pairing cannot live here without a dependency cycle
    // or without dragging the snapshot format down into this crate (historicaldocs/design/live-watch.md §6a.1).
    if let Some(expr) = &a.snapshot_on_hit {
        bail!(
            "--snapshot-on-hit is NOT a d1-live flag, and the feature EXISTS - it lives in the tool \
             that owns the snapshot format:\n    d1-replay capture --out F --snapshot-on-hit {expr}\n\
             (d1-replay depends on d1-live, so the pairing cannot go the other way. Spec: \
             historicaldocs/design/live-watch.md §6a.)"
        );
    }
    let pid = pick_pid(a.pid)?;
    let proc = Proc::open(pid)?;
    let syms = Symbols::open_default()?;
    let rw = Rw::parse(&a.rw)?;

    // ── The base, and therefore the address space this command is speaking in ───────────────────
    let base = if a.no_base {
        None
    } else {
        let image = live::open_image(cli.eboot.clone())?;
        let (ok, line) = syms.check_build(&image.sha256());
        say!(j, "{line}");
        if !ok {
            bail!("the symbol file was measured against a different eboot - refusing to translate");
        }
        let ctl = live::recover_base(&proc, &image, &syms, a.full_scan);
        say!(j, "{}", ctl.report());
        if !ctl.ok() {
            bail!("base control FAILED - every address below would be plausible-looking and wrong");
        }
        ctl.base
    };

    // ── Resolve the address, and PRINT how it resolved ──────────────────────────────────────────
    let expr = if a.abs {
        if parse_u64(&a.addr).is_err() {
            bail!(
                "--abs applies to a bare number. {:?} is an expression - say the space inside it \
                 (`a:0x…` for absolute, `v:0x…` for an eboot vaddr).",
                a.addr
            );
        }
        format!("a:{}", a.addr)
    } else {
        a.addr.clone()
    };
    let mut ctx = Ctx::new(base.unwrap_or(0), &syms);
    let mem = |ad: u64| -> Result<u64> {
        let b = proc.read(ad, 8)?;
        Ok(u64::from_le_bytes(b.try_into().unwrap()))
    };
    ctx.mem = Some(&mem);
    if base.is_none() && !expr.starts_with("a:") {
        bail!("--no-base was given, so every address must be absolute (`a:0x…` or --abs)");
    }
    let target = addr::addr(&expr, &ctx)?;
    for l in ctx.take_trace() {
        say!(j, "{l}");
    }
    let _ = Width::U64; // the value language is shared; nothing here loads a value.

    // ── Measure the debug-register offset against a child of OUR OWN before touching the client ─
    let helper = std::env::current_exe().context("locating this binary")?;
    let (dbg, dbg_detail) = d1_live::controls::debugreg_offset(&helper)
        .context("the debug-register offset control failed - refusing to poke the client")?;
    say!(j, "DEBUGREG CONTROL: PASS  u_debugreg[0] at {dbg:#x} - {dbg_detail}");

    // ── Arm ─────────────────────────────────────────────────────────────────────────────────────
    let sigs = d1_live::watch::install_signal_handlers();
    say!(j, "{}", sigs.summary());
    let spec = WatchSpec {
        addr: target,
        len: a.len,
        rw,
        slot: a.slot,
        follow_clone: a.follow_clone,
        base,
        tids: if a.tids.is_empty() { None } else { Some(a.tids.clone()) },
    };
    let mut w = Watcher::attach(pid, spec, dbg)?;
    say!(j,
        "ARMED   pid {pid}  DR{}  {:?} {} byte(s) at {target:#x}{}  on {} thread(s){}",
        a.slot,
        rw,
        a.len,
        base.and_then(|b| target.checked_sub(b))
            .map(|v| format!(" (v:{v:#x})"))
            .unwrap_or_default(),
        w.armed_threads(),
        if a.follow_clone { ", following clones" } else { "" }
    );
    for f in &w.threads_failed {
        say!(j, "  ⚠ not armed: {f}");
    }
    if w.tracked_threads() != w.armed_threads() {
        say!(j,
            "  ⛔ {} thread(s) are on the DISARM list but were NOT verified armed - a register may \
             be live on them and they will be cleared on the way out, but they are not counted as \
             armed and a 'no hit' says nothing about them.",
            w.tracked_threads() - w.armed_threads()
        );
    }
    if !rw.rip_is_the_instruction() {
        say!(j,
            "        ⚠ a data breakpoint is a TRAP: the reported RIP is the instruction AFTER the \
             store. `writer` below is the decoded store."
        );
    }
    say!(j, "WAITING up to {:.0}s for {} hit(s) - Ctrl-C disarms cleanly.", a.max_seconds, a.count);

    let show_regs = a.regs;
    let report = w.run(&proc, a.count, Duration::from_secs_f64(a.max_seconds), move |h| {
        if !j {
            print_hit(h, show_regs);
        }
    })?;
    for f in w.disarm() {
        eprintln!("d1-live: {f}");
    }

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("\n{}", report.summary());
        if report.foreign_stops > 0 {
            println!(
                "  ⚠ {} stop(s) belonged to tasks we did not arm - something else in this process \
                 is tracing, and notifications may have been stolen.",
                report.foreign_stops
            );
        }
        if report.unattributed_sigtraps > 0 {
            println!(
                "  ⚠ {} SIGTRAP(s) were NOT ours (DR6 did not name slot {}) - another debugger, or \
                 the client's own trap use.",
                report.unattributed_sigtraps, report.slot
            );
        }
        for (sig, n) in &report.signals_forwarded {
            println!("  forwarded {n}× signal {sig} to the target (never swallowed)");
        }
    }
    Ok(if report.hits.is_empty() { 1 } else { 0 })
}

fn print_hit(h: &d1_live::watch::Hit, show_regs: bool) {
    println!("\nHIT #{}  t+{} ms  tid {} ({})", h.seq, h.elapsed_ms, h.tid, h.comm);
    match (h.writer, &h.writer_text) {
        (Some(w), Some(t)) => {
            println!(
                "  writer  {w:#x}{}  {t}   [{}]",
                h.writer_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default(),
                h.writer_confidence
            );
            if let Some(v) = h.writer_vaddr {
                println!("          ⇒ name it:  capture-rig/eboot-atlas.py --addr {v:#x}");
            }
        }
        _ => println!("  writer  UNREADABLE - could not decode backwards from RIP"),
    }
    println!(
        "  RIP     {:#x}{}",
        h.rip,
        h.rip_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default()
    );
    if let Some(v) = &h.value_after {
        println!("  value   {v}  (bytes at the watched address, read after the trap)");
    }
    println!("  regs    rsp={:#x} rdi={:#x} rsi={:#x} rbx={:#x}", h.rsp, h.rdi, h.rsi, h.rbx);
    if show_regs {
        // The whole integer file, four to a line, in a fixed order so two hits diff by eye.
        let order = [
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
            "r13", "r14", "r15", "rip", "eflags", "orig_rax", "cs", "ss", "ds", "es", "fs", "gs",
            "fs_base", "gs_base",
        ];
        for chunk in order.chunks(4) {
            let mut line = String::from("         ");
            for k in chunk {
                if let Some(v) = h.regs.get(*k) {
                    line.push_str(&format!("{k:>8} {v:#018x}  "));
                }
            }
            println!("{line}");
        }
        println!(
            "          ⛔ integer registers only - no xmm/FP state was captured, so a float \
             argument is NOT in here."
        );
    }
    if h.candidates.len() > 1 {
        println!("  ⚠ backward decode is ambiguous; every candidate ending at RIP:");
        for c in &h.candidates {
            println!(
                "      {:#x}  {:<40} [{}]{}",
                c.addr,
                c.text,
                c.bytes,
                if c.writes_memory { "  writes-memory" } else { "" }
            );
        }
    }
}

// Keep `ptrace` referenced so the module's compile-time asserts travel with the binary.
#[allow(dead_code)]
const _: u64 = ptrace::DEBUGREG_OFFSET;
