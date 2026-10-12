#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""d1guest - ONE instrument for the live client's runtime state.

WHY THIS EXISTS
===============
This project keeps hitting the same wall from different directions: **the answer is in BSS or the
heap, so static analysis stops.**  `0x263D0F8` (the profile_setup config globals), `*(0x26A3890)`
(the character:signin selection node), the QueueZ instance-type checksums, the package registry's
eboot-resident slot `0x400` - none of them exist in the file.

Each time, a bespoke script re-derived the guest base by byte-signature match and re-implemented
`/proc/<pid>/mem` access.  There were five: `peek-guest.py`, `dump-registry.py`, `inject-roster.py`,
`find-in-guest.py`, `dump-descriptors.py`.  Five copies of one mechanism means five places for a
superseded constant to survive a retraction (ATLAS §4.3) and five chances to get the base wrong.
(All but `dump-registry.py` and `find-in-guest.py` are archived to
`historicaldocs/python-capture/` as of 2026-08-11; the read side of THIS file is itself now ported
to `d1-live`, so the consolidation continues one level up.)

This is that mechanism, once, with the controls made mandatory instead of optional.
`historicaldocs/design/runtime-state-access.md` is the plan of record; read it before using this.

THE THREE THINGS THIS TOOL REFUSES TO DO WITHOUT A CONTROL
==========================================================
1. **Recover the base.**  ⚠ A WRONG BASE YIELDS PLAUSIBLE-LOOKING BYTES RATHER THAN AN ERROR.
   So: two independent anchors must agree (as `peek-guest.py` does), *and* a third address must
   compare equal by direct read, *and* a shifted window at that same address must compare
   UNequal - a negative control, so an all-zero or all-nop region cannot pass by being degenerate.
   Any FAIL and the tool exits non-zero without reading.

2. **Prove the client is alive.**  ⚠ Registry OBJECTS are cleared on connection teardown while
   DESCRIPTORS survive at state 3 (ATLAS §4.6b), so a read after a disconnect shows zeros -
   indistinguishable from "our delivery never worked".  That produced two confident wrong readings
   in one session.  Every command prints a LIVENESS line; commands that read registry object
   *contents* require L3 (the account object carries our SOID) unless `--allow-dead` is passed.
   **A read after death is an error, not a zero.**

3. **Say where an address came from.**  Addresses live in `capture-rig/guest-symbols.json` with a
   `source` and a `confidence` each, gated on the eboot's SHA-256.  `0x263D0F8` is
   `profile_setup_config_block`, not a magic number in a script.

USAGE
=====
    ./d1guest.py base                          # recover + print all three controls
    ./d1guest.py live                          # the liveness ladder, L0..L3
    ./d1guest.py syms [pattern]                # what names exist, and where each came from
    ./d1guest.py selftest                      # reproduce KNOWN results (ATLAS §4.4d)

    ./d1guest.py read profile_setup_kind                     # by name
    ./d1guest.py read 0x263D0F8 --len 0x270                  # by vaddr
    ./d1guest.py read character_selection_manager --deref --struct character_selection_node
    ./d1guest.py read boot_state_names --len 320 --as ptr    # relocated slots: zero in the file

    ./d1guest.py obj 4:0                       # the account object (family 4, type 0)
    ./d1guest.py obj 3:1 --key 0xD100000000000101   # resolve by KEY, not index
    ./d1guest.py tag 0x80801825                # package-registry tag -> object
    ./d1guest.py descriptors [--watch]         # QueueZ replication descriptor table

    ./d1guest.py watch profile_setup_kind character_selection_manager --for 60

    ./d1guest.py poke <target> <value> --experiment NAME --confirm   # see WRITES below

WRITES
======
`poke` is a **hop mechanic**, and it is the lowest-quality one this project has (see
`historicaldocs/design/runtime-state-access.md` §"Hop mechanics").  A live write makes everything downstream
`synthetic_ancestor` **twice over** - the value never crossed a wire, so it is not even evidence
about our own server.  It is legitimate for exactly one thing: **identifying** a gate.  It is
self-deception the moment it is used to *satisfy* one.

So `poke` requires `--experiment <name>` and `--confirm`, prints the taint banner, and appends a
JSONL record to `runs/guest-writes.jsonl` so a write can never be silently forgotten between the
run that did it and the finding that cites it.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SYMBOLS = os.path.join(HERE, "guest-symbols.json")
WRITE_JOURNAL = os.path.join(REPO, "runs", "guest-writes.jsonl")
BASE_CACHE = "/tmp/d1guest-base.json"

