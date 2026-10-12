//! Session-parameter payloads - **reference only. We never emit these.**
//!
//! # ⛔ THE SERVER CANNOT SET A SESSION PARAMETER. NOT VIA BAP, NOT VIA ANY MESSAGE, NOT AT ALL.
//!
//! `FUN_00B8D630` - the "may I write this parameter" predicate - reads the owning session's state
//! at `S+0x8C48` and returns true iff it is one of `{6,7,8,9}`. The 11-entry state name table at
//! `0x19548D0` reads `none, peer-creating, peer-joining, peer-join-abort, peer-established,
//! peer-leaving, host-established, host-disband, host-handoff, host-reestablish, election` - so
//! states 6-9 are the four `host-*` states and the predicate is literally **"am I the host peer of
//! this session."**
//!
//! Session parameters replicate **peer to peer**, over the game's own session channel
//! (`c_network_session : c_network_channel_owner`): message **`0x25`** host→peer
//! `parameters-update` and **`0x26`** peer→host `parameter-change-request`, dispatched at
//! `0xB3AA60`. **They do not travel over BAP at all.** The server is not a peer: it holds no
//! `c_network_session`, has no peer index, and appears in no peer bitmask.
//!
//! ⇒ Any plan of the form *"the server delivers `s_activity_host_connection_information`"* is
//! **impossible**. Being a peer would mean implementing `c_network_session`, the peer channel, the
//! `0x1E` membership blob (`0x108` bytes, lookup3-style checksum, seed `0xDEADBFF6`) and the
//! `0x25`/`0x26` replication - that is a **second game client**, not a server.
//!
//! # So why is this module here?
//!
//! Because the two levers we *do* have are only legible against it. The real path is:
//!
//! ```text
//! [server]  answer BAP message 6 with message 7   ← ✅ OURS
//! [client]  0x594C60 delivers it as user_defined_type 10                            [C]
//! [client]  the activity manager, AS SESSION HOST, calls 0xB8CDF0 to write
//!           s_activity_host_connection_information.activity_host_id (+0x08)         [I] ⚠ weak link
//! [client]  replicates it to its own peers via session message 0x25                 [C]
//! [client]  0x75C200 sees a nonzero AH id and calls FUN_00594320                    [C]
//! [client]  FUN_00594320 issues BAP message 16                                      [C]
//! [server]  answer with message 17                 ← ✅ OURS
//! ```
//!
//! The `[I]` step - *who* calls `0xB8CDF0` - was searched for by two agents and not found. **Do not
//! paper over it.** Its falsifier is free: if message 7 lands and message 16 never appears, the
//! inference is wrong. The hard half is `[C]`: `FUN_00594320` returns false and re-issues nothing
//! while [`SActivityHostConnectionInformation::activity_host_id`] is zero, so 16/17 strictly
//! follows 6/7.
//!
//! Reach: **`static-client-inference`**.

/// Registry index of the `activity-host` parameter.
pub const PARAM_INDEX_ACTIVITY_HOST: u32 = 3;
/// Registry index of `s_network_session_world_controller_goal`.
pub const PARAM_INDEX_WORLD_CONTROLLER_GOAL: u32 = 0;
/// Registry index of `s_desired_activity_information`.
pub const PARAM_INDEX_DESIRED_ACTIVITY_INFORMATION: u32 = 5;
/// Registry index of `s_activity_information`.
pub const PARAM_INDEX_ACTIVITY_INFORMATION: u32 = 8;

