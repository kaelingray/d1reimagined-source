// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-bap` - the table generator and its controls, on the command line.
//!
//! This is the CLI face of [`d1_bap::derive`] (the port of `capture-rig/bap-message-idspace.py`
//! and `capture-rig/bap-message-table.py`). The protocol code itself is the library; this binary
//! only re-derives the tables from the pinned eboot and checks them against the consts.
//!
//! ```text
//! d1-bap controls [--eboot PATH] [--defs PATH]     # every control + drift check; EXIT 2 ON FAILURE
//! d1-bap message-table [--tsv PATH]                # the 58-entry transport registry
//! d1-bap idspace [--tsv PATH]                      # the 101-row investment definition table
//! d1-bap policies                                  # the 33 policy families (request -> response)
//! ```
//!
//! Exit 2 means a control failed and **no output of this tool may be cited**.

use std::process::ExitCode;

use d1_bap::derive::{self, Reader, Thunk};
use d1_eboot::descriptors;
use d1_eboot::image::PINNED_EBOOT;

const USAGE: &str = "\
d1-bap - derivation and controls for the BAP tables (d1-bap/src/tables.rs)

USAGE:
    d1-bap controls      [--eboot PATH] [--defs PATH]   run every control and drift check; exit 2 on failure
    d1-bap message-table [--eboot PATH] [--tsv PATH]    the 58-entry transport message registry
    d1-bap idspace       [--eboot PATH] [--defs PATH] [--tsv PATH]
                                                        the 101-row investment message definition table
    d1-bap policies      [--eboot PATH]                 the 33 message-policy families

Defaults: the pinned 01.29 eboot and capture-rig/schemas/investment-message-definitions.bin.
--tsv writes the byte-identical successor of the Python tool's TSV output.
Every dump runs its controls first and REFUSES (exit 2) if any fail, as the Python tools did.
";

struct Args {
    cmd: String,
    eboot: String,
    defs: String,
    tsv: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut argv = std::env::args().skip(1);
    let cmd = argv.next().ok_or(USAGE.to_string())?;
    if cmd == "--help" || cmd == "-h" || cmd == "help" {
        return Err(USAGE.to_string());
    }
    let mut args = Args {
        cmd,
        eboot: PINNED_EBOOT.to_string(),
        defs: derive::PINNED_DEFS.to_string(),
        tsv: None,
    };
    while let Some(flag) = argv.next() {
        let mut value = || argv.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--eboot" => args.eboot = value()?,
            "--defs" => args.defs = value()?,
            "--tsv" => args.tsv = Some(value()?),
            other => return Err(format!("unknown flag {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if msg == USAGE {
                // Asking for help is not a failed control.
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

/// Refuse to emit anything on a failing control - the refusal both Python tools carried (they ran
/// their controls before EVERY dump; `bap-message-table.py --tsv` printed "refusing to write on a
/// failing control" and exited 2). A dump from an image that fails its controls is a confident
/// wrong answer, and this tool must never produce one.
fn gate(controls: &[derive::Control]) -> Result<(), String> {
    let failed: Vec<&derive::Control> = controls.iter().filter(|c| !c.ok).collect();
    if failed.is_empty() {
        return Ok(());
    }
    for c in &failed {
        eprintln!("FAIL  {}  [{}]", c.label, c.detail);
    }
    Err(format!(
        "{} control(s) failed - refusing to emit (run `d1-bap controls` for the full list)",
        failed.len()
    ))
}

fn run(args: &Args) -> Result<ExitCode, String> {
    let reader = Reader::open(&args.eboot)?;
    match args.cmd.as_str() {
        "controls" => {
            let defs_data = std::fs::read(&args.defs).ok();
            let defs = match defs_data.as_deref() {
                Some(d) => Some(derive::read_definitions(d)?),
                None => None,
            };
            let controls = derive::run_controls(&reader, defs.as_deref());
            let mut failed = 0;
            for c in &controls {
                println!("{}  {}  [{}]", if c.ok { "PASS" } else { "FAIL" }, c.label, c.detail);
                failed += usize::from(!c.ok);
            }
            if failed == 0 {
                println!("\nALL {} CONTROLS PASS", controls.len());
                Ok(ExitCode::SUCCESS)
            } else {
                println!("\n{failed} CONTROL(S) FAILED - output may not be cited");
                Ok(ExitCode::from(2))
            }
        }
        "message-table" => {
            gate(&derive::run_eboot_controls(&reader))?;
            let rows = derive::transport_registry(&reader)?;
            if let Some(path) = &args.tsv {
                std::fs::write(path, derive::message_table_tsv(&rows)).map_err(|e| e.to_string())?;
                println!("wrote {path} ({} rows)", rows.len());
                return Ok(ExitCode::SUCCESS);
            }
            println!("{:>5} {:>7} {:>5} {:>10}  class", "id", "hex", "hclass", "vtable");
            let mut sorted: Vec<_> = rows.iter().collect();
            sorted.sort_by_key(|m| m.id.value());
            for m in sorted {
                let id = m.id.value().unwrap_or(0);
                let class = match m.header_class {
                    Thunk::Mov(v) => v.to_string(),
                    Thunk::Xor => "0".to_string(),
                    Thunk::Other => "?".to_string(),
                };
                println!("{id:>5} {id:>#7x} {class:>5} {:>#10x}  {}", m.vtable, m.short);
            }
            println!("\n{} messages. Slot 1 is the HEADER CLASS (0..=7), not a category.", rows.len());
            Ok(ExitCode::SUCCESS)
        }
        "idspace" => {
            let data = std::fs::read(&args.defs).map_err(|e| format!("{}: {e}", args.defs))?;
            let defs = derive::read_definitions(&data)?;
            gate(&derive::run_controls(&reader, Some(&defs)))?;
            let idx = descriptors::index(reader.image()).map_err(|e| e.to_string())?;
            let tsv = derive::idspace_tsv(reader.image(), &idx, &defs);
            match &args.tsv {
                Some(path) => {
                    std::fs::write(path, tsv).map_err(|e| e.to_string())?;
                    println!("wrote {path} ({} rows)", defs.len());
                }
                None => print!("{tsv}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        "policies" => {
            gate(&derive::run_eboot_controls(&reader))?;
            println!("{:>3} {:>4} {:>5}  rtti class", "idx", "req", "rsp");
            for f in derive::policy_families(&reader) {
                let short = f
                    .rtti_class
                    .as_deref()
                    .map(|c| c.trim_start_matches("c_investment_bap_message_policy_").to_string())
                    .unwrap_or_else(|| "?".into());
                let rsp = match f.response.value() {
                    Some(0) => "-".to_string(),
                    Some(v) => v.to_string(),
                    None => "?".to_string(),
                };
                let req = f.request.value().map(|v| v.to_string()).unwrap_or_else(|| "?".into());
                println!("{:>3} {req:>4} {rsp:>5}  {short}", f.index);
            }
            println!("\nrsp '-' means fire-and-forget (slot 6 is xor eax,eax).");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}
