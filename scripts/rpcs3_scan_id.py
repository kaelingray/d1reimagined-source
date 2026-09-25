"""Read-only scan of the current RPCS3 guest memory for one u64 object ID."""

import sys

import rpcs3_guest_read as guest


def main():
    wanted = int(sys.argv[1], 16)
    start = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0
    end = int(sys.argv[3], 16) if len(sys.argv) > 3 else 0x20000000
    base = guest.find_base()
    if base is None:
        raise RuntimeError("RPCS3 guest memory was not found")
    needles = [("be", wanted.to_bytes(8, "big")), ("le", wanted.to_bytes(8, "little"))]
    matches = []
    size = 0x10000
    for addr in range(start, end, size):
        data = guest.rpm(base + addr, min(size, end - addr))
        if data is None:
            continue
        for order, needle in needles:
            offset = data.find(needle)
            while offset != -1:
                matches.append((addr + offset, order))
                offset = data.find(needle, offset + 1)
    for addr, order in matches:
        print(f"{addr:#x} {order}")
    print(f"{len(matches)} matches")


if __name__ == "__main__":
    main()
