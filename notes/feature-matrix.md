# Master feature matrix

The definitive measure of progress against [`preservation-scope.md`](preservation-scope.md).
Target build **01.24** (BLES01857) unless a row says otherwise; "01.00" notes what the reference
build reached. Statuses: UNKNOWN, NOT INVESTIGATED, INVESTIGATING, BLOCKED, PARTIAL, WORKING,
VERIFIED, REGRESSION. WORKING = observed working in a real run; VERIFIED = WORKING plus an
automated check that would catch a regression. Evidence is linked to the run (FINAL124_*) and the
checkpoint note that records it. Update a row whenever a run changes what is known.

Primary checkpoint for 01.24: [`checkpoint-2026-10-01-profile.md`](checkpoint-2026-10-01-profile.md)
(cited below as *CP*). Earlier 01.24 work: `checkpoint-2026-09-28/29/30-*`. 01.00 history:
`checkpoint-2026-09-24-character.md`, `checkpoint-2026-09-25-orbit.md`, `test-log.md`.

Last full review: 2026-10-02.

## 1. Platform, boot and sign-in

| Feature | Status | Evidence / implementation | Known failures | Next action |
| --- | --- | --- | --- | --- |
| Patched RPCS3 boot of 01.24 (root CA swap, unsigned manifest) | WORKING | `server/prepare_rpcs3_copy.py --build 01.24`; every FINAL124 run | intermittent stall at character select (JOIN29), one Vulkan device loss (JOIN33) | track as stability items |
| Title screen, automated Cross | WORKING | `scripts/Invoke-Final124Run.ps1` | - | - |
| DNS sinkhole / loopback services | WORKING | `server/dns_sinkhole.py`, `hosts.py` | - | document full host list in a protocol doc |
| PSN check (RPCN local) | WORKING | `scripts/rpcn_local_account.py` | - | - |
| SignOn / auth3 / content manifest (01.24 binary config, fields 13/14) | WORKING | `server/signon_probe.py`, `signon_proto.py`, `build_manifest_probe.py --chains` (837 entries) | - | regression test of the full 01.24 reply |
| STUN | PARTIAL | `server/stun_stub.py` logs 01.24 probes; replies not shown to matter | 01.24 STUN reply format unknown | confirm whether anything depends on it |
| Package registration / patch chains | WORKING | `--chains` manifest; stage 19 passes (CP) | - | - |
| BAP sign-in (start 0x1e echo, shello) | WORKING | `server/bap_server.py` | - | - |
| Profile setup screens (screen bounds, brightness, subtitles) | WORKING | FINAL124_PROFILE4 (CP) | needs stick input on two screens | automate fully in the run wrapper |
| User message (0x20/0x21) | WORKING | reply `20 00`; `--user-msg` drives a character-select dialog (JOIN28) | - | - |
| UPO, services 701/702 | PARTIAL | default replies accepted | content of 701/702 not modelled | decode and persist |
| Urgent Message / Message of the Day dialogs on world entry | INVESTIGATING | not from user msg (JOIN28); 01.00 also unresolved | dialogs need Cross | find the data source (likely a BAP push or title config) |
| Startup telemetry / presence | NOT INVESTIGATED | - | - | - |
| Error database (weasel, centipede, lime, lobster, baboon, pigeon, newt, bass, urchin...) | PARTIAL | scattered through checkpoints | no single table | build `notes/errors.md` from the checkpoints |

## 2. Account, characters, inventory (investment services)

