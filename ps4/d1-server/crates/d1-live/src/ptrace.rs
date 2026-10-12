// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The raw `ptrace(2)` surface, and the x86-64 debug registers.
//!
//! Two things here are worth reading before using anything else in this crate.
//!
//! # 1. `PTRACE_SEIZE`, not `PTRACE_ATTACH`
//!
//! `PTRACE_ATTACH` sends `SIGSTOP` and **stops the target on attach**. The operator drives a live
//! shadPS4 client; stopping it for the duration of a watch would change the very timing we are
//! trying to observe, and a crash while stopped would be ours. `PTRACE_SEIZE` attaches **without**
//! stopping, and each stop after that is explicit (`PTRACE_INTERRUPT`) or caused by a real event.
//!
//! # 2. `u_debugreg` is DERIVED, not transcribed
//!
//! `offsetof(struct user, u_debugreg)` is 848 on x86-64 - a number this project's own rules forbid
//! taking on faith (CLAUDE.md: *"derive byte guards from the pinned image, never transcribe
//! them"*). The kernel's `PTRACE_POKEUSER` accepts an offset only when it lands inside
//! `u_debugreg[0..7]`, so the array's bounds are **measurable**: probe downward and upward until
//! the errors start. [`derive_debugreg_offset`] does exactly that against a live tracee and is the
//! positive-plus-negative control for the constant, run every time `controls` runs.
//!
//! Measured on this box (kernel 7.0.12, `struct user` on x86-64):
//!
//! ```text
//! POKEUSER( 840) = EIO      <- u_comm[], the gap: rejected
//! POKEUSER( 848) = ok       <- u_debugreg[0]
//! POKEUSER( 904) = ok       <- u_debugreg[7]
//! POKEUSER( 912) = EIO      <- error_code, past the array: rejected
//! ```
//!
//! (`sizeof(struct user)` is 928, not 912, because x86-64 appends `error_code` and
//! `fault_address` after the array - which is exactly why "sizeof minus 64" would have been the
//! wrong derivation and the boundary probe is the right one.)

use std::collections::BTreeMap;
use std::ffi::c_void;

use anyhow::{bail, Result};

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Requests
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub const PTRACE_PEEKUSER: u32 = 3;
pub const PTRACE_POKEUSER: u32 = 6;
pub const PTRACE_CONT: u32 = 7;
pub const PTRACE_DETACH: u32 = 17;
pub const PTRACE_GETREGS: u32 = 12;
pub const PTRACE_SETOPTIONS: u32 = 0x4200;
pub const PTRACE_SEIZE: u32 = 0x4206;
pub const PTRACE_INTERRUPT: u32 = 0x4207;
pub const PTRACE_LISTEN: u32 = 0x4208;

pub const PTRACE_O_TRACECLONE: u64 = 0x00000008;

pub const PTRACE_EVENT_CLONE: i32 = 3;
pub const PTRACE_EVENT_STOP: i32 = 128;

/// The candidate offsets [`derive_debugreg_offset`] will consider. The true value on x86-64 is
/// 848; the range brackets it generously so a different kernel layout is *discovered*, not
/// silently mis-read.
const DEBUGREG_SEARCH: std::ops::Range<u64> = 512..1400;

/// `offsetof(struct user, u_debugreg)` on x86-64 Linux. Never used without
/// [`derive_debugreg_offset`] having agreed with it at least once - see [`crate::controls`].
pub const DEBUGREG_OFFSET: u64 = 848;

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn clear_errno() {
    // `ptrace(PTRACE_PEEKUSER, …)` returns the read word, so -1 is a legal value; errno is the only
    // discriminator and it must be cleared first. Getting this wrong makes a legitimate `-1` look
    // like a failure and a failure look like data.
    unsafe { *libc::__errno_location() = 0 };
}

fn raw(request: u32, pid: i32, addr: u64, data: u64) -> Result<i64> {
    clear_errno();
    let r = unsafe { libc::ptrace(request as _, pid, addr as *mut c_void, data as *mut c_void) };
    let e = errno();
    if r == -1 && e != 0 {
        bail!("ptrace(request {request:#x}, pid {pid}, addr {addr:#x}) failed: {}", std::io::Error::from_raw_os_error(e));
    }
    Ok(r)
}

pub fn seize(pid: i32, options: u64) -> Result<()> {
    raw(PTRACE_SEIZE, pid, 0, options).map(|_| ())
}

pub fn setoptions(pid: i32, options: u64) -> Result<()> {
    raw(PTRACE_SETOPTIONS, pid, 0, options).map(|_| ())
}

pub fn interrupt(pid: i32) -> Result<()> {
    raw(PTRACE_INTERRUPT, pid, 0, 0).map(|_| ())
}

