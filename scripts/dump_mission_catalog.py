"""Inventory local Destiny packages and preserve scenario/group research privately.

Accept both the disc package directory and the final update directory. Uses
the latest registered patch per package and verified blocks across folders.
Exports named tags, scenario headers, raw scenario/root tags, entity groups
and default replication baselines. It does not contain host mission scripts
or demonstrate that any mission is playable. Run output belongs outside Git.
"""

import argparse
import collections
import datetime
import hashlib
import json
import struct
from pathlib import Path

import d1_pkg


def scenario_header(data):
    if len(data) < 0x38 or struct.unpack_from(">I", data)[0] != len(data):
        raise ValueError("scenario header has an unexpected size")
    return {"entity_group": f"0x{struct.unpack_from('>I', data, 4)[0]:08x}",
            "checksum": f"0x{struct.unpack_from('>I', data, 8)[0]:08x}",
            "bubble_count": struct.unpack_from(">I", data, 0x34)[0]}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def dump(paths, out, oodle=None, all_named=False, investment_items=False, entry_index=False):
    out.mkdir(parents=True, exist_ok=True)
    raw_dir, group_dir = out / "tags", out / "groups"
    raw_dir.mkdir(exist_ok=True)
    group_dir.mkdir(exist_ok=True)
    entry_dir = out / "entries"
    if entry_index:
        entry_dir.mkdir(exist_ok=True)
    ps = d1_pkg.PackageSet(paths, oodle)
    catalog = {"created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "scope": "latest local package chains; static data, not playable mission coverage",
               "packages": [], "named_tags": [], "scenarios": [], "groups": [],
               "exported_tags": [], "errors": []}
    for number, (pkg_id, package) in enumerate(sorted(ps.by_id.items()), 1):
        package_info = {"id": f"0x{pkg_id:04x}", "path": str(package.path),
                        "patch": package.patch_id, "language": package.language,
                        "entries": len(package.entries), "named_tags": len(package.named),
                        "registration": f"0x{package.reg_value:016x}"}
        catalog["packages"].append(package_info)
        if entry_index:
            # Metadata covers unnamed assets too without exporting every texture,
            # audio or geometry payload. Preserve reference/class and patch-block
            # provenance for later targeted extraction.
            entries = [{**entry, "tag": f"0x{d1_pkg.tag_hash(pkg_id, entry['index']):08x}",
                        "reference": f"0x{entry['reference']:08x}"} for entry in package.entries]
            blocks = [{**block, "sha1": block["sha1"].hex()} for block in package.blocks]
            metadata = f"entries/{pkg_id:04x}.json"
            write_json(out / metadata, {"package": package_info, "entries": entries,
                                       "blocks": blocks,
                                       "table_hash_valid": {name: package.table_hash_ok(name)
                                                            for name in ("entries", "blocks", "named")}})
            package_info["entry_metadata"] = metadata
        for tag in package.named:
            catalog["named_tags"].append({**tag, "package": f"0x{pkg_id:04x}"})
            suffix = tag["name"].rsplit(":", 1)[-1]
            if not all_named and suffix not in ("root", "scenario_client", "scenario_fah"):
                continue
            try:
                where = d1_pkg.split_tag_hash(tag["hash"])
                source = ps.by_id.get(where[0]) if where else None
                if source is None or where[1] >= len(source.entries):
                    raise ValueError("named tag's source package or entry is unavailable")
                data = source.entry_data(where[1], oodle)
                label = suffix if suffix in ("root", "scenario_client", "scenario_fah") else "named"
                filename = f"{tag['hash']:08x}-{label}.bin"
                (raw_dir / filename).write_bytes(data)
                row = {"name": tag["name"], "tag": f"0x{tag['hash']:08x}", "package": source.path.name,
                       "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                       "raw": f"tags/{filename}"}
                catalog["exported_tags"].append(row)
                if suffix == "scenario_client":
                    row.update(scenario_header(data))
                    catalog["scenarios"].append(row)
            except (OSError, ValueError, struct.error, RuntimeError) as exc:
                catalog["errors"].append({"package": package.path.name,
                                          "tag": f"0x{tag['hash']:08x}", "error": str(exc)})
        for entry in package.entries:
            if entry["reference"] != d1_pkg.GROUP_CLASS:
                continue
            tag = d1_pkg.tag_hash(pkg_id, entry["index"])
            try:
                data = package.entry_data(entry["index"], oodle)
                key, objects = d1_pkg.entity_group(data)
                records = [{"index": i, "kind": kind, "name": name,
                            "replicated": kind not in d1_pkg.KINDS_NOT_REPLICATED,
                            "has_state_b": kind in d1_pkg.KINDS_WITH_B}
                           for i, (kind, name) in enumerate(objects)]
                baselines = [f"{key:#x}/{kind:#x}/{i}" + ("/1" if kind in d1_pkg.KINDS_WITH_B else "")
                             for i, (kind, _) in enumerate(objects)
                             if kind not in d1_pkg.KINDS_NOT_REPLICATED]
                write_json(group_dir / f"{tag:08x}.json",
                           {"tag": f"0x{tag:08x}", "key": f"0x{key:08x}", "objects": records})
                (group_dir / f"{tag:08x}-baselines.txt").write_text(",".join(baselines) + "\n", encoding="utf-8")
                (raw_dir / f"{tag:08x}-entity-group.bin").write_bytes(data)
                catalog["groups"].append({"tag": f"0x{tag:08x}", "key": f"0x{key:08x}",
                                          "objects": len(objects), "baselines": len(baselines),
                                          "metadata": f"groups/{tag:08x}.json"})
            except (OSError, ValueError, struct.error, RuntimeError) as exc:
                catalog["errors"].append({"package": package.path.name,
                                          "tag": f"0x{tag:08x}", "error": str(exc)})
        # Avoid retaining decompressed blocks while inventorying all planets.
        package.__dict__.pop("_decoded", None)
        if number % 25 == 0:
            print(f"{number}/{len(ps.by_id)} packages; {len(catalog['scenarios'])} scenarios; "
                  f"{len(catalog['groups'])} groups; {len(catalog['errors'])} errors", flush=True)
    # The named investment_globals tag is an outer wrapper. Its +8 points to
    # the content root used by 01.24's 0x1cedcc; that root's +0x24 names the
    # activity table used by 0x1cf0ac. Do not confuse the wrapper's 338-entry
    # tag at +0x20 with this actual definition table.
    try:
        wrapper_tag = next(tag["hash"] for tag in catalog["named_tags"]
                           if tag["name"] == "investment_globals")
        wrapper = ps.tag(wrapper_tag)
        root_tag = struct.unpack_from(">I", wrapper, 8)[0]
        content_root = ps.tag(root_tag)
        activity_tag = struct.unpack_from(">I", content_root, 0x24)[0]
        activity_data = ps.tag(activity_tag)
        from rpcs3_final124_activities import activity_blob
        activity_catalog = activity_blob(activity_data)
        activity_catalog["tag"] = f"0x{activity_tag:08x}"
        write_json(out / "activities-static.json", activity_catalog)
        catalog["activities"] = {"count": activity_catalog["count"], "tag": f"0x{activity_tag:08x}",
                                 "metadata": "activities-static.json"}
        for value, data, label in ((root_tag, content_root, "content-root"),
                                   (activity_tag, activity_data, "activity-definitions")):
            (raw_dir / f"{value:08x}-{label}.bin").write_bytes(data)
    except (OSError, ValueError, struct.error, RuntimeError, StopIteration, TypeError) as exc:
        catalog["errors"].append({"scope": "01.24 investment activity table", "error": str(exc)})
    catalog["counts"] = {key: len(catalog[key]) for key in
                         ("packages", "named_tags", "scenarios", "groups", "exported_tags", "errors")}
    if entry_index:
        catalog["entry_index"] = {"directory": "entries", "packages": len(ps.by_id),
                                  "entries": sum(len(p.entries) for p in ps.by_id.values()),
                                  "blocks": sum(len(p.blocks) for p in ps.by_id.values())}
    if investment_items:
        try:
            buckets, items = d1_pkg.investment_items(ps)
            libraries = d1_pkg.all_string_libraries(ps)
            strings = {}
            for library in libraries.values():
                for key, value in library.items():
                    strings.setdefault(key, value)
            write_json(out / "items.json", {"buckets": buckets, "items": items})
            write_json(out / "strings.json", {f"0x{key:08x}": value for key, value in strings.items()})
            write_json(out / "string-libraries.json",
                       {f"0x{tag:08x}": {f"0x{key:08x}": value for key, value in texts.items()}
                        for tag, texts in libraries.items()})
            catalog["investment"] = {"items": len(items), "buckets": len(buckets),
                                     "strings": len(strings), "metadata": "items.json",
                                     "string_libraries": len(libraries),
                                     "scoped_strings": "string-libraries.json",
                                     "flat_strings_scope": "search only; repeated library hashes keep first text"}
        except (OSError, ValueError, TypeError, struct.error, RuntimeError) as exc:
            catalog["errors"].append({"scope": "investment items and strings", "error": str(exc)})
            catalog["counts"]["errors"] = len(catalog["errors"])
    catalog["scenario_packages"] = dict(collections.Counter(
        row["package"].rsplit("_", 2)[0] for row in catalog["scenarios"]))
    for package in ps.by_id.values():
        package.data.close()
    write_json(out / "catalog.json", catalog)
    print(json.dumps(catalog["counts"]), flush=True)
    return catalog


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--oodle", type=Path)
    parser.add_argument("--all-named-tags", action="store_true",
                        help="also preserve every other named tag's raw data privately")
    parser.add_argument("--investment-items", action="store_true",
                        help="also preserve decoded equipment and localized text metadata")
    parser.add_argument("--entry-index", action="store_true",
                        help="also index every entry/block, including unnamed game assets")
    args = parser.parse_args()
    # This tool deliberately emits game data. Refuse a tracked-repository output
    # folder; private local/ and explicit research folders remain available.
    repo = Path(__file__).resolve().parents[1]
    destination = args.out.resolve()
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        parser.error("game-data output must be outside the tracked repository or under ignored local/")
    dump(args.paths, destination, d1_pkg.Oodle(args.oodle) if args.oodle else None,
         args.all_named_tags, args.investment_items, args.entry_index)


if __name__ == "__main__":
    main()
