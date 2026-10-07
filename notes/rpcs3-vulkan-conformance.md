# Vulkan queue, query and resource-lifetime conformance

Core validation on the 01.24 fixture identifies concrete host/API errors before
the previously observed GPU loss. These repairs address specific contract errors;
they do not establish correct character rendering or mission playability.

## Baseline and reproduction

The private renderer remains based on RPCS3
`b5a174e933a083389e2f851005e474ccb3e676d9` with the earlier project patches and
unrelated local work retained. Each increment is exported against its own exact
pre-change source snapshot, with LF-normalized reproduction checked in an isolated
Git repository. Do not replace the private tree wholesale or imply that the public
project commit alone identifies the entire tested native binary.

JOIN150 uses the retained-readback-context executable, SHA-256
`090cf2ff657ed6a745e734b0a07bbd29262533626dc2f5f655f3f08e380d7808`.
The regular Titan starter fixture, mission proxy and original PPU watches are
preserved. Only the title's debug-output switch and process-local Khronos layer
configuration change. Vulkan validation runtime 1.4.341.1 is confirmed loaded in
the exactly owned client process; native callbacks confirm its execution.
GPU-assisted shader instrumentation is not requested by this fixture.

Observed errors:

| First native time | Observation | Meaning |
| --- | --- | --- |
| 2:59.183349 | Query begins outside a pass and ends inside it | Query scope violates `VUID-vkCmdEndQuery-None-07007` |
| 3:01.091090 | Presentation accesses a queue concurrently with submission | Missing host serialization of the same queue |
| 3:09.410591 | SPU-side Vulkan device loss | Stability failure remains; queried write range is `0x0–0xfff` |

The callback's duplicate limit suppresses later repetitions; reported counts are
not total occurrence counts. No useful finalized video segment is accepted for
JOIN150. Failed capture is recorded privately rather than represented as footage.

## Queue increment

[rpcs3-queue-synchronization.patch](../patches/rpcs3-queue-synchronization.patch)
routes WSI presentation through the same exclusive lock as queue submission.
Both operations release the lock through RAII, including exception paths.
Presentation returns its original result so the caller still handles outdated
swapchains and other recoverable outcomes. Submission checks `VkResult` after
releasing the lock, then marks its fence flushed only on success. The existing
device-loss handler can therefore run without retaining the queue lock.

This follows the Vulkan host synchronization requirements for
[presentation](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueuePresentKHR.html)
and the error/resource guarantees for
[submission](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit.html).
It adds no GPU wait, shader rewrite or synthetic acknowledgement.

The actual C++ submission method, presentation method and shared wrapper are
extracted by [rpcs3_queue_synchronization_audit.py](../scripts/rpcs3_queue_synchronization_audit.py).
CPU mocks check complete batches, zero/nonzero semaphore counts, exact error
propagation, failed-fence state, a subsequent successful submission, concurrent
lock acquisition and exception cleanup. All **48 checks pass**; the exact prior
source fails **15**. Mock execution is not native GPU acceptance.

The complete MSVC x64 Release build passes. The executable SHA-256 is
`d415a3455871a84642047b852479372271b2b6ae6a5dd48f339b506bb01e862f`.
JOIN151 compares it against JOIN150 with the same validation fixture. Its final
snapshot reaches **11:55.154100** with **zero concurrent queue callbacks and zero
device-loss entries**. This accepts host queue serialization for this bounded
scenario; it does not establish long-session stability. The fixture completes and
the exactly owned stack closes, the original watch/title hashes match, XInput is
restored and diagnostic controls are inactive.

Overall conformance still fails: ten reported query-end callbacks, render-pass
query closure violations, four descriptor-pool resets while command buffers are
pending, and depth/stencil feedback-layout mismatches. Further callback repetitions
may be suppressed. The final inspected screenshot shows the urgent-message dialog
over the world and cannot verify full-body rendering or mission progression.

## Query increment

[rpcs3-query-scope.patch](../patches/rpcs3-query-scope.patch) enables the existing
outside-renderpass query path for every driver independently of strict rendering
mode. Query begin/end then use matching scope, including draws that change
attachments or split across command-buffer submissions. It preserves query
indices, results and logical-task state instead of discarding visibility tests.
The [Vulkan query contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdEndQuery.html)
requires a query ended inside a pass to have begun in that same subpass.

