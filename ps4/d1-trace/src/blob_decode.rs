// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Native reader for the `D1BL` blob sidecar (`account-trace.bin.blob`).
//!
//! This is the Rust port of `capture-rig/world-launch-command.py`, absorbed so the shipped
//! `d1-trace` binary no longer spawns Python at runtime - it was the LAST ingest shell-out
//! (`ingest::load_blob_summary` used to run the script and scrape its stdout). It also carries the
//! two-command diff trick that bounds the world/activity fields: two known-different worlds
//! captured in the same run make the differing bytes a tight superset of the activity fields.
//!
//! ## The format, described here and only here
//!
//! One sidecar is a sequence of self-describing chunks, each:
//!
//! ```text
//! u32 magic 'D1BL' | u32 probe_id | u32 sequence | u32 length | u64 guest_address | length bytes
//! ```
//!
//! all little-endian; 24-byte header. [C] This matches the producer byte-for-byte: `WriteBlob` in
//! the shadPS4-d1trace fork's `d1_account_trace.cpp` writes `{BlobMagic, probe_id, sequence,
//! copied}` as four u32, then the address as u64, then the payload. ⚠ The prose comment ABOVE that
//! producer misstates the field order (it claims `u16 probe_id | u16 reserved | u32 sequence |
//! u64 guest_address | u32 length`); the code and every real sidecar under `runs/` agree with the
//! layout read here. A short guest read is recorded as a SHORT blob (`length` < 0x18F8), never
//! zero-filled - "we got 512 of 6392 bytes" and "the tail was zero" must not look the same.
//!
//! ## This is NOT the 232-byte trace record
//!
//! [`crate::probe_decode`] describes a different format: the fixed 232-byte `TraceRecord`
//! (magic `0x52543144`, version 2, `<IHHII15Q48s48s`). The two are distinct on every axis that
//! matters - magic (`0x4C423144` vs `0x52543144`), record shape (variable-length payload behind a
//! 24-byte header vs fixed 232 bytes), and purpose (oversized captures vs register snapshots).
//! They share only the probe-id/sequence *idspace*, which is what ties a blob back to the ordinary
//! trace record the same probe wrote. Do not unify them, and do not describe either twice.
//!
//! ## Byte-faithfulness and deliberate deviations
//!
//! Output and refusals reproduce the Python tool exactly (executed golden diff over real sidecars
//! and fixture refusals; see the porting session report). Three deviations, each of which turns a
//! silent or plausible outcome into a loud refusal:
//!
//!  1. a trailing PARTIAL header (1..23 stray bytes after the last complete blob) refuses; the
//!     Python silently ignored it, so a sidecar truncated mid-header summarised plausibly;
//!  2. `--dump`/`--extract` with an out-of-range index refuse with `no blob #N (have M)` (the
//!     message `--diff` already used); the Python died with an uncaught IndexError traceback;
//!  3. negative indices are rejected at argument parse; the Python quietly indexed from the end.

use std::fmt::Write as _;

use crate::probe_decode::compact_hex;

/// `'D1BL'` little-endian.
pub const BLOB_MAGIC: u32 = 0x4C42_3144;
/// magic + probe_id + sequence + length (u32 each) + guest address (u64).
pub const HEADER_SIZE: usize = 24;
/// The full world-launch command size; anything shorter is a partial guest read.
pub const EXPECTED_COMMAND_LEN: usize = 0x18F8;

/// One captured oversized sample (in practice: a `world_launch` command struct).
#[derive(Debug, Clone)]
pub struct Blob {
    pub probe_id: u32,
    pub sequence: u32,
    pub address: u64,
    pub bytes: Vec<u8>,
}

fn u32le(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(d[off..off + 4].try_into().unwrap())
}

fn u64le(d: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(d[off..off + 8].try_into().unwrap())
}

/// `bytes.hex()`: lowercase pairs, no separator.
fn hex_plain(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Python `repr()` for the strings this module produces (printable ASCII plus embedded NUL):
/// single quotes unless the string contains `'` and no `"`; escape the backslash, the active
/// quote, and controls as `\xNN`.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        if c == '\\' {
            out.push_str("\\\\");
        } else if c == quote {
            out.push('\\');
            out.push(c);
        } else if (c as u32) < 0x20 || (c as u32) == 0x7f {
            let _ = write!(out, "\\x{:02x}", c as u32);
        } else {
            out.push(c);
        }
    }
    out.push(quote);
    out
}

