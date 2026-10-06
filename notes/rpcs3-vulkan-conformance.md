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

## Remaining acceptance

Require no concurrent queue-use or query-scope callbacks during the same native
scenario before accepting those repairs. Record other validation failures
separately and investigate their exact resource lifetimes/layouts. A short run
without device loss does not establish crash resolution. Stable full-body,
equipment, outdoor and mission views are still required to accept the user's
flicker/distortion fix; mission handoff and combat remain separate open work.
