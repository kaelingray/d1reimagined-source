# A Guardian Rises: 01.24 acceptance checks

Updated 2026-10-08. This turns the two user-supplied tutorial
specifications into checks for the PS3 BLES01857 01.24 client. It summarizes their requirements;
it does not copy their walkthrough, dialogue, proposed API, or public-release enemy counts.

**The mission is not verified end to end.** Revival playback, the highway hand-off, objective text,
and walking to the Wall have bounded live evidence. JOIN182 verifies the first normal-view
Wall diamond. Complete revival shots/Ghost, remaining waypoints and Nav Mode, stable Breach progression, Ghost illumination,
weapons, combat, completion, and Tower hand-off remain incomplete.

## Evidence and the next gate

Current checkpoint: JOIN196 repeats natural current24 arrival; JOIN196–199 advance
through all authored revival scenes. The server supports explicit unit
appearance separately from arrangement. JOIN201's starter Generalist (appearance row 0,
dye 15/2493) shows the authored shell in the revival until the resurrection light renders
magenta and the emulator exits; JOIN197–200 reproduce a Nav guest access violation. Stable Breach lighting, torch, later waypoints and combined
graphics remain open. See [Ghost appearance evidence](ghost-appearance-0124.md).

Earlier repeat: JOIN176/177 complete the repaired 113→112 transition on the original
host connection. JOIN177 dismisses the inspected dialogs and consumes guarded
outdoor movement, stopping at its 45-second budget about 73 world units from its
start. The character menu renders a full armoured Titan, but earlier outdoor
black/green shading persists. This is bounded traversal and menu evidence; the
Breach route, combined graphics, progression and combat remain unaccepted.
See the [October 8 checkpoint](checkpoint-2026-10-08-mission.md).

JOIN178 subsequently completes the recorded route, reports the physical Wall
trigger and advances the host objective. Group-switch publication alone leaves
loaded24 inactive. A manual host request activates24 and exposes its lit interior,
but a return112 request follows; no idle24 sample is observed. Recorded-route
arrival within1.5units must not be treated as a verified final zone crossing.
Stable automatic Breach handoff remains open. The saved mission-frame capture
parses, but its replay does not match the lit original; graphics acceptance remains
open as well.

JOIN179 tightens only the route's final arrival radius to0.25 and reaches the
recorded point. The handoff still stalls. Fresh native callbacks observe a
replacement player about30units back after the manual host request; the actual
follow-player controller requests112 again. Bounded ordinary movement reaches
and passes the doorway on that replacement player without sampled idle24.
The next gate must distinguish seamless client-directed streaming from the
forced teleport/respawn experiment. Precise arrival and consumed controls do
not establish automatic Breach progression.

JOIN180 crosses through ordinary pad movement, without a manual host teleport24.
Fresh observations retain the same player, and a separate guarded continuation
traverses the initial recorded Breach corridor. Current24/target112 nevertheless
stays pending, with no idle24 samples; inspected mission scenery is dark. This is
movement progress, not a streaming, waypoint, cinematic, Ghost or graphics fix.
The [reference comparison](guardian-rises-reference-review.md) makes the user's
reported missing waypoints, revival shots and Ghost explicit acceptance failures.

JOIN181 exposes a real wire defect: kind 13 positions consume 96 bits even though
their schema width is zero. The repaired codec preserves the following marker
reference and state fields. Six independent checks fail with the original encoder;
all 175 server checks pass after repair. JOIN182 visibly draws the normal-view Wall
diamond and later reads a cleared objective/disabled marker after the Wall trigger.
The next three authored marker targets are configured, but visibility is unaccepted.
Nav Mode shows No Tracked Items. Current112/target24 remains pending and the final
entrance frame is black. Revival shots, Ghost, streaming and graphics are still open.

- **Observed:** read directly from the 01.24 executable/content, or captured in an identified
  01.24 run. Serialization tests prove a state can be encoded, not that it produces the expected
  gameplay.
- **Reference:** expected retail behavior from the supplied specifications. Their `C` labels do
  not establish the PS3 01.24 protocol, tuning, or this implementation's behavior.
- **Open:** a value, transition, or responsibility still requiring target-build evidence.
- **Proposed:** backend architecture or an acceptance test; not an original wire message.

