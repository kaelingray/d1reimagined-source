# ARCHITECTURE - Destiny 1 as a system, and what our server must be

**Canonical reference, consolidated 2026-08-11** from five source documents now retired to
`historicaldocs/design/` (see the Sources table at the end). Banners in those sources override their
bodies; every claim a banner killed is listed in "Dead claims" below and is NOT restated as live
anywhere in this document.

**State:** the whole 38-state ladder walks cleanly, `orbit -> 28..36`, and `activity:in_world`
(rung 36) has been reached three times (run `20260809-230309-FAM02-OLIVE`, activity 407,
`pvp_moonbase`, zero `error.*`). A type-9 biped now stands in-world (2026-08-11). The frontier is
off the ladder: possession/control, and earning the remaining synthetic pieces onto the wire.
Current-state docs: `design/GUARDIAN-IN-WORLD.md`, `status/TODO.md`.

**Evidence conventions.** `[C]` confirmed · `[O]` observed, not reproduced · `[R]` reported by
Bungie · `[I]` inference · `[U]` unknown. Reach tags per `CLAUDE.md`. Ownership: **CLI**
client-local · **SRV** our server · **BUN** a Bungie service · **PLT** platform (PS4/PSN/emulator) ·
**CNT** content packages · **PEER** another console.

> ## The reach fact that colours every section
> Everything past `/SignOn` was reached on bytes we invented, and everything past `content_check`
> additionally requires a client patch (`patches/CUSA00219-lab-manifest*.xml`). Every "served" and
> "built" below means **the client's own decoders and predicates are satisfied**, never that a real
> Bungie service ever did it that way. All of it is `synthetic_ancestor`. The single exception: one
> unauthenticated `/SignOn` probe against live Bungie infrastructure (§3.2), the project's only
> `observed-live` datum.

## Contents

1. §1 The system in blocks and tiers (B1-B33)
2. §2 The three concurrent state machines
3. §3 The boot-to-in-world chain: the 38-state ladder, requesting sites, the FNV-1 oracle
4. §4 The silent-failure index (S1-S21, complete; L1-L19 extension)
5. §5 The server surface: what we owe vs what we deliver
6. §6 The as-built server roles (rungs 28-36)
7. §7 Talks context: what Bungie said about activity architecture
8. §8 Open `[U]`s and unresolved contradictions
9. §9 Dead claims
10. §10 Sources

---

## §1 - the system in blocks and tiers

33 blocks, tiers 0-5. Provenance: historicaldocs/design/THE-BLOCKS.md §0-§1.

### 1.1 Where the model comes from - five independent decompositions

Where two agree, a block is `[C]`:

| source | what it gives | size |
|---|---|---|
| RTTI class names (`capture-rig/eboot-types.tsv`) | the static type decomposition | 2,633 classes, incl. **134 `*_game_system`** and **35 `c_world_controller_state_*_handler`** |
| the diagnostic catalogue (`capture-rig/diagnostic-catalogue.tsv`) | the runtime-logging decomposition Bungie hand-wrote | 117 subsystems, 2,125 strings, with evidence classes |
| the state name table | the ladder | 38 states |
| the core-engine talk, GDC 2015 - the FINAL HYBRID ENGINE slide | which components are new vs inherited from Halo: Reach | 26 components, colour-coded |
| the mission-architecture talk, GDC 2015 | the cloud tier, in Bungie's own vocabulary | Activity Host · Activity State · Sensors · Auth/Sense State · Bubble · **Worldserver** |

The engine slide's colours, decoded `[R]`:

- **BLUE - preserved from Halo: Reach, then extended (4):** `Networking` · `User Interface` ·
  `Camera` · `Game Logic (Players, Bipeds, Weapons, ...)`
- **GREEN - evolved and changed (4):** `Application Lifecycle` · `Animation` · `Pathfinding` · `AI`
- **ORANGE - new for Destiny (18):** `Server Communication` · `Progression Systems` ·
  `Object System` · `Reflection System` · `Resource Management` · `Streaming System` ·
  `Core Content Pipeline` · `Scripting` · `Memory Manager` · `Multithreading` · `Frame Loop` ·
  `Core Renderer Architecture` · `Rendering Features` · `Effects System` · `Spatial Subdivision` ·
  `Foundation Libraries` · `Content Import Features` · `Tools & Editors`

Three consequences `[R]`:

1. **`Networking` and `Server Communication` are separate components.** Our Layer B / Layer C split
   reproduces Bungie's own component boundary; the P2P transport is literally Halo: Reach's stack,
   which promotes the 2011 networking talk to a spec source.
2. **`User Interface` is BLUE** - the CUI layer is Halo: Reach's UI system, extended, which is why
   it is a retained-mode pull model (B19).
3. **`Progression Systems` is a top-level component**, a peer of `User Interface`. "Investment" is
   an architectural tier in Bungie's own diagram.

### 1.2 The tiers in one view

```
TIER 0  LOCAL BRING-UP (rungs 1-4, PLT/CNT)      B1 media check / bootload / intro / start
TIER 1  IDENTITY, ENTITLEMENT, CONTENT (5-18)    B2 platform identity · B3 entitlement ·
                                                 B4 SKU manager · B5 /SignOn bootstrap ·
                                                 B6 manifest gate · B7 package registration ·
                                                 B8 content DB / investment globals
TIER 2  THE BACKEND (orange Server Communication) B9 BAP transport (slot 0 OUT PRIMARY) ·
                                                 B10 QueueZ replication · B11 investment
                                                 server-message channel · B12 AH broker ·
                                                 B33 THE WORLDSERVER
TIER 3  THE CLIENT'S OWN STATE                   B13 investment registry · B14 investment user
                                                 record · B15 world tag loader · B16 THE
                                                 SIMULATION QUEUE (95 element slots) ·
                                                 B17 world-controller ladder · B18 ui_stage ·
                                                 B19 CUI data-component layer · B20 world /
                                                 geometry loader · B20b slice-set transition mgr
        == THE ACTIVITY WALL ==                  (rung 27 orbit stable; 28-36 now all reached)
TIER 4  SESSION, PEERS, ACTIVITY HOST (28-35)    B21 platform session · B22 managed session ·
                                                 B23 session parameter blackboard · B24 STUN/NAT ·
                                                 B25 matchmaking · B26 ACTIVITY HOST (a cloud
                                                 SERVICE, not a peer) · B27 peer mesh
TIER 5  THE SIMULATION (blue Game Logic)         B28 game systems + determinism engine ·
                                                 B29 entity replication/authority · B30 player
                                                 spawning/bipeds · B31 activity scripts/sensors/
                                                 objectives (cloud Lua, never shipped) ·
                                                 B32 incidents/telemetry (xetrov)
```

Three structural claims this model makes (historicaldocs/design/THE-BLOCKS.md §0):

1. **Three separate machines run at once** and the project had only ever modelled one: B17 (what
   the game is doing), B18 (which screen is up), B19 (what the screen says). See §2.
2. **The block that owns "which character am I playing" - B33, the Worldserver - is one Bungie has
   never published anything about.** The mission-architecture deck (pages 7 and 132) draws it in the
   cloud with no edges at all, declaring it out of scope both times. Our eboot names it too:
   `client_to_world_server_request/_response` is the real name of the 205/503/104/701 wrapper. `[R]`
3. **Almost nothing on the boot path is an RPC.** The activity to launch, the ladder's goal, the
   AH's address are all session parameters; the character, inventory, unlocks, director inputs are
   all replicated objects. Only three things are genuinely request/response: `/SignOn`,
   `manifest.bin`, and the BAP handshake. A model built around "which endpoint do I implement" will
   mis-shape the whole server.

### 1.3 The block table - tiers 0-1: local bring-up, identity, content

| # | block | owner | consumes -> produces | have it | if empty: what breaks, how loudly |
|---|---|---|---|---|---|
| **B1** | Media check / bootload / intro / start | PLT+CNT | disc + engine config -> a running engine, mounted packages | yes | **LOUD.** `RELEASE_VHALT`, and the guest uploads a crash ZIP whose `crash_info.txt` names the cause in prose. |
| **B2** | Platform identity (PSN/NP) | PLT+SRV | `sceNp*` polls, PSN OAuth -> online id, platform user key, `id_token` | yes | **SILENT-ish.** Pre-Layer-A the client polled `sceNpGetOnlineId -> SIGNED_OUT` thousands of times and never opened a socket. `[C]` |
| **B3** | Entitlement / content rights | **PLT (local, in orbit)** | `sceAppContentGetAddcontInfoList` or `/SignOn` f10 -> 32-bit offer keys | partial | **SILENT - the canonical case.** Leaves on `normal_flow` with an empty list; no `error.*`, ever. Two backing stores selected by `ownership_authority`: `content_check` sets 1 (server, from signon f10 - never sent by us), `prepare_for_orbit` sets 2 (local) => our server is off this path for the entire activity ladder. `[C]` See S1 + L17 correction: the pinned run records keys `0xE0000002` (paid) and `0xE0000001` (full sku), twice each. |
| **B4** | SKU manager | CNT (`sku_config.txt`) | AES-128-CBC + RSA-signed blob -> sku type, silo routing, endpoint names | partial | **QUIET.** Prints `finalizing sku_type=2 (north_america_digital)` and moves on. Signed over the ciphertext, unconditionally => cannot be edited; the redirect seam is DNS. `[C]` |
| **B5** | Sign-on bootstrap (`/SignOn`) | BUN <- **SRV** | HTTPS POST -> the security token, the two 16-byte channel keys, the BAP address, the manifest URL (f4.3), silo, content build, external IP, owned offer keys (f10) | yes | **LOUD, but with a lying string.** `Unable to parse signon response` means field 2 absent, not malformed bytes - the parser's return value is discarded and the consumer runs over a zero-initialised struct. `[C]` The only block with an `observed-live` datum. |
| **B6** | Content manifest gate | BUN <- **SRV** | `manifest.bin` -> a validated content generation | patched | **LOUD.** `error.content_manifest_not_provided`, codename termite; predicate is literally `url[0] == 0`. RSA-2048 over the payload against one compiled-in key => unforgeable. We flip one data byte - the origin of the `synthetic_ancestor` taint on everything downstream. `[C]` |
| **B7** | Package registration + Resourcerer | CNT | manifest + installed pkgs -> registered packages, resolvable names | yes | **LOUD BUT MISATTRIBUTED.** The manifest only decides which `.pkg` files get opened - names live inside the files, resolved by scanning package symbol tables, newest build timestamp wins (that IS the patch-override mechanism). `[C]` With a 2-byte manifest the client accepts it, registers zero packages, and dies 90 s later in a different state. |
| **B8** | Content DB / investment globals | **CNT** | packages -> definition tables addressed by content tag / ordinal | yes (76% opaque) | **SILENT.** The most load-bearing architectural fact for our server: **definitions are local content; the service sends only state.** Why "send the director its contents" is a category error, and why 76% Oodle compression bounds what we can read. `[R]` the gear-production talk (SIGGRAPH 2014) draws the same line: a Gear Slot is a content unit, "not a customization and investment unit." |

### 1.4 Tier 2: the backend

| # | block | owner | key facts | have it | failure mode |
|---|---|---|---|---|---|
| **B9** | BAP transport | **SRV** <- BUN | TCP 3075, AES-128-GCM, 6-byte envelope + 6-byte base header + class-sized extras -> 58 typed messages. The correlation model is **two in-flight queues examined at the HEAD ONLY** => replies must arrive in order per queue, matched on `(expected-reply-type, sequence)`. | yes | **LOUD, then FATAL.** `error.no_bap_connection` (Centipede); `error.lost_bap_connection` (weasel). A wrong header length is SILENT - a resumable, offset-counting read with no timeout and no log. `[C]` |
| **B10** | QueueZ replication (families 0-7) | **SRV** | server->client state: subscribe -> baseline + delta -> objects in the registry. Only a **baseline-flagged** update promotes a family 2->3, and only state 3 applies objects. State 1->2 is written **locally at send time**. `0x000D`'s own body handler is a no-op `ret`. | yes | **SILENT IN FIVE WAYS** (S3-S6, L1). |
| **B11** | Investment server-message channel | **SRV** <- BUN | client->server commands: a CUI event (`vfunc[21]`) -> a typed request -> a response handler. Ids **501** create / **502** delete / **504** login / **505** signout; delete and signout do not send on `vfunc[21]` - a confirm dialog and `vfunc[14]` sends. RTTI names the whole set (`c_create_character_server_message_component`, `c_login_character_server_message_component`, etc.), dispatched through `cui::c_investment_server_message_request_handler`/`_response_handler`. `[C]` | built 2026-08-08, default OFF | **SILENT.** No UI event, no message, no complaint. |
| **B12** | Activity-Host broker | BUN <- **SRV** | msg 6/7 (the request literally carries a **process command line**) -> a 64-bit AH id; msg 16/17 -> an IPv4 + port. What comes back is a **SECOND BAP ENDPOINT** - slot 1 `OUT FAH` or slots 2/3 `OUT GAH0/GAH1`, not multiplexed onto the primary. `[C]` | partial | **LOUD at the client, SILENT at us.** The address resolver drops a reply silently unless the echoed `activity_host_id` matches. |
| **B33** | **The Worldserver** | **BUN** | persists the account, characters, progression, inventory - the RPG. Our 205/503/104/701 messages; real RTTI name `client_to_world_server_request/_response`. | we impersonate it | **SILENT, AND UNDOCUMENTED BY CONSTRUCTION.** `[R]` Named once in the whole 4,954-file talks archive, drawn with no edges, declared out of scope. Every character/inventory/progression question in this project is a Worldserver question, and the eboot is the only source that exists. |

### 1.5 Tier 3: the client's own state - where the interesting failures live

| # | block | key facts | have it | failure mode |
|---|---|---|---|---|
| **B13** | Investment registry | the client-side object store: `family x type x slot`. 4/0 account · 4/1 profile · 4/2 (director dep) · 4/3 inventory · 4/4 unlocks · 3/0 roster · 3/1 select-card · 2/0 name+clan · 2/1 player card · 0/1 `[U]` · 5 root. | yes | **SILENT.** An object present in the registry is not an object anything reads (S7). Family 0/1 (`0x808017FD`, 20 x `0xA48`) is `[U]`. |
| **B14** | Investment user record - "which character am I playing" | tag=3 · platform key · slot A = account SOID · slot B = the RESOLVED PROFILE, written per-tick from `account.field[3]` (a u64 at C offset `+0x68`) resolved against family 4/1. **NO UI IN THE LOOP.** Slot B empty => the resolution ladder (`0x78C5F0`, returns 0/1/2/3/6; 6 = fully resolved) never returns 6 => `character:signin` parks on the select screen and pressing X does nothing. `[C]` `observed-live` | yes, working | **SILENT** (S9). Skipping the select screen is the signature of the character having been established successfully, not of a missing character. |
| **B15** | World tag loader (content demand) | decides which content tags must be resident for the current view + character. **Eight `fill_desired_*` strategies, ONE PER VIEW**: character_instance · select_character_view · peer_view · default_gear · character_creation · talent_grids · rich_presence · activity_reward_incidents. `[C]` from RTTI | partial | **SILENT.** A view you never enter never asks for its content (S10). |
| **B16** | **The simulation queue** - the deterministic event bus | **95 element slots** (see §9: 115 was the `c_investment_bap_message_*` class count, a retracted transposition). Table `0x2F8` bytes at `DAT_026708c8`, array of pointers stride 8, valid indices 0..94; allocator `0x23de00` and memset both take `mov edx, 0x2f8` in `0x2ff590`; setter `0x2ff640`, getter `0x2ff660`, dispatcher `0x2ff7c0`. **59 distinct populated slots enumerable from the 47,304-file decompiled corpus, max index 94 = 0x5E, zero slack** (59 is a floor, not the type count - a registrar that computes its index cannot appear in a textual sweep). Everything from outside enters the simulation here: the profile (`..._set_character_profile_nondeterministic_chunk`), family replication start/stop, world-tag cache state, every AH message (join_result, membership, entity_slots_allocation, global_state, advance_replication_epoch, purge, incident), entity authority change/claim/handoff, slice-set transitions, UI triggers. `[C]` from RTTI | modelled (see §5.7) | **SILENT BY CONSTRUCTION.** "The bytes are in the registry" and "the simulation has seen them" are different statements. |
| **B17** | World controller state ladder | 38 states, 35 handler classes, per-frame `update()` at vtable `+0x38`; **self-driving** - each rung's own update requests the next. The goal is not a state: it is a session parameter re-checked every frame (B23). | yes (read) | **LOUD.** `state_manager: Entering/Leaving state '%s' for reason '%s'` is unfiltered. `cleanup` is a normal transit state - read the reason. `[C]` |
| **B18** | The `ui_stage` machine | which screen is up - a second, separate machine: stage-loading goal 0..5 + substage goal -1..34. | read, never driven | **QUIET AND SELF-HEALING.** Prints every transition but `%s` name lookups resolve to `ENUM(N)`; on protocol error it says "To gracefully recover..." and continues. `[C]` `observed-live`. Three docs give three different mappings for which substage is character-select - do not infer the screen from the number. |
| **B19** | CUI data-component layer | turns game state into what the screen says. **126 pull-model components, each with `update()` at vtable slot 14** (`c_orbit_data_component` 49 callees · `c_investment_character_statistics_component` 50 · `c_investment_backpack_component` 65 · `c_cui_investment_character_state_component` 45 · `c_investment_login_info_component_v2` 32 - the last is the character-card builder). Zero diagnostic strings for `inventory` or `progression`, no `investment:` root at all. `[C]` | untraced | **STRUCTURALLY INCAPABLE OF A LOUD FAILURE** (S12). Nothing pushes; a component reading nothing renders zero, forever. |
| **B20** | World / geometry loader | a `world_launch` command (6,392 B; the world is named by a STRING at `+0x630`, not a hash) -> a loaded world. | yes | **LOUD and well-named.** `world_controller:` distinguishes every outcome by name. Loading a world != entering an activity. `[R]` changing destination is one of a handful of full-engine serialization barriers. `[C]` |
| **B20b** | Slice-set transition manager | regions, precache, teleport, public matchmaking, host delegation - the talks' bubble swap: T-15 s precache -> T-10 s matchmake -> T-5 s abort deadline -> T-0 swap. A bubble is simultaneously the unit of simulation, of asset streaming, and of matchmaking. `[R]` | no | **NEVER RUN.** 118 catalogue rows, 101 `strong`, zero seen - the largest never-executed subsystem in the client. |

