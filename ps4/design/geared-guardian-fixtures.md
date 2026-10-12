# The geared-Guardian fixture set - art arrangements, equip ordinals, and Light 385

**2026-08-10, late.** Task: produce a character fixture set that can actually render a geared
Guardian, and prove as much of it as possible **offline**. The four defects
`inventory-instance-fields.md` §0 names were found **already fixed in the tree** (verified below,
§1) - and the verification pass surfaced **two further defects nobody had named**, both now fixed
and both measured from the packages, plus the impossible Light 400 default.

**Reach:** every mechanism here is `static-client-inference` (decompiled corpus + `d1-eboot`,
27/27 controls) or **package bytes** (the client's own content, extracted and identity-checked);
the executions are `d1-replay` on `runs/snapshots/orbit-items4.snap` (20/20 controls) - a
`synthetic_ancestor` snapshot. **Nothing here is `observed-live`; no fixture has yet drawn a
Guardian on screen. The boot in §7 is the discriminator.**

---

## 0. The answer in one table

| defect | state | evidence |
|---|---|---|
| 1. items land in **Materials** (positional tab) | ✅ **already fixed** (`profile.rs` places by bucket) | §1.1 - fixture decode **and** the client's own leaf executed per slot |
| 2. `element+0x24 = 1` ('Bronze Tier') | ✅ **already fixed** (`inventory rebuild`) | §1.2 - staged decode: `+0x24 = 6299` |
| 3. `+0x18`/`+0x1C` carry hashes | ✅ **already fixed** (level 40 / equip-min 0; guards refuse the old values) | §1.2 |
| 4. zero-filled card stat table (Light overwritten ×15) | ✅ **already fixed** (`0xFF` sentinels) | §1.3 |
| ⭐ **5. NEW: equipment art-arrangement `+0x0C` = 0 and extras `+0x0E` = 0×8** | ✅ **fixed this session** | §2 - the render gate, measured end to end |
| ⭐ **6. NEW: equipped arrays filled in FILE ORDER, but the consumer indexes by the item's own def ordinal** | ✅ **fixed this session** | §3 - `def_client+0x18`, emblem cross-check |
| 7. **Light 400** - impossible (RoI caps at 385) | ✅ **fixed**: `LIGHT_ROI_CAP = 385` everywhere | §4 |

**New fixtures (the old ones are untouched, as evidence):**
`capture-rig/characters/char-geared-385.bin` · `capture-rig/profiles/profile-geared-385.bin` ·
`capture-rig/rosters-f0-geared/{f0t0,f0t1,f2t0,f2t1}-*.bin` - every one encoded by the **client's
own encoder** and decoded back `object IDENTICAL`. Operator run: §7.

---

## 1. The four known defects, re-verified against what is actually on disk

The brief described the old symptoms as current. They are not: the tree's 2026-08-10 01:03-01:04
regeneration (`d1-oracle` `inventory rebuild` / `profile build` / `character build`) already
carries defects 1-4 fixed. Verified by decoding the *current* fixtures through the client's own
codec (`d1-oracle object --decode`), not by reading the builder:

### 1.1 Placement - fixture decode + the client's own leaf, executed

`profile-geared.bin` (and the new `profile-geared-385.bin`) populate slots
`{40, 43, 53, 63, 73, 83, 93, 103, 113, 143, 153}`, **nothing in 0-19**. Then the leaf itself,
`FUN_0026DB60(table, scope, slot)`, executed on `orbit-items4.snap` (table resolved by replaying
`0x25D250` → `a:0x380A2C298`, same address the live client reported in
`capture-rig/inventory-buckets.tsv`):

```
slot 40→0x06 Subclass   43→0x07 Helmet    53→0x08 Gauntlets  63→0x09 Chest
     73→0x0A Leg        83→0x0B Class     93→0x02 Primary   103→0x03 Special
    113→0x04 Heavy     143→0x0D Vehicle  153→0x0E Ghost
    322→0xFF NOT-FOUND  ← the inverted control; "found" means found
```

Zero slots resolve to `0x05` Materials. ⚠ Two staged items (`Artifacts`, `Sparrow Horn`) are
deliberately **not** in the backpack array - their bucket ids are among the five undetermined
count-10 buckets and `slot_for_bucket` refuses to guess (they stay equipped via `+0x2B80`).

### 1.2 The element - staged decode (03-Heavy Weapons.bin)

`+0x00 = 0x…0304` · `+0x18 = 40` · `+0x1C = 0` · `+0x24 = 6299` (Gjallarhorn) · `+0x26..` = 8 ×
`0xFFFF` · `+0x44 = 0xFFFF` · `+0x58 = 0xFFFF` · `+0x86 = 1`. The negative controls
(`NEGATIVE-legacy-element-refused`, `-hash-as-level-`, `-bucket-hash-as-equip-min-`) still refuse
the old values.

### 1.3 The card - `char-geared.bin` decode

Stats `{0:…}` at entry 0 with `0xFF` in all 15 unused keys; equipment content indices filled;
emblem 1512 at slot 12. **But** every element's `+0x0C` read `0` and every `+0x0E` array read
zeros - which is defect 5 - and the equipment was in file order - defect 6 - and Light read 400 -
defect 7.

---

## 2. ⭐⭐ Defect 5: the render gate, measured end to end - and `0` was a live wrong answer

### 2.1 The mechanism `[C]` (decompiled corpus; addresses verifiable with `d1-eboot disasm`)

`FUN_0054CF70` builds the render description from the **appearance block** `0x80801855` - the
2,568-byte block the family-3/1 card carries at `+0x68`, family-0/1 at `+0x30`, family-1/1 at
`+0xB60`. Callers (all three confirmed by `d1-eboot xref 0x54cf70`, 4/4 positive controls):

| caller | resolves | is |
|---|---|---|
| `0x54EB40` ← `0xA98550` (`cui::c_cinematic_texture_camera_widget`) | **family 3/1 `+0x68`** or family 1/1 `+0xB60` (mode byte) | ⭐ the character PREVIEW |
| `0x54CF40` ← `0x58D350` | family 0/1 `+0x30` | the world-tag loader cache (peers) |
| `0x54D580` ← same widget, mode ≠1,3 | family 4 (via `0x574220(4)`) | `[I]` the profile route |

Per equipment slot (element `0x8080183C`, 64 B, at `block+0x1E8 + slot*0x40`), the builder:

1. pre-fills the output - four u64 at `out + 0x188 + slot*0x20` - with `-1` = **no geometry**;
2. reads `+0x08` (content index; `0xFFFF` skips) and resolves the definition via `vtbl[0x410]`
   (`0x25ADE0` - which **also** requires the item's world tag loaded, mode-2 gate);
3. requires the definition's **gear record** - `def+0x60`, self-relative - to be non-null;
4. ⭐ reads **`+0x0C`** (`param_5 + 0x1F4 + slot*0x40`) and resolves it with `FUN_0054B530` - a
   bounds-checked `index < count` lookup into the 24-byte-stride **art-arrangement table**
   (content, root `+0x310`, page-table addressed);
5. on success `FUN_0054E6C0` fills the four geometry handles, additionally walking the **8 × u16
   extras at `+0x0E`** - a fixed 8-iteration loop in which only `-1` is a skip.

### 2.2 Executed on the client's own table (`d1-replay`, orbit-items4.snap, 20/20 controls)

```
FUN_0054B530(_, 1229)   → 0x383F26778   a real arrangement record
FUN_0054B530(_, 0)      → 0x383F1F440   ⛔ ALSO a real record - arrangement #0
FUN_0054B530(_, 0xFFFF) → 0             the sentinel fails the bound: clean "no art"
```

⇒ the `0` every fixture shipped at `+0x0C` (and ×8 at `+0x0E`) was a **live reference to
arrangement #0** - whose record carries real per-gender content hashes (`0xE0C7EB78`/`0x8836C0C6`,
read off the snapshot) - never an "empty". The `0xFFFF`-vs-`0x0000` sentinel trap, fourth
appearance (`status/LESSONS.md` #16).

### 2.3 ⭐ The correct value is CONTENT, and it is extractable offline

The class-correct arrangement is `FUN_00289E80(def+0x60 gear record, class)`: the record is
`{i64 count, i64 self-rel → entries at +0x10, stride 4: {i8 class(-1=any), u8, u16 arrangement}}` -
first entry whose class matches, skipping `-1` arrangements. The **definition** is the package
entry named by the item's world tag (page `(tag>>13)&0x3FF` = the pkg suffix, entry `tag&0x1FFF`),
and ⭐ **the entry carries the item's own hash at `+0x78`** - an identity check no wrong entry can
pass.

**Instrument: `capture-rig/oodle/art-arrangement.py`** (Python prototype - port obligation noted in
its header) → **`capture-rig/item-art-arrangement.tsv`**, consumed by
`d1_oracle::item_index::ArtTable` (which re-checks two anchors at load). Controls - **9, all
watched, one inverted**:

| control | what it pins |
|---|---|
| `identity-hash-ci-{6299,710,1}` | three entries carry their own item hash at `+0x78` |
| `gear-record-ci-{6299,710,1}` | Gjallarhorn HAS a gear record; Striker (subclass) and 'Bronze Tier' (`hasGeometry:false`) have **NULL** - the no-geometry negatives discriminate |
| `gjallarhorn-arrangement` | ci 6299 → **1229**, all three classes (wildcard entry) |
| `NEGATIVE-shifted-base-differs` | the walk from `gear+2` does NOT reproduce 1229 |
| `emblem-equip-slot-is-12` | §3's cross-instrument anchor |

**The measured arrangements (Hunter loadout, all wildcard-class):** Helmet 6488→**3362**,
Gauntlets 5918→**4075**, Chest 3886→**2943**, Legs 5922→**4079**, Class Armor 5855→**2459**,
Primary 5040→**3450**, Special 5260→**3740**, Heavy 6299→**1229**. No arrangement (correctly):
Subclass 710, Ghost 4528, Vehicle 6362 (gear record present, **0 entries**), Artifact 4286,
Sparrow Horn 5348, emblem 1512.

---

## 3. ⭐⭐ Defect 6: the equipped arrays are ORDINAL-indexed, and file order was wrong for nearly every item

`character-screen-consumers.md` §5 named this its *"highest-value next measurement"*: `0x269080`
derefs `character + 0x2B80 + slot*8` where **`slot = def_client+0x18`** - the item's own
definition names which of the 20 entries it occupies
(`def_client = def + 0x18 + *(i64*)(def+0x18)`, self-relative). It closed **offline**, from the
same package entries:

| ordinal | item (measured) | | ordinal | item |
|---|---|---|---|---|
| 0 | Subclass | | 8 | **Heavy** |
| 1 | Helmet | | 10 | Vehicle |
| 2 | Gauntlets | | 11 | Ghost |
| 3 | Chest | | **12** | **EMBLEM** ✓ |
| 4 | Legs | | 15 | Sparrow Horn |
| 5 | Class Armor | | 16 | Artifact |
| 6 | Primary | | | |
| 7 | Special | | | |

⭐ **The cross-instrument anchor:** the emblem definition (ci 1512) says **12** - the byte a
*snapshot replay* of `investment_globals->vtbl[0x160]()[0x18]` independently returned
(`item_index::EMBLEM_SLOT`). Two instruments, one byte. `ArtTable::load` re-checks it on every
load, and the inversion was watched: corrupting the row to 11 makes the loader **refuse** with a
named error (§6).

⇒ the old fixtures put Primary (ordinal 6) at file position 1, Gjallarhorn (ordinal 8) at 3, and
so on - so `0x269080` resolved the **wrong instance for almost every slot**, independently of
everything else. Both builders now place by ordinal (card equipment array **and** profile
`+0x2B80`), refuse collisions, refuse an item claiming ordinal 12, and leave an unmeasured item
OUT with a warning rather than parking it at a guess.

---

## 4. Defect 7: Light 400 is impossible - `LIGHT_ROI_CAP = 385`

400 exceeds the Rise of Iron cap (385); a screenshot with 400 Light is self-evidently invented.
New constant `d1_oracle::profile::LIGHT_ROI_CAP = 385`, used by: `ProfileSpec::default`,
`CharacterSpec::default`, `RosterSpec::default`, and all four CLI `--light` defaults
(`profile build`, `character build`, `character build-peer`, `roster build`). The card's
`stat-sentinel-is-required` control now reads its expectation from the spec instead of a literal.
⚠ `[R]` era knowledge: whether the *client* enforces any cap is `[U]` - the point is that the
default is now a value a real service could have said.

⚠ The old geared fixtures also carried stat overrides `{1:400, 11:400}` (Defense/Attack). Those
were pure invention; the new profile ships **Light only**. Defense/Attack on screens now come from
gear - which still sums to 0 because the elements' `+0x98` stat arrays are empty (§8).

---

## 5. What changed in the tree

| file | change |
|---|---|
| `capture-rig/oodle/art-arrangement.py` | **new instrument**: package-side definition extraction (via `extract-entry.py`), gear-record walk, equip-ordinal parse; 9 controls, exit 2 |
| `capture-rig/item-art-arrangement.tsv` | **new generated table**: `content_index → {class → arrangement}, equip_slot`, with provenance + anchors in the header |
| `d1-server/crates/d1-oracle/src/item_index.rs` | `ArtTable` loader (provenance + two anchor checks), `EMPTY_ARRANGEMENT`, `equip_slot_for` |
| `d1-server/crates/d1-oracle/src/character.rs` | `EQUIP_ART_ARRANGEMENT`/`EQUIP_EXTRA_ARRANGEMENTS` constants; `EquipSlot.art_arrangement`; sentinel fill of `+0x0C`/`+0x0E` for all 20 slots; ordinal placement in `equipment_from_inventory_with`; `derive_layout` extended to fields [3]/[4] (kind-checked); 2 new controls |
| `d1-server/crates/d1-oracle/src/profile.rs` | `LIGHT_ROI_CAP` |
| `d1-server/crates/d1-oracle/src/main.rs` | art table wiring in `character build` / `build-peer`; ordinal-indexed `+0x2B80` in `profile build`; `--light` defaults |
| `d1-server/crates/d1-oracle/src/roster.rs` | default light 385 |
| `capture-rig/oodle/d1-oodle-harness.py` | ⛔ **repair**: the 2026-08-10 scrub replaced its literal paths with the un-expanded string `"$HOME/…"`, which `open()` cannot read - every consumer (extract-entry, item-index, content-table) died on import. Paths now derive from the file's own location |

`cargo build --release -p d1-oracle` clean; `cargo test --release -p d1-oracle` **45 passed**;
all six control suites (`inventory`/`profile`/`character`/`roster`/`control`/`vectors`) exit 0;
`d1-eboot controls` 27/27.

### The new fixtures - each encoded by the client's own encoder, decoded back IDENTICAL

| file | what it carries |
|---|---|
| `capture-rig/characters/char-geared-385.bin` | 3/1 card: level 40, Light **385**, 14 items at their **measured ordinals**, 8 with **measured arrangements** at `+0x0C`, `0xFFFF` sentinels in every unused `+0x08`/`+0x0C`/`+0x0E` |
| `capture-rig/profiles/profile-geared-385.bin` | 4/1 profile: **ordinal-indexed** `+0x2B80`, 11 backpack slots placed by bucket (none in Materials), Light-385 override, progression `{117: 400000}`, all-content bools set |
| `capture-rig/rosters-f0-geared/` | the family-0 pair (`f0t0`/`f0t1` - the object `0x9A3C30` reads for the character screen) with the same geared block, plus the family-2 pair |

---

## 6. Controls that were watched failing (this session's additions)

| control | inversion | result |
|---|---|---|
| `emblem-equip-slot-is-12` / `ArtTable` anchor | edited the TSV row to `11` | **refused**: *"emblem ci 1512 carries equip_slot 11, the replayed emblem slot is 12"*; restore → loads clean |
| `NEGATIVE-shifted-base-differs` | built-in: the walk from `gear+2` | returns `None`, not 1229 |
| `art-extras-extent-NEGATIVE` | writes into element `+0x1E` (the gap after the extras array) | codec **refuses at `+0x26e`** - the array really is 8 × u16 at `+0x0E` |
| `gear-record-ci-710/1` | two no-geometry items | **NULL** gear - "has arrangement" discriminates |
| replay 3-way on `FUN_0054B530` | `0` vs `1229` vs `0xFFFF` | record / record / **0** - the sentinel claim is executed, not inferred |

⚠ `art-arrangement-round-trip` (positive) has not been separately watched red; its refusal power
is demonstrated by its sibling `art-extras-extent-NEGATIVE` in the same region.

---

## 7. The operator run

```sh
d1-server/target/release/d1-harness pad run --fifo /tmp/d1pad &   # pad FIRST, as always

D1_PROFILE_FILE=capture-rig/profiles/profile-geared-385.bin \
D1_CHARACTER_FILE=capture-rig/characters/char-geared-385.bin \
D1_INVENTORY=capture-rig/inventory \
D1_ROSTER_DIR=capture-rig/rosters-f0-geared \
D1_ANSWER_FAMILIES=0,2 \
SHADPS4_BIN=~/dev/shadPS4-d1trace/build-d1trace/shadps4 \
    capture-rig/run-boot.sh GEARED-385
```

### What each outcome means

| observation | conclusion |
|---|---|
| **gear MODELS appear** (preview / in-world) | the whole §2 chain confirmed: arrangement indices + ordinals + tag loads all sufficient |
| tabs right, **Light 385**, still no models | the remaining gate is **content residency**: either the definition tag or the **art tag** is unloaded (`character-screen-consumers.md` §5's spinner `[U]`); the fixture side is done. Next instrument: read `FUN_0054C090`'s art record live for ci 6299 |
| items vanish from the equipment screen (vs before) | ordinal placement is wrong somewhere - decode the card, compare `def_client+0x18` per item against the TSV |
| character screen (pause) now shows 40/385 | family 0/1 confirmed as its reader (`character-screen-consumers.md` §7) |
| pause still 0/0 | vary `--card-key`/`--contact-key`, or the reader is in family-1 mode - serve family 1/1 (never built; `[U]`) |
| `…queuez_id_mismatch_in_header_and_rsat_payload` ("porcupine") in the log | the f0t1 `+0x00` link key regressed - §9.1; rebuild with `build-peer` (whose identity matrix refuses to write such a pair) |

⛔⛔ **The activity-ladder interlock (rungs 34-36).** `CREATION-BUILD-ORDER.md`'s task-7 lane
requires a family-0/1 whose twenty equipment `+0x08` are **all `0xFFFF`** - a *populated* f0/1
pins the world-tag loader slot at state 2 (its inner family-0/**3** lookups can never resolve;
that family has no container). **`rosters-f0-geared` is for orbit/render experiments; activity
runs should keep `D1_ROSTER_DIR=capture-rig/rosters-f0` (the empty-equipment pair).** One
fixture cannot serve both goals; this is a run-goal switch, not a bug.

---

## 8. What is still missing or `[U]`, and what closes each

| gap | instrument |
|---|---|
| **do the arrangements DRAW?** | the §7 boot - the only observable is the screen |
| the **art tag residency** (spinners) | live: `d1guest`-read `FUN_0054C090`'s 0x10-byte art record `+0x08` for ci 6299 during the run |
| ~~items' `+0x98` per-instance stats still empty~~ ✅ **MEASURED AND CLOSED 2026-08-11 (§9.2): `def+0x68` is NULL or empty for ALL 14 items, so the client's own constructor (`FUN_0026E0E0`) writes nothing into `+0x88`/`+0x98` - empty is content-correct.** The remaining stat channel is the talent-grid chain (§9.4) | the extractor ran; the gap moved to §9.4 |
| element `+0x0A` (u16) and `+0x20`/`+0x28` sub-records of `0x8080183C` | no identified reader; `d1-eboot field-xref` once a holder is named |
| family **1/1** carrier (`+0xB60`) never served; preview mode byte decides 3-vs-1 | ⇒ §10: the watch is proven armable but the measurement needs one operator screen-open; recipe and instrument state written out there |
| whether `def_client+0x18` ordinals vary per class (armor) | extract a Titan/Warlock armor piece and compare; all 13 current items measured wildcard-or-fixed |
| the arrangement records' own content (per-gender hashes vs `0x811C9DC5` unset in record 1229) | `FUN_0058D910`'s resolution path - read after a successful draw, not before |

**Taint:** everything downstream of these fixtures is `synthetic_ancestor` - level 40, Light 385,
the loadout and its ordinals are numbers *we* chose; only the arrangement indices, ordinals and
bucket ranges are the client's own content, measured.

---

## 9. 2026-08-11 - the offline verification pass before the §7 boot

Every claim here is from decoding the **on-disk** fixtures through the client's own codec
(`d1-oracle object --decode`) and from package bytes; nothing is `observed-live`.

### 9.1 ⛔⛔ `rosters-f0-geared` was PORCUPINE-STALE - found by decode, rebuilt

The 2026-08-10 20:50 `f0t1-d100000000000501.bin` decoded with **`+0x00 = 0xd100000000000101`**
(the pre-fix `CHARACTER_SOID`) - it was built **before** the link-key fix that
`rosters-f0-hybrid` (2026-08-11 02:16) carries. The client compares the QueueZ header key
(= filename key) against the RSAT payload's leading qword and rejects a mismatch with
`error.investment_server_object_queuez_id_mismatch_in_header_and_rsat_payload` ("porcupine")
**before the object enters the registry** - so the §7 run as staged would have silently lost the
family-0/1 peer view, and with it the pause-screen character (`0x9A3C30` reads family 0/1) *and*
the peer render route (`0x58D350 → 0x54CF40` resolves family 0/1 `+0x30`). `[C]` by decode of both
files; the mechanism is `main.rs`'s build-peer identity matrix (which now refuses to write such a
pair).

**Rebuilt** with `d1-oracle character build-peer --out-dir capture-rig/rosters-f0-geared
--with-family2` (defaults: level 40, Light 385, emblem 1512@12, equipment from
`capture-rig/inventory`). Verified by decode: `+0x00 = 0xd100000000000501` = filename key; all 14
items at their measured ordinals with their measured arrangements (Gjallarhorn slot 8 arr 1229,
Oryx Mask slot 1 arr 3362, …); stat[0] `{0, 385}` at `+0x838`. The f2 pair is byte-identical to
the hybrid's. `char-geared-385.bin` and `profile-geared-385.bin` re-verified by decode the same
way (ordinals, arrangements, Light 385, bucket-placed 340-slot array, ordinal-indexed `+0x2B80`) -
**no defect found in either; they were not rebuilt.**

### 9.2 ⭐ `def+0x68` measured: the stat list is EMPTY for every staged item - so empty `+0x88`/`+0x98` is CONTENT-CORRECT

The TODO's *"fill `+0x88`/`+0x98` from `def+0x68`"* premise is now measured. The client's own
element default-constructor is **`FUN_0026E0E0`**: it memcpy's a schema-default template, stamps
`+0x24`/`+0x00`, and then - **gated on `*(i64*)(def+0x68) != 0`** - calls `FUN_0028B6E0(rec,
element)`, which copies the def's stat list `{u16 stat index, u32 value}` pairs into `+0x88`/
`+0x98`. Walked from the same cached package definitions (`art-arrangement.py`, controls
`stat-list-*`): **all 13 staged items + the emblem carry a NULL `def+0x68`** (Striker and Oryx
Mask carry a present-but-empty record - the two shapes pin the walk). ⇒ the client itself would
write nothing; our empty arrays reproduce the constructor exactly. **Gear-derived Attack/Defense
reading 0 is not fixable from `def+0x68` for this loadout** - the live channel is §9.4.

### 9.3 ⭐ One real drift found and fixed: element `+0x20` (stat-curve index offset)

The same constructor sets `element+0x20 = *(u16*)(rec30+8)` of the `def+0x30` self-relative
record. Measured: **Oryx Mask (ci 6488) carries 70; every other staged item 0** (Gjallarhorn's
record is *present with 0*, which discriminates "present, 0" from "absent → 0").
`FUN_002816F0` samples the stat-group curve at `+0x20 + level`, so the helmet's Defense curve was
being sampled 70 points low. Extractor + TSV columns `curve_offset`/`stats` added
(`art-arrangement.py`, now **14 controls** incl. `NEGATIVE-curve-shifted-base-differs`);
`ArtTable` loads both (new anchor: ci 6488 → 70; a 6-column table still loads with both
unmeasured); `inventory rebuild` fills both. Elements rebuilt: `05-Helmet.bin` now decodes with
`+0x20 = 70`. Card/profile are unaffected (they do not carry element `+0x20`).

### 9.4 `[U]` the talent-grid chain is the remaining stat/progression channel - a NEW extractor, out of this lane

`FUN_002837F0` (called by the constructor with the `def+0x58` record) shows the client-correct
`+0x44`/`+0x58`/`+0x7E`/`+0x86`: `element+0x58 = *(u16*)rec58` (a **talent-grid index**, resolved
via `vtbl[0x4E0]` - table 156, a *different*, tag-gated content table), `element+0x44 =
*(u16*)(grid_def+0x30)` (the progression ordinal), and the step/variant bytes are **LCG-seeded
random rolls** bounded by per-node bytes of the grid def. `def+0x58` is present for 12 of 14
staged items. Closing this needs the grid-table page walk (another package entry) - named here,
not attempted. Until then `+0x44`/`+0x58` stay `0xFFFF`, which forfeits the level/grid stat terms
but never leaks grid #0 (`LESSONS.md` #16).

---

## 10. 2026-08-11 - the preview mode-byte measurement: instrument proven armable, hit not yet obtained

Goal (gap 3): learn which family the pause/select 3D preview asks for, by catching
`cui::c_cinematic_texture_camera_widget` (`0xA98550`) live and reading its `param_1+0x120`
mode byte (1 = family 1/1 carrier `+0xB60`, 3 = family 3/1 carrier `+0x68`, else the profile
route via `0x54D580`).

### 10.1 What was measured `[C]`

* The estate context is ideal: this run's client holds families 3 and 4 resident
  (`geared-visuals-run-1.md` §7), so a mode-3 answer would name a carrier we already serve.
* **First time `d1-live watch` was pointed at the live client.** Arming is clean: 112/112
  threads armed, 0 failed, **0 unattributed sigtraps** (shadPS4 does not use SIGTRAP
  internally), 209 SIGILL + 1132 SIGSEGV per 90 s forwarded un-swallowed (shadPS4 fastmem
  traffic). Closes the "does it arm at all" half of `live-watch.md` §10.
* **No exec hit was obtained in any window** - `0xA98550` (2 windows, 75 s + 90 s),
  `0xAF7050` (network pump complainer, 30 s), `0x60ce10` (unconfirmed sessionid-emitter
  candidate, 18 s). ⚠ No window contained a positive control that fired, so per
  `status/LESSONS.md` NONE of these no-hits is interpretable - including about the widget.
  "Exec watchpoints fire on shadPS4 guest threads at all" is still `[U]`, now sharply framed:
  arming is proven, firing is not.

### 10.2 Why the blind attempt stalled `[C]`

* Opening the character screen leaves **zero** log signature (verified against the operator's
  earlier screen-open in this same run: no ui_stage, no cui line) - so an agent that cannot
  see the screen has no same-window control for "the screen actually opened".
* SDL delivers controller input only to the **focused** window; the emulator sat unfocused
  through the first attempt. Two controllers are bound (slot 0 + slot 1) and which one owns
  the signed-in UI is unrecorded.

### 10.3 The 60-second recipe that completes it (operator present, no boot)

1. Arm: `d1-live watch --addr 0xA98550 --rw exec --len 1 --count 3 --max-seconds 600 --json`
   (a 600 s watch armed 2026-08-11 ~06:25 writes to
   `runs/20260811-050950-GEARED-385-VISUALS-X11/laneB-widget-watch.json`).
2. Operator opens the character screen (touchpad) while it runs.
3. The hit's JSON carries the full register file; `rdi` is `param_1`. Read the byte:
   `d1guest.py read --abs <rdi+0x120> --len 1` (same client, no re-arm needed).
4. Value 3 = the widget wants the family-3/1 carrier we already serve (then the empty preview
   is a §2-chain or residency question); 1 = it wants the never-built family-1/1 carrier
   (build it); anything else = the profile route `[I]`.
   Control in the same window: a second screen-open must produce a second hit.
5. If the hit never fires even with the screen visibly opening, first prove the instrument on
   a KNOWN-firing address before reading the no-hit (the still-open `live-watch.md` §10
   control); until then the no-hit says nothing about the widget.
