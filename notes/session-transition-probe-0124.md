# Read-only 01.24 session-transition probe

Current checkpoint, 8 October: JOIN176–179 repeat native113→112 activation and
ordinary movement after the routine console-output repair. The remaining
streaming gate is112→24: JOIN179 receives the host's group record but remains
in phase3. A manual host request replaces the player; it is not seamless entry.
See the [mission checkpoint](checkpoint-2026-10-08-mission.md).

Historical JOIN169: the local engine requests112, but the client does not reach an idle
active slice112 before the guarded route's deadline. No walking input is sent.
Activity-host reconnects follow two requests. Dismissing the urgent dialog
alone does not resolve this run's transition.

The earlier private timeline starts after the second timeout. Its late sample
shows 112 loaded but inactive during cleanup; this does not identify the
pre-timeout cause. Its `context_handle` field reads the wrong record and must
not be used to diagnose the simulation context.

[rpcs3_final124_session.py](../scripts/rpcs3_final124_session.py) corrects that
lookup using the actual BLES01857 01.24 instructions:

- `0x33502c` selects activity slots through `*0x1abc79c`; the state is at
  `manager + 0x4e0 + slot*0x170`.
- `0x33a808` gets the context handle from `mapping + slot*8 + 0x14`, separately
  from the session record's transport handle at `+0x70`.
- `0x1d56d0` resolves it through the descriptor at `0x1a20c70`, the two packing
  branches in `0x108960`, and the datum table rooted at `0x1896d50`.
- The active context's bytes `+0x18/+0x20`, timestamps `+0x1c/+0x30` and
  incoming records `+0x5078/+0x5080` distinguish the pending machines discussed
  in [the timeout analysis](ipahp-timeout-0124.md).
- Native and last-sent engine state, slice loading/activation and the operands
  of the `0x71aba4` early-return predicate distinguish local progress from
publication of a client state change. Flag meanings are not guessed.

The spatial controller at `0x352968` writes a follow classification to
engine `+0x1d2`; `0x35149c` tests classification4 before committing the crossing.
The reader now names this `follow_player_phase_1d2`. Its old output key
`peer_count_1d2` remains for existing timeline consumers, but **is not a peer
count**. The reader also retains the resolved target index at `+0x138` and raw
selection bytes `+0x36c..+0x393`. Volume indexes are not assumed to be global
slice-set IDs, and a received group record alone is not a completed crossing.

The reader uses the existing exact PID/start-time/executable ownership check
and verified guest allocation. It performs no writes, pauses or debugger calls.
Descriptor, handle and pointer rechecks detect some concurrent changes;
**samples remain non-atomic**. Lookup errors are retained rather than replaced
with a fabricated valid context. Timelines and raw guest observations stay
private.

Run against an already initialized owned activity, before requesting transition:

```powershell
python scripts/rpcs3_final124_session.py --manifest <private-run-manifest> --output <new-private-timeline.jsonl> --seconds 440 --interval 0.25
python -m unittest discover -s scripts -p test_rpcs3_final124_session.py -v
```

All five focused tests pass: hand-calculated native packing vectors for both
branches, valid index zero versus null, generation change detection, rejected
uninitialized descriptors, and a slot-map/record decoy that reproduces the old
lookup error while checking pending fields. These are reader checks, not mission
acceptance. The owned native launcher must start sampling automatically so tool
or inspection delays cannot miss the transition.

## JOIN171 native verification — 7 October 2026

The unchanged `cc73a62d` Titan/highway fixture completes its 60-second baseline
and eight-minute checkpoint. The owned launcher starts the reader automatically;
**1,372 non-atomic samples** span **17:34:17–17:41:37 UTC**. No context lookup
errors or descriptor/handle/pointer recheck failures occur. Cross dismisses the
actually inspected urgent dialog; the next screenshot shows outdoor cinematic
scenery, without proving player control.