/// `bytes.rstrip(b"\0").decode("ascii", "replace")`.
fn ascii_rstrip_nul(d: &[u8]) -> String {
    let end = d.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    d[..end]
        .iter()
        .map(|&b| if b < 128 { b as char } else { '\u{FFFD}' })
        .collect()
}

/// Printable runs of at least `minimum` chars, with their offsets. World names travel as strings.
fn ascii_runs(payload: &[u8], minimum: usize) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut run = String::new();
    let mut start = 0usize;
    for (index, &byte) in payload.iter().enumerate() {
        if (32..127).contains(&byte) {
            if run.is_empty() {
                start = index;
            }
            run.push(byte as char);
        } else {
            if run.len() >= minimum {
                found.push((start, std::mem::take(&mut run)));
            }
            run.clear();
        }
    }
    if run.len() >= minimum {
        found.push((start, run));
    }
    found
}

/// Walk a sidecar into its blobs, refusing exactly where the Python refused (same messages), plus
/// the trailing-partial-header refusal the Python lacked (deviation 1 in the module doc).
pub fn read_blobs(data: &[u8]) -> Result<Vec<Blob>, String> {
    let mut blobs = Vec::new();
    let mut offset = 0usize;
    while offset + HEADER_SIZE <= data.len() {
        let magic = u32le(data, offset);
        if magic != BLOB_MAGIC {
            return Err(format!(
                "bad magic {magic:#010x} at offset {offset} - file is not a tracer blob sidecar"
            ));
        }
        let probe_id = u32le(data, offset + 4);
        let sequence = u32le(data, offset + 8);
        let length = u32le(data, offset + 12) as usize;
        let address = u64le(data, offset + 16);
        let start = offset + HEADER_SIZE;
        let end = start + length;
        if end > data.len() {
            return Err(format!("truncated blob at offset {offset}: wants {length} bytes"));
        }
        blobs.push(Blob {
            probe_id,
            sequence,
            address,
            bytes: data[start..end].to_vec(),
        });
        offset = end;
    }
    if blobs.is_empty() {
        // Byte-identical with the Python for the empty file AND for a file shorter than one
        // header - both land here, both are "no blobs".
        return Err("no blobs in file - did the world-launch probe fire?".to_string());
    }
    if offset != data.len() {
        // Deviation 1: the Python's `while offset + 24 <= len` silently ignored a stray tail, so
        // a sidecar truncated mid-header produced a plausible, complete-looking summary.
        return Err(format!(
            "trailing {} byte(s) at offset {offset} - not a complete {HEADER_SIZE}-byte blob \
             header; the sidecar is truncated mid-header",
            data.len() - offset
        ));
    }
    Ok(blobs)
}

fn check_index(blobs: &[Blob], index: usize) -> Result<(), String> {
    if index >= blobs.len() {
        return Err(format!("no blob #{index} (have {})", blobs.len()));
    }
    Ok(())
}

/// The default mode: one paragraph per blob. Byte-identical to the Python's stdout.
pub fn summarise(blobs: &[Blob]) -> String {
    let mut out = String::new();
    let _ = write!(out, "{} captured world_launch command(s)\n\n", blobs.len());
    for (index, blob) in blobs.iter().enumerate() {
        let payload = &blob.bytes;
        let _ = writeln!(
            out,
            "[{index}] sequence={} guest_addr={:#018x} len={} ({:#x})",
            blob.sequence,
            blob.address,
            payload.len(),
            payload.len()
        );
        if payload.len() != EXPECTED_COMMAND_LEN {
            let _ = writeln!(
                out,
                "     ⚠ SHORT - expected 0x18f8, got {:#x}. Partial read, not zero padding.",
                payload.len()
            );
        }
        let strings = ascii_runs(payload, 4);
        if !strings.is_empty() {
            let joined: Vec<String> = strings
                .iter()
                .take(8)
                .map(|(o, s)| format!("{}@{o:#x}", py_repr(s)))
                .collect();
            let _ = writeln!(out, "     strings: {}", joined.join(", "));
        }
        let nonzero = payload.iter().filter(|&&b| b != 0).count();
        let _ = writeln!(
            out,
            "     {nonzero} non-zero bytes ({}%)",
            100 * nonzero / payload.len().max(1)
        );
        let _ = writeln!(out, "     head: {}", hex_plain(&payload[..payload.len().min(32)]));
        out.push('\n');
    }
    out
}

