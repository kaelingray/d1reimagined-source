# shadPS4 status

**2026-08-11 (later) 🎉 PLAYABLE ON RADV: run `140504-GEARED-RADV-11` - operator played 15+ min
in `pvp_moonbase` (stick, jump, Warlock super) on the AMD Radeon 8060S. Recipe + the fade-block
lever: `design/GUARDIAN-IN-WORLD.md` head. FPS number on RADV still unmeasured (F10 crashes; the
operator did not report slideshow - subjective feel TBD).**

**2026-08-11 ⭐⭐ THE FPS MYSTERY IS SOLVED - every non-RenderDoc run to date rendered on the CPU.**
`[C]` (verified live on the running client + ~40 past runs' logs): shadPS4's Vulkan auto-picker
(`vk_instance.cpp`, `gpu_id: -1`) sorts by API version -> discrete-GPU -> largest device-local
heap. This box's Radeon 8060S is INTEGRATED and llvmpipe (Mesa's CPU rasterizer) advertises all of
host RAM as its heap, so **llvmpipe won the tiebreak on every run that did not load the RenderDoc
layer** (`emulator.log: GPU_Model: llvmpipe`; the RenderDoc runs got `AMD Radeon 8060S Graphics
(RADV GFX1151)`). Fix: `VK_DRIVER_FILES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json` hides the
llvmpipe ICD; **pinned in `run-boot.sh` since run `20260811-121021-GEARED-RADV`** (do NOT pin it in
`config.json` - shadPS4 rewrites that file on exit). Per-run assertion: `grep -m1 GPU_Model
runs/<RUN>/emulator.log` must not say llvmpipe. Consequences: the **"6-7 fps in-game" datapoint was
a software-rasterizer number** and the framerate queue item must be re-measured on RADV; other
perf/jank observations from non-RenderDoc runs were also taken on llvmpipe and deserve re-checking
where they mattered (not automatically invalidated - re-measure before re-asserting either way).
Upstream shadPS4 issue (the picker never demotes `eCpu` devices), not our fork and not the server.

**2026-08-11 (lane B):** two vehicle facts from driving the live orbit client blind:
* ⚠ **SDL delivers controller input only to the FOCUSED window.** The virtual pad's presses do
  nothing while the emulator is backgrounded - no error, no log, indistinguishable from a dead
  pad. Any agent-driven input pass needs the window foregrounded first (operator action, or a
  future SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS build flag - untested).
* **`d1-live watch` arms cleanly on the live client** (112/112 threads, 0 unattributed sigtraps;
  shadPS4's fastmem SIGSEGV/SIGILL storm is forwarded un-swallowed). No exec HIT has yet been
  observed on any address, and no window had a firing positive control, so "hardware breakpoints
  fire on guest threads" remains open - `design/geared-guardian-fixtures.md` §10.

**2026-08-07:** investment signon completes (account record `2 → 3`, SOID `0xD100000000000001`) and
the long-standing `0x570AAE` null dereference is **solved and attributed** - it is a BAP response
span arriving empty, not a failed lookup. Three ablation patches converted the crash into a
deterministic `error.no_investment_connection` gate. Next artifact needed: span elements of schema
`0x808033F4`, which lives in registry slot `0x401` (a content package). See
`historicaldocs/design/character-and-orbit.md` §26.10-§26.18.

**Destiny 01.29 boots on Linux/AMD and is now EIGHT boot-flow gates deep, through `/SignOn`,
`content_check` and `package_registration`, into `bootflow:bap_signin`.** The client accepts our
signon response, takes a **CDN manifest URL we choose**, fetches it, and accepts an unsigned manifest
(with a one-byte lab patch). **It now halts deliberately in `bap_signin` with its own
`RELEASE_VHALT`** - and the halt has MOVED: `failed to load bootstrap_patchable` is solved (package
registration works, patchable packages load), and the current halt is `resourcerer failed during
unload`. Not a shadPS4 defect. Evidence: `historicaldocs/design/signon-protocol.md` §9,
`historicaldocs/design/content-manifest-gate.md` §9, and the game's own crash report preserved under
`runs/20260806-143440-manifest-served/guest-crash-report/`.

## What's going on

- Content gate is **closed** - the emulator accepts our dump and modules:
  `Game id: CUSA00219 · Title: Destiny · App Version: 01.29`.
- **It boots.** Shaders + pipelines compile, audio opens (48 kHz/8ch), video-out opens at 1080p,
  the splash screen is hidden, and Destiny's engine threads all run (`SceGnmDriver`, `job #0..3/4`,
  `TIME_CONTROL`, `Graphics Heartbeat`, `_DW HTTP Processor`, `BNG:VOICE_ENGINE`, `eq to wait flip`).
