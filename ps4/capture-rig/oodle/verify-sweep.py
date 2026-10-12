#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Sweep the Oodle decoder over many blocks and score it against the container's own evidence.

This is the answer to "does the lifted decoder actually decode *correctly*?" - a question the block
records cannot settle, because their SHA-1 covers the **stored** bytes (verified: 340/340 on the
first package tried, corpus-wide via `d1-pkg verify --blocks`). Nothing in the container stores a
digest of decompressed output.

What does settle it: most payload classes open with a `u64` length, and the **entry table** declares
that same length in a separate structure the decompressor never reads. Agreement is independent
evidence, and a chance match on one entry has probability ~2^-64.

Two classes legitimately have no length header and are expected to disagree - raw blobs
(`class_tag == 0xffffffff`) and the fixed 60-byte descriptor records that reference them. They are
reported, never hidden.

Usage:
    ./verify-sweep.py                          # default spread across investment packages
    ./verify-sweep.py --pkg ps4_orbit_01a9_3.pkg --blocks 8
"""

import argparse
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
D1PKG = os.path.join(ROOT, "d1-server", "target", "release", "d1-pkg")
PKGDIR = os.path.join(ROOT, "dumps", "patch-01.29-CUSA00219", "packages")

DEFAULT_PKGS = [
    "ps4_investment_globals_012c_1.pkg",
    "ps4_investment_globals_012e_2.pkg",   # holds the message-503 argument schema
    "ps4_globals_pvp_0169_3.pkg",
    "ps4_orbit_01a9_3.pkg",
]

sys.path.insert(0, HERE)
from importlib.machinery import SourceFileLoader  # noqa: E402
od = SourceFileLoader("oodle_decompress", os.path.join(HERE, "oodle-decompress.py")).load_module()


def run(*a):
    r = subprocess.run(a, capture_output=True, text=True)
    return r.returncode, r.stdout, r.stderr


def blocks_of(pkg, want, scratch):
    """Pick `want` compressed blocks spread across the package."""
    rc, out, err = run(D1PKG, "--dir", PKGDIR, "blocks", pkg, "--compressed-only", "--json")
    if rc != 0:
        print(f"  cannot list blocks: {err.strip()}", file=sys.stderr)
        return []
    rows = [r["block"] for r in json.loads(out) if r["stored_sha1_ok"]]
    if not rows:
        return []
    step = max(1, len(rows) // want)
    return rows[::step][:want]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--pkg", action="append", help="package to sweep (repeatable)")
    ap.add_argument("--blocks", type=int, default=4, help="blocks per package")
    ap.add_argument("--scratch", default="/tmp/d1-oodle-sweep")
    args = ap.parse_args()

    os.makedirs(args.scratch, exist_ok=True)
    pkgs = args.pkg or DEFAULT_PKGS
    tot_e = tot_a = tot_raw = 0
    blocks_ok = blocks_bad = 0
    t0 = time.time()

    for pkg in pkgs:
        print(f"\n=== {pkg} ===")
        for b in blocks_of(pkg, args.blocks, args.scratch):
            idx = b["index"]
            z = os.path.join(args.scratch, f"{pkg}.{idx}.z")
            p = os.path.join(args.scratch, f"{pkg}.{idx}.bin")
            rc, out, err = run(D1PKG, "--dir", PKGDIR, "extract-block", pkg, str(idx), "-o", z)
            if rc != 0:
                print(f"  blk {idx}: extract failed: {err.strip()}")
                blocks_bad += 1
                continue

            stored = open(z, "rb").read()
            try:
                plain, consumed, chunks = od.decompress(stored)
            except Exception as e:                       # noqa: BLE001 - one bad block is data
                print(f"  blk {idx}: DECODE RAISED {type(e).__name__}: {e}")
                blocks_bad += 1
                continue
            open(p, "wb").write(plain)

            full = consumed == len(stored)   # blocks decompress to *up to* 0x40000, not always exactly
            rc, out, err = run(D1PKG, "--dir", PKGDIR, "verify-plain", pkg, str(idx),
                               "--plain", p, "--json")
            if not out.strip():
                print(f"  blk {idx}: verify-plain produced nothing: {err.strip()}")
                blocks_bad += 1
                continue
            j = json.loads(out)
            tot_e += j["headered"]
            tot_a += j["agree"]
            tot_raw += j["raw_blob"] + j["self_classed"] + j["too_small"]
            good = full and j["agree"] == j["headered"]
            blocks_ok += good
            blocks_bad += not good
            print(f"  blk {idx:<5} stored {len(stored):<7} -> {len(plain):<7} "
                  f"chunks {chunks:<3} consumed {consumed}/{len(stored)}  "
                  f"headered {j['headered']:<5} agree {j['agree']:<5} "
                  f"unscorable {j['raw_blob']+j['self_classed']+j['too_small']:<4} "
                  f"{'OK' if good else 'SUSPECT'}")

    print(f"\n{'='*72}")
    print(f"blocks: {blocks_ok} ok, {blocks_bad} suspect   ({time.time()-t0:.1f}s)")
    if tot_e:
        print(f"headered entries scored: {tot_e}   self-length agreements: {tot_a} "
              f"({tot_a*100.0/tot_e:.2f}%)")
    else:
        print("no headered entries scored")
    print(f"unscorable (raw blobs / self-classed / too small): {tot_raw}")
    return 0 if blocks_bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
