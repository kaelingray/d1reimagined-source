# Candidate research tools and references

Reviewed: 2026-09-22. These are leads to revisit after the Xbox 360 capture. No tool below has yet been tested against our own Destiny files. Keep captured files and extracted assets outside Git.

## First choices for Destiny 1 Xbox 360 data

| Repository | Why it matters | Limitation / trigger |
| --- | --- | --- |
| [v4nguard/tiger-pkg](https://github.com/v4nguard/tiger-pkg) | Package library explicitly lists Destiny Legacy / The Taken King on X360 as supported (`d1_ttk`, `x360`). | Try on a copy of extracted `.pkg` files. Its README says Destiny 1 decompression needs `oo2core_3_win64.dll`; do not substitute an unverified download. |
| [V4NGUARD/tachyscope on Codeberg](https://codeberg.org/V4NGUARD/tachyscope) | Its current `src/main.rs` initializes `tiger-pkg` with `DestinyTheTakenKing`, and Cargo uses `tiger-parse`; potentially a direct D1 data browser. | No root README was present when checked. Treat actual X360 compatibility as unverified until we have packages to try. |
| [v4nguard/quicktag](https://github.com/v4nguard/quicktag) | Inspects Tiger package references, strings, structures, and some assets. | README marks `d1_ttk` support as work in progress. |
| [v4nguard/tiger-parse](https://github.com/v4nguard/tiger-parse) | Parsing code used by the newer tachyscope. | Adopt only for a specific data structure question. |
| [cohaereo/destinydocs](https://github.com/cohaereo/destinydocs) | Documentation for Destiny 1/2 internals. | Check version and platform relevance for each finding. |
| [v4nguard/swizzleinator](https://github.com/v4nguard/swizzleinator) | Lists Xbox 360 image detiling support. | Useful for extracted textures, not boot or sign-in. |

Update 2026-09-27 (PS3 route): `scripts/d1_pkg.py` reads the PS3 packages directly (layout from
tiger-pkg's `d1_legacy`). [lizardpeter/destiny-1](https://github.com/lizardpeter/destiny-1)
documents the v24 package format and the activity -> bubble -> map-container graph from PS4
Rise of Iron data, and [MontagueM/Charm](https://github.com/MontagueM/Charm) has the D1 activity
schemas. How they apply to the activity-host work: [`public-research.md`](public-research.md).
`d1_pkg.py items` lists the investment item list (item index, equipment slot, bucket, type,
English name) from `ps3_investment_globals_012c/012e` plus the string tables; `items --buckets`
lists the 14 equippable buckets. The item indexes are what `bap_server.py --character-gear`
takes.
`d1_types.py` reads every runtime type definition (4,387) straight from the decrypted EBOOT;
they match the run-time exports field for field. With `--packages`, it also reads the 66 web
services from the investment globals. `schema OUT.json` writes the same JSON the stand-in loads,
`type HANDLE` prints one type, and `services` lists each service's request and response
layout ([`investment-services.md`](investment-services.md)). No running game is needed.
`ps3_update.py` lists Sony's PS3 title updates and extracts chosen files from the update PKGs
over HTTP ranges. `d1_builddiff.py` compares two builds:
- types, matched by structure because handles are renumbered;
- the handles the stand-in uses;
- web services;
- strings.

`Invoke-FinalBuildExport.ps1` runs the whole 01.00 -> final comparison on the PC
([`preservation-plan.md`](preservation-plan.md)).

## Destiny 2 asset tools: background only for now

- [cohaereo/alkahest](https://github.com/cohaereo/alkahest): Destiny 2 asset viewing and renderer research. Its links to Bungie graphics talks may be useful background.
- [MontagueM/D2TagParser](https://github.com/MontagueM/D2TagParser): Destiny 2 Beyond Light tag parsing.
- [MontagueM/DestinyUnpackerCPP](https://github.com/MontagueM/DestinyUnpackerCPP) and [MontagueM/DestinyUnpacker](https://github.com/MontagueM/DestinyUnpacker): explicitly target Destiny 2 Beyond Light packages, not the Xbox 360 Taken King format.
- [nblockbuster/D2TextureRipper](https://github.com/nblockbuster/D2TextureRipper), [nblockbuster/D2StaticDocs](https://github.com/nblockbuster/D2StaticDocs), [MontagueM/D2Maps](https://github.com/MontagueM/D2Maps), and [MontagueM/DestinyMapmining](https://github.com/MontagueM/DestinyMapmining): texture, model, or map research to consult if corresponding D1 asset formats become a question. The first three are explicitly D2-focused; verify the last one's source data before borrowing its methods.
- [MontagueM/Charm](https://github.com/MontagueM/Charm): described as a Destiny 2 extraction tool; its current README also calls its branch outdated.
- [nblockbuster/tachyscope on GitHub](https://github.com/nblockbuster/tachyscope): README says it supports Destiny 2 only. Distinct from the Codeberg `V4NGUARD/tachyscope` above.

## Historical / specialized Destiny leads

- [nblockbuster/bungie-lua-decompiler](https://github.com/nblockbuster/bungie-lua-decompiler): specifically for format 14 Lua scripts in a Destiny 1 alpha. Do not assume it applies to the released Taken King build.
- [v4nguard/destinypkgtool](https://github.com/v4nguard/destinypkgtool): archived; author says to use the newer package library instead.

## Service research examples from other games

- [Demonware-Custom-Server/demonware-cod4](https://github.com/Demonware-Custom-Server/demonware-cod4): Call of Duty 4 server emulator.
- [hosseinpourziyaie/demonware-companion](https://github.com/hosseinpourziyaie/demonware-companion): archived tooling aimed at Call of Duty Demonware payload analysis.
- [jordam/demonbugger](https://github.com/jordam/demonbugger): old Demonware protocol debugger requiring Python 2.7 on Linux.
- [project-bo4/shield-development](https://github.com/project-bo4/shield-development): experimental Black Ops 4 client with a built-in Demonware server emulator.

These show possible reverse engineering patterns, but they are not Destiny servers or drop-in fixes. First establish what the Destiny client actually requests, what Xenia's Xbox Live layer provides, and which failure is observed. Keep any future service experiments local to the development environment.

## Xenia network forks and service framework

- [Nexia360 Xenia Canary netplay fork](https://github.com/Nexia360/xenia-canary) has configurable Live-like sign-in and XNet state, but its XHttp request and connection functions remain stubs in the checked source. It is a possible source of emulator code, not a verified Destiny solution.
- [Xenia Web Services](https://github.com/AdrianCassar/Xenia-WebServices) is a generic Live-like REST backend for that style of fork. The checked `src/titles` directory has no Destiny title ID `415608F8`, and its README distinguishes it from first-party Xbox services. See `notes/network-options.md` for the code comparison and current gaps.

## Additional primary references

- [Xenia Canary FAQ](https://github.com/xenia-canary/xenia-canary/wiki/FAQ): documents `xenia-vfs-dump [ContainerPath] [DestinationOutput]` as a container extraction tool. This is a fallback if opening or extracting the preserved Xbox container becomes a problem. Check support for the captured container type before relying on it; test on a working copy and keep the main file and any matching `.data` folder together.
- [Xenia Canary Quickstart](https://github.com/xenia-canary/xenia-canary/wiki/Quickstart): current steps for installing captured content and updates through Canary's **Install Content** menu. The FAQ also mentions `$TitleUpdate/[TitleID]/000B0000`, so preserve that USB folder if present.
- [Bungie: Shared World Shooter — Destiny's Networked Mission Architecture](https://www.gdcvault.com/play/1022247/shared-world-shooter-destiny-s): original Destiny networking talk. Describes overlapping peer and cloud service responsibilities; relevant when defining a realistic gameplay target.
- [Bungie.net API repository](https://github.com/Bungie-net/api): confirms that a Destiny 1 web API exists. Its public manifest and account endpoints may help interpret static identifiers, but this API is separate from the console client's in-game service protocol and is not a replacement server.
- [XWine1](https://github.com/XWine1): an Xbox One translation-layer effort and a possible future comparison, but it targets a different Destiny build and cannot use our Xbox 360 capture as-is.

## Order of use

1. Preserve and hash the console installation.
2. Run the working copy in Xenia and record the first reproducible result.
3. If file or package contents are the blocker, inspect extracted `.pkg` files with `tiger-pkg`; evaluate Codeberg tachyscope and QuickTag against the same sample.
4. If sign-in becomes the blocker, investigate the specific Xenia and Destiny behavior before adapting ideas from unrelated Demonware projects.
