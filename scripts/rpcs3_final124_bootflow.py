"""Read the BLES01857 01.24 bootflow and resource states.

This promotes the reader from the imported 2026-09-29 session. Each memory
GDB requests pause and resume the emulator; --process-manifest instead uses
read-only Windows memory reads without pausing. Neither sample is atomic.
No guest registers or memory are written. Save samples under private local/.
"""

import argparse
import datetime
import json
import socket
import struct
from pathlib import Path

CONTROL = ("127.0.0.1", 2346)
BOOTFLOW_PTR = 0x01ABC89C
BOOTFLOW_NAMES = 0x012F7F00
RESOURCE_NAMES = 0x01315588
RESOURCE_STATES = 0x01ADA1B0
REQUEST_TABLE_PTR = 0x01896D50
RESOURCE_MANAGER_PTR = 0x01A1DCE0
RESOURCE_SCHEDULER = 0x01A1DCE8
BATCH_REQUEST_TABLE = 0x019A3F80
AUXILIARY_PTR = 0x01ABC8B8
READINESS_TABLE = 0x01B558F0
# Package registry (static 01.24 analysis, live-checked 2026-10-01 FINAL124_NEXT2): a record pool
# (header: records offset, links offset, stride), a u16 package-id -> record-index map, and the
# list head/count. Record fields: +0x04 s16 id, +0x10 u64 package build time (the named-tag search
# 0xecc6c prefers the newest), +0xb4 entry count, +0xec named-tag count, +0x140 path,
# +0x178 tag-table offset in the header block, +0x17c resident header-block handle (0 = none).
PACKAGE_POOL_PTR = 0x01896510
PACKAGE_INDEX_BY_ID = 0x019C2C60
PACKAGE_LIST_HEAD = 0x019BAC30
PACKAGE_LIST_COUNT = 0x019BAC34
BOOTSTRAP_PATCHABLE_PACKAGE = 0x0172


def memory(address, size):
    with socket.create_connection(CONTROL, timeout=10) as conn:
        conn.settimeout(30)
        conn.sendall(f"mem {address:x} {size}\n".encode("ascii"))
        data = bytearray()
        while not data.endswith(b"\n.\n"):
            chunk = conn.recv(65536)
            if not chunk:
                raise EOFError("GDB control connection closed before the reply terminator")
            data.extend(chunk)
            if len(data) > size * 2 + 4096:
                raise ValueError("GDB control reply exceeds the requested memory size")
    reply = data[:-3].decode("ascii").strip()
    if reply.startswith("error:"):
        raise ValueError(reply)
    raw = bytes.fromhex(reply)
    if len(raw) != size:
        raise ValueError(f"short guest read at {address:#x}: {len(raw)} of {size} bytes")
    return raw


def request_chain(handle, read=memory, max_depth=8):
    """Resolve a request as 0x14e864 does, including a pending parent chain.

    The group table and record layout come from 01.24's handle lookup. Values
    may change between reads; preserve them as observations, not conclusions.
    """
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    result, seen = [], set()
    while handle != 0xFFFFFFFF and len(result) < max_depth:
        if handle in seen:
            raise ValueError(f"cyclic request parent handle {handle:#x}")
        seen.add(handle)
        signed = handle if handle < 0x80000000 else handle - 0x100000000
        shifted = signed >> 13
        group = shifted & (((shifted & 0xFFFFFFFF) >> 18) | 0x3FF)
        table = u32(u32(REQUEST_TABLE_PTR))
        descriptor = read(table + group * 0x40, 0x38)
        stride = struct.unpack_from(">I", descriptor, 0x30)[0]
        storage = struct.unpack_from(">I", descriptor, 0x10)[0]
        mask = struct.unpack_from(">I", descriptor, 0x34)[0]
        item = (storage + (handle & 0x1FFF) * stride) & 0xFFFFFFFF
        address = (item - (u32(item + 8) & mask)) & 0xFFFFFFFF
        raw = read(address, 0x44)
        kind, status = struct.unpack_from(">bb", raw, 6)
        parent = struct.unpack_from(">I", raw, 0xC)[0]
        result.append({"handle": f"0x{handle:08x}", "address": f"0x{address:08x}",
                       "kind_byte_6": kind, "status_byte_7": status,
                       "parent_handle": f"0x{parent:08x}", "header": raw.hex(),
                       "flag_byte_8": raw[8], "error_byte_9": raw[9],
                       "counters_40_42": list(struct.unpack_from(">HH", raw, 0x40))})
        if kind != 2 or status != 1:
            break
        handle = parent
    return result


