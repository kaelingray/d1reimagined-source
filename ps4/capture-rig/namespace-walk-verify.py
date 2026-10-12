#!/usr/bin/env python3
"""
namespace-walk-verify.py - re-derives every load-bearing constant in
historicaldocs/design/namespace-walk.md straight from the eboot, offline, in about a minute.

This is the consolidator's independent pass. It exists because four retractions in
two sessions came from a superseded constant propagating into new code
(historicaldocs/design/ATLAS.md §4.3), so the document's numbers must be reproducible from the
image and not from any document's copy of them.

Discipline, per historicaldocs/design/ATLAS.md §4.2: EVERY negative result printed here is
accompanied by a positive control computed in the SAME run. An empty control means
the scanner is broken, not the world.

    python3 capture-rig/namespace-walk-verify.py          # all sections
    python3 capture-rig/namespace-walk-verify.py -v       # + per-item detail

exit 0 iff every check passes.
"""
import argparse
import collections
import struct
import sys

EBOOT = "dumps/patch-01.29-CUSA00219/eboot.bin"

# ---------------------------------------------------------------------------
# Addressing. historicaldocs/design/ATLAS.md §4.1: the bias is PER SEGMENT and must not be
# unified. The two LOAD segments are vaddr = file offset - 0x4000. DYNAMIC,
# dynlibdata and the 0x6ffffxxx segments have p_vaddr = 0 and are addressed by
# RAW FILE OFFSET.
# ---------------------------------------------------------------------------
BIAS = 0x4000
DYNAMIC_RAW = 0x1BCA750
DYNLIBDATA_RAW = 0x1A0D100

DT_SCE_RELA = 0x6100002F
DT_SCE_RELASZ = 0x61000030          # ABSENT in this image -- the trap
DT_SCE_RELA_ALT_SIZE = 0x61000031   # the size actually lives here

# The nine schema-module name-hash registries, by their registrar reference site.
REGISTRAR_SINK = 0xEE0D0
TEARDOWN_SINK = 0xEE2A0
ELEMENT_LIST = 0x129F950            # `lea rdi` operand at 0x114D
ELEMENT_LIST_FIELD = 0x20
BINDER = 0xEE100                    # `call` at 0x1154
TAG_ALLOCATOR = 0x6E9B0

REGISTRY_A = (0x19B3530, 6338)      # spasaha
REGISTRY_B = (0x19D0BD0, 1332)      # spasaha_investment
SPAN_TAG_HOLDER = 0x19CC800         # the slot 0x570A82 reads
SPAN_ELEMENT_OBJ = 0x1492550
SPAN_ELEMENT_HASH = 0xE2E25C2C
SPAN_TAG = 0x808033F4

LEAF_POOL = (0x13D8A40, 0x14A2E00)

# ---------------------------------------------------------------------------

