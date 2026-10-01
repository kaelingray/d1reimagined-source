# Destiny 1 preservation research (PS3 / RPCS3, Xbox 360 / Xenia)

This repository tracks an attempt to run a personally owned copy of Destiny 1 without Bungie's retired online services. It started on the Xbox 360 build in Xenia. The active target is now the final PS3 build, **BLES01857 01.24 (Destiny 2.3.1 / The Taken King)**, in RPCS3, with loopback-only stand-ins for every online service the client talks to. Game files, firmware, keys, captures and logs stay out of Git in the ignored `local\` folder (see [Data policy](#data-policy)).

## Current target: 01.24 (2026-10-01)

01.24 accepts local sign-in, persists a character and its setup answers, plays the prologue, and joins the local activity host. The level-1 intro is activity **229**, with scenario checksum **`0x8fe07299`**. With the simulation player, game mode and replication baselines supplied, FINAL124_JOIN12 clears loading and briefly reaches stage **36, `activity:in_world`**. It then requests slice 24 and leaves with **lobster**, engine failure reason `0x10c`; the Ghost sequence and player movement still need verification. Activity 231 requires level 25, while 230 has no scenario. Menu flicker and distorted character lighting are fixed together by reading local colour buffers only when the SPU has written them; mission rendering on 01.24 remains unverified. The [current checkpoint](notes/checkpoint-2026-10-01-profile.md) records the evidence; the 01.00 milestones below remain historical.

The private static inventory now covers **315 packages, 921 named tags, 183 scenarios and 1,339 entity groups**, with zero extraction errors. These are game-data records, not 183 playable missions. [`dump_mission_catalog.py`](scripts/dump_mission_catalog.py) preserves scenario checksums, group metadata and baselines across the local planet/activity packages. The [runbook](notes/next-session-playbook.md) explains how to reproduce it. Fast state reads use the run's exact emulator identity and do not pause it.

The earlier stage-19 waits were resolved by registering the full package chains, including the shared manifests. The default launcher now builds the 837-entry manifest with `--chains`.

The latest gate diagnosis needed correction: `crorc` was interpreted backwards, and the emulator's PPU hook logged before its scheduler status check, allowing retries to consume an entire watch budget. The repaired diagnostic patch and direct bootflow reader are described in the [01.24 audit checkpoint](notes/checkpoint-2026-09-30-final124-audit.md). Read that checkpoint before acting on the older STUN/gate theories in `final-build.md`.

`Invoke-Final124Run.ps1` now uses explicit/runtime-relative paths, records the processes it starts, sends input only to its own emulator, and reports protocol evidence instead of counting listener banners as progress. An imported runtime must be selected with `-Local`; the repeatable commands are in the [runbook](notes/next-session-playbook.md).

## Historical 01.00 milestone (2026-09-27)

**The PS3 client signs in, reaches orbit, launches the first story mission (A Guardian Rises, `act_cosmo_m0_highway`), loads it, and spawns the guardian in the mission world.** Every service is a local stand-in; nothing contacts Sony, Demonware or Bungie.

After the latest runs (TEST194-251):

