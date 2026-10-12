#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate the eboot atlas - the binary map, as an artifact instead of as prose.

    capture-rig/eboot-atlas.py                # regenerate every TSV, run every control
    capture-rig/eboot-atlas.py --controls     # controls only, no writes
    capture-rig/eboot-atlas.py --addr 0x574220

WHY THIS EXISTS
---------------
`design/eboot-map.md` names 164 addresses by hand. The image has **35,834** prologue-anchored
functions. Six hand-written map documents together name about 500 - 1.4%. Curation was never going
to close that gap, and every hand-copied address is an ATLAS §4.3 hazard (a superseded reading
surviving in another tool's constant).

So the map is **generated**. This script is orchestration only: every byte it reports is read by
`d1-server/crates/d1-eboot` (CLAUDE.md: *do not add a new Python ELF/descriptor/xref reader -
extend the crate*), and the one thing added here is the **join** - call-graph propagation of the
subsystem names that `diagnostic-catalogue.py` already derived.

WHAT IT WRITES
--------------
    capture-rig/eboot-atlas.tsv      47,280 rows - one per function
                                     ⚠ 35,834 `55 48 89 e5` + 11,446 frame-pointer-omitted ones
                                       found as direct `call` targets. The prologue count is NOT
                                       the function count: `0x56CA70`, `0xA8C920` and `0x260C10`
                                       are all in the second group, and a prologue-only index
                                       answers "which function is that?" with the previous one.
    capture-rig/eboot-types.tsv       2,633 rows - one per C++ class, from intact RTTI
    capture-rig/eboot-namehash.tsv     ~1,900 rows - the compiled {index, name_hash} enum tables
    capture-rig/eboot-callgraph.tsv  151,630 edges - call AND jmp (ATLAS §4.2)
    capture-rig/eboot-imports.tsv        638 rows - PLT imports: NID, GOT slot, stub, resolved name
    capture-rig/eboot-thiscalls.tsv   23,449 edges - caller passed its OWN `this` in RDI.
                                     ⚠ A FACT, not an inference. Chaining class names along these
                                       edges cross-validates at 41% and is NOT done - see
                                       historicaldocs/design/eboot-atlas-roadmap.md §3.2.

CONTROLS (exit 2 on any failure - the output may not be cited)
--------------------------------------------------------------
Runs `d1-eboot controls` first and refuses to continue unless all pass, then asserts its own joins
against readings that were made **by hand, in other sessions, by other methods**.
"""
from __future__ import annotations

import argparse
import bisect
import collections
import csv
import json
import os
import random
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RIG = os.path.join(ROOT, "capture-rig")
D1_EBOOT = os.path.join(ROOT, "d1-server", "target", "release", "d1-eboot")

ATLAS_TSV = os.path.join(RIG, "eboot-atlas.tsv")
TYPES_TSV = os.path.join(RIG, "eboot-types.tsv")
NAMEHASH_TSV = os.path.join(RIG, "eboot-namehash.tsv")
CALLGRAPH_TSV = os.path.join(RIG, "eboot-callgraph.tsv")
THISCALLS_TSV = os.path.join(RIG, "eboot-thiscalls.tsv")
IMPORTS_TSV = os.path.join(RIG, "eboot-imports.tsv")
NID_DICT = os.path.join(RIG, "nid-candidates.txt")
#: shadPS4's symbol-name database - 94,275 spellings. GPL-2.0, and this project ships a shadPS4
#: fork, so it is licence-compatible (`design/licensing.md`). It takes import resolution from
#: 366/638 to 552/638. Absent, everything still works; fewer imports get names, never wrong ones.
PS4_NAMES = os.environ.get("D1_PS4_NAMES") or os.path.join(
    os.path.expanduser("~"), "dev", "shadPS4-main", "scripts", "ps4_names.txt")
DIAG_TSV = os.path.join(RIG, "diagnostic-catalogue.tsv")
EXECUTED_TSV = os.path.join(RIG, "executed-addrs.tsv")

#: How many call-graph hops a subsystem name may travel from a function that actually logs it.
#: Each hop is a weaker claim; the hop count travels in the artifact so a reader can discount it.
MAX_HOPS = 3


def die(msg: str, code: int = 2):
    print(f"\n*** {msg}", file=sys.stderr)
    sys.exit(code)


def eboot(*args: str) -> str:
    if not os.path.exists(D1_EBOOT):
        die(
            f"{D1_EBOOT} is missing.\n"
            "    cd d1-server && cargo build --release -p d1-eboot\n"
            "  ⚠ use -p: a whole-workspace build and a -p build produce different binaries."
        )
    r = subprocess.run([D1_EBOOT, *args], capture_output=True, text=True)
    if r.returncode == 2:
        die(f"d1-eboot {' '.join(args)} exited 2 - A POSITIVE CONTROL FAILED:\n{r.stdout}{r.stderr}")
    if r.returncode != 0:
        die(f"d1-eboot {' '.join(args)} failed:\n{r.stderr}")
    return r.stdout


def tsv(text: str) -> list[dict]:
    return list(csv.DictReader(text.splitlines(), delimiter="\t"))


# ── the join ───────────────────────────────────────────────────────────────────────────────────


def load_diag() -> tuple[dict[int, str], dict[int, str]]:
    """function vaddr → subsystem, and → the run it was last SEEN in.

    ⚠ `diagnostic-catalogue.md` §2: NOT-SEEN is only evidence for the ~1,540 rows whose evidence
    class is `strong`. The rest are blind by construction (the two-gate trap). This carries the
    SEEN column through verbatim and does not interpret it.
    """
    if not os.path.exists(DIAG_TSV):
        die(f"{DIAG_TSV} is missing - run capture-rig/diagnostic-catalogue.py first")
    subsystem: dict[int, str] = {}
    seen: dict[int, str] = {}
    # ⚠ Skip `#` lines: line 1 of the catalogue is its provenance/freshness stamp (added
    # 2026-08-10). A plain DictReader would adopt that comment as the header and return nothing
    # but None-keyed rows - silently, which is the failure mode the stamp exists to prevent.
    with open(DIAG_TSV, encoding="utf-8") as fh:
        rows = list(csv.DictReader((ln for ln in fh if not ln.startswith("#")), delimiter="\t"))
    for r in rows:
        if not r.get("fn"):
            continue
        fn = int(r["fn"], 16)
        if r.get("subsystem") and fn not in subsystem:
            subsystem[fn] = r["subsystem"]
        if r.get("seen") == "1" and r.get("last_run"):
            seen.setdefault(fn, r["last_run"])
    return subsystem, seen


def build_callers(edges, starts):
    callers: dict[int, list[int]] = collections.defaultdict(list)
    for a, b in edges:
        if b in starts:
            callers[b].append(a)
    return callers


def propagate(seed: dict[int, str], callers, max_hops: int = MAX_HOPS):
    """Spread a name along call edges, one hop at a time.

    THE RULE, and it is deliberately strict: a function inherits a name only when **every**
    name-bearing caller agrees on it. One dissenting caller and the function stays blank. That is
    not a majority vote, because a majority vote on a call graph launders a guess into a number.

    ⚠⚠ **STRICTNESS IS NOT ACCURACY, AND THIS WAS SHIPPED WRONG ONCE.** The first version of this
    file propagated the **full** colon-path subsystem and labelled hop 1 *"`[I]`, strong"*. Then it
    was cross-validated (`crossval` below) and hop 1 came back at **60.8%** - a coin flip with
    extra steps. Callee-direction propagation of the RTTI **class** is worse still, at **20-26%**,
    and is not used at all.

    What survives cross-validation is the **root** subsystem - the text before the first colon:
    **93.0% at hop 1, 89.6% at hop 2, 83.5% at hop 3.** A helper shared across
    `networking:session:election` and `networking:channel_manager` is still unambiguously
    `networking`; it is the leaf component that does not survive a hop.

    ⇒ `subsystem` is **hop 0 only** and is `[C]`. `subsystem_root` is the propagated column, and its
    accuracy is re-measured by a control on every run rather than asserted here.
    """
    out = {f: (s, 0) for f, s in seed.items()}
    for hop in range(1, max_hops + 1):
        new: dict[int, tuple[str, int]] = {}
        for callee, cs in callers.items():
            if callee in out:
                continue
            names = {out[c][0] for c in cs if c in out}
            if len(names) == 1:
                new[callee] = (names.pop(), hop)
        if not new:
            break
        out.update(new)
    return out


def crossval(truth: dict[int, str], callers, hops: int, frac: int = 5, seed_rng: int = 11):
    """Hold out 1/`frac` of the ground truth, propagate, and measure how often it comes back right.

    ATLAS §4.4d says a new tool is not evidence until it reproduces a known result. This is that,
    turned into a number the artifact must keep hitting: the held-out functions have a `[C]` answer
    (they print the string themselves), so a wrong prediction is *measurably* wrong, not arguable.
    """
    keys = sorted(truth)
    rng = random.Random(seed_rng)
    held = set(rng.sample(keys, max(1, len(keys) // frac)))
    seeded = {k: v for k, v in truth.items() if k not in held}
    out = propagate(seeded, callers, hops)
    pred = [k for k in held if k in out]
    ok = sum(1 for k in pred if out[k][0] == truth[k])
    return len(pred), ok, (ok / len(pred) if pred else 0.0)


def load_executed(starts: list[int]) -> dict[int, tuple[str, str]]:
    """Guest addresses ever OBSERVED RUNNING → (kind, evidence) for their containing function.

    Harvested read-only from `runs/` (200 run directories, 2026-08-06..08): 79 probe-fire
    addresses decoded through `decode-account-trace.py`'s own `read_records()`, 15 crash-backtrace
    frames, 11 catalogued-but-never-fired probe sites.

    ⚠ These are **instruction addresses inside** functions, not function starts, so each is mapped
    to its containing function - which is only correct now that the atlas indexes
    frame-pointer-omitted functions too (ATLAS §4.2d). `0x56CA70` is in this set and is exactly one
    of those.

    ⚠ The address space was verified three independent ways rather than assumed: the probe-site
    catalogue states `runtime_load_base 0x800000000`; the trace's `rip` register is consistently
    `0x800000000 + guest_vaddr`; and the C++ tracer stores the bare `guest_vaddr`, adding the base
    only when patching. 105/105 addresses land below `0x18bf0dc`. Nothing was converted.

    ⚠ **This is a floor, not a census.** Only ~90 addresses were ever probed, and 18 pre-2026-08-07
    trace files use a 216-byte record layout no committed decoder understands - their evidence is
    unrecovered rather than absent. `executed` blank means *not observed*, never *not executed*.
    """
    if not os.path.exists(EXECUTED_TSV):
        return {}
    out: dict[int, tuple[str, str]] = {}
    for r in csv.DictReader(open(EXECUTED_TSV), delimiter="\t"):
        va = int(r["addr"], 16)
        i = bisect.bisect_right(starts, va) - 1
        if i < 0:
            continue
        f = starts[i]
        kind = r["kind"]
        prev = out.get(f)
        # probe-fired outranks backtrace outranks probe-armed
        rank = {"probe-fired": 3, "backtrace": 2, "probe-armed": 1}
        if prev is None or rank.get(kind, 0) > rank.get(prev[0], 0):
            out[f] = (kind, r["example_source"])
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--controls", action="store_true", help="run controls only; write nothing")
    ap.add_argument("--addr", help="print everything known about the function containing this vaddr")
    args = ap.parse_args()

    if args.addr:
        print(eboot("atlas", "--addr", args.addr), end="")
        return 0

    # ── gate 0: the crate's own controls ───────────────────────────────────────────────────────
    print(eboot("controls"), end="")

    rows = tsv(eboot("atlas", "--tsv"))
    types = tsv(eboot("rtti", "--tsv"))
    namehash = tsv(eboot("nametab", "--tsv"))
    cg_text = eboot("callgraph")
    cg = tsv(cg_text)
    tc_text = eboot("thiscalls")
    dict_args = ["--dict", NID_DICT]
    if os.path.exists(PS4_NAMES):
        dict_args += ["--dict", PS4_NAMES]
    imp_text = eboot("imports", *dict_args, "--tsv")
    imps = tsv(imp_text)

    starts = {int(r["vaddr"], 16) for r in rows}
    by_va_pre = {int(r["vaddr"], 16): r for r in rows}
    edges = [(int(e["caller"], 16), int(e["callee"], 16)) for e in cg]
    diag_sub, diag_seen = load_diag()

    # Only seed from functions we actually have rows for; a diagnostic attributed to an address
    # that is not a prologue would otherwise inject a phantom node into the graph.
    seed = {f: s for f, s in diag_sub.items() if f in starts}
    callers = build_callers(edges, starts)
    # ⚠ The FULL colon-path is NOT propagated - cross-validation puts it at 60.8% at one hop.
    # Only the ROOT (93.0% at one hop) travels. See propagate()'s docstring.
    root_seed = {f: s.split(":")[0] for f, s in seed.items()}
    root = propagate(root_seed, callers)
    executed = load_executed(sorted(starts))

    for r in rows:
        va = int(r["vaddr"], 16)
        r["subsystem"] = seed.get(va, "")          # [C] - this function prints it
        rs, hop = root.get(va, ("", ""))
        r["subsystem_root"] = rs
        r["subsystem_root_hops"] = str(hop) if rs else ""
        r["seen_run"] = diag_seen.get(va, "")
        k, src = executed.get(va, ("", ""))
        r["executed"] = k
        r["executed_evidence"] = src

    # ── controls on the join itself ────────────────────────────────────────────────────────────
    # Every one of these is a reading made by hand, in another session, by another method. A
    # control that restates this script's own output is not a control (ATLAS §4.2 / §4.4d).
    checks: list[tuple[str, bool, str]] = []
    by_va = by_va_pre

    pro = [r for r in rows if r["anchored"] == "prologue"]
    fpo = [r for r in rows if r["anchored"] == "call-target"]
    checks.append(("35,834 prologue-anchored rows (the pinned denominator)", len(pro) == 35834, str(len(pro))))
    checks.append(("frame-pointer-omitted rows present and separated", len(fpo) > 10000, str(len(fpo))))
    # ⚠ ATLAS §4.4c's hot leaf, §4.1's registrar and §8.4's accessor are all frame-pointer-omitted.
    # If any of them ever falls back into "inside the previous function", this control catches it.
    for va, why in ((0x56CA70, "ATLAS §4.4c hot leaf"), (0xA8C920, "THE-CHAIN §4.1 registrar"),
                    (0x260C10, "THE-CHAIN §8.4 accessor")):
        r = by_va.get(va)
        checks.append((f"{va:#x} is its own function row ({why})",
                       bool(r) and r["anchored"] == "call-target", (r or {}).get("name", "<no row>")))
    checks.append(("2,633 RTTI class rows", len(types) == 2633, str(len(types))))
    checks.append(
        ("call graph carries jmp edges too (ATLAS §4.2)",
         any(e["kind"] == "jmp" for e in cg) and (0x560D27, 0x56FFB0) not in [],
         f"{sum(1 for e in cg if e['kind'] == 'jmp')} jmp edges"))
    # the mandatory tail-call control, in the graph this time
    fn_of_560d27 = None
    for va in sorted(starts):
        if va <= 0x560D27:
            fn_of_560d27 = va
        else:
            break
    checks.append(
        ("tail-call edge 0x560d27 -> 0x56ffb0 present",
         (fn_of_560d27, 0x56FFB0) in set(edges), f"from fn {fn_of_560d27:#x}"))

    # ⭐ Runtime evidence. `0x56CA70` is frame-pointer-omitted AND probe-fired, so it is the one
    # address that fails if either the FPO indexing or the executed join regresses.
    ex = [r for r in rows if r["executed"]]
    # ⚠ The floor is 35 because the MEASURED value is 40, and the first version of this control
    # guessed 60 and failed. 105 observed addresses collapse to 40 functions - probe sites cluster
    # many-to-one inside a function by design (six probes on one chain, `eboot-map` §4.4). Editing a
    # failing constant is ATLAS §6.1's failure mode, so this one says where its number came from.
    checks.append(("runtime evidence joins to >= 35 functions (measured 40 from 105 addresses)",
                   len(ex) >= 35, str(len(ex))))
    r = by_va.get(0x56CA70)
    checks.append(("0x56ca70 (FPO + probe-fired) carries runtime evidence",
                   bool(r) and r["executed"] == "probe-fired", (r or {}).get("executed", "<no row>")))
    unmapped = sum(1 for r in rows if r["executed"] and r["name_source"] == "none"
                   and not r["subsystem_root"])
    checks.append(("every function OBSERVED RUNNING is worth naming - report how many are not",
                   True, f"{unmapped} executed-but-unattributed"))

    # ⭐ Library imports - the biggest lever after RTTI (roadmap §2.1). The NID hash is one-way, so
    # a resolution is either an exact hash match or nothing: there is no "probably". What needs
    # asserting is that the pipeline is wired at all, and that a known answer comes back.
    named_imports = [i for i in imps if i["name"]]
    checks.append(("all 638 PLT stubs located", sum(1 for i in imps if i["plt_stub"]) == 638,
                   f"{sum(1 for i in imps if i['plt_stub'])}/{len(imps)}"))
    floor = 500 if os.path.exists(PS4_NAMES) else 350
    checks.append((f"NID dictionary resolves >= {floor} of the 638 imports"
                   f"{' (with shadPS4 ps4_names.txt)' if floor == 500 else ' (hand list only)'}",
                   len(named_imports) >= floor, f"{len(named_imports)}/638"))
    memcpy = [i for i in imps if i["name"] == "memcpy"]
    checks.append(("import NID Q3VBxCXhUHs resolves to memcpy at PLT stub 0x129d080",
                   bool(memcpy) and memcpy[0]["plt_stub"] == "0x129d080",
                   memcpy[0]["nid"] if memcpy else "<unresolved>"))
    # THE-CHAIN §2.2's shadPS4 defect was in sceVoiceCreatePort. The atlas must be able to find
    # the guest side of it, which is the whole point of naming imports.
    voice = [r for r in rows if "sceVoice" in r["imports"]]
    checks.append(("guest callers of sceVoice* are locatable (THE-CHAIN §2.2's defect)",
                   len(voice) > 0, f"{len(voice)} functions"))

    # ⭐ Jump-table recovery: eboot-map §2.2 read the 5-entry table at 0x79FEB8 by hand. If the
    # decoder still stopped at the table, this function would end early and lose its tail.
    r = by_va.get(0x79FAE0)
    checks.append(("0x79fae0 decodes past its embedded jump table at 0x79feb8 (eboot-map §2.2)",
                   bool(r) and int(r["end"], 16) > 0x79FEB8 and "decode-dirty" not in r["warn"],
                   f"end {(r or {}).get('end','?')}"))
    dirty = sum(1 for r in rows if "decode-dirty" in r["warn"])
    checks.append(("fewer than 1,500 dirty decodes image-wide (was 2,261 before jump-table recovery)",
                   dirty < 1500, str(dirty)))

    # eboot-map §3.0b named `0x573C80` as investment_activity_selection_manager from a nearby
    # string; the atlas must agree, and must reach it independently.
    r = by_va.get(0x573C80)
    checks.append(
        ("0x573c80 is attributed to investment_activity_selection_manager (eboot-map §3.0b)",
         bool(r) and "investment_activity_selection_manager" in (r["strings"] + r["name"]),
         (r or {}).get("name", "<no row>")))

    # eboot-map §7 read `c_world_controller_state_bootflow_profile_setup_handler` off a string at
    # 0x15C1B44 by hand; RTTI must produce the same class name.
    tnames = {t["name"] for t in types}
    checks.append(
        ("RTTI produces c_world_controller_state_bootflow_profile_setup_handler (eboot-map §7)",
         "c_world_controller_state_bootflow_profile_setup_handler" in tnames, ""))
    checks.append(
        ("RTTI produces cui::c_bootflow_screen_bounds_event_handler_component (eboot-map §7)",
         "cui::c_bootflow_screen_bounds_event_handler_component" in tnames, ""))
    # THE-CHAIN §6.4: the activity-launch message classes, named from RTTI in another session.
    for want in (
        "c_investment_bap_message_client_to_bap_get_activity_host_proxy_request",
        "c_investment_bap_message_client_to_world_server_request",
    ):
        checks.append((f"RTTI produces {want} (THE-CHAIN §6.4)", want in tnames, ""))

    # The name-hash tables must reproduce the four names THE-CHAIN §4.1 decoded by hand.
    nh = {(int(e["index"]), e["name"]) for e in namehash if e["table"] == "0x274da80"}
    checks.append(
        ("name-hash table 0x274da80 decodes index 2 -> platform_signin, 3 -> account_signin",
         (2, "platform_signin") in nh and (3, "account_signin") in nh,
         f"{len(nh)} entries"))

    # The subsystem propagation must not have invented a subsystem for a function that logs a
    # DIFFERENT one. Zero contradictions is the only acceptable answer for a rule this strict.
    bad = [f"{f:#x}" for f, s in seed.items() if root.get(f, ("", 0))[0] != s.split(":")[0]]
    checks.append(("propagation never overrides a function's own logged subsystem", not bad, " ".join(bad[:5])))

    # ⭐ THE ACCURACY CONTROL. Measured, every run - not asserted in a doc that goes stale.
    # If a future edit loosens the unanimity rule or raises MAX_HOPS, this is what catches it.
    for hops, floor in ((1, 0.85), (3, 0.75)):
        n, ok, acc = crossval(root_seed, callers, hops)
        checks.append((f"root-subsystem propagation is >= {floor:.0%} accurate at {hops} hop(s) "
                       f"(held-out cross-validation)", acc >= floor, f"{ok}/{n} = {acc:.1%}"))
    # And the negative result, asserted so nobody re-adds it: the FULL colon-path does not survive
    # a hop, and shipping it as "strong" is exactly what this control exists to prevent.
    n, ok, acc = crossval(seed, callers, 1)
    checks.append(("full colon-path subsystem is measurably WEAK at 1 hop (<80%) - it is not "
                   "propagated, and this asserts why", acc < 0.80, f"{ok}/{n} = {acc:.1%}"))

    named = sum(1 for r in pro if r["name_source"] != "none")
    attributed = sum(1 for r in pro if r["name_source"] != "none" or r["subsystem_root"])
    checks.append(("coverage strictly above the 500 hand-curated names", named > 500, str(named)))

    print()
    ok = True
    for name, passed, detail in checks:
        ok &= passed
        print(f"{'PASS' if passed else 'FAIL'}  {name}{f'  [{detail}]' if detail else ''}")
    if not ok:
        die("A JOIN CONTROL FAILED - the artifact was NOT written and may not be cited")

    # ── coverage ───────────────────────────────────────────────────────────────────────────────
    by_source = collections.Counter(r["name_source"] for r in pro)
    hops = collections.Counter(r["subsystem_root_hops"] for r in pro if r["subsystem_root"])
    n = len(pro)
    print(f"\nCOVERAGE of {n} PROLOGUE-ANCHORED functions - the denominator every published")
    print("percentage in design/ is stated against.")
    print(f"  named            {named:6}  ({100*named/n:.1f}%)")
    for k, v in by_source.most_common():
        if k != "none":
            print(f"      {k:<14} {v:6}")
    print(f"  + subsystem_root {sum(1 for r in pro if r['subsystem_root']):6}"
          f"   (hop 0 {hops.get('0',0)} · 1 {hops.get('1',0)} · 2 {hops.get('2',0)} · 3 {hops.get('3',0)})")
    print(f"  ATTRIBUTED       {attributed:6}  ({100*attributed/n:.1f}%)")
    print(f"  unattributed     {n-attributed:6}  ({100*(n-attributed)/n:.1f}%)")
    exk = collections.Counter(r["executed"] for r in rows if r["executed"])
    print(f"\n  OBSERVED RUNNING {sum(exk.values()):6}  "
          f"(probe-fired {exk.get('probe-fired',0)} · backtrace {exk.get('backtrace',0)} · "
          f"armed-only {exk.get('probe-armed',0)})  - a floor, not a census")
    fnamed = sum(1 for r in fpo if r["name_source"] != "none" or r["subsystem_root"])
    print(f"\n⚠ plus {len(fpo)} frame-pointer-omitted functions ({fnamed} attributed), which the")
    print("  denominator does not contain. The image has at least "
          f"{n + len(fpo)} functions, not {n}.")

    if args.controls:
        print("\n--controls: nothing written")
        return 0

    cols = list(rows[0].keys())
    with open(ATLAS_TSV, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols, delimiter="\t", lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    with open(TYPES_TSV, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(types[0].keys()), delimiter="\t", lineterminator="\n")
        w.writeheader()
        w.writerows(types)
    with open(NAMEHASH_TSV, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(namehash[0].keys()), delimiter="\t", lineterminator="\n")
        w.writeheader()
        w.writerows(namehash)
    with open(CALLGRAPH_TSV, "w") as fh:
        fh.write(cg_text)
    with open(THISCALLS_TSV, "w") as fh:
        fh.write(tc_text)
    with open(IMPORTS_TSV, "w") as fh:
        fh.write(imp_text)

    for p in (ATLAS_TSV, TYPES_TSV, NAMEHASH_TSV, CALLGRAPH_TSV, THISCALLS_TSV, IMPORTS_TSV):
        print(f"  wrote {os.path.relpath(p, ROOT)}  ({os.path.getsize(p)/1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
