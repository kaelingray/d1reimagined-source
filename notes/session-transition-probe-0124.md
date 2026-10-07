# Read-only 01.24 session-transition probe

The opening mission's 113-to-112 transition still fails native acceptance. In
JOIN169 the local engine requests 112, but the client does not reach an idle
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
