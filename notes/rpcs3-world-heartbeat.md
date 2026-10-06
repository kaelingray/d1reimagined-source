# State-preserving activity-host heartbeats — 2026-10-06

The seconds-only `--ah-heartbeat` option previously sent an empty type-11 message,
even while the connection owned a world state and pending teleport. An empty
message is not equivalent to resending that state. Earlier investigation observed
its effect on the simulation's request record; current controls again stall at a
loaded but inactive gameplay slice.

The default now resends the current world state, including the pending request,
switch target and peer byte, using the **same version**. Heartbeats do not increment
the version or manufacture arrival acknowledgements. A connection without world
state retains its empty type-11 keepalive. Explicit numeric subtypes, including
`SECONDS:11`, retain the old empty-message behavior for controlled diagnostics;
`SECONDS:-1` still sends no heartbeat message. Explicit `SECONDS:world` remains
supported.

An encrypted session fixture fails before the change because its pending request
is followed by empty updates. Afterward it receives repeated request-bearing
updates with the unchanged version. The affected suites pass **12 teleport**, **6
activity-session** and **51 backend** checks, including default keepalives without
world state and existing finish/drop/version behavior. This is a protocol default
repair; these CPU checks do not establish native mission completion.

`scripts/rpcs3_final124_transition.py` reads engine current/target records, peer
byte, cached world version, copied group records and active/loading/loaded slice
state through an exactly owned process. It writes no guest state and does not
pause execution. Repeated-field guards identify observed changes without claiming
an atomic snapshot. Five fixtures cover loaded-but-inactive state, unsafe indexes,
changed groups, uninitialized engines, short reads and overflow. Raw observations
stay private. The activity-log summary also retains teleport completion/drop and
connection lifecycle messages; its three checks pass.

JOIN142 uses the previous empty-heartbeat default, normal recording and the depth
candidate/validation layer disabled. At **07:29:16 UTC** it requests slice 112. By
07:29:18, that slice is loaded with no active/loading slice, engine state 6, peer
byte 2 and empty copied group records. The host world still holds request state 1
and version 2, while no corresponding target-112 client report reaches the proxy.
The connection closes at **07:30:39**, then rejoins. Its **9:01** pre-stop native
sample has zero fatal/device-loss/malformed entries. This locates a handoff failure;
it does not attribute every stalled transition to empty heartbeats.

JOIN143 repeats the same server/gear/mission fixture and executable with the new
default. The host request persists, but **native handoff still fails**. At
**07:38:47 UTC** the request changes to 112; by 07:38:49 it is loaded with no active
slice. A second connection reproduces this at **07:43:14–07:43:15**. Both retain
engine state 6, target 112, peer byte 2 and empty copied groups, without a target-112
client report. Each disconnects about two minutes after the request and rejoins.

A bounded PPU watch verifies the live simulation pointer from the function's own
argument. At native **11:58**, three consecutive calls to `0x3adf8c` retain pending
copy state 2 and host-request state 1 for slice 112. The outer update and engine
pending-request handler also continue. This rules out a stopped request tick in
that sample; it does not establish why publication or acknowledgement is missing.
Unique-content watches alone cannot establish that a function stopped running.

The **16:12** pre-stop log has zero fatal, device-loss or malformed-program entries,
with 2,125 program additions, 9,560 descriptor-set first initializations and 9,487
idle releases. This is a bounded renderer observation, not repaired pixel or mission
acceptance. Ordered cleanup closes the exactly owned stack and restores the original
watch file, XInput and byte-identical title configuration. Normal capture remains
healthy, with nine verified NAS deliveries and no queued work or errors.

Static comparison maps the type-21 sender and encoder from 01.00 to **01.24**:
`0x2720e4` calls `0x27203c`, which passes host `+0x66e0` and `+0x6900` to
`0x1db1cc`; the sender's indirect transport call uses subtype 21. The engine update
compares its `+0x88` state with `+0x1f0` and posts event `0x4f` through `0x92c5b8`.
The event adapter is `0x104aff0`. JOIN144 observes the engine event, adapter,
client-state update and sender during the initial 113 join. The state offsets need
careful naming: `+0x66e0` is the sent baseline, `+0x6900` the pending state; a
successful send copies pending to baseline at `0x27241c`. Its private early
timeline labels were reversed and are corrected before the intended handoff trace.

JOIN144 ends in **RSX GPU device loss at 3:27.678568**, before the mission teleport.
The device-fault query reports a write hint in **0x0–0xfff**. It therefore supplies
initial-join publication evidence, **no failing-handoff trace**. Original watches,
XInput and title configuration are restored and all exact owned processes close.
These mappings do not yet prove the missing link or justify manufacturing peer-byte
acknowledgements.

Next: follow the changed engine snapshot through the event, client-state dirty mask
and transport. Then obtain outdoor, body/equipment and indoor frames. Keep this
handoff investigation separate from the unresolved GPU write fault and guarded
depth experiment.

JOIN146 adds bounded context/publication watches to the rebuilt recorder executable.
It reaches active slice **113** with engine state 0, world version 1, local peer slot 0
and group 0 naming 113 with state 1 and peer byte 1. The sent/pending client records
also publish 113. The manager's call to `0x8fed64` is observed with return address
`0x33bfb0`; its context has state 6 and a valid record handle. That record's handle
is not a slice-set index.

The verified manager path (`0x33bde0`) permits context states 4–10, then requires a
nonzero context state and a nonnegative record handle at `+0x408`. Its fresh-state
update is `0x273e50`; sender `0x2720e4` publishes it. Selected/opposite context banks
follow `0x349634`/`0x349adc`, a `0x12208` slot stride and `0x9100` bank stride.
These gates are open during the initial join. A unique-content watch reaches its
48-hit bound during startup, so its later silence cannot prove a stopped function.

At **3:38.689770**, before the mission handoff, JOIN146 reports SPU-side GPU device
loss. The fault query returns a write in **0x0–0xfff** with no instruction-pointer
hint. This supplies initial-join gate evidence, no stalled-handoff gate observation.
The owned client/services/observer/launcher close; original watches, XInput and
title configuration are preserved. Capture drains without error or a queued event.
Renderer stability remains necessary before this particular handoff trace can finish.
