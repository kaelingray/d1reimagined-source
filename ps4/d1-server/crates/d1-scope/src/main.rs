// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-scope` - the read-only state plane and the leased event plane, on the command line.
//!
//! > **THE LIFECYCLE INVARIANT.** *d1-scope may never leave a hardware watchpoint armed behind a
//! > process that is not alive to disarm it … **A tracer that dies armed kills its target.***
//! > BUILD 2 honours it with four entrances to ONE stop path - normal return, error, panic and
//! > signal - plus the child's own `--max-seconds` lease for the case where none of them runs.
//!
//! ⭐ `--source` defaults to a **snapshot**. A tool that attaches to the operator's client because
//! someone forgot a flag is a tool that will one day do it during a run that mattered.

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use d1_scope::eval::Evaluator;
use d1_scope::mem_source::{MemSource, ProcMem, SnapMem};
use d1_scope::registry::Registry;
use d1_scope::sampler::Sampler;
use d1_scope::{child_mgr, controls, controls_event, events};

#[derive(Parser)]
#[command(
    name = "d1-scope",
    about = "The live client as TWO PLANES: a read-only state sampler, and ONE leased d1-live child.",
    after_help = "\
⛔ SCOPE - stated so nothing here is mistaken for more than it is:
   • STATE plane: registry -> resolve -> read -> one immutable Frame. No write path exists.
   • EVENT plane: exactly ONE `d1-live watch` child, always with a bounded --max-seconds lease,
     stopped by SIGTERM only, and never by the un-catchable signal on any path.
   • VIEWS: `gui` (the surface table) and `graph` (the flow canvas). Both need --features gui for
     a window; both render the IDENTICAL model under --headless [--json], in every build.
   • the GRAPH's colours are DERIVED from the state plane this tick and printed next to the
     registry's DECLARED state. A disagreement is reported, never resolved.
   • --source live is NOT wired for the STATE plane; `watch --pid` drives the EVENT plane and
     REFUSES the emulator unless D1_SCOPE_ALLOW_LIVE=1 is set deliberately.
⛔ `watch` exits 2 if ANY stop this session failed to prove a disarm - including one collected by
  the heartbeat, which is the path a --count-bounded or self-expired child takes.
⚠ Reads outside a snapshot's captured extents render UNMAPPED. 'not captured' and 'was zero'
  are different facts and only one of them is evidence.
⚠ Events arrive in BATCHES at each re-arm: d1-live writes its report once, on the way out."
)]
struct Cli {
    /// The surface registry. Defaults to capture-rig/surfaces/registry.toml.
    #[arg(long, global = true)]
    registry: Option<std::path::PathBuf>,

    /// `snap:<id>` · `snap:<path>` · ⭐ `live:<pid>` to sample the RUNNING client (read-only).
    ///
    /// ⛔ Defaults to a snapshot, never to the live client, and `live:` on the emulator is refused
    /// unless `D1_SCOPE_ALLOW_LIVE=1`. ⭐ Read-only either way - `MemSource` has no write method.
    #[arg(long, global = true, default_value = "snap:rung36-biped-chain")]
    source: String,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The controls. Offline. ⛔ EXITS 2 ON ANY FAILURE - output from a failed run may not be cited.
    Controls {
        /// Skip the event-plane controls (#6-#10), which spawn processes and wait out a real lease.
        /// ⚠ The summary says which set ran; this does not skip them into a pass.
        #[arg(long)]
        state_only: bool,
    },
    /// ⭐ Who holds a process, and what to do about it. Reads `/proc` only; touches nothing.
    Tracer {
        #[arg(long)]
        pid: i32,
    },
    /// ⭐ THE EVENT PLANE. Arm one leased `d1-live` child on `--pid` and print the frames.
    ///
    /// ⛔ There is no default pid, and the emulator is refused unless `D1_SCOPE_ALLOW_LIVE=1`.
    Watch(WatchArgs),
    /// Replay a recorded `d1-live --json` report into events attached to registry surfaces.
    /// Fully offline - no child, no target, nothing armed.
    Events {
        /// The report to read. Defaults to the checked-in fixture.
        #[arg(long)]
        report: Option<std::path::PathBuf>,
        /// What the slot claimed to be watching, so the two derivations can disagree.
        #[arg(long)]
        surface: Option<String>,
    },
    /// (internal, control #9 only) launch a leased child, announce it, and stand there to be killed.
    #[command(hide = true, name = "__abandon-child")]
    AbandonChild {
        #[arg(long)]
        pid: i32,
        #[arg(long)]
        addr: String,
        #[arg(long)]
        lease: f64,
        #[arg(long)]
        stdio_dir: std::path::PathBuf,
        #[arg(long)]
        live_bin: std::path::PathBuf,
    },
    /// The registry, resolved against --source.
    Surfaces {
        #[arg(long)]
        group: Option<String>,
    },
    /// ⭐ One Frame: the whole tool's state, and the only thing a frontend ever sees.
    Frame,
    /// The flow graph as rows. ⚠ BUILD 1 renders the DECLARED state; it evaluates no predicate.
    Edges {
        #[arg(long)]
        state: Option<String>,
    },
    /// ⭐⭐ THE FLOW GRAPH - a hand-drawn node/edge canvas, or `--headless [--json]`.
    ///
    /// ⛔ It arms NOTHING, exactly as `gui` does. Colours are DERIVED from the state plane this
    /// tick and shown next to the registry's DECLARED state; a disagreement is printed, never
    /// resolved. Layout, witnesses and columns come from `capture-rig/surfaces/graph.toml`.
    Graph(GraphArgs),
    /// ⭐ THE SURFACE TABLE - a window, or `--headless [--json]` for a run log with no window at all.
    ///
    /// ⛔ It arms NOTHING. The window reads no memory (a sampler thread does that and hands over an
    /// immutable Frame) and spawns no child; `d1-scope watch` remains the only way to arm anything,
    /// and it takes a deliberate `--pid`.
    Gui(GuiArgs),
}

