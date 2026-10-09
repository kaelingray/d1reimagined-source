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
### Weapon stats (JOIN233–235)

The final stat table `0x80a5e1d3` has 36 entries using the standard Destiny stat hashes
(Attack `0x15f5c563` at index 11, Impact 13, Magazine `0xe6be4c5a` at 20). The item
object's instance block (`0x80801846`, item object +0x28) holds a 36-byte u8 array at
+0x22, and its first sub-structure has the shape of a primary stat (three i32 and an
i16). JOIN235 sent the Khvostov with primary stat {Attack, 5, 5, kinetic} and index 20
= 25. The magazine still read 12, so that reading is wrong or the HUD takes the
magazine from elsewhere; the experiment was not kept. JOIN233's aimed-fire helper
(nearest moving entity from the `0x138e24` hook, position +0x40) fired 22 shots. One
targeted Dreg stopped updating after 16 shots, but without video no kill is accepted.
The Guardian died about every 20–30 s.
### JOIN236–240: blocked trigger, recorder cause, aimed-fire limits

- **The urgent dialog blocks progress.** In JOIN236 the walker stalled the moment the
  grant landed, because the urgent dialog takes input. The rifle trigger (`trig_get_weapon`,
  `0x1d:35`) therefore never reported, and Fight Through the Wall and the Fallen step never
  ran. Dismissing the dialog right after the pickup and then walking to (409.8, −797.5) makes
  the trigger report reliably (JOIN237–240).
- **Recorder cause.** RPCS3 registers the game-window shortcut `gw_toggle_recording` (F11)
  only while that window is active. Runs without video have no "Shortcut pressed" line in the
  emulator log. Posting `WM_ACTIVATE` before F11 registers the shortcut without taking focus
  (JOIN238–240, confirmed from the log line); those runs record continuously until the same
  toggle stops them.
- **Aimed fire.** Entity pointers from the `0x138e24` hook are not a reliable enemy list.
  In JOIN238 (134 aimed shots, 8 lives, 14 death/respawn incident pairs) Dregs fought at
  1.5–3 m and the video shows melee, muzzle flashes and repeated Guardian Down, but no kill.
  In JOIN239–240 the Guardian is shot at the rifle before any Dreg appears in the hook stream;
  after respawning, no enemy pursues and the helper finds only fixtures. Dreg deaths, if any,
  produce no host incident. Killing a Dreg is still unverified; it needs a real enemy list
  (squad member actors) or manual play.