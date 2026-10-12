# Grants and unlocks - the canonical reference

**State (2026-08-11).** Everything the server GRANTS a character and how the client checks it, melted
from THE-GRANT-CONTRACT, the unlock-flag reference, the rung-33 gate docs and the seven
`design/grants/` surfaces (all now under `historicaldocs/design/`). The grant pipeline is proven end
to end (run `20260809-075545-RUNG33-UNLOCK-98`: flags shipped on 205/503 read `0x02` in the live
plane). A **ten-flag** set `{98, 308, 419, 440, 834, 2667, 3389, 5015, 5987, 6018}` clears both
rung-33 walls (verified live as a plane poke on 2026-08-10, walking the client to rung 36); the
d1-bap constants that would put it on the wire still carry the superseded eight-flag body. A
**31-flag** set clears the availability gate for all 546 activities offline. Nothing on this whole
surface has ever been `observed-live` against a real Bungie service.

## Contents

- §1 The grant contract - what the server owes vs what it has sent
- §2 The unlock-flag machinery - table, setter, VM, plane, values, manifest limits
- §3 The rung-33 gates - DLC, permissions, poison flags, the deliverable bodies
- §4 Per-surface reference - 205/503 object, profile, completion flags, account, progressions, inventory, family 4/4, ownership
- §5 Delivery semantics - containers, announcers, baselines, silent failures
- §6 Open questions and unresolved contradictions
- §7 Dead claims
- §8 Sources

Evidence tags `[C]/[O]/[R]/[I]/[U]` and reach tags per `CLAUDE.md`. Reach for nearly everything
here: `static-client-inference` and `replayed-offline` (the client's own code under Unicorn on its
own memory), inheriting `synthetic_ancestor` x2 from the snapshot. `observed-live` below always
means "against our own server", still `synthetic_ancestor`.

---

## §1 THE GRANT CONTRACT

### 1.1 Provenance

Synthesised 2026-08-09 from seven independently-enumerated surfaces, each with a named consumer
instruction per object and a whole-index co-occurrence scan over all **4,555** descriptors. Every
static claim was produced with `d1-eboot controls` 22/22 PASS (build
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`) and, where replayed,
`d1-replay controls runs/snapshots/orbit-all.snap` 10 PASS / 0 FAIL. Machine-readable companions:
`design/grants/flag-index-registry.tsv` (51 seeded rows),
`capture-rig/schemas/unlock-flag-index-registry.tsv` (all 6,944 rows),
`capture-rig/schemas/progression-grants.tsv` (247 rows).

Everything in this contract is a contract we can **implement**, not one we have observed a real
server honour. A grant that satisfies our own mock is not evidence about Bungie's.

### 1.2 The honest numbers (as of 2026-08-09)

**30** grant-bearing fields have a named consumer instruction. **1** is correctly served (by
omission - the deny-list). **0** carried content on the pinned default path. Across **229** runs
with a `responder.log` (N = 229, the full corpus, `[C]`):

| marker in `runs/*/responder.log` | runs |
|---|---:|
| carried unlock flags (v1 = `{98, 834, 5015, 6018}`) | **1** - `20260809-075545-RUNG33-UNLOCK-98` |
| carried the flagless pinned 503 body | 142 |
| neither marker (pre-dates `D1_503_RAW`, or no hex echo) | 86 |
| carried the v2 8-flag constant that exists in the crate | **0** |

The client's own denominators for a fully-granted, level-40, campaign-complete account:

| state class | denominator | ever delivered (as of the audit) |
|---|---:|---:|
| unlock **flags** (u16 index space) | 6,944 defined of 8,192 | 4, once |
| unlock **values** (u32) | 238 defined of 2,048 | 0 |
| **completion** flags (storage kind 3) | 1,967 | 0 |
| **progressions** | 247 definitions, 127 character + 127 account slots | 1 (`character_level`, only with an override) |
| **inventory** items | 340 profile + 326 account slots + 666 detail records | 13 (with an override; `C+0x24` wrong on all 13) |
| **equipment** | 20 slots | 13 (with an override) |
| **offer keys** (`/SignOn` field 10) | up to 64 | 0 |

### 1.3 The default path fabricates state

`capture-rig/run-boot.sh` exports **zero** of the seven grant knobs (`D1_205_RAW`, `D1_INVENTORY`,
`D1_PROGRESSIONS`, `D1_ROSTER_DIR` not exported; `D1_CHARACTER_FILE=""`, `D1_UNLOCKS=""`,
`D1_ANSWER_FAMILIES=""`), pins the flagless 503 body at line 55, and pins `D1_PROFILE_FILE` to
`profile-old-russia.bin` at line 152 - a file whose own control (`d1-oracle profile controls` →
`baseline-is-empty`) certifies 2 non-zero qwords in 28,792 bytes. `[C]`

⛔ **Worse than empty: the zero-filled profile inventory index fabricates grants.** The client's
empty-slot sentinel is `0xFFFF`; `0x0000` is a legal item-definition index. `0x28C6D0`'s
bucket-occupancy loop does not consult the count, so every profile bucket reads as occupied and the
client stamps six unearned unlock flags: **2831, 2843, 3282, 5148, 5149, 5150** `[C]`. The
discriminating control is in the same snapshot: `account+0x4C0` is correctly `0xFFFF`-filled
(built by our Rust; the profile is a hand-made fixture). Same client, same reader, two blobs, one
right. This also contaminates `runs/snapshots/orbit-all.snap` (§6).

### 1.4 The 30 grant-bearing fields, enumerated

A "grant-bearing field" = a field of a server-authored object with (a) a named consumer instruction
and (b) a role in granting state. Family 4/type 4 is **excluded** - it failed the consumer check
outright (`d1-eboot xref 0x260C20` → 0 confirmed references, all four kinds, four positive controls
PASS, `0x260C10` the discriminating positive in the same aligned run; §4.7).

| # | object · field | status at audit |
|---|---|---|
| 1 | `0x80801941`[2] deny-list `+0x010` | ✅ served by omission |
| 2 | `0x80801941`[3] sparse flags `+0x03C` | 4 flags, 1 of 229 runs |
| 3 | `0x80801941`[4] sparse values `+0x108` | never |
| 4 | `0x80801941`[6] `vfunc[0x58]` bit `+0x2DC` | ships 0 (correct - §3.6) |
| 5 | account `[9]` progressions `+0x304C` | impossible (see below) |
| 6 | account `[10]` dense flags `+0x383C` | impossible |
| 7 | account `[11]` dense values `+0x4430` | impossible |
| 8 | account `[12]` sparse flags `+0x4838` | impossible |
| 9 | account `[12]` sparse values `+0x493C` | impossible |
| 10 | account `[14]` dense flags `+0x51A0` | impossible |
| 11 | account inventory `+0x4C8` (326) | impossible |
| 12 | account values region `+0x4B40` (plane kind 6) | ⚠ not in the wire schema - an 88-byte hole between account fields [12] and [13]; the plane builder reads a region the server cannot write `[I]` |
| 13 | profile `[1]` character-creation key `+0x008` | ships `00 00 00` |
| 14 | profile `[3]` inventory index `+0x030`/`+0x038` (340) | 13 w/ override; zero-filled by default |
| 15 | profile `[5]` equipment `+0x2B80` (20) | 13 w/ override |
| 16 | profile `[8]` progressions `+0x3660` (127) | 1 w/ override |
| 17 | profile `[11]` dense completion flags `+0x52C0` (1,967) | never |
| 18 | profile `[12]` dense values `+0x5AC0` (174) | never |
| 19 | profile `[13]` timestamps `+0x6AC0` (41) | never |
| 20 | profile `[15]` sparse flags `+0x6D54` (20) | never (+ double-gated) |
| 21 | profile `[15]` sparse values `+0x6DA8` (20) | never |
| 22 | profile stat override `+0x6CDC`/`+0x6CE0` | Light 400 w/ override |
| 23 | profile all-content bypass `+0x6E4C` | set, but inert (gated on #4) |
| 24 | family 4/3 item record (incl. `+0x38` progression, `C+0x24`) | 13 w/ override, `C+0x24` wrong |
| 25 | family 3/1 character card | served (CREATE-only) |
| 26 | family 2/0 display name + clan tag | w/ override, never announced |
| 27 | family 2/1 player card `+0x08`/`+0x0A` | w/ override, never announced |
| 28 | `/SignOn` response field 10 `id_list` | never |
| 29 | CDN manifest `entries[]` | never |
| 30 | CDN manifest `Entry.f5` bypass | never |

Rows 5-11 are "impossible" for one shared reason: `account_rsat` emits 3 of 25 account fields and
pushes `false` for fields 4-24 - one missing encoder, not seven independent gaps. ⚠ The table is
the union of what seven surfaces enumerated, not a proof no 31st exists.

### 1.5 A fully-granted account, in delivery order

| # | grant | object · field | channel | note |
|---|---|---|---|---|
| 1 | 31 unlock flags | `0x80801941`[3] | ✅ 205 + 503 | §3.5. Clears both rung-33 gates |
| 2 | deny-list empty | `0x80801941`[2] | ✅ by omission | count 0 ⇒ `0x28A170` returns "allowed". Populating it takes activities away |
| 3 | `vfunc[0x58]` bit 0 | `0x80801941`[6] | ✅ by omission | leave at 0 - §3.6 |
| 4 | 1,967 completion flags | profile `[11]` `+0x52C0` | ⛔ no CLI | 768 B already on the wire, all zero - the grant costs ZERO extra bytes |
| 5 | 174 unlock values | profile `[12]` `+0x5AC0` | ⛔ no CLI | 4,096 B already on the wire, all zero |
| 6 | 80 progressions | profile `[8]` `+0x3660` | ⚠ `--progression` exists | 80 of 247 rows grant ≥1 flag and cover all 144 progression-granted flags; 127 slots, 47 spare |
| 7 | items | profile `[3]` `+0x38` + family 4/3 | ⚠ `D1_INVENTORY` | ⛔ `0xFFFF`-fill mandatory; `C+0x24` hard-coded `1` |
| 8 | equipment | profile `[5]` `+0x2B80` | ✅ derived | 20 x u64 copies of instance keys |
| 9 | Light | profile `+0x6CDC`/`+0x6CE0` | ✅ `--light` | stat index 0; base is `+0x6CE0`, not the retracted `+0x6CE8` |
| 10 | account dense flags | account `[10]` `+0x383C`, `[14]` `+0x51A0` | ⛔ impossible | `account_rsat` pushes `false` for fields 4-24 |
| 11 | offer keys | `/SignOn` f10 | ⛔ never sent | near-zero value - §4.8 |

### 1.6 Build order (status as of the sources' last write)

- **B0 - instrument the message→plane path.** ✅ **CLOSED** by run `20260809-075545`: the four v1
  flags read `0x02` in the **live** plane at the exact addresses `0x28CACD` reads
  (`0x37f8bd39e / 0x37f8bd67e / 0x37f8be6d3 / 0x37f8beabe`), neighbours zero. The delivery channel
  works; the wrongness was elsewhere (§3.2). Also still worth doing: raise `log_req()`'s 500-char
  cap and `signon()`'s 256-byte hexdump (`d1-responder/src/main.rs:142`, `:451`) - the `/SignOn`
  request is 798 bytes and field 15 (the client's own offer-key list) has been truncated away for
  229 runs.
- **B1 - ship v2 (8 flags).** Superseded: the 8-flag body reproduces `missing_permissions`
  (§3.6). Its role as falsifier was fulfilled by the 2026-08-09/10 live results.
- **B2 - regenerate the flag pair through the client's own encoder.** ⛔ OPEN and load-bearing.
  Four of seven previously-published hex bodies lost a nibble in transcription (odd hex length -
  §3.6). Never transcribe; regenerate, with the pinned-body positive control first.
- **B3 - a `D1_UNLOCK_FLAGS=98,305,...` knob** building both bodies through `d1-oracle` and feeding
  `check_205_503_agree` (`main.rs:1918`). Strengthen the guard: it proves the bodies match, never
  what they contain - two byte-identical empty bodies pass. Add a decoded-count assertion.
- **B4 - fix `run-boot.sh` defaults; make the responder refuse a zero-filled profile** (the
  `D1_MANIFEST_FILE` treatment). Print a sha256 of every fixture path in the run banner
  (`profile-allcontent.bin` and `profile-lvl40.bin` are both exactly 20,332 bytes; which was on the
  wire in `20260809-075545` is not recoverable from the run directory).
- **B5 - `--complete` on `d1-oracle profile build`.** Field [11] is fixed-length: 768 B on the wire
  in every run already; one flag and all 1,967 cost the same. Encoder half proven (bytes at
  `+0x52C0[56]/[62]/[938] = 2` round-trip with the payload differing at exactly 3 offsets). Staged,
  not blanket: 1,762 of 1,967 are unlabelled.
- **B6 - ship 80 progressions, not 1** (§4.5). Send the final count with zero values first.
- **B7 - fix the inventory**: (a) `0xFFFF`-fill; (b) `C+0x24` hard-coded `1` at `main.rs:1758`,
  correct value in `d1_oracle::item_index::IndexTable::require`; (c) all 13 items land in bucket
  ordinal 5 because `d1-oracle/src/main.rs:684` pushes slots in staged order, leaving every
  grant-bearing bucket (16, 20, 26, 32, 33, 37) empty.
- **B8 - build the UPDATE path** (unlocks 0/1, 2/0, 2/1, the 4/0 edge rule, the 4/1 count gate,
  any mid-session grant). ⛔ Settle §6.1 first.
- **B9 - `d1-oracle account build`** (no `account.rs` exists; blocks 2,662 + 266 dense flags, 64
  sparse, 64 values, 127 account progressions, 326 inventory slots). ⛔ `D1_ACCOUNT_RSAT`
  (`main.rs:1246`) is not a shortcut - it sends the account without the profile, and a family-4
  baseline that omits the profile purges it (§5.3). ⚠ An all-zero 25-field account encodes to
  17,449 B, growing the family-4 body to ~40 KB; largest body ever observed working is 22,990 B.
  `[U]` whether the client's BAP path accepts that.

---

## §2 THE UNLOCK-FLAG MACHINERY

### 2.1 The index space and the master table

The evaluator's key is a **u16 flag index ≤ 0x1FFF** (8,192 slots). The client carries its own
master flag table:

```
0x25A320()                       -> c_investment_globals_interface singleton (DAT_02639ae0)
  vfunc[0x2B8] = 0x25F800        -> THE MASTER FLAG TABLE (snapshot abs 0x3809C86A8, 55,648 B)
      header {u32 count = 6944; i64 reloff = 0x10; u32 tag 0x80800184; u32 count; u32 tag 0x808018B8}
      rows at table+0x28, stride 8:  { u32 flagHash @+0 ; u16 storage_kind @+4 ; u16 subindex @+6 }
```

⭐ **The row's ORDINAL is the u16 index the evaluator uses** - not a field, its position. The
client states this itself: `0x28CAE0` (the setter) subscripts the table by the flag index
(`movsx eax, byte [rax + flag_index*8 + 0x14]`). `[C]`

6,944 of 8,192 indices are defined; the table is dense, so every index ≥ 6944 is dead space. `[C]`
⚠ Two later surfaces read rows at `table+0x28`; the earlier rung-33 doc said `+0x20` in the passage
whose search result was retracted (§7). Use `+0x28`.

`storage_kind` names the dense array that persists the flag; `subindex` is the slot in it:

| kind | n | dense home | schema tag | capacity | granted by |
|---:|---:|---|---|---:|---|
| 0 | 2,049 | **NONE** (`subindex == 0xFFFF` for all) | - | - | pair lists / content only |
| 1 | 2,662 | account `+0x383C` | `0x80800300` | 3,060 B | account field [10] |
| 2 | 266 | account `+0x51A0` | `0x80800302` | 512 B | account field [14] → `0x80801807`[1] |
| 3 | 1,967 | profile `+0x52C0` | `0x80804C21` | 2,048 B | profile field [11] |

Whole-array controls (not samples): the per-kind directory tables (`vfunc[0x290]` 2,662 /
`vfunc[0x298]` 266 / `vfunc[0x2A0]` 1,967) are exactly the kind-partition of the master table -
plane indices == ordinals of every kind-N row in order, hashes == master ids at those ordinals,
266/266 and 1967/1967; the same list shifted by one fails. `[C]`

### 2.2 Hash → index, and the drift

⭐⭐ **The manifest's `flagHash` values DO resolve on this client: 10 of 10 value-populated manifest
flags found, and all 6,944 client ids join into the manifest's 7,731-hash universe - a 100% join.**
(This retracts the earlier "0 of 9" negative - §7.) Hashes are stable 2016→2020. **Indices are
not**: the manifest's `value` column drifts from the true ordinal by 0..2:

| manifest hash | manifest `value` | true ordinal | Δ | kind | sub | label |
|---:|---:|---:|---:|---:|---:|---|
| 3638915056 | 98 | **98** | 0 | 1 | 57 | `director_book_visible:The Crucible` |
| 4228285953 | 159 | **159** | 0 | 3 | 56 | `activity_completion:The Dark Beyond` |
| 2388787017 | 170 | **170** | 0 | 3 | 62 | `activity_completion:A Stranger's Call` |
| 3230331915 | 185 | **185** | 0 | 3 | 70 | `activity_completion:A Key Awaits` |
| 1680320212 | 2311 | **2310** | -1 | 0 | - | `item_custom_dye` |
| 1681748763 | 2539 | **2538** | -1 | 0 | - | `director_book_visible:Strike Playlist` |
| 1631415873 | 2549 | **2548** | -1 | 0 | - | shared Moon/Venus/Mars/Reef gate |
| 4131347936 | 3558 | **3556** | -2 | 0 | - | Moon 2nd gate |
| 2748347740 | 3587 | **3585** | -2 | 0 | - | Venus 2nd gate |
| 2617018872 | 3850 | **3848** | -2 | 3 | 936 | `activity_completion:The Dreadnaught` |

Δ is 0 up to 185, -1 from 2310, -2 from 3556: the 2020 content inserted two flags below ordinal
3556 that 01.29 lacks. `[I]` from a 10-point sample - the insertion points are `[U]`; **do not
interpolate**. The safe rule needs no interpolation: **resolve `flagHash` → row ordinal through the
client's own table; never copy `value`.** A regression test worth having:
`assert ordinal(0x643D731B) == 2538`, not 2539. This drift is exactly why the off-by-one negative
`{99, 835, 5016, 6019}` scored 0/21.

### 2.3 The setter `0x28CAE0` - and kind 0 has no store

```
0x28cae0  set_unlock_flag(ctx, u16 flag_index /*esi*/, u8 value /*edx*/)
    tbl  = investment_globals->vfunc[0x2B8]()
    kind = *(i8 *)(tbl + [tbl+8] + 0x1c + index*8)      ; element+4
    sub  = *(i16*)(tbl + [tbl+8] + 0x18 + index*8 + 6)  ; element+6
    kind==3 -> dest = ctx->vfunc[0x90]() + 0x52C0 + sub   ; PROFILE
    kind==2 -> dest = ctx->vfunc[0x88]() + 0x51A0 + sub   ; ACCOUNT
    kind==1 -> dest = ctx->vfunc[0x88]() + 0x383C + sub   ; ACCOUNT
    kind==0 -> return                                     ; ⛔ NO STORE (0x28CB5F cmp eax,1 / jne 0x28CB9B)
    if (*dest != value) { *dest = value; *ctx->vfunc[0xC8]() = 0; }   ; write-on-change + cache invalidate
