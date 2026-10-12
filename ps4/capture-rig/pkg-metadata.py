#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read the UNENCRYPTED metadata out of a PS4 application package (`.pkg`).

WHAT THIS IS, AND WHAT IT IS NOT
--------------------------------
⚠ This reads the **PS4 application package** - the `\\x7fCNT` container Sony's CDN serves, e.g.
`UP0002-CUSA00219_00-DESTINYTHEGAME01-A0133-V0100_0.pkg`.

That is a **different format** from the one `d1-server/crates/d1-pkg` describes, which is Destiny's
own *content* package living inside the installed game. Two different containers that both got
called "pkg". `format-description.md`'s no-format-twice rule is not violated: neither describes the
other's layout.

**It cannot decrypt anything.** In a Destiny patch package only 2 of 30 entries are encrypted, but
the game image itself (and therefore `eboot.bin`) is inside the encrypted PFS. Getting an eboot
still requires the CFW console procedure in `CFW-DUMP-RUNBOOK.md`. What IS readable without keys:
`param.sfo`, `changeinfo/*.xml`, icons, and the entry table.

WHY IT EARNS ITS PLACE
----------------------
Two questions keep coming up and both are answered here in a second, without a 32 GB decrypt:

- **"Can the CFW console run this patch?"** → `SYSTEM_VER`. (01.33 needs fw 4.50; the console is
  on 10.01, so yes.)
- **"What actually changed between two patches?"** → diff mode. It will not give you code changes,
  but it gives firmware floor, build date, SDK generation and entitlement wiring - and `sdk_ver`
  being identical across two versions is real evidence that the runtime surface did not move.

USAGE
-----
    python3 -B capture-rig/pkg-metadata.py <pkg>              # entries + param.sfo
    python3 -B capture-rig/pkg-metadata.py <pkg> --changeinfo # patch notes, if any
    python3 -B capture-rig/pkg-metadata.py <a.pkg> --diff <b.pkg>
    python3 -B capture-rig/pkg-metadata.py --controls         # pinned expectations
