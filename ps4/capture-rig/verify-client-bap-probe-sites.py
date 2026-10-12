#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Verify send/receive probe byte guards against the pinned PS4 ELF."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import struct
import sys
from typing import Any


ELF_HEADER = struct.Struct("<16sHHIQQQIHHHHHH")
PROGRAM_HEADER = struct.Struct("<IIQQQQQQ")
PT_LOAD = 1


def parse_hex_address(value: str) -> int:
    if not isinstance(value, str) or not value.startswith("0x"):
        raise ValueError(f"address must be a 0x-prefixed string, got {value!r}")
    return int(value, 16)


def load_segments(blob: bytes) -> list[tuple[int, int, int]]:
    if len(blob) < ELF_HEADER.size:
        raise ValueError("ELF is shorter than its header")
    fields = ELF_HEADER.unpack_from(blob)
    ident = fields[0]
    if ident[:4] != b"\x7fELF" or ident[4] != 2 or ident[5] != 1:
        raise ValueError("expected a 64-bit little-endian ELF")
    phoff = fields[5]
    phentsize = fields[9]
    phnum = fields[10]
    if phentsize != PROGRAM_HEADER.size:
        raise ValueError(
            f"unexpected program-header size {phentsize}, expected {PROGRAM_HEADER.size}"
        )
    segments: list[tuple[int, int, int]] = []
    for index in range(phnum):
        offset = phoff + index * phentsize
        if offset + phentsize > len(blob):
            raise ValueError("program-header table extends beyond the file")
        header = PROGRAM_HEADER.unpack_from(blob, offset)
        p_type, p_offset, p_vaddr, p_filesz = header[0], header[2], header[3], header[5]
        if p_type == PT_LOAD and p_filesz:
            segments.append((p_vaddr, p_vaddr + p_filesz, p_offset))
    if not segments:
        raise ValueError("ELF has no file-backed PT_LOAD segment")
    return segments


def file_offset(vaddr: int, size: int, segments: list[tuple[int, int, int]]) -> int:
    for start, end, offset in segments:
        if start <= vaddr and vaddr + size <= end:
            return offset + vaddr - start
    raise ValueError(f"probe range {vaddr:#x}..{vaddr + size:#x} is not file-backed")


def parse_patch_range(value: str) -> tuple[int, int]:
    try:
        start_text, end_text = value.split(":", 1)
        start, end = int(start_text, 0), int(end_text, 0)
    except (ValueError, TypeError) as error:
        raise argparse.ArgumentTypeError("patch range must be START:END") from error
    if start >= end:
        raise argparse.ArgumentTypeError("patch range must be non-empty and half-open")
    return start, end


def parse_tracer_specs(source_path: pathlib.Path) -> dict[int, tuple[int, bytes]]:
    """Extract the enum ids and ProbeSpecs from the tracer for catalog parity checks."""
    source = source_path.read_text()
    try:
        enum_block = source.split("enum class ProbeId : u16 {", 1)[1].split("};", 1)[0]
    except IndexError as error:
        raise ValueError(f"{source_path}: ProbeId enum was not found") from error
    enum_ids = {
        name: int(value)
        for name, value in re.findall(r"^\s*(\w+)\s*=\s*(\d+),", enum_block, re.MULTILINE)
    }
    specs: dict[int, tuple[int, bytes]] = {}
    pattern = re.compile(
        r"ProbeSpec\{ProbeId::(\w+),\s*(0x[0-9a-fA-F]+),\s*\{([^}]+)\}\}",
        re.DOTALL,
    )
    for enum_name, address_text, byte_text in pattern.findall(source):
        if enum_name not in enum_ids:
            raise ValueError(f"{source_path}: ProbeSpec uses unknown ProbeId::{enum_name}")
        probe_id = enum_ids[enum_name]
        if probe_id in specs:
            raise ValueError(f"{source_path}: duplicate ProbeSpec id {probe_id}")
        guard = bytes(int(value, 16) for value in re.findall(r"0x([0-9a-fA-F]+)", byte_text))
        specs[probe_id] = (int(address_text, 16), guard)
    if not specs:
        raise ValueError(f"{source_path}: no ProbeSpecs were found")
    return specs


def verify_tracer_parity(
    catalog: dict[str, Any], source_path: pathlib.Path
) -> dict[str, Any]:
    """Require the runtime tracer to implement every catalog id at the same bytes/address."""
    specs = parse_tracer_specs(source_path)
    checked = 0
    for probe in catalog["probes"]:
        probe_id = int(probe["id"])
        name = str(probe["name"])
        if probe_id not in specs:
            raise ValueError(f"tracer lacks catalog probe {probe_id} ({name})")
        source_address, source_guard = specs[probe_id]
        catalog_address = parse_hex_address(probe["vaddr"])
        catalog_guard = bytes.fromhex(probe["original_bytes"])
        if source_address != catalog_address:
            raise ValueError(
                f"tracer/catalog address mismatch for {probe_id} ({name}): "
                f"{source_address:#x} != {catalog_address:#x}"
            )
        common = min(len(source_guard), len(catalog_guard))
        if not common or source_guard[:common] != catalog_guard[:common]:
            raise ValueError(
                f"tracer/catalog byte mismatch for {probe_id} ({name}): "
                f"{source_guard.hex()} != {catalog_guard.hex()}"
            )
        checked += 1
    return {"source": str(source_path.resolve()), "probe_count": checked, "result": "pass"}


