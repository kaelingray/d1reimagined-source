"""Inspect 01.24 world-loot records and their nonce/position cache, read-only.

Derive the self-character address from the first-slot r4 at native 0x2b36f8
in this exact run (r4 minus 0x3bd8); do not reuse another run's address.
The manager singleton is 0x1ab6498+4 (0x2b2f74). Layouts follow 0x2b4070,
0x2b36f8 and 0x2b3240. A matching position is only one spawn prerequisite;
unit references here do not establish allocation, visibility or pickup.
All observations are private and non-atomic; changed inputs are rejected.
"""

import argparse
import datetime
import json
import math
import struct
from pathlib import Path

MANAGER_POINTER = 0x1AB649C
WORLD_CONTROLLER_POINTER = 0x1A1DC80
SLOTS = 64
LOOT_OFFSET = 0x3BD8
LOOT_STRIDE = 0x38
CACHE_OFFSET = 0x210
SLOT_CACHE_OFFSET = 0xE10
CACHE_STRIDE = 0x30
MANAGER_SIZE = 0x1A24


def nonce(raw, offset=0):
    return struct.unpack_from(">i", raw, offset)[0], struct.unpack_from(">Q", raw, offset + 8)[0]


def client_data(raw):
    key, identifier = nonce(raw)
    position = struct.unpack_from(">4f", raw, 0x10)
    if not all(math.isfinite(v) for v in position):
        raise ValueError("loot client data contains a non-finite position")
    return {"nonce_key": key, "nonce_id": f"0x{identifier:016x}",
            "position": list(position), "world_id": struct.unpack_from(">i", raw, 0x20)[0]}


def snapshot(read, self_character, expected_character_id):
    if not 0 < self_character < 0x100000000 - 0x6DB8 or self_character % 8:
        raise ValueError("self character address is invalid")
    pointer = read(MANAGER_POINTER, 4)
    manager, = struct.unpack(">I", pointer)
    if not 0 < manager <= 0x100000000 - MANAGER_SIZE or manager % 8:
        raise ValueError("loot manager is absent or invalid")
    identity = read(self_character, 8)
    if struct.unpack(">Q", identity)[0] != expected_character_id:
        raise ValueError("self character identity does not match the requested character")
    raw = read(manager, MANAGER_SIZE)
    capacity, start, count = struct.unpack_from(">III", raw, 0x200)
    if capacity != SLOTS or start >= SLOTS or count > SLOTS:
        raise ValueError("loot nonce cache has an unsupported layout or bounds")
    # 0x14dcf4 -> 0x158dbc extracts bits 3..8 of the controller's
    # current-world handle. Manager +0x1a10 is a signature, not that ID.
    world_pointer = read(WORLD_CONTROLLER_POINTER, 4)
    controller, = struct.unpack(">I", world_pointer)
    if not 0 < controller <= 0x100000000 - 0x1110 or controller % 4:
        raise ValueError("world controller is absent or invalid")
    world_handle = read(controller + 0x110C, 4)
    packed_world, = struct.unpack(">I", world_handle)
    world_id = -1 if packed_world == 0xFFFFFFFF else (packed_world >> 3) & 63
    loot = read(self_character + LOOT_OFFSET, SLOTS * LOOT_STRIDE)
    cache = []
    for n in range(count):
        index = (start + n) % SLOTS
        off = CACHE_OFFSET + index * CACHE_STRIDE
        cache.append((index, raw[off:off + CACHE_STRIDE]))
    records = []
    for index in range(SLOTS):
        record = loot[index * LOOT_STRIDE:(index + 1) * LOOT_STRIDE]
        item_index, = struct.unpack_from(">h", record)
        if item_index == -1:
            continue
        if item_index < 0:
            raise ValueError("loot record has an invalid item index")
        key, identifier = nonce(record, 0x10)
        source_kind = record[8]
        matches = [{"cache_index": i, **client_data(data)}
                   for i, data in reversed(cache) if nonce(data) == (key, identifier)]
        off = SLOT_CACHE_OFFSET + index * CACHE_STRIDE
        slot_cache = raw[off:off + CACHE_STRIDE]
        fallback = client_data(slot_cache) if nonce(slot_cache) == (key, identifier) else None
        # Native 0x2b3240 selects the newest ring match before the slot fallback.
        selected = matches[0] if matches else fallback
        unit_a, unit_b = struct.unpack_from(">II", raw, index * 8)
        records.append({"slot": index, "item_index": item_index, "source_kind": source_kind,
                        "word_04": struct.unpack_from(">i", record, 4)[0],
                        "nonce_key": key, "nonce_id": f"0x{identifier:016x}",
                        "nonce_word_10": struct.unpack_from(">i", record, 0x20)[0],
                        "unit_reference": [f"0x{unit_a:08x}", f"0x{unit_b:08x}"],
                        "ring_matches": matches, "slot_match": fallback,
                        "world_spawn_source": source_kind in (3, 5),
                        "position_matches_current_world": bool(selected and selected["world_id"] == world_id)})
    # Re-read all inputs we interpreted. The dirty byte can change independently;
    # these equality checks catch some races, and do not make the sample atomic.
    if (read(MANAGER_POINTER, 4) != pointer or read(self_character, 8) != identity
            or read(WORLD_CONTROLLER_POINTER, 4) != world_pointer
            or read(controller + 0x110C, 4) != world_handle
            or read(manager, 0x1A20) != raw[:0x1A20]
            or read(self_character + LOOT_OFFSET, SLOTS * LOOT_STRIDE) != loot):
        raise ValueError("loot inputs changed during the sample")
    return {"build": "BLES01857 01.24", "atomic": False,
            "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "manager": f"0x{manager:08x}", "self_character": f"0x{self_character:08x}",
            "character_id": f"0x{expected_character_id:016x}", "current_world_id": world_id,
            "world_controller": f"0x{controller:08x}", "world_handle": f"0x{packed_world:08x}",
            "cache_start": start, "cache_count": count,
            "cache": [{"cache_index": i, **client_data(data)} for i, data in cache],
            "records": records,
            "scope": "Read-only prerequisites; unit occupancy, visible loot and successful grants are not verified."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", required=True, type=Path)
    parser.add_argument("--self-character", required=True, type=lambda v: int(v, 0))
    parser.add_argument("--character-id", required=True, type=lambda v: int(v, 0))
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    destination = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("private memory observations must stay outside tracked source or under ignored local/")
    if destination.exists():
        parser.error("preserve previous observations; choose a new output path")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = snapshot(memory.read, args.self_character, args.character_id)
    with destination.open("x", encoding="utf-8") as output:
        output.write(json.dumps(result, indent=2) + "\n")
    print(f"wrote {destination}: {len(result['records'])} loot records, {result['cache_count']} cached nonces")


if __name__ == "__main__":
    main()
