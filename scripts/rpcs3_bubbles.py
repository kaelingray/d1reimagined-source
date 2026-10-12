"""List the running activity's bubbles (read-only, BLES01857 01.00).

The object table `*(0x18a9440)` holds the scenario's bubble zone handles at +0x100c (22 for
act_cosmo_m0_highway) and the active zone set at +0x110c (bubble base = 8 * bubble). Each handle
is resolved through the tag table (rpcs3_tiger_types.resolve) and its definition dumped: size
words, printable strings, and the tag handles (0x80xxxxxx) it references. Also prints the
zone-selection hashes at 0x190a220 / 0x190a224.

Usage: python scripts/rpcs3_bubbles.py [BYTES]     (definition bytes to scan, default 0x400)
"""

import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
_argv, sys.argv = sys.argv, sys.argv[:1]
import rpcs3_tiger_types as tiger  # noqa: E402

sys.argv = _argv
u32, resolve, rpm, B = tiger.u32, tiger.resolve, tiger.g.rpm, tiger.B


def main():
    length = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0x400
    for addr in (0x190A220, 0x190A224):
        print(f"{addr:#x} = {u32(addr):#010x}")
    table = u32(0x18A9440)
    print(f"object table {table:#x}, active zone set {struct.unpack('>i', struct.pack('>I', u32(table + 0x110C)))[0]}")
    for n in range(22):
        handle = u32(table + 0x100C + n * 4)
        try:
            definition = resolve(handle)
        except ValueError:
            print(f"bubble {n:2}: {handle:#x} unresolved")
            continue
        raw = rpm(B + definition, length) or b""
        words = struct.unpack(f">{len(raw) // 4}I", raw[: len(raw) // 4 * 4])
        tags = sorted({w for w in words if w >> 24 == 0x80 and w != handle})
        strings = [s.decode() for s in re.findall(rb"[ -~]{5,}", raw)]
        print(f"bubble {n:2}: {handle:#x} -> {definition:#x}  head {' '.join(f'{w:08x}' for w in words[:8])}")
        print(f"    tags {' '.join(f'{t:#x}' for t in tags[:24])}")
        if strings:
            print(f"    strings {strings[:12]}")


if __name__ == "__main__":
    main()
