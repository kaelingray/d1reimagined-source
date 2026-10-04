# 01.24 private host investigation — 3 October 2026

Target: BLES01857 01.24. These addresses were checked in the decrypted 01.24 ELF, rather
than carried forward from the 01.00 address table. Raw logs and binaries remain private.

## A verified proxy assignment bug

JOIN88 initially requested an activity host proxy with an eight-byte request ID of zero
at 15:29:50.584. The configured reply also began with zero, so the first assignment worked.
After the Breach transition failed, requests at 15:41:15.644 and 15:42:06.777 carried ID
one. The stand-in still returned ID zero. This is an actual correlation error in the
recovery path; its timing does not explain the preceding pumpkin failure.

The **01.24 consumer at `0x48ea44`**:

- requires a 16-byte assignment body (`0x48ea5c`);
- compares its first u64 against request state `+8` (`0x48ea78`, calling `0x1da944`);
- exits without accepting a reply with a different ID (`0x48ea80`);
- on a match, copies all 16 bytes, stores the address and port through `0x13446c`, and
  sets request state `+4` to 2 (`0x48eb18`).

The reply is therefore `echoed u64 request ID | IPv4 | u16 | port`. The outer BAP message
ID is a separate correlation field and also remains echoed in the response header.

`server/bap_server.py` now replaces the configured reply's first eight bytes with the
eight-byte request. The endpoint bytes remain unchanged. Exact lengths are required:
malformed request lengths or configured reply lengths preserve the existing raw experiment
behavior, without padding or truncation. Initial zero-ID recipes remain identical.

`server/test_bap_ahp.py` tests the real encrypted session with initial, nonzero, repeated
and high-bit IDs, endpoint preservation, and malformed inputs. All four tests pass.
Restoring the previous fixed-reply behavior makes the correlation regression fail.
This code change has not yet been verified by a new live rejoin.

## AHP client activation and connection state

The session's AHP client object is at session record `+0x100`. Activation at `0x48dddc`
registers two callbacks through `0xc352b8` into `*0x18dca10`, saves handles at object
`+0x44` and `+0x50`, saves its self pointer at `+0x4c`, and sets byte `+0x5d` to one.
It performs no UDP or socket call. `0x48df5c` returns this activation flag.

`0x48e0f4`, called directly at `0x33a148`, updates the assignment and BAP connection:

- assignment state is at object `+0x0c`; the desired u64 ID is at `+0x10`;
- absent state or a changed desired ID calls `0x48df64(object+8, desired_id)`;
- that routine sends an eight-byte request through `0x47676c` and leaves the assignment
  request pending (state 1);
- accepted assignment state 2 permits connection setup; object `+0x54 == 1` selects
  available connection slots 2–3, otherwise it uses slot 1;
- the selected connection slot is stored at `+0x58`;
- `0x48e274` checks whether its connection status is 3; `0x48e2a8` checks status 5.

These fields give a bounded, read-only trace a way to distinguish activation, assignment
and connection readiness. They do not by themselves establish which session slot was
responsible for JOIN88's failure, or the missing private-host message.

## UDP ENOTCONN is not a demonstrated startup failure

JOIN88's first relevant `ENOTCONN` entries preceded the activity host assignment: RPCS3
time 0:01:28.992 onward shows an unconnected UDP socket bound to port 3074. The client
continues using `sendto` to `127.0.0.2:3074`; the same metadata query repeats throughout
the run, including before the successful initial world and highway arrival.

The checked local RPCS3 source explains this specific log sequence:

1. `net_write_sockinfo` in `rpcs3/Emu/Cell/lv2/sys_net.cpp` asks `getpeername` for the
   socket's remote endpoint while filling the `infoctl(cmd=6)` result.
2. An unconnected UDP socket has no peer; the native helper reports `ENOTCONN`.
3. The metadata writer leaves the remote endpoint at zero and continues. This error is
   not propagated as the `infoctl` result: a valid query returns `CELL_OK`.

An adjacent `ENOTCONN` line with the `infoctl` caller and `remote=0.0.0.0:0` is therefore
insufficient evidence of a failed peer packet or a cause of pumpkin. A real transport
diagnosis would need a failing send/receive or a traced protocol readiness check.

## Remaining failure distinction

The captured cleanup reason is `0x4d`. The old 01.00 name
`private_activity_host_timed_out_initializing_ipahp` remains a naming inference until the
01.24 selecting state is captured. The checked 01.24 mapper has multiple session statuses
that map to this reason, so the numeric reason alone cannot select the stalled step.

