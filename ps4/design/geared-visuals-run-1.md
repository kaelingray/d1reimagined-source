# The geared-visuals run - families 3 and 4 were delivered after all

**Run `20260811-050950-GEARED-385-VISUALS-X11`, 2026-08-11.** First boot of the geared fixture set
(`char-geared-385` / `profile-geared-385` / `rosters-f0-geared` / `D1_INVENTORY`) under XWayland.
Clean boot, orbit reached, zero `error.*`, **no porcupine** - the stale-roster fix holds on the wire.

> ## ⛔⛔ RETRACTED 2026-08-11 (same day): "GEAR WAS NEVER SENT" IS FALSE. SEE §7.
> Families 3 and 4 WERE transmitted (push path, `--> BAP QUEUEZ PUSH` lines, formerly labelled
> `--> BAP QUEUEZ CHARACTER`), WERE accepted (replication descriptors class 3 and 4 at state 3),
> and ARE resident in this run's still-live client (registry read, §7). The §2/§3 conclusion below
> came from a census that grepped only the ANSWER-path label `--> BAP FAMILY n`, which the push
> path never emits. The on-screen negatives in §1 happened WITH the full item estate resident,
> so they ARE evidence about the post-delivery layers (render, instance, entitlement).
> §2 and §3 are kept for the record; do not cite them.

> ## ⛔ (superseded by the banner above) THE RESULT IS NOT "GEAR DOES NOT DRAW". IT IS "GEAR WAS NEVER SENT."
> Every negative on screen is explained by one fact: **families 3 and 4 never reached the client.**
> Do not read this run as evidence about arrangements, ordinals, buckets or the render path.
> Those remain **untested**.

---

## 1. What the operator saw `[C]` `observed-live`

| where | result |
|---|---|
| orbit **card** | ✅ **40 / 385, emblem renders** |
| character/pause **card** | 40 / 385, **no emblem** |
| character/pause **3D model** | ⛔ absent |
| orbit **ship** | ⛔ absent |
| primary / special / heavy slots | ⛔ **LOCKED** - *"You have not yet found a Primary Weapon"* |
| applied items | ⛔ triangle/inspect does nothing |
| inventory tab | ⛔ **empty** (⚠ NOT "everything in Materials" - that is a different symptom) |
| *"maximum trial level reached"* | ✅ **did not fire this run** (it did on 2026-08-09) |

## 2. The cause, from `responder.log` `[C]`

```
-> FAMILY SUBSCRIBE ACK 0x000D for class 4 root 0xd1…0001, seq 7      <- acked, NO BODY
-> FAMILY SUBSCRIBE ACK 0x000D for class 0 root 0xd1…0001, seq 21
   ⭐ FAMILY 0 ANSWER seq 2 (2415B body)   -->  --> BAP FAMILY 0 ANSWER 2443B   ✅
-> FAMILY SUBSCRIBE ACK 0x000D for class 2 root 0xd1…0001, seq 22
   ⭐ FAMILY 2 ANSWER seq 3 (179B body)    -->  --> BAP FAMILY 2 ANSWER 207B    ✅
   ⭐⭐ FAMILY 4 DELTA seq 4 (Ping, version 1, **0 objects**, 25B)                ⛔
   (no FAMILY 3 line of any kind)                                                ⛔
```

**The bodies were BUILT and then not transmitted** - the responder logged both baselines:
```
QUEUEZ family 3 baseline: roster (0xa2ac2c74) + character (0x3431f1ba)
QUEUEZ family 4 baseline: account (0x64ae75bf, char list [0xd1…0101]) + profile (0x8127f15a, soid 0xd1…0201)
```
and the item estate was fully staged from disk (`inventory item 0..N`, keys `0xd1…0301+`).

⇒ **`D1_ANSWER_FAMILIES=0,2` answers only 0 and 2.** Families **3** (character object) and **4**
(profile + item instances) were left to the push path (`D1_PUSH_FAMILY=4,3`, `D1_PUSH_AT=echo`),
and **no family-3 or family-4 body was ever sent** - only the 0-object invalidation ping.

⚠ **The mistake I made reading this run, recorded so it is not repeated:** the responder logs
`⭐ character from D1_CHARACTER_FILE (2326 B, oracle-encoded full object)` when it **loads the
fixture**, and the byte count matches the file exactly. I cited that as "the fixture is on the wire."
**It is not.** Loading, staging, baseline-building and *sending* are four different events, and only
the `--> BAP FAMILY n ...` line is evidence of the last one. ⇒ `status/LESSONS.md`.

## 3. What each symptom maps to, given §2

* **Locked weapon slots + empty inventory + no inspect** = the client has **no item estate at all**.
  *"You have not yet found a Primary Weapon"* is the shipped string for a character that owns
  nothing - it is not a bucket-placement bug. Family 4/3 never arrived.
* **No emblem on the character screen, emblem fine on the orbit card** = the card is drawn from the
  **family-0/2 roster** (delivered ✅), the character screen from **family 3/4** (not delivered).
  ⇒ this is the cleanest available discriminator between the two delivery paths, and it is worth
  keeping as a permanent one.
