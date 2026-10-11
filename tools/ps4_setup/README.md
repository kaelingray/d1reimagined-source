# PS4 preparation helpers

Original Windows/WSL preparation code for the separate, user-supplied
`F:\d1\d1preservation-main` tree. No upstream PS4 server, format reader, emulator,
game or firmware code is copied here. Follow that project's documentation for
builds, dump identity, generated artifacts and canonical boot settings.

| Helper | Purpose |
| --- | --- |
| `Join-D1Update.ps1` | Join the eight supplied CUSA00219 01.29 chunks in numeric order. Verify sizes, source hashes, CNT magic and a full output read-back hash. Refuse existing output or partial output. |
| `Test-D1PS4Readiness.ps1` | Read-only configuration/input review with a private JSON report. Check the pinned executable and content manifest, local shadNet settings, effective firewall block, redirects and module receipt. Exit 2 when incomplete. |
| `wsl-controls-ps4.sh` | Run responder, shadNet and d1 controls in Ubuntu, retaining individual logs and actual exit codes. Fail the aggregate if any tool fails. |
| `Install-D1WSLEntrypoints.sh` | Connect the 15 built Linux tools under `/opt/d1-target` to the ignored paths expected by the documented generators. Refuse unrelated existing files; preserve Windows `.exe` files. |
| `Prepare-D1PS4Artifacts.sh` | Require the exact decrypted 01.29 identity before using the existing package/schema/atlas generators and their controls. Refuse missing or unidentified game inputs. |

The default paths match this user's Windows installation. Installed copies are
under `F:\d1\tools`; source copies here were hash-compared with them on 2026-10-11.
Preparation output lives in the PS4 tree's ignored `cfw-staging/2026-10-11` directory.
Do not commit any output, install packages, decrypted dump, module files or capture.

```powershell
& F:\d1\tools\Join-D1Update.ps1
& F:\d1\tools\Test-D1PS4Readiness.ps1
wsl -d Ubuntu-24.04 -u d1 -- bash /mnt/f/d1/tools/wsl-controls-ps4.sh
wsl -d Ubuntu-24.04 -u d1 -- bash /mnt/f/d1/tools/Install-D1WSLEntrypoints.sh
# After supplying the exact owned decrypted dump:
wsl -d Ubuntu-24.04 -u d1 -- bash /mnt/f/d1/tools/Prepare-D1PS4Artifacts.sh
```

The readiness report currently has an explicit unvalidated Windows/WSL-launcher
gate. It is a preparation report, not a replacement for `run-boot.sh` and its
containment procedure. There is no successful PS4 game boot yet.

The separately installed package extractor is
[AzaharPlus/shadPS4Plus](https://github.com/AzaharPlus/shadPS4Plus), GPL-2.0,
`PKG_EXTRACTOR_1_1`. It is not vendored. Its downloaded Windows archive matched
the SHA-256 release digest published by GitHub; the source/license receipt is local.
