# 01.24 continuation and diagnostic audit — 2026-09-30

The active target is BLES01857 **01.24**, not the historical 01.00 mission
milestone. Source baseline: `73c4f47` on `master`, matching `origin/master` after
fetch. The older imported checkout has additional uncommitted readers and notes;
it is preserved. Runtime: `F:\d1\lukespc-import\d1xenia\local`; data checkout:
`F:\d1\d1xenia-data`. The source checkout remains `F:\d1\d1xenia`.

## What survived independent review

The saved 01.24 bootflow samples from runs 019/022/023 identify stage 19,
`bootflow:bap_signin`, with stage-object substate 12 and resource slot 3 pending
at status 1. Run 020 reached substate 16 with that resource at status 2 under a
different, mostly disc manifest. These observations already existed locally,
but the 2026-09-29 checkpoint and its reader had not been committed.
Run 023's preserved sample is `FINAL124_SMOKE023-bootflow.json`; its previously
"pending" result is state 19, substate 12, bootstrap status 1, no BAP connection.

The current final-build preflight passes: exact installed 01.24 EBOOT and patch
identity, all 247 disc filenames, 1,034 required update packages at expected
sizes, 5,041 exported types, 94 service mappings, and both mission schemas.
Passing this check verifies setup and encodability, not live client acceptance.

## Corrections to the latest gate diagnosis

