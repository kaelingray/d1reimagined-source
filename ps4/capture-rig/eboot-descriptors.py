#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""THIN WRAPPER. The RSAT/investment descriptor format now lives in ONE place: Rust.

    d1-server/crates/d1-eboot/src/descriptors.rs      the reader + the four parser defects
    d1-server/crates/d1-eboot/tests/controls.rs       the controls, as tests
    d1-server/crates/d1-eboot/tests/fixtures/         THIS tool's output, pinned as golden files

WHY THIS FILE IS NO LONGER A PARSER
-----------------------------------
`design/format-description.md`: **no single format may be described twice.** Two copies of the
anchored-descriptor layout in two languages are free to drift, and this project has already paid
for that exact failure - `historicaldocs/design/ATLAS.md` §4.3 documents a superseded reading surviving inside a
sibling script's constants and carrying a wrong conclusion into new code within hours.

So the layout, the anchor scan, the sentinel test and all FOUR documented parser defects moved to
`d1_eboot::descriptors`, which is covered by tests that reproduce this script's own former output
byte for byte. What is left here is (a) mechanical byte access, which is not a format description,
and (b) a shim so the three importers keep working unchanged:

    capture-rig/array-fields.py                (BIAS / SENTINEL / LEAF / EBOOT / Eboot.index)
    capture-rig/oodle/span-tag-inventory.py    (Eboot / fields)
    capture-rig/oodle/trace-investment-schema.py (Eboot.index)

BUILD IT FIRST:  cd d1-server && cargo build --release -p d1-eboot

Usage is unchanged; every flag is forwarded:
    ./eboot-descriptors.py --tag 0x80801825            # walk the 503 response schema tree
    ./eboot-descriptors.py --tag 0x80801796 --raw      # hexdump one descriptor
    ./eboot-descriptors.py --index | head              # every descriptor found, by tag
    ./eboot-descriptors.py --index --json              # + machine-readable, with field records
"""
import functools
import json
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
EBOOT = os.path.join(ROOT, "dumps/patch-01.29-CUSA00219/eboot.bin")

# ⚠ Kept ONLY because importers read them. They are re-derived and asserted in
# `d1-eboot/tests/controls.rs`; do not treat this file as their source of truth.
# ⚠ BIAS is the TEXT case. The bias is PER SEGMENT (ATLAS §4.1) - `d1-eboot segments` prints all
#   ten, and `d1_eboot::image::Image::file_offset` is the correct mapping.
BIAS = 0x4000
ANCHOR = bytes.fromhex("50008080" "0a018080")
SENTINEL = 0x3F000000
LEAF = 0xFFFFFFFF


def _binary():
    """Locate the Rust binary; refuse loudly rather than silently reimplementing the parser."""
    env = os.environ.get("D1_EBOOT_BIN")
    if env and os.path.exists(env):
        return [env]
    for build in ("release", "debug"):
        p = os.path.join(ROOT, "d1-server/target", build, "d1-eboot")
        if os.path.exists(p):
            return [p]
    sys.exit(
        "eboot-descriptors.py: d1-eboot is not built.\n"
        "  cd d1-server && cargo build --release -p d1-eboot\n"
        "(This tool is a wrapper on purpose - the descriptor format has exactly one "
        "implementation, in Rust. See design/format-description.md.)"
    )


def _run(args, path=None, capture=True):
    cmd = _binary() + (["--eboot", path] if path else []) + args
    if not capture:
        return subprocess.call(cmd)
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode not in (0, 1):
        sys.stderr.write(out.stderr)
        sys.exit(out.returncode)
    return out.stdout


@functools.lru_cache(maxsize=4)
def _dump(path):
    """Every descriptor WITH its raw field records, in one subprocess call.

    One call, not one per descriptor: the caller asks for the fields of all 4,555 at once. (The
    original such caller, `span-tag-inventory.py`, is archived to `historicaldocs/python-capture/`;
    `d1-eboot descriptors` is the supported way to ask this now.)
    """
    return json.loads(_run(["descriptors", "--index", "--with-fields", "--json"], path))


class Eboot:
    """Mechanical byte access + the index, which is delegated.

    `u32`/`u64`/`.d` stay local because reading a little-endian word at a file offset is not a
    format description. `index()` is delegated because deciding what counts as a descriptor IS.
    """

    def __init__(self, path=EBOOT):
        self.path = path
        self.d = open(path, "rb").read()

    def u32(self, vaddr):
        return struct.unpack_from("<I", self.d, vaddr + BIAS)[0]

    def u64(self, vaddr):
        return struct.unpack_from("<Q", self.d, vaddr + BIAS)[0]

    def index(self):
        """tag -> anchor vaddr, for every descriptor in the image (4,555 in the 01.29 build)."""
        return {r["tag"]: r["anchor"] for r in _dump(self.path)}

    def _records(self):
        return {r["anchor"]: r for r in _dump(self.path)}


def fields(eb, anchor, count):
    """Yield `(off, kind, child_tag, bits, marker, serialized_start)` per field.

    A missing sentinel yields `(None, None, None, None, marker, None)` - unchanged contract, so
    callers keep using it as the "layout broke, stop this struct" signal.

    ⚠ `bits` is a bit WIDTH only for leaf fields. On a composite the same column is the byte offset
    the element count is loaded from (ATLAS §4.3 / `array-fields.py`). This wrapper preserves the
    old positional tuple, so the caller must still know which it is holding.
    """
    rec = eb._records().get(anchor)
    if rec is None:
        # Not an indexed anchor - derive the markers arithmetically, as before.
        for i in range(count):
            mk = anchor + 0x30 + i * 0x28
            if eb.u32(mk + 4) != SENTINEL:
                yield None, None, None, None, mk, None
            else:
                yield (eb.u32(mk - 8), eb.u32(mk + 8), eb.u32(mk + 0x0C), eb.u32(mk + 0x14), mk,
                       eb.u32(mk))
        return
    for f in rec["fields"][:count]:
        if not f["present"]:
            yield None, None, None, None, f["marker"], None
        else:
            yield (f["offset"], f["kind"], f["child_tag"], f["raw_bits_or_count_offset"],
                   f["marker"], f["serialized_start"])


def main():
    # Forward verbatim; the Rust CLI takes the same flags and prints the same text.
    args = sys.argv[1:]
    if not args:
        args = ["--tag", "0x80801825"]
    sys.exit(_run(["descriptors"] + args, capture=False))


if __name__ == "__main__":
    main()