- **Steady state = the title screen**, identical in signature to the best public trace:
  ```
  sceNpGetOnlineId: SIGNED_OUT (user_id=1000 shadnet_enabled=false signed_in=false)
  sceNpPartyCheckCallback: (STUBBED) called
  ```
  12,999 polls in one run. **Zero sockets, zero DNS, zero STUN, no HTTP request ever sent.**
- **That gate was a signed-in NP identity** [C, reproduced on our own SKU + version, not inherited
  from the public trace] - `g_shadnet_enabled && NpHandler::IsPsnSignedIn(user_id)`, where
  `IsPsnSignedIn` needs a ShadNet client authenticated against a server.
- **It is now satisfied** by our own Layer A server: `SIGNED_OUT` count drops to **0**, and the client
  moves on to resolving `stun1-8.signon.{deadorbit,gravityshavings}.net`. Emulator-side details,
  config keys and the caveat on that measurement live in **`status/server.md`** - this doc stops at
  the emulator itself.

### Reproducibility + the crash that wasn't (2026-08-06)

Earlier runs died deterministically at `0x8000faaf4` (`Read from address 0x8`). **Cause was
environmental, self-inflicted:** each run was launched seconds after killing the previous one, while
`systemd-coredump` compressed a **16 GB core** in the background. Ten piled up. That contention hit
the window where Destiny's threads race to initialize.

| run | faults | NP polls |
|---|---|---|
| A1 (quiet box, `ulimit -c 0`) | **0** | 2045 |
| A2 (quiet box, `ulimit -c 0`) | **0** | 2043 |
| long run | **0** | 12,999 |

It also explains the pattern that misled the investigation: base 01.00 faulted in `Loading Loop`,
01.29 in `Graphics Heartbeat` - whichever thread loses a scheduling-sensitive init race under load.

**Harness rule:** run with `ulimit -c 0` on an otherwise-quiet box. Never launch a run while a
previous core is still being processed.

**Worth reporting upstream anyway:** Destiny has an init-ordering assumption that breaks under CPU
contention on Linux. Real fragility, just not our blocker.

### Ruled out along the way [C]

- **GPU / driver / unified memory.** Ran on **lavapipe** (software Vulkan - no RADV, no AMD, no
  unified memory): reproduced the crash exactly. Mesa `25.3.6-3.fc43`, no upgrades available. A
  different-GPU Linux box would have tested nothing.
> ⚠⚠ **CORRECTED 2026-08-10 - the "permanent control build @ `8bdec718`" claim below is STALE, and it
> is what made a stock-vs-fork A/B look like a clean experiment.** Measured today:
> `~/dev/shadPS4-main` and `~/dev/shadPS4-d1trace` are **both at commit `1a51a416`**. They are not
> two commits apart.
> ⭐ **The A/B is still possible, for a reason nobody stated:** `shadPS4-d1trace` carries **16
> uncommitted modified files** (`voice.cpp`, `net_util.cpp`, `netctl.cpp`, `p2p_sockets.cpp`, …) while
> `shadPS4-main` has a **clean** tree - so `shadPS4-main` IS a stock baseline at that commit. A third
> checkout, `~/dev/shadPS4-pristine-check` (`5be3f0a3`), also exists.
> ⛔ **But do not spend a build on the black screen.** The renderer hypothesis is dead on other
> evidence: at `activity:in_world` there are **577 live sobjects and ZERO of type 9 (`biped`)** -
> there was never anything to draw. ⇒ `historicaldocs/design/biped-census.md`, `historicaldocs/status/aug10-session.md`.

