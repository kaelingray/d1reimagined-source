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
of `55efc907`. Original title/watch bytes remain intact. JOIN160 tests this
increment with core validation, the depth candidate enabled and descriptor tracing
off. Its completed result is recorded below.

## JOIN160: native failure reproduced on the cache increment

The full 500-second post-match fixture and eleven input/capture phases complete.
The core layer is verified live and immediately before shutdown. The final native
log reaches **12:37.486264**, with one
`VUID-vkResetDescriptorPool-descriptorPool-00313` callback at **3:17.903782**.
Queue-use, query-scope and device-loss counts remain zero. **Bounded conformance
fails.** The cache repair therefore has not resolved the observed native fault.
Descriptor tracing is off, so this run cannot correlate the failing command
buffer with its earlier submission or completion.

The game loads the configured keyboard pad, but posted inputs do not establish
consumed actions. Inspected native screenshots and sampled footage show loading,
an urgent-message dialog, contacting-server text and a **Rhinoceros reconnect
warning while the owned services are still running**. They do not establish
character, inventory, outdoor traversal or mission acceptance. A matching
`activity:in_world` label in memory does not override those visible results.
Original title/watch bytes, XInput and inactive controls are restored, all owned
processes close and shared RPCN is preserved without cleanup errors.

All **22 indexed originals** remain present with SHA-256 matching their indexing
receipts after rolling-buffer expiration. One earlier startup segment remains
unindexed with a full-decode/no-usable-duration failure; its original is preserved.
The last two untouched segments fully decode: **30.09 and 31.21 seconds**,
**1280×720**, game audio, rounded average/base rates **26.95/30 and 22.89/30 fps**.
These are media properties, not measured gameplay performance. Private sampled
frames contain native path overlays; no clean release or continuous fidelity
claim is made.

## JOIN161: incomplete selection comparison

The [bounded descriptor fault trace](rpcs3-descriptor-lifetime-ring.md) retains
recent lifetime records in memory and dumps only when a descriptor-reset callback
occurs. Its 20 concurrent-ring checks and 10 decoder tests pass; allocation and
retirement regression audits remain 20/20 and 23/23. The Release build passes and
is deployed at idle with a verified backup. Executable SHA-256:
`ef67aa5cf3c015de130cef958f1abfe345bd92a74964e2c20515e35e238f7645`.
JOIN161 is configured for the same 500-second Titan/world-113 fixture, the guarded
depth candidate and core validation, with text tracing off and the ring armed.
It instead remains at character selection and never receives a matchmaking
reply. The core layer is verified live. Screenshots time out and posted cursor/
Cross retries do not establish consumed actions. Disabling the ring at the safe
menu checkpoint does not restore observed progress nearly two minutes later;
the hang's cause remains unproved. No reset callback or frozen dump is obtained.

The existing runner prints its seven-minute matchmaking timeout but then enters
the post-match sampling loop anyway. That failure path now closes the exact owned
stack and throws before post-match sampling. The already loaded JOIN161 script
still uses the earlier behavior, so its unmatched run is explicitly stopped at
character selection. Cleanup verifies all owned processes closed, original title/
watch bytes, XInput, inactive controls and shared RPCN, with no errors. Its fixture
is **incomplete**, not passing conformance. This instrumentation is not a renderer
repair or a passing graphics claim.

The reused cleanup path passes **20 isolated shutdown checks** with real sleeping
child processes, including client-before-backend ordering, stale-identity rejection,
unrelated-process preservation and repeated-stop evidence retention. PowerShell
syntax passes. Those checks verify cleanup, not the missing-match branch in a
new native run or the underlying character-selection hang.

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

## JOIN162: trace-off fault and outdoor corruption reproduced

The same `ef67aa5c` diagnostic executable starts with the ring disabled. Core
validation is verified live and before shutdown; the guarded depth candidate
remains enabled. Stored-character selection and matchmaking now progress, unlike
JOIN161. That observation does not establish the selection stall's cause.

