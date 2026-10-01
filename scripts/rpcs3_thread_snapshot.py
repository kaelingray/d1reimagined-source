"""Capture all guest PPU thread PCs and short stack chains via the persistent GDB daemon.

The daemon's control port is loopback-only. The per-thread reads are sequential,
so this is a sampling aid rather than an atomic crash dump. Write the result
under local/ alongside the run logs.
"""

import argparse
import ast
import datetime
import json
import socket
import struct
from pathlib import Path


def control(command):
    with socket.create_connection(("127.0.0.1", 2346), timeout=10) as conn:
        conn.settimeout(30)
        conn.sendall(command.encode() + b"\n")
        data = b""
        while not data.endswith(b"\n.\n"):
            chunk = conn.recv(65536)
            if not chunk:
                raise EOFError("GDB control connection closed")
            data += chunk
    return data[:-3].decode().strip()


def raw(packet):
    return ast.literal_eval(control("raw " + packet))


def memory(address, size):
    return bytes.fromhex(control(f"mem {address:x} {size}"))


def chain(stack_pointer, depth=6):
    result = []
    current = stack_pointer
    seen = set()
    for _ in range(depth):
        if current in seen or not (0x10000 <= current < 0xF0000000):
            break
        seen.add(current)
        try:
            next_frame = struct.unpack(">Q", memory(current, 8))[0] & 0xFFFFFFFF
            if not next_frame or next_frame <= current:
                break
            saved_lr = struct.unpack(">Q", memory(next_frame + 16, 8))[0] & 0xFFFFFFFF
        except (OSError, ValueError, EOFError, struct.error):
            break
        result.append({"stack": f"0x{next_frame:08x}", "saved_lr": f"0x{saved_lr:08x}"})
        current = next_frame
    return result


def snapshot():
    thread_list = raw("qfThreadInfo")
    if not thread_list.startswith("m"):
        raise ValueError(f"unexpected thread list: {thread_list!r}")
    ids = thread_list[1:].rstrip("l").split(",")
    output = {"captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "atomic": False, "threads": []}
    for thread_id in ids:
        if raw("Hg" + thread_id) != "OK":
            continue
        try:
            pc = int(raw("p40"), 16)
            lr = int(raw("p43"), 16)
            sp = int(raw("p1"), 16)
            output["threads"].append({"id": thread_id, "pc": f"0x{pc:08x}",
                                      "lr": f"0x{lr:08x}", "sp": f"0x{sp:08x}",
                                      "chain": chain(sp)})
        except (OSError, ValueError, EOFError) as exc:
            output["threads"].append({"id": thread_id, "error": repr(exc)})
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    data = snapshot()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out}: {len(data['threads'])} threads")


if __name__ == "__main__":
    main()
