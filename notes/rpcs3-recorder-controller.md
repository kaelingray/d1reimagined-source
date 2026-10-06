# Native recorder controller — October 6

JOIN156 exposes an incorrect controller transition. A source starts at
18:56:23 UTC. The controller posts its stop shortcut at 18:56:53, times out
after 15 seconds, forgets that still-open source and immediately posts a new
start. No new source appears; its cleanup posts another toggle. The original
file keeps growing until 19:01:54. Successful Windows message posts do not
establish that RPCS3 consumed those shortcuts.

The controller now retains the identified source until a read handle proves
the writer released it. It posts one stop request, polls without further
toggles and starts the next segment only after closure. Unknown recording
state receives no cleanup toggle. An atomic `.pending` receipt survives a
controller exit; the existing worker promotes it only after acquiring the
controller lock and proving the source is closed. Full decode and source/copy
hash checks still precede indexing. Interrupted originals remain preserved.

Stop-request and observed-closure timestamps are separate metadata. Available
coverage is capped by decoded duration rather than assuming a delayed stop
request ended the recording. Rotation and missing-source gaps remain explicit.

Validation:

- `pwsh -NoProfile -File tools/content_capture/Test-NativeRecorder.ps1`: **11
  checks PASS**, extracting the actual controller functions and holding a real
  Windows writer open. Covers repeated polling/finally, exactly one stop, retained
  source, eventual closure, durable metadata and unknown-source cleanup.
- `python -m unittest discover -s tools/content_capture -p 'test_*.py' -q`:
  **81 tests PASS**. Four new cases cover controller-lock contention, unavailable
  writers without premature decode, delayed recovery/coverage and interrupted
  receipt publication without duplicate indexing.
- JOIN157 uses the updated controller. Its start shortcut posts successfully
  but no identifiable source appears by the deadline. It exits without the old
  blind cleanup toggle. This verifies that failure path; normal native rotation
  and delayed native receipt recovery with the new controller are **NOT RUN**.

The current recording shortcut depends on an active game window. The existing
screenshot helper already documents this constraint for window shortcuts and
uses a native control file for background screenshots. That provides a concrete
next approach for recording. The earlier missing shortcuts are consistent with
this constraint; they do not prove a native GUI deadlock. Reliable background
start/stop and continuous footage coverage remain open.
