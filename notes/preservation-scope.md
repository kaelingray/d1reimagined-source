# Preservation scope (master scope, set by the project owner 2026-10-02)

This is the project owner's statement of scope, recorded here so it does not exist only in a chat
conversation. It defines what "functionally preserved" means. Progress against it is measured in
[`feature-matrix.md`](feature-matrix.md); the per-content matrices live next to it.

## Objective

Identify, recreate, restore, emulate or replace every game-side and server-side system required
for Destiny 1 to function as closely as practical to the original live game, in a preserved
environment without dependence on the discontinued original infrastructure. Not merely boot,
menus or one activity.

- **Primary target build: 01.24** (BLES01857, Destiny 2.3.1 / Rise of Iron era PS3 final).
- 01.00 and other builds are references for reverse engineering, comparison, protocol analysis,
  behaviour analysis and regression testing. Behaviour from another build is **not** assumed for
  01.24; validate against the target build.
- Long-term target: a complete PC-playable preservation with as much original functionality as
  technically achievable.
- Immediate target: 01.24 at least at the functional level 01.00 reached; then continue toward
  full coverage via the matrices.

For every system determine: what the client expects; what requests it makes; what the original
services returned; required persistent state; required content/assets/data; dependencies between
systems; what the client handles locally; what needs server authority; what can safely be
recreated; what requires emulation of original backend logic; differences between versions; what
is implemented, incomplete, missing; how to test it automatically; how accurately the recreation
matches the original.

## Subsystems in scope

1. **Boot and initialization** - executable/game/platform init, title screen, build/version and
   patch checks, DLC/content entitlement init, network init, DNS/host resolution, HTTPS/TLS,
   certificates, authentication startup, account discovery, platform identity, user profile init,
   service/endpoint discovery, configuration downloads, startup manifests, startup telemetry and
   presence, error handling, maintenance-mode responses, offline/error states. Exact request
   sequence until the client considers itself connected; tooling to reproduce, log and validate it.
2. **Identity and sessions** - platform account identity, Bungie account mapping, auth and session
   tokens, refresh, login state, character/account ownership, expiry, reconnection, multiple
   sessions, game-instance tokens, security/session validation, signed requests, headers, cookies,
   device/platform/user/character/membership ids, error states, banned/restricted states. Which ids
   can be generated locally and which relationships must persist.
3. **Account/profile model** - profile, display name, character slots and ownership, progression,
   account-wide unlocks, Grimoire and score, account and unlock flags, tutorial and cinematic
   completion flags, activity completion, quest/story/vendor/faction progression, reputation,
   account currencies, collections, record books, DLC entitlements, expansion ownership,
   difficulty unlocks, social settings, character creation and deletion. All state persists.
4. **Character lifecycle** - create (race, gender/body, class, appearance, preview), starting
   equipment/level/subclass, character ids, persistence, loading, multiple characters, deletion,
   switching, character progression, per-character story/quest/reputation state. Test every class
   and important creation combination.
5. **Inventory** - item definitions vs instances, instance ids, weapons, armor, ghost shells, class
   items, artifacts, consumables, materials, quest/mission items, bounties, currencies, engrams,
   packages, reputation rewards, vendor purchases, vault, character and account inventory,
   Postmaster, transfer, deletion, locking, equipping, swapping, capacity, overflow, stacks and
   splitting, rarity, item level, attack/defense, Light, quality, stats, random rolls, upgrade
   nodes, perks, talent grids, perk unlocking, upgrade currencies, infusion, ascension, weapon and
   armor XP, item progression, rewards, duplicates, persistence. The client must receive correct
   **instance** data, not only static definitions.
6. **Equipment** - all slots and rules: weapons, armor, subclass, ghost, artifact, class item,
   level/class/exotic restrictions, stat effects, Light and character/derived stat calculation,
   swaps, in-activity changes, activity-specific restrictions.
7. **Definition databases** - manifest databases; investment, activity, destination, item, vendor,
   quest, bounty, sandbox, talent grid, perk, stat, objective, record, faction, progression and
   reward definitions; dialogue references; localization, string tables, hash mappings, content
   versioning. Tools to inspect and query them; which data is local and which came from services.
8. **Director** - destinations (Earth, Moon, Venus, Mars, Reef, Dreadnaught, Tower, Iron Temple,
   Crucible), story/strike/raid/patrol/quest nodes, daily/weekly activities, difficulty, modifiers,
   fireteam requirements, recommended Light, availability, locked behaviour, expansion and quest
   gating - reflecting player progression.
9. **Activity start** - selection, fireteam validation, matchmaking decision, instance and world
   session allocation, configuration, difficulty, modifiers, spawn data, mission state,
   checkpoints, join-in-progress, rejoin, completion, failure, return to orbit, restart. Every
   service call between selecting an activity and spawning.