At **17:35:45 UTC** the host requests 112. The context's second pending machine
reaches **2**, while the first remains **0**. From **17:35:47 UTC**, 1,105 samples
show 112 loaded with no active/loading slice, engine phase6 and target112.
The engine's last-sent record contains that target; a separate native host
snapshot also finds matching pending/sent records containing 112. These copies
do not prove wire delivery: the proxy receives five initial type21 messages and
**none after the request**, so it publishes no group1 switch. No idle112 sample,
session37/38 or live socket reset is observed in this bounded run.

Two separate clock reads at **17:41:47/49 UTC** both return tick5848, while the
pending start is2567. The timeout counter is returned by `0x12fc8c -> 0x2b9de8`
from `(*0x1ab6710)+0xc`; elapsed UTC alone cannot establish the local timeout.
The reader now includes that counter for subsequent timelines; this addition
was made after JOIN171's running reader loaded, and its fixture check passes.

A later screenshot request fails its twenty-second deadline. Inspected video
samples show loading and contacting-server text. Fourteen indexed originals
retain matching hashes; ten unindexed failures remain preserved. The last two
indexed raw clips fully decode, **30.81/31.04 seconds**, 1280×720 with audio.
Reported average/base rates **29.98/30 and28/30** are recording properties,
not gameplay performance. No originals are repaired or edited.

Cleanup at **17:42:18 UTC** closes the owned stack and restores the original
title/watch, XInput and inactive controls without errors. Reader verification
passes in its sampled scope; **mission arrival fails and graphics remain
unaccepted**. Next: trace the subtype21 queue/serializer result and correlate
the encoded bytes with proxy receipt. The existing transport wrapper's success
return is insufficient to prove delivery; see
[the earlier transport investigation](rpcs3-world-heartbeat.md).

Next acceptance requires native idle112, visible control and appropriate mission
progress, followed by sustained character/equipment and outdoor rendering
together. Do not force guest state, bypass resurrection with a direct112 start,
or treat a successful reader as a gameplay repair.

## JOIN172 filtered native trace — 7 October 2026

The diagnostic build `97518199` adds optional function-entry predicates; the
Titan/highway fixture and protocol remain unchanged. It completes the same
60-second baseline and eight-minute checkpoint. The automatic reader records
**1,175 non-atomic samples, 18:12:06–18:19:26 UTC**, without pointer/context
lookup or recheck failures. The filter trace records **12 calls each** at the
wrapper, queue and serializer entries, within their 80-hit budgets. Configured
r5/r6 predicates select subtype 21 and its initialized 13-byte header.

Delivery is delayed even before the transition: a 22-byte state call recorded
around 18:12:00 reaches the proxy at **18:14:58 UTC**, when the host starts the
mission clock. This observation does not identify the cause of the delay.
At **18:17:19 UTC** the host requests 112. Pending 20 reaches 2 and the engine's
last-sent target becomes 112. A later subtype 21 attempt reaches the serializer,
but the original connection receives **no type 21 after that request** and no
group 1 switch is published. Reconnect initialization messages are counted
separately, rather than mistaken for delivery on the original connection.

There are 128 samples of loaded/inactive 112 from **18:17:29–18:18:18 UTC**.
The native clock advances throughout this phase; it resets when the activity
session is recreated. At 18:18:18 the sampled session leaves 6 for 3, and the
proxy sees a live socket reset at **18:18:21 UTC**. The pending start tick 7621
and the later sampled tick 9377 represent less than 120 seconds at 30Hz. Transient
session 37/38 is not observed; the reconnect must not be labeled the earlier
local 120-second timeout without stronger evidence. The separate 18:20:10 reset
occurs during owned shutdown.

Observed vtables resolve the queue rejection gate to `0x47b7ec` (returns true
when object+0x8c is zero) and connection readiness to `0x8c6d4c` (true only for
connection+0x10f8 equal 8). A non-atomic **18:17:10 UTC** snapshot has values 1/8.
It establishes neither the gate values at an earlier send nor the actual
serializer/queue return. Function-entry watches cannot observe arbitrary
post-call instructions. Next: capture these deterministic gate inputs at the
relevant call, follow serialization/enqueue outcome and correlate the target
payload with receipt before changing transport or session behavior.

