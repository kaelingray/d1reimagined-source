//! ⭐⭐ **The Weasel fix, as a type.** One [`BapConversation`] per BAP TCP connection.
//!
//! # What the client actually does
//!
//! `[C]` `0xB2E940`, read in raw disassembly by two independent analyses. There is **no recipient
//! table**. Each BAP connection owns **two strict-FIFO in-flight rings** (`conn+0x1B80` and
//! `conn+0x1D18`, stride `0x198`, capacity ≤ 32) and the receive path examines **only the head of
//! each**:
//!
//! ```text
//! for q in 0..2:                                   # exactly two, hardcoded
//!     ring = conn + 0x1B80 + q*0x198
//!     if ring.count > 0:
//!         e    = ring.slots[ring.head]             # ONLY THE HEAD. No inner loop. No search.
//!         want = conn->sources[e.source_index]->vfunc[+0x18](e.msg_handle)
//!         if want == hdr.type and e.sequence == hdr.sequence:
//!             dispatch_response(); return
//! push = conn[0x1138]
//! if push and push->vfunc[+0x00](hdr.type):        # "do you accept this TYPE?" - sequence unread
//!     dispatch_push(); return
//! LOG "update_receive() failed to find recipient"; code = 0x12; return false
//! ```
//!
//! `0xB2C160` then calls `raise_fatal(conn, 0x12)` **with no test of the code**: it logs
//! `Fatal error '_connection_failure_failed_to_find_message_recipient' raised.` and drives the
//! connection to state 9 `_dead`. No per-type exemption, no status exemption, no grace count. On
//! screen that is **Weasel**.
//!
//! ⇒ **A wrong-but-routable reply is survivable. A right-but-unroutable reply is not.**
//!
//! # The six rules, and where each is enforced
//!
//! | rule | what it forbids | enforced by |
//! |---|---|---|
//! | **R1 ORDER** | reply *N+1* leaving before reply *N* | [`BapConversation::drain`] pops the front only |
//! | **R2 TOTALITY** | silence - it head-of-line blocks, then times out fatally (`0x0B`) | `drain` synthesises a non-200 reply past `deadline` |
//! | **R3 EXACTLY ONCE** | a duplicate reply, whose first copy already popped the head | [`ResponseTicket`] is not `Clone`; `park` consumes it |
//! | **R4 SEQUENCE ECHO** | a derived or server-side sequence | the sequence lives in the ticket; a handler never supplies one |
//! | **R5 NEVER UNSOLICITED** | pushing anything off the client's 13-entry accept-set | [`BapConversation::push`] |
//! | **R6 NEVER INVENT A RESPONSE CLASS** | replying to a fire-and-forget, which has no in-flight entry | [`EnqueueOutcome::FireAndForget`] hands back **no ticket** |
//!
//! **Why one FIFO is provably enough even though the client keeps two rings.** Both rings are fed
//! by one shared sequence counter at `conn+0x16A0`, and an entry is appended on *send completion* -
//! so the arrival order we observe on the single TCP stream is a **linear extension of both
//! per-ring orders**. Answering in global arrival order satisfies both heads at once. Do **not**
//! partition by source to gain concurrency unless you can prove the partition; the cost of being
//! wrong is the connection.
//!
//! # The type-level part
//!
//! A handler cannot name a sequence it was not given. [`BapConversation::enqueue`] is the **only**
//! way to obtain a [`ResponseTicket`], the ticket is neither `Clone` nor `Copy`, and
//! [`BapConversation::park`] consumes it. So "emit a response for a sequence that has no
//! outstanding request" is not a runtime error to be checked - it is a value that cannot be
//! constructed. [`BapConversation::park_by_sequence`] exists for call sites that cannot thread a
//! ticket through; it is checked at runtime and returns [`ParkError::UnknownSequence`], but it
//! buys none of the static guarantees and new code should not reach for it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::tables::{self, ResponsePolicy};
use crate::STATUS_OK;

/// The status a synthesised reply carries. Deliberately **not** 200: `handle_response_internal`
/// (`0x5681E0`) compares the status to 200 and `jne`s past the body decode entirely, so a non-200
/// reply is *routable but ignored* - exactly the shape rule R2 wants when we have no body to send.
///
/// `[I]` on the choice of 500 specifically. Any non-200 does the same job; 500 is chosen because it
/// is unambiguous in a hex dump (`0x01F4`) and reads as "our fault" in a log.
pub const STATUS_SYNTHESISED: u16 = 500;

/// The default in-flight deadline.
///
/// ⚠ `[U]`, and knowingly a guess. The client's own tunable is a `u32` at absolute
/// `[0x267B1B8] + 0x2684` (`historicaldocs/design/SERVER-ROLES-PLAN.md` §9 Tier 1). `0x267B1B8` is an eboot BSS
/// vaddr and **gets the base**; the pointer read *out of* it is already absolute - `d1guest read
/// --abs`. Until that read happens, "answer promptly" has no number. Two seconds is conservative in
/// the direction that matters: synthesising slightly early costs a wrong body, which is survivable;
/// synthesising too late costs `_connection_failure_timed_out` (`0x0B`), which is not.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(2);

static NEXT_CONVERSATION_ID: AtomicU64 = AtomicU64::new(1);

/// Permission to answer exactly one outstanding request.
///
/// Obtainable only from [`BapConversation::enqueue`]. Not `Clone`, not `Copy`, and consumed by
/// [`BapConversation::park`] - so the ticket *is* the "exactly once" guarantee (R3) and the
/// "echo the sequence verbatim" guarantee (R4), rather than a rule someone has to remember.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "an unparked ticket is an unanswered request: it head-of-line blocks the connection \
              and then kills it with _connection_failure_timed_out (R2)"]
pub struct ResponseTicket {
    conversation: u64,
    sequence: u32,
    request_type: u16,
    response_type: u16,
}

impl ResponseTicket {
    /// The client's sequence, to be echoed verbatim. Read-only on purpose.
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
    /// The request this ticket answers.
    pub fn request_type(&self) -> u16 {
        self.request_type
    }
    /// The response id the client's policy pairs with [`Self::request_type`].
    pub fn response_type(&self) -> u16 {
        self.response_type
    }
}

