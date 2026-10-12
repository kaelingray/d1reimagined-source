# The Activity Host - the service, the 49-message protocol, and the codec

**State.** The Activity Host (AH) is a *service*, not a peer: BAP starts it from a process command
line, and the client reaches it over its own dedicated BAP connection. All 49 message type ids are
named; 30 have pinned schema tags; the four establish-client messages (`4, 0, 1, 11`) have been
emitted by `d1-activity-host` and **accepted by a live client** (`observed-live`, run
`20260809-054417-RUNG33-BYPASS`, `EST-Y AH->D1A00000:00000001 MEM-1`). Everything the server emits
remains `synthetic_ancestor`. The live frontier (udt 5/6/7 sensor traffic, the lifetime phase byte)
is `design/activity-lifetime-state.md`; this doc is the service reference behind it.

Shorthand for provenance: **contract §N** = `historicaldocs/design/activity-host-contract.md` §N,
**codec §N** = `historicaldocs/design/ah-schema-codec.md` §N.

## Contents

1. [What the Activity Host is](#1-what-the-activity-host-is)
2. [The message table - all 49 types](#2-the-message-table---all-49-types)
3. [The lifecycle](#3-the-lifecycle)
4. [join_result - the 0x1A8 layout](#4-join_result---the-0x1a8-layout)
5. [Entity indices, reservations, keepalive and the lease](#5-entity-indices-reservations-keepalive-and-the-lease)
6. [The codec - FUN_002A4090 / FUN_002A4140](#6-the-codec---fun_002a4090--fun_002a4140)
7. [Open [U]s](#7-open-us)
8. [Dead claims](#8-dead-claims)
9. [Sources](#9-sources)

---

## 1. What the Activity Host is

**`ipahp` is resolved: the Activity Host is a SERVICE, and `AHP` = activity host *proxy*.** `[C]`
The session host asks BAP to **start** an Activity Host by sending it a **process command line**
(`/ah_type:group|fireteam /ah_package:... /ah_slice_set_names:...`). You do not send argv to another
player's PS4. This closed the earlier host-authority conflict: the fireteam/posse host *is* a peer
role (`c_world_controller_state_setup_activity_host_setup_handler::vfunc[7]` at `0x7A5250`, "As
HOST, waiting for peers to be done with setup"), but the Activity Host itself is infrastructure.
(contract headline, §3.1)

⛔ **`0x783E90` does no transport work.** It is a static-initialiser string table: 1,057
instructions, **zero calls, zero jumps**, referenced only from a ctor pointer array at `0x1A034A0`.
Earlier work treated it as the ipahp startup state machine; it is a table of names. (contract
headline)

**Transport `[C]`** (details of the BAP layer itself: `design/bap-transport.md`):

* Send: `0x299310` names itself `activity_client_send_message` via its own allocation tag -
  `send(this, ah_id, u32 type, schema_descriptor, payload, len, peer_bitmask @ this+0x6C6C)`. It
  serialises through the table-driven schema codec (`0x2A4140`) unless the raw flag is set, uses a
  0x1DF00-byte scratch buffer (max payload 122,624 B), and stamps `this+0xE48` with the send time
  on success. (contract §1)
* Receive: `c_activity_client::vfunc[9]` (`0x299430`) linear-scans a 17-entry dispatch table at
  `0x18F84A0`, `{i32 type; i32 pad; fnptr}`. ⚠ Each entry **stores its own type id** and the table
  is **unsorted** (in table order: `0, 1, 2, 4, 11, 10, 5, 7, 18, 23, 24, 27, 29, 38, 39, 42, 43`).
  Never derive a positional index mapping from it; doing so produced a dead claim (§8). (contract
  §1, codec §11.2)
* ⭐ The AH lives on its **own** `networking:server:bap` connection - slot 1 `OUT FAH` or slots 2/3
  `OUT GAH0`/`GAH1`, distinct from slot 0 `OUT PRIMARY`. An Activity Host is a second BAP endpoint,
  not traffic multiplexed onto the existing one. (contract §1)
* ⭐⭐ **The AH -> client push is BAP id 9 `bap_to_client_notification`.** BAP 100
  `activity_host_to_client_notification` is the AH -> BAP-server hop: its push policy's deliver
  step (`0x564C20`) reads the class-6 header's `+0x06` as a connection index and re-emits the body
  as BAP 9 on that connection. Pushing 100 made the client echo every push back, byte for byte
  (§8, dead claim 1). (codec banner)
* Every frame the AH writes must be **sealed in the 6-byte BAP envelope** `{u8 version = 1;
  u8 kind; u32be len}` - confirmed from the client's own parser `FUN_00B2D9F0`. An unenveloped
  inner frame begins `00 09`, which the client reads as envelope version 0 and kills the
  connection (`_connection_failure_unknown_message_type`). (codec §13.2, §13.1)

**Failure vocabulary `[C]`:** `error.private_activity_host_*` (30) and
`error.starting_private_activity_instance_*` (30) share one suffix set, plus
`error.booted_from_private_activity_host_*` (11) and `error.private_activity_instance_*` (2).
⭐ A 77-reason `error.sstm_public_{current,target}_activity_host_*` mirror proves the **public
matchmade path runs the identical AH state machine** - whatever is built for a private match is the
same machine the public game used. (contract §10)

Reach for everything static below: `static-client-inference` on the pinned 01.29 eboot
(`7271fcb9...`), licensed by `d1-eboot controls` 22/22 PASS. Run `d1-eboot controls` /
`d1-oracle activity-host controls` / `d1-activity-host controls` before citing; exit 2 means the
output may not be cited.

## 2. The message table - all 49 types

**Names `[C]`:** a pointer array at `0x18F8780`, 49 entries, **index == wire type id**, NULL at 49.
A richer `{slot, len, name}` table at `0x1A36210` (stride 0x18) agrees. Positive control: seven
handlers were matched to their own diagnostic strings independently of the table, 7 of 7 agree.
(contract §2)

⚠ **Sizes are the client's in-memory STRUCT sizes, not wire sizes.** Every schema-coded message
travels through the codec of §6; the wire is a bit-packed encoding of the struct (e.g. udt 3:
struct 0x100, wire 238 B). (contract §4 banner)

⚠ The name tables enumerate only **44 distinct ids**. Ids **9, 16, 34, 40** are unwired in both
directions (no dispatch entry, no send site); id **21** was on that list until its send site was
found outside `FUN_00299310` (codec §12.3). Id 18 `incident` appears in both directions with one
schema, which is why 17 + 28 named rows cover 44 ids. (contract §2 corrections)

Machine-readable form with controls: `d1-activity-host ids` (all 49 with direction),
`d1-oracle activity-host layout` (all pinned tags and their derivation chains).

| id | name | dir | struct size | schema tag | handler / send site | notes |
|---:|---|---|---|---|---|---|
| 0 | `entity_slots_allocated` | AH->c | 0x600 | `0x80800D64` | `0x10579F0` | the 12,288-bit entity-index grant (§5) |
| 1 | `global_activity_state` | AH->c | 0x6B0 | `0x80800E76` | `0x1057B80` | the 1,712 B activity state blob; `u16 activity_index @+0x680`, `char[0x28]` name `@+0x688` (see `design/activity-launch.md`) |
| 2 | `world_globals_state` | AH->c | `[U]` | unpinned | `0x1057EF0` | |
| 3 | `join_request` | c->AH | 0x100 | `0x80800E98` | send `FUN_0029A170` | wire 238 B, `observed-live` capture |
| 4 | `join_result` | AH->c | 0x1A8 | `0x80800A58` | `0x1058020` -> `0x29BD10` | §4; our encoding is 398 B / 3,178 bits |
| 5 | `sensor_auth_update` | AH->c | `[U]` | unpinned | `0x29D3F0` | Auth State; frontier - `design/activity-lifetime-state.md` |
| 6 | `sensor_sense_update` | c->AH | caller's | **raw, no tag** | `c_activity_client::vfunc[4]` `0x2992B0` | raw arm of `FUN_00299310` (§6) |
| 7 | `sensor_message` | AH->c | `[U]` | **not this codec** | `0x29CDE0` | decodes via `FUN_002D2090`, FNV-1a basis `0x811C9DC5`; frontier - `design/activity-lifetime-state.md` |
| 8 | `request_activity_host` | c->manager | 0x9D8 (raw site) | **raw, no tag** | `FUN_002996D0` | a process command line; carried as BAP id 6; ⚠ size conflict, see below |
| 9 | `start_activity_host` | unwired | - | - | - | no dispatch entry, no send site `[C]` |
| 10 | `start_activity_host_response` | AH->c | 0x88 | unpinned | `0x1058150` -> `0x29C260` | `{u64 ah_id; char session_id[]; ...}`; §3 |
| 11 | `replicate_membership` | AH->c | 0x5038 | `0x808009ED` | `0x29CF50` -> `0x29C730` | seeded decode + leading presence bit (§6.3) |
| 12 | `request_peer_reservation` | c->AH | 0x330 | `0x80800BB9` | driver `0x758E40` | §5.2 |
| 13 | `release_peer_reservation` | c->AH | 0x10 | `0x80800A1D` | | `{u32; u32 membership_id; u64 machine_id}` |
| 14 | `peer_leave_request` | c->AH | 0x14 | `0x80800E84` | `FUN_0029A610` | carries a leave-reason FNV-1 hash (§5.3) |
| 15 | `activity_client_keepalive_request` | c->AH | 1 | **raw, no tag** | `FUN_00298C40` | one uninitialised byte; descriptor arg `0xFFFFFFFF` (§5.3, §6) |
| 16 | `activity_client_keepalive_response` | unwired | - | - | - | ⭐ **never send** - no client handler, the drop is silent |
| 17 | `activity_client_request_state_refresh` | c->AH | 8 | `0x808009E7` | `0x29AB80` stack | `{u32 membership counter; u32 FUN_00344BB0(...)}` |
| 18 | `incident` | both | 0x2CC | `0x8080097D` | rx `0x1058180` | identical schema both ways (`DAT_019022D0` on both sides) |
| 19 | `allocate_entity_indices` | c->AH | 4 | `0x80800D21` | | `u32 count`; sets in-flight flag `ac+0x64A9` |
| 20 | `free_entity_indices` | c->AH | 0x600 | `0x80800E22` | | same 0x600 mask shape as udt 0 |
| 21 | `client_authoritative_data_update` | c->AH | 0x220 | `0x80800C56` | `FUN_0029CC20` | ⭐ own encoder `FUN_002C9290` + `messaging->vfunc[6]` direct, not `FUN_00299310`; fire-and-forget, no reply exists |
| 22 | `client_identity_update` | c->AH | 0x28 | `0x80800C2D` | | same struct as the identity embedded in `join_request+0x10` |
| 23 | `claim_authority_over_abandoned_entity_slots` | AH->c | `[U]` (shape `{u32; u8 mask[0x600]}`) | unpinned | `0x10582E0` | |
| 24 | `purge_abandoned_entity_slots` | AH->c | `[U]` | unpinned | `0x1058460` | |
| 25 | `abandon_entity_slots` | c->AH | 0x608 | `0x80800E31` | | |
| 26 | `request_purge_entity_slots` | c->AH | 0x604 | `0x80800CB8` | | |
| 27 | `reset_entity_slot_authority_mask` | AH->c | `[U]` | unpinned | `0x10585D0` | |
| 28 | `reset_mask_ack` (source's short name) | c->AH | 4 | `0x80800C9C` | | |
| 29 | `query_entity_slot_authority_mask` | AH->c | `[U]` | unpinned | `0x10586E0` | |
| 30 | `query_entity_slot_authority_mask_per_bubble_response` | c->AH | 0x608 | `0x80800CD2` | | ⚠ name per the re-dump; earlier pairing was swapped (§8) |
| 31 | `query_entity_slot_authority_mask_response` | c->AH | 0x604 | `0x80800CF6` | | ⚠ ditto |
| 32 | `abdicate_authority` | c->AH | 0x604 | `0x80800A22` | | |
| 33 | `process_debug_command` | c->AH | 0x210 | `0x80800A3D` | | dynamic-flags descriptor (kind 0x22); undecodable in a static harness (§6.4) |
| 34 | `debug_request_migration` | unwired | - | - | - | |
| 35 | `connectivity_failure` | c->AH | 8 | `0x80800A17` | | |
| 36 | `membership_acknowledgement` | c->AH | 4 | `0x80800BD0` | | acks udt 11 |
| 37 | `send_client_heartbeat` | c->AH | 0xB1E0 (45,536 B) | `0x80800D06` | callers `0x7ADE00`, `0x7B09D0` | ⛔ NOT the lease - a bulk world/time-of-day state upload (`c_time_of_day_game_system`); dynamic-flags descriptor (§6.4) |
| 38 | `script_state` | AH->c | `[U]` (bit-packed <= 8,000 B) | unpinned | `0x29D450` | Mission Script |
| 39 | `script_event` | AH->c | `[U]` | unpinned | `0x10587F0` | Mission Script |
| 40 | `request_script_updates` | unwired | - | - | - | |
| 41 | `bug_claw` | c->AH | 4 | `0x80800CAE` | | |
| 42 | `advance_replication_epoch` | AH->c | wire 2 B `{u8 epoch; u8 suppress_flag}` | unpinned | `0x1058930` | §4 |
| 43 | `reservations_failed` | AH->c | `[U]` (shape `{i32 count; u64 machine_ids[]}`) | unpinned | `0x1058A80` | marks `peer_table[i]+8 = 1` |
| 44 | `report_lag_switch` | c->AH | 0x284 | `0x80800F1F` | | |
| 45 | `connection_quality_report` | c->AH | 0x170 | `0x80800F23` | | |
| 46 | `speculative_migration` | c->AH | 0xC | `0x80800F6C` | | |
| 47 | `high_water` | c->AH | 0x24 | `0x80800F7D` | tick `0x298770` | sent every 59,999 ms - the only literal ms constant on the path |
| 48 | `refresh_inspirations` | c->AH | 4 | `0x808002D9` | | |

Binary spellings: ids 15/16/17 are `activity_client_keepalive_request` / `_response` /
`activity_client_request_state_refresh`. (contract §2 corrections)

⚠ **Unresolved size conflict on id 8.** Contract §3.1 states `request_activity_host` is "0x100F
bytes containing a process command line" carried as `client_to_activity_host_manager_request`
(BAP id 6, policy 2); codec §12.2's raw-send table reads the `FUN_002996D0` call site passing size
`0x9D8`. No banner resolves it; plausibly 0x100F is a BAP-6 envelope total, but that is `[I]`. Both
figures are kept: **0x9D8 `[C]` at the call site, 0x100F `[C]` in contract §3.1, relation `[U]`.**

⭐ **Truman's architecture is in the binary, verbatim:** udts 5/6/7 are Bungie's Auth State and
Sense State by name; udts 38/39/40 are the Mission Script. The prose vocabulary is `[C]` in the
image. (contract §2)

## 3. The lifecycle

State enum at `0x18F8640` (39 entries):
`startup_initializing_ipahp -> setting_parameters -> preparing_to_create_client -> creating_client
-> requesting_host -> establishing_client -> ok -> shutting_down -> disposing`. The 30-entry failure
enum gives the substep order; ⚠ `connecting_to_bap` sits at index 18/19, *after* both transport
resolves, the opposite of the intuitive ordering. (contract §3)

```
init ipahp -> wait session stabilization -> set parameters -> init messaging interface
  -> resolve AHP transport addr -> resolve AH transport addr -> create client -> connect to BAP
  -> wait all session peers establish
  -> send  8 request_activity_host        (session host ONLY)
  -> wait 10 start_activity_host_response
  -> wait host identifier
  -> send  3 join_request
  -> wait  4 join_result
  -> wait  0 entity_slots_allocated       (the entity-index gate)
  -> establish client -> ok
```

**Session-host asymmetry `[C]`:** only the session host sends udt 8. Peers log "waiting for session
host to relay AH ID" (`0x75C200`). (contract §3)

**Startup by argv `[C]`:** udt 8 carries the `/ah_type:.../ah_package:.../ah_slice_set_names:...`
command line (`0x5946B0` -> `0x5949B0`), as BAP id 6. BAP replies with an opaque 64-bit AH ID
rendered `%08X:%08X`. A command line is how you launch a process - this single fact settled the
service-vs-peer conflict. On a reconnect with a valid AH ID the client **skips BAP 6** ("As HOST
with valid AH ID ..., skipping activity host startup request"). (contract §3.1, codec §13.7)

**Address resolution `[C]`:** each peer resolves AH-ID -> address via `get_activity_host_proxy`
(BAP ids 16 -> 17) on slot 0 `OUT PRIMARY`, policy `0x16`. The 16-byte reply is decoded by
`c_activity_client_messaging_interface_bap::c_activity_host_transport_address_resolver::vfunc[0]`
(`0x594E70`): `bswap32(msg+0x08)` = IPv4, `bswap16(msg+0x0C)` = port, into an IPv4 sockaddr setter
(`FUN_002E3B50`, family 4). The peer then opens a separate BAP connection in slot 1/2/3 to that
address. (contract §3.2)

**Establish-client `[C]`:** `FUN_00298EF0` is a conjunction of six live flags with no sequencing
state:

```c
ac[0xE58] != 0                                  // the world exists
  && FUN_002A2220(world)                        // udt 1  arrived   (world+0x50A8)
  && blob[0] != 0 && (blob[1] == 0 || *(i32*)(blob+0x10) >= 1)
  && (**(code**)(*ac + 0x18))(ac)               // udt 4  ACCEPTED  (vfunc[3] = ac+0x130 & 0x20)
  && *(char*)(ac + 0x64AA) != 0                 // udt 0  arrived
  && (*(byte*)(ac + 0x131) & 1)                 // udt 11 arrived
```

Four of the six terms are messages, and nothing ever asks for three of them - the status ladder
(`FUN_00760470`: 26 join -> 27 entity indices -> 28 instantiation) only waits, never rejects an
early arrival. ⚠ But arrival-completeness is true of `FUN_00298EF0` and **false of the router**:
`FUN_0075AB60` matches udts 0/1/11 only against `[ac+0xC98]`, which is zero until udt 4 is
accepted - so in effect **udt 4 must land first**. `d1-activity-host` pushes all four on the join,
in `4 -> 0 -> 1 -> 11`. (codec §11.1, codec banner)

⭐ The client's own status string is the discriminator: `[AC PRIVATE CURRENT CON-y EST-y AH->id
MEM-n]` from `0x29B6E0`; `EST-` is `ac+0x130 & 0x20` (udt 4 accepted), `AH->` the stored
`join_result+0x08`, `MEM-` udt 11's image `+0x00`. (codec §11.8)

The rung-33 activity evaluation reads two fields the AH publishes inside `global_activity_state`:
the `u16` **activity index** at `blob+0x680` (`FUN_0078C770` -> `world+0x50B0+0x680`) and the
`char[0x28]` activity name at `blob+0x688`. Neither is derived from the other; see
`design/activity-launch.md`. (codec §13.4)

## 4. join_result - the 0x1A8 layout

Descriptor `0x80800A58`, struct 424 bytes, 13 fields - the richest message in the protocol. Nine
offsets were first derived from `0x29BD10`'s reads and independently confirmed by the descriptor,
a source that knew nothing about them. `[C]` (contract §4, codec §3.4)

```
[ 0] +0x000  -> 0x80800E98      the WHOLE embedded join_request (0x100 B)
             +0x000 i32  nonce        compared against ac+0xB98
             +0x008 u64  ah_id
[ 1] +0x100  i8  (3-bit leaf)   result / denial reason; 0 = accepted
[ 2] +0x101  -> 0x80804056      session_id, char[128]  (+0x181..+0x184 is padding)
[ 3] +0x184  u32                [U] no reader identified
[ 4] +0x188  u32                [U] no reader identified
[ 5] +0x18C  u32                -> activity_host_changed
[ 6] +0x190  u32                -> simulation queue
[ 7] +0x194  u8                 replication_epoch
[ 8] +0x196  u16                grace_ms            -> ac+0x6C70
[ 9] +0x198  u16                keepalive_interval_ms -> ac+0x6C74
[10] +0x19C  [11] +0x1A0  [12] +0x1A4   [U] declared, no reader identified; we send zeroes
```

* Field `[0]` **is** the request: that is why the nonce is at `+0x00` and the AH id at `+0x08`. The
  server echoes the client's whole request struct into it; `[U]` whether the client reads anything
  beyond the two fields `0x29BD10` touches. (codec §3.4, §4)
* **The denial-reason value space is `[U]`, and it is a real absence.** At `0x29C1D7` the reason
  byte is formatted through the generic `ENUM(%d)` formatter `0xE9FA0`; there is no denial-reason
  string table in this build. Positive controls: two other enum-to-string tables in the same
  subsystem were fully dumped; byte searches for `join_denied` / `session_full` return zero. ⚠ The
  18-entry `_join_result_*` list at `0x16789DD` is the session layer's enum - do not borrow it.
  (contract §7)
* **The replication epoch is a u8** (`0x29BE15 movzx esi, byte ptr [r15+0x194]`), cached at
  `ac+0xE34`, written to `peer+0xDA0` by two apply sites (`0xBAFC10` join, `0xBAFAE0` advance).
  ⭐ The activity client never reads its own copy back - the epoch is pushed into the deterministic
  simulation queue (element type `0x1A` on join, `0x16` on advance). Whole-`.text` disp32 scan for
  `0xE34`: 15 hits, none in the class; positive control: the same scan for `0xE2C` does hit inside
  it. `42 advance_replication_epoch`'s wire payload is 2 bytes `{u8 epoch; u8 suppress_flag}`.
  (contract §6)
* **`10 start_activity_host_response` (0x88 B)**: `{+0x00 u64 ah_id; +0x08 char session_id[];
  ... opaque}`. Guarded: flag `0x02` must be set (we asked) and `0x04` clear (not already started);
  validated by `0xB4F80`; on success `memcpy(ac+0xB10, msg, 0x88)`, `ac+0x130 |= 4`, and the peer
  table at `ac+0x64B8` is built (stride 10 = `{u64 machine_id; u8; u8}`, count at `ac+0x64B4`).
  (contract §4)
* **`1 global_activity_state` is 0x6B0 = 1,712 bytes total** - the largest AH-authored blob, which
  is also the strongest binary-level support for *Physics Host != Activity Host* (no transforms
  anywhere in the AH vocabulary). (contract §4, §12; codec §4)

## 5. Entity indices, reservations, keepalive and the lease

### 5.1 Entity indices - 12,288, granted as a 0x600-byte bitmask `[C]`

| direction | message | shape |
|---|---|---|
| client -> AH | `19 allocate_entity_indices` | 4 B = `u32 count`; sets in-flight flag `ac+0x64A9` |
| AH -> client | `0 entity_slots_allocated` | **0x600 B = 384 x u32 = a 12,288-bit bitmask** |
| client -> AH | `20 free_entity_indices` | same 0x600 shape |

`FUN_0008F800(ac+0x5EA8, 0x3000)` at `0xB4DF80` fixes the count: **12,288 (`0x3000`) indices; an
index is a bit position 0..12287.** They key replication entities (simulation objects), not players
and not peers; the owner holds a 12,288 x 6-byte entity record table with a free-mask and low/high
water marks, next to `c_replication_entity_database_view`. Handler `0x10579F0` ORs the mask into
the authority mask at `ac+0x5EA8`, sets dirty `ac+0x64A8` and has-been-allocated `ac+0x64AA`, and
notifies the simulation. Pool policy: `free = popcount(pool)`, `target = (high+low)/2`; over
high-water free the excess, under low-water request `target - free`. (contract §5)

**The offline seam:** with no Activity Host at all, the client runs `memset(pool, 0xFF, 0x600)` and
self-grants all 12,288 indices. That branch exists in the shipped client `[C]` - but ⛔ **it is not
a substitute for udt 0 when an AH is connected**: the seam writes `db+0x120B8` and never
`ac+0x64AA`, and `ac+0x64AA` is written in exactly one place in all 47,304 decompiled files - the
udt-0 handler. (contract §5; codec §11.7)

⭐ Because `FUN_010579F0` has no "was it requested?" check, and whether udt 19 is ever sent depends
on `*(char*)(FUN_0034D950()+0x81)` which is `[U]`, `d1-activity-host` pushes the grant
**unsolicited and whole-pool** right after `join_result`. A later udt 19 honestly returns an empty
grant; the client's own pool policy frees the excess. (codec §11.7)

Full sub-protocol: `23 claim_authority_over_abandoned` (AH-initiated), `24 purge`, `25 abandon`,
`26 request_purge`, `27/28 reset_mask(+ack)`, `29/30/31 query_mask(+responses)`,
`32 abdicate_authority`. (contract §5)

### 5.2 Reservations - per (machine, account, character) `[C]`

`12 request_peer_reservation` (0x330 B) = `{u32; u32; i32 count; peer[20]}`, each peer 0x28 B:
`{u64 machine/peer id; u64; u64 account_id; u64 character_id; u64}`. **20 records, not 100** (the
100 was the `undefined8` element count, 20 x 5 u64). Driver `0x758E40` sends one per peer, skipping
peers already in membership or already reserved; retries rate-limited to 1,000 ms.
`13 release_peer_reservation` (0x10 B) = `{u32; u32 membership_id; u64 machine_id}`.
`43 reservations_failed` (AH -> client) = `{i32 count; u64 machine_ids[]}`; handler `0x1058A80`
marks `peer_table[i]+8 = 1`.

⇒ **A seat is reserved per (machine, account, character) triple, before that peer joins.** This is
the mechanism behind the peer-side refusal `citizen join ... refused, no reservation found for
amb-nonce=%s` (`0xB74170`). (contract §7)

### 5.3 Keepalive, the lease, and eviction

**The client's keepalive (udt 15) `[C]`:** 1 byte, raw, from `FUN_00298C40`:

```c
if (now - ac[0xE48] >= (i32)ac[0x6C74] || ac[0x15]) { send(0x0F, 1); ac[0x15] = 0; }
```

⭐ `ac+0x6C74` is the interval in ms **and it comes from the AH** - the `u16` at `join_result+0x198`.
**The constructor initialises it to 0; there is no hardcoded interval in the client.** `ac+0xE48`
is bumped by any successful send, so this is a piggyback-suppressed "say something every N ms";
`ac+0x15` forces an immediate keepalive whenever the peer bitmask `ac+0x6C6C` gains a bit. The one
byte it carries is an uninitialised stack local - there is nothing in it to read. (contract §8,
codec §12.2)

**⭐⭐ The lease runs the other way: it is an idle timer on the HOST's transmissions.**
`observed-live` + `static-client-inference`: `FUN_0075E440` computes
`idle = now - last_byte_received_on_the_AH_connection` (four hops down to `conn+0x1EF8`, the same
field the BAP drought line renders) and declares `error.private_activity_host_host_crashed`
(status 36) when `idle >= timeout + grace`. Observed values: timeout 20,000 ms
(`*(int*)(FUN_0034D8D0()+0x28)`) + grace 500 ms (`FUN_001DB260()`) - both runtime config, read
from the client's own printed line. Ten client keepalives made no difference; nothing answers udt
15 and nothing should (udt 16 has no handler). **The Activity Host has to talk.**
`d1-activity-host`: `HOST_SILENCE_LIMIT_MS = 20_500`, heartbeat every `HOST_HEARTBEAT_MS = 5_125`
carrying udt 1 (chosen because its handler's success branch is an idempotent snapshot replace -
the least a repeat can do), envelope-sealed on the one shared nonce chain. (codec §12.1, §12.6,
§13.3)

**Eviction `[C]`:** 11 named AH boot reasons (`activity_host_boot_reason`, fn `0x2A4BB0`, table
`0x263CF30`, FNV-1 verified): `none, debug-command, debug-command-no-rejoin, session-lock-timeout,
ambassador-revocation-ack-timeout, disconnect, connectivity-failure, inactivity-timeout,
session-split, world-server-connectivity-failure, world-server-validation-failure`. The
session-layer boot reason table (fn `0xB79500`, table `0x1954A30`, 13 entries) includes
`11 by_activity_host` - the seam where AH authority enters the peer mesh. `0xB70150` disbands over
20 peers. All three boot delays are runtime config at `FUN_0034DB20() + 0x120/+0x128/+0x140`,
defaults `[U]`. ⇒ the eviction deadline lives server-side; the client stores only the AH-supplied
interval. `[I]` (contract §9)

**The funeral:** `14 peer_leave_request` arrives after the client has already given up; it is
recorded, never answered, and its 20 bytes carry `{u64 [U]; u64 machine_id; u32 reason name hash}` -
the one capture held `fnv1("activity_failed_to_load_missing_dlc")`, a free confirmation of which
gate fired. If the lease holds it should never arrive at all. (codec §12.5, §13.5)

## 6. The codec - FUN_002A4090 / FUN_002A4140

**`[C]` The AH wire format is the QueueZ object format with a different descriptor tag.** Decode is
`FUN_002A4090(u32 tag, void *wire, u64 len, void *out)` = BS_CTOR / BS_BEGIN_READ /
`FUN_002C9000(tag, bs, out, out, 1)` / BS_END, with the bits-consumed <= bits-available predicate;
encode is `FUN_002A4140(u32 tag, void *obj, void *buf, u32 *out_len)`, reached from
`activity_client_send_message` (`0x299310`). Both halves already existed in
`d1-server/crates/d1-oracle/src/object.rs`; the only thing ever missing was *which tag*. It is the
same descriptor machinery as the investment/RSAT grammar that `d1-eboot descriptors` walks.
(codec §1; contract §4 banner)

The server runs the client's own codec, not a reimplementation:
`d1_activity_host::codec::SchemaCodec` drives `d1_oracle::activity_host::AhCodec` (Unicorn, on a
dedicated OS thread), prewarmed at startup (~2.3 s) so the join path stays sub-millisecond.
`serve` refuses to start without the pinned eboot. (codec §6)

**Tag recovery - five static steps, no guessing** (codec §2): each message names its schema through
a runtime-filled BSS global, so a naive static read returns zero. Chain: (1) the `DAT_` global is a
relocation slot -> runtime record; (2) exactly one other slot aims at that record (a 16-byte
`{ptr, u32 checksum, u32 pad}` entry in a sorted table); (3) read the checksum at `slot+8`; (4) the
checksum occurs exactly twice in the image - the table entry and `static_record+8`; (5)
`static_record+0x50` is the descriptor anchor -> tag. ⚠ `static_record+0x60` is the same word as
`anchor+0x10`; checking one against the other looks like a cross-check and is not one. The real
controls (codec §3):

* **Size agreements**: every derived tag's `struct_size` must equal the size the client's own call
  site passes, read from an unrelated place - 29 agreements as of codec §12.3, zero disagreements.
* **Execution against real client bytes**: the `observed-live` 238-byte udt-3 `join_request`
  (pinned as `d1_oracle::activity_host::CAPTURED_JOIN_REQUEST`, from run `20260808-195418-F2-STATE3`)
  decodes under `0x80800E98` to every value the client's own diagnostic logged - nonce, ah_id,
  machine, player, account, character, session string - at 1,903 of 1,904 bits, and **re-encoding
  reproduces the 238 bytes byte for byte**. Neither half was produced by this project.
* **Negatives that must keep failing**: a decoy tag of the same struct_size does not reproduce the
  nonce; the schema wire differs from the struct image (238 != 256); an udt with no call-site row
  refuses; a wrong-size struct image is refused, not padded.
* **Structural tie**: `join_result` field `[0]` *is* `join_request` (§4), so a correct udt-3 tag
  constrains udt 4.

### 6.1 The three send arms

`FUN_00299310` has a schema arm and a **raw arm** chosen by its flags argument. Three call sites
take the raw arm, all passing descriptor `0xFFFFFFFF`: udt 6 (`0x2992B0`, caller's size), udt 8
(`FUN_002996D0`, 0x9D8), udt 15 (`FUN_00298C40`, 1 byte). **These have no schema tag by
construction** - a different refusal reason from "unpinned". And udt 21 uses neither arm: its own
encoder `FUN_0029CC20` builds a bitstream with `FUN_002C9290` (the replicated encoder, with the
client's dirty mask `ac+0x6B88`) and calls `messaging->vfunc[6]` directly - which is why a search
restricted to `FUN_00299310` call sites was structurally incapable of finding it. (codec §12.2,
§12.3)

### 6.2 Raw-struct: falsified in one boot

`RawStructCodec` (emit the in-memory struct verbatim) was kept as a labelled falsifier and spent:
run `20260808-210910-AH-RAWCODEC` failed. ⛔ `D1_AH_RAW_CODEC=1` must not be set again; the binary
prints a warning naming the falsifying run if you pass it. The schema codec is the default and the
deliverable. (codec §9.1, §3.3)

### 6.3 udt 11 `replicate_membership` - seeded decode plus one presence bit `[C]`

Tag `0x808009ED`, struct `0x5038` (20,536 B), verified by execution in both directions. The handler
(`0x29C730`) seeds the decode buffer with the current membership (`memcpy(image, ac+0xE70,
0x5038)`) and calls `FUN_002C9050`, which first reads **one presence bit** (MSB-first) and decodes
the object only if it is set; a full-state replication is `write_bit(1)` + the plain object
encoding (composition shipped in the client itself, `FUN_00628810`). The encoder mirror is
`FUN_002C9290` writing mask bit 0. Traps, both paid for:

* ⚠ `FUN_000733A0` is the bit-writer's **spill path, not a general write** - unguarded use produced
  a wire whose presence bit read back 0, the client's decoder consumed exactly 1 bit and returned
  the seed untouched, and `FUN_002C9050` still returned true. The safe one-bit primitive is the
  leaf at `0xAEF740`. (codec §11.4a)
* ⚠ **`FUN_002C9050`'s return value is a size check, never decode success.** In this build the
  "Failed to decode new membership state." line is unreachable; seeing it would mean the descriptor
  moved. (codec §11.4a, §11.8)

Struct: `[0] +0x0000 u32` (rendered `MEM-%d`; 1 for one member under every reading, semantics
`[U]`), `[1] +0x0004 u32` generation (**send 0** - a difference zeroes the peer-table count
`ac+0x64B4`), `[2] +0x0008 member[20]` stride 0x250 whose identity field is `0x80800C2D` = udt 22 =
the identity the client sent in `join_request+0x10` - ⭐ **a solo membership is an echo, not a
construction** - `[3] +0x2E48` 8,672 B `[U]`, `[4] +0x5028` 20-bit leaf (was `[U]` here; closed by
later work - see `design/player-in-world.md`), `[5] +0x502C` 20-bit occupancy mask, one bit per
member slot. Every offset is confirmed twice: by the descriptor and by `FUN_0029FE70`'s hand-written
slot-search arithmetic. ⚠ `0x29C730` sets the gate bit `ac+0x131 |= 1` unconditionally, even for an
undecodable payload - **deliberately not exploited**, because a fabricated membership would pass the
gate and then fail the slot search in silence. (codec §11.5, §11.5a)

### 6.4 udts 33 and 37 - a harness limit with a named cause

Their descriptors (`0x80800A3D`, `0x80800D06`) each contain a field of kind `0x22`, a
**dynamic-flags bitset** allocated from the game's "dynamic flags" pool; a statically mapped image
has the pool globals zeroed, so decoding faults (`READ_UNMAPPED`). The set is derived by walking
the descriptor tree (`dynamic_flags_udts()` returns exactly `[33, 37]`), reported as `NotDecoded`
at INFO. Both are client -> AH with no client handler, so nothing an AH does depends on decoding
them. Closing it means bringing the pool up in `d1-oracle::harness`; nothing on the ladder waits
for that. ⭐ udt 37's raw hex visibly carries activity name hashes. (codec §12.4, §13.7)

## 7. Open [U]s

| # | unknown | note / what would close it |
|---|---|---|
| 1 | The join-denial value space (i8 at `join_result+0x100`; only 0 = accepted known) | a live or captured session; not more static work |
| 2 | The three boot-delay defaults (`FUN_0034DB20()+0x120/+0x128/+0x140`) | same |
| 3 | The `timed_out_waiting_for_entity_indices` tick and threshold | the reason string exists in the ordered enum blob but in zero of the 47,304 decompiled files - a real absence |
| 4 | Whether the resolved AH address is the AH process or a BAP-side relay | `bap_to_activity_host_socket_handoff_request` (BAP id 153) suggests BAP accepts and hands the socket over; settle with a live BAP-17 response plus a TCP trace |
| 5 | id 8's 0x9D8 vs 0x100F size conflict (§2) | read the BAP-6 framing at the site, or capture one |
| 6 | The unpinned udts (49 minus 30 call-site rows minus the 3 raw-by-construction) | per id: find the send/handler site - not necessarily a `FUN_00299310` site, as udt 21 proved - read the global, add a `CALL_SITES` row, watch the size control |
| 7 | `join_request` field semantics: `+0x30` (u64 repeated at `+0xD8` as FTID), `+0xC0`, `+0xE0`, `+0xF4` (17 bits, `0x0BEE` in the capture), `+0xF8`, `+0xFC` | the oracle proves shape, never semantics |
| 8 | `join_result` `+0x184`, `+0x188`, `+0x19C`, `+0x1A0`, `+0x1A4` | declared by the descriptor, no reader identified; we send zeroes |
| 9 | Whether the client reads the echoed request struct in `join_result[0]` beyond the two fields `0x29BD10` touches | |
| 10 | `global_activity_state`'s content: the 1,712 bytes are the Mission Script's global state; blob `+0x01`/`+0x02` bools (`+0x00` must be non-zero, `[R]`) | zeroes are a valid description of an activity with nothing scheduled |
| 11 | Whether `D1_AH_ACTIVITY` must equal the `act_*` world name the change-world command forces | they are the only two names in the system |
| 12 | Membership `+0x00` semantics (count, generation or id); `+0x2E48` (8,672 B); member record `[1] @+0x028` and `[2] @+0x248` | zeros sent today |
| 13 | Whether the udt-11 presence bit should ever be 0 (a no-change delta) | a stub only sends full state |
| 14 | What sets udt 19's send condition, `*(char*)(FUN_0034D950()+0x81)` | moot for the server (grant is unsolicited), still unread |
| 15 | The lease constants 20,000/500 are runtime config and could move per sku_config | the heartbeat's 4x margin covers it |
| 16 | udt 17: whether the host is expected to diff against the client's membership counter | we re-publish unconditionally |
| 17 | Whether a repeated udt 1 is inert during rungs 33-36 | `D1_AH_HEARTBEAT_MS` varies it; the alternative is certain death at 20.5 s |
| 18 | Whether the client re-opens the AH channel on a healthy 32->33 transition (every observed re-open followed an error) | the reconnect path itself is `[C]` survivable |
| 19 | udt 14's first u64 (`0x...0001` in the one capture) | |
| 20 | udt 33/37 contents (dynamic-flags pool bring-up in the harness) | |
| 21 | `activity_index` validation: nothing checks `D1_AH_ACTIVITY_INDEX` against the client's activity table, whose row count is `[U]` in this build | |

## 8. Dead claims

Claims these sources once asserted, now killed. Do not re-assert any of them.

| # | dead claim | killed by |
|---|---|---|
| 1 | "The AH pushes to the client as BAP 100 `activity_host_to_client_notification`" | codec banner, 2026-08-09: BAP 100 is the AH -> BAP-server hop; `0x564C20` re-emits the body as **BAP 9** on the indexed connection. The client echoed all four of our BAP-100 pushes back byte for byte and `0x299430` never saw one. The *codec* was right; the envelope id was wrong |
| 2 | "`0x783E90` is the ipahp startup / transport state machine" | contract headline: it is a static-initialiser string table - 1,057 instructions, zero calls, zero jumps, referenced only from the ctor array at `0x1A034A0` |
| 3 | "udt 11 `replicate_membership` does not use the schema codec; its handler is `0x29CDE0`" (codec §7.1, asserted `[C]`) | codec §7.1 retraction + §11.2: the RX dispatch table stores its own type ids and is unsorted (11 sits before 10); `0x29CDE0` is udt 7 `sensor_message`; udt 11 is `0x29CF50 -> 0x29C730`, tag `0x808009ED`, round-tripped through the client's own codec. The invented positional index mapping was the whole error |
| 4 | "AH messages are raw structs on the wire" (implicit in contract §4's layouts; explicit as `RawStructCodec`) | contract §4 banner (every RX handler decodes through `FUN_002A4090` first; the `memcpy` that made `join_result` look raw copies the already-decoded struct) and run `20260808-210910-AH-RAWCODEC`, which falsified the raw codec in one boot. `D1_AH_RAW_CODEC=1` is a spent falsifier |
| 5 | "id 30 = `query_..._mask_response` (0x608), id 31 = `query_..._mask_per_bubble_response` (0x604)" | contract §2 correction (re-dump of `0x18F8780`): the names are swapped - 30 = per_bubble_response, 31 = response. The per-id sizes were re-derived from call sites (codec §5): 30 = 0x608, 31 = 0x604 |
| 6 | "The name tables cover all 49 ids" | contract §2 correction: 44 distinct ids; 9/16/21/34/40 absent from both tables |
| 7 | "udt 21 `client_authoritative_data_update` is unwired (no send site)" | codec §12.3: `FUN_0029CC20` encodes it with `FUN_002C9290` and sends via `messaging->vfunc[6]` directly; the `FUN_00299310`-restricted search could not see it |
| 8 | "udt 15's schema tag is unpinned - read the call site and pin it" | codec §12.2: the call site is read and it passes `0xFFFFFFFF` - the raw arm. There is no tag, by construction, and the payload is one uninitialised byte |
| 9 | "`37 send_client_heartbeat` is the liveness heartbeat" | contract §8: its only callers are `c_time_of_day_game_system` paths; it is a bulk world/time-of-day upload |
| 10 | "Answer the keepalive with udt 16" | contract §2 correction: udt 16 has no client handler in either direction; a stub that acks the lease is talking to itself and the drop is silent |
| 11 | "The client keepalive keeps the session alive" (implicit until §12) | codec §12.1, `observed-live`: the lease is the client's idle timer on the HOST's transmissions (~20.5 s); ten client keepalives changed nothing. The AH must transmit |
| 12 | "`request_peer_reservation` carries 100 records" | contract §7 correction: 20 records of 0x28 (100 was the u64 element count) |
| 13 | "`join_result.session_id` capacity is 139" | codec §4: the array is `0x80804056` = 128 bytes; `+0x184`/`+0x188` are two u32 fields a longer id would have overwritten |
| 14 | "`global_activity_state` is 8 + 0x6B0" | codec §4: the struct is 0x6B0 total; `0xBAFB50` unpacks a simulation-queue element, a different object |
| 15 | "Establish-client is arrival-completeness; order never matters" | codec banner: true of `FUN_00298EF0`, false of the router `FUN_0075AB60`, which matches udts 0/1/11 only against `[ac+0xC98]` - zero until udt 4 is accepted |
| 16 | "Nothing this server emits has been accepted by a client" | codec §11.9 retraction: run `20260809-054417-RUNG33-BYPASS` printed `EST-Y AH->D1A00000:00000001 MEM-1` and walked rungs 28 -> 32; the state machine accepted all four, not just the decoder |
| 17 | "The offline seam (`memset 0xFF`) can substitute for udt 0 / skip entity allocation" (contract §5's "directly exploitable") | codec §11.7: the seam writes `db+0x120B8`, never `ac+0x64AA`; `ac+0x64AA` is written only by handler `0x10579F0`. The AH must push the grant |
| 18 | "rung 33 reads activity index 0 because the repair machine de-instantiated the clients" (a correction to another doc, recorded here) | codec §13.4a: at the moment of refusal the activity client was alive and established (`STATUS: [ok]`); `blob+0x680` read 0 because the AH sent 0. Two independent bugs co-occurred |

## 9. Sources

| source | what was taken |
|---|---|
| `historicaldocs/design/activity-host-contract.md` | §1 transport; §2 the 49-id table and its corrections; §3 lifecycle, argv startup, address resolution; §4 message layouts (struct sizes) and the raw-wire retraction; §5 entity indices and the offline seam; §6 replication epoch; §7 reservations and the denial-reason absence; §8 keepalive; §9 eviction; §10 failure vocabulary; §12 not-the-AH's-job list; §13 standing unknowns |
| `historicaldocs/design/ah-schema-codec.md` | banner (BAP 9 vs 100, router ordering); §1-§6 the codec, tag recovery, controls, wiring; §7/§11 the udt-11 retraction and closure; §12 the lease, raw sends, udt 21/14, dynamic flags; §13 the envelope bug, `activity_index @+0x680`, udt 14's reason hash |

Live companions (not duplicated here): `design/activity-lifetime-state.md` (udt 5/6/7 sensor
traffic, the lifetime phase byte - the current frontier), `design/bap-transport.md` (the BAP
envelope, ciphering, message ids), `design/activity-launch.md` (mode + map, activity index vs
name), `design/player-in-world.md` (the `+0x5028` membership fix and what a served membership
unlocked).

Code: `d1-server/crates/d1-oracle/src/activity_host.rs` (derivation, `CALL_SITES`, `RAW_SEND`,
controls) and `d1-server/crates/d1-activity-host/` (protocol, codec wiring, state, listener,
controls). Rebuild whole-workspace only: `cargo build --release`, never `-p` (feature unification).