The assignment-ID repair addresses subsequent proxy reassignment and must not be advertised
as a verified 112 → 24 transition fix.

## Missing teleport completion after a state-only client delta

The pending predicates at `0x1da250` and `0x1da2d8` inspect local callback state at context
`+0x18` and `+0x20`. The second is a teleport state machine, not a located AHP wire ACK:

- `0x1da1d4` passes the world-state request at context `+0x5080` and pending state `+0x20`
  through the registered interface's vtable slot `0x84`;
- the concrete secondary vtable is `0x13eca48`, whose slot points to `0x57c4b0`;
- that wrapper invokes `0x3adf8c` with the request and pending state;
- an active host request starts local phase 1 and records its timestamp;
- phase 1 posts the target through `0x35097c` once the local readiness gates pass;
- phase 2 advances when the engine target matches, through `0x350a8c` / `0x158e1c`;
- local phase 3 waits for **host request state 3** (`0x3ae0d8`). It then runs the completion
  action and clears pending state (`0x3ae10c`). An absent request does not satisfy this gate.

JOIN88 exposes a concrete server-side omission in that finishing handshake:

| Event | Log time |
|---|---|
| Client reports target 24 / `0x68935472` | 15:36:40.892 |
| Live control queues the host teleport | 15:38:20.477 |
| Host publishes teleport state 1 | 15:38:21.948 |
| Client reports state-only arrival 4 | 15:39:54.222 |

The original host only marked `teleport_targeted` when a later type-21 delta explicitly
included the target. Here the target arrived **before** the live teleport, and subsequent
reports carried only changed fields. The host therefore never published the finishing
state 3 for slice 24, despite seeing arrival state 4. Initial 113 → 112 worked because its
target report arrived after the scripted host request.

`server/bap_server.py` now retains explicit slice fields per connection and initializes a
queued teleport's target match from that retained state. A state-only arrival can finish a
matching request. An explicit target clear or change invalidates the match and cancels a
pending delayed finish; a new host connection starts with an empty cache.

Six encrypted-session tests in `server/test_bap_teleport_delta.py` cover the JOIN88 order,
the finishing state 3 and later request expiry, differing/cleared targets (including the
zero clear record versus real slice 0), connection reset, and delayed-finish cancellation.
The original handler fails the JOIN88 regression: it keeps sending state 1 after arrival.
All 60 `test_bap*.py` tests pass. The fix still requires a
fresh live traversal to demonstrate that the complete Breach handover succeeds.

## Baseline completeness and earlier reset gates

JOIN89 later remained at client current slice 24 / state 0 for more than six minutes,
but the view was black. A fresh native sample at `0x1df644` identified the replication
context and showed bit 3 set in both outer SIM masks `+0x844` and `+0x850`. These masks
do **not** uniquely identify a missing baseline: the updater has earlier gates, and the
waiting bit is only cleared when that update path runs for its corresponding bubble.

The private `baselines_highway_0124_all.txt` superset contains 2,311 entries across 31
group keys. Comparing it to the 01.24 catalog's exported baseline lists found no omitted
static baseline in the four groups relevant to this handover:

| Group tag | Group key | Exported baselines | Missing from superset |
|---|---|---:|---:|
| Breach `0x80cb8167` | `0xfc0e45a3` | 213 | 0 |
| Highway activity `0x80cb8165` | `0x992bb471` | 43 | 0 |
| Outdoor `0x80cb8ce5` | `0xecf574b5` | 35 | 0 |
| Bootstrap `0x80b4600f` | `0x4786c0e0` | 21 | 0 |

This comparison establishes the exported static coverage. It does not establish which
records the live client instantiated or whether additional dynamic records need a baseline.
The bounded JOIN89 watch at `0x1df4d0` produced no hits, so it did not identify a missing
object. A later owned-memory read encountered an empty context pool; its sample does not
establish the pool state when the paired masks were captured.

The checked final-build reset path is:

1. `0x1d6fc8` enumerates outer SIM instances from the table at `0x1a20bf0`. It only calls
   `0x1d8e14` / `0x1d8b6c` when SIM byte `+0x10` is nonzero. At `0x1d70a8`, native
   register `r10` is the outer SIM and `r7` is this byte, including inactive instances.
