# The static and offline analysis instruments - canonical reference

**State (2026-08-11):** all four instrument families are built and controlled. `d1-eboot` is the
byte/xref/descriptor authority (gate: `d1-eboot controls`, exit 2 = do not cite; 27 checks through
2026-08-10, **31** after the four containment controls of §2.4). The generated atlas names
**14,676 of 35,834 prologue-anchored functions (41.0%)**. Ghidra 12.1.2 holds the whole binary
decompiled to disk. `d1-replay` executes client code on captured snapshots, 20/20 controls.
The naming workflow's verdict stands: **net new names clearing the ship threshold is ZERO** and the
honest coverage ceiling of the current extractor pipeline is **43.66%**.

Melted from seven source docs (see Sources). Live siblings: `design/instruments-live.md` (live-client
instruments), `design/eboot-map.md` (the address book, stays live), `design/diagnostic-catalogue.md`
(generated, stays live), `design/observability-and-ui.md` (traces/dashboards).

---

## Contents

1. [The access-mode table - pick the instrument by the question](#1-the-access-mode-table)
2. [The four-layer plan and the constraints that must survive (ATLAS)](#2-the-four-layer-plan-and-its-constraints)
3. [`d1-eboot` and the generated atlas - the binary map](#3-d1-eboot-and-the-generated-atlas)
4. [The naming-technique ship/no-ship record and the honest ceiling](#4-naming-techniques---shipno-ship-and-the-honest-ceiling)
5. [The atlas coverage roadmap - open work](#5-the-atlas-coverage-roadmap---open-work)
6. [Ghidra - the semantics instrument](#6-ghidra---the-semantics-instrument)
7. [`d1-eboot field-xref` - who touches `object + offset`](#7-d1-eboot-field-xref)
8. [`d1-replay` - the live-state replay oracle](#8-d1-replay---the-live-state-replay-oracle)
9. [Dead claims](#9-dead-claims)
10. [Sources](#10-sources)

---

## 1. The access-mode table

⭐⭐ **State the ACCESS MODE first**: a passing control proves the *instrument*, never the
*technique*, and picking the wrong row gives you a clean scan of the wrong question
(`status/LESSONS.md`, §2.5 below). Reproduced faithfully from `status/MAP.md`:

| instrument | sees | blind to |
|---|---|---|
| `d1-eboot xref` (static) | who references an **ADDRESS** - `call`/`jmp`/riprel/reloc, 4 controls | ⭐ **register-relative field access** (`mov rax,[rdi+8]` contains no address) - **by construction**; and every runtime value |
| ⭐ `d1-eboot field-xref` (static) | who READS/WRITES `object + offset`, incl. **`disp == 0`** and the callees a field's **address escapes into**; 5 controls, one inverted | the runtime instant; a base register it cannot prove (those rows are `[U]`, hidden unless `--all`); branch-dependent aliases |
| `d1-oracle` (Unicorn, offline) | code executing on inputs we supply | real runtime state |
| `capture-rig/d1guest.py` (live) | values in **memory**, right now - a *state* | registers; any chosen instant; **the write itself** |
| ⭐ `d1-live watch` (live, `DR0`/`DR7`) | ⭐ **the `RIP` of the store, on the first hit** - the *event*, with no candidate list; 4 controls | threads created after arming (unless `--follow-clone`); kernel/DMA writes; 4 slots only |
| `d1-replay` (snapshot + Unicorn) | the client's own code **executing on its real memory** | registers at the original moment - it supplies them, and searches; **a transition, which is an event not a state** |

⛔ **A third blind shape, added 2026-08-10** (`historicaldocs/design/biped-census.md` §9.6): an **indexed element of
a heap pool** (`*(base+8) + i*stride + N`) is invisible to `xref` **and** to `field-xref`.

The live rows (`d1guest.py`, `d1-live watch`) are documented in `design/instruments-live.md`; they
appear here because the table is only useful whole.

---

## 2. The four-layer plan and its constraints

**Plan of record; Layers 1 and 4 built** (drafted 2026-08-07; built 2026-08-08).
⇒ historicaldocs/design/ATLAS.md. The sentence the plan exists to enforce:

> The problem is not that we lack capability. It is that the knowledge lives in script constants,
> inline comments, one-off scratch tools, runtime traces, Rust implementations and partially
> superseded hypotheses - which is why the same structures have been interpreted several times and
> why clean-looking false readings survived until a second tool contradicted them.

### 2.1 What we have, and what the game can still yield

`[R]` (research lane, 2026-08-07, read-only inspection of both disc dumps): the build was Demonware
45 static archive linked into `eboot.bin` and stripped. **We have the cooked library code; we do not
have the library container or its symbol metadata.** No Demonware `.a`/`.prx`/`.sprx`/`.elf` exists
in either dump. The 01.00 eboot carries 112 unique `bd*` identifiers; 01.29 carries 147 `[R]`. The
linker retained source paths (Demonware's log/assert macros embed `__FILE__`) but discarded symbol
names. ⚠ `[R]` counts have not been reproduced by the execution lane; §2.6's rule (no pinned count
without a committed extractor) applies before any becomes a control.

Reachable from the eboot plus content: every reachable RSAT schema; runtime type hashes and the
**two** name-hash registries - A at `0x19B3530` (6,338 entries) and B at `0x19D0BD0` (1,332),
record `{u64 ptr, u32 hash, u32 pad}` `[C]`. ⛔ **The registry hash function is NOT recoverable from
the binary - proven, not assumed**: 118M trials plus a ~10^8 substring sweep at the noise floor,
with injection *and* scramble controls, and a sweep of all 6.9M instructions finding only two
string-hash routines (FNV-1 with `tolower`/path normalisation, zlib CRC-32), neither producing these
values. Do not re-attempt. Also reachable: message tables, codec/policy bindings, vtables/RTTI/call
graph, `__FILE__` attribution, bootflow states, config defaults, crypto and endpoints, native
canonical encodings by executing the client's own codecs offline, and which paths actually execute.

What we cannot get without an external symbol source: most original method spellings, and the
**runtime type-hash registry** names. ⚠ The original "type names proven unrecoverable" sentence was
over-broad - it is true of the runtime hash registries and **false of C++ class names**: RTTI is
intact and yields **2,633 named classes**, 2,429 vtables, 22,914 slots (§3.2).

### 2.2 The four layers

- **Layer 0** - finish the current gate first (the message-206 span oracle, seven acceptance tests).
- **Layer 1** - `d1-index`: one queryable analysis index. Built as the `d1-eboot` crate (image,
  relocations, disassembly, xrefs, descriptors, dynsym, rtti, functions, strings, nametab, atlas).
  The database is an analysis index, **not** a second wire-format implementation; the wire contract
  stays in Rust per one-protocol-one-implementer.
- **Layer 2** - `d1-oracle`: the generalised Unicorn harness (covered by
  `.claude/skills/native-oracle/SKILL.md`). Pinned controls: the 503 vector, the character RSAT
  vector, message-206 empty and non-empty span, known Oodle blocks.
- **Layer 3** - `d1-probe`: data-driven runtime instrumentation (live side; constraints in §2.5).
- **Layer 4** - `d1-atlas`: the PS4 function and class map. Built as `capture-rig/eboot-atlas.py`
  (§3).

### 2.3 The address model `[C]`

**The address bias is per-segment.** `BIAS = 0x4000` is correct for the two LOAD segments only - a
coincidence of this image, not a property of the format:

```
type 0x1        (LOAD)        file 0x00004000  vaddr 0x00000000  delta 0x4000
type 0x1        (LOAD)        file 0x018c4000  vaddr 0x018c0000  delta 0x4000
type 0x2        (DYNAMIC)     file 0x01bca750  vaddr 0x00000000  delta 0x1bca750
type 0x61000000 (dynlibdata)  file 0x01a0d100  vaddr 0x00000000  delta 0x1a0d100
type 0x6fffff00               file 0x01bcb4c0  vaddr 0x00000000  delta 0x1bcb4c0
type 0x6fffff01               file 0x01bcb530  vaddr 0x00000000  delta 0x1bcb530
```

The metadata segments have `p_vaddr = 0` and are addressed by **raw file offset** (`RELA` is read at
dynlibdata file offset `0x1A0D100 + 0xBDD0` by hand for exactly this reason). Anyone unifying the
bias into one constant silently corrupts every relocation read. PS4 eboots store pointer tables as
**zero in the file**, filled at load - a raw byte search for an address finds nothing by design.

### 2.4 The scan and attribution constraints - each cost real time, all `[C]`

- **§4.2 A scan with no positive control is not evidence of absence.** Every reference sweep must
  cover `call rel32` (`e8`), `jmp rel32` (`e9`), `lea r64,[rip+disp32]`, immediates where
  applicable, and RELA addends; every "not found" must print a control that *does* find something
  in the same run.
  **`0x560D27 -> 0x56FFB0` is the mandatory positive control for tail-call recognition** (the §26.9
  producer was entered by a `jmp`; two sessions lost).
- **§4.2b A passing control does not mean you asked about the right address.** The `0x162E2C0`
  string scan returned 0 references with the control passing - the literal actually starts at
  **`0x162E2B5`** and nothing references a string interior. The control proves the instrument, never
  the target: verify the target independently before trusting any negative.
- **§4.2c A displacement scan is structurally blind to `disp == 0`.** The QueueZ state-1 writer was
  `0x56A8A7 mov byte [r14], 2` with the `+0x28` folded into the base by `lea`; three passes with
  passing controls missed it. ⭐ Closed 2026-08-09 by `field-xref`'s offset-carrying tracker (§7),
  pinned as its control 2 (`0x33ffa2`, `mov rax,[rbx]`).
- **§4.2d ⭐ The prologue count is NOT the function count.** 35,834 is the count of `55 48 89 e5`;
  **11,472 direct `call rel32` targets are not prologues at all** (frame-pointer-omitted). A
  prologue-only index answers "which function contains this address" with **the previous function**,
  confidently and wrongly. The four most-cited FPO addresses: `0x56CA70` (the registry `find`, the
  hot leaf never to probe), `0xA8C920` (the enum name-table registrar, 172 call sites), `0x260C10`
  (the accessor that killed dead-list theory #1), `0x1DF0` (the `xor eax,eax; ret` thunk).
  ⚠ Do **not** admit `jmp rel32` targets as function starts - a `jmp` target is frequently a basic
  block inside the same function, and admitting them shreds real functions into fragments.
- **§4.2d.i ⭐⭐ Knowing it was not enough - `xref` shipped the defect anyway (2026-08-11).**
  `d1-eboot xref`'s `fn` column was `nearest_prologue(site)` for three days after §4.2d existed: it
  printed `fn 0x1d1e80` for a site at `0x1d20a1`, though `FUN_001D1E80` ends at `0x1D2020` and the
  site is inside `0x1D2030`. `0x1D1E80` **writes** a pool and `0x1D2030` **reads** it, so the reads
  filed under the writer made the pool look write-only and a session published "this dispatch is
  unreachable dead code" - the chain was live. Fixed by making the answer singular: the two-pass
  index is cached per `Image` (`Image::functions()` / `Image::containing_function()`) and every
  reader asks it. Carry these: **"nearest prologue at or before" is a DECODE ANCHOR, never an
  attribution**; **a wrong name is worse than an honest unknown** (`functions::Containment` has four
  answers, two of them failures: `Unproven` prints `fn ?0x...`, `None` prints `fn -`); the end-blind
  `Index::function_of` named `0x129f850` for a *data* address at `0x25c0c58` - prefer
  `Index::containing`. Measured blast radius: **8 re-attributions**, all `prologue -> FPO`, over
  25,930 confirmed sites; zero change to counts, site sets, instruction text or the
  `callgraph`/`thiscalls`/`atlas`/`nametab` artifacts. Pinned as **four controls in `d1-eboot
  controls` (now 31)**, each with an inverted half proven to fail by re-injecting the defect.
- **§4.2e ⭐⭐ State the ACCESS MODE before choosing the scanner (2026-08-09).** An absolute-address
  xref answers "who touches this ADDRESS"; a register-relative field access is invisible to it **by
  construction**, so a clean scan with passing controls still answers the wrong question. Worked
  example: "who writes `c_activity_loader+8` (`DAT_0267AF08`)" asked as `xref 0x267af10` gave two
  references, no writer, all four controls passing; `field-xref --class c_activity_loader --global
  0x267af08 --offset 8` returned **24 tracked accesses over 67 candidates in one command** (§7).
  Two corollaries, both paid for: (1) a field-level scan answers "who touches `object+N`", not "who
  changes what the field means" - `+8` is a heap handle, so *zero stores* was correct and the
  finding was in the **escape** rows, which named the allocator and redirected the search one
  indirection down to `FUN_00341500 @ 0x341798`, displacement **0**; (2) ⭐ controls prove the
  instrument, never the technique - before running a scan, say what the target instruction looks
  like in **bytes** and check the matching rule could match them.
- **§4.3 Superseded readings survive in other tools' constants.** A retraction that is only
  recorded does not stop the next tool re-asserting the old model (the name-hash framing published
  8 bytes low sent a fresh scanner to `base+8`, which found zero references and concluded the
  registries were statically unreachable; root cause was a decimal-to-hex mis-conversion,
  `42979504` written `0x2900BF0` instead of `0x28FD0B0`). ⇒ re-derive a constant rather than trust
  a document's copy; print and serialize addresses in hexadecimal at derivation time, never
  mixed-base. A retraction must identify every dependent assertion and tool constant - an
  annotation that leaves the old assertion executable is incomplete
  (historicaldocs/ingress/round3/process/METHOD-RULES.md, rules 3 and 12).
- **§4.5 Misaligned disassembly.** Twice this project disassembled from a guessed address and
  produced authoritative-looking garbage. Always locate the real prologue first; a
  frame-pointer-omitted function is anchored by *being a call target*, which is evidence.
- **§5.1 ⛔ `__FILE__` bracketing does NOT work in this image - measured.** Over the 462 functions
  referencing a `__FILE__` literal: one Havok file's references span **24,455 function indices**
  and **124 of 125** file spans overlap another's. The linker did not keep object-file code
  contiguous. Range inference would have produced thousands of confident wrong attributions; the
  atlas `source_file` column records **only direct references (355 functions, exact)** and infers
  nothing.
- **§4.7 A workflow agent that "failed" may have completed.** Read `agent-*.jsonl` before
  re-running; a schema-rejected `StructuredOutput` discarded 29 minutes of confirmed findings once.

### 2.5 Probe-design constraints (Layer 3; live side detailed in `design/instruments-live.md`)

Kept here because they are ATLAS constraints and several bind offline tooling too, all `[C]`:

- **§4.4** `once: true` is not sufficient - one-shot catches the first occurrence, usually the
  wrong one (class `0x4C`'s empty span vs the crashing `0x3B`). Probes must re-arm **each other**,
  never themselves (anchor `0x5700E3`); bound with a record budget; detect probe/patch overlap
  statically (`CUSA00219-empty-span.xml` over `0x570A84..0x570A8B` cost two runs); zero records is
  a failure when a probe was expected to fire; check the `armed: N` line before trusting "never
  fired"; compare sequence numbers before comparing two samples.
- **§4.4b** The re-arm PUMP must sit on an unconditional path (re-anchored from `0x583450` to
  `0x586430`); if the pump can fail for the same reason the subject can, a dead trace is
  unfalsifiable.
- **§4.4c** Never probe a hot, multi-threaded engine leaf (`0x56CA70`): UD2 arming is
  cross-modifying code; probe the caller on the game tick.
- **§4.4d ⭐ A new tool is not evidence until it reproduces a KNOWN result.** `ready-byte-report.py`
  read `probe_id` at the `record_size` index and printed probe id 232 (= `sizeof(TraceRecord)`) as
  `NEVER FIRED` for all six links across three runs - output indistinguishable from the failure it
  was built to detect. Two runs lost on a phantom. A tool that has never printed a known-correct
  answer has not been tested; it has only been run.
- **§4.4e** The run's ENVIRONMENT is part of the experiment - a missing `D1_MANIFEST_FILE` and the
  default-ON `D1_PUSH_CHARACTER` both changed results while reading as client regressions; launch
  through `run-boot.sh`, which pins and preflights everything.
- **§4.6b** Never read live guest memory except while a known object is verifiably live - registry
  objects are cleared on teardown while descriptors survive, which produced two confident wrong
  readings in one session; `inject-roster.py`'s poll-until-SOID interlock is the pattern.

### 2.6 Evidence, counts and eliminations

Every component records confidence (`[C]`/`[O]`/`[R]`/`[I]`/`[U]`), reach (`observed-live` /
`observed-capture` / `replayed-capture` / `static-client-inference` / `synthetic-minimal` /
`synthetic-generic` / `contradicted`), source build, source address, positive control, supporting
artifact, and `supersedes`/`retracted-by`. **A fact must remain representable after retraction.**

- **Do not pin a count without a committed extractor.** `4,555` anchored descriptors and the two
  name-hash registry counts (`6,338` + `1,332`) qualify; pin the *registrar-derived* counts
  (`0xC262D0` loads `0x18C2`; `0xC2B2B0` loads `0x534`), never counts found by walking until the
  data stops looking right. A wrong pinned control is worse than none.
- **Eliminations are first-class data.** Refuted results are stored with their controls, or someone
  re-runs the experiment in three weeks.
- **Acceptance discipline:** "doesn't crash" is not an acceptance criterion and neither is "the
  predicate changed" - three client patches each removed the `0x570AAE` crash and each produced a
  byte-identical error loop, one silently removing investment signon. Name the required positive
  state transition and the forbidden error outcome up front
  (historicaldocs/ingress/round3/process/METHOD-RULES.md, rule 16).

What not to build: no new hand-written Python BAP implementation, no second RSAT/schema description,
no new tracer architecture, no general-purpose decompiler, no wholesale rewrite of working scripts,
no refactor during an active experiment.

### 2.7 Diagnostic-string attribution - method rules recovered from the round-3 ingress

Salvaged 2026-08-11 from the research lane's method notes
(historicaldocs/ingress/round3/process/METHOD-RULES.md rules 8-10;
historicaldocs/ingress/round3/process/METHOD-ADDITIONS.md rule 2). The sources state no confidence
tags, and the execution lane has not re-verified the `bdLog` register claim - treat the mechanism
as the research lane's report until a `d1-eboot disasm` check at a real `bdLog` call site pins it.
These extend §3.2's demotion of `string-ref` to topic-only with the mechanism behind it:

- **A log message about `Class::method` does not name the body emitting it.** At `bdLog` call
  sites, `r8` is `__FUNCTION__` and the format string is prose - attribute by the `__FUNCTION__`
  argument, never by the message text.
- **`__FUNCTION__` is unqualified.** Publish attribution as `(file, name, extent)`, never a map
  keyed by bare name - unqualified spellings collide across classes.
- **Track simple compiler register laundering before declaring a string absent**
  (`lea rN,[rip+str]` followed by `mov r8,rN` - the same forwarding shape as §3.2's vptr-install
  trap and §7's tracker). Bound more complex forwarding as unresolved rather than reporting
  absence.
- **A reason string's xref may identify a generic mapper, not the predicate that raises the
  reason.** Record reason ownership (who renders the string) and gate ownership (whose branch
  raised it) as separate facts.

---

## 3. `d1-eboot` and the generated atlas

**Built 2026-08-08; regenerated by one command, with controls that refuse to write on failure.**
⇒ historicaldocs/design/eboot-atlas.md.

```sh
capture-rig/eboot-atlas.py                   # regenerate everything, run every control
capture-rig/eboot-atlas.py --controls        # controls only; writes nothing
capture-rig/eboot-atlas.py --addr 0x574220   # everything known about one function
```

### 3.1 The headline, stated honestly

Before the atlas: `design/eboot-map.md` named 164 addresses by hand (0.46%); all six map documents
together, ~500 (1.4%). The generated atlas: **12,763 named (35.6%)**, **14,676 named or attributed
to a subsystem (41.0%)**, 21,158 unattributed - of **35,834 prologue-anchored functions**, the
denominator every published percentage uses. Plus **11,472 frame-pointer-omitted functions the
denominator does not contain**, of which 2,064 are attributed (655 named, 86 of those `thunk` -
inherited identity, not recovered). The atlas has **47,306 rows** in total.

⚠ The original doc's §1 figures (47,280 rows / 11,446 FPO / 473 schema-tag) were stale; the
corrected exhaustive census is **47,306 / 11,472 / 495** (per the 2026-08-09 verification, §4).

⚠ **A name is a statement about role and ownership, not a spelling.**
`c_investment_bap_manager::vfunc[7]` means the client calls this function through slot 7 of that
class's vtable - a compiler-recorded fact, not what Bungie typed.

### 3.2 Artifacts and name sources

| file | rows | what it is |
|---|---:|---|
| `capture-rig/eboot-atlas.tsv` | 47,306 | one row per function: name, evidence, class, vtable slot, source file, subsystem, callers/callees, probe warnings, `executed` |
| `capture-rig/eboot-types.tsv` | 2,633 | one row per C++ class: demangled name, kind, bases, vtables, slot count |
| `capture-rig/eboot-namehash.tsv` | 1,080 | the compiled `{index, name_hash}` enum tables - 168 of them, recovered whole |
| `capture-rig/eboot-callgraph.tsv` | (deprecated) | ⛔ fails 4/11 checks; its original 7/7 green controls were all direct calls. Generate fresh with `d1-eboot callgraph` (**152,686 edges**, `call` **and** `jmp` - a call-only graph misses 9,415 tail calls) |

Name sources over the prologue-anchored population (whole-file class counts in parentheses):

| technique | named | evidence | trust it for |
|---|---:|---|---|
| RTTI vtable slot | 5,101 (5,300) | `[C]` the compiler's own output | ownership and role. Exact |
| RTTI vptr install | 3,437 (3,599) | `[C]` the constructor stores that pointer | "this function builds/destroys a C" |
| string reference | 2,388 (2,511) | `[C]` the reference; `[I]` that it names the function | topic only - see §4 (identity accuracy band 18.6-46.4%) |
| schema tag immediate | 432 (495) | the *reference* is `[C]`; ⛔ "this is a schema tag" is **refuted 0/495** (§4, S1) | nothing, pending demotion |
| `__FILE__` literal | 355 | `[C]` exact | the source file - strongest textual evidence |
| FNV-1 name-hash immediate | 55 (77) | `[C]` at a 0.13% base rate | names whose string is not in the image |
| tail-call thunk | 86 (all FPO) | `[C]` | inherits the target's identity *and should inherit its confidence* (§4, step 5) |
| library import | 995 | `[C]` - the NID matches or it does not | behaviour ("calls `sceNetSend`"), not identity |

Key mechanisms, all `[C]`:

- **RTTI is intact** (built `-frtti`): 2,633 typeinfos all named, 2,429 vtables over 2,129 classes,
  22,914 slots (243 `__cxa_pure_virtual`), naming 7,946 distinct code addresses; full inheritance
  DAG. Every pointer is zero in the file and supplied by `R_X86_64_RELATIVE`. The flavour
  discriminator is **derived, not hardcoded**: the PS4 NID of the `__cxxabiv1` vtable symbols is
  computed and looked up (`pZ9WXcClPO8 -> __si_class_type_info`) on every run. ⚠ Slots are shared:
  attribution goes to the **least-derived** class carrying the implementation; `vt_shared` reports
  repeats.
- **The vptr-install trap** (worth 3,437 names): a constructor references the vtable **group start**
  (`vptr - 0x10`, the `_ZTV` symbol) and folds the `+0x10` into the store
  (`0x5745E0 lea rax,[rip+...] ; 0x18F5AE0` / `add rax,0x10` / install at `0x26A34B0`). Aimed at
  the vptr, the scan finds 3 references image-wide and reads exactly like "constructors don't do
  this" - §2.4's §4.2b in the wild.
- **The FNV-1 oracle:** `h = (h * 0x01000193) ^ c`, basis `0x811C9DC5` - FNV-1, multiply then xor;
  FNV-1a refuted at 1.00x a length-matched control, and unit tests assert both facts so they cannot
  merge. The corpus indexes sub-tokens (the client stores `bootflow:platform_signin`, not
  `platform_signin`). ⚠ Single-token lookup only: composite meet-in-the-middle cracking has a 42%
  false-positive rate per target and is not used.
- **The 638 imports - a lookup tool, not a coverage lever:** all 638 PLT stubs located
  (`ff 25 disp32` onto GOT slots, stubs `0x129D080..0x129F850`); NID is one-way so spellings are
  confirmed, never invented. `capture-rig/nid-candidates.txt` resolves 366/638 alone; **552/638
  (87%)** with shadPS4's `ps4_names.txt` (94,275 spellings, GPL-2.0). Control:
  `Q3VBxCXhUHs -> memcpy -> stub 0x129D080`. It named only **995 functions** - only 1,015 functions
  call anything more distinctive than `memcpy`/`malloc`/`mutex_lock`, and those were already named.
  ⚠ `[U]` U13: `d1-eboot imports` with no dict resolves **0/638** while `eboot-imports.tsv` carries
  552 spellings - `0 resolved` reads as "unknown", the false-absence shape; vendor the dictionary
  or add a control.
- **Jump-table recovery:** recognising the `lea/movsxd/add/jmp` idiom took dirty decodes
  2,261 -> 1,324 and recovered 10,434 instructions. A table needs **>= 2** plausible entries before
  being skipped. Control: `0x79FAE0` decodes past its table at `0x79FEB8` to `0x7A0060`.
- **Runtime evidence** `[C]` `observed-live`: 105 guest addresses observed running, harvested
  read-only from 200 run directories (79 probe fires, 15 crash-backtrace frames, 11 armed sites),
  collapsing to 40 functions; address space verified three independent ways, 105/105 below
  `0x18bf0dc`. ⚠ A floor, not a census - `executed` blank means "not observed", never "not
  executed". ⭐ The payoff is triage: **19 functions observed running and still unattributed**
  (`0x260200`, `0x56CA70`, `0x56FFB0`, `0x570A40`, `0x571890`, `0x571940`, `0x583450`, `0x584EB0`,
  `0x585820`, `0x5858C0`, `0x586430`, `0x79C370`, ...) - worth more than the next 5,000 names.
  `0x56CA70` is both FPO and probe-fired: the single control that fails if either the FPO indexing
  or the runtime join regresses.
- **Subsystem propagation - the one join:** a function inherits a name only when **every**
  name-bearing caller agrees (a majority vote launders a guess into a number). Only the **root**
  component propagates, at a **measured** 93.0/89.6/83.5% (1/2/3 hops); the full colon-path scored
  **60.8%** and was retracted, RTTI-class propagation scored 20-26% and was killed (it would have
  added 24,805 functions - strictness is not accuracy, only a holdout tells them apart). 751
  functions log a subsystem directly (`[C]`, hop 0); propagation reaches 3,031. `crossval()`
  re-measures on every run and fails the build below 85%@1hop / 75%@3; a fourth control asserts the
  colon-path is still < 80% so nobody re-adds it.

### 3.3 The honest remainder

21,158 prologue functions carry nothing: template-instantiation floods
(`cui::c_property_value::c_typed_holder<bool>` alone has 607 functions), vendored middleware
(SpeedTree/Havok/Oodle/libc++), non-virtual member functions with no literals (median 70
instructions - real code that nothing in the binary names), ~5,978 with no direct caller, 1,082
`decode-dirty`. The enum name tables resolve to hashes, not names: 1,080 entries recovered, only
**12** resolve to a spelling - the client compiles the hash and discards the string (`patrol`,
`arena`, `trials_of_osiris`, `salvage`, `rift` have no literal anywhere in the image).

### 3.4 Controls

`capture-rig/eboot-atlas.py` refuses to write unless all pass; `d1-eboot` exits 2 on any control
failure. **22 crate controls + 19 join controls, three of which are measured accuracies rather than
assertions.** (The crate gate was 27 checks through 2026-08-10 and is **31** after §2.4's four
containment controls.) The ones that matter are readings made by hand, in another session, by
another method: prologue count == 35,834; the NID oracle resolves the four `__cxxabiv1` spellings
to syms 458/459/501/511 (derived, not stored); vtable `0x18F5AF0` `[+0x80] == 0x260A10` and
`[+0xC8] == 0x260BD0` (hand-traced byte offsets); RTTI reproduces
`c_world_controller_state_bootflow_profile_setup_handler`,
`cui::c_bootflow_screen_bounds_event_handler_component` and the two activity-launch message
classes; `0x573C80` attributes to `investment_activity_selection_manager`; name table `0x274DA80`
gives `platform_signin`(2), `account_signin`(3); family 4/4 wrapper `0x8080102E -> 0xC4060975`,
inner `0x8080102D -> 0xAF776963`; the tail-call edge `0x560D27 -> 0x56FFB0` is in the graph;
`0x56CA70`, `0xA8C920`, `0x260C10` each get their own row; propagation never overrides a logged
subsystem; root propagation >= 85%@1hop / >= 75%@3 (held-out, re-measured every run); the full
colon-path < 80%@1hop (asserting the negative so it cannot return).

### 3.4b Descriptor-table fields the array readers depend on `[C]`

Rescued 2026-08-11 from `capture-rig/oodle/queuez-object-oracle.py`, whose docstring was their only
home; that script is archived to `historicaldocs/python-capture/`. They describe the DESCRIPTOR
TABLE, whose reader is `d1-eboot`, so they belong here rather than beside any one consumer.

```
anchor +0x20  u32  ELEMENT COUNT   non-zero means this descriptor is a fixed ARRAY
anchor +0x28  u32  ELEMENT STRIDE  (2 for the u16 arrays, 8 for the u64 array)
field  +0x10  u32  BIAS added to the value before it is written
```

The bias field closes something an earlier read could only observe from samples: the identity's
three 8-bit fields carry `bias = 0x80` in the descriptor, so `u8(value + 0x80)` is **read out of the
table** rather than generalised from three observations, and the roster's u16 leaves carry
`bias = 0x8000`. ⇒ provenance `historicaldocs/design/rsat-character-object.md` section 7.

The related `(family, type) -> schema tag` map, the checksum-to-record table and the `record+0x10`
trap are NOT here: they moved verbatim into `d1-oracle/src/object.rs`'s module docs, next to the
code that uses them.

### 3.5 Contradictions the atlas settled by measurement

- `0x26A34B0` **is** `s_investment_utility_context` (settled by the vptr-install bytes).
- Family 4/4 instance checksum: wrapper `0x8080102E -> 0xC4060975`, payload
  `0x8080102D -> 0xAF776963`.
- The lab-patch gate byte: vaddr `0x18C4974` == file offset `0x18C8974` (one location, two spaces);
  the third value `0x1CC4974` is past the end of the file and cannot be this address.
- `update(bap_signin)` starts at `0x798000` =
  `c_world_controller_state_bootflow_bap_signin_handler::vfunc[5]`.
- `0x260C10` has 25 confirmed references (the earlier "40" was the candidate count) and sits in 11
  vtable slots; `0x260C00`/`0x260C20`/`0x260C30` have zero of each.
- ⭐ `cui::s_activity_requirements_not_met_entry` - a class the client has for saying *why* an
  activity is not offered; nothing else in the tree mentions it.
- ⚠ **`0x9E6448` remains unadjudicated `[U]`** (U11): `historicaldocs/design/THE-CHAIN` §3.4 and
  `historicaldocs/design/director-selectability.md` say `cui::c_launch_director_activity_component`; the atlas
  says inside `cui::c_orbit_data_component::vfunc[14]`. The region may be FPO, in which case the
  enclosing prologue is not the enclosing function - the prologue trap, in a live citation.

---

## 4. Naming techniques - ship/no-ship and the honest ceiling

**Produced by the `comprehension-recon` workflow, 2026-08-09** (12 agents, five read-only lanes,
each adversarially verified, plus a holdout-integrity audit). Governing rule (roadmap §0): *a naming
technique ships with a held-out cross-validation or it does not ship.*
⇒ historicaldocs/design/ATLAS-NAMING-SHIP-LIST.md.

### 4.1 The headline

⭐⭐⭐ **Net new function names clearing the threshold is ZERO, and the correct change to the atlas
`name` column is NEGATIVE (-4,078).** That is the workflow working: it was built to catch a
26%-accuracy disaster before it shipped, and the thing it caught was its own premise.

⛔⛔ The operator's thesis - "a name obtained by running the client's own code is `[C]` by
construction and needs no holdout" - was **refuted by measurement**: a raw behavioural screen
returned **17 mappers on a 2,000-function holdout and all 17 were artifacts** (functions returning
`rdi`, whose "pointer" decoded as the `PT_INTERP` string at vaddr 0). Execution is `[C]` only after
you prove `rdi` is the index.

### 4.2 Ships (S-rows, condensed; every number corrected by the audit)

- **S1** Drop `schema-tag` from Ghidra's strong set: **0/495** schema-tag immediates is a
  descriptor tag - exhaustive, with a positive control (the predicate `imm>>24==0x80` cannot
  exclude a real tag: all 4,555 span `0x80800000..0x80805f13`) and an inverted control (360/495 are
  `0x80000000`, INT_MIN). Corroborated non-circularly: the `0x80552cXX` cluster is SCE_NP error
  space returned as a value. ⚠ The index is an *incomplete* tag oracle (`0x80B52003`, the live
  orbit scenario-record tag, is absent), so 0/495 is an upper bound on "is a tag"; the
  return-value evidence closes it independently. Removal touches **502** corpus files, not 495:
  495 rows + 6 Ghidra-thunk-propagated + `0xfca520` (atlas `name_source` literally `none`, corpus
  file `00fca520__schema_0x80552c0b.c`).
- **S2** `gen-structs.py` shrink-control fix: the control is **vacuous** - `previous_layout()`
  returns `{}` when the baseline is absent and it prints `PASS ... [0 known]`. Green against
  nothing.
- **S3** ✅ SHIPPED 2026-08-10: `state-machine.py:150` read `row["addr"]` where the column is
  `vaddr`; a bare `except: pass` swallowed it **47,306 times**, loading 0 names. `--controls` now
  asserts `load_atlas()` returns > 40,000 rows (measured 47,306) and the pump `0x782300` resolves;
  inverted before shipping (restoring `row["addr"]` reproduces `[0 rows]`, 2 FAIL, exit 2).
- **S4** Confidence column + **empty identity slot**: below tier 1 the `name` column becomes
  `FUN_<addr>` and the spelling moves to `name_evidence`; tiers identity = {rtti, rtti-vptr,
  source-file}, topic = {string-ref}, behaviour = {import, name-hash, schema-tag (demoted)}.
- **S5** Demote `string-ref` to topic (removal of 2,511 identities): identity **12/39 = 30.8%
  point, Wilson 95% [18.6%, 46.4%], unblinded** (the judge read a corpus whose headers and
  filenames carry the answer - 13,416 in-band, 10,392 in filenames); the *removal* is robust (CI
  upper bound 46% vs the 0.85 floor). 1,187/2,511 rows share a name across 276 names;
  `~networking:managed_session` names 48 functions - definitionally not an identity.
- **S6** Retire the `~` sigil, **corrected scope**: `~` marks **four** sources - string-ref 2,511,
  import 995, schema-tag 495, name-hash 77 = **4,078** - and never rtti/rtti-vptr/source-file/
  thunk. ⛔ The rule "`~` means string-ref" would discard all **995 NID-verified import names**.
- **S7** `d1-eboot descriptors --flat --tsv -> schema-fields.tsv` (subcommand to be written in
  Rust): **99.56% (4,316/4,335 tags)** whole-population internal consistency
  (`offset+size <= struct_size`); the inverted reading (bits/count column as offset) scores
  **63.23%** - a 36-point margin, wired as exit 2. **19 overflowing tags must ship flagged**
  (`0x80800357` end `0x89FE044` vs `struct_size 0xA0`; also `0x8080046B`, `0x808005C6`,
  `0x808005D9`, `0x80802049`). ⛔ Do NOT ship "11/11" - the honest independent holdout is ~8 rows
  (0.05% coverage, selected from prior successes). Positioning: a lookup consulted once a human or
  the wire has named a tag - never an automatic annotation (N3). Prerequisite: fix the extractor
  cast-cancels-scaling bug (`(long)param_1 + 0xNN` is byte-wise but scaled anyway; present in
  3,016 corpus files, mis-scaling 7,153 of 17,435 add-form offsets - it biases *against* the
  technique, so the verdicts hold, but fix before any re-measurement).
- **S8** `this` from RTTI on `vt_shared == 1` vtable slots: costs **0 call-site arguments**
  (n=200 holdout arm: net +49); definitional, not inferred - Ghidra structurally cannot derive it
  (`hasThisPointer=false` on 4,000/4,000 sampled). Emits no names, only a parameter. Runs before
  any callee-prototype commit.
- **S9** Doc retractions (the `0x559d30` thunk, the `accot` binary typo at `0x167c3e9`, the
  nametab-empty-at-runtime negative, ghidra.md §3.1 and §10.4, the `~` trap, the stale atlas
  counts) - see §9.

### 4.3 No-ships (N-rows, one line each)

- **N1** execution-derived enum naming (559 labels / 38 mappers): >= 2 of 38 are object-pointer
  tables (`0x55c1e0` is `g_bap_header_classes`, documented by name); precision <= 94.7% and
  unbounded below; the 0/2000 holdout has no surviving artifact. Also: it names enum values, not
  functions - it moves the 41.0% figure by 0.
- **N2** "97.5% static-vs-execution agreement": survivorship-filtered (25 of 133, 81% dropped) and
  near-tautological - both sides read the same bytes; all 11 divergences are base/bound, zero are
  labels.
- **N3** descriptor-to-function join, any static bridge: **0 typed / 35,834**, over-determined -
  the best-attested schema field is read as `*(int *)(lVar4 + 0x5c)` where `lVar4` is a getter
  return value two indirections down; a join keyed on `param_1 + offset` aims at the wrong register
  by construction.
- **N4** shape-matching descriptor argmax: **64/64** four-aligned offsets below `0x100` are a
  declared field start of *some* descriptor (median low offset declared by 36) - zero information;
  its deciding test's ground truth was the technique's own output and must be struck.
- **N5** bulk callee-prototype commit: **>= 5.4% (21/390) is a LOWER BOUND on one error class**
  (the detector sees only zero-params); the complement may not be quoted as a correct rate.
  Deferred, not rejected - the affirmative half (+81% call-site args) ships later, gated.
- **N6** `thiscalls` as a prototype source: **18.7% vs a 15.5% no-ground-truth control - worse than
  chance** (it detects RDI *not reloaded*, which is what happens when the callee ignores RDI). The
  best-designed negative in the workflow.
- **N7** `thiscalls`-restricted RTTI class propagation: 27.4% vs 26.2% full graph - a 6.5x narrower
  relation buys 1.2pp.
- **N8** T4 adjacency-blocked-holdout numbers: withdrawn - recall 9.6-25.5%, accuracy 76.4-89.2%
  across 6 geometries x 30 seeds; at >= 64 KiB windows accuracy falls below the 0.85 floor.
- **N9** enum application from `nametab`: **12 of 1,080** entries resolve (1%); **941/1,080 carry
  `fnv1("")` and stay empty at runtime** - with a positive control in the same snapshot and address
  space (BSS table `v:0x263cf30` IS populated, first hash `0x2ca33bdb == fnv1("none")`). CLOSED,
  not untried.
- **N10** demangling inside Ghidra: 0 - `PT_DYNAMIC` has `p_vaddr = 0`, Ghidra holds no mangled
  symbols.

Also measured futile: guess-and-check FNV over the 101 investment ids (58,960 harvested spellings,
one match - the injected control; the inverted control proves a null is corpus **absence**, not
search failure).

⭐ N3's and N4's numbers **share an unvalidated root** (RTTI truth) and must never be presented as
mutual corroboration; the one independent check on RTTI is 82 diagnostic-seed functions whose 57
distinct classes map 57/57 to one subsystem root - **root granularity only**, nothing about slots
(U2).

### 4.4 The honest ceiling

**14,676 of 35,834 = 41.0%** attributed (12,763 named + 1,913 subsystem-only); separately 2,064 of
11,472 FPO rows; whole-file 13,418 of 47,306 (28.4%). ⭐ **Quote two denominators, always** - FPO is
24.3% of rows but **8.8% of instructions** (median 10 vs 68).

| tier | functions | status |
|---|---|---|
| already attributed | 14,676 | 41.00% |
| A - shippable at the current floor | **429** (383 unanimous-root-caller + 48 non-generic PLT, union) | +1.20% |
| B - reachable only at refuted accuracy | 544 via `thiscalls` unanimity, 3 overlapping A => +541 | measured **27.4%** - a negative result, not headroom |
| **CEILING** | **15,646 / 35,834 = 43.66%** | of which only 429 are shippable |
| C - relational upper bound | 13,584 with >= 1 attributed neighbour | nothing cross-validated above 26% except root propagation, already at its 3-hop limit |
| D - structurally anonymous | 7,574 with no attributed neighbour and no distinctive import; 5,764 with no direct caller; 153 with neither | needs an external symbol source or runtime evidence |

⛔ **The atlas is not at 41% of a reachable 100%; it is at 41% of ~43.66%** - and that is the
ceiling of the **current extractor pipeline, not of the bytes** (the census zeros are true by
construction of the atlas's own extractors). ⚠ Three uncertainty bands sit outside every number:
531 `decode-dirty` and 623 `unpadded` unattributed rows (possibly not function starts), and U10's
indirectly-entered population, size `[U]`.

Structurally out of reach: the compiled enum name tables (941/1,080 `fnv1("")`, empty at runtime
too; max 12 constants); the 101 investment message ids (FNV-1 with no in-image or in-memory
preimage; ⚠ the envelope `0..303` and investment `104..1901` namespaces are disjoint, with id 303
in both meaning different things); content-side spellings if heap-resident (U7); the 7,574
anonymous functions.

### 4.5 Guards that must exist before any write

- **G1 monotonicity/shrink with a real baseline**: exit 2 when the baseline is absent unless
  `--bootstrap`; print baseline path + sha256 + mtime inside the PASS line; six counters, none may
  decrease without `--allow-shrink`.
- **G2 dual-read with the split reported** for any tool downstream of its own output (raw
  `param_1+0xNN` incl. `disp == 0`, typed `this->NAME`, and typed+additive
  `this->field + 4`); hash `decompile-index.tsv` into the artifact header and refuse when the
  corpus predates the last `structs` run.
- **G3 confidence at the point of use**: the identity slot refuses below tier 1;
  `decompile-index.tsv` must carry `name_source` and `confidence`. ⛔ **The corpus leak**: 13,416
  file headers and 10,392 filenames embed the atlas label - any corpus-reading technique has been
  shown the answer key. Mandatory holdout procedure: rename to `<addr>.c`, strip headers, `grep -c`
  each held-out label and assert zero.
- **G4 five checklist additions** (T16-T20): survivorship filter, artifact survival, sensitivity
  lives in the holdout, lower bound != rate, novelty scope (tree-wide grep, never one artifact -
  the project's #1 failure mode; it fired four times in one lane).

---

## 5. The atlas coverage roadmap - open work

⇒ historicaldocs/design/eboot-atlas-roadmap.md. **The one rule (§0): a technique ships with a
held-out cross-validation, or it does not ship.** Techniques killed by cross-validation so far:
RTTI class propagation (20-26%), full colon-path propagation (60.8%, retracted after shipping for
an hour), `this`-flow class attribution (36.2-42.1%), middleware zoning (1.8% of functions - the
middleware is interleaved, not zoned, the same root cause as the `__FILE__` failure). The method:
hide 20% of the `[C]` facts, run the technique, count how often the hidden answer comes back, then
pin the number as a floor in `crossval()`.

Done: imports (552/638 resolved, +995 names - the "unlocks 54%" estimate was wrong by 15x: what
names a function is a *distinctive* import, and a game engine's OS calls are overwhelmingly
generic); jump tables; runtime evidence (the best item - a 19-function worklist of code known to
run and not yet named). Killed: middleware zoning, `this`-flow.

**The open queue**, in value order:

1. **The 19 observed-running unattributed functions** (§3.2) - triage, not coverage.
2. **Polish that ships without new accuracy**: split the 3,598 `ctor_or_dtor` names (Itanium slots
   0/1 + `operator delete`); name the schema-tag functions by object once S1/S7 land; collapse the
   template families (readability, not coverage); mine non-diagnostic strings; diff 01.00 vs 01.29
   (insurance for cross-build name transfer).
3. **Virtual call edges** for the ~5,978 caller-less functions - over-approximates by construction;
   cross-validate against the class-seeded functions before it touches the artifact (prior: direct
   class propagation scored 26%).
4. **The open `[U]` ledger** from the naming workflow (all offline unless marked BOOT):
   - **U1** ⭐⭐ library-signature matching against the middleware the atlas already names: 462
     `source_file` rows over 125 files - DemonWare/45 291, Havok 96, oodle 53 - versioned SDKs with
     knowable trees = a free, external, non-circular ground truth **outside** the 43.66% ceiling.
     ⛔ Licence fence: read-only structural matching; AGPL and unlicensed prior art stay unread.
   - **U2** ⭐ is RTTI itself correct at slot level? (the unvalidated root under two lanes) - test
     whether RDI/RSI/RDX are read before written in the entry block, with a no-ground-truth control
     group.
   - **U3** ⭐ is the corpus label leak load-bearing? Run one technique on a verbatim corpus and a
     stripped/renamed one; a large delta voids every corpus-reading number to date.
   - **U4** is the >= 5.4% wrong-prototype rate stable outside RTTI slots (prerequisite for the
     callee-prototype commit).
   - **U5** is there ANY tag-to-code binding, or is RSAT resolved through the package registry at
     runtime? (**BOOT**: `d1-live watch --rw exec` on the tag-resolution entry; positive control
     `(0x80800E76, the AH codec caller)`.)
   - **U6** descriptor `+0x08` (`0x4A54883A` for `global_activity_state`) is NOT `fnv1` of the type
     name; if it is a name hash under another function, 4,555 struct names become crackable - test
     the cheaper sequence/timestamp hypothesis first.
   - **U7** heap-resident `{char*, fnv1}` registry records - every scan so far excluded heap
     pointers **by construction**; re-run over the snapshot's three non-eboot regions (279 MB);
     self-validating via the fnv1 invariant.
   - **U8** mappers keyed on a pointer or arg 2+ - a measured blind spot of the N1 screen, not an
     absence.
   - **U9** where does the client render `ENUM(%d)`? (`strings` -> `xref` offline; confirm needs a
     **BOOT**). ⚠ `d1-eboot strings` with no flags prints usage, not the corpus - a
     `strings | grep` negative is not evidence of absence (bit two lanes; `debug-command` is in the
     static image at `0x1615722`).
   - **U10** functions entered ONLY indirectly - every denominator excludes them and the size is
     unknown; recover via 22,914 RTTI slots + `R_X86_64_RELATIVE` addends; positive control:
     `0x56FFB0` must be recovered.
   - **U11** adjudicate `0x9E6448` (§3.5).
   - **U12** what do the 4,555 descriptors buy in the decompiler? (the 20 frontier descriptors ->
     `/D1/schema` structs -> re-decompile -> diff; accept only with the FIT control and a
     `d1-oracle` round trip.)
   - **U13** vendor the NID dictionary into `d1-eboot` (§3.2).
   - **U14** where does root propagation cross the 85% floor? (blocked by n: needs full 5-fold
     rotation, not a 20% split.)
5. **Hard / research-grade**: cross-binary symbol transfer (constrained by `design/licensing.md`;
   do not start here); LLM-assisted bulk naming only with a falsifiable check (a proposed name must
   predict something independently verifiable, hit rate published and floored); the X360 alpha
   corpus as the only source for the 101 investment ids and slot-level RTTI validation.
   ⛔ **The type-name registries: DO NOT ATTEMPT** (§2.1 - proven uncrackable; this is the
   THE-CHAIN "space 2", not the FNV-1 space, and conflating the two lost a session).
6. **Re-pin the propagation negative control**: the "colon-path < 0.80 at 1 hop" floor has 0.5pp of
   headroom across 200 seeds (max 79.5%) - restate at < 0.85 or pin the mean; publish the null
   model (68.0%) and the non-modal split (hop 3 non-modal 80.9%) alongside every propagation
   accuracy; state the 0.85/0.75 floors as **inherited regression controls, not derived gates**.

**The stop condition:** this is documentation for preservation, not decompilation. Stop when "which
function is this and what subsystem owns it" is answerable for everything on the **boot path and
the wire path** - which the `executed` column measures directly. The engine's rendering and physics
functions can stay anonymous forever without costing a gate.

---

## 6. Ghidra - the semantics instrument

**Built and verified 2026-08-08** `[C]` `static-client-inference`.
⇒ **`design/ghidra-bringup.md` is the PROCEDURE** (zero to a citable database, with the decision
table for when to reach for Ghidra at all); this section is the result state and the caveats.
Provenance: historicaldocs/design/ghidra.md.

The atlas answers "what is this function called?"; Ghidra answers "what does this function do?".
⚠ **Ghidra outranks nothing.** Its output is `[I]`, a hypothesis; `d1-eboot` remains the authority
for bytes, relocations, xrefs and descriptors, and `d1-oracle` settles behaviour by executing the
real code. Nothing from Ghidra enters `d1-server` as code (Ghidra is Apache-2.0 and lives outside
the repo at `~/dev/ghidra_12.1.2_PUBLIC`; the GPL position is untouched).

### 6.1 Installed state

Ghidra **12.1.2** (zip sha256 `b62e81a0390618466c019c60d8c2f796ced2509c4c1aea4a37644a77272cf99d`,
matching the NSA release body), JDK 21 (Fedora's JDK 25 is rejected),
`GHIDRA_HEADLESS_MAXMEM=48G`. Project `~/dev/ghidra-projects/D1.gpr` (deliberately outside the
repo, ~2 GB regenerable); tracked pieces in `ghidra/` (`D1Atlas.java`, `D1Verify.java`,
`D1Decompile.java`, `D1AnalysisOptions.java`, `D1Structs.java`, `d1-ghidra.sh`, `lists/`,
`structs/d1-structs.tsv`, `gen-structs.py`).

Loaded state: **48,040 functions, 9,749 strongly-sourced names, 638 named PS4 import stubs, 2,633
RTTI typeinfo labels, 1,861 vtable labels, 237,615 symbols.** Whole-DB rebuild ~5 min 45 s from
tracked inputs; atlas re-run 1.5 s, idempotent.

### 6.2 ⭐ The addressing gotcha

The eboot is `e_type = 0xFE10` (`ET_SCE_DYNEXEC`); stock Ghidra has **no SCE ELF support** and
loads it off `e_machine` and the program headers anyway. Result, verified `[C]`: `segment_0`
`0x0..0x18bf0db` r-x from `PT_LOAD[0]`, `segment_1` `0x18c0000..0x2f50dbf` rw- from `PT_LOAD[1]`,
`imageBase = 0`, executable sha256 `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`
(the pinned eboot). **Ghidra addresses are our addresses, one-to-one, no rebasing.** Three caveats:

1. The file/vaddr bias is per segment (§2.3); never hardcode `0x4000`.
2. Ghidra reports the dynamic table "stripped" - it is not; `PT_DYNAMIC` has `p_vaddr = 0`, so
   Ghidra cannot map it. **Ghidra knows nothing about relocations, dynamic symbols or NIDs**;
   `d1-eboot reloc` is the only authority on the 70,682 relocations.
3. ⚠ **BSS reads as zeros here rather than refusing** - zero from Ghidra in BSS means "no file
   bytes", never "the runtime value is 0".

Auto-analysis is tuned (`D1AnalysisOptions.java`): discovery analyzers off - and critically
**`Shared Return Calls` off**, because it rewrites a tail-`jmp`-entered function into a thunk of
its target, which is exactly the §2.4 tail-call shape that cost two sessions.

### 6.3 The atlas import - and its filter

`D1Atlas.java` phases: disassemble at every atlas start (3,746,140 code units); a function at every
start (made 47,179, failed 0); name the 638 PLT stubs; apply **strong `name_source` only**; label
2,633 typeinfos and 1,861 vtables; mark the ELF entry `0xC2B470`. Everything written is
`SourceType.IMPORTED`, so our claims and Ghidra's never merge. Every one of the 47,306 rows gets a
plate comment carrying name, source, class, slot, subsystem, tags, strings and warnings; weak rows
carry an explicit `!! name_source '...' is WEAK -- not applied as an identity` line.

⛔ **The strong set is under measured correction** (§4, S1): at melt time `D1Atlas.java:62` still
includes `schema-tag`, whose identity accuracy is **0/495**; the ship list drops it, empties the
identity slot below tier 1, and suppresses Ghidra's native thunk-name mirroring (6 thunks -
`0x135190 -> 0x192490`, `0x1cfae0`, `0x23c520`, `0x2cf730`, `0x7b7540`, `0x870140` - carry an
applied `~schema_*` identity despite their own plate disclaimers). ⚠ **Filter on `name_source`,
never on `~`** - that half of the original §3.1 guidance stands; its supporting argument does not
(§9).

Deliberately not covered: no structs beyond §6.6, no signatures from us (beyond §6.6), no
vtable-to-function slot binding, no callgraph import (**indirect calls are invisible to Ghidra's
callgraph** - the pump at `0x34C010` calls `handlers[slot]` and Ghidra knows no target).

### 6.4 The positive controls - and what the run turned up

`./ghidra/d1-ghidra.sh verify` - **5 byte controls, ~4 s** - must pass before anything in the DB is
cited. Three functions already understood by hand were decompiled and compared; **all three agree**:

- `0x34C010` the command-queue pump: every constant matches (`0x1B4`, `0x80`, `0x200`, `0x1C04`,
  19 slots, both CASes, the indirect call) - Ghidra independently recovered `pump(registry)`.
- `0x260200` the availability builder: unconditional `header[0] = 1` at `0x260438` plus exactly
  five fall-through `verdict = 0` sites - `{0,0,0}` unreachable, confirmed a third way. ⚠ One
  boundary correction: the extent cited as `0x260200-0x260CAD` actually ends near `0x2608B0` (two
  instruments against one prose range).
- `0x753540` change-world: all six `world_controller:` diagnostics in their documented branches;
  the not-found path closes inside `0x341500` (`"activity '%s' not found"`).

⭐ **The cross-check found rung 28's requester** - an `[U]` in THE-CHAIN §5: the handler for state N
requests state N+1, so rung 28 `setup:activity_session_creation` is requested at **`0x7AB460`**
inside `0x7AAF60` = `c_world_controller_state_setup_orbit_handler::vfunc[7]` - `[C]` by
`d1-eboot disasm` bytes (`mov edi,0x1c` then a `jmp` into a call site shared with state 25's
cleanup - a fourth instance of the tail-call blind spot; none of the three guessed candidates).
**Rung 26 stays `[U]` for a stated reason**: cleanup transitions to a **stored** state
(`param_1 + 0x6c8`), not an immediate - `[C]` that the immediate does not exist; `[U]` the runtime
value, a *live* question.

### 6.5 The decompiled corpus

⭐ **The whole binary is decompiled to disk**: 47,304 `.c` files, 248 MB, at
`~/dev/ghidra-projects/decompiled/`, one file per function plus `decompile-index.tsv`. Full-binary
decompile: **191 s wall at 24 threads; 47,303/47,306 succeeded (99.994%)** - the three failures
(`0x141480`, `0xC55F90`, `0xC55390`) are a Ghidra internal on Havok vfuncs, none on a critical
path. Decompilation is free relative to reading; the bottleneck is verification, and that is
`d1-oracle`'s job. ⚠ **The corpus is a snapshot** - refresh after any `atlas`/`structs` run or you
grep stale names. ⚠ Duplicate names are real: 9,749 strong names cover 6,761 distinct strings;
`D1Atlas` appends the address to shared names. ⚠ 231 atlas starts had no function after
auto-analysis until `atlas` was re-run - **re-run `atlas` after `build`, it is 2 seconds.**
⛔ The corpus leak (§4.5, G3): headers and filenames embed the atlas label; strip both before using
the corpus to score anything.

Session ritual: `verify` first; grep the corpus; read the plate comment for provenance
(`FUN_00xxxxxx` means the atlas had nothing strong); treat every reading as `[I]` until `d1-eboot`
bytes or a `d1-oracle` execution settles it. Ready-made lists: `ghidra/lists/critical-path.txt`
(the 3v3 path - ⛔ its rung labels were shifted by one until 2026-08-08; fixed in place),
`controls.txt`, `bench500.txt` (⚠ sampled FROM the atlas - never a naming holdout; the filename
invites the tautology), `all.txt`.

MCP: nothing installed, and the honest counsel is "don't bother yet" - the corpus on disk is faster
to grep than any RPC. If wanted: `symgraph/GhidrAssistMCP` (MIT, single extension, script execution
off by default). ⚠ Apache-2.0 is incompatible with the GPL-2.0-only arm, so vendoring Apache code
into `d1-server` would cost the "or 2.0" option; running an external tool creates no derivative.

### 6.6 Structs - deriving shape, authoring only names

`./ghidra/d1-ghidra.sh structs`, input `ghidra/structs/d1-structs.tsv` (329 rows: struct 50, field
225, apply 43, sig 5, param 6), generator `ghidra/gen-structs.py` (**10 controls**, refuses to
write on failure). **50 structures**: the state-handler ladder (15), the task set (6), session
parameters (29); `this` typed on **43** functions, 5 skipped (rule 3 below and three functions the
decompiler believes take no arguments). The before/after: `*(long *)(param_1 + 0x38)` becomes
`this->debounce_ms`; cleanup's update reads essentially as source. ⭐ The structs paid for
themselves: three unrolled `N = 1` task sets (rungs 26, 33, 36) were found *because* the derived
fields made the shape visible - there was no callable runner to have found by xref.

The three rules, all enforced by controls: (1) derive the shape from what the class's own vfuncs
touch in the corpus - never accept a layout from prose; (2) a field takes the smallest observed
width, clamped to the next observed offset (so it can never swallow `pending_goal_state`/`
pending_goal_reason` at `+0x6C8`/`+0x6CC`); (3) `this` only where `vt_shared == 1`. Plus the
fourth control: every curated field offset must be an offset the class's vfuncs actually touch.

> ⚠ **THE SELF-REFERENCE TRAP**: the generator reads the corpus its own output rewrites - after
> `structs`, a typed function contains `this->debounce_ms`, not `param_1 + 0x38`, so a naive rescan
> proposes deleting every field it already described. Fixed by reading both forms through the
> previous generation's map, plus the shrink control. Generalised: any instrument whose input is
> downstream of its own output needs a monotonicity control. ⛔ And §4's S2: that shrink control is
> **vacuous when the baseline is absent** (`PASS ... [0 known]`) - the fix is queued.

Field-name convention: `debounce_ms` = measured; `i_something` = meaning inferred, offset/width
measured; `f_0x40` = offset real, nothing else claimed. Every field carries an inline
`[C]`/`[I]`/`[U]` comment.

⚠ §10.4's cost claim is **retracted** (§9): committing a signature does NOT cost call-site
arguments (the no-signature and committed arms are md5-identical); the loss was pre-existing, and
the correct remedy - committing **callee** prototypes - moves call-site args 1,008 -> 1,822
(+81%). Say both, or the next reader discards the correct fix with the false cause. Cosmetic but
real: `updateFunction` must be passed a calling convention or every typed function carries the
"Unknown calling convention" banner.

---

## 7. `d1-eboot field-xref`

**Who READS and who WRITES `object + offset`.** Built 2026-08-09 from the rung-33 world-transition
question; **5 pinned controls, wired into d1-eboot controls, 27 checks, exit 2 on failure** (the
crate gate is 31 checks as of 2026-08-11, §2.4). Reach: `static-client-inference` - nothing here
touches a live client. ⇒ historicaldocs/design/field-xref.md.

### 7.1 Why it exists

"Who writes `c_activity_loader+8` (`DAT_0267AF08`)?" was asked with `xref` against the field's
absolute address `0x267AF10`; the answer came back nearly empty **with all four xref controls
passing**. The field is reached register-relative
(`0x33FF30 lea rax,[rip+...]` -> forwarded in RDI -> `0x340240 mov rax,[rdi+8]`), and
`48 8b 47 08` contains **no address at all**. ⭐ Four passing controls prove the instrument, never
the technique: when a healthy scan returns nothing, ask whether the target is even *expressible* in
what the scan matches on.

### 7.2 What it does and how

```sh
d1-eboot field-xref --class c_activity_loader --global 0x267af08 --offset 8
d1-eboot field-xref --global 0x267af08 --offset 8 --all --candidates
d1-eboot --json field-xref --class c_activity_loader --global 0x267af08 --offset 8
```

Candidates are a union of five mechanisms: RTTI class vfuncs (`class-vfunc`), ctor/dtor functions
RIP-referencing the vptr **and `vptr-0x10`** (`class-vptr`), confirmed riprel users of the global
(`global-ref`), the accessors' callers (`accessor-caller`), and one hop of argument forwarding
(`accessor-forwarded` - the tracker saw the object in RDI at a `call`). For `c_activity_loader` +
`0x267AF08` this is **67 functions**; the accessors resolve to `0x33FF30` and `0x341C70`.

⛔ **The prologue trap bit this module on its first run and is now a test**: `FUN_0033FF30`
(`lea; ret`) has no prologue; attributing it via `Hit::function` (nearest prologue) named
`0x33FB20`, so `callers()` found the wrong 8 functions and the sweep silently missed its own
control (10 candidates instead of 67). Accessor attribution goes through the containment-aware
index (`accessor_attribution_survives_a_function_with_no_prologue`).

Each candidate is decoded prologue-anchored with jump tables skipped; memory operands are
classified with **iced's own instruction-info**, not by mnemonic guessing (so `lea rax,[rdi+8]` is
an ADDR, not a bogus read). A register-to-object-offset tracker (the same shape as `thiscalls`) is
seeded from RDI at entry, from `lea reg,[rip+global]`, and from **RAX after a call to an accessor**
(the edge `xref` cannot follow); propagated through `mov reg,reg` and `[rbp-N]` spills;
**moved, not killed, by `add`/`sub`/`lea reg,[reg+disp]`** - which is what closes the `disp == 0`
blind spot (`c_activity_loader::vfunc[1]` does `mov rbx,rdi` / `add rbx,8` and spells the field
`[rbx]`, displacement zero); killed on any other register write and across every `call` for the
volatile set.

**Escapes are the only "writer" that exists here.** Nothing in the image stores to
`c_activity_loader+8`; `vfunc[1]` hands the field's *address* to `FUN_0023DE00` in RDI and that
function writes through the pointer. A store-only field scan would answer "nobody writes this
field" - confidently and wrongly - so a register holding `object+offset` live in an argument
register at a `call` is reported as an **escape** with the callee named.

### 7.3 The honest limits - `[C]` vs `[U]`, per row

Every row carries its own confidence: `[C]` the base register provably held the object; `[U]` the
displacement matched and nothing else did (hidden unless `--all`, never counted as evidence); `ABS`
a RIP-relative operand landing exactly on `global + offset` - the only class `xref` could already
see. Three named limits:

1. ⚠ **The tracker is linear and branch-oblivious** - a value that becomes the object only on one
   path is missed. False negatives are possible.
2. ⚠ **The tool does not know the object's SIZE** - an `ABS` row at a large offset may be a
   neighbouring global (at `+0x18` all three ABS rows are references to `0x267AF20`, a separate
   global). An aim problem, not an instrument problem; the reader must judge it.
3. ⚠ **`--depth` is 1 by default** - a field touched two argument hops away is not in the candidate
   set unless raised.

`truncated` rows distinguish a dirty decode from inter-function padding by recording whether the
stop happened after a terminal instruction. ⛔ And the §1 blind shape stands: an indexed heap-pool
element is invisible to this tool too.

### 7.4 The five controls, and the proof they can fail

The axis: *find register-relative access an absolute-address xref is blind to by construction, and
do NOT simply report every `[reg+N]` walked past* - two different failures, so the set is a
discriminating pair:

| # | control | expects |
|---|---|---|
| 1 | `0x340240` `mov rax,[rdi+8]`, bytes `48 8b 47 08` | FOUND - reachable only through the argument-forwarding hop |
| 2 | `0x33FFA2` `mov rax,[rbx]`, bytes `48 8b 03` | FOUND - the `disp == 0` case |
| 3 | `0x33FF9D` `call 0x23DE00`, bytes `e8 5e de ef ff` | the field ADDRESS escapes in arg0 |
| 4 | INVERTED: same class/global/candidates, offset `+0x20` | **0 tracked, and >= 20 raw displacement candidates** |
| 5 | premise: `xref 0x267af10` | does NOT contain `0x340240`, and is not empty |

Each is pinned at the byte level, re-derived from the image. ⭐ Control 4's `min_matches` half stops
it being a tautology: a dead tool returns zero tracked too; demanding the sweep still surface the
raw candidates makes "empty" mean *discriminated*, not *dead*. Hand-verified that `+0x20` is
untouched (the object is `{vptr@+0, pointer@+8}`; `0x267AF18`/`0x267AF20` are separate globals;
the offset histogram puts all traffic at `+0` and `+8`).

⭐ **Each control was broken on purpose before shipping** (a control that only ever passes is
decoration): don't seed RDI => 3 positives FAIL; `tracked: true` unconditionally => the inverted
control alone FAILs (the row that catches a degenerate "report everything" implementation);
`add`/`lea` kill instead of move => 2 FAIL; drop the forwarding hop => 1 FAIL + liveness FAIL;
attribute accessors by nearest prologue (the real first-run bug) => 1 FAIL + liveness FAIL.
`cargo test -p d1-eboot` runs all five; `d1-eboot controls` exits 2 on failure (verified with the
tracker broken).

### 7.5 The worked result

`+8`: **24 tracked accesses over 124 displacement matches, 67 candidates - 24 READs, zero stores**;
the field address escapes to `0x23DE00` (create) and `0x23E030` (release), proving `+8` is a heap
handle. ⭐ The escape rows were the finding: they redirected the search one indirection down, and
`historicaldocs/design/activity-loader-writer.md` closed the rung-33 `[U]` - the block is `0x3DC` bytes holding
three activity records, and the name's writer is `FUN_00341500 @ 0x341798`
(`memcpy(block+0x000, block+0x148, 0x145)`, a pending-to-current promotion at displacement 0).
The semantic interpretation is deliberately the caller's job; the tool ships raw output.

---

## 8. `d1-replay` - the live-state replay oracle

**Built 2026-08-07, extended through 2026-08-11.** `d1-replay controls <snap>`: **20 PASS / 0 FAIL /
0 SKIP, exit 0**, re-run on six snapshots spanning three eras of the format; `cargo test --release
-p d1-replay` 26 unit + 3 integration green (4 of the unit tests belong to `rederive`, §8.9).
⇒ historicaldocs/design/replay-oracle.md.

### 8.1 The gap it closes

Three instruments existed and none answered "what value was in this register when this code ran"
(the original access-mode table, now the fuller §1): `d1-eboot` sees code, blind to every runtime
value; `d1-oracle` sees code executing, blind to anything not derivable from supplied arguments;
`d1guest.py` sees live memory, blind to registers and any chosen instant. `d1-replay` captures the
client's memory, rehydrates it into Unicorn **at the client's own addresses**, and executes chosen
functions on it: register dumps, bounded single-step tracing, watchpoints, and a **candidate
search** (`--vary`/`--hit`) that runs a function over a set of inputs and reports which produce a
chosen outcome. At the walls this project hits, the unknown is a small dense integer against a
capturable table, so the answer is reachable **by search rather than by deduction** - and where the
candidate set is exhaustive, an *empty* result is the stronger finding: the **state**, not the key,
is wrong.

### 8.2 The honest limitation - two kinds of snapshot, two kinds of claim

⛔⛔ There is no single blanket sentence any more; quote **the result's own REGISTERS block**:

| | entry registers | the sentence it supports |
|---|---|---|
| any snapshot, default | **INVENTED** - GPRs zeroed, `eflags = 0x202`, then `--arg`/`--reg` | "given the memory the client had at instant T, input **X** makes this function hit" - **never** "the client's input was X" |
| `capture --snapshot-on-hit` + `replay --use-captured-regs` | **CAPTURED** - the trapping thread's integer file via `PTRACE_GETREGS` | "with the integer arguments the client actually held, against memory read over the following ms while ~110 other threads ran" |

The claim ladder (§11a of the source): rung 1 = the invented row; rung 2 = captured registers
(⭐ the unknown stops being *yours* - "the id it actually had was `0x2C` and the function returns 0,
so the **state** is wrong, not the key", the sentence that names a server-side deliverable); rung 3
("the client executed this and got R") is **UNREACHABLE by this instrument**. Rung 2 still does not
buy: `rip`, `rsp`/`rbp` (this run's own frame - the stack is invented even when the arguments are
captured), **all FP/SSE state**, anything overridden - each printed next to its captured value.
⛔ `trap: none` means **NOT ATTEMPTED**, never "the client had no registers"; all 18 archived
snapshots predate the field, and `--use-captured-regs` **refuses** on them. ⛔ No trap-paired
capture has ever been taken from shadPS4 - the mechanism is `[C]` on toy children, `[U]` on the
client. ⚠ Even rung 2 is not an atomic freeze: `Coherence` records `all_threads_stopped: false`,
five timestamps, and which regions were read after the resume; the caveat is rendered **in the
result itself**, conditional on the file's own numbers.

Standing limits, all printed in `--help`: a snapshot is one instant; it replays a fragment (no
threads/scheduler/syscalls/GPU/allocator - a fault is a **result**, and reachability under replay
is not evidence the client reaches it); region selection is lossy by construction - a read outside
captured regions **fails loudly** and is never zero-filled ("we did not capture it" and "it was
zero" are different facts); reach tags propagate (a snapshot of a client driven by our synthetic
server stays `synthetic_ancestor`); and by default it alters one pointer of captured memory - the
stack canary (§8.5). ⚠ Do not filter mappings by size: shadPS4 reserves tens of TiB sparse and the
guest heap lives inside; an earlier tool skipped > 1 GiB mappings and searched 0.01% of memory
while reporting a confident "0 hits". Filter on Rss.

### 8.3 The expression language and rollback

Three address spaces meet here: eboot vaddr (`0xB471E0`), shadPS4 guest/host (`0x800B471E0`), file
offset (used by nothing; the bias is per-segment). **Every resolution is printed.** ⚠ A bare number
is a vaddr in an address position and a LITERAL in a value position - deliberate, and paid for
(`--arg 0` once resolved to `base + 0`); pinned as a test. Rollback journals every write via a
`MEM_WRITE` hook, restores from the snapshot, then **re-verifies and aborts rather than reporting**
if memory differs - a rollback that quietly did nothing makes candidate N+1 inherit candidate N's
writes.

### 8.4 The controls - 20 rows, honestly counted

Highlights: build gate; eboot identity; snapshot-vs-file plus a shifted-window **negative control**;
relocation (`boot_state_names v:0x192d2a0` zero in file, non-zero in snapshot); trampoline;
execute-vs-read; fault-is-a-result; rollback; three stack-canary rows (one inverted);
registry cross-check; existing-snapshots-still-load (the archive guard - 18 files, manifest-only,
with a **degeneracy check** on the raw JSON requiring at least one file to LACK the new key);
regions-do-not-overlap (inverted on real data: an overlapping copy produced FAIL, exit 2);
stop-budget-refuses-above-the-cap; phase-split-is-its-own-pair; captured-vs-invented-registers
(four runs incl. POISON; a text term; a fidelity term that waits on the first real trap capture).

> ⚠⚠ **Honesty banner: not all twenty are load-bearing.** `build gate` and `eboot identity` are one
> comparison wearing two rows; the stop-budget and phase-split rows are **pure** (they detect a
> threshold edit, not a wiring regression); `regions-do-not-overlap`'s multi-region branch had
> never run on real data; `captured-vs-invented-registers` runs against a control stand-in on every
> archived snapshot. **The honest count is 20 rows, of which ~16 can fail independently today, and
> one whole leg is waiting on the first trap-carrying capture.**

Three rows reproduce results the project had already paid for (ATLAS §4.4d): execute-vs-read
returns **3** = the measured `0x263D2E8 = 3`, obtained by executing the client's own getter;
fault-is-a-result reproduces the exact live crash address ("Read from address 0x8", §26.77);
registry cross-check reproduces `d1guest.py selftest`'s 18/18 from a file. A `replay`/`search` run
with `--no-controls` prints `NOT EVIDENCE` and exits 2.

Inversion lessons worth carrying: deleting `#[serde(default)]` from the new `Option` field changed
**nothing** (serde routes a missing Option to `None` anyway) - the annotation is documentation, not
mechanism; only making the field non-Option broke all 18 at once. Do not call an inversion done
until you have watched the thing fail. And the register-zeroing inversion **passed on first
attempt** (a neighbouring control happened to leave `r13` clear) - the POISON run was added for
exactly that; a control that cannot fail is decoration.

### 8.5 ⭐⭐ `--stack-canary` - the harness limit that silently deleted >= 45% of the search space

Nearly every function's prologue loads the guard pointer at `v:0x1A06C78` and dereferences it; the
captured bytes are a pointer into **host TLS**, unmapped in Unicorn, so the replay died at **step
8 having executed nothing - in a shape indistinguishable from "your arguments were wrong"**. Scale:
`d1-eboot xref 0x1a06c78` gives 25,848 riprel sites across **16,117 distinct functions = 44.98% of
35,834**. ⚠⚠ **Cite it as ">= 45%", never "45%"**: the riprel scanner has a known open under-count
defect (drops the disp32 under a trailing immediate; `xref 0x266deac` reports 0 where `disasm`
resolves 3, all controls green), so every miss raises the true share - safe to quote in that
direction only, never as an upper bound.

`--stack-canary <auto|off|ADDR>`, default `auto`: a dedicated in-emulator page holding
`CANARY_MAGIC = 0xd1d1ca5ad1d1ca5a` (its own page, disjoint from scratch/stack/trampoline, asserted
by a `#[test]`); redirects **only when the captured pointer is unmapped**; journalled (rollback
restores the client's pointer); ordered before `spec.writes` so `--set-mem` still overrides;
reported on every run and in every `--json`; the guard global's address re-derived **from the
instruction stream**, not trusted as a constant. ⛔ **What it buys, exactly: execution past the
canary. Nothing more** - a later fault is a result. ⭐ The tell is the **shape** of the fault
address: `0x55...` is a host pointer = *the harness cannot run this*; `0x2...`/`0x3...` is a guest
heap page = *we did not capture that region* - only the second kind is evidence about the client.
⚠ The captured pointer differs in every capture (host ASLR; three distinct values on disk) - every
control reads it out of the snapshot. `[U]`: the guard VALUE is never checked against the client's
real one (cannot be, from a memory snapshot); and a guard copy propagated through another global
would be missed by the 25,848-site count.

**The adversarial review - six defects, two of which passed 11/11 controls, exit 0:**

| # | what broke | the control that now catches it |
|---|---|---|
| D1 | `redirected` asked the PLANNER whether it planned, not memory whether it happened | `CanaryReport::effective` - the pointer **read back out of memory**; `altered()`/`redirect_landed()` computed from it alone |
| D2 | the pristine verifier samples `0x1000` of every `0x40000`; the guard sits at offset `0x6A78` - **structurally outside the sample, forever** | an explicit 8-byte compare of the guard global, alongside the wider net |
| D3 | `auto`'s "only when unmapped" safety had **no control at all** - unconditional redirect still passed | an INVERTED control: plant a mapped pointer and require `auto` to write **nothing** |
| D4 | "the canary round-trips" was never executed - the subject faults before its epilogue | a second subject, `v:0x3FE580` (15 instructions to `ret`, `__stack_chk_fail` `v:0x129D0B0` as a probe): OFF faults in 5 steps, AUTO returns in 15 |
| D5 | the scale quoted as a flat "45%" | documented as **>= 45% with the direction of error named**, in four places |
| D6 | no `#[test]`s - every claim rode on a control block needing a multi-hundred-MB snapshot | three unit tests, one inverted |

Transferable rules: ⭐⭐ **ask MEMORY whether it happened, never the planner whether it planned**;
⭐⭐ **a sampled verifier must be shown to cover the bytes it is cited for**; ⭐ a safety claim whose
branch no input exercises is not tested by any number of passing runs - only a manufactured
(inverted) input reaches it.

### 8.6 `--preset content` - one capture instead of N rounds `[C]`

The grind: three replay rounds of the rung-34 content lookup `v:0x10c500`, each faulting on a page
nobody had named (`0x2044126d0`, `0x38031ca38`, `0x22db821de`). The fix is at capture time, and it
is not a bigger window - it is **reading the client's own tables**: ⭐⭐ the client describes its own
heaps, with names and exact sizes (`0x1ABCB80`, self-relative, `0x140`-byte records
`{name[0x30], base@+0x30, size@+0x50}`; live: **199 arenas, 345.4 MiB**, incl.
`"Datum Array Table Manager"` - whose base IS the package registry slot table - `"Tag Memory"`,
`"client reference manager"`). An arena is an exact extent, so it beats a guessed window - the
fault at `0x22db821de` is `blob + 0x2bd5e` inside a `0x2be80`-byte arena, *only just*. The preset
captures the arena table, both root arenas, the slot table, every populated slot's entry array, and
bounded object windows: **+10.2 MiB on a 47 MiB snapshot**, and `v:0x10c500` goes
**27 steps/FAULT -> 94 steps/RETURNED**. What it does NOT do, stated first: a fifth population
still faults (a result); objects get a bounded window; a dangling back-offset is skipped and
counted, never zero-filled; over `--preset-max` it refuses loudly. Roots live in
`capture-rig/guest-symbols.json`, not crate constants.

The control (`preset-control`) replays the same function against two snapshots differing by one
flag and requires the WITH arm to retire strictly more instructions or return where WITHOUT
faulted; both snapshots must pass the full control block first, and the pair must genuinely differ.
**Three inversions, all exit 2** - the decisive one stubs `content()` to return zero regions while
keeping the declaration, proving the control measures **coverage**, not a manifest flag. ⚠ One-way
coupling: with `--stack-canary off` both arms die at 8 steps and the control FAILs by construction
- the canary had to land first. ⚠ The honest caveat that outranks the win: the preset makes the
lookup **replayable**, not **satisfied**.

### 8.7 First-use findings (what the instrument is for)

- The fireteam table read and then executed `[C]` `observed-live`: watchpoints confirmed the whole
  contract (gate `G+0xf4`, key `G+0xf8`, id compare `G+0x118`) by execution, not arithmetic; the
  candidate search over 11 keys found exactly `0` (the registered slot) and `0xFFFFFFFF` (the free
  sentinel is itself a matchable id - a latent aliasing hazard in the client's own design).
- ⚠ **A defect found in a shipped lab patch**: `CUSA00219-lab-manifest-T1.xml`'s not-found guard
  justified itself with "`al` is already 0 from the prologue" - false; `rax` is reused as the table
  cursor and every miss returns `al = 0xe8` = **true**, so the guard turned "crash" into "silently
  claim the lookup succeeded". `[C]` `observed-live`. Invisible to static reading, to a live memory
  dump, and to the offline oracle; it took one trace. The patch was annotated in place, not edited
  mid-experiment.

### 8.8 Boundaries and operating rules

`d1-oracle` owns eboot-to-Unicorn mapping, codec invocation, schema-tag entry points and the
`unicorn-engine` dependency; `d1-replay` owns snapshot capture/format, replay, observation and
search. `Harness::map_region` is deliberately not used: different address space (rebasing a
captured object graph means rewriting pointers - the silent-corruption class), and one dispatching
code hook per machine, ever. `d1-oracle::registry::build` overwrites `0x1A0B420` with a synthetic
registry; nothing in `d1-replay` goes near it. ⚠ The emulator is exclusive to the operator:
`capture` is read-only on `/proc/<pid>/mem`; ask for a snapshot rather than starting a run -
capture once, iterate offline (everything in §8.7 came from a single 47.9 MiB file). Licensing:
the whole transitive set audited; strongest copyleft (GPL-2.0 QEMU via `unicorn-engine`) confined
to the two crates that need it; nothing AGPL, nothing unlicensed.

The `--snapshot-on-hit` capture ladder (resolve -> plan -> rehearse -> refuse above the cap ->
allocate -> arm -> trap -> disarm-and-verify -> read while halted -> resume -> write one file), its
signal-safety history and the armed-window rules are live-side material: `historicaldocs/design/live-watch.md` and
`design/instruments-live.md`.

### 8.9 `rederive` - the activity-lifetime evidence control `[C]` (2026-08-11)

`d1-replay rederive <snap>` re-derives the `design/activity-lifetime-state.md` record facts from a
snapshot, walking the client's own datum registry (`[v:0x1A0B420]`) instead of trusting a
transcript's cached addresses: the three fixed schema slots, the record for datum `0x27FA2003`
(the `+0xA8` bound-setter quad, the `+0x108` unbound udt-7 slot, the `+0x1AC` phase cluster), the
replication slot followed from `+0x1A8`, the activity-client world pool, and the full `0x504`-byte
value image (`--dump-image` writes it as bare hex; it REFUSES when the range is uncaptured).

* **Port provenance:** promoted from the lifetime-phase lane's `/tmp/rederive.py` prototype.
  Golden diff EXECUTED on `runs/snapshots/rung36-biped-chain.snap`: stdout byte-identical, image
  hex byte-identical (1284 bytes). The registry walk carries the SIGNED stride and the
  sign-extended head mask - a zero-extending port returns a plausible wrong base.
* **A missing range is NOT a zero.** Every fact is either derived or printed as `NOT CAPTURED`
  with the wanted address and length, and the process exits 1. `rung36-in-world-moonbase.snap`
  does not carry the datum-registry heap: the prototype died in a raw `struct.error` there; the
  subcommand names all 6 missing facts. Facts go to stdout; provenance (reach tag, the
  run-the-controls pointer) goes to stderr.
* **Controls:** `d1-replay rederive <snap> --controls`, 5 rows, exit 2 on FAIL - three known
  answers (schema tags `0x808004B1`/`0x80800CF7`, the setter quad `{0x8080039D, ord 2, obj
  0x27FA2003}`, the value schema `0x808008A3` at slot `+0x0C`) and two INVERTED rows (a
  phase-byte overlay must change the output; a withheld range must refuse loudly). 5/5 exit 0 on
  `rung36-biped-chain.snap`; 4 FAIL exit 2 on the heap-less snapshot - which is the point: a
  snapshot that cannot source the facts is not citable for them. Plus 4 `#[test]`s on synthetic
  memory. ⚠ The controls always use the canonical datum regardless of `--datum`.

---

## 8b. The round-3 static tool roster - tracked, controlled, and until now undocumented

Salvaged 2026-08-11 while retiring `historicaldocs/ingress/`. These 18 tools were integrated into `capture-rig/`
on 2026-08-07 (each verified `--help` clean and, where it has one, `--self-test` passing) but were
named in **no** thematic doc, so a session looking for them had to already know they existed. They
are all offline, read-only, and take `--help`.

| tool | answers |
|---|---|
| `capture-rig/demonware_file_atlas.py` | the Demonware source-file/function atlas. **Keeps its underscore name**: three other tools import it as a module, and they resolve it by co-location (`sys.path[0]`), so renaming it breaks them. |
| **ARCHIVED** `historicaldocs/python-capture/p2p-static-atlas.py` | the P2P call graph, chokepoints and diagnostics. Successor: `d1-eboot xref`/`atlas`. |
| **ARCHIVED** `historicaldocs/python-capture/activity-launch-static-atlas.py` | the same sweep scoped to the activity-launch branch. Successor: `d1-eboot xref`/`atlas`. |
| **ARCHIVED** `historicaldocs/python-capture/activity-host-contract.py` | regenerated the Activity Host message contract; the contract is now `design/activity-host.md` and the `d1-activity-host` crate. |
| `capture-rig/shadps4-launch-api-audit.py` | which platform APIs the launch branch calls, and what shadPS4 does with each |
| `capture-rig/shadps4-p2p-gap-audit.py` | the same question for the P2P surface (this is the audit behind the `sceNpMatching2` closure) |
| `capture-rig/elimination-content-scan.py` | Elimination-specific content strings and definitions |
| **ARCHIVED** `historicaldocs/python-capture/package-symbol-inventory.py` | which `.pkg` files carry symbol tables. Successor: `d1-pkg stats` / `d1-pkg entries`. |
| `capture-rig/content-dependency-graph.py` | activity to transitive content closure |
| **ARCHIVED** `historicaldocs/python-capture/build-reference-catalog.py`, `build-launch-catalogs.py` | regenerated the bulk JSON+Markdown catalogs; that derived output was never tracked and the catalogs themselves are retired with `historicaldocs/ingress/`. |
| `capture-rig/project-source-audit.py` | audits our own tree's coverage against the client surface. ⚠ Its shipped snapshot is stale; regenerate before citing. |
| `capture-rig/activity-launch-trace.py` | the launch trace contract; reads its `launch_knowledge.json` sibling by relative path, so the two must stay co-located |
| `capture-rig/p2p-corpus.py`, `multi-trace-correlator.py` | the P2P packet corpus and cross-trace correlation |
| `capture-rig/six-player-fixtures.py`, `single-participant-fixture.py`, `two-instance-layout.py` | deterministic fixture and layout generation. ⚠ `two-instance-layout.py` is superseded and carried a wrong port plan **with a passing self-test**; banner is in the file. |
| `capture-rig/send-receive-ledger.py`, `verify-client-bap-probe-sites.py` | the send/receive ledger and the probe-site verifier |

⛔ **Six of these were ARCHIVED on 2026-08-11**, per `design/capture-rig-audit.md` §1's deletion
manifest (zero live referrers each, successor named): `activity-launch-static-atlas.py`,
`p2p-static-atlas.py`, `package-symbol-inventory.py`, `build-launch-catalogs.py`,
`build-reference-catalog.py` and `activity-host-contract.py`. They are **not deleted** - they now
live in `historicaldocs/python-capture/` (untracked, on disk), so their control discipline is still
readable. Their rows below are marked ARCHIVED and name the successor. Use the successor.

⚠ **Two co-location constraints, both load-bearing** (a rename or a move breaks them silently):
`demonware_file_atlas.py` is imported by `elimination-content-scan.py` (hard `ImportError`) and,
guarded, by the two static atlases; and three tools read a sibling data file by
`Path(__file__).with_name(...)`, so the data must keep both its name and its directory.

## 9. Dead claims

Banners override bodies; nothing below may be quoted as live.

- "The `__FILE__` xrefs bracket each file's code range" (ATLAS §3) - **measured false**: 124/125
  spans overlap; only direct references (355, exact) are recorded.
- "Type names are proven unrecoverable" - **over-broad**: true of the runtime hash registries,
  false of C++ class names (RTTI yields 2,633).
- "35,834 is the function count" - false; 11,472 FPO call-target functions besides, and the
  earlier 11,446/47,280/473 figures are stale (11,472/47,306/495).
- `xref`'s `fn` column as nearest-prologue attribution - defect, fixed 2026-08-11; it produced a
  false "this dispatch is unreachable dead code" publication.
- "`~` means `name_source = string-ref`" (CLAUDE.md trap #2, N5's scoring scheme) - false; `~`
  marks four classes and the rule would discard 995 NID-verified import names.
- ghidra.md §3.1's argument "anything that treated `~` as weak would drop good names and keep bad
  ones" - **retracted**: its load-bearing class, `schema-tag`, scores 0/495; and "thunk ...
  skipped with it" is refuted by 6 thunks carrying applied `~schema_*` identities via Ghidra's
  native thunk propagation. (The *practice* - filter on `name_source`, never on `~` - stands.)
- `schema-tag` as an identity ("this code encodes/decodes object `0x80xxxxxx`") - refuted 0/495;
  the reference is `[C]`, the tag claim is not; the `0x80552cXX` cluster is SCE_NP error space.
- ghidra.md §10.4 "committing a signature costs call-site arguments" - refuted (no-signature and
  committed arms md5-identical); the remedy (commit callee prototypes, +81%) was right while the
  diagnosis was wrong.
- "`0x559d30` is the ICF twin of `0xb32410`" - it is a thunk computing
  `name_of(this->vfunc[0]())`; "same body in the corpus" != "same function".
- N4's "the 94.6% that are right" - deleted; >= 5.4% is a lower bound on one error class and its
  complement is unmeasured.
- "The 0.85 propagation floor is derived" - it is inherited from a pre-existing regression control;
  the k-inversion runs the argument backwards.
- "Execution-derived names are `[C]` by construction" - refuted: 17/17 holdout artifacts.
- "3 new message ids" / "31 of 33 match RTTI" / the executed 33-string recovery - prior art
  (`historicaldocs/design/message-catalogue.md` §3 has all of it; the executed version returned 32, strictly
  worse; the static `reloc --slot` command returns all 33).
- Roadmap §2.1's "imports unlock 54% of what is left" - measured +995 names; the middleware-zoning
  and `this`-flow items are killed, not pending.
- Full colon-path subsystem propagation (60.8%) and RTTI class propagation along call edges
  (20-26%) - retracted/killed; controls pin both so they cannot return.
- `capture-rig/eboot-callgraph.tsv` - fails 4/11 checks; regenerate with `d1-eboot callgraph`
  (152,686 edges).
- `d1-replay --help`'s blanket "It CANNOT capture registers" - false since build 3; two modes,
  stated per result.
- "20/20 replay controls" quoted as twenty independent axes - ~16 can fail independently; one leg
  waits on the first trap-carrying capture.
- Lab patch T1's "`al` is already 0" justification - false; the guard returns true on not-found.
- The availability builder's extent "`0x260200-0x260CAD`" - ends near `0x2608B0`.
- A flat "45%" for the canary scale - must be ">= 45%" with the direction of error named.
- ATLAS Layer 4's sequencing *reason* ("bracketing pays immediately") - the advice (do source
  attribution early) survived; the reason did not.

## 10. Sources

| doc (post-consolidation location) | contributed |
|---|---|
| historicaldocs/design/ATLAS.md | the four-layer plan; §4's constraint ledger (incl. §4.2e, §4.4d, §4.2d.i); evidence and count rules |
| historicaldocs/design/eboot-atlas.md | the generated atlas: artifacts, name sources, controls, settled contradictions, the honest 59% |
| historicaldocs/design/eboot-atlas-roadmap.md | §0's holdout rule; the execution log; the killed-technique record; the stop condition |
| historicaldocs/design/ATLAS-NAMING-SHIP-LIST.md | the ship/no-ship table; the holdout-integrity audit; the ceiling; the `[U]` ledger; the guards |
| historicaldocs/design/ghidra.md | install/addressing/atlas import/controls/corpus/structs; §5.4's rung-28 finding |
| historicaldocs/design/field-xref.md | the field instrument: algorithm, limits, five controls and their inversions, the worked result |
| historicaldocs/design/replay-oracle.md | the replay oracle: claim ladder, 20 controls and their banner, the canary, the content preset |
| status/MAP.md (stays live) | the access-mode table, reproduced in §1 |
| historicaldocs/ingress/round3/inventory/catalogs/TOOL-INVENTORY.md | the §8b roster |
| historicaldocs/ingress/round3/process/METHOD-RULES.md (+ METHOD-ADDITIONS.md) | §2.7's attribution rules; the sharpenings in §2.4 and §2.6 |
