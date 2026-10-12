# Server status - the whole answering estate

**Layer A (NP identity) works, and so does everything we have needed since: PSN OAuth, `/SignOn`,
the CDN content manifest, and the complete BAP handshake including an AES-128-GCM secured channel.**
As of 2026-08-06 the client is 12/14 boot gates deep, inside `bootflow:investment_signin`.

| crate | what it is |
|---|---|
| `shadnet-wire` / `shadnet-proto` / `shadnet-server` | **Layer A** - ShadNet NP identity, port 31313. Complete for its gate. **2026-08-08: the NP Matching2 room protocol (100-115) is now implemented and OFF BY DEFAULT** - see the block below. |
| **`d1-responder`** | the answering surface: PSN OAuth (TLS 443), `/SignOn`, the CDN manifest, shadNet WebAPI, `ticket_drop` (TLS 32556), and the **BAP TCP endpoint** on 3075. Everything unhandled becomes a *named gate*, never a hang. |
| **`d1-bap`** | **the single home for BAP wire code** (`CLAUDE.md`: one protocol, one implementer). Envelope, 8-byte class-2 reply headers, the sealed secure-hello, and mode-1 AES-128-GCM. 16 tests, including one that parses the real captured client frames. **2026-08-11: `src/derive.rs` + the `d1-bap` binary re-derive every `tables.rs` const from the pinned eboot** (port of `bap-message-idspace.py` / `bap-message-table.py`, audit item 9); `d1-bap controls` runs 18 controls incl. four drift checks (exit 2 on failure), and both Python TSVs golden-diffed byte-identical. |
| `d1-run` / `d1` | the run ledger + CLI + served GUI. Gate depth, boot ladder, guest halt text. |

⚠ **`f2.6` (bap_server_ip) is LITTLE-endian; `f14` (client_external_ip) is BIG-endian** - same
message, same type, opposite order. Regression-tested in `d1-responder`.

## 2026-08-11 - ✅✅ AH **udt 5 `sensor_auth_update`** SERVED - the lifetime-phase writer

The player-spawn gate `FUN_00414730` refuses to build the Guardian biped unless the **activity
lifetime phase** (`record+0x1AC` ∈ {3,6}) is set; today a `FUN_0010C500` hook fakes it. The server
now **delivers it on the wire**, so the hook can come off once the live watch confirms the dispatch.

