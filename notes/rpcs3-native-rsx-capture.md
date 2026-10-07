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
**Native capture and replay: NOT RUN.** This is diagnostic work, with no new
graphics repair or combined character/world acceptance claimed.
