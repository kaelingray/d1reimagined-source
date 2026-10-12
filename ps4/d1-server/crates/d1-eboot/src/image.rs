// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The eboot image: program segments, the **per-segment** address map, and bounded typed reads.
//!
//! This is ATLAS Layer 1's `image.py`, in Rust. It exists to be the ONE place that knows how a
//! guest virtual address becomes a file offset, because that knowledge is currently re-hardcoded
//! as `BIAS = 0x4000` in a dozen Python scripts.
//!
//! ⚠ **THE ADDRESS BIAS IS PER-SEGMENT** (`historicaldocs/design/ATLAS.md` §4.1). `0x4000` is correct for the two
//! `PT_LOAD` segments of *this* image and is a coincidence of this image, not a property of the
//! format. Measured from the program headers of `patch-01.29-CUSA00219/eboot.bin`:
//!
//! ```text
//! type 0x1        (LOAD)        file 0x00004000  vaddr 0x00000000  delta 0x4000
//! type 0x1        (LOAD)        file 0x018c4000  vaddr 0x018c0000  delta 0x4000
//! type 0x2        (DYNAMIC)     file 0x01bca750  vaddr 0x00000000  delta 0x1bca750
//! type 0x61000000 (dynlibdata)  file 0x01a0d100  vaddr 0x00000000  delta 0x1a0d100
//! type 0x6fffff00               file 0x01bcb4c0  vaddr 0x00000000  delta 0x1bcb4c0
//! type 0x6fffff01               file 0x01bcb530  vaddr 0x00000000  delta 0x1bcb530
//! ```
//!
//! The metadata segments have `p_vaddr = 0` and are addressed by **raw file offset**. Anyone
//! "unifying" the bias into one constant silently corrupts every relocation read - so
//! [`Image::file_offset`] consults `PT_LOAD` only, and metadata segments are reached through
//! [`Image::segment_of_type`] + [`Image::raw`].
//!
//! ⚠ **BSS is not file-backed.** The second `PT_LOAD` has `p_filesz 0x149088` but
//! `p_memsz 0x1690dc0`. A vaddr past `filesz` (e.g. `0x263D0F8`, the `profile_setup` config object)
//! is a *legitimate* runtime address that static reading cannot serve; [`Image::file_offset`]
//! returns [`ImageError::Bss`] rather than a plausible-looking wrong byte.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;

pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_SCE_DYNLIBDATA: u32 = 0x6100_0000;
pub const PT_SCE_RELRO: u32 = 0x6100_0001;
pub const PF_X: u32 = 1;

/// `push rbp; mov rbp,rsp` - the only start a disassembly may be anchored on (ATLAS §4.5).
pub const PROLOGUE: [u8; 4] = [0x55, 0x48, 0x89, 0xE5];

/// The pinned build this project analyses. Everything in `design/` is measured against it.
pub const PINNED_EBOOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../dumps/patch-01.29-CUSA00219/eboot.bin");

/// SHA-256 of the pinned 01.29 eboot - the build-identity gate (ATLAS Layer 1).
/// Re-derived here from the file, and cross-checked against
/// `dumps/patch-01.29-CUSA00219/MANIFEST.sha256` line 1650 (`eboot.bin  29148264`).
pub const PINNED_EBOOT_SHA256: &str =
    "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954";
pub const PINNED_EBOOT_LEN: usize = 29_148_264;

#[derive(Debug)]
pub enum ImageError {
    NotElf64Le,
    Truncated { at: usize, len: usize },
    /// The vaddr lies inside a LOAD segment but past `p_filesz` - i.e. BSS, filled at load time.
    Bss { vaddr: u64, segment: usize },
    /// No LOAD segment covers this vaddr at all.
    Unmapped { vaddr: u64 },
    /// No LOAD segment covers this file offset.
    NotInLoad { offset: usize },
    /// A disassembly was asked to start somewhere that is not `55 48 89 e5` (ATLAS §4.5).
    NotAPrologue { vaddr: u64 },
    Io(std::io::Error),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotElf64Le => write!(f, "expected a 64-bit little-endian ELF"),
            Self::Truncated { at, len } => {
                write!(f, "read of {len} bytes at file offset {at:#x} runs past end of image")
            }
            Self::Bss { vaddr, segment } => write!(
                f,
                "vaddr {vaddr:#x} is BSS in LOAD segment {segment} (past p_filesz) - \
                 not file-backed; static reading stops here"
            ),
            Self::Unmapped { vaddr } => {
                write!(f, "vaddr {vaddr:#x} is not covered by any LOAD segment")
            }
            Self::NotInLoad { offset } => {
                write!(f, "file offset {offset:#x} is not inside a LOAD segment")
            }
            Self::NotAPrologue { vaddr } => write!(
                f,
                "vaddr {vaddr:#x} is not a `55 48 89 e5` prologue - refusing to disassemble from a \
                 guessed start (ATLAS §4.5: twice this project produced authoritative-looking garbage)"
            ),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ImageError {}

impl From<std::io::Error> for ImageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, ImageError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Segment {
    pub index: usize,
    pub p_type: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
}

impl Segment {
    /// `file_offset - vaddr` for this segment. Never assume it is the same for two segments.
    pub fn bias(&self) -> i64 {
        self.offset as i64 - self.vaddr as i64
    }
    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }
    pub fn is_executable(&self) -> bool {
        self.flags & PF_X != 0
    }
    /// vaddr range that is actually backed by bytes in the file.
    pub fn file_backed_vaddrs(&self) -> std::ops::Range<u64> {
        self.vaddr..self.vaddr + self.filesz
    }
    pub fn file_range(&self) -> std::ops::Range<usize> {
        self.offset as usize..(self.offset + self.filesz) as usize
    }
}