| what | where |
|---|---|
| `SensorPhaseUpdate` + the byte-exact 59-byte frame assembler (value + name-ref through the client's own object codec, outer frame bit-assembled); **11 controls**, `d1-oracle sensor controls` exit 2 on fail | `d1-oracle::sensor` |
| `AhMessage::SensorAuthUpdate { payload }` (pre-built once by `SchemaCodec::build_sensor`, sent **verbatim** - it is a bespoke replicated frame, not a `CALL_SITES` struct), the `sensor_due` send-once trigger, the `D1_AH_SENSOR_*` knobs; **3 new controls → 111** | `d1-activity-host` |

⭐ **The frame** (472 bits, MSB-first): `gen(8) · has_sensor_block(1)=0 · stamp(32) · init(1) ·
has_entry(1) · name-ref(55, 0x808004B1) · body_bit_length(32)=`**2+value_bits**` · fresh(1) ·
presence(1) · value(339, 0x808008A3) · terminator(1)=0`. Phase = a **4-bit bias-1** enum in the
value (`raw = phase+1`). Chain: udt 5 → `FUN_0029D3F0` → sim-queue element `0x13` → `FUN_002A1210`
→ dispatch `record+0xA8` `{0x8080039D, ord 2}` = **`FUN_0011DE30`** → `memcpy(record+0x1AC, image,
0x504)`. Executed end to end offline on `rung36-biped-chain.snap` (setter store-boundary probe
`rdi=record+0x1AC rsi=image rdx=0x504`; getter `FUN_0010C500` al=1 state=3/6; gate mask `0x48`).

**Knobs:** `D1_AH_SENSOR_PHASE=3` arms it (off by default). `D1_AH_SENSOR_GEN` (must equal the
client's `world+0x1D` or the applier silently skips), `D1_AH_SENSOR_NAME_HASH`/`_NAME_SUB`/`_NAME_I8`
(per-load, default the measured in-world pvp_moonbase group 0), `D1_AH_SENSOR_TAIL_ID`/`_TAIL_HASH`
(also set the `FUN_0010CB60` spawn preconditions `record+0x674`/`+0x678`). Sent **once**, after the
instance is Live **and** the first heartbeat has fired (so the world is resident).

⚠ **`synthetic-minimal`, promotion gate `[U]`:** whether our fresh wire element's slot is in the
world post-pass sweep is unproven offline (the target pages are not captured); the frame is
byte-exact and the guard passes today, but "reachable ≠ traversed." Settle it with the live watch on
`record+0x1AC` (identity payload `D1_AH_SENSOR_PHASE=-1` as the null control). ⇒
`design/activity-lifetime-state.md` top banner. **Retracts** that doc's udt-7 / `FUN_0010BC20`
conclusions: udt 7 hardcodes the wrong (map-wide-unbound) slot `+0x108`, and the gate's record-3
setter is `FUN_0011DE30`, not `FUN_0010BC20` (that is record 6's).

## 2026-08-11 - the QueueZ push send line now names the family

**Finding:** the family-3/4 push (`D1_PUSH_FAMILY`, `D1_PUSH_AT`) has fired in 186 of 270 runs and
was misreported as "never fired" because its send line read `--> BAP QUEUEZ CHARACTER`, while the
census grepped the ANSWER-path label `--> BAP FAMILY n`. Full retraction and live-client proof of
delivery, acceptance and residency: `design/geared-visuals-run-1.md` §7.

**Change (`d1-responder`):** the send line is now `--> BAP QUEUEZ PUSH <n>B family <f>` (shape kept
compatible with `send-receive-ledger.py`'s `PUSH_WRITE` regex, self-test PASS). Rule for any future
family-delivery census: match BOTH `--> BAP FAMILY n ANSWER` and `--> BAP QUEUEZ PUSH .* family n`.

## 2026-08-09 - ⭐⭐ THE ACTIVITY HOST WAS KILLING ITS OWN LONG RUNS

**`d1-oracle`'s Unicorn arena is a bump pointer whose `free` is a no-op, so every encode leaked its
whole working set - 131 `global_activity_state` pushes and the process is dead.** Run
`20260809-120451-MOONBASE-FRESH` hit it **13 minutes** into an activity; the host went silent and
the client reported `error.private_activity_host_host_crashed`, which reads as *the client rejected
us*. ⇒ **`historicaldocs/design/oracle-scratch-heap.md`.**

| change | where |
|---|---|
| `Harness::{heap_mark, heap_rewind, heap_high_water, guest_allocs}`; the exhaustion error now names the fix | `d1-oracle::harness` |
| `SchemaHarness::begin_call()` - rewind to the post-scaffolding mark, first line of all four `ObjectOracle` entry points | `d1-oracle::{schema,object}` |
| `ArenaStats`, `leak_control`, `activity-host leak` / `activity-host arena` subcommands; controls **28 → 34** | `d1-oracle::activity_host` |
| `AhEvent::CodecFault` at **ERROR**, `AhError::is_host_defect()`, both log paths through one `log_event()`, arena table in the startup log; controls **70 → 75** | `d1-activity-host` |

⭐ **The control ships its own inversion and it was run.** With the rewind reverted,
`d1-activity-host controls` goes **73/75** and `d1-oracle activity-host leak` goes red; with it in
place, **10,000 cycles × 5 udts = 50,000 encodes (14.2 simulated hours) move the high-water mark by
zero bytes**, guest allocations 0, bytes byte-for-byte identical.

⚠ **Only `MOONBASE-FRESH` was truncated** (6 faults). `20260809-084932-RUNG33-PERMS6` reached **107
of the 131** budget over 3.2 hours without faulting - the budget is **encode count, not wall clock**.
Anything that run observed after `20:19:55`, including rung 34's apparent indefinite park, is a
client-side timeout cascade and **not evidence about rung 34**.

## 2026-08-09 - ⭐⭐ THE ACTIVITY-HOST PUSH RODE THE WRONG BAP MESSAGE

**`d1-activity-host` now pushes BAP 9 `bap_to_client_notification`, not BAP 100.** Message 100 is
the Activity-Host → **BAP-server** hop: its push policy's deliver step (`0x564C20`) reads the
class-6 header's `+0x06` u64 as a **connection index** and re-emits the identical body as message 9
on that connection. We left `+0x06` at zero - index 0 is `OUT PRIMARY` - so in run
`20260808-234535-CODEC-LIVE2` the client forwarded all four establish-client pushes straight back to
`d1-responder`, 1:1 and byte for byte, and `c_activity_client::receive_message_from_host` never saw
one of them. The status was **26 `timed_out_waiting_for_join_response`**, not 28, so
`FUN_00298EF0` was never evaluated.

| change | where |
|---|---|
| `OURS_TO_PUSH` = `[9, 100, 123]`; `BapToClientNotification` (one codec, two ids); `ah_to_client_udt_name`; a **pinned live message-9 frame** test with a header-offset negative | `d1-bap` |
| `D1_AH_BAP_PUSH_ID` (default **9**; `100` reproduces the failure), `emit_as`, startup line naming the id | `d1-activity-host` |
| inner type `0x0009` decoded: `{recipient_id, udt (named), payload_len}` + first 32 payload bytes + the relay diagnosis. It was `** NAMED GATE **`d 18× per run | `d1-responder` |
| 3 new controls → **42**, incl. a cross-crate drift control on the 17 AH→client udt names | `d1-activity-host controls` |

⭐ `recipient_id` **must be the activity-host id** - `FUN_0075AB60` compares it with `[ac+0xC98]`
(the established AH id) or, for udt 4 only, the pending join request's. Never an account SOID, so
`d1-responder`'s `D1_AH_RECIPIENT` default is wrong (only reachable under `D1_AH_PUSH_100`).
⚠ And **order is mandatory at the router**: udt 0/1/11 are silently unroutable until udt 4 lands.

Full chain, eight functions, with the run evidence: **`historicaldocs/design/establish-client-gate.md`**.

## 2026-08-08 - ⭐ THE CHARACTER LIFECYCLE, and the bug that meant it could never fire

**The four character-lifecycle investment messages are resolved and implemented, default OFF.**

| id | name | state |
|---:|---|---|
| **501** | `login_create_character` | ✅ new - decodes the request, **writes a transcript**, mints a SOID |
| **502** | `login_delete_character` | ✅ new - acknowledged |
| **504** | `login_character` | ✅ already, since 2026-08-07 |
| **505** | `signout_character` | ✅ new - acknowledged |
| 506 | `customize_character` | ✅ new - acknowledged |

Ids from the **FNV-1 name hash** in the shipped definition table, confirmed independently by the
literal immediates in the senders. ⛔ **This retracts "501 is `login_character`"** in three live
documents - 501 is CREATE, 504 is LOGIN. Full write-up, both named structs resolved, and the
falsifiable prediction: **`historicaldocs/design/character-lifecycle-messages.md`**.

⭐⭐ **The bug that mattered more than the handlers.** `d1-responder` built the account's character
list as `&[chr]` unconditionally, so `D1_CHARACTER_SOID=0` served **one character whose SOID is 0**,
not an empty list - the "serve zero characters" logic existed as the `unused variable: char_list`
warning. The client reads `account + 0x08` as the count (`0xA110F0`), so it has been told a Guardian
already exists on **every run this project has ever done**, and therefore has never once asked us to
create one. Confirmed at the client's own codec, and pinned by
`an_empty_character_list_encodes_count_zero` with the negative control *"a list of [0] is NOT an
empty list."* Census of `runs/20260808-223926-CHARMSG-LISTEN`: zero of the four ever fired.

Enable the whole thing with one line - ⚠ **both halves**, the flag alone changes nothing:

```sh
D1_CHARACTER_LIFECYCLE=1 D1_CHARACTER_SOID=0
```

Transcript lands at `$RUN_OUT/create-character-<unix>.txt` + `.bin` (raw bitstream, byte-exact).

⚠ Default behaviour is unchanged: `run-boot.sh` pins a non-zero `D1_CHARACTER_SOID`, and with the
flag off an arriving 501/502/505 prints `** NAMED GATE **` and is not answered - exactly the old
behaviour, which is what makes the A/B honest.

## 2026-08-08 - Layer A now speaks the Matching2 room protocol, and it is **unarmed**

`shadnet-server` dispatched 4 of 40 commands. It now dispatches **20**: the four it always did, plus
all sixteen of the matchmaking block (100-115) - contexts, create/join/leave/search, member tracking
(`member_id`/`team_id`/`is_owner`/`addr`/`port`/`nat_type`), room data internal+external, kickout,
room messages, `RequestSignalingInfos`, and the **notifications** that tell the other participant
anything happened. `encode_notification` had **zero callers in the workspace**; the socket is now
split with a per-connection `mpsc` outbox, so the server can finally push.

⚠⚠ **Nothing asked for this.** `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §0/§3 marks it **contingent**:
no game-side code reaches the `sceNpMatching2` import library, D1's managed session is the PSN
Session WebAPI over HTTP, and its peer key material is minted **locally** by the client's own
MT19937. This is the implemented branch, not a satisfied gate. **`matching2_enabled` stays `false`
and the room commands stay off** - default behaviour is byte-identical to before, including the
`Unsupported(33)` answer to `ContextStart`. `--rooms on` serves them; `--matching2` advertises and
serves. §9.1 (`grep Lib_NpMatching2 <run>.log`, positive control `Lib_NpWebApi`) retires it formally.

- **No session keys are issued, and a control enforces that.** C7's verdict: `bdSecurityKeyMap::
  initKey` (`0xF875A0`) draws 8 + 16 bytes from Demonware's MT19937, takes no input and always
  returns true. Inventing a key scheme here would work only against itself.
- **Two identities never conflate.** Rooms key on our own `conn_id`, never the npid string; a second
  `Login` for a live npid is refused with `LoginAlreadyLoggedIn(6)`; `user_id` is per-npid. The
  **first** participant still gets the configured identity verbatim, so single-instance runs do not
  move. `--identity-mode per-client` gives each its own npid/token.
- **The emitter refuses what the client would mis-handle silently**: `details: None` (null-pointer
  callback with `errorCode 0`), a member with no `addr`/`port` (5-second reader-thread stall), a
  notification without the *recipient's* `ctx_id` (vanishes with no log line), and the four room
  events shadPS4's `HandleRoomEvent` drops on its `default:` arm.
- ⚠ **Two instruments disagree about a member's port, and neither is authoritative.** Nothing on the
  wire carries it and shadPS4's `GetP2PConfiguredPort()` returns **0** at HEAD; `--peer-port`
  defaults to **3074**, the UDP port D1's guest is *observed* to bind. Recorded, not reconciled.
- ⚠ **`RoomPasswordMismatch(17)` is unenforceable**, because `JoinRoomRequest` carries no password
  bytes - only a bool. We refuse a joiner who presents nothing (`RoomPasswordMissing(18)`) and never
  claim a mismatch we did not perform. A control asserts we never emit 17.
- `shadnet-server controls` = **33 checks, exit 2 on failure**, checked against *the client's own
  text* (a verbatim enum excerpt at `proto/shadps4-client-constants.h`, re-derived against the
  shadPS4 tree when present) and *the vendored schema* (parsed for field numbers; sha256 re-derived,
  never transcribed) - not against our own constants. **CLI stayed backwards compatible**: `serve`
  is the default subcommand, so `capture-rig/*.sh` needed no edit.

## ⭐⭐ 2026-08-08 - the correlation table is wired, and the rung 28-30 routes exist

**No handler in `d1-responder` writes a response header any more.** Every secured reply is
`enqueue` → handler → `park` → `drain`, one `d1_bap::BapConversation` per BAP TCP connection.
`drain` pops the **front only** and is the single secured emission point. That is the other half
of the Weasel fix: both rung-28 runs died on message ids **11** and **13**, *both of which this
responder already answered correctly* - the bug was never the bytes, it was the order.

- `D1_BAP_FIFO=1` (default) - the correlation table. **`0` restores the pre-2026-08-08
  immediate-write behaviour**, which is the only way to reproduce the Weasel as an A/B.
- A `250 ms` tick calls `drain` even with no inbound frame, so **R2 is a mechanism, not a comment**:
  a head older than `D1_BAP_DEADLINE_MS` (2000, `[U]` guess) gets a synthesised **500** instead of
  silence. ⚠ That changes what `D1_SUB_ACK=0` / `D1_ACK_PROFILE_SETUP=0` / `D1_INV_SILENT` mean -
  they now produce a 500 at the deadline. Use `D1_BAP_FIFO=0` for true silence.
- New routes: **6 → 7** (`D1_AH_MANAGER`), **16 → 17** (`D1_AH_PROXY`), **42 → 43**
  (`D1_MM_CONFIG`), message **8** decoded and deliberately unanswered.
- ⭐⭐ **2026-08-08: message 100 is UNBLOCKED.** Header classes **3 (14 B), 5 (24 B) and 6 (32 B)**
  are read and implemented in `d1-bap` (`HeaderExtras`, `push_with_header`), so
  `D1_AH_PUSH_100=1` now puts a real frame on the wire - this is the Activity Host's only way to
  answer a join request. Class 3 is confirmed **`observed-live`** against the client's own
  message-8 header. ⇒ `historicaldocs/design/bap-header-classes.md`. **102 stays refused** - it is off the
  client's 13-entry accept-set, which is a property of the client, not a gap in ours.
  ⚠ The remaining risk moved from framing to **addressing**, whose failure mode is a *silent drop*.
- ⛔ **The server cannot set a session parameter.** Our only two levers for the activity host are
  message 7 and message 17; everything between them is the client talking to its own peers.
- ⭐ **Free falsifier, now logged in place:** if message **16** never follows our message 7, the
  `[I]` link in `historicaldocs/design/SERVER-ROLES-PLAN.md` §4.2 is wrong.

Every `D1_*` knob is printed at startup (`print_knobs`), with a test asserting the table covers
every knob the source reads.

Original Layer A notes follow.

## What's going on

- `d1-server/` is a Rust cargo workspace, builds clean, tests green. It is **Layer A only** -
  Layers B (Bungie shim) and C (transport) are not started and are not reachable yet.
- It grants a **fixed synthetic identity** and then **logs everything the game asks for next**. The
  per-run gate ledger is the deliverable; the login is just the price of admission.
- Confirmed working end to end [C]: Destiny logged in, and its own `LoginRequest` told us what it
  thinks it is - **`title_id=CUSA00219`, `title_name=Destiny`**.
  ```
  ProbeServer: server 127.0.0.1:31313 is reachable (protocol v1)
  ServerInfo OK (protocol v1)
  HandleLoginReply: Logged in npid='D1Guardian' userId=1
  HandleGetTokenReply: Bearer token captured (14 chars) for accountID=1
  ```
  Session ledger: `Login` ×1 served · `GetToken` ×1 served · `GetServerFeatures` ×1 served ·
  **zero unimplemented commands requested**. For the identity gate, Layer A is *complete* - there is
  no ShadNet backlog to work through.
- **Next gate, immediately after identity opened** [C]: **16 `sceNetResolverStartNtoa` calls to 16
  distinct names** - `stun1-8.signon.deadorbit.net` and `stun1-8.signon.gravityshavings.net` - plus
  2 `sceHttpsLoadCert`. Confirmed on a clean single-instance run with host-condition provenance
  (`runs/20260806-110731-stun-clean/`: 0 faults, 0 `SIGNED_OUT`), matching the pinned public 01.33
  trace's 16 STUN attempts. The earlier double-instance measurement is superseded, not relied on.
  The run *before* identity opened made **zero** resolutions, which establishes the causal order:
  **identity first, STUN second** - not present in any *Bungie* source, but very likely documented in
  **Demonware** RE work. (Corrected 2026-08-06: an earlier version claimed "not in any public source",
  which overstated our novelty and would discourage a reader from looking where the answers are.
  D1's auth/lobby are Demonware - see `historicaldocs/design/networking-architecture.md`.)

> **✅ RESOLVED: STUN is NOT the gate - `signon` is.** Confirmed from the eboot's own diagnostic
> strings: `signon` is a server-delivered bootstrap returning `client_config`, environment, content
> build version, silo id, clock offset, **the client's external address**, and **the BAP server
> address**. Everything downstream (BAP → investment/online/matchmaking) depends on it. And the
> redirect seam is **Bungie's own supported feature** - `sku_config` accepts *signon URL overrides*,
> and that file (`sku_config.txt`, 440 bytes, obfuscated) is **already in our dump**. STUN turns out to
> be the least load-bearing of three redundant paths for the external address. Full chain:
> `historicaldocs/design/THE-BOOTSTRAP-HYPOTHESIS.md`. Corpus: `historicaldocs/design/networking-log-strings.md` (33 subsystems).
>
> Superseded note kept for the record: The eboot contains an explicit fallback -
> *"Failing back to using our local address … and defaulted to moderate NAT type"* - plus a path that
> skips STUN entirely given a valid cached external IP/NAT type. So the client may tolerate all 16
> failures and be blocked further downstream. **Before standing up coturn, get Destiny's own
> diagnostic channel flushed** (it narrates to stdout via shadPS4's `[Tty]` logger; 1,291
> `networking:*` strings exist, but only 2 lines surfaced in our clean run) and find out which path it
> took. Answering a non-gate would look like progress. See `historicaldocs/design/DEMONWARE-FINDING.md`.

## Layout

| Crate | Job |
|---|---|
| `crates/shadnet-wire` | Framing: header layout, `Command`/`Notification`/`ErrorType` enums, codec. Mirrors shadPS4 `src/shadnet/client.{h,cpp}` - **drift here is a silent wire break.** |
| `crates/shadnet-proto` | Types generated from the vendored `proto/shadnet.proto` - compiled, not hand-rolled, so our encoding cannot drift from the client's. |
| `crates/shadnet-server` | The service: session loop, handlers, identity fixture, gate ledger. |

Provenance: `proto/shadnet.proto` sha256 `61f4fef46c15b81c273ea0c29f5f7fb303542cf98d6fcbcbe7f0329594edac87`,
from shadPS4 upstream `8bdec718`. The schema is shadPS4's own (GPL-2.0-or-later, derived from rpcsn).

## The wire format (the part worth not re-deriving)

15-byte header, little-endian throughout:

```
offset size field
  0     1   PacketType   0=Request 1=Reply 2=Notification 3=ServerInfo
  1     2   CommandType
  3     4   total length, INCLUDING these 15 header bytes
  7     8   packet id - echo it back on the matching reply
 15    ..   payload
```

- **Request** payload: `u32` LE blob length, then protobuf.
- **Reply** payload: one `ErrorType` **byte**, then `u32` LE blob length, then protobuf.
- **`ServerInfo` is the exception and the trap.** The **server must speak first** - shadPS4 blocks on
  it immediately after `connect()` and drops the socket if it never arrives, so a server that waits
  to be spoken to never receives a `Login` at all. Its payload is a **bare `u32` LE protocol version**
  - *not* length-prefixed, *not* protobuf. Mismatch against the client's `SHAD_PROTOCOL_VERSION`
  (currently 1) is a disconnect. **This cost us a debugging cycle**; it's encoded as a test now.
- shadPS4 rejects a `Login` reply whose proto blob is **empty** as malformed, so a successful
  `LoginReply` must carry at least one populated field. Also a test.

## Running it

```bash
# usually you want the launcher, which starts the server and the emulator in one contained namespace:
capture-rig/run-contained-emulator.sh dumps/patch-01.29-CUSA00219/eboot.bin <run-name>

# standalone:
d1-server/target/release/shadnet-server --bind 127.0.0.1:31313

# its controls - 33 checks, exit 2 means the output may not be cited
d1-server/target/release/shadnet-server controls

# the room protocol, which is OFF unless you ask for it:
#   --rooms on          serve 100-115 while still advertising matching2_enabled=false
#                       (the only configuration in which a room command arriving is EVIDENCE)
#   --matching2         advertise it AND serve it, and bind 31314/UDP for the registration ping
#   --identity-mode per-client   each participant gets its own npid/token (two-instance work)
#   --peer-port 3074    host-order UDP port published for every member  ⚠ see the caveat above
```

Client side must be configured to match - `~/.local/share/shadPS4/`:

| File | Key | Value |
|---|---|---|
| `config.json` | `shad_net_enabled` | `true` |
| `config.json` | `shadnet_server` | `127.0.0.1:31313` |
| `config.json` | `connected_to_network` | `true` |
| `users.json` (user 1000) | `shadnet_enabled` | `true` |
| `users.json` (user 1000) | `shadnet_npid` / `shadnet_password` | `D1Guardian` / `d1-local` |

`IsPsnSignedIn` requires **both** `shad_net_enabled` **and** an authenticated ShadNet client, and
`ConnectUserById` silently skips a user whose npid or password is empty - so all five rows matter.

## Design decisions worth keeping

- **Identity is a fixed local fixture**, deliberately boring: a preservation lab wants the *same*
  identity every run so traces are comparable. It mirrors the console's fixed `1111111111111111`
  fake-activated account. Nothing is derived from Sony material.
- **`matching2_enabled` defaults to OFF** - *still*, now that the room protocol behind it exists.
  The public 01.33 trace reaches the title screen with no pre-gate Matching2 traffic, and the
  2026-08-08 recon found no game-side code reaching the import library at all. Implementing a thing
  is not evidence that anything wants it. Turn it on when something demonstrably requests it, and
  record what asked in the ledger first. ⛔ Note the trap: with the flag **false** shadPS4 answers
  `GetWorldInfoList` *itself*, so the flag-on and flag-off configurations are not comparable.
- **Unimplemented commands get a clean `Unsupported`, loudly logged**, never a plausible lie. A
  definite refusal gives the client something to act on and gives us a *named* gate.
- **The ledger is deterministic** (`BTreeMap`, not hash order), so two runs of the same experiment
  produce byte-identical summaries and a diff means something really changed.
- **Loopback bind, no outbound connections, ever.** The server warns if pointed at a non-loopback
  address.

## Next

- [x] ~~Clean single-instance re-run to confirm the 16-STUN result~~ - **done, `[C]`**
- [ ] **Answer the STUN gate**: coturn + a logging resolver inside the sealed namespace, sixteen
      names pointed at it. Does Destiny accept a generic RFC 5389 binding response, or validate
      something Bungie-specific? See `historicaldocs/design/observability-tooling.md` §2.
- [ ] **Read the Aldridge + Truman talks before designing anything that answers STUN** - they define
      what to look for, so they precede the design.
- [ ] Identify what the 2 `sceHttpsLoadCert` calls are for, now that the game gets that far
- [ ] Structured (JSON) run output so ledgers are machine-diffable, not grep-able
- [ ] Layers B and C: not started, and **not designable until we know what the STUN hosts owe the
      client.** Don't speculate ahead of the ledger.
