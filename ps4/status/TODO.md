# TODO - the cross-cutting queue

The queue below is the supplied upstream **11 August 2026** research checkpoint.
For this installation's **12 October 2026** state and contribution priorities,
read [the current contributor checkpoint](../../docs/STATUS.md). The exact
01.29 executable is missing here; these historical native observations are not
a local boot acceptance result.

> ## ⭐⭐⭐ 2026-08-11 HEAD OF THE ITEM/VISUALS LANE ⇒ `design/geared-visuals-run-1.md` §7
> ⛔⛔ **RETRACTED same day: "the fixtures were never DELIVERED" is FALSE.** The push fired
> (`--> BAP QUEUEZ PUSH` lines, formerly labelled `QUEUEZ CHARACTER`), the client ACCEPTED
> (descriptors class 3 and 4 at replication state 3), and the whole estate is RESIDENT in the live
> client (registry read: account, profile, character level 40, all 13 items, v2 field fixes in
> place). The "never sent" census grepped the ANSWER-path label `--> BAP FAMILY n`, which the push
> path never emits. Census, corrected: the push fired in 186 of 270 runs and has never failed to
> fire when armed. Guard: the send line now names the family.
> - [x] **1. Why did the family-3/4 push never send?** ANSWERED: it DID send. Wrong census pattern.
>       Evidence and consequences: `geared-visuals-run-1.md` §7.
> - [x] **2. Deliver 3/4, one variable.** MOOT, closed without a boot: delivery, acceptance and
>       residency all proven on the live client. The `D1_ANSWER_FAMILIES=0,2,3,4` A/B would test
>       nothing about delivery; do not spend a boot on it.
> - [ ] ⭐⭐ **3. NOW LIVE: the on-screen negatives are post-delivery findings.** Locked weapon
>       slots, empty inventory, no inspect, no ship, no character-screen emblem all happened WITH
>       the estate resident ⇒ `geared-guardian-fixtures.md` §7 is EXERCISED, not untested. The
>       questions are now consumer-side: render path, instance fields, entitlement and prologue
>       gates. New `[U]`s in `geared-visuals-run-1.md` §7.5 (incl. the trial gate NOT firing with a
>       resident level-40 profile).
> - [ ] **4. Separately:** the pause/select **3D model** needs the never-served **family-1/1**
>       preview carrier (gap 3). Confirmed absent this run. New code; do not bundle with 3.
> - ⭐⭐ **Character-select fall-through: the unifying "same defect" hypothesis is REFUTED.**
>       Family 3 was delivered and counted (signin took selection, substage 27, one tick) and the
>       screen still fell through. Live alternatives, recorded not chased: auto-select on a
>       1-character roster, or the `D1_ORBIT_DESTINATION=2468353467` resume (set this run).
>       ⇒ `geared-visuals-run-1.md` §5b + §7.5.


