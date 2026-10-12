# Capture-rig audit - deletion manifest, untracking manifest, promotion record

> # ⛔ CONTAINMENT FINDINGS FIRST (shell-script lane, 2026-08-11)
>
> **No breach was found.** The topology enforcement in `rig-mode.sh` and the namespace containment
> in both launchers are sound as written. But two defects make the fence UNVERIFIABLE, which
> `status/LESSONS.md` treats as the worst state:
>
> 1. **The containment assertion cannot fail.** `capture-rig/rig-mode.sh status` exits 0 in every
>    case, including `=> INDETERMINATE - set a mode explicitly` and LIVE. `CLAUDE.md`'s
>    non-negotiable says containment is *asserted* with that command; any automated caller checking
>    the exit code gets a false green on an indeterminate or LIVE rig. Not edited by this lane
>    (containment path). Proposed fix and its named verification gate: section 8, recommendation 1.
> 2. **`CLAUDE.md` line 208 is false, and the containment step has two drifted implementations.**
>    Nothing executes `run-contained-emulator.sh` (repo-wide grep: prose hits only);
>    `run-boot.sh:410` calls `run-signon-experiment.sh`, which implements its OWN unshare +
>    resolv + assertion, and the two have drifted (different DNS assertion targets, signon-only
>    hosts redirect and manifest refusal). One protocol, one implementer applies to containment
>    too. Fix: section 8, recommendation 2.
>
> Full audit: section 8 below.

**State (2026-08-11):** audit complete and section 1 EXECUTED on operator instruction: all 30 files
were archived to `historicaldocs/python-capture/` (gitignored, so they leave the tracked tree but
survive locally) and the two `__pycache__` directories were purged. The gitignore manifest in
section 2 is still PREPARED and unexecuted. The `make-manifest.py -> d1-pkg` promotion is
IMPLEMENTED and its verify verdict is CONFIRMED.

Line anchors in this document were re-verified on 2026-08-11 against the consolidated tree. Where
a previously cited doc moved to `historicaldocs/design/`, the row names the current location.

> ⛔ **Defect in this document's own first draft, recorded rather than quietly fixed.** The
> deletion manifest below originally carried 14 rows. The judge produced 31. The other 17 were lost
> because the workflow that wrote this file passed the manifest to its doc-writer through a 4000
> character slice, and a truncated handoff looks exactly like a shorter decision. The full list is
> restored below and marked. The lesson is the familiar one in a new place: a summary is not the
> document, and a handoff that silently drops its tail is indistinguishable from one that had
> nothing more to say. Rows 15 to 31 were never re-verified against the consolidated tree by the
> doc-writer, so their line anchors are the judge's, not re-checked.

---

## 1. Deletion manifest

> **EXECUTED 2026-08-11.** Every row below was moved to `historicaldocs/python-capture/`, preserving
> its subdirectory. Nothing was destroyed: `historicaldocs/` is gitignored, so the bytes stay on
> disk and leave the published tree. `historicaldocs/python-capture/INDEX.md` carries the
> superseder table and the two retired-defect traps. Verified before the move: no live Python
> import and no `SourceFileLoader` target anywhere in `capture-rig/` names an archived module.

