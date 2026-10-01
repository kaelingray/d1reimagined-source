"""Find every access to a struct offset in the decrypted Destiny EBOOT.

Several reverse-engineering questions come down to "what else touches this field?": the
SignOn response field 13 lands in the sign-on singleton at +0x47cc and, as of the 01.24
notes, its only other known use is a reset. A plain string search cannot answer that,
because the field is reached by a displacement inside an `lwz`/`stw`/`ld`/`std`, not by
a pointer to it.

This scans the code segment for PowerPC D-form and DS-form load/store instructions whose
displacement is the wanted offset and prints them with capstone, so each hit can be checked:

  python scripts/d1_offset_scan.py --elf <EBOOT.elf> --offset 0x47cc
  python scripts/d1_offset_scan.py --elf <EBOOT.elf> --offset 0x47cc --func   # show containing function

Only displacements are matched; the disassembly reports the base register so you can tell
which register holds the object. Signed offsets (such as -8) and their 16-bit encodings
(0xfff8) are equivalent. Indexed instructions have no displacement and are excluded.
Hits may access an unrelated object, or be data in the executable segment; verify each
hit with `d1_eboot.py disasm`.
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from d1_eboot import Eboot  # noqa: E402

def _check_offset(offset):
    if not -0x8000 <= offset <= 0xFFFF:
        raise ValueError("offset must be signed 16-bit or a 16-bit encoding (-0x8000..0xffff)")
    return offset & 0xFFFF


def _offset_arg(text):
    try:
        value = int(text, 0)
        _check_offset(value)
        return value
    except ValueError as exc:
        raise argparse.ArgumentTypeError(str(exc)) from exc


def scan(eb, offset, lo=None, hi=None):
    """Addresses of D-/DS-form loads/stores with this signed or encoded displacement."""
    words, base = eb.words, eb.code_va
    lo = base if lo is None else lo
    hi = base + 4 * len(words) if hi is None else hi
    target = np.uint32(_check_offset(offset))
    op = words >> 26
    # The primary opcode is the top six bits of a big-endian instruction word.
    # 32..55 are D-form integer/floating loads/stores, including lmw/stmw.
    d_form = (op >= 32) & (op <= 55)
    # DS-form has a signed 14-bit field multiplied by four; the bottom two bits
    # select ld/ldu/lwa (58: 0/1/2) or std/stdu (62: 0/1). Other subtypes are reserved.
    subtype = words & np.uint32(3)
    ds_form = ((op == 58) & (subtype <= 2)) | ((op == 62) & (subtype <= 1))
    matches = ((d_form & ((words & np.uint32(0xFFFF)) == target)) |
               (ds_form & ((words & np.uint32(0xFFFC)) == target)))
    return [base + 4 * int(i) for i in np.nonzero(matches)[0]
            if lo <= base + 4 * int(i) < hi]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--elf", required=True, help="decrypted EBOOT")
    ap.add_argument("--offset", required=True, type=_offset_arg,
                    help="signed or encoded 16-bit struct offset, e.g. 0x47cc or -8")
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
