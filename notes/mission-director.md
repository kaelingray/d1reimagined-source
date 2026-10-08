# Mission director: driving a mission from the stand-in (2026-09-28)

The disc carries no mission scripts; they ran on Bungie's activity hosts (`feature-gaps.md`,
section 2). The stand-in drives a mission by changing the replicated state of the mission's
objects: objectives, sequences, cinematics, triggers and squads. This note gives:
- what each state's fields do, from the EBOOT handlers;
- the mission-script format;
- the first script, `missions/a_guardian_rises.json`.

Object names and ids come from [`mission-objects-highway.md`](mission-objects-highway.md). An
object reference (type `0x808004b1`) is {u32 group key, 6-bit kind, 16-bit index}.

**Build scope:** the original handler addresses and field interpretations below
describe 01.00 unless an explicit 01.24 paragraph supersedes them. The 01.24
objective state is 0x2f0 bytes and its active HUD value is5, not the original
provisional value1. Playback/handoff observations do not accept complete revival,
Ghost casting or waypoints. The [8 October reference comparison](guardian-rises-reference-review.md)
records the visible gaps and actual 01.24 marker registration gates.

## What the states do

**Objective, kind 0x20, state `0x80802a1b`.** Handler `0x3561d8`. It copies the 0x2d0-byte state
to component +0x110.
- `+0x0`, u3: the objective state. `0x355abc` switches on it (jump table `0x11ed49c`, cases 1-7):
  - 1: the plain case;
  - 2: formats numbers several times (`0x17f10`), which looks like progress counters;
  - 3 and 4: call into the character/progress code (`0x295a58`, `0x426fd4`);
  - 5, 6, 7: small tracker updates.

  The apply handler also special-cases 3. So the script uses 1 = active and 3 = complete; the
  run confirms it. The value does not decide whether the objective shows; the mask below does.
- **`+0x2c0`, i32: player mask.** Bit n is player slot n, where the slot is byte +4 of the
  player table entry. The objective is active for the local player only if its bit is set.
  **The stand-in never set this mask, so no objective could show.** The mission uses −1 (all
  players).
- When the objective becomes active for the local player, and the definition's +0x44 byte is 1,
  the handler posts the HUD notification `0x54a634` with the definition's +0x68 name hash.
- `+0x40` holds 10 entries of `0x8080293c`: waypoints/sub-objectives, each with object references.
  The loop at the end copies them to the markers through `0x4e7948`.
- `+0x4`, `+0x38`: object references (for example the nav point).
- `+0x10`: progress, {bool, 5 × i32, f32}.

**Cinematic, kind 0x6, state `0x8080254d`.** Handler `0x4ed278` -> `0x4ece68`. The state goes to
component +0x108.
- Nothing happens unless `+0x4` (u32) differs from the last value: it is a play counter.
- `+0x8` bool: playing. When it turns on, the handler checks, in order:
  - the optional object reference at `+0xc` (the player must be inside that object; skipped if
    the reference is absent);
  - `+0x14` (6 bits, bias 3), a team/slot filter; −1 disables it;
  - the roster count at `+0x18` must be below 16.

  Then `0x4ecd08` copies the cast list and starts the cinematic tag (definition +0x44) through
  `0x3cd488`.
- `+0x9c`: the cast list, {3-bit count, up to 6 machine ids}. It should hold the local player's
  machine id.
- `+0x8` turning off stops playback (`0x4ecaa0`). `+0x9` true preloads the cinematic (`0x3cd82c`).

TEST360–363 set the roster but never changed `+0x4` or set `+0x8`, so nothing could start.

**Sequence, kind 0x5, state `0x808029a1`.** These are the incoming and completion sequences:
Ghost dialogue and the objective banners. Handler `0xb94414`.
- `+0x4` u8: a step. A new value other than 0xff re-evaluates the sequence through `0xb942a0`.
- `+0x0` i32: the start time in simulation ticks. `0xb942a0` returns "expired" when the start
  plus the definition's duration (+0x48 seconds × `0x1fb4d0` ticks/s, 30 by default) is already
  past "now". Otherwise it plays (`0xb93cbc`) or marks the sequence pending.
- "Now" is `0x1c94e0`: the last host tick of the type-5 packets plus the time elapsed since,
  in ticks. **The stand-in sent tick 0 in every packet**, so the client's mission clock stays
  near 0. `--ah-sim-clock 30` now sends an advancing tick. Start times use `"$now"`.

**Player trigger, kind 0x1d, state `0x80800463`.** The state is {bool, i32}. The handler
`0xb2f99c` copies it to component +0x10c. The update `0xb2fb68`:
- runs only while +0x0 is true (**armed**);
- reports only while its last report time (component +0x108) is earlier than **+0x4, the
  arming time**;
- reports through `0xb2f9ec`. That function posts an engine event (`0x251d04`, event hash from
  `0x18bc524`) and sets the last report time to now (`0x1fb424`).

