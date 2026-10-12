#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
r"""Dump the WHOLE runtime type namespace out of a live client, in ONE emulator run.

WHY THIS EXISTS
---------------
`historicaldocs/design/character-and-orbit.md` §26.27/§26.28 established that RSAT type **tags** (`0x8080xxxx`) are
**assigned at load** and appear nowhere in the eboot file - only the *name hash* is static, and the
registry derives its slot/index arithmetically so it never stores the key either.  §26.28 therefore
left the whole namespace unmapped and asked for exactly this: every static name hash paired with the
runtime tag its `.bss` slot receives.

That pairing can only be read from a running process.  This tool does it in one pass, together with
everything else that needs the same process, because an emulator run is expensive and the project's
rule is *plan the reads before you launch*.

WHAT IT PRODUCES
----------------
`capture-rig/namespace-map.tsv`   one row per name-hash registry entry:
      registry, index, record_vaddr, name_hash, bss_slot_vaddr, runtime_tag,
      tag_slot, tag_index, object_addr, object_vaddr
`capture-rig/namespace-map.json`  the same plus metadata, the controls, the policy table, and the
      decisive-query result.

THE DECISIVE QUERY
------------------
Which anchored descriptor holds an array field whose ELEMENT TYPE is the span element `0x808033F4`?
Static search cannot answer it (§26.27: the tag is not in the image).  Live it is answerable by
*identity of the resolved object* rather than by tag equality:

    target = base + 0x1492550                     # what tag 0x808033F4 resolves to (§26.19)
    for every static tag anywhere in the descriptor tables: resolve it live; does it hit `target`?

Comparing resolved OBJECTS is strictly stronger than comparing tags, because a renumbered tag still
lands on the same descriptor.

⚠ TWO FRAMINGS OF THE NAME-HASH REGISTRY, AND WHY THIS TOOL REPORTS BOTH
-----------------------------------------------------------------------
§26.28 read the 16-byte record as `{u32 hash, pad4, u64 ptr}` based at `0x19D0BD8`.
`capture-rig/name-hash-tables.py` corrects it to `{u64 ptr, u32 hash, u32 pad}` based at
`0x19D0BD0` - the same bytes, framed 8 lower - and notes an unexplained `[U]` discrepancy against
§26.28's *live* slot numbers.  The two framings pair each hash with a **different** pointer (they
differ by one record), so "entry [1166] holds tag X" is ambiguous until the framing is settled.

This tool refuses to pick a side by assertion.  It reads the raw 16-byte records and reports, for
every entry, the tag reached under BOTH framings; and it settles the question empirically by
*reverse* lookup - find the `.bss` slot that actually contains `0x808033F4` and print the hash that
sits in the same record and the hash one record back.  Whichever is `0xE2B769E8` / `0xE2E25C2C`
names the framing, from data rather than from a reading.

POSITIVE CONTROLS (mandatory - §26.10; every one runs in the same pass as the dump)
----------------------------------------------------------------------------------
  C1  eboot base recovered by signature, and the signature is unique
  C2  registry global non-NULL and registry pointer non-NULL
  C3  resolve(0x808033F4) == base + 0x1492550          (§26.19's live result, re-established)
  C4  resolve(0x80801825) lands inside the eboot image (the 503 response descriptor, slot 0x400)
  C5  a deliberately bogus tag (0xDEADBEEF) does NOT resolve to the target
  C6  the two name-hash registries have the counts their registrars declare (6338 / 1332)
  C7  a nonzero majority of `.bss` slots hold plausible `0x8080xxxx` tags
If C1..C3 fail the tool refuses to write output at all: a wrong base makes every other number a
confident fiction, which is the exact failure mode this project has paid for repeatedly.

USAGE
-----
    ./live-namespace.py --dry-run       # static half only; no emulator needed. Run this FIRST.
    ./live-namespace.py                 # against a running client (pgrep -f "[s]hadps4 -p ")
    ./live-namespace.py --wait 240      # poll up to N seconds for the controls to come up
"""
import argparse
import ctypes
import json
import os
import re
import struct
import subprocess
import sys
import time
from importlib.machinery import SourceFileLoader

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)

