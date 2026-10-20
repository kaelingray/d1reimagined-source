# 01.24 incident and disconnect diagnostics

Updated 2026-10-09. These findings come from the private BLES01857 01.24 ELF, corrected
runtime type export, and the saved JOIN81–83 activity-host logs. They identify differences
between the runs; they do not establish the client error that produces `pumpkin`.
Raw game data, logs, type exports and generated reports remain outside the tracked repository.

## Reproduce the offline comparison

`scripts/d1_incident_summary.py` streams an AHL log and decodes only client AH type 18 with
the supplied runtime schema. The measured incident-state handle is `0x8080097d`; its fields
at offsets 0, 104 and 212 hold the type chain, second list and opaque byte body respectively.
Use the corrected final-build export, not the 01.00 investment schema.

```powershell
python scripts/d1_incident_summary.py PRIVATE_JOIN83_AHL_LOG --schema PRIVATE_0124_TYPES_JSON --out PRIVATE_REPORT_JSON
```

`--out` must resolve outside the tracked repository or under ignored `local/`, matching the
catalog exporter. Reports contain IDs, counts, timestamps and SHA-256 digests, not raw bodies.
Console output is limited to 20 type IDs and chains by default (`--limit 1..100`). JSON groups,
line size, payload size and digest tracking are bounded; a capacity hit is reported explicitly
and distinct counts become lower bounds. No file is written without `--out`.

For a chain containing the same ID twice, **occurrences** counts both entries while
**messages** counts the containing packet once. Intervals are between containing messages.
The comparison after a leading u32 omits the first four opaque body bytes; it does not assign
those bytes a protocol meaning. Complete-body variants and that comparison answer different
questions. Repeated IDs, byte bodies or intervals do not prove acknowledgement or retransmission.

Focused synthetic verification:

```powershell
python -m unittest discover -s scripts -p test_d1_incident_summary.py -v
```

The nine checks cover repeated and growing chains, occurrence/message counts, malformed
length/hex/truncation/trailing data, excess array counts, oversized lines, bounded variants,
midnight and backward timestamps, wrong/recursive schemas, and output-path privacy.

## Captured differences

Times below are the local timestamps written by the AHL logger on 2026-10-02. Do not compare
them directly with UTC bootflow timestamps without converting the clock.

| Capture | Decoded type-18 messages | Incident 2665 | Entered-trigger stream | Activity socket reset |
| --- | ---: | ---: | --- | --- |
| `FINAL124_JOIN81-ahl.log` | 459 | 450 | 17:39:48.142–17:41:42.731; mean interval 0.255209 s | 17:41:42.913 |
| `FINAL124_JOIN82-ahl.log` | 291 | 286 | 18:04:12.631–18:05:29.079; mean interval 0.268239 s | 18:07:01.071 |
| `FINAL124_JOIN83-ahl.log` | 261 | **0** | Not reproduced | 18:31:25.721 |

All three logs decode without malformed incident records. JOIN81 and JOIN82's incident-2665
bodies are 49 bytes: 450 and 286 distinct complete bodies respectively, but exactly one
variant in each run after the first four bytes. This is inconsistent with describing the
captured complete byte bodies as identical retries. It still leaves open whether the leading
word denotes a fresh event, a resend envelope, or another counter.

JOIN82's type-15 traffic continues after the last 2665 report until 18:07:00.887. The socket
reset occurs **91.992 seconds after that report**. JOIN81 resets close to its last report.
The difference is important: neither a continuous trigger stream nor its absence alone
explains the disconnect from these captures.

JOIN83, with incident echo enabled, contains these different families:

| Incident ID | Messages containing ID | Occurrences | Opaque body length | Complete variants | Variants after leading u32 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 2120 | 108 | 108 | 53 | 108 | 2 |
| 734 | 103 | 103 | 53 | 103 | 2 |
| 469 | 50 | 50 | 76 | 50 | 1 |
| 2841 | 50 | 534 | 76 | 50 | 1 |
| 1623 | 50 | 534 | 76 | 50 | 1 |

The last three share a chain starting `[469, 2841, 1623]` at 18:21:12.961. Successive chains
append another `[2841, 1623]` pair, reaching 25 entries at 18:25:18.267. There are 39 messages
at that capacity, ending 18:31:03.333. Their body after the leading u32 remains constant.
This is compatible with repeated specialization or feedback during echoing; the original
packet meaning and the cause of the later reset are not established. The source echo branch
sends each incident back, even though it logs only the first echo. It is not a demonstrated ACK.

