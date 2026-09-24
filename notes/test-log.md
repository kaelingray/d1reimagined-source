# Xenia test log

Copy this section for each run. Change one input or setting per run.

## Test 001

- Date and time: 2026-09-22 21:57 BST
- Xenia Canary build or commit: `canary_experimental@74c4e4acb` (2026-09-21)
- Game input path and type (container or extracted XEX): `D:\Destiny360\01_WORKING\test001\Content\0000000000000000\415608F8\00004000\ED4E3CCA83E6F64008DDF957088F3E06` (installed game GOD container; matching `.data` folder present). Xenia identified title ID `415608F8` and version `v0.0.0.2`.
- Title update present: No. The Xbox refused to copy the 173 MB title update.
- Additional content present: Seven compatibility packs were copied into the working capture, but Xenia's default content root is `D:\Destiny360\XENIA\content`; they have not been installed there for this baseline test. The Xbox also refused to copy Destiny Content 1–3.
- `mount_cache` value: `true`
- Other nondefault settings: None intentionally changed from the installed Canary config.
- Last visible screen: Distorted pre-release beta Destiny title screen with horizontal bands, incomplete logo, and “Press A to Play”. After pressing A with an Xbox One controller, Destiny displayed: “No internet connection detected. Please check your network configuration and try again. For information on resolving this issue visit help.bungie.net and search for: canary.” Screenshots: `D:\Destiny360\LOGS\Test001-title-screen.png` and `D:\Destiny360\LOGS\Test001-canary-error.png`.
- Last meaningful log lines: Repeated `undefined extern call to 83323D5C XCustomSetDynamicActions` (2,268 occurrences at first inspection). Earlier warnings reported unimplemented `XamVoiceSetMicArrayIdleUsers` and `IoDismountVolumeByFileHandle`. These log entries are observations; their relationship to the image corruption is unproven.
- Xenia exited, crashed, or remained running: Running and responding after reaching the title screen and canary error. Gameplay not reached. Window closed normally after recording the result.
- Reproducible on a second run: Pending.
- Full log path: `D:\Destiny360\LOGS\Test001.xenia.log`
- Change from previous test: Baseline first boot from verified USB capture.

## Test 002 — HDD content import and title update boot

- Date and time: 2026-09-23, approximately 18:00–18:27 BST
- Xenia Canary build or commit: `canary_experimental@74c4e4acb` (2026-09-21)
- Isolation: Separate `D:\Destiny360\XENIA_TEST002` directory with a copy of the executable and baseline configuration. The original Test 001 environment was not modified.
- Source packages: Working copies under `D:\Destiny360\01_WORKING\test002_packages`; originals remain in the verified HDD archive. Xenia's **Content → Install Content** reported success for `000B0000\tu00000002_00000000` (“Destiny Title Update”).
- Content 1 import: Xenia identified `00000001\1XXXXXXX` as “Destiny Content 1” and type “Saved Game”, then remained at “Preparing” and crashed in `VCRUNTIME140.dll` while reading it. The install log contains repeated `STFS failed to read correct block-chain` warnings. This is a Xenia import failure, not evidence of HDD capture corruption: the source, C: archive, and D: archive hashes match. Content 2, Content 3, and the compatibility packs were not imported in this test. Import log: `D:\Destiny360\LOGS\Test002-content1-install.xenia.log`.
- Game input: The same installed game GOD container from Test 001, `D:\Destiny360\01_WORKING\test001\Content\0000000000000000\415608F8\00004000\ED4E3CCA83E6F64008DDF957088F3E06`, with its matching `.data` folder.
- Title update applied at runtime: Yes. Xenia's window title showed `v0.0.23.2`, and the log says `XEX patch applied successfully: base version: 0.0.0.2, new version: 0.0.23.2`.
- Last visible screen: Distorted Taken King title screen. After the user pressed A on the connected Xbox controller, Destiny again displayed “No internet connection detected” with error code `canary`. The update did not remove the service blocker or visible graphics corruption. Screenshot: `D:\Destiny360\LOGS\Test002-canary-error.png`.
- Xenia status: Running and responding at the error screen. Gameplay not reached.
- Boot log: `D:\Destiny360\LOGS\Test002-TU-only.xenia.log`.
- Change from previous test: Installed and applied the verified HDD title update in an isolated Xenia instance.