| delete | superseded by | evidence and re-points |
|---|---|---|
| `capture-rig/peek-guest.py` | `capture-rig/d1guest.py read <vaddr> --deref` | Migration table `historicaldocs/design/runtime-state-access.md:275` declares it fully subsumed; verified `d1guest.py` ships the `read` subcommand. Re-point `.claude/skills/runtime-probe/SKILL.md:107` (the anchor-vs-probe-site warning must keep its referent) and the live rows `design/eboot-map.md:67` and `:621` in the same change. History mentions in `historicaldocs/design/character-and-orbit.md` need no edit. |
| `capture-rig/dump-descriptors.py` | `capture-rig/d1guest.py descriptors [--watch]` | Declared subsumed at `historicaldocs/design/runtime-state-access.md:276`; verified `d1guest.py` ships the `descriptors` subcommand and every live caller (queuez-debug skill, d1-trace `probe.rs`) already uses it. Re-point `design/eboot-map.md:67`; history lines in `historicaldocs/design/character-and-orbit.md` stay as history. |
| `capture-rig/summarize-network-trace.py` | `capture-rig/send-receive-ledger.py` (`parse_network_trace`, verified at line 618) | One format described twice; the ledger copy is the one `d1-observe` shells out to. Re-point `design/instruments-live.md:764` in the same turn (`historicaldocs/design/d1-network-trace.md:93` is history and stays). |
| `capture-rig/activity-launch-static-atlas.py` | `historicaldocs/ingress/round3/ACTIVITY-LAUNCH-STATIC-ATLAS.{md,json}` (frozen artifact) plus `d1-eboot atlas`/`xref` | Zero live referrers by measurement; second Python ELF reader by import. |
| `capture-rig/p2p-static-atlas.py` | `historicaldocs/ingress/round3/P2P-STATIC-ATLAS.{md,json}` (frozen artifact) plus `d1-eboot xref`/`atlas` | Zero live referrers; its control discipline is worth one read before deletion. |
| `capture-rig/package-symbol-inventory.py` | `d1-pkg stats` and `d1-pkg entries`, plus frozen `historicaldocs/ingress/round3/CONTENT-PACKAGE-SYMBOL-INVENTORY.{md,json}` | Spot-checked by the judge: `d1-pkg stats` on the pinned estate prints packages 1647 / symbols 3242, exactly the script's `EXPECTED_PACKAGES`/`EXPECTED_SYMBOLS` guard constants. |
| `capture-rig/build-launch-catalogs.py` | its own frozen outputs in `historicaldocs/ingress/round3` | All five required inputs are frozen ingress artifacts; no caller exists. |
| `capture-rig/build-reference-catalog.py` | frozen `historicaldocs/ingress/round3` atlases (its only inputs) | Lowest-referenced file in its bucket. |
| `capture-rig/activity-host-contract.py` | `d1-server/crates/d1-activity-host` (the crate owns the lifecycle for real) | Delete only the script. The crate and `design/activity-lifetime-state.md` are fenced to another lane and must not be touched. |
| `capture-rig/autopress.sh` | `capture-rig/autodrive.sh` (strict functional superset) plus `run-boot.sh:415`'s inline title-screen press | Judge overrode the live-client KEEP: repo-wide grep confirms the only reference is `autodrive.sh:9`'s own header comment. Update that comment in the same change. Also removes one of the two files lacking an SPDX header. |
| `capture-rig/activity-launch-event.schema.json` | `historicaldocs/ingress/round3/schemas/activity-launch-event.schema.json` (byte-identical) plus the in-code enum validation in `activity-launch-trace.py` | Judge overrode the trace-capture KEEP: grep proves zero live references in `design/`, `status/`, capture-rig scripts, `d1-server`, skills or `CLAUDE.md`; the validator never loads it. Deleting removes a second description of a format. |
| `capture-rig/d1-multipeer-event.schema.json` | `historicaldocs/ingress/round3/schemas/d1-multipeer-event.schema.json` (byte-identical) plus `multi-trace-correlator.py`'s in-code checks | Same measurement as its sibling: zero live references, validator enforces in code. |
| `capture-rig/d1-p2p-corpus.schema.json` | `historicaldocs/ingress/round3/schemas/d1-p2p-corpus.schema.json` (byte-identical) plus `p2p-corpus.py`'s in-code checks | Same measurement: zero live references. |
| `capture-rig/profiles/profile-allslots.bin` | `capture-rig/profiles/profile-populated.bin` | Zero references anywhere in the tree (repo-wide grep, re-run 2026-08-11); `profile-populated.bin` is the fixture the docs and launch recipes name. |
| | | |
| **rows 15-31, restored after the truncated handoff** | | |
| `capture-rig/profiles/profile-empty-slots.bin` | capture-rig/profiles/profile-populated.bin | The empty-profile premise is explicitly retired in historicaldocs/design/THE-EMPTY-PROFILE.md section 7. |
| `capture-rig/profiles/profile-probe.bin` | capture-rig/profiles/profile-populated.bin and profile-old-russia.bin | Superseded twice over, zero references. |
| `capture-rig/inventory-test/ (entire directory, 13 .bin files)` | capture-rig/inventory/ (current served generation, regenerable by d1-oracle inventory rebuild) | Zero referrers; 03-heavy.bin carries exactly the values d1-oracle's NEGATIVE controls refuse (hash-as-level, bucket-hash-as-equip-min), so it is a live trap on disk. |
| `capture-rig/loadout-titan/ (entire directory, 9 .bin files)` | capture-rig/inventory/ (current staging target) | Zero referrers; 03-heavy.bin carries the same retired hash-in-small-field defect the negative controls exist to reject. |
| `capture-rig/progressions/1716568313.bin` | the family-4/type-1 profile progression block at +0x3660 (historicaldocs/design/family2-replication-gate.md retraction banner, verified in place) | Built on a loudly retracted premise (family 2 is the roster/player-card family, not progressions). Re-point the d1-responder/src/main.rs:1745-1797 comment at the retraction in the same turn. |
| `capture-rig/progressions/3813964388.bin` | same retraction; real progressions live in the family-4/type-1 profile |  |
| `capture-rig/progressions/695578353.bin` | same retraction; keep the SOID 0x2975AEF1 in the responder comment, not the payload on disk |  |
| `capture-rig/progressions/root.f2root` | the well-provenanced f2t0 contact files in capture-rig/rosters* (same schema 0x808018B7) | Its own document records that its provenance is unrecorded. |
| `capture-rig/testdata/two-instance/layout.json` | d1-harness layout (binary verified present; its controls.rs:340 pins the port-3075 collision this file contains) | A known-wrong layout with a passing self-test is a trap. Re-point historicaldocs/design/two-instance-harness-design.md:159 first. |
| `capture-rig/oodle/global-xref.py` | d1-eboot xref (equality by construction: the wrapper execs the Rust binary) | Judge verified current CLAUDE.md has zero occurrences of global-xref; the citation that justified keeping it is gone. |
| `capture-rig/oodle/reloc-xref.py` | d1-eboot reloc (equality by construction) | Same: zero CLAUDE.md occurrences, zero importers. |
| `capture-rig/oodle/dis-eboot.py` | d1-eboot disasm (disasm.rs:7 names this file as consolidated input) | BLOCKER FIRST: its docstring holds the only copy of the BAP response-envelope instruction table; move that prose into a design doc or d1-eboot doc-comment before deleting. |
| `capture-rig/oodle/dis-annot.py` | d1-eboot disasm (rip-relative resolution native; equivalence committed as a test in d1-eboot/tests/controls.rs) |  |
| `capture-rig/oodle/rsat-encoder-disasm.py` | d1-eboot disasm anchor modes (Anchor::Snap / Anchor::Forced) | Its whole value, refusing a misaligned start, is a first-class mode of the replacement. |
| `capture-rig/oodle/state-writer-scan.py` | d1-eboot field-xref (its pinned controls, verified exit 0, cover this script's own declared blind spot: the field address escaping into another function) | Leave a supersession note where historicaldocs/design/ATLAS.md and historicaldocs/design/character-and-orbit.md cite it as provenance. |
| `capture-rig/oodle/envelope-span-oracle.py` | capture-rig/oodle/probe-envelope-span.py (settled the same question: the span is a wrapper-level trailer) |  |
| `capture-rig/__pycache__/ and oodle/__pycache__/` | regenerated on next import (python3 -B avoids creating them) | Untracked and already gitignored; plain rm, no git action. Wait for a quiet window since a live run is up and imports may be in flight. |

---

## 2. Gitignore manifest

Untrack via `git rm --cached` (preserves the on-disk copy) and add to `.gitignore`. Every row names
the command that regenerates the file, because an ignored file with no generator is a trap for a
fresh clone.

| file | regenerated by |
|---|---|
| `capture-rig/eboot-thiscalls.tsv` | `d1-server/target/release/d1-eboot thiscalls` (or `capture-rig/eboot-atlas.py`, which writes it); add to `.gitignore` beside its four already-ignored siblings. |
| `capture-rig/eboot-imports.tsv` | `d1-server/target/release/d1-eboot imports --tsv` (default `--dict capture-rig/nid-candidates.txt`); same `.gitignore` line. |
| `capture-rig/bap-message-table.tsv` | ⭐ **`d1-bap message-table --tsv <path>`** (2026-08-11: ported; byte-identical to the Python's output, executed twice - see section 7). Values independently carried as Rust consts in `d1-bap/src/tables.rs` and drift-checked by `d1-bap controls`; nothing reads the file at build or run time. |
| `capture-rig/item-content-index.tsv` | ⭐ **`d1-oracle item-index write`** (2026-08-11: ported; the Python is superseded). Byte-identical to `item-index.py --write` apart from the `# GENERATED by` line; `d1-oracle item-index diff --against <file>` executes that comparison and exits 2 on any other difference. Needs the owned-media package dump, not Python. |
| `capture-rig/item-art-arrangement.tsv` | ⭐ **`d1-oracle art-arrangement write`** (2026-08-11: ported). Same byte-identity and same `diff` subcommand. ⛔ The consumer's provenance check was WIDENED in the same change: it used to literal-match `"GENERATED by capture-rig/oodle/art-arrangement.py"`, which rejected an honestly-labelled Rust-generated table; it now accepts any of `item_index::KNOWN_GENERATORS` and still refuses a table with no generator line. |
| `capture-rig/world-namespace.tsv` | NONE YET - no script in the tree writes it; untrack anyway (licensing: 1,114 symbol names extracted from copyrighted packages must not ship). `git rm --cached` preserves the on-disk copy. Follow-up work item: add a symbol-inventory writer to `d1-pkg`, which already parses the package headers. |
| `capture-rig/testdata/single-participant.json` | `python3 -B capture-rig/single-participant-fixture.py > capture-rig/testdata/single-participant.json` (byte-identical regeneration verified by the fixtures classifier). |
| `capture-rig/testdata/six-player.json` | `python3 -B capture-rig/six-player-fixtures.py` (seeded, deterministic, byte-identical regeneration verified). |

---

## 3. Promotion record: `make-manifest.py -> d1-pkg` - IMPLEMENTED, verify verdict CONFIRMED

The port of `capture-rig/make-manifest.py` into the `d1-pkg` crate is implemented and verified.
Verdict: **CONFIRMED**, with one real hardening gap found - inherited from the prototype rather
than introduced by the port. That gap remains open and is the crate's next hardening item.

**Exit codes** (all re-run by the verifier, not quoted): whole-workspace `cargo build --release`
from `~/dev/destiny1-preservation/d1-server` exits 0 with zero warnings after a forced rebuild;
`cargo test -p d1-pkg` passes 18/18 in both debug and release; `d1-pkg controls` 20/20 PASS exit 0;
`d1-pkg --dir /nonexistent controls` exits 2; `d1-pkg manifest --policy all` over the package cap
exits 1, refuses, and writes no file. HEAD builds independently: detached worktree, whole-workspace
release build 0, `cargo test -p d1-pkg` 18/18; worktree removed afterward.

**Golden diff, executed not asserted.** The prototype's `all` policy exits 2 rather than write
1,644 packages, so the verifier loaded `make-manifest.py` via `importlib` and lifted `PACKAGE_CAP`
in memory only (`md5sum -c` before and after: file unmodified). Byte-identical by `cmp` on all four
policies: `newest` (20,370 B, `adfbd8a4...`), `symbols-newest` (9,387 B, `c70899fa...`), `all` with
the cap lifted on both sides (85,649 B, `8fc47baf...`), and `all-generations --lang en` against the
served `full_en.bin` (52,797 B, `0c5bba3c...`, a capability the Python prototype lacks). The diff
is non-tautological: the Python digests were derived fresh and equal the constants pinned in
`manifest.rs`, so the goldens were not self-derived. `full_en.bin` mtime is unchanged
(2026-08-06 17:39:18); nothing was written to the served artifact.

**Both corrections independently reproduced** by the verifier's own estate walk: `newest` selects
396 / keeps 393, `symbols-newest` selects 178 / keeps 175 (so the earlier "178/396" figures were
pre-drop and swapped). Engine ids {66, 73, 377} map to 6 files, both spellings. Locale histogram:
938 neutral + 101 en - 6 = 1,033.

**Break attempts.** Seven refusal paths, all loud and correct, each exiting nonzero: empty dir,
missing dir (distinguished from empty, which the Python cannot do), bad `--lang`, bad `--only` hex,
guid that cannot match key 101, guid at or past `char[0x25]` even when it matches, cap overflow.
No refusal wrote a file. The public decoder survived all 52,797 truncations of the real served
manifest plus 200,000 pseudo-random inputs with zero panics; a 4 GB length bomb is rejected
without allocation.

---

## 3b. Promotion record: `gate-check.py -> d1 gate` - IMPLEMENTED

The boot-path regression assert is now `d1 gate [RUN]` (plus `d1 controls`), living in
`d1-server/crates/d1-run/src/gate.rs` with the subcommand wiring in `d1-server/crates/d1/src/main.rs`.

**The no-format-twice violation is closed the same way `d1-trace` closed it:** probe records enter
through exactly one door, a shell-out to `capture-rig/decode-account-trace.py --json`. No Rust code
describes the 232-byte record layout. When promotion item 3 below absorbs the decoder into Rust,
the single call site in `gate.rs::load_probe_tally` is the seam to repoint. Bootflow reach is
measured on the CLI's existing `Entering state` ladder parser rather than a second regex.

**Exit codes, all executed:** whole-workspace `cargo build --release` exit 0 (one pre-existing
`d1-live` warning, untouched); `cargo test -p d1-run --release` 17/17 exit 0; `d1 controls` 7/7
PASS exit 0; perturbed (`D1_REPO=/nonexistent` from a cwd outside the repo, so the decoder cannot
be found) `d1 controls` fails 2 of 7 loudly and exits 2.

**Golden diff, executed not asserted:** `gate-check.py` and `d1 gate` were both run over **59 real
run directories** (the 40 newest and 30 oldest `runs/` entries, overlapping): stdout byte-identical
and exit codes equal on all 59, including a known-PASS run (`20260810-161501-RDOC-ORBIT-JANK`,
both exit 0) and a known-FAIL run (`20260811-050950-GEARED-385-VISUALS-X11`, 503 gate regressed,
both exit 1). `render_text` reproduces the prototype's output byte for byte, banner included, so
future drift is diffable.

**Controls (7, each a `#[test]` and a `d1 controls` row):** all-pass with pinned details against a
real 6-record trace excerpt (`d1-run/testdata/gate-trace-excerpt.bin`, cut from the known-PASS run,
known answer 4/6 acct_ready true pinned by the canonical decoder); inverted 503 removal flips
exactly one gate; `REACHABLE` poisons containment despite `no route`; the 2-byte stub manifest
fails; the alive-BAP-connection trap (ATLAS 4.2b, run `20260807-162724-subackhdr-1`) both directions;
a one-byte-truncated trace fails loudly with a note; crash on either side fails the run.

**Named divergences, all deliberate:** default run selection is the newest *timestamp-named* run
directory, not newest `runs/` entry by mtime (the prototype could pick a stray file such as
`guest-writes.jsonl`; a bare name sort would let scratch dirs like `sweep-*` win forever - caught
and fixed in adversarial verify); an explicit nonexistent run path is a loud refusal, where the
prototype printed a confident all-FAIL ledger for a typo;
a corrupt or pre-232-era trace fails the probe gate loudly (the prototype silently skipped
bad-magic records); bootflow reach requires an `Entering state` line (a mention elsewhere in the
log no longer counts) - all 59 sweep runs agree despite these. The pre-232 boundary was then
measured directly: on `runs/20260807-062241-account-trace` (an 864-byte old-format trace the
decoder refuses) both tools exit 1 with identical gate verdicts, and the only textual difference
is the control line's record count (prototype salvages 1 misaligned magic hit, `d1 gate` reports 0
plus a stderr note). That is the divergence behaving exactly as designed.

**Deprecates:** `capture-rig/gate-check.py` only. `decode-account-trace.py` is NOT deprecated - it
is now a load-bearing runtime dependency of `d1 gate` (as it already was of `d1-trace`), which
raises the priority of promotion item 3.

---

## 3c. Promotion record: `message-catalogue.py -> d1-eboot catalogue` - IMPLEMENTED

The does-the-client-want-a-reply oracle is now `d1-eboot catalogue`
(`d1-server/crates/d1-eboot/src/catalogue.rs`), callable from Rust as the server-side safety
question requires. The port consumes `d1_eboot::descriptors` for the anchor/sentinel/field layout,
closing the duplicate description the TODO bullet named. Section flags mirror the prototype
(`--controls --envelopes --policies --investment --schemas --d1bap`); the machine blob is
`--json-out PATH`, or the global `--json` to stdout.

**Exit codes, all executed:** whole-workspace `cargo build --release` exit 0, zero warnings
(re-verified with the d1-eboot fingerprint invalidated, so the binary under test is the
feature-unified one); `cargo test -p d1-eboot` 42/42 exit 0 (4 lib + 5 catalogue + 33 controls);
`d1-eboot catalogue --controls` exit 0; `d1-eboot controls` exit 0 with the 14 catalogue rows
appended (45 rows, all PASS).

**Golden diff, executed not asserted:** prototype and port both run over the pinned eboot plus the
pinned `capture-rig/schemas/investment-message-definitions.bin`. Full no-flag run: 300 lines,
`diff` exit 0, byte-identical. `--envelopes` and `--policies` modes: byte-identical. `--json`
blob: identical after `jq -S` normalization on both sides (431,122-byte Python file). The full
Python output is now pinned as `d1-eboot/tests/fixtures/message-catalogue-full.txt` and reproduced
byte-for-byte by a `#[test]`, so the golden diff re-executes on every test run instead of decaying
into an assertion.

**Controls, including ones that can FAIL:** the prototype's 14 rows (4 pinned RSAT schemas, the
503 cross-instrument row referenced from a live registry walk, the two-scan envelope coverage
control, header sizes, 2 pinned policies, 2 pinned definition rows, the paired absence control,
investment tag resolution, the record-length rule), each both a `#[test]` and a `controls` row.
Inverted, both executed: flipping one byte of policy family 13's response-type thunk changes the
derived response (`0xd -> 0x1c`), flips the pinned row to `*** MISMATCH ***`, and exits 2 with
every section suppressed (proved as a `#[test]` and again through the CLI on a perturbed image
copy); a truncated or count-corrupted definitions file is refused with the prototype's exact
`stride check failed` wording.

