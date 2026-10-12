# Instruments pointed at the LIVE client - canonical reference

**State (2026-08-11):** consolidated from five design docs, now retired to `historicaldocs/design/`.
Five instruments: `d1guest.py` (⭐ reads PORTED to `d1-live` 2026-08-11, byte-identical; journalled
pokes remain here), `d1-live` (the ported read side + hardware
watchpoints, controls 18/18: 10 watchpoint + 8 read-side), `d1-scope` (both planes behind one reader, controls 17/17 offline),
the shadPS4 guest-boundary network tracer (built, never run), and the guest-diagnostics patches
(built, never run). ⛔ The two biggest open `[U]`s: `d1-live`/`d1-scope` have never armed the real
47-thread shadPS4 client, and `--snapshot-on-hit` has never been pointed at it either.

**Contents:** §0 choosing the instrument · §1 `d1guest.py` · §2 `d1-live` · §3 `d1-scope` ·
§4 network tracer · §5 guest diagnostics · §6 open `[U]`s · §7 dead claims · §8 sources.

---

## 0. The two planes, and choosing the instrument

The live client is two planes, and the split is not negotiable
(historicaldocs/design/d1-scope.md §1):

| plane | question | mechanism | cost |
|---|---|---|---|
| **STATE** | *what is this field, right now?* | `/proc/<pid>/mem`, `O_RDONLY`, `pread` | cheap, unlimited, perturbs nothing; ⛔ cannot see a transition |
| **EVENT** | *which instruction stored to this address, at the instant it did?* | `PTRACE_SEIZE` + `DR0..DR3` | four debug registers on the whole machine; a lease you take, use, and give back |

⛔ **"Settle it by replay" has failed twice.** A transition is an event, not a state - you cannot
snapshot your way to one. The wider selection table (historicaldocs/design/live-watch.md §1):

| instrument | answers | structurally blind to |
|---|---|---|
| `d1-eboot xref` | which code mentions an absolute address | `mov [rbx+8], eax` - a register-relative store mentions nothing |
| `d1-eboot field-xref` | which functions touch `class+off`, with escapes | still static: it reports zero stores to `c_activity_loader+8` and is right to |
| `d1-replay` (Unicorn) | what a function does to captured memory | **the moment** - a snapshot is a state; a write is an event |
| UD2 runtime probes | which branch a *chosen* site took | needs the site already guessed; one bad guard yields an empty trace that reads as "the game never got there" |
| `d1guest.py` | what a field holds right now | transitions, and who caused them |
| **`d1-live watch`** | **the RIP of the store, on the first hit** | writes by threads created after arming; kernel/DMA writes |

⭐ **State the question before picking the tool.** The wrong one returns a clean, confident answer
to a different question.

---

## 1. `d1guest.py` - the state plane, and the runtime-state-access plan of record

> ⭐ **SUPERSEDED FOR READS, 2026-08-11. The read side now lives in `d1-live`.**
> `d1-live read|obj|tag|descriptors|syms|live|base` is a behaviour port of the subcommands below:
> **byte-identical stdout and identical exit codes**, executed over 88 command comparisons by
> `capture-rig/golden-diff-d1guest-vs-d1live.sh`. Everything in §1 about base recovery, the liveness
> ladder, the two-derivation registry cross-check and the refusals is therefore a description of
> **both** tools; the Rust one additionally has controls that can FAIL, because
> `d1-live __mem-target` can be told to be wrong on purpose.
>
> **What is still Python-only: `poke` (and `watch`, whose Rust answer is `d1-live watch`, a strictly
> better instrument for the same question).** A live write is the one operation where a half-ported
> tool is dangerous, so it was left out of that port on purpose and gets its own reviewed change.
> ⚠ **Live-parity gate OPEN:** the whole comparison ran against a fixture process, because no
> emulator was up. The command pair that settles it on the next boot is in `status/harness.md`'s
> Next list.

**Status: adopted 2026-08-07.** The single sentence the plan exists to enforce:

> **The client is an ordinary Linux process. Anything that only exists while it runs is directly
> observable - and the reason we keep failing to observe it is not capability, it is that the
> observation has no control on it.**

`capture-rig/d1guest.py` replaced five ad-hoc scripts (`peek-guest.py`, `dump-registry.py`,
`dump-descriptors.py`, `inject-roster.py`, `find-in-guest.py`) that each carried its own base
recovery, `Mem` class, and hardcoded constants - the shape in which a retraction survives
(historicaldocs/design/runtime-state-access.md §1). It is importable (`import d1guest`): `Image`,
`Proc`, `Guest`, `Symbols`.

```
d1guest.py base | live | syms [pat] | selftest
d1guest.py read <sym|0xvaddr> [--len N] [--deref] [--struct NAME] [--as hex|u32|u64|ptr]
d1guest.py obj FAM:TYPE[:IDX] [--key 0x…] [--struct NAME]
d1guest.py tag 0x80801825
d1guest.py descriptors [--watch]
d1guest.py watch <sym…> [--interval S] [--for S]
d1guest.py poke <sym|0xvaddr> <value> --type u64 --experiment NAME --confirm
```

⚠ `d1guest.py` resolves its symbol file relative to the CWD - run it from the repo root.

### 1.1 Base recovery - three controls, all printed, or no read

⚠ **A WRONG BASE YIELDS PLAUSIBLE-LOOKING BYTES RATHER THAN AN ERROR.** And an anchor must not be
a UD2 tracer probe site (the tracer writes `0f 0b` over probes while armed - `0x586430` and
`0x583450` cost a run this way) nor a patch target (`patches/*.xml` rewrite code bytes; `Address`
there is `vaddr + 0x400000`).

Two anchors agreeing is necessary and not sufficient, so `d1guest.py` refuses to read unless all
three pass:

| # | control | what it catches |
|---|---|---|
| 1 | two independent anchors resolve to **exactly one** agreed base | a signature that occurs twice; a probe/patch-damaged anchor |
| 2 | a **third** address compares equal to the file **by direct read, not by scan** | wrong base arithmetic; a partially-relocated image |
| 3 | **negative control** - the same window shifted one byte must **differ** | a degenerate window, which would make #2 pass for free |

Recovery is cached against `(pid, /proc/pid/stat starttime, eboot sha256)`; controls 2 and 3 re-run
on every invocation. Anchor scanning restricts itself to resident executable mappings by default
(249 MiB, versus 17 GiB full sweep) and says so; `--full-scan` widens it.
⚠ shadPS4 reserves ~87 TiB of sparse address space and **the guest heap lives inside those
reservations** - a scanner that skips mappings over `0x40000000` once searched 0.01% of memory
while reporting a confident "0 hits". A heap-value search is `find-in-guest.py`'s job, not this
tool's.

Two more gates: `capture-rig/guest-symbols.json` pins the eboot's SHA-256 (a vaddr measured against
a different build is plausible-looking-but-wrong input), and the tool parses the program headers per
segment - `BIAS = 0x4000` is right for the two LOAD segments *by coincidence* - which also answers
"is this address file-backed or BSS?". Every address in the symbol file carries a `source` and a
`confidence`; struct layouts and the family/slot table live there too, and the tool derives every
registry element address twice (runtime entry table vs static table) and refuses to report a read
if they disagree. **Measured: 18/18 families agree.**

### 1.2 Timing - a liveness ladder, never a sleep

Registry **objects are cleared on connection teardown while descriptors survive at state 3** - a
read after a disconnect shows every object empty, indistinguishable from "our delivery never
worked" (that is how §26.63 wrongly concluded the character object never lands). Printed on every
command:

| level | predicate | licenses |
|---|---|---|
| **L0** | process exists, `/proc/pid/stat` starttime unchanged | nothing - but its absence is an error, not a zero |
| **L1** | the base passed all three controls | reading anything at a guest vaddr |
| **L2** | `*(object_registry_ptr) != 0` | BSS/global reads |
| **L3** | family-4/type-0 element's key equals the account SOID we serve | reading or writing registry object contents |

Commands that touch object contents demand **L3** and exit non-zero below it; `--allow-dead`
overrides loudly. A read that begins alive and finishes after death raises `Dead` rather than
returning short bytes. **A read after death is an error, not a zero.**

### 1.3 `selftest` - not optional

```
SELFTEST (known results, ATLAS §4.4d):
  PASS  family/slot table (eboot-map §3) vs runtime entry table: 18/18 families agree
  PASS  roster stride: family 3/0 stride = 0xa0 (expected 0xA0, §26.61)
  PASS  file-backed read == file image: 256 bytes at 0x1324060
  PASS  object_registry_ptr is BSS: 0x26a2040 is beyond filesz - no file read can answer it
  PASS  relocation control (boot_state_names): file all-zero=True, live all-zero=False
SELFTEST: 5/5 PASS
```

**A claim made with this tool that was not preceded by a passing `selftest` in the same session is
not a claim.**

### 1.4 Hop mechanics - the ranking

> **wire-authored > config/boot-to-X > RAM poke > controller driving** - and a hop's reach tag is
> inherited by every finding downstream of it.

* **Wire-authored** `[best]`: reach `synthetic-minimal` at worst, through the client's own decoder
  over its own transport. Where a hop is achievable either way, take this one.
