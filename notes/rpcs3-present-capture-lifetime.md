# Presentation images across native capture — 2026-10-05

The enhanced fault report is exercised in JOIN132: native 3:01.701927 reports
device loss, a successful fault query (`VkResult=0`, complete), and a GPU write
whose possible address range is `0x0–0xfff`, precision `0x1000`. This confirms the
driver's near-zero write report. It does not identify its originating GPU command.
Post-fatal guest state reads and a screenshot timeout are not gameplay acceptance.
The owned recorder/client/services/proxy/launcher are closed and XInput restored.

Source inspection finds an independent lifetime defect in presentation. Both the
linear-memory fallback and format-conversion path return a raw image pointer while
queuing its owner for retirement against the current command submission. Screenshot
or recording capture then hard-syncs that submission. Completion can destroy the
linear image, or recycle/reset the converted image, before the same flip records
its subsequent upscaling/calibration/presentation read. The asynchronous resource
manager makes the timing variable. This defect is not yet proven to originate
JOIN132's device loss or JOIN131's preview texture error.

`patches/rpcs3-present-capture-lifetime.patch` retains temporary source ownership
through the flip, independently for both eyes. Borrowed surface/cache images remain
borrowed. Linear uploads return their owner instead of scheduling early disposal.
Conversion images retain their existing recycling policy. Retirement is queued only
after the final presentation use is recorded and before its submission. Ordinary
GPU completion still performs disposal/recycling; the change does not leak images
or add a wait.

The patch also uses the capture image's format for its overlay render-pass key.
The capture attachment follows the guest presentation image and can differ from
the host swapchain; the on-screen render-pass keys retain their swapchain format.

The patch contains only this increment against the private diagnostic checkout.
Reverse application checks pass. Exact source and executable baselines remain
private; no game assets, videos or raw logs enter Git.

## Verification

`scripts/rpcs3_present_lifetime_audit.py --checkout PRIVATE_RPCS3 --out NEW_PRIVATE_DIR`
extracts the actual presentation selector, owner structure and retirement loop.
Compile its generated source as C++20. Simulated submission completion exercises
borrowed render targets, cached images, linear uploads, bitcast/typeless conversions,
allocation failure and both stereo views. Structural checks additionally require
retirement after final presentation use, ownership from the linear upload, and a
matching overlay capture render-pass format.

MSVC C++20 execution supplies **50 checks**: the pre-fix source fails eight,
covering premature destruction/recycling and retirement timing; the repaired
source passes all 50. The capture/fragment-summary Python suites also pass their
24 existing checks. These cover their own code paths, not GPU correctness.

`--baseline --checkout EXACT_FLAT_SOURCE_BACKUPS` generates the same audit for the
pre-fix source. This is CPU resource-lifetime coverage, not a Vulkan driver or pixel
correctness test. Native recording/world/menu/appearance checks remain necessary.

The full private Release build completes with its existing linker/post-build
warnings. The backed-up deployment matches SHA-256
`a700ae7d40594b3d9f71309b0db93c24e5513ec1679d3143cc778afb9757a406`.
JOIN133 starts with automatic native recording enabled and the same opt-in depth
candidate as JOIN132. It reaches a character-select screenshot and loading, then
still reports device loss at **3:13.730426**, with the same successful query and
near-zero write range. No world/menu/appearance acceptance follows. The repair
therefore does not fix the observed GPU failure by itself. Ordered cleanup is
complete, XInput is restored, and the depth marker is disabled. The local Vulkan header
and import-library setup contains no Khronos validation-layer binary; validation
layer execution must not be claimed.
