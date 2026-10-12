#!/usr/bin/env python3
"""
verify-tag-determinism.py - settle "are runtime schema tags assigned at load, or
compile-time constants?" WITHOUT an emulator run.

THE QUESTION, AND WHY THE OBVIOUS MEASUREMENT IS VACUOUS
--------------------------------------------------------
The obvious test is "compare the in-file word at each .bss slot against the live
value".  That test cannot be run, and the reason is itself the first half of the
answer: every one of the 7670 registry slots lies in the NOBITS part of LOAD
segment 1 (vaddr 0x18C0000, filesz 0x149088, memsz 0x1690DC0 -> NOBITS region
0x1A09088..0x2F50DC0).  There are no file bytes there at all, so the comparison
is trivially "all 7670 differ" and says nothing.  Check A below proves that.

THE TEST THAT DOES DECIDE IT
----------------------------
Replay the tag allocator offline from the file alone and compare against the
LIVE-measured tags in capture-rig/namespace-map.tsv (captured 2026-08-07,
pid 3382362, base 0x800000000; all five of that run's controls passed).

    tag = 0x80000000 | ((container_id << 13) & 0x7FFE000) | (index & 0x1FFF)

read out of 0x6E9B0 here rather than trusted from any document:

    0x06e9d9  418b4634       mov  eax, [r14 + 0x34]      ; container id word
    0x06e9df  c1e30d         shl  ebx, 0xd               ; << 13
    0x06e9e2  81e1ff1f0000   and  ecx, 0x1fff            ; index & 0x1FFF
    0x06e9ef  81e300e0ff07   and  ebx, 0x7ffe000
    0x06e9f5  09cb           or   ebx, ecx
    0x06e9f7  81cb00000080   or   ebx, 0x80000000

Both inputs are static: container_id is a hardcoded immediate (0x400/0x401/0x402
at 0xE9791/0xE97B1/0xE97D1, stored as 0x40000000|id into [container+0x34] by
0x70C60), and index is a u16 in a static record of the element list at 0x129F950,
walked once by binder 0xEE100 called from 0x1154.  Nothing runtime-varying feeds
the formula.

CONTROLS (mandatory - a match rate means nothing without them)
-------------------------------------------------------------
  * scramble control: shifting the offline index by +/-1 must collapse the match
    rate to ~0.  A join that matches under any shift is a degenerate join.
  * the three standing positive controls (0x80801825 -> 0x1324020,
    0x808033F4 -> 0x1492550) and the negative control (0xDEADBEEF -> None) are
    re-checked by type-registry.py --verify; run that first.

Usage:  ./verify-tag-determinism.py [--eboot PATH] [--map PATH] [--offline PATH]
Exit 0 only if every check passes.
"""

import argparse
import csv
import os
import struct
import subprocess
import sys
import tempfile
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
EBOOT = os.path.join(ROOT, "dumps/patch-01.29-CUSA00219/eboot.bin")
LIVE_MAP = os.path.join(HERE, "namespace-map.tsv")
TYPE_REGISTRY = os.path.join(HERE, "type-registry.py")

# live registry name -> the schema module that backs it (type-registry.py naming)
MODULE_OF_REGISTRY = {"A": "spasaha", "B": "spasaha_investment"}

# the two standing anchors, re-checked here as well
ANCHORS = {0x80801825: 0x1324020, 0x808033F4: 0x1492550}

_fail = []


def check(label, ok, detail=""):
    print("  [%s] %-58s %s" % ("PASS" if ok else "FAIL", label, detail))
    if not ok:
        _fail.append(label)
    return ok


def load_segments(blob):
    """Return the PT_LOAD segments as (vaddr, filesz, memsz)."""
    e_phoff = struct.unpack_from("<Q", blob, 0x20)[0]
    e_phentsize = struct.unpack_from("<H", blob, 0x36)[0]
    e_phnum = struct.unpack_from("<H", blob, 0x38)[0]
    out = []
    for i in range(e_phnum):
        o = e_phoff + i * e_phentsize
        p_type = struct.unpack_from("<I", blob, o)[0]
        _off, vaddr, _pa, filesz, memsz, _al = struct.unpack_from("<QQQQQQ", blob, o + 8)
        if p_type == 1:
            out.append((vaddr, filesz, memsz))
    return out


def residency(segs, vaddr):
    for base, filesz, memsz in segs:
        if base <= vaddr < base + memsz:
            return "FILE" if vaddr < base + filesz else "NOBITS"
    return "OUTSIDE"


