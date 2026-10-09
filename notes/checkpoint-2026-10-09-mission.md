# 9 October 2026 — rifle incident decoding

The next missing rifle prerequisite is now narrowed to the native event queue.
JOIN219 identifies the selector-2 body codec; JOIN220 confirms that a synthetic
host event decodes to the intended nonce, position and world ID before reaching
queue submission. Rifle pickup, combat and the full game remain incomplete.

The investment encoder previously consumed zero bits for dynamic-reference
kind `0x19`. The null case actually requires a six-bit primitive selector and
one absent-handle bit. Encoding and decoding now preserve those seven bits;
unsupported present references fail explicitly. Five independent wire-fixture
tests cover alignment, nullable aliases, invalid selectors and truncation.
The private control file also gains an explicit bounded host-incident command,
with five tests covering envelope bits, bounds and complete-line consumption.
No automatic rifle event or production mission change is introduced.

JOIN220 receives both 76-byte envelopes through the host connection. Its second
event reaches native queue submission with the expected decoded body, but no
queue-commit watch is installed. The loot cache stays empty and the loot/pickup
callbacks do not fire. This is decoding acceptance for a synthetic fixture,
not an authentic reward, pickup or durable grant. Detailed addresses and limits
are in [incident diagnostics](incident-diagnostics.md#join220-synthetic-body-decodes-queue-completion-unverified).

The ordinary route exhausts its budget at the initial position. The inspected
image is black with the early Level Up coach. Movement/combined rendering are
not accepted in this run, and their relationship to the event test is unknown.
The renderer executable and production mission inputs are unchanged.

JOIN219 closes at 22:47:17.519 UTC and JOIN220 at 23:08:03.260 UTC on **8 October**;
the latter is 00:08:03.260 UK on **9 October**. Each independent closure check
verifies nine exact identities closed and all three baseline configuration
hashes restored. Private receipts retain actual timestamps, source state and
hashed evidence. The previously ready October 8 reporting package stays immutable;
this late evidence belongs in the next consolidated report with its original dates.

Validation: **212 server tests**, **176 script tests**, and the actual final-schema
preflight with **1 timed / 58 reactive** changes pass. Raw game inputs, native
metadata, traces, fixtures and production material remain private. Next: trace
the queue gate, command serialization and commit; then verify the cache callback,
native pickup request and durable replicated grant. Lighting, Ghost brightness,
character distortion/flicker, Nav crash, banners, combat and mission completion
remain open.
