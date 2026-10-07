# Bounded descriptor fault trace — 2026-10-07

JOIN160 reproduces reset-00313 with descriptor text tracing off. JOIN159 does not
reproduce it while heavy tracing is active; run variation and logging overhead
prevent a causal conclusion. The next experiment uses a bounded memory trace to
retain the lead-up to an actual validation callback.

[rpcs3-descriptor-lifetime-ring.patch](../patches/rpcs3-descriptor-lifetime-ring.patch)
adds an opt-in ring, disabled unless `d1/d1descriptorring` is present. It stores the
newest **65,536** fixed-size records: command tags, physical queue submissions,
observed fence completions, pool retirement, assigned GC scopes, collection and
pool reset. A real descriptor-reset validation callback freezes the suffix and
dumps it once. Normal recording produces no trace lines. Later writes and later
callback dumps cannot overwrite the frozen evidence. The existing descriptor
text trace remains available separately.

This is diagnostic work, not a descriptor lifetime or graphics repair. It adds
host timing/locking overhead when armed, and does not establish performance
equivalence with an uninstrumented run. Queue submission records follow the
Vulkan call while the submit lock is held; another thread can observe a signalled
fence before that call returns. The record order must therefore be interpreted
with the actual operation and fence identity. Older overwritten history, descriptor
bindings and shader pixels are not recorded. A run that never reaches the callback
does not produce a frozen dump.

[rpcs3_descriptor_ring_audit.py](../scripts/rpcs3_descriptor_ring_audit.py) copies
the actual header unchanged into a standalone C++20 fixture. **20 checks pass**
for rollover, full-width payloads, concurrent writers, consistent suffixes and
one-shot freeze. Existing allocation and retirement regression checks pass
**20/20 and 23/23** with no-op mocks for the new diagnostics.

[rpcs3_descriptor_ring_decode.py](../scripts/rpcs3_descriptor_ring_decode.py)
requires a complete contiguous dump, preserves original host-time fields, reports
overwritten records and decodes signed submission results. It writes a private
JSON record and a text view compatible with the existing lifetime analyser.
Only the first callback following the frozen dump has captured lead-up; later
callbacks are counted as uncaptured. **10 parser tests pass**, including malformed,
truncated, duplicate and unknown-event rejection. No native timestamps are invented.

```powershell
python scripts/rpcs3_descriptor_ring_audit.py --source <private-rpcs3>/rpcs3/Emu/RSX/Common/descriptor_lifetime_ring.h --out <new-audit-directory>
# With MSVC configured, in the audit directory:
cl /nologo /std:c++20 /EHsc /W4 descriptor-ring-audit.cpp /Fe:descriptor-ring-audit.exe
.\descriptor-ring-audit.exe
python -m unittest discover -s tests -p test_descriptor_ring_decode.py -v
python scripts/rpcs3_descriptor_ring_decode.py --log <completed-private-native-log> --out <new-private-result-directory>
```

The scoped patch applies to the exact pre-ring private baseline and reproduces
all seven changed/added source files; unrelated renderer changes are excluded.
Patch SHA-256: `c93a2e69d170e1e927fefcbdffac8365dbff1af6e9f9d1eaca7ce79a3f704fc2`.
The full incremental MSVC x64 Release build passes, with warnings retained in
private evidence. Executable SHA-256:
`ef67aa5cf3c015de130cef958f1abfe345bd92a74964e2c20515e35e238f7645`.
It is deployed at idle with a verified backup of `4706856a`. JOIN161 starts with
the ring armed, text tracing off and core validation enabled, but **fails to reach
matchmaking**. It remains at character selection, with repeated screenshot/input
failures. The ring is disabled during the run as a diagnostic comparison; the
selection wait still has not progressed nearly two minutes later. That does not
establish the cause of the hang. No reset callback or frozen dump is obtained.
The test is closed at that safe menu checkpoint after its matchmaking deadline;
owned cleanup succeeds and its fixture-completed flag is false. It provides no
post-match conformance or combined graphics acceptance. See the
[October 7 checkpoint](checkpoint-2026-10-07-renderer.md) for combined acceptance.

JOIN162 uses the same build with the ring disabled from boot. It reaches
matchmaking and completes the 500-second fixture, but core validation reports
one reset-00313 callback at **8:11.023332** through **12:32.372609**. Outdoor
captures still alternate normal and corrupted shading, with a dialog obstructing
character inspection. No ring dump is expected in that OFF comparison. A separate
test arms only after a verified matchmaking reply; it must retain an actual
callback and complete trace before any lifetime cause is accepted. A single
selection success with the ring off does not explain JOIN161's stall.

JOIN163 arms after matchmaking but later stalls, obtains no reset callback/dump,
and fails its final screenshot. Its 500-second sampling interval therefore has
no accepted visual or conformance result. A matching-symbol host snapshot places
RSX in FIFO-empty/yield and the driver worker in its wake-event wait, without
establishing the reason for the stall. The next experiment delays arming for four
minutes after matchmaking and records pre-arm errors explicitly. See the
October 7 checkpoint for the repaired screenshot-child failure gate and tests.

JOIN164 completes a four-minute OFF interval after matchmaking and arms at
09:37:27 UTC. Its pre-arm snapshot has zero reset callbacks. The full sampling
interval and twenty later captures complete, with no validation callback or dump
through 13:20.013137. Outdoor frames still alternate normal and black/green.
The absence of a reset in this corrupted interval narrows the observed shading
failure without identifying its cause or resolving intermittent lifetime errors.
The next shading investigation uses [native RSX frame capture](rpcs3-native-rsx-capture.md).
