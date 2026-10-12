// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The positive controls - enforced, not documented.**
//!
//! `.claude/skills/native-oracle/SKILL.md`, first non-negotiable:
//!
//! > **Run the control.** Every oracle prints a `CONTROL … PASS` line first. **A decode reported
//! > without its control is not evidence.**
//!
//! In the Python that is a convention: `main()` calls `control()` unless you pass `--no-control`.
//! Here it is a **precondition on construction**. [`ObjectOracle::new`](crate::object::ObjectOracle::new)
//! and [`MessageOracle::new`](crate::message::MessageOracle::new) call
//! [`require_object_control`] / [`require_message_control`] before returning a usable oracle, so a
//! caller cannot obtain a decode without the control having passed. The controls are memoised for
//! the life of the process - they are pure functions of one pinned image - so paying for them once
//! per test binary is the whole cost.
//!
//! ## The object control
//!
//! The pinned character reference vector `e880000000000080f0181c0000`, produced independently on
//! 2026-08-06 by driving `0x105B410` (`historicaldocs/design/rsat-character-object.md` §7), decoded under tag
//! `0x80801954`. It must give SOID `0xD100000000000101` at `+0x00`, identity `00 01 00` at `+0x08`,
//! and **exactly 97 bits**.
//!
//! ⚠ **The bit count is not decoration.** A payload that is too short still "decodes correctly"
//! because the bitstream reads past the end and returns zeros - see
//! [`OracleError::ShortPayload`](crate::error::OracleError::ShortPayload).
//!
//! ## The message control
//!
//! Both pinned vectors from `rsat-schema-encode-oracle.py --selftest`: the 503 response body and
//! 206's `10`. Both were produced by earlier independent work in this project.

use std::sync::OnceLock;

use crate::error::{OracleError, Result};
use crate::message::MessageOracle;
use crate::object::ObjectOracle;

pub const CONTROL_TAG: u32 = 0x8080_1954;
pub const CONTROL_VECTOR: &str = "e880000000000080f0181c0000";
pub const CONTROL_SOID: u64 = 0xD100_0000_0000_0101;
pub const CONTROL_IDENTITY: [u8; 3] = [0, 1, 0];
pub const CONTROL_BITS: u32 = 97;

/// 503 response body, tag `0x80801825`, msgid 503.
pub const PIN_503: &str = "01f7180000000d1000000000000010000000000000000bfffffffffffffffc\
00000000000000020810220000000100";
/// 206 response, tag `0x80801869`, msgid 206, request tag `0x80801774`.
pub const PIN_206: &str = "00ce10";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectControl {
    pub tag: u32,
    pub vector: String,
    pub soid: u64,
    pub identity: [u8; 3],
    pub bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageControl {
    pub wire_503: String,
    pub wire_206: String,
}

static OBJECT: OnceLock<std::result::Result<ObjectControl, String>> = OnceLock::new();
static MESSAGE: OnceLock<std::result::Result<MessageControl, String>> = OnceLock::new();

fn run_object_control() -> std::result::Result<ObjectControl, String> {
    let vector = hex(CONTROL_VECTOR).map_err(|e| e.to_string())?;
    let mut o = ObjectOracle::new_unchecked(CONTROL_TAG).map_err(|e| e.to_string())?;
    let d = o.decode(&vector).map_err(|e| e.to_string())?;
    let soid = d.qword(0);
    let identity: [u8; 3] = d.object[8..11].try_into().unwrap();
    let mut bad = Vec::new();
    if soid != CONTROL_SOID {
        bad.push(format!("soid {soid:#018x} != {CONTROL_SOID:#018x}"));
    }
    if identity != CONTROL_IDENTITY {
        bad.push(format!("identity {identity:02x?} != {CONTROL_IDENTITY:02x?}"));
    }
    if d.bits != CONTROL_BITS {
        bad.push(format!("bits {} != {CONTROL_BITS}", d.bits));
    }
    if !bad.is_empty() {
        return Err(bad.join("; "));
    }
    Ok(ObjectControl {
        tag: CONTROL_TAG,
        vector: CONTROL_VECTOR.to_string(),
        soid,
        identity,
        bits: d.bits,
    })
}

fn run_message_control() -> std::result::Result<MessageControl, String> {
    let mut o = MessageOracle::new_unchecked(0x8080_1825, 503, 0x8080_1726)
        .map_err(|e| e.to_string())?;
    let (h, obj) = o.make_handle(&[], &[]).map_err(|e| e.to_string())?;
    o.write_object_u64(obj, 0x08, 0xD100_0000_0000_0001).map_err(|e| e.to_string())?;
    o.write_object_u64(obj, 0x18, 0x7FFF_FFFF_FFFF_FFFF).map_err(|e| e.to_string())?;
    let w503 = o.encode(h, 0x20000).map_err(|e| e.to_string())?;

    let mut o2 = MessageOracle::new_unchecked(0x8080_1869, 206, 0x8080_1774)
        .map_err(|e| e.to_string())?;
    let (h2, _) = o2.make_handle(&[], &[]).map_err(|e| e.to_string())?;
    let w206 = o2.encode(h2, 0x20000).map_err(|e| e.to_string())?;

    let mut bad = Vec::new();
    if hexs(&w503) != PIN_503 {
        bad.push(format!("503 {} != pinned {PIN_503}", hexs(&w503)));
    }
    if hexs(&w206) != PIN_206 {
        bad.push(format!("206 {} != pinned {PIN_206}", hexs(&w206)));
    }
    if !bad.is_empty() {
        return Err(bad.join("; "));
    }
    Ok(MessageControl { wire_503: hexs(&w503), wire_206: hexs(&w206) })
}

/// The object control, run at most once per process.
pub fn object_control() -> &'static std::result::Result<ObjectControl, String> {
    OBJECT.get_or_init(run_object_control)
}

/// The message control, run at most once per process.
pub fn message_control() -> &'static std::result::Result<MessageControl, String> {
    MESSAGE.get_or_init(run_message_control)
}

pub fn require_object_control() -> Result<&'static ObjectControl> {
    object_control().as_ref().map_err(|e| OracleError::ControlFailed(e.clone()))
}

pub fn require_message_control() -> Result<&'static MessageControl> {
    message_control().as_ref().map_err(|e| OracleError::ControlFailed(e.clone()))
}

pub fn hex(s: &str) -> Result<Vec<u8>> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.len() % 2 != 0 {
        return Err(OracleError::Shape(format!("odd-length hex: {s:?}")));
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|e| OracleError::Shape(format!("bad hex {:?}: {e}", &t[i..i + 2])))
        })
        .collect()
}

pub fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
