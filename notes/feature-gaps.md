# What the stand-ins still lack (gap analysis, 2026-09-27)

Where the project stands: sign-in, character creation, orbit, the first mission's load and the
guardian's spawn at the highway start all work against loopback stand-ins (README). This note
lists what Bungie's retired services still did that the stand-ins do not do yet. It is ordered by
what a player sees first. Each entry gives the evidence, the next step, and any data needed from
the disc or a PC run. The ordered commands are in [`next-session-playbook.md`](next-session-playbook.md).

## Reference: how the first mission should play

From a launch-day PS4 recording of the same content (GhostRobo, "Destiny Gameplay Walkthrough -
Part 1", YouTube `XKQfiwPzjmc`; times are video times, storyboard frames and auto-captions read
offline). It is useful as the target sequence for the stand-in's mission logic:

| Video | What happens | Our objects / state |
| --- | --- | --- |
| 0:44-2:57 | Character creation (class, race, gender, face, colours, markings) | services 501/504; appearance struct `0x8080181a` |
| 3:00-5:49 | Opening cinematic ("We called it the Traveler...") | client-side, before the activity |
| 6:36-7:25 | Letterboxed cinematic: the Ghost searches the car graveyard and finds the body | slice set 113 `cosmo_m0_phase00_highway_intro`, cinematic `0x992bb471/0x6/1` (`pf_104_res`) |
| 7:31-8:09 | First person, Ghost hovering: "Guardian. Eyes up, Guardian... it worked, you're alive... This is Fallen territory, we aren't safe here. I have to get you to the City." | end of `pf_104_res`; the switch from 113 to 112 |
| 8:15 | Control, HUD radar top right | spawn in 112 (the stand-in reaches this, without the cinematic) |
| 8:24-8:35 | "We won't survive long out in the open like this. Let's get inside the walls." Objective **Enter the Wall** top left, waypoint on the Wall | `0x992bb471/0x20/3` state + sequence `/0x5/4` |
| 8:54-9:24 | Inside: objective **Find a Weapon**, area title **THE BREACH** | trigger `/0x1d/30`; bubble 3 (The Breach) |
| 9:44-10:14 | Dark room, the Ghost's light, Fallen ("they're everywhere"); the rifle; ammo counter appears | **Grab the Rifle**; loot pickup; first equipped weapon |
| 10:19-12:45 | "Hope you know how to use that thing"; Dregs and Vandals; a chest with a sniper rifle; XP bar grows ("almost level two") | **Fight through the Wall**; squads (kind 1); loot; XP |
| 14:31-15:14 | "This was an old cosmodrome"; the Fallen ship; **Fight through the Fallen** | later objectives; bubbles 2/6/7 |

The mission's objective chain from the disc (orbit checkpoint, "Objective text") matches this
order: 3 Enter the Wall, 7 Find a Weapon, 11 Grab the Rifle, 14 Fight through the Wall,
17 Fight through the Fallen, 20/23 Find the Ship, 26 Clear out the Fallen.

## 1. The see-through ("hologram") guardian: no equipped gear

**01.24 update (2026-10-05):** the analysis below uses historical 01.00
addresses. Final starter item indexes alone left the world model transparent:
gear-slot +0xc also needs a model arrangement index. The public final starter
fixture now supplies the verified selections, and JOIN119 shows solid daylight
Titan armour. Character-menu Vulkan device loss and misplaced inventory items
remain open. Use the [current appearance evidence](guardian-appearance-0124.md).

**Symptom:** the guardian spawns but renders translucent, like a hologram. Destiny draws that
placeholder while a guardian's gear (armour and weapon art) has not loaded.

**Cause (static analysis):** the stand-in never says what the character wears.
- The guardian's look is built by `0x670e40` -> `0x6706b0` from the character data at player
  record +0x8b0 (a copy of the peer-family character summary, via the character cache
  `0x2998e0` / `0x673180`). `0x6706b0` walks **20 equipment slots**: the character progression
  struct `0x80801855`, at +0x30 of the peer summary `0x808017fd` and at +0x68 of the
  select-character summary `0x80801954`. Its +0x138 holds 20 x `0x8080183c`: {u64 item instance
  id, **i16 item index**, i8, i16, +0xe six dyes {i8 channel, i16 dye}}.
- An item index is a position in the investment globals item list. `0x41436c` treats -1 as "no
  item". `0x28a304` resolves an index through the investment-globals root tag +0xd8 (`0x288278`), to 8-byte
  entries {u32, u32 item definition tag}. The slot layout also comes from the investment
  globals: root tag +0xc8, a byte table read by `0x28a1dc`.
  - Byte +6: one item whose definition feeds appearance +4.
  - Bytes +7..+0xb: the five armour slots. Their dyes are applied by `0x670080` -> `0x66fc64`.
  - Bytes +0xc..+0xe: the three weapons, with perks in the three `0x808045c4` and stats in `0x80804703`.
  - Bytes +0xf..+0x11: three more slots (ghost / ship / sparrow?).
