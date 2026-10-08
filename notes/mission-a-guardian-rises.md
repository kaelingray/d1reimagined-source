# A Guardian Rises (act_cosmo_m0_highway, 01.24 "The Taken King: Prelude") - how it is meant to play

Sources (2026-10-02): the activity's corrected object lists (`mission-objects-highway.md`, names
read through per-record pointers from the latest local disc/final 01.24 package chains), its two
subtitle libraries (`0x80cb8d68` revival, `0x80cb91bb` mission, from
the private catalog's `string-libraries.json`), the objective/description library `0x80cc052f`,
the launch-day walkthrough timeline in `feature-gaps.md`, and today's runs (JOIN54-77).
OBSERVED = seen in content or a run; INFERRED = from names/order, to confirm.

## Areas (slice sets / bubbles / local groups)

| Area | Slice set | Bubble | Local group | Notes |
| --- | --- | --- | --- | --- |
| Revival (car graveyard, cinematic only) | 113 `cosmo_m0_phase00_highway_intro` | - | - | OBSERVED: cinematics `pf_104_res`, `pf_104_res_fp` |
| The Highway (junkyard to the Wall) | 112 | 14 "highway start" | `0xecf574b5` | OBSERVED: spawn ~(383, -962, 12) |
| The Breach (inside the Wall) | 24 (name hash `0x68935472`) | 3 "The Breach" | `0xfc0e45a3` (244 objects) | OBSERVED: client requests 24 at ~(354, -796, 14) once the mission advances (JOIN77) |
| Graveyard / Fallen ships | ? | 2/6/7 (INFERRED) | ? | phase02_graveyard objects in the activity group |
| Hangar (Dock 13) | ? | 13 "Dock 13" | `0x062dd95d` | `o_ship`, `o_hangar_gating_*`, Fallen squads |

Shared groups: the activity group `0x992bb471` (objectives, objective sequences, cinematics,
player triggers, phases) and the sequence group `0x3c2f5ed5` (dialogue `seq_d_oN_xx`, ambush and
music cues, beat trigger volumes `tv_*`).

## The Ghost companion (why the Breach is pitch black)

The Ghost that floats beside the player is an **AI squad the host spawns**, not part of the player:
- Highway: `sq_ghost` (squad, kind 1) with member `sq_ghost__ghost` (kind 2) and performance
  `perf_ghost` (kind 0x28) in bubble 14's group; path objects `o_ghost_ab`, `o_ghost_ab_int`,
  `o_ghost_bc`, `o_ghost_bc_int`, `o_ghost_c` (kind 4; "_int" = interior) and player triggers
  `pt_ab`, `pt_ab_1`, `pt_ab_2` with volumes `tv_ab`, `tv_ab_1`, `tv_ab_2` - the Ghost moves
  between waypoints a -> b -> c as the player advances (INFERRED from names).
- The Breach: `sq_nux_ghost` / `sq_nux_ghost__ghost` ("nux" = new-user experience) are
  **kind 1/index 94** and **kind 2/index 95** in `0xfc0e45a3`. The player triggers
  `pt_nux_ghost_light_up_start` and `pt_nux_ghost_light_up_continue` are kind `0x1d` at
  indices **120 and 121**. The group also contains
  `pm_nux_ghost_light_up_fly_to_gun` (+ volumes `tv_nux_ghost_light_up_*`), `ho_ghostlight`,
  `of_ghostlight`, `o_device_invis_light`, and `tv_flashlight` in the sequence group. This is the
  torch: on entering the dark Breach the Ghost lights up ("We need more light. I'll see what I can
  do."), then flies to the rifle.
- **Name correction:** the old printable-string scan shifted The Breach's names by six
  positions. Kind 5/index 100 is `seq_breach_fallen_drop`; kind `0x1d`/index 107 is
  `pt_fallen_3_spawn`. Neither is a Ghost object. `ho_ghostlight` is kind `0x18`/index 101,
  `pm_nux_ghost_light_up_fly_to_gun` is kind `0x1c`/index 123, `of_ghostlight` is kind
  `0x21`/index 124, and performance `nux_ghost` is kind `0x28`/index 125. Their exact state
  behavior is still to trace. IDs and baseline lists were unaffected by the name-decoding bug.
- Status updated 4 October: the squad gate `0x20f858` needs a fresh inactive bubble
  reset after the slice transition; see [reset evidence](simulation-bubble-reset.md).
  A generation alone leaves the Breach Ghost empty. JOIN99 establishes one physical
  actor from an explicit desired count, and JOIN101 repeats that result from the
  shipped mission rule after a manual protocol reset. The corrected squad-94 filter
  reaches configured helper attachment creation and registration. Visible torch,
  Ghost movement/dialogue and Fallen encounter counts remain open. See
  [squad counts](squad-member-counts-0124.md) and [light evidence](ghost-light-0124.md).

## Beat by beat

Dialogue = subtitle text (`0x80cb91bb` unless noted); objective titles/descriptions from
`0x80cc052f`. Objective state: 1 = incoming (banner, needs its incoming sequence at step 1),
5 = active on the HUD, 3 = completed; clear (0) before the next starts (JOIN60-70).

1. **Revival (113).** Ghost finds the body (`pf_104_res`), then first person (`pf_104_res_fp`):
   "Guardian... Guardian? Eyes up Guardian!", "It worked... You're alive!", "You don't know how
   long I've been looking for you.", "I'm a Ghost. Actually, now I'm your Ghost...", "Hold still.",
   "This is Fallen territory. We aren't safe here.", "I have to get you to the City.", "Don't
   worry, I'm still with you. We need to move, fast." (`0x80cb8d68`). Host: play both cinematics,
   stop them, teleport 113 -> 112. **PARTIAL**: playback and scripted handoff observed
   (JOIN53+), but complete shots, visible Ghost, cast and dialogue are not accepted.
   The user reports missing shots/Ghost; see the [reference review](guardian-rises-reference-review.md).
2. **Enter the Wall (112).** "We won't survive long out in the open like this. Let's get inside
   the Wall." Objective 3 "Enter the Wall" / "Take refuge inside the ancient wall."; incoming
   sequence `0x5/4`; waypoint on the Wall. Nag lines while the player dawdles (likely
   `seq_d_o1_20b/c/d`, `tv_toward_wall`): "I know this is confusing. But we need to get inside.",
   "We need to get through that Wall.", "I need you to trust me on this. We'll be safer inside.",
   "Would it help if I told you we'll have more fun inside?", "Those are cars. Trust me. They don't
   work.", "There's nothing out here but your own death... again.", "Well, if you'd rather walk
   back to your death...", "You can hear me right? We are going to die out here.", "I know I'm
   just a voice in your head, but you really should listen to me.", "I didn't bring you back just
   for you to die again, we have to move." The Ghost squad (`sq_ghost`) leads the way.
   Host: objective/sequence **WORKING** (JOIN62); Ghost squad, waypoint, nag dialogue NOT YET.
   The player trigger `0x1d/30` reports in a **type-18** message, which arrived only after the
   Breach loaded (JOIN77) - see below.
3. **Find a Weapon (Breach, dark).** Area title "THE BREACH". "Ok... I need to find you a weapon
   before the Fallen find us." Objective 7 "Find a Weapon" / "Search the wall for a weapon to
   protect you from the Fallen." "Hang tight. Fallen thrive in the dark, we won't. We need more
   light. I'll see what I can do." -> the Ghost lights up (`sq_nux_ghost`, light-up points,
   `tv_flashlight`); "Another one of these hardened military systems... And a few centuries of
   entropy working against me." (the Ghost powers the doors/lights: `o_breach_control`,
   `o_breach_doors_lights`, `o_breach_gating_lights`, cinematic `pf_104a_pwr` with trigger
   `0x1d/37`). "What was that?" / "Quiet. They're right above us." (`seq_fallen_above`,
   `tv_fallen_above`). Host: objective 7 **shown** (JOIN70/77); slice switch 112 -> 24 completes
   with a host teleport finish (JOIN77). JOIN101 verifies a physical Ghost from the
   automatic count after a manual reset; visible torch and power cinematic remain open.
