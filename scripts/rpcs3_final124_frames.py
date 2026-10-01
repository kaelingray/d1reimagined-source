"""Read the 01.24 frame-credit wait and native GCM FIFO without pausing RPCS3.

The descriptor/getter signature is verified before using this runtime's native
cellGcmGetControlRegister. An optional explicit IO mapping lets the reader
identify a jump to self at GET. This observes a wait; it never skips commands.
"""
import argparse
import datetime
import json
import struct
from pathlib import Path

FRAME_INDEX = 0x01AED138             # 0x61e8dc
FRAME_LABEL_PTR = 0x01ADC1A4         # 0x602328, 0x60239c
GCM_CONTROL_DESCRIPTOR = 0x01CF049C
GCM_CONTROL_GETTER = bytes.fromhex("81228028806900104e800020")


def snapshot(read, mapping=None):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]

    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    function, toc = struct.unpack(">II", read(GCM_CONTROL_DESCRIPTOR, 8))
    if read(function, 12) != GCM_CONTROL_GETTER:
        raise ValueError("native GCM control getter signature differs from the verified runtime")
    context = u32(toc - 0x7fd8)
    control = u32(context + 0x10)
    put, get, ref = struct.unpack(">III", read(control, 12))
    label_pointer = u32(FRAME_LABEL_PTR)
    label = u32(label_pointer)
    frame = u32(FRAME_INDEX)
    credit = (label >> 8) & 255
    result = {"build": "BLES01857 01.24", "atomic": False, "captured_utc": started,
              "frame_index": frame, "label_pointer": f"0x{label_pointer:08x}",
              "label": f"0x{label:08x}", "credit_index": credit,
              "credit_delta": (frame - credit) % 3,
              "credit_wait_condition": (frame - credit) % 3 > 1,
              "gcm_control": f"0x{control:08x}", "put": f"0x{put:08x}",
              "get": f"0x{get:08x}", "reference": f"0x{ref:08x}"}
    if mapping:
        io, ea, length = mapping
        if not 0 < length <= 0x10000000 or io + length > 0x100000000 or ea + length > 0x100000000:
            raise ValueError("FIFO mapping size is outside the supported guest range")
        if not io <= get < io + length:
            raise ValueError("FIFO GET is outside the supplied IO mapping")
        address = ea + get - io
        command = u32(address)
        # RSX old jump command has bit 29 set and a 29-bit aligned IO target.
        old_jump = command & 0xe0000003 == 0x20000000
        result.update(fifo_get_address=f"0x{address:08x}", fifo_command=f"0x{command:08x}",
                      fifo_jump_to_self=old_jump and command & 0x1ffffffc == get,
                      fifo_mapping={"io": f"0x{io:08x}", "ea": f"0x{ea:08x}", "bytes": length})
    result["frame_index_unchanged_during_sample"] = u32(FRAME_INDEX) == frame
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--process-manifest", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--fifo-mapping", nargs=3, type=lambda x: int(x, 0),
                        metavar=("IO_BASE", "GUEST_BASE", "BYTES"),
                        help="explicit known IO mapping; no mapping is guessed")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    destination = args.out.resolve()
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("private memory observations must be outside tracked source or under ignored local/")
    from rpcs3_owned_memory import OwnedMemory
    with OwnedMemory(args.process_manifest) as memory:
        result = snapshot(memory.read, args.fifo_mapping)
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {destination}: credit delta {result['credit_delta']}, "
          f"FIFO GET {result['get']}, jump to self {result.get('fifo_jump_to_self', 'unmapped')}")


if __name__ == "__main__":
    main()