def offline_dump(eboot, path):
    subprocess.run([sys.executable, TYPE_REGISTRY, "--eboot", eboot, "--dump", path],
                   check=True, stdout=subprocess.DEVNULL)
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    by_module = {}
    for e in rows:
        for m in e["modules"].split(";"):
            mod, rest = m.split("#")
            idx, slot = rest.split("@")
            by_module.setdefault(mod, {})[int(idx)] = (e, int(slot, 16))
    return rows, by_module


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eboot", default=EBOOT)
    ap.add_argument("--map", default=LIVE_MAP)
    ap.add_argument("--offline", default=None)
    a = ap.parse_args()

    blob = open(a.eboot, "rb").read()
    segs = load_segments(blob)
    live = list(csv.DictReader(open(a.map), delimiter="\t"))

    tmp = a.offline or os.path.join(tempfile.mkdtemp(), "offline-registry.tsv")
    offline, by_module = offline_dump(a.eboot, tmp)

    print("eboot   %s" % a.eboot)
    print("live    %s (%d rows)" % (a.map, len(live)))
    print("offline %s (%d elements)\n" % (tmp, len(offline)))

    # ---- A. the proposed measurement is vacuous: the slots have no file bytes
    print("== A. do the registry slots exist in the file at all? ==")
    res = Counter(residency(segs, int(r["bss_slot_vaddr"], 16)) for r in live)
    check("every registry slot is NOBITS (no in-file word to compare)",
          res.get("NOBITS") == len(live) and len(res) == 1, dict(res))
    lo = min(int(r["bss_slot_vaddr"], 16) for r in live)
    hi = max(int(r["bss_slot_vaddr"], 16) for r in live)
    nb = [(b + f, b + m) for b, f, m in segs if b <= lo < b + m]
    check("slot range sits inside the NOBITS window",
          bool(nb) and nb[0][0] <= lo and hi < nb[0][1],
          "slots 0x%x..0x%x  NOBITS 0x%x..0x%x" % (lo, hi, nb[0][0], nb[0][1]) if nb else "")
    check("CONTROL: a known FILE-resident address is classified FILE",
          residency(segs, 0x1492550) == "FILE", "0x1492550 (span element descriptor)")

    # ---- B. the formula, applied to the live tags themselves
    print("\n== B. does the live tag decompose by 0x6E9B0's arithmetic? ==")
    bad = sum(1 for r in live
              if int(r["runtime_tag"], 16) !=
              (0x80000000 | ((int(r["tag_slot"], 16) << 13) & 0x7FFE000) | (int(r["tag_index"]) & 0x1FFF)))
    check("tag == 0x80000000|((slot<<13)&0x7FFE000)|(idx&0x1FFF)",
          bad == 0, "%d/%d hold" % (len(live) - bad, len(live)))

    # ---- C. THE DECISIVE JOIN: offline prediction vs live measurement
    print("\n== C. offline replay vs live measurement, joined on .bss slot ==")

    def join(shift):
        m = d = skipped = 0
        for r in live:
            hit = by_module[MODULE_OF_REGISTRY[r["registry"]]].get(int(r["index"]) + shift)
            if hit is None:
                skipped += 1
                continue
            e, slot = hit
            if shift == 0 and slot != int(r["bss_slot_vaddr"], 16):
                skipped += 1
                continue
            if int(e["tag"], 16) == int(r["runtime_tag"], 16):
                m += 1
            else:
                d += 1
        return m, d, skipped

    m0, d0, sk = join(0)
    check("every live row joins an offline element by .bss slot", sk == 0, "%d unjoined" % sk)
    check("RUNTIME TAG offline-predicted == live-measured",
          d0 == 0 and m0 == len(live), "%d match / %d differ" % (m0, d0))

    om = od = hm = hd = 0
    for r in live:
        e, _ = by_module[MODULE_OF_REGISTRY[r["registry"]]][int(r["index"])]
        om, od = (om + 1, od) if int(e["object"], 16) == int(r["object_vaddr"], 16) else (om, od + 1)
        hm, hd = (hm + 1, hd) if int(e["name_hash"], 16) == int(r["name_hash"], 16) else (hm, hd + 1)
    check("OBJECT vaddr offline == live", od == 0, "%d match / %d differ" % (om, od))
    check("NAME HASH offline == live", hd == 0, "%d match / %d differ" % (hm, hd))

    print("\n== D. scramble control - the join must NOT survive a shift ==")
    for shift in (1, -1, 7):
        ms, ds, _ = join(shift)
        check("shift %+d collapses the match rate" % shift, ms == 0,
              "%d match / %d differ" % (ms, ds))

    # ---- E. the tag is computed, never stored
    print("\n== E. is the tag value stored anywhere in the file? ==")

    def occurrences(word):
        pat = struct.pack("<I", word)
        n, i = 0, blob.find(pat)
        while i >= 0:
            n += 1
            i = blob.find(pat, i + 1)
        return n

    check("span element tag 0x808033F4 occurs 0 times as a word",
          occurrences(0x808033F4) == 0, "explains why the static search in §26.27 found nothing")
    check("CONTROL: 0xDEADBEEF does occur, so the scanner works",
          occurrences(0xDEADBEEF) > 0, "%d occurrences" % occurrences(0xDEADBEEF))

    # ---- F. the anchors, from the offline side alone
    print("\n== F. standing anchors, reproduced from the file alone ==")
    by_tag = {int(e["tag"], 16): e for e in offline}
    for tag, obj in ANCHORS.items():
        e = by_tag.get(tag)
        check("tag 0x%08X -> object 0x%07X" % (tag, obj),
              e is not None and int(e["object"], 16) == obj,
              e["object"] if e else "no such tag")
    check("CONTROL: bogus tag 0x8080FFFF resolves to nothing",
          0x8080FFFF not in by_tag)

    print("\n== G. container histogram ==")
    print("  offline, all %d elements : %s" % (len(offline), dict(Counter(e["slot_id"] for e in offline))))
    print("  live, the %d registry entries: %s" % (len(live), dict(Counter(r["tag_slot"] for r in live))))

    print()
    if _fail:
        print("FAILED: %s" % ", ".join(_fail))
        return 1
    print("ALL CHECKS PASSED - the runtime tag is a deterministic function of static")
    print("file data (not stored in the file, and not runtime-varying either).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
