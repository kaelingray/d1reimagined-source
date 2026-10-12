# Ghidra bring-up - from your own decrypted eboot to a citable database

**State: procedure documented 2026-08-11; `verify` and `options` re-run and PASSING that day;
the full `build` was last executed 2026-08-08, before the 2026-08-10 identifier scrub, which
left a known defect in the seeding defaults (section 4). Read section 4 before running `build`.**

This is the ordered path from "I have my own decrypted eboot" to "I have a loaded, seeded,
verified Ghidra database I am allowed to cite." The result state is described in
`design/instruments-static.md` section 6; this document is the *procedure*. The entry point for
everything is **`ghidra/d1-ghidra.sh`** - read its header, it is the spec, and this document
follows it.

⚠ **Before anything else: Ghidra outranks nothing.** Its output is `[I]`, a hypothesis, always.
Section 11 is the rule and the decision table; if you read only one section, read that one.

⛔ **Licensing and shipping, non-negotiable.** The Ghidra project (~2 GB) and the decompiled
corpus (248 MB) are derived from copyrighted game code and are **never shipped, committed, or
redistributed** - you regenerate both from your own dump. The generators ship; their output does
not. Ghidra itself is Apache-2.0 and lives *outside* the repo; nothing from it is linked into
`d1-server`, so the GPL-2.0-or-later position is untouched. Vendoring Apache-2.0 code into
`d1-server` would cost the "or later" option; *running* an external tool creates no derivative
(⇒ `design/licensing.md`).

---

## 1. Prerequisites

| need | detail |
|---|---|
| **Your own dump** | `dumps/patch-01.29-CUSA00219/eboot.bin`, decrypted, sha256 `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954` (pinned in `capture-rig/guest-symbols.json`, asserted by `d1-eboot controls`). `BOOTSTRAP.md` section 1 is the console-side runbook. There is no substitute input. |
| **The atlas TSVs** | The Ghidra seed comes FROM our atlas, not the other way round. `BOOTSTRAP.md` step 3b: build `d1-server` (for `d1-eboot`), then `python3 -B capture-rig/eboot-atlas.py`, which regenerates `capture-rig/eboot-atlas.tsv` (47,306 rows), `eboot-types.tsv` (2,633), `eboot-imports.tsv` (638) and the rest, and refuses to write unless `d1-eboot controls` and its own controls pass. |
| **JDK 21** | Ghidra 12.1.2 requires it. **Fedora's default JDK 25 is rejected by the launcher** `[C]` (measured 2026-08-08). `dnf install java-21-openjdk-devel`; the script defaults `JAVA_HOME=/usr/lib/jvm/java-21-openjdk` (present on the reference box, verified 2026-08-11). |
| **Memory** | `GHIDRA_HEADLESS_MAXMEM=48G` (the script exports it; stock Ghidra honours this env var, verified in `support/analyzeHeadless` 2026-08-11). The stock 2G default is not enough: seeding 47k functions peaks near 9G, auto-analysis near 20G `[C]` (measured 2026-08-08). |
| **Disk** | ~2 GB for the project at `~/dev/ghidra-projects/`, plus 248 MB for the decompiled corpus. Both are outside the repo on purpose: regenerable, and never committed. |

## 2. Install Ghidra and verify the download

Ghidra lives **outside the repo**, at `~/dev/ghidra_12.1.2_PUBLIC` (or set `GHIDRA_HOME`).

```sh
# from the NSA release page (github.com/NationalSecurityAgency/ghidra/releases), version 12.1.2
sha256sum ghidra_12.1.2_PUBLIC_*.zip
#   must print b62e81a0390618466c019c60d8c2f796ced2509c4c1aea4a37644a77272cf99d
unzip -d ~/dev ghidra_12.1.2_PUBLIC_*.zip
ls ~/dev/ghidra_12.1.2_PUBLIC/support/analyzeHeadless   # the binary the driver needs
```

