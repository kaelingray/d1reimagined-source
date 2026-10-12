# d1-ghost - the run companion

**Status: v0 + v1 BUILT (2026-08-11).** The plan below is the design of record; what is built:
* **v0** - `d1-dash` R1-R3 fixed with pinned red controls (`observability-and-ui.md` §3.7).
* **v1** - the `d1-ghost` binary (`d1-trace/src/bin/d1-ghost.rs`, third bin on the crate): the dash
  read panels embedded by composition plus the write console. Registry, ledger and gated executor
  in `d1-trace/src/ghost/{registry,ledger,exec,dests,gui}.rs`; the activity-modes generator in
  `d1-server/crates/d1-oracle/src/activity_modes.rs`. **9 `C-GHOST-*` controls green** in
  `d1-trace controls` (64/64 total), plus `d1-oracle activity-modes controls` 6/6 on the real
  block. ⚠ The egui rendering is runtime-unverified in CI (no display); its logic is control-pinned.
* v2 (server-side `/control` door) and v3 (forge convergence) remain future.

This doc is the plan for the "one thing" the operator asked for: *"we need a log viewer, and we
need an egui debug menu to be able to control pokes and such - honestly it just needs to be one
thing."*

**The one-liner:** one window that floats beside a live run - the `d1-dash` read panels (ladder,
errors, families, TTY tape) plus a control console whose every write is **registered, gated, and
ledgered into the run directory**. Named for the Ghost: it hovers next to the Guardian, scans
things, pokes mechanisms, and narrates.

---

## 1. Why now - the problem this dissolves

1. **Pokes are invisible.** Today a poke is a shell invocation of `capture-rig/d1guest.py poke`
   recorded nowhere but the session chat. `synthetic_ancestor` taint is *remembered*, not
   *recorded* - the provenance rule that anchors the whole evidence convention rests on human
   recall. `[C]` no run directory contains any record of the writes made into its client.
2. **Poke sequencing is enforced by memory.** Applying the two code hooks before
   `activity:in_world` crashes the client before rung 34 (`design/GUARDIAN-IN-WORLD.md`, hook
   sequencing). The only guard is that whoever pokes remembers this. The tool that knows the
   current rung (`d1-dash`) and the tool that pokes (a shell) are different tools.
3. **Diagnosis is assembled from scattered instruments.** The 2026-08-11 llvmpipe finding (every
   non-RenderDoc run ever was software-rendered) took process-env reads, `top -H`, and a log grep
   across 40 runs - none of it on any panel. A `GPU_Model` lamp would have surfaced it months ago.

## 2. Relationship to the standing read-only property

`design/observability-and-ui.md` §1: *"all viewers run nothing, POST nothing, write nothing into
any client. That property outranks any feature."* **That property stands, untouched.** `d1-ghost`
is not a viewer and does not join that table - it is the project's **one write door**, a new class
with its own outranking property:

> **No invisible writes.** Every write into a live client goes through the registry, is gated by
> observed client state, and is journaled to the run directory before and after execution. A poke
> that cannot be ledgered must not execute.

This is *stronger* provenance than today, not weaker: taint becomes computable from
`runs/<run>/pokes.jsonl` instead of recalled from chat. The read side inherits `d1-dash`
unchanged; the header derives its `synthetic_ancestor` chrome from `ledger nonempty OR reach tag`
(the derivation R8 asked for, instead of unconditional chrome).

⚠ **Honest bound:** the ledger is a lower bound, not a lock. `/proc/<pid>/mem` cannot be reserved;
a shell poke bypassing ghost is still possible and still invisible. The header must render the
lamp as "no *ledgered* pokes", never "no pokes". The discipline "all pokes go through the door"
is procedural, like "all launches go through `run-boot.sh`".

## 3. What exists and is reused - build almost nothing twice

