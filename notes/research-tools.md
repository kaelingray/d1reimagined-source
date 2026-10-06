# PS3 package and protocol research tools

These build-specific findings were recorded on September 27. Later exports and
acceptance are documented in the dated checkpoints. Keep extracted data outside Git.

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
they match the run-time exports field for field. It also reads the title updates' compiled
layout (5,041 types in 01.24; [`final-build.md`](final-build.md)). With `--packages`, it also reads the 66 web
services from the investment globals. `schema OUT.json` writes the same JSON the stand-in loads,
`type HANDLE` prints one type, and `services` lists each service's request and response
layout ([`investment-services.md`](investment-services.md)). No running game is needed.
`ps3_update.py` lists Sony's PS3 title updates and extracts chosen files from the update PKGs
over HTTP ranges. `d1_builddiff.py` compares two builds:
- types, matched by structure (same handle first, then by field kinds);
- the handles the stand-in uses, with side-by-side field layouts of those that changed;
- web services;
- strings.

`Invoke-FinalBuildExport.ps1` runs the whole 01.00 -> final comparison on the PC
([`preservation-plan.md`](preservation-plan.md)). `d1_eboot.py --elf` works on any build: it
reads the TOC base from the entry descriptor, and applies the curated labels to 01.00 only.

Useful protocol and package reference findings are collected in
[`public-research.md`](public-research.md).
