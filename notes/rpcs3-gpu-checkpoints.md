# Bounded GPU command checkpoints

Repeated native device loss prevents a reliable mission-handoff trace. JOIN144 and
JOIN146 fail before the teleport; a fault-query write hint in `0x0–0xfff` does not
identify the command, shader or resource responsible. This increment adds opt-in
command checkpoints to the private Vulkan renderer so the next failure can narrow
that investigation. It is diagnostic work; flicker, character appearance and mission
acceptance remain open.

## Implementation and controls

`patches/rpcs3-gpu-checkpoints.patch` is a scoped seven-file increment against the
saved private renderer baseline based on `b5a174e9`, with earlier project patches
already present. It does not export unrelated private changes or game assets.
Create the `d1checkpoint` marker in RPCS3's **`d1/` control directory** before device
creation to request `VK_NV_device_diagnostic_checkpoints`, when supported. With the
marker absent the extension is not requested and no checkpoint commands are emitted.
The active path logs `D1CKP checkpoint instrumentation active=1`. Both device procedure
addresses must be available; unsupported or incomplete support disables diagnostics.
The NVIDIA diagnostics-config extension is not enabled.

Markers bracket command-buffer recording, graphics draw batches and compute
dispatches. They carry an opaque sequence value. A bounded 65,536-entry registry
copies operation names and records the command-buffer/pipeline handles. A reused
registry slot is attributed only when the full sequence matches; unknown or expired
markers remain explicit. No returned marker pointer is dereferenced. These diagnostics
do not add GPU waits or change shaders, geometry or rendering settings. The CPU
registry work can affect instrumented timing; its performance has not been benchmarked.

Only the existing `VK_ERROR_DEVICE_LOST` branch queries checkpoint data. Graphics
and transfer queues are queried separately, excluding null or duplicate handles.
Each result is initialized with its Vulkan structure type, capped at 64 records and
limited to the number actually returned. The query's lost-device precondition and
count contract follow [vkGetQueueCheckpointDataNV](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetQueueCheckpointDataNV.html).
Checkpoint recording follows [vkCmdSetCheckpointNV](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetCheckpointNV.html).

Returned stage/operation pairs are command-region hints. They do not prove that a
particular draw, shader or resource caused the fault. Copies and other commands are
covered by command-buffer boundaries rather than individually bracketed markers.

## Verification

`scripts/rpcs3_gpu_checkpoint_audit.py` extracts the actual registry, query and command
helper into C++ API fixtures. All **19 checks pass**, including disabled/missing
procedures, copied names, independent pipeline bindings, expired/unknown markers,
queue deduplication, structure initialization, bounded and shrinking returned counts,
and invalid positive/negative bind points. These CPU fixtures do not execute a GPU.

The full private Release build succeeds. The deployed executable matches SHA-256
`ede019ada713501d3f6cf757ba9df35d1622291fdc38edbf8f8c99d8bb6a1e14`;
the previous executable is preserved. Applying the public patch to an isolated copy
of the saved baseline reproduces all seven compiled files after newline normalization.

JOIN147 verifies native extension activation and `active=1` with normal game/audio
recording enabled. The **13:26** pre-stop snapshot has zero fatal/device-loss entries
and no checkpoint-report header. It therefore validates native activation, not a
lost-device query or returned markers. Longer survival cannot establish causality or
a graphics repair. Two loaded-but-inactive slice-112 handoffs still fail; their
[publication trace](rpcs3-world-heartbeat.md) is preserved separately.

Ordered cleanup closes all owned clients, services, proxy, observer and launcher.
The original PPU watch bytes and title configuration match their prelaunch hashes;
XInput is restored and the checkpoint marker is renamed inactive. Shared RPCN is
preserved. The normal capture worker remains live, idle and drained with the NAS
connected and no current processing/monitor errors. A transient processing permission
error cleared automatically; its originating operation is not established.

## Automatic capture

The capture worker recognizes bounded checkpoint-report continuations across log
reads. It normalizes only known operation names, queue indices and nonzero stage
masks; raw sequence values, command-buffer/pipeline handles and host paths stay in
private diagnostics. Unknown, expired and invalid records cannot establish a command
region. The next native log entry or 132 continuation lines ends the report context.

