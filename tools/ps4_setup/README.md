# PS4 preparation helpers

Original Windows/WSL preparation code for the separate, user-supplied
`F:\d1\d1preservation-main` tree. The PS4 server and format reader remain in that
separate tree; only our original helper code and a minimal crate patch live here.
No emulator, game or firmware code is vendored. Follow that project's documentation for
builds, dump identity, generated artifacts and canonical boot settings.

| Helper | Purpose |
| --- | --- |
| `Join-D1Update.ps1` | Join the eight supplied CUSA00219 01.29 chunks in numeric order. Verify sizes, source hashes, CNT magic and a full output read-back hash. Refuse existing output or partial output. |
| `Test-D1PS4Readiness.ps1` | Read-only configuration/input review with a private JSON report. Check the pinned executable and content manifest, local shadNet settings, effective firewall block, redirects and module receipt. Exit 2 when incomplete. |
| `wsl-controls-ps4.sh` | Run responder, shadNet and d1 controls in Ubuntu, retaining individual logs and actual exit codes. Fail the aggregate if any tool fails. |
| `Install-D1WSLEntrypoints.sh` | Connect the 15 built Linux tools under `/opt/d1-target` to the ignored paths expected by the documented generators. Refuse unrelated existing files; preserve Windows `.exe` files. |
| `Prepare-D1PS4Artifacts.sh` | Require the exact decrypted 01.29 identity before using the existing package/schema/atlas generators and their controls. Refuse missing or unidentified game inputs. |
| `Build-D1PfsReader.ps1` | Build the separately retained LGPL reader with a PFSC read-until-filled loop for modern .NET. Pin the original PFSC source hash, retain modified source and all compiler-input hashes, and refuse replacing an existing build. |
| `Export-D1RepackHeaders.ps1` | Export 320-byte Bungie header clips through the existing Sony/PFS reader, plus a source-size inventory. Clips are not an installed content estate. |
| `d1-pkg-header-audit.rs` | Install as a bin in the separate PS4 `d1-pkg` crate. Reuse that crate's parser, manifest selection/codec and block verification. Five scoped controls gate a header audit; `probe` verifies an extracted package's tables and every stored block. |
| `Export-D1RepackContent.ps1` | Recover the 1,647 older packages selected by an exact-reference header audit. Require the corrected PFSC reader, record its hash in the recovery plan, retain partial files and resume only matching, individually hash-checked output. It does not accept payloads or recover an executable. |
| `Test-D1RecoveredContent.ps1` | After complete recovery, run the existing pinned package controls, all table/stored-block checks, manifest generator and direct manifest controls. Refuse incomplete or existing validation output; record content-only acceptance after every gate passes. |
| `Test-D1RepackInodes.ps1` | Inspect the existing PFS reader's super-root and parsed inode objects for files outside the game directory and unlinked file records. Require the known executable positive control; report the scope separately from raw-sector carving. |
| `Apply-D1PkgOwnershipFix.ps1` / `d1-pkg-local-owner.patch` | Apply our GPL-2.0-or-later fix to the separate `d1-pkg` crate, guarded by before/after source hashes. Local blocks retain their exact file identity when engine packages share an ID/generation. The existing hash verifier is retained; eight positive/negative ownership controls join its `controls` command. |

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

The later PC decryption review used separately installed
[maxton/LibOrbisPkg](https://github.com/maxton/LibOrbisPkg), LGPL-3.0, source commit
`643477263b2644e0803e0f58b8726ea4e3f3b7d4`. It recovered and verified the base's
repack image key and independently extracted the same executable. That executable
identifies itself as build `56578.17.04.12.1251.evil`, later than the target.
The joined 01.29 update's image key was not recovered by the repack route.
See `notes/ps4-transition-2026-10-11.md` for controls, private receipt paths and limits.
Neither external tool is vendored here, and the preparation identity gates remain.

## Content rollback from the supplied repack

The repack retains earlier package generations. The header audit selects builds
through 52553 and reproduces the pinned English manifest; full-file recovery must
then pass the existing `d1-pkg` controls and `verify --blocks` before acceptance.
The corrected reader addresses [the .NET partial-read behavior](https://learn.microsoft.com/en-us/dotnet/core/compatibility/core-libraries/6.0/partial-byte-reads-in-streams).
Original LGPL sources remain unchanged. Modified PFSC source, LGPL license and
build receipts remain beside the separate library, outside both project repos.

```powershell
# Copy the Rust source into d1-server/crates/d1-pkg/src/bin/ in the PS4 tree,
# then install the guarded change and build from that workspace:
& F:\d1\tools\Apply-D1PkgOwnershipFix.ps1
cargo test -p d1-pkg --bin d1-pkg-header-audit
cargo test -p d1-pkg --lib block_ownership_positive_and_negative_controls_pass
cargo build --release -p d1-pkg --bin d1-pkg-header-audit --bin d1-pkg
# Existing header output and DLL builds are deliberately not overwritten:
& F:\d1\tools\Build-D1PfsReader.ps1
& F:\d1\tools\Export-D1RepackHeaders.ps1
# Audit command (redirect stdout to the private rollback-header-audit.json):
.\target\release\d1-pkg-header-audit.exe audit --dir ..\cfw-staging\2026-10-11\rollback-headers --independent ..\cfw-staging\2026-10-11\base-extracted\CUSA00219\packages
& F:\d1\tools\Export-D1RepackContent.ps1
# On a matching completed/checkpointed plan, use -Resume.
& F:\d1\tools\Test-D1RecoveredContent.ps1 -Output F:\d1\d1preservation-main\cfw-staging\2026-10-11\rollback-content-validation-v2
```

The initial unmodified-.NET recovery is rejected and retained as
`cfw-staging/2026-10-11/recovered-01.29-content`. The corrected recovery uses
`recovered-01.29-content-v2`; do not resume or publish files from the first folder.
The original game packages are opened read-only. All recovered content stays
private. See the transition note for the actual acceptance result and remaining
executable gate; a content rollback does not recreate the old executable.

The first full scan reported 1,119 block failures, caused by the original
`Estate::owner` selecting the `_unpatchable_0` twin for the `_unp3_0` file.
Checking the three `_unp3_0` files alone passes all 1,119 hashes. The patch's
regression fails against the old resolver and passes against the fix; changed
payloads and inverted digests remain rejected. The first validation directory is
retained as failed evidence. Use a fresh output directory for any new validation.
The full crate tests require populated default dump and manifest paths: the
supplied tests treat an existing empty packages directory as an estate to test.

The corrected full scan now passes **all 1,647 package tables/layouts and 701,164
stored-block hashes**. The final **28 controls / 0 skips** pass. Content-only
acceptance and installation receipts are in `rollback-content-validation-v2`.
The default packages path is an NTFS junction to the validated private estate;
the empty predecessor is preserved. The manifest is installed at the documented
path with its exact reference hash. All **19 library tests and 2 audit-bin tests**
pass against these installed defaults. This does not satisfy the executable or
native-launcher gates. No game inputs are tracked or copied into NAS packages.