**Named divergences, all deliberate:** control failure exits 2, not the prototype's 1 (crate
convention: exit 2 means the output may not be cited); JSON pretty-printing differs (compare with
`jq -S`, as the verify did); the prototype's single-global-bias addressing survives only behind a
guard: `Reader::new` refuses any image whose LOAD segments do not all share one bias, which is the
per-segment coincidence (ATLAS 4.1) that made the prototype correct on this build.

**Deprecates:** `capture-rig/message-catalogue.py` only. Nothing imports or executes it; every
other occurrence of the name is a prose citation of `historicaldocs/design/message-catalogue.md`.
Re-point at archive time: `design/eboot-map.md:47` and `:60` (generator attribution),
`design/queuez-replication.md:348`, `:395`, `:1369` (control attribution), and
`design/observability-and-ui.md:909`, `:966` (tool inventory rows).

---

## 4. Future promotion candidates (Python -> Rust), in order

1. `message-catalogue.py` -> `d1-eboot`: `status/TODO.md`'s own named next port (the "Next port,
   in this order" bullet); the does-the-client-want-a-reply oracle is safety-relevant and should be
   callable by the server, not a Python script. **IMPLEMENTED 2026-08-11, see section 3c. Verify
   verdict: CONFIRMED (section 7.2) - independent re-execution held under break attempts; five
   minor non-blocking hardening notes recorded.**
2. `gate-check.py` -> a `d1 gate` subcommand: fold the boot-path regression assert into the CLI
   that already parses the trace record, closing the named no-format-twice violation at
   `gate-check.py:33`. **IMPLEMENTED 2026-08-11, see section 3b. Archival blocked:
   `run-boot.sh:425` still calls the Python (section 8).**
3. `decode-account-trace.py` -> absorb into d1-trace: Tier-1 overdue runtime dependency of shipped
   binaries; needs a dedicated session because d1-trace scrapes its prose for anchors and keys
   repo-root discovery on the exact path. **DONE for d1-trace 2026-08-11
   (`d1-trace/src/probe_decode.rs`, verify CONFIRMED - see Round 3, R3.2 lane A). Residue
   RESOLVED 2026-08-11: `gate.rs` now takes an injected `ProbeDecoder` wired to the Rust port by
   the `d1` CLI; the script is archivable in the next reviewed batch.**
