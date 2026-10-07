# D1 Reimagined

An experimental Destiny 1 preservation and emulation project. The active target is
**PS3 BLES01857 01.24 (Destiny 2.3.1 / The Taken King)** in a patched RPCS3 build,
using local service stand-ins. Earlier PS3 01.00 work remains
research history. **The full game is not yet playable.**

Source and engineering notes: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
Private research inputs: [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).
The repositories were renamed on **2026-10-06**. Existing local checkout/runtime
folders may still have their earlier names; use explicit paths for imported runtimes.

## Current state — 2026-10-07

| Area | Verified scope | Remaining work |
| --- | --- | --- |
| Local sign-in and characters | 01.24 signs in, persists a stored character and setup answers, and joins the local activity host. | This uses local stand-ins; complete account/profile semantics and broader character coverage are incomplete. |
| Opening mission | Earlier native runs play the resurrection cinematics, hand control to the Guardian outside the Wall, show initial objectives and enter the Wall. Activity 229 uses scenario checksum `0x8fe07299`. | Reliable Breach slice handoff, objectives, rifle pickup, combat, mission completion and Tower arrival remain unverified. A world-entry state label alone proves none of these. |
| Guardian and inventory | Arrangement indexes and fixed equipment bucket ranges repair missing armour and misplaced item records. Bounded runs resolve five armour slots, Ghost and subclass icons, with normal Titan proportions in some views. | Later views still show texture corruption/distortion; level/light values and stable combined character, outdoor and mission rendering remain open. |
| Ghost | Authored member/performance bindings create one bound Ghost after the tested readiness/reset gate; helper components resolve. | The reset remains a protocol experiment. Visible torch behavior and automatic mission cues are unverified. |
| Vulkan stability | Actual-source CPU checks and bounded native comparisons support queue serialization and matching query scopes. GC and fence cleanup contracts have further CPU checks. | Descriptor-pool resets and depth/stencil layout errors remain under investigation. A longer run without device loss does not establish a graphics fix. |
| Automated evidence capture | Native game/audio recording, event deduplication, durable outbox, schema-1 packages, full decoding, hashes and automatic verified transfer have live acceptance. The capture suite passes 82 tests; 11 actual-controller checks cover pending source retention. Finalized originals now remain locally available after rolling buffer expiration. | Rotation gaps and intermittent native source-identification failures remain explicit. Background start/stop still needs a reliable native control path. Encounter/reward/multiplayer milestones require further reliable signals. |
| Broader content and multiplayer | Static package/scenario research and local protocol experiments provide a basis for later work. | Complete destinations, activities, AI, combat, progression, vendors, rewards, fireteams and multiplayer are not accepted. |

The [October 7 checkpoint](notes/checkpoint-2026-10-07-renderer.md) and
[Vulkan conformance record](notes/rpcs3-vulkan-conformance.md) hold the latest renderer
evidence. JOIN164 completes the 500-second interval and twenty input/capture phases
with core validation verified. No validation error or device loss appears through
**13:20.013137**, but inspected outdoor frames still alternate normal and black/green
shading. Loading and the urgent dialog obstruct character/equipment checks. **The
combined graphics fix fails acceptance even when the API log is clean.** Corruption
is visible before a late guest debugger snapshot; that pause is recorded separately.
JOIN165 completes a longer 600-second interval and twenty captures with no API
validation errors through **19:05.151916**. Inspected views include the mission
cinematic, menus and a rendered Titan with armour/rifle; weapon icons still load.
Returning to the mission leads to loading/contacting-server text, and the host
connection repeatedly reconnects. Inventory, traversal and sustained combined
graphics remain unaccepted. Its diagnostic checkpoint closes without an armed
RSX capture request; no saved frame or replay is claimed.
JOIN166 saves a private native RSX capture after a completed 600-second interval.
The inspected capture shows the inactivity dialog, so it is a UI control rather
than the outdoor fault. Two replay-only repairs allow that dialog to be replayed:
exclude the null allocation address and present the registered buffer matching
the final local colour target. Neither changes live-game shading or establishes
character/world/mission acceptance. A shorter follow-up exposes a launcher bug:
cancelling the unfinished sign-in observer closes its kept stack. The join now
requests a successful handoff after the user-profile reply and verifies observer
completion before reporting success. JOIN168 verifies that handoff with the exact
client still alive after its shorter 60-second interval. Cross dismisses the urgent
message and an inspected frame shows unobstructed outdoor scenery. A subsequent
native capture saves successfully. Follow-up inspection of untouched recordings
confirms loading was already visible before the capture marker, consistent with
the loading replay. Matching outdoor-frame capture/replay remains unverified.
JOIN169 dismisses the urgent dialog but fails to reach idle slice112; the guarded
route sends no walking input and the activity host reconnects. A
[read-only session probe](notes/session-transition-probe-0124.md) corrects the
context lookup and records the pending transition machines before timeout.
Its five focused checks pass; this is diagnostic progress, not a mission fix.

