// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The **read-only state plane**: one trait, two implementations, and a hard distinction between
//! *"we did not capture it"* and *"it was zero"*.
//!
//! ⛔ [`MemSource`] has no `write` method and never will. A poke identifies a gate and never
//! satisfies one; everything downstream of one is `synthetic_ancestor` twice over. Making that a
//! property of the *type* is the only version of the rule that survives a tired session.

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use serde::Serialize;

// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum SourceId {
    Live { pid: i32, base: u64 },
    Snapshot { id: String, path: String },
}

impl SourceId {
    pub fn label(&self) -> String {
        match self {
            SourceId::Live { pid, base } => format!("live pid {pid} base {base:#x}"),
            SourceId::Snapshot { id, .. } => format!("snap:{id}"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Mapping {
    pub start: u64,
    pub end: u64,
    pub perms: String,
    pub name: Option<String>,
}

/// ⭐ `Unmapped` is the whole point. It is a **positive statement** - "this range is not in the
/// source" - produced by consulting the extent table *before* reading, never inferred from a
/// read that came back zeros.
#[derive(Clone, Debug, Serialize)]
pub enum ReadErr {
    Unmapped { addr: u64, len: usize },
    Truncated { addr: u64, want: usize, got: usize },
    Io { addr: u64, msg: String },
    NoSource,
    Gone { pid: i32 },
}

impl std::fmt::Display for ReadErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadErr::Unmapped { addr, len } => write!(
                f,
                "UNMAPPED: {addr:#x}..{:#x} is not in this source (⛔ this is NOT a zero)",
                addr + *len as u64
            ),
            ReadErr::Truncated { addr, want, got } => {
                write!(f, "TRUNCATED at {addr:#x}: wanted {want} bytes, got {got}")
            }
            ReadErr::Io { addr, msg } => write!(f, "I/O error at {addr:#x}: {msg}"),
            ReadErr::NoSource => write!(f, "no memory source"),
            ReadErr::Gone { pid } => write!(f, "pid {pid} is gone"),
        }
    }
}

impl std::error::Error for ReadErr {}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// ⛔ **THERE IS NO `write`.**
pub trait MemSource: Send + Sync {
    fn id(&self) -> SourceId;

    /// The **measured** guest base. ⛔ Never assumed: a snapshot carries the base its capture
    /// controls justified, and a live source has to recover it.
    fn base(&self) -> Option<u64>;

    /// Bumps whenever the underlying source could have moved beneath us. A resolution stamped with
    /// an older generation is discarded, never reused.
    fn generation(&self) -> u64;

    /// The one primitive.
    fn read(&self, addr: u64, out: &mut [u8]) -> Result<(), ReadErr>;

    /// The captured/mapped extents - what makes `UNMAPPED` a measurement.
    fn mappings(&self) -> &[Mapping];

    fn read_vec(&self, addr: u64, len: usize) -> Result<Vec<u8>, ReadErr> {
        let mut v = vec![0u8; len];
        self.read(addr, &mut v)?;
        Ok(v)
    }

