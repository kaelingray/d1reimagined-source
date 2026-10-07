# Renderer checkpoint — 2026-10-07

The combined character, outdoor and mission graphics repair remains **Known
Broken / unaccepted**. The preceding [October 6 checkpoint](checkpoint-2026-10-06-renderer.md)
and [handoff](HANDOFF-2026-10-06.md) retain the original event dates and build results.

## Late October 6 evidence: JOIN158

JOIN158 uses the attachment-layout build
`55efc9072019124be3e880ca8c881339d057390c47d3bf77c799ccb6de920259`
with the guarded depth-only candidate enabled. The 01.24 Titan/world-113 fixture
completes its 500-second post-match interval and seven later input/screenshot
phases. The final native log reaches **12:02.875802**, with no queue-use,
query-scope or device-loss entries. The verified core validation layer reports
**two** `VUID-vkResetDescriptorPool-descriptorPool-00313` callbacks at
**3:39.734023** and **3:40.279613**. Bounded conformance therefore fails.

The fragment binding sample contains 400 records, including 12 eligible
zero-local depth/stencil draws using the guarded END/NOP program. None has changed
parse bytes or the known float prefix. No live malformed fragment programs occur
in the sampled interval; cached rejection messages are a separate observation.
This establishes candidate selection, not original hardware behavior or fidelity.

The lifetime trace is armed after the two errors. It reaches its 262,144-record
cap, but neither failing command buffer has a pre-error submission record. The
errors cannot be attributed to a submission/completion ordering from that trace.

Inspected final outdoor images alternate normal shading and black/green corruption.
The message-of-the-day dialog remains visible through character and inventory
input phases. Those phases do not establish a character screen, equipment view,
movement or mission progress. All owned processes close; original title/watch
bytes, XInput and inactive diagnostic controls are restored without cleanup errors.

## October 7 allocator repair

The [descriptor allocation cache increment](rpcs3-descriptor-allocation-cache.md)
repairs cached handles surviving a pool switch. Actual-source CPU fixtures pass
**20/20**, against **five failures** in the original implementation. The existing
retirement checks pass **23/23**. Source causality for the native failure is
unproved; current graphics tables use a single layout per pool.

The full incremental MSVC x64 Release build passes. The scoped patch reproduces
the exact changed source in an isolated fixture. Executable SHA-256:
`4706856a73901b1555d03af7a4f1f854b4a2950fbded89d9f51b140ec71f94a5`.
It is deployed after the previous native client closes, with a byte-verified backup
of `55efc907`. Original title/watch bytes remain intact. JOIN160 is testing this
increment with core validation, the depth candidate enabled, descriptor tracing
off, longer inputs and two repeated character captures. Its native result is pending.

## JOIN159: unobstructed inspection on the preceding build

JOIN159 still uses **55efc907**, so it does **not** test the cache increment. It
completes the same 500-second post-match interval and nine later input phases.
Core validation is verified live and at the final snapshot. Its final native log
reaches **14:06.829486** with no validation IDs, queue/query/descriptor-reset
callbacks or device-loss entries. This is bounded passing conformance, not a
repair of the preceding intermittent failures.

Descriptor tracing starts 120 seconds after the exact client launches and covers
the earlier failure window. It reaches its cap without reproducing an error.
Trace overhead and run-to-run variation prevent treating that absence as causality.

800-ms Cross inputs clear the urgent and message-of-the-day dialogs. Start opens
the character screen, R1 opens inventory, L1 returns to character and Start
returns to the outdoor scene. The returned Titan has normal proportions and
armour shading in the inspected view; five armour icons, Ghost and subclass are
visible. The inventory materials listing is empty in the sampled view. Initial
character loading, level/light zero, weapon-slot spinners and tutorial overlays
remain. A posted movement input is followed by a changed camera view, but these
snapshots do not accept traversal, combat, mission objectives or completion.

The inspected final world frames have normal shading. That does not overturn
JOIN158's corrupted frames or establish sustained combined graphics acceptance.
Cleanup restores original title/watch bytes, XInput and inactive controls, closes
all owned processes and preserves shared RPCN without errors.

## Evidence retention

The earlier worker removes successfully indexed native originals, while its
rolling copies can expire. It now retains untouched finalized originals until
an archive policy is agreed. Rolling preview copies remain bounded. The capture
suite passes **82 tests**, including source preservation after buffer expiration
and unchanged retry/idempotence behavior. The observer is restarted at idle.

A byte-identical copy of JOIN159's last native segment is preserved privately
before buffer rotation. Its original path was already removed by the preceding
worker. SHA-256 matches the earlier indexing proof; full decoding passes. It
contains **26.37 seconds**, game audio and **1280×720** video. FFmpeg reports
rounded average/base rates of **17.24 / 30 fps**; these are stream properties,
not a gameplay performance measurement. Sampled frames show the character,
inventory transition and world return. Account text and native path overlays
make it private diagnostic material. No editing, publication or new daily handoff
is performed; October 6's delivered package remains immutable.