pub fn cont(pid: i32, signal: i32) -> Result<()> {
    raw(PTRACE_CONT, pid, 0, signal as u64).map(|_| ())
}

pub fn listen(pid: i32) -> Result<()> {
    raw(PTRACE_LISTEN, pid, 0, 0).map(|_| ())
}

pub fn detach(pid: i32) -> Result<()> {
    raw(PTRACE_DETACH, pid, 0, 0).map(|_| ())
}

pub fn peekuser(pid: i32, offset: u64) -> Result<u64> {
    raw(PTRACE_PEEKUSER, pid, offset, 0).map(|v| v as u64)
}

pub fn pokeuser(pid: i32, offset: u64, value: u64) -> Result<()> {
    raw(PTRACE_POKEUSER, pid, offset, value).map(|_| ())
}

/// x86-64 `user_regs_struct`, as 27 words. Index 16 is `rip` (`r15 r14 r13 r12 rbp rbx r11 r10 r9
/// r8 rax rcx rdx rsi rdi orig_rax rip cs eflags rsp ss …`).
pub fn getregs(pid: i32) -> Result<libc::user_regs_struct> {
    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    clear_errno();
    let r = unsafe {
        libc::ptrace(PTRACE_GETREGS as _, pid, std::ptr::null_mut::<c_void>(), &mut regs as *mut _ as *mut c_void)
    };
    let e = errno();
    if r == -1 && e != 0 {
        bail!("PTRACE_GETREGS on {pid} failed: {}", std::io::Error::from_raw_os_error(e));
    }
    Ok(regs)
}

/// Every field of x86-64 `user_regs_struct`, lowercase, in the struct's own declaration order.
///
/// ⚠ This is the WHOLE integer register file, segment selectors and `orig_rax` included, because a
/// consumer that wants `fs_base` should not have to re-run the trap to get it. It is deliberately
/// **not** the 18-name subset `d1-replay`'s `RegDump::render` prints: rendering is a choice, capture
/// is not.
pub const REG_NAMES: [&str; 27] = [
    "r15", "r14", "r13", "r12", "rbp", "rbx", "r11", "r10", "r9", "r8", "rax", "rcx", "rdx", "rsi",
    "rdi", "orig_rax", "rip", "cs", "eflags", "rsp", "ss", "fs_base", "gs_base", "ds", "es", "fs",
    "gs",
];

/// The sixteen general-purpose integer registers - the subset in which a live pointer operand can
/// actually be sitting. A subset of [`REG_NAMES`], asserted as such by a unit test rather than by
/// eye.
pub const GP_REG_NAMES: [&str; 16] = [
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15",
];

/// `PTRACE_GETREGS`'s C struct as a name → value map.
///
/// ⭐ A `BTreeMap`, not a mirrored `user_regs_struct`: **a C struct layout is not a wire format**,
/// and the one consumer that matters - `d1_replay::machine::{RegDump, reg_by_name}` - already speaks
/// exactly these lowercase names, so the replay side needs no conversion and no second rendering.
///
/// ⛔ **No FP/SSE/AVX state.** `PTRACE_GETFPREGS` / `NT_X86_XSTATE` are not called, so a function
/// whose arguments are floats, or whose behaviour depends on `xmm`, CANNOT be replayed from this.
/// That hole is real and cheap to close later; until it is closed the honest phrase is *"the integer
/// register file"*, never *"the registers"*.
pub fn regs_map(r: &libc::user_regs_struct) -> BTreeMap<String, u64> {
    let pairs: [(&str, u64); 27] = [
        ("r15", r.r15),
        ("r14", r.r14),
        ("r13", r.r13),
        ("r12", r.r12),
        ("rbp", r.rbp),
        ("rbx", r.rbx),
        ("r11", r.r11),
        ("r10", r.r10),
        ("r9", r.r9),
        ("r8", r.r8),
        ("rax", r.rax),
        ("rcx", r.rcx),
        ("rdx", r.rdx),
        ("rsi", r.rsi),
        ("rdi", r.rdi),
        ("orig_rax", r.orig_rax),
        ("rip", r.rip),
        ("cs", r.cs),
        ("eflags", r.eflags),
        ("rsp", r.rsp),
        ("ss", r.ss),
        ("fs_base", r.fs_base),
        ("gs_base", r.gs_base),
        ("ds", r.ds),
        ("es", r.es),
        ("fs", r.fs),
        ("gs", r.gs),
    ];
    pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// waitpid
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// `__WALL` - **mandatory** for ptrace: without it `waitpid` ignores non-`SIGCHLD` clone children,
/// which is every thread of a seized multithreaded target. Omitting it produces a wait that never
/// returns and reads as "nothing ever wrote it".
pub const WALL: i32 = 0x40000000;

#[derive(Debug, Clone, Copy)]
pub struct WaitStatus {
    pub pid: i32,
    pub raw: i32,
}

impl WaitStatus {
    pub fn stopped(&self) -> Option<i32> {
        if self.raw & 0xff == 0x7f {
            Some((self.raw >> 8) & 0xff)
        } else {
            None
        }
    }
    pub fn event(&self) -> i32 {
        (self.raw >> 16) & 0xff
    }
    pub fn exited(&self) -> bool {
        self.raw & 0x7f == 0
    }
}

/// Non-blocking reap of any traced task. `None` = nothing to report right now.
pub fn wait_any_nohang() -> Result<Option<WaitStatus>> {
    let mut status: i32 = 0;
    let r = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG | WALL) };
    if r == -1 {
        let e = errno();
        if e == libc::ECHILD {
            return Ok(None);
        }
        bail!("waitpid failed: {}", std::io::Error::from_raw_os_error(e));
    }
    if r == 0 {
        return Ok(None);
    }
    Ok(Some(WaitStatus { pid: r, raw: status }))
}

