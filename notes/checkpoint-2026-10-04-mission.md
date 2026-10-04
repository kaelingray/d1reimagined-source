# 01.24 continuation — 4 October 2026

Target: BLES01857 01.24 / Destiny 2.3.1. This continues the
[October 3 checkpoint](checkpoint-2026-10-03-mission.md), using the two supplied
tutorial specifications as acceptance references. The first mission is still
incomplete. All game data, runtime logs, screenshots and emulator binaries remain
private; the repository carries code, patches and summarized evidence.

## JOIN92: actual entry, followed by a black handover

JOIN92 booted with a fresh shader cache on 3 October. Resurrection's teleport
finished at 21:48:36 BST, and the activity-host report reached current slice 112
at 21:50:09. The route then used ordinary controller movement, without memory
writes. The client requested slice 24 at 21:52:09.

At 21:53:26, a guarded position read placed the same player at
`(354.387, -796.157, 14.048)`, within 0.65 units of the endpoint
`(353.751, -796.292, 14.008)`. The walker subsequently coasted beyond its arrival
radius and tried to turn back until the existing 20-second doorway guard ended.
Thus its final result was `stopped`, although the recorded position had reached
the destination. A 21:54:26 screenshot shows the Breach's lit metal walkway and
HUD. This establishes visible natural entry, not a settled host handover.

A manual host teleport to slice 24 was queued at 21:55:20. A target-only client
delta at 21:55:37 reused the handler's cached current slice 24 and prematurely
finished the new request. The next delta reported current -1. The client later
reported state 4 and current 24, but a 21:56:50 frame was black without the HUD,
and the prior entity had been cleared. Successful protocol arrival did not make
this handover playable.

The walker now latches arrival from each guarded position read during turning
and movement, before subsequent coasting can lose it. It retains the same goal
radius, ownership/freshness checks and 20-second doorway bound, and releases all
movement keys when arriving. Three regression tests cover arrival during motion,
arrival before turning, and a rejected guard observation.

The host now completes a teleport only using a current slice present in that
incoming delta, or the fresh request-complete state 4. Cached current remains
available for routing decisions. Four encrypted-session regressions cover stale
current, target-only and peer-only deltas, genuine arrival and request completion.

## Baseline coverage was not enough

The late JOIN92 table contained 249 live records, including 119 awaiting a
baseline. The private host list covered their IDs, but one live record declared
state B while the exported list omitted its presence bit:
`fc0e45a3/18/101`, the Breach `ho_ghostlight` helper object.

Its actual entity-definition root component declares state A `0x8080065c` and
state B `0x8080048a`, matching the live record. Seven kind-`0x18` helper-object
definitions in the inspected package chain declare those same state types.
The exporter now includes kind `0x18` in its state-B classification. A synthetic
multi-object wire test independently models the client's declared B types and
shows the former omission consumed the following object's continuation bit.

The private baseline list was corrected with order, ID uniqueness and count
preserved. This repairs a demonstrated stream defect; the effect on the black
handover still requires a new live run. See
[private-host investigation](private-host-investigation.md).

## Emulator cache guard

The preserved cache that stalled JOIN91 contains 11,814 raw fragment programs:
11,136 fail used-source-type validation and four contain impossible branch
targets. The new [cache patch](rpcs3-fragment-cache-validation.md) validates
reads and writes before cache processing, preserving rejected files.
Its 22 standalone fixtures pass, and the full Release emulator build completed.

JOIN93 did not retain its launched emulator long enough for stack readiness.
It has no fresh native startup log or matching Windows crash event. DLL imports
match the previous binary, and direct `--version` and game-launch probes succeed.
The launcher named a stderr file that it did not create and copied an earlier
run's shared log into its summary. Those artifacts cannot establish a native
crash or successful JOIN93 boot.

The direct game-launch probe used the preserved bad cache and recorded rejected
fragment programs being skipped. This validates the deployed read guard; it
does not prove live rendering or mission completion. The old executable and
both cache generations are retained privately.

## Next live check

JOIN94 starts with the rebuilt emulator, corrected helper baseline, fresh-delta
teleport handling and latched route arrival. After observed current 112 it resets
the inactive highway generation, requests the highway Ghost generation, walks
to the actual entrance and requests the host handover only after route arrival.
Completion and baseline watches distinguish protocol completion from entity
creation and a usable visible world.

Remaining acceptance work is a stable playable Breach, correct Ghost light,
durable rifle pickup/equipment, encounters, rewards/results, Tower arrival and
relog. See [acceptance checks](guardian-rises-acceptance.md).
