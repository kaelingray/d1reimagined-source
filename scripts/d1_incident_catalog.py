"""Read incident identities and payload selectors from an extracted 01.24 catalog.

The incident database getter 0x5aef50 resolves the handle returned by 0x4696f4.
JOIN218 identifies tag 0x816564aa, class 0x80800659. Its array at +0x10 has
0x808003d3 records with stride 20. The selector at record +16 matches the
native client incident context's +0x134 in the observed cinematic case.
Definition blobs and the four auxiliary u16 fields are deliberately opaque;
this is not a loot grant, an incident-body encoder or a recovered event name.
Extracted inputs and generated reports must remain private.
"""

import argparse
import hashlib
import json
import struct
from pathlib import Path

from d1_pkg import dyn_array

RECORD_CLASS = 0x808003D3
STRIDE = 20
MAX_RECORDS = 65536
MAX_INPUT = 16 * 1024 * 1024


def parse_catalog(data):
    if not 0x50 <= len(data) <= MAX_INPUT:
        raise ValueError("incident catalog size is unsupported")
    try:
        count, first, kind = dyn_array(data, 0x10)
    except (ValueError, struct.error) as exc:
        raise ValueError("incident catalog array header is invalid") from exc
    if kind != RECORD_CLASS or not 0 < count <= MAX_RECORDS:
        raise ValueError("incident catalog record class or count is unsupported")
    if first < 0x20 or first + count * STRIDE > len(data):
        raise ValueError("incident catalog records exceed the input")
    rows = []
    for index in range(count):
        offset = first + index * STRIDE
        name_hash, relative = struct.unpack_from(">II", data, offset)
        definition = None
        definition_class = None
        if relative not in (0, 0xFFFFFFFF):
            # Relative to the pointer field, not to the containing record.
            displacement = relative - 0x100000000 if relative & 0x80000000 else relative
            definition = offset + 4 + displacement
            if definition < 0 or definition + 8 > len(data):
                raise ValueError(f"incident {index} definition exceeds the input")
            definition_class, = struct.unpack_from(">I", data, definition + 4)
        rows.append({"index": index, "name_hash": f"0x{name_hash:08x}",
                     "payload_selector": data[offset + 16],
                     "auxiliary_u16": list(struct.unpack_from(">4H", data, offset + 8)),
                     "definition_offset": definition,
                     "definition_class": (f"0x{definition_class:08x}"
                                          if definition_class is not None else None)})
    return {"record_count": count, "input_sha256": hashlib.sha256(data).hexdigest(),
            "records": rows,
            "scope": "Catalog identities and selectors only; names, auxiliary field meanings, incident bodies and native loot effects remain unverified."}


def private_output(path):
    destination = path.resolve()
    repo = Path(__file__).resolve().parents[1]
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        raise ValueError("private catalog reports must stay outside tracked source or under ignored local/")
    if destination.exists():
        raise ValueError("preserve previous reports; choose a new output path")
    return destination


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag_file", type=Path, help="already extracted private 01.24 incident catalog")
    parser.add_argument("--out", type=Path, required=True, help="new private JSON report")
    parser.add_argument("--hash", dest="hashes", action="append", type=lambda v: int(v, 0),
                        help="display records matching this name hash (repeatable)")
    args = parser.parse_args(argv)
    try:
        destination = private_output(args.out)
        with args.tag_file.open("rb") as source:
            result = parse_catalog(source.read(MAX_INPUT + 1))
        if args.hashes and any(not 0 <= h <= 0xFFFFFFFF for h in args.hashes):
            raise ValueError("name hashes must fit u32")
        with destination.open("x", encoding="utf-8") as output:
            json.dump(result, output, indent=2)
            output.write("\n")
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(f"wrote {destination}: {result['record_count']} incident records")
    for row in result["records"]:
        if args.hashes and int(row["name_hash"], 16) in args.hashes:
            print(json.dumps(row))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
