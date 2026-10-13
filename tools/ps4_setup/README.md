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
| `Install-D1WSLEntrypoints.sh` | Connect the 16 built Linux tools under `/opt/d1-target` to the ignored paths expected by the documented generators. Refuse unrelated existing files; preserve Windows `.exe` files. Build the added `d1-platform` first. |
| `Prepare-D1PS4Artifacts.sh` | Require the exact decrypted 01.29 identity before using the existing package/schema/atlas generators and their controls. Refuse missing or unidentified game inputs. |
| `Build-D1PfsReader.ps1` | Build the separately retained LGPL reader with a PFSC read-until-filled loop for modern .NET. Pin the original PFSC source hash, retain modified source and all compiler-input hashes, and refuse replacing an existing build. |
| `Export-D1RepackHeaders.ps1` | Export 320-byte Bungie header clips through the existing Sony/PFS reader, plus a source-size inventory. Clips are not an installed content estate. |
| `d1-pkg-header-audit.rs` | Install as a bin in the separate PS4 `d1-pkg` crate. Reuse that crate's parser, manifest selection/codec and block verification. Five scoped controls gate a header audit; `probe` verifies an extracted package's tables and every stored block. |
| `Export-D1RepackContent.ps1` | Recover the 1,647 older packages selected by an exact-reference header audit. Require the corrected PFSC reader, record its hash in the recovery plan, retain partial files and resume only matching, individually hash-checked output. It does not accept payloads or recover an executable. |
| `Test-D1RecoveredContent.ps1` | After complete recovery, run the existing pinned package controls, all table/stored-block checks, manifest generator and direct manifest controls. Refuse incomplete or existing validation output; record content-only acceptance after every gate passes. |
| `Test-D1RepackInodes.ps1` | Inspect the existing PFS reader's super-root and parsed inode objects for files outside the game directory and unlinked file records. Require the known executable positive control; report the scope separately from raw-sector carving. |
| `Apply-D1PkgOwnershipFix.ps1` / `d1-pkg-local-owner.patch` | Apply our GPL-2.0-or-later fix to the separate `d1-pkg` crate, guarded by before/after source hashes. Local blocks retain their exact file identity when engine packages share an ID/generation. The existing hash verifier is retained; eight positive/negative ownership controls join its `controls` command. |
| `d1-platform.rs` | Install as a bin in the separate `d1-pkg` crate. Reuse its manifest digest/pins for platform controls, a private canonical launch contract, contained live server checks and a narrowly scoped FTP bootstrap collector. No game format parser is added. |
| `Apply-D1PS4Preparation.ps1` / `canonical-preparation.patch` | Hash-guarded changes to the existing `run-boot.sh` and its delegate. Add explicit `--plan` and `--server-check` modes, exact executable/manifest gates and owned server cleanup. Native title taps respect `OPERATOR_PAD=1`; driver cancellation stops its delayed child and FIFO writes have a deadline. |
| `Prepare-D1PS4Launch.ps1` | Windows orchestration of those canonical modes. Record effective settings after the creator overrides; verify server child shutdown after a server-only run. Never start an emulator or controller. |
| `d1-byte-scan.rs` | Generic bounded byte-range planning and scanning in the existing `d1-pkg` crate. Reconstruct the complete plan, require a pinned full known-executable control, and reject altered/missing ranges or existing outputs. Nine controls and three unit tests pass on Windows and Linux. |
| `Export-D1RepackSlack.ps1` | Read-only adapter to the existing licensed PFS reader. Export only nonpayload logical image ranges and the known executable control into fresh private staging; require the corrected reader hash and contiguous uncompressed file model. No new package parser or executable acceptance path. |
| `build-registry.patch` | Our GPL-2.0-or-later change to the PS4 tree: `capture-rig/builds.json`, the `d1-build` crate and `capture-rig/d1build.py`. `D1_BUILD` selects the executable build (default `later-56578`; `01.29` stays the address reference). Executable, manifest and patch gates read the registry; address-bearing tools refuse builds without re-derived addresses. Same change as the public `ps4/` tree. |
| `Export-D1RepackFull.ps1` | Full private extraction of the owned repack's `/uroot` through the corrected LGPL reader: per-file `.partial` → length check → SHA-256 → rename → ledger, resumable with a matching plan, `SHA256SUMS` and `manifest.json` written only when complete. |
| `Test-D1BuildContent.ps1` | Re-hash a completed extraction against its `SHA256SUMS`, then run the existing `d1-pkg verify --blocks`, `stats` and default-policy English `manifest` for a registered build. The 01.29 golden controls are reference pins and are not run here. |
| `wsl-servers-loopback.sh` | Run the existing shadNet server and `d1-responder` in WSL bound to **127.0.0.1 only**, with the canonical responder settings and a fresh lab certificate; refuse busy ports or any non-loopback listener; stop only its own children when `<out>/stop` appears. |
| `Start-D1ContainedBoot.ps1` | Contained Windows boot of the selected build: refuse unless all firewall profiles are on, the exact emulator has an enabled outbound block, redirects resolve to loopback, no emulator is running and the launch/manifest identities match the registry. Starts the loopback servers, launches shadPS4 with `-i` and no client patch, taps Cross at the title, and keeps logs, window frames and the guest's own crash report in private staging. |
| `Export-D1RepackSceSys.ps1` | Copy the repack's unencrypted package-entry system files (`param.sfo`, icons, `changeinfo`, trophy) into the extraction's `sce_sys/`; encrypted entries are never read. Records `SHA256SUMS.sce_sys`. |
| `make-job-helper-guard.py` | Generate the **synthetic** build-56578 null job-worker guard (`patches/CUSA00219-56578-job-helper-guard.xml`) after verifying the ELF hash, hook bytes and padding cave. Pass it with `Start-D1ContainedBoot.ps1 -ClientPatch`; runs that use it are `synthetic_ancestor`. |
| `shadps4-0.19.0-local-ip.patch` | **D1 fork-local shadPS4 change** against tag `v.0.19.0` (`c7e065d1`, the exact commit of the binary in use). `RetrieveIp` honours `D1_GUEST_IP` only if it is an address of this machine; otherwise it uses a UDP route probe (no packet) instead of a TCP connect to `1.1.1.1:80`. `sceNetCtlGetInfo(IP_ADDRESS)` returns `NOT_AVAIL` instead of a fabricated `127.0.0.1`. This follows the 01.29 project's fork fix for `marionberry`. Also maps a single-kind NaN `V_CMP_CLASS_F32` mask (56578 uses QuietNan `0x2`) to `IsNan` instead of aborting. Not an upstream fix. |
| `dump-codec-descriptor.py` | Dump a Destiny PS4 codec descriptor table (23-byte field records), resolving nested-message pointers from `DT_SCE_RELA`. Used to show 56578's SignOn response schema (`0x1910CC0`) is unchanged from 01.29. |
| `d1-responder-56578-signon-fallback.patch` | GPL-2.0-or-later change to the PS4 tree's `d1-responder`: for a SignOn request with build `56578.*` only, append engine-config sparse key `0xF1048 = 1` (use the SignOn external address when STUN fails). Without it build 56578 stops at `package_registration` with `transport_method_invalid` (marionberry). |
| `Build-D1ShadPS4.cmd` | Build the D1 shadPS4 (tag `v.0.19.0` + `shadps4-0.19.0-local-ip.patch`) with the official LLVM 23.1.3 `clang-cl`, Discord RPC and updater off, `HAVE_ENDIAN_H=OFF` for the vendored LibreSSL. |
| `d1-synthetic-gate.rs` | Install in the standalone `d1-trace` crate. Reuse its record encoder/decoder to generate the public gate-control source fixture with synthetic registers; two positive/rejection controls and one unit test pass. No native input is read. |

