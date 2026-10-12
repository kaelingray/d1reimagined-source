// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Byte-offset tailing - the one thing no crate in this tree does.
//!
//! Four rules, each with the failure it prevents:
//!
//! 1. **Byte-offset, never a whole-file re-read.** `emulator.log` grows **38.9 KiB/s while idle in
//!    orbit** and the largest on disk is **234 MB**; `d1_run::read_lossy` does `read_to_string`,
//!    which is fine for a one-shot CLI and fatal at 1 Hz.
//! 2. **Hold the partial-line remainder across polls, as BYTES.** A tail that happens to land on a
//!    line boundary is luck. Holding bytes (not a `String`) also means a snapshot that lands
//!    mid-multibyte-character does not have to be decoded until the rest of the character arrives.
//! 3. **`from_utf8_lossy`, never `Err(_) => Ok(vec![])`.** F3 turned one `0xff` byte into **zero
//!    guest events at exit 0** - a silent lane loss that reads exactly like "the game never got
//!    there". A lossy decode is reported as a loud note, not swallowed.
//! 4. **Truncation is a domain fact.** If the file shrinks (a new run reused the path), the offset
//!    is reset and the caller is told, rather than reading garbage from the middle of a line.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// What one poll produced.
#[derive(Debug, Default)]
pub struct Batch {
    /// The within-lane ordinal of `lines[0]`, stamped by the tailer that produced the batch.
    ///
    /// ⛔ R3 (review 2026-08-09, fixed 2026-08-11): the consumer must take ordinals from HERE, not
    /// from any tailer it happens to hold. `Session::absorb` used to derive them from a `Tailer`
    /// frozen at load time while the GUI polled a *different* one - tailed lines collided, went
    /// backwards, and on a short log the subtraction wrapped `u64` to ~1.8e19.
    pub first_line_no: u64,
    /// Complete lines only. A trailing partial line stays in the tailer's remainder.
    pub lines: Vec<String>,
    /// True if any byte in this batch failed to decode as UTF-8 and was replaced.
    pub lossy: bool,
    /// True if the file shrank since the last poll (rotated or truncated).
    pub truncated: bool,
    /// Non-empty when the read itself failed. **An empty batch with an empty error is a lie.**
    pub error: String,
}

/// A byte-offset tail over one append-only file.
#[derive(Debug)]
pub struct Tailer {
    pub path: PathBuf,
    offset: u64,
    remainder: Vec<u8>,
    /// Line numbers are the tailer's own count, so they are stable across polls and are a
    /// **within-lane ordinal only** - never a cross-lane order key.
    next_line_no: u64,
}

impl Tailer {
    /// Start at the beginning of the file.
    pub fn new(path: impl Into<PathBuf>) -> Tailer {
        Tailer {
            path: path.into(),
            offset: 0,
            remainder: Vec::new(),
            next_line_no: 0,
        }
    }

    /// Start after `offset` bytes - used when the initial whole-file load already consumed them.
    pub fn resuming_at(path: impl Into<PathBuf>, offset: u64, next_line_no: u64) -> Tailer {
        Tailer {
            path: path.into(),
            offset,
            remainder: Vec::new(),
            next_line_no,
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn next_line_no(&self) -> u64 {
        self.next_line_no
    }

    /// Read whatever has been appended since the last poll and split it into complete lines.
    pub fn poll(&mut self) -> Batch {
        let mut batch = Batch {
            first_line_no: self.next_line_no,
            ..Batch::default()
        };
        let len = match std::fs::metadata(&self.path) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return batch,
            Err(e) => {
                batch.error = format!("{}: {e}", self.path.display());
                return batch;
            }
        };
        if len < self.offset {
            // Rule 4: the file shrank. Say so; do not read from the middle of a line.
            batch.truncated = true;
            self.offset = 0;
            self.remainder.clear();
        }
        if len == self.offset {
            return batch;
        }
        let mut f = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                batch.error = format!("{}: {e}", self.path.display());
                return batch;
            }
        };
        if let Err(e) = f.seek(SeekFrom::Start(self.offset)) {
            batch.error = format!("{}: seek: {e}", self.path.display());
            return batch;
        }
        let want = (len - self.offset) as usize;
        let mut buf = vec![0u8; want];
        let got = match f.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                batch.error = format!("{}: read: {e}", self.path.display());
                return batch;
            }
        };
        buf.truncate(got);
        self.offset += got as u64;

        // Rule 2: prepend the held remainder, then split on newlines and hold the tail again.
        self.remainder.extend_from_slice(&buf);
        let mut start = 0usize;
        let mut cut = 0usize;
        while let Some(rel) = self.remainder[start..].iter().position(|b| *b == b'\n') {
            let end = start + rel;
            let raw = &self.remainder[start..end];
            // Rule 3: lossy decode, and SAY so.
            let s = String::from_utf8_lossy(raw);
            if matches!(s, std::borrow::Cow::Owned(_)) {
                batch.lossy = true;
            }
            batch.lines.push(s.trim_end_matches('\r').to_string());
            self.next_line_no += 1;
            start = end + 1;
            cut = start;
        }
        if cut > 0 {
            self.remainder.drain(..cut);
        }
        batch
    }
}

