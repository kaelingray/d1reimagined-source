# PS4 retarget to the owned build 56578 — 12 October 2026 (evening)

On 12 October the user dropped the exact-01.29 executable pin, superseding the
11 October "keep exact 01.29" decision. The PS4 line now targets the executable
the owned repack actually contains. 01.29 stays registered as the **address
reference**: every fixed address in the PS4 tree's `design/`, probe-site
catalogues and BAP tables was measured on it. No native gameplay is claimed.

## The build we have

| | |
| --- | --- |
| Executable label | `56578.17.04.12.1251.evil` (April 2017) |
| SELF (launched) | 29,276,248 bytes, `ebee5df27aca00057f223b96edab461917e1a7e837b4017b7d6d84b0ae7a2a02` |
| ELF (static tools) | 29,249,016 bytes, `672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833` — the new pin |
| Package metadata | `param.sfo` VERSION **01.33** (APP_VER 01.00 is the repack's own), every `changeinfo` `app_ver="01.33"` |

The package metadata says 01.33, the end-of-life patch. That agrees with
aknumbers' 11 October video, which runs an Xbox One build "56578" against a server
set to "PS4 1.33". The executable has not yet reported its own version, so the
registry keeps `app_version` empty and records `package_version` separately.

aknumbers' GitLab still has only the 11 August snapshot and no tags. On 9 October
he wrote that the 1.33 build can be used once a public release ships. Nothing was
downloaded.

## Full extraction

`Export-D1RepackFull.ps1` extracted the repack's whole game directory (`/uroot`)
through the corrected LGPL reader, with the original opened read-only:
**2,119 files, 52,738,749,800 bytes, 2,105 Bungie packages**, finished
21:49:37 UTC. Every file went `.partial` → length check → SHA-256 → rename →
ledger; `SHA256SUMS` and `manifest.json` were written only at the end. The
`eboot.bin` matches the pinned SELF. The earlier inode review counted 2,120
regular files; the extra one is `/flat_path_table`, outside the game directory.
The brief mentions 2,158 files; that count was not reproduced here.

`param.sfo`, icons, `changeinfo` and the trophy file live in the package's entry
table, outside the PFS image. `Export-D1RepackSceSys.ps1` copied the 17
unencrypted entries into `sce_sys/` (`SHA256SUMS.sce_sys`); encrypted entries were
not read. Its `param.sfo` is byte-identical to the independent extractor's copy.
The partial Codex extraction and the 01.29 junction were left untouched.

## English manifest

The existing default policy (all generations, neutral plus English) selects
**1,392 packages, 71,003 bytes**,
`50f2b13866773d7143f65218c3205179147406ac3d32d50c536028d9de515bb0`. That is
above the **1,325-descriptor cap measured on the 01.29 client**; the 56578 cap is
not yet measured. The newest-generation policy selects 408 packages. With
`D1_BUILD=01.29` the same command still reproduces the pinned 1,033-package,
52,797-byte reference manifest exactly.

Full content validation (`Test-D1BuildContent.ps1`, 13 October 07:09–07:27 UTC):
all 2,119 extracted files re-hash to their recorded SHA-256 values, and
`d1-pkg verify --blocks` passes **2,105 packages and 964,151 stored-block SHA-1
hashes with 0 failures**. The regenerated manifest is byte-identical to the
recorded pin. An earlier attempt was stopped to give a boot a quiet machine, and
another was interrupted overnight during block verification. Both are kept as
incomplete evidence and neither is counted.

## Build parameter

`capture-rig/builds.json` in the PS4 tree, read by a new `d1-build` crate and
`capture-rig/d1build.py`. `D1_BUILD` selects a build; the default is
`later-56578`. Changes are in `tools/ps4_setup/build-registry.patch`, the same
edit as the public `ps4/` tree:

- `d1-eboot`, `d1-bap`, `d1-oracle`, `d1` (crash symbolizer), `d1-pkg`,
  `d1-harness`: default image, packages, manifest and launch paths come from the
  selected build. The 01.29 constants remain as the reference.
- `run-boot.sh`, `d1-platform`, readiness, artifact preparation: executable,
  manifest and patch gates read the registry. A build without its own client
  patch gets no patch; the 01.29 lab patch is never applied to another build.
- Address-bearing data keeps its derivation hash. Probe-site catalogues,
  `launch_knowledge.json`, the surface registry and the diagnostic catalogue
  stay pinned to 01.29 and fail their existing build gate on 56578. The
  diagnostic catalogue writes non-reference output to a per-build folder.
- The Python Oodle harness loads the 01.29 ELF by fixed path and calls fixed
  01.29 decoder addresses, so it fails closed until re-derived.

Tests: `d1-build` 3/3 (Windows and Linux), `d1-harness` 13/13 (after expanding
the registry variable in its run-boot comparison), `d1-platform` 2/2 on Linux.
The canonical patch installer recognises the new `run-boot.sh` state.

## First static look at 56578

`d1-eboot controls` on the new ELF: **21 PASS / 40 FAIL, exit 2**, as intended
for a build without re-derived addresses. Everything address-independent passes:
build registered, 2,633 RTTI types all named, 115 investment BAP message classes,
NID oracle, FNV-1 name hashes, family 4/4 instance checksums, and the
**character, roster and 503-response schemas**. The **account and profile schemas
fail**, so serialisation for those two may have changed. The world-controller
ladder is re-found from RTTI (35/35 handlers), with moved addresses, e.g. rung 28
update `0x7A6D60` → `0x7a8710`. `d1-bap controls` refuses at the 01.29 identity
gate (8 failures), as designed. Receipts:
`cfw-staging/2026-10-12/retarget-static`.

## First contained boot

Containment was checked first: all three firewall profiles on; the enabled
outbound block for the exact `shadPS4.exe`; no emulator running. The 50 original
redirects resolve to loopback. The new executable adds four service literals
(`auth.api.sonyentertainmentnetwork.com`, `s.ac.playstation.net`,
`ps4.scedev.net`, `graph.facebook.com`). They are added to `d1-hosts.txt` but
**not yet applied**: re-run `contain-d1.cmd` as administrator. The boots ran
with `-AllowUnredirected`, recorded in each receipt; the firewall still blocks
any non-loopback connection.

Servers ran in WSL, bound to 127.0.0.1 only (`wsl-servers-loopback.sh`).
A mechanics check confirmed Windows reaches 80, 443, 31313 and 31315 on
loopback, the LAN address is refused, the WebAPI serves the manifest bytes, and
both servers stop on request. shadPS4 0.19.0 started with `-i` and no client patch.

Result, two runs (the second on a quiet machine):

- Content gate passes: `CUSA00219 · Destiny · App Version 01.00` (repack
  APP_VER). shadPS4's own NP login to the local shadNet succeeds
  (`D1Guardian`, accountId 1).
- The game's code runs: modules load (eboot at `0x51c0000`), job threads start,
  graphics initialise on the RTX 2060 SUPER, the splash screen is hidden, the
  first shaders compile, and the game writes `reserved_crash_file`/`test.dat`.
- Without the guard, about 30–50 s in, the game's **Graphics Heartbeat** thread faults:
  `0xc0000005` at executable address **`0xFAAF4`**, deterministic across both
  runs. No title screen, no `/SignOn` and no request to the responder. No guest
  crash report was written.

Cause, from the new ELF. The heartbeat thread (entry `0x7E6420`, created through
`0xF7C90`) loops in `0x7DD060` until a frame counter (`[0x26E4840+0x20]`) reaches
its target. While behind, it calls `0xFB860` → `0xFAAC0` to run one pending job
itself, and sleeps 300 µs (import call, `edi=0x12C`) when there is none.
`0xFAAC0` reads the thread's job-worker pointer from TLS `+0x58`, which only
worker threads set (the `0xFAD30` worker loop). The TLS segment has no
initialised data, so a non-worker's pointer is null and `mov rsi,[rax+8]` at
`0xFAAF4` faults. The PS4 project recorded the same address on 01.29 under
Linux as a load-sensitive race; here it is deterministic. Enabling shadPS4's
pipeline cache did not help (the fault came sooner, about 10 s in), so the
setting was restored exactly.

**Synthetic guard.** `make-job-helper-guard.py` writes
`patches/CUSA00219-56578-job-helper-guard.xml` (synthetic client modification;
everything reached through it is `synthetic_ancestor`). It verifies the ELF hash
and the original bytes. A 13-byte hook at `0xFAAE7` jumps to unreferenced
padding at `0x290992` (46 bytes between a `ret` and the next prologue; no xrefs
or call-graph targets). There it calls the TLS getter. If the worker is non-null,
it runs the original instructions and returns to `0xFAAF4`. If null, it sets
`ebx=2` and jumps to the function's own exit at `0xFACF6`, which returns "no job
run" through the normal canary check and register restores. The heartbeat then
takes its existing 300 µs sleep path. A patched scratch copy disassembles as
intended. shadPS4 0.19 silently ignores a patch entry without
`AppElf="eboot.bin"`, and `-i` also drops an explicit `-p`, so with a client
patch the orchestrator omits `-i` and instead requires the automatic patch
folder to be empty. Three unapplied attempts (`guard`, `-b`, `-c`) are retained.

**Guarded run `20261013-073442-later-56578-guard-d`** (both patch lines logged as
applied): no fault for the full 8 minutes; the orchestrator stopped it.

- The game prints its own build, `build:string: '56578.17.04.12.1251.evil'`,
  and a networking session id.
- Window frames show the DESTINY logo animation, then **Connecting** after the
  Cross taps, then the game's own **"Destiny servers are not available"**
  dialog (about 105 s in).
- The responder received the game's first requests: two
  `GET /v1/users/D1Guardian/friendList` on 31315 from the game's `libhttp`,
  both answered 200.
- **Stop point:** the new build signs in to PSN at
  `POST https://auth.api.sonyentertainmentnetwork.com/2.0/oauth/token`, not the
  01.29 `auth.api.np.ac.playstation.net`. That name is one of the four not yet
  redirected, so it resolved upstream and the connection then failed (the
  firewall block held). No `/SignOn` was sent. The responder already serves
  `/2.0/oauth/token` regardless of host, so applying the redirect is the next
  step.

**After the redirects (13 October, run `20261013-074718-later-56578-guard-redirected`).**
The user applied `contain-d1.cmd`, so all 54 names resolve to loopback. Readiness
now fails only the permanent native-runtime flag. The boot ran with the guard
and without `-AllowUnredirected`; no fault occurred in 8 minutes. Responder log:

- `POST auth.api.sonyentertainmentnetwork.com/2.0/oauth/token` → 200 with
  `id_token`.
- **`POST www4.signon.deadorbit.net/SignOn`** (795 B, client build
  `56578.17.04.12.1251.evil`) → 200 placeholder success (BAP `127.0.0.1:3075`,
  manifest URL `http://cdn.deadorbit.net/manifest.bin`).
- `GET cdn.deadorbit.net/manifest.bin` → our 71,003-byte unsigned manifest.
- One full retry of SignOn and the manifest fetch, then the game's own dialog:
  **"Failed to download configuration files from Bungie servers", error code
  `turtle`**. No guest crash report.

So the server stack still answers the new build through OAuth and `/SignOn`. The
next gate is the CDN manifest's RSA signature envelope, as predicted. It cannot
be forged.

**Lab equivalent re-derived, not yet run.** The one RSA-2048 DER key is at
`0x15A0440` (01.29: `0x1591800`). The `cdn_content_manifest` handler is
`0x2B0980` (01.29: `0x2B0030`). Its only flag read is `0x2B09F0`:
`cmp byte [0x18DC974],0; je 0x2B0AF2`, which otherwise requires a body over
`0x107` bytes and verifies a signature over `body+0x108`, matching the 01.29
description. The flag ships as `0x01`, at 01.29's `0x18C4974` plus the
second-segment shift of `0x18000`.
`make-job-helper-guard.py --unsigned-manifest` writes
`patches/CUSA00219-56578-lab-manifest.xml` (guard plus that byte set to `00`,
after verifying the shipped value). The run using it was blocked by the session's
permission policy as a security-check bypass, so it awaits the user's decision.
The 1,392-entry manifest also exceeds the 01.29-measured descriptor cap, which
may be the next gate after that.

**Lab manifest run (user-approved, run `20261013-080220-later-56578-lab-manifest`).**
All three lines of `CUSA00219-56578-lab-manifest.xml` were logged as applied. No
fault occurred in 8 minutes. OAuth, `/SignOn` (`www8.signon.gravityshavings.net`)
and the 71,003-byte manifest were served twice as before, but the game's error
moved from `turtle` to **`marionberry`** ("Could not connect to the Destiny
servers"). So the unsigned 1,392-entry manifest was accepted past the
envelope, at least as far as this gate; the descriptor cap did not stop it here.
`marionberry` is `error.transport_method_invalid`. The PS4 project traced it on
01.29 to a loopback-only transport: the client rejects `127.0.0.1` STUN/BAP
addresses and wants a routable-looking local interface. It solved this on Linux
with an isolated dummy interface at `10.77.0.2`. Here every redirect and listener
is `127.0.0.1`, and shadPS4 logs `sceNetCtlGetInfo: local ip: failed to retrieve`
four times. This is the open Windows guest-transport gate. Everything past the
lab byte is `synthetic_ancestor`.

**Lab network (13 October).** The user ran `d1-lab-net.cmd`. "D1 Lab" (KM-TEST
loopback adapter) now has `10.77.0.2/24` with no gateway, the redirects point
there, and the shadPS4 block covers everything except `10.77.0.0/24`.

- With WSL servers (`20261013-083045-later-56578-labnet`), the game's OAuth timed
  out. Under mirrored networking Windows owns `10.77.0.2` itself, so Windows
  clients never reach a WSL listener on it.
- The orchestrator's `-LabIp` mode therefore runs the same `d1-responder.exe`
  and `shadnet-server.exe` natively on Windows: guest-facing responder on the
  lab address, a loopback responder for the emulator's WebAPI, shadNet on
  `127.0.0.1:31313`. Every listener is checked to be loopback or lab, and only
  its own processes are stopped (verified).
- Run `20261013-084057-later-56578-labnet-win`: OAuth, `/SignOn` and the
  manifest were served at `10.77.0.2`, the game resolved all STUN names there,
  but it still ended in `marionberry`. No BAP connection was made.

**Root cause: shadPS4's local-IP probe.** `sceNetCtlGetInfo(IP_ADDRESS)` calls
`NetUtilInternal::RetrieveIp()`, which `connect()`s a **TCP** socket to
`1.1.1.1:80` and reads `getsockname()`. Our firewall block correctly prevents
that real off-box connection, so it fails and shadPS4 reports the placeholder
`127.0.0.1` with success. The game's Demonware layer (`0x2E9FC0` on 01.29) uses
that as its own address and rejects the transport. The PS4 project fixed this
in its private fork (`shadPS4-d1trace`; `status/shadps4.md`, "local-IP fix",
with `capture-rig/retrieve-ip-control.py`): the probe becomes `SOCK_DGRAM`
(route lookup only, no packet) with a `getifaddrs()` fallback, and total
failure returns `NOT_AVAIL` instead of a fabricated loopback. Current upstream
still uses the TCP probe. Allowing the TCP probe would make a real internet
handshake and is not acceptable. The next step needs a shadPS4 build carrying
that fix; see the questions in the report.

**Patched shadPS4 (user-approved downloads, 13 October).** The shadPS4 source
at tag `v.0.19.0` (`c7e065d1`, the commit embedded in the binary in use) was
cloned with 45 submodules to `F:\d1	ools\shadPS4-src`. The official LLVM
23.1.3 Windows archive was downloaded; its SHA-256 `9ee6a219…` matches the
release's GitHub build attestation. It is unpacked to
`F:\d1	ools\llvm-23.1.3`. `Build-D1ShadPS4.cmd` builds Release with
`clang-cl`, Discord RPC and the updater off, and `HAVE_ENDIAN_H=OFF` (LLVM 23's
`endian.h` lacks the Windows byte-order helpers, which broke the vendored
LibreSSL). Result: 2,449 steps, `shadPS4.exe` `1297c91c…`. It was swapped in at
the firewall-blocked path. The stock binary (`91eb69a5…`) is kept as
`F:\d1	ools\shadPS4\stock-0.19.0-shadPS4.exe.bak`, which is deliberately not
launchable because the block is path-based; `SWAP-RECEIPT.txt` records both
hashes and how to restore.

Run `20261013-091447-later-56578-labnet-guestip` (guard + lab manifest, lab
network, Windows servers, `D1_GUEST_IP=10.77.0.2`): shadPS4 now reports
`local ip: 10.77.0.2` four times, with no IP warnings. No fault occurred in 8
minutes. The game resolves all 16 STUN names three times and opens 13 UDP
sockets. OAuth → `/SignOn` (the reply advertises BAP `10.77.0.2:3075`, client
IP `10.77.0.2`) → manifest → one retry, but still **`marionberry`** and still no
BAP connection. The local address was necessary but not sufficient.

Leading hypothesis (not yet tested): the 56578 client parses the transport
fields of our 01.29-shaped SignOn reply differently. The static check already
showed account/profile schema changes, and the client re-signs-on after the
manifest instead of dialling BAP. `error.transport_method_invalid` is at
`0x1654414`; its only reference is the error-table function `0x7855C0`, not the
decision site. Next: locate the 56578 SignOn response parser and compare its
field layout with `d1-responder`'s reply (f2.6/f2.7 BAP, f14 client IP, the
STUN/transport method), using RTTI, strings and the 01.29 design notes.

**From marionberry to investment sign-in (13 October, later).**

1. *SignOn schema is unchanged.* `dump-codec-descriptor.py` reads the codec
   records and resolves their pointers from `DT_SCE_RELA`. 56578's response
   descriptor `0x1910CC0` (parser `process_response` `0x2AA420`, same codec
   `0xA2020`) has exactly the 01.29 fields, including f2.6/f2.7 BAP and f14.
2. *Guest diagnostics re-derived (output only).* `retail_log_emit` is `0xE4AA0`
   (same 1,124 x `0x150` ring at `+0x19860`, `0x140` copy), the check is
   `0xE4D70` (1,636 sites), `printf` is `0x12AAB90` and `"%s
"` is `0x16136A0`.
   `make-job-helper-guard.py --diagnostics` adds the sink and T=1, verified by
   disassembly of a patched scratch copy, in `CUSA00219-56578-lab-diag.xml`.
   The game's own log then shows STUN falling back to `10.77.0.2`,
   `platform_signin`, `account_signin` (SignOn success; the server reports
   content build `56578.17.04.12.1251.evil`), `content_check` passed (manifest
   accepted), and then `bootflow:package_registration`: *Prerequisite
   'ENUM(15)' failed for reason 'error.transport_method_invalid'*, preceded by
   *STUN address discovery failed and not configured to use external address
   from signon.*
3. *The setting.* `0x2E0560` reads engine-config bools `+0x598` (on STUN
   failure use the SignOn f14 address) and `+0x599` (prefer SignOn over STUN),
   via `0x34E3A0` = block `+0x534` (block pointer `0x2693DF8`). The 56578
   config-delta setter switch is `0x1033F80` (decoder `0x1DB190`, magic
   `0x123456`; key 894 still maps to `+0x9EE`). `+0x598` and `+0x599` are
   **new sparse keys `0xF1048` and `0x4409EC`**, absent in 01.29, where an
   unknown key aborts the SignOn parse. `d1-responder` now appends key
   `0xF1048 = 1` to the SignOn f4.3 delta **only when the request's build is
   `56578.*`** (`d1-responder-56578-signon-fallback.patch`). The test covers
   both directions; 13/13 tests and all responder controls pass.
4. *Result* (run `20261013-094238-later-56578-signon-extaddr`):
   `package_registration` → **`bootflow:bap_signin`** → **`investment_signin`**.
   The BAP channel opened (start 0x1E/0x1F, secure hello 0x19/0x1A), then the
   client sent queuez register 0x79. The emulator then aborted in shadPS4's
   shader recompiler: `V_CMP_CLASS_F32: Unsupported float class mask: 0x2`
   (QuietNan alone). The D1 shadPS4 build now maps a single-kind NaN class
   test to `IsNan` (SPIR-V cannot distinguish them; GPU arithmetic yields only
   quiet NaNs). Rebuilt binary `03909598…` swapped in at the blocked path
   (`SWAP-RECEIPT.txt`); `shadps4-0.19.0-local-ip.patch` now covers all three
   files.
5. *Current stop* (run `20261013-094527-later-56578-signon-extaddr-b`, no
   emulator fault): investment sign-in exchanges 503 (account) and 206
   family 3 (roster `0xA2AC2C74` + character `0x3431F1BA`, both still valid in
   56578), plus queuez pushes for families 4 and 3. The client then fails with
   **`error.investment_server_object_instance_type_from_checksum_not_found`**.
   From the 56578 descriptor table
   (`retarget-static/descriptor-checksums-later.json`, 4,566 descriptors):

   | object (tag) | 01.29 checksum / size | 56578 checksum / size |
   | --- | --- | --- |
   | account `0x80801803` | `0x64AE75BF` / `0x7338` | `0x6B30D2B4` / `0x75C8` |
   | profile `0x80801743` | `0x8127F15A` / `0x7078` | `0x003BC6E5` / `0x7378` |
   | roster, character, family-4/4 wrapper | unchanged | unchanged |

   Both objects grew (+0x290 / +0x300 bytes), so the server must serialise them
   with the 56578 schemas. Changing only the checksum is not enough. This is the
   investment-schema re-derivation (plan step 5).

**Investment sign-in passed (13 October, ~10:07 UTC).** The minimal account
and profile encodings are unchanged in 56578: the inner account `0x808018C7`
still has 25 top-level fields with the same leading four, and the inner profile
`0x80801714` still has 18, ID first. So only the instance checksums need to
follow the client build. `d1-bap` gains `ACCOUNT_INSTANCE_TYPE_56578`
`0x6B30D2B4`, `PROFILE_INSTANCE_TYPE_56578` `0x003BC6E5`, `set_client_build()`
and accessors. `d1-responder` records the build from the SignOn `build=` query,
and the builders stamp checksums through the accessors (01.29 defaults
unchanged; d1-bap 161 tests, responder 13 tests and all controls pass). The
01.29 profile fixture is withheld from other builds, which get the sparse
profile (ID + orbit destination). Patch: `d1-bap-56578-instance-types.patch`.

Run `20261013-100634-later-56578-acct-profile-b`: OAuth → SignOn → content
check → package registration → BAP sign-in → **investment sign-in → world
`ambient_bootstrap` → `bootflow:profile_setup`**. The 503, 206 family-3
(roster + character), subscription and echo exchanges were all answered, and
family 4 (account `0x6B30D2B4` + profile `0x003BC6E5`) and family 3 were pushed.
About two minutes later the game halted itself (`RELEASE_VHALT`, *hitch
detected: mainloop … bootflow:profile_setup … job name: 'network_send' …
stalled*) and uploaded its crash report to our `ticket_drop`.

The game's own crash report gives its version as `evil tiger_final release
orbis 56578.17.04.12.1251.evil`, `insecure_ip 10.77.0.2`, BAP
`10.77.0.2:3075`, silo `28729`.

**Caveat for those halts:** the window frames show the Windows lock screen.
The session locked at about 10:05 UTC (the orchestrator's screenshot call
failed then, and it is now soft-failing). A locked session stops the emulator
presenting frames, which stalls the main loop that the game's watchdog
measures. The pipeline cache, enabled in the emulator config (recorded in
`SWAP-RECEIPT.txt`) to rule out shader-compile stalls, warmed with 223 entries
and did not change the result. The halt must be re-tested with the session
unlocked before it is treated as a protocol or emulator problem. One
interrupted run (`…-100517-…`) left the emulator and servers running; they were
stopped after its logs were saved.

`profile_setup` is the operator menu step on 01.29 (move down, confirm,
dismiss the brightness prompt). It needs the user's controller (`-OperatorInput`).

**Static re-derivation while the PC was locked (13 October).**

*Oodle (content decoding).* `DecodeSome` is the only function referencing the
assertion name `"OodleLZDecoder_DecodeSome"` (`0x16C3BB1`): **`0x126CA00`**
(01.29 `0x12672E0`). The scratch query `0x126C840` and create `0x126C8C0` sit
at the same offsets before it, and the frame `memset` returns to `0x12761A5`
(the library moved `+0x5720`). The harness's other hooks are confirmed by NID
(computed `SHA-1(name + suffix)`): memcpy `0x12AA7C0` (`Q3VBxCXhUHs`), memmove
`0x12AAAB0` (`+P6FRGH4LfA`), memset `0x12AA980` (`8zTFvBIAIN8`),
`__stack_chk_fail` `0x12AA7F0` (`Ou3iL1abvng`). The 01.29 harness had
memcpy/memmove labels swapped, harmlessly, since both are copy stubs. The stack
guard slot is `0x1A1F100`, and Oodle's plugin-routed aligned alloc/free are
`0x1278720`/`0x12787B0`. The 01.29 `IMP_NOP` (`0xF8ED00`) has no 56578
equivalent identified yet, and the decode did not need one. `d1-oracle` picks
these per image hash and refuses unknown images. **The client's own decoder
extracted tag `0x80A5E37A` for 56578:** `ps4_investment_globals_012f_6.pkg`
entry 890, 6,528 bytes, self-length header match, SHA-256
`84ab907ea3e794eb12023303dc3a3b84866cdc85cb7a2bb89675f95b073aee27`
(private, `retarget-static/`).

*BAP tables.* `POLICY_PTRS` is the only run of 33 relocated slots pointing at the
33 objects whose vptr is a `c_investment_bap_message_policy_*` vtable (RTTI):
`0x193BD50`. The registry `0x193A1B0`, names `0x193BB40`, codecs `0x193B540`
and push table `0x193DB30` follow at the same `+0x180A0` shift and match their
structure. With the 56578 definitions file, **`d1-bap controls`: 18/18 PASS**,
including drift checks (derived == server constants) for the 58 message
classes, 33 policies, 13-entry push set and 8 header lengths, and messages
206/503 keep their schema tags. The server's BAP tables are therefore verified
for 56578, not merely tolerated.

*Schema survey.* Of the 19 schema tags whose 01.29 checksum is documented in
`design/`, 16 are unchanged in 56578. The changed three are account and profile
(handled) and `0x80801941` (`0xB8C0245A` → `0x2859B3E2`, the world-server root
embedded in 503/205). Its 56578 layout is unchanged where it matters: 944 bytes,
8 fields, unlock flags at field [3] as a 6-bit count plus 50 × {u16, u2}, so the
503 encoding is unaffected.

*Fix found on the way.* `Image::open_default` still opened the 01.29 path; that
edit had failed in the first registry change. It now follows the registry, and
`Image::open_reference` / `Harness::open_reference` / `Content::open_reference`
serve tests and controls that assert 01.29 facts. Those skip unless
`D1_BUILD=01.29` and the 01.29 estate are present; `d1-oracle`'s Oodle control
command always uses the reference. `d1-oracle` unit tests that need the absent
01.29 executable fail as they did before this change (public CI does not run
them). Patch: `d1-56578-oodle-bap-anchors.patch`. Windows: d1-build 3, d1-eboot
all suites, d1-bap 161 tests, responder controls pass. Linux (public tree):
d1-build, d1-eboot, d1-bap, d1-responder and d1-harness suites pass.

**Codec, Activity-Host and profile re-derivation (13 October, still static).**

The 01.29 ELF is not on disk, so each 56578 address was found from the 56578 image plus the
01.29 notes' structural descriptions. None was found by offsetting a number.

*Bitstream primitives* are at the same addresses as 01.29 (ctor `0x72240`, begin-write/-read
`0x723B0`/`0x72400`, flush `0x72560`, end `0x72650`, spill `0x733A0`, read-bit `0x73430`), and each
body matches its documented behaviour. Write-one-bit is the gatekeeper vtable slot 5 (RTTI) at
`0xAF1650`, byte-identical to the 01.29 listing.

*Object codec* `0x2C9A80`/`0x2C9A90`, replicated `0x2C9AE0`: the module moved `+0xA90`. The three
documented bitset memset sites moved by exactly that. The allowance window is
`0x2C6A90..0x2C9E20`, deliberately not a blind `+0xA90`, because `0x2C9E20` memsets an unrelated
stack buffer.

*Message codec* `0x261E90`/`0x262260`, manager `0x25CBD0`, id→index `0x2800E0`. *Registry global*
`0x1A238A0`: the decoder has three rip-references to it (01.29 had three), and the walk is the
oracle's construction contract instruction for instruction.

*Activity Host:* decode/encode wrappers `0x2A4AC0`/`0x2A4B70` (capacity still `0x1DF00`),
`activity_client_send_message` `0x299D10`. A register tracker read all 30 call-site rows: udt,
descriptor global and size at the 28 send sites, 12 decode sites and 2 replicated sites. Every
global moved `+0x18090` and every udt keeps its role. Every stated size is unchanged except
**udt 37 `send_client_heartbeat`, `0xB1E0` → `0xB750`**. The RX dispatch table is at `0x1910480`
with the same 17 types, and its relocated handlers confirm the udt 0/1/4/11 rows independently.

*Results on 56578 (Windows, the owned image):*
* `d1-oracle control`: object and message PASS.
* `vectors`: all OK, including account and profile.
* `activity-host controls`: **34/34**. This includes the client's own decoder reproducing the
  captured live join_request's logged fields, a byte-identical re-encode, and a 7,297-byte
  replicate_membership through the replicated wrapper.
* sensor 11/11, roster 18/18, character 20/20, inventory 9/9, profile 12/12.
* `d1-activity-host controls`: **112/112**.
* item-index 4/5. The 5th needs `catalogue/d1-catalogue.json`, which is not on this machine.

`capture-rig/item-content-index.tsv` was generated from the 56578 packages with the client's own
decoder. It has 7,544 rows, the entry SHA-256 is `71bfdc0a…`, and the file is git-ignored and local.

*The profile changed in 56578.* The struct is `0x7378` = 29,560 B, up from 28,792. Body field
[10] `0x80801701` grew `0x100`. Field [11], the dense completion flags, went from `0x80804C21`
(2,048 elements) to **`0x8080165B` (2,560 elements)**. That is 960 B on the wire instead of 768,
and `0x80804C21` no longer exists in 56578. Everything from field [12] onward moved `+0x300`:
the stat block is now at `+0x6FD0`, the overrides at `+0x6FE0`, and the all-content override at
`+0x714C`. Fields [0]–[9] are unchanged. The negatives caught it: the old `+0x6CE0` positive
passed by accident because it now lands in the dense u32 array, and the paired negative failed.
The server's own sends are unaffected:
* The minimal profile (SOID plus 17 absent fields) still decodes through 56578's codec.
* The instance checksums are right: account and profile were already build-aware, and roster
  `0xA2AC2C74`, character `0x3431F1BA` and unlocks `0xC4060975` are unchanged in 56578.
* Grant tooling that writes completion flags must use 56578's 2,560-element array.

*Harness hardening:* stubbed memcpy/memset/alloc now refuse lengths over 256 MB as a guest fault.
On 56578, one Activity-Host negative control drives the encoder into a bitset memset of length
`-0xA4`, returning to `0x2C8F89`, a measured codec site. Before this change the whole process
aborted (`capacity overflow`). Now it is the refusal that control requires.

*Linux, public tree:* the d1-oracle and d1-activity-host failure sets are identical to the public
HEAD's: 13 and 9, every one needing game images. The two new 56578 tests pass by skipping. One
regression was caught and fixed first: `ProfileSpec::to_object_for` lets the image-free unit tests
pin 01.29 offsets.

Public commit `dba749c` (local, not pushed); patch `d1-56578-codec-ah-profile.patch`.

*Environment note:* `d1-env.ps1` sets `LIBCLANG_PATH=F:\d1\tools\LLVM\bin`, which no longer holds
`libclang.dll`, so unicorn's bindgen fails. Building d1-oracle needs
`LIBCLANG_PATH=F:\d1\tools\llvm-23.1.3\bin`.

*Still open:* the dynamic-flags fault RIP for 56578 (unobserved; the refusal text names the grow
`0x2C7630` only), field [10]'s internal change (no 01.29 sub-layout on record), and the 56578
`IMP_NOP` equivalent.

**Build 56578 reaches orbit (13 October, lab runs, user away; runs authorised).**

Run `20261013-130110-later-56578-lab-orbit-c` went title → … → `investment_signin` →
`prepare_for_orbit` → `character:signin` → `character:prologue` → `cleanup` → `setup:orbit_loading`
→ **`setup:orbit`**. It stayed in orbit, with no guest halt and no host fault, until the 14-minute
limit. A frame shows the orbit screen with the D1Guardian nameplate, Set Destination and a "Crucible
Unlocked" prompt. Everything is reached through synthetic client patches
(`patches/CUSA00219-56578-lab-orbit.xml`), so it is `synthetic_ancestor`.

*The "profile_setup stall" was not the wizard.* Three earlier runs halted with
`hitch detected … job name: 'network_send' … stalled`. The first one was put down to a locked
session. Wizard taps did not help: the posted keys never reached the unfocused window, and focused
taps lost focus. With the wizard skipped (c1: `0x57F5E0` → `mov al,1; ret`; it is the 56578 form of
`0x57E490`, called only at `0x79DAE0`), the same hitch moved to `prepare_for_orbit`. Lib.Net debug
logging showed the client spinning on `sys_sendmsg(BAP socket)`. **shadPS4's Windows
`PosixSocket::SendMessage` cast `OrbisNetMsghdr` to `WSAMSG`, whose `WSABUF` is `{len, buf}` where
an iovec is `{base, len}`, and called `WSASendMsg`, which Windows does not support on TCP.** A
multi-buffer stream send returned 0 bytes forever. Under the fix, stream sendmsg/recvmsg
gather/scatter through one `send`/`recv`, and the client's `0x0015` purchased-offers request
arrived and the conversation continued. That fix is in `shadps4-0.19.0-local-ip.patch`, in the
binary with sha `e96f7308…` (receipt in `F:\d1\tools\shadPS4\SWAP-RECEIPT.txt`).

*Voice.* shadPS4 stubs libSceVoice. In `cleanup` the client faulted three times in its voice-port
layer:
* a NULL port object in the SetMuteFlag thunk `0xB47E50`;
* the same in the GetPortInfo thunk `0xB47CA8`;
* `#DE` at `0xB493D9`, a divide by a GetPortInfo field the stub never fills.

The root is `0xB483EC` `mov byte [r14],1`. It marks voice "started" unconditionally after
Init/GetResourceInfo/Start, and every voice entry point opens with `cmp byte [rdi],0; je`. The lab
patch writes 0 there, the console-without-voice path, and keeps `xor eax,eax; ret` guards on the
six dereferencing thunks as a backstop.

*Next gate.* In orbit the client sends `0x000C` subscription requests about every 10 s, roughly
100 in the run. Each is answered with `0x000D`, yet it logs
`Failed peer-subscription validation [error error_timeout][account_soid 0xD100000000000001]` 86
times. It is waiting for something the server does not send after subscribe.

*Peer-subscription gate closed (same day).* Classes 0 and 2 subscribe and, with only bare `0x000D`
ACKs, time out as they did on 01.29: the client marks them LOST, a permanent state that refuses
later updates. The responder's existing `D1_ANSWER_FAMILIES=0,2` sends each one empty baseline
family answer after its ACK, the answer-once shape proven on 01.29. In run
`20261013-132144-later-56578-answer-fam02` both classes then subscribed exactly once and there were
**0** `Failed peer-subscription validation` lines (86 without it). The client stayed in
`setup:orbit` for the rest of the 12-minute run with no faults and only routine log lines. The
orchestrator passes such knobs as `-ExtraServerEnv 'D1_ANSWER_FAMILIES=0,2'`, and they are
recorded in `server-environment.txt`.

*Driving orbit by hand (same day; the user was away and asked me to do the controls).* For a
session with `-OperatorInput`, two helpers in `tools/ps4_setup` stand in for the pad:
`d1-pad.ps1` focuses the emulator and sends real key input (cross `NumPad2`, circle `NumPad6`, left
stick `W/A/S/D` with hold times), and `d1-shot.ps1` captures the window. The first attempt stalled
behind a Windows Firewall prompt for the new `d1-activity-host.exe` listener. I left that decision to
the user, who allowed it. From the title I reached orbit and dismissed its notices (Crucible
Unlocked, Armsday, Age of Triumph, a Destiny 2 advert dismissed with circle, not cross, so as not
to press Pre-order). I then opened the Director through Set Destination. **The Director is empty:**
Earth has no destination nodes, and the Crucible emblem shows no tooltip and does not respond.

*Unlock flags are build-specific.* The 01.29 ten-flag set is ordinals in the client's master flag
table. That table is content: tag `0x80A5FFCD`, class `0x8080179C`, element class `0x808018B8`, rows
`{u32 hash, u16 kind, u16 sub}`. It has 6,944 rows in 01.29's `012f_3` and **7,731 in 56578**, the
same count as the 2020 manifest's hash universe. Read with the client's own decoder and joined by
hash: 98, 308, 419, 440 and 834 are unchanged; 2667→2668, 3389→3391, 5015→5021, 5987→5998 and
6018→6029. Controls: 01.29[98] = `3638915056` and 01.29 ord(`1681748763`) = 2538, both documented;
56578 puts the latter at 2539, the 2020 manifest value. The responder's built-in default would have
granted the wrong five.

The 56578 503/205 pair was encoded by the 56578 client's codec after the pinned-body control. The
shared tail agrees at `503[23:]==205[3:]` with ids, `[21:]/[1:]` without. It is now the default when
SignOn reports 56578 (public `704c8d3`, patch `d1-56578-unlock-ordinals.patch`). It was also served
by override in run `…-operator-grants`: delivered (503 once, 205 by poll), and the responder
confirmed agreement. **The Director stayed empty.** The ten flags are the rung-33 DLC/permission
gates, not Director content.

*How 01.29 left orbit.* Not through the Director: `design/GUARDIAN-IN-WORLD.md` §3 used a live
memory poke (`session-params.py --launch-poke --activity-id 407`), then a cinematic-suppress byte, a
done-mask force, a join counter, two code hooks and a spawn poke. The biped existed but could not be
controlled. Reproducing that on 56578 means re-deriving those addresses and having a live poke
transport for this emulator, which is a separate piece of work.

*Build 56578 reaches `activity:in_world` on `pvp_moonbase` (13 October, run
`20261013-…-later-56578-launch-poke-b`).* The route mirrors 01.29's, rung for rung, re-derived:

1. **Leave orbit: synthetic launch poke** through `tools/ps4_setup/d1-winguest.py` (Windows
   ReadProcessMemory/WriteProcessMemory on the shadPS4 process; guest pointers are host addresses,
   and eboot-relative ones take the logged load base). The session accessor `0x775770` is
   instruction-identical to 01.29 `0x773FC0`; its manager global is `0x26CA4E0`. Writes: desired
   activity `params+0x9B8+0xEA` = 407, goal `params+0xE4` = 0x24, orbit handler (vtable
   `0x19474B0`, the single hit in the image's data range) `+0x38` debounce = 0. Every value read
   first was the predicted orbit state.
2. **Rungs 28–33 on served data:** session creation, matchmaking, `activity_host_setup` (our
   Activity Host answered the join), prologue, outro, then `activity_world_transition`:
   **"successfully changed world to: pvp_moonbase" / "World-transition finished".** The rung-33
   DLC/permission gates passed on the re-indexed 56578 unlock flags (`704c8d3`), with no poke.
3. **Emulator fix:** loading `pvp_moonbase` hit shadPS4's texture-cache `UNREACHABLE` ("unresolvable
   image overlap with equal memory address": same format, tile mode and resources, different image
   type, 16× the size). Fork-local change: treat a TYPE change at an equal address like the
   existing tile-mode reuse (free the stale image when safe, use the merged id). Binary sha
   `8cfc65fa…`, receipt in `SWAP-RECEIPT.txt`, patch regenerated.
4. **Rung 34** (`0x7969A0`: request 0x23 when done `+0x1A8` == requested `+0x198`): masks
   requested `0x7FF`, done `0x17F`. **Task 7 suppress, synthetic:** byte 0 of the
   activity-selection record (pointer at `0x26BC068`; `0x766750` = record[0]==1 || `0x78C710()`) = 1.
   Task 7 completed and **task 9 started**, as on 01.29. Tasks 9/10 (world content) stalled.
   **Done mask forced to `0x7FF`, synthetic, as 01.29.**
5. **Rungs 35 → 36 by themselves:** `activity:physics_join` → **`activity:in_world`**. 01.29's
   join-counter poke was not needed. In world, the client's own diagnostics show both connections
   established: primary BAP (109 sent / 112 received) and the Activity Host at 10.77.0.2:4110
   (169 / 64).

Synthetic in this run: the lab-orbit client patches, the launch poke, the task-7 byte and the
forced done mask. Everything else is served. The screen locked at 15:09 local (Windows lock
screen), so there is no in-world frame. I did not try to get past the lock. The client later left
`in_world` with the game's own `error.inactivity_timed_out` (the idle kick; no input reached it
while the screen was locked) and returned through `cleanup` to `setup:orbit` cleanly. The run ended
at its 45-minute limit with no fault, no crash ticket, and the servers stopped.