4. **Grab the Rifle.** "Here! I found a rifle! Grab it." Objective 11; the rifle is in
   `o_chest_primary_weapon` (Ghost flies to it: `pm_nux_ghost_light_up_fly_to_gun`); pickup ->
   first equipped weapon, ammo counter. "I hope you know how to use that thing." Host: objective
   display NOT YET confirmed; weapon grant (loot path) NOT YET.
5. **Fight through the Wall.** Objective 14. First Fallen: `sq_fallen_first_vandal`,
   `seq_fallen_intro_vandal_drop`, then drop-down/wall/floor squads (`sq_fallen_*` with spawn
   triggers `pt_fallen_*_spawn` / `tv_fallen_*_spawn`, firing areas `fa_*`, squad groups `sg_*`),
   tripmines (`o_fallen_tripwire_1..7`, `pt_tripmines`: "Tripmines! Don't touch them."), loot
   cache (`tv_lootcache`: "A loot cache. Let's see what's inside."), ambushes
   (`seq_ambush_hallway/garage/finalroom`), garage squads and nests (`sq_fallen_garage_*`,
   `sq_fallen_nest1/2` with crawlers/droppers/shanks). Lines: "They're coming for us!",
   "Incoming!", "Watch for motion on the tracker.", "Eyes forward. Watch your tracker.", "There's
   more ahead! Keep it up!", "Move!". Mission beats: `pt_mission01_complete`,
   `pt_mission02_complete` (`tv_mission0x_complete`). Host: squads (AI spawning) NOT YET.