* **Config / boot-to-X** `[good]`: `static-client-inference` to set up. ⚠ The leading candidate was
  retired - see §7 dead claims (`0x263D0F8` steers `bootflow:queuing`, not `profile_setup`).
* **RAM poke** `[weak - identification only]`: reach `synthetic_ancestor` twice over. **A poke can
  IDENTIFY a gate. It can never SATISFY one.** You expect the crash and report it in the same breath
  as the advance. Poking a value the client itself writes, to a value it itself writes, is
  comparatively safe; poking a value the client obtains by resolution (pointer, handle, SOID) is
  the §26.75 trap - poke the *input* to the resolution, never its output. `poke` therefore requires
  `--experiment <name>` and `--confirm`, prints the taint banner, and appends to
  `runs/guest-writes.jsonl`.
* **Controller driving** `[last resort]`: not evidence, transport only. Adjust *within* a screen,
  never get *to* one. **`virtual-pad.py`'s `axis` has no positive control anywhere in this
  project.**

⚠ **shadPS4 has no savestate/snapshot subsystem** - "snapshot anything reached once" is not
available; the weight falls on the other mechanics.

### 1.5 First use - the five reproduced results, and what it found

Run `20260807-190125-runtime-state-bss`, liveness L3, selftest 5/5 PASS. Beyond the selftest's five
reproduced knowns, the first session produced
(historicaldocs/design/runtime-state-access.md §6):

* `[C]` `observed-live` - `0x2A8570()` returns 3; `0x2A85B0()` is never called (`0x78CD00`
  short-circuits unless `kind == 2`). The pair is (3, 0).
* `[C]` `static-client-inference` - the accessors **ignore the object pointer**: `0x263D0F8` is 16
  bytes of zero (the `c_signon_manager` singleton); the values are fixed globals at `0x263D12C`,
  `0x263D130`, `0x263D2E8` (+ `0x263D340`). Reading `0x263D0F8` answers nothing.
* `[C]` **Retraction forced** - `0x79EB80`/`0x79EB90`/`0x79EC00`/`0x79EC20` belong to
  `bootflow:queuing` (state 17), not `profile_setup`: proven by relocated vtables read live
  (typeinfo `0x192EB90` tables at `0x192EAA0`/`0x192EB18` contain neither), a whole-process search
  for qword `0x80079EC20` (exactly ONE hit at `0x192EC18`, 17,125 MiB searched), and that vtable's
  typeinfo resolving to `"c_world_controller_state_bootflow_queuing_handler"` (control: `0x192EB90`
  resolves to the profile_setup name). Cheapest falsifier: if `0x79EC20` were profile_setup's
  update, `kind == 3` would request state 18 every frame - and the client sat unmoving for minutes.
* `[C]` The real `profile_setup` update method is **`0x79E830`** (vtable slot 7), a 7-entry jump
  table on `[handler+0x38]`; its three goals `0x16/0x15/0x17` = 22/21/23 match `eboot-map.md` §7's
  `screen bounds -> brightness -> subtitles` derived from an independent string table.
* ⭐ `[C]` `observed-live` - **THE HOP**: handler instance found by vptr search (`0x192EAA0`,
  exactly one hit) at BSS **`0x26B4070`** (stable across runs; secondary vptr `0x192EB18` at
  `+0x30` matches offset-to-top `-0x30`). Control first: no input 20 s, substage unmoved. Then
  `d1guest.py poke profile_setup_handler 7 --type u32 --offset 0x38` - within one frame, no input:
  `profile_setup -> prepare_for_orbit -> character:signin -> character:prologue -> cleanup`,
  gate-check **10/10 PASS**, zero `error.*`, BAP 48,097 B (previous best 12,429 B). And the crash
  in the same breath: `0xB45C78`, the already-known fireteam frontier - the hop delivered us to the
  wall, it did not cause it. **Reproduced 2/2** on an independent boot from a different substage.
  Both writes are in `runs/guest-writes.jsonl`. ⚠ Reach: `synthetic_ancestor` by injection - a
  transport to the frontier, not a finding; the wire route (profile display block, schema
  `0x8080181A`, field [2]) is still better and still open.
* ⭐ `[C]` `observed-live` - the profile blob is **POPULATED**, not scratch:
  `*(void**)0x26A3790 = 0x205015e70`, `+0x00 = 0xD100000000000001` (the account SOID our server
  invented and delivered in the 503 body - the client cannot have manufactured it), `+0x18 = 0`
  (`profile_setup_complete`, consistent with the wizard running), `+0x24 = 08`, `+0x25 = 06`,
  `+0x30 = 0xD100000000000201`, `0x30`-stride records with `0xFFFF` sentinels. `[U]` whether
  `+0x18` specifically maps to a settable profile-schema leaf - a lookup, not a search.
* ⚠ `[U]` - the contested `0x263D0F8` shape is NOT settled: two published readings disagree (one
  singleton with a status struct at `+0x1F0`, versus `0x263D2E8` as a base with `+0x184` =
  `id_list`), and the live bytes fit a **third** shape - `0x263D2E8` looks like element [1] of a
  0x54-stride array (`{u32 status; char lang[8]; char lang2[8]; char build[…]}`), with `0x263D278 =
  2` plausibly a count. Also `[U]`: `+0x248` is **not a small enum at runtime** - it read
  `0x21B_4D6936ED` then `0x21B_4D59267F` across two boots (a timestamp/tick counter); it is only
  read when `state == 2`, so the enum claim is unfalsified but must not be assumed.
* Free riders: `character_selection_node` decodes at profile_setup time (`slot_b =
  0xD100000000000201` is `[U]` - not one of the four SOIDs we serve); vtables and relocated pointer
  tables are readable live with relocations applied, so a class can be named by walking
  `typeinfo+8`.

### 1.6 Migration ledger (deliberately not forced)

`peek-guest.py` fully subsumed · `dump-descriptors.py` subsumed · `dump-registry.py` tag resolution
subsumed, its schema field-tree walker not (keep its sanity gate verbatim) · `inject-roster.py`
writes subsumed, its account-SOID interlock is now L3 · `find-in-guest.py` **not** subsumed - it is
the only tool that must scan the sparse reservations.

---

## 2. `d1-live` - hardware watchpoints on the live client

**State:** ⭐ built and controlled 2026-08-09, extended 2026-08-10 (build 3). **`d1-live controls`:
10/10 PASS, exit 0** (was 7/7; the three new ones are `fatal-signals-disarm`,
`a-failed-arm-is-still-disarmed`, `disarm-is-bounded-and-multithreaded`), each inverted by breaking
the thing it protects; 10 unit + 11 integration tests green. Sibling `d1-replay controls <snap>` is
**20 PASS / 0 FAIL / 0 SKIP** on six snapshots spanning three eras of the format. Since build 3
every hit carries the trapping thread's whole integer register file, and
`d1-replay capture --snapshot-on-hit` / `replay --use-captured-regs` carry it end to end.

**Where:** `d1-server/crates/d1-live` - `cargo build --release -p d1-live`, then
`d1-live watch | base | controls`. **`controls` exits 2 on any failure; do not cite output from a
binary whose controls did not pass.**

The one-paragraph version: shadPS4 executes guest code natively on x86-64,
`kernel.yama.ptrace_scope` is 0, so a CPU data-write breakpoint in `DR0` catches the guest's own
store and hands back its RIP - in one run, with no candidate list.
`d1-live watch --addr '[v:0x267AF08]+0x8'` is the whole technique.

### 2.1 Mechanism, and the traps in it

1. **`PTRACE_SEIZE`, not `PTRACE_ATTACH`** - attach without stopping the operator's live client.
2. **Arm every thread** - debug registers are per-thread; `watch` enumerates `/proc/<pid>/task`
   (111 threads on shadPS4 when written) and arms each: SEIZE -> INTERRUPT -> wait -> POKEUSER ->
   CONT.
3. **`DR0 = address`, `DR7 = enable`** via `PTRACE_POKEUSER`. ⚠ **`LEN` is not monotonic**: `00`=1,
   `01`=2, **`11`=4**, **`10`=8** - asserted as a test, with the intuitive-but-wrong table asserted
   `assert_ne!`.
4. On `SIGTRAP`, `DR6` bit *slot* names our breakpoint; `PTRACE_GETREGS` gives RIP.
5. **Disarm** - `DR7 = 0`, `DR0..DR3 = 0`, drain, `PTRACE_DETACH`. Automatic in `Drop`, on
   `SIGINT`/`SIGTERM`, and at `--max-seconds`.

⚠ **The reported RIP is the instruction AFTER the store.** A data breakpoint is a **trap**
(delivered after the storing instruction retires); an execute breakpoint (`--rw exec`) is a
**fault** (RIP *is* the instruction). Quoting raw RIP as "the writer" names the wrong instruction.
`watch` decodes backwards and prints both, labelled: `unique` · `preferred: the only candidate that
writes memory` · `longest of several memory-writing decodes` (confirm with `d1-eboot disasm`; a
`longest…` reading is `[O]`, not `[C]`, until confirmed) · `AMBIGUOUS: no candidate writes memory`
(do not quote as a writer).

The base is not assumed: `d1-live` reuses the three base controls of §1.1 verbatim; `--no-base`
skips them and then requires every address absolute.