*Watched rerun (screen unlocked, run `…-in-world-watch-b`).* The first attempt failed with
**BUFFALO**: shadPS4's startup probe of the local shadNet server got no ServerInfo within its window,
so it disabled shadNet for the run and PSN sign-in failed. The previous probe answered in 0.15 s; it
looked transient, and a restart probed OK. The rerun repeated the same route. Task 7 clears with the
suppress byte, tasks 9/10 do not, and after the forced done mask the client reaches `in_world`. The
slice-set manager reports `initial_slice_set_instantion → fully_enabled` on region `PRV00.00`
(`pvp_moonbase`, `0x65BDC6F8`), the private match advertises, and session updates succeed. **The
screen is black:** no player character is constructed, so there is no camera, exactly 01.29's
"no handle = no camera". Next frontier: the spawn. Served route: the Activity Host's udt-5
`sensor_auth_update` (`D1_AH_SENSOR_PHASE`, `design/activity-lifetime-state.md`), which needs the
per-load sensor-name hash and generation (`world+0x1D`) read from 56578 memory, and was `[U]` on
01.29 too. Synthetic route: 01.29's two code hooks (lifetime-state getter, per-player reject `je`)
plus the spawn poke, re-derived for 56578.

*Served spawn route, measured live (runs `…-sensor-phase3`, `…-sensor-measure`).* The Activity Host
sent udt-5 `sensor_auth_update` phase 3 every 10 s: 25 frames, ~18 of them while in world.
`d1-winguest.py` now reads the decoder's preconditions live, each re-derived for 56578:
* **Lifetime record:** `lifetime` replicates `0x10C500` (same address as 01.29): `0x1072C0(0x10,0)`
  (count word `0x25D6FF8`, table `0x1AD5000`), then the registry walk (`0x1A238A0`), then
  `+0x1AC`. The phase byte is at **`0x38026207C`, the same address 01.29 recorded**.
