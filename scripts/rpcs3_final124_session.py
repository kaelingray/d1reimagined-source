"""Observe 01.24 activity sessions without pausing or writing guest memory.

Layouts follow 0x33502c/0x33a808; simulation-context resolution follows
0x1d56d0 and 0x108960 in the BLES01857 01.24 ELF. Samples are not atomic.
Use an exact owned-process manifest and keep the resulting timelines private.
"""

import argparse
import datetime
import json
import struct
import time
from pathlib import Path

ENGINE_PTR = 0x1ABC8AC
MANAGER_PTR = 0x1ABC8A4
SLOT_MAP_PTR = 0x1ABC79C
CONTEXT_DESCRIPTOR = 0x1A20C70
DATUM_TABLE_PTR = 0x1896D50
SLICE_TABLE_PTR = 0x1A1DC80
UPDATE_GATE_FLAGS_PTR = 0x1B28860
UPDATE_GATE_COUNT_PTR = 0x1B28864
ACTIVITY_CLOCK_PTR = 0x1AB6710


def u32(read, address):
    return struct.unpack(">I", read(address, 4))[0]


def rotl32(value, shift):
    return ((value << shift) | (value >> (32 - shift))) & 0xFFFFFFFF


def context_address(handle, read):
    """Resolve the context as the native lookup does; retain lookup evidence.

    The native routine accepts index zero. Only 0xffffffff is its null handle.
    Rechecking descriptors detects some concurrent changes, not every race.
    """
    if handle == 0xFFFFFFFF:
        return None
    descriptor = read(CONTEXT_DESCRIPTOR, 0x34)
    storage = struct.unpack_from(">I", descriptor, 8)[0]
    offset, stride, mask = struct.unpack_from(">III", descriptor, 0x18)
    flags = struct.unpack_from(">I", descriptor, 0x30)[0]
    if not storage or not stride or stride > 0x100000:
        raise ValueError("uninitialized or unexpected context descriptor")
    index = handle & 0x1FFF
    row = (storage + offset + index * stride) & 0xFFFFFFFF
    generation = u32(read, row)
    data = generation & mask
    if flags & 0x40000000:
        packed = (mask | 0x80000000) & ~0x40000000
        packed = (packed & ~0x38000000) | (rotl32(data, 27) & 0x38000000)
        packed = (packed & ~0x07FFE000) | (rotl32(flags, 13) & 0x07FFE000)
    else:
        packed = mask & 0x7FFFFFFF
        packed = (packed & ~0x7F800000) | (rotl32(data, 23) & 0x7F800000)
        packed = (packed & ~0x007FE000) | (rotl32(flags, 13) & 0x007FE000)
    packed = (packed & 0xFFFFE000) | index
    signed = packed if packed < 0x80000000 else packed - 0x100000000
    shifted = signed >> 13
    group = shifted & (((shifted & 0xFFFFFFFF) >> 18) | 0x3FF)
    table_pointer = u32(read, DATUM_TABLE_PTR)
    table = u32(read, table_pointer)
    entry = (table + group * 0x40) & 0xFFFFFFFF
    raw = read(entry, 0x38)
    datum_stride = struct.unpack_from(">I", raw, 0x30)[0]
    datum_storage = struct.unpack_from(">I", raw, 0x10)[0]
    datum_mask = struct.unpack_from(">I", raw, 0x34)[0]
    if not datum_storage or not datum_stride or datum_stride > 0x100000:
        raise ValueError("uninitialized or unexpected datum descriptor")
    datum = (datum_storage + index * datum_stride) & 0xFFFFFFFF
    tag = u32(read, datum + 8)
    address = (datum - (tag & datum_mask)) & 0xFFFFFFFF
    if address < 0x10000 or address + 0x5088 > 0x100000000:
        raise ValueError("resolved context range is invalid")
    return {
        "address": address, "handle": handle, "packed": packed, "group": group,
        "generation_row": row, "generation": generation,
        "datum": datum, "datum_tag": tag,
        "unchanged_during_lookup": (
            read(CONTEXT_DESCRIPTOR, 0x34) == descriptor
            and u32(read, row) == generation
            and u32(read, DATUM_TABLE_PTR) == table_pointer
            and u32(read, table_pointer) == table
            and read(entry, 0x38) == raw and u32(read, datum + 8) == tag),
    }


