# Mission director: driving a mission from the stand-in (2026-09-28)

The disc carries no mission scripts; they ran on Bungie's activity hosts (`feature-gaps.md`,
section 2). The stand-in drives a mission by changing the replicated state of the mission's
objects: objectives, sequences, cinematics, triggers and squads. This note gives:
- what each state's fields do, from the EBOOT handlers;
- the mission-script format;
- the first script, `missions/a_guardian_rises.json`.

Object names and ids come from [`mission-objects-highway.md`](mission-objects-highway.md). An
object reference (type `0x808004b1`) is {u32 group key, 6-bit kind, 16-bit index}.

## What the states do

**Objective, kind 0x20, state `0x80802a1b`.** Handler `0x3561d8`. It copies the 0x2d0-byte state
to component +0x110.
- `+0x0`, u3: the objective state. The handler treats 3 specially (clears +0x3f2). Our guess is
  1 = active, 2 = complete; the run tests it.
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

**Player trigger, kind 0x1d, state `0x80800463`.** The state is {bool, i32}. Our guess is that
the bool arms the trigger. The client should report a trigger through the upstream object
stream (type 6), the way it reports its own player object `0x4786c0e0/0xc/5` (id at bit 11 in
TEST366/367). The mission matches the trigger's encoded id anywhere in a type-6 body.

**Squad, kind 0x1, state `0x8080071e`.** Handler `0x1f8d20`.
- `+0x44` (u31) is a generation: a new value re-runs the squad.
- `+0x5c` (2 bits, bias 1) picks the action:
  - 1: `0x233388` + `0x233470`, probably spawn;
  - 0: `0x2335f8`, probably despawn;
  - −1: nothing.
- `+0x10`: 6 × u32, copied to component +0x31c.
- `+0x0`, `+0x8`: object references (for example the firing area).
- Per the Truman talk, the host places the squad and the physics host (the client) spawns and
  runs the AI.

## Mission scripts

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
- **`on` matchers:**
  - `object`: the object's encoded id appears in a client message of that type;
  - `slice_set`: the client's slice-set report (type 21) names that slice set as current;
  - `regex`: over the body hex, as `--ah-on` does.

  Each rule fires once.

`missions/a_guardian_rises.json` follows the reference recording (`feature-gaps.md`):
1. Revival cinematic `pf_104_res` at +20 s.
2. The first-person part `pf_104_res_fp` at +90 s.
3. At +140 s: "Enter the Wall" (objective 3 with mask −1), its incoming sequence, and its armed
   trigger `0x1d/30`.
4. On that trigger: complete it and start "Find a Weapon" (7).
5. On `0x1d/34`: "Grab the Rifle" (11), and arm the Ghost-light cinematic trigger (`0x1d/37`
   -> `pf_104a_pwr`).
6. On `0x1d/35`: "Fight through the Wall" (14).
7. 20 s after the client reports The Breach (slice set 24): three Fallen squads.

The timings and values marked as guesses are what TEST371 checks.
