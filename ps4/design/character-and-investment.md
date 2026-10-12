# The character and the investment system - canonical reference

**State (2026-08-11):** a type-9 biped stands in `pvp_moonbase` (see `design/GUARDIAN-IN-WORLD.md`
for the current spine). This document consolidates everything durable about the character and the
investment system: the boot route, the server-object estate, the character objects and their
serializations, the four lifecycle messages, level/light/customization, the progression table, and
which object each screen reads. Sources are melted from the documents in the closing table; cite
them as `historicaldocs/design/<name>.md §N`.

**Reach discipline:** unless marked otherwise, static findings are `static-client-inference` against
`dumps/patch-01.29-CUSA00219/eboot.bin`, and everything reached across our own served bytes is
`synthetic-minimal` / `synthetic_ancestor`. Nothing here touching identity or authoritative state
may be promoted to protocol contract without capture or independent confirmation.

## Contents

1. The route: boot ladder and gates
2. The server-object estate
3. The character objects, one by one
4. The synthetic character: concrete values and pinned payloads
5. The RSAT codec and the eboot serializer oracle
6. The lifecycle messages: 501 create, 502 delete, 504 login, 505 signout (and 503, 506)
7. Level, Light, Customization - where they actually come from
8. The progression and stat tables - ordinal 117
9. Which object each character-screen element reads
10. What orbit needs (the investment content map)
11. Standing [U] gaps
12. Dead claims
13. Sources

---

## 1. The route: boot ladder and gates

### 1.1 The 38-state world-controller ladder `[C]`

State-name table at `0x192d2a0` (relocated pointer array, 38 entries; slots read zero in the file
image - `DT_SCE_RELA` applies). Index = the enum value passed to `request_state()` = `0x782710(n)`.

```
 0 pregame            10 upsell                   20 investment_signin      30 setup:activity_host_setup
 1 media_check        11 email                    21 profile_setup          31 setup:prologue_intro_loading
 2 bootload           12 confirm_account_transfer 22 prepare_for_orbit      32 setup:orbit_outro
 3 intro_screen       13 content_check            23 character:signin       33 setup:activity_world_transition
 4 start              14 content_install          24 character:prologue     34 activity:initial_slice_set_loading
 5 platform_signin    15 insufficient_space       25 cleanup                35 activity:physics_join
 6 platform_warnings  16 illegal_dlc_media        26 setup:orbit_loading    36 activity:in_world
 7 enum_content_rights 17 queuing                 27 setup:orbit            37 activity:watch_video
 8 account_signin     18 package_registration     28 setup:activity_session_creation
 9 eula               19 bap_signin               29 setup:matchmaking
```

Handler vtables (10 slots; slot 4 = `enter`, slot 7 = `update`):

| state | vtable | enter | update |
|---|---|---|---|
| 19 `bap_signin` | `0x192dcf0` | `0x797fe0` | `0x798030` |
| 20 `investment_signin` | `0x192e5c0` | `0x79bfc0` | `0x79bff0` |
| 21 `profile_setup` | `0x192eaa0` | `0x79e810` | `0x79e830` |
| 22 `prepare_for_orbit` | `0x192ea20` | `0x79e420` | `0x79e450` |
| 23 `character:signin` | `0x192f000` | `0x7a0060` | `0x7a0160` |

**`profile_setup` is NOT character select.** `[C]` It is the three first-run display screens
(screen bounds, brightness, subtitles), a local UI wizard that sends no message and reads no
character. Character select is state 23, one gate after `prepare_for_orbit`. The skip: one byte at
`profile_blob + 0x18` (account view `+0x10`) non-zero routes `investment_signin` straight to
`prepare_for_orbit`; zero runs the three screens, then the client sets the byte to 1 itself
(`0x79E830`; predicate `0x57E490`). Both branches reach `prepare_for_orbit`.
(historicaldocs/design/investment-profile-contract.md §1, §6)

**`cleanup` (25) is a normal transit state** on the happy path prologue -> orbit, not only an error
sink. Read the reason, not the state. (historicaldocs/design/character-and-orbit.md §26.71, §26.77)

Ui-stage -> ui-state map: u32 table `0x15c0640` (49 entries, index `stage + 1`) into the 42-entry
name table `0x192d6a0`. Stage `0x14` = "bootflow investment signin", `0x15/0x16/0x17` = the three
profile-setup screens, `0x1A/0x1C/0x1D` = character start / selection / creation.

### 1.2 `bootflow:investment_signin` (20) - the only network gate on this stretch `[C]`

`update` `0x79bff0` exits when `h[+0x30] == 7 && h[+0x34] == 3`, then applies three saved display
settings (account view bytes `+0x1A/+0x1C/+0x1D`, absolute blob `+0x22/+0x24/+0x25`, into config
vars `0x26c2ee8/0x26c2ee4/0x26c2ee0`) and requests state 22 (or 21 if the setup byte is clear).

* **Sub-machine A** (`h[+0x30]`, `0x79c110`): local content only - package-set 4 load, bootstrap
  activity activation. No network.
* **Sub-machine B** (`h[+0x34]`, `0x79c370`): the investment gate. Its only action is `0x5417c0()` -
  clear two status bytes, then raise **game event `0x23`** with payload `true`. Every investment
  message afterwards is emitted by the game systems, not by the bootflow handler.

Exit predicates, all required (historicaldocs/design/investment-profile-contract.md §2.5):

| # | condition | function |
|---|---|---|
| A | `user_mgr[+0x41C] == 0` (no account-version block) | `0x585190` |
| B | a local platform user exists (`g_0x2638cd8 + 0x108 + i*0x198`, 4 slots, `[+0x38]` == primary controller at `[+0x790]`) | `0x244cd0` |
| C | investment account record `tag == 3` and `record.platform_user == U` | `0x585190`, `0x585270` |
| D | `record.account_object != NULL` and `record.ready_byte != 0` | `0x586430` |
| E | `profile_mgr[+0x08] != NULL` and `[+0x08][+0x08] == record.account_object` | `0x57ca40` |
| F | `profile_mgr[+0x10] != NULL` (profile blob installed) | `0x57cc80` via `0x57e3d0` |

Failure reasons (runtime reason table `0x26b22a0`, stride 0x10, 284 entries): `0xAD`
`error.no_bap_connection`, `0xAE` `error.lost_bap_connection`, **`0xAF`
`error.no_investment_connection`** (the timeout), `0xB0`..`0xB4` the five
`error.investment_server_object_*` reasons (map `0x15c0750`), `0xB5`
`error.investment_account_version`. One timer: `h[+0x38]` vs `config[+0x20]` = engine config
`0x554`, default **30,000 ms**, settable over the wire as **client_config key 157**.
(historicaldocs/design/character-and-orbit.md §26.41)

**The investment-user account record** (single record; the "array" base IS the manager object at
`*(0x26A3888 + 8)`; stride 0x30; scan length 1) `[C]`:

```
+0x00  u8   tag (3 = signed in)
+0x01  u64  platform user key        (UNALIGNED, read as [rec+1])
+0x10  u64  account SOID             (a handle resolved against the registry, not a pointer)
+0x18  u64  READY flag               (low byte tested)
+0x20  u64  SELECTED PROFILE SOID    (slot B - the character:signin gate)
+0x28  u8   dirty     +0x29  u8  pending
manager+0x30 last-published SOID, +0x38 publish state, +0x3c timer, +0x48 sign-in substate
```

**The ready byte is a cached registry lookup, not a flag anyone sets.** `[C]` Sole non-zero writer
`0x585820` (per-tick): `record+0x18 = 0x583450(&SOID)` = "does a (family 4, type 0) object keyed by
the account SOID exist in the object registry" (`0x56CA70(reg, 4, 0, &key)`). No server response
field can set it; only delivering the account object can.
(historicaldocs/design/character-and-orbit.md §26.41, §26.46-§26.48)

The client subscribes the account root itself: `0x584EB0` requests class **4 first, then class 3**
(`0x584F74`, `0x584F8B`) once signon assigns the SOID. Delivery before that subscription is fatal;
`D1_PUSH_AT=echo` (after the keepalive loop begins) is the proven timing.
(historicaldocs/design/character-and-orbit.md §26.47-§26.48)

### 1.3 `character:signin` (23) - the character gate `[C]`

Handler `0x7A0160`. Exit: `0x78C5F0(NULL) == 6` and orbit-map-loader status `h[+0x148]` in {5, 6}
(a failed orbit map load does NOT block), then `request_state(24)`.

`0x78C5F0` is a ladder 0 -> 1 -> 2 -> 3 -> 6 over the account record: rung 1 the platform key, rung
2 slot A (`+0x10`, the account SOID, plus ready byte), rung 3 **slot B** (`+0x20`, the selected
profile SOID). Slot B's only writer is `0x585820`:

```
slot B = *(account_object + 0x68)     ; must be non-zero
         and must resolve as (family 4, type 1) = the PROFILE     (0x5834A0 / 0x5833C0)
```

**Account schema `0x80801803` field 3 at C `+0x68` is the profile SOID** - the field that opens
state 23. It is a schema field; an account payload that marks it absent decodes as zero and the gate
can never pass. (historicaldocs/design/character-and-orbit.md §26.75-§26.76; the codec-verified
28-byte payload is in §4.3 below.)

