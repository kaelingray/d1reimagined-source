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

The user confirmed that a console with GoldHEN FTP is not currently available.
The game-input gate therefore remains the owned patched union dump described
in `capture-rig/CFW-DUMP-RUNBOOK.md`. Its example `ftp_dump.py` is absent from the
supplied snapshot; do not present that invocation as a locally runnable command.
An actual collector must be restored or supplied before a console transfer.

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
