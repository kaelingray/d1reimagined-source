# QueueZ replication - the server-object model, the type namespace, and the message catalogue

**State.** The client is a replicated-object machine, not a request/response matcher: every gate this
project has crossed resolved to object state keyed `(family, type, key)` in the client's own
registry. The runtime type namespace is fully mapped (14,816 schema elements, nine registries, three
containers, one descriptor chain) and fully unnamed. The complete BAP/investment message catalogue is
extracted and controlled. Family 2 is the roster/player card (the blank orbit HUD), family 4 carries
account/profile/availability/unlocks (the blank character, the inert director). All five families
reach `state=3 REPLICATING` on a normal run.

## Contents

1. [The model: objects, not answers](#1-the-model-objects-not-answers)
2. [The coordinates: registry, two-hop reads, replication state](#2-the-coordinates)
3. [What each family actually is](#3-what-each-family-actually-is)
4. [The runtime type namespace](#4-the-runtime-type-namespace)
5. [The message catalogue](#5-the-message-catalogue)
6. [Case: the blank orbit HUD is family 2](#6-case-the-blank-orbit-hud-is-family-2)
7. [Case: the empty profile and the empty card](#7-case-the-empty-profile-and-the-empty-card)
8. [Case: director selectability](#8-case-director-selectability)
9. [Open [U]s](#9-open-us)
10. [Dead claims](#10-dead-claims)
11. [Sources](#11-sources)

---

## 1. The model: objects, not answers

Decision recorded 2026-08-07. Evidence `observed-live` (character-and-orbit §26.41-§26.62, now under
`design/character-and-investment.md`).

Every gate crossed or failed at during the signon push resolved to **object state**, never to an
answer:

| gate | what was assumed | what it actually was |
|---|---|---|
| account ready byte | a flag in the 503/206 response | a **cached registry lookup**: "is there a (class 4, type 0) object keyed by the account SOID?" (§26.41) |
| substate-B predicate 4 | an independent requirement | the **same lookup** as predicate 3 (§26.41) |
| character creation vs selection | answering a login message | the **count field of a family-3/type-0 roster object** (§26.60, §26.62) - read from the **account** object (`account + 0x08`) by `cui::c_investment_login_info_component_v2::vfunc[14]` at `0xA110F0` |
| exiting `character:signin` | a "selection accepted" reply | `record[+0x20]`, set from **an account-object field plus a family-4/type-1 object** (§26.60) |

`character:signin`'s handler reads **no response anywhere**. It reads the registry. `[C]`

The converse held all night: **every hypothesis authored from the wire was refuted**
(`0x0020 → 0x0021`, 501-as-span, `0x000C → 0x000D`) while **every hypothesis authored from the
client's own code held**. Three refutations, five confirmations, no exceptions either way.

⇒ A server built as `match msg_type { ... }` with a padded-empty fallback is the **wrong shape**. An
empty `investment_message_response` to message 701 *was* the disconnect (§26.51); to 501 it produces
an RST (§26.55). Silence must be a first-class modelled decision, delivery must be
subscription-driven, and the server should own objects keyed by the same `(family, type, key)`
coordinates the client's registry uses.

### 1.1 The message-id correction that rode on this

⛔ The original decision doc's table called investment id **501 `login_character`. It is not.**
**501 is `login_create_character`; `login_character` is 504.** Resolved from the FNV-1 name hash in
the shipped investment-message definition table and confirmed independently by the literal immediates
in the senders (`0x551040` → `0x1F5` = 501, `0x551640` → `0x1F8` = 504). The *claim* the row carried,
that creation vs selection is decided by an object count and not by answering a message, **stands and
is strengthened**. ⇒ `design/character-and-investment.md`.

### 1.2 Knowledge in env vars, and scaffolding

The env-var pile of the signon era (`D1_503_SOID`, `D1_503_RAW`, `D1_206_BODY`, `D1_PUSH_FAMILY`,
`D1_PUSH_AT`, `D1_INV_SILENT`, `D1_SUB_ACK`, `D1_INV_BODY_501`) encoded discoveries as knobs.
Knowledge that lives in shell history has no invariants: four already-won gates silently un-passed
in one evening (§26.45). `capture-rig/run-boot.sh` and `gate-check.py` are a splint, not a fix.

`capture-rig/inject-roster.py` writes objects into live client memory. It is a labelled scaffold: it
answered "is the roster the gate?" in one run after three hand-authored RSAT guesses failed (§26.62).
Anything reached through it is `synthetic_ancestor` twice over - the object never crossed a wire.

---

## 2. The coordinates

### 2.1 The registry lookup

Objects are keyed `(family, type, key)`. The finder `0x56CA70` is a **pure linear scan on each
slot's leading `u64`, with no per-slot occupancy flag** `[C]`:

```
0x56CAC7  mov  rsi, [rax]      ; the element's leading u64
0x56CACD  cmp  rsi, [rcx]      ; vs the key
0x56CAD0  jne  0x56CAC0        ; next slot
0x56CAD2  sub  rax, rdx ; ret  ; found
```

Its bounds come from a per-(family,type) entry table:

```
entry  = registry + family*0x60 + type*0x10 + 0x4F800
+0x00  u32 storage offset   (0xFFFFFFFF ⇒ the function returns 0 immediately)
+0x04  i16 slot count       (0 ⇒ the loop never executes)
+0x08  i32 stride
base   = registry + offset + 0x40A0
```

A non-zero leading `u64` in a slot IS a findable object.

### 2.2 The two-hop read

The roster widget reaches its data through a hardcoded **family-2 two-hop** `0x57F6F0` `[C]`:

```
0x57F708  cmp qword [r15],0 ; je 0x57F760     early-out on a zero contact key
0x57F720  call 0x56CA70(mgr, 2, 0, &contact[0])      hop 1: family 2/type 0 by contact key
0x57F738  cmp qword [rbx+8],0 ; je 0x57F760   2/0's +0x08 must be NON-ZERO
0x57F755  call 0x56CA70(mgr, 2, 1, &hop1[8])         hop 2: family 2/type 1 by 2/0's +0x08
```

⚠ The family and type arguments are never immediates: they are calls to two ICF-folded leaf stubs,
`0x1DF0` (`xor eax,eax; ret` ⇒ type 0) and `0x1048A40` (`mov eax,1; ret` ⇒ type 1), which the
decompiler mislabels. Reading the decompilation without resolving them hides every family/type
literal in the chain. `[C]`

A census of all **110** confirmed `call 0x56CA70` sites (five known sites as positive control) finds
no code that reads 2/0's `+0x08` as anything but a key or a presence predicate. `0x56CA70` compares
it with a bare `cmp rsi,[rcx]` - no mask, no SOID-kind test. **Any stable, non-zero, unique `u64`
works** as the hop key; its real-world form is `[U]`, only a capture closes it. `[C]`

⚠ Two other lookups share that key space: `0xA3DB55` looks up family 0/type 1 with the same key, and
`0x9E9AFD` (`cui::c_fireteam_details_component::vfunc[14]`) looks up family 2/type 1 keyed by family
6/type 0's `+0x08`. Pick values that cannot collide. `[C]`

### 2.3 Replication state, reconciled

The client's own diagnostic once read:

```
queuez_family_update_begin: ignoring family-update due to unhandled replication state,
    family_type=ENUM(4), family_root_soid=0xD100000000000001, replication state=1
```

`0x56AB40(.., mode=1)` sets state 1; `0x56BF80` only promotes 2 → 3; three static passes did not
find the 1 → N writer. This was generalised into "family 4 is stuck at state=1" and it **does not
hold** (⇒ §10 dead claims). Measured live 2026-08-09, run `20260809-084932-RUNG33-PERMS6`, with
`D1_ANSWER_FAMILIES=0,2` (families 3, 4, 5 **not** named), `capture-rig/d1guest.py descriptors`
sampled twice 5 s apart, identical both times:

```
[  0] class=0  key=0xd100000000000001  state=3 REPLICATING (update accepted)
[ 21] class=2  key=0xd100000000000001  state=3 REPLICATING (update accepted)
[252] class=3  key=0xd100000000000001  state=3 REPLICATING (update accepted)
[253] class=4  key=0xd100000000000001  state=3 REPLICATING (update accepted)
[254] class=5  key=0x7fffffffffffffff  state=3 REPLICATING (update accepted)
```

**All five families replicate, family 4 included, without being listed in `D1_ANSWER_FAMILIES`.**
The `state=1` sighting (run `20260808-045817-CREATE-ROSTER`) was the moment of a re-armed
**unsolicited push** on the create path: the first pushed baseline is accepted and later pushed
updates are ignored. `state=1` is a property of the push path at that instant, not the steady state.

⚠ Scope (2026-08-10): this must not be read as "`D1_ANSWER_FAMILIES` does nothing" -
**`D1_ANSWER_FAMILIES=0,2` is what cures `error.investment_peer_manager`** and unlocked the walk to
rung 36 `activity:in_world`. Different families, different gate, measured live.

### 2.4 Delta vs baseline, and invalidation

`0x565140` calls `0x56B5E0` unconditionally on accept and that tail chain reaches `0x574440(4)`
(the family-4 invalidator) without touching the per-object classifier `0x56B5F0`. A **baseline** at
replication state 3 goes to `0x56B2D0`, the **purge-to-keep-list** arm. ⇒ to re-trigger the family-4
invalidation chain, send a **DELTA** (`flags & 1 == 0`, `version == accepted + 1`), not a repeated
baseline - `D1_FAMILY4_INVALIDATE` in `d1-responder`, default `ping` (zero objects, purge-free).
The delta does not need a changed byte. `[C]`

---

## 3. What each family actually is

The registry mapper `0x56C550` is a two-level jump table whose leaves write four constants each; the
schema handle it stores is a `.data` slot → BSS object whose `+0x00` is the descriptor **instance
checksum** (`u32[anchor - 0x50 + 8]`). All recoverable offline. Families 0 and 2 have exactly two
types each. `[C]`

| family | type | schema tag | size | capacity | stride | instance checksum | what it is |
|---:|---:|---|---:|---:|---:|---|---|
| 0 | 0 | `0x80801921` | 80 | 20 | `0x50` | `0x1D7BB8F6` | peer roster card twin (identical 4-field tree to `0x808018B7`) |
| 0 | 1 | `0x808017FD` | 2632 = `0xA48` | 20 | `0xA48` | `0x67D97FC4` | peer rich view (the nameplate's rich fill) |
| 2 | 0 | `0x808018B7` | 80 | 231 | `0x50` | `0x3E3ACE74` | **roster contact card** (key, hop key, name, clan) |
| 2 | 1 | `0x80801810` | 48 | 231 | `0x30` | `0x0444D2D9` | **roster player card** (level, light, emblem index) |
| 3 | 0 | - | `0xA0` | - | - | `0xA2AC2C74` | character roster (count + SOIDs) - decides create vs select |
| 3 | 1 | `0x80801954` | `0xA80` = 2688 | - | - | `0x3431F1BA` | **character select card** (level u32, stat table, equipment summary) |
| 4 | 0 | `0x80801803` | `0x7338` | - | - | `0x64AE75BF` | account (span 7680 bits) |
| 4 | 1 | `0x80801743` | `0x7078` = 28792 | - | - | `0x8127F15A` | **character profile** (in-world identity, gear, progressions, stats; span 6173 bits) |
| 4 | 2 | `0x80800FB2` | 72 | - | - | - | availability input, keyed by the account SOID; `0x260200` hard-aborts without it |
| 4 | 3 | `0x80801732` | 184 | 666 | - | - | inventory elements (item instances) |
| 4 | 4 | `0x8080102E` | 3984 | - | - | `0xC4060975` | unlock state, key `0x7FFFFFFFFFFFFFFE`, 200 × `{u64 flag id, u6 value}` |

Notes:

* Capacities 231 and 20 are the mapper's own immediates (`0x56C735 mov dword [rcx+8],0xE7`,
  `0x56C708`/`0x56C57E mov dword [rcx+8],0x14`). "20 × 0xA48" and "2632 B" were never in conflict:
  `0xA48 = 2632`, the 20 is the slot count.
* The family 3/0, 3/1, 4/0, 4/1 checksums and sizes were measured off a live client
  (`observed-live`, §26.61).
* Family 2 was once believed to carry **progressions**; that reading is retracted (the historical
  `family2-replication-gate.md` is retracted as wrong-family). The `d1-responder` code that served
  family 2 was nevertheless the **right plumbing** - correct family, correct root, correct instance
  checksums for both `0x808018B7` and `0x80801810` - only its payloads and naming were wrong (⇒ §6.4).
* Family 4/3 element `+0x24` `u16` is the content index the definition lookup uses.
  ⇒ `design/items-and-gear.md`.

---

## 4. The runtime type namespace

**State:** fully mapped, fully deterministic, fully unnamed. 14,816 schema elements, nine
registries, three containers, one self-delimiting descriptor chain. Every runtime tag is predictable
**offline, from the eboot alone**. Reach: `static-client-inference` throughout, plus `observed-live`
for the 7,670-row live measurement of 2026-08-07 (pid 3382362). No part rests on our own server
answering, so nothing here carries `synthetic_ancestor`.

Re-derive, do not transcribe:

```
python3 capture-rig/namespace-walk-verify.py     # 72 checks, ~1 min, offline, exit 0 iff all pass
python3 capture-rig/type-registry.py --verify    # the binder replay, 30 checks incl. 3 controls
python3 capture-rig/verify-tag-determinism.py    # the offline-vs-live join + scramble control
```

### 4.1 Nine registries, named by their registrars `[C]`

`0xEE0D0(table, count)` appends `{ptr, count, 0}` to a global list at `0x25AB410`; exactly nine
references (four-form sweep, positive control `0x560D27 → 0x56FFB0` fired in the same run):

| module | table vaddr | entries | registrar ref |
|---|---|---:|---|
| `well_known` | `0x197F170` | 300 | `jmp` @`0xBEC846` |
| `foundation` | `0x1959320` | 308 | `jmp` @`0xBD9E06` |
| `engine` | `0x190E710` | 2992 | `call` @`0x36B9C8` |
| `ai` | `0x1961670` | 1189 | `jmp` @`0xBDFC56` |
| `investment` | `0x196D9A0` | 2007 | `jmp` @`0xBE6D76` |
| `renderer` | `0x1979F20` | 1014 | `jmp` @`0xBEB456` |
| **`spasaha`** | **`0x19B3530`** | **6338** | `call` @`0xC262E8` |
| **`spasaha_investment`** | **`0x19D0BD0`** | **1332** | `jmp` @`0xC2B2C6` |
| `audio` | - | 40 | `jmp` @`0xBECF08` |

15,520 table entries over 14,816 elements (a type may be enlisted by more than one module). The two
registries the project called "the two registries" are `spasaha` (A) and `spasaha_investment` (B).
Counts are immediates in the registrar prologues (`mov esi,0x18C2` = 6338, `mov esi,0x0534` = 1332).
The nine module names, recovered from the teardown calls into `0xEE2A0`, are **the only names
recovered anywhere in this namespace**. ⚠ `"ai"` is two characters; a `len > 2` identifier filter
silently drops it.

### 4.2 Record framing and the tag formula `[C]`

Registry records are `{u64 ptr, u32 name_hash, u32 pad}`, stride 16, ptr-first (0/6338 and 0/1332
unrelocated under ptr-first; hash-first leaves 1/1332 and pairs each hash with the following
record's pointer). The pointer field is relocated, not stored: the file word is zero and RELA
supplies the value.

The runtime tag is **computed, never stored**. The binder `0xEE100` (called exactly once, at
`0x1154`, with the global element list `0x129F950`) walks 14,816 static records and calls the tag
allocator `0x6E9B0`:

```
tag = 0x80000000 | ((container_id << 13) & 0x7FFE000) | (index & 0x1FFF)
```

⚠ The `& 0x7FFE000` mask is real (it was omitted from one agent's stated formula); immaterial for
the three real container ids but any future id ≥ `0x4000` would alias. Container ids are hardcoded
immediates: `0x400` "schema handle table 0", `0x401` "table 1", `0x402` "table 2" (each passed to
`0x70C60`, which stores `0x40000000 | id` at `container+0x34`). The element list:

```
relptr(0x129F950+0x20) -> 0x129F9A8   i64 count = 14816, records of 16 bytes at +8
record = { i64 self-relative ptr -> descriptor object; u16 container_index; u16 element_index; u32 pad }
```

Mechanically the tag IS written at runtime (all 7,670 registry `.bss` slots are NOBITS,
`0x1A09088..0x2F50DC0`; there is no in-file word). Semantically it IS a compile-time constant. Four
confirmations:

1. `[C]` Offline replay predicts every live value: 7670/7670 joined rows match on runtime tag,
   object vaddr, and name hash; scramble control (element index shifted +1/-1/+7) collapses to
   0/7668, 0/7668, 0/7656.
2. `[C]` The formula holds on the raw live tags for 7670/7670.
3. `[C]` The toolchain baked the tags in: all 6,721 header child tags and all 1,064 composite-field
   child tags resolve into the offline-computed 14,816-tag set, 0 unresolved, over 14,816 distinct
   tags. `[R]` (`type-registry.py --verify`.)
4. `[C]` Two live processes agree on `0x808033F4 → 0x1492550`.

⇒ `0x808033F4` (the span element, name hash `0xE2E25C2C`, registry-B index 1167, a 16-bit leaf at
`0x1492550`) is a stable, predictable identifier and may be hard-coded. A tag that "appears nowhere"
in a byte census is normal, not anomalous - tags appear as literals only where another descriptor
cites them as a child (`u32 0x808033F4` → 0 occurrences; control `u32 0xDEADBEEF` → 9;
`u32 0x80801825` → 1). `[I]` Cross-process stability is sampled at n = 2; no mechanism for
variation exists.

### 4.3 Containers and the descriptor chain `[C]`

Two populations, not to be conflated:

```
all 14816 elements (offline)            slot 0x400 4960   0x401 4274   0x402 5582
the 7670 A+B registry entries (live)    slot 0x400  273   0x401 4271   0x402 3126
live reverse enumeration                slot 0x400 4960   0x401 4274   0x402 5582   slot 0x403 empty
```

All three containers are eboot-resident schema handle tables; all 14,816 descriptor objects are
eboot-resident.

The descriptor table is **one self-delimiting chain**, not a fixed-stride pool. Length rule:
`length = size + 4` when `u32(O + size) == 0` (a leaf's tag-list NUL), else `length = size`. Walking
`0x13D8A40` forward yields 9188 objects terminating exactly at `0x14A2E00`
(lengths `{0x58: 6714, 0x60: 2472, 0x68: 2}`; size-field values `{0x54, 0x5C, 0x64}` only); the
remaining 5628 elements lie below, down to `0x12D97E0`. A stride-`0x58` model is wrong in **phase**
as well as stride: it reads every record 8 bytes low (the word `0x80800050` at the "record start" is
the last entry of the previous object's tag list), runs one record past the end, and manufactures a
phantom "kind 192" (`u32(0x14A2E00) = 0xC0`). The tell: the live registry resolves to `object`, and
`object = marker + 8`. The width column is a **serialized bit width**, not a C size: 323 distinct
values over the 9188 leaves, up to 2048008.

The span element descriptor, re-read from file (offsets object-relative, not marker-relative):

```
0x1492550 +0x00 size 0x54   +0x08 namehash 0xE2E25C2C   +0x10 childtag 0xFFFFFFFF (leaf)
          +0x14 width 16 bits   +0x18 align 8   +0x48 csize 0   +0x50 taglist [0x80800050]
```

### 4.4 RSAT schema records - one base, no ambiguity `[C]`

The record base is **8 below the instance checksum** (a direct code read: `0x56B650` resolves the
tag, then `0x56b6f8 mov ecx, dword ptr [rsi + 8]` compares `+0x08` against the wire
`instance_type`). Structural self-check: the `u64` at the record base equals
`end_of_fields - (base + 8)` for 1688 of 1688 records.

```
record base = descriptor_anchor - 0x50
  +0x00  u64  length of the rest of the record
  +0x08  u32  INSTANCE CHECKSUM     <-- the QueueZ instance_type on the wire
  +0x0C  u32  zero
  +0x10  u32  a class tag, SHARED between objects - NOT this object's schema tag
  +0x14  u32  decoded object size   (== anchor+0x14; the two must agree)
  +0x18  u32  8
  +0x48  i64  self-relative pointer -> +0x58
  +0x50  u32  0x80800050  \  the descriptor ANCHOR
  +0x54  u32  0x8080010A  /
  +0x58  u32  field count
  +0x60  u32  descriptor self tag   (0x80801954 character, 0x80801951 roster, ...)
  +0x64  u32  decoded object size
  +0x6C  u32  serialized span
  +0x80  field records, stride 0x28
```

Positive controls (all reproduced by `capture-rig/message-catalogue.py`):

| object | record | descriptor tag | checksum | fields | span |
|---|---|---|---|---:|---:|
| character | `0x1350750` | `0x80801954` | `0x3431F1BA` | 6 | 704 |
| roster | `0x139ADD0` | `0x80801951` | `0xA2AC2C74` | 4 | 4 |
| account (wrapper) | `0x12FA3F0` | `0x80801803` | `0x64AE75BF` | 1 | 7680 |
| profile (wrapper) | `0x1304800` | `0x80801743` | `0x8127F15A` | 1 | 6173 |

Character's five child tags: `808017FC / 8080181A / 80801755 / 80801855 / 80801923`.

### 4.5 Ruled out, one line each (do not re-open)

* **The registries' name-hash function** - closed. 113,180 strings × 52 functions × 20 transforms →
  noise-level hits; injection and scramble controls held. Only two string-hash routines exist in the
  client: FNV-1 over `tolower`'d bytes with `/`→`\` (7 sites incl. `0xBC980`) and zlib CRC-32
  (`0x2AD370`/`0x2AD3C0`). ⚠ Scope: this is true of the two TYPE-NAME registries only; the client's
  *name* namespace (enum members, bootflow states, error reasons, world/mode names) is **FNV-1
  32-bit** and `[C]` cracked.
* **`kind & 0x100` = repeated/array** - destroyed. The high byte is an optional-presence-bit
  **width**, `movzx`'d and **added** to the field's bit offset; measured histogram
  `kind>>8 = {0: 6234, 1: 602}`. No code masks or branches on `0x100` (control: `0x3ff` → 6 hits,
  `0x1fff` → 7).
* **"Which descriptor holds an array of `0x808033F4`"** - moot. The span is hard-coded by `0x261FB0`
  as a trailer (1 presence bit, then a 16-bit **byte length**, then opaque bytes), not described by
  any schema. Envelope `+0x18` is a byte length, not an element count.
* **`{u32 hash, pad, u64 ptr}` framing at `0x19D0BD8`** - rejected (§4.2).
* **"The tag is per-load; carry the name hash"** - refuted (§4.2).
* **`0xE2B769E8` is the span element's hash** - retracted; it is `0xE2E25C2C`. Root cause: decimal
  `42979504` is `0x28FD0B0`, once miswritten as `0x2900BF0` (a real address, registry-B[1224]'s
  slot), and that one bad conversion propagated through three lanes. Print addresses in one base only.
* **The 101 message-definition name hashes are drawn from this namespace** - no: 0 of 101 rows'
  `+0x00` hash is one of the 14,816 element hashes. Message names and type names are disjoint spaces.
* **The descriptor-table columns hide load-time pointers** - no byte of `0x13D8A40..0x14A2E00` is
  relocated (RELA minimum `r_offset` is `0x18C0038`).

⚠ Standing traps: this eboot has no `DT_SCE_RELASZ` (`0x61000030`) - the RELA size lives in tag
`0x61000031` (`0x1B0918` = 73825 × 24). Two distinct relocations (`0x19CC800`, `0x19D54C0`)
legitimately carry the same addend `0x28FD0B0`. Every registry `.bss` slot is NOBITS, so
"compare file word to live word" is vacuous - classify the address first, keep a FILE-resident
control (`0x1492550`) in the same run.

---

## 5. The message catalogue

Reach: `static-client-inference` throughout. Every number re-derivable by
`capture-rig/message-catalogue.py` (thirteen positive controls, non-zero exit on failure), against
`dumps/patch-01.29-CUSA00219/eboot.bin` plus the shipped definition table
`capture-rig/schemas/investment-message-definitions.bin`. `vaddr = file_offset - 0x4000`.
Wire framing lives in `design/bap-transport.md`.

### 5.1 The answer in one paragraph

There are **63 BAP envelope message classes** (of 304 possible `u16` ids) and **101 investment
message ids** inside envelope type `0x000A`. "Does the client want a reply?" is not a naming
convention and not `N → N+1`: it is a literal constant in a **policy** vtable, slot `+0x30`, and
**zero means no reply exists**. 13 of the 33 policy families and all 13 push policies carry zero.

### 5.2 The mechanism `[C]`

`c_investment_bap_message_queue::expected_reply_type` at `0x567EB0` (called by
`update_receiving_header` at `0xB2E116`):

```c
entry  = find_in_flight_entry(this, request_id);   // two ring buffers, 0x120/0x41C0
family = *(u32*)(entry + 4);                       // the POLICY FAMILY index, 0..32
policy = 0x55F400(family);                         // 0x1923CB0[family]
rsp    = policy->vtable[0x30](policy);             // 0x567F62  <-- THE EXPECTED RESPONSE TYPE
if (rsp == 0) return 0;                            // 0x567F67  <-- ZERO MEANS "NO REPLY EXISTS"
obj    = 0x559DD0(rsp);                            // g_bap_message_singletons[rsp]
return obj->vtable[0x00](obj);                     // that class's own type constant
```

Message type `0` is `none` and never appears on a wire, so a reply to a zero-policy message lands in
`"failed to find recipient for header"` →
`_connection_failure_failed_to_find_message_recipient` - the measured `0x00AB` (XETROV) behaviour,
now generalised.

Three dispatch paths:

| path | how a reply is matched | where defined |
|---|---|---|
| policy / message queue | `(expected_reply_type, sequence)` on the in-flight queue | 33 policies, `0x1923CB0`, slot `+0x30` |
| protocol_source (handshake) | request kind → reply type | `k_kind_to_reply_type`, `0x15D1B40`, indexed by `kind - 1`: kinds 1..5 → `0x001A`, `0x00FB`, `0x007A`, `0x012F`, `0x001F` |
| push | **type alone**, no sequence, no in-flight entry (`conn[0x1138]`) | 13 push-policy classes |

Header classes (`0x19234A0`, 8 entries):

| idx | class | size | flags (`vt+0x00`) |
|---:|---|---:|---|
| 0 | `c_bap_message_header` | 6 | - |
| 1 | `c_bap_message_client_request_header` | 6 | - |
| 2 | `c_bap_message_client_response_header` | 8 | `0x18` - bit `0x10` ⇒ carries a status, must be `200` |
| 3 | `c_bap_message_client_activity_request_header` | 14 | `0x04` |
| 4 | `c_bap_message_client_notification_header` | 6 | - |
| 5 | `c_bap_message_service_request_header` | 24 | `0x43` |
| 6 | `c_bap_message_service_activity_request_header` | 32 | `0x4F` |
| 7 | `c_bap_message_response_header` | 8 | `0x18` |

### 5.3 The 63 envelope classes `[C]`

Read from RTTI (name ← `typeinfo+8` ← `vtable-8`) plus constant thunks
(`+0x00` type, `+0x08` header index, `+0x10` encode, `+0x18` decode, `+0x20` type_name,
`+0x28` sub-message id, `0xFFFFFFFF` = none - a real function only for `0x000A`).

| type | class (RTTI, verbatim) | hdr | hdr bytes | role | response required? | reply |
|---|---|---:|---:|---|---|---|
| `0x0006` | `client_to_activity_host_manager_request` | 1 | 6 | request (policy 2) | **YES** | `0x0007` |
| `0x0007` | `client_to_activity_host_manager_response` | 2 | 8 | one-way (policy 3) | **NO - answering is harmful** | - |
| `0x0008` | `client_to_activity_host_notification` | 3 | 14 | one-way (policy 17) | **NO - answering is harmful** | - |
| `0x0009` | `bap_to_client_notification` | 0 | 6 | one-way (policy 9) | **NO - answering is harmful** | - |
| `0x000a` | `client_to_world_server_request` | 1 | 6 | request (policy 0,1) | **YES** | `0x000b` |
| `0x000b` | `client_to_world_server_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x000c` | `client_to_bap_subscription_request` | 1 | 6 | request (policy 13) | **YES** | `0x000d` |
| `0x000d` | `client_to_bap_subscription_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x000e` | `client_to_bap_unsubscribe_request` | 1 | 6 | request (policy 14) | **YES** | `0x000f` |
| `0x000f` | `client_to_bap_unsubscribe_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0010` | `client_to_bap_get_activity_host_proxy_request` | 1 | 6 | request (policy 22) | **YES** | `0x0011` |
| `0x0011` | `client_to_bap_get_activity_host_proxy_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0012` | `client_to_bap_client_config_request` | 1 | 6 | request (policy 15) | **YES** | `0x0013` |
| `0x0013` | `client_to_bap_client_config_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0014` | `bap_to_client_server_shutdown_notification` | 0 | 6 | one-way (policy 11) | **NO - answering is harmful** | - |
| `0x0015` | `client_to_bap_update_purchased_offers_request` | 1 | 6 | request (policy 23) | **YES** | `0x0016` |
| `0x0016` | `client_to_bap_update_purchased_offers_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0017` | `client_to_bap_account_id_translation_request` | 1 | 6 | request (policy 12) | **YES** | `0x0018` |
| `0x0018` | `client_to_bap_account_id_translation_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0019` | `client_to_bap_secure_hello_request` | 1 | 6 | handshake request (kind 1) | **YES** | `0x001a` |
| `0x001a` | `client_to_bap_secure_hello_response` | 2 | 8 | handshake reply (kind 1) | *is itself the reply* | - |
| `0x001b` | `bap_to_client_log_notification` | 0 | 6 | one-way (policy 10) | **NO - answering is harmful** | - |
| `0x001c` | `server_challenge_request` | 4 | 6 | **server push** (type-only dispatch) | **NO - answering is harmful** | - |
| `0x001d` | `server_challenge_response` | 4 | 6 | one-way (policy 24) | **NO - answering is harmful** | - |
| `0x001e` | `client_to_bap_channel_startup_request` | 1 | 6 | handshake request (kind 5) | **YES** | `0x001f` |
| `0x001f` | `client_to_bap_channel_startup_response` | 2 | 8 | handshake reply (kind 5) | *is itself the reply* | - |
| `0x0020` | `bap_to_client_user_message_request` | 1 | 6 | request (policy 25) | **YES** | `0x0021` |
| `0x0021` | `bap_to_client_user_message_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0022` | `client_to_bap_update_matchmaking_skill_request` | 1 | 6 | request (policy 26) | **YES** | `0x0023` |
| `0x0023` | `client_to_bap_update_matchmaking_skill_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0024` | `client_to_bap_friends_update_request` | 1 | 6 | request (policy 27) | **YES** | `0x0025` |
| `0x0025` | `client_to_bap_friends_generic_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0026` | `client_to_bap_party_invite_request` | 1 | 6 | request (policy 28) | **YES** | `0x0027` |
| `0x0027` | `client_to_bap_party_invite_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x0028` | `client_to_bap_check_party_invite_request` | 1 | 6 | request (policy 29) | **YES** | `0x0029` |
| `0x0029` | `client_to_bap_check_party_invite_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x002a` | `client_to_bap_update_matchmaking_core_request` | 1 | 6 | request (policy 30) | **YES** | `0x002b` |
| `0x002b` | `client_to_bap_update_matchmaking_core_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x002c` | `client_to_bap_clan_request` | 1 | 6 | request (policy 31) | **YES** | `0x002d` |
| `0x002d` | `client_to_bap_clan_response` | 2 | 8 | response class | *is itself the reply* | - |
| `0x002e` | `client_to_bap_nat_punch_request` | 1 | 6 | one-way (policy 32) | **NO - answering is harmful** | - |
| `0x002f` | `bap_to_client_nat_punch_relay_intro_notification` | 4 | 6 | **server push** (type-only) | **NO - answering is harmful** | - |
| `0x0064` | `activity_host_to_client_notification` | 6 | 32 | one-way (policy 6) | **NO - answering is harmful** | - |
| `0x0066` | `activity_host_to_client_log_notification` | 5 | 24 | one-way (policy 7) | **NO - answering is harmful** | - |
| `0x006e` | `service_to_world_server_request` | 5 | 24 | request (policy 8) | **YES** | `0x0070` |
| `0x0070` | `activity_host_to_world_server_response` | 7 | 8 | response class | *is itself the reply* | - |
| `0x0079` | `client_to_bap_queuez_register_request` | 1 | 6 | handshake request (kind 3) | **YES** | `0x007a` |
| `0x007a` | `client_to_bap_queuez_register_response` | 2 | 8 | handshake reply (kind 3) | *is itself the reply* | - |
| `0x007b` | `queuez_to_client_update_notification` | 4 | 6 | **server push** (type-only) | **NO - answering is harmful** | - |
| `0x007c` | `queuez_to_client_subscription_lost_notification` | 4 | 6 | **server push** (type-only) | **NO - answering is harmful** | - |
| `0x0096` | `bap_to_activity_host_manager_request` | 5 | 24 | request (policy 21) | **YES** | `0x0097` |
| `0x0097` | `bap_to_activity_host_manager_response` | 7 | 8 | response class | *is itself the reply* | - |
| `0x0099` | `bap_to_activity_host_socket_handoff_request` | 6 | 32 | **unbound** | `[U]` no policy, no push policy | - |
| `0x00a0` | `bap_to_activity_host_notification` | 6 | 32 | one-way (policy 18) | **NO - answering is harmful** | - |
| `0x00a1` | `bap_to_activity_host_request` | 6 | 32 | request (policy 4) | **YES** | `0x00a2` |
| `0x00a2` | `bap_to_activity_host_response` | 7 | 8 | one-way (policy 5) | **NO - answering is harmful** | - |
| `0x00ab` | `client_to_xetrov_request` | 1 | 6 | one-way (policy 19) | **NO - answering is harmful** | - |
| `0x00fa` | `client_to_bap_echo_request` | 1 | 6 | handshake request (kind 2) | **YES** | `0x00fb` |
| `0x00fb` | `client_to_bap_echo_response` | 2 | 8 | handshake reply (kind 2) | *is itself the reply* | - |
| `0x012c` | `client_to_bap_initiate_relay_connection_notification` | 1 | 6 | one-way (policy 16) | **NO - answering is harmful** | - |
| `0x012d` | `bap_to_client_request_relay_connection_notification` | 4 | 6 | **server push** (type-only) | **NO - answering is harmful** | - |
| `0x012e` | `client_to_bap_register_relay_client_request` | 1 | 6 | request (policy 20) | **YES** | `0x012f` |
| `0x012f` | `client_to_bap_register_relay_client_response` | 2 | 8 | handshake reply (kind 4) | *is itself the reply* | - |

Push types a server may legally send **unsolicited** (matched by type alone, no sequence, no
in-flight entry): `0x0006`, `0x0008`, `0x0009`, `0x0014`, `0x001B`, `0x001C`, `0x002F`, `0x0064`,
`0x007B`, `0x007C`, `0x00A0`, `0x00A1`, `0x012D`. `0x007B` is the QueueZ object push in use;
`0x007C` (subscription lost) has never been sent. `0x001C` (server challenge) is the only push in
the `0x00`-`0x2F` range never seen; its answer `0x001D` is sent by the client and must **not** be
acknowledged. `0x0099` is the single unbound class - most likely server-side-only; do not send it.
`0x0003` (`goodbye`) and `0x0000` (`none`) have log names but **no message class** - unreachable in
this build.

Corrections to the older log-string-mapper reading, all `[C]`: only `0x1b` is the log notification
(`0x1c`/`0x1d` are the challenge pair sharing its log string); the "gaps" `0x24`-`0x29` are six real
classes with no log string; fifteen further ids
(`0x64 0x66 0x6e 0x70 0x79 0x7a 0x7c 0x7b 0x96 0x97 0x99 0xa0 0xa1 0xa2 0xab`) have classes but no
log string. `0x007A` = `client_to_bap_queuez_register_response` retires the old `[U]` on kind 3's
reply.

### 5.4 The mapper, executed - and the naming wall

`FUN_00B32410(id) -> char*` was executed and swept (2026-08-09): it names **60** ids, all `<= 303`,
all envelope-level log labels (`c->ws req`, `b->c not`, ...). ⛔ `UNKNOWN` for all ten investment
controls (201-206, 501-506). ⭐⭐ **Two disjoint id namespaces:** envelope `0..303` (60 names, in the
eboot) vs investment `104..1901` (101 ids, zero names, FNV-1 hashes only) - and **id 303 exists in
both, meaning different things**. `0x559D30` is **not an ICF twin** of the mapper; it is a thunk at
vtable `+0x20` computing `name_of(this->vfunc[0]())` (`mov rax,[rdi]` / `call [rax]` /
`mov edi,eax` / `jmp 0xb32410`). Sweep control: scanning all 47,304 decompiled functions for the
mapper shape returns exactly two files, both this function ⇒ there is no second mapper.

⛔ `nametab`: 941 of 1080 entries are `fnv1("")` - the enum name tables **shipped empty**. Inverted
control: `d1-eboot strings` computes `fnv1("fetch_family")` correctly, matches the known
`name_hash`, and still reports "no corpus spelling" - the oracle fails on a known-correct answer, so
a null is corpus **absence**, not search failure. 58,960 harvested spellings hashed against all 101
`name_hash` values → one match, the injected control. ⇒ **Guess-and-check FNV is proven futile.**
`0x2DCC40D2` (inv 105), `0x5A146EAB` (104), `0xA52AE87F` (205) stay `[U]`; only the X360 ALPHA can
close them. Better names exist free: 33 policy display strings at `0x1923AA0` are full spellings,
and RTTI names all 63 envelope classes.

### 5.5 The 33 policy families `[C]`

`0x55F400(i)` → singleton (`0x1923CB0[i]`); `0x55F410(i)` → display string (`0x1923AA0[i]`).
Slot `+0x10` = request type, slot `+0x30` = response type, `0` = no response.

| fam | request | response | display string | verdict |
|---:|---|---|---|---|
| 0 | `0x000A` | `0x000B` | `client_to_world_server_nondeterministic_request` | reply required |
| 1 | `0x000A` | `0x000B` | `client_to_world_server_deterministic_request` | reply required |
| 2 | `0x0006` | `0x0007` | `client_to_activity_host_manager_request` | reply required |
| 3 | `0x0007` | `0x0000` | `client_to_activity_host_manager_response` | one-way |
| 4 | `0x00A1` | `0x00A2` | `bap_to_activity_host_request` | reply required |
| 5 | `0x00A2` | `0x0000` | `bap_to_activity_host_response` | one-way |
| 6 | `0x0064` | `0x0000` | `activity_host_to_client_notification` | one-way |
| 7 | `0x0066` | `0x0000` | `activity_host_to_client_log_notification` | one-way |
| 8 | `0x006E` | `0x0070` | `service_to_world_server_request` | reply required |
| 9 | `0x0009` | `0x0000` | `bap_to_client_notification` | one-way |
| 10 | `0x001B` | `0x0000` | `bap_to_client_log_notification` | one-way |
| 11 | `0x0014` | `0x0000` | `bap_to_client_server_shutdown_notification,` *(shipped trailing comma)* | one-way |
| 12 | `0x0017` | `0x0018` | `client_to_bap_account_id_translation_platform_to_investment_request` | reply required |
| 13 | `0x000C` | `0x000D` | `client_to_bap_subscription_request` | reply required |
| 14 | `0x000E` | `0x000F` | `client_to_bap_unsubscribe_request` | reply required |
| 15 | `0x0012` | `0x0013` | `client_to_bap_client_config_request` | reply required |
| 16 | `0x012C` | `0x0000` | `client_to_bap_initiate_relay_connection_notification` | one-way |
| 17 | `0x0008` | `0x0000` | `client_to_activity_host_notification` | one-way |
| 18 | `0x00A0` | `0x0000` | `bap_to_activity_host_notification` | one-way |
| 19 | `0x00AB` | `0x0000` | `client_to_xetrov_notification` | **one-way - the XETROV law** |
| 20 | `0x012E` | `0x012F` | `client_to_bap_register_secure_address_request` | reply required |
| 21 | `0x0096` | `0x0097` | `bap_to_activity_host_manager_request` | reply required |
| 22 | `0x0010` | `0x0011` | `client_to_bap_get_activity_host_proxy_request` | reply required |
| 23 | `0x0015` | `0x0016` | `client_to_bap_update_purchased_offers_request` | reply required |
| 24 | `0x001D` | `0x0000` | `client_to_bap_challenge_response` | one-way |
| 25 | `0x0020` | `0x0021` | `bap_to_client_user_message` | reply required - see §5.7 |
| 26 | `0x0022` | `0x0023` | `client_to_bap_matchmaking_skill_request` | reply required |
| 27 | `0x0024` | `0x0025` | `client_to_bap_friends_update_request` | reply required |
| 28 | `0x0026` | `0x0027` | `client_to_bap_friends_party_invite_request` | reply required |
| 29 | `0x0028` | `0x0029` | `client_to_bap_friends_check_party_invite_request` | reply required |
| 30 | `0x002A` | `0x002B` | `client_to_bap_matchmaking_core_request` | reply required |
| 31 | `0x002C` | `0x002D` | `client_to_bap_clan_request` | reply required |
| 32 | `0x002E` | `0x0000` | `client_to_bap_nat_punch_intro_request` | one-way |

⚠ The display string is not the class name: 29 of 33 match. Families **0 and 1** carry *swapped*
display names relative to their RTTI classes (`[U]` which is authoritative - both map
`0x000A → 0x000B`, so the catalogue is unaffected, but do not decide anything about determinism from
it). Family 20's string says `register_secure_address`, its class says `register_relay_client`.
Prefer RTTI.

### 5.6 The investment sub-id space - shipped content, not code `[C]`

The 101 investment messages ride inside envelope `0x000A` with their own ids (104..1901). The
definition table is content:

```
symbol "investment_globals" -> pkg 300 entry 1 -> +0x578 = tag 0x80A5E37A
tag 0x80A5E37A -> pkg 303 entry 890 (6528 B) = THE DEFINITION TABLE
container = entry+0x18 : i32 count = 101 @+0x00 ; self-relative array @+0x08 ; records base +0x30
record 0x40 bytes, exactly four non-zero words:
+0x00 u32 name hash   +0x10 u32 message id   +0x14 u32 request tag   +0x18 u32 response tag
```

⚠ `0x27FDD0` is **not** an id→index map - it is a linear search over `mgr_vt[0x4D0](i)` rows
comparing `word[row+0x10]`. There is no static id table in the eboot to read.

**Response required, for investment ids - a firm negative** `[C]`: every one of the 101 rows has
both a request tag and a response tag, all non-zero, all distinct (202 distinct tags, all resolving
to real descriptors). No zero, no sentinel, no direction bit. The question is answered one layer up,
uniformly: policy families 0 and 1 both map `0x000A → 0x000B`, so **the client has an in-flight
entry waiting for a `0x000B` for every investment request it sends. Silence is a stall, not a legal
outcome.** `[U]` whether individual ids tolerate an *empty* `0x000B` body - 81 of the 101 response
schemas have exactly one top-level field and decode to an 8-byte or 1-byte object, but small is not
empty.

The 101 ids joined to their schemas (`f` = top-level field count, size = decoded C object size):

| row | id | request tag | f | req size | response tag | f | rsp size |
|---:|---:|---|---:|---:|---|---:|---:|
| 47 | **104** | `0x80801811` | 1 | 1 | `0x808018f8` | 2 | 12 |
| 48 | **105** | `0x808018af` | 1 | 2 | `0x80801702` | 1 | 8 |
| 56 | **201** | `0x8080170b` | 1 | 8 | `0x80801813` | 2 | 29504 |
| 58 | **202** | `0x8080192b` | 1 | 8 | `0x808018eb` | 2 | 28800 |
| 60 | **203** | `0x8080177c` | 2 | 16 | `0x808016d9` | 2 | 192 |
| 61 | **204** | `0x80801748` | 0 | 1 | `0x808017c9` | 2 | 16 |
| 62 | **205** | `0x80801874` | 0 | 1 | `0x80801827` | 2 | 952 |
| 59 | **206** | `0x80801774` | 2 | 16 | `0x80801869` | 1 | 1 |
| 57 | **207** | `0x8080182d` | 1 | 8 | `0x808016ca` | 2 | 36 |
| 63 | **208** | `0x808019b7` | 1 | 8 | `0x808019b1` | 2 | 80 |
| 64 | **209** | `0x80801a15` | 1 | 8 | `0x808017a7` | 2 | 1632 |
| 65 | **211** | `0x80801a19` | 1 | 8 | `0x808019c9` | 2 | 32 |
| 68 | **303** | `0x808016fd` | 2 | 16 | `0x808016ec` | 1 | 8 |
| 66 | **304** | `0x80801802` | 3 | 736 | `0x8080188c` | 1 | 1 |
| 67 | **305** | `0x8080184e` | 3 | 736 | `0x808016ea` | 1 | 1 |
| 69 | **402** | `0x80801767` | 2 | 32 | `0x80801953` | 1 | 8 |
| 70 | **403** | `0x80801942` | 1 | 8 | `0x80801834` | 1 | 8 |
| 71 | **404** | `0x8080189a` | 1 | 8 | `0x808018ab` | 1 | 8 |
| 72 | **405** | `0x808018ce` | 5 | 24 | `0x80801719` | 1 | 8 |
| 73 | **406** | `0x80801a03` | 2 | 32 | `0x80801a40` | 1 | 8 |
| 74 | **501** | `0x80801860` | 3 | 60 | `0x8080185d` | 2 | 16 |
| 75 | **502** | `0x80801939` | 1 | 8 | `0x808017f9` | 1 | 8 |
| 76 | **503** | `0x80801726` | 1 | 48 | `0x80801825` | 5 | 968 |
| 77 | **504** | `0x808017ce` | 1 | 8 | `0x80801845` | 1 | 8 |
| 78 | **505** | `0x808017e5` | 0 | 1 | `0x80801886` | 1 | 8 |
| 79 | **506** | `0x80801814` | 3 | 40 | `0x80801736` | 1 | 8 |
| 80 | **601** | `0x80801757` | 2 | 32 | `0x808017ab` | 2 | 32 |
| 82 | **701** | `0x808016dd` | 1 | 1088 | `0x8080189b` | 1 | 8 |
| 83 | **702** | `0x808016cf` | 1 | 2592 | `0x8080194d` | 1 | 8 |
| 92 | **801** | `0x8080171f` | 2 | 16 | `0x80801720` | 1 | 8 |
| 94 | **802** | `0x8080174d` | 2 | 16 | `0x8080183d` | 1 | 8 |
| 95 | **803** | `0x8080184a` | 3 | 16 | `0x80801792` | 1 | 8 |
| 93 | **804** | `0x80801a1f` | 3 | 24 | `0x808019ea` | 1 | 8 |
| 96 | **901** | `0x80801777` | 3 | 16 | `0x80801888` | 2 | 12 |
| 97 | **902** | `0x8080180d` | 2 | 32 | `0x808016ef` | 1 | 8 |
| 99 | **903** | `0x80801734` | 3 | 32 | `0x8080183a` | 1 | 8 |
| 100 | **904** | `0x808019ee` | 4 | 12 | `0x808019b8` | 1 | 8 |
| 98 | **905** | `0x80801e76` | 4 | 32 | `0x808019bc` | 1 | 8 |
| 89 | **1101** | `0x80801718` | 1 | 4 | `0x8080188d` | 1 | 1 |
| 90 | **1102** | `0x808018a4` | 1 | 4 | `0x8080187e` | 1 | 1 |
| 88 | **1103** | `0x80801799` | 1 | 1 | `0x8080175c` | 2 | 8 |
| 2 | **1201** | `0x80801742` | 1 | 8 | `0x80801817` | 1 | 8 |
| 3 | **1202** | `0x8080172f` | 7 | 20 | `0x8080186f` | 1 | 8 |
| 5 | **1203** | `0x808016e9` | 2 | 8 | `0x808016c1` | 1 | 8 |
| 14 | **1205** | `0x808017b8` | 1 | 404 | `0x80801859` | 1 | 8 |
| 16 | **1206** | `0x808017f4` | 3 | 12 | `0x8080174b` | 1 | 8 |
| 20 | **1207** | `0x8080175d` | 2 | 4 | `0x80801895` | 1 | 8 |
| 21 | **1208** | `0x808017eb` | 0 | 1 | `0x8080175b` | 1 | 8 |
| 15 | **1209** | `0x80801815` | 1 | 8 | `0x8080190e` | 1 | 8 |
| 8 | **1210** | `0x808016e6` | 0 | 1 | `0x80801723` | 1 | 8 |
| 9 | **1211** | `0x808017d2` | 0 | 1 | `0x80801958` | 1 | 8 |
| 7 | **1212** | `0x80801880` | 2 | 8 | `0x80801775` | 1 | 8 |
| 10 | **1213** | `0x8080170a` | 5 | 10 | `0x808016cd` | 1 | 8 |
| 22 | **1214** | `0x80801709` | 1 | 2 | `0x8080180f` | 1 | 1 |
| 11 | **1215** | `0x808017ad` | 1 | 129 | `0x808017bc` | 1 | 8 |
| 23 | **1216** | `0x808018a9` | 2 | 8 | `0x808017d7` | 1 | 8 |
| 28 | **1218** | `0x8080181b` | 3 | 736 | `0x80801926` | 1 | 1 |
| 6 | **1219** | `0x80801960` | 1 | 1 | `0x808017cf` | 1 | 8 |
| 12 | **1220** | `0x8080193e` | 0 | 1 | `0x808018f7` | 1 | 8 |
| 24 | **1221** | `0x8080188e` | 1 | 1 | `0x8080182c` | 1 | 8 |
| 4 | **1222** | `0x808018f0` | 2 | 8 | `0x808018bc` | 1 | 8 |
| 29 | **1223** | `0x808017bf` | 5 | 32 | `0x80801940` | 1 | 8 |
| 18 | **1224** | `0x80801901` | 3 | 3 | `0x80801884` | 1 | 8 |
| 30 | **1225** | `0x8080187b` | 2 | 8 | `0x80801835` | 1 | 1 |
| 31 | **1226** | `0x808018e0` | 2 | 8 | `0x8080172e` | 1 | 1 |
| 19 | **1227** | `0x80801858` | 3 | 3 | `0x808016d6` | 1 | 8 |
| 26 | **1228** | `0x808019c5` | 0 | 1 | `0x80801a3a` | 1 | 8 |
| 27 | **1229** | `0x808016d5` | 3 | 16 | `0x80801a08` | 1 | 8 |
| 32 | **1230** | `0x808019db` | 1 | 4 | `0x80801a24` | 1 | 8 |
| 33 | **1231** | `0x808017ef` | 2 | 8 | `0x80801a12` | 1 | 1 |
| 36 | **1232** | `0x808019ac` | 1 | 256 | `0x80801a05` | 1 | 1 |
| 37 | **1233** | `0x808019cf` | 1 | 2 | `0x808019b0` | 1 | 1 |
| 17 | **1236** | `0x8080103b` | 4 | 16 | `0x8080103c` | 1 | 8 |
| 34 | **1237** | `0x8080103d` | 2 | 258 | `0x8080103e` | 1 | 1 |
| 25 | **1238** | `0x80801037` | 5 | 5 | `0x80801038` | 1 | 8 |
| 35 | **1239** | `0x8080103f` | 2 | 260 | `0x80801040` | 1 | 1 |
| 13 | **1240** | `0x80801039` | 1 | 256 | `0x8080103a` | 1 | 8 |
| 50 | **1301** | `0x80801746` | 4 | 16 | `0x808016e7` | 1 | 1 |
| 51 | **1302** | `0x8080173f` | 0 | 1 | `0x8080185b` | 1 | 1 |
| 49 | **1303** | `0x8080186e` | 6 | 140 | `0x808017aa` | 10 | 1576 |
| 52 | **1304** | `0x8080195e` | 4 | 24 | `0x8080193d` | 4 | 4 |
| 53 | **1305** | `0x8080199a` | 0 | 1 | `0x80801a34` | 3 | 216 |
| 54 | **1306** | `0x8080102f` | 0 | 1 | `0x80801030` | 2 | 3120 |
| 55 | **1307** | `0x808019ca` | 1 | 784 | `0x808019d9` | 1 | 8 |
| 81 | **1401** | `0x80801703` | 0 | 1 | `0x8080194a` | 1 | 8 |
| 87 | **1501** | `0x80801818` | 21 | 144 | `0x808018a8` | 1 | 1 |
| 38 | **1601** | `0x808019b9` | 2 | 64 | `0x808019bd` | 2 | 16 |
| 39 | **1602** | `0x808019c4` | 2 | 16 | `0x80801a2b` | 1 | 8 |
| 40 | **1606** | `0x80801957` | 1 | 8 | `0x80801a36` | 1 | 8 |
| 41 | **1609** | `0x808019f3` | 1 | 8 | `0x80801a28` | 1 | 8 |
| 42 | **1610** | `0x80801e82` | 2 | 16 | `0x80801a0d` | 1 | 8 |
| 43 | **1611** | `0x80801a35` | 3 | 24 | `0x808019bf` | 1 | 8 |
| 44 | **1612** | `0x808019ba` | 2 | 24 | `0x808019b6` | 1 | 8 |
| 45 | **1613** | `0x808019dd` | 2 | 16 | `0x808019f1` | 1 | 8 |
| 46 | **1614** | `0x808019c8` | 2 | 64 | `0x808019c7` | 1 | 8 |
| 0 | **1701** | `0x80801033` | 2 | 784 | `0x80801034` | 1 | 8 |
| 1 | **1702** | `0x80801035` | 1 | 520 | `0x80801036` | 1 | 8 |
| 84 | **1801** | `0x80801041` | 1 | 2 | `0x80801042` | 1 | 8 |
| 86 | **1802** | `0x80801043` | 1 | 1 | `0x80801044` | 1 | 8 |
| 85 | **1803** | `0x80801046` | 3 | 4 | `0x80801047` | 1 | 8 |
| 91 | **1901** | `0x8080104d` | 2 | 48 | `0x8080104e` | 1 | 8 |

Load-bearing ids: **501 `login_create_character` · 504 `login_character`** (§1.1). Payload-shape
joins `[I]` (coincidence-of-numbers, not wire-confirmed): 201's response span word is **7680** = the
account object's span exactly; 202's is **6173** = the profile's; 205 shares span 209 with 503's
response `0x80801825`.

⚠ The serialized-span word (`anchor+0x1C`) is confirmed as the RSAT dirty-field span for the four
QueueZ controls (roster 4, character 704, account 7680, profile 6173) but reads `0` for most
*message* schemas whose children plainly carry bits. `[U]` what it means for message schemas - do
not publish it as "span (bits)" for them.

### 5.7 The `0x0020` caveat: policy says *accept*, not *correct*

A wire measurement once concluded "`0x0020` has no paired response class". **The binary contradicts
that** `[C]`: class `0x0021` exists and policy 25 maps `0x0020 → 0x0021`. The measurement stands
(answering made things worse: closed at 650 B / 4 msgs vs surviving to 2477 B / 14) but the
explanation is replaced: `0x0021::decode_payload` (`0x5699A0`) is a **protobuf parse** producing a
648-byte object (`mov dword [r14], 0x288`) using the message descriptor at `0x15AB800`
(`0x5699D3: lea rsi, [rip + 0x1041E26]`, fed to the parser at `0xA2020`). An empty or invented
`0x0021` body is a malformed reply to a message genuinely in flight. **Keep the silence; do not
implement `0x0021` until the protobuf is decoded.** Generally: `response required = YES` is a
statement about the client's dispatcher, not a licence to invent a body.

### 5.8 Crate cross-check `[C]`

All 20 `TYPE_*` constants in `d1-server/crates/d1-bap/src/lib.rs` map to real classes with correct
values; zero disagreements. Naming notes: `TYPE_REGISTER_SUBSCRIBER_REQUEST/_RESPONSE`
(`0x0079`/`0x007A`) is misleading - it is the *QueueZ register*, not a subscriber registration
(`0x000C` is the subscription request). No constants exist for `0x007C` or `0x001C`/`0x001D`
(`[U]`, gaps not bugs).

---

## 6. Case: the blank orbit HUD is family 2

Built 2026-08-08 against the pinned 01.29 eboot and live orbit client pid 3071830, run
`runs/20260808-213619-UNLOCKS-ALL`. Live reads `[O] observed-live` with reach `synthetic_ancestor`
(our own server drove the client to orbit); instruction structure `[C] static-client-inference`.
⚠ Every heap address is per-run; eboot vaddrs are stable.

### 6.1 The one-line answer

> The orbit roster plate's level, light and emblem are never written, because
> `cui::c_roster_contact_id_container` binds to nothing - and it binds to nothing because
> `d1-responder` answered family 2's subscription with `object_count = 0`.

Both fill paths in `cui::c_roster_contact_widget`'s fill (`0xA3D4E0`) key on
`qword[container+0x08]`, written by **exactly one expression in the binary** -
`0xA3F0C1 mov rcx,[rax] ; 0xA3F0C4 mov [r14+8],rcx`, where `rax` is the family-2 two-hop `0x57F6F0`
(§2.2). Family 2 empty ⇒ two-hop NULL ⇒ `0xA3F0E9` stamps the container family byte **`0xFF`** ⇒
control reaches `0xA3DD68 call 0xA2DBC0(nameplate, 0xFFFF)` - the **black emblem** - and returns
without touching `nameplate+0x180` (light) or `+0x188` (level).

The client said so itself (`runs/20260808-213619-UNLOCKS-ALL/emulator.log:17436`):

```
networking:investment:push:queuez: object-count of zero in
  [family_type ENUM(2), family_root_soid 0xD100000000000001, version 0, flags 0x01, object_count 0]
```

Census of that run's QueueZ family transactions: family 0 `object_count 0`, family 2
`object_count 0`, family 4 `object_count 1`. Family 2 was registered, rooted on the account SOID,
at `state=3 REPLICATING`, `ver=0` - subscribed and never delivered into. `[O]`

### 6.2 The measured chain

* `0x581900` gate: `if (0x78DEE0() == 3 && DAT_026a37c8 != 0) return DAT_026a37c8;` - measured
  `DAT_026a37c8 = 0x20500f0d0` (non-NULL), `DAT_026b6630 = 3`. The gate passes; the two-hop was
  called and missed. `[O]` (⭐ `0x581900`'s return is a non-NULL **gate only** - `0x57F6F0` never
  reads its `rdi`; the contact is `0xA3F000`'s own `param_2`. `[C]`)
* The roster store, read live at `0x20500f0d0`: `+0x00` vptr `0x801927950`
  (`c_roster_account_translation_callback`), `+0x20` count = 1, `+0x28` contact key
  `0xD100000000000001` = the account SOID, `+0x30` `"D1Guardian"`. Contact stride `0x68`
  (`0x57E8BE imul r15, rax, 0x68`; key at `store + 0x28 + i*0x68`, `0x57E940`). `[O]`/`[C]`
* Family 2 registry entries confirmed live: `obj 2:0` stride `0x50` = 80 B = `0x808018B7`;
  `obj 2:1` stride `0x30` = 48 B = `0x80801810`; expected `count = 231`. Positive control in the
  same session: `obj 3:1` and `obj 4:1` read populated bytes with the same reader - the zeros are a
  measurement, not a dead reader. `[O]` + `[C]`
* Container reads (three instances found via `find-in-guest.py 0x801946ba0`,
  `cui::s_player_nameplate`; `[r15] = container`, `r15+8 = nameplate`): all three carry
  `+0x10 = 0xFF` (the NULL-branch literal). Two carry `"D1Guardian"` and flags `0x17`; the third has
  flags `0x00` and blanks earlier at `0xA3DAE8`. Nameplate `+0x180..+0x19f` zero except
  `+0x198 = 1.0f`. `[O]`
* ⛔ Do NOT poke the container: `cui::c_orbit_roster_component::vfunc[14]` (`0xA2A300`) rebuilds it
  from `0xA3F000` every update. The registry is the only durable poke target. `[C]`

### 6.3 The four gates of `0xA3D4E0`, in order `[C]`

| # | instruction | tests | on failure |
|---|---|---|---|
| A | `0xA3DAE4 test byte [rax+0x50],2 ; 0xA3DAE8 je` | contact flags | full blank, `0xA3DD55` |
| B | `0xA3DD05 cmp qword [rbp-0x1b8],0 ; 0xA3DD14 je` | the `0x548440()` content handle (`return DAT_026A0910` - not a server object; QueueZ can neither satisfy nor break it) | full blank |
| C | `0xA3DD16 test r12,r12 ; 0xA3DD19 je 0xA3DD5F` | ⭐ **family 2 / type 1** | `0xA3DD68` black emblem, neither fill runs - *the gate that fires* |
| D | `0xA3DD1B test rsi,rsi ; 0xA3DD1E je 0xA3DD72` | family 0 / type 1 | falls through to the compact fill |

⇒ **Family 2 alone is sufficient. Do NOT also send family 0.** When family 0/type 1 resolves,
`0xA3D4E0` runs the **rich** fill `0xA2D9F0` **instead of** the compact `0xA2DE30` - a substitution,
not an augmentation - so an empty or half-built family-0 object would blank a plate that family 2
alone would have filled. Exactly one field is rich-only: nameplate `+0x184`, which the compact fill
hardcodes to zero (`0xA2DF12 vxorps xmm0,xmm0,xmm0`). `[C]`

### 6.4 The fix: two roster objects (and the plumbing already existed)

`d1-responder`'s `build_family2_body` was fed *progression* payloads on the retracted
"family 2 = progressions" reading - but the code is the **right code**: correct family, correct root
(`0xD100000000000001` = the roster contact key), correct instance checksums
(`PROGRESSION_ROOT_INSTANCE_TYPE = 0x3E3ACE74` for 2/0, `PROGRESSION_INSTANCE_TYPE = 0x0444D2D9`
for 2/1). Keep the plumbing; rename it; replace the payloads.

**family 2 / type 0 - `0x808018B7`, 80 B:**

| offset | field | value |
|---|---|---|
| `+0x00` | `u64` key | `0xD100000000000001` - the roster contact key = the account SOID |
| `+0x08` | `u64` | the key into family 2/type 1; any stable non-zero value, e.g. `0xD100000000000501` |
| `+0x10` | `[27] u16` UTF-16 | display name - **first character, not a length prefix** (both readers pass `obj + 0x10` straight in as `src`) `[C]` |
| `+0x46` | `[5] u16` UTF-16 | clan tag |

**family 2 / type 1 - `0x80801810`, 48 B** - every leaf pinned by an instruction operand:

| offset | schema | reader | meaning |
|---|---|---|---|
| `+0x00` | `u64` | `0x56CAC7` | key = 2/0's `+0x08` |
| `+0x08` | `u16` | - | `[U]` |
| `+0x0A` | `u16` | `0x9E9B11 cmp word [rbx+0xa],0xffff ; je`, then `0x9E9B41 call 0x548860(investment_globals, idx)` | a content index with the `0xFFFF` sentinel; different index space from the emblem (8-byte entries); ship `0xFFFF` `[C]` |
| `+0x0C` | `u8` | `0xA3DD9A mov al,[r12+0xc]` → `widget+0x1c0` | `[U]` |
| `+0x10` | `u32` | `0xA2DE67 mov r13d,[r15+0x10]` → `0xA2DE93 mov [rbx+0x188]` | ⭐ **LEVEL** |
| `+0x14` | `u32` | `0xA2DE56 mov r12d,[r15+0x14]` → `0x281FA0` → `0xA2DE76 mov [rbx+0x180]` | ⭐ **LIGHT (raw)** - `0x281FA0` interpolates, so raw 400 may not display as 400 `[U]` |
| `+0x18` | `f32` | `0xA3DD72 vmovss xmm0,[r12+0x18]`; `0xA2DEC6` → `[rbx+0x190]`, frac → `+0x194` | XP |
| `+0x1C` | `u32` | `0xA3DD82 mov eax,[r12+0x1c]` → `[rbx+0x1b0]`; `0xA2DEF1` → `[rbx+0x18c]` | `[U]` |
| `+0x20` | `u16` | `0xA2DF72 movzx esi,word [r15+0x20]` → `0xA2DF7A call 0xA2DBC0` | ⭐ **EMBLEM content index** (`0xFFFF` = none) - a measured index now exists (1512, *The Reflective Proof*) ⇒ `design/items-and-gear.md` |
| `+0x24` | `0x808018DB` | - | three fields: `+0x24` a 4-byte bitfield `0x808018E5` `{u8, u8, 3 bits, 5 bits}`; `+0x28` the **peer-group comparator** (`0xA3F0C8 mov eax,[rax+0x28]` → `container+0x58`; equal values ⇒ bit 1 of `container+0x6D`, "in my group" - zero is such a value) `[C]`; `+0x2C` separate |

Bonus: `0xA2DEB2 call 0x276220` (level cap, replayed = **40**) vs `[rbx+0x188]`, `sete` →
`[rbx+0x19d]` = the "at level cap" flag. Send level 40 and the cap marker lights too.

Built: `d1-oracle roster {layout,controls,build}`
(`d1-server/crates/d1-oracle/src/roster.rs`, 18 controls, five negative, exit 2 on failure).
Fixtures `capture-rig/rosters/f2t0-d100000000000001.bin` (81 B, 644 bits) and
`f2t1-d100000000000501.bin` (41 B, 327 bits:
`+0x00 d100000000000501 +0x08 ffff +0x0a ffff +0x10 40 +0x14 400 +0x20 ffff`). Server knob
`D1_ROSTER_DIR`, default OFF. Both payloads go through the client's own encoder and are re-decoded.

⚠ **The clan tag must be NUL-terminated within its 5 units (≤ 4 characters).** The widget copy
`0x65A00` (⛔ not `0x65980`, which is `wcsncmp`) takes `n` as the *destination* capacity and stops at
the *source's* NUL; the clan copy uses `edx = 0x11` = 17 against a 5-unit field ending at the end of
the 80-byte object ⇒ an unterminated tag reads **22 bytes past the object** into the next registry
slot. `roster.rs` refuses longer tags; `clan-overread-NEGATIVE` is the control. The name copy
(`edx = 0x1B` = 27) cannot over-read; a full name truncates silently. `[C]`

⚠ The packed bitfield is a live trap: writing `0x07` into the 3-bit field at `+0x26` round-trips as
`+0x25 = 0x01` (spills into its neighbour). `colour-bitfield-NEGATIVE` asserts refusal at that exact
offset. The struct ships zero.

### 6.5 What this does not fix, said in the same breath

1. Light is raw: `0x281FA0` scales `+0x14` through the stat interpolation curve. `[U]`
2. `nameplate+0x184` stays 0 on the compact path, by construction.
3. The plate may not even read `+0x10`: `0xA3D6ED call 0x65980(widget+0xA8, stackbuf, 0x33)` is a
   second, 51-unit name field sourced from the roster store via `0x57E640`, not from family 2.
   `[C]` it exists; `[U]` which is on screen.
4. A privacy predicate can blank it anyway: `0xA3DBE4 test cl,cl ; jne 0xA3DC61`, `cl` set at
   `0xA3D735` from `0x550800` - when set, the plate uses `L""` for name and clan even though 2/0
   resolved. `[U]`
5. Everything is `synthetic-minimal`: level 40, light 400, the key `0xD1...0501` are chosen numbers.

**The falsifier:** after delivery, read `container+0x10`. `0xFF → 0x02`
(`0xA3F0B6 mov byte [r14+0x10], 2`) proves the chain in one byte. Read that, not the screen.
"Is the nameplate what the operator sees" remains the one `[I]` in the chain; the discriminator is a
poke of `nameplate+0x188 = 99` and a glance at the screen.

### 6.6 Refuted candidates (kept so nobody re-derives them)

* **hud-reader** (blank decided on family 0/type 1): refuted - `0xA3DD16 test r12,r12` decides the
  blank on family **2**/type 1; family-0 absence is a downgrade to the compact fill, not a blank.
  Every operand `0xA2DE30` uses fits the 48-byte `0x80801810` tree exactly.
* **active-character** (`cui::c_investment_login_info_component_v2` is the panel): refuted - 0
  instances in the orbit process (positive controls landed: `cui::c_orbit_data_component` ×1,
  `cui::c_orbit_roster_component` ×1, `cui::s_player_nameplate` ×3). `0xA110F0`, the only reader of
  family 3/1's `C+0x68` level, does not run in orbit. Corollary: populating the family-3 card shows
  level on the character-select screen, not the orbit HUD.
* **the `0x58DB10` mirror** (three lanes through `c_investment_translation_manager_game_system`'s
  20-slot record cache): refuted three ways - the only CUI callers have 0 instances; the mode byte
  (`+0x98`, constructor `0xFF`) was assumed at 4 and never measured; the emblem write
  (`0x9A45E8`/`0x9A45F1`) is outside the guard anyway. The record count is **20**
  (`0x58DB2F mov edi,0x14`), not 21. The state-1 pinning those lanes measured is a co-symptom, not
  the cause.
* A real mechanised twin of the operator's hypothesis exists and is waiting:
  `c_cui_investment_character_state_component`'s identity `{u64, u64, u8 kind}` at
  `+0x88/+0x90/+0x98` is written only by `vfunc[8]` (`0x9A3AE0`) from a CUI property push; the
  constructor (`0x10E5320`) leaves `kind = 0xFF`, failing `0xA871E0`'s `< 8` bind test, permanently
  inert with no diagnostic. Not the cause here (0 instances in orbit) but real.

---

## 7. Case: the empty profile and the empty card

Built 2026-08-08. The emptiness measurement is `[C]` (our own fixture decoded by the client's own
codec, offline); the Light stat index is `[O]` from snapshot replay with a passing positive control.
Nothing here is `observed-live`. Offset maps and progression/emblem specifics live on in
`design/character-and-investment.md` and `design/items-and-gear.md`.

### 7.1 The measurement

> **We shipped a 28,792-byte character profile in which exactly two fields were non-zero.**

`capture-rig/profiles/profile-old-russia.bin` (family 4/1, schema `0x80801743`, pinned by
`run-boot.sh` as `D1_PROFILE_FILE`), decoded by
`d1-oracle object --tag 0x80801743 --decode ... --json`:

```json
{"bits":162539,"struct_size":28792,
 "nonzero":[["0x0","0xd100000000000201"],["0x28","0x932015bb"]]}
```

`+0x00` SOID; `+0x28` `last_orbited_destination` = Old Russia (`0x932015BB`). Everything else zero:

| offset | field | schema | shipped |
|---|---|---|---|
| `+0x00C` | customization block, 12 fields | `0x8080181A` | 0 |
| `+0x2B80` | 20 × u64 equipped item instance IDs | `0x808045DA` | 0 |
| `+0x3660` | progression count + 127 × `{u32,u32,u32,u16}` | `0x808047A7` | 0 |
| `+0x6CD0` | character stat block / Light override | `0x808018BB` | 0 - present byte 0 |

⇒ zero progressions ⇒ no level; no equipment and no override ⇒ `0x265EF0` sums an empty iterator ⇒
light 0; no equipment ⇒ no gear, no model. **The client is correct; the object was empty.** The
family-3 fixtures were probes (ruler values, sentinels) used as if they were content - that was the
whole bug. `char-lvl40-probe.bin`'s `+0x68 = 0x28` = level 40 is read only by `0xA110F0`, the
character-select card builder, which does not run in orbit.

### 7.2 The mechanisms that populate it

* **Light override** `[C]`: `0x265EF0` checks `byte[profile + 0x6CDC]` (present flag) before gear;
  if set, returns the matching `{u8 stat_index, u32 value}` verbatim from the 7-entry array at
  `+0x6CE0`. The **Light stat index is 0** (closed offline: `d1-replay` on
  `runs/snapshots/orbit-all.snap`, `0x281F50` → `mov al,[rax+0x140]`, low byte `0x00`; positive
  control `0x276220` → `0x28` = 40 = the level cap; discriminator `0x281F70` → `0x01`. ⚠ `mov al`
  writes only the low byte - the answer is `rax & 0xFF`).
* ⛔ **Retraction honored:** the stat-override array is at **`+0x6CE0`, with SEVEN entries** - not
  `+0x6CE8` × 6. Three instruments agree: the schema (`0x80804768` count 7 × element 8 = 56, closing
  exactly at `+0x6D18`); the reader (`0x266144 lea r15,[r15+0x6cd0]`, `cmp edi,6; jg`,
  key at `+0x6CE0+8i`); the codec (7 at `+0x6CE0` round-trip `object IDENTICAL`, 7 at `+0x6CE8`
  refused at `+0x6d18`). Mechanism of the error: `lea rdi,[rdi+1]` sits after the key compare and
  before the value load; the value load is `[rax+rdi*8+0xc]` = `0x14 + 8i`, misread as a
  pre-increment. ⚠ `d1-eboot descriptors` prints the **element size** in the offset column of a
  `kind 0x1` field (`@+0x008` is the 8-byte element, not an offset).
* **Level**: one progression entry `{u32 progress, 0, 0, u16 index = 117}` at `+0x3660`
  (`character_level`'s runtime ordinal is **117**, from the progression definition table in
  `ps4_investment_globals_012f_3.pkg` block 1066 entry 8132; cumulative XP to 40 is 346,000; whether
  the cap wants `≥ 326,000` or `≥ 346,000` is `[I]`). No content hash appears as an immediate
  anywhere in the eboot (8 candidates, 0 hits, control 11,895) - the ordinal comes from content.
  ⇒ `design/character-and-investment.md`.
* **Equipment**: `+0x2B80` = 20 × u64 item instance SOIDs, each resolving in family 4/3
  (666 × 184 B, `0x80801732`); element `+0x24` `u16` is the content index.
* ⚠ **Live contradiction resolved in place**: field `[10]` of the `0x8080181A` block (absolute
  `+0x28`) was named `personality` by shape-match, but the live client compares that field against
  `0x811C9DC5` (FNV-1a-32 basis = hash of the empty string = "unset") at `0x752AE0` and it carries
  `last_orbited_destination` - orbit renders Old Russia from it. **Writing "a real value" there
  would overwrite the destination and break orbit.** The live measurement wins.

### 7.3 The populated profile

`d1-oracle profile {layout,controls,build}` (`d1-server/crates/d1-oracle/src/profile.rs`, 9
controls, exit 2 on failure) built `capture-rig/profiles/profile-populated.bin` (20,318 B /
162,539 bits, sha256 `53e115ae3a3a22c5...`), encoded by the client's own encoder (`0x2C8FF0`),
round-tripped `object IDENTICAL`:

| offset | field | value |
|---|---|---|
| `+0x0000` | SOID | `0xD100000000000201` |
| `+0x0008` | `{race, gender, class}` | `{0, 0, 1}` (`+0x08 = 0x10000`) |
| `+0x0028` | `last_orbited_destination` | `0x932015BB` Old Russia - kept, orbit depends on it |
| `+0x0038` | 340 inventory slots | `u16 = 0xFFFF` (EMPTY; `0x1056640: cmp word[rsi],0xFFFF; setne` - zero is NOT empty, it claims content index 0) |
| `+0x2B80` | 20 equipped item SOIDs | 13 × `0xD1000000000003xx` |
| `+0x3660` | progression | `--progression 117:400000` (count word gates the table: with `count=1` a write into entry 1 is dropped) |
| `+0x6CDC` | stat-override present | 1 |
| `+0x6CE0` | stat override `[0]` | `{index 0, value 400}` - ⭐ Light 400 with no gear |

Two non-zero fields became 358. Controls that can fail in the direction that matters:
`stat-array-base-NEGATIVE` (7 entries at `+0x6CE8` must be refused at `+0x6D18`),
`progression-count-gates`, `strictly-more-than-baseline` (strict superset of the baseline's non-zero
qwords). Not free: the 340 slots changed `0x0000 → 0xFFFF`
(`--phantom-item-slots` reproduces the old shape for bisecting), and the 13 SOIDs name nothing
unless `D1_INVENTORY=capture-rig/inventory` is set. ⛔ The whole file is `synthetic-minimal`.

### 7.4 The other half: the card is a SECOND object

A live read confirmed the client **stores every field** of the populated profile - and the screen
still showed level 0, light 0, black emblem, in orbit. ⇒ `[I]`: the profile is the IN-WORLD source;
orbit-adjacent surfaces draw the **character-select card** - family 3/1, schema `0x80801954`,
2,688 B - and `run-boot.sh` left `D1_CHARACTER_FILE` unset, so `d1-responder` fell back to its
built-in 13-byte identity-only `character_rsat`.

| | in-world | on the card |
|---|---|---|
| object | family 4/1 `0x80801743`, 28,792 B | family 3/1 `0x80801954`, 2,688 B |
| env | `D1_PROFILE_FILE` | `D1_CHARACTER_FILE` |
| SOID | `0xD100000000000201` | `0xD100000000000101` |
| level | derived from progression 117 at `+0x3660` | a plain `u32` at `C+0x68` (`0xA11555`) |
| light | override at `+0x6CDC`/`+0x6CE0`, else summed over gear | stat table at `C+0x870` (`0x2666F0`) |
| gear | 20 × u64 SOIDs at `+0x2B80` | 20 × 64 B summary at `C+0x250` (element `+0x00` SOID, `+0x08` u16 content index → emblem via `0xA11627`/`0xA81B00`) |

Both carry `{race, gender, class}` at `+0x08` and the same 36-byte `0x8080181A` block at `+0x0C`;
they must agree. `d1-oracle character {layout,controls,build}`
(`d1-server/crates/d1-oracle/src/character.rs`, 13 controls) built
`capture-rig/characters/char-lvl40-geared.bin` (2,326 B / 18,607 bits): level 40 at `+0x68`,
derived level 40.0f at `+0x70`, 13 equipped SOIDs at `+0x250`, all 20 content indices `0xFFFF`,
stat `[0]` = `{index 0, value 400}` at `+0x870`, stat keys `[1..15]` = `0xFF`. 53 non-zero qwords
against the identity-only card's 2.

⭐ **Two absent-markers, two widths, one trap** `[C]`: `0xFFFF` is the absent marker for a content
INDEX; **`0xFF` is the absent marker for a stat KEY.** The stat table has two readers: `0x2666F0`
breaks on first key match, but `0x266740` memsets 64 u32 slots, skips only key `0xFF`
(`movsx ... cmp rcx,-1 ; je skip`), and writes every other entry in order - so a zero-filled table
is a claim of sixteen copies of stat index 0 and **deletes Light fifteen times**. The
`stat-sentinel-is-required` control simulates `0x266740` both ways and fails unless the zeroed
variant loses Light.

⚠ The codec cannot discriminate a ±8-byte shift of the `C+0x870` table (the sibling table at
`C+0x8F0` has the identical element layout); the base is pinned by the instruction operand
(`0x2666F0` addresses `[rbx + 0x808 + 8·i]`, `rbx = card + 0x68`). Negative controls
`equipment-stride-32-NEGATIVE` (refusal at `+0x279`) and `stat-table-base-NEGATIVE` (refusal at
`+0x879`; every probe value `400 + i` so the refusal offset cannot slide) guard the two array
strides.

Not free: `D1_CHARACTER_FILE` changes the identity bytes on the wire - the responder's built-in
encoder uses `{race 0, gender 1, class 0}` (`+0x08 = 0x100`), the fixtures carry `{0, 0, 1}`
(`+0x08 = 0x10000`); `[U]` which is correct. The three sibling stat tables at
`C+0x8F0`/`C+0x970`/`C+0x9F0` ship zero (`[U]`, no reader identified;
`--sentinel-all-stat-tables` is the bisect axis). The emblem: ship `0xFFFF` unless using the
**measured** content index (1512, *The Reflective Proof*; the arithmetic route
`N = tag - 0x80A60297` is refuted - 633 discontinuities). A fabricated index is worse than an empty
slot because it renders as something and looks like success. ⇒ `design/items-and-gear.md`.

The director is **not** a content push: director books are `DestinyDirectorBookDefinition` records
in the client's own packages (Earth/Old Russia book `2707330101`, 53 nodes, empty
`isVisibleExpression` - gated on nothing). What the server owes the director is **state**: family
4/2 and family 4/4 (⇒ §8). The director needs unlock state and cache handling; the character needs
a populated profile; **neither is family 2, and they are different problems.**

---

## 8. Case: director selectability

Built 2026-08-08; **twice retracted since, and the retractions govern.** Structure (chain,
addresses, offsets) is `static-client-inference` against the pinned eboot and remains good; live
values are from snapshots a client our own server drove (`synthetic_ancestor`); nothing is
`observed-live`.

### 8.1 What is now believed (post-retraction)

* **Family 4 replicates fine** (§2.3: all five families at `state=3` on a normal run), and
* **the availability cache reports success**: probe run `20260808-110410-AVAIL-ITEM-TRACE`
  instrumented `0x260200`'s common exit - the builder runs
  **three times**, fails only on the first (early-boot) invocation at `0x2603EC`, and **succeeds on
  both later ones**, including while the director is on screen. Final header `{1, 1, 0}`;
  `ctx+0x28` populated at every entry.

⇒ **The director is blank while family 4 replicates and its availability cache reports success.**
The blank is a **consumption** failure - the CUI pull layer reading *derived* availability computed
from family-4 **unlock flags, which we serve empty** - not a replication failure and not a missing
4/2 object. That is a genuinely open problem. ⇒ `design/grants-and-unlocks.md`.

Methodological lesson (the durable part): **a snapshot of a function that runs three times is not a
measurement of that function.** The `{1,0,0}` reads caught invocation 1.

### 8.2 The structure that survives (and made the probes possible)

The availability state is a cache, ~18 KB off `s_investment_utility_context` at `0x26A34B0`
(⛔ not "the selection manager" - RTTI via vtable `0x18F5AF0` → typeinfo `0x18F5BE0` → name
`0x15A9570` says `s_investment_utility_context`; the manager is `0x573C80`,
`investment_activity_selection_manager`). Exactly one function builds it (`0x260200`), ending in a
three-byte header at `[ctx+0x38] + 8`:

```
byte[state+0] = 1                  // "cache valid"  - written on BOTH success and failure
byte[state+1] = <success flag>     // 1 = the build COMPLETED, 0 = it FAILED
byte[state+2] = 0
```

`byte[state+0]` alone tells you nothing; `byte[state+1]` tells you everything. The two-command
triage (before any poke or run):

```sh
python3 capture-rig/d1guest.py read 0x26A34B0 --len 0x40 --as u64   # [+0x20] 4/4, [+0x28] 4/2, [+0x38] state
python3 capture-rig/d1guest.py read <[+0x38]> --len 3 --abs         # {valid, SUCCEEDED, _}
```

The chain, link by link (structure `[C]`):

| # | link | address |
|---:|---|---|
| 1 | server delivers a family-4 QueueZ update | wire (`d1-bap`) |
| 2 | QueueZ applies it and announces the change | `0x56D1A0`, `0x56E7B0` |
| 3 | the announce calls `0x574440(family)`; `if (family == 4) byte[state] = 0` - invalidate | `0x574440` |
| 4 | `0x574220(4)` / `0x574340(4)` refresh the context via the pure setter `0x2609A0` | `0x574220`, `0x574340`, `0x2609A0` |
| 5 | a consumer asks for evaluated unlock state: `0x25FF40(ctx)` → `r14 = [ctx+0x38] + 8` | `0x25FF40` |
| 6 | if `byte[r14] == 0` → `ctx->vtable[0x80]()` = `0x260A10` → `0x260200(ctx, 0, r14)` rebuilds | `0x260A10`, `0x260200` |
| 6b | `0x260200` hard-aborts if `[ctx+0x28]` (family 4/2) is NULL - stamps `{1,0,0}` and returns (`0x260419 test rbx,rbx` ... `0x26042E`; `[ctx+0x30]` = `0x574040(4)` = `0x8026A3470`, an eboot global, never NULL) | `0x260419`-`0x26043D` |
| 7 | every unlock expression is evaluated by `0x28DEA0` against `r14 + 4` (jump-table stack machine; `[state+0x2000]` is a second plane) | `0x28DE40` → `0x28DEA0` |
| 8 | `0x9D8270` walks the activity's tiers; a tier with a gating expression is dropped unless `0x28DE40(ctx, tier)` is true (`0x28E4B0` gates it; a tier with no expression is accepted unconditionally) | `0x9D8270` |
| 9 | `0x9D6A10(actDef, &node+0x268)` builds the node's second container; itself calls `0x574220(4)` | `0x9D6A10` |
| 10 | `0x9D92A0` builds the `cui::c_director_activity` and computes the flag word at `node+0x20` | `0x9D92A0` |
| 11 | bit `0x8000` of `node+0x20` = "do not offer this node"; OR-ed in, sticky, at three sites | `0x9D9AAE`, `0x9D9DC4`, `0x9D9FB2` |
| 12 | `cui::c_launch_director_activity_component` appends the node only if bit `0x8000` is clear (`test ah, 0x80 ; jne <skip>`) | `0x9E6448` |
| 13 | `cui::c_director_activity_node_container_widget` holds its node at `widget+0x128`, rebuilds every update | `0x9DCF40` (call at `0x9DD078`) |

State block layout (from `[ctx+0x38]`): `+0x00` u64 next-event stamp; `+0x08` validity byte
(cleared by `0x574440`/`0x574460`, set by `0x260200` on success and failure alike); `+0x09` success
byte; `+0x0A` secondary flag cleared by `0x25FFD0`; `+0x0C` the `0x4000`-byte evaluation plane
(`+0x2000` second sub-plane); `+0x400C` a `0x800` buffer; `+0x480C` u32 count zeroed at `0x2602E1`.
A failed build still leaves partial content (37 non-zero lines measured; positive control 2/16 on a
different object) - "there is data in there" is not evidence of success; only `byte[+0x09]` is.

Invalidators: `0x574440(4)` clears the validity byte via `vtable+0xC8` = `0x260BD0`
(`mov rax,[rdi+0x38]; add rax,8`); `0x574460(4)` clears whenever the success byte is 0; `0x25FFD0`
clears after its compare loop. Callers `0x56D1A0`, `0x56E7B0`, `0x576100` - all in the
QueueZ/investment update path. Re-triggering wants a DELTA, not a repeated baseline (§2.4).

Freshness predicate `0x2609C0` (vtable `+0x78`): returns 0 if the validity byte is 0, else
`[state] > [0x264E50([ctx+8], [ctx+0x18])]` - and **TRUE is the *stop* answer** (`0x260A10` opens
with `call [vtable+0x78] ; test al,al ; jne <return>`). Measured: `0x4B3DB990 > 0x55` ⇒ 1, so no
recompute. `0x264E50` is an override selector: `if (B && byte[B+0x3E80]) return B+0x3E88; return
A+8` - replayed, default branch. Units of the two stamps `[U]`.

Bit `0x8000` producers in `0x9D92A0`: `0x9D9AAE` (`[node+0x278] == 0`, tier container failed),
`0x9D9DC4` (`[node+0x288]` inner NULL), `0x9D9FB2` (`[node+0x268] == 0` or
`byte[node+0x100] == 0`; that byte is set at `0x9D9E4F` only after `0x574220(4)` returns non-NULL
and `[node+0x138] != 0` - the server-controlled input). Other flag bits of `node+0x20`: `0x01`
`0x266640(f)` · `0x02` light above threshold · `0x04` required light > current (`0x58DB10` vs
`0x276280`) · `0x08` caller arg · `0x10` `0x5442B0(idx)` · `0x80` caller arg · `0x800`
`[def+0xE8] != -1 && [def+0xEC] != 0x811C9DC5` · `0x1000` `!0x54F410()` · `0x2000`
`0x26B3E0(ctx, idx)` · `0x4000` currently-highlighted (`0x9E62D2`, `0x9E642B`) · `0x10000`
`0x952940()` · `0x80000` caller flag.

Context fields (setter `0x2609A0`; ⚠ `r8→+0x28` and `r9→+0x20`, the reverse of argument order):
`+0x08` `0x5867D0()` · `+0x10` `0x586540()` · `+0x18` `0x5866D0()` · `+0x20`
`0x56CA70(reg, 4, [0x2639EA8])` = family 4/4 (unlocks) · `+0x28` `0x56CAF0(reg, 4, 2)` = family 4/2
· `+0x30` `0x574040(4)` · `+0x38` the cache/state block (not set by the setter).

**Family 4/2 vs 4/4 weighting** `[C]`: `0x260C10` (the 4/2 accessor, `[ctx+0x28]`) has **25**
references and vtable slot `+0x20`; `0x260C20` (the 4/4 accessor, `[ctx+0x20]`) has **0 confirmed
references** - in no vtable, called from nowhere (controls in the same sweep). Family 4/2 is what
`0x260200` demands; the historical ranking that put 4/4 first is inverted. Whether `[ctx+0x20]` is
read by direct field access anywhere is `[U]` - until found, "family 4/4 feeds the availability
block" is `[U]`, not `[I]`. Object shapes: family 4/2 = schema `0x80800FB2`, 72 B, keyed by the
account SOID `0xD100000000000001` (observed live as a key-only object,
`01 00 00 00 00 00 00 d1` + 64 zero bytes); family 4/4 = schema `0x8080102E`, 3,984 B, key
`0x7FFFFFFFFFFFFFFE`, instance checksum `0xC4060975`, key-only payload `bfffffffffffffff00`,
carrying 200 × `{u64 flag id, u6 value}`.

Ruled out for the director blank: not an unlock-expression visibility gate on the Old Russia book
(empty `isVisibleExpression`, skipped by `0x28E4B0`); not content (replaying `0x9D92A0` resolves
node index 1, zero arrivals at the bail label `0x9DAAE0`); not `0x574340`'s early exits; not the
family-4/4 registry lookup (a poke wired `ctx+0x20` and changed nothing on screen).

### 8.3 Standing `[U]`s specific to this chain

* Where family 4/4 is consumed, if anywhere (accessor unreferenced, direct reads unsearched).
* Whether the blank director is "hidden" (bit `0x8000` suppresses from
  `c_launch_director_activity_component`'s list) or "drawn but inert"
  (`0x9DCF40` does not obviously test the bit).
* `0x9D92A0` cannot be replayed to completion: `FETCH_UNMAPPED at 0x80A5E64F0`, the shared library
  mapped above the eboot (same region as the C++ runtime vtables `0x80A65B170` / `0x80A65A9A0`).
  Next snapshot capture should add `--map 'a:0x80a5e0000'`; it unblocks replaying every CUI
  function.
* The units of the `[0x37F8BD330] = 0x4B3DB990` vs `[0x37F8C27F8] = 0x55` stamps (signed `setg`).
* The 32-element second array in the 4/4 schema (`0x80801025`, `{u8,u64,u16,u16}`), unattributed.
* Context vtable slot `+0x58` (`0x260C60`: `[[ctx+8]+0x2DC] & 1`), unidentified.
* **The actual consumption path** from evaluated availability to the inert node, now that
  replication and the cache are exonerated. ⇒ `design/grants-and-unlocks.md`.

---

## 9. Open [U]s

Namespace (§4):

* `[I]` Cross-process tag stability sampled at n = 2; the free experiment is
  `capture-rig/live-tag-identity.py` + `capture-rig/verify-tag-determinism.py` piggybacked on any
  scheduled run (success: `resolve(0x808033F4) = <base>+0x1492550` in a third process, controls
  `resolve(0x80801825)` / `resolve(0xDEADBEEF)`).
* `[U]` `0x6E9B0`'s validity predicate `0x1048430([container+0])` (returns `0xFFFFFFFF` on failure;
  never fired for the 7,670 measured entries) is unread.
* `[U]` The namespace is mapped but unnamed: 14,816 hashes, zero names beyond the nine modules. The
  hash function is closed; naming requires an external symbol source, and the precondition is **one
  confirmed (name → hash) pair**. Do not restart the search.
* `[U]` A 28-element gap between the anchored-descriptor counts (4527 vs 4555) - the naive NUL
  terminator rule is the suspect.
* `[U]` Descriptor scalars `aux` (`+0x0C`), `blobA` (`+0x38`), `blobB` (`+0x40`) unread.
* `[U]` What distinguishes the three containers; `[U]` how the startup routine containing `0x1100`
  is entered (no ctor entry, no reference - read by inference).

Catalogue (§5):

* `[U]` The span word's meaning for message schemas; `[U]` whether an empty `0x000B` body is legal
  per investment id (drivable offline with the client's own decoder); `[U]` which of policy families
  0/1 is deterministic; `[U]` `0x0099`'s purpose; `[U]` the `0x0021` protobuf (648 B, descriptor
  `0x15AB800`); `[U]` the message → policy family binding at send time (enqueue path `0x567A40` /
  `0x5681E0` / `0x568770`); the payload grammar of the 63 envelope classes (not attempted -
  hand-written encoders). Investment name hashes `0x2DCC40D2` (105), `0x5A146EAB` (104),
  `0xA52AE87F` (205) - only the X360 ALPHA can close them.

Replication and HUD (§2, §6, §7):

* `[U]` The RSAT bit-grammar for encoding roster/profile objects on the wire was the original gap;
  the oracle-built fixtures (encoded by the client's own encoder) are the current answer, but what a
  **real** family-2 card contains, and whether the real service publishes family 2 on subscription
  or on selection, needs the live-service route - `/SignOn` is HTTPS and BAP is AES-128-GCM, so a
  passive tap gives metadata only.
* `[U]` Family 2/1 `+0x08`, `+0x0C`, `+0x1C`; the privacy predicate `0x550800`; which name field is
  on screen; the Light raw→display curve (`0x281FA0`, replayable offline).
* `[U]` What `family_type=ENUM(2) instance_type=ENUM(1) soid=0x2975AEF1`
  (run `20260808-193351-GRANT-ANSWERED`) meant - `0x2975AEF1` is `actual_level_light`'s content
  hash; if family 2/1 is also keyed by progression hashes there are two key spaces on one 48-byte
  schema. Discriminator: was `D1_PROGRESSIONS` set in that run.
* `[U]` Which identity triple `{race, gender, class}` is correct ({0,0,1} fixtures vs {0,1,0}
  responder built-in).
* `[I]` Which object orbit-adjacent surfaces draw (profile vs select card) - the discriminator is a
  run with `D1_CHARACTER_FILE` set; `[I]` whether the nameplate is the on-screen surface (STEP 4
  poke of `nameplate+0x188`).
* Why family 0 (peers) is also answered `object_count = 0` - same file, same fix shape, but family
  0 must be delivered **complete or not at all** (§6.3 substitution trap).

Director (§8.3, listed there).

---

## 10. Dead claims

Each line: the claim, and what killed it. None of these may be carried forward as live.

1. **"Investment id 501 is `login_character`"** - killed by the FNV-1 hash in the shipped definition
   table plus sender immediates (`0x551040` → `0x1F5`, `0x551640` → `0x1F8`): 501 =
   `login_create_character`, 504 = `login_character`.
2. **"Family 4 is stuck at `replication state=1`, so every family-4 update is ignored;
   `D1_ANSWER_FAMILIES=0,2,3,4` is the director fix"** - killed live (run
   `20260809-084932-RUNG33-PERMS6`): all five families reach `state=3 REPLICATING` without being
   listed. `state=1` was a property of the unsolicited-push path at one instant, generalised from
   one run.
3. **"The availability cache build fails ({1,0,0}) and family 4/2's absence is the director's
   blocker"** - killed by probe (run `20260808-110410-AVAIL-ITEM-TRACE`): the builder runs three
   times, fails only at early boot, succeeds while the director is on screen; final `{1,1,0}`. The
   cause is downstream of the builder (consumption).
4. **"Tags are per-load; carry the name hash, not the tag"** - killed by the offline-vs-live join
   (7670/7670 with scramble control): the tag is a compile-time-determined constant.
5. **"`0xE2B769E8` is the span element's hash"** - killed; it is `0xE2E25C2C`. Root cause: decimal
   `42979504` miswritten as `0x2900BF0` instead of `0x28FD0B0`.
6. **"`kind & 0x100` marks repeated/array fields"** - destroyed; the high byte is an
   optional-presence-bit width, only ever 0 or 1, and no code masks `0x100`.
7. **The stride-`0x58` descriptor-pool model (2918 records, a "kind 192" record, the
   `{1,4,8,...,64}` width distribution)** - killed by the self-delimiting walk: wrong in phase and
   stride; the phantom record is the next family's size word; the width column has 323 distinct
   values.
8. **"Slot `0x401` is a content package, not the eboot"** - superseded: all three containers are
   eboot-resident schema handle tables.
9. **"`0x0020` has no paired response class"** - contradicted by the binary (class `0x0021` exists,
   policy 25 maps it); the *measurement* (answering harmed) stands, the explanation is the protobuf
   body.
10. **"`0x1b`-`0x1d` are all log notifications; `0x24`-`0x29` are gaps"** - killed by the RTTI class
    table: `0x1c`/`0x1d` are the server-challenge pair; six real friend/party classes fill the
    "gaps".
11. **"`0x559D30` is an ICF twin of the mapper"** - corrected: it is a vtable `+0x20` thunk jumping
    to `0xB32410`.
12. **"Guess-and-check FNV can name the investment ids"** - proven futile, not merely unlucky:
    58,960 harvested spellings → one match, the injected control; the enum name tables shipped
    empty (941/1080 `fnv1("")`).
13. **"Delete the family-2 progression path from `d1-responder`; it cannot work"** - retracted: the
    plumbing (family, root, both instance checksums) is exactly what the roster card needs; only
    the payloads and naming were wrong.
14. **"The stat-override array is at `+0x6CE8` with 6 entries"** - killed by three instruments
    (schema, reader `0x265EF0`, codec round-trip): it is `+0x6CE0` × 7.
15. **"Give the customization block's `+0x1C` (absolute `+0x28`) a real value"** - killed by the
    live read at `0x752AE0`: that field is `last_orbited_destination`; writing it breaks orbit.
16. **"Family 2 carries progressions"** (the retracted `family2-replication-gate.md`) - wrong
    family; family 2 is the roster/player card.
17. **"The blank HUD is decided on family 0/type 1"** - killed on the operands: `0xA3DD16` tests
    family 2/1; family-0 absence downgrades to the compact fill.
18. **"The orbit panel is `c_investment_login_info_component_v2` / the `0x58DB10` record cache
    writes the HUD"** - killed: 0 instances of every candidate class in the orbit process; the mode
    byte was assumed; the emblem write is outside the guard; the record count is 20, not 21.
19. **"Level/light are drawn from family 0 and family 2 is only the key-holder"** - closed as not
    the case: the compact card alone is sufficient, and a partial family-0 object would *blank* the
    plate (substitution, not augmentation).
20. **"The container is identical on all three nameplates" / "the `[r15+0x1c4] = -1` tail writes
    discriminate exits"** - both killed by measurement: two plates differ from the third at two
    different blank instructions; `-1` has one writer and no reset, so it is sticky and cannot
    discriminate.
21. **"`0x65980` is the widget's string copy"** - wrong function: `0x65980` is `wcsncmp`; `0x65A00`
    is the copy (and its `n` is destination capacity, hence the clan-tag over-read).
22. **"The QueueZ schema handle cannot be read statically"** - superseded: all four family-0/2 tags
    and both checksums recovered offline, validated against the two known-good responder constants.
23. **"The RSAT record base is at the instance checksum" (record `+0x00` checksum)** - killed by
    the code read at `0x56b6f8` (`mov ecx,[rsi + 8]`): the base is 8 below; checksum at `+0x08`,
    decoded size at `+0x14` (and never `+0x18`, which is the constant 8).
24. **"Family 4/4 is the director deliverable, ranked first"** - inverted: `0x260200` demands 4/2;
    the 4/4 accessor has zero references. (And per claim 3, neither delivery is the current
    blocker.)
25. **"`0x26A34B0` is the selection manager"** - RTTI says `s_investment_utility_context`; the
    manager (`0x573C80`) drives it.
26. **"All six context fields are non-NULL" (the brief)** - not reproducible from any held
    snapshot; `+0x20` is NULL in both orbit snapshots.
27. **"`0x574440` is the only cache clearer" and "re-delivery must be a repeated baseline with a
    changed byte"** - corrected 2026-08-10: `0x574460(4)` and `0x25FFD0` also clear; the
    re-delivery must be a **delta** (a baseline at state 3 goes to the purge arm), and needs no
    changed byte.
28. **"There are two type registries"** - there are nine; the two are `spasaha` and
    `spasaha_investment`.
29. **"The registries' name-hash function might be crackable" / "search the image for a tag" /
    "find the descriptor whose array element is `0x808033F4`"** - all closed (see §4.5); do not
    re-open.
30. **"The emblem content index is `N = tag - 0x80A60297`"** - refuted: the world-tag table has 633
    discontinuities over its 7,544 indices; the measured route is the entry-8126 table
    (⇒ `design/items-and-gear.md`).
31. **"`0x27FDD0` is a static id→index map in the eboot"** - it is a linear search over content
    rows; the ids come from shipped content.

---

## 11. Sources

| melted source | what it contributed |
|---|---|
| `historicaldocs/design/server-object-model.md` | §1, §1.1, §1.2 (the model decision, the 501/504 correction, the four measured objects) |
| `historicaldocs/design/namespace-walk.md` | §4 (registries, tag formula, descriptor chain, ruled-out list, traps, namespace `[U]`s) |
| `historicaldocs/design/message-catalogue.md` | §5, §4.4 (envelope classes, policies, investment ids, RSAT record base, crate cross-check) |
| `historicaldocs/design/THE-EMPTY-PROFILE.md` | §7 (the emptiness measurement, profile/card builders, stat sentinels, `+0x6CE0` retraction) |
| `historicaldocs/design/orbit-hud-blank.md` | §6 (the family-2 chain, the four gates, the roster objects, refuted candidates) |
| `historicaldocs/design/director-selectability.md` | §8 (the chain and cache structure; both retraction banners honored in §8.1 and §10) |

Related live topics: `design/bap-transport.md` (wire framing, crypto) ·
`design/character-and-investment.md` (character lifecycle, progressions, offsets) ·
`design/items-and-gear.md` (content indices, emblem, inventory) · `design/grants-and-unlocks.md`
(unlock flags, the director's open consumption problem) · `design/architecture.md` ·
`design/player-in-world.md`.

Tools that re-derive this document: `capture-rig/message-catalogue.py` ·
`capture-rig/namespace-walk-verify.py` · `capture-rig/type-registry.py` ·
`capture-rig/verify-tag-determinism.py` · `d1-oracle profile|character|roster|object` (each with a
`controls` subcommand; **exit 2 means the output may not be cited**).
