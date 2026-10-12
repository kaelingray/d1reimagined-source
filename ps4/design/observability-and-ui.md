# Observability and the interface layer - canonical reference

**State (2026-08-11):** four read-only viewers exist and stay: `d1-trace` (static four-stream run
viewer, v1.1, 10 execution-confirmed defects fixed), `d1-dash` (live dashboard, built and running,
**three blocking review findings R1-R3 open, [U] whether fixed**), `d1-observe` (BAP conversation
debugger, v0.1 validated), and `d1 serve` (served HTML). The Zeek pcap pipeline is built and
controlled (14/14) but has never seen a real D1 capture. The CLI+TUI+GUI architecture doc remains
design of record; its egui trigger fired and was discharged three times. The Python retirement
queue is open: every audited file still exists on disk (verified 2026-08-11 `[C]`).

Consolidates seven design docs; originals at `historicaldocs/design/<name>.md` (Sources, §11).
Sibling live docs: `design/instruments-static.md`, `design/instruments-live.md`.
Frontier: **`design/d1-ghost.md`** (2026-08-11, design only) - the planned merge of the live
dashboard with a registered, ledgered poke console; it deliberately sits OUTSIDE §1's read-only
viewer class and states its own outranking property (no invisible writes).

## Contents

1. [The estate at a glance](#1-the-estate-at-a-glance)
2. [d1-trace - the static four-stream viewer](#2-d1-trace)
3. [d1-dash - the live dashboard](#3-d1-dash)
4. [d1-observe - the BAP conversation debugger](#4-d1-observe)
5. [Zeek - pcap to structured, diffable logs](#5-zeek-pipeline)
6. [Interface architecture - design of record](#6-interface-architecture)
7. [egui practices at the 0.36.1 pin](#7-egui-practices)
8. [The Python retirement queue](#8-python-retirement-queue)
9. [Open contradictions](#9-open-contradictions)
10. [Dead claims](#10-dead-claims)
11. [Sources](#11-sources)

---

## 1. The estate at a glance

| tool | question it answers | code | status | controls | reach |
|---|---|---|---|---|---|
| `d1-trace` | one time-ordered, searchable view of a finished run's four streams | `d1-trace/` (standalone crate) | v1.1, adversarially reviewed, 10 defects fixed | `d1-trace controls` - 46 (21 base + 25 C-DASH-*), exit 2 = do not cite | tooling-only, read-only; adds no `synthetic_ancestor` taint |
| `d1-dash` | which rung of the 37-rung ladder the *live* client is on, what refused, who answered | `d1-trace/src/dash/` + `bin/d1-dash` | built and running; **R1-R3 blocking, [U] whether fixed** | same command (the 25 C-DASH-* live there) | read-only, never writes to the guest |
| `d1-observe` | BAP head-of-line queue state, why-stuck, A/B divergence | `d1-observe/` | v0.1 validated 2026-08-07 | 11/11 golden tests, both feature sets | tooling-only, read-only |
| `zeek-logs.py` | what CHANGED between capture A and capture B (endpoints, DNS, TLS, certs) | `capture-rig/zeek-logs.py` + pinned container | built and controlled; **never run on a real capture** | `--self-test` - 14 positive+negative, exit 2 = do not cite | output will be `observed-capture`; today `[U]` about Destiny |
| `d1 serve` | served-HTML run browser (boot ladder N/14, halt banner, provenance) | `d1/` | built 2026-08-06 | rendered from the same summaries the CLI prints | read-only |
| `pcap-summary.py` | is our MODEL of the client right (eboot hostnames vs capture)? | `capture-rig/pcap-summary.py` | in the port queue (§8, Tier 3) | none ⚠ | read-only |

**Standing property, all viewers:** they run nothing, POST nothing, write nothing into any client.
Opening a run cannot change a gate. That property outranks any feature.

---

## 2. d1-trace

**Status:** v1.1. v1 was adversarially reviewed 2026-08-08 and **10 defects were confirmed by
execution; all 10 are fixed and each fix is proven by execution** (§2.4). Four made the tool assert
things that were not true (inverted timeline, invented cross-stream order, silently emptied lane,
silently zeroed anchor set), which is worse than not having the tool. **Run `d1-trace controls`
before citing anything this tool printed.**

### 2.1 The four streams, and what actually orders them

| stream | source in the run dir | count/boot | native order key |
|---|---|---|---|
| HTTP | `responder.log` (ANSI) | ~8 endpoints | line position |
| BAP frames | `responder.log` (ANSI) | ~171 frames | `msg#N` outer + inner `seq` |
| UD2 probe records | `account-trace.bin` (+ `.blob`) | thousands | record `sequence` (monotonic) |
| guest diagnostics | `emulator.log` | ~140 interesting | line position |

⚠ **There is no shared wall clock - measured, not assumed `[C]`.** On
`runs/20260808-124109-WORLD-LAUNCH-1080`: `responder.log` and `emulator.log` carry **no timestamps
at all**; only `shadnet.log` (not one of the four streams) has ISO timestamps. A global wall-clock
merge is fabricated alignment. Instead every event carries an ordering confidence, shown per row
with a legend:

- **`Exact`** - probe records (`order = sequence`, the spine) and BAP frames anchored by matching
  `(inner msg_type, sequence)` against the guest's own `secured_rx_dispatch_entry` /
  `bap_rx_dispatch_entry` probes - a real client-server join.
- **`Bracketed`** - responder events strictly between two anchored neighbours in `responder.log`
  order. Only true *between* two anchors; head and tail are extrapolations (fix F1).
- **`Approx`** - guest diagnostics, unanchored frames, anything outside the outermost anchor.

⚠ **No spine ⇒ no unified timeline `[C]`.** 133 of the 200 run dirs on this box have no usable
spine (65 no `account-trace.bin`, 18 pre-232-byte-era, 50 predate the BAP rx probes). The tool
**refuses** to synthesize cross-lane order: lanes grouped, within-lane native order, every row `≈`,
loud note. Cross-lane order is `not_observed` and it says so (fix F2).

### 2.2 No-format-twice: shell out to the Python, consume `--json`

⚠ **Superseded (2026-08-11): both shell-outs are gone.** `d1-trace` now decodes the 232-byte
record natively (`probe_decode.rs`, the byte-faithful port of `decode-account-trace.py`) and the
`D1BL` blob sidecar natively (`blob_decode.rs`, the byte-faithful port of
`world-launch-command.py`, surfaced as `d1-trace blob`). The paragraph below records the design as
it stood when this doc was written.

`d1-trace` invokes `python3 capture-rig/decode-account-trace.py --json` and
`capture-rig/world-launch-command.py`; it does **not** parse the 232-byte UD2 record in Rust.

⚠ **Correction (2026-08-08): the layout is described THREE times, not once `[C]`** - the same
`struct.Struct("<IHHII15Q48s48s")` appears in `capture-rig/decode-account-trace.py:16`,
`capture-rig/ready-byte-report.py:30`, `capture-rig/gate-check.py:33`. The duplication already cost
a wrong answer: `probe_id` is field index **2**, not 3; index 3 returns the constant `232` for
every record, so every probe reads `NEVER FIRED`. A Rust reader would be a **fourth** description.
Collapsing the three onto one module is filed, not done. The two log streams are not a second
format description - they are text the responder/guest already decoded and printed; no BAP or HTTP
wire bytes are parsed by any viewer. BAP semantics stay with `d1-observe`.

### 2.3 Search

Flagship query, confirmed on real data (`runs/20260808-110410-AVAIL-ITEM-TRACE`): did
`item_load_state_primary` ever return 2? It did, at seq 2596 and 5066, and the preceding
`item_tag_resolved` on that iteration carried `world_tag=0x80a6035a`. Grammar: `probe:<substr>`,
`stream:<http|bap|probe|guest>`, field predicates `<field><op><value>` with optional `&<mask>`
(registers, `seq`, `probe_id`; field predicates match probe events only, e.g.
`probe:item_load_state_primary rax&0xffffffff==2`), bare words as substring over summary **and**
detail. A query that cannot match says so: typo'd probe names are checked against the run's real
names, a single `=` is an error not a degraded search, unsatisfiable mask/value pairs warn.
`d1-trace summary` prints all 67 probe names.

### 2.4 The 10 execution-confirmed defects and their fixes `[C]`

Each fix verified by running the tool; F1/F3/F4/F7 additionally mutation-tested (fix reverted in a
scratch copy, matching test confirmed red). **All 10 mutations were caught.**

| # | defect (confirmed by execution) | fix / proof |
|---|---|---|
| F1 | `~ bracketed` asserted unconditionally for HTTP; false for 10 of 18 rows in the pinned run; produced a causally inverted timeline (SignOn placed before the probe its response causes) | `Placement` per event: `Anchored`/`Bracketed`/`Outside`; `Outside → Approx`. 10 pre-anchor rows now `≈`, 349 genuinely bracketed still `~`. Controls `C-NO-FALSE-BRACKET`, `C-SIGNON-ORDER` |
| F2 | no-spine runs collapsed to one order value then tiebreak-sorted into fiction; `20260806-203619-magic-soid-A`: 859 rows, 1 distinct order value; 133 of 200 dirs affected | refusal (see §2.1): same run now 890 distinct order values, 890/890 `≈`, lanes `GUEST 789 · HTTP 47 · BAP 54`. Test `f2_…refuses_to_synthesise_cross_stream_order` |
| F3 | `read_to_string` with `Err(_) => Ok(Vec::new())`: one `0xff` byte ⇒ guest lane 0 events at exit 0 | `fs::read` + `from_utf8_lossy`; errors become loud notes. Same injected file: 789 guest events (was 0). Control `C-LOSSY-UTF8` |
| F4 | anchoring scraped from `explain()` prose; a cosmetic reword took anchors 1061 → 0 at exit 0 with 12/12 tests green | `d1-trace controls` pins the invariant, exit 2. `C-ANCHOR-WITNESS` (witness probe ⇒ anchors > 0), `C-ANCHOR-RATE` (≥ 80% of id-carrying rows). Exact reword reproduced: 17 passed, 4 failed, exit 2 |
| F5 | `strip_ansi` did `bytes[i] as char` (Latin-1 decode of UTF-8); the `⭐` detail arm could never fire | iterate `line.chars()`; ⭐ lines are now timeline rows. Controls `C-UTF8-STRIP`, `C-STAR-ANNOTATION` |
| F6 | every frame emitted twice (unanchorable outer header + inner decode): anchor rate had a 50% ceiling (1061/2342) | header folded into the inner row; denominator = id-carrying rows: recv rows 2342 → 1171, **1061/1171 = 90.6%**. Controls `C-FRAME-FOLD`, `C-FRAME-FOLD-RUN` |
| F7 | search covered summary only; 1,302 responder lines dropped. Misses: `world_tag` 681→0, `Bearer` 6→0, `XETROV` 2→0, `manifest_url` 1→0, `key-only` 2→0 | words match summary and detail; unclassified lines become generic rows. All counts restored; negative control `zzz_not_present` = 0. Controls `C-SEARCH-DETAIL`, `C-WORLD-TAG`, `C-RECOVERED-LINES` |
| F8 | decoder failure refused the whole directory (18 of 133 runs carry a pre-era trace) | graceful path with decoder stderr as note. `20260807-070417-account-trace` now opens: 903 events, exit 0. **Sweep: all 200 run dirs open, 0 refusals.** |
| F9 | three silent query false negatives (typo'd probe name, single `=`, unsatisfiable mask) | all three emit a `warning:` naming the query. Controls `C-QUERY-TYPO`, `C-QUERY-UNSAT` |
| F10 | every BAP row labelled `recv`; `BapDir::Send` never constructed; 1,174 `--> BAP … REPLY` lines swallowed - the server-to-client half was absent | `Send` rows inherit the reply identity and anchor independently: **1,174 send rows, 1066/1174 = 90.8%**. Control `C-BAP-SEND` |

What did NOT need fixing (verified, do not "improve"): the `(type, seq)` anchor join (1,174
distinct keys, 0 multi-cluster, first-writer-wins is the right semantic) and the invalid-token
reporting path. Cost of F7, accepted and named: ~126 previously invisible responder rows on the
pinned timeline (HTTP rows 18 → 39); a recovered line the operator can see beats a deleted one.

### 2.5 Controls and verification

```sh
d1-trace controls          # exit 0 = OK, exit 2 = a control FAILED, output may not be cited
```

Originally **21 controls in ~1.2 s** (measured `[C]` 21 passed, 0 failed, 1.18 s, exit 0); now
**46** after the 25 `C-DASH-*` joined (§3.6). Of the base 21:

- **9 self-controls** (fixtures in a temp dir, run anywhere): `C-UTF8-STRIP`, `C-SEARCH-DETAIL`
  (with negative), `C-QUERY-TYPO`, `C-QUERY-UNSAT`, `C-NO-SPINE`, `C-LOSSY-UTF8`, `C-BAD-TRACE`,
  `C-FRAME-FOLD`, `C-RECOVERED-LINES`.
- **12 pinned-run controls** against `runs/20260808-110410-AVAIL-ITEM-TRACE` (closed, exercises all
  four lanes): `C-PROBE-COUNT` (17862), `C-GUEST-COUNT` (3648), `C-ANCHOR-WITNESS`,
  `C-ANCHOR-RATE`, `C-SPINE`, `C-NO-FALSE-BRACKET`, `C-SIGNON-ORDER`, `C-FRAME-FOLD-RUN`,
  `C-BAP-SEND`, `C-WORLD-TAG`, `C-STAR-ANNOTATION`, `C-FLAGSHIP` (hits at probe seq 2596, 5066).

Discipline pinned into the command: a control that cannot be evaluated is a control that **FAILED**
(missing pinned run ⇒ exit 2, not silence); every scan control carries a positive control, and
degradable ones a negative too; the F4 controls are unit-tested *for their red* (a synthetic broken
trace makes `C-ANCHOR-WITNESS` go red, a healthy one keeps it green, and the old F6 denominator
2342 is pinned to fail `C-ANCHOR-RATE`).

v1.1 verification `[C]`: `cargo test --release` 34 tests, 0 failures (25 unit + 9 end-to-end, up
from 12; all 12 originals still pass); all 10 mutations caught; full sweep 200/200 dirs open, 0
refusals, 67 with a real spine, 133 correctly `⚠ NOT OBSERVED`; GUI links eframe/egui `=0.36.1`
unchanged. Standalone crate outside the `d1-server` workspace, mirroring `d1-observe`: sidesteps
the feature-unification trap (whole-workspace vs `-p` builds produce different binaries) and pins
the egui set independently.

---

## 3. d1-dash

**Status: BUILT AND RUNNING (2026-08-09), then adversarially reviewed the same day. The review
confirmed 3 blocking defects (R1-R3) and its verdict is that they must be fixed, each with a
control that goes red against the reviewed code, before `d1-dash` is used to decide anything.**
`[U]` whether R1-R3 have been fixed since - no later record was found in the source docs.

```sh
python3 -B capture-rig/state-machine.py --json >/dev/null   # sanity: the ladder loads
./d1-trace/target/release/d1-dash                            # newest run under ./runs
./d1-trace/target/release/d1-dash runs/<run>
./d1-trace/target/release/d1-dash --json board <run>         # the exact struct the GUI renders
./d1-trace/target/release/d1-dash --json board --live <run>  # + one poll of the live client
./d1-trace/target/release/d1-trace controls                  # 46 checks (21 + 25 C-DASH-*), exit 2 on failure
```

At delivery `[C]`: six panels all rendering real data (verified headlessly under Xvfb); live
byte-offset tailing works (1,800 → 2,060 guest lines over 25 s with the client running);
`d1-trace controls` 46 passed, 0 failed, 4.8 s, exit 0; `cargo test --release` in `d1-trace` 58 + 9
passed, 0 failed, including a RED mutation test per control. The reviewer independently re-measured:
builds green in `d1-trace` and the `d1-server` workspace, zero warnings; controls 46/0 in 4.81 s,
also from `/tmp`; `cargo tree` shows 0 wgpu edges for `d1-trace`.

### 3.1 Purpose, and what it is NOT

> Show, live and without stopping the client, which rung of the 37-rung launch ladder the game is
> on, what refused to happen, and which of our systems answered, with every number carrying the
> file and line-shape it was scraped from.

Not a control surface (never writes, POSTs, or launches - read-only is what keeps it out of the
`synthetic_ancestor` chain). Not a protocol model (BAP semantics belong to `d1-observe`; do not
reconstruct client queue state from responder logs). Not a merged timeline (`[C]` zero timestamps
in `responder.log`/`emulator.log`; `shadnet.log`/`activity-host.log` are RFC3339 UTC; `d1guest.py`
stamps local - one observed 8 h AKDT offset reads as a next-day run). Not a log tail with colours
(`[C]` 89% of `emulator.log` is `Render.Vulkan` spam; the guest's narration is 772 `[Tty]` lines =
0.37% of 206,783). Not an error monitor (⛔ four of the 2026-08-08 session's most expensive
failures produced **no error line at all**; rung 35 `activity:physics_join` is a documented silent
150-tick hang with no timeout and no `error.*`).

### 3.2 Where the code lives, and why

A second binary inside the existing `d1-trace` crate (`src/dash/{tail,probe,reduce,panels,theme}.rs`
+ `bin/d1-dash.rs`), sharing `d1_trace::{ingest,model,query}` and the `controls` command. Not
`d1-forge`: that is a *write* tool, and `[C]` it links wgpu 30.0.0 (bare `eframe = "0.36.1"`
default features; `strings | grep -c wgpu` = 5,459; 29.4 MB binary vs 18.6 MB for the glow-pinned
crates) - adding a dashboard there drags wgpu into every whole-workspace `d1-server` build. Not a
new mode on the `d1-trace` viewer binary: that viewer asserts its own idleness (no
`request_repaint` anywhere, on purpose); a live tail needs producer-side repaint, so it gets a
separate event loop over the same library.

### 3.3 The three cross-cutting rules, and the six panels

**R1 - three states, and grey must never render as green.** Every cell is ✅ OK (observed healthy),
⚠ PASSED EMPTILY (mechanism ran, carried nothing), or ⬜ NOT OBSERVED (never read, or instrument
could not read). ⬜ and ✅ differ in shape as well as colour - the project's signature false
negative is `d1guest.py` returning zeros, not an error, for a wrong address class.

**R2 - every number names its source.** Hover carries file, exact line-shape, and
verbatim-vs-derived; one click to the raw unmodified line. A log line is a rendering, not a value:
a byte-reversed printf (`AH IP resolution successful: 'proxy -> 2.0.77.10:3600'`) once caused a
correct encoder to be "fixed".

**R3 - staleness is a value.** Per-lane last-update stamp and stale badge. `[C]` `sockets.log` is
a 60x5s `ss` loop that stops after exactly 300 s; `activity-host.log` freezes when the AH
connection closes; neither has an in-band went-quiet marker.

| panel | shows | key wrong/empty/stale rules |
|---|---|---|
| D1 ladder | rungs 1→37, current rung big, **furthest** rung as headline; denominator shelled out from `capture-rig/state-machine.py --json` (35 handler classes, rung ids 1-37, 15/16 absent) | depth is FURTHEST not LATEST; `cleanup` (rung 25) is a normal transit state (⛔ but see R1 review finding); zero state lines is an INSTRUMENT FAULT, never rung 0 (the two-gate trap: 626 of 1,614 sites pass the filter and still emit nothing) |
| D2 error tape | every `error.*` with the states it left/entered, plus codename (weasel = `error.lost_bap_connection`, elderberry, currant, termite) | ⛔ "errors: 0" must never render green alone; empty state names the current rung's expected failure signature |
| D3 header | run id, reach tag, `Quiet::{Yes,No,Unknown}` via `d1_run::Provenance::quiet`, containment, guest halt, `d1guest` liveness ladder L0-L3, controls verdict, per-lane staleness | `Unknown` ≠ `Yes` - a determinism claim from a run with no host-quiet evidence is `[O]`, never `[C]`; below L3 memory tiles grey; `synthetic_ancestor` is permanent chrome (⛔ but see R8: derive it from the reach tag, do not append unconditionally); ⛔ halt detection must read BOTH `guest-crash-report/crash_info.txt` AND `uploads/ticket_drop-*.bin` (`[C]` `runs/20260808-172032-ACTIVITY-LADDER` has only the latter and `d1 show` reports `guest_halt: None` over a real crash); controls exit non-zero ⇒ banner + grey (⛔ but see R6) |
| D4 flag strip + KTOBUCKET | `[AC PRIVATE CURRENT CON-Y EST-N AH->… MEM-0]` as six labelled lamps; latest KTOBUCKET block verbatim, unparsed, monospace | KTOBUCKET fires only on a state error - an autopsy, not a heartbeat; `CON-Y EST-N` is the 2026-08-08 failure shape; `AH->0` vs `AH->--------:--------` never collapsed; ⚠ do not parse the numeric interior (`[qlen r/t 8/-273202064]` is the client's own printf garbage) |
| D5 QueueZ families 0-5 | per family: type, root SOID, replication state 1/2/3/4, version, object count; authoritative source `python3 -B capture-rig/d1guest.py descriptors` (`[C]` 0.11 s warm, plain `/proc/<pid>/mem`, no ptrace stop); corroborating log lane rendered as a SEPARATE column | ⭐⭐ `state=3` or it did not happen - state 2 is written locally at send time, so a green may come from `descriptors` only; `object_count 0` is the third state; a `(type,root)` miss returns 0 with no string; every read is a torn single sample, never smoothed; heap pointers are ABSOLUTE (`--abs`), only eboot vaddrs get `+0x800000000`; call `d1guest.py` as a subprocess, never read `/tmp/d1guest-base.json`; do not encode "a timed-out family refuses all future updates" as a verdict - it is an unmeasured annotation |
| D6 TTY tape | guest narration only, ring buffer with hard cap, subsystem chips, `d1_trace::Query` as filter (⛔ see R9) | filter to `[Tty]` at ingest; never `read_to_string` (`[C]` 38.9 KiB/s growth while idle, 234 MB largest file, 6.4 GB across 259 runs); `fs::read` + lossy; iterate chars; strip ANSI at ingest |

Data-layer rules: byte-offset tail with held partial-line remainder; never `d1 serve`'s
`/runs.json` shape for the live page (`[C]` 15.9 s, 6.4 GB read, ~14 s user CPU per request); live
client touched only through `d1guest.py` as a subprocess at 2-5 s; **nothing is written, ever**;
every channel gets an explicit terminal condition (a dead tailer renders as a domain fact).
`d1-dash` describes zero formats: run-log text is scraped renderings, BAP is delegated to
`d1-observe --json`, the UD2 record and the 37-rung ladder are shelled out to.

### 3.4 The four recorded deviations from spec

1. **The pinned control run changed.** The spec pinned `runs/20260808-234535-CODEC-LIVE2`, which
   was still live (re-measured 2026-08-09: 869,352 raw lines and two `error.*` against the spec's
   206,783-line snapshot and one). A control artifact must be closed. Pinned instead:
   **`runs/20260808-210910-AH-RAWCODEC`** - closed, byte-for-byte the measured properties: 25
   `Entering`, 22 distinct states in the same order, furthest rung 30
   `setup:activity_host_setup`, last state `setup:orbit` (27), exactly one `error.*`, 15 KTOBUCKET
   lines, `quiet == No`. `C-DASH-LADDER-PIN` still red against the old code: `d1 show` prints
   `gate=prepare_for_orbit` - measured, not assumed.
2. **`C-DASH-STALE`'s axis is relative, not wall-clock** - staleness measured against the newest
   lane in the same run (on the pinned run: `sockets.log` 466 s behind ⇒ stale; `emulator.log` 1 s
   ⇒ not), with absolute age carried alongside. (⚠ but see R5.)
3. **`--json board` does not poll the live client unless `--live`** - otherwise the determinism
   control would measure whatever process happens to exist. A closed run opened while a different
   client is live carries "⚠⚠ TWO DIFFERENT PROCESSES ON ONE PAGE".
4. **No `egui_kittest` smoke test** - headless verification by running the real binary under
   `xvfb-run` instead; the AccessKit tree is a named gap, not claimed done.

Also built beyond spec: `C-DASH-GREY-VS-LIVE`, because `C-DASH-GREY-NOT-ZERO` as specified can be
satisfied by an always-grey table - a control that only ever passes is decoration.

### 3.5 Scope cut (v1) and the panels deliberately NOT built

v1 is six panels, one binary, one window, no plots, no config, no network. Out, each with its
recorded reason: `egui_plot`/charts (resolver trap, §7.2); `egui_tiles`/docking; run picker
(15.9 s / 6.4 GB per tree summary); A/B diff (`d1_run::diff` exists); BAP queue panel
(`d1-observe` owns it); AH conversation panel (regex over a Rust `Debug` rendering breaks
silently - blocked on a JSONL sink request to that crate's owner); named-gate board (blocked on a
one-line zero-body `warn!` in `d1-responder`, owned by another agent); silent-success board
(S1-S21 / L1-L19, v2 with its own controls); ui_stage lane (⛔ three docs give three different
substage mappings - print the raw `ENUM(N)`); live `ss`; live probe lane; config/theming/keybinding
systems (not until a third human asks); any write path (permanent, not deferred).

Open handovers to other lanes: `d1-responder` zero-body `warn!` + JSONL alongside prose;
`d1-activity-host` JSONL sink + `blocked_pushes` at close; make `d1_run::strip_ansi` `pub`
(⚠ not done as of the review, `d1-trace/src/ingest.rs:35` has its own copy).

### 3.6 The C-DASH controls

Live in `d1-trace controls` as `C-DASH-*` (25 shipped; the spec named 17, some grew paired reds).
Meta-rules that outrank the list: state the axis in words before writing the control; every
control ships a mutation test that breaks the thing on purpose; a control that cannot be evaluated
FAILED; controls run against `--json board`, headless.

| id | axis |
|---|---|
| C-DASH-LADDER-PIN | headline reflects the ACTIVITY ladder, not the obsolete 14-state bootflow spine (was red against pre-dash code: `d1 show` printed `gate=prepare_for_orbit` for a rung-30 run) |
| C-DASH-FURTHEST-NOT-LATEST | depth is `max()` not `last()`; a run reaching 30 then re-entering `bootflow:start` still reads 30 |
| C-DASH-CLEANUP-TRANSIT | `cleanup` entered for `normal_flow` is not failure and does not cap depth (⛔ half-axis - see R1) |
| C-DASH-ERROR-PIN | finds real errors and only real errors; `error.zzz_not_a_real_reason` matches 0 (⛔ under-match unguarded - see R2) |
| C-DASH-INSTRUMENT-FAULT | zero state lines ⇒ instrument fault, never rung 0 (the two-gate trap) |
| C-DASH-THREE-STATE | Ok / Empty / NotObserved serialise to three distinct tokens; no path maps NotObserved → Ok |
| C-DASH-GREY-NOT-ZERO | with no live pid, no memory tile renders a literal `0` |
| C-DASH-TAIL-SPLIT | a line split across two polls arrives exactly once, byte-identical |
| C-DASH-LOSSY-UTF8 | a mid-multibyte snapshot keeps every other line and carries a loud lossy note |
| C-DASH-UTF8-STRIP | `⭐` survives ANSI stripping through the dash ingest path |
| C-DASH-NO-MERGE | zero cross-source order keys; within-lane ordinals strictly increasing (⛔ never evaluated on the tailed path - see R3) |
| C-DASH-STALE | frozen (`sockets.log` at 300 s) distinguishable from quiet (⚠ mtime-fragile against `touch`/`rsync`) |
| C-DASH-HALT-BOTH-PATHS | a crash with only `uploads/ticket_drop-*.bin` renders as detected, not "no halt" (was genuinely red against `d1_run`'s single path) |
| C-DASH-PROVENANCE-COMPLETE | every `Board::Row` has non-empty source file + line-shape + Verbatim/Derived discriminant, no exemptions |
| C-DASH-LADDER-DENOM | rung table byte-equal to `capture-rig/state-machine.py --json`; fails the day either side drifts alone |
| C-DASH-QUIET | pinned run ⇒ `No` with reason `loadavg-1=11.95 exceeds 8.00`; no PSI lines ⇒ `Unknown`; `Unknown != Yes` in value and rendering |
| C-DASH-GREY-VS-LIVE | a successful descriptor poll produces 2 OK tiles (grey is not the only outcome) |

### 3.7 The adversarial review, 2026-08-09 - what is still open

> ✅ **R1-R3 FIXED 2026-08-11** (the d1-ghost v0 precondition). Each fix carries a RED control in
> `d1-trace controls`: **C-DASH-CLEANUP-FLOOR** (furthest excludes `cleanup` and any state entered
> for an `error.*` reason; `20260806-140313-signon-f2` reads rung 13, not 25), **C-DASH-ERROR-BOTH-FORMS**
> (entering-form errors recorded and de-duped; that run reads 2 errors, codename `termite`),
> **C-DASH-TAIL-ORDINALS** (`Batch.first_line_no` stamped by the producing `Tailer::poll()`; a
> two-Tailer GUI-style fixture stays strictly increasing, no `u64` wrap). The blocking text below is
> preserved as the record of what was wrong.

Nine of thirteen findings reproduced with a command or program. **Blocking (fix before deciding
anything with this tool):**

- ⛔ **R1 - the headline rung is inflated by `cleanup`, and it is green `[C]`.**
  `d1-dash headline runs/20260806-140313-signon-f2` prints `furthest rung 25 of 37 - cleanup ·
  errors 0` for a run whose true deepest state is `bootflow:content_check` (**rung 13**), died
  twice on `error.content_manifest_not_provided`. `cleanup` is both ladder position 25 and the
  universal failure sink, so every failed run reports at least 25 - a floor exactly on the runs
  that failed. `C-DASH-CLEANUP-TRANSIT` tested only that cleanup must not *cap* `max()`, not that
  it must not *raise* it. Fix: depth = `max()` over rungs entered for a non-`error.*` reason,
  `cleanup` excluded outright, plus the red fixtures (13 stays 13; 30 stays 30).
- ⛔ **R2 - errors reported only on the `Entering` form are invisible: "0 errors" over a named
  gate failure `[C]`.** Early-boot gate failures print only `Entering 'cleanup' … 'error.X'`;
  the tape pushes an `ErrorEvent` only in the `!entering` branch. Measured scope: eight runs, all
  boot-gate failures, showing 0 errors while the KTOBUCKET panel displays the error on the same
  screen (`error.transport_method_invalid` x2, `error.bungie_server_connection` x3,
  `error.signon_error` x2, `error.content_manifest_not_provided` x3 across
  `20260806-{131007-signon-redirect,131658-nolimit-route,132255-driven-signon,133407-signon-answered,133739-probe-varint,140313-signon-f2,141341-signon-repro}`,
  `error.content_manifest_url_invalid` in `20260806-143130-manifest-url`). Fix: record from either
  direction, de-dup on `(reason, adjacent line)`; control pinned to `20260806-140313-signon-f2`:
  2 errors, codename `termite`.
- ⛔ **R3 - live-tailed line numbers go backwards, and underflow to ~1.8e19 `[C]`.**
  `Session::absorb` derives ordinals from a `next_line_no()` frozen at load time (the GUI spawns
  its own `Tailer`); tailed lines collide and go backwards, and on a short/empty `emulator.log`
  `u64` wraps (`line_no=18446744073709551614`) - release wraps silently, a debug build panics.
  `C-DASH-NO-MERGE` evaluates false on a tailed board but only ever runs against closed runs.
  Fix: keep the counter with the reducer or have `Batch` carry its starting ordinal; add a
  two-batch monotonicity control.

Non-blocking, still real: **R4** every board rebuild re-reads and re-parses the whole
`emulator.log` on the UI thread, and the app never idles (measured: 21-25% of a core forever on a
closed 32 MB run; 94-99% + 260 MB RSS on a live 131 MB run; `d1 show` on that run takes 0.75 s
against a 250 ms tail period). **R5** relative staleness cannot see the client dying - every lane
stops together, every chip renders ✅; keep the relative measure and add wall-clock age on the live
board. **R6** the controls-failed banner promises "every derived tile greys" and nothing greys.
**R7** `[I]` the corroborating-only QueueZ log column can paint green from a send-side line alone,
and mixes `last()` object_count with sticky `any()` flags. **R8** `synthetic_ancestor` is appended
to whatever reach tag the run recorded - correct today by accident, wrong the day an
`observed-capture` run exists; derive the suffix from the reach tag. **R9** the D6 filter box is a
naive substring, re-creating F9b one module over, and `dash::reduce` duplicates
`d1_run::parse_boot_state` and the guest-marker literal. **R10** `instrument_fault` fires for the
first ~1,400 raw lines of every healthy run (first `[stdout]` line at raw line 1420 in the pinned
run); gate on elapsed time or a byte floor. **R11** smaller: `d1-forge` still drags wgpu into
whole-workspace builds (13 wgpu edges, 6 compiled wgpu dirs - `[C]` still open 2026-08-11, its
`Cargo.toml` has no `default-features = false` on eframe); per-line `to_ascii_lowercase()` in the
render path over up to 20,000 lines at 4 Hz; `C-DASH-STALE` mtime fragility; filter box has no
hover text; `probe::parse` drops `BUILD GATE:`/`BASE CONTROL:` lines; `zip_entry_names` scans for
`PK\x01\x02` anywhere including DEFLATE bodies.

Verified sound by the reviewer (tried to break, could not): the builds and control counts above;
the egui-practices retractions (§7) confirmed against vendored sources; `all_styles_mut`, fonts at
front of family, `show_rows` in all three list panels, bounded `VecDeque` with reported drop count,
producer-side repaint after send, panels before `CentralPanel`; the three-state rule on probed
paths (a run with no `emulator.log` renders `NOT_OBSERVED` and not one literal `0`);
`C-DASH-HALT-BOTH-PATHS` genuinely red against `d1_run`; the RED mutation tests are real; the
`Tailer` itself is correct (R3's bug is in the caller); D4 verbatim as specified; preconditions
discharged - 42 files staged (`d1-observe` 16, `d1-trace` 21, `d1-forge` 5), `.gitignore` gained an
unanchored `target/` (without which staging would have added 12,257 files), `d1-forge`'s caret pin
corrected to `=0.36.1` (`[C]` confirmed in-tree 2026-08-11).

---

## 4. d1-observe

**Status:** v0.1 implemented and validated 2026-08-07. **Code:** `d1-observe/`. **Reach:**
tooling-only, read-only over preserved run evidence.

A standalone Rust replay debugger: consumes the existing send/receive JSON event envelope
(`capture-rig/send-receive-event.schema.json`), deterministically reduces it into protocol state,
and exposes the same reducer through JSON/human CLI commands and a native egui replay GUI
(virtualized semantic timeline, in-flight queue, explanation, boot/world/QueueZ state, A/B
divergence). It does not launch shadPS4, send packets, change the responder, pause the guest or
write memory.

### 4.1 The corrected head-of-line model

Two separate queue incidents, the distinction executable in the golden tests:

1. Historically, unanswered `0x000C` occupied the queue head; its confirmed `0x000D` reply retires
   it even though the response handler is a semantic no-op for replication state.
2. In `20260807-165310-operator-504`, `0x000C` was already answered. The head was `0x0020` seq 10,
   then `0x0015` seq 11, investment 701 seq 12, investment 504 seq 13. The later 504 response
   failed recipient matching because the client compares only against the head (`0x0020`). The GUI
   opens on exactly this diagnosis:

```text
HEAD  inner 0x0020   seq 10   expected unknown
      inner 0x0015   seq 11   expected unknown
      investment 701 seq 12   expected 0x000B
      investment 504 seq 13   expected 0x000B
```

Positive-control twin: `20260807-170537-operator-queue-drained` (0x000C answered, 24.6 KB / 191
messages, connection open, zero `error.*`); fixture `post-suback-clean.jsonl` reaches the orbit
world and `character:signin` with an empty modeled queue. Older blocked controls:
`20260807-161008-roster-account-2`, `20260807-162406-silent504`. During acceptance, source-run
names in event `details` polluted the semantic fingerprint (A/B falsely diverged at event 0); the
mapping moved into the evidence note, and first divergence is correctly event 1. This corrects an
earlier compressed retelling that attached the 504 rejection directly to an unretired 0x000C - the
law (strict head-of-line reply matching) survives; the head identity differs.

### 4.2 Event contract and reducer invariants

Additional semantic stages recognized: `client_request_queued` (with
`details.expected_reply_type` only when the client proves it), `client_reply_matched` /
`client_request_retired`, `client_no_recipient`, `client_request_timeout` / `connection_timeout`,
`client_consumer_result`, `queuez_object_inserted`, `client_family_update_result`,
`predicate_evaluated`, `manual_memory_write`, `guest_fault`, `state_entered`, `world_changed`.

The tool never infers that a server-received request entered the client's in-flight queue; a
legacy run without queue probes gets `why-stuck` = instrumentation absent, not blame on the last
visible message. Global cross-source order is `not_observed` unless every event carries a
nondecreasing monotonic timestamp. Invariants: only the head can retire; a later matching sequence
behind a different head is head-of-line blocking, not evidence the later body was decoded; unknown
expected reply types stay unknown; object insertion + family refusal reduces to `partial_failure`;
missing instrumentation is `not_observed`, never a red failure; a manual write cannot promote a
guessed concrete type; reduction is deterministic (byte-identical JSON for the same input).

The shared core includes the six-byte BAP outer-frame accumulator (bytes 2..6 big-endian declared
length; frame = `6 + declared_length`; retains incomplete tails, drains coalesced frames, rejects
impossible lengths without draining, owns no GCM/nonce semantics) - protecting the "one TCP read
is one frame" regression without describing BAP a second time.

### 4.3 The selection-node incident (golden fixture)

All three observed accessors agree on the node layout: tag byte `+0x00 = 3`, nonzero selection key
from `+0x01`, account SOID at `+0x10`, ready byte `+0x18 = 1`, qword `+0x20 = 0`. Accessor
`0x5853C0` returns `[node+0x20]`; gate `0x78C5F0` tests that return for nonzero. The predicate
observed zero and required nonzero; the field is bounded to a resolved-object pointer with concrete
type and legitimate writer unknown; a character-SOID guess was manually written; the guest fault
followed at `0x10875B5` (`movzx esi, word [r15]`) with derived `arg0` null - which distinguishes a
real resolved object from a merely nonzero scalar. The reducer reports the predicate failure as
the earliest blocker and does not promote the guessed SOID into the field contract.

### 4.4 CLI, GUI, validation

```sh
d1-observe/target/release/d1-observe --json validate SOURCE
d1-observe/target/release/d1-observe --json timeline SOURCE
d1-observe/target/release/d1-observe --json queue SOURCE --at EVENT_OR_INDEX
d1-observe/target/release/d1-observe --json explain SOURCE --at EVENT_OR_INDEX
d1-observe/target/release/d1-observe --json unanswered SOURCE
d1-observe/target/release/d1-observe --json why-stuck SOURCE
d1-observe/target/release/d1-observe --json first-divergence RUN_A RUN_B
d1-observe/target/release/d1-observe export-incident SOURCE --at EVENT_OR_INDEX
d1-observe/target/release/d1-observe gui SOURCE [--compare RUN_B]
```

`SOURCE` is normalized JSONL, the Python conversation ledger's JSON, or a legacy run directory
(discovers and invokes `capture-rig/send-receive-ledger.py --json` read-only, limitations
preserved). Validation on `.84`, Rust 1.97.1: `cargo test` **11/11 golden tests** with the GUI
feature; `cargo test --no-default-features` **11/11** CLI-only; release build pass; Xvfb launch +
1500x900 screenshot pass, no stderr (`d1-observe/docs/head-of-line-golden.png`); legacy run
`20260807-165310-operator-504` imports 97 events with explicit warnings; two independent release
`--json validate` runs byte-identical. Release binary SHA-256 at validation:
`0b84d392ab58d47c505f50baf8ce346a4b77e84383b21a5e52d37270cd850e61`. `cargo fmt`/`clippy` were
unavailable on the host and reported as such rather than silently omitted. `why-stuck --json` is
always an object; with missing instrumentation it returns `proven: false` + reason, never a bare
JSON `null`. Cargo resolved eframe/egui 0.36.1 on 2026-08-07; the first compile rejected the older
Context/SidePanel sketch, corrected against the downloaded crate source; glow backend, no wgpu.

**Next integration boundary:** exact diagnosis of historical native runs needs the client BAP
probes to emit queue insertion, live expected type, head comparison and retirement into the
normalized JSONL - land at the probe/event-producer boundary, only when its owner is clear.
⛔ **Do not compensate by reconstructing client queue state from responder logs** - that rebuilds
the confident-but-wrong attribution this tool exists to prevent.

---

## 5. Zeek pipeline

**Status: BUILT and CONTROLLED, 2026-08-08.** Pinned podman container; wrapper
`capture-rig/zeek-logs.py`; 14 positive+negative controls pass in ~2 s on a control pcap this box
manufactures for itself. **Never yet run on a real D1 capture - there are none** (`captures/` is
empty). The *pipeline* is `[C]`; everything it will say about Destiny is `[U]` until the console is
on the wire. Reach tag for anything it ever produces: **`observed-capture`**.

### 5.1 Why, and the question it was built for

"Diffing two Zeek log sets between runs is a real answer to 'what changed.' Diffing two pcaps is
not." The target question is the **Activity Host** - a server role never observed. The client's
RTTI names `client_to_bap_get_activity_host_proxy_request` and
`bap_to_activity_host_socket_handoff_request`; the predicted shape is: BAP connection exists,
client asks BAP for a host, a second connection appears to a different endpoint. Everything that
prediction needs is metadata: does a new connection open at launch to a different IP than BAP
(`conn.log` + `--diff`); TCP or UDP, which port (`conn.log` `proto`, `id.resp_p`); before or after
the world transition (`conn.log` `ts`, first-contact order); datacentre or residential (`--rdns`,
then `whois`) - the row that decides whether Layer C is a service we write or a peer protocol we
join (D1 used player-hosted physics; `activity:physics_join` is rung 35).

### 5.2 ⛔ Scope boundary - settled, do not re-open

**Zeek is for STANDARD protocols only. No Spicy grammar. No D1-specific analyzer. Ever.** (Per
`design/format-description.md`, the authoritative file; the older "pick deliberately" wording in
`historicaldocs/design/observability-tooling.md` §3 already caused one session to re-argue this.)
What we want (`conn.log`, `dns.log`, `ssl.log`, `x509.log`) needs zero Spicy; Spicy emits C++,
which would put FFI across the most safety-critical boundary; D1-specific parsing belongs in a
Rust `pcap → JSONL` tool linking the same crate the server uses. The image ships Spicy; we do not
load it.

### 5.3 Install and invocation

```bash
capture-rig/zeek-logs.py --pull        # fetches docker.io/zeek/zeek:latest, prints the digest
capture-rig/zeek-logs.py --self-test   # and this is what says it works
```

| pinned | value |
|---|---|
| image | `docker.io/zeek/zeek@sha256:6ce464c9cc63185ac0ef40b86f1150842350895496520d7e6522c1756d07c755` |
| zeek | **8.2.1** (asserted as a control) |
| base / size | Debian 13 (trixie) / 567 MB |

The digest is pinned like the eboot's sha256, for the same reason; if upstream moves, the digest
control FAILS until updated deliberately. Licence `[C]`, verified inside the image: BSD 3-clause -
compatible with GPL-2.0-or-later, and moot besides (separate batch process, no Zeek code enters
the tree). Container flags that are decisions: `--network=none` (the batch step proves it touches
no network) and `--security-opt label=disable` (the SELinux-correct `:z`/`:Z` would rewrite the
label of the operator's capture files on disk - we refuse to mutate an artifact to read it; the
pcap is mounted read-only; without either flag the mount is `Permission denied` `[C]`).

### 5.4 ⭐ The checksum trap the pipeline was nearly built on

Measured on the control pcap: `zeek -r control.pcap` yields only
`conn.log weird.log reporter.log`, exit 0; `zeek -C -r control.pcap` yields
`conn.log dns.log ssl.log x509.log`, exit 0. **Zeek discards every packet with a bad checksum by
default**, and NIC checksum offloading makes locally-taken captures full of them - the protocol
logs are not empty, they are *absent*, while `conn.log` is still written, so the run looks
successful. That is the project's signature failure mode in the first tool pointed at a capture.
So: `zeek-logs.py` passes **`-C` by default**; the negative control asserts the without-`-C`
absence; `--validate-checksums` exists only so the behaviour can be re-measured. `-C` cannot
manufacture content. (Adjacent, handled elsewhere: `/usr/local/sbin/d1-capture-offloads.sh` kills
GRO/LRO/TSO/GSO - that fixes segment-shape distortion, not this; keep `-C`.)

### 5.5 Usage

```bash
capture-rig/zeek-logs.py --self-test                       # run before citing anything
capture-rig/zeek-logs.py captures/<stamp>-<label>/x.pcapng # analyse, report
capture-rig/zeek-logs.py x.pcapng -o /tmp/runA --json      # machine-readable
capture-rig/zeek-logs.py --diff /tmp/runA /tmp/runB        # the instrument
```

Default output `<pcap-dir>/zeek-<pcap-stem>/`; logs are JSON lines by default
(`LogAscii::use_json=T`; `--tsv` gives native format which the tool then refuses to parse rather
than half-support); every output directory gets `ZEEK-INFO.txt` (source pcap + size, digest,
version, argv, checksum mode - provenance travels with the logs); `.pcap` and `.pcapng` both work
`[C]`; one pcap per invocation (`capture.sh` writes three - run three times, do not concatenate);
**exit 2 = a control failed or the run errored, output must not be cited.** Nothing writes to
`runs/`.

### 5.6 The 14 controls, verbatim

`capture-rig/zeek-control-capture.sh` manufactures the pcap entirely inside an unprivileged
user+network namespace (`unshare -r -n`, fresh loopback only - nothing off this box, no sudo, no
route out): a four-record DNS server, an `openssl s_server`, their client, `tcpdump`. ⚠ TLS 1.2 is
deliberate: TLS 1.3 encrypts the server Certificate message, so a 1.3 control would "prove" the
pipeline with the certificate path permanently dead; D1's 2016-era client negotiates ≤1.2 anyway.
Measured 2026-08-08, ~2.0 s wall:

```
PASS  pinned image present (sha256:6ce464c9cc63...)
PASS  image reports zeek 8.2.1
PASS  control pcap generated in a sealed netns (loopback only, no sudo)  [8753 B]
PASS  zeek -C ran
PASS  conn.log has the udp/53 flow, service=dns  [1 flow(s)]
PASS  conn.log has the tcp/443 flow, service=ssl  [1 flow(s)]
PASS  dns.log has the A query for www4.signon.deadorbit.net  [1 record(s)]
PASS  ...and it carries the answer we served (127.0.0.1)
PASS  ssl.log has SNI www4.signon.deadorbit.net, TLSv12, established  [TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384]
PASS  x509.log has the certificate subject (proves the cert path is live)
PASS  ...with the SAN, so a name in a cert is recoverable and not just the SNI
PASS  logs are JSON lines (parseable without a Zeek TSV reader)  [conn=2 dns=1]
PASS  NEGATIVE: without -C, dns/ssl/x509 logs are ABSENT (checksum-offload trap)
PASS  ...and conn.log is still written, so the run LOOKS successful  [exit 0, no error]

ALL CONTROLS PASS - Zeek output from this box may be cited
```

The hostname `www4.signon.deadorbit.net` is a literal only; the self-test performs no external
network I/O.

### 5.7 Diffing, coverage, ownership

Diff keys are structural (endpoints: `proto resp_h:resp_p [service]`, ignoring ephemeral source
ports and byte counts; dns: `query qtype`; ssl: `sni @ resp_h:resp_p version`; x509: subject).
Demonstrated end to end: `+ only in B   udp 127.0.0.2:27015` **is** the shape of the answer. Split
one continuous capture at the world transition rather than restarting it:
`editcap -B "<wall clock>" run.pcapng orbit.pcapng` / `editcap -A "<wall clock>" run.pcapng
activity.pcapng`; the endpoint timeline inside a single run already answers before/after on its
own.

Does `[C]`: `conn.log` (5-tuple, proto, service, duration, bytes, state history), `dns.log`,
`ssl.log` (SNI, version, cipher, established, cert fingerprints), `x509.log` (only when the cert
crosses in the clear, i.e. TLS ≤1.2), `weird.log`/`notice.log`/`reporter.log`. Does not: any
message body (`/SignOn` is HTTPS, BAP is TCP + AES-128-GCM - a passive tap yields ciphertext); D1
framing (§5.2); certs over TLS 1.3; datacentre-vs-residential (that is `--rdns` - ⚠ opt-in, the
one place the tool can reach outside the box - plus `whois`); anything in a contained emulator run
(own netns; Zeek is for the retail-console lane).

`zeek-logs.py` vs `pcap-summary.py` - not redundant, and neither should grow into the other:
`pcap-summary.py` asks "is our MODEL of the client right?" (diffs a capture against hostname
literals compiled into the 1.29 eboot, via `tshark -T fields`); `zeek-logs.py` asks "what CHANGED
between run A and run B?" and must not learn about the eboot. They overlap on which hostnames
appeared, deliberately, from different derivations - **a disagreement between them is a finding.**

Operator playbook when the console is on the wire (⚠ **retail** console lane only -
`rig-mode.sh live` bridges to the internet and the CFW console must never be on it): self-test
first (exit 2 ⇒ stop, do not capture on a broken pipeline); `sudo capture-rig/rig-mode.sh live`;
`capture-rig/capture.sh activity-host`; sit still ~2 min in orbit (run A's material), note the
wall clock, launch the activity; ⚠ read the drop counts capture.sh prints - non-zero means redo
today, not in analysis; then zeek-full with `--rdns`, editcap split, `--diff`, `pcap-summary.py`,
`whois`. Success looks like: `+ only in B udp <addr>:<port>`, first-contacted after the BAP
endpoint, and whois says whose address it is.

Verification-state ledger: Zeek absent from Fedora 43 repos `[C]`; image runs, 8.2.1, BSD-3
verified inside `[C]`; mount needs `label=disable` under SELinux here `[C]`; without `-C` the
three logs are absent at exit 0 `[C]` (asserted as a control); with `-C` all four correct `[C]`
(14/14); `.pcapng` reads `[C]`; `--diff` surfaces a new endpoint `[C]` (manufactured captures);
anything about Destiny's actual traffic `[U]`; whether live 2026 endpoints still negotiate ≤TLS 1.2
`[U]`; whether retail-capture checksums are valid `[U]` (`-C` makes it not matter).

---

## 6. Interface architecture

**Status:** design of record for the interface layer (rev 2, post-adversarial-review), **partly
built** (2026-08-06): the CLI (`d1` - `runs`/`show`/`diff`/`promote`/`serve`, all `--json`) and
`d1 serve` (served HTML on loopback, no framework, rendering the same summaries the CLI prints so
the two cannot drift; boot-flow ladder with depth N/14, guest-halt banner, provenance, attempt
ledger) exist. **The TUI was deliberately not built - its trigger never fired.** The
supervisor-process/HTTP-transport design remains proposal; the current GUI reads run directories
directly.

### 6.1 The decisions (condensed)

Per-run **supervisor process** (spawned detached; CLI/TUI/GUI are all clients); one transport,
HTTP/JSON on a loopback socket per run, remote via `ssh -L`, no auth code; event feed served from
the run's on-disk JSONL event log (`/events?since=<seq>` - lossless, reconnect-proof, no `Lagged`
semantics); 4 crates `core`/`cli`/`tui`/`gui`, `core` never prints or draws; TUI = ratatui
0.30.2 + crossterm 0.29, hand-rolled shallow TEA, full-screen with mandatory `--plain` + non-TTY
fallback; GUI v1 = served HTML, GUI upgrade = egui built the day a timeline/plot/scrub need lands;
`present.rs` chokepoint with generic label/value rows as the schema-churn shield (typed sort keys
arrive at the `present` level when interaction demands them - still one chokepoint); `RawBody`
newtype (`Value` at rest and on the wire, `parse::<schema::vN::T>()` the only door, `.raw()`
greppable and confined to envelope-generic code); testing = `update()` unit tests as the bulk,
`TestBackend`+insta snapshots on chrome only with frozen synthetic fixtures, `egui_kittest` smoke
over the AccessKit tree, no pixel-diff CI. Serve surface is six axum routes (`/`, `/status`,
`/events`, `/runs`, `/diff/{a}/{b}` - axum 0.8 params are `{param}` not `:param` - and
`/control`), port written to `$XDG_RUNTIME_DIR/<tool>/<run-id>/addr`.

Event-loop law (from the rev-1 bug, restated in `d1-dash`'s data layer): **every `select!` source
gets an explicit terminal condition; channel/stream closure is a domain signal, never a
silently-ready branch.** Rev 1 matched `Err` from a broadcast `recv()` into a live arm; at the end
of every run that branch was ready forever and pinned a core. TUI specifics preserved: `q`
detaches (run continues), Ctrl-C cancels; `SupervisorGone` is a rendered finalized banner, not an
error; dirty-gated redraw (~4 fps live, ~0 idle, no spinners ever); tracing to file while the TUI
is active; TUI only when `stdout().is_terminal() && !plain && !json`.

### 6.2 Version-pin verification, and the process lesson

Rev 2's pins were checked 2026-08-06 by `cargo` resolution, not recollection. Its headline
correction was itself wrong: it claimed "eframe 0.35 does not exist; 0.34.2 is current" - false;
`eframe@0.35` resolved and latest was then 0.36.0. ratatui 0.30.2 ✅, crossterm 0.29.0 ✅, axum
0.8.9 ✅. **Process lesson, worth more than the fix: rev 1 was "corrected" into a false statement
by a reviewer working from changelog memory. Pin versions by asking `cargo`, and record the date**
(standing rule 8).

### 6.3 The egui bet: fired and discharged, three times

The trigger ("the first concrete timeline/plot/scrub requirement") **fired 2026-08-07,
operator-authorised**: the BAP conversation debugger's in-flight-queue scrub is a series, not a
table - HTML cannot scrub it. Resolved 2026-08-09 by reading the vendored sources: the pin is
`=0.36.1` across `eframe`/`egui`/`egui_extras`/`egui_kittest`, and **`App::logic` / `App::ui` IS
the current API** (`logic` has a default impl, `ui` is required; `eframe-0.36.1/src/epi.rs`) - the
warning that this split was "0.34-era and likely stale" was itself the stale thing. **Discharged
three times:** `d1-observe`, `d1-forge`, `d1-dash` all ship egui 0.36.1 apps. Anything that reads
as "no GUI exists yet" is history. The 5b shape stands: one plain thread doing blocking HTTP, a
std channel, producer-side `request_repaint()`, no tokio in the GUI, body wrapped in
`CentralPanel` (the `Ui` handed to `ui` has no margin or background).

### 6.4 Standing rules

1. Every feature exists in the CLI/JSON contract before or with any UI surface.
2. Frontends consume `present` output over HTTP. Nothing else. Ever. (⚠ **asserted and unfollowed -
   see §9.**)
3. No pager/search/scrollback reimplementation in the TUI; `less`, `fzf`, `jq`, `difftastic` won.
4. No config files, theming, or keybinding systems until a third human asks.
5. The schema outranks the interface.
6. `Value` at rest, types at reason; `RawBody::parse` is the only door.
7. Every event-loop source has an explicit terminal condition.
8. Pin versions by asking `cargo`, and record the date.

Mapping to reality: run+provenance exists (`run-contained-emulator.sh` → `runs/<stamp>/`); the
event log exists as unstructured text (**the gap - the honest first increment is structured JSONL
events, not the spine**); diff existed as hand-grep when written (since built as `d1_run::diff` /
`d1 diff`); `present`/serve exist only in the built CLI/HTML form, not as the supervisor design.

---

## 7. egui practices

Operator-supplied guidance, corrected in place against the vendored 0.36.1 sources (the reviewer
of `d1-dash` independently re-verified the corrections `[C]`). ⚠ The doc's original header said
"not yet acted on, the trigger has not fired" - dead; see §6.3 and §10. Read this before writing
any egui code. Framing: egui punishes retained-mode habits; the caching/virtualization discipline
*is* its performance model (text layout and tessellation are all-CPU, no retained scene).

### 7.1 API corrections at the 0.36.1 pin (banners, they override every older snippet)

- ⛔ **`Context::style()` / `Context::set_style()` do not exist at 0.36.1** (verified absent from
  `egui-0.36.1/src/context.rs`). Style is per theme. Use **`ctx.all_styles_mut(|style| …)`** -
  the survivors `style_of`/`set_style_of`/`style_mut_of` are one-theme, and
  `global_style`/`set_global_style` silently gives a one-theme-only app. Shipped in
  `d1-server/crates/d1-forge/src/theme.rs` and `d1-trace/src/dash/gui.rs`.
- ⛔ **`SidePanel` and `TopBottomPanel` do not exist at 0.36.1** (verified absent from
  `egui-0.36.1/src/containers/panel.rs`). Use `egui::Panel::{top,left,right,bottom}(id).show(ui, …)`
  with `.resizable()`/`.default_size()`/`.size_range()`; `show_inside` is deprecated in favour of
  `show`. Panels still go before `CentralPanel`.
- ⭐ **`App::logic` / `App::ui` is the CURRENT API** - do not "correct" it back (§6.3). Keep the
  trait impl a two-line shim over `tick()`/`draw()` so `egui_kittest` can drive the app headlessly;
  channel drains go in `tick()`, called from `logic()`.

### 7.2 The version pin

Resolved by reading `Cargo.lock` and vendored sources on this box, 2026-08-09, not release notes
`[C]`:

```toml
eframe = { version = "=0.36.1", optional = true, default-features = false, features = [
    "accesskit", "default_fonts", "glow", "wayland", "x11",
] }
```

`eframe`/`egui`/`egui_extras`/`egui_kittest` all **0.36.1** (`=` pins, glow, no wgpu); accesskit
0.24.1, winit 0.30.13; Rust 1.95+. ⛔ **`egui_plot` and `egui_tiles` do NOT track egui's version,
and "pin them all as a set" is wrong for them `[C]`:** a probe crate with `egui = "=0.36.1"` +
`egui_plot = "0.36.0"` resolved **two** egui crates (0.35.0 and 0.36.1) at exit 0 with no warning,
then failed at compile looking exactly like an API break. The companions for egui 0.36 are
**`egui_plot = "=0.37.0"`** and **`egui_tiles = "=0.17.0"`** (no `egui_tiles 0.36.x` exists).
Neither is in the tree or local cache; adding one is a network fetch and a new edge - if added,
verify with `cargo tree -d` that exactly one `egui` resolves. Caret pins float on `cargo update`;
use `=` (d1-forge's caret was the odd one out, since fixed).

### 7.3 The disciplines

**Look:** kill the stock style once at startup (`all_styles_mut`); one 4/8 px spacing grid
(`item_spacing (8,6)`, `button_padding (10,5)`), one corner radius everywhere
(`CornerRadius::same(u8)` - integer at 0.36), `window_shadow = Shadow::NONE`, accent
`selection.bg_fill`. Real fonts via `FontDefinitions` inserted at the **front** of the family so
egui's own remain the glyph fallback (the default fonts are the single biggest tell); report the
fallback when host fonts are absent. All state in the app struct; the UI is a pure function of it;
`ctx.memory()` only for ephemeral UI state, never domain state.

**Repaint:** never `ctx.request_repaint()` per frame (pegs a core). Background producers
`tx.send()` then `ctx.request_repaint()` **from the producer side** (`Context` is
`Send + Sync + Clone` for exactly this); `request_repaint_after(…)` scoped to when data is
flowing, idle otherwise; coalesce bursts. Plain `std::thread` + `std::sync::mpsc`; no tokio in the
GUI. Static replay apps make no periodic repaint request at all (`d1-observe` and the `d1-trace`
viewer assert their own idleness).

**Virtualization (non-negotiable):** logs/tapes via
`ScrollArea::vertical().stick_to_bottom(true).show_rows(ui, line_height, n, |ui, range| …)` over a
`VecDeque` with a hard scrollback cap - uniform line height is what keeps 50k lines at 60 fps.
Tables via `TableBuilder … body.rows(row_h, n, …)` - never iterate your data (hand-rolled
`show_rows` without `egui_extras` is equally fine; pick one per crate). Parse ANSI at ingest into
styled spans; store parsed, render dumb; for syntax highlighting cache the highlight keyed on text
hash. Right-align numeric columns in monospace; sort state lives in the app struct and sorts the
underlying data. For a real interactive terminal, `egui_term` wraps `alacritty_terminal`.

**Plotting (`egui_plot`, when v2 needs it):** never rebuild `PlotPoints` in the `show` closure -
cache the `Vec<[f64; 2]>`; min-max decimation per pixel column for large series; `link_axis` +
cursor link groups make stacked lanes one instrument instead of three widgets; own the
`label_formatter`/`x_axis_formatter`; above ~tens of thousands of vertices skip `egui_plot`
(CPU-tessellated) for an off-thread `egui::Image` or a `PaintCallback`.

**Diagnostics:** `on_hover_text` on every control and number - no dead knobs (in this tree it
carries R2's provenance: file, line-shape, verbatim-vs-derived). Wire `puffin` early when there is
a plot to profile (deferred from `d1-dash` v1 as a new dependency edge).

---

## 8. Python retirement queue

Read-only audit of `capture-rig/`, 2026-08-09. **N = 92** files (62 top-level, 30 in
`capture-rig/oodle/`), 27,075 lines. **Status 2026-08-11 `[C]`: the queue is OPEN - every DEAD,
SUPERSEDED and PORT file below still exists on disk** (verified in this worktree). Two things have
landed since the audit: `d1-forge`'s caret pin is now `=0.36.1` `[C]`, and the **`d1-live` crate
now exists** (`d1-server/crates/d1-live/` - addr/proc/ptrace/watch/symbols; the hardware-watchpoint
tool `CLAUDE.md` names) - but it has **not** absorbed `d1guest.py`: `d1-trace/src/dash/probe.rs`
still shells out to it, and `CLAUDE.md` still names `d1guest.py` as the live read/poke tool, so
§8.6 step 1 remains the top of the queue.

| bucket | count | lines |
|---|---:|---:|
| DEAD - zero references outside the frozen `historicaldocs/ingress/` archive | 10 | 2,388 |
| SUPERSEDED - a Rust crate or Python consolidation already does the job | 7 | 559 |
| WRAPPER - declared thin shim over a Rust tool | 4 | 397 |
| PORT - load-bearing, no Rust equivalent | 27 | 11,268 |
| KEEP-PY - right in Python, or a spent one-shot generator | 44 | 12,463 |

Plus **63 stale documentation references in 21 files**, including 23 runnable command lines naming
a file that does not exist at that spelling (the `historicaldocs/ingress/` underscore vs `capture-rig/` hyphen
split) - the fix list is in `historicaldocs/design/python-retirement-audit.md` §6, ready to apply
and not applied.

### 8.1 Method, and the controls that validate it

Corpus: 1,337 files under the repo root, excluding `.git`, `runs/`, `__pycache__`, `target/` and
the binary estates. A reference is the basename in either spelling plus an `import <module>`
check, excluding the file itself and `historicaldocs/ingress/`. **Positive control, same sweep, same corpus:**
`virtual-pad.py` 53 live refs, `run-boot.sh` 133, `d1guest.py` 119, `probe-guards.py` 8 - all
non-empty, the sweep is not broken `[C]`. **The near-miss that proves the control was necessary:**
`capture-rig/diagnostic_catalogue_doc.py` (481 lines) has zero filename references yet is live -
`capture-rig/diagnostic-catalogue.py:927` does `from diagnostic_catalogue_doc import render`,
which a basename sweep cannot see. ⚠ Rule for whoever executes this list: before deleting any
Python file, run `rg -n "from <module> import|import <module>" capture-rig/` with the
underscored module name. Second control: every `capture-rig/*.py` path cited in `CLAUDE.md`,
`status/**`, `design/**`, skills and shell scripts resolves `[C]` (only `d1tools/…` - a superseded
plan - and `scratchpad/…` do not). False positive checked and cleared: `zeek-control-capture.sh`'s
`dnsd.py`/`dnsq.py` are heredoc-generated at runtime - do not "fix" `[C]`.

### 8.2 DEAD (10 files, 2,388 lines) - delete, blast radius zero by measurement `[C]`

`capture-rig/p2p-static-atlas.py` (551), `capture-rig/activity-launch-static-atlas.py` (360),
`capture-rig/oodle/envelope-span-oracle.py` (287, superseded in substance by sibling
`oodle/probe-envelope-span.py`), `capture-rig/shadps4-p2p-gap-audit.py` (272, point-in-time),
`capture-rig/build-reference-catalog.py` (192), `capture-rig/shadps4-launch-api-audit.py` (188),
`capture-rig/project-source-audit.py` (179), `capture-rig/build-launch-catalogs.py` (135),
`capture-rig/package-symbol-inventory.py` (134), `capture-rig/activity-host-contract.py` (90).
All are integrated round-3 ingress deliveries nothing ever pointed at again; their artifacts are
frozen in `historicaldocs/ingress/round3/`. ⚠ Each differs byte-for-byte from its ingress original (17 of 21
checked pairs differ; only `send-receive-ledger.py`, `subscription-order-report.py`,
`verify-client-bap-probe-sites.py` are byte-identical), so deleting leaves the *pre-integration*
version - capture the integration diff first if it matters `[C]`.

### 8.3 SUPERSEDED (7 files, 559 lines) - the replacement is named

- Three disassemblers → `d1-eboot disasm` `[C]` (`d1-eboot/src/disasm.rs:5-9` names its inputs):
  `oodle/dis-eboot.py` (103), `oodle/dis-annot.py` (78, equivalence pinned as a test at
  `d1-eboot/tests/controls.rs:240`), `oodle/rsat-encoder-disasm.py` (49). Cited only as
  historical provenance; nothing invokes them.
- Two live-memory scripts → `d1guest.py` `[C]` (per `historicaldocs/design/runtime-state-access.md:275-276`, an
  already-written retirement table): `peek-guest.py` (150, "fully subsumed … a drop-in"),
  `dump-descriptors.py` (107, "same output shape, same table constants"). ⛔ Re-point
  `.claude/skills/runtime-probe/SKILL.md:107` from `peek-guest.py` to `d1guest.py` **before**
  deleting, or a real warning is lost. The other three of `d1guest.py`'s five ancestors are NOT
  subsumed and stay PORT: `dump-registry.py`, `inject-roster.py`, `find-in-guest.py`.
- Two fixture generators → `d1-harness`: `two-instance-layout.py` (43;
  `d1-harness/src/controls.rs:340` asserts the port-collision it would have caused **as a
  control** `[C]`), `six-player-fixtures.py` (29; `layout.rs:67` inverts its SOID policy - `[I]`,
  the Rust does not claim total supersession).

### 8.4 WRAPPER (4 files, 397 lines) - keep; retiring is a decision, not cleanup

`eboot-descriptors.py` (155 - **13 Python callers**, the largest caller set; ⛔ do not retire
until the importers move), `oodle/reloc-xref.py` (56), `oodle/global-xref.py` (77),
`oodle/queuez-object-oracle.py` (109). Each is argument forwarding to a release binary; their
benefit is that `CLAUDE.md` and three skills keep working unchanged.

### 8.5 PORT (27 files, 11,268 lines), ranked

**Tier 1 - ⭐ Rust already shells out to these at runtime (5 files, 3,378 lines).** Not "not yet
ported" - **live dependencies of shipped Rust binaries**; deleting one breaks a Rust tool at
runtime, not compile time:

| # | file | lines | controls | depended on by |
|---:|---|---:|---|---|
| 1 | `capture-rig/d1guest.py` | 974 | ⭐ always-on ladder (below) | `d1-trace/src/dash/probe.rs`, `dash/present.rs`, `bin/d1-dash.rs`; cited by `d1-replay/src/live.rs` |
| 2 | `capture-rig/send-receive-ledger.py` | 1,266 | `--self-test` (flag) | `d1-observe/src/input.rs:78,83` |
| 3 | `capture-rig/decode-account-trace.py` | 605 | ⚠ none | `d1-trace/src/ingest.rs`, `controls.rs` - ⚠ d1-trace's **repo-root discovery is keyed on this file's existence**; renaming it breaks d1-trace's ability to find the repo at all `[C]` |
| 4 | `capture-rig/state-machine.py` | 333 | `--controls` (exit-coded, verified PASS) | `d1-trace/src/dash/ladder.rs`, `dash_controls.rs` (`C-DASH-LADDER-DENOM`) |
| 5 | `capture-rig/world-launch-command.py` | 200 | none | ✅ ported 2026-08-11 (`d1-trace/src/blob_decode.rs`); no runtime dependents left |

⭐ **`d1guest.py`'s control ladder is the bar any port must clear.** It has no `--controls` flag
because its controls are not optional - every invocation runs, before any read: (1) BUILD GATE,
eboot sha256 must match or exit; (2) BASE CONTROL, two independent anchors must agree plus a third
direct-read comparison plus a **negative control** (a shifted window must compare UNequal, so a
degenerate all-zero region cannot pass); (3) LIVENESS L0-L3, refusing the read below the command's
required level unless `--allow-dead` labels the result. A Rust port must make the ladder a
**precondition on constructing the handle**, not a subcommand.

⭐⭐ **The `--abs` lesson must survive the port as types, not flags.** Heap pointers are absolute;
eboot vaddrs get `+0x800000000`; `read <heap-ptr>` without `--abs` reads unmapped memory and
returns zeros, indistinguishable from "torn down" - it already cost a published-then-retracted
causal theory. In Rust: `GuestVaddr` and `HostAbsolute` newtypes with no `From` between them; a
`bool` argument ports the bug, not the lesson; pin the `{1,1,0}` vs zeros contrast as the
regression control.

**Tier 2 - launch/skill critical path (5 files, 1,198 lines):** `session-params.py` (330,
`--controls`; ⛔ was under active edit at audit time), `probe-guards.py` (287, `--controls`),
`queue-command.py` (235, ⚠ no controls), `make-manifest.py` (201 - ✅ **PORTED 2026-08-11 to
`d1-pkg manifest`**; all three policies reproduced byte for byte and pinned by `d1-pkg controls`.
Retire it once the hint at `capture-rig/run-signon-experiment.sh:182` is re-pointed; until then it
is the provenance of the pinned golden digests), `gate-check.py` (145 - runs on
**every boot** via `capture-rig/run-boot.sh:303`).

**Tier 3 - `CLAUDE.md`-named instruments (7 files, 3,772 lines):** `diagnostic-catalogue.py`
(937), `message-catalogue.py` (735), `zeek-logs.py` (599, `--self-test` 14 checks exit 2),
`diagnostic_catalogue_doc.py` (481 - port **with** its parent), `eboot-atlas.py` (472 - already
orchestration-only, the smallest highest-value Tier-3 port), `bap-message-table.py` (271,
`--controls` verified ALL PASS exit 0), `pcap-summary.py` (271, ⚠ no controls).

**Tier 4 - the live tier (5 files, 1,319 lines), folds into `d1-live`:** `live-namespace.py`
(613), `dump-registry.py` (213 - the schema field-tree walker `d1guest.py` explicitly did not
subsume; its sanity gate stops a bad descriptor printing "struct 0 bytes, 2155872336 fields" -
carry verbatim), `inject-roster.py` (208 - the write path + account-SOID interlock; ⛔ never
re-point mid-experiment; guard like `d1guest.py poke`: `--experiment NAME --confirm`),
`live-tag-identity.py` (176), `find-in-guest.py` (109 - full-heap value search, "a different
job"; ⚠ carry its refusal to scan shadPS4's ~87 TiB sparse reservations).

**Tier 5 - load-bearing, no Rust home (5 files, 2,379 lines):** `demonware_file_atlas.py` (1,549 -
a Python ELF reader, forbidden shape for new work; home is `d1-eboot`; largest single port),
`verify-client-bap-probe-sites.py` (268 - ⚠ overlaps `probe-guards.py`; `[U]` whether they agree,
reconcile before porting either), `pkg-metadata.py` (254, `--controls`; NOT superseded by
`d1-pkg` - two different containers both called "pkg"), `harvest-crash.py` (176 - the crash-report
reader `CLAUDE.md` says to run before theorising), `ready-byte-report.py` (132).

### 8.6 The live-tier port order

1. **`d1guest.py`** (974 = 59% of the live-tier port; 119 live refs; everything else depends on
   it; ladder as construction precondition, `--abs` as types; nothing else starts until
   byte-equality passes). ⚠ `d1-replay capture` already snapshots `/proc/<pid>/mem` - **read
   `d1-replay/src/live.rs` before writing any base recovery** `[I]`; the anchor logic may already
   exist in Rust.
2. `dump-registry.py`'s field-tree walker (213), sanity gate verbatim.
3. `find-in-guest.py` (109), sparse-reservation refusal intact.
4. `inject-roster.py` (208), between runs only.
5. `session-params.py` + `queue-command.py` (565) after the in-flight edit lands.
6. `live-namespace.py` + `live-tag-identity.py` (789), lowest urgency.
7. `peek-guest.py` + `dump-descriptors.py` (257): **do not port - delete** (§8.3), skill doc
   re-pointed first.

⭐ The prize: step 1 plus `state-machine.py` + `decode-account-trace.py` +
`world-launch-command.py` removes every Python subprocess from `d1-trace`, and
`send-receive-ledger.py` does the same for `d1-observe` - not fewer Python files, but **no Rust
tool that fails at runtime because a script moved.** Total live tier: 1,657 lines to port, 257 to
delete outright.

### 8.7 The retirement protocol

> ⛔ **No Python file is deleted because a Rust replacement exists. It is deleted because the Rust
> replacement was run side-by-side on the same inputs and produced the same bytes.**

(The precedent: `d1-eboot/tests/controls.rs:245` records equivalence to
`disassemble-elf-range.py` as a test; only then was the Python deleted.) Checklist: (1) inventory
callers with the filename sweep AND the import-syntax sweep - a Rust binary shelling out counts as
a caller; (2) capture golden output as committed fixtures; (3) harvest every ⚠/⛔ comment
verbatim into the Rust; (4) enumerate the controls and their arming - a load-bearing flag-gated
control becomes a precondition on construction; (5) make paid lessons unrepresentable (newtypes,
not bools or comments); (6) every control tests the axis it claims, with at least one negative
(the `eboot-callgraph.tsv` lesson: 7/7 controls that were all direct calls re-test at 4/11 FAIL);
(7) ⭐ byte-equality side by side on the fixture set, committed as a `#[test]`; (8) ⭐
monotonicity/shrink control for any tool that regenerates its own input (assert ≥ N rows and that
pinned known-good rows survive - applies to `eboot-atlas.py`, `diagnostic-catalogue.py`,
`message-catalogue.py`, `type-registry.py`, `name-hash-tables.py`, `item-index.py`,
`demonware_file_atlas.py`); (9) re-point the docs in the same turn; (10) leave a supersession note
where the tool used to be named; (11) then delete, and only then.

The one-line test for whether a tool must be Rust: not "is it Python" but **"will another session
run, cite, or depend on this?"** A spent one-shot generator stays Python; anything a Rust binary
spawns as a subprocess is already past the threshold and simply overdue.

> ⛔ **RULING OVERRIDDEN 2026-08-11: `virtual-pad.py` does NOT stay Python forever.** This section
> used to assert exactly that ("a `/dev/uinput` device driver, named as such by the constitution"),
> and the assertion failed this section's own threshold test: `virtual-pad.py` is the first command
> in `CLAUDE.md`'s launch recipe, on the critical path of every run, and a shipped Rust binary
> (`d1-harness`, `layout.rs`) already spawned it as a subprocess. The port landed the same day as
> this override: **`d1-harness pad {run,print-ignore,controls}`**, carrying the FIFO vocabulary
> byte for byte, the PAD-LAW ignore list, the netns ordering constraint as a startup warning, and
> `--vendor/--product` - which closes the two-instance blocker (two pads with one hardcoded VID/PID
> are ONE device to SDL). Executed evidence, `d1-harness pad controls --require-device` 24/24 PASS
> 0 SKIP `[C]`: kernel enumeration with the requested identity and capability bitmaps; two
> simultaneous pads with distinct ids enumerating as two distinct devices (plus the inverted arm:
> two pads with one id measurably indistinguishable); the FIFO protocol end to end with events read
> back off the evdev node; and a side-by-side against the live Python pad, kernel-visible identity
> equal field for field. Still `[U]`: whether SDL maps a pad with a non-x360 GUID - needs a live
> client. `virtual-pad.py` itself stays on disk until the §8.7 checklist completes (docs re-pointed,
> supersession note left); it is now *archivable*, not archived.

Audit `[U]`s, still open: whether `probe-guards.py` and `verify-client-bap-probe-sites.py` agree
on the guards they both derive; whether `oodle/rsat-oracle.py`'s `SchemaOracle` has behaviour
`d1-oracle` lacks (`[I]` a caller problem, not a capability gap); whether
`historicaldocs/design/character-and-orbit.md:5659`'s "only direct writer" claim survives re-derivation under
`d1-eboot xref` (it was produced by a deleted call-only scanner - **the single highest-value
follow-up in the audit**); the 44 KEEP-PY classifications are docstring-level.

---

### 8.8 The doc-update debt a port creates - measured 2026-08-11

**Three workflows are in flight in a parallel lane** (two porting Python tools to Rust, one
auditing which Python tools are unused). Whatever they land, the same debt falls due: **229
references to `capture-rig/*.py` paths across 28 tracked docs go stale.** Every ported or deleted
tool silently invalidates each line that names it, and a doc naming a tool that no longer exists is
worse than no doc, because a reader will try to run it.

Measured today with a filename sweep, ranked by exposure:

| doc | refs | why it matters |
|---|---|---|
| `design/observability-and-ui.md` | 37 | this file: §8.3-8.6 enumerate the retirement candidates by name |
| `design/capture-rig-audit.md` | 32 | the deletion and gitignore manifests are keyed by path |
| `status/TODO.md` | 23 | live queue items name their instrument |
| **`BOOTSTRAP.md`** | **16** | ⭐ **highest risk despite not being the largest.** It is the from-scratch path; a new user runs its commands literally, in order. A stale command there fails on someone who has no context to debug it. **Update this one first.** |
| `design/instruments-static.md` | 16 | §8b's tool roster, already reworked once when six tools were archived |
| `design/queuez-replication.md` | 15 | |
| `design/eboot-map.md`, `status/harness.md` | 10 each | harness.md's pieces table is the operational index |
| `CLAUDE.md` | 6 | the tools table a session reads first |
| `.gitignore` | 3 | ⚠ names three generators in comments; an ignored file whose named generator no longer exists is exactly the fresh-clone trap §2 warns about |
| 18 further docs | 1-6 each | |

**The rule this implies, and it is cheap to honour:** a port lands with its doc sweep in the same
change, the way `d1-eboot`'s equivalence test landed with the deletion it licensed (§8.7). Re-run
the sweep to regenerate this table:

```
python3 - <<'EOF'
import subprocess, re, collections
files = subprocess.run(['git','ls-files','-z'],capture_output=True).stdout.decode().split('\0')
pat = re.compile(r'capture-rig/(?:oodle/)?[a-z0-9_-]+\.py')
hits = collections.Counter()
for f in [x for x in files if x and (x.endswith('.md') or x == '.gitignore')]:
    hits[f] += len(pat.findall(open(f, encoding='utf-8', errors='replace').read()))
for f, n in hits.most_common():
    if n: print(f'{n:4d}  {f}')
EOF
```

## 9. Open contradictions

Included per the banners-override-bodies rule; neither side carries a resolving banner.

1. **HTTP-only frontends vs three shipped file-reading frontends.** §6.4 rule 2 says "Frontends
   consume `present` output over HTTP. Nothing else. Ever." `[R]` (design of record). Yet `d1
   serve`, `d1-observe`, `d1-forge` and `d1-dash` all read run directories (and, for `d1-dash`,
   shell out to two Python tools) directly `[C]` - defensible because the supervisor/serve layer
   the rule presumes is recorded as missing in the same doc, and `d1-dash` recorded a scoped
   exception (the `Board` chokepoint honours the rule's intent; `dash::tail` is the single module
   that changes when a supervisor exists). The `d1-dash` reviewer's standing note: **either the
   rule earns an explicit recorded exception or it stops being a rule** - leaving it
   asserted-and-unfollowed is how the next reviewer "corrects" a working design back into a broken
   one. Unresolved.
2. **`d1-dash` "built and running, 46/46 controls green" vs the same-day review verdict** that
   R1-R3 must be fixed, each with a control that goes red against the reviewed code, "before
   `d1-dash` is used to decide anything" `[C]` both. No record that the fixes landed was found in
   the source docs. **`[U]` - treat `d1-dash` headline output as unciteable for depth/error claims
   until R1-R3 are shown closed.**
3. **"A family that timed out refuses all future updates"** - `d1guest.py`'s own annotation of a
   state constant, never measured `[U]`, yet asserted flatly in two live docs (per the dashboard
   spec's D5 note). Do not encode it as a verdict.

---

## 10. Dead claims

Carried here so no body text above can quietly resurrect them; one line each.

- "eframe 0.35 does not exist; 0.34.2 is current" (interface-architecture rev 2) - false; 0.35
  resolved, latest was 0.36.0 on 2026-08-06, and the pin is now `=0.36.1`.
- "The egui trigger has not fired / the GUI today is served HTML" (egui-practices header,
  interface-architecture §5 original, `d1/src/serve.rs` docstring) - it fired 2026-08-07 and was
  discharged three times (`d1-observe`, `d1-forge`, `d1-dash`).
- "The `App::ui`/`App::logic` split is 0.34-era and likely stale, re-check before use"
  (egui-practices closing note, interface-architecture pre-resolution) - the split IS the current
  0.36.1 API; the warning was the stale thing; do not "correct" it back.
- The `ctx.style()` / `ctx.set_style()` startup snippet (egui-practices original) - does not
  compile at 0.36.1; retracted 2026-08-09; use `all_styles_mut`.
- `SidePanel` / `TopBottomPanel` (egui-practices original) - do not exist at 0.36.1; use
  `egui::Panel::{top,left,right,bottom}`.
- "Pin `egui_plot`/`egui_tiles` as a set with egui" - wrong for those two; companions for egui
  0.36 are `egui_plot = "=0.37.0"` and `egui_tiles = "=0.17.0"`, and a same-number pin silently
  resolves two egui crates.
- "The UD2 record layout is described ONCE, in Python" (trace-viewer §2 original) - it is
  described three times, and the duplication already produced the `probe_id`-index wrong answer;
  corrected 2026-08-08.
- The `d1-dash` spec's pinned control run `runs/20260808-234535-CODEC-LIVE2` - superseded by
  `runs/20260808-210910-AH-RAWCODEC`; a still-live run cannot be a control artifact (deviation 1).
- The earlier compressed retelling attaching the 504 recipient-match failure to an unretired
  `0x000C` (conversation-debugger) - corrected: in that run the head was `0x0020` seq 10; the
  head-of-line law survives, the head identity differs.
- `historicaldocs/design/observability-tooling.md` §3's "pick deliberately" wording on Spicy -
  superseded by `design/format-description.md`'s settled boundary: no Spicy, no D1-specific
  analyzer, ever.

---

## 11. Sources

| melted source | now at | status at consolidation |
|---|---|---|
| `historicaldocs/design/d1-dashboard.md` | `historicaldocs/design/d1-dashboard.md` | spec + delivery status + adversarial review (§§0-8, REVIEW R1-R11) |
| `historicaldocs/design/trace-viewer.md` | `historicaldocs/design/trace-viewer.md` | v1.1, 10 defects fixed and proven (§§1-9) |
| `historicaldocs/design/conversation-debugger.md` | `historicaldocs/design/conversation-debugger.md` | v0.1 validated 2026-08-07 |
| `historicaldocs/design/zeek-pipeline.md` | `historicaldocs/design/zeek-pipeline.md` | built + 14 controls, no real capture yet (§§1-11) |
| `historicaldocs/design/interface-architecture.md` | `historicaldocs/design/interface-architecture.md` | design of record, rev 2, partly built (§§0-10) |
| `historicaldocs/design/egui-practices.md` | `historicaldocs/design/egui-practices.md` | corrected in place for 0.36.1; header status dead (§10) |
| `historicaldocs/design/python-retirement-audit.md` | `historicaldocs/design/python-retirement-audit.md` | read-only audit 2026-08-09; queue open (§§0-10) |

Facts newly verified during consolidation, this worktree, 2026-08-11 `[C]`: all 17 audited
DEAD/SUPERSEDED files still on disk; `d1-server/crates/d1-live/` exists (watch/ptrace/symbols)
but `d1-trace` still shells out to `capture-rig/d1guest.py`; `d1-forge/Cargo.toml` pins
`eframe = "=0.36.1"` (caret fixed) with **no** `default-features = false` (R11's wgpu drag still
open).