/// ⭐ The whole trick: two known-different worlds captured in the same run; the differing bytes
/// bracket the world/activity fields, everything identical is scaffolding to copy verbatim.
pub fn diff(blobs: &[Blob], left: usize, right: usize) -> Result<String, String> {
    for index in [left, right] {
        check_index(blobs, index)?;
    }
    let a = &blobs[left].bytes;
    let b = &blobs[right].bytes;
    let mut out = String::new();
    if a.len() != b.len() {
        let _ = writeln!(
            out,
            "⚠ lengths differ: {:#x} vs {:#x}; comparing the common prefix",
            a.len(),
            b.len()
        );
    }
    let size = a.len().min(b.len());
    let differing: Vec<usize> = (0..size).filter(|&i| a[i] != b[i]).collect();
    let _ = write!(
        out,
        "blob[{left}] vs blob[{right}]: {} of {size} bytes differ ({}%)\n\n",
        differing.len(),
        100 * differing.len() / size.max(1)
    );
    if differing.is_empty() {
        out.push_str("IDENTICAL - these two launches carry the same command. If the worlds were\n");
        out.push_str("different, the distinguishing state is NOT in this struct.\n");
        return Ok(out);
    }

    // Group into runs so a field reads as one row instead of eight.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = differing[0];
    let mut previous = differing[0];
    for &offset in &differing[1..] {
        if offset == previous + 1 {
            previous = offset;
            continue;
        }
        runs.push((start, previous));
        start = offset;
        previous = offset;
    }
    runs.push((start, previous));

    let _ = write!(
        out,
        "{} differing region(s) - these bracket the world/activity fields:\n\n",
        runs.len()
    );
    for (begin, end) in runs {
        let length = end - begin + 1;
        let left_bytes = &a[begin..=end];
        let right_bytes = &b[begin..=end];
        let mut note = String::new();
        if length == 4 {
            note = format!("   as int: {:#x} -> {:#x}", u32le(left_bytes, 0), u32le(right_bytes, 0));
        } else if length == 8 {
            note = format!("   as int: {:#x} -> {:#x}", u64le(left_bytes, 0), u64le(right_bytes, 0));
        }
        let printable = left_bytes
            .iter()
            .chain(right_bytes.iter())
            .all(|&c| (32..127).contains(&c) || c == 0);
        if printable && length >= 4 {
            note = format!(
                "   as text: {} -> {}",
                py_repr(&ascii_rstrip_nul(left_bytes)),
                py_repr(&ascii_rstrip_nul(right_bytes))
            );
        }
        let _ = writeln!(out, "  +{begin:#06x} .. +{end:#06x}  ({length:>4} B){note}");
        if length <= 32 {
            let _ = writeln!(out, "        {}", hex_plain(left_bytes));
            let _ = writeln!(out, "        {}", hex_plain(right_bytes));
        }
    }
    out.push_str("\n⇒ Next: change ONLY these bytes in a copy of a known-good command and feed it to\n");
    out.push_str("  0x754460(command*). Then read which 0x753540 diagnostic fires.\n");
    Ok(out)
}

/// Hexdump one blob (deviation 2: an out-of-range index refuses instead of a traceback).
pub fn dump(blobs: &[Blob], index: usize) -> Result<String, String> {
    check_index(blobs, index)?;
    let payload = &blobs[index].bytes;
    let mut out = String::new();
    for offset in (0..payload.len()).step_by(16) {
        let chunk = &payload[offset..payload.len().min(offset + 16)];
        let text: String = chunk
            .iter()
            .map(|&c| if (32..127).contains(&c) { c as char } else { '.' })
            .collect();
        let _ = writeln!(out, "  {offset:#06x}  {:<48}  {text}", compact_hex(chunk));
    }
    Ok(out)
}