[rpcs3_query_scope_audit.py](../scripts/rpcs3_query_scope_audit.py) extracts the full
device-policy initializer, draw-side query-start block and renderer end/cleanup
methods. Mock pass scopes cover four driver policies with strict rendering enabled
and disabled, attachment changes, empty queries, rearmed queries and wrong-task
rejection. All **29 checks pass**; the exact prior source fails **7**. The complete
MSVC x64 Release build passes; executable SHA-256 is
`e74967ad48b6fc852f58c9fc48cfeda82c3666784c94281c01c57d4c4c2ded50`.
JOIN152 reaches **14:52.575180** without query-scope callbacks, render-pass query
closure violations, concurrent queue callbacks or device loss. Three descriptor
reset callbacks and feedback-layout errors remain. This accepts matching query
scopes in the bounded fixture; overall conformance still fails. The exactly owned
stack closes and original watch/title bytes and XInput are restored.
Extra pass closures may affect performance;
no performance improvement is claimed or measured.

To generate either audit, pass `--source <private-renderer-root>` and a new
`--out <audit-directory>`, then compile the generated C++ using C++20 and run it.
For the baseline comparison, pass the matching saved pre-change source root.

## GC prefix increment

[rpcs3-gc-scope.patch](../patches/rpcs3-gc-scope.patch) keeps completed-prefix
selection and detachment inside one exclusive map lock. Previously, selection
used a reader lock followed by an upgrade. RPCS3's `reader_lock::upgrade` explicitly
allows observed state to change. A saved end iterator can then include newly
appended future scopes, or the current scope can acquire a later event ID, before
the selected range is removed. Resource callbacks still run outside the lock.

[rpcs3_gc_scope_audit.py](../scripts/rpcs3_gc_scope_audit.py) extracts the actual
prefix-detachment method and injects permitted writer activity at the lock
boundary. All **14 CPU checks pass**; the exact prior source fails **3** when a
writer appends a pending scope or delays an existing scope. Normal completed
prefixes, later completion and callback lock lifetime are also checked. This
validates atomic selection against the documented lock contract; it does not
establish that this interleaving caused JOIN151's descriptor-pool callback.
The complete MSVC x64 Release build passes; executable SHA-256 is
`4fa0276a3703ceebd53029ccaec746ebf4c10ac3ed9dc56648f7b384e83fe047`.
JOIN153 completes the same 500-second post-match fixture. The final trace reaches
**2:41:22.713699**, including an extended unattended period after the fixture ended,
with zero concurrent queue/query callbacks and zero device-loss entries. Two
descriptor-reset callbacks and feedback/draw-layout errors remain. The GC change
therefore does **not** resolve the observed descriptor-reset failure. The completed
fixture's inspected screenshot is a black loading view, so no character or mission
rendering acceptance follows from the longer process lifetime. The exactly owned
stack closes and original watch/title bytes, XInput and inactive controls are
restored. JOIN152's binary excludes this subsequent source edit. Lock-contention
performance is not measured.

## Capture reliability

During JOIN152 the automatic worker exits after a status-file lock timeout, followed
by the same error while reporting it. The corrected worker preserves pending status
fields, serializes updates from its two threads and retries telemetry writes without
terminating the observer. All **73 capture tests pass**. A live six-second status
lock is held from 14:50:03.986900 to 14:50:09.987170 UTC while idle, then from
14:54:11.386902 to 14:54:17.387362 UTC during JOIN153 recording. The exactly owned
worker and recorder survive; fresh recording status is verified at 14:59:35 UTC
with no deferred update or monitor/processing error. Coverage from approximately
14:41 to 14:48 UTC is missing, not retrospectively repaired or claimed. Outbox
payload/state handling remains durable and unchanged.

The native recorder later exits separately: JOIN153 has 63 automatically decoded
and hash-verified segments between 14:52:24 and 15:24:24 UTC, plus one initial
unusable segment. The final recorder status is FAILED before stack cleanup, while
the observer remains alive. Native trailer-error messages also appear even for
segments that pass decoding. Recording beyond the last verified segment is
unproved; the held-lock test accepts process survival and telemetry recovery,
not uninterrupted encoder operation. Cleanup leaves the observer idle with no
monitor/processing error. No new milestone or duplicate NAS package is fabricated
for this repeated fixture.

## Fence completion increment

[rpcs3-fence-completion.patch](../patches/rpcs3-fence-completion.patch) retains
command-buffer pending state, event identity and its fence when a wait returns
anything other than success. It previously reset the fence and notified collection
even on timeout. Reuse and frame cleanup now retry an expired bounded wait through
the existing indefinite completion path, and propagate real failures before
resetting the command buffer or releasing the frame's heap/resources. DMA restart
also confirms completion and defers its submitted fence through the existing
collector if waiting throws.

The [fence wait contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitForFences.html)
distinguishes timeout from a signaled fence. Vulkan forbids resetting a
[pending command buffer](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetCommandBuffer.html)
or a [fence still associated with pending work](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetFences.html).
The change repairs these lifetime contracts; the descriptor-reset cause remains
unproved. A stalled GPU can keep the completion path waiting longer than the
previous bounded wait. Timing/performance acceptance remains unmeasured.

