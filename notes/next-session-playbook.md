# Next session on the PC: ordered runbook (written 2026-09-27)

The cloud sessions of 2026-09-27 could not run the game (no `local\` there). They prepared the
experiments below and the offline tools they need. In order of cost: the offline checks take
minutes and need no emulator; each mission run takes 8-10 minutes. Background and predictions:
[`public-research.md`](public-research.md). Run everything from the repository root in `pwsh`.

## Final PS3 build export (any time, no game run needed)

`scripts\Invoke-FinalBuildExport.ps1` does the following:
1. Downloads the EBOOTs of the PS3 updates 01.13 and 01.24. It reads only those files out of
   Sony's update PKGs, so this is about 60 MB.
2. Decrypts them with RPCS3 (`rpcs3.exe --decrypt`).
3. Exports every runtime type (`d1_types.py`).
4. Writes the comparison reports 01.00 -> 01.13, 01.00 -> 01.24 and 01.13 -> 01.24
   (`d1_builddiff.py`).

```powershell
scripts\Invoke-FinalBuildExport.ps1                 # add -Versions all for every update
scripts\Invoke-FinalBuildExport.ps1 -Packages       # also the full final package set (~12 GB)
```

It needs the decrypted 01.00 EBOOT at `local\RESEARCH\ps3_decrypt\EBOOT.elf`, and for the
package steps `local\oo2core_3_win64.dll` (or `-Oodle <path>`). Push these files from
`local\RESEARCH\ps3_updates` to the data repo:
- the `.md` reports;
- `types_*.json` and `handles_*.json`;
- `updates.json` and `update-list.txt`;
- with `-Packages`, the `final_*.txt` lists.

The ELFs and packages stay private.

Done once (2026-09-28): the findings are in [`final-build.md`](final-build.md).
The data repo's `types_*.json`, `diff_*.md` and handle maps have been regenerated
with the corrected type scanner and one-to-one matcher. The full export still
needs the final 012f chain (the command at the end of `final-build.md`) for
the 01.24 service table.

For the first 01.24 boot, build the RPCS3 copy with the 01.24 patch sites. Get the hash from
`RPCS3.log`, line "PPU executable hash", after one plain boot:

```powershell
& $D1Python server\prepare_rpcs3_copy.py <source> <dest> --game-ca <ca.pem> --unsigned-manifest `
    --build 01.24 --ppu-hash PPU-<hash from the log>
```

## Baselines without the live dump

The disc now gives the full object list for a zone (`preservation-plan.md`, "Done since"). For
the highway start plus The Breach:

```powershell
python scripts\d1_pkg.py baselines local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages --oodle <dll> `
  --keys 0x992bb471,0x3c2f5ed5,0x4786c0e0,0xecf574b5,0xfc0e45a3 > local\SIGNON_LOCAL\baselines_highway_disc.txt
```

Pass it as `--ah-baselines @local\SIGNON_LOCAL\baselines_highway_disc.txt` (in `-ExtraAh`). The
live watch still runs and simply finds nothing new.

## 0. Pull and check (2 minutes)

```powershell
git pull
python -m pip install -r requirements.txt        # adds capstone for d1_eboot.py disasm
python -m unittest discover -s server -p "test_*.py"
python -m unittest discover -s scripts -p "test_*.py"
python scripts\d1_mission_preflight.py --schema local\SIGNON_LOCAL\investment_schema_full.json `
  missions\a_guardian_rises.json missions\tower_npcs.json
```

`server\test_ah_session.py` now drives the stand-in end to end (handshake, join, heartbeats,
object steps) with a fake client, so a stand-in change that breaks the join shows up here first.
For 01.24, pass its corrected `types_01.24.json` export as `--schema` instead. The mission
preflight checks every scripted state before any game launch, and `--ah-mission` repeats the
check when the stand-in starts. Both scripts passed the 01.00 and 01.24 schemas on 2026-09-28;
01.24 still needs a live behavior check.

The run scripts default to the saved 01.00 schemas. On a future 01.24 run, supply the
corrected export to both stand-ins and the 01.24 package-derived service map to BAP:

```powershell
$types = (Resolve-Path ..\d1xenia-data\ps3_updates\types_01.24.json).Path
$services = (Resolve-Path ..\d1xenia-data\ps3_updates\services_01.24.json).Path
python scripts\d1_build_preflight.py --activity-schema $types `
  --investment-schema $types --services-schema $services `
  missions\a_guardian_rises.json missions\tower_npcs.json
# Include '--schema', $types, '--services-schema', $services in -ExtraBap.
# Include '--activity-schema', $types in -ExtraAh.
```

The first 01.24 boot should establish sign-on and activity join before attempting the mission
script; the static schema check cannot verify client acceptance. The EBOOT type export has
no service ID table of its own. The separate map covers all 94 package services, including
28 added after 01.00; responses for those new services still need client behavior checks.

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

Ran once (test log, 367): no switch, as expected. That run used an older watch list, though, so
the hooks below did not run. Repeat it as TEST367b with this list only if TEST368 leaves
questions. `Invoke-ObjectExperiment.ps1` now always rewrites the watch file and saves the list it
used as `<Name>-watch.txt`.

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
1, 2, 3 in the highway start and watch the HUD. The handler (`0x3561d8`) treats 3 specially, and
shows an objective only to players in its +0x2c0 mask (`mission-director.md`), so every step
sets that mask to -1 (all players):

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST369 -Bubble 14 -Seconds 240 -ExtraAh @(
  '--ah-object-at','60@0x992bb471:0x20:3:0x80802a1b={"0": 1, "0x2c0": -1}',
  '--ah-object-at','120@0x992bb471:0x20:3:0x80802a1b={"0": 2, "0x2c0": -1}',
  '--ah-object-at','180@0x992bb471:0x20:3:0x80802a1b={"0": 3, "0x2c0": -1}') -Watch @('3561d8 200 r4:16')
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST369-ahl.log
```

Look for the objective text or a waypoint in the screenshots, and for a new client message
(the trigger `0x1d/30` should report when the guardian walks into `tv_in_wall` /
`obj_m0_phase00_m1._trigger_volume` once the objective is active). If JSON quoting breaks,
put the JSON in a file and use `=@file.json`.

## 5. The intro cinematic: decoded, now part of TEST371

The cinematic handler needs a changed play counter (+0x4), playing = true (+0x8), −1 at +0x14,
and the player's machine id in the cast list (+0x9c) (`mission-director.md`). TEST360-363 set
none of these. The first step of TEST371's mission script starts `pf_104_res` that way.
`new_user_experience` (`0x4786c0e0/0x27/4`) remains a separate candidate for first-mission
gating.

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

The schema export is optional now: `scripts/d1_types.py` reads the same types, and the service
table, offline.

Read the result:
- **Solid armour:** the slot order is confirmed.
- **Armour but wrong pieces or a crash:** the slot order is off. Repeat with the armour in
  other slots.
- **Still translucent:**
  - try the 118-byte definitions (`starter:titan,1=199,2=193,3=195,4=201,5=197`);
  - then a weapon (`starter:titan,6=2008`);
  - check the RPCS3 log for "gear is still loading".
- **If the created character is not a Titan:** use `starter:hunter` or `starter:warlock`.

## 7. TEST371: the mission script (cinematic, objectives, dialogue, squads)

`missions/a_guardian_rises.json` plays the start of the first mission from the stand-in: the
revival cinematic, "Enter the Wall" with its dialogue, then the next objectives as the client
reports each trigger, and three Fallen squads once the client is in The Breach. Background and
field meanings: [`mission-director.md`](mission-director.md). Play along: after the objective
shows, walk to the Wall and on into The Breach (TEST368's walk).

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST371 -Bubble 14 -Seconds 600 `
  -ExtraBap @('--character-gear','starter:titan') `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0','--ah-sim-clock','30',
             '--ah-mission',(Resolve-Path missions\a_guardian_rises.json).Path) `
  -Watch @('4ed278 50 r4:48','4ecd08 20','b94414 100 r4:16','3561d8 100 r4:16','1f8d20 100 r4:16')
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST371-ahl.log
Select-String 'D1P|mission|--ah-on' local\SIGNON_LOCAL\logs\TEST371-rpcs3.log, local\SIGNON_LOCAL\logs\TEST371-ahl.log |
  Select-Object -Last 80
```

What to look for:
- **At +20 s, the revival cinematic.** `0x4ecd08` is hit when it starts. If the handler runs
  but `0x4ecd08` is never hit, one of its checks refused.
- **At +140 s, "Enter the Wall"** on the HUD with the Ghost's line (`0xb94414` hits). If the
  handler runs but nothing shows, repeat with objective state 2 or 3 in place of 1.
- **At the Wall,** the stand-in log shows "--ah-on rule ... matched type 6" and the next
  objective. If no rule matches, save the type-6 bodies from the log: they show how the client
  reports a trigger.
- **In The Breach,** Fallen appear 20 s after the client reports slice set 24 (`0x1f8d20` hits).
- The HUD/screen timeline from the screenshots, against the reference video.

The stand-in also writes a structured log per connection, `events.jsonl` in its capture
folder (`local\SIGNON_LOCAL\captures\bap\<time>\`). It holds every client frame and host
message with its full body, plus every log line. Push the activity connection's file with the
logs. It answers the trigger question offline:

```powershell
$ev = (Get-ChildItem local\SIGNON_LOCAL\captures\bap\*\events.jsonl | Sort-Object Length)[-1].FullName
python server\run_log.py summary $ev                    # message counts per type, rules fired
python server\run_log.py ah $ev --type 6 --distinct     # each distinct type-6 body once
python server\run_log.py bits $ev --type 6              # which header bits vary
```

## 8. TEST372: the Tower (after TEST371 shows whether squads spawn)

Pick the Tower in orbit and join with the Tower's values; the vendors and NPCs are placed by
`missions/tower_npcs.json`. The arguments and the bubble to try are in [`tower.md`](tower.md).
The world id comes from the client's service 105 in that run.

## 9. RPCS3 housekeeping (optional, 20 minutes)

The patches apply cleanly to upstream master `3fa07db78` (`rpcs3-upstream-draft.md`). A rebuild
on master picks up the PPU thread-start fixes and the boot shader-compilation change. The
login-cinematic freeze A/B (09:30 patch vs current, game files in the same place) needs about
10 boots per side to mean anything at a 40 % freeze rate.
