// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The image's own string corpus, and the **FNV-1** name-hash oracle it feeds.
//!
//! ## ⚠ FNV-1, not FNV-1a - and this project has lost that fact once already
//!
//! `historicaldocs/design/THE-CHAIN.md` §4 is the record: the hash is **FNV-1** (multiply *then* xor), it was
//! confirmed in `[C]` form on 2026-08-06 in two separate documents, at least eight other documents
//! then said FNV-1a, and a whole session was spent re-deriving it. **FNV-1a is refuted at exactly
//! 1.00× a length-matched random control - it is pure noise.** The two share the `0x811C9DC5`
//! basis, which is how the confusion survives.
//!
//! ```text
//! h = 0x811C9DC5;  for each byte c:  h = (h * 0x01000193) ^ c        <- FNV-1
//!                                    h = (h ^ c) * 0x01000193        <- FNV-1a  (WRONG here)
//! ```
//!
//! ⚠ **Two hash spaces, and conflating them is what lost the finding** (THE-CHAIN §4.3). This
//! module serves **space 1 only** - enum members, bootflow states, error reasons, `sku_config`
//! option names, world/mode names. **Space 2** - the 6,338 + 1,332 runtime *type-name* registries
//! at `0x19B3530` / `0x19D0BD8` - is **not** FNV-anything; it is baked by the schema compiler and
//! has been proven uncrackable from the binary (118M trials with injection and scramble controls).
//! Do not point this at those registries and read the miss as a defect.
//!
//! ## The measured base rate, which is what makes a hit mean anything
//!
//! A uniform random `u32` occurs somewhere in the 26 MB image with probability **0.130%**. The
//! eboot's own identifier strings hash to a value present in the image **0.490%** of the time -
//! 5.75× a length-matched random control, ≈27σ. So a single hit is evidence; it is not proof, and
//! [`Corpus::resolve`] returns every candidate rather than picking one.

use std::collections::HashMap;

use serde::Serialize;

use crate::image::{Image, Result};

pub const FNV1_BASIS: u32 = 0x811C_9DC5;
pub const FNV1_PRIME: u32 = 0x0100_0193;

/// FNV-1 (multiply **then** xor). See the module docs before reaching for FNV-1a.
pub fn fnv1(s: &[u8]) -> u32 {
    let mut h = FNV1_BASIS;
    for &c in s {
        h = h.wrapping_mul(FNV1_PRIME) ^ c as u32;
    }
    h
}

