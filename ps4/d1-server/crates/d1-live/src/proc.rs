// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The live client as an ordinary Linux process: mappings, `/proc/<pid>/mem`, the **three base
//! controls**, and the **liveness ladder**.
//!
//! The discipline here is lifted wholesale from `capture-rig/d1guest.py` (which owns it) and
//! `historicaldocs/design/runtime-state-access.md` §2 - §3. Restated, because a snapshot inherits every one of these
//! failure modes and then *persists* them into a file that outlives the run:
//!
//! * ⚠ **A WRONG BASE YIELDS PLAUSIBLE-LOOKING BYTES RATHER THAN AN ERROR.** Two independent
//!   anchors must agree on exactly one base; a third, independent address must compare equal to the
//!   file **by direct read, not by scan**; and the same window shifted one byte must **differ**, so
//!   a degenerate all-zero window cannot make the third check pass for free.
//! * ⚠ **An anchor must not be a tracer probe site or a patch target.** The UD2 tracer writes
//!   `0f 0b` over its probes and `patches/*.xml` rewrite code bytes; either destroys a signature.
//!   The two anchors in `guest-symbols.json` are annotated as safe. We do not invent our own.
//! * ⚠ **A read after death is an error, not a zero.** Registry *objects* are cleared on connection
//!   teardown while *descriptors* survive (ATLAS §4.6b), so a late read is indistinguishable from
//!   "our delivery never worked". A snapshot taken from a dead or unverified process is **refused,
//!   not saved**.
//! * ⚠ **Do NOT filter mappings by size.** shadPS4 reserves tens of TiB of sparse address space and
//!   the guest heap lives *inside* those reservations; an earlier tool skipped anything over 1 GiB
//!   and searched 0.01 % of memory while reporting a confident "0 hits". Filter on **Rss** from
//!   `smaps` instead - resident pages are the ones that exist.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use d1_eboot::Image;

use crate::symbols::Symbols;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Death, as a distinguishable outcome
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⚠ **A read after death is an ERROR, not a zero.**
///
/// `d1guest.py` carries this as its own exception class and exits **3** on it, for a reason paid for
/// twice: registry *objects* are cleared on connection teardown while *descriptors* survive
/// (ATLAS §4.6b), so a late read shows zeros that are indistinguishable from *"our delivery never
/// worked"*. A caller that cannot tell "the client died" from "the field was zero" will publish the
/// second and mean the first. This type exists so the exit code can carry that distinction.
#[derive(Debug)]
pub struct Dead(pub String);

impl std::fmt::Display for Dead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Dead {}

/// Did this error chain come from the client dying, rather than from anything we did wrong?
pub fn is_dead(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.downcast_ref::<Dead>().is_some())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Process discovery
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⚠ `pgrep -x shadps4` matches **nothing** - the comm is `shadPS4:Main`. The `[s]` character class
/// keeps the pattern from matching the shell that evaluates it. Do not "simplify" either.
pub fn emulator_pids() -> Result<Vec<i32>> {
    let out = std::process::Command::new("pgrep")
        .args(["-f", "[s]hadps4 -p "])
        .output()
        .context("running pgrep")?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect())
}

/// Field 22 of `/proc/<pid>/stat` - makes an identity pid-recycle-proof.
pub fn starttime(pid: i32) -> Result<u64> {
    let data = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .with_context(|| format!("reading /proc/{pid}/stat"))?;
    let after = data
        .rfind(')')
        .map(|i| &data[i + 2..])
        .ok_or_else(|| anyhow!("malformed /proc/{pid}/stat"))?;
    after
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| anyhow!("/proc/{pid}/stat has no field 22"))?
        .parse()
        .context("parsing starttime")
}

#[derive(Debug, Clone)]
pub struct Mapping {
    pub start: u64,
    pub end: u64,
    pub perms: String,
    pub rss: u64,
    pub path: String,
}

impl Mapping {
    pub fn len(&self) -> u64 {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }
    pub fn readable(&self) -> bool {
        self.perms.starts_with('r')
    }
    pub fn executable(&self) -> bool {
        self.perms.as_bytes().get(2) == Some(&b'x')
    }
}

pub struct Proc {
    pub pid: i32,
    pub starttime: u64,
    mem: File,
    cached_maps: Vec<Mapping>,
}

