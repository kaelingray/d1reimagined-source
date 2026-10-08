# 01.24 revival completion and scene evidence

Updated 8 October 2026. Revival remains **PARTIAL**: restoring the sequence's
lifetime does not establish visible Ghost/Guardian, complete shots, dialogue,
animation or fades. See the [reference comparison](guardian-rises-reference-review.md).

## The premature-stop defect

The previous mission stopped the third-person performance after 70 seconds of
host time and the first-person performance after 45 seconds. JOIN184's read-only
scene trace reaches third-person indices 0–4 before the scripted handover;
indices 5–8 are not observed before that stop. JOIN185 removes the stop timers
in a private diagnostic recipe and observes all nine indices before the natural
end. Its first-person sequence is then started manually, so JOIN185 cannot
accept an automatic opening.

The native incident envelope is activity-host type 18, schema `0x8080097d`.
Native `0x580640` posts incident 2120 when a cinematic starts and incident 734
when it ends or is stopped. The measured 734 body is 53 bytes, with the cinematic
object ID at bit 303. JOIN185 observes start/end pairs for group `0x992bb471`,
kind 6, indices 1 and 2. The third-person interval is 102.494 seconds and the
first-person interval 55.700 seconds in that run. These are observations, not
replacement timers or universal durations.

The mission now matches only a decoded incident chain `(734,)`, the measured
body layout and the specified cinematic ID. A started incident, unrelated ID,
ID bits elsewhere in the body or malformed envelope cannot advance it.
On the third-person end, it clears that cinematic's playing state and starts
first person. On the first-person end, it clears playback and queues the existing
teleport protocol to slice 112 after five seconds. The original first start at
20 seconds is retained. There is no timer fallback if the end incident is absent.

Incident 734 alone cannot prove natural completion because forced stops also
emit it. Accept that distinction with a separate scene trace and inspected media.

## JOIN186: automatic handover, limited visual evidence

JOIN186 uses the unchanged C5 renderer and a private 90-second first start to
allow dialog inspection; the production recipe retains 20 seconds. There is no
manual cinematic start, manual host teleport, route input, guest memory write
or RSX capture in this run. Its external control file stays empty.

952 non-atomic performance samples, 09:03:45.616–09:11:45.316 UTC, have no reader
errors or failed rechecks. Third-person indices 0 through 8 are observed in
order, ending at 09:08:45.820; first person starts automatically and reaches its
single scene, ending at 09:09:41.979. The end incidents drive the host reactions.
The ordinary teleport reaches 112 at 09:09:50.985 in the host log. 1,500 separate
session samples include 468 idle112 samples, retain session state 6 and have no
pointer-recheck failures.

The urgent dialog covers both sequences and is dismissed only at 09:10:31 UTC.
This run therefore **does not accept the cinematic's visible shots or cast**.
The inspected 09:10:34 native frame shows outdoor scenery and the Wall diamond,
with blank objective banner text and a Level Up coach overlay.

The pre-stop log has no VUID, device-loss or fatal-severity entry through
00:09:20.943809; this does not accept rendering. Nineteen original recording
segments are inventoried, eighteen indexed; full decode of all nineteen is not
claimed. Eleven exact process identities are independently closed after stop;
title/global/watch hashes match originals, XInput and shared RPCN are preserved,
and cleanup errors/active diagnostic markers are empty.

## Read-only performance probe

`scripts/rpcs3_final124_performance.py` follows the native resource registry,
manager records and verified `0x42c060` dispatch before reading performance
instances, authored scene definitions and player replacement bindings. The
runtime asset tags are `0x80cb8d65` for the nine-scene third-person performance
and `0x80cb8d5b` for the single-scene first-person performance. Earlier private
labels that reversed these two tags are incorrect.

The native player replacement pairs are `(0,16)` and `(0,17)`. These are not an
inventory of every embedded cinematic actor. Cache presence and a local machine
ID do not establish a physical or visible Ghost. Phase -1 alone also does not
mean playback has stopped: third-person scene indices advance in that phase.

The tool performs no writes, debugger pauses or input. Resource/manager/instance
rechecks expose some concurrent changes but do not make a sample atomic or prove
actor lifetime. Output contains private runtime/content data and belongs outside
Git. Use an exact run process manifest, an exclusive private output path and a
bounded interval; stop or verify the reader closed before stopping its client.

All 183 server checks pass after promotion, including malformed incident and
shipped-mission timer regression checks. Fourteen synthetic probe checks and
nine incident-summary checks pass. JOIN185's actual four start/end packets also
pass offline object-specific matcher verification. Visible Ghost/Guardian,
unobstructed shots/audio, objective text/Nav Mode, later markers, stable Breach
progression and combined graphics remain open.