The [allocation cache repair](notes/rpcs3-descriptor-allocation-cache.md) passes 20
actual-source CPU checks against five original failures, with all 23 retirement
checks passing. Completed native runs JOIN160 and JOIN162 nevertheless reproduce
descriptor-reset errors. A [bounded lifetime ring](notes/rpcs3-descriptor-lifetime-ring.md)
has 20 concurrency checks and 10 decoder tests, but no native fault dump yet.
JOIN161 stalls at selection; JOIN163 stalls after matchmaking and fails capture.
JOIN164's four-minute delayed arm progresses without capturing a reset. These
comparisons do not identify either stall's cause or resolve the intermittent fault.

The runner now rejects failed screenshot-child exits and cleans up its owned stack,
with 19 capture/handoff checks, 173 isolated run checks and 20 shutdown checks passing. A new
[one-shot native RSX capture marker](notes/rpcs3-native-rsx-capture.md) now supports
verified native capture and UI-control replay. An inspected outdoor fault
capture and comparable replay are still needed. Existing SPU colour reads remain enabled.
Read the [October 6 handoff](notes/HANDOFF-2026-10-06.md) and October 7 checkpoint
before continuing; the earlier normal Titan/inventory views remain bounded evidence.

The depth-only shader candidate stays disabled by default. Queue, query, GC and fence
audits have 48, 29, 14 and 71 passing CPU checks respectively; these counts describe
isolated contracts rather than whole-game acceptance. Follow
[Guardian Rises acceptance](notes/guardian-rises-acceptance.md) for the visible,
continuous checks still required. The [October 5 checkpoint](notes/checkpoint-2026-10-05-mission.md),
[appearance repair](notes/guardian-appearance-0124.md) and
[inventory repair](notes/inventory-items-0124.md) retain their bounded original evidence.

Priority is core gameplay, stability, fidelity, content coverage, multiplayer/backend
completeness, then polish. Next: resolve the opening mission's stalled slice
transition using native session evidence, then verify visible control and mission
progress. Continue correlating recurring descriptor resets with submission and
completion, and require unobstructed character/equipment, outdoor and mission
views together for graphics acceptance.

## Reproducing the active target

Use Windows, PowerShell 7, Python 3.14, the dependencies in `requirements.txt`, a
compatible Vulkan driver, and a privately prepared RPCS3/game runtime. Game files,
firmware, keys and patched emulator binaries are not supplied by this source checkout.
The companion data checkout is restricted research input, not a ready-to-play release.
Start with the [current runbook](notes/next-session-playbook.md) and
[01.24 profile checkpoint](notes/checkpoint-2026-10-01-profile.md).

