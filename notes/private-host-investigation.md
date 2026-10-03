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
