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

## Test 013 — XHttpStartup diagnostic

- Date: 2026-09-24. Isolated build: `D:\Destiny360\XENIA_TEST013_XHTTP_STARTUP`, using `patches/xenia-xhttp-startup-diagnostic.patch`.
- Input: Working installed game, imported Content 1–3, and captured title update from the separate content root. The window showed `v0.0.23.2`, and the log confirmed the XEX patch applied.
- The diagnostic implementation returned 1 from `NetDll_XHttpStartup` and logged one call. No subsequent `NetDll_XHttp*` call was recorded.
- After the user pressed A, the screen still showed **canary**. This one API return is insufficient to pass the network check. The test does not simulate an HTTP connection or Destiny service.
- Log: `D:\Destiny360\LOGS\Test013-xhttp-startup.xenia.log` (private, outside Git).

## PS3 comparison 001 — Official RPCS3 baseline

- Date: 2026-09-24. Official RPCS3 `v0.0.42-20043-b5a174e9`, Vulkan, PS3 disc game `BLES01857` version `01.00`, with firmware from the user's disc and a private working copy outside Git. No PS3 title update was installed.
- The first boot precompiled 6,650 shader interpreter variants and reached a visually intact "Press X to Play" title screen. This did not show Xenia's missing horizontal bands.
- Player 1 was changed from Keyboard to XInput. RPCS3 detected the user's Xbox controller as `XInput Pad #1`; Xbox A mapped to PlayStation Cross.
- Pressing A displayed Destiny's **canary** message: "No internet connection detected." Gameplay was not reached. The screenshot was supplied by the user; see `notes/ps3-route.md` for the setup and comparison.

## PS3 comparison 002 — Network status connected

- Date: 2026-09-24. Same official RPCS3 installation and PS3 game as comparison 001. Changed only Network Status from `Disconnected` to `Connected`; PSN Status stayed `Disconnected`.
- Pressing A advanced to **buffalo**: "You must be signed into PSN in order to play Destiny." This confirms the first network gate was passed. Gameplay was not reached.
- The current RPCS3 PSN Status menu offers `Disconnected` and `RPCN`. No account setup or private server connection was attempted.

## PS3 comparison 003 — Local RPCN sign-in

- Date: 2026-09-24. Isolated copy of the same official RPCS3 build at `D:\Destiny360\RPCS3_TEST003_LOCAL_RPCN` (baseline install untouched). Network Status `Connected`, PSN Status `RPCN`.
- Sign-in service: RPCN server 1.10.0 (protocol 32, matching RPCS3 `b5a174e9`; 1.11.0 speaks 33 and is incompatible), run from `D:\Destiny360\RPCN_LOCAL` bound to `127.0.0.1` only, email validation and ticket signing disabled. A throwaway account `d1localtest` was created on that local server only with `scripts/rpcn_local_account.py`. No public RPCN account was created.
- First attempt: still **buffalo**, plus an "RPCN unknown error" popup. Cause: RPCN's UDP helper was bound to `[::1]` only, so RPCS3's IPv4 address check on `127.0.0.1:3657` went unanswered. Setting an unparsable `HostIPv6` makes RPCN fall back to `127.0.0.1:3657`.
- Second attempt: **buffalo cleared**. Destiny showed "SIGNING IN", then "Destiny servers are not available. Dismiss this message to try again."
- Log evidence (private: `D:\Destiny360\LOGS\PS3-Test003-local-rpcn.RPCS3.log`): the game called `sceNpManagerRequestTicket2` for service ID `UP0002-BLUS31181_00` and the local RPCN issued each ticket. Its network threads are named `DW Thread` and `_DW HTTP Processor` (Demonware). At boot it looked up `dev-stun.demonware.net`. After each ticket it tried HTTPS (port 443) sign-on hosts in turn: `www8.signon.deadorbit.net`, `www1.signon.gravityshavings.net`, `www1.signon.deadorbit.net`, `www2.signon.gravityshavings.net`, `www2.signon.deadorbit.net`.
- Those names resolved publicly, so these attempts reached Bungie's live sign-on service with a locally issued, unsigned ticket. RPCS3 was stopped once this was noticed. Future local tests should keep the game from contacting live services.
- What this proves: RPCS3 + a local RPCN server satisfies Destiny's PSN sign-in check, and the next gate is Bungie/Demonware sign-on over HTTPS. What remains unknown: the sign-on request and response format, and whether a compatible local service can be built.

