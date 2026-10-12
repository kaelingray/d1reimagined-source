// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ **Content bytes, in Rust** - the client's own Oodle decoder under Unicorn, plus the block and
//! entry arithmetic that turns a `.pkg` entry into its decompressed payload.
//!
//! # What this replaces, and why it is small
//!
//! `capture-rig/oodle/oodle-decompress.py` looks like a decompressor and is not one. It is a
//! ~35-line wrapper that calls **Destiny's own decoder, lifted from `eboot.bin`**, by address:
//!
//! | address | what it is |
//! |---|---|
//! | [`Q_SCRATCH`] | query the decoder's scratch size |
//! | [`INIT`] | build a decoder over that scratch |
//! | [`DECODE_SOME`] | `OodleLZDecoder_DecodeSome` - one chunk per call |
//!
//! Nothing here implements LZH/LZHLW, in any language. We do not have, read, or want a
//! third-party Oodle implementation; the only decoder involved is the one already inside the media
//! this project owns. `d1-oracle` already ran the client's code under Unicorn
//! ([`crate::harness`]), so the port is *this file*, not a codec.
//!
//! # The three things that make entry extraction non-trivial
//!
//! 1. **Generation chain.** A block record names the generation whose *file* holds its bytes, often
//!    an older package than the one you asked about. [`d1_pkg::Estate::read_block_stored`] already
//!    follows it - and verifies the block SHA-1 while it is there.
//! 2. **Block spanning.** An entry may start inside a block and continue into the next, so the
//!    payload is a concatenation.
//! 3. **Short blocks.** A block decompresses to *at most* `0x40000`. The success condition is
//!    **consuming every stored byte**, not reaching a target size - `ps4_globals_pvp_0169_3`
//!    block 561 consumes all 65,479 stored bytes at 114,688 plain bytes. Looping on
//!    `plain < 0x40000` calls `DecodeSome` once more with no input left, it reads unmapped memory,
//!    and that looks exactly like a decoder fault while being none.
//!
//! # ⛔ Two corrections this port carries, both paid for
//!
//! * **"The Oodle decode is ~30 s under Unicorn"** (`art-arrangement.py:88`, `item-index.py:47`)
//!   is **wrong**, and it is the whole reason this lane looked expensive. Measured: block 1063
//!   decodes in **0.83 s** under the Python and faster here. What actually cost 67 s was
//!   `extract-entry.py`'s `newest_package_for_id`, which launched `d1-pkg info` **once per package
//!   across 1,647 packages**; 61 s of that was *system* time. [`d1_pkg::Estate::load`] does the
//!   same work once, in process. The `/tmp/d1-plain-cache` and `/tmp/d1-def-cache` directories
//!   exist only to paper over that subprocess storm and have **no counterpart here**.
//! * ⭐ **The Rust harness's `memset` guard is stricter than the Python's.**
//!   `OodleLZDecoder_DecodeSome` zeroes a `0x20`-byte frame local and returns to
//!   [`DECODE_SOME_MEMSET_RETURN`]; Python permits any memset inside the frame window, this harness
//!   requires the return site to be registered. Without it the decode faults on the *first* chunk
//!   with `memset would clobber the STACK`, which reads like a client defect and is not one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use d1_pkg::{Block, Entry, Estate, Package};

use crate::error::{OracleError, Result};
use crate::harness::Harness;

/// `OodleLZDecoder_MemorySizeNeeded`-shaped: how much scratch the decoder wants.
pub const Q_SCRATCH: u64 = 0x0126_7120;
/// `OodleLZDecoder_Create`-shaped: build a decoder over caller-supplied scratch.
pub const INIT: u64 = 0x0126_71A0;
/// `OodleLZDecoder_DecodeSome`: one chunk per call, reporting plain/stored consumed.
pub const DECODE_SOME: u64 = 0x0126_72E0;

/// ⭐ The return site of the frame-local `memset` inside `DecodeSome`. See the module note: this
/// address is the difference between a working decode and a guard trip that looks like a fault.
/// Measured 2026-08-11 by running the decode with the guard on and reading the refusal.
pub const DECODE_SOME_MEMSET_RETURN: u64 = 0x0127_0A85;

/// A block decompresses to **at most** this much. Same constant as [`d1_pkg::BLOCK_PLAIN_SIZE`],
/// restated as a `u64` because it is a guest-side argument here.
pub const PLAIN_CAP: u64 = d1_pkg::BLOCK_PLAIN_SIZE as u64;