```powershell
python -m pip install -r requirements.txt
$env:D1_LOCAL = '<absolute-private-runtime-path>'
$env:D1_DATA_REPO = '<absolute-d1reimagined-data-checkout>'

pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local $env:D1_LOCAL -DataRepo $env:D1_DATA_REPO

# Stop only the stack owned by that exact run name.
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local $env:D1_LOCAL -DataRepo $env:D1_DATA_REPO -Stop
```

Every run needs a fresh name. The join wrapper leaves a successful stack running for
inspection; its `-Stop` closes the exactly owned client/services/proxy. Restore XInput
after scripted keyboard tests. `-SkipOpeningCinematic` skips a previously watched Mars
prologue at the verified frontend stage; mission revival cinematics still play.
The launchers prefer a sibling `d1reimagined-data` checkout and retain compatibility
with an existing `d1xenia-data` folder. Explicit `-DataRepo` / `D1_DATA_REPO` take precedence.

The current RPCS3 runtime is a private incremental build with additional local changes.
Individual patches and CPU audits are scoped increments, not a single self-contained
stock-RPCS3 installation recipe. Read their baseline and application notes before
applying them to a separate tree; preserve existing changes and caches. Fresh native
validation is required after a rebuild or GPU/toolchain change.

```powershell
python -m unittest discover -s server -p 'test_*.py'
python -m unittest discover -s scripts -p 'test_*.py'
python -m unittest discover -s tools/content_capture -p 'test_*.py'
```

Schema/build preflights and actual-source C++ audits are described in the runbook and
individual notes. CPU tests, static exports and successfully encoded replies do not
establish native gameplay. Use owned read-only observations and comparable input/
camera scenarios for live acceptance.

## Repository map

| Directory | Purpose |
| --- | --- |
| [server/](server/README.md) | Loopback DNS, STUN, SignOn/auth3, lobby, BAP and activity-host stand-ins, schema encoders, persistence and protocol tests. |
| `scripts/` | Run ownership, unattended input, preflights, guest-memory readers, traversal, data research and actual-source renderer audits. |
| `missions/` | Experimental mission/actor rules and [recorded traversal routes](missions/routes/README.md). |
| `patches/` | Scoped RPCS3 source increments; their acceptance is documented individually. |
| `notes/` | Dated checkpoints, findings, acceptance criteria and historical test evidence. |
| [tools/content_capture/](tools/content_capture/README.md) | Asynchronous native source recording, event detection, verification, packaging, outbox and transfer. |
| [notes/devlogs/](notes/devlogs/README.md) | Evidence index and historical draft edits; current production happens on a separate machine. |

## Evidence and privacy

Normal testing automatically records relevant native game output and audio when a
private capture configuration is enabled. Meaningful observed changes produce source
packages; unchanged capabilities are deduplicated. Dimensions, distinct reported average/
base frame rates, recording gaps, truncated coverage and recovery provenance stay in
metadata. A configured recorder rate is not achieved gameplay FPS.

The development machine supplies original sources and factual engineering context.
It does not perform final editing, narration, captions, uploads or publication.
Automatic visual privacy review is unavailable, so footage requires downstream review;
audio/music/rights checks are not implied by decoding or delivery. See the
[capture contract](tools/content_capture/README.md).

Never add game images/assets, firmware, emulator builds, saves, credentials, private
certificates, raw schemas/logs, screenshots or media to this source repository. Keep
them in the ignored `local/` directory or an explicitly selected private runtime.
The private companion data repository has a different, restricted asset policy and
must not be made public or bundled into a release. Preserve private instructions and
machine provenance outside both project documentation and media releases.

## Historical work

PS3 01.00 reached local sign-in, orbit and a spawned Guardian in the first mission;
its memory/lighting work informed the 01.24 port. These earlier build-specific results
are preserved in dated notes and are not current 01.24 acceptance.

The obsolete Xbox 360 emulator route, tools and research have been removed.
The supported development route is RPCS3 on PS3 01.24. Earlier PS3 results remain
in the [test history](notes/test-log.md).