```

⛔ **Kind-0 flags cannot be set by index even by the client's own setter.** They reach the plane
only through the `{u16, u2}` pair lists (arms 0/1/5) or content (arms 2/9/0xB/0xC). That is a hard
property of the design, and it is why the four kind-0 director gates (2538, 2548, 3556, 3585) and
10 of the 31 clearing flags must ride a pair list. `[C]`

The setter's invalidation byte (`ctx->vfunc[0xC8]() = 0`) is the same byte `0x574440` clears
(`0x574440` calls `[DAT_026a34b0 + 200]`, and 200 == 0xC8). Two independent paths, one byte. `[C]`

### 2.4 The expression VM `0x28DEA0`

A 64-deep postfix stack machine over 8-byte steps `{u8 op @+0; u32 operand @+4}`. The complete
opcode set, read from the client's own jump table (this outranks every manifest-derived inference;
the manifest content only ever uses nine ops):

| op | client semantics | earlier (manifest-derived) reading |
|---:|---|---|
| 1 | `push plane[u16 operand] == 2` (FLAG) | FLAG ✅ |
| 2 | NOT (`top = top==0`) | NOT ✅ |
| **3** | **OR** - `a!=0 ? 1 : (b!=0)` | ⛔ said AND |
| **4** | **AND** - `a!=0 ? (b!=0) : 0` | ⛔ said OR |
| 5 | NOR | - |
| 6, 9 | `!=` | - |
| 7 | NAND | - |
| 8 | `==` | ⛔ said "CMP ≈ >=" |
| 10 | `push values[u16 operand]` (plane+0x2000, u32, idx < 0x800) (VALUE) | VALUE ✅ |
| 11 | `push u32 literal` (CONST) | CONST ✅ |
| **12** | `>` | "NCMP = NOT op8" - the complement measurement was right, the operators wrong |
| 13 | `>=` · 14 `<` · 15 `<=` | - |
| 16..20 | `+ - * / %` (div/mod guard against 0) | - |
| **21** | **NEGATE** (`top = -top`) | ⛔ `[U]` "COERCE" |

Facts that survive from the manifest-side analysis `[C, measured]`:
- 3,065 non-empty expressions, 2,549 empty (empty = literal "unconditional" - why Earth's director
  book has no gate); 3,472 steps total; exactly 10 distinct operator shapes.
- `value == -1` on exactly the arity-2 operators (all 100 occurrences of op3/op4/op8/op12; all
  3,372 others carry `value >= 0`) - the sentinel that pinned the arity assignment.
- The five Rise-of-Iron location pairs are `VALUE(V) CONST(1) NEGATE` then op8/op12, i.e.
  `V == -1` vs `V > -1` - exact complements, which is the regularity that was measured correctly
  and explained wrongly.
- `value` on a `FLAG` step, where populated (10 flags, director books and inventory items only),
  **is the runtime flag index** - independently confirmed by the live client reading plane index 98
  for every Crucible activity. It is a property of the site, not the flag (flag 2617018872 carries
  3850 in the director book and 0 in its own location release). But it is the 2020 index - resolve
  through the client table (§2.2).
- Two runtime diagnostics: expressions are evaluated at activity completion to decide chaining
  (`0x56D3F0`: "the chain's unlock_expression evaluates as false -- electing to not
  activity-chain") `[C]`; unlock values are addressed by a primary/secondary index pair in the
  leaderboard code `[C]`.

⚠ Every infix rendering in the archived reference (director books, record books, locations) was
printed under the inverted assignment: **where it printed AND read OR, and vice versa.** Under the
real operators the Xur tie-breaker inverts cleanly: his releases evaluate to Reef = all three flags,
Tower = any-but-not-all, nowhere when none set - correct for a weekend vendor; the exhaustiveness
premise was the error.

⚠ `strings eboot.bin` tokens `equal/greater/lessequal/notequal/greaterequal` at
`0x159a417`-`0x159a5d4` are Adobe glyph names in a font table, not an operator enum. Recorded so the
next reader does not pay for the lead twice.

### 2.5 The availability plane and its twelve sources

The plane read by the activity/director path lives at `[[0x26A34B0+0x38]]+0xC`: `0x2000` flag bytes
plus `0x800` u32 values (`0x4000` total). "Owned" == byte value **2** exactly (`0x28CAC0` is
`cmp byte,2; sete` - value `0x01` fails, `0x02` passes, `0x03` fails, measured). It is stamped by
`0x260200` and is a **cache** (§2.7).

Two builders run different arm sets of `0x28B7B0`:

| builder | arms | consequence |
|---|---|---|
| `0x28D970` (full) | `0xC, 0xB`, inventory, then 9..0 descending | everything; kind 0 last, so the profile sparse array wins collisions |
| `0x28BFB0` (base) | `[0xC, 3, 1, 0]` only (from `DAT_01613500`) | kinds 4 and 7 never run; kind 3 runs but its compaction keeps only 2 of 266 bytes |

`0x28DBA0` merges the scratch planes (kinds 9..0 winning), then the inventory plane, then kinds
0xB/0xC.

The twelve arms, with consumer instructions:

| arm | source object | flags: instruction | values: instruction | capacity | key |
|---:|---|---|---|---:|---|
| **0** | profile `0x80801743` (4/1), `[ctx+0x18]` | `0x28b824 mov cl,[r15+rax*4+0x6d5a]` (`+0x6D54`/`+0x6D58`) | `0x28b855` (`+0x6DA8`/`+0x6DAC`) | 20 / 20 | sparse `{u16, u2}` - ⛔ gated on `vfunc[0x58]` |
| **1** | msg 205/503 object `0x80801941`, `[ctx+0x08]` | `0x28b8a0 mov dl,[rax+rcx*4+0x42]` (`+0x3C`/`+0x40`) | `0x28b8d0` (`+0x108`/`+0x10C`) | 50 / 50 | sparse - ⭐ ungated, read live at every build |
| 2 | profile `+0x2C20/22/24` → content lookup | - | - | - | content |
| **3** | account `+0x51A0` (kind-2 dense) | `0x28ba00 mov cl,[rbx+rdx+0x51a0]` | - | 266 / 512 | dense, by subindex |
| **4** | account `+0x383C` (+ family 4/2), via `0x28C340` | `0x28c390 mov bl,[r12+rdx+0x383c]` → `0x28c3a5 mov [r13+rsi],bl` | `+0x4430`, `vfunc[0x2C0]` | 2,662 / 3,060; 64 values | dense, by subindex |
| **5** | account sparse `0x808018C7`, `[ctx+0x10]` | `0x28ba60 mov cl,[rbx+rax*4+0x483e]` (`+0x4838`/`+0x483C`) | `0x28ba90` (`+0x493C`/`+0x4940`) | 64 / 64 | sparse |
| 6 | account `+0x4B40` | - | `0x28bad3` | - | sparse values only - ⚠ not in the wire schema (§1.4 row 12) |
| **7** | profile dense | `0x28bb60 mov bl,[r15+rdx+0x52c0]` | `0x28bbc5 mov edi,[r15+rdx*4+0x5ac0]` | 1,967 / 2,048; 174 values | dense, by subindex |
| 8 | account + character progressions → `0x28C9A0` | `0x28ca52 mov byte [r12+rsi], 2` | - | - | progression-derived (§4.5) |
| 9 | profile `+0x6AC0` timestamps vs `0x264E50` | - | - | 41 | time-derived |
| 0xB | profile `+8` → `0x2661D0` | - | - | - | content (character-creation key triple) |
| 0xC | static content, time-gated (`vfunc[0x2B0]`, 8,192 entries) | - | - | - | content |

⭐ **Arm 1 - the 50-slot pair list on `0x80801941` - is the only unlock channel that survives BOTH
builders**, is ungated, and is keyed by the runtime index directly, so it carries any of the 8,192
indices regardless of storage kind. For the ten kind-0 clearing flags it is (with the other two
sparse arrays) the only route that exists. `[C]`

⚠ **The base-builder trap.** `0x28BFB0`'s kind-3 arm builds into scratch and copies back exactly
two bytes, chosen by BSS globals: `DAT_0263a100 = 0x428899AE` → index 833 (kind 2, sub 11);
`DAT_0263a104 = 0x0FCFFEF9` → index 832 (kind 2, sub 10). Any grant reaching the client only
through `0x28BFB0` loses the other 264 account-dense flags, silently. What fills those globals is
`[U]`.

⚠ **Null objects are silent**: `0x28D970` sets a boolean when an object pointer is null and stamps
the plane anyway.

### 2.6 Plane readers, and the two layout traps

```
FLAG   0x28cac0  cmp si, 0x1fff        ; index ≤ 8191
       0x28cacd  cmp byte [rdi+rax], 2 ; rdi = plane = [ctx+0x38]+0xC.  THE instruction
VALUE  0x28cc40  cmp si, 0x7ff         ; value index ≤ 2047
       0x28cc4b  mov eax, [rdi+rax*4]  ; rdi = plane+0x2000, u32 values
