// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **A guest address space we own**, so the read side can be measured with no emulator running.
//!
//! # Why this exists
//!
//! `d1guest.py`'s read side has never had a control that could fail. Its `selftest` reproduces known
//! results *against a live client* - so it cannot run when there is no client, which is most of the
//! time, and it cannot be inverted, because you cannot ask a real client to be wrong on purpose.
//! Every claim about the address arithmetic therefore rested on the run that happened to be up.
//!
//! This module builds a **real Linux process** whose memory is laid out the way the client's is:
//!
//! * the **real eboot**'s two `PT_LOAD` segments, mapped at `BASE + p_vaddr`, executable where the
//!   image says executable - so anchor recovery, the verify compare and its shifted negative
//!   control all run against bytes nobody chose to make convenient;
//! * an **object registry** whose entry table is generated *from* `guest-symbols.json`'s static
//!   family/slot rows, so the runtime-versus-static cross-check has something true to agree with;
//! * a **package registry** with a real back-offset entry, so the signed-subtraction path is
//!   exercised rather than asserted;
//! * a **descriptor table** with claimed rows in states that accept and states that refuse.
//!
//! ⚠ **What this is not.** The registry *contents* are ours, so nothing read out of this fixture is
//! evidence about Destiny. Its reach tag is `synthetic-minimal` and it may never be cited as a
//! finding. What it measures is the **instrument**: address arithmetic, refusals, rendering, and
//! whether two implementations of the same reader agree byte for byte.
//!
//! # The perturbations are the point
//!
//! [`Perturb`] makes the fixture wrong on purpose, one axis at a time. A control that can only pass
//! is decoration; each perturbation below is a control that fires.

use std::io::Write;

use anyhow::{anyhow, bail, Context, Result};
use crate::symbols::Symbols;

/// Where the fixture maps eboot vaddr 0. Chosen to look like shadPS4's own guest window so a dump
/// taken here is visibly in the same address space shape as a real one.
pub const FIXTURE_BASE: u64 = 0x8_0000_0000;
/// The second, decoy base used by [`Perturb::DuplicateAnchor`].
pub const DECOY_BASE: u64 = 0x9_0000_0000;

const REGISTRY_LEN: usize = 0x6_0000;
const ARENA_LEN: usize = 0x10_0000;
const ARENA_OBJECT: u64 = 0x1000;
const ARENA_SLOTS: u64 = 0x1_0000;
const ARENA_ENTRIES: u64 = 0x6_0000;
const PACKAGE_ENTRY_SIZE: i32 = 0x10;

/// The tag the fixture publishes through its package registry.
pub const FIXTURE_TAG: u32 = 0x8080_1825;
/// The account SOID written into the family-4/type-0 element, so the liveness ladder reaches L3.
pub const FIXTURE_SOID: u64 = 0xD100_0000_0000_0001;

/// One axis of wrongness, injected deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perturb {
    /// Nothing. The positive leg.
    None,
    /// Write a stride into the family-4/type-0 runtime entry that disagrees with the static table.
    /// ⇒ the two-derivation cross-check must FAIL and no element address may be returned.
    FamilyStride,
    /// Flip one byte inside the verify window, in memory only. ⇒ base control 2 must FAIL, and
    /// nothing downstream may be read.
    VerifyWindow,
    /// Place a second copy of both anchors, at their true relative distance, at a different base.
    /// ⇒ the anchors now agree on TWO bases and control 1 must refuse ("need EXACTLY one").
    DuplicateAnchor,
}

impl Perturb {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "none" => Perturb::None,
            "family-stride" => Perturb::FamilyStride,
            "verify-window" => Perturb::VerifyWindow,
            "duplicate-anchor" => Perturb::DuplicateAnchor,
            other => bail!(
                "unknown perturbation {other:?} - use none | family-stride | verify-window | \
                 duplicate-anchor"
            ),
        })
    }
}

/// What the fixture prints on its first stdout line, so a parent never has to guess an address.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Banner {
    pub pid: i32,
    pub base: u64,
    pub registry: u64,
    pub package_header: u64,
    pub package_object: u64,
    pub tag: u32,
    pub account_soid: u64,
    /// An address inside no mapping at all - the target of the loud-failure control.
    pub unmapped: u64,
    /// `(vaddr, len)` of a file-backed window the fixture deliberately rewrote in memory, so the
    /// `vs FILE image: DIFFERS` path has something to report.
    pub relocated: (u64, usize),
    pub perturb: String,
}

