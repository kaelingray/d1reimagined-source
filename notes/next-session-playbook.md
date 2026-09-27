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

## 1. Offline static analysis (no emulator)

EBOOT (`local\RESEARCH\ps3_decrypt\EBOOT.elf`):

```powershell
python scripts\d1_eboot.py xref "Change slice-set"     # PS4 build has this; the formatter is on the switch path
python scripts\d1_eboot.py xref "Change world"
python scripts\d1_eboot.py strings --grep slice
python scripts\d1_eboot.py xref "objective sensor client update state"   # sanity: expect 0x1f6e98 in 0x1f6aa4
python scripts\d1_eboot.py callers 0x13f8ec            # who loads slice sets
python scripts\d1_eboot.py callers 0x42a498
python scripts\d1_addresses.py --ghidra local\d1_labels_ghidra.py        # optional: labels for Ghidra
```

The sanity line checks the tool against a known result first. If "Change slice-set" exists, put
its function into the TEST367 watch list below (`d1_eboot.py func <xref pc>` gives the start).

Packages (`local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages`):

```powershell
python scripts\d1_pkg.py named --grep cosmo            # activity/bubble roots by name
python scripts\d1_pkg.py named --grep highway
python scripts\d1_pkg.py info <one package from the output above>   # codec of the compressed blocks
python scripts\d1_pkg.py blocks <same package> --verify              # confirms the layout (SHA-1)
```

Record in the notes: the named tags and class hashes for `act_cosmo_m0_highway`, and the block
header `info` prints. D1 PS4 blocks start with 0xB7 and need `oo2core_3_win64.dll`; if the PS3
ones do too, extraction (`d1_pkg.py extract ... --oodle <dll>`) needs a user-owned copy.

## 2. TEST367: what carries a mid-session switch

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST367 -Bubble 14 `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0','--ah-zone-schedule','24@150') `
  -Watch @('42a498 4000 u r3:48 r4:48 r5:48','258778 4000 u r3:32 r4:32 r5:32')
```

Add a watch line for the "Change slice-set" function from step 1 if it was found. Afterwards:
`Select-String D1P local\SIGNON_LOCAL\logs\TEST367-rpcs3.log`. Look for hits after the switch at
150 s and the subtype at payload+9.

## 3. Prediction 2: walk into The Breach instead of scheduling a switch

Same setup without `--ah-zone-schedule`. The login automation needs the keyboard pad, so walk
with the keyboard too: `scripts\Set-PadConfig.ps1 -Mode keyboard`, write the TEST367 watches to
`local\RPCS3_TEST048_SETUP\d1\d1ppu.txt`, then
`scripts\Invoke-MissionRun.ps1 -Name TEST368 -ExtraAh @('--ah-bubble-states','-1,14=0,3=0')`.
Dismiss the urgent message (`scripts\Invoke-RpcsStep.ps1 -Keys X`) and walk through the break in
the Wall towards The Breach, in the game window with the keyboard mapping (W forward, J/L turn,
Space = the Ghost's nav mode, as in `Invoke-Explore.ps1`), or step by step with
`Invoke-RpcsStep.ps1 -Keys W -HoldMs 3000` and a screenshot after each step. Afterwards close
RPCS3 and run `Set-PadConfig.ps1 -Mode xinput`. Then:

```powershell
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST368-ahl.log
Select-String 'createSession' local\SIGNON_LOCAL\logs\TEST368-lobby.log
```

Expected if the talk's model holds: package opens about 15 s before the edge of the bubble, a
new client -> host message type (or a new type-15 body) near the edge, and either no host action
needed or a lobby request.

## 4. Prediction 1: sense state from an armed objective

1. Find the kind-3 objective sensors that are loaded at the highway start: in the latest
   bubble-14 export (e.g. `local\SIGNON_LOCAL\TEST363-objects.json`), list records with kind 3
   and their group/index. Bubble 14 has no kind-3 object of its own (TEST342); check the
   activity group 0x992bb471 too.
2. Arm objective 0 at 60 s and walk to the Wall as in step 3 (keyboard pad):
   `Invoke-MissionRun.ps1 -Name TEST369 -ExtraAh @('--ah-object-at','60@<group>:3:<index>:0x808004fa={"0": {"0": 1}}')`
   (JSON as in the orbit checkpoint; put it in a file and pass `=@file.json` if quoting breaks).
3. `python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST369-ahl.log`. A new type or
   body after the step is the first sense-state report. Also look for the objective text on
   screen.
4. If a report shows up, close the loop with `--ah-on TYPE/BODYREGEX=>0@<group>:3:<index>:...`
   stepping objective 0 to 2, and see whether the next objective appears.

## 5. Kind 4 (spawn slot) as squad placement

`Invoke-ObjectExperiment.ps1 -Name TEST370 -Bubble 14 -ExtraAh @('--ah-object-at','30@0xecf574b5:4:<index>:0x808007d9={"0": 1}')`
for one bubble-14 kind-4 index, with screenshots. Something should appear at a fixed spot (a
squad or a prop). If it is a combatant, kill it and check `ah_log_summary.py` for a report.

## 6. RPCS3 housekeeping (optional, 20 minutes)

The patches apply cleanly to upstream master `3fa07db78` (`rpcs3-upstream-draft.md`). A rebuild
on master picks up the PPU thread-start fixes and the boot shader-compilation change. The
login-cinematic freeze A/B (09:30 patch vs current, game files in the same place) needs about
10 boots per side to mean anything at a 40 % freeze rate.
