#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
r"""Are RSAT tags STATIC or ASSIGNED AT LOAD?  Answer it per-record, against a live client.

`historicaldocs/design/character-and-orbit.md` §26.27 concluded tags are "materialised at runtime" because
`0x808033F4` appears nowhere in the eboot.  That is true of `0x808033F4` - but it is NOT true of the
type system as a whole, and conflating the two is what made the enclosing schema look unfindable.

This tool separates the two populations by measurement:

  * **ANCHORED AGGREGATES declare their own tag** at `anchor+0x10`.  If a static self tag `T`
    resolves live to the very record that declares it, the tag is a *compile-time constant* and the
    whole aggregate namespace is readable offline.
  * **LEAF-POOL objects carry no self tag** - only a name hash - so theirs must come from somewhere
    else.  `0x808033F4` is one of these.

It also locates the investment message-definition blob in the live process by ROW CONTENT rather
than by header bytes (the header scan in `live-namespace.py` came back empty), and reports whether
the live rows match the file.

POSITIVE CONTROLS, in every run:
  P1  resolve(0x80801825) -> the 503 response record, in-image             (a tag known to resolve)
  P2  resolve(0x808033F4) -> base + 0x1492550                              (§26.19)
  P3  resolve(0xDEADBEEF) -> nothing                                       (a tag known absent)
  P4  the row-content scan searches for a byte string that IS in the file  (row 76 / message 503)
      alongside one that is not (a corrupted copy of it).  Empty control => broken scanner.

Usage:  ./live-tag-identity.py            # needs a running client
"""
import ctypes
import json
import os
import struct
import sys
from importlib.machinery import SourceFileLoader

HERE = os.path.dirname(os.path.abspath(__file__))
_ln = SourceFileLoader("d1_livens", os.path.join(HERE, "live-namespace.py")).load_module()
_af = _ln._af
_ed = _ln._ed
BIAS = _ln.BIAS

DEFS_BIN = _ln.DEFS_BIN
CTL_503, CTL_SPAN, CTL_BOGUS = 0x80801825, 0x808033F4, 0xDEADBEEF