- **Emulator age and our fork.** Stock upstream main (`~/dev/shadPS4-main` @ `8bdec718`, 184 commits
  ahead, includes v0.17.0) behaved identically to our v0.16.0-based fork. Keep the worktree as a
  permanent control.
- **The fs→gs TLS patcher.** Verified in memory that both of Destiny's TLS thunks were correctly
  rewritten `0x64`→`0x65` (FS→GS). The machinery works.
- **Game version and SKU as causes.** Base 01.00 behaved the same as 01.29. Public traces are
  `CUSA00568` (EU/PAL) while ours is `CUSA00219` (US, `UP0002`) - not worth chasing; SKU was never
  the variable.

> **Terminology:** the old crash was **thread-local storage**, not Transport Layer Security. Don't
> confuse it with the cert/pinning notes below.

## Pinned

| | |
|---|---|
| Fork | `~/dev/shadPS4` @ `f3b0013a` (`codex/hindsight-vr`), `v.0.16.0-2-gf3b0013a` |
| **Control build** | `~/dev/shadPS4-main` - git worktree @ `8bdec718` (stock upstream main), `build/shadps4`. **The booting runs used this.** Keep it. |
| Game | `dumps/patch-01.29-CUSA00219/` - 1655 files, 49.95 GB, decrypted |
| Modules | 26 → `~/.local/share/shadPS4/sys_modules/` |
| Launch | `capture-rig/run-contained-emulator.sh <dump>/eboot.bin <run-name>` - starts Layer A + emulator in one contained namespace. Raw form in `status/harness.md`. |
| Config | `~/.local/share/shadPS4/config.json` - now `shad_net_enabled:true`, `shadnet_server:127.0.0.1:31313`, `connected_to_network:true`. Full key list in `status/server.md`. |
| ⚠ Discord RPC | `discord_rpc_enabled:true` since 2026-08-06 (operator request). **Breaks quietness, not containment.** Transport is a unix socket (`$XDG_RUNTIME_DIR/discord-ipc-N`), so it crosses the netns exactly like `systemd-resolved` - egress default-deny does *not* stop it. Publishes the title string and our SKU `CUSA00219` (in a `store.playstation.com` image URL that Discord, not us, fetches). Control build has `ENABLE_DISCORD_RPC:BOOL=ON`. Revert that one line before any run that must stay quiet. |
| Display | real monitor on HDMI-1 (3840x2400). Xvfb `:99` + `x11vnc` are the headless fallback. |

## The boot-flow ladder as of 2026-08-06 [C]

One run, one press of X, eight gates. This is the current spine - read it as the gate ledger.

```
bootflow:media_check → bootload → intro_screen_display → start
  → platform_signin            (needs a signed-in NP identity - Layer A)
  → platform_account_warnings
  → enumerate_content_rights
  → account_signin             (needs PSN oauth/token WITH an id_token, then /SignOn)
  → content_check              (needs a CDN manifest URL via signon f4.3, then a manifest body)
  → package_registration       (registers what the manifest declares - needs the FULL estate, 1033)
  → bap_signin                 ✓ full BAP setup: 0x1e→0x1f, 0x19→0x1a, 0x79→0x7a, 0x12e→0x12f
  → investment_signin          ✗ error.no_investment_connection ("chicken")  ← WE ARE HERE
```

Twelve of fourteen. The client also reports `world_controller: successfully changed world to:
ambient_bootstrap` - it is loading worlds, not just changing states.