### 1.6 Tier 4: session, peers, activity host

| # | block | key facts | have it | failure mode |
|---|---|---|---|---|
| **B21** | Platform session | `POST /v1/sessions` (multipart, boundary `BUNGIEr0x0rz`) -> a session id + a 78-byte opaque peer descriptor. **D1 never calls `sceNpMatching2`** (0 game-side reachers vs a control where 32/38 libraries have them) => the PSN Session WebAPI over plain HTTP. `[C]` | partial | **LOUD.** `error.platform_session_not_failed` - "You have lost connection to the Destiny servers", plain English, not an animal codename => the platform layer. |
| **B22** | Managed / network session | session 0 = FIRETEAM, session 1 = POSSE, plus group and target; peer list `_joining -> _waiting -> _ready -> _established`. | yes (solo) | **LOUD.** `networking:session:membership:dump` prints the roster on every change. `peer_name=<UNKNOWN PEER NAME>` = family 2/0 (display name) empty, surfacing in a different subsystem's log. `[O]` |
| **B23** | Session parameter blackboard | ~45 replicated typed parameters incl. `activity`, `activity_host`, `desired_activity`, `initial_slice_set_status`, **`world_controller_goal`** (a PERMISSION re-checked every frame, not a destination) and `s_activity_host_connection_information` - **the AH address is a session parameter.** Activity launch is a distributed decision; with one peer it looks local, a trap for two-instance work. `[C]` | partial | **SILENT.** If the goal ever reverts the rung bails to `cleanup` immediately. |
| **B24** | STUN / NAT / Demonware lobby | 16 `stunN.*` hosts -> an external address + NAT type. | partial | **QUIET AND TOLERANT - both degrade rather than fail.** "Failing back to using our local address ... defaulted to moderate NAT type"; "Demonware lobby connection not desired, continuing without" - the lobby is optional. `[C]` `observed-live` |
| **B25** | Matchmaking | skipped entirely when the activity definition's `[+0xA9] == 0` - **21 of 540 activities (indices 393-413)**, the private ones. Confirmed live. `[C]` | bypassed | **LOUD when it runs.** |
| **B26** | **The Activity Host - a cloud SERVICE, not a peer** | started by BAP on the session host's request - the request carries a **process command line**. 49 name slots / 44 wired ids. Owns Activity State. `[R]` **two kinds**: one MISSION host per fireteam, one BUBBLE host per public bubble => a player is connected to >=2 cloud hosts at once. AHs are not fault-tolerant: "you get kicked to orbit and you get one of those wacky animal name error reports" (spoken Q&A, in no deck). | stub built (§6) | **LOUD at the boundary, then EMPTY inside.** ~30 named `error.private_activity_host_*` reasons; rung 30 fails by timeout, recoverable to orbit. **The offline seam is silent:** with no AH the client `memset`s its entity pool to `0xFF` and **self-grants all 12,288 entity indices** (S15). |
| **B27** | Peer mesh | election, membership, migration, DTLS - **PEER-LOCAL, DO NOT BUILD.** Zero service calls. The AH only scores the election (+10000), admits via reservations, evicts, and introduces behind NAT. Session ceiling 20 peers. Host migration every 160 s in retail. `[R]` six layers: game code -> game interface -> prioritization -> replication -> channel manager -> transport. `[C]`/`[R]` | no | One `[R]` tension: "The Activity Host is the session authority, who arbitrates election of new Physics Hosts" - both can be true (election arithmetic local, reconcile authoritative); unreconciled (§8). |

### 1.7 Tier 5: the simulation

| # | block | key facts | have it |
|---|---|---|---|
| **B28** | Game systems + determinism engine | 134 ordered `*_game_system` classes bracketed by `omaha_game_first`/`omaha_game_last`. | reached 2026-08-10+; at consolidation of the sources, never observed running `[U]` |
| **B29** | Entity replication / authority | `[R]` verbatim: "the host has almost all the authority ... authoritative over object health, damage events, projectile positions ... Clients can exert some limited authority over their own bipeds." Three protocols - **state** (eventual latest, most of the bandwidth), **events** (no guarantee at all), **control** (~20 bits/player, hand-packed) - and those three ARE three C++ classes (`c_replication_*_view`). Not lockstep, not dedicated servers. `[R]`+`[C]` | no |
| **B30** | Player spawning / bipeds | entity index + character binding + spawn transform -> a controllable biped. Players arrive by sim-queue element `0x0D` (`historicaldocs/design/player-creation-chain.md`). **This is the frontier.** Open `[U]`: does a player entity instantiate with an empty inventory? If not, gear is a hard spawn prerequisite even though it is provably not the director blocker. | in progress |
| **B31** | Activity scripts · sensors · objectives | **BUN - cloud-side Lua, never shipped.** `[R]` "none of our Lua logic for Activity Scripts are executing on the PS4 client." Auth State down / Sense State up at 10 Hz, batched-atomic; O(50) sensors declared per mission, O(20) live per bubble. HUD objective banners are driven down from a sensor - objectives are not client-authored. **STRUCTURALLY IMPOSSIBLE TO RECOVER FROM THE CLIENT.** | never |
| **B32** | Incidents / telemetry (xetrov) | 123 `*_incident_metadata_*` classes. `client_to_xetrov_request` is **one-way with no reply; answering it is a bug.** `[C]` | deliberately unanswered |

### 1.8 The honest ceiling

(historicaldocs/design/THE-BLOCKS.md §5.) Four ceilings, nearest first: **(1)** the AH schema codec
- published AH sizes are in-memory struct sizes, not wire sizes (NOW FALLEN: the oracle-backed codec
landed and passes 28/28 controls, §5.2 U18); **(2)** the empty world - B31 is cloud Lua, never
distributed; **(3)** opaque content - 76% of package blocks are Oodle LZH/LZHLW with no public
decoder (721 of 998 blocks in `globals_pvp_0169`); **(4)** six participants for 3v3. "3v3" itself is
not attested in the owned build - `maxPlayers 6` comes from a 2020 manifest, four years newer.

**PvP is the exception and is why the goal is PvP.** A Crucible match is mostly B27+B29+B30 with the
AH as scorekeeper and admissions desk `[I]`, well supported. `[R]` a private match is a
private-bubble chain under a single Mission Activity Host, no stranger matchmaking, one fireteam -
the simplest configuration of the architecture. `[R]` entering an empty bubble: the player spins up
the simulation in its default state from local content, is elected Physics Host, and receives one
`reconcile()` call - with zero sensors that reconcile is empty, which is precisely the "loaded but
empty world," reached by the game's own intended path. One-sentence version: **we can plausibly
rebuild everything that carries the game and none of what is the game - except in PvP, where the
players are the content.**

---

## §2 - the three concurrent state machines

Provenance: historicaldocs/design/THE-BLOCKS.md §0/§1 (B17/B18/B19).

Three separate machines run at once, loosely coupled; the project had only ever modelled the first:

| machine | what it tracks | shape | observability |
|---|---|---|---|
| **B17 world-controller ladder** | what the *game* is doing | 38 states, 35 `c_world_controller_state_*_handler` classes, self-driving per-frame `update()` at vtable `+0x38`; the goal is a session parameter re-checked every frame | LOUD - `state_manager: Entering/Leaving state '%s' for reason '%s'`, unfiltered |
| **B18 ui_stage machine** | which *screen* is up | stage-loading goal 0..5 + substage goal -1..34; a separate machine from the ladder | QUIET, self-healing: prints "Caller failed to request a stage-load before changing the substage-goal ... To gracefully recover..." **and carries on** `[C]` `observed-live`, run `20260808-213619-UNLOCKS-ALL`; name lookups print `ENUM(N)` because the compiler kept the hash and dropped the string |
| **B19 CUI data-component layer** | what the screen *says* | 126 components polling `vfunc[14]` every frame; a retained-mode properties/binding model (Halo: Reach's UI, extended `[R]`) | **silent by construction** - zero diagnostics; a component reading nothing renders zero forever |

The coupling is loose enough that the ladder can advance while the screen machine is out of step.
Verification at the CUI layer must be memory or a probe on `vfunc[14]`, never the log.

**"Which character am I playing" exists at three layers** (historicaldocs/design/THE-BLOCKS.md §3):

| layer | the concept | who sets it | state |
|---|---|---|---|
| identity / investment (B14) | the resolved profile object in slot B | a per-tick resolver, from `account.field[3]` | working |
| CUI card list (B19) | the `c_static_vector<cui::s_login_character_info, 10>` at `+0x2E0`, sorted on `+0x38`, built by `vfunc[14]` `0xA110F0` from `account + 0x08` count and `account + 0x10 + i*8` SOIDs. (The byte at `c_investment_login_info_component_v2 + 0x88`, sentinel -1, is a local-player/controller index - its only consumer `FUN_00589610(idx)` opens `if (idx < 4)`; it is written by the CUI property setter `vfunc[8]` `0xA11000`.) Answering `login_character` (504) does NOT set it - the 504 path touches only its own object. | never runs on our path - we always report >=1 character |
| activity / in-world (B26) | a character SOID on the wire, in the activity-client join request: `identity:[account %lld][character %lld][machine ...]` | sent to the AH at rung 30+ | now sent (as-built, §6) |

"Which character" is re-established, in a different representation, at each layer; getting layer 1
right says nothing about layers 2 and 3. The measurement that makes the two boot configurations
mutually exclusive: an 88-byte family-4 baseline with no profile object fires message 504 and never
reaches orbit; a baseline with the profile reaches orbit and never fires 504. `[C]` Character
CREATION - the only code in the game that grants starter gear (its own comment: it "keeps character
creation, starter inventory and the prologue mission off the critical path") - is deliberately
skipped by pushing a post-prologue character; the route to gear is to grant what creation would have
granted. Orbit's character panel draws from a third, unnamed CUI source: the select-card chain
(family 3/1: level at `+0x68`, light from the 16-entry stat table at `+0x870`, class/race/gender at
`+0x08..0x0A`) does not run in orbit, and family 4/1 is the in-world source; candidates are the
`vfunc[14]` bodies of `c_orbit_data_component` / `c_cui_investment_character_state_component` /
`c_investment_character_statistics_component` / `c_investment_progression_data_component` /
`c_investment_backpack_component`. `[C]` they exist; `[U]` what they read.

---

## §3 - the boot-to-in-world chain

Provenance: historicaldocs/design/THE-CHAIN.md. Two superseding banners on that document are honored
throughout: **(a)** rung 36 `activity:in_world` HAS BEEN REACHED, three times - every "rungs 35/36
never observed / never executed" statement in the source body is stale; **(b)** "the black screen is
a shadPS4 texture-cache defect" is NOT the explanation for the black screen in an activity - at
`activity:in_world` there were **577 live sobjects across 8 types and ZERO of type 9 (`biped`)**;
there was never anything to draw. The texture-cache warnings are real and irrelevant. (The banner's
own earlier `gg + 0xD0` framing is itself retracted: that block is memset by design; players arrive
by sim-queue element `0x0D`.) See §9.

### 3.1 The 38-state ladder, complete `[C]`

Name table at `0x192D2A0` (38 entries, pointers in `R_X86_64_RELATIVE` addends), cross-checked
against the 22 `bootflow:` string literals.

```
     LOCAL                     -- states 1-4  --  no server
  1  bootflow:media_check
  2  bootflow:bootload
  3  bootflow:intro_screen_display
  4  bootflow:start

     IDENTITY + CONTENT        -- states 5-18 --  Layer A + HTTPS
  5  bootflow:platform_signin              PSN OAuth
  6  bootflow:platform_account_warnings
  7  bootflow:enumerate_content_rights
  8  bootflow:account_signin               HTTPS /SignOn
  9  bootflow:eula          (conditional)
 10  bootflow:upsell        (conditional)
 11  bootflow:email         (conditional)
 12  bootflow:confirm_account_transfer (conditional)
 13  bootflow:content_check                manifest.bin
 14  bootflow:content_install (conditional)
 15  bootflow:insufficient_content_space (conditional)
 16  bootflow:illegal_dlc_media (conditional)
 17  bootflow:queuing                      (the signon queue; conditional)
 18  bootflow:package_registration         1033 packages

     THE GAME BACKEND          -- states 19-22 -- Demonware BAP + investment
 19  bootflow:bap_signin                   TCP 3075, AES-GCM
 20  bootflow:investment_signin            QueueZ families
 21  bootflow:profile_setup                LOCAL wizard (NOT character select)
 22  bootflow:prepare_for_orbit

     CHARACTER + ORBIT         -- states 23-27
 23  character:signin
 24  character:prologue        (skips itself; substage 5 requests cleanup)
 25  cleanup                   a NORMAL transit state - read the REASON
 26  setup:orbit_loading       slice-set / destination
 27  setup:orbit               stable

     ACTIVITY LAUNCH           -- states 28-37 -- ALL NOW REACHED (banner; was "never reached")
 28  setup:activity_session_creation
 29  setup:matchmaking
 30  setup:activity_host_setup      <- the Activity Host: a fourth server role
 31  setup:prologue_intro_loading
 32  setup:orbit_outro
 33  setup:activity_world_transition
 34  activity:initial_slice_set_loading
 35  activity:physics_join
 36  activity:in_world         <- THE GOAL CONDITION, reached 3x (2026-08-10)
 37  activity:watch_video
  0  pregame
```

(States 9-12 and 14-16 are conditional interstitial/error branches hanging off state 7's region and
are not on our path; the signon field-8 bits select the interstitials: bit 2 `0x04` eula, bit 4
`0x10` email, bit 5 `0x20` upsell, bit 6 `0x40` confirm_account_transfer - §5.8/6.3 of the ledger.)

### 3.2 Stage facts worth keeping (chain order)

- **States 1-4** local. `bootload` requested at `0x79C9B0` (jmp) in `0x79C980`. `[C]`
- **States 5-6** platform sign-in: `sceNp*` + PSN OAuth over HTTPS
  (`auth.api.np.ac.playstation.net/2.0/oauth/token`), answered by `d1-server`. Destiny's PSN
  *session* is `sceNpWebApi` HTTP (`POST /v1/sessions` etc.), not a native call. `[C]`/
  `synthetic-generic`. `[U]` W1: what a real PSN `id_token` contains. A shadPS4 defect lived here:
  `sceVoiceCreatePort()` declared with no parameters returned `ORBIS_OK` while discarding the
  out-pointer - 156 calls/boot, never a port; fixed in our fork.
- **State 8 `/SignOn`**: the client posts and gets back config, environment, silo id, external
  address, and the BAP server address. The eboot carries `signon.deadorbit.net` and
  `signon.gravityshavings.net` (both NXDOMAIN in 2026); the live host `www4.signon.deadorbit.net` is
  not in the 01.29 eboot - the runtime host list comes from `sku_config.txt`. Key addresses `[C]`:
  request descriptor `0x18F8B40` (struct `0x6D6`) · response descriptor `0x18F8CE0` (struct
  `0x7B8`) · parse loop `0xA17D0` · field-type handler table `0x18ED0A0` (16 slots) ·
  `process_response` `0x2A99D0` · f2.6 `bap_server_ip` `movbe this+0x41F0` at `0x2AA574` · f2.7
  `bap_server_port` at `0x2AA584`, both consumed by
  `bap_connection::resolve_transport_address` `0xB2B840`. HTTP handling `0x2ABCC0`: **503 and 509
  back off retries immediately - never answer `/SignOn` with either.** f4.3 (the manifest URL) MUST
  be emitted; the magic and keys are raw `u32` LE `fixed32`, not varints (`0xA2180` is
  `read_fixed32`: `mov edx, 4`).
- **The only `observed-live` datum** `[C]` `observed-live`: `www4.signon.deadorbit.net ->
  205.209.23.9:443`, TLS 1.3, CN `*.signon.deadorbit.net`, issuer "Destiny Secure Server Authority
  1", valid 2025-05-08..2027-05-08 (renewed in 2025). `POST /SignOn` (empty body) -> HTTP 200, 42
  bytes of protobuf: `{1: 2, 7: {1:5, 4:"Error"}, 12: 1786216877 (a live clock), 13:
  "D2_ProdRetail:Pending"}`. The service is up and identifies as Destiny 2; whether a D1 title still
  gets D1 routing is `[U]` (retail-console question).
- **State 13 `content_check`**: fetch + validate `manifest.bin` from the key-100 URL (engine-config
  accessor `0x34D980` `+0xA84`, from `get_content_manifest_url` `0x2A8970` at `0x2A89AB`). The
  predicate is literally `url[0] == 0` at `0x79A3FF`; failure `mov ebx, 0x7d` at `0x79A4B8` (byte
  pattern `bb 7d 00 00 00` occurs once in the 26 MB text segment). The signature is unforgeable:
  `0x2B0030` gates on the byte at vaddr `0x18C4974` (file offset `0x18C8974`); `body[8..0x108)` is a
  256-byte RSA-2048 signature over `body[0x108..]` against one 270-byte DER key at `0x1591800`;
  registrar `0xEEDD0` rejects the 17-byte "Ling-Ling's head" placeholder and caps the list at one.
  We route around it with the lab patch - the taint origin. `[U]` W3: a real `manifest.bin`;
  `cdn.deadorbit.net` resolves and is directly fetchable, untested.
- **State 18 `package_registration`**: entered from `0x79AFB2` (in `0x79AD00`) and `0x79EC93` (in
  `0x79EC20`, the **queuing** handler - not profile_setup), the latter gated on signon state == 3.
  `bootflow:queuing` (17) is the signon queue: state 3 -> package_registration, state 2 sub 1..2 ->
  back to account_signin, sub 4 -> error dialog; `0x78CD00` is
  `signon_manager::get_status(&kind, &sub)`.
- **State 19 `bap_signin`**: TCP 3075, AES-128-GCM (measured; NOT the SCTP/DTLS shape once inferred
  from `bd*` symbols). Handshake `0x1e->0x1f`, `0x19->0x1a`, `0x79->0x7a`, `0x12e->0x12f`. The whole
  boot is 10 BAP message types: `0x00FB` echo (131/run) · `0x000B` websocket (24) · `0x007B` QueueZ
  push (5) · `0x000D` subscribe response (3) · `0x0013` client_config (2, `[U]` W5 what a real one
  carries) · `0x0021` user message (2) · `0x0016` upo (1) · `0x0018` acct_id_trans (1) · `0x007A`
  (1) · `0x012F` (1). `lost_bap_connection` fires when any message sits unanswered at a queue head.
- **State 20 `investment_signin`**: the client subscribes to object families and the server pushes
  over QueueZ. Exit predicate `[C]`: `0x79BFF0` needs `h[+0x30]==7 && h[+0x34]==3` - an account
  record with tag==3 bound to the local platform user, a non-null account object with its ready byte
  set, and a profile blob. The critical path is hand-rolled bit/byte (RSAT), not protobuf. All five
  families reach `state=3 REPLICATING` only when each subscription is answered per-subscription
  right after the `0x000D` ACK (`D1_ANSWER_FAMILIES`); a one-shot `D1_PUSH_FAMILY` cannot
  substitute (it fires at seq 7; classes 0/2 subscribe at seq 20/21). Five gates fell here
  (fireteam NULL crash / `platform_session_not_failed` / `family_record_not_found` via shared nested
  struct `0x80801941` / `lost_bap_connection` x2 on `0x0017`+`0x0012` / `Failed to decode [policy
  1]` on 104) - and three of the five fixes were already written and merely unwired.
- **State 21 `profile_setup`**: NOT character select - a 7-step local UI wizard at `0x79E830`
  (screen bounds -> brightness -> subtitles; ui substages `0x16`/`0x15`/`0x17`). Sends no message.
  Skippable: `[0x26B4070+0x38] = 7` skips all three screens, confirmed live.
- **State 22 `prepare_for_orbit`**: requested from `0x79E886` (wizard done) and `0x79C0ED` (in
  `0x79BFF0`, when `0x57E490() != 0`, i.e. wizard skipped).
- **State 23 `character:signin`**: requested at `0x79E486` (jmp) in `0x79E450`, gated on
  `h[+0x134] in {5,6}`. The gate was the account schema's field 3 - the profile SOID at C `+0x68`.
  Machinery: ladder `0x78C5F0`, substage machine `0x7A0160` with jump table `0x7A0364`, roster
  count `0x78C9F0`, goal write `mov [r15+0x44], 7` at `0x7A01DF`. `[U]` I1: in-game "change
  character" crashes; no backtrace ever captured (`capture-rig/harvest-crash.py` exists, unused).
- **State 24 `character:prologue` skips itself**: substage is 5 exactly when the u16 index is
  `0xFFFF`; the jump table at `0x79FEB8` covers only 0..4 and substage 5 is the only branch that
  requests a transition - to cleanup (`0x79fe89: mov edi, 0x19`). All 546 records via `0x548860`
  carry `+0x18 == 0x811C9DC5` (the empty sentinel) => the prologue can never launch.
- **State 25 `cleanup`**: 48 requesting sites across 30 functions; a normal transit state.
  `0x782720(state, reason, ctx)`: reason < `0x10` is the non-error class; >= `0x10` kicks to
  `bootflow:start`. Entry points: `0x782710(state)`, `0x783690(state, reason)`, `0x782720`.
- **State 26 `setup:orbit_loading`**: handler `0x7AC6C0`; `0x752AE0` is `orbit_map_loader`. The one
  boot-path field is `last_orbited_destination` (u32 hash, character view `+0x480`); unset is a
  tolerated failure falling back to two configured default destinations - why we reach orbit with a
  blank character. There is no "Orbit" destination hash in the manifest; do not put the orbit
  activity tag `0x80B52001` into a destination-hash field.
- **State 27 `setup:orbit`**: requested at `0x7AC6B4` (jmp) in `0x7AC630` via `0x783690`. Reference
  run `20260807-225145-INV104-ORBIT`: 19 minutes sustained, zero `error.*`, 188 BAP messages, 0
  decode failures.

### 3.3 The transition census - every rung and its one requesting site `[C]`

`capture-rig/state-request-census.py`, 6/6 controls PASS, 87 sites, 25 of them tail calls. Read with
the source's corrections: **the "inside" column is the handler of rung N-1** - the ladder is
self-driving, each rung's own `update` (vtable `+0x38`) requests the next (so `0x7A6D60` IS rung 28,
`c_world_controller_state_setup_activity_session_creation_handler::vfunc[7]`, and it requests 29).
Rung 28's requester is `0x7AB460` inside `0x7AAF60` = orbit's own update (found by decompilation;
`mov edi,0x1c` tail-merges into a call site shared with rung 25, invisible to an immediate scan).
`0x797420` is positively eliminated (bounded by `cmp edi,0x12 ; bt 0x62000,edi`, reaching only
13/17/18). Rung 26 remains `[U]` for a stated reason: it is entered through cleanup's stored goal
state (`[this+0x6C8]`), so no immediate exists to find. And rung 28's `+0x118`/`+0x128` (and
cleanup's `+0x6A0`/`+0x6B0`) are **64-bit bitmasks over a fixed task table**, not queue pointers -
rung 28 waits for 7 named tasks to report done (function pointers at `this+0x38`, readable live).

| -> state | requested at | inside | gate on that edge |
|---|---|---|---|
| 2 `bootload` | `0x79C9B0` (jmp) | `0x79C980` | - |
| 5 `platform_signin` | `0x79F090`'s region | `0x79F090` | reads engine config `+0x534` (a timeout) |
| 17 `queuing` | `0x79A787`, `0x79B0CD` | `0x79A380`, `0x79AD00` | content/install progress |
| 18 `package_registration` | `0x79AFB2`, `0x79EC93` | `0x79AD00`, `0x79EC20` | signon state == 3 |
| 19 `bap_signin` | `0x79CD0B` (jmp) | `0x79CCD0` | - |
| 20 `investment_signin` | `0x798041` (jmp) | `0x798000` | - |
| 21 `profile_setup` | `0x79C0ED` | `0x79BFF0` | `0x57E490() == 0` |
| 22 `prepare_for_orbit` | `0x79E886` and `0x79C0ED` | `0x79E830`, `0x79BFF0` | wizard done or `0x57E490() != 0` |
| 23 `character:signin` | `0x79E486` (jmp) | `0x79E450` | `h[+0x134] in {5,6}` |
| 24 `character:prologue` | `0x7A0208` | `0x7A0160` | - |
| 25 `cleanup` | 48 sites | 30 functions | error sinks + the normal transit |
| 26 `setup:orbit_loading` | `[U]` - entered via cleanup's stored goal `[this+0x6C8]` | - | - |
| 27 `setup:orbit` | `0x7AC6B4` (jmp -> `0x783690`) | `0x7AC630` | - |
| 28 `setup:activity_session_creation` | `0x7AB460` | `0x7AAF60` (orbit's own update) | - |
| 29 `setup:matchmaking` | `0x7A6E7C` (-> `0x783690`) | `0x7A6D60` | - |
| 30 `setup:activity_host_setup` | `0x7A9ECE` | `0x7A9C20` | - |
| 31 `setup:prologue_intro_loading` | `0x7A636C` | `0x7A5250` | - |
| 32 `setup:orbit_outro` | `0x7ACFB5` | `0x7ACD90` | - |
| 33 `setup:activity_world_transition` | `0x7ACBBF` (jmp) | `0x7ACB90` | - |
| 34 `activity:initial_slice_set_loading` | `0x7A7E36` | `0x7A7C00` | - |
| 35 `activity:physics_join` | `0x795165` (jmp) | `0x795130` | - |
| 36 `activity:in_world` | `0x7964EE` | `0x796460` | - |

`[C]` **There is no config-driven "boot directly to state N" in this build** (control passed on both
halves: only 6 of 50 transition-owning functions read engine config at all, and all six read a
timeout that leads to `cleanup`). And a timeout is never a hop: per-state engine-config timeouts
raise reasons at or above the error threshold and restart the boot (S18).

### 3.4 The FNV-1 name-hash oracle `[C]` `static-client-inference`

The client's name hash is **FNV-1 (32-bit), NOT FNV-1a** (multiply THEN xor, basis `0x811C9DC5`,
prime `0x01000193`; at least one site lowercases - `tolower` at `0x64E20`; loop `0x1DC7F5`-
`0x1DC83F`, sku table `0x25C1040`). Evidence with controls (pinned eboot sha256 `7271fcb9...`,
`d1-eboot controls` 10/10 PASS): 7/7 sku option names cross-validated against decrypted plaintext;
corpus test on the eboot's 37,548 identifier-like strings scores FNV-1 at 184 hits (0.490%) vs a
length-matched random control at 32 (0.085%) - a 5.75x (~27 sigma) excess - while FNV-1a scores
exactly the control rate (pure noise, refuted). Base rate for a random u32 appearing in the image:
0.130%.

The structural proof: a compiled enum->name-hash table at `0x9CD4A7`-`0x9CD523` builds 8
`{u32 index, u32 name_hash}` entries into `[0x274DA80]` and registers with `0xA8C920(table, 8)`:
idx 0 `0x2CA33BDB` = fnv1("none") · 1 `0x4247521A` = fnv1("controller_signin") `[I]` · 2
`0xE9D46191` = fnv1("platform_signin") `[C]` · 3 `0xEFA4CF17` = fnv1("account_signin") `[C]` · 4
`0xE28BECEF` `[U]` · 5 `0x783BF8AD` = fnv1("content_check") `[C]` · 6 `0x837DEB9C` =
fnv1("checking_achievements") `[I]` · 7 `0xBEE20D1C` `[U]`. `d1-eboot xref 0xa8c920 --kind call`
finds **172 call sites** => ~172 such tables; `d1-eboot nametab` decodes **168 of 172, 1,080
`{index, name_hash}` entries** into `capture-rig/eboot-namehash.tsv` (only 12 resolve to a spelling
- the compiler keeps the hash and discards the string; the artifact gives `index -> hash`, which is
what you need to TEST a candidate name).

Names whose string is absent from the eboot but whose FNV-1 hash is present as an instruction
immediate: `patrol` `0xD07BB2D7` (`v:0x4894C5`) · `arena` `0x288BE48A` (`v:0x9D93FA` + 4 more) ·
`trials_of_osiris` `0x63C30386` (`v:0x9DB2CE`) · `salvage` `0xE8BBD8FC` (`v:0x3DC37D`) · `rift`
`0xA2DFA680` (`v:0x3DC35F`; rift/salvage/control are three consecutive `mov [rip+d], imm32` - a
game-mode name-hash global block) · `orbit` `0xB084DD2B` (`v:0x7579BE` in `0x757690`, global
`[0x26B09F0]`) · `tower` `0x6796A864` (`v:0x9E4C80`). This is the tool for the
`last_orbited_destination` hash space and for content-index blanks.

**Two hash spaces - conflating them is what lost the finding once** (historicaldocs/design/
THE-CHAIN.md §4.3): space 1, NAMES (enum members, bootflow states, error reasons, sku options,
world/mode names; `0x274DA80` + ~171 siblings, reason table `0x26B22A0`) is FNV-1 and **cracked**;
space 2, the TYPE-NAME REGISTRIES (6,338 + 1,332 runtime type tags; registry A `0x19B3530`,
registry B `0x19D0BD0`) is **not any known function** - 118M trials plus a ~10^8 substring sweep
with injection and scramble controls, all at the noise floor; the hashes are baked by the schema
compiler and the names are not in the binary. Naming space 2 needs an external symbol source.
⛔ **Corrected 2026-08-11:** registry B is `0x19D0BD0`; `0x19D0BD8` is the *rejected* `{u32 hash,
pad, u64 ptr}` framing hypothesis (`design/queuez-replication.md` §4.2), i.e. the base read 8 bytes
high - the same off-by-8 shape `status/LESSONS.md` already warns about.

Composite (2-part `A_B`) meet-in-the-middle cracks have a **42% false-positive rate per target**
(300 random targets -> 126 spurious); single-token lookups scored 0/300, so single-token hits are
`[C]` and composite results are `[I]`.

### 3.5 The wall that was - director / destination selection (historical)

Between orbit and rung 28 the project spent four sessions at "the director draws Earth and nothing
is selectable." Kept because the mechanism is load-bearing: **the director subsystem emits NO
diagnostic strings at all** (a `director*:` scan returns nothing), and `[R]` the talks' framing -
all Activity Scripts run in the cloud on Activity Hosts, always the authority over Activity State -
means a blank director is the *expected* behaviour of a client with no activity service. The live
chain into the wall (`[C]` disasm + `observed-live`): a family-4 update announces itself
(`0x56D1A0`/`0x56E7B0`/`0x576100` -> `0x574440(family)`; validity byte cleared via `vtable+0xC8` =
`0x260BD0`) -> the singleton `0x26A34B0` is refreshed (`0x574220(4)`/`0x574340(4)` through pure
setter `0x2609A0`) -> a consumer asks for availability (`0x25FF40` -> `0x260A10` -> freshness
`0x2609C0` -> builder `0x260200`) -> the builder runs and succeeds (`{1,1,0}` with `--abs`) -> the
director UI filters and appends (`0x9D92A0` builds `cui::c_director_activity`, sticky "do not
offer" bit `0x8000` OR-ed at `0x9D9AAE`/`0x9D9DC4`/`0x9D9FB2`; `byte[node+0x100]` set at `0x9D9E4F`
only after `0x574220(4)` non-NULL and `[node+0x138] != 0` - the one server-controlled input;
`0x9E6448` appends only if bit `0x8000` clear). The famous "cache built then destroyed" claim was
the `--abs` instrument trap and is dead (§9). What actually moved the ladder: the unlocks object
(4/4, instance checksum `0xC4060975`), `D1_ANSWER_FAMILIES=0,2`, and the served membership fix -
see §6 and `design/GUARDIAN-IN-WORLD.md`.

### 3.6 The activity-launch seam

- `0x753540` = `world_controller` change-world entry; its diagnostics distinguish every outcome by
  name (`successfully changed world to: %s` SEEN 104x orbit, 206x ambient_bootstrap).
- The `world_launch` command, one captured specimen `[C]` `observed-live` (bytes): 6,392 B
  (`0x18F8`), self-describing size at `+0x0000`; 50 x 24-byte activity slots from `+0x0010` (stride
  `0x18`: u32 activity hash - all `0x811C9DC5` = fnv1("") = UNSET in the specimen, u32 `0xFFFFFFFF`,
  16 zero bytes); boundary record `{0x811C9DC5, 0}` at `+0x04D0`; u16 `0xFFFF` sentinels from
  `+0x059E`; **the world name as NUL-terminated ASCII at `+0x0630`** ("ambient_bootstrap") - the
  world is named by a STRING, not a hash; `0x80` entries from `+0x0650`. Arithmetic wobble,
  unadjudicated: 50 x `0x18` from `+0x10` ends at `0x4C0`, not `0x4D0` - either the count is 51 or
  the boundary is off by one slot.
- **`0x753540` is never called directly at runtime.** It is command-slot 3 of a queued dispatch:
  registry singleton `0x34C140` (= `mov rax, [0x267B110]`), registrar `0x34BDC0(reg, slot, handler,
  secondary, r8, r9)` (per-slot buffer at `+0x200 + slot*0x1C04`), the only registrar caller
  `0x752ED0` (17/17 registrations), pump entry `0x751060`, dispatcher at **`0x34C0BC`** in
  `0x34C010` (`lea rdi, [r14+rax+0x200]; call [r14+r13*8+0x80]`). Slot 0xA (`0x754320`) builds a
  command and calls `0x753540`; slot 0xB (`0x754460`) takes an already-built one. The slot-3 buffer
  address is derivable: `0x205036210` = `0x20503B81C - 0x200 - 3*0x1C04` `[I]`.
- The client's own AH vocabulary (`world_controller:activity_manager:` strings) names the whole
  rung 28-36 protocol: requesting AH startup, connecting to AH ID, join request/response, waiting
  for entity indices, ready/waiting for instantiation, waiting for session parameters, waiting for
  the registered peer mask, host-migration during startup. The BAP message classes come from intact
  RTTI: 67 `c_investment_bap_message_*` classes including
  `client_to_bap_get_activity_host_proxy_request/_response`,
  `client_to_activity_host_manager_request/_response`,
  `bap_to_activity_host_socket_handoff_request`, and `client_to_world_server_request/_response`.

### 3.7 Appendix A carried forward - the refusal ladder

Reason ids index the table at `0x26B22A0` (stride `0x10`, `{const char *name; u32 fnv1_hash}`, 284
entries, 260 of them `error.*`).

| stage | refusal | id | on-screen codename |
|---|---|---|---|
| platform_signin | `sceNpGetOnlineId: SIGNED_OUT` (a poll, not an `error.*`) | - | - |
| account_signin | `Signon server did not report a silo id` / `...content build version` | - | - |
| account_signin | `Unable to parse signon response` - means field 2 absent, NOT malformed bytes | - | - |
| account_signin | `Client config decoding failed. Check for invalid overrides.` (`0x160CF00`) | - | - |
| account_signin | `Signon denied: reason [%d]` | - | Squirrel (platform-account problem) |
| account_signin | transport | - | Marionberry = `error.transport_method_invalid` |
| content_check | `error.content_manifest_not_provided` (`0x1645D12`) | `0x7D` | termite |
| content_check | `error.content_manifest_url_invalid` | `0x7E` | toad |
| content_check | `error.content_manifest_invalid` / `_guid_mismatch` / `_signature_failure` / `error.no_cdn_connection` | `0x7C` / `0x7F` / `0x80` / `0x7B` | - |
| package_registration | `package id %d is out of range, can't register` (`0x164D5BC`) | - | - |
| package_registration | `Patchable package registration failed with result %s (%d)` (`0x164D536`) - prints a blank name | - | - |
| bap_signin | `RELEASE_VHALT / failed to load bootstrap_patchable` · `resourcerer failed during unload` | - | - |
| bap_signin | `failed to load client startup globals` / `...bootstrap activity` / `...investment globals` | - | - |
| bap_signin | `error.no_bap_connection` - never connected (10 s timeout) | `0xAD` | Centipede |
| bap_signin | `error.lost_bap_connection` - came up, then torn down | `0xAE` | weasel |
| bap transport | `update_receive() failed to find recipient for header` => means ORDERING | - | - |
| bap transport | `Failed to decode [policy N]` => means the BODY was wrong | - | - |
| investment_signin | `error.no_investment_connection` (30 s, cfg `+0x554`, key 157) | `0xAF` | chicken |
| investment_signin | `error.investment_server_object_family_record_not_found` | `0xB0` | - |
| investment_signin | `...instance_type_from_checksum_not_found` / `...instance_not_found` | `0xB1` / `0xB2` | - |
| investment_signin | `...queuez_id_mismatch_in_header_and_rsat_payload` / `...version_out_of_order` | `0xB3` / `0xB4` | - |
| investment_signin | `error.investment_account_version` | `0xB5` | - |
| any state | shared guard `0x7914E0`, bits 0..19 - bit 17 `0x558AA0(0) != 3` -> 174; bit 19 -> `0xAF` | - | - |
| fireteam | `error.platform_session_not_failed` - "You have lost connection to the Destiny servers", plain English, not an animal => the platform layer | - | - |
| the wall | nothing. The director emits no diagnostic string of any kind. | - | - |
| activity launch | `world_controller: no activity specified...` / `failed to activate activity` / `load successful, but failed to instantiate` | - | - |
| PvP (ahead) | `networking:investment:peer: All peers do not have valid account SOIDs...` - that exact string has still never printed | - | - |
| activity setup (rung 30) | `error.investment_peer_manager` - the `investment:peer:` family is LIVE and reachable at rung 30 (printed 59x in run `20260809-223241-TASK7-SUPPRESS` as `Failed peer-subscription validation [error error_timeout][account_soid 0x...]`), correcting the row above's "never printed" filing for the family (the specific SOID-validity string is still unseen). Time-dependent; once olive has fired, later attempts in that session are poisoned, not measurements - reboot. **CURED 2026-08-10 by `D1_ANSWER_FAMILIES=0,2`** (`peer failures: 0`). | - | OLIVE |

The two-gate diagnostics trap applies to every row: 626 of 1,614 diagnostic sites already pass the
default filter and still emit nothing, because the sink at `0xE4AD0` is a ring buffer with no output
call. Absence of a string is evidence only for the ~1,540 rows `design/diagnostic-catalogue.md` §2
classes as `strong`. And rung 35 is the documented silent hang (S19): `activity:physics_join` is a
150-tick countdown with no timeout, no failure path, no `error.*` at all.

### 3.8 Appendix B carried forward - what our server must answer, by stage

`historicaldocs/design/message-catalogue.md` is the authority for message id -> schemas -> whether a response is
expected; `design/eboot-map.md` §3-§4 for the family/type model. This is the chain-ordered
must-answer list only.

| stage | endpoint / message | our obligation |
|---|---|---|
| platform_signin | ShadNet TCP 31313 | the server speaks first - `ServerInfo` is a bare `u32` LE version. `Login`, `GetToken`, `GetServerFeatures` |
| platform_signin | PSN OAuth, TLS 443 `auth.api.%s.ac.playstation.net` | token with an `id_token` |
| account_signin | HTTPS `POST /SignOn` | HTTP 200 + raw protobuf. f2 with all of f2.1-f2.7; f5 silo id; f6 content build version; f2.6/f2.7 = the BAP address |
| content_check | `http://<key-100 URL>/manifest.bin` | prefer `http://`; RSA envelope routed around by the lab patch |
| package_registration | - | a 1033-package manifest (`capture-rig/manifests/full_en.bin`) |
| bap_signin | TCP 3075 | `0x001F` (142 B, status `0x00C8`, 128-byte payload mandatory) · `0x001A` (sealed, frame 114 B) · `0x007A` (bodyless) · `0x012F` (bodyless) · `0x00FB` (bodyless keepalive) |
| bap_signin | secured channel | AES-128-GCM, 12-byte nonce, tag PRECEDES ciphertext, no AAD, per-direction counters |
| investment_signin | `0x000D` subscription response | bodyless - MUST answer. Unanswered, it is the weasel |
| investment_signin | `0x0013` client_config | NOT header-only - 2 required varints |
| investment_signin | `0x0016` upo · `0x0018` acct-id-translation · `0x0021` user message | `0x0016` works header-only; `0x0021` needs a real body (648-byte object) |
| investment_signin | `0x007B` QueueZ push | the object-delivery channel |
| investment_signin | investment 503 · 504 · 205 · 701 · 702 · 104 | 205 and 503 share nested struct `0x80801941`; its field `[0]` is the family-5 root, which must be `0x7FFFFFFFFFFFFFFF` in both |
| investment_signin | objects: 4/0 account · 4/1 profile · 3/0 roster · 3/1 character | the account object's field 3 (`+0x68`) = the profile SOID is the character gate |
| - | `0x00AB` `client_to_xetrov_request` | one-way telemetry. No reply exists. Do not answer. |

The silence law: answering wrongly is worse than not answering, and `N -> N+1` pairing is a
regularity, not a rule - `0x567EB0` is the literal constant: `policy->vtable[0x30]()` returns the
expected response type, and 0 means no reply exists (13 of 33 policy families plus all 13 push
policies). Short-payload trap: the client's bitstream reads past the end of a short buffer and
returns zeros; size the payload from the schema's own field count, never from what decodes. The
entire server-settable configuration surface is ONE `0x3298`-byte block at `*(void**)0x267B1B8`,
1,129 live keys - and `0x1DB2E0` resets the whole block to compiled-in defaults before applying the
stream, so a client_config delta is a full snapshot, not a patch.

---

## §4 - the silent-failure index

Provenance: historicaldocs/design/THE-BLOCKS.md §2 (S1-S21, complete) and
historicaldocs/design/SERVER-SURFACE-LEDGER.md §2 (L1-L19). The thesis: a block that fails loudly
costs one grep; a block that *succeeds emptily* costs a night. The rule: **a block that succeeds
emptily is not a bug in the block - it is a missing observable.** Name, in advance, the one thing
you will read to distinguish success from empty success, and read it in the same breath as declaring
the gate passed. Corollary, learned expensively: a retraction that lives in one document while three
others still assert the dead claim is itself a silent failure.

A meta-correction the source carried at its own head: two of the four canonical silent failures the
commissioning brief named were themselves retracted claims that outlived their measurement - "a
director availability cache stamped itself valid after failing" (verdict dead, mechanism alive: the
header IS stamped valid on both paths `[C]`, but the builder succeeds `{1,1,0}` while the director
is on screen; the `{1,0,0}` reads caught the early-boot invocation of a thrice-run function) and "a
QueueZ family ACKed 56 subscriptions and quietly timed out to a state that refuses all future
updates" (ACK-then-timeout measured `[O]`; "refuses all future updates" is `d1guest.py`'s own
annotation, never measured).