/// Block until this specific task reports, with a deadline. Used only for the arming handshake.
pub fn wait_for(pid: i32, timeout: std::time::Duration) -> Result<WaitStatus> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let mut status: i32 = 0;
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG | WALL) };
        if r == pid {
            return Ok(WaitStatus { pid, raw: status });
        }
        if r == -1 {
            let e = errno();
            bail!("waitpid({pid}) failed: {}", std::io::Error::from_raw_os_error(e));
        }
        if std::time::Instant::now() >= deadline {
            bail!("timed out waiting for task {pid} to stop");
        }
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Debug registers
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// What the CPU should watch for. The encodings are the Intel SDM's `R/W` field, and the two we
/// cannot express are deliberate: `10b` is I/O breakpoints (needs `CR4.DE`, meaningless here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rw {
    /// `00b` - instruction execution. A **fault**: `RIP` is the instruction itself, and `LEN` must
    /// be 1. This turns the tool into a hardware breakpoint - "did this code ever run?" - with no
    /// byte patching and therefore none of the UD2 tracer's guard failure modes.
    Exec,
    /// `01b` - data **writes** only. A trap: `RIP` is the instruction *after* the store.
    Write,
    /// `11b` - data reads **or** writes. Also a trap.
    ReadWrite,
}

impl Rw {
    pub fn bits(self) -> u64 {
        match self {
            Rw::Exec => 0b00,
            Rw::Write => 0b01,
            Rw::ReadWrite => 0b11,
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "exec" | "x" => Rw::Exec,
            "write" | "w" => Rw::Write,
            "rw" | "readwrite" | "read-write" => Rw::ReadWrite,
            other => bail!("unknown --rw {other:?} - use exec | write | rw"),
        })
    }
    /// A trap fires *after* the instruction; a fault fires *at* it. This is the single fact most
    /// likely to make a reader mis-name the writer, so it is a method, not a comment.
    pub fn rip_is_the_instruction(self) -> bool {
        self == Rw::Exec
    }
}

/// `LEN` field encoding - note that it is **not** monotonic: 4 bytes is `11b` and 8 bytes is `10b`.
/// Writing this by hand from memory is how a watch silently covers the wrong four bytes.
pub fn len_bits(len: u64) -> Result<u64> {
    Ok(match len {
        1 => 0b00,
        2 => 0b01,
        4 => 0b11,
        8 => 0b10,
        other => bail!("--len {other} is not a hardware watchpoint length (1, 2, 4 or 8)"),
    })
}

/// Build a `DR7` value for one slot.
///
/// * `L`/`G` (bits `slot*2`, `slot*2+1`) - local and global enable. Linux arms the slot when
///   *either* is set; both are set so the breakpoint survives a context switch on any kernel.
/// * `LE`/`GE` (bits 8, 9) - "exact data breakpoint match". Ignored by modern CPUs, recommended
///   by the SDM, harmless.
/// * `R/W` (bits `16 + slot*4`), `LEN` (bits `18 + slot*4`).
pub fn dr7_for(slot: usize, rw: Rw, len: u64) -> Result<u64> {
    if slot >= 4 {
        bail!("x86-64 has four debug-address registers; slot {slot} does not exist");
    }
    if rw == Rw::Exec && len != 1 {
        bail!("an execute breakpoint must have --len 1 (the CPU requires LEN=00b for R/W=00b)");
    }
    let lb = len_bits(len)?;
    Ok((1 << (slot * 2))
        | (1 << (slot * 2 + 1))
        | (1 << 8)
        | (1 << 9)
        | (rw.bits() << (16 + slot * 4))
        | (lb << (18 + slot * 4)))
}