/// `[C]` In-memory size of [`SActivityHostConnectionInformation`]. `m_value` lives at `param+0xE8`
/// and `m_pending` at `param+0x108`, and the equality test between them is a `0x20`-byte `memcmp`.
pub const ACTIVITY_HOST_PARAM_SIZE: usize = 0x20;
/// `[C]` In-memory size of `s_network_session_world_controller_goal`. Fields `[U]`.
pub const WORLD_CONTROLLER_GOAL_PARAM_SIZE: usize = 0x0C;
/// `[C]` In-memory size of `s_desired_activity_information`. Fields `[U]`.
pub const DESIRED_ACTIVITY_INFORMATION_PARAM_SIZE: usize = 0x9C;
/// `[C]` In-memory size of `s_activity_information`. Fields `[U]` except that it carries the
/// `activity_nonce` that [`SActivityHostConnectionInformation::activity_nonce`] must equal.
pub const ACTIVITY_INFORMATION_PARAM_SIZE: usize = 0xA8;

/// ⛔ **NOT OURS TO SEND.** Parameter index 3, `activity-host`.
///
/// # ⚠ Two serializations, and that is a finding rather than a disagreement
///
/// One analysis found this struct serialized **bit-packed, 208 bits (26 bytes)** by the registry
/// codec pair `0xB8D1A0`/`0xB8D270`. Another found it serialized as a **raw `0x20` struct copy** by
/// vtable slots 9/11 on the `0x25` wire path. **Both are true - there are two independent
/// serializations of the same struct.** The `0x25` peer path uses the raw copy; the registry's
/// bit-packed codec's consumer is `[U]` (plausibly the QoS / advertised session properties, which
/// would be the only place a *server* ever sees a session-parameter value).
///
/// ⇒ **Do not derive a wire length from the `0x20` size column: memory size ≠ wire size**, and this
/// struct is the proof. [`Self::to_memory_bytes`] is the **raw in-memory** form and is named that
/// way on purpose; the bit-packed form is deliberately **not implemented**, because writing it
/// would require inventing a field order nobody has read.
///
/// ```text
/// +0x00  u64  activity_nonce         `[C]` must equal s_activity_information.activity_nonce
/// +0x08  u64  activity_host_id       `[C]` ⭐ THE AH ID. 0 == "no AH id".
/// +0x10  u32  registered_peer_mask   `[C]` gate: mask >> (own_slot & 0x1F) & 1
/// +0x14  u32  field_14               `[U]`
/// +0x18  u16  field_18               `[U]`
/// +0x1A  u8[6] padding               inside the 0x20 memcmp
/// ```
///
/// ⛔ **RETRACTED: the `{addr, addr2, port}` hypothesis for `+0x14`/`+0x18`.** Two analyses
/// independently found exactly **one** writer (`0xB8D0D0`), **one** call site (`0x77657D`, the
/// *repair* path) passing `(0, 0)`, and **no reader**. One setter with one zero-valued call site is
/// not evidence of meaning, and the client already gets address and port from message 17, so a
/// second copy here would be redundant. **Emit zeros** - the only value the client is ever observed
/// to write, and what [`Default`] gives you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SActivityHostConnectionInformation {
    /// `[C]` Must equal `s_activity_information.activity_nonce`.
    pub activity_nonce: u64,
    /// `[C]` ⭐ The activity-host id. **`0` means "no AH id"**, and while it is zero
    /// `FUN_00594320` issues no message 16 - which is the hard half of the 6/7 → 16/17 ordering.
    pub activity_host_id: u64,
    /// `[C]` The gate is `mask >> (own_slot & 0x1F) & 1`.
    pub registered_peer_mask: u32,
    /// `[U]`. See the retraction above: emit zero.
    pub field_14: u32,
    /// `[U]`. See the retraction above: emit zero.
    pub field_18: u16,
}

