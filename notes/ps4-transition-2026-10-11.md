# PS4 01.29 transition - 2026-10-11

The user selected PS4 as the active path and supplied the `d1preservation-main`
snapshot plus owned-console packages. The PS4 tree remains separate from this
PS3 repository. Its server workspace declares GPL-2.0-or-later. This checkpoint
records local setup evidence; upstream August 11 gameplay reports are not local
acceptance and include synthetic memory/code interventions.

## Inputs and the next gate

| Input | Observed result |
| --- | --- |
| Larger US install package | 52,138,213,376 bytes. Existing `pkg-metadata.py` reads CUSA00219, APP_VER 01.00. |
| Smaller initially supplied install package | Header identifies EU CUSA00568, which differs from the update's region. It subsequently disappeared from its supplied path; this work did not delete or move it. |
| Eight update chunks | Parts 0-6 are 4,294,967,296 bytes each; part 7 is 1,402,798,080 bytes. Total 31,467,569,152. Part 0's SFO is CUSA00219, APP_VER 01.29, TARGET_APP_VER 01.28, firmware floor 4.01. |
| Required effective game dump | Missing: the decrypted CUSA00219 01.29 `eboot.bin` and installed Bungie `packages/`. |

The metadata reader passes its three 01.29 checks against the supplied first
chunk, exposed through an ignored junction at its existing reference path.
The 01.33 checks are skipped because that package is absent.

The separately extracted US base `eboot.bin` is 29,276,248 bytes, SELF magic
`4f153d1d`, SHA-256
`ebee5df27aca00057f223b96edab461917e1a7e837b4017b7d6d84b0ae7a2a02`.
The embedded ELF header is at `0x120`; its second LOAD has file offset
`0x18dc000`, virtual address `0x18d8000`, file size `0x149530`.
The pinned image's second LOAD is at file offset `0x18c4000`, virtual address
`0x18c0000`, file size `0x149088`. This layout differs from the project's pinned image.
A SELF wrapper alone does not prove encryption, and package APP_VER alone does
not independently identify the executable. This staged file is not an accepted
decrypted 01.29 image. Extraction was stopped, and partial output remains outside
the pinned dump directory.

