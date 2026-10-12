<!-- SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project -->
<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# d1-scope - crate skeleton spec

> **THE LIFECYCLE INVARIANT.** *d1-scope may never leave a hardware watchpoint armed behind a
> process that is not alive to disarm it: at most one `d1-live` child exists at any instant, it is
> always launched with a bounded `--max-seconds` lease shorter than the parent's heartbeat gap, it
> is stopped only by `SIGTERM` and only after the parent has waited for the child's own confirmed
> disarm on exit, and every path out of d1-scope - normal return, error, panic, `SIGINT`, `SIGTERM`
> - runs that stop. **A tracer that dies armed kills its target.** That is measured, every time, not
> theoretical, and the target is the operator's live client.*

Architecture and rationale: **`historicaldocs/design/d1-scope.md`**. Surface and edge data:
**`capture-rig/surfaces/registry.toml`**. This file is the implementation contract.

**On disk now:** `Cargo.toml` (the pin, its own workspace root), `Cargo.lock`, `SPEC.md`,
`tests/fixtures/d1-live/*` (four recorded `--json` documents + provenance), and a `src/main.rs`
**stub that exits 2**. Nothing below is implemented. ⚠ Build from *this* directory:
`cd d1-server/crates/d1-scope && cargo build --release`.

---

## 1. Module layout

| module | owns | must not |
|---|---|---|
| `mem_source` | the `MemSource` trait + `ProcMem` (`/proc/<pid>/mem`, `O_RDONLY`, `pread`) and `SnapMem` (a `runs/snapshots/*.snap` extent table). `Mapping`, `ReadErr`, `SourceId`, base recovery. | expose any write path; return `0` for an unmapped read; open the fd `O_RDWR` |
| `registry` | parse `capture-rig/surfaces/registry.toml` into `Registry { surfaces, edges, opens, meta }`; resolve `addr` strings **through `d1_live::addr`**; own the `NOT-EXPRESSIBLE` resolvers (Rust in, `a:0x…` out). | parse an address itself; rewrite the TOML; accept an `addr` the grammar rejects |
| `sampler` | the state plane: one tick = re-resolve **every** chain from its root, read fields, evaluate liveness, evaluate edge predicates, emit one `Frame`. | cache a resolved address across ticks; skip a liveness gate; write anything |
| `child_mgr` | the event plane: `ChildMgr` owning `slots: [Option<WatchSpec>; 4]`, spawn/lease/heartbeat/stop, `TracerPid` refusal, `Drop` guard, signal handler, parsing d1-live `--json` (incl. the zero-hit document). | link or call ptrace; `SIGKILL`; spawn without `--max-seconds`; hold two children |
| `frame` | ⭐ **the `present` chokepoint.** Builds the immutable `Frame` and every display string/sort key in it. The CLI's `--json` and the UI render *this and nothing else*. | be imported by `ui` for anything but reading; contain egui types |
| `ui` | eframe/egui shell: `App::logic` drains the channel, `App::ui` draws. Surface table (`egui_extras::TableBuilder`), event list, child banner, theme lifted from `d1-forge/src/theme.rs`. | compute a value; read a `MemSource`; own state that isn't view state (sort col, pan/zoom, selection) |
| `graph` | layered layout (~200 lines) + `egui::Painter` node/edge canvas, `RectTransform` pan/zoom, per-node `ui.interact`, cached galleys. | take an external graph/plot crate; decide an edge's state |
| `controls` | the 18 controls of `historicaldocs/design/d1-scope.md` §6, as `#[test]`s **and** a `controls` subcommand that **exits 2** on any failure. Offline; runs against `SnapMem` and the recorded fixtures. | touch a live process; pass unconditionally |
| `main` | clap CLI, the run loop, `SIGINT`/`SIGTERM` handler, and the single call into the stop path. | contain logic that isn't wiring |

**Dependency direction, enforced by review:**
`main → {ui, controls, sampler, child_mgr, registry}`, `ui → frame`, `graph → frame`,
`sampler → {registry, mem_source, frame}`, `child_mgr → frame`. ⛔ **`frame` depends on nothing in
this crate and knows no egui type.** `mem_source` and `registry` never depend on `ui`.