impl Proc {
    pub fn open(pid: i32) -> Result<Self> {
        let starttime = starttime(pid)?;
        let mem = File::open(format!("/proc/{pid}/mem"))
            .with_context(|| format!("opening /proc/{pid}/mem (need ptrace permission)"))?;
        let mut p = Self { pid, starttime, mem, cached_maps: Vec::new() };
        p.cached_maps = p.read_mappings()?;
        Ok(p)
    }

    pub fn alive(&self) -> bool {
        starttime(self.pid).map(|t| t == self.starttime).unwrap_or(false)
    }

    /// A read that begins alive and finishes dead is an **error**, never a short buffer of zeros.
    pub fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        if !self.alive() {
            return Err(anyhow::Error::new(Dead(
                "the client is gone - this read is an ERROR, not a zero".into(),
            )));
        }
        let mut buf = vec![0u8; len];
        match self.mem.read_exact_at(&mut buf, addr) {
            Ok(()) => {}
            Err(e) => {
                if !self.alive() {
                    return Err(anyhow::Error::new(Dead(format!(
                        "the client died during a read at {addr:#x} - an ERROR, not a zero"
                    ))));
                }
                return Err(e).with_context(|| format!("reading {len} bytes at {addr:#x}"));
            }
        }
        Ok(buf)
    }

    /// Read into a caller-owned buffer. ⭐ **No allocation inside the caller's timed window.**
    ///
    /// Why this exists rather than [`Proc::read`]: `d1-replay capture --snapshot-on-hit` reads tens
    /// of MiB while a client thread is halted at a watchpoint, and a fresh `vec![0u8; 22 MiB]` per
    /// region faults and zeroes a new mapping *inside* that window (measured 2.0x slower,
    /// `historicaldocs/design/live-watch.md` §6a.5). The caller allocates once, before anything is armed, and this
    /// fills slices of it. It also keeps Rust's allocation-failure handler - which calls `abort()`,
    /// and `abort()` does not unwind, so no `Drop` runs - out of the stop entirely.
    ///
    /// ⚠ Unlike [`Proc::read`] this does **not** re-check liveness on every call: two `/proc` reads
    /// per 1 MiB chunk is exactly the cost the caller is trying to avoid. It checks on *failure*, so
    /// a read that dies mid-way still reports an ERROR rather than a short buffer - and the caller
    /// is expected to have checked `alive()` once before the window opens.
    pub fn read_into(&self, addr: u64, buf: &mut [u8]) -> Result<()> {
        match self.mem.read_exact_at(buf, addr) {
            Ok(()) => Ok(()),
            Err(e) => {
                if !self.alive() {
                    return Err(anyhow::Error::new(Dead(format!(
                        "the client died during a read at {addr:#x} - an ERROR, not a zero"
                    ))));
                }
                Err(e).with_context(|| format!("reading {} bytes at {addr:#x}", buf.len()))
            }
        }
    }

    /// Best-effort read: `None` when the range is not resident/readable. Used only where a miss is
    /// itself the answer (pointer plausibility), never where a zero could be mistaken for data.
    pub fn try_read(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.mem.read_exact_at(&mut buf, addr).ok().map(|()| buf)
    }

    pub fn mappings(&self) -> &[Mapping] {
        &self.cached_maps
    }

    pub fn mapping_of(&self, addr: u64) -> Option<&Mapping> {
        self.cached_maps.iter().find(|m| m.start <= addr && addr < m.end)
    }

    fn read_mappings(&self) -> Result<Vec<Mapping>> {
        let smaps = std::fs::read_to_string(format!("/proc/{}/smaps", self.pid))
            .with_context(|| format!("reading /proc/{}/smaps", self.pid))?;
        let mut out = Vec::new();
        let mut cur: Option<(u64, u64, String, String)> = None;
        for line in smaps.lines() {
            if let Some((range, rest)) = line.split_once(' ') {
                if let Some((lo, hi)) = range.split_once('-') {
                    if let (Ok(lo), Ok(hi)) = (u64::from_str_radix(lo, 16), u64::from_str_radix(hi, 16))
                    {
                        let mut it = rest.split_whitespace();
                        let perms = it.next().unwrap_or("----").to_string();
                        if perms.len() == 4 {
                            let path = rest.split_whitespace().nth(4).unwrap_or("").to_string();
                            cur = Some((lo, hi, perms, path));
                            continue;
                        }
                    }
                }
            }
            if let Some(rss_line) = line.strip_prefix("Rss:") {
                if let Some((lo, hi, perms, path)) = cur.take() {
                    let rss = rss_line
                        .split_whitespace()
                        .next()
                        .and_then(|k| k.parse::<u64>().ok())
                        .unwrap_or(0)
                        * 1024;
                    out.push(Mapping { start: lo, end: hi, perms, rss, path });
                }
            }
        }
        Ok(out)
    }

    /// Byte-signature scan over readable mappings **with resident pages**.
    ///
    /// `executable_only` is the cheap default (≈ 250 MiB here versus ≈ 17 GiB): a *code* signature
    /// is in an executable mapping by definition. It is the wrong filter for a heap value, and the
    /// caller must say which job it is doing.
    pub fn find(&self, needle: &[u8], executable_only: bool, limit: usize) -> (Vec<u64>, u64) {
        let mut hits = Vec::new();
        let mut scanned = 0u64;
        for m in self.cached_maps.iter() {
            if m.rss == 0 || !m.readable() || (executable_only && !m.executable()) {
                continue;
            }
            let Some(blob) = self.try_read(m.start, m.len() as usize) else { continue };
            scanned += blob.len() as u64;
            let mut i = 0usize;
            while i + needle.len() <= blob.len() {
                match blob[i..].windows(needle.len()).position(|w| w == needle) {
                    Some(j) => {
                        hits.push(m.start + (i + j) as u64);
                        if hits.len() >= limit {
                            return (hits, scanned);
                        }
                        i += j + 1;
                    }
                    None => break,
                }
            }
        }
        (hits, scanned)
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Base recovery - three controls, all printed, all mandatory
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ControlLine {
    pub name: String,
    pub pass: bool,
    pub detail: String,
    /// A bare follow-up line, rendered under this control without a PASS/FAIL column.
    ///
    /// ⚠ `serde(default)`: the archived snapshots in `runs/snapshots/` carry `BaseControls` in their
    /// manifests and predate this field. An absent field must read as "no note", never as a parse
    /// error - the same discipline `d1-replay`'s `Manifest::trap` documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BaseControls {
    pub base: Option<u64>,
    pub lines: Vec<ControlLine>,
}

impl BaseControls {
    fn check(&mut self, name: &str, pass: bool, detail: String) -> bool {
        self.lines.push(ControlLine { name: name.into(), pass, detail, note: None });
        pass
    }
    pub fn ok(&self) -> bool {
        !self.lines.is_empty() && self.lines.iter().all(|l| l.pass)
    }
    pub fn report(&self) -> String {
        let mut s = format!(
            "BASE CONTROL: {}{}",
            if self.ok() { "PASS" } else { "FAIL" },
            self.base.map(|b| format!("  base={b:#x}")).unwrap_or_default()
        );
        for l in &self.lines {
            s.push_str(&format!("\n  {}  {}: {}", if l.pass { "PASS" } else { "FAIL" }, l.name, l.detail));
            if let Some(n) = &l.note {
                s.push_str(&format!("\n  {n}"));
            }
        }
        s
    }
}

/// Where a recovered base is remembered between invocations.
///
/// ⚠ Deliberately **not** `d1guest.py`'s `/tmp/d1guest-base.json`. Two programs writing one file is
/// one format described twice, and the two would drift the moment either learned a new key. The
/// discipline is copied; the file is not shared.
pub const BASE_CACHE: &str = "/tmp/d1-live-base.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedBase {
    pid: i32,
    starttime: u64,
    base: u64,
    sha256: String,
}

