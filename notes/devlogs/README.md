# Development blog material

The workflow changed on October 5: this development machine now supplies original
source footage and evidence packages through [automatic NAS capture](../../tools/content_capture/README.md).
The separate content machine owns episode construction, editing, rendering and
publishing. Existing review cuts and drafts below are historical material, not the
normal production workflow. New packages state maturity and remaining issues explicitly.

Engineering takes priority. Prepare an episode when a tested change is worth showing;
do not turn static asset discovery or a passing serializer test into a gameplay claim.
Use the current checkpoint and acceptance notes as the source of truth.

The chosen presentation is a captioned short and a narrated, captioned longer edit.
Episode 01 covers the October 5 authored Ghost binding fix, the separate manual movement
probe and the unfinished in-world rendering. Both review cuts now use **recorded gameplay
in every section**, including the explanation, regression and outro sections that previously
had blank cards. Footage comes from fresh JOIN115/116 tests, recorded by RPCS3 itself.
The manifest records private clip basenames, source windows and caption/narration text.
These are edited test runs with some reused b-roll, not a continuous completed playthrough.
Windows desktop speech supplies clearly labeled synthetic narration for the review cut.
Replace it with recorded narration if desired before publishing.

## Render privately

Install `Pillow` and either supply `--ffmpeg` or install `imageio-ffmpeg` in a task-local
Python environment. Long edits currently need Windows PowerShell 5.1, System.Speech and
the selected desktop voice (default: Microsoft Hazel Desktop). Set an installed voice
with `--voice`. Native game audio is retained in the short and lowered under narration in
the longer edit. No music is added.

```powershell
python scripts/render_devlog.py notes/devlogs/01-2026-10-05.json `
    --footage <private-selected-clip-directory> --out <new-private-output-directory> --mode short
python scripts/render_devlog.py notes/devlogs/01-2026-10-05.json `
    --footage <private-selected-clip-directory> --out <another-new-private-output-directory> --mode long
```

The output directory must be new. The renderer refuses tracked output paths, missing
footage, escaped source paths and trims beyond the recording. A section may repeat its
selected footage only with an explicit Boolean `loop: true`; repeated b-roll is recorded in
the private timing manifest. Missing media never falls back to an empty card. Six focused
regressions cover those source checks. Run `python -m unittest discover -s scripts -p
'test_render_devlog.py'`.

Captions sit over moving footage; the complete landscape view is retained for traversal
and appearance comparisons. Ghost clips are explicitly cropped detail views. The same
moving source fills the surrounding space, keeping every section visually populated.
The renderer exports MP4, SRT and a private source/timing manifest. Each narrated caption
has its own WAV; timing is aligned to output frames. `--narration-cache <previous-output>`
requires matching voice metadata and reuses a WAV only when its saved job text matches.
PCM intermediates and explicit sample/frame clocks keep audio monotonic through loops
and edit boundaries; AAC is encoded once in the final MP4. Verify decoded video
frames, full decode, duration and audio before treating a render as ready for review.
Rendering does not upload or publish anything. Captures, frames, audio and videos stay
outside Git under the existing data policy.

Episode 01's revised private outputs are a 52-second 1080×1920 short and a
5:26.375 1920×1080 narrated edit, both H.264/24-fps with stereo AAC. Full decoding,
section frame review and video-area motion checks pass for all 8/38 segments.

The [Episode 02 inventory draft](02-2026-10-05-inventory.md) covers the user's
Materials-tab observation and the tested equipment-record/bucket repair.
Reviewed before/after video with native audio is retained privately. This is an
outline and selected footage, not a finished episode or a renderer-stability claim.

## Episode 01 publishing drafts

**Short title:** Ghost existed. Destiny's mission couldn't use him.

**Short post:** A small Destiny 1 preservation milestone on PS3 01.24: one Ghost now has
the authored member and performance bindings the intro mission needs. A separate manual
action test moved him, but torch behavior, progression and Guardian rendering still need
work. Experimental local build; the full game is not yet playable. #Destiny1 #GamePreservation #Devlog

**YouTube title:** Restoring Destiny 1's Ghost: why spawning him wasn't enough | Devlog 01

**YouTube description:** This episode documents the authored Ghost binding fix on PS3
01.24, the native traces behind it, a separate manual movement experiment, and the
remaining rendering and mission gaps. Local sign-in, saved characters, revival and
highway traversal are partially working. The readiness reset is still a test action;
torch, rifle pickup, combat, mission completion and Tower arrival are unverified.
This review edit uses recorded local gameplay, native game audio and synthetic draft
narration. Some footage is reused as b-roll; diagnostic claims come from the linked
development evidence, not the appearance of an illustrative clip.

Repository: https://github.com/p3n1cillin/d1xenia

**Outline:** Current playable slice → count-only spawn bug → authored member fix →
regression versus live evidence → manual movement probe → replay skip → rendering
defect → first-mission priorities. Use the private timing manifest to add chapters
after reviewing the finished edit.

## Capture and evidence

Use RPCS3's native F11 recorder, which captures the rendered game and game audio rather
than the desktop. `scripts/Send-RpcsKey.ps1 -Keys F11` supports the exact owned emulator
identity; pass both `-ProcessId` and `-ProcessIdentity` from its process manifest. Start
once, stop once, and verify the resulting MP4 after the encoder finishes. An active file
can initially be zero bytes and cannot be probed before finalization; that is not a reason
to repeatedly toggle or restart the emulator. Recordings are stored privately under the
selected RPCS3 copy's `recordings/BLES01857/` directory.

JOIN115 records ordinary controller traversal, a normal inventory model, the bound Ghost
and the manually triggered configured action. Read-only observations repeat one actor
with both owner bindings and roughly 21 units of movement. The readiness reset and action
remain external experiments; a blue core does not establish a working torch. JOIN116
supplies revival and daylight appearance footage. Selected clips and their original
recording/trim/crop index stay private alongside the rendered review files.

For future edits, prioritize a same-camera count-only versus authored-member comparison,
an uninterrupted action test with position telemetry, and visible torch, rifle pickup or
combat only after each actually works. Keep unresolved rendering visible and labeled.