* **No ship in orbit** = ships are item instances. Same cause. `[I]` - no separate ship fixture
  exists either, so this stays confounded until items land.
* **No 3D model in character/pause** = expected regardless; the pause/select preview reads a
  **family-1/1 carrier we have never served** (`design/geared-guardian-fixtures.md` gap 3). This one
  is NOT explained by §2 and is the one genuinely new gap this run confirms. `[O]`
* **No "maximum trial level"** = the entitlement gate did not fire because the level-40 *profile*
  never landed. ⚠ Expect it to RETURN once family 4 is delivered - it is not fixed, it is unexercised.

## 4. ⚠ The tension this run exposes, which must be settled before the next one

`status/TODO.md` carries two standing claims that now collide:
1. *"Make `D1_ANSWER_FAMILIES=0,2` the default - it is what cures **olive**
   (`error.investment_peer_manager`)."*
2. *"Docs saying `D1_ANSWER_FAMILIES` is 'not the fix' are talking about families **3 and 4** - those
   still stand."*

So 3/4 were deliberately kept off the ANSWER path, and the PUSH path was supposed to carry them.
**The push did not fire.** ⇒ `[U]` **why** - and that is the whole next step. Three candidates:
* the `echo` trigger fires but the push is gated on a condition not met in an orbit-only run;
* the push fires and fails silently (no `-->` line ⇒ likely never reached the send);
* `D1_PUSH_AT=echo` is the wrong trigger for this flow and `relay`/`investment` is correct.

## 5. Next, in order (each is cheap and offline-checkable first)

1. ⭐⭐ **Find why the family-3/4 push never sent.** Read the push trigger in `d1-responder`
   (`D1_PUSH_AT`, `D1_PUSH_FAMILY`, `D1_PUSH_CHARACTER`) and determine what it waits for. **Offline,
   free.** A log-grep census across previous runs answers "has this push EVER fired?" - if it has,
   diff that run's conditions against this one; if it never has, it is built-but-unwired, which is
   this project's signature defect and would explain every item-path run to date.
2. **Then re-run with the smallest change that delivers 3/4.** Either fix the push, or A/B
   `D1_ANSWER_FAMILIES=0,2,3,4` **as a deliberate, single-variable experiment** - noting that
   olive is the risk and that its symptom (`error.investment_peer_manager` at rung 30) is loud and
   immediate, so the test is cheap and self-diagnosing. ⛔ Change ONE thing.
3. **Only then** is any statement about arrangements / ordinals / buckets / "does gear draw"
   meaningful. Everything in `design/geared-guardian-fixtures.md` §7 remains **untested**.