#[derive(clap::Args)]
struct GraphArgs {
    /// No window: tick, build the same model, print it, exit. ⭐ Same `GraphModel` the canvas draws.
    #[arg(long)]
    headless: bool,
    /// How many ticks to build. `0` = until interrupted.
    ///
    /// ⛔ Mode-dependent, for the same reason as [`GuiArgs::ticks`]: `--headless` ⇒ 1 (build, print,
    /// exit); the **canvas** ⇒ 0. A flat `1` froze the window on its first frame.
    #[arg(long)]
    ticks: Option<u64>,
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,
    /// Replay a recorded `d1-live --json` report so the edge badges and the writer log are real.
    /// Fully offline - no child, no target, nothing armed.
    #[arg(long)]
    events: Option<std::path::PathBuf>,
    /// What the armed slot claimed to be watching, so the two attributions can disagree.
    #[arg(long)]
    surface: Option<String>,
    /// The layout + witness file. Defaults to capture-rig/surfaces/graph.toml.
    #[arg(long)]
    spec: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 2000)]
    stale_after_ms: u64,
}

impl GraphArgs {
    /// See [`GuiArgs::effective_ticks`] - identical rule, and the same control covers both.
    fn effective_ticks(&self) -> u64 {
        d1_scope::effective_ticks(self.ticks, self.headless)
    }
}

#[derive(clap::Args)]
struct GuiArgs {
    /// No window: tick, render the same table, print it, exit. ⭐ The rows are byte-identical to
    /// the ones the window draws - same `TableModel`, same `TableRow::cell`.
    #[arg(long)]
    headless: bool,
    /// How many ticks to dump. `0` = until interrupted.
    ///
    /// ⛔⛔ **THE DEFAULT IS MODE-DEPENDENT AND IT HAS TO BE.** Leaving it unset means:
    /// * `--headless` ⇒ **1** - tick, print, exit. Correct: the dump is a one-shot.
    /// * the **window** ⇒ **0** - sample until interrupted.
    ///
    /// ⚠ A flat default of `1` shipped once and made the window sample **exactly once**: the sampler
    /// thread returned, the frame stopped advancing, and `--stale-after-ms` (2 s) then painted every
    /// row `STALE`. It looked like *"the client went quiet"* and was *"the tool stopped asking."*
    /// **That is the crate's own thesis turned on itself** - a frozen window and a quiet client are
    /// pixel-identical, which is exactly why `STALE` exists. The staleness badge WORKED; the default
    /// made a frozen window the normal case. ⇒ resolved in [`GuiArgs::effective_ticks`], with a
    /// control that fails if the two modes are ever collapsed back into one number.
    #[arg(long)]
    ticks: Option<u64>,
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,
    /// Replay a recorded `d1-live --json` report so the event badges are real. Fully offline.
    #[arg(long)]
    events: Option<std::path::PathBuf>,
    /// What the armed slot claimed to be watching, so the two attributions can disagree.
    #[arg(long)]
    surface: Option<String>,
    #[arg(long, default_value = "")]
    filter: String,
    /// group · name · addr · value · state · event
    #[arg(long, default_value = "group")]
    sort: String,
    #[arg(long)]
    desc: bool,
    #[arg(long)]
    no_group: bool,
    #[arg(long)]
    no_fields: bool,
    /// ⭐ Older than this and every row renders `STALE ‹…›`. A frozen window and a quiet client are
    /// otherwise pixel-identical.
    ///
    /// ⚠ It has no effect under `--headless`, which prints each frame immediately: age is ~0 by
    /// construction there and the dump says so on every tick.
    #[arg(long, default_value_t = 2000)]
    stale_after_ms: u64,
}

impl GuiArgs {
    /// ⭐ The one place the mode-dependent `--ticks` default is decided. An explicit `--ticks N`
    /// always wins, in both modes.
    ///
    /// ⛔ Do NOT "simplify" this to a `default_value_t` on the field: the two modes genuinely want
    /// different numbers, and the flat default that used to be there froze the window on its first
    /// frame (see the field's doc comment). `controls` asserts both arms.
    fn effective_ticks(&self) -> u64 {
        d1_scope::effective_ticks(self.ticks, self.headless)
    }
}