2. `0x1d8b6c` obtains the **actual resource bubble** through `0x14dcf4`. That routine
   derives it from the loaded scenario's slice at `+0x110c` using `0x158dbc`: index
   `-1` produces bubble `-1`; otherwise the bubble is `(index >> 3) & 0x3f`.
   At `0x1d8b98`, `r3` still holds this bubble, so the native register log can observe
   it without reading the unlogged `r27`. Client slice reports alone do not prove that
   this resource-side slice has changed.
3. An invalid resource bubble (`< 0` or `>= 128`) exits before baseline readiness.
   If neither the corresponding `+0x850` waiting bit nor `+0x858` bit 0 is set, it also
   exits before readiness. At `0x1d8be8`, `r3` is the embedded replication context
   (outer SIM `+0x18`) passed to the predicate.
4. `0x1df358` examines occupied records, using the pool's occupancy bitmap and `0x48`
   record stride. A pending-baseline byte at record `+0x15` reaches `0x1df4d0`, where
   `r30` still points to the complete record. The predicate fails only for such a
   record assigned to the current resource bubble, a negative bubble, or an out-of-range
   bubble; pending records for other valid bubbles are ignored.
5. Successful readiness runs the reset callbacks and clears the matching `+0x850`
   waiting bit at `0x1d8cbc`, leaving its `+0x844` bit set. Capturing the active flag and
   actual resource bubble is therefore necessary before interpreting paired set bits
   as an object-baseline failure.

These JOIN89 observations did not identify a specific baseline defect. JOIN92 later
provided the concrete record and framing evidence below.

## JOIN92: helper-object state-B bit omitted from baseline exports

JOIN92 reached the Breach entrance visibly, then a host teleport to already-current
slice 24 reloaded the view and deleted the local unit. The resulting idle view was
black. The live outer simulation was active (`+0x10 = 1`); an owned, read-only
snapshot found 249 occupied replication records, 119 awaiting a baseline, including
113 Breach records and six activity records. After the controlled inactive-slot
reset `3:1`, `0x1df4d0` identified the first readiness blocker as
`0x992bb471 / 0x20 / 7` (`obj_investigate`), state A `0x80802a1b`, no state B,
bubble `-1`, pending byte `+0x15 = 1`. This establishes a real baseline-readiness
failure in that run; it does not establish that every earlier black view had the
same cause.

Checking only pending records found no missing ID or B-flag mismatch. Checking
**all 249 instantiated records** exposed one incorrect export:

| Record | Live state A | Live state B | Previous exported baseline |
|---|---|---|---|
| `ho_ghostlight`, `0xfc0e45a3 / 0x18 / 101` | `0x8080065c` | `0x8080048a` | `0xfc0e45a3/0x18/101` (no B bit) |

The helper record itself had already cleared its pending byte. That does not make
its framing safe for the objects that follow it. In the final reader `0x1df644`,
record `+8 != 0xffffffff` requires a B-presence bit at `0x1df88c`, after the A
reset/presence bits. The stand-in's previous default baseline for this record had
only two body bits. The reader therefore consumes the next object's `1`
continuation bit as B presence and starts reading that following object as B state.
The per-object length protects the unknown-ID skip path; this known-record path
does not seek back to the declared body end. The defective helper entry is position
2,012 in the zero-based 2,311-entry superset, followed by the fly-to-gun marker,
triggers and other Breach records. This proves a malformed stream; the fresh live
run must still establish how much of the stalled handover it resolves.

Baseline-file order initially made the six pending activity records appear to
precede the defective helper. Actual wire order explains the apparent mismatch:
`bap_server.replace_object()` removes the previous entry and appends the changed
specification. Replaying JOIN92's 20 logged object changes against its initial
player/game-mode and default-baseline list puts the helper at wire-list index
1,999. **All 119 pending records follow it; none precede it.** In particular,
`obj_investigate` moves from initial index 1,745 to 2,307. The other five pending
activity records and the three changed Breach floor/wall squads likewise move
behind the helper. This is consistent with the malformed B framing blocking the
remaining stream; it does not justify a separate object-ordering fix. The private
`FINAL124_JOIN92-baseline-order-proof.json` retains the replayed change IDs,
positions and complete pending-record comparison.

