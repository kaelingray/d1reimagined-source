# Preservation plan: from 01.00 to the final PS3 build (research session 2026-09-28)

This plan builds on the repository as it stands (README, orbit checkpoint, test log,
`mission-director.md`, `feature-gaps.md`, `tower.md`, `investment-services.md`). It does not
repeat what those already document.

Evidence labels:
- **CONFIRMED**: seen in a run, in the client's own data, or in Sony/Bungie primary sources.
- **LIKELY**: strong static evidence, not yet run.
- **UNKNOWN**: open; each comes with an experiment.

---

## 1. Assessment of the current implementation

**What works (CONFIRMED, README gate table).** Every step from the title screen to a spawned
guardian at the highway start runs on loopback stand-ins:
- RPCN;
- DNS sinkhole;
- STUN;
- SSLv3 sign-on with a patched CA;
- auth3;
- the Demonware lobby and matchmaking;
- BAP/investment, with the character created and selected;
- the activity host: join, entity indices, activity state, zone record, heartbeats, world-state
  deltas, type-5 simulation packets with baselines, the simulation player with its spawn grant,
  the game mode.

RPCS3 carries two Destiny fixes: SPU DMA write tracking for video memory, and descriptor-pool
reclamation. There is also an instrumented build (PPU function-entry hooks via `d1\d1ppu.txt`).

**Offline tooling, a strength.** Everything below runs from the disc data and the EBOOT alone:
- `d1_pkg.py`: packages (Oodle via a contained loader), patch chains, bubbles, replicated groups,
  strings, the item list;
- `d1_types.py`: all 4,387 runtime types and the 66 web services;
- `d1_eboot.py` / `d1_addresses.py`: disassembly, xrefs and labels.

Most protocol structure can now be read without booting the game.

**Weak points:**
- `server/bap_server.py` is one module holding transport, services, investment objects, the
  activity host and experiments. It has about 70 CLI flags, and runs are composed from long
  PowerShell argument lists. That suited discovery, but it will not scale to many activities or
  builds.
- State lives in memory plus a JSON character store (class and appearance only). There is no
  inventory, progression or story persistence.
- Everything is tied to 01.00: type handles, service wires, offsets and addresses are constants
  in code and notes.
- Mission logic is scripted by hand from decoded fields (`missions/*.json`). Good as data, but
  many field meanings are still static-analysis guesses.

## 2. Activity-host architecture

Bungie's model (Truman talk, `public-research.md`): cloud activity hosts run only the Lua mission
scripts at 10 Hz. They own authoritative "auth state" and receive "sense state" from the
**physics host**, one of the player consoles, which simulates the world and the AI.

| Responsibility | Status | Evidence / where |
| --- | --- | --- |
| Join handshake (join request/response, entity-index ranges) | CONFIRMED | orbit checkpoint; `--ah-join`, `--ah-entities` |
| Activity state (map name, world id, checksum, bubble count) | CONFIRMED | lime/lobster gates; `--ah-state` |
| Keep-alive / crash detection (baboon if the host is silent) | CONFIRMED | `--ah-heartbeat` |
| World state (type 11): zone set, member mask, group records | CONFIRMED at join | TEST180s; `--ah-world-state` |
| Mid-mission zone changes pushed by the host | CONFIRMED ignored | TEST366/367: the client stays in 112 |
| Bubble/slice-set switches | LIKELY client-driven | `0x31dab0` follows the player's position; the client reports type 21 |
| Roster and simulation packets (type 5) | CONFIRMED | 65-slot roster, host tick, replicated object stream |
| Baselines for every replicated object of the active bubbles before anything applies | CONFIRMED | `0x1ce1b8`; `--ah-baselines` |
| Simulation player with owner id and spawn grant | CONFIRMED | `--ah-sim-player`; spawn at TEST194 |
| Game mode (lifetime, kind 0x10) state 3 for spawning | CONFIRMED | `0x35793c` |
| Host tick (simulation clock) | LIKELY required | `0x1c94e0`; the stand-in sent 0 until `--ah-sim-clock` |
| Script state: objectives, sequences, cinematics, triggers, squads (auth state) | LIKELY | EBOOT handlers (`mission-director.md`) |
| Client sense state (type 6 upstream objects, type 21 own state) | CONFIRMED sent | the type-6 per-object header is not decoded |
| Trigger reports | UNKNOWN which message | armed/arming-time logic CONFIRMED statically (`0xb2fb68`) |
| AI simulation | LIKELY client (physics host) | Truman; squads are only placed by the host |
| Checkpoints, death/revive, wipes | UNKNOWN | `hard_wipe_globals` (0x22), `lifetime_timer` (0x11) objects exist |
| Rewards, XP, loot grants | UNKNOWN (split host/investment) | `loot_component_send_pickup_message`, services 12xx |
| Mission completion / return to orbit | UNKNOWN (505 seen on leave) | outro cinematic 105, service 505 |
| Machine list / boot (type 43) | CONFIRMED | TEST176 |