JOIN83 also sends 12,196 AH type-15 messages through 18:31:25.668. Its final saved bootflow
sample is 17:23:58.692 UTC (18:23:58 local), several minutes before the reset. Those samples
cannot supply the actual error reason at failure. JOIN83's lack of 2665 means its failure is
not an experiment proving or disproving an acknowledgement specifically for that incident.

## What the executable establishes

These addresses are for **01.24 only**:

- `0xe2e9ec` receives host type 18 and decodes the incident state through `0x2766b8`.
  Valid decoded state passes through an interface query at `0x1cadf8`, then to the event
  interface wrapper `0x12f2e8` when the activity gates pass. This is event delivery, not a
  located acknowledgement format. The generic decoder proceeds through `0x1db1a8` and
  `0x278d6c`; successful schema decoding alone does not establish the host semantics.
- `0xc32cd8` creates the named `network queued incident pool`: native records are `0x480`
  bytes, with a pool handle at `0x1c17e6c`. `0xc32dcc` ingests a native incident using its
  u32 at `+0x70`, a 64-slot queue, and a pending bit at record `+0x474`. It copies `0x470`
  bytes of event data. An already pending entry at that position is ignored.
- `0xc330b0` drains eligible queue entries, dispatches via `0x1cae2c`, clears the pending
  bit and frees the record. Its callers include `0x5addc0` and `0x721d10`. Queue ingestion
  and draining do not reveal a separate ACK packet or directly post a disconnect error.
- `0x5ae4dc` selects local versus per-player queued delivery. `0x5ae674` builds the named
  `send_incident` work item. This local native-event queue and the AH type-18 opaque body
  should not be treated as interchangeable layouts without tracing serialization.

The old 01.00 trigger update address `0xb2fb68` does **not** identify that routine in the
01.24 ELF. An unchanged numeric address points into unrelated code. Likewise the 01.00
private-host error-name table cannot prove the 01.24 meaning of a captured numeric reason.

## Next useful evidence

The immediate objective is to capture the actual failure reason and the state that selects it,
then reproduce the same traversal without incident echo. A precise read-only watch candidate
is `0x355d54`, the central bootflow error-posting routine. Caller `0x35fa7c` passes category
in r3 (`0x19`), reason in r4 and context in r5. A bounded watch must remain active through
the socket reset, not stop at an earlier steady-state bootflow snapshot.

For private-host reasons, `0x35dd14` selects a reason into the pointer in r7 from session and
controller states; the private controller fields include `+0x8fe8`/`+0x8fec`. Relevant reason
maps are at `0x1309ca0`, `0x1309ccc`, `0x1309cf8`, `0x1309d24`, `0x1309d9c`, `0x1309e14`
and `0x1309e8c`. Correlate a posted reason with the actual failing check and its target-build
mapping before attributing it to a host establishment, teleportation, wipe or incident timeout.

For event generation, compare successive producer inputs and the native incident `+0x70`
against the first opaque-body word. That trace can distinguish new generated events from
resend envelopes. A real ACK proposal needs a verified consumer and field format, not an
echo of the incoming type-18 state.

Squad spawning remains a separate acceptance gate: later code analysis identifies `+0x844`
and `+0x850` as transient roster-reset masks, with baseline-ready processing clearing only
the latter. Type 27 writes a separate activity-level value. Neither should be presented as
a verified direct simulation-authority setter. See [mission acceptance](guardian-rises-acceptance.md).

## JOIN218: native catalog and resource address correction

The 01.24 instruction `lis r3,0x18e` followed by a load with displacement
`-0x11c4` addresses **`0x18dee3c`**, not `0x18eee3c`. The earlier private
scripting-resource lookup was 64 KiB too high. Read-only native checks now resolve:

| Resource | Global address | Codec handle |
| --- | --- | --- |
| AH incident, type 18 | `0x18dedc4` | `0x8080097d` |
| AH script event, type 39 | `0x18dee3c` | `0x80800c0a` |
| Base script data | `0x18ded3c` | `0x808009a6` |
| AH script state, type 38 | `0x18ded40` | `0x808052e5` |

These codec types are already in the final schema. The wrong-address value
`0x80804c4c` is not evidence of a missing script-event codec. Resource metadata
points four bytes after the type-definition start; use the definition start when
interpreting that metadata.

Client incident interface `*(0x1a93300)` dispatches through handlers including
`0x5aea30` and `0x5aede4`. They can call the loot position-recording paths, but
are incident handlers rather than BAP reply callbacks. JOIN218 observes five
entries into `0x5aea30`. An inspected cinematic context has payload selector 17
at `+0x134` and body type `0x80800792`; it does not select the branch that calls
`0x2b279c`. The loot recording paths and service-601 entry have zero watch hits.
No host incident is injected in this run.

