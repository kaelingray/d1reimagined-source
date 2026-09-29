"""Scan the decrypted Destiny EBOOT for the STUN request/response codec.

01.00 sends two probes (`1e 03 00` = type 30 IP discovery, `14 02 00 xx` = type 20 NAT
discovery) and the stand-in in server/stun_stub.py answers type 31 / 21. 01.24 sends
different probes (`00 01 00 01` and a 29-byte `0e 02 ...`), so the request types are
immediates or table entries in the Demonware STUN client and are not obvious from the
strings. This helper searches the code for the constants that shape the packets:

  --li V         `li rD, V` (opcode 0x38000000 | V) in the Demonware code range
  --word V       a full 32-bit literal equal to V
  --cmpwi V      `cmpwi rD, V`; the immediate is the LOW 16 bits of the word
                 (0x2c030004 = `cmpwi r3, 4`), so only the opcode above them is fixed

Example:
  python scripts/stun_scan.py --elf <EBOOT.elf> --li 29 --word 0x01000100 --cmpwi 0x1e

Addresses are ELF virtual addresses; 01.00 code starts at 0x10000, 01.24 at 0x10000 too
(only the TOC and a few .rodata pointers differ between builds).
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from d1_eboot import Eboot  # noqa: E402

# The Demonware library (bdCore/bdNet/bdSocket/STUN) sits in the low part of the code
# segment in every build examined; the game's own modules start after it.
DEMONWARE_RANGE = (0xA00000, 0xC00000)


def literal(value):
    return np.uint32(value & 0xFFFFFFFF)


def find_li(words, base, value):
    opcode = np.uint32(0x38000000 | (value & 0xFFFF))
    return [base + 4 * int(i) for i in np.nonzero(words == opcode)[0]]


def find_word(words, base, value):
    return [base + 4 * int(i) for i in np.nonzero(words == literal(value))[0]]


def find_cmpwi(words, base, value):
    """`cmpwi rD, SIMM` is primary opcode 11 (bits 26-30), with the CR field in bits 6-10,
    the destination register in bits 11-15 and the signed 16-bit immediate in bits 0-15.
    Verified against capstone: 0x2c030004 decodes as `cmpwi r3, 4`, so the immediate is the
    LOW half of the word, not the high half (a high-half mask matches ~6000 false hits)."""
    opcode = np.uint32(0x2C000000)
    imm = np.uint32(value & 0xFFFF)
    hits = np.nonzero(((words & np.uint32(0xFC000000)) == opcode) & ((words & np.uint32(0xFFFF)) == imm))[0]
    return [base + 4 * int(i) for i in hits]


def report(label, addresses, lo, hi):
    in_range = [a for a in addresses if lo <= a < hi]
    print(f"{label}: {len(addresses)} total, {len(in_range)} in 0x{lo:x}-0x{hi:x}")
    for a in in_range[:20]:
        print(f"    0x{a:08x}")
    outside = [a for a in addresses if not (lo <= a < hi)]
    if outside:
        print(f"    (outside range, first: {' '.join('0x%x' % a for a in outside[:8])})")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--elf", required=True, help="decrypted EBOOT")
    ap.add_argument("--lo", type=lambda v: int(v, 0), default=DEMONWARE_RANGE[0])
    ap.add_argument("--hi", type=lambda v: int(v, 0), default=DEMONWARE_RANGE[1])
    ap.add_argument("--li", type=lambda v: int(v, 0), action="append", default=[])
    ap.add_argument("--word", type=lambda v: int(v, 0), action="append", default=[])
    ap.add_argument("--cmpwi", type=lambda v: int(v, 0), action="append", default=[])
    args = ap.parse_args(argv)
    if not (args.li or args.word or args.cmpwi):
        ap.error("give at least one of --li, --word, --cmpwi")

    eb = Eboot(args.elf)
    words, base = eb.words, eb.code_va
    print(f"{args.elf}: code 0x{base:x}, {len(words)} words, scan 0x{args.lo:x}-0x{args.hi:x}")
    for value in args.li:
        report(f"li {value:#x}", find_li(words, base, value), args.lo, args.hi)
    for value in args.word:
        report(f"word {value:#x}", find_word(words, base, value), args.lo, args.hi)
    for value in args.cmpwi:
        report(f"cmpwi {value:#x}", find_cmpwi(words, base, value), args.lo, args.hi)


if __name__ == "__main__":
    main()