```

- ⚠ **Stride trap.** `0x28B7B0` writes values into a `0x6000`-byte SCRATCH buffer at
  `+0x2000 + idx*8` (presence byte) / `+0x2004 + idx*8` (u32); `0x28BFB0` then compacts into the
  real `0x4000`-byte plane at stride **4**. Writer index j == reader index j. A server object's
  `{u16 index, u32 value}` at index j is read by `VALUE(j)`. Anyone deriving the plane layout from
  `0x28B7B0`'s offsets gets it wrong; anyone deriving a server object's indices from them is right.
- The value scratch is compacted only where the presence byte is set; `memset(plane,0,0x4000)`
  first, flags copied 1:1.

`0x28DE40` (the expression-evaluate wrapper) has **68 call sites in 53 functions** - the complete
census of things in this client that consult unlock flags (`d1-eboot xref`, 4/4 controls PASS).
Named consumers include `c_investment_character_progression_iterator`,
`c_investment_inventory_visible_iterator<0,340>`, the UPDATE announcer `0x56D3F0`,
`cui::s_activity_skull_modifier`, `cui::s_activity_type_unlock_requirement`,
`cui::s_activity_requirements_not_met_entry`, `cui::c_faction_data_component::vfunc[14]`,
`cui::s_investment_material_requirement`,
`cui::c_investment_vendor_interaction_display_component::vfunc[14]`, `cui::c_investment_vendor_item`,
`cui::s_investment_unlock_flag`, plus 41 unnamed functions at `0x263xxx-0x290xxx` and
`0x543xxx-0x555xxx`. Only the activity-availability gate has been swept; the other 52 sites are
enumerable by the same method and were not run.

### 2.7 Cache invalidation - the family-5/5 rule

The plane is stamped once (`0x260200`'s last stores set `valid = 1` **on success and failure
alike**) and `0x25FF40` short-circuits on the validity byte forever after. `0x574440(family)` is
the only clearer (`if (family == 4) *((char*)vfunc[0xC8](DAT_026a34b0)) = 0;`), and
`d1-eboot xref 0x574440` (4/4 controls PASS) finds exactly two reachers - **neither is the family-4
UPDATE announcer**:

1. `0x56D1A0 @ 0x56D1FB` - the family-4 **end-of-family** handler (batch completion).
2. `0x56E830 @ 0x56E835` - a two-instruction thunk (`mov edi,4; jmp 0x574440`) in the padding
   between `0x56E7B0`'s `ret` and `0x56E840`'s prologue, called twice from `0x56BF80` (the
   hard-coded **family-5/5** handler, descriptor at `+0x4FA30`) at `0x56C106`/`0x56C11F`, on
   create (`*slot == 0`) and on change (`memcmp != 0`).

⇒ **Writing or CHANGING `0x80801941` - the object that rides inside BAP 205/503 and never crosses
QueueZ - is what clears the family-4 availability cache.** `[C]` Consequences:

- ✅ The grant is self-invalidating: delivering the flags changes `0x80801941`, firing
  `0x56BF80` → `0x56E830` → `0x574440(4)`.
- ⛔ **Byte-identical is a no-op.** With 503 and 205 carrying identical `0x80801941` (the guard
  enforces it), the only clear in a whole run happens at 503 (responder.log line ~140), 35 lines
  before the family-4 baseline (line 177). Every one of the 23 later 205s changes nothing. If
  anything stamps the plane between those lines it is stamped from an empty family 4 and never
  invalidated again for the life of the process. `[C]` mechanism, `[I]` whether it currently bites.
  Cheap fix if it does: make the second 205 differ by one byte.
- One control worth keeping: `0x574440` clears through `vfunc[0xC8]`, `0x25FF40` reads through
  `vfunc[0xC0]`; the vtable at `0x18F5AF0` has both pointing at `0x260BD0` (ICF-folded), so the
  clear does invalidate what the reader consults. (`[U]`: whether `DAT_026A34B0` and `0x25FF40`'s
  `param_1` are the same instance.)

### 2.8 Unlock VALUES - the second space

Flags are booleans; unlock values are integers **in a different hash space**. The two spaces
overlap in exactly one hash (`861538567`) out of 7,731 x 1,375. `[C]`
- `DestinyObjectiveDefinition.unlockValueHash` is populated on 1,377 of 1,388 objectives (1,375
  distinct, all with `contentIdentifier`); 1,231 equal the objective's own hash, 146 do not.
- Expressions read values through op10; only two value hashes appear in the manifest content:
  `2246811594` (the SRL statistic, 16 record expressions) and `408185636` (also the `itemHash` of
  the Rise of Iron questline root). `[I]` the value tracks RoI questline progress.
- Runtime: **238 defined value indices** of 2,048 accepted - 64 dense in `account+0x4430`, 174
  dense in `profile+0x5AC0`, disjoint, max index 1382. `[C]`
- Per-item value contributions combine by a content rule (`0x28D210`, `vfunc[0x538](idx)`, byte
  `+5`): 1 = MIN, 2 = MAX, **3 = SUM** (a running total - the shape of a currency/material total).
- ⇒ the profile must carry **two** unlock structures; any model of "unlocks" as a single bitfield
  is wrong by construction.

### 2.9 What the manifest can and cannot say

The archived world manifest is Bungie's public **2020** capture; the target client is 01.29 (2016).
Generator: `capture-rig/unlock-flag-reference.py` (68 controls, exit 2 on failure); tables in
`capture-rig/schemas/unlock-*.tsv` + `unlock-flag-reference.json`:
`unlock-flag-catalogue.tsv` (7,731) · `unlock-flag-references.tsv` (3,728) · `unlock-gates.tsv`
(44) · `unlock-activity-flags.tsv` (564) · `unlock-expressions.tsv` (5,614) ·
`unlock-operators.tsv` (9) · `unlock-values.tsv` (1,379) · `unlock-flag-index-candidates.tsv` (10) ·
`unlock-stripped-fields.tsv` (13) · `unlock-hash-collisions.tsv` (23).

- **Coverage: 446 of 7,731 flags (5.8%) have any manifest reference at all.** 7,285 are invisible
  to the manifest - referenced by content (the packages), not the API. "Not in the manifest" is not
  evidence a flag is unused (the two-gate trap in a new costume).
- Ten unlock fields are present in the schema and zero in every row (each paired with a populated
  positive control, so "all zero" cannot be a reader bug): `DestinyGrimoireCardDefinition.unlockFlagHash`
  (834 rows), `...rankCollection[].unlockFlagHash` (180), `DestinyInventoryItemDefinition.bountyResetUnlockHash`
  (8,609), `.questTrackingUnlockValueHash` (8,609), `DestinyVendorDefinition.sales[].licenseUnlockHash`
  (8,382), `.unlockValueHash` (95), `DestinySpecialEventDefinition.unlockEventHash`/`.activeUnlockValueHash`
  (3/3), `DestinyTriumphSetDefinition.lockdownUnlockHash` (1), `DestinyBondDefinition.providedUnlockValueHash`
  (4). Positive controls: `DestinyObjectiveDefinition.unlockValueHash` 1,377/1,388,
  `DestinyActivityDefinition.completionFlagHash` 564/653, `DestinyBondDefinition.providedUnlockHash` 1/4.
- Whole gating axes with no manifest counterpart (RTTI from our own eboot):
  `s_activity_type_unlock_requirement` (activity TYPES carry up to 16 unlock requirements),
  `s_investment_activity_playing_requirements_*` (checked while playing, distinct from entry),
  `s_investment_activity_requirement_client_definition`, and
  `s_investment_activity_unlock_string_client_definition` /
  `..._strings_label_client_definition` - ⭐ **the client ships human-readable names for unlock
  requirements, in the packages we already have dumped.** That is where the names of the 7,285
  unlabelled flags (and 6,637 of the 6,944 indices) live. Highest-value offline question on this
  surface.
- The UI shows at most 4 unmet requirements (`s_activity_requirements_not_met_entry`,
  `c_static_vector<...,4>`).
- All 7,731 `DestinyUnlockFlagDefinition` rows are `{flagHash, unlockType: 0, isOffer: false}` - a
  flag is meaningful only through what references it.
- Hash-collision hygiene: a blind integer walk over the manifest produced 67 collisions
  (e.g. "154 perkHashes references" are real sandbox perks). Reference count is 446, not 513;
  full list `unlock-hash-collisions.tsv`.

### 2.10 What gates what (content survey)

⚠ Operator note: infix renderings below name the flags only; the archived reference's AND/OR
tokens were inverted (§2.4).

**Director books (9)** - `isVisibleExpression` flags; all nine `isDefaultExpression`s empty:

| book | nodes | gate flags |
|---|---:|---|
| `2707330101` Old Russia, Earth | 53 | UNCONDITIONAL (empty expression) |
| `473350382` (unnamed) | 31 | UNCONDITIONAL |
| `986601339` Ocean of Storms, Moon | 34 | 4131347936 · complete[The Dark Beyond] 4228285953 · 1631415873 |
| `2043310511` Ishtar Sink, Venus | 33 | 2748347740 · complete[A Stranger's Call] 2388787017 · 1631415873 |
| `499756083` Meridian Bay, Mars | 39 | complete[A Key Awaits] 3230331915 · 1631415873 |
| `3295618674` The Reef, Asteroid Belt | 20 | complete[A Stranger's Call] 2388787017 · 1631415873 |
| `2149902399` Dreadnaught, Rings of Saturn | 16 | complete[The Dreadnaught] 2617018872 |
| `3667195314` The Crucible | 21 | 3638915056 (= index 98) |
| `1648658521` Strike Playlist | 5 | 1681748763 (= index 2538) |

A destination book can be gated on completing a mission on that destination (the Dreadnaught's only
gate is completing "The Dreadnaught") - "book visible" is not a precondition for "mission
reachable"; D1 chains story missions by quest as well as director.

**Record books (5)**: `2175864601` Moments of Triumph → 361852057; `3192310198` SRL → 757683720,
103916830; `3471683641` Competitive Spirit → 232191257; `243968262` Rise of Iron → 539284216;
`840570351` Age of Triumph → 2919915085. Plus 43 `claimedUnlockHash` flags and one
`requirementUnlockExpressions` flag (900223847, AoT page 0). `539284216` also selects the
`cosmo1_devils_lair` location release.

**Location releases (8 gated of 150)**: Xur (`1438636527`, flags 2172408844/45/46);
`2784818374` touchdown (complete[The Dreadnaught] vs NOT); `3028684324` cosmo1_devils_lair
(flag 539284216 vs NOT); five Rise-of-Iron NPC rows (`2681509024` lord_saladin, `3088891288`
shiro_4, `3122562238` efrideet, `3180904991` tyra_karn, `3155644950` iron_observatory) sharing one
expression over value 408185636, flag 3197532640 (Iron Banner vendor flag) and
complete[Iron Temple] 3054678575.

**Vendors (5 carry flags)**: Xur `2796397637` `unlockFlagHashes` 2172408844/45/46; Iron Banner
`2610555297` → 3197532640; Hunter Vanguard `3003633346` `enabledUnlockFlagHashes` → 3121290652;
Titan Vanguard `1990950` → 1238696360; Warlock Vanguard `1575820975` → 2678892537. ⚠ These are a
bare array, not an expression; whether the client ANDs or ORs them is `[U]`.

**Items and records**: `equippingBlock.customDyeExpression` = 2,852 references to exactly ONE flag
(1680320212, index 2310) - rank sites by distinct flags, not raw count. Ranked by distinct flags:
activity completion 212, record rewards 170 (mostly `NOT flag:X` reward variants; named records),
record-book claims 43, director books 9, locations 7, vendors 7, custom dye 1, bonds 1
(`DestinyBondDefinition.providedUnlockHash` 3923613635, `r1_s4_refer_a_friend`). 16 SRL record
expressions read `VALUE(2246811594) CONST(n)` with a comparison (the statistic thresholds).

**Activity completion flags**: 564 `completionFlagHash` rows, **212 distinct** (a completion flag
is per mission, shared across difficulty variants - up to 12 rows share one). 537/564 rows (205/212
distinct) resolve to a client index; **537 of 537 are kind 3** (zero exceptions: "activity
completion" is client kind 3). The 7 absent hashes are era-correct (Age of Triumph 2017, SRL, IB
Mayhem Clash, four reprised strikes). Four consecutive flags span all four vanilla destinations -
the WEEKLY completions `4185808064-4185808067` (`...65` = Nightfall; `...64/66/67` = Heroic tiers
32/26/30): one flag, not one per strike (40 rows share the four). Five flags are both a completion
flag and a gate: 4228285953 (Dark Beyond → Moon), 2388787017 (Stranger's Call → Venus + Reef),
3230331915 (A Key Awaits → Mars), 2617018872 (Dreadnaught → book + touchdown), 3054678575
(Iron Temple → the five RoI NPC locations). `1631415873`, `4131347936`, `2748347740`, `3638915056`,
`1681748763` are set by **no** activity completion - they come from content or the server (and all
five are kind 0 or kind 1: server-delivered).

⭐ **Tool debt**: no `flags` subcommand exists anywhere resolving `flagHash ↔ ordinal ↔ (kind,
subindex)`. Per the Rust rule it belongs in the workspace with the five whole-array controls, the
ten-hash control and the ordinal regression as `#[test]`s plus a `controls` subcommand exiting 2.

---

## §3 THE RUNG-33 GATES

Rung 33 (`setup:activity_world_transition`, task fn `0x7A7420`) runs two gates in order: the DLC
gate `0x54F410` (reason `0x17` `activity_failed_to_load_missing_dlc`, string "has unpurchased
prerequisites", `0x164EC35`), then the permissions gate `0x546E70` (reason `0x18`
`..._missing_permissions`, "is lacking permissions", `0x164ECE4`). Passing both prints
"Starting world-transition." (`0x164ED94`). ⚠ Both failures are **state-transition reasons, not
`error.*` strings** - `grep 'error\.'` shows a clean run. The reason bytes are set at
`0x7A75AE mov esi, 0x17` and `0x7A78D4 mov esi, 0x18`; the emitter is unique (one site in 47,304
decompiled files requests reason `0x17`: `0x7A7420:97 FUN_00782720(0x19, 0x17, ...)`). `[C]`
For the launch seam itself (mode u16 at `blob+0x680` plus map name at `blob+0x688`) see
`design/activity-launch.md`.

### 3.1 Gate 1 - DLC `0x54F410`: unlock state, never ownership

```
0x7A7420 (rung 33's only task)
  └ 0x54F410(activity_index)  ->  0x54F430(idx) != -1
       └ per requirement:  0x28DE40(ctx, req)
            └ 0x28DEA0( 0x25FF40(ctx) + 4 , req )       ; the expression VM
                 └ op 1 FLAG:  0x28CAC0(plane, u16 idx)
                 └ op10 VALUE: 0x28CC40(plane+0x2000, u16 idx)
```

`0x25FF40(ctx)` returns `[ctx+0x38] + 8` - the availability cache, not any family-4/4 object.
Watchpoint replay of `0x54F410(393)` with four stores armed `[C]`: the plane fired (1 read, at
plane+98, from `0x28CACD`); the local offer-key store `0x26A4970`, the signon store `0x263D46C`
and `ownership_authority` `0x26A4A74` never fired. **Rung 33 never touches entitlement**; `/SignOn`
field 10 is not this gate.

Which flags, per activity block (whole-plane watchpoint, offline):

| activity indices | first-requirement flag reads | minimal clearing set (greedy removal) |
|---|---|---|
| 393, 394, 399-414 | `[98]` | `{98, 834}` |
| 395, 396, 397, 398 | `[98]` | `{98, 834, 5015, 6018}` |
| 0-6 | `[3389, 308, 419, 440]` | `{308, 419, 440, 3389}` |
| 11-13 | `[3388, 307, 2667, 834, 833, 830]` | `{307, 830, 833, 834, 2667, 3388}` |
| 380-392, 415 | `[3390/3389/3387, 309/308/305, 5987, 6018, 5015, ...]` | - |
| 416-419 | `[3391, 310, 834, 833, 830]` | - |

⚠ **These are first-requirement measurements, not the requirement set** - evaluation
short-circuits on the first unsatisfied requirement, so granting a flag reveals more. Progressive
closure on 393: `{}` reads `{98}`; `{98}` reads `{98,306,830,833,834,3872}`; adding those reads
`{...,2667,3876,3877}` and the gate passes. Minimal ≠ read.

Flag 98 is `director_book_visible:The Crucible` - two independent instruments agree (the manifest's
`value 98` on hash 3638915056; the live client reading plane index 98 for every Crucible activity).
Kind 1, subindex 57, native home `account+0x383C[57]`. Flags 834 / 5015 / 6018 are required
(dropping 834 makes `0x54F410(393)` fire again - asserted) but unnamed: kind 2 has **0 manifest
references out of 266 flags**; names live in the content's unlock-string label. Do not guess.

Off-by-one negative: `{99, 835, 5016, 6019}` clears 0/21. Value plane: **zero** reads across every
sampled activity, with a forced instrument control that does fire ⇒ no numeric required-level /
required-light predicate exists on this path; a level requirement would be `FLAG(2531+N-1)` and no
sampled activity uses it. `[C]` (sampled: 16 activities under `0x54F410`, 393 under `0x546E70`).

### 3.2 The falsification - the live client gates activity index 0

v1 `{98, 834, 5015, 6018}` was derived from `0x54F410(393)`, shipped, and the live plane reads
proved delivery worked (§1.6 B0) - and rung 33 still refused. Cause, measured: the live client
evaluates `0x54F410(0)`. With and without the v1 grant, index 0 reads the same four addresses
`{3389, 308, 419, 440}` - the shipped flags were not insufficient, they were **irrelevant**.
`0x54F410(0)` with `{308, 419, 440, 3389}` returns 0 (passes). `[C]`