## Retarget to build 56578 (12 October)

The exact-01.29 executable pin was dropped on 12 October; see the
[retarget checkpoint](../../notes/ps4-retarget-2026-10-12.md). In the PS4 tree,
`git apply` `build-registry.patch` (the private tree is verified to be exactly its
result). Then `D1_BUILD` selects a build; the default is `later-56578`, and `01.29`
remains the address reference. Rows above that mention the exact 01.29 gate now
read their identities from `capture-rig/builds.json`.

```powershell
& F:\d1\tools\Export-D1RepackFull.ps1            # -> dumps\later-56578-CUSA00219 (resumable)
& F:\d1\tools\Export-D1RepackSceSys.ps1          # param.sfo, icons, changeinfo -> sce_sys
& F:\d1\tools\Test-D1BuildContent.ps1 -Output <new private dir>
& F:\d1\tools\Test-D1PS4Readiness.ps1 -Report <new private json>
& F:\d1\tools\Start-D1ContainedBoot.ps1 -Name <run-name>   # refuses on any containment gap
```

`Start-D1ContainedBoot.ps1` refuses to launch when hosts redirects are missing.
`-AllowUnredirected` overrides that and records it in the receipt; the firewall
block still applies. Re-run `contain-d1.cmd` as administrator after
`d1-hosts.txt` changes.

