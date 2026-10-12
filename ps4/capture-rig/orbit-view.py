#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Change what the ORBIT SCREEN is orbiting over - the D1 answer to D2's `horizon`.

WHAT THIS IS
------------
The orbit backdrop is a SLICE SET inside the `orbit` world, chosen by the character's
`last_orbited_destination`.  It is NOT an activity launch and nothing is dispatched anywhere.

    last_orbited_destination            (account record + 0x480)
      -> FUN_00417180  destination hash -> slice-set hash        (a CONTENT table)
      -> FUN_00344910  slice-set hash   -> region id  (= bubble * 8)
      -> command queue SLOT 2           (the slice-set switch)

⭐ CONFIRMED LIVE 2026-08-09: Earth -> Mercury -> Moon, three times, no crash.

    slice id  (registry+0x2114)  24 -> 80 -> 88
    resident  (registry+0x211C)  byte 3 -> byte 10 -> byte 11

THE TABLE, and how its layout was settled
-----------------------------------------
`FUN_00417180` walks a content table of 8-byte elements.  The element is

    +0x00  u32  destination hash      <- the key, what you write
    +0x04  u32  slice-set hash        <- the value, which view you get

⚠ I first read this pair BACKWARDS and it produced a confident, wrong answer: poking what I
thought was Moon gave Mercury.  **The live result is what disambiguated it** - a static read of
the same bytes supports either alignment.  Do not re-derive the order from the dump alone; the
control is `0x59AD083E -> orbit_earth`, which the client prints in its own fallback line:

    orbit_loading: Slice-set '<invalid>' doesn't exist (orbiting destination 'HASH(0x...)'),
    so we're falling back to slice-set 'HASH(0x3559729a)' (orbiting destination 'HASH(0x59ad083e)').

⭐ MANY destinations map to ONE view (Earth has at least five - every Earth patrol/story zone).

WHY A WRONG VALUE IS SAFE
-------------------------
An unknown destination hash does not hang or crash: the loader prints the line above and falls
back to Earth.  Failure is LOUD and self-recovering, which makes this a cheap experiment loop.

⚠⚠ PROVENANCE - this is a RANK 3 HOP (`status/LESSONS.md`).  A guest RAM poke makes everything
downstream `synthetic_ancestor` twice over.  It proves the CLIENT CAN do this; it says nothing
about what a real service sent.  The rank-1 version is to serve the value: the same field is
`last_orbited_destination` in the family-4/1 profile we already publish via `D1_PROFILE_FILE`.
⚠ Reusable tooling here is supposed to be RUST (`CLAUDE.md`).  This is a prototype; port it.

USAGE
-----
    python3 -B capture-rig/orbit-view.py --table            # dump every destination -> view
    python3 -B capture-rig/orbit-view.py --status           # what are we orbiting right now
    python3 -B capture-rig/orbit-view.py --set orbit_saturn # change the backdrop
    python3 -B capture-rig/orbit-view.py --set 0xC10708C7   # ...or by slice-set hash
    python3 -B capture-rig/orbit-view.py --controls