The flag is independently declared by the actual component. In final entity tag
`0x80cb8244` (class `0x80800861`), the signed link at `+0xc` locates the root
component at `+0x9e4`. Relative to that component, its replicated ID is at `+0x24`,
state B at `+0x38`, and state A at `+0x3c`. They match the live record exactly.
The latest package `025c` chain contains seven kind-`0x18` helper definitions in
three groups; all declare A `0x8080065c` and B `0x8080048a`. This evidence corrects
the previous tentative statement that the HO asset alone did not establish a B
requirement. The disc's base `025c` package independently has ten helper definitions
with the same A/B declarations, including the same Breach helper ID, supporting
the shared table correction for both the base and final package readers.

`scripts/d1_pkg.py` now includes verified kind `0x18` in `KINDS_WITH_B`, which is
also used by `dump_mission_catalog.py`. The narrow correction preserves object IDs,
ordering and the existing non-replicated kinds. It adds the required **zero**
B-presence bit; it does not send a new B value or a guessed torch state. Two
synthetic regressions exercise baseline export and the following-object boundary
with final seven-bit kind IDs. Both fail against the previous kind table; all 16
package-tool tests pass with the correction.

Before the next join, the private superset's Breach helper entry was corrected to
`/1`; all 249 JOIN92 live records then matched its flags. Its five selected later
helper entries (`0xfe8501fa`, indices 30–34) were corrected too, using their actual
component declarations. The seventh definition belongs to an unselected group.
The private list retains exactly 2,311 unique IDs in the same order. Private proof
reports retain the before/after hashes and the exact changed IDs. Previous catalog
coverage checks compared two outputs of the same incomplete kind table, so they
could establish ID coverage while overlooking this framing defect.

## JOIN96: non-replicated AI IDs can resolve to metadata

After repairing the helper's B bit, JOIN96 reached the route endpoint and retained
correct fresh-delta teleport behavior. The slice-24 baseline reader nevertheless
faulted. The native log's `0x2785ec` is the reset function's LLVM block entry;
the faulting null dereference is `lwz r5,8(r5)` at `0x278674`. Reading the preserved
caller stack establishes the decoded object ID as `0xfc0e45a3 / 0x29 / 141`,
named `fa_e3_entry` in the final group. Register-bank values left by the LLVM hook
alone do not reliably establish the active state type.

The actual reader at `0x1df644` calls `0x27a1f8`, which resolves the ID through
`0x155c50` and reads runtime-object `+0x104` through `0x20f368`. For a replicated
helper this field resolves to an occupied 0x48-byte pool record with the matching
ID and A/B types. The firing-area lookup instead resolves to metadata for tag
`0x80cb8214`, component class `0x808007cf`. Its first words are a tag handle,
definition offset and class handle, not a replicated object ID. Treating them as
a record makes state A zero and state B a component class; the default-reset path
then reads a null descriptor. The metadata's negative value at purported bubble
`+0x3c` also lets it pass the reader's bubble comparison.

Owned registry samples independently establish the related kinds:

| Kind / sampled index | Registered runtime entry | Replication consequence |
| --- | --- | --- |
| `0x18 / 101` | HO component; `+0x104` resolves to its actual pool record | Retain, including state B. |
| `0x29 / 141` | AI entry, class `0x80800548`; field resolves to definition metadata | Exclude firing-area definitions. |
| `0x2a / 183` | Same AI entry with its own definition offset | Exclude firing-area-set definitions. |
| `0x2d / 181` | Position-set definition, class `0x80800962`; field resolves to another definition | Exclude position-set definitions. |
| `0x2b / 198` | Registry handle `0xffffffff`, no runtime object | This observation alone does not change its classification. |

The 249 occupied records contain none of the three excluded kinds. All of their
actual B declarations match the corrected private baseline list, but 32 records
remain pending: 26 Breach and six activity records. The host's old assumption that
every non-replicated ID would take the unknown-ID skip branch is therefore false:
some resolve successfully to runtime metadata and cannot safely be included in a
superset. No emulator null-pointer workaround or guest-memory write is used.

`d1_pkg.KINDS_NOT_REPLICATED` now also includes `0x29`, `0x2a` and `0x2d`, shared
with the catalog exporter. The new regression interleaves them with a helper,
prop and trigger and checks that retained IDs keep their original indexes and
B flags. The private 2,311-entry list loses 345 firing areas, 507 firing-area sets
and four position sets. Its 1,455 retained entries are unique, preserve relative
order, keep all 31 groups and still cover all 249 observed records. Private stack,
registry and before/after reports retain the exact evidence. JOIN97 tests the
result in a new client; pending-record clearance and visible playability remain
separate live checks.
