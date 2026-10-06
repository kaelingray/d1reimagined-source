# Automatic source capture acceptance — 2026-10-05

The development machine now supplies source material to the separate content machine.
Final episode editing, narration, captions, rendering and publication are no longer
this machine's normal workflow. Configuration and recordings stay private.

## Verified

- 22 isolated Python tests pass: file closure/decoding, metadata, SHA-256, interrupted
  and corrupt uploads, atomic readiness, backoff, duplicate handling, independent
  machines, secret rejection, review flags, retention, buffer bounds, observation
  suppression, build separation, before/after pairing, transient startup recovery,
  experimental depth observations and log tail sharing/backlog/partial-line handling.
- `Test-ContentCapture.ps1` passes eight owned-process checks. The native recorder
  drains while the client and backend remain alive; a stale sidecar cannot stop an
  unrelated process. Existing isolated run and join-shutdown suites also pass.
- JOIN127 supplied a real native game/audio stream. The first attempt exposed a
  temporary startup read being mistaken for process exit; the monitor now checks the
  process handle's exit status separately and preserves recording through transitions.
- The repeat generated closed segments, passed full decode and matching-copy hashes,
  detected the first native-buffer validation event, saved its available pre/post
  window, remuxed without re-encoding, extracted two source frames, generated context,
  transferred and verified every file, then renamed the NAS package to `.ready`.
- That automated clip is 93.29 seconds. The requested window started before recording
  was available; actual start/end and segmented-recording gaps are explicitly labelled.
  It contains the revival scene and urgent-message overlay. It demonstrates the capture
  pipeline, not new mission progression or a repaired renderer.
- Three original JOIN122/124/125 recordings were also fully decoded and transferred.
  They share the inventory comparison group and specify useful source windows. These
  are native originals, retaining higher-quality source footage than browser exports.
- Four packages were verified on the user's 500 GB share in this machine's incoming
  folder. Local packages remain retained. No package is a completed-playthrough claim.
- JOIN127, its proxy, launcher and recorder exited by exact identity. XInput was
  restored. The independent idle transfer worker remains enabled and has a current-user
  Startup shortcut for automatic retry recovery. Below Normal priority was verified.

## Limits and next work

Current automatic observations cover world entry, selected renderer failures, new
default backend responses, native buffer validation, experimental canonical depth
draws and repository revisions. They do
not claim arbitrary mission, combat, inventory UI or multiplayer acceptance. Those
detectors need authoritative signals as the corresponding gameplay systems develop.
Native rotation can produce small gaps, and performance impact has not been benchmarked
across devices. Hardware encoding was not silently enabled or changed.

Automated footage is flagged as potentially sensitive and requires downstream
publication review. Text is scanned automatically; visual secret detection is not
implemented. Raw account/network logs are not submitted automatically.

JOIN128/129 exposed an integration gap: ordinary Python file opening could not read
RPCS3's live Windows log, so renderer diagnostics were silently unavailable to the
observer even while native video segments were recording. The shared reader now uses
read/write/delete sharing, also reused by the route observer. Its regression fixture
keeps a Windows writer open during the read. Bounded chunks drain in order instead
of dropping an earlier brief failure in a large tail; incomplete lines are retained.
Tail offsets reset for a new owned run. Successful memory samples clear old transition
errors, and repository polling is limited to once per minute so it does not delay
every two-second diagnostic tick. Failed records remain preserved locally.

JOIN130 then supplies the live acceptance for this repair: the worker automatically
detects native Vulkan device loss, records a Known Broken BEFORE event, selects its
buffer, creates a 74.65-second game/audio source package and two source frames, and
submits it to the same 500 GB share. No manual clip selection, naming, metadata entry
or transfer command is used. The available post-event footage is truncated by the
failed renderer and labelled accordingly. Five packages are now submitted, with local
copies retained. The owned test stack is fully exited and the worker remains idle
and healthy for future recording/retries.

JOIN131 later adds two verified packages, bringing the total to **seven**. The
Experimental canonical-depth observation automatically preserves a 60.55-second
outdoor source window. A native visual observation from the development observer
then supplies the persistent character-preview leg corruption and its observed
timestamp to the same detector/pending queue. The worker selects its existing
buffer, remuxes a 121.32-second Known Broken BEFORE clip, extracts two frames and
verifies its NAS package. Both extracted frames visibly show the texture error;
one also shows a live "Contacting Destiny Servers" warning. This visual observation
is an engineering signal, not a general automatic pixel classifier. Clip selection,
naming, packaging and transfer require no user action.

