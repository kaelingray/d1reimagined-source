# A Guardian Rises — beat-by-beat target vs. 01.24 restoration (8 October 2026)

Source: the user's private PS4 walkthrough (25:21, SHA-256 `8fabe0c5…5149ae`), reviewed
again on 8 October at one frame per second for 04:25–08:30 and every two seconds for
08:25–19:35. Times are reference-video positions. Dialogue is paraphrased; the original
media and stills stay private. "Ours" is the best native evidence from JOIN186–190 on the
01.24 build; it is the target for a 1:1 restoration, not a claim of completion.

Legend: **OK** matches in the tested scope, **PARTIAL**, **MISSING**, **NOT REACHED**.

## Revival (slice 113, third-person `pf_104_res`, first-person `pf_104_res_fp`)

| Ref | What the original shows | Ours | Status |
| --- | --- | --- | --- |
| 04:25–04:50 | Fade in, "COSMODROME" location card, sunrise panorama, tree, bridge | Vista and tree shots present (JOIN188 09:51:42–09:52:15 UTC) | PARTIAL: location card not confirmed |
| 04:50–05:25 | Car graveyard, towers, pull past the overturned truck, overhead shot of the cars | Same shots in order | OK (visual sample) |
| 05:27–05:35 | Wrecked car; **the Ghost flies in through the window and turns to camera** | JOIN201: the Generalist Shell crosses the truck shot, passes through the cars, enters the car window and turns to camera | OK (sampled at 2 fps) |
| 05:37–05:40 | Ghost's scan POV (blue rings, green tint) | Present | OK |
| 05:41–05:48 | Fallen captain and dregs on the ridge | Present | OK |
| 05:49–05:56 | Low shot of the Wall, Ghost voice ("is it possible?") | Wall shots present | PARTIAL: audio not reviewed |
| 05:57–06:03 | **Ghost resurrection close-up: shell opens, blue light sphere** | JOIN202: the shell opens into the blue sphere in front of the Wall (JOIN201 exited here once; JOIN202 did not) | OK (sampled); intermittent exit open |
| 06:05–06:17 | Black, eyes opening, "Guardian… eyes up" | ~20 s black/very dark | PARTIAL: much darker/longer |
| 06:19–06:45 | Bright daylight first person; **Ghost hovers centre and talks**, Guardian's hands rise, warning about Fallen territory, Ghost flies off toward the Wall | JOIN202: the Ghost hovers centre and the hands rise, after a one-frame normals-buffer flash; the scene is much darker than the reference | PARTIAL (lighting) |
| 06:47–07:00 | Control; Ghost talks; "Enter the Wall" objective and Wall diamond | Control, objective and diamond (JOIN182/188) | PARTIAL: objective text/Nav Mode open |

The host now hands over on the client's cinematic-end incidents (JOIN186–188), so timing
follows the game. The remaining revival defects are visual: the Ghost is absent and the
first-person part is too dark. JOIN194–199 subsequently distinguish a valid base
unit/cache from an optional cosmetic override and still show malformed geometry.
The earlier claim that daylight absence ruled out a rendering/binding fault was
too strong; see [the corrected diagnosis](ghost-appearance-0124.md).

## Highway to the Wall (slice 112)

| Ref | Original | Ours | Status |
| --- | --- | --- | --- |
| 07:00–07:20 | Run through the cars to the Wall, through the gap and up the ramp to the door | Guarded route walk (automated input) | OK (route) |
| 07:20–07:27 | Door with fallen panel, stairs inside | Same | OK |
| 07:27–07:33 | Ghost: "need to find you a weapon before the Fallen find us"; objective becomes **Find a Weapon** | Objective handover plus an extra "NEW MISSION / The Taken King: Prelude" banner (JOIN202) that the reference does not show; `seq_d_o2_10` plays (JOIN190 audio) | PARTIAL: remove the extra banner; line unconfirmed (no subtitles) |
| 07:31–07:35 | Lit corridor with windows, diamond ahead | Same corridor, lit (JOIN188/190) | OK |

## The Breach (slice 24)

