// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Entity indices - the one piece of authoritative state a stub Activity Host really can own.
//!
//! An **entity index** is a bit position `0..12287` keying a *replication entity* (a simulation
//! object). Not a player, not a peer. `[C]`: `FUN_0008F800(ac+0x5EA8, 0x3000)` at `0xB4DF80` sizes
//! the pool at `0x3000` = 12,288 bits = `0x600` bytes.
//!
//! The protocol, from `historicaldocs/design/activity-host-contract.md` §5:
//!
//! | direction | udt | shape |
//! |---|---|---|
//! | client → AH | 19 `allocate_entity_indices` | `u32 count` |
//! | AH → client | 0 `entity_slots_allocated` | the `0x600` mask |
//! | client → AH | 20 `free_entity_indices` | the `0x600` mask |
//!
//! The client's apply (`FUN_010579F0`) **ORs** the granted mask into its authority mask at
//! `ac+0x5EA8`, 384 `u32` words at a time (`while ((int)lVar5 != 0x180)`), sets the dirty flag at
//! `ac+0x64A8` and the has-been-allocated flag at `ac+0x64AA`. `[C]`
//!
//! ## ⭐ The offline seam - the cheapest way past the `wait entity_slots_allocated` gate
//!
//! `historicaldocs/design/activity-host-contract.md` §5 records a branch in the shipped client that does
//! `memset(pool, 0xFF, 0x600)` - **with no Activity Host, the machine self-grants all 12,288
//! indices.** [`EntitySlotMask::grant_all`] mirrors it exactly, so a run can compare "the AH
//! granted everything" against "the client self-granted everything" byte for byte.
//!
//! ⚠ `[U]` **what selects that branch** is still open, and it is the highest-value recon target for
//! a first in-world slice. Nothing here can select it; only the client can.

use crate::protocol::ENTITY_MASK_STRUCT_SIZE;

/// `[C]` `0x3000` - bit positions `0..=12287`.
pub const ENTITY_INDEX_COUNT: usize = 0x3000;
/// `[C]` `0x600` - 384 `u32` words.
pub const ENTITY_MASK_BYTES: usize = ENTITY_MASK_STRUCT_SIZE;
/// 384.
pub const ENTITY_MASK_WORDS: usize = ENTITY_MASK_BYTES / 4;

/// A 12,288-bit entity-index mask, in the client's own word layout.
///
/// ⚠ The **word** layout is `[C]` (384 `u32`s ORed in a loop). The **byte** layout of a word on
/// the wire is not, because the wire form goes through the schema codec - see [`crate::codec`].
/// [`Self::to_struct_bytes`] emits little-endian words, which is the in-memory image the client's
/// handler works on; it is deliberately *not* called `to_wire_bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySlotMask {
    words: Vec<u32>,
}

impl Default for EntitySlotMask {
    fn default() -> Self {
        Self::empty()
    }
}

impl EntitySlotMask {
    /// All bits clear.
    pub fn empty() -> Self {
        Self { words: vec![0u32; ENTITY_MASK_WORDS] }
    }

    /// ⭐ The client's own offline branch: `memset(pool, 0xFF, 0x600)`.
    pub fn grant_all() -> Self {
        Self { words: vec![u32::MAX; ENTITY_MASK_WORDS] }
    }

    /// Read a mask out of a `0x600`-byte struct image (little-endian words).
    pub fn from_struct_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != ENTITY_MASK_BYTES {
            return None;
        }
        let words = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some(Self { words })
    }

    /// The in-memory struct image the client's handler ORs into `ac+0x5EA8`.
    pub fn to_struct_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ENTITY_MASK_BYTES);
        for w in &self.words {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out
    }

    /// Is index `i` set?
    pub fn get(&self, index: usize) -> bool {
        index < ENTITY_INDEX_COUNT && self.words[index / 32] & (1u32 << (index % 32)) != 0
    }

    /// Set index `i`. Out-of-range is a no-op returning `false`.
    pub fn set(&mut self, index: usize) -> bool {
        if index >= ENTITY_INDEX_COUNT {
            return false;
        }
        self.words[index / 32] |= 1u32 << (index % 32);
        true
    }

    /// Clear index `i`.
    pub fn clear(&mut self, index: usize) -> bool {
        if index >= ENTITY_INDEX_COUNT {
            return false;
        }
        self.words[index / 32] &= !(1u32 << (index % 32));
        true
    }

    /// How many indices are set. The client's pool policy is `free = popcount(pool)`.
    pub fn count(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// OR another mask in, exactly as `FUN_010579F0` does.
    pub fn union_with(&mut self, other: &EntitySlotMask) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a |= *b;
        }
    }

    /// Clear every index the other mask has set - the `free_entity_indices` direction.
    pub fn subtract(&mut self, other: &EntitySlotMask) {
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
            *a &= !*b;
        }
    }
}

/// The Activity Host's authoritative allocator for one activity instance.
///
/// Deliberately the dullest possible policy: hand out the lowest free indices. The client's own
/// pool keeps low/high water marks and asks for `(high+low)/2 - free`; nothing about *our* side
/// needs to be clever, and a clever allocator would only make a run harder to read.
#[derive(Debug, Clone)]
pub struct EntitySlotAllocator {
    granted: EntitySlotMask,
}

impl Default for EntitySlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl EntitySlotAllocator {
    pub fn new() -> Self {
        Self { granted: EntitySlotMask::empty() }
    }