- The stand-in's summaries carry only {id, class, appearance}. All 20 slots are empty, so no gear
  loads. The same applies to the character-select screen (`0x80801954` +0x68) and to the self
  character (`0x80801714`: +0x30 inventory of 256 x `0x808016e0` {i16 item index, u64 instance id,
  i32, i32}; +0x1838 the 20 equipped instance ids).
- Other progression fields are still unnamed. At `0x80801855`:
  - +0: an i32, probably the level. Absent now; the "Level Up" toast may come from this.
  - +0x4: five {u8, 8 x u32}.
  - +0xb8: 32 x u32 (hashes; the builder feeds both through `0x28b750` / `0xbdbb44`, likely
    talent-grid nodes or perks).
  - +0x458: 8 x {u8, u64}.
  - +0x4d8: 26 x i16 (stats?).

**Fix in the stand-in (done):** `bap_server.py --character-gear SLOT=ITEM[/CH:DYE...],...` (or
`@file.json`) equips items on all three objects. `--character-overlay JSON` sets any other
character field by offset (`{"peer": {"0x30": {"0": 1}}}` sets the progression i32).

**Item list decoded (2026-09-27, from the uploaded investment packages).** The investment
globals root is the named tag `investment_globals` (`0x80a58000`, package 012c).
- **Items.** Its +0xd8 is the item list: 3,181 items, whose definitions are in package 012e.
  `scripts/d1_pkg.py items <packages> --oodle <dll>` lists each item: index, equipment slot,
  bucket, type and English name.
- **Buckets.** Its +0xb0 is the 14 equippable buckets (`items --buckets`): 0 Subclass,
  1 Helmet, 2 Gauntlets, 3 Chest, 4 Legs, 5 Class Armor, 6 Primary, 7 Special, 8 Heavy,
  9 Ships, 10 Vehicle, 11 Ghost, 12 Emblems, 13 Shaders. The bucket hashes are the ones
  Bungie's public Destiny 1 API used (Helmet `0xcd887e07`, Gauntlets `0xd3b5f9fc`, ...).