## PS3 comparisons 004–013 — Local-only sign-on path

Date: 2026-09-24. All runs used a fresh copy of `D:\Destiny360\RPCS3_TEST003_LOCAL_RPCN` made by `server/prepare_rpcs3_copy.py` (the baseline stays untouched), the same working game copy (`BLES01857` 01.00), and the loopback RPCN server from comparison 003. From 004 on, RPCS3's **DNS address** was `127.0.0.1` and `server/dns_sinkhole.py` answered every name locally, so no run could reach live Bungie/Demonware hosts by name. Tools are described in `server/README.md`; private logs are in each `D:\Destiny360\RPCS3_TESTnnn_*\log` folder and `D:\Destiny360\SIGNON_LOCAL\logs`.

| Test | Change from previous | Result |
| --- | --- | --- |
| 004 `RPCS3_TEST004_SIGNON_PROBE` | DNS sinkhole (all names → 127.0.0.1); sign-on probe on 127.0.0.1:443/80 | New error **marionberry** ("Could not connect to the Destiny servers"). Only `dev-stun.demonware.net` was looked up; no PSN ticket request, no sign-on connection. |
| 005 `RPCS3_TEST005_SIGNON_NETTRACE` | `sys_net`/`DnsHook` trace logging only | Game sends UDP from its port 3074 to the STUN host on 3074: pairs of 3- and 4-byte probes, six attempts ~540 ms apart. With STUN at 127.0.0.1 they loop back to the game's own socket. Also UPnP SSDP to 239.255.255.250:1900. marionberry again. |
| 006 `RPCS3_TEST006_STUN_STUB` | STUN host → 127.0.0.2; `server/stun_stub.py` answers there | **marionberry cleared.** Probes: `1e 03 00` (IP discovery) then `14 02 00 xx` (NAT discovery); stub replies in the Black Ops 4 emulator layout were accepted. Game requested the NP ticket and connected to the probe for `www8.signon.deadorbit.net`, `www1/www2.signon.gravityshavings.net`, `www1/www2.signon.deadorbit.net`. ClientHello: **SSLv3 only**, suites 3DES-SHA, RC4-SHA, RC4-MD5, DES-SHA, export suites, no SNI, no extensions. OpenSSL 3 refused (`VERSION_TOO_LOW`). Screen: "SIGNING IN" → "Destiny servers are not available", fully local. A 29-byte type `0x0e` UDP packet repeats every 15 s to the STUN host (unanswered). |
| 007a/b `RPCS3_TEST007_SSL3_LOCAL_CA` | Pure-Python SSLv3 server; per-host loopback IPs/certs; local CA (CN/O, UTF8String, CRLF) over all 50 firmware roots | Guest access violation at `0x0010723c` ~70 ms after `sceNpManagerGetTicket`, before any sign-on DNS. Reproduced twice. The game reads only `CA01`–`CA05`; the loader stopped after CA01. |
| 008 `RPCS3_TEST008_DNAS_SHAPED_CA` | CA reshaped to C/O/CN with SKI/AKI, only CA01–05 replaced | Same crash. Register dump: `r23 = "ps3_ppu"`, `r24 = "https://www5.signon.gravityshavi…"`, `r29 = 0xd38` — looks like the game's own fatal-error path during sign-on setup. TTY log empty. |
| 009 `RPCS3_TEST009_CA_LF` | LF line endings | Loader still stops after CA01; same crash. |
| 010 `RPCS3_TEST010_CA_PRINTABLE` | PrintableString names (as in the SCEI roots) | Same. |
| 011 `RPCS3_TEST011_CA_SLOT5_ONLY` | Local CA only in CA05; CA01–04 original | CA01–05 all read, but the libssl second pass (CA21 etc., present in 006) never ran; same crash. |
| 012 `RPCS3_TEST012_CA_SLOT1_ONLY` | Local CA only in CA01 | Loader stopped after CA01 again → the loader rejects our root wherever it sits. Same crash. |
| 013 `RPCS3_TEST013_CA_FIXED_DATES` | Validity fixed to 2014-01-01…2037-12-01 | Same stop and crash. |

