#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Resolve a content **tag** (or package+index) to its decompressed bytes.

This is the tool the content lane actually needs: given `0x80AA2840`, hand back the payload. It
combines the two halves that are deliberately kept separate -

* `d1-server/crates/d1-pkg` - the container format (the ONLY description of it), and
* `oodle-decompress.py` - the client's own decoder under emulation.

Handles the three things that make this non-trivial:

1. **Generation chain.** A block record names the generation whose *file* holds its bytes, which is
   often an older package than the one you asked about.
2. **Block spanning.** An entry can start inside a block and continue into the next one, so the
   payload is a concatenation. Verified by the container's own self-length field.
3. **Short blocks.** A block decompresses to *at most* 0x40000; decoding must stop on input
   exhaustion, not on a target size.

Usage:
    ./extract-entry.py 0x80AA2840 -o out.bin
    ./extract-entry.py --pkg ps4_globals_0151_3.pkg --index 2112 -o out.bin
"""

import argparse
import json
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
D1PKG = os.path.join(ROOT, "d1-server", "target", "release", "d1-pkg")
PKGDIR = os.path.join(ROOT, "dumps", "patch-01.29-CUSA00219", "packages")
CACHE = os.environ.get("D1_PLAIN_CACHE", "/tmp/d1-plain-cache")

from importlib.machinery import SourceFileLoader  # noqa: E402
od = SourceFileLoader("oodle_decompress", os.path.join(HERE, "oodle-decompress.py")).load_module()


def run(*a):
    r = subprocess.run(a, capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"d1-pkg failed: {' '.join(a)}\n{r.stderr.strip()}")
    return r.stdout


def tag_parts(tag):
    return ((tag >> 13) & 0x3FFF) - 0x400, tag & 0x1FFF


def newest_package_for_id(pkg_id):
    """Highest generation of the package with this id, at locale 0."""
    best = None
    for name in sorted(os.listdir(PKGDIR)):
        if not name.endswith(".pkg"):
            continue
        try:
            info = run(D1PKG, "--dir", PKGDIR, "info", name)
        except SystemExit:
            continue
        ids = [l for l in info.splitlines() if " id " in l]
        if not ids:
            continue
        f = ids[0].split()
        pid = int(f[f.index("id") + 1])
        gen = int(f[f.index("gen") + 1])
        loc = int(f[f.index("locale") + 1])
        if pid == pkg_id and loc == 0 and (best is None or gen > best[1]):
            best = (name, gen)
    return best[0] if best else None


def plain_block(pkg, index):
    """Decompressed bytes of one block, cached on disk (decoding is slow)."""
    os.makedirs(CACHE, exist_ok=True)
    p = os.path.join(CACHE, f"{pkg}.{index}.bin")
    if os.path.exists(p):
        return open(p, "rb").read()
    z = os.path.join(CACHE, f"{pkg}.{index}.z")
    out = run(D1PKG, "--dir", PKGDIR, "extract-block", pkg, str(index), "-o", z)
    stored = open(z, "rb").read()
    if "Oodle-compressed" in out:
        data, consumed, _ = od.decompress(stored)
        if consumed != len(stored):
            raise SystemExit(f"block {index}: decode consumed {consumed}/{len(stored)}")
    else:
        data = stored
    open(p, "wb").write(data)
    os.unlink(z)
    return data


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("tag", nargs="?", help="content tag, e.g. 0x80AA2840")
    ap.add_argument("--pkg", help="package name (instead of a tag)")
    ap.add_argument("--index", type=int, help="entry index (with --pkg)")
    ap.add_argument("-o", "--out", help="write payload here")
    ap.add_argument("--hex", type=int, default=64, help="bytes of hex to print")
    args = ap.parse_args()

    if args.tag:
        tag = int(args.tag, 0)
        pkg_id, index = tag_parts(tag)
        pkg = newest_package_for_id(pkg_id)
        if not pkg:
            raise SystemExit(f"no package with id {pkg_id} (tag {tag:#x})")
        print(f"tag {tag:#010x} -> package id {pkg_id} ({pkg}), entry {index}")
    else:
        if not args.pkg or args.index is None:
            raise SystemExit("give a tag, or --pkg with --index")
        pkg, index = args.pkg, args.index

    ents = json.loads(run(D1PKG, "--dir", PKGDIR, "entries", pkg, "--json"))
    e = next((x for x in ents if x["index"] == index), None)
    if e is None:
        raise SystemExit(f"entry {index} has no data in {pkg}")
    loc = e["loc"]
    blk = loc & 0x3FFF
    off = ((loc >> 14) & 0x3FFF) * 16
    size = (loc >> 28) & 0x3FFFFFFF
    print(f"entry {index}: class {e['class_tag']:#010x} block {blk} offset {off} size {size}")

    # Concatenate forward across blocks until the declared size is satisfied.
    data = b""
    b, o = blk, off
    while len(data) < size:
        chunk = plain_block(pkg, b)
        take = chunk[o:o + (size - len(data))]
        if not take:
            raise SystemExit(f"block {b} gave nothing at offset {o}; short by {size - len(data)}")
        data += take
        b, o = b + 1, 0
    print(f"recovered {len(data)} bytes across {b - blk} block(s)")

    # The container's own cross-check: most payloads open with a u64 length equal to the entry size.
    if len(data) >= 8:
        declared = struct.unpack_from("<Q", data, 0)[0]
        print(f"self-length header: {declared} ({'MATCHES' if declared == size else 'not a length header'})")
    print("head:", data[:args.hex].hex())
    if args.out:
        open(args.out, "wb").write(data)
        print("wrote", args.out)


if __name__ == "__main__":
    sys.exit(main())
