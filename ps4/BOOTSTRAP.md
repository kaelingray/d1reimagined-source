# BOOTSTRAP - from a fresh clone to a working tree

This repository ships **generators, servers, and research text** - about 10 MB that is ours. It
deliberately ships **no game, patch, or firmware asset** and none of the artifacts derived from
them. Everything derived regenerates in one command against a disc you own, with its own controls,
and refuses to be cited if they fail. This file is the order in which to do that.

Every command below was verified on the reference box on 2026-08-11: the script or binary exists,
the flag spelling is real, and where a pass count is given the command was actually run. Steps that
need the ~50 GB dump, a live client, or minutes of CPU are marked as such rather than implied to
have been re-run for this document.

---

## 0. Prerequisites and the legal frame

**Owned media only.** You supply your own disc and console. This project keeps a game playable
after its online services retired; it does not distribute the game. Nothing in this repo lets you
play Destiny without owning it, and nothing you generate from your own dump may be redistributed.

The pinned target, which every tool gates on:

| | |
|---|---|
| Title | Destiny (PS4), `CUSA00219` (US) |
| Patch | 01.29 |
| Decrypted eboot sha256 | `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954` |

The sha256 is pinned in `capture-rig/guest-symbols.json` (the `build` block) and asserted by
`d1-eboot controls`. A different SKU or patch level will fail controls everywhere, by design.

Software you need on the box. **Tiered**, because most of the tree works with only the first
tier - do not install everything before you need it.

### Tier 1: required to build and to regenerate anything