A different zip hash means a different release body: stop, do not proceed on "probably fine".

Environment knobs `d1-ghidra.sh` reads (all have working defaults): `GHIDRA_HOME`,
`D1_GHIDRA_PROJ_DIR` (default `~/dev/ghidra-projects`), `D1_GHIDRA_PROJ_NAME` (default `D1`),
`JAVA_HOME`, `GHIDRA_HEADLESS_MAXMEM`.

## 3. The addressing contract - read before trusting any address

The eboot is `e_type = 0xFE10` (`ET_SCE_DYNEXEC`). Stock Ghidra has **no SCE ELF support**, yet
loads it off `e_machine` plus the program headers anyway. Result `[C]`: `segment_0`
`0x0..0x18bf0db` r-x, `segment_1` `0x18c0000..0x2f50dbf` rw-, `imageBase = 0`.
**Ghidra addresses are our addresses, one-to-one, no rebasing.** Three caveats, each of which has
cost time elsewhere:

1. **The file/vaddr bias is per segment.** `0x4000` is a coincidence of the two LOAD segments in
   this image, not a property of the format - never hardcode it
   (`design/instruments-static.md` section 2.3).
2. **Ghidra knows nothing about relocations, dynamic symbols or NIDs.** It reports the dynamic
   table "stripped"; it is not - `PT_DYNAMIC` has `p_vaddr = 0`, so Ghidra cannot map it.
   `d1-eboot reloc` is the only authority on the 70,682 relocations.
3. **BSS reads as zeros rather than refusing.** Zero-from-Ghidra in BSS means "no file bytes",
   NEVER "the runtime value is 0". Read live values with `capture-rig/d1guest.py read`.

## 4. ⛔ Known defect first: the scrubbed `$HOME` literals (current at HEAD)

The 2026-08-10 identifier scrub replaced absolute home paths with `$HOME` across the tree. In
shell that expands; **in Java and in a Python string literal it does not**. Confirmed 2026-08-11:

* `ghidra/D1Atlas.java:57` and `ghidra/D1Structs.java:73` - `REPO = "$HOME/dev/destiny1-preservation"`,
  a dead literal. Their *default* TSV paths therefore resolve to nothing.
* `ghidra/gen-structs.py:49` - `CORPUS = pathlib.Path("$HOME/dev/ghidra-projects/decompiled")`,
  same defect. `[C]` `python3 -B ghidra/gen-structs.py --controls` exits 2 today with 3 failures
  ("every handler vfunc has a decompiled corpus file" and downstream), purely from this path.
* `capture-rig/eboot-atlas.py` `PS4_NAMES` - same literal, but that one degrades **silently**:
  a post-scrub atlas regeneration names 366/638 imports instead of 552/638, never wrongly.

Consequences and workarounds, per subcommand:

* **`atlas` and `structs` are fine if you pass the paths explicitly** - both scripts take them as
  positional args and `d1-ghidra.sh` forwards `"$@"`:

  ```sh
  ./ghidra/d1-ghidra.sh atlas "$PWD/capture-rig/eboot-atlas.tsv" \
      "$PWD/capture-rig/eboot-types.tsv" "$PWD/capture-rig/eboot-imports.tsv"
  ./ghidra/d1-ghidra.sh structs "$PWD/ghidra/structs/d1-structs.tsv"
  ```

* **`build` bakes in no args** for its two internal seed steps, so a from-scratch `build` at
  current HEAD should abort at "seed from atlas" when `D1Atlas` cannot open the default path.
  `[I]` from reading the code; not executed (a build costs ~6 minutes and the project lock).
* **`gen-structs.py` has no CLI override for the corpus path** (only `--controls` and `--out`);
  it is broken until the one-line fix lands.

The real fix is one line in each file and belongs to a wiring pass, not to this document. Until
it lands, the only way to run stock `build` or `gen-structs.py` unmodified is a literal-named
symlink shim from your invocation directory (run from the repo root):