    fn read_u64(&self, addr: u64) -> Result<u64, ReadErr> {
        let mut b = [0u8; 8];
        self.read(addr, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    fn read_u32(&self, addr: u64) -> Result<u32, ReadErr> {
        let mut b = [0u8; 4];
        self.read(addr, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    /// A NUL-terminated ASCII string, bounded. Returns `None` if the first byte is unmapped.
    fn read_cstr(&self, addr: u64, max: usize) -> Result<String, ReadErr> {
        let mut out = String::new();
        for i in 0..max {
            let mut b = [0u8; 1];
            self.read(addr + i as u64, &mut b)?;
            if b[0] == 0 {
                break;
            }
            out.push(b[0] as char);
        }
        Ok(out)
    }

    /// Is every byte of `addr..addr+len` inside a captured extent?
    fn covers(&self, addr: u64, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let mut cur = addr;
        let end = match addr.checked_add(len as u64) {
            Some(e) => e,
            None => return false,
        };
        let maps = self.mappings();
        while cur < end {
            let Some(m) = maps
                .iter()
                .filter(|m| m.start <= cur && cur < m.end)
                .max_by_key(|m| m.end)
            else {
                return false;
            };
            cur = m.end;
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// SnapMem - a `runs/snapshots/*.snap`, read through d1-replay's own reader.

pub struct SnapMem {
    id: String,
    path: String,
    snap: d1_replay::snapshot::Snapshot,
    maps: Vec<Mapping>,
}

impl SnapMem {
    /// ⛔ Opens **read-only**. This crate never calls `Snapshot::save`, and nothing in
    /// `runs/snapshots/` is ever created, truncated or removed by d1-scope.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let snap = d1_replay::snapshot::Snapshot::load(&path)?;
        let maps = snap
            .manifest
            .regions
            .iter()
            .map(|r| Mapping {
                start: r.addr,
                end: r.addr + r.len,
                perms: r.perms.clone(),
                name: Some(r.origin.clone()),
            })
            .collect();
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".into());
        Ok(Self { id, path: path.display().to_string(), snap, maps })
    }

    /// Resolve a registry snapshot id (`orbit-all`) to its path under `runs/snapshots/`.
    pub fn path_for_id(id: &str) -> PathBuf {
        crate::snapshots_dir().join(format!("{id}.snap"))
    }

    pub fn manifest(&self) -> &d1_replay::snapshot::Manifest {
        &self.snap.manifest
    }

    /// The six known `--preset content` gaps and everything else that was skipped, verbatim.
    pub fn skipped(&self) -> &[d1_replay::snapshot::Skip] {
        &self.snap.manifest.skipped
    }
}

impl MemSource for SnapMem {
    fn id(&self) -> SourceId {
        SourceId::Snapshot { id: self.id.clone(), path: self.path.clone() }
    }

    fn base(&self) -> Option<u64> {
        Some(self.snap.base())
    }

    /// A snapshot is one instant and cannot move beneath us. Constant, by construction.
    fn generation(&self) -> u64 {
        1
    }

    fn read(&self, addr: u64, out: &mut [u8]) -> Result<(), ReadErr> {
        if out.is_empty() {
            return Ok(());
        }
        // ⭐ Coverage FIRST. The extent table is what makes "unmapped" a statement rather than an
        // inference from a read that happened to fail.
        if !self.covers(addr, out.len()) {
            return Err(ReadErr::Unmapped { addr, len: out.len() });
        }
        match self.snap.read(addr, out.len()) {
            Ok(v) if v.len() == out.len() => {
                out.copy_from_slice(&v);
                Ok(())
            }
            Ok(v) => Err(ReadErr::Truncated { addr, want: out.len(), got: v.len() }),
            Err(e) => Err(ReadErr::Io { addr, msg: e.to_string() }),
        }
    }

    fn mappings(&self) -> &[Mapping] {
        &self.maps
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// ProcMem - `/proc/<pid>/mem`, opened O_RDONLY, held.

pub struct ProcMem {
    pid: i32,
    file: File,
    base: Option<u64>,
    generation: u64,
    maps: Vec<Mapping>,
}

impl ProcMem {
    /// ⛔⛔ **THE EMULATOR GUARD.** The operator's client may be live on this box. A tool that
    /// attaches to it because someone forgot a flag is a tool that will one day do it during a run
    /// that mattered - so opening a process whose `comm` looks like shadPS4 is **refused by name**
    /// unless `D1_SCOPE_ALLOW_LIVE=1` is set explicitly.
    ///
    /// ⚠ `pgrep -x shadps4` matches nothing: the comm is `shadPS4:Main`. The check is a
    /// case-insensitive `shadps4` substring, precisely because the obvious form is the wrong one.
    pub fn open(pid: i32) -> anyhow::Result<Self> {
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
        if comm.to_ascii_lowercase().contains("shadps4")
            && std::env::var("D1_SCOPE_ALLOW_LIVE").as_deref() != Ok("1")
        {
            anyhow::bail!(
                "REFUSED: pid {pid} comm {:?} looks like the emulator. d1-scope BUILD 1 does not \
                 read the operator's live client. Set D1_SCOPE_ALLOW_LIVE=1 to mean it.",
                comm.trim()
            );
        }
        // ⭐ `File::open` is O_RDONLY|O_CLOEXEC. `controls::mem_source_is_readonly` re-derives that
        //    from /proc/self/fdinfo rather than trusting this comment.
        let file = File::open(format!("/proc/{pid}/mem"))?;
        let maps = Self::parse_maps(pid)?;
        Ok(Self { pid, file, base: None, generation: 1, maps })
    }

    pub fn with_base(mut self, base: u64) -> Self {
        self.base = Some(base);
        self
    }

    /// The open-file flags as the kernel reports them, for control #4.
    pub fn fd_flags(&self) -> anyhow::Result<u32> {
        let fd = self.file.as_raw_fd();
        let text = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("flags:") {
                return Ok(u32::from_str_radix(rest.trim(), 8)?);
            }
        }
        anyhow::bail!("no `flags:` line in /proc/self/fdinfo/{fd}")
    }

    fn parse_maps(pid: i32) -> anyhow::Result<Vec<Mapping>> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/maps"))?;
        let mut out = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let (Some(range), Some(perms)) = (it.next(), it.next()) else { continue };
            let Some((a, b)) = range.split_once('-') else { continue };
            let (Ok(start), Ok(end)) =
                (u64::from_str_radix(a, 16), u64::from_str_radix(b, 16))
            else {
                continue;
            };
            let name = line.split_whitespace().nth(5).map(|s| s.to_string());
            out.push(Mapping { start, end, perms: perms.to_string(), name });
        }
        Ok(out)
    }
}

impl MemSource for ProcMem {
    fn id(&self) -> SourceId {
        SourceId::Live { pid: self.pid, base: self.base.unwrap_or(0) }
    }

    fn base(&self) -> Option<u64> {
        self.base
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn read(&self, addr: u64, out: &mut [u8]) -> Result<(), ReadErr> {
        if out.is_empty() {
            return Ok(());
        }
        if !self.covers(addr, out.len()) {
            return Err(ReadErr::Unmapped { addr, len: out.len() });
        }
        let fd = self.file.as_raw_fd();
        let mut done = 0usize;
        while done < out.len() {
            let n = unsafe {
                libc::pread(
                    fd,
                    out[done..].as_mut_ptr() as *mut libc::c_void,
                    out.len() - done,
                    (addr + done as u64) as libc::off_t,
                )
            };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                return match e.raw_os_error() {
                    Some(libc::ESRCH) => Err(ReadErr::Gone { pid: self.pid }),
                    _ => Err(ReadErr::Io { addr: addr + done as u64, msg: e.to_string() }),
                };
            }
            if n == 0 {
                return Err(ReadErr::Truncated { addr, want: out.len(), got: done });
            }
            done += n as usize;
        }
        Ok(())
    }

    fn mappings(&self) -> &[Mapping] {
        &self.maps
    }
}
