# 01.24 incident and disconnect diagnostics

Updated 2026-10-02. These findings come from the private BLES01857 01.24 ELF, corrected
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