The remaining Breach gate is **visible Ghost illumination and stable mission progression**.
The host now retains type-21
slice deltas, publishes teleport finish state 3 when the client completes the request,
and echoes the request ID in a subsequent host-address reply. These repairs pass encrypted
session regressions; they do not prove the client can play the Breach. JOIN92 visibly
entered the Breach with ordinary movement, then became black after a host handover.
Fresh-delta arrival, route arrival latching, helper state-B framing and unsafe AI
baseline exclusions are now corrected. The changed mission passes all 156 server tests
and the actual schema preflight. The camera-tool repair passes all 119 script tests.
JOIN97–102 clear all 249 live baselines and show the Breach. JOIN101's ordinary
walk reaches near the spawned Ghost and returns without cleanup; turning toward
the windows restores a clean bright view after dark corridor samples. The historical
count-only rule creates one physical Ghost after a manual reset, and the
manually tested helper reaches attachment registration. JOIN102 resolves all twelve
configured helper components linked to the Ghost. Controlled views toward the Ghost
remain dark while the reverse corridor is lit. A generation-1 state is consumed
without verifying illumination. Visible torch behavior and
stable return-slice completion remain open. See the
[October 4 checkpoint](checkpoint-2026-10-04-mission.md).

JOIN112 repeats the automatic authored-member bindings. It also identifies
the generation callback's queried interface as damage, so its absence on
the Ghost does not establish a torch blocker. The configured attachment's
lighting components and transforms remain the investigation target.
JOIN113 verifies one guarded character-login retry with the first press
withheld, followed by the separately guarded Mars skip and matchmaking.
That minimal host recipe later returns lobster; it is frontend recovery
evidence, not an accepted mission join.

JOIN107/108 show why the count-only Ghost is insufficient: it has a
physical unit but no authored-member or performance binding. JOIN109's
isolated member State A creates exactly one unit with both bindings while
all desired counts remain zero. JOIN111 repeats one unit and both bindings
through the automatic rule after the readiness/reset gate, with no manual
member state. The shipped rule replaces the count-only action with that
member state. The performance cue remains
unset, so this does not verify illumination. The
[October 5 checkpoint](checkpoint-2026-10-05-mission.md) records the live
test and the fresh automatic-rule verification.

In JOIN81/82 the client reported the Enter-the-Wall trigger in
type 18 (`incident`, incident type 2665) about four times per second, and both runs later
disconnected with `pumpkin`. Advancing the objective and disarming the trigger did not
stop that stream; echoing incidents in JOIN83 also failed to prevent a disconnect. Schema decoding
shows JOIN83 contained no incident 2665, so that run did not reproduce the Enter-the-Wall stream.
JOIN81/82's complete incident bodies all differ; their bytes after the leading four-byte word
are constant. JOIN82's incident stream stops about 92 seconds before the activity socket closes.
The reports may be newly generated events rather than retransmissions, and these captures do not
establish the disconnect cause. See [incident diagnostics](incident-diagnostics.md). A passing
test must identify the actual failure condition and
keep the activity connected beyond the previous failure interval while movement and fresh
events continue.

Separately, JOIN79 established that a new highway Ghost squad generation spawns its member
only when bubble authority passes the client gate. Changing the authority bit manually made
the client report the Ghost member; this is diagnostic evidence, not a production mechanism.
JOIN87 and JOIN88 subsequently spawned the highway Ghost through protocol messages alone:
a fresh inactive bubble-14 generation after current slice 112, followed by a squad generation.
The member report took about 70 seconds and 21 seconds respectively. Code
analysis identifies the gate's two masks as transient roster-reset state: changing an inactive
bubble slot's generation sets both, and baseline-ready processing clears only the blocking mask.
Those runs validate the reset masks and generation path. JOIN98 subsequently
found zero live members in the Breach Ghost squad despite an exact member-ID
match in its type-6 stream. The matcher can also find references inside other
states, so these earlier reports alone do not verify a physical spawn.
JOIN99's explicit one-member desired count subsequently creates an active
Ghost actor with a valid physical unit datum; JOIN101/102 repeat physical creation
from the automatic mission rule after a manual bubble reset. See
[squad counts](squad-member-counts-0124.md). The native unit position is known;
visible Ghost behavior, retail ownership and enemy spawning remain open.
Type 27 only writes a separate activity-level value and is not a
demonstrated direct setter of the squad gate.