/// What [`BapConversation::enqueue`] decided about an inbound message.
#[derive(Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// The client's policy pairs this request with a response, an in-flight entry exists on its
    /// side, and the request is now at the tail of our FIFO. Park a body against the ticket.
    Awaiting(ResponseTicket),
    /// Policy says `expected_response_type == 0`. **No in-flight entry was ever created**, so any
    /// reply is unroutable and fatal (R6). Not queued, and no ticket exists to reply with.
    FireAndForget,
    /// The id is in neither the policy table nor the observed-live set. Log a **named gate** and
    /// drop it. Do not guess `N → N+1`.
    Unknown,
    /// Policy pairs this request with a response whose **header class we cannot write**
    /// (classes 3, 5 and 6 - 14, 24 and 32 bytes - are `[U]`), or whose id has no registry row.
    ///
    /// This is R2 losing to physics, and it is reported rather than papered over: writing a 6-byte
    /// header where the client wants 32 does not error, it parks the client in its resumable header
    /// read (`0xB2ED40`) forever, with no timeout, no log and no error branch.
    UnwritableResponse { response_type: u16, header_class: Option<u8> },
    /// The same sequence is already outstanding. The client's counter is monotonic per connection,
    /// so this means our framing double-delivered a frame - a bug on our side, not the client's.
    DuplicateSequence(u32),
}

/// Why a body could not be parked.
#[derive(Debug, PartialEq, Eq)]
pub enum ParkError {
    /// R6 / miss conditions (A) and (D): no outstanding request carries this sequence. This is the
    /// Weasel, caught one layer before the socket.
    UnknownSequence(u32),
    /// R3 / miss condition (E): this sequence already has a reply parked. The first pops the head;
    /// the duplicate is unroutable.
    DuplicateReply(u32),
    /// The handler produced a reply for a different message type than the request's policy pairs
    /// with. Impossible via [`BapConversation::park`]; reachable via
    /// [`BapConversation::park_by_sequence`].
    WrongResponseType { sequence: u32, expected: u16, got: u16 },
    /// A ticket minted by a different [`BapConversation`]. Sequences are per connection, so a
    /// ticket that crossed connections would answer the wrong client's head.
    ForeignTicket { sequence: u32 },
}

/// Why an unsolicited frame was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PushError {
    /// R5: the id is not in the client's 13-entry push accept-set at `0x1925A90`. Sending it is the
    /// Weasel with extra steps.
    NotPushable(u16),
    /// The id is on the client's whitelist but is not ours to originate - `9, 20, 27, 28, 47, 160,
    /// 161, 301` belong to Bungie services, and `6`/`8` are client→activity-host.
    NotOurs(u16),
    /// The id has no registry row, so there is no header class to write.
    ///
    /// ⚠ Until 2026-08-08 this also covered classes **3, 5 and 6**, whose layouts were unread -
    /// which is why message **100** was refused. All eight classes are now implemented
    /// ([`HeaderExtras`], `historicaldocs/design/bap-header-classes.md`), so this variant means only "unknown
    /// message id" today. Kept as-is so the error surface did not change shape.
    UnwritableHeader { msg_type: u16, header_class: Option<u8> },
}

impl std::fmt::Display for ParkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSequence(seq) => {
                write!(f, "no outstanding request at sequence {seq} (this is the Weasel)")
            }
            Self::DuplicateReply(seq) => write!(f, "sequence {seq} already has a reply parked"),
            Self::WrongResponseType { sequence, expected, got } => write!(
                f,
                "sequence {sequence} expects response type {expected}, handler produced {got}"
            ),
            Self::ForeignTicket { sequence } => {
                write!(f, "ticket for sequence {sequence} belongs to another connection")
            }
        }
    }
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPushable(id) => {
                write!(f, "message {id} is not on the client's push accept-set")
            }
            Self::NotOurs(id) => write!(f, "message {id} is pushable but not ours to originate"),
            Self::UnwritableHeader { msg_type, header_class } => write!(
                f,
                "message {msg_type} has header class {header_class:?}, whose layout is unread"
            ),
        }
    }
}

impl std::error::Error for ParkError {}
impl std::error::Error for PushError {}

/// One outstanding request.
#[derive(Debug)]
struct Pending {
    sequence: u32,
    request_type: u16,
    response_type: u16,
    arrived: Instant,
    /// `None` = the handler has not answered yet. Parked replies are **bodies only** - the header
    /// is written by [`BapConversation::drain`] and never by a handler.
    reply: Option<Vec<u8>>,
    /// Set when `drain` had to invent a non-200 reply because this entry aged past the deadline.
    synthesised: bool,
}

/// The correlation table for one BAP TCP connection.
///
/// Every invariant maps to a client-side fatal: responses leave in arrival order or `0x12`; every
/// request is answered exactly once or `0x12` (duplicate) / `0x0B` (timeout); the reply echoes the
/// sequence verbatim or `0x12`; we never reply to a fire-and-forget or `0x12`.
#[derive(Debug)]
pub struct BapConversation {
    id: u64,
    queue: VecDeque<Pending>,
    deadline: Duration,
    /// Server-side counter for **push frames only**. `0xB2E940` tests a push's type and never reads
    /// its sequence, so this space is ours - it must never be derived from, or collide with, the
    /// client's counter at `conn+0x16A0`.
    push_seq: u32,
    /// Count of replies `drain` had to invent. A run with a non-zero value here is a run where R2
    /// was doing real work, which is worth saying out loud in the log.
    synthesised_count: u64,
}

impl BapConversation {
    /// A conversation with an explicit in-flight deadline. See [`DEFAULT_DEADLINE`] for why the
    /// number is currently a guess and what one live read would replace it with.
    pub fn new(deadline: Duration) -> Self {
        Self {
            id: NEXT_CONVERSATION_ID.fetch_add(1, Ordering::Relaxed),
            queue: VecDeque::new(),
            deadline,
            push_seq: 1,
            synthesised_count: 0,
        }
    }

    /// [`Self::new`] with [`DEFAULT_DEADLINE`].
    pub fn with_default_deadline() -> Self {
        Self::new(DEFAULT_DEADLINE)
    }

