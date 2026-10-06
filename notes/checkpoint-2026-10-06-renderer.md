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

JOIN139 runs with the same privately scoped core/GPU-assisted/safe-mode fixture as
JOIN137, native recording enabled and the guarded depth candidate. The validation
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
preserve older queued intents with their original build. All **42 capture tests** pass.

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

JOIN140 is the same deployed renderer with the validation DLL absent, title debug
output restored and normal capture active. Its native result follows below. Complete
mission/body/menu visual acceptance, Ghost illumination and combat remain unverified.