### ⚠ shadPS4 shader-translator fix (2026-08-06) - a LOCAL MODIFICATION to the control build

`V_CMP_CLASS_F32` killed the emulator outright while compiling shaders for `ambient_bootstrap`:
`vector_alu.cpp:1325 Unreachable code!`, mask `0x2`. The stock code tested `(mask & NaN) == NaN`,
demanding **both** NaN bits, so a **quiet-NaN-only** mask matched no branch. Partial masks are legal
in GCN - `V_CMP_CLASS` tests membership of an arbitrary subset of ten float classes.

Rewritten to build the predicate per-bit (NaN, signed infinities, finite negatives, finite positives,
empty mask → false). Emulator now survives the world load. **This modifies `~/dev/shadPS4-main`, which
was the pristine control build** - the change is a normal commit there, and every run records
`emulator commit` in `RUN-INFO.txt`, so affected runs stay identifiable. Worth upstreaming.

### ⭐ shadPS4 local-IP fix (2026-08-08) - a FORK-LOCAL MODIFICATION to `shadPS4-d1trace`

**The guest was being told its own address is `127.0.0.1`, and told it *successfully*.**

`NetUtilInternal::RetrieveIp()` (`src/core/libraries/network/net_util.cpp`) determined the console's
IP by opening a **`SOCK_STREAM`** socket and `connect()`ing to **`1.1.1.1:80`**. Our containment has
no default route, so that fails `ENETUNREACH` - and `sceNetCtlGetInfo(ORBIS_NET_CTL_INFO_IP_ADDRESS)`
then returned **`ORBIS_OK` with a fabricated `"127.0.0.1"`**.

**Who asks, and why it is not cosmetic** `[C]`: D1's **Demonware** layer asks - eboot **`0x2E9FC0`**
(`demonware/45/bdCore/...`, subsystem `networking:demonware`) calls `sceNetCtlGetInfo(0xE)` and feeds
the answer straight to `sceNetInetPton(AF_INET, …)`. Its own strings are *"networking:demonware:
Error calling sceNetCtlGetInfo to obtain local ip address:"* and *"Couldn't parse local IP address
%s"* - **neither could ever fire, because `127.0.0.1` parses cleanly.** A second consumer,
`0xAE9940`, publishes the value under the key **`insecure_ip`**. ⇒ This answers
`PLAN-B-SHADNET-MATCHING2.md` §9.11 with a third option it did not list: the peer-advertised address
comes from **`sceNetCtlGetInfo`**, not `sceNpSignalingGetLocalNetInfo` - **D1 does not import a single
`sceNpSignaling*` NID.**

**The change** (both files carry a `D1-PRESERVATION FORK-LOCAL CHANGE - NOT AN UPSTREAM FIX` banner):
1. `net_util.cpp` - the route probe becomes **`SOCK_DGRAM`** (a `connect()` on a datagram socket does
   the route lookup and source-address selection, **sends no packet and cannot block**), with a
   **`getifaddrs()` fallback** when there is no route at all.
2. `netctl.cpp` - on total failure, `sceNetCtlGetInfo(IP_ADDRESS)` now returns
   **`ORBIS_NET_CTL_ERROR_NOT_AVAIL`** and zeroes the buffer instead of reporting success with a
   plausible lie, so the guest can take its own error branch.

⚠ **The obvious fix - "just use `getifaddrs`, first non-loopback" - is wrong on this box**, and
`PLAN-B-SHADNET-MATCHING2.md` §6.4 prescribed exactly that. This box has **five** non-loopback IPv4
interfaces and the first is `10.99.99.1` (the PS4 capture segment), not the routable `203.0.113.84`.
The probe is kept *because* it selects the routable source address; only its protocol changed.

