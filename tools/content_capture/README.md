# D1 Reimagined — automatic development source capture

This subsystem supplies original game footage and context to the separate content
machine. It does not edit episodes, add captions/music, or publish. Game development
and acceptance remain authoritative; capture errors never fail a game launch.

Copy `config.example.json` to private `local/content-capture/config.json`. Set
`CONTENT_NAS_PATH`, a unique `DEVELOPMENT_MACHINE_ID`, local paths and `FFMPEG_PATH`.
Do not put credentials in this file. Use the existing Windows SMB connection. Paths
may be overridden by the documented environment keys in `capture.py`; NAS addresses
are not embedded in source. `D1_CONTENT_CONFIG` selects a different private config.

`scripts/Start-LocalStack.ps1` starts the independent worker when enabled configuration
exists. `scripts/Start-ContentCapture.ps1 -InstallStartup` also installs a current-user
Startup shortcut so queued transfers recover after login without starting a game.
The worker has an OS process lock and an exact process-identity receipt. No config
means no capture side effects. `-Stop` requests cooperative shutdown, preserving files.

## Verified state — 2026-10-06

The capture suite passes **77 tests**. Live acceptance includes native game/audio
segments, interrupted-fragment recovery, durable event/outbox handling, full decoding,
verified SMB transfer and atomic ready transitions. Status-lock contention is repaired
and tested with the live worker during recording. Source-start identification still
fails intermittently in some runs; diagnostics now record shortcut results, source
timing and cleanup. Do not claim uninterrupted coverage or a complete zero-touch
solution for every failure. Recorder failure preserves sources and does not stop
engineering. New metadata uses inspected media properties, including distinct rounded
average/base rates, instead of assuming the configured frame rate was achieved.

Current detectors are described below. Mission completion, AI, rewards and multiplayer
require reliable acceptance signals before automatic milestone classification can be
added. Missing footage and unverified tests remain explicit. Normal capture/transfer
is automatic; final editorial and publication review happens on the production machine.
See the [project status](../../README.md) and
[October 6 evidence](../../notes/checkpoint-2026-10-06-renderer.md).

## Normal operation

The worker finds ready run manifests and verifies the RPCS3 PID, executable, creation
time and 01.24 guest markers using the existing read-only reader. It starts RPCS3's
native game/audio recorder through the existing owned input helper. It rotates closed
segments locally (30 seconds by default), preserving 60 seconds before and 30 seconds
after an event. Buffer retention includes two extra segments and a byte cap. Rotation
can introduce short gaps, which metadata explicitly records; this is not a frame-exact
continuous replay buffer. It captures neither the desktop nor a microphone.

Completed native segments are fully decoded and copied with matching SHA-256 before
their redundant originals are removed. Only this recorder's identified files are
eligible. Selected segments are remuxed without re-encoding into immutable local source
packages; those packages are retained after delivery. Unselected buffer copies expire.
The controller refuses to toggle an already active native recorder. Client shutdown
asks the exact matching recorder to finalize before terminating the emulator.

The monitor keeps persistent capability observations per configured build/context,
seeds already known behavior from `KNOWN_CAPABILITIES`, and suppresses unchanged
observations. Current detectors cover world entry, renderer fatal/device loss, malformed
fragment programs, Vulkan validation errors, new default backend responses, native buffer validation and Git
revision changes. Default backend replies are labelled **Stubbed / Mocked**; engine
state is **Partially Working** evidence. Neither establishes playable gameplay. Absence
of errors is never treated as a crash fix. A failure followed by explicit passing
acceptance shares a comparison group; a mere observation cannot become `BUG_FIXED`.

Vulkan validation errors preserve up to three distinct normalized VUID identifiers
and their observed count. The native debug-report callback's explicit error format
is also recognized when its first line omits identifiers. Bounded continuation lines
can supply identifiers from the callback's specification links, including across log
reads. The next log entry or 64 continuation lines ends that context; it resets for
each owned run. Warnings and enabled settings do not trigger bug events.
Raw object handles, diagnostic context and host paths stay in the private log.
The event is **Known Broken** and does not identify the cause or claim a repair.
Device-loss detection requires renderer/Vulkan evidence; a fatal GDB socket message
during debugger teardown is not classified as a GPU failure.

Native exception exits are detected through the retained emulator handle even when
the native log has no fatal line. Known exception statuses are labelled **Known Broken**
under a separate process-exception feature; ordinary exits and forced-termination
codes do not prove a crash. A failed exit query preserves the live observation handle.
An intentional run-stop marker still takes precedence.

