# Harness status

**The run/observe/pull apparatus. Contained lane works end to end, boot runs are reproducible and
self-checking, and runtime state is observable three ways: live values (`d1-live read|obj|tag|
descriptors`, ported byte-identically from `d1guest.py`, whose `poke` is all that is left of it),
offline re-execution (`d1-replay`), and the write event itself (`d1-live watch`). The live
console-capture lane, the irreplaceable one, is half built: the analysis half is done and
controlled (`capture-rig/zeek-logs.py`), the capture half still waits on hardware.** As of
2026-08-11.

Instrument depth lives in the thematic docs; this file is the operational index:
* **`design/instruments-live.md`** - d1guest.py, d1-live watch, d1-scope, the network tracer,
  guest diagnostics. Controls, warnings (never SIGKILL an armed tracer), blind spots.
* **`design/instruments-static.md`** - the access-mode table (read it before picking a scanner),
  d1-eboot, the atlas, Ghidra, field-xref, d1-replay including `--stack-canary` and presets.
* **`design/observability-and-ui.md`** - d1-trace, d1-dash, the d1-observe conversation debugger,
  the Zeek pipeline, interface architecture, the Python retirement queue.

## Pieces

| Piece | What it does |
|---|---|
| `capture-rig/run-boot.sh` | **the only launcher that pins the known-good environment.** Prints every setting at startup; that block is the run's provenance. Wraps `run-contained-emulator.sh`. |
| `capture-rig/run-contained-emulator.sh` | contained run: one unprivileged netns + Layer A + emulator + provenance; asserts containment before anything runs, writes `containment.txt` |
| `capture-rig/rig-mode.sh {live\|contained\|status\|selftest}` | topology switch + containment assertion. ⭐ Real exit codes since 2026-08-11: **status 0=CONTAINED, 1=LIVE, 2=INDETERMINATE**, so `rig-mode.sh status && <thing>` is now correct. Verifies the drop RULES not just the table, and distinguishes cannot-read (no root) from absent. `selftest` checks the verdict logic with no root and no rig. |
| `capture-rig/virtual-pad.py` | uinput virtual gamepad; how an agent presses X. Start it BEFORE the run; `--print-ignore` emits the PAD-LAW ignore list; drive via FIFO (`echo 'press cross' > /tmp/d1pad`). ⭐ **Ported 2026-08-11 to `d1-harness pad`** (side-by-side equal, see below); the Python remains what the launchers invoke until the docs are re-pointed |
| `capture-rig/gate-check.py` | asserts the 9 gates already won on any run dir; **read it before reading anything else out of a run** |
| `capture-rig/harvest-crash.py` | harvests + symbolizes the guest's own crash report; the next run destroys it, harvest immediately. ⭐ **Ported 2026-08-11 to `d1 crash`** (stack-mode golden diff byte-identical over 17 real ticket_drop addresses; full-harvest stdout byte-identical over a staged real crash folder); archivable in the next reviewed batch |
| `capture-rig/d1guest.py` | live guest memory read/**poke**, liveness ladder L0-L3, `selftest` 5/5. ⚠ Its READ side is now ported (see below); **`poke` is still Python-only and is the only reason to reach for this script.** |
| `d1-server/crates/d1-live` | hardware watchpoints (`d1-live watch`) **plus the read side of `d1guest.py`**: `read · obj · tag · descriptors · syms · live · base`. 18/18 controls (10 watchpoint + 8 read-side), exit 2 = do not cite |
| `d1-server/crates/d1-replay` | the replay oracle: snapshot live memory, execute the client's own functions offline; 13 controls |
| `d1-server/crates/d1-eboot` | static analysis: `segments\|descriptors\|disasm\|xref\|field-xref\|atlas\|controls`, 31 pinned controls |
| `d1-server/crates/d1-oracle` | the native oracle: the client's own RSAT codecs under Unicorn; fixtures are made here. ⭐ **Since 2026-08-11 it also owns the CONTENT lane**: `oodle` (the client's own Oodle decoder + the `.pkg` entry walk), `item-index`, `art-arrangement` - each with `controls`, `write` and an executed `diff`. |
| `d1-server/crates/d1-harness` | two-instance harness, BUILT AND COLD; `preflight` exits 3 (REFUSED) on two named operator decisions. ⭐ Since 2026-08-11 also owns **`pad {run,print-ignore,controls}`** - the Rust port of `virtual-pad.py` with `--vendor/--product` (the two-instance VID/PID blocker, fixed at the kernel level); `pad controls` 24/24 with the device tier executed, exit 2 = do not cite |
| `d1-trace/` (`d1-trace`, `d1-dash`) | post-hoc run viewer + live dashboard; `d1-trace controls` = 46 invariants, exit 2 = do not cite |
| `d1` CLI (`d1-server/target/release/d1`) | `runs · show · diff · promote · serve · gate · crash · controls`; `promote` stamps `[C]`/`[O]` from host-quiet evidence automatically; `gate` is the boot-path regression assert (run it BEFORE reading anything else out of a run; exit 1 = the run is a confound), ported from `capture-rig/gate-check.py` - since 2026-08-11 its probe lane decodes `account-trace.bin` NATIVELY via `d1-trace`'s `probe_decode` (no Python at runtime; proven by running it with a poisoned `python3` on a 17,862-record real trace); `crash` harvests + symbolizes the guest crash report (prologue-anchored, never a guessed va-N); `controls` = 7 gate + 6 crash controls, exit 2 = do not cite |
| `capture-rig/session-params.py` | KTOBUCKET launch-parameter parse (`--ktobucket`), 15 controls + 4 inversions; also `--launch-poke` |
| `capture-rig/zeek-logs.py` | pcap -> conn/dns/ssl/x509 JSON + `--diff` of two runs; pinned Zeek 8.2.1 container; `--self-test` 14 controls |
| `capture-rig/ftp_dump.py` | recursive decrypted pull off the console + `MANIFEST.sha256` |
| `capture-rig/preflight.sh` / `capture.sh` | pre-run rig checks / `dumpcap` capture |
| `capture-rig/fetch-d1-manifest.py` | archives Bungie's D1 manifest as WARC + BagIt; API key outside the repo, redacted in records |

**`runs/` is gitignored.** `d1 promote <run>` emits the markdown block that lets evidence survive.

### ⭐ The CONTENT lane moved to Rust (2026-08-11) - and the blocker was imaginary

`d1-oracle {oodle,item-index,art-arrangement}` replace `capture-rig/oodle/oodle-decompress.py`,
`extract-entry.py`, `item-index.py` and `art-arrangement.py`. **Both generated TSVs regenerate
byte-identically**, apart from the single `# GENERATED by` line - proven with an executed `diff`,
not asserted:

```sh
d1-oracle item-index      diff --against capture-rig/item-content-index.tsv    # exit 0
d1-oracle art-arrangement diff --against capture-rig/item-art-arrangement.tsv  # exit 0
```

Three things this cost, all worth carrying:

* ⛔ **"The Oodle decode is ~30 s under Unicorn" was wrong** and it is why the lane looked
  expensive. One block: **0.83 s** in Python, faster in Rust. The minutes came from
  `extract-entry.py` launching `d1-pkg info` as a subprocess **once per package across 1,647
  packages** - 61 s of *system* time. `Estate::load` does it once, in process; there is no
  `/tmp/d1-plain-cache` in the Rust path. Whole staged art table: **0.3 s**.
* ⛔ **There was never a codec to port.** `oodle-decompress.py` is a 35-line wrapper around three
  eboot entry points, and `d1-oracle` had already ported the harness that calls them
  (`harness.rs`). The port is ~60 lines.
* ⭐ **The Rust harness's `memset` guard is stricter than the Python's** and blocks the decode by
  default: `DecodeSome` zeroes a frame local and returns to `0x1270A85`, which must be registered
  with `allow_stack_memset_from`. `oodle::DECODE_SOME_MEMSET_RETURN`, with a test that goes red if
  the site stops being load-bearing.

✅ **CLOSED 2026-08-11:** `d1-run/src/gate.rs` no longer shells out to
`decode-account-trace.py` and no longer keys repo-root discovery on any script path (the
`find_script` walk is deleted outright - the probe lane needs no repo root at all). The gate takes
an injected `ProbeDecoder`; the `d1` CLI wires in `d1_trace::probe_decode` (injection because
`d1-trace` depends on `d1-run`, so the direct dependency would be a cycle). Failure shape
preserved: a missing trace is still quiet, a refused trace is still a loud `probe_note` - but the
refusal is now always about the trace bytes, never about Python being absent. Verified by running
`d1 gate` over the pinned 17,862-record run with a poisoned `python3` on PATH: full decode, native
tally identical to the canonical decoder (8/15 probe-15 true), zero spawns. That was the last
runtime Python dependency of a shipped Rust binary.

### The virtual pad moved to Rust (2026-08-11) - and the two-instance blocker fell with it

`d1-harness pad {run,print-ignore,controls}` is a behaviour port of `capture-rig/virtual-pad.py`:
same uinput capability set, same default identity (`045e:028e` v`0110`), same FIFO vocabulary
(`press | tap | hold | axis | sleep | quit`) with the Python's defaults and reply strings byte for
byte, same PAD-LAW ignore list (`pad print-ignore` matched `--print-ignore` byte for byte,
executed side by side). The netns ordering constraint is now IN the tool: `pad run` prints the
pad-before-emulator note at startup and escalates to a WARNING when an emulator is already up.

What the port adds is `--vendor/--product/--name`: per-instance pad identities.
`pad controls --require-device` (24/24 PASS, 0 SKIP, executed on this box) proved with real
kernel devices: enumeration with the requested identity and exact KEY/ABS bitmaps; **two
simultaneous pads with distinct ids as two distinct devices** plus the inverted arm (two pads
with ONE id, measurably indistinguishable - the defect, reproduced); the FIFO protocol end to end
with events read back off the evdev node; and a kernel-visible field-for-field match against the
live Python pad. Two deliberate divergences, both refusals where the Python died: a malformed
number in a FIFO command and a negative sleep no longer kill the device mid-run.

⚠ Still `[U]`: SDL's mapping of a non-x360 GUID (a live-client question), and the launchers
(`run-boot.sh`, `autodrive.sh`, `CLAUDE.md`, `BOOTSTRAP.md`) still invoke the Python - re-point
them after the SDL question settles, then walk §8.7's retirement checklist for `virtual-pad.py`.

### The read side moved to Rust (2026-08-11)

`d1-live read|obj|tag|descriptors|syms|live` is a behaviour port of `capture-rig/d1guest.py`, not a
reimplementation: **byte-identical stdout and identical exit codes**, verified by
`capture-rig/golden-diff-d1guest-vs-d1live.sh` over 88 command comparisons (16 read-side commands x
4 fixture configurations, plus 5 `syms` patterns). Exit codes carry the same three facts the Python
carries and the skills branch on: **1** an ordinary refusal, **2** a control refused so the output
may not be cited, **3** the client DIED, which is an error and never a zero.

What made that verifiable with no emulator running is `d1-live __mem-target`: a fixture process that
maps the **real eboot** at a base and lays out an object registry from `guest-symbols.json`, a
package registry with a real signed back-offset, and a descriptor table. It can be made **wrong on
purpose** (`--perturb family-stride|verify-window|duplicate-anchor`), which is what finally gives the
read side controls that can FAIL. The Python's `selftest` never could: it needs a live client, and
nobody can ask a real client to be wrong.

#### The adversarial pass found four defects the golden diff could not reach (2026-08-11)

A byte-identical diff proves the two tools agree on the inputs the diff generates. It says nothing
about the inputs it does not. An adversarial re-verification drove both tools with malformed input
and found four places where the Rust **answered where the Python refused**. All four are fixed, and
all four are now the `garbage-in-refuses-loudly` control - which was itself inverted, by restoring
the wrapping multiply and watching the suite go to 17/18 exit 2.

| defect | what it did | now |
|---|---|---|
| **wrapping registry arithmetic** | `obj 576460752303423492:0` (that is `4 + 2^59`, and `0x60 * 2^59 == 0` mod 2^64) aliased onto family 4/0's entry and printed the **ACCOUNT object's bytes at exit 0**, headed "cross-check SKIPPED" | checked; refuses, as `d1guest.py` does |
| **`--as <unknown>`** | printed the provenance, the ladder and the `guest … -> host …, 64 bytes` header, then **no data at all, exit 0** - a successful read with an empty result | the value list is enforced, exit 2, matching `argparse`'s `choices=` |
| **defaulted `registry_layout` keys** | a symbol file with `entry_stride_family` deleted was answered from a compiled-in constant; the Python raises `KeyError`. ⛔ This is the sixth copy of exactly what `d1guest.py` collapsed five scripts to remove: *a place where a superseded constant survives a retraction* | required, never defaulted |
| **unbounded `package_registry_layout` offsets** | `slot_data_base = 0x3C` read an 8-byte field out of a `0x40`-byte record and **panicked**, exit 101. A crash is loud but it is not a refusal | bounds-checked; refuses |

Also tightened: `tag` refuses a value that does not fit in `u32` instead of silently narrowing it.

⭐ Two things the adversarial pass **confirmed** rather than broke: a poisoned base cache (a wrong
base, correctly keyed to the live pid) is caught by controls 2 and 3, which re-run on the cached
path exactly as the Python's do; and the read-side control group **fails rather than skips** when a
client is up, naming the reason.

⚠ Known limit of the fixture, stated so nobody over-reads a green suite: the fixture's runtime entry
table is generated **from** `guest-symbols.json`'s static rows, so corrupting the static table
corrupts both derivations together and the cross-check still passes. The control proves the
cross-check notices a **desync**; it can never prove the symbol file's contents are right.

⛔ **`poke` was deliberately NOT ported.** Writing into a live client is the one operation where a
half-ported tool is dangerous; it is the lowest-quality hop mechanic this project has (the value
never crossed a wire, so it is not evidence even about our own server); and its journal
(`runs/guest-writes.jsonl`) is what stops a write being forgotten between the run that did it and the
finding that cites it. It gets its own reviewed change. Until then `d1guest.py poke` is the tool.

⚠ **The live-parity gate is OPEN.** Every comparison above ran against the fixture, because the
emulator was closed. Two things the fixture cannot exercise: a **111-thread** process with a moving
heap, and a **real** package registry (its slot stride is read from `header+0x10` and hardcoded to
`0x40` by the Python; `d1-live` reads it and says so if it is not 0x40). Settle it on the next boot
with the pair in the "Next" list below.

## Standing run rules (each one already paid for)

* **The pad, FIRST.** udev hotplug is network-namespaced; a pad created after the emulator enters
  the netns is never announced to it. And THE PAD LAW: pass
  `SDL_GAMECONTROLLER_IGNORE_DEVICES="$(virtual-pad.py --print-ignore)"` so every physical pad is
  excluded and slot 0 goes to ours; the default once handed slot 0 to the operator's pad and every
  scripted press went nowhere.
* **One emulator at a time.** `pgrep -x shadps4` matches nothing (comm is `shadPS4:Main`); use
  `pgrep -f "[s]hadps4 -p "`. The `[s]` class is load-bearing: without it the pattern matches the
  shell evaluating it.
* **Sweep stale servers before launching, only when no run is live**; with a client up the only
  safe test is namespace identity (`readlink /proc/<pid>/ns/net`).
* **Quiet box:** `ulimit -c 0`, never launch while systemd-coredump is compressing a 16 GB core.
  Record `/proc/pressure/*` into `RUN-INFO.txt`; a determinism claim with no host-quiet evidence
  is `[O]`, not `[C]`.
* **`isConnectedToNetwork=false` is NOT containment**; netns alone is not either
  (systemd-resolved answers over a unix socket that crosses namespaces). The launcher bind-mounts
  resolv.conf/nsswitch.conf so DNS fails closed.
* **Capturing a contained run:** tcpdump on a host interface sees nothing; capture inside the
  netns (`nsenter -t <pid> -n tcpdump`), or better, log the nft drop rule - the gate is defined
  by packets that DON'T exist, so an attempt log beats a pcap.
* **The client uploads its own crash report** (`runs/<run>/uploads/ticket_drop-*.bin`,
  `crash_info.txt` inside). Read it before theorising.

## Known quirks (already paid for)

* shadPS4 collapses a log line identical to the previous one (`Skipped N duplicate messages..`)
  and this really does eat KTOBUCKET lines; `session-params.py --ktobucket` reports it.
* DHCP loses to the AP's dnsmasq (wildcard `0.0.0.0:67` socket swallows the broadcast). Static IP
  is the fix; don't re-debug.
* NM stores `forward-delay 0` but doesn't push it with STP off; the offloads script forces it.
* GNOME with no monitor advertises zero outputs, so SDL fails; Xvfb `:99` + x11vnc is the
  fallback.
* `nft` chain is named `guard` (`fwd` is reserved).
* Never move this tree over exFAT (silently mangles 672 filenames; every count/hash still passes).
* Zeek without `-C` silently drops every offloaded-checksum packet: `conn.log` present, app logs
  absent, exit 0. `-C` is the default in `zeek-logs.py`; the negative control asserts the trap.

## Next

- [ ] **Build the live console-capture lane** (the only irreplaceable artifact): consoles A/B
      online, SPAN/mirror or passive tap into `dumpcap`. The analysis half is ready; the
      operator's step-by-step is `historicaldocs/design/zeek-pipeline.md` §10.
- [ ] Fold the pad + press cadence into `run-signon-experiment.sh` so a gate-ladder run is one
      command (mechanism proven, packaging remains).
- [ ] Two-instance harness arming: blocked on the two REFUSED preflight items
      (`ControllerIsolationUnresolved`, `ActivationReviewMissing` - operator decisions). The
      third blocker fell 2026-08-11: `d1-harness pad run --vendor/--product` mints distinct pad
      identities, and `pad controls` proved two simultaneous pads enumerate as two distinct
      kernel devices (A 045e:028e, B 045e:028f). Still `[U]` before choosing `distinct-usb-ids`:
      whether SDL maps the non-x360 GUID - settle on the next boot with
      `d1-harness pad run --product 0x028F` up and shadPS4's controller log open.
- [ ] **Close the d1guest/d1-live live-parity gate**, first boot that reaches L3. With the client up:
      `python3 -B capture-rig/d1guest.py descriptors > /tmp/py.txt` then
      `d1-server/target/release/d1-live descriptors > /tmp/rs.txt`, and the same pair for
      `read profile_setup_kind`, `obj 4:0`, and `tag 0x80801825 --len 128`. Normalise only the
      `HH:MM:SS` stamp and `diff`. ⚠ Use `--no-cache` on both, and clear `/tmp/d1guest-base.json`
      and `/tmp/d1-live-base.json` first, or the anchor-scan line is compared against a cache.
- [ ] Port `d1guest.py poke` into `d1-live`, as its own reviewed change, keeping
      `--experiment`/`--confirm`, the taint banner and the `runs/guest-writes.jsonl` journal.
- [ ] Assert egress default-deny + fail-closed DNS pre-flight and audit post-run, every run.

Full 2026-08-09 narrative with every instrument's history: `historicaldocs/status/harness-2026-08-09.md`.