def scheduler_snapshot(read=memory):
    """Observe admission state for the paired kind-3 resource request.

    01.24's 0x150928 reads the token at RESOURCE_SCHEDULER and the active
    handle at manager+0x14070. 0x14ffb4 reads the queue/retry fields. 0xdfa44
    resolves a token using table-relative storage and stride, and reads its
    first word; keep that word numeric until the underlying phase is decoded.
    These separate reads pause/resume execution and are not an atomic view.
    """
    def checked_read(address, size):
        if address <= 0 or address + size > 0x100000000:
            raise ValueError(f"invalid scheduler memory range {address:#x}+{size:#x}")
        raw = read(address, size)
        if len(raw) != size:
            raise ValueError(f"short scheduler read at {address:#x}: {len(raw)} of {size} bytes")
        return raw

    manager = struct.unpack(">I", checked_read(RESOURCE_MANAGER_PTR, 4))[0]
    globals_raw = checked_read(RESOURCE_SCHEDULER, 12)
    token = struct.unpack_from(">H", globals_raw)[0]
    job_state, deferred_event = struct.unpack_from(">II", globals_raw, 4)
    result = {
        "manager_address": f"0x{manager:08x}",
        "globals_address": f"0x{RESOURCE_SCHEDULER:08x}",
        "globals_raw": globals_raw.hex(),
        "active_token": f"0x{token:04x}",
        "job_state_4": job_state, "deferred_event_8": deferred_event,
        "manager_fields": None, "active_batch_record": None,
    }
    if manager:
        raw = checked_read(manager + 0x1405C, 0x1C)
        queue_head, _, retry_count, retry0, retry1, active, flagged = struct.unpack(">7I", raw)
        result["manager_fields"] = {
            "raw_1405c_14077": raw.hex(), "queue_head": f"0x{queue_head:08x}",
            "retry_count": retry_count,
            "retry_handle_slots": [f"0x{retry0:08x}", f"0x{retry1:08x}"],
            "active_handle": f"0x{active:08x}", "flagged_count_14074": flagged,
        }
    if token != 0xFFFF:
        # 0x10a1d4 initializes this linked array: relative storage at +0,
        # stride at +8 and capacity (u16) at +0xc. 0xdeac8 initializes it
        # with stride 0x40/capacity 2. Check observed bounds before indexing.
        table = checked_read(BATCH_REQUEST_TABLE, 0x20)
        offset = struct.unpack_from(">I", table)[0]
        stride = struct.unpack_from(">I", table, 8)[0]
        capacity = struct.unpack_from(">H", table, 0xC)[0]
        if not offset or stride < 0x40 or stride % 4 or token >= capacity:
            raise ValueError(f"invalid scheduler token table: token={token:#x}, "
                             f"offset={offset:#x}, stride={stride:#x}, capacity={capacity}")
        address = BATCH_REQUEST_TABLE + offset + token * stride
        raw = checked_read(address, 0x40)
        result["active_batch_record"] = {
            "table_address": f"0x{BATCH_REQUEST_TABLE:08x}", "table_header": table.hex(),
            "capacity": capacity, "stride": stride, "address": f"0x{address:08x}",
            "status_word_0": struct.unpack_from(">i", raw)[0], "raw": raw.hex(),
        }
    return result