**Creation vs selection** is decided by `0x78C9F0`, which reads the **family-3 ROOT (type 0)
roster** keyed by the ACCOUNT SOID: object absent, or `count == 0` at `+0x08` -> CREATION;
`count >= 1` -> SELECTION. `[C]` Confirmed live by injection and then over the wire.
(historicaldocs/design/character-and-orbit.md §26.60, §26.62, §26.69)

⚠ Two different lists exist and two different readers use them
(historicaldocs/design/character-and-orbit.md §26.66):

| list | object | reader | decides |
|---|---|---|---|
| roster | family 3 / type 0 (`0xA0`) | `0x78C9F0` (state machine) | CREATION vs SELECTION |
| account list | family 4 / type 0 `+0x08` count, `+0x10 + 8i` SOIDs | `0xA110F0` (card builder) | how many select cards render |

Both must agree; both carry the same schema child `0x80804621` = `{u4 count; u64 storage[10]}`.
`serves_no_characters()` in `d1-responder` zeroes both together.

**No reply commits a selection.** `[C]` The state 23 exit is entirely data-driven by registry object
state; message 504 is sent by the client but the transition itself reads objects.
(historicaldocs/design/character-and-orbit.md §26.60)

### 1.4 `character:prologue` (24) skips itself `[C]` static, confirmed live

`0x79F9F0` sets substage 5 exactly when the u16 index read via `0x574220(4)` from **family 4 /
type 4** (checksum `0xAF776963`, size `0xF90` - the unlock/campaign object) is `0xFFFF`. Substage 5
is the only branch that requests a transition, and it requests state 25 (`cleanup`), i.e. the orbit
path. Absent that object the index stays `0xFFFF` and the prologue routes itself past itself -
observed live in run `20260807-175730` (entered and left in the same breath).
(historicaldocs/design/character-and-orbit.md §26.71, §26.77)

The `0x811C9DC5` sentinel check at `0x79FB2C` (`call 0x548860; cmp [rdx+0x18], 0x811C9DC5`) is on
the substage-0 path (a record exists). The `+0x18` is on the **resolved record** returned by
`0x548860`, not proven to be raw `character_object+0x18` - do not write the sentinel into the
QueueZ character object at a guessed offset.
(historicaldocs/design/rsat-character-object.md §9; historicaldocs/design/character-and-orbit.md §2, §26.56)

`0x811C9DC5` is the FNV 32-bit offset basis = the hash of the empty string = the engine's canonical
"no value". ⚠ The client's hash is **FNV-1** (multiply then xor), not FNV-1a; the basis is shared so
the constant cannot distinguish them. An all-zero field is NOT "none" - zero is a valid-looking
hash. (historicaldocs/design/character-and-orbit.md §26.56)

### 1.5 gate-minimal / orbit-minimal / PvP-minimal `[C]`/`[I]`

| milestone | persistence needed |
|---|---|
| leave `profile_setup` | none - a per-session synthetic account + profile satisfies every predicate |
| controllable character at orbit | one durable character record; `last_orbited_destination` has a tolerated-failure fallback to two configured default destinations (`g_0x26B07A4`, `g_0x26B07A0`, validated via `0x344910`) |
| private PvP | durable per-participant: six distinct stable account SOIDs, per-account subscription state (`networking:investment:peer` validates peer subscriptions) |

(historicaldocs/design/investment-profile-contract.md §1, §7-§8)

---

## 2. The server-object estate

### 2.1 Family / type map, checksums, schema tags `[C]`

Family/slot mapper `0x56C550` (jump table `0x56C8C4`; nested type table for family 4 at `0x56C8E4`).
Registry lookups go through `0x56CA70(registry, family, type, &key)`; type arguments come from
constant thunks (`0x1DF0` = type 0, `0x1048A40` = type 1, `0x59710` = type 2). Registry singleton
`0x56A0D0` / `[0x26A2040]`.

| family/type | schema tag | checksum | size | cap | identity |
|---|---|---|---:|---:|---|
| 0 / 1 | `0x808017FD` | - | 2632 (`0xA48`) | - | **peer view** - what the character screen reads (§9) |
| 2 / 0 | `0x808018B7` | - | 80 × 231 | 231 | player display name + clan tag (two-hop lookup `0x57F6B0`/`0x57F6F0`) |
| 2 / 1 | `0x80801810` | - | 48 (`0x30`) | 231 | player-card handles `[I]` |
| 3 / 0 | - | **`0xA2AC2C74`** | `0xA0` | 1 | **roster** {u64 account SOID, i32 count, u64 char SOIDs[]} |
| 3 / 1 | `0x80801954` | **`0x3431F1BA`** | `0xA80` | 10 | **character select card** |
| 4 / 0 | `0x80801803` | **`0x64AE75BF`** | `0x7338` | 1 | **account** |
| 4 / 1 | `0x80801743` | **`0x8127F15A`** | `0x7078` | 1 | **the active character's full state ("profile")** |
| 4 / 2 | `0x80800FB2` | - | `0x48` | 1 | `[U]` |
| 4 / 3 | `0x80801732` | - | `0xB8` × 666 | 666 | inventory (⇒ `design/items-and-gear.md`) |
| 4 / 4 | `0x8080102E` | **`0xAF776963`** | `0xF90` | 1 | unlocks / campaign (⇒ `design/grants-and-unlocks.md`) |
| 5 / - | - | - | - | - | root registered at init as `0x7FFFFFFFFFFFFFFF` (global `0x2639EA0`; neighbour `0x2639EA8` = `0x7FFFFFFFFFFFFFFE`) |

Schema-handle globals (BSS, filled at load; read live then deref one qword): 0/1 `0x1966268`,
2/1 `0x1966278`, 3/1 `0x1966280`, 4/3 `0x1966330`, 4/4 `0x1966338`. Schema records: family 3/0
`0x139ADD0`, 3/1 `0x1350750`, 4/1 `0x1304800` (checksum at record `+0x08`, size at `+0x14`).
(historicaldocs/design/character-and-orbit.md §26.61; historicaldocs/design/investment-content-map.md §2;
historicaldocs/design/level-light-customization.md §2.1)

⚠ **Unresolved tag discrepancy** `[U]`: historicaldocs/design/rsat-character-object.md §4 reads the
character schema record at `0x1350758` as tag **`0x808017DC`** (6 fields, span `0x2C0` bits), while
the live registry, the descriptor walks and `d1-oracle`'s pinned decode vectors all give
**`0x80801954`** (2688 B) for family 3/1. Both describe the same `0xA80` layout with the same
children (`0x808017FC`, `0x8080181A`, `0x80801755`, `0x80801855`, `0x80801923`). No resolving
banner exists; prefer `0x80801954` (three independent confirmations) but the record-level reading
has not been formally retracted.

### 2.2 Delivery and replication - the character-relevant summary

Full protocol: `design/queuez-replication.md`. What this document's objects depend on:

* Objects arrive as `0x007B queuez_to_client_update_notification` bodies: `u32be transaction_count`,
  then transactions `{u32be family, u64be root_soid, u32be version (0xFFFFFFFF = NONE), u8 flags,
  u32be object_count}` (21-byte header), then objects `{u32be instance_type (the checksum), u64be
  soid, u32be rsat_length, rsat bytes}`. Baseline requires `flags` bit 0 set; versions monotonic;
  header soid must equal the soid inside the RSAT payload. `[C]`
  (historicaldocs/design/investment-profile-contract.md §5.2)
* Replication descriptor state 1 -> 2 is **local and unconditional**: the client sets state 2 at
  the moment it sends `0x000C` (subscription request, `u8 family_class || u64be root_soid`); state
  2 -> 3 requires a baseline with flags bit 0. No wire message advances state 1; a `state=1`
  rejection is a race on an early push, not a wall. `[C]`
  (historicaldocs/design/character-and-orbit.md §26.64-§26.65)
* `0x000C` must be **answered** (`0x000D`, bodyless, class 2, status `0x00C8`) - not to advance
  state (the handler is a `ret` stub) but because an unanswered request head-of-line blocks the
  client's in-flight queue and eventually kills the connection. `[C]` `observed-live`
  (historicaldocs/design/character-and-orbit.md §26.74)
* Never send `0x7C` (subscription-lost): it drives the descriptor to state 4. `[C]`
* Object registration and family replication are different things: an object can reach the registry
  (clearing the ready byte) while the family update is ignored. `[C]`
  (historicaldocs/design/character-and-orbit.md §26.57)
* ⚠ `(update REFUSED)` in `d1guest.py descriptors` output is the **tool's annotation** of state 4,
  not the client's words; "a timed-out family rejects a late push" is `[I]`, never measured.
  (historicaldocs/design/investment-content-map.md §1)

The standing law, paid for repeatedly: **answering a message wrongly is worse than not answering
it**, and a negative result from a run with an unanswered request outstanding is not a result at
all (the connection is already poisoned). (historicaldocs/design/character-and-orbit.md §26.51,
§26.74)

---

## 3. The character objects, one by one

### 3.1 Family 3/1 - the character-select card (`0x80801954`, 2688 B) `[C]`