```sh
mkdir -p '$HOME/dev'                     # a directory literally named $HOME - yes, really
ln -sfn "$(pwd)" '$HOME/dev/destiny1-preservation'
ln -sfn ~/dev/ghidra-projects '$HOME/dev/ghidra-projects'
# ... run build / gen-structs.py from this directory ...
rm -rf '$HOME'                           # remove it afterwards; never commit it
```

`[U]` The shim and a post-scrub `build` are both **untested end to end**. What would close it:
apply the one-line fixes, run `./ghidra/d1-ghidra.sh build`, and watch the "seed from atlas"
phase print its row counts (D1Atlas laps every phase).

## 5. `build` - import, seed, analyse, re-seed, verify (~6 min)

```sh
./ghidra/d1-ghidra.sh build
```

Five phases, straight from the script:

1. **Import, `-noanalysis -overwrite`** - the eboot as Ghidra sees it, no guessing yet.
2. **Seed from the atlas** (`D1Atlas.java`) - disassemble at every atlas start, create 47k
   functions, name the 638 PLT stubs, apply strong names, label 2,633 typeinfos and 1,861
   vtables, mark the entry `0xC2B470` (`_ELF_entry`). Details in section 6.
3. **Auto-analyse**, `-max-cpu 24 -analysisTimeoutPerFile 10800`, with
   `D1AnalysisOptions.java tune` as a preScript. The tuning is the point: we hand Ghidra 47k
   function starts, so *discovery* analyzers are pure cost and are turned off - including
   **`Shared Return Calls`**, which rewrites tail-`jmp`-entered functions into thunks and would
   silently move atlas boundaries. *Interpretation* analyzers (Stack, Decompiler Switch
   Analysis, Non-Returning, Reference, Demangler GNU, x86 Constant Reference) stay on.
   Confirmed live in the database 2026-08-11 via `./ghidra/d1-ghidra.sh options`.
4. **Re-seed** - see section 6 for why this is not optional.
5. **`verify`** - section 7. `build` ends by running it; so should every session.

Timings `[C]` (measured 2026-08-08): whole rebuild ~5 min 45 s; memory peaks ~9G (seed) and
~20G (analysis). Loaded result `[C]`: 48,040 functions, 9,749 strongly-sourced names, 638 named
PS4 import stubs, 2,633 RTTI typeinfo labels, 1,861 vtable labels, 237,615 symbols.

⚠ **Only ONE process may hold the project.** A second invocation fails with a lock error - that
is Ghidra protecting the database, not a bug. Check for a concurrent session before any
subcommand, including `verify`.

## 6. `atlas` - the seed, and why it runs AFTER `build` too

```sh
./ghidra/d1-ghidra.sh atlas   # plus the three explicit TSV paths until section 4's fix lands
```

⚠ **Run `atlas` again after `build`**: auto-analysis flow-repair removes ~231 atlas function
starts, and re-running restores them. It costs 2 seconds (46 s cold, 1.5-2 s warm `[C]`
2026-08-08) and is idempotent - safe to re-run any time.

What makes the seed trustworthy rather than merely convenient:

* **Name provenance is filtered.** Only `name_source` classes that are *identities* are applied
  as names: `rtti`, `rtti-vptr`, `source-file`, `schema-tag`. Weak classes (`string-ref`,
  `import`, `name-hash`) are skipped. ⚠ Filter on `name_source`, never on the `~` prefix - weak
  and some strong rows are all `~`-prefixed.
  ⛔ Pending correction (`design/instruments-static.md` section 4, S1): `schema-tag` measured
  **0/495** as an identity and is queued to be dropped from the default strong set at
  `D1Atlas.java:62`; as of this writing it is still there. The strong set is overridable as the
  4th positional arg (comma-separated), e.g. `rtti,rtti-vptr,source-file`.