fn page_down(a: u64) -> u64 {
    a & !0xFFFu64
}
fn page_up(a: u64) -> u64 {
    (a + 0xFFF) & !0xFFFu64
}

unsafe fn map_fixed(addr: u64, len: usize) -> Result<*mut u8> {
    let p = libc::mmap(
        addr as *mut libc::c_void,
        len,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE,
        -1,
        0,
    );
    if p == libc::MAP_FAILED || p as u64 != addr {
        bail!(
            "could not map {len:#x} bytes at {addr:#x} ({}). The fixture refuses to relocate: an \
             address it did not get is an address the controls would then be measuring in the wrong \
             space.",
            std::io::Error::last_os_error()
        );
    }
    Ok(p as *mut u8)
}

unsafe fn map_anon(len: usize) -> Result<*mut u8> {
    let p = libc::mmap(
        std::ptr::null_mut(),
        len,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        -1,
        0,
    );
    if p == libc::MAP_FAILED {
        bail!("mmap {len:#x}: {}", std::io::Error::last_os_error());
    }
    Ok(p as *mut u8)
}

/// `pgrep -f "[s]hadps4 -p "` matches, **minus this process and the shells that launched it**.
///
/// ⚠ Measured the hard way on first run: the fixture takes its decoy argv precisely so that pattern
/// matches it, so a naive `emulator_pids()` here reports the fixture itself plus the subshell that
/// forked it, and the safety refusal fires against its own reflection. That is CLAUDE.md's
/// `pkill -f` trap wearing a different hat - **a pattern that matches the shell that issued it**.
///
/// The exclusion is by identity, not by weakening the pattern: our own pid, every ancestor up the
/// `PPid` chain, and any process whose `/proc/<pid>/exe` is this same binary. A real shadPS4 is none
/// of those, so the guard keeps all of its teeth.
fn foreign_emulator_pids() -> Vec<i32> {
    let me = std::process::id() as i32;
    let mut mine = std::collections::BTreeSet::new();
    let mut p = me;
    for _ in 0..64 {
        mine.insert(p);
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{p}/status")) else { break };
        let Some(ppid) = stat.lines().find_map(|l| l.strip_prefix("PPid:")) else { break };
        let Ok(ppid) = ppid.trim().parse::<i32>() else { break };
        if ppid <= 1 || mine.contains(&ppid) {
            break;
        }
        p = ppid;
    }
    let self_exe = std::fs::read_link(format!("/proc/{me}/exe")).ok();
    crate::proc::emulator_pids()
        .unwrap_or_default()
        .into_iter()
        .filter(|pid| !mine.contains(pid))
        .filter(|pid| {
            let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
            !(exe.is_some() && exe == self_exe)
        })
        .collect()
}

