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

## Update (comparisons 117–125): destination travel, not an activity
- The director's launch after the story cutscene is a **destination travel**: `0x32f5e0`
  calls `0x41eec8(sel, {destination 0x7e = 126, flag}, activity 0)` on the
  investment_activity_selection_manager `*(0x1906960)` (+0 i16 destination, +2 u8, +4 u32
  activity) and enters state 0x18. With activity 0, `0x30c448` makes the activity loader skip
  the activity index/event 0x36 (state 3), so the destination is expected to load through the
  world change. (Changing the latched fireteam value at runtime did not help: experiment
  TEST118.) Event 0x36 is what sets the activity index and loads a scenario plus its three
  parts (`0x30d488` -> `0x1f4a54`, `0x1f4f00`/`0x1f4e60`, commit `0x1f5300`).
- World change = engine request slot 3 (`*(0x18a4328)`, flags +0x114 + 4*i: 1 posted, 2/3 in
  progress; handler `0x302728` -> `0x66be30`). With world 125 (service 105's value, which looks
  like the client's *current* world) the loaded scenario (`ambient_cosmo`, `cosmo_patrols_1`)
  becomes active and is dropped at once -> "no active activity"; orbit's own
  `ambient_bootstrap` stays active with the same flow. So the state's world must be the
  destination's: 126.
- World 126 needs the destination's unlocks: with `--self-flags 1;2@B0,C0-1023` it fails the DLC
  check (`0x4105c4`, unlock expression via `0xb67bc4`); with `--self-flags 2` (all true) it
  passes and the next gate is "World-transition requires permissions." (0x18, 0x17) from
  `0x40ddc4` -> `0xb4f494`: three destination checks `0xb51c64` (uses a float from `0xb5016c`,
  probably level), `0xb51894`, `0xb51698` (the last only for the fireteam leader).
- `scripts/rpcs3_activity_poll.py` polls these structures (read-only) beside a GDB trace.

## Update (comparisons 126–136): activity 126 permission solved
- The "world" in the fireteam target / activity selection is an **activity index**: 126 = the
  first story mission (its definition, found at run time, names `act_cosmo_m0_highway`).
  Service 105's 125 is something else (probably the current activity/world).
- "World-transition requires permissions." came from `0xb51698` (leader-only check): the
  activity's first unlock expression is `NOT flag[215]` (ops `01 00 00d7`, `02`; format: u32
  count, u32 offset, 4-byte ops from +offset+0x14 = u8 opcode, u8, i16 index; opcode 1 = flag
  == 2, 2 = NOT, 3..9 binary). The flag table there is the **universe** investment object
  (cache header type 0xf04cc38e, soid 0x7fffffffffffffff; table at 0x7009fe8a), defaults from
  content, flag 215 = 2. The universe struct's +0x3c list (0x8080185c = {opt i16 index,
  opt u2 tri-state}) overrides it: `bap_server.py --universe-flag-values 215=1` (sign-in and
  service 104) -> the check passes (TEST135).
- Remaining: the world change to activity 126 with scenario `act_cosmo_m0_highway` (or
  `ambient_cosmo`) loads it into the resource manager, which is emptied again at once ->
  "Transition completed, but we don't have an active activity." The activity loader still
  aborts earlier because the activity selection's +4 is 0 (`0x30c448`); +4 = 1 is set by
  `0x3291a4` (copy of the fireteam target) only when the game state `0xb26d88()` is 8 and the
  current world passes `0x305608`. Next: how the real flow reaches that state (or supplies
  +4) before the loader runs.
- Tooling: `Invoke-JoinCycle.ps1 -TraceAfter 'ahp req'` starts the trace at the join, so polled
  functions do not use up -Hits.

