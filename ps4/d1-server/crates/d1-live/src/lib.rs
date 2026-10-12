// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-live` - **the live client as an ordinary Linux process**, and the one instrument that can
//! answer *"which instruction wrote this field?"* in a single run.
//!
//! # The gap this closes
//!
//! "Who writes `c_activity_loader+8`?" is a **runtime** question, and this project has repeatedly
//! tried to answer it with static scans that are structurally blind to the only kind of write that
//! matters:
//!
//! | instrument | answers | blind to |
//! |---|---|---|
//! | `d1-eboot xref` | *which code mentions this address* | `mov [rbx+8], eax` - a register-relative store mentions nothing |
//! | `capture-rig/eboot-callgraph.tsv` | direct call edges | `lea`-materialised pointers (it fails 4/11 controls - CLAUDE.md trap 12) |
//! | `d1-replay` (Unicorn) | *what a function does to captured memory* | the moment; a snapshot is a **state**, a write is an **event** |
//! | UD2 runtime probes | *which branch a chosen site took* | needs the site already guessed, and one bad guard yields an empty trace |
//! | **`d1-live watch`** | **the RIP of the store, on the first hit, with no candidate list** | writes by threads created after arming; DMA/kernel writes |
//!
//! The box is x86-64 with `kernel.yama.ptrace_scope = 0`, and shadPS4 executes guest code
//! **natively**. So a CPU data-write breakpoint in `DR0`..`DR3` catches the guest's own store
//! instruction. No search space, no premise to get wrong, no positive-control-shaped hole: either
//! it traps and names an address, or nothing writes that byte range on the threads we armed.
//!
//! # ⚠ WHAT THIS INSTRUMENT CANNOT DO - read before quoting any result
//!
//! 1. **Debug registers are PER-THREAD.** Linux stores `DR0..DR7` in `task_struct`, not per
//!    process. shadPS4 has dozens of threads; a watchpoint armed on one of them is invisible to
//!    every other. `watch` therefore arms **every thread in `/proc/<pid>/task`** by default. A
//!    thread created *after* arming is **not** armed unless `--follow-clone` is given - and a miss
//!    from that cause looks exactly like "nothing wrote it".
//! 2. **A data breakpoint is a TRAP, not a FAULT.** The exception is delivered *after* the storing
//!    instruction retires, so the reported `RIP` is the **next** instruction. `watch` decodes
//!    backwards to name the store and reports both addresses; never quote the raw `RIP` as "the
//!    writer". (An *execute* breakpoint, `--rw exec`, is a fault: there `RIP` is the instruction.)
//! 3. **Four slots, hardware.** `DR0..DR3`. Address must be naturally aligned to its length
//!    (`--len 8` ⇒ 8-byte aligned). Misalignment is refused, not silently narrowed.
//! 4. **It perturbs.** `PTRACE_SEIZE` does not stop the target, but arming needs one
//!    `PTRACE_INTERRUPT` stop per thread (microseconds), and **every trap stops that thread until
//!    we continue it**. A watch on a hot field will visibly slow a live run.
//! 5. **⛔ Do not `SIGKILL` this tool while it is armed.** It disarms on exit, on panic, and on
//!    `SIGINT`/`SIGTERM`, and it self-disarms after `--max-seconds`. `SIGKILL` skips all of that,
//!    and whether the kernel flushes a ptrace hardware breakpoint on implicit detach is a
//!    *measured* question - [`controls::tracer_death_safety`] measures it, on our own child,
//!    every time `controls` runs.
//! 6. **Everything downstream inherits the run's reach tag.** Watching a client that reached its
//!    state over our synthetic server is `synthetic_ancestor`, however exact the RIP is.
//!
//! # Address semantics - identical to `capture-rig/d1guest.py`, deliberately
//!
//! ⚠ CLAUDE.md trap 1, paid for with a retracted causal theory: **only eboot vaddrs get the base.
//! Anything read *out of* memory is already absolute.**
//!
//! * a bare number, or a `guest-symbols.json` name, or `v:0x…` ⇒ an **eboot vaddr**, `base + N`;
//! * `--abs`, or `a:0x…` ⇒ an **absolute** host address, used as-is;
//! * `[…]` dereferences through the live process, and `+`/`-` after it are plain offsets.
//!
//! **A heap address must be `--abs` (or `a:`).** Every resolution is printed, because a resolution
//! nobody saw is a resolution nobody checked.
//!
//! # Modules
//!
//! * [`addr`] - the address/value expression language (shared with `d1-replay`).
//! * [`symbols`] - reads `capture-rig/guest-symbols.json`, the project's one address book.
//! * [`proc`] - `/proc/<pid>/mem`, mappings, the three base controls, the liveness ladder.
//! * [`ptrace`] - the raw `ptrace(2)` surface, including the **derived** `u_debugreg` offset.
//! * [`watch`] - arm, wait, report, disarm.
//! * [`session`] - `open_guest`: the build gate, the one-emulator rule, the base controls, the ladder.
//! * [`registry`] - the object registry, the package registry, and the descriptor table.
//! * [`format`] - rendering, byte-identical to `d1guest.py`.
//! * [`fixture`] - a synthetic guest address space, so the read side has controls that can FAIL.
//! * [`controls`] - the watchpoint controls, each with its inversion.
//! * [`readcontrols`] - the READ-SIDE controls, each with its inversion, driven off [`fixture`].
//!
//! # The read side - `d1guest.py` parity
//!
//! ```text
//! d1-live read profile_setup_kind                  # by name, with its provenance
//! d1-live read 0x263D0F8 --len 0x270               # by eboot vaddr
//! d1-live read character_selection_manager --deref --struct character_selection_node
//! d1-live read boot_state_names --len 320 --as ptr # relocated slots: zero in the file
//! d1-live obj 4:0                                  # the account object (family 4, type 0)
//! d1-live obj 3:1 --key 0xD100000000000101         # resolve by KEY, not index
//! d1-live tag 0x80801825                           # package-registry tag -> object
//! d1-live descriptors [--watch]                    # the QueueZ replication descriptor table
//! ```
//!
//! ⛔ **`poke` is deliberately absent.** Writing into a live client is the one operation where a
//! half-ported tool is dangerous, it is the lowest-quality hop mechanic this project has (a written
//! value never crossed a wire, so it is not evidence even about our own server), and it deserves its
//! own reviewed change with its own journal. Use `capture-rig/d1guest.py poke` until then.

pub mod addr;
pub mod controls;
pub mod fixture;
pub mod format;
pub mod proc;
pub mod ptrace;
pub mod readcontrols;
pub mod registry;
pub mod session;
pub mod symbols;
pub mod watch;