4. **Separately, the pause/select 3D model**: serve the family-1/1 preview carrier, or first
   `d1-live watch` the preview mode byte (`0xA98550`'s `param_1+0x120`) to learn whether the widget
   is asking for family 3 or family 1. New code either way - do not bundle it with 1-3.

## 5b. ⭐⭐ THE CHARACTER-SELECT SCREEN IS NOT SKIPPED - IT FALLS THROUGH `[C]`

The operator asked why the boot never showed character select. It is **entered**, and so is the
prologue state - both pass through in one tick. The exact trace:

```
Leaving  'bootflow:prepare_for_orbit'
Entering 'character:signin'
  ui_stage substage  25 -> 27 -> 28 -> 19
Leaving  'character:signin'          (normal_flow)
Entering 'character:prologue'
Leaving  'character:prologue'        (normal_flow)   <- immediate, did nothing
Entering 'cleanup' -> 'setup:orbit_loading' -> 'setup:orbit'
```

⭐ **Substage 27 IS the character-select screen** (`status/TODO.md`: the creation screen is
`character:signin` substage **29**, entered 38 times historically as `27 -> 29`). We reach 27 and
advance to **28**, never 29. So the select stage is reached and immediately satisfied.

### ⭐⭐⭐ The unifying hypothesis: this is the SAME defect as §2, not a second one `[I]`, testable

**Family 3's baseline is literally `roster + character`** - the character *roster* is what the select
screen enumerates. Family 3 was never transmitted (§2). ⇒ the select stage has **nothing to
enumerate**, satisfies itself instantly, and falls through; and the same missing families are why
the character screen has no items. **One cause, two symptoms.**

⚠ Held as `[I]`, because two other explanations are alive and must be eliminated, not assumed:
* **auto-select on a one-character roster** - normal, correct behaviour, nothing to fix;
* **`D1_ORBIT_DESTINATION = 2468353467`** (`last_orbited_destination`, SET this run) resuming
  straight to orbit.

⇒ **The discriminator is free and comes with §5.1's fix:** deliver families 3/4 and re-boot. If the
select screen then *stops and waits*, the fall-through was the delivery failure and both symptoms
close together. If it still flashes past with a populated roster, it is auto-select or the
destination record, and only then is it worth its own investigation. ⛔ **Do not chase the select
screen as a separate defect until §5.1/§5.2 have run** - that would be debugging a symptom of a
known cause.

⚠ Also noted for later: **`character:prologue` enters and exits immediately.** That is the
first-mission state, and *"You have not yet found a Primary Weapon"* is the shipped string for a
character that has not completed it. `[U]` whether a legitimately-geared character is supposed to
skip the prologue (almost certainly yes - it is not a new character) or whether the empty item
estate is what makes the client render the pre-prologue text. §5.2 answers this too.

## 6. What this run DID establish `[C]`

* The **rebuilt geared roster is correct on the wire** - no porcupine, family 0 answered with a
  2415 B / 2-object body, and the orbit card renders **40/385 with the emblem**. The stale-key
  defect found offline is confirmed fixed live.
* `D1_X11=1` boots and reaches orbit cleanly; menus are usable.
* ~~The family-0/2 path is healthy end to end; the family-3/4 path delivers nothing.~~ ⛔ Retracted;
  see the banner and §7. Both paths delivered.

## 7. RETRACTION EVIDENCE - the push fired, the client accepted, the estate is resident (2026-08-11)

Three independent levels, all on this same run.

**7.1 Sent `[C]` (responder.log 202-209).** On the first echo (`D1_PUSH_AT=echo`), the push loop
walked `D1_PUSH_FAMILY=4,3`:
```
QUEUEZ CHARACTER PUSH seq 0 ... (22937B body)  -->  --> BAP QUEUEZ CHARACTER 22965B   <- family 4
QUEUEZ CHARACTER PUSH seq 1 ... (2400B body)   -->  --> BAP QUEUEZ CHARACTER 2428B    <- family 3
```
22937 B = account + profile (20332 B) + 4/2 key-only + 13 items; 2400 B = roster + character.
Family 3 additionally went out inside the message-206 response (2400 B span, `D1_206_SPAN=1`,
new default since 2026-08-10).

**7.2 Accepted `[C]` (live descriptor read, `d1guest descriptors`, selftest 5/5 PASS, liveness L3).**
```
class=0 state=3 REPLICATING   class=2 state=3   class=3 state=3   class=4 state=3 ver=1   class=5 state=3
```
Class 4 at ver=1 means the 0-object delta ping (version 1) was also accepted, despite the client's
one logged complaint (`object-count of zero`, emulator.log 19390 - the ONLY client-side queuez line
in the whole run).

**7.3 Resident `[C]` (live registry read on this run's client, ~45 min after boot, nothing further
delivered in between).** `obj 4:0` account with character list `0xd1..0101`; `obj 4:1` profile key
`0xd1..0201`; `obj 3:1` character key `0xd1..0101` with level 40; `obj 4:3:0..12` all 13 items,
keys `0xd1..0301..030d`, index 13 empty. Item `+0x18 = 0x28` (level 40) - the v2 fixture fix is in
place live, where the 2026-08-09 read had a hash written there.

**7.4 The census, corrected `[C]`.** Across all 270 `runs/*/responder.log`: the push fired in 186
runs and has NEVER failed to fire when armed. The "never fired" claim grepped `--> BAP FAMILY [34]`,
an ANSWER-path label the push path never emits. Positive control: the same scanner sees this run's
`--> BAP FAMILY 0/2 ANSWER` lines. Guard added: the responder now logs
`--> BAP QUEUEZ PUSH <n>B family <f>`, so a family census matches both
`--> BAP FAMILY n ANSWER` and `--> BAP QUEUEZ PUSH .* family n`.

**7.5 Consequences.**
* §3's symptom-to-cause map is retracted line by line. Locked slots, empty inventory, no inspect,
  no ship, no character-screen emblem all occurred WITH the estate resident: they are now real
  evidence about the consumers (render path, instance fields, entitlement and prologue gates).
  `geared-guardian-fixtures.md` §7 is therefore EXERCISED by this run, not untested.
* §5b's unifying hypothesis is REFUTED: family 3 was delivered and counted (signin took selection,
  substage 27, count 1) and the select screen still fell through in one tick. The live alternatives
  stand: auto-select on a 1-character roster, or the `D1_ORBIT_DESTINATION=2468353467` resume
  (it was set this run). Recorded, not chased, per §5b's own rule.
* `[U]` NEW: the trial-level gate did NOT fire although the level-40 profile is resident - §3's
  "unexercised" reading no longer explains it. Mechanism unknown; candidates: the gate samples
  before the push lands, or reads a field the v2 profile changed.
* `[U]` NEW: the 2026-08-09 client REFUSED the same-shaped family-4 push twice
  (`queuez_family_update_begin: ignoring family-update due to unhandled replication state`,
  GEARED-GUARDIAN emulator.log 6591/857807), yet that night produced the items-in-Materials
  observation; this run shows no refusal and full acceptance. Wire differences since: the 206-span
  answer and the family-4 delta ping. Which one flipped acceptance is unmeasured, and moot for
  this run: the live state read outranks it.
