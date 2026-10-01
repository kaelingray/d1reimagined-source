"""Sample Destiny PS3 01.24 bootflow state through the persistent RPCS3 GDB daemon.

The addresses are specific to the decrypted BLES01857 01.24 EBOOT. This reads
guest memory only; the samples are sequential, so they are not atomic. Save
them under local/ with the run logs to compare stalls without breakpoints.
"""

import argparse
import datetime
import json
import struct
from pathlib import Path

from rpcs3_thread_snapshot import memory


BOOTFLOW_PTR = 0x01ABC89C
BOOTFLOW_NAMES = 0x012F7F00
RESOURCE_NAMES = 0x01315588
RESOURCE_STATES = 0x01ADA1B0


def u32(address):
    return struct.unpack(">I", memory(address, 4))[0]


def cstring(address, limit=96):
    if not address:
        return None
    return memory(address, limit).split(b"\0", 1)[0].decode("ascii", "replace")


def sample():
    bootflow = u32(BOOTFLOW_PTR)
    if not bootflow:
        raise ValueError("01.24 bootflow singleton is not initialized")
    fields = memory(bootflow + 0x118, 0x1C)
    current, subcurrent = struct.unpack_from(">II", fields)
    target, subtarget = struct.unpack_from(">II", fields, 0x10)
    if current >= 38:
        raise ValueError(f"unexpected bootflow state {current}")
    stage_object = u32(bootflow + 0x80 + current * 4)
    stage_fields = memory(stage_object + 0x30, 0x10) if stage_object else b""
    resources = []
    states = memory(RESOURCE_STATES, 8 * 12)
    names = memory(RESOURCE_NAMES, 8 * 4)
    for index in range(8):
        state, handle, extra = struct.unpack_from(">III", states, index * 12)
        label_addr = struct.unpack_from(">I", names, index * 4)[0]
        resources.append({"slot": index, "name": cstring(label_addr), "state": state,
                          "handle": f"0x{handle:08x}", "extra": f"0x{extra:08x}"})
    return {
        "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "build": "BLES01857 01.24", "atomic": False,
        "bootflow_object": f"0x{bootflow:08x}",
        "current": current, "name": cstring(u32(BOOTFLOW_NAMES + current * 4)),
        "subcurrent": subcurrent, "target": target, "subtarget": subtarget,
        "stage_object": f"0x{stage_object:08x}",
        "stage_fields_30_3f": stage_fields.hex(),
        "stage_state_30": struct.unpack_from(">I", stage_fields)[0] if stage_fields else None,
        "stage_state_34": struct.unpack_from(">I", stage_fields, 4)[0] if stage_fields else None,
        "resources": resources,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    result = sample()
    output = json.dumps(result, indent=2) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(output, encoding="utf-8")
        print(f"wrote {args.out}: {result['name']}, substate {result['stage_state_30']}")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
