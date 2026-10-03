# A Guardian Rises: 01.24 acceptance checks

Updated 2026-10-02 against repository `64bd695`. This turns the two user-supplied tutorial
specifications into checks for the PS3 BLES01857 01.24 client. It summarizes their requirements;
it does not copy their walkthrough, dialogue, proposed API, or public-release enemy counts.

**The mission is not verified end to end.** Revival, the highway hand-off, objective display,
and walking to the Wall have live evidence. The Breach progression, Ghost illumination,
weapons, combat, completion, and Tower hand-off remain incomplete.

## Evidence and the next gate

- **Observed:** read directly from the 01.24 executable/content, or captured in an identified
  01.24 run. Serialization tests prove a state can be encoded, not that it produces the expected
  gameplay.
- **Reference:** expected retail behavior from the supplied specifications. Their `C` labels do
  not establish the PS3 01.24 protocol, tuning, or this implementation's behavior.
- **Open:** a value, transition, or responsibility still requiring target-build evidence.
- **Proposed:** backend architecture or an acceptance test; not an original wire message.

The immediate gate is **diagnosing the Breach disconnect and repeated incidents**, followed by
establishing squad authority through the protocol. In JOIN81/82 the client reported the Enter-the-Wall trigger in
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
The highway case now has live validation; Breach ownership and enemy spawning remain open.
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
| Appearance and rendering | The same custom Guardian appears correctly in creation, inventory, revival, mission, and later social space; repeated frames show no flicker or distorted lighting. | Selective SPU-written color-buffer reads eliminated sampled setup magenta frames and restored normal Titan lighting. Mission/social-space coverage and other appearance combinations still require captures. Keep color reads enabled as required by the fix. |
| Account intro and character revival | Account introduction and each character's resurrection have distinct progression. Revival plays with the actual Guardian, then hands control to an unarmed player without a lasting fade. | Both revival cinematic objects and automatic 113 -> 112 hand-off work in JOIN53+. The script currently uses timed start/stop steps, not a verified cinematic-completion acknowledgement. First versus later character intro flags are open. |
| Enter the Wall | Objective/nav guidance appears; normal movement reaches the entrance. The correct trigger advances the mission once and the activity stays connected. | Objective 3 is visible; recorded-route traversal reaches the Wall, reproduced in JOIN88 with extra climb points. Highway Ghost spawning works through the protocol in JOIN87/88. The Breach disconnect is the immediate gap: JOIN88 posts cleanup reason 0x4d. This is a final-build session-status mapping; the exact waiting state needs a live sample. |
| Find a Weapon / The Breach | Client-requested slice 24 completes at the actual entrance; correct objective, darkness, Ghost torch, path, power sequence, lights, Fallen reaction, and weapon access occur in order. | Slice 24 hash `0x68935472`, bubble 3 and local group `0xfc0e45a3` are observed. Find-a-Weapon HUD works; host teleport completes the switch after walking there. Ghost light/power and automatic trigger-to-switch progression are open. Teleporting without traversal can leave the player outside the geometry. |
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
