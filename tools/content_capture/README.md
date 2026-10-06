# Automatic development source capture

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
copy before removing an expired local package. The automatic worker runs this maintenance
hourly while no owned game is active. Failed transfers and important originals
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

The current suite has **42 tests**, including callback continuations, native exception
classification and durable observation-to-capture handoff. An abrupt native crash can
leave the active media segment unfinished: only fully decoded source segments are
submitted, and the actual window plus missing post-event tail remain explicit in metadata.