PT_LOAD = 1


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# The eboot image: per-segment vaddr↔file mapping.
#
# ⚠ ATLAS §4.1: THE BIAS IS PER SEGMENT.  `BIAS = 0x4000` is correct for the two LOAD segments of
# THIS image and is a coincidence of it, not a property of the format - the metadata segments have
# p_vaddr == 0 and are addressed by raw file offset.  Every existing script hardcodes 0x4000; this
# one parses the program headers so a future image cannot silently corrupt every read.
# ──────────────────────────────────────────────────────────────────────────────────────────────────
class Image:
    def __init__(self, path):
        self.path = path
        self.blob = open(path, "rb").read()
        if self.blob[:4] != b"\x7fELF":
            raise SystemExit(f"{path}: not an ELF")
        phoff = struct.unpack_from("<Q", self.blob, 0x20)[0]
        phes = struct.unpack_from("<H", self.blob, 0x36)[0]
        phn = struct.unpack_from("<H", self.blob, 0x38)[0]
        self.loads = []          # (vaddr, filesz, memsz, offset)
        for i in range(phn):
            p = phoff + i * phes
            ptype = struct.unpack_from("<I", self.blob, p)[0]
            off, va, _pa, fsz, msz = struct.unpack_from("<QQQQQ", self.blob, p + 8)[:5]
            if ptype == PT_LOAD:
                self.loads.append((va, fsz, msz, off))
        if not self.loads:
            raise SystemExit(f"{path}: no PT_LOAD segments")

    def sha256(self):
        return hashlib.sha256(self.blob).hexdigest()

    def region_of(self, vaddr):
        """'file' | 'bss' | None - which part of the image a vaddr falls in."""
        for va, fsz, msz, _off in self.loads:
            if va <= vaddr < va + fsz:
                return "file"
            if va <= vaddr < va + msz:
                return "bss"
        return None

    def read(self, vaddr, n):
        """File-backed bytes at a vaddr, or None if the range is BSS / outside the image."""
        for va, fsz, _msz, off in self.loads:
            if va <= vaddr and vaddr + n <= va + fsz:
                fo = off + (vaddr - va)
                return self.blob[fo:fo + n]
        return None


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# The symbol file
# ──────────────────────────────────────────────────────────────────────────────────────────────────
def _int(v):
    return int(v, 0) if isinstance(v, str) else v


class Symbols:
    def __init__(self, path=SYMBOLS):
        self.path = path
        self.doc = json.load(open(path))
        self.by_name = {s["name"]: s for s in self.doc["symbols"]}
        self.structs = self.doc.get("structs", {})
        self.layout = {k: _int(v) if isinstance(v, str) else v
                       for k, v in self.doc["registry_layout"].items()
                       if k not in ("confidence", "source")}
        self.families = {(f["family"], f["type"]): f for f in self.doc["families"]}

    def check_build(self, image):
        want = self.doc["build"]["sha256"]
        got = image.sha256()
        ok = want == got
        return ok, (f"BUILD GATE: {'PASS' if ok else 'FAIL'}  eboot sha256 {got[:16]}… "
                    f"{'==' if ok else '!='} symbol-file {want[:16]}…")

    def resolve(self, token):
        """name | 0xvaddr -> (vaddr, symbol_or_None)."""
        if token in self.by_name:
            s = self.by_name[token]
            return _int(s["vaddr"]), s
        try:
            return int(token, 0), None
        except ValueError:
            raise SystemExit(f"unknown symbol and not a number: {token!r}  "
                             f"(try `d1guest.py syms`)")


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# The process
# ──────────────────────────────────────────────────────────────────────────────────────────────────
def emu_pids():
    # ⚠ `pgrep -x shadps4` matches NOTHING - the comm is `shadPS4:Main`.  The `[s]` character class
    # keeps the pattern from matching the shell that evaluates it.
    out = subprocess.run(["pgrep", "-f", "[s]hadps4 -p "], capture_output=True, text=True).stdout
    return [int(x) for x in out.split()]


def proc_starttime(pid):
    """Field 22 of /proc/<pid>/stat - makes a cached base pid-recycle-proof."""
    with open(f"/proc/{pid}/stat") as f:
        data = f.read()
    return int(data[data.rindex(")") + 2:].split()[19])


class Proc:
    def __init__(self, pid):
        self.pid = pid
        self.starttime = proc_starttime(pid)
        self.rf = open(f"/proc/{pid}/mem", "rb", 0)
        self._wf = None

    # -- liveness -----------------------------------------------------------------------------
    def alive(self):
        try:
            return proc_starttime(self.pid) == self.starttime
        except (FileNotFoundError, ProcessLookupError, ValueError):
            return False

    def require_alive(self, what):
        if not self.alive():
            raise Dead(f"the client died during {what} - this is an ERROR, not a zero")

    # -- memory -------------------------------------------------------------------------------
    def read(self, addr, n):
        self.require_alive("a read")
        self.rf.seek(addr)
        data = self.rf.read(n)
        if data is None or len(data) != n:
            self.require_alive("a read")
            raise Dead(f"short read at {addr:#x}: wanted {n}, got {0 if data is None else len(data)}")
        return data

    def write(self, addr, data):
        if self._wf is None:
            self._wf = open(f"/proc/{self.pid}/mem", "wb", 0)
        self.require_alive("a write")
        self._wf.seek(addr)
        return self._wf.write(data)

    # -- mappings -----------------------------------------------------------------------------
    def regions(self, executable_only=False):
        """Readable mappings with RESIDENT pages.

        ⚠ Do NOT filter by region SIZE.  shadPS4 reserves enormous sparse ranges (87 TiB in one
        measurement) and the guest heap lives INSIDE them; an earlier tool skipped anything over
        1 GiB and thereby searched 0.01% of memory while reporting a confident "0 hits".
        Filter by Rss from smaps instead - we touch only committed memory.
        """
        out, cur = [], None
        for line in open(f"/proc/{self.pid}/smaps"):
            m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S{4})", line)
            if m:
                cur = (int(m.group(1), 16), int(m.group(2), 16), m.group(3))
                continue
            if cur and line.startswith("Rss:"):
                rss = int(line.split()[1]) * 1024
                if rss > 0 and "r" in cur[2] and (not executable_only or "x" in cur[2]):
                    out.append(cur)
                cur = None
        return out

    def find(self, sig, executable_only=False, limit=16):
        hits, scanned = [], 0
        for lo, hi, _perm in self.regions(executable_only):
            try:
                self.rf.seek(lo)
                blob = self.rf.read(hi - lo)
            except Exception:
                continue
            if not blob:
                continue
            scanned += len(blob)
            start = 0
            while True:
                j = blob.find(sig, start)
                if j < 0:
                    break
                hits.append(lo + j)
                start = j + 1
                if len(hits) >= limit:
                    return hits, scanned
        return hits, scanned


