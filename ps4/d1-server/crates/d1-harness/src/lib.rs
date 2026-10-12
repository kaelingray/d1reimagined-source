// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-harness` - the two-participant lab apparatus, **built cold**.
//!
//! Four things live here and nothing else:
//!
//! 1. [`layout`] - one run id, two instances, two participants, every mutable root separate and
//!    every endpoint deterministic. Pure arithmetic: it creates no directory, opens no socket and
//!    launches nothing.
//! 2. [`preflight`] - the executable form of `historicaldocs/design/two-instance-harness-design.md`'s activation
//!    checklist. It **refuses with a named reason** rather than proceeding on a doubt, and a check
//!    it cannot evaluate refuses too.
//! 3. [`procs`] - shutdown that signals only resolved, re-verified process ids recorded for the
//!    run. No pattern, no unresolved variable, no pid ≤ 1.
//! 4. [`pad`] - the `/dev/uinput` virtual gamepad (`pad run`), ported from
//!    `capture-rig/virtual-pad.py` with `--vendor/--product` so two instances can present two
//!    distinct pads. A kernel input device is not an emulator: creating one starts nothing.
//!
//! ⛔ **Nothing in this crate starts an emulator, and nothing lifts the one-emulator guard.** The
//! guard is lifted by an operator's activation review; the most this crate will ever say is
//! `READY`, which means "every check passed", not "something is running".

pub mod controls;
pub mod layout;
pub mod pad;
pub mod preflight;
pub mod procs;