- At `0x362a0c`, `crorc` computes BA OR NOT BB. The zero-state path writes one
  at `0x362a1c`; the assertion that zero is a terminal no-op is inverted.
  [IBM's reference](https://www.ibm.com/docs/en/aix/7.2.0?topic=set-crorc-condition-register-complement-instruction)
  and RPCS3's local interpreter/translator agree on the operation.
- `0x332d24` applies `cntlzw`/`rlwinm` without a following inversion: it returns
  one when `0x126a40` returns zero. That callee indexes its fields by `4*r4`;
  entry samples from different callers/indexes do not establish this result.
- The existing hook logs before the compiled partition's scheduler status check.
  When the check defers execution, retries can consume the watch budget without
  executing guest instructions. In `RUN_GATE2`, 60 identical gate dumps span
  12,625 microseconds; 40 identical advance dumps span 7,682 microseconds. These
  are bursts, not independent observations throughout a session.
- A hook address identifies an RPCS3 compiled partition, which can start inside
  a logical guest function. A zero hit count at an arbitrary instruction does
  not establish that the instruction was never executed.

`patches/rpcs3-destiny-main-memory-blit.patch` now places the hook after status
admission and advances its PPU object-cache prefix from `d1p2` to `d1p3`.
The complete patch passes `git apply --check --cached` against source baseline
`b5a174e93`. The transferred executable remains the old build. No MSBuild/MSVC
toolchain is installed on this PC, so this patch has not been compiled or
deployed here; direct guest-memory samples are used for the current experiment.

## Tool and launcher repairs

`d1_offset_scan.py` now decodes displacement forms from primary opcodes, including
integer, floating, and DS forms. Indexed operations have no immediate struct
displacement and are excluded. DS subtype bits are masked separately, so `ldu`
and `lwa` references are no longer missed. `d1_eboot.py` uses supported PPC64
disassembly and handles those subtype bits in TOC references too. The prior
claim that Capstone silently rejects PPC64 mode was not reproducible.

Manifest selection can now use `--inventory-file` to exclude stale/unlisted
updates. Numeric suffix alone is not release chronology: the inventory marks
`ps3_investment_globals_012f_5.pkg` stale while final `_4` remains valid. The
smoke launcher supplies the verified final inventory automatically. Explicit
replacements remain experimental overrides. A missing update directory fails
instead of silently producing a disc-only manifest.

The run wrapper resolves paths from `Paths.ps1` and explicit parameters, uses a
fresh run name, saves process identities and protocol counts, and reports
launcher/preflight/service failures. It sends input only to its owned emulator
and cleans up only recorded processes with matching identity. Shared RPCN is
retained. Occupied debugger ports are refused instead of attaching to another
emulator. `-ManifestFile` permits controlled content comparisons.

The imported test copy's GPU adapter was changed from the transferred RTX 5070
name to this PC's RTX 2060 SUPER. Both config files were backed up locally before
editing. The old watch file was preserved and logging disabled for direct memory
sampling; no service reply or guest behavior was patched to force progress.

## Direct observation and next target

`scripts/rpcs3_final124_bootflow.py` promotes the earlier private reader. It checks
memory reply lengths and reports stage stability, stage substates, the eight
resource records, pending count, and optionally the bootstrap request/parent
chain and resource scheduler (`--request`). The auxiliary mode, readiness masks,
and bounded active descriptor summaries are always included. Samples are sequential,
not atomic, and belong under private `local/`. Each daemon memory request briefly
pauses/resumes the emulator; the stage guard runs after the optional reads.

The saved ELF identifies the state-12 wait precisely:

1. Resource base `0x1ada1b0`, slot 3 at `0x1ada1d4` (three u32 words: status,
   request handle, asset tag), pending count at base `+0x60`.
2. `0x531fe0(..., 3)` submits `client_bootstrap_patchable`; `0x53280c` maps
   resource statuses 1/3 to the pending mask. State 12 waits for its bit 3 to clear.
3. Poller `0x532718`, called from `0x35c050`, skips work when pending count is
   nonpositive. Otherwise it resolves handles and calls `0x14e864`.
4. That query reads signed request byte `+7`, following the parent handle at
   `+0xc` when kind byte `+6` is 2 and status is 1. A result <=1 remains pending;
   a result >1 invokes completion `0x531e78`. Result 3 is an error.

These are resource-load prerequisites inside the BAP boot step. Stage 19 alone
does not prove a STUN/NAT dependency, and unanswered UDP probes alone do not
identify its current blocker. Correlate the request and pending count before
making another reply-shape change. The reproduction and cleanup commands are
at the top of `next-session-playbook.md`.

## New run results

| Run | Inputs / change | Observed result |
| --- | --- | --- |
| `FINAL124_AUDIT01` | Initial repaired wrapper, default 311-entry manifest | Launch failed before RPCS3 started. The ownership record exposed an immediate-process metadata race; HTTPS upload aliases also attempted duplicate socket binds. Both defects were repaired. Three identified children with null executable identities required explicit cleanup; the new Python-child lifecycle tests cover that race. |
| `FINAL124_AUDIT02` | Inventory-filtered manifest (byte-identical to baseline), deduplicated listeners, disabled pre-status hooks | 240-second observation: one SignOn success, one manifest reply, 21 STUN probes/replies, zero BAP connections/frames. Direct samples agree on stage 19, substate 12, bootstrap status 1, pending count 1. Request kind 2/status 1 waits on parent kind 3/status 1. |
| `FINAL124_AUDIT03` | 160-entry disc manifest with only `0151` replaced by update `_4`, reproducing the earlier run 020 profile | 240-second observation: one SignOn success, one manifest reply, 21 STUN probes/replies, zero BAP connections/frames. Retained-session samples agree on stage 19, substate 16, bootstrap status 2, invalidated bootstrap request handle, and pending count 0. The separate asset-readiness wait is still active. |

AUDIT02's two independent samples are preserved as
`RESEARCH/ps3_updates/FINAL124_AUDIT02-bootflow-{1,2}.json`. The client-side
01.24 PPU hash was verified in the preserved emulator log. Its 311-entry
manifest SHA-256 is `d39d356b5dbf165f9ca9f24ce3df6e0588a8ebf315a5d6392584e8c990f7c3e7`;
inventory filtering changes no bytes on this already clean installed package set.
The final run index preserves 18 log fingerprints, 325 package-open events,
and two screenshots of the black spinner. The owned stack was closed after
sampling and the final emulator log was copied before another launch.

The paired parent/child handles are expected, not an infinite query recursion:
`0x1523c4` transfers kind-2 request lists to a new kind-3 request and stores the
reciprocal handles at both `+0xc` fields. Query `0x14e864` follows only kind 2.
The request snapshot includes the complete `0x44`-byte record (six list
descriptors and counters at `+0x40/+0x42`) to inspect the scheduler dependency.

SignOn listeners now deduplicate aliases by address/port; the HTTP Host and SNI
paths are unchanged. Run tooling records explicit executable paths at launch,
checks PID/start time/path before cleanup, and saves emulator logs with Windows
read/write/delete sharing. Capture also runs after failed runs, and retained
emulators can report a deferred capture without losing their session.

AUDIT03's comparison manifest is 7,925 bytes with SHA-256
`6903c8d4cb6420818a8356507ecfe944badfc0a904685ba4c9e0f7903233d037`.
It changes many package choices relative to AUDIT02; the comparison establishes
that resource completion is possible under a different profile, not that `0172`
alone causes the first wait. No guest state was written to bypass either wait.

The state-16 poll `0x3592d8` reads the auxiliary singleton at `0x01abc8b8`,
then its signed mode at `+0x80`, and calls `0x856d70` / `0x85619c`. The
initialized readiness table is at `0x01b558f0` (byte `+0xe8`). The poll waits
while `required[mode] & (0x7ff & ~completed)` is nonzero, with u16 mode masks
starting at the table and the u16 completed mask at `+0xba`. AUDIT03 observes
mode 2, required mask `0x0002`, completed mask zero, missing mask `0x0002`,
and initialized flag 1. This identifies a concrete asset prerequisite before
BAP; it is not evidence that changing a STUN reply can resolve it.

The resource scheduler reader also records manager queue/retry/active-handle
fields and the bounded active token record. In AUDIT03 it observes an empty
queue, zero retries, active handle `0x02fa0002`, token zero, and batch status
word 8. The word remains numeric until its phase and dependency are verified.

The label table at `0x018b7608` names bit index 1 **`ui_core`** (pointer
`0x01345280`). The active descriptor has content handle `0x80814000`,
loaded pointer `0x01b5ef00`, and allocation pointer `0x01b55b10`. The loaded
record's byte `+0xf` is zero; completion updater `0x8564e4` waits for it to
become one before publishing readiness bits at `0x856670`. The active kind-3
request `0x02fa0002` remains status 1 with counters `+0x40/+0x42 = 0/1`.
This narrows the next investigation to the pending `ui_core` load and its scheduler
phase. Intrusive list links alone do not identify the cause of the pending I/O.

AUDIT03 was retained for three consistent bootflow samples and then closed using
its ownership manifest. Its final index preserves 19 log fingerprints, 179
package-open events, two black-spinner screenshots, and no fatal events. By
cleanup there were 62 STUN probes/replies and still no additional SignOn or BAP
progress. The final closed record confirms no owned processes remain; shared
RPCN was left running.

## Validation and resume boundary

The server suite passes 82 tests and the scripts suite passes 59 (141 total).
`scripts/Test-Final124Run.ps1` passes its isolated Windows fixtures, including
process metadata timing, unrelated-process survival, interruption during
registration, hung-launcher child cleanup, shared log snapshots, capture after
failure, and deferred capture for an exclusively locked retained log. Independent
review found no additional actionable launcher issue. `git diff --check` passes.

The unresolved live boundary is before BAP, with content loading prerequisites
identified separately for substate 12 and substate 16. Reproduce the final
manifest and record the scheduler token/active request before making a focused
content or emulator change. The comparison profile is diagnostic and should not
replace the final manifest. The corrected RPCS3 trace patch still requires a
Windows C++ build environment before trace-hit counts can be trusted again.

All raw logs, requests, guest memory, screenshots, and generated manifests remain
under the ignored runtime. Only code, tests, and research summaries are committed.

## 2026-10-01: MATILDASPC, d1p3 build (Claude session)

**Toolchain.** The second PC now builds RPCS3 (see the README's "Moving to another PC"). The
d1p3 binary built here (`6d0217aa...`) and the main PC's staged one (`6a79e3e7...`) both
contain `kusa-d1p3`. The staged one is the one in use: swapped in as `rpcs3.exe` in
`RPCS3_FINAL124_SMOKE001` and `RPCS3_TEST048_SETUP`, with the old exe kept as `rpcs3.d1p2.exe`.

**FINAL124_D1P3_WARMUP / WARMUP2.** First boot on d1p3. WARMUP aborted at its first Cross
(45 s): `Send-RpcsKey` found no game window during the PPU recompile, so warm-up boots must
use `-PressSeconds 0`. WARMUP2 (no presses): the PPU recompile took about 3 minutes, not the
expected 20-40. At emulator time 3:12 the game was drawing (D1MEM surface 3M, Bungie logo).
The run was then closed through its ownership manifest. The wrapper's `.stop` file only
interrupts the launcher, not the observation loop.

**FINAL124_NEXT.** Died at 57 s: `dns_sinkhole.py` raised `ConnectionResetError [WinError
10054]` from `recvfrom` (Windows reports ICMP port-unreachable for an earlier reply that way),
and the wrapper aborted because an owned process had exited. Fixed in `e8df00c`: the DNS, STUN
and session-listener receive loops ignore it.

**FINAL124_NEXT2** (final manifest `manifest-final-smoke.bin`, d1p3, 300 s, Cross for 150 s,
`-KeepRpcs3`). SignOn 1/1, manifest reply 1, 24 STUN probes (15 answered), 0 BAP. The
`--request` snapshot (`RESEARCH/ps3_updates/FINAL124_NEXT2-bootflow.json`) reproduces stage 19
`bap_signin`, substate **12**, slot 3 `client_bootstrap_patchable` state 1, handle
`0x01878002`, extra `0x80ae4000`, pending 1. The kind-2 request has status 1 and parent
`0x01fa0002`; that kind-3 parent is status 1, with the child handle at +0xc and
`0003 0003 00000001` at +0x10. The scheduler's active handle is `0x01fa0002`, and its retry
slot is `0x06fa0001`. **Expected = actual**: the wait reproduces on the corrected build.

**What slot 3 loads (static, 01.24 EBOOT).** `0x531fe0(mgr, slot)` reads the slot name from
`0x01315588[slot]` and resolves it with `0xe659c` -> `0xecc6c`. That function is a named-tag
search over the loaded packages' named-tag tables (68-byte entries: tag, class, char[60]
name), keeping the match from the package with the highest u64 at record +0x10. If the
lookup returns -1, the slot gets status 5. Otherwise it creates a request with
`0x15216c(..., 7, 2, 0)` and stores the handle at +4 and the tag at +8, which is the
snapshot's "extra". So **`client_bootstrap_patchable` = tag `0x80ae4000` = package
`0x0172`, entry 0** (by `d1_pkg.tag_hash`).
- The request handles `0x01878002` / `0x01fa0002` / `0x06fa0001` are **request-pool
  handles**, not tag handles: group = (h >> 13) & 0x3ff (0x3c, 0x3d0), item = h & 0x1fff, pool
  stride 0x48, descriptor table `*(*0x01896d50)`. The handoff's "handle >> 13 = package index"
  applies to tags, not to these handles. The records hold no tag. The tag lives only in the
  slot's +8.

**Package 0172 in FINAL124_NEXT2's RPCS3.log.** Both `_1` (01.13, 12.5 MB) and `_2` (01.20,
38 KB overlay) are `stat`ed at 1:56; `_1` is **never opened**. `_2` is opened at 2:39 on
async_job (644 bytes read, closed), then reopened at 2:47 on `fios mediathread 2` (fd 3) and
`fstat`ed, with no close before the 4:54 snapshot. Right after it the same thread opens the
**disc** `ps3_shared_manifest_0430_0.pkg`. This RPCS3 build aggregates `sys_fs_read` (`⁂
sys_fs_read [944]`), so the log cannot show whether `_2` was read after 2:47. The 09-29
checkpoint's run 022 (explicit `_0`+`_1`+`_2` chain, all three opened) also stopped at
substate 12, so chain registration alone is not the answer.