### CLI surface (every feature exists here before or with any UI)

```
d1-scope controls                                  # 18 controls, offline. exit 2 on any FAIL.
d1-scope surfaces [--group G] [--json]             # the registry, resolved against --source
d1-scope frame [--json]                            # ⭐ one Frame: the whole tool's state
d1-scope edges  [--state red|grey|unknown|...]     # the flow graph as rows
d1-scope watch  --surface NAME[.FIELD] [--rw write|exec] [--seconds N]   # ONE leased child
d1-scope gui                                       # the same Frame, drawn

--source live[:PID] | snap:<runs/snapshots/NAME.snap>      # default: snap, never live
```

⭐ **`--source` defaults to a snapshot.** A tool that attaches to the operator's client because you
forgot a flag is a tool that will one day do it during a run that mattered.

---

## 2. `Frame` - built once per tick, never mutated, the only thing a frontend sees

```rust
/// One tick of the world, immutable. Handed to the UI as `Arc<Frame>`; serialised verbatim by
/// `d1-scope frame --json`. ⭐ If it is not in here, no frontend may know it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Frame {
    pub seq: u64,                    // monotonic tick counter; 0 is the first frame
    pub ts_unix_ms: u64,             // wall clock, for pasting into a doc
    pub ts_mono_ms: u64,             // monotonic, for deltas - never subtract wall clocks
    pub tick_ms: f32,                // how long this tick's reads took (perturbation budget)
    pub source: SourceId,            // Live { pid, base } | Snapshot { id, path }
    pub generation: u64,             // MemSource::generation() when this frame was built;
                                     // a resolution stamped with an older one is discarded
    pub surfaces: Vec<SurfaceView>,
    pub edges: Vec<EdgeView>,
    pub events: Vec<EventView>,      // newest last; capped, with `events_dropped`
    pub events_dropped: u64,
    pub child: ChildStatus,
    pub notes: Vec<Note>,            // loud, persistent conditions (failed disarm, refusals)
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum SourceId {
    Live { pid: i32, base: u64 },
    Snapshot { id: String, path: String },
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SurfaceView {
    pub name: String,
    pub group: String,
    pub addr_expr: String,           // verbatim from registry.toml
    pub resolved: Resolved,
    pub resolve_trace: Vec<String>,  // every hop, from d1_live::addr - a resolution nobody saw
                                     // is a resolution nobody checked
    pub liveness: LivenessView,
    pub fields: Vec<FieldView>,
    pub confidence: String,          // [C] [O] [R] [I] [U]
    pub reach: String,               // observed-live | replayed-capture | synthetic_ancestor | ...
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum Resolved {
    Addr(u64),
    Unmapped { addr: u64 },                 // ⛔ NOT zero. The region was not captured / not mapped.
    Unresolvable { why: String },           // a deref hop failed; `why` names the hop
    NotExpressible { resolver: String },    // needs a Rust resolver (registry `addr` reserved value)
    Unknown,                                // registry `addr` == "[U]" - see the [[open]] entry
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FieldView {
    pub name: String,
    pub off: u64,
    pub ty: String,
    pub value: Value,
    pub note: Option<String>,
}

/// ⛔ `Unmapped` and `NotMeasurable` are NOT `U32(0)`. "Not captured", "liveness said no" and
/// "was zero" are three different facts and only one of them is evidence.
#[derive(Clone, Debug, serde::Serialize)]
pub enum Value {
    U8(u8), U16(u16), U32(u32), U64(u64), I32(i32),
    Ptr(u64),
    Bytes(Vec<u8>),
    Ascii(String),
    Unmapped,
    NotMeasurable { liveness: String },
    NotRead,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LivenessView { pub expr: String, pub verdict: Verdict }

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub enum Verdict { Live, NotLive, Unknown }

#[derive(Clone, Debug, serde::Serialize)]
pub struct EdgeView {
    pub id: String,                  // "E-15"
    pub up: String, pub down: String,
    pub mech: String,
    pub state: EdgeState,            // ⛔ DATA, from the predicates - the UI only picks a colour
    pub why: String,                 // which arm decided it, with the values that decided it
    pub guard: Option<String>,       // e.g. E-13: "only valid IN AN ACTIVITY"
    pub guard_active: bool,          // guard hit ⇒ state is Unknown, never Red
    pub evaluable: bool,             // false ⇒ a server-side or [U] endpoint; never Red
    pub as_of: String,
}

/// ⛔ FIVE STATES. Collapsing any two mis-accuses an edge. `Grey` = upstream not populated ⇒
/// never exercised (untested, not broken). `Retired` = dead by construction; E-18 only.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub enum EdgeState { Green, Red, Grey, Unknown, Retired }

#[derive(Clone, Debug, serde::Serialize)]
pub struct EventView {
    pub seq: u64,
    pub elapsed_ms: u64,
    pub surface: String,             // which watch produced it
    pub addr: u64,
    pub tid: i32,
    pub rip: u64,     pub rip_vaddr: Option<u64>,     // ⚠ rip = the instruction AFTER the store
    pub writer: u64,  pub writer_vaddr: Option<u64>,  // ⭐ show THIS one
    pub writer_text: String,
    pub writer_confidence: String,   // verbatim from d1-live. Never invent one.
    pub value_after: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum ChildStatus {
    None,
    Running { pid: i32, spec: WatchSpecView, lease_s: f64, lease_remaining_s: f64, armed_threads: u32 },
    Finished { spec: WatchSpecView, hits: u32, stop_reason: String, exit: i32 },
    /// ⭐ A parseable document with zero hits is THIS, not `Failed`: a real negative result.
    NoHit    { spec: WatchSpecView, watched_s: f64, stop_reason: String },
    Failed   { spec: WatchSpecView, stderr: String },
    /// ⛔ LOUD AND PERSISTENT: SIGTERM was sent and the child never confirmed exit.
    /// No further spawn is permitted until it is reaped.
    StopTimedOut { pid: i32, waited_s: f64 },
    Refused  { why: String },        // TracerPid, slot exhaustion, second child, no lease
}
```

