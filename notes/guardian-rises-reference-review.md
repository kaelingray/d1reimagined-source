# A Guardian Rises: reference comparison — 8 October 2026

The user's PS4 walkthrough is the intended visible behavior reference. The first
review sampled selected timestamps and was presented too broadly. It missed the
absent objective waypoints and did not establish complete revival shots or the
Ghost's presence. The user identified those omissions. Revival is now PARTIAL;
objective text, object-state receipt or a cinematic-start callback cannot accept
the visible sequence.

The local reference is 25:21.32, 640×360 H.264 with stereo audio, SHA-256
`8fabe0c51307d4e5f945665505a47c3cf8cc51dadf9905dcacc2c9f3bd5149ae`.
Review now covers 305 exact-seek stills at five-second intervals over the full
duration, with closer samples around the revival and power/rifle cues. This is
visual sampling, not every-frame playback or a complete audio/dialogue review.
Times below are reference-video positions, not native-test timestamps or mission
clock constants. Sequence boundaries are approximate. The reference includes
editing and older PS4 UI: its numeric reward/defence values are not assumed to be
01.24 constants. Original media and extracted stills remain private.

| Reference time | Visible behavior required | Current 01.24 gap / acceptance rule |
| --- | --- | --- |
| 00:40–01:30 | Titan, Warlock and Hunter previews | Does not demonstrate every character-creation step; all classes still require native checks. |
| 01:35–04:25 | Logos, Mars/account introduction and Traveler backstory | First-view versus later-view intro flags must be independent of per-character revival. |
| 04:30–06:05 | Cosmodrome panorama, car graveyard, searching Ghost, scan POV, watching Fallen and resurrection energy | Verify all authored shots and actors. Current first cinematic is stopped after 70 seconds of host time; the longer reference sequence motivates investigation, not a guessed duration replacement. |
| 06:10–06:55 | First-person awakening, visible hovering Ghost, Guardian hand, warning, Ghost departure and fade | Missing Ghost/shots remain reported defects. Cast, effects, dialogue and fade removal require continuous native evidence. |
| 07:00–07:35 | Unarmed control, Enter the Wall, a world diamond on the Wall, then Find a Weapon on entry | Titles have partial evidence; normal-view and Nav Mode waypoints do not. Pre-granted fixture weapons cannot accept the authentic opening. |
| 07:40–08:15 | The Breach, camera-following spotlight, a world diamond, Ghost moving ahead and powering the light | Ghost member receipt is not visibility. The power performance is separate from the moving torch; both need native checks. |
| 08:19–08:25 | Grab the Rifle; Ghost by the rifle box; Khvostov pickup and armed ammo HUD; Fight through the Wall with marker | Grant once, equip/save, ammo/reload/fire, correct marker and reconnect/replay behavior are unaccepted. |
| 08:40–11:50 | Dreg combat, Nav Mode Ghost/objective, loot cache and special weapon, tripmine beams and crouch tutorial | Enemy presence, AI, damage/death, real rewards, weapon switch and hazards must work rather than only advancing the title. |
| 12:05–14:20 | The Divide, Fallen flyover, Fight through the Fallen markers and combat | Continuous streaming, flyover, markers and encounter progression remain open. |
| 14:25–15:30 | Level-up, character/armour/subclass menus, Find the Ship, ability upgrade and route diamond | A sampled armoured Titan is bounded appearance evidence; actual XP, unlocks and durable progression are still required. |
| 16:00–17:10 | Rahn, Devil Captain; Clear out the Fallen; adds, damage/shields and encounter-clear Ghost cue | Authenticated boss/add completion, abilities and the correct objective transition remain open. |
| 17:20–18:35 | Guardian and Ghost ship outro, repair, movement, escape and Fallen firing | Accept full cast/animation/dialogue and shots; playing the cinematic object alone is insufficient. |
| 18:45–19:10 | Advancement, activity rewards, mission summary and Arcadia ship award | Durable completion and rewards must occur once with correct statistics. |
| 19:20–20:35 | Tower flight and arrival cinematic with Guardian, Ghost and populated courtyard | Arrival, cast and intact rendering require native evidence. |
| 20:40–22:30 | Welcome, Titan markers to Zavala, vendor reward, Equip New Armor and character/subclass view | Vendor markers, class-specific reward, equip acknowledgement and persistence remain open. Optional Postmaster/Hunter-vendor visits are shown too. |
| 22:35–23:20 | Gunsmith marker, Arm Yourself choice, weapon award | Correct class/level choices, grant and equip must be durable; copied reference numbers are insufficient. |
| 23:25–24:35 | Shipwright marker, Amanda's Repairs Complete reward, ship inventory and Go to Orbit | Ship award/ownership and tutorial transitions must be idempotent. |
| 24:40–25:21 | Orbit, Director, Restoration and Tower destinations | Unlock/selection state and reconnect persistence remain open. |

## 01.24 marker investigation

The checked ELF is SHA-256
`e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206`.
Actual objective apply `0x3a68c0` copies the 0x2f0-byte state to component+0x110.
State+0x40 contains ten fixed 0x40-byte marker entries (`0x8080293c`), not a counted
list. Apply copies the root reference at state+4 into each entry+4, chooses a
style with state+0xc through `0x3a67ac`, and calls `0x57a7d8` for every entry.

Registration at `0x57a7d8` requires both nonzero entry type at entry+0 and a
non-null object reference at entry+0x20. Helper `0x155f60` checks kind/index
sentinels; it does not resolve the reference or prove its target is loaded.
The current mission sets neither marker entry. JOIN62/63 already failed tests
that filled entry+4 or root+4 alone; repeating those is not a waypoint repair.
Correct target, style and normal/Nav Mode display still need a bounded native
test before a marker candidate can be promoted.

The cinematic state has separate counted machine-ID lists at +0x18 (up to16)
and +0x9c (up to6). Their presence does not establish Ghost casting. The script
currently fills the local player at +0x9c and uses fixed stop timers. Resolve the
authored performance/cast and completion behavior before changing the automatic
mission. Neither longer timers nor an arbitrary Ghost ID is an accepted fix.

See [mission acceptance](guardian-rises-acceptance.md),
[mission walkthrough](mission-a-guardian-rises.md) and
[8 October checkpoint](checkpoint-2026-10-08-mission.md).