    /// Grant `count` indices and return **only the newly granted ones**, which is what
    /// `entity_slots_allocated` carries (the client ORs it in). Grants fewer than asked if the pool
    /// runs out; `count()` on the result says how many.
    pub fn allocate(&mut self, count: u32) -> EntitySlotMask {
        let mut grant = EntitySlotMask::empty();
        let mut remaining = count as usize;
        let mut i = 0usize;
        while remaining > 0 && i < ENTITY_INDEX_COUNT {
            if !self.granted.get(i) {
                self.granted.set(i);
                grant.set(i);
                remaining -= 1;
            }
            i += 1;
        }
        grant
    }

    /// ⭐ Grant the **whole pool** in one unsolicited message, and record it as granted.
    ///
    /// This is what an Activity Host pushes right after `join_result`, because `FUN_010579F0` has
    /// no "was it requested?" check and `ac+0x64AA` - the flag `FUN_00298EF0` reads - is written in
    /// exactly one place in the whole image, that handler. Waiting for `19 allocate_entity_indices`
    /// makes the gate depend on `*(char*)(FUN_0034D950()+0x81)`, which is `[U]`, and in run
    /// `20260808-234535-CODEC-LIVE2` the request never came.
    ///
    /// ⚠ It goes through the allocator rather than returning [`EntitySlotMask::grant_all`] directly
    /// so that a later `free_entity_indices` is subtracted from a pool that agrees with what the
    /// client actually holds. Returning the constant would leave `granted()` empty and the next
    /// `allocate` would hand out indices the client already has.
    pub fn grant_all(&mut self) -> EntitySlotMask {
        self.granted = EntitySlotMask::grant_all();
        EntitySlotMask::grant_all()
    }

    /// Take indices back - the `free_entity_indices` / `abandon_entity_slots` direction.
    pub fn free(&mut self, mask: &EntitySlotMask) {
        self.granted.subtract(mask);
    }

    /// Everything currently out on loan.
    pub fn granted(&self) -> &EntitySlotMask {
        &self.granted
    }

    /// How many indices remain.
    pub fn free_count(&self) -> usize {
        ENTITY_INDEX_COUNT - self.granted.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two numbers the whole subsystem hangs on, and the relationship between them.
    #[test]
    fn twelve_thousand_two_hundred_and_eighty_eight_indices_in_a_1536_byte_mask() {
        assert_eq!(ENTITY_INDEX_COUNT, 12_288);
        assert_eq!(ENTITY_MASK_BYTES, 1_536);
        assert_eq!(ENTITY_MASK_WORDS, 384);
        assert_eq!(ENTITY_INDEX_COUNT, ENTITY_MASK_BYTES * 8);
        // 0x180 is the literal loop bound in FUN_010579F0.
        assert_eq!(ENTITY_MASK_WORDS, 0x180);
    }

    #[test]
    fn the_offline_seam_is_a_memset_of_0xff() {
        let all = EntitySlotMask::grant_all();
        assert_eq!(all.count(), ENTITY_INDEX_COUNT);
        assert_eq!(all.to_struct_bytes(), vec![0xFFu8; ENTITY_MASK_BYTES]);
        // Positive control on the same helper: empty is not all.
        assert_eq!(EntitySlotMask::empty().to_struct_bytes(), vec![0u8; ENTITY_MASK_BYTES]);
    }

    #[test]
    fn struct_bytes_round_trip_and_bit_zero_is_word_zero_bit_zero() {
        let mut m = EntitySlotMask::empty();
        m.set(0);
        m.set(31);
        m.set(32);
        m.set(12_287);
        let bytes = m.to_struct_bytes();
        assert_eq!(bytes[0], 0x01, "index 0 is the low bit of word 0");
        assert_eq!(bytes[3], 0x80, "index 31 is the high bit of word 0");
        assert_eq!(bytes[4], 0x01, "index 32 is the low bit of word 1");
        assert_eq!(bytes[ENTITY_MASK_BYTES - 1], 0x80, "index 12287 is the last bit");
        assert_eq!(EntitySlotMask::from_struct_bytes(&bytes), Some(m));
    }

    #[test]
    fn from_struct_bytes_refuses_a_wrong_size() {
        assert_eq!(EntitySlotMask::from_struct_bytes(&[0u8; 4]), None);
        // Positive control: the right size is accepted.
        assert!(EntitySlotMask::from_struct_bytes(&[0u8; ENTITY_MASK_BYTES]).is_some());
    }

    #[test]
    fn the_allocator_grants_only_what_is_new_and_returns_it() {
        let mut a = EntitySlotAllocator::new();
        let first = a.allocate(4);
        assert_eq!(first.count(), 4);
        assert!(first.get(0) && first.get(3) && !first.get(4));
        let second = a.allocate(2);
        assert_eq!(second.count(), 2);
        assert!(!second.get(0), "already-granted indices are not re-granted");
        assert!(second.get(4) && second.get(5));
        assert_eq!(a.granted().count(), 6);
        a.free(&first);
        assert_eq!(a.granted().count(), 2);
    }

    #[test]
    fn the_allocator_grants_short_rather_than_lying_when_the_pool_runs_out() {
        let mut a = EntitySlotAllocator::new();
        assert_eq!(a.allocate(ENTITY_INDEX_COUNT as u32).count(), ENTITY_INDEX_COUNT);
        assert_eq!(a.free_count(), 0);
        assert_eq!(a.allocate(1).count(), 0, "an empty grant, not a wrapped one");
    }

    #[test]
    fn union_is_what_the_client_does_on_apply() {
        let mut have = EntitySlotMask::empty();
        have.set(1);
        let mut grant = EntitySlotMask::empty();
        grant.set(2);
        have.union_with(&grant);
        assert!(have.get(1) && have.get(2));
    }
}
