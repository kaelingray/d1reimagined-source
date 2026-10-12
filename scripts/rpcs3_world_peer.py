"""Read the active world-controller slice's group peer fields from RPCS3.

Run while Destiny is in a mission. BLES01857 01.00 addresses. Read-only.
The group record at slice +0x310 has type 0x80800c6f; its nested
0x80800e4a fields are at +0x318 (i8), +0x319 (state), +0x31a (peer),
and +0x328 (bool). The game's 0x31ca20 compares +0x31a with the local
world-state peer slot at +0x230 (set by 0x25bc10 -> 0x1c9ab8).
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rpcs3_guest_read as guest  # noqa: E402


def u32(data, offset):
    return struct.unpack_from(">I", data, offset)[0]


def i32(data, offset):
    return struct.unpack_from(">i", data, offset)[0]


def main():
    base = guest.find_base()
    if base is None:
        raise SystemExit("Destiny guest memory not found")
    ptr = guest.rpm(base + 0x18EA604, 4)
    if not ptr or len(ptr) != 4:
        raise SystemExit("World-controller slice pointer unavailable")
    slice_addr = u32(ptr, 0)
    data = guest.rpm(base + slice_addr, 0x410)
    if not data or len(data) < 0x410:
        raise SystemExit(f"Could not read slice {slice_addr:#x}")
    print(f"slice={slice_addr:#x} group_zone={i32(data, 0x310)} local_peer_slot={i32(data, 0x230)}")
    print(f"group_state={data[0x319]} group_peer={struct.unpack_from('>b', data, 0x31a)[0]} group_flag={data[0x328]}")
    print(f"group_318_330={data[0x318:0x330].hex()} group_byte_348={data[0x348]}")


if __name__ == "__main__":
    main()
