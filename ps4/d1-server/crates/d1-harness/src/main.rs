// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-harness` - generate the two-participant layout, run the activation preflight, keep the pid
//! ledger, and shut the run down safely.
//!
//! ## Exit codes
//!
//! | code | meaning |
//! |---|---|
//! | 0 | did what was asked. For `preflight`, **READY** - every check passed. Nothing was started. |
//! | 2 | a control failed, or the tool refused to act on its own inputs. **Do not cite the output.** |
//! | 3 | `preflight` **REFUSED**, with named reasons printed. This is the normal outcome. |
//!
//! ⛔ There is no subcommand that launches anything, on purpose. (`pad run` creates a kernel
//! input device and blocks serving it - it starts no process and no emulator.)

use std::collections::BTreeMap;

use clap::{Args, Parser, Subcommand, ValueEnum};

use d1_harness::layout::{self, Options};
use d1_harness::preflight::{self, Stage};
use d1_harness::procs::{self, Action, Ledger, Skip};

#[derive(Parser, Debug)]
#[command(
    name = "d1-harness",
    about = "Two-instance lab apparatus: layout, preflight, pid ledger, shutdown. Launches nothing."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the two-participant layout. Creates nothing.
    Layout(LayoutArgs),
    /// Print one participant's environment as `export` lines. Creates nothing.
    Env(EnvArgs),
    /// The activation preflight. **Exit 3 = REFUSED, and that is the expected outcome.**
    Preflight(PreflightArgs),
    /// Record a live process in the run's pid ledger.
    Pid(PidArgs),
    /// Plan (and only with `--execute`, perform) a shutdown of recorded pids.
    Shutdown(ShutdownArgs),
    /// The /dev/uinput virtual gamepad (ported from capture-rig/virtual-pad.py). Start it BEFORE
    /// the emulator; drive it over the control FIFO.
    Pad(PadArgs),
    /// Run every control. **Exit 2 means a control failed and the output may not be cited.**
    Controls,
}

#[derive(Args, Debug)]
struct PadArgs {
    #[command(subcommand)]
    action: PadAction,
}

#[derive(Args, Debug, Clone)]
struct PadIdArgs {
    /// USB vendor id, 16-bit hex. The default is SDL's built-in x360 GUID; a non-default id
    /// changes the SDL joystick GUID (mapping consequences untested against a live client).
    #[arg(long, default_value = "0x045E")]
    vendor: String,
    /// USB product id, 16-bit hex. Give each harness instance its own so two pads are two
    /// devices to SDL.
    #[arg(long, default_value = "0x028E")]
    product: String,
}