/// Build the address space, print the banner, and hold for `hold_secs`.
///
/// ⛔ **Refuses to start while a real shadPS4 is running.** Its whole job is to be discoverable by
/// instruments that look for the client, and two discoverable targets is exactly how a measurement
/// gets attributed to the wrong process. That refusal is also what keeps this out of a live run.
pub fn mem_target_main(hold_secs: f64, perturb: Perturb, eboot: Option<std::path::PathBuf>) -> Result<i32> {
    let running = foreign_emulator_pids();
    if !running.is_empty() {
        bail!(
            "REFUSING: shadPS4 is running (pid(s) {running:?}). This fixture exists to be found by \
             `pgrep -f \"[s]hadps4 -p \"`, and a second match would make every instrument refuse to \
             guess - or worse, measure the wrong process. Close the emulator, or do not run the \
             offline controls during a live run."
        );
    }

    let image = crate::proc::open_image(eboot)?;
    let syms = Symbols::open_default()?;
    let (ok, line) = syms.check_build(&image.sha256());
    if !ok {
        bail!("{line}\nthe fixture must be built from the eboot the symbol file was measured against");
    }

    // ── The image, at BASE + p_vaddr, with the image's own protections ──────────────────────────
    let mut exec_spans: Vec<(u64, usize)> = Vec::new();
    for seg in image.load_segments() {
        let lo = page_down(FIXTURE_BASE + seg.vaddr);
        let hi = page_up(FIXTURE_BASE + seg.vaddr + seg.memsz);
        let len = (hi - lo) as usize;
        unsafe { map_fixed(lo, len)? };
        let bytes = image
            .bytes(seg.vaddr, seg.filesz as usize)
            .map_err(|e| anyhow!("reading segment {} out of the image: {e}", seg.index))?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (FIXTURE_BASE + seg.vaddr) as *mut u8,
                bytes.len(),
            );
        }
        if seg.is_executable() {
            exec_spans.push((lo, len));
        }
    }

    // ── A file-backed window rewritten in memory, so `vs FILE image: DIFFERS` has a subject ──────
    //
    // This is not decoration either: PS4 eboots keep pointer tables ZERO in the file and fill them
    // at load (ATLAS §4.2), and `boot_state_names` is the project's standing example. A reader that
    // never diffs against the file cannot see that, and has twice concluded "the table is empty".
    let reloc_va = syms.lookup("boot_state_names").unwrap_or(0x192D2A0);
    let reloc_len = syms.sym("boot_state_names").and_then(|s| s.len).unwrap_or(320);
    for i in 0..(reloc_len / 8) {
        let v = FIXTURE_BASE + 0x1600000 + (i as u64) * 0x40;
        unsafe {
            std::ptr::copy_nonoverlapping(
                v.to_le_bytes().as_ptr(),
                (FIXTURE_BASE + reloc_va + (i as u64) * 8) as *mut u8,
                8,
            );
        }
    }

    // ── The OBJECT registry ─────────────────────────────────────────────────────────────────────
    let reg = unsafe { map_anon(REGISTRY_LEN)? } as u64;
    let entry_base = syms.layout.get("entry_table_base").copied().unwrap_or(0x4F800);
    let sf = syms.layout.get("entry_stride_family").copied().unwrap_or(0x60);
    let st = syms.layout.get("entry_stride_type").copied().unwrap_or(0x10);
    let eb = syms.layout.get("element_base").copied().unwrap_or(0x40A0);

    let put = |addr: u64, bytes: &[u8]| unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
    };

    for f in &syms.families {
        let ent = reg + f.family * sf + f.typ * st + entry_base;
        if ent + 12 > reg + REGISTRY_LEN as u64 {
            bail!("the entry table does not fit the fixture's registry region - widen REGISTRY_LEN");
        }
        let off = (f.storage - eb) as u32;
        let mut stride = f.size as u32;
        if perturb == Perturb::FamilyStride && f.family == syms.account_family && f.typ == syms.account_type {
            // The injected disagreement: one bit of stride, which is exactly the kind of drift the
            // two-derivation cross-check exists to catch.
            stride ^= 0x10;
        }
        put(ent, &off.to_le_bytes());
        put(ent + 4, &(f.cap as u16).to_le_bytes());
        put(ent + 8, &stride.to_le_bytes());
        // A recognisable body, so a wrong element address prints visibly wrong bytes rather than
        // zeros that could be mistaken for "the client has not filled it in".
        for i in 0..f.cap {
            let e = reg + f.storage + i * f.size;
            put(e, &(0xF0_0000_0000_0000u64 | (f.family << 16) | (f.typ << 8) | i).to_le_bytes());
        }
    }
    // L3: the account element's key qword is the SOID the liveness predicate expects.
    let acct = syms
        .family(syms.account_family, syms.account_type)
        .ok_or_else(|| anyhow!("guest-symbols.json has no account family row"))?;
    put(reg + acct.storage, &FIXTURE_SOID.to_le_bytes());
    // A second identity in the character family, so `--key` has something to find that is NOT at
    // index 0 - an index and a key are different questions and the fixture must be able to tell.
    if let Some(chr) = syms.family(3, 1) {
        put(reg + chr.storage + 2 * chr.size, &0xD200_0000_0000_002Au64.to_le_bytes());
    }

    // ── The QueueZ descriptor table ─────────────────────────────────────────────────────────────
    let dtab = syms.layout.get("descriptor_table").copied().unwrap_or(0x60);
    let dstride = syms.layout.get("descriptor_stride").copied().unwrap_or(0x40);
    // One row in a state that ACCEPTS updates and one in a state that REFUSES them, because that
    // difference is the whole reason anyone reads this table.
    for (idx, class, key, state, err, ver) in [
        (5u64, 3u8, 0x8080_18B7_0000_0001u64, 3u8, 0u8, 7i32),
        (9, 1, 0x8080_1825_0000_0002, 1, 7, 0),
    ] {
        let d = reg + dtab + idx * dstride;
        put(d, &[class]);
        put(d + 0x20, &key.to_le_bytes());
        put(d + 0x28, &[state, err]);
        put(d + 0x3C, &ver.to_le_bytes());
    }

    // ── The PACKAGE registry ────────────────────────────────────────────────────────────────────
    let arena = unsafe { map_anon(ARENA_LEN)? } as u64;
    let header = arena;
    let slots = arena + ARENA_SLOTS;
    let entries = arena + ARENA_ENTRIES;
    let object = arena + ARENA_OBJECT;
    put(header, &slots.to_le_bytes());
    let stride_off = syms.package_layout.get("header_slot_stride").copied().unwrap_or(16);
    put(header + stride_off, &0x40u64.to_le_bytes());

    let (slot, index) = crate::registry::Guest::tag_slot(FIXTURE_TAG);
    let rec = slots + slot * 0x40;
    if rec + 0x40 > arena + ARENA_LEN as u64 {
        bail!("slot {slot:#x} falls outside the fixture arena - widen ARENA_LEN");
    }
    let dbo = syms.package_layout.get("slot_data_base").copied().unwrap_or(16);
    let eso = syms.package_layout.get("slot_entry_size").copied().unwrap_or(48);
    let mko = syms.package_layout.get("slot_mask").copied().unwrap_or(52);
    put(rec + dbo, &entries.to_le_bytes());
    put(rec + eso, &PACKAGE_ENTRY_SIZE.to_le_bytes());
    put(rec + mko, &(-1i32).to_le_bytes()); // mask = -1: the whole `loc` is the back-offset
    let ent = entries + index * PACKAGE_ENTRY_SIZE as u64;
    if ent + 0x10 > arena + ARENA_LEN as u64 {
        bail!("entry {index} falls outside the fixture arena - widen ARENA_LEN");
    }
    put(ent, &FIXTURE_TAG.to_le_bytes());
    put(ent + 8, &(ent - object).to_le_bytes()); // the SIGNED back-offset the client subtracts
    put(object, b"D1-LIVE FIXTURE PACKAGE OBJECT  ");

    // ── The two BSS roots ───────────────────────────────────────────────────────────────────────
    let reg_ptr_va = syms
        .lookup("object_registry_ptr")
        .ok_or_else(|| anyhow!("guest-symbols.json has no object_registry_ptr"))?;
    let pkg_ptr_va = syms
        .lookup("package_registry_global")
        .ok_or_else(|| anyhow!("guest-symbols.json has no package_registry_global"))?;
    put(FIXTURE_BASE + reg_ptr_va, &reg.to_le_bytes());
    put(FIXTURE_BASE + pkg_ptr_va, &header.to_le_bytes());

    // ── Perturbations that live in the image window ─────────────────────────────────────────────
    if perturb == Perturb::VerifyWindow {
        let a = FIXTURE_BASE + syms.verify.vaddr + 4;
        unsafe { *(a as *mut u8) ^= 0xFF };
    }
    if perturb == Perturb::DuplicateAnchor {
        let lo = syms.anchors.iter().map(|a| a.vaddr).min().unwrap();
        let hi = syms.anchors.iter().map(|a| a.vaddr + a.len as u64).max().unwrap();
        let span = page_up(hi - lo) as usize + 0x1000;
        let at = page_down(DECOY_BASE + lo);
        let p = unsafe { map_fixed(at, span)? };
        for a in &syms.anchors {
            let sig = image.bytes(a.vaddr, a.len).map_err(|e| anyhow!("{e}"))?;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    sig.as_ptr(),
                    p.add((DECOY_BASE + a.vaddr - at) as usize),
                    sig.len(),
                );
            }
        }
        unsafe {
            if libc::mprotect(at as *mut libc::c_void, span, libc::PROT_READ | libc::PROT_EXEC) != 0 {
                bail!("mprotect on the decoy: {}", std::io::Error::last_os_error());
            }
        }
    }

    // Executable last: the copies and the perturbations above all need write access first.
    for (lo, len) in exec_spans {
        unsafe {
            if libc::mprotect(lo as *mut libc::c_void, len, libc::PROT_READ | libc::PROT_EXEC) != 0 {
                bail!("mprotect {lo:#x}: {}", std::io::Error::last_os_error());
            }
        }
    }

    let banner = Banner {
        pid: std::process::id() as i32,
        base: FIXTURE_BASE,
        registry: reg,
        package_header: header,
        package_object: object,
        tag: FIXTURE_TAG,
        account_soid: FIXTURE_SOID,
        // Deliberately far above anything mapped: the target of the "an unmapped read must FAIL
        // LOUDLY, not return zeros" control.
        unmapped: 0x7F00_0000_0000,
        relocated: (reloc_va, reloc_len),
        perturb: format!("{perturb:?}"),
    };
    println!("{}", serde_json::to_string(&banner).context("serialising the fixture banner")?);
    std::io::stdout().flush().ok();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(hold_secs);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(0)
}