* **World pool:** `worlds` reads the pool via `0x2A00A0` (`[0x26556D8] + i*[0x26556E0]`, stride
  `0xDB0` as on 01.29).
* **Group table:** read from the globals container (`0x1071D0`: groups `{base, hash}` at `+0x10`,
  stride `0x14`).

Results in world:
* `world[0]+0x1C = 1` (enabled) and **`+0x1D = 0`**, equal to the frame's `gen` (check at
  `0x2A1E88`).
* Group 0 hash is **`0xC2DACAF7`** with base 0, so name-ref sub 3 = record index 3, the lifetime
  record. In orbit the single group is `0x4786C0E0`, as on 01.29.
* The record stays at phase **−1**, `+0x1B4 = 0x811C9DC5`, and `+0x674/+0x678 = −1/0x811C9DC5`.

So every precondition this side of the walker holds, and the 01.29 defaults are correct for 56578,
yet nothing is stored. That isolates the one step 01.29 left `[U]`: whether the post-pass
(`FUN_002CE2A0` dispatch via `record+0xA8`) fires for a freshly delivered element. The next probe
needs a runtime hit counter or breakpoint in the client (the decoder `0x2A1C40`, the walker
`0x2D0CD0`, the setter `0x11DE30`), which is a new capability.

*Hit counters: where the served phase is lost.* `tools/ps4_setup/d1-wincount.py` adds entry hit
counters to the live client. It allocates a page within ±2 GB of the image, relocates each target's
first whole instructions into a stub (only push/mov/xor with no rip-relative operand or branch),
and has the stub do `lock inc` on a counter and jump back. The entry gets a 5-byte `jmp`, the
original bytes are saved to JSON, and `remove` restores them exactly; all six were verified
afterwards. `d1-relaunch.ps1` re-runs launch → rung 34 → in_world from orbit in the same session.
The 56578 setter is **`0x11DE50`** (01.29 `0x11DE30`; `memcpy(…, 0x504)` at `0x11DF10`).
Counts in world with the sensor repeating every 10 s:

| function | hits | what it is |
|---|---:|---|
| `0x29DDF0` udt-5 RX handler | 9 | one per frame |
| `0xBB1D00` sim-queue resolve world (`0x75D6B0(ah_id)`) | 9 | world found |
| `0x2A1C40` frame decoder | 9 | gen / enabled checks pass |
| `0x2D0CD0` named-globals walker | 8 | runs |
| `0x2D1600` walker terminator (`has_entry = 0`) | = walker (51/51 over a run) | frame parses to its end |
| `0x2D2F20` → `0x2D2F30` post-pass | **0** | never |
| `0x2CED30` dispatch, `0x11DE50` setter | **0** | never |

The post-pass runs inside a sweep (`0x2D1400` loop). It enumerates the world's resident object
set, resolves each entry's registry object `rbx`, and calls the post-pass when `rbx+0x60` equals
the current partition id (`0x345620`) or is ≥ 0x80. The lifetime record's `+0x60` is `0x8080039D`
(≥ 0x80), and its dispatch guard fields are as 01.29 documented (`+0xA8 = 0x8080039D`, ordinal 2,
`+0xB0 = 0x27FA2003`). So it **would** be swept if it were in the set. It never is. **This settles
01.29's `[U]`: the post-pass fires only for content resident in the world, and a freshly delivered
element outside that set is parsed and dropped.** The resident set is what tasks 9/10 (world
content) load, which is exactly what the forced done mask skips. The served sensor therefore cannot
land on this route until the world content is served or its residency is established another way.
The remaining in-world options are serving the task-9/10 content, or 01.29's synthetic spawn (the
lifetime-state getter hook, the per-player reject `je`, and the spawn word). 01.29's next steps
(two code hooks and a spawn poke to construct a biped) are not attempted on 56578.

