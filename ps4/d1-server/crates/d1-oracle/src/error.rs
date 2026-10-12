// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Oracle errors - typed, because two of them are *findings* rather than plumbing.
//!
//! [`OracleError::ShortPayload`] is the §26.67 trap made unmissable, and
//! [`OracleError::ControlFailed`] is the difference between a result and a rumour.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    /// The pinned eboot is missing or is not the image the constants were measured on.
    Eboot(String),
    /// The tag is not an anchored descriptor in this image.
    UnknownTag(u32),
    /// Unicorn itself refused.
    Unicorn(String),
    /// A guest-side guard tripped: stack-clobbering memset, null dispatch, instruction limit …
    Fault(String),
    /// The client's own codec rejected the input.
    CodecReturned { func: u64, rc: u8 },
    /// ⚠ **The trap that cost a session** (`historicaldocs/design/character-and-orbit.md` §26.67).
    ///
    /// A payload that is too *short* still "decodes correctly": the bitstream reads past the end
    /// and returns zeros, which is indistinguishable from "those fields were absent". A 17-byte
    /// account body decoded cleanly while the client read **157 bits out of 136**. So an
    /// under-length payload is a **hard error here**, never a quiet success.
    ShortPayload { bits_read: u32, bits_available: usize },
    /// The positive control did not pass, so nothing downstream is evidence.
    ControlFailed(String),
    /// A decode produced something other than the asserted shape.
    Shape(String),
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eboot(m) => write!(f, "eboot: {m}"),
            Self::UnknownTag(t) => {
                write!(f, "tag {t:#010x} is not an anchored descriptor in this eboot")
            }
            Self::Unicorn(m) => write!(f, "unicorn: {m}"),
            Self::Fault(m) => write!(f, "guest fault: {m}"),
            Self::CodecReturned { func, rc } => {
                write!(f, "the client's own codec {func:#x} returned {rc} - it rejected this input")
            }
            Self::ShortPayload { bits_read, bits_available } => write!(
                f,
                "UNDER-LENGTH PAYLOAD: the client read {bits_read} bits from a {bits_available}-bit \
                 buffer. The decode 'succeeded' only because the bitstream returned zeros past the \
                 end - which is indistinguishable from 'those fields were absent'. Do not put this \
                 on the wire (§26.67)."
            ),
            Self::ControlFailed(m) => write!(
                f,
                "POSITIVE CONTROL FAILED: {m} - nothing this oracle reports is evidence until it \
                 passes"
            ),
            Self::Shape(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for OracleError {}

pub type Result<T> = std::result::Result<T, OracleError>;