Current evidence: [mission walkthrough](mission-a-guardian-rises.md),
[director/state analysis](mission-director.md), and the JOIN79-82 section of the
[profile checkpoint](checkpoint-2026-10-01-profile.md). The earlier black-world instructions
in [next-session-playbook.md](next-session-playbook.md) predate the successful revival/walking
runs; do not treat those earlier limits as today's milestone.

## Phase checks

Each row needs both visible client behavior and the corresponding state/traffic evidence.
Later objective labels must not be used to claim that an encounter or item grant worked.

| Phase | Acceptance check | Current 01.24 evidence / remaining gap |
| --- | --- | --- |
| Empty account and creation | An empty roster permits creation. Class, race, presentation, and appearance survive restart. Creation retries return the same character. Three independent slots can be selected without replacing another character. | Service 501 carries creation; saved request data reconstructs the character. SQLite has slot allocation, transactions, and retry tests; BAP still presents its first stored character. Live creation/selection of multiple characters is open. |
| New-character state | Level 1, correct initial subclass and class armor, default Ghost shell; no usable firearm or travel capability. Completion/unlock state represents an unprogressed character. | Final starter kit definitions are verified below. Broad test unlock flags and CLI equipment overlays do not prove authentic initial progression or durable inventory. |
| Appearance and rendering | The same custom Guardian appears correctly in creation, inventory, revival, mission, and later social space; repeated frames show no flicker or distorted lighting. | Selective SPU-written color-buffer reads eliminated sampled setup magenta frames and restored normal inventory Titan lighting. JOIN117 isolates missing armour arrangement indexes in the world profile; JOIN118 supplies them and JOIN119 repeats solid daylight Titan armour with the public fixture and five valid model roots. JOIN123 additionally resolves subclass, Ghost and armour icons and removes their Materials listing through individual item records and fixed inventory ranges. Vulkan device loss still occurs around the character screen; level/light remain zero. Indoor views, other classes, full revival appearance and social space remain to be checked; combined menu/world stability is open. Keep color reads enabled. See [appearance evidence](guardian-appearance-0124.md). |
| Account intro and character revival | Account introduction and each character's resurrection have distinct progression. Revival includes all authored shots, the visible animated Ghost, resurrection effects, correct Guardian/cast, dialogue and first-person awakening, then hands control to an unarmed player without a lasting fade. | Playback and scripted 113 -> 112 handoff observed in JOIN53+; complete revival is PARTIAL. The user reports missing shots and Ghost. The script uses timed start/stop steps, not a verified completion acknowledgement. First versus later character intro flags remain open. See the [timestamped comparison](guardian-rises-reference-review.md). |
| Enter the Wall | Objective/nav guidance appears; normal movement reaches the entrance. The correct trigger advances the mission once and the activity stays connected. | Objective 3 is visible; recorded-route traversal reaches the Wall, reproduced in JOIN88 with extra climb points. Highway Ghost reset/generation and encoded member IDs are observed in JOIN87/88; physical spawn needs separate proof. JOIN90's guarded preload walk reached within 2.51 units of the route endpoint before the local slice changed. JOIN88 posts cleanup reason 0x4d, which maps to three final-build session states; its exact selecting state remains unknown. |
| Find a Weapon / The Breach | Client-requested slice 24 completes at the actual entrance; correct objective, darkness, Ghost torch, path, power sequence, lights, Fallen reaction, and weapon access occur in order. | JOIN97–102 clear all 249 live baselines and show the interior after host handover. JOIN109/111 replace the unbound count-only Ghost with one authored member carrying both member and performance bindings; JOIN111 tests the automatic state after a manual bubble reset. The manual filter/helper previously resolves twelve linked components, but its generation callback queries a damage interface absent on the Ghost; this does not establish a torch blocker. Controlled nearby views and consumed generations still do not verify torch light. Visible torch/power, stable idle transition and automatic trigger-to-switch progression remain open. |
| Grab the Rifle | Correct Khvostov 7G-02 instance is granted once, equipped in Primary, and saved. Armed first-person view, ammo, reload, and firing work. Replay/reconnect cannot duplicate or lose the item. | Rifle container and relevant objective/cinematic/trigger objects are identified. HUD display for objective 11, pickup transaction, item definition selection, and durable equip are not verified. A title change is insufficient. |
| Fight through the Wall | Ordered Fallen encounters run on the expected simulator, with damage, death, radar and ammo behavior. Ghost cues, cache, tripmines, crouching, exit conditions, and ordinary respawn work. | Named squads, cue/trigger objects, firing areas, mines and exit objects are catalogued. Squad authority and spawning are incomplete. Do not invent encounter counts, kill XP, checkpoint transforms, or hazard tuning. |
| First cache | Cache grants the class-appropriate Special weapon and verified currency once to this character; weapon switching works and survives restart. | Retail reference expects Preacher Mk. 20 for Titan/Warlock and Calcutta-LR2 for Hunter. Target-build reward definition, amount, claim message and persistence are open. The reference's 48 Glimmer is not a confirmed 01.24 constant. |
| The Divide | Correct exterior loads; ship arrival/deployment and combat occur; progression advances using the actual encounter/route condition. | Phase02 and associated cue/trigger names exist. Required kills versus optional outdoor combat, slice/group mapping and dropship behavior remain open. |
| Level 2 and Dock 13 | Expected level/XP change and initial grenade node become available and persist. Dock 13 loads and guidance changes toward the ship. Mission can finish without requiring the player to obey the grenade-menu tutorial. | Dock 13 bubble 13/group `0x062dd95d` and ship/Fallen objects are identified. Forced minimum level versus earned XP, grenade-node IDs/activation, and progression transactions need verification. |
| Clear Out the Fallen | Ship is present; correct hangar enemies including Rahn simulate. Shields/health/deaths function; all required encounter conditions, not merely a boss-health check, complete once. | Hangar squad/object names are extracted. Counts, waves, Rahn definition/tuning, kill reports and completion expression are open. |
| Escape and results | Outro plays with the custom Guardian; combat and objective state end; results contain the actual rewards/progression and no transition hangs. | `pf_105_cto`/outro objects are identified. Completion protocol, results payloads and reward commit are open. |
| Tower and relog | Correct City/Tower transition succeeds; introduction quest and Restoration availability match 01.24. Restart keeps inventory, XP, currencies, character appearance and completion; resurrection does not replay for the completed character. | Not yet verified. Ship story recovery must be distinguished from the time the usable ship item enters inventory. |

