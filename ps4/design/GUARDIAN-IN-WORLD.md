# 🎉🎉🎉 A GUARDIAN IS IN-WORLD, RENDERED - 2026-08-11 `[C]` `observed-live`

> ## 🎉🎉🎉🎉 SAME DAY, LATER: **PLAYABLE.** Run `140504-GEARED-RADV-11`, on the real GPU.
> Operator played for 15+ minutes: stick moves the Guardian, jump audible, **Warlock super fired
> and re-fired** (then went permanent - super economy is unserved state, doing improv). The
> missing picture was a held SCREEN-FADE OBJECT, not a render failure:
> **`[0x2694B90]` → fade block** (stable alloc `0x3806fca00`): `+0x00` count, `+0x14` mode,
> `+0x18` alpha (`0x3f800000` = 1.0f = black). Zero all three (`--abs`, through the pointer) and
> the picture appears. On this run it was **set-once, stuck** - the engine parked the fade at 1.0
> waiting for a release event we never serve (`[I]` the task-9/10 hand-off the done-mask force
> skips). The decaying/held lever is the OTHER byte (`[0x26A33E8]` fly-in suppress, §2 update).
> Full verified sequence: launch poke → suppress held → mask 0x1FF natural → frozen hooks →
> force → `in_world` → **wait `fully_enabled`** → spawn word → zero the fade block.

**First time in project history.** Run `20260811-021850-HYBRID-F0-2`, `pvp_moonbase` / activity 407.
The Guardian biped is **constructed, rendered, and standing in the world** with the Ghost out. ~2 fps,
cooked shadows, no player control yet - but a real D1 Guardian on our own server, end to end.

**Live proof:** sobjects census **type 9 (BIPED) = 1** at world pos `(-66.83, 54.15, 8.82)` (it moved
from its spawn point `(-49.4, 45.0, 7.2)`, so it is live in the simulation); `player[0]+0x644 =
0x4cfac067` (valid sobject handle, was `0xFFFFFFFF`); emulator alive; **operator confirms it on screen.**

---

## THE RECIPE - every poke/hook, in order (all reversible)

### 1. The server fixtures (committed - these are the REAL, served half)
* **Membership fix** (`ffdb981`): AH `replicate_membership` sets the simulation-occupancy mask
  `+0x5028` (was 0) + echoes the member fireteam id ⇒ the client builds its own player record.
* **Hybrid family-0 fixture** (`373f091`, `7977579`): `d1-oracle character build-peer --no-equip
  --no-emblem` → `capture-rig/rosters-f0-hybrid/` - a valid base character with **all 20 equipment
  content indices at `0xFFFF`**, peer `+0x00` = the QueueZ link key (porcupine fix). This clears
  **rung-34 task 7 NATURALLY** (done mask `0x17F → 0x1FF` with no poke) and installs the character
  definition: **`player+0x85C` and `player+0x1660` become valid** (`0x811c9dc580a6a06c`, were `-1`).

### 2. Boot
```sh
D1_ANSWER_FAMILIES=0,2 D1_ACTIVITY_HOST=1 D1_AH_BIND_PORT=4110 \
D1_ROSTER_DIR=$PWD/capture-rig/rosters-f0-hybrid OPERATOR_PAD=1 \
SHADPS4_BIN=$HOME/dev/shadPS4-d1trace/build-d1trace/shadps4 \
  capture-rig/run-boot.sh <NAME>
```

> ### ⭐ 2026-08-11 UPDATE - the recipe re-proven on the real GPU (run `20260811-124741-GEARED-RADV-5`), three corrections
> 1. **Sequence that works, on RADV, with the GEARED roster:** orbit → launch poke → wait for the
>    slice-set transition to complete + the `Timeout entering the prologue-filler` line (~4-5 min;
>    the done mask reads `0x1FF` by then, tasks 0-8 including task 7 natural) → **hooks first,
>    freeze-write-thaw** (`kill -STOP` → all 18 hook bytes → `kill -CONT`; a torn multi-poke patch
>    on the hot getter crashed run `121021`, `LESSONS.md`) → mask `0x7FF` + join `0` → the client
>    walks 34 → `physics_join` → **`in_world` within seconds** → spawn word `player[0]+0x0C = 3`
>    (chain `[[0x2725700]+8]+0x80`, all raw-vaddr reads) → the fade lifts, Guardian spawns.
> 2. ⛔ **PARTIALLY RETRACTED (same day, run `134907-GEARED-RADV-9`): the cine-suppress poke is
>    not load-bearing for the LADDER, but it IS the SCREEN lever.** RADV-6/7 skipped it and were
>    black with audio despite a live biped; RADV-2 and RADV-9 poked it and rendered. It is the
>    fly-in curtain suppress (`activity-launch.md` §4.3 lever A), **it decays** (every ledger
>    re-poke shows `before=00`) **and must be HELD** - re-poke `[0x26A33E8]` deref byte0 = 1 every
>    ~2 s from the ladder walk onward. Operator-confirmed: rendered in-game the moment the
>    hold + spawn landed. ⚠ Side effect: with suppress held from the start, the `Timeout entering
>    the prologue-filler` log line may never print - gate on the done mask reading `0x1FF`
>    directly, not on that line.
> 2a. ⭐ **SEQUENCING RULE (RADV-10 negative / RADV-9 positive): fire the spawn word only after
>    `slice_set_transition_manager` logs `to 'fully_enabled'`.** A spawn into a half-instantiated
>    world constructs a biped (type 9 appears) but **never binds `player+0x644`** (stays 0, vs a
>    real handle like `0x4cfac067`), and no handle = no camera = black screen forever - an edge
>    re-trigger of the spawn word does NOT re-run the bind. RADV-9's "wasted" 6-min prologue wait
>    was accidentally load-bearing: it let instantiation finish. With the fly-in suppress held,
>    total orbit-to-fully_enabled is ~2-3 min. **Verify the handle 10 s post-spawn** instead of
>    trusting the poke.
> 2b. **Crash signature of the forced path, now named** `[O]` (RADV-9, ~60 s post-spawn):
>    `c_simulation_sobject_entity_definition::vfunc[19]` (`0xBBDEDE`) read NULL`+0x4f` - the
>    NOP'd-gate biped lacks an entity-definition backpointer a legit spawn would fill. This is
>    the "budget for crashes" instability with a face; sessions last ~1 min post-spawn.
> 3. ⛔ **`d1guest read` takes raw vaddrs like `poke`** - a pre-based `0x8...` address silently reads
>    an empty page as zeros. An afternoon of "the mask never fills" was this bug (`LESSONS.md`).

