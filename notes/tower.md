# The Tower (social space) from the disc (2026-09-28)

Read offline from the uploaded Tower packages: `ps3_city_tower_activities_act_023d`,
`ps3_city_tower_ambient_activi_0244` (+ `_0247_en`) and `ps3_city_tower_destination_024c` /
`0250` (+ `_024f_en`). The commands are `d1_pkg.py named`, `bubbles` and `groups --objects` on a
folder holding those packages. Nothing has been run in the game yet.

## Activities

| Named tag | Class | Name |
| --- | --- | --- |
| `0x80c7a001` / `0x80c7a005` | activity / scenario_client | `city_tower_default1` |
| `0x80c7a002` / `0x80c7a00b` | | `city_tower_queen` (Queen's emissary event) |
| `0x80c7a003` / `0x80c7a011` | | `city_tower_iron_banner` |
| `0x80c88001` / `0x80c88003` | | `ambient_city_tower` |
| `0x80c98005` | map root | `map:city_tower:root` (destination hash `0x8be28003`) |

The scenario_client of `city_tower_default1` has the same layout as the mission's:
- +4 is the activity group key `0x545aec79` (FNV-1 of `city_tower_default1`);
- +8 is **the checksum `0x046c1622`**;
- **5 bubbles**, with name hashes 0 `0xf28fc859`, 1 `0x26e1bec5`, 2 `military`, 3 `plaza`,
  4 `0x81eb50ae`.

Each bubble has 5-11 map containers. The world id is what the client sends in service 105 when
the Tower is chosen in the director; the stand-in logs it.

## Replicated groups

- **Game mode, `0x40aebf90`:** 0xf/0 scoreboard, 0x11/1 lifetime_timer, 0x10/2 lifetime,
  0x27/3 new_user_experience, **0xc/4..33 players[0..29]**. So `--ah-sim-player 0x40aebf90:4:OWNER`
  and `--ah-game-mode 0x40aebf90:2:3`. The mission used `0x4786c0e0` with players[0] at index 5.
- **Script, `0xf2d88bd0`:** 0x6/0 `cine_hanger_a_landing`, 0x6/1 `cine_hanger_b_landing` (the
  arrival cinematics), 0x9/2 host, kill volumes (plaza, boulevard, overwatch, underwatch,
  hangar), a firing area.
- **Event vendors:** `0x83105ee7` (queen: `sq_queen_vendor`, its guard) and `0xf687db0a`
  (`sq_iron_banner_vendor`), in their own activity variants.
- **Ambient `0x8d3f1166`, the boulevard, 84 objects:**
  - vendors: `vendor_avatar`, `vendor_nm` (New Monarchy), `vendor_black_market`,
    `vendor_playtest`;
  - civilians: kneelers, standers, sitters, leaners, walkers; `the_mask`;
  - droid patrols (kind 0x38) and influencer firing areas (0x2a).
- **Ambient `0x19d43443`, the military/hangar area, 71 objects:** `vendor_ships`,
  `vendor_vanguard`, `vendor_future_war_cult`, `vendor_dead_orbit`, `vendor_black_market`, the
  Hunter clubhouse, ship welders and signalers, droid guards.
- **Ambient `0x8cc31436`, underwatch and plaza, 113 objects:**
  - underwatch: the three class mentors (guardian / hunter / warlock), `shaxx`, `artcite`,
    workers;
  - plaza: `vendor_cryptarch`, `vendor_postmaster`, `vendor_gunsmith`, `vendor_mtx`,
    `vendor_city_bounty`, droid greeters.
- **`0xeaaf16e2`:** 16 player objects (kind 0xc).

**Every vendor and NPC is a squad (kind 1).** The host places squads (`mission-director.md`), so
a stand-in that sends only baselines gets an empty Tower. `missions/tower_npcs.json` places all
53 squads of the three ambient groups 10 s after the join. It uses the same squad action as the
mission script (`+0x44` generation, `+0x5c` = 1, still a guess).

## A first Tower run (TEST372, after TEST371 settles the squad action)

1. In orbit, choose the Tower. Note the world id in service 105 and the activity the client
   joins in the `-ahl.log`.
2. Run the join with the Tower's values. The zone set is the base slice set of the spawn
   bubble: 8 x bubble, plaza = 24.

   ```
   --ah-map city_tower_default1 --ah-world <from 105> --ah-checksum 0x046c1622 --ah-bubbles 5
   --ah-bubble-states -1,3=0 --ah-zone 24:0x811c9dc5 --ah-world-state 24:1
   --ah-sim-player 0x40aebf90:4:OWNER:1 --ah-game-mode 0x40aebf90:2:3
   --ah-mission missions/tower_npcs.json --ah-sim-clock 30
   ```

   Baselines come from the live dump (`--ah-baselines-watch`), as for the mission.
3. Try each bubble's base slice set (0, 8, 16, 24, 32) if 24 is not where the guardian lands.
