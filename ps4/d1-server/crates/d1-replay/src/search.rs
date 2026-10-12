// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Candidate search - **the feature the replay oracle exists for.**
//!
//! ⚠ **CORRECTED 2026-08-10:** this used to open *"a replay cannot recover the register values the
//! client had"*. That is true of `/proc/<pid>/mem`, which has no registers in it, and therefore true
//! of every snapshot in `runs/snapshots/` - but it is **not** true of a snapshot taken by
//! `capture --snapshot-on-hit`, whose registers come from `PTRACE_GETREGS` at a hardware-watchpoint
//! stop. `--use-captured-regs` starts a search from those instead of from zeros, and the search's
//! own header says which it did.
//!
//! ⭐ **And it changes what a search MEANS, so read the header before quoting a hit.** With an
//! invented baseline every register is ours. With a captured one, only the **varied** knobs are
//! ours: a hit still names an input that works rather than the input the client used, but the rest
//! of the entry state is the client's, so a hit is a much narrower claim than it used to be.
//!
//! At the walls this project keeps hitting, the unknown is a **small dense integer** - a
//! slot id, a family/type key, a local-user index - searched for by a four-iteration linear scan
//! over a table we *can* capture. So the answer is reachable **by search rather than by deduction**:
//! run the real function over every plausible input, against the real memory, and report which
//! inputs produce the outcome you named.
//!
//! What the answer means, precisely:
//!
//! > *"Given the memory the client actually had at instant T, input **X** makes this function
//! > hit."*
//!
//! and **not** *"the client's input was X"*. Those are different claims and only the first is
//! supported. Where the candidate set is exhaustive over the type (all 2^32 is not, but "every
//! value present in the table" and "0..=255" often are) an *empty* result set is the stronger
//! finding: **nothing** would have made it hit, so the table, not the key, is what is wrong.
//!
//! Varying a value in **memory** is as important as varying a register: at the fireteam-session
//! wall the searched key was `[G+0xf8]`, not an argument.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};
use unicorn_engine::RegisterX86;

use crate::addr::Width;
use crate::machine::{reg_by_name, Machine, RegDump, RunSpec};

/// One knob to turn.
#[derive(Debug, Clone)]
pub enum Knob {
    Reg { name: String, reg: RegisterX86 },
    Mem { addr: u64, width: Width, label: String },
}