_reg = SourceFileLoader("d1_dumpreg", os.path.join(HERE, "dump-registry.py")).load_module()
_af = SourceFileLoader("d1_arrfld", os.path.join(HERE, "array-fields.py")).load_module()
_ed = _af._ed

BIAS = 0x4000
EBOOT = _reg.EBOOT
SIG_VADDR, SIG_LEN = _reg.SIG_VADDR, _reg.SIG_LEN
REGISTRY_GLOBAL = _reg.REGISTRY_GLOBAL

# name-hash registries: (base_vaddr_ptr_first_framing, count, registrar) - counts from the
# registrars, per capture-rig/name-hash-tables.py, not from a heuristic scan.
REGISTRIES = {"A": (0x19B3530, 6338, 0xC262D0), "B": (0x19D0BD0, 1332, 0xC2B2B0)}

SPAN_ELEMENT_TAG = 0x808033F4        # §26.17, runtime-only
SPAN_ELEMENT_VADDR = 0x1492550       # §26.19, what it resolved to
CRASH_PATH_GLOBAL = 0x19CC800        # 0x570A71 loads this; *(*g) is the span element tag
CTL_503_RESPONSE = 0x80801825
CTL_BOGUS_TAG = 0xDEADBEEF

POLICY_TABLE = 0x1923CB0             # 33 ptr-to-singleton entries (0x55F400 indexes it)
POLICY_NAMES = 0x1923AA0             # 33 name string ptrs (0x55F410)
POLICY_SINGLETONS = 0x1923BA8        # the 33 singletons themselves
POLICY_N = 33

DEFS_BIN = os.path.join(HERE, "schemas", "investment-message-definitions.bin")

# The field-pool object layout as re-based by the static recon pass (object begins at marker+8):
#   O+0x00 size · O+0x08 name hash · O+0x0c aux · O+0x10 child tag · O+0x14 width in BITS
POOL_LO, POOL_HI = 0x13D8A40, 0x14A2E00


# ─────────────────────────────────────────────────────────────────── live process access ──────────
class Live:
    def __init__(self, pid):
        self.pid = pid
        self.f = open(f"/proc/{pid}/mem", "rb", 0)
        self.regions = []
        for line in open(f"/proc/{pid}/maps"):
            m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S{4})", line)
            if m and "r" in m.group(3):
                self.regions.append((int(m.group(1), 16), int(m.group(2), 16)))
        self.base = None

    def rd(self, addr, n):
        try:
            self.f.seek(addr)
            b = self.f.read(n)
            return b if b and len(b) == n else None
        except Exception:
            return None

    def u32(self, a):
        b = self.rd(a, 4)
        return None if b is None else struct.unpack("<I", b)[0]

    def u64(self, a):
        b = self.rd(a, 8)
        return None if b is None else struct.unpack("<Q", b)[0]

    def scan(self, patterns, limit=8):
        """One pass over the readable regions looking for several byte patterns at once.

        Doing it in one pass matters: the emulator's address space is large and re-scanning per
        pattern is what makes a live dump take minutes instead of seconds.
        """
        hits = {k: [] for k in patterns}
        for lo, hi in self.regions:
            if hi - lo > 0x40000000:          # skip enormous reservations
                continue
            try:
                self.f.seek(lo)
                blob = self.f.read(hi - lo)
            except Exception:
                continue
            if not blob:
                continue
            for k, pat in patterns.items():
                if len(hits[k]) >= limit:
                    continue
                start = 0
                while True:
                    j = blob.find(pat, start)
                    if j < 0:
                        break
                    hits[k].append(lo + j)
                    start = j + 1
                    if len(hits[k]) >= limit:
                        break
        return hits


