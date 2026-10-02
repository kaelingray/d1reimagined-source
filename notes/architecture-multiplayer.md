# Multiplayer-first architecture and state ownership

Mandatory requirement (project owner, 2026-10-02; see [`preservation-scope.md`](preservation-scope.md)):
every backend system, data model, API, activity system, persistence layer, session, tool and test
is designed so several independent Destiny clients, accounts, characters, fireteams and activities
can run at the same time. Single-client shortcuts are allowed only to prove a hypothesis, must be
marked, isolated, and listed in the debt register below with their replacement path.

Every design question is checked against: *will this still be correct when several independent
clients, accounts, characters, fireteams and activities run at the same time?*

## 1. Scopes

All mutable state belongs to exactly one of these scopes, and code receives the scope's id
explicitly (never a global "current player"):

| Scope | Identifier | Examples of state |
| --- | --- | --- |
| Global preservation state | - | static definitions, reset schedule, global vendor rotations |
| Account | account id (u64, from sign-in; membership id) | profile, account flags, unlocks, Grimoire, vault, account currencies, collections, settings (service 701 data) |
| Character | character id (u64, globally unique) | inventory, equipment, progression, subclass nodes, quests, bounties, reputation, per-character story/cinematic flags |
| Player session | session id (per BAP connection) | connection, crypto channels, presence, current character, pending requests |
| Fireteam | fireteam id | members, leader, privacy, launch state |
| Matchmaking session | ticket / group id | queue, search parameters, population |
| Activity instance | activity instance id | activity definition, difficulty, modifiers, checkpoint, mission script state, completion |
| World instance | world instance id (one per simulation/host session) | world state (version, peer slots, group records, slice-set requests), simulation (roster, replicated objects, tick), game mode |
| Server instance | process/service id | sockets, caches |

## 2. State ownership (authority)

**OBSERVED** = established from 01.24 runs or code; **INFERRED** = from structure, to be confirmed.

| State | Authority | Evidence |
| --- | --- | --- |
| Account, characters, inventory, currencies, progression | preservation backend (investment services, BAP) | OBSERVED: the client fetches/pushes families via BAP (206, qz pushes) and never creates instances itself |
| Item instance ids | backend | OBSERVED: instance ids arrive in family blobs |
| Slice set the player is in | client engine is authoritative for itself (`authority-over-current-slice-set`); the host coordinates group switches via world-state group records 0/1 and the group request @0x2e48+0x21c8 | OBSERVED: JOIN36-42 |
| World state (peer slots, groups, version) | activity host (one per world instance) | OBSERVED: type 11 world state, copied to the simulation at +0x70 |
| Replicated mission objects (objectives, cinematics, sequences, triggers, squads, game mode) | activity host | OBSERVED: type 5 object stream; the client applies, never invents |
| Activity quarantine | activity host via game mode state | OBSERVED: JOIN26 |
| Player position / movement | INFERRED: owning client (physics host for its own player), relayed to peers | client type-6 upstream objects carry the player object |
| AI simulation, enemy health | INFERRED: a physics-host client (Bungie's "physics host" model) with the activity host placing squads | 01.00 squad analysis; not yet tested on 01.24 |
| Mission objective progression | activity host (script) | OBSERVED: objective states come from the host |
| Fireteam membership | fireteam service (backend) | INFERRED (mm/fireteam BAP services) |
| Rewards / loot | backend reward service; activity host reports events | INFERRED; see `loot-and-rewards.md` |
| Match score (Crucible) | NOT INVESTIGATED | - |

## 3. Target components

Logical services with explicit interfaces; they may share one process today and must be separable
later. Current code is mapped to them.

| Component | Responsibility | Today |
| --- | --- | --- |
| Gateway (BAP framing, crypto, routing) | connections, channels, request routing per session | `server/bap_server.py` (framing + everything else) |
| Authentication/session | SignOn, auth3, sign-in, session ids | `signon_probe.py`, `bap_server.py` (503) |
| Profile/inventory (investment) | accounts, characters, items, families, qz pushes | `bap_server.py` + JSON store |
| Social/presence, fireteam, matchmaking | presence, fireteams, mm req/rsp, ahp | `bap_server.py` (`--mm-rsp`, `--ahp-reply` constants) |
| Activity-instance service | instances, membership, checkpoints, completion | not separate yet |
| Activity host (world instances) | world state, simulation, scripts | `bap_server.py` proxy mode (`--ah-*`), one world per connection |
| Persistence | durable, transactional player DB | JSON file (`SIGNON_LOCAL/characters-0124.json`) |
| Static data | definitions, catalog, schemas | `d1xenia-data`, `dump_mission_catalog.py`, `d1_types.py` |
| Admin/debug | live views, logs | `run_log.py`, owned-memory readers |

## 4. Debt register (single-client assumptions)

| Debt | Where | Effect with several clients | Replacement path |
| --- | --- | --- | --- |
| One character per account in the store | `save_character()` overwrites `id`/`request` | second character replaces the first | character table keyed by character id; account -> characters relation |
| Non-transactional JSON read-modify-write | `load_store`/`save_*` | concurrent sessions lose writes | durable DB (SQLite first) with transactions, versioned rows, idempotency keys |
| Starter item instance ids `0x100 + slot` | `character_gear()` | ids collide across characters/accounts | backend-allocated, globally unique instance ids |
| One world per activity-host connection; client always roster slot 0, world-state peer slot 0 | proxy (`--ah-roster 0`, `world_state_delta`) | two clients in "the same" activity get two private worlds | activity-host service owning world instances; peers join an instance; roster and peer slots allocated per instance |
| Activity-host behaviour configured by process arguments (`--ah-zone`, `--ah-teleport`, mission script) | proxy CLI | one configuration for every client of the process | per-instance configuration from the activity-instance service (activity definition + script) |
| World-state version counter | fixed 2026-10-02: per connection (`world_version` in `handle()`) | was process-global before | per world instance once instances are shared |
| Mission director timeline relative to the connection's join or its client's in-world report (`clock`) | `--ah-mission` | each client runs its own copy | script state per activity instance, events from all members |
| Debug live control file (`--ah-control`) | proxy | a line applies to every connected client | operator console on the activity-instance service, addressed by instance id |
| Constant mm/ahp replies | `--mm-rsp`, `--ahp-reply` | every client "matches" alone | matchmaking/fireteam service |

## 5. Rules for new code

1. Functions take explicit ids (account, character, session, instance) or an object scoped to one.
2. No new module-level mutable state for game data; per-connection state lives in the connection
   handler, per-instance state in an instance object.
3. Grants and spends (items, currency, rewards, quest/activity completion, checkpoints) carry an
   idempotency key and are written transactionally.
4. Identifiers are stable and backend-allocated (never memory addresses or slot numbers).
5. Every log line carries the ids it concerns (`run_log.py` event records: connection, account,
   character, instance).
6. Tests that touch shared state include a two-session case (simulated clients are acceptable until
   two real clients run).
