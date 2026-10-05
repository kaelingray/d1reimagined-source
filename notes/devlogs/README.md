# Development blog material

Engineering takes priority. Prepare an episode when a tested change is worth showing;
do not turn static asset discovery or a passing serializer test into a gameplay claim.
Use the current checkpoint and acceptance notes as the source of truth.

The chosen presentation is a captioned short and a narrated, captioned longer edit.
Episode 01 covers the October 5 authored Ghost binding fix, the separate manual movement
probe and the unfinished in-world rendering. Its manifest records exact capture basenames
and caption/narration text. These are **test-capture montages**, not continuous gameplay.
Windows desktop speech supplies clearly labeled synthetic narration for the review cut.
Replace it with recorded narration if desired before publishing.

## Render privately

Install `Pillow` and either supply `--ffmpeg` or install `imageio-ffmpeg` in a task-local
Python environment. Long edits currently need Windows PowerShell 5.1, System.Speech and
the selected desktop voice (default: Microsoft Hazel Desktop). Set an installed voice
with `--voice`. No game audio or music is added.

```powershell
python scripts/render_devlog.py notes/devlogs/01-2026-10-05.json `
    --captures <private-RPCS3-screenshot-directory> --out <new-private-output-directory> --mode short
python scripts/render_devlog.py notes/devlogs/01-2026-10-05.json `
    --captures <private-RPCS3-screenshot-directory> --out <another-new-private-output-directory> --mode long
```

The output directory must be new. The renderer refuses tracked output paths, checks
caption bounds, and exports MP4, SRT and a private timing manifest. Each narrated caption
has its own WAV, so subtitle timing follows measured audio duration. Verify decoded video
frames, full decode, duration and audio before treating a render as ready for review.
Rendering does not upload or publish anything. Captures, frames, audio and videos stay
outside Git under the existing data policy.

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
This review edit uses actual test captures and synthetic draft narration.

Repository: https://github.com/p3n1cillin/d1xenia

**Outline:** Current playable slice → count-only spawn bug → authored member fix →
regression versus live evidence → manual movement probe → replay skip → rendering
defect → first-mission priorities. Use the private timing manifest to add chapters
after reviewing the finished edit.

**Useful footage for the next edit:** continuous highway-to-Wall traversal; a same-camera
count-only versus authored-member comparison; an uninterrupted action test with position
telemetry; Guardian emote and inventory in one session; a visible torch, rifle pickup or
combat only once each is actually working. Avoid filler footage that implies later content
has been restored.
