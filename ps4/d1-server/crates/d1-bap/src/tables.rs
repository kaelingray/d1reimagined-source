//! The three generated tables the correlation table stands on.
//!
//! Every table here was **re-derived in this session from the pinned 01.29 eboot**
//! (`d1-eboot controls` → 22/22 PASS, build sha256
//! `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`) rather than transcribed from
//! `historicaldocs/design/SERVER-ROLES-PLAN.md`. The derivation for each is written above it, with the exact
//! `d1-eboot` invocation, so the next reader can re-run it instead of trusting this file.
//!
//! Reach: **`static-client-inference`**, with four pairings corroborated `observed-live` (marked
//! [`PolicySource::ObservedLive`] - they are the exchanges our own server completes in every run).
//!
//! # Why these tables exist at all
//!
//! `0xB2E940` - the client's receive path - is a **head-of-two-FIFOs match**, not a lookup. It has
//! no per-type exemption and no grace count: a reply the head is not waiting for raises `0x12`
//! `_connection_failure_failed_to_find_message_recipient` and drives the connection to state 9
//! `_dead`. The on-screen name for that is **Weasel**. So the server must know, before it answers
//! anything:
//!
//! 1. **does this request expect a reply at all** ([`POLICIES`]) - replying to a fire-and-forget is
//!    instantly unroutable, because no in-flight entry was ever created for it; and
//! 2. **how many header bytes does the reply carry** ([`HEADER_LEN`]) - a short header does not
//!    error, it parks the client in its resumable header read (`0xB2ED40`) with no timeout, no log
//!    and no error branch, which is indistinguishable from "the game never got there".

// ─── (a) header classes - the eight inner-header shapes ────────────────────────────────────────

/// Inner-header length in bytes, indexed by **header class**.
///
/// `[C]` Derived here, not transcribed. The codec table at `0x19234A0` holds **8** singleton
/// pointers; each singleton's vptr leads to a vtable whose **slot 1 (`+0x08`)** is a
/// `mov eax, <len> ; ret` thunk:
///
/// ```text
/// d1-eboot reloc --slot 0x19234a0 --span 64        # 8 singletons: 0x1923460 .. 0x1923498
///   → vptrs 0x1922e50 0x1922ef0 0x1922f90 0x1923030 0x19230d0 0x1923170 0x1923210 0x19232b0
///   → slot 1 thunks:      6      6      8     14      6     24     32      8
/// ```
///
/// The receive path reaches it as
/// `0x559DD0(type) → singleton; singleton->vfunc[+0x08]() → class; 0x55C1E0(class) → codec;
/// codec->vfunc[+0x08]() → header size`.
pub const HEADER_LEN: [usize; 8] = [6, 6, 8, 14, 6, 24, 32, 8];

/// Per-class flag byte. **Transcribed from `historicaldocs/design/SERVER-ROLES-PLAN.md` §0.1 - NOT re-derived
/// here**, unlike [`HEADER_LEN`]. Bit [`HEADER_FLAG_HAS_STATUS`] means "carries a `u16be` status
/// at `+6`", and it is set only on classes **2 and 7**.
///
/// The cross-check this file *can* make on its own: classes 2 and 7 are exactly the classes whose
/// length is `6 + 2`, which is what a status-carrying base header must be. That is asserted as a
/// test. The remaining bits are `[U]` and nothing here reads them.
pub const HEADER_FLAGS: [u8; 8] = [0x00, 0x00, 0x18, 0x04, 0x00, 0x43, 0x4F, 0x18];

/// Bit in [`HEADER_FLAGS`] meaning "this class carries a `u16be` status at `+6`".
/// Getter `0x55D7C0` = `movzx eax, word [rsi+6]`; setter `0x55D7B0` = `mov [rdx+6], si`.
pub const HEADER_FLAG_HAS_STATUS: u8 = 0x10;