/// [`recover_base`], but the **anchor scan** (the expensive control) may be served from a cache
/// keyed on pid + start time + eboot SHA-256.
///
/// ⭐ The two cheap controls - the direct verify compare and its shifted negative control - re-run
/// on **every** call regardless. A cache that skipped those would be a cache of a conclusion rather
/// than of a measurement, and a wrong base yields plausible-looking bytes rather than an error.
///
/// The start time (`/proc/<pid>/stat` field 22) is what makes the key pid-recycle-proof.
pub fn recover_base_cached(
    proc: &Proc,
    image: &Image,
    syms: &Symbols,
    full_scan: bool,
    use_cache: bool,
) -> BaseControls {
    let cached: Option<u64> = if use_cache {
        std::fs::read_to_string(BASE_CACHE)
            .ok()
            .and_then(|t| serde_json::from_str::<CachedBase>(&t).ok())
            .filter(|c| {
                c.pid == proc.pid && c.starttime == proc.starttime && c.sha256 == syms.eboot_sha256
            })
            .map(|c| c.base)
    } else {
        None
    };

    let Some(base) = cached else {
        let ctl = recover_base(proc, image, syms, full_scan);
        if let (true, Some(b)) = (ctl.ok(), ctl.base) {
            let _ = std::fs::write(
                BASE_CACHE,
                serde_json::to_string(&CachedBase {
                    pid: proc.pid,
                    starttime: proc.starttime,
                    base: b,
                    sha256: syms.eboot_sha256.clone(),
                })
                .unwrap_or_default(),
            );
        }
        return ctl;
    };

    let mut ctl = BaseControls { base: Some(base), lines: Vec::new() };
    ctl.check(
        "anchors",
        true,
        format!(
            "cached for pid {} (start {}); scan skipped - the two checks below still run every time",
            proc.pid, proc.starttime
        ),
    );
    verify_controls(proc, image, syms, base, &mut ctl);
    ctl
}