class Eboot:
    def __init__(self, path):
        self.d = open(path, "rb").read()

    def off(self, v):
        return v + BIAS

    def u8(self, v):   return self.d[self.off(v)]
    def u16(self, v):  return struct.unpack_from("<H", self.d, self.off(v))[0]
    def u32(self, v):  return struct.unpack_from("<I", self.d, self.off(v))[0]
    def i32(self, v):  return struct.unpack_from("<i", self.d, self.off(v))[0]
    def u64(self, v):  return struct.unpack_from("<Q", self.d, self.off(v))[0]
    def i64(self, v):  return struct.unpack_from("<q", self.d, self.off(v))[0]
    def raw64(self, o): return struct.unpack_from("<Q", self.d, o)[0]

    def bytes(self, v, n):
        return self.d[self.off(v):self.off(v) + n]

    def cstr(self, v):
        o = self.off(v)
        e = self.d.index(b"\x00", o)
        return self.d[o:e].decode("latin1")

    def relptr(self, v):
        """self-relative pointer: value = &field + *(i64*)&field, 0 meaning NULL"""
        d = self.i64(v)
        return 0 if d == 0 else v + d

    def phdrs(self):
        e_phoff = self.raw64(0x20)
        e_phentsize = struct.unpack_from("<H", self.d, 0x36)[0]
        e_phnum = struct.unpack_from("<H", self.d, 0x38)[0]
        out = []
        for i in range(e_phnum):
            p = e_phoff + i * e_phentsize
            out.append(dict(
                type=struct.unpack_from("<I", self.d, p)[0],
                off=self.raw64(p + 0x08), vaddr=self.raw64(p + 0x10),
                filesz=self.raw64(p + 0x20), memsz=self.raw64(p + 0x28)))
        return out

    def dynamic(self):
        out, o = {}, DYNAMIC_RAW
        while True:
            t = self.raw64(o)
            if t == 0:
                break
            out[t] = self.raw64(o + 8)
            o += 16
        return out

    def rela(self):
        """{r_offset: r_addend}, plus {r_addend: [r_offset,...]}.

        PS4 eboots keep pointer tables ZERO IN THE FILE and fill them at load, so
        raw byte searches for an address find nothing by design -- go through RELA.
        """
        dyn = self.dynamic()
        base = DYNLIBDATA_RAW + dyn[DT_SCE_RELA]
        size = dyn.get(DT_SCE_RELASZ) or dyn[DT_SCE_RELA_ALT_SIZE]
        n = size // 24
        fwd, rev, types = {}, collections.defaultdict(list), collections.Counter()
        for i in range(n):
            p = base + i * 24
            roff, rinfo, radd = (self.raw64(p), self.raw64(p + 8), self.raw64(p + 16))
            fwd[roff] = radd
            rev[radd].append(roff)
            types[rinfo & 0xFFFFFFFF] += 1
        return fwd, rev, types, n

    def xref4(self, target, lo=0, hi=None):
        """call rel32 (e8), jmp rel32 (e9) into `target`. ATLAS §4.2."""
        hi = hi if hi is not None else self.text_end()
        hits = collections.defaultdict(list)
        d = self.d
        for i in range(lo, hi - 5):
            b = d[self.off(i)]
            if b == 0xE8 or b == 0xE9:
                disp = struct.unpack_from("<i", d, self.off(i) + 1)[0]
                if i + 5 + disp == target:
                    hits["call" if b == 0xE8 else "jmp"].append(i)
        return hits

    def text_end(self):
        return self.phdrs()[0]["filesz"]

    def count_u32(self, v):
        pat = struct.pack("<I", v)
        n, i = 0, self.d.find(pat)
        while i != -1:
            n += 1
            i = self.d.find(pat, i + 1)
        return n


class Checks:
    def __init__(self, verbose):
        self.ok = 0
        self.bad = []
        self.verbose = verbose

    def section(self, title):
        print("\n== " + title)

    def check(self, name, got, want, note=""):
        good = got == want
        if good:
            self.ok += 1
        else:
            self.bad.append(name)
        mark = "[PASS]" if good else "[FAIL]"
        line = f"  {mark} {name:<62} {note}"
        print(line.rstrip())
        if not good or self.verbose:
            print(f"         got={got!r} want={want!r}")
        return good

    def note(self, text):
        print("         " + text)