## Test 003 — Vulkan graphics comparison

- Date: 2026-09-23, approximately 18:43 BST.
- Xenia Canary build: `canary_experimental@74c4e4acb`, isolated at `D:\Destiny360\XENIA_TEST003_VULKAN`.
- Input: Same installed game and title update as Test 002. The only deliberate change was the graphics backend from Direct3D 12 to Vulkan.
- Result: The updated game reached its Taken King title screen at `v0.0.23.2`. The logo and image still had missing horizontal bands. The log confirms a Vulkan device on the NVIDIA RTX 5070 and that the title update patch applied. Vulkan did not resolve the observed image corruption.
- Log: `D:\Destiny360\LOGS\Test003-Vulkan.xenia.log`.

## Test 004 — Intact Content 3 package alongside the updated game

- Date: 2026-09-23, approximately 18:54 BST.
- Xenia Canary build: `canary_experimental@74c4e4acb`, isolated at `D:\Destiny360\XENIA_TEST004_CONTENT3`.
- Input: Same Direct3D 12 game and title update as Test 002, plus a hash-verified working copy of the intact `3XXXXXXX` CON package in the title's `00000001` content directory.
- Result: Xenia reached the same distorted Taken King title screen at `v0.0.23.2` without a startup crash. The log confirms the title update applied but does not establish that the game mounted or used Content 3. This test does not demonstrate gameplay progress.
- Log: `D:\Destiny360\LOGS\Test004-Content3.xenia.log`.

## Tests 005–006 — First patched importer and clean failure

- Date: 2026-09-23. Source: Canary commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19`, locally built with the filename and contiguous-file changes.
- Test 005 used a separate `D:\Destiny360\XENIA_TEST005_STFS` folder. The normal Content 1 installer stalled on `360_investment_globals_012c_6.pkg`, whose chain appeared to end after one of 36 blocks. The same symptom affected two more noncontiguous files. The test process was stopped after inspection. Log: `D:\Destiny360\LOGS\Test005-patched-install.xenia.log`.
- Test 006 added a zero-byte/failed-read guard to Xenia's extractor and tested in `D:\Destiny360\XENIA_TEST006_STFS_FAILFAST`. The importer returned error `C0000001` instead of stalling. Log: `D:\Destiny360\LOGS\Test006-failfast-install.xenia.log`.

## Tests 007–008 — Patched import succeeds for all three content files

- Date: 2026-09-23. Build: same Canary commit, plus active hash-record selection. All runs used an isolated `D:\Destiny360\XENIA_TEST007_STFS_ACTIVE` content root and the normal Xenia content installer, invoked through a temporary diagnostic entry point. Original captures were read only.
- Test 007 imported `1XXXXXXX`: header result `00000000`, install result `00000000`. The extracted package contains 275 files totaling 86,747,359 bytes; the separate Xenia header copy is 40,960 bytes. The three files that previously appeared truncated have full expected sizes. Log: `D:\Destiny360\LOGS\Test007-active-hash-install.xenia.log`.
- Test 008 imported `2XXXXXXX` and `3XXXXXXX` into the same root. Both header and install results were `00000000`. Content 2 contains 87 files totaling 4,045,557,760 bytes; Content 3 contains 68 files totaling 1,869,400,064 bytes. Logs: `D:\Destiny360\LOGS\Test008-content2-install.xenia.log` and `D:\Destiny360\LOGS\Test008-content3-install.xenia.log`.
- A read-only listing of each source package returned without truncated-chain warnings. The Content 1 listing is `D:\Destiny360\LOGS\Content1-Xenia-patched-vfs-active-hash.txt`; equivalent Content 2 and 3 listings are in `D:\Destiny360\LOGS`. The automated diagnostic entry point was removed from the final source patch.
- This establishes successful import only. Test 009 below checks the game with the imported packages.

## Test 009 — Updated game with all three imported content files

- Date: 2026-09-23. Final local Xenia build from Canary commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19` plus `patches/xenia-content1-stfs.patch`. The temporary import diagnostic entry point was removed and the app rebuilt.
- Isolation: Executable and configuration at `D:\Destiny360\XENIA_TEST009_FULL_CONTENT`; content root points to `D:\Destiny360\XENIA_TEST007_STFS_ACTIVE\content`, which contains imported Content 1–3 and a working copy of the title update. Input is the working copy of the installed game from Test 001.
- The log confirms the title update patched Destiny from `v0.0.0.2` to `v0.0.23.2`. The user pressed A and provided a screenshot showing the same **canary** “No internet connection detected” message. Gameplay was not reached. The title image also remains visibly corrupt.
- Log: `D:\Destiny360\LOGS\Test009-full-content-boot.xenia.log`. Screenshot: `D:\Destiny360\LOGS\Test009-full-content-canary.png`.
- The log records one call to unimplemented `NetDll_XHttpStartup`. In this pinned Xenia source, `UserProfile::signin_state()` always reports `SignedInLocally`, and the `NetDll_XHttp*` exports are listed without implementations. These are concrete emulator limitations to investigate; the test does not isolate which one causes Destiny's canary message.

