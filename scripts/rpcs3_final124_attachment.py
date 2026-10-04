"""Inspect a freshly observed 01.24 attachment container without guest memory writes.

Supply the owner datum from the creator's registration, the selected target unit
and the configured asset tag. Run-specific handles must come from that same run.
The bounded component chain, ownership and target links are checked independently
of function hooks that the PPU compiler may inline. Output stays private.
"""
import argparse
import datetime
import json
import struct
from pathlib import Path

COMPONENT_POOL = 0x1A21CB0
UNIT_POOL = 0x1A1D9E0
UNIT_MASK = 0x1A21138
NETWORK_COMPONENT = 0x80AAD032
ABSENT = 0xFFFFFFFF


def u32(read, address):
    return struct.unpack(">I", read(address, 4))[0]


def resolve_datum(read, handle):
    if not 0 <= handle < ABSENT:
        raise ValueError("absent or invalid datum")
    signed = handle if handle < 0x80000000 else handle - 0x100000000
    shifted = signed >> 13
    bank = shifted & (((shifted & ABSENT) >> 18) | 0x3FF)
    descriptor = u32(read, u32(read, 0x1896D50)) + bank * 0x40
    stride = u32(read, descriptor + 0x30)
    if not 0 < stride <= 0x10000:
        raise ValueError("unsupported datum stride")
    row = u32(read, descriptor + 0x10) + (handle & 0x1FFF) * stride
    return (row - (u32(read, row + 8) & u32(read, descriptor + 0x34))) & ABSENT


def unit_membership(read, handle):
    """Report the exact unit-pool discriminator and occupancy gate used by the HO."""
    index = handle & 0x1FFF
    capacity = u32(read, UNIT_POOL + 0x10)
    if not 0 < capacity <= 0x2000:
        raise ValueError("unsupported unit capacity")
    bank_matches = ((handle >> 13) & 0x3FF) == u32(read, UNIT_POOL + 0x30)
    in_range = index < capacity
    # The native gate uses low index bits alone. Do not read beyond the unit bitmap.
    bit = bool(u32(read, UNIT_MASK + (index // 32) * 4) & (1 << (index % 32))) if in_range else None
    return {"index": index, "unit_bank_matches": bank_matches,
            "unit_index_in_range": in_range, "unit_mask_bit": bit}


def snapshot(read, owner, target, asset):
    pool = read(COMPONENT_POOL, 0x40)
    base, stride, capacity = (struct.unpack_from(">I", pool, off)[0] for off in (8, 12, 16))
    bank = struct.unpack_from(">I", pool, 0x30)[0]
    if stride != 0x18 or not 0 < capacity <= 0x2000:
        raise ValueError("unsupported component pool")

    def row(handle):
        if not 0 <= handle < ABSENT or (handle >> 13) & 0x3FF != bank or handle & 0x1FFF >= capacity:
            raise ValueError("component datum is outside the selected pool")
        address = base + (handle & 0x1FFF) * stride
        raw = read(address, stride)
        words = struct.unpack(">6I", raw)
        size = words[0] & 0x3FFFFFFF
        if words[0] & 0xC0000000 != 0xC0000000 or not 0x14 <= size <= 0x10000:
            raise ValueError("component row is not an occupied bounded allocation")
        return address, raw, words, (address - words[2]) & ABSENT, size

    _, owner_row, owner_words, runtime, _ = row(owner)
    header = read(runtime, 0x14)
    actual_asset, count, header_type = struct.unpack_from(">3I", header, 4)
    if actual_asset != asset or header_type != 0x80800906 or not 0 < count <= 64:
        raise ValueError("registered container does not match the configured asset/layout")

    records, seen, rows = [], {owner & 0x1FFF}, [(owner, owner_row)]
    links = []
    handle = owner_words[4]
    for _ in range(count):
        if handle & 0x1FFF in seen:
            raise ValueError("attachment component chain contains a cycle")
        seen.add(handle & 0x1FFF)
        _, raw, words, address, size = row(handle)
        if size < 0x20 or words[3] != owner:
            raise ValueError("component does not belong to the registered container")
        component_header = read(address, 0x20)
        links.append((address, component_header))
        if struct.unpack_from(">I", component_header, 0x1C)[0] != target:
            raise ValueError("component target differs from the selected unit")
        record = {"datum": f"0x{handle:08x}", "runtime": f"0x{address:08x}", "size": size,
                  "configured_type": f"0x{words[5]:08x}",
                  "runtime_type": f"0x{struct.unpack_from('>I', component_header, 8)[0]:08x}",
                  "target": f"0x{target:08x}"}
        if words[5] == NETWORK_COMPONENT:
            if size < 0x24:
                raise ValueError("network component allocation does not contain its binding")
            binding = u32(read, address + 0x20)
            binding_address = resolve_datum(read, binding)
            binding_tail = read(binding_address + 0x104, 8)
            next_binding, associated = struct.unpack(">2I", binding_tail)
            links += [(address + 0x20, struct.pack(">I", binding)),
                      (binding_address + 0x104, binding_tail)]
            record["binding"] = {"datum": f"0x{binding:08x}", "runtime": f"0x{binding_address:08x}",
                                 "next": f"0x{next_binding:08x}",
                                 "associated_unit": f"0x{associated:08x}",
                                 "matches_selected_target": associated == target,
                                 "unit_membership": unit_membership(read, associated)}
        rows.append((handle, raw))
        records.append(record)
        handle = words[4]
    if handle != ABSENT:
        raise ValueError("component chain exceeds the configured count")
    if (read(COMPONENT_POOL, 0x40) != pool or read(runtime, 0x14) != header
            or any(row(handle)[1] != raw for handle, raw in rows)
            or any(read(address, len(raw)) != raw for address, raw in links)):
        raise ValueError("attachment structure changed during the sample")
    return {"build": "BLES01857 01.24", "atomic": False,
            "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "owner": f"0x{owner:08x}", "target": f"0x{target:08x}", "asset": f"0x{asset:08x}",
            "runtime": f"0x{runtime:08x}", "component_count": count,
            "owner_unit_membership": unit_membership(read, owner),
            "target_unit_membership": unit_membership(read, target), "components": records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", required=True, type=Path)
    for name in ("owner", "target", "asset"):
        parser.add_argument(f"--{name}", required=True, type=lambda s: int(s, 0))
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    destination = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("private observations must stay outside tracked source or under ignored local/")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = snapshot(memory.read, args.owner, args.target, args.asset)
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {destination}: {result['component_count']} components verified")


if __name__ == "__main__":
    main()
