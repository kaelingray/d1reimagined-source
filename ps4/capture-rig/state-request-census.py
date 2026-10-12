#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Census of every world-controller STATE REQUEST in the D1 eboot.

The two request entry points (`design/eboot-map.md` §2.6):

    0x782710(state)                 -> tail-calls 0x782720(state, reason=0, context=NULL)
    0x783690(state, reason)         -> the two-argument form
    0x782720(state, reason, context)

Every `bootflow:`/`setup:`/`activity:` transition in the client goes through one of these, so the
set of their call sites IS the state-transition graph.  This tool finds them and recovers the
STATE and REASON immediate at each site by disassembling forward from the enclosing prologue.

WHY NOT `find-direct-call-xrefs.py`
-----------------------------------
That tool only recognises `E8 rel32`.  `0x79E4B0` reaches `0x783690` by `E9 rel32` (a tail call) --
exactly the defect ATLAS §4.2 retracted.  This scans BOTH forms and prints a positive control for
each.  A run without a passing control is not evidence of absence.

⚠ A linear sweep desyncs on data islands, so every hit is re-derived by anchoring on the enclosing
`55 48 89 e5` prologue and walking FORWARD (ATLAS §4.5).  Sites whose prologue cannot be found are
reported as `UNANCHORED`, never silently dropped.

Usage:
    ./state-request-census.py                 # the full census, grouped by target state
    ./state-request-census.py --json out.json
