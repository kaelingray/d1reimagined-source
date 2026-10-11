# D1 Reimagined

An experimental Destiny 1 preservation and emulation project. On **2026-10-11** the
user selected **PS4 CUSA00219 patch 01.29** as the active engineering target, using
the supplied `d1preservation-main` project in a separate working tree with shadPS4.
This repository retains the **PS3 BLES01857 01.24 / RPCS3** implementation and its
verified research; earlier PS3 01.00 work remains history.
**The full game is not yet playable.**

Source and engineering notes: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
Private research inputs: [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).
The repositories were renamed on **2026-10-06**. Existing local checkout/runtime
folders may still have their earlier names; use explicit paths for imported runtimes.

## Current state - 2026-10-11

PS4 setup follows the supplied project's `BOOTSTRAP.md`, `CLAUDE.md` and status
documents. The local US base package reports APP_VER 01.00; the eight CUSA00219
01.29 update chunks are now merged with a verified full read-back hash. The pinned,
decrypted 01.29 executable and effective content directory are still missing, and
the console is currently unavailable with GoldHEN FTP. The local firewall,
redirects and 37 firmware-module hashes pass a fresh configuration check. Responder
controls pass; shadNet has 32 passes and one explicit skip. No PS4 native gameplay
or successful Windows/WSL launcher is claimed. See the
[PS4 transition checkpoint](notes/ps4-transition-2026-10-11.md) and
[original setup helpers](tools/ps4_setup/README.md).

The table below is the retained PS3 checkpoint from **2026-10-10**, not PS4 acceptance.

| Area | Verified scope | Remaining work |
| --- | --- | --- |
| Local sign-in and characters | 01.24 signs in, persists a stored character and setup answers, and joins the local activity host. | This uses local stand-ins; complete account/profile semantics and broader character coverage are incomplete. |
| Opening mission | The revival hands over on native cinematic-end incidents; the Wall and rifle objective diamonds have native evidence. Automatic Breach dialogue and prop activation open the rifle barrier. Since JOIN228, the mission announces the rifle drop and an unarmed Guardian can claim it through the native pickup path. JOIN232's mission-spawned Dregs attack, kill the Guardian and allow respawn. JOIN252 suppresses unwanted urgent notices; JOIN253 shows the Wall objective handover without the extra mission banner. | First person remains too dark; usable room lighting and companion Ghost glow are unaccepted. Enemy kills, original encounter placement, ammunition fidelity, later encounters, completion and Tower remain open. JOIN253 loses its activity connection and rejoins; post-grant banner acceptance is incomplete. Nav input after revival causes a guest access violation in JOIN197–199. |
| Guardian and inventory | Arrangement indexes and fixed equipment bucket ranges repair missing armour and misplaced item records. Bounded runs resolve five armour slots, Ghost and subclass icons. A matching service-601 rifle claim clears pending loot, equips the Khvostov and publishes self/peer updates; JOIN226/228 show first-person firing. With the player database enabled, the handler records a durable item instance with a retry key. Database grant retries and item moves enforce account/character ownership. | The stand-in covers one rifle definition; magazine 12 differs from the reference's 25/184. Native reconnect/reload inventory acceptance and broader loot/equipment semantics remain incomplete. Texture corruption/distortion, level/light values and stable combined rendering remain open. |
| Ghost | The starter Generalist Shell now uses its art record's world appearance (row 0) and dye 15/2493. JOIN201 shows the authored shell fly in through the car window and turn to camera, matching the reference. Automatic Breach squad activation and dialogue have bounded native evidence. | JOIN202 repeats the revival through the default, including the resurrection sphere and first-person Ghost, and the Breach torch follows the camera. JOIN201 exited once on the resurrection light. First-person brightness, held Ghost, the power beat and combined graphics remain unaccepted. See [appearance evidence](notes/ghost-appearance-0124.md). |
| Vulkan stability | Actual-source CPU checks and bounded native comparisons support queue serialization and matching query scopes. GC and fence cleanup contracts have further CPU checks. | Descriptor-pool resets and depth/stencil layout errors remain under investigation. A longer run without device loss does not establish a graphics fix. |
| Automated evidence capture | Native game/audio recording, event deduplication, durable outbox, schema-1 packages, full decoding, hashes and automatic verified transfer have live acceptance. The capture suite passes 82 tests; 11 actual-controller checks cover pending source retention. Finalized originals now remain locally available after rolling buffer expiration. | Rotation gaps and intermittent native source-identification failures remain explicit. Background start/stop still needs a reliable native control path. Encounter/reward/multiplayer milestones require further reliable signals. |
| Broader content and multiplayer | Static package/scenario research and local protocol experiments provide a basis for later work. | Complete destinations, activities, AI, combat, progression, vendors, rewards, fireteams and multiplayer are not accepted. |

