# Player in world - how a player record and a biped come to exist, and the instruments that proved it

**State, 2026-08-11.** A type-9 biped stands in `pvp_moonbase` (activity 407) - see
`design/GUARDIAN-IN-WORLD.md`, the live spine, for the current recipe and queue. This document is the
canonical reference for the spawn chain itself, the two walls that fell (the membership fix and the
one root cause), the biped census instrument, and the watch plan. Possession/control remains `[U]`.

Consolidates four historical docs; provenance is cited per section. **Banners override bodies:**
the census document is retracted in part (the census stands, its diagnosis does not) and every
killed claim is listed in "Dead claims" at the end.

## Contents

1. [The spawn chain (reference)](#1-the-spawn-chain-reference)
2. [The walls that fell and how](#2-the-walls-that-fell-and-how)
3. [The census instrument and findings](#3-the-census-instrument-and-findings)
4. [The watch plan and what remains](#4-the-watch-plan-and-what-remains)
5. [Dead claims](#5-dead-claims)
6. [Sources](#6-sources)

---

## 1. The spawn chain (reference)

Reach: `static-client-inference`, from the pinned 01.29 eboot (`d1-eboot controls` 27/27 PASS) and
the decompiled corpus. Provenance: `historicaldocs/design/biped-spawn-watch.md` §2.

### 1.1 The whole chain in one function - `FUN_0052BB10` `[C]`

`FUN_0052BB10` is the crash-report field writer for `player_position_x/y/z`. It answers "is there a
local player, and does it have a body in the world", printing the literal string `unknown` when the
answer is no. Its chain, disassembled (`d1-eboot disasm 0x52bb10 --to 0x52bc3f`):

```asm
0x0052bb1c  call   0x905170                     ; gate: "is the game running"
0x0052bb23  je     0x52bb7a                     ;   false -> "player_position_x: unknown"
0x0052bb25  call   0x921af0                     ; is controller 0 mapped?  -1 = no
0x0052bb31  call   0x921a10                     ; -> u16 LOCAL PLAYER INDEX
0x0052bb36  lea    rcx, [0x2725700]             ; the "players" block POINTER
0x0052bb3d  mov    rcx, [rcx]                   ;    -> block            (ONE deref)
0x0052bb40  mov    rcx, [rcx+8]                 ;    -> element base
0x0052bb47  imul   rdx, rax, 0x2af0             ; PLAYER STRIDE = 0x2AF0
0x0052bb4e  mov    eax, [rcx+rdx+0x644]         ; player+0x644 : the LIVE biped's accessor handle
0x0052bb55  cmp    eax, 0xffffffff
0x0052bb58  jne    0x52bb6e
0x0052bb5a  lea    rdi, [rcx+rdx+0x64c]         ; player+0x64C : the CORPSE snapshot pair
0x0052bb62  call   0x71f60                      ;    validate {generation, handle}
0x0052bb69  call   0x6ec210                     ;    sobject handle -> accessor handle
0x0052bb70  call   0x6ec110                     ; accessor handle -> SOBJECT handle
0x0052bb75  cmp    eax, 0xffffffff              ; 0xFFFFFFFF => NO BODY => "unknown"
;  --- the player HAS a body: ---
0x0052bbb3  and    eax, 0x1fff                  ; index = handle & 0x1FFF (13 bits)
0x0052bbb8  lea    rcx, [0x26795e0]             ; the "sobjects" POOL HEADER (INLINE, no deref)
0x0052bbbf  imul   eax, [rcx+0x10]              ; hdr+0x10 = STRIDE
0x0052bbc3  mov    rcx, [rcx+8]                 ; hdr+0x08 = ELEMENT BASE
0x0052bbc7  vmovaps xmm0, [rax+rcx+0xe0]        ; POSITION at sobject+0xE0 (16-byte aligned)
0x0052bbd0  vandps xmm0, xmm0, [0x162a180]      ;  = {ffffffff,ffffffff,ffffffff,00000000}
0x0052bbd8  vorps  xmm0, xmm0, [0x162a190]      ;  = {0,0,0,1.0f}   => it only forces w = 1.0
```

`sobject + 0xE0` is a `float4` position `{x, y, z, _}`; the two SIMD constants only homogenise `w`.
The chain in words: a game-globals gate -> the controller->player map -> the players block
(20 x 0x2AF0) -> a component-accessor handle at `player+0x644` -> the component accessors pool ->
the sobjects pool (1500 x 0xF0) -> the position at `sobject+0xE0`.

`player+0x64C` is the CORPSE, not a fallback for the same thing. `FUN_009160E0` ("player loses its
object") snapshots the dying sobject's orientation (`+0xD0`) and position (`+0xE0`) into
`player+0x60`/`+0x70`, writes the sobject handle as a `{generation, handle}` pair to `player+0x64C`
(`0x009161B9  mov [r15+r12+0x64c], rax`), and only then calls `FUN_0090FFC0(i, -1, ...)` to clear
`+0x644`. So: `+0x644 == -1` with a valid `+0x64C` means the biped existed and died; both `-1`
means it never existed.

### 1.2 The three containers - three DIFFERENT families `[C]`

Confusing these reads a plausible pointer and hands back plausible garbage, with no error anywhere.

| container | anchor | family | reach the base | built by |
|---|---|---|---|---|
| **sobjects** | `0x26795E0` **is** the header, inline in BSS | `s_data_array` (`FUN_00070530` -> `FUN_000702C0`) | `base = [v:0x26795E0 + 8]` - **one** deref | `FUN_0032CE60`: `("sobjects", ..., 0xF0, 0x10, 0, 0x5DC, 8, 2, 0x12, 0)` |
| **players** | `0x2725700` holds a **pointer to a block** | a different family (`FUN_0023F080` -> `FUN_00074400`) | `base = [[v:0x2725700] + 8]` - **two** derefs | `c_players_game_system::vfunc[1]` (`0x90AA80`): `("players", 0x14, 0x2AF0, 4, &DAT_02725700, 7, ...)` |
| **"player array"** | `0x2670940` | `s_data_array`, `20 x 0x1C0` | - | `c_player_manager_game_system::vfunc[0]` (`0x3024B0`) |

`"player array"` is NOT the players array - a third, unrelated 20-slot pool. The name is a trap.

`0x26795E8` and `0x26795F0` (the base and stride) have **zero** absolute references in the whole
image (`d1-eboot xref 0x26795e8` -> 0 confirmed, controls PASS) while `0x26795E0` has **2,290**:
every access is register-relative off the header. An absolute xref on the base pointer is blind by
construction.

### 1.3 The `s_data_array` header and occupancy machinery `[C]`

`FUN_00070530` sizes and allocates; `FUN_000702C0` fills the header. Sizes for `sobjects`:
`elem = align_up(0xF0, 0x10) = 0xF0` · `data = 0xF0 * 0x5DC = 0x57E40` ·
`bitv = (((0x5DC+31)>>5)*4 + 0x1F) & ~7 = 0xD8` · `request = 0x57F38` (one block, three regions).

| off | meaning | sobjects value |
|---|---|---|
| `+0x00` | ptr -> state object | `block + 0x57F18` |
| `+0x08` | element array base | `block` |
| `+0x10` | stride | `0xF0` |
| `+0x14` | capacity | `0x5DC` (1500) |
| `+0x18` | peak-live watermark (see §3.2) | |
| `+0x1C`/`+0x20`/`+0x24` | salt offset 0, salt stride `0xF0`, salt mask `0xFFFFFFFF` | |
| `+0x34` | registry index - becomes handle bits 13+ | |
| `+0x41` | `char name[0x30]` | `"sobjects"` |
| `+0x74` | FNV-1 of the name | |

**State object** (0x20 B at `[hdr+0x00]`, built by `FUN_00076970`): `+0x00` element base ·
`+0x08` ptr -> bitvector object · `+0x10` stride · `+0x18` u16 FREE count · `+0x1A` u16 capacity ·
`+0x1E` u16 free-list head (`0xFFFE` = none). **Bitvector object:** `+0x00` u64 byte size (192) ·
`+0x08` u32 bit count (**1536** - larger than capacity 1500; bound every scan by capacity, as the
client's own iterator `FUN_0006EF50` does, passing `hdr+0x14`) · `+0x10` u64 -> word array
(`bitvec+0x18`).

* live count = `u16@(state+0x1A) - u16@(state+0x18)` (accessor `FUN_0006F130`; absolute form for a
  sibling pool at `0x5E3010`).
* is index i live? `FUN_0006EEE0(hdr, u16 idx)`: `idx != 0xFFFF && state[8] != 0 &&
  idx < bitcount && (words[idx>>5] >> (idx&31)) & 1`.
* **BIT SET => OCCUPIED**, proven from both directions: `FUN_01048430` (allocate) does
  `*w |= 1<<(i&31)`; `FUN_0006ECB0` / `FUN_0006EE30` / `FUN_00076970` (free/init) clear it. Not an
  inference. Canonical walker: `FUN_0006EF50`, iterator head `FUN_0008F550`.

**Per-element header** (`sobject+0x00..+0x10`): `+0x00` u32 salt/generation (incremented on both
free and alloc; low 8 bits go into the handle; `FUN_0032D180` saves only this dword, wipes the whole
`0xF0`, restores it) · `+0x04` u16 next-free index while FREE, object flags while LIVE (`& 4` =
pending delete) · `+0x06` u16 **`0xFEFE` <=> slot FREE** (independent occupancy signal) · `+0x08`
i8 **object TYPE enum** (§1.4) · `+0x0C` u32 the sobject's own handle (self-consistency check).

**Handle encoding** (`FUN_0006E6E0`, matching `FUN_00071F60`): 13-bit index in `[12:0]`, array id
in `[22:13]`, and for sobjects (the `0x40000000` bit clear)
`handle = (salt & 0xFF) << 23 | (array_id & 0x3FF) << 13 | index`. The 32-bit handle carries a salt
but neither `FUN_006EC110` nor `FUN_006EC210` checks it - a stale handle silently reads a live slot.
Only `FUN_00071F60`, taking a `{generation, handle}` pair, validates against the pool's salt array.

### 1.4 The 25-entry object-type table - **9 = biped** `[C]`

Set at construction from the object definition (`FUN_0032D180`:
`*(u8*)(sobj+8) = *(u8*)(defn+0x8E)`), read corpus-wide as
`*(char*)((h & 0x1fff)*DAT_026795F0 + 8 + DAT_026795E8)`; `-1` = none. The name table is
`DAT_026798C0 = &DAT_02697E20`, `DAT_026798D0 = 0x19` (25 entries x 0x20), built at runtime by
`0x529730..0x529D30`; entry `+0x00` is a `const char*`.

| # | name | # | name | # | name |
|---:|---|---:|---|---:|---|
| 0 | `inherited` | 9 | **`biped`** | 18 | `item_loot` |
| 1 | `static_mesh` | 10 | `creature` | 19 | `gear` |
| 2 | `prop_simple` | 11 | `weapon` | 20 | `hop_on` |
| 3 | `prop_cinematic` | 12 | `vehicle` | 21 | `hop_on_gear_biped` |
| 4 | `prop_simulated` | 13 | `turret` | 22 | `hop_on_gear_weapon` |
| 5 | `prop_expensive` | 14 | `emitter` | 23 | `hop_on_gear_ship` |
| 6 | `prop_garbage` | 15 | `projectile` | 24 | `system` |
| 7 | `speedtree` | 16 | `item` | | |
| 8 | `interactive` | 17 | `item_ammo` | | |

The table validates itself against the code: `type == '\t'` (9 = biped), `type == '\f'` (12),
`type == '\r'` (13), `type - 9U < 2` (biped-or-creature) at e.g. `0x2D8530`,
`c_spasaha_ai_game_interface::vfunc[2]` (`0x482300`), `0x516B40`. **`c_biped` does not exist as an
RTTI class** - biped-ness is the enum value 9; the only `biped` RTTI names are
`c_reject_biped_contact_listener` and two simulation-event definitions.

Other identifying sobject fields (from `FUN_0032D180`): `+0x0C` self handle · `+0x44` parent ·
`+0x48` next sibling · `+0x4C` first child · `+0x54` component-accessor block handle · `+0x98`
group id · `+0xA0` a u64 from `glue+0x80` · `+0xA8` accessor handle · `+0xB0`/`+0xC0`/`+0xD0`
orientation basis · `+0xE0` position. `[U]` there is no 32-bit `0x8xxxxxxx` definition tag stored
in the sobject - the definition is resolved from the object glue (`FUN_00102980` / `FUN_00335F50`,
through registry `DAT_01A0B420`); whether `sobject+0xA0` is the glue identity or a network-entity
id is unclosed. Do not cite a definition tag from this chain - none was read.

### 1.5 The players block - a different family, its own bit vector `[C]`

`FUN_0023F080("players", 0x14, 0x2AF0, 4, &DAT_02725700, 7, ...)` -> `FUN_0023DE00` (generic engine
allocation, writes the block pointer into `DAT_02725700`) -> `FUN_00074400(block, "players", 0x14,
0x2AF0, 4)`.

| off | meaning | players value |
|---|---|---|
| `+0x00` | ptr -> active bit vector | |
| `+0x08` | element base - the corpus' `*(long*)(DAT_02725700+8)` | |
| `+0x10` | u64 element size | `0x2AF0` |
| `+0x18` | i32 high-water mark | |
| `+0x20` | `char name[0x20]` | `"players"` |
| `+0x40` | u32 magic | `0x64407440` |
| `+0x44` | u32 capacity | 20 |
| `+0x5C` | i32 live count | |

The players name is at `+0x20`, NOT `+0x41` - the two families disagree, and the magic
`0x64407440` at `+0x40` does not exist in the sobjects header. Iterator: `FUN_00074A90` (`next()`),
used at `0x3C3DD0`, `0x3F7840` and `0x411830` (the spawn function itself). A second, independent
bridge to the biped: `*(u32*)(players_base + i*0x2AF0 + 0x04) & 0x1FFF` is that player's sobject
index, used in absolute form at `0x3C7010`, `0x3E5500`, `0x3F29B0`, `0x3E92B0`, `0x411830`. The
census can reach the biped two ways and they must agree - a control, not a convenience.

### 1.6 The local-player chain, and the gate that decides everything `[C]`

| function | what it reads | "absent" |
|---|---|---|
| `FUN_00905170` | `DAT_02724950` = `"game globals"`, 0x2B68 B. Returns `gg[1]` iff `gg != 0 && gg[0x2B48] != 0 && gg[0] == 0`. `+0x2B48` = world up; `+0x00` = start in progress; `+0x01` = game running | returns 0 => `unknown` immediately |
| `FUN_00921AF0` | literally `return (*(int*)(DAT_027257F0 + 0xC0) == -1) ? -1 : 0;`. `DAT_027257F0` = `"player mapping globals"`, 0xDC B | `-1` |
| `FUN_00921A10(0)` | `*(u32*)(DAT_027257F0 + 0xC0)` - the local player index | `0xFFFFFFFF` |
| `FUN_00921790(ctl, pi)` | the **only** writer of `+0xC0`. On map it also calls `FUN_009216D0(ctl, players[pi]+0x644, 0)` - it reads the biped handle at map time, so the biped is expected to already exist | |
| `FUN_006EC110(accessor) -> sobject` | pool `DAT_02670BB0` = `"component accessors"`, `0x20 x 0x2000`; returns `*(u32*)(obj + 0x2C)` | `-1 => -1` |
| `FUN_006EC210(sobject) -> accessor` | `*(u32*)([0x26795E8] + (h&0x1fff)*[0x26795F0] + 0xA8)` | |

### 1.7 The creator - `FUN_00411830`, and the silent gate `[C]`

Called from `0x912DF0`. The atlas labels it `c_incident_player_interface_client::ctor_or_dtor` from
an `rtti-vptr` guess - **the name is wrong, the address is right.** Its creation sequence:

```c
uVar34   = *(u32*)(DAT_02670948 + DAT_02670950*slot + 0x4C);   // -> player index
piVar43  = FUN_006697C0(uVar34);                               // -> players[i] + 0x1660
if ((piVar43 != 0) && (*piVar43 != -1)) {          // THE GATE - silent, no diagnostic
    FUN_00308230(&rec, *piVar43);                  // build creation record from the definition hash
    uVar45 = FUN_0104EF20(&rec);
    FUN_00307EF0(uVar45, local_1b88);              // set the SPAWN POSITION into the record
    iVar30 = FUN_0032D180(uVar45);                 // CREATE THE SOBJECT; -1 on failure
    if (iVar30 != -1) {
        uVar29 = FUN_006EC210(iVar30);
        FUN_00911D60(uVar34, uVar29);              // players[i]+0x644 = accessor handle
    }
}
```

* `FUN_0032D180` zeroes `+0xE0` and `+0xE8` explicitly - a freshly created sobject is **at the
  origin** until something positions it.
* The arrival transform comes from a spawn-candidate iterator (`FUN_00131420`/`571`/`810`/`9A0`)
  scored against `DAT_02695740` = `"player spawning globals"`, 0x20 B, gated by `FUN_00415B70`. If
  no candidate passes, the code falls to `LAB_0041295C`, which sets the transform to zero/identity -
  spawn at the origin, silently.
* `c_player_spawning_game_system::vfunc[2]` (`0x4115A0`) memsets those globals to 0 when
  `FUN_00132100()` returns `0xFFFFFFFF` (no activity/tag context); zeroed spawn globals make
  `FUN_00415B70`'s radius term degenerate.
* **Player spawn emits ZERO diagnostic strings.** The entire `0x40E000-0x415FFF` module contains
  exactly two string literals, both allocation tags in `0x4114D0`. Nothing in any log will ever
  report a skipped or failed spawn, on success or on failure.

**The writer of `players[i]+0x644` is `FUN_0090FFC0` = `player_set_object(i, handle, flags, skip)`**
(`d1-eboot disasm 0x90ffc0 --to 0x910400`):

```asm
0x0091002F  lea  r15, [r12+r14+0x644]              ; &players[i].object_handle   (r14 = i*0x2AF0)
0x0091031A  mov  [rax], r15d                       ; ATTACH: players[i]+0x644 = handle
0x00910387  mov  [rax], r15d                       ; (written twice)
0x009103C3  mov  qword ptr [r12+r14+0x64c], -1     ; invalidate the corpse cache
0x009101C0  mov  dword ptr [rbx+0x9d8], 0xffffffff ; DETACH: object loses its player
```

Initialisation to `-1` is at `0x0090D78D` / `0x0090D799` in `FUN_0090D4F0` (add-player) and again
at `0x0090CAE2` - the instruction proving the field was unset beforehand. Three functions a raw
displacement scan offers as `+0x644` writers (`0x4F6BF0`, `0x65CEF0`, `0x8EDE20`) are
displacement collisions (a byte write, a different `0x2C80`-stride array, an observer field) - do
not cite them.

### 1.8 The tick lane - alive all the way down to the per-player loop `[C]`

`c_game_system_interface` has NO tick slot: six virtuals, a 3-up / 3-down lifecycle.
`c_game_system_list` (typeinfo `0x18F18A0`) fans its six impls over the child array, slots 0-2
forward (`0x23F620`/`0x23F590`/`0x23F6B0`, children `+0x00`/`+0x08`/`+0x10`) and slots 3-5
**reverse** (`0x23F6F0`/`0x23F5D0`/`0x23F660`, children `+0x18`/`+0x20`/`+0x28`) - the signature of
a construct/destruct pair. So `0x411790` (`c_player_spawning_game_system::vfunc[4]`) and
`0x1089E10` (`c_director_game_system::vfunc[4]`) are DESTRUCTORS, firing only at shutdown.

The per-frame lane is a separate **14-entry step table at `v:0x192C2E0`** (stride `0x10`), distinct
from the six lifecycle slots at `v:0x192C2A8`:

```
[1] 0x74A780   carries the literal "game_tick" (registers scope FUN_00241680("game_tick"))
     -> FUN_00912DF0 (sole caller 0x74A780)
          -> FUN_00411830   hits every ~500 ms
               -> per-player loop gated on players[i]+0x0C bit 0
                    -> FUN_00414730 -> FUN_0032D180 -> biped
```

`FUN_00411830` has exactly one caller (`FUN_00912DF0`, itself with exactly one caller). It runs
every simulation tick for the whole life of the world, since orbit - the spawn is **not**
rung-gated. The loop body was empty only because there were no player records.

### 1.9 The camera - `[DAT_02723060] + 0x46C` `[C]`

`"observer globals"` = `DAT_02723060`, 0xA60 B, one observer of stride `0xA58` (proven by
`FUN_008F2160`'s `param_1*0xA58 + DAT_02723060`). The write is in `FUN_008F1D00`, the per-frame
camera solve: `camera_pos = focus(obs+0x768..0x770) - distance(obs+0x790) x
forward(obs+0x494..0x49C)`, stored to `+0x46C/+0x470/+0x474`, clamped to +/-100000.0.

`FUN_008F1D00` **early-outs** unless `param_1 > 0.0001` and at least one of the nine blend weights
at `obs + 0x138 + k*4` (k = 0..8) is `> 0.0`; otherwise it jumps to `LAB_008F1FC9` and `+0x46C` is
never written - it keeps whatever `FUN_0023DE00` left, i.e. `(0,0,0)`: camera at the origin,
ambient audio perfect, screen black. And the accessor `FUN_008F09A0(controller)` returns NULL the
instant the local player index is unset (`if (!FUN_00921AB0(controller)) return 0;`, which is
`[DAT_027257F0]+0xC0 != -1`). **The camera and the biped share a gate.**

The `game_camera_pos %.7E %.7E %.7E` line (`0x88EBA0`, caller `0x88A770`, gated on
`*(char*)(param_1+0x3514)`) is a rendering, not a value - read `[DAT_02723060]+0x46C`, not the
printf.

### 1.10 The probe ladder - run in order; the first bad reading names the failure

All addresses absolute after the first deref; `capture-rig/d1guest.py read` does exactly one level
of `--deref` and no arithmetic. First: `d1guest.py base` and `d1guest.py live` - any control FAIL
and nothing below is evidence.

| # | read | healthy | what a bad value MEANS |
|---:|---|---|---|
| 1 | `0x26795E0 --len 0x80` | `+0x10 = f0`, `+0x14 = 5dc`, `+0x41 = "sobjects"` | stop - wrong base/deref (three-constant control) |
| 2 | `0x2724950 --deref --len 0x2B60`, bytes `+0x2B48`, `+0x00`, `+0x01` | non-null, `1`, `0`, `1` | the game is not "running"; `FUN_00905170` fails |
| 3 | `0x27257F0 --deref`, `s16 @ +0xBC` | `1` | zero controllers mapped |
| 4 | `s32 @ (map + 0xC0)` | `0..19` | **`-1` => no local player.** `FUN_00921790` never ran - camera accessor returns NULL too |
| 5 | `0x2725700 --deref --len 0x80` | `+0x10 = 2af0`, `+0x20 = "players"`, `+0x40 = 0x64407440`, `+0x44 = 20`, `+0x5C >= 1` | wrong container |
| 6 | `s32 @ (players_base + pi*0x2AF0 + 0x1660)` | a valid hash | **`-1` => `FUN_00411830` SKIPS THE ENTIRE SPAWN, silently** |
| 7 | `u32 @ (players_base + pi*0x2AF0 + 0x644)` | a valid handle | **`-1` => no live biped.** `FUN_0090FFC0`'s attach never ran |
| 8 | `u64 @ (players_base + pi*0x2AF0 + 0x64C)` | `-1` while 7 is valid | not `-1` while 7 IS `-1` => the biped existed and was destroyed (`FUN_009160E0`) |
| 9 | `u32 @ (players_base + pi*0x2AF0 + 0x04) & 0x1FFF` | agrees with 7 via `FUN_006EC110` | the two bridges disagree => a control failure, not a finding |
| 10 | `i8 @ (sobject + 0x08)` | **`9` = `biped`** | any other value => the player's object is not a biped |
| 11 | `float[3] @ (sobject + 0xE0)` | map coordinates | `(0,0,0)` => created but never positioned - `FUN_00415B70` rejected every candidate |
| 12 | `float[3] @ ([0x2723060] + 0x46C)` | tracks step 11 | `(0,0,0)` => camera at the origin - the black screen |
| 13 | `float @ (obs + 0x138 + k*4)`, k=0..8 | >=1 positive | all `<= 0` => `FUN_008F1D00` early-outs, never writes `+0x46C` |
| 14 | `0x2695740 --deref --len 0x20` | non-zero | all zero => `vfunc[2]` took the `FUN_00132100() == -1` branch |
| 15 | `u32 @ ([0x26B10A0] + 0x23C)` | `0` | non-zero => rung 35's LEAVE never ran |
| 16 | `u8 @ ([0x26B10A0] + 0x08)`, bit 0 | clear | set => a type-0x3A sim-queue element enqueued by rung 36's ENTER and NEVER DISPATCHED |
| 17 | `i32 @ ([0x26B10A0] + 0x238)` | != `-1` | `-1` => rung 36's ENTER `FUN_00764B20` did nothing |
| 18 | `u8 @ ([0x26B1408] + 0x88)` | `4` (`fully_enabled`) | `3` => `FUN_00764C10()` never went false |
| 19 | `u8 @ ([[0x26B13E8] + 0x1A0] + 0x30)` | `1` | `0` => rung 36's mode-escalation block never ran |

The full occupancy sweep, for "how many objects exist and what are they":

```
hdr    = 0x026795E0
state  = u64 @ hdr;  base = u64 @ (hdr+0x08);  stride = u32 @ (hdr+0x10);  cap = u32 @ (hdr+0x14)
bitvec = u64 @ (state + 0x08);  words = u64 @ (bitvec + 0x10)
live   = u16 @ (state + 0x1a) - u16 @ (state + 0x18)

for i in 0 .. cap-1:                                  # cap (1500), NOT bit count (1536)
    if (u32 @ (words + (i>>5)*4)) >> (i & 31) & 1:    # SET => OCCUPIED  [C]
        e = base + i*stride
        assert u16 @ (e+0x06) != 0xFEFE               # corroborating free sentinel
        assert (u32 @ (e+0x0c)) & 0x1fff == i         # self-handle check
        type = i8 @ (e+0x08)                          # 9 = biped
        pos  = float[3] @ (e+0xE0)
```

A histogram of `type` over the live set IS the entity census. Controls (each invertible): **PC-3a**
sobjects header `0xF0` / `0x5DC` / `"sobjects"` · **PC-3b** players block `0x2AF0` / `"players"` /
magic `0x64407440` / capacity 20 · **PC-3c** the two bridges agree (corrupt the mask to
`& 0xFFFF` and they must disagree) · **PC-3d** live count equals popcount over `[0, cap)` ·
**PC-3e** the census differs between orbit and in-world (run first, in orbit, before the poke).
A reusable census is a Rust tool: `d1-live census` in the `d1-live` crate (its address grammar
already parses `[[v:0x2725700]+0x8]+0x644`), with these controls as its suite.

### 1.11 The negative baseline - banked from real crash reports `[C]`

`FUN_0052BB10` writes into `crash_info.txt`, which the guest uploads and `d1-responder` saves. Five
archives in the tree, all pre-world, all reading `state:: bootflow:bap_signin` /
`change-world: loaded orbit` / `player_position_x/y/z: unknown` - the measured, unambiguous
NEGATIVE reading. `crash_info.txt` also carries `activity_tier`, `activity-slice-set-index`,
`map-slice-set-index`, `in-public-bubble`, `is_current_bubble_authority`: an in-world crash report
settles the biped question by itself (extract from the first `PK\x03\x04` to the last
`PK\x05\x06`). Two strong-evidence catalogue rows (`capture-rig/diagnostic-catalogue.tsv`) that
never fired: `0x16812AC` (`0xB72660`) `local player add failed, refusal=(%s)` and `0x168632D`
(`0xB89590`) `player-properties received but no player associated with peer/user` - so the
session-level player add succeeded and the peer<->player association exists; any failure is
downstream of session membership. (Do not use the catalogue's SEEN column for post-2026-08-09
runs; it predates them.)

### 1.12 In-world log evidence from the first black-screen run `[C]` `observed-live` `synthetic_ancestor`

From `runs/20260809-230309-FAM02-OLIVE/emulator.log`, three in-world entries, first window lines
178602-185838: `Entering state 'activity:in_world'` (178602) ·
`slice_set_transition_manager: Mode changed ... to 'fully_enabled'` (179344; mode table
`0 disabled · 1 initial_slice_set_precaching · 2 initial_slice_set_instantion ·
3 enabled_for_core_load · 4 fully_enabled`; we went 2 -> 4; a discriminator, not a cause) ·
`matchmaking_manager: ... the AH has not acknowledged this for 'PRIVATE activity'` x 57 at ~1 Hz
(printed by `FUN_00769980` when `FUN_00769CB0` returns false; the result only sets/clears bit 1 of
`param_1+0x21`, the **advertisement** status - a real ours-to-fix gap for the 3v3 endgame, NOT
evidence about the local biped; `FUN_00298FB0(ac)` returned true and a later term failed) ·
`session:membership:dump: players valid: 0x1` (a session player, not a game-world biped) ·
`KTOBUCKET: tasks requested: [0x0]` (rung 36 runs no tasks; its single task `FUN_007953E0` is the
post-game skill write, requested only at game phase 7) · exit for `error.inactivity_timed_out`.
`Multiplayer Team = -1` in `activities:team_picker:dump` fired six times, only at posse
entry/exit call sites - a lead, `[U]` whether in-world code cares.

---

## 2. The walls that fell and how

### 2.1 The one root cause - `[0x27257F0]+0xC0 == -1` `[C]`

Provenance: `historicaldocs/design/THE-ONE-ROOT-CAUSE.md` (2026-08-10 late; two independent
read-only lanes, 33 agents, 0 errors, raw returns in
`design/workflow-results/2026-08-10-serverside-getin-game.journal.jsonl`, 526 KB).

> Rung-34 task 9's leg A and the missing in-world biped are the SAME failure: the
> controller->player map at `[0x27257F0]+0xC0` reading `-1`.

`~schema_0x80000000_303130()` - the value task 9 passes to `FUN_001192A0` - is
`c_spasaha_engine_interface::vfunc[39]`, resolved offline through the live vtable
(`[[v:0x2633a98]]+0x138 -> 0x6E6640`). It returns `-1` if and only if `[0x27257F0]+0xC0 == -1`;
that address read `ff ff ff ff ff ff ff ff`. One field, two symptoms - every previous framing had
treated them as separate walls.

**Two records, not one.** The claim that task 9 reads `[+8]` and the spawn gate reads `[+9]` of one
content record was retracted: they read TWO different records. Leg A (`FUN_001192A0`) resolves an
entity-keyed sub-block (`entity_table+0x50 -> tag -> +0x1F8/+0x200`); task 9 passes
`~schema_0x80000000_303130()`, the spawn gate passes the player entity handle; `[+8]` and `[+9]`
are adjacent bytes of THIS record. Leg B (`FUN_0010C500`) reads the lifetime STATE at `+0x1AC` of
a global client-bundle instance - a different record entirely. Leg A was answered (it is the
controller->player map); leg B is a second wall behind the first.

**Direction, called correctly by the census doc before the breakpoints confirmed it:**
`FUN_00921790` is the only writer of `+0xC0` and runs inside add-player, so `+0xC0 == -1` is an
EFFECT, not a cause (`historicaldocs/design/biped-census.md` banner and §5.1). This exonerated
`OPERATOR_PAD=1`.

### 2.2 Leg B - the lifetime-state record `[C]` offline, and where it now lives

`[C]` offline against `rung36-biped-chain.snap` (`d1-replay controls` 20 PASS / 0 FAIL / 0 SKIP).
The record is a "client bundle instance": local runtime RAM in a datum array, built from the loaded
world's own content. `FUN_001072C0(0x10, 0, &ref)` returns `{tag=0x27FA2003, offset=0,
class=0x8080039D}` in-world; tag decodes to datum-array slot `0x3D1`, element 3; the registry entry
back-points to `v:0x25C0DD0`, the BSS object `FUN_001D6D30` registers as
`FUN_000713E0(&DAT_025C0DD0, "client bundle instances", 0x400, 8, ...)`; live capacity `+0x24` =
`0x400`. Created by the slice-set switch: `FUN_00344490` -> `FUN_00345250` x3 -> `FUN_001D7F90`
("add to global client bundle") -> `FUN_001D0770(class, &DAT_025C0DD0)`. It ran correctly for
`pvp_moonbase`: orbit holds tag `0x54FA2003` blueprint `0x80B52052`, in-world holds tag
`0x27FA2003` blueprint `0x80F080C3` - the content side is healthy.

What was wrong: the instance was **default**. `+0x1AC` (lifetime state) = `0xFF` in both orbit and
in-world; `+0x1B4` = `0x811C9DC5` (FNV-1 basis, "no name"); `+0x1B8` = `-1`; `+0x674` = `-1`;
`FUN_0010CB60` (the accessor `FUN_00411830` calls) returned `al=0`. The writer is static-invisible
by construction: all eleven users of globals index `0x10` are readers; a corpus-wide grep for
stores at literal `+0x1AC` finds only the CRI Sofdec video system; the class tags `0x8080039D` /
`0x8080074C` appear nowhere in the eboot (control: two known schema tags each appear exactly once),
so the schema is content-defined. The phases `{3,6}` pass the spawn gate; `{6,7,8}` satisfy
`FUN_0010C750`.

Measured live (run `20260810-194139-GEARED-1`, `d1-live watch --rw exec --addr 0x75b040 --len 1`,
112 threads, `--follow-clone`): HIT #1 at t+86530 ms, tid 2879520
(`rsp=0x7efb74ce8 rdi=0x0 rsi=0x0 rbx=0x204f14a18`). `FUN_0075B040` -> `FUN_0075A540` publishes
sim-queue element `0x1D` `activity_manager_instance_start`, gated on
`session+0x8C48 in {6,7,8,9}` (`[0x26B1850]+0x8C48` read 1 in orbit, `+0x8C58` read 6; the field
climbs during the attempt - do not read its orbit value as a refusal). All definition-table slots
registered (live, `[0x26708C8]`): `0x0D`->`0x801956d28` · `0x1D`->`0x801956a48` ·
`0x1E`->`0x801956a50` · `0x1F`->`0x801956a58` · `0x52`->`0x80191eef8`. The element published and
the phase still read NONE.

**This record's resolution is owned by `design/activity-lifetime-state.md` (live, fenced -
reference only):** the phase byte `+0x1AC` arrives as a WIRE FIELD, Activity Host message type 7
`sensor_message` (32 bytes arriving already formed). Note the contradiction with the source's own
hedge - THE-ONE-ROOT-CAUSE §2 argued `[I]` that "we cannot set the phase by serving a field" (a
spasaha state machine); the fenced live doc supersedes that. See Dead claims.

### 2.3 The door the player record enters by - sim-queue element `0x0D` `[C]`

Provenance: `historicaldocs/design/biped-census.md` banner;
`historicaldocs/design/THE-ONE-ROOT-CAUSE.md` §6. Players arrive by
`FUN_002FF640(0xd, &DAT_01956D28)` -> `FUN_00BB1BC0` -> apply-added (`FUN_00BA6860`) ->
`FUN_0090D1E0`; the full chain `0xBB1BC0 -> 0xBAA790 -> 0xBA6860 -> {0x90CBB0, 0x90D1E0,
0x90F530}`. Measured with `d1-live --rw exec` on a live in-world client: `FUN_00BB1BC0` hits
unprompted at t+304 ms (the queue drains fine); `FUN_0090D1E0` never hits in 35 s with every gate
satisfied. The break was one function boundary wide: the call at `0xBA73BF`, guarded by
`if (lVar9 == 0)`, was not taken.

An earlier, superseded measurement localised the same thing from the dispatcher: an exec breakpoint
on the sim-queue dispatcher's `jmp rax` (`0xbb1c13`) over a driven join gave 176 dispatches, five
cases firing 92 times, and case 12 `player_update_apply` ZERO - with case 12 holding a unique
jump-table target, so no tail-merge confound. Nothing enqueued the player element; the applier and
the `elem[1] & 1` gate were retired as suspects. (Also learned: orbit at rest has NO sim-queue
traffic, so a NO-HIT taken while parked is uninterpretable - the control must be in the same
window.)

### 2.4 The membership fix - the seventh wrong-field wall, and the fall `[C]` `observed-live`

Provenance: `historicaldocs/design/player-in-world-membership-fix.md` (2026-08-11). The seventh
wall of the same shape - a field of the replicated blob we published wrong - and the one that had
blocked every in-world attempt in project history. No poke manufactured the player: the repaired
response crossed the wire and the client populated its own simulation cache and player pool.

**The defect in one sentence:** the Activity Host set the local-member mask
(`replicate_membership +0x502C`) but left the **simulation-occupancy mask (`+0x5028`) zero**, and
did not echo the member fireteam id from the join request - so `FUN_00BA0B10` never copied member
0's machine id into the simulation-machine cache, the simulation machine was never populated, and
the per-player gather (`FUN_00BA3C90`) had no player to append. Every downstream symptom -
`count=0`, no element 12, no player record, the controller->player map at `-1`, no biped - hung
off this one zero bit.

| field | image offset | was | now |
|---|---|---|---|
| local-member mask | `+0x502C` | set | set |
| **simulation-occupancy mask** | **`+0x5028`** | **0** | **bit 0 set** |
| member-0 fireteam id | `+0xD0` (member image) | 0 | echoed from join-request `+0xD8` |
| acknowledged session host | `+0x5020` | (unset) | player id |

`+0x5028` bit 0 is the bit `FUN_00BA0B10` tests before copying member 0's machine id into the
simulation-machine cache. The fireteam id is consumed by `FUN_0075DB30` (`fireteam_from_player`)
and the zero-fireteam case is suppressed in `FUN_00BA3D60` - a zero there silently dropped the
member from the simulation membership even when the occupancy bit was right.

**The fix:** `d1-server/crates/d1-oracle/src/activity_host.rs` (field constants +
`replicate_membership` image), `d1-server/crates/d1-activity-host/src/messages.rs` (copy member
properties incl. fireteam id from the join request; set `+0x5028`),
`d1-server/crates/d1-activity-host/src/controls.rs`. Validation: 108 AH controls PASS ·
`d1-oracle` 15 tests · `d1-activity-host` 67 tests · both crates build release clean. (Author: the
operator's GPT/Codex lane, 2026-08-11.)

**Live proof** (run `20260811-...-SIM-MASK`, in `activity:in_world`, read with `d1guest`):

| field | before (the one root cause) | after |
|---|---|---|
| player-pool count `[[0x2725700]+8]+0x18` | 0 | **1** |
| player[0] spawn-request bit `+0x0C` | 0 | **3** (gate `&1` now passes) |
| controller->player map `[0x27257F0]+0xC0` | `ff ff ff ff ff ff ff ff` (-1) | **`ec700000` ...** |
| player[0] record head | zeroed | real pointers (`0x37f84b0b0`), stride `0x2AF0`, `+0x18=1` |

The membership response was **consumed**, not just built: the client independently allocated and
populated player slot 0, bound the controller, and set the spawn request - the first time a player
record has existed in-world in project history, produced by a served field, not a poke. `[I]` the
`0xBA73BF` guard (§2.3) now passes; the downstream reads are `[C]` but that specific guard was not
re-measured after the fix.

### 2.5 The corrected next gate - the family-0 loadout chain, NOT tasks 9/10 `[C]`

The membership-fix doc's own §3/§5 initially read `player+0x644` as `0` and named tasks 9/10 as
the next gate; both were corrected in place (operator/GPT correction): `player+0x644` reads
`0xFFFFFFFF` (an explicit empty handle), and the client exits the spawn path before geometry or
sobject allocation because two earlier fields are `-1`:

```
family-0 loadout data
  -> translation-manager slot +0x34   (loader state; wants 5)
  -> player +0x85C                    (was 0xFFFFFFFF)
  -> player +0x1660                   (was 0xFFFFFFFF - the FUN_00411830 gate)
  -> FUN_00411830
  -> FUN_0032D180  creates the biped
  -> player +0x644  becomes valid     (sobject type 9)
```

The biped was skipped because `+0x85C`/`+0x1660` were `-1`, coming from a family-0 loadout the
roster never served coherently. Tasks 9/10 may become a later gate (via `FUN_00414730`, the
activity-lifetime-state check) but could not be the first gate while `+0x1660` was invalid.

**The plan of record, since executed:** a hybrid family-0 `0/1` fixture (plus the `0/0` object)
preserving valid base-character/appearance data with all twenty equipment-reference `u16`s at
decoded offset `0x220 + i*0x40` set to `0xFFFF` - the sentinels let the world-tag loader reach
state 5 without the absent family `0/3`. Served via `D1_ROSTER_DIR` with `D1_ANSWER_FAMILIES`
including 0; on a fresh player add the client runs
`FUN_0090FBC0 -> FUN_010891A0 -> FUN_009105B0 -> FUN_0073C3E0 -> FUN_006697F0`. Acceptance
checklist, in order: loader state 5 · rung-34 done `0x7FF` naturally · `player+0x85C != -1` ·
`player+0x1660 != -1` · spawn bit set · `player+0x644 != -1` · attached sobject type 9 · camera
follows it · controller input moves it. Per `design/GUARDIAN-IN-WORLD.md`: the fixture
(`capture-rig/rosters-f0-hybrid/`, built by `d1-oracle character build-peer --no-equip
--no-emblem`) cleared rung-34 task 7 naturally (done mask `0x17F -> 0x1FF`, no poke) and
`+0x85C`/`+0x1660` became valid (`0x811c9dc580a6a06c`); a type-9 biped now exists. If `+0x1660` is
valid and `+0x644` stays `-1`, THEN instrument `FUN_00414730` - only at that point do tasks 9/10 /
activity-lifetime content lead. Note: `capture-rig/rosters-f0` (without `-hybrid`) is NOT the
all-`0xFFFF` fixture - its decoded active entries remain.

### 2.6 What the done-mask force actually cost `[C]`

Tasks 9/10 do NOT install or activate game systems (task 9 `FUN_00794D30` is the prologue-filler
hold; task 10 `FUN_00794E70` is `perform_prologue_filler_exit_playback`; neither makes a
game-system call). Installation happens at module init: `c_game_system_list` count = 108 in both
orbit and rung-34 snapshots; `v:0x2695740` populated with live floats; sim-queue def slot 82 =
`v:0x191EEF8`. What the force DID cost: task 10's `FUN_007648D0(0xC)` never wrote
`cinematic_manager+0x23C`, and `FUN_007646B0(2)` never enqueued sim-queue element `0x39` - the
cinematic manager parked mid-phase. A real poke-caused divergence, in the cinematic plane.

Behaviour IS content-driven, and slot 2 is where content enters:
`c_player_spawning_game_system::vfunc[2]` (`0x4115A0`) resolves a tag via `FUN_00132100()`, walks
the package registry `DAT_01A0B420`, and copies 0x20 bytes from a content record into the globals
block. Across six snapshots the block at `a:0x37F9789A0` is byte-identical in five orbit captures
and differs in the one `pvp_moonbase` capture at exactly one float, `+0x14` (`1.0 -> 1.4`) - and
all four `0x411830` sites are pure reads, so the tick did not write it. `vfunc[2]` re-ran across
the world change and pulled the activity's own record. `[O]`, n=5 vs 1.

---

## 3. The census instrument and findings

> **Retraction status (banner carried forward, and it overrides the body):** the census document is
> RETRACTED IN PART, 2026-08-10 late. **Every measurement stands; the headline diagnosis does
> not.** "The world came up with an EMPTY ROSTER" read as a defect to be fixed by filling
> `gg + 0xD0` is dead: `gg + 0xD0` is zero BY DESIGN -
> `c_omaha_game_first_game_system::vfunc[1]` memsets the block, and the only reference to
> `FUN_009048A0` publishes it to `gg` already zeroed. "All 0x1220 bytes zero" is what a healthy
> client looks like at game start, and `FUN_00904AC0` adding zero players is not the break.
> Players arrive by the sim-queue element `0x0D` door (§2.3). See Dead claims.

Reach: everything in this section is `synthetic_ancestor` - rungs 34-36 were reached with pokes (a
forced task done-mask `0x1FF -> 0x7FF` skipping tasks 9 and 10, a suppressed-cinematics byte, poked
permission flags, 16 invented entitlement offer keys). Nothing here is evidence about a real
service. Provenance: `historicaldocs/design/biped-census.md`.

### 3.1 The selection gap that forced a new instrument

None of the three `rung36-in-world-*` snapshots capture the heap the classic census needs:

| datum | address (identical in every snapshot) | in the in-world snapshots? |
|---|---|---|
| `sobjects` element array | `0x37fbbf600` | no |
| `sobjects` state object / bit vector | `0x37fc17518` / `0x37fc17440` | no |
| `players` block (holds `+0x644`) | `0x37f815570` | no |
| `game globals` | `0x37f8000c0` | no |
| `observer globals` (the camera) | `0x204fc5170` | no |

Only `rung34-task8-cleared-registry` and `orbit-all` carry all four. "A missing range is NOT a
zero" (`d1-replay`). shadPS4's guest arena allocator is DETERMINISTIC: every singleton pointer
holds the identical value in all 16 snapshots across four runs on three days. The gift: a wide
heap window from one run is dimensionally comparable to another's. The trap: "same address" is
never evidence of anything - pointer identity as a control always passes.

### 3.2 The instrument - a whole-BSS pool census on the monotonic high-water mark `[C]`

`hdr+0x18` is not merely a "peak-live watermark": `d1-eboot field-xref --global 0x26795E0 --offset
0x18` (4 controls PASS including the inverted one) returns 4 tracked accesses over 1846
displacement matches and names the writer, `FUN_0006E750` (the s_data_array allocate routine):

```
[C]  fn 0x0006e750  0x0006e7c6  READ   mov eax, [r14+0x18]
[C]  fn 0x0006e750  0x0006e7d8  WRITE  mov [r14+0x18], esi
```

(That is `xref`-invisible by construction - `mov [r14+0x18]` contains no address; the access mode
dictated the scanner.) Its decompilation (`0006e750__schema_0x80000000_6e750.c` in the decompiled
corpus) ends the allocation path with:

```c
uVar4 = *(ushort *)((long)plVar2 + 0x1a) - uVar4;          // live = capacity - free
if ((int)uVar4 < 0 || uVar4 <= *(uint *)(param_1 + 3)) {   // param_1+3 == hdr+0x18
    uVar4 = *(uint *)(param_1 + 3);                        //   keep the old value
}
*(uint *)(param_1 + 3) = uVar4;                            // hdr+0x18 = max(hdr+0x18, live)
```

**`hdr+0x18 = max(hdr+0x18, live)` on every allocation, never decreasing.** `hdr+0x18 == 0` means
"not one element has EVER been allocated from this pool since process start" - not "none right
now". That is why it works on a snapshot.

Pool headers are inline in BSS (the eboot window always covers them). Signature: `hdr+0x40 == 0x01`
followed by a NUL-terminated ASCII name at `hdr+0x41`, with `+0x08` base, `+0x10` stride, `+0x14`
capacity, `+0x18` the mark. BSS is `0x1a09088..0x2f50dc0` (`d1-eboot segments`; bias per-segment).
The scanner found **193 pools in 2.7 seconds**. It was a Python prototype (`/tmp/census/pools.py`,
session-local) and belongs in Rust per the operator rule.

### 3.3 The controls - every one can fail, one is inverted

| control | what it tests | result |
|---|---|---|
| PC-3a: sobjects header reads stride `0xF0`, capacity `0x5DC` (1500), `"sobjects"` at `+0x41` | base, bias and deref at once - three constants from three unrelated places | PASS in every snapshot |
| PC-3d: bit-vector popcount over `[0, capacity)` equals `capacity - free` | the bit vector is the one the pool maintains | PASS (577 = 577; 53 = 53) |
| PC-3e: the census **differs** between orbit and in-world | the walk reads live state | PASS - `sobjects` 63 -> 577, `component accessors` 326 -> 1383 -> 1418, 70+ pools move, `activity clients` 0 -> 1 |
| free-sentinel: no bit-set slot carries `0xFEFE` at `+0x06` | a second occupancy signal | PASS - 0 violations of 577 |
| self-handle: `(u32 @ elem+0x0C) & 0x1FFF == index` for every live slot | stride and base are right | PASS - 0 mismatches of 577 |
| negative control: the eboot file image has zeros throughout BSS | reading live memory, not the file | PASS - every header holds live heap pointers |
| tool controls: `d1-eboot controls` 27/27 · `d1-replay` BASE + L0-L3 · `field-xref` 3 positive + 1 inverted | the instruments themselves | ALL PASS |

**PC-3f - proving the snapshots were taken IN-WORLD, from the snapshot itself.** A monotonic mark
cannot distinguish "in-world" from "was in-world an hour ago", and the run DID fall back:
`runs/20260809-230309-FAM02-OLIVE/emulator.log` records 3 `Entering state 'activity:in_world'`
(lines 178603 / 252253 / 380038) and 3 `Leaving` (185839 / 318343 / 403487), each for
`error.inactivity_timed_out`. Package-registration pools are NOT monotonic (cleared on
unregister), so they discriminate current state:

| package pool | orbit | in-world x3 |
|---|---:|---:|
| `packages/ps4_orbit_01a9_3.pkg` | 764 | absent |
| `packages/ps4_orbit_01ac_2.pkg` | 225 | absent |
| `packages/ps4_ui_orbit_0026_3.pkg` | 1165 | absent |
| `packages/ps4_race_set_all_00ee_3.pkg` | absent | 579 |
| `packages/ps4_globals_pvp_0169_3.pkg` | 1451 | 2773 / 2780 |
| `activity clients` | 0 | 1 |

PASS: orbit content unregistered, PvP content loaded, so the snapshots were taken inside an
activity. (`inworld-B`/`C` read 2780 where `inworld-A` reads 2773 - a later window.) The honest
limit at the time: no positive calibration for a non-zero `player array` existed - later supplied
live by the membership fix (player-pool count 1, §2.4).

### 3.4 The result `[C]` `synthetic_ancestor`

Full sobject census (only where the heap was captured):

| | `orbit-all` (setup:orbit) | `rung34-task8-cleared-registry` (rung 34, pvp_moonbase 407) |
|---|---:|---:|
| live sobjects | **53** | **577** |
| `system` (24) | 42 | 356 |
| `prop_simple` (2) | 1 | 98 |
| `emitter` (14) | 0 | 39 |
| `interactive` (8) | 0 | 36 |
| `static_mesh` (1) | 6 | 23 |
| `prop_simulated` (4) | 0 | 19 |
| `prop_cinematic` (3) | 4 | 6 |
| **`biped` (9)** | **0** | **0** |
| `creature`(10) `weapon`(11) `vehicle`(12) `gear`(19) | 0 | 0 |

A fully loaded PvP world with 577 objects and not one biped, weapon, creature, vehicle or piece of
gear - everything present is scenery, emitters, interactives and engine systems.

In-world high-water marks, read from BSS at the `activity:in_world` instant:

| pool | orbit | rung 34 | in-world x3 |
|---|---:|---:|---:|
| `sobjects` | 63 | 577 | **577** |
| `component accessors` | 326 | 1383 | **1418** |
| `sobject message queues` | 46 | 309 | 309 |
| `deterministic_render_records` | 24 | 450 | 457 |
| **`player array`** | 0 | 0 | **0** |
| `player start locations` | 0 | 0 | **0** |
| `actors` | 0 | 0 | **0** |
| `ai player state` | 0 | 0 | **0** |
| `s_actor_input_data array` | 0 | 0 | **0** |
| `s_actor_executor_data array` | 0 | 0 | **0** |
| `character physics component registry` | 0 | 0 | **0** |
| `ragdolls` | 0 | 0 | **0** |
| `spawn volume instances` | 0 | 0 | **0** |
| `player spasaha array` | 0 | 0 | **0** |
| `perk manager / effects / timer registries` | 0 | 0 | 0 |
| `gear component registry` | 3 | 3 | 3 |
| `native player control instances` | 1 | 1 | 1 |

`sobjects` reads 577 at rung 34 and 577 in-world while `component accessors` climbed 1383 -> 1418
over the same interval: the client went on allocating, and never allocated another sobject - across
all **three** entries into `activity:in_world` (the mark is process-monotonic).
`player start locations == 0` independently corroborates the §1.7 warning: zero spawn candidates
because the pool was never populated.

The `player array` heap, read directly from `rung34-task8-cleared-registry.snap` - a third
agreeing route:

```
[v:0x2670940]        -> state @ 0x37fa3a8a0 : free = 20 (+0x18), capacity = 20 (+0x1A)  =>  LIVE = 0
[[v:0x2670940]+0x8]  -> bitvec @ 0x37fa3a880 : 8 bytes, 64 bits, words @ 0x37fa3a898
words                -> 00 00 00 00 00 00 00 00      <== NOT ONE BIT SET
```

### 3.5 The mechanism - causal, not correlated `[C]`

`FUN_00411830`'s first executable statements iterate exactly the `player array` bit vector:

```c
uVar45 = *(undefined8 *)(*(long *)(DAT_02670940 + 8) + 0x10);  // words = player_array.state->bitvec->words
uVar29 = FUN_0006f140(&DAT_02670940);                          // = *(u32*)(hdr+0x14) = CAPACITY (20)
FUN_0008f550(local_1c20, uVar45, uVar29);                      // bit_vector_iterator_begin(it, words, cap)
...
if ((int)local_1c38 <= (int)local_1c48[0]) goto LAB_00413de7;  // exhausted -> stack check -> RETURN
```

Every player-index read (`*(uint *)((DAT_02670950 * slot) + 0x4c + DAT_02670948)`, decompilation
lines 263, 300, 336, 473, 486, 702) lives inside that loop body, and so does the whole creation
sequence (`FUN_00308230` -> `FUN_0104EF20` -> `FUN_00307EF0` -> `FUN_0032D180` -> `FUN_00911D60`).
With an all-zero bit vector the loop body never executes once; `FUN_0032D180` is never called on
this path; the gear-bearing step `FUN_00308230(&rec, *piVar43)` (creation record from the
character's definition hash) is inside the loop, so the abort was strictly upstream of every gear
consideration - refuting Hypothesis B ("no biped because no equippable gear") as the proximate
cause.

Scope precisely: `d1-eboot xref 0x32D180` returns 76 call sites - it is the general sobject
constructor and most callers build scenery/props/emitters (how the 577 came to exist). §3.4's
measurement is independent of the mechanism: it enumerates every live sobject's type regardless of
creator, and finds zero of type 9. The two agree. `player array` is not vestigial:
`d1-eboot xref 0x2670940` returns 421 confirmed references across four reference kinds (controls
PASS) - a live, heavily used pool that had simply never been given an element.

### 3.6 One rung further up - the roster block, and what the retraction left standing

The population path runs once per world load, off `change_world`:

```
FUN_00753540 "change_world"
  -> FUN_0034CB50            ("load successful, but failed to instantiate world and start game")
      -> c_omaha_world_controller_shim::vfunc[4]  @ 0x92B750     [C] via reloc on vtable 0x1933540
           -> *(u8*)(game_globals + 0x2B48) = 1                  "world is up"
           -> FUN_00904AC0:
                 for i in 0..19:
                     if (*(char*)(gg + 0xD0 + i*0xE8) != 0)      the roster gate
                         FUN_0090D1E0(i, gg + 0xD0 + i*0xE8, 0)
                           -> FUN_0090D4F0  (add-player)
                                -> FUN_00921790(...)   the ONLY writer of the local-player index +0xC0
                                -> initialises players[i]+0x644 / +0x64C to -1  (0x90D78D / 0x90D799)
                                -> ... -> FUN_006697F0   writes players[i]+0x1660
```

Measured `[C]` in both `orbit-all` and `rung34-task8-cleared-registry` (game globals @
`0x37f8000c0`): `gg + 0x2B48` = 1 (world up) · `gg + 0x01` = 1 (game running; `FUN_00905170`
passes) · `gg + 0x00` = 0 · **`gg + 0xD0 .. +0x12F0` (20 x `0xE8`) all 0x1220 bytes ZERO** ·
players header name `"players"`, magic `0x64407440`, cap 20, live count `+0x5C` = 0. The
measurements stand. The diagnosis ("empty roster = the defect") is retracted: the block is zeroed
by design (banner above), and `FUN_0090D1E0` is instead invoked from the sim-queue `0x0D` door
(§2.3). The `[I]` that `gg+0xD0` is filled from the change-world command (argued from adjacency:
`0xD0 + 20*0xE8 = 0x12F0`, immediately below the launch state block at `[0x2724950]+8+0x12F8`) is
dead with it.

### 3.7 The `!= -1` landmine - the default is `0`, and `0` PASSES `[C]`

For all 20 players, in both heap-bearing snapshots:

```
players[i] + 0x1660 (spawn gate)   = 0x00000000     <== NOT -1
players[i] + 0x644  (biped handle) = 0x00000000     <== NOT -1
players[i] + 0x64C  (corpse pair)  = 0x0000000000000000
```

`+0x644 == 0` rather than `-1` is itself a finding: `-1` is written by add-player
(`0x90D78D`/`0x90D799`), so a slot reading `0` was never initialised at all - one step further
back than both of the ladder's `-1` cases. And it settles an open `[U]` in the worst direction:
the spawn's inner gate is `if (*piVar43 != -1)`; the measured default `0` **passes** - so a
roster populated without `FUN_006697F0` also writing a real definition hash would have
`FUN_00411830` call `FUN_00308230(&rec, 0)` and build a creation record from **definition hash
0**. Whatever creates a player must assert `+0x1660`, or "no biped" is swapped for "a biped built
from definition hash 0". This survives the retraction and still matters.

### 3.8 Secondary - the equipped-gear state at the time `[C]`

Read from the object registry (`[v:0x26A2040]` -> `0x37f8dfe80`), which the in-world snapshots do
capture; family storage offsets from `capture-rig/guest-symbols.json`:

| | `orbit-all` | `rung34-...-registry` | in-world (A and B) |
|---|---:|---:|---:|
| fam 3/1 `C+0x250` equipped slots non-empty | 0/20 | 14/20 | 0/20 |
| fam 4/1 `+0x2B80` equipped instance SOIDs | 0/20 | 13/20 | 0/20 |
| fam 4/3 inventory elements with any non-zero byte | 0/666 | 0/666 | 0/666 |

The empty case is unambiguous: the 64-byte slot repeats with `u16 @ +0x08 = 0xFFFF` (the empty
marker; `0x0000` is a VALID index on this path) and `u32 @ +0x20 = 0x811C9DC5` (FNV-1 offset
basis, hash of the empty string):

```
0x37f8fb9e8  00 00 00 00 00 00 00 00  ff ff ff ff ff ff ff ff
0x37f8fb9f8  ff ff ff ff ff ff ff ff  ff ff ff ff ff ff 00 00
0x37f8fba08  c5 9d 1c 81 00 00 00 00  ff 00 ff ff ff 00 ff ff
```

Do NOT read the 14/20 row as "gear purged between rung 34 and rung 36" - it is a DIFFERENT RUN:
`rung34-task8-cleared-registry` is from `20260809-120451-MOONBASE-FRESH`; the in-world snapshots
from `20260809-230309-FAM02-OLIVE`; `orbit-all` from `20260808-011129-DEST-OFFSET-PROBE` (also
0/20). The difference tracks server configuration, not the rung; the fam-3/1 headers differ too
(`0000010000000000` vs `00010000ffff0000`). One genuine cross-run observation: the `player array`
mark is 0 in MOONBASE-FRESH too, the run with 14/20 equipped - gear alone does not populate it
(that run never reached rung 36, so it does not test gear at the spawn). `[U]` the 340-slot
content array was not located: character `+0x38` returns repeating `0x811C9DC5`, profile `+0x38`
all zeros; closing instrument is `field-xref` on the profile class at `+0x38`, or `d1-live watch`
while an item name resolves in orbit (the resolve is the positive control).

### 3.9 Transferable lessons (kept with the instrument)

1. A monotonic counter is a snapshot's best friend: a high-water mark says something about the
   whole process from one instant.
2. When the heap is missing, ask what lives in BSS - the pool headers are inline; 193 pools, 2.7 s,
   no extra capture.
3. "Zero everywhere" needs a mechanism, not just a measurement: `player array == 0` alone is
   equally consistent with "never ran" and "this build never uses this pool"; only the 421 xrefs
   plus the spawn function's own opening instructions discriminate.
4. Cross-run comparison is a trap in a deterministic emulator - always print the run directory.
5. State the access mode before choosing the scanner (`hdr+0x18`'s writer is invisible to `xref`
   by construction; `field-xref` found it in one call).
6. **`field-xref` also has a shape it cannot see, and reports the miss cleanly with all four
   controls PASSING.** `d1-eboot field-xref --global 0x2725700 --offset 0x1660` returns
   `0 tracked over 0 displacement matches, 788 candidates` - the instrument is fine, the question
   is mis-shaped: `field-xref` resolves `singleton + disp`, and `players[i]` is
   `*(base + 8) + i*0x2AF0`, a double indirection plus an index scale. Three shapes, three
   instruments: `xref` = "who touches this ADDRESS" · `field-xref` = "who touches
   `singleton + N`" · an indexed element of a heap pool = **neither** - enumerate consumers
   through the named accessor instead (`xref 0x6697C0` -> 11 confirmed), or watch the address at
   runtime. A clean zero from a controlled instrument is still the right answer to the wrong
   question - the third distinct time this project was bitten by that.
7. A default of `0` where the code compares `!= -1` is a landmine, not a detail (§3.7).

---

## 4. The watch plan and what remains

Provenance: `historicaldocs/design/biped-spawn-watch.md` §3-§6. A census is a state; the open
questions are events, and you cannot snapshot your way to an event - that is what `d1-live` is for.

### 4.1 Constraints and safety

* **One tracer per tracee.** `d1-live watch` PTRACE_SEIZEs every thread of shadPS4 (111 threads
  when written) and takes one `--addr` per invocation - two watches cannot run at once. Four debug
  registers exist and three sit idle; making `--addr` repeatable across `--slot 0..3` is the
  single highest-leverage instrument change.
* An attempt costs a poke, not a boot; the Activity Host hot-swaps. Sequential watches are
  minutes, not boots.
* **A tracer that dies armed kills its target** `[C]` (abandoned tracee dies of SIGTRAP, status
  `0x85`, core dumped - the hardware breakpoint outlives the implicit detach). NEVER SIGKILL
  `d1-live watch` at shadPS4. Ctrl-C, SIGTERM, panic, error return and `--max-seconds` are all
  safe (`Watcher::disarm` runs in `Drop` with a verified drain of `SigPnd`/`ShdPnd`).
  `--max-seconds` defaults to 120 and is a seatbelt.
* A data breakpoint reports RIP as the instruction AFTER the store - quote the `writer` line. A
  `longest of several memory-writing decodes` confidence label is `[O]` until settled with
  `d1-eboot disasm`.
* Read the zero-hit summary in words: evidence is only about the threads armed at start;
  `--follow-clone` is not optional.
* Alignment is mandatory and `d1-live` refuses rather than rounding: `+0x644` is 4-aligned
  (`--len 4`); `+0x64C` is not 8-aligned; `+0x648` is 8-aligned and one 8-byte watch there covers
  the tail of `+0x644` and the head of `+0x64C`.

### 4.2 The layers

* **Layer 0** - the ladder (§1.10), no watchpoint, always first.
* **Layer 1** - "did anything ever TRY to give the player a body?" (when steps 7 and 8 both read
  `-1`): write watch on `players_base + pi*0x2AF0 + 0x644`, `--len 4 --rw write --slot 3 --count 4
  --max-seconds 240 --follow-clone`. Outcomes: writer in `0x910000..0x9103FF` = the attach ran
  (built-in known-answer check - the stores are pinned at `0x0091031A`/`0x00910387`); writer only
  `0x90D78D`/`0x90D799`/`0x90CAE2` = initialisation to `-1`, reset never attached; another writer
  = an alias finding; never fires with the positive control passing in the same session = the
  definitive negative, nothing ever attempted the attach; never fires with the control also silent
  = the instrument, cite nothing. Cheaper reachability sibling: `--rw exec --len 1 --count 1` on
  `0x411830` or `0x912DF0`.
* **Layer 2** - "who positions the biped?" (step 7 valid, step 11 `(0,0,0)`): write watch on
  `sobject_addr + 0xE0` (`sobject_addr = base + (handle & 0x1FFF) * 0xF0`), `--len 8` (16-byte
  aligned field). This is where the watch strictly outranks static analysis, measured:
  `field-xref --global 0x26795e0 --offset 0xe0` returns 718 displacement matches across 3,416
  candidate functions, 9 tracked, every tracked hit a `lea rip` materialisation, not a store.
  Expected placer shortlist: the `0x300A00` / `0x300B10` / `0x301160` / `0x301350` transform
  family - `0x301350` is the one `FUN_0090FFC0` itself calls at `0x0091032A`. Fires repeatedly =
  simulated normally, go to Layer 3; fires once at ~0 = placed at origin, never moved; never fires
  = the object is inert, nothing simulates it.
* **Layer 3** - the camera (`[0x2723060] + 0x46C`, `--len 8`): fires inside `FUN_008F1D00` = the
  solve runs with wrong inputs (read focus `obs+0x768..0x770`, distance `obs+0x790`, forward
  `obs+0x494..0x49C`; follow `FUN_008E3120` to the director, `DAT_02722DD0`, `"director
  globals"`, 0x420 B); never fires = the early-out (all nine blend weights `<= 0`, or
  `FUN_008F09A0` NULL because the local index is unset) - ladder step 13 answers which without a
  watchpoint.

### 4.3 The positive controls

* **PC-1** - do guest threads stop at all: exec watch on `0x769980` (prologue-anchored;
  rate-limited on `1000 < now - _DAT_026B1570`; entered >= 57 times in 119 s in-world), `--count 1`
  mandatory (fires at frame rate). Silent for 30 s while in-world = instrument failure, cite no
  negative from the session.
* **PC-2** - a known answer from an independent static derivation: the activity-loader writer
  `FUN_00341500 @ 0x341798`, `memcpy(block+0x000, block+0x148, 0x145)` (PENDING -> CURRENT
  activity name). Watch `'[[v:0x267AF08]+0x8]+0x0' --len 8 --rw write`, armed BEFORE the activity
  poke. Watch the POINTEE, not the field: `c_activity_loader+8` is a heap handle written once by
  an allocator; a watch on the field itself never fires and reads as failure.
* **PC-3** - the census's own five controls (§3.3), PC-3e free in orbit before the poke.

### 4.4 The capture-preset fix and the falsifier

Add to the in-world capture (prefer `--map` over `--region` - it takes the whole enclosing mapping
so a wrong length cannot silently truncate; verify with `d1-replay info` that each address landed
inside a region before citing any read - "not captured" and "was zero" are different facts):

```sh
d1-replay capture --out runs/snapshots/rung36-in-world-FULL.snap \
  --run-dir runs/<RUN> --state "rung 36 activity:in_world - full entity heap" \
  --map 'a:0x37fbbf600'   `# sobjects element array (via [v:0x26795E0]+0x08)` \
  --map 'a:0x37fc17518'   `# sobjects state object + bit vector` \
  --map '[v:0x2725700]'   `# the players block - holds +0x644` \
  --map '[v:0x2724950]'   `# game globals` \
  --map '[v:0x27257F0]'   `# player mapping globals - +0xC0 is the local player index` \
  --map '[v:0x2723060]'   `# observer globals - +0x46C is the camera position` \
  --map '[v:0x2670940]'   `# player array` \
  --map '[v:0x2670BB0]'   `# component accessors`
```

Also extend `--preset content` to cover the datum-array registry table around
`a:0x204403280 + 0x40000` and the slot-`0x3D1` arena: of 17 banked snapshots only
`rung36-biped-chain` and `orbit-all` resolve the client-bundle record; three other in-world
captures do not contain `a:0x380261ED0`, and both rung-34 captures FAULT at
`READ_UNMAPPED 0x204443520`.

**The census's falsifier:** if a live in-world read ever shows `player array +0x18 > 0` while no
biped exists, §3.5's mechanism is wrong and the census conclusion must be retracted in place.

### 4.5 What remains `[U]`

| `[U]` | the instrument that closes it |
|---|---|
| **Possession/control - the Guardian stands but does not walk** (the frontier; queue in `design/GUARDIAN-IN-WORLD.md`) | the live spine owns this; do not duplicate it here |
| The specific store that sets `sobject+0xE0` for a player biped (718 displacement matches / 3,416 candidates, 9 tracked, all `lea`) | §4.2 Layer 2's write watch - one hit, no candidate list |
| Whether the sobject carries a content definition tag at all (`+0xA0` is a u64 from `glue+0x80`; resolution goes through `FUN_00102980` / `FUN_00335F50` and registry `DAT_01A0B420`) | live-read `+0xA0` for a spawn with a known content name; or `field-xref` on the accessor payload from `sobject+0x54` |
| `DAT_0267960C` (header `+0x2C`, slice/bucket id) - written 0 at construction, zero absolute stores, but that negative assumed an absolute-symbol search space | `d1-live watch --addr 'a:0x267960c'`, or `field-xref` on data-array offset `+0x2C` |
| Whether `"player start locations"` (`DAT_026AADE0`, 16 x 4 B) is live at all in 01.29 - only references in all 47,304 files are the registrar `0x6B4200`, three destructors, two schema (de)serialisers, no gameplay reader | a read watchpoint on `*(void**)(0x26AADE0+8)` |
| Whether `Multiplayer Team = -1` matters in-world | one grep during a census run; then `s_object_filter_type_wrapper<s_object_filter_player>` (typeinfo `0x191E970`) if it does |
| Whether rung 36's type-0x3A batch element is ever dispatched (ladder step 16; `[0x26B10A0]+0x08` bit 0 = enqueued and never dispatched) | ladder step 16; exec breakpoint on `c_simulation_queue_cinematic_batch_request_definition::vfunc[2]` |
| The `e_activity_slice_set_loading_task` / `e_activity_in_world_task` enumerator spellings | not recoverable from this image - the task entry struct `{u64 prereq, fn_plain, pmf_ptr, this_adjust}` has no name column and `d1-eboot nametab` (168/172 tables whole) has no matching table |
| The 340-slot content array's real offset (§3.8) | `field-xref` on the profile class at `+0x38`, or `d1-live watch` while a name resolves in orbit |
| Whether skipped tasks 9/10 are upstream of anything that still matters (reframed by §2.5: only if `+0x1660` is valid and `+0x644` stays `-1`, via `FUN_00414730`) | compare a run clearing 9/10 honestly; the pool census is the diff tool |
| Camera at the origin (`[0x2723060]+0x46C`) - uncaptured in-world at census time | §4.4's capture; read the field, not the printf; expect a consequence of the local index |
| The lifetime phase and its `+0x108` unbound setter | fenced: `design/activity-lifetime-state.md` owns it - reference only |

Session-keeping: `error.inactivity_timed_out` ended five in-world sessions and killed one
mid-measurement; stick movement delays it, button presses do NOT (measured 2026-08-10: the pad
acked every `press a: ok` and the client timed out anyway; `axis lx/ly` bought ~18x more time).
Per the membership-fix correction, the inactivity holder now works (held a process > 1 h) and is
no longer a blocker.

Do-not-re-arm list (each entry justified in §5): `0x411790`, `0x1089E10` (destructors),
`0x414680` (resurrection apply - `c_simulation_queue_player_resurrection_definition::vfunc[2]`,
element 82; nobody resurrects, NO-HIT expected), `0x414730` (inside the per-player loop),
`0x33C210`, `0xF8210` (bare `ret` bytes).

---

## 5. Dead claims

Every claim below was asserted by one of the four source documents and is DEAD. Do not carry any
of them forward.

| dead claim | what killed it |
|---|---|
| "The world came up with an EMPTY ROSTER" as a defect to fix by filling `gg + 0xD0` | Retraction banner on `historicaldocs/design/biped-census.md`: `gg + 0xD0` is zero BY DESIGN - `c_omaha_game_first_game_system::vfunc[1]` memsets the block and `FUN_009048A0` publishes it already zeroed; all-zero is what a healthy client looks like at game start |
| `[I]` that `gg+0xD0` is filled from the change-world command (census §5.1) | Same banner - nobody fills it; the door is sim-queue element `0x0D` |
| The proposed watch on `gg + 0xD0` ("it has never been non-zero, so any hit is the answer") | A trap guaranteeing a NO-HIT that reads as a failed search: "any hit is the answer" needs a hit to be possible (census §7 row 1; lesson in `status/LESSONS.md`) |
| "Task 9 and the spawn gate read adjacent bytes of ONE record" (asserted by `historicaldocs/design/arrival-handoff.md` §6.3, `status/TODO.md`, and the session brief) | THE-ONE-ROOT-CAUSE §1 retraction: two DIFFERENT records - leg A `FUN_001192A0` (entity-keyed sub-block) vs leg B `FUN_0010C500` (`+0x1AC` of a global client-bundle instance) |
| `[0x27257F0]+0xC0 == -1` as the BREAK, with `OPERATOR_PAD=1` as the suspect | Census §5.1's direction call, confirmed by breakpoints: `FUN_00921790` is the only writer and runs inside add-player, so `-1` is an EFFECT; `OPERATOR_PAD=1` exonerated. The field itself was then closed outright by the membership fix (§2.4) |
| "Tasks 9/10 install or activate game systems" | Refuted: task 9 `FUN_00794D30` and task 10 `FUN_00794E70` make no game-system call; installation is at module init (`c_game_system_list` count 108 in orbit and rung 34; `v:0x2695740` live; slot 82 = `v:0x191EEF8`) |
| "The game systems are never ticked" (the two NO-HITs on `0x411790` / `0x1089E10`) | Those are DESTRUCTORS (3-up/3-down lifecycle, no tick slot) - correct answers to a wrong question; the real tick lane is the 14-entry table at `v:0x192C2E0` and it is alive down to the per-player loop. Do not re-arm |
| "`0x33C210` and `0xF8210` are ICF-folded generics" | They are single-byte `ret` instructions used as vtable filler and universal no-op callback (also `0x57EA30`, `0x2E2A90`, `0x116AD0`, `0x50FE20`); any control built on them is invalid |
| "`0x414680` NO-HIT means a dead system" | It is the apply handler for element 82 = player RESURRECTION; nobody resurrects, so NO-HIT is expected |
| "`d1-eboot xref <vtable>` finds all instantiations" | Structurally blind to every heap-constructed class (the compiler emits `lea reg,[vtable - 0x10]; add reg,0x10`): 94 of 129 `*_game_system` vtables referenced only at V, 33 only at V-0x10, 0 both, 2 neither - asking only V loses 26% and reads as "never instantiated". Always query both V and V-0x10 |
| "The player-element applier or the `elem[1] & 1` gate is the block" | Dispatcher distribution (exec breakpoint on `jmp rax` at `0xbb1c13`): 176 dispatches, case 12 `player_update_apply` ZERO with a unique jump-table target - nothing enqueued the element (superseding measurement, `historicaldocs/design/player-element-never-enqueued.md`); the enqueue itself was then fixed by the membership response |
| The cinematic hypothesis ("skipping rung-34 tasks 7/9/10 cost the spawn") | Refuted four independent ways (spawn-watch §7): tasks 4 and 6 are the same cinematic family with the same suppress fast-path and were already done (`done = 0x07F`, 2026-08-09 ~12:14, pre-poke); element `0x39` (`c_simulation_queue_cinematic_start_stop_definition`, payload `{u32 name_hash; u8 arg; u8 flags}`, handler `0x007644E0`) only places props at a compile-time constant transform; rung 35's LEAVE (`0x007963C0`) unconditionally resets the phase (`FUN_007648D0(0)`; `FUN_00764950()`); rung 36's ENTER has one unrequested task, the post-game skill write |
| "The next gate is tasks 9/10" and "`player+0x644` reads `0`" (membership-fix §3/§5 first reading) | Corrected in place (§4 of the source): `+0x644` reads `0xFFFFFFFF` (an empty handle), and the gate was the family-0 loadout chain (`+0x85C`/`+0x1660` at `-1`) - since crossed by the hybrid fixture (`design/GUARDIAN-IN-WORLD.md`) |
| "`error.inactivity_timed_out` blocks in-world sessions" (as a standing blocker for this work) | The inactivity holder works (held a process > 1 h) - no longer a blocker (membership-fix §4 correction) |
| "`capture-rig/rosters-f0` is the all-`0xFFFF` fixture" | It is not - its decoded active entries remain; the delivered fixture is `capture-rig/rosters-f0-hybrid/` |
| Hypothesis B: "no biped because it has no equippable gear" (as proximate cause) | The spawn aborts strictly upstream of every gear consideration (empty `player array` bit vector; the gear-bearing step is inside the never-entered loop). Gear was real and broken, but not the reason the Guardian had no body |
| "The black screen is a pure renderer/camera problem" | The census brief's falsifier did not fire: no occupied biped slot was found, so that reading was never demonstrated |
| `historicaldocs/design/THE-BLOCKS.md` B28: "the 134 `*_game_system` classes never observed running `[U]`" | Closed `[C]`: 108 are constructed, entered, and content-populated, provable offline from banked snapshots |
| The three displacement-scan "`+0x644` writers" `0x4F6BF0`, `0x65CEF0`, `0x8EDE20` | Checked individually: a byte write, a different `0x2C80`-stride array, an observer field - displacement collisions. Do not cite |
| The diagnostic catalogue's SEEN column, for runs after 2026-08-09 | It reads `seen=0` for lines the run printed 57 times (`0x163EC3A`); it predates these runs. Cite the run log |
| `[I]` "we cannot set the lifetime phase by serving a field" (THE-ONE-ROOT-CAUSE §2's honest ceiling) | Superseded by the fenced live doc `design/activity-lifetime-state.md`: the phase byte `+0x1AC` is a WIRE FIELD - Activity Host message type 7 `sensor_message`, 32 bytes arriving already formed. No banner exists on the source; this table is the banner |

---

## 6. Sources

| source | melted into | status at consolidation |
|---|---|---|
| `historicaldocs/design/player-in-world-membership-fix.md` | §2.4, §2.5 | live finding `[C]` `observed-live`, with its own in-place §4 correction honoured |
| `historicaldocs/design/THE-ONE-ROOT-CAUSE.md` | §2.1-§2.3, §2.6, Dead claims | live findings; its §6 was already superseded in place; its §2 phase hedge superseded by `design/activity-lifetime-state.md` |
| `historicaldocs/design/biped-census.md` | §3, Dead claims | RETRACTED IN PART - census stands, diagnosis dead |
| `historicaldocs/design/biped-spawn-watch.md` | §1, §4, Dead claims | reference material live; its cinematic-hypothesis target refuted by its own §7 |

Raw agent returns behind §2.1:
`design/workflow-results/2026-08-10-serverside-getin-game.journal.jsonl` (526 KB). Related live
docs: `design/GUARDIAN-IN-WORLD.md` (the current spine - recipe, synthetic remainder, queue) ·
`design/activity-lifetime-state.md` (fenced: phase byte, spawn-gate wire field) ·
`design/activity-host.md` · `design/activity-launch.md` · `design/queuez-replication.md` ·
`historicaldocs/design/player-creation-chain.md` ·
`historicaldocs/design/player-element-never-enqueued.md` ·
`historicaldocs/design/rungs-31-to-36-preflight.md` · `historicaldocs/design/rung34-task7.md` ·
`historicaldocs/design/live-watch.md` · `historicaldocs/design/activity-loader-writer.md` · `historicaldocs/design/THE-BLOCKS.md` ·
`status/LESSONS.md`.