Inspected native screenshots show black/loading and the urgent dialog. Two
owned Cross presses are recorded; the later image shows loading. After reconnect,
inspected recording samples show outdoor cinematic scenery behind a reappeared
urgent dialog. They do not prove control or unobstructed combined graphics.
Twenty-two indexed originals retain matching hashes; three unindexed failures
are preserved. The last two original clips fully decode, **30.3/18.29 seconds**,
1280×720 with audio; reported average/base rates 14.1/30 and 12.41/30 describe the
recordings, not gameplay performance. Originals remain untouched.

Cleanup at **18:20:11 UTC** closes all exactly owned processes and restores
title/watch, XInput and inactive controls with no errors. The pre-stop native
log has zero validation IDs, device loss and fatal entries, with core validation
verified. No idle 112, traversal, combat, mission completion or graphics repair
is accepted. A subsequent parser guard rejects unsupported live r0/r2 predicates;
44 CPU checks and its Release build pass. That final guard build has not yet had
a full native join; JOIN172's supported r5/r6 evidence belongs to `97518199`.

## JOIN173 actual gates and return values — 7 October 2026

The `c5bc7f57` diagnostic build adds `if=lr==VALUE` caller selection without
changing the compiled callback ABI. Its actual-source CPU fixture passes **52
checks** and the incremental Release build succeeds. The same Titan/highway
fixture, transport protocol and rendering settings are retained. No RSX capture
is armed. The automatic reader records **1,052 non-atomic samples,
19:00:32–19:07:51 UTC**, with zero context lookup, pointer or descriptor/handle
recheck failures.

Seventeen subtype21 attempts reach the wrapper, queue, serializer and direct
branch. Eight distinct header/caller combinations emit the configured unique
gate/return watches. The queue rejection gate observes object+0x8c equal1 and
readiness observes inner+0x10f8 equal8, including for the new 34-byte payload.
Actual shared helper entries `0xd98a58` and `0xd98a54` emit **r3=1** before guest
register restoration. The 34-byte header is observed at native elapsed
**0:06:56.414828/415140**, with LR `0x47a208/0x476840` respectively. No selected
asynchronous branch or failure cleanup hit occurs. Unique header/LR keys can
suppress repeated calls, so these results must not be described as every return.

This establishes that the earlier readiness/outer-return hypotheses do not
explain this observed attempt. It still does not establish delivery:
`0x479fa8` sets its success value unconditionally after calling direct enqueue
`0x4797b8`. That routine can fail a later allocation or leave a serialized record
for transport consumption. The kind17 mapper's eligibility method resolves to
`0xa6220`, which returns constant1; its alternative-path method returns constant0.
Neither should be blamed without stronger evidence.

The original connection receives five initial subtype21 messages. At
**19:01:57.209 UTC** the host requests112. No subsequent subtype21 reaches that
connection before the live reset at **19:04:04.398 UTC**, and no group1 switch is
published. Pending20 reaches2; pending18 remains0. There are **244** samples with
112 loaded and inactive from **19:02:08.225–19:03:56.197 UTC** in this first
attempt. The native clock advances from2487 at the first pending sample, with
pending start2485, to6047 before the sampled session is removed at19:03:57.
Session37/38 is not sampled, so the approximately120-second wall-clock interval
is not alone proof of the exact teardown path. Reconnect initialization and its
second19:06:39 request are separate from the original connection. Across both
attempts, **432** samples show loaded/inactive112 and none shows idle active112.
The reset at19:08:35.870 belongs to owned shutdown.

An initial private analysis mistakenly treated the loaded-word array as a list
of indices and reported zero loaded112 samples. It is a bitset; the corrected
counts above use word3/bit16 and supersede that invalid result. Original timeline
bytes are preserved.

