#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Enumerate the 247-row progression definition table AND the unlock flags each step grants.

⭐ THE FINDING THIS EXISTS FOR: `0x28C9A0` walks a character's progressions and, for every step
already reached, does `mov byte [plane + flag_index], 2` - where `flag_index` is the u16 at
step_element + 4, the field `historicaldocs/design/progression-table.md` §7 guessed was "a localisation index".
It is not. IT IS AN UNLOCK FLAG INDEX. Progressions are a GRANT CHANNEL into the same availability
plane rung 33 reads.

    0x0028ca40  mov   rsi, [rax]                    ; rax = def+0x10, self-relative step ptr
    0x0028ca43  add   rsi, rax                      ; step array base
    0x0028ca46  movsx rsi, word [rsi+rdx*8+0x14]    ; step[i].second_u32 low 16 bits
    0x0028ca4c  cmp   rsi, -1                       ; 0xFFFF = grants nothing
    0x0028ca52  mov   byte [r12+rsi], 2             ⭐ THE GRANT

Usage:
    PYTHONPATH=~/.local/lib/d1-tools python3 -B capture-rig/progression-grants.py --controls
    PYTHONPATH=~/.local/lib/d1-tools python3 -B capture-rig/progression-grants.py --tsv out.tsv
"""

import argparse
import importlib.util
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

PKG = "ps4_investment_globals_012f_3"
PROG_BLOCK, PROG_OFF = 1066, 145616      # progression definition array, 247 x 56
STAT_BLOCK, STAT_OFF = 515, 172176       # stat definition array, 36 x 16
ELEM = 56


def _ct():
    spec = importlib.util.spec_from_file_location(
        "content_table", os.path.join(HERE, "oodle/content-table.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def parse(d, arr, elem=ELEM):
    """Return (count, [record dicts]) for the definition array at block offset `arr`."""
    total, count = struct.unpack_from("<QQ", d, arr)
    base = arr + 0x30
    out = []
    for i in range(count):
        r = base + i * elem
        h, scope, flag, _pad = struct.unpack_from("<IBBH", d, r)
        steps, reloff = struct.unpack_from("<Qq", d, r + 8)
        rec = dict(ordinal=i, hash=h, scope=scope, flag=flag, steps=steps,
                   step_off=r + 0x10 + reloff, grants=[], costs=[])
        # step array header {u64 count, u32 class, u32 0} then count x {u32 to_advance, u32 x}
        sa = rec["step_off"]
        if 0 <= sa < len(d) - 16:
            scount = struct.unpack_from("<Q", d, sa)[0]
            rec["step_count_hdr"] = scount
            n = min(scount, steps)
            for s in range(n):
                e = sa + 0x10 + s * 8
                if e + 8 > len(d):
                    break
                adv, second = struct.unpack_from("<II", d, e)
                rec["costs"].append(adv)
                rec["grants"].append(second & 0xFFFF)
        out.append(rec)
    return count, out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--controls", action="store_true")
    ap.add_argument("--tsv")
    ap.add_argument("--find-flag", type=lambda x: int(x, 0), action="append", default=[])
    a = ap.parse_args()

    ct = _ct()
    d = ct.block_plain(PKG, PROG_BLOCK)
    count, recs = parse(d, PROG_OFF)

    if a.controls:
        fails = []
        def chk(name, ok, detail=""):
            print(("PASS  " if ok else "FAIL  ") + name + (("\n        " + detail) if detail else ""))
            if not ok:
                fails.append(name)
        chk("247 records", count == 247, f"count={count}")
        cl = recs[117]
        chk("ordinal 117 is character_level, 40 steps",
            cl["hash"] == 0x6650C0F9 and cl["steps"] == 40,
            f"hash=0x{cl['hash']:08X} steps={cl['steps']}")
        chk("character_level's 40 steps grant exactly the 40 flags 2531..2570 (permuted, no sentinel)",
            sorted(cl["grants"]) == list(range(2531, 2571)),
            f"in step order {cl['grants'][:4]} ... {cl['grants'][-2:]}")
        chk("character_level costs match the published D1 XP curve",
            cl["costs"][:8] == [0, 900, 4100, 5000, 6000, 7000, 8000, 9000]
            and cl["costs"][-3:] == [10000, 15000, 20000],
            f"{cl['costs'][:8]} ... {cl['costs'][-3:]}")
        # NEGATIVE: a progression with zero steps must grant nothing
        z = [r for r in recs if r["steps"] == 0]
        chk("zero-step progressions grant nothing (negative control)",
            len(z) > 0 and all(r["grants"] == [] for r in z),
            f"{len(z)} zero-step rows, all empty")
        # NEGATIVE: 0xFFFF sentinel really occurs, i.e. not every step grants
        anyff = sum(1 for r in recs for g in r["grants"] if g == 0xFFFF)
        chk("the 0xFFFF 'grants nothing' sentinel is present (discriminator)",
            anyff > 0, f"{anyff} steps grant nothing")
        print("\n" + ("ALL CONTROLS PASS" if not fails else f"{len(fails)} CONTROL(S) FAILED"))
        sys.exit(2 if fails else 0)

    if a.find_flag:
        want = set(a.find_flag)
        for r in recs:
            hits = [(i, g) for i, g in enumerate(r["grants"]) if g in want]
            if hits:
                print(f"ordinal {r['ordinal']:3d} hash 0x{r['hash']:08X} scope {r['scope']} "
                      f"steps {r['steps']}: {hits}")
        return

    rows = []
    for r in recs:
        gs = [g for g in r["grants"] if g != 0xFFFF]
        rows.append((r["ordinal"], f"0x{r['hash']:08X}", r["scope"], r["flag"], r["steps"],
                     len(gs), (min(gs) if gs else ""), (max(gs) if gs else ""),
                     r["costs"][-1] if r["costs"] else "",
                     sum(r["costs"]) if r["costs"] else ""))
    hdr = ["ordinal", "hash", "scope", "flag", "steps", "granting_steps",
           "flag_lo", "flag_hi", "last_cost", "total_cost"]
    out = open(a.tsv, "w") if a.tsv else sys.stdout
    print("\t".join(hdr), file=out)
    for row in rows:
        print("\t".join(str(x) for x in row), file=out)
    if a.tsv:
        out.close()
        print(f"wrote {a.tsv}: {len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