What this establishes: with DNS, STUN (NAT discovery) and PSN sign-in served locally, Destiny reaches its HTTPS sign-on step without any live service. The sign-on client speaks SSLv3 only, which `server/sslv3.py` implements (self-tested for RC4-SHA and 3DES-SHA). The game trusts only the firmware SCEI DNAS roots CA01–CA05, and the firmware `libssl` certificate loader rejects any replacement file tried so far — after line endings, name shape and string types, and validity dates were matched — so a local CA cannot be injected through `dev_flash`. RPCS3's own HLE `cellSslCertificateLoader` does no validation, but this game uses the LLE `libssl.sprx`. Whether the check is a signature/whitelist test or something else is not yet known.

## PS3 comparison 014 — Game CA patch, firmware roots untouched

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST014_GAME_CA_PATCH` of the comparison 003 baseline, made with `server/prepare_rpcs3_copy.py --game-ca`. Same local DNS sinkhole, STUN stub, loopback RPCN and `signon_probe.py --mode terminate --destiny-hosts` as comparison 006, with the SSLv3 server from 007. Firmware `CA01`–`CA05` left original.
- Static analysis (EBOOT decrypted with `rpcs3.exe --decrypt` into `D:\Destiny360\RESEARCH\ps3_decrypt`, outside Git): at `0x106880` the game calls `cellSslCertificateLoader(0x1f)` for CA01–CA05, then `cellHttpsInit(2, …)` with that buffer **and** a PEM string built into the executable at `0x011b7218` ("Tiger Root Authority", 2013–2033, then GeoTrust Global CA). If either step fails, the game takes its fatal path, which explains the crash in 007–013. The SSL callback registered with `cellHttpClientSetSslCallback` (`0x10816c`) reads each certificate's subject name, then returns `verifyErr` with a masked set of flags cleared, so the game itself refuses untrusted chains.
- Change: an RPCS3 patch (`server/game_ca_patch.py`; hash `PPU-10d41d88e5ce442299c2ba4cb681f0b307f33527`) writes the local `ca_v4` PEM plus a NUL over the embedded string in guest memory. The log confirmed `PAT: Applied patch … (<- 2)`. Game files on disk are unchanged.
- Result: **handshake completed.** After the NP ticket request, the game resolved `www7.signon.deadorbit.net` (→ 127.0.1.7). It accepted the local leaf over SSLv3 RSA_RC4_128_SHA and sent `POST /SignOn?platform=ps3_ppu&build=41375.14.07.03.1501.live` (`User-Agent: PS3Application libhttp/4.6.0-000 (CellOS)`, `X-TransactionID: 0`, `Content-Type: application/octet-stream`, 390-byte body). The probe answered 503 and the game made no further sign-on attempt. The screen showed "SIGNING IN", then "Destiny servers are not available. Dismiss this message to try again." (user screenshot). No crash occurred. The raw request is kept privately under `D:\Destiny360\SIGNON_LOCAL\captures`.
- Body shape (protobuf wire format): field 1 = 2; field 2 = 294-byte NP ticket (version 2.1, the local RPCN account `d1localtest`, service `UP0002-BLUS31181_00`); field 3 = `"en"`; field 5 = a 15-character ID string; field 6 = build `41375.14.07.03.1501.live`; field 7 = 0; field 13 = a 64-bit varint; field 15 = packed varints `0xE0000002, 0xE0000001`; field 17 = `"europe_disc"`; field 19 = `"en"`. Field meanings are not yet known.

## PS3 comparison 015 — Local SignOn success reply

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST015_SIGNON_SUCCESS` (same as 014, game CA patch applied). The one change: `signon_probe.py --mode signon` answers `POST /SignOn` with HTTP 200 and a success message built by `server/signon_proto.py`. The schema comes from the game's nanopb descriptors (request `0x011ca1c0`, response `0x011ca300`; handler `0x126744`). The reply points the game at `127.1.1.127:12850`, where `server/session_listener.py` logs traffic.
- Input: the user was away, so Player 1 in this copy was switched from XInput to the keyboard handler (Cross = X; original saved as `Default.xinput.yml.bak`), and X was posted to the game window from a script. A Windows Firewall prompt for the new `rpcs3.exe` path appeared and was left unanswered; all traffic was loopback.
- Result: **SignOn accepted on the first attempt.** The game did not contact the session address. Instead it resolved `panther-ps3-auth3.prod.demonware.net` (sinkholed to 127.0.0.1) and sent `POST /auth/` with `Content-Type: application/json`, retrying at 1, 2 and 4 s while the probe answered 503. The screen stayed on "SIGNING IN". The raw requests are in `D:\Destiny360\SIGNON_LOCAL\captures`.
- Next gate: Demonware authentication. This session did not continue past it.

