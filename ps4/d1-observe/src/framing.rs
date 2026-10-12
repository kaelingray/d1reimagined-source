// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

use std::fmt;

/// Stateful splitter for the six-byte BAP outer envelope.
///
/// Bytes 2..6 are a big-endian payload length; a complete frame is exactly
/// `6 + declared_length` bytes. The splitter retains incomplete tails and
/// drains every coalesced frame in one feed.
#[derive(Debug, Clone)]
pub struct BapFrameAccumulator {
    buffered: Vec<u8>,
    max_payload: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    DeclaredLengthTooLarge { declared: usize, maximum: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclaredLengthTooLarge { declared, maximum } => {
                write!(f, "declared BAP payload {declared} exceeds maximum {maximum}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

impl BapFrameAccumulator {
    pub fn new(max_payload: usize) -> Self {
        Self { buffered: Vec::new(), max_payload }
    }

    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        self.buffered.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            if self.buffered.len() < 6 {
                break;
            }
            let declared = u32::from_be_bytes([
                self.buffered[2],
                self.buffered[3],
                self.buffered[4],
                self.buffered[5],
            ]) as usize;
            if declared > self.max_payload {
                return Err(FrameError::DeclaredLengthTooLarge {
                    declared,
                    maximum: self.max_payload,
                });
            }
            let total = 6 + declared;
            if self.buffered.len() < total {
                break;
            }
            frames.push(self.buffered.drain(..total).collect());
        }
        Ok(frames)
    }
}

impl Default for BapFrameAccumulator {
    fn default() -> Self {
        Self::new(16 * 1024 * 1024)
    }
}