- **Slot order (strong hypothesis).** The equipment slots of the character data follow this
  bucket order. It fits the appearance builder: five armour slots, three weapons, then three
  more. The runtime table that maps the builder's roles to slots comes from another globals
  root (`0x1c0708`: +0xfc of the registry's root, resolved by `0x1beef0`; that root is not identified yet), so a run confirms it.
- **Starter gear.** A new guardian wakes up in "wire armor" ("Your Ghost built this wire armor
  from available mass"):
  - Hunter: items 5-14;
  - Titan: 193-202;
  - Warlock: 381-390.

  Each piece has a 118-byte and a 152-byte definition. The kit also has the class's first
  subclass (774 / 776 / 778) and the Generalist Shell ghost (892). A new guardian has no
  weapon; the first mission hands out the rifle. `bap_server.py --character-gear starter:titan`
  (or `hunter` / `warlock`) equips the kit; `starter:titan,6=2008` adds a primary weapon
  (2008 = Cydonia-AR2).
- **Build age.** Some item text is placeholder: every class item is "Makeshift Scarf", every
  subclass "Bladedancer". The list also names later content (Trials of Osiris, Queen's
  Orders). The mission's rifle (Khvostov) is not in the list under that name.

## 2. Mission logic (activity host scripts)

**Progress (2026-09-28).** The states of objectives, cinematics, sequences, triggers and squads
are decoded, and `bap_server.py --ah-mission missions/a_guardian_rises.json` scripts the
mission start. Details: [`mission-director.md`](mission-director.md); the first run is TEST371.
Two fixes came out of this:
- objectives need the player mask at +0x2c0;
- the host tick must advance (`--ah-sim-clock`).

Retail activity scripts ran only on Bungie's activity hosts (Truman talk, `public-research.md`).
None of the 79,148 decompressed Cosmodrome tags contains Lua bytecode (`\x1bLua`), so the disc
does not carry them either. **The stand-in has to reimplement each mission's script** from its
declared data: phases, objectives (kind 0x20), sequences (kind 5), cinematics (kind 6), triggers
(kind 0x1d) and squads (kind 1), with the reference sequence above as the target. In order:

1. **Intro cinematic** (slice set 113, then 112). The spawner refuses to spawn in a cinematic
   slice set, so the real host started the client in 113 and switched to 112 when the cinematic
   ended. Open: which cinematic state fields start and end it (playbook step 5).
2. **Objectives and dialogue.** Step the objective state (TEST369). The Ghost's lines are
   sequences (kind 5) that the host starts together with each objective.
3. **Triggers.** The client reports trigger volumes (`_player_trigger`, kind 0x1d) as sense state
   in type-6 upstream objects. The host reacts by advancing the objective. Open: the type-6
   per-object header, and which client message a trigger produces.
4. **Squads / enemies.** The host places a squad through its sensor (kind 1) as auth state. The
   physics host, which is the client, spawns and runs the AI and reports kills back. Open: the
   squad sensor fields ("spawned, alive count, firing area").
5. **Loot and the first weapon.** "Grab the Rifle" is a loot pickup
   (`loot_component_send_pickup_message`, `investment_loot_object_manager`). The client asks,
   the investment side grants the item (a self-family inventory delta), and the weapon is
   equipped. Without it the guardian has no gun after 10:14.
6. **XP and level.** The XP bar fills during the mission (video 12:45), so XP and progression
   updates must flow from the host to the character data.
7. **Mission completion.** The outro slice set 105 (`pf_105_cto`, Dock 13), rewards, and the
   return to orbit (service 505 is the "back to orbit" path seen before).
8. **Bubble changes.** Walking into The Breach is client-driven: `0x31dab0` follows the player's
   position (TEST368). The host must follow with world-state records (`--ah-follow-client-zone`).

## 3. Character and account services (BAP / investment)

The client knows 66 web services. The stand-in answers each with a default reply: result
error_none, fields absent. Only a few are understood:

| Service | Known use |
| --- | --- |
| 104 | universe object (server clock, flags) |
| 105 | world transition (the destination) |
| 201 | reply 0x2000 bytes: the account object |
| 202 | reply 0x55b8 bytes: a full character object (`0x80801714`-like: inventory, equipped, ...) |
| 203 | reply 0x80: one item's details? |
| 206 | family fetch |
| 501 / 503 / 504 / 505 | create character / sign in / select character / back to orbit |
| 701, 702 | result-only; 701 has a 1080-byte request |
| 1201-1227 | 27 services, all result-only replies: most likely item actions (equip, transfer, dismantle, lock, ...) whose effect arrives as self-family deltas |
| 1303 | reply with a 500-byte blob list |

Missing:
- **Request types (done 2026-09-28).** `scripts/d1_types.py` reads every type definition from
  the EBOOT and the service table from the investment globals, so each service's request and
  response layout is known without a run. See [`investment-services.md`](investment-services.md)
  for the table, the wrapper function per service, and tentative meanings.
- **Inventory, equip, vault.** The character and inventory screens act through the 12xx services.
  Each needs a handler that changes the stored character and pushes a self-family delta. The
  pattern already exists for 504's `push_self`.
- **Progression.** Level, XP, currencies (glimmer), reputation. Candidates are `0x80801755`
  (self +0x29a8: 4 x i32, 3 x u64, {bool, u64}), the 32/128-entry i32 tables at self
  +0x50a8/+0x5128/+0x5398, and the progression struct above.
- **Persistence.** The character store keeps only the 501 request (class and appearance).
  Needed: inventory, equipped, progression, and the story-progress flags. The character flag
  table C (self +0x3bc0, 1024 flags) is where `--self-flags` gates story access.
- **Vendors, bounties, postmaster, grimoire** (`ui_launch_vendor_screen` exists). These are later,
  and all service-driven.

## 4. Other activities and multiplayer

- **Tower / social space** (decoded from the disc: [`tower.md`](tower.md); every vendor and NPC is a
  host-placed squad, `missions/tower_npcs.json` places them): a public activity host. The client has "sstm_public_current/target
  activity host" join paths and their errors. It needs the same host join as the mission, with
  a social activity.
- **Patrol, strikes, public events:** public activity hosts plus Demonware matchmaking
  (`createSession`) for bubble matchmaking.
- **Crucible:** `ps3_globals_pvp_*` packages; a different game mode and a peer-to-peer physics
  host between consoles.
- **Fireteams / more than one player:** the peer family holds up to 20 peers, the simulation
  roster more than one machine, and Demonware sessions and NAT. Only one local player has been
  tried.

## 5. Emulator and tooling

- The login-cinematic freeze (about 40 % of boots) is unresolved (playbook step 6).
- The urgent-message dialog's trigger is unknown.
- Blocked in the cloud: running the game. The container has the EBOOT and packages only.

## Spawn blockers (for reference)

`0x35793c` and its callers refuse a spawn with these messages (table at `0x11eda20`): FAH / GAH
not instantiated; no slice set loaded; in a cinematic slice set; gear still loading; no
simulation player; awaiting faction assignment; no player sensor assigned; hard wipe active;
player object failed to create; invalid player blueprint handle; invalid character class
handle; no character sheet for the player; spawn override points blocked.

## Data wanted

Uploaded on 2026-09-27 and read: the investment globals and assets, the player / weapon / ship
gear packages, globals 0151/0154/0155/0156 and the client startup globals. Still wanted, from a
PC run:

- the `rpcs3_tiger_types.py schema` output with the request types (section 3);
- screenshots from TEST370: the guardian with the starter kit in character select and in the
  mission (third person, if the camera allows).
