# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Markdown renderer for `diagnostic-catalogue.py`.

Kept separate so the scanner stays readable; it has no analysis in it beyond aggregation.
Nothing here may compute a number the scanner did not already produce.
"""

from __future__ import annotations

import collections
import datetime

HOT_ORDER = ["voice/session", "bap", "investment/registry", "world_controller states"]
HOT_SPAN = {
    "voice/session": (0xB41000, 0xB48000),
    "bap": (0xB2E000, 0xB33000),
    "investment/registry": (0x570000, 0x590000),
    "world_controller states": (0x790000, 0x7A1000),
}


def esc(s: str) -> str:
    return (s.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")
             .replace("\t", "\\t").replace("|", "\\|").replace("`", "'"))


def hot_of(fn):
    if fn is None:
        return None
    for k, (lo, hi) in HOT_SPAN.items():
        if lo <= fn < hi:
            return k
    return None


def render(img, scan, rows, rtti, idx, thresholds, unmatched, ctl, allocs, n_lines) -> str:
    ts = [v for v in thresholds.values() if v is not None]
    min_T = min(ts) if ts else 5
    T_hist = collections.Counter(ts)
    ev = collections.Counter(r.seen_evidence(min_T) for r in rows)
    seen = [r for r in rows if r.seen]
    o: list[str] = []
    w = o.append

    # ---------------------------------------------------------------- header
    w("# The client's own diagnostic strings - a catalogue, keyed for lookup\n")
    w(f"""**Every diagnostic / log / status string Destiny 1 (CUSA00219 patch 01.29) can print, what
subsystem owns it, which function references it, what data that site has in hand - and ⭐ whether we
have ever actually seen it fire.** Produced by
`capture-rig/diagnostic-catalogue.py`, which re-derives every number below from
`dumps/patch-01.29-CUSA00219/eboot.bin` plus every `runs/*/emulator.log`, and **exits non-zero
if any of its {len(ctl)} positive controls fails**.

Full per-string rows - all {len(rows)} of them, every column - are in
**`capture-rig/diagnostic-catalogue.tsv`**. This document is the index, the caveats, and the
tables worth reading with your eyes.

Reach tags: the static half (address, subsystem, referencing function, arity, gate) is
**`static-client-inference`**; the SEEN half is **`observed-live`** over {len(thresholds)} runs of
our own client. `vaddr = file_offset - 0x4000` for the text mapping (⚠ per-segment - ATLAS §4.1).
""")

    w(f"""> ## Why this document exists
>
> The client crashed in a 4-slot table lookup and three sessions theorised about a "missing
> fireteam identity" for hours. The function that fills that table references a string literal
> reading **`voice_globals`** - it is the *voice chat* subsystem, and shadPS4 stubs `sceVoice`
> entirely. One string reference, never looked up, because **there was nowhere to look it up**.
>
> Worse, twice in one night a theory was built on a diagnostic string that had **never actually
> fired**. There was no way to ask "have we ever seen this?" without hand-grepping every run.
> That question is now a column.
""")

    # ---------------------------------------------------------------- §0
    w("---\n\n## 0. The answer in one paragraph\n")
    w(f"""**{len(rows)} diagnostic strings** across **{len(idx)} subsystems**, of which **{len(seen)}
