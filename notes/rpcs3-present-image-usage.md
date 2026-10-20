# Temporary-image usage and capture tracing — 2026-10-05

The presentation [ownership repair](rpcs3-present-capture-lifetime.md) exposes a
second concrete issue in `create_temporary_subresource_storage`. Pool lookup checks
the caller's requested usage mask, but a new allocation always uses only TRANSFER_DST
and SAMPLED. Presentation conversion requests TRANSFER_SRC as well, so a new image
omits that requested use. Conversely, lookup can reuse an image lacking the helper's
mandatory upload/view uses when they were omitted from the caller's mask.

`patches/rpcs3-present-image-usage.patch` combines the requested mask with TRANSFER_DST
and SAMPLED once, then uses that exact mask for both lookup and allocation. It retains
compatible reuse and allocation-failure behavior. This corrects a resource contract;
it does not establish the cause of the observed near-zero GPU write.

The patch adds an opt-in `d1/d1media.on` probe: at most 200 lines per activation,
one per two seconds, with marker polling every half second. It records whether each
eye is borrowed, converted, linear or absent; source format/usage/layout, capture and
overlay state, source/buffer dimensions, swapchain format and submission/completion
IDs. It does not dump pixels, change draws or force additional GPU waits. Keep the
marker absent outside an owned diagnostic run.

## Verification

`scripts/rpcs3_texture_usage_audit.py --source PRIVATE_VKTextureCache.cpp --out NEW_PRIVATE_DIR`
extracts the actual allocation helper and generates C++20 fixtures. Simulated pool and
allocation records test zero, sampled, transfer-source, storage and attachment requests,
rejection of insufficient reuse, retention of compatible reuse and failed allocation
cleanup. The repaired source passes **21 checks**; the exact previous source fails
**13**. These are resource-contract checks, not Vulkan execution or pixel acceptance.

The scoped patch reverse-applies cleanly to the private diagnostic renderer. A full
Release build completes with its existing linker/post-build warnings, and deployment
matches SHA-256 `f3073f434db73bae0b63233cca630038df3a4b5317184784a15c9da5bf90e5f5`.
The prior executable and exact source are backed up privately.

JOIN134 starts with automatic recording and the same opt-in depth candidate. The
per-game config overrides the temporary global debug-output setting, so the Khronos
validation layer is **not loaded** in this run; it is an ordinary recording-enabled
usage comparison. Its current probe shows borrowed BGRA8 sources and matching BGRA8
capture/swapchain formats. It still reports device loss at **3:01.334012**, with a
successful query and the same `0x0–0xfff` GPU write range. The final sampled sources
are borrowed, so those samples do not exercise the repaired temporary-source paths.
The resource corrections therefore do not resolve this observed fault. Ordered
cleanup is complete, the global debug setting is restored, and XInput/markers reset.

A private, scoped validation component is available for the next test. The pinned
LunarG SDK 1.4.341.1 download matches its official SHA-256 and has a valid LunarG
signature. Only the validation DLL and its manifest are extracted; no SDK installation,
driver replacement, registry change or system environment change is performed. Force
the layer only in the owned launch's environment and verify both its loaded module
and actual diagnostics before claiming validation coverage. Raw logs remain private.

JOIN135 corrects the per-game override and verifies the loaded validation module in
the exact owned process. Core validation logs no VUID error before device loss at
**3:19.887520**. The fault query succeeds and reports a GPU write in
`0xd06922000–0xd06922fff`, so the failing address is not consistently near zero.
Final sampled sources are still borrowed BGRA8 with matching source/capture formats.
The client freezes before world acceptance; later screenshot timeouts establish no
input recovery. Ordered cleanup restores the per-game debug setting and markers.

JOIN136 uses the same executable and fixture with scoped GPU-assisted validation and
safe mode enabled. Instrumentation can suppress invalid accesses and changes timing,
so extended runtime alone cannot establish a graphics fix. The [Khronos shader
instrumentation documentation](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/main/docs/gpu_av_shader_instrumentation.md)
describes that diagnostic behavior. Automatic capture remains enabled.
It supplies identifier-free native descriptor and depth-layout errors, automatically
delivered as a Known Broken BEFORE source package. See the subsequent
[binding-cache investigation](rpcs3-descriptor-binding-cache.md) for the exact scope
of the CPU repair and remaining native acceptance.
