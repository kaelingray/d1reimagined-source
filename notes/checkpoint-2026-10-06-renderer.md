# Renderer continuation — 2026-10-06

The [idle pool repair](rpcs3-descriptor-pool-retirement.md) closes a concrete lifetime
gap: frame age alone could destroy descriptor pools with GPU work or owner-capturing
reset callbacks outstanding. The actual-source CPU comparison fails 13 of 23 checks
before the repair and passes all 23 afterward. The full Release build exits zero,
with existing conversion/linker/post-build warnings. Deployed SHA-256 is
`b5eba7b0da6984c03bd739b37b1661d2e38275502f50db3a845a0dd411813660`;
the previous executable and exact scoped source baseline are retained privately.

JOIN138 terminates during launcher ownership verification before a usable renderer
test. Its manifest records failure while the retained launch handle reports no exit
at that instant. The stack's owned processes are subsequently verified absent.
This does not establish a graphics regression; the reason for that transient identity
failure remains unproven. The uniquely named JOIN139 retry launches successfully.

JOIN139 runs with privately scoped core/GPU-assisted/safe-mode validation and
native recording enabled. A marker-location error leaves the guarded depth candidate
and media probe **disabled**: the markers were written beside the executable, but
the source polls the `d1/` control subdirectory. This differs from JOIN137 and
invalidates a matching-fixture comparison. The validation
DLL is verified in the exact client process. Boot includes cache compilation, so
absolute native runtimes are not comparable as a performance claim. At **4:52** it
repeats the persistent-stream dimension/component report, including an image-sampling
instruction paired with unsigned texel-buffer resource metadata. Idle reclamation
continues, reaching **148** released tables before the final sampled interval.

The process then exits abruptly. Windows Application Error event 1000 matches the
owned PID, **exact creation FILETIME**, and executable path. It identifies exception
`0xc0000005` in `VkLayer_khronos_validation.dll`, offset `0x320514`, at **06:46:26 UTC**.
The native log has no fatal/device-loss record for this exit. This is a host validation
DLL access violation, not an observed Vulkan device-loss result. The final native log,
structured Windows event, module receipt and launcher diagnostics are retained privately.
Owned services/proxy/launcher are closed before the next control.

## Shader metadata ambiguity in the validator

The [SDK-series post-processing source](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/vulkan-sdk-1.4.341.0/layers/gpuav/instrumentation/post_process_descriptor_indexing.cpp)
gets the pipeline associated with an instrumented shader ID, then searches every
stage's resource-variable map for a module-local variable ID and accepts the first
match. That lookup does not restrict the match to the originating shader module.
Different stages can therefore supply different resources under the same numeric ID.

`scripts/vulkan_validation_stage_lookup_audit.py` extracts that actual lookup branch
into C++ mocks. Four checks reproduce two cross-stage counterexamples and confirm
distinct-ID and unmatched-ID controls. A fragment image access can receive vertex
buffer metadata, including its set/type; reversing stage order produces the converse.
This makes the reported persistent-stream mismatch an **unreliable originating-defect
diagnosis** until the actual shader and descriptors are correlated. The counterexample
does not establish that every native report is false, identify the validation DLL
crash site, or explain the independently observed uninstrumented GPU write faults.
The installed layer is not patched, and no game shader is rewritten to match its report.

## Automatic capture and durability

Known native exception statuses now trigger a separate failure observation through
the retained process handle, including exits with no native fatal line. Ordinary exits
and forced-termination codes are excluded; intentional stop markers still take precedence.
Failed exit queries do not imply that the process ended.

Run-associated capability changes atomically retain their capture intents in project
state. Queue publication merges by event ID before acknowledging the intent. A busy
queue, failed acknowledgement or restart can no longer discard a newly deduplicated
failure. Selective acknowledgement preserves newer observations, and build changes
preserve older queued intents with their original build. All **44 capture tests** pass.
The last two exercise a real retained Windows process handle with a controlled
exception-status exit, and a failed exit query that must preserve the live reader
and recorder. The controlled status fixture does not cause or diagnose a native crash.

The identity-matched JOIN139 Windows record is fed back to that automatic pipeline,
which selects existing source segments, packages them and verifies NAS delivery.
The ninth package contains **60.42 seconds** of game/audio context and two frames,
labelled **Known Broken / BEFORE**, with the validation-DLL exception explicitly
separated from GPU-device loss. All seven NAS files and six payload sizes/SHA-256
hashes are independently verified. Original media remains local.

The last complete source segment ends **28.4 seconds before** the Windows exception;
the active segment did not provide accepted finished media. These are pre-crash
preparing-screen views, not footage of the crash itself or a repaired character.
Metadata records the actual window and truncated post-event coverage. Recovering
the final native segment after an abrupt host crash remains capture follow-up work.

## JOIN140: default candidate-off native control

JOIN140 uses the same deployed renderer with the validation DLL absent, title debug
output restored and normal capture active. The depth candidate and media probe are
also **off**, due to the same misplaced markers. No `D1MEDIA` lines appear; the
marker-path source audit establishes why. Retain that configuration difference when
comparing earlier candidate-enabled runs. The misplaced inactive markers are renamed
after cleanup; neither directory contains an active experimental marker.

The last native memory sample is **17:09**: **2,123** program additions, **9,699**
descriptor-set first initializations and **9,625** idle-table releases. There are zero
device-loss or malformed-program entries. The only final fatal line is the GDB socket
at **17:08** during ordered debugger teardown; it is not a GPU failure. The pre-stop
15:36 snapshot has zero fatal entries. Thus recording and reclamation operate through
this bounded run, without establishing the repair as the cause of its longer survival.