def tag_of(container_id, index):
    """0x6E9B0, read off the instructions (see section 5)."""
    return 0x80000000 | ((container_id << 13) & 0x7FFE000) | (index & 0x1FFF)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eboot", default=EBOOT)
    ap.add_argument("-v", "--verbose", action="store_true")
    a = ap.parse_args()

    eb = Eboot(a.eboot)
    c = Checks(a.verbose)
    print(f"eboot {a.eboot}")

    # -- 0. segments -------------------------------------------------------
    c.section("0. program headers -- the bias is per-segment (ATLAS §4.1)")
    loads = [p for p in eb.phdrs() if p["type"] == 1]
    c.check("two LOAD segments", len(loads), 2)
    l1 = loads[1]
    nobits = (l1["vaddr"] + l1["filesz"], l1["vaddr"] + l1["memsz"])
    c.check("LOAD[1] vaddr/filesz/memsz",
            (hex(l1["vaddr"]), hex(l1["filesz"]), hex(l1["memsz"])),
            ("0x18c0000", "0x149088", "0x1690dc0"),
            f"NOBITS 0x{nobits[0]:x}..0x{nobits[1]:x}")

    # -- 1. RELA -----------------------------------------------------------
    c.section("1. DYNAMIC / RELA, addressed by RAW FILE OFFSET")
    dyn = eb.dynamic()
    c.check("DT_SCE_RELASZ (0x61000030) is ABSENT -- the trap",
            DT_SCE_RELASZ in dyn, False,
            "size lives in tag 0x61000031; assuming the standard tag KeyErrors")
    fwd, rev, types, nrel = eb.rela()
    c.check("RELA entry count", nrel, 73825,
            f"0x{dyn[DT_SCE_RELA_ALT_SIZE]:x} / 24")
    c.check("reloc types", dict(types), {8: 70682, 6: 22, 1: 3121})
    c.check("min r_offset (nothing below the descriptor tables relocates)",
            hex(min(fwd)), "0x18c0038")

    # -- 2. the constant that caused two retractions ----------------------
    c.section("2. RELA[0x19CC800] -- the decimal->hex slip that caused §26.28/§26.31")
    add = fwd[SPAN_TAG_HOLDER]
    c.check("RELA[0x19CC800].r_addend == 0x28FD0B0 (NOT 0x2900BF0)",
            hex(add), "0x28fd0b0", f"decimal {add}")
    c.check("exactly two r_offsets carry addend 0x28FD0B0",
            [hex(x) for x in sorted(rev[0x28FD0B0])], ["0x19cc800", "0x19d54c0"],
            "the container slot-array holder, and registry-B[1167]'s pointer")
    c.check("CONTROL: 0x2900BF0 is a real address, owned by a different entry",
            [hex(x) for x in sorted(rev[0x2900BF0])], ["0x19d5850"],
            "so the wrong constant was plausible, not absurd")

    # -- 3. record framing -------------------------------------------------
    c.section("3. registry record framing: {u64 ptr, u32 hash, u32 pad}")

    def framing(base, n, ptr_first):
        unrel, pads, hashes = 0, 0, []
        for i in range(n):
            r = base + i * 16
            po = r if ptr_first else r + 8
            ho = r + 8 if ptr_first else r
            pa = r + 12 if ptr_first else r + 4
            if po not in fwd:
                unrel += 1
            hashes.append(eb.u32(ho))
            if eb.u32(pa) != 0:
                pads += 1
        asc = all(hashes[i] < hashes[i + 1] for i in range(len(hashes) - 1))
        return unrel, pads, asc, hashes

    for base, n, label in ((REGISTRY_A[0], REGISTRY_A[1], "A"),
                           (REGISTRY_B[0], REGISTRY_B[1], "B")):
        u_pf, p_pf, a_pf, H = framing(base, n, True)
        u_hf, _, _, _ = framing(base + 8, n, False)
        c.check(f"registry {label} ptr-first: 0 unrelocated pointers", u_pf, 0,
                f"base 0x{base:X}  pads {p_pf}  hash ascending {a_pf}")
        if label == "B":
            c.check("registry B hash-first (§26.28): 1 unrelocated -- rejects it",
                    u_hf, 1, f"base 0x{base + 8:X}")
            HB = H
    i1167, i1166 = HB.index(SPAN_ELEMENT_HASH), HB.index(0xE2B769E8)
    c.check("hash 0xE2E25C2C is registry-B index 1167", i1167, 1167)
    c.check("hash 0xE2B769E8 is registry-B index 1166", i1166, 1166)
    c.check("ptr-first B[1167] slot == RELA[0x19CC800]",
            hex(fwd[REGISTRY_B[0] + i1167 * 16]), "0x28fd0b0")
    c.check("CONTROL: 0xDEADBEEF is not a registry-B hash",
            0xDEADBEEF in HB, False, "so 'found at 1167' is a measurement")

    # -- 4. nine registries ------------------------------------------------
    c.section("4. NINE registries, not two -- registrar-derived counts")
    reg_hits = eb.xref4(REGISTRAR_SINK)
    c.check("0xEE0D0 has 9 references (2 call + 7 jmp)",
            sum(len(v) for v in reg_hits.values()), 9,
            str({k: [hex(x) for x in v] for k, v in reg_hits.items()}) if a.verbose else "")
    ctl = eb.xref4(0x56FFB0)
    c.check("MANDATORY CONTROL: 0x56FFB0 <- tail jmp at 0x560D27 (ATLAS §4.2)",
            [hex(x) for x in ctl.get("jmp", [])], ["0x560d27"])

    tables, total = [], 0
    for kind, addrs in reg_hits.items():
        for site in sorted(addrs):
            w = eb.bytes(site - 0x60, 0x65)
            li = w.rfind(bytes.fromhex("488d3d"))
            if li < 0:
                li = w.rfind(bytes.fromhex("488d05"))
            tv = (site - 0x60 + li + 7) + struct.unpack_from("<i", w, li + 3)[0]
            mi = w.rfind(b"\xbe")
            cnt = struct.unpack_from("<I", w, mi + 1)[0]
            tables.append((site, tv, cnt))
            total += cnt
    c.check("nine registries enlist 15520 entries in total", total, 15520)
    counts = sorted(x[2] for x in tables)
    c.check("counts are the nine module sizes", counts,
            sorted([300, 308, 2992, 1189, 2007, 1014, 6338, 1332, 40]))

    # module names, from the matching teardown sites
    td = eb.xref4(TEARDOWN_SINK)
    names = {}
    for kind, addrs in td.items():
        for site in sorted(addrs):
            base = site - 0x140
            w = eb.bytes(base, 0x145)
            j = 0
            while True:
                j = w.find(bytes.fromhex("488d"), j)
                if j < 0 or j + 7 > len(w):
                    break
                if w[j + 2] in (0x3D, 0x35, 0x05, 0x15, 0x0D, 0x1D, 0x25, 0x2D):
                    t = (base + j + 7) + struct.unpack_from("<i", w, j + 3)[0]
                    try:
                        s = eb.cstr(t)
                    except Exception:
                        s = ""
                    # 'ai' is two characters -- do not raise this lower bound
                    if 1 < len(s) < 32 and s.isascii() and all(ch.isalnum() or ch == "_" for ch in s):
                        names[site] = s
                j += 1
    c.check("nine teardown sites, nine module names",
            sorted(names.values()),
            sorted(["well_known", "foundation", "engine", "ai", "investment",
                    "renderer", "spasaha", "spasaha_investment", "audio"]))

    # -- 5. the tag allocator ---------------------------------------------
    c.section("5. tag allocator 0x6E9B0 -- prologue FIRST (ATLAS §4.5)")
    c.check("prologue 55 48 89 e5 at 0x6E9B0",
            eb.bytes(TAG_ALLOCATOR, 4).hex(), "554889e5",
            f"file offset 0x{eb.off(TAG_ALLOCATOR):x}")
    win = eb.bytes(TAG_ALLOCATOR, 0x90)
    for pat, desc in (("418b4634", "mov eax,[r14+0x34]   (container id field)"),
                      ("c1e30d", "shl ebx,0xd"),
                      ("81e1ff1f0000", "and ecx,0x1fff       (index)"),
                      ("81e300e0ff07", "and ebx,0x7ffe000    (the mask §26.31 omitted)"),
                      ("81cb00000080", "or  ebx,0x80000000")):
        k = win.find(bytes.fromhex(pat))
        c.check(f"0x{TAG_ALLOCATOR + k:X}  {pat:<14}", k >= 0, True, desc)
    c.check("tag(0x401, 0x13F4) == 0x808033F4 (span element)",
            hex(tag_of(0x401, 0x13F4)), "0x808033f4")
    c.check("tag(0x402, 0x1F08) == 0x80805F08 (registry-B[1166], a DIFFERENT type)",
            hex(tag_of(0x402, 0x1F08)), "0x80805f08")

    c.section("6. the tag's inputs are compile-time immediates")
    for va, want in ((0xE9791, 0x400), (0xE97B1, 0x401), (0xE97D1, 0x402)):
        b = eb.bytes(va, 5)
        c.check(f"0x{va:X}: mov edx,0x{want:X}", (b[0], struct.unpack_from("<I", b, 1)[0]),
                (0xBA, want), b.hex())
    b = eb.bytes(0x114D, 12)
    lea_t = 0x1154 + struct.unpack_from("<i", b, 3)[0]
    call_t = 0x1159 + struct.unpack_from("<i", eb.d, eb.off(0x1154) + 1)[0]
    c.check("0x114D lea rdi -> the global element list 0x129F950",
            hex(lea_t), "0x129f950", b.hex())
    c.check("0x1154 call -> the binder 0xEE100", hex(call_t), hex(BINDER))

    # -- 7. the element list ----------------------------------------------
    c.section("7. the global element list -- 14816 elements, static records")
    lst = eb.relptr(ELEMENT_LIST + ELEMENT_LIST_FIELD)
    n = eb.i64(lst)
    c.check("element count", n, 14816, f"list -> 0x{lst:X} (self-relative)")
    base = lst + 8
    elems, conts, objs = [], collections.Counter(), []
    for i in range(n):
        r = base + i * 16
        obj = eb.relptr(r)
        cont, idx = struct.unpack_from("<HH", eb.d, eb.off(r) + 8)
        elems.append((obj, cont, idx, eb.u32(obj + 8)))
        conts[cont] += 1
        objs.append(obj)
    c.check("container histogram", dict(conts), {0: 4960, 1: 4274, 2: 5582},
            "slots 0x400 / 0x401 / 0x402")
    c.check("every element object is distinct", len(set(objs)), n)
    c.check("all objects lie in one contiguous chain",
            (hex(min(objs)), hex(max(objs))), ("0x12d97e0", "0x14a2da8"))
    byhash = {h: o for o, _, _, h in elems}
    c.check("14816 distinct name hashes", len(byhash), n)
    c.check("span element resolves", hex(byhash[SPAN_ELEMENT_HASH]), hex(SPAN_ELEMENT_OBJ))

    # -- 8. the span element descriptor -----------------------------------
    c.section("8. the span element descriptor object 0x1492550")
    O = SPAN_ELEMENT_OBJ
    c.check("+0x08 name hash", hex(eb.u32(O + 8)), hex(SPAN_ELEMENT_HASH),
            "bytes 2c 5c e2 e2 at 0x1492558")
    c.check("+0x10 child tag == 0xFFFFFFFF (LEAF)", hex(eb.u32(O + 0x10)), "0xffffffff")
    c.check("+0x14 serialized width == 16 bits", eb.u32(O + 0x14), 16)
    c.check("+0x00 size", hex(eb.u32(O)), "0x54")
    c.check("+0x48 csize == 0 (leaf)", eb.u32(O + 0x48), 0)

    # -- 9. computed, never stored ----------------------------------------
    c.section("9. tags are COMPUTED, never STORED (negative + control, one run)")
    for v in (SPAN_TAG, 0x80802AAF):
        c.check(f"u32 0x{v:08X} occurs 0 times in the 29MB image", eb.count_u32(v), 0,
                "a tag appears as a literal only where another descriptor cites it")
    c.check("CONTROL: 0xDEADBEEF does occur", eb.count_u32(0xDEADBEEF) > 0, True,
            f"{eb.count_u32(0xDEADBEEF)} occurrences -- the scanner works")
    c.check("CONTROL: 0x80801825 occurs (it IS cited as a child tag)",
            eb.count_u32(0x80801825), 1)

    # -- 10. the .bss slots are NOBITS ------------------------------------
    c.section("10. every registry .bss slot is NOBITS")
    slots = [fwd[REGISTRY_B[0] + i * 16] for i in range(REGISTRY_B[1])]
    slots += [fwd[REGISTRY_A[0] + i * 16] for i in range(REGISTRY_A[1])]

    def classify(v):
        for p in loads:
            if p["vaddr"] <= v < p["vaddr"] + p["filesz"]:
                return "FILE"
            if p["vaddr"] + p["filesz"] <= v < p["vaddr"] + p["memsz"]:
                return "NOBITS"
        return "OUTSIDE"

    h = collections.Counter(classify(s) for s in slots)
    c.check("all 7670 A+B slots are NOBITS", dict(h), {"NOBITS": 7670},
            f"range 0x{min(slots):x}..0x{max(slots):x} -- no in-file word exists")
    c.check("CONTROL: a known file-resident address classifies FILE",
            classify(SPAN_ELEMENT_OBJ), "FILE")

    # -- 11. the descriptor table is self-delimiting ----------------------
    c.section("11. the descriptor table is ONE self-delimiting chain, not stride 0x58")
    o, walked, lens = LEAF_POOL[0], [], collections.Counter()
    while o < LEAF_POOL[1]:
        sz = eb.u32(o)
        if sz == 0 or sz > 0x400:
            break
        ln = sz + 4 if eb.u32(o + sz) == 0 else sz
        walked.append(o)
        lens[ln] += 1
        o += ln
    c.check("walk terminates EXACTLY at 0x14A2E00", hex(o), "0x14a2e00")
    c.check("9188 objects in the leaf region", len(walked), 9188,
            f"lengths {dict(lens)}")
    W = set(walked)
    c.check("all 9188 are elements of the global list", len(W & set(objs)), 9188)
    stride = set(0x1464340 + k * 0x58 for k in range(2918))
    c.check("the stride-0x58 model agrees with the real walk on ZERO objects",
            len(stride & W), 0, "it reads every record 8 bytes low (at the marker)")
    c.check("0x1492550 is a walked object", SPAN_ELEMENT_OBJ in W, True)
    c.check("CONTROL: its marker 0x1492548 is NOT", 0x1492548 in W, False)
    widths = [eb.u32(x + 0x14) for x in walked]
    ten = {1, 4, 8, 12, 16, 24, 32, 40, 48, 64}
    c.check("width column has a long tail (§26.29's ten-value claim is wrong)",
            len(set(widths)) > 10, True,
            f"{len(set(widths))} distinct; {sum(1 for x in widths if x not in ten)} outside the ten")
    c.check("0x14A2E00 begins a DIFFERENT record family", hex(eb.u32(0x14A2E00)), "0xc0",
            "read at stride 0x58 this size word is §26.29's phantom 'kind 192'")

    # -- 12. the span trailer ---------------------------------------------
    c.section("12. the span is hard-coded in 0x261FB0, not schema-described")
    for va, pre, desc in ((0x2626C0, "e8", "call 0x73430  read ONE bit"),
                          (0x2626C7, "0f84", "je 0x26278A  absent => {NULL,0}"),
                          (0x2626D4, "be10000000", "mov esi,0x10  16-bit BYTE LENGTH"),
                          (0x2626D9, "e8", "call 0x72C70  read 16 bits"),
                          (0x2626DE, "41894718", "mov [r15+0x18],eax")):
        c.check(f"0x{va:X}", eb.bytes(va, 6).hex().startswith(pre), True,
                f"{eb.bytes(va, 6).hex()}  {desc}")
    c.check("0x2626C0 -> 0x73430", hex(0x2626C5 + eb.i32(0x2626C1)), "0x73430")
    c.check("0x2626D9 -> 0x72C70", hex(0x2626DE + eb.i32(0x2626DA)), "0x72c70")
    c.check("0x2626C7 -> 0x26278A", hex(0x2626CD + eb.i32(0x2626C9)), "0x26278a")

    # -- 13. kind high byte -----------------------------------------------
    c.section("13. `kind & 0x100` is a presence-bit WIDTH, not an array flag")
    for va, pat, desc in ((0x105DF05, "43807ce53100", "cmp byte [r13+r12*8+0x31],0  (marker+0x09 alone)"),
                          (0x105E0D2, "410fb630", "movzx esi,byte [r8]          (r8 = &marker+0x09)"),
                          (0x105E0D6, "01d6", "add esi,edx                  (ADDED to the bit offset)")):
        c.check(f"0x{va:X}  {pat}", eb.bytes(va, len(pat) // 2).hex(), pat, desc)

    # -- 14. the definition table is a DIFFERENT name space ---------------
    c.section("14. the 101 message-definition name hashes are a DISJOINT name space")
    try:
        dd = open("capture-rig/schemas/investment-message-definitions.bin", "rb").read()
        cnt = struct.unpack_from("<I", dd, 0x18)[0]
        present = sum(1 for i in range(cnt)
                      if struct.unpack_from("<I", dd, 0x40 + i * 0x40)[0] in byhash)
        c.check("0 of 101 definition-row hashes are element hashes", (present, cnt), (0, 101))
        c.check("CONTROL: known element hashes DO test present",
                all(h in byhash for h in (SPAN_ELEMENT_HASH, 0xE2B769E8, 0x000119B4)), True,
                "so the membership test is not vacuous")
    except FileNotFoundError:
        print("  [SKIP] investment-message-definitions.bin not found")

    print("\n" + "=" * 74)
    if c.bad:
        print(f"FAILED {len(c.bad)} of {c.ok + len(c.bad)} checks:")
        for b in c.bad:
            print("   " + b)
        return 1
    print(f"ALL {c.ok} CHECKS PASSED - historicaldocs/design/namespace-walk.md re-derives from the image.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