#[derive(clap::Args)]
struct WatchArgs {
    /// ⛔ REQUIRED, and never defaulted. A tool that attaches to the operator's client because
    /// someone forgot a flag is a tool that will one day do it during a run that mattered.
    #[arg(long)]
    pid: i32,
    /// The address, in `d1-live`'s grammar (`v:0x…` an eboot vaddr · `a:0x…` absolute · `[…]` a
    /// deref · `+N` a plain offset). Resolved by the CHILD, at every arm.
    #[arg(long)]
    addr: String,
    /// The registry surface this slot is meant to be watching. Attribution is derived independently
    /// from the address, and the two are compared.
    #[arg(long)]
    surface: Option<String>,
    #[arg(long, default_value_t = 8)]
    len: u64,
    /// `write` · `rw` · `exec` (a hardware breakpoint; needs `--len 1`).
    #[arg(long, default_value = "write")]
    rw: String,
    /// ⭐ The child's `--max-seconds`. It is re-armed well before this expires, so it is a
    /// **backstop**: the longest a watchpoint can outlive a d1-scope that is hard-killed.
    #[arg(long, default_value_t = 20.0)]
    lease: f64,
    /// How long this session runs. The child is stopped and confirmed disarmed at the end.
    #[arg(long, default_value_t = 60.0)]
    seconds: f64,
    /// Hits per child before it exits and is re-armed.
    #[arg(long, default_value_t = 32)]
    count: usize,
    /// Recover the guest base, so `v:` addresses and `RIP → v:` translation work. Only meaningful
    /// against the pinned client.
    #[arg(long)]
    base: bool,
    #[arg(long)]
    follow_clone: bool,
    /// Put the child's stdout/stderr here. ⭐ Do this for anything that matters: it is what keeps
    /// the child's own report readable if d1-scope is hard-killed.
    #[arg(long)]
    stdio_dir: Option<std::path::PathBuf>,
}

/// Ask the `d1-live` BINARY for the measured guest base. ⛔ Spawned, never linked - see the note in
/// [`open_source`] and control #4.
///
/// ⭐ `d1-live base` runs three controls (two independent anchors agreeing on exactly one base, a
/// file-image verify, and a **negative control** proving the verify window discriminates). We refuse
/// unless it reports `PASS`, because an unmeasured base is not a smaller answer - it is one that
/// reads plausible values out of the wrong place.
fn measure_base_via_d1_live(pid: i32) -> Result<u64> {
    let bin = std::env::var("D1_LIVE_BIN").unwrap_or_else(|_| "d1-live".into());
    let out = std::process::Command::new(&bin)
        .args(["base", "--pid", &pid.to_string()])
        .output()
        .map_err(|e| anyhow::anyhow!("could not run `{bin} base --pid {pid}`: {e}. Set D1_LIVE_BIN."))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    // ⚠ Require the PASS *and* the value. A report that printed a base on a FAILING control set is
    //   exactly the "receipt proves you built it, never that it is right" shape.
    let line = text
        .lines()
        .find(|l| l.contains("BASE CONTROL") && l.contains("PASS"))
        .ok_or_else(|| anyhow::anyhow!("`d1-live base` did not report PASS - refusing to guess a base.\n{text}"))?;
    let hex = line
        .split("base=")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .ok_or_else(|| anyhow::anyhow!("`d1-live base` PASSed but printed no base= value:\n{line}"))?;
    let b = u64::from_str_radix(hex.trim().trim_start_matches("0x"), 16)
        .map_err(|e| anyhow::anyhow!("unparseable base {hex:?} from `d1-live base`: {e}"))?;
    Ok(b)
}