The normal capture configuration is restored after the temporary recording-disabled
renderer control. The worker is healthy and idle, local packages are retained, and
all seven packages contain verified source media/context/frame files plus hashes.
No package claims the graphics or networking issues are fully repaired.

A later diagnostic/status read exposed a transient Windows denial when replacing
`machine-status.json`. Atomic JSON writes now retry known sharing/access failures
for a bounded 0.77 seconds, preserving the prior file on permanent failure. A process
lock serializes status read/merge/write so observer and transfer updates do not lose
each other's fields. Successful processing clears the old processing error. All
**34 capture tests** pass, including five new recovery/concurrency checks and a real
Windows reader-held destination. The restarted normal worker is healthy and idle,
with no monitor or processing error; NAS delivery remains verified for seven packages.

The next increment detects explicit Vulkan validation errors as Known Broken BEFORE
events and retains only bounded, normalized VUID identifiers and error counts.
The native callback format is also recognized when its first line omits VUIDs. Five fixtures verify
deduplication, comparison pairing and private-context exclusion, and reject warnings,
configuration messages and unrelated errors. Two further fixtures distinguish an
intentional GDB teardown fatal from device loss reported on RSX, PPU or SPU threads.

JOIN136 supplies native acceptance for the validator detector. It observes the real
callback with an identifier-free first line, preserves a **112.64-second** original game/audio source
window and two frames, and automatically delivers the Known Broken BEFORE package.
All seven package files and six payload sizes/SHA-256 hashes are verified on the
500 GB share. **Eight packages** are now delivered. A brief processing access error
recovers and the pending queue drains; no user capture or transfer is required.
This establishes diagnostic capture, not a repaired renderer. The detailed
[descriptor investigation](rpcs3-descriptor-binding-cache.md) records its limitations.

The follow-up parser reads bounded callback continuation lines, which can contain
VUIDs in specification links. It carries context across reads and clears it at the
next log entry, after 64 continuation lines, or for a new owned run. Three new
fixtures cover normalized extraction, warning/unrelated-context exclusion and
bounded split reads. All **37 capture tests** pass. The already delivered JOIN136
package remains unchanged; its IDs were omitted by the previous first-line parser.

The [October 6 continuation](checkpoint-2026-10-06-renderer.md) adds native exception
exit detection and atomically retained observation-to-capture intents. All **42 tests**
pass, including recovery after queue/acknowledgement failures and restart. The ninth
NAS package supplies **60.42 seconds** of pre-crash game/audio context for the
identity-matched Windows validation-DLL exception. Its last complete source frame
predates the exception by 28.4 seconds; actual/truncated coverage is explicit.
All seven files and six payload hashes/sizes are independently verified. The worker
is active for the subsequent control and requires no user recording or transfer action.

Two Windows handle checks bring the suite to **44 passing tests**. A controlled child
exception-status exit verifies durable capture intent before handle closure; a failed
exit query preserves the live reader and recorder. The seventeen-minute JOIN140
control keeps capture running, then drains during ordered shutdown. Afterward the
normal worker is healthy and idle, with no errors or pending queue and nine verified
NAS deliveries. That run does not establish a settled Guardian or repaired graphics.

Keep the source-capture worker alongside development. The engineering priority remains
the [remaining GPU fault and preview texture](rpcs3-device-fault-detail.md), then
reliable Ghost illumination, mission transitions, weapon pickup and combat.

The [fragmented recording repair](rpcs3-recording-fragments.md) now covers interrupted
MOV/MP4 writers. All **58 tests** pass. Twelve checks of the actual native header block
pass, and both interrupted synthetic formats recover 120 clean frames with audio;
the baseline recovers none. JOIN145 separately verifies the rebuilt native recorder,
including its two B-frames: **688 clean frames / 23.83 seconds** survive deliberate
termination without a trailer. Recovery shortens only the private buffer copy, verifies
its exact prefix hash and preserves the unfinished original unchanged.

The worker records recovery evidence, uses the footage end for delayed observations,
deduplicates retried segments and includes recovery provenance in package metadata.
The first integration package repeated its final segment and is preserved as
`.superseded`, excluded from ready ingestion. Its replacement references that package
and automatically delivers **84.25 seconds**, three distinct segments and two source
frames. All seven files and six payload hashes/sizes are independently verified on
the configured 500 GB share. **Ten ready packages** are delivered. The queue is empty
and the worker remains healthy and idle. This intentional recording test establishes
source-capture recovery; GPU, character and mission acceptance remain open.