**Next, smallest first.** (1) Read the `0x80ae4000` load state live, meaning the tag's
resolution in the package registry for 0172, to see whether the overlay `_2` resolves entry 0
into `_1`'s data and whether FIOS has a pending read on fd 3. (2) Compare against run 020
(substate 16, slot 3 done): which 0172 package that manifest served, and whether it used the
disc 0430/0431 shared manifests.

### 2026-10-01 (cont.): package registry live, three manifests on d1p3

**Registry (static analysis from the main PC, checked live).** Pool `*0x1896510` (records at +0x20,
stride 0x1a8; 314 records in the final manifest), u16 id -> index map at `0x19c2c60`, head/count
`0x19bac30`/`0x19bac34`, so there is one record per package id. Live record fields: +0x04 id,
**+0x10 = the package's build time** (equal to the .pkg header's build time; 0xecc6c prefers the
newest), **+0xb4 = entry count** (not a block size), +0xec named-tag count, +0x140 the
`packages/<name>.pkg` path, +0x178 tag-table offset, +0x17c resident header block.
`rpcs3_final124_bootflow.py --package HEXID` now dumps it (`--request` adds 0172), as of `9fce601`.

**Package 0172 offline.** `_2` (01.20) is an overlay. Of its 118 blocks, 68 are stored in patch 0
(the disc `_0`), 49 in patch 1 (01.13 `_1`) and 1 in itself. Entry 0, the root tag `0x80ae4000`
(48 bytes, pointing at entries 2 and 1), sits in a patch-0 block. `d1_pkg.py blocks --verify` on
`_2` with `_0`/`_1` alongside: 118/118 SHA-1 match, so the shipped chain is internally
consistent. Entry 0 is byte-identical in `_1` and `_2`. A word scan of all entries for
cross-package tags is too noisy to isolate a missing dependency (about 230 plausible packages in
both `_0` and `_2`); not pursued.

