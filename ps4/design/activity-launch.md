# Activity launch - orbit to `activity:in_world`, canonical reference

**State (2026-08-11):** the boot ladder walks orbit → 28…36 cleanly and reproducibly on our own
Activity Host answers. Every gate on the path is named and most have wire knobs; the remaining
synthetic pieces (done-mask force, two code hooks, spawn poke) are owned by
`design/GUARDIAN-IN-WORLD.md`. The lifetime/phase record (the last big wall) is the CURRENT FRONTIER
and lives in `design/activity-lifetime-state.md` - this doc references it, it does not duplicate it.
Reach for the whole ladder: `synthetic_ancestor` - every gate below rung 34 was crossed on our own
answers.

Consolidated 2026-08-11 from twelve design docs (see Sources). Banners in the sources override their
bodies; every claim killed by a banner is listed once, in "Dead claims", and never carried as live.

## Contents

1. [The ladder](#1-the-ladder)
2. [What we launch - MODE plus MAP](#2-what-we-launch---mode-plus-map)
3. [The levers](#3-the-levers)
4. [The rung-34 task cluster](#4-the-rung-34-task-cluster)
5. [Arrival / spawn handoff findings](#5-arrival--spawn-handoff-findings)
6. [Orbit as a viewer](#6-orbit-as-a-viewer)
7. [Boot-to-x: the static shortcuts](#7-boot-to-x-the-static-shortcuts)
8. [Open [U]s](#8-open-us)
9. [Dead claims](#9-dead-claims)
10. [Sources](#10-sources)

---

## 1. The ladder

**Reach:** `static-client-inference`, generated/verified by `capture-rig/state-machine.py`
(11 controls, exit 2 on failure), cross-checked against RTTI class names and the `bootflow:` state
strings. ⇒ historicaldocs/design/activity-launch-ladder.md.

### 1.1 The mechanism `[C]`

**The ladder is self-driving from rung 27 onward.** 35 classes, all deriving from
`c_world_controller_tiger_base_state_handler` (vtable `0x192D920`), one per state, each with a
per-frame `update` (vtable `+0x38`, `vfunc[7]`, pure-virtual in the base, overridden by all 35).
**Each rung's update requests the next rung; nothing external walks the machine.** The whole
question "how do we get into an activity" reduces to: what does each rung wait for?

Per-transition lifecycle slots, settled in historicaldocs/design/arrival-handoff.md §1.6 `[C]`:
**slot 4 (`+0x20`) is ENTER, slot 5 (`+0x28`) is LEAVE** (the client's own
`Leaving state '%s' for reason '%s'` / `Entering state '%s'` printfs stand next to the two indirect
calls in `FUN_00782300`). Slot 2 (`+0x10`) is the construction/registration hook (rung 34's slot 2
`0x00794050` writes the 11 task entries), not the per-transition enter. Read
`state-machine.py`'s `slot28` column as LEAVE; its docstring's `+0x10`/`+0x40` pair is a different
lifecycle.

Transition primitives:

```
0x782720(state, reason_code, reason_name)   the real one - owns `world_controller:state_manager`
0x783690(state, reason)   -> 0x782720(state, reason, NULL)
0x782710(state)           -> 0x782720(state, 0, NULL)
0x783BE0                  `world_controller:state_manager: On the way to goal-state '%s'...`
```

**The state id is literally the rung number** (`mov edi, 0x1d ; call 0x783690` = request
29 = `setup:matchmaking`). `esi` is a reason code, not a parameter: 20 of 27 sites on `0x783690`
request rung 25 `cleanup` with a distinct reason (`0x9, 0x38, 0x79, 0x82, 0x8E, 0x90, 0xA4, 0xAF,
0x111, 0x112, 0x11B`, ...) - the mapping onto the 260 `error.*` reasons. Reasons `< 0x10` are the
non-error class; `>= 0x10` kicks to `bootflow:start`.

### 1.2 `cleanup` is a router, and every rung pumps a task BITMASK set `[C]`

`c_world_controller_state_cleanup_handler::vfunc[7]` = `0x7A3750` carries its destination as data:
once `tasks.done_mask (+0x6B0) == tasks.requested_mask (+0x6A0)` it dispatches to the goal state
stored at `+0x6C8` with reason `+0x6CC` and sets `+0x6BC = 3` (requested). Most transitions route
**through** cleanup. `[U]` who writes `cleanup + 0x6C8`.

`+0x6A0`/`+0x6B0` (and rung 28's `+0x118`/`+0x128`) are **64-bit bitmasks over a fixed-size task
table**, not queue pointers (see Dead claims #1). One table entry is `0x20` bytes:
`{u64 prereq_mask; void *fn_plain; void *pmf_ptr; long pmf_adj}` (Itanium pointer-to-member, low bit
set ⇒ virtual). A task runs when `(done ^ requested) & prereq_mask == 0`; returning 1 sets the
started bit, 2 sets started and done; the runner stops when `done == requested`. Seven template
instantiations, seven offset agreements (`8 + N*0x20` lands on each runner's own mask offsets):

| runner | rung | masks at | mask literal | N tasks |
|---|---|---|---|---:|
| `0x1092230` | 1 `media_check` | `+0x68/0x70/0x78` | - | 3 |
| `0x1092690` | **28** | `+0xE8/0xF0/0xF8` | `0x7F` | **7** |
| `0x1092160` | **34** | `+0x168/0x170/0x178` | `0x7FF` | **11** |
| `0x10924C0` | **25** `cleanup` | `+0x668/0x670/0x678` | `0x7FFFFFFFFFFFF` | **51** |
| inlined | 26, 33, 36 | (each) | `& 1` | 1 |

Ghidra types: `s_world_controller_task_set_{3,7,11,51}` in `ghidra/structs/d1-structs.tsv`.

### 1.3 The ladder, 25 → 36 `[C]`

| rung | state | enter | **update** | exit |
|---:|---|---|---|---|
| 25 | `cleanup` | `0x7A0750` | **`0x7A3750`** | `0x7A43A0` |
| 26 | `setup:orbit_loading` | `0x7AC4B0` | **`0x7AC630`** | - |
| 27 | `setup:orbit` | `0x7AAC20` | **`0x7AAF60`** | - |
| **28** | `setup:activity_session_creation` | `0x7A6540` | **`0x7A6D60`** | `0x7A7050` |
| **29** | `setup:matchmaking` | `0x7A8A20` | **`0x7A8D10`** | - |
| **30** | `setup:activity_host_setup` | `0x7A4FC0` | **`0x7A5250`** | `0x7A6380` |
| 31 | `setup:prologue_intro_loading` | `0x7ACB30` | **`0x7ACD90`** | `0x7A6380` |
| 32 | `setup:orbit_outro` | `0x7ACB30` | **`0x7ACB90`** | - |
| 33 | `setup:activity_world_transition` | `0x7A72C0` | **`0x7A7C00`** | `0x7A7E80` |
| 34 | `activity:initial_slice_set_loading` | `0x794050` | **`0x795130`** | `0x795180` |
| 35 | `activity:physics_join` | `0x796380` | **`0x796460`** | - |
| 36 | **`activity:in_world`** | `0x795370` | **`0x7957E0`** | - |

Each update requests the next (verified against the transition census): 27's update `0x7AAF60`
requests **28** at `0x7AB460`; 28 → 29 at `0x7A6E7C`; matchmaking helper `0x7A9C20` → 30 at
`0x7A9ECE`; 30 → 31 at `0x7A636C`; 31 → 32 at `0x7ACFB5`; 32 → 33 at `0x7ACBBF`; 33 → 34 at
`0x7A7E36`; 34 → 35 at `0x795165`; 35 → **36** at `0x7964EE`. Rung 26 is reached via cleanup's
stored goal state, so no immediate exists to find. ⚠ `state-machine.py --fn` annotates transitions
from the nearest preceding `mov edi, imm`, which is WRONG at tail-merged shared call sites; treat as
a hint, confirm at the branch.

### 1.4 The gate out of orbit is a network session parameter `[C]` for the code, `[U]` layouts

`0x7AAF60` (orbit's update) stays in orbit until the session parameter at
`FUN_00773fc0(0) + 0x9b8` has a value (a short, `-1` = unset; `0x811C9DC5`, the FNV-1 offset basis,
is the "unset" sentinel), then requests rung 28 (`0x1c`). ⇒ **the activity to launch is carried as a
session parameter, not a BAP call** - which is also how a fireteam leader's choice reaches members.

The parameter classes, named by RTTI: `c_network_session_parameter_world_controller_goal`
(`s_network_session_world_controller_goal`), `..._desired_activity`
(`s_desired_activity_information`), `..._activity` (`s_activity_information`),
**`..._activity_host` (`s_activity_host_connection_information`)** - the AH's address is a session
parameter that rungs 28-30 fill in - `..._desired_roster_join_activity`, and
`c_generic_network_session_parameter<...>` over `s_activity_start_status`,
`s_replicated_activity_matchmaking_progress`, `s_network_session_parameter_initial_slice_set_status`,
`s_network_session_parameter_orbit`. `[U]` the session object's parameter offsets and the layouts of
`s_activity_host_connection_information` / `s_network_session_world_controller_goal`.

432 diagnostic rows matching `activity|matchmak|session_creation|activity_host` were `seen=0` across
205 runs (overwhelmingly `evidence=strong`; positive control on the SEEN column:
`0x1648A2F`/`0x1648BB4`/`0x1648BF8` all `seen=1`) ⇒ `[C]` the client had, at that measurement, never
entered rungs 28-30 in any recorded run. That string cluster - notably `0x7A7ED0` (rung 33, sole
caller `0x7A7420`: `Could not get the activity name from the cli...` / `Unable to get world options
from the activit...` / `Setting world options based on game mode '%s...'`) - is the producer of the
change-world command's `activity_name` and world options.

### 1.5 The command queue is NOT the launch path `[C]`

All 17 handlers are registered at `0x752ED0` (the only caller of registrar `0x34BDC0` in the image),
into the world-controller command queue singleton `[0x267B110]` (`0x34C140`). Slots 15 and 17 are
never registered (NULL handler). **There is no "launch activity" slot** - the queue is
`world_controller`'s resource/world channel, which is why forcing slot 3 loaded a world and moved no
state. The client's own describer strings name each slot; the two that matter here:

| slot | handler | the client's own name |
|---:|---|---|
| 2 | `0x7532D0` | `Change slice-set (slice-set='%d', spawn='%s').` |
| **3** | **`0x753540`** | **`Change world (activity_name='%s', symbol='%s', rebuild_omaha_options='%s', random_seed='%c 0x%08X')`** |
| 10 | `0x754320` | `Reload world.` |
| 11 | `0x754460` | `Exit game.` |

Dispatch is a queued-command pump: `0x751060` → `0x34C010`, which at `0x34C0BC` calls
`handlers[slot]` (`[r14+r13*8+0x80]`) with `RDI` = the per-slot buffer at
`registry + 0x200 + slot*0x1C04`. **`0x753540` is never called directly at runtime** - its two
`call rel32` sites (`0x75440D`, `0x754484`) never fired across four observed world changes; the
riprel registration at `0x752F47` is the real edge. `[C]`
⇒ historicaldocs/design/world-launch-command.md §4.

---

## 2. What we launch - MODE plus MAP

**Reach:** `static-client-inference` + `[O]` from run `20260809-084932-RUNG33-PERMS6`;
`synthetic_ancestor`. ⇒ historicaldocs/design/activity-and-map-namespace.md.

A launch is **two independent values**, adjacent fields of the `global_activity_state` blob
(`world + 0x50B0`), and **the name is nowhere derived from the index**. Setting one and not the
other is a silent misconfiguration.

| | what | where | knob |
|---|---|---|---|
| **MODE** | `u16` activity index into the client's 540-entry table | `global_activity_state + 0x680` | `D1_AH_ACTIVITY_INDEX` |
| **MAP** | NUL-terminated ASCII world name | `global_activity_state + 0x688` (`char[0x28]`) | `D1_AH_ACTIVITY` |

### 2.1 The MODE - 21 launchable activities, all Crucible private-match `[C]`

Rung 29 runs the matchmaking seeker unless `activity_def[+0xA9] == 0`. That is 21 of 540: indices
**393-413** exactly (independently re-derived from `ps4_investment_globals_012f_3` block 1063,
elements at `0x9920`, stride `0x10`, `{u32 hash, u32 pad, i64 self-rel -> definition}`).

| idx | mode | idx | mode | idx | mode |
|---:|---|---:|---|---:|---|
| 393 | Clash | 401 | Rift | 409 | Inferno Skirmish |
| 394 | Control | 402 | Skirmish | 410 | Inferno Salvage |
| 395 | Supremacy | 403 | Inferno Clash | 411 | Inferno Doubles |
| 396 | Mayhem Supremacy | 404 | Inferno Control | 412 | Mayhem Rumble |
| 397 | Inferno Supremacy | 405 | Mayhem Clash | 413 | Sparrow Racing |
| 398 | Rumble Supremacy | 406 | Zone Control | | |
| 399 | Rumble | **407** | **ELIMINATION** | | |
| 400 | Salvage | 408 | Inferno Rumble | | |

**407 = Elimination is the project's stated endgame** (3v3 private-match); identical gate profile to
393 (`+0xA9 = 0x00`, `+0x8C = 0`). Prefer 407 for test launches.

⛔ **Patrol is NOT reachable on this route.** Patrol Cosmodrome (idx 7) and Patrol the Cosmodrome
(idx 92) both carry `+0xA9 = 0xFF` (matchmaking REQUIRED) and content-family `+0x8C = 4`; the Tower
(idx 0) is also `0xFF`. The constraint is not PvE-vs-PvP; only the private-match band skips
matchmaking. A patrol launch stalls at rung **29**. ⚠ Do not trust the Bungie manifest's
`is_matchmade` column - both patrols read `False` there while the client's own `+0xA9` requires
matchmaking. Indices 369-372 (`Icarus`, `Skyline`, `Last Exit`, `Floating Gardens`) are MAP names in
the activity table, all `+0xA9 = 0xFF` - a map cannot be launched as an activity.

### 2.2 The MAP - the world-name namespace

**Package inventory** (`dumps/patch-01.29-CUSA00219/packages/`, 1,647 packages): every PvP map ships
a three-part set (`_activities_a`, `_ambient_acti`, `_destination`) ⇒ a map is a self-contained
world `[C]`. **32 PvP map stems**:

```
pvp_anomaly      pvp_arena_hive   pvp_breach       pvp_cabal_camp   pvp_city_defense
pvp_colony_ship  pvp_cosmobattle  pvp_den          pvp_dos          pvp_eq
pvp_factory      pvp_greenhouse   pvp_hive_cell    pvp_icarus       pvp_ketch
pvp_longshot     pvp_marsbattle   pvp_moonbase     pvp_oasis        pvp_outskirts
pvp_refinery     pvp_rift         pvp_rooftops     pvp_srl_mars01   pvp_srl_venus01
pvp_subway       pvp_tomb         pvp_venus_hcc    pvp_vertigo      pvp_vex_tube
pvp_well         pvp_wilderness_town
```

⚠ **Unresolved count discrepancy, both kept:** this inventory yields **32** stems `[C]`, while
`historicaldocs/design/package-container-pvp-content.md` §7.5 (cited by two other sources) enumerates **33 PvP
maps** `[R]`. No banner resolves it; reconcile against §7.5 before citing either number.

**Symbol tables** (all 1,647 packages, 3,242 symbol records, 1,114 distinct names →
`capture-rig/world-namespace.tsv`): **54 `map:<slug>:root` symbols, 242 top-level (colon-free)
activity symbols.** Every destination ships the family `<activity>`, `<activity>:scenario_client`,
`<activity>:same_dest_filler`, `<activity>:spaceflight_filler`, `<activity>:spaceflight_out`,
`ambient_<dest>`, `map:<dest>:root`. `city_tower` demonstrates map ≠ activity: ONE map, seven
activities on it (`city_tower_default1` `0x545AEC79`, `_crimson` `0xFD399DE0`, `_fol` `0xDACB5210`,
`_harvest` `0xC0D6AD44`, `_iron_banner` `0x2299CFA8`, `_queen` `0x3A3C3609`, `_srl` `0xC5AC70AE`).

**The string to publish at `blob+0x688` is the top-level ACTIVITY symbol** `[C]`, two independent
instances: `pvp_moonbase` (published, `successfully changed world to: pvp_moonbase` observed live)
and `act_cosmo_m0_highway` (= *A Guardian Rises*, measured working). Note
`act_cosmo_m0_highway` does NOT match its package stem (`cosmo_activities_activity`), so stem →
world-name is not an identity; world names are content strings, not eboot strings (29,840 eboot
strings contain no `act_cosmo` and no `pvp_` world names). A wrong name fails LOUD:
`world_controller:state:prologue_intro_loading: Activity '%s' does not exist.` (never fired in 231
runs), and the AH hot-swaps, so each candidate costs a poke, not a boot.

The identifier hash is **FNV-1 32-bit** (`h = (h * 0x01000193) ^ c`, multiply THEN xor - NOT
FNV-1a, though both share basis `0x811C9DC5`): `orbit` → `0xB084DD2B`, `pvp_moonbase` →
`0x65BDC6F8`, both matching live-measured scenario records. D1 and D2 share the hash and much of the
namespace machinery (§6).

---

## 3. The levers

### 3.1 The activity checksum - `global_activity_state + 0x5C` `[C]` `observed-live`

> **CONFIRMED LIVE 2026-08-09:** `D1_AH_ACTIVITY=orbit D1_AH_ACTIVITY_INDEX=407
> D1_AH_ACTIVITY_CHECKSUM=0x33f044d3`, poked 407 - `Activity checksum mismatch!` GONE.
> ⇒ historicaldocs/design/activity-checksum.md.

**The checksum is a property of the WORLD (the scenario), not of the activity.**
`expected = *(u32*)(scenario_record + 0x0C)`, where the record is what the content tag at
`activity_manager + 0x278` resolves to (manager singleton `*(u64*)0x26B13F8`), and `+0x0C` is
selected because `c_activity_client + 0x18` indexes `{"PRIVATE","PUBLIC"}` and the AC logs itself
`[[AC PRIVATE CURRENT CON-Y EST-Y AH->D1A00000:00000001 MEM-1]]` ⇒ index 0. Comparator
`FUN_00299100` (sole caller `0x75A9E0` @ `0x75AA56`): `cmp eax, [rcx+rdx*4+0xc]` with
`eax = blob+0x5C`, `rcx` = the record resolved through registry global `0x1A0B420` (`>>13` bucket +
13-bit index, D1's tag encoding), `rdx = ac+0x18`.

`param_3` is the **scenario handle** - `*(u32*)(manager + 0x278)`, written by
`c_online_world_controller::vfunc[3]` (`0x75B8B0`), set to `-1` by `vfunc[4]` (`0x75BAF0`), and
named by the client's own `world_controller:activity_manager: No valid scenario handle -- unable to
get matchmaking settings for slice-set '%d'.` ⛔ The `0xFFFFFFFF` skip branch is client-side state;
it cannot be published, and poking it would satisfy the gate rather than identify it.

The `orbit` scenario record, read live (tag `0x80B52003`, slot `0x5A9`):

| offset | value | what |
|---|---|---|
| `+0x08` | `0xB084DD2B` | FNV-1(`"orbit"`) |
| **`+0x0C`** | **`0x33F044D3`** | checksum, index 0 = PRIVATE |
| `+0x10` | `0x718882D2` | checksum, index 1 = PUBLIC |
| `+0x30` | self-rel → `"orbit"` | ASCII name |
| `+0x48` | `16` | slice-set / bubble count |
| `+0x50` | `0x20` self-rel | bubble array at `record+0x80`, stride `0x18` |

Consequences: changing `D1_AH_ACTIVITY_INDEX` cannot change the expected checksum; changing
`D1_AH_ACTIVITY` (the world) can. ⛔ A PvP map's record cannot be read from orbit (only the loaded
world's scenario is resident); the instrument is
`python3 -B capture-rig/scenario-checksum.py watch --hz 50` during a poke - it prints the resolved
record (name, `+0x0C`, `+0x10`) each time the handle changes, in the ~100 ms window between
`load successful` and `returning to pregame`. A byte grep of packages does not shortcut this
(`FNV-1("pvp_moonbase")` appears 0 times in the 18 `ps4_pvp_moonbase_*` packages; the record lives
in compressed blocks). `pvp_moonbase`'s checksum was later measured: `0x5966e491` (used in the
rung-34 runs). ⚠ The AC array's stride is a `u32` (`0x29F610`: `DAT_0263C958 + (h & 0x1fff) *
DAT_0263C960`, `imul` on a dword); reading it as a qword yields garbage.

### 3.2 The second gate - bubble count and per-bubble state (`FUN_002A0320`) `[C]`

The instant the checksum passes, `0x75A9E0` calls `FUN_002A0320` on the same scenario record, with
`do_validate` true for the primary activity client (session 0). Rules:

| # | blob offset | width | record offset | rule | `orbit` value |
|---|---|---|---|---|---|
| 1 | `+0x5C` | u32 | `+0x0C + ac[+0x18]*4` | equal | `0x33F044D3` |
| 2 | `+0x10` | u32 | `+0x48` (bubble count) | equal | `16` |
| 3 | `+0x14+i` | i8 × count | per-bubble chosen state | see below | - |

Failure strings, `[C]`: **`invalid host activity state`** (`blob+0x10 != record+0x48` and
`blob+0x10 == 0` - the publish-nothing case), `activity client bubble count mismatch` (wrong
non-zero count), `activity client bubble state index was valid, but state itself was invalid`
(bubble chosen `0` but the scenario's own state-0 flag clear), `activity client bubble state > 1
chosen`. Knob: **`D1_AH_BUBBLE_COUNT`** writes `blob+0x10 = count` and `blob+0x14[0..count] = 0xFF`
in one call; refuses count > 64 (the `0x80804050` struct is 64 bytes); all values survive the
client's own codec (`FUN_002A4140` → `FUN_002A4090`, round-trip controlled).

⛔ **But `0xFF` per bubble is only half the story.** `0xFF` ("unchosen") passes rung 33 and then
**UNREGISTERS the bubble's slice-set region**, guaranteeing rung 34's
`error.sstm_slice_not_found` ("lobster") - one field, two rungs, opposite signs (§4.2). Knob:
**`D1_AH_BUBBLE_CHOSEN`** (accepts `0`, `0-4,9-14`, or `all`; out-of-range refuses to start). For
orbit the correct pattern is `D1_AH_BUBBLE_COUNT=16 D1_AH_BUBBLE_CHOSEN=0-4,9-14` (orbit bubbles
5-8 and 15 have sub-bubble count 0 ⇒ must stay `0xFF`); for a one-bubble PvP map,
`D1_AH_BUBBLE_COUNT=1 D1_AH_BUBBLE_CHOSEN=0`.

### 3.3 Who writes `c_activity_loader + 8` - and why it is not a gate `[C]`

> Adversarially re-verified 2026-08-09; headline confirmed:
> **`FUN_00341500 @ 0x341798` is the writer, it ran, and `c_activity_loader` is not a gate.**
> ⇒ historicaldocs/design/activity-loader-writer.md.

`c_activity_loader` is the singleton at `0x267AF08` (`FUN_0033ff30()` is literally
`lea rax,[rip+...] ; ret` = `&DAT_0267af08`); `+8` is a pointer to a `0x3DC`-byte block allocated
once by `vfunc[1]` (`0x33FF40`) with `block[0] = 0` (empty name). Block layout: `+0x000` CURRENT
activity name `char[0x100]` (**what rung 33 reads** - `FUN_00340240(p) = **(char**)(p+8) != 0`);
`+0x100` 8 content tag qwords (`-1`); `+0x140/+0x144` state; `+0x148` PENDING name (same record
shape, stride `0x148`); `+0x290` loader task handle; `+0x294` mirror of the last written slot.

* **`FUN_003404D0 @ 0x340558`** writes the PENDING slot (the request). When CURRENT is empty it
  *cannot* write it.
* **`FUN_00341500 @ 0x341798`** is the only instruction in the image that drives `block[0]` from
  empty to non-empty: `memcpy(block, block+0x148, 0x145)`, the pending → current promotion. Guard:
  current empty-or-different AND pending occupied AND `strcmp(name, block+0x148) == 0`; otherwise
  `LAB_003416dc` returns TRUE having written nothing.
* Erasers: `FUN_00341a50` (`0x341bce mov byte [r12],0`), inlined via `FUN_003401c0` (unloads slot 0;
  called unconditionally at `0x7539e4` before the promotion - the premise that makes the proof
  airtight) and `FUN_00340140` (**full teardown** of both slots; called at `0x753f97` on the
  change-world failure tail).

**In the failing rung-33 runs the name WAS written** `[C]` `observed-live`: `FUN_0034cb50` can only
return FALSE (printing `load successful, but failed to instantiate world and start game`, 5/5
transitions) if `FUN_00340240(loader)` returned TRUE at `0x34cb87`; combined with the
unconditional pre-clear at `0x7539e4`, only `0x341798` can have written it. The five
`failed to change to '<name>'` lines carried real names every time (`act_cosmo_m0_highway`,
`orbit` ×3, `pvp_moonbase`). *"Transition completed, but we don't have an active activity"* is a
**post-mortem, not a cause**: the failure tail unloads the activity and clears the name again before
rung 33 looks. ⇒ the single unmet obligation on the rung-33 path was the checksum (§3.1), by
elimination of the other seven links (world name delivered · pending written · name resolved
(`activity '%s' not found` = 0) · content tags passed · promotion ran · instantiate FAILED ·
teardown ran).

Negative-claim control: the admissible channel is the `FUN_000e4da0(5, 4)` filter feeding
`FUN_000e4ad0` (fired 5x in the same function); the second channel
(`FUN_002416b0("change-world", "loading %s"/...)`) provably ran 5x and produced **0** log lines - a
zero-count claim on that channel would be flatly wrong.

### 3.4 The captured `world_launch` command (queue slot 3) `[C]` shape, `observed-live`

Specimen: `runs/20260808-142443-WORLD-LAUNCH-1080-C/account-trace.bin.blob`, probe 88
`WorldChangeEntry` at `0x753540` entry, guest `0x20503b81c`, 6,392 bytes (`0x18F8`), 530 non-zero.

```
+0x0000  u32   0x000018F8      self-describing size
+0x0008  u32   0x00000001
+0x0010  50 x 24-byte ACTIVITY SLOTS (stride 0x18, ends 0x4D0)
           +0x00 u32 content/activity hash   +0x04 u32 0xFFFFFFFF   +0x08 16 bytes
+0x059E  run of u16 0xFFFF empty-slot sentinels
+0x0630  char[]  THE WORLD NAME, NUL-terminated ASCII  ("ambient_bootstrap")
+0x0650  u32 0x00000080 entries
```

All 50 slots read `{0x811C9DC5, 0xFFFFFFFF}` = unset; the specimen is a world-only load ("no
activity specified" is the matching diagnostic), proving shape + sentinels + that **the world is
named by a STRING, not a hash** `[C]`. Command field `+0x630` is `activity_name` per the client's
own slot-3 describer; the command's `0x650` state block is copied to `scratch + 0x12F8` and
`0x12F8 + 0x628 = 0x1920` ties it to `global_activity_state + 0x688`. The buffer address is
derivable at runtime: `slot 3 buffer = [0x267B110] + 0x200 + 3*0x1C04`; an untimed external poke is
ruled out (the queue clears its slot). `[U]` the activity-slot hash space (only the unset value ever
observed), the meaning of `+0x08` and the `0x00000080` entries.

### 3.5 KTOBUCKET - the client's own launch parameters, in every failure dump

**Reach:** `static-client-inference` for semantics; `synthetic_ancestor` for all observed values.
Tool: `capture-rig/session-params.py --ktobucket <run>` (22 controls + 4 inversions, exit 2 on
failure). Emitter `FUN_00782720` dumps eleven pinned printf formats (byte-exact at their eboot
offsets, e.g. `0x164C307` `ACTIVITY ACTNAME:[%s] SLICESET HASH:[0x%08x]`, `0x164C3F4`
`ACTIVITY: activity=%d, insp=%d, override activity=%s, bubbles=%s, reservations=%d, nonce=%d`) every
time a world-controller transition fails.

**The launch-request struct is `c_activity_client + 0x138`** (`0x2990F0`: `lea rax,[rdi+0x138]`),
gated by `ac+0x130 & 2` (`0x299A80`). Six pokeable levers:

| printed | in request | absolute | what | only value ever seen |
|---|---|---|---|---|
| `activity` | `s16 [req+0x0E]` | `+0x146` | activity-definition index | **393** |
| `insp` | `s8 [req+0x10]` | `+0x148` | insertion point (spawn/phase) | **-1 sentinel** |
| `override activity` | `char[] [req+0x98]` | `+0x1D0` | replacement activity def | `""` sentinel |
| `bubbles` | `char[] [req+0x198]` | `+0x2D0` | sub-regions to load | `""` sentinel |
| `reservations` | `u32 [req+0x698]` | `+0x7D0` | player-slot reservations | 1 |
| `nonce` | `u32 [req+0x04]` | `+0x13C` | per-attempt launch nonce | varies |

String field sizes are `[I]` from printf adjacency, not a proven capacity. Across 208 dumps:
`ACTNAME=NONE` + basis hash = the rung-30/33 attempts; `orbit` + `0x3559729a`/`0x8265820b` = slice
set resolved in orbit. ⚠ A sentinel slice set correlating with failure is a correlation, not a
mechanism `[I]`. Two reader traps: `-` in the table means the line was never printed (no established
AC), not "unset"; and **shadPS4 EATS duplicate KTOBUCKET lines** (`Skipped N duplicate messages..`),
which the reader detects and charges correctly (controls K9a/K9b).

### 3.6 The permission walls at rung 33, and the ten-flag union `[O]` `synthetic_ancestor`

From run `20260809-225119-UNION8-MOONBASE` (⇒ historicaldocs/design/rung34-task7.md): **baboon**
(`activity_failed_to_load_missing_dlc`) cleared by the union-8 flags `D1_503_RAW`+`D1_205_RAW`
`{98,308,419,440,834,3389,5015,6018}` (guard printed `205/503 agree on the shared 0x80801941 (44
bytes)`; `missing_dlc` 3 → 0). **urchin** (`activity_failed_to_load_missing_permissions`) needed
two more: poking `{2667, 5987} = 0x02` into the derived plane (`[0x26A34E8] + 0xC + index`) passed
rung 33 (`successfully changed world to: pvp_moonbase` + `World-transition finished.`). Positive
control: the four shipped union-8 flags read `0x02` in the plane, the two missing read `0x00`.
⇒ **the correct 503/205 body is a TEN-flag union `{98,308,419,440,834,2667,3389,5015,5987,6018}`** -
serve it and both walls fall with no poke. Serving grants/flags is owned by
`design/grants-and-unlocks.md`.

⛔ **olive** (`error.investment_peer_manager`): 59x `Failed peer-subscription validation
[error error_timeout]` tears the activity down from underneath and degrades with time/attempts - a
poke after olive appears is a poisoned-connection measurement; stop and reboot. Related: the AH
codec's scratch-heap exhaustion self-terminated long parks (~16 min → `error.
private_activity_host_host_crashed`); the bump-arena bug is FIXED, but do not read an old long-park
fallback as a client gate.

---

## 4. The rung-34 task cluster

Rung 34 (`activity:initial_slice_set_loading`) pumps **11 tasks** via `FUN_01092160`. Handler
pinned live: state-manager `[0x26B13E8]`, handler table `sm + 0x80 + state*8`, slot `0x22` →
**`0x26B5588`** (BSS, vtable `0x192D9B0`). Masks (`tasks` at `handler+0x30`): `+0x198`
**requested** (`vfunc[4]` `0x795090` assigns `0x7FF` on entry), `+0x1A0` **started**, `+0x1A8`
**done**; task 7's tick counter `+0x1B0`, task 10's `+0x1B4`. Read them:
`d1guest read 0x26B5720 --len 0x20`. Update `0x795130`: pump, then
`if (done == requested) FUN_00782720(0x23,0,0)` → rung 35. **The done mask has exactly one
consumer** - that `if` (`field-xref`, 3 positive + 1 inverted control) `[C]`.

### 4.1 Task 8 - the PAH tabulated region record (served, cleared 2026-08-09) `[C]`

Task 8 (`FUN_00794CC0`) waits on `FUN_0077A150()` = bit 7 of `[[0x26B1438]+2]`, set only when the
slice-set transition manager (`sstm = [0x26B1408]`) assigns the slice-set-switch task, which
requires a **PAH tabulated region record** inside `replicate_membership` (tag `0x808009ED`, field
`[3]` @ `+0x2E48` → `0x80800BD7`, 40 × `0xD8` records): `FUN_007775B0` copies to `sstm+0x490` iff
`record.region_id (+0x00) == sstm+0x138` (PENDING region) AND
`record.peer_tokens[+0x38 + local_peer_index] == sstm+0x1D0` (token). We had been sending 8,672
bytes of zeros. Knobs: **`D1_AH_SLICE_TOKEN=1`** (correct for EVERY poke - `sstm+0x1D0` resets to 0
on fallback), **`D1_AH_SLICE_REGION=0`**, **`D1_AH_SLICE_PEER=0`**; slot 1's region id is written
`-1` on purpose (terminator - otherwise 39 zeroed duplicates overwrite the good record, last writer
wins). A membership sent once at join is re-scanned at every transition (`FUN_0077E5A0` resets
`sstm+0x3B0` to `-1`); no republish or `MEM-%d` bump needed.

⛔ The bug that hid this for a session: the AH's `RequestStateRefresh` handler rebuilt the
membership from `solo()` (a pure echo) and never re-applied `set_pah_region` - the client's refresh
0.37 s after join overwrote the good record. Fixed + controlled (77 AH controls, inversion
reproduces the live `(0,0,0)` bytes). **Rule: when a re-publish path rebuilds an object from an
ECHO, every field that is ours to assert must be re-applied there - enumerate, don't spot-check.**
Cleared live: `Region 'PRV00.00 (HASH: 0x65bdc6f8)' is being assigned the slice-set-switch task.`,
done `0x07F → 0x17F`.

### 4.2 Task 2 - `error.sstm_slice_not_found` ("lobster"): the delete-this value `[C]`

> Verdict 2026-08-09 (⇒ historicaldocs/design/slice-set-residency.md): not residency, not a missing
> field. `global_activity_state+0x14` published as all `0xFF` makes rung 33's registrar
> **UNREGISTER every bubble's region**; rung 34 then asks for that region and is told it does not
> exist.

* Emitter `FUN_007812D0` (`slice_set_transition_manager` polling update); lookup `FUN_00344A50`
  over the region registry (`container &DAT_0267AF20`, `registry = *(container+8)`): `+0x10` holds
  512 × `0x10`-byte region slots (`+0x00` handle, `-1` = UNSET), `+0x2014` 64 per-bubble content
  tags, `+0x2114` current id / `+0x2118` loading id, `+0x211C/+0x215C/+0x219C` three 512-bit
  bitsets (resident / failed-load / failed-instantiate). Return 5 = "cannot be found"; `0xd` =
  `error:slice_set_not_found` → KTOBUCKET renders `error.sstm_slice_not_found`.
* `('PENDING')` in the error line is a **slot label** (`DAT_0192D250`: `CURRENT / PENDING / unknown
  / not loaded`, indexed by the emitter's slot argument), not a region state. `PRV00.00` is
  `sprintf("%03s%02d.%02d", "PRV"|"PUB", id, id)` of the integer region id - composed at runtime,
  not a content name.
* Region id = `bubble_index * 8` (`FUN_0012D6A0`); inverse `(id >> 3) & 0x3F` (`FUN_0012D6E0`).
* The sole slot writer is `FUN_00343020`: `param_3 != -1` REGISTERS (returning
  `*(registry+0x2014+bubble*4) != -1`); `param_3 == -1` UNREGISTERS. Its registrar callers:
  `FUN_002A0320` (rung 33's bubble gate - chosen `0x00` registers when the scenario's sub-bubble
  flag is set, `0xFF` unregisters) and `FUN_007528B0` (orbit-only, via `orbit_map_loader`).
  **`0xFF` does not mean "ignore"; it means "erase this bubble's region."**
* Live positive control 16/16 on orbit (registered set `{0-4, 9-14}` predicted from the decompile
  matches the live registry; inverted wrong-base control agrees only 11/16, so the control can
  fail). `orbit` and `pvp_moonbase` are one mechanism; moonbase's `array[0]` equalling its name hash
  (`0x65BDC6F8`) is authoring coincidence, resolved `[C]`.
* Fix shipped: `D1_AH_BUBBLE_CHOSEN` (§3.2), 65/65 controls, inverted before shipping.
* `[U]` (non-blocking): whether a given map's `registry+0x2014` entry 0 is populated
  (`FUN_00343020`'s register path returns 0 silently if not; `[I]` strongly yes - the printed hash
  followed the world).

### 4.3 Tasks 7 / 9 / 10 - the fly-in cinematic cluster `[C]`

One cluster: task 7 `FUN_00794A60` = prologue-filler ENTRY playback, task 9 `FUN_00794D30` = the
hold/wait, task 10 `FUN_00794E70` = EXIT playback. **Task 10 is free** (same suppress fast path as
task 7, returns 2 immediately; only ever waited on task 9's bit, prereq `0x290`).

* **Task 7's 150-tick timeout is NOT a safety net** (measured `handler+0x1B0 = 0`): `0x96` is
  written only after playback starts; on the cannot-start path task 7 returns 1 forever, silently.
* **The suppress lever:** task 7's first statement is `if (FUN_00765070()) return 2;`.
  `FUN_00765070` = `FUN_0078af50() || (byte 0 of [0x26A33E8] pointee == 1)` - lever A is ONE BYTE of
  the activity-selection record (`0x130` B, consumed by rung 27); lever B compares the orbit session
  parameter's `i_orbit_name_hash` against `DAT_026b64a8`. Confirmed by execution offline
  (`d1-replay`, AL `0x00 → 0x01`, 108 steps both ways) then live: poking `[0x26A33E8]` byte 0 = 1
  cleared task 7 (done `0x17F → 0x1FF`, task 9 STARTED for the first time in project history, run
  `20260809-225119-UNION8-MOONBASE`). ⛔ The poke does NOT survive a teardown (byte reads `00` after
  fallback; re-arm per attempt). `[U]` whether D1's user-facing "skip fly-in animation" setting
  backs `FUN_00765070` (would make it a served profile value, not a poke); `[U]` whether
  `DAT_018C4AC8` (global cinematics enable) is a shipped user setting.
* Alternative content route: task 7 also returns 2 when `desc.f18 == -1` (no prologue filler
  configured), keyed on the activity name we publish; per-map probe is one read of `[0x26B1418]`
  deref `+0x80` (`0xFFFFFFFF` ⇒ done; positive control: `pvp_moonbase` must read `0x55A13D91`).
* **Task 9 is the only cinematic task with NO suppress fast path** - suppression only skips the
  playback tick; the content gate runs regardless. Its two survivors are pure content-DB lookups
  through package registry `DAT_01A0B420`: (1) `FUN_001192A0(...)` non-NULL and `[result+8] != 0`;
  (2) `FUN_0010C500(&out)` true with `out != 0` (reads the byte at `+0x1AC`, rejecting `-1`).
  **These are the same two functions the player-spawn gate calls** - see §5 and
  `design/activity-lifetime-state.md` (the live frontier doc for the lifetime/phase record; the
  phase byte `+0x1AC` ∈ {3,6} is a wire field, AH message type 7 `sensor_message`).
* Tooling unlock `[C]`: nearly every function in this binary has a stack canary (`*DAT_01A06C78`
  via `gs:`), which is why so much was "unreplayable" -
  `d1-replay replay <snap> --func 0x10c500 --arg scratch --set-mem 'v:0x1a06c78=0000000008000000'`
  opens the class. Capture snapshots with a wide content net (the walk reaches `0x22DB...`,
  `0x2044...`, `0x38031...`).

Task 5 (`FUN_00794660`, "Waiting for managed-session-start for all peers.") fired once and is not a
blocker. Task 8's `[U]` on which instruction sets `[0x26B1438]+2` bit 7 remains (a one-form byte
scan found 15 sites, none in `0x76`-`0x78` - not evidence of absence; `d1-live watch` closes it).

---

## 5. Arrival / spawn handoff findings

⇒ historicaldocs/design/arrival-handoff.md. `d1-eboot controls` ALL PASS for every static claim.

### 5.1 Hypothesis C is REFUTED `[C]` - forcing the rung-34 done mask skipped nothing

Five independent proofs, each with a passing control:

1. The done mask has exactly ONE consumer: `if (done == requested) FUN_00782720(0x23,0,0)` - the
   advance to rung 35. Forcing it means "advance now" and nothing else.
2. Task 9 has NO side effects on any path (its only non-read call is inside the not-suppressed
   branch). Forcing bit 9 is a perfect substitute.
3. Rung 35's LEAVE (`0x007963C0`, proven to be LEAVE = vtable slot 5 by the state manager's own
   printfs) begins with three unconditional statements - `FUN_007648D0(0)` (phase 0),
   `FUN_00764950()` (phase 0, handle -1, stop the sound), `FUN_00765000(0x3f)` (sim-queue type
   `0x38` broadcast stop-all) - tearing the cinematic subsystem down identically whether tasks
   7/9/10 played or were skipped. A skip cannot strand a bracket that was never opened.
4. Camera work has its OWN sim-queue element type:
   `c_simulation_queue_cinematic_camera_emote_action_definition` = `0x35`; the exit path pushes
   `0x39` (`cinematic_start_stop_definition`). Nothing on this path enqueues the camera element.
   (Full recovered set: `0x35` camera emote · `0x37` object · `0x38` cache gear · `0x39` start/stop
   · `0x3A` batch request.)
5. The `0x51` branch of the `0x39` handler is inert unless a type-`0x3A` batch set
   `[0x26B1418]+0x94`, and that batch is issued by task 6 - also suppressed. Task 10's
   non-suppressed body would have done nothing even if it had run.

Corollaries: `FUN_007646B0`'s terminal act is to START a cinematic (no player, no biped, no camera,
no input); `FUN_007648D0` is a general UI/audio state broadcast whose written field
(`[0x26B10A0]+0x23C`) has exactly one reader, `c_video_system` (`0xAAF440`), and whose broadcast
half is `[I]` a Wwise SetState. **Do not spend a boot avoiding the mask force**; a "minimal-forcing
run" is likely an indefinite silent stall (task 9 has no timeout and no error path, and its
"Holding" strings print only under a dev switch, `DAT_026B6D50`).

Falsifier, stated plainly: the verdict rests on rung 35's LEAVE actually running; one live read of
`[[0x26B10A0]+0x23C]` at the black screen tests it. `0` ⇒ REFUTED stands; `10`/`11`/`12` ⇒ reopen.

### 5.2 What replaced it - and the §6.3 retraction

The original replacement claim ("task 9 and the spawn gate read two ADJACENT BYTES of one record")
is **RETRACTED** (banner, 2026-08-10 late): they read **TWO DIFFERENT RECORDS**.

* **Leg A** `FUN_001192A0` - an ENTITY-keyed sub-block. Task 9 passes
  `~schema_0x80000000_303130()`; the spawn gate passes the player ENTITY handle. Leg A is ANSWERED:
  that schema fn is `c_spasaha_engine_interface::vfunc[39]` and returns `-1` iff
  `[0x27257F0]+0xC0 == -1` - the controller→player map. **Task 9's leg A and the missing biped are
  ONE root cause.**
* **Leg B** `FUN_0010C500` - the lifetime state at `+0x1AC` of a global client-bundle instance, a
  different record and a SECOND wall behind the first. The spawn gate (`FUN_00414730`, called at
  `0x00411E0F` from the per-tick player-spawning update `FUN_00411830`) requires the state in
  `{3,6}` (`(0x48 >> state) & 1`), refusing on `{0,1,2,4,5,7,8,9}`. Measured `0xFF` = not found
  (`replay-oracle` §9.6, at orbit, `synthetic_ancestor`). ⇒ **the live record/phase work is
  `design/activity-lifetime-state.md`** - do not duplicate it here.

The spawn gate is NOT cinematic-gated `[C]` (complete callee set contains no cinematic or
world-controller function; controlled vocabulary scan passes in both directions). The biped is
created client-side at `0x00412BDF` (`call 0x32D180`); possession and camera binding are ONE action
(`FUN_0090FFC0` writes `player+0x644` and reaches the only writer of `observer+0xC4`,
`[0x27257F0]+0xC4`); a dead camera on a black screen is the signature of `player+0x644 == -1`.
`FUN_003D5310()` must be FALSE (`activity_def[+0x1C]` / `[+0x5079]`, two static "no player
spawning" flags - expected 0 for activity 407, cheap to read).

⛔⛔ **The refusal diagnostic is structurally UNOBSERVABLE - do not grep for it.** `"can't spawn due
to the current activity lifetime state ('%s')"` is `vsnprintf`'d (`FUN_010548E0`) into a stack
buffer that is never read; 0 hits in every run is a structural false negative (the string IS in the
image at file offset `0x1625926` - the grep was well-formed). Third variant of the two-gate trap:
no sink call at all. Instruments instead: `d1guest` read `players[0]+0x644` and `[0x27257F0]+0xC4`
(players base `*(long*)(0x2725700+8)`, stride `0x2AF0`, `--abs`); `players[0]+0x1660` (biped
definition) and `+0x0C` bit 0 (spawn requested); `d1-live watch --rw exec --addr 0x41480B`
(reached only on refusal; read `EDI` at `0x4147C7` for the state number).

---

## 6. Orbit as a viewer

⇒ historicaldocs/design/orbit-as-a-viewer.md (Part 1 superseded by Parts 2-3, measured live) and
historicaldocs/design/orbit-bubble-namespace.md.

### 6.1 The mechanism: backdrop vs dispatch

The orbit BACKDROP is `orbit_map_loader` (`FUN_00752380`) driving **queue slot 2** (slice-set
switch) off a stored destination; **slot 3** (change world) is the activity-dispatch path (what
rung 33 uses). The director is NOT involved - `FUN_00752AE0` reads `last_orbited_destination` from
the account record; the director is only the UI that writes it. D1 separates *which world's
geometry is loaded* (a string, slot 3) from *which region inside it is instantiated* (an integer,
slot 2). Slot 2's command is 12 bytes `{u32 0xC, u32 region_id, u32 token}`; dispatch = write the
command into `registry + 0x200 + slot*0x1C04`, then `registry + 0x1B4 + slot*4 = 2`
(`capture-rig/queue-command.py`). There is also a teleport path: `FUN_00779BA0(sstm, id, &token,
flag)` → `sstm+0x3A4/0x3A8/0x3AC`, reached via `FUN_0077A1C0`; callers `0x3F5860` / `0x41BAC0`
`[U]`. This is NOT a debug-only table - slot 3 fires in ordinary play.

### 6.2 Orbit's 16 bubbles, read live `[C]`/`[O]`

Scenario tag `0x80B52003`; slice-set table record tag `0x80B5202C` (a DIFFERENT, thicker record:
count `+0x28`, array `+0x30`, hash at `+0x40 + i*0xA0`, stride `0xA0`). Region id = bubble × 8.

| bubble | hash | name | valid | | bubble | hash | name | valid |
|---:|---|---|:-:|---|---:|---|---|:-:|
| 0 | `0x8265820B` | `[U]` | 1 | | 9 | `0xDDF44209` | `orbit_mars` | 1 |
| 1 | `0x132E9BF9` | `[U]` | 1 | | 10 | `0xD959DDED` | `orbit_mercury` | 1 |
| 2 | `0x11E694B6` | `[U]` | 1 | | 11 | `0xE303548D` | `orbit_moon` | 1 |
| **3** | `0x3559729A` | **`orbit_earth`** (the live one) | 1 | | 12 | `0x19927D90` | `orbit_reef` | 1 |
| 4 | `0xF5B4ABB3` | `[U]` | 1 | | 13 | `0xC10708C7` | `orbit_saturn` | 1 |
| 5-8 | `0xF5B4ABB0/B1/B6/B7` | `[U]` | 0 | | 14 | `0xD244A995` | `orbit_venus` | 1 |
| | | | | | 15 | `0xD51C4771` | `[U]` | 0 |

Bubbles 4-8 share prefix `0xF5B4AB`; the five final characters lie in one aligned 8-value block
(`'0'`-`'7'` if digits) - that constraint is all that survives (see Dead claims #17). **The name
must be LOOKED UP, not cracked** (32-bit FNV is saturated; meet-in-the-middle returns perfect
nonsense). Next place to look: the 156 unexamined bytes per `0x80B5202C` entry, a live read `[R]`.

D2 corroboration `[C]`/`[R]`: same `orbit_map_loader` format string, same `0x811C9DC5` sentinel,
region ids all multiples of 8, same FNV-1, same `ambient_*` backdrop-world convention; D1's
`orbit_earth` and D2's are the same name and hash. ⚠ D1 has no `slice_set_loader` subsystem (that
name is D2-only); D1 uses `orbit_map_loader` + `slice_set_transition_manager`. D1 slice-set regions
have no symbol-table names (`plaza`/`orbit_earth` absent from all 1,114 package symbols, four
positive controls resolving).

### 6.3 The selector, and the wire-side lever

`FUN_00752A10` picks orbit's slice set by `slice_set_target` (`orbit_map_loader + 0x104`): target 0
→ `DAT_026B07A4`, target 1 → `DAT_026B07A0`, **target 2 → `last_orbited_destination`** (account
state) - our client takes target 2 `[O]`. **`last_orbited_destination` is family-4/type-1 profile
`+0x28`, already served: `D1_ORBIT_DESTINATION`** (`d1-responder`, default `2468353467` =
`0x932015BB` = `DESTINATION_OLD_RUSSIA`) - a **rank-1 wire lever**, vs the rank-3 pokes below.
⚠ The transform is `[U]`: the value is a destination id in a different namespace, mapped to a
slice-set hash by `FUN_00752A10`/`FUN_007AC6C0`; hypothesis `[I]`: destination → parent planet's
orbit bubble (we ship Old Russia, client shows `orbit_earth`). Falsifiable in one boot with a Moon
destination - and driving the choice at the loader level may also precache (the residency blocker
below).

### 6.4 Live campaign results `[C]` (rank-3 pokes, `synthetic_ancestor` twice over)

* Slot-2 dispatch works: `{0x0C, 88, 0x811C9DC5}` → `world_controller: Failed slice-set switch to
  'HASH(0xe303548d)'` - the client resolved id 88 → bubble 11 → `orbit_moon`, confirming the
  layout. It FAILED on **residency**: only the orbited region's bit is set in `+0x211C`;
  `0x752380` checks the precache, it does not do it. The D2 tool bounces the state machine
  (re-enter `bootflow:prepare_for_orbit`) so the loader precaches, then switches.
* **The backdrop changes on command**: one `u32` into `last_orbited_destination`
  (`[0x26A3790] + 0x4C0`) plus a ladder cycle (`session-params.py --launch-poke`) - Earth
  (`0x59AD083E`, slice 24) → Mercury (`0x1002D408`, 80) → Moon (`0xBA7B60B2`, 88), reversible, no
  crash. Precache is `FUN_003436F0` ("slice set precache"; `+0x2118` = LOADING id, `+0x2114` =
  CURRENT id).
* The 46-destination table (`capture-rig/orbit-destinations.tsv`, `{u32 dest_hash, u32
  slice_hash}` stride 8, two independent copies agreeing 46/46): Earth 13 · Mars 11 · Venus 7 ·
  Mercury 5 · Moon 4 · Reef 3 · ?bubble4 3 · **Saturn 0** - D1 ships no Crucible map on the
  Dreadnaught, so every Saturn attempt failed at resolution. Most promising untried route: enter
  rung 22 with `0x26B07A0 = <slice hash>` (loader22 runs target 1).
* Lessons paid for: any scan of a structure you have poked must compare two independent copies
  (P3.5's self-contamination); poke the input to a resolution, never a machine's own state flag
  (writing `sstm+0x88 = 1` killed the client); never read a multi-byte constant by eye (the
  `0x80192F410` vs `0x800192F410` vptr bug, caught only by the positive control also failing).

---

## 7. Boot-to-x: the static shortcuts

⇒ historicaldocs/design/boot-to-x.md. All `static-client-inference` on
`dumps/patch-01.29-CUSA00219/eboot.bin` unless tagged.

**There is no config-driven "boot directly to state N" in this build** `[C]`, with passing controls
on both halves: all 87 `request_state` sites (25 of them E9 tail-calls; census
`capture-rig/state-request-census.py`, 6/6 controls) use immediates, and only 6 of 50
transition-owning functions read engine config at all - all six for a timeout that aborts to
`cleanup`. The entire server-settable configuration surface is ONE `0x3298`-byte block at
`*(void**)0x267B1B8` ("Engine Configuration", 1129 live keys of 1155, holes 103, 248, 525-531,
1236-1252; `capture-rig/engine-config-keys.py`, table
`capture-rig/schemas/engine-config-keys.tsv`). Keys are anonymous (no per-variable name table; a
string sweep for `boot_to`/`skip_bootflow`/`quickstart`/`dev_mode`/`autologin` returns zero).
⚠ Never send a HOLE key (aborts the whole signon parse); the delta RESETS the block first, so it is
a full snapshot, not a patch.

**Per-state timeouts** are the one config mechanism that moves the machine (keys 984-1019 →
`+0x30F0..+0x3180`, u32 ms, 0 = disabled; only states 28/29/30 have non-zero defaults 90 s / 600 s
/ 180 s). ⛔ A timeout is not a hop: reasons `0x10`/`0x11` are error-class ⇒ `cleanup` ⇒
`kick_to_bootflow_start`. Useful only as a watchdog for unattended runs.

**The skip that matters is one byte** `[C]` static / `[U]` live:
`bootflow:investment_signin`'s update (`0x79BFF0`, the only requester of state 21) branches on
`0x57E490()` = `*(uint8*)(*(void**)(0x26A3790) + 0x18) != 0`. Non-zero ⇒ straight to state 22
`prepare_for_orbit`, and `bootflow:profile_setup` (screen bounds / brightness / subtitles - the
brightness slider that stalls unattended runs) never runs. The wizard (`0x79E830`, substep
`handler+0x38`) advances only on UI confirm events; the operator's observed `b` behavior is the
back edge `3 → 0`. Runtime write **c1**: `*(uint8*)(*(void**)0x26A3790 + 0x18) = 1` - the mildest
available taint (sole consumer is the branch, both outcomes converge on state 22), still
category-(c) `synthetic_ancestor`; record it in the run manifest. The prize (a5) is setting the
byte from the wire via the investment profile object - blocked on `[U]` §8 (blob provenance).
Category (b) (file/command line) is EMPTY - the signon response is the only Engine Configuration
writer, and `sku_config.txt` is RSA-signed.

⚠ Unresolved base-address discrepancy, both kept: `historicaldocs/design/signon-protocol.md` §4.5 /
`content-manifest-gate.md` §2.1 treat `0x263D2E8` as an object base while every call site passes
`0x263D0F8` (the `c_signon_manager` singleton) as `this`; the consistent reading `[I]` is one
singleton at `0x263D0F8` with an embedded status struct at `+0x1F0` (`0x263D2E8` = signon state,
3 = success; `0x263D340` = sub-status). One live dump of `0x263D0F8..0x263D600` settles it.

---

## 8. Open [U]s

| # | `[U]` | instrument / experiment |
|---|---|---|
| 1 | Who writes `cleanup + 0x6C8` (the stored goal state) | `field-xref` on the cleanup handler class; or `d1-live watch` |
| 2 | Which 7 named tasks rung 28 waits on | function pointers in the table at `this+0x38`, readable live |
| 3 | Session-parameter offsets; layouts of `s_activity_host_connection_information`, `s_network_session_world_controller_goal` | static struct work + live reads at rungs 28-30 |
| 4 | The `world_launch` activity-slot hash space; `+0x08`; the `0x00000080` entries; whether a world name alone suffices | capture a specimen with a real activity (`orbit`); the diagnostics discriminate by name |
| 5 | Scenario handle CURRENT vs TARGET at check time (decides a PvP map's checksum source) | `scenario-checksum.py watch --hz 50` during one poke - also yields that map's `D1_AH_ACTIVITY_CHECKSUM` |
| 6 | Whether any world's bubble count exceeds 64, or needs a bubble pre-chosen where all-`0xFF` fails | `scenario-checksum.py now`/`watch` during the ~2 s residency |
| 7 | Whether a PvP map's `registry+0x2014` entry 0 is populated (silent `FUN_00343020` failure) | the run itself discriminates (rung 33 failing with no message); `watch --hz 500` |
| 8 | Which instruction sets `[0x26B1438]+2` bit 7 | `d1-live watch` hardware watchpoint during a rung-34 run |
| 9 | Does the `[0x26A33E8]` suppress poke get rewritten per frame within an attempt; does the "skip fly-in animation" user setting back `FUN_00765070`; is `DAT_018C4AC8` profile-backed | `d1-eboot field-xref`/`xref` on `0x18C4AC8`; profile diff |
| 10 | Would `FUN_0010C500`/`FUN_001192A0` pass on `pvp_moonbase` mid-activity (measured `0xFF` at orbit only) | offline PC-1: `d1-replay` with the canary redirect against an in-activity snapshot ⇒ `design/activity-lifetime-state.md` |
| 11 | The writer of the `+0x1AC` lifetime byte | `d1-live watch --abs <addr> --len 1 --rw write`, address computed as `FUN_0010C430` does ⇒ `design/activity-lifetime-state.md` |
| 12 | The phase enum `[0x26B10A0]+0x23C` at the black screen (the §5.1 falsifier) | one `d1guest` read |
| 13 | `FUN_00752A10`/`FUN_007AC6C0`: destination id → bubble transform; precache-via-destination (O1/O2) | vary `D1_ORBIT_DESTINATION` or poke profile `+0x28`; watch the residency bitset |
| 14 | Names of orbit bubbles 0,1,2,4-8,15 | dump `0x80B5202C`'s 156 bytes/entry live; LOOKUP, never a preimage crack |
| 15 | Teleport callers `0x3F5860` / `0x41BAC0`; whether `slice_set_target` (`loader+0x104`) is settable; which table copy the resolver reads | decompiled corpus; `d1-live watch` on `handler+0x178` and the table |
| 16 | Profile blob provenance (`profile_mgr[+0x10]`) - unblocks the wire-side profile-setup skip (a5) | read `*(void**)0x26A3790` and `+0x18/+0x22/+0x24/+0x25` live at `bootflow:investment_signin` |
| 17 | 32 vs 33 PvP maps (§2.2) | reconcile the package-inventory stems against `package-container-pvp-content.md` §7.5 |
| 18 | `insp = -1` sentinel: would poking `ac+0x148` to 0 change the launch | one poke before the transition, re-read the KTOBUCKET block |
| 19 | `FUN_00341960`'s strict half (`opts+3 & 1` from command `+0x6DE`); whether `FUN_00065030` null-guards a NULL dst | one `d1guest` read of the slot-3 command; non-blocking |
| 20 | Signon singleton base `0x263D0F8` vs `0x263D2E8` (§7) | one live dump `0x263D0F8..0x263D600` |

---

## 9. Dead claims

Each line: the killed claim, and what killed it. Banners override bodies; none of these may be
carried forward as live.

1. "`+0x6A0`/`+0x6B0` (cleanup) and `+0x118`/`+0x128` (rung 28) are a task-queue head/tail pointer pair" - killed three ways: the literal `0x7F` write, XOR arithmetic in the client's own pending/active log, and seven runner instantiations agreeing on `8 + N*0x20` mask offsets (historicaldocs/design/activity-launch-ladder.md §1.3).
2. "Rung 28's requester is one of `0x797420`/`0x797530`/`0x7A3750`" - killed: it is `0x7AB460` inside orbit's own update `0x7AAF60` (tail-merged with a rung-25 site); `0x797420` is bounded to states 13/17/18 (ladder §2.2).
3. THE-CHAIN §5 / ACTIVITY-LAUNCH-PLAN §2's "requester OF rung N" labeling - killed: the site is the handler of rung N-1 (ladder §2.2).
4. "The activity checksum is per-activity; `activity_def+0xA4` is the value" - killed live: `D1_AH_ACTIVITY_CHECKSUM=0x0261283F` (407's `+0xA4`) still mismatched, and the comparator never reads the activity definition; it is per-WORLD (scenario `+0x0C`/`+0x10`) and per-match-type (historicaldocs/design/activity-checksum.md §0, §5).
5. "A checksum mismatch is harmless / not on the status-6 path" (rungs-31-36 preflight §1.5) - true for rung 30 only; killed at rung 33 by the observed `transition_error` abort (historicaldocs/design/activity-and-map-namespace.md §3).
6. "(a) the change-world command carried an EMPTY world name" (rung33-world-transition §4c.1) - killed by line numbers: the empty `successfully changed world to:` is a boot-time event 40k lines earlier; all five transitions printed real names (historicaldocs/design/activity-loader-writer.md §5).
7. "`c_activity_loader`'s writer is unknown / the loader is a gate" - killed: `FUN_00341500 @ 0x341798` provably promoted (pre-clear at `0x7539e4` + `FUN_00340240` TRUE at `0x34cb87` + writer enumeration); the failure tail erases the name afterward (historicaldocs/design/activity-loader-writer.md).
8. "There is a third entry path into `0x753540` that a call/jmp/riprel/rela scan cannot find" - killed: the riprel at `0x752F47` is the slot-3 registration; the caller is the queue pump at `0x34C0BC` (historicaldocs/design/world-launch-command.md §4).
9. "The rung-34 slice error is the `orbit` CONFOUND, not a new mechanism" (aug10-startup-context) - killed: all-`0xFF` bubble state UNREGISTERS the region, world-independently; the moonbase run demonstrated it (historicaldocs/design/slice-set-residency.md).
10. "`('PENDING')` is a region state to chase" - killed: `DAT_0192D250` is a slot-label table indexed by the emitter's slot argument (historicaldocs/design/slice-set-residency.md §2).
11. "The slice token is `sstm[0x1D0]+1` per transition, so a second poke needs 2" - killed twice: `token advanced to 001` on both transitions, and `sstm+0x1D0` resets to 0 on fallback; `D1_AH_SLICE_TOKEN=1` is correct for every poke (historicaldocs/design/rung34-task7.md banner + §5).
12. "The record reached the client / the join-path receipt proves delivery" - killed by a live read: the AH's `RequestStateRefresh` echo-rebuild overwrote the PAH record 0.37 s after join, last writer wins (historicaldocs/design/rung34-task7.md).
13. "`handler+0x1A0` is `requested`; tasks 8/9/10 are not requested yet" - killed: `requested` is `+0x198` = `0x7FF` (assigned by `vfunc[4]`); `+0x1A0` is `started`; task 8 ran every tick (historicaldocs/design/rung34-task7.md corrections).
14. "Task 7's 150-tick timeout is a non-fatal safety net" (preflight §1.6) - killed, measured: `handler+0x1B0 = 0`; the timer arms only after playback starts, so the cannot-start path hangs silently forever (historicaldocs/design/rung34-task7.md §3).
15. "Task 7 is the EMPTY PROFILE / a character-keyed lookup" (and the first correction "family 4/type 2") - killed: the character byte is dead code on the `param==2` path, the iterator is `c_investment_roster_game_system` and the walk SUCCEEDS; the miss is a served family-0/type-0 object (historicaldocs/design/rung34-task7.md banner).
16. "Hypothesis C: forcing the rung-34 done mask skipped an arrival handoff that would have spawned the player" - killed five ways: single mask consumer; task 9 side-effect-free; rung-35 LEAVE unconditional teardown; the camera is element `0x35`, not `0x39`; the `0x51` branch is inert without a task-6 batch (historicaldocs/design/arrival-handoff.md).
17. "Task 9 and the spawn gate read two ADJACENT BYTES (`[+8]`/`[+9]`) of ONE record" (arrival-handoff §6.3) - RETRACTED: two different records - leg A is the entity-keyed controller→player map (`[0x27257F0]+0xC0`), leg B the lifetime byte `+0x1AC` of a global client-bundle instance ⇒ `design/activity-lifetime-state.md` (historicaldocs/design/arrival-handoff.md top banner).
18. "We took the suppressed path for task 10" (biped-spawn-watch §7 framing) - killed: bit 10 was forced before the pump reached it, so `FUN_00794E70` never ran in either branch (exit string `0x164C739` NOT SEEN, `strong`) (historicaldocs/design/arrival-handoff.md §3.1).
19. "Cite `diagnostic-catalogue.tsv` for seen/n_runs" - killed: the checked-in TSV was stale (148 `strong` rows flipped 0 → 1 on regeneration); the run-dependent columns must come from the live CLI, now guarded by a provenance stamp and `--check-stale` (historicaldocs/design/arrival-handoff.md §3.2).
20. "`plaza` cannot exist because D1 slice sets are unnamed integers" (orbit-as-a-viewer Part 1) - half killed live: slice sets ARE FNV-1-named (bubble 3 = `orbit_earth` = `0x3559729A`, byte-identical to D2's), merely absent from package symbol tables; `plaza` itself is still not a D1 name (historicaldocs/design/orbit-as-a-viewer.md Part 2).
21. "Slot 3 is the orbit-backdrop lever" (Part 1 framing) - killed: the backdrop is `orbit_map_loader` driving slot 2 off `last_orbited_destination`; slot 3 is the activity-dispatch path (historicaldocs/design/orbit-as-a-viewer.md Part 2 banner).
22. "`+0x2118` current / `+0x2114` loading" (Part 2) - killed: swapped; `+0x2118` is the LOADING id, `+0x2114` the CURRENT id (historicaldocs/design/orbit-as-a-viewer.md P3.2).
23. "Saturn present: True - 2 destinations" - killed: both rows were the session's own earlier pokes read back out of an edited table copy; caught only by the two-copy agreement control (historicaldocs/design/orbit-as-a-viewer.md P3.5).
24. "Bubbles 4-8's final characters are `'1','2','3','4','5'`, self-validating" - killed arithmetically: pairwise deltas all < 8, so eight character assignments are equally self-consistent (including another consecutive run at `'6'`); the other session's `'3','0','1','6','7'` is equally unestablished; only the aligned 8-value block survives (historicaldocs/design/orbit-bubble-namespace.md §2).
25. "`0x79EC20` is `bootflow:profile_setup`'s update, steered by a config object at `0x263D0F8`" (character-and-orbit §26.79) - killed by RTTI: `0x79EC20` belongs to `...bootflow_queuing_handler`; profile_setup's update is `0x79E830`; `0x263D0F8` is the `c_signon_manager` singleton and `0x2A8570`/`0x2A85B0` are constant-folded getters on it, not cvar accessors (historicaldocs/design/boot-to-x.md §0.1-§0.2).
26. "Key 1254 is a setter-switch hole" (content-manifest-gate §3.3) - killed: its jump-table arm `0x107A4E2` is live; the hole list is 26 entries exactly (historicaldocs/design/boot-to-x.md §0.3).
27. "`state-machine.py`'s slot 2 (`+0x10`) is ENTER and slot 8 (`+0x40`) is EXIT, per-transition" - settled: the per-transition pair is vtable slots 4/5 (ENTER/LEAVE), proven by the state manager's own printfs; slot 2 is the construction/registration hook (historicaldocs/design/arrival-handoff.md §1.6).
28. "THE-BLOCKS: the simulation queue has 115 element types" - killed: `DAT_026708C8` is allocated `0x2F8` bytes = 95 slots, highest index used `0x5E` = 94 (historicaldocs/design/arrival-handoff.md §8; THE-BLOCKS carries the banner).

---

## 10. Sources

All twelve melt into this document; originals move to `historicaldocs/design/` unchanged. Cite them
for full derivations, controls, and instrument transcripts.

| source (historicaldocs/design/) | contributed |
|---|---|
| `activity-launch-ladder.md` | §1 - the 35-handler machine, transition primitives, task bitmasks, session-parameter gate, command-queue negative |
| `activity-and-map-namespace.md` | §2 - MODE/MAP split, 393-413 table, patrol verdict, PvP stems; superseded §3 folded into Dead claims #4-#5 |
| `activity-checksum.md` | §3.1-§3.2 - comparator, scenario record, PRIVATE index, `D1_AH_ACTIVITY_CHECKSUM` (confirmed live), `D1_AH_BUBBLE_COUNT` |
| `activity-loader-writer.md` | §3.3 - the loader block, the promotion at `0x341798`, "not a gate", the log-channel control |
| `world-launch-command.md` | §1.5, §3.4 - the captured slot-3 command, the queue dispatch answer |
| `slice-set-residency.md` | §4.2 - lobster, the region registry, `0xFF` = erase, `D1_AH_BUBBLE_CHOSEN` |
| `ktobucket-levers.md` | §3.5 - the eleven formats, the six levers at `ac+0x138`, the two reader traps |
| `rung34-task7.md` | §3.6, §4 - task pump, task 8 PAH fix, tasks 7/9/10, suppress lever, ten-flag union, olive |
| `arrival-handoff.md` | §5 - Hypothesis C refutation, §6.3 retraction, spawn gate, unobservable diagnostic, ENTER/LEAVE slots |
| `orbit-as-a-viewer.md` | §6 - slots 2/3 as viewer, bubble table, destinations, Saturn, contamination lesson |
| `orbit-bubble-namespace.md` | §6.2-§6.3 - the selector, `D1_ORBIT_DESTINATION`, the bubbles 4-8 retraction |
| `boot-to-x.md` | §7 - no boot-to-x, engine-config key space, per-state timeouts, the one-byte profile-setup skip |

Live sibling docs (do NOT duplicate; reference): `design/activity-lifetime-state.md` (the
lifetime/phase record - CURRENT FRONTIER), `design/GUARDIAN-IN-WORLD.md` (the spine),
`design/activity-host.md` (AH protocol/messages), `design/player-in-world.md` (membership,
`+0x5028`), `design/architecture.md`, `design/content-and-packages.md`,
`design/grants-and-unlocks.md` (the 503/205 flag bodies).