4. `d1guest.py` `read`/`obj`/`tag`/`descriptors`/`poke` -> d1-live: the parity d1-live already
   advertises; a multi-session port that cannot start while a live run depends on the script.
   **READ SIDE DONE 2026-08-11 (section 7.1). Verify verdict: BROKEN on arrival, CONFIRMED after
   five corrections. `poke` deliberately NOT ported (its own reviewed change). Open: the
   `[U] live-parity` gate - the whole golden diff ran against a fixture because no emulator was
   up; settle command in `status/harness.md` Next.**
5. `find-in-guest.py` -> d1-live `search`: carry the do-not-scan-sparse-reservations constraint
   verbatim.
6. `live-namespace.py` plus `dump-registry.py`'s `walk()` and its sanity gate -> d1-live: the
   live-tier port plan is already written; nothing else can regenerate the namespace map.
7. `session-params.py` and `scenario-checksum.py` -> Rust, after the activity-host lane releases
   them: their offsets are already duplicated in Rust prose, the exact drift
   one-protocol-one-implementer forbids.
8. `name-hash-tables.py`, `type-registry.py`, `namespace-walk-verify.py` -> `d1-eboot` subcommands
   (already queued in `status/TODO.md`); the tag-formula derivation lands beside the constant four
   crates hardcode.
9. `bap-message-idspace.py` plus `bap-message-table.py` -> `d1-bap`: the tables they derive are
   already carried as re-derived Rust consts in `d1-bap/src/tables.rs`; moving the derivation
   beside the constants leaves the id space described once. **DONE 2026-08-11 (section 7.3).
   Verify verdict: BROKEN as delivered on one dropped refusal (the controls-before-dump gate),
   CONFIRMED after the correction. Fully offline-verifiable; no live gate.**

---

## 5. Judge downgrades - the audit's honesty record

Where the judge overturned or refused a classifier verdict, and why. These are kept so the next
auditor does not re-litigate them from scratch.

1. `capture-rig/field-pool.py`: UNSURE -> KEEP. Judge ran the side-by-side the classifier asked
   for: `d1-eboot descriptors --index --json --with-fields` exposes only
   kind/child_tag/raw_bits/offset per field; `field-pool.py` surfaces name hash, aux, align,
   flags A/B, blobA/B, csize and the tag-list trailer of the 0x58 leaf record, with 8 controls
   passing. Not redundant. Defect found: its default eboot path is a literal unexpanded `$HOME`
   string (scrub damage) - it only runs with an explicit `--eboot`.
2. `capture-rig/orbit-view.py`: UNSURE -> KEEP. It is the only regenerator of
   `orbit-destinations.tsv`'s dest_hash/slice_hash/view columns (`--table`, live read via
   `d1guest.py`); record it as that TSV's generator in
   `historicaldocs/design/orbit-as-a-viewer.md`.
