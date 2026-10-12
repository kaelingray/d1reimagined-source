"""Build a diagnostic content manifest with selected update packages substituted.

The input packages stay where they are; only a small protobuf manifest is written.
Use this to compare final-build registration against the disc-only manifest.
"""

import argparse
import json
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
import signon_proto  # noqa: E402


def replacement_entry(path, markers=(1,)):
    match = re.fullmatch(r"ps3_.+?_([0-9a-f]{4})(?:_[a-z]{2,3})?_\d+\.pkg", path.name)
    if not match:
        raise ValueError(f"not an update package: {path.name}")
    with path.open("rb") as stream:
        header = stream.read(16)
    if len(header) != 16:
        raise ValueError(f"short package header: {path}")
    magic, version, package_id, marker, value = struct.unpack(">4HQ", header)
    if (magic, version, package_id) != (0x18, 4, int(match.group(1), 16)) or marker not in markers:
        raise ValueError(f"unexpected package header: {path}")
    return package_id, path.stem.encode("ascii"), value


def block_patches(path):
    """Patch ids of the other files this package's blocks are stored in (header +0x20 is its own
    patch id; the block table count/offset is at +0xd0, 32-byte records with the u16 patch id
    at +8; see d1_pkg.py)."""
    with path.open("rb") as stream:
        header = stream.read(0xd8)
        if len(header) != 0xd8:
            raise ValueError(f"short package header: {path}")
        own = struct.unpack_from(">H", header, 0x20)[0]
        count, offset = struct.unpack_from(">II", header, 0xd0)
        stream.seek(offset)
        table = stream.read(32 * count)
    if len(table) != 32 * count:
        raise ValueError(f"short block table: {path}")
    return sorted({struct.unpack_from(">H", table, 32 * i + 8)[0] for i in range(count)} - {own})


def chain_entries(selected, disc_packages, update_packages_dir, disc_by_name, allowed_updates):
    """Entries for the earlier patch files a selected package's blocks live in, oldest first.

    01.24 overlays keep most blocks in earlier files (e.g. 0172_2 in 0172_0 and 0172_1), and the
    client reads them only when the manifest registers those files too (FINAL124_BOTH_CHAINS,
    2026-10-01). Patch 0 comes from the disc; later patches from the allowed update files."""
    name = selected[1].decode("ascii") + ".pkg"
    path = update_packages_dir / name if update_packages_dir and (update_packages_dir / name).is_file() \
        else disc_packages / name
    stem = name[:-4].rsplit("_", 1)[0]
    out = []
    for patch in block_patches(path):
        other = f"{stem}_{patch}"
        if other.encode("ascii") in disc_by_name:
            out.append(disc_by_name[other.encode("ascii")])
            continue
        candidate = update_packages_dir / f"{other}.pkg" if update_packages_dir else None
        if candidate is None or not candidate.is_file() or (
                allowed_updates is not None and candidate.name not in allowed_updates):
            raise ValueError(f"{name}: blocks in patch {patch}, but {other}.pkg is not available")
        out.append(replacement_entry(candidate, markers=(0, 1)))
    return out


def build(disc_packages, replacements, guid, update_packages_dir=None, inventory_file=None, chains=False):
    if inventory_file is not None and update_packages_dir is None:
        raise ValueError("an update package inventory requires an update packages directory")
    if update_packages_dir is not None and not update_packages_dir.is_dir():
        raise ValueError(f"update packages directory does not exist: {update_packages_dir}")
    allowed_updates = None
    if inventory_file is not None:
        inventory = json.loads(inventory_file.read_text(encoding="utf-8"))
        packages, stale = inventory["packages"], set(inventory["stale"])
        if not isinstance(packages, dict) or not stale <= packages.keys():
            raise ValueError("update inventory has invalid packages or stale names")
        # Patch numbering restarted in 01.20. Old _5 files can coexist with the
        # final _4, so filenames alone cannot distinguish superseded chains.
        allowed_updates = packages.keys() - stale
    original = signon_proto.disc_packages(disc_packages)
    by_id = {entry[0]: entry for entry in original}
    if len(by_id) != len(original):
        raise ValueError("disc manifest has duplicate package IDs")
    if update_packages_dir:
        latest = {}
        pattern = re.compile(r"^ps3_.+?_([0-9a-f]{4})(?:_([a-z]{2,3}))?_(\d+)\.pkg$")
        for path in sorted(update_packages_dir.glob("*.pkg")):
            if allowed_updates is not None and path.name not in allowed_updates:
                continue
            match = pattern.fullmatch(path.name)
            if match and match.group(2) in (None, "en"):
                with path.open("rb") as stream:
                    header = stream.read(8)
                # the manifest lists patchable packages (marker 1); with chains also the
                # marker-0 shared manifests (0430-0432), whose 01.13+ copies hold the
                # updated entry masks (e.g. tag 0x8106000e for package 0172)
                if len(header) != 8 or struct.unpack(">4H", header)[3] not in ((0, 1) if chains else (1,)):
                    continue
                package_id, version = int(match.group(1), 16), int(match.group(3))
                if package_id not in latest or version > latest[package_id][0]:
                    latest[package_id] = (version, replacement_entry(path, (0, 1) if chains else (1,)))
        by_id.update({entry[0]: entry for _, entry in latest.values()})
    for path in replacements:
        entry = replacement_entry(path)
        if not update_packages_dir and entry[0] not in by_id:
            raise ValueError(f"replacement ID {entry[0]:04x} is absent from disc manifest")
        by_id[entry[0]] = entry
    chain_by_id = {}
    if chains:
        disc_by_name = {entry[1]: entry for entry in original}
        for package_id, entry in by_id.items():
            chain_by_id[package_id] = chain_entries(entry, disc_packages, update_packages_dir,
                                                    disc_by_name, allowed_updates)
    entries = []
    for package_id in sorted(by_id):
        entries.extend(chain_by_id.get(package_id, ()))
        entries.append(by_id[package_id])
    return signon_proto.content_manifest(guid.encode("ascii"), entries), entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--disc-packages", type=Path, required=True)
    parser.add_argument("--replace", type=Path, action="append", default=[])
    parser.add_argument("--update-packages-dir", type=Path,
                        help="include the highest numbered English or neutral update file per package ID")
    parser.add_argument("--inventory-file", type=Path,
                        help="final ps3_update inventory JSON; excludes stale and unlisted update files")
    parser.add_argument("--chains", action="store_true",
                        help="also register, before each selected package, the earlier patch files "
                        "its blocks are stored in (and the marker-0 shared manifests)")
    parser.add_argument("--guid", default="00000000-0000-0000-0000-000000000001")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    payload, entries = build(args.disc_packages, args.replace, args.guid,
                             args.update_packages_dir, args.inventory_file, args.chains)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(payload)
    print(f"wrote {args.out}: {len(payload)} bytes, {len(entries)} entries")
    for path in args.replace:
        package_id, name, value = replacement_entry(path)
        print(f"replacement {package_id:04x} {name.decode()} header=0x{value:016x}")


if __name__ == "__main__":
    main()