def verify(
    eboot: pathlib.Path,
    catalog_path: pathlib.Path,
    patch_ranges: list[tuple[int, int]],
    tracer_source: pathlib.Path | None = None,
) -> dict[str, Any]:
    blob = eboot.read_bytes()
    catalog = json.loads(catalog_path.read_text())
    schema_version = catalog.get("schema_version")
    if schema_version not in (1, 2):
        raise ValueError("probe catalog schema_version must be 1 or 2")
    digest = hashlib.sha256(blob).hexdigest()
    if digest != catalog.get("eboot_sha256"):
        raise ValueError(
            f"eboot hash mismatch: got {digest}, expected {catalog.get('eboot_sha256')}"
        )
    probes = catalog.get("probes")
    if not isinstance(probes, list) or not probes:
        raise ValueError("probe catalog needs a non-empty probes array")
    segments = load_segments(blob)
    seen_ids: set[int] = set()
    seen_names: set[str] = set()
    checked: list[dict[str, Any]] = []
    for probe in probes:
        probe_id = int(probe["id"])
        name = str(probe["name"])
        if probe_id in seen_ids or name in seen_names:
            raise ValueError(f"duplicate probe id/name at {name}")
        seen_ids.add(probe_id)
        seen_names.add(name)
        vaddr = parse_hex_address(probe["vaddr"])
        expected = bytes.fromhex(probe["original_bytes"])
        if not expected:
            raise ValueError(f"probe {name} has an empty byte guard")
        offset = file_offset(vaddr, len(expected), segments)
        actual = blob[offset:offset + len(expected)]
        if actual != expected:
            raise ValueError(
                f"probe {name} byte mismatch at {vaddr:#x}: "
                f"got {actual.hex()}, expected {expected.hex()}"
            )
        probe_range = (vaddr, vaddr + len(expected))
        overlaps = [
            (start, end) for start, end in patch_ranges
            if probe_range[0] < end and start < probe_range[1]
        ]
        if overlaps:
            rendered = ", ".join(f"{start:#x}:{end:#x}" for start, end in overlaps)
            raise ValueError(f"probe {name} overlaps active patch range(s): {rendered}")
        checked.append({
            "id": probe_id,
            "name": name,
            "vaddr": f"0x{vaddr:x}",
            "file_offset": f"0x{offset:x}",
            "bytes": actual.hex(),
        })
    if schema_version == 1:
        rearm_groups = [catalog.get("rearm", {})]
    else:
        rearm_groups = catalog.get("rearm_groups")
        if not isinstance(rearm_groups, list) or not rearm_groups:
            raise ValueError("schema version 2 needs a non-empty rearm_groups array")
    seen_groups: set[str] = set()
    for rearm in rearm_groups:
        name = str(rearm.get("name", "legacy"))
        if name in seen_groups:
            raise ValueError(f"duplicate rearm group {name}")
        seen_groups.add(name)
        for field in ("entry_probe", "pump_probe"):
            if rearm.get(field) not in seen_names:
                raise ValueError(
                    f"rearm group {name} {field} does not name a declared probe"
                )
    report = {
        "schema_version": schema_version,
        "eboot": str(eboot.resolve()),
        "eboot_sha256": digest,
        "probe_count": len(checked),
        "patch_range_count": len(patch_ranges),
        "probes": checked,
        "result": "pass",
    }
    if tracer_source is not None:
        report["tracer_parity"] = verify_tracer_parity(catalog, tracer_source)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("eboot", type=pathlib.Path)
    parser.add_argument(
        "--catalog",
        type=pathlib.Path,
        default=pathlib.Path(__file__).with_name("client-bap-probe-sites.json"),
    )
    parser.add_argument(
        "--patch-range",
        action="append",
        default=[],
        type=parse_patch_range,
        metavar="START:END",
        help="reject a probe overlapping this active half-open guest-vaddr range",
    )
    parser.add_argument(
        "--tracer-source",
        type=pathlib.Path,
        help="also require catalog id/address/guard parity with d1_account_trace.cpp",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        report = verify(args.eboot, args.catalog, args.patch_range, args.tracer_source)
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"verify_client_probe_sites: {error}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(
            f"client probe sites: PASS ({report['probe_count']} probes, "
            f"eboot {report['eboot_sha256']})"
        )
        if "tracer_parity" in report:
            print(
                f"tracer parity: PASS ({report['tracer_parity']['probe_count']} catalog probes)"
            )
        for probe in report["probes"]:
            print(
                f"  {probe['id']:2d} {probe['name']:26} "
                f"{probe['vaddr']} file {probe['file_offset']} {probe['bytes']}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
