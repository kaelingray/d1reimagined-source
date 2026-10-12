#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""scenario-checksum - what value does the client expect at `global_activity_state + 0x5C`?

WHAT THIS ANSWERS
=================
Rung 33's world transition aborts with `Activity checksum mismatch!` when the Activity Host's
published `global_activity_state + 0x5C` disagrees with the client's own content.  `FUN_00299100`
reads the right-hand operand like this (`d1-eboot disasm 0x299100`, verbatim):

    0x0029916b  lea    rcx, [rip+0x17722ae]      ; -> 0x1A0B420, the package/tag registry global
    0x00299172  mov    rcx, [rcx]
    0x00299175  mov    rsi, [rcx]                ; registry base
    0x00299178  mov    rcx, [rsi+rbx+0x10]       ; bucket data_base   (rbx = slot * 0x40)
    0x0029917d  movsxd rdx, [rsi+rbx+0x34]       ; bucket mask (SIGNED)
    0x00299182  imul   r12d, [rsi+rbx+0x30]      ; (tag & 0x1fff) * entry_size
    0x00299188  and    rdx, [rcx+r12+8]
    0x0029918d  add    rcx, r12
    0x00299190  sub    rcx, rdx                  ; rcx = THE SCENARIO RECORD
    0x00299193  movsxd rdx, [r15+0x18]           ; rdx = c_activity_client + 0x18
    0x00299197  mov    eax, [rax+0x5c]           ; eax = OUR published blob+0x5C
    0x0029919a  cmp    eax, [rcx+rdx*4+0xc]      ; <- the expected value

The tag is `*(u32*)(activity_manager + 0x278)` - the **scenario handle**, written by
`c_online_world_controller::vfunc[3]` (`0x7896F0` -> `FUN_0075B8B0`) and named by the client's own
`world_controller:activity_manager: No valid scenario handle …` diagnostic.  The manager singleton
is `*(u64*)0x26B13F8` (`FUN_007669A0`).

`c_activity_client + 0x18` is the index into `{"PRIVATE", "PUBLIC"}` - `FUN_002980A0` writes it and
`sprintf`s the very name the mismatch line prints:

    *(u32*)(ac + 0x18) = param_2;                       // index into {"PRIVATE","PUBLIC"}
    *(u32*)(ac + 0x1c) = param_3;                       // index into {"CURRENT","TARGET"}
    sprintf(ac + 0x20, "%s %s", name_a, name_b);

Every run so far logs `[AC PRIVATE CURRENT …]`, so the index is **0** and the expected value is
`*(u32*)(scenario_record + 0x0C)`.  See `historicaldocs/design/activity-checksum.md`.

⚠ THE RECORD IS ONLY RESIDENT WHILE ITS WORLD IS LOADED.  In orbit, slot 0x5A9 holds exactly one
scenario record - `orbit`.  To learn a PvP map's checksum you must sample during the ~100 ms in
which the client has loaded the target world and is failing to instantiate it (`world_controller:
load successful, but failed to instantiate world and start game`).  That is what `watch` is for.

USAGE
=====
    ./scenario-checksum.py controls          # 4 controls; exit 2 on failure. Run this first.
    ./scenario-checksum.py now               # the current scenario and its checksums
    ./scenario-checksum.py watch --hz 50     # print every time the scenario handle changes