**Control:** `capture-rig/retrieve-ip-control.py` - **extracts the shipped function body verbatim
from `net_util.cpp`**, compiles it, and runs three arms: host-with-route → the kernel's own routable
source; contained netns → `10.77.0.2`; contained netns **with an unreachable default route** →
`10.77.0.2` *promptly* (the old `SOCK_STREAM` probe still hangs there at a 60 s cap - measured).
`--meta` compiles the naive getifaddrs-only variant and asserts it **FAILS** arm A, so arm A is a
real control and not decoration. Exit 2 = do not cite. **3/3 arms pass, meta passes.**

⚠ **Not yet observed in a run.** This is a build-time result; it changes what D1 is told about
itself, so it needs one boot-to-orbit control run before anything downstream is trusted (see `Next`).

### BAP handshake, complete `[C]`

```
client -> 0x001e c->b start req    140B, "DESTINY", envelope mode 2
  we   -> 0x001f c->b start rsp    142B: 8-byte header (status 200) + 128-byte payload
client -> 0x0019 b->c shello req    39B, echoes OUR signon f2.4 security token
  we   -> 0x001a b->c shello rsp    98B: sealed, AES-128-CBC + HMAC-SHA256 keyed by signon f2.2/f2.3
client -> ???                       28B, **envelope mode 1** = the secured channel
```

Two traps that cost runs, both worth remembering:

1. **Reply inner headers are 8 bytes, not 6** (6-byte base + `u16be status` = 200). Eight header
   classes exist (6/6/**8**/14/6/24/32/8). A short header parks the client in a resumable header-read
   with **no timeout, no log and no error branch** - the symptom is total silence, not a complaint.
2. **`decode_payload` gates on `L < 93`, not `L >= 93`.** Sealing a 32-byte plaintext (already
   block-aligned) makes PKCS#7 add a whole extra block → `L = 96` → `failed to decode [bad size 100]`.
   Let 28 bytes pad naturally to 32. Note the client prints the **header-stripped body** size (100),
   not the envelope payload (108).

**Mode 1 (the secured channel) is SOLVED** `[C]` - **AES-128-GCM**, 12-byte nonce, 16-byte tag, no
AAD, and the **tag PRECEDES the ciphertext** on the wire:

```
envelope(6) || tag[16] || ciphertext
```

Key is the session key we issued in the secure hello (`plaintext[12..28]`). The nonce comes from the
channel seed (`plaintext[0..12]`), with the **low bit of the last byte as a direction flag** - client
sends under `…d0`, we send under `…d1` - and independent counters per direction.

Verified the strongest way available: the **real captured 22-byte packet authenticates** under the key
we issued and decrypts to `00 79 00 00 00 02` - a bodyless `register subscriber` request at sequence
2. A GCM tag verifying confirms cipher, nonce derivation, key and wire order *simultaneously*; it
cannot pass by coincidence. Test: `d1-bap` `captured_mode1_packet_authenticates_and_decodes`.

`[U]` **The nonce increment rule is unconfirmed** - only one packet per direction has been seen. The
implementation uses "+2 per packet, preserving the direction bit", which is the least-surprising model
consistent with that single sample and nothing more. If the channel desynchronises after the second
message either way, suspect this first.

⚠ **"Centipede" on screen is the outer 10 s `bootflow:bap_signin` timeout, not a handshake failure.**
Every run since the secure channel came up has crossed the entire handshake and then stalled on the
next *unanswered* RPC. Read the BAP sub-state, never the UI.

**The content problem is solved, and the manifest is now regenerable.** A 1033-package manifest
(neutral+English, all generations, built from observed package headers) opens 1036 packages and
clears the tag loader with no VHALT. The 175 symbol-bearing packages are the *name-resolution* set
only - zero-symbol packages are real dependencies and must be registered too.

⭐ Rebuild the served manifest with **`d1-pkg manifest -o capture-rig/manifests/full_en.bin`**
(default policy `all-generations --lang en`). It reproduces the served file **byte for byte**;
`d1-pkg controls` pins that, exits 2 on failure, and its output may not be cited otherwise. This
closes ledger L10: `full_en.bin` is gitignored, so a fresh clone has to regenerate it, and until
now nothing could. The old prototype `capture-rig/make-manifest.py` is **superseded** and kept only
as the source of the pinned golden digests.

