// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The executor: gate -> ledger intent -> execute -> ledger result (`design/d1-ghost.md` §6).
//!
//! Two layers, deliberately split so the whole orchestration is testable with no live client:
//!
//! - [`command_spec`] turns a registry [`Action`](super::registry::Action) into the exact
//!   `d1guest.py` / `session-params.py` invocation, as data. `C-GHOST-NO-LAUNCH` asserts every
//!   program it names is in [`TOOL_WHITELIST`](super::registry::TOOL_WHITELIST): ghost is never a
//!   launcher.
//! - [`apply`] runs the gate, writes the ledger intent BEFORE anything executes, drives a
//!   [`Backend`], and writes the result. The `Backend` is a trait so a recording stub exercises
//!   `C-GHOST-LEDGER-FIRST`, `C-GHOST-FAIL-CLOSED`, `C-GHOST-RUNG-GATE`, and `C-GHOST-READBACK`
//!   headlessly; the live [`SubprocessBackend`] is the only thing that touches a real client.
//!
//! ⛔ The order in [`apply`] is load-bearing and is the property the whole tool exists for: the
//! intent line must hit disk (fsync) BEFORE the write executes. A crash between them is a visible
//! dangling intent, not a silent write.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::ledger::{self, Ledger, Line};
use super::registry::{Action, Poke, ValTy, TOOL_GUEST, TOOL_PARAMS};
use super::Observed;

/// One subprocess invocation, as data - so a control can inspect the program and args without
/// spawning anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl CommandSpec {
    /// Human-readable one-liner for the ledger `target` field and the button hover.
    pub fn display(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
    }
}

/// The `d1guest.py read` spec that reads `len` bytes at a DIRECT poke's target (the readback probe).
/// `Write`/`Patch` get one before and one after; `DerefWrite`/`Launch` have no single readback (a
/// DerefWrite reads each field at its resolved `--abs` address instead).
pub fn read_spec(action: &Action, len: usize) -> Option<CommandSpec> {
    let (target, abs) = match action {
        Action::Write { target, abs, .. } => (*target, *abs),
        Action::Patch { target, abs, .. } => (*target, *abs),
        Action::Launch { .. } | Action::DerefWrite { .. } => return None,
    };
    let mut args = vec!["read".to_string(), target.to_string(), "--len".into(), len.to_string()];
    if abs {
        args.push("--abs".into());
    }
    Some(CommandSpec { program: TOOL_GUEST, args })
}

/// The `d1guest.py poke` spec for a single DIRECT [`Action::Write`]. Every value is hex-prefixed
/// (an operator ledger rule; a bare decimal into `poke` reads as the wrong integer for `0x..` data).
pub fn poke_spec(action: &Action, experiment: &str) -> Option<CommandSpec> {
    let Action::Write { target, abs, offset, ty, value } = action else {
        return None;
    };
    let mut args = vec!["poke".to_string(), target.to_string(), format!("{value:#x}")];
    args.push("--type".into());
    args.push(ty.as_str().into());
    if *offset != 0 {
        args.push("--offset".into());
        args.push(format!("{offset:#x}"));
    }
    if *abs {
        args.push("--abs".into());
    }
    args.push("--experiment".into());
    args.push(experiment.into());
    args.push("--confirm".into());
    Some(CommandSpec { program: TOOL_GUEST, args })
}

/// `d1guest.py read <target> --len 8 [--abs]` - reads the 8-byte pointer at a DerefWrite's base.
pub fn ptr_read_spec(target: &str, abs: bool) -> CommandSpec {
    let mut args = vec!["read".to_string(), target.to_string(), "--len".into(), "8".into()];
    if abs {
        args.push("--abs".into());
    }
    CommandSpec { program: TOOL_GUEST, args }
}

/// `d1guest.py read 0x<addr> --abs --len <n>` - reads a resolved heap address.
pub fn abs_read_spec(addr: u64, len: usize) -> CommandSpec {
    CommandSpec {
        program: TOOL_GUEST,
        args: vec!["read".to_string(), format!("{addr:#x}"), "--abs".into(), "--len".into(), len.to_string()],
    }
}

