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

## Handoff items

- 12 October package `matildaspc-20261012T201422-…`: NAS re-check at
  21:03:55 UTC passes (all three payloads match `hashes.json`, no unlisted
  files). Receipt: `PRIVATE_ENGINEERING/2026-10-12-daily-handoff-receipt.json`
  beside the dated handoff notes. No existing `PRIVATE_ENGINEERING` folder was
  found on this PC, so its location should be confirmed.
- This work is after the 12 October cutoff and belongs to the 13 October edition.

## Next

1. ~~Apply the redirects~~ (done 13 October); OAuth and `/SignOn` now pass.
2. Decide on the unsigned-manifest lab byte (above). Then the 1,392-entry
   manifest meets the unmeasured descriptor cap; a newest-generation or capped
   selection is the fallback.
3. Re-derive the build-specific parts: Oodle entry points (blocking schema
   `0x80A5E37A`), the eboot atlas, BAP tables/registry, the manifest cap and the
   account/profile schemas. Use RTTI and strings, since the 01.29 bytes are not
   available for signature matching.
4. Check for aknumbers' 1.33 release before deep address work. Its executable
   hash decides whether his addresses apply directly.

Window frames are window-rectangle captures and include a sliver of the desktop
at the edges. They are private evidence and need cropping before any daily
media use.