```
C+0x0000  u64                     character SOID
C+0x0008  {u8 race, u8 gender, u8 class}          (0x808017FC)
C+0x000C  0x8080181A              CUSTOMIZATION (36 B, 12 fields)   -> §7.3
C+0x0030  0x80801755              4 x u32 (+0x30,34,38,3c), 3 x u64 (+0x40,48,50), +0x58 {bool,u64}
C+0x0068  0x80801855              2568 B, 16 fields:
   C+0x0068  u32                  LEVEL                              <- 0xA11555
   C+0x006C  float (kind 0x10b)
   C+0x0070  float                floor()'d, shown next to level     <- 0xA1151C
   C+0x0074  float
   C+0x0078  5   x 68 B (0x8080196C = {u8, [16]u32})   talent-grid-ish  [U]
   C+0x01CC  32  x u32
   C+0x0250  20  x 64 B (0x8080183C)                   EQUIPMENT     <- 0xA11627
   C+0x0750  8   x 16 B ({u8, u64})
   C+0x07D0  32  x u16
   C+0x0810 / 0830 / 0850   16 x u16  (three tables)
   C+0x0870  16  x {u8 stat_index, u32 value}          STATS incl. LIGHT  <- 0x2666F0
   C+0x08F0 / 0970 / 09F0   16 x {u8, u32}  three more stat tables   [U]
C+0x0A70  0x80801923              {float, u32, u16}                  <- 0xA11589
```

Equipment element `0x8080183C` (64 B): `{u64 @+0, u16 @+8, u16 @+0xA, u16 @+0xC, [8]u16 @+0xE,
{u32,u32} @+0x20, [6]{u8,u16} @+0x28}`. Field `[1]` (u16 at element `+8`) is the item's content
index; `0xA11627` reads `[character + 0x258 + 64*slot]` for the emblem (slot 12, from
`investment_globals->vtbl[0x160]()[0x18]`). `0xFFFF` is the empty sentinel everywhere in this
client; `0x0000` is a valid index. (historicaldocs/design/level-light-customization.md §2.2, §2.1a)

### 3.2 Family 4/1 - the full character / "profile" (`0x80801743` -> `0x80801714`, 28,792 B) `[C]`

The in-world source of truth. One-field wrapper over an 18-field struct whose first three fields
match the card exactly:

```
[0]  +0x0000  u64  SOID
[1]  +0x0008  -> 0x808017FC  {race, gender, class}
[2]  +0x000C  -> 0x8080181A  36-byte customization block
[3]  +0x0030  -> 0x80800F9F  {u32, -> 0x80800FA1 (10,880 B = 340 x 32)}   backpack items
[4]  +0x2AB8  -> 0x808019FF
[5]  +0x2B80  -> 0x808045DA  20 x u64   EQUIPPED ITEM INSTANCE IDs
[6]  +0x2C20  -> 0x808017B9    [7] +0x2C40 -> 0x8080193C
[8]  +0x3660  -> 0x808047A7  {u32 count (7 bits), [127] progression}   PROGRESSIONS
[9]  +0x3E58  -> 0x80801755    [10] +0x3E90 -> 0x80801701
[11] +0x52C0  [12] +0x5AC0  [13] +0x6AC0  [14] +0x6CC0    [U]
[15] +0x6CD0  -> 0x808018BB  character stat block incl. the OVERRIDE array (§7.2)
[16] +0x6E64  [17] +0x7064
```