/// `d1guest.py poke 0x<addr> 0x<value> --abs --type <ty> ...` - writes a resolved heap address.
/// The heap target is `--abs`; the value is hex-prefixed (both operator ledger rules).
pub fn abs_poke_spec(addr: u64, ty: ValTy, value: u64, experiment: &str) -> CommandSpec {
    CommandSpec {
        program: TOOL_GUEST,
        args: vec![
            "poke".to_string(),
            format!("{addr:#x}"),
            format!("{value:#x}"),
            "--abs".into(),
            "--type".into(),
            ty.as_str().into(),
            "--experiment".into(),
            experiment.into(),
            "--confirm".into(),
        ],
    }
}

/// One byte of a [`Action::Patch`], as a `d1guest.py poke` of a u8 at `target --offset i`. A patch
/// is a SEQUENCE of these inside a freeze window (the backend owns the freeze).
pub fn patch_byte_spec(target: &str, abs: bool, i: usize, byte: u8, experiment: &str) -> CommandSpec {
    let mut args = vec!["poke".to_string(), target.to_string(), format!("{byte:#x}")];
    args.push("--type".into());
    args.push("u8".into());
    args.push("--offset".into());
    args.push(format!("{i:#x}"));
    if abs {
        args.push("--abs".into());
    }
    args.push("--experiment".into());
    args.push(experiment.into());
    args.push("--confirm".into());
    CommandSpec { program: TOOL_GUEST, args }
}

/// The `session-params.py --launch-poke` spec.
pub fn launch_spec(action: &Action) -> Option<CommandSpec> {
    let Action::Launch { activity_id } = action else {
        return None;
    };
    Some(CommandSpec {
        program: TOOL_PARAMS,
        args: vec![
            "--launch-poke".into(),
            "--activity-id".into(),
            activity_id.to_string(),
        ],
    })
}

/// Every command spec an action would run - the surface `C-GHOST-NO-LAUNCH` scans. For a patch this
/// enumerates one byte-poke per byte; the freeze signals are NOT subprocess spawns and are not here.
pub fn all_specs(action: &Action, experiment: &str) -> Vec<CommandSpec> {
    match action {
        Action::Write { .. } => [read_spec(action, 8), poke_spec(action, experiment), read_spec(action, 8)]
            .into_iter()
            .flatten()
            .collect(),
        Action::Patch { target, abs, bytes, .. } => {
            let mut v = vec![read_spec(action, bytes.len()).unwrap()];
            for (i, b) in bytes.iter().enumerate() {
                v.push(patch_byte_spec(target, *abs, i, *b, experiment));
            }
            v.push(read_spec(action, bytes.len()).unwrap());
            v
        }
        Action::Launch { .. } => launch_spec(action).into_iter().collect(),
        Action::DerefWrite { target, abs, derefs, fields, .. } => {
            // The resolve reads (one per pointer hop) then, per field, an --abs read/poke/read. All
            // d1guest; the resolved addresses are runtime values, so these carry placeholder 0s -
            // NO-LAUNCH only checks the program.
            let mut v = vec![ptr_read_spec(target, *abs)];
            for _ in *derefs {
                v.push(abs_read_spec(0, 8));
            }
            for f in *fields {
                v.push(abs_read_spec(0, f.ty.width()));
                v.push(abs_poke_spec(0, f.ty, f.value, experiment));
                v.push(abs_read_spec(0, f.ty.width()));
            }
            v
        }
    }
}

/// What a backend does when a poke executes. Split from the command construction so tests drive the
/// orchestration with a recording stub and the live path is the only thing that spawns a process.
pub trait Backend {
    /// Read `len` bytes at a DIRECT action's target (Write/Patch). Returns the raw bytes.
    fn read(&mut self, action: &Action, len: usize) -> std::io::Result<Vec<u8>>;
    /// Apply a DIRECT write (a single typed poke, or a freeze-bracketed byte sequence for a patch).
    /// Returns the subprocess exit code of the last write.
    fn write(&mut self, action: &Action, experiment: &str) -> std::io::Result<i32>;
    /// Leave orbit. Returns the launch tool's exit code.
    fn launch(&mut self, action: &Action) -> std::io::Result<i32>;
    /// Read the 8-byte pointer stored at a DerefWrite's base target. `abs` selects `--abs`.
    fn read_ptr(&mut self, target: &str, abs: bool) -> std::io::Result<u64>;
    /// Read `len` bytes at a resolved absolute heap address (`--abs`).
    fn read_at(&mut self, addr: u64, len: usize) -> std::io::Result<Vec<u8>>;
    /// Write a typed value at a resolved absolute heap address (`--abs`). Returns the exit code.
    fn write_at(&mut self, addr: u64, ty: ValTy, value: u64, experiment: &str) -> std::io::Result<i32>;
}

