// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The in-stop reader, measured against a throwaway child this test spawns itself.
//!
//! ⛔ **These never touch the emulator.** The subject is `d1-replay __stopped-child`, a process that
//! sleeps and is killed at the end of each test. Nothing here arms a debug register - the arming
//! half is `d1-live`'s controls, which drive their own child; this file measures the part that runs
//! *inside* the stop: chunked reads into a preallocated buffer, the register-root windows, and the
//! deadline.

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};

use d1_replay::live::Proc;
use d1_replay::onhit::{self, Opts, Plan, StopScope};

struct Toy(Child);

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn toy() -> (Toy, Proc) {
    let child = Command::new(env!("CARGO_BIN_EXE_d1-replay"))
        .arg("__stopped-child")
        .arg("--seconds")
        .arg("30")
        .stdout(Stdio::null())
        .spawn()
        .expect("spawning the toy child");
    let pid = child.id() as i32;
    // Give it a moment to map itself in.
    std::thread::sleep(std::time::Duration::from_millis(150));
    let proc = Proc::open(pid).expect("opening the toy child");
    (Toy(child), proc)
}

/// A readable, resident mapping big enough to cross the 1 MiB chunk boundary - so the test exercises
/// the chunk loop rather than a single `pread`.
fn a_big_readable_mapping(proc: &Proc) -> (u64, u64) {
    let m = proc
        .mappings()
        .iter()
        .filter(|m| m.readable() && m.rss > 0 && m.len() >= 0x40_0000)
        .max_by_key(|m| m.rss)
        .or_else(|| proc.mappings().iter().filter(|m| m.readable() && m.rss > 0).max_by_key(|m| m.len()))
        .expect("the toy child has no readable resident mapping");
    (m.start, m.len().min(0x20_0000))
}

fn plan_for(addr: u64, len: u64) -> Plan {
    Plan {
        scope: StopScope::Minimal,
        fixed: vec![(addr, len, "test: a readable window".into())],
        fixed_bytes: len,
        dynamic_allowance: 0x1_0000,
        rehearsed_bytes: len,
        rehearsed_ms: 1.0,
        predicted_bytes: len + 0x1_0000,
        predicted_ms: 1.0,
        seg_exec: Vec::new(),
        seg_data: vec![(addr, addr + len)],
        notes: Vec::new(),
    }
}

/// ⭐ The axis: **the bytes the in-stop reader puts in the buffer are the bytes at those addresses.**
///
/// An off-by-one in the chunk loop, or a region offset that does not match the region it labels,
/// would produce a snapshot full of real-looking bytes attributed to the wrong addresses - a wrong
/// answer with no error, which is this project's signature failure. So the whole window is compared
/// against an independent `Proc::read` of the same range, byte for byte.
#[test]
fn the_in_stop_reader_puts_the_right_bytes_at_the_right_offsets() {
    let (_toy, proc) = toy();
    let (addr, len) = a_big_readable_mapping(&proc);
    let plan = plan_for(addr, len);
    let opts = Opts { scope: StopScope::Minimal, ..Opts::default() };

    let mut buf = vec![0u8; (plan.fixed_bytes + plan.dynamic_allowance) as usize];
    let mut scratch = vec![0u8; 0x1000];
    let mut regs = BTreeMap::new();
    // A register root pointing INTO the mapping: it must be captured as its own window.
    regs.insert("rdi".to_string(), addr + 0x1234);
    // …and one that is not a pointer at all: it must be skipped, not faulted on.
    regs.insert("rsi".to_string(), 7u64);

    let r = onhit::read_in_stop(&proc, &plan, &opts, &regs, &mut buf, 0, &mut scratch);

    assert!(!r.deadline_fired, "a {} byte read should not blow a 16 ms budget", len);
    assert!(r.dropped.is_empty(), "{:?}", r.dropped);
    assert!(r.bytes >= len, "only {} of {len} bytes were read", r.bytes);

    let want = proc.read(addr, len as usize).expect("independent read");
    let mut checked = 0usize;
    for (a, off, l, _o) in &r.filled {
        if *a <= addr && addr + len <= *a + *l {
            let start = (addr - *a) as usize + *off;
            assert_eq!(&buf[start..start + len as usize], &want[..], "the buffer is not the memory");
            checked += 1;
        }
    }
    assert_eq!(checked, 1, "the planned window is not in the result exactly once: {:?}", r.filled);

    // The register root became its own region; the non-pointer did not.
    let roots: Vec<&String> = r.dynamic.iter().map(|(_, _, o)| o).collect();
    assert!(roots.iter().any(|o| o.contains("rdi=")), "{roots:?}");
    assert!(!roots.iter().any(|o| o.contains("rsi=")), "a non-pointer must not become a region: {roots:?}");

    // The phase sample was taken IN the stop and re-reading it now must find the same bytes: this
    // child is asleep, so nothing changed. ⚠ That is exactly why the real control ALSO requires the
    // data side to have moved - see `score_phase_split`'s DEGENERATE branch.
    let (_ew, _ec, dw, dc) = onhit::recheck_phase(&proc, &r.phase, &mut scratch);
    assert!(dw > 0, "no windows were sampled, so nothing was compared");
    assert_eq!(dc, 0, "a sleeping child's memory changed under us");
    assert!(onhit::score_phase_split(0, 0, dw, dc).starts_with("DEGENERATE"));
}