**We are now at the BAP transport boundary, and the client is talking** `[C]`:

```
TCP 10.77.0.2:3075  ->  140 B
01 02 00 00 00 86 | 00 1e 00 00 00 00 "DESTINY" 01 <key/nonce-looking material>
^type ^ big-endian length 0x86 = 134;  134 + 6 = 140
```

`[I]` 6-byte envelope = 2-byte type + 4-byte BE length, corroborated by the client's own
`Failed to receive data by offset [size 6][offset 0]`. It then waits, times out in
`_channel_starting`, and raises `_connection_failure_suicide`. → `historicaldocs/design/bap-protocol.md` (in progress)

⚠ **`f2.6` (bap_server_ip) is LITTLE-endian; `f14` (client_external_ip) is BIG-endian.** Same message,
same `uint32` type, opposite byte order. Big-endian `f2.6` made the client dial **2.0.77.10** and fail
with `no_bap_connection`. Regression test: `d1-responder` `bap_ip_and_client_ip_use_opposite_byte_orders`.

**The remaining path to the goal is four states, and it is now explicit** `[C]` - the eboot contains
**22** `bootflow:` state literals; these 14 are the spine, the other 8 are conditional branches
(`eula`, `email`, `upsell`, `queuing`, `content_install`, `confirm_account_transfer`,
`illegal_dlc_media`, `insufficient_content_space`):

```
… bap_signin  →  investment_signin  →  profile_setup  →  prepare_for_orbit
     (here)                          (character select)      (ORBIT = the goal)
```

The spine is encoded in `d1_run::BOOTFLOW_SPINE` and mirrored in the GUI, so `d1` can report depth as
`N/14` and name what remains. Also `[C]`: the binary holds **260** `error.*` reason strings, including
a large `error.private_activity_host_*` family - that is our target scope (3v3 private match), so the
vocabulary for it exists and is readable. Extract: `historicaldocs/design/content-manifest-gate.md` §10.

**Error codenames observed on screen, mapped to internal reasons** - none of these three are
documented publicly for D1:

| codename | internal reason |
|---|---|
| Marionberry | `error.transport_method_invalid` |
| Squirrel | platform-account problem |
| **termite** | `error.content_manifest_not_provided` |
| **chicken** | `error.no_investment_connection` |
| **weasel** | `error.lost_bap_connection` - BAP came up and was then torn down. Distinct from centipede (never connected). |
| **toad** | `error.content_manifest_url_invalid` |
| ⭐⭐ **OLIVE** | **`error.investment_peer_manager`** - ⭐ **FIRST OBSERVATION IN D1, 2026-08-09**, run `20260809-223241-TASK7-SUPPRESS`. Fires at rung **30** `setup:activity_host_setup` → `cleanup`. Driven by **59×** `networking:investment:peer: Failed peer-subscription validation [error error_timeout][account_soid 0xD100000000000001].` plus one `Account soids in error state:`. ⚠ **OLIVE is a documented *Destiny 2* code** (generic networking/server-side) and appears to be **unreported for D1 anywhere public** - so this row is the codename→reason mapping *and* the internal diagnostic, which no public D2 source carries. ⛔ **It is TIME-DEPENDENT, not config:** attempt 1 of the same run reached rung 33 before the peer subscription soured; attempt 2 bailed at rung 30. ⇒ **a poke after olive appears is a poisoned-connection measurement** (`LESSONS.md`). |

⚠ **The lab patch is a client modification.** Runs past `content_check` used
`patches/CUSA00219-lab-manifest.xml`, which clears the CDN manifest RSA-envelope byte. Which patch a
run used is recorded in its `RUN-INFO.txt`. Everything reached through it is `synthetic_ancestor`.