10. **Orbit** - entering orbit, ship, fireteam display, player cards, joining/leaving, leader
    promotion, activity selection, matchmaking state, launch countdown/cancel, rejoin, presence,
    privacy, invites.
11. **Playable spaces** - world/activity instances and ids, player assignment, spawn points,
    checkpoints, public/private spaces, bubble/zone transitions, phases, mission scripts, instance
    persistence and teardown, joining existing instances, departure, reconnection. What is client
    logic versus server infrastructure.
12. **Patrol** - destination loading, public areas, enemy spawning, public events, patrol missions
    and beacons, zone transitions, chests, resource nodes, random encounters, Taken events,
    court-style encounters, world bosses, collectibles, dead ghosts, rewards, reputation,
    population, joining/leaving public instances.
13. **Story missions** - per mission: unlock conditions, difficulty, spawn, scripting, objectives,
    dialogue, encounters, checkpoints, respawn zones, darkness zones, bosses, completion, rewards,
    quest/story progression, cinematics, return to orbit. A mission completion matrix.
14. **Strikes** - selection, playlists, matchmaking, difficulty, modifiers, scripting, bosses,
    checkpoints, wipes, rewards and strike loot, reputation, daily/weekly progression, heroic,
    Nightfall. Each strike tested independently.
15. **Raids** - Vault of Glass, Crota's End, King's Fall, Wrath of the Machine and build variants:
    launch, fireteam requirements, difficulty, checkpoint storage, encounter state and completion,
    wipes, revives, mechanics, loot checkpoints, weekly lockouts, rewards, challenge and hard
    modes, checkpoint persistence. Each encounter has a documented test state.
16. **Crucible** - matchmaking, private matches, teams, map and mode selection, initialization,
    spawning, scoring, rounds, death/respawn, heavy ammo, supers/abilities, objectives, timer,
    mercy, completion, rewards, reputation, bounties, scoreboard. Which parts depend on
    peer-to-peer, dedicated/session infrastructure or external matchmaking.
17. **Matchmaking** - queues, search, skill parameters, connection parameters, population,
    fireteam/strike/Crucible matchmaking, search expansion, match found/cancel, joins, leaves,
    timeouts; reproduce locally, replace with deterministic grouping, or a custom service.
18. **Fireteams and social** - create/join/invite/request, leader and promotion, privacy settings,
    removal and departure, orbit sync, launch, join in progress, member status, inspection;
    friends (platform/Bungie), presence, status, current activity, joining friends, recent
    players, roster, blocking, muting, clans. Preserved equivalents where platform services are
    gone.
19. **Networking** - HTTP/HTTPS/web APIs, TCP/UDP, peer-to-peer, NAT traversal, platform
    networking, session traffic, voice, world/entity/movement/combat synchronization, host
    migration, reliable/unreliable messaging, keepalives, timeouts, encryption, packet formats,
    compression, serialization. Protocol documentation for every protocol.
20. **Combat and AI** - whether combat/AI is local or has network/server dependencies: spawning,
    activation, pathfinding, targeting, weapons, abilities, bosses, health, shields, damage, death,
    respawning enemies, waves, script triggers, difficulty scaling; player weapons, melee,
    grenades, supers, class abilities, movement, damage/crits/elements, buffs, debuffs, shields,
    regeneration, death, revives, respawn, ammo and drops, switching, reloading, exotics, perks.
21. **Subclasses** - all Titan/Hunter/Warlock subclasses: unlock, nodes, super, grenades, melee,
    movement, passives, upgrade progression, node persistence.
22. **Progression** - character XP, levels and cap, Light and gear contribution, XP rewards, motes,
    weapon/armor/subclass/quest/reputation XP, activity and level-up rewards.
23. **Quests and bounties** - acquisition, inventory, steps, objectives, counters, hidden
    objectives, activity/item requirements, updates, completion, rewards, abandon/reacquire,
    branches; bounty acquisition, tracking, turn-in, rewards, reputation, expiry, rotations. Test
    every quest chain in the build.
24. **Vendors** - inventory, categories, rank and reputation requirements, rank-up packages,
    currencies, purchase/sell, daily/weekly rotation, quest vendors, faction packages, Cryptarch,
    Postmaster, Gunsmith, Speaker, Vanguard, Crucible, factions, Reef, Iron Banner, Trials, DLC
    and Iron Temple vendors - reflecting progression.
25. **Economy** - Glimmer, Legendary Marks, Strange Coins, Motes, planetary materials, Weapon
    Parts, Armor Materials, Hadronic Essence, Plasteel, Sapphire Wire, Spinmetal, Helium, Spirit
    Bloom, Relic Iron, upgrade materials, raid/DLC/event currencies: acquisition, spending,
    limits, persistence.