/// The 58 transport message ids, each with the **header class** taken from message-class vtable
/// slot 1 (`+0x08`).
///
/// ⛔ **The `category` column of `capture-rig/bap-message-table.tsv` is this - the HEADER CLASS -
/// and not "1 = request, 2 = response, 3 = notification".** That gloss is retracted in
/// `historicaldocs/design/SERVER-ROLES-PLAN.md` §0.1: slot 1 takes all eight values `0..=7`, which a three-valued
/// category cannot, and it indexes an eight-entry table exactly. The *values* in the tsv were always
/// right; only the story was wrong.
///
/// Rows are the tsv's, with three blanks filled in by walking the vtables directly here:
///
/// ```text
/// bap_to_client_notification                 vt 0x1923810  slot0 = 9   slot1 = 0
/// bap_to_client_log_notification             vt 0x1923880  slot0 = 27  slot1 = 0
/// bap_to_client_server_shutdown_notification vt 0x19238f0  slot0 = 20  slot1 = 0
/// ```
///
/// ⚠ **Not complete.** Ids `25` (`0x19`, secure-hello request), `28` (`0x1C`), `29` (`0x1D`),
/// `161` and `162` are absent - the 58-entry registry at `0x1922110` does not carry them, and
/// `0xB316C0` builds hello requests directly rather than through it. [`header_class`] returns
/// `None` for those; do not read that as "class 0".
///
/// ⚠ **Decimal/hex collision, forever.** `16` is `get_activity_host_proxy_request` in **decimal**
/// and `TYPE_UPO_RESPONSE` in **hex**. Every id in this table is decimal. Every `TYPE_*` constant in
/// `lib.rs` is the same number written in hex.
pub const MESSAGE_CLASS: &[(u16, u8, &str)] = &[
    (6, 1, "client_to_activity_host_manager_request"),
    (7, 2, "client_to_activity_host_manager_response"),
    (8, 3, "client_to_activity_host_notification"),
    (9, 0, "bap_to_client_notification"),
    (10, 1, "client_to_world_server_request"),
    (11, 2, "client_to_world_server_response"),
    (12, 1, "client_to_bap_subscription_request"),
    (13, 2, "client_to_bap_subscription_response"),
    (14, 1, "client_to_bap_unsubscribe_request"),
    (15, 2, "client_to_bap_unsubscribe_response"),
    (16, 1, "client_to_bap_get_activity_host_proxy_request"),
    (17, 2, "client_to_bap_get_activity_host_proxy_response"),
    (18, 1, "client_to_bap_client_config_request"),
    (19, 2, "client_to_bap_client_config_response"),
    (20, 0, "bap_to_client_server_shutdown_notification"),
    (21, 1, "client_to_bap_update_purchased_offers_request"),
    (22, 2, "client_to_bap_update_purchased_offers_response"),
    (23, 1, "client_to_bap_account_id_translation_request"),
    (24, 2, "client_to_bap_account_id_translation_response"),
    (26, 2, "client_to_bap_secure_hello_response"),
    (27, 0, "bap_to_client_log_notification"),
    (30, 1, "client_to_bap_channel_startup_request"),
    (31, 2, "client_to_bap_channel_startup_response"),
    (32, 1, "bap_to_client_user_message_request"),
    (33, 2, "bap_to_client_user_message_response"),
    (34, 1, "client_to_bap_update_matchmaking_skill_request"),
    (35, 2, "client_to_bap_update_matchmaking_skill_response"),
    (36, 1, "client_to_bap_friends_update_request"),
    (37, 2, "client_to_bap_friends_generic_response"),
    (38, 1, "client_to_bap_party_invite_request"),
    (39, 2, "client_to_bap_party_invite_response"),
    (40, 1, "client_to_bap_check_party_invite_request"),
    (41, 2, "client_to_bap_check_party_invite_response"),
    (42, 1, "client_to_bap_update_matchmaking_core_request"),
    (43, 2, "client_to_bap_update_matchmaking_core_response"),
    (44, 1, "client_to_bap_clan_request"),
    (45, 2, "client_to_bap_clan_response"),
    (46, 1, "client_to_bap_nat_punch_request"),
    (47, 4, "bap_to_client_nat_punch_relay_intro_notification"),
    (100, 6, "activity_host_to_client_notification"),
    (102, 5, "activity_host_to_client_log_notification"),
    (110, 5, "service_to_world_server_request"),
    (112, 7, "activity_host_to_world_server_response"),
    (121, 1, "client_to_bap_queuez_register_request"),
    (122, 2, "client_to_bap_queuez_register_response"),
    (123, 4, "queuez_to_client_update_notification"),
    (124, 4, "queuez_to_client_subscription_lost_notification"),
    (150, 5, "bap_to_activity_host_manager_request"),
    (151, 7, "bap_to_activity_host_manager_response"),
    (153, 6, "bap_to_activity_host_socket_handoff_request"),
    (160, 6, "bap_to_activity_host_notification"),
    (171, 1, "client_to_xetrov_request"),
    (250, 1, "client_to_bap_echo_request"),
    (251, 2, "client_to_bap_echo_response"),
    (300, 1, "client_to_bap_initiate_relay_connection_notification"),
    (301, 4, "bap_to_client_request_relay_connection_notification"),
    (302, 1, "client_to_bap_register_relay_client_request"),
    (303, 2, "client_to_bap_register_relay_client_response"),
];