An observation tied to a run commits its capture intent in the same atomic project-state
write as the capability transition. The pending queue merges by event ID before
acknowledging that intent. Queue contention, failed acknowledgement or worker restart
therefore leaves a recoverable job, while unchanged capabilities remain deduplicated.
Queued intents retain their original build when configuration changes.

Additional systems can append explicit finished-clip events to private
`automatic-events.jsonl` or add evidence detectors in `observations()`. Test, encounter,
reward and multiplayer acceptance still need concrete project signals before they can
be detected reliably. The system does not infer those successes from arbitrary log text.

Validation, remuxing, screenshots and SMB transfer run on one worker thread separate
from monitoring and recording. The process uses Below Normal priority. Native recorder
encoding uses the configured RPCS3 encoder; no emulator encoding/settings are silently
changed. Hardware encoding can be configured there after a performance comparison.

## Package and transfer contract

```
incoming/<machine>/<year>/<month>/<day>/<unique-clip-id>.ready/
    clip.mp4
    metadata.json
    development-event.json
    notes.md
    screenshot-01.png
    screenshot-02.png
    hashes.json
```

Screenshots are optional frames from the footage, not designed thumbnails. Metadata
records actual timestamps, build, Git state, maturity, remaining issues, evidence,
comparison groups and the available pre/post window. Dev Blog number defaults to null;
the content machine groups episodes. SHA-256 and size cover every package file except
the hash manifest itself. The manifest is compared with the local original on transfer.

Each machine writes only into its own incoming folder. Copy goes to a unique
`.uploading` directory, verifies file count, sizes and all hashes, and only then renames
the directory to `.ready` in the same parent. **The receiver must ingest only `.ready`
directories.** Incomplete `.uploading` directories are deliberately ignored and retained
for diagnosis; a retry creates a new staging name. Recovery after a rename is idempotent.

Known secrets in metadata or text support are rejected locally. Automated game footage
has `contains_sensitive_information: true`, `requires_manual_review: true` and
`automatic_security_check.publication_review_required: true` because automated visual
secret detection is unavailable. It is transferred automatically for the content
machine to inspect, **and must be excluded from publication until safe review/redaction**.
Generic uncertain input stays in a local review hold. No raw emulator/backend logs or
personal identifiers are included automatically; evidence uses narrowly described facts.

Local receipts track waiting, transferring, verifying, submitted and failed-retrying
states, attempts, retry deadline and successful transfer time. NAS failures back off
exponentially and preserve timestamps/content. Local packages are retained 30 days by
default. `prune-submitted` is an explicit maintenance command and re-verifies the NAS
copy before removing an expired local package. When enabled by the private retention policy, the worker runs this maintenance
hourly while no owned game is active. Do not enable deletion without an agreed archive policy. Failed transfers and important originals
are never silently discarded. Inspect `machine-status.json`, `monitor.err`,
`native-recorder.log`, state receipts and `development-events.jsonl` for diagnostics.

## Override commands and tests

These controls are for overrides and integration; normal recording and transfer do not
require the user to invoke them.

```powershell
python tools/content_capture/capture.py --config <private-config> status
python tools/content_capture/capture.py --config <private-config> event DISCOVERED renderer "An evidence-backed discovery"
python tools/content_capture/capture.py --config <private-config> capture BUG "What happened" --video <closed-source.mp4> --recorded-at <timestamp-with-timezone>
python tools/content_capture/capture.py --config <private-config> sync --force
python -m unittest discover -s tools/content_capture -p 'test_*.py' -v
```

Tests isolate media packaging, secret filters, native-file closure, event classification,
failure/fix pairing, buffer bounds, duplicates, multiple machines, corrupted/interrupted
transfers, backoff, rename recovery, hash verification, safe retention and reading an
open Windows native log. Diagnostic tails drain bounded chunks without discarding
earlier failures or split lines. The canonical zero-local depth observation remains
Experimental and does not label renderer device loss fixed. Live native
recording and actual SMB delivery are separate acceptance checks; unit fixtures alone
do not establish capture performance or gameplay functionality.

