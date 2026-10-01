"""Build a diagnostic content manifest with selected update packages substituted.

The input packages stay where they are; only a small protobuf manifest is written.
Use this to compare final-build registration against the disc-only manifest.
"""

import argparse
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
import signon_proto  # noqa: E402


def replacement_entry(path):
    match = re.fullmatch(r"ps3_.+?_([0-9a-f]{4})(?:_[a-z]{2,3})?_\d+\.pkg", path.name)
    if not match:
        raise ValueError(f"not an update package: {path.name}")
    with path.open("rb") as stream:
        header = stream.read(16)
    if len(header) != 16:
        raise ValueError(f"short package header: {path}")
    magic, version, package_id, marker, value = struct.unpack(">4HQ", header)
    if (magic, version, package_id, marker) != (0x18, 4, int(match.group(1), 16), 1):
        raise ValueError(f"unexpected package header: {path}")
    return package_id, path.stem.encode("ascii"), value


def build(disc_packages, replacements, guid, update_packages_dir=None):
    original = signon_proto.disc_packages(disc_packages)
    by_id = {entry[0]: entry for entry in original}
    if len(by_id) != len(original):
        raise ValueError("disc manifest has duplicate package IDs")
    if update_packages_dir:
        latest = {}
        pattern = re.compile(r"^ps3_.+?_([0-9a-f]{4})(?:_([a-z]{2,3}))?_(\d+)\.pkg$")
        for path in sorted(update_packages_dir.glob("*.pkg")):
            match = pattern.fullmatch(path.name)
            if match and match.group(2) in (None, "en"):
                with path.open("rb") as stream:
                    header = stream.read(8)
                if len(header) != 8 or struct.unpack(">4H", header)[3] != 1:
                    continue  # the manifest lists patchable packages only
                package_id, version = int(match.group(1), 16), int(match.group(3))
                if package_id not in latest or version > latest[package_id][0]:
                    latest[package_id] = (version, replacement_entry(path))
        by_id.update({entry[0]: entry for _, entry in latest.values()})
    for path in replacements:
        entry = replacement_entry(path)
        if not update_packages_dir and entry[0] not in by_id:
            raise ValueError(f"replacement ID {entry[0]:04x} is absent from disc manifest")
        by_id[entry[0]] = entry
    entries = [by_id[package_id] for package_id in sorted(by_id)]
    return signon_proto.content_manifest(guid.encode("ascii"), entries), entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--disc-packages", type=Path, required=True)
    parser.add_argument("--replace", type=Path, action="append", default=[])
    parser.add_argument("--update-packages-dir", type=Path,
                        help="include the highest numbered English or neutral update file per package ID")
    parser.add_argument("--guid", default="00000000-0000-0000-0000-000000000001")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    payload, entries = build(args.disc_packages, args.replace, args.guid, args.update_packages_dir)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(payload)
    print(f"wrote {args.out}: {len(payload)} bytes, {len(entries)} entries")
    for path in args.replace:
        package_id, name, value = replacement_entry(path)
        print(f"replacement {package_id:04x} {name.decode()} header=0x{value:016x}")


if __name__ == "__main__":
    main()