* **Rust**, stable toolchain with `cargo`. Builds the whole `d1-server` workspace.
* **Python 3.**
* **capstone** (x86 disassembler; 8 `capture-rig/` tools import it, including `eboot-atlas.py`,
  which is step 3b's first real generator):
  `python3 -m pip install --target ~/.local/lib/d1-tools capstone`
* **unicorn** (CPU emulator; runs the client's own decompressor and codecs from Python, needed by
  everything under `capture-rig/oodle/` and so by step 3b item 1):
  `python3 -m pip install --target ~/.local/lib/d1-tools unicorn`

Both Python packages install to the same prefix on purpose; scripts that need them look there, or
accept `PYTHONPATH=~/.local/lib/d1-tools`. See `capture-rig/oodle/README.md`.

### Tier 2: required to run the game

* **shadPS4**, built separately, outside this repo. See step 5 - it carries a `[U]` you should read
  before assuming a fresh clone can reproduce the reference build.
* **A Linux kernel with `/dev/uinput`**, writable by your user. `capture-rig/virtual-pad.py`
  creates a real evdev gamepad through it, which is how an unattended run presses buttons. On the
  reference box a POSIX ACL grants the user `rw` so no `sudo` is needed
  (`crw-rw----+`); if yours does not, grant it rather than running the pad as root.
* **`unshare`** (util-linux) and **`nft`** (nftables). `run-contained-emulator.sh` builds the
  network namespace and asserts containment through them, and **refuses to run if it cannot prove
  containment**. Without these there is no supported way to launch.
* A **Vulkan-capable GPU driver**. The reference box is Radeon 8060S on RADV; the emulator is a
  Vulkan client.
* **A local resolver the run can point the client at.** The containment launcher rewrites the
  namespace `resolv.conf` to `127.0.0.1` and redirects hosts, so name lookups the client makes
  must be answered inside the namespace rather than reaching the internet. You need a local DNS
  responder listening on `127.0.0.1:53` (the run brings up the answering estate; a stub resolver
  such as `dnsmasq` or `systemd-resolved` bound to loopback is enough to spin one up if your box
  has none). The launcher asserts reachability and refuses to start if the client could escape to
  a real nameserver, so this is not optional - it is part of proving isolation. In-game network
  settings are not a substitute.

### Tier 3: optional, per task - install when the task arrives

| tool | for | notes |
|---|---|---|
| **Ghidra** 12.1.2 + **JDK 21** | decompilation and semantics | ⇒ **`design/ghidra-bringup.md` is the full procedure** (install, seed from our atlas, verify, decompile), and it is a prerequisite for the 36 citations of the decompiled corpus across 13 docs. ⚠ JDK 21 specifically: the launcher rejects Fedora's JDK 25. Needs ~48 GB of heap and ~2.2 GB of disk, both outside the repo. ⛔ Neither the project nor the corpus is ever shipped - you regenerate both from your own dump. |
| **RenderDoc** 1.42 | frame capture, the framerate frontier | Fedora 43 ships it via `dnf`; the reference install is `/usr/lib64/renderdoc/librenderdoc.so` with `renderdoccmd` on PATH `[C]`. No shadPS4 rebuild needed - it loads as a Vulkan layer. Set `D1_RENDERDOC=1` **at launch** (it implies `D1_X11=1`; a running client cannot be retrofitted). Evidence: `design/renderdoc-orbit-jank.md`. |
| **Zeek** 8.2.1 + **podman** | pcap to structured logs, and run-to-run diffs | `capture-rig/zeek-logs.py` runs it as a **digest-pinned container**, so you need podman rather than a host Zeek. `--self-test` builds its own control pcap in a sealed namespace (14 controls). |
| **Wireshark** (`dumpcap`) | packet capture | `capture-rig/capture.sh` re-execs via `sg wireshark`, so your user needs that group. ⚠ A contained run is inside a network namespace: capturing from the host sees **nothing**. |
| **aria2c** | the patch estate downloader | only for `psn-acquisition/`; not needed to build or run anything. |
| **RenderDoc MCP server** | agent-driven frame inspection | lives in `external/agentic-renderdoc-master/`, outside this repo and not required. |

## 1. Obtain the decrypted dump

Everything downstream needs this one input. There is no substitute and no fallback path.

Console-side procedure: **`capture-rig/CFW-DUMP-RUNBOOK.md`** - taking an owned 10.00/10.01 PS4
through GoldHEN to a decrypted dump, entirely inside an isolated network segment. Non-negotiable
while you do it: the CFW console must never reach the internet (a firmware update past 13.00 is
irreversible), and containment is by topology, not by console settings.

Where the tools expect the dump to land (paths are compiled and scripted defaults, verified in
`d1-eboot`, `d1-pkg`, `capture-rig/run-boot.sh`, and `capture-rig/oodle/extract-entry.py`):

```
dumps/patch-01.29-CUSA00219/eboot.bin       the decrypted 01.29 eboot (sha256 above)
dumps/patch-01.29-CUSA00219/packages/       the content-package estate (~50 GB, 1655 files)
dumps/sys_modules/                          26 decrypted system modules
```

The 26 modules are installed for the emulator at `~/.local/share/shadPS4/sys_modules/`
(`status/shadps4.md`, Pinned table).

---

## 2. Build

Three separate Rust trees, each with its own `target/`:

```
cd d1-server && cargo build --release        # the workspace: d1, d1-eboot, d1-pkg, d1-oracle,
                                             # d1-replay, d1-live, d1-responder, d1-loadout,
                                             # d1-forge, d1-harness, d1-activity-host, d1-bap,
                                             # shadnet-server, ... -> d1-server/target/release/
cd d1-trace  && cargo build --release        # run viewer (d1-trace, d1-dash) -> d1-trace/target/release/
cd d1-observe && cargo build --release       # -> d1-observe/target/release/
```

⚠ **Whole-workspace `cargo build --release` and `cargo build --release -p <crate>` produce
DIFFERENT binaries** (cargo feature unification across workspace members). This has cost a real
session: a workspace build once pulled a second rustls crypto provider into `d1-responder` through
unrelated crates and the TLS listener died silently while plain HTTP kept serving. The root cause
is fixed in code, but the lesson stands: build the whole workspace, do not mix workspace-built and
`-p`-built binaries in one experiment, and never treat a clean compile as evidence the server
works - assert the run instead (step 5).

---

## 3. Regenerate the derived artifacts, in dependency order

The gitignore policy (see `.gitignore`, "EBOOT- AND CONTENT-DERIVED ARTIFACTS"): the generators
ship, the output does not, because the output is mechanically derived from copyrighted material.
A fresh clone is therefore missing every file below until you regenerate it from your own dump.

### 3a. Needed before you can run the game

**`capture-rig/manifests/full_en.bin`** - the served CDN content manifest. Without it the client
dies about 90 seconds in with `RELEASE_VHALT / failed to load bootstrap_patchable`, an error that
reads as a package or bootstrap regression and has cost this project runs twice. `[C]` It is not a
regression; it is this file being absent. Regenerate:

```
d1-server/target/release/d1-pkg manifest -o capture-rig/manifests/full_en.bin
```

Defaults (`--policy all-generations --lang en`) reproduce the served 52,797-byte file **byte for
byte** from your package headers; the sha256 is pinned inside `d1-pkg controls`
(`design/architecture.md` ledger row L10). Needs `dumps/.../packages/` and the built `d1-pkg`.

### 3b. Needed before you can cite any static analysis

In this order (later items read earlier ones):

**1. `capture-rig/schemas/investment-message-definitions.bin`** - the 101-record investment
message-definition table. It is shipped CONTENT, not eboot: content tag `0x80A5E37A`, package 303
entry 890 (`design/character-and-investment.md` section 6.1 `[C]`). Extract it through the
client's own decompressor:

```
mkdir -p capture-rig/schemas
python3 -B capture-rig/oodle/extract-entry.py 0x80A5E37A \
    -o capture-rig/schemas/investment-message-definitions.bin
```

Needs the packages, the built `d1-pkg`, the eboot, and unicorn (section 0). `[I]` Do this before
running `d1-eboot controls`: the catalogue controls read this file at its default path
(`d1-eboot catalogue --help` documents it) and are expected to fail without it. Not tested with
the file absent, because the reference tree always has it.

**2. The eboot atlas TSVs** - `capture-rig/eboot-{atlas,types,namehash,callgraph}.tsv` (plus
refreshed copies of the tracked `eboot-imports.tsv` and `eboot-thiscalls.tsv`):

```
python3 -B capture-rig/eboot-atlas.py              # regenerate every TSV, run every control
python3 -B capture-rig/eboot-atlas.py --controls   # controls only, writes nothing
```

It runs `d1-eboot controls` first and refuses to continue unless all pass. Roughly 47,000 rows;
takes minutes, not hours. Not re-run for this document.

**3. The remaining offline `capture-rig/schemas/` tables** (each script runs its own controls and
refuses to write on failure):

```
d1-server/target/release/d1-bap idspace --tsv > capture-rig/schemas/bap-message-idspace.tsv
python3 -B capture-rig/engine-config-keys.py  --tsv capture-rig/schemas/engine-config-keys.tsv
PYTHONPATH=~/.local/lib/d1-tools python3 -B capture-rig/progression-grants.py \
    --tsv capture-rig/schemas/progression-grants.tsv
```

**4. The Bungie-manifest-derived tables** (director graph and unlock flags). These need the
archived Bungie D1 API manifest under `manifest/`, which is gitignored and not shipped (it is
Bungie's data). Fetch it yourself:

```
python3 -B capture-rig/fetch-d1-manifest.py --out manifest/
```

Needs a Bungie API key in `~/.config/d1/bungie-api-key` or `$BUNGIE_API_KEY`. `[U]` Whether the
legacy D1 endpoints still answer today is unknown; they were alive-but-gated when the archive was
made, and they have broken at least once before (the script's header records this). Then:

```
python3 -B capture-rig/director-map.py            # director-*.tsv + director-map.json
python3 -B capture-rig/unlock-flag-reference.py   # unlock-*.tsv + unlock-flag-reference.json
```

⚠ REACH for everything in item 4 is `static-client-inference` at best against a 2020 manifest,
four years newer than the 01.29 client - the scripts' own headers carry the caveat.

Optional: `catalogue/` (item data joined for loadout work) regenerates with `d1-loadout build`
from the same `manifest/` archive.

### 3c. Live-derived - regenerate after your first runs, not before

* **`capture-rig/diagnostic-catalogue.tsv`** (and the tracked `design/diagnostic-catalogue.md`):
  `python3 -B capture-rig/diagnostic-catalogue.py --write`. ⚠ Measured on a clean worktree with
  zero runs while writing this file: it writes output, but two SEEN controls fail (one pins a
  corpus of at least 100 runs) and the output is stamped **must not be cited**. The static columns
  are fine; the SEEN column is measured from your own `runs/*/emulator.log` and only becomes
  citable once you have a run corpus.
* **`capture-rig/namespace-map.{json,tsv}`**: `python3 -B capture-rig/live-namespace.py` - needs a
  live client (runtime type tags exist only in a running process). `--dry-run` does the static
  half only.
* **`capture-rig/namespace-tag-identity.json`**: `python3 -B capture-rig/live-tag-identity.py`,
  also live.
* **Snapshots** (`d1-replay` inputs): none ship. Create your own from a live client with
  `d1-replay capture -o <file>.snap`, then gate it with `d1-replay controls <file>.snap`.

### 3d. Files with no generator - honest gaps

Four files in `capture-rig/schemas/` on the reference box have **no generator in the tree** and
are not shipped; nothing in the build or the launch path reads them, so a fresh clone does not
need them, but citations to them exist in older docs:

* `investment-503-schema.txt` - produced by a one-off LIVE registry walk on a running client
  (`capture-rig/message-catalogue.py` line 117 records this). `[U]` Reproducible in principle from
  a live client; no script exists.
* `investment-503-response-tree.txt`, `investment-message-definitions.tsv`,
  `unlock-flag-index-registry.tsv` - one-off session dumps. `[U]` No generator, no reader in code.

**`capture-rig/world-namespace.tsv`** - untracked as of 2026-08-11 and it is the one entry that
fails this repo's own test. 1,114 symbol names extracted from copyrighted packages, so it cannot
ship under the owned-media rule; and **nothing in the tree writes it, in any language**. A fresh
clone therefore cannot obtain it at all. Nothing in the build or the launch path reads it, so this
does not block you, but it makes a `d1-pkg` symbol-inventory writer **required work rather than a
nice-to-have**. ⇒ `design/capture-rig-audit.md` section 2.

### 3e. Everything else that is derived, and the one command each

Added 2026-08-11 when the untracking policy was applied in full. None of these block a first boot;
regenerate them when you need the thing that reads them. Each was verified to actually reproduce.

| file | regenerate with |
|---|---|
| `capture-rig/item-content-index.tsv` | `d1-oracle item-index write` |
| `capture-rig/item-art-arrangement.tsv` | `d1-oracle art-arrangement write` |
| `capture-rig/eboot-thiscalls.tsv` | `d1-eboot thiscalls` |
| `capture-rig/eboot-imports.tsv` | `d1-eboot imports --tsv` |
| `capture-rig/bap-message-table.tsv` | `d1-bap message-table --tsv` |

The two harness fixtures under `capture-rig/testdata/` are NOT in that table: they stay **tracked**,
because their generators were archived the same day on the reasoning that the committed fixtures
superseded them. Untracking the output of an archived generator leaves a clone with neither. That
pair of individually-reasonable decisions is recorded in `status/LESSONS.md`.

⭐ The two item tables have a **golden diff you can run yourself**, which is the honest way to
believe the table you just generated:

```
d1-server/target/release/d1-oracle item-index diff
```

It renders the table from your packages and compares byte for byte against a file the Python
wrote, exiting 2 on any difference outside the generator byline. On the reference box: identical,
7,552 lines, 203,123 bytes, same sha256 once rendered with the other tool's byline.

⚠ These two were held back from untracking for most of a day on a stale note claiming `d1-oracle`
could only *consume* them. It regenerates them. **Re-check a capability before treating it as a
constraint** - the note was true when written and false by the time it was acted on.

---

## 4. Prove the tree is sound before trusting anything

Project rule, stated everywhere the tools are documented: **exit 2 means the output may not be
cited.** Run each tool's controls before citing anything it says. Pass counts below were measured
on the reference box on 2026-08-11 with the full dump present; your counts should match unless a
tool has since grown controls.

| command | measured result | needs |
|---|---|---|
| `d1-server/target/release/d1-eboot controls` | 45 PASS, exit 0 | eboot + schemas bin (3b.1) |
| `d1-server/target/release/d1-pkg controls` | 20 PASS, exit 0 | packages |
| `d1-server/target/release/d1-oracle control` | 2 PASS, exit 0 | eboot |
| `d1-server/target/release/d1-live controls` | 11 PASS, exit 0 | eboot (offline, no emulator) |
| `d1-server/target/release/d1 controls` | 7 PASS, exit 0 | in-crate testdata only |
| `python3 -B capture-rig/zeek-logs.py --self-test` | 14 PASS, exit 0 | zeek installed |
| `d1-trace/target/release/d1-trace controls` | 46 passed, exit 0 | see caveat below |
| `d1-server/target/release/d1-replay controls <snap>` | not run here | a snapshot you captured (3c) |

Note the spelling: `d1-oracle`'s gate is `control` (singular); several of its subcommands
(`activity-host`, `sensor`, ...) carry their own further `controls`.

⚠ `d1-trace controls` was verified passing on the reference box, where its assertions reference
real run directories under `runs/`. `[U]` Whether all 46 pass on a fresh clone with an empty
`runs/` was not tested; if it exits 2 before you have runs, treat its output as uncitable until
you have a run corpus, not as broken.

---

## 5. The first contained run

### 5a. The emulator, which this repo does not contain

**shadPS4 is a separate project and is not vendored here.** Build it yourself from upstream
(`github.com/shadps4-emu/shadPS4`, a CMake project); its own README carries the current build
dependencies, which change faster than this file could track. Then point `SHADPS4_BIN` at the
binary. `run-boot.sh` defaults it to `~/dev/shadPS4-d1trace/build-d1trace/shadps4`, which is the
reference box's path and will not exist on yours.

**What upstream gets you `[C]`:** stock shadPS4 boots this game. The reference box measured stock
main behaving **identically** to the local fork on the crash under investigation, and the reference
commit for both trees is `1a51a416` (`status/shadps4.md`). So a clean upstream build is a
legitimate starting point, not a degraded one.

**What it does NOT get you `[U]`:** the reference tracer tree carries **16 uncommitted modified
files** (`voice.cpp`, `net_util.cpp`, `netctl.cpp`, `p2p_sockets.cpp`, and others). Those are the
UD2 runtime-probe and network-trace additions. **They are not in this repository and are not
reproducible from it** - `design/instruments-live.md` and the `runtime-probe` skill describe the
method, not a patch series you can apply. Consequence, stated plainly: **a fresh clone can boot the
game and run the server, but cannot reproduce the guest-side probe traces.** Closing that gap means
either publishing those patches or re-deriving them from the method docs; nobody has done it.

⚠ One emulator at a time, and the game needs a quiet box: `ulimit -c 0`, and never start a run
while `systemd-coredump` is still compressing a previous 16 GB core - that contention alone was
enough to crash the game during init, and it cost hours of misdiagnosis.

### 5b. The launch

⛔ Do not invent a launch procedure. `capture-rig/run-boot.sh` is the ONLY launcher that pins the
known-good environment (manifest file, lab patch, containment, pad law), and it prints every
setting at startup - that block is the run's provenance.

⚠ **Corrected 2026-08-11, this paragraph used to name the wrong script.** `run-boot.sh` delegates
to **`capture-rig/run-signon-experiment.sh`** (line 410), and that is where containment actually
lives: `unshare -rnm`, `resolv.conf` to 127.0.0.1, nss `resolve` stripped, a hosts redirect, and an
in-namespace reachability ABORT before anything runs. It refuses to start if it cannot prove
isolation. `run-contained-emulator.sh` is a second, drifted implementation that **nothing execs**;
verify the launcher you are actually running, not the one a doc names. In-game network settings are
not containment. ⇒ `CLAUDE.md` non-negotiables, `design/capture-rig-audit.md`.

```
# 1. THE PAD, FIRST. udev hotplug is network-namespaced; a pad created after the emulator
#    enters its namespace is never announced to it.
d1-server/target/release/d1-harness pad run --fifo /tmp/d1pad &
#    ⭐ For TWO instances you must give each pad a distinct identity, or SDL sees one device:
#    ... pad run --fifo /tmp/d1pad-b --product 0x028F &

# 2. The run.
SHADPS4_BIN=<your shadps4 build> capture-rig/run-boot.sh FIRST-BOOT
```

Set `OPERATOR_PAD=1` if a human with a real controller is driving (menus are cursor-driven; do not
drive both pads at once). `run-boot.sh` pins `D1_MANIFEST_FILE` to the file you built in 3a and
`DIAG_PATCH` to the tracked lab manifest patch (`patches/CUSA00219-lab-manifest-T1.xml`); without
the manifest the client dies with the misleading `bootstrap_patchable` halt described in 3a.

A healthy start (from the `boot-run` skill, section 3), checked in the newest `runs/<name>/`:

* `responder.log` contains `listening` lines **including the :443 TLS listener** - that is the one
  that dies silently.
* `grep -c "panicked" responder.log` is 0.
* The client reaches the title screen and, with the server answering, moves past `SIGNED_OUT`.
* Regression gate, once you have a first good run to compare against:
  `d1-server/target/release/d1 gate <run>`.

If it crashes: the client uploads its own crash report. The responder saves it under
`runs/<run>/uploads/ticket_drop-*.bin`; `crash_info.txt` inside has the stack, the halt line and
the bootflow state. Read it before theorising.

Two operational traps that have cost runs: `pkill -f` matches the shell that issued it (use a
character class: `pkill -9 -f "release/[d]1-responder"`), and `pgrep -x shadps4` matches nothing
(the process name is `shadPS4:Main`; use `pgrep -f "[s]hadps4 -p "`).

---

## 6. What you will NOT have - and that is deliberate

* **`dumps/`** - your own console dump replaces it (step 1).
* **`runs/`, `captures/`, `logs/`** - per-experiment artifacts; yours accumulate from step 5.
* **The patch estate** (`psn-acquisition/`) and **`cfw-staging/`** - preserved copyrighted
  material, never shipped.
* **`historicaldocs/`** - superseded internal docs, local reference only. Where a tracked doc
  cites `historicaldocs/...`, the citation is to history you do not have; the live claim is
  restated in the tracked doc.
* **The decompiled corpus and the Ghidra project** - reproduce from your own disc with
  `ghidra/d1-ghidra.sh build` (imports the eboot, seeds it from the atlas you built in 3b, runs
  addressing controls; `decompile` regenerates the corpus in minutes).
* **`manifest/` and `catalogue/`** - Bungie's data; fetch and derive your own (3b.4).
* **Snapshots and the emulator** - captured and built by you (3c, step 5).

A tree that has completed steps 1 through 4 can cite static analysis; a tree through step 5 can
run experiments. Read `CLAUDE.md` and `status/` before doing either.