### 4.1 S1-S21, complete

| # | block | how it succeeds emptily | the observable that tells them apart |
|---|---|---|---|
| **S1** | B3 entitlement | `enumerate_content_rights` enters and leaves on `normal_flow` with zero entitlements. No `error.entitlement_enumeration_failed`, ever. `[C]` **Corrected by L17/§6.7 of the ledger: the pinned run is NOT empty** - it records `0xE0000002` (paid tier) and `0xE0000001` (full sku), twice each; only add-on keys are missing, a shadPS4 defect (`app_content.cpp:27` ships a 16-byte all-zero key). | `ownership_manager: recording locally determined offer key 0x...` - count the lines. Zero = broken; two = healthy. |
| **S2** | B7 package registration | a 2-byte manifest is accepted, zero packages register, the client dies 90 s later in a different state. `packages: 0` is legal for parsing. | `Patchable package registration failed with result %s (%d)` prints a blank name - the mapper is an ICF-folded stub returning `""`. Use the package count, or the crash ZIP. Now guarded by a launcher refusal. |
| **S3** | B10 QueueZ - family never leaves state 2 | only a baseline-flagged update promotes 2->3, and only state 3 applies objects. A family at 2 accepts, logs, discards. State 1->2 is written locally at send time - "it subscribed" tells you nothing about the server. | `d1guest descriptors` - `state=3` or it did not happen. Never infer replication from the responder's send log. And do not "fix" it by sending `0x000D`: that handler is a no-op `ret`. |
| **S4** | B10 QueueZ - `(type, root)` miss | the family-slot lookup walks 257 slots, finds no match, and returns 0 with no string of any kind. Rooting family 2 at the character instead of the account made the failure quieter. `[C]` | "Fewer errors" is not progress on this path. Read the slot table out of memory and check the `(type, root)` pair exists. |
| **S5** | B10 QueueZ - bitstream pre-gate | a bad bitstream is rejected before the family state machine runs; the only log is a generic "failed to process update" from a caller that never invoked family logic. `[C]` | The message names the bitstream, not the family. Decode payloads in-stream, not standalone - the cursor and error flag persist across objects. |
| **S6** | B10 QueueZ - empty baseline | measured: `push:queuez: object-count of zero in [family_type ENUM(0) ... object_count 0]`, same for ENUM(2). The family replicates and carries nothing. `[O]` run `20260808-213619` | grep `object-count of zero`. Downstream tell: `peer_name=<UNKNOWN PEER NAME>` in the session membership dump (2/0 is the display name). |
| **S7** | B13 registry vs enumeration | an object is in the registry and nothing ever asks for it. Inventory is enumerated from **340 slots inside the profile**, not from the registry. `[C]` | Read the enumerator's input, not the registry. And `0x0000` is not "empty" - `0xFFFF` is: a zero-filled array is 340 copies of item index 0. |
| **S8** | B13 stat tables | two readers: one breaks on first key match; the other does not break and writes every non-`0xFF` entry into a key-addressed array - 15 zero entries overwrite Light with zero fifteen times. `[C]` | Sentinels: `0xFF` for a stat key, `0xFFFF` for a content index. A zero there is a claim, not an absence. Three sibling stat tables are left zero by default. |
| **S9** | B14 character resolution | slot B empty => the ladder never reaches 6 => the select screen parks. Pressing X does nothing - the exit is data-driven, not event-driven. `[C]` `observed-live` | Read the record: `tag==3`, slot A != 0, ready byte != 0, slot B != 0. Slot B holds a resolved object, not a raw SOID - poking a SOID opened the gate and crashed three frames later. |
| **S10** | B15 world tag loader | a view you never enter never requests its content. Eight per-view strategies; only visited views' run. `[C]` structure, `[I]` consequence | `[U]` with a mechanism: probe the eight virtuals. None has ever been observed firing - because nobody has looked. |
| **S11** | B16 simulation queue | data lands in the registry and is never enqueued into the simulation. `[U]` at the time of the source - **since refuted by the client's own control flow** (§5.7): elements 17/18 carry every BAP response and push into the queue. | The element types are named, the queue is a concrete object; read it (`d1guest read 0x2670900 --abs`). |
| **S12** | B19 CUI data components | the layer cannot fail loudly. It polls; reading nothing renders zero, every frame, forever. Zero diagnostic strings for `inventory` or `progression`; no `investment:` root at all. `[C]` | "A wrong or absent inventory will look exactly like a correct one, in the log." Verification must be memory or a probe on `vfunc[14]`, never the log. |
| **S13** | B18 ui_stage | prints "Caller failed to request a stage-load ... To gracefully recover..." and carries on. `[C]` `observed-live` | A recovery line is a finding, not noise. The numbers lie by omission: three docs, two name tables, three mappings for character-select - do not infer the screen from the substage number. |
| **S14** | B12/B26 activity host - three distinct silent drops | (a) the proxy-response echo is dropped silently unless `activity_host_id` matches - "it looks exactly like the server never answered"; (b) message 100's push policy gates on `recipient_id` - a wrong value is a silent drop, not an error; (c) a short header parks the receive path forever with no timeout, no log, no error branch. `[C]` | Assert the id at the emitter; require an observed AH-originated effect (an entity-index grant, a membership change), never "the frame was built." And never send udt 16 - a stub that acks its own keepalive lease is talking to itself, silently. |
| **S15** | B26 entity indices | with no AH the client `memset`s its pool to `0xFF` and self-grants all 12,288 indices - it proceeds as if admitted. `[C]` | The absence of `timed_out_waiting_for_entity_indices` is not evidence indices were granted. Check whether the grant came from the wire. |
| **S16** | B9 BAP framing | a wrong header length parks a resumable read with no timeout and no log; the `0x001f` stall (6 bytes where 8 were needed) produced total silence until a 10 s connect timeout. `[C]` | Always state the header class with the length and assert it in a test. `HEADER_LEN[class]` is the control. |
| **S17** | B5 signon | `Unable to parse signon response` does not mean malformed bytes: the parser's return value is discarded and the consumer runs over a zero-initialised struct. A byte-perfect protobuf that omits field 2 gives the identical four log lines; omitting field 1 entirely reads as `0 = success`. `[C]` | Assert on fields present, not on the absence of that string. |
| **S18** | B17 timeouts | a timeout is not a hop. Per-state engine-config timeouts raise reasons at or above the error threshold, so they restart the boot rather than advancing. "Setting key 997 does not skip the brightness screen; it restarts the client after N ms." `[C]` | Read the reason on every `cleanup`, always. |
| **S19** | rung 35 | the documented silent hang. `activity:physics_join` is a 150-tick countdown waiting for a loading-cinematic object to stop resolving. No timeout, no failure path, no `error.*` at all; if the cinematic never ends it counts negative forever. `[C]` | Nothing in the log will ever say so. The one rung where "no errors" is compatible with "hung permanently." |
| **S20** | cross-cutting - the diagnostics themselves | the log has two gates, and 626 of 1,614 sites already pass the filter and still emit nothing because the sink is a ring buffer with no output call. Widening the filter yields zero new lines. | Absence is evidence only for the ~1,544 rows classed `strong`. 81 are `weak-blind`, 324 are not `retail_log` sites at all. Cite the evidence class, never the raw SEEN bit. |
| **S21** | cross-cutting - our own instruments | a probe group that fails to arm yields zero records, indistinguishable from "never got there." A `d1guest read` without `--abs` returns zeros, indistinguishable from "torn down." A null-guard patch returned `al = 0xe8` - a silent false "lookup succeeded" - for a day. | A control must test the axis you are worried about: "three stable reads over ten seconds" is a control against a race; a wrong address is perfectly stable. Two instruments disagreeing is a finding, not noise. |

### 4.2 The ledger's extension - L1-L19 (server-side silent failures)

