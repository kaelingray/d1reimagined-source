// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ **THE GOLDEN DIFF, as a test** - the Rust path against the Python path's own printed digest.
//!
//! `capture-rig/oodle/oodle-decompress.py` prints a **SHA-1**, and every doc that quotes the pinned
//! block quotes that SHA-1. The crate pins a SHA-256 (`d1-pkg` already carries `sha2`, and
//! `manifest::sha256_hex` is the tree's one digest helper), so on its own the crate can never be
//! matched by eye against the Python's output line. This test closes that gap by computing the
//! **SHA-1 of the same bytes** and asserting it equals the number the Python printed:
//!
//! ```text
//! ./d1-server/target/release/d1-pkg --dir dumps/patch-01.29-CUSA00219/packages \
//!     extract-block ps4_investment_globals_012f_3 1063 -o /tmp/blk1063.z
//! PYTHONPATH=~/.local/lib/d1-tools python3 -B capture-rig/oodle/oodle-decompress.py \
//!     /tmp/blk1063.z /tmp/blk1063.bin
//! → 262144 bytes from 32463 stored, 16 chunks, consumed 32463/32463
//! → plain sha1 b7af28cbb6410f85ebf1b110e9a58af30594c1f3
//! ```
//!
//! ⚠ This file previously carried its own copy of the `DecodeSome` loop, which made **three**
//! descriptions of it in the tree (here, `oodle-decompress.py`, `content-table.py`). It now calls
//! [`d1_oracle::oodle`] instead: one description, in Rust, as the project rule requires.
//!
//! ⚠ Skips (does not fail) when the dump estate is absent, so a clone without owned media still
//! runs the suite.

use std::io::Write;
use std::process::Command;

use d1_oracle::oodle::{
    default_pkgdir, Content, PIN_BLOCK, PIN_BLOCK_PLAIN_LEN, PIN_BLOCK_PLAIN_SHA1, PIN_PACKAGE,
};

/// No `sha1` dependency in `d1-oracle`, and this is the only place that needs one. Shell out to
/// coreutils rather than widen the crate's dependency surface for a single cross-check.
fn sha1_hex(b: &[u8]) -> Option<String> {
    let mut c = Command::new("sha1sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    c.stdin.as_mut()?.write_all(b).ok()?;
    let o = c.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&o.stdout).split_whitespace().next()?.to_string())
}

#[test]
fn the_rust_decode_matches_the_sha1_the_python_tool_printed() {
    if !default_pkgdir().is_dir() {
        eprintln!("SKIP: no package estate");
        return;
    }
    let mut c = Content::open_default().expect("estate + eboot");
    let (stored, compressed) = c.stored_block(PIN_PACKAGE, PIN_BLOCK).expect("stored block");
    assert!(compressed, "block {PIN_BLOCK} must be Oodle-compressed for this to test anything");
    let d = c.oodle().decompress_whole(&stored).expect("decode");

    assert_eq!(d.plain.len(), PIN_BLOCK_PLAIN_LEN);
    assert_eq!(d.consumed, stored.len(), "every stored byte must be consumed");
    assert_eq!(d.chunks, 16, "the Python reported 16 chunks for this block");

    let Some(got) = sha1_hex(&d.plain) else {
        eprintln!("SKIP: no sha1sum on PATH");
        return;
    };
    assert_eq!(
        got, PIN_BLOCK_PLAIN_SHA1,
        "the Rust decode disagrees with the SHA-1 oodle-decompress.py printed for the same block"
    );
}