/// Recover the host address at which eboot vaddr `0` is mapped.
pub fn recover_base(proc: &Proc, image: &Image, syms: &Symbols, full_scan: bool) -> BaseControls {
    let mut ctl = BaseControls::default();

    // Control 1 - two INDEPENDENT anchors must agree on EXACTLY ONE base.
    let mut sets: Vec<std::collections::BTreeSet<u64>> = Vec::new();
    let mut details = Vec::new();
    for a in &syms.anchors {
        let Ok(sig) = image.bytes(a.vaddr, a.len) else {
            ctl.check("anchors", false, format!("anchor {} at {:#x} is not file-backed", a.name, a.vaddr));
            return ctl;
        };
        let (hits, scanned) = proc.find(sig, !full_scan, 16);
        // ⚠ `scanned/2**20:.0f` in the Python ROUNDS; integer division truncates, and the two
        // disagree by 1 MiB on the real image. A provenance line that differs between two readers
        // of the same process is a diff nobody can dismiss at a glance, so it is matched exactly.
        details.push(format!(
            "{}@{:#x} -> {} hit(s) in {:.0} MiB",
            a.name,
            a.vaddr,
            hits.len(),
            scanned as f64 / (1u64 << 20) as f64
        ));
        sets.push(hits.into_iter().map(|h| h - a.vaddr).collect());
    }
    let mut agreed: std::collections::BTreeSet<u64> = sets[0].clone();
    for s in &sets[1..] {
        agreed = agreed.intersection(s).copied().collect();
    }
    let agreed: Vec<u64> = agreed.into_iter().collect();
    let mut detail = details.join("; ");
    // `f"{[hex(b) for b in agreed]}"` - a Python list of hex STRINGS, quotes and all.
    detail.push_str(&format!(
        " => agreed [{}]",
        agreed.iter().map(|b| format!("'{b:#x}'")).collect::<Vec<_>>().join(", ")
    ));
    if agreed.len() != 1 {
        detail.push_str("  (need EXACTLY one)");
        ctl.check("anchors", false, detail);
        if !full_scan {
            // A separate line, as in the Python: it is advice, not a control result, and printing
            // it with a FAIL column would read as a second failed check.
            if let Some(l) = ctl.lines.last_mut() {
                l.note = Some(
                    "note: only EXECUTABLE resident mappings were scanned; retry with --full-scan"
                        .into(),
                );
            }
        }
        return ctl;
    }
    ctl.check("anchors", true, detail);
    let base = agreed[0];
    ctl.base = Some(base);
    verify_controls(proc, image, syms, base, &mut ctl);
    ctl
}