- All 11 arrival loading tasks complete and the loading screen is dismissed. Zone set 104 (bubble 13's base) is loaded and active.
- The stand-in activity host replicates all 121 of the mission's objects (type-5 object stream): the local player's **simulation player** (with the spawn grant) and the game-mode object in its "playing" state.
- The peer family carries the character summary, so the client builds the player's appearance and blueprint. The spawner picks a spawn point and **creates the player's unit**.
- The screen shows the world with the HUD radar. On top are an "Urgent message" dialog ("Destiny servers will go offline temporarily…", dismissed with X) and a "Level Up" toast. The dialog's trigger is still unknown; ruled out so far: the client config reply, services 701/702, the BAP shutdown notification, the heartbeat host message and the universe-gated message list (TEST230-251).
- **In-mission rendering is fixed with a patched RPCS3 build (TEST229).** Stock RPCS3 showed black/magenta blocks and overexposure after moving. Destiny's SPU lighting jobs write the HDR light buffer directly in video memory, and HUD targets alias that memory. RPCS3's surface cache checks only three memory samples per surface, so it missed the SPU writes and rebuilt the buffer from stale cached pieces. [`patches/rpcs3-destiny-fix.patch`](patches/rpcs3-destiny-fix.patch) tracks SPU DMA writes to video memory per page, so affected surfaces are reloaded from guest memory. The game config needs Write Color Buffers, Read Color Buffers, Read/Write Depth Buffer and Handle RSX Memory Tiling enabled.
- **Long sessions no longer run out of memory (TEST249: 30-minute soak, every frame clean).** Stock RPCS3 grew by about 14 GB a minute in the mission and died 7-12 minutes in. Every descriptor set reserved about 0.9 MB, and Destiny cycles through about 255k cached pipelines. The same patch shrinks those pools and releases the descriptor pools of pipelines that have been idle for 256 frames.

**The guardian now spawns at the real mission start (TEST342):** the highway outside the Cosmodrome Wall, in daylight among the rusted cars. Until then runs had used bubble 13, Dock 13, which is the end of the mission. The mission's 22 bubbles are the Cosmodrome's areas (bubble map in the orbit checkpoint): 0 Skywatch, 2 The Blast, 3 The Breach, 6 Mothyards, 7 The Steppes, 13 Dock 13, **14 the highway start**, 21 The Devils' Lair, others still being mapped.

What a real host still has to provide there, from analysis of the executable (details in the orbit checkpoint):

- **The Ghost-revival intro** is a cinematic slice set, zone set 113 (bubble 14's second slot). The spawner refuses to spawn in a cinematic slice set. Slot 113 holds a trigger object (kind 4, a step counter) and a cast list (kind 6, player ids). Experiments to start it are running (TEST350-363).
- **Objectives** such as "Enter the Wall" are replicated script state machines: per-bubble objective sensors (kind 3, 24 states each) and step objects (kind 4; bubble 14 has 28). The stand-in sends them all at state 0, so no objective starts yet.
- **Enemies** come from the bubble-local objects (kinds 1, 2, 4), still to be driven.

**What is still missing** (see-through guardian = no equipped gear, mission scripts, loot, XP, item services, other activities) and the data wanted for it: [`notes/feature-gaps.md`](notes/feature-gaps.md).

Work has moved to the final PS3 build, 01.24: [`notes/preservation-plan.md`](notes/preservation-plan.md). A static comparison with 01.00 ([`notes/final-build.md`](notes/final-build.md)) found stable type handles and web services, an unchanged sign-on reply, and no Demonware lobby step. It also records the RPCS3 patch addresses for that build.

The mission start is scripted from the stand-in (`--ah-mission`, [`notes/mission-director.md`](notes/mission-director.md)): revival cinematic, objectives with dialogue, trigger-driven steps and Fallen squads, still to be run.

Static analysis of the EBOOT (2026-09-27 evening) shows a mid-mission bubble change starts on the client, from the player's position (world-controller update `0x31dab0` -> engine request slot 2 "Change slice-set"), not from a host world-state push; the next run walks into The Breach to confirm it ([runbook](notes/next-session-playbook.md)). Also next: the urgent-message trigger, an intermittent emulator freeze in the login cinematic (40 % of boots since the last rebuild; retried automatically), and upstreaming the RPCS3 fixes (drafts in [`notes/rpcs3-upstream-draft.md`](notes/rpcs3-upstream-draft.md), not filed). Tests now run on a second PC (see "Moving to another PC"). The next PC session's ordered experiments and commands: [`notes/next-session-playbook.md`](notes/next-session-playbook.md). Detailed, dated findings: [`notes/checkpoint-2026-09-25-orbit.md`](notes/checkpoint-2026-09-25-orbit.md); per-run results: [`notes/test-log.md`](notes/test-log.md).

### Gates passed on the PS3 route

Each gate was cleared one at a time. Destiny's error code names are in bold.

| Stage | What was needed |
| --- | --- |
| Title screen | RPCS3 renders a clean "Press X to Play" (Xenia's 360 title screen stays corrupted). |
| **canary**, **buffalo** | RPCS3 network status Connected; a loopback RPCN server with a local account for the PSN check. |
| **marionberry** | DNS sinkhole plus a Demonware STUN/NAT stub. |
| Sign-on HTTPS | A minimal SSLv3 server, and an RPCS3 patch replacing the root CA embedded in the executable with a private test CA. |
| SignOn / auth3 / lobby | Replies built from the game's own protobuf and bdByteBuffer formats; the Demonware lobby challenge derivation was recovered from the client. |
| Content checks | Local title config and content manifest (unsigned manifest accepted through an RPCS3 patch). |
| **centipede**, **pig** | A Bungie session server (BAP) stand-in: sealed `shello`, GCM-encrypted requests, account ID translation, universe object with the server clock. |
| Character creation / select | Services 501/504, character store, self-object deltas. |
| Orbit | Reached with the Titan character ("Set Destination", post-launch state). |
| Story restriction | Account/character unlock flags; universe flag 215 override for the mission's permission check. |
| **currant**, **cloudberry** | Demonware matchmaking (`createSession`) replies; activity-host proxy address (`ahp`) reply. |
| Activity host join | Activity-host manager reply, host join response, entity indices, host establish, activity state. |
| **lime** (activity resources) | Activity state naming the scenario, world 126, activity checksum, bubble count. |
| **lobster** | Zone record in the activity state (`--ah-zone`). |
| **baboon** (`private_activity_host_host_crashed`) | Periodic host messages (`--ah-heartbeat`). |
| Zone set load and activation | Host world-state delta in host message type 11 (`--ah-world-state`); zone set 105 becomes active. |
| Peer in the simulation roster | Host type-5 simulation packets with the client in the roster (`--ah-roster`). |
| Local player created and bound | World-state member mask (+0x5028) in the type-11 delta: the machine table keeps our machine and the group code creates the player. |
| Replicated objects applied | Baselines for all 22 replicated objects in the type-5 object stream (`--ah-baselines`); the client applies nothing until every object has one. |
| Simulation player | Simulation player 5 replicated with the local player's id as owner (`--ah-sim-player`); the client binds its player to it. |
| Loading screen dismissed | Game-mode object replicated in a non-zero state (`--ah-game-mode`): all loading tasks complete. |
| Spawn allowed | Zone set at a bubble base (`--ah-bubble-states -1,13=0 --ah-zone 104`), game-mode state 3, sim-player spawn grant (`--ah-sim-player ...:1`). |
| Guardian spawned | Peer family (0) with the account and character summaries (`--peer-family`): appearance and blueprint built, player unit created. |
| Mission start area | Bubble 14, zone set 112 (`--ah-bubble-states -1,14=0 --ah-zone 112`), with its 138 baselines: the highway outside the Wall. |

## Reproducing a run

Everything runs on one Windows PC against a private RPCS3 copy (`local\RPCS3_TEST048_SETUP`) and the game working copy in `local\01_WORKING`. The scripts find these through `scripts/Paths.ps1` and `server/local_paths.py` (`D1_LOCAL` overrides the location). `scripts/Start-LocalStack.ps1` starts the loopback services, RPCS3 and a GDB daemon. `scripts/Invoke-TitanLogin.ps1` drives the title screen and character select unattended. `scripts/Invoke-JoinCycle.ps1` does both, adds the activity-host proxy, and can trace functions through the GDB daemon while the client joins the mission. `scripts/Invoke-MissionRun.ps1 -Name TESTnnn [-Bubble 14]` wraps it with the full argument set below; `scripts/Invoke-ObjectExperiment.ps1` adds the live baseline feed, timed screenshots and object exports for replicated-state experiments:

```powershell
scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
  -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
  -ExtraBap @('--self-flags','2','--universe-flag-values','215=1','--peer-family') `
  -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','act_cosmo_m0_highway',
            '--ah-world','126','--ah-checksum','0x79130968','--ah-bubbles','22','--ah-bubble-states','-1,14=0',
            '--ah-zone','112:0x811c9dc5','--ah-world-state','112:1','--ah-roster','0',
            '--ah-sim-player','0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1','--ah-game-mode','0x4786c0e0:3:3',
            '--ah-baselines','@local\SIGNON_LOCAL\baselines_highway_z112.txt','--ah-heartbeat','1') `
  -TraceAfter 'ahp req' -Trace 0x3204a8 -Dumps 'r5:64'
```

The baselines file lists the mission's replicated objects as `KEY/KIND/INDEX[/B]`, comma-separated. It is dumped from a running game with `scripts/rpcs3_replicated_objects.py` once the zone set has loaded (138 objects for bubble 14, 121 for Dock 13's `baselines_highway_z104.txt`); `--ah-baselines-watch FILE` lets the stand-in pick up a list written during the run. The sim-player owner is the local player id (`pla +0x18`), which follows from the stand-in account.

One cycle takes 8–10 minutes. Useful companions:

- `scripts/rpcs3_activity_poll.py`: read-only watcher for the activity loader, world controller, zone state and object table.
- `scripts/rpcs3_gdb_daemon.py`: breakpoint traces with register and memory dumps; `rN:0` prints a register. **RPCS3 ignores GDB breakpoints under the LLVM PPU decoder**, so traces have been empty since at least TEST202. The instrumented build replaces them with a PPU function-entry hook: watches go in `d1\d1ppu.txt` next to `rpcs3.exe` (`ADDR [MAXHITS] [rN:LEN ...]`, or `coverage` to log every function's first call), and the output is `D1P`/`D1C` lines in `RPCS3.log`. `scripts/rpcs3_text_marker.py` marks when a UI text appears in memory. Details: [orbit checkpoint](notes/checkpoint-2026-09-25-orbit.md).
- `scripts/rpcs3_tiger_types.py`: dumps the game's runtime type definitions, used to export the typed message schemas.
- `scripts/rpcs3_replicated_objects.py`: lists the client's replicated objects in the `--ah-baselines` format.
- `scripts/Invoke-RpcsStep.ps1`: key presses and in-emulator screenshots (the instrumented build takes one when `d1\d1shot.on` appears, so the game window can stay in the background); `scripts/Invoke-MovementCheck.ps1` with `scripts/frame_blocks.py` scores in-mission frames.
- `scripts/d1trace_timeline.py`, `scripts/d1dump_scan.py`, `scripts/Invoke-MoveDump.ps1`: analysis of the instrumented RPCS3 build's timeline traces and render-target dumps.
- `scripts/Invoke-SoakTest.ps1`: long in-mission runs (periodic movement, scored screenshots, RPCS3 memory).
- `scripts/Invoke-BubbleSweep.ps1`: boots each given mission bubble in turn, feeds its replicated objects back to the stand-in (`--ah-baselines-watch`) so the guardian spawns there, and screenshots the Ghost's nav mode (location name).
- `scripts/ah_log_summary.py LOG`: the client -> activity-host messages of a proxy log (`TESTnnn-ahl.log`), showing only the first occurrence of each type/body next to the stand-in's pushes, to spot new client reports among the ~30/s type-15 messages.
- `scripts/d1_eboot.py`: static analysis of the decrypted EBOOT (`local\RESEARCH\ps3_decrypt\EBOOT.elf`): strings, references to an address or string (SNC `lis`+`addic`/`ori` pairs, TOC loads, data pointers), callers, the containing function and disassembly, e.g. `d1_eboot.py xref "Change slice-set"`.
- `scripts/d1_addresses.py`: index of every EBOOT address in the notes with curated names ([`notes/eboot-labels.tsv`](notes/eboot-labels.tsv), generated [`notes/eboot-addresses.md`](notes/eboot-addresses.md)); `d1_addresses.py 0x3204a8` prints what is known about an address, `--ghidra FILE` writes a Ghidra script that labels the decrypted EBOOT.
- `scripts/d1_pkg.py`: offline reader for the disc's Tiger packages (named tags, entries, blocks, extraction; compressed blocks need an Oodle decoder). Public Destiny networking and package research applied to the stand-in: [`notes/public-research.md`](notes/public-research.md).
- `scripts/stun_scan.py`: searches a decrypted EBOOT for the immediates that shape the STUN packets (`--li`, `--word`, `--cmpwi`, scoped to the Demonware code range); used for the 01.24 STUN format hunt ([01.24 checkpoint](notes/checkpoint-2026-09-28-final124-run.md)).
- `scripts/d1_offset_scan.py`: finds every access to a struct field (`--offset 0x4460`), for questions of the form "what else touches this field?". Used to locate the SignOn field-13 consumer in 01.24.
- `scripts/Set-PadConfig.ps1 -Mode keyboard|xinput`: switch the RPCS3 copy between the keyboard pad the scripts drive and the user's XInput controller (the XInput config is active at the moment).

Unit tests: `python -m unittest discover -s server -p "test_*.py"` and `python -m unittest discover -s scripts -p "test_*.py"`.
Before a scripted mission run, use `python scripts/d1_mission_preflight.py --schema <activity_schema.json> missions/a_guardian_rises.json missions/tower_npcs.json` to check its state fields against the selected client build.
For a full build check covering join, world, player, game mode, gear, unlock flags and service mappings, use `scripts/d1_build_preflight.py` as shown in the [runbook](notes/next-session-playbook.md).

### Moving to another PC

Copy the whole repository folder, including `local\` (about 80 GB), to the other PC. There, install PowerShell 7 (`winget install Microsoft.PowerShell`; the scripts use .NET APIs that Windows PowerShell 5.1 lacks, and must run under `pwsh`), Python 3.14 with `pip install -r requirements.txt`, and the GPU's Vulkan driver. Then set the Vulkan adapter name in both RPCS3 configs (`config.yml` and `custom_configs\config_BLES01857.yml`, `Adapter:`) to the new GPU, and fix the one absolute path in `local\RPCS3_TEST048_SETUP\config\games.yml` to the new `local\01_WORKING\ps3_destiny\` location. Leave out (or rename) the Vulkan pipeline cache, `local\RPCS3_TEST048_SETUP\cache\BLES01857\ppu-*\shaders_cache`: on a different GPU RPCS3 rebuilds all of it (about 445k pipelines) at boot, which froze the second PC's desktop. The PPU LLVM objects there are built for the source CPU, so a different CPU recompiles them once (20-40 minutes, black game window); do one warm-up boot before running unattended scripts, whose login step gives up after 15 minutes. The RPCS3 build in `local\` runs as is. The source tree and toolchain (`C:\dev\rpcs3`, Qt, Vulkan SDK; see the orbit checkpoint) are only needed to rebuild it.

Rebuilding RPCS3 on the second PC (MATILDASPC, done 2026-10-01; source, Qt and Vulkan SDK copied to `F:\d1\lukespc-import\dev`):

1. Install Visual Studio 2026 Build Tools with the C++ workload (MSVC 14.51, Windows SDK 10.0.26100; about 3.5 GB on F:, a few GB on C:):
   `winget install -e --id Microsoft.VisualStudio.BuildTools --override "--quiet --wait --norestart --installPath F:\VS\BuildTools --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
2. Apply the diagnostic patch to a clean tree at `b5a174e93`: `git reset --hard`, `git apply --check` then `git apply patches\rpcs3-destiny-main-memory-blit.patch`, then `git add -N rpcs3/Emu/RSX/Common/local_mem_writes.h rpcs3/Emu/RSX/d1diag.h`. For 01.24 also apply `patchespcs3-destiny-spu-color-reads.patch` on top (the setup-menu magenta fix; deployed only to `RPCS3_FINAL124_SMOKE001` until 01.00 mission lighting is rechecked with it).
3. Move aside any copied CMake build folders under `build\tmp` whose `CMakeCache.txt` names another PC's path (here `glslang`, `libcurl`, `openal-soft`, `protobuf_build`, kept as `*.from-lukespc`). Their third-party libraries then reconfigure; everything else builds incrementally.
4. Build from PowerShell, with the VS Installer folder on `PATH` (the third-party CMake steps call `vsdevcmd.bat`, which needs `vswhere.exe`):
   ```powershell
   $env:PATH = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer;' + $env:PATH
   $env:VULKAN_SDK = 'F:\d1\lukespc-import\dev\vulkansdk'
   $env:QTDIR = 'F:\d1\lukespc-import\dev\qt\6.11.2\msvc2022_64'
   cd F:\d1\lukespc-import\dev\rpcs3
   & F:\VS\BuildTools\MSBuild\Current\Bin\amd64\MSBuild.exe rpcs3.sln /t:rpcs3 /p:Configuration=Release /p:Platform=x64 /m:4 /nr:false
   ```
   The result is `bin\rpcs3.exe` (about 25 minutes the first time). Check that it contains `kusa-d1p3`; a local build won't byte-match one from the other PC.

## Repository layout

- `server/`: loopback service stand-ins (DNS, STUN, SignOn/auth3, Demonware lobby, BAP session server and activity-host proxy, investment/bit-stream encoder) and their tests. See [`server/README.md`](server/README.md).
- `scripts/`: stack control, unattended input, the GDB tracing daemon, guest-memory readers and pollers, and the Xbox/Xenia helper scripts.
- `notes/`: research notes. [`ps3-route.md`](notes/ps3-route.md) covers setup, the dated checkpoints ([character](notes/checkpoint-2026-09-24-character.md), [orbit and activity](notes/checkpoint-2026-09-25-orbit.md)) record findings and addresses, and [`test-log.md`](notes/test-log.md) holds per-run results.
- `patches/`: RPCS3 source patches for the PS3 route (`rpcs3-destiny-fix.patch`: the rendering fix; `rpcs3-destiny-main-memory-blit.patch`: the fix plus the `D1T`/`D1DIAG` tracing and one-frame surface dump used to find it; `rpcs3-destiny-spu-color-reads.patch`, applied on top: colour targets in video memory are loaded from guest memory only where SPU DMA wrote, which removes the 01.24 setup-menu magenta blocks) and Xenia patches from the Xbox 360 route.

## Xbox 360 / Xenia route (paused)

The Xbox 360 installation, content packs and title update were captured and verified by SHA-256 (`notes/inventory.md`). In Xenia Canary the game reaches a distorted "Press A to Play" screen and stops at error **canary** after pressing A. A local Xenia patch fixed the Content 1 STFS import (`patches/xenia-content1-stfs.patch`, `notes/content-analysis.md`). Faking the Ethernet link, online sign-in state and XHttp startup did not clear canary (`notes/network-trace.md`). Xenia has no Live/XHttp backend to build on (`notes/network-options.md`), and the graphics corruption is a separate issue (`notes/graphics.md`). The PS3 route renders correctly and exposes the same services with better tooling, so work moved there. The Xbox capture procedure and the first Xenia experiment are in the appendix of `notes/inventory.md`; per-test results are in `notes/test-log.md`.

## Data policy

Never commit game images, firmware, emulator builds, save data, keys, certificates, raw captures, logs, exported schemas or screenshots. They live in the git-ignored `local\` folder (`SIGNON_LOCAL`, `RPCS3_*`, `00_ORIGINAL_DUMP`; moved there from `D:\Destiny360` on 2026-09-27, so older notes still use that path). All listeners bind to loopback addresses only, and the DNS sinkhole never forwards upstream.