### 3. Ladder to in-world (pokes - `capture-rig/d1guest.py poke … --confirm`)
| poke | addr | value | why |
|---|---|---|---|
| launch | `session-params.py --launch-poke --activity-id 407` | - | leave orbit |
| suppress cine | `[0x26A33E8]` deref byte0 | `1` | task-7 first-line escape |
| done mask | `0x26B5730` | `0x7FF` (u32) | force past tasks 9/10 (world content we don't serve) |
| join ctr | `0x26B5770` | `0` (u32) | rung 35 → 36 |

### 4. ⭐ THE TWO HOOKS that produced the biped (the brute-force half - `[U]` provenance, disposable)
| hook | addr | patch | original | why |
|---|---|---|---|---|
| **`FUN_0010C500`** (activity-lifetime-state getter) | `0x10C500` | `c7 07 03 00 00 00 b8 01 00 00 00 c3` (`mov [rdi],3; mov eax,1; ret`) | `55 48 89 e5 41 56 53 48 83 ec 20 49` | fabricate lifetime state 3 (accepted by `FUN_00414730`'s `0x48` mask) |
| **per-player reject `je`** in `FUN_00411830` | `0x411E16` | `90 90 90 90 90 90` (6 NOPs) | `0f 84 71 03 00 00` (`je 0x41218d`) | `FUN_00414730` still returned `al=0`; NOP the branch so execution falls into the construction body at `0x411E1C` |

### 5. Fire the spawn
`player[0]+0x0C = 3` (u32). ⇒ `FUN_00411830`'s per-tick loop runs the (now-unguarded) construction
→ **`FUN_0032D180` builds the biped**, `player+0x644` goes valid, sobject type 9 appears.

⚠ **Addresses:** `player[0] = [[0x2725700]+8] + 0x80` (⛔ the pool HEADER is at `+0x0`; player 0 is
`+0x80` - reading the header as player 0 is off by `0x80` and gives garbage). sobjects pool hdr
`v:0x26795E0` (base `+0x08`, stride `+0x10`=`0xF0`, cap 1500, type = low byte `+0x08`, 9 = biped,
pos `+0xE0`).

---

## What is REAL vs SYNTHETIC (honest provenance)
* **REAL, served, committed:** the membership fix and the hybrid family-0 fixture. Task 7 clears on
  the wire; the character definition (`+0x85C`/`+0x1660`) is installed by served data, not a poke.
* **SYNTHETIC pokes/hooks:** the done-mask force (skips tasks 9/10 = world content), the two code
  hooks (§4), and the spawn-word poke. These fabricate the task-9/10 outputs the biped path demands
  rather than serving them. Everything in-world is `synthetic_ancestor`.

## `[U]` THE LAST MILE - possession / control / framerate
* **No player control yet** - a deliberate forward stick push did not move the biped; it is
  constructed but not possessed. The controller→player→biped data chain is intact
  (`[0x27257F0]+0xC0` slot 0 = `0xEC700000`, `player+0x644` valid), so the gap is the possession/camera
  bind that a *legitimate* spawn runs after construction - which the `0x411E16` NOP jumped over.
* **The Ghost persists and won't dismiss**, shadows are "cooked", ~2 fps. Cosmetic/perf; the ~2 fps is
  partly the live hardware watchpoints and the forced path.

## Next, to earn each synthetic piece back onto the wire
1. **Possession/control:** trace what a clean spawn does after `0x411E1C` (the construction body) to
   bind camera + input; that is the difference between "a Guardian stands there" and "you play it".
2. ⛔⛔ **Tasks 9/10 - REFRAMED 2026-08-11 ⇒ `design/activity-lifetime-state.md`.** The
   `FUN_0010C500` "lifetime phase" (`+0x1AC`) is **not a servable wire field** - it is a **local
   BlamScript-VM variable** (setter `FUN_0010BC20`, `c_bs_game_system` binding table `0x18FEB50`),
   and the AH `script_state` channel (udt 38) that could carry it is a **decode-and-drop stub in
   01.29**. Measured offline on this very snapshot: the real `FUN_0010C500` returns `al=0`, state
   byte `0xFF`, i.e. **our hook carried the whole in-world result**. ⭐ The activity-MANAGER ladder
   DOES reach state 6 on our server, so the wall is strictly the script layer. **Open `[U]`:** does
   407's BlamScript load + tick locally, stuck on a servable precondition? If not, this is the
   documented cloud-script ceiling and the two hooks are the honest cost.
   *(Leg A `FUN_001192A0` = the controller→player map, a separate wall - `THE-ONE-ROOT-CAUSE.md`.)*
3. **Then the biped constructs with no code patch at all** - contingent on #2's open question.

## Session operational findings (2026-08-11, the reproduction + RenderDoc push)

⛔ **FPS - CORRECTED: X11 does NOT fix in-game fps.** Earlier I recorded "~30 fps on XWayland" - that
was the **orbit/menu** screen, not in-game. **In-game on XWayland is 6-7 fps** (operator, on screen).
The `D1_X11`-no-`D1_RENDERDOC` A/B was attempted (2026-08-11) and did **not** yield a number - see the
measurement traps below. The one clean datapoint remains **6-7 fps in-game**; the 30 was a menu; and
whether RenderDoc vs the world is the drain is still `[U]`.

⛔⛔ **HOW NOT TO MEASURE IN-GAME FPS - two dead ends, both paid for:**
1. **The shadPS4 fps overlay `hotkey_show_fps = F10` CRASHES the client.** Sending F10 (via `xdotool`)
   killed shadPS4 outright, twice. **Do not use F10.**
2. **An in-world screenshot is BLACK unless the biped exists** - no player ⇒ no camera ⇒ nothing
   presented (confirmed: `import -window` of a no-biped in-world session is pure black + title bar).
   So an fps A/B needs the full crash-prone spawn recipe just to get a rendered frame, and then F10
   can't be used to read the rate.
⇒ **`[U]` a headless fps measurement.** Neither the guest log nor the host `shad_log` emits a
rate-countable per-present event. The open options: a **guest frame counter** sampled over time (not
yet located), or the **operator's eyes** on the overlay-free window (the number just isn't on screen
without F10). This is a "when the operator is watching" task, not a headless one.

⛔ **HOOK SEQUENCING: apply the two code hooks ONLY after `activity:in_world`.** Applying them at
orbit / during the ladder walk **crashes the client before rung 34** - the `je` NOP is in the per-tick
spawner `FUN_00411830`, which runs every frame; with no valid player and the reject branch removed it
executes the construction path on garbage. The stable order is: ladder → in-world → hooks → spawn.

⚠ **The forced-construction path is UNSTABLE.** The biped renders, but the client crashed several
times this session (the NOP'd gate + faked getter is a synthetic path the rest of the engine does not
expect). Each `activity:in_world` reproduction is a fresh boot + the full recipe; budget for crashes.

⚠ **RenderDoc capture is `hotkey_capture_frame = F12`** (`input_config/global.ini`), fired at the
XWayland window with `xdotool key --window <shadPS4-win> F12`. A ~2 GB `.rdc` appeared momentarily at
`~/.local/share/shadPS4/captures/CUSA00219/CUSA00219_capture.rdc` then did NOT persist - RenderDoc
holds the capture in the injected app instance; securing it to disk reliably was not solved this
session (likely needs `qrenderdoc` connected to the running app, or a clean app exit). ⇒ **`[U]`: the
render capture of the Guardian was triggered but not banked.** The **memory** snapshot WAS banked.

## THE ARTIFACT - `runs/snapshots/guardian-in-world-moonbase.snap` (60 MB)
The in-world instant, captured live: **sobjects slot 0 `+0x08 = 0x09` = type-9 BIPED is IN the
snapshot**, plus `--preset content` (11,647 objects), the player pool and the full sobjects pool. So
"a Guardian existed in the world" is answerable **offline, forever**. ⚠ `d1-replay controls` reports
**1 FAIL** on it - the stack-canary control at `v:0x10c500`, because the snapshot faithfully captured
our **hook** there (not a real defect; the function is our stub in this capture).

## Related
`historicaldocs/design/player-in-world-membership-fix.md` · `historicaldocs/design/player-element-producer-chain.md` ·
`historicaldocs/design/THE-ONE-ROOT-CAUSE.md` (the `+0xC0 == -1` field, closed) ·
`runs/snapshots/guardian-in-world-moonbase.snap` · `runs/20260811-021850-HYBRID-F0-2/`