| Ref | Original | Ours | Status |
| --- | --- | --- | --- |
| 07:35–07:37 | Leaving the lit corridor: **a circular torch light follows the camera** | JOIN202: the circular torch follows the camera through the Breach | OK (sampled) |
| 07:38–07:45 | Ghost: "Quiet. They're right above us."; "THE BREACH / OLD RUSSIA, EARTH"; stairs down | Area title appears at the crossing (JOIN190); `pt_fallen_above` reports and `seq_fallen_above` plays (no audible line yet) | PARTIAL |
| 07:45–07:49 | Stairs up, then stairs down, red flare on the floor | JOIN202: guarded walk climbs to the upper platform (z 19.3) and sees the railings with the red flare | PARTIAL (path differs from the reference camera; no dialogue checked) |
| 07:50–07:59 | **Blue Ghost visible ahead**, "Hang tight… we need more light" | JOIN204: the companion Ghost hovers on the platform but renders as a black silhouette with no core glow | PARTIAL (rendering) |
| 08:00–08:10 | Ghost flies up to a panel: "hardened military systems…"; screen goes dark | `pf_104a_pwr` has no cinematic content in any build (ends at once, as in retail). JOIN204: o2_40 "Hang tight" and o2_50 "hardened military systems" play by control; the companion Ghost is a black silhouette by the fuse panel | PARTIAL (dialogue); Ghost/lights MISSING |
| 08:10–08:16 | Lights come on; Ghost glows | — | MISSING |
| 08:17–08:20 | Rifle chest; "Here! I found a rifle! Grab it." Objective **Grab the Rifle** | JOIN203: objective Grab the Rifle shown after Investigate; no chest/dialogue verified | PARTIAL |
| 08:21–08:24 | Khvostov 7G-02 pickup, grimoire card, ammo 25, objective **Fight Through the Wall** | Fixture rifle already held | MISSING (authentic grant) |

## Fight through the Wall → the Divide

| Ref | Original | Status |
| --- | --- | --- |
| 08:25–09:40 | Corridors, first dregs, melee, weapon tutorials ("Push R1 to melee"), loot cache, grimoire | NOT REACHED |
| 09:45–10:05 | Nav Mode Ghost overlay ("THE BREACH"); special weapon (shotgun) drop and swap tutorial | NOT REACHED |
| 10:09–10:30 | Tripmine laser beams, crouch tutorial | NOT REACHED |
| 10:30–11:50 | Garage/nest fights: vandals, shanks, dregs | NOT REACHED |
| 11:53–12:05 | Round door, fan tunnel, Ghost line about Fallen holding the place | NOT REACHED |
| 12:05–12:25 | **THE DIVIDE**, night aurora sky; Fallen skiff flyover and red flare; Ghost about an old Cosmodrome | NOT REACHED |
| 12:27–14:20 | Objective **Fight Through the Fallen**; combat among containers and buildings | NOT REACHED |
| 14:21–15:27 | **LEVEL UP (level 2)**; character screen (Striker, Flashbang Grenade) and ability tutorial | MISSING: ours shows a "Level Up" tip from the start, which is out of order |
| 14:57–15:11 | Objective **Find the Ship**, Nav Mode | NOT REACHED |
| 15:29–16:00 | Hangar approach fights | NOT REACHED |
| 16:00–17:03 | **Clear out the Fallen** at the ship (Rahn/Devil Captain area), grenade tutorial | NOT REACHED |
| 17:05–18:35 | Outro cinematic `pf_105_cto`: Guardian, Ghost and jumpship, launch, escape | NOT REACHED |
| 18:37–19:10 | Advancement, Activity Rewards, Mission Summary (A Guardian Rises), Arcadia jumpship | NOT REACHED |
| 19:15–19:35 | Flight to the Tower (next activity) | NOT REACHED |

## The common blocker: the Ghost (resolved for the revival fly-in in JOIN201)

JOIN201 sends the Generalist Shell's world appearance from its art record (row 0) with
its authored dye 15/2493 instead of the investment value 42, which selects a generic
vehicle-like unit. The authored shell then plays the reference fly-in. The next blocker
is the renderer exit on the resurrection light. See [Ghost appearance](ghost-appearance-0124.md).

Earlier analysis, kept for the record:

The Ghost is missing in every place the reference shows it: the revival shots, the
first-person conversation, the Breach companion and its light. The user's view that
the "torch" is the Ghost fits the evidence: the camera-following light appears where the
Ghost should be accompanying the player, and the visible blue Ghost appears seconds later.
JOIN190 shows that `tv_flashlight` only gates a dialogue line.

JOIN192 compares the daylight car shot frame for frame: the camera move is identical
and the reference Ghost crosses the window over ~5 s, while ours has no Ghost at all.
The cinematic Ghost is therefore **not created**, rather than drawn black. Codex's
performance probe records two player replacement pairs, `(0,16)` and `(0,17)`; the
second is the likely Ghost slot, filled from the local player's own Ghost.

Earlier hypothesis (JOIN192 negative): the cinematic and companion Ghost
render with the player's equipped Ghost shell. The 01.24 starter fixture equips Ghost
item 862 in slot 11 **without a model arrangement**, so the client receives −1 and gets
no model art, as the armour did before JOIN118. Next: read item 862's arrangement from
the live gear asset library (JOIN191), supply it, then compare the revival's Ghost shots
and the Breach torch against this script.
