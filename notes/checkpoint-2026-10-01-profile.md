# 01.24 profile setup continuation — 2026-10-01

Source baseline: `4fdd051`, matching `origin/master`. Claude's latest work has
resolved the package-registration and qz object-hash blockers: the final
837-entry manifest registers every required patch chain, the 01.24 BAP start
response echoes its 128-byte request, and build-specific qz hashes let the family
push pass investment sign-in. `FINAL124_HASH1` reached stage 21 `profile_setup`
and the "SET SCREEN BOUNDS" screen. Its stack was already closed when this
continuation started. The earlier September 30 stage-19 waits are historical.

Runtime remains `F:\d1\lukespc-import\d1xenia\local`; source is
`F:\d1\d1xenia`, data is `F:\d1\d1xenia-data`. 01.24 uses its own private
`SIGNON_LOCAL/characters-0124.json` store. The 01.00 store is preserved.

## Observation after BAP

`Invoke-Final124Run.ps1 -ContinueAfterBap` now keeps observing for the full
`-BootSeconds`, and keeps scheduled input active until `-PressSeconds`, after
the first BAP connection. It reports that connection once and records the mode
in the run summary. The default still ends at the first BAP connection.
Protocol counts do not prove profile setup, character creation, or orbit;
screenshots, direct bootflow samples, and matching service traffic are needed.

The isolated Windows regression test verifies a frame that arrives after the
initial BAP connection is included in continuation mode. All ownership,
interruption, log-capture, and environment-restoration fixtures still pass.
The current Python suites pass 87 server tests and 65 script tests (152 total).

Fresh run `FINAL124_PROFILE1` uses the default all-chain manifest, current
01.24 schemas and separate store, `-BootSeconds 360 -PressSeconds 260
-ContinueAfterBap -KeepRpcs3`. Preflight passes and emits 837 manifest entries.
Its observed outcome and saved evidence are recorded below after completion.

## Profile1: complete setup, then an unanswered-request failure

`FINAL124_PROFILE1` reproduced stage 21 and kept its encrypted BAP session up
through the setup screens. Cross advances Screen Bounds. On Brightness the
cursor starts above the choices; moving the left stick down (`S`, 300 ms) onto
the default value 4 and pressing Cross advances. Circle dismisses this dialog
back to Screen Bounds, so it does not complete setup. Subtitles similarly needs
the cursor moved down (`S`, 200 ms) onto Language-Based (Default), then Cross.
Repeated Cross alone does not finish either selection.

Completing Subtitles at 11:10:34 led to `0x20`, UPO (`0x15`), then service 701.
The stand-in answered only 701; the client closed immediately afterward and
showed **weasel**. A direct sample records cleanup stage 25, subcurrent 174.
The final log and index preserve 971 package-open events and the error screen.
This establishes the failure boundary; it does not yet prove the server reply
is the only cause.

The 01.24 name table identifies `0x20/0x21` as user message request/response.
The registered response codecs resolve to `0x47bf9c` (user messages) and
`0x477ad8` (UPO). Both call the protobuf decoder, with descriptors at
`0x013c4080` and `0x012e6af0`. UPO contains only a repeated field and permits
an empty result list. User-msg has optional string fields 1–3 and **required
integer field 4**: its packed descriptor has type byte `0x00`, size 4 and no
default. The decoder's required-field check at `0xbfbe0c` rejects its absence.
The final stand-in reply is `20 00` (protobuf field 4 = zero), with no optional
message strings. Both replies use encrypted status 200 and the matching request
ID, before subsequent WS replies.
A socket-pair test establishes a real shello session, sends user-msg, UPO and
echo requests together, and verifies the required protobuf field, all reply IDs,
order, status and channel nonce progression.

The sampler now distinguishes raw `stage_word_30/34` from verified
`stage_state_30/34`. The latter are only interpreted for stage 19: profile
setup's `+0x30` contains `0x013e8b74`, a pointer, rather than a substate.
Samples of other stages keep raw fields and report null interpreted substates.

`FINAL124_PROFILE2` tested an empty user-msg reply. It failed immediately after
that reply, before UPO/service 701, again with weasel. Its indexed evidence
preserves 1,039 package-open events. This prompted the required-field correction;
the empty user-msg variant is not the final implementation.

`FINAL124_PROFILE3` reached Subtitles but stopped presenting frames and sending
BAP traffic after 11:24:22, before any user-msg request. Its direct sample still
reports stage 21, and a 22-thread snapshot is saved privately. The emulator
remained responsive to process inspection and memory reads, with rising syscall
counters and no captured access violation or frozen-emulation event. This is a
separate setup stall; it does not validate or reject the corrected field-4 reply.
The final log/index preserves 971 package opens. The owned stack was closed
before repeating the experiment as `FINAL124_PROFILE4`.

