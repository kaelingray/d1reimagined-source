# Content and packages - the `.pkg` estate, and the manifest that gates it

**State.** The `.pkg` container format is fully decoded and executable as the Rust crate
`d1-server/crates/d1-pkg` (the single description of the format; no Kaitai, deliberately).
Registration, name resolution, the CDN-manifest gate and its wire format are all `[C]` and
live-confirmed; the 1033-package manifest `capture-rig/manifests/full_en.bin`, pinned by
`capture-rig/run-boot.sh` as `D1_MANIFEST_FILE`, is what clears the `bap_signin` resourcerer halt
(`failed to load bootstrap_patchable`). Oodle decompression of block payloads is verified.

**Reach.** Everything here is `static-client-inference` against
`dumps/patch-01.29-CUSA00219/eboot.bin`, cross-checked against read-only parses of all **1647**
`.pkg` files in `dumps/patch-01.29-CUSA00219/packages/` (file evidence we own, `observed-capture`
strength), except where a line says otherwise. The live confirmations in §4.8 are from emulator
runs behind our own signon and a client data patch: everything downstream of them is
`synthetic_ancestor`. Confidence tags per `CLAUDE.md`: `[C]` confirmed, `[O]` observed, `[R]`
reported, `[I]` inference, `[U]` unknown.

**Owned media.** This document describes formats, offsets, counts, identifiers and hashes only.
No package content is reproduced.

---

## Contents

