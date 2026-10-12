// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-replay` - the **live-state replay oracle**.
//!
//! # The gap this closes
//!
//! The project has three instruments and none of them answers *"what value was in this register
//! when this code ran"*:
//!
//! | instrument | sees | blind to |
//! |---|---|---|
//! | `d1-eboot` (static) | code | every runtime value |
//! | `d1-oracle` (Unicorn, offline) | code executing | anything not derivable from args we supply |
//! | `capture-rig/d1guest.py` (live) | values in **memory** | registers; any chosen instant |
//!
//! This crate captures the live client's **memory** into a portable snapshot, rehydrates it into
//! Unicorn **at the client's own addresses**, and then executes chosen functions on that real state
//! with full observation: register dumps, bounded single-step tracing, read/write watchpoints, and
//! a **candidate search** that runs a function repeatedly over a set of inputs and reports which
//! ones produce a chosen outcome.
//!
//! # ⚠ WHAT THIS INSTRUMENT CANNOT DO - read this before quoting any result
//!
//! 1. **The entry registers are INVENTED unless the snapshot carries a trap and you ask for them.**
//!
//!    ⛔ **This limitation is now per-snapshot, and no blanket sentence about it is true.** Do not
//!    quote this paragraph at a result; quote the result's own `REGISTERS` block, which every run
//!    prints in every mode ([`machine::RegisterOrigin`]) and every `--json` carries.
//!
//!    * **The default, and every snapshot in `runs/snapshots/`.** `/proc/<pid>/mem` exposes memory,
//!      not the CPU state of the client's threads, so a replay **is not a reproduction of "the
//!      moment"** - it is *your* call, with *your* arguments, against *real* memory. Anything
//!      phrased as "this is what the client did" is a claim this tool cannot support. What it does
//!      support is "given the memory the client actually had, this function behaves like *this*, and
//!      *these* inputs make it hit." The key space at the current wall is tiny and dense, so
//!      **search replaces deduction** - and that is the whole design.
//!      ⚠ `trap: None` means **NOT ATTEMPTED**, never "the client had no registers".
//!    * ⭐ **A trap-paired snapshot, replayed with `--use-captured-regs`.** `d1-live` stops a thread
//!      at a hardware watchpoint and `PTRACE_GETREGS` hands back its whole integer register file;
//!      `capture --snapshot-on-hit` writes it into [`snapshot::Manifest::trap`], and
//!      `replay --use-captured-regs` starts from it. The honest sentence becomes *"this function,
//!      with the integer arguments the client actually held, against memory read over the following
//!      milliseconds while ~110 other threads ran"* - see [`snapshot::Coherence`], which exists to
//!      stop that being rounded up to "the state at the moment".
//!
//!    ⛔ **Four things a captured baseline still does NOT give you**, all of them printed in the
//!    block: `rip` (the entry is `--func`), `rsp`/`rbp` (this run's own frame, so the call can stop
//!    by returning), any register an `--arg`/`--reg`/`--vary` overrides, and **all** FP/SSE state -
//!    there is no `xmm` in a capture, so a float argument is invented in both modes.
//!
//!    ⚠ **Status, 2026-08-10.** Schema, capture and consumption are all built and controlled
//!    (`captured-vs-invented-registers`, and the pair in `d1-live`). **No trap-paired capture has
//!    ever been taken from shadPS4**, so every artifact on disk today is the first bullet.
//!
//!    ⭐ **And the caveat now travels with the result.** [`snapshot::TrapProvenance`] is rendered by
//!    `capture`, `info`, **`replay` and `search`**, in both the human and the `--json` renderings
//!    ([`machine::RunReport::trap`]). Until build 3 `replay` - the most-used subcommand, and the one
//!    whose transcripts get pasted into `design/` docs - named the trap **nowhere**, so a result
//!    taken from a trap-paired snapshot was silent about the coherence smear while
//!    `--use-captured-regs` invited the strongest possible reading of it.
//! 2. **A snapshot is one instant.** Memory captured at time *T* says nothing about time *T±ε*.
//!    The manifest records the liveness level and client state at *T* precisely so a later reader
//!    cannot forget which instant they are reasoning about.
//! 3. **It replays a fragment, not the program.** No threads, no scheduler, no syscalls, no GPU, no
//!    allocator. A function that reaches any of those faults - which is reported as a *result*, not
//!    hidden. Reachability of a code path under replay is not evidence the client reaches it.
//! 4. **Region selection is lossy by construction.** A snapshot holds what you asked for. Reads
//!    outside it fail loudly (they are never zero-filled), and every skipped mapping is recorded
//!    with its reason - *what you did not capture is part of the evidence*.
//! 5. **Everything downstream of a replay inherits the snapshot's reach tag.** A snapshot taken
//!    from a client whose state was reached over our own synthetic server is
//!    `synthetic_ancestor`, and no amount of faithful re-execution changes that.
//! 6. ⭐ **By default it ALTERS one pointer: the stack canary.** At least 16,117 of the binary's
//!    35,834 functions (**≥45%** - a floor; `d1-eboot`'s riprel scanner has a known open defect
//!    that can only under-count) dereference the guard pointer at `v:0x1A06C78` in their prologue,
//!    and the client's own value there points into **host** thread-local storage that Unicorn
//!    cannot map - so without intervention nearly every replay dies at step 8 having executed
//!    nothing, in a shape indistinguishable from "the arguments were wrong". `--stack-canary auto`
//!    (the default) repoints it at a mapped guest slot. That is *our* value, not the client's: it
//!    is printed on every run, recorded in every `--json` ([`machine::CanaryReport`]), rolled back
//!    like any other write, and it buys exactly one thing - **execution past the canary**, never
//!    "the function works". `--stack-canary off` restores the unaltered behaviour.
//!
//! # Why it does not reuse `d1-oracle`'s harness
//!
//! `d1-oracle` maps the eboot into a **file-address space** - vaddr `0` … `0x300_0000` - because it
//! executes pure codecs on synthetic inputs. A live snapshot is in the **client's own address
//! space**: the eboot sits at `base` (measured, ≈ `0x8_0000_0000`) and every captured pointer,
//! vtable slot and heap object is expressed in that space. Rebasing a captured heap graph down into
//! the oracle's window would mean rewriting pointers, which is exactly the class of silent
//! corruption this project keeps paying for. So this crate maps the snapshot **at its true
//! addresses** and shares no mapping code with `d1-oracle` - deliberately, and that boundary is
//! recorded in `historicaldocs/design/replay-oracle.md`.
//!
//! # Modules
//!
//! * [`addr`] - (re-exported from `d1-live`) the address/value expression language (`[v:0x2815860]+0xf8`), which always prints
//!   how it resolved. A resolution that is not shown is a resolution nobody checked.
//! * [`symbols`] - reads `capture-rig/guest-symbols.json`, the project's one address book.
//! * [`live`] - (re-exported from `d1-live::proc`) `/proc/<pid>/mem`, mappings, the three base
//!   controls, the liveness ladder.  The **native hardware watchpoint** lives in `d1-live` too:
//!   this crate observes a captured *state*, `d1-live watch` observes the *event*.
//! * [`snapshot`] - the portable snapshot format, capture and load.
//! * [`machine`] - snapshot → Unicorn, and the observation apparatus.
//! * [`preset`] - ⭐ named capture presets. `--preset content` derives the whole package/content
//!   registry working set from the client's own tables in ONE capture, instead of N rounds of
//!   "replay, fault on an uncaptured page, add a `--region`, re-capture".
//! * [`search`] - candidate search.
//! * [`controls`] - the mandatory positive controls (ATLAS §4.4d).
//! * [`rederive`] - ⭐ the activity-lifetime evidence control: re-derive the record facts
//!   (`design/activity-lifetime-state.md`) from a snapshot, saying NOT CAPTURED where the bytes
//!   are not there, instead of trusting a transcript.

// ⭐ The live-process foundation moved to `d1-live` (2026-08-09) and is RE-EXPORTED here, not
// copied.  `d1guest.py` proved what a second copy costs: five scripts re-derived the guest base and
// gave a superseded constant five places to survive a retraction (ATLAS §4.3).  The native
// hardware-watchpoint instrument needed exactly this base, these mappings and this address
// language, so it took ownership of them instead of forking them.  `crate::live`, `crate::addr` and
// `crate::symbols` still resolve, and they are the SAME types `d1-live` uses.
pub use d1_live::addr;
pub use d1_live::proc as live;
pub use d1_live::symbols;

pub mod controls;
pub mod machine;
pub mod onhit;
pub mod preset;
pub mod rederive;
pub mod search;
pub mod snapshot;

pub use snapshot::{Manifest, Snapshot};