The deployed d1p3 executable
SHA-256 matches the
handoff: `6a79e3e758d062afd6512d4766901280b29e4f869e7f9b1b9500fb883930702f`.

## Profile4: replies accepted, character UI reached

`FINAL124_PROFILE4` used the corrected reply and completed Subtitles at
11:34:59. User-msg received its two-byte response; the client then sent UPO,
accepted its empty response, issued service 701 and continued keep-alives.
It reached **CHOOSE YOUR CLASS** and then **CUSTOMIZE YOUR APPEARANCE**.
The direct sample names `character:signin` (bootstrap resource state 2).
This validates the protocol correction against the actual 01.24 decoder.

Choosing Titan and Finish sent service 501 at 11:39:35; the stand-in answered,
saved the 01.24 request, pushed self version 2 and answered the following
select-character family fetch (206). The appearance screen was still visible
at 11:40:13, so reaching orbit and completing the creation flow are **not yet
verified**. The new store contains an actual 01.24 creation request.

The user requested the visible graphical flicker be fixed during this work.
Profile4 reproduces magenta bottom rectangles on Brightness/Subtitles and an
intermittent black rectangle in the character preview. All required color/depth
read/write and tiling settings are already enabled. A one-frame surface dump
and a short RSX trace are saved privately; investigation continues separately
from the now-passing post-setup protocol gate.

## Account active-character field migration

The creation wait has a concrete layout error: the self account (0x808018c7)
contains its roster at +0x08. 01.00 then has a u64 active-character SOID at
+0x60. Both 01.13 and 01.24 insert a u32 at +0x60 and move the SOID to +0x68.
`family_blob()` still wrote the old offset, silently truncating the SOID and
leaving the actual active field absent. It now selects the verified scalar
layout following the roster in the selected schema. Unknown populated layouts
fail explicitly. A wire round-trip regression covers both layouts, preserving
all 64 bits and the roster, and ensuring the new u32 is not populated with the
character ID. Render3 subsequently validated this live, as recorded below.

## Rendering experiments

`FINAL124_RENDER1` changed only Strict Rendering Mode to true. Four sampled
Brightness frames were clean, but the fifth (11:46:03) again has a magenta lower
rectangle (21.29% of the full image) and missing text. Strict mode is **not a
fix**. It was restored before the next trial. This run also completed setup
using the stored 01.24 character; it did not contain the active-ID correction,
which was made after its server had started.

`FINAL124_RENDER2` was stopped immediately: its trial shader-mode string did
not match this RPCS3 build's enum. It is not a valid comparison. The actual enum
is `Legacy Recompiler (single-threaded)`, confirmed in `system_config_types.cpp`.
`FINAL124_RENDER3` uses that value, baseline strict mode, and the corrected
active-character field. It reproduced the same magenta bottom rectangle on
Brightness at 11:52:12, so synchronous compilation is **not a fix** either.

Selecting the stored Titan in Render3 sent service 504 at 11:57:51. The corrected
self v2 push then let the client proceed through services 701/702 into the
prologue cinematic. The direct sample at 11:58:43 confirms stage 24,
`character:prologue`. This validates the +0x68 active-character migration live;
the prior creation wait was not a missing 501 reply.

Six short render dumps on Brightness include pink data in early passes (0xce8e0000
ARGB8 / 0xceb60000 FP16), without NaN/Inf. The rendering trial therefore also
tests CPU fallback image copies: NV0039 already performs a post-copy write
barrier, whereas NV3089's CPU image-copy path does not. A private diagnostic
build adds the barrier and write-generation notification after the copy, behind
`d1cpuwritefix.on`, to test this hypothesis. It is **not a verified fix** and is
not yet added to the tracked emulator patches.

Render3 stopped presenting frames/sending BAP traffic during the prologue,
after its last keep-alive at 12:00:22. A direct sample still reports stage 24.
Its owned stack was closed and final evidence indexed (971 package opens).
The CPU-copy trial built successfully on this PC (MSBuild exit 0); the private
binary hash is `84624c770f0d46a6b3dc39f27dd2c1f4fe7583ab5edde3523dbc5a8a1a8233ff`.
The baseline executable is retained as `rpcs3.before-cpuwrite.exe`. Fresh run
`FINAL124_CPUWRITE1` uses baseline graphics settings and the trial marker;
it reproduced severe magenta/white blocks on Screen Bounds at 12:09:04.
The trial is **not a fix**. Its owned stack was stopped, indexed (971 package
opens), and the baseline executable and NV3089 source restored. The trial marker
was renamed to an inert run-specific `.off` file.

The second PC **does** now have MSVC/MSBuild at `F:\VS\BuildTools`; the later
October 1 audit append/README supersede the original handoff's toolchain warning.
The deployed emulator has been restored to the verified main-PC d1p3 build.

