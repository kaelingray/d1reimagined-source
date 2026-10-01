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
The current Python suites pass 85 server tests and 65 script tests (150 total).

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