class Dead(RuntimeError):
    """The client is not alive enough for this read to mean anything."""


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# Base recovery - ONE mechanism, THREE controls
# ──────────────────────────────────────────────────────────────────────────────────────────────────
class BaseControl:
    def __init__(self):
        self.lines = []
        self.ok = True
        self.base = None

    def check(self, name, ok, detail):
        self.lines.append(f"  {'PASS' if ok else 'FAIL'}  {name}: {detail}")
        self.ok = self.ok and ok
        return ok

    def report(self):
        head = (f"BASE CONTROL: {'PASS' if self.ok else 'FAIL'}"
                + (f"  base={self.base:#x}" if self.base is not None else ""))
        return "\n".join([head] + self.lines)


def recover_base(proc, image, syms, full_scan=False, use_cache=True, verbose=True):
    """(base, BaseControl).  Recovered ONCE per process and cached; the cheap checks re-run always."""
    ctl = BaseControl()

    cached = None
    if use_cache and os.path.exists(BASE_CACHE):
        try:
            c = json.load(open(BASE_CACHE))
            if c.get("pid") == proc.pid and c.get("starttime") == proc.starttime \
                    and c.get("sha256") == syms.doc["build"]["sha256"]:
                cached = c["base"]
        except Exception:
            cached = None

    if cached is not None:
        ctl.base = cached
        ctl.check("anchors", True, f"cached for pid {proc.pid} (start {proc.starttime}); "
                                   f"scan skipped - the two checks below still run every time")
    else:
        anchors = syms.doc["anchors"]
        if len(anchors) < 2:
            raise SystemExit("guest-symbols.json needs at least two anchors")
        sets, details = [], []
        for a in anchors:
            va, n = _int(a["vaddr"]), a["len"]
            sig = image.read(va, n)
            if sig is None:
                raise SystemExit(f"anchor {a['name']} at {va:#x} is not file-backed")
            hits, scanned = proc.find(sig, executable_only=not full_scan)
            sets.append({h - va for h in hits})
            details.append(f"{a['name']}@{va:#x} -> {len(hits)} hit(s) in {scanned/2**20:.0f} MiB")
        agreed = sorted(set.intersection(*sets))
        ok = ctl.check("anchors", len(agreed) == 1,
                       "; ".join(details) + f" => agreed {[hex(b) for b in agreed]}"
                       + ("" if len(agreed) == 1 else "  (need EXACTLY one)"))
        if not ok:
            if not full_scan:
                ctl.lines.append("  note: only EXECUTABLE resident mappings were scanned; "
                                 "retry with --full-scan")
            return None, ctl
        ctl.base = agreed[0]

    base = ctl.base

    # Control 2 - an INDEPENDENT address, by direct comparison rather than by scan.  A scan that
    # found the right bytes says nothing about whether the arithmetic that turned them into a base
    # was right; this does.
    v = syms.doc["verify"]
    va, n = _int(v["vaddr"]), v["len"]
    want = image.read(va, n)
    try:
        got = proc.read(base + va, n)
    except Dead as e:
        ctl.check("verify", False, str(e))
        return None, ctl
    ctl.check("verify", got == want, f"{n} bytes at {va:#x} "
                                     f"{'match' if got == want else 'DIFFER from'} the file image")

    # Control 3 - the NEGATIVE control.  ⚠ Control 2 alone can pass for the wrong reason: if the
    # verify window is degenerate (all zero, all 0xCC, a run of nops) then almost any base matches.
    # Read the SAME window shifted by one byte; it must NOT match.  If it does, the window carries
    # no information and control 2 proved nothing.
    try:
        shifted = proc.read(base + va + 1, n)
    except Dead as e:
        ctl.check("negative control", False, str(e))
        return None, ctl
    discriminates = shifted != want
    ctl.check("negative control", discriminates,
              "the same window shifted by 1 byte "
              + ("differs - the window discriminates, so the verify above means something"
                 if discriminates else
                 "ALSO MATCHES - the window is degenerate, so the verify above proved nothing"))

    if not ctl.ok:
        return None, ctl

    if cached is None:
        try:
            json.dump({"pid": proc.pid, "starttime": proc.starttime, "base": base,
                       "sha256": syms.doc["build"]["sha256"]}, open(BASE_CACHE, "w"))
        except OSError:
            pass
    return base, ctl


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# The registry, and the liveness ladder
# ──────────────────────────────────────────────────────────────────────────────────────────────────
class Guest:
    """A live client, with a proven base.  This is the thing other scripts should import."""

    def __init__(self, proc, image, syms, base):
        self.proc, self.image, self.syms, self.base = proc, image, syms, base
        self._registry = None

    # -- primitive reads ----------------------------------------------------------------------
    def read(self, vaddr, n):
        return self.proc.read(self.base + vaddr, n)

    def read_abs(self, addr, n):
        return self.proc.read(addr, n)

    def u32(self, vaddr):
        return struct.unpack("<I", self.read(vaddr, 4))[0]

    def u64(self, vaddr):
        return struct.unpack("<Q", self.read(vaddr, 8))[0]

    # -- the object registry ------------------------------------------------------------------
    @property
    def registry(self):
        if self._registry is None:
            self._registry = self.u64(_int(self.syms.by_name["object_registry_ptr"]["vaddr"]))
        return self._registry

    def family_slot(self, family, typ):
        """(address, cap, stride, control_lines).

        POSITIVE CONTROL, and it is the whole point of doing it this way: the element address is
        derived TWICE - once from the client's RUNTIME entry table, once from the STATIC family/slot
        mapper table in guest-symbols.json (read out of 0x56C550, eboot-map §3).  They must agree,
        and so must cap and stride.  Two independent derivations landing on one address is what a
        recovered base plus a registry pointer cannot otherwise give you.
        """
        L = self.syms.layout
        reg = self.registry
        ent = reg + family * L["entry_stride_family"] + typ * L["entry_stride_type"] \
            + L["entry_table_base"]
        off, cap, stride = struct.unpack("<IHxxI", self.proc.read(ent, 12))
        lines = []
        if off == 0xFFFFFFFF:
            return None, cap, stride, [f"  FAIL  family {family}/{typ}: entry offset is -1 "
                                       f"(region not allocated in this process)"]
        addr_rt = reg + L["element_base"] + off
        static = self.syms.families.get((family, typ))
        if static is None:
            lines.append(f"  ----  family {family}/{typ}: no static entry in guest-symbols.json, "
                         f"cross-check SKIPPED")
        else:
            addr_st = reg + _int(static["storage"])
            ok = (addr_rt == addr_st and cap == static["cap"]
                  and stride == _int(static["size"]))
            lines.append(f"  {'PASS' if ok else 'FAIL'}  family {family}/{typ} cross-check: "
                         f"runtime {addr_rt:#x} cap={cap} stride={stride:#x} vs static "
                         f"{addr_st:#x} cap={static['cap']} stride={_int(static['size']):#x}"
                         f"  [{static['identity'] or '-'}]")
            if not ok:
                return None, cap, stride, lines
        return addr_rt, cap, stride, lines

    def find_by_key(self, family, typ, key):
        addr, cap, stride, lines = self.family_slot(family, typ)
        if addr is None:
            return None, -1, lines
        for i in range(cap):
            if struct.unpack("<Q", self.proc.read(addr + i * stride, 8))[0] == key:
                return addr + i * stride, i, lines
        return None, -1, lines + [f"  ----  no element in {cap} slot(s) carries key {key:#018x}"]

    # -- the package registry (schema/content tags) -------------------------------------------
    @staticmethod
    def tag_slot(tag):
        eax = ctypes.c_int32(tag).value
        esi = eax >> 13
        edx = (ctypes.c_uint32(esi).value >> 0x12) | 0x3FF
        return (ctypes.c_int32(edx).value & esi) & 0xFFFFFFFF, tag & 0x1FFF

    def package_registry(self):
        p1 = self.u64(_int(self.syms.by_name["package_registry_global"]["vaddr"]))
        if not p1:
            return None
        return struct.unpack("<Q", self.proc.read(p1, 8))[0]

    def resolve_tag(self, tag):
        reg = self.package_registry()
        if not reg:
            return None, "package registry pointer is NULL - the client has not built it yet"
        slot, index = self.tag_slot(tag)
        rec = self.proc.read(reg + slot * 0x40, 0x40)
        data_base = struct.unpack_from("<Q", rec, 0x10)[0]
        esize = struct.unpack_from("<i", rec, 0x30)[0]
        mask = struct.unpack_from("<i", rec, 0x34)[0]
        if not data_base or esize <= 0:
            return None, f"slot {slot:#x} is empty in this process"
        ent = data_base + index * esize
        loc = struct.unpack("<Q", self.proc.read(ent + 8, 8))[0]
        # `and rbx,[rsi+rax+8]` then `sub rsi,rbx`: the mask is a sign-extended i32 and the
        # subtraction is SIGNED, so the masked loc is a signed 64-bit BACK-offset.
        return ent - ctypes.c_int64(mask & loc).value, \
            f"slot {slot:#x} index {index} data_base={data_base:#x} entry_size={esize}"

    # -- LIVENESS ------------------------------------------------------------------------------
    def liveness(self, account_soid):
        """(level, lines).  L0 process · L1 base · L2 registry built · L3 objects live."""
        lines, level = [], 0
        if not self.proc.alive():
            return 0, ["  L0 FAIL  the process is gone"]
        lines.append(f"  L0 PASS  pid {self.proc.pid} alive")
        level = 1
        lines.append(f"  L1 PASS  base {self.base:#x} (see BASE CONTROL)")
        try:
            reg = self.registry
        except Dead as e:
            return 1, lines + [f"  L2 FAIL  {e}"]
        if not reg:
            return 1, lines + ["  L2 FAIL  object_registry_ptr is NULL - the client has not built "
                               "the registry yet"]
        lines.append(f"  L2 PASS  object registry {reg:#x}")
        level = 2
        fam = self.syms.doc["liveness"]["account_family"]
        typ = self.syms.doc["liveness"]["account_type"]
        addr, cap, stride, ctl = self.family_slot(fam, typ)
        if addr is None:
            return 2, lines + ctl + [f"  L3 FAIL  cannot locate family {fam}/{typ}"]
        key = struct.unpack("<Q", self.proc.read(addr, 8))[0]
        if key == account_soid:
            lines.append(f"  L3 PASS  account (fam{fam}/t{typ}) key={key:#018x} == expected - "
                         f"registry objects are LIVE")
            level = 3
        else:
            lines.append(f"  L3 FAIL  account (fam{fam}/t{typ}) key={key:#018x} != expected "
                         f"{account_soid:#018x}. ⚠ Objects are cleared on connection teardown "
                         f"while descriptors survive; a read now is indistinguishable from "
                         f"'delivery never worked' (ATLAS §4.6b).")
        return level, lines


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# Formatting
# ──────────────────────────────────────────────────────────────────────────────────────────────────
def hexdump(b, addr=0, label=None):
    out = []
    for i in range(0, len(b), 16):
        chunk = b[i:i + 16]
        txt = "".join(chr(c) if 32 <= c < 127 else "." for c in chunk)
        pre = f"{addr + i:#014x}" if label is None else f"{label}+{i:#06x}"
        out.append(f"  {pre}  {chunk.hex(' '):<47}  {txt}")
    return "\n".join(out)