> # 🎉🎉🎉 2026-08-11 - **A GUARDIAN IS IN-WORLD, RENDERED** ⇒ `design/GUARDIAN-IN-WORLD.md`
> Type-9 biped on screen with the Ghost out, `pvp_moonbase`/407. **First time in project history.**
> ⭐ **SERVED (real, committed):** the `+0x5028` membership fix builds the player record; the **hybrid
> family-0 fixture** (`build-peer --no-equip --no-emblem` → `rosters-f0-hybrid`) clears **rung-34
> task 7 NATURALLY** (done `0x17F→0x1FF`, no poke) and installs the character definition.
> ⭐ **SYNTHETIC (disposable):** done-mask force + 2 code hooks (`FUN_0010C500`, `je@0x411E16`) +
> `player[0]+0x0C=3`. Artifact: `runs/snapshots/guardian-in-world-moonbase.snap` (biped inside it).
> ### The queue from here, in order
> - [~] **1. Earn tasks 9/10 onto the wire** - ✅✅ **SERVED 2026-08-11 ⇒ `design/activity-lifetime-state.md`.**
>       ⛔ Every "NOT a wire field / BlamScript-VM variable / udt 7" framing here and in that doc's
>       §4c/§4d is **refuted**. The `FUN_0010C500` lifetime phase (`+0x1AC` ∈ {3,6}) **IS a wire
>       field**, delivered by AH **udt 5 `sensor_auth_update`** (sim-queue element 0x13 → applier
>       `FUN_002A1210` → dispatch record+0xA8 `{0x8080039D, ord 2}` = **`FUN_0011DE30`**, which
>       `memcpy`s the value to `record+0x1AC`; udt 7 hardcodes the WRONG slot +0x108, unbound
>       map-wide). **The server now emits it:** `d1_oracle::sensor` (11 controls, byte-exact vs the
>       audited frame through the client's own codec) + `d1-activity-host` `AhMessage::SensorAuthUpdate`,
>       armed by **`D1_AH_SENSOR_PHASE`**, sent once after Live+first-heartbeat. Chain executed end
>       to end offline on `rung36-biped-chain.snap` (setter store-boundary probe; getter al=1
>       state=3/6; gate mask 0x48). ⚠ **`[U]` promotion gate:** whether our fresh wire element's
>       slot is in the world post-pass sweep - **settle LIVE** (task #4 below). Until then the two
>       hooks + done-mask force stay. Leg A (`FUN_001192A0`) is a separate wall (`THE-ONE-ROOT-CAUSE.md`).
> - [ ] **task #4 - RUN THE LIVE udt-5 EXPERIMENT (Lane B / main).** Add `D1_AH_SENSOR_PHASE=3` to
>       the in-world recipe (`design/activity-lifetime-state.md` top banner has the exact line + the
>       AH hot-swap). **Discriminator:** `d1guest.py read` of `record+0x1AC` flips `0xFF→3` and the
>       spawn gate passes WITHOUT the `FUN_0010C500` hook / `je@0x411E16` / done-mask force. **Null
>       control:** `D1_AH_SENSOR_PHASE=-1` must change nothing. ⚠ `gen` (default 0) must equal the
>       client's `world+0x1D` or the applier silently skips - read it live, set `D1_AH_SENSOR_GEN`.
> - [ ] **2. Possession / control** - trace what a clean spawn does past `0x411E1C` to bind camera +
>       input. Today the biped stands there; this is the "walk it" line.
> - [x] ⭐⭐ **2b. SOLVED LATER SAME DAY - the in-world screen lever is the FADE BLOCK, not the
>       fly-in byte:** `[0x2694B90]` → `+0x00` count / `+0x14` mode / `+0x18` alpha (1.0f=black),
>       zero all three (`--abs`). RADV-11: operator PLAYED 15+ min (stick, jump, super). The
>       fly-in byte below is still needed - it suppresses the LOAD-screen curtain and deletes the
>       ~5-min prologue wait - but it is not the in-world fade. Serving the release is task #15.
> - [ ] ⭐ **2b-old. The fly-in suppress** `[0x26A33E8]` deref byte0 = 1 (activity-launch.md
>       §4.3 lever A) - **decays, must be HELD** (~2 s re-poke). Serve it instead:
>       (a) lever B is a served session parameter (`FUN_0078af50`: orbit `session+0x59c` name hash
>       == `DAT_026b64a8` AND a u16 via `+0x9b8` != -1) - trace which published param lands at
>       `+0x59c`; (b) `[U]` whether the user-facing "skip fly-in animation" setting backs
>       `FUN_00765070` (would make it a served profile value). Until then the hold is a poke loop.
> - [ ] **2c. Forced-path crash, now NAMED (RADV-9, ~60 s post-spawn):**
>       `c_simulation_sobject_entity_definition::vfunc[19]` (`0xBBDEDE`) reads NULL`+0x4f` - the
>       NOP'd-gate biped lacks its entity-definition backpointer. Find what a legit spawn stores
>       there (likely in the `0x411E1C` construction body the NOP skips half of); this is the same
>       work as #2 possession and probably the same fix.
> - [ ] **2d. The prologue-filler timeout costs minutes per run** - the client waits out its own
>       timer on fly-in content we never serve (~40 s-5 min variance, `[U]` why). Suppress held
>       from ladder start removes the wait AND the log line - gate scripts on the done mask, not
>       the `Timeout entering the prologue-filler` line.
> - [ ] **3. FPS** - ⭐ **RE-MEASURE ON RADV: the "6-7 fps in-game" number was llvmpipe** (Mesa's CPU
>       rasterizer - the auto-picker chose it over the real GPU on every non-RenderDoc run; solved +
>       pinned 2026-08-11, `status/shadps4.md` head). First RADV run: `20260811-121021-GEARED-RADV`.
>       ⛔ `hotkey_show_fps` **F10 CRASHES the client**, and a no-biped in-world screenshot is BLACK,
>       so this needs the operator watching.
> - [ ] **4. Bank a RenderDoc capture** - F12 fires it but the `.rdc` did not persist `[U]`.
>

> ### ⭐⭐ 2026-08-10 LATE - GEARED-GUARDIAN FIXTURES v2: art arrangements + equip ordinals + Light 385
> ⇒ **`design/geared-guardian-fixtures.md`**. Two NEW defects found and fixed offline, both measured
> from the packages: **equipment `+0x0C`/`+0x0E` shipped `0` = a live reference to arrangement #0**
> (the render gate - `FUN_0054B530(0)` resolves a real record, replayed) and **the equipped arrays
> are ordinal-indexed by `def_client+0x18`, not file order** (emblem def says 12 == the replayed
> emblem slot - two instruments, one byte). Plus Light 400 → **385** (`LIGHT_ROI_CAP`) everywhere.
> New instrument `capture-rig/oodle/art-arrangement.py` (9 controls) → `item-art-arrangement.tsv`.
> ⭐ **2026-08-11 offline verification pass ⇒ `design/geared-guardian-fixtures.md` §9:** the on-disk
> `rosters-f0-geared` was **PORCUPINE-STALE** (f0t1 `+0x00` = pre-fix `CHARACTER_SOID`; the client
> would have rejected the peer view before the registry) - **rebuilt + decode-verified**; card and
> profile decode-verified clean, not rebuilt. `def+0x68` **measured EMPTY for all 14 items** and
> element `+0x20` extracted (Oryx Mask 70, all else 0) - art-arrangement.py now **14 controls**.
> - [ ] **Boot the §7 run** (`profile-geared-385` / `char-geared-385` / `rosters-f0-geared`) - the
>       only remaining discriminator for "does gear DRAW".
> - [ ] ⛔ interlock: `rosters-f0-geared` is for ORBIT runs; activity-ladder runs keep
>       `rosters-f0-hybrid`/`rosters-f0` (a populated f0/1 pins task 7's world-tag loader at state 2).
> - [x] ~~fill items' `+0x88`/`+0x98` from `def+0x68`~~ - **MEASURED AND CLOSED (§9.2): `def+0x68`
>       is NULL/empty for every staged item, so the client's own constructor writes nothing - empty
>       is content-correct.** The live stat channel is the talent-grid chain, a NEW extractor
>       against table 156 (§9.4, named `[U]`).

> # 🛰 **2026-08-10 - THE LADDER IS FINISHED. THE QUEUE STARTS HERE.**
>
> **Rung 36 `activity:in_world` reached and reproducible** (`20260809-230309-FAM02-OLIVE`,
> `pvp_moonbase`/407, zero `error.*`) - **and there is no Guardian in it.** Narrative, refutations and
> the two flagged contradictions: **`historicaldocs/status/aug10-session.md`**. State: `CLAUDE.md`'s top blocks.
>
> ⛔ **Everything below this block that frames orbit, the director, rung 30/33/34, `missing_dlc`,
> `establish-client-gate.md` or the renderer as the frontier is HISTORY.** Kept for its evidence, not
> its priorities.
>
>   # ✅✅✅ 2026-08-11 - **PLAYER RECORD CREATED IN-WORLD (served, not poked)** ⇒ `historicaldocs/design/player-in-world-membership-fix.md`
>   The AH membership response left the **simulation-occupancy mask `+0x5028`** zero and dropped the
>   member **fireteam id**; fixed in `d1-activity-host`/`d1-oracle` (108 controls PASS). Live: pool
>   count `+0x18`=1, spawn bit `+0x0C`=3, map `+0xC0` no longer `-1`. **THE ONE ROOT CAUSE is closed.**
>   - [ ] ⭐ NEW FRONTIER: `player[0]+0x644`=0 → no biped. The done-mask force skips tasks 9/10 (world
>         content). Does a **legitimately-completed** task 9/10 build the biped? That is the next wall.
>   - [ ] ⚠ solve the inactivity holder - it still ends every in-world deep read.
>   - [ ] retract everywhere: my "gather binds empty slot1" was a stale-teardown read (see doc §6b).
>
> ### ⭐⭐⭐⭐⭐ 2026-08-10 LATEST - **THE PLAYER ELEMENT IS NEVER ENQUEUED** ⇒ `historicaldocs/design/player-element-never-enqueued.md`
> Measured with an intrinsic control: **176 sim-queue dispatches across a full clean 27→36 walk,
> five cases firing 92 times, case 12 `player_update_apply` ZERO** (its jump-table target is unique -
> no tail-merge confound). The applier and the `elem[1]&1` gate are **retired**; the break is the
> **session→simulation bridge**. Plan of record is that doc's §6, operator-approved:
>   ⭐ **PRODUCER CHAIN NAMED** ⇒ `historicaldocs/design/player-element-producer-chain.md`. The producer is
>   `FUN_00BAA660`; the gather `FUN_00BA3C90` runs 8×/window in-world but returns **count=0**, so the
>   producer never fires (0 hits, clean) - measured. The session-manager player table reads healthy
>   (mask=1, state 10), so the gap is the gather's OWN session resolver `FUN_0075DA50(*(machine+0x10))`
>   returning 0 - the machine's world object doesn't bind to the activity slot.
>   ⭐⭐ NARROWED (join 6): the resolver SUCCEEDS - inst0's world handle resolves to the machine world
>   obj. There are **TWO sessions**: slot0 (state 6, mask 0x01, our player, VALID) and slot1 (state 0,
>   EMPTY). count=0 + slot0 valid ⇒ the gather binds the **empty slot1** (state 0 fails {4..9}). The
>   wall is the **machine→session binding**, not the resolver, not the player filter.
> - [x] ~~name the producer~~ / ~~resolver returns 0?~~ REFUTED - resolver matches.
> - [ ] **NEXT:** confirm the bind - `d1-live watch --rw rw` on slot1's mask (`sess1+0x620+0x2C`)
>       during a join; OR read machine0`+0x18/+0x20` identity within 2 s of in_world (before teardown).
> - [ ] ⚠⚠ **SOLVE THE INACTIVITY HOLDER FIRST** - it has killed the last 3 in-world deep reads; the
>       stick delays but does not defeat it. Find the timeout field, or hold it harder.
> - [ ] tool gap: `d1-live watch` is one-`--addr`/one-tracer-per-process; multi-`--addr` (several DR
>       slots in one process) would give a same-window gather+producer control in one join.
>
> ### ⭐⭐⭐⭐ THE HEAD OF THE QUEUE, 2026-08-10 LATE - **SERVE A TEAM**
> ⇒ **`historicaldocs/design/player-creation-chain.md`**. The player chain is named end to end and the session layer
> is **green** (`Player add completed successfully.`, `players valid: 0x1`). The one red field is
> **`activities:team_picker: … Multiplayer Team=-1`** on **407 Elimination, a team mode** - and
> `d1-activity-host` has **no team handling at all**. Sixth wall of the same shape as the checksum,
> the bubble count, the bubble state, the activity index and the PAH region: *a field of the
> replicated blob we publish as `-1`*.
> 1. **Offline first, no boot:** on `runs/snapshots/rung36-biped-chain.snap` (13/13 controls), find
>    which field of udt 11 `replicate_membership` / udt 1 `global_activity_state` the team picker
>    reads. `d1-eboot descriptors` + `d1-replay replay` on its accessor.
> 2. **Then a knob**, `D1_AH_TEAM`, the way `D1_AH_BUBBLE_COUNT` was built - inverted before shipping.
> 3. **Discriminator, no poke needed:** `P+0x90` (machine slot, live `-1`) or `Q+0x10` (sweep mask,
>    live `0`) becoming valid on their own ⇒ `[C]`, and §1's chain completes by itself.
>
> ⚠ The remaining `[U]` in the chain, with its instrument: does `FUN_00BB3FD0` run and does it create
> the player? **`d1-live watch --rw exec --addr 0xbb3fd0`** - one hit is proof, no candidate list.
>
> ### ⭐⭐⭐ (superseded framing) THE HEAD OF THE QUEUE - the world has nobody in it
>
> - [ ] ⭐⭐⭐ **WHO WRITES THE ROSTER - `gg + 0xD0 + i*0xE8`?** *(`gg = [0x2724950]`, 20 slots ×
>       `0xE8`.)* **This is the actual fix.** All twenty enable bytes are zero, so `FUN_00904AC0` adds
>       **zero** players, no `player array` element is ever allocated, and the per-tick spawn
>       `FUN_00411830` iterates an empty bit vector and returns before the creation record, the
>       definition hash, or anything to do with gear. **577 live sobjects in-world across 8 types,
>       ZERO of type 9 (`biped`).** ⇒ `historicaldocs/design/biped-census.md` §5.1.
>   - ⛔⛔ **THE INSTRUMENT THIS ITEM PROPOSED IS A TRAP - DO NOT RUN IT.** It read: *"`d1-live watch
>     --abs --addr '<gg+0xD0>' --len 8` across a boot. That block has never been non-zero, so ANY hit
>     is the answer."* ⚠ It would **HIT** - `vfunc[2]` → `FUN_009048A0` does
>     `memcpy(gg+8, src, 0x2b40)`, which spans `gg+0xD0`, and a DR breakpoint traps on the **store,
>     not the value**. The write copies **zeros** out of a block `vfunc[1]` just memset. ⛔ So the run
>     costs a boot and returns a confident answer to *"who touches these bytes"* while the question
>     meant *"who FILLS them"*. ⭐ **`xref 0x9048a0` answers it statically in 0.4 s.**
>     ⇒ `status/LESSONS.md` (2026-08-10 entry).
>   - ⭐⭐ **AND THE ANSWER WAS ALREADY IN THIS ITEM, ONE BULLET DOWN.** The line below called it
>     *"a second writer chain … unread"*; it is **the actual door**: `0xBB1BC0 → 0xBAA790 → 0xBA6860
>     → {0x90CBB0, 0x90D1E0, 0x90F530}` - the **sim-queue element `0x0D` player update**. Measured
>     live with `--rw exec`: `0xBB1BC0` **hits unprompted**, `0x90D1E0` **never hits**. **The break is
>     the guarded call at `0xBA73BF`.** ⇒ ⭐⭐⭐ `historicaldocs/design/player-creation-chain.md`.
>     ⚠ *The tree already knew, again* - it was filed as the secondary chain while the primary was
>     memset-by-design.
>   - ⭐ **The roster IS filled from the network, not the change-world command** - which makes it
>     exactly the server surface we owe.
>   - ⛔⛔ **LANDMINE - do not ship the roster fill alone.** `players[i] + 0x1660` (the spawn's inner
>     gate) is compared `!= -1` and the **measured default is `0`, which PASSES**, on all 20 slots in
>     two snapshots. Populating the roster without also asserting `+0x1660` builds a creation record
>     from **definition hash 0** - swapping *"no biped"* for *"a biped built from nothing."*
> - [ ] ⭐⭐ **THE ACTIVITY-LIFETIME-STATE RECORD (game-system-state block index `0x10`).** The
>       player-spawn gate `FUN_00414730` and rung-34 **task 9** call **the same two functions**
>       (`FUN_0010C500`, `FUN_001192A0`) on **adjacent bytes of one content record** - task 9 wants
>       `[+8] != 0`, the spawn wants `[+9] != 0` **and** a state in `{3,6}`. `replay-oracle.md` §9.6
>       already measured that byte as **`0xFF` = not found**. ⇒ **chase the RECORD, not task 9.** The
>       spawn re-reads it every tick and **cannot be forced.** ⇒ `historicaldocs/design/arrival-handoff.md` §6.3.
>   - [ ] **Free first step, no boot:** re-run `d1-replay replay --func 0x10c500` against an
>         **in-activity** snapshot instead of the orbit one (`runs/snapshots/rung36-in-world-*.snap`).
>         If it still returns `al = 0`, task 9 can never complete on this map and the done-mask force
>         is **mandatory**, not merely convenient.
>   - ⚠ Also cheap and *loud* if wrong: `FUN_003D5310()` must be FALSE -
>     `activity_def[+0x1C] != 0 || activity_def[+0x5079] != 0`, two static per-activity
>     **"no player spawning"** flags. For a Crucible private match they should be 0.
>   - ⛔ **DO NOT GREP FOR THE REFUSAL STRING.** *"can't spawn due to the current activity lifetime
>     state"* is `vsnprintf`'d into a stack buffer that is never read - **0 hits is a structural false
>     negative.** Use `d1-live watch --rw exec --addr 0x41480B` (reached only on refusal, so one hit
>     is proof). `LESSONS.md`, `arrival-handoff.md` §6.4.
> - [ ] ⚠⚠ **SETTLE THE FLAGGED CONTRADICTION BEFORE SPENDING A BOOT.** `CLAUDE.md` names the
>       **controller→player map** (`[0x27257F0]+0xC0 = 0xFF…`, suspect `OPERATOR_PAD=1`) as the break;
>       `biped-census.md` §5.1 says that field's **only writer, `FUN_00921790`, runs inside add-player**
>       - which would make it an **effect** of the empty roster, not its cause, and would exonerate
>       `OPERATOR_PAD`. **One command decides it:** `d1-eboot xref 0x921790` plus `field-xref` on
>       `[0x27257F0]+0xC0` - is there a second writer reachable from controller sign-in?
>       ⇒ `historicaldocs/status/aug10-session.md` §3.
>
> ### ⭐⭐ SHIP THESE ON THE WIRE - each replaces a poke we are currently doing by hand
>
> - [ ] ⭐⭐ **THE TEN-FLAG GRANT BODY.** `{98, 308, 419, 440, 834, **2667**, 3389, 5015, **5987**, 6018}`.
>       The shipped union-8 constant is **missing `2667` and `5987`**, which is exactly `urchin`
>       (`activity_failed_to_load_missing_permissions`) - we cleared it by **poking** the plane
>       (`[0x26A34E8]+0xC+idx = 0x02`). Regenerate `INVESTMENT_503_UNLOCKS_RAW` and
>       `INVESTMENT_205_UNLOCKS_BODY` in `d1-server/crates/d1-bap/src/lib.rs` (⚠ **they must ship
>       together**; the responder's startup guard prints `✅ 205/503 agree on the shared 0x80801941
>       (44 bytes)` - if it disagrees, stop). Both walls then fall on the wire with no poke.
>       ⛔ **`historicaldocs/design/rung33-unlock-gate.md` §6.3 has a copy-pasteable EIGHT-flag run line** - bannered,
>       but do not paste it. ⭐ `historicaldocs/design/grants/rung33-permissions-gate.md` and
>       `historicaldocs/design/THE-GRANT-CONTRACT.md` §182 **already carried the six-flag core with 2667/5987**.
> - [ ] ⭐ **Make `D1_ANSWER_FAMILIES=0,2` the default.** It is what cures **olive**
>       (`error.investment_peer_manager`) - `peer failures: 0` where the two prior runs died at rung
>       30. `run-boot.sh:101` still exports it **empty**, so it survives only on a command line. ⛔ This
>       is the project's signature failure (*built and switched off*) for the fourth time.
>       ⚠ Docs that say `D1_ANSWER_FAMILIES` is "not the fix" are talking about families **3 and 4**
>       (`historicaldocs/design/director-selectability.md`, `CHARACTER-CREATE-PLAN.md` §3.2 step 2b) - those still
>       stand. **`0,2` is a different claim and it is measured.**
> - [ ] **Serve the task-7 suppress value instead of poking it.** `[0x26A33E8]` deref byte 0 = 1
>       (`FUN_00765070`, "suppressing cinematics"), and it **resets to 0 on every teardown**. ⭐ D1 has
>       a user-facing *"skip fly-in animation"* setting; if it backs `FUN_00765070` this becomes a
>       served value. Closer: `d1-eboot field-xref`/`xref` on `0x18C4AC8` (the global cinematics
>       enable) for its writer - if it is profile-backed, the re-arm retires. `[U]`, cheap.
> - [ ] **Fix the in-world capture preset** - a **selection** bug, not a result. None of the three
>       `rung36-in-world-*.snap` captured the sobject element array, the players block, game globals or
>       observer globals, so the classic census could not run on them at all. The corrected
>       `d1-replay capture --map …` invocation is `historicaldocs/design/biped-census.md` §8a. ⚠ Verify with
>       `d1-replay info` that each address landed **inside a region** before citing any read -
>       *"not captured"* and *"was zero"* are different facts.
>
> ### ⭐ THE ITEM PATH - four independent defects, all ours ⇒ `historicaldocs/design/inventory-instance-fields.md`
>
> - [ ] ⭐⭐⭐ **The inventory tab is POSITIONAL - fix the PROFILE's slot placement, not the item.**
>       `FUN_0026DB60(scope, slot_index)` maps a *slot index* to a bucket id from a 39-record content
>       table; **slots 0-19 are bucket `0x05` = Materials**, Heavy is **113-122**. `ProfileSpec::item_slots`
>       writes item *n* at slot *n*, so all thirteen land in 0-12. **Nothing about the item is consulted.**
> - [ ] **`element + 0x24` must carry the item's real content index.** It is hardcoded to `1` =
>       *'Bronze Tier'* (`stats:{}`, `equippable:false`, `hasGeometry:false`) - that is the "3 Attack"
>       and the refusal to equip. ⭐ Two content indices exist for one item and **neither is derived
>       from the other**: the profile slot's `u16` (which we fill correctly - that is why the grid name
>       resolves) and the element's `+0x24` (which we never filled - that is why the detail panel does
>       not). Same bug class as MODE-vs-MAP.
> - [ ] **Stop writing hashes into `+0x18` and `+0x1C`.** `+0x18` is the item's **LEVEL**; `+0x1C` is
>       the equip **MINIMUM STAT**. We write an item hash and a `bucketTypeHash` there, both ~10⁹ ⇒
>       every item is un-equippable **by construction**, and as `i32` the hash is -730,737,871, which
>       also floors the Attack curve. ⛔ **The "Requires Level 953998653 ≈ Heavy `bucketTypeHash`
>       953998645, eight away" theory is DEAD** - the `+8` was the coincidence it was flagged `[I]` to be.
> - [ ] **The missing 3D model is a fourth, separate defect** - geometry is an **art-arrangement
>       index at `0x8080183C[3]` in the character RENDER object**, a different object in a different
>       family. The 4/3 element has no art field at all.
> - [ ] ⚠ Two sentinel landmines on this object: `+0x44` (progression ordinal) and `+0x58` (talent
>       grid index) treat **`0xFFFF` as "none"** - our `0` silently selects **grid #0 and leaks its
>       stats**, and a `0xFFFF` at `+0x44` makes the progression iterator reject the item entirely.
>
> ### ✅ Collapsed - done on the night of 2026-08-09/10
>
> - [x] ~~rungs 34 / 35 / 36~~ - **walked.** `orbit → 28 → … → 36` in one boot, three times.
> - [x] ~~rung-34 task 7~~ - cleared by the suppress byte; **task 9 dispatched for the first time.**
> - [x] ~~`baboon` (`missing_dlc`)~~ - union-8 `D1_503_RAW` + `D1_205_RAW`.
> - [x] ~~`urchin` (`missing_permissions`)~~ - cleared **by poke**; the wire fix is the ten-flag body above.
> - [x] ~~`olive` (`error.investment_peer_manager`)~~ - cured by `D1_ANSWER_FAMILIES=0,2`.
> - [x] ~~"the establish gate is the frontier"~~ - solved (BAP **9**, not 100) and walked past.
> - [x] ~~Hypothesis A (renderer/texture-cache), B (gear), C (cinematic/arrival handoff)~~ -
>       **all three REFUTED.** ⇒ **do not un-suppress cinematics**; the done-mask force is exonerated;
>       do not spend a shadPS4 build on the black screen. `historicaldocs/status/aug10-session.md` §4.

> ## ⛔⛔ 2026-08-10 - THE BLOCK BELOW IS CORRECT ABOUT DELIVERY AND **WRONG ABOUT THE MECHANISM**
> Two claims in it are dead, and one of them was steering the whole investigation:
> * ⛔ *"a zeroed bucket lands everything in the default tab. One cause, all four symptoms."* -
>   **the tab is POSITIONAL and never consults the item.** There are **FOUR independent defects**, not
>   one. ⇒ `historicaldocs/design/inventory-instance-fields.md` §0, and its §7.2: *co-occurring symptoms are evidence
>   of a common TRIGGER, not a common cause.*
> * ⛔⛔ *"this is a better candidate than the done-mask poke for 'no biped in-world': a weapon with no
>   bucket is in no equipment slot, so the Guardian has nothing equipped."* - **REFUTED.** The spawn
>   aborts **two gates upstream** of the creation record and of everything gear-related; there is no
>   player record at all. ⇒ `historicaldocs/design/biped-census.md` §5/§7.
> * ⛔ The *"Requires Level 953998653 / Heavy bucketTypeHash 953998645, eight away"* `[I]` is dead -
>   `+0x18` is the item's **level** and we write a hash into it.

> ## ⭐⭐⭐ 2026-08-09 LATE - THE ITEM PATH'S REAL BLOCKER IS THE **INSTANCE**, NOT DELIVERY
> Served `char-lvl40-geared.bin` + `profile-lvl40.bin` + `D1_INVENTORY` for the **first time ever**
> (they had existed unserved since 00:26 the same night). Result, operator-observed:
> * every item lands in the **MATERIALS** tab, not Helmet/Gauntlets/Chest/Legs/Primary/Special/Heavy
> * Gjallarhorn reads **"Requires Level 953998653"** - ⚠ Heavy Weapons' bucketTypeHash is
>   **953998645**, eight away. `[I]` only; the `+8` may be coincidence and must not be asserted.
> * **"3 Attack"**, and **no 3D model** in preview
>
> ⭐ **The NAME resolves**, so the u16 content index in the profile's 340-slot array is CORRECT
> (Gjallarhorn = 6299, hash `0xd471d331`, `capture-rig/item-content-index.tsv`). Delivery works.
> ⛔ **What is empty is the INSTANCE.** Live `d1guest obj 4:3` (184 B stride `0xB8`, cap 666,
> registry cross-check PASS):
> ```
> +0x00 SOID d1000000_00000301 | +0x08 0 | +0x10 1 | +0x18 c3c96257925cdeea | +0x20 1,1
> +0x28 .. +0xB8   ALL ZERO
> ```
> `0x80801732` is **8 fields**: `[1]→0x80801920`, `[2]→0x80801711 {u32,u32,u16}`, `[3] u16`,
> `[4]→0x80801018`, `[5]→0x80801846` (80 B, incl. `0x8080176b {u32,u32,u32,u16}`). **We fill three.**
> ⇒ **Where an item LIVES is instance state, not definition state** - a zeroed bucket lands
> everything in the default tab. One cause, all four symptoms. ⇒ `historicaldocs/design/inventory-instance-fields.md`
> (agent attributing the fields against their consumers; two call sites request family 4/type 3:
> `0x1087746`, `0xABF235` - `historicaldocs/design/item-render-path.md`).
> ⚠ **And this is a better candidate than the done-mask poke for "no biped in-world"**: a weapon with
> no bucket is in no equipment slot, so the Guardian has nothing equipped.
>
> ## ⭐⭐ ENTITLEMENT: A LEVEL-40 PROFILE TRIPS A TRIAL GATE AN EMPTY PROFILE NEVER DID
> Serving the geared profile produced **"maximum trial level reached"** + **"buy Destiny - The
> Collection"** (flashes once at start). Live state, measured:
> ```
> ownership_authority (0x26A4A74) = 2   -> the LOCAL store is authoritative
> local  0x26A4970 count@0x26A4A70 = 2  -> {0xE0000002 paid tier, 0xE0000001 full sku}
> signon 0x263D46C count@0x263D56C = 0  -> all 0xFF, NEVER SENT
> ```
> ⇒ the client believes we own **base game only**. **Level 40 is a Taken King cap**, so a level-40
> character on a base-only entitlement is refused. ⛔ **This gate is invisible with an empty profile**
> - it only appears once you serve content the entitlement does not cover.
> ⚠ **Poked 16 speculative keys** `0xE0000003..0xE0000012` + count 18 (client filter is
> `(key & 0xF0000000) == 0xE0000000`). **The real DLC offer-key values remain `[U]`** - only
> `0xE0000001`/`0xE0000002` are attested. The upsell still flashed. **Do not cite the sweep as a fix.**
> ⇒ Next: read what `sceAppContentGetAddcontInfoList`'s consumer actually compares against, and/or
> finally send **`/SignOn` field 10 `id_list`** (`0x2A7090`) - a server surface never once exercised.

**Convention:** this file holds only what's *blocking* or *next across areas*. Area-specific lists
live in `shadps4.md`, `server.md`, `cfw-console.md`, `harness.md`. Check items off in place; when a group is
done, collapse it to one line under **Done**. Keep this file short - if it grows past a screen,
consolidate.

**Scope and distance to the goal: `status/ROADMAP.md`.** This file is the queue; that one is the map.

---

## ⭐ NEW 2026-08-09 - `d1-live` is built and controlled but **never pointed at shadPS4**

- [ ] **Run the watch once against the live client** and close the last `[U]` in
      `historicaldocs/design/live-watch.md` §10: do shadPS4's guest threads take a hardware watchpoint at all?
      The exact command is `live-watch.md` §6; it is read-only, costs no boot, and can be armed while
      the client sits in orbit. Two things to look for in the report: threads listed as `⚠ not armed`,
      and a non-zero `unattributed sigtraps` (⇒ the emulator uses `SIGTRAP` itself; try `--slot 3`).
      ⛔ **Ctrl-C to stop it, never `SIGKILL`** - a tracer that dies armed kills its target (`[C]`).
- [ ] **The question it was built for:** who writes `c_activity_loader+8`? ⭐ **That `[U]` is now
      CLOSED statically** - `FUN_00341500 @ 0x341798`, found via `field-xref`'s *escape* rows one
      indirection down (`historicaldocs/design/activity-loader-writer.md`; `rung33-world-transition.md` §4b is
      retracted in place). ⇒ **use it as the watch's KNOWN-result control** (ATLAS §4.4d), and watch
      the **pointee**, not `+8` - `+8` is a stable heap handle and will never fire:
      `--addr '[[v:0x267AF08]+0x8]+0x0'`.
- ⭐⭐ **The rule both instruments encode - STATE THE ACCESS MODE BEFORE CHOOSING THE SCANNER.**
      Absolute xref answers *"who touches this ADDRESS"* and is blind to `mov rax,[rdi+8]` **by
      construction**, controls and all; `field-xref` is the static field instrument, `d1-live watch`
      the runtime one. Every `[U]` writer/reader must name which one closes it.
      ⇒ `status/LESSONS.md`, `historicaldocs/design/ATLAS.md` §4.2e.


> ## ⭐⭐⭐ NEW 2026-08-09 (later) - **"ANTEATER" WAS OURS, AND SO IS INDEX 0.** ⇒ `historicaldocs/design/ah-schema-codec.md` §13
>
> Two independent bugs in `d1-activity-host`, both fixed, neither yet run.
>
> 1. ⛔ **`AhConnection::on_tick` wrote UNENVELOPED, UNSEALED frames.** The heartbeat's only path
>    returned `d1-bap` *inner* frames straight to the socket, so the client read our
>    `msg_type >> 8` as the envelope version: **`Failed to parse envelope version '0' (expected 1)`**
>    → `_connection_failure_unknown_message_type` → `[status 33][change reason 101]`
>    `error.private_activity_host_lost_connection_to_bap`. **522 ms after the first heartbeat**, and
>    the connection was RST. ⭐ Two negative controls: the no-heartbeat run has 0 of these across 22
>    connections, and the one connection *in the same run* that never reached a heartbeat was never
>    killed. ⇒ one `seal_all` with two triggers; **57 controls**, 4 of which fail when inverted.
> 2. ⛔ **The activity index rung 33 gates on is a `u16` WE PUBLISH** at
>    `global_activity_state+0x680` (`FUN_0078C770` → `FUN_002A1F50` = `world+0x50B0` = our blob),
>    and we sent **0**. `HASH(0x8AE8DA69)` = **"Tower"** = `activity_table[0]`, nothing more.
>    ⛔ **The repair machine is NOT why** - the client was `EST-Y … MEM-1 STATUS: [ok]` at the moment
>    of the refusal. Banner added in place on `rung33-unlock-gate.md` §11. ⇒ **`D1_AH_ACTIVITY_INDEX`**.
>
> **Next run** (both fixes, plus the union-8 flags so nothing is confounded):
> ```sh
> D1_ACTIVITY_HOST=1 D1_AH_BIND_PORT=4110 D1_AH_ACTIVITY=act_cosmo_m0_highway \
> D1_AH_ACTIVITY_INDEX=393 \
> D1_503_RAW=180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100 \
> D1_205_RAW=1bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100 \
> SHADPS4_BIN=~/dev/shadPS4-d1trace/build-d1trace/shadps4 \
>     capture-rig/run-boot.sh RUNG33-ENVELOPE
> ```
> **Discriminator:** no `Failed to parse envelope version`, no `Soft KTO … error_lost_connection_to_bap`,
> no `private_repair`, and the rung-33 line naming `HASH(0x6488b18a)' index '393'`. Expect
> `World-transition requires permissions.` (`0x18`) next - `rung33-unlock-gate.md` §7.

> ## ⭐⭐ NEW 2026-08-09 - RUNG 33's `missing_dlc` IS A **SYMPTOM**. **`historicaldocs/design/rung33-unlock-gate.md`**
>
> The gate reads **UNLOCK STATE**, never ownership (`0x28CACD cmp byte [rdi+rax], 2`; watchpoints over
> both offer-key stores fired zero times while the plane fired). ⭐ **The grant pipeline is PROVEN
> LIVE** - 205/503 → the client's derived plane, four bytes at `0x02` at the exact evaluator
> addresses, run `20260809-075545`.
>
> ⛔ **And the gate still refused - because the client gates on activity index `0`, not 393.**
> Index 0's expression reads `{3389, 308, 419, 440}` and never touches flag 98. The shipped flags
> were irrelevant, not insufficient.
>
> ⭐⭐ **Index 0 is itself downstream.** `error.private_activity_host_lost_connection_to_bap`
> (reason **`0x65`**, "anteater") fires at rung 32 and starts the **private-activity repair state
> machine**, which runs `cleanup_activity_clients` and de-instantiates the slice-set. Rung 33 then
> reads `blob+0x680 = 0`. **9/9 occurrences across two runs, the AH error precedes the refusal every
> time.** ⇒ **THE BLOCKER IS THE ACTIVITY-HOST CONNECTION DYING, not content and not unlocks.**
> Handed to the activity-host lane.
>
> **Next run - removes rung 33 as a confound; do NOT expect rung 34:**
> ```sh
> D1_503_RAW=180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100 \
> D1_205_RAW=1bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100 \
>     capture-rig/run-boot.sh RUNG33-UNION8
> ```
> `{98, 308, 419, 440, 834, 3389, 5015, 6018}` clears **28/28** offline (indices 0-6 and 393-413);
> empty plane 0/28; every index +1 → 0/28. `d1-responder` prints
> `✅ 205/503 agree on the shared 0x80801941 (44 bytes)` at startup - if it disagrees, stop.
> ⛔ `D1_UNLOCKS` (family 4/4) is the **wrong object**; it is SOID-keyed and nothing reads it.
> ⚠ Ruled out with controls: the second (value) plane is never read; the value must be exactly `2`
> (1 and 3 both fail); reason `0x17` has exactly **one** emitter in 47,304 files, so we did not pass
> 33 and fail downstream. Profile/account unlock arrays are **not** needed - 205/503 alone persists.

> ## ⭐⭐ NEW 2026-08-09 - RUNG 30 IS REPRODUCIBLE AND THE WALL IS ONE QUESTION
>
> **`historicaldocs/status/SESSION-2026-08-08.md`** is the night's narrative. **`CLAUDE.md`'s WHERE-ARE-WE** is the state.
>
> Rungs 27→30 walk on one poke, 4×. The AH payload codec is SOLVED (it is the client's own QueueZ
> object codec - only the tag was missing). **All four establish messages - udt 4, 0, 1, 11 - encode,
> transit, and PROVABLY ARRIVE**, proven by the client's own message-9 mirrors on the primary BAP
> connection. **And `CON-Y EST-N AH->0 MEM-0` does not move.**
>
> ⛔ **STALE - see the `✅ SOLVED` banner immediately below, and the top of this file: rungs 30-36
> have since walked.** ~~⇒ **The frontier is `historicaldocs/design/establish-client-gate.md`**~~: why the client ignores four correctly
> delivered messages. Transport and encoding are proven; what is left is semantics, a missing
> precondition, or a **silent routing drop** (message 100's `recipient_id` is `[I]` and a wrong value
> is a silent drop, not an error).
>
> ### ✅ ⭐⭐ SOLVED 2026-08-09 - **WE WERE SENDING THE WRONG BAP MESSAGE**
>
> **`historicaldocs/design/establish-client-gate.md`.** BAP **100** is the AH → **BAP-server** hop, not the AH →
> client hop: its deliver step (`0x564C20`) reads the class-6 header's `+0x06` as a *connection
> index* and re-emits the body as BAP **9** on it. We send `+0x06 = 0` = `OUT PRIMARY`, so the
> client forwarded all four pushes to `d1-responder` - that is what the "message-9 mirrors" were.
> `c_activity_client::receive_message_from_host` never saw one of them, and the status was **26
> `timed_out_waiting_for_join_response`**, so `FUN_00298EF0` was never even evaluated.
> ⇒ `d1-activity-host` now pushes **BAP 9** (`D1_AH_BAP_PUSH_ID=100` reproduces the failure), and
> `d1-responder` decodes inner type `0x0009`. Whole workspace green; `d1-activity-host controls`
> **42/42**.
>
> ### The queue, in order
> - [x] ⭐⭐ **Close the establish gate** - wrong message id, not a false flag. See above.
> - [x] ⭐ **Decode BAP message 9** in `d1-responder` (header class 0) - done; it now prints
>       `{recipient_id, udt (named), payload_len}` and diagnoses the relay in place.
> - [ ] ⭐⭐ **RUN IT.** No launcher change needed. Discriminator, from the client's own status line:
>       `EST-Y AH->d1a0…:…0001 MEM-1` = all four landed → expect status 6 and rung 31;
>       `EST-Y … MEM-0` = a router ORDER race (`FUN_0075AB60` matches udt 0/1/11 only against
>       `[ac+0xC98]`, zero until udt 4 is applied) → gate 0/1/11 on the client's udt 17;
>       inbound message-9 frames still arriving = we are still sending 100.
> - [ ] **Re-run with the new fixtures** - `char-lvl40-geared.bin` and `profile-lvl40.bin` now carry
>       **real item content indices** (emblem *The Reflective Proof* = 1512 at slot 12).
>       ⚠ Needs `D1_INVENTORY=capture-rig/inventory`, which `run-boot.sh` does NOT set.
> - [ ] **`historicaldocs/design/d1-dashboard.md`** - the debug dashboard (workflow running). Its hard requirement:
>       *a gate that succeeds with an empty answer is invisible.*
> - [ ] ⚠ **The virtual pad does not bind.** shadPS4 took the physical Stadia controller; the ignore
>       list reached the process and the pad still never drove it. **The next run needs the operator
>       at the title screen.** Worth fixing - it is the only thing preventing an unattended run.
> - [ ] ⛔ **`capture-rig/eboot-callgraph.tsv` fails 4/11 controls** (cannot see `lea`-materialised
>       function pointers; its original 7/7 were all direct calls). Do not use it for gate discovery.
>       A negative in `rungs-29-to-36.md` §5.6 rests on it.
>
> ### Free capability, already built, still off
> `D1_ROSTER_DIR` (family-2 contents) · `D1_UNLOCKS` · `D1_FAMILY42` · `D1_CHARACTER_LIFECYCLE` ·
> and the `d1-oracle` message encoder, which generates correct bodies for **97 of 101** investment
> ids while the responder still answers 7 by hand.
> ⚠ **Wiring the encoder naively is a TRAP**: today an unhandled id fails LOUDLY
> (`bitstream error when decoding message-arguments`); after, all 22 stubs would decode cleanly and
> become **silent empty successes**.

> ## ⭐ NEW 2026-08-08 - the CHARACTER LIFECYCLE is implemented, and it needs ONE RUN to falsify
>
> **`historicaldocs/design/character-lifecycle-messages.md`** - the four client→server character messages
> (`THE-BLOCKS.md` B11), resolved and built. **501 create · 502 delete · 504 login · 505 signout**,
> from the FNV-1 name hash in the shipped definition table, confirmed by the literal immediates.
> ⛔ **Retracts "501 is `login_character`"** in three live docs.
>
> ⭐⭐ **The finding that outranks the handlers:** we have told the client a Guardian already exists
> on *every run this project has ever done*. `D1_CHARACTER_SOID=0` produced a list of **one character
> with SOID 0**, not an empty list; the zero-character path was the `unused variable: char_list`
> warning. Fixed and pinned by a codec-verified test. Census of `20260808-223926-CHARMSG-LISTEN`:
> none of the four messages has ever fired.
>
> ⛔⛔ **CORRECTED 2026-08-09 - "none of the four has ever fired" is a ONE-RUN census, not a fact.**
> Across all **231** `runs/*/responder.log`: **501 fired 9× in 8 runs, 505 fired 2×.** The creation
> screen has been entered **38 times** (`ui_stage` substage `27 → 29`). **Four 501s carry a full
> 52-byte body** and decode at 397 bits - ⭐ **the create transcript already exists, four times over.**
> ⇒ **`historicaldocs/design/CHARACTER-CREATE-PLAN.md`** - the plan of record: creation is hosted by rung 23
> `character:signin` substage 29, the predicate is `FUN_0078C9F0` (family-3 root count at `+0x08`),
> and ⭐⭐ **"Finish does nothing" and "the director isn't working" are ONE defect** - family 4 stuck
> at `replication state=1`, fixed by `D1_ANSWER_FAMILIES`, which `run-boot.sh:101` exports **empty**.
>
> - [ ] ⛔⛔ **SUPERSEDED 2026-08-09 - re-derived by the `creation-aftermath-recon` workflow, and TWO
>       of these three were WRONG.** ⇒ **`historicaldocs/design/CREATION-BUILD-ORDER.md`** is the build order.
>       ~~(a) the empty-roster short-circuit bypasses `D1_PROFILE_FILE` ⇒ three variables~~ - **do the
>       OPPOSITE:** pre-501 it must serve **no profile at all** (`+0x68 = 0`, no 4/1 object), or slot B
>       resolves and the creator closes on its first settled tick.
>       ~~(b) `D1_ANSWER_FAMILIES` must include **4**~~ - **wrong:** measured live, family 4 reaches
>       **state 3** without it (the push path promotes it); adding it is an untested variable on the
>       most expensive run of the project.
>       ✅ (c) 105 **is** a padded-empty stub - that stands, and it is now **SOLVED**: body
>       `1800000000` = `character_lifecycle::RESULT_OK_BODY`, already in the crate. Wiring, not authoring.
>       ⭐⭐ **And the item none of the three had:** answering 206 with the pinned constant at Finish
>       **PURGES family 4** (state 3 + `flags&1` ⇒ purge-to-keep-list, deleting the profile). It is a
>       **mandatory safety interlock**, not an optimisation.
> - [x] ~~Collect a create transcript~~ - already on disk in `20260807-144230-char501`,
>       `20260808-045053-HOLD-SELECT`, `-045454-CREATE-ANSWER`, `-045817-CREATE-ROSTER`.
> - [ ] **Run it**, for the thing we do *not* have: the **activity-selection record** at
>       `DAT_026A33E8` (`0x130` B) that rung 27 consumes - see the plan's §5.3.
> - [ ] `[U]` with a mechanism: which CUI 64-bit path hash fires each component - read
>       `0x27409A8` / `0x27411C0` / `0x2745940` / `0x2746A60` and build a 64-bit path-hash oracle.
> - [ ] Fix `THE-BLOCKS.md` §3.2 row 2 downstream consumers: the CUI byte at
>       `c_investment_login_info_component_v2 + 0x88` is a **controller** index, not a character
>       index, and answering 504 does not set it. (Corrected in place in `THE-BLOCKS.md` itself.)

> ## ⭐ NEW 2026-08-09 - THE ITEM CONTENT INDEX IS MEASURED (gear, model, emblem)
>
> **`historicaldocs/design/item-content-indices.md`.** The `u16` this client keys every item by is the **element
> index of a 7,544 × 24 B table in `ps4_investment_globals_012f_3.pkg` entry 8126** (element `+0x00`
> = the item's content hash, `+0x10` = its world tag). It is the argument of the client's own
> definition lookup `0x25ADE0` (`investment_globals->vtbl[0x410](u16)`), and that is *executed*, not
> inferred: replayed on `runs/snapshots/orbit-all.snap`, the client's own arithmetic lands on the
> element carrying each item's hash, and the 181,056 element bytes in memory are byte-identical to
> the package entry. All 13 staged items + a real emblem now have measured indices.
> **The emblem SLOT is 12** (`investment_globals->vtbl[0x160]()[0x18]`, replayed `0x0C`).
>
> ⛔ **RETRACTED in five docs: `N = tag - 0x80A60297`.** The world-tag table is contiguous only for
> its first **112** indices - 633 discontinuities over 7,544, and `0x80A6035A` is index **197**, not
> 195. *"Verified 4-for-4"* was a claim about four points.
>
> Tools: `capture-rig/oodle/item-index.py --controls` (7, exit 2) ·
> `capture-rig/item-content-index.tsv` (7,544 rows) · `d1-oracle`'s `item_index` module.
> Fixtures regenerated: `capture-rig/characters/char-lvl40-geared.bin` (emblem at slot 12) and
> `capture-rig/profiles/profile-lvl40.bin` (**the 340-slot array now NAMES 13 items** -
> `item-render-path.md` §0 link 3, crossed).
>
> - [ ] **Run it.** `D1_CHARACTER_FILE=capture-rig/characters/char-lvl40-geared.bin
>       D1_PROFILE_FILE=capture-rig/profiles/profile-lvl40.bin D1_INVENTORY=capture-rig/inventory`.
>       ⚠ Without `D1_INVENTORY` the 13 SOIDs name nothing. The falsifier is the **emblem** - it is
>       the only field whose slot is measured rather than assumed.
> - [ ] The **in-world** emblem needs an emblem *item instance* (`d1-forge` / `loadouts/*.json`):
>       its SOID in profile `+0x2B80[12]` and in a 340-slot with index 1512. Only the
>       character-SELECT card's emblem is shipped (it needs no SOID).
> - [ ] `roster.rs` may now ship family 2/1 `+0x20 = 1512` instead of `0xFFFF`
>       (`orbit-hud-blank.md` §7.7 item 1, superseded).
> - [ ] ⚠ **Only slot 12 of the 20-slot equipment space is measured.** Everything else keeps the
>       staged inventory's file order - `[U]`.
> - [x] ⭐ **DONE 2026-08-11. `item-index.py` AND `art-arrangement.py` are ported.**
>       `d1-oracle {oodle,item-index,art-arrangement} {controls,write,diff}`. **The blocker was
>       imaginary**: there was never a codec to port - `oodle-decompress.py` is a 35-line wrapper
>       around three eboot entry points, and `d1-oracle` already ran the client's code under Unicorn
>       (`harness.rs`). Both TSVs regenerate **byte-identically** apart from the `# GENERATED by`
>       line, proven by an executed `diff`. ⛔ "The Oodle decode is ~30 s under Unicorn" is
>       **retracted**: 0.83 s per block in Python, faster in Rust; the minutes were
>       `extract-entry.py` shelling out to `d1-pkg info` once per package across 1,647 packages.
>       ⇒ `d1-server/crates/d1-oracle/src/{oodle,item_index,art_arrangement}.rs`.
> - [ ] `content-table.py` and `verify-sweep.py` still load the Python harness. They are **audit
>       tools, not on any binary's runtime path**, so this is cleanup, not a defect - each is now
>       one call to `d1_oracle::oodle::Content` away from being Rust.
> - [x] ✅ **CLOSED 2026-08-11.** `gate.rs` now takes an injected `ProbeDecoder`; the `d1` CLI
>       wires in `d1_trace::probe_decode` (injection, not a dependency - `d1-trace` depends on
>       `d1-run`, so the direct edge would be a cycle). `find_script` and the script-path-keyed
>       repo-root discovery are deleted. The quiet-note degradation is preserved but now only
>       fires for a real reason (unreadable or refused trace bytes). Proven by `d1 gate` over the
>       pinned 17,862-record run with a poisoned `python3` on PATH: native tally 8/15 identical
>       to the canonical decoder, zero Python spawns. Same change: `harvest-crash.py` ported to
>       `d1 crash` (stack-mode golden diff byte-identical over 17 real ticket_drop addresses).
>       ⇒ `status/harness.md`.

> ## ⭐⭐ THE HEAD OF THE QUEUE (2026-08-08) - **`historicaldocs/design/SERVER-ROLES-PLAN.md`**
>
> **The plan of record for rungs 28 → 36 is written.** Seven recon agents; all findings reconciled;
> six contradictions with the previous brief called out in its §0. **Read that document before
> writing any BAP code.** Background on how we got here: `historicaldocs/design/RUNG-28-REACHED.md`.
>
> ⛔ **PHASE 0 IS A HARD GATE.** Nothing past rung 28 is observable until the BAP connection
> survives. Every later phase is measured by a client diagnostic a fataled client cannot emit.
>
> **The killer is ours, and it is an ORDERING bug, not a missing codec.** The client keeps no
> recipient table: two strict-FIFO in-flight rings, **head-only** match on `(expected_response_type,
> sequence)`, and a miss is an unconditional fatal (`0x12`). Both types that killed rung 28 -
> id 11 `client_to_world_server_response` (= our `0x000B`) and id 13
> `client_to_bap_subscription_response` (= our `0x000D`) - **are already implemented, under other
> names.** ⚠ The re-subscription-churn story in `RUNG-28-REACHED.md` §4b is *unsupported*; it is
> head-of-line blocking. See `SERVER-ROLES-PLAN.md` §0.4.
>
> - [ ] **Phase 0 - `d1-bap::BapConversation`**, the correlation table. Six rules (order · totality ·
>       exactly-once · sequence echo · never-unsolicited · never-invent-a-response-class) that make
>       an orphaned response impossible *by construction*. Spec: **§2**, the API is written out.
>       `reply_header` becomes private; handlers park **bodies**, never headers.
> - [ ] **Phase 0 - two generated tables, with controls that exit 2** (`d1-eboot bap-policies`,
>       `d1-eboot bap-header-classes`): `RESPONSE_FOR` from the client's **policy** vtables
>       (slot 2 = request id, slot 6 = response id; `resp == 0` means *no reply is legal*), and
>       `header_class` / `HEADER_LEN = [6,6,8,14,6,24,32,8]`.
> - [ ] **Phase 0 - reconcile `0x000C`/`0x000D`.** Canonical
>       `TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST/_RESPONSE`; keep `TYPE_FAMILY_SUBSCRIBE_*` as
>       `#[deprecated]` aliases; **DELETE `TYPE_QUEUEZ_SUBSCRIBE`** (zero call sites, and the
>       client's QueueZ family is `0x79`-`0x7C`). Migration in **§5.3**. Do this *before* touching
>       the subscription path.
> - [ ] **Phase 0.5 - OBSERVE. DO NOT BUILD.** One run plus three live reads closes four `[U]`s:
>       the timeout tunable at `[0x267B1B8]+0x2684` (⚠ `--abs`), whether a **posse session** exists
>       (`FUN_00773F30(1)`), and the ring capacity. Plus a **zero-cost** log replay of the two
>       existing rung-28 runs to confirm the ordering violation. **§7.**
>
> **⛔⛔ SESSION PARAMETERS: THE CLIENT IS THE AUTHORITY. THE SERVER CAN NEVER SET ONE.**
> `0xB8D630` is literally *"am I the host peer of this session"* (states 6-9 are the four `host-*`
> states). Parameters replicate **peer-to-peer** over the game's own session channel (msg `0x25`/
> `0x26`), **not over BAP**. Any plan of the form *"the server delivers
> `s_activity_host_connection_information`"* is impossible - delete it wherever it appears.
> **Our only two levers are BAP message 7 and message 17.** The chain is in **§4.1**.
>
> - [ ] **Phase 2 - message 6 → 7** (`activity_host_manager`, fixed 137-byte reply). The reply that
>       hands the client an AH id. Falsifier for the whole chain: if message 16 never appears
>       afterwards, the `[I]` link in §4.2 is wrong.
> - [ ] **Phase 3 - message 16 → 17** (`get_activity_host_proxy`, fixed 16-byte reply
>       `{u64be echo, u32be ipv4, u32be port}`). ⭐ Success is a named string:
>       `AH IP resolution successful: '%s -> %s'.`
> - [ ] **Phase 1 - MM configuration is message 42 → 43, NOT 34/35.** Minimum accepted reply is
>       *field 4 present*. ⚠ The nonce failure is a **symptom of the lost connection**, not a gate -
>       expect it to vanish at Phase 0, in which case Phase 1 leaves the critical path.
> - [x] ⚠ **Do not push message 102.** It is absent from the client's 13-entry push accept-set
>       (`0x1925A90` = `{6,8,9,20,27,28,47,100,123,124,160,161,301}`). Pushing it *is* the Weasel.
>       ✅ **Message 100's 32-byte class-6 header is READ and implemented** (2026-08-08) - with
>       classes 3 and 5 alongside it, class 3 confirmed `observed-live`. Phase 4 is unblocked.
>       ⇒ `historicaldocs/design/bap-header-classes.md`, `d1_bap::HeaderExtras`.
>       ⚠ Next risk is **addressing, not framing**: the four class-6 fields default to zero and
>       `[I]` says the activity-host id belongs at `+0x18` (slot pair 8/9, the same pair that
>       carries it at `+0x06` in class 3). A wrong value is a **silent drop**, not an error.
> - [ ] `[U]` **Why the orbit handler's `+0x38` debounce stamp was never set.** We poked past it;
>       that is a hop, not an answer, and it is probably server-side.
>       ⓘ It is now `this->debounce_ms` in Ghidra - see the struct block below.
>
> **⛔ THE CEILING, stated up front (§8):** the Activity Host's mission logic is **cloud-side Lua**
> (Truman, GDC 2015). It is not in the eboot and not in the PKGs. **Reaching rung 36 with a stub
> yields a loaded but EMPTY world** - geometry, sky, your character, movement; no enemies, no
> objectives, no completion. That is the honest outcome, and it is still the preservation result.
>
> **New instruments** (⚠ Python prototypes - **owed a Rust port**, see CLAUDE.md):
> `capture-rig/state-machine.py` (35-rung ladder, 11 controls) - **PORTED 2026-08-11** to
> `d1-eboot ladder` (byte-identical golden diff vs the prototype; ~27x faster; the script stays
> until `d1-trace dash/ladder.rs` repoints to the subcommand) ·
> `capture-rig/bap-message-table.py` + `.tsv` (all 58 BAP message ids, 9 controls) ·
> `capture-rig/session-params.py` (session parameters + `--launch-poke`, 7 controls).
> ⚠ **Three standing corrections** to those instruments and their docs are listed in
> `SERVER-ROLES-PLAN.md` §10 - `bap-message-table.tsv`'s `pair_id` is a memory-layout artefact and
> its `category` column is really the **header class**; `session-params.py` has two wrong sizes.
> Docs: `historicaldocs/design/activity-launch-ladder.md`, `historicaldocs/design/bap-message-table.md`,
> `historicaldocs/design/RUNG-28-REACHED.md`, **`historicaldocs/design/SERVER-ROLES-PLAN.md`**.

> ## ⭐ GHIDRA NOW HAS STRUCTS (2026-08-08) - `historicaldocs/design/ghidra.md` §10
>
> `*(long *)(param_1 + 0x38)` reads **`this->debounce_ms`**. 49 structures, 238 fields, `this` typed
> on **43** functions, 5 accessors named + signed. `./ghidra/d1-ghidra.sh structs`; the layouts are
> a data file (`ghidra/structs/d1-structs.tsv`) generated by `ghidra/gen-structs.py` (9 controls),
> so **adding a class is an edit, not code** - §10.6 is the recipe.
>
> ⛔ **Retraction it forced:** `cleanup+0x6A0`/`+0x6B0` and rung 28's `+0x118`/`+0x128` are **task
> bitmasks, not a queue head/tail pair.** Rung 28 is waiting for **7 named tasks** to report done,
> not for a queue to drain. Retracted in place: `historicaldocs/design/activity-launch-ladder.md` §1.3, with the
> four-instantiation derivation (`8 + N*0x20`, N = 3/7/11/51) that settles it.
>
> - [ ] **Read the 7 task entries at `rung28 + 0x38`** (`s_world_controller_task_entry[7]`:
>       prereq mask + pointer-to-member). That names, one by one, what rung 28 is blocked on -
>       and it is readable live, not another elimination.
> - [ ] ⚠ Typing `this` **costs the caller's guessed call-site arguments** (`FUN_005441c0(x)` →
>       `FUN_005441c0()`). Measured with a control: the cause is committing *any* signature, not
>       the struct. `historicaldocs/design/ghidra.md` §10.4 has the mitigation if it ever matters.
> - [ ] `[U]` 2,584 of the 2,633 RTTI classes are still label-only.

> ⭐⭐ **`historicaldocs/design/THE-CHAIN.md` is the new spine - cold boot → in-world, in order.** Read it before
> picking anything below: its **§7 dead list** names 20 claims that are already killed (five docs
> still assert them with no banner), and its **§8** lists 25 places where two *current* docs
> disagree. Its **Appendix A** is the refusal ladder and **Appendix B** the chain-ordered
> must-answer list.
>
> ⛔ **Two corrections from that pass that change what you should do:**
> 1. **The name hash is FNV-1, not FNV-1a - and it was already `[C]` since 2026-08-06**
>    (`sku-config-format.md` §5, `content-manifest-gate.md` §8), while 8 docs said FNV-1a and two
>    task lists still schedule an **FNV-1a-first** preimage effort that cannot work. Verified here
>    with a measured noise floor: FNV-1a is **exactly at noise**, FNV-1 is **5.75×**.
> 2. **`0x753540`'s missing caller is FOUND** - it is a queued command dispatch at **`0x34C0BC`**
>    (slot 3, registered at `0x752F47` via `0x34BDC0`, registry `[0x267B110]`). The xref *did* report
>    it as a `riprel`; it was read as noise. `world-launch-command.md` §4/§5.3 updated.

> ⭐ **The binary map is generated now.** `capture-rig/eboot-atlas.py --addr 0xNNN` before you
> disassemble anything by hand; `historicaldocs/design/eboot-atlas.md` for what it covers (41.0%), `historicaldocs/design/eboot-atlas-roadmap.md` for how to get the rest and what it
> cannot (§5, the honest 62%). It settled §8.1, §8.4, §8.5c, §8.5o and **§8.5t** by measurement -
> **the family-4/4 checksum is `0xC4060975`** and `eboot-map.md` §4.3 is corrected.
> ⚠ New trap, already paid for: **the prologue count is not the function count** (`ATLAS.md` §4.2d).

> ⭐⭐ **2026-08-08 - WE CAN READ THE CLIENT AS C NOW. `historicaldocs/design/ghidra.md`.**
> Ghidra 12.1.2, the eboot loaded at *our* addresses (5 byte controls PASS), seeded with the atlas:
> 48,040 functions, 9,749 strong names, 638 named import stubs, 2,633 RTTI classes.
> **The whole binary is already decompiled to disk** - 47,304 `.c` files, 248 MB, at
> `~/dev/ghidra-projects/decompiled/`. **Grep that before hand-disassembling anything.**
> Start: `./ghidra/d1-ghidra.sh verify` then `ghidra.md` §8. Sizing is not a constraint - 500
> functions decompile in 2.2 s, the entire binary in 191 s.
> ⚠ Ghidra output is `[I]`, always. `d1-eboot` still owns bytes/xrefs/relocations; `d1-oracle` still
> settles behaviour. `ghidra.md` §5 is the three positive controls, quoted, all agreeing.
>
> ⭐ **It already paid for itself: rung 28's requester is named** - `0x7AB460`, inside **`0x7AAF60`
> = `c_world_controller_state_setup_orbit_handler::vfunc[7]`** `[C]`, none of the three candidates
> `THE-CHAIN.md` §5 guessed. The census missed it because the state immediate is set and then
> **`jmp`s into a call site shared with state 25** - `ATLAS.md` §4.2's blind spot, a fourth time.
> Rung 26 stays `[U]` **for a stated reason**: `cleanup_handler::vfunc[7]` transitions to a *stored*
> state (`[param_1+0x6c8]`), so no immediate exists to find. `ghidra.md` §5.4.
> **Not yet promoted into `THE-CHAIN.md` - operator's call.**

## Now (pick from here)

- [ ] ⭐⭐⭐ **TEST THE CHARACTER-SCREEN FIX (built 2026-08-10, UNTESTED).**
      ⇒ **`historicaldocs/design/character-screen-consumers.md`.** The screen showing **0/0** is
      `c_cui_investment_character_state_component::vfunc[14]` (`0x9A3C30`), which reads **family 1
      or family 0/1**, *never* family 3 - `0xA110F0` is the only function in the binary that
      resolves family 3 and it builds the *select tiles* (the screen that already reads 40/400).
      **Four independent defects, all now fixed and all controlled:**
      family 0/1 `0x808017FD` was never built (its tags have sat named in `roster.rs` since
      2026-08-08 - the signature failure, 5th time) · `element+0x1C` carried a `bucketTypeHash`
      where the client reads the **equip minimum stat** ⇒ the **padlock** · the inventory tab is
      **positional** and we wrote item *n* at slot *n* ⇒ everything in **Materials** · the
      **spinner** is a content-residency fact (`request+0x1B`), still `[U]`.
      New: `d1-oracle inventory {layout,controls,rebuild}`, `d1-oracle character build-peer`,
      `capture-rig/inventory-buckets.tsv` (all **39** bucket records, measured live **and** by
      replay on a different boot, with an inverted not-found control).
      **Launch:** `D1_ROSTER_DIR=capture-rig/rosters-f0`,
      `D1_PROFILE_FILE=capture-rig/profiles/profile-geared.bin`,
      `D1_CHARACTER_FILE=capture-rig/characters/char-geared.bin`,
      `D1_INVENTORY=capture-rig/inventory`. Outcome table: that doc's §7.
      ⚠ Bisect the nameplate against `D1_ROSTER_DIR=capture-rig/rosters` - family 0 **substitutes**
      the rich fill `0xA2D9F0` for the compact one.
      ⭐ Next measurement if it lands: the **equipped-array ordinal** - `0x269080` derefs
      `character + 0x2B80 + slot*8` where `slot = def_client+0x18`, i.e. **the item's own definition
      names its slot**, and we fill that array in file order. Same positional trap, one array over.

- [ ] ⭐⭐⭐ **WALK THE ACTIVITY STATE LADDER - states 28 → 36.** This is "getting in game".
      **World loading is SOLVED** (any of 28 `act_*` names; `historicaldocs/design/ACTIVITY-LAUNCH-SOLVED.md`) and
      **shadPS4's transition crash is FIXED** (`texture_cache.cpp:438`, 225 non-fatal warnings,
      client survived). What remains is that swapping the world does **not** move the bootflow
      state machine: the client sits in **27 `setup:orbit`** with the new world underneath, so the
      screen is black with the orbit menu over it.
      `THE-CHAIN.md` §5 already names a requesting site for every rung:
      29 `0x7A6E7C` · 30 `0x7A9ECE` · 31 `0x7A636C` · 32 `0x7ACFB5` · 33 `0x7ACBBF` ·
      34 `0x7A7E36` · 35 `0x795165` · **28 is `[U]`** (one of `0x797420`/`0x797530`/`0x7A3750`).
  - [ ] Find what state 28 `setup:activity_session_creation` requires - it is the first rung and
        the only one without a known requester.
  - [ ] ⭐ **The state package is mapped:** `[0x2724950]+8+0x12F8`, `0x650` bytes - the command is
        literally `{u32 size} + this block`, and **99% is invariant; only the world name differs.**


- [x] ~~⭐⭐⭐ **FORCE AN ACTIVITY LAUNCH**~~ - **DONE 2026-08-08.**
      `world_controller: successfully changed world to: act_cosmo_m0_highway` - **A Guardian Rises**,
      from a command we wrote. Two writes: blob into `registry+0x200+3*0x1C04`, then
      `state[3]=2`; the client's own pump dispatches. The activity is named by an ASCII string at
      command **`+0x630`** (NOT the 50 activity slots at `+0x10`). **28 real `act_*` names
      recovered from live memory.** Tool: `capture-rig/queue-command.py`.
      Full writeup: **`historicaldocs/design/ACTIVITY-LAUNCH-SOLVED.md`**.

- [ ] ⭐⭐⭐ **⚠ "the blocker is shadPS4" is **NOT established** `[U]`** - 5/5 successful world changes crash the renderer, 0/18 refusals do, but nothing distinguishes *emulator limitation* from *we skipped the teardown*. (⛔ A "the queue is the dev console" reframe was asserted and **retracted**: the atlas's `~` marks `string-ref`, its weakest naming class, not a console command.) **Next: read the 0x650-byte block at `0x904B50()+0x12F8`** - `0x754320` builds its command FROM it, so it is the likely state package. ~~THE BLOCKER IS NOW shadPS4, NOT DESTINY.~~ Every *successful* world change kills the
      emulator (`texture_cache.cpp:438 ResolveOverlap: Unreachable code!`, 3/3); every *failure* is
      clean and the client survives. Boot-time transitions are fine - **re-entering a world after
      boot** is what breaks. ⇒ next work is in the shadPS4 fork's texture cache.
  - [ ] Then re-fire `act_cosmo_m0_highway` and read what it asks for next - expect the Activity
        Host layer (`activity_host_connection_data`, `activity_host_manager_request`,
        `world_server_request`, all present in the eboot).

- [ ] ⚠ **RETRACTED: "the availability cache is built then destroyed."** That was an address-
      translation error - heap pointers were read through `d1guest read` WITHOUT `--abs`, which
      adds the eboot base and lands in unmapped memory reading as zeros. With `--abs` the cache
      reads `{1,1,0}` = **built and successful**. Correct
      `historicaldocs/design/availability-and-item-gate-probes.md` §7, `THE-BLANKS.md` I2, and any doc citing it.


- [x] ~~⭐⭐⭐ **RUN THE INSTRUMENTED BOOT**~~ - **DONE 2026-08-08**,
      `runs/20260808-110410-AVAIL-ITEM-TRACE`, 5,641 records, 0 guard failures.
      **Both answers, and both kill a theory:**
      **(a) the availability builder SUCCEEDS** (AL=1 on both post-boot invocations; the one failure
      at `0x2603EC` is early-boot only) ⇒ `director-selectability.md`'s central claim is **refuted**
      and the item below is struck; **(b) `0x3491A0` returns load state 2** ⇒ the item chain's last
      unverified link **passes**. Also `[C]`: 679 of 681 profile slots name **content index 0** -
      `light-and-gear.md` §1.2's zero-vs-`0xFFFF` bug, measured.
      ⚠ The 680/681 gate "pass" rate is a **dedup-bitmap artefact** (`0x5557AC bt/jb`); only **3**
      real load-state checks happened. Count distinct tags, never records.
      Full writeup: `historicaldocs/design/availability-and-item-gate-probes.md` §6.

- [ ] **Collapse the three copies of the UD2 record layout.** `struct.Struct("<IHHII15Q48s48s")` is
      written out in `capture-rig/decode-account-trace.py:16`, `ready-byte-report.py:30` **and**
      `gate-check.py:33` - a live violation of `design/format-description.md`'s no-format-twice rule.
      ⚠ It has **already cost a wrong answer**: `ready-byte-report.py:60-64` records reading field
      index 3 instead of 2, which returns the constant `232` for every record, so *every* probe read
      `NEVER FIRED` and the tool reported a broken chain that was not broken. Caught only because
      232 is recognisable. Fix = the two consumers import the reader from `decode-account-trace.py`.
      Found while correcting `historicaldocs/design/trace-viewer.md` §2, which had claimed it was described once.

- [ ] ⭐⭐⭐ **WHAT ZEROES THE AVAILABILITY CACHE?** - *the first causal explanation of the blank
      director; everything before this was elimination.* **Measured live 2026-08-08** (run
      `…-1080-C`, liveness L3): the builder wrote `AL=1` twice (seq 4977, 5046) to `0x37f8bd338`,
      the singleton still points there (`ctx+0x38 = 0x37f8bd330`, intact), and that memory reads
      `{0,0,0}` - stable across three reads, not a race.
      **`header[0]=1` is written UNCONDITIONALLY at `0x260438`, so `{0,0,0}` is unreachable from the
      builder.** Something else zeroes or frees the block. Not `0x5745F5` - that NULLs the *pointer*,
      which survived.
  - [ ] **Do this first, it costs one run and no code:** `d1guest watch` the 3 header bytes and find
        *when* they go to zero relative to the `avail_rearm_pump` sequence numbers. Converts
        "something clears it" into a bounded window.
  - [ ] Then probe the teardown path - the `investment/registry` cluster
        (`0x574340`/`0x574440`/`0x574460`) that references the singleton.
      Evidence: `historicaldocs/design/availability-and-item-gate-probes.md` §7.

- [ ] **The `world_launch` command is captured** - `historicaldocs/design/world-launch-command.md`. Self-describing
      `0x18F8`; **50 activity slots all `{0x811C9DC5, 0xFFFFFFFF}` = unset**; **world named by a
      NUL-terminated ASCII string at `+0x630`**. We hold `ambient_bootstrap`, not `orbit`.
  - [x] ~~⚠ **Fix the re-arm before re-running.** … There is a THIRD entry into `0x753540` that
        call/jmp/riprel/rela scanning misses.~~ **ANSWERED 2026-08-08** - it is a **queued command
        dispatch**: `0x34C0BC` in `0x34C010` does `lea rdi,[r14+rax+0x200]` (the slot buffer) then
        `call qword [r14+r13*8+0x80]`. `0x753540` is **command slot 3**, registered at `0x752F47` by
        `0x752ED0` via `0x34BDC0`; registry singleton `0x34C140` = `[0x267B110]`. The riprel scan
        *did* find it (`lea rdx,[rip+0x5f2]` in `0x752ed0`) and it was read as noise.
  - [ ] **Re-arm `WorldChangeEntry` from `0x34C0BC`**, which demonstrably runs, then capture an
        `orbit` command and diff it against the held `ambient_bootstrap` one.
  - [ ] **Cheap, no code:** `d1guest read 0x267B110` - if the registry base is `0x205036210`, the
        slot-3 buffer address is derivable and an in-flight rewrite gets much easier.
  - [ ] ⛔ External poke is ruled out: the command is transient - its address reads all zeros in the
        live client after the launch returns.

- [x] ~~⭐⭐ **THE DIRECTOR IS BLANK BECAUSE AN AVAILABILITY CACHE FAILED AND STAMPED ITSELF VALID.**~~
      **REFUTED 2026-08-08 by probe.** The builder **succeeds** (`AL=1`) on both post-boot
      invocations and `ctx+0x28` is **never null**. The `{1,0,0}` snapshots that motivated this
      caught the first, early-boot invocation of a function that runs three times.
      `historicaldocs/design/director-selectability.md` carries the refutation banner; its *structure* is still
      good and is what made the probes possible. Evidence:
      `historicaldocs/design/availability-and-item-gate-probes.md` §6.1.
  - [ ] ⚠ **`0x260C20`, the accessor for `[ctx+0x20]` = family 4/4, has ZERO references** - control in
        the same sweep: `0x260C10` (family 4/2) has 25 and sits at vtable+0x20. `director-and-unlocks.md`
        §6's ranking is inverted; corrected there.
  - [ ] The cache is never rebuilt afterwards: only `0x574440(family==4)` clears the validity byte,
        and family 4 is at **`replication state=1`** (guest log), so every family-4 update is ignored.
        Fix replication first - `queuez-debug` §2 - or nothing below it is measurable.
  - [ ] Offline experiment not yet run: `--probe` the five `[rsp+0xbf]` clears while replaying
        `0x260200` on `orbit-items4.snap`, where family 4/2 *was* present and the verdict was still 0.

- [ ] ⭐⭐ **THE INVENTORY ITEM IS ORPHANED - fix the PROFILE, not the item.**
      Artifact: **`historicaldocs/design/item-render-path.md`** (built 2026-08-08). The client never enumerates the
      family-4/type-3 inventory registry; it walks a **340-slot array inside the profile object**
      (`0x80801743`, C+`0x38`, stride 32: `u16 index @+0`, `u64 item SOID @+8`) and looks each SOID up
      via `0x56CA70(reg, 4, 3, &soid)`. **Exactly two call sites in the whole eboot request family
      4/type 3** - `0x1087746` and `0xABF235`. Our profile's 340 slots are **all zero**, so nothing
      references the item we delivered. **340 (profile) + 326 (account, `0x80801803` C+`0x4C0`) = 666**,
      the measured registry capacity - the confirmation, not a coincidence.
  - [ ] ⚠ **`0x0000` is not empty - `0xFFFF` is** (predicate `0x1056640`: `cmp word[rsi],0xFFFF; setne`).
        Fill all 340 profile slots and all 326 account slots with `0xFFFF` first; today the client
        reads 666 phantom items of index 0.
    - [x] ~~The **340 PROFILE slots** are done~~ - `capture-rig/profiles/profile-populated.bin`,
          built by `d1-oracle profile build` (9 controls, exit 2 on failure).
          **`historicaldocs/design/THE-EMPTY-PROFILE.md` §7.** The 326 ACCOUNT slots are still zero.
  - [ ] **Item identity on this path is a `u16` index into a content table, NOT a 32-bit item hash.**
        Invert `[0x26A0910] → 0x5484F0 → {i32 count, i64 off @+8, 0x18-byte header, count × 16 B,
        world tag at +8}` to find the index whose tag == `0x4BF4BE3F`. Needs a live client past
        content load (`d1guest read`, read-only) or `d1-pkg`. **This is the one number we lack.**
  - [ ] Cost check already done: the arrays are fixed-width - populated and zeroed profiles both
        encode to **20,318 B / 162,539 bits**, so the family-4 baseline size does not move.
  - [ ] Then read `0x1055A30` - the *visible* iterator's advance, i.e. the UI's real visibility rule,
        the highest-value unread function. And watch for `0x162EF5E` *"fill investment item cache
        for 0x%llx"* (fn `0x576C00`), never yet seen.
  - [ ] ⚠ Correction for `loadout-framework.md`: `{itemHash, bucketTypeHash}` is **not** directly
        expressible on the wire. There is no bucket-hash field on this path; slot *position* is the
        only positional information, and identity is the u16 index.

- [ ] ⭐ **LEVEL / LIGHT / APPEARANCE all live in the SAME profile object - `historicaldocs/design/level-light-customization.md`.**
      Independent walk of `0x80801743` reproduces that doc's 340/326 item slots exactly, and adds:
      **progressions** `+0x3660` (u32 count, then 127 × `{u32 progress, u32, u32, u16 index}`),
      **equipped gear** `+0x2B80` (20 × u64 item SOID), **stat override** `+0x6CD0`
      (`{bool present, 7 × {u8 stat_index, u32 value}}`), **customization** `+0x0C` (36 B, 12 fields).
      Light is a *stat*: computed by summing equipped gear (`0x265EF0`) **unless** the override block
      is set - so light can be made non-zero with **no gear at all**. Level in-world is derived from a
      progression; the level *number* on the character-select card is a plain u32 at family-3 `C+0x68`
      (screen we are never on - which is why the 11/22/33/44 markers changed nothing).
  - [x] ~~The two numbers we lack: the **u16 progression index for `character_level`** and the
        **u8 stat index for Light**~~ - **BOTH CLOSED 2026-08-08 from content.**
        ⭐ **`character_level` = progression ordinal 117**; **`Light` = stat ordinal 0** (and the
        `0x281F70` discriminator is now named: **`Defense` = 1**; `Attack` = 11).
        Extracted from `ps4_investment_globals_012f_3.pkg` - progressions block 1066 entry 8132
        (247 × 56 B), stats block 515 entry 580 (36 × 16 B). The content **states** the progression
        ordinal in a one-element array, `{0x6650C0F9, 117}`, and a snapshot scan finds 117 present
        while the two other on-disk candidates (116, 109) are absent.
        ⇒ **`historicaldocs/design/progression-table.md`**; tool `capture-rig/oodle/content-table.py --controls`
        (5 controls, exit 2 on failure) - **owed a Rust port into `d1-pkg`.**
        ⚠ The ordinal is a property of the **content version**, not the format (109 → 116 → 117
        across 2014 / 2015 / 2016 packages), and the definition element grew 40 → 48 → 56 bytes.
    - [x] ~~**Light's stat index is `0`**~~ - closed offline by snapshot replay (`0x281F50` →
          `rax & 0xFF = 0`, positive control `0x276220` → 40 = the level cap).
          `historicaldocs/design/THE-EMPTY-PROFILE.md` §4.
    - [x] ~~**The override block is BUILT and shipping**~~ - `+0x6CDC = 1`, entry 0 `{0, 400}`.
          ⛔ **It is at `+0x6CE0`, 7 entries - NOT `+0x6CE8`/6.** `light-and-gear.md` §3 and
          `level-light-customization.md` §4.3 are retracted in place; the `+0x6CE8` reading is now a
          **negative control** that must keep failing (`THE-EMPTY-PROFILE.md` §7.1).
    - [ ] ⭐ **NEXT, and it is one flag:** `d1-oracle profile build --progression 117:400000`.
          The writer was already wired and a control proves the 7-bit count word at `+0x3660` gates
          the table. Cumulative XP to level 40 is **346,000**; whether the cap wants `≥ 326,000` or
          `≥ 346,000` is `[I]` (the step-walk was not read), so write above it and read the
          displayed level back before pinning a number. `historicaldocs/design/progression-table.md` §7.
  - [ ] ⚠ **`d1guest tag` cannot resolve manifest content hashes** - `index = tag & 0x1FFF`,
        `slot = f(tag>>13)`. Any 32-bit value "resolves" if that slot is populated. False-positive
        generator; see §6.2.
  - [ ] ⚠ **Family 2 is NOT progressions** (`investment-content-map.md` §3 corrected in place).

- [ ] ⭐ **RUN IT: the family-4/type-4 object, `D1_UNLOCKS=key`.** Everything offline is done and
      built - checksum `0xC4060975` (4 controls), key-only payload `bfffffffffffffff00` (round-tripped
      through the client's own decoder, both near-miss neighbours excluded), `d1-bap` 60 tests,
      responder wired, **default off**. Evidence: **`historicaldocs/design/director-and-unlocks.md`**.
      **Earth's director book is gated on NOTHING** (empty `isVisibleExpression`), so this may be
      sufficient on its own - which it was not expected to be.
  - [ ] Read **three** signals, not one: `0x574220` returns the manager singleton in *both* cases and
        is **not** acceptance. Watch post-call `RAX` at `0x5742C2`, the manager's `+0x20`, then
        whether Earth is selectable.
  - [ ] Cheapest first pass is the **late-orbit key-only poke** (rank 3) - same information, does not
        disturb the known-good boot route. Only then the opt-in wire delivery.
  - [ ] Then click `A Guardian Rises` (`1846390409` / `1856964953`) - level 1, party 1-3, **not
        matchmade**, Old Russia. Not patrol: patrol takes the public-session path deliberately.
  - [ ] Classify the first new diagnostic: `0x753540` activation · `0x7A5250` private setup ·
        `0x75F710` AH startup · `0x760470` awaiting `activity_host_connection_data` · `0x77AC20`
        public matchmaking · `0x7A8D10` matchmaking started · `0x7A9160` all peers joined.
  - [ ] ⚠ Do **not** fold into the sign-in baseline until a full boot regression passes - family
        4/type 4 also touches the character-prologue path, and orbit was reached with it *absent*.
- [ ] **Find where the 7,731 unlock flags' state lives.** Best guess: a bitfield in the 28,792-byte
      profile object, which we already serve and can edit with `d1-oracle`. Highest-value offline
      question after the run above; nobody has looked.
- [ ] **The second array in family 4/type 4** - 32 × `{u8,u64,u16,u16}` @+0xc88. Nobody has looked.

- [x] ~~**The REPLAY ORACLE is built: `d1-server/crates/d1-replay`**~~ - **DONE, 2026-08-07.**
      `cargo test -p d1-replay` **5/5**; `d1-bap` 36/36 and `d1-eboot` 23/23 unchanged; workspace
      `--release` clean. Artifact doc: **`historicaldocs/design/replay-oracle.md`**. Snapshot the live client's
      memory → rehydrate into Unicorn **at the client's own addresses** → execute chosen functions
      with register dumps, single-step tracing, watchpoints and **candidate search**. **10/10
      controls PASS** on a real snapshot, three of them reproducing known results independently
      (`0x2A8570` executes to **3**; `0xB45E20(rdi=0)` faults at **0x8**; registry cross-check
      **18/18**). Licence audit: adds **no new licence surface** - a subset of `d1-oracle`'s deps
      plus `iced-x86`, which `d1-eboot` already carries.
      ⚠ **It cannot capture registers** and says so everywhere; the claim it supports is *"input X
      makes this function hit"*, never *"the client's input was X"*.
  - [ ] **⚠ FIX THE NULL-GUARD PATCH - it returns `true` on not-found.**
        `patches/CUSA00219-lab-manifest-T1.xml`'s guard on `0xB471E0`/`0xB47260` justifies itself
        with *"`al` is already 0 from the prologue"*. **Measured false by execution**
        (`historicaldocs/design/replay-oracle.md` §5.3): `RAX` is reused as the table cursor
        (`add rax,[rcx]` at `0xB4721B`, `add rax,0x24` each iteration), so the guarded exit returns
        **`al = 0xe8`** - i.e. the lookup *silently claims success*. Every run since
        `20260807-180823` carried it. The patch is **annotated in place, not changed** (editing a
        patch line mid-experiment confounds the run in flight). A correct guard must land on a
        `xor eax, eax` before the `ret`.
        Also measured on the post-voice-fix snapshot: slot 0 **is** registered (id 0) and the key
        `[G+0xf8]` is 0, so the lookup now **hits** - **the guard may be unnecessary; re-measure
        before keeping it.**
  - [ ] Get a snapshot deeper than `bootflow:profile_setup`. The one on disk
        (`runs/snapshots/20260807-215142-profile-setup-L2.d1snap`, L2) predates
        `character:signin`/fireteam-session creation, so anything about *that* instant is still
        unmeasured. **Ask the operator to take one during a run** - `d1-replay capture` is read-only
        and takes ~10 s; do not start a run to get one.

- [x] ~~**The load-bearing static instruments are Rust now: `d1-server/crates/d1-eboot`**~~ -
      **DONE, 2026-08-07.** `cargo test -p d1-eboot` **23/23**, `cargo test -p d1-bap` still 36/36.
      One crate replaces five Python tools and the `BIAS = 0x4000` constant that was copy-pasted
      into ~12 scripts. Equivalence verified against the originals: descriptor index **4,555 tags
      identical**, **0 field-record mismatches over every descriptor**, schema walks byte-identical
      (pinned as golden fixtures generated by the *Python*), 52/52 RIP-relative sites identical,
      relocation lookups identical. ~100× faster (xref 5.8 s → 0.036 s).
      **Two defects found by the diff, both in the Python:**
      1. ⚠ `find-direct-call-xrefs.py`'s own docstring advertised `--control 0x571940:0x5719f2`,
         which **FAILS** - `0x5719f2` calls `0x5858c0`. A tool whose example control fails invites
         exactly ATLAS §6.1's "fix it by editing the constant". Tool deleted; the correct control is
         pinned in `tests/controls.rs`.
      2. ⚠ `disassemble-elf-range.py` printed `warning: disassembly stopped after 0x16 of 0x18` and
         exited 1 on a **clean** range, because it hands capstone exactly `blob[first:last]` and the
         last instruction straddles `end`. That is indistinguishable from the data-island truncation
         the warning exists to report. Tool deleted; `truncated_at` is now only set when decoding
         really stopped.
  - [x] ~~**The NATIVE ORACLE is Rust: `d1-server/crates/d1-oracle`**~~ - **DONE, 2026-08-07.**
        `cargo test -p d1-oracle` **14/14 in 0.18 s**; `d1-bap` 36/36 and `d1-eboot` 23/23 unchanged.
        A new crate rather than a `d1-eboot` module, on two grounds: `d1-eboot` is documented as
        *nothing here executes anything*, and `unicorn-engine` is **GPL-2.0-only** - confining the
        strongest copyleft in the tree to one crate boundary keeps `d1-eboot` permissive-dep-only.
        Licence audit before adding: unicorn + unicorn-sys GPL-2.0, bundled QEMU GPL-2.0,
        `glib_compat` LGPL-2.1, build deps (bindgen/cc/cmake/pkg-config/heck) MIT-or-Apache - all
        GPL-2-compatible with our GPL-2.0-or-later. Built with `default-features = false,
        features = ["arch_x86"]`, so nine unused QEMU targets never compile.
        **What it buys beyond speed:** the three things the skill could previously only *ask* for
        are now impossible to skip - the control is a precondition on construction, an under-length
        payload is `OracleError::ShortPayload` rather than a clean-looking object, and the
        **exact** non-zero-qword set is assertable. All three pinned vectors reproduce
        (character 97 bits · account 221 bits / `+0x00 +0x08 +0x10 +0x68` · family-4/4 67 bits),
        as do the roster/profile bodies and the message codec's 503+206 selftest.
        `oodle/queuez-object-oracle.py` → thin wrapper.
  - [ ] ⚠ `oodle/rsat-schema-encode-oracle.py` is the **last full duplicate of a codec
        invocation** - kept only because `rsat-kind-wire-survey.py` and `rsat-array-encoding.py`
        import its `SchemaOracle` class. Port those two probes onto `d1_oracle::MessageOracle`, then
        reduce it to a wrapper like its sibling. Until then it is marked PARTIALLY SUPERSEDED in its
        own docstring and must not be used to derive a payload.
  - [x] ~~**Next port:** `message-catalogue.py`~~ - **PORTED 2026-08-11** as `d1-eboot catalogue`
        (`d1-server/crates/d1-eboot/src/catalogue.rs`); it consumes `d1_eboot::descriptors`, so the
        re-declared anchor/sentinel constants are gone. Golden diff EXECUTED: full output, both
        section flags and the `jq -S` JSON all byte-identical to the prototype; the full Python
        output is pinned as `d1-eboot/tests/fixtures/message-catalogue-full.txt` and re-diffed by a
        `#[test]` on every run. Promotion record: `design/capture-rig-audit.md` section 3c.
  - [ ] **Next port, in this order:** `type-registry.py` + `name-hash-tables.py` +
        `namespace-walk-verify.py` (three readers of the same registries).
  - [ ] ⚠ **Three Python disassemblers remain and are now redundant**: `oodle/dis-eboot.py`,
        `oodle/dis-annot.py`, `oodle/rsat-encoder-disasm.py`. All three are subsets of
        `d1-eboot disasm`. Left alone only because the `oodle/` lane was live; retire them next.
  - [ ] `[U]` `0xB2E030` has **no** static reference of any of the four kinds, with all four
        controls passing. That is now a citable negative - it is reached indirectly (computed
        dispatch or a runtime-filled pointer). Worth one probe.

- [x] ~~**THE INVESTMENT_SIGNIN GATE**~~ - **PASSED, 2026-08-07** `[C]` `observed-live`
      `synthetic_ancestor`. §26.41/§26.46/§26.47/§26.48. `record+0x18` has ONE non-zero writer
      (`0x58585E`) storing the return of `0x583450` = *"is there a (class 4, type 0) registry object
      keyed by the account SOID?"* - a **cached lookup result, not a flag**, which is why no response
      body could set it. Cleared by pushing a class-4 account object as `TYPE_QUEUEZ_OBJECT` **after**
      the client subscribes (`D1_PUSH_FAMILY=4 D1_PUSH_AT=echo`). All four substate-B predicates pass.
      Ladder now reaches **`bootflow:profile_setup` → `bootflow:prepare_for_orbit`**, two rungs
      further than this project has ever been.

- [x] ~~**THE WEASEL**~~ - **CURED, 2026-08-07** `[C]` `observed-live`. §26.49/§26.50/§26.51.
      `error.lost_bap_connection` was **caused by our own reply**: a header-only
      `investment_message_response` for id 701. With `D1_INV_SILENT=701` the connection stays open
      past keepalive 22, **no `error.*` fires at all**, and the boot holds at
      `bootflow:prepare_for_orbit`.
      **THE LAW, now twice-confirmed: in this protocol, answering wrongly is worse than not
      answering.** `0x00AB` (XETROV), `0x0020`, and 701's response are three cases where our answer
      was the defect. `N -> N+1` pairing is a regularity, not a rule.
  - [ ] `[U]` Does 701 eventually need a *correct* response? Row 82 has rsp tag `0x8080189B`, so a
        response class exists - it wants one, just not the empty one. Not blocking.

- [x] ~~**MILESTONE: character select, with a character, from the wire**~~ - **2026-08-07** `[C]`
      `observed-live`. §26.69. Run `20260807-161008-roster-account-2`, gate-check **10/10**, BAP to
      3961B. Nothing injected - roster + account + character all arrived as QueueZ baselines.
      Operator: *"we made it to character select. character shows as an option."*

- [x] ~~**THE WEASEL WAS `0x000C`**~~ - **CURED, 2026-08-07** `[C]` `observed-live`. §26.74. BAP
      **head-of-line blocks**: `0xB2E940`/`0xB2E030` examine only the *head* of each in-flight queue,
      so one unanswered request times out and blocks every later reply behind it. `0x000C` (the real
      family-subscription request) had gone unanswered **since the project's first run**. Answering it
      → 130 messages, never closed, zero errors. ⚠ **We had refuted this fix earlier the same day**
      - the refutation was measured on an already-poisoned connection, and it watched the wrong
      observable (subscription state, which the reply cannot move). All five in-flight replies are now
      derived from the client's own tables: `0x000D`, `0x0016`, `0x0021`, 701 and 504.

- [x] ~~**⭐⭐ THE CHARACTER GATE**~~ - **OPEN, 2026-08-07** `[C]` `observed-live` `synthetic_ancestor`.
      §26.76/§26.77. Run `20260807-175730-field3-plus-702`: **zero `error.*` in the whole run**,
      `character:signin` → `character:prologue` → `cleanup (normal_flow)` → **fireteam session
      created as host**. The gate was the **account schema's field 3** - a 64-bit leaf at C offset
      `+0x68` carrying the **PROFILE SOID**, which we had never sent. `0x585820` (the only writer of
      slot B) does `0x5834A0(&A)` → account, requires `[account+0x68] != 0`, requires `0x5833C0` to
      resolve it as family 4/**type 1** = the profile, then copies it into slot B. Verified through
      the client's own codec: the 28-byte payload decodes to exactly four non-zero qwords.
      Three static predictions landed at once - §26.76's field, §26.71's *prologue skips itself*, and
      `eboot-map.md` §2.5's *`cleanup` is a normal transit state*.
  - [x] ~~Investment **702**~~ - appeared for the first time once the account named a profile, and a
        header-only reply killed the connection (the 701 disease). **`historicaldocs/design/message-catalogue.md`
        row 83 made it a lookup**: response `0x8080194D` is the same one-composite shape as 504/701,
        same body. First real use of the catalogue as designed.

- [ ] **⭐ UNATTENDED RUNS - one screen away, and the fix is probably server-side.** §26.79.
      `capture-rig/autodrive.sh` (new) drives title → all boot gates → substage 22 with nobody at the
      machine, then stalls on **`ENUM(21)` = the brightness screen**. ⚠ Do NOT try more inputs:
      every button and all four analog axes were swept one at a time and none advance it, while
      `press b` goes *back* (so input provably arrives). **And `virtual-pad.py`'s `axis` has no
      positive control anywhere** - at substage 22, `press a` *alone* advances, so every earlier
      "the stick worked" was the button. See `status/LESSONS.md`.
  - [x] ~~The lead: `profile_setup`'s update `0x79EC20` is steered by a config object at
        `0x263D0F8`~~ - **BOTH HALVES REFUTED, 2026-08-07** `[C]` `static-client-inference`.
        **`historicaldocs/design/boot-to-x.md`.** `0x79EC20` is the **queuing** handler (vtable slot `0x192EC18` →
        typeinfo `0x192ECE0` → `c_world_controller_state_bootflow_queuing_handler`);
        `profile_setup::update` is **`0x79E830`**. And `0x2A8570`/`0x2A85B0` **ignore `rdi`** - they
        are constant-folded getters on the **signon manager** singleton (`0x263D0F8`; state at
        `+0x1F0`, sub-status at `+0x248`, guards at `+0x34`/`+0x38`). No engine-config value reaches
        any of them. ⇒ **there is no config-driven "boot to X" anywhere in the binary**: 87
        `request_state` sites censused (`capture-rig/state-request-census.py`, 6/6 controls, 25
        tail-calls), and only **6 of 50** transition functions read config - all timeouts, five of
        which go to `cleanup`.
  - [ ] ⭐ **THE SKIP IS ONE BYTE: `*(uint8*)(*(void**)0x26A3790 + 0x18) = 1`.** `[C]` static,
        `[U]` live. `0x79BFF0` (`investment_signin::update`, vtable-confirmed) is the **only**
        requester of state 21 and branches on `0x57E490()` - which reads exactly that byte.
        Non-zero ⇒ straight to `prepare_for_orbit`, wizard never runs. `profile_setup::update`
        writes the same byte itself on completion. **Category (c)** - a BSS write, so
        `synthetic_ancestor`; but the mildest available (one byte, one consumer, both branches
        converge on state 22).
  - [x] ~~`[U]` **Promote it to category (a)** - one free live read settles it~~ - **READ TAKEN,
        2026-08-07: THE BLOB IS POPULATED** `[C]` `observed-live`. `historicaldocs/design/runtime-state-access.md`
        §6.4c. `*(void**)0x26A3790 = 0x205015e70` (a real object), and `+0x00` is
        **`0xD100000000000001` - the account SOID OUR SERVER INVENTED and delivered in the 503 body**,
        so the client cannot have manufactured it locally. `+0x18 = 0` (consistent with the wizard
        running), `+0x22 = 0`, **`+0x24 = 8`**, **`+0x25 = 6`**, `+0x30 = 0xD100000000000201`, and
        `0xFFFF`-sentinel records at 0x30 stride. ⇒ **not scratch; category (a) is live.**
    - [ ] ⚠ **Still `[U]`, and do not over-read the result:** "keyed by a server-delivered SOID"
          proves it is not scratch; it does **not** prove `+0x18` maps to a settable field. That is
          now a **lookup** - map `+0x18` to a profile-schema leaf exactly as §26.76 mapped the
          account's `+0x68` to field 3.
    - [ ] Cheap confirmation that was attempted and missed its window: `0x79E830`'s exit does
          `call 0x57CC80; mov byte [rax+0x10],1`, so if `0x57CC80` returns `blob+8` the **client
          itself** writes `+0x18`. Poke the substage, then re-read within ~2 s (the hop reaches
          `cleanup` and the `0xB45C78` crash first).
  - [x] ~~**UNATTENDED RUNS**~~ - **UNBLOCKED, and it did not need the byte above.**
        `capture-rig/d1guest.py poke profile_setup_handler 7 --type u32 --offset 0x38` -
        `profile_setup`'s three screens are a counter at `[0x26B4070 + 0x38]` (**static BSS**, so the
        address is stable across runs); `0x79E830` treats any value > 6 as "done" and requests
        `prepare_for_orbit`. **2/2 runs**, from substage 3 and from substage 1:
        `20260807-190125-runtime-state-bss` (10/10 gates, zero `error.*`, **BAP 48,097 B** vs the
        previous best 12,429 B) and `20260807-191857-profile-blob-read` (10/10, zero `error.*`).
        Both then died at the *known* §26.78 frontier `0xB45C78` - the hop delivers us to the
        existing wall, it does not create one. ⚠ `synthetic_ancestor` by injection; the wire route
        (`+0x18`, or the profile display block `0x8080181A`) is strictly better and still open.
        **`autodrive.sh` is no longer the mechanism** - keep it for the title screen only.
  - [ ] **Watchdog, wire-settable today:** every state's timeout is `client_config` key
        984-1019 (`boot-to-x.md` §3.4; `profile_setup` = **key 997**, u32 ms, default 0 = disabled).
        ⛔ It is **not** a hop - reason `0x10` is in the error class, so it goes to `cleanup` and
        kicks to `bootflow:start`. Use it to turn an indefinite hang into a detectable restart.
  - [ ] Alternative: populate the profile's display block, schema `0x8080181A` (36 B, 12 leaves),
        profile field [2]. We currently send profile field 0 only. ⚠ `boot-to-x.md` §5 shows the
        three settings the client actually restores are blob `+0x22`/`+0x24`/`+0x25` (brightness,
        screen bounds, subtitles) - check the schema maps onto those before building anything.

- [x] ~~**Every `ENUM(n)` in every log is now a NAME**~~ - `design/eboot-map.md` **§7**: substage
      string table at `0x164993F`, `name_index = ENUM(n) + 10`, verified by three self-matching
      states. `ENUM(21)` = brightness · `ENUM(24)` = **bootflow fireteam create** (§26.78's crash
      stage) · `ENUM(27)` = character selection. ⚠ `ENUM(-1)` resolves to `raid` and is meaningless -
      it is the "no goal" sentinel indexing into the pre-bootflow part of the table.

- [x] ~~**THE FIRETEAM NULL CRASH**~~ - **FIXED 2026-08-07** `[C]`. §26.82. shadPS4's
      `sceVoiceCreatePort()` was declared **with no parameters**, discarding the guest's out-pointer
      while returning `ORBIS_OK` (156 calls/boot). No voice channel was ever registered → all four
      slots `0xFFFFFFFF` → NULL deref. Fixed in our fork (+ `sceVoiceGetResourceInfo`). Upstream-worthy.

- [x] ~~**THE PSN PLATFORM SESSION**~~ - **OPEN 2026-08-07** `[C]` `observed-live`. §26.83. Destiny
      creates it over **`sceNpWebApi` (HTTP)**, so it lands on our own answering surface. Added
      `POST /v1/sessions` (⚠ the id must be **`NNN-` + UUID**; the client's own zeroed `PUT` gave us
      the template), `PUT /v1/sessions/{id}`, `PUT …/presence/gameData`, `DELETE …/presence/gameStatus`.
      `update_session` now **succeeds** and the client carries our id in its own session tag.

- [x] ~~**Investment 205**~~ - appears **only once the session exists**; unanswered it head-of-line
      blocked the connection. ⚠ Its response `0x80801827` is a **952-byte** object, NOT the 8-byte
      shape 504/701/702 share. Derived via `d1-oracle` (27 B wire → `0x3D8` alloc), framing confirmed
      by re-deriving 504's known-good wire first.

- [ ] **⭐⭐ THE WALL: `error.investment_server_object_family_record_not_found`** (Prereq `ENUM(19)`).
      `unable to find family-record for handle_queuez_object_update, family_type=ENUM(5),
      family_root_soid=0x0000000000000000`. We push only families 4 and 3, so **prime suspect is our
      own all-zero 205 body**. Known good: family 5 is rooted at **`0x7FFFFFFFFFFFFFFF`**.
      **⭐ THE FULL PLAN IS IN `historicaldocs/design/character-and-orbit.md` §26.83 - step 0 is a FREE A/B
      (`D1_INV_SILENT=205`) that can refute it. Run that before editing anything.**
  - [ ] Secondary: family-4 push lands at `replication state=1`, not 3 - hold it until state 3
        rather than firing on `D1_PUSH_AT=echo`.

- [x] ~~**⛔ MY OWN `0xB471E0` NULL-GUARD PATCH WAS WRONG**~~ - **DISABLED**. It reasoned `al` was 0
      from the prologue; **RAX is reused as the table cursor**, so the guarded exit returned
      **true** - a silent false "lookup succeeded", in every run from `20260807-180823`. Found by
      `d1-replay` executing the real function on a snapshot; invisible to static reading, memory
      dumps and the offline oracle. `status/LESSONS.md`.

- [ ] **⚠ HARNESS: nothing enforces single-occupancy of the emulator across sessions.** An agent
      launched over the operator's live run tonight. `run-boot.sh` needs an owner-identifying lockfile
      that a second claimant **refuses** rather than clobbers.

- [ ] ~~**THE NEW FRONTIER: a NULL session object in the fireteam path.**~~ *(superseded - it was
      voice; see above)* §26.77/§26.78.
      `Unhandled access violation at 0x800b45e20: Read from address 0x8` → guest **`0xB45E20`**,
      a thunk `mov edi,[rdi+8]; movzx esi,sil; jmp 0x129e510` called with `rdi == NULL`. Its
      neighbour `0xB45E00` validates a triple of ids against `0xFF`, so this cluster is
      **session-id accessors**. The session logs as `[fireteam:00000000:00000000]` - no ids - and
      the crash follows `fireteam session host address changed, resetting state`.
      ⚠ `[U]` **Do not assume this is ours.** `session_class=system-link-demonware`, and `sceNpParty*`
      is STUBBED in this shadPS4 build - a fireteam is a party. This is the first gate that may not
      be on the BAP wire at all. Disambiguate before building anything.

- [x] ~~**`character:signin` exits on `qword [*(0x26A3890) + 0x20] != 0`**~~ - solved, see above. §26.75.
      **The wall is now one named qword, not a message.** Gate `0x78C5F0` (called from handler
      `0x7A0160` at `0x7A01D1`) is a four-rung ladder returning 0→1→2→3→6; it must return 6. All
      three accessors decode the same 0x30-byte tagged node at `*(0x26A3890)`:
      `+0x00 tag(==3)` · `+0x01 key` · `+0x10 slot A` · `+0x18 ready` · `+0x20 slot B`.
      Live with a character selected: tag 3 ✔, key ✔, **A = the account SOID** ✔, ready ✔,
      **B = 0 ✘** → the ladder returns 3.
  - [x] ~~Is `+0x20` really the gate?~~ **YES** `[C]` - poking it non-null moved `ui_stage` 28 → 19
        instantly, after 16 minutes frozen. ⚠ `synthetic_ancestor`, client modification.
  - [ ] `[U]` **What legitimately writes slot B.** No wire message has been shown to fill it. A SOID
        is the wrong shape: the poke crashed at `0x10875b5` (`movzx esi, word [r15]`, r15 = `index*0x20
        + arg0+8`, faulting at `0x20f8 = 263*0x20`, so **arg0 was NULL**) - slot B must be a *resolved
        object*, and merely passing the non-null test lets execution reach a consumer that wants a
        real one. Find its writer; the node is named now, so this is a lookup.

- [x] ⛔⛔ ~~**⚠ The black screen is shadPS4, not Destiny** `[C]` §26.75 … **A/B against stock
      `shadPS4-main` before investing anywhere else.**~~ - **STRUCK 2026-08-10.** The texture-cache
      warnings are real and irrelevant to the in-activity black screen: at `activity:in_world` there
      are **577 live sobjects across 8 types and ZERO of type 9 (`biped`)**. **There was never
      anything to draw.** ⛔ **Do not spend a shadPS4 build on this.** ⚠ And the proposed A/B is not
      the clean experiment it reads as - `~/dev/shadPS4-main` and `~/dev/shadPS4-d1trace` are the
      **same commit `1a51a416`**, with the fork's 16 changes **uncommitted** (`status/shadps4.md`'s
      banner). ⇒ `historicaldocs/design/biped-census.md`, `historicaldocs/status/aug10-session.md`.
      Corollary, worth its own line: **a black screen is not a state report** - the client was two
      rungs *further* than the last run that visibly showed a character card.

- [ ] **The path past it is mostly clear, statically** - pre-read, not guessed:
  - [x] ~~prologue (24) needs a skip mechanism~~ - **it skips ITSELF** (§26.71). `0x574220(4)` reads
        family 4/**type 4**, which we never send, so the index stays `0xFFFF`, substage is 5, and the
        handler falls out of its own jump table to `0x79FE89` = request state 25 `cleanup`. Every
        in-range substage jumps *past* that. `[U]` live.
  - [x] ~~full boot ladder~~ - all 38 states resolved (`design/eboot-map.md` §2.5). ⚠ **`cleanup` (25)
        is a NORMAL transit state** between prologue and orbit_loading, not only an error sink; it
        even disposes the BAP connection by design. Read the *reason*, not the state.
  - [ ] `setup:orbit_loading` (26) resolves an **orbit slice-set** from an **orbiting destination**,
        with fallbacks for both. The orbit-destination lives **in the character profile** - now sent.
        `[U]` whether the destination *field* must be populated too.

- [x] ~~**Send the profile (fam4/t1)**~~ - **DONE** §26.72. Family 4 is now a two-object baseline
      (account + profile), mirroring family 3 (roster + character). **All four known objects cross
      the wire.** `profile_rsat` independently reproduces the RSAT lane's oracle vector.

- [ ] ⭐ **Stop being reactive: `historicaldocs/design/message-catalogue.md`** - every message id → request schema,
      response schema, and **whether a response is expected at all**. Extracted from the definition
      table (`0x27FDD0(msgid)` → row; `+0x10` id, `+0x14` request tag, `+0x18` response tag). That
      last column is the generalisation of the 701 / `0x0020` / XETROV law. **Agent running.**

- [ ] ~~**READY TO RUN - the roster + a populated account now go over the wire.**~~ *(done, see the
      milestone above)* `[C]` §26.66-§26.68.
      `cargo build --release` is clean, `cargo test -p d1-bap` 29/29.
      **Two things changed, and the second one is the new finding:**
      1. The roster (fam3 **root**, `0xA2AC2C74`) is now *sent* rather than poked into memory by
         `inject-roster.py` - family 3 carries a **two-object baseline**, roster + character.
      2. ⭐ **There are TWO character lists and we were filling the wrong one.** `0xA110F0`, the
         selection-card builder, counts characters out of the **ACCOUNT** object (fam4/t0
         `+0x08`), not the roster: `cmp dword [rbx+8],0 / jle` → zero cards. The roster only
         decides CREATION vs SELECTION (`0x78C9F0`). That is exactly *"D1Guardian, but no guardian
         to select."* `account_rsat()` now carries the list.
      ⚠ `run-boot.sh` no longer pins `D1_ACCOUNT_RSAT` - pinning it restores the empty-list payload.
      **Watch for:** (a) SELECTION as before, (b) **a guardian card renders** ← the new variable,
      (c) client emits 504. If (a) holds and (b) does not, read what `0xA110F0` does with the
      family-4 **type 2** object it also fetches (`0xA1124D`) - `0x48` bytes, never sent, `[U]`.

- [x] ~~**The RSAT bit-grammar**~~ - **DERIVED, 2026-08-07** `[C]`. Roster and profile both solved
      and round-trip verified through the client's own object codec `0x2C9000`
      (`capture-rig/oodle/queuez-object-oracle.py`, `--control` passes). Grammar in `d1-bap`:
      `character_list_rsat`. ⚠ **Short-payload trap**: the client's bitstream reads *past the end*
      of a short buffer and returns zeros, indistinguishable from "fields absent" - a 17-byte
      account payload decodes correctly while the client reads 157 bits out of 136. Size payloads
      from the schema's field count; regression guard is a test that keeps the trap reproducible.

- [ ] ~~**THE WHOLE REMAINING GAP: two objects we have never sent.**~~ *(superseded by the two items
      above - kept for the measurement.)* `[C]` §26.65.
      Read live at `profile_setup` with replication healthy:
      ```
      descriptor class=3 / class=4  state=3 REPLICATING (updates accepted)
      ROSTER    fam3/t0  EMPTY      <- never sent
      CHARACTER fam3/t1  POPULATED  <- our push lands
      ACCOUNT   fam4/t0  POPULATED  <- our push lands
      PROFILE   fam4/t1  EMPTY      <- never sent
      ```
      | object | fam/type | checksum | size | unblocks |
      |---|---|---|---|---|
      | roster | 3/0 | `0xA2AC2C74` | `0xA0` | CREATION → SELECTION (`0x78C9F0` reads its count) |
      | profile | 4/1 | `0x8127F15A` | `0x7078` | `record[+0x20]` → `0x78C5F0(0)==6` → exit state 23 |
  - [ ] **The one missing capability: the RSAT bit-grammar to encode them.** Not a new gate, not a
        new mechanism - a serialization format for two objects whose decoded layouts, checksums,
        sizes and registry addresses are all verified. **Derive it from the client's own tables via
        `rsat-oracle.py`; do NOT hand-author by analogy** (that is the 0-for-3 pattern). Agent running.
  - [x] ~~"the character object does not land"~~ - **RETRACTED** (§26.65). It lands. The §26.63 read
        was post-teardown. ATLAS §4.6b.
  - [x] ~~replication state 1 blocks us~~ - **SOLVED** (§26.64). 1→2 is LOCAL and unconditional
        (`0x56A8A7`, set when the client SENDS `0x000C`); nothing we transmit advances it. Live
        descriptors sit at state **3**. The `state=1` rejection was a race on an earlier push.

- [ ] **Ask the research lane for SECURED-path receive probes.** The 11 installed client-BAP probes
      fired exactly twice, both in the plaintext handshake (`expected_type 0x1F`, `0x1A`); `0xB2E0DC`
      is never reached once the channel is mode-1 - not for any echo, 503, 206, 501, 701 or either
      QueueZ push. Not budget (423/60,000), not rearm. So that catalogue is the **pre-channel**
      dispatch and structurally cannot see the QueueZ push consumer. Also want: probes on the
      **insertion** step (acceptance ≠ insertion), and a same-run guest-side positive control so
      "no records" is distinguishable from "probes on the wrong function".

- [ ] ⚠ **Do not harden the server around unsolicited pushes** until the subscribe→snapshot flow is
      understood. `historicaldocs/design/server-object-model.md` - the honest server may be *simpler* than the
      current workaround. The one safe fix now is retiring the env-var pile into code as knowledge.

- [ ] ⚠ **§26.42 and §26.43 are RETRACTED (§26.45)** - the short BAP transcripts they were built on
      were caused by a missing `D1_503_SOID`/`D1_503_RAW`, not by the bypass patch or the push.
      Do not cite their transcript tables. `patches/CUSA00219-force-account-ready.xml` is still a
      bad idea on §26.41's reasoning (the ready byte is computed, so overriding it can only lie to
      the client) but has **not** been shown to change BAP behaviour.

- [x] ~~**Runs silently losing gates already passed**~~ - **FIXED.** Three times on 2026-08-07 a
      forgotten env var un-passed a won gate and the failure surfaced three gates earlier than the
      thing under test. `capture-rig/run-boot.sh` now pins the whole known-good environment
      (`D1_MANIFEST_FILE`, `D1_503_SOID`+`D1_503_RAW`, `D1_206_BODY`, `D1_PUSH_*`, SOIDs, tracer
      output) with a loud preflight, and `capture-rig/gate-check.py` asserts all 9 already-passed
      gates after every run. Validated against the known-good run `130633`: 9/9 PASS.

- [x] ~~**503 + pig + `0x00AB` + message 206 syntax**~~ - **DOWN/ATTRIBUTED, 2026-08-07.**
      The native 503 encoder oracle supplies account SOID `0xD100000000000001` and family-5 root
      `0x7FFFFFFFFFFFFFFF`; account record `2 → 3` is reproducible and pig is gone. `0x00AB` is
      one-way XETROV telemetry, parsed but not answered. The responder's cross-message 503-body bug
      is fixed; id 206 now receives its native fixed-point body `10`. See
      `historicaldocs/design/character-and-orbit.md` §25-§26.4.

- [x] ~~**Empty object range → guest NULL dereference at `0x570AAE`**~~ - **SOLVED/ATTRIBUTED,
      2026-08-07.** Producer chain read end to end and confirmed live. `0x56FFB0` is reached by a
      **tail call** (`jmp` at `0x560D27`), not virtual dispatch - §26.9's "static analysis cannot
      find it" is **retracted in §26.10**; the scan had only looked for `call`. Chain:
      `handle_response_internal` (`0x567670`) → policy virtual `+0x58` (`0x560CF0`) → tail jmp →
      `0x56FFB0` → registry dispatch `0xBF25A0`. The "collection" is a **BAP response envelope**
      `{u32 class id, ptr response, ptr repeated-field, i32 count}` in a 128 KB arena. `0x1F70` is
      `mov al,1; ret`, so the dispatch is **unconditional** and an empty span is always fatal.
      BAP message-policy class names now read from RTTI (§26.11). See §26.10-§26.15.

- [x] ~~**What makes the BAP response span non-empty**~~ - **ANSWERED OFFLINE, 2026-08-07
      (§26.32 → `historicaldocs/design/span-sweep.md`).** **ONE presence bit**, read immediately after the response
      object's bits, then a **u16 length in BYTES** that must be ≥ 1. **No schema participates** -
      the span is read after the field walker `0x2C9000` returns, and every write to envelope
      `+0x10`/`+0x18` sits on that one path. The client calls it the *"optional variable payload"*.
      Derived twice independently (decoder `0x261FB0`, encoder `0x261BE0`) - they converge. Retracts
      §26.27's array bit, supersedes §26.19's "list of `u16`", resolves §26.23's `queuez_object_body`
      tension. Collapses these earlier sub-items: descriptor pull (§26.19), codec `0x7B` (§26.23),
      envelope decoder (§26.25/§26.26), static tag search (§26.27), the QueueZ-push and
      tail-append eliminations (§26.20/§26.24).

- [ ] **⭐ NEXT GATE, and it is now a SERVER-BYTES change: message 206's body must carry a span.**
      `[C]` The fault is **ours, in what we send** - not a schema we fail to model and not a
      misidentified message class. 206's shipped body `10` has the presence bit **0**; `18…` carries
      a span on the same one-byte schema `0x80801869`. Everything below is **SYNTHETIC** (the
      client's own code driven offline under Unicorn) and needs the one live run to be promoted.
  - [ ] **PROPOSAL to the `d1-bap` owner - do not implement from another session** (one protocol,
        one implementer). `investment_206_response` (`d1-bap/src/lib.rs:1101`) body `10` →
        a bit-packed span. First shot: `queuez_empty_family_body_for(4, acct)` (25 B), which codec
        `0x7B` **accepts**, measured. Exact bytes and derivation in `historicaldocs/design/span-sweep.md` §6.
  - [ ] ⚠ **The span is bit-shifted by 5 bits, NOT byte-appended.** `D1_206_TAIL` (which emits
        `10 || tail`) therefore cannot express the fix - that is why §26.22 test 5 and §26.24 failed.
        The smallest enabling change is a body-**override** form of that same labelled hook.
  - [ ] ⚠ **Do NOT ship span = `00000000` (count 0).** It opens the envelope gate but codec `0x7B`
        answers `invalid queuez transaction count 0` and returns FALSE - and the caller discards that
        return, so it is silent. A/B discriminator only.
  - [ ] ⚠ **`present=1, length=0` does not clear the gate.** `0x570038 jle` tests the COUNT, so a
        non-NULL span pointer with count 0 still skips handle B and still dies at `0x570AAE`.
  - [ ] **The one live run that settles it.** Success = `msg 0x003b count != 0` in the tracer,
        **never** the absence of a crash - four interventions have now removed the crash while
        leaving the gate shut. Cheap extra in the same run: probe `0x27FDD0`'s return value, which
        settles envelope `+0x00` and re-opens §26.16 on evidence instead of ordering.
        Second free rider, zero marginal cost: run `capture-rig/live-tag-identity.py` against the
        same pid. Cross-process tag stability is sampled at n=2; a third process takes §26.33's
        remaining `[I]` to `[C]`. Controls are already in the tool (`0x80801825` / `0xDEADBEEF`).
  - [ ] `[U]` Then: span **content** beyond a baseline family, if the connection still fails.
        Codec `0x7B`'s layout is confirmed and equals `queuez_object_body` - `historicaldocs/design/span-sweep.md` §5.
  - [x] ~~**Carry the NAME HASH forward, not the tag**~~ - **REVERSED, 2026-08-07 (§26.33 →
        `historicaldocs/design/namespace-walk.md`).** The tag is a **compile-time constant**:
        `tag = 0x80000000 | ((container_id << 13) & 0x7FFE000) | (index & 0x1FFF)` (`0x6E9B0`), both
        inputs static. Offline replay predicts **7670/7670** live tags/objects/hashes; scramble
        control 0/7668 at ±1. ⇒ **`0x808033F4` may be hard-coded and no live read is ever needed to
        obtain a tag again.** Namespace fully walked: **NINE** registries, 15520 entries over 14816
        elements, all eboot-resident. `0xE2E25C2C` / registry-B **index 1167** confirmed four ways;
        §26.31's `[U]` on index 1166 discharged (it is `0x80805F08` → `0x13C4FD0`, a different type).
        The hash function stays **CLOSED** (§26.31 (4)) - and naming is now gated on finding **one**
        confirmed (name → hash) pair, not on more search: the 101 message-definition hashes are a
        *disjoint* name space (0/101 are element hashes; control: 4 known element hashes test present).
        Re-derive: `capture-rig/namespace-walk-verify.py` (72 checks, offline, ~1 min).
  - [ ] ⚠ **Ablation is airtight (§26.18a):** signon completes, both dispatches run, no crash - and
        the client STILL kicks `error.no_investment_connection` ×8. The crash was never the gate, and
        filling the span is **necessary but not proven sufficient**. All ablation patches
        (`span-guard`, `skip-msg3b`, `empty-span`) are diagnostics, never fixes. `synthetic_ancestor`.
  - [ ] ⚠ `CUSA00219-empty-span.xml` patches `0x570A84`, the same address as the `SpanElementType`
        probe; the tracer byte-guard then refuses to arm and the run yields ZERO records. Always
        check the record count before trusting a patched run.

  ⚠ **DO NOT hand-fit a grammar, and DO NOT re-parse a structure without checking the retraction
    list first.** Four parser defects produced four confident wrong readings; one of them was
    *already retracted in §13.4* and re-made anyway. Reading by inspection has failed here every
    time - drive the client's own code.

- [x] ~~**MESSAGE 503**~~ - **DECODES (2026-08-06).** It never needed the recovered schema: the
      argument bitstream was simply **empty**. `D1_INV_PAD=64` appends zero bytes = "every optional
      field absent". Zero bitstream errors, BAP stays up (echo keepalives), failure moved
      **weasel → chicken**. ⇒ 503 has **no required fields**.
- [x] ~~Root-SOID hypothesis ("we've been pushing to an address the client never agreed to")~~ -
      **REFUTED, `runs/20260806-183719-acct-root-platformid`.** Family 4, `object_count=0`, root =
      the client's own `0x44ED979226358B44` ⇒ rejection **byte-identical** to the one under our
      invented `0xD1…01`. Zero bitstream errors, 503 accepted, BAP up. The estate was never
      mis-addressed; **no root is subscribed at all.** → §10.5
- [ ] `[O]` Unanswered at exactly this point, probably presence bookkeeping, but logged rather than
      dropped: `404 DELETE /v1/users/D1Guardian/presence/gameStatus?notificationWithData=true`.
- [ ] Family-4 slot-0 account object (`ACCOUNT_INSTANCE_TYPE 0x64AE75BF`, schema record `0x12FA3F8`)
      with a **non-zero first qword** (`0x57CC80`). `D1_ACCOUNT_RSAT=<hex>`, no rebuild.

- [x] ~~**"the identity is server-delivered - send BAP type `0x26`"**~~ - **WITHDRAWN, do not act on
      it.** `0xBAB860` is the receive half of an *internal simulation-queue proxy*
      (`c_investment_player_identifier_proxy_game_system`), produced locally by `0x58A940` →
      `0xBAB740`. There is no such wire message. The identity is **local and VALID** - `0x2B79C0`,
      the validity predicate, is literally `cmp byte [rdi],0; setne al; ret` = "the platform
      identifier is non-empty", and the wire carries `"D1Guardian"`. Confirmed three ways in §10.2.
      ⚠ Trap worth keeping: *a message-typed jump table is equally the shape of an internal event
      queue* - always xref who **emits** the type before concluding the network delivers it.
- [x] ~~**Find the 503 argument schema - the definition table**~~ - **FOUND AND EXTRACTED
      (2026-08-06).** Symbol `investment_globals` → pkg 300 entry 0 (index object) → entry 1 (globals
      struct) → `+0x578 = 0x80A5E37A` → **pkg 303 entry 890**, 6528 B, **101 records**. Layout
      confirmed live (`0x40` stride, id `+0x20`, schema tags `+0x24`/`+0x28`). Committed:
      `capture-rig/schemas/investment-message-definitions.tsv`. **503 → `0x80801726`/`0x80801825`;
      504 → `0x808017ce`/`0x80801845`.** → `historicaldocs/design/character-and-orbit.md` §12
- [x] ~~**GET THE REAL SCHEMA BYTES FOR `0x80801825`**~~ - ✅ **RECOVERED (2026-08-06)** from the
      **live client's package registry**, via `capture-rig/dump-registry.py`. eboot base found by
      signature search in `/proc/<pid>/mem`; `0x1A0B420` → registry; slot `0x400` **entry_size 24**
      (not 16) and the `loc` back-offset is **SIGNED**. **503 response = 968-byte struct, 5 FIELDS**
      (composite · u64 · u32 · u32 · 944-byte composite). Walker validated against the request, which
      reproduces the captured `36 flat bytes + present + u64` exactly.
      **The tool resolves ANY tag - 504 and the other 99 definition entries included, no further RE.**
      `capture-rig/schemas/investment-503-schema.txt`. → `historicaldocs/design/character-and-orbit.md` §17
- [ ] **⭐⭐ NEXT: DRIVE THE RSAT WRITER - the codec is one we already have an oracle for.**
      `[C]` The field walker `0x2C8680` calls only `0x105DD20`/`0x105E3C0`, whose prologues are
      **byte-identical to `0x105B410`** - the generic RSAT writer §5 already drove under Unicorn to
      emit the character payload as a byte-exact vector. **Investment message arguments ARE RSAT
      bitstreams.** Plan: (1) snapshot the live registry with `capture-rig/dump-registry.py` - the
      piece the §15.2 tracer lacked, now obtainable; (2) build a 968-byte struct using the §17
      offsets (`+0x08` composite · `+0x10` u64 SOID · `+0x14` u32 · `+0x18` u32 · `+0xA0` 944-byte
      composite); (3) drive `0x105E3C0` with descriptor `0x80801825`; (4) the emitted bitstream **is**
      the 503 body - derived, not guessed. → `historicaldocs/design/character-and-orbit.md` §18
- [x] ~~pin the encoding for `kind & 0x100`~~ - **MOOT.** The client's own writer decides it; see
      above. Do not hand-decode it.
- [ ] (old) pin the encoding for `kind & 0x100`. Two schema-shaped bodies (f4 absent, f4
      present) both decode with **zero** complaints and both still chicken. `[I]` `0x100` likely means
      **ARRAY** (length-prefixed) rather than "optional" (presence-flagged) - so our zero bits read as
      *empty arrays*, the §11.5 silent-empty trap again. Emit non-empty arrays once the count encoding
      is known. → §17.4
- [ ] (was) GET THE REAL SCHEMA BYTES FOR `0x80801825`. ⚠ **Value-guessing is exhausted** - the §13
      feedback channel only separates *malformed* from *well-formed*; a body that parses is accepted
      silently regardless of whether its values are right. Two different field layouts both parsed and
      neither fired the action (§15.1).
      **Routes (a) and (b) are ELIMINATED - both blocked on the SAME missing table.** (a) rebuild slot
      `0x400`'s entry table statically → **zero** candidates. (b) Unicorn trace → the field walker does
      a **second** registry resolution at `0x2623F0` for *every field's sub-type tag*, so stubbing the
      top level only defers the problem one level (`capture-rig/oodle/trace-investment-schema.py` -
      scaffolding works, checked in, reusable once a registry snapshot exists).
      ⇒ **(c) INSTRUMENT THE LIVE CLIENT is the only route left**, and it needs exactly one address:
      **package registry global = vaddr `0x1A0B420`** (`registry = **0x1A0B420`; both resolution sites
      `lea` it - verified they agree). Read it in a running client and the slot table, entry table and
      **every** schema descriptor become walkable with the §14.3 arithmetic - 503, 504 and the other
      99 definition entries all at once. → `historicaldocs/design/character-and-orbit.md` §15.2
- [x] ~~**FEEDBACK CHANNEL for the 503 response**~~ - **OPEN (2026-08-06).** `D1_503_SOID=1` sends a
      body with a **leading presence bit of 1**, which forces the decoder to commit; it then names its
      complaint (`bitstream error when decoding message-arguments`, `Failed to decode [policy 1]`)
      and tears down → **weasel**. ⚠ **A zero-leading body is indistinguishable from an empty one** -
      "no error" proves nothing. **Weasel means we were heard**; chicken means we were ignored. → §13

- [x] ~~**LIFT THE OODLE DECODER**~~ - **DONE (GPT lane, 2026-08-06).** Destiny's own decoder is run
      under **Unicorn** emulation out of our eboot: `0x1267120` size query, `0x12671A0` init,
      `0x12672E0` decode. 16/16 chunks, 262144 B, input consumed exactly. Preserved at
      `capture-rig/oodle/` (was `/tmp`). It has already yielded the real 01.29 records for investment
      messages **501 (create) through 504 (login)**.
- [ ] **⭐ FOLLOW `0x80AA2840`** - the login (504) definition points at this content tag, `[I]` the
      request/response schema object carrying character SOID and field widths. In progress, GPT lane.
- [ ] **Finish the Oodle verification**: block records' `sha1[20]` does NOT cover the stored bytes, so
      it likely covers the **decompressed** bytes. One lane's attempt was inconclusive because it
      guessed the output size; the logical size is in the entry `loc` field (bits 28..57). Redoing it
      with the real size turns "it decodes" into "it decodes correctly", against a digest Bungie
      shipped. Cheap and worth it.
- [x] ~~Author the message-503 response bitstream~~ - blocked upstream on Oodle; see above.
- [x] ~~Decode the mode-1 secured framing~~ - AES-128-GCM, solved and confirmed on the wire.
- [x] ~~Answer BAP secure-hello~~ - **done.** Full handshake in `status/shadps4.md`; codec in
      `d1-server/crates/d1-bap` (9 tests, incl. parsing the real captured frames).
- [ ] `[C]` **NULL RESULT, recorded so it is not repeated:** replying with a bare 6-byte envelope
      (`01 02 00 00 00 00`) changed **nothing** - byte-identical client behaviour to sending no reply
      at all (`_connection_failure_suicide` in `_channel_starting`). The `Failed to read envelope
      offset '0'` / `[size 6][offset 0]` lines that disappeared earlier went away because we started
      **holding the socket open**, NOT because any reply was accepted. Runs:
      `20260806-152413-bap-endian` (closed) vs `-152657-bap-hold` (no reply) vs
      `-153201-bap-envelope-probe` (6-byte reply). Comparing against the wrong baseline produced a
      false positive here once already.
- [x] ~~shadNet as BAP prior art~~ - **no.** Zero real `bap` hits and zero Destiny mentions in its
      source; it is PSN identity/WebAPI emulation only. Closed; do not re-check.
- [x] ~~`resourcerer failed during unload`~~ - solved by registering the full package estate (1033).
- [x] ~~BAP address byte order~~ - `f2.6` is LITTLE-endian, `f14` is BIG-endian. Regression-tested.
- [x] ~~Build a real content manifest~~ - done, and it crossed the `bootstrap_patchable` gate.
      Only **175** of 795 packages declare symbols at all, so the meaningful full set is 175 entries
      (9.4 KB), well under the 1325 cap - not the 1033 file-entries first estimated.
- [x] ~~Redirect all 26 eboot hostname literals~~ - done (51 host entries now).
- [x] ~~Serve `ticket_drop`~~ - done: TLS on 32556, uploads archived to `$D1_UPLOAD_DIR`. Note the
      game did not upload on the last halt, so **reading the crash folder off disk remains the
      reliable path** and `d1` now parses it automatically.
- [ ] `[U]` Probe the **silo_id transform**: sent 12345, engine reports 28729 (exactly +0x4000).
- [ ] Fix shadPS4's `sceNetCtlGetInfo` - it reports no local IP, and the engine records
      `insecure_ip: 127.0.0.1`. Will matter for P2P.
- [x] ~~`d1`/`d1-run` gate-depth reporting~~ - **done.** Ladder, depth vs `BOOTFLOW_SPINE` (14 spine
      states of 22 total), terminal reason, guest `RELEASE_VHALT` text, all in `runs`/`show`/`diff`
      and in the GUI. The trap it guards: depth is the FURTHEST state, not the latest - a failed run
      loops back to `bootflow:start`. 9 tests.
- [ ] **Get a Bungie.net API key** (`bungie.net/en/Application`, free + instant) and pull the D1
      manifest. ⚠ **Correcting a subagent's finding: the D1 endpoints are NOT gone.**
      `https://www.bungie.net/d1/Platform/Destiny/Manifest/` returns **HTTP 200 with
      `ApiKeyMissingFromRequest`** - alive and *gated*, not 404. Archive as WARC + BagIt, all 7
      languages + gear databases.
- [ ] `[U]` **Lift `OodleLZDecoder_DecodeSome`** (vaddr `0x12672e0`, our own decrypted eboot) into a
      harness. **76% of package blocks are Oodle LZH/LZHLW** with no public decoder, which is what
      blocks recovering an Elimination activity definition. Self-checking experiment.
- [ ] `[I]` **Probe signon `f6`**: we echo the executable build (`52553.16.09.28.1551.evil`) but
      `networking:sku_manager` reports base content version **`52553-6-2940354`**, and the crash
      report agrees (`content_build 52553`, `content_revision 6`). Silencing the complaint is not
      proof of correctness.
- [ ] `[I]` **Manifest `offers` ids are probably `0xE0000001` (full sku) / `0xE0000002` (paid tier)** -
      the ownership manager logs them as "locally determined". First concrete candidate for values we
      would otherwise have invented.
- [ ] Walk diagnostics below T=2 for `bap_signin` - it is silent at both T=4 and T=2, so its sites are
      quieter than 79%. T=1/T=0 patches already exist in the patch file. **T=1 is now the default and
      has been for 110+ runs**, so the remaining blind set is exactly the 81 rows
      `diagnostic-catalogue.md` §2 classes `weak-blind` (level 0) - `T=0` is the only run that can
      ask about them, and the catalogue names them before you spend the run.
- [ ] Fold `virtual-pad.py` into `run-signon-experiment.sh` so a gate-ladder run is one command

## Blocked on hardware

- [ ] **Live capture lane** (consoles A/B, tap → `dumpcap`). The only irreplaceable artifact in the
      project - and its value just went up sharply: **if the bootstrap hypothesis holds, a retail
      console's cold-boot traffic contains the endpoint table our 2016 client is missing.** Not merely
      protocol reference: *the thing we lack*, in one response, obtainable only from a working client.
      Meanwhile `panther-*` is already NXDOMAIN - the estate is dying in front of us. → `harness.md`,
      `historicaldocs/design/THE-BOOTSTRAP-HYPOTHESIS.md`

## Soon

- [ ] Decide 01.29-only vs also staging **01.33** (the only version with public shadPS4 evidence -
      useful as a bring-up control if 01.29 fights us)
- [ ] Repeatable run-artifact discipline: one directory per experiment with versions, commits,
      config, fixture, manifest → `harness.md`
- [ ] Console **D** from `CFW-DUMP-RUNBOOK.md`
- [ ] Read the **Aldridge "I Shot You First"** + **Truman "Shared-World Shooter"** talks before any
      Layer C (transport) work - they define what to look for, so they precede the capture
- [ ] Execute the capture-rig deletion manifest (operator-gated; prepared 2026-08-11, nothing
      deleted yet) - `design/capture-rig-audit.md`

## Later / unlit

- [ ] **Layer B1 (Demonware) is now REACHED, not unreachable.** As of 2026-08-06 the client
      initialises it: `bdAuthPS4.cpp:312 Unknown SCE IssuerID[100], default to PROD environment` -
      our synthetic PSN `id_token` has an issuer Demonware doesn't know, and it falls back to PROD
      rather than failing. Design work is now warranted. `[I]` expect a wall at BAP `secure_hello`,
      since we send 16 zero bytes where key material belongs.
- [ ] Private-server client path on hardware: patch `sceHttpsLoadCert`'s single CA + the two
      `sceHttpsSetSslCallback` callbacks (real firmware enforces these; the emulator doesn't)
- [ ] Two-instance harness (automated input is **done** - `capture-rig/virtual-pad.py`)
- [ ] Feed shadPS4 gaps upstream as they're attributed - the divergence log *is* the contribution
      pipeline

---

## Done

- [x] **The diagnostic-string catalogue exists (2026-08-07)** - `design/diagnostic-catalogue.md` +
      `capture-rig/diagnostic-catalogue.py` (20 controls) + `capture-rig/diagnostic-catalogue.tsv`.
      2,125 strings, 117 subsystems, referencing function / arity / gate level, and ⭐ a **SEEN**
      column over every `runs/*/emulator.log`. `--addr 0xNNN` answers "which subsystem is this
      function?" - the query that cost three sessions when the answer was `voice_globals`.
      Two by-products: it **resolves all five `[U]` rows** in `guest-diagnostics.md` §3's system-id
      table (and corrects two more), and it finds that **315 strings reach the sink with no filter in
      front of them**, so for those SEEN/NOT-SEEN is a clean execution trace.
- [x] **`content_check` + `package_registration` CROSSED (2026-08-06)** - the manifest URL is signon
      response `f4.3`, an engine-config delta stream (`u32 magic 0x00123456`, then `(u32 key, value)`
      pairs, key **100** = URL) and it is the **only** writer of that URL in the binary. A prediction
      made before the run held exactly: a 404 URL turned `not_provided` into `url_invalid`. With a
      one-byte lab patch clearing the RSA-envelope check, a 2-byte manifest (`2a 00`) passes.
      Corrected `signon-protocol.md`'s "do not emit f4.3" - the magic is right, the framing is
      **fixed32**, not varint. → `historicaldocs/design/content-manifest-gate.md`
- [x] **The game hands us its own crash report** - `RELEASE_VHALT`, halt reason, engine state, and an
      `active_packages.txt`, written to shadPS4's temp dir. Preserved per-run under
      `runs/<run>/guest-crash-report/`. This is the single richest diagnostic channel found so far.

- [x] **`/SignOn` WALL BROKEN (2026-08-06)** - the master gate is passed. Full response schema
      recovered from the client's own **table-driven descriptor tables** (not protoc-generated) and
      confirmed live on the first attempt: the client logs `obtained from SignOnResponse: [succeeded]`,
      echoes our `environment`, believes our BAP address, and **bypasses STUN entirely** because
      response field 14 supplies its external address. Key trap recorded: `Unable to parse signon
      response` does **not** mean malformed bytes - it means "not actionable", and a byte-perfect
      response omitting field 2 produces the identical four log lines.
      → `historicaldocs/design/signon-protocol.md` (spec §1-§8, live confirmation §9)
- [x] **The DNS route worked - the RSA signature never had to be forged.** Answering the eboot's
      hostname literals inside the sealed namespace redirected the client with no file edit. The
      `sku_config` signature verifier was never touched. `historicaldocs/design/sku-config-format.md` records the
      wall; this records the bypass.
- [x] **Guest diagnostics running** - `patches/CUSA00219-guest-diagnostics.xml` (sink + filter) is what
      makes every gate above readable; the client narrates its own state machine.
- [x] **Autonomous input solved** - `capture-rig/virtual-pad.py`, a `/dev/uinput` virtual gamepad. No
      sudo, no emulator patch, works on Wayland where `xdotool` cannot. Verified against shadPS4's own
      SDL3 before use, then drove a real run.
- [x] **Fixed a silent harness bug:** the one-emulator guard never fired, because `pgrep -x shadps4`
      cannot match a process whose comm is `shadPS4:Main`. That, not a race, is how two emulators ran
      at once. Both launchers now match the command line.

- [x] **Content gate closed** (2026-08-05/06) - decrypted 01.00 base (18.61 GB) + decrypted 01.29
      patched game (49.95 GB) + 26 decrypted `sys_modules`, all hashed. Was the blocking
      prerequisite for every emulator gate.
- [x] **CFW jailbreak** - Vue System Backup → GoldHEN 2.4b18 on fw 10.01; FTP decrypt-on-read
      discovered (etaHEN unnecessary)
- [x] **01.29 installed on console** - split PKG concatenated to defeat `CE-35659-8`
- [x] **Contained rig live** - isolated segment, nft guard, fail-closed DNS, verified with the
      console connected
- [x] **shadPS4 BOOTS (2026-08-06)** - Destiny 01.29 reaches the title screen on Linux/AMD,
      reproducibly, zero faults. Parity with the best public trace, on our own SKU and version.
      The earlier "deterministic crash" was self-inflicted load (`systemd-coredump` chewing 16 GB
      cores between runs); harness rule is now `ulimit -c 0` on a quiet box. Along the way ruled out
      GPU/driver/unified-memory (lavapipe), emulator age (stock main), the fs→gs TLS patcher, and
      game version/SKU. → `shadps4.md`, `historicaldocs/SHADPS4-FIRST-LIGHT.md`
- [x] Archive copied + byte-verified against the Mac; 672 exFAT-mangled filenames repaired
- [x] **Layer A built and the NP identity gate OPENED (2026-08-06)** - `d1-server/` Rust workspace
      (wire + generated proto + service); Destiny logs in, `SIGNED_OUT` → 0, and it asked ShadNet for
      nothing else. Next gate named: name resolution for the STUN/signon hosts. → `status/server.md`
- [x] **Contained run harness** - `capture-rig/run-contained-emulator.sh`: one netns for server +
      emulator, DNS fail-closed, containment asserted and recorded per run, one-instance guard
- [x] **STUN gate confirmed `[C]`** - clean single-instance run, 0 faults, 16 resolver calls to 16
      distinct names, with host-condition provenance. Supersedes the double-instance measurement.
- [x] **Host-condition provenance in the launcher** - loadavg + PSI pre/post per run. Adopted rule:
      a determinism claim without host-quiet evidence is `[O]`, not `[C]`.
- [x] Repo under git, pushed private to `gitlab.com/aknumbers/destiny1-preservation` (assets excluded)
- [x] **Bootstrap chain CONFIRMED (2026-08-06)** - `signon` is the master gate, not STUN; it returns
      config/environment/silo/external-address/BAP-address; `sku_config` signon URL override is the
      redirect seam and the file is already in our dump. 33 networking subsystems catalogued (my own
      pass had found 12).
- [x] Docs: `capture-rig/CFW-DUMP-RUNBOOK.md`, `historicaldocs/SHADPS4-FIRST-LIGHT.md`, `status/`

## Post-orbit: populate the world (opened 2026-08-08)

**Plan of record: `historicaldocs/design/investment-content-map.md`.** Orbit is reached and empty; the client has
already named what it wants.

- [ ] **Answer the family-0 and family-2 subscribes.** The client asks **83× each per run** and we
      push only families 3 and 4, so both sit at `state 4 LOST/TIMED-OUT (update REFUSED)`.
      Schemas measured: family 0/1 = `0x808017FD`, family 2/1 = `0x80801810`.
      ⚠ **Timing is part of the contract** - a timed-out family REFUSES a late push. Answer on the
      subscribe, not at relay time.
      ✅ Success criterion is **measured, not logged**: `d1guest descriptors` must show state 2 → 3.
- [ ] **Attribute `0x80801732`'s 8 fields** (inventory element, 666 × `0xB8`, currently all zero),
      then push ONE item and read it back with `d1guest obj 4:3` before any loadout work.
- [ ] **Attribute `0x8080102E`'s 200 × {u64, u6}** against the manifest's 7,731
      `DestinyUnlockFlagDefinition` rows - this is the campaign-completion table, read by
      `investment_activity_selection_manager`.
- [ ] **The director.** The client says `orbit_map_loader: The character's last-orbited-destination
      is invalid` and falls back; `HASH(0x811c9dc5)` is the FNV-1a basis, i.e. the hash of an EMPTY
      string. Work back from `0x752AE0` and `0x7A8190` to find what sets it.
- [ ] **`d1-forge` / `d1-loadout`** - egui loadout framework (subagent lane). Needs a control-plane
      HTTP route on our side once its request contract lands.

⚠ **This phase is BLIND to the guest log.** The diagnostic catalogue has **zero** strings for
`inventory` or `progression` (controls in the same query: `account_id_translation` 8, `queuez` 10).
A wrong inventory will look exactly like a correct one. Verify in memory, not in the log.

## ⭐ Destination lane (opened 2026-08-08) - `historicaldocs/design/HANDOFF-2026-08-08.md`

- [ ] **Family 4 / type 4 object, key `0x7FFFFFFFFFFFFFFE`** - the director's lookup target, all
      zeros today. Instance type from `d1guest tag 0x8080102E`; encode via `d1-oracle object
      --tag 0x8080102E`; **`--decode` to verify**; extend `queuez_family4_from_parts` to
      `object_count = 3`. ⚠ ADD to the baseline, never replace account/profile.
- [ ] **Prove it offline first** - `--set-mem` into `0x37F92E340` on `runs/snapshots/orbit-all.snap`,
      replay `0x574220`, watch the lookup at `0x5742BD`.
- [ ] **`/control/loadout/apply`** - the server side of the GUI's contract (`d1_loadout::apply`).
      First increment: route exists, honours `dry_run`, echoes `request_id`, `accepted: false`.
- [ ] **One inventory item** into family 4/type 3, verified by `d1guest obj 4:3` read-back.
- [ ] **Read `historicaldocs/destiny1/docs/talks/`** - never opened; names/intent for families 0/2, the unlock id
      domain, and the director.
- [ ] `[U]` **Does a player entity instantiate with an empty inventory?** If not, gear is a hard
      spawn prerequisite and jumps the queue.

## ⭐ Plan B - the peer/platform layer (opened 2026-08-08) - `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md`

**⭐⭐ C7 IS SOLVED, AND IT IS SMALLER THAN WE THOUGHT.** Nobody issues the peer key material -
**the client mints it itself**, locally, with Demonware's MT19937 (`bdSecurityKeyMap::initKey`,
`0xF875A0`: 8 random bytes → bdSecurityID, 16 → bdSecurityKey, **no input at all**). The "36-byte
{securityID, key}" in the old brief was the **0x24-byte secure address**; the real record is
**78 bytes** `{ID(8) | addr(0x24) | key(0x10) | session-id(0x12)}`. It reaches the joiner as the
**`session-data` multipart part of `POST /v1/sessions`** - the **PSN Session WebAPI over plain
HTTP**, which shadPS4 routes to *any host we configure*. ⇒ **C7 is a store-and-forward of an opaque
blob in `d1-responder`.** `peer-mesh-and-the-server-surface.md` §4 is **RETRACTED in place**.

⚠ **D1 never calls `sceNpMatching2`.** Library `U` = 28 imports, full call-graph closure = 26
functions, **all** inside Sony's NP Toolkit, **0 game-side** - against a control where **32 of 38**
import libraries *do* have game-side reachers. ⇒ **the shadNET room protocol is not our surface, and
`matching2_enabled` stays OFF.** Tagged `[I]`-strong; §9.1 names the one-grep experiment.

**2026-08-08 - §3 (the room protocol) HAS BEEN BUILT ANYWAY, AND LEFT UNARMED.** `shadnet-server`
now dispatches **20 of 40** commands: all sixteen of 100-115, with per-connection notification
outboxes (the stream is split; `encode_notification` had zero callers before), the emitter guards
for `details`/`addr`+`port`/`ctx_id`, per-npid identities with `LoginAlreadyLoggedIn(6)` on
collision, and **33 controls** checked against shadPS4's own enum text and the vendored schema
rather than against our constants. **Defaults are unchanged and byte-identical** - `matching2_enabled
= false`, room commands answer `Unsupported(33)`, `capture-rig/*.sh` needed no edit. `--rooms on`
serves them. **No session keys are issued and a control enforces that** (§1's verdict).
⇒ The remaining Plan B work below is unchanged; §3 is no longer a reason not to do §3b.

- [ ] ⭐ **`d1-responder`: the PSN Session WebAPI.** ~6 endpoints - `POST /v1/sessions`
      (`multipart/mixed`, boundary `BUNGIEr0x0rz`, store the `session-data` part **verbatim**),
      `GET /v1/sessions/{id}/sessionData` (**exactly 78 bytes back**), `POST /v1/sessions/{id}/members`,
      the invitation pair, `GET/PUT /v1/sessions/{id}`. Point shadPS4's `shadnet_webapi_server` at us.
      **Control: a 77- or 79-byte body must be REJECTED**, and the join must then fail at the *named*
      site `failed to create managed session for client` (`0xB6BBE0`) rather than stall. **Spec §3b.**
- [ ] **Pin `apiGroup` vs `userPath` before writing a single route** - shadPS4 composes
      `base_url + userPath` and ignores apiGroup. It logs both. `[U]` today.
- [ ] ⭐ **ZERO-COST, DO FIRST: read the existing run logs.** Did the managed-session POST succeed,
      or did the client take an offline branch? `Asynchronous session creation completed successfully`
      **and** `…failed` are **both** `seen=1` at `0xB69A10`. No new run needed.
- [ ] **One grep closes the whole Matching2 branch**: `grep Lib_NpMatching2 <run>.log`
      (positive control in the same grep: `Lib_NpWebApi` must be present).
- [ ] **Cheapest harness win: does BAP 38-41 (`party_invite`) carry the 78-byte descriptor?**
      If yes, a two-instance join needs **no PSN invitation plumbing at all** - pure `d1-bap`.

### ⛔⛔ NINE LOAD-BEARING CRATES ARE UNTRACKED IN GIT - including every instrument we cite `[C]` 2026-08-10

- [ ] ⭐ **`git add d1-server/crates/d1-replay`.** Found while landing `--stack-canary`: the replay
      oracle - **4,626 lines, 13 controls (one inverted), 10 unit tests**, the artifact doc
      `historicaldocs/design/replay-oracle.md`, and the instrument two agents worked on all night - **has never been
      in version control.** There is no baseline, so there was no `git diff` to review, no three-way
      merge available, and the only way to land a second agent's work was to **reconstruct the base
      by reverse-applying a hand-written patch file**. That worked (machine.rs/controls.rs came back
      byte-identical, which is what proved ownership was disjoint), but it is not a procedure anyone
      should need twice. ⚠ It is **not** `.gitignore`d - `git check-ignore` exits 1. It was simply
      never added.
- [ ] **And it is not alone.** Every one of these is untracked, ~46k lines total:
      `d1-oracle` (9,984) · `d1-bap` (7,588) · `d1-activity-host` (6,971) · `d1-eboot` (6,722) ·
      `d1-replay` (4,626) · `d1-harness` (3,504) · `d1-live` (3,144) · `d1-loadout` (2,937) ·
      `d1-pkg` (936) - plus `shadnet-server/src/{lib,controls,matching,registry,rooms,signaling}.rs`
      and its whole `tests/` directory. ⛔ **`d1-eboot`, `d1-live` and `d1-replay` are the three
      instruments the evidence convention tells every session to run before citing a finding.**
      A control that is not in version control cannot be shown to have been passing when the finding
      was made.
- [ ] ⚠ **Check `target/` is excluded before adding** (the unanchored `target/` rule covers it) -
      the last `git add` of a standalone crate proposed 12,257 files for exactly this reason.

### ⛔ Instrument defects found - published negatives may be wrong until fixed

- [ ] **`d1-eboot` NID oracle uses base64 `+/`; PS4 uses `+-`.** **86 of 638 imports (13.5%) silently
      unresolvable** - every NID containing `-`. All four pinned NID controls use `-`-free NIDs, so
      the control **cannot** detect it. One-character fix **plus a `-`-containing control**.
- [ ] **`d1-eboot xref` riprel scanner drops disp32 when the instruction carries a trailing
      immediate** (`80 3D <disp32> <imm8>`). `xref 0x266deac` reports **0** references while `disasm`
      resolves **3** - with all four xref controls passing. *Any "this global is never touched"
      conclusion since the crate shipped is suspect.* Test: `xref(0x266DEAC)` must return exactly 3
      sites at `0x2E8088`, `0x2E8110`, `0x2E8124`.
      ⚠ **This defect now has a published consumer.** `historicaldocs/design/replay-oracle.md` §10.2 cites
      `xref 0x1a06c78` = 25,848 sites / 16,117 functions / 35,834 = **44.98%** for how much of the
      binary the stack canary blocks. Because every miss is a reference NOT found, that figure is a
      **floor** and is quoted as **"≥45%"** with the direction of error named - ⛔ it must never be
      reused for a claim needing an **upper** bound. Fixing the scanner will move it **up**.
- [ ] **`capture-rig/single-participant-fixture.py` mints SOIDs in the banned `0xD1…` space and has
      NO banned-SOID assertion** - it was driven to emit exactly `0xD100000000000001` and `validate()`
      accepted it. Also plain SHA-256 rather than HMAC, and its documented invocation name is wrong.
- [ ] **`capture-rig/two-instance-layout.py`'s port plan is not realizable** - `game_port` is not
      settable (the guest hardcodes 3074/2001/2002) and instance-B's `3075` collides with
      `d1-responder`'s BAP listener. Its self-test only checks the two differ *from each other*.
      **Delete the field.** All three fixture generators are Python prototypes owed a Rust port.

### Two-instance harness - DESIGNED, LEFT UNARMED (activation is the operator's call)

- [ ] **Topology:** outer netns + bridge holding the servers at `10.77.0.1`; two inner netns at
      `.2`/`.3`. **Two namespaces are MANDATORY** - the guest binds UDP 3074/2001/2002 on `0.0.0.0`
      and none is parameterised. Containment is unchanged: still no physical iface, no host veth.
- [ ] ⛔ **`shadnet-server` must bind `10.77.0.1`, not loopback** - loopback is per-namespace, so the
      current `--bind 127.0.0.1` would be invisible to **both** instances.
- [ ] ⭐ **`sceNetCtlGetInfo: local ip: failed to retrieve` is ROOT-CAUSED and it is our own doing.**
      `RetrieveIp()` `connect()`s to `1.1.1.1:80` and reads `getsockname()`; our containment has no
      default route ⇒ `ENETUNREACH`. **Adding a route is strictly worse - measured: the connect
      HANGS (20 s+ under a cap; production sets no timeout).** Fix is emulator-side: `getifaddrs()`,
      already used twice in the same file. Otherwise **both peers advertise `0.0.0.0`**.
- [ ] **`d1 preflight2p`** - refuse-by-default, exit 2, an unknown is a refusal: identity distinct at
      all four layers · reachability · containment in all three namespaces · **each instance reports
      its own 10.77.0.x** · two pads created *before* entering their namespaces · uninstrumented
      control run on file · manifest set for both · **both `--release` and `-p` binaries verified**.
- [ ] **`run-signon-experiment.sh:291` `kill %1 %2 %3` is job specs, not PIDs** - unsafe the moment a
      second emulator exists, and silent. Use a per-run pidfile.

⛔ **Forbidden as acceptance evidence: "both reached the same screen" and "it didn't crash."**
The bar is §8: two distinct participants · a blob A never generated arriving at B · bidirectional
per-plane packets · **exactly one** elected host · **one action authoritatively applied and observed
by the other peer, with a severed-link negative control.**
