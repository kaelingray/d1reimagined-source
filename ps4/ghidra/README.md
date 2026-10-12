# ghidra/ - the Ghidra estate

Ghidra is a *tool we run*, not code we ship: Apache-2.0, installed outside the repo at
`~/dev/ghidra_12.1.2_PUBLIC`, project at `~/dev/ghidra-projects/D1.gpr` (~2 GB, regenerable,
deliberately untracked). Nothing from it is linked into `d1-server`.

**The full bring-up procedure - fresh dump to a citable database - is
`design/ghidra-bringup.md`. Read it first.** ⚠ It opens with a currently-live defect in the
seeding defaults (literal `$HOME` from the 2026-08-10 scrub) that its section 4 works around.

⚠ **Ghidra outranks nothing.** Its output is `[I]`; `d1-eboot` is the authority for bytes,
relocations and xrefs, `d1-oracle`/`d1-replay` settle behaviour by executing the real code.
⛔ The Ghidra project and the decompiled corpus are derived from game code: **never shipped** -
regenerate them from your own dump.

## Entry point: `./ghidra/d1-ghidra.sh`

| subcommand | does | cost `[C]` 2026-08-08 |
|---|---|---|
| `build` | import + seed from the atlas + tuned auto-analysis + re-seed + verify | ~5 min 45 s, peaks ~20G RAM |
| `atlas` | (re)apply atlas names/labels; **run after `build`** (flow-repair drops ~231 starts) | 2 s warm |
| `verify` | 5 byte controls + image identity; **nothing may be cited if it fails** | ~4 s script time |
| `structs` | apply `structs/d1-structs.tsv`: 50 structs, `this` on 43 fns | ~15 s |
| `options` | print (or `tune`) the auto-analysis option set | seconds |
| `decompile OUTDIR ADDRS [THREADS] [TIMEOUT]` | bulk decompile to one `.c` per function + index | 500 fns @16 = 2 s; all 47,306 @24 = 191 s |
| `gui` | open the project in the GUI (needs a display) | - |

⚠ Only ONE process may hold the project; a second fails with a lock error (protection, not a bug).

## Tracked files

| file | what it is |
|---|---|
| `d1-ghidra.sh` | the one entry point; its header is the spec |
| `D1Atlas.java` | seeds the DB from `capture-rig/eboot-atlas.tsv` (+types, +imports): functions, strong names only (filter on `name_source`, never on `~`), plate-comment provenance on every row |
| `D1Verify.java` | the addressing positive control: 5 known-byte reads; throws on any mismatch |
| `D1AnalysisOptions.java` | prints/tunes analyzers: discovery off (incl. `Shared Return Calls`), interpretation on |
| `D1Structs.java` | applies `structs/d1-structs.tsv`; everything `SourceType.IMPORTED`; run after `atlas` |
| `D1Decompile.java` | bulk decompiler driver; writes `decompile-index.tsv` with per-function timing |
| `gen-structs.py` | derives `structs/d1-structs.tsv` from the decompiled corpus; 10 controls, exit 2 on failure, refuses to write |
| `structs/d1-structs.tsv` | 50 structs / 225 fields / 43 applies / 5 sigs; names human-authored, shape derived |
| `lists/all.txt` | every atlas function start (47,306) - the full-corpus decompile list |
| `lists/critical-path.txt` | the 3v3 path, every address cited from a doc (labels corrected 2026-08-08) |
| `lists/controls.txt` | 3 hand-understood functions to diff after a rebuild |
| `lists/bench500.txt` | 500 fns for throughput sizing; ⚠ sampled FROM the atlas, never a naming holdout |

Prerequisite chain: your own decrypted eboot (`dumps/patch-01.29-CUSA00219/eboot.bin`) and the
atlas TSVs from `python3 -B capture-rig/eboot-atlas.py` (`BOOTSTRAP.md` step 3b) come FIRST -
the Ghidra database is seeded from our atlas, not the other way round.