* **Everything the script writes is `SourceType.IMPORTED`**; Ghidra's own guesses stay
  `ANALYSIS`. Our claims and Ghidra's never merge, and the GUI can always tell them apart.
* **Duplicate names get an address suffix.** 9,749 strong names cover only 6,761 distinct
  strings (549 functions share one ctor/dtor name); a name used more than once gets `_<vaddr>`
  appended so decompiled call sites stay navigable.
* **Every atlas row gets a plate comment** - name, source, class, vslot, subsystem, tags,
  strings, warnings - and weak rows carry an explicit "name_source is WEAK" line. Provenance is
  kept visible even where identity is not claimed.

## 7. `verify` - the gate on citing anything

```sh
./ghidra/d1-ghidra.sh verify   # ~4 s of script time; JVM startup dominates the wall clock
```

`D1Verify.java` prints the image identity (language, imageBase, executable sha256, entry point),
the memory map, **5 byte controls** - addresses whose bytes are known independently, straight
from the file - and the function/data/symbol counts. If a single byte disagrees it **throws**,
and the rule is absolute: **nothing in the database may be cited until `verify` passes.** A byte
control failure means the load itself is misconfigured (wrong base, wrong file, wrong SKU), not
that one address is off.

Run 2026-08-11 on the reference database `[C]`: all 5 byte controls PASS; executable sha256
matches the pin; 48,040 functions, 78,170 defined data, 237,680 symbols. (The symbol count
drifts upward with GUI use; the byte controls and the sha256 are the invariants, the counts are
a health indicator.)

**Session ritual: `verify` first, every session, before citing.** It is also the cheapest way to
prove the project is not locked by someone else.

## 8. `structs` - deriving shape, authoring only names

```sh
python3 -B ghidra/gen-structs.py --controls   # 10 offline checks, exit 2 on failure
python3 -B ghidra/gen-structs.py              # regenerate ghidra/structs/d1-structs.tsv
./ghidra/d1-ghidra.sh structs                 # apply to the database (~15 s)
```

⛔ Currently blocked by the section 4 corpus-path defect - `--controls` exits 2 today.

The TSV (330 lines: 50 structs, 225 fields, 43 `this` applications, 5 signatures) turns
`*(long *)(param_1 + 0x38)` into `this->debounce_ms`. The rule the generator enforces: **a
struct is derived, never transcribed** - field offsets and widths come from what the class's own
vfuncs actually touch in the decompiled corpus; only the *names* are human-authored, and a
curated name at an offset the corpus never touches is a control failure. `this` is typed only
where `vt_shared == 1`. A struct applied to the wrong type makes decompilation worse, silently;
the controls exist because that failure mode gives no error.

Order matters, both ways:

* **`atlas` before `structs`** - D1Atlas rewrites plate comments; D1Structs appends to them.
* **`decompile` after `structs`** - the corpus on disk still shows the untyped form until
  refreshed.
* ⚠ **The self-reference trap**: the generator reads the corpus its own output rewrites. After
  `structs` + a corpus refresh, a naive rescan finds `this->field`, not `param_1 + N`, and
  proposes deleting every field it already derived. The generator reads both forms through its
  previous generation, and a shrink control refuses to lose offsets. ⛔ That shrink control is
  **vacuous when no previous TSV exists** (`design/instruments-static.md` section 4, S2 - fix
  queued): on a truly fresh clone, treat the first generation with suspicion.

## 9. `decompile` - the corpus

```sh
./ghidra/d1-ghidra.sh decompile OUTDIR ADDRS [THREADS] [TIMEOUT]
```

`ADDRS` is either a comma-separated hex list (`0x34c010,0x260200`) or a file with one address
per line (`#` comments allowed). Defaults: 8 threads, 120 s per function. Mid-function
addresses resolve to their containing function. Output: one `.c` per function (header carries
entry, body range, decompile ms) plus `decompile-index.tsv` with per-function timing. The run is
`-readOnly` - it never mutates the database.