/// The result of one [`apply`] call, for the GUI and the controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub seq: u64,
    pub outcome: String,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub exit: Option<i32>,
    /// Set when the gate refused; the poke did not execute and no client was touched.
    pub refused_reason: Option<String>,
}

/// Run one registered poke end to end. `ts` and `seq` are supplied by the caller so the crate takes
/// no wall-clock of its own (deterministic controls); the GUI stamps `ts` from `SystemTime` and
/// `seq` from [`ledger::next_seq`].
///
/// ⛔ ORDER (the property this tool exists for):
/// 1. gate -> on refusal, ledger a paired intent+result(refused) and DO NOT execute.
/// 2. ledger the INTENT and fsync it. If that fails, refuse: a poke that cannot be ledgered must
///    not execute (`C-GHOST-FAIL-CLOSED`). The client has not been touched at this point.
/// 3. execute via the backend.
/// 4. ledger the RESULT. A crash before this leaves a visible dangling intent by design.
pub fn apply(
    poke: &Poke,
    obs: &Observed,
    backend: &mut dyn Backend,
    ledger: &Ledger,
    seq: u64,
    ts: &str,
    note: Option<String>,
) -> std::io::Result<Applied> {
    let target_str = describe_target(&poke.action);
    let intent = Line::Intent {
        ts: ts.to_string(),
        seq,
        name: poke.name.to_string(),
        class: poke.class.as_str().to_string(),
        target: target_str.clone(),
        rung: obs.rung,
        note,
    };

    // 1. Gate. A refusal is still ledgered as a paired intent+result so the attempt is not
    //    invisible - "the operator tried to poke below the gate" is a fact worth keeping.
    if let Some(reason) = poke.gate.unmet(obs) {
        ledger.append(&intent)?;
        ledger.append(&Line::Result {
            ts: ts.to_string(),
            seq,
            before: String::new(),
            after: String::new(),
            outcome: ledger::OUTCOME_REFUSED.to_string(),
            exit: None,
        })?;
        return Ok(Applied {
            seq,
            outcome: ledger::OUTCOME_REFUSED.to_string(),
            before: Vec::new(),
            after: Vec::new(),
            exit: None,
            refused_reason: Some(reason),
        });
    }

    // 2. Intent BEFORE execute. Fail closed: if this errors, the client is untouched and we return
    //    the error so the caller renders a refusal, not a warning after a write.
    ledger.append(&intent)?;

    // 3. Execute.
    let (before, after, outcome, exit) = match &poke.action {
        Action::Launch { .. } => {
            let exit = backend.launch(&poke.action)?;
            let outcome = if exit == 0 { ledger::OUTCOME_APPLIED } else { ledger::OUTCOME_FAILED };
            (Vec::new(), Vec::new(), outcome.to_string(), Some(exit))
        }
        Action::DerefWrite { target, abs, derefs, base_offset, fields } => {
            // Resolve the pointer chain to a base heap address, then write each field --abs. A
            // resolve failure is a FAILED result (not a propagated error), so the poke still
            // ledgers a complete pair rather than leaving a dangling intent.
            match resolve(backend, target, *abs, derefs, *base_offset) {
                Ok(base) => {
                    let mut before = Vec::new();
                    let mut after = Vec::new();
                    let mut all_applied = true;
                    let mut last_exit = 0;
                    for f in *fields {
                        let addr = base.wrapping_add(f.offset);
                        let b = backend.read_at(addr, f.ty.width()).unwrap_or_default();
                        let exit = backend.write_at(addr, f.ty, f.value, poke.name)?;
                        let a = backend.read_at(addr, f.ty.width()).unwrap_or_default();
                        last_exit = exit;
                        if exit != 0 || a != field_bytes(f).as_slice() {
                            all_applied = false;
                        }
                        before.extend(b);
                        after.extend(a);
                    }
                    let outcome = if all_applied {
                        ledger::OUTCOME_APPLIED
                    } else if last_exit != 0 {
                        ledger::OUTCOME_FAILED
                    } else {
                        ledger::OUTCOME_MISMATCH
                    };
                    (before, after, outcome.to_string(), Some(last_exit))
                }
                Err(_) => (Vec::new(), Vec::new(), ledger::OUTCOME_FAILED.to_string(), None),
            }
        }
        action => {
            let len = write_width(action);
            let before = backend.read(action, len).unwrap_or_default();
            let exit = backend.write(action, poke.name)?;
            let after = backend.read(action, len).unwrap_or_default();
            let outcome = classify(action, &after, exit);
            (before, after, outcome, Some(exit))
        }
    };

    // 4. Result.
    ledger.append(&Line::Result {
        ts: ts.to_string(),
        seq,
        before: hex(&before),
        after: hex(&after),
        outcome: outcome.clone(),
        exit,
    })?;

    Ok(Applied {
        seq,
        outcome,
        before,
        after,
        exit,
        refused_reason: None,
    })
}