### 2.2 Address semantics - identical to `d1guest.py`, deliberately

⚠ Paid for with a retracted causal theory: **only eboot vaddrs get the base. Anything read out of
memory is already absolute.**

| you write | means |
|---|---|
| `0x267AF08`, a `guest-symbols.json` name, or `v:0x267AF08` | an eboot vaddr -> `base + N` |
| `--abs 0x204eda2f8`, or `a:0x204eda2f8` | an absolute host address, used as-is |
| `[…]` | dereference a u64 through the live process |
| `+`/`-` after a term | a plain offset - `[v:0x267AF08]+0x8` never adds base twice |

**A heap address must be `--abs` (or `a:`).** Every resolution is printed before anything is armed.

### 2.3 The ten controls

`d1-live controls` is offline - every control drives a throwaway child (`__writer`), never the
emulator. Each was inverted before shipping and failed only its own control:

| control | axis it protects |
|---|---|
| `debugreg-offset` | `offsetof(struct user, u_debugreg)` is **measured**, not transcribed - the acceptance pattern must be bounded on both sides and show the DR4/DR5 `EIO` hole (840 EIO · 848-872 ok = DR0-DR3 · 880/888 EIO = DR4/DR5 · 896/904 ok = DR6/DR7 · 912 EIO). ⚠ `sizeof(struct user)` is **928**, not 912; "sizeof minus 64" would derive 864 and be confidently wrong |
| `catches-the-writer` | caught RIP must equal store + insn_len, from the child's own inline-asm labels |
| `address-specific` | a decoy 16 bytes away stays silent while the same child then trips the real target |
| `registers-at-the-hit` | the register map on a `Hit` is the trapping thread's live integer file - complete, keyed by name, holding the store's own pointer operand; an all-zero map FAILs; the never-stored decoy in a register scores DEGENERATE, not PASS |
| `disarm-is-load-bearing` | an armed, abandoned child **dies** (leg A), so disarming matters |
| `fatal-signals-disarm` ⭐ | **any catchable terminating signal** leaves through a verified disarm - inversion: removing `SIGHUP` from the set FAILed ("SIGHUP with the handlers installed still KILLED the child") |
| `a-failed-arm-is-still-disarmed` ⭐ | an arm that fails after its pokes landed stays on the disarm list; `attach` refuses and the child ends alive with `DR7 == 0` |
| `disarm-is-bounded-and-multithreaded` ⭐ | the capture-time disarm is bounded, measured across **four** threads; a zero bound makes the capture REFUSE |
| (plus the two remaining of the original seven, exercised in the same suite) | |

### 2.4 ⛔ SAFETY - the NEVER-SIGKILL rule, and its correction

`[C]`, measured by control, re-measured every run: **a tracer that exits while `DR0` is armed KILLS
its target.** The abandoned control child dies of `SIGTRAP` (status `0x85`, core dumped) every
time - on this kernel a ptrace hardware breakpoint outlives the implicit detach.
=> **NEVER `SIGKILL` `d1-live watch` while it is pointed at shadPS4.** If the kernel ever flushes
on detach, the control reports its own inversion as DEGENERATE rather than banking a free pass.

⛔⛔ **CORRECTED 2026-08-10 (build 3): "everything except SIGKILL is safe" WAS FALSE.** What is
safe is the **caught** set, and until build 3 that set was `SIGINT`/`SIGTERM` only. `SIGHUP` was
measured lethal 2 of 2 (toy child, never the emulator): default action terminates without
unwinding, so no `Drop` runs, the kernel auto-detaches with `DR7` set, and the client dies of
`SIGTRAP` on its next write - within a window up to `--hit-timeout` (120 s default), and the normal
workflow is two terminals, so a dropped ssh session is exactly how it fires. As built:

| set | signals | handler |
|---|---|---|
| `GRACEFUL_SIGNALS` | `HUP INT QUIT TERM USR1 USR2 ALRM VTALRM PROF XCPU XFSZ` (11) | leave through the verified disarm; ⭐ a **second** signal runs `emergency_disarm()` and `_exit`s - impatience gets a safe landing before someone reaches for `kill -9` |
| `FATAL_SIGNALS` | `SEGV BUS ILL FPE ABRT SYS` (6) | raw-syscall `emergency_disarm()`, restore `SIG_DFL`, re-raise; `SIGABRT` covers Rust's allocation-failure `abort()` |
| deliberately NOT handled | `SIGPIPE` | Rust leaves it `SIG_IGN`, so `println!` panics on EPIPE, and a panic unwinds and disarms |
| cannot be handled | `SIGKILL`, `SIGSTOP` | ⛔ still the one true hazard |

⚠ Only the `SIGHUP` leg is an automated control; the hand-run `SIGABRT`/`SIGSEGV` pairs (child
ALIVE with handlers, DEAD with `--no-handlers`) are `[O]`, not `[C]`.

⛔⛔ **KNOWN DEFECT, UNFIXED:** `emergency_disarm()` returns how many tids it cleared and **both
handlers discard it** - a rescue that cleared 0 of 47 threads is byte-identical in output to one
that cleared 47 of 47. The honest closer (a raw async-signal-safe `write(2)` receipt to fd 2, ~15
lines) is **NOT BUILT**. Until it is: after any crash of an armed `d1-live`/`d1-replay`, **verify
the client yourself** before continuing.

⛔⛔ **Second unfixed defect:** the terminal `Watcher::disarm` (from `Drop` / the graceful path)
detaches a thread it could NOT prove clear, prints a loud `⛔ tid N could NOT be proven disarmed …
Detaching anyway … if it takes SIGTRAP it will die` - **and the controls tally still prints
`PASS (10/10 pass)`, exit 0, underneath it.** Operator rule until fixed:
`grep -c "could NOT be proven disarmed"` the output of any armed run and treat non-zero as a
failure whatever the tally says.

⛔ The lethal bug the fourth control caught: the first disarm cleared `DR7`, read it back zero,
detached - and the child still died ~1 in 3, because a `SIGTRAP` already queued into a group-stop
is never reported and detach leaves it pending. The fix is a **verified drain** via
`/proc/<tid>/status` `SigPnd`/`ShdPnd`: 12/12 clean after, ~2/6 before. The generalisable lesson:
*the read-back proved the register was clear, which was true and not the property that mattered* -
and the earlier `let _ =` swallowed failed pokes silently; `disarm` now shouts per-thread failures.

⚠ **A `Watcher` owns `waitpid(-1)` for its whole process** - two in one process steal each other's
notifications (false "no hit") and exit statuses; the tests serialise. Stops for unarmed tasks are
counted `foreign_stops` and left alone.

### 2.5 ⭐⭐ The argument-capture idiom (`--rw exec`)

The highest-yield move: not "did X run" but *"X ran, and here are the arguments"* - on x86-64 SysV
the first six integer arguments are `rdi rsi rdx rcx r8 r9`, and `d1-live` prints the register file
at every hit.

```sh
# 1. Controls first. Exit 2 means do not cite anything that follows.
./target/release/d1-live controls

# 2. ARM BEFORE THE TRIGGER. --len 1 is MANDATORY for --rw exec.
./target/release/d1-live watch --rw exec --addr 0xba3b00 --len 1 --max-seconds 40

# 3. In a SECOND shell, poke whatever should cause the call, then read the regs off the hit.
python3 -B capture-rig/d1guest.py poke <trigger> 1 --type u8 --abs --experiment "..." --confirm
```

Real hit, 2026-08-10, `FUN_00BA3B00` (the player-add sweep): `rdi=0x204fb34a0` was the
simulation-players object `Q`, `rsi` the dirty flag, `rbx` the machine record - three predicted
identities confirmed by the CPU in one stop. A second watch on `FUN_00BB3FD0` returned
`rdi=P, rsi=0`. This is the only instrument here that answers *"what was actually passed"* -
`field-xref` cannot see runtime register values, and `d1-replay` executes with **your** values.

⭐ **A NO-HIT is a first-class result** - `FUN_0090D1E0` armed 35 s with every precondition
satisfied returned no hit, localising the break to one function boundary. But repeat the tool's own
limit when citing: it is evidence about *the threads armed at start*; threads created afterwards
were not armed (`--follow-clone`). It is NOT evidence that nothing ever calls the address.

#### ⛔ THE GOTCHA: `--rw exec` REQUIRES `--len 1`, and it refuses without it

```
d1-live: an execute breakpoint must have --len 1 (the CPU requires LEN=00b for R/W=00b)
```

An x86 debug-register rule, not a tool preference. **It cost one armed run on 2026-08-10** - the
watch refused, the trigger was poked into a session with nothing armed, and that fire was wasted.
The refusal is correct behaviour: read it, add `--len 1`, re-arm - and re-fire the trigger, because
the first one is gone.

Three more things that bite:

* **Arm BEFORE the trigger, and confirm `ARMED` in the output before poking.** Poking first means
  the hit never happens and the run reads as a clean negative.
* ⚠ **Every trigger is one-shot in practice.** `--count 1` (the default) disarms after the first
  hit, and the client's consume-and-clear semantics usually eat a poked flag. Budget one arm per
  fire.
