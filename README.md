# d1xenia research data

Private companion data for [d1xenia](https://github.com/p3n1cillin/d1xenia).
The source copies remain in `d1xenia/local`; this repository contains a curated subset
for investigating Destiny's PS3 mission replication.

| Directory | Source in `d1xenia/local` | Contents |
| --- | --- | --- |
| `eboot/` | `RESEARCH/ps3_decrypt/` | Decrypted `EBOOT.elf` |
| `schema/` | `SIGNON_LOCAL/` | Current activity, investment, character and world schemas; highway baselines; TEST350 and TEST362 object exports |
| `logs/` | `SIGNON_LOCAL/logs/` | Available TEST364–366 activity and RPCS3 logs |
| `packages/` | `01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages/` | 37 intact disc packages plus parts for three oversized packages, covering Cosmodrome, investment, gear, globals and startup data |

`SHA256SUMS.txt` lists the size and SHA-256 hash of every copied data file. All
37 intact packages and all seven package parts are individually below 100 MB.
The 60 data files total 1,456,577,120 bytes.
`package_names.txt` lists all 247 package filenames from the disc's
`PS3_GAME/USRDIR/packages` directory, including packages not copied into this
repository. It contains names only, one per line.

The Cosmodrome activity package `ps3_cosmo_activities_activity_025c_0.pkg`,
destination package `ps3_cosmo_destination_0274_0.pkg`, and player gear package
`ps3_gear_player_0104_0.pkg` exceed GitHub's per-file limit. Their 64 MiB parts
and original hashes are recorded in
`split_packages.json`. To check the parts, run
`python reassemble_packages.py --verify-only`. To rebuild the original `.pkg`
files, run `python reassemble_packages.py --output-dir <destination-directory>`.
The script verifies each part and the rebuilt package against SHA-256 before
reporting success. Destination packages `0270`, `0271`, and `0272`, and English
globals `0153`, are included intact.

The requested `ps3_gear_player_00fd_0.pkg` was absent from the disc's package
directory and could not be included. The other 24 packages from that request
are present, with `0104` represented by two parts.

Only four requested TEST360–366 log files existed in `SIGNON_LOCAL/logs` when this
bundle was made: `TEST364-rpcs3.log`, `TEST365-rpcs3.log`, `TEST366-ahl.log`, and
`TEST366-rpcs3.log`. There were no matching BAP or lobby logs there. The optional
`oo2core_3_win64.dll` was supplied from the user's Downloads folder. It is an
unsigned 64-bit Windows DLL containing an Oodle decompression export. Its original
download source and distribution terms have not been verified; it was not executed
while preparing this repository.

This repository intentionally excludes private CA keys, the RPCN account database,
firmware, console keys, save data, RPCS3 installations and caches. Keep the repository
private because it contains game binaries and logs.
