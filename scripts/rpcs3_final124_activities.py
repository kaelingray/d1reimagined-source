"""Preserve the initialized 01.24 activity table from an owned emulator.

This is a read-only catalog, not a list of playable missions. Indices belong to
this exact build; do not infer them from the 01.00 service-105 convention.
Output contains private game data and must stay outside the tracked repository.
"""
import argparse
import datetime
import json
import struct
from pathlib import Path

ACTIVITY_CACHE = 0x01A2082C


def activities(read):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    root = u32(ACTIVITY_CACHE)
    if not root:
        raise ValueError("activity cache is not initialized yet")
    table = root + 4
    count, offset = struct.unpack(">II", read(table, 8))
    if not 0 < count <= 4096 or offset > 0x1000000:
        raise ValueError("unexpected 01.24 activity table header")
    rows = []
    for index in range(count):
        entry = table + offset + 0x14 + 8 * index
        relative = u32(entry + 4)
        definition = entry + relative + 4
        raw = read(definition, 0x50)
        rows.append({"index": index, "hash": f"0x{struct.unpack_from('>I', raw)[0]:08x}",
                     "scenario": f"0x{struct.unpack_from('>I', raw, 0x40)[0]:08x}",
                     "required_level": struct.unpack_from(">I", raw, 0x48)[0],
                     "address": f"0x{definition:08x}", "prefix": raw.hex()})
    if u32(ACTIVITY_CACHE) != root or read(table, 8) != struct.pack(">II", count, offset):
        raise ValueError("activity table changed during the read")
    return {"build": "BLES01857 01.24", "atomic": False,
            "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "count": count, "activities": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    destination = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("game-data output must be outside the tracked repository or under ignored local/")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = activities(memory.read)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"saved {result['count']} activity definitions to {destination}")


if __name__ == "__main__":
    main()