* ⭐ **Signal forwarding is normal.** A healthy run reports e.g. `forwarded 143x signal 4` and
  `forwarded 13848x signal 11`. A run with **zero** forwarded signals is the suspicious one.

### 2.6 ⚠ What this instrument cannot do

1. **Threads created after arming are not armed** unless `--follow-clone` - a miss from that cause
   is indistinguishable from "nothing wrote it"; every report states how many threads were armed,
   and the zero-hit summary says *"This is evidence ONLY about the N thread(s) armed at start."*
2. **Four slots** (`DR0..DR3`).
3. **Alignment is mandatory** - `--len 8` needs 8-byte alignment; `watch` refuses rather than
   rounding down to different bytes.
4. **It perturbs** - one brief stop per thread to arm, one stop per hit; bound with `--count`,
   `--max-seconds`, `--tid`.
5. **Signals are forwarded, never swallowed**; a `SIGTRAP` whose `DR6` does not name our slot
   counts as `unattributed`.
6. **It sees CPU stores only** - not kernel writes into user memory (`read(2)` into a buffer, DMA).

### 2.7 The worked example

Question: who writes `c_activity_loader+8`? `field-xref` reports 24 tracked accesses, zero stores -
and is right (register-relative writes escape it).
⛔ Correction 2026-08-09: this `[U]` was since **closed statically** by reading `field-xref`'s
escape rows one indirection down - the writer is `FUN_00341500 @ 0x341798`
(`memcpy(block+0x000, block+0x148, 0x145)`) => `historicaldocs/design/activity-loader-writer.md`. That sharpens,
not weakens, the case: static xref took four functions of reading; the CPU names it in one stop -
and a known answer is exactly what a new tool's first live run should reproduce.

```sh
cd d1-server && cargo build --release -p d1-live
./target/release/d1-live controls          # must print PASS and exit 0
./target/release/d1-live base              # optional: base controls against the running client

./target/release/d1-live watch \
    --pid "$(pgrep -f '[s]hadps4 -p ' | head -1)" \
    --addr '[v:0x267AF08]+0x8' \
    --len 8 --rw write \
    --count 3 --max-seconds 240 --follow-clone
```

⛔ **Ctrl-C to stop it. Do not `SIGKILL` it.** It self-disarms at `--max-seconds` regardless.
Reading a hit: quote the **writer**, not RIP; feed `v:` addresses to
`capture-rig/eboot-atlas.py --addr`; zero hits is not "nothing writes it" - read the summary line's
armed-thread scope. Follow-up (the field is a pointer to a heap block, so watch the pointee):

```sh
./target/release/d1-live watch --addr '[[v:0x267AF08]+0x8]+0x0' --len 8
```

### 2.8 `--snapshot-on-hit` - pairing the trap with a memory image

⭐⭐ Built 2026-08-10 (build 3), as **`d1-replay capture --snapshot-on-hit EXPR`** (not a `d1-live`
flag - the dependency runs `d1-replay -> d1-live`; `d1-live watch --snapshot-on-hit` is accepted
and REFUSED, naming the real command). ⛔ **It has never been pointed at shadPS4** - every number is
from toy children; `[U]` whether the client's guest threads tolerate the stop is not closeable
offline, and `Coherence` is instrumented so the first live run reports its own cost.

The design turns on one measured fact: **clearing the debug registers costs 0.68 ms for 111
threads, and a tracee left stopped with nothing armed SURVIVES its tracer's death** (3 of 3
measured legs) - so the capture runs with nothing armed anywhere; the lethal window is only the
trap + clear (steps 1-2).

The operator sequence, exactly (two shells; the two-shell shape is why `SIGHUP` had to be handled):

```sh
# ── SHELL A ──────────────────────────────────────────────────────────────────
# 0. CONTROLS FIRST, BOTH CRATES. Exit 2 anywhere means nothing below may be cited.
cd d1-server
cargo build --release -p d1-live && cargo build --release -p d1-replay   # -p, NOT the workspace
./target/release/d1-live   controls                                      # want 10/10, exit 0
./target/release/d1-replay controls ../runs/snapshots/rung36-biped-chain.snap   # 20 PASS / 0 FAIL

# 1. Confirm the address resolves in the LIVE client before arming anything.
#    ⚠ d1-replay read reads a SNAPSHOT, not a process; the live read is d1guest's.
#    ⚠ d1guest resolves its symbol file relative to the CWD - run it from the repo root.
( cd .. && python3 -B capture-rig/d1guest.py read '[v:0x267AF08]+0x8' --len 8 --as ptr )

# 2. THE ARM + CAPTURE. Resolves, plans, REHEARSES the in-stop read against the live
#    client, refuses above the hard cap BEFORE arming, then arms and blocks on the trap.
./target/release/d1-replay capture \
    --out ../runs/snapshots/task9-dispatch.snap \
    --snapshot-on-hit '[v:0x267AF08]+0x8' --hit-rw write --hit-len 8 \
    --stop-scope state \
    --hit-timeout 120 --disarm-deadline-ms 1500 \
    --run-dir ../runs/<RUN-NAME> --state "task 9 dispatch, rung 34"

# ── SHELL B, ONLY AFTER SHELL A HAS PRINTED ITS "armed N thread(s)" LINE ─────
python3 -B capture-rig/session-params.py --launch-poke --activity-id 407
```

⚠ For an EXEC breakpoint it is `--hit-rw exec --hit-len 1` - **`--hit-len 1` is mandatory** and the
tool refuses without it. ⛔⛔ **While shell A is armed: Ctrl-C safe, SIGTERM safe, closing the
terminal safe (since build 3), and `SIGKILL` KILLS THE CLIENT.** If it looks hung, signal it a
second time - that path runs `emergency_disarm()` and `_exit`s deliberately.

Receipts to read, in order: `rehearsed at N ms` -> `REFUSING to arm` (⭐ good news, no trigger
spent) or `armed N thread(s)` (**now** fire shell B) -> `DISARM … N of N proven clear in X ms` ->
⛔ `could NOT be proven disarmed` (check the client before continuing) -> `TRAP … memory read over
N ms while M other threads ran` (the artifact's own caveat).

Key numbers and rules (historicaldocs/design/live-watch.md §6a):

* Scopes: `minimal` ~0.1 MiB / ~0.75 ms; **`state`** (default) ~22.7 MiB / ~4.6 ms (register roots,
  stack below rsp, eboot seg1 = every non-executable LOAD segment); `wide` up to the budget. Budget
  64 MiB / 16 ms (one 60 fps frame); **hard cap 512 MiB / 250 ms, refused BEFORE arming** so no
  trigger is spent. eboot seg0 (code) is read after the resume and the assumption is **checked
  every run** by the phase-split control (patches and the UD2 tracer rewrite code at load).
* Client walls: mainloop block detector halts at **20,000 ms** (`RELEASE_VHALT`); AH lease
  20,000+500 ms.
* `--snapshot-on-hit` implies `--count 1` and refuses more. One arm, one trap, one artifact.
* ⛔ **This is not an atomic freeze** - one thread stops, ~110 keep running. Write "the trapping
  thread's registers, paired with memory read over the following N ms while N other threads ran",
  never "the state at the moment of the write". `Coherence::all_threads_stopped` is FALSE in the
  file by default; `--stop-threads all` is specified, `[U]`, and REFUSED.
* `--snapshot-when resumed` exists for wide captures and is not the default because its failure
  mode is silent: a freed-and-reused object under a captured `rdi` replays happily to a confident
  wrong answer.
* `Manifest::trap == None` means **NOT ATTEMPTED**, never "no registers". ⛔ No FP/SSE/AVX state is
  captured - "the integer register file", not "the registers".
* Compat: **18/18 archived snapshots still load** (`existing-snapshots-still-load`; the inversion
  made the field non-Option and all 18 failed at once - `#[serde(default)]` on an `Option` is a
  no-op, the control is the guard, not the annotation).
* Controls (each inverted 2026-08-10, exit 2): `capture-runs-unarmed` (DR7 read out of the child =
  0 with the disarm, `0x90303` without) · `panic-during-capture-is-survivable` ·
  `regions-do-not-overlap` (inverted on a copy of a real snapshot) ·
  `stop-budget-refuses-above-the-cap` · `phase-split-is-its-own-pair` (seg1 unchanged =>
  DEGENERATE, not PASS) · `registers-at-the-hit` · `existing-snapshots-still-load` ·
  `fatal-signals-disarm` · `a-failed-arm-is-still-disarmed` ·
  `disarm-is-bounded-and-multithreaded` · `the-trap-caveat-is-in-the-result` (a fabricated ATOMIC
  trap must print NO smear warning) · `trap-honesty-fields-round-trip` (`threads_total: None` must
  never decode as `Some(0)` - "0 of 0 kept running" is the atomic-freeze claim printed from a
  measurement failure).
* ⛔ Defect banner on that table: **`absent-is-not-empty` is listed and does not exist in code**
  (cannot exist until `replay --from-trap` does, also not built); three controls still drive a
  1-thread child where the client has ~47; a loud disarm failure does not fail the tally;
  `stop-budget-refuses-above-the-cap` and `phase-split-is-its-own-pair` are pure functions that
  cannot catch a wiring regression.
* What it buys (§6a.8): rung 34's task 9 - an exec breakpoint on the dispatch with
  `--snapshot-on-hit` yields the real arguments AND the content registry at that instant; the
  lookup can then be replayed offline, varying the served object, until it succeeds. That is
  *"which bytes would our responder have to serve"* - a question about server code, answered
  without a poke.
* ⚠ **Do the `minimal` run first and read the disarm line before ever asking for `state`.**

### 2.9 Provenance and taint

A watchpoint result is `[C]` about *the client we watched* and nothing more. If that client reached
its state over our own synthetic server - which today it always has - the finding is
**`synthetic_ancestor`**. Naming the instruction that wrote a field is a fact about the binary's
control flow (portable); naming *why* it ran is not. ⚠ Unlike `d1guest.py poke`, a watch is
read-only - a probe with no hop-mechanic taint of its own. That is the point of preferring it.

---

## 3. `d1-scope` - the live client as two planes, behind one reader

**Where:** `d1-server/crates/d1-scope` - ⛔ **its own workspace root** (own `[workspace]`, own
`Cargo.lock`); it does not build from `d1-server/`'s workspace, and the `d1-live` child it spawns
comes from the **other** workspace (`$D1_LIVE_BIN` overrides). Two builds are required before any
live use. Provenance of the whole lane: `static-client-inference` / `replayed-capture` only - it
never touched the emulator.

> **THE LIFECYCLE INVARIANT.** d1-scope may never leave a hardware watchpoint armed behind a
> process that is not alive to disarm it: at most one `d1-live` child at any instant, always
> launched with a bounded `--max-seconds` lease, stopped only by `SIGTERM` and only after waiting
> for the child's own confirmed disarm, and every path out runs that stop. **A tracer that dies
> armed kills its target** - measured, every time.

⛔⛔ **The BUILD-2 lifecycle review returned `DEFECT`, and the verdict does not revert to
`SOUND`.** The defect: `ChildMgr::heartbeat()` - the normal collection point for every
`--count`-bounded cycle and self-expired lease - read `confirmed_disarm` only to decide relaunch
and returned `Ok(())`; the session then printed *"(no child was running at session end)"* and
exited 0 on the exact reading that means the tracer may have died armed.
✅ **Fixed in build 4, covered by control #15** (`raise_if_unconfirmed` is the one place a receipt
becomes a refusal; `session_exit_code` decides over the whole session's receipts; the inversion
measured the old branch returning `Ok(())` **159 times** against a dead unconfirmed child).
⚠⚠ But the load-bearing residual is open: **no control has ever armed more than ONE thread** -
every event-plane target is a 1-thread `__writer`; shadPS4 has **47**. `threads_armed >= 1` cannot
fail, and armed-N/disarmed-N-1 (the failure that actually kills the client) is structurally
unreachable in the suite. **Until the §3.6 smoke test runs, `d1-scope watch` against the operator's
client is `[U]` with a named instrument, not `[C]` - and every `0 hits` it prints against the
client is `[U]`, not a negative result.**