pub struct Image {
    path: PathBuf,
    data: Vec<u8>,
    segments: Vec<Segment>,
    /// The two-pass function index, built at most once per image. It lives here so that *"which
    /// function contains this address?"* has exactly one answer for the whole crate - the atlas
    /// asked the index and got FPO functions right while `xref` asked
    /// [`Image::nearest_prologue`] and got a neighbour's name (see [`crate::functions::Containment`]).
    funcs: OnceLock<crate::functions::Index>,
}

impl Image {
    /// Open the pinned eboot, or whatever `D1_EBOOT` names.
    pub fn open_default() -> Result<Self> {
        let path = std::env::var("D1_EBOOT").unwrap_or_else(|_| PINNED_EBOOT.to_string());
        Self::open(path)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = std::fs::read(&path)?;
        Self::from_bytes(path, data)
    }

    pub fn from_bytes(path: PathBuf, data: Vec<u8>) -> Result<Self> {
        if data.len() < 0x40 || &data[..4] != b"\x7fELF" || data[4] != 2 || data[5] != 1 {
            return Err(ImageError::NotElf64Le);
        }
        let phoff = u64::from_le_bytes(data[0x20..0x28].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(data[0x36..0x38].try_into().unwrap()) as usize;
        let phnum = u16::from_le_bytes(data[0x38..0x3A].try_into().unwrap()) as usize;
        let mut segments = Vec::with_capacity(phnum);
        for index in 0..phnum {
            let p = phoff + index * phentsize;
            if p + 56 > data.len() {
                return Err(ImageError::Truncated { at: p, len: 56 });
            }
            let rd_u32 = |o: usize| u32::from_le_bytes(data[p + o..p + o + 4].try_into().unwrap());
            let rd_u64 = |o: usize| u64::from_le_bytes(data[p + o..p + o + 8].try_into().unwrap());
            segments.push(Segment {
                index,
                p_type: rd_u32(0),
                flags: rd_u32(4),
                offset: rd_u64(8),
                vaddr: rd_u64(16),
                filesz: rd_u64(32),
                memsz: rd_u64(40),
                align: rd_u64(48),
            });
        }
        Ok(Self { path, data, segments, funcs: OnceLock::new() })
    }

    /// The two-pass function index - prologue-anchored **and** frame-pointer-omitted functions -
    /// decoded once and cached for the life of this `Image` (~0.2 s for the pinned eboot).
    pub fn functions(&self) -> &crate::functions::Index {
        // `functions::build` only propagates errors from bounded reads it already tolerates, so an
        // empty index here would mean the image has no executable bytes - which `containing`
        // reports as `Containment::None`, i.e. an honest unknown rather than a wrong name.
        self.funcs.get_or_init(|| crate::functions::build(self).unwrap_or_default())
    }

    /// ⭐ **The containing function of `va`** - the question [`Image::nearest_prologue`] does not
    /// answer. Use this for attribution; use `nearest_prologue` only to pick a decode anchor.
    pub fn containing_function(&self, va: u64) -> crate::functions::Containing {
        self.functions().containing(va)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn sha256(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&self.data);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn load_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter().filter(|s| s.is_load() && s.filesz > 0)
    }

    /// LOAD segments with `PF_X`. This is where instructions live and the only place a
    /// call/jmp/rip-relative scan may look.
    pub fn executable_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments
            .iter()
            .filter(|s| s.is_load() && s.is_executable() && s.filesz > 0)
    }

    /// First segment of a given `p_type`. Metadata segments (`PT_DYNAMIC`,
    /// `PT_SCE_DYNLIBDATA`, …) have `p_vaddr == 0` and are addressed by raw file offset.
    pub fn segment_of_type(&self, p_type: u32) -> Option<&Segment> {
        self.segments.iter().find(|s| s.p_type == p_type)
    }

    /// Guest vaddr → file offset, **per segment**. LOAD only; BSS is a distinct error.
    pub fn file_offset(&self, vaddr: u64) -> Result<usize> {
        let mut in_bss = None;
        for s in self.segments.iter().filter(|s| s.is_load()) {
            if vaddr >= s.vaddr && vaddr < s.vaddr + s.filesz {
                return Ok((s.offset + (vaddr - s.vaddr)) as usize);
            }
            if vaddr >= s.vaddr && vaddr < s.vaddr + s.memsz {
                in_bss = Some(s.index);
            }
        }
        match in_bss {
            Some(segment) => Err(ImageError::Bss { vaddr, segment }),
            None => Err(ImageError::Unmapped { vaddr }),
        }
    }