(historicaldocs/design/level-light-customization.md §2.1; the 340 + 326 = 666 cross-check against
the account's 326-slot array at `0x80801803` C+0x4C0 is §2.1a.)

The account (4/0, `0x80801803` -> `0x808018C7`, 29,496 B, 25 fields): `[0]` u64 SOID, `[1]` +0x08
`0x80804621` character list, `[2]` +0x60 u32 `[U]`, **`[3]` +0x68 u64 profile SOID** (the slot-B
source), `[6]` +0x4C0 item array (326 x 32), `[9]` +0x304C account progressions (fixed 127).

### 3.3 The peer view - family 0/1 (`0x808017FD`, 2632 B) `[C]`

A pure re-layout of the card: `[0x00,0x30)` verbatim, then the same 2,568-byte `0x80801855` block
at **`+0x30`** (card has it at `+0x68`), trailer `0x80801923` at `+0xA38`. The card and peer view
differ by exactly one field ([3] `0x80801755` on the card only). Built by
`d1-oracle character build-peer`; the `f0t1-*.bin` fixtures are this object.
(historicaldocs/design/character-screen-consumers.md §1.3)

The same `0x80801855` block lives in four carriers; in every carrier `block+0x00` = level u32,
`block+0x08` = the float `0x161550` returns, `block+0x1E8` = the 20 x 64 B equipment summary,
`block+0x808` = the 16-entry stat table:

| carrier | block at | reader |
|---|---|---|
| family 3/1 card | `+0x68` | `0xA110F0` (select tiles) |
| family 0/1 peer view | `+0x30` | `0x9A3C30` mode 4, `0x9A5F50`, `0xA1AB60`, `0xA2D9F0` |
| family 1 object | `+0xB60` | `0x9A3C30` mode 1, `0x9A5F50` |
| translation-manager record (stride `0xE48`) | `+0x430` | `0x9A3C30` mode 4 via `0x58DB10` |

### 3.4 Family 2 is the player directory, NOT progressions `[C]` shapes, `[I]` naming

* 2/0 `0x808018B7`, 80 B x 231: `{u64 key, u64 key2, [27] u16 @+0x10, [5] u16 @+0x46}` - a 26-char
  UTF-16 display name and a 4-char clan tag.
* 2/1 `0x80801810`, 48 B x 231: `{u64 key, u16, u16, u8, 0x808016F8 @+0x10, 0x808018DB @+0x24}`.
* Access is a two-hop player lookup: `0x57F6B0(key)` finds 2/0; `0x57F6F0(key)` finds 2/0 then uses
  its `+0x08` as the key into 2/1. Consumers: roster/scoreboard rows (`0x582BC0`), emblem-shaped
  `{u16,u16}` handles (`0x9E96C0` -> `0x78A000`).

The real progression stores are family 4/1 `+0x3660` (character) and 4/0 `+0x304C` (account) - §8.
(historicaldocs/design/level-light-customization.md §6.1;
historicaldocs/design/investment-content-map.md §3 carries the original refuted family-2 reading.)

---

## 4. The synthetic character: concrete values and pinned payloads

Everything in this section is `synthetic-minimal`; downstream state is `synthetic_ancestor`. The
SOIDs are ours, not Bungie constants; the account SOID must equal what account-id translation
issued, and the object-header SOID must equal the SOID serialized inside the RSAT.

### 4.1 Identity - Titan / Human / Female `[C]`

Wire order is **`{race, gender, class}`** at `object+0x08` - ⚠ NOT the `{class, race, gender}`
order the public manifest lists. The wire stores **small type values**, not manifest hashes:

```
race    Human   0        gender  Female  1        class   Titan   0
object+0x08 = 00 01 00
```

Byte construction from helper `0x264CE0` (`[rcx+0]=race, [rcx+1]=gender, [rcx+2]=class`); the
gender input is bounded 0..1. `[C]` on the construction and gender; `[I]` strong on naming the two
zero-valued endpoints. (historicaldocs/design/rsat-character-object.md §6)

Manifest hash names (2020 manifest, **era caveat** - naming corpus, not era-correct truth; where
manifest and the 01.29 install disagree, the install wins):

```
class    Titan 3655393761 (0) - Hunter 671679327 (1) - Warlock 2271682572 (2)
race     Human 3887404748 (0) - Awoken 2803282938 (1) - Exo 898834093 (2)
gender   Male 3111576190 (0)  - Female 2204441813 (1)
```

Destination hashes for `last_orbited_destination` (same era caveat): The Last City 2612139675, Old
Russia 2468353467, Ishtar Sink 518553403, The Reef 3393905939, The Crucible 2777041980, Ocean of
Storms 4233735899, Meridian Bay 4072959335, Dreadnaught 2897855902. ⚠ There is no "Orbit"
destination; the orbit activity content tag `0x80B52001` is a different namespace.
(historicaldocs/design/character-and-orbit.md §3)

### 4.2 The pinned payloads (all emitted or verified by the client's own codec)

| object | payload | size / bits |
|---|---|---|
| character (3/1), SOID `0xD100000000000101` | `e880000000000080f0181c0000` | 13 B / 97 bits |
| roster (3/0) | `e880000000000000c74400000000000404` | 17 B / 136 bits |
| account (4/0), list only | `e880000000000000c74400000000000404000000` | 20 B / 157 bits |
| account (4/0), with field 3 = profile SOID | `e880000000000000c74400000000000405d100000000000002000000` | 28 B / 221 bits |
| profile (4/1), key only | `e880000000000100800000` | 11 B / 82 bits |

The account-with-field-3 payload decodes through the client's codec to a 29,496-byte object with
exactly four non-zero qwords: `+0x00` `0xd100000000000001` (account SOID), `+0x08` 1 (count),
`+0x10` `0xd100000000000101` (character SOID), `+0x68` `0xd100000000000201` (profile SOID).
⚠ The profile SOID is `0xD100000000000201` - an early hand-read of the dump as `0x…0002` was wrong
(historicaldocs/design/character-and-orbit.md §26.76.1).

Complete 54-byte QueueZ family-3 baseline body (account root `0xD100000000000001`):

```
0000000100000003d1000000000000010000000001000000013431f1bad1000000000001010000000de880000000000080f0181c0000
```

(historicaldocs/design/rsat-character-object.md §1, §8;
historicaldocs/design/character-and-orbit.md §26.67, §26.76)

Character bit grammar (rebuild for any SOID):

```
1 . u64be(character_soid) . 1 . 1+u8(race+0x80) . 1+u8(gender+0x80) . 1+u8(class+0x80) . 0000 . pad to byte
```

Roster/account grammar: `1 . u64be(key) . 1 . u4(count) . count x u64be(soid) . 0 x (field_count - 2) . pad`.

⚠ **The short-payload trap:** the client's bitstream reads past the end of a short buffer and
returns zeros, indistinguishable from "every remaining field absent" - a 17-byte account payload
decodes "correctly" while overrunning 21 bits. Size payloads from the schema's own field count;
the regression guard is `account_payload_is_long_enough_for_every_field_the_client_reads` in
`d1-server/crates/d1-bap`. (historicaldocs/design/character-and-orbit.md §26.67)

### 4.3 What each object buys

| served object | opens |
|---|---|
| account 4/0 with character list | `investment_signin` ready byte; select-card count |
| account field 3 (`+0x68`) + profile 4/1 under that SOID | `character:signin` exit (slot B) |
| roster 3/0 count >= 1 | SELECTION instead of CREATION |
| character 3/1 | the card's data; the in-world biped needs 4/1 and family 0 (§9) |
| family 4/4 absent | prologue skips itself (§1.4) |

---

## 5. The RSAT codec and the eboot serializer oracle

**The investment message codec IS the RSAT object codec** - one generic, reflection-driven
serializer family (`0x105Bxxx`-`0x105Exxx`), one grammar family with leading presence bits. `[C]`
(historicaldocs/design/character-and-orbit.md §18)

### 5.1 Key vaddrs `[C]` (`vaddr = file_offset - 0x4000` for the text segment)

| vaddr | purpose |
|---|---|
| `0x72240` / `0x723B0` / `0x72400` / `0x72C70` / `0x733A0` | bitstream ctor / begin-write / begin-read / read-bits / write-bits |
| `0x2C8FF0` -> `0x2C8420` | higher-level RSAT encode wrapper |
| `0x2C9000` -> `0x2C8680` | schema-driven field walker (object codec entry) |
| `0x105B410` | generic lower-level RSAT writer (the oracle) |
| `0x105DD20` / `0x105E3C0` | generic reader/writer pair of the same codec |
| `0x261BE0` / `0x261FB0` | investment server-message encode / decode (u16 message id then RSAT body) |
| `0x1350758` | character schema record (checksum `0x3431F1BA`); descriptor `0x13507A8` |
| `0x1A0B420` | the package registry global (three inlined walks at `0x2622C7`, `0x2623FE`, `0x2624EF`) |

Nested descriptors used by the character schema: `0x808017FC` -> `0x12FF778`, `0x8080181A` ->
`0x13B2540`, `0x80801755` -> `0x13ABF90`, `0x80801855` -> `0x136B9B0`, `0x80801923` -> `0x12FA958`,
`0x80804705` -> `0x12E83A8`. (historicaldocs/design/rsat-character-object.md §2, §4)

### 5.2 Descriptor format, final reading `[C]`

Anchored descriptors: field records stride `0x28`; `marker+0` is `u32 serialized_start` (NOT a
constant sentinel), `marker+4` = `0x3F000000`; field kind/flags at `+0x08` (`0x101/0x105/0x106/…`
carry the `0x100` **optional-presence** flag - NOT an array flag); child tag at `+0x0C`
(`0xFFFFFFFF` = leaf); width in bits at `+0x14`; **the field's C offset is at `marker-8`**
(`marker+0x20` is the NEXT field's offset - the defect that produced three retracted readings).
Registry keying is `(tag_slot(tag), tag & 0x1FFF)` - never the low 13 bits alone (656 collisions).
Tags are compile-time constants (`0x80000000 | ((container << 13) & 0x7FFE000) | (index & 0x1FFF)`);
all three containers (`0x400/0x401/0x402`) are eboot-resident.
(historicaldocs/design/character-and-orbit.md §24.10-§24.12, §26.33)

Offline reader: `capture-rig/eboot-descriptors.py` (4,555 descriptors); live resolver:
`capture-rig/dump-registry.py`.

### 5.3 Dirty-field map and watched reads (the character encoding) `[C]`

The encode context is `{bitset*, 0, 1, object*, bitstream*}`; the physical dirty-bit position is
`serialized_index + 1` (physical bit 0 reserved). Mask `0x3E` (SOID + identity parent + 3 children)
makes the real writer read exactly `object+0x00` (8 B SOID) and `object+0x08/09/0A` (identity) -
the watched reads that corrected the descriptor-tail misreading (tails are NOT C offsets).
(historicaldocs/design/rsat-character-object.md §5, §10)

### 5.4 Per-field encoding rules - measured at the codec, never derived `[C]`

| descriptor kind | width | encoding |
|---|---:|---|
| `0x103` | 9 | 1 presence bit, then `value + 0x80` (i8) |
| `0x3` @ 8 | 8 | `value + 0x80`, no presence bit |
| `0x4` @ 16 | 16 | `value + 0x8000` |
| `0x5` @ 32 | 32 | `value + 0x8000_0000` |
| `0x9` @ 32 | 32 | raw, no bias |
| `0x6` @ 64 | 64 | raw big-endian, no bias |
| `0x3` @ 4 | 4 | `value + 1` (so `-1` encodes as 0) |
| `0x3` @ 2 | 2 | `value + 1` |

⚠⚠ **The bias is per FIELD, not per width.** Any "consistent" arithmetic model gets one wrong; the
Rust decoder carries an explicit per-field table pinned by three codec vectors plus a negative
control. A zeroed result composite encodes `18 00 00 00 00` - a hand-written `00 00 00 00 00` would
encode `-1`, a different value that still looks empty.
(historicaldocs/design/character-lifecycle-messages.md §4, §5.2)

⚠ **A leading-zero-bit payload is indistinguishable from an empty one** - any body starting with
zero bits decodes cleanly as "all absent". "No error" says nothing about whether content was
understood. (historicaldocs/design/character-and-orbit.md §11.5)

**Doctrine: do not hand-fit grammars.** Drive the client's own encoder under Unicorn
(`native-oracle` skill, `d1-oracle`) and verify round-trips offline before anything touches the
wire. Four hand readings of this format were retracted across two sessions; every result that held
was read out of, or executed through, the client's own code.

---

## 6. The lifecycle messages

### 6.1 The ids, resolved by FNV-1 name hash and confirmed by literal immediates `[C]`

The shipped definition table (`capture-rig/schemas/investment-message-definitions.bin`, content tag
`0x80A5E37A`, pkg 303 entry 890, **101 records of 0x40 bytes**) is
`{+0x00 u32 FNV-1 name hash, +0x10 u32 id, +0x14 request tag, +0x18 response tag}` and nothing
more - no handler binding, no flags. ⚠ Ids are not sorted in the table.

| id | name | FNV-1 hash | sender | wire id |
|---:|---|---|---|---|
| **501** | `login_create_character` | `0xE906221F` | `0x551040` | `0x1F5` |
| **502** | `login_delete_character` | `0xF721133E` | `0x5510C0` | `0x1F6` |
| 503 | `login_account` | `0x0C662090` | `0x5515D0` (flag 1, 48-byte body from Platform User `+0x08..+0x37`) | `0x1F7` |
| **504** | `login_character` | `0x7D6146CC` | `0x551640` | `0x1F8` |
| **505** | `signout_character` | `0x1CE11BB6` | `0x551C80` | `0x1F9` |
| 506 | `customize_character` | `0x27B12A16` | - | - |

All funnel through the send core `FUN_00551FB0(msg_id, flag, payload, dest)` at `0x551FB0`. Free on
the same pass: 201 `fetch_account`, 202 `fetch_character`, 203 `fetch_item`, 204 `fetch_time`,
403 `inventory_equip_item`, 601 `loot_pickup_item`. Negative control:
`fnv1("create_character") = 0x71738EDD` and `fnv1("delete_character") = 0x63FB2504` appear nowhere
in the table. The remaining ~90 ids are unresolved by preimage `[U]`.
(historicaldocs/design/character-lifecycle-messages.md §1-§2)

⚠ **501 is `login_create_character`; 504 is `login_character`.** Three documents once said 501 was
`login_character`; every experiment that "answered 501" was answering create with a login-shaped
body. (historicaldocs/design/character-lifecycle-messages.md banner;
historicaldocs/design/character-and-orbit.md §26.55)

### 6.2 Schemas `[C]` (from `d1-eboot descriptors`, confirmed at the codec)

```
501 login_create_character   req 0x80801860 (60 B, 3 fields, 397 bits)  rsp 0x8080185D (16 B, 2 fields, 100 bits)
502 login_delete_character   req 0x80801939 ( 8 B, 1 field,   64 bits)  rsp 0x808017F9 ( 8 B, 1 field,  36 bits)
503 login_account            req 0x80801726 (48 B)                      rsp 0x80801825 (968 B, 5 fields)
504 login_character          req 0x808017CE ( 8 B, 1 field,   64 bits)  rsp 0x80801845 ( 8 B, 1 field,  36 bits)
505 signout_character        req 0x808017E5 ( 1 B, 0 fields,   0 bits)  rsp 0x80801886 ( 8 B, 1 field,  36 bits)
506 customize_character      req 0x80801814 (40 B, 3 fields)            rsp 0x80801736 ( 8 B, 1 field,  36 bits)
```

502/504/505/506 all answer with the same `0x80801796` `{i4, i32}` composite; the client's encoder
emits `18 00 00 00 00` for a zeroed object at every one. Only 501 differs: `{result composite,
u64}` where the u64 is the new character's SOID (`[I]`, strong - creation is the one operation that
mints an id; `d1-oracle message --tag 0x8080185D --msgid 501` emits wire id `01f5`). 504's observed
request body: `01 f8` + u64be character SOID + trailing pad.
(historicaldocs/design/character-lifecycle-messages.md §4;
historicaldocs/design/character-and-orbit.md §26.70)

**503's response, the winning shape** `[C]`: `{+0x00 composite 0x80801796 {4-bit, 32-bit}, +0x08
u64 account SOID, +0x10 u32, +0x14 u32, +0x18 composite 0x80801941 (944 B)}` - `0x80801941` field
[0] is the **family-5 root SOID**, which must be `0x7FFFFFFFFFFFFFFF` (a zero there produces
`error.investment_server_object_family_record_not_found` for `(5, 0)`). Message 205's response
`0x80801827` embeds the same `0x80801941`. The pinned working body:
`D1_503_RAW=180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020810220000000100`.
(historicaldocs/design/character-and-orbit.md §24.10-§25, §26.83-§26.84; the wider signon story is
`design/signon-and-entitlement.md`)

### 6.3 The 501 request is the character object's own prefix ⭐ `[C]`

```
+0x00  bits 0..27     0x808017FC   the identity triple {race, gender, class}
+0x04  bits 27..261   0x8080181A   the 36-byte, 12-field customization block
+0x28  bits 261..397  0x80804257   char[17]   (used by 0x80801860 ALONE - exists only for creation)
```

27 + 234 + 136 = 397 exactly - no top-level presence bit, no slack. The create request is the
prefix of the character object itself: whatever the client sends here is, by construction, what a
character is made of. `0x808017FC` and `0x8080181A` are shared with the family-3 card
(`0x80801954`), the family-4 profile (`0x80801714`), family 0/1 (`0x808017FD`), and (for the
customization block) message 506. (historicaldocs/design/character-lifecycle-messages.md §4.1)

`s_create_character_server_message_request_arguments` - 60 bytes (`0x3C`), 57 used + 3 pad, lives
at `c_create_character_server_message_component + 0xB0`, built in place by the ctor `0x996D50`:

| offset | size | field |
|---|---:|---|
| `+0x00`/`+0x01`/`+0x02` | 1 each | race `[I]` / gender `[I]` / class `[C]` (validated `0x264E80`; ctor default `-1,-1,-1` via `FUN_00264CE0(0xFF,0xFF,0xFF)`) |
| `+0x04` | 36 | customization block `0x8080181A`: i16 f0; i8 f1..f3; i16 f4..f6; f7/f8 nested `0x80804705` = i16[3] each; i16 f9; **u32 f10 = FNV-1 name hash, raw** (ctor writes `0x811C9DC5` = unset); 2-bit f11 (biased +1) |
| `+0x28` | 17 | **the PSN Online ID** - NUL-terminated ASCII, from Platform User record `+0x3C`, NOT from the UI |

The Platform User record (table `DAT_02638CD8`, 4 entries at `+0x108`, stride `0x198`): `+0x00` u64
handle, `+0x08` 0x30-byte credential blob (the entire message-503 payload), `+0x38` u32 controller
index, `+0x3C` 17-byte online id string. D1 characters are unnamed; the create request stamps the
platform identity. (historicaldocs/design/character-lifecycle-messages.md §5, §5.1)

### 6.4 The components and their traps `[C]`

All four are `c_event_handler_component` subclasses with **22-slot** vtables:

| component | typeinfo | send path |
|---|---|---|
| `c_create_character_server_message_component` | `0x193A010` | vt[21] `0x996FB0` -> `0x551040` |
| `c_cui_delete_character_server_message_component` | `0x193A690` | vt[21] `0x99B3A0` opens a dialog only; vt[14] `0x99B2E0` polls it and sends via `0x5510C0` |
| `c_login_character_server_message_component` | `0x193E6F0` | vt[21] `0x9B82A0` is an adjustor thunk (`add rdi,0xB0; jmp 0x551640`) - no prologue, invisible to call-graph scans |
| `c_cui_signout_character_server_message_component` | `0x193DC10` | vt[21] dialog only; vt[14] `0x9B25C0` -> `0x551C80` |

A probe on slot 21 records "the user asked" and never sees the delete/signout message. The four
component factories are reached from static `.data.rel.ro` layout tables, not code (ctor xrefs
return 0). Which CUI path hash fires each component is `[U]` - the layout tables at
`0x27409A8/0x27411C0/0x2745940/0x2746A60` hold 64-bit path hashes and our oracle is 32-bit FNV-1.
(historicaldocs/design/character-lifecycle-messages.md §3)

### 6.5 The char_list dead-code finding, and when the messages fire `[C]`

`d1-responder` originally built the account's character list as `&[chr]` unconditionally, so
`D1_CHARACTER_SOID=0` served **a list of one character whose SOID is 0**, not an empty list - the
intended empty-list path existed only as dead code (the `unused variable: char_list` warning).
Measured through the client's own codec:

```
d1-oracle object --tag 0x80801803 --decode e880000000000000c1d100000000000201000000
  -> count field ABSENT = 0 characters