| # | the silent failure | discriminating observable |
|---|---|---|
| **L1** | **There is no QueueZ delta encoder.** `queuez_family_header_for` (`d1-bap/src/lib.rs:2409`) is the only family-header constructor and hardcodes `version = 0`, `flags = QUEUEZ_FLAG_BASELINE`. And **2/0, 2/1 and 0/1 have an UPDATE announcer (`0x56D3F0`) and no CREATE announcer (`0x56D290`)** - a baseline sets the registry bytes and notifies nothing downstream of a cache. `[C]` | Both delta-path strings are `strong`-class NOT-SEEN in 229 runs while same-function siblings are SEEN. Falsifiable prediction: baseline alone leaves the nameplate blank; baseline + a byte-different delta fills it. First delta after a version-0 baseline must be exactly version 1. |
| **L2** | every response and push rides a **silent allocator**: `0x300090` (`ecx=1`) emits five diagnostics; `0x2FFEA0` (`ecx=0`) suppresses all five, and sim-queue elements 16/17/18 all call the silent one. Element 18 has no diagnostic anywhere on its drop path. `[C]` | `d1guest read 0x2670900 --abs` gives counts and limits. Free positive control: sim-queue ids 86/87 are dev cheats that spawn an object through this exact bus. |
| **L3** | the 22 stubbed investment ids become silent successes the moment they decode: today the empty pad fails loudly (`bitstream error`, 76 occurrences / 48 logs); wire the oracle naively and they all answer nothing, cleanly. Paid twice already (206's zero body, 503's zero body). `[C]` | `warn!` a distinguishable marker on every id answered from a zero-body table. |
| **L4** | our `** NEW GATE **` logger is blind to method mismatches: axum `fallback` catches unmatched paths only; a routed path with an unrouted method returns a bare 405 with no log line. `[C]` | add `.method_not_allowed_fallback`; until then every `never_touched` verdict about a routed path is unfalsifiable. |
| **L5** | ✅ CLOSED 2026-08-09 - **BAP message 9 is the client RELAYING our own AH pushes back upstream** because we addressed them wrong: message 100's deliver step (`0x564C20`) reads class-6 header `+0x06` as a connection index and re-emits the body as message 9 on it; we sent `+0x06 = 0` = `OUT PRIMARY`. The responder used to log `** NAMED GATE ** no secured handler for inner type 0x0009` and drop 2 KB of ground truth per occurrence (`main.rs:3107`); observed frames 447 B and 1743 B, bodies exactly message 100's layout, udts 4 then 1. `[O]` | decoding msg 9 is our only direct observable for "did the AH push land in the client." As-built: `d1-responder` decodes inner type `0x0009` and `d1-activity-host` pushes 9, not 100. |
| **L6** | two components disagreed on `recipient_id`; a wrong value is a silent drop. ✅ SETTLED 2026-08-09: `state.rs` is right - `FUN_0075AB60` compares against `[ac+0xC98]` (the established AH id), or for udt 4 the pending join request's AH id, never an account SOID. | `D1_AH_RECIPIENT`'s default is wrong and matters only under `D1_AH_PUSH_100`. |
| **L7** | `D1_AH_HOST_TOKEN` (`0xD100_0000_0000_0A01`) and `FIRST_ACTIVITY_HOST_ID` (`0xD1A0_0000_0000_0001`) disagree by default; only `run-boot.sh:140` reconciles them. `0x594E70` drops a mismatched echo with no diagnostic. `[C]` | run the responder standalone and message 7 hands out a token no AH recognises - indistinguishable from "never answered." |
| **L8** | `D1_AH_HOST_TEXT`'s responder default (`<ip>:<port>`) violates the AH crate's own passing control: message 7's `host_text` is a **session id**, not an address (`registry.rs` asserts `!text.contains(':')`). `[C]` | two of our own components, one control, opposite answers. |
| **L9** | message 100 was mis-triggered, zero-headered and mis-addressed at once: the gate had no `inner_type` condition, no `conn_index` guard, and a one-shot latch firing on the first secured frame of any connection. `[C]` | fire behind `inner_type == 8` and `conn_index`; drop the latch. |
| **L10** | CLOSED 2026-08-11: `full_en.bin` is regenerable byte for byte by `d1-pkg manifest -o capture-rig/manifests/full_en.bin` (policy `all-generations --lang en`; sha256 pinned by `d1-pkg controls`, exit 2 on failure). Corrected counts: 1,033 = 1,647 - 608 language files - 6 engine files (3 ids x `_unp3_0` + `_unpatchable_0`); the legacy policies measure 393/175/1,644. `[C]` | was: if lost, the client boots with the wrong package set and dies 90 s later as `RELEASE_VHALT / failed to load bootstrap_patchable`, reading as a BAP regression - now a one-command rebuild, which matters because the file is gitignored. ⇒ `historicaldocs/design/SERVER-SURFACE-LEDGER.md` L10. |
| **L11** | `Package.f1` and signon f10 are an atomic pair: all 1,033 `f1` offer ids are literally 0 and `signon_success()` never emits f10. The moment `f1` carries a real DLC id, `is_offer_key_owned()` runs against `ownership_authority == 1` - the f10 list never sent - and packages vanish with no diagnostic. `[C]` | ship both or neither. ⭐ 2026-08-11: now **enforced in code, not remembered** - `d1_pkg::manifest::encode` takes no offer-id parameter, and `build` re-decodes its own output and hard-errors on any nonzero `f1`. A control asserts the detector can see a forged nonzero id, so the check is not a tautology. |
| **L12** | investment 305 and 1501 are span-carrying: a zero body there is a silent empty success on the character-select path, and `investment_message_response_with_body` cannot express a span (the presence bit at bit 4 makes the u16 length straddle byte boundaries). `[C]` | leave 305/1501 stubbed until a span-capable builder exists. |
| **L13** | QueueZ error codes 3 (no type-slot matches the checksum) and 4 (no free slot) emit no string; the sink `0x5747F0` is first-error-wins. `[C]` | an object with an unregistered checksum vanishes with zero output; a second failure in the same update is not even recorded. |
| **L14** | a byte-identical QueueZ redelivery calls nothing: `0x56B5F0` - `if new: CREATE; elif memcmp differs: UPDATE; else nothing`. `[C]` | never use a resend as a probe. |
| **L15** | a 131,072-byte ceiling on server pushes (`0x563590`: `< 0x20001`), with no guard in our code; max ever sent 20,520 B. `[C]` | one bounds check turns a future silent drop into a named gate. |
| **L16** | the responder's R2 deadline (2 s empty status-500 for an unanswered queue head) has NEVER fired in 223 runs - so "a wrong body is survivable; silence is not" is `[I]` for every id we do not answer 200. `[C]` | force one with `D1_SUB_ACK=0` on a throwaway run. |
| **L17** | `bootflow:enumerate_content_rights` is a data-collection step, not a gate - it cannot fail. `[C]` (This corrects S1.) | count the `recording locally determined offer key` lines: zero = broken, two = healthy. |
| **L18** | investment 205 is a stub wearing a valid shape: 25 bytes whose only non-zero content is `+0x08 = 0x7FFFFFFFFFFFFFFF`, against a 952-byte response schema (`0x80801827`). 79 wire occurrences, `rc=1`, no error, client proceeds. `[C]` | scoring it "served" hides it; it is demoted to stubbed. |
| **L19** | `c_cui_*` reads are a pull model, so "no announcement" != "never read" - but anything behind a cache is invisible for baseline-delivered family 0, 2 and 3/0 data. `[C]` structure, `[I]` consequence | verification here must be memory or a probe, never the log. |

---

## §5 - the server surface: what we owe vs what we deliver

Provenance: historicaldocs/design/SERVER-SURFACE-LEDGER.md (audit date 2026-08-08/09; the per-item
statuses are a snapshot of that date - the as-built §6 work has since moved several rows). Every
surface was enumerated from the client (RTTI, descriptor tables, dispatch tables, relocations,
strings), then audited against `d1-server/`, `capture-rig/` and 257 `runs/`.

### 5.1 The headline number, and its scoring rule

An item counts as SERVED only if (1) our answer carries a body derived from the client's own
tables/descriptors or from bytes the client itself emitted - not an invented constant, not an empty
frame - AND (2) it is on by default in `capture-rig/run-boot.sh`. Everything else is STUBBED,
BUILT-BUT-UNWIRED, NEVER-TOUCHED, or IMPOSSIBLE.

| # | surface | addressable | we answer at all | genuinely served | % |
|---|---|---|---|---|---|
| 1 | HTTP/HTTPS + shadNet commands | 85 (90 - 5 impossible) | 12 | **8** | 9 % |
| 2 | BAP transport message ids | 56 (65 - 9 Bungie-internal) | 14 req + 1 push | **9** | 16 % |
| 3 | Investment sub-ids inside BAP `0x000A` | 30 sendable (of 101 defined) | 8 | **6** | 20 % |
| 4 | QueueZ replicated `(family, type)` slots | 19 | 11 | **4** | 21 % |
| 5 | Activity-Host `user_defined_type`s | 49 (17 are ours to send) | 1 | **1** | 2 % |
| - | **de-duplicated request surface, rows 1-5** | **239** | 47 | **28** | **~11.7 %** |
| 6 | Content / manifest / entitlement (3 wire requests; the rest referenced, not sent) | 41 | 14 | **8** | 20 % |
| 7 | Simulation-queue element ids (delivery, not request) | 95 (12 server-causable) | 3 | **2** | 2 % |

The denominator is de-duplicated (`0x000A`'s 30 sub-ids, `0x0008/0x0064`'s 49 udts and `0x007B`'s 19
slots are counted once; the sim queue is excluded from the 239 entirely - it is the bus every wire
message lands on after delivery).

**The second, more useful number: of the ~40 request kinds the client has actually emitted in 257
runs, we answer 37.** 9 HTTP kinds answered 9; 3 shadNet commands answered 3; 17 BAP inbound ids
answered 16 (the one miss was inner type `0x0009`, dropped with `** NAMED GATE **` - see L5; now
decoded as-built); 9 investment ids answered 7 (501 and 505 answered with an empty body, measured
to kill the connection); 1 AH udt (`3 join_request`) received and kept opaque until the oracle codec
landed. **The gap is almost entirely LATENT** - it opens past rung 28, or with a second console, or
when a player picks up an item.

Census softness, stated: 54 of 101 investment names are `[U]` (content-side; all 104,933 eboot
strings and 57,947 identifier tokens hashed, zero hits); the authoritative runtime BAP codec table
is BSS (`0x26a0db0`, 304 entries), unread live; 13 of 17 AH->client udts had no schema tag derived;
71 of 74 investment-globals definition tables are unopened, and no real retail `manifest.bin` has
ever been seen; payload schemas for all 95 sim-queue elements are `[U]`.

### 5.2 Built but unwired - free capability (the project's signature failure)

Eighteen items existed, passed their own controls, and were switched off (statuses as of the audit;
several since wired):

- **U1** investment 501/502/505/506 response builders (`d1-bap/src/character_lifecycle.rs`, 782
  lines) behind `D1_CHARACTER_LIFECYCLE=1`, default OFF. **U2** `D1_ANSWER_FAMILIES=0,2` - defaults
  empty in responder AND `run-boot.sh:101` (this is the knob that later cured OLIVE). **U3** a real
  unlock object: `capture-rig/unlocks/unlocks-200-campaign.bin` (1,785 B) vs the default 9-byte
  key-only 4/4. **U4/U13** 13 encoded inventory items in `capture-rig/inventory/`; `D1_INVENTORY`'s
  printed default lied (serve path `main.rs:1341` has no default; the staging path does). **U5**
  `D1_FAMILY42=1`. **U6** the whole `d1-activity-host` role: 28 controls all pass, ran in 6 of 257
  runs. **U7** shadNet Matching2 (20 commands) behind `--matching2`/`--rooms` - expected-negative
  for D1.
- **U8** `d1-oracle message` builds schema-valid bodies through the client's own encoder for 97 of
  101 ids, and `d1-oracle` was not a dependency of `d1-responder`. **U9** `d1-oracle roster` (1,266
  lines, 18 controls) builds 2/0 and 2/1 and derives 0/0 and 0/1 instance checksums (`0x1D7BB8F6`,
  `0x67D97FC4`) that exist nowhere else; the two filename conventions disagreed (hex fixture stems
  vs a decimal `parse::<u64>()` that silently substituted a wrong key). **U10**
  `HeaderExtras::class6_activity_host(ah_id)` had zero production callers - every message 100 went
  out with 26 zero header bytes. **U11** shadNet notification 17 `WebApiPushEvent` - the only
  server->client channel on the HTTP surface; the client registers 7 push dataTypes every run and
  has never received one. **U12** BAP message 27 `bap_to_client_log_notification` - a confirmed
  consumer (`0x564820`, `strong`, level 3) that would print our own text into the guest log; absent
  from `OURS_TO_PUSH = [100, 123]`. **U14** 1,129 engine-config keys mapped
  (`capture-rig/schemas/engine-config-keys.tsv`) with no consumer - the capability was
  unrepresentable (`client_config_response_body` had no field-3 parameter). **U15** the director
  graph (9 books, 24 destinations, 11 places, 653 activities, 52 record books, ~40,000 rows), zero
  consumers. **U16** `TYPE_QUEUEZ_SUBSCRIPTION_LOST = 0x007c`, `TYPE_SERVER_CHALLENGE/_ANSWER =
  0x001c/0x001d` defined, referenced nowhere; 124 would be refused (on `PUSH_ACCEPT_SET`, absent
  from `OURS_TO_PUSH`). **U17** AH lease enforcement, `blocked_pushes`, `AhBind::Handoff`,
  `advance_replication_epoch` - test-only callers; `drive()` had no timer. **U18** ✅ the AH schema
  codec, WIRED at audit time: `d1-activity-host` gained `d1-oracle`, controls inverted, 28/28 PASS,
  including "the DEFAULT codec decodes the 238 bytes the LIVE CLIENT sent, to the nonce it logged"
  and "join_request `0x80800E98`, join_result `0x80800A58` - two instruments, one answer."

### 5.3 BAP transport ids - the load-bearing rows