26. **Engrams and rewards** - engram drops, inventory, rarity, Cryptarch decoding, reward
    generation, item-level generation, loot pools, reputation effects, special engrams; enemy,
    mission, strike, raid, Crucible, patrol, public-event, quest, vendor/reputation package, chest,
    weekly and activity-specific rewards, exotic drops, randomization, duplicates, overflow,
    Postmaster routing. Reverse engineer all reward generation.
27. **Reputation** - Vanguard, Crucible, Dead Orbit, FWC, New Monarchy, Cryptarch, Gunsmith,
    Queen's Wrath, House of Judgment, Iron Banner, Trials, DLC systems: ranks and rewards.
28. **Resets and rotations** - daily/weekly reset, daily missions, heroics, Nightfalls, raid
    lockouts, weekly rewards, vendor and bounty rotations, events, weekly modifiers; deterministic
    or configurable scheduling.
29. **Modifiers** - burns, Heroic, Epic, Juggler, Lightswitch, Angry, Specialist, Small Arms,
    Exposure, Match Game, Grounded, Airborne, Trickle, Chaff and build-specific ones.
30. **Events** - public events, heroic public events, Taken invasions, Court of Oryx, Archon's
    Forge, Warsat, Walker, extraction crews, faction conflicts, timed world encounters.
31. **Special modes** - Prison/Challenge of Elders, Court of Oryx, Archon's Forge, Trials, Iron
    Banner, SRL, festivals, Crimson Days, Queen's Wrath, Armsday, other limited/legacy activities;
    which content is still physically present in the final build.
32. **DLC** - base game, The Dark Below, House of Wolves, The Taken King, Rise of Iron:
    entitlements, story, destinations, activities, vendors, items, quests, raids, strikes,
    subclasses, level/Light changes, system changes.
33. **Cinematics, dialogue, music** - cinematic triggering (story, intro, expansion, character),
    skipping, completion flags, replay; mission/Ghost/vendor/ambient/radio/strike/raid/event
    dialogue; music transitions.
34. **Checkpoints, death and revival** - story/strike/raid/mission checkpoints, re-entry, wipe
    reset, weekly reset, fireteam sharing; death, respawn timers, restricted respawn, darkness
    zones, Ghost and fireteam revives, raid revive limits, wipe detection, checkpoint reload.
35. **Postmaster and Vault** - overflow detection, lost items, capacity, retrieval, deletion,
    packages/messages; vault storage categories, limits, deposit/withdraw, account-wide access.
36. **Kiosks and cosmetics** - exotic/shader/emblem/ship/sparrow/quest kiosks, abandoned quests,
    legacy recovery; shaders, emblems, ships, sparrows, horns, ghost shells, appearance, emotes;
    sparrow behaviour and racing.
37. **Grimoire and trophies** - unlocks, cards, score, triggers, persistence; trophy triggers,
    preserved through a replacement system where platform integration is gone.
38. **Platform dependencies** - PSN/Xbox Live, friends, achievements, invites, voice, commerce,
    entitlements, matchmaking, identity: which must be replaced for PC preservation; entitlement
    checks and their client effect.
39. **Errors** - a database of every error: name/code, trigger, request, expected behaviour, known
    fix, origin (client, backend, networking, activity state).
40. **Backend logging and tooling** - timestamp, client, account, character, endpoint, request,
    response, status, session, activity, errors, state mutations; verbose mode without touching
    core logic. Capture, decode, store, compare, replay sessions; diff 01.00 vs 01.24 and
    successful vs failed runs, highlighting changed fields.
41. **Test automation** - repeatable tests for startup, authentication, characters, inventory,
    vendors, orbit, Director, activity launch, mission completion, rewards, persistence,
    reconnection, multiple characters, multiplayer; pass/fail with logs. Soak tests.
42. **Build comparison** - structured 01.00 vs 01.24 (and others): endpoints, request/response
    structures, manifests, added/removed systems, auth, progression, inventory, activities. Do not
    rediscover known differences.
43. **Player database** - durable schema for accounts, characters, items, inventory, equipment,
    vault, quests, objectives, bounties, progression, reputation, currencies, activity history,
    checkpoints, unlocks, Grimoire, vendor state, social, sessions, rewards; documented,
    reproducible migrations; transactional writes, backups, validation, recovery, snapshots,
    export/import, test profiles; never depend solely on one mutable database.
44. **Multiplayer** - multiple independent clients authenticate, coexist, see each other, join
    fireteams, enter activities together, synchronize state, revive each other, receive individual
    rewards, return to orbit, reconnect. Architecture designed for this even while testing with
    one client.