Visible frames show the revival/world background and local urgent/MOTD dialogs,
interspersed with loading. Dismissing a dialog does not yield accepted body or equipment
views in this run. The client returns to engine state 0 and displays the outdoor scene,
then re-enters state 6. A read-only snapshot at **07:04:50 UTC** finds current slice -1,
target 112, 112 loaded but no active/loading slice, and an empty engine world-state
group copy. This is consistent with the known slice-transition handshake problem;
an in-world bootflow label does not establish a settled Guardian.

All exact owned client/service/proxy/launcher identities are closed. XInput and the
byte-identical title configuration are restored; normal capture is healthy and idle,
with nine delivered packages and no queued work or errors. The deployed renderer
remains the tested build. Complete mission/body/menu visual acceptance, Ghost
illumination and combat remain unverified.

Next: repeat the recording-enabled comparison with the candidate marker in the actual
`d1/` control directory and verify its activation before interpreting results. Then
correlate the loaded-but-inactive slice transition with the host group/version exchange,
and obtain stable outdoor, character/equipment and indoor appearance samples. Keep
validator ambiguity, CPU lifetime contracts and native pixel acceptance distinct.

## JOIN141–143: device loss and slice-handoff isolation

JOIN141 places controls in the correct `d1/` directory and verifies the media probe
with native lines, including `recording=1`. At **4:09.139203** it reports device loss
on an SPU thread, with a write hint in **0x0–0xfff** and six instruction-pointer hints.
The last memory sample has 550 descriptor-set initializations and 348 idle releases.
The 400 binding diagnostics contain no synthetic bind: the candidate is armed,
but its execution is not established by this trace. Do not call this a completed
candidate-on pixel comparison. Ordered cleanup closes all owned processes and
restores XInput with no active experimental controls.

JOIN142 uses default controls with recording enabled. Its read-only timeline captures
slice 112 loaded but inactive after the host request, followed by a dropped connection
and rejoin. It has no GPU fault before ordered shutdown. The
[world-heartbeat repair](rpcs3-world-heartbeat.md) changes the seconds-only default
to resend the owned state with unchanged version; numeric diagnostic subtypes remain
explicit. All affected CPU checks pass. JOIN143 preserves the request but repeats
the loaded/inactive handoff twice, followed by live connection drops about two minutes
after each request. A bounded non-unique trace proves that the request and engine
ticks still run in the stalled state. No target-112 report reaches the host.

The 16:12 pre-stop log has zero fatal/device-loss/malformed entries, 2,125 program
additions, 9,560 descriptor-set initializations and 9,487 idle releases. All owned
processes are closed, the original PPU watch file and XInput are restored, and title
configuration hashes match. Capture is healthy and idle with nine NAS packages.
The renderer executable remains unchanged and the depth candidate stays disabled.
JOIN144 observes the verified 01.24 publication chain at compiled function entries
during the initial join, then reports RSX device loss at 3:27.678568, before the
mission teleport. The write hint again spans 0x0–0xfff. It supplies no failed-handoff
trace. Its exactly owned stack is closed and original controls/input are restored.

The [recording-fragment repair](rpcs3-recording-fragments.md) retains complete MOV/MP4
fragments after an interrupted writer and recovers only validated prefixes in private
buffer copies. Twelve actual-block C++ checks and all 58 capture checks pass; killed
synthetic writers recover 120 clean video frames and audio, while the previous
format recovers none. The native Release build succeeds. JOIN145 deliberately
interrupts the owned recorder before its trailer and recovers 688 clean video frames,
23.83 seconds with audio, while preserving the unfinished original. Import verification,
timestamps and retry deduplication are fixed. A corrected 84.25-second package with
three distinct source segments is automatically delivered; seven files and six payload
hashes/sizes are verified. The earlier duplicate-segment package is preserved as
superseded. Ten ready packages are now delivered and the normal worker is healthy,
idle and drained. GPU and character/mission acceptance remain open.

JOIN146 repeats bounded function-entry publication tracing on the rebuilt recorder
executable. It reaches active 113, verifies an open activity-context publication gate
and the corresponding client report, then loses the device at **3:38.689770** on an
SPU thread. The queried fault is again a write in **0x0–0xfff**, without an instruction
pointer. The fault precedes the mission handoff and cannot establish its missing link.
All owned test processes close, the original watch hash and title configuration match
after cleanup, and XInput is restored. The automatic capture worker is healthy and
idle with ten ready NAS packages and no pending work. Return to the GPU fault before
attempting another handoff acceptance; keep context handles distinct from slice indices.

## JOIN147: checkpoint activation and stalled-handoff publication

The [GPU checkpoint increment](rpcs3-gpu-checkpoints.md) brackets command buffers,
graphics batches and compute dispatches with bounded opaque markers. It is opt-in,
queries only after device loss and retains no raw GPU handles in automatic content
metadata. Nineteen checks of the actual C++ code and all **63 capture tests** pass;
the private Release build and scoped patch reproduction also pass.

JOIN147 verifies the extension and active instrumentation while normal game/audio
recording runs. The **13:26** pre-stop snapshot contains no fatal/device-loss entry
or checkpoint report. Native query execution remains unverified; this is not evidence
of fixed flicker or character distortion. Both slice-112 handoffs still stall.
The [publication trace](rpcs3-world-heartbeat.md) now follows changed client records
through the sender and encoder, while the proxy receives no corresponding target
report. Subtype-specific transport tracing is the next missing observation.

All owned test processes close. The original watch file and title configuration
hashes match, XInput is restored, experimental controls are inactive and shared RPCN
remains running. Capture is healthy, idle and drained with the 500GB NAS connected.
Ten prior ready packages remain delivered; this run creates no duplicate milestone
for unchanged capabilities. The checkpoint event's native/NAS acceptance awaits a
recognized returned marker. Continue transport diagnosis and GPU/visual acceptance.