/// The header class of `msg_type`, or `None` if the id has no row in the 58-entry registry.
///
/// `None` is **not** "class 0". See [`MESSAGE_CLASS`]'s note on the five missing ids.
pub fn header_class(msg_type: u16) -> Option<u8> {
    MESSAGE_CLASS.iter().find(|(id, _, _)| *id == msg_type).map(|(_, class, _)| *class)
}

/// The inner-header length `msg_type` demands, in bytes.
pub fn header_len(msg_type: u16) -> Option<usize> {
    header_class(msg_type).map(|class| HEADER_LEN[class as usize])
}

/// Whether `msg_type`'s header carries the `u16be` status field at `+6`.
pub fn header_carries_status(msg_type: u16) -> Option<bool> {
    header_class(msg_type).map(|class| HEADER_FLAGS[class as usize] & HEADER_FLAG_HAS_STATUS != 0)
}

/// The client's own name for `msg_type`, from RTTI.
pub fn message_name(msg_type: u16) -> Option<&'static str> {
    MESSAGE_CLASS.iter().find(|(id, _, _)| *id == msg_type).map(|(_, _, name)| *name)
}

// ─── (b) the policy table - does this request expect a reply, and which one ────────────────────

/// How a row of [`POLICIES`] was obtained. Provenance, not confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicySource {
    /// Walked out of a `c_investment_bap_message_policy_*` vtable: **slot 2 (`+0x10`)** is a
    /// `mov eax, <request id> ; ret` thunk and **slot 6 (`+0x30`)** is the same for the response id,
    /// with `0` (`xor eax,eax`) meaning *no response is expected at all*. This is the structure
    /// `c_investment_bap_message_queue::vfunc[3]` (`0x567EB0`) reads at runtime.
    PolicyVtable,
    /// No `c_investment_bap_message_policy_*` class exists for this id: it belongs to the
    /// connection's own protocol source (channel startup, secure hello, QueueZ register, echo)
    /// rather than to the investment message queue. The pairing is **`observed-live`** - our server
    /// has answered it in every run since 2026-08-06 and the client advances.
    ObservedLive,
}

/// One row of the client's request→response policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BapPolicy {
    /// Request message id, **decimal**.
    pub request: u16,
    /// The response id the client will be waiting for, or `None` for fire-and-forget.
    ///
    /// ⭐ `None` is the dangerous case: no in-flight entry is ever created for a fire-and-forget
    /// send, so **any** reply to it is unroutable and fatal on arrival.
    pub response: Option<u16>,
    /// The client's own class name, with the `c_investment_bap_message_policy_` prefix stripped.
    pub policy_class: &'static str,
    pub source: PolicySource,
}

