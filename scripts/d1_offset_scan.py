"""Find every access to a struct offset in the decrypted Destiny EBOOT.

Several reverse-engineering questions come down to "what else touches this field?": the
SignOn response field 13 lands in the sign-on singleton at +0x47cc and, as of the 01.24
notes, its only other known use is a reset. A plain string search cannot answer that,
because the field is reached by a displacement inside an `lwz`/`stw`/`ldz`/`std`, not by
a pointer to it.

This scans the code segment for PowerPC load/store instructions whose 16-bit displacement
is the wanted offset and prints them with capstone, so each hit can be checked:

  python scripts/d1_offset_scan.py --elf <EBOOT.elf> --offset 0x47cc
  python scripts/d1_offset_scan.py --elf <EBOOT.elf> --offset 0x47cc --func   # group by function

Only displacements are matched; the base register is reported so you can tell which
register holds the object. Several false positives are expected (the same 16 bits used by
an unrelated instruction form), so verify each hit with `d1_eboot.py disasm`.
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from d1_eboot import Eboot  # noqa: E402

# The D-form load/store family. In 64-bit PowerPC these share the extended opcode, which
# does NOT land in bits 0-5: `stb`/`stw`/`std` are 0b0111110 0 (bits 21-30) so bits 0-5 read
# as 12, while `lbz`/`lwz`/`ld` read as 18. A hand-written bits-0-5 table therefore misses
# every store, so forms are identified by disassembling instead.
DS_FORM = frozenset({
    "lbz", "lbzu", "lhz", "lhzu", "lhax", "lwa", "lwz", "lwzu", "stb", "stbu",
    "sth", "sthu", "stwx", "stw", "std", "stdu", "lmw", "stmw", "ldarx", "stdcx",
})


def is_ds_form(mnemonic):
    return mnemonic in DS_FORM


def _disassembler():
    try:
        import capstone
    except ImportError:  # pragma: no cover - requirements.txt lists capstone
        sys.exit("needs capstone for the instruction check: pip install capstone")
    return capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN)


def scan(eb, offset, lo=None, hi=None):
    """Addresses of D-form load/stores whose displacement field equals `offset`."""
    words, base = eb.words, eb.code_va
    lo = base if lo is None else lo
    hi = base + 4 * len(words) if hi is None else hi
    target = np.uint32(offset & 0xFFFF)
    candidates = [base + 4 * int(i) for i in np.nonzero((words & np.uint32(0xFFFF)) == target)[0]]
    candidates = [pc for pc in candidates if lo <= pc < hi]
    if not candidates:
        return []

    # Disassemble one instruction per candidate instead of a span: a candidate may sit
    # inside another instruction's operand field, and a linear sweep would then decode it
    # as the middle of that instruction and report a bogus mnemonic.
    md = _disassembler()
    hits = []
    for pc in candidates:
        raw = eb.read(pc, 4)
        if not raw:
            continue
        ins = next(md.disasm(raw, pc), None)
        if ins is not None and is_ds_form(ins.mnemonic):
            hits.append(pc)
    return hits


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--elf", required=True, help="decrypted EBOOT")
    ap.add_argument("--offset", required=True, type=lambda v: int(v, 0),
                    help="struct offset whose accesses to find, e.g. 0x47cc")
    ap.add_argument("--lo", type=lambda v: int(v, 0))
    ap.add_argument("--hi", type=lambda v: int(v, 0))
    ap.add_argument("--func", action="store_true", help="also show the containing function")
    a = ap.parse_args(argv)

    eb = Eboot(a.elf)
    hits = scan(eb, a.offset, a.lo, a.hi)
    print(f"{a.elf}: {len(hits)} load/store instructions with displacement {a.offset:#x}")
    for pc in hits:
        start = None
        if a.func:
            start = eb.function_of(pc)[0]
        print(f"  0x{pc:08x}" + (f"  in {eb.name(start)}" if start else ""))
        for line in eb.disasm(pc, 1):
            print(f"      {line}")


if __name__ == "__main__":
    main()