/// The raw command bytes of one blob (the caller writes them to `--out`).
pub fn extract(blobs: &[Blob], index: usize) -> Result<&[u8], String> {
    check_index(blobs, index)?;
    Ok(&blobs[index].bytes)
}

/// Build one syntactically valid sidecar chunk for tests and self-controls (the same pattern as
/// [`crate::probe_decode::make_record_bytes`]).
pub fn make_blob_bytes(probe_id: u32, sequence: u32, address: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
    out.extend_from_slice(&BLOB_MAGIC.to_le_bytes());
    out.extend_from_slice(&probe_id.to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&address.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `f-good` fixture the Python goldens were captured over: bytes 0..48, then a string that
    /// exercises BOTH repr quote rules (contains `'`, `"` and `\`), then 10 NULs. 72 bytes = SHORT.
    fn good_fixture() -> Vec<u8> {
        let mut payload: Vec<u8> = (0u8..48).collect();
        payload.extend_from_slice(b"hello'wor\"ld\\x");
        payload.extend_from_slice(&[0u8; 10]);
        make_blob_bytes(88, 52, 0x0000_0002_0503_b81c, &payload)
    }

    /// The deterministic two-world pair (orbit vs moon) the Python diff golden was captured over.
    fn pair_fixture() -> Vec<u8> {
        let mut pa = [0u8; 32];
        pa[8..13].copy_from_slice(b"orbit");
        pa[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x80]);
        pa[30] = 0x01;
        let mut pb = pa;
        pb[8..16].copy_from_slice(b"moon\0\0\0\0");
        pb[20..24].copy_from_slice(&[0xc5, 0x9d, 0x1c, 0x81]);
        pb[30] = 0x02;
        let mut data = make_blob_bytes(88, 52, 0x0000_0002_0503_b81c, &pa);
        data.extend_from_slice(&make_blob_bytes(86, 99, 0x0000_0007_efcf_1f48, &pb));
        data
    }

    /// Executed golden, pinned: `python3 -B capture-rig/world-launch-command.py f-good.bin`.
    /// Covers the SHORT warning, the repr of a string carrying `'`, `"` and `\`, the floor-divided
    /// percentage, and the 32-byte head.
    #[test]
    fn summarise_matches_python_golden() {
        let blobs = read_blobs(&good_fixture()).unwrap();
        let expected = "1 captured world_launch command(s)\n\n\
             [0] sequence=52 guest_addr=0x000000020503b81c len=72 (0x48)\n\
             \u{20}    ⚠ SHORT - expected 0x18f8, got 0x48. Partial read, not zero padding.\n\
             \u{20}    strings: ' !\"#$%&\\'()*+,-./hello\\'wor\"ld\\\\x'@0x20\n\
             \u{20}    61 non-zero bytes (84%)\n\
             \u{20}    head: 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\n\n";
        assert_eq!(summarise(&blobs), expected);
    }

    /// Executed golden, pinned: the pair summary (both strings lines, both SHORT warnings).
    #[test]
    fn summarise_pair_matches_python_golden() {
        let blobs = read_blobs(&pair_fixture()).unwrap();
        let expected = "2 captured world_launch command(s)\n\n\
             [0] sequence=52 guest_addr=0x000000020503b81c len=32 (0x20)\n\
             \u{20}    ⚠ SHORT - expected 0x18f8, got 0x20. Partial read, not zero padding.\n\
             \u{20}    strings: 'orbit'@0x8\n\
             \u{20}    7 non-zero bytes (21%)\n\
             \u{20}    head: 00000000000000006f7262697400000000000000000000800000000000000100\n\n\
             [1] sequence=99 guest_addr=0x00000007efcf1f48 len=32 (0x20)\n\
             \u{20}    ⚠ SHORT - expected 0x18f8, got 0x20. Partial read, not zero padding.\n\
             \u{20}    strings: 'moon'@0x8\n\
             \u{20}    9 non-zero bytes (28%)\n\
             \u{20}    head: 00000000000000006d6f6f6e0000000000000000c59d1c810000000000000200\n\n";
        assert_eq!(summarise(&blobs), expected);
    }

    /// Executed golden, pinned: `--diff 0 1` over the pair - a text region (with NUL rstrip), an
    /// int region, a single-byte region, the region grouping, and the footer.
    #[test]
    fn diff_matches_python_golden() {
        let blobs = read_blobs(&pair_fixture()).unwrap();
        let expected = "blob[0] vs blob[1]: 10 of 32 bytes differ (31%)\n\n\
             3 differing region(s) - these bracket the world/activity fields:\n\n\
             \u{20} +0x0008 .. +0x000c  (   5 B)   as text: 'orbit' -> 'moon'\n\
             \u{20}       6f72626974\n\
             \u{20}       6d6f6f6e00\n\
             \u{20} +0x0014 .. +0x0017  (   4 B)   as int: 0x80000000 -> 0x811c9dc5\n\
             \u{20}       00000080\n\
             \u{20}       c59d1c81\n\
             \u{20} +0x001e .. +0x001e  (   1 B)\n\
             \u{20}       01\n\
             \u{20}       02\n\n\
             ⇒ Next: change ONLY these bytes in a copy of a known-good command and feed it to\n\
             \u{20} 0x754460(command*). Then read which 0x753540 diagnostic fires.\n";
        assert_eq!(diff(&blobs, 0, 1).unwrap(), expected);
    }

    /// Executed golden, pinned: unequal lengths warn and compare the common prefix.
    #[test]
    fn diff_unequal_lengths_matches_python_golden() {
        let mut pa = [0u8; 32];
        pa[8..13].copy_from_slice(b"orbit");
        pa[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x80]);
        pa[30] = 0x01;
        let mut pb_long = pa.to_vec();
        pb_long[8..16].copy_from_slice(b"moon\0\0\0\0");
        pb_long[20..24].copy_from_slice(&[0xc5, 0x9d, 0x1c, 0x81]);
        pb_long[30] = 0x02;
        pb_long.extend_from_slice(b"XY");
        let mut data = make_blob_bytes(88, 1, 0x1000, &pa);
        data.extend_from_slice(&make_blob_bytes(86, 2, 0x2000, &pb_long));
        let blobs = read_blobs(&data).unwrap();
        let got = diff(&blobs, 0, 1).unwrap();
        assert!(
            got.starts_with(
                "⚠ lengths differ: 0x20 vs 0x22; comparing the common prefix\n\
                 blob[0] vs blob[1]: 10 of 32 bytes differ (31%)\n"
            ),
            "{got}"
        );
    }

    /// Two identical commands: the diff must say so, in the Python's words (this is the negative
    /// arm of the two-worlds trick - "the distinguishing state is NOT in this struct").
    #[test]
    fn diff_identical_matches_python_golden() {
        let pa = [7u8; 32];
        let mut data = make_blob_bytes(88, 1, 0x1000, &pa);
        data.extend_from_slice(&make_blob_bytes(88, 2, 0x1000, &pa));
        let blobs = read_blobs(&data).unwrap();
        let expected = "blob[0] vs blob[1]: 0 of 32 bytes differ (0%)\n\n\
             IDENTICAL - these two launches carry the same command. If the worlds were\n\
             different, the distinguishing state is NOT in this struct.\n";
        assert_eq!(diff(&blobs, 0, 1).unwrap(), expected);
    }

    /// Executed golden, pinned: `--dump 0` over f-good - offset column, 48-wide hex column, the
    /// printable gutter, and the padded final partial row.
    #[test]
    fn dump_matches_python_golden() {
        let blobs = read_blobs(&good_fixture()).unwrap();
        let expected = "\
             \u{20} 0x0000  00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f   ................\n\
             \u{20} 0x0010  10 11 12 13 14 15 16 17 18 19 1a 1b 1c 1d 1e 1f   ................\n\
             \u{20} 0x0020  20 21 22 23 24 25 26 27 28 29 2a 2b 2c 2d 2e 2f    !\"#$%&'()*+,-./\n\
             \u{20} 0x0030  68 65 6c 6c 6f 27 77 6f 72 22 6c 64 5c 78 00 00   hello'wor\"ld\\x..\n\
             \u{20} 0x0040  00 00 00 00 00 00 00 00                           ........\n";
        assert_eq!(dump(&blobs, 0).unwrap(), expected);
    }

    /// Refusals, byte-identical with the Python (each of these CAN fail: delete a check in
    /// `read_blobs` and it does).
    #[test]
    fn refuses_bad_magic_with_the_python_message() {
        let mut bad = make_blob_bytes(1, 2, 0, b"abcd");
        bad[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        assert_eq!(
            read_blobs(&bad).unwrap_err(),
            "bad magic 0xdeadbeef at offset 0 - file is not a tracer blob sidecar"
        );
        // The second blob's offset is interpolated into the message.
        let mut two = good_fixture();
        two.extend_from_slice(&bad);
        assert_eq!(
            read_blobs(&two).unwrap_err(),
            "bad magic 0xdeadbeef at offset 96 - file is not a tracer blob sidecar"
        );
    }

    #[test]
    fn refuses_truncated_payload_with_the_python_message() {
        let good = good_fixture();
        assert_eq!(
            read_blobs(&good[..good.len() - 5]).unwrap_err(),
            "truncated blob at offset 0: wants 72 bytes"
        );
    }

    #[test]
    fn refuses_empty_and_sub_header_files_with_the_python_message() {
        let msg = "no blobs in file - did the world-launch probe fire?";
        assert_eq!(read_blobs(&[]).unwrap_err(), msg);
        // 23 bytes cannot hold one header; the Python also lands on "no blobs" here.
        assert_eq!(read_blobs(&[0u8; 23]).unwrap_err(), msg);
    }

    /// Deviation 1: a stray partial header AFTER a valid blob refuses loudly. The Python silently
    /// summarised the file as complete - the exact "plausible partial result" failure shape.
    #[test]
    fn refuses_trailing_partial_header_unlike_the_python() {
        let mut data = good_fixture();
        data.extend_from_slice(&[0x44, 0x31, 0x42, 0x4c, 0x00, 0x00, 0x00]);
        let err = read_blobs(&data).unwrap_err();
        assert!(err.contains("truncated mid-header"), "{err}");
        assert!(err.contains("trailing 7 byte(s) at offset 96"), "{err}");
    }

    /// Index refusals: `--diff`'s message (Python) now also guards `dump`/`extract` (deviation 2).
    #[test]
    fn refuses_out_of_range_indices() {
        let blobs = read_blobs(&good_fixture()).unwrap();
        assert_eq!(diff(&blobs, 0, 5).unwrap_err(), "no blob #5 (have 1)");
        assert_eq!(dump(&blobs, 3).unwrap_err(), "no blob #3 (have 1)");
        assert_eq!(extract(&blobs, 9).unwrap_err(), "no blob #9 (have 1)");
    }

    #[test]
    fn extract_returns_the_exact_payload() {
        let blobs = read_blobs(&good_fixture()).unwrap();
        let payload = extract(&blobs, 0).unwrap();
        assert_eq!(payload.len(), 72);
        assert_eq!(&payload[..4], &[0, 1, 2, 3]);
        assert_eq!(&payload[62..72], &[0u8; 10]);
    }

    /// INVERTED control: perturb one sidecar payload byte, prove the output changes.
    #[test]
    fn perturbing_the_sidecar_changes_the_output() {
        let clean = pair_fixture();
        let mut perturbed = clean.clone();
        perturbed[HEADER_SIZE + 8] ^= 0x01; // first payload's 'o' in "orbit"
        let a = summarise(&read_blobs(&clean).unwrap());
        let b = summarise(&read_blobs(&perturbed).unwrap());
        assert_ne!(a, b, "a one-bit sidecar change must change the summary");
        let da = diff(&read_blobs(&clean).unwrap(), 0, 1).unwrap();
        let db = diff(&read_blobs(&perturbed).unwrap(), 0, 1).unwrap();
        assert_ne!(da, db, "a one-bit sidecar change must change the diff");
    }
}