Inspected native images show outdoor cinematic behind the urgent dialog, then
black/loading after an owned Cross. Final inspected recording samples show
loading/contacting-server UI. Eighteen indexed untouched originals hash-match;
seven unindexed failures remain preserved. The last two originals fully decode,
**39.24/20.82 seconds**, 1280×720 with audio. Reported average/base rates
29.97/30 and28.04/30 describe the recordings, not gameplay FPS.

Cleanup at **19:08:36 UTC** closes the exact client, probe, services and proxy;
original title/watch, XInput and inactive controls are restored with no errors.
Core validation is verified before stop, and the bounded log contains zero
validation IDs, device loss or fatal entries. Guardian control, combat, mission
completion and combined character/equipment/world rendering remain unaccepted.

Next: correlate the initialized serialized record with actual queue allocation
and consumer entry, then with host receipt. Preserve real resurrection-to112
flow; change transport only when the observed failure supports the change.

## JOIN174 direct allocation and transport consumption — 7 October 2026

The same `c5bc7f57` build and Titan/highway fixture replace outer-return watches
with caller-filtered direct allocation and consumer watches. The automatic
reader records **1,017 non-atomic samples, 19:32:45–19:40:04 UTC**, with no lookup
or pointer/context recheck failures. Sixteen subtype21 attempts reach direct
enqueue, its record allocator and the later mapper hook after successful body
allocation. No selected serializer-failure cleanup hit occurs.

For each of two requested transitions, the 34-byte payload becomes a **47-byte
serialized record**. The allocator-side hook and transport-side mapper entry
observe an identical initialized 24-byte record prefix: sequence, channel17,
serialized-buffer pointer/length and auxiliary pointer/length. The first matched
pair occurs at native elapsed **0:06:10.710615/711634**; the second at
**0:11:05.481820/482848**. These actual entries establish successful allocation
and arrival at the consumer for these records. They do not prove a completed
packet header, forwarding to the socket or host receipt.

The consumer caller is `0x47a6d4`, entered from `0x8c5bdc`. Runtime tables and
actual instructions show that **kind17 selects codec index8**, at vtable
`0x13e9f78`, rather than index17. Its serializer is `0x47837c`; packet descriptor
selection returns3. The earlier private codec17 lookup was the wrong selection
and must not support this path's diagnosis. The actual packet factory3 resolves
header encoding to `0x5b2610 -> 0x8c8220`, reached after consumer output by
`0x8c5bdc`. Next: trace that encoder's result and the forwarding queue before
changing the supported protocol. A late non-atomic snapshot shows both the direct
queue and inner forwarding queue empty; it does not identify the earlier outcome.

The original connection receives four initial subtype21 messages, then none
after its **19:34:45.825 UTC** request. It resets at **19:36:49.242 UTC** and
reconnect initialization belongs to a different connection. The first attempt
has **273** loaded/inactive112 samples from19:34:49.572 to19:36:46.018. Across both
attempts there are326; no idle112 is sampled. Pending20 remains2 during the failed
transition, pending18 stays0, and session37/38 is not sampled. No group1 switch,
Guardian movement, combat or completed mission is accepted.

The inspected19:34:37 native image shows the urgent dialog over outdoor scenery;
19:35:04 shows black/loading after a Cross. An invocation mistake runs the input
helper's default150ms Cross before an attempted function call fails: it is a
script. Only this owned RPCS3 client is live at the surrounding checks, but no
400ms press or helper-level identity validation is claimed for that invocation.
No walking input is sent. Subsequent calls must invoke the script with explicit
PID and creation-time/executable identity.

Twenty indexed originals hash-match and three unindexed failures remain
preserved. The last two originals fully decode **30.33/16.50 seconds**, 1280×720
with audio; reported average/base24.58/30 and19.07/30 are recording properties.
Inspected samples show loading UI, not controlled gameplay. Cleanup finishes
**19:40:51 UTC**, exact client/services/proxy/probe all closed, original title/watch
and XInput restored, controls inactive, errors empty. Core validation is verified
before stop and the bounded log has zero validation IDs/device loss/fatal entries.
Combined character/equipment/outdoor/mission rendering remains unaccepted.