## PS3 comparison 017 — Lobby header acknowledged

- Date: 2026-09-24. Copy `D:\Destiny360\RPCS3_TEST017_LOBBY_HEADER`, services as in 016, plus `server/lobby_probe.py` on `127.0.3.1:3074`, which answers the greeting with `server_header_ack` (protocol 210).
- Result (017b): the client accepted the header and sent a 153-byte `client_auth` frame (`AB 82`). The probe did not reply, and the client disconnected after about 10 s (**cattle**). Captures are in `D:\Destiny360\SIGNON_LOCAL\captures\lobby210`. Static analysis of that frame is in `notes/ps3-route.md`.

## PS3 comparison 018 — Lobby authentication completed

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST018_LOBBY_AUTH` of 017. The only change: `lobby_probe.py` derives the `CLIENTCHAL` verify from the auth3 session key and answers `client_auth` with `server_auth_done`. Before the live run, the derivation reproduced the 017b client challenge offline.
- Input: X posted to the game window from a script. The game reached the lobby without further input.
- Result: SignOn and auth3 succeeded as before. In the lobby, the probe logged `client_auth title=5378 challenge_verified=True` and sent `server_auth_done`. RPCS3 shows the client read the full 10-byte frame and began waiting for the next frame. A game job thread closed the socket about 70 ms later, after a `stat` of `/dev_hdd1/downloads/` returned ENOENT. No further DNS lookups or connections followed. The screen showed **termite**: "Failed to download configuration files from Bungie servers, please try again." (RPCS3 screenshot, kept privately under the test folder.)
- Next gate: termite. Leads are listed in `notes/ps3-route.md`.

## PS3 comparisons 019–022 — Title configuration, content manifest, packages

- Date: 2026-09-24. Each run used a fresh copy of 017 (`RPCS3_TEST019_MANIFEST_URL`, `…020_UNSIGNED_MANIFEST`, `…021_MANIFEST_PACKAGES`, `…022_MANIFEST_HASHES`). From 020 on, the copies were made with `prepare_rpcs3_copy.py --game-ca … --unsigned-manifest`. Services were the same as in 018, plus `signon_probe.py --manifest-url http://content.local.test/manifest` (and `--manifest-packages <disc packages folder>` from 021).
- 019: The SignOn configuration supplied the URL. The game sent `GET /manifest`, the probe replied 503, and the screen showed **nightingale**.
- 020: An unsigned, GUID-only manifest was accepted. The screen showed "The game disc cannot be read." The halt text read from guest memory was "failed to load bootstrap_patchable".
- 021: The manifest listed 160 disc packages. The halt text was "Patchable package registration failed with result (-90)".
- 022: Each package entry also carried its header u64. Registration completed, and the game connected to the SignOn session address `127.1.1.127:12850`, sending an 80-byte message with the SignOn token. There was no reply; after 10 s the screen showed **centipede**. Screenshots and logs are kept privately in the test folders and `SIGNON_LOCAL/logs`.
- Details and addresses are in `notes/ps3-route.md`.

## PS3 comparisons 023–026 — BAP session server

- Date: 2026-09-24. Fresh copies of 017 with `--game-ca … --unsigned-manifest` (`RPCS3_TEST023_BAP_SHELLO` … `RPCS3_TEST026_WS_ZERO`). `server/bap_server.py` replaced `session_listener.py` on 127.1.1.127:12850.
- 023: `shello rsp` was accepted, then an encrypted `qz_reg req` arrived (timeout, centipede).
- 024: `qz_reg rsp` was accepted, then `rrc req` arrived.
- 025: `rrc rsp` and `echo rsp` were accepted. `c->ws req` for service 503 went unanswered; after 10 s the screen showed **chicken** ("connection to Destiny character servers lost").
- 026: A zero-filled `c->ws rsp` was received by the client but still ended in chicken.
- Details are in `notes/ps3-route.md`.

