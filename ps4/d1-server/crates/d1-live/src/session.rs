// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `open_guest` and `print_liveness` - **the gate every read-side command goes through**.
//!
//! This is `d1guest.py`'s `open_guest()` in Rust, and it is a single function on purpose. The
//! refusals it carries are the most valuable thing in that script, and a command that skipped one
//! would produce a plausible-looking number rather than an error:
//!
//! 1. **BUILD GATE.** The symbol file records the SHA-256 of the eboot it was measured against. A
//!    different image means every vaddr in the file is suspect, so the read is refused rather than
//!    performed against a build nobody checked. `--ignore-build-gate` exists and says, in its own
//!    message, that you are asserting you re-verified the addresses yourself.
//! 2. **ONE emulator, or none.** Zero is an ERROR, not an empty result - a runtime read with no
//!    runtime has no answer. Two is a refusal to guess: one emulator, one owner (ATLAS §4.6).
//! 3. **THE THREE BASE CONTROLS.** ⚠ A wrong base yields plausible-looking bytes rather than an
//!    error. Two independent anchors must agree on exactly one base; a third address must compare
//!    equal by direct read; and that same window shifted one byte must differ. Any FAIL exits 2
//!    **without reading anything**.
//! 4. **THE LIVENESS LADDER**, printed by every command, and *required* at a level each command
//!    chooses. Registry objects are cleared on connection teardown while descriptors survive
//!    (ATLAS §4.6b), so a late read of an object shows zeros indistinguishable from "our delivery
//!    never worked". That mistake has been made twice. `--allow-dead` reads anyway and tells you to
//!    label the result.

use std::path::PathBuf;

use anyhow::{bail, Result};
use d1_eboot::Image;

use crate::proc::{self as live, Liveness, Proc};
use crate::registry::Guest;
use crate::symbols::Symbols;

/// A refusal that carries the exit code `d1guest.py` would have used.
///
/// ⚠ **1 and 2 are different facts and the skills branch on them.** In the Python, a
/// `SystemExit("message")` exits **1** (an ordinary refusal: no client, a bad symbol, an index past
/// the capacity) while `SystemExit(2)` exits **2** (a *control* refused: the base did not verify,
/// the two-derivation cross-check disagreed, no element carries that key). Collapsing them would
/// make "the tool said no" indistinguishable from "the measurement is not citable".
#[derive(Debug)]
pub struct Refusal {
    pub code: i32,
    pub msg: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}
impl std::error::Error for Refusal {}

/// A control-grade refusal: exit **2**, and the output above it may not be cited.
pub fn control_refusal(msg: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(Refusal { code: 2, msg: msg.into() })
}

/// The exit code an error chain asks for: 3 if the client died, 2 if a control refused, else 1.
pub fn exit_code(e: &anyhow::Error) -> i32 {
    if crate::proc::is_dead(e) {
        return 3;
    }
    e.chain()
        .find_map(|c| c.downcast_ref::<Refusal>().map(|r| r.code))
        .unwrap_or(1)
}

/// The options every read-side command shares - the flags `d1guest.py` puts on its top-level parser.
#[derive(Debug, Clone, Default)]
pub struct Opts {
    pub pid: Option<i32>,
    pub eboot: Option<PathBuf>,
    pub symbols: Option<PathBuf>,
    pub ignore_build_gate: bool,
    pub full_scan: bool,
    pub no_cache: bool,
    pub allow_dead: bool,
    pub account_soid: Option<String>,
    /// Collect the provenance without printing it.
    ///
    /// ⚠ For LIBRARY callers only - the controls open sessions that are *expected* to refuse, and a
    /// library that writes to stdout turns a control report into an interleaved mess. The CLI never
    /// sets this: a resolution nobody saw is a resolution nobody checked.
    pub quiet: bool,
}

/// A proven client: an image, a symbol file, a process, and a base that passed all three controls.
pub struct Session {
    pub image: Image,
    pub syms: Symbols,
    pub proc: Proc,
    pub base: u64,
    /// The provenance lines, in order - already PRINTED by [`Session::open`], and kept only so a
    /// caller that needs them again (a snapshot manifest, a JSON mode) does not have to re-derive.
    pub provenance: Vec<String>,
    allow_dead: bool,
    account_soid: u64,
}