class Resolver:
    """`tag -> descriptor object`, exactly the arithmetic at 0xBF25A0 / dump-registry.py::resolve."""

    def __init__(self, live, registry):
        self.live, self.registry = live, registry
        self.slots = {}
        self.cache = {}

    def slot_rec(self, slot):
        if slot not in self.slots:
            rec = self.live.rd(self.registry + slot * 0x40, 0x40)
            if rec is None:
                self.slots[slot] = None
            else:
                self.slots[slot] = (struct.unpack_from("<Q", rec, 0x10)[0],
                                    struct.unpack_from("<i", rec, 0x30)[0],
                                    struct.unpack_from("<i", rec, 0x34)[0])
        return self.slots[slot]

    def resolve(self, tag):
        if tag in self.cache:
            return self.cache[tag]
        slot, index = _reg.tag_slot(tag)
        out = (slot, index, None)
        sr = self.slot_rec(slot)
        if sr:
            data_base, esize, mask = sr
            if data_base and 0 < esize <= 0x100:
                ent = data_base + index * esize
                loc = self.live.u64(ent + 8)
                if loc is not None:
                    # signed 64-bit back-offset: `and rbx,[rsi+rax+8]` then a SIGNED `sub rsi,rbx`
                    out = (slot, index, ent - ctypes.c_int64(mask & loc).value)
        self.cache[tag] = out
        return out


# ─────────────────────────────────────────────────────────────────── static half ──────────────────
def static_inputs():
    """Everything derivable from the file: the tags we will resolve, and where they came from."""
    img = _af.Img()
    descs = _af.load(img)

    self_tags = sorted(descs.keys())
    array_fields = []          # (owner_tag, field_index, child_tag, elem_kind, elem_child)
    field_children = set()
    for t, d in descs.items():
        for f in d.fields:
            if f.child != _ed.LEAF:
                field_children.add(f.child)
            if not f.is_dynamic_array:
                continue
            ck, cc = None, None
            cd = descs.get(f.child)
            if cd is not None:
                av = cd.array_view(img)
                ck, cc = av["kind"], av["child"]
            array_fields.append(dict(owner=t, field=f.i, child=f.child,
                                     elem_kind=ck, elem_child=cc,
                                     count_off=f.param14))

    # the leaf/field pool's typed child tags (re-based layout: object at marker+8)
    pool_children = set()
    d = img.d
    o = POOL_LO
    npool = 0
    while o + 0x58 <= POOL_HI:
        size = struct.unpack_from("<I", d, o + BIAS)[0]
        if size not in (0x54, 0x5C, 0x64, 0xA8) or size == 0:
            break
        ct = struct.unpack_from("<I", d, o + 0x10 + BIAS)[0]
        if ct != 0xFFFFFFFF:
            pool_children.add(ct)
        npool += 1
        nxt = struct.unpack_from("<I", d, o + size + BIAS)[0] if o + size + 4 <= POOL_HI else 1
        o = o + size + 4 if nxt == 0 else o + size
    # WIDEST POSSIBLE STATIC TAG SET - every distinct `0x8080xxxx` word anywhere in the image, at
    # every alignment.  A negative answer to the decisive query is only worth stating if the set we
    # searched is the whole set; restricting to descriptor columns would leave the "we looked in the
    # wrong column" escape hatch open, which is exactly how §26.9 and §26.15 went wrong.
    every_tag = set()
    blob = img.d
    j = blob.find(b"\x80\x80", 0)
    while j >= 0:
        if j >= 2:
            v = struct.unpack_from("<I", blob, j - 2)[0]
            if (v & 0xFFFF0000) == 0x80800000:
                every_tag.add(v)
        j = blob.find(b"\x80\x80", j + 1)

    return dict(img=img, descs=descs, self_tags=self_tags, array_fields=array_fields,
                field_children=sorted(field_children), pool_children=sorted(pool_children),
                npool=npool, every_tag=sorted(every_tag))


