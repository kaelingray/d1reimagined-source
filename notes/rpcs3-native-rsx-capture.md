# Native RSX frame capture for the shading investigation

JOIN164 reproduces alternating normal and black/green outdoor shading without
an observed validation error or malformed-fragment-store message. The next
experiment records the actual RSX commands/resources behind a rendered frame;
a screenshot and clean API log alone do not identify the cause.

[rpcs3-native-rsx-capture-marker.patch](../patches/rpcs3-native-rsx-capture-marker.patch)
adds one opt-in marker to the existing Vulkan presentation diagnostic poll.
Create `d1/d1rsxcapture.on` beside the privately deployed executable only after
verifying the exact owned process and inspecting its current native image.
The marker must be successfully removed before the existing
`g_user_asked_for_frame_capture` request is set. No marker means no capture request.
The existing screenshot marker still follows its earlier behavior.

RPCS3's native capture path records the next complete frame and saves
`captures/<title>_<timestamp>_capture.rrc.gz`. **A successful capture pauses the
emulator.** Plan owned cleanup or an explicit resume; later frame timeouts must
not be mistaken for a spontaneous renderer stall. The request log is not proof
of successful saving. Require the native `Capture successful` line and a complete,
nonempty file with recorded hash and timestamps. A captured loading frame is not
evidence of the outdoor fault. Replay fidelity and reproduction require separate
inspection; CPU/SPU work outside the recorded RSX stream remains a limitation.

Capture changes timing, can consume substantial memory/disk, and is not a
performance comparison. Captures contain game data and transient private state.
Keep them outside source/data repositories and production packages. Never
substitute a replay frame for an original gameplay acceptance recording.

The isolated patch applies to the preserved pre-marker private `VKPresent.cpp`
and reproduces the resulting source after line-ending normalization. Unrelated
private renderer modifications are excluded. Patch SHA-256:
`713b35612d9ab5f531efb28af38c8bcfdb6d4a298ff31e613a09a75cb764cfe4`.
The x64 Release verification exits **0**; warnings are retained privately.
Executable SHA-256:
`714e432f39969b3d19cd585e40963e8ee8670208d9bc7ea853e4e93324ebba6b`.
JOIN165 completes its baseline and visual phases, but the bounded diagnostic
checkpoint expires before the marker is armed. **Native capture and replay:
NOT RUN in JOIN165.** JOIN166 saves one native capture after its longer inspected
checkpoint. This captures the inactivity dialog, not the outdoor fault; no new
live-game graphics repair or combined character/world acceptance is claimed.

Local RPCS3 source also provides `--no-gui --rsx-capture <private-capture-path>`
for replay. `BootRsxCapture` loads the compressed file and validates its native
version and host endianness before starting the replay thread. It clears the
game title and loads the global configuration. The ordinary CLI game's
`--config` branch does not apply to this capture branch: explicitly verify the
replay settings and restore any temporary configuration after owned cleanup.
A repeated replay image is diagnostic evidence; compare it with the original
captured scene before assigning any fault or improvement to it.

## Native capture and replay verification

JOIN166 completes its 600-second baseline and saves a **495,990-byte** native
capture at **2026-10-07 13:23:25 UTC**, with the successful native log line,
recorded SHA-256 and expected pause. The requested screenshot actually shows
the **Still there?** inactivity dialog. Earlier outdoor cinematic screenshots
do not make this captured frame an outdoor-fault reproduction. Owned cleanup
restores the tested configuration, watch list and input state without errors.

Full gzip decoding and command-prefix/resource-reference inspection pass for
this file: 277 memory mappings, 201 data blocks and 11,771 commands. The native
parser subsequently reaches replay execution. These checks do not establish
the complete game's accuracy or the capture format's general correctness.

The first native replay fails with `Capture Replay: context alloc failed`.
[rpcs3-native-rsx-replay-context.patch](../patches/rpcs3-native-rsx-replay-context.patch)
changes only capture boot initialization: reserve main memory starting at
`0x10000`, excluding the null address returned as the allocator's failure sentinel.
The rebuilt replay reaches command execution, but both inspected output frames
are black. Its fallback presents buffer 1 while the stream's final colour-A
target matches registered display buffer 0; no display flip command is captured.

[rpcs3-native-rsx-replay-display.patch](../patches/rpcs3-native-rsx-replay-display.patch)
selects a registered display buffer with matching offset and pitch when the
final colour-A target uses local framebuffer memory. It retains the previous
fallback when no match exists and leaves captured flip commands unchanged.
Both later native replay images visibly reproduce the captured inactivity dialog.
Their decoded RGB pixels exactly match the inspected original screenshot.
Core validation is verified; the bounded replay log has no validation IDs or
device loss. Both scoped patches reproduce the modified sources in isolated
Git fixtures, and their incremental Release builds exit zero. The new executable
is `cc73a62d8a079fca2ca3e995647c468485bb6095c548a95596de4e73a6c90397`.

One intervening replay attempt ends during startup without current native output.
Its old log snapshot is explicitly rejected. The private launcher now waits for
the complete PID/start-time/executable identity and rejects stale logs. All
replay attempts close their owned processes and restore the original global
configuration; capture replay clears the title, so title-only settings are insufficient.

This verifies the diagnostic route for one UI control. Capture/replay fidelity
for the corrupted outdoor scene, live character rendering, equipment changes,
traversal and the first mission remain unaccepted. Captures and diagnostic images
stay private; they are not repository assets or production-package payloads.

JOIN168 verifies the repaired short-join handoff and saves another native
capture after an inspected unobstructed outdoor cinematic view. Both replay
images show loading UI instead of that earlier view. The next captured frame
may differ from the reference; matching outdoor replay and reproduction of the
outdoor fault remain unverified. The native parser runs, core validation is
verified, and owned cleanup restores configuration. See the
[October 7 checkpoint](checkpoint-2026-10-07-renderer.md) for the actual timing
and scope. No new live-game graphics repair is claimed.