6. **Fight through the Fallen (graveyard).** Objective 17. "Fallen Ships! This close to the
   surface?!", "The Fallen have a tighter hold on this place than I thought. Just a little bit
   further. Let's hope there's something left out there..." (`tv_trans_graveyard`,
   `tv_graveyard_reveal`, `tv_graveyard_navmode`, `ap_fallen_graveyard_scream`).
7. **Find the Ship (hangar, Dock 13).** Objectives 20 / 23. "This was an old Cosmodrome. There's
   got to be something we can fly out of here.", "I'm picking up signs of an old jumpship. Could be
   our ticket out of here." (`tv_hangar1/2`, `tv_ghost_scan`).
8. **Clear out the Fallen.** Objective 26. "There's a ship! Clear them out!" (Dock 13 squads
   `sq_m0_fallen_*`, `sq_ch1_fallen_on_playership_1..3`).
9. **Escape.** "Alright, let me see if I can get us out of here." -> outro cinematic `pf_105_cto`
   (`cosmo_m0_phase04_outro`), mission complete, rewards, orbit.

## Gating observed in the runs

- JOIN76/77: walking past ~(354, -796, z 14) inside the Wall gives an in-world **LOADING**
  ("ATTENTION - Contacting Destiny Servers...") until the host advances the mission (Enter the
  Wall complete, Find a Weapon started); then the client requests slice set 24 by itself. Bubble
  states were not the cause (all 22 set to 0 changed nothing).
- The client's switch request then sat at record state 4 until the host published a teleport
  request for 24 (state 1 -> state 3 finish), as in the 113 -> 112 hand-off.
- The Enter the Wall player trigger reported (type 18, carrying `0x992bb471/0x1d/30`) only after
  that - the trigger's report and the gating are linked; the exact order is to trace.

## What the host has to do (implementation order)

1. Advance the mission on the player trigger / gating (done by hand in JOIN77; automate: the
   gating should complete objective 3 and start objective 7 when the player reaches the Wall).
2. Finish client-initiated slice switches (send the teleport request/finish for the requested
   set) - stand-in change.
3. **Squads**: decode kind-1 squad state (spawn, member count, generation) -> Ghost companion
   (`sq_ghost`, `sq_nux_ghost`) and Fallen squads; AI then runs on the client (INFERRED: the
   Breach squads' members are kind-2 objects with their own state).
4. Ghost light / power sequence (`pf_104a_pwr`, light objects), rifle chest and weapon grant.
5. Later areas (graveyard, hangar), outro, completion and rewards.