"""

from __future__ import annotations

import argparse
import pathlib
import struct
import sys

PKG_MAGIC = b"\x7fCNT"
SFO_MAGIC = 0x46535000
FILENAME_TABLE_ID = 0x200
PARAM_SFO_ID = 0x1000
ENCRYPTED_FLAG = 0x8000_0000

REPO = pathlib.Path(__file__).resolve().parent.parent
# Pinned controls: two packages we hold and have verified by SHA-1.
CONTROLS = [
    (
        "01.29",
        REPO / "psn-acquisition/pkgs-us/patch-01.29"
        / "UP0002-CUSA00219_00-DESTINYTHEGAME01-A0129-V0100_0.pkg",
        {"APP_VER": "01.29", "TITLE_ID": "CUSA00219", "SYSTEM_VER": 0x04010000},
    ),
    (
        "01.33",
        REPO / "psn-acquisition/pkgs-us/patch-01.33"
        / "UP0002-CUSA00219_00-DESTINYTHEGAME01-A0133-V0100_0.pkg",
        {"APP_VER": "01.33", "TITLE_ID": "CUSA00219", "SYSTEM_VER": 0x04508000},
    ),
]


class PkgError(RuntimeError):
    pass


def read_entries(handle) -> list[dict]:
    handle.seek(0)
    header = handle.read(0x1000)
    if header[:4] != PKG_MAGIC:
        raise PkgError("not a PS4 application package (missing \\x7fCNT magic)")
    entry_count = struct.unpack_from(">I", header, 0x10)[0]
    table_offset = struct.unpack_from(">I", header, 0x18)[0]
    handle.seek(table_offset)
    table = handle.read(entry_count * 32)
    entries = []
    for index in range(entry_count):
        eid, name_offset, flags1, _flags2, offset, size = struct.unpack_from(
            ">IIIIII", table, index * 32
        )
        entries.append(
            {
                "id": eid,
                "name_offset": name_offset,
                "encrypted": bool(flags1 & ENCRYPTED_FLAG),
                "offset": offset,
                "size": size,
                "name": "",
            }
        )
    names = next((e for e in entries if e["id"] == FILENAME_TABLE_ID), None)
    if names:
        handle.seek(names["offset"])
        blob = handle.read(names["size"])
        for entry in entries:
            start = entry["name_offset"]
            if start < len(blob):
                end = blob.find(b"\x00", start)
                entry["name"] = blob[start : end if end >= 0 else None].decode(
                    "utf-8", "replace"
                )
    return entries


def read_sfo(handle, entries: list[dict]) -> dict:
    entry = next((e for e in entries if e["id"] == PARAM_SFO_ID), None)
    if entry is None:
        raise PkgError("package has no param.sfo entry")
    handle.seek(entry["offset"])
    blob = handle.read(entry["size"])
    magic, _version, key_offset, data_offset, count = struct.unpack_from("<IIIII", blob, 0)
    if magic != SFO_MAGIC:
        raise PkgError("param.sfo magic mismatch")
    values: dict[str, object] = {}
    for index in range(count):
        key_rel, fmt, length, _max_length, data_rel = struct.unpack_from(
            "<HHIII", blob, 0x14 + index * 0x10
        )
        key_start = key_offset + key_rel
        key = blob[key_start : blob.index(b"\x00", key_start)].decode()
        raw = blob[data_offset + data_rel : data_offset + data_rel + length]
        values[key] = (
            struct.unpack_from("<I", raw, 0)[0]
            if fmt == 0x0404
            else raw.rstrip(b"\x00").decode("utf-8", "replace")
        )
    return values


def firmware(value: object) -> str:
    if not isinstance(value, int):
        return str(value)
    return f"{value:#010x}  (firmware {(value >> 24) & 0xFF:x}.{(value >> 16) & 0xFF:02x})"


def split_pubtool(value: object) -> dict[str, str]:
    if not isinstance(value, str):
        return {}
    return dict(part.split("=", 1) for part in value.split(",") if "=" in part)


def show(path: pathlib.Path, changeinfo: bool) -> None:
    with path.open("rb") as handle:
        entries = read_entries(handle)
        sfo = read_sfo(handle, entries)
        print(f"=== {path.name} ===")
        for key in ("TITLE", "TITLE_ID", "APP_VER", "TARGET_APP_VER", "CATEGORY"):
            if key in sfo:
                print(f"  {key:16} {sfo[key]}")
        if "SYSTEM_VER" in sfo:
            print(f"  {'SYSTEM_VER':16} {firmware(sfo['SYSTEM_VER'])}")
        for key, value in split_pubtool(sfo.get("PUBTOOLINFO")).items():
            if key in ("c_date", "sdk_ver", "st_type"):
                print(f"  {key:16} {value}")
        print(f"\n  {len(entries)} entries, "
              f"{sum(1 for e in entries if e['encrypted'])} encrypted")
        for entry in entries:
            if entry["name"]:
                mark = "ENC" if entry["encrypted"] else "   "
                print(f"    {mark} {entry['size']:>10}  {entry['name']}")
        if changeinfo:
            print()
            for entry in entries:
                if entry["name"].startswith("changeinfo/"):
                    handle.seek(entry["offset"])
                    text = handle.read(entry["size"]).decode("utf-8", "replace")
                    print(f"  --- {entry['name']} ---")
                    print("    " + text.strip().replace("\n", "\n    "))


def diff(left: pathlib.Path, right: pathlib.Path) -> None:
    loaded = []
    for path in (left, right):
        with path.open("rb") as handle:
            loaded.append(read_sfo(handle, read_entries(handle)))
    a, b = loaded
    a.update({f"pub:{k}": v for k, v in split_pubtool(a.get("PUBTOOLINFO")).items()})
    b.update({f"pub:{k}": v for k, v in split_pubtool(b.get("PUBTOOLINFO")).items()})
    a.pop("PUBTOOLINFO", None)
    b.pop("PUBTOOLINFO", None)
    print(f"{'key':22} {left.name[-18:]:<26} {right.name[-18:]:<26}")
    print("-" * 78)
    for key in sorted(set(a) | set(b)):
        va, vb = a.get(key), b.get(key)
        if key == "SYSTEM_VER":
            va, vb = firmware(va), firmware(vb)
        sa, sb = str(va), str(vb)
        if sa == sb:
            continue
        print(f"  {key:20} {sa[:25]:<26} {sb[:25]:<26} <-- DIFFERS")


def run_controls() -> int:
    failures = 0
    for label, path, expected in CONTROLS:
        if not path.exists():
            print(f"SKIP  {label}  {path} not present")
            continue
        try:
            with path.open("rb") as handle:
                sfo = read_sfo(handle, read_entries(handle))
        except PkgError as error:
            print(f"FAIL  {label}  {error}")
            failures += 1
            continue
        for key, want in expected.items():
            got = sfo.get(key)
            if got != want:
                print(f"FAIL  {label}  {key}: expected {want!r}, got {got!r}")
                failures += 1
            else:
                print(f"PASS  {label}  {key} = {want!r}")
    print()
    if failures:
        print(f"{failures} CONTROL(S) FAILED - output may not be cited")
        return 2
    print("ALL CONTROLS PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("pkg", nargs="?", type=pathlib.Path)
    parser.add_argument("--diff", type=pathlib.Path, metavar="OTHER.pkg")
    parser.add_argument("--changeinfo", action="store_true")
    parser.add_argument("--controls", action="store_true")
    args = parser.parse_args()

    try:
        if args.controls:
            return run_controls()
        if args.pkg is None:
            parser.error("give a .pkg, or --controls")
        if args.diff:
            diff(args.pkg, args.diff)
        else:
            show(args.pkg, args.changeinfo)
    except PkgError as error:
        print(f"pkg-metadata: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