`sceNetCtlGetInfo: local ip: failed to retrieve` is a **shadPS4 stub gap**, not a game problem: it is
why the client reports `Client local address: [127.0.0.1:65535]` even though the namespace has
`10.77.0.2` on `d1net`. Harmless now that f14 of the signon response supplies an external address, but
it will matter for P2P. Worth fixing emulator-side.

## Next

- [ ] **Build a manifest that declares packages** - the live blocker. `bap_signin` halts without
      `client_bootstrap_patchable`. 795 distinct package base names in the dump vs a 1325 cap, so a
      complete manifest fits; open `[U]` is `Package.id` semantics and the `path` string form.
      → `historicaldocs/design/content-manifest-gate.md` §9.5
- [ ] Redirect **all 26** eboot hostname literals, not the subset - `dm-partnernet.upload.deadorbit.net`
      was missing and cost a diagnosis detour. List in `historicaldocs/design/content-manifest-gate.md` §9.4.
- [ ] `[U]` **Probe the silo_id transform**: we sent 12345, the engine reports 28729 (= +0x4000).
- [x] ~~Clear `error.content_manifest_not_provided`~~ - done; it was signon `f4.3` key 100.
- [x] ~~Fix `sceNetCtlGetInfo` to report the namespace's real local IP~~ - done 2026-08-08,
      fork-local in `shadPS4-d1trace`; see the local-IP fix section above. 3/3 control arms + meta.
- [ ] ⭐ **Run ONE boot-to-orbit control run on the new binary.** The fix changes what D1 is told its
      own address is (`127.0.0.1` → `10.77.0.2`), and Demonware consumes it. Expect in the log:
      `Lib_Net local ip 10.77.0.2 (interface d1net, no default route -- getifaddrs fallback)` and
      `Lib_NetCtl local ip: 10.77.0.2`; expect **no** `networking:demonware: Error calling
      sceNetCtlGetInfo` and **no** `Couldn't parse local IP address`. Orbit must still be reached.
      Until that run exists the fix is `[C]` at build time and `[U]` in the client.
- [ ] `[U]` **Settle `PLAN-B-SHADNET-MATCHING2.md` §9.1 from the same log** - `grep Lib_NpMatching2`.
      shadPS4 already logs `LOG_INFO(Lib_NpMatching2, "called")` on **every** D1-imported entry point
      including the three that do nothing, so **no emulator change is needed to run this experiment**.
      Positive control in the same grep: `Lib_NpWebApi` must be present.
- [ ] Capture a title-screen frame as the baseline artifact (GNOME dbus screenshots are blocked by
      policy; use the emulator's own screenshot path or x11vnc)
- [ ] Compare our full log against the public 0.17.0 trace call-for-call; log divergences
- [ ] File the init-ordering-under-contention fragility upstream
- [x] ~~Light the NP identity gate~~ - done by our own Layer A server, `status/server.md`
- [x] ~~Record a proper run artifact~~ - `run-contained-emulator.sh` writes `RUN-INFO.txt` +
      `containment.txt` per run
- [x] ~~Get past the crash~~ - it was self-inflicted load, not a shadPS4 defect
- [x] ~~Rule out GPU / driver / unified memory~~ - lavapipe reproduces it exactly
- [x] ~~Confirm the AOT patcher rewrote the accessor thunks~~ - both, verified in memory
- [x] ~~Test stock current upstream~~ - identical behaviour; fork exonerated

## Notes worth keeping

- **Cert pinning is not enforced on this path.** `sceHttpsLoadCert` and both
  `sceHttpsSetSslCallback` callbacks are STUBBED, so pointing the emulator at our own server needs
  no cert work. On hardware it will. See `historicaldocs/SHADPS4-FIRST-LIGHT.md` → *Bonus intel*. **Live now:** with
  identity open the game actually reaches 2 `sceHttpsLoadCert` calls - worth identifying.
- `isConnectedToNetwork=false` is **not** containment - the resolver still calls host
  `getaddrinfo`. External default-deny egress is a launch precondition either way.
