# The activity-lifetime-state record - the wall behind tasks 9/10 and the spawn gate

> # ✅✅ SERVED, 2026-08-11 - THE PHASE IS A WIRE FIELD, DELIVERED BY AH **udt 5 `sensor_auth_update`**
>
> **This supersedes the udt-7 conclusion in the retraction banner below AND §4d.** An
> adversarially-verified workflow (`wf_f7f6ddbd`, 36 agents, two Fable audits) plus a dedicated
> Lane-A verification settled all four unknowns, and the server now emits the message.
>
> ⭐ **The chain, executed end to end on the clean `runs/snapshots/rung36-biped-chain.snap`
> (`d1-replay controls` 20/20, exit 0; `d1-eboot controls` exit 0):**
>
> ```
> AH udt 5 sensor_auth_update (server→client)
>   → FUN_0029D3F0 → FUN_002406D0 → FUN_00BAF140  (enqueue sim-queue element type 0x13)
>   → FUN_002FFD70 drain → FUN_00BB1BC0 case 0x13 → FUN_00BAF230 (resolve world by ah_id, drop if null)
>   → FUN_002A1210  (decode the 59-byte replicated frame over schema 0x80800CF7)
>   → FUN_002D0240 walk → FUN_002CE2A0  (guard record+0xA8 != 0/-1, record+0xB0 != -1 - PASSES today)
>   → PMF dispatch on record+0xA8 = {0x8080039D, ord 2}  → FUN_0011DE30
>   → memcpy(record+0x1AC, value_image, 0x504)           → phase byte lands at 0x38026207C
>   → FUN_0010C500 reports state → spawn gate FUN_00414730 accepts {3,6} (mask 0x48 @0x4147E5)
> ```
>
> **Executed proofs (Lane A, this session):**
> * **Value carries the phase:** `d1-oracle object --tag 0x808008A3` round-trips the phase-3/6/−1
>   value blocks **byte-identical** through the client's own object codec; the phase is a **4-bit,
>   bias-1** enum (`raw = phase+1`: −1→0, 3→4, 6→7).
> * **Setter reaches the store:** `d1-replay replay --func 0x11de30` with a planted `{0x808008A3,
>   ptr}` box and this session's value image reaches `memcpy` at `0x11DEF0` with **`rdi=0x38026207C`
>   (record+0x1AC), `rsi`=image, `rdx=0x504`** (probe @0x11DEF0). It faults only because `memcpy`
>   is an **unmapped libc import thunk** (`jmp [0x1a07c98]`) - the copy is proven by its
>   call-boundary args, not executed; copy semantics are not in doubt. `[C]`
> * **Getter/gate accept it:** `FUN_0010C500` returns `al=0`/NONE on the unmodified record, and
>   `al=1 state=3` / `state=6` with `record+0x1AC` planted 3/6 (`--set-mem`), `state=4` for a
>   planted 4 - combined with the static gate mask `0x48` = accept {3,6} / `0x3B7` = deny the rest.
>
> **The wire frame - 59 bytes / 472 bits, MSB-first:** `gen(8)=world+0x1D · has_sensor_block(1)=0 ·
> stamp(32) · init(1)=1 · has_entry(1)=1 · name-ref(55, schema 0x808004B1) · body_bit_length(32)=`
> **`2+value_bits`** (audit correction: counts fresh+presence+value, **not** value alone) `·
> fresh(1)=1 · presence(1)=1 · value(339, schema 0x808008A3) · terminator(1)=0`. The name-ref
> `{hash, i8, sub}` selects the record: group hash (per-load) + sub 3.
>
> ### What is SERVED vs still SYNTHETIC
> * **SERVED (committed, controlled):** `d1_oracle::sensor` assembles the byte-exact frame (11
>   controls, `d1-oracle sensor controls` exit 0, byte-exact against the audited payloads through
>   the client's own codec); `d1-activity-host` carries it as `AhMessage::SensorAuthUpdate`, sends
>   it verbatim, and arms it from **`D1_AH_SENSOR_PHASE`** - sent **once**, after the instance is
>   Live **and** the first heartbeat has fired (so the world is resident, not merely joined).
> * **⚠ `[U]` THE PROMOTION GATE (reach: `synthetic-minimal`, everything downstream
>   `synthetic_ancestor`):** whether our server-delivered element's slot is in the world's post-pass
>   **sweep container** - i.e. whether `FUN_002CE2A0`'s dispatch fires on a *fresh wire* element or
>   only on content resident at load. `FUN_002A1210` is reached from exactly one caller and the
>   guard passes today, but "reachable ≠ traversed." **Settle it live** (see the experiment at the
>   end of this banner). Until then the `FUN_0010C500` hook, the `je@0x411E16` NOP and the done-mask
>   force stay.
> * **⚠ `gen` is per-boot and NOT server-derivable** (the server cannot read the client's
>   `world+0x1D`); a wrong `gen` is a **silent skip** (`jne` @0x2A145C). `D1_AH_SENSOR_GEN`
>   overrides; default 0 (the measured in-world value). Likewise the **name hash is per-load**
>   (`0xC2DACAF7` in-world, `0x4786C0E0` in orbit); `D1_AH_SENSOR_NAME_HASH` overrides, default the
>   measured in-world pvp_moonbase group 0.
>
> ### ⛔ CORRECTIONS this forces on §4d (in place, below)
> * **udt 7 `sensor_message` is the WRONG message.** Its handler `FUN_0029CDE0` hardcodes the
>   setter slot at **record+0x108**, which is authored empty on **every** object of the family
>   (four censuses, 0 bound) - a udt-7 push decodes, resolves, fails the guard, and does nothing,
>   silently. The phase setter is bound at a **different slot, record+0xA8**, reached by udt 5.
> * **`FUN_0010BC20` is NOT the writer of the byte the gate reads.** It is ordinal 2 of a *different
>   class* (`0x80800680` = globals record 6, body `0x380262D40`). The gate's record is class
>   `0x8080039D` (globals record 3), whose ordinal-2 setter is **`FUN_0011DE30`** - matched on the
>   *type*, not just the ordinal (the sig hash `0x4D7DDECB` marks "the setter" in every class).
>
> ### ▶ THE READY-TO-RUN LIVE EXPERIMENT (task #4 - Lane B / main drives it)
> ```sh
> # AH hot-swap (no client reboot): send udt 5 once the Guardian run is in-world.
> D1_ANSWER_FAMILIES=0,2 D1_ACTIVITY_HOST=1 D1_AH_BIND_PORT=4110 \
> D1_ROSTER_DIR=$PWD/capture-rig/rosters-f0-hybrid OPERATOR_PAD=1 \
> D1_AH_SENSOR_PHASE=3            # arm the sensor; gen defaults 0, name defaults the in-world hash
>     capture-rig/run-boot.sh <RUN-NAME>
> ```
> **Discriminator:** `d1guest.py read` of `record+0x1AC` (`0x38026207C` in the guardian capture;
> re-resolve per boot via the `FUN_0010C430` walk) flips **`0xFF → 3`** after the server's message,
> and the spawn gate passes **WITHOUT** the `FUN_0010C500` hook, the `je@0x411E16` patch, or the
> done-mask force. **Null control:** `D1_AH_SENSOR_PHASE=-1` (the identity payload) must change
> **nothing** observable - if it does, the experiment is invalid. If `record+0x1AC` stays `0xFF`,
> the post-pass sweep did not include our slot (`[U]` above): read `world+0x1D` live and confirm it
> equals `D1_AH_SENSOR_GEN`, then `d1-live watch --rw write --len 1` on `record+0x1AC` across one
> send to see whether the store is even attempted.
>
> **Tools & controls:** `d1-oracle sensor build --phase 3` (the frame) · `d1-oracle sensor controls`
> (11, exit 2 on fail) · `d1-activity-host controls` (111, incl. 3 sensor/state controls) ·
> the setter/getter replays above on `rung36-biped-chain.snap`.

> # ⛔⛔ RETRACTION, 2026-08-11, SAME SESSION - §4c AND §5b ARE WRONG
>
> **§4c's headline ("the phase is a BlamScript-VM variable, not a wire field") and §5b's verdict
> ("this is the cloud-script ceiling") are RETRACTED.** They were reached from an incomplete trace:
> I established that `FUN_0010BC20` is registered in the `c_bs_game_system` binding table and
> stopped there, treating "registered as a script-callable native" as "invoked by script." **It is
> not.** The binding table is only the *dispatch mechanism*; the question is who dispatches, and I
> never asked it.
>
> ⭐ **THE PHASE IS A WIRE FIELD, DELIVERED BY ACTIVITY-HOST MESSAGE TYPE 7 (`sensor_message`).**
> Verified in this session with `d1-eboot xref` and a snapshot read, three hops, no candidate list:
> * `xref 0x352720` (the PMF dispatcher that invokes the setter) ⇒ **exactly one** call site,
>   `0x029CF1C`, inside `FUN_0029CDE0`.
> * `xref 0x29cde0` ⇒ **no call/jmp at all**; its only reference is `rela 0x18F8518`.
> * `0x18F8518` is the fnptr half of RX-table entry `0x18F8510`, read live out of
>   `guardian-in-world-moonbase.snap` as **`{type = 7, fnptr = 0x29CDE0}`** - the
>   `c_activity_client::vfunc[9]` `receive_message_from_host` table.
>
> ⇒ The setter is reachable **only** from the udt-7 handler. Nothing local invokes it: no script
> tick, no activity-manager state machine, no engine path. `[C]`
>
> ⚠ **AND THE COMPLICATION, stated in the same breath** (see §4d): the handler dispatches through a
> **bound-setter reference stored in the record at `+0x108`**, and in our snapshot that reference is
> **unbound (`-1`)**. So "send udt 7" is necessary but **not yet demonstrated sufficient**. Do not
> promote this to a plan until §4d's `[U]` is closed.
>
> **The lesson, recorded:** *registered ≠ invoked.* A function in a dispatch table has a
> **mechanism**, not a caller. This is the "built-but-unwired" trap read backwards, and it cost a
> published wrong conclusion. ⇒ `status/LESSONS.md`.

**2026-08-11, session `earn-tasks-9-10`.** This is the investigation doc for the last synthetic
piece of `design/GUARDIAN-IN-WORLD.md`: what has to be true, and served, for the lifetime phase to
advance so the done-mask force and both code hooks come off.

Prior art: `historicaldocs/design/THE-ONE-ROOT-CAUSE.md` §2 (the record, its creation, its default-ness),
`historicaldocs/design/arrival-handoff.md` §5.3/§6.3 (the two-legged gate). This doc supersedes neither; it
continues them.

---

## 1. `[C]` The phase never advanced - even in-world with the biped standing

Measured offline on `runs/snapshots/guardian-in-world-moonbase.snap` (19/20 `d1-replay controls`
PASS; the 1 FAIL is the canary control at `v:0x10c500` reading our own captured hook - the
documented artifact, compensated below):

* Replayed the REAL `FUN_0010C500` by restoring the hook's 12 original bytes with
  `--set-mem 'v:0x10c500=554889e54156534883ec2049'` → **RETURNED, 94 steps, `al = 0`**.
* Probed the byte-load at `0x10c58f`: the state byte is at **`0x38026207C`**, and the dump reads
  **`0xFF` = NONE**. Record base `0x380261ED0` (the byte is `+0x1AC`); the datum element for tag
  `0x27FA2003` / class `0x8080039D` sits at `0x380262000` in this capture.

⇒ **Our `FUN_0010C500` hook carried the whole in-world result.** The membership fix did not move
the phase; the activity manager never advanced it. The record is still the default instance.

## 2. `[C]` The record's accessor family, and the one writer found

* **Read-only accessors** (all resolve globals record `0x10` via `FUN_001072C0(0x10,0,&ref)` then
  the package registry `DAT_01A0B420`): `FUN_0010C3D0` (tag), `FUN_0010C430` (state byte, raw),
  `FUN_0010C500` (state, validated - the spawn-gate/task-9 getter), `FUN_0010C5D0` (name hash,
  seeds `0x811C9DC5`), `FUN_0010C690` (`+0x1B8`), `FUN_0010C750` (state ∈ `{6,7,8}`),
  `FUN_0010C810` (`+0x1AD`), `FUN_0010C8D0` (`+0x1AE` bool), `FUN_0010C990`, `FUN_0010CA50`,
  `FUN_0010CB60` (returns `+0x674 != -1 && +0x678 != 0x811C9DC5` - **the accessor `FUN_00411830`
  itself calls**, so the spawner needs `+0x674`/`+0x678` too, not just the phase).
* **The writer**: `FUN_0010BC20(record, variant)` copies a **32-byte block into
  `record+0x1AC..+0x1CB`** - exactly the phase cluster (`+0x1AC` state, `+0x1B4` name hash,
  `+0x1B8` handle...). The value comes out of the variant's embedded pointer.
  ⭐ **Why every earlier writer scan missed it:** the getters do `add rsi, 0x1ac; movsx [rsi]` -
  the offset never appears as a memory-operand displacement, so the "exhaustive displacement-0x1AC
  store scan" (arrival-handoff §6.5) was structurally blind to a writer built the same way. The
  corpus-wide grep for `+ 0x1ac) =` found it in one pass.
* `FUN_0010BC20` has **zero direct callers**; its only reference is a RELA slot at `0x18FEB80`
  inside a `{sig_hash, fnptr, 0}` **binding table** (span at least `0x18FE900..0x18FEC00`; sibling
  getter `FUN_0010BBF0` two slots earlier). The sig hashes (`0x3088896a` etc.) repeat across many
  handlers = **type signatures, not field names**; the spellings are not in the eboot's string
  corpus ⇒ content-authored names. This is the script/reflection layer's field-write surface.
  `[I]` the writer is invoked by the spasaha script system by name.

## 3. `[C]` The instance-start applier chain (sim-queue element `0x1D`)

RTTI: `c_simulation_queue_activity_manager_instance_start_definition` vtable `0x01956600`,
apply = vfunc[2] = stub `0xBAFCD0` → **`FUN_0075B0C0(byte0, byte1, byte2)`** (3-byte payload).
Siblings: `0x1E` create_client → `0xBAFCF0` → `FUN_0075C080`; `0x1F` dispose → `0xBAFD10` →
`FUN_0075B130`.

`FUN_0075B0C0`:
1. `if (FUN_009052E0())` → `[[0x26B13F8]] + 0x4F0 + byte0*0x1F0 = 6` - the **manager instance's
   own state**, at `0x20502EAD0` for instance 0 in the guardian run. ⚠ NOT the content record.
2. `[[0x26B0A68]] + 0x10 + byte0*8 = 1` (byte) - a started flag.
3. `FUN_002B2E70()`, then tail `FUN_00762AD0(byte1, byte2)` - writes 6 into `DAT_026B1050`
   entries.

**The gate `FUN_009052E0`**: returns TRUE if `DAT_02724950 == 0` or both its first two bytes are
zero; else returns `*[DAT_02724958] != 0`. In the guardian snapshot `[0x2724950] = 0x37F8000C0`
(non-null) and `[0x2724958] = 0x204CC3710`; **neither pointee page is in the snapshot** - a
missing range is not a zero. `[U]` whether the gate passed live (⇒ live read next run).
⚠ `0x2724950` is `gg`, the roster global from `status/TODO.md` - the same object family.

⇒ Even a successful applier only marks the manager instance running. **Something else must run the
phase machine and write the content record via §2's setter - and it never runs.** `[U]` what.

### 3b. `[C]` CORRECTIONS from the instance-chain agent + the run log (2026-08-11)

* **`FUN_009052E0` is the film/replay PLAYBACK predicate** (`game_tick` renders it under
  `game_is_playback`), not an authority gate. In live play it returns 0 - the applier's state-6
  write is for replays. **Live, the state is advanced by a per-frame LADDER**, `FUN_007583F0 →
  FUN_007584C0` (from `game_tick` via `0x750a30/0x766790`): states 1→2→3→4→5, each with its own
  handler (`FUN_0075EAE0/0x75F060/0x75F1E0/0x75F710/0x760470`), and **state 6 = RUNNING is
  written only by the state-5 handler `FUN_00760470`** after: valid AH id → registered → BAP join
  request → join response → entity indices → instantiation.
* **`[C]` OUR RUNS COMPLETE THE LADDER.** `runs/20260811-021850-HYBRID-F0-2/emulator.log` walks
  every rung for instance `'PRIVATE CURRENT'` (column 0 = "PRIVATE", closing the agent's `[U]`):
  `Initialize BAP messaging` → `Pumping creation` → `Connected` → `startup response after 272ms` →
  `sent join request to AH D1A00000:00000001` → `waiting for instantiation` →
  **`'…CON-Y EST-Y AH->D1A00000:00000001 MEM-1' is ready for instantiation!`** ×3 attempts.
  ⇒ **The activity-manager instance reaches state 6 on our server.** The manager layer is
  HEALTHY; the wall is strictly above it (the script layer inside the activity-client world).
* Element `0x1D/0x1E/0x1F` are **purely client-local** (xref: publishers are the world-controller
  tick, private_repair, and `c_world_controller_state_setup_activity_host_setup_handler` - no
  network handler upstream). The AH's wire surface is the `activity_client_*` element family
  (global_state = our udt 1, join_result, entity_slots, membership, world_globals_state,
  incident, purge, advance_epoch).
* The activity-client WORLD (the spasaha world) is created at ladder state 3 via element `0x1E`
  (`FUN_00761A40`): handle → `[0x26B0A68]+0x14+idx*8`, mirrored into `[0x26B1050]`;
  `c_spasaha_engine_interface::vfunc[98/99/100/102]` read exactly that handle
  (pools: "activity clients" 0x6C88 B, "activity client worlds" 0xDB0 B @`0x263C9D8`,
  "activity client world globals" 0x5768 B @`0x263CA50`).

## 4. `[C]` The AH script channel exists and both sides stayed silent

The 49-udt AH table has **38 `script_state`**, **39 `script_event`**, **40
`request_script_updates`** plus sensors **5/6/7**. The client's RX dispatch table `0x18F84A0`
**accepts 38 (handler `FUN_0029D450`) and 39 (`0x10587F0`)** - dumped from the snapshot.

* `FUN_0029D450` (udt 38 RX): bitstream-decodes the payload via `FUN_002C9000(*DAT_01902138, …)`
  into a 16 KB script-state buffer (`FUN_001D9250`/`FUN_001D92C0`), then fires the receiving
  object's vfunc[2] notify. Payload length capped at 8000 units.
* `[C]` **In the guardian run the client sent NO udt 38/39/40** and we sent none
  (`runs/20260811-021850-HYBRID-F0-2/activity-host.log`; client→AH udts observed:
  3,14,15,17,18?,20,21,36,37,47).
* Our AH already lists 38/39 (and 5/7) as sendable (`d1-activity-host::protocol::AH_TO_CLIENT`),
  and the oracle codec path is generic over udts. **Implementation surface exists; the payload
  and the trigger are the unknowns.**

## 4b. `[C]` The udt-1 blob is delivered INTO the simulation, and its consumer map

The full path, each hop read this session:

```
udt 1 wire → RX FUN_01057B80 (decode FUN_002A4090 into a 0x6B0 stack image; ac+0x130 |= 0x40)
  → target->vfunc[7] … FUN_002407B0 → spasaha_engine->vfunc[108] (0x6E7650 stub)
  → FUN_00BAF5D0: memcpy the 0x6B0 blob into a sim-queue payload
  → FUN_002FF7C0(0x17, *DAT_0199C4F8, …)   - element 0x17 activity_client_global_state
  → applier 0xBAFB50 (c_simulation_queue_activity_client_global_state_definition vfunc[2])
  → FUN_002A14E0: memcpy(world+0x50B0, blob, 0x6B0); world+0x50A8 = 1
```

⭐ The measured dispatch distribution (`player-element-never-enqueued.md` §2) shows **case 0x17
fired 13×** during the join - our blob provably lands in world state, repeatedly.

**Consumer map of `world+0x50B0` (= our blob), via `xref 0x2A1F50` - 12 unique callers:**

| offset | reader | meaning |
|---|---|---|
| `+0x000` bool | `FUN_0077E440` | gate on the whole region-transition intake (we send 1 ✓) |
| `+0x002` bool | `FUN_0075D4C0` (← `FUN_00761FC0` ← `FUN_00769980`) | ✅ SETTLED this session: **"the activity host is draining"** - suppresses session advertising in `world_controller:matchmaking_manager`. Our 0 = "not draining" is correct. Not the phase driver. |
| `+0x010` u32 | `FUN_002A0320` | bubble count ("invalid host activity state" when 0 - solved wall) |
| `+0x014`+i bytes | `FUN_002A0320` | per-bubble state (solved wall) |
| `+0x054` 10-bit | `FUN_0077E440` | region index → transition intake `FUN_0077E5A0(…,2,…)` |
| `+0x058` u32 | `FUN_0077E440` | region name hash → same intake |
| `+0x05C` u32 | `FUN_00299100` | compared against content record `+0xC` (the checksum wall - solved) |
| `+0x060` | `FUN_00299BE0` | base of the `0x80800CC5` sub-struct handed out wholesale |
| `+0x520` u64 | `FUN_002A0FA0` | time-base-ish (`count × float/1000` math) |
| `+0x680` u16 | `FUN_0078C770`/`0x78C6D0`/`0x78C8B0`/time-of-day | activity index (solved wall) |
| **`+0x682` u16** | `FUN_0078C800`, `FUN_0078C8B0` | **a second index next to activity - `[U]`, likely zero-filled by us** |

Schema note: inner `0x80800B8F` field [9] @`+0x60` → `0x80800CC5`, whose field [1] @`+0x064` is a
**50-entry × 24 B `{u32 name-hash, kind 0x29}` array** (6-bit count) - the udt-1 RX initializes
all 50 hash slots to the FNV basis before decode. `[U]` what the array names (bubbles? scripts?
sensors?).

## 5. The two-legged gate, restated with today's facts

`FUN_00414730(entity)` refuses unless ALL of:
* leg A: `FUN_001192A0(entity)` non-NULL and `[+9] != 0` (entity-keyed record; task 9 wants
  `[+8]`),
* leg B: `FUN_0010C500` true and state ∈ `{3,6}` (the record above; `-1` renders `'NONE'`),
* `FUN_00240630/0x240670` content bytes at `+0x1C`, and a second half walking
  `FUN_00107550(0)`-resolved content (`+0x2BD4E` table).

⚠ `[O]` In the guardian run the gate still returned `al=0` **with the state hooked to 3** - that
is why the `je@0x411E16` NOP exists. Which OTHER leg refused is `[U]`; the offline replay of
`0x414730` on the guardian snapshot faults at `FUN_00344B70` reading `0x37F97AC04`
(world-controller heap, not captured). ⇒ next run: capture `0x37F97xxxx` + `0x20502xxxx` regions,
or probe live.

## 4d. ⭐⭐⭐ THE ACTUAL WRITER PATH - AH message type 7 `sensor_message` `[C]`

This section **supersedes §4c and §5b**. The chain, every hop verified this session:

```
AH udt 7 sensor_message  (server -> client, on the activity-client channel)
  → c_activity_client::vfunc[9] "receive_message_from_host"      0x00299430
      linear-scans RX table 0x18F84A0 {u32 type, u32 pad, fnptr}
  → entry {type=7, fn=0x29CDE0}                                  0x18F8510  [C, read from the snapshot]
  → FUN_0029CDE0:
      1. FUN_002D2090(payload, &name_hash, buf, &size, &type_tag)   - decode 3 schema fields
      2. FUN_001071D0(&name_hash, 0, &ref)                         - resolve the schema global
                                                                     BY NAME HASH (same globals
                                                                     array FUN_001072C0(0x10,…) walks)
      3. read the bound-setter PMF at  record + 0x108
         guard: ref[0] != 0 && ref[0] != -1 && ref[2] != -1
      4. FUN_001D3190(&box, type_tag, buf)   - box the decoded value {u32 tag, u64 ptr}
      5. FUN_00352720(record+0x108, &box)    - PMF dispatch  [ONLY caller of this dispatcher]
  → FUN_0010BC20(this, box):  *(u64*)(this+0x1AC .. +0x1C4) = ptr[0..3]
```

⭐ **The 32 bytes arrive already formed from the wire.** There is no arithmetic advance and no
constant `3`/`6` stored anywhere in the client - which is exactly why every "find the phase-advance
function" scan came back empty. `[C]`

**Corrections this chain forces:**
* `FUN_0010BBF0` is **not** a getter - it is the **clear/default-init** (ordinal 0), which is what
  leaves the cluster at `-1`. The table is `{fnptr, this_adjust, sig_hash, flags}` (an Itanium PMF
  plus metadata), **not** `{sig_hash, fnptr, 0}` as §4c wrote it; `FUN_0010BC20` is **ordinal 2**.
* `FUN_009052E0` is `game-instance resident && local simulation live` (its globals are literally
  named `"game globals"` / `"playback globals"`; the flag is driven by the simulation mode field,
  1 ⇒ live, 2 ⇒ film). A **local** predicate, true in any normal session, with no server input.
* The RX table has **no type 6**. Types present: `0,1,2,4,11,10,5,7,18,23,24,27,29,38,39,42,43`.

### ⛔ "`FUN_0010BC20` IS DEAD CODE" - CLAIMED, AND REFUTED IN PLACE `[C]`

A parallel agent returned the opposite verdict: *"the writer is dead code: zero callers transitively,
and the `runtime function bindings` pool `0x25C0C40` is written but never read - there is no
instruction sequence in this binary that stores 3 or 6 into that byte."* **That is wrong**, and the
refutation is worth more than the claim, because it is the same failure mode as this file's own
retracted §4c, running in the opposite direction.

**The dispatch chain is LIVE, verified instruction by instruction:**

```
FUN_0029CDE0 (udt 7)  --call 0x29CF1C-->  FUN_00352720(ref = record+0x108, box)
  0x352748  call 0x1CFB10          - resolve the PMF: FUN_001CFB10(out, ref)
              0x1CFB48  call 0x1D2030   - THE BINDING LOOKUP
                0x1D2087  mov rcx,[0x25C0C70] |  0x1D2094  mov eax,[0x25C0C6C]
                0x1D20A1  mov rsi,[0x25C0C58] |  0x1D20AB  mov esi,[0x25C0C84]
                                        ^^^ THE POOL, READ. Returns table_base + ordinal*0x18.
  0x35274D..0x3527A2  unpack {fnptr, adjust}, add the object offset
  0x3527A2  test r8b,1 ; 0x3527A8 mov rax,[rdi]; mov r8,[r8+rax-1]   - Itanium PMF virtual bit
  0x3527C2  call r8     ------------------------------------------->  FUN_0010BC20
```

`0x1D2030` is a real function (`0x1D2030..0x1D2150`, 71 insns) with **three** callers -
`0x1CFAE0`, `0x1CFB10`, `0x1CFB70` - the three PMF-resolution arities. `[C]`

**The two compounding artifacts that produced "dead":**
1. ⭐ **`xref <pool_base>` is blind to interior-offset access.** The reader never names `0x25C0C40`;
   it reads `+0x18`, `+0x2C`, `+0x30`, `+0x44` as four separate rip-relative absolutes. An xref on
   the base can only ever return the *constructor*, which is exactly what it returned.
2. ⭐⭐ **`d1-eboot xref`'s `fn` column attributed a frame-pointer-omitted function to the preceding
   PROLOGUE.** `FUN_001D1E80` really ends at `0x1D2020`, but the reads at `0x1D2087`+ printed as
   `fn 0x01D1E80`, so the *lookup* was rendered as part of the *registrar* - and the pool reads
   looked like pool writes.
   ✅ **FIXED 2026-08-11.** `d1-eboot xref 0x25c0c58` and `0x25c0c70` now print `fn 0x01d2030`, and
   an address in no function prints `fn -` rather than a neighbour's name. Attribution goes through
   `functions::Index::containing` - the same two-pass index `atlas --addr` always used - pinned by
   four controls in `d1-eboot controls` (31/31) with inverted halves. ⇒ ATLAS §4.2d.i,
   `status/harness.md`. The manual cross-check against `eboot-atlas.py --addr` is no longer
   required, though it remains a valid independent check.
3. The supporting *"callgraph ancestor closure = zero callers"* is not evidence at all: the call is
   `call r8`. ⚠ **`status/TODO.md` already carries the banner** *"`eboot-callgraph.tsv` fails 4/11
   controls (cannot see `lea`-materialised function pointers). Do not use it for gate discovery."*
   THE TREE ALREADY KNEW.

⇒ **The udt-7 → setter path stands.** What follows is the gate that genuinely remains.

### ⚠⚠ THE OPEN GATE - the record's setter binding is UNBOUND in our snapshot `[C]`

Read from `guardian-in-world-moonbase.snap`, record base `0x380261ED0` (confirmed: `+0x00` =
blueprint `0x80F080C3`, the in-world value `THE-ONE-ROOT-CAUSE.md` §2 recorded, and `+0x1AC` lands
exactly on the probed state byte `0x38026207C`):

| field | value | consequence |
|---|---|---|
| `+0x108` (ref[0] type tag) | `0xFFFFFFFF` | ⛔ fails `ref[0] != -1` |
| `+0x10C` (ref[1] ordinal) | `0xFFFFFFFF` | |
| `+0x110` (ref[2] object tag) | `0xFFFFFFFF` | ⛔ fails `ref[2] != -1` |
| `+0x1AC` (phase) | `0xFF` | NONE |
| `+0x1B4` | `0x811C9DC5` | FNV basis = "no name" |

⇒ **The whole record is default-constructed, the sensor binding included.** A udt-7 push today
would decode, resolve, hit the guard, and do nothing - **silently**, with no diagnostic. That is
this project's signature failure mode, so it must be settled *before* a boot, not during one.

⇒ `[U]` **WHO BINDS `record+0x108`, and is it servable?** Three shapes, and they have very
different costs:
1. **Content-populated at slice-set load** - then our record is default because something upstream
   in world-content residency failed ⇒ a content/loading fix, and udt 7 works afterwards.
2. **Bound by an earlier wire message** (a different udt, or an earlier sensor) ⇒ a second server
   surface to implement first.
3. **Bound by the activity's own authored script** ⇒ §4c's ghost returns for this one field, and
   the honest answer may still be the hooks.

⇒ Handed to an adversarially-verified workflow. **Nothing here is a plan until that returns.**

## ⛔ 4c. SUPERSEDED BY §4d - "THE WRITER IS NAMED, AND IT IS NOT A WIRE FIELD" WAS WRONG `[contradicted]`

*Kept for its evidence about the binding table and the stub channels, which stands. Its **verdict**
does not: `FUN_0010BC20` is registered in the `c_bs_game_system` table but is **invoked only by the
udt-7 handler**, not by script. See the banner at the top of this file.*

Two agents closed this. **The `+0x1AC` lifetime cluster is a 32-byte SCHEMATIZED struct written by
`FUN_0010BC20`, a setter registered in the `c_bs_game_system` (BlamScript VM) binding table
`0x18FEB50`** - not by any Activity-Host message.

* Binding table `0x18FEB50`, layout `{u64 fnptr; u64 0; u32 sig_hash; u32 flags}` × 3:
  `{0x10BBF0 getter, 0x67774671, 2}`, `{0xF8210 ret-stub, 0x3088896A, 0}`,
  `{0x10BC20 setter, 0x4D7DDECB, 2}`. Registered by `FUN_003548D0` under tag `*DAT_019024C8`
  into registry `0x267B300`; consumed by `FUN_001D1E80`, called **only** from
  `c_bs_game_system::vfunc[1]` (`0x1D2640`). The three sig hashes have **no corpus spelling** ⇒
  content-authored BlamScript function names. `[C]`
* The setter copies 32 bytes from `variant+8` into `record+0x1AC`; the paired getter default-inits
  from schema `*(DAT_019024C0+8)`. So `+0x1AC` is a script-VM variable, set by the **map's own
  BlamScript running locally**, per machine.

### The AH "script" channel is a decode-and-drop STUB in build 01.29 `[C]`
* **udt 38 `script_state`** (`FUN_0029D450`): decodes into a 16 KB name-table+arena buffer
  (`FUN_001D9250`/`FUN_001D92C0` - both have exactly ONE caller each, this handler), fires
  `c_activity_client::vfunc[2]` (`0x2992A0` = `mov rax,[rdi+0xC98]; ret`, the AH-handle getter),
  and **discards the decoded result**. `FUN_002C9000` hard-returns 1 so the guard is vacuous.
* **udt 39 `script_event`** (`0x10587F0`): same shape - decode a 272-byte record, fire the same
  no-op getter, drop it.
* **udt 40 `request_script_updates`**: **no sender exists.** All 31 client→AH sends go through
  `FUN_00299310(this,dest,type,tag,src,size,flags)` with a literal type; 38/39/40 are absent from
  the set. There is no other send helper.

⇒ ⛔ **A server that pushes a perfectly-formed udt 38 changes nothing on the client, and does not
even error.** The dead 16 KB buffer type strongly suggests an *earlier* build wired script_state
to the variable store; 01.29 does not. `[I]`

### udt 2 `world_globals_state` is REAL but writes the wrong object `[C]`
`FUN_01057EF0` (real, not a stub) → element `0x18` (`FUN_00BAF640`, `*DAT_0199C500`) → applier
`FUN_002A2270`: writes `world_globals+0x14` and authority masks `+0x54/+0x5C/+0x60`. That is the
**world-globals** object (`FUN_002A0FA0` reads its `+0x520` time-base), **not** the `+0x1AC`
content record. We never send udt 2 (`activity-host.log`: 0 occurrences). Candidate for other
effects, but not the phase driver.

## ⛔ 5b. RETRACTED - "the documented cloud-script ceiling" `[contradicted]` ⇒ §4d

*The reasoning below is superseded. The phase is delivered by AH udt 7, a wire field. The udt-38
`script_state` stub findings in §4c stand as evidence about **that channel** - they simply were
never the channel that carries this field. What remains genuinely open is §4d's `+0x108` binding.*

### (superseded text follows)
## 5b-old. THE HONEST CONCLUSION - this is the documented cloud-script ceiling, now pinned

`SERVER-ROLES-PLAN.md` §8 predicted it: *"the Activity Host's mission logic is cloud-side script.
Reaching rung 36 yields a loaded but EMPTY world."* This investigation **pins the exact
mechanism**: the lifetime phase `{NONE→3/6}` is a **BlamScript variable** advanced by the
activity's authored script, and the one wire channel that could have replicated script state
(udt 38) is a stub in 01.29.

⭐ **BUT the BlamScript VM is LOCAL and the scripts are content in packages we have decrypted on
disk.** So the phase is not necessarily unreachable - the sharp remaining question is:

> **Does activity 407's BlamScript actually LOAD and TICK on our server, and if so what precondition
> is it stuck waiting on?** If it ticks but waits on something we can serve (a sensor, entity slots,
> an event), there is still a served path. If it never loads because we don't serve a script-content
> reference, serving that reference is the path. Either way the door is the BlamScript VM, not an AH
> field. `[U]` - handed to a focused agent.

## 6. Next (queue for this session)

1. Agents out reading: the script_state decode consumer, the binding-table walker / spasaha phase
   machine, and the instance-start → tick chain. Their findings land here.
2. Live-run discriminators, in one boot: read `[[0x26B13F8]]+0x4F0` (did the applier run), read
   the `FUN_009052E0` gate pointees, replay-toggle: **poke `record+0x1AC = 6`** (address computed
   live via the `FUN_0010C430` walk) with NO `0x10C500` hook - if task 9 then completes and the
   spawn gate passes without the NOP, the record is the whole wall and "make the server cause
   that byte" is the entire remaining work. A poke to identify, never to satisfy.
3. Then: implement the server-side cause (candidate surfaces, in current preference order:
   udt 38 `script_state` push; a `global_activity_state` field we zero-fill; sensors 5/6/7).