Served: 6/7 (AH manager), 10/11 (`client_to_world_server` - the investment channel; its 30 sub-ids
below), 12/13 (subscription), 16/17 (AH proxy, `D1_AH_ENDIAN=be` - adjudicated, see §9), 18/19
(client_config; field 3 = the whole 1,129-key engine config, omitted), 21/22 (purchased offers -
header-only and that is CORRECT: `0x59F540` never dereferences the payload; not a grant channel),
23/24 (account-id translation), 25/26 (secure hello), 30/31 (channel startup), 32/33 (user message),
42/43 (matchmaking core - the surface rung 28 stalls on), 121/122 (queuez register), 123 (QueueZ
push - every server-supplied object), 171 (xetrov - served by being deliberately ignored), 250/251
(echo), 302/303 (relay register). Notable non-served: **9** (`bap_to_client_notification` - the
client SENDS it; see L5), **27** (log notification, U12), 28/29 (`server_challenge` - the
transport's second auth factor, the one place a retail capture changes the answer), **100**
(class-6 header, 32 B - the only message that can answer the msg-8 join request), **102** (NOT
pushable on the primary - absent from the 13-entry push accept-set `{6, 8, 9, 20, 27, 28, 47, 100,
123, 124, 160, 161, 301}` at `0x1925A90`; pushing it kills the client), 124 (subscription lost -
the only non-silent way to tell the client a family died), 36-41 (friends/party - the client's own
name mapper returns `UNKNOWN` for all six => `[I]` vestigial in this build), 110/112/150/151/153/
160/161/162/170 (Bungie-internal, impossible; 153 `socket_handoff` means retail may not use our
"msg 17 -> new TCP" model), 14/15 (unsubscribe), 34/35 (skill), 44/45 (clan - where investment
1601-1614 actually travel), 46/47 and 300/301 (NAT punch / relay - two-console work).

### 5.4 Investment sub-ids (30 sendable of 101; the client can only ask for 30)

`0x551FB0` is the single send entry point; exactly 30 call sites in 30 distinct functions, each a
thin wrapper whose only `mov edi, imm32` is the id - a bijection. `[C]` The 30: 104, 105, 205, 206,
305, 402-406, 501-505, 601, 701, 702, 801, 802, 804, 901, 903-905, 1201, 1501, 1801-1803.

Key rows: **104** served (byte-exact vs oracle, 70 occurrences) · **105** the PROLOGUE (solved
2026-08-09: shares 501/502/505/506's result composite `1800000000`; a wiring change) · **205**
stubbed-wearing-a-valid-shape (L18) · **206** served by configuration (`D1_206_BODY` pinned; code
default `10` measured insufficient; 167 occurrences; the B16 sim-queue entry point) · **305**
CHARACTER SELECT, span-carrying, deliberately stubbed (L12) · **402-406** inventory
(equip/unequip/transfer/flags) · **501** character creation - the only code that grants starter
gear · **502** delete · **503** `login_account` - THE ACCOUNT SOID, 236 occurrences, served by
configuration (`D1_503_SOID`+`D1_503_RAW`) · **504** `login_character`, served byte-exact ·
**505** signout - the only stub with an observed kill (32-byte header-only -> decode failure -> TCP
close -> 5x `lost_bap_connection`) · **506** built and unreachable (no send wrapper; belongs to the
tail) · **601** `loot_pickup_item` (its zero body is a 21-byte real object) · **701/702**
profile set account/character, served (657 and 762 occurrences) · **801/802/804** talent
activation · **901/903/904/905** the economy loop · **1201** `cheat_advance_time` - a developer
cheat shipped in retail (wrapper `0x550DA0`, tail-jmp'd from
`c_mock_activity_vendor_list_component::vfunc[14]`); answering it moves the game clock ·
**1501** `player_report`, span-carrying · **1801-1803** record books.

**The other 71 ids (201, 202, 203, 204, 207-209, 211, 303, 304, 506, 803, 902, 1101-1103,
1202-1240, 1301-1307, 1401, 1601-1614, 1701, 1702, 1901) have no send wrapper in this build - a
SERVER-TO-SERVER SURFACE, NOT OURS.** The client reaches the clan block through envelope `0x002C`,
1401 through `0x0015`, 201/202 through `0x006E`. Building them is not on the path to an activity.
Tail-merge control: 504 and 1201 show `n_callers=0` in the atlas but are entered by tail `jmp`
(`0x9B82A7`, `0x9B896D`) - neither is dead. `[C]`

### 5.5 QueueZ slots - 19 slots, and 8 families (not 6)

**There are 8 families (0-7).** The mapper `0x56C550` has 8 jump-table cases; families 6 and 7 have
live schema tags, RTTI-named consumers (`cui::c_fireteam_details_component::vfunc[14]` reads 6; the
`join_fireteam` path reads 7), and family 7 has a wired subscribe site (`0x5828F0 ->
0x56AB40(mgr, 7, root, mode 1)`). `[C]` Completeness proof: sorting the 19 regions by
`storage_offset` and walking `off + capacity*stride` tiles the arena with zero gaps/overlaps, ending
exactly at the slot-index table at `+0x4F800`.

| slot | what | status (audit) | notes |
|---|---|---|---|
| 3/0 | character roster (`0x80801951`) | served | gates `character:signin` |
| 3/1 | select-card, 2688 B x 10 (`0x80801954`) | served | `D1_CHARACTER_FILE` empty by default => identity-only card |
| 4/0 | ACCOUNT, 29,496 B (`0x80801803`) | served | everything downstream |
| 4/1 | PROFILE, 28,792 B (`0x80801743`) | served | inventory enumerates from 340 slots INSIDE it (S7) |
| 4/3 | inventory item, 184 B x 666 (`0x80801732`) | served, default-off | no DELETE announcer - removal is silent |
| 4/4 | unlock flags, 3,984 B (`0x8080102E`, checksum `0xC4060975`) | stubbed (9-byte key-only = no unlocks) | director selectability, rung-33 gates |
| 4/2 | 72 B single instance (`0x80800FB2`) | stubbed | the only populated slot with NO announcer of any kind |
| 2/0 | roster root - display name + clan tag (`0x808018B7`, 231 slots) | built_but_unwired x2 | the display name everywhere; L1: no CREATE announcer |
| 2/1 | 48-byte player card (`0x80801810`) | built_but_unwired x2 | emblem + card; L1. Same checksum is also the progression stat block - one 48-byte struct used as both (ledger §6.11) |
| 0/0 | peer-family root (`0x80801921`, capacity 20 = the session ceiling) | stubbed (answered, never filled) | descriptor tree field-for-field identical to 2/0 |
| 0/1 | peer investment body, 2,632 B (`0x808017FD`) | stubbed | **NEGATIVE VALUE until family 2 lands**: when 0/1 resolves, `0xA3D4E0` runs the rich fill `0xA2D9F0` INSTEAD OF the compact one - an empty 0/1 blanks a nameplate family 2 alone would have filled |
| 1/3 | same item schema as 4/3 | never_touched | family 1 is a second, richer character representation (`0x54EB40` renders a card from family 3 at `child+0x68` or family 1 at `child+0xB60`) |
| 1/0, 1/1 | family-1 root (2,048 B) + body (5,496 B) | never_touched | zero code presence |
| 6/0, 6/1 | fireteam root (16 B) + body (1,000 B) | never_touched | - |
| 7/0, 7/1 | party root (16 B) + body (152 B) | never_touched | consumer `0x954240` under `join_fireteam` |
| 5/5 | world-server/service root, 944 B (`0x80801941`) | served OUT OF BAND | not over QueueZ: it is the nested struct in BAP 205/503; the client writes it into the registry itself via `0x56BF80`, which also self-promotes the family 2->3. Family 5 shows `version = -1` => never had a QueueZ update applied. Sending 5/5 over QueueZ is not a gap; it is not the delivery path |

A family-6 or family-7 subscription would be answered `object_count = 0`, quietly (the dispatch
falls through to `queuez_empty_family_body_for`).

### 5.6 The Activity-Host udt surface (49; enumeration `[C]` three ways)

49 name-table entries (`0x18F8780`, NULL at 49) = 17 receive-dispatch entries (`0x18F84A0`, bounded
by `if (0x10 < uVar5)`) + 28 distinct send ids across 31 call sites of `FUN_00299310` - 1 shared
(18 incident) + 5 wired to nothing (9, 16, 21, 34, 40). Key rows: **udt 10**
`start_activity_host_response` - the only one served at audit; never travels this wire, injected
from BAP message 7 by `0x594C60`; proof `observed-live` that `host_token == ah_id` and `host_text ==
session_id`. **udt 4 `join_result`** (tag `0x80800A58`, 398 bit-packed wire bytes - a 424-byte raw
struct push earned 21 s of nothing then ELDERBERRY). **udt 1 `global_activity_state`** (tag
`0x80800E76`, exactly 1,712 bytes - the descriptor's struct_size, not 8 more; Auth State; absent =
an empty world, not a refused one). **udt 0 `entity_slots_allocated`** (tag `0x80800D64`, `0x600`
mask; may be skippable per S15). **udt 3 `join_request`** - received 238 B live, decodes and
re-encodes byte-for-byte; carries all four candidate identities. Inbound decoders built for 15, 19,
20, 22, 35 (`connectivity_failure` - the client telling us why it is unhappy), 36. 42/43 have no
`CALL_SITES` row => `SchemaCodecMissing`. 10 AH->client ids never touched (2, 5, 7, 11, 23, 24, 27,
29, 38, 39): **11 `replicate_membership`** keeps `MEM-N` at 0 forever => no peers; **38
`script_state`** is the deep ceiling (cloud Lua); 5 and 7 use a different codec (`FUN_002D2090`) and
are off the join path - **udt 7 `sensor_message` is the wire field that carries the lifetime phase
byte (see `design/activity-lifetime-state.md`)**. 5 impossible (9, 16, 21, 34, 40) - never send udt
16. 21 client->AH ingest-only ids (6, 8, 12-14, 17, 18, 25, 26, 28, 30-33, 37, 41, 44-48); 47
`high_water` is a free liveness beacon once the client is established.

### 5.7 Content and the simulation queue as surfaces

Content: the client asks for exactly three things on the wire (`GET /manifest.bin`, BAP
`0x0012->0x0013`, `0x0015->0x0016`); everything else is content it owns on disk, referenced by
ordinal. **The manifest resolves nothing by name** - names live in symbol tables inside the `.pkg`
files, ties broken by newest build timestamp (the patch-override mechanism). Manifest f3 packages
measurably complete (0 in-manifest-never-opened, 0 opened-not-in-manifest beyond the 3 engine
`unp3`); f2 entries (DLC catalogue, max 64; `Entry.f5` "no entitlement needed" is the only way to
declare a package registrable without an ownership check - the L11 mitigation); f1/f4 never
touched. Signon f10 never emitted (L11); f8 is the interstitial selector (bit 5 `0x20` =
`bootflow:upsell`, the only known route to the Rise-of-Iron upsell tile - ledger §6.3's correction
of the "Demonware lobby bit" claim); f6 `content_build_version` default
`"52553.16.09.28.1551.evil"`. `sceAppContent*` add-on keys are impossible for a server and blocked
by a shadPS4 all-zero key. `sku_config.txt` is NOT a wall on this surface (its reader appears in
neither owned-key store's writer set); there is no cached manifest anywhere in the 47 GB estate.

Simulation queue (95 ids; 12 server-causable; S11's "no evidence either way" refuted by control
flow): **17** `..._bap_message_queue_response_event` - every BAP response we emit becomes this
element; **18** `..._bap_push_event` - every push (the registry write happens in the apply,
`0xBAB620`, not the push handler; ceiling 131,072 B); **16** - every client->server BAP request is
issued from the deterministic tick, not the network thread; 26/23/28 = AH join_result /
global_state / entity slots (udt-mediated); 20-22, 24, 25, 27 AH-mediated with no tag derived
(at audit); **29/30/31** `activity_manager_instance_start/_create_client/_dispose` - **rung 28->30
is a sim-queue round trip, not a message exchange** (`FUN_0075C200` enqueues element 30); 40/41
profile-set elements gated on a SOID equality check with no diagnostic on mismatch, and the
producer memcmps registry-vs-simulation and does not enqueue at all if equal; 44/45/46
select-character refresh and start/stop family replication - answering the subscription is not the
same event as 45; 50/77 world-tag-loader cache state; 79 `slice_set_transition` (B20b); 84
`facet_notify_prepare_failure` - a failure notification that itself travels the queue (self-
defeating error path); **86/87 `cheat_drop_object`/`cheat_hop_on_object` - dev cheats that spawn an
object through the production bus: a ready-made positive control for "is the queue draining",
needing no server**; ~30 never-touched entity/authority/UI ids (2/4 are the replication layer);
0, 3, 33, 51, 80 sentinels/retired; 15 unnamed interceptor-handled ids `[U]`; ~20 client-local
presentation ids (cinematics, character-creation UI, NUX, voice, resurrection, video).

### 5.8 What we do not know how to serve, by the evidence that would close it

- **Retail-console capture only:** a real manifest's entries/offers (fetched into a heap buffer,
  never written to disk); the CDN container URL family; the 16-byte add-on entitlement keys; BAP
  28/29 `server_challenge`; whether a signon reauth needs a different body. A passive tap gives
  ciphertext plus metadata - plan a capture around **endpoint topology** only (where the AH lives:
  a second long-lived connection appearing at activity launch).
- **Content extraction (Oodle sweeps):** 71 of 74 investment-globals tables; the `character_level`
  progression ordinal (block 1066 @145616, read index 117's hash); 54 of 101 investment message
  names (FNV-1 over `verb_noun[_noun]`; the vocabulary is content-side, not in the binary).
- **Bounded disassembly:** the remaining AH->client schema tags (one descriptor global per receive
  handler); udt 11 (`0x8080053F`) and udt 38 (`0x808042B3`) prove `d1-eboot descriptors`' 4,555 is
  a floor, not a total; investment 201/202/1212/1306 are oracle tooling gaps; the payload schemas
  of all 95 sim-queue elements (descriptor-driven through `FUN_002C8FF0`); the 15 unnamed
  interceptor ids (`0xBAA000-0xBAF000`).
- **A live experiment:** does baseline + delta light the screen (L1); does a second baseline purge
  or get rejected (ledger §6.5, unadjudicated - the census says `0x56B2D0` purges and re-baselines,
  the responder comment reports rejection); is a 500 survivable (L16); does a decode failure retire
  the in-flight entry; the recipient sweep; signon f8 = `0x20` -> upsell; do families 1/6/7 ever
  get subscribed (the log is blind by construction - read the registry).
- **The honest ceiling no evidence moves:** the 1,712-byte Activity State blob's contents (a stub
  of zeros is a valid activity state in which nothing happens - the mission script is cloud Lua);
  join-denial reason values (no reason-string table exists in this build - the client renders
  `ENUM(%d)`); the QueueZ family/type enum names (compiled out - `ENUM(4)` is all the client will
  ever say; a finding, not a search gap).

---

## §6 - the as-built server roles (rungs 28-36)

Provenance: historicaldocs/design/SERVER-ROLES-PLAN.md - the implementation spec of record for
rungs 28-36, written 2026-08-08 from seven parallel recon reports (`d1-eboot controls` 22/22 PASS,
eboot sha256 `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`). **Presented here
as the as-built spec: every rung 28-36 has since been reached** (run `20260809-230309-FAM02-OLIVE`).
Reach: `static-client-inference` unless marked; three items corroborated `observed-live` (the 9-byte
subscription body, the head-of-line fatal, the two header-class controls). Ownership rule: ONE
PROTOCOL, ONE IMPLEMENTER - all BAP framing, the correlation table and the codecs live in `d1-bap`;
`d1-responder` may not lay down a header byte; `d1-activity-host` sits above the wire and contains
no framing.

### 6.1 Six corrections that shaped the build

1. **The `category` column is the HEADER CLASS, not request/response/notification.** Message-class
   vtable slot 1 (`+0x08`) is the header class id; the class-size table at `0x19234A0` is
   `[6, 6, 8, 14, 6, 24, 32, 8]`, flags `[0x00, 0x00, 0x18, 0x04, 0x00, 0x43, 0x4F, 0x18]` (bit
   `0x10` = carries a `u16be` status, classes 2 and 7 only). Every message Phases 0-3 needed (6/7,
   10/11, 12/13, 16/17, 42/43) uses classes 1 and 2 - the two shapes `d1-bap` already emitted.
   Message 100 is class 6 = 32-byte header; 8 is class 3 = 14; 102 is class 5 = 24.
2. **Message 102 is NOT pushable** on the primary connection (absent from the 13-entry push
   accept-set at `0x1925A90`); pushing it kills the client with the Weasel. Whether it is legal on
   `OUT FAH`/`OUT GAH*` is `[U]`.
3. **The client is the authority for session parameters; the server can never set one.**
   `FUN_00B8D630` (the "may I write" predicate) returns true iff the owning session's state at
   `S+0x8C48` is one of the four `host-*` states {6,7,8,9} (11-entry state name table `0x19548D0`)
   - "am I the host peer." Parameters replicate peer-to-peer over the session channel, msg `0x25`
   host->peer / `0x26` peer->host, dispatched at `0xB3AA60`. They do not travel over BAP at all.
   Any plan of the form "the server delivers `s_activity_host_connection_information`" is
   impossible.
4. **The Weasel is head-of-line blocking, not re-subscription churn.** The asserted churn was
   searched for with a positive control and not found; rung 28 is simply the first point where the
   client has several requests outstanding at once. (A churn mechanism static analysis cannot rule
   out - `c_investment_bap_message_queue::stop()` at `0x568770` - got a distinguishing probe.)
5. **`pair_id` in `bap-message-table.tsv` is a memory-layout artefact** (vtables are 8 slots;
   `+0x40` is typeinfo, `+0x70` is the next class's slot 0). The load-bearing source is the POLICY
   table: policy vtable slot 2 (`+0x10`) = request id, slot 6 (`+0x30`) = response id, `resp == 0`
   = no response expected at all.
6. **`"As HOST, peers done"` never fired**, and the rung-28 gate `0x7A6A90` does not read the
   `activity_host` session parameter - it reads two `u64`s out of the local peer's replicated
   property blob of each session and reports EXPECTED iff both are zero. `POSSE [WAITING]` almost
   certainly meant `FUN_00773F30(1)` returned NULL - no usable posse session, created locally by
   `FUN_00772F20` from rung-28 step 3 (`0x7A6730`). The rung-28 blocker could be entirely local -
   which is why the plan put an observe-only phase after Phase 0 and forbade building past it.

### 6.2 The correlation table - `d1-bap::BapConversation` (the Weasel fix)

What the client actually does (`[C]`, `0xB2E940`, verified in raw disassembly twice): there is no
recipient table. Each BAP connection owns two strict-FIFO in-flight rings (`conn+0x1B80`,
`conn+0x1D18`, stride `0x198`, capacity <= 32) and the receive path examines only the head of each,
matching `(expected-reply-type, sequence)`; otherwise the type-keyed push whitelist; otherwise
`update_receive() failed to find recipient` -> `raise_fatal(conn, 0x12)` with no per-type
exemption, no grace count -> connection state 9 `_dead`. The sequence is a `u32` from a single
per-connection counter at `conn+0x16A0`, big-endian at inner-header bytes `[2..6]`; type is
compared before sequence; status is irrelevant to the lookup; an in-flight entry is created on send
completion only if `expected_response_type != 0`; the only benign removal is a successful head
match; a head older than `tunables+0x4c` ms is fatal (`_connection_failure_timed_out`, `0x0B`).
**A wrong-but-routable reply is survivable. A right-but-unroutable reply is not.**

The six rules that make every miss impossible:

- **R1 ORDER** - emit responses in exactly arrival order (a single FIFO is provably safe: both
  rings are fed by one shared counter, so arrival order is a linear extension of both per-ring
  orders).
- **R2 TOTALITY** - answer every request that expects an answer; silence head-of-line blocks and
  then times out fatally. A wrong body is survivable; an absent reply is not.
- **R3 EXACTLY ONCE** - a duplicate reply is unroutable and fatal.
- **R4 SEQUENCE ECHO** - the reply's `u32be` sequence equals the request's, byte for byte.
- **R5 NEVER UNSOLICITED except the whitelist** `{6, 8, 9, 20, 27, 28, 47, 100, 123, 124, 160,
  161, 301}`; of it, only 123 and (later) 100/9 are ours to send.
- **R6 NEVER INVENT A RESPONSE CLASS** - if policy says no response, any reply is fatal (the
  measured `0x0020` case: answering with `0x0021` hung the client at 650 B where silence reached
  2477 B).

Handlers park BODIES against sequences; `drain()` writes headers and emits from the front only,
synthesising a non-200 for an aged head. Generated tables (Phase-0 deliverables, controls exit 2):
`RESPONSE_FOR` from the ~33 policy vtables (reproduces `0x0C->0x0D`, `0x0A->0x0B`, `0x0E->0x0F`,
`0x6E->0x70`, `6->7`, `0x10->0x11`, asserts `0x0020 -> None`, `0x00AB -> None`); `HEADER_LEN` /
`header_class` from `0x19234A0` (controls: `header_class(123)==4`, `HEADER_LEN[4]==6`;
`header_class(0x00FB)==2`, `HEADER_LEN[2]==8`; `header_class(100)==6`, `HEADER_LEN[6]==32`).
`reply_header` became private to the conversation module. `D1_INV_SILENT` is a violation of R2 by
construction - kept for experiments, but it must emit a non-200 instead of nothing.

### 6.3 The message structs (normative contract = the byte-offset tables)

- **Framing** `[C]`: envelope always plaintext, 6 B (`u8 version=1, u8 mode (0|2 plain, 1 sealed),
  u32be payload_size`; decoder `0xB2D9F0`). Header classes 0/1/4: `u16be type, u32be seq` (6 B,
  `0xB2EE60`). Classes 2/7 add `u16be status` (8 B; getter `0x55D7C0`, setter `0x55D7B0`;
  STATUS_OK = 200, `0xB31EF9`).
- **16 `GetActivityHostProxyRequest`** `[C]`: class 1, body exactly 8 B - `u64be activity_host_id`
  (0 is legal = "the proxy itself"). Decoder `0x562B90`. Sent under POLICY index 22 (`0x594D40`
  passes a policy index, not a message id; resolve via `0x1923CB0[22] -> vtable 0x19243A0 ->
  vfunc[2] -> 16`).
- **17 `GetActivityHostProxyResponse`** `[C]`: class 2, body exactly 16 B - `u64be echoed id` (the
  recipient `0x594E70` drops silently on mismatch), `u32be proxy_ipv4` (wire bytes `0x08..0x0B` =
  a.b.c.d), `u32be proxy_port` (`[I]` on width; the client consumes only the low 16 bits - emit a
  BE u32 and both readings hold; this `[U]` is closed by construction). Success observable:
  `AH IP resolution successful: '%s -> %s'.`
- **6 `ActivityHostManagerRequest`** `[C]`: class 1, body always `0x100F` = 4111 B (decoder
  `0x55AC00`); byte 0 = inner type from a 6-value enum (1, 3 requests; 2 response; 4
  bap_to_activity_host_manager; 0 and 5 have no codec). Type 1: `+0x001 u16be args_len`,
  `+0x003 char[0x200]` args - **the activity-host LAUNCH COMMAND LINE**
  (`/ah_type:group|fireteam[ /ah_package:X][ /ah_slice_set_names:Y]`, builder `0x5946B0`) -
  `+0x203 u16be nested_len`, `+0x205 u8[0x200]` an ENCODED BAP MESSAGE ID 10, `+0x405 u16be
  bitstream_len`, `+0x407 u8[0xC00]` bit-packed blob (`[U]`; strong `[I]`: the network-session
  parameter set), `+0x1007 u64be trailing_id` (pre-swapped by the builder - do not swap twice).
  Type 3 = type 1 minus args. There is no enum-to-string table for "received unknown type %s" -
  the %s prints the decimal of the byte.
- **7 `ActivityHostManagerResponse`** `[C]`: class 2, body exactly `0x89` = 137 B: `u8 inner_type
  = 2` (nothing else decodes), `u64be host_token` (the only byte-swapped field; passes through
  address fixup `FUN_0076C1D0`), `char[0x80] host_text` NUL-terminated. Consumer `0x594C60` copies
  `{u64 at +1, 128 B at +9}` into one 0x88 blob and delivers it as udt 10. As adjudicated by
  `observed-live` udt 10: **`host_token == ah_id`, `host_text == session_id`** (not an address -
  L8).
- **8 `client_to_activity_host_notification`**: `[C]` body / class-3 14-byte header: `u8
  inner_type (1|2)`, `u32be user_defined_type`, `i32be payload_size (0..=0x1DF00)`, `u32be aux`
  (type 1 only, `[U]`), payload. udt 8 never reaches the wire - intercepted client-side and
  rebuilt as message 6. A notification: `expects_response == None`; do not answer (enforced by R6).
- **100 `activity_host_to_client_notification`**: class 6 = 32-byte header (a wrong header length
  parks the client's resumable read forever - S16). Body: `u8 inner_type = 1`, `u64be
  recipient_id` (must equal the established AH id - L6), `u32be user_defined_type` (getter
  `0x55E480`), `i32be payload_size`, payload. On the push whitelist. As-built note: the deliver
  step re-emits on the connection index in header `+0x06`; the AH now pushes message 9 (L5).
- **42/43 matchmaking core** (NOT 34/35 - that is the skill policy): ordinary protobuf, request
  descriptor `0x1929E30` (struct `0x410`), response `0x192A110` (struct `0x2290`). Request f2 is
  the discriminator (1 search, 2 update-ad, 3 delete-ad, 4 configuration); f11 matchmaking_key
  `{activity_hash, matchmaking_content_hash, skill_bucket_hash}` - f11.3 omitted when the bucket
  equals `0x811C9DC5` (the unset sentinel, confirmed in code at `0x544F70`/`0x544ED0`). For kind 4
  the client reads exactly ONE thing: the PRESENCE of response field 4 (`0x599F80` tests
  `rsp+0x1DE0`); minimal stub `f1=4, f2=echoed nonce, f4 {f4.1 {}}` - on the wire `22 02 0A 00`.
  Repeated-field caps are hard (f3.1 cap 50, f4.2.2 cap 16). **The nonce is client-internal**
  (`FUN_00598710` compares against the stash from its own outgoing request); `current request
  nonce: '0x0'` means the interface was already zeroed by connection loss - downstream of the
  Weasel, not an independent gate. `0x5EC025C8 = fnv1("social")` is the matchmaking content hash
  at rung 28; `0xDF283AFB` is a content-database activity hash, not an FNV of any eboot string
  `[U]`.
- **10/11 and 12/13, already implemented under other names**: 10/11 = `0x000A/0x000B` =
  `client_to_world_server_request/_response` (the client's own short string is `c->ws rsp` - the
  "investment message channel" and the "world server channel" are the same pair); 12/13 =
  `0x000C/0x000D` = subscription. `0x000D` has a genuinely empty body (encode and decode slots
  both `0x1F70` = `mov al,1; ret`; *unsupported* is spelled `0x14D0` = `xor al,al; ret`).
  `0x000A/0x000B` are not a byte struct: one shared codec (`0x261BE0`/`0x261FB0`) with a direction
  flag over `16 bits index || RSAT args || 1 bit has_payload || [16 bits len || len*8]`;
  deterministic vs nondeterministic policies differ only in a client-side flag with no wire
  representation - the server cannot distinguish them and must not try. `[C]`
- **The `0x000C` naming reconciliation**: the client's QueueZ family is `0x79` register-req /
  `0x7A` register-rsp / `0x7B` object / `0x7C` subscription-lost; `0x000C` has nothing to do with
  QueueZ. Canonical constants follow the client's own class names
  (`TYPE_CLIENT_TO_BAP_SUBSCRIPTION_*`, `TYPE_CLIENT_TO_WORLD_SERVER_*`); `TYPE_QUEUEZ_SUBSCRIBE =
  0x000c` deleted outright. Doc-comments carry both decimal and hex forever (16 decimal is the AH
  proxy request; `0x0016` is upo).

### 6.4 Session-parameter delivery - the client is the authority

The real path: server answers msg 6 with 7 (Phase 2) -> `0x594C60` delivers udt 10 -> the activity
manager, AS SESSION HOST, writes `s_activity_host_connection_information.activity_host_id` (`+0x08`)
via `0xB8CDF0` (`[I]`, the one weak link; falsifier: if msg 16 never appears after Phase 2, the
inference is wrong - and `FUN_00594320` provably re-issues nothing while the id is zero) -> the
client replicates to its own peers via `0x25` -> `0x75C200` sees a nonzero AH id -> `FUN_00594320`
issues msg 16 on channel 0 -> server answers 17 (Phase 3) -> client connects to that address
(`FUN_00559170`) -> server accepts on a second BAP listener (Phase 4). **Our two levers are message
7 and message 17; everything between them is the client talking to itself.** The struct itself
(param index 3, memory `0x20`: `u64 activity_nonce` (must equal `s_activity_information`'s), `u64
activity_host_id`, `u32 registered_peer_mask` (gate: `mask >> (own_slot & 0x1F) & 1`), `+0x14/+0x18`
`[U]` - emit zeros, the only value the client is ever observed to write) has TWO independent
serializations: bit-packed 208 bits by the registry codec `0xB8D1A0`/`0xB8D270`, raw `0x20` copy on
the `0x25` peer path - memory size != wire size, and this struct is the proof. If we ever need to
BE a peer, that is a second game client, not a server (the `0x1E` membership blob is 0x108 bytes,
lookup3-style checksum, seed `0xDEADBFF6`); any phase proposal requiring it is wrong until proven
otherwise.

### 6.5 Phases, acceptance, and where they landed

Phase order: 0 the Weasel fix (hard gate - nothing past rung 28 is observable until the connection
survives) -> 0.5 OBSERVE, do not build (read the ladder; the timeout tunable at `[0x267B1B8] +
0x2684`; is there a posse session; ring capacity at `ring+0x88`) -> 1 matchmaking config (43) -> 2
AH manager (7) -> 3 AH proxy (17; `d1-activity-host` born here, not before) -> 4 the AH connection
itself (second BAP listener, header classes 3/5/6, messages 8 and 100) -> 5 rungs 31-36
(client-owned; server watches). Acceptance was defined by named client strings, never "did not
crash": Phase 0 - the three recipient/timeout fatals stop while `setup:activity_session_creation`
still appears; Phase 2 - message 16 appears at all, `waiting for session host to relay AH ID`
stops; Phase 3 - `AH IP resolution successful`, then `connecting to AH ID '%s' [nonce ...]`; Phase
4 - a connection named `OUT FAH`/`OUT GAH0` reaching state 8 `_connected` (name table `0x1953BE0`),
no new `_connection_failure_*` on the second connection. **All rungs 28-36 have since been walked**
(with the honest accounting of which pieces were still synthetic in `design/GUARDIAN-IN-WORLD.md`).

Crate layout (standing law): `d1-bap` owns all framing, codecs, `BapConversation`, the generated
tables, all crypto (AES-128-CBC+HMAC-SHA256 secure hello; AES-128-GCM tag-first sealed frames);
`d1-responder` owns transports, the read loop, TLS, env knobs, dispatch - and no header bytes;
`d1-activity-host` owns AH id allocation/registry, the endpoint registry, the udt application
protocol - and no framing; `d1-eboot` owns the generated tables' controls. Whole-workspace `cargo
build --release` and `-p <crate>` produce different binaries (feature unification) - build and test
the way you run. Wire tests pin captured bytes and named regressions, not round-trips.

The ceiling, restated from the plan (still true as-built): we are not building the Activity Host's
mission logic and we cannot - the Lua is server-side content, not in the eboot, not in the PKGs.
Reaching rung 36 with a stub AH yields a LOADED BUT EMPTY WORLD: geometry, sky, your character,
physics, movement; no enemies, no objectives, no completion. Rung 35 is P2P; rungs 31-34 are
client-owned; the server owns 28-30. What reaching 36 proves - and it has now been proven - is that
the entire boot -> orbit -> session -> matchmaking -> world-load -> in-world walk is understood and
reproducible against a private server. That is the preservation result.

---

## §7 - talks context: what Bungie said about activity architecture

Provenance: historicaldocs/design/ACTIVITY-ARCHITECTURE-FROM-THE-TALKS.md. **Its banner is honored:
the talk analysis stands; the experiment proposed from it is refuted** (the 4/4 "Activity State"
experiment: family 4/4's accessor `0x260C20` has zero meaningful xrefs, so nothing reads it as
Activity State; its `e88000000000000080` "key-only" payload was also wrong - that constant is the
account SOID; the real one is `bfffffffffffffff00`). Read this section for architecture, never for
the next experiment.

Source: the "Shared World Shooter: Destiny's Networked Mission Architecture" talk, GDC 2015 (full
OCR transcript in `historicaldocs/destiny1/docs/talks/gdc2015/sharedWorldShooter-destinysNetworkedMissionArchitecture/`,
2,717 lines, deck alongside), cross-checked against the GDC 2011 Halo: Reach networking talk.
Evidence grade `[R]` - prose, no addresses, no byte layouts; everything must be confirmed in the
binary before it is load-bearing.

The headline: activities are not client-driven and not P2P. "Activity Hosts are cloud-hosted
machines that run a stripped down simulation of just our Mission Script logic ... All Activity
Scripts run in the cloud ... The Activity Host is always the Authority over Activity State." A
blank director is therefore the expected behaviour of a client with no activity service - the
fourth server role, which §6 built.

The vocabulary, so our `[U]`s have names:

| Bungie's term | what it is | our observable |
|---|---|---|
| Activity Host | cloud machine running the Mission Script; authority over Activity State | the §6 role |
| Activity State | the object set a script declares it cares about | udt 1 `global_activity_state` (1,712 B) |
| Sensors | how a script declares the state it needs ("You can't reference an object in Activity Script without including it in Activity State") | udt 5/7 sensor messages; the earlier family-4/4 identification is REFUTED (see §9) |
| Auth State | Activity Host -> client (authoritative) | what a stub AH must send |
| Sense State | client -> Activity Host | udt 6 `sensor_sense_update` |
| Physics Host | a console simulating bipeds/bullets/animation | the client itself |
| Bubble | "an autonomous unit of simulation"; a player simulates exactly one at a time | destination / slice-set |

Structural facts worth keeping `[R]`: every activity supports co-op and seamless join-in-progress -
activity entry is a matchmaking/session operation even solo; PS4 host migrations every 160 seconds;
a bubble is the unit of simulation with one more resident in memory (where you are heading);
Activity State is deliberately tiny (the example squad is "3 AI alive, using this specific firing
area" while bipeds/skeletons/animation live outside it) - the object to author is small and
declarative, not a world dump. What the talks do NOT say: no byte layouts, addresses or message
ids; they describe the 2014-2015 architecture vs our 01.29 (Oct 2016) build; nothing about QueueZ
families 0 and 2; nothing about whether a player entity can instantiate with an empty inventory.

The archive: 4,799 files under `historicaldocs/destiny1/docs/talks/`, ~40 `derived/transcript.txt` files (the
CATALOG.md undercount saying 3 is stale - historicaldocs/design/THE-BLOCKS.md §8). Corpus-wide zero
hits for `character select`, `bootflow`, `investment service`, `profile service`, `QueueZ`,
`Resourcerer`, or any D1 service name other than `Worldserver` - **there is no published Bungie
decomposition of the tier we are rebuilding; the eboot is the source.** Method rule: grep the
transcripts before deriving a subsystem's purpose from disassembly.

---

## §8 - open `[U]`s and unresolved contradictions

### 8.1 Open `[U]`s, by what closes them

| `[U]` | closer |
|---|---|
| W1 - a real PSN `id_token` | a live client with a real PSN ticket |
| W2 - a real `/SignOn` success body | ditto (the error path is already `observed-live`) |
| W3 - a real `manifest.bin` | `cdn.deadorbit.net` resolves and is directly fetchable; untested |
| W4 - the real BAP key exchange | plaintext => a second jailbroken console |
| W5 - a real `client_config` (`0x13`) body | ditto |
| what families 0 and 2 ARE (schemas `0x808017FD`, `0x80801810`) | schema + a probe; 2/0's root decodes as display name + clan tag |
| what family 1 is (a second, richer character representation) | zero code presence anywhere; registry read |
| family 4/2's consumer | probe which of `0x5833C0/0x583410/0x583450/0x5834A0` resolves the type-2 constant |
| I1 - the in-game "change character" crash | `capture-rig/harvest-crash.py`, never pointed at it |
| the `last_orbited_destination` hash space, and its profile C offset | the FNV-1 oracle (§3.4); one ruler probe |
| which substage is character-select | a targeted external wordlist through FNV-1 against the 37-entry table at `0x273c800` (the eboot's own 34,702 tokens already tried: 2 hits, `none`/`general` - the names are not in the binary) |
| who requests state 26 | entered via cleanup's stored goal `[this+0x6C8]`; no immediate exists |
| rung 28's 7 tasks - which 7 | function pointers at `this+0x38`, readable live |
| who writes `0xB8CDF0` (the AH id into the session parameter) | the §6.4 falsifier held (msg 16 appeared), but the writer is still unnamed |
| header classes 3 (14 B) and 5 (24 B) field layouts | walk `0x19234A0` codec `vfunc[+0x08]` accessors as class 2 was done |
| does a player entity instantiate with an empty inventory | the question that could reorder everything: if not, gear is a hard spawn prerequisite |
| the eight `fill_desired_*` virtuals - do any fire in orbit (P2) | eight known addresses, one probe run |
| the `vfunc[14]` orbit character panel source (§2) | probe `c_cui_investment_character_state_component` / `c_investment_character_statistics_component` in orbit |
| `cui::c_orbit_data_component + 0x8E` (the trial flag) | one live read; two docs predict opposite values |
| sim-queue payload schemas (95) and the 15 interceptor ids | descriptor walks; handler bodies `0xBAA000-0xBAF000` |
| the `0xC00` bit-packed blob in message 6 | `0x2C8FF0 -> 0x2C8420`; strong `[I]` the session parameter set |
| enum-table hash lowercasing | indistinguishable (every recovered name already lowercase); assume yes |
| idx 4 `0xE28BECEF` and idx 7 `0xBEE20D1C` in the bootflow enum table | resisted 126,183 tokens; needs a 3-part MITM or external vocabulary |
| whether a D1 title still gets D1 routing from the live D2 signon | a retail console |
| where message 102 is legally delivered | read the push handlers on the `OUT FAH`/`OUT GAH*` connections (`0x559650`) |
| what distinguishes transport ring 2 from ring 3 | find the ring push site; if split by header class, AH replies get their own ordering domain (relaxes R1) |
| the wire-index <-> internal-index mapping for `0x000A` | disassemble `0x27FDD0` before writing any 0x000A encoder |
| `0xDF283AFB` (the rung-28 activity hash) | read the record out of the client's definition manager, or the public API manifest keyed 3743483643 |

### 8.2 Contradictions still standing (both sides carried, neither adjudicated)

1. **Inventory slot layout** (THE-CHAIN §8.2): `u16 content index @+0, u64 SOID @+8` from
   descriptor `0x808016E0` vs `key @+0x10, u16 @+8` measured in the loop at `0x1087710`. One is
   wrong; the measurement is stronger evidence, the descriptor more authoritative. Reconcile before
   writing an item.
2. **BAP port 3075 vs 3074** (THE-CHAIN §8.5d): observed connects say 3075; the f2.6/f2.7 record in
   the signon doc says 3074. f2.7 IS the port the client dials, so both cannot describe one run.
3. **GCM nonce increment** (§8.5e): +1 per operation `[C]` read out of `0x6CDE0`/`0x6CF03` vs the
   shipped "+2 preserving the direction bit" `[U]` model. We ship the `[U]` model, not the `[C]`
   one - a confidence conflict and a mechanism conflict.
4. **The signon-manager object start** (§8.5g): `0x263D0F8` vs `0x263D2E8`; one live dump of
   `0x263D0F8..0x263D600` settles it. (The ledger's §6.3 separately pinned a base of `0x263D7A0`
   by three anchors for the field-8 work - three candidate bases now circulate.)
5. **Brightness config global** (§8.5p): `0x26C2EEA` vs `0x26C2EE8` - looks like a transcription
   slip; unadjudicated.
6. **Reason-name initializer range** (§8.5q): `0x783E00-0x785400` vs `0x783A00-0x7885A0`; both
   agree on base `0x26B22A0`, stride `0x10`, 284 entries.
7. **Which state loads `investment_globals`** (§8.5r): `bap_signin` sub-state `0x12` vs
   `investment_signin` sub-machine A.
8. **Dangling forward references** (§8.5s): `character-and-orbit.md` cites §26.85/§26.86, which do
   not exist - the last two orbit gates' detail survives only in §26.84's summary.
9. **A second QueueZ baseline: purge or reject** (ledger §6.5): `0x56B2D0` purges and re-baselines
   vs the responder comment reporting rejection. Settled by `D1_RESEND_FAMILIES` + watching the
   delete-announcer `0x56E7B0`.
10. **`character_level` progression ordinal** (ledger §6.8): the content census says 117;
    `d1-oracle/src/profile.rs:58` says `[U]`. Until read (block 1066 index 117), the progression
    row scores `never_touched`.
11. **Does the AH arbitrate host election** (THE-BLOCKS §7.2): peer-local mesh with a +10000
    scoring term vs `[R]` "the Activity Host is the session authority, who arbitrates election."
    Both can be true (local arithmetic, authoritative reconcile); nobody has said so with evidence.
12. **Does a timed-out family refuse later updates** (THE-BLOCKS §7.5): asserted flatly by
    `status/TODO.md` and the `queuez-debug` skill; two docs say it is the tool's own annotation.
    Push into a state-4 family and read back.
13. **83x vs 56x re-subscribes** for classes 0 and 2: different runs, never reconciled - quote
    neither.
14. **Are families 0 and 2 replicating** (THE-BLOCKS §7.3): three run configurations gave three
    answers (all five at state 3 / 0 and 2 LOST at state 4 / answered with zero objects while 4
    sat at state 1). Not three opinions - always read `d1guest descriptors` in the same run as the
    claim.
15. **`0x0020`'s reply mechanism** (THE-CHAIN §8.5j): "no paired response class" vs "class 0x0021
    exists, policy family 25 maps it." The measurement survives (silence beat answering); the
    mechanism is inverted and unexplained.
16. **`world_launch` slot arithmetic** (§3.6): 50 x `0x18` ends at `0x4C0`, not the observed
    boundary `0x4D0` - either 51 slots or an off-by-one.

### 8.3 Numbers in circulation that should not be quoted

From historicaldocs/design/THE-CHAIN.md §7.1: 680/681 availability "passes" (a dedup-bitmap
artefact; only 3 real load-state checks happened - count distinct tags, never records) · 83x vs 56x
(above) · 28,792 bytes for the 4/4 C struct (it is 3,984 = `0xF90`; 28,792 is the profile) · "513
referenced" unlock flags (it is 446, 5.8%; the 154-row `perkHashes[]` hit was a hash collision and
the 2,852 `customDye` refs are one flag tested 2,852 times).

---

## §9 - Dead claims

Every claim below was believed, acted on, and killed - by a banner, a measurement, or an
adjudication. **None of them is live anywhere above.** Seeded from historicaldocs/design/THE-CHAIN.md
§7 (rows 1-20), then extended with every banner-kill in the five sources. Do not re-derive these.

### 9.1 From the source documents' own banners

| # | dead claim | killed by |
|---|---|---|
| B1 | **"B16's simulation queue has 115 element slots"** | THE-BLOCKS' 2026-08-10 retraction banner: the table is `0x2F8` bytes / stride 8 = **95 slots**, highest registered index 94, zero slack; **115 is the count of `c_investment_bap_message_*` RTTI classes** from an adjacent `d1-eboot controls` line. Cite 95 for the table, 94 for the highest index, 59 only as "at least". |
| B2 | **"the black screen in an activity is a shadPS4 texture-cache defect"** | THE-CHAIN's superseding banner: measured at `activity:in_world` - 577 live sobjects across 8 types, ZERO of type 9 (`biped`). There was never anything to draw; the texture-cache warnings are real and irrelevant. (The orbit-era black screen note in the source body described a different, earlier situation.) |
| B3 | **"the roster at `gg + 0xD0` being empty is the cause"** (the banner's own first framing) | itself retracted: the block is memset by design; players arrive by sim-queue element `0x0D` (`historicaldocs/design/player-creation-chain.md`). |
| B4 | **"rungs 35/36 have never been observed / never executed"** | superseded 2026-08-10: rung 36 reached three times, whole ladder in one boot, zero `error.*`. |
| B5 | **family 4/4 is the talks' "Sensor"/Activity State table** | the talks doc's own refutation: 4/4 is account/campaign/**unlock** state (`s_investment_unlock_flag*` RTTI; consumed by `investment_activity_selection_manager`; required in orbit before any AH exists). The shape match was a coincidence of numbers. |
| B6 | **the 4/4 experiment payload `e88000000000000080`** | it decodes to `0xD100000000000001` = the account SOID; the real key-only form is `bfffffffffffffff00` (asserted in `d1-bap`). |
| B7 | **"serve 4/4 keyed `0x7FFFFFFFFFFFFFFE` to make Earth selectable"** (the experiment) | run and eliminated; accessor `0x260C20` has no meaningful readers. The talk ANALYSIS stands; the experiment does not. |

### 9.2 THE-CHAIN §7's dead list, carried forward

| # | dead claim | killed by |
|---|---|---|
| 1 | family 4/4 keyed `0x7FFFFFFFFFFFFFFE` is the director blocker | delivered and poked - Earth still not selectable; structurally, `0x260C20` ~0 refs vs `0x260C10`'s 40 |
| 2 | payload `e88000000000000080` for 4/4 | = B6 above |
| 3 | unlock flags gate the director | Earth's `DestinyDirectorBookDefinition 2707330101` has an empty `isVisibleExpression`; 200 flags set from the manifest changed nothing |
| 4 | family 4 is stuck at replication state 1, so updates are ignored | all five families reach state 3 with `D1_ANSWER_FAMILIES` |
| 5 | "state 4 REFUSES updates permanently" | `(update REFUSED)` is `d1guest.py`'s own annotation, not measured behaviour (still an open experiment - §8.2 #12) |
| 6 | the prologue must be proven before orbit | all 546 records carry the `0x811C9DC5` empty sentinel => the prologue can never launch |
| 7 | character select is the critical path / hold-the-screen patches | both patches failed on the roster-non-empty branch; one NOPed the wrong function, the other removed the only legitimate exit |
| 8 | the availability builder FAILS, stamping `{1,0,0}` | it runs three times; only the early-boot invocation fails; both post-boot invocations write `AL=1`; `ctx+0x28` is never null |
| 9 | family 4/4 is the "Sensor" table | = B5 above |
| 10 | `[0x2639EA8] == 0`, so the 4/4 lookup is never reached | it is not zero; inferred from an empty `PROBES:` block instead of reading the address |
| 11 | family 2 = progressions | family 2/0 root `0x808018B7` = a 26-char display name + 4-char clan tag; progressions live at 4/1 `+0x3660` and 4/0 `+0x304C` |
| 12 | `0x2609C0` is a failing predicate | it is a cache-freshness predicate, it returns TRUE, and TRUE is the STOP answer |
| 13 | the 4/4 instance type is `[U]` | it is `0xC4060975` (`0x8080102E` is a 1-field wrapper whose checksum is `0xC4060975`; `0xAF776963` belongs to the inner schema `0x8080102D`); pinned in `d1-eboot controls` |
| 14 | the content index for an item is `[U]` | CLOSED 2026-08-09: the element index of `ps4_investment_globals_012f_3` entry 8126 (7,544 x 24 B, hash at `+0x00`), argument of `0x25ADE0`. "Tags are contiguous" is REFUTED - 633 discontinuities; `0x80A6035A` is index 197, not 195 |
| 15 | `0x5745F5` destroys the availability cache | it NULLs the pointer; the pointer is intact |
| 16 | "find what reads the cache" is the next step | necessary but not sufficient - a reader finds zeros however correct it is |
| 17 | `profile_setup` is character select and needs authoritative character state | it is a three-screen local display-settings wizard at `0x79E830` that sends no message |
| 18 | "one TCP read is one BAP frame" | long since fixed; 188 messages, 0 decode failures |
| 19 | investment 701 must be answered | 701 is not the gate; silence works and an empty response is harmful |
| 20 | `d1guest tag` proves 2020 manifest hashes resolve on the 2016 client | it computes `index = tag & 0x1FFF` - arithmetic, not a lookup |

### 9.3 Killed inside the boot chain and the census

| # | dead claim | killed by |
|---|---|---|
| C1 | **"the availability cache is built successfully and then DESTROYED before anything reads it"** (THE-CHAIN's own §3.3 headline) | the `--abs` trap: `0x37F8BD330` is a heap pointer; without `--abs`, `d1guest read` added the eboot base and read unmapped memory as zeros. With `--abs` the header reads `{1,1,0}` - built and successful. The "control" (3 stable reads over 10 s) tested stability when the worry was address correctness. |
| C2 | "the name-hash function cannot be cracked from the binary - CLOSED, do not re-attempt" (as a universal) | true only of the TYPE-NAME registries (space 2); the NAMES space is **FNV-1**, cracked, 7/7 sku names cross-validated. "Destiny hashes content with FNV-1a" is flatly false. |
| C3 | "`0x811C9DC5` is the FNV-1a-32 offset basis" (as an identification) | FNV-1 and FNV-1a share the basis; the client's hash is FNV-1. |
| C4 | rungs 29's requester "belongs to rung 29" (and every same-shaped row) | the census column is the handler of rung N-1: the ladder is self-driving. |
| C5 | rung 28's requester is one of `0x797420`/`0x797530`/`0x7A3750` | all three wrong; it is `0x7AB460` inside `0x7AAF60` (orbit's own update), hidden by a tail-merged `mov edi,0x1c`. `0x797420` is positively eliminated. |
| C6 | cleanup `+0x6A0`/`+0x6B0` and rung 28 `+0x118`/`+0x128` are a task-queue head/tail pointer pair | they are 64-bit bitmasks over a fixed task table (the client XORs them in its own log; XOR is meaningless on pointers). Rung 28 waits for 7 named tasks. |
| C7 | "there is a third `world_launch` entry path that a call/jmp/riprel/rela scan does not find" | the scan found it; it was reported as a riprel and read as noise. `0x753540` is command-slot 3, invoked indirectly at `0x34C0BC`. |
| C8 | "external poke of the world_launch command is ruled out because the command is transient" | ruled out only for an UNTIMED poke: the buffer is a fixed slot reachable from `[0x267B110]`. |
| C9 | "DO NOT EMIT signon field 4.3" (`signon-protocol.md` §8.5a) | wrong and expensive: f4.3 is the only writer of the manifest URL; the magic and keys are raw fixed32, not varints; emitted that way it is accepted live. Obeying it means never crossing `content_check`. |
| C10 | "answering `0x000C` with `0x000D` does not advance replication - refuted fix" | THE WEASEL WAS `0x000C` AND WE HAD REFUTED THE FIX: four runs, one variable; the answered run never closed, zero errors. A negative result from a run with an unanswered request outstanding is not a result. |
| C11 | "the signon URL override needs no code patching" (sku_config re-encryption) | the file is RSA-signed over the ciphertext, unconditionally; the cheap route is DNS inside the sealed namespace. |
| C12 | the brightness screen "needs the STICK" `[C]` | later, more controlled reading: all four axes swept, none advance it; the virtual pad's `axis` has never been shown to do anything on any screen. |
| C13 | "must answer `0x0017`/`0x0018` to leave `profile_setup`" | neither message appeared in 142 prior runs that reached orbit anyway - falsified as a precondition. |
| C14 | `bap-protocol.md` §8's message-name table (string-adjacency derivation) | corrected three times: `0x7A` is `queuez_register_response`; `0x1C`/`0x1D` are `server_challenge_request/_response` (a live authentication surface); `0x24`-`0x29` are friends/party messages, not gaps. |
| C15 | skip-`profile_setup` ranking (c1 easier than c2) | inverted: `[0x26B4070+0x38] = 7` skips all three screens, confirmed live and unattended. |

### 9.4 Killed in the server-surface audit and the roles plan

| # | dead claim | killed by |
|---|---|---|
| D1 | **`D1_AH_ENDIAN` "must be LE"** | adjudicated BE by a two-consumer, one-swap argument on two runs (BE: resolver prints garbage, connect succeeds `_connected[10.77.0.2:3075]` x11; LE: resolver prints pretty, connect dials `2.0.77.10:780` and fails). The original three-site derivation was correct and its retraction is itself retracted. Beware: `encode()`'s doc comment, `RUNG-30-REACHED.md` §3.2's body, and the responder's "negative control" log label all still assert the falsified reading - cite `main.rs:1777` and the two-run table, never a method name. |
| D2 | "signon field 8 switches the Demonware `/auth/` lobby tier on - we already control the switch" | field 8 is the bootflow INTERSTITIAL SELECTOR (eula/email/upsell/account-transfer); `Demonware lobby connection not desired` is the else branch that fires when no interstitial is required (257 hits / 196 logs). `/auth/` remains `[U]` with no known trigger. |
| D3 | "QueueZ has 6 families (0-5)" | the mapper `0x56C550` has 8 cases; families 6 and 7 have live tags, RTTI-named consumers, and (7) a wired subscribe site. |
| D4 | "the client never subscribes family 2 - push is the only route" (`main.rs` comment) | across all 93 ACK-logging runs the classes seen are 0 (338x), 2 (338x), 4 (97x); class 3 never appears. |
| D5 | THE-BLOCKS S1's "entitlement enumeration is empty in every run" | the pinned run records `0xE0000002` and `0xE0000001`, twice each; only add-on keys are missing (a shadPS4 all-zero-key defect). A silent-failure index that lists a working block is the same disease it documents. |
| D6 | "`d1-activity-host` was never launched" / "the AH schema codec is missing, blocked on BSS" | it ran six times against a live client, answered 6 with 7, received a real 238-byte join_request; the oracle-backed codec landed (U18), 28/28 controls. Also dead: `0x80800F58` as join_result's tag (size-matching lost; the tag is `0x80800A58`) and "global_activity_state is 1,720 bytes" (it is exactly 1,712). |
| D7 | "message 100's push-accept was falsified by the RAWCODEC run" | over-strong: the pushed payload was a 424-byte struct where the wire wants 398 bit-packed bytes; `FUN_002A4090`'s size check fails a wrong-shape decode with no diagnostic. Nothing about the accept-set follows from a run with an invalid payload. |
| D8 | BAP message 9 frames are noise / unhandled junk | they are the client RELAYING our own AH pushes because header `+0x06` (a connection index) was 0 = `OUT PRIMARY`; decoding msg 9 is the only direct observable for AH push delivery (L5). |
| D9 | `recipient_id` = the client's account SOID (the responder default) | `FUN_0075AB60` compares against the established AH id (or, udt 4 only, the pending join request's AH id) - never an account SOID. |
| D10 | the brief's "category at +0x08 (1=request 2=response 3=notification)" | it is the header class id, values 0-7 against an 8-entry class table. |
| D11 | the brief's "100 / 102 - we push" | 102 is not on the push accept-set; pushing it on the primary kills the client. |
| D12 | `pair_id` as the request/response pairing source | a memory-layout artefact (typeinfo at `+0x40`); the policy table is the source. |
| D13 | the rung-28 "re-subscription churn orphans in-flight replies" mechanism | searched with a positive control, not found; head-of-line blocking is what the receive path implements (three `observed-live` corroborations). |
| D14 | "As HOST, peers done" fired / the client is posse host | 0 hits in either run (positive controls passed); `0x7A6D60` returns at the task-bitmask check first. |
| D15 | the `{addr, addr2, port}` hypothesis for `s_activity_host_connection_information +0x14/+0x18` | one writer, one call site passing `(0, 0)`, no reader. Emit zeros. |
| D16 | "message 6's unknown-type string implies a name table" | `0x16008CC` is the literal "%d"; the %s prints the decimal of the type byte. |
| D17 | `TYPE_QUEUEZ_SUBSCRIBE = 0x000c` (the name) | `0x000C` is `client_to_bap_subscription_request`; the QueueZ family is `0x79`-`0x7C`. Deleted outright. |
| D18 | `session-params.py`'s `desired_join_controls` size 6 at `+0xE4` and `live_quality` `0x20` | size 4 at `+0xE1`; `0x284 / 0x24`; six parameters get real names from the client's registry at `0xB913B0`. |
| D19 | "`0x79EC20` is profile_setup's handler" | it is the QUEUING handler; `profile_setup` is `0x79E830`. Everything downstream of the misattribution (`0x78CD00` etc. as "the profile_setup config path") is wrong. |
| D20 | `eboot-map.md` §4.3's `0xAF776963` as the family-4/4 instance checksum | `0xC4060975` (the wrapper's checksum); `0xAF776963` is the inner schema `0x8080102D`. |
| D21 | "config key 1254 is a hole" | its jump-table entry `0x107A4E2` is a live arm. |
| D22 | the `0x001A` sealed frame is 98 B | 114 B (payload exactly 100, `sealed_len` 96, ct 48), from three literal compares in `0x561AE0`. |
| D23 | the `investment:peer:` refusal family "has never printed and is ahead (PvP)" | it printed 59x at rung 30 (OLIVE) and is cured by `D1_ANSWER_FAMILIES=0,2`; only the specific SOID-validity string remains unseen. |
| D24 | "no boot-flow / no machine-readable talks coverage - do not commission transcription" (CATALOG.md, as a whole) | ~40 transcripts now exist locally, including the UI/Director talk, the asset-pipeline talk, and the spoken Q&A ("Activity Hosts are not fault-tolerant"); the "no wire protocol in the talks" half still holds. |

### 9.5 Silent re-assertions to watch for

The corollary the sources kept proving: a retraction that lives in one document while others still
assert the dead claim is itself a silent failure. Known standing re-assertions at consolidation
time: `status/TODO.md` and the `queuez-debug` skill still state dead claim #5 flatly;
`historicaldocs/design/investment-content-map.md` still carries #2 and #10; `historicaldocs/design/`'s prologue
docs carry #6 with no banners; `d1-bap/src/activity_host.rs`'s `encode()` doc comment, the
responder's endian log label, and `historicaldocs/design/RUNG-30-REACHED.md` §3.2's body all invert
D1. When touching those files, fix the assertion in the same turn.

---

## §10 - Sources

This document supersedes the five sources below for architectural reference. They move to
`historicaldocs/design/` unedited; cite them as `historicaldocs/design/<name>.md §N` for history and
full detail. Live sibling docs for adjacent topics: `design/bap-transport.md` ·
`design/queuez-replication.md` · `design/activity-launch.md` · `design/activity-host.md` ·
`design/peer-and-platform.md` · `design/eboot-map.md` (stays live - the address book) ·
`design/GUARDIAN-IN-WORLD.md` (the current spine) · `design/activity-lifetime-state.md`.

| source (now historical) | was | what this document absorbed |
|---|---|---|
| `historicaldocs/design/THE-BLOCKS.md` | the architectural model, 2026-08-08 (+ 2026-08-10 banner) | §1 blocks/tiers, §2 state machines, §4.1 S1-S21, the ceiling, the 95-slot correction |
| `historicaldocs/design/THE-CHAIN.md` | the order of operations, 2026-08-08 (+ two 2026-08-10 banners) | §3 ladder/census/FNV-1/appendices, §8 contradictions, §9.2/§9.3 dead lists |
| `historicaldocs/design/SERVER-SURFACE-LEDGER.md` | the owes-vs-delivers audit, 2026-08-08/09 | §5 surfaces, §4.2 L1-L19, §9.4 adjudications |
| `historicaldocs/design/SERVER-ROLES-PLAN.md` | the implementation spec for rungs 28-36, 2026-08-08 | §6 as-built roles, the correlation table, the message structs |
| `historicaldocs/design/ACTIVITY-ARCHITECTURE-FROM-THE-TALKS.md` | the talks analysis (banner: experiment refuted) | §7 talks context, §9.1 B5-B7 |

Rules that outrank anything here: `CLAUDE.md` (taint propagation, one protocol one implementer, no
format twice, poke-to-identify-never-to-satisfy) · `status/LESSONS.md` · `historicaldocs/design/ATLAS.md` §4-§5
(tail-merge and segment-offset traps) · `design/diagnostic-catalogue.md` §2 (before citing any
NOT-SEEN).
