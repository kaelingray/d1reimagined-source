# Automatic source capture acceptance — 2026-10-05

The development machine now supplies source material to the separate content machine.
Final episode editing, narration, captions, rendering and publication are no longer
this machine's normal workflow. Configuration and recordings stay private.

## Verified

- 18 isolated Python tests pass: file closure/decoding, metadata, SHA-256, interrupted
  and corrupt uploads, atomic readiness, backoff, duplicate handling, independent
  machines, secret rejection, review flags, retention, buffer bounds, observation
  suppression, build separation, before/after pairing and transient startup reads.
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
default backend responses, native buffer validation and repository revisions. They do
not claim arbitrary mission, combat, inventory UI or multiplayer acceptance. Those
detectors need authoritative signals as the corresponding gameplay systems develop.
Native rotation can produce small gaps, and performance impact has not been benchmarked
across devices. Hardware encoding was not silently enabled or changed.

Automated footage is flagged as potentially sensitive and requires downstream
publication review. Text is scanned automatically; visual secret detection is not
implemented. Raw account/network logs are not submitted automatically.

Keep the source-capture worker alongside development. The engineering priority remains
the [explicit zero-local fragment binding](rpcs3-fragment-zero-binding.md), then reliable
Ghost illumination, mission transitions, weapon pickup and combat.