`FINAL124_NOREADCOLOR1` changes only Read Color Buffers to false, retaining
Read/Write Depth, Write Color and tiling. The September 27 test log places the
first setup-menu flicker after buffer reads were enabled (before custom builds).
This tests that connection; disabling color reads is not yet a verified solution
and must not be generalized to the 01.00 mission lighting, which needed reads.

That trial had eleven clean Brightness and eleven clean Subtitles samples
(0% magenta), but Choose Your Class rendered the Titan in rainbow colours
(12:16:17–12:16:24). **Rejected: it breaks character lighting.** Color reads
were restored before further testing. No launcher default or tracked emulator
patch was changed to disable them. The user also explicitly requires both
menus and character/gameplay rendering to be correct.

`FINAL124_NOREADDEPTH1` retains color reads and disables only depth reads.
Brightness still flickered: multiple captures at 12:19:48–12:20:27 show the
same 21.3% magenta lower rectangle. Rejected; depth reads restored. A short
non-dumping trace shows the HDR buffer reloaded once each frame, rather than
an absence of reloads. `FINAL124_ACCURATEDMA1` retains all baseline buffer
settings and enables Accurate SPU DMA to test CPU/GPU transfer synchronization.

Accurate DMA also reproduced the lower magenta rectangle at 12:24:56 and was
restored. The next run, `FINAL124_LABELSTRICT1`, combines Strict Rendering Mode
with Allow Host GPU Labels, retaining all buffer reads/writes. Strict mode alone
failed earlier. RPCS3 maintainer discussion in
[issue 17413](https://github.com/RPCS3/rpcs3/issues/17413)
describes delayed texture-read semaphore releases and host labels as complementary
controls for SPU/GPU output races in another game. That is a hypothesis for this
test, not evidence that Destiny has the same cause or that this setting fixes it.

The label/strict trial exited at 12:29:25. Windows Application Error identifies
`nvoglv64.dll` and exception `0xc0000005`, rather than an RSX verification fatal.
This is not a validated synchronization fix; both settings were restored.
`FINAL124_NOWRITECOLOR1` disables only Write Color Buffers with reads retained.
It removes the textured menu background (12:34:00 and 12:34:22), then stops
presenting frames. Rejected; writes restored.

`FINAL124_GRID1` retains baseline settings and uses a private build with 33
surface-memory samples (the existing SPU generation check is retained). Build
exit 0, hash `0da465694d7ad6cbedaf33196054fdbb5cf065e5b8235dce133612261110b7d8`.
Seven of eleven Brightness samples at 12:41:12–12:41:28 have the same ~21.3%
magenta rectangle. Rejected; the source header has been restored. No failed
graphics experiment is in the tracked emulator patches.

Grid1 also selected the saved character (504 at 12:45:32) and **played the
prologue video**: 12:46:41 shows the astronaut on Mars, and the direct sample
confirms `character:prologue`. This is stronger evidence than merely entering
stage 24; it does not establish orbit yet. The video finishes at 12:48:49,
then the client requests services 105/205 and account-ID translation before
closing BAP at 12:48:54 with error **PIG**. The next protocol boundary is
therefore after the prologue, not a successful orbit transition. Type `0xab`
is `c->x not` in the 01.24 name table (`0x8c7f48`, table `0x134f3d0`), so
its presence does not justify inventing a request/reply pair.

A subsequent private diagnostic build captures guest HDR memory immediately
before `load_memory`, without a forced GPU writeback. It also traces SPU write
notifications in the HDR range. The prior render dumps forced writebacks and
could change the very cache state being investigated. Diagnostic output remains
private; this change is not a rendering fix.


`FINAL124_PRELOAD1` captures the guest HDR buffer without forcing writebacks.
Seventeen initial captures are finite RGBA16F: R/G/B are equal, alpha is zero,
and the RGB range is approximately 0.007–0.027. The existing generation check
notices the SPU writes and reloads the buffer each traced frame. Heavy per-PUT
tracing slows rendering substantially and suppresses the flicker; after tracing
is disabled, five of six Brightness samples at 13:03:31–13:03:40 are magenta.
The clean traced data is timing-dependent evidence, not proof that the source
is always correct. The diagnostic source changes have been rolled back.

The next controlled trial, `FINAL124_SNAPSHOT1`, retains every baseline buffer
read/write. It takes upload-heap snapshots for local render-target reloads,
using the existing CPU conversion path (`source_is_userptr`), behind the private
`d1snapshot.on` marker. The ordinary zero-copy upload uses a shared DMA block;
`VKDMA::dma_block::load` copies host bytes into that reusable block while GPU
reads are queued. Snapshotting tests that lifetime/race hypothesis without
removing the color data that character lighting needs. Build exit 0. This is
still a trial until menus and characters pass with tracing disabled.


Snapshot1 logged the candidate path at HDR and other local targets but still
produced magenta in seven of thirteen Brightness samples at 13:07:39–13:07:59.
Rejected; marker renamed inert, source restored. `FINAL124_ALIAS1` instead
cleared deferred GPU inheritance when the SPU write generation changed. Five
of eleven Brightness samples at 13:15:15–13:15:39 are magenta. Rejected; marker
and source restored. The source has no combination of these failed candidates.

The next private trial waits for the Vulkan queue at back-end semaphore release
before writing the CPU-visible label. The existing fallback calls `thread::sync`,
which flushes memory and pending occlusion reports, but does not unconditionally
wait for the graphics queue. `d1backendwait.on` tests full completion explicitly
without enabling imported host labels (the previous host-label/strict trial
crashed the NVIDIA driver). All baseline rendering options remain enabled.


`FINAL124_BACKENDWAIT1` built successfully and logged real queue completion at
back-end labels, with Host GPU Labels false and all buffer reads/writes true.
Brightness still flickered at 13:20:49–13:20:54. Rejected; marker and source
restored. The trial does not establish delayed back-end completion as the
cause. The next trial compares per-page SPU generations at GPU readback request
and host flush, and preserves CPU-written pages when a pending writeback is
older. These experiments remain private until a combined rendering check passes.


`FINAL124_FLUSHGUARD1` built successfully and entered the per-page guard path.
The first twenty sampled transfers report no newer SPU pages; that limited
sample is not proof that none occur later. Five of nine Brightness captures at
13:29:13–13:29:34 still have the magenta rectangle. Rejected; marker renamed,
source restored. `FINAL124_CPUREADS1` tests local color initialization only for
ranges with recorded SPU writes, including HDR/exposure buffers, while retaining
the baseline configuration. This tests whether loading untouched color ranges
causes the menu artifact without discarding the CPU lighting needed by avatars.
It is not yet a supported configuration or a verified fix.

## CPU-reads trial passes; tracked as a patch (Claude, 2026-10-01 afternoon)

Codex reached its usage limit with `FINAL124_CPUREADS1` running; Claude took over the
emulator and finished that observation before closing the owned stack (final log/index
saved privately). With `d1cpureads.on`, colour targets in video memory were loaded from guest
memory only when the per-page SPU write generation (`rsx::local_mem_write_gen`) was non-zero.
A pixel count (R, B > 150 and G < 90) reproduces the earlier failures exactly (the 21.29-21.30%
lower rectangle in about half of each baseline run's Brightness frames: Grid1 7/11,
BackendWait1, FlushGuard1 5/9). CPUREADS1 had **0 of 51** magenta frames: 4 Screen Bounds,
11 Brightness, 9 Subtitles, character select, and 7 Choose Your Class frames, with the Titan
lit normally (no rainbow, unlike NoReadColor1). The trial log shows the HDR ARGB8 target
`0xce8e0000` initialised without a load (read=0) and the FP16 `0xceb60000` / `0xcf880000`
targets SPU writes reach still loaded (read=1).

The behaviour is now `patches/rpcs3-destiny-spu-color-reads.patch`, applied on top of
`rpcs3-destiny-main-memory-blit.patch`: no marker file or logging, and it only narrows Read
Color Buffers (off stays off; targets in main memory keep the plain setting). Built on this
PC (MSBuild exit 0), deployed **only** to `RPCS3_FINAL124_SMOKE001`; the trial build is kept
there as `rpcs3.cpureads-trial.exe` and its marker renamed `FINAL124_CPUREADS1-cpureads.off`.
`RPCS3_TEST048_SETUP` (01.00) is unchanged until a 01.00 mission run confirms that mission
lighting, which needed colour reads, is unaffected.

## PIG after the prologue: the universe object moved to service 205

01.00's service 104 returns the universe object (type `0x80801941` at reply +8); without its
root `0x7fffffffffffffff` the client raised **pig** (0x18, 0xa2,
`error.investment_server_object_family_record_not_found`; 01.24 has stripped the failure name
strings). In 01.24, 104's reply is a bool and **205's reply (`0x80801827`) carries the same
universe type** (grown from 200 to 944 bytes; root, time, flags and values keep their offsets).
The client calls 205 right after the prologue's service 105 (world 230). The stand-in now picks
the universe service from the schema (`universe_service()`; 104 on 01.00, 205 on 01.24) and
leaves 01.24's 104 bool unset; `d1_build_preflight.py` checks both.

`FINAL124_UNIVERSE2` (tracked-patch emulator): 26 of 26 setup frames free of magenta; select
stored Titan -> prologue video -> 105, 205 (universe) -> acct id trans -> **client config,
104, then peer (0) and roster (2) subscriptions**, the same sequence 01.00 used before `ahp
req`. PIG is gone. The client then closed with **weasel** 0.12 s after the roster push.
Before the subscriptions it had sent `c->b mm req` (0x2a, 29-byte protobuf: 1=2, 2=4, 3=a
64-bit id, 11={timestamp, ...}), which the stand-in left unanswered; replies must follow
request order. The `mm rsp` codec (0x2b) live-read from the codec table is `0x477d9c`, whose
nanopb descriptor `0x01306900` (19-byte entries: u32 tag, type byte, sizes, default pointer)
has only optional fields: 1 int32, 2 int64, 3-6 strings (7012/1048/16/104 bytes). The stand-in
now answers with an empty `mm rsp`; `FINAL124_MM1` tests it.

## Profile setup on every launch: the settings live in the account, saved by service 701

The game makes no PS3 save-data calls at all (the emulator log records every HLE call, e.g.
46 module loads, trophy and cellGame calls; both `savedata` folders are empty). The setup
answers are server-side: service 701's request (`0x808016dd` -> `0x80801716`, 1088 bytes) is
the **account user data**, the same type as the self account `0x808018c7` field **+0x80**. Its
`+0x410/+0x10` struct `0x808017f0` (21 bytes of bools and small enums) holds the setup choices
(captured: 0=true, 3=2, 5=1, 7=3, 12=8, 13=6, 18=true, 20=3). The client sends 701 about every
30 s; the stand-in acknowledged and discarded it, so each sign-in pushed an account without
+0x80 and the client ran Screen Bounds/Brightness/Subtitles again.

The stand-in now stores the latest 701 body per account in the character store
(`account_data`, next to the character) and writes it into the self account's +0x80
(`account_data_offset()` matches the 701 request type in the schema). The local 01.24 store was
seeded from `FINAL124_UNIVERSE2`'s last 701 (backup `characters-0124.before-account-data.json`).
`FINAL124_SETTINGS1`: sign-in restored it, the self push grew to 3062 bytes, and the client
sent user msg/UPO immediately (previously only after Subtitles) and showed **character select
directly, with no setup screens**.

`FINAL124_MM1` (empty `mm rsp`): sign-in -> character select without setup -> stored Titan ->
prologue -> 105/205 -> client config, 104, acct id trans -> `mm req` answered -> 702 -> peer and
roster subscriptions. **No weasel**: the session stays up, and the client repeats `mm req` every
10 s (field 3, a 64-bit id, changes each time) and 702 every 30 s, showing the black loading
screen with the spinner. The direct sample names stage **`setup:activity_session_creation`**,
the step after the 01.00 subscriptions (01.00 asked for `ahp req` here). Next: what `mm rsp`
must carry for the session to be created (its consumer, and fields 1-6), then the activity
host route on 01.24.

After about 90 s of empty replies the client gives up: service 505, unsubscribes peer and
roster, re-fetches the select-character family and shows **"Unable to find a match for your
Fireteam" (newt)**. So `mm req` is the orbit/fireteam matchmaking poll, and an empty `mm rsp`
means "nothing found yet". Relevant 01.24 strings sit beside the descriptor: "Bungie
matchmaking interface" (`0x01306b90`, referenced from `0x48ebc8` in `0x48eb9c`),
`send_message_via_bap`, `/ah_type:%s`, `/ah_package:`, `/ah_slice_set_names:`; elsewhere
`matchmaking-host-selected`, `orbit_matchmaking`, `setup:matchmaking`.

D1P hooks on the mm codec (dismissing newt makes the client retry): the encoder (`0x477d44`,
codec vtable `0x13e9d98` slot 2) is reached from the sender **`0x4906d8`** (call at
`0x490984` -> `0x47676c`; it fills field 1 = 2 and field 2 = 4 and keeps a state at
`this+0x2c`: 1, 6, 7, 3). The decoder (`0x477d9c`, vtable `0x13e9dc0` slot 3) is called from the
generic reply path `0x47a340` (decode into a stack buffer, then the request object's vtable
slot `0x2c` at `0x47a430` consumes it; chain `0x47a5a0 0x47be80 0x928774 0x930c78`). Next:
resolve that slot-0x2c consumer (the matchmaking request object created by `0x4906d8`) and
the `mm rsp` fields it needs for "match found", then the activity host connection it leads to.
The watch list was restored afterwards.

## Matchmaking reply: field 4 is the match; then the activity host proxy (01.00's ahp req)

The poll consumer is **`0x490a30`** (the manager's reply dispatcher `0x5b5358` switches on
the request kind: 4 = poll -> `0x490a30`, 1 -> `0x4912dc`, 2/3 -> `0x48f8dc`; the manager
object, 0x59c0 bytes, vtable `0x13edc38`, is created by `0x48eb9c` and its poll object sits at
`+0x5088`). When the reply has field 4 (has-flag at struct +0x1b80) it copies that 1048-byte
field to poll `+0x34`, sets `+0x30` ("match found") and stops polling; otherwise it sets state
5 and polls again. The bootflow step `0x36da20` reads the status (vtable slot 14 -> `0x490b08`:
1 busy, 2 found, 3 idle) and returns 2 on a match; otherwise it restarts the poll (slot 16)
with the activity id from `0x34d288`.

In this nanopb build the type nibble 6 is a **submessage** (5 string, 4 bytes, 2 fixed32): the
"string defaults" are nested descriptors. Full `mm rsp` (`0x01306900`): 1 int32, 2 int64,
3 = {1: repeated {1: {84-byte blob}, 2: char[32], 3: fixed32, 5: fixed32}, 2: int},
4 = {1: {9 ints, 2 fixed32}, 2: {1: {6 ints, 1 fixed32}, 2: repeated same}}, 5 = {1: u64},
6 = {1: char[32], 2: {8 ints/fixed32}}. `mm req` (`0x01306496`): 1-6 ints (seen: 1 = 2,
2 = 4 poll, 3 = random u64), 7 = {1: {u64, ints, repeated int, {fixed32 x3, 3 x char[32]},
u64 x2}, 2, 3: {bool}}, 8 = {1: u64, 2: {that 336-byte struct, {84-byte blob}, char[33]}, 3},
9 = {u64}, 10/11 = {u32, u32} (the poll sends 11 = {1846390409, 1686739444}).

- `FINAL124_MM2`, field 4 sent as a marker string: not a valid submessage -> decode failure ->
  **weasel**; the consumer never ran.
- `FINAL124_MM3`, `--mm-rsp 4=` (present, empty submessage): the consumer ran (`0x490a30`,
  chain `0x5b53f0 0x5b3004 0x47a434`), and 0.45 s later the client sent **`ahp req`** (0x10,
  8 zero bytes): the activity host proxy request that 01.00 sent at this point. The stand-in had
  no `--ahp-reply`, answered the next request instead, and the client closed.

`bap_server.py --mm-rsp "N=VALUE,..."` sets mm rsp fields (1-2 integers, 3-6 submessages as
hex). Next, `FINAL124_MM4`: `--ahp-reply 00000000000000007f01018000003233` (proxy
127.1.1.128:12851, as in 01.00) and a proxy stand-in with `--ah-mgr-reply` (02 | host id 1 |
char[128] "d1local-host").

`FINAL124_MM4B` (`--mm-rsp 4= --ahp-reply 00000000000000007f01018000003233`, proxy stand-in on
127.1.1.128:12851 with `--ah-mgr-reply` = 02 | host id 1 | char[128] "d1local-host"; the proxy
must start after `Invoke-Final124Run`, which refuses to launch while any `bap_server` runs):
`ahp req` -> `ahp rsp`; the client opened the proxy connection (start req echo, shello field
2 = 2), sent **`c->ah mgr req`** (4111 bytes in 01.24; 3103 in 01.00) -> 137-byte `mgr rsp`,
then **`c->ah not` host 1 type 3, 238 bytes** (the host join request, 232 bytes in 01.00). With
no join response it waited about 20 s, then sent 505, unsubscribed and returned to character
select. This is the 01.00 activity join path. Next, `FINAL124_MM5`: the proxy answers with the
01.00 orbit recipe on the 01.24 schemas (`--ah-join --ah-entities 8192:12288 --ah-state
--ah-map orbit --ah-world 230`, world 230 being 01.24's service 105 request).

`FINAL124_MM5` (proxy adds `--ah-join --ah-entities 8192:12288 --ah-state --ah-map orbit
--ah-world 230` on the 01.24 schemas): the **join response was accepted** - the client sent
host types 17 (8 bytes), 37 (1311 bytes), 21 (163 and 10 bytes) and the steady type 15, the same
pattern as an established 01.00 client - then about 1 s later service 505 and **lime**
("loading activity resources"). The 01.24 bootflow names (table `0x012f7f00`) explain the
route: 24 `character:prologue` -> 28 `setup:activity_session_creation` -> 29 `setup:matchmaking`
-> 30 `setup:activity_host_setup` -> **31 `setup:prologue_intro_loading`** -> 33
`setup:activity_world_transition` -> 34 `activity:initial_slice_set_loading` -> 35
`activity:physics_join` -> 36 `activity:in_world`. A new character goes from the prologue
straight into the intro activity (the first Cosmodrome mission), not to orbit (26/27
`setup:orbit_loading`/`setup:orbit`); world 230 is that activity in 01.24's numbering. 01.00
reached orbit directly after login (test 062) and joined activities only for missions, so
the 01.00 mission recipe (`Invoke-MissionRun.ps1`: `act_cosmo_m0_highway`, checksum, 22
bubbles, zone, roster, sim player, game mode, baselines, heartbeat) is the template; its
01.00-specific values need 01.24 equivalents. `FINAL124_MM6` tries the highway map with world
230.

`FINAL124_MM6` (map `act_cosmo_m0_highway`, world 230): still **lime**, but after about 4 s
instead of 1 (the client also sent host type 14, 20 bytes, before leaving). 01.24 makes no
Demonware lobby connection at all (the lobby probe stayed silent). A read-only scan of the
emulator's memory for `*:scenario_client` registry keys (scratch helper) lists the 01.24
scenarios: `act_cosmo_m0_highway`, `act_cosmo_m0_5_hangar`, the other story missions,
`orbit`, `cine_*`, patrols and PvP maps, so the highway scenario name is unchanged.

On 01.00, the activity state's "world" had to be the **activity index** of the mission (126 for
`act_cosmo_m0_highway`), while service 105 carried the current one (125). By analogy
`FINAL124_MM7` used **world 231** (with the 01.00 checksum `0x79130968` and 22 bubbles):
past lime, the bootflow sat in 30 `setup:activity_host_setup` and the client stayed on the
proxy for 75 s, then showed **banana** ("You must have the required content in order to play
this activity"), the content/unlock gate 01.00 passed with `--self-flags 2` and
`--universe-flag-values 215=1`. `FINAL124_MM8` adds both.

`FINAL124_MM8` (MM7 plus BAP `--self-flags 2 --universe-flag-values 215=1`): past banana; the
direct sample names **31 `setup:prologue_intro_loading`**, then the client left the proxy
after host types 37/14/21 and showed **urchin** ("You do not have access to this activity"),
the 01.24 counterpart of 01.00's "World-transition requires permissions." (0x18, 0x17). On 01.00
that came from the activity's first unlock expression `NOT flag[215]` (universe flags,
overridden through the universe +0x3c list). 01.24 has more flags, so the index (or the
activity) probably differs. Next: read activity 231's unlock expression at run time (the
01.00 path was `0x40ddc4` -> `0xb4f494` -> checks `0xb51c64`, `0xb51894`, `0xb51698`; the
expression format is u32 count, u32 offset, 4-byte ops from +offset+0x14: u8 opcode, u8, i16
index; opcode 1 = flag == 2, 2 = NOT) and confirm 231 is the highway activity.