FMT = {"u8": ("<B", 1), "u16": ("<H", 2), "u32": ("<I", 4), "u64": ("<Q", 8),
       "i32": ("<i", 4), "i64": ("<q", 8), "ptr": ("<Q", 8)}


def show_struct(guest, sdef, data, vaddr, name):
    print(f"  struct {name} ({sdef['size']} bytes)  {sdef.get('confidence','')}  "
          f"{sdef.get('source','')}")
    for f in sdef["fields"]:
        fmt, size = FMT[f["type"]]
        off = f["off"]
        if off + size > len(data):
            print(f"    +{off:#06x} {f['name']:<12} <past the end of the read>")
            continue
        val = struct.unpack_from(fmt, data, off)[0]
        note = f"   {f['note']}" if f.get("note") else ""
        print(f"    +{off:#06x} {f['name']:<12} {f['type']:<4} = {val:#x} ({val}){note}")


# ──────────────────────────────────────────────────────────────────────────────────────────────────
# Commands
# ──────────────────────────────────────────────────────────────────────────────────────────────────
def open_guest(args, need_base=True):
    syms = Symbols(args.symbols)
    image = Image(args.eboot or os.path.join(REPO, syms.doc["build"]["eboot"]))
    ok, line = syms.check_build(image)
    print(line)
    if not ok and not args.ignore_build_gate:
        raise SystemExit("refusing: the symbol file was measured against a different eboot. "
                         "Every vaddr in it is suspect. Pass --ignore-build-gate only if you have "
                         "re-verified the addresses yourself.")
    pids = emu_pids()
    if not pids:
        raise SystemExit("no running shadps4 (pgrep -f '[s]hadps4 -p ') - "
                         "a runtime read with no runtime is an ERROR, not an empty result")
    if len(pids) > 1:
        raise SystemExit(f"{len(pids)} emulators running ({pids}); refusing to guess. "
                         f"One emulator, one owner (ATLAS §4.6).")
    proc = Proc(pids[0])
    if not need_base:
        return syms, image, proc, None
    base, ctl = recover_base(proc, image, syms, full_scan=args.full_scan,
                             use_cache=not args.no_cache)
    print(ctl.report())
    if base is None:
        raise SystemExit(2)
    return syms, image, proc, Guest(proc, image, syms, base)