Machine-status updates serialize the full read/merge/write across the observer,
transfer worker and CLI process. Atomic JSON replacement retries only known transient
Windows sharing/access errors, with a bounded 0.77-second delay; permanent failures
preserve the previous file and remain visible. Successful processing clears its stale
error. The five recovery fixtures include a real Windows reader holding the destination
open and 30 concurrent status updates from independent capture instances.

The current suite has **77 tests**, including callback continuations, native exception
classification, durable observation-to-capture handoff and interrupted-fragment recovery.
The [GPU checkpoint detector](../../notes/rpcs3-gpu-checkpoints.md) also preserves
normalized stage/operation hints from a bounded native device-loss report. Raw marker,
pipeline and command-buffer handles stay private. The event is Experimental and cannot
claim a renderer fix; unknown or expired records do not establish a command region.
DMA readback operation labels are recognized; resource handles and range details remain
private. Package importance uses the importer values `LOW_VALUE` and `REJECT`; the
older CLI spelling `LOW` is accepted and normalized to `LOW_VALUE` in new packages.
Existing submitted packages are immutable.

Engineering reports can use the same outbox, retries, payload verification and atomic
ready transition without requiring or duplicating video. The `report` command accepts
a closed Markdown report, an events-array JSON file and context metadata JSON. It
creates schema-1 `metadata.json`, `daily-report.md`, `events.json` and `hashes.json`.
Earlier media packages can be referenced by identity in the events/context. No media
decode check is claimed for a text-only report. Text is scanned again before transfer;
empty payloads are rejected. Reports stay private and require downstream publication
review. This text-only transfer exception cannot submit additional unreviewed footage.
Report metadata may supply `reporting_date` as `YYYY-MM-DD` for its destination date
folder; original event instants stay in UTC. Before transfer the worker records NAS
free space and required bytes. Insufficient capacity retains the durable package for
automatic retry and exposes `NAS_CAPACITY_ERROR` in machine status.
The [native recorder patch](../../patches/rpcs3-recording-fragments.patch) writes initial
MOV/MP4 metadata and bounded fragments. After a writer exits, the worker can recover
complete fragments into a private buffer copy; the unfinished original stays intact.
Strict full decoding and a SHA-256 comparison with the original prefix precede its
index receipt. Failure preserves retry eligibility. Regular unfinished recordings
without initial metadata are not recoverable through this path.

Recovery byte counts and source/prefix hashes follow the selected segments into package
metadata. Delayed imports use the recording end for their event timestamp; discarded
tail coverage remains excluded. Retried copies of one native segment are selected once,
preferring verified recovery provenance. Actual/truncated coverage stays explicit.
The [native acceptance](../../notes/rpcs3-recording-fragments.md) verifies interruption,
recovery and automatic NAS delivery separately from synthetic tests and gameplay.

Status-file contention is recoverable telemetry. Updates from the observer and its
transfer thread serialize within one Capture instance, then take the process lock.
If that lock or local status I/O is unavailable, the previous snapshot keeps its
original timestamp, the returned status indicates `STATUS_UPDATE_DEFERRED`, and
pending fields merge into the next successful write. Package/retry state remains
durable and separate. A status-reporting failure cannot terminate the observer's
retry loop. Three tests cover real lock contention, concurrent deferred fields and
failed status I/O. The live worker survives six-second held status locks both idle
and during native recording, then resumes fresh status updates. Its recorder also
survives the recording contention test. This repairs an observed worker exit;
it does not claim uninterrupted coverage for the earlier capture gap.

The native recorder appends timestamped JSON phases to its stderr log: start
request, shortcut result, source identification/missing deadline, stop request,
source finalization/timeout and any unidentified-source cleanup toggle. These
diagnostics preserve boolean input results and do not restart unknown recording
state automatically. A source file appearing later is not a decoded segment
receipt. Failed runs keep their originals; missing coverage stays explicit until
the actual source is verified and associated with the owned run.

Every newly validated buffer segment and video package includes inspected
`video_properties`: pixel width/height and distinct reported average/base frame
rates. These values reuse the existing full-decode input description, adding no
second encoding or probe job. Missing properties remain null. The rates are
rounded banner values: FFmpeg prints `avg_frame_rate` as `fps` and `r_frame_rate`
as `tbr`, as shown in its [stream inspection implementation](https://ffmpeg.org/doxygen/trunk/dump_8c_source.html).
They do not establish constant frame pacing or gameplay performance. Decode
failure cannot receive passing media metadata, and caller-supplied metadata cannot
replace inspected properties. Earlier ready packages remain immutable.
