# The map - how the pieces fit

> ## 🎉 2026-08-11 - a **rendered Guardian is in-world** ⇒ `design/GUARDIAN-IN-WORLD.md` (read it
> with `CLAUDE.md`'s top block). The served half is the AH `+0x5028` membership fix plus the hybrid
> family-0 fixture; the rest is still synthetic pokes/hooks and is the work queue in `status/TODO.md`.

**One question drives everything:** *what is the next observable condition Destiny refuses to
proceed without?* Every piece below exists to answer it, or to prove an answer.

> **Dated banner history moved (2026-08-11).** The stack of WHERE-WE-ARE banners that lived here
> (2026-08-06 through 2026-08-10: the ladder finish, the missing-body measurement, the atlas,
> THE-CHAIN/THE-BLANKS pointers, the post-orbit handoff) is preserved verbatim in
> **`historicaldocs/status/MAP-banner-history.md`**. Current state: `CLAUDE.md`'s top block and
> `design/GUARDIAN-IN-WORLD.md`. The system reference is now **`design/architecture.md`** (the
> blocks, the 38-state ladder, the silent-failure index, the must-answer list - what THE-CHAIN and
> THE-BLOCKS carried).

## Scope: a vertical slice, not a world revival

The target is **one fixed roster entering an Elimination session**, then a Trials wrapper, then
Special-Ammo-era behaviour. Not patrol, not raids, not matchmaking at large. The era boundary is
**Hotfix 2.5.0.2, 14 Feb 2017** - after it, special ammo changes and the thing we are preserving stops
existing. Everything below is in service of that slice; anything that doesn't move it is out of scope.

## Why two clients (non-negotiable)

shadPS4's stubs are *more permissive* than real firmware. Emulator-only development converges on
satisfying stubs instead of Destiny's actual contract, and that defect stays invisible until
hardware. So every failure gets disambiguated:

| both fail | CFW works only | shadPS4 works only |
|---|---|---|
| **server bug** | **shadPS4 gap** (→ upstream contribution) | **we satisfied a stub - the server is wrong** |

A gate is passed only when both clients advance, or the divergence is attributed.

## The lane split is physics, not preference

PSN requires current firmware. The jailbreak ceiling is firmware **13.00**. Therefore **no console
can both see Bungie and be instrumented.**

- **Consoles A/B - current firmware, retail, PSN-safe.** Can reach Bungie. Cannot be hooked.
  Their only job: **pcap**. Server→client traffic exists nowhere else. *Awaiting hardware.*
- **Consoles C/D - 10.01, CFW.** Can be hooked and dumped. Can never reach Bungie. One is on the
  bench now and gave us the content gate.
- **C/D updating past 13.00 is irreversible.** Update blocking is the critical control.

## Layers of the thing we have to build

| Layer | What | State |
|---|---|---|
| **A** | PS4/NP surface - the console-identity calls (`sceNp*`) | **OPEN** - satisfied by our own `d1-server` |
| **B1** | **Demonware** - auth, lobby, matchmaking, STUN/NAT/QoS | **the current frontier.** Documented middleware with an existing CoD RE community and a Rust reference implementation. **B1 comes before B2.** |
| **B2** | Bungie's own game services - manifest, inventory, characters, vendors | behind B1. **Character select lives here.** The public bnet API is a *sibling*, not a spec - but its hash-keyed manifest is a Rosetta stone. |
| **C** | P2P game transport - the actual match | **architecture is published** (Aldridge/Tribes); only the *encoding* is unknown. Not "unlit". |

**Layer A is open** [C, 2026-08-06]. The booted 01.29 client used to poll `sceNpGetOnlineId` →
`SIGNED_OUT` thousands of times and never open a socket. `d1-server` (Layer A) now grants a synthetic
identity and `SIGNED_OUT` drops to zero - and Destiny asked ShadNet for **nothing else**.

D1's networking is **Activision's Demonware** middleware `[C]` - which is why Layer B splits into B1
and B2 above, and why a large body of CoD-community prior art applies. See
`historicaldocs/design/networking-architecture.md` and `historicaldocs/design/DEMONWARE-FINDING.md`.

**The gate that replaced it is `signon`, not STUN** [C, 2026-08-06]. `signon` is a server-delivered
bootstrap that hands the client its config, environment, silo id, external address and **BAP server
address**; BAP then carries inventory/progression/matchmaking. The redirect seam is Bungie's own
`sku_config` **signon URL override**, and that file is already in our dump (440 bytes, obfuscated).
See `historicaldocs/design/THE-BOOTSTRAP-HYPOTHESIS.md`.

Previously recorded, still true but not the blocker: **name resolution** [C, confirmed on a clean run]. The moment identity
opened, the client tried to resolve 16 hosts - `stun1-8.signon.{deadorbit,gravityshavings}.net` - and
2 `sceHttpsLoadCert`. **Identity first, STUN second**; that ordering is not in any public source.
Layer B still cannot be *observed*: no client has yet emitted a single Bungie application request.
See `status/server.md`.

## Two sessions, one seam

**Session 2 produces specifications, corpora and decisions. This box produces the running server and
the measurements.** The hard constraint behind it: **one protocol, one implementer** - two sessions
implementing parts of one protocol produce a silent model mismatch. Full split, including what must
*not* be split: `historicaldocs/design/work-split.md`. Paste-able handoff for the research lane:
`historicaldocs/design/session2-brief.md`.

