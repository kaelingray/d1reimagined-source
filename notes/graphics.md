# Xenia title-screen corruption

Symptom (Tests 001–013): the title screen shows regular **64-pixel horizontal strips** across the Destiny logo and the background; whole strips of the logo are missing and background strips differ in brightness. Direct3D 12 and Vulkan look the same (Test 003), so the fault is in code shared by both backends.

## Unattended harness

`scripts/Invoke-XeniaGraphicsTest.ps1` creates `D:\Destiny360\XENIA_GFX_<TestId>`, copies the patched Test 009 executable and config, applies config overrides, boots the working-copy game with the imported Content 1–3 and title update, and presses **F12** (Xenia's guest-output screenshot) at set times. It can also press **F4** (one-frame GPU trace). No controller input is needed because the title screen appears on its own. Screenshots, config snapshots, and logs are copied to `D:\Destiny360\LOGS\Gfx-<TestId>*` (private).

## Settings tests (2026-09-24)

Each changed one setting from the Test 009 config. Screenshots at 60 s and 90 s were compared pixel by pixel with the baseline (`G00`).

| Test | Setting | Result |
| --- | --- | --- |
| G00 | none (baseline) | strips reproduced |
| G01 | `render_target_path_d3d12 = "rov"` | identical to baseline (0 pixels differ > 16/255) |
| G02 | `readback_resolve = "fast"` | identical except ≤ 11 pixels (animated stars) |
| G03 | `readback_resolve = "full"` | identical except ≤ 10 pixels |
| G04 | `clear_memory_page_state = true` | identical except ≤ 10 pixels |
| G05 | `execute_unclipped_draw_vs_on_cpu = false` | identical except ≤ 11 pixels |

Conclusion: the strips do not depend on the D3D12 render-target path, CPU readback of resolves, GPU-written page tracking, or the unclipped-draw EDRAM extent estimate. The baseline log contains no GPU warnings. Remaining candidates include texture decoding or invalidation of a streamed texture, a resolve/copy region calculation, or a skipped draw.

## GPU trace

Xenia's frame trace (F4) is compiled only into Debug builds (`XE_ENABLE_TRACE_WRITER_INSTRUMENTATION`), so pressing F4 in the Release test build did nothing (`G10_trace`). `patches/xenia-trace-writer-release.patch` adds an `#ifndef` guard so the switch can be set at build time; normal builds are unchanged. A separate tree, `build-trace`, is configured with `/DXE_ENABLE_TRACE_WRITER_INSTRUMENTATION=1` for trace capture and the `xenia-gpu-d3d12-trace-dump` replay tool.
