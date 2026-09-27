# Next session on the PC: ordered runbook (written 2026-09-27)

The cloud sessions of 2026-09-27 could not run the game (no `local\` there). They prepared the
experiments below and the offline tools they need. In order of cost: the offline checks take
minutes and need no emulator; each mission run takes 8-10 minutes. Background and predictions:
[`public-research.md`](public-research.md). Run everything from the repository root in `pwsh`.

## 0. Pull and check (2 minutes)

```powershell
git pull
python -m pip install -r requirements.txt        # adds capstone for d1_eboot.py disasm
python -m unittest discover -s server -p "test_*.py"
python -m unittest discover -s scripts -p "test_*.py"
```

`server\test_ah_session.py` now drives the stand-in end to end (handshake, join, heartbeats,
object steps) with a fake client, so a stand-in change that breaks the join shows up here first.

## 1. Offline analysis (done in the cloud session; what is left)

The cloud session ran the EBOOT and package analysis on the uploaded data (orbit checkpoint,
"Static analysis of the uploaded EBOOT" and "PS3 packages read offline"). Left for the PC:

```powershell
# the disc's package names, so manifest package ids map to names (cloud has only ten packages)
Get-ChildItem local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages -Name > local\package-names.txt
python scripts\d1_pkg.py named local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages > local\named-tags.txt
python scripts\d1_addresses.py --ghidra local\d1_labels_ghidra.py        # optional: labels for Ghidra
```

Both text files are small and hold no game data; adding them to the data repo lets the next cloud
session map the 136 bubble definitions and 81 activity infos in the manifests to packages. Tag
contents need a user-owned `oo2core_3_win64.dll` (`d1_pkg.py extract ... --oodle <dll>`): every
PS3 tag block is compressed with the 0xB7 Oodle framing.

## 2. TEST367: the slice-set switch chain, host-pushed (expected: no switch)

Static analysis (orbit checkpoint, "how a slice-set switch starts") says a mid-mission switch
starts from the player's position (`0x31dab0`) or from a pending request (`0x31bcd0`, fed by the
record at simulation +0x5080), and ends in engine request slot 2 (`0x11f91c`). These watches cover
the whole chain; the budgets are large so none runs out (TEST366 stopped at 10 hits):

```powershell
$w = @('25715c 5000 r5:0',                 # type-11 world state still handled in the world?
       '31dab0 3000 u r3+0x89:1 r3+0x90:4', # player-bubble follower: slice state / current object
       '31d358 500 r4:0 r5:0 r6:4',         # target written: state (2 arrival, 3 player bubble, 5 request), index, hash
       '31bcd0 500 r3:0 r4:4 r5:0',         # pending request raised
       '4e9470 3000 u r5:16 r6:20',         # record at sim+0x5080 vs the spawner's copy
       '11f91c 500 r4:0 r5:12')             # engine request posted (slot 2 = change slice set)
scripts\Invoke-ObjectExperiment.ps1 -Name TEST367 -Bubble 14 `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0','--ah-zone-schedule','24@150') -Watch $w
Select-String 'D1P' local\SIGNON_LOCAL\logs\TEST367-rpcs3.log | Select-Object -Last 60
```

Read: whether `0x25715c` keeps hitting about once a second after 5:21 (type 11 is handled in
the world); `0x31d358` with r4 = 2 at arrival and no 3/5 later; no `0x11f91c` with r4 = 2 after
the join. The 150 s zone schedule should change nothing.

## 3. TEST368: walk into The Breach (expected: the client switches by itself)

Same watches, no zone schedule, and a person walking. `Invoke-ObjectExperiment.ps1` switches to
the keyboard pad, dismisses the urgent message and takes screenshots; during its `-Seconds`
window walk in the game window with the keyboard mapping (W forward, J/L turn, Space = the
Ghost's nav mode, as in `Invoke-Explore.ps1`) through the break in the Wall towards The Breach:

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST368 -Bubble 14 -Seconds 600 `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0') -Watch $w
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST368-ahl.log
Select-String 'D1P' local\SIGNON_LOCAL\logs\TEST368-rpcs3.log | Select-Object -Last 60
Select-String 'createSession' local\SIGNON_LOCAL\logs\TEST368-lobby.log
```

The stand-in log now shows the client's own slice-set reports (`client slice set: current
112:..., target 24:..., state N`, from client type 21), so the switch is visible in
`TEST368-ahl.log` even without hooks. If the client switches and is then removed from the world
(lobster or a similar error), repeat as TEST368b with `--ah-follow-client-zone` added to
`-ExtraAh`: the stand-in then updates its world-state group record to the zone set the client
reports, as a host tracking the fireteam would.

Expected from the static analysis: at the bubble edge `0x31d358` with r4 = 3 and r5 = 24 (The
Breach's base slice set), then `0x11f91c` with r4 = 2, and the Breach loading, all without any
host action. Bubble 3 must be registered (`3=0`), or its slice set is not in the scenario object
table and the follower ignores it. Also look at what the client sends around the edge
(`ah_log_summary.py`) and whether the lobby sees a new `createSession` (public bubble
matchmaking, `public-research.md`).

## 4. TEST369: show the first objective (names from the disc, `mission-objects-highway.md`)

The first objective "Enter the Wall" is `0x992bb471/0x20/3`
(`cosmo_m0_phase00_highway.obj_m0_phase00_m1._player_objective`, state 0x80802a1b); its
sequences are `0x992bb471/0x5/4` (incoming) and `/5` (completion), its trigger
`0x992bb471/0x1d/30` (`._player_trigger`). Step the objective's state byte (field 0, 3 bits)
1, 2, 3 in the highway start and watch the HUD; the handler (`0x3561d8`) treats 3 specially:

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST369 -Bubble 14 -Seconds 240 -ExtraAh @(
  '--ah-object-at','60@0x992bb471:0x20:3:0x80802a1b={"0": 1}',
  '--ah-object-at','120@0x992bb471:0x20:3:0x80802a1b={"0": 2}',
  '--ah-object-at','180@0x992bb471:0x20:3:0x80802a1b={"0": 3}') -Watch @('3561d8 200 r4:16')
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST369-ahl.log
```

Look for the objective text or a waypoint in the screenshots, and for a new client message
(the trigger `0x1d/30` should report when the guardian walks into `tv_in_wall` /
`obj_m0_phase00_m1._trigger_volume` once the objective is active). If JSON quoting breaks,
put the JSON in a file and use `=@file.json`.

## 5. The intro cinematic, now that its objects are known

Slice set 113 (`cosmo_m0_phase00_highway_intro`) loads the cinematic `0x992bb471/0x6/1`
(`pf_104_res._cinematic`, state 0x8080254d with a player-id roster) and
`0x992bb471/0x4/0` (`skeleton_static_1`). TEST360-363 stepped 0x4/0 (a prop, not a trigger) and
put the player in the roster of kind 6 index 1, but the state's other fields stayed at their
defaults. Next: export the live 0x8080254d schema field list (`--schema-out` of
`rpcs3_replicated_objects.py` already covers it) and set the cinematic's start/playing fields,
with the watch `4ed278 200 r4:32`. (Bubble 14's own kind-4 objects are `o_ghost_*` lights and
`pigeon_1-22`, so stepping them is not useful.) `new_user_experience` (`0x4786c0e0/0x27/4`) is
a separate candidate for first-mission gating.

## 6. TEST370: the see-through guardian (gear)

The guardian renders translucent because the stand-in equips nothing: all 20 equipment slots of
the character summaries are empty (`feature-gaps.md`, section 1). The item list is now decoded
from the disc, and `--character-gear starter:titan` equips the Titan's starter "wire armor",
subclass and ghost shell. Screenshot the character select screen and the guardian in the
mission:

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST370 -Bubble 14 -Seconds 120 `
  -ExtraBap @('--character-gear','starter:titan')
python scripts\rpcs3_tiger_types.py schema local\SIGNON_LOCAL\investment_schema_full.json
```

The second command (with the game running) exports every service's request type, which names
the 66 services (`feature-gaps.md`, section 3). Add the JSON to the data repo.

Read the result:
- **Solid armour:** the slot order is confirmed.
- **Armour but wrong pieces or a crash:** the slot order is off. Repeat with the armour in
  other slots.
- **Still translucent:**
  - try the 118-byte definitions (`starter:titan,1=199,2=193,3=195,4=201,5=197`);
  - then a weapon (`starter:titan,6=2008`);
  - check the RPCS3 log for "gear is still loading".
- **If the created character is not a Titan:** use `starter:hunter` or `starter:warlock`.

## 7. RPCS3 housekeeping (optional, 20 minutes)

The patches apply cleanly to upstream master `3fa07db78` (`rpcs3-upstream-draft.md`). A rebuild
on master picks up the PPU thread-start fixes and the boot shader-compilation change. The
login-cinematic freeze A/B (09:30 patch vs current, game files in the same place) needs about
10 boots per side to mean anything at a 40 % freeze rate.