### 3.1 The state plane

* `MemSource` has `read`, `mappings`, `base`, `generation` - **and no `write`**. The poke ban is a
  type, not a habit. Two impls: `ProcMem` (`/proc/<pid>/mem` `O_RDONLY`) and `SnapMem`
  (`runs/snapshots/*.snap`); every control runs against `SnapMem`.
* ⛔ **Re-resolve every pointer chain from scratch, every tick.** `[[v:0x27E7F18]+0x12A78+0x10]` is
  a route, not an address; caching last tick's result is how you read a freed slot and publish a
  confident zero. `generation()` bumps on re-attach/pid change/snapshot swap and stale resolutions
  are discarded.
* **UNMAPPED is not zero** - a read outside any mapping renders `UNMAPPED`, never 0. Six of the
  registry's surfaces read `UNMAPPED` on `--preset content` snapshots and would otherwise be false
  reds.
* **Liveness before value** - a surface whose liveness predicate fails renders `NOT-MEASURABLE`,
  not 0 (E-13: the machine record's lifetime fields read zero in orbit at either deref level - a
  false red without the guard).
* Seven render states (`VALUE` / `UNMAPPED` / `NOT-LIVE` / `NOT-READ` / `UNRESOLVED` /
  `NOT-EXPRESSIBLE` / `[U]`), and **`STALE` wraps rather than replaces** (`STALE ‹UNMAPPED›`):
  staleness is evidence about the instrument, the reading is evidence about the client, and a
  reader needs both.

### 3.2 The event plane - one child, four slots, a lease

* **d1-scope issues no `ptrace` syscall. Ever.** Debug registers are touched by the `d1-live` child
  only; the shipped control is `c4_readonly_and_no_debug_surface` (reads the binary's undefined
  dynamic symbols, pins the one diagnostic-string residual by identity, scans every source file
  with runtime-assembled needles).
* **The lease is a dead-man's switch**: every spawn carries `--max-seconds`, `LEASE_MAX` = 120 s
  (an over-long lease is REFUSED, not clamped); a d1-scope that hangs, panics or is killed leaves a
  child whose own timer disarms it. Stop is **`SIGTERM`** then a bounded wait for the child's
  confirmed disarm; `SIGKILL` appears nowhere and a control fails the run if it does.
  `confirmed_disarm` is a four-term conjunction - "the process is gone" is not it.
* **`TracerPid` refusal**: a process may have exactly one ptracer; d1-scope refuses by message
  (naming the holder) rather than failing in ways that read like a wrong address. Slot ceiling:
  `MAX_SLOTS = 4` specs registered, **exactly one armed** (`argv` hardcodes `--slot 0`); unarmed
  slots render "NOT ARMED" in words.
* ⛔ **`NoHit` vs error is classified on stdout parseability, never the exit code** - a parseable
  zero-hit document and a broken run both exit 1, and they are opposite facts. Eight event badges;
  `ARMED · nothing yet` and `ARMED · NEVER FIRED` must never share a badge.
* ⚠ TRAP vs FAULT is honoured **per access mode** (write: `rip == writer + len`; exec:
  `writer == rip`, confidence `exact`) - the recorded exec fixture caught a mode-blind control that
  would have accused a correct reading.

### 3.3 The not-list (no change may make d1-scope do these)

Write to the client · link or call ptrace itself · `SIGKILL` anything · launch or manage the
emulator · invent a second address language (everything resolves through `d1_live::addr`) · define
a second format (it parses `registry.toml` and never rewrites it) · speak HTTP / become a
supervisor · take an external graph/plot crate (none is vendored offline) · delete or overwrite
anything in `runs/snapshots/` · report a value without its provenance (⚠ shadPS4's arena allocator
gives singletons identical addresses across runs and days, so "same address" is a control that
always passes).

### 3.4 Controls - 17/17 PASS, and what that does and does not mean

```sh
cd d1-server/crates/d1-scope && cargo build --release --features gui \
  && ./target/release/d1-scope controls ; echo "EXIT=$?"
# => 17 of 17 controls PASS   EXIT=0
# registry receipts: 118 PASS / 0 FAIL / 7 SKIP
# control expressions: 110 parse · 59 paren-unwrap · 14 resolver · 4 prose
```

Every control exits 2 on failure and every one was inverted once. Build history: BUILD 1 state
plane 5/5 · BUILD 2 event plane 10/10 (14 tests) · BUILD 3 surface table 9/9 in both builds (19
tests) · BUILD 4 flow graph 17/17. 12,585 lines, 3,791 (30%) controls. The 7 SKIPs, named: 3
receipts are `snap = "n/a"`; 4 are `verified = false` claims the registry itself disclaims, of
which 1 does not reproduce (`memory_arena_table [orbit-all] u64@(v:0x1ABCB80+0x10) => 320, want
0x24568`) - reported every run, correctly non-fatal.

Notable inversions that changed the code: control #4 failed on its own scanner (rustc const-folded
the needle bytes into the binary; `std::hint::black_box` fixed it); #5's inversion exposed a
missing check ("a surface addr is an address, not a typed value"); #13 was comparing full label
strings and passed its own inversion until stems were compared; the registry controls found four
registry errors (`players_block` offsets `+0x60/+0x64` -> **`+0x40/+0x44`**; two `[BSS]+0x08`
vtable-slot-vs-contents mistakes; a `"PRIVATE"` prefix receipt). #8's inversion was sharpened by a
measurement (1000 hits in 0.30 s let the child disarm on its own before the survival check).

⛔ **What "17 of 17" does NOT mean** (re-measured, all still open):

1. **`controls --source` is inert** - `--source snap:rung36-biped-chain`, `snap:orbit-all` and
   `snap:/nonexistent/nope.snap` all print the identical `118 PASS / 0 FAIL / 7 SKIP`, exit 0.
2. **The default build prints "17 of 17" with two render halves unrun** (#11, #17 need
   `--features gui`; the unrun legs are exactly the UI-only-column and silently-clipped-node
   catchers).