({100*len(seen)/len(rows):.1f}%) have ever appeared in a run** - and that is not a measure of how far
the client gets, because {ev.get('none',0)} of them are not wired to the log sink at all and
{ev.get('weak-blind',0)} more are *mechanically incapable* of printing at any threshold this project
has ever run. Netting those out, **{ev.get('strong',0)} strings are ones whose absence is real
evidence that their code did not execute** - including every `networking:investment:peer:` line, which
is why "the peer account-SOID family never fired" is now a fact rather than a hunch. The single most
useful structural finding is that **{sum(1 for r in rows if r.unfiltered)} strings reach the sink with
NO filter in front of them**: those print unconditionally the moment our sink patch is applied, so for
them SEEN/NOT-SEEN is a clean execution trace.
""")

    # ---------------------------------------------------------------- §1 usage
    w("---\n\n## 1. How to use it - three queries and a TSV\n")
    w("""```bash
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
""")

    # ---------------------------------------------------------------- §2 SEEN
    w("---\n\n## 2. ⚠ What the SEEN column is worth - read this before citing it\n")
    # ⛔ 2026-08-10: a THIRD gate exists (no sink call at all) and this catalogue is structurally
    # blind to it.  The banner lives HERE, in the renderer, because the .md is fully regenerated -
    # a retraction written only into the generated file is erased by the next `--write`.
    w("""> ⛔⛔ **A THIRD GATE EXISTS AND THIS DOCUMENT CANNOT SEE IT (2026-08-10).** The two gates below are
> both about a *logger that was called*. There is a third shape: **a site whose only formatting call
> is a `vsnprintf` into a stack buffer that is never read - no sink call at all.** Worked example:
> `can't spawn due to the current activity lifetime state ('%s')` in `FUN_00414730`
> (`FUN_010548E0` is `vsnprintf`; the buffer is referenced exactly twice - its declaration and that
> call). **0 hits across every run, and that is a structural false negative.** Positive control: the
> string IS in the image at file offset `0x1625926`.
>
> ⛔ **Such a site is not a `retail_log` site, so it is NOT IN THIS CATALOGUE AT ALL** - it cannot even
> be classified `none`. ⇒ **the `strong` evidence class below means \"absence is evidence *for
> catalogued sites*\", never \"absence is evidence for this string\".** Before treating a missing
> diagnostic as evidence, read the emitter and name the function that would have output it.
> ⇒ `historicaldocs/design/arrival-handoff.md` §6.4, `status/LESSONS.md`.

""")
    w(f"""**A string absent from every log does NOT prove its code never ran.** `status/LESSONS.md`
and `historicaldocs/design/guest-diagnostics.md` record the trap: there are **two independent gates**, and only one
of them is opened by our patch unconditionally.

| gate | address | what it does | our runs |
|---|---|---|---|
| 1 · **filter** | `0xe4da0` | `threshold[system] <= level` - a HIGHER threshold is QUIETER | replaced by a uniform `T`, and **`T` has never been below {min_T}** |
| 2 · **sink** | `0xe4ad0` | `memcpy` into a 1124-slot ring **that nothing ever drains** | replaced by `printf("%s\\n")` in {len(ts)} of {len(thresholds)} runs |

Thresholds actually used, measured from each run's own log rather than from the patch file:

| T | runs | sites that pass |
|---:|---:|---:|
""")
    for t in sorted(T_hist):
        passes = sum(1 for r in rows if r.level is not None and r.level >= t)
        w(f"| {t} | {T_hist[t]} | {passes} |")
    w(f"| - (no sink patch) | {len(thresholds)-len(ts)} | 0 |\n")

    w(f"""Because the lowest threshold ever run is **T={min_T}**, every site whose own level is
below {min_T} has been **structurally invisible for this project's entire history**. So each row
carries an *evidence class*, and it is the number to quote, not the raw SEEN bit:

| class | rows | what a NOT-SEEN verdict means |
|---|---:|---|
| `seen` | {ev.get('seen',0)} | it fired; the TSV names the most recent run |
| **`strong`** | {ev.get('strong',0)} | it reaches the sink and passes the filter we ran ⇒ **absence means the code did not execute** |
| `weak-blind` | {ev.get('weak-blind',0)} | its level is below every threshold we have run ⇒ absence proves **nothing**; re-run at `T=0` to ask |
| `none` | {ev.get('none',0)} | not a `retail_log` site at all (status overlays, registry names, allocation tags) ⇒ absence proves **nothing** |

⭐ **The strongest sub-class**: {sum(1 for r in rows if r.unfiltered)} strings are `lea`d into an
emit call with **no `retail_log_check` in front of them at all**. Those are threshold-independent -
with the sink patched they print every single time the instruction is reached. Both halves of the
worked example are here: `world_controller:state_manager: Entering state '%s' for reason '%s'.`
(`0x1648203`) is unfiltered and appears in {len(next(r for r in rows if r.vaddr==0x1648203).runs)} runs,
while `networking:investment:peer: Account soids in error state: %s` (`0x162f4a1`) is **also
unfiltered** and appears in **zero**. The second fact is a measurement, not a silence.

