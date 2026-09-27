# d1xenia research data

Private companion data for [d1xenia](https://github.com/p3n1cillin/d1xenia).
The source copies remain in `d1xenia/local`; this repository contains a curated subset
for investigating Destiny's PS3 mission replication.

| Directory | Source in `d1xenia/local` | Contents |
| --- | --- | --- |
| `eboot/` | `RESEARCH/ps3_decrypt/` | Decrypted `EBOOT.elf` |
| `schema/` | `SIGNON_LOCAL/` | Current activity, investment, character and world schemas; highway baselines; TEST350 and TEST362 object exports |
| `logs/` | `SIGNON_LOCAL/logs/` | Available TEST364–366 activity and RPCS3 logs |
| `packages/` | `01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages/` | Ten small disc packages: bootstrap, manifests, Earth shared data, Cosmodrome activity and destination data |

`SHA256SUMS.txt` lists the size and SHA-256 hash of every copied data file. All ten
packages are individually below 100 MiB. The 25 data files total 303,954,016 bytes.

Only four requested TEST360–366 log files existed in `SIGNON_LOCAL/logs` when this
bundle was made: `TEST364-rpcs3.log`, `TEST365-rpcs3.log`, `TEST366-ahl.log`, and
`TEST366-rpcs3.log`. There were no matching BAP or lobby logs there. The optional
`oo2core_3_win64.dll` was not found in the project files and is not included.

This repository intentionally excludes private CA keys, the RPCN account database,
firmware, console keys, save data, RPCS3 installations and caches. Keep the repository
private because it contains game binaries and logs.