3. ⛔⛔ **14 of 55 surfaces (25%) carry a liveness predicate the evaluator abstains on, and
   `sampler.rs` treats `Unknown` EXACTLY like `Live`** - their field values render as admissible
   numbers with no marker. The failing parse shape is measured: `TY@( <single-deref> + <offset> )`
   (`TY@v:…` works, `TY@([[…]]+off)` works). Three of the 14 are
   `u8@([v:0x2724950]+0x2B48) == 1` ("the world is up") guarding `game_globals`,
   `player_roster_block`, `launch_state_block` - exactly the surfaces consulted to ask "is there a
   player?". A `0` there today is unflagged.
4. **Control #1 has a latent silent skip** - `Snaps::load` opens only `REQUIRED_SNAPS`; retargeting
   25 receipts to another real `.snap` dropped 118 -> 95 PASS, 7 -> 30 SKIP, and #1 still PASSED
   (`pass >= 90` absolute floor).
5. **Every event control target has 1 thread; the client has 47** (R1). The named offline fix:
   `d1-live __writer --threads N`, assert `threads_armed == N` and a hit from a non-main TID and a
   caught partial disarm.

### 3.5 The graph view, briefly

Edge states green / red / grey / unknown / **retired** - collapsing any two mis-accuses an edge;
grey (upstream never populated) comes before any downstream question, so one break is not charged
to three edges; two simultaneous reds are shown as two and labelled INDEPENDENT. **A poked witness
renders `POKED` and every edge touching it `unknown`** - on `rung36-biped-chain` the graph refuses
to colour `sim_player_set` because the witness is in that snapshot's `poked_fields`; a graph that
scored "populated => green" would have painted the project's own break green because we wrote the
byte. Four nodes decline a witness and render `NO-WITNESS` with the closing instrument named.

### 3.6 The live smoke test (never yet run - this is the gate)

```sh
# 0. Single-ptracer rule: ask first, touch nothing.
EMU=$(pgrep -f "[s]hadps4 -p " | head -1); echo "$EMU"
d1-scope tracer --pid "$EMU"          # reads /proc only
# Any TracerPid N => that holder must release: SIGTERM, Ctrl-C, or wait out its <=120 s lease.
# ⛔⛔ DO NOT hard-kill it - a tracer that dies armed KILLS ITS TARGET.

# 1. Build BOTH binaries - different workspaces.
cd d1-server && cargo build --release -p d1-live
cd crates/d1-scope && cargo build --release --features gui
./target/release/d1-scope controls; echo "EXIT=$?"     # must print 17 of 17 PASS, EXIT=0
# ⚠ d1-scope does not build d1-live; a stale child binary is a silent difference -
#   check /proc/<child>/exe for "(deleted)" after a rebuild.

# 2. LEG A - THE POSITIVE CONTROL. Run first, do not skip.
#    Address: world_controller_state_manager+0x1B0 (current_state) - written on every ladder
#    transition, and the client prints its own receipt for each ("Entering state '%s'…"):
#    two instruments, one answer - the only shape that makes 0 hits interpretable.
mkdir -p /tmp/d1scope-smoke
D1_SCOPE_ALLOW_LIVE=1 d1-scope watch \
    --pid "$EMU" \
    --surface world_controller_state_manager \
    --addr '[v:0x26B13E8]+0x1B0' --len 4 --rw write --base \
    --lease 20 --count 32 --seconds 90 \
    --stdio-dir /tmp/d1scope-smoke ; echo "EXIT=$?"
```

Verdicts stated in advance: `threads_armed == 47` and hits == transitions => the event plane works
on the real target. `threads_armed == 1` => R1 is real; do not interpret any `0 hits` until fixed
offline. `47` armed but 0 hits while the log shows transitions => a real defect in attribution or
address, not a finding about the client. Any `⛔ DISARM NOT CONFIRMED` / exit 2 => stop, do not
hard-kill anything, let the lease expire, read `/tmp/d1scope-smoke/*.err`. Also record the arm
latency - over ~1 s against 47 threads means R4 (the `lease + 3 s` stop deadline) is live.
⚠ `--stdio-dir` always (a child whose parent dies loses the pipe and with it the report written
after disarming); `D1_SCOPE_ALLOW_LIVE=1` is required and deliberate; exit 2 from `watch` means one
thing only - some stop did not prove a disarm (never a missing file; that is exit 1).
⚠ Leg B's two candidate watches survive only as a thread-coverage exercise - see §7 dead claims
("any hit is the answer").

---

## 4. `d1-network-trace` - the shadPS4 guest-boundary attempt ledger

**Status:** implemented and compiled in the isolated `shadPS4-d1trace` worktree (a sibling checkout
outside this repo). The dedicated binary contains **both the account and network tracers**. Runtime
verification is intentionally pending; no emulator run was launched for this work.
**Reach: instrumentation only** - a trace records what the synthetic-ancestor client attempted at
the emulator boundary; it does not establish what the retired services did.

**The question:** for one socket operation, distinguish (1) our responder says it wrote bytes;
(2) the host socket delivered bytes to shadPS4; (3) the guest consumed them from `sceNet*`;
(4) Destiny's BAP/investment decoder accepted them. The responder proves (1) and owns the BAP
plaintext log; guest diagnostics and the account tracer prove parts of (4); this tracer is the
missing independent witness at (2)/(3). It is an **attempt ledger, not a packet capture**: failed
connects, empty/partial reads, failed sends, and calls into shadPS4's stubbed P2P surface are
first-class observations. `[C]` A request that produced no traffic may still be the gate.

**Hook placement `[C, source]`:** the `sceNet*` exports are not the complete boundary (guest kernel
`read`/`write` on a socket also reaches the socket virtual interface), so hooks live in the shared
implementation: `PosixSocket::{Bind,Listen,Connect,Accept,Close,SendPacket,ReceivePacket,
SendMessage,ReceiveMessage}`, the equivalent `UnixSocket` methods, and `P2PSocket` plus the
signaling/control/Matching2 helpers (stubs - the trace records the attempt and the existing
refusal; it does not implement P2P). `socket_key` is the socket object's address; `native_socket`
the host fd; connected TCP sends usually have no per-call endpoint - the preceding connect record
provides the mapping.

**Perturbation controls:** host load has already changed boot outcomes here, so the hot path does
only a monotonic timestamp + tid read, fixed-size record construction, a bounded payload copy (256
bytes default, max 512), and a lock-free reservation in an 8,192-slot ring. A background thread
drains at most 128 records per batch to JSONL. Guest threads do no formatting, allocation, locking
or file I/O; overload increments `dropped` rather than blocking. `EAGAIN`/`EWOULDBLOCK` from
ordinary receive polls are counted in `suppressed_wouldblock` and not serialized (failed sends and
P2P stub attempts are never suppressed); set `D1_NETWORK_TRACE_RECORD_WOULDBLOCK=1` only for a
named receive-poll experiment. **A trace with nonzero `dropped` is incomplete** - the summarizer
fails unless given `--allow-loss`. This is still an intervention: a causal claim needs the normal
uninstrumented control and quiet-host provenance.

**Activation:** compiled only with `ENABLE_D1_NETWORK_TRACE=ON`; dormant unless
`D1_NETWORK_TRACE_OUT` names a file; runtime guards require `CUSA00219`, `APP_VER=01.29`.

```text
D1_NETWORK_TRACE_OUT=/absolute/run/path/network-trace.jsonl
D1_NETWORK_TRACE_PAYLOAD=256                 # optional, 0..512
D1_NETWORK_TRACE_RECORD_WOULDBLOCK=0         # optional; default 0
```

Output is JSONL, schema version 1: `start` / `socket-attempt` / `stats` / `stop`, with cumulative
`attempts`, `enqueued`, `written`, `dropped`, `suppressed_wouldblock`.
`capture-rig/summarize-network-trace.py` validates and reduces it, failing on malformed records and
nonzero drops by default.

**What it can prove for an investment-503 run:** the BAP TCP connection reached `10.77.0.2:3075`;
the reply crossed the guest socket boundary; one read or several partials; whether closure/error
preceded the investment consumer; ciphertext matches the responder's frame. **What it cannot:** a
valid frame rejected by the RSAT/investment decoder (that is the account tracer's and the
disassembly's job); it does not replace pcap for retail captures (TLS/BAP mode-1 is encrypted at
this boundary; the responder has the keys and logs plaintext). The P2P hooks gain value after
orbit: the first call into a stubbed P2P method is itself a named emulator gap even at zero
packets.

**Verification state:** `[C]` hook reach checked against shadPS4's socket virtual interface ·
`[C]` the analyzer passes synthetic JSONL validation · `[C]` a cold Linux x86-64 build completed
with both `ENABLE_D1_ACCOUNT_TRACE=ON` and `ENABLE_D1_NETWORK_TRACE=ON`; binary
`shadPS4-d1trace/build-d1nettrace/shadps4`, SHA-256
`09c94eb5ca2b4d18be9ee4600086f6a5c67fb2f7b55ac8b1c5c2ca925d072a94` ·
`[U]` the C++ writer has not been exercised against a running title (compile success validates the
hook surface and types, not event ordering, queue behavior under load, or JSON output) ·
`[U]` no runtime trace exists - do not describe the tracer as runtime-confirmed until one run has a
`start`, socket events, a stats record with `dropped=0`, and an uninstrumented control.
**Licensing:** new project code in the GPL-2.0-or-later shadPS4 worktree; no excluded AGPL or
unlicensed project read or used; `d1-server/crates/d1-bap` remains the one wire-code home.

