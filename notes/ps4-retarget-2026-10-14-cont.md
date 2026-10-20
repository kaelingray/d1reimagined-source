# 14 Oct (continued): a team-0 spawn on build 56578 (runs `teamspawn`, `team0`)

**Why a team stops the spawn.**
- With team 0 held from orbit, the spawn gate 0x415700 rejected every call: 2,947 calls, 2,644 rejects at 0x41314D, 0 accepts at 0x412DDC.
- Execute breakpoints put the exit at 0x415780 `cmp byte [rax+9], 0`. `rax` is the bundle's B block (obj+0x290).
- The +0x299 latch in 0x4098E0 (local-player branch 0x409BD1) sets only when:
  - A+0x38 (obj+0x250) == obj+0x2C8;
  - obj+0x2C8 = `movsx byte [player + 0x738]`, which is the player's **team**, sign-extended;
  - and 0x77B920 is true.
- So A+0x38 is the bundle's team field. It must be −1 for no team, **0x00000000 for team 0**. 0xFFFFFF00 does not match: the byte is sign-extended, not merged into the dword.
- Live test, Elimination, team 0: writing obj+0x250 = 0 latched B+9 = 1 within a second. The biped was created (`player+0x644 = 0x4CFAC067`).
- Same spawn point, same Ghost-out pose, **still no weapon**. Team assignment is not the weapon gate.
- `cfw-staging/sensor-clients-pvp_moonbase-bundle7-team0.txt` now serves A+0x38 = 0 for a team-0 run.

**Other dead ends this stretch:**
- `c_simulation_queue_investment_user_manager_universe_update` carries performance counters only.
- The equipped-index helper 0x2696F0 has only UI callers.
- The usable-equipment iterator's game-side user, 0x66CBE0, is datamine telemetry.
- The constructor callers 0x3CE000–0x40F1B0 are activity-script systems (spasaha, ghost link, capturables).
- We already grant all 12,288 entity slots unsolicited, so slot exhaustion is not the cause.
- The world-tag loader update 0x577120 runs every frame through 0x577D50 per desired set (+0x57F8, +0x5C20, +0x7F10, +0x7FB8). A set refills only when its vfunc +0x90 says so, and none does in world.

**Tool trap.** `d1-watchwrite.py --exec` with a small `--max-hits` killed the emulator on detach: an unhandled 0x80000004 at the watched address. Exec hits can still be in flight after the drain. Use counters for anything hot, or give exec watches a long `--seconds` and a high hit cap.
