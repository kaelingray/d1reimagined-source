"""Dump the 01.24 startup-state name table from the decrypted EBOOT.

The client logs its progress as `bootflow:<name>`. The names live as 32-byte
strings in .rodata and are indexed by a pointer table; walking that table gives
the authoritative order of the whole startup path, which is what we have to
satisfy one state at a time:

    0 pregame ... 19 bootflow:bap_signin ... 27 setup:orbit
    30 setup:activity_host_setup ... 36 activity:in_world

`scripts/summarize_final_smoke.py` reads the same field out of a captured
bootflow object, so this table is the key for interpreting those captures.

    python scripts/d1_startup_states.py --elf local/RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf
    python scripts/d1_startup_states.py --elf <elf> --json states.json
"""

import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
try:
    from d1_eboot import Eboot  # noqa: E402
except ModuleNotFoundError:
    # run from anywhere: fall back to the repository's scripts folder
    for cand in (Path.cwd() / "scripts", Path(__file__).resolve().parent):
        if (cand / "d1_eboot.py").exists():
            sys.path.insert(0, str(cand))
            break
    from d1_eboot import Eboot  # noqa: E402

# The first entry is 'pregame'; a row of 0xffffffff terminates the table.
TABLE_START_GUESS = 0x012F7F00
NULLS = (0x00000000, 0xFFFFFFFF)


def read_table(elf: Eboot, start: int, limit: int = 64):
    """[(index, table_slot_addr, string_addr, name)] from the pointer table at `start`."""
    rows = []
    for i in range(limit):
        slot = start + i * 4
        raw = elf.read(slot, 4)
        if len(raw) != 4:
            break
        (ptr,) = struct.unpack(">I", raw)
        if ptr in NULLS:
            break
        try:
            name = elf.read(ptr, 96).split(b"\x00")[0].decode("ascii", "replace")
        except Exception:  # a pointer we cannot read is not a state name
            break
        rows.append((i, slot, ptr, name))
    return rows


def find_table(elf: Eboot, marker: bytes = b"pregame"):
    """Locate the table by scanning the .rodata pointer arrays for `marker`."""
    needle = marker.ljust(4, b"\x00")[:4] if len(marker) >= 4 else marker
    for base in range(0x012F0000, 0x01320000, 4):
        raw = elf.read(base, 4)
        if len(raw) != 4:
            break
        (ptr,) = struct.unpack(">I", raw)
        if ptr in NULLS:
            continue
        try:
            if elf.read(ptr, len(needle)) == needle:
                return base
        except Exception:
            continue
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", type=Path, default=Path(
        "local/RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf"))
    ap.add_argument("--start", type=lambda v: int(v, 0), default=None,
                    help="table start address (default: located by --marker)")
    ap.add_argument("--marker", default="pregame")
    ap.add_argument("--json", type=Path, help="also write the rows as JSON")
    args = ap.parse_args()

    elf = Eboot(args.elf)
    start = args.start
    if start is None:
        start = find_table(elf, args.marker.encode())
    if start is None:
        sys.exit(f"could not locate the startup-state table with marker {args.marker!r}")

    rows = read_table(elf, start)
    if not rows:
        sys.exit(f"no state names at {start:#x}")
    print(f"startup-state table at {start:#x}: {len(rows)} states\n")
    for i, slot, ptr, name in rows:
        print(f"{i:3d}  {slot:#010x}  {ptr:#010x}  {name}")

    if args.json:
        payload = {"table": f"{start:#x}", "states": [
            {"index": i, "slot": f"{slot:#x}", "string": f"{ptr:#x}", "name": n}
            for i, slot, ptr, n in rows]}
        args.json.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"\nwrote {args.json}")


if __name__ == "__main__":
    main()