def print_liveness(guest, args, require=0):
    soid = int(args.account_soid, 0) if args.account_soid else \
        _int(guest.syms.doc["liveness"]["account_soid"])
    level, lines = guest.liveness(soid)
    print(f"LIVENESS: L{level}")
    print("\n".join(lines))
    if level < require and not args.allow_dead:
        raise SystemExit(f"refusing: this read needs L{require} and the client is at L{level}. "
                         f"Pass --allow-dead to read anyway, and label the result accordingly.")
    return level


def cmd_base(args):
    open_guest(args)
    return 0


def cmd_live(args):
    _s, _i, _p, guest = open_guest(args)
    print_liveness(guest, args)
    return 0


def cmd_syms(args):
    syms = Symbols(args.symbols)
    pat = args.pattern
    print(f"{'name':<32} {'vaddr':>12} {'type':<7} {'reg':<7} conf  source")
    for s in syms.doc["symbols"]:
        if pat and pat not in s["name"]:
            continue
        print(f"{s['name']:<32} {_int(s['vaddr']):>#12x} {s.get('type',''):<7} "
              f"{s.get('region',''):<7} {s.get('confidence','')}  {s.get('source','')}")
        if s.get("note"):
            print(f"{'':<32} {s['note']}")
    if not pat:
        print("\nstructs: " + ", ".join(syms.structs))
        print("families: " + ", ".join(f"{f}/{t}={syms.families[(f,t)]['identity'] or '-'}"
                                       for f, t in sorted(syms.families)))
    return 0