45. **PC route** (separate workstream; no speculative native-port work mixed with proven
    emulator/server work) - emulator requirements, CPU/GPU, graphics, input, controllers,
    keyboard/mouse, audio, shader compilation, performance, networking, filesystem, save paths,
    platform APIs, launcher, configuration, resolution, framerate, widescreen, stability.
46. **Stability** - crashes, deadlocks, memory, network stalls, server exceptions, disconnects,
    loading hangs, activity-load failures, save corruption, races, multi-client scaling.
47. **Documentation** - README, architecture, server/API, protocols, endpoints, database, build
    differences, status, known issues, test matrix, activity/quest/vendor matrices, item system,
    setup, PC/emulator setup, development environment, reverse-engineering notes, references.

## Status vocabulary and matrices

Every feature carries exactly one status: UNKNOWN, NOT INVESTIGATED, INVESTIGATING, BLOCKED,
PARTIAL, WORKING, VERIFIED, REGRESSION - with description, build tested, dependencies, current
implementation, known failures, source files, captures, last test date, next action. Separate
matrices for story missions, patrol zones, strikes, raids, Crucible maps and modes, quests,
bounties, vendors, public events, special activities and DLC content. No category is complete
until every entry has been tested.

## Evidence priority

1. Captures from the original live game. 2. Behaviour of preserved functioning builds. 3. Game
binaries and reverse engineering. 4. Manifest/static data. 5. Historical recordings/screenshots.
6. Archived API documentation. 7. Community technical research. 8. Inference. Observed and
inferred behaviour are always labelled; guesses are never presented as facts.

## Working method

Multiple agents (Claude, Codex, others) work here. Before significant work: inspect recent
commits and branches, read the status documentation, check whether another agent owns the
subsystem; document as you go; commit logically separated changes; do not overwrite recent work
without understanding it; review experimental-model work. For each incomplete feature: establish
the exact failure; capture logs/network; locate client/server code; compare with known-working
behaviour; smallest testable hypothesis; instrument; smallest useful change; automatic test;
compare; document; regression coverage; commit; next blocker. No large speculative rewrites. Each
feature moves DISCOVER -> UNDERSTAND -> IMPLEMENT -> TEST -> VERIFY -> DOCUMENT -> REGRESSION TEST.

## "Fully functional" means

Complete startup/login; persistent accounts; all classes; inventory/equipment; progression;
vendors; quests; bounties; economy; Director; orbit; patrol; story; strikes; raids;
multiplayer/fireteams and Crucible to the intended preservation scope; rewards; checkpoints; DLC;
social/presence equivalents; resets/rotations; all state survives restarts; automated regression
tests for major activities; content systematically tested; deviations documented. Booting or a few
working activities is not "fully functional".

Target stack: game client + emulation/compatibility layer + preservation backend + player database
+ activity/session services + multiplayer support + static/manifest data + test automation +
development/RE tools + complete technical documentation.

## Multiplayer-first requirement (added 2026-10-02)

Everything is engineered from the beginning for multiple accounts, characters, concurrent clients,
PCs and emulator instances, orbit, fireteams, shared patrol spaces, matchmade activities, strikes,
raids, Crucible, private activities, join-in-progress, reconnection, presence, inspection, shared
and per-player state, simultaneous activity and public-space instances, matchmaking, session
discovery, allocation and teardown, host migration/replacement, and cross-process/cross-machine
components. No subsystem may assume one user, character, activity, world instance, connection,
inventory, session, checkpoint, quest state, vendor state, identity or global mutable game state.
Principles: no singleton player; session isolation; instance isolation; server authority where
required (never blindly trust client state); concurrency safety (transactions, locking, optimistic
concurrency, idempotency, retries, duplicates, reconnects); clean network boundaries between
components; stable deterministic identifiers; multi-client testability (simulated clients where
real ones are not yet possible). Every feature also records multiplayer implications, current
single-client limitations and readiness: NOT DESIGNED, SINGLE-CLIENT ONLY, MULTIPLAYER-AWARE,
MULTI-CLIENT TESTED, MULTIPLAYER VERIFIED. SINGLE-CLIENT ONLY is technical debt unless isolated as
marked temporary experiment code. Multiplayer-capable content is tested solo, 2, 3 and full
fireteam size. Feature lifecycle: DISCOVER -> UNDERSTAND -> DEFINE STATE OWNERSHIP -> DESIGN FOR
MULTIPLAYER -> IMPLEMENT -> SINGLE-CLIENT TEST -> MULTI-CLIENT TEST -> VERIFY -> DOCUMENT ->
REGRESSION TEST. Architecture, scopes, authority and the debt register:
[`architecture-multiplayer.md`](architecture-multiplayer.md).