## PS3 comparisons 027–027b — Investment sign-in result code

- Date: 2026-09-24. Run with `scripts/Invoke-Rpcs3LocalTest.ps1`, which makes a fresh copy from TEST017 and restarts `bap_server.py` with the given arguments.
- 027b (`--ws-result 5`): the client disconnected immediately after the service-503 reply and showed **chimpanzee** (Bungie account problem). With result 0 (026) the client waited, then showed chicken. The reply is therefore processed; the missing piece comes after sign-in. Details are in `notes/ps3-route.md`.

## PS3 comparisons 028–032 — Investment sign-in, GDB tracing

- 028: after the sign-in reply, pushed empty `self` and `select_character` queuez families. No change (chicken after 10 s).
- 029: GDB trace of milestone/error calls. Chicken is (0x18, 0xa1), raised by the sign-in step's 10 s timeout.
- 030: typed sign-in reply with the account id (`--ws-signin`). No change.
- 031: GDB checkpoints showed the second success predicate (`0x294a5c`, user entry state 3) never passes.
- 032: GDB entry trace of the user-manager range `0x286000`–`0x297000`. Sign-in is started by posting event 0x22.
- Details are in `notes/ps3-route.md`.

## PS3 comparisons 049–062 — Character creation to orbit

- Date: 2026-09-24/25. RPCS3 copy `RPCS3_TEST048_SETUP` (XInput pad, async shaders) reused for every run; logs `SIGNON_LOCAL/logs/TEST05x_*`/`TEST06x_*`.
- 049–054 (Codex): lobby keepalive echo; service 501 (create character) answered with an id; select_character/self families carry the character. Finish still spun. See `notes/checkpoint-2026-09-24-character.md`.
- 054 (idle ~1 h): **caterpillar**. `lobby_probe.py` read at most 97 frames, and 40 s keepalives used them up. The loop is now unbounded.
- 055: GDB showed the 501 reply delegate is the table's no-op; creation succeeded client-side. Killing an attached GDB client froze the emulator: RPCS3's stub serves only the first connection per session. Added `scripts/rpcs3_gdb_daemon.py` (holds the one connection, serves `hits`/`mem` on 127.0.0.1:2346).
- 056: `bap_server.py --store` remembers the created character; the next boot showed **character select** with the Titan (level 0). Pressing it sent service 504 and the screen went black.
- 057–058: the bootflow's character step (`0x32f748`) needs the user entry's active character (+0x20), copied from the self root's u64 at +0x60 (`0x2942fc`). 504 now sets it via a versioned self delta; the step then completed, followed by **centipede**.
- 058b–060: centipede is BAP health condition 15 (connection state != 7). The connection failed with reason 0x12 because replies must answer the oldest pending request (`0x7c3458`), and "acct id trans" (0x17) was unanswered, so the next 701/702 replies matched nothing. (059: leaving 701 unanswered showed "Contacting Destiny Servers" and failed at once, so replies stay on.)
- 061: 0x18 reply (the request echoed plus one u64 account id per name) fixed centipede. The game subscribed to peer (0) and roster (2), called service 104, sent `client config req`, then **pig** (0x18, 0xa2): 104's reply carries the universe object, whose root must be `0x7fffffffffffffff`.
- 062: 104 reply with the universe root plus a `client config rsp` (`{1: 1, 2: 1}`): **the game reached orbit** ("Set Destination", player card d1localtest, level 0).

## PS3 comparisons 063–081 — Server clock, unlock flags, activity join