/// Open whatever `--source` names. ⭐ `snap:<id>` · `snap:<path>` · `live:<pid>`.
///
/// ⛔⛔ **`live:` READS THE OPERATOR'S CLIENT, AND THE GUARD IS NOT HERE - IT IS IN
/// [`ProcMem::open`]**, which refuses any pid whose `comm` looks like shadPS4 unless
/// `D1_SCOPE_ALLOW_LIVE=1` is set deliberately. That is on purpose: **one guard, at the thing that
/// opens the file descriptor**, so no future caller can route around it by constructing a source
/// some other way.
///
/// ⭐ It is still a **read-only** source. [`MemSource`] has no `write` method at all, so "live" here
/// means *sample the running client*, never *poke it*. Control #4 re-derives the O_RDONLY fd from
/// `/proc/self/fdinfo` rather than trusting this comment.
///
/// ⚠ The default remains a **snapshot**. A tool that attaches because someone forgot a flag is a
/// tool that will one day do it during a run that mattered.
fn open_source(spec: &str) -> Result<Box<dyn MemSource>> {
    if let Some(rest) = spec.strip_prefix("live:") {
        let pid: i32 = rest.trim().parse().map_err(|_| {
            anyhow::anyhow!("--source live:<pid> - {rest:?} is not a pid")
        })?;
        let mem = ProcMem::open(pid)?; // ⛔ the emulator guard lives in here

        // ⭐⭐ THE BASE IS MEASURED, NEVER ASSUMED - and it is measured by the `d1-live` BINARY,
        //    SPAWNED, not linked.
        //
        // ⛔⛔ THIS IS NOT A STYLE CHOICE - CONTROL #4 ENFORCES IT, AND IT CAUGHT THE MISTAKE.
        //    The first version of this function called `d1_live::proc::recover_base(..)` directly.
        //    It worked, read the live client correctly, and **failed control #4**:
        //        FAIL the debug-API name appears 2x ... a SECOND occurrence fails this control
        //    `d1_live::proc` carries that surface, so linking it put the debug API inside the
        //    STATE plane - the one thing this crate's whole safety argument says is impossible.
        //    ⇒ The state plane stays debug-API-free; `d1-live` is a SUBPROCESS here exactly as it is
        //    for the event plane. One architecture, one rule, enforced by a control that can fail.
        let base = measure_base_via_d1_live(pid)?;
        eprintln!("⭐ live pid {pid}: measured guest base {base:#x} (via `d1-live base`, 3 controls)");
        return Ok(Box::new(mem.with_base(base)));
    }
    let rest = spec.strip_prefix("snap:").ok_or_else(|| {
        anyhow::anyhow!(
            "--source must be `snap:<id>`, `snap:<path>` or `live:<pid>` (got {spec:?})"
        )
    })?;
    let p = if rest.contains('/') {
        std::path::PathBuf::from(rest)
    } else {
        SnapMem::path_for_id(rest)
    };
    Ok(Box::new(SnapMem::open(p)?))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // ⚠ Handled before the registry is opened: the control-#9 helper must be able to run in a bare
    // checkout, and it has no business needing a surface registry to stand still and be killed.
    if let Cmd::AbandonChild { pid, addr, lease, stdio_dir, live_bin } = &cli.cmd {
        return d1_scope::controls_event::abandon_child_main(
            *pid,
            addr.clone(),
            *lease,
            stdio_dir.clone(),
            live_bin.clone(),
        )
        .map_err(|e| anyhow::anyhow!(e));
    }
    let reg = match &cli.registry {
        Some(p) => Registry::open(p)?,
        None => Registry::open_default()?,
    };

    match &cli.cmd {
        Cmd::Controls { state_only } => {
            let results = if *state_only {
                controls::run_state_only(&reg)?
            } else {
                controls::run_all(&reg)?
            };
            let mut failed = 0;
            for c in &results {
                println!("{} {}  {}", if c.pass { "PASS" } else { "FAIL" }, c.id, c.name);
                for l in &c.lines {
                    if !c.pass || cli.json || l.contains("FAIL") || l.starts_with("    ·") {
                        println!("{l}");
                    }
                }
                if !c.pass {
                    failed += 1;
                }
            }
            println!(
                "\n{} of {} controls PASS",
                results.len() - failed,
                results.len()
            );
            if failed > 0 {
                eprintln!(
                    "\n⛔ {failed} control(s) FAILED - exit 2. Nothing this run produced may be cited."
                );
                std::process::exit(2);
            }
            println!("⭐ every control passed, and every one of them has been inverted once.");
        }
        Cmd::Surfaces { group } => {
            let ms = open_source(&cli.source)?;
            let syms = d1_live::symbols::Symbols::open_default()?;
            let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(&reg);
            let mut s = Sampler::new(&reg, ev);
            let f = s.tick();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&f)?);
            } else {
                println!("source   {}", f.source.label());
                for n in &f.notes {
                    println!("note[{}] {}", n.level, n.text);
                }
                for v in &f.surfaces {
                    if let Some(g) = group {
                        if v.group != *g {
                            continue;
                        }
                    }
                    println!(
                        "{:<36} {:<11} {:<30} {:<34} {:?}",
                        v.name,
                        v.group,
                        v.addr_expr,
                        v.resolved.render(),
                        v.liveness.verdict
                    );
                }
            }
        }
        Cmd::Frame => {
            let ms = open_source(&cli.source)?;
            let syms = d1_live::symbols::Symbols::open_default()?;
            let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(&reg);
            let mut s = Sampler::new(&reg, ev);
            let f = s.tick();
            println!("{}", serde_json::to_string_pretty(&f)?);
        }
        Cmd::Edges { state } => {
            let ms = open_source(&cli.source)?;
            let syms = d1_live::symbols::Symbols::open_default()?;
            let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(&reg);
            let mut s = Sampler::new(&reg, ev);
            let f = s.tick();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&f.edges)?);
            } else {
                for e in &f.edges {
                    let st = format!("{:?}", e.state).to_lowercase();
                    if let Some(want) = state {
                        if st != *want {
                            continue;
                        }
                    }
                    println!(
                        "{:<5} {:<7} {:<34} -> {:<34} {}{}",
                        e.id,
                        st,
                        e.up,
                        e.down,
                        e.mech.chars().take(60).collect::<String>(),
                        if e.guard_active { "  ⚠GUARDED" } else { "" }
                    );
                }
                println!("\n⚠ states above are DECLARED in registry.toml, not evaluated this tick.");
            }
        }

        // ── the event plane ─────────────────────────────────────────────────────────────────────
        Cmd::Tracer { pid } => {
            let comm = child_mgr::comm_of(*pid);
            let tracers = child_mgr::tracer_pids(*pid).unwrap_or_default();
            println!("pid {pid}  comm {comm:?}  alive {}", child_mgr::alive(*pid));
            if tracers.is_empty() {
                println!("TracerPid  0 - nothing is tracing it");
            } else {
                for t in &tracers {
                    println!("TracerPid  {t}  ({})", child_mgr::comm_of(*t));
                }
                println!(
                    "\n⇒ to hand it over, that holder must release it. A `d1-live watch` releases on \
                     SIGTERM, on Ctrl-C, and by itself at its --max-seconds lease.\n\
                     ⛔ Do NOT hard-kill it: a tracer that dies armed KILLS ITS TARGET."
                );
            }
            match child_mgr::check_target(*pid, None) {
                Ok(()) => println!("\nd1-scope WOULD attach to this process."),
                Err(e) => println!("\nd1-scope REFUSES this process:\n{}", e.message()),
            }
        }

        Cmd::Events { report, surface } => {
            let path = report.clone().unwrap_or_else(controls_event::fixture_path);
            let text = std::fs::read_to_string(&path)?;
            let (rep, missing) = events::parse_report(&text).map_err(|e| anyhow::anyhow!(e))?;
            if !missing.is_empty() {
                eprintln!("⚠ the report is missing key(s) we depend on: {missing:?}");
            }
            let ms = open_source(&cli.source)?;
            let syms = d1_live::symbols::Symbols::open_default()?;
            let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(&reg);
            let mut s = Sampler::new(&reg, ev);
            s.armed_surface = surface.clone();
            s.inbox.push(rep);
            let f = s.tick();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&f)?);
            } else {
                for n in &f.notes {
                    println!("note[{}] {}", n.level, n.text);
                }
                for e in &f.events {
                    print_event(e);
                }
                println!("\n{} event(s) from {}", f.events.len(), path.display());
            }
        }

        Cmd::Watch(a) => return cmd_watch(&cli, &reg, a),

        Cmd::Gui(a) => return cmd_gui(&cli, a),

        Cmd::Graph(a) => return cmd_graph(&cli, a),

        Cmd::AbandonChild { .. } => unreachable!("handled before the registry is opened"),
    }
    Ok(())
}

