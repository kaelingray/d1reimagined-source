// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The virtual gamepad - `capture-rig/virtual-pad.py`, ported behaviour for behaviour.
//!
//! ## Why a kernel device at all
//!
//! The previous autonomous-input attempt used `xdotool key` against Xvfb and produced **zero**
//! SignOn requests: Xvfb has no window manager, so there is no `_NET_ACTIVE_WINDOW` and the key
//! has nothing to be delivered to. Wayland has no unprivileged key-injection path at all.
//! `/dev/uinput` sidesteps the display server entirely: it creates a **real evdev device in the
//! kernel**. SDL enumerates it like any other pad, so it works on Wayland, on X11 and headless -
//! and it is readable from inside the emulator's network namespace, because device nodes are not
//! netns-scoped.
//!
//! ## Two ordering constraints, both load-bearing
//!
//! 1. **Create the pad BEFORE launching the emulator.** udev hotplug arrives over
//!    `NETLINK_KOBJECT_UEVENT`, and netlink *is* network-namespaced - a pad created after the
//!    emulator is inside `unshare -rn` is never announced to it. Initial enumeration reads sysfs
//!    (not namespaced) and does find a pre-existing device. `run` prints this note at startup and
//!    escalates it to a WARNING when an emulator is already up.
//! 2. **The process must stay alive.** The device disappears when the fd closes. Hence the FIFO
//!    control interface rather than a fire-and-forget invocation.
//!
//! ## THE PAD LAW
//!
//! An agent-driven emulator must NOT claim a physical pad belonging to the operator. `print-ignore`
//! emits the exact `SDL_GAMECONTROLLER_IGNORE_DEVICES` value to use, built from the **real**
//! VID/PIDs in `/proc/bus/input/devices` - the `0xFFFF/0xFFFF` convention is a no-op because it is
//! a VID/PID match list and `0xFFFF` matches no real device. With `OPERATOR_PAD=1` the launcher
//! skips the ignore list entirely and the virtual pad is not required; do not drive both input
//! sources at one menu.
//!
//! ## Identity
//!
//! The default identity reports as an Xbox 360 pad (`045e:028e`, bus USB, version `0x0110`)
//! because SDL ships a built-in mapping for that GUID; no `SDL_GAMECONTROLLERCONFIG` needed.
//! `--vendor`/`--product` exist so two simultaneous pads can present distinct ids (the
//! two-instance harness blocker: `SDL_GAMECONTROLLER_IGNORE_DEVICES` matches on VID/PID alone,
//! so two pads with one id are ONE device to SDL). ⚠ A non-default id changes the SDL joystick
//! GUID, so the built-in x360 mapping no longer applies - whether shadPS4 still binds such a pad
//! is `[U]`, untested against a live client.
//!
//! ## The FIFO vocabulary is a contract
//!
//! `press <btn>` | `tap <btn> [n] [gap]` | `hold <btn> [secs]` | `axis <name> <value> [secs]` |
//! `sleep <secs>` | `quit`/`exit`. Scripts (`capture-rig/autodrive.sh`) and docs write these
//! bytes; the reply strings on stdout are preserved verbatim from the Python. One deliberate
//! divergence: a malformed number (`tap a x`) crashed the Python process - and with it the
//! device, mid-run; here it is refused as `unknown command` and the pad survives.
//!
//! ⚠ `bootflow:profile_setup`'s BRIGHTNESS screen (ui stage ENUM(21)) ignores every DIGITAL
//! input - cross, circle, square, triangle, start, L1, R1 and all four d-pad directions, measured
//! one press at a time across two runs. If a screen swallows every button, reach for `axis`
//! before concluding the pad is broken; `press` alone cannot express a slider.
//!
//! On a DualShock layout shadPS4 maps SDL south (`BTN_A`) to **cross**, the "press X to play"
//! button Destiny wants.

use std::ffi::CString;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::controls::{check, skipped, Control, State};

// ── uinput / evdev constants (linux/uinput.h, linux/input-event-codes.h) ─────────────────────
// _IOW('U', n, int) = dir(write)<<30 | size(4)<<16 | 'U'<<8 | n
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564; // _IOW('U', 100, int)
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565; // _IOW('U', 101, int)
const UI_SET_ABSBIT: libc::c_ulong = 0x4004_5567; // _IOW('U', 103, int)
const UI_DEV_CREATE: libc::c_ulong = 0x5501; // _IO('U', 1)
const UI_DEV_DESTROY: libc::c_ulong = 0x5502; // _IO('U', 2)

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const SYN_REPORT: u16 = 0;
pub const BUS_USB: u16 = 0x03;

const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_Z: u16 = 2;
const ABS_RX: u16 = 3;
const ABS_RY: u16 = 4;
const ABS_RZ: u16 = 5;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

/// Xbox 360 pad button layout - SDL's built-in mapping keys off the resulting GUID.
pub const BTN: &[(&str, u16)] = &[
    ("a", 0x130),
    ("b", 0x131),
    ("x", 0x133),
    ("y", 0x134),
    ("lb", 0x136),
    ("rb", 0x137),
    ("back", 0x13A),
    ("start", 0x13B),
    ("guide", 0x13C),
    ("ls", 0x13D),
    ("rs", 0x13E),
];

/// Friendly aliases. Destiny is a PS4 title, so the operator thinks in DualShock names.
pub const ALIAS: &[(&str, &str)] = &[
    ("cross", "a"),
    ("circle", "b"),
    ("square", "x"),
    ("triangle", "y"),
    ("l1", "lb"),
    ("r1", "rb"),
    ("options", "start"),
    ("share", "back"),
    ("ps", "guide"),
    ("l3", "ls"),
    ("r3", "rs"),
    ("enter", "a"),
    ("accept", "a"),
    ("cancel", "b"),
];

/// (code, min, max) - sticks are signed 16-bit, triggers 0..255, hats -1..1.
pub const AXES: &[(u16, i32, i32)] = &[
    (ABS_X, -32768, 32767),
    (ABS_Y, -32768, 32767),
    (ABS_RX, -32768, 32767),
    (ABS_RY, -32768, 32767),
    (ABS_Z, 0, 255),
    (ABS_RZ, 0, 255),
    (ABS_HAT0X, -1, 1),
    (ABS_HAT0Y, -1, 1),
];

pub const HAT: &[(&str, (u16, i32))] = &[
    ("up", (ABS_HAT0Y, -1)),
    ("down", (ABS_HAT0Y, 1)),
    ("left", (ABS_HAT0X, -1)),
    ("right", (ABS_HAT0X, 1)),
];

/// Named ANALOG axes, for screens the d-pad cannot drive (the brightness slider).
pub const STICK: &[(&str, u16)] = &[
    ("lx", ABS_X),
    ("ly", ABS_Y),
    ("rx", ABS_RX),
    ("ry", ABS_RY),
    ("l2", ABS_Z),
    ("r2", ABS_RZ),
];

// ── identity and the uinput setup payload ────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct PadIdentity {
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

impl Default for PadIdentity {
    fn default() -> Self {
        PadIdentity {
            name: "D1 Preservation Virtual Pad".into(),
            vendor: 0x045E,  // the classic x360 GUID SDL already knows
            product: 0x028E,
            version: 0x0110,
        }
    }
}

/// `0x045E`, `045E` or `045e` -> `u16`. Hex only, like the ids in `/proc/bus/input/devices`.
pub fn parse_hex_u16(s: &str) -> Option<u16> {
    let t = s.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    u16::from_str_radix(t, 16).ok()
}

/// `struct uinput_user_dev`: `char name[80]; struct input_id id; __u32 ff_effects_max;`
/// `__s32 absmax[64], absmin[64], absfuzz[64], absflat[64];` - 1116 bytes, native layout,
/// byte-identical to what the Python packs with `struct.pack("80sHHHHI" + "64i" * 4, ...)`.
pub const SETUP_PAYLOAD_LEN: usize = 1116;

pub fn setup_payload(id: &PadIdentity) -> Vec<u8> {
    let mut out = vec![0u8; SETUP_PAYLOAD_LEN];
    let name = id.name.as_bytes();
    let n = name.len().min(79); // the Python truncates with [:79]
    out[..n].copy_from_slice(&name[..n]);
    out[80..82].copy_from_slice(&BUS_USB.to_ne_bytes());
    out[82..84].copy_from_slice(&id.vendor.to_ne_bytes());
    out[84..86].copy_from_slice(&id.product.to_ne_bytes());
    out[86..88].copy_from_slice(&id.version.to_ne_bytes());
    // ff_effects_max at 88..92 stays 0; absmax at 92, absmin at 348, fuzz/flat stay 0.
    for &(code, lo, hi) in AXES {
        let c = code as usize;
        out[92 + c * 4..92 + c * 4 + 4].copy_from_slice(&hi.to_ne_bytes());
        out[348 + c * 4..348 + c * 4 + 4].copy_from_slice(&lo.to_ne_bytes());
    }
    out
}