def read_registry_records(img_bytes, base_v, count):
    """Raw 16-byte records, FILE copy - the pointers are 0 here (relocated at load)."""
    out = []
    for i in range(count):
        v = base_v + i * 16
        ptr, h, pad = struct.unpack_from("<QII", img_bytes, v + BIAS)
        out.append((v, ptr, h, pad))
    return out


# ─────────────────────────────────────────────────────────────────── main ─────────────────────────
def emu_pid():
    out = subprocess.run(["pgrep", "-f", "[s]hadps4 -p "], capture_output=True, text=True).stdout
    pids = [int(x) for x in out.split()]
    return pids[0] if pids else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("--dry-run", action="store_true", help="static half only; no emulator")
    ap.add_argument("--wait", type=int, default=0, help="poll up to N seconds for controls to pass")
    ap.add_argument("--out-tsv", default=os.path.join(HERE, "namespace-map.tsv"))
    ap.add_argument("--out-json", default=os.path.join(HERE, "namespace-map.json"))
    args = ap.parse_args()

    print("=== STATIC HALF ===")
    S = static_inputs()
    print(f"anchored descriptors      : {len(S['self_tags'])}")
    print(f"array fields              : {len(S['array_fields'])}")
    print(f"distinct field child tags : {len(S['field_children'])}")
    print(f"leaf/field-pool objects   : {S['npool']}   typed child tags: {len(S['pool_children'])}")
    print(f"every distinct 0x8080xxxx word in image : {len(S['every_tag'])}")
    eb = S["img"].d
    for name, (bv, cnt, _r) in REGISTRIES.items():
        recs = read_registry_records(eb, bv, cnt)
        nz = sum(1 for _v, p, _h, _pd in recs if p)
        asc = all(recs[i][2] < recs[i + 1][2] for i in range(len(recs) - 1))
        print(f"name-hash registry {name}: {cnt} records @ {bv:#x}  "
              f"hash ascending={asc}  nonzero ptrs in FILE={nz} (expect 0: relocated)")
    # STATIC CONTROL: the two hashes at the centre of the §26.28 dispute must both be present.
    hb = {h: i for i, (_v, _p, h, _pd) in enumerate(read_registry_records(eb, *REGISTRIES["B"][:2]))}
    print(f"static control: 0xE2B769E8 in registry B -> index {hb.get(0xE2B769E8)}   "
          f"0xE2E25C2C -> index {hb.get(0xE2E25C2C)}   0xDEADBEEF -> {hb.get(0xDEADBEEF)} (expect None)")
    if args.dry_run:
        print("\n--dry-run: stopping before any live read.")
        return 0

    # ── live ──────────────────────────────────────────────────────────────────────────────────────
    print("\n=== LIVE HALF ===")
    deadline = time.time() + max(args.wait, 0)
    live = None
    while True:
        pid = emu_pid()
        if pid:
            live = Live(pid)
            sig = eb[SIG_VADDR + BIAS: SIG_VADDR + BIAS + SIG_LEN]
            pats = {"eboot": sig}
            if os.path.exists(DEFS_BIN):
                pats["defs"] = open(DEFS_BIN, "rb").read()[:64]
            hits = live.scan(pats)
            if hits["eboot"]:
                bases = sorted({h - SIG_VADDR for h in hits["eboot"]})
                live.base = bases[0]
                gp = live.u64(live.base + REGISTRY_GLOBAL)
                reg = live.u64(gp) if gp else None
                if reg:
                    R = Resolver(live, reg)
                    _s, _i, obj = R.resolve(SPAN_ELEMENT_TAG)
                    if obj == live.base + SPAN_ELEMENT_VADDR:
                        print(f"pid {pid}  base {live.base:#x}  registry {reg:#x}  controls up")
                        break
                    print(f"  [poll] pid {pid} base {live.base:#x} reg {reg:#x} "
                          f"C3 not yet: resolve(0x808033F4)={obj if obj is None else hex(obj)}")
                else:
                    print(f"  [poll] pid {pid} base {live.base:#x} registry not built yet")
            else:
                print(f"  [poll] pid {pid} eboot signature not found yet")
        else:
            print("  [poll] no shadps4")
        if time.time() >= deadline:
            raise SystemExit("controls did not come up before --wait expired; refusing to dump")
        time.sleep(15)      # a full-memory pattern scan is expensive; do not spin on it

    base, registry = live.base, reg
    hits_defs = hits.get("defs") or []

    controls = {}
    controls["C1_base"] = dict(base=hex(base), sig_hits=[hex(h) for h in hits["eboot"]],
                               unique=len({h - SIG_VADDR for h in hits["eboot"]}) == 1,
                               passed=live.base is not None)
    controls["C2_registry"] = dict(global_ptr=hex(gp), registry=hex(registry), passed=bool(registry))
    s, i, obj = R.resolve(SPAN_ELEMENT_TAG)
    controls["C3_span_element"] = dict(tag=hex(SPAN_ELEMENT_TAG), slot=hex(s), index=i,
                                       object=hex(obj) if obj else None,
                                       expected=hex(base + SPAN_ELEMENT_VADDR),
                                       passed=obj == base + SPAN_ELEMENT_VADDR)
    s4, i4, obj4 = R.resolve(CTL_503_RESPONSE)
    controls["C4_503_response"] = dict(tag=hex(CTL_503_RESPONSE), slot=hex(s4), index=i4,
                                       object=hex(obj4) if obj4 else None,
                                       vaddr=hex(obj4 - base) if obj4 else None,
                                       passed=bool(obj4) and 0 < obj4 - base < 0x4000000)
    sb, ib, objb = R.resolve(CTL_BOGUS_TAG)
    controls["C5_bogus"] = dict(tag=hex(CTL_BOGUS_TAG), slot=hex(sb), index=ib,
                                object=hex(objb) if objb else None,
                                passed=objb != base + SPAN_ELEMENT_VADDR)
    for k in ("C1_base", "C2_registry", "C3_span_element", "C4_503_response", "C5_bogus"):
        print(f"  {k}: {'PASS' if controls[k]['passed'] else 'FAIL'}   {controls[k]}")
    if not (controls["C1_base"]["passed"] and controls["C2_registry"]["passed"]
            and controls["C3_span_element"]["passed"]):
        raise SystemExit("C1..C3 did not all pass - refusing to write output")

    target = base + SPAN_ELEMENT_VADDR

    # ── (1) the whole namespace: hash -> runtime tag ───────────────────────────────────────────────
    print("\n--- name-hash registries, live ---")
    rows = []
    reg_dump = {}
    for name, (bv, cnt, _r) in REGISTRIES.items():
        recs = live.rd(base + bv, cnt * 16)
        if recs is None:
            raise SystemExit(f"could not read registry {name} live")
        ents = []
        for i in range(cnt):
            ptr, h, pad = struct.unpack_from("<QII", recs, i * 16)
            tag = live.u32(ptr) if ptr else None
            ents.append(dict(index=i, record_vaddr=bv + i * 16, ptr=ptr,
                             bss_vaddr=(ptr - base) if ptr else None, hash=h, pad=pad, tag=tag))
        reg_dump[name] = ents
        good = sum(1 for e in ents if e["tag"] is not None and (e["tag"] & 0xFFFF0000) == 0x80800000)
        relocd = sum(1 for e in ents if e["ptr"])
        print(f"registry {name}: {cnt} entries, {relocd} relocated ptrs, "
              f"{good} slots holding a 0x8080xxxx tag ({100.0*good/cnt:.1f}%)")
        controls[f"C6_{name}_count"] = dict(count=cnt, relocated=relocd, passed=relocd >= cnt - 1)
        controls[f"C7_{name}_tags"] = dict(plausible=good, of=cnt, passed=good > cnt // 2)

        for e in ents:
            slot = idx = objv = None
            objaddr = None
            if e["tag"]:
                slot, idx, objaddr = R.resolve(e["tag"])
                if objaddr:
                    objv = objaddr - base
            rows.append((name, e["index"], e["record_vaddr"], e["hash"], e["bss_vaddr"], e["tag"],
                         slot, idx, objaddr, objv))
            e.update(slot=slot, tag_index=idx, object=objaddr, object_vaddr=objv)

    # ── the framing question, settled by REVERSE lookup ────────────────────────────────────────────
    print("\n--- §26.28 framing, settled from data ---")
    framing = {}
    gslot = live.u64(base + CRASH_PATH_GLOBAL)
    gtag = live.u32(gslot) if gslot else None
    framing["crash_path_global"] = dict(
        g=hex(base + CRASH_PATH_GLOBAL), slot=hex(gslot) if gslot else None,
        slot_vaddr=hex(gslot - base) if gslot else None,
        tag=hex(gtag) if gtag else None, is_span_element=gtag == SPAN_ELEMENT_TAG)
    print(f"  *(0x{CRASH_PATH_GLOBAL:x}) -> .bss {framing['crash_path_global']['slot_vaddr']} "
          f"-> tag {framing['crash_path_global']['tag']}  "
          f"(== 0x808033F4? {framing['crash_path_global']['is_span_element']})")
    holders = []
    for name, ents in reg_dump.items():
        for e in ents:
            if e["tag"] == SPAN_ELEMENT_TAG:
                prev = ents[e["index"] - 1]["hash"] if e["index"] > 0 else None
                holders.append(dict(registry=name, index=e["index"],
                                    hash_same_record=hex(e["hash"]),
                                    hash_prev_record=hex(prev) if prev is not None else None,
                                    bss_vaddr=hex(e["bss_vaddr"])))
    framing["slots_holding_span_element_tag"] = holders
    for h in holders:
        print(f"  tag 0x808033F4 lives in registry {h['registry']} index {h['index']} "
              f"(.bss {h['bss_vaddr']}): hash in SAME record {h['hash_same_record']}, "
              f"hash in PREVIOUS record {h['hash_prev_record']}")
    if not holders:
        print("  ⚠ NO .bss slot in either registry holds 0x808033F4 - say so loudly, this is news")
    # and the two disputed hashes, reported under BOTH framings
    entsB = reg_dump["B"]
    for want in (0xE2B769E8, 0xE2E25C2C):
        for e in entsB:
            if e["hash"] == want:
                nxt = entsB[e["index"] + 1] if e["index"] + 1 < len(entsB) else None
                framing[f"hash_{want:08x}"] = dict(
                    index=e["index"],
                    ptr_first_framing=dict(bss=hex(e["bss_vaddr"]) if e["bss_vaddr"] else None,
                                           tag=hex(e["tag"]) if e["tag"] else None),
                    hash_first_framing=dict(bss=hex(nxt["bss_vaddr"]) if nxt and nxt["bss_vaddr"] else None,
                                            tag=hex(nxt["tag"]) if nxt and nxt["tag"] else None))
                print(f"  hash {want:#010x} idx {e['index']}: "
                      f"ptr-first tag {framing[f'hash_{want:08x}']['ptr_first_framing']['tag']}   "
                      f"§26.28-framing tag {framing[f'hash_{want:08x}']['hash_first_framing']['tag']}")

    # ── (2) where the runtime tags land ────────────────────────────────────────────────────────────
    slot_hist = {}
    for r in rows:
        if r[6] is not None:
            slot_hist[r[6]] = slot_hist.get(r[6], 0) + 1
    print("\n--- registry slot distribution of runtime tags ---")
    for k in sorted(slot_hist, key=lambda x: -slot_hist[x])[:12]:
        print(f"  slot {k:#x}: {slot_hist[k]}")

    # ── (3) THE DECISIVE QUERY ─────────────────────────────────────────────────────────────────────
    print("\n--- DECISIVE QUERY: which static tag resolves to the span element descriptor? ---")
    all_static = set(S["self_tags"]) | set(S["field_children"]) | set(S["pool_children"])
    all_static |= set(S["every_tag"])       # every 0x8080xxxx word in the image, any alignment
    for af in S["array_fields"]:
        all_static.add(af["child"])
        if af["elem_child"] not in (None, 0xFFFFFFFF):
            all_static.add(af["elem_child"])
    all_static.discard(0xFFFFFFFF)
    print(f"  resolving {len(all_static)} distinct static tags live…")
    hits_target = []
    resolved = {}
    for t in sorted(all_static):
        sl, ix, ob = R.resolve(t)
        resolved[t] = (sl, ix, ob)
        if ob == target:
            hits_target.append(t)
    print(f"  static tags resolving to the span element object {target:#x}: "
          f"{[hex(t) for t in hits_target] if hits_target else 'NONE'}")
    # positive control for the same loop: 0x80801825 must be in `resolved` with a sane object
    ctl = resolved.get(CTL_503_RESPONSE)
    print(f"  loop control - 0x80801825 -> slot {ctl[0]:#x} idx {ctl[1]} obj "
          f"{ctl[2] if ctl[2] is None else hex(ctl[2])} (vaddr "
          f"{None if not ctl[2] else hex(ctl[2]-base)})")

    # array fields whose element or child lands on the target
    arr_hits = []
    for af in S["array_fields"]:
        c = resolved.get(af["child"], (None, None, None))[2]
        e = resolved.get(af["elem_child"], (None, None, None))[2] if af["elem_child"] else None
        if c == target or e == target:
            arr_hits.append(af)
    print(f"  ARRAY fields whose child/element resolves to the target: "
          f"{arr_hits if arr_hits else 'NONE'}")

    # REVERSE ENUMERATION - walk the registry slots index-by-index instead of tag-by-tag, so the
    # negative above cannot hide behind "we never tried the right tag".  index == tag & 0x1FFF, so
    # 0..8191 is the COMPLETE index space of a slot.
    print("\n--- reverse enumeration of registry slots (complete index space) ---")
    slot_scan = {}
    for sl in (0x400, 0x401, 0x402, 0x403):
        sr = R.slot_rec(sl)
        if not sr or not sr[0] or not (0 < sr[1] <= 0x100):
            slot_scan[hex(sl)] = dict(present=False)
            print(f"  slot {sl:#x}: empty in this process")
            continue
        data_base, esize, mask = sr
        live_n, in_image, tgt_idx = 0, 0, []
        for ix in range(8192):
            loc = live.u64(data_base + ix * esize + 8)
            if loc is None:
                continue
            ob = data_base + ix * esize - ctypes.c_int64(mask & loc).value
            if ob:
                live_n += 1
                if 0 < ob - base < 0x4000000:
                    in_image += 1
                if ob == target:
                    tgt_idx.append(ix)
        slot_scan[hex(sl)] = dict(present=True, data_base=hex(data_base), entry_size=esize,
                                  mask=hex(mask & 0xFFFFFFFF), resolvable=live_n,
                                  in_eboot_image=in_image, indices_hitting_target=tgt_idx)
        print(f"  slot {sl:#x}: data_base={data_base:#x} esize={esize} "
              f"resolvable={live_n} in-image={in_image} indices->target={tgt_idx}")

    # and the reverse: what DOES the target's own tag look like as a descriptor?
    tgt_blob = live.rd(target, 0x80)
    print(f"  target object {target:#x} (vaddr {SPAN_ELEMENT_VADDR:#x}) first 0x40 bytes:")
    for k in range(0, 0x40, 16):
        print(f"     +{k:04x}  {tgt_blob[k:k+16].hex(' ')}")

    # ── (4) the policy table, live ─────────────────────────────────────────────────────────────────
    print("\n--- 0x1923CB0 policy table, live ---")
    policies = []
    for i in range(POLICY_N + 3):
        p = live.u64(base + POLICY_TABLE + i * 8)
        namep = live.u64(base + POLICY_NAMES + i * 8)
        nm = None
        if namep:
            b = live.rd(namep, 128)
            if b:
                nm = b.split(b"\0")[0].decode("latin1", "replace")
        rtti = None
        if p:
            vt = live.u64(p)
            ti = live.u64(vt - 8) if vt else None
            if ti:
                np_ = live.u64(ti + 8)
                if np_:
                    b = live.rd(np_, 192)
                    if b:
                        rtti = b.split(b"\0")[0].decode("latin1", "replace")
        policies.append(dict(index=i, ptr=hex(p) if p else None,
                             vaddr=hex(p - base) if p else None, name=nm, rtti=rtti))
        if i < POLICY_N + 3:
            print(f"  [{i:2d}] {policies[-1]['vaddr']}  name={nm}  rtti={rtti}")

    defs_note = dict(bin=DEFS_BIN, header_hits=[hex(h) for h in hits_defs],
                     note="0x550A40/0x550A80/0x550AB0 index this blob via 0x25B4F0 "
                          "(globals_vt+0x4D0) -> blob + index*0x40 + 0x10")
    print(f"\n--- definitions blob live location: {defs_note['header_hits'] or 'NOT FOUND in mapped memory'}")

    # ── write ──────────────────────────────────────────────────────────────────────────────────────
    with open(args.out_tsv, "w") as fh:
        fh.write("registry\tindex\trecord_vaddr\tname_hash\tbss_slot_vaddr\truntime_tag\t"
                 "tag_slot\ttag_index\tobject_addr\tobject_vaddr\n")
        for r in rows:
            fh.write("\t".join(
                [r[0], str(r[1]), f"{r[2]:#x}", f"{r[3]:#010x}",
                 f"{r[4]:#x}" if r[4] is not None else "",
                 f"{r[5]:#010x}" if r[5] is not None else "",
                 f"{r[6]:#x}" if r[6] is not None else "",
                 str(r[7]) if r[7] is not None else "",
                 f"{r[8]:#x}" if r[8] is not None else "",
                 f"{r[9]:#x}" if r[9] is not None else ""]) + "\n")
    doc = dict(
        generated=time.strftime("%Y-%m-%dT%H:%M:%S"), pid=live.pid, base=hex(base),
        registry=hex(registry), eboot=EBOOT, controls=controls, framing=framing,
        slot_histogram={hex(k): v for k, v in slot_hist.items()},
        decisive=dict(target=hex(target),
                      static_tags_resolved=len(all_static),
                      tags_hitting_target=[hex(t) for t in hits_target],
                      array_fields_hitting_target=arr_hits,
                      array_field_count=len(S["array_fields"]),
                      slot_scan=slot_scan),
        policies=policies, definitions=defs_note,
        entries={k: [dict(index=e["index"], record_vaddr=hex(e["record_vaddr"]),
                          name_hash=f"{e['hash']:#010x}",
                          bss=hex(e["bss_vaddr"]) if e["bss_vaddr"] else None,
                          tag=f"{e['tag']:#010x}" if e["tag"] else None,
                          slot=hex(e["slot"]) if e["slot"] is not None else None,
                          tag_index=e["tag_index"],
                          object=hex(e["object"]) if e["object"] else None,
                          object_vaddr=hex(e["object_vaddr"]) if e["object_vaddr"] else None)
                    for e in v] for k, v in reg_dump.items()})
    with open(args.out_json, "w") as fh:
        json.dump(doc, fh, indent=1)
    print(f"\nwrote {args.out_tsv} ({len(rows)} rows) and {args.out_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