---

## 3. `MemSource` - the read-only state plane

```rust
/// ⛔ THERE IS NO `write`. The poke ban is a property of the type, not of anyone's discipline:
/// a poke identifies a gate and never satisfies one, and everything downstream of one is
/// `synthetic_ancestor` twice over.
pub trait MemSource: Send + Sync {
    /// Where this frame's numbers came from. Printed next to every value: shadPS4's arena
    /// allocator hands out identical addresses across runs and days, so "same address" is a
    /// control that ALWAYS passes and agreement without provenance proves nothing.
    fn id(&self) -> SourceId;

    /// The measured guest base (`+0x800000000` for the pinned client), or `None` when only
    /// absolute addresses are meaningful. ⛔ Measured, never assumed.
    fn base(&self) -> Option<u64>;

    /// Bumps whenever the underlying process could have moved beneath us (re-attach, pid change,
    /// snapshot swap). Any resolution stamped with an older generation is discarded, not reused.
    fn generation(&self) -> u64;

    /// The ONE primitive. `Unmapped` is a distinct outcome from a successful read of zero bytes
    /// of value - never conflate them.
    fn read(&self, addr: u64, out: &mut [u8]) -> Result<(), ReadErr>;

    /// The captured/mapped extents. Used to answer "was this region even captured?" BEFORE
    /// reporting a value, so `UNMAPPED` is a positive statement rather than an inference.
    fn mappings(&self) -> Result<&[Mapping], ReadErr>;

    fn read_u64(&self, addr: u64) -> Result<u64, ReadErr> {
        let mut b = [0u8; 8];
        self.read(addr, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Mapping { pub start: u64, pub end: u64, pub perms: String, pub name: Option<String> }

#[derive(Debug, thiserror::Error)]  // or a hand-rolled Display; no new dep is required
pub enum ReadErr {
    /// ⭐ The whole point. Rendered as `UNMAPPED`, never as 0.
    Unmapped { addr: u64, len: usize },
    Truncated { addr: u64, want: usize, got: usize },
    Io { addr: u64, source: std::io::Error },
    NoSource,
    /// The target vanished mid-tick. Not an error to shout about - the client exited.
    Gone { pid: i32 },
}
```

