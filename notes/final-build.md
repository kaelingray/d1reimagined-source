# The final PS3 build (01.24) against 01.00: static findings

Sources are the EBOOTs of updates 01.13 and 01.24, decrypted on the PC by
`scripts/Invoke-FinalBuildExport.ps1` (SHA-256 in the data repo's `ps3_updates/EXPORT_VALIDATION.md`). The final
investment globals chains 012c and 012f were read over HTTP ranges with `ps3_update.py fetch`.
Everything here is static. Nothing has been checked in a running 01.24 yet. CONFIRMED means read
directly from the executable or the packages.

## Reading the update executables

- **The type definitions changed format (CONFIRMED).** The first export found only 351 types in
  01.24; the true count is 5,041. Two changes caused the miss.
  - Definitions are 0x20 bytes shorter and link their field list from def+0x34, not +0x54.
  - Each 0x28-byte field entry now leads with a repeat count; the rest of the entry follows 4
    bytes later.
  - A fixed array such as `u8[128]` is now one entry {128, offset = stride, ...}. 01.00 listed
    every element. All 265 such arrays are the sole field of their type, with count × stride =
    size.
  - `d1_types.py` reads both layouts. It picks the one that finds the most types, and expands
    arrays back into elements, so the two builds compare field for field. The 01.00 export is
    byte-identical to before.
- **The TOC base moved:** 0x175a5e8 (01.00), 0x184fc00 (01.13), 0x1891798 (01.24).
  - `d1_eboot.py` now reads it from the entry point's function descriptor.
  - The curated labels are 01.00 addresses. They are applied to that build only.
- **Type handles are stable (CONFIRMED).** 01.00 → 01.24:
  - 4,318 of 4,387 types match (3,771 structurally identical, 547 changed);
  - 69 were removed and 759 added;
  - only 183 matched types were renumbered.

  Almost every handle the stand-in writes keeps its number. `d1_builddiff.py` now tries the
  same handle first. It compares field kinds (offsets shift whenever a field is inserted) and
  prints side-by-side layouts of the stand-in's types.

## What changed for the stand-in

**Character objects (CONFIRMED, same handles, grown types):**

| Where | 01.00 | 01.24 |
| --- | --- | --- |
| progression `0x80801855` | 0x770; the 20 gear slots at +0x138 | 0x8c8; three new f32 after +0; gear slots at **+0x1e8** |
| gear slot `0x8080183c` | 0x28: {u64 id, i16 item, u8, i16, +0xe dyes} | **0x30**: {u64 id, i16 item, i16, i16, +0x10 new `0x80801069`, +0x18 dyes} |
| peer summary `0x808017fd` | progression at +0x30 | progression at +0x30 (size 0x7b0 → 0x908) |
| select summary `0x80801954` | progression at +0x68 | progression at +0x68 (size 0x7e8 → 0x940) |
| self character `0x80801714` | 0x55b0: inventory +0x30 (0x1808), equipped ids +0x1838 | 0x6db8: inventory +0x30 (**0x2808**), new `0x808019ff` +0x2838, equipped ids **+0x2900** |
| inventory entry `0x808016e0` | 0x18: {i16 item, u64 id, i32, i32} | 0x20: one more i32 |
| account `0x808018c7` | 0x1ff8: flag table A +0x12a8 (1,024), table B +0x16a8+1 (256) | 0x7050: B at **+0x4f78+8**; A LIKELY `0x80804b0c` at +0x383c (**2,560** flags); vault-like `0x80801087` (326 items) at +0x4c0 |
| character flags (table C) | self +0x3bc0, 1,024 | self **+0x5000**, 2,048 (`0x80804c21`) |

- `bap_server.character_layout(schema)` derives the gear offsets from whichever schema the
  stand-in loads. It finds each field by its nested type's handle, so `--character-gear`
  writes 01.24's layout when given a 01.24 schema. With the 01.00 schema (or none) it gives
  the old constants.
- The `--account-flags` tables are still 01.00-only.

**Mission objects (CONFIRMED):**

- The object id `0x808004b1` widened the kind field from 6 to 7 bits. The mission tooling
  encodes ids from the schema, so a 01.24 schema handles this.
- The objective `0x80802a1b` grew (0x2d0 → 0x2f0), but its player mask stays at **+0x2c0**. A
  machine id and a second object id were added at +0x2cc/+0x2d4.
- The cinematic, sequence, trigger and squad types are still to be compared (see the diff
  report).

**Web services (CONFIRMED):**

- The table moved from the 012c tag 0x80a5806b (now an empty 60-byte record) to tag
  **0x80a5e21a in package 012f**. The investment root is still 0x80a58000, and most of its
  children now point into 012f.
- The record format (36 bytes, element class 0x80801820) is unchanged. There are 94 services,
  up from 66.
- **All 66 old services keep their wire ids and their type handles.**
- 28 are new:
  - 208, 209, 211;
  - 406;
  - 804;
  - 904, 905;
  - 1228–1233;
  - 1305–1307;
  - 1601, 1602, 1606, 1609–1614;
  - 1701, 1702;
  - 1801.
- 19 old services changed a request or response payload (often only through a nested type).
  Those the stand-in answers: **104** universe (both), **503** sign-in (response `0x80801825`
  0xe0 → 0x3c8), **201/202/203/205** (responses). Service **206** (family fetch) is unchanged.
- `d1_types.py services --packages <folder with 012c and 012f chains>` prints the table. It
  already searches every package.

**Sign-on (CONFIRMED):**

- The nanopb success message (field 2) is identical, so the stand-in's reply format still
  fits.
- New optional fields:
  - request 20 (8 integers) and 21 (five strings of 64/64/128/32/32 bytes);
  - response 13 (string[64]) and 14 (u32).
- The request table is at 0x12e66dd and the response table at 0x12e6850. The handler is at
  0x28dbd4 (01.00: 0x126744). It decodes into a stack struct at r1+0x70:
  - field 13 is copied to the sign-on singleton at +0x47cc. No other code touches it except a
    reset at 0x28cf38, so its use is UNKNOWN;
  - field 14 is wrapped in an address record (u32, then u16 4 at +0x12) and handed to the
    `network_demonware` module (0x1326e8 returns it; 0x132cb4 validates and stores it at
    +0x184). Its STUN/relay code (0x132cf4) compares it with the module's own address (+0x154)
    and records whether they match, then passes it with a port to 0x1197bc.

  This is LIKELY the client's public IPv4 address as the sign-on server saw it, used for NAT
  traversal. `signon_probe.py --public-ip ADDR` sends it (optional; the byte order is not
  confirmed).
- `SignOn?platform=%s&build=%s` and the two sign-on domains are unchanged.
- New: `stun1`–`stun8.signon.deadorbit.net` / `.gravityshavings.net` and
  `destiny-stun.{us,eu,jp,au}.demonware.net`. The DNS sinkhole already answers every name
  locally.

**Demonware lobby (LIKELY removed):**

- 01.24 has none of 01.00's `panther-ps3-{lobby,auth,auth3}.{prod,cert,dev}.demonware.net`
  names. It also lacks the strings of the 01.00 lobby client module around 0x42c9d8
  (`online_demonware memory`, `failed to connect to lobby`, `platform account id changed`,
  `BLUS30843`). 01.13 still has all of them.
- The Demonware library (version 44: bdCore, bdSocket, bdLobby/bdAuthService, bdNet) is still
  linked, and its strings now mention NAT punch, QoS probes and STUN.
- So the final client probably goes from sign-on straight to BAP. The auth3 → lobby 3074 step
  that 01.00 needs would be gone.
- How (and whether) 01.24 reaches a Demonware auth server is UNKNOWN. Candidates are response
  field 13 or the title configuration. **Check on the first boot:** log DNS queries and
  connections after sign-on.

**BAP (LIKELY unchanged):** the message names (`shello`, `sub req`/`sub rsp`, `qz->c`, `ws`)
are all present. A `network_bap_nat_punch_queue` was added.

**Error ids:** 01.24 drops the `error.*` string table (248 names in 01.00 → 1). An error code on
screen is therefore no longer found by string. Map it through the 01.00 table where the meaning
carried over.

**Patches for RPCS3 (CONFIRMED addresses; the PPU hash comes from the first boot):**

- The embedded sign-on CA is the identical 2,390-byte PEM ("Tiger Root Authority" + GeoTrust) at
  **0x12d3740**, used at 0x113894 (01.00: 0x11b7218).
- The content-manifest signature flag is **0x18dca64** (initially 1), read only at 0xc65d14,
  which has the same `cmplwi 0x108` shape as 01.00's 0xb6f3cc (01.00: 0x179d5f4).
- `game_ca_patch.BUILDS["01.24"]` holds both. The PPU hash must be copied from RPCS3's log
  ("PPU executable hash") on the first 01.24 boot:
  `prepare_rpcs3_copy.py --game-ca ... --build 01.24 --ppu-hash PPU-...`. A local
  reimplementation of RPCS3's hash did not reproduce the known 01.00 value, so it is not
  guessed.

**Packages and DLC (CONFIRMED strings):**

- The patch folder is built as `%s/game/%s/USRDIR/%08x/` with `/dev_hdd0` and the constant
  0xa0000001 (0xc951c, a case of the path-root switch at 0xc8758), i.e.
  `/dev_hdd0/game/<title>/USRDIR/a0000001/`.
- Other new path strings: `packages/%d\`, `%s_%s.pkg`, `unpatchable\packages`,
  `client_bootstrap_patchable` / `client_bootstrap_unpatchable`, and
  `Patchable/Unpatchable package registration failed with result %s (%d)` (0x366b88).
- New DLC strings:
  - `DLC Patch/CDN/Cache/Entitlement/Marketplace Containers`;
  - `dlc_installer error 0-7`, `dlc_url 0-7`;
  - `Apply quick patch (package='%s', is_asset='%s')`;
  - `World-transition requires DLC.` / `... requires permissions.`;
  - `bootflow:illegal_dlc_media`;
  - `bootflow:check_media: NPDRM check failed`;
  - `offer_key.edat`, `destiny.edat`.
- New bootflow states: `check_media` and `confirm_account_transfer`.

## Reports

Regenerate with the current scripts (the data repo's first `diff_*.md` were made with the old
scanner and undercount 01.13/01.24 by about 4,600 types):

```
python scripts/d1_types.py --elf <01.24 EBOOT.elf> schema types_01.24.json
python scripts/d1_builddiff.py --old-elf <01.00 EBOOT.elf> --new-elf <01.24 EBOOT.elf> --names 01.00,01.24 \
    --map-out handles_01.00_01.24.json --out diff_01.00_01.24.md
python scripts/ps3_update.py fetch BLES01857 --versions 01.13,01.20,01.22,01.23 \
    --match ps3_investment_globals_012f_1.pkg,ps3_investment_globals_012f_2.pkg,ps3_investment_globals_012f_3.pkg,ps3_investment_globals_012f_4.pkg --out <dir>
```

The final 012f chain is _1 (01.13), _2 (01.20), _3 (01.22) and _4 (01.23). 01.19's _5 is stale.