| Feature | Status | Evidence / implementation | Known failures | Next action |
| --- | --- | --- | --- | --- |
| Account (self family 0x808018c7, 01.24 active-character layout) | WORKING | `family_blob()` layout fix; Render3 (CP) | - | persistence across restarts test |
| Character creation (class, appearance, service 501) | PARTIAL | Titan created and stored, FINAL124_PROFILE4 | Hunter/Warlock creation not tested on 01.24 | test all three classes |
| Character select / log in | WORKING | JOIN runs select the stored Titan | - | - |
| Starting gear (final starter kits per class) | PARTIAL | `--character-gear starter:01.24:CLASS` (CP) | appearance in mission not verified (first person) | third-person check (orbit/Tower/cinematic) |
| Multiple characters, deletion, switching | NOT INVESTIGATED | - | - | - |
| Item definitions / catalog (6,557 items, 17 buckets, strings) | WORKING (tooling) | `scripts/dump_mission_catalog.py`, private export | - | query tool; link to item-system doc |
| Item instances, vault, Postmaster, transfers, equip changes | NOT INVESTIGATED (01.24) | 01.00 had partial character families | - | decode 01.24 inventory families |
| Talent grids, perks, stats, Light | NOT INVESTIGATED | - | - | - |
| Unlock / universe flags | PARTIAL | `--self-flags`, `--universe-flag-values 3390=1,309=1` | flag meanings mostly unknown | map flags to definitions |
| Grimoire, records | NOT INVESTIGATED | - | - | - |
| Player database (durable store) | PARTIAL | `server/player_db.py` schema v1 (accounts, characters, items, idempotent grant ledger, migrations; concurrency tests) and `bap_server.py --player-db`; legacy JSON store still the default | not yet used in a live run; inventory families not served from it | live run with `--player-db`; serve inventory from it |

## 3. Orbit, Director, matchmaking

| Feature | Status | Evidence / implementation | Known failures | Next action |
| --- | --- | --- | --- | --- |
| Post-prologue matchmaking poll (mm req/rsp) | WORKING | `--mm-rsp 4=` (CP) | - | - |
| Activity host assignment (ahp req/rsp) | WORKING | `--ahp-reply` | - | - |
| Orbit (ship, Director) | NOT INVESTIGATED (01.24) | 01.00 reached orbit after login; 01.24 new characters go to the intro activity | - | after completing the intro, or with a character whose intro is done |
| Director nodes, availability, difficulty | NOT INVESTIGATED | activity table 338 definitions decoded | - | - |
| Fireteams, invites, presence | NOT INVESTIGATED | - | - | design with multiplayer (scope 44) |
| Matchmaking proper (strikes, Crucible) | NOT INVESTIGATED | - | - | - |

## 4. Activity host (world sessions)

