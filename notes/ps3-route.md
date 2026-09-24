# PS3 / RPCS3 comparison track

Started 2026-09-24 at the user's request. The user added a PS3 Destiny ISO under the workspace. The ISO is excluded from Git, as are both game-image folders. It mounts read-only as a UDF volume labelled `DESTINYTHEGAME` and contains `PS3_GAME/PARAM.SFO`, `PS3_GAME/USRDIR/EBOOT.BIN`, and a disc firmware update. A working copy with 257 files (7,927,290,856 bytes) was prepared outside Git under `D:\Destiny360\01_WORKING\ps3_destiny`; the source ISO remains unchanged. Robocopy reported zero failures and excluded the firmware file, which was copied separately.

The official RPCS3 Windows build `v0.0.42-20043-b5a174e9` was downloaded from the [RPCS3 binary release](https://github.com/RPCS3/rpcs3-binaries-win/releases/tag/build-b5a174e933a083389e2f851005e474ccb3e676d9). Its archive matched the adjacent publisher SHA-256 file (`D188BBD3C1D89F060E37376ABF7DE6B2B68379F68DD6B4DA1CD7B4AF75E31273`). It is extracted at `D:\Destiny360\TOOLS\RPCS3\rpcs3-v0.0.42-20043`. Sony's [official firmware page](https://www.playstation.com/en-gb/support/hardware/ps3/system-software/) lists PS3 firmware 4.93. Its linked update host produced a certificate-name mismatch in this environment, so no HTTPS validation was bypassed. The PS3 ISO includes a `PS3_UPDATE/PS3UPDAT.PUP` (268,435,456 bytes), copied privately to `D:\Destiny360\TOOLS\RPCS3\PS3UPDAT-from-disc.PUP` (SHA-256 `C52A0D8C0F4D9FDFE2C488C89EE3E509ECB3C84ABA8288A7A3ED65497F60D051`). That disc firmware may be older than 4.93. RPCS3's firmware installer populated `dev_flash` with 1,064 files. A first launch of the working `EBOOT.BIN` opened a game window identified as Destiny `BLES01857`; the visible in-game result is pending.

This route has relevant precedent: an [RPCS3 issue for Destiny BLES01857](https://github.com/RPCS3/rpcs3/issues/12802) reports reaching character selection with private servers in a modified build, although it also documents a graphics crash. The [RPCN compatibility list](https://wiki.rpcs3.net/index.php?title=RPCN_Compatibility_List) lists Destiny as unavailable through generic RPCN and says private servers are required. [cohaereo/rpcs3-v4nguard](https://github.com/cohaereo/rpcs3-v4nguard) is a Destiny-focused RPCS3 fork, but its latest listed prerelease and repository activity are from September 2022. It is an older research lead rather than a current drop-in fix. These sources establish that modified RPCS3 and private servers have been used for Destiny; they do not establish a public, working Destiny backend or a playable result on this machine.

The first visible screen said "Precompiling shader interpreter variants" and began building 6,650 base variants. RPCS3 remained responsive while this ran. It then reached Destiny's "Press X to Play" title screen. The logo and background looked intact in the user's screenshot, a clear improvement over the missing horizontal bands in the Xenia title screen. RPCS3's Player 1 handler initially defaulted to Keyboard; changing it to XInput detected the user's Xbox controller as "XInput Pad #1", with A mapped to PlayStation Cross. Pressing A displayed Destiny's **canary** error: "No internet connection detected." The screenshot confirms that the base game, without a PS3 title update installed, reaches the same network gate as Xenia. RPCS3 listed the disc as version 01.00 and an available update as 01.24; the update was not installed for this baseline test.

In a second run, the same official RPCS3 installation was changed only from Network Status **Disconnected** to **Connected**, leaving PSN Status **Disconnected**. Pressing A then advanced from canary to **buffalo**: "You must be signed into PSN in order to play Destiny." This isolates a passed network-status check and a new PSN sign-in gate. The current RPCS3 UI lists only **Disconnected** and **RPCN** under PSN Status; it has no local simulated sign-in choice. No RPCN account was created or connected during these tests.

After RPCS3 closed normally, its private log recorded both run configurations: `Internet enabled: Disconnected` / `PSN status: Disconnected` for the first run and `Internet enabled: Connected` / `PSN status: Disconnected` for the second. The second run initialized `sceNp` and attempted a UDP bind on port 1001. Repeated `ENOTCONN` socket messages appeared later while the game was at the error screen. Those messages do not establish the cause of buffalo; the on-screen text explicitly names the missing PSN sign-in. The full log remains outside Git at `D:\Destiny360\TOOLS\RPCS3\rpcs3-v0.0.42-20043\log\RPCS3.log`.

The fork's August 2022 commit [`1f065d3`](https://github.com/cohaereo/rpcs3-v4nguard/commit/1f065d3) adds a `sys_net_bnet_sendmsg` implementation and Destiny-specific texture-cache crash workarounds. Its September 2022 commit [`08e2b9d`](https://github.com/cohaereo/rpcs3-v4nguard/commit/08e2b9d) disables some verification failures and changes graphics code. Those changes target later network and rendering failures; the fork does not document a bundled Destiny service. The modern official build already renders the title correctly, so an old fork binary is not the next useful diagnostic until a compatible backend or post-login failure is available.

A local RPCN server (see `notes/test-log.md`, PS3 comparison 003) cleared buffalo. Destiny then requested an NP ticket for service `UP0002-BLUS31181_00` and tried Demonware/Bungie HTTPS sign-on hosts (`www*.signon.deadorbit.net`, `www*.signon.gravityshavings.net`), ending at "Destiny servers are not available." The public V4NGUARD custom-server project demonstrated Destiny on RPCS3 reaching the Tower but has not published its server.

Next: determine whether a usable Destiny PS3 service endpoint exists and what RPCN/PSN sign-in it requires. Only then test authentication or later gameplay in a separate RPCS3 installation. Do not push the ISO, firmware, extracted game, emulator binary, save data, or raw network traces.

## Local service stand-ins (2026-09-24, comparisons 004–013)

A loopback-only toolset in `server/` now replaces each gate the game hits before sign-on, and keeps every name the game resolves on this PC (`server/README.md`):

1. **DNS** — `dns_sinkhole.py`, set as RPCS3's DNS address, answers everything locally.
2. **Demonware STUN / NAT discovery** — the game probes `dev-stun.demonware.net:3074` from its own UDP port 3074. With no answer it shows **marionberry**. `stun_stub.py` answers IP discovery (type 30 → 31) and NAT discovery (20 → 21) using the layout in [project-bo4/shield-development `stun_server.cpp`](https://github.com/project-bo4/shield-development/blob/6b8f7ce/source/proxy-dll/demonware/servers/stun_server.cpp) (GPL-3.0; format reference only, no code copied). Destiny accepts it and moves on. The stub runs on 127.0.0.2 because RPCS3 holds the game's `0.0.0.0:3074`.
3. **PSN sign-in** — loopback RPCN, as in comparison 003.
4. **HTTPS sign-on** — the client offers only SSL 3.0 (3DES/RC4/DES/export suites, no SNI). `sslv3.py` implements the server side; `signon_probe.py` gives each sign-on hostname its own loopback address so the right certificate is chosen without SNI.

**Current blocker: certificate trust.** Destiny loads only firmware roots `CA01`–`CA05` (SCEI DNAS Root 01–05; LF-terminated PEM, PrintableString C/O/CN, SHA-1, 2048-bit). The firmware `libssl.sprx` loader reads a replacement root and stops, whatever slot it occupies; the game then hits a fatal-error path (`0x0010723c`, `r23 = "ps3_ppu"`) while preparing the sign-on URL. Matching line endings, subject structure, string types, extensions and validity dates did not help. RPCS3's HLE `cellSslCertificateLoader` (in `rpcs3/Emu/Cell/Modules/cellSsl.cpp`) simply concatenates files, but this game uses the LLE module.

Next options, most promising first:

- Find the rejecting check in the decrypted LLE `libssl.sprx` (RPCS3 can decrypt firmware modules) and apply an RPCS3 patch in the isolated copy so the loader accepts a local root.
- Leave the DNAS roots intact and instead patch the game's HTTPS verification (it imports `cellHttpClientSetSslCallback` and `cellSslCertGet*`), so any chain is accepted for the sign-on hosts only.
- Try RPCS3's HLE `libssl` for the loader only — unlikely to work, because the LLE `libhttp` depends on the real SSL implementation.

Once the handshake completes, `signon_probe.py` will save the first sign-on request under `D:\Destiny360\SIGNON_LOCAL\captures` for protocol work.

## Sign-on reached (2026-09-24, comparison 014)

The certificate blocker was in the game, not the firmware. Destiny passes `cellHttpsInit` two CA buffers: firmware CA01–CA05, and a PEM string built into the EBOOT (`0x011b7218`) that holds Bungie's own "Tiger Root Authority" and GeoTrust Global CA. Replacing firmware roots broke the first buffer and crashed the game. Leaving those roots alone and patching the embedded string in guest memory with the local CA (`server/game_ca_patch.py`, an RPCS3 patch) keeps every verification step on and lets the SSLv3 handshake complete.

The game now sends its first sign-on request to the local probe: `POST /SignOn?platform=ps3_ppu&build=41375.14.07.03.1501.live`, a 390-byte protobuf body that wraps the NP ticket with language, build and region fields (`notes/test-log.md`, comparison 014).

A fallback exists if a later host needs it: the SSL callback's return at `0x108224` (`extsw r3, r27`) could return 0 and accept any chain. That fallback is not needed now.

The `SignOn` exchange uses nanopb 0.1.x messages. Their field tables are compiled into the EBOOT, so `server/signon_proto.py` documents both messages. A local success reply was accepted in comparison 015. The game then moved on to Demonware authentication (`POST /auth/` on `panther-ps3-auth3.prod.demonware.net`), which is the current gate.

## Demonware auth3 request (comparison 015 captures)

After the local SignOn reply, the client looks up `panther-ps3-auth3.prod.demonware.net` (the EBOOT also names `lobby`, `auth` and `auth3` hosts for the `prod`, `cert` and `dev` environments; all nine now resolve to `127.0.3.1`–`9`). It sends one request per attempt over the same SSLv3 client as SignOn:

- `POST /auth/`, `Content-Type: application/json`, `X-TransactionID: 0`, about 490 bytes. The URL is built from the format string `%s://%s:%d/auth/`.
- Body: a flat JSON object whose values are all **strings**, even the numeric ones:
  - `auth_task`: `"18"` in every attempt.
  - `title_id`: `"5378"` in every attempt.
  - `iv_seed`: a decimal 32-bit value that changes on every attempt.
  - `extra_data`: a JSON document encoded as a string, `{"token": "<base64>"}`. The token decodes to the same 294-byte NP ticket that the SignOn body carries (version 2.1, issued by the local RPCN for `d1localtest`, service `UP0002-BLUS31181_00`).
- When the reply is 503, the client resends: five requests in all, arriving at about 0, 0.1, 1.2, 3.3 and 7.4 s. No further request was captured, and the screen stays on "SIGNING IN".

The request-side strings sit together in the EBOOT near `0x12ca360` (`auth_task`, `iv_seed`, `title_id`, `extra_data`, the URL format, `application/json`). The client classes are named `bdAuth`, `bdAuthPS3` and `bdAuthService`. The raw requests are kept privately in `D:\Destiny360\SIGNON_LOCAL\captures`.

## Auth3 accepted (comparison 016)

`server/auth3_proto.py` implements a synthetic local reply, enabled with `signon_probe.py --mode auth3`. Request task 18 expects **response task 19**, with `code=700` and the request's `iv_seed`. All three are emitted as decimal strings. Inspection of `0xa2c7a8` shows the JSON integer reader accepts both string and numeric nodes (types 1 and 2); the string constructor at `0xa2d954` writes type 1. The PS3 handler at `0xa264f8` supplies expected task 19 and decoded size 145 to the generic handler at `0xa24b94`.

The tickets are base64-encoded plaintext at this stage. Client ticket = packed 128-byte `bdAuthTicket` followed by 17 bytes of PS3 information; server ticket = opaque 128-byte local token. The key and token are generated per reply and kept together outside Git for later lobby work. No real PSN identity is validated by this diagnostic responder.

Important layout distinction from `0xa2c1dc`: the **wire** ticket has no native alignment padding. Its format is `<IBIIIQQ64s24s3s4s`: magic `0xEFBDADDE`, type, title, issue/expiry timestamps, license/user IDs, name, session key, reserved bytes, hash. The session key is 24 bytes at **wire offset 0x61**, copied to **object offset 0x68**. The PS3 extension starts at wire offset 128; two u32s and three byte flags are consumed. Their semantics and the remaining padding are still unverified; this test used zeros.

At 15:10 on 2026-09-24, an isolated `D:\Destiny360\RPCS3_TEST016_AUTH3` copy completed local SignOn and received one successful auth3 reply. It then resolved the lobby to `127.0.3.1` and connected to TCP 3074. The listener received 28 bytes: five little-endian u32 values `200, 200, 210, 210, 65535`, followed by eight changing bytes. With no lobby response, it disconnected about 9.5 seconds later and showed **cattle**, "Lost connection to partner services." This demonstrates acceptance of auth3 and entry into lobby negotiation, not completion of lobby authentication or gameplay.

Evidence: private `SIGNON_LOCAL/logs/test016-{probe,dns,lobby}.log`, request/session files in `SIGNON_LOCAL/captures`, and the lobby binary capture. The synthetic HTTP integration test preceded the game attempt and is distinguishable by its 56-byte request on port 80; the actual game request was 491 bytes over SSLv3 on port 443. Two wire-contract tests also passed, including invalid task/title/seed rejection.

## Lobby authentication accepted (comparisons 017–018)

Comparison 017b answered the lobby greeting with `server_header_ack` (`lobby_probe.py`). The client replied with a 153-byte `client_auth` frame, then timed out with **cattle** because the probe stayed silent. The client code at `0xa3ad10`/`0xa3af98` (BLES01857 01.00) shows the full exchange, and the 017b capture reproduces it byte for byte:

- **Framing.** Each frame is a u32le length followed by the body. A first body byte of 1 or 2 marks an encrypted frame. Any other value, such as `AB`, is plaintext with a 1-byte header followed by the message type. The receive limit is the greeting's fifth u32 (65535).
- **`client_auth` (`AB 82`).** The body is an LSB-first `bdBitBuffer`: header byte `0x07`, a few type/flag bits, title ID 5378 as a u32 at bit 14, the auth3 `iv_seed` as a u32 at bit 51, the 128-byte auth3 server ticket at bit 83, and 5 padding bits. An 8-byte challenge follows. Because the ticket is bit-packed, a byte search does not find it.
- **Key derivation.** The transcript is the greeting minus its two 200 markers, the whole `server_header_ack` frame, and the `client_auth` frame without its challenge. Then `H = SHA1(transcript)`, `PRK = HMAC-SHA1(key=H, msg=24-byte auth3 session key)`, and `HKDF-Expand(PRK, "CLIENTCHAL", 16)`. The KDF at `0xa37308` is RFC 5869 expand with HMAC-SHA1, confirmed against RFC test case 4. Bytes 0–7 are the challenge the client sends. Bytes 8–15 go to connection object +0x1f0.
- **`server_auth_done` (`AB 83`).** The 8-byte payload must equal the saved bytes 8–15. `0xa3a4e0` compares them. A mismatch logs "Server replied with an invalid verify"; a match logs "Completed authentication procedure. Connected to MatchMaking" and sets the connection to READY. The frame is `0a000000 ab 83 <verify>`.
- **Frame keys.** `HKDF-Expand(PRK, "BDDATA", 72)` produces two 20-byte values (object +0x200 and +0x214) and two 16-byte keys for the two cipher objects (+0x1f8 and +0x1fc). Their roles, presumably MAC and cipher keys for the two directions, are not yet traced.

`lobby_probe.py` now selects the auth3 session by proof: the session key must reproduce the client's challenge. It then sends `server_auth_done` and saves the derived values privately beside the capture (`lobby-keys.json`, outside Git).

**Comparison 018** (`D:\Destiny360\RPCS3_TEST018_LOBBY_AUTH`, a copy of 017) changed only the lobby probe. SignOn, auth3, greeting and `client_auth` ran as in 017b. The probe verified the challenge and sent `server_auth_done`. The RPCS3 trace shows the client reading all 10 bytes, then waiting for the next frame header, which is consistent with the READY path. About 70 ms later a game job thread closed the socket. The client made no further DNS lookups or connections. The in-emulator screenshot shows a new error, **termite**: "Failed to download configuration files from Bungie servers, please try again." Cattle no longer appears.

Leads for termite (not yet confirmed):

- The SignOn handler (`0x126744`) stores optional response fields in the sign-on singleton at `0x18a44f8`. A debug routine at `0xb6ba4c` labels two of them: field 5 (u64, +0x41f0) is `silo_id`, and field 6 (string[64], +0x4204) is `signon_server_reported_content_version` ("none reported" when absent). Field 4 is `{1 u64 → +0x4460, 2 u32 → +0x4468, 3 bytes}`. Its bytes are decoded by the callback `0x126fec` → `0x1214e8` using a large configuration message with built-in defaults (descriptor `0x11c7677`). The local SignOn reply leaves fields 4, 5 and 6 unset.
- The executable's error-ID table (`0x175eaf8`, 242 entries) includes `error.content_manifest_not_provided` (100), `content_manifest_url_invalid` (101), `no_cdn_connection` (98) and `bungie_server_connection` (96). The name-to-code mapping for termite has not been found; the display text lives outside the EBOOT.
- A `stat` of `/dev_hdd1/downloads/` (ENOENT) ran just before the close. The nearby strings are DLC-related (`offer_key.edat`, "DLC Data"), so it is probably a side effect.