[rpcs3_fence_completion_audit.py](../scripts/rpcs3_fence_completion_audit.py)
extracts the full command-buffer chunk and frame-cleanup method, plus the actual
DMA submission/wait/restart block. Mock result sequences cover timeout, five
failure statuses, later successful completion, idle/reused buffers, frame heap
retention and DMA fence ownership. All **71 checks pass**; the exact pre-change
source fails **51**. This proves the injected completion/error contracts, not
native GPU fidelity. The three-file patch reproduces normalized changed sources.
The full MSVC x64 Release build passes; executable SHA-256 is
`051e8eb5d7e53ab322918111c51c707f00a0a05e5ed09354372edc59b44026c8`.
JOIN154 repeats the matching native fixture, with automatic final snapshot and
cleanup. It completes the same 500-second post-match scenario and reaches
**11:22.537209**, with zero queue/query/device-loss entries. Two descriptor-reset
callbacks and feedback/draw-layout errors remain; no unsuccessful command-buffer
wait is logged. The timeout path therefore has CPU acceptance and no demonstrated
native trigger in this comparison. Overall conformance still fails. The inspected
final screenshot is the urgent-message dialog over a dark world, not full-body or
mission proof. Automatic cleanup closes the exactly owned stack, restores original
watch/title bytes and XInput, and disables diagnostic controls without errors.
The observer remains alive, but this run's recorder exits before producing a
segment receipt: later source coverage is unproved and requires separate repair.

## Descriptor lifetime diagnostics

[rpcs3-descriptor-timeline.patch](../patches/rpcs3-descriptor-timeline.patch)
adds an opt-in trace linking command-buffer event tags, physical queue submission
order, confirmed fence completion, disposable scope assignment, pool retirement
and pool reset. The private `d1/d1descriptor` marker is polled at most every
500 ms. A shared process budget permits 262,144 records and one exhaustion notice;
toggling the marker cannot reset that budget. Runtime handles remain private.
Submission ordinals are allocated under the existing queue guard. Cleanup and
submission behavior are unchanged when tracing is disabled. The cleanup header
also gains an include guard for its additional include path.

[rpcs3_descriptor_timeline_audit.py](../scripts/rpcs3_descriptor_timeline_audit.py)
extracts the actual gate, ordinal helper and disposable class. All **17 CPU checks
pass**, covering concurrent reservations, throttled polling, budget exhaustion,
reactivation and unchanged resource ownership. Queue, GC and fence audits still
pass **48, 14 and 71 checks** with tracing disabled. These mocks do not establish
native submission order or repair rendering. The six-file patch reproduces the
normalized changed sources. The complete MSVC x64 Release build passes after
repairing the missing include guard; executable SHA-256 is
`5d94e44f96007beb94a9b542b7283056533b553e863de573c8857636098e0735`.
JOIN155 completes the same fixture at **11:21.772973**, with no queue, query,
descriptor-reset, layout or device-loss entries. The trace reaches its budget at
**3:43.081868**, before the earlier comparisons' first descriptor failures.
Diagnostic timing differs, and no failing pool is correlated in this run, so this
does not establish a fix or its cause. The final inspected image has the urgent
message over visibly distorted outdoor scenery. Full-body/mission acceptance is
still absent. Automatic cleanup closes the owned stack, restores original
watch/title bytes and XInput, and disables both diagnostic controls. Next: compare
with tracing disabled and reserve a later trace window for the failing scene.
Trace overhead is unmeasured; coverage beyond exhaustion is unavailable.

The recorder now logs timestamped start/stop requests, shortcut results, source
identification and finalization through stderr. Diagnostic logging does not add
values to PowerShell's success stream or alter the eight-second source deadline.
It records an unidentified-source cleanup toggle explicitly. These phases help
distinguish delayed native output from a failed shortcut; they do not themselves
repair missing footage or prove a closed, decoded recording. Input diagnostics
also record eligible windows and the actual key-down/key-up post results. A
zero-count enumeration selects exactly one game window during JOIN155; it posts
no input. The recorder's missing-source failure does not reproduce in this run.
At 18:39 UTC, **20 of 22** receipts have full-decode and matching retained/source
hash acceptance. One initial segment is unusable; another has a permission error
and remains eligible for automatic retry. These intervals are unverified, and
rotation gaps remain explicit. The observer stays alive and returns to idle.