    /// The deadline this conversation synthesises at.
    pub fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Call on **every** decoded inbound message, from the framing layer, **before any handler
    /// runs**. That ordering is what makes R1 structural rather than a convention: the FIFO records
    /// arrival order at the point arrival actually happens.
    pub fn enqueue(&mut self, sequence: u32, request_type: u16) -> EnqueueOutcome {
        self.enqueue_at(sequence, request_type, Instant::now())
    }

    /// [`Self::enqueue`] with an explicit clock, for tests and for replaying a captured transcript.
    pub fn enqueue_at(
        &mut self,
        sequence: u32,
        request_type: u16,
        now: Instant,
    ) -> EnqueueOutcome {
        let response_type = match tables::response_for(request_type) {
            ResponsePolicy::Expects(id) => id,
            ResponsePolicy::FireAndForget => return EnqueueOutcome::FireAndForget,
            ResponsePolicy::Unknown => return EnqueueOutcome::Unknown,
        };
        if !can_write_header(response_type) {
            return EnqueueOutcome::UnwritableResponse {
                response_type,
                header_class: tables::header_class(response_type),
            };
        }
        if self.queue.iter().any(|p| p.sequence == sequence) {
            return EnqueueOutcome::DuplicateSequence(sequence);
        }
        self.queue.push_back(Pending {
            sequence,
            request_type,
            response_type,
            arrived: now,
            reply: None,
            synthesised: false,
        });
        EnqueueOutcome::Awaiting(ResponseTicket {
            conversation: self.id,
            sequence,
            request_type,
            response_type,
        })
    }

    /// Park a reply **body** against its ticket. The handler never writes a header, never picks a
    /// sequence, and never touches the socket.
    ///
    /// Consuming the ticket is what makes R3 static: there is no second copy to park.
    pub fn park(&mut self, ticket: ResponseTicket, body: Vec<u8>) -> Result<(), ParkError> {
        if ticket.conversation != self.id {
            return Err(ParkError::ForeignTicket { sequence: ticket.sequence });
        }
        self.park_inner(ticket.sequence, ticket.response_type, body)
    }

    /// The checked, ticket-free form, for call sites that cannot thread a ticket through.
    ///
    /// It enforces the same invariants at runtime - an unknown sequence is
    /// [`ParkError::UnknownSequence`], a second reply is [`ParkError::DuplicateReply`] - but it
    /// buys none of the static guarantees. Prefer [`Self::park`].
    pub fn park_by_sequence(
        &mut self,
        sequence: u32,
        response_type: u16,
        body: Vec<u8>,
    ) -> Result<(), ParkError> {
        self.park_inner(sequence, response_type, body)
    }

    fn park_inner(
        &mut self,
        sequence: u32,
        response_type: u16,
        body: Vec<u8>,
    ) -> Result<(), ParkError> {
        let pending = self
            .queue
            .iter_mut()
            .find(|p| p.sequence == sequence)
            .ok_or(ParkError::UnknownSequence(sequence))?;
        if pending.response_type != response_type {
            return Err(ParkError::WrongResponseType {
                sequence,
                expected: pending.response_type,
                got: response_type,
            });
        }
        if pending.reply.is_some() {
            return Err(ParkError::DuplicateReply(sequence));
        }
        pending.reply = Some(body);
        Ok(())
    }

    /// Drain from the **front only**, and only while the front is ready.
    ///
    /// If the front has aged past [`Self::deadline`], synthesise a
    /// [`STATUS_SYNTHESISED`] reply for it rather than let the queue rot: a wrong body is
    /// survivable, silence is not (R2). Returns complete inner frames - header **plus** body - in
    /// the one order that is legal.
    ///
    /// The caller seals each frame and advances the channel nonce. `drain` deliberately knows
    /// nothing about the envelope or the cipher.
    pub fn drain(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(front) = self.queue.front() {
            let aged = now.saturating_duration_since(front.arrived) >= self.deadline;
            if front.reply.is_none() && !aged {
                break;
            }
            let mut front = self.queue.pop_front().expect("front exists");
            if front.reply.is_none() {
                front.synthesised = true;
                front.reply = Some(Vec::new());
                self.synthesised_count += 1;
            }
            let status = if front.synthesised { STATUS_SYNTHESISED } else { STATUS_OK };
            let body = front.reply.take().unwrap_or_default();
            let mut frame = write_header(front.response_type, front.sequence, status)
                .expect("enqueue rejected unwritable response types");
            frame.extend_from_slice(&body);
            out.push(frame);
        }
        out
    }

    /// An unsolicited frame. Bypasses the FIFO by design - a push has no in-flight entry and the
    /// client never reads its sequence - and is validated against both the client's 13-entry
    /// accept-set and our own narrower "ours to send" set.
    ///
    /// The sequence comes from this conversation's own push counter.
    pub fn push(&mut self, msg_type: u16, body: &[u8]) -> Result<Vec<u8>, PushError> {
        let sequence = self.push_seq;
        let frame = self.push_with_sequence(msg_type, sequence, body)?;
        self.push_seq = self.push_seq.wrapping_add(1);
        Ok(frame)
    }

    /// [`Self::push`] with a caller-chosen sequence, for call sites that already run their own push
    /// counter. The client does not read it; keeping it stable only keeps our own logs legible.
    pub fn push_with_sequence(
        &self,
        msg_type: u16,
        sequence: u32,
        body: &[u8],
    ) -> Result<Vec<u8>, PushError> {
        self.push_with_header(msg_type, sequence, &HeaderExtras::default(), body)
    }

    /// [`Self::push_with_sequence`] plus the class-3/5/6 [`HeaderExtras`].
    ///
    /// For a class-0/1/2/4/7 id the extras are ignored - those classes have no room for them -
    /// so this is safe to call unconditionally.
    ///
    /// ⭐ This is the entry point for **BAP 100** `activity_host_to_client_notification`, whose
    /// class-6 header is 32 bytes. A 6-byte header there does not error: it parks the client in
    /// its resumable header read (`0xB2ED40`) with no timeout, no log and no error branch, which
    /// is indistinguishable from "the game never got there". That is why the length is asserted.
    pub fn push_with_header(
        &self,
        msg_type: u16,
        sequence: u32,
        extras: &HeaderExtras,
        body: &[u8],
    ) -> Result<Vec<u8>, PushError> {
        if !tables::is_pushable(msg_type) {
            return Err(PushError::NotPushable(msg_type));
        }
        if !tables::is_ours_to_push(msg_type) {
            return Err(PushError::NotOurs(msg_type));
        }
        // Pushes are requests we originate, so they carry no status: whether the class has one is
        // read from the table rather than assumed.
        let mut frame = write_header_with(msg_type, sequence, STATUS_OK, extras).ok_or(
            PushError::UnwritableHeader { msg_type, header_class: tables::header_class(msg_type) },
        )?;
        frame.extend_from_slice(body);
        Ok(frame)
    }