The default paths match this user's Windows installation. Installed copies are
under `F:\d1\tools`; source copies here were hash-compared with them on 2026-10-11.
Preparation output lives in the PS4 tree's ignored `cfw-staging/2026-10-11` directory.
Do not commit any output, install packages, decrypted dump, module files or capture.

The [12 October recovery review](../../notes/ps4-recovery-2026-10-12.md) records
the complete 73,943,032-byte logical nonpayload scan and inspection of the
original second owned package. Neither yielded the exact 01.29 executable.

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

## Preparation while the executable is unavailable

Build the original `d1-platform.rs` helper in the separate PS4 crate before
installing the 16 entrypoints. The minimal canonical patch refuses unrelated or
mixed source revisions; it is idempotent only for the exact resulting revision.

```powershell
Copy-Item F:\d1\tools\d1-platform.rs F:\d1\d1preservation-main\d1-server\crates\d1-pkg\src\bin\d1-platform.rs
& F:\d1\tools\Apply-D1PS4Preparation.ps1
# From the PS4 d1-server workspace, using its configured toolchains:
cargo test -p d1-pkg --bin d1-platform
cargo build --release -p d1-pkg --bin d1-platform
# Also build the Linux bin into /opt/d1-target/d1-server with the existing WSL toolchain.
wsl -d Ubuntu-24.04 -u d1 --exec bash /mnt/f/d1/tools/Install-D1WSLEntrypoints.sh
& F:\d1\tools\Prepare-D1PS4Launch.ps1 -Mode Plan -Name review-0129
& F:\d1\tools\Prepare-D1PS4Launch.ps1 -Mode ServerCheck -Name services-0129
```

Each name/mode gets a new private directory under `cfw-staging/launch-preparation`.
Existing output is refused. Plan records the canonical environment after all
overrides and identifies Linux settings that need adaptation. ServerCheck runs
the existing shadNet/responder inside the existing namespace, with loopback and
dummy `10.77.0.2/24`, no default route and redirected hosts. It checks real sockets,
HTTP/WebAPI/TLS manifest bytes, untrusted TLS rejection and 404 handling, and verifies
both server children are gone. It does not exercise a guest client, activity logic,
or the Windows transport port. Those acceptance fields remain false.

On 11 October: **10 platform controls** pass separately on Windows and Ubuntu;
**2 unit tests** pass on each platform. The actual canonical server run passes
**19/19** checks and child cleanup. Wrong manifests, missing/wrong executables,
reused output and unrelated canonical-source changes are refused. The creator
plan actually withholds the profile and changes the push family to `4`.
Readiness remains **12 PASS / 2 FAIL**, exit 2: exact eboot and native Windows/WSL
runtime acceptance are still open.

The supplied default profile fixture exists (20,318 bytes); its presence does not
establish native acceptance. The stock Windows emulator also lacks the supplied
project's private probe hooks. Do not forward Linux Vulkan or controller paths
blindly, or treat a Linux network namespace as containment of a Windows executable.

After the 11 October daily cutoff, the original unconditional title-input task
was corrected: operator mode or `PRESS=0` creates no driver. Six actual-function
checks with a real Linux FIFO verify autonomous taps, those two no-input modes,
delayed child cleanup, preservation of an unrelated child and bounded writes
without a reader. The game remains untested; these checks exercise input
orchestration only. This later evidence belongs to the next eligible daily report.

When the owned console becomes available, `d1-platform pull-bootstrap` can retrieve
only the exact missing executable and its metadata into a new private staging
directory. **The console must already be isolated according to the supplied
project's documentation**; this helper neither sets up nor proves that isolation.
Give its literal private IPv4 address and GoldHEN FTP port (default 2121). It disables
proxy/config/glob behavior and ignores the FTP PASV host address. Transfers retain
partials on failure and refuse replacing files. The executable's exact size/hash
must pass before promotion. `param.sfo` is staged and hashed, but its version is
explicitly unverified. No file is installed automatically, and no console transfer
has been tested while the console is unavailable.

```powershell
& F:\d1\d1preservation-main\d1-server\target\release\d1-platform.exe controls
# Only after console isolation and FTP are ready: use pull-bootstrap --help
# for --ip, --port and a new --name; never substitute the later executable.
```

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