/// Resolve a [`Action::DerefWrite`] pointer chain to its base heap address:
/// `addr = *(target)`; then `addr = *(addr + o)` for each `o` in `derefs`; then `+ base_offset`.
fn resolve(
    backend: &mut dyn Backend,
    target: &str,
    abs: bool,
    derefs: &[u64],
    base_offset: u64,
) -> std::io::Result<u64> {
    let mut addr = backend.read_ptr(target, abs)?;
    for o in derefs {
        let b = backend.read_at(addr.wrapping_add(*o), 8)?;
        if b.len() < 8 {
            return Err(std::io::Error::other("deref read returned fewer than 8 bytes"));
        }
        addr = u64::from_le_bytes(b[..8].try_into().unwrap());
    }
    if addr == 0 {
        return Err(std::io::Error::other("pointer chain resolved to NULL - refusing to write"));
    }
    Ok(addr.wrapping_add(base_offset))
}

/// The little-endian bytes one DerefWrite field is supposed to land - the readback oracle.
fn field_bytes(f: &super::registry::WriteField) -> Vec<u8> {
    match f.ty {
        ValTy::U8 => vec![f.value as u8],
        ValTy::U16 => (f.value as u16).to_le_bytes().to_vec(),
        ValTy::U32 => (f.value as u32).to_le_bytes().to_vec(),
        ValTy::U64 => f.value.to_le_bytes().to_vec(),
    }
}

/// The width, in bytes, of the readback a write/patch verifies.
fn write_width(action: &Action) -> usize {
    match action {
        Action::Write { ty, .. } => ty.width(),
        Action::Patch { bytes, .. } => bytes.len(),
        // DerefWrite is executed field-by-field, never through this width; 0 is unused.
        Action::Launch { .. } | Action::DerefWrite { .. } => 0,
    }
}

/// ⛔ C-GHOST-READBACK: readback != intended is a MISMATCH; no path maps mismatch to applied.
fn classify(action: &Action, after: &[u8], exit: i32) -> String {
    if exit != 0 {
        return ledger::OUTCOME_FAILED.to_string();
    }
    let intended = intended_bytes(action);
    if after == intended.as_slice() {
        ledger::OUTCOME_APPLIED.to_string()
    } else {
        ledger::OUTCOME_MISMATCH.to_string()
    }
}

/// The exact little-endian bytes a write is supposed to land - the oracle the readback is checked
/// against.
pub fn intended_bytes(action: &Action) -> Vec<u8> {
    match action {
        Action::Write { ty, value, .. } => match ty {
            ValTy::U8 => vec![*value as u8],
            ValTy::U16 => (*value as u16).to_le_bytes().to_vec(),
            ValTy::U32 => (*value as u32).to_le_bytes().to_vec(),
            ValTy::U64 => value.to_le_bytes().to_vec(),
        },
        Action::Patch { bytes, .. } => bytes.to_vec(),
        Action::Launch { .. } => Vec::new(),
        Action::DerefWrite { fields, .. } => {
            let mut v = Vec::new();
            for f in *fields {
                v.extend(field_bytes(f));
            }
            v
        }
    }
}

