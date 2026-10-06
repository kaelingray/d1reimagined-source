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
default. Native acceptance is pending. Compare request retention and group/version
exchange before judging gameplay, then obtain outdoor, body/equipment and indoor
frames. Keep this handoff investigation separate from the unresolved GPU write
fault and guarded depth experiment.