`0x7A7420` takes the index from `0x78C770` → `0x762C00`/`0x2A1F50` → `*(u16*)(blob+0x680)` of the
Activity Host's udt-1 `global_activity_state`. Two independent co-occurring bugs, both since
identified as ours:
1. The AH connection died first (`error.private_activity_host_lost_connection_to_bap`, reason
   `0x65` = 101; sibling `0x68` = 104 `..._host_crashed`, `0xAE` `error.lost_bap_connection`;
   resolved from the name-hash table `0x26B22A0`, stride `0x10`, hash at `+8`, registered by
   `0x783E00`). 9/9 occurrences across two runs the AH error preceded the refusal. Root cause was
   `AhConnection::on_tick` writing inner frames with no envelope - fixed.
2. **`d1-activity-host` sent `blob+0x680 = 0` unconditionally** in every run, repair or no repair
   (the "repair machine de-instantiates" mechanism was refuted in place: at the moment of refusal
   the activity client was alive and re-established). `HASH(0x8ae8da69)` = 2,330,516,073 = "Tower"
   = `activity_table[0]`. `D1_AH_ACTIVITY_INDEX` now sets the field.

Fixing only the connection would have reproduced the same `missing_dlc` line. The refusal's
activity name hash was constant across every refusal in both runs - a downstream symptom read as
the gate.

### 3.3 Gate 2 - permissions `0x546E70`: the same plane, plus a deny-list

`return A && B && (bVar4 ? true : C)` where `0x264620(ctx, idx, &A, &B, &C)` computes:

| | predicate | meaning | measured (393, empty plane) |
|---|---|---|---|
| A | `0x267DB0(edi=idx, sil=override)` - no ctx arg; Ghidra's 6-param signature is wrong | content-rating / engine-shim check: reads `activitydef+0x8C`, `0x266DA0` → `i_investment_engine_shim` (`c_investment_engine_shim_spasaha`, the only impl, vptr `0x1921B50+0x10`, `vfunc[8]` = `0x547420`); "allowed" == result != 7; nothing can produce 7 | **1 - passes** (10/10 for idx ∈ {0,100,200,300,393,395,400,413,420,500}; `[I]` always-true). ⚠ real-client bug: the `xmm0` it compares is an uninitialised register |
| B | `0x2679B0(ctx, idx)` → `0x2678D0` | fireteam requirements (unlock expressions via `0x28DE70` → `0x28DEA0`) + deny-list `0x28A170` | **0 - fails**: `0x267959` `rax = 0` with `rsi = 0x62 = 98` - the expression is false; `0x267976` not-denied = 1 |
| C | `0x267860(ctx, idx)` → `0x2676F0` | leader requirements, then deny-list, then fireteam requirements | **0 - fails** |
| bVar4 | `0x773F30(0)` record, `[+0x0C] != [+0x5D0C]` | fireteam-leader-ish; live both 0 ⇒ bVar4 = 0 ⇒ **C is required** | not a server object |

⚠ Replay caveat carried by every number: `--set-mem v:0x1a06c78=0000000000700000` redirects the
stack-guard pointer (host address the snapshot cannot resolve); behaviour-neutral but a
modification. ⚠ A first replay that passed ctx as A's first argument hit the `test sil,sil`
early-out in 13 steps and read as "A passes" - the corrected call does 866 steps. **The step count
is the discriminator.**

The permissions predicates iterate `s_investment_unlock_flag_expression` (RTTI:
`c_investment_activity_{leader,fireteam}_requirements_iterator` :
`c_investment_label_elements_iterator<..., s_investment_unlock_flag_expression>`) against the SAME
plane through the SAME instruction `0x28CACD`. **Permissions ARE unlocks, evaluated against a
different requirement label.** `[C]`

Flag reads during `0x546E70(393)`: distinct `{98, 306, 830, 833, 834, 2667, 3872, 3876, 3877}`.
Exhaustive 512-subset sweep: 5 passing subsets - `{98,833,2667}`, `{98,834,2667}` (both minimal),
`{98,830,834,2667}`, `{98,833,834,2667}`, `{98,830,833,834,2667}`. Shape:
`flag98 AND (833 OR 834) AND 2667 AND NOT(306 OR 3872 OR 3876 OR 3877)` - ⚠ a truth table over 9
variables is **consistent with** the expression, not the expression; the bytecode itself (step
array `{u8 op @+0; u32 operand @+4}`, count at `rsi+0`, reachable by probing `0x267954`) was never
dumped. With the gate passing, the plane is read 33 times (11 + 11 + 11, exactly the loop shape
`0x2676F0` predicts); on an empty plane, 2. The read count is itself an instrument.

Per-block requirements:

| activities | permissions gate needs | DLC gate needs |
|---|---|---|
| 393, 394, 399-413 | `{98, 834, 2667}` | `{98, 834}` |
| 395-398 | `{98, 834, 5987}` (5987 replaces 2667) | `{98, 834, 5015, 6018}` |

One six-flag set `{98, 834, 2667, 5015, 5987, 6018}` clears BOTH gates for 21/21 of 393-413;
negatives: empty plane 0/21, all-indices+1 0/21. **⚠ STATE THE N: the permissions gate was swept
only over 393-413 and has never been reached in any run (`seen = 0` on all seven of `0x7A7420`'s
strings). Do not report 546/546 for gate 2.**

### 3.4 The poison flags - "grant everything" is a trap

⛔⛔ Setting all 8,192 flag bytes to `2` makes the DLC gate PASS and the permissions gate FAIL -
with a different reason byte (`0x18` not `0x17`) and different string, presenting as a brand-new
wall. Four flags must stay CLEAR: **306, 3872, 3876, 3877** - each alone shuts the gate (measured
one-by-one on top of `{98,834,2667}`; 830 and 833 added are harmless). Zero of 512 passing subsets
contain any of them. What they MEAN is `[U]` ("retired-playlist markers" is a story, not a
finding). All four read 0 in the live plane today, so a targeted grant is safe.

⚠ Transcription landmine: the activity-0 set contains **308**; the poison list contains **306**.
`3389/3390/3391` are granted; `3872/3876/3877` are poison in the same visual neighbourhood.
**Never hand-edit an index.**

### 3.5 The 31-flag full-game set

Sweep of `0x54F430(N)` for N = 0..719 with a whole-plane watchpoint (`0x548860` returns null above
545 ⇒ the activity table is 546 entries), iterated to fixpoint: empty plane → 2/546 pass (those two
read no flag); grant the 30 observed → 545/546 (only 332 left); grant the 70 then seen → 546/546,
zero new reads. **31 indices suffice for 546/546** (the round-1 30 plus 5436):

```
98, 305, 307, 308, 309, 310, 419, 435, 440, 830, 833, 834, 2486, 2508, 2515, 2565,
2600, 2667, 2668, 2669, 2968, 3387, 3388, 3389, 3390, 3391, 3912, 5015, 5436, 5987, 6018
```