So each arming reports once: the host arms with {0: true, 4: "$now"}, which needs
`--ah-sim-clock`. The client message that carries the report is not known yet. TEST366/367 saw
type 6 (the client's own player object) and one type 18, which decodes as 8 object references
of type `0x808004b1` but not this mission's. The script therefore matches the trigger's encoded
id in any client message type (`"type": "any"`).

**Squad, kind 0x1, state `0x8080071e`.** Handler `0x1f8d20` in 01.00.
- `+0x44` (u31) is a generation: a new value re-runs the squad.
- `+0x5c` (2 bits, bias 1) decides what happens to the squad's current members when the
  generation changes:
  - 0: destroy them (`0x2335f8` -> `entity_destroy`);
  - 1: release them (`0x233388`: a virtual call per member);
  - −1: nothing.

  Either way `0x233470` then clears the member list. It is **not** a spawn switch; the scripts
  use −1. In the corrected 01.24 static type export, the same 2-bit, bias-1 field is at
  **+0x74** in the enlarged 0x7c-byte squad state. Mission JSON names it
  `squad_existing_members`; expansion maps that name to +0x5c for 01.00 or +0x74 for 01.24.
  The 01.24 runtime handler has not yet been traced, so its behavior remains to be verified.
- `+0x40` (2 bits): a per-member value applied to every member when it changes (`0x857cd8`),
  probably an AI state.
- `+0x50` goes to component +0x110 and is applied to the members as well. `+0x54`, `+0x58` and
  `+0x4c` go to +0x348 / +0x34c / +0x354.
- **What makes the client spawn members is still open.** It is not in the apply handler. Per
  the Truman talk, placement is auth state and the physics host spawns, so the likely trigger is
  a new generation in a loaded bubble. The run shows whether that is enough.
- `+0x10`: 6 × u32, copied to component +0x31c.
- `+0x0`, `+0x8`: object references (for example the firing area).
- Per the Truman talk, the host places the squad and the physics host (the client) spawns and
  runs the AI.

**01.24 runtime (JOIN79, 2026-10-02).** OBSERVED unless marked.
- The squad component's handler table sits at `0x12e71d0`: 8 entries of {descriptor, name hash,
  flags}, listed from `0x1899120`. The state apply is `0x20d1dc`; it is reached through the
  generic state dispatch `0x27a3f4`, the same path as the sequences. The per-frame update is
  `0x20c4a4`, and the generation handler is `0x20cc38`.
- Both the apply and the update start with the **authority gate `0x20f858`**:
  - the object's +0x2c is its bubble (14 for `sq_ghost`; -1 for activity/sequence objects,
    which then use the local slot);
  - the gate looks up the object's group in the table at `*0x1917100` and gets the simulation
    record (one record shared by all six groups, at `0x6023e780` in JOIN79);
  - `0x1d8e5c` then requires that slot's bit to be **set in +0x844 and clear in +0x850**.
- Without that, the apply returns before `0x20cc38`: three generations in JOIN79 had no effect,
  and +0x844/+0x850 were all zero.
- +0x844/+0x850 are per-roster-slot flags (65 slots). Their writers:
  - `0x1d8468` (simulation start) sets +0x844 to all ones and copies it to +0x850;
  - `0x1d90e0` (the roster delta, called per slot from `0x1d9440`) sets both bits when a slot's
    generation changes, and clears both once an active slot has been processed;
  - `0x1d99cc` and `0x1d9940` act on the local slot (`0x1daf04`) when its bit is in +0x844 but
    not in +0x850 (`0x1e0410(sim+0x18, 1)`).

  How a real host produces "set in +0x844, clear in +0x850" for a bubble slot is still OPEN; the
  JOIN80 hooks record the transitions.
- **Test (memory write on the owned test emulator):** setting bit 14 of +0x844 made the next
  generation (+0x44 = 4, existing members -1) run `0x20cc38`, `0x245204` and `0x20c714`.
  - Within a second, the client sent type-6 upstream messages for group `0xecf574b5`: the squad
    (kind 1/28), the six kind-4 spawn slots, and then **`sq_ghost__ghost` (kind 2/29)**, the
    Ghost member.
  - This tests client-side creation in bubble 14; it does not establish that host kind-2
    State A is never needed. The October 5 authored-member test creates and binds the
    Breach Ghost through kind-2 State A, while the client still creates the physical unit.
  - Not seen on screen yet: the run ended on the inactivity kick a minute later.
- Encoding: the 01.24 squad state is 0x7c bytes. +0x5c does not exist in 01.24; use
  `squad_existing_members` (+0x74).

## Mission scripts

