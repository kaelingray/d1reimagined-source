// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The **message** codec - `0x261BE0` (encode) / `0x261FB0` (decode).
//!
//! Port of `capture-rig/oodle/rsat-schema-encode-oracle.py`'s `SchemaOracle` encode/decode.
//!
//! ⚠ **This is not the object oracle.** This codec wraps the object in a `u16` message id and a
//! trailing span-presence bit; [`crate::object`] does neither. Investment message bodies (503, 504,
//! 701, 702 …) go through here; QueueZ server objects go through the object oracle. The two differ
//! by exactly that id + bit, and **picking the wrong one silently yields a wrong answer** - no
//! error, just a payload the client will not accept.
//!
//! The message handle is a 0x20-byte struct: `+0x00` u16 message id, `+0x08` object pointer,
//! `+0x10` span pointer, `+0x18` i32 span count.
//!
//! ⚠ `out` from the decoder is a **handle, not the object**. `out+0x08` holds the pointer to the
//! decoded C struct; reading `out` directly yields that pointer where the first field should be
//! (a plainly heap-shaped `0x05061820`) and it *will* be reported as a decoded value if you let it.

use crate::control;
use crate::error::{OracleError, Result};
use crate::schema::SchemaHarness;

pub const ENCODER: u64 = 0x26_1BE0; // (handle, 1, out, capacity, out_len)
pub const DECODER: u64 = 0x26_1FB0; // (in, in_len, direction, out, capacity, decoded_size)
pub const HANDLE_SIZE: u64 = 0x20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMessage {
    pub rc: u8,
    /// The decoded C struct reached through `handle+0x08`.
    pub object: Vec<u8>,
    pub span: Vec<u8>,
    pub span_count: i32,
    pub alloc: u64,
}

pub struct MessageOracle {
    s: SchemaHarness,
    msgid: u16,
}

impl MessageOracle {
    /// Build an oracle for `tag`. **The positive control runs first** (memoised): the 503 response
    /// body and 206's `10` must both reproduce before any new claim is made.
    pub fn new(tag: u32, msgid: u16, request_tag: u32) -> Result<Self> {
        control::require_message_control()?;
        Self::new_unchecked(tag, msgid, request_tag)
    }

    pub(crate) fn new_unchecked(tag: u32, msgid: u16, request_tag: u32) -> Result<Self> {
        Ok(Self { s: SchemaHarness::new(tag, msgid, request_tag)?, msgid })
    }

    pub fn tag(&self) -> u32 {
        self.s.tag
    }
    pub fn struct_size(&self) -> usize {
        self.s.struct_size
    }

    /// `(handle, object)` - the object pointer is returned so a caller can poke individual fields
    /// the way the Python selftest does.
    pub fn make_handle(&mut self, object: &[u8], span: &[u8]) -> Result<(u64, u64)> {
        let obj = self.s.make_object(object)?;
        let handle = self.s.h.alloc(HANDLE_SIZE)?;
        self.s.h.write_u16(handle, self.msgid)?;
        self.s.h.write_u64(handle + 0x08, obj)?;
        if !span.is_empty() {
            let p = self.s.h.alloc(span.len() as u64 + 16)?;
            self.s.h.write(p, span)?;
            self.s.h.write_u64(handle + 0x10, p)?;
        }
        self.s.h.write_i32(handle + 0x18, span.len() as i32)?;
        Ok((handle, obj))
    }

    pub fn write_object_u64(&mut self, obj: u64, offset: u64, value: u64) -> Result<()> {
        self.s.h.write_u64(obj + offset, value)
    }

    pub fn encode(&mut self, handle: u64, capacity: u64) -> Result<Vec<u8>> {
        let out = self.s.h.alloc(capacity)?;
        let out_len = self.s.h.alloc(8)?;
        let rc = (self.s.h.call(self.s.c.msg_encoder, &[handle, 1, out, capacity, out_len])? & 0xFF) as u8;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: self.s.c.msg_encoder, rc });
        }
        let n = self.s.h.read_u32(out_len)? as usize;
        self.s.h.read(out, n)
    }

    pub fn decode(&mut self, wire: &[u8], capacity: u64) -> Result<DecodedMessage> {
        let inp = self.s.h.alloc(wire.len() as u64 + 16)?;
        self.s.h.write(inp, wire)?;
        let out = self.s.h.alloc(capacity)?;
        let dsz = self.s.h.alloc(8)?;
        let rc = (self
            .s
            .h
            .call(self.s.c.msg_decoder, &[inp, wire.len() as u64, 1, out, capacity, dsz])?
            & 0xFF) as u8;
        let objp = self.s.h.read_u64(out + 8)?; // ⚠ the handle's object pointer, not `out`
        let object =
            if objp != 0 { self.s.h.read(objp, self.s.struct_size)? } else { Vec::new() };
        let ptr = self.s.h.read_u64(out + 0x10)?;
        let count = i32::from_le_bytes(self.s.h.read(out + 0x18, 4)?.try_into().unwrap());
        let span = if ptr != 0 && count > 0 && count <= 0x10000 {
            self.s.h.read(ptr, count as usize)?
        } else {
            Vec::new()
        };
        Ok(DecodedMessage { rc, object, span, span_count: count, alloc: self.s.h.read_u64(dsz)? })
    }
}