def snapshot(read):
    engine, manager, mapping = (u32(read, p) for p in (ENGINE_PTR, MANAGER_PTR, SLOT_MAP_PTR))
    if not all((engine, manager, mapping)):
        raise ValueError("activity engine/session manager is not initialized")
    raw = read(engine + 0x88, 0x4E0)
    selected_raw = read(mapping, 16)
    slots = list(struct.unpack(">4I", selected_raw))
    pair = lambda offset: list(struct.unpack_from(">iI", raw, offset - 0x88))
    result = {
        "atomic": False, "engine": engine, "manager": manager, "mapping": mapping,
        "flags_88_8b": list(raw[:4]),
        "update_gate_bytes_80_81": list(read(engine + 0x80, 2)),
        "last_sent_flags": list(raw[0x1F0 - 0x88:0x1F4 - 0x88]),
        "native_state_differs_from_last_sent": raw[:0x168] != raw[0x168:0x2D0],
        "native_current": pair(0x90), "native_target": pair(0x130),
        "last_sent_current": pair(0x1F8), "last_sent_target": pair(0x298),
        "engine_version": struct.unpack_from(">I", raw, 0x3B0 - 0x88)[0],
        "local_peer": struct.unpack_from(">I", raw, 0x3B4 - 0x88)[0],
        "reported_peer_byte": raw[0x1D0 - 0x88],
        "target_state_13c": raw[0x13C - 0x88],
        "target_phase_141": raw[0x141 - 0x88],
        "peer_count_1d2": raw[0x1D2 - 0x88],
        "group0": raw[0x3B8 - 0x88:0x490 - 0x88].hex(),
        "group1": raw[0x490 - 0x88:0x568 - 0x88].hex(),
        "mapped_slots": slots, "sessions": [],
    }
    # 0x71aba4 is the first early-out in 0x353c7c. Preserve the operands;
    # the meaning of the flags is not inferred from their addresses.
    gate_flags = u32(read, UPDATE_GATE_FLAGS_PTR)
    gate_count = u32(read, UPDATE_GATE_COUNT_PTR)
    operands = list(read(gate_flags, 2)) if gate_flags else [0, 0]
    count = u32(read, gate_count) if gate_count else 0
    result["global_update_gate"] = {"flags_pointer": gate_flags, "count_pointer": gate_count,
                                    "flags": operands, "count": count,
                                    "predicate": bool(any(operands) and count)}
    # 0x12fc8c -> 0x2b9de8 returns this counter. It need not advance at
    # wall-clock speed; do not infer a 120-second timeout from UTC alone.
    clock = u32(read, ACTIVITY_CLOCK_PTR)
    result["activity_clock"] = {"pointer": clock, "tick_0c": u32(read, clock + 0xC) if clock else None}
    for slot in sorted(set(slots) - {0xFFFFFFFF}):
        if slot >= 16:
            raise ValueError(f"unexpected mapped activity slot {slot}")
        record = manager + 0x4E0 + slot * 0x170
        handle_address = mapping + slot * 8 + 0x14
        state, handle = u32(read, record), u32(read, handle_address)
        session = {"slot": slot, "state": state, "context_handle": handle,
                   "transport_handle": u32(read, record + 0x70)}
        if state == 6 and handle != 0xFFFFFFFF:
            try:
                lookup = context_address(handle, read)
                address = lookup["address"]
                header = read(address, 0x34)
                incoming = read(address + 0x5078, 16)
                session["context"] = {
                    "lookup": lookup, "header_00_33": header.hex(),
                    "active_4": header[4], "deferred_5": header[5],
                    "pending_18": header[0x18], "pending_since_1c": struct.unpack_from(">I", header, 0x1C)[0],
                    "pending_20": header[0x20], "pending_since_30": struct.unpack_from(">I", header, 0x30)[0],
                    "incoming_5078_5087": incoming.hex(),
                    "unchanged_handle_state": (u32(read, handle_address) == handle and u32(read, record) == state),
                }
            except (ValueError, OSError, struct.error) as error:
                session["context_error"] = str(error)
        result["sessions"].append(session)
    table = u32(read, SLICE_TABLE_PTR)
    slice_raw = read(table + 0x110C, 72)
    result["active_loading"] = list(struct.unpack_from(">2i", slice_raw))
    result["loaded_words"] = list(struct.unpack_from(">16I", slice_raw, 8))
    result["pointers_unchanged"] = (
        all(u32(read, p) == value for p, value in ((ENGINE_PTR, engine), (MANAGER_PTR, manager), (SLOT_MAP_PTR, mapping), (SLICE_TABLE_PTR, table)))
        and read(mapping, 16) == selected_raw)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=360)
    parser.add_argument("--interval", type=float, default=0.5)
    args = parser.parse_args()
    if not 0 < args.seconds <= 3600 or not 0.1 <= args.interval <= 10:
        parser.error("seconds must be in (0,3600]; interval in [0.1,10]")
    from rpcs3_owned_memory import OwnedMemory
    with args.output.open("x", encoding="utf-8", buffering=1) as stream, OwnedMemory(args.manifest) as memory:
        deadline = time.monotonic() + args.seconds
        previous = None
        while time.monotonic() < deadline:
            state = snapshot(memory.read)
            state["utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
            stream.write(json.dumps(state) + "\n")
            summary = {key: state[key] for key in ("flags_88_8b", "native_current", "native_target", "active_loading")}
            summary["sessions"] = [{"slot": s["slot"], "state": s["state"],
                                    "pending": [s.get("context", {}).get("pending_18"), s.get("context", {}).get("pending_20")],
                                    "incoming": s.get("context", {}).get("incoming_5078_5087"),
                                    "error": s.get("context_error")} for s in state["sessions"]]
            if summary != previous:
                print(json.dumps({"utc": state["utc"], **summary}), flush=True)
                previous = summary
            time.sleep(args.interval)


if __name__ == "__main__":
    main()
