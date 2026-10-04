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

## JOIN99: physical Ghost creation and character preview

JOIN99 repeats the cleared Breach handover with all 249 occupied records
baseline-ready. At 19:30:58 BST a fresh inactive bubble reset and explicit
one-variant desired count were published. By 19:31:00 the Ghost squad has
one live member, an active actor datum and a valid physical unit datum.
The native creation path agrees with those reads. A generation alone had
left that same squad empty. The mission now schedules this verified desired
count on its existing delayed current-slice-24 rule; bubble readiness remains
a prerequisite, and this does not supply counts for Fallen encounters.

The helper's typed squad selector still creates no attachment. Native tracing
shows its first-result handler advancing past the sole member before checking
the bound. JOIN100 therefore tests the actual `of_ghostlight` filter with a
direct kind-2 member reference. The filter alias alone adds no automatic light
action. See [squad counts](squad-member-counts-0124.md) and
[light selection](ghost-light-0124.md).

Normal controller input opens the character screen from the Breach. Frames
at 19:39:01 and 19:40:55 show a Titan with normal proportions and armor
lighting, without the earlier magenta distortion. These extend the corrected
setup preview evidence to the inventory view during a mission. The rifle in
that preview comes from the starter-gear overlay; it does not prove durable
pickup or equipment. Loading inventory slots, the in-world third-person
Guardian and later social-space appearance remain open. The sampled Breach
frames also remain clean, but none demonstrates the Ghost's torch.

## JOIN100: Ghost unit selection and registered attachment

The fresh run again clears all 249 occupied baselines. One desired count
creates one Ghost actor, whose unit is returned by actor accessor `0x191960`
at `+0x30`. The filter's direct member-95 reference finds no network binding;
the physical Ghost is bound under squad 94 instead. Changing the filter's
reference to that squad yields the actual Ghost unit, reaches the configured
attachment creator from the HO caller, and leaves a new HO-101 binding to
the target. Subsequent creation attempts recognize the existing attachment.

This is a successful native attachment path, beyond the empty selectors in
JOIN98/99. It is not yet visible torch acceptance: the Ghost is about 65 units
ahead of the player. A normal guarded walk responds, then stops at client
cleanup. The screenshot explicitly says the client returned to Orbit for
inactivity, despite brief movement. JOIN101 tests the automatic desired count
and repeats the corrected filter promptly before walking closer. The manual
states and LLVM watch limitations are in [the light note](ghost-light-0124.md).

The changed mission passes the actual 5,041-type schema preflight and all
156 server tests, including a 72-bit regression for its one-Ghost count and
required modes. The unchanged script suite previously passed all 115 tests.

## JOIN101: automatic Ghost count, close walk and rendering checks

The guarded route reaches its endpoint at 20:27:06 BST. By 20:27:15 an
owned snapshot has all 249 occupied records baseline-ready, including the
required Ghost records. The client subsequently retains current slice 24
while the engine is in state 3 with return target 112; settled transition
completion remains open.

After a manual protocol reset `3:1`, the **shipped mission rule** publishes
the one-variant desired count at 20:27:36.660, twenty seconds after its
current-24 match. No manual count update is used. By 20:27:38 the squad has
generation 1, desired counts `[1, 0, 0, 0, 0, 0, 0, 0]` and one live member.
Actor `0x02f58000` contains unit `0x74fb82ce` at `+0x30`; the unit resolves
to `(390.458, -786.975, 19.122)`. Both native physical-creation entries agree.
This validates the automatic count after readiness and reset, not an
automated bubble reset or complete Ghost behavior.

The corrected squad-94 filter and HO states are then sent manually. The
creator `0x1f7c94` receives the Ghost unit with `lr=0xc11e58`, followed by
helper registration at `0x1e5060` with `lr=0xc11ee0` and the exact HO ID.
Registration's `r4=0x65fac971` is a separate datum; it must not be called the
selected Ghost unit. The actual filter/helper sequence still requires a
visible illumination check before it becomes an automatic mission action.

Ordinary guarded input reaches `(382.896, -794.349, 14.162)` at 20:28:36,
about twelve units from the Ghost. The nearby frame remains dark. The
20:34:50 inventory frame nevertheless shows a normally proportioned Titan
with clean armor lighting, extending JOIN99's rendering check. Level 0 and
loading equipment slots remain; the preview rifle is the starter overlay,
not a verified weapon grant.

A second guarded walk returns to `(340.888, -795.741, 14.115)` at 20:37:23
without cleanup or a position jump. The initial view there is dark but
contains faint geometry. Turning toward the windows produces a clean,
bright interior at 20:41:25, retained in the final 20:42:55 frame. Thus the
dark samples do not demonstrate a persistent full-screen fade or recurrence
of the magenta artifact. They also do not establish the torch: camera
orientation and local illumination must be checked together near the Ghost.
The owned run was stopped after preserving the trace and captures.

The next light test should resolve the created helper entity separately
from its target, inspect its component state/placement, and capture the
corridor with a controlled view direction. The generation path reaches a
default-payload update through the entity interface; it is not a demonstrated
light-on setter. See [the light investigation](ghost-light-0124.md).

## JOIN102: helper components, controlled view and camera repair

At 23:17:19 BST the fresh run again has 249 occupied records and zero
pending baselines. Following the manual bubble reset, the shipped
current-24 rule creates one Ghost by 23:17:42. The corrected manual
filter/helper sequence again reaches creator and registration. The
registered container is independently resolved: all twelve configured
components exist, share that owner and target the actual Ghost unit.
Their configured types and runtime header classes are recorded in
[the light investigation](ghost-light-0124.md); none is yet semantically
identified as a light.

A guarded 120-second walk cannot reach `(385, -791)` through the corridor
obstacles. The tool releases input on expiry. Subsequent camera input
reveals that the walker's pointer validator rejects valid pitched unit
vectors. It now validates their full 3D length and retains a usable
horizontal heading. Recovery also retains an unchanged valid pointer,
avoiding unnecessary movement probes after a short turn. All 119 script
tests pass, including four camera regression tests.

Controlled views from about `(386.7, -795.5, 14.45)` face the Ghost at
actual yaw 64.74 degrees and pitches 2.90, 12.65 and 29.42 degrees.
Those views remain dark with visible HUD. Turning back to yaw 173.99,
pitch 1.26 shows lit orange corridor geometry. Earlier window frames
are also clean. This further rules against a persistent whole-view fade,
but does not establish an unobstructed Ghost view or a working torch.

The helper accepts a manual generation-1 state at 23:33:52; both current
and consumed generations reach one. The same Ghost and all its desired
counts remain present. Bounded native watches do not observe the
generation action, so its payload semantics remain unverified. Further
controlled views stay dark. No automatic helper/light action is added.
The owned run is stopped after preserving the snapshots and captures.

Continue by identifying the instantiated components' activation and
transform requirements and finding an unobstructed view. Stable return
completion, torch, durable weapon pickup, combat and mission completion
remain open. The earlier combined color-buffer rendering fix is retained.
