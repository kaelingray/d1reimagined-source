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

The private baseline list was initially corrected with order, ID uniqueness and
count preserved. JOIN96's result below identifies a second defect after that
framing repair. See
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

JOIN94 made steady preload progress, but 98.02% of the archived cache's 198,264
pipeline entries referred to rejected raw programs. Repeated reads and logging
made that archive unsuitable for the launcher's eight-minute sign-in window.
The owned run was stopped before gameplay and its cache preserved. The loader
now remembers deterministic rejected fragment hashes for each preload, skipping
later pipeline variants before their vertex/fragment reads. It retries transient
I/O failures and rechecks files on the next preload. The full Release build of
this additional change also passed and was deployed with a separate backup.

JOIN95 used the smaller JOIN92 cache. The stack became ready, but the first
title-input attempt failed while the Destiny game window was not yet available.
The wrapper cleaned up the exact owned processes; no sign-in or mission result
is claimed. Startup now captures real stdout/stderr and distinguishes an actual
exit from failure to verify process identity. It rejects shared logs older than
the launched emulator. See [startup evidence](run-startup-evidence.md).

## JOIN96: arrival fixes work; an AI definition crashes baseline decoding

JOIN96 booted with the rebuilt emulator and smaller JOIN92 cache. PPU execution
started about 2 minutes 43 seconds after launch. Startup retained the actual
emulator output, and title input waited for the owned game window.

The revival handover completed normally: state 4 at 13:31:49 BST, current slice
112 at 13:31:56, then inactive highway reset `14:2` and a fresh highway Ghost
generation. Ordinary controller movement reached the route endpoint at 13:36:15,
with position `(352.517, -796.696, 14.040)` inside its unchanged 1.5-unit radius.
The route returned `route_reached`; the arrival latch avoided JOIN92's coasting
failure. No guest memory was written.

The host request for slice 24 was published at 13:36:17. A target-only state-2
delta at 13:36:28 did **not** finish it using cached current 24; the next delta
reported current -1, and state 3 followed. This validates the stale-arrival fix.
The client then froze while decoding a baseline, before a completed handover or
playable Breach frame. Both late screenshots show the black loading view.

The preserved stack identifies `0xfc0e45a3 / 0x29 / 141`, `fa_e3_entry`, as the
object being decoded. Its registry lookup resolves an AI definition's metadata
instead of a replication record; the reader interprets metadata as state types
and faults on a null type descriptor. An owned, read-only snapshot still contains
249 actual records, now with 32 awaiting baseline, compared with JOIN92's 119.
Their IDs and B flags all match the corrected list. The smaller pending count
shows progress past the former helper framing defect, not complete readiness.

The baseline exporter now excludes verified non-replicated kinds `0x29`, `0x2a`
and `0x2d`. Direct registry reads distinguish their metadata from the helper's
real pool record. The private list drops 856 such entries, retaining 1,455 unique
IDs in all 31 selected groups and covering all 249 observed records with matching
B flags. A regression protects both exclusion and original object indexes.
All **155 server tests and 115 script tests pass**.

JOIN96's sampled title and exterior route frames show no magenta/rainbow blocks.
It did not provide a new inventory-model or stable Breach rendering check. The
existing combined flicker/character-lighting fix remains enabled, including
colour-buffer reads.

## JOIN97: baseline readiness and visible control in the Breach

The filtered-list run reached the route endpoint at 18:22:16 BST. A temporary
Windows lock on the diagnostic watch file stopped the private helper before it
queued the handover. The watch update was recovered, with a bounded retry added
for the next helper, and the host request was queued at 18:24:45.

The client reported state 4 and current 24 at 18:25:05–06. A fresh native
`0x1df644` observation identified this run's replication context before an owned
snapshot: **249 occupied records, zero awaiting baseline**, with all four loaded
groups retained. There was no recurrence of JOIN96's decoder fault. The 18:25:54
frame shows the Breach's windowed metal walkway and radar, and subsequent frames
at 18:28:36 and 18:33:43 retain the interior without the prior magenta blocks.

A newly armed player hook identified a current entity at
`(324.444, -795.639, 14.101)`. A normal 600 ms movement input changed its position
to `(325.885, -796.227, 14.057)`. This establishes visible controller response
after the handover, rather than a black view with a deleted unit. The engine is
still in state 3 with a return target 112 while reporting current 24, so a fully
settled idle transition is not claimed.

At 18:31:02 an inactive bubble reset `3:1` was sent through the control file.
The reset's bit 3 remains set in outer SIM `+0x844`, while `+0x850` is clear.
At 18:31:18 the client's type-6 stream contains the exact final-schema IDs of
`sq_nux_ghost` and its member `0xfc0e45a3 / 2 / 95`, as well as a floor squad and
member. A subsequent explicit Ghost generation was accepted at 18:32:22. The
ID match precedes that latter generation; do not attribute it to the later
action. JOIN98 below narrows this evidence: the arbitrary-offset matcher does
not establish a physical Ghost spawn, visible torch or completed encounter.

Opening the character screen at 18:29:48 produced loading placeholders and no
Guardian model. Those frames cannot verify character-model appearance; inventory
projection remains incomplete. The sampled exterior/interior rendering and the
earlier corrected Titan preview are distinct evidence. JOIN98 adds the named
`ho_ghostlight` alias and tests its selector after readiness and the Ghost member
report. The alias itself adds no automatic mission action.

Remaining acceptance work is a stable playable Breach, correct Ghost light,
durable rifle pickup/equipment, encounters, rewards/results, Tower arrival and
relog. See [acceptance checks](guardian-rises-acceptance.md).

## JOIN98: repeated readiness and the empty Ghost selector

JOIN98 reached the guarded route endpoint at 18:54:41 BST, then reported
current slice 24 at 18:54:52. The handover briefly cleared its record pool;
an empty pool was rejected as readiness even though its pending count was zero.
A subsequent owned snapshot contains all 249 records with zero pending,
including the required Ghost squad/member/helper IDs.

Reset `3:1` cleared the waiting bit. At 19:04:00 the named helper accepted its
typed selector, and twelve native creation attempts show the expected absent
reference and squad reference decoded exactly. None reached member selection
or attachment creation. The sampled Breach frames show no new visible light.
The squad had zero live members at `+0x1d8` before the test.

This corrects the earlier interpretation of arbitrary-offset type-6 ID matches
as spawn evidence: an encoded member reference can occur inside another state.
Likewise, `0x245204` only clears a bookkeeping byte, despite earlier notes
calling it a spawn hook. The final squad state has separate desired member
counts at `+0x2c`. A one-member candidate matches the extracted Ghost variant
and round-trips through the final schema; JOIN99 tests it live. Details are in
[the count investigation](squad-member-counts-0124.md) and
[Ghost-light evidence](ghost-light-0124.md).

JOIN98 subsequently returned to Orbit for inactivity. The notice was captured
before its owned stack was stopped. No in-mission Guardian model or completed
torch check is claimed from this run.