The project's required decrypted ELF is 29,148,264 bytes, SHA-256
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`.
Keep that identity gate. Changing the pin to an unidentified build would make
the documented diagnostic addresses and oracle calls unsafe to interpret.

### Follow-up: PC decryption and executable identity

The complete eight-part update is the correct CUSA00219 **01.29 install package**.
The earlier layout comparison examined the separately supplied base package's
executable, not an executable extracted from that update. It does not establish
that the update is the wrong version.

The base executable now has an independently observed build identifier:
`56578.17.04.12.1251.evil`, dated **2017-04-12**, at SELF file offset `0x18e3d5a`.
The reference 01.29 executable is documented as `52553.16.09.28.1551.evil`, dated
**2016-09-28** (`design/content-and-packages.md:1056` and
`design/signon-and-entitlement.md:43` in the supplied PS4 tree).
Thus the base package's APP_VER 01.00 describes its repack metadata; its executable
is a later build than the target. Its exact Sony patch number is not established.
The second LOAD starts 98,304 bytes (`0x18000`) later in virtual memory than the
reference. Do not apply reference codec addresses to this build.

An existing LGPL-3.0 reader, [maxton/LibOrbisPkg](https://github.com/maxton/LibOrbisPkg),
was compiled separately from unmodified source commit
`643477263b2644e0803e0f58b8726ea4e3f3b7d4`, under `F:\d1\tools\liborbispkg\0.2.245`.
No external reader source was copied into either project. Its existing cryptographic
methods and PFS reader were used; no second package-format reader was written.

| Check | Supplied base | Joined 01.29 update |
| --- | --- | --- |
| Shared metadata key digest | Verified | Verified |
| Fake-package image-key recovery and digest | Recovered, 32 bytes, verified | No key recovered |
| Conventional 32-zero repack passcode | Matches | Does not match |
| Wrong-passcode negative control | Rejected | Rejected |

The update fails at the final image-key RSA unwrap with the fake-package keyset;
that method succeeds on the base in the same review. This tests the ordinary repack
route, not every conceivable retail decryption method. No matching retail image
key or passcode was recovered, and no update executable was extracted.

The base's PFS was actually decrypted and its executable selectively extracted
with this second reader at **11:55:32 UTC**. Its SHA-256 exactly matches the earlier
GPL extractor's output. The inner filesystem lists 2,119 files, including 2,105
Bungie content packages; only the executable was copied in this check. All eight
SELF segment headers have encryption and compression flags clear. A plaintext
SELF wrapper does not need cryptographic decryption and is not the pinned ELF.

Private receipts: `pc-decryption-review.json`, `patch-key-envelope-review.json`,
`base-build-identity-review.json` and `pc-decrypt-check/extraction-receipt.json`,
under the PS4 tree's ignored `cfw-staging/2026-10-11`. The library source/build
receipt is beside the separate DLL. No keys were printed or recorded, originals
were opened read-only, and nothing was promoted into the pinned dump directory.
No emulator was launched. The matching decrypted 01.29 input gate remains open.

The user confirmed that a console with GoldHEN FTP is not currently available.
The game-input gate therefore remains the owned patched union dump described
in `capture-rig/CFW-DUMP-RUNBOOK.md`. Its example `ftp_dump.py` is absent from the
supplied snapshot; do not present that invocation as a locally runnable command.
An actual collector must be restored or supplied before a console transfer.

### Follow-up: recovering retained 01.29 content

The user asked to keep trying a rollback. The supplied later repack retains older
Bungie package generations. Its inner PFS inventory contains **2,119 files**,
including **2,105 content packages** and exactly one `eboot.bin`. No earlier
executable, backup ELF or nested archive is listed.

The existing PFS reader exported 320-byte header clips. An original Rust instrument,
installed as a bin in the separate PS4 `d1-pkg` crate, reuses its existing parser,
selection and manifest codec. Five scoped controls pass; 46 headers agree with the
independent GPL extractor. Selecting builds through 52553 gives **1,647 files**,
totalling **49,917,028,352 bytes**. English/neutral selection gives **1,033 rows**,
52,797 bytes, SHA-256
`0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be`:
the exact pinned manifest. This initial result concerns headers, not payloads.

The first payload probe rejected the unmodified LibOrbisPkg output: layout and
length were valid, but both entry-table and block-table hashes failed. The modern
.NET reader returned short decompression reads; the upstream PFSC implementation
called `DeflateStream.Read` only once per sector and ignored its count.
[Microsoft documents this behavior change](https://learn.microsoft.com/en-us/dotnet/core/compatibility/core-libraries/6.0/partial-byte-reads-in-streams).
The first bulk copy was stopped at **309 files / 9,548,271,616 bytes**. Rejected files
and an unfinished partial remain in `recovered-01.29-content`; they are not accepted
and must not be resumed into a correct estate.

`Build-D1PfsReader.ps1` builds a separate LGPL reader with a read-until-filled loop,
an early-EOF error and a short-read counter. Original upstream sources remain
unchanged; modified source, license and source/build receipts remain outside Git.
No cryptographic algorithm or keyset changed. The first corrected 233,091,072-byte
package, `ps4_arch_cabal_0059_0.pkg`, completed **115 short reads** and exactly matches
the independent native extractor's full-file SHA-256:
`0b2bb108849e78a2313831dcc8530b091a0461cb8dec6c2813af35402983c36a`.
The existing Rust reader verifies its tables and **all 1,434 stored-block hashes**.
The uncorrected file remains a discriminating negative control. The corrected
recovery uses a fresh `recovered-01.29-content-v2` folder and a plan pinned to the
reader hash. Full-estate acceptance is recorded below when completed.

The update was also checked for a possible keyless route: its PFS header explicitly
sets `Encrypted`, and the existing reader refuses it without a decryption key.
The digest-verified base image key does **not** verify against the update's image-key
digest, despite their matching content IDs. No retail image key was recovered.
The later base executable is not an inverse patch from which the exact older
executable can be reconstructed by removing the retained content generations.
Executable rollback remains unaccepted; the original build pin is preserved.

Private rollback evidence is under `cfw-staging/2026-10-11`: `repack-file-inventory.json`,
`rollback-header-audit.json`, `rollback-reader-positive-control.json`,
`rollback-payload-probe-v2.json`, `rollback-payload-negative-control.json`,
`rollback-key-reuse-review.json`, `rollback-update-keyless-control.json` and the
recovery plan/ledger/progress files. Header clips, full game files and receipts
containing private input paths are not copied into either Git repository or NAS
production packages. No PS4 native run is claimed.

## PC checks

- F: is NTFS with ample free space. The project was not moved onto exFAT.
- The `d1` WSL account exists (uid/gid 1000, sudo member) and is the default user.
  Mirrored networking remains configured. WSL emits a systemd user-session startup
  warning; it does not prevent the control binaries from running.
- All three Windows firewall profiles are enabled. ActiveStore contains the
  outbound block for the exact portable `shadPS4.exe`, all profiles.
- All 50 unique expected service names resolve to loopback on Windows; the reference
  has 51 redirect rows because the CDN occurs twice. Linux signon resolution also
  returns loopback.
- Portable shadNet/WebAPI targets remain local, UPnP disabled. All 37 firmware-11.00
  module hashes match their local receipt. Upstream used firmware 10.01 modules;
  compatibility has not been established by a game run.
- No existing RPCS3 or shadPS4 process was found before preparation.
- Fifteen Linux executable entrypoints now link the `/opt/d1-target` builds to
  the project's ignored `target/release` paths. This repairs the generator/build
  path mismatch without replacing the separately built Windows `.exe` files.
- Readiness exits 2: decrypted build, installed content, full manifest and validated
  Windows/WSL canonical-launcher adaptation are missing. No native boot is claimed.

## Verification and continuation

Responder controls: **22 PASS**, exit 0. shadNet: **32 PASS, 1 SKIP**, exit 0.
The skip needs emulator source. `d1 controls` has **12 PASS, 1 FAIL**, exit 2;
its real-eboot crash-symbolization check is not accepted without the game input.
The aggregate control helper preserves failures instead of masking their exit code.

The PowerShell helpers parse successfully. A retry with an existing `.partial`
output is correctly refused. The readiness check correctly returns a non-ready
report and exit 2. The merged update completed at **11:21:38 UTC (12:21:38 UK)**
with a verified source/read-back receipt. Its SHA-256 is
`ea50720647dcc4a9d03e47beb931320b35f18a9eb18a05aaee051074726570bb`.
Concatenation is not a decryption or build-identity result.

All three Bash helpers pass syntax checks. The linked `d1-pkg --help` runs through
the generator's expected path. Linux Python imports capstone 5.0.7 and unicorn 2.1.4
with `/opt/d1-tools` on its dependency path.

The artifact-preparation helper follows the existing package/schema/atlas generators
and controls, with the exact decrypted executable identity as its first gate.
On the current tree it refuses the missing executable with exit 2 before generating
anything. That negative check is accepted; successful artifact generation awaits
the actual game input.

After a valid owned dump: follow `BOOTSTRAP.md` to generate the full English
content manifest (52,797 bytes, pinned by the package controls), extract schema
tag `0x80A5E37A`, then pass the dump-dependent tools' controls. Implement the
Windows/WSL adaptation using `run-boot.sh` and its actual delegate
`run-signon-experiment.sh`, with fail-closed containment and recorded settings.
The reference explicitly reports that loopback-only networking was rejected by
the game; mirrored localhost connectivity alone does not establish a usable
guest transport. Resolve that condition in the port before claiming sign-in.

The October 10 PS3 checkpoint remains committed and pushed at `3d1d669`;
no PS3 protocol/gameplay change was made during this transition. This day's
preparation created no new gameplay footage or NAS media package.