/// `struct input_event { struct timeval time; __u16 type; __u16 code; __s32 value; }` - 24 bytes
/// on x86_64, time zeroed exactly as the Python does.
pub fn encode_event(etype: u16, code: u16, value: i32) -> [u8; 24] {
    let mut b = [0u8; 24];
    b[16..18].copy_from_slice(&etype.to_ne_bytes());
    b[18..20].copy_from_slice(&code.to_ne_bytes());
    b[20..24].copy_from_slice(&value.to_ne_bytes());
    b
}

/// The Python's axis arithmetic, verbatim: `int(hi * value)` for a non-negative deflection,
/// `int(-lo * value)` for a negative one (both truncate toward zero), clamped to `[lo, hi]`.
/// The re-centre value is `0` for a signed axis and `lo` for an unsigned one.
pub fn axis_value(code: u16, value: f64) -> Option<(i32, i32)> {
    let &(_, lo, hi) = AXES.iter().find(|&&(c, _, _)| c == code)?;
    let v = if value >= 0.0 { (hi as f64 * value) as i64 } else { (-(lo as f64) * value) as i64 };
    let v = v.clamp(lo as i64, hi as i64) as i32;
    Some((v, if lo < 0 { 0 } else { lo }))
}

// ── the device ───────────────────────────────────────────────────────────────────────────────

fn ioctl_val(fd: RawFd, req: libc::c_ulong, val: libc::c_int) -> io::Result<()> {
    if unsafe { libc::ioctl(fd, req, val) } < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

fn ioctl_none(fd: RawFd, req: libc::c_ulong) -> io::Result<()> {
    if unsafe { libc::ioctl(fd, req) } < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

fn write_all_fd(fd: RawFd, buf: &[u8]) -> io::Result<()> {
    let mut off = 0;
    while off < buf.len() {
        let n = unsafe {
            libc::write(fd, buf[off..].as_ptr() as *const libc::c_void, buf.len() - off)
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        off += n as usize;
    }
    Ok(())
}

pub struct VirtualPad {
    fd: RawFd,
    pub identity: PadIdentity,
}

impl VirtualPad {
    /// Create the kernel device and centre every axis. An uninitialised pad can read as a held
    /// stick, which in a menu shows up as phantom cursor drift and looks like the game ignoring us.
    pub fn create(id: &PadIdentity) -> io::Result<VirtualPad> {
        let path = CString::new("/dev/uinput").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            let e = io::Error::last_os_error();
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "/dev/uinput: {e} (this box grants access via a POSIX ACL, not a group - \
                     check `getfacl /dev/uinput`)"
                ),
            ));
        }
        let pad = VirtualPad { fd, identity: id.clone() };
        for ev in [EV_KEY, EV_ABS, EV_SYN] {
            ioctl_val(fd, UI_SET_EVBIT, ev as libc::c_int)?;
        }
        for &(_, code) in BTN {
            ioctl_val(fd, UI_SET_KEYBIT, code as libc::c_int)?;
        }
        for &(code, _, _) in AXES {
            ioctl_val(fd, UI_SET_ABSBIT, code as libc::c_int)?;
        }
        write_all_fd(fd, &setup_payload(id))?;
        ioctl_none(fd, UI_DEV_CREATE)?;
        std::thread::sleep(Duration::from_millis(300));
        pad.centre_all()?;
        Ok(pad)
    }

    fn emit(&self, etype: u16, code: u16, value: i32) -> io::Result<()> {
        write_all_fd(self.fd, &encode_event(etype, code, value))
    }

    fn syn(&self) -> io::Result<()> {
        self.emit(EV_SYN, SYN_REPORT, 0)
    }

    pub fn centre_all(&self) -> io::Result<()> {
        for &(code, lo, _) in AXES {
            self.emit(EV_ABS, code, if lo < 0 { 0 } else { lo })?;
        }
        self.syn()
    }

    /// Press (or, for a hat direction, deflect) for `hold`, then release. `Ok(false)` = unknown
    /// button, nothing emitted. `name` must already be lowercased, as the FIFO loop does.
    pub fn press(&self, name: &str, hold: Duration) -> io::Result<bool> {
        let name = ALIAS.iter().find(|&&(k, _)| k == name).map(|&(_, v)| v).unwrap_or(name);
        if let Some(&(_, (code, val))) = HAT.iter().find(|&&(k, _)| k == name) {
            self.emit(EV_ABS, code, val)?;
            self.syn()?;
            std::thread::sleep(hold);
            self.emit(EV_ABS, code, 0)?;
            self.syn()?;
            return Ok(true);
        }
        let Some(&(_, code)) = BTN.iter().find(|&&(k, _)| k == name) else {
            return Ok(false);
        };
        self.emit(EV_KEY, code, 1)?;
        self.syn()?;
        std::thread::sleep(hold);
        self.emit(EV_KEY, code, 0)?;
        self.syn()?;
        Ok(true)
    }

    /// Deflect an analog axis to `value` (-1.0..1.0, or 0..1 for triggers) then re-centre.
    pub fn axis(&self, name: &str, value: f64, secs: Duration) -> io::Result<bool> {
        let Some(&(_, code)) = STICK.iter().find(|&&(k, _)| k == name) else {
            return Ok(false);
        };
        let (v, centre) = axis_value(code, value).expect("every STICK code is in AXES");
        self.emit(EV_ABS, code, v)?;
        self.syn()?;
        std::thread::sleep(secs);
        self.emit(EV_ABS, code, centre)?;
        self.syn()?;
        Ok(true)
    }
}

impl Drop for VirtualPad {
    fn drop(&mut self) {
        let _ = ioctl_none(self.fd, UI_DEV_DESTROY);
        unsafe { libc::close(self.fd) };
    }
}

// ── the FIFO vocabulary ──────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum Parsed<'a> {
    Empty,
    Quit,
    Press { btn: &'a str },
    Tap { btn: &'a str, n: i64, gap: f64 },
    Hold { btn: &'a str, secs: f64 },
    Axis { axis: &'a str, value: f64, value_raw: &'a str, secs: f64 },
    Sleep { secs: f64 },
    Unknown,
}

/// The Python's command dispatch, including its defaults (tap: n=1 gap=1.0; hold: 1.0 s;
/// axis: 0.35 s). A malformed number parses to `Unknown` where the Python raised and died.
pub fn parse_line(line: &str) -> Parsed<'_> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Parsed::Empty;
    }
    let cmd = parts[0].to_lowercase();
    let int_at = |i: usize, default: i64| -> Option<i64> {
        match parts.get(i) {
            Some(s) => s.parse().ok(),
            None => Some(default),
        }
    };
    let float_at = |i: usize, default: f64| -> Option<f64> {
        match parts.get(i) {
            Some(s) => s.parse().ok(),
            None => Some(default),
        }
    };
    match cmd.as_str() {
        "quit" | "exit" => Parsed::Quit,
        "press" if parts.len() >= 2 => Parsed::Press { btn: parts[1] },
        "tap" if parts.len() >= 2 => match (int_at(2, 1), float_at(3, 1.0)) {
            (Some(n), Some(gap)) => Parsed::Tap { btn: parts[1], n, gap },
            _ => Parsed::Unknown,
        },
        "hold" if parts.len() >= 2 => match float_at(2, 1.0) {
            Some(secs) => Parsed::Hold { btn: parts[1], secs },
            None => Parsed::Unknown,
        },
        "axis" if parts.len() >= 3 => match (parts[2].parse::<f64>().ok(), float_at(3, 0.35)) {
            (Some(value), Some(secs)) => {
                Parsed::Axis { axis: parts[1], value, value_raw: parts[2], secs }
            }
            _ => Parsed::Unknown,
        },
        "sleep" if parts.len() >= 2 => match float_at(1, 0.0) {
            Some(secs) => Parsed::Sleep { secs },
            None => Parsed::Unknown,
        },
        _ => Parsed::Unknown,
    }
}

