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
bounded context and important-log-entry reset. Actual NAS delivery of this new event
type remains pending until a native device-loss report returns recognized markers.
