# Public research applied to the activity-host stand-in (2026-09-27)

Public material on how Destiny's activity hosts, bubbles and packages work, mapped onto what the
PS3 runs have shown so far (orbit checkpoint, TEST194-366). Each mapping ends in a prediction the
next runs can check. Nothing here was tested against the game; these are leads, and the
checkpoint notes stay the record of what the client actually does.

## Sources

- Justin Truman (Bungie), "Shared World Shooter: Destiny's Networked Mission Architecture",
  GDC 2015: [slides with speaker notes (PDF)](https://media.gdcvault.com/gdc2015/presentations/Truman_Justin_Shared_World_Shooter.pdf),
  [GDC Vault](https://gdcvault.com/play/1022247/Shared-World-Shooter-Destiny-s),
  [video](https://www.youtube.com/watch?v=Iryq1WA3bzw). Slide numbers below refer to the PDF.
- David Aldridge (Bungie), "I Shot You First: Networking the Gameplay of Halo: Reach", GDC 2011
  ([GDC Vault](https://www.gdcvault.com/play/1014345/I-Shot-You-First-Networking)). Destiny
  started from this code base and its PvP networking model (Truman, slides 19-25).
- [MontagueM/Charm](https://github.com/MontagueM/Charm) `Tiger/Schema/Activity/ActivityStructsROI.cs`
  and `Tiger/Schema/Static/StaticMapData.cs`: Destiny 1 (Rise of Iron, PS4) tag schemas for
  activities, bubbles and map data.
- [v4nguard/tiger-pkg](https://github.com/v4nguard/tiger-pkg) `src/d1_legacy`: the big-endian
  (PS3/X360) v24 package reader.
- [lizardpeter/destiny-1](https://github.com/lizardpeter/destiny-1): a PS4-based D1 package and
  executable study (`spec/D1_TIGER_PACKAGE_v24.md`, activity/bubble graph notes, a PS4 EBOOT
  string pass). PS4 Rise of Iron, so layouts and class ids need checking against the 2014 PS3
  build.

## Destiny's activity model (Truman 2015) against the stand-in

| Talk (slide) | What the PS3 runs show | Consequence for the stand-in |
| --- | --- | --- |
| Activity hosts are cloud machines that run a stripped-down simulation of only the mission script logic, ticked at 10 Hz (8, 87-88). **"None of our Lua logic for Activity Scripts are executing on the client"** (80). | The client holds the script's named objects (`obj_m0_phase00_m1`, `tv_in_wall`, `pf_104_res`, ...) but nothing on the client advances them; every objective sits at state 0 (TEST262, TEST363). | The mission logic of A Guardian Rises is not in the client to find. The stand-in has to play the script itself: step objective sensors, place squads, start cinematics, in the order the mission runs. The objective list in "Expected mission flow" is the script to reproduce. |
| Activity state is a minimal set of **sensors** declared up front by each activity; the activity host is authoritative (**auth state**, host -> client); the physics host reports changes up (**sense state**, client -> host), at 10 Hz, atomically per update (73-86). | The replicated objects are sent host -> client in type-5 object streams; kind 3 is literally the "objective sensor" (`0x1f6aa4`, string "objective sensor client update state"). | The replicated kinds are sensor types. What the client reports back is sense state, and the script advances on it. |
| A squad sensor holds "spawned? how many alive? firing area", not the AI; `place()` sends auth state and the physics host spawns the AI; kills are reported by sense state (79-84, 92). | Kind 4 is a spawn slot with a generation counter: a higher u32 makes the client destroy and recreate the tracked entity (TEST363). Kinds 0x20/0x27 sit in the player spawner. | Kind 4 is probably the auth half of a squad/spawner sensor: raising the counter is `place()`. Its sense half (alive count) should come back from the client once something has spawned. |
| The physics host is one of the peer consoles; the activity host elects it and on election sends a **Reconcile** of all sensor state, which the new physics host must apply exactly (48-52, 89-94). Entering an empty activity bubble makes that player the physics host, who then gets reconcile() (94). | The client applies no replicated state until every record has a baseline (`0x1ce1b8`), and then all at once. In a solo run the client owns simulation for group peer 0 (world-state group record, TEST253). | The baseline rule is the reconcile contract. The solo client is the physics host, so it is the one that evaluates trigger volumes and reports them. |
| Mission hosts may not create or change networked objects in public bubbles, but may set objectives, play dialogue and fullscreen FX for the fireteam anywhere (104-107). One mission host owns all private bubbles of the destination for a fireteam (100-101). | One stand-in plays both roles. The mission's objects are split into an activity group (0x992bb471 = "act_cosmo_m0_highway"), per-bubble groups (0xecf574b5 for bubble 14, 0x062dd95d for bubble 13) and a game-mode group (0x4786c0e0). | The per-bubble groups probably belong to whichever host owns that bubble (the bubble host for a public bubble, the mission host for a private one). The activity group follows the fireteam across bubbles. |
| Bubbles are the unit of simulation, streaming and matchmaking; at most two bubbles' assets are resident (current + precache). Transition along a **z-leg**: precaching starts 15 s out; for public bubbles matchmaking for a new host starts at 10 s and is abandoned at 5 s, when the player spins up a standalone bubble (32-38). | Transition kinds in the EBOOT: `transitioning:initial_slice_set`, `normal_z_leg`, `debug_initiated`, `teleportation`. A second world-state delta from the host is not acted on (TEST350), and in the world type 11 no longer reaches its handler (TEST366). | Bubble changes are probably driven by the client walking into a z-leg (`normal_z_leg`), not pushed by the host. The host then acknowledges, or for a public bubble matchmaking restarts (Demonware `createSession` again, the **currant**/**cloudberry** gates). Test below. |
| Host timeouts are lenient: 15-20 s without packets before the old host is dropped (58). | **baboon** after a 90 s heartbeat gap (TEST361); 1 s heartbeats hold (TEST363). | Consistent; the client-side threshold is config +0x13f8 + +0x2d10 (orbit checkpoint). |
| A separate WorldServer persists characters and progression (130). | Services 501/504, the character store and the BAP stand-in. | Unchanged. |
| Halo: Reach networking: **state data** (eventual consistency, only the latest value matters), **events** (may be dropped) and **control data** (input stream); per-object priority, no delivery guarantee (Aldridge 2011). | The type-5 object stream carries a baseline, then state. | Resending an object's full current state is always legal, so the stand-in can re-send state whenever it is unsure. |

### Predictions and tests

1. **Sense state arrives only for armed sensors.** In TEST253 the client sent only type 15
   (1 byte, ~30/s) and setup messages (3, 6, 17, 18, 20, 21, 36, 37), and its movement was not
   sent. That fits the model: movement is P2P traffic, and sense state is sent only when a sensor
   changes. With every objective at state 0, no trigger volume is armed, so nothing reports. Test:
   step the highway start's kind-3 objective sensor to 1 (the "incoming" phase of
   `obj_m0_phase00_m1`, `--ah-object 0xecf574b5:3:<index>:0x808004fa=...`), then walk the guardian
   to the break in the Wall (`tv_ab`/`tv_in_wall`) and log every client -> host type. A new type,
   or a type-15 body other than 00/0x41, would be the first sense-state message. Bubble 14 has no
   kind-3 object of its own (TEST342), so the sensor may be the activity-group one; list kind 3
   across all groups first. `scripts/ah_log_summary.py <NAME>-ahl.log` lists each new client
   message type/body once, in time order next to the stand-in's pushes.
2. **Transitions start on the client.** From the highway start (bubble 14) with bubble 3
   registered, walk through the Wall into The Breach instead of sending a zone schedule. Hook the
   slice-set loader `0x13f8ec`, the progress poster `0x3204a8` and the notification entry
   `0x42a498`, and watch the lobby log for a new `createSession`. Expectation: precaching (package
   opens) about 15 s before the swap, a client -> host message naming the target bubble, and no
   new world state needed from the host, or a lobby request if the next bubble is public.
3. **The "Change slice-set" formatter.** The PS4 build has
   `Change slice-set (slice-set='%d', spawn='%s').` and
   `Change world (activity_name='%s', symbol='%s', rebuild_omaha_options='%s', random_seed='%c 0x%08X').`
   (lizardpeter/destiny-1, `notes/D1_EBOOT_EXECUTABLE_MAP_SPAWN_FIRST_PASS_2026-09-27.md`),
   next to `payload spawn identifier unavailable in this build`. If the PS3 EBOOT has the same
   strings, the function that formats them sits on the slice-set change path. Hooking it (PPU
   hook, r3-r6 dumps) during a transition names its caller, and so the message or state that
   starts a switch. It is the most direct follow-up to TEST366/367.
4. **Kind 4 = squad placement.** Step one bubble-14 kind-4 object from 0 to 1 while the guardian
   stands in the highway start. Expectation: an entity (a Fallen squad, or a scripted prop)
   appears at a fixed spot, and killing it produces client -> host sense state.

## Package data: where activities, bubbles and slice sets live

Charm's D1 schemas (Rise of Iron, PS4) give the tag chain from an activity to its world data.
Class ids are written as u32 values; Charm writes them byte-reversed (`2E058080` = 0x8080052e):

```text
named tag (package named-tag table, 68-byte rows: tag hash, class hash, char[60] name)
  -> SActivity 0x8080052e { +0x08 Tag<S36068080> names, +0x10 DynamicArray<S0A418080> Bubbles }
       S0A418080 0x8080410a { Tag<SBubbleDefinition> }
  -> SBubbleDefinition 0x808091e0 { +0x08 DynamicArray<SMapContainerEntry 0x80800767> MapResources }
  -> SMapContainer { +0x18 DynamicArray<SMapDataTableEntry 0x80804109> MapDataTables }
  -> SMapDataTable { DynamicArray<SMapDataEntry 0x80800406, 0x90 bytes> }  (placements, entities)
S36068080 0x80800636: activity dev name, BubbleNames[] (SDB068080 0x808006db: bubble name
  string hash, BubbleIndex into Bubbles[])
SUnkActivity 0x80800616: location/destination names, dev name, +0x58 phases (S3F078080: string
  hash, int)
S93298080 0x80802993: dev name +0x60, Directives[] (SD7318080 0x808031d7: description,
  objective, objective2 string hashes) - objective texts
SD9128080 0x808012d9: "sq_machine"-style scripted groups: +0x20 typed member lists pointing at
  SMapDataEntry records, +0x30 Locations[] (position + rotation)
```

Leads for the stand-in:

- **Slice sets = a bubble's map containers.** A bubble lists one or more map containers. The
  runs number zone (slice) sets 8 x bubble + n, and bubbles 13 and 14 each have one extra set
  (105 for the Dock 13 outro, 113 for the intro). That fits "map container n of bubble b" =
  slice set 8b + n. Check: read the activity's Bubbles[] and the container count per bubble.
- **Squads and locations are data.** The scripted groups (`SD9128080`) carry the member entity
  names and spawn locations the stand-in would otherwise have to guess. They probably back the
  kind-4 spawn slots (28 in the highway start, 20 in The Breach, 5 in Dock 13).
- **Objective texts are data.** Directives carry the "Enter the Wall" style strings. Mapping
  them to kind-3 entry indices would give the order in which the stand-in steps objectives.
- **Class ids may carry over.** The PS3 runtime type registry (`scripts/rpcs3_tiger_types.py`)
  can check directly whether 0x8080052e, 0x808091e0, 0x80800767, 0x80800636 and 0x808012d9
  exist in the 2014 build with Charm's sizes (0x28, 0x18, 0x4, 0x38, 0x58). If they match, the
  schemas apply unchanged.

### Reading the PS3 packages offline

`scripts/d1_pkg.py` (new) reads the disc packages without the game: header, entries, blocks,
named tags, tag hash -> package/entry, SHA-1 checks and extraction. First steps on the PC:

```powershell
python scripts\d1_pkg.py named --grep cosmo        # activity/bubble roots by name, no Oodle needed
python scripts\d1_pkg.py info local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages\<pkg>.pkg
python scripts\d1_pkg.py blocks <pkg> --verify     # confirms the layout (block SHA-1s)
```

Compressed blocks (flag 0x100) need an Oodle 2 decoder. `info` prints the decoder id from the
first compressed block's header. If it is one the open-source
[ooz](https://github.com/powzix/ooz) implements (LZNA, Kraken, Mermaid/Selkie, BitKnit,
Leviathan), no proprietary DLL is needed; otherwise `--oodle` takes a user-owned
`oo2core_3_win64.dll` (tiger-pkg's requirement for D1). Never download an unverified copy or
commit one.

## Other references checked

- [cra0kalo/DestinyPKGTool](https://github.com/cra0kalo/DestinyPKGTool): an early C# X360/PS3
  extractor, superseded by tiger-pkg.
- [nblockbuster/bungie-lua-decompiler](https://github.com/nblockbuster/bungie-lua-decompiler):
  Bungie Lua (format 14) from a D1 internal alpha, whose packages still shipped scripts.
  Retail activity scripts run on the activity host (Truman, slide 80), so retail client packages
  probably do not contain them. Once blocks can be decompressed, `d1_pkg.py scan --hex 1b4c7561`
  (the Lua signature) settles it.
- FNV-1/FNV-1a of about 13,000 candidate names (mission, phase, area and generic names) matched
  none of the unknown group keys 0xecf574b5, 0x062dd95d, 0x3c2f5ed5 and 0x4786c0e0; only
  0x992bb471 = FNV-1("act_cosmo_m0_highway") reproduces. The named-tag table and the bubble-name
  string hashes (S36068080) are the next candidate sources.
