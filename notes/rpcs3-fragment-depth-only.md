# Experimental zero-local depth fragment — 2026-10-05

The [binding investigation](rpcs3-fragment-zero-binding.md) now has a guarded
implementation candidate. It is **disabled by default** and is not an accepted
Vulkan device-loss or combined menu/world fix.

## Implementation

`patches/rpcs3-fragment-depth-only-candidate.patch` applies on top of the private
diagnostic renderer with the binding probe and leading-NOP literal correction.
`git apply --reverse --check` succeeds against the tested checkout. It contains
only this increment; unrelated private RPCS3 modifications remain outside it.

The `d1/d1depthfp.on` marker enables the candidate on the RSX thread. The predicate
requires local address zero, no vertex varyings, no writes to any color attachment,
no shader depth export or kill declaration, and disabled alpha testing, alpha to
coverage, polygon stipple and enabled-texture alpha kill. Ordinary bindings retain
their existing source. Eligible draws use a stable, 16-byte RSX END NOP through the
normal parser/cache/decompiler path. Vertex processing, raster draws and fixed
depth/stencil/occlusion processing remain active; the draw is not skipped.

Eligibility is checked on every pipeline analysis. A change in color or coverage
state can return to ordinary fragment handling even without a new shader binding.
Selecting either path marks both fragment source and state dirty. SPU source-write
generation checks are retained for ordinary guest programs and excluded for the
canonical host-owned instruction.

This is a hypothesis about Destiny's zero-local draws. The captured state supports
a depth/stencil interpretation; it does not establish original RSX hardware behavior
or universal validity for other titles. Savestate transitions and other games are
not accepted. Keep the marker absent outside controlled comparisons.

## Validation and native limits

- The generated C++ audit uses the actual predicate, actual parser and actual shader
  control declarations. MSVC C++20 compilation passes **20 depth fixtures and 12
  parser fixtures**, including every color attachment and fragment side-effect
  exclusion, same-binding state changes and canonical instruction metadata.
- The full private Release build completes. Its deployed SHA-256 matches the build:
  `ab9b5f4a21c16bdf4bcac642e9676f0bce8a65f18311d54d55cbe8a595d74054`.
  The previous executable is backed up. Existing build warnings remain.
- JOIN128 runs the same executable with the candidate disabled. It reports Vulkan
  device loss at native time **3:09.654691**, shortly after the urgent-message
  dismissal. No malformed-source diagnostic preceded that failure in this run.
- JOIN129 enables the candidate from boot. A fresh bounded probe supplies **400
  bindings**, including **15 canonical zero-local draws**. All have control
  `0x03000000`, no color writes/varyings/alpha/coverage/stipple/texture kill, and
  depth testing and depth writes enabled. Three also enable stencil testing.
  None reads the known floating-point TXD source or changes its first sampled bytes.
- Through native **7:52**, JOIN129 retains **zero malformed-source diagnostics**
  and **2,100 program additions**. The last observed image is a loading screen;
  there is no accepted live world/menu/body comparison from this run. Vulkan
  device loss still occurs at **7:55.125342**. The candidate therefore does not
  resolve the device-loss fault. Program counts from different gameplay phases
  cannot establish a measured performance improvement.
- Both failed runs are stopped by exact identity, client before services, with
  recorder drain first and XInput restored. The marker is removed after the test.
  Post-fatal memory reads are not gameplay acceptance.

JOIN130 repeats the candidate and still reports device loss at native **3:04.432329**,
this time surfaced on an SPU thread. It supplies a valid character-select screenshot
before the failure; no visible world/menu/body acceptance follows. Its early 400-line
probe contains ordinary bindings and no zero-local draw, so it adds no canonical-path
acceptance beyond JOIN129. The same asynchronous device error appearing on different
CPU threads does not identify the GPU operation that caused it. Ordered cleanup is
verified complete and the experimental markers are absent afterwards.

The repaired content worker reads that fatal from the open native log, automatically
preserves its available buffer and submits a **74.65-second** native game/audio source
package. Metadata labels the renderer Known Broken, links a BEFORE comparison group,
records the truncated post-event window and requires downstream publication review.
The package's NAS receipt is SUBMITTED; this is failure documentation, not a crash fix.

`scripts/rpcs3_fragment_depth_audit.py --checkout PRIVATE_RPCS3 --out NEW_PRIVATE_DIR`
generates the standalone audit. Compile the emitted source as C++20 and run it.
`scripts/rpcs3_fragment_binding_summary.py --log PRIVATE_LOG --out PRIVATE_JSON`
preserves full binding details privately and summarizes draw-state combinations.
Its two regression tests distinguish canonical and original guest sources and
reject incomplete diagnostic records.

## JOIN131 recording control and partial world/menu acceptance

JOIN131 begins with automatic recording disabled only for this owned comparison;
the default capture configuration is unchanged. It plays the resurrection scenes,
creates the Guardian in slice 112, and retains the five correct arrangements/model
roots. At native 13:44 it has zero fatal/malformed-source entries, zero recording
starts and 2,118 program additions. The captured character preview has normal body
proportions and solid armour, equipment icons resolve, Materials shows 0/20, and the
returning outdoor seated emote has solid armour.

Automatic capture is restored late in the same run. Its fresh probe brings the
total to 800 bindings / 30 canonical zero-local draws, including depth-write-disabled
query-style draws. All sampled color/alpha/coverage/varying exclusions remain clear.
The worker automatically detects the Experimental canonical-path observation and
submits a 60.55-second source window. Available pre-event footage starts late and is
labelled; no recording command or manual clip selection is needed.

The later character preview at 22:39:05 and 22:40:30 BST has persistent coloured noise
on the leg texture while its proportions and icons remain correct. The subsequent
outdoor dance has solid armour in its sampled view. The final native 17:48 snapshot
has zero fatal/malformed-source entries, 2,119 additions and eight recording starts.
This is a useful bounded world/menu result, not complete rendering acceptance.
The timing does not establish recording as the cause of either the texture error or
earlier device loss; longer independent controls and resource-lifetime tracing are
needed. Indoor appearance, temporal flicker and other classes remain unaccepted.

Ordered cleanup is verified, experimental markers are removed, XInput is restored,
and the normal capture worker remains healthy and enabled. The enhanced diagnostic
build is deployed afterwards; its fault-report path has not yet been exercised.

Next: trace the character-preview texture and remaining device loss with the
[expanded Vulkan report](rpcs3-device-fault-detail.md), then repeat indoor body checks.
The cache guard and parser fix remain independent improvements; neither a refusal
to store bad shaders nor absence of their log messages proves renderer stability.