The current backend and diagnostic changes pass **225 server tests** and **176 script tests**,
rerun on 10 October. Earlier actual final-schema preflight passes cover 1 timed / 58
reactive changes; that preflight was not rerun for the October 10 evidence handoff.
The [rifle investigation](notes/rifle-pickup-0124.md) corrects the cue's object
selection: the previous chest was about 137 units from the marker; `o_weapon`
matches it. JOIN209 verifies native creation and occupied unit-pool membership;
JOIN211 verifies automatic barrier opening. Earlier unarmed runs could reach the
chest without acquiring a rifle; later runs accept the native pickup described
below. The [loot mapping](notes/loot-and-rewards.md)
now identifies the separate pending-loot table and nonce/position cache, with a
read-only probe. JOIN217 verifies that the native manager reads diagnostic rifle
records, but their nonce/position cache is empty and approach produces no claim.
JOIN218 identifies the native incident catalog and corrects the scripting-resource
address lookup. A bounded offline reader parses its 3,083 records; one selector
matches an observed cinematic callback. JOIN219 identifies the selector-2 body
codec and a missing seven-bit null dynamic-reference field. JOIN220's explicit
synthetic event decodes to the intended nonce/position/world and reaches native
queue submission. JOIN221–223 follow it through execution and dispatch. With the
recipient player ID and the Breach world set, the handler records the nonce, the rifle
unit is allocated, and walking onto it sends a live service-601 claim. The stand-in now
grants a matching claim (JOIN224–226): the acquisition notification appears, the pending
record clears, and after the self and peer updates the Khvostov is in first person and
fires. Since JOIN228 the mission script announces the drop itself and the stand-in owns
the pending record (`--world-loot`), with no private inputs; see the
[rifle pickup record](notes/rifle-pickup-0124.md#join221226-native-pickup-and-service-601-grant-synthetic-event). See [incident diagnostics](notes/incident-diagnostics.md#join220-synthetic-body-decodes-queue-completion-unverified).
Visible room illumination and companion
Ghost glow remain unaccepted.

The [equipment appearance support](notes/ghost-appearance-0124.md) corrects
the earlier claim that an empty-looking Ghost descriptor proved a missing lookup:
the base unit and cache can be valid while the image remains wrong. The private
field-combination runs reproduce oversized geometry, dark/green first-person
output and a Nav guest access violation. JOIN200 directly captures the null
component pointer before the crash; its temporary default is withdrawn.
**Neither cinematics nor the joint
flicker/distortion fix passes acceptance.**

The [October 10 evidence review](notes/checkpoint-2026-10-10-mission.md),
[October 9 checkpoint](notes/checkpoint-2026-10-09-mission.md),
[October 8 checkpoint](notes/checkpoint-2026-10-08-mission.md),
[October 7 renderer checkpoint](notes/checkpoint-2026-10-07-renderer.md) and
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
A [filtered native entry trace](notes/ppu-watch-filters.md) now preserves its
budget for the relevant state message, with 52 actual-source CPU checks passing,
including caller selection through the link register.
JOIN172 verifies the supported filters in the native client: calls reach the
serializer, but the original connection receives no state message after the
teleport request. The clock advances and the client reconnects without activating
112. Mission control and combined graphics remain unaccepted.
JOIN173 verifies the caller filters and actual shared return helpers in the native
client. Readiness checks pass and both outer layers return success for the
transition message, while the original host connection receives none after the
request. The direct branch reports success without testing later queue consumption.
JOIN174 then matches successful allocation with the transport consumer for the
same serialized transition record, while host receipt still fails. BAP forwarding
is the next check. No protocol or graphics repair is claimed by these diagnostics.

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
completeness, then polish. The [reference comparison](notes/guardian-rises-reference-review.md)
corrects earlier overbroad revival acceptance: the user reports missing shots,
Ghost and waypoints. Full-duration five-second visual sampling now records the
required sequence; this is not every-frame or full-audio review. Next: identify
the failing Nav component and the cinematic Ghost's actual model/transform;
then verify visible Ghost/torch and settled Breach progression. Obtain a matching outdoor
RSX capture while the owned client is live. JOIN178 saves a mission-frame capture,
but its replay does not match the lit original interior; capture/replay fidelity
needs diagnosis before using that replay as evidence of the live fault.
Continue correlating recurring descriptor resets with submission and
completion, and require unobstructed character/equipment, outdoor and mission
views together for graphics acceptance.

The latest backend change removes two flushed console writes for each exact
routine type-15/00 notification; all binary captures, structured events and
packet processing remain enabled. `--verbose-wire` restores those console lines.
JOIN175 records long waits around console output and no idle112. JOIN176 receives
the transition deltas, completes the teleport and remains in the original host
session through its bounded checkpoint. Its last inspected footage still has the
urgent dialog over outdoor scenery. Eight socket/session checks and the broader
88-check backend suite pass; an earlier timing-sensitive failure is retained in
the private record. This repairs the tested transition, not Guardian control,
combat or the combined graphics fault. JOIN177 repeats idle112 on the original
connection, clears both dialogs, consumes movement and renders a full armoured
Titan in the character menu. Earlier black/green outdoor shading remains visible;
later bright frames and a rendered menu do not pass combined graphics acceptance.
The native capture attempt misses the bounded deadline and is rejected before
creating a marker. See the
[session evidence](notes/session-transition-probe-0124.md#join175176-routine-console-backpressure--7-october-2026).

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

The obsolete Xbox 360 emulator route and tools are retired. One historical
research checkout remains privately pending removal; it is not part of the active
PS3 source or runtime.
The supported development route is RPCS3 on PS3 01.24. Earlier PS3 results remain
in the [test history](notes/test-log.md).
