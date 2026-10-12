"""Read occupied 01.24 replication records from an explicitly observed simulation.

Use r3 at 0x1df644/0x1df358 as --sim. The old 01.00 helper treats pool +0x14
as a record count and reads free slots. This reader follows the occupancy bitmap
used by 0x1df358 instead. Reads are private, read-only and non-atomic.
"""
import argparse
import datetime
import json
import struct
from collections import Counter
from pathlib import Path


def snapshot(read, sim):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    pool = u32(sim)
    header = read(pool, 0x40)
    data, capacity, word_14, trailer, stride = (struct.unpack_from(">I", header, off)[0]
                                               for off in (8, 0x10, 0x14, 0x18, 0x1c))
    if stride != 0x48 or trailer != 0x44 or not 0 < capacity <= 0x2000:
        raise ValueError("simulation pool has an unsupported replication record layout")
    bitmap_descriptor = u32(u32(pool) + 8)
    descriptor = read(bitmap_descriptor, 12)
    byte_count, bit_capacity, bits = struct.unpack(">III", descriptor)
    if not capacity <= bit_capacity <= 0x4000 or byte_count != (bit_capacity + 7) // 8 or byte_count % 4:
        raise ValueError("simulation occupancy bitmap has inconsistent bounds")
    bitmap = read(bits, byte_count)
    words = struct.unpack(f">{byte_count // 4}I", bitmap)
    records = []
    for index in range(capacity):
        if not words[index // 32] & (1 << (index % 32)):
            continue
        address = data + index * stride
        raw = read(address, 0x44)
        key, kind, object_index = struct.unpack_from(">IBxH", raw)
        state_b, state_a = struct.unpack_from(">II", raw, 8)
        pending = raw[0x15]
        if not key or kind > 0x7f or pending not in (0, 1) or not 0x80800000 <= state_a < 0x82000000:
            raise ValueError("occupied simulation slot has an invalid replication record header")
        has_b = state_b != 0xffffffff
        records.append({"pool_index": index, "address": f"0x{address:08x}",
                        "group": f"0x{key:08x}", "kind": kind, "index": object_index,
                        "state_a": f"0x{state_a:08x}",
                        "state_b": f"0x{state_b:08x}" if has_b else None,
                        "awaiting_baseline": pending,
                        "bubble": struct.unpack_from(">i", raw, 0x3c)[0],
                        "baseline": f"{key:#x}/{kind:#x}/{object_index}" + ("/1" if has_b else "")})
    if (u32(sim) != pool or read(pool, 0x40) != header
            or read(bitmap_descriptor, 12) != descriptor or read(bits, byte_count) != bitmap):
        raise ValueError("simulation occupancy changed during the sample")
    return {"build": "BLES01857 01.24", "atomic": False,
            "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "simulation": f"0x{sim:08x}", "pool": f"0x{pool:08x}",
            "capacity": capacity, "pool_word_14": word_14, "occupied": len(records),
            "awaiting_baseline": sum(r["awaiting_baseline"] for r in records),
            "groups": dict(Counter(r["group"] for r in records)), "records": records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", required=True, type=Path)
    parser.add_argument("--sim", required=True, type=lambda x: int(x, 0),
                        help="simulation argument observed in this exact run; no address is guessed")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    destination = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("private memory observations must stay outside tracked source or under ignored local/")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = snapshot(memory.read, args.sim)
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {destination}: {result['occupied']} occupied, "
          f"{result['awaiting_baseline']} awaiting baseline")


if __name__ == "__main__":
    main()
