# PS3 orbit checkpoint (2026-09-25)

Current handoff for the local RPCS3 Destiny test (supersedes the 2026-09-24 character note's
"Next step"). With the loopback stand-ins the game now signs in, shows character select with
the stored Titan, and **loads into orbit** ("Set Destination"). Rendering is still
rainbow-distorted in places; that is a separate graphics issue.

## How to run

- RPCS3 copy `D:\Destiny360\RPCS3_TEST048_SETUP` (XInput pad = Player 1, async shaders).
  Launch it directly (not `Invoke-Rpcs3LocalTest.ps1`, which makes a keyboard-pad copy).
- Services: rpcn, `dns_sinkhole.py --destiny-hosts`, `stun_stub.py`, `signon_probe.py --mode
  auth3 --destiny-hosts --manifest-url http://content.local.test/manifest --manifest-packages
  ...\USRDIR\packages`, `lobby_probe.py --timeout 60`, and
  `bap_server.py --ws-signin --schema D:\Destiny360\SIGNON_LOCAL\investment_schema_character.json
  --store D:\Destiny360\SIGNON_LOCAL\characters.json`.
- The BAP stand-in can be restarted while the game shows an error; dismissing reconnects.
- Debugging: start `scripts/rpcs3_gdb_daemon.py` right after RPCS3 and never kill it while
  RPCS3 runs (the stub serves one connection per emulator session; a second one freezes the
  game). Use `python scripts/rpcs3_gdb_daemon.py send "hits N SECONDS ADDR,..."` or `send mem
  ADDR LEN`. Each breakpoint hit pauses the game for about a second.

## What the server now does (see test-log 049–062)

- 501 create: id = account id + 1; saved to `--store`; self delta publishes the character and
  makes it active (untested since the active-character fix: Finish may now proceed).
- 504 select: reply error_none, self delta (next version) with root +0x60 = character id.
- 0x17 acct id trans: echo request + u64 account id per name.
- 0x12 client config: protobuf `{1: 1, 2: 1}` (placeholder values).
- 104: result plus universe object `{0: 0x7fffffffffffffff, 8: 1}`.
- Families 0 (peer) and 2 (roster) are pushed as empty snapshots on subscription.

## Rules learned

- BAP replies must follow request order per queue (`0x7c3458`); any unanswered request
  blocks every later reply (failure 0x12 -> centipede).
- Bootflow errors are raised by the health loop `0x327d90` (conditions table `0x11ea038`):
  15 = BAP state (0xa0 centipede), 17 = investment sign-in (0xa1 chicken, reasons 0xa2+).

## Next

- Try Set Destination / the Tower and watch for new requests; level 0 and empty gear come
  from absent character progression fields (`0x80801954` +0x30, +0x68).
- Re-test character creation (Finish) now that 501 sets the active character.

## Update (comparisons 063–081)

- Run with `--universe-time s --self-flags "1;2@B0,C0-1023"`: past trial and purchase gates;
  selecting the Titan plays a story cutscene and starts an activity join.
- Current gate: the activity join's **Demonware bdMatchMaking createSession** request on the
  lobby connection (encrypted `AB 85` frame, format in test-log 080–081) is unanswered, so
  the join fails with **currant**. `lobby_probe.py` must decrypt post-auth frames and answer
  with an encrypted task reply (server keys: AES key 1, HMAC key 1; reply format still to be
  read from the client's bdRemoteTaskManager, receive path `0xa3a640` -> type byte ->
  `0xa3e8ac`). The ahp reply (16 bytes) likely identifies the activity host/session.
- Unattended loop: `Start-LocalStack.ps1 -Name X -BapArgs ...` then
  `Invoke-TitanLogin.ps1 -Name X`, then arm traces via the GDB daemon.
## Update (comparisons 082–093): activity join protocol

The activity join is a **private activity instance**: the client runs activity-host-side
logic partly itself, but negotiates through Bungie services. Steps now answered locally:
1. Demonware lobby bdMatchMaking createSession (encrypted AB 85 frames; `lobby_probe.py
   --reply-type 0x01 --session-result`).
2. `c->b_ahp` (0x10) on the main BAP: reply `u64 request id | IPv4 | u16 | port` = address of
   the **activity host proxy** (a second BAP server; `bap_server.py --ahp-reply ...`).
3. On the AHP connection: shello (field 2 = 2), `c->ah mgr req` (0x06, 3103 bytes) answered by
   `c->ah mgr rsp` (0x07, `02 | u64 host id | char[128]`; `--ah-mgr-reply`).
4. The client then sends `c->ah not` (0x08) to the host (u64 host id, type 3, 232 bytes) and
   waits for the host's answer: currently failure (0x18, 0x2a).
Next: the activity host messages (startup response, host identifier, host join/response,
entity indices). The error-name table `0x175eafc` lists every step. Test loop:
`%TEMP%\...\scratchpad\cycle2.ps1` pattern (stack + login + trace + AHP stand-in), about 8 min.
The TEST048 copy uses a keyboard pad now; the user's XInput config is saved as
`config\input_configs\global\Default.xinput.user.yml.bak`.
## Update (comparisons 094–107): activity host join answered
One command runs a full unattended join (about 10 minutes; `-Trace`/`-Dumps` optional):

    scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
      -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
      -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','orbit','--ah-world','125')

