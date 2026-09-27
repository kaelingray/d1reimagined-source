# Destiny 1 preservation research (PS3 / RPCS3, Xbox 360 / Xenia)

This repository tracks an attempt to run a personally owned copy of Destiny 1 without Bungie's retired online services. It started on the Xbox 360 build in Xenia. The active route is now the PS3 build (BLES01857 01.00) in RPCS3, with loopback-only stand-ins for every online service the client talks to. Game files, firmware, keys, captures and logs stay outside Git under `D:\Destiny360` (see [Data policy](#data-policy)).

## Current status (2026-09-27)

**The PS3 client signs in, reaches orbit, launches the first story mission (A Guardian Rises, `act_cosmo_m0_highway`), loads it, and spawns the guardian in the mission world.** Every service is a local stand-in; nothing contacts Sony, Demonware or Bungie.

After the latest runs (TEST194):

- All 11 arrival loading tasks complete and the loading screen is dismissed. Zone set 104 (bubble 13's base) is loaded and active.
- The stand-in activity host replicates all 121 of the mission's objects (type-5 object stream): the local player's **simulation player** (with the spawn grant) and the game-mode object in its "playing" state.
- The peer family carries the character summary, so the client builds the player's appearance and blueprint. The spawner picks a spawn point and **creates the player's unit**.
- The screen shows the world with the HUD radar. On top are an "Urgent message" dialog (dismissed with X) and a "Level Up" toast.
- **In-mission rendering is fixed with a patched RPCS3 build (TEST229).** Stock RPCS3 showed black/magenta blocks and overexposure after moving. Destiny's SPU lighting jobs write the HDR light buffer directly in video memory, and HUD targets alias that memory. RPCS3's surface cache checks only three memory samples per surface, so it missed the SPU writes and rebuilt the buffer from stale cached pieces. [`patches/rpcs3-destiny-fix.patch`](patches/rpcs3-destiny-fix.patch) tracks SPU DMA writes to video memory per page, so affected surfaces are reloaded from guest memory. The same patch also does VRAM-to-main-memory blits on the CPU. The game config needs Write Color Buffers, Read Color Buffers, Read/Write Depth Buffer and Handle RSX Memory Tiling enabled.

Next: stability in the world (host heartbeats, the replicated state that a real host would keep updating), the urgent-message flag, and controller input. Detailed, dated findings: [`notes/checkpoint-2026-09-25-orbit.md`](notes/checkpoint-2026-09-25-orbit.md); per-run results: [`notes/test-log.md`](notes/test-log.md).

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

## Reproducing a run

Everything runs on one Windows PC against a private RPCS3 copy (`RPCS3_TEST048_SETUP`) and the game working copy under `D:\Destiny360`. `scripts/Start-LocalStack.ps1` starts the loopback services, RPCS3 and a GDB daemon. `scripts/Invoke-TitanLogin.ps1` drives the title screen and character select unattended. `scripts/Invoke-JoinCycle.ps1` does both, adds the activity-host proxy, and can trace functions through the GDB daemon while the client joins the mission:

```powershell
scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
  -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
  -ExtraBap @('--self-flags','2','--universe-flag-values','215=1','--peer-family') `
  -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','act_cosmo_m0_highway',
            '--ah-world','126','--ah-checksum','0x79130968','--ah-bubbles','22','--ah-bubble-states','-1,13=0',
            '--ah-zone','104:0x811c9dc5','--ah-world-state','104:1','--ah-roster','0',
            '--ah-sim-player','0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1','--ah-game-mode','0x4786c0e0:3:3',
            '--ah-baselines','@D:\Destiny360\SIGNON_LOCAL\baselines_highway_z104.txt','--ah-heartbeat','1') `
  -TraceAfter 'ahp req' -Trace 0x3204a8 -Dumps 'r5:64'
```

The baselines file lists the mission's replicated objects as `KEY/KIND/INDEX[/B]`, comma-separated. It is dumped from a running game with `scripts/rpcs3_replicated_objects.py` once zone set 104 has loaded (121 objects for this mission). The sim-player owner is the local player id (`pla +0x18`), which follows from the stand-in account.