## 3. Mission scripting and objective findings

The full field meanings are in `mission-director.md`. In summary:
- **Mission scripts are not on the disc** (CONFIRMED: no Lua bytecode in 79,148 decompressed
  Cosmodrome tags). They must be rewritten per activity. The declarative data is on the disc:
  phases, objectives with texts, sequences, cinematics, triggers, squads, nav points and bubble
  links, all read by `d1_pkg.py groups`.
- **Objective (kind 0x20):**
  - shows only for players in its +0x2c0 mask (LIKELY; the stand-in never set it);
  - state switch cases 1-7; 3 = complete is LIKELY.
- **Cinematic (kind 6):** starts on a changed +0x4 counter with +0x8 playing and the player in
  the +0x9c cast list (LIKELY).
- **Sequence (kind 5):** plays while start + duration is ahead of the simulation clock (LIKELY).
- **Trigger (kind 0x1d):** armed at +0; reports once per arming time at +4 (LIKELY).
- **Squad (kind 1):** +0x44 generation, +0x5c member action, +0x40 AI state. What spawns members
  is UNKNOWN.
- **Kind 3, objective sensors:** 24 u7 states per bubble (orbit checkpoint), not yet exercised.
  They are LIKELY the per-bubble "sense" side of objectives.
- **Kind 4, spawn slots with a generation counter:** a higher u32 destroys and recreates the
  entity (CONFIRMED, TEST357-359).
- **`missions/a_guardian_rises.json`** encodes all of the above for the first mission.
  End-to-end tested in the fake-client harness only.

## 4. Requirements to complete A Guardian Rises

In the video order (`feature-gaps.md`); each item gives its evidence level.

1. **Revival cinematic 113, then 112.** Cinematic start fields are LIKELY known. Whether the
   client returns to gameplay by itself after `pf_104_res` / `_fp` is UNKNOWN. → TEST371.
2. **First objective and dialogue:** mask + state + sequence start time (LIKELY). → TEST371 / 369.
3. **Trigger-driven steps:** which message carries the report is UNKNOWN. → TEST371 logs every
   client message containing the trigger's id.
4. **The Breach:** a client-driven slice switch (LIKELY), plus baselines for bubble 3's 244
   objects via the live watch (existing). → TEST368.
5. **Enemies:** squad placement (UNKNOWN trigger), the client runs the AI (LIKELY), kills
   reported back (UNKNOWN form).
6. **Rifle pickup:** a loot pickup message, then an inventory grant (services 12xx, self-family
   delta), then equip. Everything after the pickup is UNKNOWN.
7. **Death/revive and checkpoints:** UNKNOWN. Look at `lifetime_timer` / `hard_wipe_globals` and
   the spawner's refusal list (`hard wipe active`).
8. **XP and level:** progression fields (`0x80801855` +0, +0x458) and a host or investment
   update. UNKNOWN.
9. **Completion:** outro cinematic (slice 105 `pf_105_cto`), rewards, and 505 back to orbit.
   UNKNOWN sequence.
10. **Visible guardian:** gear equipped (`--character-gear starter:titan`). LIKELY; TEST370/371.

## 5. PS3 Destiny version timeline

Sources:
- Sony's title-update list `https://a0.ww.np.dl.playstation.net/tpl/np/BLES01857/BLES01857-ver.xml`
  (CONFIRMED; the US disc BLUS31181 has the identical chain);
- the PKG headers, file tables and PARAM.SFO of every patch, read over HTTP ranges;
- the Tiger package headers inside each patch (build number, branch, date).

The Bungie-version mapping is LIKELY: it is inferred from the dates.