The whole cycle is now `scripts/Invoke-Final124Join.ps1` (boot, login, matchmaking, proxy, join,
bootflow samples every second, screenshot; `-Stop` closes the stack and the proxy). Its
defaults are the MM8 recipe; `-World`, `-Map`, `-BapExtra` and `-ProxyExtra` vary it.
`FINAL124_JOIN1` validated the script: the same path to 30/31 and **urchin**; `-Stop` closed the
owned stack and the proxy. (The launcher job's output pipe can stay open while the kept stack
runs, so the script waits for it at most 60 s.)

## Urchin: the intro activity's access rule is NOT flag[3390]

"World-transition requires DLC." / "...requires permissions." remain in 01.24 (`0x012fcc20`,
`0x012fcc40`), referenced from the world-transition step `0x36e92c`: DLC check `0x470384(world)`
-> failure (0x19, 0x17); permission check **`0x46db2c(obj, world)`** -> failure (0x19, 0x18),
shown as **urchin**. `0x46db2c` calls `0xc36d4c`, which fills three results:
`0xc39ecc(world, leader, level)`, `0xc39a94(obj, world)` and `0xc3987c(obj, world)`; the first
two must pass, and the third too when the player is fireteam leader (the 01.00 layout:
`0xb51c64`/`0xb51894`/`0xb51698`). Both of the last two evaluate the activity's unlock
expressions through `0xc5cfa4` -> evaluator **`0xc5cbcc`**: `{u32 count, u32 offset}`, 8-byte
entries at `+offset+0x14` (u8 opcode, operand word at +4): 1 = flag test `0xc5be34` (byte table of
0x2000 flags, true when == 2), 0xa = value lookup `0xc5c014`, 0xb = constant, 2 = NOT, 0x15 =
negate, 3..0x14 = binary operators (jump table at `0x013c3234`); `0xc58e84` checks a world list.

