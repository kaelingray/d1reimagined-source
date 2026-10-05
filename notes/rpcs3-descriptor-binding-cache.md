# Descriptor binding cache — 2026-10-05

The recording-enabled JOIN136 control uses the same renderer as JOIN134/135,
with privately scoped Khronos GPU-assisted validation and safe mode. The loaded
module and native feature-adjustment diagnostics establish that the layer is active.
At native **4:05**, it reports that `persistent_input_stream` expects an unsigned
texel buffer but receives a BGRA8 2D image descriptor. Later diagnostics disagree
with the tracked depth/stencil feedback-loop layout. The complete log has **65**
validation error lines; repeated-message suppression limits its coverage.

No device loss appears before controlled shutdown at native **11:47**. The final
fatal entry belongs to the GDB socket during debugger teardown, not a Vulkan fault.
The last views show the local urgent-message overlay and then loading; a world
bootflow number does not establish completed spawn or usable gameplay. Instrumentation
can suppress invalid accesses and changes timing, so this is no graphics acceptance.
All owned processes are closed, XInput and the title debug flag are restored, and
the opt-in depth/media markers are absent.

Tracing the reported descriptor mismatch exposes a separate concrete cache defect
in `vkutils/commands.cpp`. Both bind overloads compare only descriptor handles in
a single array. They omit pipeline layout and use the same cache for graphics and
compute. Vulkan keeps independent bindings per pipeline type and requires compatible
layouts; see [vkCmdBindDescriptorSets](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindDescriptorSets.html).
The source alone does not establish that this defect caused JOIN136's diagnostic.

`patches/rpcs3-descriptor-binding-cache.patch` stores handles, count and exact layout
independently for graphics and compute. Command-buffer begin resets both entries.
The no-offset overload only elides an identical full operation. The dynamic-offset
overload retains its always-bind behavior, conservatively invalidates that point's
static entry, and delegates an empty offset span to the static path. It adds no waits,
descriptor updates, shader rewrites or draw suppression.

`scripts/rpcs3_descriptor_cache_audit.py` extracts the actual header cache declarations,
reset and both binding functions into C++20 API mocks. Fourteen checks cover independent
bind points, static deduplication, layout changes and higher-set disturbance, different
static/dynamic sets, dynamic offset updates, overload sharing and reset. The exact
previous source fails **five** checks; the repaired source passes all **14**.
These are CPU binding-contract checks, not executed Vulkan validation or pixel tests.
The scoped patch reverse-applies cleanly. The full Release build exits successfully
with its existing linker/post-build warnings. Deployment matches SHA-256
`5cd68e01dca9ff6561b406f284339a9923ea7903b2f2f798414b1ff3cb26025f`;
the prior executable is backed up. Native retest is outstanding.

The automatic worker recognizes the native validation error format despite its
omitted VUIDs, saves the existing pre/post buffer and supplies a **112.64-second**
Known Broken BEFORE source package with game audio and two extracted frames.
The 500 GB NAS contains all seven files with verified payload sizes and SHA-256 hashes;
eight source packages are now delivered. Raw object handles and diagnostic context
stay private. No user clip selection, labelling or transfer is required.