/// FNV-1 over the lowercased spelling. `sku_config`'s site provably lowercases (`tolower` at
/// `0x64E20`); the enum-table site is indistinguishable because every recovered name is already
/// lower case (THE-CHAIN §4.3), so both forms are indexed and it costs nothing.
pub fn fnv1_lower(s: &str) -> u32 {
    fnv1(s.to_ascii_lowercase().as_bytes())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StrKind {
    /// `[A-Za-z_][A-Za-z0-9_]*` - the shape a hashed name has.
    Identifier,
    /// Contains `\` or `/` and ends `.cpp`/`.h` - a `__FILE__` from an assert or log macro.
    SourcePath,
    /// Contains a `%` conversion - a diagnostic format string.
    Format,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct Str {
    pub vaddr: u64,
    pub len: usize,
    pub kind: StrKind,
    pub text: String,
}

fn classify(s: &str) -> StrKind {
    let b = s.as_bytes();
    if (s.ends_with(".cpp") || s.ends_with(".h") || s.ends_with(".inl"))
        && (s.contains('\\') || s.contains('/'))
    {
        return StrKind::SourcePath;
    }
    if !b.is_empty()
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
    {
        return StrKind::Identifier;
    }
    if s.contains('%') {
        return StrKind::Format;
    }
    StrKind::Other
}

/// Every NUL-terminated printable run of at least `min_len` bytes in a file-backed LOAD segment.
///
/// Read-only data lives inside the same `PF_X` segment as the code in this image (segment 0 spans
/// `0x0..0x18bf0dc`), so this deliberately does not filter by segment flags.
pub fn extract(image: &Image, min_len: usize) -> Result<Vec<Str>> {
    let mut out = Vec::new();
    for seg in image.load_segments() {
        let r = seg.file_range();
        let blob = &image.data()[r.start..r.end.min(image.data().len())];
        let mut i = 0usize;
        while i < blob.len() {
            let c = blob[i];
            if !(0x20..0x7F).contains(&c) && c != b'\t' && c != b'\n' {
                i += 1;
                continue;
            }
            let start = i;
            while i < blob.len() {
                let c = blob[i];
                if (0x20..0x7F).contains(&c) || c == b'\t' || c == b'\n' {
                    i += 1;
                } else {
                    break;
                }
            }
            // A string is only a string if it is NUL-terminated; otherwise it is a byte run that
            // happens to be printable, and those are what make a naive `strings` untrustworthy.
            if i < blob.len() && blob[i] == 0 && i - start >= min_len {
                let text = String::from_utf8_lossy(&blob[start..i]).into_owned();
                out.push(Str {
                    vaddr: seg.vaddr + start as u64,
                    len: i - start,
                    kind: classify(&text),
                    text,
                });
            }
            i += 1;
        }
    }
    Ok(out)
}

#[derive(Debug, Default)]
pub struct Corpus {
    pub strings: Vec<Str>,
    /// FNV-1 (and lowercased FNV-1) of every identifier-like string → its spellings.
    hashes: HashMap<u32, Vec<String>>,
}

impl Corpus {
    pub fn build(image: &Image, min_len: usize) -> Result<Self> {
        let strings = extract(image, min_len)?;
        let mut hashes: HashMap<u32, Vec<String>> = HashMap::new();
        let add = |tok: &str, hashes: &mut HashMap<u32, Vec<String>>| {
            for h in [fnv1(tok.as_bytes()), fnv1_lower(tok)] {
                let e = hashes.entry(h).or_default();
                if !e.iter().any(|x| x == tok) {
                    e.push(tok.to_string());
                }
            }
        };
        for s in &strings {
            if s.kind == StrKind::Identifier {
                add(&s.text, &mut hashes);
            }
            // ⭐ Split on non-identifier bytes and index the pieces too. This is not padding: the
            // client never stores `platform_signin` on its own - it stores
            // `bootflow:platform_signin`, and it is the **component** whose FNV-1 the compiled enum
            // table at `0x274DA80` holds. Without this, the harvester decodes the table's hashes
            // and resolves none of them, which reads exactly like "the oracle doesn't work".
            // Same for `error.content_manifest_not_provided` inside the reason table.
            for tok in s.text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if tok.len() >= 3
                    && tok.as_bytes()[0].is_ascii_alphabetic()
                    && tok != s.text
                {
                    add(tok, &mut hashes);
                }
            }
        }
        Ok(Self { strings, hashes })
    }

    /// Fold extra spellings into the oracle - the demangled RTTI class names, which are real
    /// identifiers the compiler recorded but which never appear as plain strings in that form.
    pub fn add_names<I: IntoIterator<Item = String>>(&mut self, names: I) {
        for n in names {
            for tok in std::iter::once(n.as_str())
                .chain(n.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')))
            {
                if tok.len() < 3 || !tok.as_bytes()[0].is_ascii_alphabetic() {
                    continue;
                }
                for h in [fnv1(tok.as_bytes()), fnv1_lower(tok)] {
                    let e = self.hashes.entry(h).or_default();
                    if !e.iter().any(|x| x == tok) {
                        e.push(tok.to_string());
                    }
                }
            }
        }
    }

    /// Every corpus spelling whose FNV-1 is `h`. Empty is the normal case and is not a defect -
    /// THE-CHAIN §4.2 lists seven names (`patrol`, `arena`, `trials_of_osiris`, …) whose **hash**
    /// is compiled in while the **string** appears nowhere in the image.
    pub fn resolve(&self, h: u32) -> &[String] {
        self.hashes.get(&h).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn identifiers(&self) -> impl Iterator<Item = &Str> {
        self.strings.iter().filter(|s| s.kind == StrKind::Identifier)
    }

    pub fn by_vaddr(&self) -> HashMap<u64, &Str> {
        self.strings.iter().map(|s| (s.vaddr, s)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four names THE-CHAIN §4.1 recovered from the compiled bootflow enum table at
    /// `0x274DA80`. If this test ever fails, someone has "fixed" the hash into FNV-1a again.
    #[test]
    fn fnv1_reproduces_the_compiled_bootflow_table() {
        assert_eq!(fnv1(b"none"), 0x2CA3_3BDB);
        assert_eq!(fnv1(b"platform_signin"), 0xE9D4_6191);
        assert_eq!(fnv1(b"account_signin"), 0xEFA4_CF17);
        assert_eq!(fnv1(b"content_check"), 0x783B_F8AD);
        // and the reason-name table (content-manifest-gate.md §8)
        assert_eq!(fnv1(b"normal_flow"), 0x03CF_BB27);
        assert_eq!(fnv1(b"error.content_manifest_not_provided"), 0x598A_95DC);
        // the empty string is the NULL sentinel seen all over the content tables
        assert_eq!(fnv1(b""), FNV1_BASIS);
    }

    /// FNV-1a is the refuted variant; asserting it is *different* keeps the two from being
    /// silently merged by a future edit (THE-CHAIN §4).
    #[test]
    fn fnv1a_is_a_different_function() {
        fn fnv1a(s: &[u8]) -> u32 {
            let mut h = FNV1_BASIS;
            for &c in s {
                h = (h ^ c as u32).wrapping_mul(FNV1_PRIME);
            }
            h
        }
        assert_ne!(fnv1(b"platform_signin"), fnv1a(b"platform_signin"));
    }
}