D1P hooks armed only once `mm req` arrived (the evaluator is hot; an earlier 120-hit budget
was spent before the join, FINAL124_JOIN2), `FINAL124_JOIN3`: the check ran with world 0xe7
(231) and both rule evaluations read the expression `{2, 0x1c}` = `01 .. 0d3e`, `02` =
**NOT flag[3390]**, with the flag table all 2 (true) by default, followed by the (0x19, 0x18)
raise from `0x36ea74`. That is 01.00's `NOT flag[215]` renumbered (01.24 has 8192 flags), so
`Invoke-Final124Join.ps1` now overrides universe flag **3390 = 1**. `FINAL124_JOIN4` tests it.

`FINAL124_JOIN4` (3390 = 1): still urchin, although a live read of the flag table showed the
override applied. The table at `0x600a1b9c` (header type `0xf04cc38e` = universe, root
`0x7fffffffffffffff`; 8192 bytes: 4331 x 2, 3690 x 0, 171 x 1) had flag 3390 = 1, so the
service 205 / sign-in universe `+0x3c` list (01.24 count width 6, entries `0x8080185c`
{i16 index, u2}) does reach it. `FINAL124_JOIN5` (hooks armed at mm req): `0xc39a94` walks the
activity's expression list and stops at the first false one: expression 1 `NOT flag[3390]`
(now true), expression 2 `{2, 0x44}` = **`NOT flag[309]`** (0x135, still 2 -> false); the
leader check `0xc3987c` evaluates the same pair. Its world-list test `0xc58e84` passes (empty
list). The level check `0xc39ecc` (activity definition +0x48 vs the player's level via
`0xc38d10`, failing on result 7) ran with leader flag 0. Script default is now `3390=1,309=1`;
`FINAL124_JOIN6` tests it.