def main():
    img = _af.Img()
    descs = _af.load(img)
    pid = _ln.emu_pid()
    if not pid:
        raise SystemExit("no running shadps4")
    live = _ln.Live(pid)

    eb = img.d
    sig = eb[_ln.SIG_VADDR + BIAS: _ln.SIG_VADDR + BIAS + _ln.SIG_LEN]

    # row-content patterns for the definition blob: 12 bytes at record+0x10 = {id, req tag, rsp tag}
    defs = open(DEFS_BIN, "rb").read()
    row76 = defs[0x40 + 76 * 0x40 + 0x10: 0x40 + 76 * 0x40 + 0x1C]
    row59 = defs[0x40 + 59 * 0x40 + 0x10: 0x40 + 59 * 0x40 + 0x1C]
    bad = bytes([row76[0] ^ 0xFF]) + row76[1:]          # the negative control for the same scan

    hits = live.scan({"eboot": sig, "row76": row76, "row59": row59, "row76_corrupt": bad})
    if not hits["eboot"]:
        raise SystemExit("eboot signature not found - refusing to report anything")
    base = sorted({h - _ln.SIG_VADDR for h in hits["eboot"]})[0]
    gp = live.u64(base + _ln.REGISTRY_GLOBAL)
    registry = live.u64(gp) if gp else None
    if not registry:
        raise SystemExit("registry not built")
    R = _ln.Resolver(live, registry)
    print(f"pid {pid}  base {base:#x}  registry {registry:#x}")

    ctl = {
        "P1_503": R.resolve(CTL_503)[2],
        "P2_span": R.resolve(CTL_SPAN)[2],
        "P3_bogus": R.resolve(CTL_BOGUS)[2],
    }
    print(f"  P1 resolve(0x80801825) = {ctl['P1_503'] and hex(ctl['P1_503'])} "
          f"(vaddr {ctl['P1_503'] and hex(ctl['P1_503'] - base)})  "
          f"PASS={ctl['P1_503'] is not None}")
    print(f"  P2 resolve(0x808033F4) = {ctl['P2_span'] and hex(ctl['P2_span'])}  "
          f"PASS={ctl['P2_span'] == base + _ln.SPAN_ELEMENT_VADDR}")
    print(f"  P3 resolve(0xDEADBEEF) = {ctl['P3_bogus']}  PASS={ctl['P3_bogus'] is None}")

    # ── A. do anchored aggregates resolve to the record that DECLARES them? ───────────────────────
    print("\n--- A. anchored self tags: does each resolve to its own record? ---")
    same, near, elsewhere, unresolved = 0, 0, 0, 0
    deltas = {}
    misses = []
    for t, d in sorted(descs.items()):
        _s, _i, ob = R.resolve(t)
        if ob is None:
            unresolved += 1
            misses.append((t, d.anchor, None))
            continue
        v = ob - base
        delta = d.anchor - v
        if 0 <= delta <= 0x400:
            near += 1
            deltas[delta] = deltas.get(delta, 0) + 1
            # the declaration test: the anchor this record resolves to declares THIS tag
            if img.u32(d.anchor + 0x10) == t:
                same += 1
        else:
            elsewhere += 1
            if len(misses) < 12:
                misses.append((t, d.anchor, v))
    print(f"  {len(descs)} anchored descriptors: {near} resolve to their own record "
          f"(anchor - object in 0..0x400), {same} of those have anchor+0x10 == the tag, "
          f"{elsewhere} land elsewhere, {unresolved} do not resolve")
    print(f"  anchor-minus-object deltas: "
          f"{sorted(deltas.items(), key=lambda kv: -kv[1])[:8]}")
    for t, a, v in misses[:12]:
        print(f"    MISS tag {t:#010x} anchor {a:#x} -> object vaddr {v if v is None else hex(v)}")

    # ── B. every 0x8080xxxx word in the image, resolved ───────────────────────────────────────────
    print("\n--- B. resolvability of every 0x8080xxxx word in the image ---")
    S = _ln.static_inputs()
    res, unres, inimg = 0, 0, 0
    for t in S["every_tag"]:
        _s, _i, ob = R.resolve(t)
        if ob is None:
            unres += 1
        else:
            res += 1
            if 0 < ob - base < 0x4000000:
                inimg += 1
    print(f"  {len(S['every_tag'])} distinct tags: {res} resolve ({inimg} into the eboot image), "
          f"{unres} do not")

    # ── C. the definition blob, live ──────────────────────────────────────────────────────────────
    print("\n--- C. investment message-definition blob, live ---")
    print(f"  row76 pattern {row76.hex()} hits: {[hex(h) for h in hits['row76']]}")
    print(f"  row59 pattern {row59.hex()} hits: {[hex(h) for h in hits['row59']]}")
    print(f"  CONTROL corrupted row76 {bad.hex()} hits: "
          f"{[hex(h) for h in hits['row76_corrupt']] or 'none (correct)'}")
    live_defs = None
    for h in hits["row76"]:
        blob_base = h - 0x10 - 76 * 0x40 - 0x40
        got = live.rd(blob_base, len(defs))
        if got == defs:
            live_defs = blob_base
            print(f"  blob found at {blob_base:#x} (vaddr {blob_base - base:#x} if in image) "
                  f"and is BYTE-IDENTICAL to {os.path.basename(DEFS_BIN)}")
            break
        if got:
            diff = sum(1 for a, b in zip(got, defs) if a != b)
            print(f"  candidate {blob_base:#x}: {diff}/{len(defs)} bytes differ from the file")
    if live_defs is None and hits["row76"]:
        print("  ⚠ rows present but the blob does not align to the file layout at any hit")
    if not hits["row76"]:
        print("  ⚠ NOT FOUND - but the eboot signature WAS found in the same pass, so the scanner "
              "works; regions larger than 1GiB are skipped and could still hold it")

    out = os.path.join(HERE, "namespace-tag-identity.json")
    with open(out, "w") as fh:
        json.dump(dict(pid=pid, base=hex(base), registry=hex(registry),
                       controls={k: (hex(v) if isinstance(v, int) else v) for k, v in ctl.items()},
                       anchored=dict(total=len(descs), own_record=near, declares_tag=same,
                                     elsewhere=elsewhere, unresolved=unresolved,
                                     deltas={hex(k): v for k, v in deltas.items()}),
                       every_tag=dict(total=len(S["every_tag"]), resolved=res,
                                      in_image=inimg, unresolved=unres),
                       defs=dict(row76_hits=[hex(h) for h in hits["row76"]],
                                 row59_hits=[hex(h) for h in hits["row59"]],
                                 corrupt_control_hits=[hex(h) for h in hits["row76_corrupt"]],
                                 blob=hex(live_defs) if live_defs else None)), fh, indent=1)
    print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