/// Python `str(float)` for the reply strings: shortest round-trip, but integral values keep a
/// trailing `.0` (`str(1.0) == "1.0"` where Rust's `{}` prints `1`).
pub fn fmt_f(x: f64) -> String {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 1e16 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

fn unknown_axis_msg() -> String {
    let mut names: Vec<&str> = STICK.iter().map(|&(k, _)| k).collect();
    names.sort_unstable();
    let list = names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
    format!("UNKNOWN AXIS (have [{list}])")
}

fn secs_dur(s: f64) -> Duration {
    Duration::from_secs_f64(s.max(0.0))
}

/// Execute one FIFO line. Reply strings are the Python's, byte for byte (the button token is
/// echoed in its original case; the lookup is case-insensitive). Returns `Ok(true)` on `quit`.
pub fn handle_line(pad: &VirtualPad, line: &str, out: &mut dyn Write) -> io::Result<bool> {
    match parse_line(line) {
        Parsed::Empty => {}
        Parsed::Quit => return Ok(true),
        Parsed::Press { btn } => {
            let ok = pad.press(&btn.to_lowercase(), Duration::from_millis(80))?;
            writeln!(out, "press {btn}: {}", if ok { "ok" } else { "UNKNOWN BUTTON" })?;
        }
        Parsed::Tap { btn, n, gap } => {
            // The Python ignores the per-press verdict here; preserved.
            for i in 0..n.max(0) {
                pad.press(&btn.to_lowercase(), Duration::from_millis(80))?;
                writeln!(out, "tap {btn} {}/{n}", i + 1)?;
                out.flush()?;
                if i + 1 < n {
                    std::thread::sleep(secs_dur(gap));
                }
            }
        }
        Parsed::Hold { btn, secs } => {
            let ok = pad.press(&btn.to_lowercase(), secs_dur(secs))?;
            writeln!(
                out,
                "hold {btn} {}s: {}",
                fmt_f(secs),
                if ok { "ok" } else { "UNKNOWN BUTTON" }
            )?;
        }
        Parsed::Axis { axis, value, value_raw, secs } => {
            let ok = pad.axis(&axis.to_lowercase(), value, secs_dur(secs))?;
            let verdict = if ok { "ok".to_string() } else { unknown_axis_msg() };
            writeln!(out, "axis {axis} {value_raw} {}s: {verdict}", fmt_f(secs))?;
        }
        Parsed::Sleep { secs } => std::thread::sleep(secs_dur(secs)),
        Parsed::Unknown => writeln!(out, "unknown command: {}", line.trim())?,
    }
    out.flush()?;
    Ok(false)
}

/// Create the control FIFO and serve commands until `quit`, a signal, or an error. The Python
/// re-opened the FIFO per EOF (a FIFO read returns EOF each time the last writer closes); here
/// one non-blocking handle is polled, which has the same semantics and stays interruptible.
pub fn run_fifo(pad: &VirtualPad, fifo: &Path, out: &mut dyn Write, stop: &AtomicBool) -> io::Result<()> {
    if fifo.exists() {
        std::fs::remove_file(fifo)?;
    }
    let cpath = CString::new(fifo.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "fifo path contains NUL"))?;
    if unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) } < 0 {
        return Err(io::Error::last_os_error());
    }
    writeln!(
        out,
        "control fifo: {}  (commands: press <btn> | tap <btn> <n> <gap> | quit)",
        fifo.display()
    )?;
    out.flush()?;

    let result = (|| -> io::Result<()> {
        let f = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(fifo)?;
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            if stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            let read = (&f).read(&mut chunk);
            match read {
                Ok(0) => {
                    // No writer (or writer closed). Flush a trailing unterminated line, then idle.
                    if !pending.is_empty() {
                        let line = String::from_utf8_lossy(&pending).to_string();
                        pending.clear();
                        if handle_line(pad, &line, out)? {
                            return Ok(());
                        }
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(n) => {
                    pending.extend_from_slice(&chunk[..n]);
                    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                        let raw: Vec<u8> = pending.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&raw[..raw.len() - 1]).to_string();
                        if handle_line(pad, &line, out)? {
                            return Ok(());
                        }
                    }
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::Interrupted =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(e),
            }
        }
    })();
    let _ = std::fs::remove_file(fifo);
    result
}

// ── THE PAD LAW: the ignore list ─────────────────────────────────────────────────────────────

/// The `SDL_GAMECONTROLLER_IGNORE_DEVICES` value covering every *physical* input device: each
/// `/proc/bus/input/devices` entry with a `js` handler, minus our own identity so the virtual pad
/// stays visible. Duplicates collapse, first occurrence wins - exactly the Python's behaviour.
/// (Divergence, malformed input only: an `I:` line with no Vendor/Product is skipped, where the
/// Python would have emitted the literal `0xNone/0xNone`.)
pub fn ignore_list_from(text: &str, self_vendor: u16, self_product: u16) -> String {
    let sv = format!("{self_vendor:04x}");
    let sp = format!("{self_product:04x}");
    let mut out: Vec<String> = Vec::new();
    let mut cur: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("I:") {
            let mut vendor = None;
            let mut product = None;
            for tok in rest.split_whitespace() {
                if let Some((k, v)) = tok.split_once('=') {
                    match k {
                        "Vendor" => vendor = Some(v.to_string()),
                        "Product" => product = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
            cur = vendor.zip(product);
        } else if line.starts_with("H: Handlers=") {
            if let Some((v, p)) = cur.take() {
                if line.contains("js") && !(v == sv && p == sp) {
                    let item = format!("0x{v}/0x{p}");
                    if !out.contains(&item) {
                        out.push(item);
                    }
                }
            }
        }
    }
    out.join(",")
}

/// Live ignore list. An unreadable `/proc/bus/input/devices` yields an empty string, as in the
/// Python (its `except OSError: pass`).
pub fn ignore_list_live(self_vendor: u16, self_product: u16) -> String {
    std::fs::read_to_string("/proc/bus/input/devices")
        .map(|t| ignore_list_from(&t, self_vendor, self_product))
        .unwrap_or_default()
}

// ── /proc/bus/input/devices, parsed for read-back controls ──────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputDev {
    pub name: String,
    pub sysfs: String,
    pub bus: String,
    pub vendor: String,
    pub product: String,
    pub version: String,
    pub handlers: Vec<String>,
    /// `B:` capability bitmaps, e.g. `("KEY", "7cdb000000000000 0 0 0 0")`.
    pub bits: Vec<(String, String)>,
}

pub fn parse_input_devices(text: &str) -> Vec<InputDev> {
    let mut out = Vec::new();
    for stanza in text.split("\n\n") {
        let mut d = InputDev::default();
        let mut saw_i = false;
        for line in stanza.lines() {
            if let Some(rest) = line.strip_prefix("I:") {
                saw_i = true;
                for tok in rest.split_whitespace() {
                    if let Some((k, v)) = tok.split_once('=') {
                        match k {
                            "Bus" => d.bus = v.into(),
                            "Vendor" => d.vendor = v.into(),
                            "Product" => d.product = v.into(),
                            "Version" => d.version = v.into(),
                            _ => {}
                        }
                    }
                }
            } else if let Some(rest) = line.strip_prefix("N: Name=") {
                d.name = rest.trim().trim_matches('"').to_string();
            } else if let Some(rest) = line.strip_prefix("S: Sysfs=") {
                d.sysfs = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                d.handlers = rest.split_whitespace().map(str::to_string).collect();
            } else if let Some(rest) = line.strip_prefix("B: ") {
                if let Some((k, v)) = rest.split_once('=') {
                    d.bits.push((k.to_string(), v.trim().to_string()));
                }
            }
        }
        if saw_i {
            out.push(d);
        }
    }
    out
}

fn live_input_devices() -> Vec<InputDev> {
    std::fs::read_to_string("/proc/bus/input/devices")
        .map(|t| parse_input_devices(&t))
        .unwrap_or_default()
}

fn find_by_name(name: &str) -> Vec<InputDev> {
    live_input_devices().into_iter().filter(|d| d.name == name).collect()
}

/// The kernel's `B:` format for a bit set: 64-bit words, highest first, `%lx` each.
pub fn bitmap_words(bits: &[u16]) -> String {
    let words = bits.iter().map(|&b| b as usize / 64).max().unwrap_or(0) + 1;
    let mut w = vec![0u64; words];
    for &b in bits {
        w[b as usize / 64] |= 1 << (b % 64);
    }
    w.iter().rev().map(|x| format!("{x:x}")).collect::<Vec<_>>().join(" ")
}

pub fn expected_key_bitmap() -> String {
    bitmap_words(&BTN.iter().map(|&(_, c)| c).collect::<Vec<_>>())
}

pub fn expected_abs_bitmap() -> String {
    bitmap_words(&AXES.iter().map(|&(c, _, _)| c).collect::<Vec<_>>())
}

// ── the netns ordering constraint, as a loud warning ────────────────────────────────────────

/// Pids of live emulator processes, by the launchers' own predicate (`shadps4` plus a `-p` arg;
/// `pgrep -x` misses it because the process renames itself `shadPS4:Main`).
pub fn live_emulators() -> Vec<i32> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for e in entries.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let args: Vec<String> =
            raw.split(|&b| b == 0).map(|s| String::from_utf8_lossy(s).to_string()).collect();
        if args.iter().any(|a| a.contains("shadps4")) && args.iter().any(|a| a == "-p") {
            out.push(pid);
        }
    }
    out
}