| Run (d1p3) | Manifest | 0172 record (+0x140, build time, entries) | 0172 files opened | Stage 19 | Slot 3 | Active request |
| --- | --- | --- | --- | --- | --- | --- |
| FINAL124_NEXT2 | `manifest-final-smoke.bin` (`_2` only, disc shared) | `_2`, `0x56fcaa7c`, 949 | `_2` only (async + FIOS); `_0`/`_1` never | substate 12 | 1, `0x01878002` | `0x01fa0002` |
| FINAL124_CHAIN_D1P3B | `manifest-final-bootstrap-chain.bin` (`_0`+`_1`+`_2`, shared `_4`) | `_2`, `0x56fcaa7c`, 949 | all three, by async and by FIOS (`_2`, `_1`, disc `_0`) | substate 12 | 1, `0x01878002` | `0x01fa0002` |
| FINAL124_DISC0172_D1P3 | `manifest_0151_override.bin` (run 020; `_0` only) | `_0`, `0x53b251f2`, 943 | disc `_0` (async + FIOS) | substate **16** | **2**, pending 0 | `0x02fa0002` |

**Conclusion.** The substate-12 wait follows the 01.20 version of package 0172 (`_2`). Making its
base files reachable (the explicit chain) changes which files FIOS opens, but not the outcome: the
same kind-2 request, its kind-3 parent `0x01fa0002` and the retry slot `0x06fa0001` remain. With
the disc `_0` the slot completes (run 020 reproduced), but the final manifest with `0172_0`
crashed in run 021. The next question is what the kind-3 request `0x01fa0002` waits on while
`_2` is registered. Next test: hook-trace the request poller `0x14e864` and the status writer, or
diff first-call coverage between FINAL124_NEXT2 and FINAL124_DISC0172_D1P3 around the load.
`01a3` also differs (`_1` vs `_0`) between the two manifests.