fn print_event(e: &d1_scope::frame::EventView) {
    println!(
        "\nEVENT #{}  t+{} ms  tid {}  {}  [{}]",
        e.seq,
        e.elapsed_ms,
        e.tid,
        e.surface,
        e.how
    );
    if let Some(f) = &e.field {
        println!("  field   {f}");
    }
    match e.writer {
        Some(w) => println!(
            "  writer  {w:#x}{}  {}   [{}]{}",
            e.writer_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default(),
            e.writer_text,
            e.writer_confidence,
            if e.writer_quotable { "" } else { "   ⚠ NOT QUOTABLE - close it with d1-eboot disasm" }
        ),
        None => println!("  writer  UNDECODABLE"),
    }
    // ⚠ Always printed, always second, always labelled: RIP is the instruction AFTER the store.
    println!(
        "  RIP     {:#x}{}   ⚠ the instruction AFTER the store",
        e.rip,
        e.rip_vaddr.map(|v| format!("  = v:{v:#x}")).unwrap_or_default()
    );
    if let Some(v) = &e.value_after {
        println!("  value   {v}  (bytes at the watched address, read after the trap)");
    }
    if let Some(d) = &e.disagreement {
        println!("  ⚠⚠ {d}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The surface table - one sampler, two renderings.

/// Everything the sampler needs, owned, so the whole loop can move onto its own thread.
///
/// ⭐ **This is why the UI thread can be proven never to read memory.** The source, the symbols and
/// the evaluator are constructed *inside* [`run_sampler`] and never escape it; the only thing that
/// crosses the boundary is an immutable [`d1_scope::frame::Frame`].
struct SampleJob {
    source: String,
    registry: Option<std::path::PathBuf>,
    events: Option<std::path::PathBuf>,
    armed_surface: Option<String>,
    interval: Duration,
    /// `0` = until the sink says stop.
    ticks: u64,
}

/// Tick until the sink says stop. ⛔ Every read in this whole tool happens under this function.
fn run_sampler(job: SampleJob, mut sink: impl FnMut(d1_scope::frame::Frame) -> bool) -> Result<()> {
    let reg = match &job.registry {
        Some(p) => Registry::open(p)?,
        None => Registry::open_default()?,
    };
    let ms = open_source(&job.source)?;
    let syms = d1_live::symbols::Symbols::open_default()?;
    let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(&reg);
    let mut s = Sampler::new(&reg, ev);
    s.armed_surface = job.armed_surface.clone();

    // ⚠ A recorded report is a ONE-SHOT BATCH, delivered on the first tick - exactly as a real
    //   child delivers it, at its exit. Re-injecting it every tick would manufacture events that
    //   never happened and would make the badges count them again and again.
    if let Some(p) = &job.events {
        let text = std::fs::read_to_string(p)?;
        let (rep, missing) = events::parse_report(&text).map_err(|e| anyhow::anyhow!(e))?;
        if !missing.is_empty() {
            eprintln!("⚠ the report is missing key(s) we depend on: {missing:?}");
        }
        // ⭐ The replay also reconstructs the CHILD STATUS the document came out of, so the badges
        //   are the ones a live child would have produced. ⚠ Its receipt claims NO disarm - see
        //   `events::replayed_child_status`.
        s.child = events::replayed_child_status(&rep, s.armed_surface.clone());
        s.inbox.push(rep);
    }

    let mut n = 0u64;
    loop {
        let f = s.tick();
        if !sink(f) {
            return Ok(());
        }
        n += 1;
        if job.ticks != 0 && n >= job.ticks {
            return Ok(());
        }
        if child_mgr::interrupted() {
            return Ok(());
        }
        std::thread::sleep(job.interval);
    }
}

fn table_params(a: &GuiArgs) -> Result<d1_scope::table::TableParams> {
    let sort = d1_scope::table::Column::parse(&a.sort).ok_or_else(|| {
        anyhow::anyhow!(
            "--sort {:?} is not a column. One of: {}",
            a.sort,
            d1_scope::table::Column::ALL.map(|c| c.title()).join(" · ")
        )
    })?;
    Ok(d1_scope::table::TableParams {
        filter: a.filter.clone(),
        sort,
        descending: a.desc,
        group_by_group: !a.no_group,
        show_fields: !a.no_fields,
        age_ms: 0,
        stale_after_ms: a.stale_after_ms,
        held_surfaces: Vec::new(),
        refusals: Vec::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The flow graph - one sampler, two renderings, exactly as the table.

fn cmd_graph(cli: &Cli, a: &GraphArgs) -> Result<()> {
    let reg = match &cli.registry {
        Some(p) => Registry::open(p)?,
        None => Registry::open_default()?,
    };
    let spec = match &a.spec {
        Some(p) => d1_scope::graph::GraphSpec::open(p)?,
        None => d1_scope::graph::GraphSpec::open_default()?,
    };
    // ⛔ REFUSE A DRIFTED PAIR RATHER THAN DRAWING AN INCOMPLETE PICTURE. An edge whose endpoint
    //   has no node is simply not drawn, and a missing arrow looks exactly like an arrow that was
    //   never declared - the silent-failure shape this whole crate is built against.
    let complaints = spec.consistency(&reg);
    if !complaints.is_empty() {
        for c in &complaints {
            eprintln!("⛔ {c}");
        }
        anyhow::bail!(
            "{} inconsistency(ies) between {} and {} - refusing to draw a picture that is missing \
             edges without saying so",
            complaints.len(),
            reg.path.display(),
            spec.path.display()
        );
    }

    let params = d1_scope::graph::GraphParams {
        stale_after_ms: a.stale_after_ms,
        ..Default::default()
    };
    let job = SampleJob {
        source: cli.source.clone(),
        registry: cli.registry.clone(),
        events: a.events.clone(),
        armed_surface: a.surface.clone(),
        interval: Duration::from_millis(a.interval_ms),
        ticks: a.effective_ticks(),
    };

    if a.headless {
        let json = cli.json;
        let reg2 = reg;
        let spec2 = spec;
        return run_sampler(job, move |f| {
            let built = std::time::Instant::now();
            let mut p = params.clone();
            p.age_ms = built.elapsed().as_millis() as u64;
            let m = d1_scope::graph::GraphModel::build(&f, &reg2, &spec2, &p);
            if json {
                match serde_json::to_string(&m) {
                    Ok(s) => println!("{s}"),
                    Err(e) => eprintln!("⛔ serialising the graph model: {e}"),
                }
            } else {
                println!("{}", m.to_text());
            }
            true
        });
    }

    run_graph_window(job, reg, spec, params)
}

#[cfg(not(feature = "gui"))]
fn run_graph_window(
    _job: SampleJob,
    _reg: Registry,
    _spec: d1_scope::graph::GraphSpec,
    _params: d1_scope::graph::GraphParams,
) -> Result<()> {
    anyhow::bail!(
        "this binary was built WITHOUT the window. Rebuild it:\n    \
         cd d1-server/crates/d1-scope && cargo build --release --features gui\n\
         ⭐ `--headless` works in every build and renders the identical model."
    )
}

#[cfg(feature = "gui")]
fn run_graph_window(
    job: SampleJob,
    reg: Registry,
    spec: d1_scope::graph::GraphSpec,
    params: d1_scope::graph::GraphParams,
) -> Result<()> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<std::sync::Arc<d1_scope::frame::Frame>>();
    let provenance = format!("source {} · interval {:?}", job.source, job.interval);
    let ctx_slot: std::sync::Arc<std::sync::Mutex<Option<egui::Context>>> = Default::default();
    let ctx_for_thread = ctx_slot.clone();

    std::thread::Builder::new().name("d1-scope-sampler".into()).spawn(move || {
        let r = run_sampler(job, |f| {
            if tx.send(std::sync::Arc::new(f)).is_err() {
                return false;
            }
            if let Ok(g) = ctx_for_thread.lock() {
                if let Some(c) = g.as_ref() {
                    c.request_repaint();
                }
            }
            true
        });
        if let Err(e) = r {
            eprintln!("⛔ the sampler thread stopped: {e:#}");
        }
    })?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1700.0, 1000.0])
            .with_min_inner_size([1000.0, 600.0])
            .with_title("d1-scope - flow graph"),
        ..Default::default()
    };
    eframe::run_native(
        "d1-scope-graph",
        options,
        Box::new(move |cc| {
            if let Ok(mut g) = ctx_slot.lock() {
                *g = Some(cc.egui_ctx.clone());
            }
            d1_scope::ui::install_fonts(&cc.egui_ctx);
            d1_scope::ui::apply_theme(&cc.egui_ctx);
            Ok(Box::new(d1_scope::ui_graph::GraphApp::new(rx, reg, spec, params, provenance)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

fn cmd_gui(cli: &Cli, a: &GuiArgs) -> Result<()> {
    let params = table_params(a)?;
    let job = SampleJob {
        source: cli.source.clone(),
        registry: cli.registry.clone(),
        events: a.events.clone(),
        armed_surface: a.surface.clone(),
        interval: Duration::from_millis(a.interval_ms),
        ticks: a.effective_ticks(),
    };

    if a.headless {
        let json = cli.json;
        let mut prev = d1_scope::table::Signatures::new();
        return run_sampler(job, move |f| {
            let built = std::time::Instant::now();
            let mut p = params.clone();
            // ⭐ The age is REAL - measured from the instant the frame was built, exactly as the
            //   window measures it from the instant the frame arrived. A dump that hard-coded 0
            //   could never log a STALE row, and STALE is the state a run log most needs.
            p.age_ms = built.elapsed().as_millis() as u64;
            let mut m = d1_scope::table::TableModel::build(&f, &p);
            m.mark_changes(&prev);
            prev = m.signatures();
            if json {
                // JSONL: one model per line, so a run can be `tail -f`'d and diffed.
                match serde_json::to_string(&m) {
                    Ok(s) => println!("{s}"),
                    Err(e) => eprintln!("⛔ serialising the table model: {e}"),
                }
            } else {
                println!("{}", m.to_text());
                println!(
                    "{} of {} rows · {} · seq {} · tick {:.1} ms · age {} ms{}",
                    m.rows.iter().filter(|r| r.kind != d1_scope::table::RowKind::GroupHeader).count(),
                    m.rows_total,
                    m.source,
                    m.frame_seq,
                    m.tick_ms,
                    m.age_ms,
                    if m.stale { " · ⛔ STALE" } else { "" }
                );
                // ⚠ SAY WHAT THIS DUMP CANNOT SEE. A headless run prints each frame the instant it
                //   is built, so its age is always ~0 and NO ROW HERE WILL EVER READ `STALE` -
                //   however low `--stale-after-ms` is set. STALE is a statement about a CONSUMER
                //   that stopped being fed, and this consumer cannot be in that position. Offering
                //   a knob that can never fire, silently, is the failure this line exists to
                //   prevent; the window is where staleness is real.
                if !m.stale {
                    println!(
                        "⚠ a headless dump prints each frame immediately, so `age` is ~0 by \
                         construction and STALE cannot appear here. It is a property of the WINDOW."
                    );
                }
                for n in &m.notes {
                    println!("{n}");
                }
            }
            true
        });
    }

    run_window(job, params)
}

#[cfg(not(feature = "gui"))]
fn run_window(_job: SampleJob, _params: d1_scope::table::TableParams) -> Result<()> {
    // ⛔ NOT a silent fallback to headless. A window that quietly became a text dump is a tool
    //   whose operator does not know what they are looking at.
    anyhow::bail!(
        "this binary was built WITHOUT the window. Rebuild it:\n    \
         cd d1-server/crates/d1-scope && cargo build --release --features gui\n\
         ⭐ `--headless` works in every build and renders the identical table."
    )
}

#[cfg(feature = "gui")]
fn run_window(job: SampleJob, params: d1_scope::table::TableParams) -> Result<()> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<std::sync::Arc<d1_scope::frame::Frame>>();
    let provenance = format!("source {} · interval {:?}", job.source, job.interval);
    let ctx_slot: std::sync::Arc<std::sync::Mutex<Option<egui::Context>>> = Default::default();
    let ctx_for_thread = ctx_slot.clone();

    // ⭐ THE SAMPLER RUNS HERE, AND ONLY HERE. Everything the window ever sees crossed this channel
    //    as an immutable Frame.
    std::thread::Builder::new().name("d1-scope-sampler".into()).spawn(move || {
        let r = run_sampler(job, |f| {
            if tx.send(std::sync::Arc::new(f)).is_err() {
                return false; // the window closed
            }
            if let Ok(g) = ctx_for_thread.lock() {
                if let Some(c) = g.as_ref() {
                    c.request_repaint();
                }
            }
            true
        });
        if let Err(e) = r {
            // ⛔ LOUD. The window's "waiting for the first frame…" placeholder points here on
            //    purpose: an empty table and a sampler that never started must not look alike.
            eprintln!("⛔ the sampler thread stopped: {e:#}");
        }
    })?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([980.0, 520.0])
            .with_title("d1-scope - surfaces"),
        ..Default::default()
    };
    eframe::run_native(
        "d1-scope",
        options,
        Box::new(move |cc| {
            if let Ok(mut g) = ctx_slot.lock() {
                *g = Some(cc.egui_ctx.clone());
            }
            let fonts = d1_scope::ui::install_fonts(&cc.egui_ctx);
            d1_scope::ui::apply_theme(&cc.egui_ctx);
            Ok(Box::new(d1_scope::ui::ScopeApp::new(rx, params, provenance, fonts)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

/// One bounded watch session: arm, heartbeat, present, and leave through a confirmed disarm.
fn cmd_watch(cli: &Cli, reg: &Registry, a: &WatchArgs) -> Result<()> {
    let rw = child_mgr::Rw::parse(&a.rw).map_err(|e| anyhow::anyhow!(e))?;
    let log = child_mgr::new_log();
    let mut mgr = child_mgr::ChildMgr::new(a.pid, Duration::from_secs_f64(a.lease), log.clone())
        .map_err(|e| anyhow::anyhow!(e.message()))?
        .with_count(a.count)
        .with_follow_clone(a.follow_clone);
    if a.base {
        mgr = mgr.with_base();
    }
    if let Some(d) = &a.stdio_dir {
        mgr = mgr.with_stdio_dir(d.clone());
    }
    mgr.add_slot(child_mgr::SlotSpec {
        surface: a.surface.clone(),
        addr_expr: a.addr.clone(),
        len: a.len,
        rw,
    })
    .map_err(|e| anyhow::anyhow!(e))?;
    mgr.launch().map_err(|e| anyhow::anyhow!(e.message()))?;
    if !mgr.wait_until_armed(Duration::from_secs(20)) {
        // ⛔ Not a warning. A session that never armed would report a false "nothing wrote it".
        eprintln!("{}", mgr.stderr_text());
        anyhow::bail!("the d1-live child never confirmed ARMED - refusing to present a false negative");
    }
    eprintln!("{}", mgr.stderr_text());
    for p in mgr.pending() {
        eprintln!("⚠ {p}");
    }

    let ms = open_source(&cli.source)?;
    let syms = d1_live::symbols::Symbols::open_default()?;
    let ev = Evaluator::new(ms.as_ref(), &syms)?.with_registry(reg);
    let mut s = Sampler::new(reg, ev);
    s.armed_surface = mgr.armed_surface();

    // ⛔ A heartbeat REFUSAL is not an anyhow error: an anyhow error exits 1, and 1 is what a
    //   missing file looks like. An unconfirmed disarm is the one condition this whole crate
    //   exists to be loud about, and it exits 2 - through the single decision below.
    let mut refusal: Option<String> = None;

    let deadline = std::time::Instant::now() + Duration::from_secs_f64(a.seconds);
    while std::time::Instant::now() < deadline && !child_mgr::interrupted() {
        if let Err(e) = mgr.heartbeat() {
            // ⛔ STOP SAMPLING. No further child may start, and continuing to print frames would
            //    read as a healthy session.
            refusal = Some(e.message());
            break;
        }
        // Drain whatever the child has handed back since the last tick.
        if let Ok(mut l) = log.lock() {
            s.inbox.append(&mut l.reports);
        }
        s.child = mgr.status();
        let f = s.tick();
        if cli.json {
            println!("{}", serde_json::to_string(&f)?);
        } else {
            for e in &f.events {
                print_event(e);
            }
            for n in &f.notes {
                if n.level != "info" {
                    println!("note[{}] {}", n.level, n.text);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // ⭐ THE STOP IS EXPLICIT AND ITS RECEIPT IS PRINTED. The `Drop` guard would do it anyway; a
    //   session that leaves silently teaches the reader that the guard is optional.
    let receipt = mgr.stop("session-end");
    if let Ok(mut l) = log.lock() {
        s.inbox.append(&mut l.reports);
    }
    s.child = mgr.status();
    let f = s.tick();
    if cli.json {
        println!("{}", serde_json::to_string(&f)?);
    } else {
        for e in &f.events {
            print_event(e);
        }
    }
    if let Some(m) = &refusal {
        println!("\n{m}");
    }
    match &receipt {
        Some(r) if r.confirmed_disarm => println!(
            "\n⭐ DISARM CONFIRMED - child {} left through {:?} after {} ms, {} hit(s).",
            r.child_pid,
            r.stop_reason.clone().unwrap_or_default(),
            r.waited_ms,
            r.hits
        ),
        Some(r) => println!(
            "\n⛔ DISARM NOT CONFIRMED - child {} exit_code {:?} signal {:?} complaints {:?}. \
             Its --max-seconds lease will finish the job; DO NOT hard-kill it.",
            r.child_pid, r.exit_code, r.exit_signal, r.complaints
        ),
        // ⛔ ONLY reachable as a benign statement once the receipts have been consulted below.
        //    Printed unconditionally, this line was the whole defect: a child collected by
        //    `heartbeat()` leaves `self.child == None`, so `stop("session-end")` returns `None`
        //    and the session read as *"nothing was ever running"*.
        None => {}
    }

    // ⭐⭐ ONE DECISION, OVER THE WHOLE SESSION'S RECEIPTS - not over the last one.
    //    `heartbeat()` collects a child that finished on its own (every `--count`-bounded cycle,
    //    every self-expired lease) and that receipt never reaches `receipt` above.
    let unconfirmed = child_mgr::unconfirmed_receipts(&log);
    if unconfirmed.is_empty() {
        if receipt.is_none() && refusal.is_none() {
            println!("\n(no child was running at session end, and every stop this session confirmed its disarm)");
        }
        return Ok(());
    }
    eprintln!(
        "\n⛔⛔ {} of this session's stops did NOT prove a disarm. A tracer that dies armed KILLS \
         ITS TARGET - DO NOT hard-kill anything; each child's own --max-seconds lease is the \
         escalation. Check the target by hand: `d1-scope tracer --pid {}`.",
        unconfirmed.len(),
        a.pid
    );
    for r in &unconfirmed {
        eprintln!(
            "   · how={:?} child={} exit_code={:?} signal={:?} stop_reason={:?} complaints={:?}",
            r.how, r.child_pid, r.exit_code, r.exit_signal, r.stop_reason, r.complaints
        );
    }
    std::process::exit(child_mgr::session_exit_code(&log));
}