def readiness_snapshot(read=memory):
    """Read the mode and readiness masks tested by 01.24's 0x3592d8.

    0x3402b0 returns *(0x01abc8b8); +0x80 is a signed mode, not a request
    handle. 0x856e54 selects READINESS_TABLE only when its +0xe8 byte is set.
    0x85619c tests mode's u16 mask against u16(table+0xba) within 0x7ff.
    Raw fields are retained to distinguish this wait from resource slot 3.
    """
    def checked_read(address, size):
        if address <= 0 or address + size > 0x100000000:
            raise ValueError(f"invalid readiness memory range {address:#x}+{size:#x}")
        raw = read(address, size)
        if len(raw) != size:
            raise ValueError(f"short readiness read at {address:#x}: {len(raw)} of {size} bytes")
        return raw

    auxiliary = struct.unpack(">I", checked_read(AUXILIARY_PTR, 4))[0]
    auxiliary_raw = checked_read(auxiliary + 0x80, 8) if auxiliary else None
    mode = struct.unpack_from(">i", auxiliary_raw)[0] if auxiliary_raw else None
    raw = checked_read(READINESS_TABLE, 0xEC)
    masks = list(struct.unpack_from(">10H", raw))
    readiness_mask = struct.unpack_from(">H", raw, 0xBA)[0]
    initialized = raw[0xE8] != 0
    required = masks[mode] if initialized and mode is not None and 0 <= mode < len(masks) else None
    missing = required & (0x7FF & ~readiness_mask) if required is not None else None
    # 0x8564e4 polls count(+0x8c) entries of stride 0x18 starting at +0x14.
    # Keep summaries within the captured table; do not dereference load pointers.
    count = struct.unpack_from(">I", raw, 0x8C)[0]
    descriptor_limit = (0x8C - 0x14) // 0x18
    descriptors = []
    for index in range(min(count, descriptor_limit)):
        offset = 0x14 + index * 0x18
        bit_index, handle, loaded, allocation, word_10 = struct.unpack_from(">5I", raw, offset)
        descriptors.append({
            "index": index, "offset": f"0x{offset:02x}", "bit_index": bit_index,
            "content_handle": f"0x{handle:08x}", "loaded_pointer": f"0x{loaded:08x}",
            "allocation_pointer": f"0x{allocation:08x}", "word_10": f"0x{word_10:08x}",
            "byte_14": raw[offset + 0x14],
        })
    return {
        "auxiliary_address": f"0x{auxiliary:08x}",
        "auxiliary_raw_80_87": auxiliary_raw.hex() if auxiliary_raw else None,
        "mode_80": mode,
        "auxiliary_word_84": struct.unpack_from(">I", auxiliary_raw, 4)[0] if auxiliary_raw else None,
        "table_address": f"0x{READINESS_TABLE:08x}", "table_raw": raw.hex(),
        "initialized_flag_e8": raw[0xE8], "mode_masks": masks,
        "readiness_mask_ba": readiness_mask, "required_mode_mask": required,
        "missing_mode_mask": missing,
        "poll_ready": True if mode == -1 else (missing == 0 if missing is not None else None),
        "descriptor_count_8c": count,
        "descriptor_start_ac": struct.unpack_from(">I", raw, 0xAC)[0],
        "requested_count_a4": struct.unpack_from(">I", raw, 0xA4)[0],
        "control_word_a8": struct.unpack_from(">I", raw, 0xA8)[0],
        "loading_word_b0": struct.unpack_from(">i", raw, 0xB0)[0],
        "descriptors": descriptors, "descriptors_truncated": count > descriptor_limit,
    }


def package_records(package_ids, read=memory):
    """Registered record for each package id, via the id -> index map (one record per id)."""
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    pool = u32(PACKAGE_POOL_PTR)
    records_offset, _, stride = struct.unpack(">III", read(pool, 12))
    result = {"pool": f"0x{pool:08x}", "stride": stride,
              "head": struct.unpack(">H", read(PACKAGE_LIST_HEAD, 2))[0],
              "count": u32(PACKAGE_LIST_COUNT), "packages": []}
    for package_id in package_ids:
        index = struct.unpack(">H", read(PACKAGE_INDEX_BY_ID + package_id * 2, 2))[0]
        entry = {"id": f"0x{package_id:04x}", "index": index if index != 0xFFFF else None}
        if index != 0xFFFF:
            address = (pool + records_offset + index * stride) & 0xFFFFFFFF
            raw = read(address, 0x180)
            entry.update({
                "record": f"0x{address:08x}",
                "record_id": f"0x{struct.unpack_from('>H', raw, 4)[0]:04x}",
                "build_time_10": f"0x{struct.unpack_from('>Q', raw, 0x10)[0]:016x}",
                "entry_count_b4": struct.unpack_from(">I", raw, 0xB4)[0],
                "named_count_ec": struct.unpack_from(">I", raw, 0xEC)[0],
                "path_140": raw[0x140:0x178].split(b"\0", 1)[0].decode("ascii", "replace"),
                "named_offset_178": f"0x{struct.unpack_from('>I', raw, 0x178)[0]:x}",
                "header_block_17c": f"0x{struct.unpack_from('>I', raw, 0x17C)[0]:08x}",
            })
        result["packages"].append(entry)
    return result