Host messages are typed structs (investment bit format); schema exported to
`SIGNON_LOCAL\activity_schema.json` (`rpcs3_tiger_types.py schema OUT 0x80800a58 ...`, the 64
handles in the tables at `0x18cffe0`/`0x18d0000`). The AHP stand-in now:
1. answers the host join request (c->ah not type 3, `0x80800e98`) with the **join response**
   (b->c not type 4, `0x80800a58`: request echoed, result 0, session id) — `--ah-join`;
2. grants **entity indices** (type 0, 384-word bitmap) — `--ah-entities`;
3. sends host flag type 11 and an **activity state** (type 1, `0x80800e76`: bool @0, i32 @0x10,
   map name char[40] at @0x60 @0x994, world id i16 at @0x60 @0x990) — `--ah-state --ah-map --ah-world`.
Failure progression: 0x2a (join response) -> 0x2b (entity indices) -> 0x2c (establish client) ->
0x36 "loading activity resources" (error **lime**) -> with `--ah-map orbit`: "World-transition
requires DLC." (world id 0) -> with `--ah-world 125` (the i16 the client sent in service 105):
the world transition completes, then (0x18, 0x36) "Transition completed, but we don't have an
active activity." (`0x334260`: the resource manager has no map loaded).

Why: the loader (`0x3363a0`) needs an activity index `*(0x30e0c8()+0x94)`; it is -1, so after
retries it falls back to loading orbit resources (state 0x1f). That index is set by
`0x30d488`, the receiver of a **world controller RPC** (descriptor table `0x1724c60`, pairs
sender/receiver, e.g. `0xd24e14`/`0x30d488`) that the host sends over the simulation stream.
Host type 38 (`0x25b9a8`) looks like that stream's carrier: a typed header (`*(0x179eb20)`)
then u32 bit length and up to 0x1f40 bytes of raw bit stream. Next: decode the simulation
stream framing (types 38/5/7) and the world controller RPC for the activity index, or find
the map name / activity index for world 125 so the activity itself loads.

## Update (comparisons 108–112): why the activity never loads
- The loader (`0x3362c0`) now starts loading the requested scenario (`--ah-map act_cosmo_m0_highway`
  reached `0x1f4a54(rm, "act_cosmo_m0_highway")`), but within ~2 s switches to state 3 (orbit
  fallback) without computing the activity index: `0x30c448` says abort. Its test `0x323ed0`
  aborts while the **group target** session record (group manager `*(0x18ea68c)`, record
  +0x7f8, valid bit +0xd0, world i16 at +0xd8) is pending and the **group current** record
  (+0x450, hash at +0x524) still equals the hash latched on entering orbit (`0x323e6c`, global
  `0x18ecdb8`). I.e. the fireteam's move from the current group session to the target group
  session (the Demonware lobby session from createSession) has not completed, so the client
  follows the group instead of loading the activity; the world transition then ends with
  "no active activity".
- Scenario names are in guest memory as registry keys (`act_cosmo_m0_highway:scenario_client`,
  `city_tower_default1:…`, `.map:cosmo…`), so valid names can be listed from a running game.
- Next: find what promotes the group target to current (lobby session join/state in
  `lobby_probe.py`: bdMatchMaking joinSession / session updates, or a BAP fireteam message),
  then the loader should compute the index, post event 0x36 and load the scenario.

## Update (comparisons 113–116): fireteam target blocks the activity load
Polling guest memory directly (no GDB; `rpcs3_guest_read.py`) during a join shows the group
manager `*(0x18ea68c)` with two groups (names table: fireteam_target / posse_target /
group_target; group g at mgr + g*0x11938, session buffer +0x10 + [+8]*0x8c98, lobby state
+0x7eb8, data +0x63b0). Group 0 = fireteam: current record at data+0x450 (vtable 0x12b7f58),
target record at data+0x7f8 (vtable 0x12c3ae0), each with valid +0xd0.
- When the director launches (about 6 s before the join), the fireteam **target** becomes valid
  (world i16 0x7e at +0xd8, 64-bit ids at +0xe0/+0xe8, +0xf0) and the **current** record is
  re-validated with hash +0xd4 = 0x811c9dc5 (FNV-1 of an empty string) - the same value latched
  on entering orbit (`0x323e6c`, global 0x18ecdb8). `0x323ed0` therefore reports "fireteam is
  travelling" and the activity loader aborts (`0x30c448` -> state 3).
- The world transition state object (static 0x18eba30, vtable 0x1725c40..: enter `0x333c04`,
  update `0x334004`) runs in phases: +0xa20 odd = copy the host scene (world 125,
  `act_cosmo_m0_highway`) - works; then `0x333dfc` bumps +0xa28 and phase 1 needs an active
  scenario in the resource manager, which the aborted loader never loaded -> "Transition
  completed, but we don't have an active activity."
- `--ah-world 126` (the fireteam target's world) fails the DLC check; 125 is right for the state.
- Host type 2 (`{bool, float}`, sets join flag 0x80) did not change this.
Open question: what makes the fireteam current record's hash non-empty (or clears the
target) once the target activity host is joined. The record is rebuilt locally by the
leader (`0x7bc690` -> `0x807d58` -> `0x805c28` diff -> `0x8045ac` copy); its source is built
in the session/peer code around `0x7bc600` (fields at +0x21b4..+0x21d8 of that object).
