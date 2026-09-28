# 01.24 PS3 live boot checkpoint — 2026-09-28

This records the first final-build runs on the owner's Windows PC. Raw logs,
screenshots, sign-on requests, game files, keys, and package data remain under
`local/` and are excluded from Git. The source repo contains only the stand-in
changes and this account. The `d1xenia-data` repo has the previously exported
ELF, type and service schemas; no new game binaries are pushed here.

## Setup

- Main repo baseline: `8fb0ce1` after fast-forwarding `master` from origin.
- Data repo baseline: `0d6693f` on `main`; corrected 01.24 schema has 5,041
  types, package-derived service map has 94 services.
- PS3 title: BLES01857, exact 01.24 EBOOT and 1,034 required update package
  files (10,305,603,584 bytes) installed in isolated RPCS3 copy
  `local/RPCS3_FINAL124_SMOKE001`. The package inventory and sizes passed
  `d1_final_preflight.py`.
- The isolated copy has both local RPCS3 patches for the 01.24 PPU hash:
  private local CA and unsigned content manifest. Its controller config is
  `Default.keyboard.automation.yml`; `Invoke-RpcsStep.ps1 -Keys X` presses
  Cross without the physical controller. The user's ordinary RPCS3 setup is
  not the test copy.
- Local stack: loopback RPCN, DNS, STUN stub, SSLv3 SignOn/manifest,
  Demonware lobby probe and BAP stand-in. The 01.24 title config uses a
  binary override format, not the 01.00 protobuf. It sets override `0x65`
  (manifest GUID) and `0x66` (manifest URL). Live GET `/manifest` verifies
  those values reach the client. SignOn field 14 reports `127.0.0.1` in
  the later runs.
- The `scripts/Start-FinalBuildSmoke.ps1` launcher now builds a diagnostic
  manifest from the disc plus the latest installed patchable update file
  per English/neutral package ID. This yields 311 entries. Marker-zero
  shared-manifest packages are excluded. It is a local hypothesis for the
  content manifest, not a recovered production manifest.

## Live runs

| Run | Change | Visible result | Recorded technical result |
| --- | --- | --- | --- |
| 001 | Initial 01.24 boot, XInput profile | Taken King title | Controller unavailable for autonomous input. |
| 002 | 01.00-format title config, keyboard profile | RPCS3 crash after Cross | PPU `0x008d32f8`; game string nearby: `Client config decoding failed. Check for invalid overrides.` |
| 003 | Omit SignOn config | `termite` | No config URL to download. |
| 004 | Empty binary config, magic `0x123456`, terminal zero ID | `termite` | Parser accepts binary wrapper; no URL. |
| 005 | 01.24 binary GUID/URL config; disc-only manifest (160 packages) | `marionberry` after Verifying Content | Client fetched `/manifest` (HTTP 200, 7,925 bytes); only 12 package opens, no BAP. |
| 006 | Map new 01.24 STUN names to local stub | `marionberry` | New `00 01 00 01` UDP probe reaches stub; stub gives no response. |
| 007 | Add SignOn response field 14 = `127.0.0.1` | Passed Verifying Content to Preparing, then RPCS3 crash | 192 package-open events; PPU `0x0012a4a4`, read access violation at `0x8`. Disc `ps3_globals_0151_0.pkg` was opened immediately before the crash. No BAP/lobby connection. |
| 008 | Same as 007, replace only manifest package ID `0x0151` with update `_4` | Black loading screen; no crash during capture | Client opened `ps3_globals_0151_4.pkg` from update folder. 179 package opens and 292 `ENOTCONN` log entries during the captured window; no BAP. |
| 009 | Same as 007, manifest covers latest patchable updates and disc-only IDs (311 entries) | Black loading screen; no crash during capture | 325 package opens across 317 unique paths, including update `ps3_client_bootstrap_patchabl_0172_2.pkg` and `ps3_globals_0151_4.pkg`; 165 `ENOTCONN` entries, no BAP. |

The comparison between 007 and 008 strongly implicates manifest package
selection in the crash: the only intended protocol change was the package
entry for `0x0151`, the client used its update file, and the crash vanished
within the observed window. This does **not** prove that `0x0151` was the
sole fault or that our union manifest matches the retail server response.
Both 008 and 009 remained on black loading screens while RPCS3 was still
running; the network retries may be relevant but are not yet a proven cause.
The repeated `ENOTCONN` is specifically RPCS3's `sys_net_infoctl(cmd=6)`
on UDP socket 51, bound to `0.0.0.0:3074` with remote `0.0.0.0:0`.
It starts before SignOn, so its large count alone is **not** evidence that
this is the loading blocker.