The 500-second post-match fixture and eleven later input/capture phases complete.
The final native log reaches **12:32.372609**, with one reset-00313 callback at
**8:11.023332** and zero queue-use/query-scope/device-loss entries. **Bounded
conformance fails.** The later callback also shows why testing only the earlier
three-minute failure window would miss this occurrence. Neither text tracing nor
the memory ring is armed, so there is no submission/completion correlation.

All eleven phase screenshots are inspected. The first three show loading; later
frames show the urgent-message dialog over outdoor scenery alternating normal
shading and black/green corruption. Character, equipment and traversal inputs are
obstructed and do not establish those actions. The combined rendering fix remains
**Known Broken**; a memory label still does not establish mission progress.

Cleanup at **08:52:52 UTC** verifies original title/watch bytes, XInput, inactive
controls, all owned processes closed, shared RPCN preserved and no errors.
All **23 indexed originals** remain present with their recorded SHA-256; one
startup segment remains unindexed with a decode failure and its original retained.
The penultimate original fully decodes: **30.14 seconds**, **1280×720**, game audio,
rounded average/base rates **9.42/30 fps**. The last original fails full decoding.
The recorder has already produced a recovered fragment copy, explicitly reporting
**118,084 discarded tail bytes**. That existing derivative is preserved unchanged,
hash-verified and fully decoded: **23.43 seconds**, **1280×720**, game audio,
rounded average/base rates **8.9/30 fps**. Original bytes remain untouched. Stream
rates do not establish gameplay performance; private sampled footage includes
native path overlays and further corrupted outdoor frames.

The next controlled comparison keeps the ring off during selection and arms it
only after this run's matchmaking reply, exact client identity and core layer are
verified. It retains the same baseline interval and eleven phases, then adds late
dialog dismissal/body/world checks because JOIN162's early Cross presses occurred
during loading. Those extra phases are separately identified; their result and a
captured lifetime fault remain unproved until the run completes.

## JOIN163: stalled rendering, no captured reset fault

JOIN163 keeps the ring off through character selection and arms it after verified
matchmaking at **08:57:44 UTC**. Core validation is verified at arming and before
shutdown. The 500-second sampling interval completes, but rendering stops making
useful progress and the final screenshot command fails. There is no reset
callback or frozen ring dump through native **12:15.839029**, and no queue/query
or device-loss entry. This stalled interval is **not passing conformance or
graphics acceptance**. None of the twenty intended later phases completes.

A private host dump is captured after the sampling interval, before owned
shutdown. Offline unwinding uses the matching executable/PDB and local image
unwind tables. At that instant the RSX thread yields in the FIFO-empty branch;
the Vulkan driver worker waits for its wake event. This does not identify why
the guest stops supplying frames or establish the trace as the stall's cause.
Dump capture can perturb timing and provides one host snapshot, not a continuous
comparison. Cleanup restores the original configuration and closes all owned
processes without errors. All dump/log/media inputs remain private.

All **11 indexed originals** remain present with their recorded hashes; **13
unindexed sources** retain their recorded decode/duration failures and original
bytes. The last two indexed sources fully decode (**30.25/30.19 seconds**, 1280×720,
game audio). Four sampled frames show an urgent dialog over dark scenery followed
by black loading views. They provide no accepted body, equipment or mission result.

The runner had ignored the failed native PowerShell screenshot child's exit
status and printed `done`; its caller then marked the baseline completed before
the later visual step failed. [Checked child execution](../scripts/Final124RunHelpers.ps1)
now propagates input/screenshot failures, and the join runner cleans up exact
owned processes before rethrowing. Failed final capture cannot reach `done`.
[Fourteen isolated checks](../scripts/Test-Final124JoinCapture.ps1) pass, including
actual first/final failure paths through the join runner with harmless sleeping
children and a mock service. All **20 existing shutdown checks** pass. These
test process boundaries and ownership, not native gameplay or renderer repair.

The next trace comparison retains a four-minute OFF interval after matchmaking
before arming. It records any reset callbacks preceding that arm separately;
they cannot be attributed to later retained events. The new failure gate also
requires successful native captures before fixture completion.