/// ⭐ The request→response policy, **derived here** from the client's own policy vtables.
///
/// ```text
/// for each c_investment_bap_message_policy_* in capture-rig/eboot-types.tsv:
///     d1-eboot reloc --slot <vtable> --span 96      # slot 2 = +0x10, slot 6 = +0x30
///     d1-eboot disasm <target> --count 2 --force    # mov eax, <id> ; ret
/// ```
///
/// 33 policy classes, every one of them resolving to a pair of constant thunks - no partial reads,
/// no guesses. `capture-rig/bap-message-table.tsv`'s **`pair_id` column MUST NOT be used**: message
/// vtables are 8 slots (`0x40`), so `+0x48` is the Itanium-ABI RTTI mangled name and `+0x70` is slot
/// 0 of *whatever class the linker emitted next* - right for `6→7`/`10→11`/`12→13`/`16→17` by
/// declaration-order coincidence and nonsense elsewhere (`251→30`, `122→12`, `7→150`).
///
/// ⚠ **Deliberately not deduplicated:** ids `10` appears twice, once for each of the deterministic
/// and nondeterministic world-server policies. They differ **only** in a client-side flag with no
/// wire representation - the server cannot tell them apart and must not try. [`response_for`]
/// therefore takes the first match, and a test asserts the two rows agree.
pub const POLICIES: &[BapPolicy] = &[
    // ── from the policy vtables ────────────────────────────────────────────────────────────────
    p(6, Some(7), "client_to_activity_host_manager_request"),
    p(7, None, "client_to_activity_host_manager_response"),
    p(8, None, "client_to_activity_host_notification"),
    p(9, None, "bap_to_client_notification"),
    p(10, Some(11), "client_to_world_server_nondeterministic_request"),
    p(10, Some(11), "client_to_world_server_deterministic_request"),
    p(12, Some(13), "client_to_bap_subscription_request"),
    p(14, Some(15), "client_to_bap_unsubscribe_request"),
    p(16, Some(17), "client_to_bap_get_activity_host_proxy_request"),
    p(18, Some(19), "client_to_bap_client_config_request"),
    p(20, None, "bap_to_client_server_shutdown_notification"),
    p(21, Some(22), "client_to_bap_update_purchased_offers_request"),
    p(23, Some(24), "client_to_bap_account_id_translation_request"),
    p(27, None, "bap_to_client_log_notification"),
    p(29, None, "client_to_bap_challenge_response"),
    p(32, Some(33), "bap_to_client_user_message"),
    p(34, Some(35), "client_to_bap_matchmaking_skill_request"),
    p(36, Some(37), "client_to_bap_friends_update_request"),
    p(38, Some(39), "client_to_bap_friends_party_invite_request"),
    p(40, Some(41), "client_to_bap_friends_check_party_invite_request"),
    p(42, Some(43), "client_to_bap_matchmaking_core_request"),
    p(44, Some(45), "client_to_bap_clan_request"),
    p(46, None, "client_to_bap_nat_punch_intro_request"),
    p(100, None, "activity_host_to_client_notification"),
    p(102, None, "activity_host_to_client_log_notification"),
    p(110, Some(112), "service_to_world_server_request"),
    p(150, Some(151), "bap_to_activity_host_manager_request"),
    p(160, None, "bap_to_activity_host_notification"),
    p(161, Some(162), "bap_to_activity_host_request"),
    p(162, None, "bap_to_activity_host_response"),
    p(171, None, "client_to_xetrov_notification"),
    p(300, None, "client_to_bap_initiate_relay_connection_notification"),
    p(302, Some(303), "client_to_bap_register_relay_client_request"),
    // ── transport-level, no policy class exists; pairing is `observed-live` ────────────────────
    o(25, Some(26), "secure_hello (0x0019 → 0x001A)"),
    o(30, Some(31), "channel_startup (0x001E → 0x001F)"),
    o(121, Some(122), "queuez_register (0x0079 → 0x007A)"),
    o(250, Some(251), "echo (0x00FA → 0x00FB)"),
];

const fn p(request: u16, response: Option<u16>, policy_class: &'static str) -> BapPolicy {
    BapPolicy { request, response, policy_class, source: PolicySource::PolicyVtable }
}

const fn o(request: u16, response: Option<u16>, policy_class: &'static str) -> BapPolicy {
    BapPolicy { request, response, policy_class, source: PolicySource::ObservedLive }
}

/// What the client expects back for a given request id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsePolicy {
    /// The client created an in-flight entry and is waiting for exactly this id.
    Expects(u16),
    /// `expected_response_type == 0`: **no entry exists**, so any reply is fatal (rule R6).
    FireAndForget,
    /// The id is in neither the policy table nor the observed-live set. Log a named gate and drop.
    /// Do **not** guess `N → N+1`; that heuristic is what R6 exists to forbid.
    Unknown,
}