/// The CPU requires a data breakpoint to be naturally aligned to its length. An unaligned request
/// is **refused**: silently rounding it down would watch a different four bytes than the caller
/// asked for and report a real, wrong answer.
pub fn check_alignment(addr: u64, len: u64) -> Result<()> {
    if len > 1 && addr % len != 0 {
        bail!(
            "address {addr:#x} is not {len}-byte aligned - x86 data breakpoints must be naturally \
             aligned. Watch {:#x} with --len {len}, or use a smaller --len.",
            addr & !(len - 1)
        );
    }
    Ok(())
}

pub fn dr_offset(base: u64, index: usize) -> u64 {
    base + (index as u64) * 8
}

/// ⭐ The **fingerprint** `PTRACE_POKEUSER` leaves on `u_debugreg`, and the reason this constant is
/// measured rather than trusted.
///
/// `ptrace_set_debugreg()` opens with *"There are no DR4 or DR5 registers"* and returns `EIO` for
/// indices 4 and 5, while every offset between `sizeof(user_regs_struct)` and `u_debugreg[0]`, and
/// everything past `u_debugreg[7]`, is `EIO` too. So the array is not a plain accepting run - it is
/// a hole-punched one, and only one offset in the whole USER area produces this exact pattern:
///
/// ```text
/// X-8   X+0  X+8  X+16 X+24  X+32 X+40  X+48 X+56  X+64
/// EIO   ok   ok   ok   ok    EIO  EIO   ok   ok    EIO
///       DR0  DR1  DR2  DR3   DR4  DR5   DR6  DR7
/// ```
///
/// A contiguous-run derivation would have found "4 words at 848" and refused; a transcribed 848
/// would have been right by luck. The hole is what makes this a *measurement*.
const DEBUGREG_FINGERPRINT: [bool; 8] = [true, true, true, true, false, false, true, true];

/// Measure `offsetof(struct user, u_debugreg)` against a **stopped tracee**.
///
/// Both signs are built in: the word below the array and the word above it must be **rejected**,
/// and the DR4/DR5 hole must be **present**, so a kernel that accepted everything - or nothing - 
/// fails instead of silently agreeing.
///
/// ⚠ Writes zero to every debug register of the tracee while probing (harmless: `DR7 = 0` disables
/// everything). The search deliberately starts well above `sizeof(user_regs_struct)` so it can
/// never land in the general-purpose registers and clobber them. Only ever call this on a process
/// we own - never on the emulator.
pub fn derive_debugreg_offset(pid: i32) -> Result<(u64, String)> {
    let accepts = |off: u64| pokeuser(pid, off, 0).is_ok();
    let mut seen_any = false;

    for off in DEBUGREG_SEARCH.step_by(8) {
        let pattern: [bool; 8] = std::array::from_fn(|i| accepts(off + i as u64 * 8));
        seen_any |= pattern.iter().any(|b| *b);
        if pattern != DEBUGREG_FINGERPRINT {
            continue;
        }
        if accepts(off - 8) {
            bail!(
                "offset {:#x} matched the DR pattern but {:#x} is also writable - the window is \
                 not bounded below, so this is not u_debugreg[0]",
                off,
                off - 8
            );
        }
        if accepts(off + 64) {
            bail!(
                "offset {off:#x} matched the DR pattern but {:#x} is also writable - the window is \
                 not bounded above",
                off + 64
            );
        }
        return Ok((
            off,
            format!(
                "POKEUSER rejects {:#x}; accepts DR0..DR3 at {off:#x}..={:#x}; rejects the DR4/DR5 \
                 hole at {:#x},{:#x}; accepts DR6,DR7 at {:#x},{:#x}; rejects {:#x}",
                off - 8,
                off + 24,
                off + 32,
                off + 40,
                off + 48,
                off + 56,
                off + 64
            ),
        ));
    }

    if !seen_any {
        bail!(
            "PTRACE_POKEUSER accepted no offset in {:#x}..{:#x} - this kernel does not expose \
             u_debugreg through the USER area, so a hardware watchpoint cannot be armed this way",
            DEBUGREG_SEARCH.start,
            DEBUGREG_SEARCH.end
        );
    }
    bail!(
        "PTRACE_POKEUSER accepted some offsets in {:#x}..{:#x}, but none showed the DR0-3 / hole / \
         DR6-7 pattern this kernel's ptrace_set_debugreg() must produce. Refusing to guess.",
        DEBUGREG_SEARCH.start,
        DEBUGREG_SEARCH.end
    )
}