    /// File offset → guest vaddr, LOAD only (the inverse of [`Image::file_offset`]).
    pub fn vaddr(&self, offset: usize) -> Result<u64> {
        for s in self.segments.iter().filter(|s| s.is_load()) {
            let r = s.file_range();
            if r.contains(&offset) {
                return Ok(s.vaddr + (offset as u64 - s.offset));
            }
        }
        Err(ImageError::NotInLoad { offset })
    }

    /// Which LOAD segment covers this vaddr (file-backed part only).
    pub fn segment_for_vaddr(&self, vaddr: u64) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|s| s.is_load() && vaddr >= s.vaddr && vaddr < s.vaddr + s.filesz)
    }

    /// Raw file bytes - the addressing mode for the metadata segments.
    pub fn raw(&self, offset: usize, len: usize) -> Result<&[u8]> {
        self.data
            .get(offset..offset + len)
            .ok_or(ImageError::Truncated { at: offset, len })
    }

    pub fn bytes(&self, vaddr: u64, len: usize) -> Result<&[u8]> {
        let off = self.file_offset(vaddr)?;
        self.raw(off, len)
    }

    pub fn u8(&self, vaddr: u64) -> Result<u8> {
        Ok(self.bytes(vaddr, 1)?[0])
    }
    pub fn u16(&self, vaddr: u64) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(vaddr, 2)?.try_into().unwrap()))
    }
    pub fn u32(&self, vaddr: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(vaddr, 4)?.try_into().unwrap()))
    }
    pub fn u64(&self, vaddr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(vaddr, 8)?.try_into().unwrap()))
    }
    pub fn i32(&self, vaddr: u64) -> Result<i32> {
        Ok(i32::from_le_bytes(self.bytes(vaddr, 4)?.try_into().unwrap()))
    }

    /// NUL-terminated string starting exactly at `vaddr`.
    ///
    /// ⚠ ATLAS §4.2b: *nothing references a string interior.* If a scan for a literal returns
    /// nothing, verify you have the string's **true start** before believing the negative -
    /// [`Image::string_start_at_or_before`] does that.
    pub fn cstr(&self, vaddr: u64, max: usize) -> Result<&str> {
        let off = self.file_offset(vaddr)?;
        let end = (off + max).min(self.data.len());
        let slice = &self.data[off..end];
        let n = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        Ok(std::str::from_utf8(&slice[..n]).unwrap_or(""))
    }

    /// Scan back to the NUL that precedes `vaddr` and return the true start of that string.
    /// Guards ATLAS §4.2b (`0x162E2C0` was the interior of a literal starting at `0x162E2B5`).
    pub fn string_start_at_or_before(&self, vaddr: u64, max_back: usize) -> Result<u64> {
        let off = self.file_offset(vaddr)?;
        let lo = off.saturating_sub(max_back);
        let mut p = off;
        while p > lo {
            if self.data[p - 1] == 0 {
                break;
            }
            p -= 1;
        }
        self.vaddr(p)
    }

    /// Every occurrence of `needle` inside file-backed LOAD segments, as guest vaddrs.
    ///
    /// Deliberately **not** a whole-file `find`: a hit in a metadata segment would be converted
    /// with the text bias and produce a plausible, wrong vaddr (ATLAS §4.1).
    pub fn find_all(&self, needle: &[u8]) -> Vec<u64> {
        let mut out = Vec::new();
        for s in self.load_segments() {
            let r = s.file_range();
            let hay = &self.data[r.start..r.end.min(self.data.len())];
            let mut i = 0usize;
            while i + needle.len() <= hay.len() {
                match hay[i..].windows(needle.len()).position(|w| w == needle) {
                    Some(k) => {
                        let at = i + k;
                        out.push(s.vaddr + at as u64);
                        i = at + 1;
                    }
                    None => break,
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Nearest `push rbp; mov rbp,rsp` at or before `site`, inside the same segment.
    ///
    /// ⚠ ATLAS §4.5: *"twice this project has disassembled from a guessed address and produced
    /// authoritative-looking garbage."* Always locate the real prologue first.
    ///
    /// ⛔ **This is a DECODE ANCHOR, not an attribution.** 11,446 functions have no prologue, so
    /// for a site inside one of them this returns the *previous* function - confidently and
    /// wrongly. Naming the function a site belongs to is [`Image::containing_function`].
    pub fn nearest_prologue(&self, site: u64, max_back: usize) -> Option<u64> {
        let off = self.file_offset(site).ok()?;
        let seg = self.segment_for_vaddr(site)?;
        let lo = (seg.offset as usize).max(off.saturating_sub(max_back));
        let hi = (off + PROLOGUE.len()).min(self.data.len());
        let hay = &self.data[lo..hi];
        let k = hay
            .windows(PROLOGUE.len())
            .rposition(|w| w == PROLOGUE)?;
        self.vaddr(lo + k).ok()
    }
}