## Update (comparisons 137–148): into the activity world, then "lobster"
Full run (about 10 minutes):

    scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
      -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
      -ExtraBap @('--self-flags','2','--universe-flag-values','215=1') `
      -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','act_cosmo_m0_highway',
                '--ah-world','126','--ah-checksum','0x79130968','--ah-bubbles','22')

- Resource manager (`*(0x18b477c)`): names +0 / +0x148 (active buffers), +0x294 (requested);
  part slots +0x100 (active) / +0x394 (requested), kinds = suffix table 0x175ccb0 (4
  `:scenario_client`, 15 `:root`, 6..14 transition cinematics = index*3 + in/filler/out,
  index 0 spaceflight / 1 same_dest / 2 no_ship). Parts move to the active buffer only when the
  load completes with status 2 (`0x1f50fc`); an empty buffer loses its name.
- The world change (engine request slot 3, handler `0x302728`) loaded the scenario, then
  `0x147cd4` -> game `0x304628` failed and logged `load-failed` (unloading it again). Causes, in
  order: `0x258550` "activity checksum mismatch" (host state u32 @0x5c must equal the scenario
  definition's word at +8: 0x79130968, read at compare time), then `0x1c7608` "activity client
  bubble count mismatch" (host state i32 @0x10 = scenario bubble count, 22; i8[64] @0x14 bubble
  state indices, -1 = default).
- With both: the world load succeeds, the transition completes (status 0x21) and the client is
  in the activity world for ~1 s, sends activity-host messages 37, 20 (1536 zero bytes), 36
  (4 bytes), then 14/21 and leaves: "You have been returned to Orbit due to an error"
  (**lobster**, failure (0x18, 0xfe) from `0x327e44`, reason via `0x326fe4`/`0x3268d8`).
  Next: what the host must answer after arrival (types 20/36) / what `0x326fe4` checks.
- Why "lobster" (TEST149): once in the world, `0x327d90` checks 30 leave conditions every frame
  (mask at +8 of 0x18eb978). Condition 34's handler reads a state byte at `0x30e088()+0x89`
  (>= 6 = error) and maps it (table 0x11e9ec4) to the failure reason; 0xfe = state 12, set at
  `0x31c350` when the session peer table (`0x13e2a0()`, lookup `0x140820`, 0x200 peers) has no
  peer at the expected machine index (result 0 or 5). I.e. the activity host is not a known
  peer. The peer roster is what host type-5 simulation packets carry (`0x1c85e0`: u8 epoch,
  bool, 65-bit mask + delta of the 65-slot roster type 0x80800cf7 {opt char8 id, opt u16,
  bool}, u32, then the replicated object stream `0x1ce4a4`). Next: send type-5 packets with the
  host (and the player) in the roster, epoch = simulation +0x5010.
- Client messages after arrival: type 20 = `0x80800e22` (384-word bitmap, zero), type 36 =
  `0x80800bd0` (u32 0).
- Correction (TEST150/151): the "missing peer" is object table index 0. `0x13e2a0()` (global
  0x18a943c, table `*(+4)`: +0 scenario handle, +8.. 0x200 entries of 8 bytes, masks +0x1154/
  +0x1194) is filled by `0x140ce4` from the scenario's zone data for the **active zone set**
  (table +0x110c, -1 = none). +0x110c is set by `0x13ff9c` from engine request slot 2
  ("switch zone set", handler `0x30256c` -> `0x1403dc`), posted at `0x31ebf4` by the client's
  world-controller zone state (`0x30e128()`, records at +0x8 / +0xa8, 0xa0 bytes each) - i.e.
  the zone set must come from the host's world controller (replicated state / RPC events
  0x33..0x36), which the stand-in does not send yet. The type-5 roster only drives peers
  (`0x1c8280` -> session interface `0x123478` / `0x1234c8`).
- Zone set (TEST152-154): object table slot 0 = bubble 0's first object (bubbles use slots
  8*i; in orbit slot 0 holds orbit's `:scenario_client` handle and the active zone set becomes
  0x10). For the activity the table stays empty and +0x110c stays -1. The zone set is chosen
  by the world/zone load state machine `0x4b2d00` (state at +0x100: 2 = after world load ->
  `0x4b2b9c` default bubble states -> 3 = select zone: hash from global 0x190a220 / 0x190a224 or,
  mode 2, from character data via `0x293308`/`0x28e9f0` (+0x480) -> `0x1406f0` index ->
  `0x13f8ec` load or engine request slot 2 "switch zone set"). It is driven from the activity
  loader's normal path (`0x33612c`) and in-world states (`0x3298b8`, `0x32e638`, `0x32f840`);
  in our travel-only flow it never ran before the leave check fired right after status 0x21.
  So the remaining blocker is again the travel-only activity selection (+4 = 0, `0x30c448`).
- `scripts/rpcs3_activity_poll.py` now also shows the zone object `*(0x18ea618)` and the object
  table's active zone set and first slots.
- The leave check's state-12 test (`0x31bfe0`, world controller object r30 = `*(0x18ea570)`):
  for group session `0x315450(2)` (lobby state +0x7eb8) it reads record +0x90 (or +0x130 for
  the target group): i32 object index at +0, state byte at +0x11 (== 2 -> check), and requires
  that object to exist in the scenario object table (`0x140820`, index 0 here). The leader's
  activity selection is always travel-only (+4 = 0; only the fireteam-follow path `0x3291a4`
  writes 1), so travel is the normal path for a leader; the missing piece is the group's
  activity-host object that a real host would provide (via its simulation stream) before the
  world-controller record says it should exist.
- The in-activity state object 0x18eb978 uses vtable 0x12b8888 (+0x18 leave check `0x327d90`,
  +0x1c update `0x332dec`); the zone machine `0x4b2d00` is not called from it.
- Slice set (TEST156/157): `*(0x18ea604)` (0x410 bytes) gets a 0x150-byte copy of the zone object
  `*(0x18ea618)` at +0x88 on arrival (`0x31b5c8`). On arrival the zone object becomes byte0 = 1,
  byte1 = 2, target record +0xa8 = {i32 index 0, u32 hash 0, i32 0} (default {-1, 0x811c9dc5,
  -1}), +0x148 = 01000200; the leave check then looks up object 0 (slice set +0x130), which is
  absent -> lobster. The join response's trailing fields (`--ah-join-fields`) do not affect it.
  Open: what sets the zone object's target record to index 0 / hash 0 at arrival (probably a
  default when the host provided no zone set), and what the real host supplies instead.
- Where the zone record comes from: the world controller is authoritative on the client. When
  slice set +0x88..+0x1d8 differs from the zone object, `0x31b8b0`/`0x31bf00`/`0x31e6d8` send
  it as event 0x4a (`0x828688` -> poster `0x262bc4`, struct 0x80802e0e = {u8, u8, i8,
  0x80800d9f {optional record 0x80800d82 @8, optional record @0xa0, i8 @0x140, i8 @0x141,
  u8 @0x142}}); the receiver (RPC table 0x12c3ee0 entry 11, decoder `0xeb4f28`, receiver
  `0x8289f0` -> `0x31bca0`) copies it into the zone object. The target record itself is written
  by `0x31d358(slice, state, index, &hash)` (+0x89 state, +0x130 index, +0x134 hash); on arrival
  `0x31d778` calls it with state 2 and the index/hash from: slice +0x1e8 if set and present in
  the object table, else the zone selection global `*(0x1a7134c)` (+8 name hash, looked up with
  `0x1406f0`, `0xb25bec`), else the **host activity state @0x54 (u32 index) / @0x58 (i32
  hash)** (activity object +0x50b0, `0x1c9298`). Our state left both 0, hence record {0, 0}.
  The leave check's final test (`0x31c2f8`) looks the index up with `0x140820`: status 0 or 5 ->
  state 12 (lobster), 6 -> 13, 7 -> 14; index -1 skips it. The zone machine `0x31eaf8` also
  skips index -1, and for a present object (status 2/3) posts the slot-2 zone switch with the
  record's index and hash. `bap_server.py --ah-zone INDEX:HASH` sets the two fields.
- TEST158b (`--ah-zone -1:0x811c9dc5`): the arrival record is -1 (`0x31d358` r5 = -1), slice +0x89
  stays 2 and **lobster is gone**: the client stays in the activity world for ~72 s (slice
  +0x1d2 counts 2 -> 3 -> 4 at +10 s / +17 s), then leaves with **baboon** ("Removed from the game
  world"), service 505, back to orbit. (TEST158 itself stalled at character select: the client
  stopped sending BAP frames, unrelated.) Next: what baboon checks (probably host simulation
  traffic the stand-in never sends).
- Baboon (TEST159-161): the leave is condition 30 (`0x32798c`) with reason 0x67. Reasons index the
  failure name table at `0x175eafc` in 8-byte entries at (reason - 0x18); condition 30 maps the
  activity-host session status (`0x303c68(0, 0)` = `*(0x30e048() + i*0x140 + 0x8a0)`, >= 9 =
  failed) through `0x326358` (table 0x11ea1c4, status - 9): status 36 -> 0x67 =
  `error.private_activity_host_host_crashed`. (Statuses 9..38 are the
  `private_activity_host_*` errors: 26 timed out waiting for join response, 27 entity indices,
  28 establish, 31 lost session, 33 lost connection to BAP, 35 booted, 36 host crashed, ...)
  Status 36 is set at `0x308c48` while in status 6 (in activity): `0x25b210(host)` = now - the
  host messaging object's (host +0xba8) last receive time, compared with `0x121518()` (config
  +0x13f8 unless overridden by `*(0x175c554)`) + config +0x2d10, gated by `0x1c97b8(0x11e179c)`.
  The stand-in pushed host messages only at the join, so after the timeout (~70 s in activity)
  the host counts as crashed. `bap_server.py --ah-heartbeat SECONDS[:SUBTYPE]` pushes an empty
  host message (default type 11) every SECONDS after the join, piggybacked on the client's
  ~30/s type-15 messages.
- `rpcs3_gdb_daemon.py`: a dump spec `rN:0` prints register N itself (e.g. a loop index).
- TEST158 and TEST160 stalled (no network syscalls, F12 screenshots not written, CPU still busy):
  an emulator hang, unrelated to the arguments (the same arguments worked in 158b/159/161).
- TEST162 (`--ah-heartbeat 1`, type 11 every second): **no baboon** - no failure in a 300 s
  trace window (only the transition steps 0x1e..0x21 at the join), still in the activity after
  6+ minutes (client host messages at ~30/s). The screen is the black loading screen with the
  spinning loader: the client waits in the activity world, presumably for the host's
  simulation stream (type 5: roster, replicated objects - see above), which the stand-in
  does not send. F12 screenshots need the game window to have focus (Qt shortcut); the
  PrintWindow capture of the game window works while it is unfocused.

## Update (comparisons 162–163): the in-activity loading screen
- After arrival the game-flow state is base+0x1d50 (vtable 0x12b7ff8, update `0x3281d0`): an
  11-task list at +0x30 (0x18-byte entries {u64 dependency mask, desc}; runner `0xd264dc`:
  a task runs once its dependencies are done, returns 0 not started / 1 running / 2 done;
  masks +0x138 required, +0x140 started, +0x148 done). When done == required it posts
  transition step 0x22 (the black loading screen ends there). Tasks: 0 `0x32895c`, 1
  `0x328a38`, 2 `0x328260`, 3 `0x3282e0`, 4 `0x328310`, 5 `0x328ae0`, 6 `0x3283b8`, 7 `0x3284b8`,
  8 `0x328610`, 9 `0x3286a8`, 10 `0x3287e4`. TEST162 (zone record -1): done 0xfb, task 2 stuck.
- Task 2 = world-controller zone readiness (`0x31ba0c`): a current record (slice +0x90 / zone +8)
  exists, or the target record's (zone +0xa8) index has status 3/4 in the scenario object
  table (`0x140820`: entry -1 -> 5; +0x1154 bit -> 6; +0x1194 bit -> 7; +0x110c active -> 4;
  +0x1110 pending -> 2; +0x1114 bit loaded -> 3; else 1).
- The table (`*(0x18a9440)`: +0 scenario handle, +8 0x200 entries of 8 bytes, +0x100c list of
  the scenario's bubble zone handles (22 for act_cosmo_m0_highway), +0x110c active, +0x1110
  pending, +0x1114 loaded bitset) is indexed bubble*8 + sub-slot: `0x14a8dc(i)` = 8*i (the
  bubble's base), `0x14a8e8(i, s)` = 8*i + s + 1. The sim start (`0x1c7608`, loop at
  `0x1c7800`) registers entries (`0x13f570`) from the host activity state's bubble states
  (@0x14): -1 leaves the base entry absent (only bubbles 13 and 14 got sub-slot entries, 105
  and 113); a state >= 0 registers the base and its sub-states. Scenario definition: +0x14
  count / +0x18 offset of 0x90-byte bubble entries (+0x28 name hash).
- With the target record at a present entry (`--ah-zone 105:0x811c9dc5`, TEST163) the world
  controller loads that zone set itself (`0x31f6bc`: status 1 -> `0x13f8ec(table, 105)`), it
  reaches status 3 and **tasks 0-7 complete** (done 0xff). Task 9 waits for task 8; task 8
  (`0x328610`) finishes when a current record exists or once zone byte 2 bit 0x80 is set,
  which then sets world-controller state 2 so the zone machine (`0x31eaf8`) posts the zone-set
  switch. Bit 0x80 (slice +0x8a) is set by `0x31c750` (via `0x31d054`) only when slice +0x310
  (group-1 host record, 0xd8 bytes) matches: with an activity host session (`0x304b78(0,0)`
  != 0) the record comes from the host simulation object (`0x25a988(host)` = host +0xbb8,
  records at +0x2e50 / +0x2f28: i32 zone index == slice +0x138, per-peer byte at +0x38 +
  `0x25bc10(host)` == slice +0x1d0), copied by `0x31d06c`. The stand-in never fills it (both
  records are defaults), so the load stops there. Next: which host message carries these
  records (not types 1, 2, 4, 18, 27, 29, 38, 39, 42, 43 by struct size; likely the type-5
  simulation stream).
- `bap_server.py --ah-bubble-states DEFAULT[,BUBBLE=STATE...]` sets the activity state's
  per-bubble states (default -1).