    /// `(sequence, request_type, age)` for everything still outstanding, oldest first.
    ///
    /// `d1-responder` prints this on every fatal-adjacent event so a stall is legible without a
    /// probe: the head named here is the entry the client is blocked on.
    pub fn outstanding(&self, now: Instant) -> impl Iterator<Item = (u32, u16, Duration)> + '_ {
        self.queue
            .iter()
            .map(move |p| (p.sequence, p.request_type, now.saturating_duration_since(p.arrived)))
    }

    /// How many requests are outstanding.
    pub fn pending_len(&self) -> usize {
        self.queue.len()
    }

    /// Is anything outstanding?
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }

    /// `(sequence, request_type)` of the entry the client is head-of-line blocked on, if any.
    pub fn head(&self) -> Option<(u32, u16)> {
        self.queue.front().map(|p| (p.sequence, p.request_type))
    }

    /// How many replies this conversation has had to invent. Non-zero means R2 saved a connection
    /// a handler would have stalled - say so in the run log.
    pub fn synthesised_count(&self) -> u64 {
        self.synthesised_count
    }
}

// ── the class-3 / 5 / 6 extra header fields ───────────────────────────────────────────────────

/// The extra fields carried by the **activity/service** header classes 3, 5 and 6.
///
/// `[C]` - read out of the client's own encoders and decoders on the pinned 01.29 eboot
/// (`d1-eboot controls` 22/22), and cross-checked against **`observed-live` bytes**. Full write-up
/// and evidence table: `historicaldocs/design/bap-header-classes.md`.
///
/// | class | len | `+0x00` | `+0x02` | `+0x06` | `+0x0E` | `+0x16` | `+0x18` |
/// |---|---|---|---|---|---|---|---|
/// | **3** `client_activity_request` | 14 | `u16be` type | `u32be` seq | **`u64be`** [`Self::field_06`] | - | - | - |
/// | **5** `service_request` | 24 | `u16be` type | `u32be` seq | `u64be` [`Self::field_06`] | `u64be` [`Self::field_0e`] | **`u16be`** [`Self::field_16`] | - |
/// | **6** `service_activity_request` | 32 | `u16be` type | `u32be` seq | `u64be` [`Self::field_06`] | `u64be` [`Self::field_0e`] | ⚠ **`u16` NOT swapped** | `u64be` [`Self::field_18`] |
///
/// The accessors tile each class's byte count **exactly, with no gap and no overlap** - that
/// tiling is the layout's positive control. Encoder / decoder pairs, which mirror each other
/// instruction for instruction: class 3 `0x55CAB0`/`0x55CB20`, class 5 `0x55CF40`/`0x55CFC0`,
/// class 6 `0x55D240`/`0x55D2D0`.
///
/// ## ⚠ `field_16` is the one field where byte order is not a choice
///
/// Class **5** byte-swaps it (`rol word [+0x16], 8`); class **6** does **not** - it is copied
/// verbatim by both `0x55D240` and `0x55D2D0`. So for class 6 "little-endian `u16`" and "two
/// opaque bytes" are the same two instructions, and this type emits it **native-endian for class 6
/// and big-endian for class 5**, matching the client exactly.
///
/// ## What the fields mean
///
/// `[C]` **Class 3's `field_06` is the activity-host id.** Not inferred - read off the wire: the
/// client's own message-8 join request in `runs/20260808-195418-F2-STATE3` carried
/// `00 08 | 00 00 00 03 | D1 00 00 00 00 00 0A 01` and logged that AH as `D1000000:00000A01`.
/// That is the whole 14-byte class-3 header, and it is this crate's `observed-live` control for
/// the entire method.
///
/// `[I]` **Class 6's `field_18` is very likely the same field.** The header classes expose their
/// fields through *semantic* vtable slot pairs, and the pair that reads class 3's activity-host id
/// (slots 8/9, `0x55D1C0`/`0x55D1D0` at `+0x06`) is the same pair that class 6 wires to `+0x18`
/// (`0x55D530`/`0x55D540`). Classes 5 and 6 expose `+0x06` through a *different* pair (4/5).
/// ⚠ Marked `[I]`, not `[C]`: no message-100 frame has ever been on a wire, so nothing has
/// confirmed it. [`Self::default()`] is all zeros and that is what ships until a run says
/// otherwise.
///
/// `[U]` `field_06`, `field_0e` and `field_16` on class 6. Zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeaderExtras {
    /// `u64be` at `+0x06`. Classes 3, 5, 6. **On class 3 this is the activity-host id** `[C]`.
    pub field_06: u64,
    /// `u64be` at `+0x0E`. Classes 5, 6 only. `[U]`
    pub field_0e: u64,
    /// `u16` at `+0x16`. Classes 5, 6 only. ⚠ **big-endian on class 5, native on class 6.** `[U]`
    pub field_16: u16,
    /// `u64be` at `+0x18`. Class 6 only. `[I]` the activity-host id - see the type docs.
    pub field_18: u64,
}

impl HeaderExtras {
    /// A class-3 header's only extra field: the activity-host id. `[C]`
    pub fn activity_host(ah_id: u64) -> Self {
        Self { field_06: ah_id, ..Self::default() }
    }

    /// A class-6 header carrying the activity-host id in the slot-8/9 field at `+0x18`. `[I]` -
    /// see the type docs before using it; [`Self::default`] (all zeros) is what ships.
    pub fn class6_activity_host(ah_id: u64) -> Self {
        Self { field_18: ah_id, ..Self::default() }
    }
}

/// Can we lay down a header for this message id at all?
///
/// All eight classes are implemented since 2026-08-08; only an id with **no registry row** is
/// unwritable. Classes 0, 1 and 4 are the 6-byte base, classes 2 and 7 add a `u16be` status, and
/// classes 3, 5 and 6 add the [`HeaderExtras`] fields.
fn can_write_header(msg_type: u16) -> bool {
    tables::header_class(msg_type).is_some()
}