// ── the CLI entry points ─────────────────────────────────────────────────────────────────────

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_signal as *const () as usize;
        sa.sa_flags = 0; // no SA_RESTART: blocking calls must EINTR so the loop can clean up
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
    }
}

fn parse_ids(vendor: &str, product: &str, version: &str) -> Result<(u16, u16, u16), String> {
    let v = parse_hex_u16(vendor).ok_or(format!("--vendor {vendor:?} is not 16-bit hex"))?;
    let p = parse_hex_u16(product).ok_or(format!("--product {product:?} is not 16-bit hex"))?;
    let ver = parse_hex_u16(version).ok_or(format!("--version {version:?} is not 16-bit hex"))?;
    Ok((v, p, ver))
}

pub fn print_ignore_cli(vendor: &str, product: &str) -> i32 {
    match parse_ids(vendor, product, "0x0110") {
        Err(e) => {
            eprintln!("REFUSED: {e}");
            2
        }
        Ok((v, p, _)) => {
            println!("{}", ignore_list_live(v, p));
            0
        }
    }
}

pub fn run_cli(fifo: &str, name: &str, vendor: &str, product: &str, version: &str) -> i32 {
    let (v, p, ver) = match parse_ids(vendor, product, version) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("REFUSED: {e}");
            return 2;
        }
    };
    // The ordering constraint that has cost runs, in the tool instead of only in the docs.
    eprintln!(
        "NOTE: the pad must exist BEFORE the emulator enters its network namespace - udev \
         hotplug is netlink, netlink is netns-scoped, and a pad created after entry is never \
         announced to the emulator."
    );
    let emus = live_emulators();
    if !emus.is_empty() {
        eprintln!(
            "WARNING: emulator already running (pid {emus:?}); it will NEVER see this pad. \
             If this pad is for that run, stop, start the pad first, relaunch."
        );
    }
    if std::env::var("OPERATOR_PAD").as_deref() == Ok("1") {
        eprintln!(
            "WARNING: OPERATOR_PAD=1 is set - run-boot.sh will neither require this pad nor \
             ignore-list the physical ones. Do not drive two input sources at one menu."
        );
    }
    if (v, p) != (0x045E, 0x028E) {
        eprintln!(
            "WARNING: non-default VID/PID {v:04x}:{p:04x} changes the SDL joystick GUID; SDL's \
             built-in x360 mapping no longer applies and SDL_GAMECONTROLLERCONFIG may be needed. \
             Untested against a live client."
        );
    }
    let identity = PadIdentity { name: name.to_string(), vendor: v, product: p, version: ver };
    let pad = match VirtualPad::create(&identity) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("REFUSED: {e}");
            return 2;
        }
    };
    println!("pad created; SDL_GAMECONTROLLER_IGNORE_DEVICES={}", ignore_list_live(v, p));
    install_signal_handlers();
    let mut out = io::stdout();
    match run_fifo(&pad, Path::new(fifo), &mut out, &STOP) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("pad: {e}");
            2
        }
    }
}

// ── controls ─────────────────────────────────────────────────────────────────────────────────
//
// Two tiers. `pure_controls` runs everywhere and is folded into `d1-harness controls`; the
// device tier (`pad-dev:` names) creates real kernel devices and lives under
// `d1-harness pad controls`. The axis of the device tier is the kernel's own read-back
// (`/proc/bus/input/devices`, the evdev node), never our own bookkeeping.

/// A `Write` that several threads can share; the FIFO end-to-end controls read replies out of it.
#[derive(Clone, Default)]
pub struct SharedOut(Arc<Mutex<Vec<u8>>>);

impl SharedOut {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).to_string()
    }
}

impl Write for SharedOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const IGNORE_FIXTURE: &str = "I: Bus=0005 Vendor=054c Product=09cc Version=8100\n\
N: Name=\"Wireless Controller\"\n\
H: Handlers=event15 js0\n\
\n\
I: Bus=0003 Vendor=045e Product=028e Version=0110\n\
N: Name=\"D1 Preservation Virtual Pad\"\n\
H: Handlers=event16 js1\n\
\n\
I: Bus=0011 Vendor=0001 Product=0001 Version=ab41\n\
N: Name=\"AT Translated Set 2 keyboard\"\n\
H: Handlers=sysrq kbd event3 leds\n\
\n\
I: Bus=0005 Vendor=054c Product=09cc Version=8100\n\
N: Name=\"Wireless Controller duplicate\"\n\
H: Handlers=event17 js2\n";

// ---- the Python source, parsed rather than transcribed --------------------------------------

#[derive(Debug, Default, PartialEq)]
pub struct PyTables {
    pub btn: Vec<(String, u16)>,
    pub alias: Vec<(String, String)>,
    pub axes: Vec<(u16, i32, i32)>,
    pub hat: Vec<(String, (u16, i32))>,
    pub stick: Vec<(String, u16)>,
}

fn py_int(s: &str, idents: &std::collections::BTreeMap<String, i64>) -> Option<i64> {
    let s = s.trim();
    if let Some(v) = idents.get(s) {
        return Some(*v);
    }
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b.trim()),
        None => (false, s),
    };
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else {
        body.parse().ok()?
    };
    Some(if neg { -v } else { v })
}