Throughput `[C]` (measured 2026-08-08): 500 functions @16 threads = 2 s; **all 47,306 @24
threads = 191 s**, 47,303 succeeded (the 3 failures are a Ghidra internal on Havok vfuncs, none
on a critical path). Decompilation is free relative to reading it; the bottleneck is
verification, and that is `d1-oracle`'s job. Reference corpus: 47,304 `.c` files, 248 MB, at
`~/dev/ghidra-projects/decompiled/`.

The ready-made lists in `ghidra/lists/`:

| list | rows | use it for |
|---|---|---|
| `all.txt` | 47,306 | the full corpus rebuild after any `atlas`/`structs` run |
| `critical-path.txt` | 97 lines | the 3v3-private-match path, every address cited from a doc. ⛔ Its rung labels were shifted by one until 2026-08-08; fixed in place - the header explains the rule (the requester of rung N is the handler of rung N-1) |
| `controls.txt` | 3 addrs | the hand-understood functions to diff against after a rebuild (distinct from D1Verify's 5 *byte* controls) |
| `bench500.txt` | 500 | throughput measurement. ⚠ Sampled FROM the atlas - never use it as a naming holdout; the filename invites the tautology |

⚠ **The corpus is a snapshot.** Refresh after any `atlas` or `structs` run, or you grep stale
names. ⛔ **The corpus leak**: headers and filenames embed the atlas label
(`design/instruments-static.md` section 4.5, G3) - strip both before using the corpus to score
any naming method.

## 10. `gui`

```sh
./ghidra/d1-ghidra.sh gui
```

Opens the project in the Ghidra GUI; needs a display. ⛔ On the reference box, displays
`:0 :1 :99 :1003` belong to the capture rig - do not steal them. The one-process lock applies:
close the GUI before any headless subcommand, and vice versa.

## 11. Where Ghidra sits in the evidence hierarchy

**Ghidra outranks nothing.** Every reading out of it is `[I]` `static-client-inference` - a
hypothesis to be settled by an instrument with authority. This is the single most important
thing on this page. The failure shape it prevents: decompiled C *looks* like source, so a
reading out of it gets written down as fact and then re-cited; three specific blind spots make
that dangerous here - BSS reads as zeros (section 3), the 70,682 relocations are invisible
(section 3), and **indirect calls are invisible to Ghidra's callgraph** (the command-queue pump
at `0x34C010` calls `handlers[slot]` and Ghidra knows no target).

| question | instrument | why |
|---|---|---|
| what bytes/instructions are at address X | `d1-eboot segments` / `disasm` | the authority for file bytes; no BSS-as-zeros lie |
| who references address X | `d1-eboot xref` | Ghidra misses relocation- and indirection-borne references |
| who touches `object + offset` | `d1-eboot field-xref` | a different question from xref; the wrong tool answers a different question cleanly |
| relocations, dynamic symbols, NIDs | `d1-eboot reloc` | Ghidra cannot map `PT_DYNAMIC` at all |
| what is this function called | `capture-rig/eboot-atlas.py --addr 0xNNN` | the atlas is the naming authority; Ghidra was *seeded from it* |
| what does this function do (hypothesis) | Ghidra corpus + GUI | its one job: readable semantics, fast |
| does the code actually behave that way | `d1-oracle` / `d1-replay` | they *execute* the real code; behaviour outranks reading |
| which branch did the live client take | `d1-live watch`, `runtime-probe` skill | runtime state exists nowhere in the file |

Session ritual: `verify` first; grep the corpus; read the plate comment for provenance
(`FUN_00xxxxxx` means the atlas had nothing strong there); treat every reading as `[I]` until
`d1-eboot` bytes or a `d1-oracle` execution settles it. Promotion to `[C]` comes from the
settling instrument, never from a second reading of the same decompilation.

## 12. Regenerating from scratch - the whole ladder, with timings