/// A short human-readable target for the ledger.
fn describe_target(action: &Action) -> String {
    match action {
        Action::Write { target, offset, ty, value, .. } => {
            format!("{target}+{offset:#x} = {value:#x} ({})", ty.as_str())
        }
        Action::Patch { target, bytes, .. } => {
            format!("{target} <- {}-byte patch", bytes.len())
        }
        Action::Launch { activity_id } => format!("launch activity {activity_id}"),
        Action::DerefWrite { target, fields, .. } => {
            format!("*{target} <- {}-field write (resolved, --abs)", fields.len())
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The live backend: spawns `d1guest.py` and `session-params.py` under the project's Python. The
/// only [`Backend`] that touches a real client.
///
/// ⛔ Freeze-write-thaw for patches: a torn multi-byte patch on a live thread crashes the client
/// (`design/GUARDIAN-IN-WORLD.md`). The backend SIGSTOPs the emulator, writes each byte, then
/// SIGCONTs. If it has no pid to freeze, a patch REFUSES rather than tearing - fail closed.
pub struct SubprocessBackend {
    /// Repository root, so the relative tool paths resolve.
    pub repo_root: PathBuf,
    /// The emulator pid to freeze around a patch. `None` disables patches (they refuse).
    pub emu_pid: Option<u32>,
    /// Extra args threaded onto every `d1guest.py` call (e.g. `--pid`, liveness overrides).
    pub guest_common: Vec<String>,
}

impl SubprocessBackend {
    fn run(&self, spec: &CommandSpec) -> std::io::Result<(i32, Vec<u8>)> {
        let mut cmd = Command::new("python3");
        cmd.arg(self.repo_root.join(spec.program));
        // d1guest common args go right after the subcommand; simplest correct placement is to
        // append them, which argparse accepts for optional flags.
        cmd.args(&spec.args);
        if spec.program == TOOL_GUEST {
            cmd.args(&self.guest_common);
        }
        cmd.current_dir(&self.repo_root);
        let out = cmd.output()?;
        let code = out.status.code().unwrap_or(-1);
        Ok((code, out.stdout))
    }

    /// Send a signal to the emulator pid via `kill`. Not a spawn of a launcher; process control.
    fn signal(&self, sig: &str) -> std::io::Result<()> {
        let Some(pid) = self.emu_pid else {
            return Err(std::io::Error::other("no emulator pid to freeze around a patch"));
        };
        let status = Command::new("kill").arg(sig).arg(pid.to_string()).status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!("kill {sig} {pid} failed")));
        }
        Ok(())
    }
}

impl Backend for SubprocessBackend {
    fn read(&mut self, action: &Action, len: usize) -> std::io::Result<Vec<u8>> {
        let spec = read_spec(action, len).ok_or_else(|| std::io::Error::other("no readback for a launch"))?;
        let (_code, stdout) = self.run(&spec)?;
        Ok(parse_hexdump(&String::from_utf8_lossy(&stdout), len))
    }

    fn write(&mut self, action: &Action, experiment: &str) -> std::io::Result<i32> {
        match action {
            Action::Write { .. } => {
                let spec = poke_spec(action, experiment).unwrap();
                Ok(self.run(&spec)?.0)
            }
            Action::Patch { target, abs, bytes, .. } => {
                // Freeze, write every byte, thaw. If freeze fails, refuse before any byte lands.
                self.signal("-STOP")?;
                let mut last = 0;
                let mut err = None;
                for (i, b) in bytes.iter().enumerate() {
                    let spec = patch_byte_spec(target, *abs, i, *b, experiment);
                    match self.run(&spec) {
                        Ok((code, _)) => last = code,
                        Err(e) => {
                            err = Some(e);
                            break;
                        }
                    }
                }
                // Always thaw, even on error - a frozen emulator is worse than a torn patch.
                let cont = self.signal("-CONT");
                if let Some(e) = err {
                    return Err(e);
                }
                cont?;
                Ok(last)
            }
            Action::Launch { .. } => Err(std::io::Error::other("launch is not a write")),
            // apply() resolves a DerefWrite and calls write_at, so it never reaches here.
            Action::DerefWrite { .. } => Err(std::io::Error::other(
                "DerefWrite is resolved by apply() and applied via write_at, not executed directly",
            )),
        }
    }

    fn launch(&mut self, action: &Action) -> std::io::Result<i32> {
        let spec = launch_spec(action).ok_or_else(|| std::io::Error::other("not a launch action"))?;
        let mut cmd = Command::new("python3");
        cmd.arg(self.repo_root.join(spec.program)).args(&spec.args).current_dir(&self.repo_root);
        Ok(cmd.status()?.code().unwrap_or(-1))
    }

    fn read_ptr(&mut self, target: &str, abs: bool) -> std::io::Result<u64> {
        let (_code, stdout) = self.run(&ptr_read_spec(target, abs))?;
        let bytes = parse_hexdump(&String::from_utf8_lossy(&stdout), 8);
        if bytes.len() < 8 {
            return Err(std::io::Error::other(format!(
                "pointer read at {target} returned {} bytes, not 8",
                bytes.len()
            )));
        }
        Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
    }