impl Knob {
    pub fn label(&self) -> String {
        match self {
            Knob::Reg { name, .. } => name.clone(),
            Knob::Mem { label, .. } => label.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidates {
    pub knob: Knob,
    pub values: Vec<u64>,
}

/// `1,2,3` · `0..15` · `0..0x100:4` · `0xffffffff`
pub fn parse_values(s: &str) -> Result<Vec<u64>> {
    let num = |t: &str| -> Result<u64> {
        let t = t.trim();
        if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u64::from_str_radix(h, 16).map_err(|e| anyhow!("{t:?}: {e}"))
        } else if let Some(n) = t.strip_prefix('-') {
            Ok((-(n.parse::<i64>().map_err(|e| anyhow!("{t:?}: {e}"))?)) as u64)
        } else {
            t.parse().map_err(|e| anyhow!("{t:?}: {e}"))
        }
    };
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (range, step) = match part.split_once(':') {
            Some((r, st)) => (r, num(st)?.max(1)),
            None => (part, 1),
        };
        match range.split_once("..") {
            Some((lo, hi)) => {
                let (lo, hi) = (num(lo)?, num(hi)?);
                if hi < lo {
                    bail!("empty range {range:?}");
                }
                let mut v = lo;
                loop {
                    out.push(v);
                    if v > hi.saturating_sub(step) {
                        break;
                    }
                    v += step;
                }
            }
            None => out.push(num(range)?),
        }
    }
    if out.is_empty() {
        bail!("no candidate values parsed from {s:?}");
    }
    Ok(out)
}

/// The outcome that counts as a hit.
#[derive(Debug, Clone)]
pub enum Predicate {
    RetEq(u64),
    RetNe(u64),
    NoFault,
    Fault,
    Reached(u64),
    NotReached(u64),
    /// A register's value at the first arrival at a probe address.
    AtRegEq { addr: u64, reg: String, value: u64 },
    AtRegNe { addr: u64, reg: String, value: u64 },
}

impl Predicate {
    /// `ret==0` · `ret!=0` · `nofault` · `fault` · `reached:0x…` · `!reached:0x…` ·
    /// `0x…:rdi!=0` (register at a probe)
    ///
    /// Addresses here are **already resolved absolutes** - the caller resolves them through
    /// `addr::addr` so the resolution is printed like every other one.
    pub fn parse(s: &str, resolve: &dyn Fn(&str) -> Result<u64>) -> Result<Self> {
        let s = s.trim();
        let num = |t: &str| -> Result<u64> {
            let t = t.trim();
            if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                u64::from_str_radix(h, 16).map_err(|e| anyhow!("{t:?}: {e}"))
            } else {
                t.parse().map_err(|e| anyhow!("{t:?}: {e}"))
            }
        };
        if s == "nofault" {
            return Ok(Predicate::NoFault);
        }
        if s == "fault" {
            return Ok(Predicate::Fault);
        }
        if let Some(r) = s.strip_prefix("ret==") {
            return Ok(Predicate::RetEq(num(r)?));
        }
        if let Some(r) = s.strip_prefix("ret!=") {
            return Ok(Predicate::RetNe(num(r)?));
        }
        if let Some(r) = s.strip_prefix("!reached:") {
            return Ok(Predicate::NotReached(resolve(r)?));
        }
        if let Some(r) = s.strip_prefix("reached:") {
            return Ok(Predicate::Reached(resolve(r)?));
        }
        // ADDR:REG!=V / ADDR:REG==V - split on the LAST ':' so an address expression may contain one.
        if let Some(i) = s.rfind(':') {
            let (a, rest) = (&s[..i], &s[i + 1..]);
            if let Some((reg, v)) = rest.split_once("!=") {
                return Ok(Predicate::AtRegNe {
                    addr: resolve(a)?,
                    reg: reg.trim().to_ascii_lowercase(),
                    value: num(v)?,
                });
            }
            if let Some((reg, v)) = rest.split_once("==") {
                return Ok(Predicate::AtRegEq {
                    addr: resolve(a)?,
                    reg: reg.trim().to_ascii_lowercase(),
                    value: num(v)?,
                });
            }
        }
        bail!(
            "cannot parse outcome {s:?}. Forms: ret==N | ret!=N | nofault | fault | reached:ADDR | \
             !reached:ADDR | ADDR:REG==N | ADDR:REG!=N"
        )
    }