⭐ PORT TARGET.  Python here is a prototype (CLAUDE.md's "a reusable tool is a Rust tool").  This
belongs in the `d1-server` workspace next to `d1-live`; it is listed with the other outstanding
ports in `historicaldocs/design/activity-checksum.md` §7.
"""

import argparse
import importlib.util
import os
import struct
import sys
import time
import types

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

_spec = importlib.util.spec_from_file_location(
    "d1guest", os.path.join(REPO, "capture-rig", "d1guest.py"))
d1guest = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(d1guest)

# `FUN_007669A0` returns this singleton; `FUN_0075B8B0` writes +0x270/+0x274/+0x278.
ACTIVITY_MANAGER_PTR = 0x26B13F8
SCENARIO_HANDLE_OFF = 0x278
# `FUN_00299100`: expected = *(u32*)(record + CHECKSUM_BASE + ac[+0x18] * 4).
CHECKSUM_BASE = 0x0C
# The record's own identity: a self-relative `char*` and the FNV-1 of what it points at.
NAME_SELFREL_OFF = 0x30
NAME_HASH_OFF = 0x08
# ⭐ `FUN_002A0320` (the SECOND world-transition gate) reads the scenario's BUBBLE COUNT here as an
# i64, and requires OUR `global_activity_state + 0x10` to equal it. The bubbles ARE the slice-sets.
SLICE_COUNT_OFF = 0x48
BUBBLE_COUNT_OFF = 0x48
SLICE_SELFREL_OFF = 0x50
# The blob field the count must equal, and the 64-byte unchosen-state array, for the D1_AH_* hint.
BLOB_BUBBLE_COUNT_OFF = 0x10
BLOB_BUBBLE_STATE_CAP = 64


def fnv1(s: bytes) -> int:
    h = 0x811C9DC5
    for b in s:
        h = (h * 16777619) & 0xFFFFFFFF
        h ^= b
    return h


def open_client(args):
    ns = types.SimpleNamespace(
        symbols=d1guest.SYMBOLS, eboot=None, ignore_build_gate=False,
        full_scan=False, no_cache=False, account_soid=None, allow_dead=args.allow_dead)
    syms, image, proc, guest = d1guest.open_guest(ns)
    return proc, guest, ns


def read_cstr(proc, addr, cap=128):
    b = proc.read(addr, cap)
    return b.split(b"\0")[0]


class Scenario:
    """One resolved scenario record, with everything `FUN_00299100` touches."""

    def __init__(self, proc, tag, addr):
        self.tag, self.addr = tag, addr
        self.head = proc.read(addr, 0x60)
        self.name_hash = struct.unpack_from("<I", self.head, NAME_HASH_OFF)[0]
        self.checksums = list(struct.unpack_from("<4I", self.head, CHECKSUM_BASE))
        off = struct.unpack_from("<q", self.head, NAME_SELFREL_OFF)[0]
        try:
            self.name = read_cstr(proc, addr + NAME_SELFREL_OFF + off).decode("latin1")
        except Exception:
            self.name = None
        self.slice_count = struct.unpack_from("<q", self.head, SLICE_COUNT_OFF)[0]
        self.slice_hashes = []
        try:
            # `FUN_0075B910`: `lVar2 = *(long*)(record+0x50)` then the element is read at
            # `lVar2 + lVar8 + 0x60 + i*0x18` - the offset is added to the RECORD, not to its own
            # field.  Getting that wrong reads the validity flags as if they were hashes.
            base = addr + struct.unpack_from("<q", self.head, SLICE_SELFREL_OFF)[0] + 0x60
            n = self.slice_count if 0 < self.slice_count <= 256 else 0
            raw = proc.read(base, n * 0x18)
            for i in range(n):
                h, = struct.unpack_from("<I", raw, i * 0x18)
                ok, = struct.unpack_from("<i", raw, i * 0x18 + 8)
                self.slice_hashes.append((h, ok))
        except Exception:
            pass

    @property
    def name_hash_agrees(self):
        return self.name is not None and fnv1(self.name.encode("latin1")) == self.name_hash

    @property
    def bubble_count(self):
        # `FUN_002A0320` reads the SAME field (record+0x48) as the count of bubbles to validate.
        return self.slice_count

    @property
    def max_valid(self):
        # On the `-1` (unchosen) bubble path the inner sub-bubble loop runs iff a bubble's valid
        # flag is > 1. If every valid flag is <= 1, all-0xFF bubble state passes trivially.
        return max((v for _, v in self.slice_hashes), default=0)

    def line(self):
        agree = "OK" if self.name_hash_agrees else "MISMATCH"
        return (f"tag {self.tag:#010x}  name={self.name!r}  "
                f"+0x08 name_hash={self.name_hash:#010x} [{agree}]  "
                f"⭐ +0x0C PRIVATE={self.checksums[0]:#010x}  "
                f"+0x10 PUBLIC={self.checksums[1]:#010x}  "
                f"⭐ +0x48 BUBBLE_COUNT={self.bubble_count}  max_valid={self.max_valid}")


def current_scenario(proc, guest):
    """(tag, Scenario|None, why)."""
    mgr = guest.u64(ACTIVITY_MANAGER_PTR)
    if not mgr:
        return None, None, "activity manager singleton [0x26B13F8] is NULL"
    tag, = struct.unpack("<I", proc.read(mgr + SCENARIO_HANDLE_OFF, 4))
    if tag == 0xFFFFFFFF:
        return tag, None, "scenario handle is -1 - FUN_00299100 SKIPS the check entirely"
    obj, why = guest.resolve_tag(tag)
    if obj is None:
        return tag, None, why
    return tag, Scenario(proc, tag, obj), why


def cmd_now(args):
    proc, guest, ns = open_client(args)
    d1guest.print_liveness(guest, ns)
    tag, sc, why = current_scenario(proc, guest)
    if tag is None:
        print(why)
        return 1
    print(f"activity manager = {guest.u64(ACTIVITY_MANAGER_PTR):#x}   "
          f"scenario handle (+0x278) = {tag:#010x}")
    if sc is None:
        print(f"  {why}")
        return 1
    print(f"  record @ {sc.addr:#x}   ({why})")
    print(f"  {sc.line()}")
    print("  ⇒ publish  D1_AH_ACTIVITY_CHECKSUM=" f"{sc.checksums[0]:#010x}"
          "   (FUN_00299100; the AC logs itself as PRIVATE ⇒ index 0)")
    if sc.bubble_count > BLOB_BUBBLE_STATE_CAP:
        print(f"  ⚠ D1_AH_BUBBLE_COUNT={sc.bubble_count} EXCEEDS the {BLOB_BUBBLE_STATE_CAP}-byte "
              f"blob+0x14 field - this world needs more than the unchosen-array can hold")
    else:
        print(f"  ⇒ publish  D1_AH_BUBBLE_COUNT={sc.bubble_count}"
              f"   (FUN_002A0320; blob+0x10 must equal record+0x48, and blob+0x14[0..{sc.bubble_count}]=0xFF)")
    if sc.max_valid > 1:
        print(f"  ⚠ max bubble valid-flag = {sc.max_valid} > 1 - the -1-path INNER loop WILL run "
              f"for some bubbles; all-0xFF is not provably sufficient for this world")
    for i, (h, ok) in enumerate(sc.slice_hashes):
        if h:
            print(f"    bubble[{i:2}] HASH({h:#010x}) valid={ok}")
    return 0


def cmd_watch(args):
    proc, guest, ns = open_client(args)
    d1guest.print_liveness(guest, ns)
    mgr = guest.u64(ACTIVITY_MANAGER_PTR)
    print(f"watching activity_manager({mgr:#x}) + 0x278 at {args.hz} Hz - "
          f"poke an activity now; every CHANGE is printed")
    last, period = None, 1.0 / args.hz
    deadline = time.time() + args.for_ if args.for_ else None
    while deadline is None or time.time() < deadline:
        try:
            tag, = struct.unpack("<I", proc.read(mgr + SCENARIO_HANDLE_OFF, 4))
        except Exception as e:
            print(f"WATCH ENDED: {e}")
            return 3
        if tag != last:
            stamp = time.strftime("%H:%M:%S")
            if tag == 0xFFFFFFFF:
                print(f"{stamp}  scenario handle CLEARED (-1) - the check is skipped")
            else:
                obj, why = guest.resolve_tag(tag)
                if obj is None:
                    print(f"{stamp}  tag {tag:#010x}: {why}")
                else:
                    print(f"{stamp}  {Scenario(proc, tag, obj).line()}", flush=True)
            last = tag
        time.sleep(period)
    return 0


def cmd_controls(args):
    """Four controls. Each names the axis it protects; two of them can FAIL by construction."""
    proc, guest, ns = open_client(args)
    ok = True

    # 1. LIVENESS. Axis: "a read after teardown returns zeros that look like a real answer."
    level, lines = guest.liveness(d1guest._int(guest.syms.doc["liveness"]["account_soid"]))
    print("\n".join(lines))
    if level < 3:
        print(f"  FAIL  liveness L{level} < L3")
        ok = False

    tag, sc, why = current_scenario(proc, guest)
    if sc is None:
        print(f"  FAIL  no resolvable scenario record: {why}")
        return 2

    # 2. POSITIVE. Axis: "am I reading the RIGHT OFFSETS of the RIGHT RECORD?"  The record carries
    #    its own name twice - once as a self-relative string, once as an FNV-1 hash.  A wrong
    #    record, a wrong base, or a wrong offset breaks the agreement.  It is not a race control.
    h = fnv1(sc.name.encode("latin1")) if sc.name else None
    good = sc.name_hash_agrees
    if h is None:
        print("  FAIL  record has no readable name string")
    else:
        print(f"  {'PASS' if good else 'FAIL'}  record self-identity: name {sc.name!r} hashes to "
              f"{h:#010x} and +0x08 reads {sc.name_hash:#010x}")
    ok &= bool(good)

    # 3. NEGATIVE. Axis: "would control 2 pass on ANYTHING?"  Do the same test one dword early.
    #    It MUST fail, or control 2 is decoration.
    shifted = struct.unpack_from("<I", sc.head, NAME_HASH_OFF - 4)[0]
    bad = (h is not None and shifted != h)
    print(f"  {'PASS' if bad else 'FAIL'}  negative control: +0x04 reads {shifted:#010x}, which is "
          f"NOT the name hash - the offset discriminates")
    ok &= bool(bad)

    # 4. POSITIVE. Axis: "is the record really a SCENARIO?"  A scenario owns a slice-set array, and
    #    the client prints slice-set hashes out of it (`falling back to slice-set 'HASH(0x…)'`).
    nz = [h_ for h_, _ in sc.slice_hashes if h_]
    good4 = 0 < sc.slice_count <= 256 and len(nz) > 0
    print(f"  {'PASS' if good4 else 'FAIL'}  slice-set array: count={sc.slice_count}, "
          f"{len(nz)} non-zero hash(es), e.g. "
          f"{', '.join(f'{x:#010x}' for x in nz[:4])}")
    ok &= bool(good4)

    # 5. POSITIVE. Axis: "is record+0x48 (the bubble count FUN_002A0320 gates on) a value we can
    #    actually publish?"  It must fit the 64-byte blob+0x14 field, and be the count of real
    #    bubbles.  A count that overflows the field, or is zero, means the all-0xFF fix cannot apply.
    good5 = 0 < sc.bubble_count <= BLOB_BUBBLE_STATE_CAP
    print(f"  {'PASS' if good5 else 'FAIL'}  bubble count record+0x{BUBBLE_COUNT_OFF:02X}="
          f"{sc.bubble_count} fits the {BLOB_BUBBLE_STATE_CAP}-byte blob+0x14 field ⇒ publishable")
    ok &= bool(good5)

    # 6. INVERTED. Axis: "is +0x48 the RIGHT offset for the count?"  Read the qword one slot earlier
    #    (+0x40); it MUST NOT look like a valid bubble count, or +0x48 is not discriminating.
    early = struct.unpack_from("<q", sc.head, BUBBLE_COUNT_OFF - 8)[0]
    bad6 = not (0 < early <= BLOB_BUBBLE_STATE_CAP)
    print(f"  {'PASS' if bad6 else 'FAIL'}  inverted: record+0x{BUBBLE_COUNT_OFF - 8:02X}={early} "
          f"is NOT a plausible bubble count - the offset discriminates")
    ok &= bool(bad6)

    # 7. POSITIVE. Axis: "does the benign all-0xFF path actually pass for THIS world?"  On the -1
    #    path the inner sub-bubble loop runs iff a bubble's valid flag is > 1.  All <= 1 ⇒ trivial.
    good7 = sc.max_valid <= 1
    print(f"  {'PASS' if good7 else 'FAIL'}  every bubble valid-flag <= 1 (max={sc.max_valid}) ⇒ "
          f"the -1-path inner loop never runs ⇒ all-0xFF passes")
    ok &= bool(good7)

    print("ALL CONTROLS PASS" if ok else "*** A CONTROL FAILED - do not cite this output ***")
    return 0 if ok else 2


def main():
    ap = argparse.ArgumentParser(prog="scenario-checksum",
                                 description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("--allow-dead", action="store_true")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("now")
    w = sub.add_parser("watch")
    w.add_argument("--hz", type=float, default=50.0)
    w.add_argument("--for", dest="for_", type=float, default=0.0,
                   help="seconds to watch (0 = forever)")
    sub.add_parser("controls")
    args = ap.parse_args()
    return {"now": cmd_now, "watch": cmd_watch, "controls": cmd_controls}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