The interface's database getter `0x5aef50 -> 0x4696f4` identifies catalog tag
`0x816564aa`, content class `0x80800659`. Its array at `+0x10` has **3,083**
records of content class `0x808003d3`, stride 20. Each record exposes an identity
hash at +0, a field-relative definition link at +4, four still-unnamed u16 fields
at +8, and a payload selector byte at +16. Catalog index 2120 has hash
`0xaeca3822` and selector 17, matching the inspected native cinematic context.
This is a bounded correlation, not recovery of all incident meanings.

`scripts/d1_incident_catalog.py` reads an already extracted private catalog,
checks array and definition bounds, and writes an exclusive private JSON report:

```powershell
python scripts/d1_incident_catalog.py <private-extracted-catalog> `
    --out <new-private-report.json> --hash 0xaeca3822
```

Five checks cover independent record layout, signed field-relative links,
malformed headers, truncated/out-of-range definitions and output preservation.
The complete script suite passes **176 tests**. The actual private catalog parses
successfully. Extracted definitions, reports and native traces remain outside Git.
Content class hashes are not wire codec handles. Names, auxiliary-field meanings,
incident body encoding and the event that supplies the rifle nonce/position are
still unverified; this reader does not grant loot or send an incident.

JOIN218 closes cooperatively at **22:31:18.214 UTC on 8 October**. Independent
verification confirms nine exact owned identities closed and all three baseline
configuration hashes restored. No mission, renderer or inventory behaviour changes
in this run, and no new gameplay or graphics acceptance is claimed. Next: trace
the incoming incident's body decoding into the client action context, then verify
the native cache insertion and service-601 claim for the unarmed starter.

## JOIN219: incoming body selection

Read-only native checks verify the game interface at `0x1ab9d7c`, vtable
`0x13e7cf0`. Incoming wrapper `0x12f2e8` calls its `+0x1a0` method:
`0x2fcd64 -> 0x92c134 -> 0x2700a4`. The last routine looks up the first
catalog index, reads its payload selector and selects a body adapter through
`0x26b74c`. Selectors can override the generic adapter; selector 1 does so.
Selector 2 uses codec **`0x80800cb6`**, and selector 17 uses the already observed
cinematic codec `0x80800792`.

Catalog index 1504 has selector 2 and hash `0x7a0fd954`, registered to handler
`0x5aede4`. That handler passes its decoded body to `0x2b2a60`. The latter reads
the nonce at +0/+16, position at +96 and world ID at +112, matching the generic
selector-2 codec. This identifies a candidate cache-recording route; it does not
recover the original event name or the starter reward's identity.

JOIN219 has three entries each into the local incident dispatch, selector
preparation and cinematic handlers. Incoming-host, loot-recording and service-601
watches have zero hits. No incident is sent in this metadata run. It closes at
**22:47:17.519 UTC on 8 October**, with nine exact identities closed and all three
baseline hashes restored by independent checks.

The selector-2 body also exposes an encoder defect: nested field kind `0x19`
has width zero but consumes a six-bit primitive selector and a nullable handle.
The old encoder omitted it, shifting every later field. Native reader `0xe4524c`
delegates selectors `0x15`/`0x16`/`0x17` to the nullable-handle reader, and
`0xbee110` maps an absent handle to the null reference pair. The encoder now
supports **null only**, emitting selector `0x16` and one absent bit. Present
references and unknown selectors fail explicitly because their namespace/fixup
semantics are still unsupported. Five independent regression methods fail on the
previous implementation and pass with the repair; adjacent sentinel fields verify
the seven-bit alignment.

The existing private control file now accepts an explicit host incident:

```json
{"incident": {"types": [1504], "body": "<validated-private-body-hex>"}}
```

`encode_incident` validates the measured envelope schema, type-chain capacity,
integer ranges and body/count bounds. A complete valid line is sent once through
the existing host notification path; partial or invalid lines send nothing. The
body remains opaque to this debug control. It supplies no ACK semantics, automatic
mission event, reward authority or durable grant. Use only a body whose target-build
layout and intended experiment have been checked; the example is not a replayable
game payload. Five further tests cover an independent envelope bit fixture,
capacity/range rejection, count overflow and control consumption.

## JOIN220: synthetic body decodes; queue completion unverified

This bounded native test uses the same C5 executable and an unarmed starter with
one **synthetic** pending rifle record. Its nonce is diagnostic, not recovered
reward authority. The explicit selector-2 body has 570 significant bits (72
bytes); the complete incident envelope has 76 bytes. Two control lines reach
`0x12f2e8` and `0x2700a4` through the existing host connection.

The second event also reaches `0x27d744` with command type `0x19`. Its input
contains the intended nonce key/ID, a null reference pair, XYZ
`[410.644, -794.385, 17.723]`, homogeneous coordinate 1 and world ID 3. This is
independent native evidence that the seven-bit field repair preserves the body
layout. The live position quantizer is zero, agreeing with the raw-XYZ codec.

`0x27d744` is a **queue submission routine**. Entry alone does not establish
successful serialization, allocation, commit or later consumption. It checks
`0x12f9d4`, serializes the native command, allocates via `0x1f6ec8` and commits
via `0x1f6f88`; these inner stages are not watched in JOIN220. Earlier wording
that this run proved an accepted queued event is narrowed to submission.

Both post-event loot samples still have zero cached nonces. Handler `0x5aede4`,
recording paths `0x2b2a60`/`0x2b2f74` and pickup-service entry `0x472518` have
zero hits. No service-601 claim or weapon grant is accepted. The ordinary route
releases input at its budget without moving from the initial outdoor position;
the inspected view is black with the early Level Up coach. This run does not
reproduce JOIN217's successful rifle approach, and its cause remains open.

The dynamic reference name is intentionally broader than “tag reference”:
native classification also handles actor and other namespaces. Only the null
case is implemented. The source naming cleanup after this run does not change
the encoded bytes. JOIN220 closes at **23:08:03.260 UTC on 8 October**
(**00:08:03.260 UK on 9 October**), with nine exact identities closed, all three
configuration hashes restored, XInput and shared RPCN preserved, and no cleanup
errors or active diagnostic controls. Next: verify the queue gate, serialization
and commit before attributing the missing callback to dispatch or the event's
semantic identity.

## JOIN221: the event is committed, executed and dispatched; the handler filter rejects it

JOIN221 repeats JOIN220's private fixture (unarmed starter, one synthetic pending
rifle record, the same 76-byte selector-2 envelope) with no movement. The watch list
was re-armed between ten identical sends, each phase one stage further along the
path. Gate watches filtered only by return address are exhausted by unrelated
commands first. Filtering them to the incident frame (stack `0xd00fc960`; the
`0x27d744` frame is `0x10150` bytes) isolates the event.

Verified native path for catalog index 1504 (hash `0x7a0fd954`):

| Stage | Native evidence |
| --- | --- |
| Queue submission `0x27d744`, type `0x19` | `0x12f9d4` gate passes; serialize `0x1db184` succeeds (`0x2051` bits, `0x40b` bytes); `0xbee268` passes; allocation `0x1f6ec8` and copy `0x99e380` run; commit `0x1f6f88` is called |
| Commit | `0x27d4c4` routes type `0x19` to the second list; `0x27cbb8` appends it to the enabled list (`0x306169c0`) |
| Execution, about 84 ms later | list executor `0x1f6b34` (per-frame `0x32f0f0`) calls fallback `0x27d600`; type-`0x19` handler `0x104ad30` deserializes and calls `0x92c774` |
| Target lookup | `0x336708(1)` resolves object `0x6023e780`; delivery `0x1d97c8` passes both gates |
| Dispatch | dispatcher `+0x40` = `0x5ae4dc`; `0x1cae2c` reaches interface `*(0x1a93300)` method `0x5aee80`; role gate `0x1ca6a4` passes; `0x1cb0b0` calls generic handler `0x5aea30` |
| Per-event handler | `0x5ae7a4` finds registration 3 (`0x7a0fd954` → `0x5aede4`, schema `0x84f68b44`) and calls the check `0x269f10` → dispatcher `+0x28` = `0x5ae120` |
| Handler filter | delivery filter kind 1 names the local character `0x0a3d4a2c8fbea6ba`; `0x26a544` receives schema `0x84f68b44` and the body (selector 2, codec `0x80800cb6`, adapter `0x1a93330`, nine schema records with the generic `0x1124697d` first) |

`0x5aede4`, `0x2b2a60` and `0x472518` still have zero hits. The queue, executor,
target lookup and dispatch are therefore **not** the missing step. `0x26a544`
passes at once only for selector 0 or the generic schema. Otherwise it searches
the body's 84-byte schema records (`0x26b9b0`), expands recipients (`0x26a3dc`) and
matches each against the filter (`0x26a00c`). The selector-2 adapter vtable
(`0x13e7770`, slot 5 `0x28cbbc`) registers `0x84f68b44` within its schema family,
so the remaining candidates are a missing matching record or a recipient set that
excludes the local character. JOIN222 watches that evaluation directly.

The list's own enabled byte, the commit's second `0x12f9d4` check and the eight
other registrations are recorded privately. No grant, loot-cache entry or
service-601 claim is observed. JOIN221 closes at **07:44:58.8 UTC on 9 October**
(owned stop `0xffffffff`), with all seven exact identities closed, the three
configuration hashes restored, XInput and shared RPCN preserved, and no cleanup
errors or active controls.

## JOIN222: the recipient field admits the event; the rifle reaches the loot cache

JOIN222 watches the evaluation inside `0x26a544`. The selector-2 body carries nine
84-byte schema records; `0x84f68b44` is the record for `0x80800e70`+40 (body
offset `0x40`), value kind 1 (a 64-bit ID). `0x26a3dc`/`0x26a1d8` turn that field
into one recipient of type `0x80800e5d`, and `0x26a00c` compares it with the
delivery filter. The kind-1 filter (vtable `0x13e70e0`) resolves its player slot
through the table at `0x1a1d720` (`0x270bec`) and compares the slot entry's +0x30
word. For the local player that word is **1**: the activity player ID, the same
value as the last field of `--ah-sim-player`, not the 64-bit character ID.

| Send (same nonce, position) | `0x80800e70`+40 | World | Result |
| --- | --- | --- | --- |
| 1 | 0 (JOIN220 body) | 3 | filter has no recipient; `0x5aede4` not called |
| 2 | character ID | 3 | recipient compared and rejected |
| 3 | **1** | 3 | **`0x5aede4` → `0x2b2a60`**; one cache entry (world 3); manager `0x2b36f8`/`0x2b3240` visits item 3598 |
| 4 | 1 | **14** | second cache entry for world 14; the rifle record's unit reference becomes `0x5ad896f7`/`0x7bfb81c1` and its position matches the current world |

The JOIN220 body re-encodes byte-for-byte from its recorded values. Only the
recipient and world fields change. This is the first native evidence of the
loot path allocating a world unit for the starter rifle. It is still a
**synthetic** private record and event. The original event name, the authentic
nonce authority, visibility at the chest, the native pickup request (service 601)
and a durable grant remain unverified. The player stayed at the outdoor start,
so the unit was not inspected. JOIN222 closes at **08:05:43.9 UTC on 9 October**
with all exact identities closed, the configuration hashes restored, XInput and
shared RPCN preserved, and no cleanup errors or active controls.

## JOIN223: the rifle spawns in the Breach and the client claims it (service 601)

JOIN223 uses the same private fixture, sends the JOIN222 event (recipient 1,
world 14) once in the outdoor slice, then follows the ordinary guarded route
into the Breach. The loot unit allocated outdoors (`0xed3763f5`/`0x7afb81c1`) is
released when the slice changes: the Breach's current world ID is **3**
(handle `0x18`), not 14. Codex's original world 3 was correct for the rifle position.
A second event with world 3, sent at the wall, adds a world-3 cache entry. The rifle
record then gets unit `0xee2f6847`/`0x23fb81bd`, and the bauble constructor
`0x2b2c00` runs.

Walking onto `(410.644, -794.385)` triggers the native pickup: the deferred
callback (`0xcbb5f0`) calls `0x472518`. The client sends one **service-601
request** to the backend (19 bytes, `02594800000010a3d000000002170800000010`). It
decodes with the measured request type `0x80801757` as
`{0: 3, 8: {0: {0: 1, 8: 0x0a3d000000002170}, 16: 1}}`: source kind 3 and exactly
the pending record's nonce `{1, 0x0a3d000000002170}` and word 1. This is the first
live pickup claim observed.

The stand-in has no 601 handler, so it sends the generic default reply. The
request is queued without a response callback (`0x47676c` with null callback
arguments), so the grant must arrive as a character-state update. No update is
sent: the pending record and the loot unit remain, and no item is granted.
Recording segments stopped before the walk, so the visible rifle model was not
inspected. JOIN223 closes at **08:24:36.0 UTC on 9 October** (owned stop
`0xffffffff`), with all exact identities closed, the configuration hashes
restored, XInput and shared RPCN preserved, and no cleanup errors or active
controls.

Next: answer service 601 by removing the claimed pending record and adding the
item to the character, then publish the self update. After that, send the event
from the mission script on Breach entry instead of the private control file.