---

## 5. Guest diagnostics - why 2 of 1,291 lines appear, and how to get the rest

**The question:** the 01.29 retail build carries ~1,291 `networking:` diagnostic strings, but a
clean run surfaced only two distinct lines. The brief offered three explanations: (a) buffering,
(b) a compiled-in filter, (c) the code not being reached.

⭐ **The real answer was a FOURTH that none of them named: the sink is a no-op.** A logger that
formats correctly, passes its filter, and then `memcpy`s into a ring buffer nobody drains is not
"buffering", not "filtered", and not "unreached" - it is *plumbed to nowhere*. It is the category
that produces a **false negative**: at the build's default threshold **626 of 1,614 log sites
(39%) already pass the filter** and none appear, so widening the filter alone yields nothing and
reads as (c). The discriminating first move worked: compare one visible string against one missing
string - ours reached *different sinks* (the two visible lines are a direct `printf` PLT call, not
the retail-log path at all, so they were never evidence about that path).

⛔ **2026-08-10 - a THIRD SHAPE, which no filter or sink patch reaches:** some refusal sites never
call the logger at all - they `vsnprintf` into a **stack buffer that is never read**
(`FUN_010548E0` at `FUN_00414730`). A run-log grep for such a string is a **structural false
negative**. (`historicaldocs/design/arrival-handoff.md` §6.4.)

Ruled out `[C]`: host-side buffering (the emulator's `Logger::log` flushes on every newline); the
game stopping early (the run reaches signon, STUN, `scePlayGo`, runs 39 minutes); the devkit debug
TTYs (the guest opens `/dev/console` and `/dev/deci_tty6` `O_WRONLY` but never writes to either -
still a latent blind spot, fixed defensively below).

### 5.1 `[C]` The mechanism - two gates

Every site compiles to the same shape (example `networking:signon: Entering state %s` at
`0x2a70e9`): `retail_log_check(system, level)` at **`0xe4da0`** (GATE 1), then format via
`0x1047480` into a stack buffer, then `retail_log_emit(site_id, text)` at **`0xe4ad0`** (GATE 2).

* **Gate 1** reads `g_retail_log` (pointer at `0x256eef0`), threshold defaults to 5 if never
  allocated, `thresholds[system]` at `+0x19808`, and returns `threshold <= level`. ⚠ **The sense is
  easy to get backwards: a higher threshold is quieter.** Valid levels 0..5; the setter at
  `0xe4ba0` rejects `>= 6`. `retail_log_init` at `0xe4890` allocates `0x75ba8` bytes and sets 22
  thresholds to 4.
* **Gate 2 never outputs anything**: it reserves a ring slot by CAS, `memcpy`s `0x140` bytes of
  text, sets a ready flag. **No call to any output function.** All 12 rip-relative references to
  `g_retail_log` live inside one 0x7c0-byte module (`0xe4890`..`0xe5000`); the ring's only
  consumers are debug/status-overlay accessors (`0x76fea7`, `0x76fede`, `0xae5d58`). `[C]` Nothing
  drains it to stdout. The struct layout accounts for the allocation exactly (format registry
  512 x `0xcc` = `0x19800`; thresholds[22] at `+0x19808`; text ring 1124 x `0x150` = `0x5C340` at
  `+0x19860`; cursor at `+0x75BA0`; total **`0x75BA8`** - a good check the reading is right).
* `[C]` The two visible lines are a separate function: `0x593c0`, a ~5-second rate-limited routine
  calling the `printf`/`puts` PLT stubs directly (`0x129d450` / `0x129d460`). Of all **1,287**
  NUL-preceded `networking:` strings, exactly two reach a printf-family PLT stub, and they are
  these two. `[O]` They appear as a pair 48 times, always after a `sceKernelStat: error = 2`. The
  channel "working" proves only that `printf` reaches the emulator's `[stdout]` Logger - nothing
  about the retail-log path, which has never emitted a byte. `[C]` The formatter `0x1047480` makes
  only two PLT calls (`0x129d0b0`, `0x129d490`), neither printf nor puts.

### 5.2 `[C]` Measured: levels, systems, thresholds

Over all **1,632** `call 0xe4da0` sites (1,614 statically recoverable; 18 `[U]`):

| message level | sites | | uniform threshold T | sites that emit |
|---:|---:|---|---:|---:|
| 0 | 80 | | 0 | **1614 (100%)** |
| 1 | 265 | | 1 | 1534 (95%) |
| 2 | 277 | | 2 | 1269 (79%) |
| 3 | 366 | | 3 | 992 (61%) |
| 4 | 626 | | 4 | **626 (39%)** |
| 5 | 0 | | 5 | 0 (0%) |

`[C]` Exactly **22** system ids (0..21), matching the 22 thresholds set at `0xe4890` and the 22
setter calls at `0x1db453`..`0x1db55a`; the id -> subsystem mapping was recovered by correlating
gate sites with the strings they format:

| sys | subsystem | L0 | L1 | L2 | L3 | L4 | total | T=4 | T=3 | T=2 |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | `channel_manager` / `channel` | 24 | 50 | 32 | 31 | 27 | 164 | 27 | 58 | 90 |
| 1 | `demonware` / `transport` | 5 | 1 | 7 | 3 | 11 | 27 | 11 | 14 | 21 |
| 2 | `activity_client` | 0 | 15 | 11 | 7 | 7 | 40 | 7 | 14 | 25 |
| 3 | `[U]` | 4 | 4 | 7 | 4 | 10 | 29 | 10 | 14 | 21 |
| 4 | `[U]` | 15 | 22 | 26 | 29 | 21 | 113 | 21 | 50 | 76 |
| 5 | `online` | 0 | 26 | 34 | 44 | 63 | 167 | 63 | 107 | 141 |
| 6 | `logic` / `online` / `configuration` / `leaderboard` | 13 | 83 | 65 | 42 | 36 | 239 | 36 | 78 | 143 |
| 7 | `investment` | 0 | 0 | 0 | 4 | 103 | 107 | 103 | 107 | 107 |
| 8 | `investment` | 0 | 0 | 2 | 3 | 16 | 21 | 16 | 19 | 21 |
| 9 | **`session`** | 9 | 11 | 26 | 91 | 151 | 288 | 151 | 242 | 268 |
| 10 | `session` / `logic` | 0 | 4 | 10 | 20 | 14 | 48 | 14 | 34 | 44 |
| 11 | **`stun`** | 0 | 17 | 12 | 2 | 39 | 70 | 39 | 41 | 53 |
| 12 | `messages` | 0 | 0 | 0 | 0 | 4 | 4 | 4 | 4 | 4 |
| 13 | **`signon`** / `online` | 10 | 3 | 15 | 22 | 47 | 97 | 47 | 69 | 84 |
| 14 | **`sku_manager`** | 0 | 1 | 0 | 5 | 27 | 33 | 27 | 32 | 32 |
| 15 | `[U]` | 0 | 0 | 0 | 0 | 3 | 3 | 3 | 3 | 3 |
| 16 | `simulation` | 0 | 1 | 4 | 2 | 20 | 27 | 20 | 22 | 26 |
| 17 | `team_picker` | 0 | 17 | 0 | 0 | 0 | 17 | 0 | 0 | 0 |
| 18 | `[U]` | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 | 1 |
| 19 | **`managed_session`** | 0 | 10 | 25 | 50 | 26 | 111 | 26 | 76 | 101 |
| 20 | `platform_user` | 0 | 0 | 1 | 3 | 0 | 4 | 0 | 3 | 4 |
| 21 | `[U]` | 0 | 0 | 0 | 4 | 0 | 4 | 0 | 4 | 4 |

`[U]` **The live thresholds are not knowable statically** - `0x1db440` reads them out of a
game-data config blob (`*(void**)0x267b1b8 + 0x26ec`, fields `0x30`..`0x84`); they may be 5 (fully
silent). That is exactly why the fix overrides the array rather than writing to it.

### 5.3 The fix - two byte patches, no emulator rebuild

shadPS4's `-p/--patch <file>` applies an XML byte-patch list right after ELF load;
`Address = vaddr + 0x400000`; `AppVer` must equal `param.sfo`'s `APP_VER` (**`01.29`**,
`TITLE_ID=CUSA00219`) **or non-mask lines are silently skipped**. Delivered as
`patches/CUSA00219-guest-diagnostics.xml`.

* **Patch B (required) - give the sink an output.** `Address=4E4AD0`, 14 bytes: replaces
  `retail_log_emit` with a tail-call to `printf("%s\n", text)` -
  `48 8D 3D F9 07 52 01` (`lea rdi, [rip+0x15207f9]` -> `"%s\n"` @ `0x16052d0`, the only such
  literal in the binary) · `30 C0` (`xor al, al`) · `E9 72 89 1B 01` (`jmp 0x129d450`, the printf
  PLT stub already proven to reach the emulator's `[stdout]` Logger; `rsi` already holds the text;
  the tail jmp leaves `rsp % 16 == 8`; the return value is unused at every call site). Using `%s`
  avoids `%` in runtime values being interpreted as conversions. **Trade-off, stated plainly:**
  this *replaces* the ring write, so the in-game network-status overlay stops being fed - `[I]`
  irrelevant for log capture, but a real behaviour change.