**Coverage control, and it is two-sided:** {len(unmatched)} of the {n_lines:,} distinct guest lines
observed across all runs failed to match a catalogued format string. A hole there would make every
NOT-SEEN verdict suspect; it is asserted as a control, and it is how two false negatives (formats
ending in a `%s` that rendered empty, so the log's trailing space was stripped away) were caught.
""")

    # ---------------------------------------------------------------- §3 mechanism
    w("---\n\n## 3. The mechanism, and how a string is attributed `[C]`\n")
    w(f"""Every diagnostic site compiles to the same four-call shape, and the scanner reads exactly
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

The scanner walks each of the **{len(scan.prologues):,}** prologue-anchored functions, decoding
**{scan.insn_count:,}** instructions, and closes an *emit group* at every `call 0xe4ad0`: every format
string `lea`d since the previous emit is what that call prints, and the filter that was armed is the
one it passed. That is what produces the `sys` / `level` / `unfiltered` columns.

Measured totals: **{len(scan.gate_sites)}** filter calls, **{len(scan.emit_sites)}** emit calls,
**{sum(1 for r in rows if r.emits)}** catalogued strings reaching an emit.
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
**Every start address here is correct by construction.** The {sum(1 for r in rows if not r.refs)} rows
flagged `ref=none` are the honest residue (§5).

An earlier revision of this tool tried to compensate by also testing `start+1`, and manufactured a
whole fake subsystem: 13 Demonware `Error: …` strings catalogued as **`rror: …`**, one byte past their
true start. That is ATLAS §4.2b created by the instrument rather than walked into, and it is why the
`start+1` path is gone rather than merely fixed.
""")

    # ---------------------------------------------------------------- §3.2 packed tables
    names = img.blob[0x1649940 - img.text_lo:0x1649940 - img.text_lo + 4096].split(b"\x00")
    w("""### 3.2 Packed name tables - enumerated as tables, not as rows `[C]`

A *packed name table* is a run of NUL-separated names indexed by an integer. Nothing `lea`s the
individual entries, so reference-first sees the table and not its members, and listing each name as a
catalogue row would imply a call site that does not exist. The two known tables are recorded here
instead, and both are asserted as controls.

| anchor | entries | indexing | owner |
|---|---:|---|---|""")
    w(f"| **`0x164993f`** (names begin `0x1649940`) | {sum(1 for n in names[:64] if n)}+ | "
      f"`name_index = ENUM(n) + 10` | the `ui_stage` substage goal - `design/eboot-map.md` §7 |")
    w("| `0x1682e1d` | 12 | `[U]` | managed-session host migration reason codes - §5.1 |")
    w(f"""
Spot-check of the first, reproduced on every run of this tool: index 29 (`ENUM(19)`) reads
`{names[29].decode() if len(names) > 29 else '?'}`, which is `bootflow:bap_signin` naming itself -
§7's own control, re-derived here rather than copied (ATLAS §4.3).

⚠ And the correction §3.1 predicts: §7 publishes entry -10 as **`@waiting_for_login`**. The `@` is
`0x40`, the last byte of the float `4.3` that precedes the table. The name is `waiting_for_login` and
the table's first name starts at `0x1649940`. Every offset in §7 is unaffected.
""")

    # ---------------------------------------------------------------- §4 system ids
    w("---\n\n## 4. `retail_log` system id → subsystem, derived rather than guessed `[C]`\n")
    bysys = collections.defaultdict(collections.Counter)
    for r in rows:
        if r.sys is None:
            continue
        sub = r.subsystem or "(no prefix)"
        bysys[r.sys][":".join(sub.split(":")[:2])] += 1
    w("""`historicaldocs/design/guest-diagnostics.md` §3 built this table by correlating each gate site with the
`networking:` string it goes on to format - which necessarily mislabels every system whose strings are
**not** `networking:`. Deriving it from emit groups instead resolves all five `[U]` rows and corrects
two more. Counts are catalogued strings, not gate sites.

| sys | subsystem (this tool) | strings | `guest-diagnostics.md` §3 said |
|---:|---|---:|---|""")
    OLD = {0: "channel_manager / channel", 1: "demonware / transport", 2: "activity_client",
           3: "`[U]`", 4: "`[U]`", 5: "online", 6: "logic / online / configuration / leaderboard",
           7: "investment", 8: "investment", 9: "session", 10: "session / logic", 11: "stun",
           12: "messages", 13: "signon / online", 14: "sku_manager", 15: "`[U]`",
           16: "simulation", 17: "team_picker", 18: "`[U]`", 19: "managed_session",
           20: "platform_user", 21: "`[U]`"}
    for s in sorted(bysys):
        tot = sum(bysys[s].values())
        top = " · ".join(f"`{k}`" for k, _ in bysys[s].most_common(3))
        old = OLD.get(s, "")
        mark = " **⬅ resolves `[U]`**" if old == "`[U]`" else ""
        w(f"| {s} | {top}{mark} | {tot} | {old} |")
    w("""
Corrections worth carrying: **sys 2 and sys 3 are `world_controller:activity_manager`**, not
`activity_client`; **sys 5 is the `world_controller` state machine**, not `online`; **sys 4 is
`world_controller:slice_set_transition_manager`** - 108 strings, the single largest subsystem in the
image, and it had no name at all before.
""")

    # ---------------------------------------------------------------- §5 subsystem index
    w("---\n\n## 5. The subsystem index - the primary key\n")
    onec = sum(1 for d in idx if d["strings"] == 1 and ":" not in d["subsystem"])
    w(f"""{len(idx)} subsystems, rolled up to two levels. **`fn range`** is the span of functions that
reference this subsystem's strings, which is what makes an unknown function address answerable; use
`--addr` for the exact answer, because the spans overlap.

⚠ `[I]` **{onec} of these rows carry exactly one string and one path component** (`in_os_party_chat`,
`estimated`, `current`, …). Those are almost certainly *labels* in a status dump rather than
subsystems - the classifier cannot tell `label: %s` from `subsystem: message` and does not pretend
to. `--addr` marks them `(label?, 1 string)`; treat any single-string row here as unnamed.

| subsystem | strings | SEEN | string range | fn range | fns |
|---|---:|---:|---|---|---:|""")
    for d in idx:
        fr = f"`{d['fn_lo']:#x}`..`{d['fn_hi']:#x}`" if d["fn_lo"] is not None else "-"
        w(f"| `{d['subsystem']}` | {d['strings']} | {d['seen']} | "
          f"`{d['str_lo']:#x}`..`{d['str_hi']:#x}` | {fr} | {d['fns']} |")

    noref = [r for r in rows if not r.refs]
    w(f"""
### 5.1 The {len(noref)} strings nothing references `[C]`

Reference-first cannot see a string reached by *indexing a packed table*, and this is what that
residue looks like - one table, one subsystem family:
""")
    w("\n| vaddr | text |\n|---|---|")
    for r in noref:
        w(f"| `{r.vaddr:#x}` | `{esc(r.text)}` |")
    w("""
They are consecutive, they share the `host_handoff:` / `host_reestablish:` / `peer_reestablish:`
prefixes, and none carries a format specifier - i.e. a **reason-code name table** for managed-session
host migration, indexed the way `design/eboot-map.md` §7's ui-substage table is. `[I]` Not chased
further; noted so the next reader does not read `ref=none` as "unused".
""")

    # ---------------------------------------------------------------- §6 hot ranges
    w("---\n\n## 6. The four ranges we are stuck in, in full `[C]`\n")
    w("""These are the address ranges named in the current investigations. Every string referenced
from them is listed whether or not it carries a subsystem prefix - `voice_globals` is exactly the case
that motivated it.

`L` is the site's own message level (`-` = no filter in front of it). `E` is the §2 evidence class:
**SEEN** it fired · `strong` it would have printed, so absence means the code did not run ·
`blind` its level is below every threshold we have run · `-` not a `retail_log` site at all, so
absence proves nothing.
""")
    for hk in HOT_ORDER:
        lo, hi = HOT_SPAN[hk]
        sel = [r for r in rows if hot_of(r.fn) == hk]
        nseen = sum(1 for r in sel if r.seen)
        w(f"\n### 6.{HOT_ORDER.index(hk)+1} `{hk}` - `{lo:#x}`..`{hi:#x}` - "
          f"{len(sel)} strings, {nseen} SEEN\n")
        w("| vaddr | fn | L | E | arity | text |\n|---|---|---:|---|---:|---|")
        for r in sorted(sel, key=lambda r: (r.fn or 0, r.vaddr)):
            e = r.seen_evidence(min_T)
            mark = {"seen": "**SEEN**", "strong": "strong", "weak-blind": "blind", "none": "-"}[e]
            lv = r.level if r.level is not None else "-"
            w(f"| `{r.vaddr:#x}` | `{r.fn:#x}` | {lv} | {mark} | {r.arity} | `{esc(r.text)}` |")

    # the voice punchline
    vg = next((r for r in rows if r.text == "voice_globals"), None)
    if vg:
        w(f"""
#### ⭐ The `voice/session` range in one line

`{vg.vaddr:#x}` `voice_globals` is `lea`d at `0xb41047` by **`0xb41000`**, which allocates
`0x6ba0` bytes and stores the pointer at BSS **`0x2815770`**. Everything else in that range is the
**voice status overlay** owned by `0xb44460` (`PLAYERS: %5s %16s …`, `TALKING?`, `LAST TALKED`) plus
`0xb46250` `voice engine orbis internal` and `0xb463e0` `BNG:VOICE_ENGINE`. **Not one of these
{len([r for r in rows if hot_of(r.fn) == 'voice/session'])} strings is a `retail_log` site**, so the
whole range is evidence class `none`: it could be running and we would see nothing either way. That
is the honest statement of what the logs can and cannot tell us about voice.
""")

    # ---------------------------------------------------------------- §7 globals
    w("---\n\n## 7. Globals and allocation tags each range owns `[I]`\n")
    w("""A snake_case identifier string `lea`d by a function that also takes the address of a
data/BSS global. Two known cases anchor it as controls - `0xb41000 → voice_globals @0x2815770` and
`0xe4890 → retail_log @0x256eef0` - and both are asserted on every run. **This is the one heuristic
in the tool; a row here is a lead, not a fact.** Restricted below to the four ranges.

The `investment/registry` block is the most immediately useful thing in this document after §6: it is
a **registry-object name → constructing function** map, so `investment_roster` is `0x5817d0` and
`investment_peer_subscription_manager` is `0x57c730` without disassembling anything.

| range | fn | tag | global |
|---|---|---|---|""")
    seen_pairs = set()
    for a in allocs:
        h = a.get("hot")
        if not h:
            continue
        key = (a["fn"], a["tag"])
        if key in seen_pairs:
            continue
        seen_pairs.add(key)
        w(f"| `{h}` | `{a['fn']:#x}` | `{esc(a['tag'])}` | `{a['global']:#x}` |")
    w(f"\n{len(allocs)} candidate pairs image-wide; `--json` emits all of them.\n")

    # ---------------------------------------------------------------- §8 rtti
    w("---\n\n## 8. RTTI class names - subsystem names for free `[C]`\n")
    ns = collections.Counter()
    for r in rtti:
        n = r.subsystem or ""
        ns[n.split("::")[0] if "::" in n else "(global)"] += 1
    w(f"""**{len(rtti)} Itanium-mangled type-name strings** in the image, demangled where the shape is
simple (`26c_x` → `c_x`, `N3cui30c_yE` → `cui::c_y`). They are in the TSV's sibling JSON
(`--json`), not in the main table, because they are class names rather than printable diagnostics -
but they name subsystems the log strings never mention.

| namespace | classes |
|---|---:|""")
    for k, v in ns.most_common(15):
        w(f"| `{k}` | {v} |")
    vhits = [r for r in rtti if "voice" in (r.subsystem or "").lower()]
    w(f"""
The `voice` slice, which is the whole reason this section exists:

| vaddr | class |
|---|---|""")
    for r in vhits:
        w(f"| `{r.vaddr:#x}` | `{esc(r.subsystem or r.text)}` |")

    # ---------------------------------------------------------------- §9 never fired
    w("---\n\n## 9. Notable NOT-SEEN, evidence class `strong` `[C]`\n")
    w("""These reach the sink and pass the threshold we ran. They have never printed, so **the
instruction was never reached** in any of this project's runs. Filtered to the subsystems the current
investigations touch; the TSV has all of them.
""")
    w("\n| subsystem | vaddr | fn | text |\n|---|---|---|---|")
    WANT = ("networking:investment", "networking:server", "networking:signon",
            "networking:demonware", "world_controller:state_manager")
    n = 0
    for r in rows:
        if r.seen or r.seen_evidence(min_T) != "strong":
            continue
        if not (r.subsystem or "").startswith(WANT):
            continue
        w(f"| `{r.subsystem}` | `{r.vaddr:#x}` | `{r.fn:#x}` | `{esc(r.text)[:120]}` |")
        n += 1
        if n >= 60:
            break
    w(f"\n({n} shown; `--subsystem <prefix>` lists any family in full.)\n")

    # ---------------------------------------------------------------- §10 scope
    w("---\n\n## 10. What was deliberately EXCLUDED - say it or the silence lies\n")
    w(f"""A silent cap reads as "covered everything". This is the cap, stated:

| excluded | how much | why, and what would include it |
|---|---|---|
| Strings with no `subsystem:` prefix that are *not* referenced from the four §6 ranges | most of the ~14,800 printable runs in the image | the brief's scope. Add a range to `HOT_RANGES` and re-run to pull any other neighbourhood in. |
| `Error:` / `Warning:`-style Demonware severities | ~13 `bdJSON` strings and their neighbours | `SUBSYS_RE` requires a **lowercase** identifier path, so `Error:` is not a subsystem. They are catalogued only if a §6 function references them. |
| Shared-library and `sce*` strings | not counted | they belong to the emulator's HLE surface, not the client. ⚠ `sceNpPartyCheckCallback` - one of the two strings a theory was built on - **is not in this eboot at all**; it is a shadPS4 symbol. Nothing in this catalogue could ever have marked it SEEN, and that is the correct answer. |
| RTTI names as full catalogue rows | {len(rtti)} | they are class names, not printable text; they get §8 and the JSON, and are excluded from the main table so `arity` / `SEEN` stay meaningful. |
| Non-`lea` reference forms for strings | - | `call`/`jmp`/RELA are scanned by `d1-eboot xref`; a string literal is reached by `lea` in this image and the `ref=none` residue (§5.1) is 12 rows, so the loss is bounded and named. |
| Log volume per run | - | SEEN is a *set* question. The TSV records how many runs and the most recent, not counts per run. |
| The 5 runs with no sink patch | {len(thresholds)-len(ts)} of {len(thresholds)} | they emit nothing through this path by construction; included in the corpus, they simply contribute nothing. |
""")

    # ---------------------------------------------------------------- §11 controls
    w("---\n\n## 11. Positive controls - printed on every run, exit 2 on failure\n")
    w("| control | result |\n|---|---|")
    for name, ok, detail in ctl:
        w(f"| {name} | {'**PASS**' if ok else '**FAIL**'}{'  `' + esc(str(detail)) + '`' if detail else ''} |")
    w(f"""
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

*Generated {datetime.date.today().isoformat()} by `capture-rig/diagnostic-catalogue.py` from eboot
`{img.sha256[:16]}…` and {len(thresholds)} runs. Do not hand-edit - re-run the generator.*
""")
    return _tidy("\n".join(o) + "\n")


def _tidy(md: str) -> str:
    """Drop blank lines that fall *between* two table rows.

    Paragraph blocks here end with a newline (so paragraphs are separated) and table rows are
    appended one per call, which puts a blank line between a header separator and its first row.
    Markdown then renders the table as literal text. Collapsing only the between-rows case keeps
    every intentional paragraph break.
    """
    lines = md.split("\n")
    out: list[str] = []
    for i, ln in enumerate(lines):
        if ln.strip() == "" and out and out[-1].lstrip().startswith("|"):
            nxt = next((x for x in lines[i + 1:] if x.strip() != ""), "")
            if nxt.lstrip().startswith("|"):
                continue
        out.append(ln)
    return "\n".join(out)