/// Read a whole file once and return `(text, bytes_consumed)`, so the tailer can resume exactly
/// where the load stopped with no race window: the offset is the length of what was actually read,
/// not a second `metadata()` call taken afterwards.
pub fn load_and_offset(path: &Path) -> (String, u64, bool, Option<String>) {
    match std::fs::read(path) {
        Ok(bytes) => {
            let n = bytes.len() as u64;
            let text = String::from_utf8_lossy(&bytes);
            let lossy = matches!(text, std::borrow::Cow::Owned(_));
            (text.into_owned(), n, lossy, None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            String::new(),
            0,
            false,
            Some(format!("{} is absent - that lane is empty", path.display())),
        ),
        Err(e) => (
            String::new(),
            0,
            false,
            Some(format!(
                "⚠ could not read {}: {e} - that lane is EMPTY BECAUSE OF AN ERROR, not because \
                 the run produced nothing",
                path.display()
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d1-dash-tail-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn append(p: &Path, s: &[u8]) {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
        f.write_all(s).unwrap();
    }

    /// ⭐ The axis: does a line split across two polls get lost, duplicated, or corrupted?
    /// A one-shot read of the final file is the oracle.
    #[test]
    fn a_line_split_across_polls_arrives_exactly_once() {
        let d = tmpdir("split");
        let p = d.join("emulator.log");
        append(&p, b"alpha\nbravo\ncharl");
        let mut t = Tailer::new(&p);
        let b1 = t.poll();
        assert_eq!(b1.lines, vec!["alpha", "bravo"]);
        append(&p, b"ie\ndel");
        let b2 = t.poll();
        assert_eq!(b2.lines, vec!["charlie"]);
        append(&p, b"ta\n");
        let b3 = t.poll();
        assert_eq!(b3.lines, vec!["delta"]);

        let all: Vec<String> = b1
            .lines
            .into_iter()
            .chain(b2.lines)
            .chain(b3.lines)
            .collect();
        let oracle: Vec<String> = std::fs::read_to_string(&p)
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(all, oracle, "tail must be byte-identical to a one-shot read");
        assert_eq!(t.next_line_no(), 4);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// F3: one bad byte must not zero the lane, and the lossiness must be LOUD.
    #[test]
    fn an_invalid_byte_keeps_every_other_line_and_says_so() {
        let d = tmpdir("lossy");
        let p = d.join("emulator.log");
        append(&p, b"first\ntainted \xff line\nthird\n");
        let mut t = Tailer::new(&p);
        let b = t.poll();
        assert_eq!(b.lines.len(), 3, "line count, not `> 0`");
        assert_eq!(b.lines[0], "first");
        assert_eq!(b.lines[2], "third");
        assert!(b.lossy, "the lossy decode must be reported");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A multibyte character split by the poll boundary must not be mangled - holding BYTES, not a
    /// String, is what makes that work.
    #[test]
    fn a_multibyte_char_split_across_polls_survives() {
        let d = tmpdir("mb");
        let p = d.join("emulator.log");
        let star = "⭐".as_bytes(); // 3 bytes
        append(&p, b"pre ");
        append(&p, &star[..1]);
        let mut t = Tailer::new(&p);
        let b1 = t.poll();
        assert!(b1.lines.is_empty());
        append(&p, &star[1..]);
        append(&p, b" post\n");
        let b2 = t.poll();
        assert_eq!(b2.lines, vec!["pre ⭐ post"]);
        assert!(!b2.lossy, "a rejoined character is not a lossy decode");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn truncation_is_a_domain_fact() {
        let d = tmpdir("trunc");
        let p = d.join("emulator.log");
        append(&p, b"one\ntwo\n");
        let mut t = Tailer::new(&p);
        assert_eq!(t.poll().lines.len(), 2);
        std::fs::write(&p, b"fresh\n").unwrap();
        let b = t.poll();
        assert!(b.truncated);
        assert_eq!(b.lines, vec!["fresh"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_absent_file_is_an_empty_batch_with_no_error() {
        let d = tmpdir("absent");
        let mut t = Tailer::new(d.join("nope.log"));
        let b = t.poll();
        assert!(b.lines.is_empty() && b.error.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
