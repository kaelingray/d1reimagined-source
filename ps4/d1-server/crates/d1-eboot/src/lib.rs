// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-eboot` - the static-analysis instruments for the Destiny 1 PS4 eboot.
//!
//! This crate is ATLAS Layer 1 (`historicaldocs/design/ATLAS.md` §3), in the language the project ships in. It is
//! **not** a wire-protocol implementation: nothing here encodes or decodes anything that goes on a
//! socket. Protocol code lives in `d1-bap` under *one protocol, one implementer*.
//!
//! What moved here, and why these four and not others:
//!
//! | module | replaces | why it had to be one implementation |
//! |---|---|---|
//! | [`image`] | the `BIAS = 0x4000` constant copied into ~12 scripts | the bias is **per-segment**; one wrong unification corrupts every relocation read (ATLAS §4.1) |
//! | [`reloc`] | `oodle/reloc-xref.py` | vtable slots are **zero in the file**; without the addend map, "not found" is meaningless |
//! | [`disasm`] | four separate Python disassemblers | prologue anchoring, and reporting truncation instead of swallowing it (ATLAS §4.5) |
//! | [`xref`] | `find-direct-call-xrefs.py` + `oodle/global-xref.py` | scanning `call` but not `jmp` cost two sessions (ATLAS §4.2) |
//! | [`fieldxref`] | nothing - the technique was missing | an absolute-address xref is blind to `[rdi+8]` **by construction**, and that blindness cost the rung-33 question a session |
//! | [`descriptors`] | `eboot-descriptors.py` | four parser defects that each produced a confident wrong reading |
//! | [`catalogue`] | `message-catalogue.py` | the "does the client want a reply" oracle is consulted by the server, so it must be callable from Rust; it also re-declared the descriptor anchor constants, which now exist once |
//! | [`ladder`] | `state-machine.py` | the 35-rung boot-ladder table is load-bearing UI in `d1-trace dash`, which shelled out to the Python (~0.75 s per load, 36 subprocesses); derived from RTTI + relocations in-process, never transcribed |
//!
//! Everything that can return "nothing" carries a **positive control** and says PASS/FAIL, because
//! a scan without one is not evidence of absence.

pub mod atlas;
pub mod catalogue;
pub mod descriptors;
pub mod disasm;
pub mod dynsym;
pub mod fieldxref;
pub mod functions;
pub mod image;
pub mod ladder;
pub mod nametab;
pub mod reloc;
pub mod rtti;
pub mod strings;
pub mod xref;

pub use image::{Image, ImageError, Segment};
