"""Read the 01.24 slice handoff without pausing or writing guest state.

Engine/group fields follow 0x351df0 and the zone phases documented in the
profile checkpoint. A loaded slice is not necessarily active. Samples remain
private, non-atomic observations even when the captured fields stay unchanged.
"""
import argparse
import datetime
import json
from pathlib import Path
import struct


ENGINE_PTR = 0x01ABC8AC
SLICE_TABLE_PTR = 0x01A1DC80


def snapshot(read):
    def checked(address, size):
        if address <= 0 or address + size > 0x100000000:
            raise ValueError("invalid transition memory range")
        raw = read(address, size)
        if len(raw) != size:
            raise ValueError("short transition memory read")
        return raw

    def u32(address):
        return struct.unpack(">I", checked(address, 4))[0]

    engine = u32(ENGINE_PTR)
    result = {"build": "BLES01857 01.24", "atomic": False,
              "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "engine": f"0x{engine:08x}", "initialized": bool(engine)}
    if not engine:
        return result
    fields = checked(engine + 0x88, 0x14C)
    groups = checked(engine + 0x3B0, 0x1B8)
    version, local_slot = struct.unpack_from(">Ii", groups)

    def slice_record(offset):
        record = fields[offset - 0x88:offset - 0x88 + 0x40]
        word_0, name_hash, index = struct.unpack_from(">iIi", record)
        return {"word_0": word_0, "name_hash": f"0x{name_hash:08x}",
                "slice_set": index, "raw": record.hex()}

    def group_record(offset):
        record = groups[offset - 0x3B0:offset - 0x3B0 + 0xD8]
        return {"slice_set": struct.unpack_from(">i", record)[0],
                "sequence": struct.unpack_from(">b", record, 8)[0],
                "state": struct.unpack_from(">b", record, 9)[0],
                "peer": struct.unpack_from(">b", record, 10)[0],
                "local_peer_byte": record[0x38 + local_slot] if 0 <= local_slot < 20 else None}

    result.update(flags_88_8b=list(fields[:4]), current=slice_record(0x90),
                  target=slice_record(0x130), peer_byte_1d0=fields[0x1D0 - 0x88],
                  cached_world_version=version, local_peer_slot=local_slot,
                  group_current=group_record(0x3B8), group_switch=group_record(0x490))
    table = u32(SLICE_TABLE_PTR)
    result["slice_table"] = f"0x{table:08x}"
    table_fields = checked(table + 0x110C, 0x48) if table else None
    if table_fields:
        active, loading = struct.unpack_from(">ii", table_fields)
        loaded = struct.unpack_from(">16I", table_fields, 8)
        target = result["target"]["slice_set"]
        result["slices"] = {"active": active, "loading": loading,
                            "target_loaded": bool(loaded[target // 32] & (1 << (target % 32)))
                            if 0 <= target < 512 else None}
    else:
        result["slices"] = None
    result["fields_unchanged_during_sample"] = (
        u32(ENGINE_PTR) == engine and u32(SLICE_TABLE_PTR) == table
        and checked(engine + 0x88, len(fields)) == fields
        and checked(engine + 0x3B0, len(groups)) == groups
        and (not table_fields or checked(table + 0x110C, len(table_fields)) == table_fields))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    destination = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("private memory observations must stay outside tracked source or under ignored local/")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = snapshot(memory.read)
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