d1-oracle object --tag 0x80801803 --decode e880000000000000c40000000000000001d100000000000201000000
  -> count == 1
```

Fixed as `serves_no_characters()`: `chr == 0` means no characters, for family 4 AND family 3
together (`D1_EMPTY_ROSTER` is a synonym). A list of `[0]` is NOT an empty list.

⚠ The count gate is two-headed: the CUI card list reads the **account** (family 4) `+0x08`; the
**state machine** reads the **family-3 root** `+0x08` via `FUN_0078C9F0` at `0x78C9F0`. Both must
be zero for creation. (historicaldocs/design/character-lifecycle-messages.md §7 and its banner)

**The "never fired" claim is retracted:** across all 231 `runs/*/responder.log`, 501 fired 9 times
in 8 runs (four with full 52-byte bodies decoding at exactly 397 bits; block `+0x1C` = `0x811C9DC5`
unset in all four; the 17-byte PSN Online ID round-trips), 505 fired twice; 502 and 506 never.
Create transcripts already exist. (historicaldocs/design/character-lifecycle-messages.md banner)

### 6.6 Implementation (d1-responder / d1-bap) `[C]`

`d1-server/crates/d1-bap/src/character_lifecycle.rs` + responder dispatch. Knobs:

| knob | default | effect |
|---|---|---|
| `D1_CHARACTER_LIFECYCLE` | off | answer 501/502/505/506 (off = `** NAMED GATE **` line, no reply) |
| `D1_CHARACTER_SOID=0` | non-zero (pinned by `run-boot.sh`) | serve an account with zero characters |
| `D1_CREATED_CHARACTER_SOID` | `0xD100000000000101` | the SOID minted in the 501 reply, then served back as 3/1 + 4/1 |
| `D1_CHARACTER_TRANSCRIPT_DIR` | `$RUN_OUT` | where `create-character-<t>.txt/.bin` land |
| `D1_EMPTY_ROSTER` | off | synonym for `D1_CHARACTER_SOID=0` |

⛔ `D1_INV_SILENT` still wins, and 504 keeps its own long-standing branch (proven route to orbit).
Controls include the negative `decoder_rejects_a_wrong_bias` and
`an_empty_character_list_encodes_count_zero`.
(historicaldocs/design/character-lifecycle-messages.md §9)

### 6.7 `cui::s_login_character_info` - the select-card CUI record `[C]`

160 bytes (`0xA0`), vtable `0x1946C40`; slot 7 `get_property` key 0 exposes the character
`s_server_guid` at `+0x08`. Layout (fill loop `0xA110F0`, copier `0x115F020`): `+0x10/+0x14`
class-name string index/FNV hash (init `0xFFFFFFFF`/`0x811C9DC5`), `+0x18` level cap (init -1),
`+0x1C` level = `*(card+0x68)`, `+0x24` = `*(card+0xA74)`, `+0x28` f32 = `*(card+0xA70)`, `+0x2C`
level fraction, `+0x34/35/36` class/race/gender from `card+0x0A/0x08/0x09`, `+0x38` owning
platform-user object (**the sort key**, comparator `0x9F3D50`), `+0x40` guid dup, `+0x48` source
kind 3, `+0x50` 0x30 nameplate/emblem block, `+0x80` i16 emblem handle from
`card + 0x258 + slot*0x40`, `+0x9C/+0x9D` at-cap / can-level flags. Ten records land in the
`ui::c_static_vector<…,10>` at `component + 0x2E0`, sorted on `+0x38`.

The one-byte `+0x88` on `c_investment_login_info_component_v2` with sentinel `0xFF` is a
**local-player / controller index (0..3)**, set only by the ctor and the CUI property setter -
answering 504 does NOT set it. It is not a character index.
(historicaldocs/design/character-lifecycle-messages.md §6)

---

## 7. Level, Light, Customization - where they actually come from

The one-table answer `[C]`:

| | source | object | offset |
|---|---|---|---|
| Level (select card) | a plain u32 the server sends | family 3/1 | `C+0x68` |
| Level (in world) | derived from a progression | family 4/1 | `+0x3660` table |
| Light (select card) | a stat in a 16-entry table | family 3/1 | `C+0x870` |
| Light (in world) | summed over equipped gear, or an explicit override | family 4/1 | `+0x2B80` gear / `+0x6CD0` override block |
| Customization | 12 fields after the identity triple | family 3/1 AND 4/1 | `+0x0C` (both) |

(historicaldocs/design/level-light-customization.md §0)

### 7.1 Level `[C]`

On the card, level is not derived - the server states it at `C+0x68`; the "at cap" flag compares
against `investment_globals[0x170]` (accessor `0x276220`, value `0x28` = **40**). In the world the
client stores **raw progress, not a level**, in the per-character progression table (§8); the level
comes from walking the progression definition's steps.

Progression element (family 4/1 `+0x3660` u32 count serialized as 7 bits, `+0x3664` 127 x 16 B;
account: 4/0 `+0x304C`, fixed 127, iterator walk at `+0x3058`):

```
+0x00 u32 current progress    +0x04 u32 periodic A ("daily", zeroed by 0x265C20)
+0x08 u32 periodic B ("weekly", zeroed by 0x265D80)    +0x0C u16 PROGRESSION INDEX (0xFFFF = empty)
```

Evidence chain: the const iterators (`0x273BE0/0x273B70/0x273BC0` character, `0x273AD0` account),
the QueueZ change announcer in `0x56D3F0` (reads the same layout, fires `0x573610(mgr, key,
value)`), and the schema - all agree.
(historicaldocs/design/level-light-customization.md §3)

**A server writes level as one entry: `{progress, 0, 0, index = 117}`** - see §8 for the ordinal.
Whether "level 40" needs progress >= 326,000 or >= 346,000 is `[I]` (step semantics unread); the
cap clamps, so write comfortably above 346,000 and read the displayed level back before pinning.

### 7.2 Light `[C]`

Light is a **stat**; its index is a content constant: `0x281F50` = `globals->vtbl[0x68]()[0x140]` =
**0** (`Light`), `+0x145` = 1 (`Defense`); confirmed independently from content (§8). `0x281EE0`
scales a raw stat through the `DestinyStatDefinition` display-interpolation curve (`0x1056940`).

* Select card: `0x2666F0` / `0x266670` scan the 16 x `{u8 stat_index, u32 value}` table at
  `character C+0x870` (`p+0x808` where `p = C+0x68`). Three more identical tables at
  `C+0x8F0/0x970/0x9F0` are `[U]`.
* In world: the aggregator `0x2660E0` -> `0x265EF0`. If the override presence byte at
  `profile+0x6CDC` is set, the 7-entry `{u8 stat_index, u32 value}` array at **`+0x6CE0`**
  (`0x80804768`; a second identical table at `+0x6D18`) is used **verbatim - no gear touched**.
  ⚠ The array base is `+0x6CE0`, NOT `+0x6CE8`; the `+0x6CE8` reading is retracted (a
  pre/post-increment misread; pinned as a negative control in `d1-oracle profile controls`).
  Otherwise light is summed over the 20 equipped item instance ids at `profile+0x2B80`, each
  resolved into family 4/3 and to its definition via the u16 at `item+0x24`.

The override block is the cheapest lever: light non-zero in the world with zero items shipped.
⚠ `d1-eboot descriptors` prints an array field's ELEMENT SIZE in the offset column - the `@+0xNN`
of a kind-0x1 field is stride, not a displacement (the trap behind the `+0x6CE8` error).
(historicaldocs/design/level-light-customization.md §1.1, §4 and banner)

⚠ Grep trap: the only `light` strings in the eboot are renderer/DualShock terms; there is no
gameplay "light" string.

### 7.3 Customization `[C]` structure, `[I]` field naming

`0x8080181A`, 36 bytes, 12 fields, at `+0x0C` in BOTH the card and the profile, directly after the
identity triple:

```
+0x00 i16   +0x02/+0x03/+0x04 i8 x3   +0x06/+0x08/+0x0A i16 x3
+0x0C [3]u16 (0x80804705)   +0x12 [3]u16   +0x18 i16
+0x1C u32 (kind 0x9 - RAW 32-bit, hash-shaped)   +0x20 i8, 2 bits (bool)
```

The block is a 1:1 shape match onto D1's public `DestinyCharacterCustomization` (12 members; the
hash, two small arrays, one bool, eight scalars line up uniquely) `[I]` - and the create-request
containment (§6.3) promotes the block identification to a contract. **Which scalar is which is
`[U]`** (resolve via the creation writer around `0x9D2640`, not by guessing).

⚠ **Field `[10]` (`+0x1C` in the block) is a 32-bit FNV-1 name hash whose referent depends on the
container.** The create component's ctor initialises it to `0x811C9DC5` (unset). In the **profile**
that field position was measured live as `last_orbited_destination`; `personality` remains a
consistent-but-unproven candidate for the create case `[U]`. The old "field [10] = personality"
reading is dead. (historicaldocs/design/character-lifecycle-messages.md §4.1;
historicaldocs/design/level-light-customization.md banner)

⚠ **Contradiction, unresolved, both readings kept:** historicaldocs/design/level-light-customization.md
(banner, 2026-08-08) states block `+0x1C` / absolute `+0x28` IS `last_orbited_destination` -
"writing Old Russia's hash there is what makes orbit render" `[O]`. But
historicaldocs/design/investment-content-map.md §8.1 (also 2026-08-08) reports a three-way
round-trip-verified marker probe at `+0x28`, `+0x480` and `+0x4C0` of the served 4/1 object coming
back `HASH(0x00000000)` - **none of the three** - leaving the C offset `[U]`. What is `[C]` by
causation: serving a full profile object feeds the field the consumer `0x752AE0` reads
(`chr_view + 0x480`, i.e. profile-blob `+0x4C0`), and the unset sentinel is tolerated (fallback to
configured defaults). A "ruler" probe (`*(u32*)(obj+k) = k`) would settle it in one run.

### 7.4 The visible Guardian is mostly gear, and default gear is a content path `[C]` fn/data, `[I]` consequence

The art loaders (`world_tag_loader_fill_desired_*`): `0x574D90` character instance (family 4/2 +
4/4 via selection manager `0x26A34B0`), `0x574E00` peer view (family 0/1), `0x574ED0` select
character view (family 3/1), **`0x574FB0` default gear - item list entirely from
investment-globals content, no server object at all**, `0x575210` character creation, `0x575270`
talent grids (family 4/1). A default-but-correct-looking Guardian is a supported client path.
(historicaldocs/design/level-light-customization.md §5.3)

---

## 8. The progression and stat tables - ordinal 117

### 8.1 The answer `[C]`

**`character_level` is progression ordinal 117 (`0x0075`) in the 01.29 content** - the u16 the
client writes at progression element `+0x0C`. Stated outright by the content three ways: a
one-element singleton `{0x6650C0F9, 117}` (entry 8131, class `0x8080190F`), a 39-pair
`{hash, ordinal}` cross-reference block (entry 8133, class `0x8080031C`, elements `0x80800313`; 40
matches, 0 mismatches), and the array index (`(0x25298 - 0x23900) / 56 = 117.0`). Upgraded to
`observed-live` by a byte scan of `runs/snapshots/orbit-all.snap`: `{character_level, 117}` present
(2 hits), `{…, 116}` and `{…, 109}` **absent**, with two positive controls in the same scan.
(historicaldocs/design/progression-table.md)

Location: `ps4_investment_globals_012f_3.pkg` (content id 303, build 52553, 2016-09-28), block
1066, entry idx 8132, tag `0x80A5FFC4`, class `0x80801779`, offset 145616 (`0x238D0`), 247 elements
x 56 B, element class `0x808016E3`. Stat table: block 515, entry idx 580, tag `0x80A5E244`, class
`0x808017EC`, offset 172176, 36 x 16 B, element class `0x80801870`.

### 8.2 ⚠ The ordinal is a property of the CONTENT VERSION, not the format `[C]`

| package | build / date | count | element size | `character_level` ordinal | steps |
|---|---|---:|---:|---:|---:|
| `012c_0` | 41345, 2014-06-30 | 198 | 40 B | **109** | 20 |
| `012f_1` | 46419, 2015-08-16 | 242 | 48 B | **116** | 40 |
| `012f_2` | 51854, 2016-08-19 | 247 | 56 B | 117 | 40 |
| **`012f_3`** | **52553, 2016-09-28** | 247 | 56 B | **117** | 40 |

The element grew 8 bytes per era; hard-coding 56 mis-parses older packages into plausible garbage.
The client loads `012f_3` (gen chain resolves newest; runtime cap 40 vs vanilla's 20 steps; the
snapshot discriminator). **Do not carry 117 to any other build without re-reading it.**

### 8.3 Ordinals and the curve

| name | hash | ordinal | scope | steps |
|---|---:|---:|---:|---:|
| `actual_level` | `0x447CA1B4` | 0 | 4 | 0 |
| `actual_level_character` | `0xE3547A64` | 1 | 3 | 41 |
| `actual_level_gear` | `0x04485DB6` | 2 | 5 | 0 |
| `actual_level_light` | `0x2975AEF1` | 3 | 3 | 0 |
| `base_item_level` | `0x7ACF6FD7` | 11 | 1 | 100 |
| `character_display_xp` | `0xC49699FC` | 115 | 1 | 1 |
| **`character_level`** | **`0x6650C0F9`** | **117** | 1 | 40 |
| `character_prestige` | `0x79002D5E` | 118 | 1 | 1 |

Stats: **`Light` = 0** (`0x8E8B4E10`), `Defense` = 1 (`0xE854FA8E`), `Attack` = 11 (`0x15F5C563`).

`character_level`'s step array (class `0x808017FA`, 40 x `{u32 progress_to_advance, u32 ?}`) is the
published D1 XP curve: 0, 900, 4100, 5000, 6000, 7000, 8000, then 9000 each through level 37
(cumulative 301,000), 10,000, 15,000, 20,000 - cumulative **346,000** at 40.

**Content hashes are stable across the whole D1 lifetime; only the ordinal moves.** All eight 2020
manifest hashes resolve on 01.29 content with scope + step count matching 8/8 - the public manifest
is a usable name oracle for this build, but never for an ordinal, index or row order (its own
`index` column is 0 on every row). The wire/runtime key is a u16 ordinal; no content hash appears
as an immediate in the eboot at all (0 hits for eight hashes vs 11,895 for the `0x811C9DC5`
control). (historicaldocs/design/progression-table.md §8;
historicaldocs/design/level-light-customization.md §3.3)

Instrument: `capture-rig/oodle/content-table.py --controls` (exit 2 on failure; element size is
DERIVED, never assumed; owed a Rust port into `d1-pkg`).

---

## 9. Which object each character-screen element reads

### 9.1 The two-screens answer ⭐ `[C]`

The main screen and the character screen can disagree (40/400 vs 0/0 at the same instant) because
**two different readers read two different objects**:

| screen | function | object | level | Light |
|---|---|---|---|---|
| character-SELECT tiles | `cui::c_investment_login_info_component_v2::vfunc[14]` `0xA110F0` | family 3/1 | `C+0x68` (`0xA11555`) | table at `C+0x870` (`0xA11511`) |
| character summary / stats | `c_cui_investment_character_state_component::vfunc[14]` `0x9A3C30` | family 1 `+0xB60` or family 0/1 `+0x30` | `block+0x00` | `0x2666C0(block+0x808, 0x281F50())` |

⭐ **`0xA110F0` is the ONLY function in the 47,304-function corpus that resolves family 3 for UI.**
The full census of `0x56CA70(…, 3, …)` callers: `0x54EB40`, `0x574ED0`, `0x5763D0`, `0x78C9F0`,
`0xA110F0`. No other component can ever display a number put in the family-3 card. Zeros on the
character screen were never a delivery failure - they are an object (family 0/1) that was never
built. (historicaldocs/design/character-screen-consumers.md §0-§1)

`0x9A3C30` has two source modes selected by the byte at `this+0x98`: mode 1 reads family 1 and
family 2; mode 4 reads family 0 plus the translation-manager mirror via `0x58DB10` (binary search
over 20 records, stride `0xE48`, key `record+0x08`, gate `*(char*)record < 5`, returns
`record+0x430`). The mirror is fed by `c_investment_peer_subscription_manager_game_system`
(`0x58C7F0`, `0x58CB40`) with a **two-hop lookup in family 0** ending at `Y+0x30` - exactly the
family-0 block offset. **Family 3 cannot reach it.** Siblings `0x9A5F50` (stat list) and `0xA1AB60`
(stat group) behave identically; legal families are 1 and 4 (via 0), never 3.
(historicaldocs/design/character-screen-consumers.md §1.2, §1.4)

Select-tile field reads, instruction by instruction: level `0xA11555` (`C+0x68`), raw Light
`0xA11511` (`0x266670` over `C+0x870`), scaled Light `0xA11501` (`0x2666F0`), second number
`0xA1151C` (floor of `C+0x70`), emblem `0xA11627` (equipment slot 12's `+0x08` u16), grimoire-ish
u32 `0xA115D2` (`C+0x38`), class/race/gender `0xA115DE` (`C+0x0A/0x08/0x09`).

### 9.2 The other three defects, briefly (details: `design/items-and-gear.md`)

Independent, not one cause: padlocks = inventory element `+0x1C` is the equip MINIMUM STAT
(`0x269971`; the requirement table's one live stat id is `0x0075` = 117 = `character_level`, `[I]`
that the minimum stat is the required level); wrong tab = the tab is POSITIONAL
(`FUN_0026DB60(scope, slot)`, 39-record bucket table in `capture-rig/inventory-buckets.tsv`);
spinners = a content-residency fact (definition AND art tag must both be resident,
`FUN_008DB440` -> `FUN_003491A0(...) == 2`) - no instance-field correctness clears a spinner.
(historicaldocs/design/character-screen-consumers.md §0, §1.5-§1.6, §2)

Fixtures: `d1-oracle character build-peer` (`f0t1-*.bin` + the family-2 pair; `--card-key`
defaults to the shipped `f2t1-` fixture's key because `0xA3DB55` looks family 0/1 up with the same
key as family 2/1 `[I]`). The current in-world state uses the hybrid
`build-peer --no-equip --no-emblem` roster (`capture-rig/rosters-f0-hybrid/`).

---

## 10. What orbit needs (the investment content map)

Measured 2026-08-08 against a live client parked in `setup:orbit`
(historicaldocs/design/investment-content-map.md):

* The client subscribes **families 0 and 2** keyed by the account SOID; unanswered, they go state
  2 SUBSCRIBED -> 4 LOST/TIMED-OUT and the client re-subscribes forever (~83/run). `[C]`
  `observed-live`. Family 0/1 is the peer view (§3.3, §9); family 2 is the player directory (§3.4).
  Serving family 0 is what fixed the character screen and (as the hybrid fixture) clears rung-34
  task 7 - see `design/GUARDIAN-IN-WORLD.md` for current state.
* Families 4/3 (inventory) and 4/4 (campaign/unlocks) are allocated and were all-zero: the blank
  character and empty inventory were measured, not inferred. `0x8080102E` nests 3200 bytes of
  `0x80801028 {u64, u6}` = 200 x (id, 6-bit state) - unlock-flag shaped `[I]`
  (⇒ `design/grants-and-unlocks.md`).
* ⚠ **This phase is blind to the log:** the client ships ZERO diagnostic strings for `inventory` or
  `progression` (positive controls in the same census: `destination` 14, `unlock` 7). A wrong or
  absent inventory looks exactly like a correct one in the log; verify by reading live memory
  (`d1guest obj 4:3`) after a push.
* Director path: `0x752AE0` (`orbit_map_loader`, the `last_orbited_destination` consumer and its
  `0x811C9DC5` sentinel + fallback), `0x7A8190` (`activity_world_transition`, "cannot find
  character profile" among three causes), `0x56D3F0` (activity completion `unlock_expression`
  gate), `0x59A3B0` (matchmaking destination-name fallback). The activity-selection gate `0x574220`
  PASSES and the family-4/4 lookup at `0x5742BD` is skipped while `[0x2639EA8] == 0` - the
  selection manager was not consulting the unlock object at all.
* The 2020 manifest on disk (`manifest/data/`, version `88857.20.11.05.1145-2`; era boundary Hotfix
  2.5.0.2, 2017-02-14) is `[R]`-grade for era questions, good for hash -> name:
  `DestinyInventoryItemDefinition` 8,609 rows, `DestinyUnlockFlagDefinition` 7,731,
  `DestinyProgressionDefinition` 252, `DestinyActivityDefinition` 653, buckets 40, destinations 24.
* ⚠ `d1guest tag` cannot resolve a manifest content hash - it decomposes its argument
  arithmetically as a runtime `0x8080xxxx` tag and is a false-positive generator on anything else.
  Use the package/content tooling.
  (historicaldocs/design/level-light-customization.md §6.2)

The launchable-activity story (mode + map, 407 = Elimination, Patrol unreachable) is
`design/activity-lifetime-state.md` territory; the manifest's Elimination row (activityHash
2263964303, type 39921727, destination 2777041980) closed a package-corpus `[U]` and stands with
the era caveat. (historicaldocs/design/character-and-orbit.md §4)

---

## 11. Standing [U] gaps

| gap | how to close |
|---|---|
| block `+0x1C`'s referent in a CREATE request (`personality`?) | first real transcript, or 32-bit preimage over content names |
| which CUI path hash fires each lifecycle component | read the layout tables, build a 64-bit CUI-path hash oracle |
| 501's response u64 really is the new SOID (`[I]`) | send a distinctive SOID, watch for a family-3 subscribe keyed on it |
| whether the client accepts `result = 0` as success (4-bit leaf biased +1) | a `-1` A/B, one env line |
| the remaining ~90 investment message names | widen the FNV-1 preimage vocabulary |
| which customization scalar is face/skin/lip/eye/decal/hair/feature | disassemble the creation writer `0x9D2640` |
| the three extra stat tables at card `C+0x8F0/0x970/0x9F0` | find readers by the `+0x888/+0x908/+0x988` addressing pattern |
| family 4/2 (`0x48`) identity | unattributed |
| profile 4/1 fields `[3] +0x30`, `[11]..[14]` | unattributed |
| the `last_orbited_destination` C offset in the served 4/1 object (§7.3 contradiction) | a ruler probe (`*(u32*)(obj+k) = k`) over the object, one run |
| whether "level 40" needs progress >= 326,000 or >= 346,000 | write above 346,000, read the displayed level back |
| whether a late push to a state-4 family is really refused | push family 0 late, read the descriptor back |
| account schema field `[2]` (`+0x60` u32) requirement | nothing reads it on the boot path; left absent |
| which family-1 type `0x9A3C30` mode 1 reads | `d1-live watch` on `this+0x98` during a screen open |
| the character schema tag discrepancy (`0x808017DC` vs `0x80801954`, §2.1) | re-read record `0x1350758` head vs the live registry record |
| 502/506 have never fired on any run | drive delete / customize from the UI with the lifecycle flag on |

---

## 12. Dead claims

Each line: the killed claim, and what killed it. Do not re-assert these.

1. "501 is `login_character`" (three docs) - killed by the FNV-1 name-hash table + literal
   immediates: 501 = `login_create_character`, 504 = `login_character`.
2. "`create_character` / `delete_character`" as message names - the hashes appear nowhere in the
   101-row table (negative control).
3. "Not one of the four lifecycle messages ever fired" - killed by the 231-run census (501 fired 9
   times, 505 twice); the count-gate mechanism stands.
4. "profile_setup is character select" (old ROADMAP) - it is the three-screen settings wizard;
   select is state 23.
5. "The Oodle decode exposed message-definition records 501-504" - retracted by its own lane; no
   defensible source (RGBA-like records conflated).
6. Descriptor tails read as raw C offsets (identity at `+0x0C`) - killed by watched execution:
   reads at `+0x08..+0x0A`.
7. "Slot 1 is the only object kind accepted by family 3" - killed by the family-3/type-0 roster.
8. "Pushing the family-3 character object makes `character:signin` see count 1" - the count is the
   family-3 root roster; the card list is the ACCOUNT list.
9. "The roster is THE character list" - it is one of two; the UI enumerates the account's list.
10. "Family 2 is progressions" - refuted; progressions live at 4/1 `+0x3660` and 4/0 `+0x304C`;
    family 2 is name + clan tag.
11. "State 4 = update REFUSED, so timing is part of the contract" - the label is `d1guest.py`'s own
    annotation; `[I]`, never measured.
12. The stat override array at `+0x6CE8` - retracted; it is at `+0x6CE0` (a pre/post-increment
    misread; pinned as a negative control).
13. Customization field `[10]` = `personality` (as established fact) - the field is a raw FNV-1
    name hash whose referent depends on the container; in the profile it was measured as
    `last_orbited_destination`; `personality` survives only as a `[U]` candidate for create.
14. "The CUI one-byte character index with sentinel -1" - it is a controller index
    (component `+0x88`).
15. "The account identity is server-delivered via BAP type 0x26" - `0xBAB860` is an in-process
    simulation-queue proxy (`c_investment_player_identifier_proxy_game_system`).
16. "`0x8080xxxx` is a class/type namespace, not content" - they ARE content tags in eboot-resident
    registry containers (`0x400/0x401/0x402`).
17. Every pre-§24.10 descriptor "field array" reading (field count at `+0x28`; "marker" u64
    sentinel; offsets at `marker+0x20`; variable stride) - final format: offset at `marker-8`,
    stride `0x28`, `marker+0` is `u32 serialized_start`.
18. "The 503 payload is one field off" - retracted; the SOID at `+0x08` was correctly positioned
    (the dumper's offset column was wrong, not the payload).
19. "kind & 0x100 = repeated/array" - it is an optional-presence flag; the span is one presence
    bit + u16 length, no schema involvement.
20. "The span element is name hash `0xE2B769E8`" and "tags are per-load, carry the name hash" -
    element is `0xE2E25C2C`; tags are compile-time constants and may be hard-coded.
21. "The producer of `0x56FFB0` cannot be found statically" - it is a tail call (`jmp` at
    `0x560D27`); a scan with no positive control is not evidence of absence.
22. The `0x4C`->503 / `0x3B`->206 class-id mapping from ordering alone - unsafe as derived; later
    settled by full enumeration.
23. "The account record is cleared after signon writes it" - the polls preceded the write (two boot
    cycles in one trace); it was an ordering artifact, and the actual blocker was the ready byte.
24. "Telling the client it has the account object makes it stop asking" and "a family-3 push at
    relay makes the client hang up" - both confounded by a missing `D1_503_RAW`; retracted.
25. "`0x000C -> 0x000D` does not work" (the refutation) - the wrong refutation; the ack IS required
    (head-of-line blocking), it just does not advance replication state (state 1->2 is local).
26. "`0x0020` has no paired response class" - `0x0021` exists (protobuf, 648-byte object); the
    silence-beats-answering measurement stands, the explanation was wrong.
27. "Our character push does not land" - a post-teardown registry read; it lands. Never read
    registry contents unless the account object is verifiably live.
28. "Destiny hashes content with FNV-1a" - the client's hash is FNV-1 (multiply then xor); the
    shared basis proves nothing.
29. "`profile_setup` is steered by config at `0x263D0F8` / a boot-to-X shortcut exists" - that
    handler is `bootflow:queuing` (state 17) polling the signon manager; `profile_setup::update` is
    `0x79E830`; no config-driven boot-to-X exists in the binary.
30. "The in-world black screen is a shadPS4 texture-cache defect" - superseded for
    `activity:in_world`: zero type-9 bipeds existed; there was never anything to draw (and the
    "`gg+0xD0` unfilled" framing is itself retracted - players arrive by sim-queue element `0x0D`).
31. The profile SOID as `0xD100000000000002` - a hand-read hex dump; it is `0xD100000000000201`.
32. "The fireteam crash is a missing identity / peers need valid account SOIDs" - it was
    `sceVoiceCreatePort` stubbed with no parameters in shadPS4; fixed in the fork.
33. "The 205 body is being read AS a family-5 update" - right defendant, wrong charge: 205's
    `0x80801941` field [0] NAMES the family-5 root; send `0x7FFFFFFFFFFFFFFF`.
34. "Slot `0x401` is a content package" - all three descriptor containers are eboot-resident.
35. The `0x58`-stride field-pool reading (stride, start bound, ten width values) - the table is
    self-delimiting; corrected twice.
36. "Bucket ids 0x11/0x12/0x13 have no bucket-definition row" - they are records 15/16/23,
    non-equippable, not unreachable.
37. "One cause for all four character-screen defects" - four independent mechanisms.
38. "`0x0079`/`0x007A` are subscriber registration" - they are `queuez_register`; the subscription
    request is `0x000C`.

---

## 13. Sources

| melted source | what it contributed |
|---|---|
| `historicaldocs/design/character-and-orbit.md` | the route, the gates, the account record, slot B / field 3 (§26.76), the roster/two-lists finding (§26.60, §26.66), the payload grammar (§26.67), the prologue skip (§26.71), the descriptor-format saga (§24), the laws (§26.51, §26.74) - and ~30 of the dead claims |
| `historicaldocs/design/character-lifecycle-messages.md` | the four lifecycle messages, FNV-1 id resolution, the create request/struct, the PSN online id, encoding rules, `s_login_character_info`, the char_list fix |
| `historicaldocs/design/rsat-character-object.md` | the family-3 character object, the serializer-oracle method, the 13-byte payload, the identity order, the dirty-bit map |
| `historicaldocs/design/investment-profile-contract.md` | the state machine (states 19-23), exit predicates, reason table, profile blob fields, gate/orbit/PvP-minimal, the 0x7B format |
| `historicaldocs/design/investment-content-map.md` | family schema tags read live, families 0/2 subscriptions, the log-blindness warning, the director path, the manifest era caveat |
| `historicaldocs/design/progression-table.md` | ordinal 117, the content-version dependence, the stat ordinals, the XP curve, the extraction instrument |
| `historicaldocs/design/level-light-customization.md` | the Level/Light/Customization source map, the 4/1 profile layout, the progression store, the override block, family 2's real identity |
| `historicaldocs/design/character-screen-consumers.md` | the two-screens/two-objects answer, the family-3 census, the peer-view re-layout, the translation-manager mirror |

Live sibling references: `design/GUARDIAN-IN-WORLD.md` (current spine),
`design/queuez-replication.md`, `design/items-and-gear.md`, `design/grants-and-unlocks.md`,
`design/signon-and-entitlement.md`, `design/bap-transport.md`, `design/activity-lifetime-state.md`.