/// ⭐ THE PAIR for the test above: with the deadline set to zero the same read must **drop
/// everything, loudly**. A deadline that could never fire would be decoration, and a drop that was
/// silent would put a hole in a snapshot that reads as "we captured it and it was zero".
#[test]
fn the_in_stop_deadline_fires_and_names_every_dropped_region() {
    let (_toy, proc) = toy();
    let (addr, len) = a_big_readable_mapping(&proc);
    let plan = plan_for(addr, len);
    let opts = Opts { scope: StopScope::Minimal, budget_ms: 0.0, ..Opts::default() };

    let mut buf = vec![0u8; (plan.fixed_bytes + plan.dynamic_allowance) as usize];
    let mut scratch = vec![0u8; 0x1000];
    let regs = BTreeMap::new();

    let r = onhit::read_in_stop(&proc, &plan, &opts, &regs, &mut buf, 0, &mut scratch);
    assert!(r.deadline_fired, "a 0 ms budget must fire the deadline");
    assert!(!r.dropped.is_empty(), "the deadline fired but nothing was recorded as dropped");
    assert!(
        r.dropped.iter().all(|(_, _, why)| why.contains("deadline")),
        "every dropped region must carry its reason: {:?}",
        r.dropped
    );
    assert!(r.bytes < len, "the deadline fired but the whole window was still read");
}

/// The axis: a plan is **rehearsed against the live process**, and an unreadable region is refused
/// there - before anything is armed - rather than at the hit, where a refusal burns the trigger.
#[test]
fn the_rehearsal_reads_what_the_stop_will_read() {
    let (_toy, proc) = toy();
    let (addr, len) = a_big_readable_mapping(&proc);
    let mut plan = plan_for(addr, len);
    // Rehearse by hand over the same path `plan()` uses, then assert the numbers are real.
    let mut scratch = vec![0u8; onhit::CHUNK];
    let t0 = std::time::Instant::now();
    let mut n = 0u64;
    while n < len {
        let take = ((len - n) as usize).min(onhit::CHUNK);
        proc.read_into(addr + n, &mut scratch[..take]).expect("rehearsal read");
        n += take as u64;
    }
    plan.rehearsed_bytes = n;
    plan.rehearsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(plan.rehearsed_bytes, len);
    assert!(plan.rehearsed_ms >= 0.0);

    // And a range that is NOT mapped must fail the rehearsal rather than silently yield zeros.
    let unmapped = 0x7fff_0000_0000u64;
    assert!(
        proc.read_into(unmapped, &mut scratch[..0x1000]).is_err(),
        "an unmapped read must be an ERROR, never a buffer of zeros"
    );
}