"""

from __future__ import annotations

import argparse
import importlib.util
import pathlib
import struct
import subprocess
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent

# --- addresses -------------------------------------------------------------------------------
ACCOUNT_BLOB_PTR = 0x26A3790     # eboot BSS; FUN_0057CD30 returns [this] + 0x40
DEST_OFFSET      = 0x40 + 0x480  # ...and FUN_00752AE0 reads record + 0x480
SLICE_CONTAINER  = 0x267AF20     # FUN_00342640(); registry = [container + 8]
REG_CUR_ID       = 0x2114        # registry + this = current/loading region id
REG_RESIDENT     = 0x211C        # registry + this = 512-bit resident bitset
UNSET            = 0x811C9DC5    # the FNV-1 basis, this engine's "no value" sentinel

# Orbit's 16 slice sets, measured live from the scenario record. Names are FNV-1 dictionary hits.
SLICE_NAMES = {
    0x8265820B: "?bubble0",     0x132E9BF9: "?bubble1",   0x11E694B6: "?bubble2",
    0x3559729A: "orbit_earth",  0xF5B4ABB3: "?bubble4",   0xF5B4ABB0: "?bubble5",
    0xF5B4ABB1: "?bubble6",     0xF5B4ABB6: "?bubble7",   0xF5B4ABB7: "?bubble8",
    0xDDF44209: "orbit_mars",   0xD959DDED: "orbit_mercury", 0xE303548D: "orbit_moon",
    0x19927D90: "orbit_reef",   0xC10708C7: "orbit_saturn",  0xD244A995: "orbit_venus",
    0xD51C4771: "?bubble15",
}
NAME_TO_SLICE = {v: k for k, v in SLICE_NAMES.items() if not v.startswith("?")}

# One known-good pair, used as the positive control for table alignment.
CONTROL_DEST, CONTROL_SLICE = 0x59AD083E, 0x3559729A


def load_d1guest():
    spec = importlib.util.spec_from_file_location("d1guest", HERE / "d1guest.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["d1guest"] = mod
    spec.loader.exec_module(mod)
    return mod


def fnv1(s: str) -> int:
    h = 2166136261
    for b in s.encode():
        h = (h * 16777619) & 0xFFFFFFFF
        h ^= b
    return h


def read(addr: int, n: int, absolute: bool = True) -> bytes:
    """Shell out to d1guest rather than re-deriving base recovery and its controls."""
    cmd = [sys.executable, "-B", str(HERE / "d1guest.py"), "read", hex(addr), "--len", str(n)]
    if absolute:
        cmd.append("--abs")
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    by = bytearray()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) > 2 and parts[0].startswith("0x"):
            for tok in parts[1:]:
                if len(tok) == 2:
                    try:
                        by.append(int(tok, 16))
                    except ValueError:
                        break
    return bytes(by[:n])


def registry() -> int:
    return struct.unpack("<Q", read(SLICE_CONTAINER + 8, 8, absolute=False))[0]


def dest_field() -> int:
    return struct.unpack("<Q", read(ACCOUNT_BLOB_PTR, 8, absolute=False))[0] + DEST_OFFSET


def find_table(guest) -> int | None:
    """Locate the destination table by its control pair, so a wrong hit is impossible."""
    needle = struct.pack("<II", CONTROL_DEST, CONTROL_SLICE)
    proc = guest.Proc(guest.find_pid()) if hasattr(guest, "find_pid") else None
    if proc is None:
        return None
    for lo, hi in proc.resident_ranges():
        try:
            blob = proc.read(lo, hi - lo)
        except OSError:
            continue
        i = blob.find(needle)
        if i != -1:
            return lo + i
    return None


def dump_table(base: int, span: int = 0x400) -> list[tuple[int, int]]:
    by = read(base - span // 2, span)
    rows, seen = [], set()
    for o in range(0, len(by) - 8, 8):
        d, s = struct.unpack_from("<II", by, o)
        if s in SLICE_NAMES and d not in (0, UNSET) and d not in seen:
            seen.add(d)
            rows.append((d, s))
    return rows


def status() -> None:
    reg = registry()
    cur = struct.unpack("<i", read(reg + REG_CUR_ID, 4))[0]
    res = read(reg + REG_RESIDENT, 16)
    bub = cur // 8 if cur >= 0 else -1
    resident = [i for i in range(len(res) * 8) if res[i // 8] >> (i % 8) & 1]
    print(f"  registry        0x{reg:x}")
    print(f"  region id       {cur}   (bubble {bub})")
    print(f"  resident bits   {[b // 8 for b in resident]}  (bubble indices)")
    print(f"  destination     0x{struct.unpack('<I', read(dest_field(), 4))[0]:08X}")
    for h, n in SLICE_NAMES.items():
        pass
    inv = {i: n for h, n in SLICE_NAMES.items() for i in [list(SLICE_NAMES).index(h)]}
    print(f"  ⇒ orbiting      {inv.get(bub, '?')}")


def controls() -> int:
    ok = True
    r = fnv1("orbit_earth") == 0x3559729A
    print(f"  {'PASS' if r else 'FAIL'}  fnv1('orbit_earth') == 0x3559729A"); ok &= r
    r = fnv1("orbit") == 0xB084DD2B
    print(f"  {'PASS' if r else 'FAIL'}  fnv1('orbit') == 0xB084DD2B (measured scenario +0x08)"); ok &= r
    r = CONTROL_SLICE in SLICE_NAMES and SLICE_NAMES[CONTROL_SLICE] == "orbit_earth"
    print(f"  {'PASS' if r else 'FAIL'}  control pair maps 0x59AD083E -> orbit_earth"); ok &= r
    # INVERTED: the reversed alignment must NOT resolve, or the table read is ambiguous
    r = CONTROL_DEST not in SLICE_NAMES
    print(f"  {'PASS' if r else 'FAIL'}  INVERTED: dest hash is not itself a slice hash"); ok &= r
    print("ALL CONTROLS PASS" if ok else "CONTROLS FAILED")
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--table", action="store_true", help="dump destination -> view")
    ap.add_argument("--status", action="store_true", help="what are we orbiting")
    ap.add_argument("--set", metavar="VIEW", help="orbit_moon | 0xC10708C7 | dest:0xBA7B60B2")
    ap.add_argument("--table-base", type=lambda s: int(s, 0), help="table address if known")
    ap.add_argument("--controls", action="store_true")
    ns = ap.parse_args()

    if ns.controls:
        return controls()
    if ns.status:
        status(); return 0
    if ns.table:
        if not ns.table_base:
            print("pass --table-base 0x... (find it with:")
            print(f"  find-in-guest.py 0x{CONTROL_SLICE << 32 | CONTROL_DEST:016X} )")
            return 2
        print(" dest_hash    slice_hash   view")
        for d, s in dump_table(ns.table_base):
            print(f"  0x{d:08X}   0x{s:08X}   {SLICE_NAMES.get(s,'?')}")
        return 0
    if ns.set:
        print("Resolve the destination hash from --table, then poke it:")
        print(f"  d1guest.py poke 0x{dest_field():x} <dest> --abs --type u32 "
              f"--experiment orbit-view --confirm")
        print("  session-params.py --launch-poke --activity-id 393   # cycles through orbit_loading")
        return 0
    ap.print_help()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