### 2026-10-01 (cont.): first-call coverage, stuck vs passing (d1p3)

FINAL124_COV_FINAL (final manifest; substate 12) against FINAL124_COV_DISC0172 (run 020
manifest; substate 16), each 420 s with `coverage` in `RPCS3_FINAL124_SMOKE001\d1\d1ppu.txt`
(restored afterwards). About 82k first calls each, 81,838 shared. `scripts/d1_coverage_diff.py STUCK
PASS --after 0:01:20` lists 3 blocks only in the stuck run and 746 only in the passing run.

- **Stuck run only:** `0x103b68/0x103b90/0x103ba4` (job #1, 114.8 s), the retry arm of a
  lock-free CAS loop (`lwarx`/`stwcx.`, up to 10,000 tries) in `0x103828`. Contention: a symptom,
  not a cause.
- **Passing run only, from 112.29 s:** partitions inside `0xb7134c` called from `0xed458`, a
  20-byte digest check (`sha1(data, len) == expected`, called by the header loaders `0xeaf6c`
  and `0xebd68`). Its padding branch depends on data length, and `_0`'s tables differ in size
  from `_2`'s, so this difference is probably not causal.
- **Passing run only, from 119.46 s:** the completion path, starting at `0xf13d8` inside
  **`0xf12a8`** (called from `0xe6d28` and `0xe7f40`), which both runs call. Then the stage-19
  tick advances (`0x362e74`...). The partition at `0xf13d8` is reached only when the u16 at +0x30
  of the entry `0x19c6520 + idx*0x44` (idx = `0xf12a8`'s 8th argument, r8/r20) is **not**
  `0xffff`. Lead: in the stuck run that entry stays empty. Next, read the `0x19c6520` table live
  in a stuck run and identify what fills +0x30, and which caller (`0xe6d28` or `0xe7f40`) passes
  the index.

**Correction (FINAL124_SLOTS_STUCK, live, same session):** `0x19c6520` is not a stream table. Its
three real 0x44-byte entries are **memory arenas**: base `0xc0000000` (RSX local memory, size
0x0d91f000, used 0x1f7900), `0x30aa1d80` (0x6ff000, used 0) and `0x311a1e00` (0x4ff000, used
0x44100). +0x20 is the granularity, +0x30 the u16 head of an allocation list (`0xffff` in all
three while stuck). Past entry 2 is unrelated data. `0xef20c` returns size minus used, so
**`0xf12a8` is an arena allocator** (fallback arena via its 9th argument; errors -0x46, -0x4e,
-0x51). The passing-only `0xf13d8` path walks an existing allocation list, which differs because
the loaded data differs. **Like the SHA-1 padding branch, this is data-driven, not shown to be
causal.** Nothing in the stuck run is out of memory. First-call coverage cannot isolate the
blocker, because the stuck run has no unique error path. Next approach: trace the lifecycle of the
kind-3 request `0x01fa0002`, i.e. which code writes request status byte +7 and what the scheduler
does with the active request each tick, using d1p3 hooks with register dumps on the status
writers.