/// Look a request id up in [`POLICIES`].
pub fn response_for(request: u16) -> ResponsePolicy {
    match POLICIES.iter().find(|row| row.request == request) {
        Some(BapPolicy { response: Some(id), .. }) => ResponsePolicy::Expects(*id),
        Some(BapPolicy { response: None, .. }) => ResponsePolicy::FireAndForget,
        None => ResponsePolicy::Unknown,
    }
}

/// The policy row for a request id, for diagnostics that want the class name and the provenance.
pub fn policy_for(request: u16) -> Option<&'static BapPolicy> {
    POLICIES.iter().find(|row| row.request == request)
}

// ─── (c) the push accept-set - what may legally arrive unsolicited ─────────────────────────────

/// ⭐ **The client's own push whitelist**, re-derived here rather than transcribed.
///
/// ```text
/// d1-eboot reloc --slot 0x1925a90 --span 120     # exactly 13 non-null singleton pointers
///   → each singleton's vptr → vtable slot 0 → mov eax, <id> ; ret
/// ```
///
/// `0xB2E940` consults `conn[0x1138]->vfunc[+0x00](hdr.type)` **only after both in-flight rings have
/// failed to match**, and it asks about the **type alone** - a push's sequence is never read. An id
/// outside this set, arriving with no outstanding request, is the Weasel.
pub const PUSH_ACCEPT_SET: [u16; 13] = [6, 8, 9, 20, 27, 28, 47, 100, 123, 124, 160, 161, 301];

/// The subset of [`PUSH_ACCEPT_SET`] that is **ours to originate**.
///
/// Everything else on the whitelist is Bungie-internal - `6`/`8` are client→activity-host, and
/// `20, 27, 28, 47, 160, 161, 301` belong to services we are not. Sending one is not "a message
/// the client tolerates", it is us pretending to be a component we have not built.
///
/// * **123** `queuez_to_client_update_notification` - `observed-live`, this is the push that
///   delivers our account/character objects and it is how orbit was reached.
/// * **100** `activity_host_to_client_notification` - framable since 2026-08-08 (header class 6 is
///   read), but ⛔ **it does not reach the activity client.** Its push policy's deliver step
///   (`0x564C20`) is a *relay*: it re-emits the body as message 9 on the connection named by the
///   class-6 header's `+0x06`. Kept ours-to-push only so `D1_AH_BAP_PUSH_ID=100` can reproduce the
///   measured failure.
/// * ⭐ **9** `bap_to_client_notification` - **added 2026-08-09.** Same body codec as 100 (the two
///   message classes share vtable slots 2/3/4/6), header class 0, and it is the id whose deliver
///   step (`0x5642E0` → `0x75CB40` → `0x75CB70`) resolves `recipient_id` and calls
///   `c_activity_client::receive_message_from_host`. We originate it because on this project the
///   Activity Host and the BAP relay are the same estate: the client dials our AH directly, so
///   there is no second hop to make it for us. See
///   [`crate::activity_host::BapToClientNotification`] and `historicaldocs/design/establish-client-gate.md`.
pub const OURS_TO_PUSH: [u16; 3] = [9, 100, 123];

/// Is this id legal as an unsolicited frame *to this client*?
pub fn is_pushable(msg_type: u16) -> bool {
    PUSH_ACCEPT_SET.contains(&msg_type)
}