/// Controls 2 and 3, split out so the cached path can re-run them without re-scanning.
fn verify_controls(proc: &Proc, image: &Image, syms: &Symbols, base: u64, ctl: &mut BaseControls) {
    // Control 2 - a THIRD, independent address, by direct comparison rather than by scan. A scan
    // that found the right bytes says nothing about whether the arithmetic that turned them into a
    // base was right; this does.
    let v = &syms.verify;
    let Ok(want) = image.bytes(v.vaddr, v.len) else {
        ctl.check("verify", false, format!("verify window {:#x} is not file-backed", v.vaddr));
        return;
    };
    let got = match proc.read(base + v.vaddr, v.len) {
        Ok(g) => g,
        Err(e) => {
            ctl.check("verify", false, e.to_string());
            return;
        }
    };
    ctl.check(
        "verify",
        got == want,
        format!(
            "{} bytes at {:#x} {} the file image",
            v.len,
            v.vaddr,
            if got == want { "match" } else { "DIFFER from" }
        ),
    );

    // Control 3 - the NEGATIVE control. Control 2 alone can pass for the wrong reason: a degenerate
    // window (all zero, a run of nops) matches almost any base. Shift by one byte; it must NOT match.
    let shifted = match proc.read(base + v.vaddr + 1, v.len) {
        Ok(s) => s,
        Err(e) => {
            ctl.check("negative control", false, e.to_string());
            return;
        }
    };
    let discriminates = shifted != want;
    ctl.check(
        "negative control",
        discriminates,
        if discriminates {
            "the same window shifted by 1 byte differs - the window discriminates, so the verify above means something".into()
        } else {
            "the same window shifted by 1 byte ALSO MATCHES - the window is degenerate, so the verify above proved nothing".to_string()
        },
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The liveness ladder
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Liveness {
    pub level: u8,
    pub lines: Vec<String>,
}

impl Liveness {
    pub fn report(&self) -> String {
        let mut s = format!("LIVENESS: L{}", self.level);
        for l in &self.lines {
            s.push_str(&format!("\n{l}"));
        }
        s
    }
}

/// L0 process · L1 base · L2 the client has built the object registry · L3 the account object
/// carries the SOID we serve, so registry object *contents* mean something.
pub fn liveness(proc: &Proc, syms: &Symbols, base: u64, account_soid: u64) -> Liveness {
    let mut lines = Vec::new();
    if !proc.alive() {
        return Liveness { level: 0, lines: vec!["  L0 FAIL  the process is gone".into()] };
    }
    lines.push(format!("  L0 PASS  pid {} alive", proc.pid));
    lines.push(format!("  L1 PASS  base {base:#x} (see BASE CONTROL)"));

    let Some(reg_ptr_va) = syms.lookup("object_registry_ptr") else {
        lines.push("  L2 FAIL  guest-symbols.json has no object_registry_ptr".into());
        return Liveness { level: 1, lines };
    };
    let reg = match proc.read(base + reg_ptr_va, 8) {
        Ok(b) => u64::from_le_bytes(b.try_into().unwrap()),
        Err(e) => {
            lines.push(format!("  L2 FAIL  {e}"));
            return Liveness { level: 1, lines };
        }
    };
    if reg == 0 {
        lines.push("  L2 FAIL  object_registry_ptr is NULL - the client has not built the registry yet".into());
        return Liveness { level: 1, lines };
    }
    lines.push(format!("  L2 PASS  object registry {reg:#x}"));

    // ⭐ L3 is `family_slot`, not a raw entry-table read. It has to be: the cross-check between the
    // client's RUNTIME entry table and the STATIC family/slot mapper is what turns "there is a
    // pointer here" into "and both derivations agree it is the account". A ladder that skipped it
    // would report L3 PASS on a registry whose two derivations disagree - health, reported for the
    // exact condition the cross-check exists to catch. ⚠ On SUCCESS the cross-check lines are not
    // printed (they are the `obj` command's job); on FAILURE they are, because then they are the
    // explanation.
    let (fam, typ) = (syms.account_family, syms.account_type);
    let fr = match crate::registry::family_slot_at(proc, syms, reg, fam, typ) {
        Ok(f) => f,
        Err(e) => {
            lines.push(format!("  L3 FAIL  {e}"));
            return Liveness { level: 2, lines };
        }
    };
    let Some(elem) = fr.addr else {
        lines.extend(fr.lines);
        lines.push(format!("  L3 FAIL  cannot locate family {fam}/{typ}"));
        return Liveness { level: 2, lines };
    };
    let key = match proc.read(elem, 8) {
        Ok(b) => u64::from_le_bytes(b.try_into().unwrap()),
        Err(e) => {
            lines.push(format!("  L3 FAIL  {e}"));
            return Liveness { level: 2, lines };
        }
    };
    if key == account_soid {
        lines.push(format!(
            "  L3 PASS  account (fam{fam}/t{typ}) key={key:#018x} == expected - registry objects are LIVE"
        ));
        Liveness { level: 3, lines }
    } else {
        lines.push(format!(
            "  L3 FAIL  account (fam{fam}/t{typ}) key={key:#018x} != expected {account_soid:#018x}. \
             ⚠ Objects are cleared on connection teardown while descriptors survive; a read now is \
             indistinguishable from 'delivery never worked' (ATLAS §4.6b)."
        ));
        Liveness { level: 2, lines }
    }
}

/// The pinned eboot, or `$D1_EBOOT`, or an explicit path.
pub fn open_image(path: Option<PathBuf>) -> Result<Image> {
    match path {
        Some(p) => Image::open(p).map_err(|e| anyhow!("{e}")),
        None => Image::open_default().map_err(|e| anyhow!("{e}")),
    }
}