    fn read_at(&mut self, addr: u64, len: usize) -> std::io::Result<Vec<u8>> {
        let (_code, stdout) = self.run(&abs_read_spec(addr, len))?;
        Ok(parse_hexdump(&String::from_utf8_lossy(&stdout), len))
    }

    fn write_at(&mut self, addr: u64, ty: ValTy, value: u64, experiment: &str) -> std::io::Result<i32> {
        Ok(self.run(&abs_poke_spec(addr, ty, value, experiment))?.0)
    }
}

/// Pull the first `len` bytes out of a `d1guest.py` hexdump block. The dump line shape is
/// `  <addr>  <hex bytes ...>  <ascii>` (`hexdump()` in d1guest.py). We take the hex column of the
/// first data line and truncate to `len`. Best-effort: on any surprise, returns what it parsed.
pub fn parse_hexdump(text: &str, len: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for line in text.lines() {
        // A data line starts with two spaces then a 0x-address; the hex column is the run of
        // `[0-9a-f ]` after the address, before the two-space gutter to the ascii column.
        let t = line.trim_start();
        if !t.starts_with("0x") {
            continue;
        }
        // After the address token, the rest up to the double-space gutter is the hex.
        let Some(rest) = t.split_once("  ").map(|(_, r)| r) else { continue };
        let hexcol = rest.split("  ").next().unwrap_or("");
        for tok in hexcol.split_whitespace() {
            if let Ok(b) = u8::from_str_radix(tok, 16) {
                out.push(b);
                if out.len() >= len {
                    return out;
                }
            } else {
                break;
            }
        }
    }
    out.truncate(len);
    out
}