impl Session {
    /// Open the one live client and prove the base.
    ///
    /// ⚠ **The provenance is printed HERE, as it is produced, not returned for the caller to print.**
    /// Measured the hard way: collecting it and printing it after `open` returned `Ok` meant a
    /// refusal printed *nothing at all* - the BUILD GATE line and the failing BASE CONTROL block,
    /// the only evidence for why the tool said no, were dropped on exactly the path where they
    /// matter. `d1guest.py` prints them inside `open_guest` for the same reason.
    pub fn open(o: &Opts) -> Result<Self> {
        let syms = match &o.symbols {
            Some(p) => Symbols::open(p)?,
            None => Symbols::open_default()?,
        };
        let image = match &o.eboot {
            Some(p) => live::open_image(Some(p.clone()))?,
            None => live::open_image(None)?,
        };
        let mut provenance = Vec::new();

        let (ok, line) = syms.check_build(&image.sha256());
        if !o.quiet {
            println!("{line}");
        }
        provenance.push(line);
        if !ok && !o.ignore_build_gate {
            bail!(
                "refusing: the symbol file was measured against a different eboot. Every vaddr in \
                 it is suspect. Pass --ignore-build-gate only if you have re-verified the addresses \
                 yourself."
            );
        }

        let pid = match o.pid {
            Some(p) => p,
            None => {
                let pids = live::emulator_pids()?;
                match pids.len() {
                    1 => pids[0],
                    0 => bail!(
                        "no running shadps4 (pgrep -f '[s]hadps4 -p ') - a runtime read with no \
                         runtime is an ERROR, not an empty result"
                    ),
                    n => bail!(
                        "{n} emulators running ({pids:?}); refusing to guess. One emulator, one \
                         owner (ATLAS §4.6)."
                    ),
                }
            }
        };
        let proc = Proc::open(pid)?;

        let ctl = live::recover_base_cached(&proc, &image, &syms, o.full_scan, !o.no_cache);
        let report = ctl.report();
        if !o.quiet {
            println!("{report}");
        }
        provenance.push(report);
        let Some(base) = ctl.base.filter(|_| ctl.ok()) else {
            // ⛔ Exit 2, as `d1guest.py` does: this is a CONTROL refusing, so nothing printed above
            // it may be cited. ⚠ A WRONG BASE YIELDS PLAUSIBLE-LOOKING BYTES RATHER THAN AN ERROR,
            // which is why it is a refusal and not a warning.
            return Err(control_refusal(
                "base control FAILED - refusing to read. A wrong base yields plausible-looking \
                 bytes rather than an error.",
            ));
        };

        let account_soid = match &o.account_soid {
            Some(s) => parse_u64(s)?,
            None => syms.account_soid,
        };
        Ok(Session { image, syms, proc, base, provenance, allow_dead: o.allow_dead, account_soid })
    }

    pub fn guest(&self) -> Guest<'_> {
        Guest::new(&self.proc, &self.image, &self.syms, self.base)
    }

    /// The ladder, its rendered text, and the refusal it earns - **as three separate values**.
    ///
    /// ⚠ The split matters for output fidelity: `d1guest.py` PRINTS the whole ladder to stdout and
    /// only then raises. A port that folded the ladder into the error message would move it to
    /// stderr, and a reader piping stdout would see a refusal with no evidence attached.
    ///
    /// `require` differs per command and the differences are load-bearing: `read` needs L2 by
    /// default (the registry exists), `obj` needs **L3** (registry object *contents* mean
    /// something), and `tag`/`descriptors` need **L0** - descriptors survive the teardown that
    /// clears objects (ATLAS §4.6b).
    pub fn liveness(&self, require: u8) -> (Liveness, String, Option<anyhow::Error>) {
        let l = live::liveness(&self.proc, &self.syms, self.base, self.account_soid);
        let text = l.report();
        let refusal = if l.level < require && !self.allow_dead {
            Some(anyhow::anyhow!(
                "refusing: this read needs L{require} and the client is at L{}. Pass --allow-dead \
                 to read anyway, and label the result accordingly.",
                l.level
            ))
        } else {
            None
        };
        (l, text, refusal)
    }
}

pub fn parse_u64(s: &str) -> Result<u64> {
    let t = s.trim();
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
        u64::from_str_radix(b, 2).ok()
    } else if let Some(oo) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
        u64::from_str_radix(oo, 8).ok()
    } else {
        t.parse::<u64>().ok()
    };
    v.ok_or_else(|| anyhow::anyhow!("not a number: {s:?}"))
}