#[derive(Subcommand, Debug)]
enum PadAction {
    /// Create the pad, then block serving the FIFO vocabulary
    /// (press | tap | hold | axis | sleep | quit). The device lives as long as this process.
    Run {
        /// Control FIFO to create and read.
        #[arg(long, default_value = "/tmp/d1pad")]
        fifo: String,
        /// The uinput device name the kernel reports.
        #[arg(long, default_value = "D1 Preservation Virtual Pad")]
        name: String,
        #[command(flatten)]
        id: PadIdArgs,
        /// USB version, 16-bit hex.
        #[arg(long, default_value = "0x0110")]
        version: String,
    },
    /// Print the PAD-LAW SDL_GAMECONTROLLER_IGNORE_DEVICES value (every physical js device,
    /// excluding the identity given here) and exit.
    PrintIgnore(PadIdArgs),
    /// Pad controls, including the device tier (creates and destroys real uinput devices).
    /// **Exit 2 means a control failed and pad output may not be cited.**
    Controls {
        /// Fail (instead of skip) any device control that cannot execute here.
        #[arg(long)]
        require_device: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct LayoutArgs {
    #[arg(long, default_value = "d1-two-participant-v2")]
    seed: String,
    /// Directory the run tree *would* live under. Not created.
    #[arg(long)]
    root: Option<String>,
    #[arg(long)]
    repo: Option<String>,
    #[arg(long)]
    emulator_bin: Option<String>,
    #[arg(long)]
    game: Option<String>,
    #[arg(long)]
    patch: Option<String>,
    /// Give participant A a derived identity instead of run-boot.sh's pinned known-good one.
    /// ⚠ Doing this means the run can no longer be compared with any single-participant run.
    #[arg(long)]
    no_anchor: bool,
    /// Write to a file instead of stdout. Refuses to overwrite.
    #[arg(long)]
    out: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct EnvArgs {
    #[command(flatten)]
    layout: LayoutArgs,
    /// Which participant.
    #[arg(long)]
    slot: String,
}

#[derive(Args, Debug, Clone)]
struct PreflightArgs {
    /// A layout JSON written by `d1-harness layout --out`. Without it the layout is regenerated
    /// from the seed, which is fine for a dry check and wrong for an armed one - the run you are
    /// about to clear must be the run you generated.
    #[arg(long)]
    layout: Option<String>,
    #[command(flatten)]
    gen: LayoutArgs,
    #[arg(long, value_enum, default_value_t = StageArg::Layout)]
    stage: StageArg,
    /// Which participant's namespace this preflight is running inside.
    #[arg(long)]
    slot: Option<String>,
    /// The operator's own words. Nothing reaches READY without it, and it is recorded verbatim.
    #[arg(long)]
    activation_review: Option<String>,
    /// `role=sha256`, repeatable. Pins a binary between runs so a relink cannot go unnoticed.
    #[arg(long = "expect-sha")]
    expect_sha: Vec<String>,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum StageArg {
    /// Pure checks over the layout. Safe anywhere.
    Layout,
    /// Adds the containment and namespace probes for one participant.
    Namespace,
    /// Everything, including capture arming, provenance and the activation review.
    Arm,
}

#[derive(Args, Debug, Clone)]
struct PidArgs {
    /// The ledger file. Its directory must already exist.
    #[arg(long)]
    ledger: String,
    #[arg(long)]
    run_id: Option<String>,
    #[arg(long)]
    role: String,
    #[arg(long)]
    pid: i32,
    /// A substring that must be present in the process's command line, now and at shutdown.
    #[arg(long)]
    marker: String,
}

#[derive(Args, Debug, Clone)]
struct ShutdownArgs {
    #[arg(long)]
    ledger: String,
    /// Actually send the signal. Without this the plan is printed and nothing happens.
    #[arg(long)]
    execute: bool,
    /// `TERM` (default) or `KILL`.
    #[arg(long, default_value = "TERM")]
    signal: String,
}

fn options(a: &LayoutArgs) -> Options {
    let mut o = Options::default();
    o.seed = a.seed.clone();
    if let Some(r) = &a.repo {
        o.repo = r.clone();
        o.root = format!("{r}/runs");
        o.game = format!("{r}/{}", d1_build::selected().launch);
        o.patch = d1_harness::layout::build_patch(r);
    }
    if let Some(r) = &a.root {
        o.root = r.clone();
    }
    if let Some(b) = &a.emulator_bin {
        o.emulator_bin = b.clone();
    }
    if let Some(g) = &a.game {
        o.game = g.clone();
    }
    if let Some(p) = &a.patch {
        o.patch = p.clone();
    }
    o.anchor_a = !a.no_anchor;
    o
}

fn main() {
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    match Cli::parse().command {
        Cmd::Controls => d1_harness::controls::main(),

        Cmd::Pad(a) => match a.action {
            PadAction::Run { fifo, name, id, version } => {
                d1_harness::pad::run_cli(&fifo, &name, &id.vendor, &id.product, &version)
            }
            PadAction::PrintIgnore(id) => {
                d1_harness::pad::print_ignore_cli(&id.vendor, &id.product)
            }
            PadAction::Controls { require_device } => {
                d1_harness::pad::controls_main(require_device)
            }
        },

        Cmd::Layout(a) => {
            let l = layout::build(&options(&a));
            let problems = layout::validate(&l);
            if !problems.is_empty() {
                eprintln!("REFUSING to emit an invalid layout:");
                for p in &problems {
                    eprintln!("  - {p}");
                }
                return 2;
            }
            let json = layout::to_json(&l);
            match a.out {
                None => println!("{json}"),
                Some(path) => {
                    if std::path::Path::new(&path).exists() {
                        eprintln!(
                            "REFUSING to overwrite {path}: a layout is a run's identity, and \
                             replacing one silently makes two runs share it"
                        );
                        return 2;
                    }
                    if let Err(e) = std::fs::write(&path, format!("{json}\n")) {
                        eprintln!("{path}: {e}");
                        return 2;
                    }
                    eprintln!("wrote {path}");
                }
            }
            0
        }

        Cmd::Env(a) => {
            let l = layout::build(&options(&a.layout));
            let Some(p) = l.participants.iter().find(|p| p.slot == a.slot) else {
                eprintln!("no participant '{}'; slots are: a b", a.slot);
                return 2;
            };
            println!("# participant {} - {}", p.slot, p.participant_id);
            println!("# instance {} - netns {} - {}", p.instance_id, p.netns, p.addr);
            println!("# reach: {}", l.reach);
            for (k, v) in &p.env {
                println!("export {k}={}", shell_quote(v));
            }
            println!("# shadPS4 settings for {}/shadPS4/config.json:", p.xdg_data_home);
            for (k, v) in &p.shadps4_settings {
                println!("#   {k} = {v}");
            }
            println!("# users.json shadnet_npid must read: {}", p.np.npid);
            0
        }

        Cmd::Preflight(a) => {
            let l = match &a.layout {
                Some(path) => match std::fs::read_to_string(path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
                {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        return 2;
                    }
                },
                None => layout::build(&options(&a.gen)),
            };
            let mut expect = BTreeMap::new();
            for pair in &a.expect_sha {
                let Some((k, v)) = pair.split_once('=') else {
                    eprintln!("--expect-sha wants role=sha256, got {pair:?}");
                    return 2;
                };
                expect.insert(k.to_string(), v.to_string());
            }
            let stage = match a.stage {
                StageArg::Layout => Stage::Layout,
                StageArg::Namespace => Stage::Namespace,
                StageArg::Arm => Stage::Arm,
            };
            let report = preflight::run(
                &preflight::Input {
                    layout: &l,
                    stage,
                    slot: a.slot.clone(),
                    activation_review: a.activation_review.clone(),
                    expect_sha: expect,
                },
                &preflight::Real,
            );
            println!("run {}   stage {:?}   slot {:?}", l.run_id, stage, a.slot);
            for c in &report.checks {
                match c.verdict {
                    preflight::Verdict::Pass => println!("PASS    {}", c.name),
                    preflight::Verdict::Refuse(r) => println!("REFUSE  {}   [{r}]", c.name),
                }
                if !c.detail.is_empty() {
                    println!("        {}", c.detail);
                }
            }
            if report.ready() {
                println!(
                    "\nREADY - every check passed. NOTHING HAS BEEN STARTED; this tool does not \
                     start anything."
                );
                0
            } else {
                let reasons = report.reasons();
                println!("\nREFUSED - {} reason(s): {reasons:?}", reasons.len());
                3
            }
        }

        Cmd::Pid(a) => {
            let mut ledger = if std::path::Path::new(&a.ledger).exists() {
                match procs::load(&a.ledger) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("{e}");
                        return 2;
                    }
                }
            } else {
                Ledger { run_id: a.run_id.clone().unwrap_or_default(), records: vec![] }
            };
            let rec = match procs::record(&a.role, a.pid, &a.marker, &procs::RealProcs) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("REFUSED: {e}");
                    return 2;
                }
            };
            if let Err(e) = procs::add(&mut ledger, rec.clone()) {
                eprintln!("REFUSED: {e}");
                return 2;
            }
            if let Err(e) = procs::save(&a.ledger, &ledger) {
                eprintln!("REFUSED: {e}");
                return 2;
            }
            println!(
                "recorded {} pid {} starttime {} marker {:?}",
                rec.role, rec.pid, rec.starttime, rec.marker
            );
            0
        }

        Cmd::Shutdown(a) => {
            let ledger = match procs::load(&a.ledger) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{e}");
                    return 2;
                }
            };
            let signal = match a.signal.to_uppercase().as_str() {
                "TERM" | "SIGTERM" => libc::SIGTERM,
                "KILL" | "SIGKILL" => libc::SIGKILL,
                other => {
                    eprintln!(
                        "REFUSED: signal {other:?} is not one of TERM, KILL. A shutdown tool with \
                         an open-ended signal argument is a shell injection with extra steps."
                    );
                    return 2;
                }
            };
            let plan = procs::plan(&ledger, &procs::RealProcs);
            println!("run {}   ledger {}", ledger.run_id, a.ledger);
            for action in &plan {
                match action {
                    Action::Signal { role, pid } => println!("SIGNAL  {role:<16} pid {pid}"),
                    Action::Skip { role, pid, reason } => {
                        let why = match reason {
                            Skip::Exited => "already exited".to_string(),
                            Skip::PidReused { recorded, now } => {
                                format!("PID REUSED - recorded starttime {recorded}, now {now}")
                            }
                            Skip::MarkerMismatch { marker, cmdline } => {
                                format!("marker {marker:?} absent from {cmdline:?}")
                            }
                            Skip::Unsignallable(d) => format!("unsignallable: {d}"),
                        };
                        println!("SKIP    {role:<16} pid {pid}   {why}");
                    }
                }
            }
            if !a.execute {
                println!("\ndry run - nothing was signalled. Add --execute to act on this plan.");
                return 0;
            }
            for line in procs::execute(&plan, signal) {
                println!("{line}");
            }
            0
        }
    }
}

/// Single-quote for a shell, the only way that is safe for arbitrary content.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