The accepted final native source reports **1280×720**, an approximately **9 fps**
average and a **30 fps** base rate. New automatic buffer/package metadata retains
these inspected properties rather than assuming the configured recorder rate.
The rounded banner values are not a gameplay performance measurement. The full
capture suite passes **77 tests**, including average/base-rate separation,
fractional rates, missing input properties and failed decoding. Actual native
validation reproduces these fields with full decode PASS and audio present.
The tested worker is deployed while no game is active; existing ready packages
remain immutable.

## Attachment layout cache/binding candidate

JOIN156 repeats the prior executable with descriptor tracing off. It completes the
fixture and later input phases at 11:56.782564 with no validation IDs or device loss.
Final inspected views remain black loading screens, including one contacting-servers
banner. Posted inputs and an in-world reader label supply no body/menu acceptance.
Exact-owner cleanup, original settings, XInput and inactive controls pass.

The [attachment-layout increment](../patches/rpcs3-attachment-layout.patch) refreshes
the render-pass key at use and restores sampleable depth layout immediately before
emulated depth-compare binding. Actual source control flow exposes late transitions
after the cached key was chosen. All 31 CPU checks pass against 15 baseline failures;
scoped one-file reproduction and full Release build pass. The candidate is built but
not deployed or run natively. JOIN156 does not validate it. See the
[handoff](HANDOFF-2026-10-06.md) for hashes, scope, sources and next acceptance.

## Remaining acceptance

JOIN157 now tests the deployed attachment-layout increment in the same fixture,
with tracing off and the core layer verified. Its final log reaches 11:48.007191
with zero validation IDs, queue/query/descriptor-reset callbacks or device loss.
Because JOIN156 already has a clean bounded comparison, this does not isolate
the increment as the native cause of improvement. Inspected outdoor images
continue to alternate between normal shading and black/green corruption, and
startup dialogs obstruct the character check. Malformed fragment-program entries
need further investigation. Exact-owner cleanup and configuration restoration pass.

Require no concurrent queue-use or query-scope callbacks during the same native
scenario before accepting those repairs. Record other validation failures
separately and investigate their exact resource lifetimes/layouts. A short run
without device loss does not establish crash resolution. Stable full-body,
equipment, outdoor and mission views are still required to accept the user's
flicker/distortion fix; mission handoff and combat remain separate open work.

## Late October 6 guarded-depth comparison and October 7 cache audit

JOIN158 uses executable `55efc907` with the optional guarded depth candidate enabled.
The 500-second post-match fixture completes, but core validation reports two
`VUID-vkResetDescriptorPool-descriptorPool-00313` callbacks at 3:39.734023 and
3:40.279613. The final native log reaches **12:02.875802** without queue/query or
device-loss entries. The lifetime trace begins after the failures and reaches
its cap; it cannot correlate their submissions. Inspected outdoor views still
alternate normal shading and black/green corruption, and the message-of-the-day
obstructs character/equipment phases. This is failed bounded conformance and
unaccepted visual/gameplay behavior.

The separate [allocation cache increment](rpcs3-descriptor-allocation-cache.md)
drops unissued old-pool handles at rollover. All 20 actual-source CPU checks pass
against five failures in the original allocator; all 23 retirement checks also
pass with current diagnostic mocks. The native failure's cause remains unproved:
inspected graphics tables allocate one layout per owned pool. Follow the
[October 7 checkpoint](checkpoint-2026-10-07-renderer.md) for native build/run status.

JOIN159 repeats `55efc907` with the guarded depth candidate enabled, an earlier
bounded trace and longer menu inputs. It completes the fixture and nine phases
with the core layer verified, reaching **14:06.829486** without validation IDs or
queue/query/descriptor-reset/device-loss entries. Inputs clear the dialogs and
reach character/inventory/world views. The sampled Titan and final world shading
look normal, but initial loading, level/light zero and weapon spinners remain.
The trace covers the earlier failure window and reaches its cap without a failure.
That does not establish repair of intermittent faults or sustained combined
fidelity. JOIN160 tests the deployed allocator increment with descriptor tracing
off and verified core validation. It completes its 500-second fixture and eleven
phases, reaching **12:37.486264** with one reset-00313 callback at **3:17.903782**,
zero queue/query callbacks and zero device loss. Bounded conformance fails;
the cache increment has not resolved the native fault. Inspected screenshots and
sampled native footage show loading, dialogs and a Rhinoceros reconnect warning,
so character/world acceptance is not established. Exact details are in the
October 7 checkpoint.

The next comparison uses a [bounded memory trace](rpcs3-descriptor-lifetime-ring.md)
that freezes on a real descriptor-reset callback. It has 20 passing ring checks,
10 passing decoder tests, 20 allocation and 23 retirement regression checks, and
a passing Release build. JOIN161 native validation and causal correlation remain
pending. It changes diagnostics, not Vulkan resource lifetime or visual acceptance.