impl SActivityHostConnectionInformation {
    /// The **raw in-memory** `0x20`-byte form - the one the `0x25` peer path copies verbatim.
    /// Little-endian, because this is x86-64 struct memory and not a wire encoding.
    ///
    /// ⚠ This is **not** the registry's bit-packed 208-bit encoding, and it is not something we may
    /// put on any wire. It exists so a live read of `param+0xE8` can be parsed and printed.
    pub fn to_memory_bytes(&self) -> [u8; ACTIVITY_HOST_PARAM_SIZE] {
        let mut b = [0u8; ACTIVITY_HOST_PARAM_SIZE];
        b[0x00..0x08].copy_from_slice(&self.activity_nonce.to_le_bytes());
        b[0x08..0x10].copy_from_slice(&self.activity_host_id.to_le_bytes());
        b[0x10..0x14].copy_from_slice(&self.registered_peer_mask.to_le_bytes());
        b[0x14..0x18].copy_from_slice(&self.field_14.to_le_bytes());
        b[0x18..0x1A].copy_from_slice(&self.field_18.to_le_bytes());
        b
    }

    /// Parse a `0x20`-byte live read of `param+0xE8` (or `+0x108` for `m_pending`).
    pub fn from_memory_bytes(b: &[u8; ACTIVITY_HOST_PARAM_SIZE]) -> Self {
        let u64_at = |at: usize| {
            let mut v = [0u8; 8];
            v.copy_from_slice(&b[at..at + 8]);
            u64::from_le_bytes(v)
        };
        Self {
            activity_nonce: u64_at(0x00),
            activity_host_id: u64_at(0x08),
            registered_peer_mask: u32::from_le_bytes([b[0x10], b[0x11], b[0x12], b[0x13]]),
            field_14: u32::from_le_bytes([b[0x14], b[0x15], b[0x16], b[0x17]]),
            field_18: u16::from_le_bytes([b[0x18], b[0x19]]),
        }
    }

    /// `[C]` The predicate `FUN_00594320` is really asking: has anything written the AH id yet?
    pub fn has_activity_host(&self) -> bool {
        self.activity_host_id != 0
    }

    /// `[C]` `mask >> (own_slot & 0x1F) & 1`.
    pub fn peer_is_registered(&self, own_slot: u32) -> bool {
        self.registered_peer_mask >> (own_slot & 0x1F) & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activity_host_parameter_round_trips_its_memory_form() {
        let p = SActivityHostConnectionInformation {
            activity_nonce: 0x0123_4567_89AB_CDEF,
            activity_host_id: 0xD100_0000_0000_00AA,
            registered_peer_mask: 0b1011,
            field_14: 0,
            field_18: 0,
        };
        let bytes = p.to_memory_bytes();
        assert_eq!(bytes.len(), ACTIVITY_HOST_PARAM_SIZE);
        assert_eq!(SActivityHostConnectionInformation::from_memory_bytes(&bytes), p);
        // The AH id is at +0x08 and nothing else may drift into it.
        assert_eq!(&bytes[0x08..0x10], &0xD100_0000_0000_00AAu64.to_le_bytes());
    }

    /// The retraction, as a test: the only value the client is ever observed to write to
    /// `+0x14`/`+0x18` is zero, and that is what `Default` must give.
    #[test]
    fn the_two_unknown_fields_default_to_zero() {
        let p = SActivityHostConnectionInformation::default();
        assert_eq!(p.field_14, 0);
        assert_eq!(p.field_18, 0);
        assert_eq!(p.to_memory_bytes()[0x14..0x1A], [0u8; 6]);
    }

    /// The gate that decides whether message 16 can ever be sent.
    #[test]
    fn a_zero_activity_host_id_means_no_activity_host() {
        assert!(!SActivityHostConnectionInformation::default().has_activity_host());
        assert!(SActivityHostConnectionInformation {
            activity_host_id: 1,
            ..Default::default()
        }
        .has_activity_host());
    }

    #[test]
    fn the_peer_mask_gate_reads_the_low_five_bits_of_the_slot() {
        let p = SActivityHostConnectionInformation {
            registered_peer_mask: 1 << 3,
            ..Default::default()
        };
        assert!(p.peer_is_registered(3));
        assert!(!p.peer_is_registered(4));
        // `own_slot & 0x1F` wraps, and the client does the same masking.
        assert!(p.peer_is_registered(3 + 32));
    }
}