| Feature | Status | Evidence / implementation | Known failures | Next action |
| --- | --- | --- | --- | --- |
| Join (request/response, entity indices, activity state, checksum `0x8fe07299`, 22 bubbles) | WORKING | `bap_server.py --ah-join ...`; JOIN10+ | - | - |
| Simulation packets (roster, sim player spawn grant, game mode, baselines, clock) | WORKING | JOIN24+ | - | multi-peer roster |
| Game mode state incl. **activity quarantine** | WORKING | `game_mode()` sends bubble -1 (JOIN27) | real per-activity quarantine values unknown | find what the original host sent |
| World state (peer slot, group records, **version**) | WORKING | `world_state_delta()`; version fix (JOIN39) | - | full peer client-state relay |
| Slice-set switch protocol (group record 1, per-peer byte, world-state version) | WORKING | `--ah-group-switch`; version per session; JOIN39, JOIN51 | finish must not bump the version (JOIN43-50 black screen) | automated regression run |
| Host-driven teleport (world-state group request @0x2e48+0x21c8) | WORKING | mission step `{"at": S, "teleport": SET}` or `--ah-teleport ZONE@SECONDS` (state 1 -> finish state 3 at the client's arrival report, same version; dropped on arrival); JOIN51/53 | one world per proxy connection (debt) | per activity instance with several peers |
| Client state relay (type 21 -> world-state peer slot) | NOT INVESTIGATED | - | - | needed for multiplayer |
| Heartbeats / host timeout (baboon) | WORKING | `--ah-heartbeat` (01.24 runs use `1:-1` from JOIN41) | an empty type-11 heartbeat clears the teleport field | - |
| Multi-client activity sessions | NOT INVESTIGATED | - | - | scope 44 |

## 5. Mission: A Guardian Rises / The Taken King: Prelude (intro activity, world 229)

| Feature | Status | Evidence | Known failures | Next action |
| --- | --- | --- | --- | --- |
| Arrival in 112 (highway), visible world, HUD, walking | WORKING | JOIN27 | Urgent Message dialogs | - |
| Arrival in 113 (intro cinematic slice set) | WORKING | JOIN32-35 | one GPU device loss (JOIN33) | - |
| Revival cinematics `pf_104_res`, `pf_104_res_fp` | WORKING | JOIN34/35 with the director; null object refs fix (JOIN32) | letterbox stale strip (rendering) | - |
| Hand-off 113 -> 112 after the intro | WORKING | automatic, JOIN51 (no cinematics); JOIN53 after both cinematics (stopped by the script) | a cinematic left to end on its own keeps the screen black (JOIN52) | - |
| "New Mission" banner (incoming sequence) | WORKING | JOIN31 | - | - |
| Objective display / waypoints | INVESTIGATING | handler `0x3a68c0` runs; nothing shown (JOIN31) | gate `0x20f048` (definition +0x24) | trace in 112 after the hand-off |
| Player triggers, Ghost light cinematic, weapon pickup | NOT INVESTIGATED (01.24) | script exists (`missions/a_guardian_rises.json`) | - | after objectives |
| Enemies (Fallen squads in The Breach) | NOT INVESTIGATED (01.24) | squad layout mapped (+0x74) | spawn trigger unknown | - |
| Zone change 112 -> 24 (The Breach) by walking | PARTIAL | follow-player request seen (JOIN25); protocol now known | not re-tested with the switch protocol | test with `--ah-group-switch` |
| Mission completion, rewards, return to orbit | NOT INVESTIGATED | - | - | - |

## 6. Content matrices (to be generated)

Story missions, patrol zones, strikes, raids, Crucible maps and modes, quests, bounties, vendors,
public events, special activities and DLC content each get their own matrix generated from the
decoded activity table (338 definitions) and catalog, so that every entry is listed even before it
is tested. Status for all of them is currently NOT INVESTIGATED except the intro activity above.

## 7. Systems not yet investigated on any build

Loot and reward generation (engrams, loot pools, world-drop RNG, Cryptarch), vendors, economy,
reputation, quests/bounties, progression/XP, subclasses, resets/rotations, modifiers, events,
special modes, Crucible, raids, strikes, patrol, checkpoints, Grimoire, kiosks/cosmetics,
sparrows, social. Each moves to INVESTIGATING when its research note is opened.

## 8. PC route (separate workstream)

| Feature | Status | Evidence | Next action |
| --- | --- | --- | --- |
| RPCS3 rendering fixes (SPU DMA video-memory writes, descriptor pools) | WORKING | `patches/rpcs3-destiny-fix.patch` | upstream drafts await approval |
| Diagnostics build (PPU hooks, screenshots, traces) | WORKING | `patches/rpcs3-destiny-main-memory-blit.patch` | - |
| Input (keyboard pad profile, scripted input) | WORKING | `Set-PadConfig.ps1`, `Send-RpcsKey.ps1` | - |
| Long-session stability | PARTIAL | memory leak fixed on 01.00; 01.24 soak not run | soak test on 01.24 |
| Xenia (Xbox 360) route | NOT INVESTIGATED recently | `Start-XeniaTest.ps1` | - |

## 9. Multiplayer readiness by subsystem

Readiness: NOT DESIGNED, SINGLE-CLIENT ONLY, MULTIPLAYER-AWARE, MULTI-CLIENT TESTED, MULTIPLAYER
VERIFIED. Debts and replacement paths: [`architecture-multiplayer.md`](architecture-multiplayer.md) section 4.

| Subsystem | Readiness | Why |
| --- | --- | --- |
| SignOn / auth3 / manifest | MULTIPLAYER-AWARE | per-connection; stateless replies; never run with two clients |
| BAP gateway (framing, crypto, sessions) | MULTIPLAYER-AWARE | one handler and channel state per connection |
| Account / character store | SINGLE-CLIENT ONLY | JSON read-modify-write, one character per account, non-unique starter instance ids |
| Inventory / item instances | NOT DESIGNED | - |
| Fireteams, presence, matchmaking | NOT DESIGNED | constant replies |
| Activity host: world state, simulation, scripts | SINGLE-CLIENT ONLY | one private world per connection, client fixed in roster/peer slot 0; world-state version now per connection |
| Slice-set switch protocol (group records, group request) | MULTIPLAYER-AWARE | protocol is group-level by design (records name peers, request is per group); implementation still per connection |
| Mission director | SINGLE-CLIENT ONLY | timeline per connection, configured by process arguments |
| Rewards / loot | NOT DESIGNED | research opened (`loot-and-rewards.md`), design constraints recorded |
| Logging (`run_log.py`) | MULTIPLAYER-AWARE | per-connection event files; ids not yet on every record |
| Test tooling (run wrappers) | SINGLE-CLIENT ONLY | one emulator per run; second emulator instance not yet supported |