"""
import argparse
import json
import struct
import sys
from collections import defaultdict

import capstone

EBOOT = "$HOME/dev/destiny1-preservation/dumps/patch-01.29-CUSA00219/eboot.bin"
BIAS = 0x4000
TEXT_LO, TEXT_HI = 0x1000, 0x18BF0DC
PROLOGUE = b"\x55\x48\x89\xe5"

REQ_ONE = 0x782710   # request_state(state)
REQ_TWO = 0x783690   # request_state(state, reason)
REQ_THREE = 0x782720  # request_state(state, reason, context)

# Sites established before this tool existed -- ATLAS §4.2/§4.4d positive controls.
CONTROLS = {
    0x79FE8E: (REQ_ONE, 0x19),    # prologue handler -> cleanup            (eboot-map §2.6)
    0x79E886: (REQ_ONE, 0x16),    # profile_setup    -> prepare_for_orbit  (this session)
    0x79EC93: (REQ_ONE, 0x12),    # queuing          -> package_registration
    0x79EC4F: (REQ_TWO, 0x19),    # queuing          -> cleanup (reason 0x79)
    0x79ECB1: (REQ_TWO, 0x08),    # queuing          -> account_signin
    0x79E4B0: (REQ_TWO, 0x19),    # TAIL CALL (E9)   -> cleanup (reason 0x38)
}

STATES = [
    "pregame", "bootflow:media_check", "bootflow:bootload", "bootflow:intro_screen_display",
    "bootflow:start", "bootflow:platform_signin", "bootflow:platform_account_warnings",
    "bootflow:enumerate_content_rights", "bootflow:account_signin", "bootflow:eula",
    "bootflow:upsell", "bootflow:email", "bootflow:confirm_account_transfer",
    "bootflow:content_check", "bootflow:content_install", "bootflow:insufficient_content_space",
    "bootflow:illegal_dlc_media", "bootflow:queuing", "bootflow:package_registration",
    "bootflow:bap_signin", "bootflow:investment_signin", "bootflow:profile_setup",
    "bootflow:prepare_for_orbit", "character:signin", "character:prologue", "cleanup",
    "setup:orbit_loading", "setup:orbit", "setup:activity_session_creation", "setup:matchmaking",
    "setup:activity_host_setup", "setup:prologue_intro_loading", "setup:orbit_outro",
    "setup:activity_world_transition", "activity:initial_slice_set_loading",
    "activity:physics_join", "activity:in_world", "activity:watch_video",
]

_d = None


def data():
    global _d
    if _d is None:
        _d = open(EBOOT, "rb").read()
    return _d


def off(va):
    return va + BIAS


def state_name(n):
    if n is None:
        return "?"
    return STATES[n] if 0 <= n < len(STATES) else f"<{n}>"


def find_prologue(target_va, max_back=0x6000):
    d = data()
    hi = off(target_va)
    lo = max(0, hi - max_back)
    idx = d.rfind(PROLOGUE, lo, hi + 4)
    return idx - BIAS if idx >= 0 else None


def scan_refs(targets):
    """Every E8/E9 rel32 in the text segment whose destination is in `targets`."""
    d = data()
    hits = []
    lo, hi = off(TEXT_LO), off(TEXT_HI)
    for i in range(lo, hi - 5):
        op = d[i]
        if op != 0xE8 and op != 0xE9:
            continue
        rel = struct.unpack_from("<i", d, i + 1)[0]
        site = i - BIAS
        dest = site + 5 + rel
        if dest in targets:
            hits.append((site, dest, "call" if op == 0xE8 else "jmp"))
    return hits


def recover_args(site_va, dest):
    """Disassemble forward from the enclosing prologue, tracking edi/esi/edx immediates.

    Returns (func_va, edi, esi, edx, confirmed) where `confirmed` means capstone's own decoding
    landed exactly on `site_va` with the expected branch target -- i.e. the byte match was not a
    mid-instruction false positive.
    """
    fn = find_prologue(site_va)
    if fn is None:
        return None, None, None, None, False
    d = data()
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    blob = d[off(fn):off(site_va) + 8]
    edi = esi = edx = None
    confirmed = False
    for ins in md.disasm(blob, fn):
        if ins.address == site_va:
            confirmed = ins.mnemonic in ("call", "jmp") and ins.op_str == hex(dest)
            break
        if ins.address > site_va:
            break
        m, o = ins.mnemonic, ins.op_str
        if m == "mov":
            parts = o.split(", ")
            if len(parts) == 2:
                try:
                    v = int(parts[1], 0)  # capstone prints 0..9 WITHOUT the 0x prefix
                except ValueError:
                    continue
                if parts[0] in ("edi", "rdi"):
                    edi = v
                elif parts[0] in ("esi", "rsi"):
                    esi = v
                elif parts[0] in ("edx", "rdx"):
                    edx = v
        elif m == "xor":
            parts = o.split(", ")
            if len(parts) == 2 and parts[0] == parts[1]:
                if parts[0] in ("edi", "rdi"):
                    edi = 0
                elif parts[0] in ("esi", "rsi"):
                    esi = 0
                elif parts[0] in ("edx", "rdx"):
                    edx = 0
        elif m in ("call", "jmp"):
            # a call between the prologue and our site clobbers the arg registers
            edi = esi = edx = None
    return fn, edi, esi, edx, confirmed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    args = ap.parse_args()

    targets = {REQ_ONE: "request_state(state)",
               REQ_TWO: "request_state(state, reason)",
               REQ_THREE: "request_state(state, reason, ctx)"}
    hits = scan_refs(set(targets))

    rows = []
    for site, dest, form in hits:
        fn, edi, esi, edx, ok = recover_args(site, dest)
        rows.append(dict(site=site, dest=dest, form=form, func=fn,
                         state=edi, reason=esi, ctx=edx, decoded=ok))

    # --- POSITIVE CONTROL -------------------------------------------------------------------
    found = {r["site"]: r for r in rows}
    print("POSITIVE CONTROL (ATLAS §4.2) - sites known before this tool existed:")
    all_ok = True
    for site, (dest, state) in sorted(CONTROLS.items()):
        r = found.get(site)
        ok = r is not None and r["dest"] == dest and r["state"] == state and r["decoded"]
        all_ok &= ok
        got = "MISSING" if r is None else (
            f"dest={r['dest']:#x} state={r['state']} decoded={r['decoded']}")
        print(f"  {'PASS' if ok else 'FAIL'}  {site:#x} -> {dest:#x} expect state {state}: {got}")
    print(f"  control: {'ALL PASS' if all_ok else 'FAILED - treat every negative below as void'}")
    print()
    e9 = [r for r in rows if r["form"] == "jmp"]
    print(f"tail-call (E9) sites found: {len(e9)}  "
          f"(if 0 this scan is blind to tail calls - ATLAS §4.2)")
    print(f"total sites: {len(rows)}   "
          f"decoded: {sum(1 for r in rows if r['decoded'])}   "
          f"state immediate recovered: {sum(1 for r in rows if r['state'] is not None)}")
    print()

    by_state = defaultdict(list)
    for r in rows:
        by_state[r["state"]].append(r)
    for st in sorted(by_state, key=lambda x: (x is None, x)):
        print(f"--- state {st if st is not None else '?'} = {state_name(st)}  "
              f"({len(by_state[st])} site(s))")
        for r in sorted(by_state[st], key=lambda x: x["site"]):
            fn = f"{r['func']:#x}" if r["func"] else "UNANCHORED"
            rs = f" reason={r['reason']:#x}" if r["reason"] is not None else ""
            print(f"    {r['site']:#010x}  {r['form']:4}  in {fn}"
                  f"  -> {r['dest']:#x}{rs}{'' if r['decoded'] else '   [UNCONFIRMED]'}")
    if args.json:
        with open(args.json, "w") as f:
            json.dump(rows, f, indent=1)
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