| piece | where | reused as |
|---|---|---|
| six live panels, three-state cells, provenance hover | `d1-trace/src/dash/` | the whole read side, as a library |
| tail/ingest/query (byte-offset tail, ANSI strip, `[Tty]` filter) | `d1_trace::{ingest,model,query}` | log viewer internals |
| write-side HTTP pattern (one plain thread, blocking call, producer-side repaint, four failure modes) | `d1-server/crates/d1-forge/src/control.rs` | the console's apply path shape |
| live memory read/write | `capture-rig/d1guest.py read\|poke` | v1 execution backend (subprocess) |
| activity launch seam | `capture-rig/session-params.py --launch-poke` | the one non-memory registry action |
| supervisor + `/control` route design | `design/observability-and-ui.md` §6.1 | v2 server side, already specified |
| egui `=0.36.1` pin, glow, no-tokio-in-GUI shape | §6.3 / §7 of the same doc | unchanged |

**Where the code lives:** a third binary in the `d1-trace` crate (`bin/d1-ghost.rs` +
`src/ghost/{registry,ledger,console}.rs`), for the same reason `d1-dash` lives there and not in
`d1-forge`: the crate is glow-pinned and outside the `d1-server` workspace, so wgpu stays out of
whole-workspace builds. Controls join `d1-trace controls` as `C-GHOST-*`.

## 4. The poke registry

A Rust const table in the crate (not a config file - reusable tools are Rust, and the entries are
load-bearing facts with citations, exactly what `#[test]` controls pin). One entry per known poke:

| field | meaning |
|---|---|
| `name` | short slug, the ledger key (`done-mask-7ff`, `je-411e16-nop`, ...) |
| `class` | `Probe` \| `Satisfy` \| `Launch` - rendered on the button. The house rule is *"poke to identify a gate, never to satisfy one"*; a `Satisfy` button renders with the standing warning that the deliverable is server code |
| `target` | eboot vaddr or heap expression + bytes, or the whitelisted tool invocation |
| `gate` | preconditions from observed state: minimum rung, liveness at least L3, phase |
| `citation` | design doc + section that owns this poke - nonempty, enforced by a control |
| `verify` | expected read-back after the write |

v1 registry contents (the currently known estate, from `design/GUARDIAN-IN-WORLD.md` and
`design/activity-lifetime-state.md`): the rung-34 done-mask force, the `FUN_0010C500` hook, the
`je @ 0x411E16` NOP, the spawn poke, the `+0x1AC` phase byte, and `--launch-poke --activity-id N`.
Registry entries are expected to *shrink* over time - a poke earned back onto the wire is deleted
here, which makes the registry double as the honest list of what is still synthetic.

Free-form read (address / len / access mode) is unrestricted - reads are safe. Free-form *write*
exists behind a deliberately unpleasant expander, classed `Freeform`, and is ledgered like
everything else.

### 4.1 The destination selector - the `Launch` entry grown into a picker

The launch action is not one button; it is a **MODE x MAP selector populated from the game's own
data**, none of it hand-typed (`design/activity-launch.md` §2):

* **MODE list:** the client's 540-entry activity table (from the investment globals block that
  §2.1 re-derived), with the client's own launchability rule evaluated per entry:
  `+0xA9 == 0` (no matchmaking) and the private-match band 393-413. Default 407 Elimination,
  the stated endgame.
* **MAP list:** the 32 `pvp_*` stems (`activity-launch.md` §2.2, `[C]`), pinned in
  `ghost::dests::PVP_MAPS` with de-slugged labels; the stem IS the world-name string. ⚠ The
  32-vs-33 discrepancy (§2.2) is carried, not resolved. **Built 2026-08-11** as a dropdown; the
  observed-live pair (`pvp_moonbase x 407`) is marked, every other pair `[U]`.
  ⚠ **The MAP is AH-served, not a client poke.** The world name is published at `blob+0x688` by the
  Activity Host (`D1_AH_ACTIVITY=<stem>`), so the picker FIRES the MODE (client-side activity-index
  poke, verified) and COMPOSES the map into the `D1_AH_ACTIVITY`/`_INDEX` launch config for the AH.
  A click-time client-side map poke is a `[U]` - it needs a live-verified pointer chain to
  `global_activity_state`, and shipping an unverified write address is the exact crash-the-client
  trap the deref fix exists for.