    pub fn eval(&self, r: &crate::machine::RunReport) -> bool {
        match self {
            Predicate::RetEq(v) => r.rax == *v,
            Predicate::RetNe(v) => r.rax != *v,
            Predicate::NoFault => !r.exit.is_fault(),
            Predicate::Fault => r.exit.is_fault(),
            Predicate::Reached(a) => r.reached(*a),
            Predicate::NotReached(a) => !r.reached(*a),
            Predicate::AtRegEq { addr, reg, value } => {
                r.probes.get(addr).and_then(|d| d.get(reg)) == Some(*value)
            }
            Predicate::AtRegNe { addr, reg, value } => {
                matches!(r.probes.get(addr).and_then(|d| d.get(reg)), Some(v) if v != *value)
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Predicate::RetEq(v) => format!("returns {v:#x}"),
            Predicate::RetNe(v) => format!("returns != {v:#x}"),
            Predicate::NoFault => "does not fault".into(),
            Predicate::Fault => "faults".into(),
            Predicate::Reached(a) => format!("reaches {a:#x}"),
            Predicate::NotReached(a) => format!("does NOT reach {a:#x}"),
            Predicate::AtRegEq { addr, reg, value } => format!("{reg} == {value:#x} at {addr:#x}"),
            Predicate::AtRegNe { addr, reg, value } => format!("{reg} != {value:#x} at {addr:#x}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<u64>,
    pub rax: u64,
    /// ⚠ Read THIS for a bool-returning function; `rax`'s upper bytes are the callee's leftovers.
    pub al: u8,
    pub exit: String,
    pub steps: u64,
    pub hit: bool,
    pub probes: BTreeMap<u64, RegDump>,
}

pub struct SearchResult {
    pub rows: Vec<Row>,
    pub knobs: Vec<String>,
    pub hits: Vec<usize>,
    pub pristine: Vec<String>,
}

/// Cartesian product over every knob, one replay per combination.
pub fn search(
    m: &mut Machine,
    base_spec: &RunSpec,
    knobs: &[Candidates],
    hit: Option<&Predicate>,
    verify_every: bool,
) -> Result<SearchResult> {
    if knobs.is_empty() {
        bail!("candidate search with no --vary is just a replay; use `replay`");
    }
    let mut combos: Vec<Vec<u64>> = vec![Vec::new()];
    for k in knobs {
        let mut next = Vec::new();
        for c in &combos {
            for v in &k.values {
                let mut c2 = c.clone();
                c2.push(*v);
                next.push(c2);
            }
        }
        combos = next;
    }

    let mut rows = Vec::new();
    let mut hits = Vec::new();
    let mut pristine = Vec::new();
    for (n, combo) in combos.iter().enumerate() {
        let mut spec = base_spec.clone();
        spec.rollback = true;
        for (k, v) in knobs.iter().zip(combo) {
            match &k.knob {
                Knob::Reg { reg, .. } => spec.regs.push((*reg, *v)),
                Knob::Mem { addr, width, .. } => {
                    let bytes = v.to_le_bytes()[..width.bytes()].to_vec();
                    spec.writes.push((*addr, bytes));
                }
            }
        }
        let r = m.run(&spec)?;
        let is_hit = hit.map(|p| p.eval(&r)).unwrap_or(false);
        if is_hit {
            hits.push(rows.len());
        }
        rows.push(Row {
            values: combo.clone(),
            rax: r.rax,
            al: r.al,
            exit: r.exit.render(),
            steps: r.steps,
            hit: is_hit,
            probes: r.probes.clone(),
        });
        // The rollback control. Cheap enough to run on the first and last candidate always, and on
        // every candidate with --verify-every. A rollback that silently did nothing would make
        // candidate N+1 inherit candidate N's writes.
        if verify_every || n == 0 || n + 1 == combos.len() {
            let (ok, detail) = m.verify_pristine()?;
            pristine.push(format!("{}  candidate {n}: {detail}", if ok { "PASS" } else { "FAIL" }));
            if !ok {
                bail!(
                    "ROLLBACK CONTROL FAILED after candidate {n}: {detail}. Every later row would \
                     be contaminated, so the search is aborted rather than reported."
                );
            }
        }
    }

    Ok(SearchResult {
        rows,
        knobs: knobs.iter().map(|k| k.knob.label()).collect(),
        hits,
        pristine,
    })
}

pub fn reg_knob(name: &str) -> Result<Knob> {
    let reg = reg_by_name(name)
        .ok_or_else(|| anyhow!("unknown register {name:?} - try rdi/rsi/rdx/rcx/r8/r9/rax/…"))?;
    Ok(Knob::Reg { name: name.to_ascii_lowercase(), reg })
}

#[cfg(test)]
mod tests {
    use super::parse_values;

    #[test]
    fn candidate_value_lists_and_ranges() {
        assert_eq!(parse_values("1,2,3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_values("0..3").unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(parse_values("0..8:4").unwrap(), vec![0, 4, 8]);
        assert_eq!(parse_values("0xffffffff").unwrap(), vec![0xffff_ffff]);
    }
}
