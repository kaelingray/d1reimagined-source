# The client's own diagnostic strings - a catalogue, keyed for lookup

**Every diagnostic / log / status string Destiny 1 (CUSA00219 patch 01.29) can print, what
subsystem owns it, which function references it, what data that site has in hand - and ⭐ whether we
have ever actually seen it fire.** Produced by
`capture-rig/diagnostic-catalogue.py`, which re-derives every number below from
`dumps/patch-01.29-CUSA00219/eboot.bin` plus every `runs/*/emulator.log`, and **exits non-zero
if any of its 21 positive controls fails**.

Full per-string rows - all 2155 of them, every column - are in
**`capture-rig/diagnostic-catalogue.tsv`**. This document is the index, the caveats, and the
tables worth reading with your eyes.

Reach tags: the static half (address, subsystem, referencing function, arity, gate) is
**`static-client-inference`**; the SEEN half is **`observed-live`** over 248 runs of
our own client. `vaddr = file_offset - 0x4000` for the text mapping (⚠ per-segment - ATLAS §4.1).

> ## Why this document exists
>
> The client crashed in a 4-slot table lookup and three sessions theorised about a "missing
> fireteam identity" for hours. The function that fills that table references a string literal
> reading **`voice_globals`** - it is the *voice chat* subsystem, and shadPS4 stubs `sceVoice`
> entirely. One string reference, never looked up, because **there was nowhere to look it up**.
>
> Worse, twice in one night a theory was built on a diagnostic string that had **never actually
> fired**. There was no way to ask "have we ever seen this?" without hand-grepping every run.
> That question is now a column.

---

## 0. The answer in one paragraph

**2155 diagnostic strings** across **117 subsystems**, of which **327
(15.2%) have ever appeared in a run** - and that is not a measure of how far
the client gets, because 325 of them are not wired to the log sink at all and
81 more are *mechanically incapable* of printing at any threshold this project
has ever run. Netting those out, **1422 strings are ones whose absence is real
evidence that their code did not execute** - including every `networking:investment:peer:` line, which
is why "the peer account-SOID family never fired" is now a fact rather than a hunch. The single most
useful structural finding is that **317 strings reach the sink with
NO filter in front of them**: those print unconditionally the moment our sink patch is applied, so for
them SEEN/NOT-SEEN is a clean execution trace.

---

## 1. How to use it - three queries and a TSV

```bash
# ⭐ the query that cost three hours: which subsystem is this function in?
capture-rig/diagnostic-catalogue.py --addr 0xb45c78

# everything one subsystem can say (prefix match; `networking:investment` works too)
capture-rig/diagnostic-catalogue.py --subsystem networking:investment:peer

# substring search over every catalogued string AND every RTTI class name
capture-rig/diagnostic-catalogue.py --text voice

# regenerate this document and the TSV; exits 2 if a control fails
capture-rig/diagnostic-catalogue.py --write
```

Each query line is `SEEN? <string vaddr> fn <function> L<level> <text>`. `ref=none` in the
function column means no instruction in the image takes this string's address - see §5.

The TSV columns: `vaddr · tier · subsystem · root · fn · hot_range · sys · level · emits ·
unfiltered · evidence · arity · specs · n_refs · ref_sites · seen · n_runs · last_run · text`.
**`evidence` is the column to filter on** - see §2. `last_run` is ordered by log mtime, not by
directory name, because not every run directory is named with a timestamp.

⚠ This document changes every time a run is added, by design: SEEN is measured against
`runs/*/` as it stands. A diff that shows only `n_runs` moving is not a finding.

---

## 2. ⚠ What the SEEN column is worth - read this before citing it

> ⛔⛔ **A THIRD GATE EXISTS AND THIS DOCUMENT CANNOT SEE IT (2026-08-10).** The two gates below are
> both about a *logger that was called*. There is a third shape: **a site whose only formatting call
> is a `vsnprintf` into a stack buffer that is never read - no sink call at all.** Worked example:
> `can't spawn due to the current activity lifetime state ('%s')` in `FUN_00414730`
> (`FUN_010548E0` is `vsnprintf`; the buffer is referenced exactly twice - its declaration and that
> call). **0 hits across every run, and that is a structural false negative.** Positive control: the
> string IS in the image at file offset `0x1625926`.
>
> ⛔ **Such a site is not a `retail_log` site, so it is NOT IN THIS CATALOGUE AT ALL** - it cannot even
> be classified `none`. ⇒ **the `strong` evidence class below means "absence is evidence *for
> catalogued sites*", never "absence is evidence for this string".** Before treating a missing
> diagnostic as evidence, read the emitter and name the function that would have output it.
> ⇒ `historicaldocs/design/arrival-handoff.md` §6.4, `status/LESSONS.md`.


**A string absent from every log does NOT prove its code never ran.** `status/LESSONS.md`
and `historicaldocs/design/guest-diagnostics.md` record the trap: there are **two independent gates**, and only one
of them is opened by our patch unconditionally.

| gate | address | what it does | our runs |
|---|---|---|---|
| 1 · **filter** | `0xe4da0` | `threshold[system] <= level` - a HIGHER threshold is QUIETER | replaced by a uniform `T`, and **`T` has never been below 1** |
| 2 · **sink** | `0xe4ad0` | `memcpy` into a 1124-slot ring **that nothing ever drains** | replaced by `printf("%s\n")` in 242 of 248 runs |

Thresholds actually used, measured from each run's own log rather than from the patch file:

| T | runs | sites that pass |
|---:|---:|---:|
| 1 | 214 | 1470 |
| 2 | 15 | 1212 |
| 4 | 13 | 593 |
| - (no sink patch) | 6 | 0 |

Because the lowest threshold ever run is **T=1**, every site whose own level is
below 1 has been **structurally invisible for this project's entire history**. So each row
carries an *evidence class*, and it is the number to quote, not the raw SEEN bit:

| class | rows | what a NOT-SEEN verdict means |
|---|---:|---|
| `seen` | 327 | it fired; the TSV names the most recent run |
| **`strong`** | 1422 | it reaches the sink and passes the filter we ran ⇒ **absence means the code did not execute** |
| `weak-blind` | 81 | its level is below every threshold we have run ⇒ absence proves **nothing**; re-run at `T=0` to ask |
| `none` | 325 | not a `retail_log` site at all (status overlays, registry names, allocation tags) ⇒ absence proves **nothing** |

⭐ **The strongest sub-class**: 317 strings are `lea`d into an
emit call with **no `retail_log_check` in front of them at all**. Those are threshold-independent -
with the sink patched they print every single time the instruction is reached. Both halves of the
worked example are here: `world_controller:state_manager: Entering state '%s' for reason '%s'.`
(`0x1648203`) is unfiltered and appears in 239 runs,
while `networking:investment:peer: Account soids in error state: %s` (`0x162f4a1`) is **also
unfiltered** and appears in **zero**. The second fact is a measurement, not a silence.