def cmd_read(args):
    syms, image, proc, guest = open_guest(args)
    vaddr, sym = syms.resolve(args.target)
    if sym:
        print(f"symbol {sym['name']} @ {vaddr:#x}  {sym.get('confidence','')}  "
              f"region={sym.get('region','?')}  {sym.get('source','')}")
        if sym.get("note"):
            print(f"  note: {sym['note']}")
    print_liveness(guest, args, require=args.require)

    n = args.len or (sym.get("len") if sym else None) or 64
    struct_name = args.struct or (sym.get("struct") if sym else None)
    if struct_name and not args.len:
        n = max(n, syms.structs[struct_name]["size"])

    addr = vaddr if args.abs else guest.base + vaddr
    label = None
    if args.deref:
        ptr = struct.unpack("<Q", proc.read(addr, 8))[0]
        print(f"[{args.target}] = {ptr:#018x}")
        if ptr == 0:
            print("  NULL - nothing to dump. ⚠ For a heap object this may mean the client has not "
                  "built it yet OR that it has been torn down; check LIVENESS above.")
            return 0
        addr = ptr
        label = "*"

    data = proc.read(addr, n)
    where = image.region_of(vaddr)
    print(f"guest {vaddr:#x} -> host {addr:#x}, {n} bytes"
          + (f"   [image region: {where}]" if where and not args.deref else ""))

    if args.as_ == "hex" or not args.as_:
        print(hexdump(data, addr))
    if args.as_ in ("u32", "u64", "ptr"):
        fmt, size = FMT[args.as_]
        for i in range(0, len(data) - size + 1, size):
            v = struct.unpack_from(fmt, data, i)[0]
            extra = ""
            if args.as_ == "ptr" and v and guest.base <= v < guest.base + (1 << 32):
                extra = f"   -> guest {v - guest.base:#x}"
            print(f"  +{i:#06x}  {v:#0{size*2+2}x}{extra}")
    if struct_name:
        show_struct(guest, syms.structs[struct_name], data, vaddr, struct_name)

    # If the range is file-backed, showing the DIFF against the file is free and is often the
    # whole point - `boot_state_names` is zero in the file and populated by relocation at load.
    if where == "file" and not args.deref:
        fdata = image.read(vaddr, n)
        if fdata is not None:
            same = fdata == data
            print(f"  vs FILE image: {'IDENTICAL' if same else 'DIFFERS'}"
                  + ("" if same else f" in {sum(a != b for a, b in zip(fdata, data))}/{n} bytes"
                                     f" - relocations, patches or runtime writes"))
    return 0


def cmd_obj(args):
    syms, image, proc, guest = open_guest(args)
    print_liveness(guest, args, require=3)
    parts = args.spec.split(":")
    fam, typ = int(parts[0], 0), int(parts[1], 0)
    idx = int(parts[2], 0) if len(parts) > 2 else 0
    if args.key is not None:
        addr, idx, lines = guest.find_by_key(fam, typ, int(args.key, 0))
        print("REGISTRY CONTROL:")
        print("\n".join(lines))
        if addr is None:
            raise SystemExit(2)
        print(f"family {fam}/{typ} key {int(args.key,0):#018x} found at index {idx}")
    else:
        base_addr, cap, stride, lines = guest.family_slot(fam, typ)
        print("REGISTRY CONTROL:")
        print("\n".join(lines))
        if base_addr is None:
            raise SystemExit(2)
        if idx >= cap:
            raise SystemExit(f"index {idx} >= capacity {cap}")
        addr = base_addr + idx * stride
    n = args.len or 128
    data = proc.read(addr, n)
    print(f"element @ {addr:#x}, {n} bytes")
    print(hexdump(data, addr))
    if args.struct:
        show_struct(guest, syms.structs[args.struct], data, 0, args.struct)
    return 0


def cmd_tag(args):
    syms, image, proc, guest = open_guest(args)
    print_liveness(guest, args)
    for t in args.tags:
        tag = int(t, 0)
        obj, why = guest.resolve_tag(tag)
        if obj is None:
            print(f"tag {tag:#010x}: {why}")
            continue
        print(f"tag {tag:#010x}: {why}  -> object @ {obj:#x}")
        print(hexdump(proc.read(obj, args.len or 128), obj))
    return 0


def cmd_descriptors(args):
    syms, image, proc, guest = open_guest(args)
    print_liveness(guest, args)
    L = guest.syms.layout
    reg = guest.registry
    STATE = {0: "free", 1: "1 CLAIMED (update REFUSED)", 2: "2 SUBSCRIBED (update accepted)",
             3: "3 REPLICATING (update accepted)", 4: "4 LOST/TIMED-OUT (update REFUSED)"}
    ERR = {0: "-", 1: "unknown", 2: "no_content", 3: "moved", 4: "not_found",
           5: "service_unavailable", 6: "unauthorized", 7: "TIMEOUT"}
    last = None
    while True:
        tbl = proc.read(reg + L["descriptor_table"],
                        L["descriptor_count"] * L["descriptor_stride"])
        rows = []
        for i in range(L["descriptor_count"]):
            d = tbl[i * L["descriptor_stride"]:(i + 1) * L["descriptor_stride"]]
            key, state, err = struct.unpack_from("<Q", d, 0x20)[0], d[0x28], d[0x29]
            if state or key:
                rows.append(f"  [{i:3}] class={d[0]:<3} key={key:#018x}  "
                            f"state={STATE.get(state, state)}  err={ERR.get(err, err)}  "
                            f"ver={struct.unpack_from('<i', d, 0x3c)[0]}")
        out = "\n".join(rows) or "  (no claimed descriptors)"
        if out != last:
            print(time.strftime("%H:%M:%S"))
            print(out, flush=True)
            last = out
        if not args.watch:
            return 0
        time.sleep(2)


