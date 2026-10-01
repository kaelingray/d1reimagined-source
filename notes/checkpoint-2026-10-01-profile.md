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
[issue 17413](https://github.com/RPCS3/rpcs3/issues/17413#issuecomment-3152820679)
describes delayed texture-read semaphore releases and host labels as complementary
controls for SPU/GPU output races in another game. That is a hypothesis for this
test, not evidence that Destiny has the same cause or that this setting fixes it.