/// Extract `NAME = {...}` / `NAME = [...]` from Python source: the text between the opening
/// bracket and its match, with per-line `#` comments removed.
fn py_block(src: &str, name: &str, open: char, close: char) -> Option<String> {
    let key = format!("{name} = {open}");
    let start = src.find(&key)? + key.len();
    let mut depth = 1usize;
    let mut end = start;
    for (i, ch) in src[start..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                end = start + i;
                break;
            }
        }
    }
    let body = &src[start..end];
    Some(
        body.lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Split on top-level commas (commas inside `(...)` belong to a tuple).
fn py_items(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn py_key_value(item: &str) -> Option<(String, String)> {
    let item = item.trim();
    let rest = item.strip_prefix('"')?;
    let q = rest.find('"')?;
    let key = rest[..q].to_string();
    let value = rest[q + 1..].trim().strip_prefix(':')?.trim().to_string();
    Some((key, value))
}

/// Parse `capture-rig/virtual-pad.py`'s BTN/ALIAS/AXES/HAT/STICK tables out of its source. This
/// is the same move `controls.rs` makes on `run-boot.sh`: compare against a file this crate does
/// not own, so editing either side fails the control.
pub fn parse_python_tables(src: &str) -> Option<PyTables> {
    // Identifier values: `ABS_X, ABS_Y, ... = 0, 1, ...` assignment lines.
    let mut idents = std::collections::BTreeMap::new();
    for line in src.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((lhs, rhs)) = line.split_once(" = ") else {
            continue;
        };
        let names: Vec<&str> = lhs.split(',').map(str::trim).collect();
        if names.is_empty() || !names.iter().all(|n| n.starts_with("ABS_")) {
            continue;
        }
        let vals: Vec<&str> = rhs.split(',').map(str::trim).collect();
        if vals.len() != names.len() {
            continue;
        }
        for (n, v) in names.iter().zip(&vals) {
            idents.insert(n.to_string(), py_int(v, &std::collections::BTreeMap::new())?);
        }
    }

    let mut t = PyTables::default();
    for item in py_items(&py_block(src, "BTN", '{', '}')?) {
        let (k, v) = py_key_value(&item)?;
        t.btn.push((k, py_int(&v, &idents)? as u16));
    }
    for item in py_items(&py_block(src, "ALIAS", '{', '}')?) {
        let (k, v) = py_key_value(&item)?;
        t.alias.push((k, v.trim_matches('"').to_string()));
    }
    for item in py_items(&py_block(src, "AXES", '[', ']')?) {
        let inner = item.trim().strip_prefix('(')?.strip_suffix(')')?;
        let f: Vec<&str> = inner.split(',').map(str::trim).collect();
        if f.len() != 3 {
            return None;
        }
        t.axes.push((
            py_int(f[0], &idents)? as u16,
            py_int(f[1], &idents)? as i32,
            py_int(f[2], &idents)? as i32,
        ));
    }
    for item in py_items(&py_block(src, "HAT", '{', '}')?) {
        let (k, v) = py_key_value(&item)?;
        let inner = v.trim().strip_prefix('(')?.strip_suffix(')')?;
        let f: Vec<&str> = inner.split(',').map(str::trim).collect();
        if f.len() != 2 {
            return None;
        }
        t.hat.push((k, (py_int(f[0], &idents)? as u16, py_int(f[1], &idents)? as i32)));
    }
    for item in py_items(&py_block(src, "STICK", '{', '}')?) {
        let (k, v) = py_key_value(&item)?;
        t.stick.push((k, py_int(&v, &idents)? as u16));
    }
    Some(t)
}

fn rust_tables() -> PyTables {
    PyTables {
        btn: BTN.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
        alias: ALIAS.iter().map(|&(k, v)| (k.to_string(), v.to_string())).collect(),
        axes: AXES.to_vec(),
        hat: HAT.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
        stick: STICK.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn tables_diff(a: &PyTables, b: &PyTables) -> Vec<String> {
    fn cmp<T: PartialEq + std::fmt::Debug>(name: &str, x: &[T], y: &[T], out: &mut Vec<String>) {
        if x != y {
            out.push(format!("{name}: python {x:?} vs rust {y:?}"));
        }
    }
    let mut out = Vec::new();
    cmp("BTN", &a.btn, &b.btn, &mut out);
    cmp("ALIAS", &a.alias, &b.alias, &mut out);
    cmp("AXES", &a.axes, &b.axes, &mut out);
    cmp("HAT", &a.hat, &b.hat, &mut out);
    cmp("STICK", &a.stick, &b.stick, &mut out);
    out
}

fn python_pad_path() -> String {
    format!("{}/capture-rig/virtual-pad.py", crate::layout::default_repo())
}

/// The always-safe tier: no device, no subprocess. Folded into `d1-harness controls`.
pub fn pure_controls() -> Vec<Control> {
    let mut c = Vec::new();

    // ---- struct layout -------------------------------------------------------------------
    let base = setup_payload(&PadIdentity::default());
    let ok_layout = base.len() == SETUP_PAYLOAD_LEN
        && base[80..82] == BUS_USB.to_ne_bytes()
        && base[82..84] == 0x045Eu16.to_ne_bytes()
        && base[84..86] == 0x028Eu16.to_ne_bytes()
        && base[86..88] == 0x0110u16.to_ne_bytes()
        && base[92..96] == 32767i32.to_ne_bytes()      // absmax[ABS_X]
        && base[348..352] == (-32768i32).to_ne_bytes(); // absmin[ABS_X]
    c.push(check(
        "pad: uinput_user_dev payload is byte-compatible (1116 bytes, ids at 82/84/86)",
        ok_layout,
        format!("{} bytes", base.len()),
    ));

    let other = setup_payload(&PadIdentity { product: 0x028F, ..Default::default() });
    let differing: Vec<usize> = (0..base.len()).filter(|&i| base[i] != other[i]).collect();
    c.push(check(
        "pad: changing the product id moves exactly the product bytes (inverted)",
        differing == vec![84],
        format!("differing offsets: {differing:?}"),
    ));

    let ev = encode_event(EV_KEY, 0x130, 1);
    c.push(check(
        "pad: input_event encoding is 24 bytes with type/code/value at 16/18/20",
        ev.len() == 24
            && ev[16..18] == EV_KEY.to_ne_bytes()
            && ev[18..20] == 0x130u16.to_ne_bytes()
            && ev[20..24] == 1i32.to_ne_bytes()
            && ev[..16].iter().all(|&b| b == 0),
        "time zeroed, exactly as the Python packs it",
    ));

    // ---- tables, against the Python source this crate does not own ------------------------
    match std::fs::read_to_string(python_pad_path()) {
        Err(_) => c.push(skipped(
            "pad: button/axis/alias tables agree with capture-rig/virtual-pad.py",
            "capture-rig/virtual-pad.py not readable from here",
        )),
        Ok(src) => {
            match parse_python_tables(&src) {
                None => c.push(check(
                    "pad: button/axis/alias tables agree with capture-rig/virtual-pad.py",
                    false,
                    "the Python source no longer parses; re-pin the tables",
                )),
                Some(py) => {
                    let diff = tables_diff(&py, &rust_tables());
                    c.push(check(
                        "pad: button/axis/alias tables agree with capture-rig/virtual-pad.py",
                        diff.is_empty(),
                        if diff.is_empty() {
                            format!(
                                "{} buttons, {} aliases, {} axes, {} hats, {} sticks - parsed, \
                                 not transcribed",
                                py.btn.len(),
                                py.alias.len(),
                                py.axes.len(),
                                py.hat.len(),
                                py.stick.len()
                            )
                        } else {
                            diff.join("; ")
                        },
                    ));
                    // The comparator must be able to fail, or the control above is vacuous.
                    let mutated = src.replace("\"a\": 0x130", "\"a\": 0x999");
                    let drifted = parse_python_tables(&mutated)
                        .map(|m| !tables_diff(&m, &rust_tables()).is_empty())
                        .unwrap_or(false);
                    c.push(check(
                        "pad: the table comparator detects a drifted keycode (meta)",
                        drifted && mutated != src,
                        "one keycode perturbed in a copy; the diff must be non-empty",
                    ));
                }
            }
        }
    }

    // ---- the FIFO vocabulary ---------------------------------------------------------------
    let vocab_ok = parse_line("press a") == Parsed::Press { btn: "a" }
        && parse_line("press CROSS") == Parsed::Press { btn: "CROSS" }
        && parse_line("tap a") == Parsed::Tap { btn: "a", n: 1, gap: 1.0 }
        && parse_line("tap a 3 1.5") == Parsed::Tap { btn: "a", n: 3, gap: 1.5 }
        && parse_line("hold a") == Parsed::Hold { btn: "a", secs: 1.0 }
        && parse_line("hold a 2.5") == Parsed::Hold { btn: "a", secs: 2.5 }
        && parse_line("axis ly -1")
            == Parsed::Axis { axis: "ly", value: -1.0, value_raw: "-1", secs: 0.35 }
        && parse_line("axis lx 0.6 0.8")
            == Parsed::Axis { axis: "lx", value: 0.6, value_raw: "0.6", secs: 0.8 }
        && parse_line("sleep 2") == Parsed::Sleep { secs: 2.0 }
        && parse_line("quit") == Parsed::Quit
        && parse_line("exit") == Parsed::Quit
        && parse_line("QUIT") == Parsed::Quit
        && parse_line("") == Parsed::Empty
        && parse_line("   ") == Parsed::Empty
        && parse_line("bogus stuff") == Parsed::Unknown
        && parse_line("press") == Parsed::Unknown
        && parse_line("axis ly") == Parsed::Unknown;
    c.push(check(
        "pad: the FIFO vocabulary parses with the Python's defaults (press/tap/hold/axis/sleep/quit)",
        vocab_ok,
        "n=1 gap=1.0 for tap, 1.0s hold, 0.35s axis - the contract other scripts write",
    ));
    c.push(check(
        "pad: a malformed number is refused instead of killing the pad (deliberate divergence)",
        parse_line("tap a x") == Parsed::Unknown && parse_line("axis ly zz") == Parsed::Unknown,
        "the Python raised ValueError here, which tore down the device mid-run",
    ));

    // ---- axis arithmetic --------------------------------------------------------------------
    let axis_ok = axis_value(ABS_Y, -1.0) == Some((-32768, 0))
        && axis_value(ABS_Y, 1.0) == Some((32767, 0))
        && axis_value(ABS_X, 0.6) == Some((19660, 0)) // int(32767 * 0.6) truncates
        && axis_value(ABS_X, -0.5) == Some((-16384, 0))
        && axis_value(ABS_Z, 1.0) == Some((255, 0))
        && axis_value(ABS_Z, 0.5) == Some((127, 0)) // 127.5 truncates toward zero
        && axis_value(ABS_Y, -2.0) == Some((-32768, 0)); // clamped
    c.push(check(
        "pad: axis arithmetic matches the Python (truncate toward zero, clamp, recentre)",
        axis_ok,
        "ly -1 => -32768; lx 0.6 => 19660; l2 0.5 => 127; out-of-range clamps",
    ));
    c.push(check(
        "pad: reply floats format as Python str() (1.0 keeps its .0)",
        fmt_f(1.0) == "1.0" && fmt_f(0.35) == "0.35" && fmt_f(1.5) == "1.5" && fmt_f(2.0) == "2.0",
        "hold/axis replies embed these",
    ));

    // ---- PAD LAW ignore list ---------------------------------------------------------------
    let got = ignore_list_from(IGNORE_FIXTURE, 0x045E, 0x028E);
    c.push(check(
        "pad: PAD-LAW ignore list - js devices in, self out, keyboard out, duplicates collapsed",
        got == "0x054c/0x09cc",
        format!("fixture => {got:?}"),
    ));
    let got_other_self = ignore_list_from(IGNORE_FIXTURE, 0x045E, 0x028F);
    c.push(check(
        "pad: PAD-LAW ignore list includes 045e:028e once self is someone else (inverted)",
        got_other_self == "0x054c/0x09cc,0x045e/0x028e",
        format!("self=045e:028f => {got_other_self:?}"),
    ));
    c.push(check(
        "pad: the live /proc/bus/input/devices parses without error",
        // Empty is legal (a headless box); the axis is "does not error", the fixture above is
        // the axis for correctness.
        std::fs::read_to_string("/proc/bus/input/devices").is_ok(),
        format!(
            "{} devices visible",
            live_input_devices().len()
        ),
    ));

    c
}

// ---- device tier -----------------------------------------------------------------------------

fn uinput_open_error() -> Option<String> {
    let path = CString::new("/dev/uinput").unwrap();
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
    if fd < 0 {
        Some(io::Error::last_os_error().to_string())
    } else {
        unsafe { libc::close(fd) };
        None
    }
}

fn wait_for_dev(name: &str, deadline: Duration) -> Option<InputDev> {
    let start = Instant::now();
    loop {
        let found = find_by_name(name);
        if let Some(d) = found.into_iter().next() {
            return Some(d);
        }
        if start.elapsed() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait for a device with `name` whose sysfs path is NOT in `existing`. This is how the
/// side-by-side control stays safe while a production pad with the same hardcoded name is live
/// in another lane: the live pad is identified by sysfs and never matched, let alone touched.
fn wait_for_new_dev(
    name: &str,
    existing: &std::collections::BTreeSet<String>,
    deadline: Duration,
) -> Option<InputDev> {
    let start = Instant::now();
    loop {
        if let Some(d) =
            find_by_name(name).into_iter().find(|d| !existing.contains(&d.sysfs))
        {
            return Some(d);
        }
        if start.elapsed() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn sysfs_set_for_name(name: &str) -> std::collections::BTreeSet<String> {
    find_by_name(name).into_iter().map(|d| d.sysfs).collect()
}

fn wait_sysfs_gone(name: &str, sysfs: &str, deadline: Duration) -> bool {
    let start = Instant::now();
    loop {
        if !find_by_name(name).iter().any(|d| d.sysfs == sysfs) {
            return true;
        }
        if start.elapsed() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_gone(name: &str, deadline: Duration) -> bool {
    let start = Instant::now();
    loop {
        if find_by_name(name).is_empty() {
            return true;
        }
        if start.elapsed() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn event_node(dev: &InputDev) -> Option<PathBuf> {
    dev.handlers
        .iter()
        .find(|h| h.starts_with("event"))
        .map(|h| PathBuf::from(format!("/dev/input/{h}")))
}

/// Open the pad's evdev node for reading. udev applies the seat ACL asynchronously
/// (ID_INPUT_JOYSTICK => uaccess), so EACCES is retried until the deadline.
fn open_event_readable(path: &Path, deadline: Duration) -> io::Result<std::fs::File> {
    let start = Instant::now();
    loop {
        match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path) {
            Ok(f) => return Ok(f),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && start.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

fn read_events(f: &mut std::fs::File, min: usize, deadline: Duration) -> Vec<(u16, u16, i32)> {
    let start = Instant::now();
    let mut raw: Vec<u8> = Vec::new();
    let mut out = Vec::new();
    let mut chunk = [0u8; 24 * 32];
    while out.len() < min && start.elapsed() < deadline {
        match f.read(&mut chunk) {
            Ok(0) => std::thread::sleep(Duration::from_millis(10)),
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
        while raw.len() >= 24 {
            let ev: Vec<u8> = raw.drain(..24).collect();
            let etype = u16::from_ne_bytes([ev[16], ev[17]]);
            let code = u16::from_ne_bytes([ev[18], ev[19]]);
            let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
            out.push((etype, code, value));
        }
    }
    out
}

fn drain_events(f: &mut std::fs::File) {
    let mut chunk = [0u8; 24 * 32];
    loop {
        match f.read(&mut chunk) {
            Ok(0) => return,
            Ok(_) => continue,
            Err(_) => return,
        }
    }
}

fn handler_kinds(dev: &InputDev) -> Vec<String> {
    let mut kinds: Vec<String> = dev
        .handlers
        .iter()
        .map(|h| h.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
        .collect();
    kinds.sort();
    kinds.dedup();
    kinds
}

fn bit(dev: &InputDev, key: &str) -> String {
    dev.bits
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn device_controls(c: &mut Vec<Control>) {
    let pid = std::process::id();

    // ---- C-D1: create one pad, prove the KERNEL saw what we asked for ----------------------
    {
        let name = format!("D1 pad ctl {pid} solo");
        let id = PadIdentity { name: name.clone(), ..Default::default() };
        match VirtualPad::create(&id) {
            Err(e) => c.push(check(
                "pad-dev: the kernel enumerates a created pad with the requested identity",
                false,
                format!("create failed: {e}"),
            )),
            Ok(pad) => {
                let dev = wait_for_dev(&name, Duration::from_secs(3));
                let detail;
                let ok = match &dev {
                    None => {
                        detail = "device never appeared in /proc/bus/input/devices".to_string();
                        false
                    }
                    Some(d) => {
                        detail = format!(
                            "bus={} vid={} pid={} ver={} EV={} KEY={} ABS={} handlers={:?}",
                            d.bus,
                            d.vendor,
                            d.product,
                            d.version,
                            bit(d, "EV"),
                            bit(d, "KEY"),
                            bit(d, "ABS"),
                            d.handlers
                        );
                        d.bus == "0003"
                            && d.vendor == "045e"
                            && d.product == "028e"
                            && d.version == "0110"
                            && bit(d, "EV") == "b" // SYN|KEY|ABS
                            && bit(d, "KEY") == expected_key_bitmap()
                            && bit(d, "ABS") == expected_abs_bitmap()
                            && d.handlers.iter().any(|h| h.starts_with("event"))
                    }
                };
                c.push(check(
                    "pad-dev: the kernel enumerates a created pad with the requested identity",
                    ok,
                    detail,
                ));
                drop(pad);
                c.push(check(
                    "pad-dev: dropping the pad removes the device (fd close = device gone)",
                    wait_gone(&name, Duration::from_secs(3)),
                    "the process must stay alive for the pad to exist - now proven, not asserted",
                ));
            }
        }
    }

    // ---- C-D2 ⭐ the two-instance blocker: distinct ids => two devices ----------------------
    {
        let name_a = format!("D1 pad ctl {pid} A");
        let name_b = format!("D1 pad ctl {pid} B");
        let id_a = PadIdentity { name: name_a.clone(), ..Default::default() };
        let id_b = PadIdentity { name: name_b.clone(), product: 0x028F, ..Default::default() };
        match (VirtualPad::create(&id_a), VirtualPad::create(&id_b)) {
            (Ok(pad_a), Ok(pad_b)) => {
                let dev_a = wait_for_dev(&name_a, Duration::from_secs(3));
                let dev_b = wait_for_dev(&name_b, Duration::from_secs(3));
                let (ok, detail) = match (&dev_a, &dev_b) {
                    (Some(a), Some(b)) => (
                        a.sysfs != b.sysfs
                            && a.vendor == "045e"
                            && b.vendor == "045e"
                            && a.product == "028e"
                            && b.product == "028f",
                        format!(
                            "A {} => {}:{}; B {} => {}:{}",
                            a.sysfs, a.vendor, a.product, b.sysfs, b.vendor, b.product
                        ),
                    ),
                    _ => (false, "one or both pads never enumerated".to_string()),
                };
                c.push(check(
                    "pad-dev: ⭐ two simultaneous pads with distinct --vendor/--product enumerate \
                     as two distinct devices (the two-instance blocker)",
                    ok,
                    detail,
                ));
                // Each instance's PAD-LAW list must ignore the OTHER instance's pad and keep its
                // own visible - the exact mechanism layout.rs's DistinctUsbIds describes.
                let from_a = ignore_list_live(0x045E, 0x028E);
                let from_b = ignore_list_live(0x045E, 0x028F);
                let has_js = dev_a.as_ref().map(|d| d.handlers.iter().any(|h| h.starts_with("js")))
                    == Some(true)
                    && dev_b.as_ref().map(|d| d.handlers.iter().any(|h| h.starts_with("js")))
                        == Some(true);
                if has_js {
                    c.push(check(
                        "pad-dev: each identity's ignore list excludes itself and covers the peer",
                        from_a.contains("0x045e/0x028f")
                            && !from_a.contains("0x045e/0x028e")
                            && from_b.contains("0x045e/0x028e")
                            && !from_b.contains("0x045e/0x028f"),
                        format!("A-side: {from_a:?}; B-side: {from_b:?}"),
                    ));
                } else {
                    c.push(skipped(
                        "pad-dev: each identity's ignore list excludes itself and covers the peer",
                        "no js handler on the virtual pads (joydev not loaded); the ignore list \
                         keys on js",
                    ));
                }
                drop(pad_a);
                c.push(check(
                    "pad-dev: teardown independence - dropping pad A removes A and leaves B",
                    wait_gone(&name_a, Duration::from_secs(3))
                        && wait_for_dev(&name_b, Duration::from_millis(200)).is_some(),
                    "two instances must be stoppable one at a time",
                ));
                drop(pad_b);
                let _ = wait_gone(&name_b, Duration::from_secs(3));
            }
            _ => c.push(check(
                "pad-dev: ⭐ two simultaneous pads with distinct --vendor/--product enumerate \
                 as two distinct devices (the two-instance blocker)",
                false,
                "pad creation failed",
            )),
        }
    }

    // ---- C-D3 inverted arm: SAME identity twice is visibly the defect ----------------------
    {
        let name = format!("D1 pad ctl {pid} twin");
        let id = PadIdentity { name: name.clone(), ..Default::default() };
        match (VirtualPad::create(&id), VirtualPad::create(&id)) {
            (Ok(p1), Ok(p2)) => {
                std::thread::sleep(Duration::from_millis(300));
                let twins = find_by_name(&name);
                let indistinct = twins.len() == 2
                    && twins[0].vendor == twins[1].vendor
                    && twins[0].product == twins[1].product;
                c.push(check(
                    "pad-dev: two pads with ONE identity are indistinguishable by VID/PID - the \
                     defect the port fixes is real and detectable (inverted)",
                    indistinct,
                    format!(
                        "{} devices named {:?}, ids {:?}",
                        twins.len(),
                        name,
                        twins.iter().map(|d| format!("{}:{}", d.vendor, d.product)).collect::<Vec<_>>()
                    ),
                ));
                drop(p1);
                drop(p2);
                let _ = wait_gone(&name, Duration::from_secs(3));
            }
            _ => c.push(check(
                "pad-dev: two pads with ONE identity are indistinguishable by VID/PID - the \
                 defect the port fixes is real and detectable (inverted)",
                false,
                "pad creation failed",
            )),
        }
    }

    // ---- C-D4..C-D7: the FIFO protocol end to end, read back from evdev --------------------
    {
        let name = format!("D1 pad ctl {pid} fifo");
        let id = PadIdentity { name: name.clone(), ..Default::default() };
        let tmp = std::env::temp_dir().join(format!("d1-pad-ctl-{pid}"));
        let _ = std::fs::create_dir_all(&tmp);
        let fifo = tmp.join("pad.fifo");
        match VirtualPad::create(&id) {
            Err(e) => c.push(check(
                "pad-dev: FIFO end to end - press a emits BTN_SOUTH down/up on the evdev node",
                false,
                format!("create failed: {e}"),
            )),
            Ok(pad) => {
                let dev = wait_for_dev(&name, Duration::from_secs(3));
                let node = dev.as_ref().and_then(event_node);
                let reader = node
                    .as_ref()
                    .map(|n| open_event_readable(n, Duration::from_secs(3)));
                match reader {
                    Some(Ok(mut evf)) => {
                        let pad = Arc::new(pad);
                        let out = SharedOut::default();
                        let stop = Arc::new(AtomicBool::new(false));
                        let (tpad, tout, tstop, tfifo) =
                            (pad.clone(), out.clone(), stop.clone(), fifo.clone());
                        let session = std::thread::spawn(move || {
                            let mut o = tout;
                            run_fifo(&tpad, &tfifo, &mut o, &tstop)
                        });
                        // Wait for the session to create its FIFO.
                        let t0 = Instant::now();
                        while !fifo.exists() && t0.elapsed() < Duration::from_secs(3) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        let send = |cmd: &str| {
                            if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(&fifo) {
                                let _ = f.write_all(cmd.as_bytes());
                            }
                        };

                        drain_events(&mut evf);
                        send("press a\n");
                        let got = read_events(&mut evf, 4, Duration::from_secs(2));
                        c.push(check(
                            "pad-dev: FIFO end to end - press a emits BTN_SOUTH down/up on the \
                             evdev node",
                            got == vec![
                                (EV_KEY, 0x130, 1),
                                (EV_SYN, 0, 0),
                                (EV_KEY, 0x130, 0),
                                (EV_SYN, 0, 0),
                            ] && out.text().contains("press a: ok"),
                            format!("events: {got:?}; replies: {:?}", out.text()),
                        ));

                        drain_events(&mut evf);
                        send("axis ly -1 0.05\n");
                        let neg = read_events(&mut evf, 4, Duration::from_secs(2));
                        send("axis ly 1 0.05\n");
                        let pos = read_events(&mut evf, 4, Duration::from_secs(2));
                        c.push(check(
                            "pad-dev: FIFO axis deflects and recentres; the opposite input flips \
                             the sign (inverted input, changed output)",
                            neg == vec![
                                (EV_ABS, 1, -32768),
                                (EV_SYN, 0, 0),
                                (EV_ABS, 1, 0),
                                (EV_SYN, 0, 0),
                            ] && pos
                                == vec![
                                    (EV_ABS, 1, 32767),
                                    (EV_SYN, 0, 0),
                                    (EV_ABS, 1, 0),
                                    (EV_SYN, 0, 0),
                                ],
                            format!("ly -1 => {neg:?}; ly 1 => {pos:?}"),
                        ));

                        drain_events(&mut evf);
                        send("press zzz\n");
                        let t0 = Instant::now();
                        while !out.text().contains("press zzz")
                            && t0.elapsed() < Duration::from_secs(2)
                        {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        let stray = read_events(&mut evf, 1, Duration::from_millis(200));
                        c.push(check(
                            "pad-dev: an unknown button is refused and emits nothing",
                            out.text().contains("press zzz: UNKNOWN BUTTON") && stray.is_empty(),
                            format!("stray events: {stray:?}"),
                        ));

                        send("quit\n");
                        let joined = session.join();
                        let clean = matches!(joined, Ok(Ok(())));
                        c.push(check(
                            "pad-dev: quit ends the session and removes the FIFO",
                            clean && !fifo.exists(),
                            format!("session result: {joined:?}"),
                        ));
                        drop(pad);
                        let _ = wait_gone(&name, Duration::from_secs(3));
                    }
                    Some(Err(e)) => c.push(skipped(
                        "pad-dev: FIFO end to end - press a emits BTN_SOUTH down/up on the evdev \
                         node",
                        format!(
                            "cannot read the pad's evdev node ({e}); udev's uaccess seat ACL did \
                             not arrive - device creation was still proven above"
                        ),
                    )),
                    None => c.push(check(
                        "pad-dev: FIFO end to end - press a emits BTN_SOUTH down/up on the evdev \
                         node",
                        false,
                        "no event handler found for the pad",
                    )),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

// ---- side-by-side with the Python original (retirement protocol step 7) ---------------------

fn python3_available() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn side_by_side_ignore(c: &mut Vec<Control>) {
    let script = python_pad_path();
    if !Path::new(&script).exists() || !python3_available() {
        c.push(skipped(
            "pad: print-ignore matches virtual-pad.py --print-ignore byte for byte",
            "python3 or capture-rig/virtual-pad.py unavailable",
        ));
        return;
    }
    match std::process::Command::new("python3").args(["-B", &script, "--print-ignore"]).output() {
        Err(e) => c.push(check(
            "pad: print-ignore matches virtual-pad.py --print-ignore byte for byte",
            false,
            format!("python3 failed: {e}"),
        )),
        Ok(out) => {
            let py = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
            let rs = ignore_list_live(0x045E, 0x028E);
            c.push(check(
                "pad: print-ignore matches virtual-pad.py --print-ignore byte for byte",
                out.status.success() && py == rs,
                format!("python {py:?} vs rust {rs:?}"),
            ));
        }
    }
}

fn side_by_side_device(c: &mut Vec<Control>) {
    const NAME: &str = "pad-dev: kernel-visible identity matches the Python pad field for field \
                        (side by side)";
    let script = python_pad_path();
    if !Path::new(&script).exists() || !python3_available() {
        c.push(skipped(NAME, "python3 or capture-rig/virtual-pad.py unavailable"));
        return;
    }
    // The Python hardcodes its device name, and a production pad with that exact name may be
    // live in another lane. Disambiguate by sysfs path: only a device that appears AFTER our
    // own spawn is ours; the pre-existing set is never matched and never written to.
    let default_name = PadIdentity::default().name;
    let pre_existing = sysfs_set_for_name(&default_name);
    let tmp = std::env::temp_dir().join(format!("d1-pad-py-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let fifo = tmp.join("py.fifo");
    let child = std::process::Command::new("python3")
        .args(["-B", &script, "--fifo"])
        .arg(&fifo)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(ch) => ch,
        Err(e) => {
            c.push(check(NAME, false, format!("could not spawn the Python pad: {e}")));
            return;
        }
    };
    let py_dev = wait_for_new_dev(&default_name, &pre_existing, Duration::from_secs(5));
    // Tell the Python pad to exit through its own FIFO; fall back to kill only if it wedges
    // (it is our own child, spawned three lines up).
    let t0 = Instant::now();
    while !fifo.exists() && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(&fifo) {
        let _ = f.write_all(b"quit\n");
    }
    let t0 = Instant::now();
    let mut exited = false;
    while t0.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(_)) = child.try_wait() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !exited {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_dir_all(&tmp);
    let Some(py) = py_dev else {
        c.push(check(NAME, false, "the Python pad never enumerated"));
        return;
    };
    if !wait_sysfs_gone(&default_name, &py.sysfs, Duration::from_secs(5)) {
        c.push(check(NAME, false, "the Python pad did not tear down"));
        return;
    }

    let rs_pad = match VirtualPad::create(&PadIdentity::default()) {
        Ok(p) => p,
        Err(e) => {
            c.push(check(NAME, false, format!("rust pad create failed: {e}")));
            return;
        }
    };
    let rs_dev = wait_for_new_dev(&default_name, &pre_existing, Duration::from_secs(5));
    let rs_sysfs = rs_dev.as_ref().map(|d| d.sysfs.clone());
    drop(rs_pad);
    if let Some(s) = &rs_sysfs {
        let _ = wait_sysfs_gone(&default_name, s, Duration::from_secs(5));
    }
    let Some(rs) = rs_dev else {
        c.push(check(NAME, false, "the Rust pad never enumerated"));
        return;
    };
    let fields = [
        ("bus", &py.bus, &rs.bus),
        ("vendor", &py.vendor, &rs.vendor),
        ("product", &py.product, &rs.product),
        ("version", &py.version, &rs.version),
        ("name", &py.name, &rs.name),
    ];
    let mut diff: Vec<String> = fields
        .iter()
        .filter(|(_, a, b)| a != b)
        .map(|(k, a, b)| format!("{k}: py={a} rs={b}"))
        .collect();
    for key in ["EV", "KEY", "ABS"] {
        if bit(&py, key) != bit(&rs, key) {
            diff.push(format!("B:{key}: py={} rs={}", bit(&py, key), bit(&rs, key)));
        }
    }
    if handler_kinds(&py) != handler_kinds(&rs) {
        diff.push(format!(
            "handler kinds: py={:?} rs={:?}",
            handler_kinds(&py),
            handler_kinds(&rs)
        ));
    }
    c.push(check(
        NAME,
        diff.is_empty(),
        if diff.is_empty() {
            format!(
                "identity, EV/KEY/ABS bitmaps and handler kinds all equal ({}:{} v{}, {:?})",
                rs.vendor,
                rs.product,
                rs.version,
                handler_kinds(&rs)
            )
        } else {
            diff.join("; ")
        },
    ));
}

/// The full pad suite: pure + python side-by-side + device tier. With `require_device`, a skipped
/// device control becomes a failure - for the box whose whole point is having `/dev/uinput`.
pub fn controls(require_device: bool) -> Vec<Control> {
    let mut c = pure_controls();
    side_by_side_ignore(&mut c);
    match uinput_open_error() {
        Some(e) => c.push(skipped(
            "pad-dev: device controls (identity, two-pad distinctness, FIFO end to end)",
            format!("/dev/uinput not accessible: {e} - every pad-dev control is UNEXECUTED"),
        )),
        None => {
            device_controls(&mut c);
            side_by_side_device(&mut c);
        }
    }
    if require_device {
        for ctl in &mut c {
            if ctl.name.starts_with("pad-dev") && ctl.state == State::Skipped {
                ctl.state = State::Fail;
                ctl.detail = format!("{} (FAILED: --require-device)", ctl.detail);
            }
        }
    }
    c
}

pub fn controls_main(require_device: bool) -> i32 {
    crate::controls::report(&controls(require_device))
}

// ── tests ────────────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_pad_controls_pass() {
        let failed: Vec<&str> = pure_controls()
            .iter()
            .filter(|c| c.state == State::Fail)
            .map(|c| c.name)
            .collect();
        assert!(failed.is_empty(), "failed: {failed:?}");
    }

    #[test]
    fn payload_matches_the_python_struct_pack() {
        // struct.pack("80sHHHHI" + "64i" * 4, ...) == 80 + 8 + 4 + 4*256 == 1116, native order.
        let p = setup_payload(&PadIdentity::default());
        assert_eq!(p.len(), 1116);
        assert_eq!(&p[..27], b"D1 Preservation Virtual Pad");
        assert_eq!(p[27], 0, "name is NUL padded");
        // ABS_HAT0Y (0x11) min/max are the hat's -1/1.
        assert_eq!(p[92 + 0x11 * 4..92 + 0x11 * 4 + 4], 1i32.to_ne_bytes());
        assert_eq!(p[348 + 0x11 * 4..348 + 0x11 * 4 + 4], (-1i32).to_ne_bytes());
    }

    #[test]
    fn a_name_longer_than_79_bytes_truncates_like_the_python() {
        let long = "x".repeat(200);
        let p = setup_payload(&PadIdentity { name: long, ..Default::default() });
        assert_eq!(p[78], b'x');
        assert_eq!(p[79], 0, "byte 79 stays NUL: the Python slices [:79]");
    }

    #[test]
    fn alias_resolution_reaches_the_same_keycodes() {
        // cross => a => 0x130; l1 => lb => 0x136; up is a hat, not a button.
        let resolve = |n: &'static str| -> &'static str {
            ALIAS.iter().find(|&&(k, _)| k == n).map(|&(_, v)| v).unwrap_or(n)
        };
        let code = |n: &'static str| BTN.iter().find(|&&(k, _)| k == resolve(n)).map(|&(_, c)| c);
        assert_eq!(code("cross"), Some(0x130));
        assert_eq!(code("enter"), Some(0x130));
        assert_eq!(code("accept"), Some(0x130));
        assert_eq!(code("cancel"), Some(0x131));
        assert_eq!(code("l1"), Some(0x136));
        assert_eq!(code("options"), Some(0x13B));
        assert_eq!(code("up"), None);
        assert!(HAT.iter().any(|&(k, _)| k == "up"));
    }

    #[test]
    fn the_python_table_parser_reads_the_real_source_or_is_absent() {
        let Ok(src) = std::fs::read_to_string(python_pad_path()) else {
            eprintln!("SKIPPED: capture-rig/virtual-pad.py not present");
            return;
        };
        let py = parse_python_tables(&src).expect("the Python source parses");
        assert_eq!(tables_diff(&py, &rust_tables()), Vec::<String>::new());
    }

    #[test]
    fn kernel_bitmap_format_matches_the_probe() {
        // Measured on this box: a pad with only BTN_SOUTH prints KEY=1000000000000 0 0 0 0.
        assert_eq!(bitmap_words(&[0x130]), "1000000000000 0 0 0 0");
        assert_eq!(expected_abs_bitmap(), "3003f");
    }

    #[test]
    fn device_controls_pass_where_uinput_is_available() {
        if let Some(e) = uinput_open_error() {
            eprintln!("SKIPPED: /dev/uinput not accessible here ({e}); run `d1-harness pad controls` on the rig");
            return;
        }
        let failed: Vec<String> = controls(false)
            .iter()
            .filter(|c| c.state == State::Fail)
            .map(|c| format!("{}: {}", c.name, c.detail))
            .collect();
        assert!(failed.is_empty(), "failed: {failed:?}");
    }
}