## Values already established from 01.24

- Tutorial activity index **229**, hash **`0xd03ec63d`**, checksum **`0x8fe07299`**, scenario
  **`act_cosmo_m0_highway`**. Its client banner is **The Taken King: Prelude** in current runs;
  the public mission name A Guardian Rises must not overwrite extracted target-build strings.
  Activity indexes and content hashes are different identifiers.
- Revival uses slice 113; gameplay begins in 112. Breach is slice 24/hash `0x68935472`.
- Objective behavior observed in JOIN60-70: **1 incoming**, incoming sequence **step 1**,
  then **5 active**. Completion uses 3; the outgoing objective must be cleared to 0 before
  the next HUD line shows. The exact completion-side progression effect remains unverified.
- Final starter gear below uses **investment-list indexes**, not assumed item hashes or
  globally unique item-instance IDs. The kit includes no weapon, ship, or Sparrow.

| Class | Initial subclass index | Helmet / gauntlets / chest / legs / class armor indexes | Ghost shell index |
| --- | --- | --- | --- |
| Titan | 710 (Striker) | 204 / 198 / 200 / 206 / 202 | 862 (Generalist) |
| Hunter | 709 (Gunslinger) | 16 / 10 / 12 / 18 / 14 | 862 (Generalist) |
| Warlock | 713 (Voidwalker) | 392 / 386 / 388 / 394 / 390 | 862 (Generalist) |

Use `starter:01.24:CLASS`; the shorter `starter:CLASS` retains historical 01.00 indexes.
The final fixture also supplies the separate verified starter armour model arrangements.
Correct inventory item indexes alone do not guarantee a world model; see
[the arrangement mapping](guardian-appearance-0124.md).
Names were checked with scoped localization libraries; the flattened search-string index can
associate a reused hash with the wrong library. See the starter-kit/catalog section in the
[profile checkpoint](checkpoint-2026-10-01-profile.md) and `STARTER_GEAR_0124` in
`server/bap_server.py`.

