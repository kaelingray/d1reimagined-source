# Items and gear - canonical reference

**State (2026-08-11):** the whole item pipeline is attributed: the family-4/type-3 element field by
field, the 7,544-entry content index, the positional slot->bucket tab, the render path, and the
art-arrangement geometry route. Four independent serving defects were found and each has a named
fix site. Nothing item-shaped has yet been confirmed by a correct item on screen; the current
frontier is **`design/geared-guardian-fixtures.md`** and **`design/geared-visuals-run-1.md`** (live
docs - not duplicated here).

Sources melted into this doc now live under `historicaldocs/design/`; the closing table maps them.
Evidence tags `[C]/[O]/[R]/[I]/[U]` and reach tags are preserved from the sources.

## Contents

1. [Who holds items, who enumerates them](#1-who-holds-items-who-enumerates-them)
2. [The family-4/type-3 element, field by field](#2-the-family-4type-3-element-field-by-field)
3. [The definition lookup and its accessor family](#3-the-definition-lookup-and-its-accessor-family)
4. [The item content index and the emblem slot](#4-the-item-content-index-and-the-emblem-slot)
5. [The tab is positional: slot -> bucket](#5-the-tab-is-positional-slot---bucket)
6. [The render path, from QueueZ object to drawable](#6-the-render-path-from-queuez-object-to-drawable)
7. [Geometry: the art-arrangement index](#7-geometry-the-art-arrangement-index)
8. [The on-screen numbers decoded](#8-the-on-screen-numbers-decoded)
9. [Staging a correct item](#9-staging-a-correct-item)
10. [Light and level](#10-light-and-level)
11. [Default gear, the character push, and the blank-Guardian scope limit](#11-default-gear-the-character-push-and-the-blank-guardian-scope-limit)
12. [The loadout framework and the manifest](#12-the-loadout-framework-and-the-manifest)
13. [Availability builder and item-gate probes](#13-availability-builder-and-item-gate-probes)
14. [Method traps paid for here](#14-method-traps-paid-for-here)
15. [Standing [U]](#15-standing-u)
16. [Dead claims](#16-dead-claims)
17. [Sources](#17-sources)

---

## 1. Who holds items, who enumerates them

`[C]` `static-client-inference` against the pinned 01.29 eboot (sha256 `7271fcb9...`).
(historicaldocs/design/item-render-path.md §1-§2, historicaldocs/design/light-and-gear.md §1.)

**The registry is NOT the inventory.** An item object in family 4 / type 3 is never enumerated. The
client walks slot arrays inside two *other* objects and resolves each slot's SOID into 4/3:

| holder | array | descriptor chain |
|---|---|---|
| profile (family 4/1, tag `0x80801743`) | **340 x 32 B at C+0x30** (container; array base C+0x38) | `0x80801743` -> `0x80801714` [3] @+0x030 -> `0x80800F9F` -> +0x08 -> `0x80800FA1` (10880 B) |
| account (family 4/0, tag `0x80801803`) | **326 x 32 B at C+0x4C0** | `0x80801803` -> `0x808018C7` [6] @+0x4C0 -> `0x80800FE6` -> +0x08 -> `0x80800FE8` (10432 B) |
| profile, third source | 64 x 56 B at C+0x3E90 (`0x80801829`, u16 at +0, `0xFFFF` sentinel) | walked inline in `0x555940`, stride `0x38` |

Slot element (`0x808016E0`, 32 B): `u16 content index @+0x00` / `u64 item-instance SOID @+0x08` /
3 x u32 @+0x10/+0x14/+0x18. Code and reflection agree byte-exactly: the walker reads
`base + 8 + i*32` (the u16) and `base + 0x10 + i*32` (the u64) with `base = profile + 0x30`, i.e.
array base +0x38; loop bounds `cmp ebx, 0x153` = 340 and `cmp ebx, 0x145` = 326.
(The apparent slot-layout disagreement in historicaldocs/design/availability-and-item-gate-probes.md
§1.2 - "u16 at +8, key at +0x10" - is the same layout seen from the +0x30 container base, resolved
by the descriptor walk. `[C]`)

⭐ **340 + 326 = 666 - the QueueZ family-4/type-3 capacity.** Two array sizes read from two objects
sum to the registry capacity measured live. The strongest single confirmation in this layer.

**`0x0000` is not empty; `0xFFFF` is.** ⚑ The empty-slot predicate is three instructions at
`0x1056640`: `cmp word ptr [rsi], 0xFFFF ; setne al ; ret`. A zero-filled profile claims the
Guardian owns 340 copies of item index 0 (index 0 is a real item, hash `0xCBAE0A9F`). Every profile
shipped before 2026-08-08 said exactly that; the AVAIL-ITEM-TRACE run measured the consequence
(§13.3). Prefer *absent* to *zero*: `account_rsat` marks its array not-present and never lied.

**Registry API** (`historicaldocs/design/item-render-path.md` §1): `0x56CA70` =
`server_object* find(registry, int family, int type, u64* key)`; walks
`base = registry + entry.offset + 0x40A0`, `count = u16 @entry+4`, `stride = u32 @entry+8`,
comparing `element[0]` to `*key`. Its diagnostic (fn `0x56BE50`) is
`"server_object instance could not be found for family_type=%s instance_type=%s with soid=0x%016llX"`.
`0x56CAF0` returns the container base. The `type` argument is always a called one-instruction
thunk: `0x1DF0`->0, `0x1048A40`->1, `0x59710`->2, **`0x11E50`->3**, `0x84200`->4, `0x55B1D0`->5.
Scanning all 110 call sites of `0x56CA70` and all 4 of `0x56CAF0` (positive control: the same scan
recovered 4/0, 4/1, 4/2, 4/4, 3/0, 3/1, 0/0, 0/1, 2/0, 2/1, 5/5, 6/*, 7/*): exactly **two** sites
request family 4 / type 3:

| site | in | does |
|---|---|---|
| `0x1087746` | `0x1087660` (the 340-slot walker) | passes the element to `0x555730` |
| `0x0ABF235` | `0xABF190` (peer/presence snapshot) | reads `element+0x38` (u32) into a 340-entry u32 array at +0x4B0, sets bit i of a 340-bit mask at +0x484; gated on `esi == 0`, `index <= 0x153`; reached from `0x56D3F0` (`world_controller:investment:activity`) |

⚠ That two-site census was the basis of the retracted "only three fields are read" claim; the UI
reaches elements by a third route (§2, dead claim D6).

---

## 2. The family-4/type-3 element, field by field

**Schema tag `0x80801732`, 184 B (`0xB8`), 666 registry slots, instance type `0xEF38431D`.**
Built by attributing fields to their consumers, not by guessing layout. `d1-eboot controls` was
27 PASS / exit 0 at the top of that work; function semantics from the decompiled corpus
(`~/dev/ghidra-projects/decompiled`, 47,304 `.c` files); names from
`capture-rig/eboot-atlas.py --addr`; hash->name from the D1 public world manifest (8,609 item rows;
a naming aid only, never client-behaviour evidence).
(historicaldocs/design/inventory-instance-fields.md §1.)

| element off | schema | width | meaning | proof | ev |
|---|---|---|---|---|---|
| `+0x00` | leaf | u64 | **item-instance SOID** - the registry lookup key | `0x56CA70` compares `element[0]` to the key; `*param_3` in `0x572A10` | `[C]` |
| `+0x08` | `0x80801920`[0] | u64 | - | none found | `[U]` |
| `+0x10` | `0x80801920`[1] | 2 bits | - (we wrote `1`) | none found | `[U]` |
| **`+0x18`** | `0x80801711`[0] | i32 | ⭐⭐ **the item's LEVEL.** `FUN_0026F480` returns it (selector call `0x14D0` = `30 c0 c3` = `xor al,al; ret` => constant 0); four callers gate `rec[0x10] <= v <= rec[0x14]` (`0x284D20`, `0x285E00`, `0x2863C0`, `0x287870`); `0x2816F0` adds it into the stat curve. We wrote the item HASH here; as i32 Gjallarhorn's is **-730,737,871** | `0x26F492` (verified `8b 44 83 18` = `mov eax,[rbx+rax*4+0x18]`), `FUN_002816F0` | `[C]` |
| **`+0x1C`** | `0x80801711`[1] | u32 | ⭐⭐ **minimum required stat value** for the equip check, read as `element + 0x1C + i*4` over the equip stat-requirement table (`globals vfunc[53]` = `0x25E410`; live count 1, stat id `0x0075`). We wrote `bucketTypeHash` (~10^9) here => every item un-equippable by construction | `c_investment_character_equipment_const_iterator::ctor` `0x269760`, at `0x269971`: `cmp eax,[rcx+rbx*4+0x1c]` / `jge` / else `or byte [rax],8` (the not-equippable flag) | `[C]` (instruction), `[O]` (count, one snapshot) |
| **`+0x20`** | `0x80801711`[2] | i16 | ⭐ **stat-curve index offset**: `FUN_002816F0` samples the stat-group curve at `(i16)(element+0x20) + level` | `FUN_002816F0`, `FUN_0026F110`; set by the client's own ctor `FUN_0026E0E0` (§9) | `[C]` |
| **`+0x24`** | leaf | u16 | ⭐⭐⭐ **CONTENT INDEX -> the item definition.** Argument of `investment_globals->vtbl[0x410]` | 20 distinct functions (24 textual sites; 2 are an ICF thunk duplicate of `0x260200`), incl. `c_investment_inventory_visible_iterator<0,340>` (`0x28CC70`) and `0x269760` | `[C]` |
| **`+0x26`** | `0x80801018` | 16 B = 8 x u16 | ⭐ **extra content indices** whose world tags this item also wants loaded, plus an art-arrangement override source. ⛔ **NOT `0xFFFF`-terminated**: `0x555730`'s tail is a fixed 8-iteration loop where `0xFFFF` is a per-entry SKIP | `0x555730`: `sVar2 = *(short *)(param_2 + 0x26 + lVar7 * 2); if (sVar2 != -1) recurse(...); while ((char)lVar7 != '\b')` | `[C]` |
| **`+0x38`** | `0x8080176B`[0] | u32 | ⭐ **progression points (XP)** - `FUN_00284630` subtracts per-level costs until underflow | `FUN_00284630`, `iVar5 = *param_1` | `[C]` |
| `+0x3C` | `0x8080176B`[1] | u32 | periodic progress bucket, zeroed at progression reset | `FUN_00265C20` zeroes it, same class as the character/account element | `[C]` |
| `+0x40` | `0x8080176B`[2] | u32 | second periodic bucket, by symmetry | none found | `[I]` |
| **`+0x44`** | `0x8080176B`[3] | u16 | ⭐ **progression ordinal** -> `vtbl[0x478]` (`0x25B200`, stride `0x38`). `0xFFFF` = none - and it makes the progression iterator `0x10559F0` **reject the item entirely** | `FUN_00284630`: `if ((short)param_1[3] == -1) return 0;` then `vtbl[0x478](globals,(short)param_1[3])` | `[C]` |
| `+0x48` | `0x80804629` | 15 B = 5 x 3 B | - | none found | `[U]` |
| **`+0x58`** | `0x80801846`[2] | u16 | ⭐⭐ **talent grid index** -> `vtbl[0x4E0]` (`0x25B960`). `0xFFFF` is the only "none": `FUN_00283A30` returns 0 only for `-1`, so a `0` silently selects grid #0 and leaks its stats | `FUN_00286EA0` (same `param_1` then passed at `+0x24`); `0x283A30`, `0x260200`, `0x28D210`, `0x26F710` | `[C]` |
| **`+0x5A`** | `0x808043C6` | 36 B = 36 x u8 | ⭐ talent-grid **node state** array for the `+0x58` grid | `FUN_002826C0` / `FUN_002828D0` node iterators; `FUN_00286EA0` walks a bitmask beside `abStack_5c[36]` | `[C]` |
| `+0x7E` | `0x8080405E` | 8 B = 8 x u8 | node **step selection**, consumed as `byte/255.0` interpolants | `FUN_00286440`, `FUN_002869B0` | `[C]` |
| `+0x86` | `0x80801846`[5] | u8 | - (we wrote `1`) | none found | `[U]` |
| **`+0x88`** | `0x80805A08` | 16 B = 8 x u16 | ⭐ per-instance **stat indices**, keys paired with `+0x98` | `FUN_0028B6E0` writes `+0x88` and `+0x98` together from an 8-byte `{u16,u32}` source | `[C]` |
| **`+0x98`** | `0x80804EA5` | 32 B = 8 x u32 | ⭐⭐ **per-item stat values**, indexed by position in the definition's stat list at `definition+0x68`; clamped to `definition+0x20` | `0x28CC70` / `0x28CC60` / `0x28CFD0` read/write `+0x98 + i*4` on the element proven by `*(u16*)(uVar9+0x24)` in the same block | `[C]` |

⚠ Every remaining `[U]` row is *unsearched*, not proven unread - the scans that produced "no
consumer found" are blind to indexed array access and to argument-forwarded holders (§14).

### 2.1 The array-count rule these widths rest on

(historicaldocs/design/inventory-instance-fields.md §1.1.) The first proposed rule ("the `kind`
byte's `0x100` bit marks array elements") is **retracted**: `0x80800BD7` (8672 B) has three
`kind 0x1` fields at `@+0x000`, `@+0x21C0`, `@+0x21C8` - plain nested members.

⭐ **The array marker is structural.** A descriptor is an inline array iff it has exactly one field
whose `offset == the element's size` and `container_size % offset == 0`; then
`count = container_size / offset`, element 0 at `+0`. `kind`'s low nibble is the wire-type class
(`0x1` struct, `0x3` bitfield, `0x4` u16, `0x5` u32, `0x6` u64, `0x7` u8); the `0x100` bit stays
`[U]`. Array descriptors anchor on **element[1]**, not element[0]: `0x80800FA1`'s field offset is
`0x20` (= 1 x 32), `0x80805513`'s is `0x44` (= 1 x 68).

Positive controls, each proven by code: `0x80800FA1` (10880 B, one field `@+0x020` ->
`0x808016E0` 32 B => 340, byte-exact with §1); `0x80800F9D` (16 B, one field `@+0x002`, kind `0x4`,
structurally identical to `0x80801018`) walked by `FUN_0054E6C0` with
`do { ... } while ((int)lVar17 != 8);` over `param_7 + i*2`. `[C]` Corroborations: `+0x98` = 32/4 =
8 x u32 and code indexes `+0x98 + i*4`; `+0x5A` = 36 x u8 and `0x286EA0` carries a 36-byte stack
array; `+0x26` = 8 x u16 and `0x555730` loops exactly 8 times.

---

## 3. The definition lookup and its accessor family

(historicaldocs/design/inventory-instance-fields.md §2.)

`c_investment_globals_interface::vfunc[130]` = **`0x25ADE0`** (slot 130 x 8 = `0x410`, RTTI-named).
Body:

```c
piVar4 = this->vfunc[0x168]();                       // the item table
if (index < *piVar4 && entry != 0) {
    uVar1 = *(uint *)(entry + 0x10);                 // the entry's WORLD TAG
    iVar3 = this->vfunc[0x18]();                     // content-availability mode
    if (iVar3 - 3U > 1) {                            // modes 3,4 pass outright
        if (iVar3 != 2) return 0;
        if (FUN_003491A0(FUN_003462B0(), uVar1) != 2) return 0;   // tag must be LOADED
    }
    ...
```

Load-bearing consequences, all `[C]`:

1. **Stride `0x18` = 24 bytes** - the same table `capture-rig/item-content-index.tsv` was generated
   from (§4); table and accessor agree on stride and source.
2. **The lookup itself checks the world tag is loaded** (`FUN_003491A0 == 2`); if not, it returns 0
   - no definition, not a wrong one. Its tail shows why: the definition is **package-resident**,
   addressed by `(tag >> 13) & 0x3FF` (a `0x40`-byte page record in `*DAT_01A0B420`) and
   `tag & 0x1FFF` (the entry), stride at page `+0x30`. The bucket, the stat list at `def+0x68` and
   the geometry flag all live inside the loaded package: "item has no model" and "item's tag is not
   loaded" are one failure with two faces.
3. Bounds check is `index < 7544`, so `+0x24 = 1` is in range and returns a real (wrong) definition
   - content index 1 = **'Bronze Tier'** (`stats:{}`, `equippable:false`, `hasGeometry:false`).
   ⚠ **A wrong content index is silent** - in the strong sense: `diagnostic-catalogue.py --addr`
   puts `0x25ADE0`, `0x26F110`, `0x286560`, `0x28CC70` all outside every catalogued function; the
   whole investment/inventory-iterator layer emits no diagnostics. The only observable is on
   screen.

### 3.1 The sibling accessors

| vtbl | slot | addr | stride | used with | tag-load gate? |
|---|---|---|---|---|---|
| `+0x410` | 130 | `0x25ADE0` | `0x18` | `element+0x24` - item definition | yes |
| `+0x430` | 134 | `0x25AF70` | `0x40` | `0x28CC70` (before the `+0x24` lookup) | no |
| `+0x478` | 143 | `0x25B200` | `0x38` | `element+0x44` - progression definition | no |
| `+0x4E0` | 156 | `0x25B960` | `0x18` | `element+0x58` - talent grid | yes (same body shape as 130) |
| `+0x538` | 167 | `0x25B630` | `0x08` | a stat id in `0x28CC70` | no |

All five are bounds-checked `table[index]` accessors taking a `short`. ⛔ **All five treat `0` as a
valid index; the empty marker is `0xFFFF`** (`status/LESSONS.md` #16) - and the old fixtures wrote 0.

---

## 4. The item content index and the emblem slot

(historicaldocs/design/item-content-indices.md.) **Reach:** `static-content-inference` (own package
dumps decoded by the client's own Oodle decoder under Unicorn) plus `[O]` offline replay of the
client's own accessors on `runs/snapshots/orbit-all.snap`. That snapshot came from a client our
synthetic server drove, so anything about *our objects* stays `synthetic_ancestor`; the table
itself is retail content read by retail code. Nothing here is `observed-live` or
`observed-capture`.

### 4.1 Where the number lives

```text
ps4_investment_globals_012f_3.pkg   entry 8126   tag 0x80A5FFBE   class 0x80801933
    block 1085, offset 121536, size 181104 = 0x30 header + 7544 x 24

array header (same 0x30 bytes as every investment-globals array)
    +0x00 u64 181104   +0x08 u64 7544   +0x10 u64 16
    +0x18 {u32 0, u32 0x80800184}       +0x20 u64 7544
    +0x28 {u32 0x808016BD, u32 0}       <- the ELEMENT class
    +0x30 7544 x 24 B

element (0x808016BD, 24 B)
    +0x00  u32  item content hash     <- the public-manifest DestinyInventoryItemDefinition hash
    +0x04  u32  0
    +0x08  u64  0                     <- zero on all 7,544 rows  [U] what it is for
    +0x10  u32  world tag             <- the item's own content entry
    +0x14  u32  0
```

7,544 elements, 7,544 distinct hashes, zero duplicates - an index is never ambiguous. The element
index **is** the content index by the client's own argument: `0x25ADE0` is
`investment_globals->vtbl[0x410](u16)`, and its body computes `idx*24` and reads
`element+0x10` (the world tag):

```asm
0x25adf2  call qword [rax+0x168]        ; resolve this table's content tag (= 0x25DAC0)
0x25adf8  movsxd rdx, [rax]             ; the count bound
0x25adfb  movsx  rcx, bx                ; (i16) idx
0x25ae05  jbe 0x25ae95                  ; out of range -> return 0
0x25ae0b  mov  rdx, [rax+8]
0x25ae0f  lea  rax, [rdx+rax+0x18]      ; the array base
0x25ae14  lea  rcx, [rcx+rcx*2]         ; idx*3
0x25ae18  lea  rdx, [rcx*8]             ; idx*24 - the 24-byte stride
0x25ae25  mov  ebx, [rax+rcx*8+0x10]    ; element+0x10 = the item's world tag
```

`0x54C090` (the other index->tag accessor, used by the 340-slot walker's `0x555730`) reads a
sibling **16-byte** table with the same 7,544 count: two independent tables, one index space. Its
body: `if (i32)D[0] <= idx: return 0xFFFFFFFF; base = D + (i64)D[8] + 0x18; return u32 at
base[idx*16 + 8]`, where `D` comes from the global content handle at `[0x26A0910]`
(`{u32 content_tag @+0; u8 loaded @+4}`, allocated by `0x5477F0`, initialised to tag `0xFFFFFFFF`;
`0x548490()` = "is set", `0x548440()` = the handle, `0x5484F0(h)` = the resourcerer resolve).

### 4.2 Where the index is consumed - five fields, one number

| object | field | reader |
|---|---|---|
| family 4/1 profile | `+0x38 + 32*i`, u16 @+0 (the 340-slot array) | `0x1087660` -> `0x555730` -> `0x54C090` - **the array that makes gear load** |
| family 4/3 element | `+0x24` u16 | `0x265EF0`: `movzx esi, word [rbx+0x24]` -> `0x25ADE0` |
| family 3/1 card | `C+0x250 + 64*slot`, u16 @+8 | `0xA11627` -> the emblem colour resolver `0xA81B00` |
| family 2/1 roster card | `+0x20` u16 (emblem), `+0x0A` u16 | `0xA2DF72` -> `0xA2DBC0`; `0x9E9B41` -> `0x548860` |

### 4.3 The measured indices - 13 staged items and a real emblem

Full 7,544-row mapping: **`capture-rig/item-content-index.tsv`**, generated by
**`d1-oracle item-index write`** since 2026-08-11 (previously
`capture-rig/oodle/item-index.py --write`, whose output the Rust reproduces **byte for byte** apart
from the `# GENERATED by` line - `d1-oracle item-index diff --against …`, exit 0), provenance header
`sha256(entry payload) = 71bfdc0a9236ae22ea5db55a2915e8578a636fdaa48ded1c317838e80c7d2c96`.

| # | slot file | item | hash | content index | world tag |
|--:|---|---|---:|---:|---|
| 0 | `00-Subclass` | Striker | `0x925CDEEA` | **710** | `0x80A62B95` |
| 1 | `01-Primary Weapons` | Eyasluna | `0x43850CE1` | **5040** | `0x80A63C9B` |
| 2 | `02-Special Weapons` | 1000-Yard Stare | `0x658D9391` | **5260** | `0x80A63D77` |
| 3 | `03-Heavy Weapons` | Gjallarhorn | `0xD471D331` | **6299** | `0x80A64196` |
| 4 | `04-Ghost` | Competitive Shell | `0xC98412E4` | **4528** | `0x80A63A9A` |
| 5 | `05-Helmet` | Oryx Mask | `0x74CB6B9B` | **6488** | `0x80A64227` |
| 6 | `06-Gauntlets` | Scarab's Vigil Gauntlets | `0x0B597477` | **5918** | `0x80A64019` |
| 7 | `07-Chest Armor` | Twilight Garrison | `0xE9BECC83` | **3886** | `0x80A6380E` |
| 8 | `08-Leg Armor` | Scarab's Vigil Greaves | `0x77258119` | **5922** | `0x80A6401D` |
| 9 | `09-Class Armor` | Silimar's Iron Sash | `0xD6BD15DA` | **5855** | `0x80A63FDA` |
| 10 | `10-Artifacts` | Wings of Apris-Fol | `0xD9D4D849` | **4286** | `0x80A6399E` |
| 11 | `11-Vehicle` | Arecibo Answer | `0xDFD44A7A` | **6362** | `0x80A641D5` |
| 12 | ⭐ reserved - the emblem slot | The Reflective Proof | `0x5508AC2A` | **1512** | `0x80A62EB7` |
| 13 | `12-Sparrow Horn` | Duck Horn | `0x7D79A673` | **5348** | `0x80A65118` |

Gjallarhorn Year-1 `0x4BF4BE3F` => index **2085** (the other Gjallarhorn, `0xD471D331`, is 6299).

### 4.4 The emblem slot is 12 `[O]`

```asm
0xa11602  call 0x25a320 ; call qword [rcx+0x160]  ; investment globals' well-known-slots block
0xa11618  movsx rax, byte [rax+0x18]              ; THE EMBLEM SLOT (signed; -1 = none)
0xa11623  shl   rax, 6                            ; x 64 = the card's equipment stride
0xa11627  mov   ax, [r13+rax+0x258]               ; = card + 0x250 + 64*slot + 8
0xa11642  cmp   di, 0xffff ; je                   ; 0xFFFF = no emblem
0xa1166a  call  0xa81b00                          ; the emblem colour resolver
```

Replayed offline: `0x25DA40(globals)` -> `0x380A2EDA0`, byte at `+0x18` = `0x0C` = **12**. Nine
independent call sites read the same byte, all feeding emblem paths: `0x266356`, `0x9A44CB`,
`0x9A45CE`, `0x9BD9A9`, `0xA1160D`, `0xA2DB5F`, `0xA56713`, `0xA59E45`, `0xA71DF7` (found by
scanning `.text` for `ff 91 60 01 00 00` / `ff 90 60 01 00 00` - 53 sites total; the population
also reads `+0x11` and `+0x41`). ⚠ **Only slot 12 is measured; every other slot's meaning is
`[U]`.** The well-known-slots block at `vtbl[0x160]()+0x0A..+0x19` is
`00 01 02 03 04 05 10 06 07 08 09 0b 0a 0f 0c 0e` - a permutation of `{0..16}\{13}`, plausibly a
display order. Other fixed offsets read at `+0x11` (`0x278B1A`) and `+0x41` (`0xA24A52`,
`0xA831F9`).

**The in-world emblem** comes out of the profile's 340-slot array via the equipped list. The
in-world card filler `0x266230` reads `vtbl[0x160]()[0x18]` (the slot), then
`0x26D960(profile, slot)`:

```asm
0x26d964  rcx = [rdi + 0x2b80 + 8*slot]         ; the 20 x u64 EQUIPPED list, by slot
0x26d96f  if (rcx == 0) return -1
0x26d980  for (i = 0; i <= 0x153; i++)          ; 340
0x26d989      if ([profile + 0x40 + 32*i] == rcx) return i
```

then `ax = [rbx+rax*32+0x38]` - the content index. => the `+0x2B80` equipped list is indexed by the
same equipment-slot space as the card's `C+0x250`, and slot 12 of it is the emblem. The builders
leave `+0x2B80[12]` **zero** (an item parked there would render as the player's emblem). ⛔ The
in-world emblem is therefore still undelivered: it needs an emblem family-4/3 element whose SOID
sits in `+0x2B80[12]` and in a 340-slot with index 1512. The character-SELECT card's emblem needs
no SOID (`0xA11627` reads only the u16) and is shipped.

### 4.5 Instruments, controls, fixtures

`capture-rig/oodle/item-index.py --controls` -> **7 PASS, exit 0** (exit 2 = nothing citable).
The seven controls and their axes (historicaldocs/design/item-content-indices.md §3): **pipeline**
(re-derives the activity table's index 1 = `0xDF283AFB`, block 1063, sha1 `b7af28cbb641...`);
**table shape** (`0x30 + 7544x24`, count stated twice, element class `0x808016BD`, 7,544 distinct
hashes); **content == memory** (`0x25ADE0(globals, 0)` computes base `0x380963B20`; the 181,056
bytes there are byte-identical to the package entry); **execution** (the client's own
`rax + rcx*8` lands on `0x380967DB0` / `0x3809889A8` / `0x38096C8E0` for Striker / Gjallarhorn /
the emblem, each `u32` the right hash); **bound-NEGATIVE** (idx 7543 reached, **7544 refused**);
**world-tag-anchor** (`0x54C090(handle,0) = 0x80A60297`, `(197) = 0x80A6035A`, both UD2-probed
live in `runs/20260808-110410-AVAIL-ITEM-TRACE`; discriminator `(195) = 0x80A60358`, so the
retracted contiguous route fails inside the anchoring measurement);
**hash-space-is-items-NEGATIVE** (`character_level` `0x6650C0F9`, stat Light `0x8E8B4E10`, bucket
Titan Subclass `0xC3C96257` all ABSENT; 4,631 of the manifest's 5,293 item rows resolve). The
controls do **not** cover rendering: index -> element is proven, element `+0x10` -> pixels is one
resourcerer load away and needs a run.

`d1-server/crates/d1-oracle/src/item_index.rs` consumes the TSV without re-measuring: it validates
the provenance header and `require()` **refuses a hash 01.29 content does not know**. Oracle
controls: `d1-oracle character controls` 15 PASS (adds `emblem-index-is-measured` - slot 12, 1512,
card `+0x558`; `not-an-item-is-REFUSED`), `d1-oracle profile controls` 13 PASS (adds
`item-slots-populated` - slot 3 stays `0xFFFF`; `item-slot-padding-NEGATIVE` - a byte at element
`+0x02` must be refused at `+0x03A`, because `0x808016E0` leaves `+0x02..+0x07` uncovered).
`cargo test -p d1-oracle` 34 + 1 + 14 + 1 PASS (2026-08-11).

⭐ **2026-08-11: the module is now BOTH halves.** `item_index::SourceTable` reads the table out of
the packages through `d1_oracle::oodle` (the client's own Oodle decoder, in Rust) and renders the
same TSV; `d1-oracle item-index {controls,write,diff}` is the CLI. ⛔ The note that used to sit here
- *"`item-index.py` is Python and owed a Rust port; the Oodle decode runs the client's own decoder
under Unicorn, so the port is not a transliteration"* - is **retracted**. There was never a codec
to port: `oodle-decompress.py` is a 35-line wrapper around three eboot entry points and
`d1-oracle::harness` already called that way. The Python's own 7 controls are preserved as 4 Rust
controls here plus 4 in `oodle` (one INVERTED), and the execution-side controls (`content ==
memory`, `bound-NEGATIVE`, `world-tag-anchor`) remain `d1-replay`'s, still in the Python. **That is
the one gap:** the Rust generator proves the table's *shape and bytes*, not that the client's own
accessor lands on the same base - see §4.5a.

**2026-08-09 fixtures** (superseded by the geared roster in `design/geared-guardian-fixtures.md`;
kept for provenance): `capture-rig/characters/char-lvl40-geared.bin` - 2,326 B / 18,607 bits,
sha256 `1b18f6d2dd1565d3...`, 53 non-zero qwords: SOID `0xd100000000000101`, identity `{0,0,1}` at
`+0x08`, LEVEL 40 at `+0x68`, derived 40.0f at `+0x70`, 14 equipment records at `+0x250` stride
`0x40` (`u64 SOID @+0`, `u16 index @+8`; SOIDs `0xd100000000000301..030d`), emblem index `0x05E8` =
1512 at `+0x558` with SOID 0, six unused slots `+0x5D8..+0x718` = `0xFFFF`, stat[0]
`{index 0 (Light), 0x190 = 400}` at `+0x870`, 15 stat keys `0xFF` = absent at `+0x878..+0x8E8`.
`capture-rig/profiles/profile-lvl40.bin` - 20,332 B / 162,655 bits, sha256 `b0fd742490b6635d...`,
373 non-zero qwords: SOID `0xd100000000000201`, `+0x28` last_orbited_destination `0x932015BB`
(Old Russia), the 13 items at 340-slots 0-12 in file order (the pre-positional-fix placement,
§5), remaining 327 slots `0xFFFF`, equipped list `+0x2B80 + 8*slot` (12 SOIDs in slots 0..11,
`+0x2BE0` slot 12 = 0 reserved for the emblem, `+0x2BE8` slot 13 = Duck Horn), progression count 1
/ progress 400,000 at `+0x3660` with `progression[0].index = 0x75 = 117` (`character_level`) at
`+0x3670`, stat override present at `+0x6CDC` and `override[0] = {index 0 (Light), 400}` at
`+0x6CE0`. Round trips `object IDENTICAL` for both.

⚠ The 13 SOIDs only resolve if the inventory is served: `D1_INVENTORY=capture-rig/inventory`.
`run-boot.sh` does not set it. Without it `0x1087660` skips the registry find
(`cmp qword [r12+rbx+0x10], 0; je`) but **still calls the world-tag gate with the u16** - tags load,
the item just has no instance. Bisect axes: `--no-content-indices` (both builders) reproduces the
pre-2026-08-09 all-`0xFFFF` shape; `--phantom-item-slots` reproduces the pre-2026-08-08 all-zero
shape. Do not flip two axes in one run.

⚠ **Version drift:** assume the ordinal moves across content versions - the sibling
progression-table extraction gives 109 / 116 / 117 for `character_level` across 2014 / 2015 / 2016
content. This table was read from `012f_3` and nothing else.

### 4.5a ⚠ What the Rust generator does NOT carry across - the named gap

`d1-oracle item-index controls` and `d1-oracle oodle controls` cover the **content** side: the
decode reproduces the Python's pinned block (SHA-1 `b7af28cb…` / SHA-256 `cba12f0a…`, 262,144 B,
16 chunks, `activity[1] = 0xDF283AFB`), the array header describes the payload it precedes, the
entry's self-length matches the container, the pins hold, and the two NEGATIVE hash-space checks
hold. One control is **INVERTED** (flip a stored byte, the output must change) and one is a
NEGATIVE on the harness itself (unregister `DECODE_SOME_MEMSET_RETURN` and the decode must be
refused).

**Four** of `item-index.py`'s seven controls are **not** ported, because they are `d1-replay`'s, not
the decoder's (an earlier revision of this section said three and omitted the second row):

| control | what it proves | still lives in |
|---|---|---|
| `content == memory` | `0x25ADE0(globals, 0)` computes an array base whose 181,056 bytes are byte-identical to the package entry | `capture-rig/oodle/item-index.py --controls`, needs `runs/snapshots/orbit-all.snap` |
| `execution: index -> element` | `0x25ADE0(globals, i)`'s own address arithmetic lands on the element whose `+0x00` is that item's hash, and exit RBX is that element's world tag | same |
| `bound-NEGATIVE` | idx 7543 reaches the load, **7544 is refused** by the client's own bound | same |
| `world-tag-anchor` | two live UD2-probed tags reproduce, and the retracted contiguous route is refuted by the same measurement | same |

⛔ **These four cannot be reconstructed from the TSV, and an attempt to do so would be wrong.**
`0x54C090`'s answers and the TSV's column 3 are **different tag spaces**: the live anchors are
`0x80A60297` at index 0 and `0x80A6035A` at 197, while the shipped table's column 3 reads
`0x80a628cf` at index 0 and `0x80a62994` at 197. The shipped table's own tags are all 7,544
distinct with 146 discontinuities, not the "6,750 distinct / 633 discontinuities" quoted for
`0x54C090`. Anyone restoring `world-tag-anchor` must execute the accessor, not read the file.
`[U]`: whether `world_tag_of`'s doc claim ("what `0x54C090` returns", `item_index.rs`) survives that
execution, since the two spaces do not agree at the two indices we can compare.

⭐ **RESTORED 2026-08-11 (adversarial verify): the fifth omission, which was not `d1-replay`'s.**
`item-index.py`'s `hash-space-is-items-NEGATIVE` asserted `not present **and hits > 4000**`. The
port kept only the negative half, leaving a scan with no positive control in the same window - a
table that resolved *nothing* would have passed it cleanly. `item_index::controls` now carries
`catalogue-coverage-and-INVERTED` against `catalogue/d1-catalogue.json`: 4,631 of 5,293
public-manifest item rows resolve, and the same hashes perturbed resolve **0**. As in the Python, an
absent catalogue **fails** the control rather than skipping it.

⚠ **The perturbation is a wide XOR, not `+1`, and that is a measurement.** The item hash space is
not uniform: 2,592 of the 7,544 table hashes have `h+1` also present, so an off-by-one inversion
still resolves 2,143 of 5,293 rows (`+2` resolves 1,165). The first attempt at this control used
`+1`, went red, and the red was the hash space, not the table. `CATALOGUE_INVERSION_MASK` carries
the reason and a test asserts the density still holds, so the justification cannot go stale silently.

**Consequence, stated plainly:** the Rust proves the table's *bytes*; the Python still holds the
proof that the client *reads those bytes at that base*. The snapshot exists and `d1-replay` is a
Rust crate, so the remaining work is wiring, not measurement - but until it is wired, deleting
`item-index.py` deletes three controls. `[U]`: whether a `d1-oracle`-side replay control can be
added without `d1-oracle` depending on `d1-replay` (which is another crate's lane).

---

## 5. The tab is positional: slot -> bucket

⭐⭐⭐ (historicaldocs/design/inventory-instance-fields.md §3.2; announced as the shared AUGMENTED
2026-08-10 banner on three of the source docs.)

`cui::c_investment_backpack_item::ctor` (`0x9F2CC0`) at `0x9F2D60`:

```
mov  edx, [r12+0x24]     ; the SLOT INDEX
mov  esi, [r12+0x34]     ; the SCOPE (0 = profile, 1 = account)
call 0x26db60            ; bucket id = f(scope, slot)      <- the item is not an input
```

`FUN_0026DB60` is a 106-byte leaf scanning **39 records** of `0x14` bytes
(`{u8 id @+0x00, i32 first_slot @+0x04, i32 slot_count @+0x08, u8 scope @+0x10}`), returning the
first whose scope matches and whose `[first, first+count)` contains the slot. The table is
**content** (via `vfunc[19]` = `0x25D250`, from a tag at `globals+0x108`) - not on the wire, not
ours to change. Four independent readers agree on the record layout (`0x26DB60`, `0x26BFB0`,
`0x1055A30`, `0x9EB2B0`); the count 39 reproduces an independently-recorded 39. "Where an item
lives" is neither a wire field nor instance state - it is where you wrote it in the profile's slot
array. `ProfileSpec::item_slots` wrote item n at slot n, which is why every item landed in
Materials.

⭐ The ranges, read live off a snapshot (`d1-replay`, 10 controls PASS):

| slots | bucket | | slots | bucket |
|---|---|---|---|---|
| **0 - 19** | `0x05` **Materials** | | 83 - 92 | Class Armor |
| 20 - 39 | Consumables | | **93 - 102** | Primary Weapons |
| **40 - 42** | Subclass | | **103 - 112** | Special Weapons |
| **43 - 52** | Helmet | | **113 - 122** | **Heavy Weapons** |
| **53 - 62** | Gauntlets | | 123 - 132 | Ships |
| **63 - 72** | Chest Armor | | 133 - 142 | bucket `0x0F` |
| **73 - 82** | Leg Armor | | 143 - 152 | Vehicle |
| | | | 153 - 162 | Ghost |

⚠ The `0x05 = Materials` naming is `[I]` by set-elimination against the manifest, but prediction
and observation agree: we wrote slots 0,1,2,... and saw Materials. **All 39 records are measured**
- `capture-rig/inventory-buckets.tsv`, read out of the live client *and* independently re-derived
by executing `FUN_0026DB60` on `runs/snapshots/orbit-all.snap` (a different boot), with an
inverted not-found control (one slot past the last range returns `0xFF` in each scope) and a
zero-gap / zero-overlap tiling check. Consumed as `d1_oracle::inventory::BUCKET_TABLE`.
⛔ The earlier claim that bucket ids `0x11` (163-167), `0x12` (168), `0x13` (209) "have no
bucket-definition row - no tab can reach them" is **retracted**: they ARE records 15, 16 and 23 of
the table; **non-equippable** buckets, not unreachable ones (the client's own leaf returns `0x13`
for slot 209).

⭐ The tell that split this from the `+0x24` defect: the grid tile name resolves (built by
`0x9F2CC0` from the **profile slot's** u16, which we filled correctly) while the detail panel,
stats and equip check resolve the definition from the **element's `+0x24`**, which we never
filled. Two content indices for one item, neither derived from the other. Same bug class as
MODE-vs-MAP (`historicaldocs/design/activity-and-map-namespace.md`).

---

## 6. The render path, from QueueZ object to drawable

(historicaldocs/design/item-render-path.md §0, §2; historicaldocs/design/light-and-gear.md §2.)
`static-client-inference` unless marked.

| # | condition | where enforced | state |
|---|---|---|---|
| 1 | family 4 is REPLICATING (state 3) when the object arrives | `queuez_family_update_begin` | PASS `[O]` - the item rode the subscription answer (seq 7, 20,679 B), not a push |
| 2 | a 4/3 element exists whose `C+0x000` u64 == the item-instance SOID | `0x56CA70` | PASS `[O]` - measured at `0x37f910448` |
| 3 | the profile's 340-slot array names that SOID | `0x1087660`, slot u64 @+0x08 | **CLOSED 2026-08-09** - `profile-lvl40.bin` names 13 items |
| 3b | the slot's u16 is not `0xFFFF` | `0x1056640` | passes |
| 4 | the world-tag loader runs over the slots | `0x574D90` -> `0x555940` -> `0x1087660` | runs |
| 5 | the u16 resolves to a world tag | `0x54C090(0x548440(), idx)`; `0xFFFFFFFF` = none | indices now measured (§4) |
| 6 | the tag loads (`0x3491A0(...) == 2`) | `0x555730` | `[C]` load state 2 obtained live (§13.2) |
| 7 | the element's own list at `C+0x026` also loads | `0x555730` tail, fixed 8 iterations | `[U]` (we must fill all eight with `0xFFFF`) |
| 8 | UI enumerates the same 340 slots with a stricter iterator | `c_investment_inventory_visible_iterator<0,340>` vtable `0x18F8150`, built at `0x28C6D0` / `0x28CC70` / `0x57B4F0` | `[U]` |
| 9 | UI builds a backpack item and a widget | `0x113FE10` / `0x9F2CC0` -> `c_static_vector<shared_ptr<c_investment_backpack_item>, 340>` (ti `0x1945078`) -> `0xA07F10` / `0xA08560` | `[U]` |

**The world-tag loader layer.** Eight `c_investment_world_tag_loader_fill_desired_*` classes derive
from `i_investment_world_tag_loader_fill_desired` (typeinfo `0x1926C88`), one virtual each:

| class | vtable | method |
|---|---|---|
| `..._items_character_instance` | `0x1926A90` | **`0x574D90`** |
| `..._talent_grids_character_instance` | `0x1926AD0` | `0x575270` |
| `..._items_peer_view` | `0x1926B10` | `0x574E00` |
| `..._items_select_character_view` | `0x1926B50` | `0x574ED0` |
| `..._items_default_gear` | `0x1926B90` | `0x574FB0` |
| `..._handles_character_creation` | `0x1926BD0` | `0x575210` |
| `..._string_lists_rich_presence` | `0x1926C10` | `0x5752E0` |
| `..._items_activity_reward_incidents` | `0x1926C50` | `0x575300` |

Caches `c_investment_world_tag_loader_cache_base<N, request_type>` exist for N = 4, 8, 40, 128,
340, 1460 (type 1) and 768 (type 2); the last vtable slot is the constant thunk returning the
request type (`0x1048A40`->1, `0x59710`->2) - the control that the vtable identification is right.

`0x574D90` gets game-system 4 (`0x574220(4)`) and tail-calls `0x555940`, which walks the three
sources of §1. Per item, `0x555730(u16 idx, element_or_null, collector, bitmapA, bitmapB, out)`:
resolve `tag = 0x54C090(...)`; if not already in the bitmap, request it
(`collector->vtable[0x38]`) and check `0x3491A0(0x3462B0(), tag) == 2`; a second table
(`0x25C9A0()`, `vtable[0x558](idx)`) yields a second tag, requested the same way; then, only if
the element is non-NULL, the fixed 8-iteration walk over `element+0x26`.

**The iterator/predicate family:**

| predicate | used by | test |
|---|---|---|
| `0x1056640` | `inventory_iterator<340>` (ti `0x18F7810`, vtable `0x18F7AC0`) / `<326>` (ti `0x1922018`, vtable `0x1921FF0`) - the world-tag loader | `slot.u16 != 0xFFFF` |
| `0x10559F0` | `item_progression_iterator<0,340>` (vtable `0x18F77E0`) | `slot.u16 != 0xFFFF` AND `slot.u64 SOID != 0` AND element (via `iter[+0x20]->vtable[0]`) has `u16 @+0x44 != 0xFFFF` |
| `0x1055B80` | slot 3 of the visible / progression / active-objective iterators | `byte @ rsi+0x12 == 0` (which byte of what record: `[U]`) |

`c_investment_inventory_visible_iterator<0,340>` (ti `0x18F7820`, vtable `0x18F8150`, virtuals
`0x10559D0 / 0x1056640 / 0x1055A30 / 0x1055B80`) keeps the weak slot-1 test but replaces advance
with **`0x1055A30`**, which consults `0x25A320()->vtable[0x98]()` and bounds itself on
`[obj+0x84]` - the UI's real visibility rule, still unread and the single highest-value unread
function on this path. Inheritance: `inventory_iterator<340>` <- `inventory_visible_iterator<0,340>`
<- `item_progression_const_iterator<0,340>` <- `item_progression_iterator<0,340>`;
`inventory_active_objective_item_iterator<0,340>` also derives from the visible iterator.
`e_investment_inventory_bucket_scope_type` is instantiated only with value 0 in this build (4
instantiations, all `...type0E`; control: the same scan finds `world_tag_request_type1E` / `2E`).

⚠ The UI does **not** reach elements through `0x56CA70`: the visible iterator resolves each element
through its own `vfunc[0x98]` (`0x28CC70`, argument `slot[i] + 0x08` = the SOID). Elements reached
that way additionally read `+0x24`, `+0x58`, `+0x98` - the refutation of the two-site census's
"only three fields read" (dead claim D6).

**UI classes, addresses in hand, untraced `[U]`:** `c_inventory_bucket_data_component` (vtable
`0x1944680`, 0x170 B, alloc `0x113A420`); `c_investment_backpack_item` (vtable `0x1944950`, built
by `0x113FE10` / `0x9F2CC0` on a `c_investment_item_context`); `c_investment_item_widget` (vtable
`0x1946750`, built at `0xA07F10` / `0xA08560`); `c_investment_equip_inventory_item_component`
(vtable `0x193B940`, built at `0x10EAEB0`).

**Two never-fired diagnostics sit exactly on this path**: `0x162EF5E`
`"fill investment item cache for 0x%llx"` (fn `0x576C00`) and `0x162EF36` `"unload investment item
cache for 0x%llx"` (fn `0x575D30`). Both are evidence class `none` (SEEN 0, n_runs 0) so absence
proves nothing; appearance would be direct confirmation. The diagnostic catalogue has **no
`investment:` root at all** (1276 networking / 507 world_controller / 6 ui / 0 investment) - the
coverage gap is the catalogue's.

**What the empty era looked like, measured `[C]`:** `capture-rig/profiles/profile-old-russia.bin`
(20,318 B) had exactly two non-zero fields - SOID `0xd100000000000201` and `+0x0028`
last_orbited_destination `0x932015BB`. Run `20260808-025446-GJALLARHORN` delivered the item
(key `0xd100000000000301`, 20,679 B body after the `0x000D` ACK) and it landed; no slot named it,
so no path to the screen existed. `[O]` `synthetic-minimal`, `synthetic_ancestor`. (That run's log
also shows one family-4 push with `object_count 1` refused at `replication state=1` during
`ambient_bootstrap` - the `queuez-debug` trap, not the cause here.) The `0xFFFF` sentinel fill
costs zero bytes: zeroed and populated profiles both encode to 20,318 B / 162,539 bits; the
first populated round-trip read back `+0x0038 = 0x1E3F` (a placeholder proving the encoding, never
a real index) and `+0x0040 = 0xD100000000000301`.

---

## 7. Geometry: the art-arrangement index

(historicaldocs/design/inventory-instance-fields.md §3.1, incorporating its CORRECTED 2026-08-10
banner. Current fixture work: `design/geared-guardian-fixtures.md`.)

⛔ **The family-4/type-3 element carries no art-arrangement field at all.** Its only contributions
to geometry are `+0x24` (stamped to `out+0x184`) and `+0x26` (an override list). Fixing `+0x24`
alone does not make an item draw.

The art-arrangement index is field **`[3] @+0x00C` of `0x8080183C`** - a u16 in the 64-byte
equipment-slot record, 20 of which live in `0x808045D9` at `+0x1E8` of the appearance block
`0x80801855`, which hangs off the **character render object**: family **0/1** (`+0x30`), **1/1**
(`+0xB60`), **3/1** (`+0x68`) - schema and disassembly agreeing byte-exactly across all three.
⭐ The corrected framing: the "object we have never served" was the card we already serve (family
3/1 as `D1_CHARACTER_FILE`; family 0/1 peer view via `build-peer`); the never-served thing was
**one field of it**. `FUN_0054CF70` reads `block + 0x1F4 + slot*0x40` (caller chains:
`0xA98550 -> 0x54EB40` resolves family 3/1 `+0x68` for the preview; `0x58D350 -> 0x54CF40`
resolves family 0/1 `+0x30`). We shipped `0` there - IN RANGE for `FUN_0054B530`'s bounds check,
silently naming arrangement #0 - and 8 zeros in the extras array at `+0x0E`, each a live reference
(`FUN_0054E6C0` skips only `-1`).

`FUN_0054CF70` builds the render description: four u64 per equipment slot at
`out + 0x188 + slot*0x20`, pre-initialised to `0xFFFFFFFFFFFFFFFF` = "no geometry". The gate is the
arrangement index, resolved by `FUN_0054B530` against a 24-byte-stride table at investment-globals
root `+0x310` - a different table from the 16-byte world-tag table at `+0x1D0`.

⭐ The class-correct value is CONTENT, derivable offline: `FUN_00289E80(def+0x60 gear record,
class)`, where the definition is the package entry named by the item's world tag (it carries the
item's own hash at `+0x78` - the identity check). Gjallarhorn `0xD471D331` -> arrangement **1229**
(wildcard class). Instrument: **`d1-oracle art-arrangement`** since 2026-08-11 (15 controls, **two
inverted**: the walk from `gear+2`, and the curve offset read two bytes late) - a port of
`capture-rig/oodle/art-arrangement.py` that reproduces its output **byte for byte** apart from the
`# GENERATED by` line (`d1-oracle art-arrangement diff --against …`, exit 0, executed). Consumed as
`capture-rig/item-art-arrangement.tsv` by `d1_oracle::item_index::ArtTable`.

⛔ **The consumer's provenance check was widened in the same change.** It literal-matched
`"GENERATED by capture-rig/oodle/art-arrangement.py"`, which meant a Rust generator writing an
honest byline was **rejected by the shipped binary**. It now accepts any of
`item_index::KNOWN_GENERATORS` and still refuses a table with no generator line at all - which is
the thing the check was ever for.

What stands regardless of the arrangement index: geometry also requires the definition to RESOLVE
(`vtbl[0x410]`'s world-tag-load gate, §3) and `def+0x60` non-null - a subclass (Striker) and
'Bronze Tier' have no model *by content*, and an unloaded tag means no model however good the
index. ⭐ This is **not** the rung-34 task-7 stuck world-tag cache: `FUN_0058D220` (whose
`cmp byte ptr [rcx],4` is task 7's gate) has five callers and none is in the preview-widget chain
`0xA97270 -> 0xA98550 -> 0x54EB40`.

---

## 8. The on-screen numbers decoded

(historicaldocs/design/inventory-instance-fields.md §4.)

### 8.1 "Requires Level 953998653" - closed: the screen reads `+0x1C`

Eliminations, each with a passing positive control: `953998653` (`0x38DCDD3D`) and `953998645`
(`0x38DCDD35`) appear nowhere in the eboot as 32-bit LE constants (control: `fnv1
("platform_signin") = 0xE9D46191` and tag `0x80801732` found by the same search - bucket hashes
are data, not code); `953998653` is none of the 40 `DestinyInventoryBucketDefinition` rows
(`953998645` IS Heavy Weapons).

The mechanism, closed 2026-08-10: `FUN_00589900`, called by the item widget's data fill at
`0x9FAF45`, is literally `return *(u32*)(element + 0x1c + primary_stat_index * 4)` - **the number
on screen is `+0x1C`**, where `stage_loadout()` wrote `Slot::Heavy.bucket_hash() = 953998645`
(`d1-loadout/src/model.rs`). The **+8** is the stat-curve term: `FUN_0026F110` computes
`(float)*(int*)(element + 0x18 + cVar6*4) + curve((float)(short)(FUN_00286560(element+0x38) +
*(short*)(element+0x20)), <table via vtbl[0x4C8]>)` - base + curve, with the curve contributing 8.
`[C]` on the read shape and the writer; the identification of `FUN_0026F110` itself (infuse?
dismantle? comparison?) stays `[U]`. ⚠ The shared AUGMENTED banner on three source docs called the
+8 "a coincidence"; the canonical doc's own CLOSED banner supersedes that: the +8 is the predicted
curve term. Experiment A (write 41/42/43 into `+0x18/+0x1C/+0x20` and read the panel) is closed
and does not need to be run.

The stronger reader that fixed the magnitude: `0x269760` (equip iterator ctor) treats the same
field as a **minimum stat requirement** - `0x269968 call 0x276200` (the character's current value
of stat id `0x0075`), `0x269971 cmp eax,[rcx+rbx*4+0x1c]`, `jge`, else `or byte [rax],8` (not
equippable). Requirement-table live count 1, so the loop touches exactly `+0x1C`. We told the
client "this item requires stat >= 953,998,645" - un-equippable by construction.

Also live: `FUN_00572A10` computes a level comparison from the element - if `+0x58 != -1` and
`+0x44 != -1`, `iVar3 = FUN_00284640(+0x58 via vtbl[0x4E0])` (a requirement) vs
`iVar4 = FUN_00284630(param_3 + 7)` (the item's level from `+0x38` XP and `+0x44` ordinal). =>
`+0x38`/`+0x44`/`+0x58` are the level machinery; `0` there asserts "progression definition 0" and
"table-156 entry 0", not "none". `[C]`

### 8.2 "3 Attack" - Attack is computed, not stored

The item widget's populate `0x9F8580` memcpys the whole 184-byte element into `widget+0x1A0`
(0xB8 B) and the 32-byte profile slot into `widget+0x180` (it also stores `slot+0x10` into UI int
`widget+0x2D0`, and `+0x18`'s value into `widget+0x2C8` at `0x9F8C6B`), selects the primary stat
from `def+0x50` (array-0 element 0's u8; **Attack = 11, Defense = 1, Light = 0**), and calls
`0xA83F00`, whose `out+0x3C` is `FUN_0026F710` = the talent-grid term `FUN_002869B0` plus, per
stat in `def+0x50`:

```c
x = (float)( *(i16*)(element + 0x20) + level );
v = interpolate(stat_group_curve(stat), x);          // 0x1056720
v = round( v + (float)*(i32*)(element + 0x18 + sel*4) );
return (int)(v * weight * scale);
```

With `+0x18` = -730,737,871 every term floors; the residual **3** is most plausibly the grid term
added afterwards. `[I]`. Where the literal 3 comes from stays `[U]` (with `+0x98` zero and Bronze
Tier's stat list empty, `0x28CC70`'s match loop never fires and nothing is read). `+0x98` is a
separate channel: per-instance stat values paired positionally with `def+0x68`, read by
`FUN_0028CBB0`, aggregated by `0x28D210` via the stat def's aggregation byte at `statdef+5`
(1 = min, 2 = max, 3 = sum). The free two-item read (Chest Armor `+0x1C` = 14,239,492, Leg Armor =
20,886,954: does the panel show ~14,239,49x?) was the zero-cost discriminator; the mechanism is
now closed by `FUN_00589900` above.

---

## 9. Staging a correct item

(historicaldocs/design/inventory-instance-fields.md §5. The delivered fixtures implementing this
live in `design/geared-guardian-fixtures.md`; runs in `design/geared-visuals-run-1.md`.)

**There are two change sites, and the tab fix is not in the element.**

**First, the profile:** place each item's slot inside its bucket's range (§5): Gjallarhorn at
113-122, a helmet at 43-52, the Striker subclass at 40-42; every other slot `0xFFFF`. (The
one-boot discriminator: move one item from slot 0 to slot 43; the tab moves while the detail panel
stays broken, separating defect 1 from defect 2.)

**Then the element** (`stage_loadout()` in `d1-server/crates/d1-responder/src/main.rs`); anything
not listed stays 0:

| off | was | should be | why | ev |
|---|---|---|---|---|
| `+0x00` | SOID | unchanged | registry key; must equal the profile slot's u64 @+0x08 | `[C]` |
| `+0x24` | `1` | the item's **content index** (Gjallarhorn Y1 `0x4BF4BE3F` => 2085; `0xD471D331` => 6299; Striker `0x925CDEEA` => 710) | the definition lookup's argument | `[C]` |
| `+0x58` | `0` | `0xFFFF` unless a real grid index is known | `0` claims entry 0 of table 156 | `[C]` |
| `+0x26` (8 x u16) | all `0` | `0xFFFF` in ALL EIGHT | `0` is a valid content index; the loop is fixed 8 iterations, setting only slot 0 does nothing | `[C]` |
| `+0x88` / `+0x98` | all `0` | MEASURED 2026-08-11: leave EMPTY - `def+0x68` is NULL/empty for all 14 staged items, and the client's own ctor `FUN_0026E0E0` copies only when present | `0x28CBB0` / `0x28B6E0` / `0x26E0E0`; => `design/geared-guardian-fixtures.md` §9.2 | `[C]` package bytes |
| `+0x20` | `1` | MEASURED 2026-08-11: `*(u16*)(rec30+8)` of `def+0x30`'s self-relative record (what `FUN_0026E0E0` stores). Oryx Mask = 70, every other staged item 0. Extracted by `art-arrangement.py`, filled by `inventory rebuild` | `FUN_002816F0` samples the curve at `+0x20 + level` | `[C]` |
| `+0x38` | `0` | XP for the desired item level, if `+0x44` names a progression | `FUN_00284630` | `[U]` |
| `+0x1C` | `bucketTypeHash` | **`0`** (or the item's real stat-`0x0075` minimum) | it is the equip minimum, compared at `0x269971` | `[C]` |
| `+0x18` | item hash | **the item's LEVEL** - a small int (e.g. `40`) | as i32 the hash is -730,737,871: floors every stat term, fails every level gate | `[C]` |
| `+0x44` | `0` | a **real** progression ordinal (e.g. `base_item_level`), with `+0x38` = its progress | `0xFFFF` makes `0x10559F0` reject the item entirely; `0` names ordinal 0. Only a real ordinal is right (the earlier `0xFFFF` advice is retracted) | `[C]` |

⭐ The lookup table already exists in Rust:
`d1_oracle::item_index::IndexTable::{load_default, index_of, require}` - `stage_loadout()` writing
`1` while this sat unwired is the project's signature failure (built, controlled, never called).
⛔ A guard should ship with any change here: `d1_oracle::profile::StagedItem` decodes
`content_index_field` out of every staged file and `ProfileSpec::item_slots` holds the correct
index for the same item; **nothing asserts they are equal**, and such a control can only fail in
the direction we care about.

**Reach:** everything structural is `static-client-inference` against the pinned 01.29 eboot; live
element bytes quoted from `d1guest obj 4:3` are `synthetic_minimal` (our fixture - evidence about
`d1-responder`, never about a real service); manifest cross-references are naming aids. First
confirmation on screen belongs to the geared-visuals runs.

---

## 10. Light and level

(historicaldocs/design/light-and-gear.md §3-§5, with its retraction banner applied.)

**Light is a stat, not a field.** `0x265EF0` sums it over the 20 equipped items at profile
`+0x2B80`. There is an override, the cheapest path to non-zero Light:

```
profile +0x6CDC   bool, clamps to 1  - override PRESENT
profile +0x6CE0   7 x { u8 stat_index, u32 value }   (8-byte stride)
profile +0x6D18   the SECOND override block's `present` bool
```

⛔ The pre-2026-08-08 reading (`+0x6CE8`, 6 entries, `+0x6D18` "a separate bool") is **retracted**;
three instruments agree on the correction: the schema (`0x80804768` @ `+0x6CE0`, header count 7,
element 8, `7x8 = 56` = its struct size); the reader `0x265EF0` (`cmp [rax+rdi*8+0x10], bl` with
`rax = profile+0x6CD0` => key at `+0x6CE0+8i`, i = 0..6; the `lea rdi,[rdi+1]` sits after the key
compare, so only the value load sees it: `[rax+rdi*8+0xc]` = `+0x14+8i`); and the codec ⚑ (7
entries at `+0x6CE0` round-trip IDENTICAL; 7 at `+0x6CE8` are REFUSED at `+0x6D18`; `+0x6CDC`
clamps `sent 07, got 01`). Pinned as a negative control in `d1-oracle profile controls`. When the
override is present the client returns the value verbatim and never touches equipped items - Light
can be set on a Guardian with an empty inventory. The Light stat index was rulered
(`capture-rig/profiles/profile-light6.bin`, six slots, values 100..600) and the shipped fixtures
use **index 0 = Light** (consistent with `def+0x50`'s Attack = 11 / Defense = 1 / Light = 0).
On the character-select card, light is purely stored: `0x2666F0` scans a 16-entry
`{u8 stat_index, u32 value}` table at family-3 `C+0x870` - different object, different mechanism.

**Level is two different things:**

| where | mechanism |
|---|---|
| character-select card (family 3/1) | a plain u32 the server states, at `C+0x68`, read at `0xA11555` |
| in world (family 4/1) | a **progression**: `+0x3660` u32 count, then 127 x 16 B `{u32 current, u32 daily, u32 weekly, u16 progression_index}`, `0xFFFF` = empty |

Account progressions are the same element type at family 4/0 `+0x304C`, fixed 127. Confirmed three
ways: iterator vtables (`0x273BE0` / `0x273B70` / `0x273BC0`), the QueueZ change announcer inside
`0x56D3F0`, and the schema. The `character_level` progression index for 2016 content is
**`0x75` = 117** (109 / 116 for 2014 / 2015 - version drift is real). ⛔ **Family 2 is NOT
progressions**: it is `{u64, u64, u16[27], u16[5]}` - a 26-character display name and 4-character
clan tag; `0x57F6B0`/`0x57F6F0` are a two-hop player lookup feeding roster and player-card code.
("231 slots ~ 252 progression definitions" was a coincidence-of-numbers argument, wrong.)

**Which object to populate:** family 4/1 - the 28,792-byte (C-struct) profile `0x80801743` - is
where in-world state lives: level, light, progressions, equipment slots, stats. ⚠ Family 3 is the
character-select *card*, not the character: markers written into it (11/22/33/44 at
`C+0x30..0x3C`, 40 at `C+0x68`) are read by `0xA110F0`, a screen the client is not on in orbit.

`0x8080176B` is the element class of *both* progression containers (`0x80804745` = 2032/16 = 127
per account, `0x808047A7` for the profile) *and* the item element's `+0x38` block; one reader,
`FUN_00275870`, walks all three.

⛔ **`d1guest tag` cannot resolve manifest content hashes.** It computes `index = tag & 0x1FFF`,
`slot = f(tag >> 13)` - decomposition arithmetic, not a lookup. `0x4BF4BE3F & 0x1FFF = 7743`, the
dumped object all zeros. The claim "2020 manifest item hashes resolve on the 2016 client" built on
that output is retracted; whether they resolve is `[U]`.

---

## 11. Default gear, the character push, and the blank-Guardian scope limit

(historicaldocs/design/light-and-gear.md §6b and its 2026-08-10 banner.)

⛔⛔ **Scope limit, banner-grade:** the no-gear explanation below explains a Guardian with no gear.
It does **not** explain the absence of a Guardian at `activity:in_world` - the player spawn aborts
two gates upstream of the creation record and of everything gear-related (no player record exists
at all; the creator `FUN_0090D1E0` is simply never called; the earlier `gg + 0xD0` framing is
retracted). **Hypothesis B - "no biped because it has no equippable gear" - is REFUTED as the
proximate cause.** The gear is genuinely broken and worth fixing; it is not why the body was
missing. => `historicaldocs/design/player-creation-chain.md`, `design/player-in-world.md`.

**Why the Guardian had no gear:** `run-boot.sh`'s `D1_PUSH_CHARACTER` exists to make
`character:signin` see count 1 and take selection; its own comment says it keeps character
creation, **starter inventory** and the prologue off the critical path. Character creation is
where `c_investment_world_tag_loader_fill_desired_items_default_gear` (`0x574FB0`) runs - we
skipped the only code that grants gear. But creation is not the escape hatch: it drops into
`setup:prologue_intro_loading` / `character:prologue`, a cinematic the lab cannot run
(operator-identified). The route: take what creation would have granted and give it to a pushed,
post-prologue character.

**Three family-3 states, not two:**

| family 3 | `character:signin` does |
|---|---|
| not sent at all | walks straight through - no UI, no creator, no selection |
| roster with 0 characters | shows the character creator |
| roster + 1 character | selection |

`D1_PUSH_CHARACTER=0` gives the FIRST row, measured (run `LET-ME-CREATE`: zero family-3 objects,
client went `character:signin -> character:prologue -> cleanup -> orbit` without stopping).
`D1_EMPTY_ROSTER=1` was added for the middle row, and ⚠ the account's character list must agree -
`0xA110F0` reads the count from the ACCOUNT, not the roster.

**The default-gear list is readable with no server object:** `0x25A320()` -> singleton
`0x22DBA5B80` -> vtable `+0x88` = `0x25D150` -> list `0x3808FDA68`, count at `+0x10` = 3, entries
at `+0x18`. First entry carries real `0x8080xxxx` **world tags** - `0x80800184` and `0x8080170F` -
the form the item path actually resolves, not manifest hashes. `[O]` The record layout beyond
entry 0 is not a clean `0x20` stride and is `[U]`.

⛔ Retracted: "character selection works." `message 504` (the documented selection success signal)
fired in three 2026-08-07 runs serving an 88-byte family 4 and an 87-byte family 3 (sparse, most
fields absent) and in none of the 2026-08-08 runs serving 21 KB / 2.3 KB oracle-encoded objects
with every field present-and-zero. Bisect from the sparse baseline, not from the full one.

---

## 12. The loadout framework and the manifest

(historicaldocs/design/loadout-framework.md.) **Reach:** `static-client-inference` at best, and
the source is Bungie's public 2020 manifest, not our client. Except where marked `[O]`/`[C]`
(measured against the preserved database), nothing has been confirmed against the 01.29 client.

**What exists:** `d1-server/crates/d1-loadout` (data model: `Loadout`, `ItemRef`, `Slot`,
validation, era classifier, world-manifest reader, catalogue extract - **no wire code**), CLI
(`d1-loadout build | controls | search | show | validate | slots`, all `--json`; 17 controls, exit
2 on failure), `d1-server/crates/d1-forge` (egui editor; runs nothing - reads a JSON file, POSTs
to a URL), `catalogue/d1-catalogue.json` (gitignored, 5,293 equippable items), `loadouts/*.json`
(three Year-1 Vault of Glass sets: `y1-titan-vault-of-glass.json` Striker/Kabr/Fatebringer/Found
Verdict/Gjallarhorn; `y1-hunter-...` Bladedancer/Prime Zealot/The Last Word/Praedyth's
Revenge/Corrective Measure; `y1-warlock-...` Voidwalker/Hezen Lords/Obsidian Mind). Tests:
`cargo test -p d1-loadout` (24), `-p d1-forge` (9); manifest-backed tests parse the real database
and pair every "found nothing" assertion with a "found something" control. ⚠ The first test
fixture silently skipped via `.ok()?` - eight tests reported ok while asserting nothing; it now
panics on any failure after the file is located.

**Pins** (verified by cargo, 2026-08-08, not recalled): egui/eframe/egui_extras/egui_kittest
**0.36.1** as a set; rusqlite 0.40.1 `bundled`; zip 8.6.0 `default-features = false,
features = ["deflate-flate2-zlib-rs"]`; ureq 3.3.0 rustls. All GPL-2.0-or-later compatible;
nothing read from the excluded prior art. egui 0.36 breaks recorded so nobody "fixes" them back:
`SidePanel`/`TopBottomPanel` unified into `egui::Panel::{left,right,top,bottom}` with
`.default_size()`/`.size_range()`; `Panel::show_inside` deprecated for `show`;
`Context::style()`/`set_style()` replaced by per-theme `style_of`/`set_style_of`/`style_mut_of`
and `ctx.all_styles_mut(...)`. The `App` trait (`logic` + `ui`) is as
`interface-architecture.md` §5b describes.

**The Loadout JSON (`schema_version: 1`):** `hash` (the manifest `itemHash`) is the only
load-bearing field; everything else (`name`, `tier`, `bucket_hash`, `item_type_name`, `icon`,
`class_type`, `damage`, `era`, `era_note`) is a display cache re-resolved by
`Catalogue::refresh_ref`. `equipped` is a list, not a map (so `duplicate_slot` stays reportable);
deserialisation is tolerant (unknown fields ignored, newer `schema_version` a hard error);
serialisation is deterministic (sorted by `Slot::ALL`). Validation errors: `empty_name`,
`duplicate_slot`, `zero_hash`, `bucket_slot_mismatch`, `class_mismatch`; warnings:
`post_era_item`, `too_many_exotic_weapons`, `too_many_exotic_armor`, `level_out_of_range`,
`light_out_of_range`, `not_in_catalogue` / `stale_cache` under `validate --refresh`. **All `[I]`**
- manifest-consistency rules never observed enforced by the client. ⚠ Wire constraint (from §6):
`{itemHash, bucketTypeHash}` is not directly expressible on the wire; the client identifies an
item by a u16 content index (§4) and position (§5). The translation now exists:
`item-content-index.tsv` + the bucket ranges.

**The Apply contract** (`POST /control/loadout/apply`, loopback bind only, `ssh -L` as the auth
layer): types in `d1_loadout::apply` (`ApplyRequest`/`ApplyResponse`), path constant
`d1_loadout::APPLY_PATH`. A control-plane call between our own tools - NOT a game protocol
message. Request: `contract_version`, `request_id` (echoed; correlation + retry identity),
`target {index, character_id}`, `dry_run`, and the whole loadout document (never a delta).
`ApplyResponse` defaults `accepted: false` so an empty body can never read as success. The client
distinguishes: connect refused/timeout (muted, expected), 404/405 (muted - route not implemented,
expected today), 501/other non-2xx (red), 2xx with a non-`ApplyResponse` body (red), 2xx
`accepted: false` + `issues` (red), 2xx `accepted: true` (green). Server side does not exist yet;
suggested order: echo route honouring `dry_run` -> server-side validation into `issues` -> actual
state change.

**The manifest**
(`manifest/data/world_en_world_sql_content_39adcbfdc2021172e8ccf4720ca76023.content`, version
`88857.20.11.05.1145-2`): a ZIP with one DEFLATE member wrapping a SQLite 3 database (5.7 MB ->
59 MB), 39 tables. Three traps, all `[C]` measured:

1. **`id` is the hash reinterpreted as signed i32.** Gjallarhorn: `itemHash` 3564229425, `id`
   -730737871; `MIN(id)` = -2146969240. Filter `id > 0` silently drops half the game; parse
   `itemHash` from the JSON.
2. **The `json` column is declared BLOB and stores TEXT** (`typeof(json)` = 'text' for all 8,609
   item rows); binding `Vec<u8>` fails with `Invalid column type Text at index 0`.
3. **The same item exists under several hashes** - Gjallarhorn is 1274330687 (Year 1) AND
   3564229425 (Rise of Iron quest version). **An item is a hash, never a name.**

Useful hashes. Classes (`DestinyClassDefinition`): Titan 0 / 3655393761, Hunter 1 / 671679327,
Warlock 2 / 2271682572; `classType == 3` on an item = "any class", not a fourth class. Races:
Human 3887404748, Awoken 2803282938, Exo 898834093. Genders: Male 3111576190, Female 2204441813.
Equip buckets (`DestinyInventoryBucketDefinition` rows with `category == 3`, in `bucketOrder` -
`category` is why this is not guesswork; vault category 1 and currency category 2 are excluded):

| slot | bucketHash | | slot | bucketHash |
|---|---|---|---|---|
| Subclass | 3284755031 | | Artifact | 434908299 |
| Primary Weapons | 1498876634 | | Vehicle | 2025709351 |
| Special Weapons | 2465295065 | | Sparrow Horn | 3796357825 |
| Heavy Weapons | 953998645 | | Ships | 284967655 |
| Ghost | 4023194814 | | Shaders | 2973005342 |
| Helmet | 3448274439 | | Emblems | 4274335291 |
| Gauntlets | 3551918588 | | Emotes | 3054419239 |
| Chest Armor | 14239492 | | | |
| Leg Armor | 20886954 | | | |
| Class Armor | 1585787867 | | | |

Damage (`DestinyDamageTypeDefinition.enumValue`): Kinetic 1 / 3373582085, Arc 2 / 2303181850,
Solar 3 / 1847026933, Void 4 / 3454344768. Subclasses are items and class-locked: Striker
2455559914, Defender 2007186000, Sunbreaker 21395672; Bladedancer 2962927168, Gunslinger
1716862031, Nightstalker 4143670656; Voidwalker 3828867689, Sunsinger 3658182170, Stormcaller
1256644900 (the three Taken King subclasses appear twice, the Year-1 six once). ⚠ Seven weapons
carry a real class restriction: Ace of Spades 552354419 (Hunter), Fabian Strategy 2748609458
(Titan), Tlaloc 803312564 (Warlock), Immobius 2008951974 (Titan), Stillpiercer 1742712674
(Hunter), Susanoo 4257542172 (Warlock), one redacted Classified heavy 2291160782 (Titan). Counts:
8,609 items -> 5,293 equippable; tiers 3,827 Legendary / 1,687 Common / 1,146 Rare / 559 Uncommon /
461 Exotic / 926 Basic-or-None; 2,942 items (34%) carry no `sourceHashes`.

**Era.** The manifest has no date-added field (2020 capture, 2016 target: 01.29 = Bungie 2.4.1,
18 Oct 2016). Two signals: (1) reward sources can disprove, never prove -
`d1_loadout::era::SOURCE_TIMING` sides each of 64 `DestinyRewardSourceDefinition` activities of
18 Oct 2016; confidently after: Age of Triumph (28 Mar 2017), Raid Reprise `[I]`, The Dawning
(13 Dec 2016); verdicts `post_era` (not in our build) / `sources_in_era` (**no evidence against,
NOT "in era"**) / `sources_mixed` / `unknown` (the default, a third of the catalogue) / `pinned`;
controls: Fatebringer (Adept) 1288233773 -> `post_era`, Fatebringer 3892023023 -> `sources_in_era`.
(2) `qualityLevel` separates Year-1 copies from re-issues, `[O]` measured: Gjallarhorn
1274330687 q70 vs 3564229425 q0; Vex Mythoclast 346443849 q70 vs 4113238754 q0; Thunderlord
3191797831 q70 vs 57660787 q0; Found Verdict 892741686 q60 vs 1444401177 q0; Helm of Saint-14
2994845059 q70 vs 941890991 q0; controlled in the other direction by TTK-or-later exotics all
q0 (Nemesis Star 3851373522, Sleeper Simulant 3012398149, Touch of Malice 3688594189, Zhalo
Supercell 255654879, Outbreak Prime 3742521821). Year-1 vs later only, not 01.29 specifically.
`[U]` level 30 / light 30 in the shipped loadouts are Year-1 caps on a level-40 build - internally
inconsistent in the game's own terms, and the files say so. Y1 detail: Universal Remote and Vex
Mythoclast sit in the Primary bucket, correct for Year 1 only. Item icons are bungie.net paths;
nothing fetches them and this box must not reach it.

---

## 13. Availability builder and item-gate probes

(historicaldocs/design/availability-and-item-gate-probes.md.) **Reach:** the static setup is
`static-client-inference`; the run results are `observed-live` *for the client's own control
flow*, but the run was driven against our answering surface, so every statement about what the
client did with our objects is `synthetic_ancestor`. Run:
`runs/20260808-110410-AVAIL-ITEM-TRACE`, 5,641 records, `grep "byte guard failed" emulator.log`
-> 0 (all 17 probes armed; quiet means quiet).

**Setup facts that outlived the run.** `0x260200` (the availability builder) writes a 3-byte
header at its single common exit `0x260438`: `mov byte [r13], 1` (unconditional), `mov [r13+1],
al` (the accumulated verdict; optimistic init `mov byte [rsp+0xbf], 1` at `0x260238`, five failure
sites all `mov byte [rsp+0xbf], 0` and all FALL THROUGH - read the trace as a set), `mov byte
[r13+2], 0`. The item loop (`0x1087710` head, `0x1087778` per-iteration increment) calls the gate
`0x555730` **even for empty slots** (`cmp qword [r12+rbx+0x10], 0 / je` skips only the registry
find; `movzx edi, word [r12+rbx+8]` is handed to the gate anyway with element = 0). Probe
catalogue: `capture-rig/director-item-probe-sites.json`, 17 sites, ids 69-85; groups anchored on
`AvailEntry 0x260200` / pump `AvailRearmPump 0x260438` and `ItemLoopHead 0x1087710` / pump
`ItemLoopPump 0x1087778` (anchoring the pump on the gate call `0x1087768` would go dead exactly
when the loop runs without requesting a tag). Per-probe cap 800 on the 9 item probes against a
60,000 global budget (one pass = 340 + 326 + 64 = 730 slots x 9 probes). Run recipe:
`python3 -B capture-rig/probe-guards.py --controls` (75/75), then
`OPERATOR_PAD=1 D1_ANSWER_FAMILIES=0,2,4 D1_PUSH_CHARACTER=1 D1_PUSH_FAMILY=4,3 D1_FAMILY42=1
./capture-rig/run-boot.sh AVAIL-ITEM-TRACE`, decode with
`capture-rig/decode-account-trace.py`.

### 13.1 The availability builder SUCCEEDS `[C]`

Three invocations: #201-204 `avail_fail_b` at `0x2603EC`, AL=0 (early boot); #4974-4976 AL=1;
#5124-5126 AL=1 with `header_before = 00 01 00`. The director was blank with a valid, successful
availability cache: `historicaldocs/design/director-selectability.md`'s central claim is REFUTED, and the
`{1,0,0}` snapshots that motivated it had caught invocation 1 of a three-invocation function.
`ctx+0x28 = 0x37f92e2f8` at every entry - never null.

### 13.2 Load state 2 PASSES `[C]`

`0x3491A0` returned 0 once (early boot) and **2** on both later passes - the last unverified link
of §6's chain (link 6) is obtained by the client.

### 13.3 ⚠ The 680/681 "pass rate" is an artefact - do not quote it

681 tag resolutions produced exactly two distinct tags: `0x80A60297` x679 and `0x80A6035A` x2.
`0x5557AC` is a dedup bitmap (`bt eax, ecx / jb` skip keeps the 1 set at `0x555778`;
`or [rcx+rdx*4], ebx` records and falls into the check): 678 of the verdicts are short-circuits.
**Only 3 real load-state checks happened in the whole run** - one distinct item, three times.
Count distinct tags, never records. And the profile named garbage, measured: 679 of 681 slots
named index 0 (`0x80A60297`), the zero-filled-profile failure predicted in §1, now `[C]`. The
other tag `0x80A6035A` is index **197** (the "index 195" line was a subtraction from the refuted
contiguous claim; executed: `0x54C090(handle,195) = 0x80A60358`, `(196) = 0x80A60359`,
`(197) = 0x80A6035A`).

### 13.4 The cache is built successfully and then DESTROYED

Measured live in `runs/20260808-142443-WORLD-LAUNCH-1080-C` (client in orbit, director blank,
liveness L3): the builder wrote success twice (seq 205 AL=0 `r13=0x37f8bd338`; seq 4977 and 5046
AL=1 -> `{1,1,0}`); the singleton still points at the block (`d1guest read 0x26A34B0`: `+0x28 =
0x37f92e2f8`, `+0x38 = 0x37f8bd330`, intact; `0x37f8bd330 + 8` = the builder's R13); and that
memory is **zero** (three reads over ~10 s identical - not a race). Since `0x260438`'s first
instruction unconditionally sets `header[0] = 1`, `header[0] == 0` is unreachable from the builder
`[C]`: the block was zeroed or freed-and-reused by something else. **What zeroes the block is
`[U]`.** It is not `0x5745F5` (`mov qword ptr [rcx+0x38], 0` NULLs the pointer, which is intact).
Next: sample the header over time with `d1guest watch` to bracket the zeroing between
`avail_rearm_pump` sequence numbers (cheap, no rebuild) before probing the teardown cluster
(`0x574340`/`0x574440`/`0x574460`). ⚠ Before reading any quiet trace as "never got there", check
`grep "byte guard failed"` - `ValidateProbeBytes()` is all-or-nothing.

---

## 14. Method traps paid for here

Compact index; full narratives in historicaldocs/design/inventory-instance-fields.md §7 and
historicaldocs/design/item-render-path.md §7.

- **Matching a variable NAME is not matching an OBJECT.** Ghidra reuses local names across objects
  in one function; a textual field census over decompiler output without a control a wrong object
  would fail returns a clean scan of the wrong question.
- **Literal-offset scans are blind to computed displacements.** `xref` misses `object + literal`;
  `field-xref` misses `object + base + index*scale`. When a descriptor says inline array, search
  the ARRAY BASE (`+ 0x18`, not `+ 0x1c`; `+ 0x98`, not `+ 0x9c`). "No consumer found" for any
  offset past the base is not evidence of absence.
- **A byte-pattern scan of one encoding is not a scan of the field.** `8b 44 83 18` found the
  `+0x18` reader; the real `+0x1C` reader was `3b 44 99 1c` (`cmp`, different registers) at
  `0x269971`. Enumerate opcode x register pair x displacement width, or use `d1-eboot field-xref`.
- **A reachability closure from known call sites misses argument-forwarded holders** (`0x269760`
  receives the element from `0x260200`, `0x269580`). Treat unclosed `[U]`s as unsearched.
- **When two searches disagree, disassemble both cited addresses before believing either** - here
  both instructions existed; the "contradiction" was two partial scans.
- **Co-occurring symptoms are evidence of a common TRIGGER, not a common cause.** Serving an item
  for the first time exercised four mechanisms at once; before attributing symptom S to cause C,
  name the instruction that reads C on the path that produces S.
- **A positive control proves the instrument found the thing you already knew; it cannot prove the
  search space was complete.** State the space, not just the control.
- **"Contiguous, verified 4-for-4" is a claim about four points.** When a claim is about a whole
  table, sample the whole table (7,544 reads killed the subtraction formula).
- **Check the eliminated list before opening a new investigation** - "bucket 5 covers slots 0-19"
  had been filed as an *eliminated* theory in `historicaldocs/design/BRIEF-FOR-GPT-2026-08-08.md:167`;
  it was the answer under the wrong heading.
- **`0x0000` is not empty; `0xFFFF` is** - every u16-id array in this layer.
- **The type argument to `0x56CA70` is a called thunk, not an immediate**; and `0x56C990-0x56CA40`
  is a block of twelve `mov eax, N; ret` thunks with zero references - decoys.
- **Trust `d1-eboot`'s AIM CHECK**: it caught a transposed hex digit that would have read as "the
  RTTI string is referenced by nothing".

---

## 15. Standing [U]

| `[U]` | what would close it |
|---|---|
| **Do the measured indices RENDER?** | the geared runs - `design/geared-visuals-run-1.md` (frontier; the emblem is the falsifier, the only field with a measured slot) |
| The in-world emblem | an emblem family-4/3 element, SOID in `+0x2B80[12]` and in a 340-slot with index 1512 (§4.4); loadout lane (`d1-forge` / `loadouts/*.json`) |
| Element fields `+0x08`, `+0x10`, `+0x40`, `+0x48`, `+0x86` - no found consumer | `d1-live watch` on a live element, or `d1-eboot field-xref` once a holder class is named; current negatives are uncontrolled greps blind to array indexing |
| What `FUN_0026F110` is (infuse? dismantle? comparison?) | `diagnostic-catalogue.py --addr 0x26f110` for the subsystem, then `xref` its callers |
| Whether `+0x58`'s table (accessor slot 156) is the talent grid | dump entry 0's shape from `ps4_investment_globals_012f_3.pkg` and compare with `DestinyTalentGridDefinition` row counts |
| The literal source of "3 Attack" | fix `+0x24` and re-read; if the value becomes real the question dissolves |
| Which slot of `+0x98` is Attack | read `definition+0x68`'s list for content index 2085 offline, or watch `+0x98` after the `+0x24` fix |
| Whether `element+0x86 = 1` survives our encode | live dump showed `+0x28..+0xB8` all zero against `stage_loadout` writing `o[0x086] = 1`; re-stage and re-read |
| Non-emblem well-known slots (`vtbl[0x160]()` block; readers of `+0x11`, `+0x41`) | attribute the reader sites `0x278B1A`, `0xA24A52`, `0xA831F9` |
| Content-index element `+0x08` (zero on all 7,544 rows) | a build where it is non-zero, or a reader |
| `0x1055A30` - the visible iterator's advance, the UI's real visibility rule | read it; it consults `0x25A320()->vtable[0x98]()` and `[obj+0x84]` |
| `0x1055B80`'s `byte @ record+0x12` | identify the record |
| The 64-entry list at profile `+0x3E90` (equipment? quests?) | its u16s also feed `0x555730` |
| `0xABF190`'s `esi` argument (peer index?) | if so, the presence snapshot is local-player-only |
| Whether 2020 manifest hashes resolve on the 2016 client | still open after the `d1guest tag` retraction |
| Whether the client enforces any loadout validation rule | first observed accept/reject re-tags it `[C]` |
| What zeroes the availability cache block | `d1guest watch` bracketing (§13.4) |
| The customization hash at `0x8080181A +0x1C` | same content-table inversion family; belongs to `design/character-and-investment.md` |

---

## 16. Dead claims

Each line: the claim, and what killed it. Never carry these forward as live.

| # | dead claim | killed by |
|---|---|---|
| D1 | `+0x24 = 1` is the single cause of all four gear symptoms | the tab is positional (`0x26DB60`, item not an input) and geometry lives in another object; four independent defects (inventory-instance-fields §0/§7.2) |
| D2 | the inventory tab is derived from the item (a wire bucket field exists) | `FUN_0026DB60(scope, slot_index)` - a 39-record content table; nothing about the item consulted |
| D3 | "Requires Level 953998653" is a mangled Heavy `bucketTypeHash`; alternatively the +8 is "a coincidence" | `FUN_00589900` at `0x9FAF45` returns `element+0x1C` (our 953998645) and the +8 is the stat-curve term; the "coincidence" line in the shared AUGMENTED banner is superseded by the canonical CLOSED §4.3 |
| D4 | `N = tag - 0x80A60297` (the world-tag table is contiguous, "verified 4-for-4"; asserted in five docs) | executing `0x54C090` over all 7,544 indices: 633 discontinuities, 6,750 distinct tags, first break at index 112; Gjallarhorn would compute 15,615, outside the table |
| D5 | tag `0x80A6035A` is index 195 | measured: `(195) = 0x80A60358`, `(197) = 0x80A6035A` |
| D6 | only three element fields (`+0x26`, `+0x38`, `+0x44`) are read by any reachable code | the UI reaches elements via the iterator's own `vfunc[0x98]`, additionally reading `+0x24`, `+0x58`, `+0x98`; the two-site census's premise chose its search space |
| D7 | `C+0x026` is `0xFFFF`-terminated | `0x555730`'s tail is a fixed 8-iteration loop; `0xFFFF` is a per-entry skip - all eight must be set |
| D8 | nothing reads `+0x18`/`+0x1C` (so hashes parked there are harmless) | `FUN_0026F480`+four level gates read `+0x18`; `0x269971` and `FUN_00589900` read `+0x1C` |
| D9 | the descriptor `kind` byte's `0x100` bit marks inline-array elements | counter-example `0x80800BD7` (three `kind 0x1` fields at non-stride offsets); the array marker is structural |
| D10 | bucket ids `0x11`/`0x12`/`0x13` have no bucket-definition row and are unreachable | they are records 15, 16, 23 of the measured 39; non-equippable, not unreachable (`inventory-buckets.tsv`) |
| D11 | write `0xFFFF` at element `+0x44` for "no progression" | `0x10559F0` rejects the whole item on `0xFFFF`; only a real ordinal is right |
| D12 | `d1guest tag <itemHash>` proves 2020 manifest hashes resolve on the 2016 client | it is decomposition arithmetic (`0x4BF4BE3F & 0x1FFF = 7743`, object all zeros); false positive |
| D13 | the profile stat-override array is at `+0x6CE8` with 6 entries; `+0x6D18` a separate bool | schema (`0x80804768` count 7) + reader `0x265EF0` + codec refusal at `+0x6D18`: `+0x6CE0`, 7 entries; `+0x6D18` is the second block's present bool |
| D14 | the in-world Guardian is missing because it has no equippable gear (Hypothesis B) | the spawn aborts two gates upstream; `FUN_0090D1E0` never called; the `gg + 0xD0` framing retracted (=> `design/player-in-world.md`) |
| D15 | the availability builder fails, blanking the director | AVAIL-ITEM-TRACE: AL=1 on both post-boot invocations; the cache is built and then destroyed by something else |
| D16 | the item gate passed 680/681 items | `0x5557AC` is a dedup bitmap; 3 real load-state checks, one distinct item |
| D17 | family 2 carries progressions (231 slots ~ 252 definitions) | family 2 is `{u64, u64, u16[27], u16[5]}` - display name + clan tag, a player lookup |
| D18 | character selection works (message 504 observed) | fired only for sparse 88 B / 87 B objects (2026-08-07); full present-and-zero oracle encodes do not select |
| D19 | the art-arrangement carrier is "an object we have never served" | it is field `+0x0C` of the equipment record in the family-3/1 card already served; the missing thing was one field (=> `design/geared-guardian-fixtures.md`) |
| D20 | leave `+0x88`/`+0x98`/`+0x20` at guessed values | measured 2026-08-11 against package bytes and the client's ctor `FUN_0026E0E0`: `+0x88`/`+0x98` empty, `+0x20` = `*(u16*)(rec30+8)` (Oryx Mask 70, others 0) |

---

## 17. Sources

| doc (now under historicaldocs/design/) | contributed |
|---|---|
| `historicaldocs/design/inventory-instance-fields.md` | §2, §3, §5, §7, §8, §9, §14; the four-defect attribution; the field table; the three-blind-scans record |
| `historicaldocs/design/item-content-indices.md` | §4; the 7,544 x 24 B table, the 14 measured indices, the emblem slot 12, the contiguity refutation, the controls |
| `historicaldocs/design/item-render-path.md` | §1, §6; the registry API, the world-tag loaders, the iterator family, the 340+326=666 confirmation |
| `historicaldocs/design/loadout-framework.md` | §12; crates, schema, Apply contract, manifest traps, era signals |
| `historicaldocs/design/light-and-gear.md` | §10, §11; the override-block correction, level's two mechanisms, family-2 refutation, default gear, the blank-Guardian scope banner |
| `historicaldocs/design/availability-and-item-gate-probes.md` | §13; probe design, builder verdict, the dedup artefact, the destroyed cache |

Live neighbours: `design/geared-guardian-fixtures.md` and `design/geared-visuals-run-1.md` (the
current frontier for this theme); `design/character-and-investment.md` (character screens, cards,
customization); `design/queuez-replication.md` (families, replication states, subscription vs
push); `design/content-and-packages.md` (packages, tags, Oodle extraction, progression tables);
`design/player-in-world.md` (spawn, player record, biped).