| PS3 APP_VER | Size | Content | Tiger build (changelist.date.branch) | Bungie update (LIKELY) |
| --- | ---: | --- | --- | --- |
| 01.00 (disc) | - | 244 packages | 41345.14.06.30 `live` | launch, 9 Sep 2014 |
| 01.01-01.12 | - | **no longer on Sony's server** | - | 1.0.x, 1.1.0 The Dark Below (Dec 2014), 1.2.0 House of Wolves (May 2015) |
| 01.13 | 9.03 GB | cumulative: EBOOT + 478 packages (272 package ids not on the disc) | 46419.15.08.16 `evil` | 2.0.0 The Taken King (8 Sep 2015) |
| 01.14 | 31 MB | EBOOT only | - | 2.0.0.x hotfix |
| 01.15 | 31 MB | EBOOT only | - | 2.0.0.x hotfix |
| 01.16 | 387 MB | EBOOT + 216 packages | 46880.15.09.18 | 2.0.1 |
| 01.17 | 50 MB | EBOOT + 98 packages | 47337.15.10.27 | Festival of the Lost / 2.0.2 |
| 01.18 | 353 MB | EBOOT + 225 packages | 47625.15.11.16 | 2.1.0 (Dec 2015, Sparrow Racing) |
| 01.19 | 100 MB | EBOOT + 174 packages | 48604.16.01.24 | 2.1.1/2.1.2 (Feb 2016) |
| 01.20 | 1.12 GB | EBOOT + 384 packages, **patch numbering restarts at _2** | 49795.16.03.30 | 2.2.0 April Update (Apr 2016) |
| 01.21 | 33 MB | EBOOT + 9 packages | 50118.16.04.16 | 2.2.1 |
| 01.22 | 220 MB | EBOOT + 81 packages | 50735.16.05.27 | 2.3.0 (June 2016) |
| 01.23 | 56 MB | EBOOT + 62 packages | 51273.16.07.06 `v230` | 2.3.0.x |
| **01.24** | 32 MB | **EBOOT only** | - | **2.3.1, 26 Jul 2016: the final legacy-console update** |