* **Patch A - collapse the filter to one tunable byte.** `Address=4E4DA0`, 7 bytes:
  `83 FE <T>` (`cmp esi, T`) · `0F 9D C0` (`setge al`) · `C3`. Same sense as the original `setle`,
  uniform across systems, independent of both the threshold array and the config blob (removing
  the `[U]` above), and no longer dependent on `g_retail_log` being allocated.

Both payloads were derived programmatically and round-tripped through `objdump` to confirm the
`lea`/`jmp` targets land on `0x16052d0` and `0x129d450` exactly.

```bash
shadps4 -p patches/CUSA00219-guest-diagnostics.xml <game>
```

The file ships Patch B plus five mutually exclusive threshold variants; **B and exactly one
threshold** should be enabled (T=4 default). Success: `Applied patch:` lines, and
`[Tty] <Info> … [stdout] networking:<subsystem>: …` lines beyond the two known ones.

`[I]` **Start at T=4 and walk down; do not start at 0.** T=0 turns on all 1,614 sites including
per-frame paths (potentially tens of thousands of lines per second), and **extra host load causes
init-race failures in this title**. The ladder: T=4 (626 sites: `signon` 47, `sku_manager` 27,
`stun` 39, `demonware` 11, `managed_session` 26) -> T=3 (992) -> T=2 (1269) -> T=1 (1534) -> T=0
(1614, firehose).

**Defensive emulator change (not the fix):** `/dev/deci_tty6` and `/dev/console` discarded writes
and returned 0 instead of nbytes; both now delegate to the line-buffered `Devices::Logger`
(implemented in the sibling `shadPS4-main` checkout at `8bdec718`; 4 files, relinks clean). `[C]`
Destiny never writes to either; this closes a blind spot, not the bug.

**Status:** `[C]` question answered (a filter, with the correction that the unprinted ring is the
binding constraint) · `[C]` fix implemented and statically verified · `[C]` emulator change
builds · **`[U]` not verified at runtime** - a verification run would settle whether the patches
apply, whether `printf` from many guest threads behaves (`[I]` the emulator's Logger locks per
write, so it should), and the actual line volume at T=4.

**Method note:** the eboot has no section headers, so plain `objdump -d` is unusable; everything
came from scanning RIP-relative displacements and `call rel32` targets over the raw image (both
LOAD segments share `vaddr = fileoff - 0x4000`) and disassembling windows with
`objdump -b binary --adjust-vma`.

---

## 6. Open `[U]`s - the consolidated ledger

Every row names its instrument; `[U]` with a named instrument is a finding.

| id | `[U]` | instrument |
|---|---|---|
| L1 | **shadPS4's guest threads have never been watched by `d1-live`** - controls prove the mechanism on our own children only | the §2.7 worked run; threads refusing SEIZE and per-activity threads are both visible in the report |
| L2 | **whether shadPS4 itself uses `SIGTRAP` or debug registers** (the live-watch §10 question) - a collision would show as non-zero `unattributed sigtraps` | try `--slot 3` first, read the counter |
| L3 | `--snapshot-on-hit` never pointed at shadPS4: does a guest thread honour `PTRACE_INTERRUPT` inside 1500 ms? does the client survive a ~4.6 ms stop? what is the realised stop end to end (the rehearsal is a floor, not a bound)? is the phase split still true under patches? does `--follow-clone` strand a late thread? | one `capture --snapshot-on-hit --stop-scope minimal` at a known-hot address, then `state`; read the `DISARM` line and `Coherence`'s stamps |
| L4 | `emergency_disarm()` receipt (raw `write(2)` to fd 2) - not built | ~15 lines in `d1-live`; until then verify the client after any armed crash |
| L5 | a `could NOT be proven disarmed` line does not fail the controls tally | count `disarm()`'s failure list into the verdict, DEGENERATE when it never fires |
| L6 | `replay --from-trap`, `absent-is-not-empty`, `--stop-threads all`, FP/SSE capture, a `stop-window-budget` control that fails a run | each named NOT BUILT in historicaldocs/design/live-watch.md §6a.9/§6a.7 |
| S1 | **d1-scope has never armed the real 47-thread client**; `threads_armed >= 1` cannot fail; partial disarm structurally unreachable in the suite | `d1-live __writer --threads N` offline, then §3.6 leg A |
| S2 | `Unknown` liveness treated as `Live` for 14 of 55 surfaces (the `TY@(deref+off)` parse gap) | teach `eval` the form; make `Unknown != Live` in `sampler.rs`; ship `liveness-gates-value` against the registry |
| S3 | `--source live` for the state plane not wired; `ProcMem` has only read d1-scope's own process | wire it plus a throwaway-process control |
| S4 | `controls --source` inert; "17 of 17" printed with render halves unrun; #1's `REQUIRED_SNAPS` skip | reject the flag; refuse the words; assert `REQUIRED_SNAPS` superset + relative floor |
| S5 | registry U1-U10 (activity blob address, `FUN_00BB3FD0`, `Q+0x20` skip byte, cache-state offset, team-picker field - **U6 is the cheapest open item in the project, offline against `rung36-biped-chain.snap`** - camera, `Q+0x10` writer, availability cache, roster contact container) | named per row in historicaldocs/design/d1-scope.md §19.2 |
| G1 | `d1guest`: profile blob `+0x18` -> profile-schema leaf mapping; `slot_b = 0x…0201` identity; `+0x248` field type; the `0x263D0F8` third-shape reading | one schema lookup (§26.76's method); one lookup by §26.76's owner; a `state == 2` run; one more structured dump |
| N1 | the network tracer has never produced a runtime trace | one run with `start` + events + `stats dropped=0` + an uninstrumented control |
| D1 | the guest-diagnostics patches are not runtime verified | one run with `-p`, look for `Applied patch:` and new `[stdout] networking:` lines |

---

## 7. Dead claims

Retracted by their sources' own banners; never carry these forward as live.

* "`0x79EB80`-`0x79EC20` are `profile_setup`'s methods; the update method is `0x79EC20`" - dead;
  they belong to `bootflow:queuing` (state 17). `profile_setup`'s update method is `0x79E830`.
* "`0x263D0F8`'s fields may be engine-config entries making the brightness screen server-skippable"
  - dead as posed; it is the wrong state's handler, and the accessors ignore the pointer anyway.
* "Everything except `SIGKILL` is safe while `d1-live` is armed" - FALSE; `SIGHUP` was measured
  lethal 2 of 2. What is safe is the **caught** signal set.
* "The `c_activity_loader+8` writer can only be found by a live watch" - closed statically:
  `FUN_00341500 @ 0x341798`, via `field-xref`'s escape rows.
* "`d1-scope` has a control `no_ptrace_call_path`" - no such function has ever existed; the shipped
  (and stronger) control is `c4_readonly_and_no_debug_surface`.
* d1-scope's 18-row control table - the BUILD-0 plan, not the binary; the binary ships **17**
  controls with different names (planned #6 `liveness-gates-value` was never implemented).
* "`absent-is-not-empty` is a shipped `d1-replay` control" - listed in a table, never written;
  cannot exist until `replay --from-trap` does.
* "d1-scope exit 0 implies every disarm was confirmed" - false before build 4 (`heartbeat()`
  returned `Ok(())` 159 times against a dead unconfirmed child); fixed, control #15.
* "The disarm costs about 0.7 ms" read as a bound - it was a measurement; step 2 was unbounded
  until build 3's `--disarm-deadline-ms`.
* "Any hit is the answer" for the `gg+0xD0` (U2) and `[v:0x27257F0]+0xC0` watches - retracted;
  `gg+0xD0` is memset by design and nobody writes it, `+0xC0` is written only inside a creator that
  never runs, so a no-hit is *guaranteed* and the design fails like a real negative. "Any hit is
  the answer" is only a good design when a hit is POSSIBLE.
* "The missing `networking:` lines are buffering, or a filter alone, or the game not getting
  there" - dead; the sink is a no-op ring nobody drains (626 sites already pass the filter), and a
  third class formats into a stack buffer that is never read, which no filter or sink patch can
  reach.

---

## 8. Sources

Cited throughout as `historicaldocs/design/<name>.md §N`.

| source | contributed |
|---|---|
| `historicaldocs/design/runtime-state-access.md` | §1 - the plan of record, `d1guest.py`, base controls, liveness ladder, hop mechanics, first-use findings and retraction |
| `historicaldocs/design/live-watch.md` | §2 - `d1-live`, the ten controls, the signal-safety corrections and defects, the argument-capture idiom, `--snapshot-on-hit` |
| `historicaldocs/design/d1-scope.md` | §3 - the two planes, the lifecycle verdict, 17 controls and their holes, the smoke test |
| `historicaldocs/design/d1-network-trace.md` | §4 - the guest-boundary attempt ledger |
| `historicaldocs/design/guest-diagnostics.md` | §5 - the retail_log mechanism, the fourth explanation, the patches |

Companion live-instrument docs for other topics: `design/instruments-static.md` (offline binary
tooling), `design/observability-and-ui.md`, `design/eboot-map.md`.