*🎉 Build 56578: a Guardian in world, rendered and moving (13 October, run
`…-later-56578-forced-spawn`, `pvp_moonbase` / 407).* This is 01.29's GUARDIAN-IN-WORLD recipe,
re-derived for 56578, driven through `d1-winguest.py`:
1. **Served:** `D1_ROSTER_DIR=capture-rig/rosters-f0-hybrid` (family 0 answered with the
   2-object hybrid fixture, 2,415 B) with `D1_ANSWER_FAMILIES=0,2`, and the Activity Host with
   membership. With the fixture, **rung-34 task 7 cleared on its own** (done `0x17F → 0x1FF`, no
   suppress poke). The character definition was installed (`player+0x85C = player+0x1660 =
   0x811C9DC580A6A06C`, 01.29's value), and **player slot 0** appeared in the spawner's player table
   (`0x2689580`, 20 slots; it was empty without the fixture, which is why the spawner never reached
   its gate in earlier runs).
2. **Synthetic:** the launch poke, then the frozen hooks (`NtSuspendProcess` around the write): the
   getter `0x10C500` set to `mov [rdi],3; mov eax,1; ret`, and the per-player reject `je` at
   `0x412DD6` (spawner `0x412800`, gate `0x415700`) set to NOPs. Then the done mask forced to
   `0x7FF`, `in_world`, and the spawn word `player[0]+0x0C = 3` (pool `[[0x273EA80]+8]`, stride
   `0x2AF0`; player 0 is at the start of the pool).
3. **Body built:** `player[0]+0x644` went `0xFFFFFFFF → 0x7BFAC57B`.
4. **Picture:** still black until the stuck screen fade was cleared. That is `[0x26AD7F0]` (01.29
   `[0x2694B90]`) → block `0x3806FCA00` (**the same heap address 01.29 recorded**): count 1,
   mode 1, alpha 1.0. All three were zeroed (`unfade`), and the task-7 suppress byte is held by
   re-writing it every 2 s. **First-person view on the Moon, Ghost out, radar up**
   (`cfw-staging/2026-10-13-op/64-unfaded.png`).
5. **Control:** 2 s of left stick moved the view through the scene (`65-moved.png`). The forced
   spawn is controllable on 56578, which 01.29's first forced spawn was not.

Still synthetic: the lab-orbit client patches, the launch poke, the getter hook, the reject NOP, the
done-mask force, the spawn word, the fade zero and the held suppress byte. The served route
(udt-5 phase) remains blocked at the residency sweep (above), and the done-mask force skips the
world content that would fix it.

*Orchestrator.* `Start-D1ContainedBoot.ps1` now:
* taps confirm while the log reports `profile_setup`, controlled by `-WizardTaps`/`-WizardIntervalSeconds`;
* brings the window to the foreground and sends real key input, with `-NoForeground` to opt out.

A PowerShell trap cost one run: `$wizardTaps` and the `$WizardTaps` parameter are the same variable.

*Lab patch files* (PS4 tree):
* `CUSA00219-56578-lab-diag-skip-wizard.xml` is lab-diag plus the c1 skip.
* `CUSA00219-56578-lab-orbit.xml` is that plus the voice entries. It is the set that reached orbit.

## Handoff items

- 12 October package `matildaspc-20261012T201422-…`: NAS re-check at
  21:03:55 UTC passes (all three payloads match `hashes.json`, no unlisted
  files). Receipt: `PRIVATE_ENGINEERING/2026-10-12-daily-handoff-receipt.json`
  beside the dated handoff notes. No existing `PRIVATE_ENGINEERING` folder was
  found on this PC, so its location should be confirmed.
- This work is after the 12 October cutoff and belongs to the 13 October edition.

## Next

1. ~~Apply the redirects~~ (done 13 October); OAuth and `/SignOn` now pass.
2. ~~Unsigned-manifest lab byte~~ (approved and run 13 October; the game moves
   on to `marionberry`).
3. **Windows guest transport:** give the guest a non-loopback, non-routable
   local address (the 01.29 Linux design used `10.77.0.2` on an isolated
   interface). On Windows that means a host-only adapter, which is a system
   network change the user would make as administrator. Then point the redirects
   and the loopback-only servers at that address, and fix shadPS4's empty
   `sceNetCtlGetInfo` local IP if it persists.
4. Re-derive the build-specific parts: Oodle entry points (blocking schema
   `0x80A5E37A`), the eboot atlas, BAP tables/registry, the manifest cap and the
   account/profile schemas. Use RTTI and strings, since the 01.29 bytes are not
   available for signature matching.
5. Check for aknumbers' 1.33 release before deep address work. Its executable
   hash decides whether his addresses apply directly.

Window frames are window-rectangle captures and include a sliver of the desktop
at the edges. They are private evidence and need cropping before any daily
media use.