## Test 010 — Verbose network call trace

- Date: 2026-09-23. Same patched build, content root, title update, and game input as Test 009; isolated at `D:\Destiny360\XENIA_TEST010_NETWORK_TRACE`. Enabled debug-level and high-frequency kernel-call logging for this run only.
- The user pressed A and again saw canary. The full 182 MB log is `D:\Destiny360\LOGS\Test010-network-trace.xenia.log`. A redacted call-count summary is in `notes/network-trace.md`.
- The log records 7,339 calls to `NetDll_XNetGetEthernetLinkStatus`; the pinned Xenia source always returns zero. It also records 52,816 sign-in-state calls, 734 title-address calls, and one undefined `NetDll_XHttpStartup` call. The verbose logger does not record return values, so these values were checked in the pinned source.

## Test 011 — Ethernet-link diagnostic

- Date: 2026-09-23. A separate diagnostic build at `D:\Destiny360\XENIA_TEST011_ETHERNET_ACTIVE` used `patches/xenia-ethernet-diagnostic.patch` with `XENIA_TEST_ETHERNET_ACTIVE=1`. This changed only the Ethernet-link return value to the active bit. The log confirmed the override ran.
- The title update applied to `v0.0.23.2`. After pressing A, the user still saw the same canary message. An active-link answer alone does not solve the network blocker. Log: `D:\Destiny360\LOGS\Test011-ethernet-active.xenia.log`. Screenshot: `D:\Destiny360\LOGS\Test011-ethernet-active-canary.png`.
- The diagnostic source change was removed from the local Xenia checkout after this run. The content-import patch and its tested executable remain separate.

## PS3 comparison 001 — Official RPCS3 baseline

- Date: 2026-09-24. Official RPCS3 `v0.0.42-20043-b5a174e9`, Vulkan, PS3 disc game `BLES01857` version `01.00`, with firmware from the user's disc and a private working copy outside Git. No PS3 title update was installed.
- The first boot precompiled 6,650 shader interpreter variants and reached a visually intact "Press X to Play" title screen. This did not show Xenia's missing horizontal bands.
- Player 1 was changed from Keyboard to XInput. RPCS3 detected the user's Xbox controller as `XInput Pad #1`; Xbox A mapped to PlayStation Cross.
- Pressing A displayed Destiny's **canary** message: "No internet connection detected." Gameplay was not reached. The screenshot was supplied by the user; see `notes/ps3-route.md` for the setup and comparison.

## PS3 comparison 002 — Network status connected

- Date: 2026-09-24. Same official RPCS3 installation and PS3 game as comparison 001. Changed only Network Status from `Disconnected` to `Connected`; PSN Status stayed `Disconnected`.
- Pressing A advanced to **buffalo**: "You must be signed into PSN in order to play Destiny." This confirms the first network gate was passed. Gameplay was not reached.
- The current RPCS3 PSN Status menu offers `Disconnected` and `RPCN`. No account setup or private server connection was attempted.
