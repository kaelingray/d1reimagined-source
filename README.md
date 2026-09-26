# Destiny 1 preservation research (PS3 / RPCS3, Xbox 360 / Xenia)

This repository tracks an attempt to run a personally owned copy of Destiny 1 without Bungie's retired online services. It started on the Xbox 360 build in Xenia. The active route is now the PS3 build (BLES01857 01.00) in RPCS3, with loopback-only stand-ins for every online service the client talks to. Game files, firmware, keys, captures and logs stay outside Git under `D:\Destiny360` (see [Data policy](#data-policy)).

## Current status (2026-09-26)

**The PS3 client signs in, reaches orbit, launches the first story mission (A Guardian Rises, `act_cosmo_m0_highway`) and stays inside the activity indefinitely. It sits on the activity's loading screen, which is not yet dismissed.** Every service is a local stand-in; nothing contacts Sony, Demonware or Bungie.

The loading screen belongs to an 11-task list in the game's arrival state. After the latest runs:

- Tasks 0–8 complete. The mission's zone set is loaded and **active** in the scenario object table.
- Task 9 is still running and task 10 has not started. They wait for the local player to be bound to a player slot.
- That binding happens at the world change: the game setup's machine table holds just our machine.
- About one second later it is undone. World-controller RPC 16 copies the activity-host connection's member list into the machine table, and that list is empty in the local flow.
- The client posts RPC 16 itself, because after a world change it acts as the session host and world-controller authority (the network role in the game setup is 0).
- There is no player record to bind in any case. Player records come from a player-join event that the group code emits only for players already on the manager's expected-player list. That list stays empty, although our player ID is in the host's join participants and in the world state we send.

Current work: finding what should fill the expected-player list (and the connection's member list). Ruled out so far:

- Host message type 43 is a *boot list*: naming the client produces error **bass**.
- Host type 24 is a per-host entity-index grant.
- The session-change listeners never fire in this flow.

Detailed, dated findings: [`notes/checkpoint-2026-09-25-orbit.md`](notes/checkpoint-2026-09-25-orbit.md); per-run results: [`notes/test-log.md`](notes/test-log.md).

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

## Reproducing a run

Everything runs on one Windows PC against a private RPCS3 copy (`RPCS3_TEST048_SETUP`) and the game working copy under `D:\Destiny360`. `scripts/Start-LocalStack.ps1` starts the loopback services, RPCS3 and a GDB daemon. `scripts/Invoke-TitanLogin.ps1` drives the title screen and character select unattended. `scripts/Invoke-JoinCycle.ps1` does both, adds the activity-host proxy, and can trace functions through the GDB daemon while the client joins the mission:

```powershell
scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
  -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
  -ExtraBap @('--self-flags','2','--universe-flag-values','215=1') `
  -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','act_cosmo_m0_highway',
            '--ah-world','126','--ah-checksum','0x79130968','--ah-bubbles','22',
            '--ah-zone','105:0x811c9dc5','--ah-world-state','105:1','--ah-roster','0','--ah-heartbeat','1') `
  -TraceAfter 'ahp req' -Trace 0x3204a8 -Dumps 'r5:64'
```

One cycle takes 8–10 minutes. Useful companions:

- `scripts/rpcs3_activity_poll.py`: read-only watcher for the activity loader, world controller, zone state and object table.
- `scripts/rpcs3_gdb_daemon.py`: breakpoint traces with register and memory dumps; `rN:0` prints a register.
- `scripts/rpcs3_tiger_types.py`: dumps the game's runtime type definitions, used to export the typed message schemas.
- `scripts/Invoke-RpcsStep.ps1`: key presses and F12 screenshots.

Unit tests: `python -m unittest discover -s server -p "test_*.py"`.

## Repository layout

- `server/`: loopback service stand-ins (DNS, STUN, SignOn/auth3, Demonware lobby, BAP session server and activity-host proxy, investment/bit-stream encoder) and their tests. See [`server/README.md`](server/README.md).
- `scripts/`: stack control, unattended input, the GDB tracing daemon, guest-memory readers and pollers, and the Xbox/Xenia helper scripts.
- `notes/`: research notes. [`ps3-route.md`](notes/ps3-route.md) covers setup, the dated checkpoints ([character](notes/checkpoint-2026-09-24-character.md), [orbit and activity](notes/checkpoint-2026-09-25-orbit.md)) record findings and addresses, and [`test-log.md`](notes/test-log.md) holds per-run results.
- `patches/`: Xenia source patches from the Xbox 360 route.

## Xbox 360 / Xenia route (paused)

The Xbox 360 installation, content packs and title update were captured and verified by SHA-256 (`notes/inventory.md`). In Xenia Canary the game reaches a distorted "Press A to Play" screen and stops at error **canary** after pressing A. A local Xenia patch fixed the Content 1 STFS import (`patches/xenia-content1-stfs.patch`, `notes/content-analysis.md`). Faking the Ethernet link, online sign-in state and XHttp startup did not clear canary (`notes/network-trace.md`). Xenia has no Live/XHttp backend to build on (`notes/network-options.md`), and the graphics corruption is a separate issue (`notes/graphics.md`). The PS3 route renders correctly and exposes the same services with better tooling, so work moved there. The Xbox capture procedure and the first Xenia experiment are in the appendix of `notes/inventory.md`; per-test results are in `notes/test-log.md`.

## Data policy

Never commit game images, firmware, emulator builds, save data, keys, certificates, raw captures, logs, exported schemas or screenshots. They live under `D:\Destiny360` (`SIGNON_LOCAL`, `RPCS3_*`, `00_ORIGINAL_DUMP`). All listeners bind to loopback addresses only, and the DNS sinkhole never forwards upstream.