/// Guest scratch for one stored block and one plain block. A stored block never exceeds
/// [`PLAIN_CAP`] (the compressed flag is only set when compression actually won), so 1 MiB each is
/// four times the worst case and still trivial against `HEAP_SIZE`.
const IO_SPAN: u64 = 0x10_0000;

/// The pinned package estate, same convention as `d1_eboot::image::PINNED_EBOOT`.
/// `$D1_PKGDIR` overrides it.
pub const PINNED_PKGDIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../dumps/patch-01.29-CUSA00219/packages");

/// The default package directory: `$D1_PKGDIR`, else the selected build's packages
/// (`capture-rig/builds.json`). [`PINNED_PKGDIR`] remains the 01.29 reference estate.
pub fn default_pkgdir() -> PathBuf {
    std::env::var_os("D1_PKGDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| d1_build::selected().packages_path())
}

/// One decode's result. `consumed != stored.len()` is a **failure**, surfaced rather than truncated.
#[derive(Debug, Clone)]
pub struct Decoded {
    pub plain: Vec<u8>,
    pub consumed: usize,
    pub chunks: usize,
}

/// The client's own Oodle decoder, held ready.
///
/// One harness serves many blocks. Between decodes the guest heap is rewound to a mark and a
/// **fresh decoder is created**, which is what the Python got by constructing a whole new `Harness`
/// per call - at the cost of re-mapping a 29 MB image every time. [`Oodle::controls`] asserts the
/// reuse is sound by decoding, decoding something else, and decoding the first thing again.
pub struct Oodle {
    h: Harness,
    input: u64,
    output: u64,
    mark: u64,
}

impl Oodle {
    /// Open the pinned eboot and prepare the decoder.
    pub fn open_default() -> Result<Self> {
        Oodle::open_with_memset_allow(true)
    }

    /// ⚠ **Test seam, not a decoding path.** With `allow = false` the frame-local `memset` inside
    /// `DecodeSome` is left refused, so a control can prove [`DECODE_SOME_MEMSET_RETURN`] is
    /// load-bearing rather than decorative. A decode built this way is expected to fail; nothing
    /// it produces may be used.
    pub fn open_with_memset_allow(allow: bool) -> Result<Self> {
        let (_image, mut h) = Harness::open_default()?;
        if allow {
            // ⭐ See DECODE_SOME_MEMSET_RETURN. Registered once, for exactly one site.
            h.allow_stack_memset_from([DECODE_SOME_MEMSET_RETURN]);
        }
        let input = h.alloc_aligned(IO_SPAN, 16)?;
        let output = h.alloc_aligned(IO_SPAN, 16)?;
        let mark = h.heap_mark();
        Ok(Oodle { h, input, output, mark })
    }

    /// Decompress one stored block. Terminates on **input** exhaustion (see the module note).
    pub fn decompress(&mut self, stored: &[u8]) -> Result<Decoded> {
        self.decompress_to(stored, PLAIN_CAP)
    }

    /// [`Oodle::decompress`] with an explicit plain cap, for a caller that knows the block is short.
    pub fn decompress_to(&mut self, stored: &[u8], plain_cap: u64) -> Result<Decoded> {
        if stored.len() as u64 > IO_SPAN {
            return Err(OracleError::Shape(format!(
                "stored block is {} bytes, larger than the {IO_SPAN:#x}-byte guest input window",
                stored.len()
            )));
        }
        if plain_cap > IO_SPAN {
            return Err(OracleError::Shape(format!(
                "plain cap {plain_cap:#x} exceeds the {IO_SPAN:#x}-byte guest output window"
            )));
        }
        // Everything the *decoder* allocates is above the mark; the two I/O windows are below it and
        // survive, which is what makes reuse cheap.
        self.h.heap_rewind(self.mark)?;
        self.h.write(self.input, stored)?;

        let need = self.h.call(Q_SCRATCH, &[0])?;
        let scratch = self.h.alloc_aligned(need, 16)?;
        let decoder = self.h.call(INIT, &[0, plain_cap, scratch])?;
        let result = self.h.alloc_aligned(0x20, 16)?;

        let (mut dec, mut comp, mut chunks) = (0u64, 0u64, 0usize);
        while dec < plain_cap && (comp as usize) < stored.len() {
            self.h.write(result, &[0u8; 0x20])?;
            let ok = self.h.call(
                DECODE_SOME,
                &[
                    decoder,
                    self.output,
                    dec,
                    plain_cap,
                    plain_cap - dec,
                    self.input + comp,
                    stored.len() as u64 - comp,
                    0,
                    0,
                    result,
                ],
            )?;
            let v = self.h.read(result, 0x20)?;
            let n_plain = u32::from_le_bytes(v[0..4].try_into().unwrap()) as u64;
            let n_stored = u32::from_le_bytes(v[4..8].try_into().unwrap()) as u64;
            if ok == 0 || n_plain == 0 {
                break;
            }
            dec += n_plain;
            comp += n_stored;
            chunks += 1;
        }
        let plain = self.h.read(self.output, dec as usize)?;
        Ok(Decoded { plain, consumed: comp as usize, chunks })
    }

    /// The same decode with the Python's refusal attached: a short read is an error, not a truncated
    /// success.
    pub fn decompress_whole(&mut self, stored: &[u8]) -> Result<Decoded> {
        let d = self.decompress(stored)?;
        if d.consumed != stored.len() {
            return Err(OracleError::Shape(format!(
                "decode consumed {}/{} stored bytes - the output is suspect, refusing it \
                 (a block decodes to *at most* {PLAIN_CAP:#x}, so a short PLAIN result is normal; \
                 a short STORED read is not)",
                d.consumed,
                stored.len()
            )));
        }
        Ok(d)
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Content: entry bytes, with the generation chain and block spanning handled
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// How many decompressed blocks to keep. 256 KiB each; the two generators touch well under this.
const BLOCK_CACHE_CAP: usize = 48;

/// The package estate plus the decoder: *"give me the bytes of entry N"*.
pub struct Content {
    estate: Estate,
    oodle: Oodle,
    dir: PathBuf,
    cache: BTreeMap<(usize, u32), Vec<u8>>,
    order: Vec<(usize, u32)>,
}

impl Content {
    /// Open [`default_pkgdir`] and the pinned eboot.
    pub fn open_default() -> Result<Self> {
        Content::open(&default_pkgdir())
    }

    pub fn open(dir: &Path) -> Result<Self> {
        let estate = Estate::load(dir).map_err(|e| {
            OracleError::Shape(format!(
                "{}: {e} (set $D1_PKGDIR, or check the owned-media dump is present)",
                dir.display()
            ))
        })?;
        Ok(Content {
            estate,
            oodle: Oodle::open_default()?,
            dir: dir.to_path_buf(),
            cache: BTreeMap::new(),
            order: Vec::new(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn estate(&self) -> &Estate {
        &self.estate
    }

    /// The decoder, for a caller that has stored bytes of its own.
    pub fn oodle(&mut self) -> &mut Oodle {
        &mut self.oodle
    }

    /// `Package::name` carries the `.pkg` suffix; every doc, TSV header and CLI invocation in this
    /// tree names packages *without* it. Accept both rather than make the caller remember.
    fn package_index(&self, name: &str) -> Result<usize> {
        let want = name.trim_end_matches(".pkg");
        self.estate
            .packages
            .iter()
            .position(|p| p.name.trim_end_matches(".pkg") == want)
            .ok_or_else(|| {
                OracleError::Shape(format!("no package named {name:?} in {}", self.dir.display()))
            })
    }

    /// The **newest** generation of the package with this id at locale 0 - what a content tag names.
    /// Re-derivation of `extract-entry.py::newest_package_for_id`, minus the 1,647 subprocesses.
    pub fn newest_index(&self, pkg_id: u16) -> Result<usize> {
        let p: &Package = self
            .estate
            .newest(pkg_id, 0)
            .ok_or_else(|| OracleError::Shape(format!("no package with id {pkg_id} at locale 0")))?;
        self.package_index(&p.name.clone())
    }

    fn ensure_block(&mut self, pkgi: usize, block_index: u32) -> Result<()> {
        if self.cache.contains_key(&(pkgi, block_index)) {
            return Ok(());
        }
        let pkg = &self.estate.packages[pkgi];
        let block: Block = pkg.block(block_index).ok_or_else(|| {
            OracleError::Shape(format!("{}: no block {block_index}", pkg.name))
        })?;
        let stored = self
            .estate
            .read_block_stored(pkg, &block)
            .map_err(|e| OracleError::Shape(format!("{}: block {block_index}: {e}", pkg.name)))?;
        let plain = if block.compressed() {
            self.oodle.decompress_whole(&stored)?.plain
        } else {
            stored
        };
        if self.order.len() >= BLOCK_CACHE_CAP {
            let old = self.order.remove(0);
            self.cache.remove(&old);
        }
        self.order.push((pkgi, block_index));
        self.cache.insert((pkgi, block_index), plain);
        Ok(())
    }

    /// One block's **stored** bytes and whether they are Oodle-compressed. The block SHA-1 is
    /// verified here, by [`d1_pkg::Estate::read_block_stored`], before anything is decoded.
    pub fn stored_block(&self, pkg_name: &str, block_index: u32) -> Result<(Vec<u8>, bool)> {
        let pkgi = self.package_index(pkg_name)?;
        let pkg = &self.estate.packages[pkgi];
        let block = pkg
            .block(block_index)
            .ok_or_else(|| OracleError::Shape(format!("{}: no block {block_index}", pkg.name)))?;
        let stored = self
            .estate
            .read_block_stored(pkg, &block)
            .map_err(|e| OracleError::Shape(format!("{}: block {block_index}: {e}", pkg.name)))?;
        Ok((stored, block.compressed()))
    }

    /// One block, decompressed. The block SHA-1 is verified on the stored bytes by
    /// [`d1_pkg::Estate::read_block_stored`] before anything is decoded.
    pub fn plain_block(&mut self, pkg_name: &str, block_index: u32) -> Result<&[u8]> {
        let pkgi = self.package_index(pkg_name)?;
        self.ensure_block(pkgi, block_index)?;
        Ok(self.cache.get(&(pkgi, block_index)).expect("just inserted").as_slice())
    }

    /// ⭐ The decompressed payload of one entry, concatenated forward across blocks if it spans.
    /// Returns `(class_tag, bytes)`.
    pub fn entry_bytes(&mut self, pkg_name: &str, index: u32) -> Result<(u32, Vec<u8>)> {
        let pkgi = self.package_index(pkg_name)?;
        self.entry_bytes_at(pkgi, index)
    }

    fn entry_bytes_at(&mut self, pkgi: usize, index: u32) -> Result<(u32, Vec<u8>)> {
        let (name, e): (String, Entry) = {
            let pkg = &self.estate.packages[pkgi];
            let e = pkg
                .entry(index)
                .ok_or_else(|| OracleError::Shape(format!("{}: no entry {index}", pkg.name)))?;
            (pkg.name.clone(), e)
        };
        if !e.has_data() {
            return Err(OracleError::Shape(format!("{name}: entry {index} has no data")));
        }
        let size = e.size() as usize;
        let mut data = Vec::with_capacity(size);
        let (mut b, mut o) = (e.block(), e.block_off() as usize);
        while data.len() < size {
            self.ensure_block(pkgi, b)?;
            let chunk = self.cache.get(&(pkgi, b)).expect("just inserted");
            let take = &chunk[o.min(chunk.len())..(o + (size - data.len())).min(chunk.len())];
            if take.is_empty() {
                return Err(OracleError::Shape(format!(
                    "{name} entry {index}: block {b} gave nothing at offset {o}; short by {}",
                    size - data.len()
                )));
            }
            data.extend_from_slice(take);
            b += 1;
            o = 0;
        }
        Ok((e.class_tag, data))
    }

    /// ⭐ A content **tag** to its bytes: `tag_pkg`/`tag_index` name the newest generation of a
    /// package id at locale 0 and an entry within it. Returns `(package name, entry index,
    /// class tag, bytes)`.
    pub fn entry_bytes_for_tag(&mut self, tag: u32) -> Result<(String, u32, u32, Vec<u8>)> {
        let id = d1_pkg::tag_pkg(tag);
        if id < 0 || id > u16::MAX as i32 {
            return Err(OracleError::Shape(format!("tag {tag:#010x} names package id {id}")));
        }
        let index = d1_pkg::tag_index(tag);
        let pkgi = self.newest_index(id as u16)?;
        let name = self.estate.packages[pkgi].name.clone();
        let (class_tag, data) = self.entry_bytes_at(pkgi, index)?;
        Ok((name, index, class_tag, data))
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

use crate::profile::Control;

/// The pinned decode. Produced by the **Python** path first
/// (`oodle-decompress.py` on `d1-pkg extract-block ps4_investment_globals_012f_3 1063`), so
/// agreement here is a golden diff across two implementations, not a self-check.
pub const PIN_PACKAGE: &str = "ps4_investment_globals_012f_3";
pub const PIN_BLOCK: u32 = 1063;
pub const PIN_BLOCK_PLAIN_LEN: usize = 262_144;
pub const PIN_BLOCK_PLAIN_SHA256: &str =
    "cba12f0a72447742e4da6f5cba01196852b2b10e3efac250a9bbadd741cbd28b";
/// The same block's SHA-1, which is the digest the Python tool prints and every earlier doc quotes.
/// Kept so a reader can match this pin to `oodle-decompress.py`'s own output line without rerunning
/// anything: `b7af28cb…` there and `cba12f0a…` here are the same 262,144 bytes.
pub const PIN_BLOCK_PLAIN_SHA1: &str = "b7af28cbb6410f85ebf1b110e9a58af30594c1f3";
/// The second, independent pin: the activity table's element 1 at `+0x9930`.
pub const PIN_ACTIVITY_1: u32 = 0xDF28_3AFB;

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn no(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: false, detail: detail.into() }
}

/// Controls for the decoder and the entry walk. **Exit 2 if any `pass` is false.**
///
/// One of them is **inverted**: a single flipped stored byte must change the output (or fault). A
/// decoder that ignored its input would pass every positive check here and fail that one.
pub fn controls() -> Vec<Control> {
    let dir = default_pkgdir();
    if !dir.is_dir() {
        return vec![no(
            "estate",
            format!("{} is not a directory - the owned-media dump is absent, so nothing here ran", dir.display()),
        )];
    }
    let mut c = match Content::open(&dir) {
        Ok(c) => c,
        Err(e) => return vec![no("estate", e.to_string())],
    };
    let mut v = Vec::new();

    // 1. THE PIPELINE, against a value the PYTHON path produced. If this fails, nothing below it is
    //    evidence about anything.
    let plain = match c.plain_block(PIN_PACKAGE, PIN_BLOCK) {
        Ok(p) => p.to_vec(),
        Err(e) => {
            v.push(no("oodle-pipeline", format!("{PIN_PACKAGE} block {PIN_BLOCK}: {e}")));
            return v;
        }
    };
    let sha = d1_pkg::manifest::sha256_hex(&plain);
    let act1 = if plain.len() >= 0x9934 {
        u32::from_le_bytes(plain[0x9930..0x9934].try_into().unwrap())
    } else {
        0
    };
    let pipeline_ok =
        plain.len() == PIN_BLOCK_PLAIN_LEN && sha == PIN_BLOCK_PLAIN_SHA256 && act1 == PIN_ACTIVITY_1;
    v.push(if pipeline_ok {
        ok(
            "oodle-pipeline",
            format!(
                "{PIN_PACKAGE} block {PIN_BLOCK} -> {} B, sha256 {}…, activity[1] = {act1:#010x} \
                 (the values the Python path produced)",
                plain.len(),
                &sha[..16]
            ),
        )
    } else {
        no(
            "oodle-pipeline",
            format!(
                "{PIN_PACKAGE} block {PIN_BLOCK} -> {} B sha256 {sha} activity[1] {act1:#010x}; \
                 want {PIN_BLOCK_PLAIN_LEN} / {PIN_BLOCK_PLAIN_SHA256} / {PIN_ACTIVITY_1:#010x}",
                plain.len()
            ),
        )
    });

    // 2. INVERTED. Flip one stored byte in the middle of the block and demand the answer change.
    //    A decode that faults counts as changed - what must not happen is the SAME plain block.
    let inverted = (|| -> Result<String> {
        let (mut stored, compressed) = c.stored_block(PIN_PACKAGE, PIN_BLOCK)?;
        if !compressed {
            return Err(OracleError::Shape(format!(
                "block {PIN_BLOCK} is not Oodle-compressed, so this control tests no decoder"
            )));
        }
        let at = stored.len() / 2;
        stored[at] ^= 0xFF;
        match c.oodle().decompress(&stored) {
            Ok(d) => {
                let s = d1_pkg::manifest::sha256_hex(&d.plain);
                if s == PIN_BLOCK_PLAIN_SHA256 && d.consumed == stored.len() {
                    Err(OracleError::Shape(format!(
                        "a flipped stored byte at +{at:#x} produced the SAME plain block - the \
                         decode is not reading its input"
                    )))
                } else {
                    Ok(format!(
                        "stored[{at:#x}] ^= 0xFF -> {} B, {}/{} consumed, sha256 {}… - changed, as \
                         required",
                        d.plain.len(),
                        d.consumed,
                        stored.len(),
                        &s[..16]
                    ))
                }
            }
            Err(e) => Ok(format!("stored[{at:#x}] ^= 0xFF -> the decode refused ({e}) - changed, as required")),
        }
    })();
    v.push(match inverted {
        Ok(d) => ok("oodle-INVERTED-flipped-input", d),
        Err(e) => no("oodle-INVERTED-flipped-input", e.to_string()),
    });

    // 3. Harness reuse is sound: decode the pin, decode something else, decode the pin again.
    //    The Python built a whole new Harness per call; if that mattered, this is where it shows.
    let reuse = (|| -> Result<String> {
        let other = c.plain_block(PIN_PACKAGE, PIN_BLOCK + 1)?.len();
        let (stored, _) = c.stored_block(PIN_PACKAGE, PIN_BLOCK)?;
        let again = d1_pkg::manifest::sha256_hex(&c.oodle().decompress_whole(&stored)?.plain);
        if again != PIN_BLOCK_PLAIN_SHA256 {
            return Err(OracleError::Shape(format!(
                "re-decoding block {PIN_BLOCK} through the SAME harness gave sha256 {again}, not \
                 the pin - the decoder carries state across decodes and this reuse is unsound"
            )));
        }
        Ok(format!(
            "block {PIN_BLOCK}, then block {} ({other} B), then block {PIN_BLOCK} again: same digest",
            PIN_BLOCK + 1
        ))
    })();
    v.push(match reuse {
        Ok(d) => ok("oodle-harness-reuse", d),
        Err(e) => no("oodle-harness-reuse", e.to_string()),
    });

    // 4. The entry walk: the item table's own entry, self-length checked. `extract-entry.py` prints
    //    this check; here it is enforced.
    let entry = (|| -> Result<String> {
        let (class_tag, data) = c.entry_bytes(PIN_PACKAGE, 8126)?;
        let declared = u64::from_le_bytes(data[0..8].try_into().unwrap()) as usize;
        if class_tag != 0x8080_1933 {
            return Err(OracleError::Shape(format!(
                "entry 8126 is class {class_tag:#010x}, not 0x80801933"
            )));
        }
        if declared != data.len() {
            return Err(OracleError::Shape(format!(
                "entry 8126 self-length header says {declared}, the container says {}",
                data.len()
            )));
        }
        Ok(format!(
            "{PIN_PACKAGE} entry 8126 class {class_tag:#010x}: {} B, self-length header MATCHES",
            data.len()
        ))
    })();
    v.push(match entry {
        Ok(d) => ok("entry-bytes-self-length", d),
        Err(e) => no("entry-bytes-self-length", e.to_string()),
    });

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠ Skips (does not fail) when the owned-media dump is absent, so a clone without it still runs
    /// the suite. It CAN fail: perturb [`DECODE_SOME_MEMSET_RETURN`] and watch the pipeline control
    /// trip on `memset would clobber the STACK`.
    #[test]
    fn the_clients_own_decoder_reproduces_the_python_paths_pinned_block() {
        if !default_pkgdir().is_dir() {
            eprintln!("SKIP: no package estate");
            return;
        }
        for c in controls() {
            assert!(c.pass, "{}: {}", c.name, c.detail);
        }
    }

    /// ⭐ NEGATIVE - [`DECODE_SOME_MEMSET_RETURN`] is load-bearing, not decoration.
    ///
    /// Without that one registered site the decode faults on the *first* chunk with
    /// `memset would clobber the STACK`, which reads exactly like a client defect. This test is the
    /// only record in the tree that the address matters; delete the constant and it goes red.
    #[test]
    fn without_the_memset_allow_site_the_decode_is_refused() {
        if !default_pkgdir().is_dir() {
            eprintln!("SKIP: no package estate");
            return;
        }
        let c = Content::open_default().expect("estate");
        let (stored, compressed) = c.stored_block(PIN_PACKAGE, PIN_BLOCK).expect("block");
        assert!(compressed, "the pinned block must be compressed for this to test anything");
        drop(c);

        let mut bare = Oodle::open_with_memset_allow(false).expect("harness");
        match bare.decompress(&stored) {
            Err(e) => {
                let m = e.to_string();
                assert!(
                    m.contains("memset"),
                    "expected the memset guard to trip, got a different failure: {m}"
                );
            }
            Ok(d) => panic!(
                "the decode SUCCEEDED with the memset-allow site unregistered ({} plain bytes) - \
                 DECODE_SOME_MEMSET_RETURN no longer describes anything and should be deleted",
                d.plain.len()
            ),
        }
    }
}