`FINAL124_JOIN6` (3390 = 1, 309 = 1): still urchin, but now both rule checks walk all their
expressions - group 1 `NOT flag[3390]`, `NOT flag[309]`; group 2 `flag[435]`, `flag[2670]`,
`flag[5003]` (plain tests; all 2 in the live table) - and the world-list test `0xc58e84`. Every
term now evaluates true, so the remaining suspect is the level check `0xc39ecc`: it resolves
the activity definition (`0x1cb820()->vfn 0x188(world)`) and passes its `+0x48` with the
player's level (`0xc38970`: a float at universe-object +0x4004 + 4 * index, the index being a
content constant at `+0xda`) to `0xc38d10` -> the virtual `0x2a1d04` (`0xc39028`, `0xc45c718`),
failing when the result is 7. The live universe-object floats read 1.0, 1.0, 0.0001, 5.0 at
indices 0-3 and 0 after that. The universe type also has a `{i16 index, i32 value}` override
list at +264 (`0x80804ee0` -> `0x80805bc4` -> `0x80801a3c`), probably those values.
Dismissing urchin replays the prologue and retries the whole join automatically.

Hook fix: dumps based on r1 used `ppu.gpr[1]`, which is stale at a translated function entry
(the hook receives the live stack pointer separately), so `r1+OFF` dumps read the wrong place.
`rpcs3-destiny-main-memory-blit.patch` now uses the live value (two lines; `git apply --check`
against `b5a174e93` passes); rebuilt and deployed to the 01.24 copy (previous build kept as
`rpcs3.colorreads.exe`). `FINAL124_JOIN7` dumps `0x46db2c`'s three result bytes from its frame
(`sp - 0x2c..-0x2a` relative to the raising caller) when (0x19, 0x18) is raised.