**October 4 correction:** the reset masks and generation-handler observations
above are valid, but they do not independently prove physical member creation.
`0x245204` clears a bookkeeping byte. The object matcher can find a member ID
inside another object's payload, and JOIN98's Breach Ghost squad still has zero
live members after its generation. The final state has separate desired member
counts at `+0x2c`; see [the count investigation](squad-member-counts-0124.md).
Kind-2 components in the replication pool must be distinguished from physical
member datums and visible scene objects.

**October 5 update:** JOIN109 verifies that authored Ghost member State A
creates one physical unit and supplies the member/performance bindings that
the count-only path leaves absent. JOIN111 repeats this from the delayed
current-24 mission reaction, after ready baselines and the manual bubble
reset, without a manual member state. The shipped Ghost action now uses
`sq_nux_ghost__ghost`; it does not also send `sq_nux_ghost` desired counts.
The separate manual performance test reaches native dispatch and moves the
same Ghost; its timing and torch effect are not enabled automatically. See
[the detailed evidence](ghost-light-0124.md#join111-the-automatic-rule-creates-and-binds-one-ghost).

`bap_server.py --ah-mission FILE` reads the script and expands it into `--ah-object-at` (timed)
and `--ah-on` (reactive) changes:

```json
{"objects": {"NAME": "GROUP:KIND:INDEX:TYPE[:B]"},
 "steps": [{"at": 20, "set": {"NAME": {"offset": "value"}}},
           {"on": {"type": 6, "object": "NAME"}, "delay": 0, "set": {}},
           {"on": {"type": 21, "slice_set": 24}, "delay": 20, "set": {}},
           {"on": {"type": 15, "regex": "^41"}, "set": {}}]}
```

- **State values** are the `--ah-object` JSON: offsets as keys, nested objects for structs, lists
  for counted arrays. Two placeholders are filled in when a change is sent:
  - `"$player"`: the local player's machine id, the `--ah-sim-player` owner;
  - `"$now"`: the host tick.
  Squad states may also use the key `"squad_existing_members"` for the build-specific field
  above. Other keys remain numeric offsets.
- **`on` matchers:**
  - `object`: the object's encoded id appears in a client message of that type;
  - `slice_set`: the client's slice-set report (type 21) names that slice set as current;
  - `regex`: over the body hex, as `--ah-on` does.

  Each rule fires once.
- **`{"at": S, "teleport": SET}`**: a host teleport of the activity session to slice set SET
  (world-state group request; see `checkpoint-2026-10-01-profile.md`, JOIN51-53).
- **`"clock"`** (top level): where `at` counts from. `"world"` (default) = when the stand-in sends
  the world state and simulation; `"ready"` = the client's first in-world report (type 17 status
  `0x8000000e`, about 14 s later on 01.24), so the script's times do not depend on load time.
  The clock is per connection today (one activity instance per connection - a multiplayer debt in
  `architecture-multiplayer.md`); with shared instances it belongs to the activity instance.

`missions/a_guardian_rises.json` (clock `ready`) follows the reference recording (`feature-gaps.md`):
1. Revival cinematic `pf_104_res` at +20 s.
2. The first-person part `pf_104_res_fp` at +90 s; both stopped at +90/+135 s (a cinematic that
   ends on its own leaves a black fade, JOIN52).
3. At +140 s: host teleport 113 -> 112, "Enter the Wall" (objective 3 with mask −1), its incoming
   sequence, and its armed trigger `0x1d/30`.
4. On that trigger: complete it and start "Find a Weapon" (7).
5. On `0x1d/34`: "Grab the Rifle" (11), and arm the Ghost-light cinematic trigger (`0x1d/37`
   -> `pf_104a_pwr`).
6. On `0x1d/35`: "Fight through the Wall" (14).
7. 20 s after the client reports The Breach (slice set 24): three Fallen squads.

The timings and values marked as guesses are what TEST371 checks.

Before a mission run, check both scripts against the activity schema that the stand-in will
load. The preflight encodes and decodes every scripted state to catch missing offsets and values
that do not fit the selected build's schema. `--ah-mission` now runs the same check at startup:

```powershell
python scripts\d1_mission_preflight.py --schema local\SIGNON_LOCAL\investment_schema_full.json `
  missions\a_guardian_rises.json missions\tower_npcs.json
```

The offline check passes for both scripts against the saved 01.00 activity schema and the
corrected 01.24 EBOOT type export. It verifies serialization, not client behavior.

## Live control (debug tool)

`--ah-control FILE` applies mission steps appended to FILE while a session runs, one JSON line
each: `{"set": {"obj_enter_wall": {"0": 1, "0x2c0": -1}}}` (object names from `--ah-mission`).
A line `{"teleport": SLICE_SET}` queues a host teleport (e.g. 24, the Breach) at once.
Lines starting with `#` are ignored; each connection starts reading at the file's size, so old
lines are not replayed. Used to re-send states during reverse engineering without restarting.
Temporary: applies to every connection of the stand-in; the replacement is an operator console
on the activity-instance service (debt register in `architecture-multiplayer.md`).