- 063: universe +8 is the server clock. `--universe-time s` (Unix seconds) turned "trial mode unavailable" into post-launch state (faction ships, Dark Below / House of Wolves news).
- 064–077: account and character unlock flags (self root +0x12a8 table A, +0x16a8+1 table B, character +0x3bc0 table C; tri-state, 2 = true) feed the unlock-expression evaluator (`0xb67480` sources). Bisection: **B0 is the full-game purchase flag**. Story restriction ("temporarily restricted") persisted with all A/B values; C all true skipped it by jumping into a story cutscene and an activity join.
- 078–080: the activity join sends service 105, acct id trans, client config, 104, subscriptions, then **ahp req** (0x10; reply 0x11 is 16 raw bytes, decoder `0x416f14`). Zero reply: unsubscribe, error **currant** after ~0.2 s; patterned reply: 12 s wait, then currant `(0x18, 0x21)` from `0x333050`. No socket connect happened.
- 080–081: at the join the client also sends an encrypted Demonware lobby frame. Envelope (send path `0xa391bc`): `u32le len | AB 85 | u32le seq | IV16 | AES-128-CBC(BDDATA key 0, pt = u32le len + message + pad) | HMAC-SHA1(BDDATA mac key 0, frame[:-8])[:8]`. Captured plaintext (GDB at `0xa396c4`): header `86 8a`, bdMatchMaking (service 0x8a) task 1 (createSession): addresses blob, session params blob with "d1localtest". Unanswered, so the join times out (currant). `server/bdbuffer.py` reads/writes the typed bdByteBuffer.
- Tooling: keyboard pad in the TEST048 copy (user's XInput config saved as `Default.xinput.user.yml.bak`); `Start-LocalStack.ps1`, `Invoke-TitanLogin.ps1` (title to Log In unattended, ~1.5 min), `Invoke-RpcsStep.ps1` (keys + F12 screenshot; PrintWindow captures go stale while the desktop is locked), `Send-RpcsKey.ps1` (real scan codes, holds); GDB daemon `hits ... r5:512` memory dumps.

- 082: lobby_probe decrypts/answers encrypted lobby frames (`LobbyCrypto`). A task reply (type byte 0x01, typed uint64 transaction 0x8000000000000000, uint32 error, uint8 task id, uint32 count, uint32 total) with no results: the client followed up with bdMatchMaking task 3 (deleteSession) of a zero session id and failed the join (currant).
- 083: createSession reply with one 8-byte session-id blob result: the later deleteSession carried **our** id, so the reply format is right. The join now fails differently: **cloudberry** ("Could not connect to the Destiny servers").

- 084–089: the post-ahp wait is the activity-join step; `c->b_ahp` (0x10) is consumed by `0x42a5a0`: 16 bytes = u64 request id (must equal the request's, here 0) | u32 IPv4 | u16 unused | u16 port, stored via `0x1dac14`; then `0x429c64` opens a second BAP connection to that address. (A patterned id was ignored; an all-zero reply gave address 0.0.0.0 and failed at once.)
- 090: ahp reply `0000000000000000 7f010180 0000 3233`: the client connected to a second BAP stand-in (127.1.1.128:12851), shello with field 2 = 2, then sent **`c->ah mgr req`** (0x06, 3103 bytes: activity host manager / host startup request). Unanswered: join fails after ~11 s. Next: the 0x07 reply (decoder `0x415be4`).

- 091–093: second BAP stand-in (activity host proxy, AHP) answers `c->ah mgr req` with **`c->ah mgr rsp`** (0x07): exactly 137 bytes, `02 | u64 host id | char[128]` (decoder `0x415be4`, consumer `0x42a514` -> `0x31181c` strncpy 0x80). With host id 1 and a host string, the client next sends **`c->ah not`** (0x08) via the proxy: `u64 host id | 01 | u32 3 | u32 0xe8 | 232 bytes` (probably the host startup request), then fails with **(0x18, 0x2a)** after ~7 s (previously 0x21). Next: the activity host's replies (host startup response, host identifier, join) through the proxy.

- 094–098: activity host join. A `b->c not` (0x09) pushed on the proxy connection is routed `0x305484 -> 0x42a498 -> 0x258778`, which dispatches on the subtype (table at `0x175cf5c`); host messages are typed structs in the investment bit format (`0x25bd0c` / `0x25bc88`, client->host types at `*(0x179eba4)`, host->client at `*(0x179eba8)`; schema exported to `SIGNON_LOCAL\activity_schema.json`). `c->ah not` is `u64 host id | 01 | u32 type | u32 length | u32 0 | body` (the u32 0 was first mistaken for part of the body). The host join request (type 3) is a `0x80800e98` struct: i32 random token, i64, ids (account and character), player info (u4, `char[128]` e.g. `Fri_09252026_112315_000001@ps3@User@RPCS3-110`, …); the client keeps a copy (`0x259c84`). The **join response** is type 4 (`0xd0d560 -> 0x256ad8`), a `0x80800a58` struct: the request echoed (the token must match), result 0 (3 bits, min 1), `char[128]` session id, then ints. With it the join passed (`0x256dc0`, flags 0x10|0x20): the client then sends types 17 (8 bytes), 37 (1087 bytes) and type 15 every ~33 ms, and fails with **(0x18, 0x2b)**. Failure codes are step index + 0x18 in the table at `0x175eafc`: 0x2a = timed out waiting for join response, 0x2b = **timed out waiting for entity indices**. Entity indices arrive as host type 0 (`0xd0d020`): a `0x80800d64` struct holding a 384-word bitmap ORed into the client's index set (+0x5bf8). `bap_server.py --ah-join [--ah-entities FIRST:LAST]`; `Invoke-JoinCycle.ps1 -AhHost NAME`.
- 099–101: after the join response. **Entity indices** (host type 0, `--ah-entities 8192:12288`) moved the failure to 0x2c (timed out waiting to establish client); `0x25831c` = indices received (+0x61fa). **Established** (`0x258324`) also needs host flag 0x100 (host type 11, body ignored, `0x25715c`) and an activity state: host type 1 (`0xd0d250`), a `0x80800e76` struct whose 0xa20-byte body goes to the simulation (`0x1c95f0`, +0x50b0, present flag +0x50a8) with bool @0 set and bool @1 clear or i32 @0x10 > 0. With `--ah-state` (type 11 + minimal state) the client was established and the join moved on to **loading activity resources**: failure (0x18, 0x36) from `0x336610` (0x36 - 0x18 = 0x1e, `error.loading_activity_resources`), error code **lime**. Meanwhile the client sends host types 17 (8 bytes), 37 (1087 bytes), 21 (172 bytes, player string), 15 (1 byte, ~30/s) and 14 (16 bytes). Next: the activity state fields (@8 u64, @0x60 `0x80800cc5`) the resource loader needs.
- 102–111: activity loading. The descriptor in the activity state (@0x60 @0x994, char[40]) is a **scenario name** looked up by prefix in the resource registry (`0xe72f4`, names in guest memory such as `orbit:scenario_client`, `.map:orbit:root`, `city_tower_default1:scenario_client`, `act_cosmo_m0_highway:scenario_client`, `act_cosmo_m1_dome_a…`, `act_cinematic_queen_101…`). World id: `--ah-world 125` (the i16 in the client's service 105 request) passes the DLC check (`0x4105c4`, "World-transition requires DLC."). The loader (`0x3362c0`, state at +0x30) starts loading the scenario (state 0, `0x1f4a54`), waits for it (state 1, `0x1f4864`), asks `0x30c448` whether to abort (state 3 = fall back to orbit resources, status 0x1f), otherwise computes the activity index (`0x30b698`, global override `*(0x175ea34)`, default 1) and posts **event 0x36** `{index, scenario name, 3 bytes}` (`0x30bdd0`); the world controller's RPC 0 object (0x18ea574, registered for 0x36 at `0x30c54c`) receives it (`0xd24e14` -> `0x30d488`) and stores the index at `*(0x18ea60c)+0x94`, which state 2 needs. RPCs 1/2/3 use events 0x35/0x33/0x34 (event dispatcher `0x262a80`, table `*(0x18cd744)`, poster `0x262bc4`). The loader skips the post when the requested scenario equals the loaded one (case-insensitive `0xba738`), so `--ah-map orbit` never sets the index; `act_cosmo_m0_highway` also ended in the orbit fallback (under investigation). RPCS3 GDB breakpoints only fire at function/block entries.
- 112–158: see `notes/checkpoint-2026-09-25-orbit.md` (activity selection, unlock and permission
  checks, checksum/bubble count, lobster). 158b: with the activity state's zone record set to -1
  (`--ah-zone -1:0x811c9dc5`), the client stays in the activity ~72 s, then **baboon**.
- 159–162: baboon = activity-host status 36 `private_activity_host_host_crashed` (no host message
  within the timeout). `--ah-heartbeat 1` (type 11 every second) removes it: 162 stayed in the
  activity 6+ minutes, on the loading screen (no host simulation stream yet).
- 163: `--ah-zone 105:0x811c9dc5` (bubble 13 sub-slot 1): the world controller loads that zone
  set, loading tasks 0-7 complete; tasks 8-10 wait for the host simulation's zone record.
- 164: + `--ah-world-state 105:1` (type-11 world-state delta): zone set 105 active, loading tasks
  0-8 complete; task 9 still running.
- 165–166: `--ah-roster 0` (type-5 simulation packet with the client in roster slot 0, resent
  with the heartbeat): the client adds the peer; no local player yet (peer table empty).
- 167–176: local player binding traced (machine table set at the world change, cleared by RPC 16
  from an empty group entry; host machine list; RPCS3 GDB has no watchpoints). 174: host type 24
  (per-host index grant) sent at the join had no visible effect. 176: host type 43 naming the
  client = boot list -> error **bass**.
- 177–179: listeners never fire from boot; player creation is event 0xc from the RPC 16 sender,
  gated on the manager's expected-player list (empty) - see the checkpoint note.
- 180: world-state member mask (+0x5028) -> RPC 16 carries our machine, player join event 0xc,
  player pool 1, local user bound. Task 9 now waits for the player's object and a kind-0x10 lookup.
- 181–182: all bubble states 0 / bubble 0's zone set active: no change; task 9 waits for the
  game-mode object's slot byte and the player's world object (not spawned).
- 183: traces: the spawner runs for our player every frame and `0x35793c` returns 0 - the
  player has no simulation player (pla +0x50); the 16 kind-0xc simulation-player entities have
  owner id 0. See the checkpoint note.
- 184: `--ah-sim-player` (owner = peer id, wrong: the binder compares pla +0x18, the player id):
  the object is received into the record buffer but nothing is applied (other records still
  await a baseline).
- 185: `--ah-baselines` for all 22 replicated objects + owner 0x0a3d4a2c8fbea6ba: states
  applied, the local player is bound to simulation player 5, spawn event 0x4c posted; task 9
  and the spawn check wait on the game-mode state (0).
- 186: + `--ah-game-mode 0x4786c0e0:3:2`: **all loading tasks done, loading screen gone** (black
  screen, no unit). Spawn check now fails on the active zone set (105 is not a bubble base).
- 187–188: `--ah-zone 104` / `0` without bubble states: **lobster** (base entries absent).
- 189: `--ah-bubble-states -1,13=0` + zone 104: zone set 104 active, 121 replication records,
  99 without a baseline -> nothing applied.
- 190: `--ah-baselines @FILE` (all 121, dumped with `scripts/rpcs3_replicated_objects.py`):
  loading done, sim player bound; spawn check refuses game-mode state 2.
- 191: game-mode state 3: readiness passes, spawn event state 1; the spawner waits for the sim
  player's entity +0x154 byte (grant).
- 192: grant attempt on the wrong field (real32 +0x10): no change.
- 193: `--ah-sim-player ...:1` (state bool +0x48 = entity +0x18c): the spawner runs the spawn
  body, finds a spawn point, and fails creating the player object: blueprint handle (player
  +0x1428) is -1 because the peer family (0) is empty.
- 194: `--peer-family` (family 0 = account + character summaries): blueprint set, player unit
  created - **the guardian spawns in the mission** (HUD radar, world rendered; "Urgent message"
  dialog and "Level Up" toast on top).
- 195: game config: tiling + write color buffers: same black blocks / exposure drift.
- 196: + read color/depth, write depth: correct lighting at first, black block back after ~1 min.
- 198: narrower self flags `1;2@A0-1023,B0,C0-1023` led to banana (required content) before the
  mission. The F12 screenshot is authoritative; PrintWindow showed a stale shader screen.
- 199: Force CPU Blit enabled with the TEST196 buffer options; RPCS3 crashed at character
  selection (Windows access violation `0xc0000005`, offset `0x3a56cb8`) before entering the
  mission. Setting restored. See the checkpoint for paths and external RSX issue references.
- 200: Allow Host GPU Labels enabled as the single graphics change. Mission reached; lighting
  temporarily even while stationary, then split dark-left/overexposed-right after movement
  (F12 14:12:11, persisting at 14:13:01). Level Up prompt appeared over the broken frame.
  Per-game setting restored to false. Next: obtain an RSX frame capture in a GUI-enabled run.
