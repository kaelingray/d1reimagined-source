# RenderDoc: the orbit frame, captured and measured - 2026-08-10

> # ⭐⭐⭐⭐⭐ THE STOPPING POINT IS NAMED: **THE GAME PAINTS ITS OWN FINISHED FRAME BLACK** `[C]` 2026-08-10
>
> **The last draw of the in-world frame is a fullscreen untextured quad that blacks out a completed
> image.** Measured by saving the display buffer *between* the two final draws:
>
> | after event | display buffer `382` | PNG size |
> |---|---|---:|
> | **76546** - copy of the composited scene | ⭐ **the finished First Light frame** | **2,696,948 B** |
> | **76555** - untextured fullscreen quad | ⛔ **black** | **82,420 B** |
>
> `382` is the image the presenter samples (`GetReadOnlyResources` at event 76567 ⇒ `[382]`), and it is
> written by **exactly two draws in the whole frame** (`GetUsage` ⇒ 5 uses: 2 ColorTarget, 1 sampled).
> Draw 76546 samples `6637` and copies it in. **Draw 76555 samples NOTHING** - zero bound textures - 
> blend `One / InvSrcAlpha`, write mask RGBA, under a **guest** PM4 marker
> (`gfx:0x2c2317a64:DrawIndexAuto`). ⇒ **the game issues it, not the emulator.**
>
> ⭐ **The control was already in hand:** the *same* texture `382` ends the **orbit** frame holding the
> full image (`final-382.png`, 3.2 MB) and ends the **in-world** frame black. Same buffer, same
> presenter, same code path, opposite outcome.
>
> ## ⛔⛔ THIS KILLS BOTH OF THE NIGHT'S RENDERER THEORIES, INCLUDING MINE FROM AN HOUR AGO
>
> **1. It is NOT shadPS4's texture cache.** I proposed `FindImage`'s overlap-resolve failure
> (`texture_cache.cpp:590` - which really does `FreeImage()` and substitute a blank) as the mechanism.
> **Measured by density against the state windows of the same run:**
>
> | window | `Image overlap resolve failed` | `SanitizeCopyLayers` |
> |---|---:|---:|
> | orbit - screen **correct** | **33.31** / 1k lines | 499.6 / 1k |
> | in-world - screen **black** | **31.56** / 1k lines | 473.4 / 1k |
>
> **Identical, marginally lower in-world.** A cause cannot fire at the same rate while the symptom is
> absent. ⇒ **`CLAUDE.md`'s original judgement - the texture-cache noise is "real and irrelevant" - 
> was RIGHT, and my re-opening of it above is RETRACTED.**
>
> **2. It is NOT a missing character/biped.** The frame is fully rendered *before* the blackout. A
> missing player cannot explain a quad drawn over a finished image. The biped absence is real and
> unrelated. ⇒ `historicaldocs/design/biped-census.md` stands, untouched, and is a **separate** failure.
>
> ## ⚠ WHAT IT PROBABLY IS - `[U]`, with a mechanism, not a guess list
>
> A fullscreen premultiplied-alpha quad drawn last, over everything, is the shape of a **screen fade**.
> The leading candidate is **our own rung-34 poke**: `done` was forced `0x1FF → 0x7FF`, and
> `CLAUDE.md` states plainly that **tasks 9 and 10 never actually completed** - *"this is the
> load-bearing poke and it SKIPS work."* If either task is the fade-in / hand-off to gameplay, the
> screen is left faded out **by our own shortcut**, and the black screen is self-inflicted.
>
> ⛔ **Not yet established, and I will not assert it.** What would settle it, cheaply:
> 1. `d1-live watch --rw exec` on the fade submitter, or the constant-buffer address behind draw 76555,
>    during a live in-world session - names the writer with no candidate list.
> 2. A capture with tasks 9/10 **not** skipped (needs the real content lookups to pass) - if the quad
>    disappears, it is proven.
> 3. `private_repair`'s `Submitting fade out over '%lld'ms` is **NOT** it - **0 occurrences** in this
>    run. ⚠ Weak on its own: that is the only fade string in the catalogue, so the general fade path
>    is unlogged (`LESSONS.md`'s third diagnostics variant).
>
> ⭐ **The methodological point:** the frame was *finished and correct* one draw before the end. Every
> theory this project has had about the black screen - content, character, renderer, texture cache - 
> was looking upstream of a step that had already succeeded.

> # ⭐⭐⭐⭐⭐ THE IN-WORLD BLACK SCREEN IS A **PRESENTATION** FAILURE. THE WORLD RENDERS. `[C]` 2026-08-10
>
> **Captured live at `activity:in_world` on `pvp_moonbase` / activity 407, zero `error.*` in the whole
> run.** ⇒ `runs/20260810-161501-RDOC-ORBIT-JANK/captures/inworld-moonbase-407.rdc` (1.4 GB,
> **76,574 events**).
>
> | | orbit | **in-world** |
> |---|---:|---:|
> | events | 3,663 | **76,574** |
> | drawcalls | 253 | **8,176** |
> | render passes | 188 | **455** |
> | ⭐ draws per pass | 1.35 | **17.97** |
> | `vkCmdCopyImage` | 55 | 55 |
> | `vkCmdCopyBufferToImage` | 60 | 60 |
>
> ⭐⭐ **The scene HDR buffer contains First Light, fully lit and textured** - the lunar base, two
> geodesic domes, supply crates, moon regolith with tyre tracks, interior lighting.
> ⇒ `captures/inworld-hdr-6659.png` (3.2 MB) and `inworld-hdr-6704.png` (3.2 MB).
> ⛔ **The final composited target is BLACK** - `captures/inworld-final-524729.png`, 72 KB of nothing,
> written by the last of 8,176 drawcalls.
>
> ⇒ **8,176 draws are submitted, the world is rasterised correctly into a 1920×1080
> `R16G16B16A16_FLOAT` buffer, and then the frame is lost between that buffer and the screen.**
> That is `CLAUDE.md`'s branch **(B)** - *submitted and never presented* - and it is the emulator's,
> not ours.
>
> ⚠⚠ **THIS RE-OPENS SOMETHING THE PROJECT CLOSED.** `CLAUDE.md`'s 2026-08-10 block states of the
> texture-cache noise: *"the texture-cache noise is real and irrelevant, and chasing it would have cost
> a shadPS4 build."* That judgement was made when the belief was *nothing is drawn*. **Something is
> drawn - an entire map - and it does not reach the screen.** The `FindImage: Image overlap resolve
> failed` ×52,055 is now a live suspect again, not noise.
> ⛔ **It does NOT resurrect the biped claim.** No Guardian is visible in the scene buffer, consistent
> with `historicaldocs/design/biped-census.md`'s zero type-9 sobjects. **The world renders; the player still does
> not exist.** Two independent failures, and this capture separates them for the first time.
>
> ⭐ **The methodological point, which is the whole reason this was catchable:** the final-target image
> is black, and a black image is exactly what this project has spent months (correctly) refusing to
> interpret. **It only became evidence because the same instrument, in the same session, produced a
> perfect orbit frame** - and because the intermediate buffer was checked instead of the last one.
> **Never read the final target alone.**

`[C]` `observed-live` · run `20260810-161501-RDOC-ORBIT-JANK` · capture
`runs/20260810-161501-RDOC-ORBIT-JANK/captures/orbit-jank-1.rdc` (316 MB, 3,663 events)

> ## ⛔⛔ RETRACTED WITHIN THE HOUR - **shadPS4 IS NOT THE PASS SPLITTER. D1 IS.** `[C]` 2026-08-10
>
> §7's item 1 proposed *"name the pass splitter … if it is a conservative barrier on every
> texture-cache touch, that is a fixable emulator-side defect and the single highest-value performance
> finding available."* **Measured, and it is false.**
>
> `Scheduler::BeginRendering` (`vk_scheduler.cpp:31`) **already batches** - it returns early when
> `is_rendering && render_state == new_state`. So a pass break requires either a genuine render-state
> change or a forced `EndRendering()`. Which one dominates is answerable from the capture, by asking
> of every **adjacent pair of draws**: were they split, and did their bound targets differ?
>
> ```
> 253 draws  ⇒  252 adjacent pairs
>    66  in the SAME pass                     (batching works)
>   177  SPLIT, and the targets DIFFER        ⇐ 95% of splits - MANDATORY, the game's own structure
>     9  SPLIT with IDENTICAL targets         ⇐ the only candidates for a needless split
> ```
> `66 + 177 + 9 = 252` - every pair accounted for, which is the control on the census.
>
> ⇒ **~188 render passes is a faithful translation of what D1 actually submits**, not an emulator
> defect. Eight of the nine identical-target splits are a bare `END → BEGIN` with nothing between them
> (consistent with the buffer-barrier `EndRendering()` at `vk_pipeline_common.cpp:34`); the ninth has
> two `vkCmdCopyBufferToImage` between, i.e. legitimate. **Fixing all nine would remove 9 of 186 pass
> breaks - a micro-optimisation, not the jank.**
>
> ⚠ **What this costs the document:** §2's *measurement* stands entirely (1.35 draws/pass is real), but
> its *implication* - that the structure is pathological and someone should fix it - does not. **A
> frame of 188 small passes is what this renderer does.** The jank therefore remains `[U]`, and §7 is
> rewritten below.
>
> ⭐ **The lesson, and it is `LESSONS.md`'s oldest one wearing new clothes:** I had the draws-per-pass
> ratio and reached for "the emulator is doing something dumb" without asking *what would the correct
> output look like?* The discriminator - same-target-but-split - cost one query against a capture that
> was already open. **A ratio is not a defect until you have measured what the non-defective ratio is.**

> ## ⭐⭐⭐ THE (A)-vs-(B) QUESTION IS SETTLED, AND IT IS **(B)**
>
> `CLAUDE.md`'s RenderDoc block names the two causes this project has conflated from the start:
> **(A)** nothing is submitted (ours) versus **(B)** submitted and never presented (the emulator's).
> **Orbit submits a complete, correct frame and it reaches the screen.** The saved final colour target
> (`captures/final-309.png`) shows Earth, the sun, the destination node, `SET DESTINATION`, the
> `D1Guardian` nameplate and the `Invite Friends` / `Menu` footer. Nothing is missing from the picture.
>
> ⇒ **The orbit jank is not a missing-content problem. It is a frame-structure problem, and it is
> the emulator's translation, not the client's submission.**

---

## 1. What was measured

One captured frame, parked in `setup:orbit`, on a client launched `D1_RENDERDOC=1`.

| quantity | value |
|---|---:|
| events in the capture | **3,663** |
| actions | **1,312** |
| **drawcalls** | **253** (192 `vkCmdDraw`, 61 `vkCmdDrawIndexed`) |
| **render passes** (`BeginPass`/`EndPass`) | **188** |
| ⭐ **draws per render pass** | **1.35** |
| `vkCmdBeginRendering(Load)` | **127** |
| `vkCmdEndRendering(Store)` | **128** |
| `vkCmdCopyImage` | **55** |
| `vkCmdCopyBufferToImage` | **60** |
| `vkCmdDispatch` | **59** |
| distinct textures | **183** |

## 2. ⭐⭐ The signature: one draw per render pass, ~188 times a frame

Run-length encoding the command stream collapses to the same three-op cycle, over and over:

```
BEGINx1@26   DRAWx1@28   ENDx1@31
BEGINx1@45   DRAWx1@47   ENDx1@50
BEGINx1@60   DRAWx2@62   ENDx1@78
BEGINx1@79   DRAWx1@81   ENDx1@84
BEGINx1@94   DRAWx1@96   ENDx1@99
…
```

**734 runs over 801 classified ops.** A healthy frame batches many draws inside a few passes; this one
opens and closes a pass for essentially every draw, and the overwhelming majority are
`BeginRendering(**Load**)` → `EndRendering(**Store**)`, i.e. the attachment is read in and written out
around a single draw.

⇒ **This is the mechanism behind the `SanitizeCopyLayers` count.** The sibling session's live census of
the *previous* run's log recorded **786,211 `SanitizeCopyLayers` against 52,055
`FindImage: Image overlap resolve failed` - ~15:1**, a different shape from the historical 1:1. Each
pass boundary is an opportunity for shadPS4's texture cache to re-examine image overlap; ~188 of them
per frame is what feeds that counter.

## 3. ⚠ AND THE OBVIOUS BANDWIDTH STORY IS **WRONG** - most of these targets are tiny

The tempting claim is "127 Load/Store of a 1920×1080 HDR target ≈ 4 GB/frame". **Measured, it is not.**
Sampling 25 drawcalls across the frame and reading each one's bound attachments through the replay
thread:

| attachment | count in sample |
|---|---:|
| `COLOR 1920x1080 R16G16B16A16_FLOAT` | 4 |
| `DEPTH 1920x1080 D32S8` | 4 |
| `COLOR 1920x1080 R8G8B8A8_SRGB` | 3 |
| `COLOR 32x32 R16G16B16A16_FLOAT` | 3 |
| `COLOR 32x32 R8G8B8A8_UNORM` | 3 |
| `COLOR 640x360` · `240x135` · `80x90` · `60x135` · `486x108` · `512x512` | 1 each |
| ⭐ `COLOR 2x8` · `1x4` · `4x16` `R16G16B16A16_FLOAT` | 1 each |

**Targets of `1×4`, `2×8`, `4×16` and `32×32` are getting their own full Vulkan render pass with a
Load and a Store.** These are reduction / lookup / tone-curve style passes that cost the PS4 GPU
almost nothing and cost a desktop Vulkan driver a *fixed* per-pass price regardless of size.

⇒ **The cost is per-pass overhead multiplied by ~188, not attachment bandwidth.** Enlarging the
analysis to "it's thrashing the framebuffer" would have been a confident, plausible, wrong diagnosis - 
the same shape as every entry in `status/LESSONS.md`. The sample is what refutes it.

⚠ **Honest bound:** 25 of 253 draws were sampled, chosen by stride. The census above is a *sample*, not
a complete attachment inventory. It is enough to refute "they are all the big target"; it is **not**
enough to state the exact per-size distribution. ⇒ a full pass needs all 253.

## 4. The instrument, and how it was driven

⭐ **`external/agentic-renderdoc-master` drives a live RenderDoc replay session over MCP** - but the
route that worked here is **not** the MCP.

| piece | state |
|---|---|
| MCP server | installed into `external/agentic-renderdoc-master/.venv` (`mcp 2.0.0`), own suite **6 passed**, registered at user scope, `claude mcp list` → **✔ Connected** |
| RenderDoc extension | `~/.local/share/qrenderdoc/extensions/agentic_renderdoc`, auto-loaded via `UI.config`'s `AlwaysLoad_Extensions` |
| the bridge | JSON-lines TCP on **19876** |
| ⭐ what was actually used | **`scripts/probe.py`** - talks to the extension directly, bypassing the MCP server |

⛔ **A mid-session MCP registration is invisible to the running session** - the tool list is fixed at
session start, so `mcp__renderdoc__*` never appears however healthy the server is. `probe.py` is the
escape hatch and gives the identical `Eval` surface.

### ⚠ Three traps paid for here

1. ⛔ **`QT_QPA_PLATFORM=wayland qrenderdoc` is WRONG, and `CLAUDE.md` prescribes it.** RenderDoc has no
   native-Wayland support (its own CMake gates it behind
   `ENABLE_UNSUPPORTED_EXPERIMENTAL_POSSIBLY_BROKEN_WAYLAND`) and warns that it will crash, hang or
   fail to render. **Use `DISPLAY=:0 XAUTHORITY=<mutter Xwaylandauth> QT_QPA_PLATFORM=xcb`.**
   ⇒ this is the *same* fact the project already documented for the shadPS4 side and then contradicted
   in its own GUI invocation line.
2. ⛔ **A replay query from the wrong thread SEGFAULTS qrenderdoc (exit 139), killing the session
   mid-analysis.** `ctx.GetTextures()` called directly runs on the bridge thread. **Every replay query
   must go through `ctx.replay(callback)`**, which wraps `ctx.Replay().BlockInvoke`. The controller
   handed to the callback is the safe object (`controller.GetTextures()`, `SetFrameEvent`,
   `GetPipelineState`, `SaveTexture`).
3. ⚠ **`get_all_actions()` returns serialized dicts (`eventId`/`name`/`flags`), not RenderDoc action
   objects.** `a.GetName(sf)` fails on all 1,312 and returns a uniform error - which reads exactly like
   "the capture has no names". `inspect(a)` says `dict` in one call.

### ⭐ Firing the capture

`hotkey_capture_frame = f12` in `~/.local/share/shadPS4/input_config/global.ini`.
⚠ **`xdotool key --window <id>` (XSendEvent) is NOT reliably honoured by SDL** - the capture that
landed did so on the synthetic attempt, but the follow-up XTEST attempt produced no new file, so
**neither path is proven; verify by the FILE, never by the keypress returning 0.**
⛔ **shadPS4 overwrites a single `CUSA00219_capture.rdc`** rather than numbering captures - the
11:33 capture from run `20260810-113522-RDOC-3` was destroyed by this one. **Copy it into the run
directory immediately or lose it.**

## 5. The positive control, and why it was mandatory

The first three images saved (`present-165/166/167.png`, the 1920×1011 `B8G8R8A8_UNORM` swapchain
images) came back **blank white, all three byte-identical in size**. Taken at face value that reads as
*"orbit renders nothing"* - the exact false negative this project is built to catch.

⭐ **They were the wrong images**: the swapchain images were not written at the event queried. Saving
the **final drawcall's own bound output target** (`309`, `1797×1011 B8G8R8A8_UNORM`, event 3656)
produced the complete orbit frame. ⇒ `captures/final-309.png`, `captures/final-382.png`.

⚠ **"An empty orbit capture means the instrument is broken, not the world"** - stated in advance by the
sibling session, and it fired within the hour. Had the blank been reported, it would have manufactured
a renderer bug that does not exist.

⭐ **A corroboration that came free:** the nameplate in the captured frame reads `D1Guardian` with
level **0** and light **✦0** - independent visual confirmation of the blank-HUD roster finding in
`historicaldocs/design/orbit-hud-blank.md`, from a completely different instrument.

## 6. What this does NOT say

- ⛔ It says **nothing about `activity:in_world`.** This is an orbit frame. The 577-sobject /
  zero-biped finding in `historicaldocs/design/biped-census.md` is untouched by it.
- ⛔ It does **not** name the shadPS4 code that splits the passes. "188 passes exist" is measured;
  *why* the translator emits them is `[U]`.
- ⚠ The `~15:1 SanitizeCopyLayers : FindImage` ratio is from the **previous** run
  (`20260810-145553-X11-ORBIT`), not this one. Same build, same world, but it is a sibling
  measurement, not this capture's.

## 7. Next - REWRITTEN after the retraction above

⛔ **"Name the pass splitter" is CLOSED, answered, and the answer is "the game".** Do not re-open it.
⛔ **The pass count is not the jank.** Any further work that starts from "188 passes is too many" is
starting from a refuted premise.

**The jank cause is `[U]` again**, and honestly so. What can still be asked of this capture, and of the
next one:

1. **Where does the frame's TIME go?** Draw/pass counts are structure, not cost. RenderDoc can time
   the actual events (`FetchCounters` / the pipeline-statistics + timestamp counters). **That is the
   measurement that was skipped**, and it is the only one that names a hot spot rather than a shape.
   Until it is taken, *nothing here is a performance finding* - it is a frame inventory.
2. **Capture `activity:in_world` and diff it against this orbit frame.** Costs one `HOTKEY_RENDERDOC`
   press on an already-armed client.
3. **Full 253-draw attachment inventory** - turns §3's 25-draw sample into a distribution.
4. ⚠ **Re-ask whether the jank is even in the captured frame.** A capture is one frame; the operator's
   symptom is *stutter over time*. Frame-to-frame variance, texture-cache thrash across frames, and
   host-side stalls are all invisible to a single capture. **The instrument may be answering a
   different question from the one the symptom poses** - which is this project's signature failure, and
   it applies to the tool I just built.