Assuming the section 4 fixes have landed (or the shim is in place):

| step | command | time `[C]` 2026-08-08 |
|---|---|---|
| 0 | dump + `cargo build --release` + `BOOTSTRAP.md` steps 1-3a | hours (console) + minutes |
| 1 | `python3 -B capture-rig/eboot-atlas.py` | minutes |
| 2 | `./ghidra/d1-ghidra.sh build` | ~5 min 45 s |
| 3 | `./ghidra/d1-ghidra.sh atlas` (restores ~231 starts) | 2 s warm |
| 4 | `./ghidra/d1-ghidra.sh verify` (must PASS before anything is cited) | ~4 s script time |
| 5 | `python3 -B ghidra/gen-structs.py && ./ghidra/d1-ghidra.sh structs` | ~15 s |
| 6 | `./ghidra/d1-ghidra.sh decompile ~/dev/ghidra-projects/decompiled ghidra/lists/all.txt 24` | 191 s |
| 7 | `./ghidra/d1-ghidra.sh verify` again; diff `ghidra/lists/controls.txt` decompilations against the documented readings | ~4 s + reading |

Total, dump to citable corpus: well under 15 minutes of machine time after step 0.

## 13. Traps, in one table

| trap | consequence | rule |
|---|---|---|
| JDK 25 on PATH | launcher rejects it | JDK 21; the script sets `JAVA_HOME` |
| stock 2G heap | OOM mid-seed or mid-analysis | `GHIDRA_HEADLESS_MAXMEM=48G` (script default) |
| second process on the project | lock error | one holder at a time; that error is protection, not breakage |
| skipping the post-`build` `atlas` | ~231 functions silently missing | re-run `atlas`; 2 s |
| literal `$HOME` in the Java/Python defaults | seed/structs abort; `gen-structs.py` controls exit 2 | section 4; pass paths explicitly until fixed |
| post-scrub atlas regen without `ps4_names.txt` | 366 instead of 552 named imports, silently | section 4; the names are fewer, never wrong |
| zero read from BSS | "the value is 0" fabricated | zero in BSS means "no file bytes"; read live |
| trusting Ghidra xrefs/callgraph | indirection- and relocation-borne references missing | `d1-eboot xref` / `field-xref` / `reloc` |
| filtering atlas names on `~` | weak names applied as identities | filter on `name_source` only |
| hardcoding the `0x4000` bias | wrong file offsets on non-LOAD segments | the bias is per segment |
| stale corpus after `atlas`/`structs` | grepping names that no longer exist | refresh the corpus; it is 191 s |
| citing without `verify` | possibly citing a misloaded database | `verify` first, every session |

## 14. Open `[U]`s

1. `[U]` **Has `build` run end to end since the 2026-08-10 scrub?** No. Close: land the one-line
   path fixes, run `build`, confirm the "seed from atlas" phase prints its row counts and the
   final `verify` passes.
2. `[U]` **Does a `D1Atlas` exception abort `build`?** `set -euo pipefail` protects only if
   `analyzeHeadless` exits non-zero on a postScript exception; `D1Verify`'s header says it does
   (`[R]`), but it has not been demonstrated for the seed phase. Close: point `atlas` at a
   missing TSV and check `$?`. If it does NOT abort, a failed seed would flow into auto-analysis
   of an unseeded database - a silent, expensive failure.
3. `[U]` **`gen-structs.py` on a truly fresh clone**: the shrink control is vacuous with no
   previous TSV (section 8; queued fix S2). Close: the queued fix, then a fresh-clone dry run.
4. `[U]` **The `schema-tag` drop (S1)**: queued but not landed; the 9,749-name count will change
   when it does. Close: land it, re-run `atlas`, restate the counts here and in
   `design/instruments-static.md` section 6.
5. `[U]` **Bring-up on a non-reference box** (different JDK path, different core count) has
   never been exercised; every timing above is from the reference box.