1. [Registration - how a logical name becomes a `.pkg`](#1-registration)
2. [The container format](#2-the-container-format)
3. [Content inventories - investment, PvP, the 33 maps](#3-content-inventories)
4. [The `content_manifest_not_provided` gate](#4-the-manifest-gate)
5. [Sidebar: server-message schemas live in content](#5-server-message-schemas)
6. [Open `[U]`s](#6-open-us)
7. [Dead claims](#7-dead-claims)
8. [Sources](#8-sources)

---

## 1. Registration

Provenance: historicaldocs/design/package-registration.md §§0-10.

### 1.1 The resolution mechanism `[C]`

**Resolution is by logical name, over the symbol tables inside registered packages - never by
`Package.id`, never by path prefix, and the manifest is not consulted.** The manifest's only job is
to decide **which `.pkg` files get opened and registered**.

The global-resource name table is a `const char *` array at vaddr **`0x192cb90`** (recovered
through `DT_SCE_RELA` relocations; the slots read as zero in the file image):

| index | name |
| --- | --- |
| 0 | `none` |
| 1 | `owc_client_startup` |
| **2** | **`client_bootstrap_unpatchable`** |
| **3** | **`client_bootstrap_patchable`** |
| **4** | `client_startup_globals` |
| 5 | `client_development_fallback` |
| 6 | `investment_assets` |
| **7** | `investment_globals` |

Four independent confirmations of the indexing, from the `bap_signin` handler at `0x798050` and
its sibling at `0x7a2d40`, where each call site pairs an index with its own error string:

| call site | index | error string it guards |
| --- | --- | --- |
| `0x79820b` (`unload`) | `esi = 2` | `failed to unload bootstrap unpatchable` |
| `0x798289` (`load`) | `esi = 3` | **`failed to load bootstrap_patchable`** |
| `0x79839f` (`load`) | `esi = 7` | `failed to load investment globals` |
| `0x7a2d58` (`load`) | `esi = 2` | `failed to load bootstrap unpatchable` |

`0x164ce3e` is the exact halt string the failing run produced, so that halt is `bap_signin`
sub-state `0x0c`, `global_resource_load(index 3)`. `[C]`

`global_resource_load` (`0x751830`) reads the name from `0x192cb90` and gates on
**`resourcerer_find_symbol(name) != -1`** (`0xcdbb0`); on failure the 12-byte slot
(`{state, handle, tag}`) is set to state 5 (NOT FOUND). On success it queues
`queue_load(job_mgr, 7, 2, 0, desc)` at `0x3491e0` with desc format
`global resource load of '%s'` (`0x163bba0`) and stores the tag at slot `+8`. `[C]`

`resourcerer_find_symbol` (`0xcdbb0` is `0xd89a0(name, NULL)`) walks the linked list of registered
package contexts (head at `0x256abf0`, array descriptor at `0x25693e8`, next-pointer chain via
`ctx + 0x248`) and each package's symbol table (`count` at header `+0xec`, table pointer at
`+0xf0`, 0x44 bytes per entry, name at record `+8`). Consequences: `[C]`

1. Resolution is by name over contents of registered packages.
2. **Ties break on the package file's build timestamp** (header `[0x10:0x14]`, Unix time;
   `[0x14:0x18]` is always zero, so the u64 read at `+0x10` is the timestamp). Newest generation
   wins - this is the patch-override mechanism.
3. Names are compared with a **60-byte (`0x3c`) cap**.

File-side confirmation: parsing `(count @0xec, offset @0xf0, 0x44-byte records, name at +8)` over
all 1647 packages yields **501 packages with symbols, 501/501 parsed cleanly, 1114 distinct
printable names, zero garbage**; in every file the first record's name sits at file offset
`0x1008`. Symbol-table record (0x44 bytes): `{u32 tag_handle; u32 secondary; char name[0x3c];}`.
`[C]`

### 1.2 Where the seven logical names live `[C]`

| logical name | owning package id | files | notes |
| --- | --- | --- | --- |
| `client_bootstrap_patchable` | **`0x0172`** | `ps4_client_bootstrap_patchabl_0172_{0,1,2}.pkg` | tag `0x80ae4000` |
| `client_bootstrap_unpatchable` | `0x0179` | `ps4_client_bootstrap_unp3_0.pkg`, `ps4_client_bootstrap_unpatchable_0.pkg` | engine-registered, not manifest |
| `client_startup_globals` | `0x018d` | `..._018d_{0,1,2,3}.pkg` | |
| `client_development_fallback` | `0x0180` | `..._0180_0.pkg` | |
| `investment_assets` | **`0x0135`** | `ps4_investment_assets_0135_{0,1,2,3}.pkg` | |
| `investment_globals` | **`0x012c`** | `ps4_investment_globals_012c_{0,1,2,3}.pkg` | |
| `owc_client_startup` | - | **none** | not in any package's symbol table `[C]` |

Ids `0x0137, 0x0139, 0x013a ...` are different packages that merely share the
`ps4_investment_assets_` filename prefix. Only `0x0135` carries the `investment_assets` symbol:
**each logical name is owned by exactly one package id; there is no id-selection problem.** `[C]`
(This kills an earlier claim - see §7.)

Filename rule `[C]`: base names (minus `_<gen>.pkg`) are capped at **34 characters** (584 of 795
distinct base names sit exactly at 34); the rule `"ps4_" + logical[:25] + "_" + id4hex`
reconstructs every logical name in the engine's global-resource string run at vaddr `0x163bc13`.

### 1.3 `register_package` (`0xd6330`) `[C]`

Reached only from `0xcf4cc`, the async job dispatched by `register_package_async` (`0xcdc00`).
Signature: `register_package(const char *name /*rdi*/, const uint32 *flags /*rsi*/,
uint16 id /*edx*/, uint64 guid /*rcx*/, void *unused /*r8, always 0*/)`.

**Path construction** - the only place `packages/` appears:

```c
if (strstr(name, ".pkg"))  strlcpy(ctx->path, name, 0x100);
else                       sprintf(ctx->path, "packages/%s.pkg", name);   // 0x16080ae
ctx->install_type = (*flags & 0x40) ? 5 : 12;                             // ctx+0x240
```

`packages/%s.pkg` and `%s_%s.pkg` are the only two pkg-path format strings in the binary; the
latter belongs to `async_write_unpatchable_packages` (`0x798930`), a utility-drive copy to
`unpatchable\packages\`, not a load path. `[C]`

**The flags word** `[C]`:

| bit | effect |
| --- | --- |
| `0x01` | strict id range: `(u16)(id+0x400) in [0x40a, 0x0dff]` (i.e. `0x0a <= id < 0x0a00`, plus a `[0xa00,0xc00)` special case) |
| `0x02` | `[U]` - not read anywhere in `register_package` |
| `0x04` | verify `guid` against header `[0x08]`; mismatch => `-90` |
| `0x08` | verify `id` against header `[0x04]`; mismatch => `-91` |
| `0x20` | `[U]` |
| `0x40` | install type **5** instead of **12** |

Two call sites, two flag values `[C]`:

- **Hard-coded unpatchable set** (`0x799190`, bootflow state 5): `flags = 0x40`,
  `id = 0xffffffff`, `guid = 0` - no id/guid check, install type 5.
- **Manifest-driven set** (`0x79cd20`, `bootflow:package_registration`): `flags = 0x2e`
  (`0x20|0x08|0x04|0x02`) - id and guid both checked, install type 12. The value is
  `content_manifest_required() ? 0x2e : 0x02`, and `0x750ec0` returns a constant `1` (§4.2), so
  it is always `0x2e`.

**Result codes** `[C]` (the mapper `0xcfce0` returns the empty string at `0x129f8b8` in this
build, so `Patchable package registration failed with result %s (%d)` at `0x164d536` prints a
blank name and a raw number):

| value | meaning |
| --- | --- |
| `1` | success |
| `2` | already registered (same id, same generation, same GUID) - benign |
| `-64` (`0xffffffc0`) | same id+generation already registered with a **different GUID** |
| `-84` (`0xffffffac`) | could not open the file / create an accessor |
| `-86` (`0xffffffaa`) | id out of range (flags bit `0x01` path only) |
| `-88` (`0xffffffa8`) | header `[0x13c]` != actual file size |
| `-90` (`0xffffffa6`) | GUID mismatch - `Package.d` != header `[0x08]` |
| `-91` (`0xffffffa5`) | package id mismatch - `Package.c` != header `[0x04]` |
| `-93` (`0xffffffa3`) | secondary-header validation failed, or a same-id generation conflict |
| `-95` (`0xffffffa1`) | header read / size query failed |
| `-65` (`0xffffffbf`) | bad magic, bad version, generation > 0xff, or id >= `0x0c00` |

Binding id constraint with `flags = 0x2e`: **`0x000a <= Package.c <= 0x0bff`** (the handler
pre-screens against a wider `[0x0a, 0x17ff]`, raising `package id %d is out of range, can't
register` at `0x164d5bc`; `register_package` is the tighter of the two). Every id in the estate,
`0x0042` ... `0x0753`, passes. `[C]`

**Localization is a warning, not an error** `[C]`: header `[0x22]`, first non-zero value wins into
`0x256abf8`; a mismatch logs `Package localization mismatch, attempting to register %d package
when %d package is already registered; if you're using both init.txt and content manifest, make
sure the localizations match` and registration proceeds.

**Same-id ordering** `[I]`: `0xd6830` compares generations when a second package with the same id
registers; the branch depends on `r8b` from `0x6fb60(id + 0x400)` (a lookup in already-loaded
content) then `0x6f130`. On a cold boot that lookup returns 0, so the insert path
(`0xd68b0` -> `0xd6a59`) accepts any generation order. **Register generations ascending (on-disk
`_0, _1, _2 ...`); a logged result `-93` is the discriminator that would invert this.** Same id +
same generation + different GUID returns `-64`, which is why a manifest must not list both
`ps4_client_bootstrap_unpatchable_0` and `ps4_client_bootstrap_unp3_0` (id `0x0179`, both
generation 0, different GUIDs). `[C]`

### 1.4 `bootflow:package_registration` (`0x79cd20`) `[C]`

Per manifest `Package` (count from `cdn_manifest_package_count` at `0x2afdb0` ->
`container[0x8210]`; element `i` from `0x2aff00` -> `container + 0x8218 + i*0x118`):

1. If `is_id_of_class(p->f1, 0x10)` (`0x2b0940`: `(f1 & 0xf0000000) == 0xb0000000`), find the
   entry via `0x2b08b0` (scans `entries[]` on `Entry.f1`); if `Entry.f5 == 0` (bool at `+0xc4`,
   confirmed by `cmp byte [rax+0xc4], 0` at `0x79ce33`) and `lookup_object(p->f1) == -1`
   (`0xb9a00`), the package is **skipped as not owned**.
2. Path: `path_set` (`0x884f0`, `'\\'` -> `'/'`, strip leading `/`) then
   `path_format(mode=4)` (`0x88af0`) which yields the **stem only** (component 3 of the four-way
   split at `0x88d30`: dir / last-dir-component / stem / extension). So
   `"ps4_client_bootstrap_patchabl_0172_2"` and
   `"packages/ps4_client_bootstrap_patchabl_0172_2.pkg"` both open the same file; any `.` in the
   name would truncate it, and no package filename in the estate contains one. `[C]`
3. Range pre-screen on `f3`, then `register_and_track(this, name, &{0x2e}, (int16)p->f3, p->f4)`
   (`0x789790` -> `0xcdc00`); substate 1, then poll `0xcdbe0` until `!= 3`.

**Descriptor cross-check** (descriptor `0x15abdb0`, the `Package` sub-message of `0x18f9490`):

| field | descriptor says | code reads |
| --- | --- | --- |
| f1 `required uint32` | data `+0x00` | `mov edi, [r14]` at `0x79ce1a` |
| f2 `required string[257]` | data `+0x04` | `lea rsi, [r14+4]` at `0x79ce60` |
| f3 `optional uint32` | flag `+0x105`, data `+0x108` | `cmp byte [r14+0x105], 0` / `mov r15d, [r14+0x108]` at `0x79ce85`, `0x79ce8c` |
| f4 `optional uint64` | flag `+0x10c`, data `+0x110` | `mov r8, [r14+0x110]` at `0x79ceca` |
| element size | end `0x8330` - base `0x8218` = `0x118` | `imul rax, rax, 0x118` at `0x2aff09` |

**Field semantics, settled** `[C]`:

```proto
message Package {
  required uint32 offer_id    = 1;  // TAGGED entitlement id, NOT the package id
  required string path        = 2;  // only the STEM is used -> packages/<stem>.pkg
  optional uint32 package_id  = 3;  // the 16-bit resourcerer id; must equal header[0x04]
  optional uint64 guid        = 4;  // must equal header[0x08]
}
```

- f3/f4 are optional in the schema but **mandatory in practice** with `flags = 0x2e`: the parser's
  clear pass zeroes them, so an absent f3 yields id 0 (=> `-91`) and an absent f4 yields GUID 0
  (=> `-90`). `[C]`
- f3 is consumed as `movsx ecx, r15w` and compared 16-bit; only the low 16 bits matter. `[C]`
- The on-disk header `[0x04]` id equals the 4-hex id in the filename in **1641/1641** files that
  have one, so `Package.c` is **observable, not invented**. `[C]`

**Tagged-id classes** (`0x2b0940(id, mask)`): mask bit `0x01` tests `id == 0`; `0x02` tests
`(id & 0xf0000000) == 0xd0000000`; `0x04` `0xe...`; `0x08` `0xc...`; `0x10` `0xb...`; `0x20`
`0xa...`. `package_registration` tests only class `0xb`. A second consumer, `0x1e3b90` (DLC/store
module), enumerates class `0xd` packages and hashes `Package.path` into a `0x52d`-bucket table;
`0x52d = 1325` is also the `packages` descriptor capacity - an independent confirmation of the
cap. **A `Package.id` of 0 belongs to no class and is skipped by both consumers: the safe value
for a synthetic manifest.** `[C]`

### 1.5 Where `ps4_` comes from `[C]`

`ps4` is a literal at `0x1608b36`, referenced exactly twice, and **neither reference is on the
manifest path**:

1. `0x7991a1`, bootflow state 5: `snprintf(plat, 0x100, "ps4")`, then for each of three names from
   the table at `0x192dd60` (`client_bootstrap_unp3_0`, `ui_startup_unp3_0`,
   `ui_bootflow_unp3_0`), `sprintf(name, "%s_%s", plat, entry)`, then
   `register_package_async(name, &{0x40}, 0xffffffff, 0)`. These are the unpatchable packages that
   load without a manifest - why the client boots at all with a `2a 00` manifest.
2. `0x798967`, `async_write_unpatchable_packages` - the utility-drive copy, same three names.

**For manifest-registered packages the `ps4_` prefix must be in the manifest string.** `[C]`

### 1.6 Is a real manifest on disk? No. `[C]`

Three negative checks:

1. **Filesystem.** The only non-`.pkg` files in `dumps/patch-01.29-CUSA00219/` are `eboot.bin`,
   `sku_config.txt`, `sce_discmap*.plt`, `sce_module/`, `sce_sys/`, `_console_metadata/` and the
   dump's own manifests. No manifest, no `init.txt`, no package list. `[C]`
2. **Content.** `grep -l -a -F 'ps4_client_bootstrap_patchabl' *.pkg` over all 1647 files / 47 GB
   returned no matches; a serialized `CdnContentManifest` necessarily contains its packages'
   on-disk stems as plaintext protobuf strings. `[C]` Limit: rules out a plaintext copy only; a
   compressed/encoded one would be missed. `[I]`
3. **`ps4_shared_manifest_0430/0431/0432`** are ordinary resourcerer packages (magic `0x18`,
   version 7, ids `0x0430`-`0x0432`, generations 0-3, all localization 0) with zero symbol-table
   entries; the name refers to Destiny's in-game shared-manifest content, not the CDN manifest
   (see §3.2). `[C]`

Related negatives: `km_cache.txt` / `km_cdn.txt` are deleted, not read (§4.7). `init.txt` is the
default of an engine-init options struct (`0x5913f0` stores `"init.txt"` into `opts+0x10`),
selected by the `/init:` switch (`0x16008f2`), reachable only from a `/host/` file share - a
dev/QA script absent from retail. `[I]` The crash report's `running from file share: true` comes
from `0x877d0` testing a `/host/` mount, very likely stubbed under shadPS4 - an emulator artifact,
do not build on it. `[I]` `install type: 5` in the crash report is a literal written at
`0x1e39c1`-`0x1e39f3` when the content module's state is 0 - it restates "no content registered",
not the §1.3 per-path install type. `[C]` on the literal, `[I]` on the reading.
`sce_discmap.plt` / `sce_discmap_patch.plt` (both exactly 3,288,792 bytes, magic `PLT `
`50 4c 54 20`, version 2) are dense binary sector tables with **zero** printable package strings
(`ps4_` 0, `.pkg` 0, `packages` 0, `client_bootstrap` 0) - not a name/id index. `[C]`

---

## 2. The container format

Provenance: historicaldocs/design/package-registration.md §2;
historicaldocs/design/package-container-pvp-content.md §§1-6, 20-23.

### 2.1 The header (0x140 bytes at file offset 0) `[C]`

`register_package` reads `0x140` bytes at offset 0 via `0x8ba70(acc, 0, 0x140, &ctx)` at
`0xd64f9`, so header offset = context offset = file offset.

| off | size | field | evidence |
| --- | --- | --- | --- |
| `0x00` | u16 | **magic `0x0018`** | `cmp word [ctx], 0x18` at `0xd6565`; 1647/1647 files |
| `0x02` | u16 | **format version 7** | compared to `0x3b180()` at `0xd654d`; 1647/1647 |
| `0x04` | u16 | **package id** | `0xd656f`; matches the 4-hex filename id in 1641/1641 |
| `0x06` | u16 | type/flag | `1` in 1636 files, `0` in 11. `[U]` |
| `0x08` | u64 | **GUID** | compared to manifest `Package.d` at `0xd667e` |
| `0x10` | u32 | **build timestamp (Unix)** | the `find_symbol` tie-break; 41345 -> 2014-07-01, 46419 -> 2015-08-18, 51854 -> 2016-08-20, 52553 -> 2016-10-01 |
| `0x14` | u32 | 0 (always) | 1647/1647 - why `+0x10` reads safely as u64 |
| `0x18` | u32 | **content build number** | matches the build string; {41345, 46419, 51854, 52553} |
| `0x1c` | u32 | **content revision** | 41345 -> 3, 46419 -> 3, 51854 -> 0, **52553 -> 6**; the client reported `content_build 52553, content_revision 6` `[O]` |
| `0x20` | u16 | **generation** (`_N` suffix) | equals the filename's trailing `_N` in 1647/1647; `> 0xff` rejected at `0xd664e` |
| `0x22` | u16 | **localization id** | read at `0xd670f`; clean partition (below) |
| `0x24` | char[0x80] | build string | e.g. `evil tool_lib test pc_x64 52553.16.09.28.1551.evil 16.09.28 1551` |
| `0xa4` | u32 | **`[U]` bitmask** | 28 distinct values, bit 0 never set, <= 12 bits used (§2.7) |
| `0xa8` | u32 | **`[U]` hash**, pure function of `(content_build, [0xa4])` | 84 distinct `(build,rev,a4)` triples, 84 distinct `a8` |
| `0xac` | u32 | constant `2` | 1647/1647 |
| `0xb0` | u32 | **signature file offset**, constant `0x800` | `mov esi,[ctx+0xb0]` -> `0x8ba70(acc, [0xb0], 0x100, buf)` at `0xd6575`-`0xd6590` |
| `0xb4` | u32 | **entry count** (1 ... `0x2000`) | `0xd65b0`: `0` or `> 0x2000` => result `-65` |
| `0xb8` | u32 | **entry-table file offset** | `0xd7c3a`; `...%d bytes of entry table at offset %d...` |
| `0xbc` | u8[20] | **entry-table SHA-1** | `0xd7e9d`: `lea rdx,[ctx+0xbc]` into the validator, guarded by `Failed to validate entry table hash` |
| `0xd0` | u32 | **block count** | `0xd7c6d`: `mov r14d,[ctx+0xd0]; shl r14d,5` |
| `0xd4` | u32 | **block-table file offset** | `0xd7dac`; `...%d bytes of block table at offset %d...` |
| `0xd8` | u8[20] | **block-table SHA-1** | guarded by `Failed to validate block table hash` |
| `0xec` | u32 | **symbol-table entry count** | validated on all 1647 files |
| `0xf0` | u32 | **symbol-table file offset** (`0x1000` or 0) | validated on all 1647 files |
| `0xf4` | u8[20] | **symbol-table SHA-1** | `Failed to validate symbol table hash` |
| `0x108`-`0x138` | | zero | 1647/1647 |
| `0x13c` | u32 | **total file size** | `cmp [ctx+0x13c], actual_size` at `0xd6643`; matches `stat()` in 1647/1647 |

The three table descriptors are a uniform `{u32 count; u32 offset; u8 sha1[20]}` at stride
`0x1c`, at `0xb4`, `0xd0`, `0xec`, ending exactly at `0x108`. Record sizes come from the
disassembly: `shl esi, 4` at `0xd7ead` (entry record = 0x10) and `shl r14d, 5` at `0xd7c75`
(block record = 0x20). `[C]`

**Localization map** - `[0x22]` against the language token in the filename is a clean partition
with zero exceptions over 1647 files `[C]`:

| `[0x22]` | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| token | *(neutral)* | `en` | `fr` | `it` | `de` | `sp` | `jpn` | `pt` |
| files | 938 | 101 | 102 | 102 | 100 | 100 | 102 | 102 |

Generation `_N` is a **patch layer**, not a variant: package `0x0172` is `_0` at build 41345, `_1`
at 46419, `_2` at 51854. The newest generation is what `find_symbol` prefers; older ones supply
the tags that generation did not touch. `[C]` (The earlier "four content builds = four variants"
reading is dead - §7.)

### 2.2 The header is RSA-2048 signed `[C]`

`0xd65a7` calls `0xd9370(header_buf, sig_buf)` where `sig_buf` is `0x100` bytes read from file
offset `[0xb0]` (`= 0x800`). `0xd9370` tries each key in the table at `0xeee00`/`0xeee10`;
`0xd9420` runs digest init/update/final (`0xfa45d0` / `0xfa4650` over exactly `0x140` bytes /
`0xfa4770`, 0x20-byte output = SHA-256), then `0xfaa000(sig, 0x100, digest, 0x20, 3, key)` -
RSA verify, 2048-bit.

**`[0x800:0x900]` is an RSA-2048 signature over SHA-256 of `header[0x000:0x140]`.** The three
table SHA-1s live inside the signed header, so the whole table structure is signature-covered: we
can read packages freely, we cannot fabricate one. The modulus table is at `0x19f9060`; the RSA
was not evaluated. `[C]` on the structure; this also explains result code `-93`
(secondary-header validation).

### 2.3 On-disk layout `[C]`

```
0x0000                header (0x140 bytes, signed)
0x0800                RSA signature (0x100 bytes)
0x1000                symbol table   n_sym x 0x44
0x1000+RU(n_sym*0x44) entry  table   n_ent x 0x10      == header[0xb8]
  + RU(n_ent*0x10)    block  table   n_blk x 0x20      == header[0xd4]
  + RU(n_blk*0x20)    block payload bytes ... EOF      == header[0x13c]
                      RU(x) = round up to 0x800
```

`header[0xb8] == 0x1000 + RU(n_sym * 0x44)` in 1647/1647 files;
`header[0xd4] == header[0xb8] + RU(n_ent * 16)` in 1647/1647 - and 16 is the only record size in
1...256 for which that holds near universally. `ps4_orbit_01a9_3.pkg`'s block table ends at
`0x7b800`, exactly the file size - an exact-termination case pinning the 32-byte block record.

### 2.4 Entry record (16 bytes) and block record (32 bytes) `[C]`

```c
struct entry {            // 0x10 bytes; index i == the tag's low 13 bits
    uint32 class_tag;     // +0x00  tag of this entry's class/type (see §3.3)
    uint32 u04;           // +0x04  [U] flags/kind - do NOT read as "alignment" (§2.7)
    uint64 loc;           // +0x08  packed:
                          //          bits  0..13  block index   (14 b)
                          //          bits 14..27  offset in the block, in 16-byte units
                          //          bits 28..57  size in bytes (30 b)
                          //          bits 58..63  always 0 in 4 713 258 samples
};
```

The bit split is read off the client's accessors: `movabs rsi, 0x3ffffff0000000` at `0xd865f`
(the "has data" test on bits 28..57), `mov ecx, 0x3fff` at `0xd87e7` (block index), `shl rsi, 5`
at `0xd87ef` (x 0x20 into the block table). The x16 on the offset field is forced by the corpus:
entries in one block sit at field values `0, 4, 9, 96, 100, 104, 108 ...` with byte sizes
`56, 76, 48, 54, 54, 54, 54` - contiguous 16-byte-aligned packing only under the x16 reading.
`size == 0` means this package does not supply the entry.

```c
struct block {            // 0x20 bytes
    uint32 file_offset;   // +0x00  byte offset inside the OWNING generation's .pkg
    uint32 stored_size;   // +0x04  bytes as stored
    uint16 gen;           // +0x08  generation that physically holds the bytes; compared against
                          //        the package header's [0x20] at 0xd87f8
    uint16 flags;         // +0x0a  bit 0 = Oodle-compressed. Only 0 and 1 observed.
    uint8  sha1[20];      // +0x0c  SHA-1 of the STORED bytes (see §2.6)
};
```

Logical block size is **`0x40000` (256 KB)**; 20 515 of 20 759 sampled raw blocks are exactly
that, the remaining 244 are shorter tails. Uncompressed stored sizes reach exactly `0x40000` and
never exceed it; compressed ones never reach it (corpus-wide, `d1-pkg stats`). Every compressed
block sampled begins with byte `0xb7` (687/687); `[C]` on the constant, `[U]` on what it encodes.
**A decompressed block is at most `0x40000`, not always exactly** - `ps4_globals_pvp_0169_3`
block 561 consumes all 65479 stored bytes at 114688 plain bytes; a decode loop that terminates on
output size then reads unmapped memory and looks exactly like a decoder defect. Terminate on
input exhaustion. `[C]`

### 2.5 The tag `[C]`

`0xd8590(tag, out_entry_ptr)` resolves a tag:
`sel = (tag >> 13) & (tag < 0 ? 0x3fff : 0x3ff)` (`0xd85a5`-`0xd85b7`), `slot = k_pkg_slot[sel]`
(u16 table at `0x25692f0`, `0x4000` entries, `0xffff` = unregistered), `idx = tag & 0x1fff`
(`0xd85f9`), bounds-checked against header `[0xb4]` at `0xd861a`, record at
`entry_table + idx * 0x10` (`shl rcx, 4` at `0xd8651`), and "has data" is
`(entry->loc & 0x3ffffff0000000) != 0`. `sel` is **`package_id + 0x400`**:

```python
def make_tag(pkg_id, idx):  return 0x80000000 | (((pkg_id + 0x400) & 0x3FFF) << 13) | (idx & 0x1FFF)
def tag_pkg(tag):           return ((tag >> 13) & 0x3FFF) - 0x400
def tag_idx(tag):           return tag & 0x1FFF
```

Three independent supports: `register_package` computes `id + 0x400` (`0xd6685`); over the 3242
symbol records in the estate, `((tag >> 13) & 0x3fff) - 0x400` equals the containing package's
header id in 3232 cases (99.7%; the 10 exceptions are genuine cross-package symbols, e.g.
`ps4_hiveship_activities_activ_0538` names `mission_rook` with a tag belonging to package
`0x53b`); and the 13-bit index caps entry counts at `0x2000`, exactly the bound `register_package`
enforces at `0xd662d`.

**Locale consequence** `[C]`: the tag carries only `(package_id, entry_index)` - no locale field.
All seven locale variants of id `0x016b` have the same 1724 entries and the same non-empty index
set, differing only in bytes; 66 of 396 package ids exist in more than one locale. A locale
variant is a byte-level substitution inside one tag space, so **only one locale of a given id can
be registered at once** - the mechanical reason behind the "neutral + one language" manifest
(§4.9).

Reading an entry: concatenate **consecutive** blocks starting at `entry.block`, resolving each
block's bytes from the file whose header `[0x20]` equals `block.gen` in the same
`(package_id, locale)` chain, SHA-1-checking stored bytes, Oodle-decompressing where
`flags & 1`, then slicing `[block_off : block_off + size]`. 5.2% of entries span more than one
block. `[C]`

### 2.6 Verification, generations, and the Rust tool `[C]`

**Independent cross-checks** (ascending difficulty of passing by accident):

| # | Check | Result |
| --- | --- | --- |
| 1 | Layout arithmetic vs disassembly (`off_entry`, `off_block` formulas of §2.3) | 1647/1647 |
| 2 | SHA-1 of each reconstructed table equals the header digest | entry 1647/1647, block 1647/1647, symbol 501/501 |
| 3 | SHA-1 of each reconstructed block equals its block-record digest, reading from the predicted owning generation's file | 400/400 on `ps4_orbit_01a9_3` across three files; precondition on every extraction, never failed |
| 4 | Extracted raw entries produce an expected foreign magic (`RIFF ... WAVE`) | yes |
| 5 | RIFF `size` inside the payload equals the entry-table `size` field | 65/65 on multi-block entries; 1202/1202 over every readable entry of `ps4_globals_pvp_016b_en_3` |
| 6 | `shared_manifest` payload self-description: record `[0:8]` == entry `size`, `[8:12]` == the tag computed from `(package_id, entry_index)` | 22 479/22 479 |
| 7 | Generation prediction estate-wide: no block record names a generation newer than its own file or absent from the chain | 0 / 700 045 violations across 792 chains |

**The block SHA-1 covers the STORED bytes** - measured 340/340 on
`ps4_investment_globals_012c_1` and corpus-wide via `d1-pkg verify --blocks`. Consequence: the
container stores **no digest of decompressed output**, so decompressed blocks are verified by an
independent route: most payload classes open with a u64 length that the entry table independently
declares. Excluding the two populations that legitimately carry no length header (raw blobs,
`class_tag == 0xffffffff` or pointed at by a same-package record; and self-classed entries whose
`class_tag` names an entry in the same package - an exclusion computed from the entry table
alone, never from decompressed bytes), the result is **473/473 length-headered entries agree,
over 89 blocks and 4 packages. 100%.** `[C]`

**Generations** `[C]`: every generation ships a **complete** entry table and block table (tables
are restated, not deltas); only payload bytes are deduplicated across generations.
`ps4_orbit_01a9_3.pkg` is 494 KB and owns zero blocks - all 14 888 of its block records point
into generations 0/1/2 (2565 / 8186 / 4137). Measured deltas:

| chain | gen -> gen | entries identical | changed | added |
| --- | --- | --- | --- | --- |
| `0x1a9` | 0 -> 1 | 408 | 814 | +124 |
| `0x1a9` | 1 -> 2 | 536 | 810 | +29 |
| `0x1a9` | 2 -> 3 | 1374 | 1 | 0 |
| `0x12c` | 1 -> 2 | 792 | 7400 | 0 |
| `0x169` | 2 -> 3 | 2992 | 2 | +287 |

**Registering only the newest generation is broken by construction** - its block table
dereferences into older generation files. This is the mechanical proof of "one manifest `Package`
per `.pkg` file, per generation" (§4.9). Counts are **not** monotonic (71 chains shrink; patches
can remove content). Chains must be keyed `(package_id, locale)`, and the three engine-registered
unpatchable packages ship twice with the same id and generation
(`ps4_client_bootstrap_unp3_0` / `ps4_client_bootstrap_unpatchable_0`, id `0x179`; likewise
`0x42`, `0x49`), so `(id, loc, gen)` is not unique for those six files - handle them explicitly.

**The format is executable**: `d1-server/crates/d1-pkg` is the single description (deliberately
no Kaitai `.ksy` - reading an entry requires resolving the generation chain, an algorithm, not a
layout):

```
d1-pkg info <pkg>                                  header + table integrity
d1-pkg blocks <pkg> [--compressed-only] [--json]   block table, each stored SHA-1 checked
d1-pkg entries <pkg> [--block N] [--json]          entry table, unpacked loc fields
d1-pkg extract-block <pkg> <index> -o f            stored bytes, chain-resolved, SHA-1 checked
d1-pkg verify-plain <pkg> <index> --plain f        cross-check a DECOMPRESSED block
d1-pkg verify [--blocks]                           table hashes + layout across the estate
d1-pkg stats                                       corpus-wide counts
```

8 unit tests; estate-wide ones skip cleanly when `dumps/` is absent. `d1-pkg verify` passes on
every package in the estate for table hashes and §2.3's layout arithmetic.

**Oodle.** 532 600 of 701 164 blocks (76.0%) are compressed with **Oodle LZH / LZHLW / LZNIB era**
(Oodle 1.x, pre-Kraken), per the eboot's own assert strings
(`0x016b3a07` OodleLZCompressors.cpp, `0x016b3a81` `OodleLZDecoder_DecodeSome`, `0x016b3c7a`
the LZH/LZHLW compressor assert, `0x016b6dbf` lznib.cpp). `OodleLZDecoder_DecodeSome` is at vaddr
**`0x12672e0`**; its only callers are inside the streaming wrapper at **`0x6c050`** (`0x6c0f3`,
`0x6c249`). No public decoder covers this era (the open `ooz` work covers
Kraken/Mermaid/Selkie/Leviathan/LZNA/BitKnit only) - the route is the client's own decoder.
Earlier sections of the source doc treated Oodle as an unclimbed wall (readable share then:
530 938 entries / 11.3%, 39.4 GB / 23.7% of 165.9 GB logical); the later addendum records
**decompression achieved and verified** (the 473/473 result above, and the `0x40000`-tail lesson
of §2.4). `[C]`

### 2.7 Honest unknowns in the container `[U]`

- **Header `0xa4`**: 28 values across the estate (`4, 12, 14, 78, 132, 134, 142, 398, 518, 526,
  646, 654, 686, 782, 910, 942, 1006, 1966, 2126, 2702, 2734, 2756, 2766, 2798, 2958, 2990, 3054,
  4014`) - clearly a bitmask (bit 0 never set, bit 2 always set, <= 12 bits used). **Header
  `0xa8`**: a 32-bit hash that is a pure function of `(content_build, 0xa4)` (84 groups, 84
  distinct values). Best reading: (schema-group mask, schema hash) `[I]`; **no consumer found**
  anywhere in the 26 MB text segment. `[U]`
- **Entry `u04`**: observed values over 4.7 M entries: `0x10` (2 304 172), `0x20` (786 580),
  `0x1` (663 443), `0x208` (455 476), `0x200` (118 519), `0x100` (101 874), `0x41` (79 997),
  `0x5` (78 901), `0x80` (69 918), plus `0x3`, `0x0`, `0x308`; upper 16 bits take 283 distinct
  values. `0x1/0x3/0x5/0x41/0x208/0x308` rule out a pure alignment field. Left `[U]` deliberately
  rather than named on a plausible subset.
- Block `flags` bits beyond bit 0: only values 0 and 1 occur in 701 164 records. `[U]`
- Header `0x06` type/flag word (`1` x1636, `0` x11). `[U]`
- Install type 5 vs 12 - which mount/accessor each selects (`/app0/` and `/download0/` are
  adjacent candidates in the well-known-path table; `0x8ac00`/`0x8ac30` untraced). `[U]`
- Registration flags bits `0x02` and `0x20` (set by the manifest path, read nowhere). `[U]`
- Package ids 0 and 1 are absent from disk in both dumps (min id `0x000a`, `ps4_ui_core_000a`)
  yet receive 2.3 M `class_tag` references; how tags into that space resolve at runtime is
  untraced (read the writers of `k_pkg_slot` at `0x25692f0`). `[U]`

---

## 3. Content inventories

Provenance: historicaldocs/design/package-container-pvp-content.md §§6-9.

**Reach note.** Nothing in this section depends on our synthetic signon or authored manifest, so
nothing here inherits `synthetic_ancestor` - it is one of the few parts of the project whose
evidence is not downstream of bytes we invented.

### 3.1 The corpus, enumerated `[C]`

1647 files, **4 713 258 entries** (4 700 085 non-empty), **701 164 block records**, **3242 symbol
records / 1114 distinct names**. Per entry the enumerator emits: package file / id / GUID /
generation / timestamp / locale, entry index, computed tag, `class_tag`, `u04`, block index,
in-block offset, size, resolved block span, owning generation, physical byte range in the owning
file, and the SHA-1 of every block touched.

### 3.2 `ps4_shared_manifest_0430/0431/0432` - the readable investment index `[C]`

Zero-symbol packages whose contribution is the cross-package index over investment content:

| id | newest | entries | blocks | compressed blocks | chain bytes |
| --- | --- | --- | --- | --- | --- |
| `0x0430` | `_3` | 8192 | 47 | **0** | 12.6 MB |
| `0x0431` | `_3` | 8192 | 112 | **0** | 33.8 MB |
| `0x0432` | `_3` | 6095 | 85 | **0** | 29.4 MB |

**22 479 records, none compressed.** Every record begins:

```c
struct shared_manifest_record {
    uint64 size;        // == the entry-table size field         [verified 22479/22479]
    uint32 self_tag;    // == make_tag(pkg_id, entry_index)      [verified 22479/22479]
    uint32 class_tag;   // 0x80803463 (46%), 0x80802797 (35%), 0x808008ce, ...
    uint32 target_tag;  // +0x10 - a tag in ANOTHER package
    ...                 // inline arrays shaped {u64 count; u32 element_class_tag; u32 0; elems...}  [I]
};
```

Top `target_tag` destinations: `0x0430` -> `investment_globals_012c` (3313),
`investment_assets_0139` (2786), `client_development_fallba_0180` (619),
`investment_assets_013a` (431), `hiveship_activities_activ_0538` (67); `0x0431` ->
`investment_globals_012e` (2502), `investment_assets_0137` (2226), `investment_assets_0135`
(769), `investment_assets_013b` (482), `venus_activities_activity_03f0` (192),
`cosmo_activities_activity_025c` (99); `0x0432` -> `investment_globals_012f` (1533),
`investment_assets_0141` (460), `investment_assets_013d` (447), `investment_assets_0142` (408),
`mars_activities_activityo_0284` (174), `cosmo_activities_activity_025c` (124).

A symbol-only reduction of the estate would have dropped all three files (0 symbols each) and
lost every entry's class reference and the investment index.

### 3.3 `class_tag`, estate-wide `[C]` (dual use `[O]`)

Of 4 713 258 values: package id 0 (`0x80800000`-`0x80801fff`) 2 220 918 (engine-internal class
registry - no package with id 0 or 1 exists on disk; `register_package` reserves ids below
`0x0a`); `0xffffffff` 898 616 (null); **bit 31 clear** 514 961 (**not a tag** - a 32-bit hash,
confirmed on audio entries in `globals_pvp_016b` whose `class_tag` is a per-asset hash); package
id 1: 83 254; `shared_manifest` `0x430/0x431/0x432`: 69 912; other on-disk ids ~930 000 (937 085
resolve to an in-range entry index in an existing package; 709 do not). So `class_tag` is usually
the entry's class and sometimes a bare 32-bit identifier; the rule selecting between them is not
established.

### 3.4 Investment family census `[C]`

Newest generation per id; "chain" = summed bytes of all generations, which is what a manifest
must register.

| id | file | gens | entries | blocks | raw B | comp B | chain |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `012c` | `investment_globals_012c` **(carries the `investment_globals` symbol)** | 0-3 | 8192 | 425 | 1.3 M | 18.9 M | 23.8 M |
| `012e` | `investment_globals_012e` | 0-2 | 8192 | 515 | 0 | 46.4 M | 56.7 M |
| `012f` | `investment_globals_012f` | 0-3 | 8192 | 1088 | 0.6 M | 110.6 M | 130.0 M |
| `0130` | `investment_globals_0130` | 1-3 | 8192 | 47 | 1.8 M | 2.2 M | 4.8 M |
| `0131` | `investment_globals_0131` | 1-3 | 8192 | 55 | 0 | 2.8 M | 3.1 M |
| `0132` | `investment_globals_0132` | 2-3 | 4650 | 60 | 0 | 1.3 M | 1.6 M |
| `0135` | `investment_assets_0135` **(carries `investment_assets`)** | 0-3 | 8192 | 180 | 0.7 M | 13.2 M | 19.8 M |
| `0137`-`0145` | `investment_assets_*` (9 more ids) | | 8192 each | up to 5035 | | | up to 791 M (`0140`) |

`investment_globals` totals **45 610 entries** over 6 ids; `investment_assets` **~120 000
entries** over 11 ids (`0x135, 0x137, 0x139-0x145`). Only `0x012c` and `0x0135` carry symbols -
one each. Everything else is reachable **only by tag**, which is why the whole family must be
registered even though nothing names it.

Adjacent and almost certainly required with it (gear content is the topic of
`design/items-and-gear.md`): `gear_player` (9 ids `0x00fc`-`0x0105`, 69 409 entries - plausibly
the class/race/gender armour split `[I]`; `0x0105` alone is 606 MB), `gear_weapons`
(`0x011c/011e/011f`, 16 525), `gear_ships` (`0x0114`, 1825), `globals` (`0x0151`-`0x0158`,
44 925), `client_startup_globals` (`0x018d`, 1061), `client_bootstrap_patchable` (`0x0172`,
1062), `ui_pvp` `0x0034` / `ui_pve` `0x002d` (15 entries each - tiny, single-block, compressed).

### 3.5 `globals_pvp` `[C]`

Four ids, 41 files, two roles:

| id | locale | files | newest entries | blocks | raw / comp | chain bytes | content |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `0x0169` | neutral | 4 | 3281 | 998 | 277 / 721 | 232 MB | the neutral PvP definition + asset package |
| `0x016b` | x7 languages | 28 | 1724 | ~104 | ~100 / 4 | 26 MB each | PvP announcer VO + localized text |
| `0x0735` | neutral | 2 | 2664 | 349 | 103 / 246 | 67 MB | Rise-of-Iron (`com01`) neutral PvP |
| `0x0737` | x7 languages | 7 | **1** | 1 | 0 / 1 | 10 KB | RoI localized stub - a single 24-byte entry |

`0x016b` is 96% raw: **1202 of 1724 entries extract, all 1202 `RIFF/WAVE`** with declared length
exactly equal to the entry-table size field - the Crucible announcer and PvP voice-over, one
asset per entry, `class_tag` a per-asset hash. The remaining 248 entries carry
`class_tag = 0x80800714`, 24 bytes each, in compressed blocks - the metadata/string records that
would name the VO lines. The neutral `0x0169` has 784 distinct class tags over 3281 entries (top
five: `0x80801ad7` x798, `0xffffffff` x479, `0x8080080a` x336, `0x80800861` x251, `0x80800734`
x106); its definition records live in the 721 compressed blocks (132 MB).

**`globals_pvp` = one neutral definition/asset package per era, plus a per-language voice-over
and string companion. Mode rules are content records in the neutral package.** `[C]` on the
partition; `[U]` on the rule contents.

### 3.6 The 33 PvP maps `[C]`

Provenance: **historicaldocs/design/package-container-pvp-content.md §7.5** (cited elsewhere by
that exact section number).

Every Crucible map is its own destination triple - `ps4_pvp_<map>_destination`,
`ps4_pvp_<map>_activities_activityonly`, `ps4_pvp_<map>_ambient_activityonly` - and each names
itself in its symbol table:

```
pvp_<map>                       pvp_<map>:scenario_client
pvp_<map>:same_dest_filler      pvp_<map>:spaceflight_filler   pvp_<map>:spaceflight_out
ambient_pvp_<map>               ambient_pvp_<map>:scenario_client
map:pvp_<map>:<8 hex>:root      map:pvp_<map>:root             map:pvp_<map>:unused
```

**33 distinct `map:pvp_<slug>:root` symbols and 34 distinct `pvp_<slug>` activity symbols.** The
slugs: `anomaly, arena_hive, breach, cabal_camp, city_battleground, city_defense, colony_ship,
cosmobattle, den, dos, eq, factory, fast_lane, greenhouse, hive_cell, icarus, ketch, longshot,
marsbattle, moonbase, oasis, outskirts, refinery, rift, rooftops, srl_mars01, srl_venus01,
subway, venus_hcc, vertigo, vex_tube, well, wilderness_town`.

- `ps4_pvp_city_defense_activiti` uniquely carries a second activity,
  `pvp_city_defense_carbon` - a variant of the same map; hence 34 activity symbols against 33
  maps.
- `ps4_pvp_tomb_destination` names `map:pvp_fast_lane:*`, not `map:pvp_tomb:*` - the package
  slug, the activity symbol (`pvp_tomb`) and the internal map name (`pvp_fast_lane`) disagree: a
  concrete counterexample to "the package name identifies the content." `[O]`
- The 8-hex token in `map:<slug>:<hex>:root` is not per-map (`ab5a0aa2` appears for `icarus`,
  `rift`, `srl_mars01`, `srl_venus01`, `subway`, `tomb`, `vertigo`, `well`) - a shared
  variant/bubble hash, not a map identifier. `[O]`
- **No map package contains a mode.** Map activities are mode-agnostic; map x mode is an
  investment object. (Launching a map is a separate seam - `design/activity-launch.md`.)

⚠ Internal inconsistency, unresolved in the source: the same document later refers twice to "the
28 maps" / "28 map slugs" (its §8.4 and §9) against the 33 slugs actually listed and counted
above. No banner resolves it; the enumerated list of 33 is the primary datum, the "28" references
are carried here as an unexplained discrepancy. `[U]`

### 3.7 Elimination

**Confirmed** `[C]`: the mode exists in this build. The eboot carries six PvP HUD objective-data
classes: `c_thud_arsenal_objective_data`, `c_thud_control_objective_data`,
`c_thud_elimination_objective_data`, `c_thud_racing_objective_data`, `c_thud_rift_objective_data`,
`c_thud_salvage_objective_data` (Elimination's `c_properties_accessor_base<...>` at
`0x1666b40` / `0x1666b70`). A HUD objective class with a properties accessor exists to be fed
from content. Also present: the private-match lobby is data-driven
(`c_private_lobby_option_bucket`, `c_private_lobby_option_value`,
`c_private_lobby_options_component`, `c_private_lobby_options_select_handler`) - option buckets
and values are content, not code.

**Searched and empty**: `elimination` in all 3242 symbol records (0), as plaintext in all 11
`globals_pvp` files (0), in the eboot outside the two RTTI strings (0); PvP mode words
(`salvage`, `rumble`, `skirmish`, `clash`, `doubles`, `supremacy`, `inferno`, `mayhem`,
`combined_arms`, `trials_of_osiris`, `iron_banner`) hit only `control` x11 and `rumble` x1 in
unrelated contexts; rule vocabulary (`round`, `revive`, `lives`, `sudden_death`, `overtime`,
`team_size`) hit nothing mode-specific - only `join_in_progress_weight`,
`activity_join_in_progress`, `activity_rejoin_in_progress`, `pick_teams_jip`,
`fireteam_size_requirment_failed` [sic].

**Not known** `[U]` (recorded in the source before Oodle was verified; now recoverable in
principle from decompressed `globals_pvp_0169` / `investment_globals`): era-correct display name
and definition hash/id; activity type / mode enum value; Elimination-eligible maps; **team count
and team size - "3v3" is not represented in any recovered data; do not fill it from player
memory**; round / score / win / time / revive / lives / join-in-progress / overtime rules;
private-match eligibility flags; required package sets; mode-to-announcer-line mapping (the
announcer audio itself is located and extractable - 1202 WAVs per language, §3.5).

### 3.8 The Bungie manifest (external) `[U]`

Not preserved locally (`psn-acquisition/manifests-recovered/` holds two PSN patch JSONs only; the dossier and
`dumps/` have none). `GET https://www.bungie.net/d1/Platform/Destiny/Manifest/` returns HTTP 200
with `{"ErrorCode":2102,"ErrorStatus":"ApiKeyMissingFromRequest"}` - **the `/d1/` namespace is
alive and merely key-gated**; only the historical unauthenticated `world_sql_content_*.content`
shortcut paths are 404. No key exists on this box. When a key is available: preserve the raw
`GetDestinyManifest` JSON and every `mobileWorldContentPaths` / `mobileGearAssetDataBases` entry
with URL, time, size, SHA-256; state both sides of any mapping explicitly (our side: a tag, a
`class_tag`, a symbol name, or an 8-hex `map:...:<hex>:root` token; their side: a named field);
**do not assume FNV** - any transform needs >= 3 independently known pairs plus negative
controls; quantify coverage/collisions over the 1114-name corpus; on disagreement the 01.29 dump
wins - the manifest is a decoder, never an authority. The map slugs of §3.6 are a
hash-guessing-free starter set (semantic match to `DestinyActivityDefinition`
`placeHash`/`destinationHash` names).

---

## 4. The manifest gate

Provenance: historicaldocs/design/content-manifest-gate.md §§0-10;
historicaldocs/design/package-registration.md §7.

### 4.1 The predicate `[C]`

- Reason string `error.content_manifest_not_provided` at vaddr `0x1645d12` (file `0x1649d12`).
- Handler: `c_world_controller_state_bootflow_content_check_handler` (typeinfo name at
  `0x15c0db0`, `std::type_info` at `0x192e000`, vtable body at `0x192dfb0`). Slots: `+0x00`
  `0x752d30` (shared stub), `+0x08` `0x1046360`, `+0x10` `0x79a2b0` reset (`this+0x30 = 0`),
  `+0x20` `0x79a2e0` on_enter (sets `this+0x30 = 1` unless entry reason is `0xb`
  `verify_new_content`), `+0x28` `0x79a890` exit/cleanup (§4.7), `+0x38` `0x79a380` update - the
  gate. `this+0x30` = sub-state (0 idle, 1 need-URL, 2 downloading, 3 done); `this+0x34` =
  bootflow result; `this+0x38` = started bool.
- Reasons are an int index into a `{const char *name; uint32 hash;}` table, stride `0x10`, built
  at runtime by an initializer at `0x783e00`-`0x785400`, base `0x26b22a0` (index 0 =
  `normal_flow`). The stored hashes are **FNV-1 (32-bit), not FNV-1a**:
  `fnv1("normal_flow") = 0x03cfbb27`, `fnv1("exiting_game") = 0x8e680ae0`,
  `fnv1("error.content_manifest_not_provided") = 0x598a95dc`, all matching the stored immediates.
  Second check: the CDN error-code -> reason jump table maps codes 4/6/7/8 to reasons
  `0x7f`/`0x80`/`0x7d`/`0x7e`, matching the CDN module's own strings `"guid mismatch"` /
  `"signature failure"` / `"url not provided"` / `"bad url (404)"` in the same order.

| idx | name |
| --- | --- |
| 0x7b | `error.no_cdn_connection` |
| 0x7c | `error.content_manifest_invalid` |
| **0x7d** | **`error.content_manifest_not_provided`** |
| 0x7e | `error.content_manifest_url_invalid` |
| 0x7f | `error.content_manifest_guid_mismatch` |
| 0x80 | `error.content_manifest_signature_failure` |

**Reason `0x7d` is loaded at exactly one place in the 26 MB text segment**: `mov ebx, 0x7d` at
`0x79a4b8` (byte pattern `bb 7d 00 00 00` occurs once). The update handler's sub-state 1 arm
(`0x79a3e8`):

```c
char url[0x200];                                      // rbp-0x220
if (signon_manager::get_content_manifest_url(url)) {  // 0x2a8970
    if (url[0] == '\0') {                             // 0x79a3ff
        if (content_manifest_required())              // 0x750ec0 -> always 1
            fail(reason = 0x7d, code = 7);            // 0x79a4b8
        else this->substate = 3;                      // unreachable
    } else {
        cdn_content_manifest::start(url,                                 // 0x2aff30
                                    signon_manager::get_manifest_guid(), // 0x2a8a00
                                    *(uint8*)0x18c4aec /* = 1 */);
        this->substate = 2;
    }
} else {                                              // 0x79a473
    if (content_manifest_required()) fail(reason = 0x7c, code = 0);
    else { /* 0x79a492 "Could not obtain content manifest url" - unreachable */ }
}
```

**The concrete condition is `url[0] == 0`: the assembled URL C string is empty.** `0x750ec0`
returns constant 1 (a `__cxa_guard` byte at `0x26b0718`, then unconditional `mov al, 1; ret`) -
`[C]` for the constant, `[I]` that it is `is_content_manifest_required()` - so there is **no
data-driven escape hatch** and the diagnostic `Could not obtain content manifest url`
(`0x164d35b`) is dead code in this build. The observed reason was `0x7d`, not `0x7c`, so the
failing run passed `0x2a8970`'s signon-state gate and the string really was empty. `[O]` + `[C]`

### 4.2 Where the URL comes from `[C]`

`signon_manager::get_content_manifest_url` (`0x2a8970`; `rdi` is an unused this): returns false
unless signon state at `0x263d2e8` is 3 (success), or the fallbacks
(`0x263d12c == 0 && 0x263d130 == 0 && state == 2 && 0x263d340 == 4`) hold - `[I]` that 3 is
success, `[U]` what the other three globals mean. Then, with `cm = *(void**)0x267b1b8 + 0xa84`
and `aux = *(void**)0x267b1b8 + 0x970`: if `aux[0x7e]` (config `+0x9ee`, default 1),
`out = strlcpy(prefix at cm+0x125) + strlcat(body at cm+0)`; else body only.

`0x267b1b8` points at the **"Engine Configuration"** block, `0x3298` bytes, registered at
`0x351bf0` (`0x23de00(&g_engine_config, "Engine Configuration" /*0x161d2e2*/, size=0x3298, 4,
..., defaults_thunk=0x104bab0, 0xa)`). Manifest-relevant fields (offsets from block base):

| offset | key | type | size | meaning | default |
| --- | --- | --- | --- | --- | --- |
| `0x9ee` | 894 | `bool` | 1 | use the URL prefix | **1** `[C]` |
| `0xa84` | **100** | `char[]` | `0x100` | **content manifest URL body** | `""` `[C]` |
| `0xb84` | **101** | `char[]` | `0x25` | **content manifest GUID** (36 chars + NUL) | `""` `[C]` |
| `0xba9` | **102** | `char[]` | <= `0x83` | **content manifest URL prefix** | `""` `[C]` |

Sizes from the next field's offset (`0xb84 - 0xa84 = 0x100`, `0xba9 - 0xb84 = 0x25`; `0x25 = 37`
is a canonical GUID + NUL, and the later `strncmp` length is also `0x25`). The `0x83` prefix
bound is inferred from the next mapped offset (`0xc2c`), not from a bounds check. `[I]`
`reset_engine_config` (`0x34dbb0`) memsets the block and writes nothing into `0xa84..0xc2b`
(it does set `0x9ee = 1`): out of the box the assembled URL is empty - the state the failing run
was in. Because the prefix defaults to enabled and empty, `prefix + body == body`: **setting key
100 alone is sufficient.**

**The only writer** `[C]` - confirmed by exhaustive `E8`-relative caller scans:

```
signon response f4 (client_config) sub-field 3   [callback field, descriptor 0x15a7ba0]
  -> fn 0x2aa340   (thunk)
  -> 0x1db2e0      (wrapper; sole caller of 0x1db000; FIRST calls 0x34dbb0() = full reset)
  -> 0x1db000      (config-delta reader; sole caller of 0x351c80)
  -> 0x351c80(key, &stream)  (sole caller of 0x1073170)
  -> 0x1073170(&stream, key) (the 1155-entry setter switch; writes config+0xa84 for key 100)
```

Callers of `0x351c80`: `{0x1db067}`. Callers of `0x1073170`: `{0x351ca3}`. References to
`0x267b1b8` outside the accessor block (`0x34d8c7..0x34dbda`) and the setter switch
(`0x1073200..0x107bxxx`): only `0x351c32`, `0x351c77`, `0x351e4d` (registration,
`is_initialised()`, defaults writer). **No config file, no command line, no tag, no other message
can set the content-manifest URL in this build. It is the signon response or nothing.** `[C]`
`[I]` This is the mechanism `historicaldocs/design/THE-BOOTSTRAP-HYPOTHESIS.md` predicted,
carried inside the signon response itself. (The signon response schema lives in
`design/signon-and-entitlement.md`.)

### 4.3 The config-delta wire format (`client_config.3`) `[C]`

The parse loop's wire-type-2 callback branch (`0xa1c2e`) reads a varint length and hands the
callback a length-limited copy of the stream (`{read_fn @0, ptr @8, remaining @0x10, err
@0x18}`). Then `0x1db000`:

```c
bool decode(stream *s) {
    uint32 v;
    if (!read_u32le(s, &v)) return false;      // 0xa2180
    if (v != 0x123456)      return false;      // 0x1db02e
    for (;;) {
        if (!read_u32le(s, &v)) v = 0;
        if (v == 0)        return true;        // clean end
        if (s->err)        return false;
        if (!set_var(s, v)) return false;      // 0x351c80 -> 0x1073170
    }
}
```

⚠ `0xa2180` is **`read_fixed32`, not a varint reader** (4 raw LE bytes; `0xa2230` is the 8-byte
sibling; the genuine LEB128 reader is `0xa1490`). **Magic and keys are raw u32 LE; only string
lengths are varints** - the single easiest thing to get wrong.

Value encodings by handler: `string` = LEB128 length then raw bytes, no NUL (the setter writes
the NUL); `bool`/`uint8` = 1 raw byte (not a varint); `uint32`/`float` = 4 raw LE bytes;
`uint64`/`double` = 8 raw LE bytes. `[C]`

Key space `[C]`: a dense jump table for keys **100 ... 1254** (`esi -= 0x64; cmp esi, 0x482; ja
default`, table at `0x107b8f4`, `0x483` entries) plus five sparse keys `0x85ed79`, `0x87a60a`,
`0x885e1b`, `0x891512`, `0x899a86`. ⚠ Any other key - including the 26 holes inside 100...1254:
**103, 248, 525-531, 1236-1252 (inclusive), 1254** - falls to the default arm, returns false,
aborts the whole signon parse with `callback failed`, and `0x1db2e0` logs
`Client config decoding failed. Check for invalid overrides.` (`0x160cf00`). ⚠ String setters
have **no length clamp** (`0x10731f9` copies n bytes then writes the NUL): keep the URL <= 255
bytes, the GUID <= 36, the prefix <= ~130, or neighbouring config fields are corrupted. ⚠
`0x1db2e0` **resets the entire Engine Configuration to defaults before applying the stream** -
the blob is a full snapshot, not a patch; everything you want overridden must be present every
time. `[C]`

### 4.4 The bytes to add to a signon response `[C]`

`client_config` is response field 4 (tag `0x22`), descriptor `0x15a7ba0`; sub-fields 1
(`int64 cookie`) and 2 (`int32 social_matchmaking_id`) are **required**. Sub-field 3 is the
callback, tag `0x1a`:

```
22 <len>                       # f4 client_config {
   08 <varint>                 #   f4.1 cookie                (required, int64)
   10 <varint>                 #   f4.2 social_matchmaking_id (required, int32)
   1a <varint len>             #   f4.3 config delta, opaque bytes:
      56 34 12 00              #        u32 LE magic 0x00123456
      64 00 00 00              #        u32 LE key 100  (manifest URL body)
      <leb128 n> <n bytes>     #        the URL, no NUL, n <= 255
      00 00 00 00              #        u32 LE key 0 = end
                               # }
```

Worked example (cookie 0, id 0, URL `http://cdn.deadorbit.net/manifest.bin`, 37 = `0x25` bytes):

```
22 38 08 00 10 00 1a 32 56 34 12 00 64 00 00 00 25 68 74 74 70 3a 2f 2f 63 64 6e 2e 64 65 61 64 6f 72 62 69 74 2e 6e 65 74 2f 6d 61 6e 69 66 65 73 74 2e 62 69 6e 00 00 00 00
```

(The source doc calls this blob "the 58 bytes of §4.1" in one place and "the 56 bytes of §4.1" in
its live-confirmation section; the flat sequence above is 58 bytes. Unresolved source
discrepancy, almost certainly a typo; both figures preserved here.) Field order inside the
response does not matter to the parser; ascending is conventional. To also set the GUID, add key
101 (`65 00 00 00`) with a 36-byte value; key 102 (`66 00 00 00`) is the prefix string; key 894
(`7e 03 00 00`) is the use-prefix bool as **one raw byte** (`00` disables).

`signon.deadorbit.net` and `signon.gravityshavings.net` are the only service hostnames baked into
the eboot; **there is no CDN host literal anywhere in the binary** - itself evidence the CDN URL
was always server-supplied. Any name works inside the sealed namespace; bare
`http://10.x.x.x:port/...` works too. `[I]` (Live-confirmed: `cdn.deadorbit.net` was our
invention and the client used it happily, §4.8.)

### 4.5 The download and the signature wall `[C]`

`cdn_content_manifest::start` (`0x2aff30`) allocates a **`0x96480`-byte** (615 552) response
buffer, `strncpy`s the GUID (`0x25` bytes) to `0x26427e8`, stores the `0x18c4aec` flag (= 1) at
`0x264280d`, and issues an async HTTP GET via `0x9dea0` on the client at `0x26427d8`. The poll
path maps HTTP 404 -> error 8, other non-OK -> error 2 (`0x2b0070`-`0x2b0087`); status/error live
at `0x2642834` / `0x2642838`.

`0x2b0030`: `if (*(uint8*)0x18c4974)` - the byte is **1** in the shipped RW image (vaddr
`0x18c4974`, file offset `0x18c8974`, exactly one reference in the binary). When set:

- body must be > `0x107` bytes, else error 5 (`insufficient download size`);
- `body[0..4)` LE must equal 1, `body[4..8)` must be 0 (`0xeefc8`-`0xef011`);
- `body[8..0x108)` is a 256-byte RSA signature over `body[0x108..]`;
- verification is against the key list at `0x25ab528`. Two keys are registered at startup
  (`0xf8c`, `0xfa1`) but the first is a 17-byte placeholder (`"Ling-Ling's head"`) rejected by
  registrar `0xeedd0` for being < `0x81` bytes, and the registrar caps the list at one entry -
  so exactly **one live key: a 270-byte DER blob at `0x1591800`**
  (`30 82 01 0a 02 82 01 01 00 cd 8b 61 d2 ...`), RSA-2048;
- failure => error 6 (`signature failure`) => reason `0x80`.

Same `[8-byte version][256-byte RSA-2048 signature][payload]` envelope as `sku_config.txt`. **We
cannot forge it.** When the byte is 0, the code skips to `0x2b0132` and parses the entire body as
the manifest with no envelope. The lab patch `patches/CUSA00219-lab-manifest.xml` flips that one
data byte (patch-file address `0x1CC4974`, i.e. vaddr `0x18c4974` + `0x400000` load base; eboot
file offset `0x18c8974`) from `01` to `00`. Anything reached through it is `synthetic_ancestor`.

### 4.6 The parse and the acceptance checks `[C]`

Same table-driven protobuf codec as signon: `0xa1470(stream, buf, len)` then
`0xa2020(stream, 0x18f9490, container)`. Descriptor **`0x18f9490`**; the walked struct end is
`0x9647e` against the `0x96480` allocation - 2-byte alignment slack cross-validating the walk.

```proto
message CdnContentManifest {            // descriptor 0x18f9490
  repeated Offer   offers   = 1;        // max 64,   elem 272 B, count @ +0x4408 region
  repeated Entry   entries  = 2;        // max 64,   elem 248 B, count @ +0x4408
  repeated Package packages = 3;        // max 1325, elem 280 B, count @ +0x8210
  repeated Extra   extras   = 4;        // max 800,  elem 264 B, count @ +0x62b50
  optional string  guid     = 5;        // <= 36 chars, stored @ +0x96459, flag @ +0x96458

  message Offer   { required uint32 id=1; required string a=2 /*<=128*/;
                    required uint32 c=3; optional string d=4 /*<=128*/; }
  message Entry   { required uint32 id=1; required uint32 b=2; required Detail c=3;
                    required uint32 d=4; required bool e=5; optional string f=6 /*<=47*/; }
  message Package { required uint32 id=1; required string path=2 /*<=256*/;
                    optional uint32 c=3; optional uint64 d=4; }
  message Extra   { optional string a=1 /*<=256*/; optional uint32 b=2; }
}
```

`Detail` is three optional sub-messages (a 48-char string; three 24-byte `bytes`; a 48-char
string). `[C]` shapes, `[U]` meanings for everything except `guid`.

Acceptance, in order (`0x2b0290`-`0x2b074e`): codec failure => error 1 (`decode failure`); two
128-slot hash dedup passes over `entries.id` and `offers.id`, any duplicate => error 3
(`duplicate offer keys`); if `packages` count is 0 it is derived by scanning up to `0x52d` slots
for a non-empty path (absent `packages` is legal to parse); since `0x264280d != 0`, `guid`
(field 5) **must be present** (flag at `+0x96458`) - else error 4 (`guid mismatch`) - and
`strncmp(config_guid, manifest.guid, 0x25)` must be 0; then status = 2 and the handler's
sub-state-2 arm accepts. Status 3 is set only by `0x2af830`, which early-outs unless status is
already 2: **no shortcut around a successfully parsed manifest**. `[C]`

Boot-path semantics of the other fields `[C]`/`[U]`: `entries` is the entitlement/DLC catalogue -
`Package.f1` is a foreign key into `Entry.f1`, `Entry.f5` (bool `+0xc4`) is the "no ownership
check needed" flag, and `0x2af8c9` builds a filtered copy of `entries` (dropping ids whose object
is in state 3) into `0x2642818`. `Entry.f6` (<= 47 chars) is **not** the logical-resource name.
`offers` (f1) is consumed only by the store/DLC module at `0x1e3570`, never on the boot path;
`extras` (f4) has no boot-path consumer. `[C]` on the boot path; `[U]` on store semantics.
Per the 2026-08-08 correction (see `design/signon-and-entitlement.md`): offer keys such as
`0xE0000001` (full sku) / `0xE0000002` (paid tier) appear in **`entries` (field 2), not
`offers`** - `0x2AFC40` reads the count from `manifest_buffer + 0x4408` and `0x2AFD80` indexes
`manifest_buffer + 0x4410 + i*0xF8` (elem 248 = `0xF8`); an `Entry` is `{+0x00 offer key,
+0x09 char[49] product label, +0xC0 product type, +0xC4 bool "no entitlement needed",
+0xC6 char[17]}`, and `0x59D150` hands the labels to the PS-Store open call. ⛔ `entries` does
**not** grant ownership - ownership lives in `0x26A4970` (local, `sceAppContent*`) or `0x263D46C`
(signon field 10). A manifest with zero entries makes `0x2AFC40()` = 0 and `0x2B08B0(key)` NULL
for every key.

### 4.7 `km_cache.txt` / `km_cdn.txt` - unrelated `[C]`

Entries 2 and 3 of a generic temp-filename table at `0x18ecb50`
(`0: invalid.txt  1: iaa.txt  2: km_cache.txt  3: km_cdn.txt  4: vm.txt  5: nv.txt  6: temp/
7: datamine/ ...`; the same table's tail is `10: /app0/ 11: /download0/ 12: / 13: . 14: ..`).
`0x84390(i)` is delete-by-index; the `content_check` exit hook `0x79a890` calls `0x84390(2)`,
`0x84390(3)`, then `0x84410` (`iaa.txt`) and `0x84300(1)` - it **deletes** them. The manifest is
fetched over HTTP into a heap buffer and never written to disk. `[U]` what "km" actually caches.

### 4.8 Live confirmation `[C]`/`[O]` - runs `runs/20260806-143130-manifest-url/` and `runs/20260806-143440-manifest-served/`

Reach: emulator runs against our own responder; downstream state is `synthetic_ancestor`.

1. **The falsifiable prediction held** `[C]`: key 100 pointing at a URL that 404s produced
   `'bootflow:content_check' for reason 'normal_flow'` then `'cleanup' for reason
   'error.content_manifest_url_invalid'`, and the client really fetched
   `GET cdn.deadorbit.net:80/manifest.bin` (`scheme=http, port=80, isEnableKeepalive=0`). The
   whole chain - f4.3 accepted, delta applied to key 100, CDN client uses it - is confirmed end
   to end. This also killed the older "do not emit f4.3" trap: emitted with fixed32 magic/keys it
   is accepted, and `Client config decoding failed` never appeared. `[C]` On-screen codenames:
   **"toad"** = `content_manifest_url_invalid`, **"termite"** = `not_provided` (operator
   observed; neither documented publicly for D1).
2. **Lab patch + a 2-byte manifest (`2a 00`, field 5 = empty string) passes `content_check`**
   `[C]`: the bootflow advanced through `bootflow:package_registration` and
   `bootflow:bap_signin` - two further gates crossed; empty `entries`/`offers` pass dedup, absent
   `packages` parses, empty guid matches the empty key-101 default.
3. **But an empty manifest is not a working manifest.** The game then halted deliberately:
   `RELEASE_VHALT`, `halt information: failed to load bootstrap_patchable`,
   `state:: bootflow:bap_signin`, with `active_packages.txt` empty - zero packages registered.
   The causal chain: no `packages` declared, so `package_registration` registered none, and
   `bap_signin`'s resourcerer load of `client_bootstrap_patchable` (§1.1) failed. The
   `*_unpatchable` packages load without a manifest, which is why the client got that far.
   **`packages` being optional for parsing does not make it optional for booting.** Reproducible
   at diagnostics thresholds T=4 and T=2. `[O]`
4. **Crash-report intel** `[O]`: `signon_server_reported_content_version =
   52553.16.09.28.1551.evil` (our signon f6 echo); `environment_string = live` (our f13);
   `insecure_ip = 127.0.0.1` (the `sceNetCtlGetInfo` stub gap); `silo_id = 28729` where we sent
   12345 (`28729 - 12345 = 16384 = 0x4000` exactly; `[U]` whether the client ORs in `0x4000` or
   the field is composite); `version = evil tiger_final release orbis 52553.16.09.28.1551.evil`
   (engine is Tiger, `evil` is the branch); `running from file share: true` `[U]` (likely
   emulator artifact, §1.6); `file_server_status = lost` `[U]`; `install type: 5` (see §1.6);
   uploader `TigerFoundation`, `application/x-eviltiger-upload`, to
   `dm-partnernet.upload.deadorbit.net:32556/ticket_drop`. The upload failed
   (`bdHTTPWrapper-ps4.cpp:665 Failed to get HTTP status code: [0x80431065]`) because that
   hostname was unredirected - a **consequence** of the halt, not its cause (log ordering:
   `bap_signin` 2928 -> crash folder 3110 -> `ticket_drop` 3158 -> HTTP error 3186 -> `ud2`
   3198).
5. **The 26 service hostname literals in the eboot** (redirect all of them): the 16
   `stun{1..8}.signon.{deadorbit,gravityshavings}.net`, 4 `stun.{us,eu,jp,au}.demonware.net`,
   `signon.deadorbit.net`, `signon.gravityshavings.net`, `dm-partnernet.upload.deadorbit.net`,
   `help.bungie.net`, `www.bungie.net`, and the template `auth.api.%s.ac.playstation.net`.
   `cdn.deadorbit.net` is NOT among them.
6. Diagnostics walk to T=2 also surfaced: `networking:sku_manager: base content version:
   52553-6-2940354` - ⚠ the real content version is `52553-6-2940354`, not the executable build
   string `52553.16.09.28.1551.evil` we echo into signon f6; the crash report's
   `content_build 52553 / content_revision 6` agree. `[I]` we may be sending the wrong identifier
   in f6; it silenced the complaint, which is not the same as being correct.

### 4.9 The manifest that boots - schema `[C]`, sufficiency `[I]`, and `D1_MANIFEST_FILE`

**Diagnostic minimum** (clears exactly the `failed to load bootstrap_patchable` predicate):
the three generations of id `0x0172`, plus the four of `0x012c` because `bap_signin` sub-state
`0x12` loads `investment_globals` (index 7, `failed to load investment globals`) two steps later:

| stem (`Package.path`) | `Package.c` (f3) | `Package.d` (f4) |
| --- | --- | --- |
| `ps4_investment_globals_012c_0` | `0x012c` | `0x44b9fe59eb806c94` |
| `ps4_investment_globals_012c_1` | `0x012c` | `0x69acab1c38d4da13` |
| `ps4_investment_globals_012c_2` | `0x012c` | `0x70ddf03802c8219b` |
| `ps4_investment_globals_012c_3` | `0x012c` | `0xdc221aab20f62be3` |
| `ps4_client_bootstrap_patchabl_0172_0` | `0x0172` | `0xc22292b942d10f36` |
| `ps4_client_bootstrap_patchabl_0172_1` | `0x0172` | `0xd5cd1abc17d02a78` |
| `ps4_client_bootstrap_patchabl_0172_2` | `0x0172` | `0x8c6fbbe03c06bd92` |

⚠ This 7-package probe (363 bytes encoded) clears name resolution only;
`client_bootstrap_patchable` is a tag graph spanning most of the estate and `0x3491e0` will start
resolving dependencies. Treat it as a diagnostic probe, not the goal. `[I]`

**The manifest you actually want: one `Package` per `.pkg` FILE (per generation), restricted to
localization 0 plus one language, excluding the six engine-registered unpatchable files**
(`ps4_client_bootstrap_unp3_0`, `ps4_ui_startup_unp3_0`, `ps4_ui_bootflow_unp3_0`,
`ps4_client_bootstrap_unpatchable_0`, `ps4_ui_startup_unpatchable_0`,
`ps4_ui_bootflow_unpatchable_0` - pairwise GUID-conflicting at the same id+generation):

| set | count | encoded size |
| --- | --- | --- |
| neutral only (loc 0) | **932** | 47 366 B |
| neutral + English (loc 0,1) | **1033** | 52 797 B |
| every file in the dump (all 8 locales) | 1641 | **exceeds the 1325 cap** |

The per-locale figure fitting under 1325 while the all-locale figure does not is itself evidence
of the shape (`[C]` counts, `[I]` inference) - and §2.6's generation-chain finding upgrades
"one entry per file" to a **mechanical requirement**. 52 797 B is far inside the `0x96480`
download buffer.

**Byte encoding** `[C]` schema:

```
CdnContentManifest := Package* guid

Package  := 1a <len>                         # f3 packages, length-delimited
              08 <varint>                    #   f1 offer_id   -> emit 0 (classless, skipped by both consumers)
              12 <varint len> <bytes>        #   f2 path       -> the on-disk stem
              18 <varint>                    #   f3 package_id -> header[0x04], 0x0a..0x0bff
              20 <varint>                    #   f4 guid       -> header[0x08], u64 LE value
guid     := 2a <varint len> <bytes>          # f5, must be PRESENT; must equal Engine Config key 101 (empty by default)
```

Worked example, one package:

```
1a 36                                              # Package, 0x36 bytes
   08 00                                           # f1 offer_id = 0
   12 24 70 73 34 5f 63 6c 69 65 6e 74 5f 62 6f    # f2 "ps4_client_bootstrap_patchabl_0172_2"
         6f 74 73 74 72 61 70 5f 70 61 74 63 68
         61 62 6c 5f 30 31 37 32 5f 32
   18 f2 02                                        # f3 = 0x0172
   20 92 fb 9a e0 83 fc ee b7 8c 01                # f4 = 0x8c6fbbe03c06bd92
```

**Serving it**: send Engine Config key 100 in signon f4.3 (§4.4), keep
`patches/CUSA00219-lab-manifest.xml` so the RSA envelope is skipped (§4.5), and serve the body
raw at the key-100 URL; the manifest guid (f5) must match key 101 - leave both empty. Reach:
everything past this point is `synthetic_ancestor`.

**`D1_MANIFEST_FILE`** `[C]` (as-built, from `capture-rig/run-boot.sh` and `status/LESSONS.md`):
`capture-rig/run-boot.sh` pins
`D1_MANIFEST_FILE="${D1_MANIFEST_FILE:-$REPO/capture-rig/manifests/full_en.bin}"` - the
1033-package neutral+English body above - and asserts the file exists and prints its size at
startup. A run launched without it (2026-08-07) served a 2-byte manifest and died at
`bootflow:bap_signin` with `failed to load bootstrap_patchable`, reading exactly like a
regression in the patch under test; `capture-rig/run-signon-experiment.sh` now refuses to start
without it. The environment is part of the experiment.

---

## 5. Server-message schemas

Provenance: historicaldocs/design/package-container-pvp-content.md §§14-19, 24. Kept here because
the lookup chain runs through the container/tag machinery of §2; the investment/BAP wire protocol
itself is owned elsewhere (`design/signon-and-entitlement.md` for signon;
one protocol, one implementer).

**Context.** With BAP connected inside `bootflow:investment_signin`, the client rejected our
reply to investment server-message id **503** with `bitstream error when decoding
message-arguments`. Question: is the 503 argument schema recoverable from the package corpus?

### 5.1 The lookup chain `[C]`

- `id_to_index` (`0x27fdd0`): if `investment_globals_loaded()` (`0x25c9e0`), get the interface
  (`0x25c9a0` -> global `0x2639ae8`, BSS; constructor `0x25bbb0` registers it under the name
  string at `0x1612928`, **`Investment Globals Interface`**; RTTI `0x15a9530`, vtable
  `0x18f4f70`), call vtable `+0x248` (`0x25ef80`) for the definition container, then **linear
  scan**: vtable `+0x4d0` (`0x25b4f0`) per index, matching `u16` at record `+0x10` (via the
  biased pointer) against the message id. No id -> slot table exists.
- `0x25ef80` resolves a **content tag held at investment-globals object `+0x578`** using exactly
  the §2.5 tag maths (`0x25efa5`-`0x25efb3` byte-identical to `0xd85a5`-`0xd85b7` - a third
  independent site), then `container = resolved entry bytes + 0x18`.
- `0x25b4f0`: `count = i32 at container+0x00`; `base = (container+8) + i64 at container+8`
  (**self-relative**); returns `base + index*0x40 + 0x10` - records are **0x40 bytes** and the
  accessor pointer is pre-biased by `0x10`.
- Record fields (element-relative): `+0x20` u16 **message id** (503 for ours); `+0x24` u32 **tag
  of the argument schema, selector 0**; `+0x28` u32 **selector 1** (decoder `0x261fb0` at
  `0x2621ef`-`0x2622a3`; `sar esi, 0xd` confirms these are content tags).
- Four one-line thunks hardcode the selector (found via `DT_SCE_RELA` vtable slots): `0x569b80`
  (sel 0, `+0x14` via bias, **encode** `0x261be0`; slots `0x1926200`, `0x19262e0`), `0x569ba0`
  (sel 0, decode `0x261fb0`; `0x1926208`, `0x19262e8`), `0x569c10` (sel 1, encode; `0x1926270`,
  `0x1926350`), `0x569c30` (sel 1, decode; `0x1926278`, `0x1926358`). Each vtable holds an
  (encode, decode) pair at `+0x00` and `+0x70`: selector 0 and selector 1 are two distinct
  message-codec objects - **request and response**. Which selector is which is `[U]`: decode both
  schemas and the one reproducing the observed request (§5.2) is the request; **do not assume
  the order**. `0x261be0` is the encoder that writes what we owe (same Unicorn-harness shape as
  the RSAT writer oracle).
- Message body structure from the decoder's three size-guard strings: **[16-bit message id]
  [alignment padding][argument block][optional variable-length payload]**. `[C]` on the stages,
  `[U]` on each encoding.

### 5.2 Where the schema lives, and the observed request `[C]`/`[O]`

Both hops land in the investment globals: `investment_globals` resolves via its symbol to tag
`0x80A5C000` = **package `0x012e`, entry 0** (60 bytes, block 105, Oodle-compressed), and
`ps4_investment_globals_012e_2.pkg` has **515 blocks, all compressed, 0 raw bytes**. The root
record needs >= 1404 bytes (`root + 0x578`), reached through `0x548374`
(`root_tag = *(uint32*)(resolved(outer_tag) + 0x10)`, stored at `iface+0x0c` by `0x25c970`);
`[I]` that `outer_tag` is the `investment_globals` symbol tag. Independent negative: a scan of
**every uncompressed block in the estate (14.5 GB)** for the structural fingerprint (i32 count,
self-relative i64 at +8, 0x40-stride records with ascending u16 ids at `+0x20` and two
well-formed tags at `+0x24`/`+0x28`) found 351 candidates, **all false positives** (u16 clusters
in geometry/mesh data), and a tighter scan of 713 MB of raw investment/shared_manifest/globals/
bootstrap content for `u16 503` + two tags: **0 hits**. The definitions are not in any readable
content in this build, and not in `shared_manifest` (whose records are a different class,
`0x80800148` dominant in `0x430`, with a u32 tag at `+0x10`, not a u16 message id).

The client's own 503 request `[O]` `observed-live` (from
`runs/20260806-164452-investment-T1/responder.log`, decrypted BAP inner frame
`00 0a 00 00 00 04`, type `0x000a`, seq 4, 47-byte body): `0x00` u16be **503**; `0x02` a
**36-byte NUL-padded name field** containing our own PSN online id (issued by Layer A at
`HandleLoginReply`); `0x26` **9 bytes `[U]`** beginning `a2 76 cb c9 ...` (the log truncates the
last 5 - re-log the frame in full before reasoning about them). `[I]` 503 is an
identity/character lookup by display name; the fields are byte-aligned despite the "bitstream"
name. Reach: the bytes are authentic client behaviour; the session context is
`synthetic_ancestor`.

### 5.3 The unblock - a non-200 status skips the decode `[C]`

`handle_response_internal` (`0x5681e0`): if the policy's flags carry bit `0x10`
(`test al, 0x10` at `0x56826b`), the response status is read (`call [rax + 0x78]`), and if
`!= 0xc8` (200; `cmp r14w, 0xc8` at `0x568283`) the decode is **skipped entirely**
(`jne 0x56833f`), falling through to `bap_message_queue: [%s] Request failed, type=%d, status=%s`
(`0x56854b`) and the failure handler `0x5680b0`
(`Investment message request failed (id: %d)`). **Reply to 503 with a class-2 header status !=
200 (e.g. `0x1F4` / 500) and the client never looks at the body.** Three mutually exclusive
outcomes, one run decides: (a) failure logged and bootflow moves on - 503 is not a gate, iterate
to find what is; (b) failure logged then a halt naming 503 - the schema is mandatory; (c) no
change, still `bitstream error` - this policy lacks flag `0x10` (we only ever sent 200, which is
consistent with both settings, so the flag's state is `[U]`). Supporting: the client carries
**48** `c_investment_bap_message_(push_)policy_*` classes; the push path (`0x563170`) has its own
tolerant branch (`Received a response-message that we're treating as a push-message with non-OK
status code '%s' -- ignoring it`) - two independent non-OK escape hatches, `[O]` neither
exercised at the time of writing.

If the schema does turn out mandatory, the extraction recipe: decompress
`ps4_investment_globals_012e`, read the root record, take `*(uint32*)(root + 0x578)`, resolve
with §2.5's tag maths, walk 0x40-byte elements to `u16 503` at `+0x20`, read the two schema tags
at `+0x24`/`+0x28`, and use the observed request to tell request from response. The minimum valid
503 response body is `[U]` until then - a speculative body is a guess the client will reject in
the same words it already has.

---

## 6. Open `[U]`s

Container and estate:

- Header `0x06` type/flag; header `0xa4` bitmask and `0xa8` hash (no consumer found; cheapest
  probe: verify one RSA signature via the modulus at `0x19f9060`, then diff `0xa4` between two
  packages differing in one content family).
- Entry `u04` sub-fields (correlate `u04 & 0xffff` against observed 16-byte packing gaps).
- Block `flags` bits beyond bit 0 (parse the 01.00 base estate, 250 files, the same way).
- The compressed-block leading byte `0xb7` - what it encodes.
- `class_tag` dual-use selection rule; whether any bit-31-clear value equals a known content hash
  (join against the `map:...:<hex>:root` tokens or FMOD/Wwise event ids).
- Package ids 0/1 runtime resolution (read the writers of `k_pkg_slot` at `0x25692f0`).
- The 01.00 base estate is unenumerated; every count here is 01.29 only.
- `owc_client_startup` resolving to nothing is `[C]` for this dump's 501 symbol-bearing
  packages; `[U]` whether it is dev-only or present in the base estate.
- The `shared_manifest` record body beyond the 16-byte prologue and `+0x10` target tag (inline
  array shape is `[I]` from two hand-read records).
- The internal "28 maps" vs the enumerated 33 slugs discrepancy (§3.6).

Registration and gate:

- Registration flags bits `0x02` and `0x20`; install type 5 vs 12 mount selection.
- Generation registration order is `[I]` ascending; a logged result `-93` is the discriminator.
- `Package.id = 0` safety is proven against both found consumers of `cdn_manifest_package`
  (`0x79cd20`, `0x1e3b90`); a consumer reading the container directly would not have shown up.
  `[I]`
- The §1.6 grep negative rules out a plaintext on-disk manifest only. `[I]`
- Semantics of `0x263d12c` / `0x263d130` / `0x263d340 == 4` in `0x2a8970` (only matters for a
  pre-signon manifest fetch).
- Meanings of manifest `Offer`/`Entry.Detail`/`Extra` fields beyond the boot path; the three
  24-byte `bytes` in `Detail` smell like hashes/keys.
- Whether `0x9dea0` honours `https://` (prefer `http://`; the TLS trust store question is
  inherited from the signon client). Whether the `0x9ee` prefix bool has writers besides key 894
  and the defaults.
- `silo_id` 28729 vs sent 12345 (`+0x4000` exactly - OR-in or composite field?). Whether signon
  f6 should carry `52553-6-2940354` instead of the executable build string.
- The other ~1150 engine-config keys (the switch at `0x1073170` is a complete offset map; worth a
  generated Rust table if we ever drive engine config deliberately).
- Whether the 7-package probe reaches a different failure than the 1033-package manifest's path -
  the dependency graph behind `0x3491e0` is untraced.

Content:

- Everything in §3.7's Elimination list (now unblocked in principle by verified Oodle
  decompression, but not yet extracted): display name, mode enum, eligible maps, team size,
  round/revive rules, private-match flags, mode-to-announcer mapping.
- The Bungie `/d1/` manifest pull (needs an out-of-band API key; discipline in §3.8).
- Which of the 48 investment message policies is "policy 1"; request-vs-response selector; the
  9 trailing bytes of the 503 request; whether our 503 policy has status flag `0x10` (§5.3).
- Whether `profile_setup` needs content at all or only authoritative state - the container does
  not answer it; the character-creation content is present and tag-addressable, so "cannot find
  the definitions" will not be the blocker.

---

## 7. Dead claims

Banners override bodies. Each claim below appeared in a source and was killed there or by a later
source; none is live.

| # | Dead claim | What killed it |
| --- | --- | --- |
| 1 | "`investment_assets` has 15 competing ids (`investment_globals` 6), so the manifest's job includes selecting which id is current." | Correction in historicaldocs/design/package-registration.md §1.4: ids `0x0137, 0x0139, ...` merely share the filename prefix; only `0x0135` (and `0x012c`) carry the symbols. Each logical name is owned by exactly one id; no id-selection problem. (The whole family still registers - reachable by tag, §3.4 - but not to disambiguate a name.) |
| 2 | "The dump spans four content builds; packages come from four content builds" read as four variants, with generation selection a silent-content-error risk. | Correction in historicaldocs/design/package-registration.md §2: the four numbers are the four **patch generations** (patch layers, not variants); `find_symbol` prefers the newest and older generations supply untouched tags. |
| 3 | "`Package.path` is unlikely to be the raw filename; where `ps4_` comes from is unknown" (content-manifest-gate §9.5/§10.2). | historicaldocs/design/package-registration.md §§1.1, 4.4, 5 `[C]`: the stem IS the on-disk base name, `packages/<stem>.pkg` is the open path, and the `ps4_` prefix must be in the manifest string (the engine builds it only for the three hard-coded unpatchable packages). |
| 4 | "795 distinct package base names against the 1325 cap, so a complete manifest fits" (one entry per base name). | historicaldocs/design/package-registration.md §7.2 + package-container-pvp-content.md §5: one `Package` per `.pkg` **file** (per generation) is a mechanical requirement (newest generation's block table dereferences older files); per-locale (1033) fits the cap, all-locale (1641) does not. |
| 5 | "The manifest's `offers` ids are probably the offer keys `0xE0000001`/`0xE0000002`." `[I]` | Half-confirmed, half-corrected 2026-08-08 (banner in historicaldocs/design/content-manifest-gate.md §10.4): offer keys live in **`entries` (field 2)**, not `offers`; and `entries` does not grant ownership - ownership lives in `0x26A4970` or signon field 10 (`0x263D46C`). |
| 6 | "The crash-upload HTTP error (`0x80431065`) caused the halt." | Log ordering in historicaldocs/design/content-manifest-gate.md §9.4: `bap_signin` 2928 -> crash folder 3110 -> `ticket_drop` 3158 -> HTTP error 3186 -> `ud2` 3198. The upload failure is a consequence of the halt. |
| 7 | "Do not emit signon `f4.3`" (a trap recorded in the old signon doc). | Live run 2026-08-06 (historicaldocs/design/content-manifest-gate.md §9.1): emitted with fixed32 magic/keys, f4.3 is accepted; no `Client config decoding failed` line. See `design/signon-and-entitlement.md`. |
| 8 | "Entry counts grow monotonically across generations." | historicaldocs/design/package-container-pvp-content.md §5: 71 chains shrink; patches can remove content. Recorded as a wrong prediction by its author. |
| 9 | "`shared_manifest` is the likely home of the server-message schemas." | historicaldocs/design/package-container-pvp-content.md §15: its records are a different class (`0x80800148` dominant) with a u32 tag at `+0x10`, not a u16 message id; the schemas are in no readable content in this build. |
| 10 | "The historical Bungie content paths 404, so the API surface is effectively gone." | Correction §9a of the same doc: the `/d1/` namespace is alive and key-gated; only the unauthenticated `world_sql_content_*.content` shortcut is dead. |
| 11 | "The block SHA-1 does not cover the stored bytes and probably covers the decompressed ones" (floated in another lane). | historicaldocs/design/package-container-pvp-content.md §21: false and withdrawn; measured 340/340 on `ps4_investment_globals_012c_1` and corpus-wide via `d1-pkg verify --blocks`. The container stores no digest of decompressed output. |
| 12 | "The §14 message-definition record offsets (0x40 stride, u16 id at +0x20, tags at +0x24/+0x28) were all three wrong." | Self-correction banner in historicaldocs/design/package-container-pvp-content.md §24: **the correction was the error** - the accessor `0x25b4f0` returns a pointer pre-biased by `0x10`, so the codec's `[rax+0x10]/[+0x14]/[+0x18]` are record `+0x20/+0x24/+0x28`. §14 stands. |
| 13 | "Oodle LZH/LZHLW is unreadable - the one hard remaining wall / BLOCKING" (76% of blocks). | Superseded by the later addendum of the same doc (§§20-23): decompression achieved and verified (473/473 independent length-header agreements; the `0x40000`-tail termination lesson). The earlier readable-share figures (11.3% of entries, 23.7% of bytes) describe the pre-decompressor state only. |
| 14 | "A decode loop may terminate when 0x40000 output bytes are produced." | historicaldocs/design/package-container-pvp-content.md §23: decompressed blocks are at most `0x40000`, not always exactly (block 561 of `ps4_globals_pvp_0169_3`: 65479 stored -> 114688 plain); terminate on input exhaustion or the failure mimics a decoder defect. |

---

## 8. Sources

| source (now under historicaldocs/) | what it contributed | date range |
| --- | --- | --- |
| `historicaldocs/design/package-registration.md` | §1 registration and resolution, §2.1 header fields, §4.9 minimum/1033 manifests and byte encoding, the on-disk-manifest negative | 2026-08-06 |
| `historicaldocs/design/package-container-pvp-content.md` | §2 container format (entry/block tables, tag, generations, Oodle, `d1-pkg`), §3 inventories including **§7.5** (the 33 PvP maps - preserve that section number in citations), §5 server-message schema chain and the 503 story | 2026-08-06 (+ addenda) |
| `historicaldocs/design/content-manifest-gate.md` | §4 the gate: predicate, engine config, config-delta format, download/signature/parse, live confirmation, crash-report intel | 2026-08-06 (+ 08-08 banner) |

Live cross-topic homes: `design/activity-launch.md` (launching a map/mode),
`design/items-and-gear.md` (gear content families), `design/signon-and-entitlement.md` (the
signon response carrying f4.3, entitlement/ownership, signon field 10).
As-built facts on `D1_MANIFEST_FILE` are from `capture-rig/run-boot.sh` and `status/LESSONS.md`.