**`ProcMem` construction rules:** open `/proc/<pid>/mem` with `O_RDONLY` **once** and hold the fd
(`controls::mem_source_is_readonly` verifies the flags via `/proc/self/fdinfo`); read with `pread` at
absolute host addresses; parse `/proc/<pid>/maps` on construction and whenever `generation()` bumps;
`ESRCH`/`EIO` ⇒ `Gone`, not a panic.

**`SnapMem`:** the extent table of a `runs/snapshots/*.snap`. Its `mappings()` is what makes
"the region was never captured" a *measured* statement - the six known `--preset content` gaps
(SSTM, cinematic manager, activity loader, game globals, unlock plane, activity manager) are
asserted by name in `controls::snapshot_coverage_honest`.

---

## 4. `child_mgr` - the spawn/stop contract, in one place

```rust
pub struct WatchSpec {
    pub surface: String,      // for attribution back into the Frame
    pub addr_expr: String,    // the registry expression, kept verbatim for the receipt
    pub resolved: u64,        // handed to d1-live as `a:0x…` - one resolver, one grammar
    pub len: u64,             // 1|2|4|8, naturally aligned
    pub rw: Rw,               // Write | Exec   (Exec ⇒ len 1)
    pub slot: usize,          // 0..=3, owned by ChildMgr
    pub count: usize,
    pub lease_s: f64,         // ⛔ ALWAYS Some, ALWAYS <= lease_max
}
```

* `spawn` refuses, by name, if: a child is live · `TracerPid != 0` and is not ours · all four slots
  are taken · `lease_s > lease_max` · a previous stop timed out.
* argv is built by **one** function, `argv(&WatchSpec) -> Vec<String>`, which always emits
  `--max-seconds`; `controls::child_lease_bounded` asserts against *that* function so the assertion
  cannot drift from the thing it protects.
* `stop()` = `SIGTERM` → wait (bounded `stop_timeout`, default 5 s) → reap → `confirmed_disarm`.
  `Drop for ChildMgr` calls it. The `SIGINT`/`SIGTERM` handler sets a flag the run loop turns into
  the same call. **One stop path, four entrances, zero `SIGKILL`.**
* Output classification is on **stdout parseability**, not exit code: parseable + hits ⇒ `Finished`;
  parseable + `hits: []` ⇒ **`NoHit`** (a result); unparseable ⇒ `Failed` with stderr verbatim. Both
  of the last two exit 1 - the recorded fixtures in `tests/fixtures/d1-live/` are precisely that pair.

---

## 5. Implementation order (each step ends with a control that can fail)

1. `mem_source` + `SnapMem` → controls 3, 4, 17.
2. `registry` (parse + resolve through `d1_live::addr`) → controls 1, 16.
3. `frame` + `sampler` → controls 5, 6, 14, 15; `d1-scope surfaces/frame/edges --json` works.
4. `child_mgr` against a **stub child**, then the recorded fixtures → controls 7-13.
5. `ui` (table, events, child banner) → control 18.
6. `graph` (layout + `Painter`) → control 18 extended to nodes/edges.

⭐ Steps 1-4 are the whole tool for a CLI user and require **no live client at all**. Do not invert
this order: an egui window that reads `/proc` before the controls exist is how a tool ships that
nobody can prove is honest.

⛔ **Do not write `fn update(&mut self, ctx, frame)`** - `eframe::App` at 0.36.1 has no such method
(`logic` + `ui`), `SidePanel`/`TopBottomPanel` do not exist (`egui::Panel::left(id).show(ui, …)`),
and `Context::style()`/`set_style()` are gone (`ctx.all_styles_mut(…)`). The full trap list is
`historicaldocs/design/d1-scope.md` §5 and §8.
