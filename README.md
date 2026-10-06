# D1 Reimagined

An experimental Destiny 1 preservation and emulation project. The active target is
**PS3 BLES01857 01.24 (Destiny 2.3.1 / The Taken King)** in a patched RPCS3 build,
using local service stand-ins. The earlier Xbox 360/Xenia and PS3 01.00 work remains
research history. **The full game is not yet playable.**

Source and engineering notes: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
Private research inputs: [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).
The repositories were renamed on **2026-10-06**. Existing local checkout/runtime
folders may still have their earlier names; use explicit paths for imported runtimes.

## Current state — 2026-10-06

| Area | Verified scope | Remaining work |
| --- | --- | --- |
| Local sign-in and characters | 01.24 signs in, persists a stored character and setup answers, and joins the local activity host. | This uses local stand-ins; complete account/profile semantics and broader character coverage are incomplete. |
| Opening mission | Earlier native runs play the resurrection cinematics, hand control to the Guardian outside the Wall, show initial objectives and enter the Wall. Activity 229 uses scenario checksum `0x8fe07299`. | Reliable Breach slice handoff, objectives, rifle pickup, combat, mission completion and Tower arrival remain unverified. A world-entry state label alone proves none of these. |
| Guardian and inventory | Arrangement indexes and fixed equipment bucket ranges repair missing armour and misplaced item records. Bounded runs resolve five armour slots, Ghost and subclass icons, with normal Titan proportions in some views. | Later views still show texture corruption/distortion; level/light values and stable combined character, outdoor and mission rendering remain open. |
| Ghost | Authored member/performance bindings create one bound Ghost after the tested readiness/reset gate; helper components resolve. | The reset remains a protocol experiment. Visible torch behavior and automatic mission cues are unverified. |
| Vulkan stability | Actual-source CPU checks and bounded native comparisons support queue serialization and matching query scopes. GC and fence cleanup contracts have further CPU checks. | Descriptor-pool resets and depth/stencil layout errors remain under investigation. A longer run without device loss does not establish a graphics fix. |
| Automated evidence capture | Native game/audio recording, event deduplication, durable outbox, schema-1 packages, full decoding, hashes and automatic verified transfer have live acceptance. The capture suite passes 77 tests. | Rotation gaps and intermittent native source-identification failures remain explicit. Encounter/reward/multiplayer milestones require further reliable signals. |
| Broader content and multiplayer | Static package/scenario research and local protocol experiments provide a basis for later work. | Complete destinations, activities, AI, combat, progression, vendors, rewards, fireteams and multiplayer are not accepted. |

The [October 6 checkpoint](notes/checkpoint-2026-10-06-renderer.md) and
[Vulkan conformance record](notes/rpcs3-vulkan-conformance.md) hold the latest renderer
evidence. JOIN154 completes its bounded fixture but retains descriptor/layout errors.
JOIN155 completes without validation IDs or device loss, but diagnostic trace coverage
ends before earlier failures and the inspected outdoor scene is still distorted.
Those results do **not** accept the flicker/distortion repair. A new attachment-layout
candidate passes 31 CPU checks against 15 failures in the original control flow;
its full Release build passes, but it is not deployed and native/visual acceptance
is **NOT RUN**. JOIN156 repeats the earlier build with tracing off and no validation
IDs through 11:56.782564; its final inspected loading views supply no new rendering
acceptance. Read the [October 6 handoff](notes/HANDOFF-2026-10-06.md) before continuing.

The depth-only shader candidate stays disabled by default. Queue, query, GC and fence
audits have 48, 29, 14 and 71 passing CPU checks respectively; these counts describe
isolated contracts rather than whole-game acceptance. Follow
[Guardian Rises acceptance](notes/guardian-rises-acceptance.md) for the visible,
continuous checks still required. The [October 5 checkpoint](notes/checkpoint-2026-10-05-mission.md),
[appearance repair](notes/guardian-appearance-0124.md) and
[inventory repair](notes/inventory-items-0124.md) retain their bounded original evidence.

Priority is core gameplay, stability, fidelity, content coverage, multiplayer/backend
completeness, then polish. Next: validate the attachment-layout correction in the same
native scenario, correlate any remaining descriptor reset with actual submission and
completion, and obtain stable character/equipment, outdoor and mission views together.
Continue the stalled slice-transition investigation once the renderer permits it.

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
| `patches/` | Scoped RPCS3 and historical Xenia source increments; their acceptance is documented individually. |
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

The Xbox 360/Xenia route is paused. It reached a distorted title screen and the
`canary` error; a content import patch did not establish a working online/gameplay
path. See [the original inventory](notes/inventory.md),
[network options](notes/network-options.md) and [test history](notes/test-log.md).