The 01.24 client queries four regional `destiny-stun` names and eight
`stunN.signon` names under each of `deadorbit.net` and
`gravityshavings.net`; the local DNS maps these to `127.0.0.2`. It sends
the four-byte `00 01 00 01` probe and later 29-byte `0e 02 ...`
packets from port 3074. The older `stun_stub.py` understands 01.00 request
types 20/30 only, and logs both new packet shapes without answering. The
01.24 STUN response format remains unknown.

## Evidence kept locally

- Per-run raw logs: `local/SIGNON_LOCAL/logs/FINAL124_SMOKE00*-{rpcs3,signon,dns-queries,stun,bap,lobby}.log`.
- Structured indexes with each raw file's byte count and SHA-256, HTTP
  request lines, DNS results, unique STUN datagrams, package-open paths,
  socket-error counts, fatal lines, and screenshot fingerprints:
  `local/RESEARCH/ps3_updates/run-index/FINAL124_SMOKE00*.json` (runs
  001–009; 003/004 lack a preserved RPCS3 log). Runs 005–009 index every
  screenshot taken during that run.
- HTTP request captures (private, may contain account material):
  `local/SIGNON_LOCAL/captures/`; each relevant SignOn log names its capture.
- 007 crash screen: `local/RPCS3_FINAL124_SMOKE001/screenshots/BLES01857/BLES01857_screenshot_2026_09_28_19_25_20.png`.
- 008 black loading screen: same folder, `BLES01857_screenshot_2026_09_28_19_35_51.png`.
- 009 black loading screen: same folder, `BLES01857_screenshot_2026_09_28_19_40_58.png`.
- Diagnostic manifests: `local/RESEARCH/ps3_updates/manifest_0151_override.bin`
  (160 entries) and `manifest_final_union.bin` (311 entries).
- Full package download transcript:
  `local/RESEARCH/ps3_updates/fetch-final124-resume.log`.

`scripts/summarize_final_smoke.py` regenerates a local JSON index after
copying RPCS3's log to the per-run log name. Example:

```powershell
Copy-Item local/RPCS3_FINAL124_SMOKE001/log/RPCS3.log local/SIGNON_LOCAL/logs/MY_RUN-rpcs3.log
python scripts/summarize_final_smoke.py --logs-dir local/SIGNON_LOCAL/logs --run MY_RUN `
  --screenshot local/RPCS3_FINAL124_SMOKE001/screenshots/BLES01857/SCREENSHOT.png `
  --out local/RESEARCH/ps3_updates/run-index/MY_RUN.json
```

## Resume and next checks

The prepared RPCS3 copy and packages are reusable. To repeat run 009:

```powershell
pwsh -NoProfile -File scripts/Start-FinalBuildSmoke.ps1 `
  -DataRepo ../d1xenia-data `
  -Rpcs3Dir local/RPCS3_FINAL124_SMOKE001 `
  -GameUsrdir local/01_WORKING/ps3_destiny/PS3_GAME/USRDIR `
  -Name FINAL124_NEXT -Run
$env:D1_RPCS3=(Resolve-Path local/RPCS3_FINAL124_SMOKE001).Path
pwsh -NoProfile -File scripts/Invoke-RpcsStep.ps1 -Keys X -WaitSeconds 20
```

1. Inspect the repeated `ENOTCONN` source in RPCS3's network trace and the
   01.24 ELF. Determine whether the client is waiting for a STUN reply,
   a BAP connection, a content request, or another local service. Avoid
   assuming all `ENOTCONN` lines explain the black screen.
2. Recover the 01.24 `00 01 00 01` / `0e 02` STUN response format from the
   game's parser or a lawful capture from the owner's console. The older
   01.00 format is different. Record exact request/response bytes and
   validate with one changed variable per run.
3. Check why the 311-entry manifest still causes disc package opens after
   loading the updated bootstrap, and whether the manifest needs additional
   offer or localization fields. Compare the package-open sequence between
   008 and 009 in the saved indexes.
4. Once BAP is reached, use `types_01.24.json` and
   `services_01.24.json` from the data repo, preserve all BAP and activity
   host captures, and index each run before making handler changes.

Keep raw run data under `local/`; never stage private keys, account data,
decrypted ELFs, packages, or full captures in the code repository.