/// Build the inner header for `msg_type` with all-zero extras. Classes 3, 5 and 6 get their full
/// length; see [`write_header_with`].
fn write_header(msg_type: u16, sequence: u32, status: u16) -> Option<Vec<u8>> {
    write_header_with(msg_type, sequence, status, &HeaderExtras::default())
}

/// Build the inner header for `msg_type`, writing the status only if its class carries one and the
/// activity/service fields only if its class carries those.
///
/// Private on purpose: `lib.rs`'s [`crate::reply_header`] hard-codes the 8-byte class-2 shape,
/// which is right for every reply we send today and wrong for a class-6 push. This is the version
/// that asks the table.
///
/// ⚠ The status is written **iff** `HEADER_FLAGS[class] & 0x10`, which is set only on classes 2
/// and 7. It is **not** set on class 6 (`0x55D220: mov al, 0x4F; ret`, and `0x4F & 0x10 == 0`) -
/// so a message-100 header carries no status, and its status accessor slots 14/15 really are the
/// `0x50FE20` / `0x1DF0` stubs.
fn write_header_with(
    msg_type: u16,
    sequence: u32,
    status: u16,
    extras: &HeaderExtras,
) -> Option<Vec<u8>> {
    let class = tables::header_class(msg_type)?;
    let len = tables::HEADER_LEN[class as usize];
    let has_status = tables::HEADER_FLAGS[class as usize] & tables::HEADER_FLAG_HAS_STATUS != 0;
    let mut header = Vec::with_capacity(len);
    header.extend_from_slice(&msg_type.to_be_bytes());
    header.extend_from_slice(&sequence.to_be_bytes());
    if has_status {
        header.extend_from_slice(&status.to_be_bytes());
    }
    match class {
        // 14 B - one u64be at +0x06.
        3 => header.extend_from_slice(&extras.field_06.to_be_bytes()),
        // 24 B - u64be @+0x06, u64be @+0x0E, u16 **BE** @+0x16 (`0x55CF40` rol word, 8).
        5 => {
            header.extend_from_slice(&extras.field_06.to_be_bytes());
            header.extend_from_slice(&extras.field_0e.to_be_bytes());
            header.extend_from_slice(&extras.field_16.to_be_bytes());
        }
        // 32 B - u64be @+0x06, u64be @+0x0E, u16 **NATIVE** @+0x16, u64be @+0x18.
        6 => {
            header.extend_from_slice(&extras.field_06.to_be_bytes());
            header.extend_from_slice(&extras.field_0e.to_be_bytes());
            header.extend_from_slice(&extras.field_16.to_ne_bytes());
            header.extend_from_slice(&extras.field_18.to_be_bytes());
        }
        _ => {}
    }
    debug_assert_eq!(header.len(), len, "header class {class} must be {len} bytes");
    (header.len() == len).then_some(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST};

    fn t0() -> Instant {
        Instant::now()
    }

    fn ticket(conv: &mut BapConversation, seq: u32, ty: u16, now: Instant) -> ResponseTicket {
        match conv.enqueue_at(seq, ty, now) {
            EnqueueOutcome::Awaiting(t) => t,
            other => panic!("expected Awaiting, got {other:?}"),
        }
    }

    // ── ⭐ THE WEASEL REGRESSION ──────────────────────────────────────────────────────────────

    /// ⭐⭐ **The named regression.** This is the failure that killed both rung-28 runs:
    ///
    /// ```text
    /// update_receive() failed to find recipient for header [sequence N][type c->b sub rsp][status 200]
    /// Fatal error '_connection_failure_failed_to_find_message_recipient' raised.
    /// -> error.lost_bap_connection -> cleanup -> kick_to_bootflow_start
    /// ```
    ///
    /// A response for a sequence with no outstanding request must be **refused before the socket**.
    #[test]
    fn a_response_for_an_unknown_sequence_is_refused() {
        let mut conv = BapConversation::with_default_deadline();
        // Nothing outstanding at all - miss condition (A).
        assert_eq!(
            conv.park_by_sequence(11, 0x000B, vec![]),
            Err(ParkError::UnknownSequence(11))
        );

        // …and still refused when *other* requests are outstanding - miss condition (B), which is
        // the one that actually fires, because a lookup miss and an ordering miss are byte-identical
        // in the client's log.
        let now = t0();
        let t = ticket(&mut conv, 7, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, now);
        assert_eq!(
            conv.park_by_sequence(9, 0x000B, vec![]),
            Err(ParkError::UnknownSequence(9))
        );
        // Positive control on the same call: the sequence that IS outstanding parks fine.
        assert_eq!(conv.park(t, vec![]), Ok(()));
    }

    /// The same guarantee, made structural. A ticket is the only way to name a sequence, it is not
    /// `Clone`, and `park` consumes it - so R3 is not a check that can be forgotten.
    ///
    /// The commented line below is the whole point: it does not compile.
    #[test]
    fn a_ticket_cannot_be_forged_or_reused() {
        let mut conv = BapConversation::with_default_deadline();
        let t = ticket(&mut conv, 5, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, t0());
        assert_eq!(conv.park(t, vec![]), Ok(()));
        // conv.park(t, vec![]);  // ⛔ error[E0382]: use of moved value: `t`
        //
        // And the ticket-free path catches the same thing at runtime:
        assert_eq!(
            conv.park_by_sequence(5, 0x000D, vec![]),
            Err(ParkError::DuplicateReply(5))
        );
    }

    /// R6. A fire-and-forget send never created an in-flight entry, so there is nothing to answer
    /// and - the point - **no ticket is issued**, which makes "answer it anyway" unwritable.
    #[test]
    fn a_fire_and_forget_message_yields_no_ticket() {
        let mut conv = BapConversation::with_default_deadline();
        // 0x00AB = 171, client_to_xetrov_notification. Slot 6 is `xor eax,eax`.
        assert_eq!(conv.enqueue(1, 0x00AB), EnqueueOutcome::FireAndForget);
        assert!(conv.is_idle());
        assert_eq!(conv.park_by_sequence(1, 0x00AC, vec![]), Err(ParkError::UnknownSequence(1)));
    }

    /// R6's other half: an id we have not walked is `Unknown`, never `N → N+1`.
    #[test]
    fn an_unknown_message_id_is_not_guessed() {
        let mut conv = BapConversation::with_default_deadline();
        assert_eq!(conv.enqueue(1, 0xFFFF), EnqueueOutcome::Unknown);
        assert!(conv.is_idle());
    }

    // ── R1: ordering ──────────────────────────────────────────────────────────────────────────

    /// ⭐ The mechanism that actually killed rung 28. Two requests are outstanding; the *second*
    /// one's handler finishes first. Answering it immediately is what raises `0x12`, because the
    /// client compares only the head.
    #[test]
    fn replies_leave_in_arrival_order_even_when_handlers_finish_out_of_order() {
        let now = t0();
        let mut conv = BapConversation::with_default_deadline();
        let first = ticket(&mut conv, 40, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now);
        let second = ticket(&mut conv, 41, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, now);

        // The second handler wins the race.
        conv.park(second, vec![]).unwrap();
        assert!(conv.drain(now).is_empty(), "nothing may leave while sequence 40 is unanswered");

        conv.park(first, vec![0xAA]).unwrap();
        let frames = conv.drain(now);
        assert_eq!(frames.len(), 2);
        // 0x000B, seq 40, status 200, then the body.
        assert_eq!(frames[0], vec![0x00, 0x0B, 0, 0, 0, 40, 0x00, 0xC8, 0xAA]);
        // 0x000D, seq 41, status 200, empty body.
        assert_eq!(frames[1], vec![0x00, 0x0D, 0, 0, 0, 41, 0x00, 0xC8]);
        assert!(conv.is_idle());
    }

    /// R4. The sequence in the frame is the client's, byte for byte, big-endian at `[2..6]`.
    #[test]
    fn the_reply_echoes_the_clients_sequence_verbatim() {
        let now = t0();
        let mut conv = BapConversation::with_default_deadline();
        let t = ticket(&mut conv, 0xDEAD_BEEF, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, now);
        assert_eq!(t.sequence(), 0xDEAD_BEEF);
        conv.park(t, vec![]).unwrap();
        let frames = conv.drain(now);
        assert_eq!(&frames[0][2..6], &0xDEAD_BEEFu32.to_be_bytes());
    }

    // ── R2: totality ──────────────────────────────────────────────────────────────────────────

    /// R2. Silence is not neutral. A head older than the client's tunable raises
    /// `_connection_failure_timed_out` (`0x0B`) - a *different* fatal from the Weasel, and the one a
    /// naive "just drop stale replies" fix would trade the first one for.
    #[test]
    fn an_aged_head_is_answered_rather_than_left_to_rot() {
        let now = t0();
        let mut conv = BapConversation::new(Duration::from_millis(100));
        let stalled = ticket(&mut conv, 3, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now);
        let behind = ticket(&mut conv, 4, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, now);
        conv.park(behind, vec![]).unwrap();

        assert!(conv.drain(now).is_empty());

        let later = now + Duration::from_millis(150);
        let frames = conv.drain(later);
        assert_eq!(frames.len(), 2, "the stalled head unblocks everything behind it");
        // Synthesised: status 500, not 200. Routable, and the client skips the body decode.
        assert_eq!(frames[0], vec![0x00, 0x0B, 0, 0, 0, 3, 0x01, 0xF4]);
        assert_eq!(frames[1], vec![0x00, 0x0D, 0, 0, 0, 4, 0x00, 0xC8]);
        assert_eq!(conv.synthesised_count(), 1);
        // The ticket for the synthesised entry is now stale, and parking it is refused rather than
        // sent as a duplicate - miss condition (E).
        assert_eq!(conv.park(stalled, vec![]), Err(ParkError::UnknownSequence(3)));
    }

    /// A handler that answers before the deadline keeps status 200. The synthesis path must not
    /// fire on a healthy connection, or every body we do have would be discarded by the client.
    #[test]
    fn a_prompt_reply_keeps_status_200() {
        let now = t0();
        let mut conv = BapConversation::new(Duration::from_millis(100));
        let t = ticket(&mut conv, 1, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now);
        conv.park(t, vec![0x01, 0x02]).unwrap();
        let frames = conv.drain(now + Duration::from_millis(150));
        assert_eq!(frames[0], vec![0x00, 0x0B, 0, 0, 0, 1, 0x00, 0xC8, 0x01, 0x02]);
        assert_eq!(conv.synthesised_count(), 0);
    }

    // ── header shapes ─────────────────────────────────────────────────────────────────────────

    /// The two header shapes we already put on the wire, as byte-for-byte controls. If either
    /// regresses, a run stalls with no log at all.
    #[test]
    fn header_shapes_match_the_two_live_controls() {
        // Class 2 - the echo reply, 8 bytes with status.
        assert_eq!(write_header(0x00FB, 5, STATUS_OK).unwrap(), vec![0x00, 0xFB, 0, 0, 0, 5, 0x00, 0xC8]);
        // Class 4 - the QueueZ push, 6 bytes, no status.
        assert_eq!(write_header(123, 9, STATUS_OK).unwrap(), vec![0x00, 0x7B, 0, 0, 0, 9]);
    }

    /// Only an id with **no registry row** is unwritable now. All eight header classes are
    /// implemented; an unknown id must still be refused rather than guessed at.
    #[test]
    fn an_unknown_message_id_is_refused_not_guessed() {
        assert!(write_header(0x0FFF, 1, STATUS_OK).is_none(), "no registry row for 0x0FFF");
        assert!(write_header(0x0001, 1, STATUS_OK).is_none(), "id 1 is a gap in the enum");
        // Positive control: the classes we can write, one per implemented class.
        for id in [13u16, 123, 8, 102, 100] {
            assert!(write_header(id, 1, STATUS_OK).is_some(), "id {id} must be writable");
        }
    }

    /// ⭐ **Header class 3, against `observed-live` bytes.**
    ///
    /// The client's own BAP message 8 in `runs/20260808-195418-F2-STATE3` opened with exactly
    /// these 14 bytes, and the client logged that activity host as `D1000000:00000A01`. This is
    /// the only place in the crate where a class-3/5/6 header is checked against a real wire, so
    /// it is the control for the whole method - the other two classes are the same three
    /// instructions in the same encoder family.
    #[test]
    fn header_class_3_reproduces_the_live_message_8_header() {
        const AH_ID: u64 = 0xD100_0000_0000_0A01;
        let h = write_header_with(8, 3, STATUS_OK, &HeaderExtras::activity_host(AH_ID)).unwrap();
        assert_eq!(
            h,
            vec![0x00, 0x08, 0x00, 0x00, 0x00, 0x03, 0xD1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0A, 0x01]
        );
        assert_eq!(h.len(), tables::HEADER_LEN[3]);
        // Negative control: the id must land big-endian. Little-endian is a different 14 bytes,
        // and the client would read a different activity host rather than fail - the exact shape
        // of the msg-17 byte-order trap in historicaldocs/design/RUNG-30-REACHED.md §3.1.
        assert_ne!(&h[6..14], &AH_ID.to_le_bytes()[..]);
    }

    /// ⭐ Class 6 = 32 bytes, and every field lands where the client's accessors read it.
    ///
    /// Decoded back through the same offsets the client's `0x55D2D0` uses, which is a round-trip
    /// rather than a restatement of the encoder.
    #[test]
    fn header_class_6_is_32_bytes_and_round_trips_through_the_clients_offsets() {
        let extras = HeaderExtras {
            field_06: 0x0102_0304_0506_0708,
            field_0e: 0x1112_1314_1516_1718,
            field_16: 0xABCD,
            field_18: 0xD100_0000_0000_0A01,
        };
        let h = write_header_with(100, 7, STATUS_OK, &extras).unwrap();
        assert_eq!(h.len(), 32, "class 6 is `0xB1BA0: mov eax, 0x20; ret`");
        assert_eq!(h.len(), tables::HEADER_LEN[6]);
        assert_eq!(u16::from_be_bytes(h[0..2].try_into().unwrap()), 100);
        assert_eq!(u32::from_be_bytes(h[2..6].try_into().unwrap()), 7);
        assert_eq!(u64::from_be_bytes(h[6..14].try_into().unwrap()), extras.field_06);
        assert_eq!(u64::from_be_bytes(h[14..22].try_into().unwrap()), extras.field_0e);
        // ⚠ +0x16 is NOT byte-swapped by class 6's codec - native, unlike class 5.
        assert_eq!(u16::from_ne_bytes(h[22..24].try_into().unwrap()), extras.field_16);
        assert_eq!(u64::from_be_bytes(h[24..32].try_into().unwrap()), extras.field_18);
    }

    /// Class 5 is class 6 minus the `+0x18` u64 - and its `+0x16` **is** byte-swapped, which is
    /// the one place the two classes genuinely differ. A test that could not tell them apart would
    /// be decoration.
    #[test]
    fn header_class_5_is_24_bytes_and_swaps_the_field_that_class_6_does_not() {
        let extras = HeaderExtras { field_16: 0x00FF, ..HeaderExtras::default() };
        let c5 = write_header_with(102, 1, STATUS_OK, &extras).unwrap();
        let c6 = write_header_with(100, 1, STATUS_OK, &extras).unwrap();
        assert_eq!(c5.len(), 24);
        assert_eq!(c6.len(), 32);
        assert_eq!(&c5[22..24], &[0x00, 0xFF], "class 5 rol word [+0x16], 8 => big-endian");
        assert_eq!(&c6[22..24], &0x00FFu16.to_ne_bytes()[..], "class 6 never swaps +0x16");
        // The discriminating assertion: on a little-endian host these two really are different
        // bytes, so the test fails if either class borrows the other's rule.
        #[cfg(target_endian = "little")]
        assert_ne!(&c5[22..24], &c6[22..24]);
    }

    /// No header class may carry a status unless `HEADER_FLAGS & 0x10` says so - and it does not
    /// say so for class 6.
    ///
    /// ⚠ This asserts against a re-reading of `0x55D220` (`mov al, 0x4F; ret`). `0x4F & 0x10 == 0`.
    /// A session brief claimed class 6 has the bit set; the arithmetic refuses it, and the
    /// accessor evidence agrees - classes 2 and 7 are the only ones with a status getter
    /// (`0x55D7C0`), and they are the only two whose flag byte is `0x18`.
    #[test]
    fn only_classes_2_and_7_carry_a_status() {
        let with_status: Vec<usize> = (0..8)
            .filter(|c| tables::HEADER_FLAGS[*c] & tables::HEADER_FLAG_HAS_STATUS != 0)
            .collect();
        assert_eq!(with_status, vec![2, 7]);
        assert_eq!(tables::HEADER_FLAGS[6], 0x4F);
        assert_eq!(tables::HEADER_FLAGS[6] & tables::HEADER_FLAG_HAS_STATUS, 0);
        // 200 must not appear anywhere in a class-6 header built with a non-default status.
        let h = write_header_with(100, 0, STATUS_OK, &HeaderExtras::default()).unwrap();
        assert!(h[6..].iter().all(|b| *b == 0), "class 6 body fields default to zero, no status");
    }

    // ── R5: pushes ────────────────────────────────────────────────────────────────────────────

    /// R5. Only the client's 13-entry accept-set may arrive unsolicited, and only **three** of
    /// those are ours. `13` - one of the two types that killed rung 28 - must be a reply, never a
    /// push.
    ///
    /// ⭐ **PARTIALLY INVERTED 2026-08-09.** `9` used to be the example of "on the client's list,
    /// but not ours to originate". It is ours now: it is the only id whose deliver step reaches
    /// `c_activity_client::receive_message_from_host`, and on this project there is no separate
    /// BAP relay to make the AH→BAP→client hop for us. `160` keeps the role `9` used to play, so
    /// the test still discriminates between the two lists rather than collapsing them.
    #[test]
    fn only_whitelisted_ids_may_be_pushed() {
        let mut conv = BapConversation::with_default_deadline();
        assert_eq!(conv.push(0x000D, &[]), Err(PushError::NotPushable(0x000D)));
        assert_eq!(conv.push(0x000B, &[]), Err(PushError::NotPushable(0x000B)));
        assert_eq!(conv.push(102, &[]), Err(PushError::NotPushable(102)));
        // On the client's list, but not ours to originate.
        assert_eq!(conv.push(160, &[]), Err(PushError::NotOurs(160)));
        assert_eq!(conv.push(301, &[]), Err(PushError::NotOurs(301)));
        // And 9 now frames, with the plain 6-byte class-0 header. The sequence is the
        // conversation's own push counter and is deliberately not asserted here.
        let frame = conv.push(9, &[0xAA]).unwrap();
        assert_eq!(frame.len(), 6 + 1, "message 9 is header class 0 = 6 bytes");
        assert_eq!(&frame[0..2], &[0x00, 0x09]);
        assert_eq!(frame[6], 0xAA, "the body starts at +6 - no status, no extras");
    }

    /// ⭐⭐ **INVERTED 2026-08-08.** This test used to assert that message 100 was refused because
    /// header class 6 was unread. Class 6 is read, so it now asserts the frame goes out - and it
    /// still discriminates, because it pins the 32-byte header length and the body's position.
    ///
    /// The gate that replaced it: a wrong `field_18` (or a wrong body `recipient_id`) is a
    /// **silent drop**, not an error. Framing is no longer the blocker; addressing is.
    #[test]
    fn message_100_goes_out_with_a_32_byte_class_6_header() {
        let mut conv = BapConversation::with_default_deadline();
        let body = [0xAAu8, 0xBB, 0xCC];
        let frame = conv.push(100, &body).unwrap();
        assert_eq!(frame.len(), 32 + body.len());
        assert_eq!(&frame[0..2], &[0x00, 0x64], "u16be type = 100");
        assert_eq!(&frame[2..6], &1u32.to_be_bytes(), "u32be sequence from the push counter");
        assert_eq!(&frame[32..], &body, "body starts after 32 header bytes, not 6");
        // Negative control: the shape that would have stalled the client forever. If a future
        // refactor reintroduces the 6-byte header this fails instead of hanging a run.
        assert_ne!(frame.len(), 6 + body.len());
        assert_ne!(&frame[6..6 + body.len()], &body);
    }

    /// The activity-host id, if it belongs at `+0x18` (`[I]`), reaches the wire unchanged.
    #[test]
    fn message_100_carries_the_activity_host_id_at_offset_0x18() {
        const AH_ID: u64 = 0xD100_0000_0000_0A01;
        let conv = BapConversation::with_default_deadline();
        let frame = conv
            .push_with_header(100, 4, &HeaderExtras::class6_activity_host(AH_ID), &[0x01])
            .unwrap();
        assert_eq!(&frame[24..32], &AH_ID.to_be_bytes());
        // …and the default really is all zeros, so nothing is smuggled in behind the caller.
        let plain = conv.push_with_sequence(100, 4, &[0x01]).unwrap();
        assert_eq!(&plain[6..32], &[0u8; 26]);
        assert_ne!(plain, frame);
    }

    /// The push that works today, reproduced byte for byte: 6-byte class-4 header, then the body.
    /// This is the frame `queuez_push` writes, and it is how orbit was reached.
    #[test]
    fn the_queuez_push_reproduces_the_shape_that_reached_orbit() {
        let mut conv = BapConversation::with_default_deadline();
        let frame = conv.push(crate::TYPE_QUEUEZ_OBJECT, &[0xDE, 0xAD]).unwrap();
        assert_eq!(frame, vec![0x00, 0x7B, 0, 0, 0, 1, 0xDE, 0xAD]);
        assert_eq!(frame, crate::queuez_push(1, &[0xDE, 0xAD]));
        // The counter is ours and advances independently of the client's.
        let next = conv.push(crate::TYPE_QUEUEZ_OBJECT, &[]).unwrap();
        assert_eq!(&next[2..6], &2u32.to_be_bytes());
    }

    // ── bookkeeping ───────────────────────────────────────────────────────────────────────────

    /// A ticket from another connection cannot answer this one's head. Sequences are per
    /// connection; crossing them would be the Weasel with a plausible-looking sequence number.
    #[test]
    fn a_ticket_from_another_conversation_is_refused() {
        let mut a = BapConversation::with_default_deadline();
        let mut b = BapConversation::with_default_deadline();
        let ta = ticket(&mut a, 1, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, t0());
        let _tb = ticket(&mut b, 1, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, t0());
        assert_eq!(b.park(ta, vec![]), Err(ParkError::ForeignTicket { sequence: 1 }));
    }

    /// A handler that produces the wrong response type is caught here rather than on the wire,
    /// where a type mismatch and a sequence mismatch are byte-identical in the client's log.
    #[test]
    fn a_wrong_response_type_is_caught_before_the_socket() {
        let mut conv = BapConversation::with_default_deadline();
        let _t = ticket(&mut conv, 2, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, t0());
        assert_eq!(
            conv.park_by_sequence(2, 0x000B, vec![]),
            Err(ParkError::WrongResponseType { sequence: 2, expected: 0x000D, got: 0x000B })
        );
    }

    /// The diagnostic a stalled run needs: which entry is the client blocked on, and for how long.
    #[test]
    fn outstanding_names_the_head_the_client_is_blocked_on() {
        let now = t0();
        let mut conv = BapConversation::with_default_deadline();
        let _a = ticket(&mut conv, 10, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now);
        let _b = ticket(&mut conv, 11, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST, now);
        assert_eq!(conv.head(), Some((10, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST)));
        assert_eq!(conv.pending_len(), 2);
        let later = now + Duration::from_millis(500);
        let rows: Vec<_> = conv.outstanding(later).collect();
        assert_eq!(rows[0].0, 10);
        assert!(rows[0].2 >= Duration::from_millis(500));
    }

    /// Our framing double-delivering a frame is our bug, and it must not silently create two
    /// entries for one client request.
    #[test]
    fn a_repeated_sequence_is_reported_not_queued_twice() {
        let now = t0();
        let mut conv = BapConversation::with_default_deadline();
        let _t = ticket(&mut conv, 6, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now);
        assert_eq!(
            conv.enqueue_at(6, TYPE_CLIENT_TO_WORLD_SERVER_REQUEST, now),
            EnqueueOutcome::DuplicateSequence(6)
        );
        assert_eq!(conv.pending_len(), 1);
    }
}