* ⭐ **Non-launchable entries carry the client's own refusal** (`+0xA9 = 0xFF matchmaking
  required`, content family). ⚠ **Amended 2026-08-11:** the design first said "greyed, not hidden";
  in practice 519 of 540 are non-launchable, so the console defaults to **launchable-only** with a
  **"show unlaunchable (reachability map)" checkbox** that reveals them with their reasons. The
  honest-map property is preserved (one click away), without a 540-row wall by default.
* **Names:** only the 21 launchable indices (plus a handful of pinned non-launchable ones -
  Tower, the Patrols, the map-name band) have display names; block 1063 carries no name strings, so
  the rest are `activity N` until the external Bungie `DestinyActivityDefinition` manifest is
  cross-referenced locally into the (gitignored) TSV. `[U]` follow-up, not yet built.
* **Pair provenance:** each MODE x MAP combination carries its reach tag - `pvp_moonbase` x 407
  is `observed-live` (rung 36); everything untried renders `[U]`, and a successful launch from
  the selector upgrades the pair in a small on-disk ledger of attempted combinations. An
  activity attempt costs a poke, not a boot - failures recover to orbit - so sweeping the `[U]`
  space is cheap and the selector makes the sweep legible.
* **Gate:** enabled only at `setup:orbit` with liveness L3, like every registry entry.

Data path: the MODE table is generated by **`d1-oracle activity-modes write`** (the table is
CONTENT - block 1063 of `ps4_investment_globals_012f_3`, walked via the Oodle decoder - **not**
`d1-eboot` data; this line is corrected from the design's first draft). Output
`capture-rig/activity-modes.tsv` is gitignored (the generator ships, its output does not). Two
controls guard it, split by where the data lives: **`d1-oracle activity-modes controls`** pins the
launchability facts against the real block (21 launchable, band 393-413, 407 Elimination, one
INVERTED, needs the owned-media dump); **`C-GHOST-DESTS`** pins the consumer headless off a fixture
(parse, greyed-entries-carry-a-reason, the shared column-header contract, wrong-header-rejected).
⇒ built 2026-08-11, `d1-server/crates/d1-oracle/src/activity_modes.rs` +
`d1-trace/src/ghost/dests.rs`.

## 5. The ledger - `runs/<run>/pokes.jsonl`

Append-only JSONL, two lines per poke, written by ghost itself:

1. **intent** - appended *before* execution: RFC3339 UTC ts, seq, name, class, target, requested
   bytes, current rung, operator note (optional free text).
2. **result** - appended after: read-back before, read-back after, outcome
   (`applied` / `mismatch` / `failed`), tool exit status.

Fail closed: if the ledger cannot be appended (missing run dir, read-only fs), the poke refuses.
A crash between the two lines leaves a visible dangling intent - which is a fact worth having,
not a bug. Consumers: the ghost header, `d1 show` (render the poke count and classes in the run
summary), and any future promotion review ("was this gate crossed on a ledgered write?").

## 6. Execution path

**v1: subprocess, no new formats.** Memory writes shell to `capture-rig/d1guest.py poke` exactly
as `d1-dash` shells to `d1guest.py descriptors` for reads; the launch action shells to
`session-params.py`. Ghost describes zero memory formats and owns zero address arithmetic beyond
what the registry entry states. The Python port of `d1guest.py` is the retirement queue's problem
(§8 of the observability doc), not this tool's - when a Rust reader/writer lands, ghost swaps the
subprocess for the crate call and nothing else changes.

**v2: the door moves server-side.** Per the standing rule "every feature exists in the CLI/JSON
contract before or with any UI surface": `d1 poke <name>` (CLI parity, same registry via the
shared crate), then the §6.1 supervisor grows the already-specified `/control` route and ghost
becomes a pure HTTP client. `d1-forge` then POSTs loadouts to the same door - one writer, one
protocol, and the forge-as-a-tab question is deferred until the operator actually feels the
alt-tab friction (v3, decide then, not now).

## 7. Guards - what the merged tool can enforce that a shell cannot

* **Rung gates.** Hook buttons grey below rung 36; the ladder panel and the poke console are the
  same process, so the gate reads observed state, not recollection.
* **Liveness gates.** Below `d1guest` liveness L3, every write control greys (the dash already
  computes L0-L3).
* ⛔ **Ghost is never a launcher.** Containment lives in `run-boot.sh` →
  `run-signon-experiment.sh` and nowhere else; the project already carries one drifted duplicate
  launcher and does not need a third. Ghost *attaches* to the newest (or named) run directory; it
  spawns nothing except the two whitelisted tools above, enforced by a control over the command
  table.
* **New header lamp while we are here:** `GPU_Model` scraped from `emulator.log` at attach,
  rendered red when it says `llvmpipe` (2026-08-11: every non-RenderDoc run to date was
  software-rendered and nothing surfaced it).

## 8. Non-goals (v1, each with its reason)

Launching or killing runs (containment, above); merging `d1-forge` (works today, pre-run tool,
wgpu/glow boundary - revisit at the v2 door); BAP semantics (`d1-observe` owns them); plots
(`egui_plot` resolver trap, §3.5); config/theming/keybindings (not until a third human asks);
any second implementation of memory access, log parsing, or the rung table (no format twice).

## 9. Build order

* **v0 - precondition:** fix `d1-dash` R1-R3 with their red controls (§3.7 of the observability
  doc). The review's verdict stands: the dash must not decide anything until then, and a control
  surface wired to an untrusted display is worse than a shell.
* **v1 - the merge:** `d1-ghost` binary = dash panels + console + registry + ledger + GPU lamp.
  `C-GHOST-*` controls below. This is the deliverable.
* **v2 - the door:** `d1 poke` CLI, supervisor `/control`, ghost and forge as clients.
* **v3 - convergence:** forge-as-tab decision, AH hot-swap (`nsenter` swap of the Activity Host)
  as a registry action, pad macros - each only when its trigger fires.

## 10. Controls - `C-GHOST-*`, joining `d1-trace controls`

House meta-rules apply: state the axis in words first, every control ships a mutation test that
breaks the thing on purpose, a control that cannot be evaluated FAILED, all run headless.

| id | axis |
|---|---|
| C-GHOST-LEDGER-FIRST | the intent line hits disk before the write executes; mutation: reorder append and execute, control goes red |
| C-GHOST-FAIL-CLOSED | with an unwritable ledger the poke refuses and renders the refusal; it does not execute-then-warn |
| C-GHOST-RUNG-GATE | a registry entry gated at rung 36 refuses against a rung-30 fixture, headlessly, via `--json` |
| C-GHOST-READBACK | read-back != requested renders MISMATCH red; no path maps mismatch to applied |
| C-GHOST-TAINT-LAMP | nonempty ledger ⇒ header lamp; empty ledger + synthetic reach tag ⇒ still lamp; empty + clean ⇒ "no ledgered pokes", never "no pokes" |
| C-GHOST-NO-LAUNCH | the command table contains exactly the two whitelisted tools; anything else is a compile-time/test failure |
| C-GHOST-CITATION | every registry entry has a nonempty citation and a class; no exemptions |
| C-GHOST-DESTS | the destination selector's MODE table matches the `d1-eboot`-derived generator output byte-for-byte; a greyed entry carries a nonempty refusal reason |
| C-GHOST-DANGLING-INTENT | an intent with no result renders as a loud dangling-write banner, not silence |

## 11. Open questions `[U]`

* Does the two-line ledger need fsync between intent and execute, or is the crash window
  acceptable as rendered evidence?
* AH hot-swap as a registry action: it is a shell class of its own (`nsenter`), v2 at the
  earliest - does it belong in this registry or in the supervisor?
* Whether `d1 show` grows the poke-count rendering in v1 (cheap, and makes the ledger legible
  outside the GUI) or waits for the v2 CLI work.