Details behind the table:
- Every patch file was last modified on 28 Jul 2016 (CONFIRMED, HTTP `Last-Modified`).
- Bungie called 2.3.1 "the last scheduled deployment for Legacy Consoles". Support ended on
  16 Aug 2016; Rise of Iron was PS4/Xbox One only (CONFIRMED:
  [Shacknews](https://www.shacknews.com/article/96013/todays-destiny-patch-will-be-the-last-update-for-the-xbox-360-and-ps3-versions),
  [GameSpot](https://www.gamespot.com/articles/destinys-next-update-is-ps3xbox-360s-last-heres-wh/1100-6442171/)).
- Patch content installs to `USRDIR/a0000001/packages` (CONFIRMED).
- The new package ids include:
  - `moon_hive_raid1` (Crota's End);
  - `reef` and `mercury` (House of Wolves);
  - `hiveship` (The Dreadnaught) and `marathon_*` (the Taken King class quests);
  - `impactsite`;
  - about 30 `pvp_*` maps, Sparrow Racing (`pvp_srl_*`), and new investment globals 0130/0131.

## 6. Recommended final preservation build

**BLES01857 (or BLUS31181) at APP_VER 01.24:** the 01.00 disc, with patches 01.13 to 01.24
installed in order. This is Destiny 1 on PS3 with The Taken King and every legacy-console update
up to 2.3.1, the most complete Destiny that ever ran on PS3. The DLC data is inside the patches
(CONFIRMED); owning the expansions only unlocks it. How the PS3 client checks that is UNKNOWN (see
the unknowns). Both regions carry the same chain, so either disc works.

## 7. 01.00 → final: known differences (so far)

The static diff of the decrypted 01.13 and 01.24 EBOOTs is done; details and the tables are in
[final-build.md](final-build.md).

| Area | Finding | Status |
| --- | --- | --- |
| Packages | 244 disc ids → about 516 ids; patches layer `_N` files over the disc's `_0` (e.g. the final investment globals 012c read 108 of 159 blocks from the disc) | CONFIRMED |
| Patch folder | `/dev_hdd0/game/<title>/USRDIR/a0000001/` (format `%s/game/%s/USRDIR/%08x/`, constant 0xa0000001, 01.24 0xc951c); patchable/unpatchable package registration | CONFIRMED |
| Patch-file selection | 01.19 left `_5` files, then 01.20 restarted at `_2`, so the client does not simply pick the highest patch number | CONFIRMED oddity, mechanism UNKNOWN (the manifest is the lead) |
| Type definitions in the EBOOT | new compiled layout (definition link at +0x34, counted field entries); `d1_types.py` reads both | CONFIRMED |
| Runtime types | 4,387 → 5,041 statically exported; one-to-one heuristic matches 4,308 (3,744 identical, 564 changed), with 79 unmatched old, 733 unmatched new and 156 renumbered | LIKELY |
| Character objects | same handles, grown: gear slot 0x28 → 0x30, gear at progression +0x1e8, equipped ids at self +0x2900, bigger inventory, flag tables 2x-2.5x | CONFIRMED |
| Object ids | kind field 6 → 7 bits | CONFIRMED |
| Web-service table | moved to tag 0x80a5e21a in package 012f; same record format; 66 → 94 services; every old wire id and type handle kept; 19 payloads changed | CONFIRMED |
| Sign-on | success message unchanged; new optional request fields 20/21 and response fields 13/14 | CONFIRMED |
| Demonware lobby/auth3 | host names and the 01.00 lobby client strings are gone in 01.24 (present in 01.13) | LIKELY removed |
| Error ids | the `error.*` string table is gone (248 → 1) | CONFIRMED |
| RPCS3 patch sites | 01.24: embedded CA 0x12d3740 (same PEM), manifest signature flag 0x18dca64, calculated PPU hash `PPU-ac58999f254e8d9dc817426c8c84b068ee8caaea` | CONFIRMED offline; live log check pending |
| Build ids | tool strings `live` → `evil` → `v230`; changelist 41345 → 51273 | CONFIRMED |

## 8. Expected server compatibility

Updated from the static diff; still to be proven by a first boot.

| System | Expectation | Why |
| --- | --- | --- |
| DNS sinkhole, STUN, RPCN, SSLv3 sign-on transport | UNCHANGED (LIKELY) | the sinkhole answers every name; the CA patch has its 01.24 address (`game_ca_patch.py --build 01.24`) |
| Sign-on reply | UNCHANGED (CONFIRMED format) | the success message is identical; the new fields are optional |
| Demonware lobby/matchmaking | PROBABLY NOT NEEDED (LIKELY) | the lobby client and its hosts are gone; find out on the first boot what, if anything, replaces it |
| BAP transport (shello, GCM, qz/ws frames) | UNCHANGED (LIKELY) | the same message names; NAT punch added |
| Service wire ids | UNCHANGED plus 28 new (CONFIRMED) | the final table |
| Service and investment object schemas | CHANGED but same handles (CONFIRMED) | regenerate with `d1_types.py`; the stand-in's schema-driven encoders follow |
| Investment data (items, buckets) | SIGNIFICANTLY CHANGED (CONFIRMED) | new packages; read with `d1_pkg.py items` over the final chains |
| Activity-host protocol (types 3/5/6/11/21...) | SLIGHTLY CHANGED (LIKELY) | the same engine; object ids widened; schemas regenerated |
| Mission scripts | UNCHANGED for the Cosmodrome story, rewritten per activity anyway | the objects are data |
| RPCS3 fixes | UNCHANGED (LIKELY) | engine-level SPU/RSX behaviour |
| The stand-in's hardcoded offsets | gear offsets now derived from the schema (`character_layout`); the account-flag tables and the 01.00 hook addresses remain | CONFIRMED |

## 9. Migration strategy

- **Incremental migration through 01.01-01.12 is not possible** from Sony's server (CONFIRMED:
  the list starts at 01.13). Available steps: 01.00 → 01.13 (one year of changes in one jump),
  then 01.14 to 01.24 (small steps).
- **Recommendation:**
  1. Keep 01.00 as the reference build for finishing A Guardian Rises. Its knowledge base is
     deepest, and the same mechanisms carry over.
  2. Start **static** migration work on 01.24 right away (the diffs in section 10). It costs no
     emulator time and tells us what breaks.
  3. For **runtime**, jump straight to 01.24 rather than 01.13. The steps from 01.13 to 01.24 are
     small, and the patches are cumulative in content (all must be installed anyway).
  4. Use 01.13 only as a static midpoint when a 01.00 → 01.24 difference is hard to explain.
- **Automated loop:**
  1. Install the build in a separate RPCS3 profile.
  2. Generate its build profile (types, services, items, activities, addresses).
  3. Boot with the stand-in in "record" mode.
  4. Detect the first failure: the error code on screen (the bootflow error table
     `0x11ea038`), the last client message, the last handled service.
  5. Map the failure to the diff report.
  6. Patch the profile or handler, and repeat.

  This is the existing TEST loop (Invoke-JoinCycle/MissionRun) with a per-build profile and a
  failure classifier added.

## 10. Build-comparison tooling plan

What exists and what to add. Each tool takes two builds (an ELF, and a package folder with its
patch chain):

1. **Types.** `d1_types.py diff A.elf B.elf`: match types by field-shape fingerprint (size + field
   kinds/offsets + nested shapes), not by handle, since handles are renumbered. Report added,
   removed and changed types, and a handle-mapping table A → B.
2. **Services.** Wire id, request and response shapes (via the mapping from item 1); locate the
   final service table first.
3. **Strings.** `d1_eboot.py strings` for both builds; show added and removed strings grouped
   by prefix (`error.*`, `bootflow:*`, service names).
4. **Functions.** Fingerprint functions by their call graph, referenced strings and constants,
   to port the 01.00 labels (`eboot-labels.tsv`) to the final build. Most names carry over
   automatically when a function references the same string or type.
5. **Packages and tags.** Per package id: patch chain, named tags, class histogram; per
   activity: bubbles, groups and objects (`d1_pkg.py bubbles/groups`); diff by name.
6. **Investment.** Item list (`items`), buckets, vendors and progressions, diffed by item and
   bucket hashes. These are stable API hashes (CONFIRMED for buckets).
7. **Report generator.** One Markdown/HTML report answering "what changed between A and B".

## 11. Tiger engine findings useful here

CONFIRMED:
- v24 big-endian packages with patch layering;
- tag hash = 0x80800000 + (pkg<<13) + index;
- dynamic arrays;
- localized strings (index + language table);
- replicated entity groups (object index = position in the group);
- scenario_client (group key, checksum, per-bubble slice sets, phase list);
- investment globals: item list, buckets, slot table, service table in 01.00;
- **all serializable runtime types compiled into the EBOOT**, which self-identify.

LIKELY:
- activity, bubble and objective definitions carry everything the host script needs except the
  logic (which object to set when);
- item definitions link to gear art in the investment assets.

What the client data can reconstruct without hand work:
- the host message and state schemas;
- the object lists and baselines (from groups; today they are dumped live);
- the bubble/slice-set tables;
- objective texts and chains;
- item and bucket tables;
- the Tower NPC placements.

What must be written by hand: the mission logic (Lua is server-side only), reward tables (LIKELY
server-side), vendor inventories (UNKNOWN whether any are in the data).

## 12. Automation opportunities

- **Baselines from the disc.** Generate `--ah-baselines` from `d1_pkg.py groups` (group key,
  kind, index, state types from the class) instead of dumping them from a running game.
- **Mutation runner.** For one object, iterate candidate values of a field in one boot (timed
  `--ah-object-at` steps), with hooks on the handler and screenshots. Output a table: value →
  handler hits → screen change. Built on the existing experiment script.
- **Structured run log.** The stand-in writes JSON lines (client message type, decoded body via
  the schema, pushed objects, rule firings). `ah_log_summary.py` then becomes a query tool.
- **Decode type 6 automatically.** Hook `0x1cdb10` once, then fit the per-object header against
  the logged bodies.
- **Screen classifier.** OCR the objective text and error codes from screenshots, so runs grade
  themselves.
- **Label porting** across builds (tool 4 in section 10).
- **Mission visualiser.** Render a mission's phases → objectives → triggers → sequences from the
  group data, with live state from the run log.

## 13. Server architecture improvements

Split `bap_server.py` along the lines already visible in the code:

- `transport/`: BAP framing, shello, GCM channels (stable across builds).
- `build_profile/`: per client build, generated by the tools, never hand-written:
  - type schema (from `d1_types.py`);
  - service table;
  - activity and bubble tables;
  - item table;
  - constants now hardcoded (type handles of root/self/peer objects, message type numbers,
    field offsets).

  One profile for 01.00, one for 01.24.
- `services/`: one handler per service wire, registered by name. The default is "result ok,
  fields absent", as today.
- `store/`: SQLite (accounts, characters, inventory, equipment, progression, flags). It feeds
  the families (self, select, peer) instead of CLI flags.
- `activity_host/`: session (join, roster, heartbeats), world (zone, bubbles, world state),
  replication (object table, baselines, deltas, clock), and the mission director (loads
  `missions/*.json` per activity).
- `config`: a run profile file (YAML/JSON) replacing the long CLI lists. The CLI stays for
  experiments.
- **Recording/replay:** every connection's decoded traffic to JSON lines, and a replay harness
  (the fake client exists in `test_ah_session.py`).

## 14. Multiplayer architecture

- **Peer-to-peer (LIKELY):** the physics host is a player console elected by the activity host
  (Truman). Simulation traffic between consoles goes through Demonware sessions and NAT. On one
  PC, that means several RPCS3 instances with distinct loopback addresses (the project already
  uses 127.x addresses per service) and RPCN accounts.
- **Server-side:**
  - activity host (roster up to 65 slots, CONFIRMED; world state member mask, CONFIRMED);
  - investment (peer family up to 20 peers, CONFIRMED);
  - lobby matchmaking.
- **Fireteams, strikes, raids:** the same host model with more roster members and a
  physics-host election. Host migration and Reconcile messages are UNKNOWN.
- **PvP:** the game modes are the same activity-host framework (the `globals_pvp` packages).
  Physics-host-based with lag compensation (LIKELY); UNKNOWN.

**Architecture requirement:** keep per-player state keyed by machine id and account in the host
and the store (not "the one client"), and never assume a single roster slot.

## 15. Major unknowns

1. How the client reports triggers, kills and pickups (sense-state messages; the type-6 header).
2. What makes squads spawn members.
3. Death/revive/checkpoint flow and the host's role in it.
4. Mission completion, rewards and XP granting (host vs investment services).
5. The urgent-message trigger and the login-cinematic freeze (existing).
6. Changes in the BAP/investment protocol in the final build (the service table is found; 19
   payloads changed) and what replaces the Demonware lobby step.
7. How the PS3 client checks DLC/expansion entitlements.
8. How the client picks package patch files after the 01.20 renumbering.
9. Physics-host election and Reconcile for more than one player.
10. Where vendor inventories and reward tables come from (data or server).

## 16. Required experiments

See "Top 10 unknowns" below for the fastest experiment per unknown. The ready-to-run ones are in
`next-session-playbook.md` (TEST368-372).

## 17. Roadmap

Reordered where the evidence suggests it. "Static" milestones need no emulator time and can run
in parallel.

| # | Milestone | Depends on | Notes |
| --- | --- | --- | --- |
| M1 | Guardian visible and equipped | TEST370/371 | gear presets exist |
| M2 | Ghost revival sequence plays | TEST371 | cinematic fields decoded |
| M3 | First objective shows | TEST369/371 | mask found |
| M4 | Trigger-driven objective progress | trigger report | the "any message" matcher is in place |
| M5 | Walk into The Breach with baselines | TEST368 | client-driven switch |
| S1 | Final-build static diff (types, strings, packages) | the 01.24 ELF decrypted on the PC | DONE (`final-build.md`) |
| M6 | Enemies spawn and fight (client AI) | squad spawn trigger | |
| M7 | Rifle pickup → inventory → equip | loot message, services 12xx | persistence store |
| M8 | Death/revive, checkpoints | UNKNOWN 3 | |
| M9 | A Guardian Rises completes, rewards/XP persist | M4-M8 | |
| S2 | Build profiles + server refactor | S1 | before more activities |
| M10 | Additional story missions (data-driven scripts) | S2 | the Cosmodrome first |
| M11 | Tower social space with NPCs | TEST372 | `tower.md` |
| M12 | Patrol | M10 | public-activity host paths |
| M13 | Final (01.24) client signs in | S1, S2 | moved earlier than in the brief |
| M14 | 01.24 reaches orbit | M13 | |
| M15 | 01.24 loads and plays A Guardian Rises | M14 | reuse the mission scripts |
| M16 | Strikes | M10 | |
| M17 | Fireteams (two local clients) | multi-instance RPCS3 | |
| M18 | Taken King content on 01.24 | M15, entitlements | |
| M19 | PvP | M17 | |
| M20 | Raids, complete final-PS3 preservation | all | |

**Why the final build comes earlier.** Everything after M9 would otherwise be built twice. Once
the static diff (S1) shows the protocol is mostly the same, it is cheaper to move to 01.24 at
M13 and build every later milestone on it.

---

## Done since the plan was written

- **Baselines from the disc.** `d1_pkg.py baselines <packages> --oodle <dll> --keys <group keys>`
  prints the `--ah-baselines` list. For the highway start (keys 0x992bb471, 0x3c2f5ed5,
  0x4786c0e0, 0xecf574b5) it gives 158 objects covering all 139 live-dumped records, with the
  same state-B flags. For Dock 13 (0x062dd95d in place of 0xecf574b5) it gives 165 covering all
  121.
  - Why it works: the client creates a record only for objects whose definitions are loaded,
    and skips unknown ids, so a disc superset is safe. Adding The Breach's group (0xfc0e45a3)
    covers the walk into bubble 3 without the live watch.
  - State B is fixed per kind: 1, 2, 3, 4, 0xc, 0x1c and 0x27 have it.
- **Final-build export.** `scripts/Invoke-FinalBuildExport.ps1` (PC):
  - fetches the update EBOOTs over HTTP ranges (`ps3_update.py`);
  - decrypts them with RPCS3;
  - exports the types;
  - writes `d1_builddiff.py` reports 01.00 -> 01.13 -> 01.24.

- **Package selection and DLC gating lead to the content manifest (LIKELY).** The content
  manifest that our sign-on stand-in serves (`signon_proto.content_manifest`, `ps3-route.md`)
  registers every package the game may open: field 3 entries {offer ref, path, package id,
  registration value = the u64 at package header +8}. Field 1 holds the offers, which tie
  packages to purchases. The 01.00 client also has "DLC Entitlement / CDN / Marketplace
  Containers", `bootflow:enumerate_content_rights`,
  `waiting_for_local_entitlement_enumeration`, `activity_failed_to_load_missing_dlc`, and the
  unlock-expression check for world transitions (`0x4105c4`, driven by account flags we already
  serve). So on the final build:
  - which `_N` patch file is used for each package id is most likely whatever the manifest
    lists, not the highest number on disk (unknown 8);
  - the Taken King packages can be listed without an offer. What is left to check is the local
    entitlement enumeration (unknown 7).

  01.00 has no string for the patch folder `a0000001`. The final EBOOT builds it as
  `%s/game/%s/USRDIR/%08x/` with 0xa0000001 (see below).
  - Experiment: boot 01.24 with a manifest listing the disc `_0` files plus every patch file
    (bare names, then `a0000001/packages/...` paths), and read the registration result (-90 /
    -91 codes as in comparison 021).
  - `ps3_update.py fetch --packages` now writes `final/packages.json` (file → the update that
    supplied it) for building that manifest.

- **The final build, statically (2026-09-28; [final-build.md](final-build.md)).** From the
  user's export of the 01.13/01.24 EBOOTs and the final 012c/012f chains:
  - the scanner now reads the updates' type layout (5,041 types in 01.24);
  - type handles are stable;
  - the web services are the same 66 plus 28 new;
  - the sign-on reply is unchanged;
  - the Demonware lobby client is likely gone;
  - the RPCS3 patch sites for 01.24 are known;
  - the stand-in derives its gear offsets from the loaded schema.

  First-boot checklist for 01.24:
  1. Build the copy with `--game-ca ... --unsigned-manifest --build 01.24`.
  2. Compare the PPU hash in the first RPCS3 log with the calculated value above.
  3. Export the runtime schema (or use `d1_types.py schema`).
  4. Watch DNS and connections after sign-on to see whether anything replaces the lobby.

## NEXT 10 ENGINEERING TASKS

1. **Run TEST371, the mission script**
   - WHY: it checks five static findings at once (gear, cinematic start, objective mask,
     sequence clock, trigger report).
   - WHAT: the runbook step 7 command.
   - FILES: `missions/a_guardian_rises.json`, `server/bap_server.py` (`--ah-mission`,
     `--ah-sim-clock`), `Invoke-ObjectExperiment.ps1`.
   - EXPERIMENT: as written, then walk to the Wall.
   - EXPECTED: the cinematic at +20 s, "Enter the Wall" at +140 s, and hook hits on
     `0x4ecd08`, `0x3561d8`, `0xb94414`.
2. **Decrypt and diff the 01.24 EBOOT (DONE 2026-09-28: `final-build.md`)**
   - WHY: the key question of how much of the server carries over.
   - WHAT: `rpcs3 --decrypt`, then `d1_types.py schema`. Add `d1_types.py diff` with shape
     fingerprints and `d1_eboot.py strings` diffs.
   - FILES: `scripts/d1_types.py`, `scripts/d1_eboot.py`.
   - EXPERIMENT: offline.
   - EXPECTED: a handle map 01.00 → 01.24 and a list of changed service/host types.
3. **Locate the final service table (DONE 2026-09-28: tag 0x80a5e21a in package 012f)**
   - WHY: needed for every BAP reply in 01.24.
   - WHAT: search the final investment globals chains and globals 0151-0156 for 36-byte records
     with wire ids, and the final EBOOT's registry root +0x20c equivalent.
   - FILES: `d1_types.py service_table`.
   - EXPERIMENT: offline, with the patch chains (the scratchpad method; the user can download
     the chains).
   - EXPECTED: a table like `investment-services.md` for 01.24.
4. **Baselines from the disc (DONE 2026-09-28: `d1_pkg.py baselines --keys ...`)**
   - WHY: removes the live-dump step (`--ah-baselines-watch`) and makes new activities one
     command.
   - WHAT: `d1_pkg.py baselines <activity>` from the groups and bubble tables.
   - FILES: `scripts/d1_pkg.py`, `bap_server.py`.
   - EXPERIMENT: compare with `baselines_highway_z112.txt` (138 objects).
   - EXPECTED: an identical list.
5. **Structured run log (DONE 2026-09-28: `events.jsonl` per connection, `server/run_log.py`)**
   - WHY: every later experiment needs decoded traffic.
   - WHAT: JSON-lines output of client messages (decoded by type), pushes and rule firings.
   - FILES: `bap_server.py`, `ah_log_summary.py`.
   - EXPERIMENT: `test_ah_session`.
   - EXPECTED: one line per message, decodable offline.
6. **Decode the type-6 header**
   - WHY: trigger, kill and pickup reports.
   - WHAT: hook `0x1cdb10` / `0x1ce134` with buffer dumps in one run.
   - FILES: runbook, `d1ppu.txt` watches.
   - EXPERIMENT: add to TEST371's watch list (`1cdb10 20 r3:64`).
   - EXPECTED: a per-object header layout, so the upstream objects can be parsed.
7. **Persistence store**
   - WHY: gear, inventory, XP and progression must survive runs.
   - WHAT: SQLite store behind the self/select/peer families, replacing `--store` JSON and
     `--character-gear`.
   - FILES: `bap_server.py` (`family_blob`, `character_from_request`).
   - EXPERIMENT: create, gear, restart, and check the character is unchanged.
   - EXPECTED: an identical character across runs.
8. **Squad spawn**
   - WHY: enemies (M6).
   - WHAT: a mutation run over squad fields (+0x44 generation, +0x40, +0x0 location, +0x54/+0x58
     counts) in The Breach.
   - FILES: `missions/`, experiment script.
   - EXPERIMENT: TEST371's Breach step plus timed variants; hooks on `0x1f8d20`, `0x1f7e74`,
     `0x230c70`.
   - EXPECTED: the field combination that creates Fallen.
9. **Build profiles (STARTED: `character_layout(schema)` derives the gear offsets; the CA patch
   has per-build sites)**
   - WHY: stop hardcoding 01.00 constants.
   - WHAT: move type handles, message numbers and object ids into a generated
     `profiles/01.00.json`; load them by `--build`.
   - FILES: `bap_server.py`, `investment.py`, the new `profiles/`.
   - EXPERIMENT: unit tests pass with the profile.
   - EXPECTED: no behaviour change.
10. **Entitlement/DLC check reconnaissance**
    - WHY: the Taken King content needs it on 01.24.
    - WHAT: static search for "Requires Expansion" / DLC checks (`0x4105c4` in 01.00), NP
      entitlement calls (sceNp* imports) and investment account flags.
    - FILES: `d1_eboot.py`.
    - EXPERIMENT: offline, then boot 01.24 without DLC licences.
    - EXPECTED: know whether entitlements come from NP (RPCN) or BAP (our stand-in).

## TOP 10 TECHNICAL UNKNOWNS, with the fastest experiment

1. **Trigger report message.** TEST371: the "any" matcher logs which client message type fired;
   diff the bodies before and after the Wall.
2. **Squad spawn trigger.** One-boot mutation of squad fields in The Breach, with hooks on the
   spawn candidates.
3. **Objective state values.** TEST369 with the mask: step 1..6, one screenshot each.
4. **Does the client leave the revival cinematic by itself?** TEST371: watch the slice-set
   reports (type 21) after `pf_104_res_fp`.
5. **Death/revive flow.** Let the guardian die in The Breach (walk off a ledge) with hooks on
   the spawner and the `hard_wipe_globals` / `lifetime_timer` handlers; log the client messages.
6. **Rewards and XP.** Complete an objective, then log BAP requests (services 12xx) and host
   messages; check whether the client asks for anything.
7. **Final service table.** DONE: 012f tag 0x80a5e21a; 94 services, the old 66 unchanged in
   wire id and handles.
8. **DLC entitlement check.** Offline static search, then boot 01.24 with no licences and read
   the error code.
9. **Patch-file selection after 01.20.** Install 01.19 then 01.24 in RPCS3 and log which
   `_N.pkg` files the game opens (the RPCS3 file log).
10. **The urgent-message trigger.** Bisect `--self-flags` tables A/B/C in halves over about 10
    boots (the existing flags tooling).