A newly observed command-region hint creates an **Experimental** development event,
with automatic footage packaging and the existing verified NAS transfer path. It
cannot classify the renderer as fixed. The full capture suite has **63 passing tests**;
five added checks cover privacy, deduplication, split reports, invalid/expired records,
bounded context and important-log-entry reset. Actual NAS delivery is verified below.

## JOIN148: returned native markers and automatic delivery

JOIN148 activates the deployed probe and reports RSX device loss at **3:50.961315**,
before the mission handoff. The fault query again reports a write in **0x0–0xfff**.
Queue 0 returns two records, at stages `0x1` and `0x2000`, both attributed to the same
`command-buffer.end` marker with cached graphics pipeline zero. Queue 1 returns none.
This accepts the real lost-device query, registry attribution and stage reporting.
It does not identify the faulting operation: the end marker selected only the graphics
binding, so it omitted any compute binding and DMA work within that recording.

The automatic worker detects the recognized hints, creates an **Experimental /
FEATURE_PARTIAL** event, and delivers its package without a user capture or transfer
action. All seven files and six payload sizes/SHA-256 hashes are independently checked
on the configured 500 GB share. Eleven ready packages are now present. The package
contains **60.42 seconds** of native game/audio context; its final segment ends **3.45
seconds before** event detection. Truncated coverage is explicit. Inspected source
frames show preparing/message UI, not a playable Guardian, a repaired character or
the crash itself. No raw GPU handles or private native logs enter the package.

Ordered cleanup closes the exactly owned stack and launcher. Original watch bytes,
title configuration and XInput are preserved, experimental controls are inactive,
shared RPCN remains running and capture returns healthy, idle and drained.

## Readback context increment

`patches/rpcs3-gpu-readback-context.patch` extends the saved checkpoint-enabled
private baseline in five files. Every marker copies both pipeline bindings, recording
counts for graphics batches, compute dispatches and texture readbacks, plus the latest
readback's source image, mapped buffer, offset, length, capacity, dimensions, pitch,
format and transformation flags. Buffer begin clears this context. Readbacks are
bracketed at entry, mapping and completion; entry explicitly has no mapped destination.
Only copied numbers are retained, with no resource ownership or pointer dereference.
The existing bounded registry and lost-device-only query remain in place. The larger
record increases registry memory; instrumented timing remains unbenchmarked.

These details remain private diagnostics. Automatic capture accepts the three new
operation labels but retains only stage/operation hints. Pipeline/resource handles,
offsets and capacities are excluded from content evidence. Thirty checks of actual
C++ registry/query/command-helper code pass, including retained compute bindings at
buffer end, independent snapshots, recording reset and disabled/closed-buffer paths.
All **70 capture tests** pass, including DMA-context privacy and importer importance
labels. The full Release build succeeds; scoped patch application reproduces all five
compiled files after newline normalization. The deployed executable matches SHA-256
`090cf2ff657ed6a745e734b0a07bbd29262533626dc2f5f655f3f08e380d7808`;
the checkpoint-only executable is preserved.

JOIN149 activates the increment and reports SPU-side device loss at **3:54.671530**,
before the handoff, while waiting for a fence. Both queue-0 stages again return the
same buffer-end marker; queue 1 returns none. The new context records no graphics
batch, **two compute dispatches and one readback**, with a nonzero compute binding.
The latest readback is 384 × 624, pitch 8192, format 97 and byte-swap flag 2. Its
destination offset 5120 plus length 5,106,688 is within capacity 5,177,344. This
validates native context retention and does not establish an out-of-bounds DMA
destination or identify the faulting shader/descriptor. The latest completed region
need not be the originating fault. No shader, geometry or rendering policy is changed.

Exactly owned test processes close, XInput is restored, watch bytes and title
configuration hashes match, controls are inactive and shared RPCN remains running.
The capture worker is healthy, idle and drained. No duplicate event is generated for
the unchanged observed checkpoint capability. Next: correlate the retained compute
pipeline with its kernel and descriptor resources, then obtain stable character,
equipment, outdoor and mission views after a demonstrated repair.