## Resolving the intro activity index (FINAL124_JOIN8/9)

The corrected stack dump confirms JOIN7's three access results were `00 01 01`:
level false, both expression checks true. JOIN9 reproduces this with activity 231:
`0xc38d10` receives required level **25** (`r4 = 0x19`), while the comparison's
player-side value is **1.0**. Raising `(0x19, 0x18)` from `0x36ea74` is therefore
an expected level rejection, not evidence that another universe override is missing.

JOIN8 was launched with `-World 230` (confirmed from the proxy's command line),
not the wrapper's default 231. Its later traced transition has `01 01 01` and
raises `(0x19, 0x37)` from `0x36ebb4`: "Transition completed, but we don't have an
active activity." The screen says **lime**. The retry reused the original activity
state; restarting a proxy during a prologue is not a clean activity comparison.

A read-only inspection of the 01.24 activity table resolves the mismatch. The
content provider's vtable +0x188 points to `0x1cbb68`; its +0x1c accessor returns
the activity table through cache `0x1a2082c` (raw root +4). The table has 338
entries. Each entry is at `table + offset + 0x14 + 8 * index`; its +4 relative
pointer resolves to `entry + relative +4`. Definition +0x48 is the required level,
and +0x40 holds the scenario reference:

| Activity | Hash | Required level | Scenario reference |
| --- | --- | --- | --- |
| 229 | `0xd03ec63d` | 1 | `0x80a585cc` |
| 230 | `0x5a6705db` | 1 | `0xffffffff` (none) |
| 231 | `0x8d389a14` | 25 | `0x80a585cc` |

Activity **229** is the next intro candidate: it shares 231's scenario and accepts
a level-1 character. The earlier analogy that service 105's 230 implies mission
231 was not established for this build. No character level or client permission
code has been changed to evade the rejection. The constants accessor's +0xda
selects universe numeric index 0 for the comparison, confirmed live.

The join runner now records the proxy's executable and creation time in
`<run>-proxy-process.json`, and cleanup uses the shared ownership checker. Old
bare `<run>-proxy.pid` files do not authorize stopping a process. Proxy arguments
also use the existing Windows argument quoting helper. The script parses cleanly;
93 server tests and 65 script tests pass. The updated main-memory patch applies
cleanly against RPCS3 base `b5a174e93`; its stack diagnostics were rebuilt and used
in the live evidence above. Raw memory, logs and screenshots remain private.