Negative controls: the 31 each +1 → 0/120 vs 120/120 for the correct 31 on the same activities.
⚠ "31 suffice", not "31 are each necessary" - leave-one-out over 546 was not run; the one
individually-proven-necessary flag is 98. Synthesis check by set arithmetic `[C]`: the permissions
six-flag core is a **subset** of the 31, and the four poison flags are **absent** from it ⇒ one
31-entry pair list clears both gates (subject to §3.3's N caveat).

Storage split of the 31 (from `capture-rig/schemas/unlock-flag-index-registry.tsv`):

| storage | count | members / where the server writes it |
|---|---:|---|
| kind 0 - no dense slot | 10 | `419, 435, 440, 2486, 2508, 2515, 2565, 2968, 5436, 5987` - MUST ride a sparse pair list (profile 20 / account 64 / 205+503 50) |
| kind 1 - `account+0x383C+sub` | 1 | index 98, subindex 57 (account field [10]) |
| kind 2 - `account+0x51A0+sub` | 18 | subindices `1,3,4,5,6,9,11,12,218,219,220,228,229,230,231,232,249,254` (account field [14]) |
| kind 3 - `profile+0x52C0+sub` | 2 | 2600 → sub 621, 3912 → sub 952 (profile field [11]) |

Over the full 70 gate-read indices: kind 0 = 38, kind 1 = 1, kind 2 = 26, kind 3 = 5. Named among
them: 98 Crucible book; 125/126 `activity_completion:Vault of Glass`; 291
`activity_completion:Prison of Elders`; 2486/2508/2515 `vendor_enabled:{Hunter,Titan,Warlock}
Vanguard`; 3914 `location_release:...iron_observatory/efrideet/lord_saladin`; 5987
`location_release:...cosmo1_devils_lair`. ⭐ All 31 fit in the sparse arrays we already own (134
slots total: 20 + 64 + 50, each keyed by the runtime index directly, no subindex translation); the
dense blocks are the durable path and the only one that scales past 134.

### 3.6 The wire bodies - and the ten-flag deliverable

**The carrier.** `0x80801941` (944 B, 8 fields) has exactly two parents across all 4,555
descriptors, reproduced by four surfaces with four methods, and nothing wraps either parent:
investment **503** response `0x80801825` (968 B, field [4], object at C+0x018) and investment
**205** response `0x80801827` (952 B, field [1], object at C+0x008). Fields that matter, with the
frame arithmetic settled (object-relative + carrier offset; both earlier readings were correct,
two frames):

| field | object offset | 503 C offset | 205 C offset | what |
|---|---|---|---|---|
| [2] deny-list | count `+0x010` / array `+0x014` | `+0x028` / `+0x02C` | `+0x018` / `+0x01C` | 20 x u16 activity indices FORBIDDEN. **Send count 0** |
| [3] flags | count `+0x03C` / array `+0x040` | `+0x054` / `+0x058` | `+0x044` / `+0x048` | 50 x `{u16 index, u2 value}`, stride 4. ⭐ THE DELIVERABLE |
| [4] values | count `+0x108` / array `+0x10C` | `+0x120` / `+0x124` | `+0x110` / `+0x114` | 50 x `{u16 index, u32 value}` |
| [6] gate bit | `+0x2DC` | `+0x2F4` | `+0x2E4` | `vfunc[0x58]`; bit 0 only. **Send 0** |

Element `0x8080185C` = 4 B `{u16 index @+0x000 (kind 0x104, 16 bits); u2 value @+0x002 (kind
0x103, 2 bits)}`; owned == `2`. Container `0x80804681` = 204 B `{u32 count @+0x000; 200 B array
@+0x004}` ⇒ 50 slots. Read counts/strides from container headers (`anchor+0x20`/`+0x28`), never
from `d1-eboot descriptors`' offset column (it prints element size). ⚠ Capacity is 50, not 31 -
the "u5 count" is a wire width, not a ceiling (counts 30/31/32/40/50 all produced distinct valid
bodies: 102/105/107/127/152 B). ⚠ Encoder rule proven both directions: only `count` serialises - a
body with 50 filled slots and `count = 0` is **byte-identical to empty**.

**The gate bit, `[6]`: leave it 0.** Setting it (a) un-gates the profile's 20-slot sparse array
(`0x28B7B0` case 0 - the last writer over the whole plane) and (b) in `0x7A7420`
(`0x7A74BC call [rax+0x58]` / `0x7A74C1 je` / `0x7A74CD cmp byte [rax+0x6e4c],0` /
`0x7A74D4 jne 0x7A75D5`) makes `vfunc[0x58]() && profile[+0x6E4C] != 0` **skip both rung-33 gates
outright** - a bypass that makes the client stop asking the question and would hide a wrong grant.
This also explains the failed `profile+0x6E4C = 01 01` poke: the byte is never consulted while the
bit is 0. `d1-oracle profile build --all-content`'s help text claims it bypasses both gates - that
claim is contradicted (inert without the bit). The same override is applied inside `0x264620`
before computing A.

**The deny-list, `[2]`, is the one real "may DO" object.** `ctx->vfunc[0x18]` = `0x78970`
(`s_investment_utility_context::vfunc[3]`, `mov rax,[rdi+8]; ret`) returns the 205/503 object;
`0x28A170` reads `[rdi+0x10]` (count) and `0x28A182 cmp word [rdi+rdx*2+0x14], si`. Container
`0x808045F0` (44 B `{u32 count; array}`) → `0x808046A3` (40 B, u16 elements ⇒ 20 x u16). Consulted
at 6 sites in 5 functions (`0x264130`, `0x2676A0`, `0x2676F0` x2, `0x2678D0`, `0x267A20`) - every
activity-permission predicate. Proven with a discriminating negative: empty → allowed;
`count=1, list[0]=393` → 393 DENIED; `list[0]=394` → 393 allowed. Absence is silent and
PERMISSIVE; a populated list denies with no diagnostic (symptom: reason `0x18` on a correct
account). Keep sending it empty.

**Hex hygiene.** ⛔⛔ Four of the seven hex bodies quoted in the audit trail are not valid hex (odd
character count: the 31-flag 503 = 245 chars, 31-flag 205 = 205, union-10 503 = 141) - three
independent lanes each lost a nibble transcribing encoder output into prose. **The contract ships a
regeneration command, never transcribed hex.** Regeneration, with mandatory positive control first:

```sh
# POSITIVE CONTROL - must rebuild the pinned bodies byte-for-byte before any new body is trusted
d1-oracle message --tag 0x80801825 --msgid 503 --request-tag 0x80801726 --obj <968B: +0x008,+0x018>
#  -> 01f7180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020810220000000100
#     == d1_bap::INVESTMENT_503_PINNED_RAW
d1-oracle message --tag 0x80801827 --msgid 205 --request-tag 0x80801874 --obj <952B: +0x008=0x7FFF...>
#  -> 00cd 1bfffffffffffffffc00000000000000020810220000000100   == investment_205_response_body()
```

Assert on every result: even hex length, and `b503[21..] == b205[1..]` (the shared `0x80801941`
tail; 503's copy starts at wire byte 21, 205's at byte 1). Both checks are one line; both would
have caught all four bad bodies. 503 object constants: `+0x008` account SOID
`0xD100000000000001`, `+0x018` family-5 root `0x7FFFFFFFFFFFFFFF`.

**Body history:**

- ⛔ v1 4-flag `{98, 834, 5015, 6018}` - SHIPPED (run `20260809-075545`) and a no-op: wrong
  activity index (§3.2). Dead.
- ⛔ v2 8-flag `{98, 308, 419, 440, 834, 3389, 5015, 6018}` - clears 28/28 offline (0-6 and
  393-413) for the DLC gate but is **missing `{2667, 5987}`**: on the wire it reproduces
  `missing_permissions`. This pair parses and agrees (`len(503) = 65 B`, `len(205) = 45 B`,
  `b503[21..] == b205[1..]` True `[C]`) and exists as `d1_bap::INVESTMENT_503_UNLOCKS_RAW` /
  `INVESTMENT_205_UNLOCKS_BODY` (`d1-server/crates/d1-bap/src/lib.rs:1467/:1479`) - referenced
  only from a `#[test]` (`d1-responder/src/main.rs:3875,3884`) and one `warn!` (`:1942`); no
  production path selects them. Recorded as the falsifier only:

  ```
  D1_503_RAW=180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100
  D1_205_RAW=1bfffffffffffffffc00000000000000020918062f8134f81a3f81b8f8342f8d3df9397f9782f0220000000100
  ```

- ⭐ **CURRENT: ship TEN - `{98, 308, 419, 440, 834, 2667, 3389, 5015, 5987, 6018}`** - both walls
  fall on the wire with no poke. Verified live 2026-08-10 as a plane poke
  (`[0x26A34E8]+0xC+idx = 0x02` for 2667/5987 on top of the eight) plus `D1_ANSWER_FAMILIES=0,2`
  and the rung-34 task-7 suppress byte: the client walked to rung 36 `activity:in_world` on
  `pvp_moonbase`/407. The ten-flag pair must be REGENERATED through `d1-oracle message` (no
  trustworthy transcription exists) and the two `d1-bap` constants replaced in the same change.
- The 31-flag pair (546/546 DLC coverage) is the follow-on body; same regeneration path (B2).

**Env-var traps:** `D1_503_RAW` is nested inside `D1_503_SOID` (`main.rs:2672`) - setting it alone
does nothing and logs nothing. `run-boot.sh` pins `D1_503_RAW` and does not mention `D1_205_RAW`
at all. `D1_UNLOCKS` is the wrong knob (family 4/4; nothing reads it - §4.7); leave it at `key`.
Watch the startup line `205/503 agree on the shared 0x80801941 (44 bytes)`; a disagreement means
205 wins and the run cannot be interpreted.

### 3.7 Wire order - 205 is a poll and the last writer

`observed-live`, `runs/20260809-054417-RUNG33-BYPASS/responder.log`: 503 answered ONCE (line 140);
family-4 baseline 175-177; family-3 196; **205 answered 23 times** (lines 250, 675, 1152, ...,
5885, roughly every poll interval); family 0 at 320; family 2 at 332. `[O]` for the count of 23
(N = 1 run); `[C]` that 205 lands last. Shipping the grant on 503 alone is overwritten within ~2 s
and then 22 more times by a body declaring `count = 0`. **Ship both, byte-identical on the shared
struct** - `check_205_503_agree()` already enforces this at startup (its negative arm is
asserted); keep it, and add the §1.6 B3 content assertion. Same-struct-two-answers is the known
killer here: when 503 and 205 disagreed about field [0], the client died with
`family_record_not_found`.

---

## §4 PER-SURFACE REFERENCE

### 4.1 The 205/503 object `0x80801941` - full field map

| # | off (obj-rel) | shape | status |
|---:|---|---|---|
| 0 | `+0x000` | u64 | account SOID - served (`0xD100000000000001`) |
| 1 | `+0x008` | u64 | family-5 root - served (`0x7FFFFFFFFFFFFFFF`) |
| 2 | `+0x010` | `{count; 20 x u16}` | **the activity deny-list** (§3.6) - reader `0x28A170` via `vfunc[3]` = `0x78970`. Live: count 0, slots `0xFFFF` |
| 3 | `+0x03C` | `{count; 50 x {u16,u2}}` | unlock flags - plane arm 1 |
| 4 | `+0x108` | `{count; 50 x {u16,u32}}` | unlock values - plane arm 1 |
| 5 | `+0x29C` | `{count; 5 x {u32,u32,u16}}` | `[U]` NO READER FOUND - slots pre-filled `0x811C9DC5` = FNV-1a basis = hash("") ⇒ two name hashes + a u16 |
| 6 | `+0x2DC` | u32 | the `vfunc[0x58]` gate bit (reader `0x260C60`: `mov rax,[rdi+8]; mov eax,[rax+0x2dc]; and eax,1`). Live 0 `[O]` |
| 7 | `+0x2E0` | `{count; 50 x u32}` | `[U]` NO READER FOUND - 50 name-hash slots, pre-filled `0x811C9DC5` |

Coverage behind the two NO-READER claims: `0x28B7B0` case 1 reads only `+0x3C/+0x40` and
`+0x108/+0x10C`; `0x260C60` reads only `+0x2DC`; the nine call sites of getter `0x5867D0`
(`0x574220`, `0x574340`, `0x581F00`, `0x58C990`, `0x66B9F0`, `0x66BF10`, `0x9A5F50`, `0x9BF0B0`,
`0x9E96C0`) inspected. ⚠ That coverage list missed `0x28A170`'s route to field [2] via `0x78970` -
the reason field [2]'s earlier NO-READER verdict died (§7). Fields [5]/[7]: settle by setting a
count to 1 with a known hash and replaying `0x260200` under a whole-object watchpoint.

⚠ The container **type** `0x80804681` (the 50-slot flag container) appears in FOUR objects, not
one: `0x80801941`[3] (ours), `0x80800CC5`[5] @+0x4C8 (reachable from `0x80800B8F`[9] - the AH's
`global_activity_state`; our own Activity Host emits it a third time as 50 zero slots at rung
30/31, after every 205 poll and immediately before rung 33), `0x808017AA`[4] @+0x548 and
`0x80801A34`[1] @+0x004 (no containers of their own). "Exactly two messages" is exact for
`0x80801941`, NOT for flag delivery in general. `[U]` whether any of the other three lands after
205 (a fourth silent-overwrite path) - no case of `0x28B7B0` sources from the AH blob copy, but
nobody has asked with a watchpoint on `world+0x50B0+0x528`.

### 4.2 The profile (family 4/1, `0x80801743` → `0x80801714`, 28,792 B, 18 fields)

Full character/investment context: `design/character-and-investment.md`. The grant-relevant map:

| # | off | tag | shape | what |
|---:|---|---|---|---|
| 0 | `+0x0000` | - | u64 | profile SOID (live `0xD100000000000201`) |
| 1 | `+0x0008` | `0x808017FC` | 3 x u8 | content key triple - plane arm 0xB (`0x2661D0` → `vfunc[0x380](p[0x0A], p[0x08])`, each u16 in the result's `+0xB0` list becomes plane value 2). Live `00 00 00`. `[I]` class/race/gender |
| 2 | `+0x000C` | `0x8080181A` | 36 B | appearance / customization |
| 3 | `+0x0030` | `0x80800F9F` | `{u32 count; 340 x 32 B}` | ⭐⭐ THE INVENTORY INDEX (§4.6) |
| 4 | `+0x2AB8` | `0x808019FF` | `{u16,u16; 16 x 12 B}` | `[U]` |
| 5 | `+0x2B80` | `0x808045DA` | 20 x u64 | ⭐ EQUIPMENT (§4.6) |
| 6 | `+0x2C20` | `0x808017B9` | 32 B | plane arm 2 keys. Live all zero |
| 7 | `+0x2C40` | `0x8080193C` | 2,592 B | `[U]` - 20 x `{u64, u16, 17xu16, u4}` inside |
| 8 | `+0x3660` | `0x808047A7` | `{u7 count; 127 x 16 B}` | PROGRESSIONS `{u32,u32,u32,u16 ordinal}`. Live count 0 |
| 9 | `+0x3E58` | `0x80801755` | 56 B scalars | `[U]` |
| 10 | `+0x3E90` | `0x80801701` | 5,168 B | `[U]` |
| 11 | `+0x52C0` | `0x80804C21` | 2,048 x 2-bit + presence | ⭐⭐ DENSE COMPLETION FLAGS, 1,967 usable (§4.3) |
| 12 | `+0x5AC0` | `0x80805BC7` | 1,024 x u32 | DENSE UNLOCK VALUES, 174 usable |
| 13 | `+0x6AC0` | `0x80800FA9` | 64 x u64 | plane arm 9 timestamps vs `0x264E50` (41 rows) |
| 14 | `+0x6CC0` | `0x80801724` | `{u64; {u16,u32}}` | `[U]` - NO READER FOUND |
| 15 | `+0x6CD0` | `0x808018BB` | 404 B, 14 fields | the sparse 20-flag/20-value arrays (`+0x6D54`, `+0x6DA8`) - ⛔ gated on `vfunc[0x58]` |
| 16 | `+0x6E64` | `0x8080472B` | 128 x u32 | `[U]` (contains the `+0x6E4C`-adjacent region) |
| 17 | `+0x7064` | `0x808016F8` | 20 B | `[U]` |

Writers: whole-index co-occurrence over 4,555 descriptors gives `0x80801714` exactly two parents -
the QueueZ 4/1 wrapper `0x80801743` (a root; no descriptor contains it) and investment message
**202** (`fetch_character`) response `0x808018EB`[1] - and 202 has no send wrapper in this build,
so **QueueZ 4/1 is the only effective writer; no last-writer race on the profile.** Same for the
account (`0x808018C7` → wrapper `0x80801803`; message 201 unsendable as a client request). `[C]`

Sparse-array live reads (two snapshots agreeing): profile `+0x6D54`, account `+0x4838`, msg503
`+0x003C` all `count 0`, slots `{0xFFFF, 0}` - **we have granted this account nothing**; the
plane's 58 non-zero bytes (56 x `2`, 2 x `1`) come from content arms PLUS the zero-fill phantom
items (§1.3).

### 4.3 Completion flags - profile field [11] and the dense channel

A completion flag is one 2-bit element in profile field [11] @`+0x52C0` (tag `0x80804C21`).
⚠ Size correction that survives: the element is kind `0x103` - a presence bit PLUS 2 value bits =
**3 bits each**, so the field is **768 B on the wire, always** (not 512), and each element carries
its own presence bit (relevant to anyone hand-packing; the count-vs-slots failure mode does not
exist here, but presence bits do).

The consumer loop (`0x28B7B0` case 7, from `d1-eboot disasm`):

```
0x0028bb41  call qword ptr [rcx+0x2a0]        ; the 1,967-row kind-3 directory
0x0028bb60  mov  bl, [r15+rdx+0x52c0]         ; reads the profile's dense byte
0x0028bb6f  movsx rsi, word ptr [rsi+rdx*8+0x14]  ; directory[i] -> plane index
0x0028bb75  mov  [rdi+rsi], bl                ; plane[index] = the byte
```

then `0x28CACD cmp byte [rdi+rax], 2`. Siblings: `0x0028ba00` (account `+0x51A0`, kind 2),
`0x0028bbc5` (profile dense values). Watchpoint replay: kind arg 7 → 256 events on
`profile+0x52C0`; kind 5 (negative) → none. `[C]`

**The grant, executed by the client's own code (4/4 with a discriminating negative):**

```
profile+0x52C0[ 56] = 2  ->  plane[ 159] = 2    complete[The Dark Beyond]
profile+0x52C0[ 62] = 2  ->  plane[ 170] = 2    complete[A Stranger's Call]
profile+0x52C0[ 70] = 2  ->  plane[ 185] = 2    complete[A Key Awaits]
profile+0x52C0[938] = 2  ->  plane[3850] = 2    complete[The Dreadnaught]
NEGATIVE (array untouched): all four plane bytes 0
```

⚠ N = 4 driven end-to-end; the directory correspondence is proven for all 1,967 (§2.1). Do not
report "537 of 537 granted".

Carrier uniqueness: `0x80804C21` appears in `0x80801714`[11] only (plus a 2,048 B one-composite
wrapper `0x80804E56` that is not a second message) ⇒ **no last-writer hazard for kind 3**; the
hazard is specific to `0x80801941`.

**The director, fully explained by this table** (`replayed-offline`, live plane all-zero at every
gate): Earth and book `473350382` are unconditional (empty expression); every other book's gate
flags all read 0. Client-ordinal homes: Moon complete → `profile+0x52C0[56]` (idx 159); Moon 2nd
gate idx 3556 kind 0 (no dense home); shared gate idx 2548 kind 0; Venus complete → `[62]` (170);
Venus 2nd 3585 kind 0; Mars complete → `[70]` (185); Reef → `[62]`; Dreadnaught → `[936]` (3848);
Crucible → idx 98 kind 1 `account+0x383C[57]`; Strike Playlist idx 2538 kind 0. ⚠ Ordinals
2538/2548/3556/3585, NOT the manifest's 2539/2549/3558/3587 - the §2.2 drift is the single most
likely way to ship a silent no-op off this document. The four kind-0 director gates have no dense
home and no setter path: they must ride a pair list.

Announcement: `0x56D3F0` (UPDATE) diffs the dense arrays on the edge `old != 2 && new == 2`
(`0x0056dea7`/`0x0056deb8` for profile; `0x0056d4f0`/`0x0056d502` for account) and calls
`0x54BD30(ctx, index)` per newly-set flag, then a dirty stamp (`0x556AB0(&DAT_026a3710)+0x40C = 1`).
The CREATE handler `0x56D290`'s 4/1 arm does NOT contain this loop. ⇒ **completion delivered in a
baseline grants silently** (the gate opens - case 7 re-reads every build - but the player is never
told); an announced completion requires an UPDATE with a 0→2 transition. Deliverable: set the
1,967 kind-3 bytes (or the 205 manifest-named subset first - staged, since 1,762 are unnamed and
blanket-2 can gate onto uninstalled content); plus `account+0x383C[57] = 2` for the Crucible.
Whether our own encoder round-trips field [11] is verified (3-offset payload diff); whether the
live director changes is not (never run on hardware).

### 4.4 The account (family 4/0, `0x808018C7`, 29,496 B)

Grant fields: `[9]` progressions `+0x304C` (bare 127, no count word, unused ordinal `0xFFFF`);
`[10]` dense flags `+0x383C` (kind 1, 2,662 used / 3,060 B); `[11]` dense values `+0x4430` (64);
`[12]` sparse flags/values `+0x4838`/`+0x493C` (64 + 64); `[14]` dense flags `+0x51A0` (kind 2,
266 used / 512 B); inventory `+0x4C0`/`+0x4C8` (326 slots). **All impossible today**: `account_rsat`
emits 3 of 25 fields and pushes `false` for 4-24; there is no `account.rs` in `d1-oracle` (§1.6
B9). Plane arm 4 reads `+0x383C` via delegation (`0x28C340`) - a grep of `0x28B7B0` alone misses
it (a missing offset inside one function is not an absence when that function delegates).
⚠ Two carriers exist for the account struct: family 4/0 and BAP message **201**'s response
(`0x80801813`) - 201 has no client send wrapper, appears in no responder.log, and whether its
response would land in the same slot is `[U]`. If ever wired, two carriers with different contents
reproduce the `family_record_not_found` failure shape.

### 4.5 Progressions

**A progression is a grant channel into the unlock plane, and it does not open activities.**
Mechanism, every hop an instruction `[C]`:

```
account +0x304C [127] {u32 current, u32 pA, u32 pB, u16 ordinal}   (bare, no count)
profile +0x3660 u32 count(7 bits) + [127] same element
item    +0x0038 ONE element (family 4/3; walked only if EQUIPPED and *(short*)(item+0x58) != -1)
  ├ 0x275990  walks account then profile, skipping ordinal 0xFFFF, calling 0x275870 per entry
  │ 0x260200  also calls 0x275870(item+0x38, ...) per equipped item
  ├ 0x275870  THE STEP WALK: def = globals->vtbl[0x478](ordinal); while (remaining >= step[s].cost)
  │           {remaining -= cost; s++; if (repeat_last_step && s > steps-1) s = steps-1; level++;}
  │           value = level + remaining/(float)step[s].cost;
  │           level_array[ordinal] = MAX(level_array[ordinal], value)   ; merge by MAX
  │           level array = [ctx+0x38]+8+0x4004, 0x800 B = 512 floats
  ├ 0x276200  read: lrintf(floorf(level_array[ordinal]))
  └ 0x28C9A0  THE GRANT (plane arm 8): for i < min(level, steps):
                flag = *(i16*)(step_array+0x10+8*i+4); if (flag != -1) plane[flag] = 2
              0x0028ca46 movsx rsi, word [rsi+rdx*8+0x14] / 0x0028ca52 mov byte [r12+rsi], 2
```

Step element (8 B): `+0x00 u32 progress_to_advance; +0x04 u16 UNLOCK FLAG INDEX GRANTED (0xFFFF =
nothing); +0x06 u16 0`. Two readers land on the same element from different bases - that agreement
is the control. Definition element (56 B, `0x808016E3`): `+0x00` content hash (8/8 vs manifest),
`+0x04` scope (245/246), `+0x05` `repeatLastStep` (246/246), `+0x08` step count (8/8), `+0x10`
self-relative step-array pointer.

**The table**: 247 rows, 246 named from the 2020 manifest (240/246 step counts agree) -
`capture-rig/schemas/progression-grants.tsv`, extractor `capture-rig/progression-grants.py
--controls` (6/6 PASS, exit 2; reads pkg block 1066 through the client's own Oodle decoder; owed a
Rust port into `d1-pkg`). By scope: 0 = 2 record books (account table); 1 = 37 CHARACTER (level,
factions, weeklies); 2 = 202 ITEM (178 bounties, 42 `weapon_*`, 5 `armor_*`, 3 `ghost_*`, 9
subclass `build_*`); 3-6 = 6 derived. ⚠ **The iterators do NOT filter by scope** - an item-scope
ordinal in the character table still grants its flags (say so if you do it).

Key rows (ordinal · name · steps · repeat · max progress · flags granted):
**117 · `character_level` · 40 · 0 · 346,000 · the 40 flags 2531-2570** (contiguous, permuted, no
sentinel); 118 `character_prestige` 1/1/25,000/0; 115 `character_display_xp` 1/1/100,000/0;
116 `character_gear_tilt` 11/0/1,000/0; 11 `base_item_level` 100/0/90/0; 127 `faction_cryptarch`
3/1/7,000/3 (2571-2573); 128 `faction_eris` 5/1/9,000/5 (2585-2588, 5000); 129
`faction_event_iron_banner` 5/0/8,500/5 (2576-2580); 130 `faction_event_queen` 3/0/7,000/3
(2581-2583); 131 `faction_fotc_vanguard` 3/1/6,000/3 (2594-2596); 132 `faction_pvp` 3/1/6,000/3
(2589-2591); 133/134/135 `faction_pvp_{dead_orbit,future_war_cult,new_monarchy}` 25/1/61,000/6
each (2602-2607, 2608-2613, 2619-2624); 189 `r1_s3_factions_fallen` 3/1/7,000/3 (2629-2631); 190
`r1_s3_factions_queen` 3/1/7,000/3 (2632-2634); 191 `r1_s4_factions_gunsmith` 5/1/9,000/4
(5011-5014); 119 `Classified` 10/0/9,000/10 (5573-5582); 121 `d15.fall.record_books.rise_of_iron`
24/0/23,000/0; 106-114 the 9 subclass `build_*` rows 28/0/519k-535k/1-2 each.
`repeatLastStep = 1` ⇒ the level is uncapped (step index clamps, level keeps counting) - huge
progress = high faction rank.

**The XP curve, closed**: `character_level` costs `[0, 900, 4100, 5000, 6000, 7000, 8000, 9000x30,
10000, 15000, 20000]` = 346,000. Simulating `0x275870`: progress 0 → level 1.0 (step[0] costs 0);
345,999 → 39.99995 (floor 39); **346,000 → 40.0**; 400,000 → 40.0 clamped. Step i's cost is what
you pay to LEAVE step i. `[I]`-strong (faithful simulation, not an execution).

⭐⭐ **The negative that matters `[C]`**: union of flags granted by all 247 progressions x all
steps = 144 distinct indices in [314, 5655]. Flags read by the measured activity gates =
`{98, 306, 308, 419, 440, 830, 833, 834, 2667, 3389, 3872, 3876, 3877, 5015, 6018}`.
**Intersection = empty set.** Level 40 with every faction maxed opens nothing; flag 98 is not
earned by playing. ⛔ Do not spend a run on "maybe it wants a level". ⚠ Bound: the 15 read flags
come from 16 sampled activities, not all 546; the grant side (all 247 x all steps) is complete.

**Delivery traps (all silent)**: ship in the baseline (the plane cache is not cleared by a
family-4 object update - §2.7); ⭐ changing the progression COUNT suppresses every progression
announcement in that update (`0x56D3F0` 4/1 arm gates the walk on `old[+0x6CC] == new[+0x6CC]`,
i.e. `+0x3660` as a long index) - send the final count with zero values first, then raise;
announcement is slot-aligned and increase-only (`old.value < new.value`, key != `0xFFFF`); account
and character merge by MAX (you cannot lower a value from the other table); families 1/0 and 1/1
carry progression arrays (`0x8080191C`[2] @+0x10 bare 127; `0x808017A5`[3] @+0x30 counted 127) but
NO instruction was found that reads them - `[U]` with a mechanism, do not ship expecting effect.
0 of 2,125 catalogued diagnostic strings mention progressions; the orbit HUD level/light plate is
family-2 roster state and does NOT move when progressions are fixed. Falsifier that works:
`d1-replay read <snap> '[v:0x26a34b0+0x38]+0x400c'` → `level_array[117]` (measured live today:
0.0, while the family-3 card asserts level 40 - display state the server asserts, not derived).

**What to send** (all `synthetic-minimal`, never tried live): profile `+0x3660` count = N with
`{346000, 0, 0, 117}` plus the faction rows; account `+0x304C` for the 2 scope-0 rows (unused
slots ordinal `0xFFFF`); items `+0x38` per equipped item. `d1-oracle profile build --progression
117:346000` exists (⚠ its help text still calls ordinal 117 `[U]` - stale, closed 2026-08-08).
Never hand-pack (a short body decodes fine and reads zeros past the end).

⛔ **Grimoire does not exist in this client**: byte-scan of the 01.29 eboot - `grimoire`/`Grimoire`
0/0 hits against six positive controls (`progression` 24, `faction` 17, `talent_grid` 22,
`record_book` 14, `unlock_flag` 10, `investment` 676). A bungie.net / companion concept; the
server owes the client nothing for it. `[C]`

### 4.6 Inventory, buckets, equipment, currency (character-state)

Full inventory/item context: `design/character-and-investment.md`. Grant-relevant mechanics:

- **`profile+0x30` is the INDEX; family 4/3 is the DETAIL**, joined on the u64 instance key at
  `elem+0x08` (`c_investment_client_item_repository::vfunc[0]` = `0x573FD0` →
  `0x56CA70(mgr, family, type=3, key)`). Items shipped to 4/3 without a profile slot are
  unreachable by every reader traced. Element `0x808016E0` (32 B): `+0x00` u16 item definition
  index (`0xFFFF` = EMPTY - the sentinel; readers `0x28C6D0`, `0x26D3F0`, `0x26DA00`); `+0x08` u64
  instance key; `+0x10` u32 STACK QUANTITY (`0x26D3F0` sums it per bucket over the same definition
  index; `0x26D070` clamps against it); `+0x14`/`+0x18` `[U]` no reader found.
- Three arrays share the element type: profile 340 (`0x80800FA1` @+0x38), account 326
  (`0x80800FE8` @`account+0x4C8`; `0x26D3F0`'s literal bound `0x146` = 326 confirms), family 1/1's
  20 (`0x808046FE` @`+0x828+8`, never touched).
- **The bucket table** (`investment_globals->vfunc[0x98]()`, count 39 at `+0x84`, rows `+0x88`
  stride `0x14`: `{i32 bucket_ord; i32 first_slot; i32 n_slots; i16 flag_index; u8 plane_value;
  u8 src}`): the 39 rows partition profile slots [0,322) and account slots [0,318) with zero gaps
  and zero overlaps. **An item's bucket is its SLOT RANGE, not a field** - placing an item means
  choosing the slot. Bucket rows that are themselves a grant ("any non-empty slot in this range
  sets this flag"): ord 16 profile [169,189) → 2831; 20 profile [189,209) → 2831; 21 account
  [0,20) → 2831; 26 profile [262,278) → 2843; 28 profile [230,240) → 3282 (plane value 0); 32
  profile [278,279) → 5150; 33 profile [279,280) → 5148; 37 profile [290,322) → 5149; 38 account
  [311,312) → 6029; 39 account [312,318) → 6030.
- ⛔ The zero-fill sentinel bug (§1.3) makes every profile bucket read occupied while the item
  loop (which does consult the count) yields nothing.
- **Equipment** = `profile+0x2B80[slot] = instance key`, where `slot` is the item definition's own
  content-supplied ordinal (`*(i8)(itemdef + 0x30 + *(i64)(itemdef+0x18))`; emblem = 12). Readers
  `0x269510` (with `0x26DA00` as the reverse index over 340 slots) and `0x2694D0` (mismatched key
  ⇒ slot forced `0xFF` = unequipped). Live: all 20 zero.
- **Currency has no dedicated object**: stack quantity summed per bucket, plus unlock values with
  combine rule 3 = SUM (§2.8). `[I]` glimmer/marks/materials are one or both; no reader names them.
- **Vendor stock is content, not a server object** (pull-model CUI; purchase path is investment
  901/903/904/905; "buy now" `0x54F150` consults the offer-key store). No descriptor carries
  vendor stock. The whole CUI/investment layer emits zero diagnostics (0 of 2,125 strings match
  `vendor|currency|glimmer|material|inventory`).
- Content indices the server references by ordinal (all `investment_globals` vtable slots):
  `0x098` buckets (39) · `0x0A8` timestamps (41) · `0x290` kind-1 rows (2,662) · `0x298` kind-2
  rows (266) · `0x2A0` kind-3 rows (1,967) · `0x2B0` kind-0xC static (0 on this snapshot) ·
  `0x2B8` THE flag table (6,944) · `0x2C8` dense values (174) · `0x328` plane-kind-2 lookup ·
  `0x410` item definition by u16 index · `0x538` value combine rule. Plus two item-definition
  fields: the equipment-slot ordinal and the item's value-contribution list (`itemdef+0x68`).

### 4.7 Family 4/4 - NOT a grant surface `[C]`

`0x8080102E` (3,984 B): `[0]` u64 key; `[1]` 200 x `{u64 @+0, u6 @+8}` (`0x80801028`); `[2]`
`{u6 count @+0xC88; 32 x {u8, u64, u16, u16}}` @+0xC90. **No consumer**: accessor `0x260C20`
(`mov rax,[rdi+0x20]; ret`) has 0 confirmed references over call/jmp/riprel/rela (70,682
`R_X86_64_RELATIVE` addends scanned) with `0x260C10` (family 4/2: 2 calls + 11 vtable slots) as
the discriminating positive in the same aligned run; the only three functions touching the
registry key `DAT_02639EA8` (= `0x7FFFFFFFFFFFFFFE` live, one below the family-5 root - a
client-side global, not choosable) merely store the slot at `ctx+0x20`; the plane builders pass
`vfunc[0x20]` (= family 4/2) to `0x28B7B0` and pass 4/4 to nothing; live `[ctx+0x20]` = 0;
`0x8080102E` has 0 parents across 4,555 descriptors. Both arrays are u64-SOID-keyed - the shape of
a replication-state table, not a grant (`[I]`). The unlock evaluator's key is a u16 ≤ 0x1FFF; a
u64 cannot be that key; the pair element `0x8080185C` appears in exactly three containers and none
is `0x8080102x`. **Do not build it; `D1_UNLOCKS` (which carries it) is a dead knob.** Absence is
silent.

### 4.8 Ownership and offer keys - a clean negative

Full signon context: `design/signon-and-entitlement.md`. **Ownership grants nothing on the
gameplay path** `[C]`: `is_offer_key_owned` (`0x59F250`) has 5 callers (xref = 5, corpus grep of
47,304 files = the same 5, and NO rela reference ⇒ no vtable consumer hides from the scan) - orbit
trial banner (`0x9E68F2`, key `0xE0000001`), per-item "Buy X now" (`0x54F1CD`), PS-Store machine
(`0x59D298`), upsell predicate (`0x59F8D1` ← `0x9CC530`), marketplace (`0x59FFB1`). Rung 33 never
touches the stores (§3.1), and the entire activity ladder (rungs 28-34 handlers) installs task
tables, not prerequisite masks - ownership is not evaluated there at all (N = 28 of 35 handler
classes read; 7 have no `vfunc[2]` in the corpus).

- The one non-UI consumer: DLC content-container registration `0x1E3A40` (from
  `bootflow:content_check` `vfunc[7]` `0x79A380`) - per manifest `entries[]` row, `Entry+0xC4`
  ("no entitlement required") bypasses, else `0x1E3AD3 call 0x2A8A40` checks the **signon** list
  directly (field 10 is the only input, `ownership_authority` ignored). Our manifest declares zero
  `entries` ⇒ the loop body never runs. Absence silent; only a UD2 probe at `0x1E3AC2`/`0x1E3AD3`
  would see a populated `entries[]` matter.
- Boot prerequisite `0x10` (`0x59F2A0`, "owned content resolves locally", evaluated by states with
  mask N > 16 incl. `setup:orbit`) cannot fail on `0xE...`-class keys: `0x2B0940(k,4)` tests
  `(k & 0xF0000000) == 0xE0000000` and short-circuits the content lookup; the local store only
  ever holds `0xE` keys (`0xBC0A0` discards others). A LOUD tripwire only for a server that puts a
  non-`0xE` value in field 10.
- Stores: local `0x26A4970[64]` + count `0x26A4A70` (writer `0x59E3D0` from `sceAppContent*`);
  signon `0x263D46C[64]` + count `0x263D56C` (writer `0x2A7090`); selector `ownership_authority`
  `0x26A4A74` (writer `0x59F170` only). Live: authority 2, local = `{E0000002, E0000001}`, signon
  count 0 with a `0xFF` fill - proof the field-10 path RAN and copied zero keys. Replay: keys
  `0xE0000001/2` → owned, `0xE0000003/4` → not (local store fired as positive control; signon
  store 0 events).
- Wire crossings per boot: `/SignOn` request f15 (client's own set, echo) → response f10
  (overwrites the store - last writer wins over the client's claim) → BAP `0x0015`
  `update_purchased_offers` request (client packs count + 64-entry local array, `0x128` B) →
  BAP `0x0016` response: has a full 64-entry IdList decoder (`0x55BB43` → `0xA2020`, size
  `0x108`) but the handler `0x59F540` never touches the decoded body ⇒ NO READER; sending it is
  harmless and useless. The IdList descriptor `0x15AB3E0` has exactly two carriers (signon f10 aux
  pointer rela `0x18F8DA7`; the `0x0016` decode riprel).
- **Correct field-10 payload today: exactly `[0xE0000001, 0xE0000002]`** (full sku + paid tier) -
  the only two offer-key literals compiled into the binary (grep for `0xe0000003`..`0xe0000008`:
  0 hits in 47,304 files). ⛔ **Do not invent an expansion key** (is Rise of Iron `0xE0000003`? -
  `[U]`): the high nibble is the only validation, so a fabrication is silently ACCEPTED and then
  misses every content lookup forever. The real key is readable from one retail console with the
  DLC installed (`sceAppContentGetEntitlementKey` via `0xBC0A0`) or a retail CDN manifest's
  `Entry+0x00`.
- Three different "upsell" decisions, unrelated: the orbit trial banner
  (plane-selected `is_offer_key_owned(0xE0000001)`, computes 0 = not trial; the on-screen "trial"
  text is `cui::s_activity_requirements_not_met_entry` from `0x9D92A0` - unlock state, not
  ownership); the upsell tile (`0x59F880`: manifest entry with product type 5, returns 0 with zero
  entries without ever calling `0x59F250`); `bootflow:upsell` state 10 (reads nothing about
  ownership; entry route `/SignOn` f8 bit 5 `0x20`, `[R]` not re-verified, 0 runs). The upsell
  strings all spell **offer** (`error.upsell_required`,
  `cui::c_investment_tracked_offer_upsell_event_handler_component`, ...) ⇒ `[I]` upsell reads the
  offer/ownership store, not the unlock plane; a level CAP would be an unlock VALUE (only 238
  defined - cheap to enumerate if that fails).

---

## §5 DELIVERY SEMANTICS - how a grant reaches a consumer, and the ways it silently does not

QueueZ replication mechanics in general belong to `design/queuez-replication.md`; this section
keeps what a grant author must know.

### 5.1 The 19 containers - a CLOSED set

`0x56C550` is the hard-coded registry mapper; its runtime copy is the descriptor table at
`registry + 0x4F800 + family*0x60 + type*0x10`, records `{u32 offset_rel_0x40A0, u32 capacity,
u32 stride, u32 instance_type}` (confirmed by `0x56BF80` reading `+0x4FA30/34/38/3C` for 5/5).
The 19 containers **tile the registry with zero gaps** from `+0x040A0` to `+0x4F800`, which is
exactly where the descriptor table begins - a structural proof of completeness, N = 19 exact.
Per-family type index bound 6 (`0x56B70B cmp r15d,6`); 8 x 6 = 48 slots, 29 empty (mapper default
zeroes and sets `instance_type = 0xFFFFFFFF`). `instance_type` (the wire key) and stride were read
out of the client's own image (descriptor checksum at `anchor - 0x48`); 13/13 agreement with the
live-client-derived tables, and `struct_size == stride` 19/19.

| fam/type | schema tag | instance_type | base | cap | stride | what |
|---|---|---|---|---:|---:|---|
| 0/0 | `0x80801921` | `0x1D7BB8F6` | `+0x040A0` | 20 | `0x50` | peer-family root |
| 0/1 | `0x808017FD` | `0x67D97FC4` | `+0x046E0` | 20 | `0xA48` | peer investment body |
| 1/0 | `0x8080191C` | `0x536956F0` | `+0x11480` | 1 | `0x800` | `[U]` - carries a bare-127 progression array @+0x10 |
| 1/1 | `0x808017A5` | `0x91B5E5A9` | `+0x11C80` | 1 | `0x1578` | `[U]` - richer character body: 127 progressions @+0x30, 20-slot inventory @+0x828+8, 20-slot equipment @+0xAB0 |
| 1/3 | `0x80801732` | `0xEF38431D` | `+0x131F8` | 20 | `0xB8` | inventory item (same schema as 4/3) |
| 2/0 | `0x808018B7` | `0x3E3ACE74` | `+0x14058` | 231 | `0x50` | display name + clan tag (but see §6.2) |
| 2/1 | `0x80801810` | `0x0444D2D9` | `+0x18888` | 231 | `0x30` | player card / emblem (see §6.2) |
| 6/0 | `0x808018DC` | `0x7C570C6C` | `+0x1B3D8` | 1 | `0x10` | fireteam root `[I]` |
| 6/1 | `0x8080188A` | `0xCD35804D` | `+0x1B3E8` | 1 | `0x3E8` | fireteam body `[I]` (only consumers: `cui::c_fireteam_details_component::vfunc[14]` @`0x9E9842`/`0x9E987F`) |
| 7/0 | `0x8080192C` | `0x3AE14227` | `+0x1B7D0` | 1 | `0x10` | `[U]` |
| 7/1 | `0x80801899` | `0xA7952170` | `+0x1B7E0` | 1 | `0x98` | `[U]` |
| 3/0 | `0x80801951` | `0xA2AC2C74` | `+0x1B878` | 1 | `0xA0` | character roster |
| 3/1 | `0x80801954` | `0x3431F1BA` | `+0x1B918` | 10 | `0xA80` | character select-card |
| 4/0 | `0x80801803` | `0x64AE75BF` | `+0x22218` | 1 | `0x7338` | account |
| 4/1 | `0x80801743` | `0x8127F15A` | `+0x29550` | 1 | `0x7078` | character profile |
| 4/3 | `0x80801732` | `0xEF38431D` | `+0x305C8` | 666 | `0xB8` | inventory item |
| 4/2 | `0x80800FB2` | `0x0ECCD1F8` | `+0x4E478` | 1 | `0x48` | director dependency |
| 4/4 | `0x8080102E` | `0xC4060975` | `+0x4E4C0` | 1 | `0xF90` | replication-state table, not unlocks (§4.7) |
| 5/5 | `0x80801941` | `0xB8C0245A` | `+0x4F450` | 1 | `0x3B0` | world-server root (rides BAP 205/503) |

Families 6 and 7 exist; every earlier doc said "families 0-5".

Second wire paths (whole-descriptor scan + all 101 investment-message subtrees): only two of the
19 have one - `0x80801941` (503 + 205, §3.6/§3.7) and `0x80801732` (QueueZ 1/3 and 4/3, plus
message **203**'s response `0x808016D9`[1], plus `0x808019A8`[0] inside `0x80802FC9`, not an
investment-message root, `[U]`). Message 203 has never been exercised in any run. Messages
201/202/203 (account/profile/item second carriers) have no client send wrapper ⇒ no cross-message
overwrite risk for account, profile, items.

### 5.2 Announcers - and 11 of 19 containers have none

`0x56B5F0` applies an object: RSAT-decode (`0x56B833 call 0x2C9000`), soid check
(`0x56B844`/`0x56B84B` - mismatch prints "server_object id mismatch in queuez_header"), then slot
first-qword zero → CREATE `0x56D290`; else `memcmp` (`0x56B86E call 0x129D0E0`); **equal →
`0x56B87B je 0x56B73F` RETURN, neither announcer runs**; differ → UPDATE `0x56D3F0`.

| announcer | covers (jump tables hand-decoded; type 2 falls to default in both) |
|---|---|
| CREATE `0x56D290` | 3/1, 4/0, 4/1, 4/3, 4/4 |
| UPDATE `0x56D3F0` | 0/1, 2/0, 2/1, 4/0, 4/1, 4/3, 4/4 |
| DELETE `0x56E7B0` | 4/0, 4/1, 4/4 only |
| end-of-family `0x56D1A0` | family 4, family 0 |

Consequences: **3/1 is CREATE-only** (the character card announces once, never again however much
level/Light changes). **0/1, 2/0, 2/1 are UPDATE-only** - a first baseline announces NOTHING, and
`d1_bap::queuez_family_header_for` (`lib.rs:2487`) unconditionally sends `QUEUEZ_FLAG_BASELINE`
with family version 0: **the server has never emitted an UPDATE and structurally cannot** until a
delta emitter exists (§1.6 B8) - every family-0/2 object we ever delivered announced nothing.
**4/3 has no DELETE arm** - item removal is silent. **No announcer at all**: 0/0, 1/0, 1/1, 1/3,
3/0, 4/2, 5/5, 6/0, 6/1, 7/0, 7/1 (11 of 19). ⚠ Honest bound: CUI is a pull model (`vfunc[14]`
per frame), so "no announcement" ≠ "never read" - it means nothing invalidates derived/cached
state when the object changes.

Gates inside the family-4 announcers that drop grants (all `[C]` from `0x56D3F0`):
- 4/0: the whole update body requires the qword at `account+0x68` non-zero in BOTH old and new
  (`0x56D4A4`/`0x56D4BB`) - else progressions, unlock transitions and item announcements all skip,
  silently.
- 4/1: the progression walk requires the count at `+0x6CC` (as long index = byte `+0x3660`)
  unchanged - adding a progression suppresses every progression announcement in that update.
- Unlock transitions announce only on the edge `old != 2 && new == 2` (account `+0x383C`, profile
  `+0x52C0`); a flag already at 2 in a baseline is never announced (the grant still lands - the
  plane re-reads on every build; only the notification is lost).
- Progressions: increase-only, slot-aligned, key != `0xFFFF`.
- End-of-family `0x56D1A0` (family 4) also runs `0x58F7F0`, `0x5857D0`, `0x574440(4)`, `0x57AD50`,
  `0x583EE0(...,2)`, `0x3C8F30`; its caller `0x56D0F0` then diffs a 1024-bit mask against
  `0x56EC10(4,...)` and calls `0xABFD20(i)` per newly-set bit - a second, batch-level unlock-delta
  announcer, reached only at end-of-family.

### 5.3 A baseline is a SET; the acceptance state machine

`0x56AF60` (`queuez_family_update_begin`), family state at `slot+0x88`:

| state | baseline bit | outcome | loudness |
|---|---|---|---|
| 3 | set | `0x56B023 call 0x56B2D0` (PURGE) then accept, `[+0x9C] = version`, **no version check** | silent |
| 3 | clear | version gate `0x56B11B/0x56B123/0x56B128`; mismatch rejects | LOUD: "queuez version is out of order ..." |
| 2 | set | `0x56B046 mov byte [r12+r14+0x88], 3` - the only 2→3 promotion | silent |
| 2 | clear | reject | LOUD: "ignoring delta-update due to lack of baseline" |
| 0, 1, 4... | any | reject | LOUD: "ignoring family-update due to unhandled replication state ... replication state=%d" |

⭐ This closes the old `[U]`: **a late push into a timed-out (state 4) family IS refused**
(`0x56AFE3 cmp eax,3` / `0x56B02A cmp eax,2`; every other value falls to the named reject arm).
`[C]` `static-client-inference`. Still `[U]`: whether anything moves a family back OUT of state 4
(today, once families 0 and 2 time out, no delivery rescues them).

`0x56B2D0` (the purge): walks all six types; keep-list empty ⇒ memset + DELETE-announce every live
slot of the family; else keeps only slots whose `(instance_type, soid)` is in the delivered
keep-list (16-byte records). ⇒ ⛔ **Re-sending a family-4 baseline that omits 4/3 WIPES THE
INVENTORY, silently** (4/3 has no DELETE announcer). Same for any omitted object.

The quietest failure: the `(family_type, family_root_soid)` scan over 257 slots
(`param_1 + 0x60 + i*0x40`, bound `0x56AFCC cmp r14d,0x4040`) falls out to the epilogue on a miss -
return 0, no string, no counter, no latch.

### 5.4 The sticky error latch, and the silent-failure index

`0x5747F0(ctx, code)`: `if (*(char*)(ctx+1) == 0) *(char*)(ctx+1) = code;` - ONE error per apply
pass, the rest discarded; `ctx = vfunc[1](&DAT_026A35F0)`. Codes: 2 family record not found; 3 no
matching `instance_type`; 4 slot allocation failed; 5 soid mismatch (also prints); 6 version out
of order (also prints). Three of five have NO string. **The observable is a byte in memory, not a
log line.**

| absence / failure | loudness | evidence or observable |
|---|---|---|
| family `(type, root)` not subscribed | SILENT | epilogue jump, nothing |
| unknown `instance_type` | quiet | latch code 3 only |
| family at state 0/1/4 | LOUD | replication-state string |
| delta with no baseline / version skip / soid mismatch | LOUD | named strings |
| byte-identical redelivery | SILENT | `0x56B87B je` - neither announcer; **the recipe is "deliver, then re-send with a byte CHANGED", never "re-send"** |
| (family,type) has no announcer | SILENT | 11 of 19 rows |
| baseline omitted a live object | SILENT for 4/3, 3/1 and all no-announcer rows | purge + (mostly) no DELETE arm |
| 4/0 `+0x68` zero | SILENT | update body skipped |
| 4/1 progression count changed | SILENT | `+0x6CC` gate |
| availability cache never cleared | SILENT | `0x260200` stamps valid=1 on failure too; `0x25FF40` short-circuits forever |
| sparse array `count = 0` with slots filled | SILENT | byte-identical wire body (§3.6) |
| dense block all zeros | SILENT | fixed-size field; zero IS "not owned" |
| null object at plane build | SILENT | boolean set, plane stamped anyway |
| grant reaching only `0x28BFB0` | SILENT | 264 of 266 account-dense flags dropped (§2.5) |

### 5.5 The recipe

1. Answer the subscribe; only a baseline-flagged update promotes state 2 → 3. Verify state=3 by
   memory, or it did not happen.
2. Put the grant in a baseline that names every object that must survive.
3. Order matters twice: 205 lands after 503 and after the family baselines (bodies must agree, or
   205 wins); within family 4, progression count before progression values.
4. Force the re-read: close a family-4 batch (`0x56D1A0`) or re-send 205/503 with the `0x80801941`
   bytes CHANGED (`0x56BF80`'s `memcmp != 0` branch). Byte-identical does nothing, twice over.
5. Verify by memory, not by log: the container at `registry + base + slot*stride`, the sticky byte
   `ctx+1`, and the plane itself.

### 5.6 The silent-grant observables (the per-grant falsifier column)

The single most useful line for any run:

```
d1-replay read runs/snapshots/<snap> '[v:0x26a34b0+0x38]+0xc' --len 0x2000
```

(58 non-zero of 8,192 today; all six permissions-gate targets and all four poison flags read 0.)
Live equivalent: `capture-rig/d1guest.py read '[[0x26A34B0+0x38]]+0xC+<idx>'` → expect `02`.

| grant | the ONLY thing that distinguishes "granted" from "shipped empty" |
|---|---|
| 205/503 sparse flags | plane byte at the index reads `2` (offline: `'[v:0x26a34b0+0x38]+0xc+98'`) |
| 205/503 sparse values | plane `+0x2000 + idx*4` (final plane stride 4, not the scratch stride 8) |
| profile dense completion `+0x52C0` | plane byte reads `2`; or diff the encoded profile - a serialised field changes bytes |
| profile dense values `+0x5AC0` | 0 is a legal value; only a positive control discriminates |
| account dense flags | decode the emitted account (`d1-oracle object --tag 0x808018C7 --decode`; today 28 bytes, 221 bits, 4 fields) |
| account sparse flags | payload LENGTH changes (17,449 → 17,452) - unlike dense blocks |
| profile sparse flags `+0x6D54` | read `[ctx+0x08]+0x2DC` bit 0 FIRST - dead weight until 1 |
| progressions | `'[v:0x26a34b0+0x38]+0x400c'` → `level_array[117]` (live today 0.0 while the family-3 card asserts 40) |
| inventory item join | the item's plane contributions; watchpoint `0x28D210` |
| inventory zero-fill | dump `profile+0x38` stride 32: slots must read `ff ff`, not `00 00` (account `+0x4C8` is the positive control) |
| equipment | 20 x u64 at `+0x2B80` match instance keys already in `+0x38` |
| Light override | presence bool `+0x6CDC`; ⚠ do NOT use the orbit HUD (family-2 roster view, blank regardless) |
| family 2/0, 2/1 | UPDATE-only; 2/1's announce additionally gated on the u16 at `+0x08` or `+0x0A` changing |
| family 3/1 card | CREATE-only, the mirror |
| `/SignOn` f10 | `0x263D56C` (count) and `0x263D46C[0..]`; a `0xFF` fill proves the path ran with zero keys |
| manifest `entries[]` | invisible in every log - needs a UD2 probe at `0x1E3AC2`/`0x1E3AD3` |
| the whole QueueZ apply path | read the sticky byte `ctx+1` in memory, never the log |

⚠ Meta-trap repeated on purpose: `missing_dlc` (0x17) and `missing_permissions` (0x18) are
state-transition REASONS; `grep 'error\.'` has already produced one wrong "clean run" report.

---

## §6 OPEN QUESTIONS AND UNRESOLVED CONTRADICTIONS

Contradictions are stated with both reads and the read that settles them; nobody picks a winner
without the measurement.

1. **Is a repeated family baseline accepted or rejected?** `d1-responder`'s own env help
   (`D1_RESEND_FAMILIES`: "a repeated baseline is REJECTED", quoting the client's "failed to
   process update for [family_type ...]") vs `0x56AF60`'s state-3 arm, which accepts and purges
   with no version check `[C]` static. If a rejection exists it is at `0x565140` or in our own
   send path. Settle: `d1-replay replay 0x56AF60` state=3 baseline bit set, or read the family
   slot's `[+0x9C]` after a second baseline. Gates B8 and any mid-session grant.
2. **What are instance_types `0x0444D2D9` / `0x3E3ACE74`?** `d1-responder/src/main.rs:1588/1594`
   names them `PROGRESSION_*` ("FAMILY 2 - PROGRESSIONS, i.e. LEVEL and LIGHT");
   `d1-oracle/src/roster.rs:210` calls the same pair family 2/0 and 2/1 checksums; the delivery
   census reads 2/0 as display-name + clan tag and 2/1 as the 48-byte player card. Run
   `20260808-193351` evidence shows the client looking 2/1 up by progression hash `0x2975AEF1` =
   `actual_level_light`. Settle with a watchpoint on the 2/1 consumer's key argument. ⚠ Regardless:
   a live mutual-exclusion bug - `main.rs:3436` tries `build_roster_family_body` first, so
   **`D1_ROSTER_DIR` set ⇒ `D1_PROGRESSIONS` is never read**, silently.
3. **`0x80804681` rides in four containers** (§4.1) - are any of the other three an answered
   message landing after 205? If one is, it is a fourth silent-overwrite path. Includes the AH
   `global_activity_state` copy at blob `+0x528` (`[U]`, watchpoint named).
4. **`0x80801941` fields [5] and [7]** - 50 + 5 FNV-prefilled name-hash slots, no reader found.
   Settle: count=1 with a known hash, replay `0x260200` with a whole-object watchpoint.
5. **Names**: 6,637 of 6,944 flag indices (all 266 kind-2 among them, including 834/5015/6018);
   the 1,762 unlabelled kind-3; the 144 progression-granted flags; what the four poison flags
   MEAN. All live in the content's `s_investment_activity_unlock_string_client_definition`,
   reachable through the proven Oodle path (`d1-pkg extract-block` + `content-table.py`) - not the
   eboot, not the manifest.
6. **The requirement expression bytecode** - the 9-variable truth table is consistent with, not
   identical to, the expression. Dump the step array via `0x267954`'s `rsi`.
7. **Real expansion offer keys** (§4.8) - one retail console or one retail CDN manifest.
8. **Whether a populated manifest `entries[]` changes anything** - one run + UD2 probes.
9. **Activities outside 393-413 at the permissions gate**; and the ~400 activity indices whose
   VALUE-requirement status was never sampled.
10. **Do item-scope progressions grant flags?** `0x28C9A0` is driven only by the account and
    character iterators; RTTI names `c_investment_talent_grid_intrinsic_unlock_flag_iterator`
    (`0x2830F0`). Replay `0x28B7B0` case 10 with the plane watched. B6 routes around it.
11. **Families 1 and 7 identities**; family 1's progression arrays have no found reader (§4.5).
12. **Message 201/203 consumption** (second carriers, never exercised).
13. **Where the two 2020-inserted flags sit** in the ordinal space (do not interpolate).
14. **Vendor `unlockFlagHashes[]` combiner** - AND or OR; two vendors would answer it live.
15. **The kind-2 pinned globals** `DAT_0263a100/0263a104` (flags 833/832) - what fills them and
    whether their `0x28BFB0` special treatment matters at runtime.
16. **`vfunc[0x2B0]` kind-0xC table** returns 0 on this snapshot; its 8,192-entry static content
    is unprofiled.
17. **Whether the client's BAP path accepts a ~40 KB family-4 body** (all-composite account) -
    largest observed working is 22,990 B.
18. **Instrument defects to fix before trusting the next bisect**: `d1-replay --watch-read`
    reports values from the snapshot backing store, not the emulator - with `--set-mem`, all 22
    watch events printed `value 0x0` for bytes provably `2` (addresses live, values stale;
    deserves a `#[test]`). And `runs/snapshots/orbit-all.snap` is contaminated by the zero-fill
    profile (340 phantom items of definition index 0 fed through `vfunc[0x410](0)` and
    `0x28D210`'s combine rules) - recapture after B4.
19. **Anything `observed-live` against a real service** - every number here is
    `synthetic_ancestor`. Live services are up (`www4.signon.deadorbit.net`, cert renewed 2025);
    `/SignOn` is HTTPS and BAP is AES-128-GCM, so a passive tap gives ciphertext - the console
    itself must be instrumented.
20. **Whether the ten-flag body has since shipped on the wire** (the sources end 2026-08-10 with
    it verified as a poke and the d1-bap constants still carrying eight flags) - check
    `d1-server/crates/d1-bap/src/lib.rs` and the latest `responder.log` before citing either
    state.

---

## §7 DEAD CLAIMS

Each line: the killed claim, and what killed it. None of these may be re-asserted as live.

1. "op3 = AND, op4 = OR" (and every infix rendering built on it) - killed by the VM `0x28DEA0`
   jump table: op3 = OR, op4 = AND.
2. "op21 is a COERCE / unknown" - killed: NEGATE (`top = -top`).
3. "op8 ≈ `>=`, op12 = NOT op8" as operators - killed: op8 is `==`, op12 is `>` (13 `>=`, 14 `<`,
   15 `<=`); the RoI complement measurement itself stands (`V == -1` vs `V > -1`).
4. The Xur location-release exhaustiveness premise - killed: under the real operators Xur is
   nowhere when no flag is set, correct for a weekend vendor.
5. "manifest flag hashes do not resolve on this client: 0 of 9 present" (published twice, with a
   control) - killed: 10 of 10 present, 100% join of all 6,944 client ids into the manifest's
   7,731; the control tested table parsing, not the search. The surviving rule: never copy the
   manifest `value` column (drift 0..2); resolve hash → ordinal in the client's own table.
6. "`value` on a FLAG step might not be an index" (`[U]`) - closed positive: it IS the runtime
   flag index, at the two populated sites, for that content version.
7. "the family-4/type-4 object is the unlock table" - killed: `{u64, u6}` SOID-keyed, evaluator
   key is u16, accessor `0x260C20` has 0 references, plane builders pass it to nothing.
8. The v1 four-flag body `{98, 834, 5015, 6018}` as the rung-33 fix - killed: the live client
   gates activity index 0; the shipped flags were never read (identical read sets with and
   without).
9. "the repair machine de-instantiates the activity clients, so `blob+0x680` reads 0" - killed:
   at the refusal the activity client was alive; `d1-activity-host` sent 0 unconditionally. The
   9/9 ordering observation stands.
10. The eight-flag body `{98, 308, 419, 440, 834, 3389, 5015, 6018}` and its copy-pasteable
    `D1_503_RAW`/`D1_205_RAW` run line - killed: missing `{2667, 5987}`, reproduces
    `missing_permissions`. Ship ten.
11. "`0x546E70` returns 0 with or without the flags" - retracted: the plane is a cache; flags
    written into the plane make it pass (`{98,834,2667}` suffices for 393); the earlier
    measurement wrote flags into the message object of a snapshot that cannot rebuild the plane.
12. "the permissions residual failure is not a flag and not a value" (the B/C-residual reading) -
    contradicted by the 512-subset sweep (5 passing subsets, all flag-determined) and by the
    2026-08-10 live result: poking the plane for `{2667, 5987}` cleared `missing_permissions`.
13. "`0x546E70`'s A predicate passes" from a 13-step replay - the call passed ctx as A's first
    argument and hit the `test sil,sil` early-out; the corrected call (866 steps) also passes,
    but the first number was not a measurement of A.
14. "the 21 activities read ONE flag (98)" as the requirement set - it is a first-requirement
    measurement; evaluation short-circuits and granting reveals more.
15. The value-plane offsets of the plane-builder arms as the final plane layout - killed: they
    are scratch-buffer offsets (stride 8); `0x28BFB0` compacts to stride 4; writer index ==
    reader index.
16. "the plane's 58 non-zero bytes are entirely content-sourced" - partly killed: the zero-filled
    profile inventory fabricates flags 2831, 2843, 3282, 5148, 5149, 5150 through the
    bucket-occupancy loop.
17. "503-alone is probably a silent no-op" - understated to death: 205 is a 23x-per-run poll and
    always the last writer; 503-alone is overwritten within ~2 s.
18. "the availability cache is cleared by a family-4 object UPDATE" - killed: `xref 0x574440`
    finds exactly two reachers, the family-4 end-of-family handler and the family-5/5 thunk
    `0x56E830`; writing/changing `0x80801941` is the invalidator.
19. "profile field [11] is 512 B and there is nothing to forget to set" - killed: element kind
    `0x103` = presence bit + 2 value bits = 3 bits each ⇒ 768 B, and each element carries its own
    presence bit.
20. "`0x80801941` field [2] has NO READER" - killed: `0x28A170` reads it via
    `vfunc[3]` = `0x78970`; it is the activity deny-list, proven with a discriminating deny test.
21. "the end-to-end message→plane path has N = 0" (and the permissions doc's "load-bearing
    `[U]`") - closed by run `20260809-075545`: the shipped flags read `0x02` in the live plane at
    the addresses `0x28CACD` reads.
22. "`d1-oracle profile build --all-content` bypasses both rung-33 gates" (its help text) -
    contradicted: `+0x6E4C` is never consulted while `0x80801941` field [6] bit 0 is 0.
23. "BAP `0x0016` cannot grant - named to the instruction" - sharpened to death: the response HAS
    a full 64-entry offer-key decoder; what is missing is the reader (`0x59F540` never touches
    the decoded body).
24. "progression step field `+4` is a localisation index" - killed: it is the unlock flag index
    the step grants (`character_level` grants 2531-2570); and the "≥326,000 or ≥346,000" `[U]`
    is closed at 346,000.
25. "grimoire-ish u32" for family-3/1 `C+0x38` - killed: grimoire has no client-side subsystem at
    all (0 string hits against six positive controls); the field is `[U]`.
26. "only 513 of 7,731 flags are referenced" and "2,852 customDyeExpression references" as a
    ranking - killed: 446 after removing 67 hash collisions; the 2,852 are one flag tested 2,852
    times; rank by distinct flags.
27. Four published hex bodies (31-flag 503/205, union-10 503, and one earlier) - killed by parity:
    odd hex length is not hex; a nibble was lost in transcription. The contract ships a
    regeneration command, never transcribed hex.
28. "state 4 (LOST) might accept a late push" (`[U]` held open by three docs) - closed negative:
    `0x56AF60` rejects every state outside {2, 3} with the named diagnostic.
29. "granting everything is at worst harmless" - killed: the whole plane at `2` passes the DLC
    gate and SHUTS the permissions gate with a different reason byte; four poison flags
    `{306, 3872, 3876, 3877}` must stay clear.
30. "the Light base stat is at `+0x6CE8`" - retracted earlier; the base is `+0x6CE0` with the
    presence bool at `+0x6CDC`.

---

## §8 SOURCES

All ten source documents are melted here in full and retired to `historicaldocs/`; banners in
those files overrode their bodies and are honoured above.

| source (now under historicaldocs/design/) | what this document took from it |
|---|---|
| `THE-GRANT-CONTRACT.md` | §1 contract, honest numbers, the 30 fields, build order (its §0-§4); the silent-grant index (§2 → our §5.6); delivery rules (§3); blanks (§5 → our §6); contradiction ledger (§6 - offsets settled by arithmetic, stride settled, invalidator settled, 6.2/6.4/6.7 still open) |
| `unlock-flag-reference.md` | the manifest-side corpus: operator histogram and arity solving, the ten value-populated flags, gates survey, values space, stripped fields, collisions, RTTI names (its §1-§9) - all read through its four-corrections banner |
| `rung33-unlock-gate.md` | the DLC gate to the instruction (§0-§3), the family-4/4 retraction (§4), the VM corrections (§5), the wire bodies and controls (§6), the index-0 falsification (§10), the AH cause chain (§11) - read through its 2026-08-10 SUPERSEDED banner (ship ten, not eight) |
| `grants/character-state-grants.md` | profile 18-field map, inventory element and bucket table, sentinel bug, equipment, dense planes, combine rules, family-4/4 no-consumer proof, the 205/503 unattributed arrays, content-ordinal table (its §1-§8) |
| `grants/completion-flags.md` | the master-table model and setter `0x28CAE0`, the 10/10 retraction and drift staircase, the executed 4-flag grant, the director table, the announcer edge rule (its §1-§8) |
| `grants/delivery-semantics.md` | the 19-container closed set, announcer split and jump tables, baseline-is-a-set, acceptance state machine, sticky latch, availability-cache correction, wire order (its §1-§7) |
| `grants/ownership-offer-keys.md` | the ownership census, `0x59F250`'s five callers, prerequisite 0x10, content registration `0x1E3A40`, field-10 payload, the `0x0016` no-reader sharpening, the three upsell decisions (its §1-§9) |
| `grants/progressions.md` | the 247-row table and step element, the XP curve closure, the empty-intersection negative, item/family-1 carriers, grimoire negative, delivery traps, the extractor (its §1-§8) |
| `grants/rung33-permissions-gate.md` | `0x546E70` to the instruction, the A/B/C split, the 512-subset sweep, poison flags, the deny-list, `vfunc[0x58]` verification, the watch-read defect (its §1-§8) |
| `grants/unlock-flag-index-registry.md` | the index-space census, storage kinds and cross-validation, the 546-activity fixpoint sweep and the 31, the storage split, the `0x28BFB0` two-byte trap, per-channel absence table (its §1-§7) |

Companion artifacts (live): `design/grants/flag-index-registry.tsv` (grow it, don't restart it) ·
`capture-rig/schemas/unlock-flag-index-registry.tsv` · `capture-rig/schemas/unlock-*.tsv` +
`unlock-flag-reference.json` (generator `capture-rig/unlock-flag-reference.py`) ·
`capture-rig/schemas/progression-grants.tsv` (`capture-rig/progression-grants.py`) ·
`d1-server/crates/d1-bap/src/lib.rs` (the 205/503 constants and the agreement guard).

Sibling live docs: `design/character-and-investment.md` (character/profile/item identity) ·
`design/queuez-replication.md` (QueueZ mechanics beyond grants) · `design/activity-launch.md`
(the launch seam, `blob+0x680`/`+0x688`) · `design/signon-and-entitlement.md` (signon protocol,
entitlement plumbing).
