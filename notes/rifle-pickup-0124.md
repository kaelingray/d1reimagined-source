# 01.24 Breach rifle: correct object, pickup still open

Updated 2026-10-08, continuation after JOIN205–207 on source `694b183`.

The rifle cue previously created `o_chest_primary_weapon`, whose authored
placement is about 137 horizontal units away from the rifle waypoint. The first
mission's `o_weapon` matches the waypoint instead. The mission now creates it
alongside `seq_d_o2_80`, 30 seconds after the automatic Ghost-light trigger.
The old chest alias remains available for diagnostics.

| Object | Group / kind / index | Authored position | Unit asset |
| --- | --- | --- | --- |
| Previously selected `o_chest_primary_weapon` | `fc0e45a3 / 4 / 7` | (534.865, −736.800, 16.507) | `80cb82a8` |
| Corrected `o_weapon` | `992bb471 / 4 / 6` | (410.644, −794.385, 17.723) | `80cb828c` |
| Rifle waypoint | `992bb471 / 2c / 78` | (410.780, −794.410, 18.520) | — |

These identities come from the latest disc/update package chains, corrected
relative name pointers and each definition's embedded replicated object ID.
The weapon and marker differ by about 0.14 horizontal units. This correction
does not establish an inventory grant or a visible pickup prompt.

## Native evidence

JOIN208 used the earlier chest selection. An obstructed initial route and a
later position discontinuity prevented a useful rifle test. Its cleanup passed.

JOIN209 used `694b183` plus the rifle correction, the committed Titan starter
kit and existing C5 emulator. No guest writes, teleport, renderer rebuild or
manual mission-state changes were used. The route crossed naturally into the
Breach. At 19:04:22 UK (18:04:22 UTC), the platform trigger scheduled the automatic
chain; at 19:04:53 UK, the host sent `o_weapon`.

The native creator received unit asset `80cb828c` at the authored rifle position
and returned a non-absent unit datum. A later read-only sample verified its
unit-pool bank, index and occupancy bit, with the row and registry unchanged
during rechecks. The three control/light units also passed these checks. This
establishes allocation, not illumination, usable collision or pickup.

The successful stair approach runs south through (383, −797), (387, −799),
(390, −795), (394, −794.5), then (395, −791.5). Approaching north directly from
the lower corridor hits geometry. From the upper walkway, JOIN209 again reached
the grey barrier near y −791; an ordinary jump did not pass it. Side approaches
are diagnostic exploration, not an accepted route to the rifle. The east attempt
stops near x 417.6 and the south attempt near y −788.8. Neither establishes that
the authored path is inaccessible. JOIN209 closes cooperatively at 18:12:27 UTC;
all nine exact owned identities and three baseline configuration hashes verify
closed/restored, with XInput restored, RPCN preserved and no cleanup errors.

The route helper had a separate close-range defect: its two-unit intermediate
waypoint tolerance could release movement before reaching a requested one-unit
final tolerance. The final waypoint now uses the caller's tighter tolerance.
A slow-pad-poll regression fails before this repair and passes afterwards;
existing arrival, guard and facing checks remain intact. This improves test
precision, not the world's collision geometry.

## Open checks

The host's object-stream encoder emits State A and always marks optional State B
absent. Kind-4 props have secondary state type `80800387`; its actuation semantics
have not been established. Native creation therefore does not rule out a
mission-state fault in the lamps. Do not send the secondary type in the State A
slot: that changes wire framing.

Next: identify the barrier and authored path or gate actuation, confirm the
visible weapon and interaction prompt, then verify unarmed-to-Khvostov pickup,
ammo, grimoire, objective handover and durable inventory grant. Room illumination,
the black companion Ghost, extra mission banners, Nav Mode and combined
character/world flicker and distortion remain open.

Verification: 198 server tests, 166 script tests and final 01.24 schema preflight
(1 timed / 55 reactive changes) pass. Recordings, extracted package data, unit
handles, probe outputs and production inputs remain private.

## JOIN210–211: barrier opens through the prop activation command

The barrier is opened by State A `+0x10` typed command `0x8080279a` on the Breach control,
door-light and gating-light props, with all three channels at (1, 1, 1.0) (JOIN210, manual).
The mission sends it automatically 2 s after creating those props; JOIN211 then reaches the
rifle without any control lines, trigger `0x1d:35` reports and Fight Through the Wall starts.
A visible weapon, pickup prompt, ammo, grimoire and inventory grant remain unverified; the
fixture still pre-equips a rifle.
## JOIN212–215: no pickup prompt at the chest

With the barrier open, the closed chest renders at the rifle point, but holding Square (Z) from
either side, with the camera tilted down, produces no prompt and no client request; activating
`o_weapon` with the barrier command changes nothing. The pickup is probably a loot object the
host must place there; see [loot and rewards](loot-and-rewards.md). A second Message of the Day
dialog can block input after control (JOIN212); automated drives now dismiss it.