/// The repo-relative tool path check `C-GHOST-NO-LAUNCH` uses when it also wants to confirm the
/// files exist on disk (the live-attach smoke check). Pure so it can run headless.
pub fn tool_paths(repo_root: &Path) -> Vec<PathBuf> {
    super::registry::TOOL_WHITELIST.iter().map(|t| repo_root.join(t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost::registry::{find, Class, Gate};

    /// A recording backend: no client, canned readbacks, records every call so the orchestration
    /// controls run headlessly.
    struct Rec {
        /// Bytes `read`/`read_at` return; the executor reads twice (before/after) so we pop front.
        reads: Vec<Vec<u8>>,
        write_exit: i32,
        /// The pointer `read_ptr` resolves to (the DerefWrite base).
        ptr: u64,
        calls: Vec<String>,
    }
    impl Rec {
        fn new(reads: Vec<Vec<u8>>, write_exit: i32) -> Rec {
            Rec { reads, write_exit, ptr: 0x1000, calls: Vec::new() }
        }
    }
    impl Backend for Rec {
        fn read(&mut self, _a: &Action, _len: usize) -> std::io::Result<Vec<u8>> {
            self.calls.push("read".into());
            Ok(if self.reads.is_empty() { Vec::new() } else { self.reads.remove(0) })
        }
        fn write(&mut self, _a: &Action, _e: &str) -> std::io::Result<i32> {
            self.calls.push("write".into());
            Ok(self.write_exit)
        }
        fn launch(&mut self, _a: &Action) -> std::io::Result<i32> {
            self.calls.push("launch".into());
            Ok(0)
        }
        fn read_ptr(&mut self, _t: &str, _abs: bool) -> std::io::Result<u64> {
            self.calls.push("read_ptr".into());
            Ok(self.ptr)
        }
        fn read_at(&mut self, _addr: u64, _len: usize) -> std::io::Result<Vec<u8>> {
            self.calls.push("read_at".into());
            Ok(if self.reads.is_empty() { Vec::new() } else { self.reads.remove(0) })
        }
        fn write_at(&mut self, _addr: u64, _ty: ValTy, _v: u64, _e: &str) -> std::io::Result<i32> {
            self.calls.push("write_at".into());
            Ok(self.write_exit)
        }
    }

    fn tmp_run(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d1-ghost-exec-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn live_obs() -> Observed {
        Observed { rung: Some(36), state: "activity:in_world".into(), liveness: 3 }
    }

    /// C-GHOST-NO-LAUNCH: every command spec any registry entry would run names a whitelisted tool.
    #[test]
    fn no_spec_escapes_the_whitelist() {
        for p in super::super::registry::registry() {
            for spec in all_specs(&p.action, p.name) {
                assert!(
                    super::super::registry::TOOL_WHITELIST.contains(&spec.program),
                    "{} spawns non-whitelisted {}",
                    p.name,
                    spec.program
                );
            }
        }
    }

    /// C-GHOST-READBACK: a matching readback is applied; a wrong one is mismatch, never applied.
    #[test]
    fn readback_mismatch_is_never_applied() {
        let p = find("done-mask-7ff").unwrap(); // u32 = 0x7FF
        let d = tmp_run("readback");
        let led = Ledger::for_run(&d);
        // after-read returns exactly the intended bytes -> applied.
        let mut good = Rec::new(vec![vec![0, 0, 0, 0], vec![0xff, 0x07, 0, 0]], 0);
        let a = apply(&p, &live_obs(), &mut good, &led, 0, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_APPLIED);
        // after-read returns something else -> mismatch.
        let mut bad = Rec::new(vec![vec![0, 0, 0, 0], vec![0xde, 0xad, 0, 0]], 0);
        let a = apply(&p, &live_obs(), &mut bad, &led, 1, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_MISMATCH, "a wrong readback must not read applied");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// C-GHOST-LEDGER-FIRST: the intent line is on disk before the write call runs. We prove the
    /// order by checking that when the ledger append fails (missing dir), the backend is NEVER
    /// asked to write - execute cannot precede a durable intent.
    #[test]
    fn intent_precedes_execute() {
        let missing = std::env::temp_dir().join(format!("d1-ghost-noexist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        let led = Ledger::for_run(&missing);
        let p = find("done-mask-7ff").unwrap();
        let mut rec = Rec::new(vec![], 0);
        let r = apply(&p, &live_obs(), &mut rec, &led, 0, "2026-08-11T00:00:00Z", None);
        assert!(r.is_err(), "a poke whose intent cannot be ledgered must return an error");
        assert!(rec.calls.is_empty(), "the client was touched before a durable intent existed");
    }

    /// C-GHOST-FAIL-CLOSED (paired with the ledger test): a refused gate still ledgers a paired
    /// intent+result and never calls the backend.
    #[test]
    fn a_gated_poke_refuses_without_touching_the_client() {
        let d = tmp_run("gated");
        let led = Ledger::for_run(&d);
        let hook = find("hook-0010c500").unwrap(); // rung-36 gate
        let too_early = Observed { rung: Some(30), state: "setup:orbit".into(), liveness: 3 };
        let mut rec = Rec::new(vec![], 0);
        let a = apply(&hook, &too_early, &mut rec, &led, 0, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_REFUSED);
        assert!(a.refused_reason.unwrap().contains("rung"));
        assert!(rec.calls.is_empty(), "a refused poke must not touch the client");
        // ...and the refusal is ledgered as a complete pair - not a dangling intent.
        let lines: Vec<Line> = led.read_all().into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert!(ledger::dangling(&lines).is_empty(), "a refusal is a complete pair, not dangling");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The launch path ledgers before/after empty and outcome applied on exit 0 - but ONLY at
    /// orbit. In-world it must refuse (the window has an upper edge, or a re-launch strands the
    /// client).
    #[test]
    fn launch_ledgers_a_clean_pair_at_orbit_only() {
        let d = tmp_run("launch");
        let led = Ledger::for_run(&d);
        let p = find("launch-407").unwrap();
        assert_eq!(p.class, Class::Launch);
        // At orbit (rung 27): applied.
        let orbit = Observed { rung: Some(27), state: "setup:orbit".into(), liveness: 3 };
        let mut rec = Rec::new(vec![], 0);
        let a = apply(&p, &orbit, &mut rec, &led, 0, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_APPLIED);
        assert_eq!(rec.calls, vec!["launch"]);
        // In-world (rung 36): refused, client untouched - this is the strand-the-game fix.
        let mut rec2 = Rec::new(vec![], 0);
        let b = apply(&p, &live_obs(), &mut rec2, &led, 1, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(b.outcome, ledger::OUTCOME_REFUSED);
        assert!(b.refused_reason.unwrap().contains("window"));
        assert!(rec2.calls.is_empty(), "an out-of-window launch must not fire");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The hexdump parser recovers the exact bytes from a real d1guest.py dump line.
    #[test]
    fn hexdump_parse_recovers_bytes() {
        let dump = "guest 0x26b5730 -> host 0x38000000, 4 bytes\n  0x0038000000  ff 07 00 00                                       ....\n";
        assert_eq!(parse_hexdump(dump, 4), vec![0xff, 0x07, 0x00, 0x00]);
    }

    /// A patch enumerates one byte-poke per byte, all u8, plus two readbacks - and stays in the
    /// whitelist. (The freeze signals are not subprocess specs.)
    #[test]
    fn a_patch_is_a_byte_sequence() {
        let hook = find("je-411e16-nop").unwrap();
        let specs = all_specs(&hook.action, hook.name);
        // read + 6 byte-pokes + read
        assert_eq!(specs.len(), 1 + 6 + 1);
        for s in &specs[1..specs.len() - 1] {
            assert!(s.args.iter().any(|a| a == "u8"), "patch byte must be a u8 poke");
        }
    }

    /// A MultiWrite (the fade lever) applies every field and is APPLIED only if all read back;
    /// one wrong field makes the whole poke a MISMATCH, and it ledgers as ONE pair. The fade is a
    /// DerefWrite: the pointer is resolved once (read_ptr), then each field is read/written --abs.
    #[test]
    fn a_derefwrite_is_one_poke_all_or_nothing() {
        let p = find("clear-fade").unwrap();
        let d = tmp_run("derefwrite");
        let led = Ledger::for_run(&d);
        // read_at sequence (no deref hops): before(=old), after(=0) per field, interleaved.
        let z = vec![0u8, 0, 0, 0];
        let old = vec![1u8, 0, 0, 0];
        let mut all_ok = Rec::new(
            vec![old.clone(), z.clone(), old.clone(), z.clone(), old.clone(), z.clone()],
            0,
        );
        let a = apply(&p, &live_obs(), &mut all_ok, &led, 0, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_APPLIED);
        assert_eq!(a.after.len(), 12, "three u32 fields readback concatenated");
        assert_eq!(all_ok.calls.iter().filter(|c| *c == "read_ptr").count(), 1, "pointer resolved once");
        assert_eq!(all_ok.calls.iter().filter(|c| *c == "write_at").count(), 3, "each field --abs");
        // Exactly one intent + one result on disk.
        let lines: Vec<Line> = led.read_all().into_iter().flatten().collect();
        assert_eq!(lines.len(), 2, "a multi-field lever is ONE ledgered poke");
        // One field reads back wrong -> the whole poke is a mismatch.
        let mut one_bad = Rec::new(
            vec![old.clone(), z.clone(), old.clone(), vec![9, 0, 0, 0], old.clone(), z.clone()],
            0,
        );
        let b = apply(&p, &live_obs(), &mut one_bad, &led, 1, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(b.outcome, ledger::OUTCOME_MISMATCH, "one wrong field fails the whole lever");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A double-deref (spawn-word-3) resolves through two pointer hops before writing --abs.
    #[test]
    fn a_double_deref_resolves_two_hops() {
        let p = find("spawn-word-3").unwrap();
        let d = tmp_run("doublederef");
        let led = Ledger::for_run(&d);
        // read_ptr gives p; one deref hop reads a pointer (8 bytes); then field before/after.
        let ptr_bytes = 0x2000u64.to_le_bytes().to_vec();
        let mut rec = Rec::new(vec![ptr_bytes, vec![0, 0, 0, 0], vec![3, 0, 0, 0]], 0);
        let a = apply(&p, &live_obs(), &mut rec, &led, 0, "2026-08-11T00:00:00Z", None).unwrap();
        assert_eq!(a.outcome, ledger::OUTCOME_APPLIED);
        assert_eq!(rec.calls.iter().filter(|c| *c == "read_ptr").count(), 1);
        // one deref hop (read_at for 8 bytes) + before + after = 3 read_at calls.
        assert_eq!(rec.calls.iter().filter(|c| *c == "read_at").count(), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Gate sanity used by the RUNG-GATE control at the exec layer.
    #[test]
    fn gate_unmet_is_the_gate() {
        let g = Gate { min_rung: Some(36), max_rung: None, min_liveness: 3 };
        assert!(g.unmet(&Observed { rung: Some(36), state: String::new(), liveness: 3 }).is_none());
        assert!(g.unmet(&Observed { rung: Some(36), state: String::new(), liveness: 2 }).is_some());
    }
}