## Resolve reference conflicts with target-build evidence

1. **Special weapon:** the second specification's later exclusion row conflicts with both
   specifications' explicit cache reward. Keep the cache as a required investigation/check;
   do not exclude Special weapons. Determine the actual 01.24 reward through definitions and
   pickup traffic rather than selecting an arbitrary low-level item.
2. **Outdoor combat:** one reference says clear the Divide encounter before continuing; the
   other calls those fights optional. Trace the actual exit/phase conditions. Do not add an
   unverified all-enemies-dead gate.
3. **Ship timing:** one reference grants the Arcadia item at mission completion; the other
   separates escape/story recovery from Shipwright registration. Save verified story progress
   and inspect inventory before/after results and Tower onboarding to locate the item grant.
4. **Death:** the no-Darkness-zone requirement conflicts with a proposed full-fireteam hangar
   reset. Use measured ordinary respawn/checkpoint behavior; do not implement a wipe rule from
   that proposal. Tutorial remains solo while independent tutorial instances must coexist.
5. **Settings:** the claim that setup answers are mostly local is contradicted by observed
   service-701 account persistence. Do not discard those writes or reuse another account's
   answers.
6. **Ghost visibility:** highway squad existence does not mean the Ghost should visibly float
   beside the Guardian immediately. Current research says it stays inside the player there;
   Breach illumination is the first necessary visible companion behavior.
7. **Names and wire architecture:** `Activity.Create`, `Pickup.Grant`, `Cinematic.Complete`,
   and similar names in the references describe proposed internal operations. The actual
   observed messages include `incident`, `sensor_auth_update`, `sensor_sense_update`, and
   `client_authoritative_data_update`. Do not send invented envelopes or assume every original
   packet carries the proposed instance/player fields.

## Persistence and isolation acceptance

The SQLite ownership/grant foundation is implemented in `server/player_db.py`; JOIN69 restored
an imported character/account through `--player-db`. This does not establish mission reward
integration. The current BAP path selects the first stored character and CLI gear still uses
test instance IDs; complete roster/selection and database-backed equipment remain work.
XP, currencies, quest/activity completion and checkpoint tables are planned, not delivered by
schema v1. [Player database](player-database.md) and
[multiplayer architecture/debt register](architecture-multiplayer.md) describe that boundary.

Proposed backend tests should use explicit account, character and activity ownership:

- Repeat creation, rifle/cache claims and activity completion with the same event key; get the
  same result and no duplicate item/currency/XP. Restart between commit and reply, then retry.
- Run two new accounts in separate tutorial instances. Advancing one trigger, killing an enemy,
  claiming loot or completing the mission must not alter the other profile or world.
- Give one account multiple characters. Selection, deletion and later creation must preserve
  the others and respect the three-slot limit; retry keys cannot grant to the wrong owner.
- Disconnect after each permanent grant and before completion. Permanent items/progression
  remain; unfinished encounters resume/reset only according to the verified checkpoint model.
- Commit completion and its verified permanent rewards before acknowledging the completed
  transition. A relog must neither lose that state nor replay first-clear rewards.

## Recording a pass

For each phase retain the run name, exact build/emulator revision, character/account and activity
identity, recipe, screenshots, relevant decoded messages, and before/after durable state. Keep
raw content/captures/databases private; publish derived findings and code. Mark memory writes,
manual live-control steps, blanket unlocks and CLI item overlays as diagnostic intervention.
They can establish a mechanism but cannot satisfy the final new-character flow.

Preflight the mission against the 01.24 activity schema before a run. Then verify native
behavior: the preflight alone cannot prove trigger ordering, acknowledgements, AI, lighting,
loot or persistence. Cataloguing an activity/entity group does not establish that it is playable.

Final acceptance requires an empty-profile character to reach Tower through the complete
sequence with no developer intervention, retaining appearance and permanent progression after
client/backend restart, and an independent second tutorial instance to do the same without
crossing state. Until those runs exist, record individual phase passes rather than marking the
whole mission preserved.