/// Is this id one **we** are entitled to originate?
pub fn is_ours_to_push(msg_type: u16) -> bool {
    OURS_TO_PUSH.contains(&msg_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── header classes ────────────────────────────────────────────────────────────────────────

    /// The plan's three named controls, and the two that matter are **live**: they describe frames
    /// our server already puts on the wire in runs that work.
    #[test]
    fn header_class_controls_from_runs_that_already_work() {
        // Our working QueueZ push: a bare 6-byte header, no status.
        assert_eq!(header_class(123), Some(4));
        assert_eq!(header_len(123), Some(6));
        assert_eq!(header_carries_status(123), Some(false));

        // Our working echo reply: 8 bytes with status 200. 0x00FB == 251.
        assert_eq!(header_class(0x00FB), Some(2));
        assert_eq!(header_len(0x00FB), Some(8));
        assert_eq!(header_carries_status(0x00FB), Some(true));

        // The one that would have silently broken Phase 4.
        assert_eq!(header_class(100), Some(6));
        assert_eq!(header_len(100), Some(32));
    }

    /// The cross-check this file can make without the flags table: a status is two bytes on top of
    /// the 6-byte base, so "carries a status" and "is 8 bytes long" must be the same set.
    #[test]
    fn status_bearing_classes_are_exactly_the_eight_byte_classes() {
        for class in 0..8usize {
            let has_status = HEADER_FLAGS[class] & HEADER_FLAG_HAS_STATUS != 0;
            assert_eq!(
                has_status,
                HEADER_LEN[class] == 8,
                "class {class}: flags say has_status={has_status}, len={}",
                HEADER_LEN[class]
            );
        }
        // …and that set is {2, 7}.
        let with_status: Vec<usize> =
            (0..8).filter(|c| HEADER_FLAGS[*c] & HEADER_FLAG_HAS_STATUS != 0).collect();
        assert_eq!(with_status, vec![2, 7]);
    }

    #[test]
    fn every_message_class_row_indexes_a_real_header_class() {
        assert_eq!(MESSAGE_CLASS.len(), 58, "the registry at 0x1922110 has 58 entries");
        for (id, class, name) in MESSAGE_CLASS {
            assert!(*class < 8, "{name} (id {id}) has header class {class}");
        }
    }

    /// Absence is a real answer here, and it is not "class 0". These five ids genuinely have no row.
    #[test]
    fn the_five_ids_outside_the_registry_return_none_not_zero() {
        for id in [25u16, 28, 29, 161, 162] {
            assert_eq!(header_class(id), None, "id {id} must not be invented into the table");
        }
        // Positive control for the same lookup, so "returns None" is not just a broken function.
        assert_eq!(header_class(26), Some(2));
    }

    // ── the policy table ──────────────────────────────────────────────────────────────────────

    /// The six pairings `historicaldocs/design/SERVER-ROLES-PLAN.md` §2.4 names as the policy table's controls.
    /// Written in the plan's hex; asserted against our decimal walk.
    #[test]
    fn policy_table_reproduces_the_six_named_controls() {
        assert_eq!(response_for(0x0C), ResponsePolicy::Expects(0x0D));
        assert_eq!(response_for(0x0A), ResponsePolicy::Expects(0x0B));
        assert_eq!(response_for(0x0E), ResponsePolicy::Expects(0x0F));
        assert_eq!(response_for(0x6E), ResponsePolicy::Expects(0x70));
        assert_eq!(response_for(6), ResponsePolicy::Expects(7));
        assert_eq!(response_for(0x10), ResponsePolicy::Expects(0x11));
    }

    /// Xetrov is one-way. The plan asserts this and the walk agrees: `0x00AB` = 171,
    /// `client_to_xetrov_notification`, slot 6 is `xor eax,eax`.
    #[test]
    fn xetrov_is_fire_and_forget() {
        assert_eq!(response_for(0x00AB), ResponsePolicy::FireAndForget);
    }

    /// ⚠⚠ **A TWO-INSTRUMENT DISAGREEMENT, RECORDED RATHER THAN RECONCILED AWAY.**
    ///
    /// `historicaldocs/design/SERVER-ROLES-PLAN.md` §2.4 lists `0x0020 → None` as a **control** for this table,
    /// on the strength of a live measurement: answering `0x0020` with `0x0021` made the client hang
    /// up at 650 B where silence reached 2477 B.
    ///
    /// **The policy vtable says otherwise.** `c_investment_bap_message_policy_bap_to_client_user_message`
    /// (vtable `0x1924DC0`) has slot 2 → `0xB1BA0` = `mov eax, 0x20 ; ret` and slot 6 → `0xA7200` =
    /// `mov eax, 0x21 ; ret`. That is a paired request/response, not a fire-and-forget, and the
    /// registry agrees: ids 32/33 are `bap_to_client_user_message_request` (class 1) and
    /// `_response` (class 2).
    ///
    /// The table follows the **eboot**, for two reasons:
    ///  1. §0.5 makes the policy vtables the load-bearing source, and this is that source; and
    ///  2. `lib.rs`'s own `family_subscribe_response` doc already retracts the confound - those
    ///     `0x0020`/`701`/`504` replies "were rejected for being *out of order*, not for being
    ///     wrong", because `0x000C` was sitting unanswered at a queue head the whole time. The
    ///     measurement that produced the plan's control is exactly the measurement the Weasel fix
    ///     explains.
    ///
    /// ⇒ **The plan's asserted control is wrong and must be corrected in place.** This test exists
    /// so that correction cannot be quietly lost. If a future run shows that answering `0x0020`
    /// still kills a connection whose queue is otherwise clean, that is new evidence and this test
    /// is where to record it.
    #[test]
    fn policy_table_contradicts_the_plans_asserted_control_for_0x0020() {
        assert_eq!(response_for(0x0020), ResponsePolicy::Expects(0x0021));
        assert_eq!(policy_for(0x0020).unwrap().policy_class, "bap_to_client_user_message");
    }

    /// The two world-server policies differ only in a client-side flag with no wire representation.
    /// If they ever disagree on the wire pair, the server has been told apart something it cannot
    /// tell apart, and `response_for`'s first-match is no longer safe.
    #[test]
    fn the_two_world_server_policies_agree_on_the_wire_pair() {
        let rows: Vec<_> = POLICIES.iter().filter(|r| r.request == 10).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].response, rows[1].response);
        assert_eq!(rows[0].response, Some(11));
    }

    /// Guessing `N → N+1` is the heuristic R6 forbids. An id we have not walked must say so.
    #[test]
    fn unknown_ids_are_unknown_not_guessed() {
        assert_eq!(response_for(0xFFFF), ResponsePolicy::Unknown);
        assert_eq!(response_for(28), ResponsePolicy::Unknown); // server challenge: no policy class
        // Positive control on the same lookup.
        assert_eq!(response_for(12), ResponsePolicy::Expects(13));
    }

    /// Every response id a policy names must itself be a message the registry knows, or we cannot
    /// write its header. `162` is the one exception and it is *why* the exception list is explicit.
    #[test]
    fn every_paired_response_has_a_header_class_except_the_known_gap() {
        for row in POLICIES {
            if let Some(resp) = row.response {
                if resp == 162 {
                    assert_eq!(header_class(resp), None, "162 is the known registry gap");
                    continue;
                }
                assert!(
                    header_class(resp).is_some(),
                    "{} pairs to {resp}, which has no registry row",
                    row.policy_class
                );
            }
        }
    }

    // ── the push accept-set ───────────────────────────────────────────────────────────────────

    #[test]
    fn push_accept_set_is_the_thirteen_entries_at_0x1925a90() {
        let mut sorted = PUSH_ACCEPT_SET;
        sorted.sort_unstable();
        assert_eq!(sorted, [6, 8, 9, 20, 27, 28, 47, 100, 123, 124, 160, 161, 301]);
    }

    /// The brief said "100 / 102 - we push". Half of that is wrong and the wrong half is fatal.
    #[test]
    fn message_102_is_not_pushable() {
        assert!(!is_pushable(102));
        assert!(!is_ours_to_push(102));
        // Positive control: the ids either side of it in the AH family.
        assert!(is_pushable(100));
        assert!(is_pushable(160));
    }

    /// Nothing we may originate is outside what the client will accept.
    #[test]
    fn everything_ours_to_push_is_on_the_clients_whitelist() {
        for id in OURS_TO_PUSH {
            assert!(is_pushable(id), "we claim to push {id}, which the client would reject");
        }
    }

    /// The three named responses in `PUSH_ACCEPT_SET`'s doc must stay off our list: they are
    /// replies, and R5 says a reply is never unsolicited.
    #[test]
    fn responses_are_never_pushable() {
        for id in [11u16, 13, 102] {
            assert!(!is_pushable(id), "{id} must be a reply, never a push");
        }
    }
}