**Coverage control, and it is two-sided:** 0 of the 13,850 distinct guest lines
observed across all runs failed to match a catalogued format string. A hole there would make every
NOT-SEEN verdict suspect; it is asserted as a control, and it is how two false negatives (formats
ending in a `%s` that rendered empty, so the log's trailing space was stripped away) were caught.

---

## 3. The mechanism, and how a string is attributed `[C]`

Every diagnostic site compiles to the same four-call shape, and the scanner reads exactly
this - no name matching, no heuristics:

```asm
mov  edi, <system>        ; 0..21
mov  esi, <level>         ; 0..5      (`xor esi,esi` for level 0 - a scan that misses this
call 0xe4da0              ;            loses all 80 level-0 sites and never notices)
test al, al ; je skip
lea  rdi, [FORMAT]  ; call 0xe4a40    ; register the format string -> site id
lea  rsi, [FORMAT]  ; call sprintf    ; format into a stack buffer
call 0xe4ad0                          ; EMIT - closes the group
```

The scanner walks each of the **35,834** prologue-anchored functions, decoding
**4,295,415** instructions, and closes an *emit group* at every `call 0xe4ad0`: every format
string `lea`d since the previous emit is what that call prints, and the filter that was armed is the
one it passed. That is what produces the `sys` / `level` / `unfiltered` columns.

Measured totals: **1632** filter calls, **1588** emit calls,
**1828** catalogued strings reaching an emit.
⚠ The two counts are *not* equal and that is the finding - see §2.

### 3.1 The scan is REFERENCE-FIRST, on purpose `[C]`

A C string pool has no framing, so "find the strings, then ask who references them" walks straight
into ATLAS §4.2b. Two live examples in this very image:

* `voice_globals` at `0x167d3e0` is preceded by the float bytes `9a 99 89 40`. A greedy printable-run
  scan calls it **`@voice_globals`**; a NUL-preceded scan **misses it entirely**. Only the `lea` at
  `0xb41047` knows where it starts.
* `design/eboot-map.md` §7's ui-substage table is published as starting at `0x164993f` with entry -10
  spelled **`@waiting_for_login`**. Same artefact: the `@` is `0x40`, the tail of a float. The names
  begin at `0x1649940`. The offsets in §7 are unaffected - only the first entry's spelling is.

So this tool enumerates *references* and reads the string at whatever address the code points at.
**Every start address here is correct by construction.** The 12 rows
flagged `ref=none` are the honest residue (§5).

An earlier revision of this tool tried to compensate by also testing `start+1`, and manufactured a
whole fake subsystem: 13 Demonware `Error: …` strings catalogued as **`rror: …`**, one byte past their
true start. That is ATLAS §4.2b created by the instrument rather than walked into, and it is why the
`start+1` path is gone rather than merely fixed.

### 3.2 Packed name tables - enumerated as tables, not as rows `[C]`

A *packed name table* is a run of NUL-separated names indexed by an integer. Nothing `lea`s the
individual entries, so reference-first sees the table and not its members, and listing each name as a
catalogue row would imply a call site that does not exist. The two known tables are recorded here
instead, and both are asserted as controls.

| anchor | entries | indexing | owner |
|---|---:|---|---|
| **`0x164993f`** (names begin `0x1649940`) | 47+ | `name_index = ENUM(n) + 10` | the `ui_stage` substage goal - `design/eboot-map.md` §7 |
| `0x1682e1d` | 12 | `[U]` | managed-session host migration reason codes - §5.1 |

Spot-check of the first, reproduced on every run of this tool: index 29 (`ENUM(19)`) reads
`bootflow bap signin`, which is `bootflow:bap_signin` naming itself -
§7's own control, re-derived here rather than copied (ATLAS §4.3).

⚠ And the correction §3.1 predicts: §7 publishes entry -10 as **`@waiting_for_login`**. The `@` is
`0x40`, the last byte of the float `4.3` that precedes the table. The name is `waiting_for_login` and
the table's first name starts at `0x1649940`. Every offset in §7 is unaffected.

---

## 4. `retail_log` system id → subsystem, derived rather than guessed `[C]`

`historicaldocs/design/guest-diagnostics.md` §3 built this table by correlating each gate site with the
`networking:` string it goes on to format - which necessarily mislabels every system whose strings are
**not** `networking:`. Deriving it from emit groups instead resolves all five `[U]` rows and corrects
two more. Counts are catalogued strings, not gate sites.

| sys | subsystem (this tool) | strings | `guest-diagnostics.md` §3 said |
|---:|---|---:|---|
| 0 | `networking:channel_manager` · `networking:channel` · `networking:stream_manager` | 160 | channel_manager / channel |
| 1 | `networking:demonware` · `networking:transport` · `high_water:demonware_heap` | 26 | demonware / transport |
| 2 | `world_controller:activity_manager` · `world_controller:state` · `networking:activity_client` | 41 | activity_client |
| 3 | `world_controller:activity_manager` · `world_controller:activity` · `world_controller` **⬅ resolves `[U]`** | 29 | `[U]` |
| 4 | `world_controller:slice_set_transition_manager` · `world_controller_slice_set_transition_manager` **⬅ resolves `[U]`** | 111 | `[U]` |
| 5 | `world_controller:state_manager` · `world_controller:state` · `world_controller` | 162 | online |
| 6 | `networking:logic` · `networking:online` · `world_controller:state` | 240 | logic / online / configuration / leaderboard |
| 7 | `networking:investment` · `(no prefix)` · `networking:log_notification` | 106 | investment |
| 8 | `networking:investment` · `world_controller:investment` · `world_controller:online` | 21 | investment |
| 9 | `networking:session` · `networking:logic` · `world_controller:session_manager` | 256 | session |
| 10 | `networking:session` · `networking:logic` · `world_controller:session_manager` | 48 | session / logic |
| 11 | `networking:stun` · `networking:messages` · `(no prefix)` | 69 | stun |
| 12 | `networking:messages` | 4 | messages |
| 13 | `networking:signon` · `networking:online` · `networking:online_consumable_manager` | 89 | signon / online |
| 14 | `networking:sku_manager` | 32 | sku_manager |
| 15 | `activities:spawn_volumes` · `activities:hard_wipes` · `activities:teleportation` **⬅ resolves `[U]`** | 3 | `[U]` |
| 16 | `networking:simulation` | 25 | simulation |
| 17 | `networking:team_picker` | 14 | team_picker |
| 18 | `ui:errors` **⬅ resolves `[U]`** | 1 | `[U]` |
| 19 | `networking:managed_session` | 108 | managed_session |
| 20 | `networking:platform_user` | 4 | platform_user |
| 21 | `perk_state` · `perk_state_stack` **⬅ resolves `[U]`** | 4 | `[U]` |

Corrections worth carrying: **sys 2 and sys 3 are `world_controller:activity_manager`**, not
`activity_client`; **sys 5 is the `world_controller` state machine**, not `online`; **sys 4 is
`world_controller:slice_set_transition_manager`** - 108 strings, the single largest subsystem in the
image, and it had no name at all before.

---

## 5. The subsystem index - the primary key

117 subsystems, rolled up to two levels. **`fn range`** is the span of functions that
reference this subsystem's strings, which is what makes an unknown function address answerable; use
`--addr` for the exact answer, because the spans overlap.

⚠ `[I]` **28 of these rows carry exactly one string and one path component** (`in_os_party_chat`,
`estimated`, `current`, …). Those are almost certainly *labels* in a status dump rather than
subsystems - the classifier cannot tell `label: %s` from `subsystem: message` and does not pretend
to. `--addr` marks them `(label?, 1 string)`; treat any single-string row here as unnamed.

| subsystem | strings | SEEN | string range | fn range | fns |
|---|---:|---:|---|---|---:|
| `networking:session` | 280 | 16 | `0x15d1e60`..`0x1688b52` | `0xb6a3e0`..`0xb961e0` | 95 |
| `networking:investment` | 132 | 20 | `0x1612a3c`..`0x167c545` | `0x261be0`..`0xb32810` | 60 |
| `networking:managed_session` | 118 | 38 | `0x1616e07`..`0x16876bc` | `0x2b5470`..`0xb92fc0` | 52 |
| `networking:logic` | 117 | 8 | `0x163f650`..`0x16812ac` | `0x771000`..`0xb72660` | 48 |
| `world_controller:slice_set_transition_manager` | 115 | 9 | `0x1640660`..`0x16440bf` | `0x7771d0`..`0x7812d0` | 31 |
| `world_controller:state` | 114 | 32 | `0x164c460`..`0x1650358` | `0x794350`..`0x7ad680` | 44 |
| `world_controller:state_manager` | 100 | 35 | `0x1647e1b`..`0x164c3d8` | `0x782010`..`0x793ae0` | 25 |
| `networking:channel_manager` | 84 | 0 | `0x1671453`..`0x1674979` | `0xaefeb0`..`0xafcb20` | 29 |
| `networking:online` | 83 | 25 | `0x162a93e`..`0x1633260` | `0x544f70`..`0x5a01f0` | 37 |
| `world_controller:activity_manager` | 71 | 23 | `0x16156fb`..`0x163e3db` | `0x2a43b0`..`0x7618e0` | 19 |
| `networking:stun` | 66 | 20 | `0x1619614`..`0x1632362` | `0x2df4a0`..`0x59c420` | 23 |
| `networking:channel` | 65 | 0 | `0x16705fd`..`0x1672f03` | `0xae9fd0`..`0xaf64d0` | 24 |
| `networking:server` | 59 | 19 | `0x167aaf3`..`0x167c1b0` | `0xb2b540`..`0xb31e50` | 33 |
| `networking:signon` | 53 | 17 | `0x1615802`..`0x16167fc` | `0x2a5fd0`..`0x2abcc0` | 16 |
| `networking:sku_manager` | 33 | 4 | `0x160cf53`..`0x160d89d` | `0x1db9c0`..`0x1ddf50` | 8 |
| `networking:activity_client` | 30 | 7 | `0x161392a`..`0x163070d` | `0x298470`..`0x594e70` | 19 |
| `world_controller:session_manager` | 27 | 1 | `0x163f76f`..`0x16404fe` | `0x7717a0`..`0x7764a0` | 11 |
| `world_controller:matchmaking_manager` | 26 | 8 | `0x163e960`..`0x163f2de` | `0x7674e0`..`0x769cb0` | 9 |
| `networking:simulation` | 26 | 0 | `0x1688d8d`..`0x168aad0` | `0xba0ca0`..`0xbbba30` | 11 |
| `networking:demonware` | 22 | 2 | `0x1603893`..`0x161b576` | `0x9e6c0`..`0x2efab0` | 10 |
| `crash` | 19 | 0 | `0x16013dc`..`0x1606d97` | `0x5b0e0`..`0xc3170` | 11 |
| `networking:leaderboard` | 18 | 1 | `0x1678ce0`..`0x1679330` | `0xb21180`..`0xb22100` | 5 |
| `networking:platform_user` | 17 | 8 | `0x1611f46`..`0x1612513` | `0x244980`..`0x2474a0` | 8 |
| `world_controller` | 17 | 5 | `0x163c1c7`..`0x1649416` | `0x7532d0`..`0x78b500` | 5 |
| `crs_test` | 16 | 0 | `0x1607185`..`0x16074b9` | `0xc5160`..`0xc5ca0` | 4 |
| `networking:team_picker` | 15 | 0 | `0x1621a48`..`0x1621dec` | `0x417380`..`0x419a90` | 5 |
| `networking:configuration` | 15 | 0 | `0x16757c0`..`0x1675eb5` | `0xb0bda0`..`0xb0bda0` | 1 |
| `networking:messages` | 14 | 0 | `0x167ccfa`..`0x16845a5` | `0xb39440`..`0xb84160` | 10 |
| `networking:composition` | 12 | 0 | `0x1675309`..`0x16756f0` | `0xb095d0`..`0xb0b460` | 2 |
| `world_controller:ui_stage` | 11 | 6 | `0x1648968`..`0x1648e46` | `0x786e20`..`0x787a80` | 5 |
| `networking:desirability` | 11 | 0 | `0x1675f00`..`0x1676297` | `0xb0cb00`..`0xb0d680` | 3 |
| `ai` | 8 | 0 | `0x160b697`..`0x1622326` | `0x1943c0`..`0x42d300` | 4 |
| `cortex` | 6 | 0 | `0x16054c3`..`0x160786a` | `0xb21c0`..`0xcae00` | 5 |
| `networking:transport` | 6 | 0 | `0x161aad4`..`0x161b4d7` | `0x2e63c0`..`0x2eec70` | 5 |
| `world_controller:orbit_map_loader` | 6 | 1 | `0x163bc7f`..`0x163be19` | `0x752380`..`0x752ae0` | 3 |
| `world_controller:retail_datamine` | 6 | 2 | `0x163f3b0`..`0x163f5c2` | `0x76ac80`..`0x76f920` | 3 |
| `networking:stream_manager` | 6 | 1 | `0x166f00d`..`0x167503f` | `0xae1c70`..`0xb05aa0` | 4 |
| `networking:connection` | 6 | 0 | `0x16749c5`..`0x1674bac` | `0xafd070`..`0xafe9a0` | 5 |
| `resourcerer:contexts` | 5 | 0 | `0x160817c`..`0x1608318` | `0xd6330`..`0xd79b0` | 2 |
| `networking:relay_channel_manager` | 5 | 0 | `0x162d3e0`..`0x162ecf7` | `0x565a00`..`0x56f230` | 3 |
| `world_controller:investment` | 5 | 0 | `0x162e8b0`..`0x162eaef` | `0x56d3f0`..`0x56d3f0` | 1 |
| `ui:errors` | 5 | 0 | `0x1649477`..`0x165ebd4` | `0x78b8c0`..`0x94f340` | 3 |
| `host_handoff` | 5 | 0 | `0x1682e1d`..`0x1682eb2` | - | 0 |
| `peer_reestablish` | 5 | 0 | `0x1682f7e`..`0x168306e` | - | 0 |
| `crash:automation` | 4 | 0 | `0x1603590`..`0x1606d64` | `0x9c3d0`..`0xc2920` | 3 |
| `platform:events` | 4 | 1 | `0x1613745`..`0x16137f5` | `0x294440`..`0x294960` | 2 |
| `halt` | 3 | 0 | `0x1605d51`..`0x165ea86` | `0xbe9d0`..`0x946d30` | 2 |
| `networking:bap` | 3 | 3 | `0x162ac79`..`0x162acfb` | `0x558c80`..`0x558c80` | 1 |
| `networking:online_consumable_manager` | 3 | 1 | `0x1632415`..`0x1632497` | `0x59ca40`..`0x59cc70` | 3 |
| `perk_state` | 3 | 0 | `0x1636dd8`..`0x1636e9b` | `0x68f450`..`0x6967f0` | 3 |
| `world_controller:online` | 3 | 2 | `0x163d421`..`0x1649597` | `0x75f710`..`0x78ac20` | 3 |
| `world_controller_slice_set_transition_manager` | 3 | 1 | `0x1642de4`..`0x1642f07` | `0x77f6b0`..`0x77f6b0` | 1 |
| `graphics:surfaces` | 3 | 0 | `0x16545d7`..`0x1654667` | `0x819ca0`..`0x10ae0e0` | 3 |
| `activities:team_picker` | 3 | 3 | `0x167037a`..`0x167042a` | `0xae94d0`..`0xae94d0` | 1 |
| `networking:sessionid` | 2 | 1 | `0x1601259`..`0x16012c0` | `0x593c0`..`0x593c0` | 1 |
| `build:string` | 2 | 1 | `0x1601275`..`0x1601300` | `0x593c0`..`0x593c0` | 1 |
| `resourcerer` | 2 | 0 | `0x16078df`..`0x16081c5` | `0xcdc00`..`0xd7280` | 2 |
| `player_position_x` | 2 | 0 | `0x162a1b0`..`0x162a1f2` | `0x52bb10`..`0x52bb10` | 1 |
| `player_position_y` | 2 | 0 | `0x162a1c6`..`0x162a20d` | `0x52bb10`..`0x52bb10` | 1 |
| `player_position_z` | 2 | 0 | `0x162a1dc`..`0x162a228` | `0x52bb10`..`0x52bb10` | 1 |
| `world_controller:activity` | 2 | 0 | `0x163e293`..`0x163e344` | `0x758e40`..`0x758e40` | 1 |
| `world_controller:face_to_face_join` | 2 | 2 | `0x164912f`..`0x1649170` | `0x78ad80`..`0x78ae40` | 2 |
| `prepare` | 2 | 0 | `0x1651de1`..`0x1651e08` | `0x7e9fc0`..`0x7e9fc0` | 1 |
| `graphics:command_buffer` | 2 | 0 | `0x165200c`..`0x1652048` | `0x7ed310`..`0x7ed7e0` | 2 |
| `host_reestablish` | 2 | 0 | `0x1682ef5`..`0x1682f42` | - | 0 |
| `ioq` | 2 | 0 | `0x16b7321`..`0x16b7353` | `0x1286260`..`0x1286260` | 1 |
| `application_state` | 1 | 0 | `0x16015fa`..`0x16015fa` | `0x67090`..`0x67090` | 1 |
| `c_fragment_ring_buffer` | 1 | 0 | `0x1601840`..`0x1601840` | `0x75dd0`..`0x75dd0` | 1 |
| `files` | 1 | 0 | `0x16019bb`..`0x16019bb` | `0x7e710`..`0x7e710` | 1 |
| `dlc:map` | 1 | 0 | `0x1605ae4`..`0x1605ae4` | `0xbca20`..`0xbca20` | 1 |
| `version` | 1 | 0 | `0x1605c08`..`0x1605c08` | `0xbe410`..`0xbe410` | 1 |
| `system:minidump` | 1 | 0 | `0x1606955`..`0x1606955` | `0xc0850`..`0xc0850` | 1 |
| `stack` | 1 | 0 | `0x1606a72`..`0x1606a72` | `0xc11f0`..`0xc11f0` | 1 |
| `session_id` | 1 | 0 | `0x1606a7b`..`0x1606a7b` | `0xc0f20`..`0xc0f20` | 1 |
| `crash_report_data` | 1 | 0 | `0x1606f92`..`0x1606f92` | `0xc4210`..`0xc4210` | 1 |
| `crs_test:game_folder` | 1 | 0 | `0x16074e3`..`0x16074e3` | `0xc5a00`..`0xc5a00` | 1 |
| `crs_test:reports_folder` | 1 | 0 | `0x16074fc`..`0x16074fc` | `0xc5a00`..`0xc5a00` | 1 |
| `crs_test:dump_folder` | 1 | 0 | `0x1607518`..`0x1607518` | `0xc5a00`..`0xc5a00` | 1 |
| `resourcerer:system_memory_container` | 1 | 0 | `0x1607c80`..`0x1607c80` | `0xd3aa0`..`0xd3aa0` | 1 |
| `resourcerer:defrag` | 1 | 0 | `0x1607ce5`..`0x1607ce5` | `0xd3aa0`..`0xd3aa0` | 1 |
| `resourcerer:load` | 1 | 0 | `0x1607d79`..`0x1607d79` | `0xd3f40`..`0xd3f40` | 1 |
| `networking:retail_log` | 1 | 0 | `0x1608847`..`0x1608847` | `0xe4ba0`..`0xe4ba0` | 1 |
| `name` | 1 | 0 | `0x1608c0c`..`0x1608c0c` | `0xf9520`..`0xf9520` | 1 |
| `activities:client` | 1 | 0 | `0x1608f00`..`0x1608f00` | `0x103320`..`0x103320` | 1 |
| `activities:spawn_volumes` | 1 | 0 | `0x160a9b2`..`0x160a9b2` | `0x175e30`..`0x175e30` | 1 |
| `activities:hard_wipes` | 1 | 0 | `0x1614c7b`..`0x1614c7b` | `0x2a1f60`..`0x2a1f60` | 1 |
| `activities:teleportation` | 1 | 0 | `0x1614d33`..`0x1614d33` | `0x2a20c0`..`0x2a20c0` | 1 |
| `networking:friends` | 1 | 1 | `0x1616bbf`..`0x1616bbf` | `0x2b2670`..`0x2b2670` | 1 |
| `high_water:demonware_heap` | 1 | 1 | `0x161a9ca`..`0x161a9ca` | `0x2e3360`..`0x2e3360` | 1 |
| `dir` | 1 | 0 | `0x161aa0e`..`0x161aa0e` | `0x2e3960`..`0x2e3960` | 1 |
| `networking:link` | 1 | 0 | `0x161ab94`..`0x161ab94` | `0x2e78b0`..`0x2e78b0` | 1 |
| `sobjects:lifetime` | 1 | 0 | `0x161ca5e`..`0x161ca5e` | `0x32feb0`..`0x32feb0` | 1 |
| `networking:log_notification` | 1 | 0 | `0x162d08a`..`0x162d08a` | `0x564820`..`0x564820` | 1 |
| `perk_state_stack` | 1 | 0 | `0x1636f50`..`0x1636f50` | `0x697ff0`..`0x697ff0` | 1 |
| `world_controller:transition_error` | 1 | 1 | `0x163c145`..`0x163c145` | `0x754a60`..`0x754a60` | 1 |
| `state` | 1 | 0 | `0x1648248`..`0x1648248` | `0x782300`..`0x782300` | 1 |
| `world_controller:ui_stage_manager` | 1 | 0 | `0x1648ebd`..`0x1648ebd` | `0x787e80`..`0x787e80` | 1 |
| `audio:stream_manager` | 1 | 0 | `0x1650c3d`..`0x1650c3d` | `0x7bf400`..`0x7bf400` | 1 |
| `audio` | 1 | 0 | `0x1650d23`..`0x1650d23` | `0x7c1fd0`..`0x7c1fd0` | 1 |
| `extract` | 1 | 0 | `0x1651dab`..`0x1651dab` | `0x7e9fc0`..`0x7e9fc0` | 1 |
| `graphics` | 1 | 0 | `0x1652106`..`0x1652106` | `0x7edc30`..`0x7edc30` | 1 |
| `visibility` | 1 | 0 | `0x16553ed`..`0x16553ed` | `0x837410`..`0x837410` | 1 |
| `ui:error_manager` | 1 | 0 | `0x165eb8e`..`0x165eb8e` | `0x94e9d0`..`0x94e9d0` | 1 |
| `networking:update` | 1 | 1 | `0x166ef9f`..`0x166ef9f` | `0xae1c70`..`0xae1c70` | 1 |
| `activity` | 1 | 0 | `0x166f064`..`0x166f064` | `0xae2100`..`0xae2100` | 1 |
| `mem` | 1 | 0 | `0x166f071`..`0x166f071` | `0xae2100`..`0xae2100` | 1 |
| `link` | 1 | 0 | `0x166f0af`..`0x166f0af` | `0xae2100`..`0xae2100` | 1 |
| `estimated` | 1 | 0 | `0x166f439`..`0x166f439` | `0xae2100`..`0xae2100` | 1 |
| `current` | 1 | 0 | `0x166f447`..`0x166f447` | `0xae2100`..`0xae2100` | 1 |
| `config` | 1 | 0 | `0x166ff16`..`0x166ff16` | `0xae70c0`..`0xae70c0` | 1 |
| `max_players` | 1 | 0 | `0x166ff8f`..`0x166ff8f` | `0xae2100`..`0xae2100` | 1 |
| `live_players` | 1 | 0 | `0x166fffe`..`0x166fffe` | `0xae2100`..`0xae2100` | 1 |
| `party_player_counts_can_join` | 1 | 0 | `0x16700b0`..`0x16700b0` | `0xae2100`..`0xae2100` | 1 |
| `machine` | 1 | 0 | `0x167872b`..`0x167872b` | `0xb17630`..`0xb17630` | 1 |
| `in_os_party_chat` | 1 | 0 | `0x167d408`..`0x167d408` | `0xb44460`..`0xb44460` | 1 |
| `muted_players` | 1 | 0 | `0x167d740`..`0x167d740` | `0xb4b960`..`0xb4b960` | 1 |
| `tlsf_create` | 1 | 0 | `0x16a2498`..`0x16a2498` | `0xe93ef0`..`0xe93ef0` | 1 |

### 5.1 The 12 strings nothing references `[C]`

Reference-first cannot see a string reached by *indexing a packed table*, and this is what that
residue looks like - one table, one subsystem family:


| vaddr | text |
|---|---|
| `0x1682e1d` | `host_handoff: in stable state` |
| `0x1682e3b` | `host_handoff: in stable state (timed out)` |
| `0x1682e65` | `host_handoff: picked handoff peer` |
| `0x1682e87` | `host_handoff: sent host_transition message` |
| `0x1682eb2` | `host_handoff: timed out waiting for host_reestablish from new host` |
| `0x1682ef5` | `host_reestablish: finished waiting for old migration, started host migration` |
| `0x1682f42` | `host_reestablish: managed host migration complete (as host)` |
| `0x1682f7e` | `peer_reestablish: starting managed session host migration` |
| `0x1682fb8` | `peer_reestablish: finished waiting for old migration, started host migration as client` |
| `0x168300f` | `peer_reestablish: managed host migration complete` |
| `0x1683041` | `peer_reestablish: membership update complete` |
| `0x168306e` | `peer_reestablish: opening channels complete` |

They are consecutive, they share the `host_handoff:` / `host_reestablish:` / `peer_reestablish:`
prefixes, and none carries a format specifier - i.e. a **reason-code name table** for managed-session
host migration, indexed the way `design/eboot-map.md` §7's ui-substage table is. `[I]` Not chased
further; noted so the next reader does not read `ref=none` as "unused".

---

## 6. The four ranges we are stuck in, in full `[C]`

These are the address ranges named in the current investigations. Every string referenced
from them is listed whether or not it carries a subsystem prefix - `voice_globals` is exactly the case
that motivated it.

`L` is the site's own message level (`-` = no filter in front of it). `E` is the §2 evidence class:
**SEEN** it fired · `strong` it would have printed, so absence means the code did not run ·
`blind` its level is below every threshold we have run · `-` not a `retail_log` site at all, so
absence proves nothing.


### 6.1 `voice/session` - `0xb41000`..`0xb48000` - 29 strings, 0 SEEN

| vaddr | fn | L | E | arity | text |
|---|---|---:|---|---:|---|
| `0x167d3e0` | `0xb41000` | - | - | 0 | `voice_globals` |
| `0x167d3ee` | `0xb44460` | - | - | 1 | `current voice session: %s` |
| `0x167d408` | `0xb44460` | - | - | 1 | `in_os_party_chat: %s` |
| `0x167d41d` | `0xb44460` | - | - | 1 | `current voice output setting: %s` |
| `0x167d43e` | `0xb44460` | - | - | 1 | `current voice c_network_session type: %s` |
| `0x167d467` | `0xb44460` | - | - | 0 | `current voice c_network_session type: NO VOICE SESSION` |
| `0x167d49e` | `0xb44460` | - | - | 1 | `local peer index: %i` |
| `0x167d4b3` | `0xb44460` | - | - | 1 | `voice channels: %i` |
| `0x167d4c6` | `0xb44460` | - | - | 1 | `voice lifecycle: %s` |
| `0x167d4da` | `0xb44460` | - | - | 0 | `CONTROLLERS: ` |
| `0x167d4e8` | `0xb44460` | - | - | 2 | `        controller:(%i) player:(%i)` |
| `0x167d50c` | `0xb44460` | - | - | 9 | `PLAYERS: %5s %16s %4s %6s %8s   %8s %4s %20s %17s` |
| `0x167d53e` | `0xb44460` | - | - | 0 | `INDEX` |
| `0x167d544` | `0xb44460` | - | - | 0 | `NAME` |
| `0x167d549` | `0xb44460` | - | - | 0 | `LOCAL?` |
| `0x167d550` | `0xb44460` | - | - | 0 | `TALKING?` |
| `0x167d559` | `0xb44460` | - | - | 0 | `FLAGS` |
| `0x167d55f` | `0xb44460` | - | - | 0 | `TEAM` |
| `0x167d564` | `0xb44460` | - | - | 0 | `PROC MODE` |
| `0x167d56e` | `0xb44460` | - | - | 0 | `LAST TALKED` |
| `0x167d57a` | `0xb44460` | - | - | 9 | `         %5d %16ls %4d %6s %8s 0x%08x %4d %20s %17lld` |
| `0x167d5b0` | `0xb44460` | - | - | 0 | `remote` |
| `0x167d5b7` | `0xb44460` | - | - | 0 | `talking` |
| `0x167d5bf` | `0xb44460` | - | - | 0 | `silent` |
| `0x167d5c6` | `0xb44460` | - | - | 2 | `PLAYERS: %5s %128s` |
| `0x167d5d9` | `0xb44460` | - | - | 0 | `VOICE SETTINGS` |
| `0x167d5e8` | `0xb44460` | - | - | 2 | `         %5d %128s` |
| `0x167d6b8` | `0xb46250` | - | - | 0 | `voice engine orbis internal` |
| `0x167d6f2` | `0xb463e0` | - | - | 0 | `BNG:VOICE_ENGINE` |

### 6.2 `bap` - `0xb2e000`..`0xb33000` - 100 strings, 7 SEEN

| vaddr | fn | L | E | arity | text |
|---|---|---:|---|---:|---|
| `0x167b223` | `0xb2e020` | - | strong | 3 | `networking:server:bap: [%s - %s] Failed to read header offset '%d'.` |
| `0x167b267` | `0xb2e020` | - | **SEEN** | 4 | `networking:server:bap: [%s - %s] update_receiving_header() failed to find recipient for header [sequence %d][type %s].` |
| `0x167b2de` | `0xb2e020` | - | strong | 5 | `networking:server:bap: [%s - %s] update_receiving_header() failed to call get_payload_memory_for_response() [sequence %d][type %s][size %d].` |
| `0x167b36b` | `0xb2e020` | - | strong | 5 | `networking:server:bap: [%s - %s] update_receiving_header() failed to call push get_payload_memory_for_response() [sequence %d][type %s][size %d].` |
| `0x167b544` | `0xb2e4c0` | - | strong | 5 | `networking:server:bap: [%s - %s] Failed to read payload [type %s][size %d][offset %d].` |
| `0x167af29` | `0xb2e660` | - | strong | 3 | `networking:server:bap: [%s - %s] Failed to read encrypted payload offset '%d'.` |
| `0x167af78` | `0xb2e660` | - | strong | 2 | `networking:server:bap: [%s - %s] Received an encrypted message with an invalid signature.` |
| `0x167afd2` | `0xb2e940` | - | strong | 5 | `networking:server:bap: [%s - %s] Failed to handle message and encountered a fatal failure (%s) [sequence %d][type %s].` |
| `0x167b049` | `0xb2e940` | - | **SEEN** | 5 | `networking:server:bap: [%s - %s] update_receive() failed to find recipient for header [sequence %d][type %s][status %s].` |
| `0x167b841` | `0xb2ed40` | - | **SEEN** | 4 | `networking:server:bap: [%s - %s] Failed to receive data by offset [size %d][offset %d].` |
| `0x167b0c2` | `0xb2ee60` | - | strong | 2 | `networking:server:bap: [%s - %s] Failed to decode BAP header.` |
| `0x167b100` | `0xb2ee60` | - | strong | 3 | `networking:server:bap: [%s - %s] Unknown BAP type '%s'.` |
| `0x167b138` | `0xb2f030` | - | strong | 5 | `networking:server:bap: [%s - %s] Failed to decode header [type %s][size %d][sequence %d].` |
| `0x167b192` | `0xb2f030` | - | strong | 3 | `networking:server:bap: [%s - %s] base header decode mismatch [type %s].` |
| `0x167b3fd` | `0xb2f270` | - | **SEEN** | 4 | `networking:server:bap: [%s - %s] Failed to handle response-message [sequence %d][id %d].` |
| `0x167b456` | `0xb2f270` | - | strong | 4 | `networking:server:bap: [%s - %s] Handled response-message, but the BAP connection has been closed [sequence %d][id %d].` |
| `0x167b4ce` | `0xb2f550` | - | strong | 4 | `networking:server:bap: [%s - %s] Handled push-message, but the BAP connection has been closed [sequence %d][type %s].` |
| `0x167b59b` | `0xb2f930` | - | strong | 7 | `networking:server:bap: %s Failed to pack '%s' message [source %d][type %s][sequence %d][header %d][payload %d].` |
| `0x167b60b` | `0xb2f930` | - | strong | 0 | `secure` |
| `0x167b612` | `0xb2f930` | - | strong | 0 | `unencrypted` |
| `0x167b61e` | `0xb2f930` | - | strong | 6 | `networking:server:bap: [%s - %s] Failed to send message [message %d][type %s][size %d][offset %d].` |
| `0x167b681` | `0xb2f930` | - | strong | 4 | `networking:server:bap [%s - %s] Failed to send handoff response [size %d][offset %d].` |
| `0x167b72b` | `0xb2ff90` | - | strong | 4 | `networking:server:bap: [%s - %s] Failed to send data by offset [size %d][offset %d].` |
| `0x167b780` | `0xb30100` | - | strong | 4 | `networking:server:bap: [%s - %s] Failed to write data '%d' bytes, endpoint closed [ip address %s].` |
| `0x167b7e3` | `0xb30100` | - | strong | 5 | `networking:server:bap: [%s - %s] Failed to write data '%d' bytes, error '%d' [ip address %s].` |
| `0x167b899` | `0xb303d0` | - | **SEEN** | 3 | `networking:server:bap: [%s - %s] Failed to read data, endpoint closed [ip address %s].` |
| `0x167b8f0` | `0xb303d0` | - | strong | 4 | `networking:server:bap: [%s - %s] Failed to read data, unknown error '%d' [ip address %s].` |
| `0x167bda4` | `0xb306e0` | - | strong | 2 | `networking:server:bap: [%s - %s] Failed to generate channel startup request.` |
| `0x167bdf1` | `0xb306e0` | - | strong | 2 | `networking:server:bap: [%s - %s] Fatal error encountered in enter_state_channel_starting().` |
| `0x167be4d` | `0xb30840` | - | strong | 2 | `networking:server:bap: [%s - %s] Failed to generate login request.` |
| `0x167be90` | `0xb30840` | - | strong | 2 | `networking:server:bap: [%s - %s] Fatal error encountered in enter_state_authenticating().` |
| `0x167bc58` | `0xb309f0` | - | strong | 3 | `networking:server:bap: [%s - %s] Failed to bind to port '%d'.` |
| `0x167bc96` | `0xb309f0` | - | strong | 3 | `networking:server:bap: [%s - %s] failed to listen to port '%d'.` |
| `0x167bd2b` | `0xb309f0` | - | strong | 2 | `networking:server:bap: [%s - %s] Fatal error encountered in enter_state_listening().` |
| `0x167beea` | `0xb30d90` | - | strong | 3 | `networking:server:bap: [%s - %s] Ran out of memory setting the encryption key.  BAP Connection Heap Max used size [%u]` |
| `0x167bf61` | `0xb30d90` | - | **SEEN** | 2 | `networking:server:bap: [%s - %s] Received unreadable hello response.` |
| `0x167c025` | `0xb31110` | - | - | 0 | `bap connection manager globals` |
| `0x167c044` | `0xb31110` | - | - | 0 | `BAP Connection Heap` |
| `0x167c058` | `0xb313e0` | - | **SEEN** | 2 | `networking:server:bap: primary BAP server IP address: %s, port: %s` |
| `0x167c100` | `0xb31e50` | - | strong | 2 | `networking:server:bap:protocol_source: protocol response status failed %s, status: %s` |
| `0x167c156` | `0xb31e50` | - | strong | 1 | `networking:server:bap:protocol_source: failed to decode secure hello response, header: %s` |
| `0x167c1b0` | `0xb31e50` | - | strong | 1 | `networking:server:bap:protocol_source: failed to decode response type %s` |
| `0x167c20c` | `0xb31e50` | - | - | 0 | `b->c shello req` |
| `0x167c21c` | `0xb31e50` | - | - | 0 | `b->c shello rsp` |
| `0x167c22c` | `0xb31e50` | - | - | 0 | `c->b goodbye not` |
| `0x167c23d` | `0xb31e50` | - | - | 0 | `c->ah mgr req` |
| `0x167c24b` | `0xb31e50` | - | - | 0 | `c->ah mgr rsp` |
| `0x167c259` | `0xb31e50` | - | - | 0 | `c->ah not` |
| `0x167c263` | `0xb31e50` | - | - | 0 | `b->c not` |
| `0x167c26c` | `0xb31e50` | - | - | 0 | `b->c log not` |
| `0x167c279` | `0xb31e50` | - | - | 0 | `c->b start req` |
| `0x167c288` | `0xb31e50` | - | - | 0 | `c->b start rsp` |
| `0x167c297` | `0xb31e50` | - | - | 0 | `c->b skill req` |
| `0x167c2a6` | `0xb31e50` | - | - | 0 | `c->b skill rsp` |
| `0x167c2b5` | `0xb31e50` | - | - | 0 | `c->b mm req` |
| `0x167c2c1` | `0xb31e50` | - | - | 0 | `c->b mm rsp` |
| `0x167c2cd` | `0xb31e50` | - | - | 0 | `c->b nat req` |
| `0x167c2da` | `0xb31e50` | - | - | 0 | `b->c nat not` |
| `0x167c2e7` | `0xb31e50` | - | - | 0 | `c->b user msg req` |
| `0x167c2f9` | `0xb31e50` | - | - | 0 | `b->c user msg rsp` |
| `0x167c30b` | `0xb31e50` | - | - | 0 | `b->c shutdown not` |
| `0x167c31d` | `0xb31e50` | - | - | 0 | `c->b upo req` |
| `0x167c32a` | `0xb31e50` | - | - | 0 | `c->b upo rsp` |
| `0x167c337` | `0xb31e50` | - | - | 0 | `c->ws req` |
| `0x167c341` | `0xb31e50` | - | - | 0 | `c->ws rsp` |
| `0x167c34b` | `0xb31e50` | - | - | 0 | `c->b sub req` |
| `0x167c358` | `0xb31e50` | - | - | 0 | `c->b sub rsp` |
| `0x167c365` | `0xb31e50` | - | - | 0 | `c->b unsubscribe_request` |
| `0x167c37e` | `0xb31e50` | - | - | 0 | `c->b_unsubscribe_response` |
| `0x167c398` | `0xb31e50` | - | - | 0 | `c->b_client_config_request` |
| `0x167c3b3` | `0xb31e50` | - | - | 0 | `c->b_client_config_response` |
| `0x167c3cf` | `0xb31e50` | - | - | 0 | `c->b_ahp req` |
| `0x167c3dc` | `0xb31e50` | - | - | 0 | `c->b_ahp rsp` |
| `0x167c3e9` | `0xb31e50` | - | - | 0 | `c->b accot_id_trans req` |
| `0x167c401` | `0xb31e50` | - | - | 0 | `c->b acct_id_trans rsp` |
| `0x167c418` | `0xb31e50` | - | - | 0 | `c->b clan req` |
| `0x167c426` | `0xb31e50` | - | - | 0 | `c->b clan rsp` |
| `0x167c434` | `0xb31e50` | - | - | 0 | `ah->c not` |
| `0x167c43e` | `0xb31e50` | - | - | 0 | `ah->c log not` |
| `0x167c44c` | `0xb31e50` | - | - | 0 | `s->ws req` |
| `0x167c456` | `0xb31e50` | - | - | 0 | `s->ws rsp` |
| `0x167c460` | `0xb31e50` | - | - | 0 | `c->b qz_reg req` |
| `0x167c470` | `0xb31e50` | - | - | 0 | `c->b qz_reg rsp` |
| `0x167c480` | `0xb31e50` | - | - | 0 | `qz->c update not` |
| `0x167c491` | `0xb31e50` | - | - | 0 | `qz->c sub lost not` |
| `0x167c4a4` | `0xb31e50` | - | - | 0 | `b->ah mgr req` |
| `0x167c4b2` | `0xb31e50` | - | - | 0 | `b->ah mgr rsp` |
| `0x167c4c0` | `0xb31e50` | - | - | 0 | `b->ah mgr hoff` |
| `0x167c4cf` | `0xb31e50` | - | - | 0 | `b->ah not` |
| `0x167c4d9` | `0xb31e50` | - | - | 0 | `b->ah req` |
| `0x167c4e3` | `0xb31e50` | - | - | 0 | `b->ah rsp` |
| `0x167c4ed` | `0xb31e50` | - | - | 0 | `s->x not` |
| `0x167c4f6` | `0xb31e50` | - | - | 0 | `c->x not` |
| `0x167c4ff` | `0xb31e50` | - | - | 0 | `echo req` |
| `0x167c508` | `0xb31e50` | - | - | 0 | `echo rsp` |
| `0x167c511` | `0xb31e50` | - | - | 0 | `c->b irc not` |
| `0x167c51e` | `0xb31e50` | - | - | 0 | `b->c rrc not` |
| `0x167c52b` | `0xb31e50` | - | - | 0 | `c->b rrc req` |
| `0x167c538` | `0xb31e50` | - | - | 0 | `b->c rrc rsp` |
| `0x167c545` | `0xb32810` | 4 | strong | 1 | `networking:investment:bap:header_base: decoded invalid bap type %d` |

### 6.3 `investment/registry` - `0x570000`..`0x590000` - 36 strings, 2 SEEN

| vaddr | fn | L | E | arity | text |
|---|---|---:|---|---:|---|
| `0x162ed8d` | `0x573790` | - | - | 0 | `investment_activity_reward_tracker` |
| `0x162edc0` | `0x573c80` | - | - | 0 | `investment_activity_selection_manager` |
| `0x162ede6` | `0x574810` | - | - | 0 | `investment_game_manager` |
| `0x162edfe` | `0x574810` | - | - | 0 | `Investment Land Grab [Unused]` |
| `0x162ee1c` | `0x574810` | - | - | 0 | `Investment Output State Land Grab [Unused]` |
| `0x162ee47` | `0x574810` | - | - | 0 | `investment_game_manager_nondeterministic_state` |
| `0x162ef36` | `0x575d30` | - | - | 1 | `unload investment item cache for 0x%llx` |
| `0x162ef5e` | `0x576c00` | - | - | 1 | `fill investment item cache for 0x%llx` |
| `0x162ef84` | `0x578530` | - | - | 0 | `investment_item_definition_loader` |
| `0x162f301` | `0x578e70` | - | - | 0 | `investment_joinability_manager` |
| `0x162f418` | `0x57b2d0` | - | - | 0 | `investment_latency_record_manager` |
| `0x162f474` | `0x57b810` | - | - | 0 | `investment_location_manager_tracker` |
| `0x162f498` | `0x57bd80` | - | strong | 2 | `%s0x%llx` |
| `0x162f4a1` | `0x57bd80` | - | **SEEN** | 1 | `networking:investment:peer: Account soids in error state: %s` |
| `0x162f4de` | `0x57bd80` | - | strong | 1 | `networking:investment:peer: All peers do not have valid account SOIDs and the grace period timer of '%dms' has elapsed.` |
| `0x162f556` | `0x57c080` | - | **SEEN** | 2 | `networking:investment:peer: Failed peer-subscription validation [error %s][account_soid 0x%016llX].` |
| `0x162f5ba` | `0x57c730` | - | - | 0 | `investment_peer_subscription_manager` |
| `0x162f5df` | `0x57de70` | - | - | 0 | `investment_profile_manager_deterministic_state` |
| `0x162f60e` | `0x57de70` | - | - | 0 | `investment_profile_manager_nondeterministic_state` |
| `0x162f7f0` | `0x5817d0` | - | - | 0 | `investment_roster` |
| `0x1601bba` | `0x5827b0` | - | - | 0 | `invalid` |
| `0x162f820` | `0x5827b0` | - | - | 1 | `0x%016llX` |
| `0x162f82a` | `0x582a70` | - | - | 0 | `investment_roster_party_details` |
| `0x162f84a` | `0x584540` | - | - | 0 | `investment_ui_trigger_manager` |
| `0x162f944` | `0x586160` | - | - | 0 | `investment_user_manager` |
| `0x162f95c` | `0x586160` | - | - | 0 | `investment_presence_manager` |
| `0x162fac2` | `0x586e40` | - | - | 0 | `investment_user_message_manager` |
| `0x162fb74` | `0x588a40` | - | - | 0 | `investment_loot_object_manager` |
| `0x162fb9f` | `0x588d30` | - | - | 0 | `loot_manager_record_client_data_from_nonce` |
| `0x162fb93` | `0x588ed0` | - | - | 0 | `loot-bauble` |
| `0x162fbe0` | `0x589d50` | - | - | 0 | `investment_activity_proxy` |
| `0x162fc6b` | `0x58a200` | - | - | 0 | `investment_peer_subscription_proxy` |
| `0x162fd2c` | `0x58a850` | - | - | 0 | `investment_player_identifier_proxy` |
| `0x162fd4f` | `0x58c310` | - | - | 0 | `investment_social_presence_manager` |
| `0x162fd72` | `0x58d460` | - | - | 0 | `investment_translation_manager` |
| `0x162fd91` | `0x58d460` | - | - | 0 | `investment_translation_ui_cache` |

### 6.4 `world_controller states` - `0x790000`..`0x7a1000` - 109 strings, 7 SEEN

| vaddr | fn | L | E | arity | text |
|---|---|---:|---|---:|---|
| `0x164b150` | `0x7908c0` | 3 | strong | 1 | `world_controller:state_manager:private_repair: Activity instance status is already failing ('%s'), restarting repair.` |
| `0x164af9c` | `0x7909a0` | 2 | **SEEN** | 2 | `world_controller:state_manager:private_repair: Repair state '%s' took '%lld'ms.` |
| `0x164af01` | `0x790ae0` | 3 | strong | 0 | `world_controller:state_manager:private_repair: Abandoning repair attempt and stopping repair state machine because the world controller state is changing.` |
| `0x164b59f` | `0x790fc0` | - | strong | 0 | `world_controller:state_manager: No ethernet connection found.` |
| `0x164b5dd` | `0x790fc0` | - | strong | 0 | `world_controller:state_manager: An ethernet connection was found but we don't have network memory.` |
| `0x164b640` | `0x790fc0` | - | strong | 0 | `world_controller:state_manager: An ethernet connection was found but we don't have valid endpoints.` |
| `0x164b6a4` | `0x7911d0` | - | strong | 0 | `world_controller:state_manager: The transport method is not ready to send secure data.` |
| `0x164b6fb` | `0x7912b0` | - | strong | 0 | `world_controller:state_manager: Could not retrieve the primary player identifier.` |
| `0x164b74d` | `0x791390` | - | strong | 1 | `world_controller:state_manager: Primary player identifier '%s' was valid but the platform user was not signed-in to the online service.` |
| `0x164b4d0` | `0x7914e0` | - | **SEEN** | 2 | `world_controller:state_manager: Prerequisite '%s' failed for reason '%s', proceeding to cleanup state.` |
| `0x164b537` | `0x7914e0` | - | strong | 1 | `world_controller:state_manager: Before changing state, found a more accurate state-change reason: '%s'.` |
| `0x164bfda` | `0x791be0` | - | strong | 2 | `networking:logic:session: hierarchy validation is delayed because we don't have a recent enough posse membership (have %d < need %d)` |
| `0x164c05f` | `0x791be0` | - | strong | 2 | `networking:logic:session: Fireteam player '%d' (peer '%d') not in activity-session, kicking.` |
| `0x164b7d5` | `0x7923c0` | - | strong | 0 | `world_controller:state_manager: Prerequisite failed: deployment is draining now.` |
| `0x164b826` | `0x792540` | - | strong | 2 | `world_controller:state_manager: Peer mask '%s' is requesting fireteam-rescue, but we are ignoring them because we are in state '%s'.` |
| `0x164b8ab` | `0x792540` | - | strong | 1 | `world_controller:state_manager: Fireteam host sees peer mask '%s' requesting fireteam-rescue, and is telling the fireteam to help them out.` |
| `0x164b937` | `0x792540` | - | strong | 1 | `world_controller:state_manager: Fireteam client sees that the host has activated fireteam-rescue -- joining them in cleanup [peers in trouble: %s].` |
| `0x164b9cb` | `0x792540` | - | strong | 1 | `world_controller:state_manager: Fireteam client sees peer mask '%s' requesting fireteam-rescue, but the host has not yet activated rescue -- waiting.` |
| `0x164ba61` | `0x792a30` | - | strong | 0 | `world_controller:state_manager: The host appears to be in a different game, sploiking back out through orbit to sync up.` |
| `0x164bada` | `0x792af0` | 3 | strong | 0 | `world_controller:state_manager:rejoin: We've detected that our session nonces don't match, shutting down all matchmaking.` |
| `0x164bb54` | `0x792af0` | 4 | strong | 1 | `world_controller:state_manager:rejoin: Rejoining but are waiting to receive our initial activity membership before continuing. Posse state=%s` |
| `0x164bbe2` | `0x792af0` | 4 | strong | 1 | `world_controller:state_manager:rejoin: Rejoining but are waiting for ANY host to be reflected in the activity before continuing. Posse state=%s` |
| `0x164bc72` | `0x792af0` | - | strong | 1 | `world_controller:state_manager:rejoin: Rejoining but are waiting for the new host to be reflected in the activity before continuing (it's still us). Posse state=%s` |
| `0x164bd16` | `0x792af0` | 4 | strong | 1 | `world_controller:state_manager:rejoin Rejoining and are not the '%s' host, sploiking back out through orbit to sync up.` |
| `0x164bd8e` | `0x792ec0` | - | strong | 1 | `world_controller:state_manager: Private activity session in bad shape [status %s].` |
| `0x164bde1` | `0x792ec0` | - | strong | 1 | `world_controller:state_manager: Booted from private activity session [boot reason %s].` |
| `0x164be38` | `0x7930b0` | - | strong | 2 | `world_controller:state_manager: Handling ignored private activity session boot by starting repair [boot reason %s][ah boot reason %s].` |
| `0x164c1cc` | `0x793440` | - | strong | 2 | `world_controller:state_manager: In state '%s', fireteam is missing these required session parameters: '%s'` |
| `0x164c237` | `0x793440` | - | strong | 2 | `world_controller:state_manager: In state '%s', posse is missing these required session parameters: '%s'` |
| `0x164c29f` | `0x793440` | - | strong | 2 | `world_controller:state_manager: In state '%s', group is missing these required session parameters: '%s'` |
| `0x164c307` | `0x793440` | - | strong | 1 | `world_controller:state_manager: Not host of '%s', not attempting session parameter repair; waiting for host to fix.` |
| `0x164c37b` | `0x793440` | - | strong | 1 | `world_controller:state_manager: All '%s' session parameters have been successfully repaired.` |
| `0x164c3d8` | `0x793440` | - | strong | 1 | `world_controller:state_manager: Session '%s' parameter repair failed.` |
| `0x164bebf` | `0x793880` | - | strong | 1 | `world_controller:state_manager: Private activity instance status is '%s' and is an error but we can't find a state change reason for it. Using instance_ok_unknown.` |
| `0x164bf63` | `0x793880` | - | strong | 2 | `world_controller:state_manager: Private activity client is connected to activity host '%s' but posse host reports '%s'` |
| `0x164c0bc` | `0x793ae0` | - | strong | 4 | `world_controller:state_manager: Instance status not error, not resolving. [status %d][type %d][aspect %d], starting %s` |
| `0x164c133` | `0x793ae0` | - | **SEEN** | 6 | `world_controller:state_manager: resolved instance error to change reason [status %d][type %d][aspect %d][change reason %d][boot reason %d][starting %s].` |
| `0x164c7c3` | `0x794350` | 3 | strong | 0 | `world_controller:state:setup_initial_slice_set_loading: Unable to request system container memory.` |
| `0x164c826` | `0x794350` | 3 | **SEEN** | 0 | `world_controller:state:setup:initial_slice_set_loading: Waiting to allow system container memory usage...` |
| `0x164c4f4` | `0x794530` | 1 | strong | 0 | `world_controller:state:activity:initial_slice_set_loading: perform_prologue_intro_playback was skipped due to suppressing cinematis` |
| `0x164c890` | `0x794660` | 2 | **SEEN** | 0 | `world_controller:state:setup:initial_slice_set_loading: Waiting for managed-session-start for all peers.` |
| `0x164c460` | `0x794750` | 4 | strong | 1 | `world_controller:state:activity:initial_slice_set_loading: Error loading initial slice-set: %s` |
| `0x164c4bf` | `0x794750` | - | - | 1 | `Encountered an error (%d) loading initial slice set.` |
| `0x164c578` | `0x7948d0` | 1 | strong | 0 | `world_controller:state:activity:initial_slice_set_loading: perform_cinematic_exchange was skipped due to suppressing cinematis` |
| `0x164c5f7` | `0x794a60` | 1 | **SEEN** | 0 | `world_controller:state:activity:initial_slice_set_loading: perform_prologue_filler_entry_playback was skipped due to suppressing cinematis` |
| `0x164c682` | `0x794a60` | - | - | 1 | `world_controller:state:activity:initial_slice_set_loading: Timeout entering the prologue-filler (given %d ticks).` |
| `0x164c6f4` | `0x794d30` | - | - | 0 | `Holding` |
| `0x164c6fc` | `0x794d30` | - | - | 0 | `Enter 'world_controller_spaceflight_hold_loop 0' to continue` |
| `0x164c739` | `0x794e70` | 1 | strong | 0 | `world_controller:state:activity:initial_slice_set_loading: perform_prologue_filler_exit_playback was skipped due to suppressing cinematis` |
| `0x164c92f` | `0x7953e0` | 2 | strong | 0 | `world_controller:state:in_world: As HOST, skipping skill write, skill write not requested.` |
| `0x164c910` | `0x7955e0` | - | - | 0 | `Enter in-world` |
| `0x164c91f` | `0x795c50` | - | - | 0 | `Enter post game` |
| `0x164c98a` | `0x795d40` | 4 | strong | 0 | `world_controller:state:in_world: team based skill submission requested, can't get scorboard data, skill write being skipped` |
| `0x164ca06` | `0x795d40` | 4 | strong | 0 | `world_controller:state:in_world: team based skill submission requested, no winning team set` |
| `0x164ca90` | `0x796d40` | - | - | 0 | `failed to find activity video!` |
| `0x164caaf` | `0x796d40` | - | - | 0 | `invalid video activity index!` |
| `0x164cacd` | `0x797530` | 2 | strong | 0 | `world_controller:state:bootflow_account_signin_handler: account is underage - cannot request email address.` |
| `0x164cb39` | `0x797530` | 3 | **SEEN** | 0 | `world_controller:state:bootflow_account_signin_handler: Demonware lobby connection not desired, continuing without Demonware lobby connection.` |
| `0x164cbc8` | `0x7978b0` | 2 | strong | 1 | `world_controller:state:bootflow_account_signin_handler: initiated dw connection for %s` |
| `0x164cc1f` | `0x7978b0` | 4 | strong | 1 | `failed to initiate DW connection for player '%s'` |
| `0x164cc50` | `0x7978b0` | 4 | strong | 0 | `world_controller:state:bootflow_account_signin_handler: failed to initiate dw connection, we are toast!` |
| `0x164cdc6` | `0x7978b0` | 4 | strong | 0 | `world_controller:state:account_signin_handler: failed to start DW lobby for user` |
| `0x164ccb8` | `0x797b20` | 4 | strong | 2 | `world_controller:state:bootflow_account_signin_handler: Sign in handler took too long to establish connections! dw state=%d, local address resolved=%s` |
| `0x164cd4f` | `0x797b20` | - | - | 1 | `IP failed to resolve after '%lld' milliseconds` |
| `0x164cd7e` | `0x797b20` | - | - | 2 | `Demonware failed to connect: lobby state '%d' after '%lld' milliseconds` |
| `0x164ce17` | `0x798050` | - | - | 0 | `failed to unload bootstrap unpatchable` |
| `0x164ce3e` | `0x798050` | - | - | 0 | `failed to load bootstrap_patchable` |
| `0x164ce61` | `0x798050` | - | - | 0 | `failed to load investment globals` |
| `0x164ce83` | `0x798580` | - | - | 0 | `Primary outbound BAP connection failed during start_connection` |
| `0x164cec2` | `0x798580` | - | - | 1 | `BAP connection failed due to hitting timeout '%lld'ms` |
| `0x164cf35` | `0x798890` | - | - | 0 | `async_write_unpatchable_packages` |
| `0x1608b36` | `0x798930` | - | - | 0 | `ps4` |
| `0x164cf56` | `0x798930` | - | - | 0 | `unpatchable\\packages` |
| `0x164d1ec` | `0x798930` | - | - | 2 | `%s_%s.pkg` |
| `0x164d1f6` | `0x798930` | - | - | 2 | `unable to write unpatchable package {{'%s'}} to utility drive {{0x%08x}}` |
| `0x164cf6b` | `0x798d10` | - | - | 0 | `unable to create unpatchable package directory` |
| `0x164cf9a` | `0x798d10` | - | - | 0 | `unable to open async unpatchable package location` |
| `0x164cfcc` | `0x798d10` | - | - | 0 | `unabled to close async unpatchable package location` |
| `0x164d000` | `0x798d10` | - | - | 2 | `Unpatchable package registration failed with result %s (%d)` |
| `0x164d03c` | `0x798d10` | - | - | 0 | `failed to load bootstrap unpatchable` |
| `0x164d061` | `0x798d10` | - | - | 0 | `Unable to create directory` |
| `0x164d07c` | `0x798d10` | - | - | 0 | `Unable to open async location` |
| `0x164d09a` | `0x798d10` | - | - | 0 | `Unabled to close async location` |
| `0x164d23f` | `0x798d10` | - | - | 2 | `%s_%s` |
| `0x164d0ba` | `0x799550` | - | - | 0 | `Too many packaged font files` |
| `0x164d0d7` | `0x799550` | - | - | 0 | `Invalid file request` |
| `0x164d0ec` | `0x799550` | - | - | 0 | `Unexpected file size. Not aligned to block size` |
| `0x164d11c` | `0x799550` | - | - | 0 | `Failed to obtain file accessor` |
| `0x164d13b` | `0x799550` | - | - | 0 | `Not enough font staging space. Increase k_font_staging_buffer_size!` |
| `0x164d17f` | `0x799550` | - | - | 0 | `No embedded fonts found. Unable to write fonts` |
| `0x164d1ae` | `0x799aa0` | - | - | 1 | `Unable to write out font file, reason: {{%d}}` |
| `0x164d1dc` | `0x799aa0` | - | - | 0 | `Bad file handle` |
| `0x164d245` | `0x799dc0` | 3 | strong | 0 | `world_controller:state:bootflow_confirm_account_transfer: account transfer accepted` |
| `0x164d299` | `0x799dc0` | 3 | strong | 0 | `world_controller:state:bootflow_confirm_account_transfer: account transfer rejected` |
| `0x164d2ed` | `0x799fb0` | 4 | strong | 1 | `world_controller:state:bootflow_confirm_account_transfer: dialog [%s] missing from ui content, auto-rejecting` |
| `0x164d35b` | `0x79a380` | - | - | 0 | `Could not obtain content manifest url` |
| `0x164d381` | `0x79ad00` | - | - | 0 | `::DLC Install Complete` |
| `0x164d3b0` | `0x79b8a0` | 3 | strong | 0 | `world_controller:state:bootflow_enumerate_content_rights_handler: local entitlement enumeration failed` |
| `0x164d417` | `0x79c110` | - | - | 0 | `failed to load client startup globals` |
| `0x164d43d` | `0x79c110` | - | - | 0 | `required successful load of bootstrap activity` |
| `0x164d46c` | `0x79c710` | - | - | 0 | `Invalid SKU info.` |
| `0x164d47e` | `0x79c7d0` | - | - | 0 | `sku_manager must be initialized before wait_for_application_password.` |
| `0x164d4c4` | `0x79c7d0` | - | - | 0 | `Unable to show platform keyboard which is required for application password.` |
| `0x15c1080` | `0x79c9c0` | - | - | 0 | `fa0dd431-9c37-4f13-9fd3-df55340dfddd` |
| `0x164d536` | `0x79cd20` | - | - | 2 | `Patchable package registration failed with result %s (%d)` |
| `0x164d570` | `0x79cd20` | - | - | 0 | `successful package verification` |
| `0x164d5bc` | `0x79cd20` | - | - | 1 | `package id %d is out of range, can't register` |
| `0x164d617` | `0x79e4c0` | - | - | 1 | `failed to load [%s]` |
| `0x164d62b` | `0x79efc0` | - | - | 0 | `::sentinel file found, install failed: nuke and pave` |

#### ⭐ The `voice/session` range in one line

`0x167d3e0` `voice_globals` is `lea`d at `0xb41047` by **`0xb41000`**, which allocates
`0x6ba0` bytes and stores the pointer at BSS **`0x2815770`**. Everything else in that range is the
**voice status overlay** owned by `0xb44460` (`PLAYERS: %5s %16s …`, `TALKING?`, `LAST TALKED`) plus
`0xb46250` `voice engine orbis internal` and `0xb463e0` `BNG:VOICE_ENGINE`. **Not one of these
29 strings is a `retail_log` site**, so the
whole range is evidence class `none`: it could be running and we would see nothing either way. That
is the honest statement of what the logs can and cannot tell us about voice.

---

## 7. Globals and allocation tags each range owns `[I]`

A snake_case identifier string `lea`d by a function that also takes the address of a
data/BSS global. Two known cases anchor it as controls - `0xb41000 → voice_globals @0x2815770` and
`0xe4890 → retail_log @0x256eef0` - and both are asserted on every run. **This is the one heuristic
in the tool; a row here is a lead, not a fact.** Restricted below to the four ranges.

The `investment/registry` block is the most immediately useful thing in this document after §6: it is
a **registry-object name → constructing function** map, so `investment_roster` is `0x5817d0` and
`investment_peer_subscription_manager` is `0x57c730` without disassembling anything.

| range | fn | tag | global |
|---|---|---|---|
| `world_controller states` | `0x798890` | `async_write_unpatchable_packages` | `0x1a06c78` |
| `bap` | `0xb309f0` | `bap_connection` | `0x1a06c78` |
| `bap` | `0xb2e940` | `enum_out_of_range` | `0x28140c0` |
| `bap` | `0xb2f930` | `enum_out_of_range` | `0x28141a0` |
| `bap` | `0xb31e50` | `enum_out_of_range` | `0x28143e8` |
| `investment/registry` | `0x589d50` | `investment_activity_proxy` | `0x19cc988` |
| `investment/registry` | `0x573790` | `investment_activity_reward_tracker` | `0x1926738` |
| `investment/registry` | `0x573c80` | `investment_activity_selection_manager` | `0x26a33e0` |
| `investment/registry` | `0x574810` | `investment_game_manager` | `0x19269b8` |
| `investment/registry` | `0x574810` | `investment_game_manager_nondeterministic_state` | `0x1926948` |
| `investment/registry` | `0x578530` | `investment_item_definition_loader` | `0x1926a08` |
| `investment/registry` | `0x578e70` | `investment_joinability_manager` | `0x1927408` |
| `investment/registry` | `0x57b2d0` | `investment_latency_record_manager` | `0x1927608` |
| `investment/registry` | `0x57b810` | `investment_location_manager_tracker` | `0x1927698` |
| `investment/registry` | `0x588a40` | `investment_loot_object_manager` | `0x1928108` |
| `investment/registry` | `0x57c730` | `investment_peer_subscription_manager` | `0x1927728` |
| `investment/registry` | `0x58a200` | `investment_peer_subscription_proxy` | `0x19cc8e8` |
| `investment/registry` | `0x58a850` | `investment_player_identifier_proxy` | `0x1928398` |
| `investment/registry` | `0x586160` | `investment_presence_manager` | `0x1927df8` |
| `investment/registry` | `0x57de70` | `investment_profile_manager_deterministic_state` | `0x1927918` |
| `investment/registry` | `0x57de70` | `investment_profile_manager_nondeterministic_state` | `0x1927918` |
| `investment/registry` | `0x5817d0` | `investment_roster` | `0x1927a08` |
| `investment/registry` | `0x582a70` | `investment_roster_party_details` | `0x1927a98` |
| `investment/registry` | `0x58c310` | `investment_social_presence_manager` | `0x1928478` |
| `investment/registry` | `0x58d460` | `investment_translation_manager` | `0x1928518` |
| `investment/registry` | `0x58d460` | `investment_translation_ui_cache` | `0x1a06c78` |
| `investment/registry` | `0x584540` | `investment_ui_trigger_manager` | `0x1927c18` |
| `investment/registry` | `0x586160` | `investment_user_manager` | `0x1927e78` |
| `investment/registry` | `0x586e40` | `investment_user_message_manager` | `0x1927f08` |
| `investment/registry` | `0x588d30` | `loot_manager_record_client_data_from_nonce` | `0x2642840` |
| `voice/session` | `0xb41000` | `voice_globals` | `0x2815770` |

789 candidate pairs image-wide; `--json` emits all of them.

---

## 8. RTTI class names - subsystem names for free `[C]`

**2384 Itanium-mangled type-name strings** in the image, demangled where the shape is
simple (`26c_x` → `c_x`, `N3cui30c_yE` → `cui::c_y`). They are in the TSV's sibling JSON
(`--json`), not in the main table, because they are class names rather than printable diagnostics -
but they name subsystems the log strings never mention.

| namespace | classes |
|---|---:|
| `(global)` | 1978 |
| `cui` | 298 |
| `sce` | 58 |
| `_GLOBAL__N_1` | 13 |
| `ui` | 7 |
| `SpeedTree` | 4 |
| `font` | 3 |
| `AK` | 3 |
| `c_activity_client_messaging_interface_bap` | 2 |
| `hclPhysicsClothWorld` | 2 |
| `c_demonware_http_upload_client` | 1 |
| `detail` | 1 |
| `c_cui_screen_widget` | 1 |
| `hkLargeBlockAllocator` | 1 |
| `hkContainerTempAllocator` | 1 |

The `voice` slice, which is the whole reason this section exists:

| vaddr | class |
|---|---|
| `0x15cd6f0` | `c_simulation_queue_send_user_interface_voice_notification_definition` |
| `0x15d0e00` | `cui::c_voice_channel_data_component` |
| `0x15d0e30` | `cui::c_voice_channel_switcher_component` |
| `0x15d1bd0` | `c_simulation_queue_send_voice_incident_definition` |
| `0x15d1c10` | `c_voice_controller_monitor` |
| `0x167d620` | `49c_simulation_queue_element_schematized_definitionI41s_simulation_queue_voice_incident_payloadE` |
---

## 9. Notable NOT-SEEN, evidence class `strong` `[C]`

These reach the sink and pass the threshold we ran. They have never printed, so **the
instruction was never reached** in any of this project's runs. Filtered to the subsystems the current
investigations touch; the TSV has all of them.


| subsystem | vaddr | fn | text |
|---|---|---|---|
| `networking:demonware` | `0x1603893` | `0x9e6c0` | `networking:demonware: SSL validation error: internal library error. allowed flags=0x%x, received=0x%x, effective=0x%x` |
| `networking:demonware` | `0x1603909` | `0x9e6c0` | `networking:demonware: SSL validation error: format is invalid. allowed flags=0x%x, received=0x%x, effective=0x%x` |
| `networking:demonware` | `0x160397a` | `0x9e6c0` | `networking:demonware: SSL validation error: common name and the host name do not match. allowed flags=0x%x, received=0x%` |
| `networking:demonware` | `0x1603a04` | `0x9e6c0` | `networking:demonware: SSL validation error: validity period has expired. allowed flags=0x%x, received=0x%x, effective=0x` |
| `networking:demonware` | `0x1603a7f` | `0x9e6c0` | `networking:demonware: SSL validation error: validity period has not started. allowed flags=0x%x, received=0x%x, effectiv` |
| `networking:demonware` | `0x1603afe` | `0x9e6c0` | `networking:demonware: SSL validation error: unknown or untrusted CA. allowed flags=0x%x, received=0x%x, effective=0x%x` |
| `networking:investment:server_message:encode` | `0x1612a3c` | `0x261be0` | `networking:investment:server_message:encode: payload_bitstream error when encoding message` |
| `networking:investment:server_message:decode` | `0x1612a97` | `0x261fb0` | `networking:investment:server_message:decode: message out_payload_buffer too small to fit message structure` |
| `networking:investment:server_message:decode` | `0x1612b02` | `0x261fb0` | `networking:investment:server_message:decode: failed to decode message index %d` |
| `networking:investment:server_message:decode` | `0x1612b51` | `0x261fb0` | `networking:investment:server_message:decode: failed to get the server message shared definition for index %d` |
| `networking:investment:server_message:decode` | `0x1612bbe` | `0x261fb0` | `networking:investment:server_message:decode: failed to decode message, alignment-padding for the message arguments would` |
| `networking:investment:server_message:decode` | `0x1612c59` | `0x261fb0` | `networking:investment:server_message:decode: failed to decode message, decoded message arguments would exceed out_payloa` |
| `networking:investment:server_message:decode` | `0x1612ce2` | `0x261fb0` | `networking:investment:server_message:decode: failed to decode message, decoded optional variable payload would exceed ou` |
| `networking:investment:server_message:decode` | `0x1612dce` | `0x261fb0` | `networking:investment:server_message:decode: rsat error when decoding message-arguments` |
| `networking:investment:server_message:encode` | `0x1612e83` | `0x261be0` | `networking:investment:server_message:encode: rsat error when encoding message-arguments into payload` |
| `networking:investment:server_message:encode` | `0x1612ee8` | `0x261be0` | `networking:investment:server_message:encode: bitstream error when encoding message-arguments into payload` |
| `networking:signon:platform` | `0x161583a` | `0x2a5fd0` | `networking:signon:platform: retrieveSCEAccessToken failed to start.` |
| `networking:signon` | `0x1615985` | `0x2a7090` | `networking:signon: Token expiring: reauthenticating` |
| `networking:signon` | `0x16159b9` | `0x2a8170` | `networking:signon: cannot refresh content rights, signon operation already in progress` |
| `networking:signon` | `0x1615a10` | `0x2a8170` | `networking:signon: can only refresh content rights if signed in` |
| `networking:signon` | `0x1615a50` | `0x2a82d0` | `networking:signon: cannot refresh email, signon operation already in progress` |
| `networking:signon` | `0x1615a9e` | `0x2a82d0` | `networking:signon: can only refresh email if signed in` |
| `networking:signon` | `0x1615ad5` | `0x2a8c70` | `networking:signon: Error sending signon request - aborting signon attempt` |
| `networking:signon` | `0x1615b4b` | `0x2a8df0` | `networking:signon: Entering signon queue` |
| `networking:signon` | `0x1615b74` | `0x2a8df0` | `networking:signon: Signon server returned a platform error` |
| `networking:signon` | `0x1615baf` | `0x2a8df0` | `networking:signon: Signon server returned a configuration error` |
| `networking:signon` | `0x1615bef` | `0x2a8df0` | `networking:signon: Signon server returned a commercialization error` |
| `networking:signon` | `0x1615c33` | `0x2a8df0` | `networking:signon: Signon server returned a version mismatch error` |
| `networking:signon` | `0x1615c76` | `0x2a8df0` | `networking:signon: Signon server returned an internal server error` |
| `networking:signon` | `0x1615d5b` | `0x2a8df0` | `networking:signon: Signon server denied the login` |
| `networking:signon` | `0x1615d8d` | `0x2a8df0` | `networking:signon: Signon server reported that the user is suspended` |
| `networking:signon` | `0x1615dd2` | `0x2a8df0` | `networking:signon: User action required` |
| `networking:signon` | `0x1615dfa` | `0x2a8df0` | `networking:signon: Signon server reported that the title is in prelaunch` |
| `networking:signon` | `0x1615e43` | `0x2a7090` | `networking:signon: Reauthentication failed because server is offline.  Booting.` |
| `networking:signon` | `0x1615efe` | `0x2a7090` | `networking:signon: Server did not report a content build version` |
| `networking:signon:client` | `0x1616032` | `0x2a99d0` | `networking:signon:client: Client clock offset from server clock: [%lld] seconds` |
| `networking:signon:client` | `0x161623f` | `0x2aabd0` | `networking:signon:client: Signon server url %d added: %s` |
| `networking:signon:client` | `0x1616278` | `0x2aabd0` | `networking:signon:client: Too many signon server urls specified.  Limit is %d` |
| `networking:signon:client` | `0x16162c6` | `0x2aabd0` | `networking:signon:client: Invalid signon server url: %d bytes is longer than max of %d` |
| `networking:signon:client` | `0x161631d` | `0x2aaee0` | `networking:signon:client: Error encoding signon request` |
| `networking:signon:client` | `0x1616370` | `0x2ab8e0` | `networking:signon:client: Token size %d is too large for buffer (%d)` |
| `networking:signon:client` | `0x1616439` | `0x2abcc0` | `networking:signon:client: Server busy: status %d.  Backing off retries immediately.` |
| `networking:signon:client` | `0x161648d` | `0x2abcc0` | `networking:signon:client: Signon server request failed with an internal server error.` |
| `networking:signon:client` | `0x1616537` | `0x2aa350` | `networking:signon:client: Could not allocate %d bytes for security token` |
| `networking:signon:client` | `0x1616580` | `0x2aa6b0` | `networking:signon:client: Sign-on Server OFFLINE. time_remaining[%us]` |
| `networking:signon:client` | `0x16165c6` | `0x2aa8f0` | `networking:signon:client: Signon denied: reason [%d]` |
| `networking:signon:client` | `0x16165fb` | `0x2aa8f0` | `networking:signon:client: Denial message: %s` |
| `networking:signon:client` | `0x1616628` | `0x2aa8f0` | `networking:signon:client: Account suspension end time %s` |
| `networking:demonware:memory` | `0x1616b35` | `0x2b1330` | `networking:demonware:memory: Failed to allocate '%ld' bytes of memory.` |
| `networking:demonware` | `0x161ad0e` | `0x2e9fc0` | `networking:demonware: Couldn't parse local IP address %s` |
| `networking:demonware` | `0x161ad47` | `0x2e9fc0` | `networking:demonware: Error calling sceNetCtlGetInfo to obtain local ip address: %x` |
| `networking:demonware` | `0x161adba` | `0x2e9fc0` | `networking:demonware: unable to set local address for failback transport startup` |
| `networking:demonware:transport` | `0x161afbf` | `0x2ed9c0` | `networking:demonware:transport: could not unregister secure key with ID [%s] because it was not registered, socket_route` |
| `networking:demonware:transport` | `0x161b217` | `0x2ef370` | `networking:demonware:transport: bdSocketRouter::sendTo reported sending a different number of bytes than the packet size` |
| `networking:demonware:transport` | `0x161b33c` | `0x2ef6a0` | `networking:demonware:transport: rejecting incoming connection because we have no free connection slots` |
| `networking:demonware:transport` | `0x161b52f` | `0x2edce0` | `networking:demonware:transport: security to %s terminate because of %s` |
| `networking:investment:server_message_interface` | `0x162aa3c` | `0x551fb0` | `networking:investment:server_message_interface: failed to send message. message_type:[%s] bap_connection_type:[%s] bap_c` |
| `networking:investment:account_id_translation:decode` | `0x162ad85` | `0x55a490` | `networking:investment:account_id_translation:decode: output buffer too small to fit platform_to_investment_response head` |
| `networking:investment:account_id_translation:decode` | `0x162ae00` | `0x55a490` | `networking:investment:account_id_translation:decode: output buffer too small to fit platform_to_investment_response acco` |
| `networking:investment:account_id_translation:encode` | `0x162ae80` | `0x55a090` | `networking:investment:account_id_translation:encode: bitstream error when encoding header` |

(60 shown; `--subsystem <prefix>` lists any family in full.)

---

## 10. What was deliberately EXCLUDED - say it or the silence lies

A silent cap reads as "covered everything". This is the cap, stated:

| excluded | how much | why, and what would include it |
|---|---|---|
| Strings with no `subsystem:` prefix that are *not* referenced from the four §6 ranges | most of the ~14,800 printable runs in the image | the brief's scope. Add a range to `HOT_RANGES` and re-run to pull any other neighbourhood in. |
| `Error:` / `Warning:`-style Demonware severities | ~13 `bdJSON` strings and their neighbours | `SUBSYS_RE` requires a **lowercase** identifier path, so `Error:` is not a subsystem. They are catalogued only if a §6 function references them. |
| Shared-library and `sce*` strings | not counted | they belong to the emulator's HLE surface, not the client. ⚠ `sceNpPartyCheckCallback` - one of the two strings a theory was built on - **is not in this eboot at all**; it is a shadPS4 symbol. Nothing in this catalogue could ever have marked it SEEN, and that is the correct answer. |
| RTTI names as full catalogue rows | 2384 | they are class names, not printable text; they get §8 and the JSON, and are excluded from the main table so `arity` / `SEEN` stay meaningful. |
| Non-`lea` reference forms for strings | - | `call`/`jmp`/RELA are scanned by `d1-eboot xref`; a string literal is reached by `lea` in this image and the `ref=none` residue (§5.1) is 12 rows, so the loss is bounded and named. |
| Log volume per run | - | SEEN is a *set* question. The TSV records how many runs and the most recent, not counts per run. |
| The 5 runs with no sink patch | 6 of 248 | they emit nothing through this path by construction; included in the corpus, they simply contribute nothing. |

---

## 11. Positive controls - printed on every run, exit 2 on failure

| control | result |
|---|---|
| build identity (sha256) == pinned 01.29 eboot | **PASS**  `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954` |
| `call 0xe4da0` sites == 1632 (guest-diagnostics.md §3) | **PASS**  `1632` |
| ...of which 1614 have both immediates statically recoverable | **PASS**  `1614` |
| level histogram == {0:80, 1:265, 2:277, 3:366, 4:626} | **PASS**  `{0: 80, 1: 265, 2: 277, 3: 366, 4: 626}` |
| 22 distinct retail_log system ids (0..21) | **PASS**  `22` |
| ⭐ `voice_globals` catalogued at 0x167d3e0 | **PASS**  `0x167d3e0` |
| ⭐ `voice_globals` attributed to fn 0xb41000 | **PASS**  `0xb41000` |
| ⭐ allocation tag `voice_globals` -> global 0x2815770 from fn 0xb41000 | **PASS**  `0x2815770` |
| allocation tag `retail_log` -> global 0x256eef0 from fn 0xe4890 | **PASS**  `0x256eef0` |
| ui-substage table anchor 0x164993f reads `@waiting_for_login` | **PASS**  `b'@waiting_for_login\\x00l'` |
| ...+10 entries in, ENUM(19) == `bootflow bap signin` | **PASS**  `bootflow bap signin` |
| ⭐ SEEN: `world_controller:state_manager: Entering state` marked SEEN | **PASS**  `239 runs` |
| ...and it is UNFILTERED, so SEEN is a clean execution trace for it | **PASS** |
| ⭐ SEEN(0x1648203) agrees with an independent literal grep of runs/ [both say SEEN] | **PASS**  `seen=1 grep=239 run(s) (e.g. sweep-sub0-5)` |
| ⭐ SEEN(0x162f4a1) agrees with an independent literal grep of runs/ [both say SEEN] | **PASS**  `seen=1 grep=2 run(s) (e.g. 20260809-225119-UNION8-MOONBASE)` |
| ⭐ the SEEN column still DISCRIMINATES - strong-evidence rows that are NOT seen | **PASS**  `1422 strong + unseen` |
| ⭐ every observed guest line is explained by a catalogued string | **PASS**  `0 unexplained` |
| SEEN scan found >=100 runs with a measured threshold | **PASS**  `242/248` |
| catalogue >= 1700 strings | **PASS**  `2155` |
| RTTI names >= 2000 | **PASS**  `2384` |
| decoder desync low: unconfirmed `lea` sites < 1% of confirmed refs | **PASS**  `0 unconfirmed` |

The decoder's four-part reproduction of `historicaldocs/design/guest-diagnostics.md` §3 - 1,632 gate sites, 1,614
with both immediates, the exact level histogram, 22 system ids - is the control that matters most,
because it was derived by a different tool, in a different session, from the same bytes (ATLAS §4.4d:
*a new tool is not evidence until it reproduces a known result*).

**Cross-tool check of the attribution itself**, run by hand against `d1-eboot xref` (a Rust
implementation with its own decoder and its own controls) - 4/4 exact, same functions and the same
instruction sites:

| string | this tool | `d1-eboot xref` |
|---|---|---|
| `0x1648203` | fn `0x782300` @ `0x78249f`,`0x782502` | identical |
| `0x162f556` | fn `0x57c080` @ `0x57c568`,`0x57c5a2` | identical |
| `0x167b267` | fn `0xb2e020` @ `0xb2e2f2`,`0xb2e344` | identical |
| `0x164b74d` | fn `0x791390` @ `0x7913fd`,`0x79142b` | identical |

`[U]` This cross-check is **manual, not automated** - wiring it in would make this tool depend on a
built `d1-eboot`. Re-run it by hand after any change to `Scan.run()`.

`[U]` **Not controlled**: that a *specific* run's threshold was in force for the whole run. `T` is read
from the `Applied patch:` line at load; nothing re-checks it later. No mechanism to change it mid-run
exists, so this is a completeness note, not a live doubt.

---

*Generated 2026-08-10 by `capture-rig/diagnostic-catalogue.py` from eboot
`7271fcb926401df8…` and 248 runs. Do not hand-edit - re-run the generator.*