## Version targeting

**01.29 (US) to play · 01.33 to bring up.** 01.29 (`c_date 20161010` → 2.4.1) sits inside the
Special-Ammo era, private matches, and the restored spawn grant. 01.33 is the only version with
public shadPS4 evidence - though as of 2026-08-06 **our own 01.29 now boots**, so that reason has
largely expired. Base is **01.00**; the disc is **CUSA00219** (US, `UP0002`). The project's older
`CUSA00568` references are a different SKU (EU/PAL) and the wrong target - not worth chasing.

## Where the docs live

> **Restructured 2026-08-11.** The estate was culled (52 historical docs) and then the survivors
> were melted into the thematic reference set below. Every melted or superseded original is
> preserved verbatim in root `historicaldocs/` (see `historicaldocs/design/INDEX.md` and
> `historicaldocs/status/INDEX.md` for why each doc is there). Provenance citations in the live
> docs point into `historicaldocs/`; the docs below are the current surface. Each carries a
> "Dead claims" section - the retraction record survives the melt.

**The reference set (`design/`), one doc per theme:**

| doc | owns |
|---|---|
| `architecture.md` | the system in blocks and tiers, the three state machines, the 38-state ladder, the 21-row silent-failure index, what the server owes vs delivers |
| `bap-transport.md` | BAP envelope, handshake, header classes 3/5/6, all 58 message ids, envelope span, message 9 vs 100 routing |
| `signon-and-entitlement.md` | /SignOn request/response schema, sku_config.txt format and signature gate, entitlement offer keys and stores |
| `queuez-replication.md` | the replicated-object model, the type namespace, family map, message catalogue, blank-HUD and empty-profile findings |
| `character-and-investment.md` | character objects and codecs, lifecycle messages 501/502/504/505, progressions, level/light/customization |
| `grants-and-unlocks.md` | the grant contract, the unlock-flag VM and registry, every grant surface, delivery semantics |
| `items-and-gear.md` | the inventory element, item content index, render path, loadout framework, light and gear |
| `content-and-packages.md` | .pkg registration and container format, content inventories, the manifest gate |
| `activity-launch.md` | the launch ladder, mode+map naming, the launch levers, the rung-34 task cluster, orbit as a viewer |
| `activity-host.md` | the AH as a service: all 49 message types, join_result, entities, reservations, the lease, the codec |
| `player-in-world.md` | the spawn chain, the membership fix, the biped census, possession [U]s |
| `peer-and-platform.md` | the peer/server boundary, key material, the sessions API plan (C7), authority model, the vision |
| `instruments-static.md` | the access-mode table, atlas, Ghidra, field-xref, the replay oracle |
| `instruments-live.md` | d1guest, d1-live watch, d1-scope, the network tracer, guest diagnostics |
| `observability-and-ui.md` | dashboards, trace viewer, conversation debugger, Zeek, interface architecture, the Python retirement queue |

**Standalone live docs:**
- `design/GUARDIAN-IN-WORLD.md` - **the current spine.** Read it first.
- `design/eboot-map.md` - ⭐ **the address book**; its §0 is the index to everything protocol.
  An unknown should be a lookup, not a new investigation.
- `design/activity-lifetime-state.md`, `design/geared-guardian-fixtures.md`,
  `design/geared-visuals-run-1.md` - the live lanes' frontier docs.
- `design/renderdoc-orbit-jank.md` - the framerate frontier (the game paints its own frame black).
- `design/prologue-and-bucket-unlock-gate.md` - the `character:prologue` substage gate and the
  inventory bucket-unlock check, both reading the unlock plane; contests `eboot-map.md` §2.6's
  family-4/4 reading. `static-client-inference`, unverified live.
- `design/d1-ghost.md` - the run companion's write door (registry/ledger/gated executor);
  `C-GHOST-*` in `d1-trace controls`.
- `design/ghidra-bringup.md` - the Ghidra procedure: zero to a citable database. Load-bearing,
  because 36 citations across 13 docs rest on the decompiled corpus, which is Ghidra output
  and is never shipped. Its section 11 carries the outranks-nothing rule.
- `design/diagnostic-catalogue.md` - generated; regenerate, do not hand-edit.
- `design/licensing.md`, `design/format-description.md` - standing decisions.

**Everything else:**
- `historicaldocs/` - the preserved history: superseded plans, retracted premises, milestones,
  session narratives, and every melted source. Will become gitignored after this branch merges
  (`publish/untrack-historicaldocs.sh`).
- `status/` - the eight core docs plus `PUBLIC-PUSH-PLAN.md` (the public-push procedure).
- `historicaldocs/ingress/` - the research lane's former inbox, RETIRED untracked 2026-08-11.
  Its salvage landed first (method rules, retractions, the tool roster, the bdLog ABI); the rest was
  measured superseded or already integrated. `historicaldocs/INDEX.md` records the disposition.
- `historicaldocs/destiny1/` - the pre-execution research dossier (retired 2026-08-11; Bungie patch notes and third-party evidence moved to `external/`).
- `capture-rig/CFW-DUMP-RUNBOOK.md` - the console C/D dump procedure (live, operational).

## Evidence convention

`[C]` confirmed · `[O]` observed, not reproduced · `[R]` reported by another session ·
`[I]` inference · `[U]` unknown, needs an experiment.
