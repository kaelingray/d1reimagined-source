"""Preserve the initialized 01.24 activity table from an owned emulator.

This is a read-only catalog, not a list of playable missions. Indices belong to
this exact build; do not infer them from the 01.00 service-105 convention.
The +0x44 tag is deliberately unlabeled pending analysis of its consumers.
Output contains private game data and must stay outside the tracked repository.
"""
import argparse
import datetime
import json
import struct
from pathlib import Path

ACTIVITY_CACHE = 0x01A2082C


def activity_table(read, root):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

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
                     "tag_44": f"0x{struct.unpack_from('>I', raw, 0x44)[0]:08x}",
                     "required_level": struct.unpack_from(">I", raw, 0x48)[0],
                     "address": f"0x{definition:08x}", "prefix": raw.hex()})
    if read(table, 8) != struct.pack(">II", count, offset):
        raise ValueError("activity table changed during the read")
    return {"build": "BLES01857 01.24", "atomic": False,
            "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "count": count, "activities": rows}


def activities(read):
    root = struct.unpack(">I", read(ACTIVITY_CACHE, 4))[0]
    if not root:
        raise ValueError("activity cache is not initialized yet")
    result = activity_table(read, root)
    if struct.unpack(">I", read(ACTIVITY_CACHE, 4))[0] != root:
        raise ValueError("activity cache changed during the read")
    return result


def activity_blob(data):
    """Decode the same relative definitions in an immutable package tag."""
    def read(address, size):
        if address < 0 or address + size > len(data):
            raise ValueError("activity definition is outside the package tag")
        return data[address:address + size]
    result = activity_table(read, 0)
    result.update(atomic=True, source="static package tag")
    for row in result["activities"]:
        row["offset"] = row.pop("address")
    return result


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