def cmd_watch(args):
    syms, image, proc, guest = open_guest(args)
    print_liveness(guest, args)
    targets = [(t,) + syms.resolve(t) for t in args.targets]
    deadline = time.time() + args.for_ if args.for_ else None
    last = {}
    while deadline is None or time.time() < deadline:
        try:
            proc.require_alive("the watch loop")
        except Dead as e:
            print(f"WATCH ENDED: {e}")
            return 3
        for name, vaddr, sym in targets:
            n = (sym.get("len") if sym else None) or 8
            n = min(n, 64)
            data = proc.read(guest.base + vaddr, n)
            if last.get(name) != data:
                stamp = time.strftime("%H:%M:%S")
                print(f"{stamp}  {name} @ {vaddr:#x}  {data.hex(' ')}")
                last[name] = data
        time.sleep(args.interval)
    return 0


def cmd_poke(args):
    syms, image, proc, guest = open_guest(args)
    level = print_liveness(guest, args, require=args.require)
    if not args.confirm or not args.experiment:
        raise SystemExit("poke requires --experiment <name> and --confirm. A live write is a "
                         "HOP MECHANIC of the lowest evidential quality; see "
                         "historicaldocs/design/runtime-state-access.md before using it.")
    vaddr, sym = syms.resolve(args.target)
    fmt, size = FMT[args.type]
    value = int(args.value, 0)
    addr = (vaddr if args.abs else guest.base + vaddr) + args.offset
    if args.deref:
        addr = struct.unpack("<Q", proc.read(addr, 8))[0] + args.offset
        if not addr:
            raise SystemExit("deref gave NULL - refusing to write to offset 0")
    before = proc.read(addr, size)
    proc.write(addr, struct.pack(fmt, value))
    after = proc.read(addr, size)
    ok = after == struct.pack(fmt, value)
    print("=" * 96)
    print("⚠ CLIENT MODIFICATION. Everything downstream of this run is `synthetic_ancestor` TWICE")
    print("  OVER - the value never crossed a wire, so it is not evidence about the real service")
    print("  and not even evidence about our own server. A poke can IDENTIFY a gate; it cannot")
    print("  SATISFY one. Report the crash, if any, in the same breath as the advance.")
    print("=" * 96)
    print(f"  target   {args.target} +{args.offset:#x} -> host {addr:#x}")
    print(f"  before   {before.hex(' ')}")
    print(f"  after    {after.hex(' ')}   {'OK' if ok else '*** READBACK MISMATCH ***'}")
    rec = {"t": time.strftime("%Y-%m-%dT%H:%M:%S"), "pid": proc.pid, "experiment": args.experiment,
           "target": args.target, "vaddr": vaddr, "offset": args.offset, "host_addr": addr,
           "type": args.type, "value": value, "before": before.hex(), "after": after.hex(),
           "liveness": level, "readback_ok": ok}
    os.makedirs(os.path.dirname(WRITE_JOURNAL), exist_ok=True)
    with open(WRITE_JOURNAL, "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(f"  journalled to {WRITE_JOURNAL}")
    return 0 if ok else 2


def cmd_selftest(args):
    """Reproduce results this project ALREADY KNOWS.

    ATLAS §4.4d: a new tool is not evidence until it has reproduced a known result.
    `ready-byte-report.py` printed NEVER FIRED for three runs because it read probe_id at the
    field index that actually holds record_size - output indistinguishable from the failure it
    was built to detect. So this command exists, and a claim made with this tool that has not
    been preceded by a passing selftest in the same session is not a claim.
    """
    syms, image, proc, guest = open_guest(args)
    results = []

    def t(name, ok, detail):
        results.append((name, ok, detail))
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {detail}")

    print("SELFTEST (known results, ATLAS §4.4d):")

    # 1. eboot-map §3's family/slot table vs the client's runtime entry table, for every family we
    #    have a static row for.  This is the cross-check that a wrong base would break.
    agree = 0
    checked = 0
    for (fam, typ) in sorted(syms.families):
        addr, cap, stride, lines = guest.family_slot(fam, typ)
        checked += 1
        if addr is not None:
            agree += 1
        else:
            print("   " + lines[-1])
    t("family/slot table (eboot-map §3) vs runtime entry table",
      agree == checked, f"{agree}/{checked} families agree on address, capacity and stride")

    # 2. The character-roster stride is 0xA0 - inject-roster.py refuses to write if it is not.
    _a, _c, stride, _l = guest.family_slot(3, 0)
    t("roster stride", stride == 0xA0, f"family 3/0 stride = {stride:#x} (expected 0xA0, §26.61)")

    # 3. A file-backed read must reproduce the file exactly.  If this fails the base is wrong even
    #    though the anchors agreed - the strongest single check in the tool.
    va = _int(syms.doc["verify"]["vaddr"])
    n = 256
    t("file-backed read == file image",
      proc.read(guest.base + va, n) == image.read(va, n),
      f"{n} bytes at {va:#x}")

    # 4. A BSS read must NOT be servable from the file (it isn't in the file at all) - this is the
    #    control that says "this instrument is reading something static analysis cannot".
    bss = _int(syms.by_name["object_registry_ptr"]["vaddr"])
    t("object_registry_ptr is BSS", image.region_of(bss) == "bss",
      f"{bss:#x} is in a PT_LOAD memsz range beyond filesz - no file read can answer it")

    # 5. The relocated boot-state name table: zero in the file, real pointers live.
    va = _int(syms.by_name["boot_state_names"]["vaddr"])
    fdata = image.read(va, 64)
    ldata = proc.read(guest.base + va, 64)
    fzero = fdata is not None and set(fdata) == {0}
    t("relocation control (boot_state_names)", fzero and set(ldata) != {0},
      f"file bytes all-zero={fzero}, live bytes all-zero={set(ldata) == {0}} "
      f"- PS4 eboots keep pointer tables zero in the file and fill them at load (ATLAS §4.2)")

    bad = [n for n, ok, _ in results if not ok]
    print(f"\nSELFTEST: {len(results) - len(bad)}/{len(results)} PASS"
          + (f"   FAILED: {', '.join(bad)}" if bad else ""))
    return 0 if not bad else 1


# ──────────────────────────────────────────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser(
        prog="d1guest",
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__)
    ap.add_argument("--symbols", default=SYMBOLS)
    ap.add_argument("--eboot", default=None)
    ap.add_argument("--ignore-build-gate", action="store_true")
    ap.add_argument("--full-scan", action="store_true",
                    help="scan ALL resident readable mappings for the anchors, not just executable "
                         "ones (slower; use if the anchor control fails)")
    ap.add_argument("--no-cache", action="store_true", help="re-scan for the base even if cached")
    ap.add_argument("--allow-dead", action="store_true",
                    help="read even though the liveness predicate failed. ⚠ The result is then "
                         "indistinguishable from 'delivery never worked' - label it.")
    ap.add_argument("--account-soid", default=os.environ.get("D1_ACCOUNT_SOID"),
                    help="override the liveness account SOID (default: $D1_ACCOUNT_SOID or the "
                         "symbol file)")
    ap.add_argument("--require", type=int, default=2, metavar="L",
                    help="minimum liveness level for this read (default 2 = registry built)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("base", help="recover the guest base and print the three controls")
    sub.add_parser("live", help="print the liveness ladder")
    sub.add_parser("selftest", help="reproduce known results before trusting this tool")

    p = sub.add_parser("syms", help="list symbols, with source and confidence")
    p.add_argument("pattern", nargs="?")

    p = sub.add_parser("read", help="read a guest vaddr or symbol")
    p.add_argument("target")
    p.add_argument("--len", type=lambda s: int(s, 0), default=None)
    p.add_argument("--deref", action="store_true", help="read the qword there, then dump AT it")
    p.add_argument("--abs", action="store_true", help="target is already a host address")
    p.add_argument("--struct", default=None)
    p.add_argument("--as", dest="as_", choices=["hex", "u32", "u64", "ptr"], default=None)

    p = sub.add_parser("obj", help="a server-object registry element, by family:type[:index]")
    p.add_argument("spec", metavar="FAM:TYPE[:IDX]")
    p.add_argument("--key", default=None, help="resolve by KEY instead of index")
    p.add_argument("--len", type=lambda s: int(s, 0), default=None)
    p.add_argument("--struct", default=None)

    p = sub.add_parser("tag", help="resolve a 0x8080xxxx schema/content tag through the package "
                                   "registry")
    p.add_argument("tags", nargs="+")
    p.add_argument("--len", type=lambda s: int(s, 0), default=None)

    p = sub.add_parser("descriptors", help="the QueueZ replication descriptor table")
    p.add_argument("--watch", action="store_true")

    p = sub.add_parser("watch", help="poll symbols, printing only on change")
    p.add_argument("targets", nargs="+")
    p.add_argument("--interval", type=float, default=1.0)
    p.add_argument("--for", dest="for_", type=float, default=0)

    p = sub.add_parser("poke", help="WRITE guest memory (a hop mechanic - read the doc first)")
    p.add_argument("target")
    p.add_argument("value")
    p.add_argument("--type", choices=list(FMT), default="u64")
    p.add_argument("--offset", type=lambda s: int(s, 0), default=0)
    p.add_argument("--deref", action="store_true")
    p.add_argument("--abs", action="store_true")
    p.add_argument("--experiment", default=None)
    p.add_argument("--confirm", action="store_true")

    args = ap.parse_args()
    fn = {"base": cmd_base, "live": cmd_live, "syms": cmd_syms, "read": cmd_read, "obj": cmd_obj,
          "tag": cmd_tag, "descriptors": cmd_descriptors, "watch": cmd_watch, "poke": cmd_poke,
          "selftest": cmd_selftest}[args.cmd]
    try:
        return fn(args)
    except Dead as e:
        print(f"DEAD: {e}", file=sys.stderr)
        return 3
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