## JOIN175/176 routine console backpressure — 7 October 2026

JOIN175 preserves the original backend and `c5bc7f57` renderer. Its transition
state arrives in separate payloads rather than the previous34-byte combined
delta, so fixed47-byte allocation/consumer/header watches do not emit. That is
a filter-scope limitation, not evidence of a failed allocator or header encoder.
All16 subtype21 attempts reach the broad wrapper/direct/allocator watches.
The1,156 session samples contain548 loaded/inactive112 observations and no
idle112, with no lookup/recheck failures. Owned cleanup finishes20:14:43UTC.

A non-atomic transport snapshot at20:08:37UTC has an empty direct/forward queue,
inner state8 and packet counter3403; the original proxy is still receiving frames
near1300. This suggests a receive backlog rather than a lost queue record.
The original event/console timestamps independently bound2,023 unique frame
prints:32 exceed one second, with93.919seconds of total elapsed time and a worst
interval of6.579seconds. The interval is immediately before the flushed stdout
print through the structured log timestamp after it; it includes scheduling
delay and does not identify the OS cause. A broader event-to-log measure also
includes event IO and must not be treated as pure stdout time.

`server/bap_server.py` now suppresses only the two console lines for the exact
encrypted AH notification with flag1, subtype15, length1, reserved0 and body00.
The host ID is still parsed and every packet still follows normal processing.
Full encrypted/plain binary files and structured frame/AH events remain retained.
Other messages remain visible; `--verbose-wire` restores the previous verbosity.
No ACK, schema, teleport state, guest memory or renderer behavior is changed.

Two actual encrypted socket tests send64 routine packets, a nonroutine type15
and a type21. They verify contiguous frame/event capture, all original binary
files, host reaction execution and the verbosity override. The eight session
tests pass. The broader command, run from `server`, is:

```powershell
python -m unittest test_ah_session test_bap_server test_activity_join test_bap_teleport_delta test_run_log
```

Its first88-test run fails one existing150ms teleport test while the native test
is active. That12-test module subsequently passes in isolation, and the full
88-test command passes on repeat in10.577seconds after native closure. The
initial failure remains recorded; no timing expectation was weakened.

JOIN176 tests the same Titan/highway fixture and renderer with the changed
backend. The broad producer hook observes all10 subtype21 allocations. Its
consumer/header watches select37-byte bodies; the observed combined transition
uses47bytes, so those watches do not support a header-return claim. Actual
host receipt provides stronger delivery evidence:

- **20:24:41.288UTC:** host requests112.
- **20:24:42.299UTC:** host decodes current−1, target112, state2, peer byte2.
- **20:24:42.695UTC:** fresh state4 arrives; host publishes finishing state3.
- **20:24:43.906UTC:** current112 arrives; the host clears the group switch.
- **20:24:48.663–20:30:39.950UTC:**1,397 samples show active112 with no loading
  and phase0. Pending20 clears, group0 is112 and group1 is empty.

All1,747 non-atomic samples retain session6 with no lookup/recheck failures.
The original connection persists until owned shutdown, with5,873 routine
notifications fully captured. Cleanup finishes20:31:22UTC, all exact processes
closed, original title/watch and XInput restored, controls inactive, errors empty.
Core validation is verified before stop; the bounded log contains no validation
IDs, device loss or fatal entries. No RSX capture is armed.

Twenty-three indexed originals hash-match; one unindexed failure is retained.
The last two originals fully decode30.32/10.69seconds,1280×720 with audio. Their
reported average/base9.12/30 and9.14/30 describe recordings, not gameplay FPS.
Inspected samples show the urgent dialog over outdoor scenery and a level-up
prompt. No additional Cross or walking input occurs after the baseline checkpoint.
This verifies the repaired transition in one native run; it does not verify
visible Guardian control, character proportions, sustained graphics, combat or
mission completion. Next: repeat with owned dialog dismissal and inspected
Guardian movement, then resume the real mission route and combined fidelity gate.