One cycle takes 8–10 minutes. Useful companions:

- `scripts/rpcs3_activity_poll.py`: read-only watcher for the activity loader, world controller, zone state and object table.
- `scripts/rpcs3_gdb_daemon.py`: breakpoint traces with register and memory dumps; `rN:0` prints a register. **RPCS3 ignores GDB breakpoints under the LLVM PPU decoder**, so traces have been empty since at least TEST202. The instrumented build replaces them with a PPU function-entry hook: watches go in `C:\dev\d1ppu.txt` (`ADDR [MAXHITS] [rN:LEN ...]`, or `coverage` to log every function's first call), and the output is `D1P`/`D1C` lines in `RPCS3.log`. `scripts/rpcs3_text_marker.py` marks when a UI text appears in memory. Details: [orbit checkpoint](notes/checkpoint-2026-09-25-orbit.md).
- `scripts/rpcs3_tiger_types.py`: dumps the game's runtime type definitions, used to export the typed message schemas.
- `scripts/rpcs3_replicated_objects.py`: lists the client's replicated objects in the `--ah-baselines` format.
- `scripts/Invoke-RpcsStep.ps1`: key presses and F12 screenshots; `scripts/Invoke-MovementCheck.ps1` with `scripts/frame_blocks.py` scores in-mission frames.
- `scripts/d1trace_timeline.py`, `scripts/d1dump_scan.py`, `scripts/Invoke-MoveDump.ps1`: analysis of the instrumented RPCS3 build's timeline traces and render-target dumps.
- `scripts/Invoke-SoakTest.ps1`: long in-mission runs (periodic movement, scored screenshots, RPCS3 memory).

Unit tests: `python -m unittest discover -s server -p "test_*.py"`.

## Repository layout

- `server/`: loopback service stand-ins (DNS, STUN, SignOn/auth3, Demonware lobby, BAP session server and activity-host proxy, investment/bit-stream encoder) and their tests. See [`server/README.md`](server/README.md).
- `scripts/`: stack control, unattended input, the GDB tracing daemon, guest-memory readers and pollers, and the Xbox/Xenia helper scripts.
- `notes/`: research notes. [`ps3-route.md`](notes/ps3-route.md) covers setup, the dated checkpoints ([character](notes/checkpoint-2026-09-24-character.md), [orbit and activity](notes/checkpoint-2026-09-25-orbit.md)) record findings and addresses, and [`test-log.md`](notes/test-log.md) holds per-run results.
- `patches/`: RPCS3 source patches for the PS3 route (`rpcs3-destiny-fix.patch`: the rendering fix; `rpcs3-destiny-main-memory-blit.patch`: the fix plus the `D1T`/`D1DIAG` tracing and one-frame surface dump used to find it) and Xenia patches from the Xbox 360 route.

## Xbox 360 / Xenia route (paused)

The Xbox 360 installation, content packs and title update were captured and verified by SHA-256 (`notes/inventory.md`). In Xenia Canary the game reaches a distorted "Press A to Play" screen and stops at error **canary** after pressing A. A local Xenia patch fixed the Content 1 STFS import (`patches/xenia-content1-stfs.patch`, `notes/content-analysis.md`). Faking the Ethernet link, online sign-in state and XHttp startup did not clear canary (`notes/network-trace.md`). Xenia has no Live/XHttp backend to build on (`notes/network-options.md`), and the graphics corruption is a separate issue (`notes/graphics.md`). The PS3 route renders correctly and exposes the same services with better tooling, so work moved there. The Xbox capture procedure and the first Xenia experiment are in the appendix of `notes/inventory.md`; per-test results are in `notes/test-log.md`.

## Data policy

Never commit game images, firmware, emulator builds, save data, keys, certificates, raw captures, logs, exported schemas or screenshots. They live under `D:\Destiny360` (`SIGNON_LOCAL`, `RPCS3_*`, `00_ORIGINAL_DUMP`). All listeners bind to loopback addresses only, and the DNS sinkhole never forwards upstream.