3. `capture-rig/orbit-destinations.tsv`: UNSURE -> KEEP tracked. 46 rows, partially hand-enriched
   (name/status columns are not in `orbit-view.py`'s raw dump), live-derived and not one-command
   regenerable; de minimis size.
4. `capture-rig/characters/char-ruler-11-22-33-44.bin`, `char-sentinel.bin`,
   `capture-rig/profiles/profile-slot0-7743.bin`: UNSURE -> KEEP. Each is named by a design-ledger
   row; 2-20 KB each; evidence retention beats cleanup on the eve of going public.
5. `capture-rig/launch_knowledge.json`: UNSURE -> KEEP. Sole input of the KEEP tool
   `activity-launch-trace.py`. Coordinator action: stamp its snapshot block FROZEN 2026-08-07
   before the public push, since `current_gate` asserts a long-passed project state.
6. `capture-rig/client-bap-probe-sites.json` and `capture-rig/director-item-probe-sites.json`:
   UNSURE -> KEEP. Both probe-site catalogs are cited by `capture-rig/executed-addrs.tsv`
   provenance rows; the former is the `--catalog` input of `verify-client-bap-probe-sites.py`
   (`design/bap-transport.md:598`, with a known default-filename mismatch flagged in
   `historicaldocs/ingress/INTEGRATION-PLAN-round3.md`), the latter is named by `design/items-and-gear.md:910`
   (17 sites, ids 69-85).

---

## 6. Scope note

This audit prepared manifests only: it deleted nothing, moved nothing, killed no process, launched
nothing, and made no git writes. A live run was up throughout; its scripts and fixtures in
`capture-rig/` were left untouched.

---

## Round 3: the shipped-binary Python dependencies (2026-08-11)

Two shipped Rust binaries depended on Python at runtime - the project rule ("a reusable tool is a
Rust tool") violated in its worst direction, on the eve of the public release. Both claims were
re-verified against HEAD by measurement before any porting, both ports landed, and both drew a
CONFIRMED verdict from an independent adversarial verify pass that re-ran every command itself.
Crate fence for the round: this lane owned `d1-pkg`, `d1-oracle`, `d1-trace` only.

### R3.1 The two claims, measured `[C]`

**Claim 1 - `d1-oracle` reads Python-generated TSVs at runtime and prints the Python regeneration
command: CONFIRMED at HEAD.** `git show HEAD:d1-server/crates/d1-oracle/src/item_index.rs`:

```
61:  pub const DEFAULT_PATH: &str = "capture-rig/item-content-index.tsv";
190:      "{DEFAULT_PATH} not found in this directory or any parent. Generate it: \
191:       PYTHONPATH=~/.local/lib/d1-tools python3 -B capture-rig/oodle/item-index.py \
192:       --write {DEFAULT_PATH}")));
402:      python3 -B capture-rig/oodle/art-arrangement.py --staged --write \
```

One correction to the audit's framing: "a fresh clone gets a Rust binary that cannot work" was
overstated. Both TSVs are **tracked and clean** (`git ls-files` lists
`capture-rig/item-content-index.tsv` and `capture-rig/item-art-arrangement.tsv`), so a clone could
always *read* them. The real defect was that no clone could **regenerate or extend** them without
Python plus a hand-installed `unicorn` wheel - which, the verify pass proved, is importable by no
interpreter on this box either, so the Python generator path was already unrunnable even here.

**Claim 2 - `d1-trace` shells out to `capture-rig/decode-account-trace.py` and keys repo-root
discovery on that exact path: CONFIRMED at HEAD.** `git show HEAD:d1-trace/src/ingest.rs`:

```
148:  let script = match find_script(run_dir, "decode-account-trace.py") {
155:  let output = match Command::new("python3")
156:      .arg(&script)
157:      .arg("--json")
```

and `git show HEAD:d1-trace/src/controls.rs`:

```
90:   if p.join("capture-rig/decode-account-trace.py").is_file() {
```

**Extra finding from the round - CLOSED 2026-08-11:** `d1-server/crates/d1-run/src/gate.rs:130`
*also* shelled to the same Python script (`find_script(run_dir, "decode-account-trace.py")` then
`python3 -B ... --json`). `d1-run` sat outside both round-3 fences and was correctly left alone
then; a follow-up lane replaced the shell-out with an injected `ProbeDecoder` (the CLI wires in
`d1_trace::probe_decode`; the direct dependency would be a cycle), deleted `find_script`, and
rewrote the stale header comment. No Python-at-runtime site survives.

### R3.2 What landed, with the executed evidence

**Lane A - `d1-trace`: native probe decoder.** New `d1-trace/src/probe_decode.rs` (byte-faithful
port: 91-probe catalogue, 232-byte record reader with all four refusals verbatim, `explain()` with
the Python's quirks preserved including the dead `len(a) > 0x33` guard, JSON and human emitters);
`ingest.rs` decodes natively (subprocess removed) and the F4 anchor-scrape defect is closed
structurally - `explain()` now *emits* the anchor labels from the same constants the scraper
*reads*; `controls.rs` repo-root re-keyed on `capture-rig/` + `runs/` directories; new
`d1-trace decode <trace.bin> [--json]` subcommand with the Python's CLI contract (`error:` to
stderr, exit 2).

- Golden diff, executed: `python3 capture-rig/decode-account-trace.py --json` vs
  `d1-trace decode --json` over **every** `runs/*/account-trace.bin`: **193 byte-identical, 18
  refusal-identical (same stderr, both exit 2 - the pre-232-era runs), 0 mismatches**. Largest
  trace (13.9 MB in, 51,431,344-byte JSON out) byte-identical via `cmp`, JSON and human modes.
- Verify verdict: **CONFIRMED**, and it earned its keep - it found and fixed one latent defect
  (Python `json.dumps` defaults `ensure_ascii=True`; a crafted non-ASCII `armed` record diverged
  at the byte level; fixed with an exact-Python `ensure_ascii()` pass plus a unit test), and it
  proved both new controls by sabotage: disabling the magic check turned C-DECODE-REFUSE red,
  making `explain` ignore the sample bytes turned the inverted C-DECODE-ANCHOR-LABELS red, both
  exit 2, both sabotages reverted.
- Exit codes (verify pass): `cargo build --release` 0 (0 warnings), `--no-default-features` 0,
  `cargo test --release` 0 (79 tests), `d1-trace controls` 0 (48/48).
- Re-measured while writing this section: `d1-trace controls` exit 0;
  `PATH="" d1-trace decode --json runs/20260808-110410-AVAIL-ITEM-TRACE/account-trace.bin` emits
  16,809,674 bytes, exit 0 - no Python reachable, full decode.

**Lane B - `d1-oracle`: native TSV generators and the Oodle content chain.** New
`d1-server/crates/d1-oracle/src/oodle.rs` (the client's own decoder at
`0x1267120`/`0x12671A0`/`0x12672E0` driven by the pre-existing Rust harness `harness.rs`, plus the
generation chain: block spanning, entry-by-tag, no `/tmp` cache) and
`src/art_arrangement.rs` (`FUN_00289E80` and the four self-relative walks, every bounds guard
carried across including which comparison each uses); `item_index.rs` gained `write` and
`diff --against` subcommands and its refusals now name the Rust commands (`d1-oracle item-index
write`), not Python. One deliberate residue: the malformed-row message at `item_index.rs:539`
still says "regenerate with art-arrangement.py" - cosmetic, worth fixing next touch.

- Golden diff, executed three times independently (port agent, verify agent, and once more while
  writing this section): `d1-oracle item-index write` and `d1-oracle art-arrangement write` vs the
  tracked TSVs = **exactly one changed line each, the `# GENERATED by` byline**; 7,552 lines /
  203,123 bytes and 55 lines / 2,956 bytes otherwise byte-identical. Cross-implementation digest:
  `d1-oracle oodle block --pkg ps4_investment_globals_012f_3 --block 1063` yields sha1
  `b7af28cbb6410f85ebf1b110e9a58af30594c1f3` (262,144 bytes) - the same digest the Python path
  recorded at HEAD in `capture-rig/oodle/content-table.py`. Two implementations, one digest.
  Provenance note from the verify pass: the Python side of that pin is a **pre-existing**
  measurement reproduced by the Rust, not a fresh run - `unicorn` is not importable here.
- Verify verdict: **CONFIRMED**, with corrections applied. It overturned the port report's
  headline: the whole-workspace build is **exit 0**, not 101 - the 101 was the concurrent lane
  mid-edit in `d1-live/src/registry.rs`, gone on re-run. It proved all three inverted/negative
  controls by perturbing the perturbations (each went red with exit 2, then reverted green). It
  ran a break battery: every doctored/truncated/absent input refuses loudly with exit 1 or 2; the
  duplicate-hash refusal fires; one latent faithful-to-Python hole named
  (`decompress_whole(&[])` returns an empty `Ok`, unreachable today).
- Exit codes (verify pass): whole-workspace `cargo build --release` **0** (twice, including a
  from-scratch recompile); `cargo test --release -p d1-oracle -p d1-pkg` 0; all three `controls`
  subcommands 0.
- Re-measured while writing this section: `d1-oracle {oodle,item-index,art-arrangement} controls`
  all exit 0; whole-workspace `cargo build --release` exit 0.
- Cargo.lock: **one line from this lane** (`d1-pkg` as a dependency of `d1-oracle`, keeping
  GPL-2.0 `unicorn-engine` confined to one crate). **No new external crate.** `d1-trace` is
  deliberately outside the workspace; its lock was untouched.

### R3.3 Sizing verdict on the Oodle lane: ONE ROUND, and it was one

The audit's sizing premise was false and must not be re-derived. There is **no codec to port**:
`oodle-decompress.py` was a ~35-line wrapper whose actual decode loop is 13 lines, running
Destiny's own decoder lifted from `eboot.bin` under CPU emulation - and the emulation harness was
**already in Rust** (`d1-oracle/src/harness.rs`, ported earlier with every paid-for warning
carried across). The scary "the Oodle decode is ~30 s under Unicorn" comment
(`art-arrangement.py:88`, `item-index.py:47`) was measured **wrong**: the Python decode of block
1063 takes 0.834 s. The real 67.7 s cost was `extract-entry.py` launching `d1-pkg info` as a
subprocess once per package across 1,647 packages (61 s of *system* time) - a subprocess storm,
not a codec, eliminated by `d1_pkg::Estate::load` in-process. The `/tmp` disk caches existed to
paper over that storm and do not exist in the Rust path.

**Nothing remains on the Oodle lane except archival.** The remaining Python-at-runtime work is a
different, smaller lane:

- ✅ **CLOSED 2026-08-11:** `d1-run/src/gate.rs` no longer shells to `decode-account-trace.py`.
  It takes an injected `ProbeDecoder`; the `d1` CLI passes `d1_trace::probe_decode` as a library
  call (not a subprocess). The note-degradation survives but only for a real reason (refused
  trace bytes); proven Python-free with a poisoned `python3` over a 17,862-record real trace.
- ✅ **CLOSED 2026-08-11:** `d1-trace`'s blob-sidecar lane no longer shells to
  `capture-rig/world-launch-command.py` - the D1BL reader is native (`d1-trace/src/blob_decode.rs`,
  surfaced as `d1-trace blob`, pinned by `C-BLOB-*` controls). The d1-dash live lanes still shell
  to `d1guest.py`/`state-machine.py` **by design** - they carry the liveness ladder and are not
  fixture or decode paths.
- The 232-byte trace layout is still described a second and third time in
  `capture-rig/ready-byte-report.py` and `gate-check.py` (pre-existing duplication, documented in
  `d1-trace/src/model.rs`).

### R3.4 Can a fresh public clone build working fixtures without Python?

**YES.** The fixture path (`d1-oracle character build-peer`, rosters, profiles, the Activity Host
messages) needs the two TSVs, which are tracked, read natively, and now *regenerable* natively
(`d1-oracle item-index write`, `d1-oracle art-arrangement write`) - regeneration requires the
owned-media package dumps, which no clone gets from git, but that is the owned-media policy, not a
tooling gap, and the Python generators needed the same dumps *plus* an unpublishable
hand-installed `unicorn` wheel. `d1-trace` decodes traces with `PATH=""` (proved above). The one
Python-at-runtime residue in shipped Rust is `d1-run/gate.rs` - a run-summary lane, not fixture
building, and it degrades to a note rather than failing a clone.

### R3.5 Deprecated Python - PREPARED, not executed (coordinator's archival batch)

Nothing was deleted or moved. Retraction/supersession banners were placed where noted.

| file | superseded by | banner | archivable? |
|---|---|---|---|
| `capture-rig/oodle/oodle-decompress.py` | `d1-oracle/src/oodle.rs` | placed | yes |
| `capture-rig/oodle/item-index.py` | `d1-oracle item-index write` | placed | yes |
| `capture-rig/oodle/art-arrangement.py` | `d1-oracle art-arrangement write` | placed | yes |
| `capture-rig/oodle/extract-entry.py` | in-process `d1_pkg::Estate` (its only runtime consumer was `art-arrangement.py`) | **not placed** | yes, once bannered |
| `capture-rig/oodle/d1-oodle-harness.py` | `d1-oracle/src/harness.rs` (pre-round port) | in README | only together with its loaders |
| `capture-rig/decode-account-trace.py` | `d1-trace decode` / `probe_decode.rs` | - | ✅ **YES as of 2026-08-11** - `d1-run/src/gate.rs` repointed (injected `ProbeDecoder`, wired by the `d1` CLI); no runtime invoker remains. Archival itself is the coordinator's reviewed batch |

`capture-rig/oodle/content-table.py` and `verify-sweep.py` are audit tools, never in a shipped
binary's runtime path; they load the harness and carry their own copy of the decode loop, so they
are the coordinator's call, not this round's deprecation.

---

## 7. Porting round 2 - queue items 4 and 9 landed, item 1 verified (2026-08-11)

Each port below is an author report paired with an independent adversarial verify that re-ran
every command itself. Two of the three verify reports reached this doc-writer **truncated in the
handoff** (the same defect this document's own first-draft banner records), so every number in
7.1 and 7.3 marked "re-measured" was re-executed against the tree while writing this section, not
transcribed.

**THE EMULATOR WAS CLOSED for the whole round.** No live client existed and none was launched.
Every half that is offline-verifiable was verified; the halves that need a live target are named
`[U]` gates with the exact command that settles them.

| port | landed in | controls | golden diff | verdict |
|---|---|---|---|---|
| `d1guest.py` read side -> `d1-live` | `d1-live` (`registry.rs`, `fixture.rs`, `format.rs`, `readcontrols.rs`, `session.rs`, `main.rs`) + `capture-rig/golden-diff-d1guest-vs-d1live.sh` | 18/18, exit 0 | executed, 4 configs x 21 comparisons, ALL IDENTICAL | **BROKEN on arrival -> CONFIRMED after five corrections** |
| `message-catalogue.py` -> `d1-eboot catalogue` (section 3c) | `d1-eboot/src/catalogue.rs`, `tests/catalogue.rs`, pinned fixture | 45 rows PASS, exit 0 (14 catalogue) | executed, 300 lines + JSON via `jq -S`, byte-identical | **CONFIRMED** |
| `bap-message-idspace.py` + `bap-message-table.py` -> `d1-bap` | `d1-bap/src/derive.rs` + the new `d1-bap` binary | ALL 18 PASS, exit 0 (incl. 4 drift checks) | executed, both TSVs byte-identical | **BROKEN as delivered (one dropped refusal) -> CONFIRMED after correction** |

Whole-workspace `cargo build --release` from `~/dev/destiny1-preservation/d1-server`: **exit 0**
(re-measured while writing this section). One cross-lane note for the record: the d1-live verify
initially could not reproduce "build: 0" because a concurrent lane had `d1-oracle` mid-edit
(non-exhaustive match, exit 101); it was that lane's transient state, since fixed, and the same
confound appeared in reverse in the d1-oracle verify. Both resolved to exit 0 on re-run.

### 7.1 `d1guest.py` read side -> `d1-live` - CONFIRMED after five corrections

`d1-live read|obj|tag|descriptors|syms|live|base` is a behaviour port: byte-identical stdout and
identical exit codes (1 refusal / 2 do-not-cite / 3 client DIED), driven by
`capture-rig/golden-diff-d1guest-vs-d1live.sh` against `d1-live __mem-target` - a fixture process
mapping the REAL eboot, whose registries can be made wrong on purpose
(`--perturb family-stride|verify-window|duplicate-anchor`). That is what finally gives the read
side controls that can FAIL; the Python's `selftest` never could, because nobody can ask a real
client to be wrong.

**Exit codes** (verify's own, spot re-measured by the doc-writer): `cargo test -p d1-live
--release` exit 0 (19 lib + 9 readside + 11 watchpoint); `d1-live controls` exit 0, PASS 18/18
(re-measured: 18/18, exit 0); golden diff `none` re-measured: ALL IDENTICAL, exit 0, fixture
killed, `pgrep -f "[s]hadps4 -p "` count 0 afterward.

**Golden diff, executed not asserted:** all four configurations x 21 comparisons ALL IDENTICAL
before and after the corrections, and the diff genuinely discriminates: `none` gives exit-0 dumps,
`family-stride` collapses the five `obj` cases to exit 1, `verify-window` and `duplicate-anchor`
refuse every command at exit 2 with different bytes. One coverage gap, named: `family-stride`
drops the liveness ladder to L2, so `cmd_obj`'s exit-2 cross-check refusal is never reached
through the diff.

**The five defects - the port answered where the Python refuses.** All fixed; the golden diff
stayed byte-identical on all four configs afterwards.

| defect | Python | port before correction |
|---|---|---|
| wrapping registry arithmetic | refuses (`OverflowError`) | `obj 576460752303423492:0` (4 + 2^59; 0x60 * 2^59 == 0 mod 2^64) aliased onto family 4/0 and printed the ACCOUNT object's bytes at exit 0 under "cross-check SKIPPED" |
| `--as <unknown>` | argparse `choices=`, exit 2 | printed provenance, ladder and a "64 bytes" header, then no data, exit 0 |
| defaulted `registry_layout` keys | `KeyError`, exit 1 | answered from a compiled-in constant - the sixth copy of what `d1guest.py` collapsed five scripts to remove |
| unbounded package offsets | unreachable (hardcoded) | `slot_data_base = 0x3C` -> slice panic, exit 101 (a crash is loud but is not a refusal) |
| `--len 0 --struct X` | 592 bytes (struct-sized) | read 64 |

Plus `tag 0xFFFFFFFFFF` silently narrowed to `0xFFFFFFFF`; now refused.

**Inversions - and one that is decoration.** Genuine: repointing the verify window at BSS ->
15/17 exit 2; one flipped byte in the real eboot -> 10/17 exit 2. The new
`garbage-in-refuses` control was itself inverted: restoring the wrapping multiply took the suite
to FAIL (17/18), exit 2, naming the aliased family; reverted, back to 18/18. ⛔ Decoration,
stated so nobody over-reads a green suite: corrupting the static family table still passes,
because the fixture generates its runtime entry table FROM `guest-symbols.json`'s static rows -
the cross-check control proves a **desync** is noticed; it can never prove the symbol file is
right. ⚠ The verify also nearly published a false inversion: three perturbations "failed"
identically because its own fixture was still running and the read-side group refuses to start
beside a discoverable process (that it fails rather than skips is correct and was confirmed).

**Refusals carried:** build gate, one-emulator rule, two-anchor agreement, verify + shifted
negative control, `-1` entry offset, cross-check SKIPPED-not-passed, L3-for-`obj`, `--allow-dead`,
cap bound, the 1/2/3 exit split, cache poisoning caught by controls 2 and 3 re-running on the
cached path, corrupt eboots (bad magic, truncated, no PT_LOAD, empty) all refused. Lost in message
only, not outcome: `no PT_LOAD segments` and `anchor is not file-backed` become base-control FAILs
at exit 2 rather than exit 1.

**Open gates and residue, named:**

1. `[U]` **live-parity.** Everything ran against the fixture (reach: `synthetic-minimal` about
   Destiny; what it measures is two readers of the same bytes agreeing). Settle on the first boot
   that reaches L3, with the command pair in `status/harness.md` Next: `descriptors`,
   `read profile_setup_kind`, `obj 4:0`, `tag 0x80801825 --len 128`, both tools `--no-cache`,
   both base caches cleared first, normalise only the `HH:MM:SS` stamp.
2. `[U]` **package slot stride** - the port reads `header+0x10` where the Python hardcodes
   `0x40`; only a live run confirms the real value.
3. `--pid` is a new escape hatch with no Python counterpart; defaults preserve the one-emulator
   refusal, but it is now bypassable.
4. Exit 2 is overloaded: clap usage errors and control refusals share it (inherited from
   argparse, not new).
5. `--allow-dead` and `--full-scan` remain unexercised by any control.

⛔ **`poke` was deliberately NOT ported** - a live write is the one operation where a half-ported
tool is dangerous, and its `runs/guest-writes.jsonl` journal is load-bearing. Its own reviewed
change; until then `d1guest.py poke` is the tool.

### 7.2 `message-catalogue.py` -> `d1-eboot catalogue` - verify verdict CONFIRMED

The port record is section 3c; this entry records the adversarial verify. Verdict: **CONFIRMED** -
independently re-executed (build 0 with the fingerprint invalidated so the tested binary is the
feature-unified one; 42/42 tests debug and release; golden diff re-run: 300 lines byte-identical,
JSON identical after `jq -S`, all five section flags identical). Break attempts all refused
loudly: urandom and empty files, truncated eboot, truncated defs with the prototype's exact
`stride check failed` wording, and a hand-crafted multi-bias eboot (LOAD phdr shifted +0x1000)
proving the ATLAS-4.1 parity guard is real. The inverted control was rebuilt independently
(family-13 response thunk imm XOR 0x11): both implementations flip to `*** MISMATCH ***`,
suppress every section, and are byte-identical even on the perturbed image; `--json-out` writes
no file on control failure, matching the Python's return-before-write.

Five minor non-blocking hardening notes, for the crate's next touch: (1) `Reader::inside`/
`const_fn` use unchecked `va + self.bias`, which wraps in release for adversarial pointers near
u64::MAX (contained by the controls gate; suggest `checked_add`); (2) `render_policies`/
`render_d1bap` print `0xffffffff` where the Python raises on a `None` type; (3) the multi-bias
refusal reuses `NotInLoad{offset:0}` and reads "file offset 0x0", loud but misleading; (4)
stride-valid-but-short defs panics exit 101 (parity with the prototype's own crash) instead of a
failing control row; (5) `vtables_by_rtti` anchors the digit run where the Python regex can match
mid-run - no effect on the pinned image, worth a comment if another eboot is ever added.

### 7.3 `bap-message-idspace.py` + `bap-message-table.py` -> `d1-bap` - CONFIRMED after correction

The derivation that used to live in two Python scripts now lives beside the constants it
generates: `d1-bap/src/derive.rs` plus the new `d1-bap` binary
(`controls | message-table | idspace | policies`, `--tsv` for the byte-identical successor of the
Python TSVs). `d1-bap controls` re-derives every `tables.rs` const from the pinned eboot - the
"no format described twice" violation item 9 named is closed, and drift between the consts and
the binary is now a red test rather than a silent divergence.

**Verdict: BROKEN as delivered on one dropped refusal; CONFIRMED after the correction.** The
derivation itself had no drift and both golden diffs were byte-identical, but the port as
delivered dropped the single most important refusal both Python tools carried: **running the
controls before every dump and refusing to emit on a failure** (`bap-message-table.py --tsv`
printed "refusing to write on a failing control" and exited 2). The verify proved it with a
confident-wrong-answer reproduction before fixing it; `main.rs` now gates every dump
(`gate(&run_controls(...))` before `message-table`, `idspace` and `policies`).

**Re-measured by the doc-writer** (the verify report was truncated in the handoff, so nothing
here is transcribed): whole-workspace build exit 0; `cargo test -p d1-bap --release` 161 passed,
exit 0; `d1-bap controls` ALL 18 CONTROLS PASS, exit 0; `d1-bap message-table --tsv` vs
`python3 -B capture-rig/bap-message-table.py --tsv` byte-identical by `cmp` (and regenerating the
tracked `capture-rig/bap-message-table.tsv` with the Python leaves `git diff` empty, so the diff
is against real current bytes); `d1-bap idspace --tsv` vs `bap-message-idspace.py --tsv`
byte-identical by `cmp`; the restored gate fires - a truncated definitions file makes
`d1-bap idspace` refuse at exit 2 with the Python's exact wording
(`stride check failed: 0x40 + 101*0x40 != 100 - this is not the definition table`).

**Facts the Python encoded, carried across** (doc-comment in `derive.rs`): relocated words read
as zero in the file, so every `.data.rel.ro` pointer comes from the reloc addend map; a thunk
that is not `mov eax, imm; ret` is not a constant (the three `xor eax, eax` notification rows
keep their blank `category` cells in the TSV for byte parity while the drift tests use the
truth); `+0x70` `pair_id` is re-derived only as a layout pin - `tables.rs` retracts it and the
pairing API remains `tables::response_for`; the definition-file stride check is a refusal, not a
warning; the two policy name tables disagree on rows 0/1, so comparisons key on RTTI class names.

**No live gate.** This port is a static derivation from the pinned eboot plus a pinned schema
file; every half of it is offline-verifiable and was verified.

---

## 8. Shell-script lane audit - capture-rig/*.sh (2026-08-11)

All ten scripts read in full. Nothing was executed against the rig, modified, killed, or
committed. **Provenance note:** the lane's report reached this doc-writer truncated after the
`rig-mode.sh` row; the rows below for `rig-mode.sh` (completed), `capture.sh`, `preflight.sh`,
`emu-display.sh`, `autodrive.sh`, `zeek-control-capture.sh` and the golden-diff script were
re-derived by the doc-writer reading each file in full, not reconstructed from memory.

Containment verdict: **no breach** - see the banner at the top of this document for the two
containment-adjacent defects (the assertion that cannot fail; the drifted duplicate containment
implementation). A third finding sits beside them:

3. **A one-instance guard is a silent no-op for the third time in project history.** The pattern
   `pgrep -f "[s]hadps4 -p "` guards against a second emulator in 5 files, but
   `run-contained-emulator.sh:115` launches with `-g "$GAME"` only (no `-p`), so an emulator
   started by that script matches NO guard anywhere - not its own (line 42), not
   `run-signon-experiment.sh:49`, not `emu-display.sh:71` (which would then edit config.json
   while shadPS4 is alive and lose the edit), not `autodrive.sh:78`'s liveness loop. Same failure
   class as the `pgrep -x shadps4` no-op both scripts' comments memorialize. Exposure is low
   today only because nothing calls `run-contained-emulator.sh`.

### 8.1 Per-script table

| script | what it does / callers | findings | verdict |
|---|---|---|---|
| `run-boot.sh` (425 ln) | Pins the whole known-good environment then delegates to `run-signon-experiment.sh`; the canonical launcher, cited by CLAUDE.md, the boot-run skill and 6+ crates' doc-comments. | (a) line 415 press loop runs UNCONDITIONALLY, so with `OPERATOR_PAD=1` and a pad FIFO present it injects 3 phantom X presses at t=60-69s while the operator drives - the two-input hazard its own line 291 comment forbids (workaround: `PRESS=0`); (b) line 425 still calls `gate-check.py`, deprecated by the verified `d1 gate` (section 3b), and the `\|\| true` there would silently swallow the script being archived; (c) line 309 `virtual-pad.py --print-ignore ... \|\| true` silently degrades to ignoring NO physical pads if the helper fails - a PAD LAW violation path. | **KEEP AS SHELL + FIX IN PLACE.** The file IS the pinned-environment documentation; its long-run direction is shrinking as pins move into d1-responder defaults. |
| `run-signon-experiment.sh` (362 ln) | The real containment + serving run: namespace, hosts redirect, TLS cert, shadnet + d1-responder + optional activity host, emulator. Called by `run-boot.sh` and directly by humans varying one thing. | `set -euo pipefail`; kill-then-start race handled properly. (a) preflight checks EMU_BIN/SERVER_BIN/GAME but NOT `d1-responder` or `d1-activity-host` - a missing responder yields a client that boots against nothing, a wasted run that reads as a protocol regression; (b) no inner EXIT trap: `kill %1 %2 %3` (line 358) runs only on clean fallthrough, so Ctrl-C leaks shadnet-server + d1-responder into the dead namespace - the exact leak CLAUDE.md tells every session to sweep; (c) the 31313 wait loop (line 269) falls through SILENTLY after 5s; (d) line 360 `rc=$?` is unreachable for nonzero under `set -e`, so RUN-INFO.txt records exit status only for rc=0 runs; (e) the `DRIVE=1` block (275-295) is a retired xdotool approach its own line 300 comment says produced zero SignOn requests - dead mode inside the canonical path; (f) hardcoded `/run/user/1001` (uid-specific). | **KEEP AS SHELL + FIX IN PLACE** (a, b, c cheap; e is a deletion candidate). Namespaces, bind mounts, exec chains, job control: legitimately shell's job. |
| `run-contained-emulator.sh` (121 ln) | Minimal contained run (no redirect, no responder). Callers: NONE executable - prose only, despite CLAUDE.md:208. | (a) guard blind to its own `-g`-only launches (finding 3); (b) usage promises `[-- extra shadps4 args...]` but line 103 hands `"$@"` to `shadnet-server`, never to the emulator; (c) unreachable `rc=$?` (line 119); (d) silent wait-loop fallthrough (108-111). The drifted containment duplicate (banner, finding 2). | **CONSOLIDATE** (coordinator decision): thin wrapper over `run-signon-experiment.sh`, or archive with a pointer + fix CLAUDE.md:208 in the same change. Do not leave two containment implementations. |
| `rig-mode.sh` (78 ln) | Switches console topology live/contained/status; THE CFW containment. Humans with sudo; cited by CLAUDE.md and the CFW runbook. | Enforcement sound; false-green guards good (drop-rule count not table existence, `readlink -e` first). (a) exit code always 0 - the assertion cannot fail (banner, finding 1); (b) the mode arms never verify their own outcome by read-back before exiting; (c) line 65 delegates to `/usr/local/sbin/d1-capture-offloads.sh`, an off-tree unversioned dependency whose failure is SILENT (no `set -e`, exit status ignored) - a fresh host or a renamed helper degrades containment setup without a word. | **KEEP AS SHELL + FIX IN PLACE**, but the fix touches the containment path: coordinator's change, with the named verification gate in 8.2 rec 1. |
| `capture.sh` (65 ln) | Retail-console capture: one dumpcap per bridge member + br0, environment manifest, drop counts. | Preflight gate enforced (aborts on failure); the `sg wireshark` re-exec is correct. Defect: drop counts are PRINTED but not enforced - the script exits 0 with drops, relying on a human reading the banner its own line 4 shouts about; it could exit nonzero when any count is nonzero. | **KEEP AS SHELL**; one small fix (exit nonzero on drops). |
| `preflight.sh` (61 ln) | Asserts the capture rig is sane; run before every session. | The model citizen: accumulates `fail` and `exit $fail` - exactly the propagation `rig-mode.sh status` lacks. One oddity: line 16 has a stray `cat .../master/uplink` before the `readlink` (dead code, errors suppressed, harmless). | **KEEP.** |
| `emu-display.sh` (101 ln) | Sets shadPS4 render/window resolution safely; refuses while the emulator runs (config.json is rewritten on exit) and refuses when per-game configs would override the edit. | Guards correct and the paid-for traps are memorialized in place. Exposure: its running-emulator guard is the `-p `-pattern (line 71), blind to a `-g`-only launch (finding 3 family). | **KEEP.** |
| `autodrive.sh` (118 ln) | Adaptive no-operator driver to `character:signin`; stops before character select unless `--through-select`. | Design is right (reads the client's own substage narration, repertoire instead of a fixed script, stop-at-signin safety). (a) liveness loop line 78 is the `-p `-pattern (finding 3 family); (b) header comment cites `autopress.sh`, which section 1 archives - update the comment when that archival executes (already flagged in the section 1 row). | **KEEP.** |
| `zeek-control-capture.sh` (156 ln) | Manufactures the positive-control pcap for `zeek-logs.py` inside a sealed user+net namespace (loopback only, no sudo, nothing leaves). | Exemplary control discipline: TLS 1.2 deliberate (1.3 would silently empty x509.log), refuses an empty or sub-2000-byte pcap at exit 2, the first-draft's consumed-accept trap documented in place. No defects found. | **KEEP.** |
| `golden-diff-d1guest-vs-d1live.sh` (115 ln) | NEW this round (section 7.1's executable golden diff). Four configurations, exit-code comparison, single timestamp normalisation, decoy discipline documented. | Currently UNTRACKED. If it does not land with the port, the golden diff decays into an assertion nobody can re-run. | **KEEP + TRACK** (coordinator: include in the port's commit). |

### 8.2 Ranked recommendations

1. ✅ **DONE 2026-08-11: `rig-mode.sh status` is a real assertion.** Exit 0 CONTAINED / 1 LIVE /
   2 INDETERMINATE, and the `contained`/`live` arms now assert they ARRIVED before exiting 0
   (`contained` prints "Do NOT power on a CFW console until this says CONTAINED" and exits
   nonzero if it did not get there). Two things the fix had to get right beyond the exit codes:
   * **"cannot read" is not "absent".** Without root `nft` fails, and reporting that as a missing
     guard would be a false RED that trains people to ignore this output. The three states are now
     distinct: `yes`, `no (guard table absent)`, and `unknown (cannot read nftables without root)`.
     Only `yes` can produce CONTAINED.
   * **The verdict is a pure function with its own selftest.** `rig-mode.sh selftest` exercises
     seven rows, three of them inversions (guard rules missing, guard unreadable, interface
     unconfigured - each must NOT pass), and needs neither root nor a rig.

   **Verification status, stated honestly.** Proven: the selftest can FAIL (weakening the verdict
   so `guard` is not required flips exactly the two inverted rows to FAIL, exit 2); non-root
   `status` on the real rig exits **2** with the reason; and `sudo rig-mode.sh status` on the real
   rig reports **CONTAINED, exit 0**. `[U]` still unverified from this seat, and only the operator
   should close it: the LIVE verdict (exit 1) and a deliberately-broken-guard state. Both require
   changing the rig topology, and deleting the guard table would momentarily remove containment
   from a console whose firmware update past 13.00 is irreversible - so that experiment is not
   worth running merely to test an exit code. Check it opportunistically the next time the rig
   goes to `live` for a Lane B capture.
2. **One containment implementation.** Declare `run-signon-experiment.sh` the canonical
   containment path, reduce `run-contained-emulator.sh` to a thin delegating wrapper or archive
   it with a pointer, and correct CLAUDE.md:208 in the same change.
3. **Close the `-g`-only guard blindness** (finding 3): launch with `-p` everywhere, or widen the
   five guard patterns; do it before anything starts calling `run-contained-emulator.sh` again.
4. **`run-signon-experiment.sh` hardening:** preflight `d1-responder`/`d1-activity-host`
   binaries; add an EXIT trap so Ctrl-C does not leak servers into the dead namespace; make the
   31313 wait loop fail loudly; fix the unreachable `rc=$?`; delete the retired `DRIVE=1` block;
   un-hardcode `/run/user/1001`.
5. **`run-boot.sh`:** guard the line-415 press loop on `OPERATOR_PAD`; repoint line 425 from
   `gate-check.py` to `d1 gate` (this also unblocks the gate-check archival row in section 9);
   make a failed `--print-ignore` fatal rather than silently ignoring no pads.
6. **`capture.sh`:** exit nonzero when any drop count is nonzero.
7. **Track `golden-diff-d1guest-vs-d1live.sh`.**

---

## 9. Next archival manifest (round 2) - PREPARED, NOT EXECUTED

Same shape and same rules as section 1: nothing here has been deleted or moved; the coordinator
archives to `historicaldocs/python-capture/` in one reviewed batch, after the re-points, and only
after the blocking rows clear.

| file | superseded by | evidence and re-points |
|---|---|---|
| `capture-rig/message-catalogue.py` | `d1-eboot catalogue` | Golden diff executed and pinned as a fixture `#[test]` (`d1-eboot/tests/fixtures/message-catalogue-full.txt`), verify CONFIRMED (7.2). Nothing imports or executes it. Re-point at archive time: `design/eboot-map.md:47`, `:60`; `design/queuez-replication.md:348`, `:395`, `:1369`; `design/observability-and-ui.md:909`, `:966`. |
| `capture-rig/bap-message-table.py` | `d1-bap message-table [--tsv]` | TSV byte-identical, executed twice (port lane + doc-writer, 7.3); the derivation now lives beside the consts it feeds and is drift-checked by `d1-bap controls`. Section 2's regeneration row already repointed. Nothing imports it. |
| `capture-rig/bap-message-idspace.py` | `d1-bap idspace` / `d1-bap policies` | TSV byte-identical by `cmp` (7.3); the three-id-space distinction it existed to enforce is carried in `derive.rs`'s doc-comment and its controls (definition rows 59/76 pinned). Nothing imports it. |
| `capture-rig/gate-check.py` | `d1 gate` (section 3b) | Golden diff over 59 real run dirs, byte-identical. ⛔ **BLOCKED until `run-boot.sh:425` is repointed at `d1 gate`** (8.2 rec 5) - archiving first would make the canonical launcher's `\|\| true` swallow the loss silently. Also re-point the `status/harness.md` instrument row at archive time. |
| `capture-rig/d1guest.py` | read side: `d1-live read\|obj\|tag\|descriptors\|syms\|live\|base` (7.1) | ⛔ **NOT ARCHIVABLE.** `poke` is deliberately unported and load-bearing; `capture-rig/orbit-view.py --table` still calls the script; and the `[U] live-parity` gate is open. Revisit after the first L3 boot settles parity and the poke port lands as its own reviewed change. |

Round 3's rows (R3.5) stand except one: the four oodle-lane files remain archivable (one still
needs its banner), and `decode-account-trace.py` UNBLOCKED 2026-08-11 - `d1-run/src/gate.rs` was
repointed at the Rust port, so it joins the archivable set (same reviewed batch as
`harvest-crash.py`, whose port `d1 crash` landed the same day).
