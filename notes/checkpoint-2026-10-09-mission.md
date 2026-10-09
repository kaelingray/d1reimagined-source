# 9 October 2026 — rifle incident decoding

The next missing rifle prerequisite is now narrowed to the native event queue.
JOIN219 identifies the selector-2 body codec; JOIN220 confirms that a synthetic
host event decodes to the intended nonce, position and world ID before reaching
queue submission. Rifle pickup, combat and the full game remain incomplete.

The investment encoder previously consumed zero bits for dynamic-reference
kind `0x19`. The null case actually requires a six-bit primitive selector and
one absent-handle bit. Encoding and decoding now preserve those seven bits;
unsupported present references fail explicitly. Five independent wire-fixture
tests cover alignment, nullable aliases, invalid selectors and truncation.
The private control file also gains an explicit bounded host-incident command,
with five tests covering envelope bits, bounds and complete-line consumption.
No automatic rifle event or production mission change is introduced.

JOIN220 receives both 76-byte envelopes through the host connection. Its second
event reaches native queue submission with the expected decoded body, but no
queue-commit watch is installed. The loot cache stays empty and the loot/pickup
callbacks do not fire. This is decoding acceptance for a synthetic fixture,
not an authentic reward, pickup or durable grant. Detailed addresses and limits
are in [incident diagnostics](incident-diagnostics.md#join220-synthetic-body-decodes-queue-completion-unverified).

The ordinary route exhausts its budget at the initial position. The inspected
image is black with the early Level Up coach. Movement/combined rendering are
not accepted in this run, and their relationship to the event test is unknown.
The renderer executable and production mission inputs are unchanged.

JOIN219 closes at 22:47:17.519 UTC and JOIN220 at 23:08:03.260 UTC on **8 October**;
the latter is 00:08:03.260 UK on **9 October**. Each independent closure check
verifies nine exact identities closed and all three baseline configuration
hashes restored. Private receipts retain actual timestamps, source state and
hashed evidence. The previously ready October 8 reporting package stays immutable;
this late evidence belongs in the next consolidated report with its original dates.

Validation: **212 server tests**, **176 script tests**, and the actual final-schema
preflight with **1 timed / 58 reactive** changes pass. Raw game inputs, native
metadata, traces, fixtures and production material remain private. Next: trace
the queue gate, command serialization and commit; then verify the cache callback,
native pickup request and durable replicated grant. Lighting, Ghost brightness,
character distortion/flicker, Nav crash, banners, combat and mission completion
remain open.

## JOIN221–223: from queue submission to a live pickup claim

JOIN221 follows the synthetic event past queue submission: it is committed,
executed (`0x104ad30` → `0x92c774`), delivered to object 1 and dispatched to the
generic handler `0x5aea30`. There the registered handler `0x5aede4` is filtered
by a recipient check. JOIN222 identifies that recipient field: `0x80800e70`+40
(body offset `0x40`) must hold the **activity player ID** (1 for the local
player), not the character ID. With that value set, `0x5aede4` records the nonce
and position, and the loot manager allocates a unit for the rifle record.
JOIN223 finds that the Breach is world **3**. With world 3 the rifle unit is
allocated in the Breach, and walking onto the rifle's position makes the client
send a **service-601 claim** with the pending record's source kind and nonce.

The event and pending record are still a private synthetic fixture, not the
original reward authority. The stand-in does not yet answer 601 with a grant, so
no item is added. The rifle model at the chest was not visually inspected.
Details and addresses are in
[incident diagnostics](incident-diagnostics.md#join223-the-rifle-spawns-in-the-breach-and-the-client-claims-it-service-601).
All three runs end on their bounded time limit with clean, independently
verified closure.

Next: grant the claimed item through 601 and a self update, then move the event
into the mission script so the rifle appears at the chest in the reference
order.

## JOIN224–226: the claimed rifle is granted

The stand-in now answers service 601. A matching claim clears the pending record,
equips the Khvostov with its art values, and publishes self and peer updates. In
JOIN226 the acquisition notification appears, then the rifle is in first person
with crosshair and ammo counter, and R1 fires. The magazine still reads 12, and the
urgent dialog and extra mission banner still appear. The event and pending record
remain a private synthetic fixture. See the
[rifle pickup record](rifle-pickup-0124.md#join221226-native-pickup-and-service-601-grant-synthetic-event).
Since JOIN228 the drop comes from the mission script: a `loot` table and a
`loot_drop` step make the activity host send the event, and `--world-loot`
(the same mission) gives the signed-in character the matching pending record.

## JOIN231–232: first Fallen combat in the Breach

The Breach group (`0xfc0e45a3`, catalog `0x80cb8167`) lists every Fallen
encounter of Fight Through the Wall: `sq_fallen_first_vandal` (index 21, no
authored member), then floor, wall and dropdown Dreg squads with authored members
(`…__dreg`). The mission restarted three of these squads but sent no desired
counts or member states, so no Fallen spawned.

JOIN231 sent, through the control file after the rifle grant, a one-member desired
count for `sq_fallen_first_vandal` and the authored member state used for the Ghost
(`{0: 1, 4: -1, 5: 0, 6: true}`) for `sq_fallen_floor_a__dreg`,
`sq_fallen_floor_b__dreg` and `sq_fallen_wall_l_a__dreg`. Within seconds the radar
shows red contacts, Dregs reach the rifle room and attack, and the screen reddens
with damage. The Guardian is killed twice ("Guardian Down", killed by a Dreg of the
House of Devils) and respawns each time. A targeted Dreg shows its name and health
bar. The automated input fires in a fixed direction and does not aim, so no kill is
established. No Vandal was identified in the sampled frames.

This is the first native Fallen AI, damage, death and respawn in the 01.24
restoration. The mission now sends the same four states 30 s after the rifle trigger
(near the reference's first contact, about 40 s after the pickup); JOIN232 tests that
automatic step. Still open: aiming and kills, encounter placement relative to the
reference (Vandal in the corridor, Dregs in the tank room), XP and grimoire
popups, and later squads.
JOIN232 repeats it with the mission step alone: 30 s after the rifle trigger the host
sends the four states (rules 35–38). The recording shows Fight Through the Wall, red
radar contacts, damage, "Guardian Down" (Dreg, House of Devils) and respawn, with no
control lines. The extra "New Mission" banner and urgent dialog still appear after the
grant.