def sample(read=memory, include_request=False, package_ids=()):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    def cstring(address):
        return read(address, 96).split(b"\0", 1)[0].decode("ascii", "replace") if address else None

    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    bootflow = u32(BOOTFLOW_PTR)
    if not bootflow:
        raise ValueError("01.24 bootflow singleton is not initialized")
    fields = read(bootflow + 0x118, 0x1C)
    current, subcurrent = struct.unpack_from(">II", fields)
    target, subtarget = struct.unpack_from(">II", fields, 0x10)
    if current >= 38:
        raise ValueError(f"unexpected 01.24 bootflow state {current}")
    stage_object = u32(bootflow + 0x80 + current * 4)
    stage_fields = read(stage_object + 0x30, 0x10) if stage_object else b""
    resources = []
    states = read(RESOURCE_STATES, 8 * 12)
    names = read(RESOURCE_NAMES, 8 * 4)
    for index in range(8):
        state, handle, extra = struct.unpack_from(">III", states, index * 12)
        label_addr = struct.unpack_from(">I", names, index * 4)[0]
        resources.append({"slot": index, "name": cstring(label_addr), "state": state,
                          "handle": f"0x{handle:08x}", "extra": f"0x{extra:08x}"})
    result = {
        "captured_utc": started,
        "build": "BLES01857 01.24", "atomic": False,
        "bootflow_object": f"0x{bootflow:08x}",
        "current": current, "name": cstring(u32(BOOTFLOW_NAMES + current * 4)),
        "subcurrent": subcurrent, "target": target, "subtarget": subtarget,
        "stage_object": f"0x{stage_object:08x}",
        "stage_fields_30_3f": stage_fields.hex(),
        "stage_word_30": struct.unpack_from(">I", stage_fields)[0] if stage_fields else None,
        "stage_word_34": struct.unpack_from(">I", stage_fields, 4)[0] if stage_fields else None,
        # These state offsets are verified for bap_signin only. Profile setup
        # has a different layout: +0x30 contains a pointer, not a substate.
        "stage_state_30": struct.unpack_from(">I", stage_fields)[0] if stage_fields and current == 19 else None,
        "stage_state_34": struct.unpack_from(">I", stage_fields, 4)[0] if stage_fields and current == 19 else None,
        "resources": resources,
        "resource_pending_count": u32(RESOURCE_STATES + 0x60),
        "asset_readiness": readiness_snapshot(read),
    }
    if include_request:
        result["bootstrap_request_chain"] = request_chain(int(resources[3]["handle"], 16), read)
        result["resource_scheduler"] = scheduler_snapshot(read)
    if package_ids:
        result["package_registry"] = package_records(package_ids, read)
    result["stage_unchanged_during_sample"] = (u32(BOOTFLOW_PTR) == bootflow and
                                                read(bootflow + 0x118, 0x1C) == fields)
    result["finished_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    return result


def world_snapshot(read):
    """Observe the engine leave state and double-buffered world controllers.

    Layout follows 01.24 getters 0x340230, 0x350648 and 0x349634. The
    controller status is a different state machine from the engine byte.
    """
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]
    engine = u32(0x01ABC8AC)
    base = u32(0x01ABCA14)
    result = {"engine": f"0x{engine:08x}", "engine_state_89": None,
              "controllers": []}
    if engine:
        result["engine_state_89"] = struct.unpack(">b", read(engine + 0x89, 1))[0]
    if base and read(base, 1) != b"\0":
        for index in (1, 2):
            record = base + index * 0x12208
            selector = u32(record + 8)
            if selector not in (0, 1):
                raise ValueError("unexpected world-controller buffer selector")
            controller = record + selector * 0x9100 + 0x10
            result["controllers"].append({"index": index, "selector": selector,
                "address": f"0x{controller:08x}",
                "status_8fec": u32(controller + 0x8FEC)})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--process-manifest", type=Path,
                        help="read the identified Windows emulator directly, without pausing it")
    parser.add_argument("--request", action="store_true", help="also resolve the bootstrap request and pending parents")
    parser.add_argument("--world", action="store_true", help="also read engine and world-controller states")
    parser.add_argument("--package", action="append", type=lambda v: int(v, 16), default=[],
                        metavar="HEXID", help="also dump the registered package record for this id "
                        "(repeatable; --request adds 0172, client_bootstrap_patchable)")
    args = parser.parse_args()
    package_ids = list(dict.fromkeys(args.package + ([BOOTSTRAP_PATCHABLE_PACKAGE] if args.request else [])))
    if args.process_manifest:
        from rpcs3_owned_memory import OwnedMemory
        with OwnedMemory(args.process_manifest) as process:
            result = sample(read=process.read, include_request=args.request, package_ids=package_ids)
            if args.world:
                result["world"] = world_snapshot(process.read)
        result["reader"] = "owned Windows process (no pause)"
    else:
        result = sample(include_request=args.request, package_ids=package_ids)
        if args.world:
            result["world"] = world_snapshot(memory)
    output = json.dumps(result, indent=2) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(output, encoding="utf-8")
        detail = f", substate {result['stage_state_30']}" if result['stage_state_30'] is not None else ""
        print(f"wrote {args.out}: {result['name']}{detail}, "
              f"bootstrap resource {result['resources'][3]['state']}")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
