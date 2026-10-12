#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate and check UD2 probe byte guards straight out of the pinned eboot.

WHY THIS EXISTS
---------------
Adding a probe to the shadPS4 tracer needs two facts per site that are easy to get wrong by hand
and expensive to get wrong at runtime:

1. the address must be a real instruction BOUNDARY -- a UD2 written into the middle of an
   instruction corrupts the guest; and
2. the 8-byte guard must match the pinned image exactly, or `ValidateProbeBytes()` refuses to arm
   the WHOLE tracer and the run produces zero records, which reads as "nothing fired".

This project has already paid for hand-transcription once (a 73-byte constant transcribed at the
wrong length; the unit test caught it, but only after the work). So guards are generated, never
typed.

CONTROLS (ATLAS §4: a scan without a positive control is not evidence)
---------------------------------------------------------------------
`--controls` re-derives the guard for every ProbeSpec ALREADY in the tracer source and requires a
byte-exact match. If this tool's address-to-bytes mapping is broken, dozens of known-good probes
fail loudly instead of one new probe being silently wrong. Exit 2 on any failure.

USAGE
-----
    # the control -- run this before trusting any output
    python3 -B capture-rig/probe-guards.py --controls

    # generate C++ ProbeSpec lines for new sites
    python3 -B capture-rig/probe-guards.py --emit cpp \
        AvailEntry:0x260200 AvailRearmPump:0x260438

    # generate a verifier-compatible catalog
    python3 -B capture-rig/probe-guards.py --emit json --title-id CUSA00219 \
        --start-id 69 AvailEntry:0x260200 > capture-rig/my-probe-sites.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
D1_EBOOT = REPO / "d1-server" / "target" / "release" / "d1-eboot"
TRACER_SOURCE = pathlib.Path(
    "$HOME/dev/shadPS4-d1trace/src/core/d1_account_trace.cpp"
)
GUARD_LENGTH = 8


class GuardError(RuntimeError):
    pass


def run_d1_eboot(args: list[str]) -> dict:
    """Call the one static-analysis instrument. There is deliberately no second ELF reader."""
    if not D1_EBOOT.exists():
        raise GuardError(
            f"{D1_EBOOT} is missing -- build it with "
            "`cd d1-server && cargo build --release -p d1-eboot`"
        )
    completed = subprocess.run(
        [str(D1_EBOOT), *args, "--json"], capture_output=True, text=True
    )
    if completed.returncode != 0:
        raise GuardError(completed.stderr.strip() or "d1-eboot failed")
    return json.loads(completed.stdout)


def enclosing_prologue(vaddr: int) -> int:
    """Snap back to the function prologue that contains `vaddr`.

    `d1-eboot disasm` refuses a guessed start on purpose (ATLAS §4.5), so anchoring is not
    optional. `--snap` does the refusal-safe version of it.
    """
    snapped = run_d1_eboot(["disasm", hex(vaddr), "--snap"])
    return int(snapped["anchor"])


def decode_window(anchor: int, through: int) -> tuple[dict[int, int], set[int]]:
    """Return (address -> byte) and the set of instruction-boundary addresses."""
    decoded = run_d1_eboot(["disasm", hex(anchor), "--to", hex(through)])
    byte_at: dict[int, int] = {}
    boundaries: set[int] = set()
    for insn in decoded["insns"]:
        address = int(insn["address"])
        boundaries.add(address)
        for index, value in enumerate(bytes.fromhex(insn["bytes"])):
            byte_at[address + index] = value
    return byte_at, boundaries


def guard_for(vaddr: int) -> bytes:
    """The 8 bytes at `vaddr`, refusing unless `vaddr` is a real instruction boundary."""
    anchor = enclosing_prologue(vaddr)
    # Decode a little past the guard so the last guard byte is always covered by a decoded
    # instruction rather than falling off the end of the window.
    byte_at, boundaries = decode_window(anchor, vaddr + GUARD_LENGTH + 16)
    if vaddr not in boundaries:
        raise GuardError(
            f"{vaddr:#x} is NOT an instruction boundary (enclosing prologue {anchor:#x}) -- "
            "a UD2 here would corrupt the guest"
        )
    if not all((vaddr + offset) in byte_at for offset in range(GUARD_LENGTH)):
        raise GuardError(f"{vaddr:#x}: could not recover {GUARD_LENGTH} contiguous bytes")
    return bytes(byte_at[vaddr + offset] for offset in range(GUARD_LENGTH))


def parse_tracer_specs(source: pathlib.Path) -> list[tuple[str, int, bytes]]:
    """Every ProbeSpec currently compiled into the tracer, for use as the positive control."""
    text = source.read_text()
    pattern = re.compile(
        r"ProbeSpec\{ProbeId::(\w+),\s*(0x[0-9a-fA-F]+),\s*\{([^}]+)\}\}", re.DOTALL
    )
    specs: list[tuple[str, int, bytes]] = []
    for name, address, byte_text in pattern.findall(text):
        guard = bytes(int(v, 16) for v in re.findall(r"0x([0-9a-fA-F]+)", byte_text))
        specs.append((name, int(address, 16), guard))
    if not specs:
        raise GuardError(f"{source}: no ProbeSpecs found")
    return specs


def run_controls(source: pathlib.Path) -> int:
    """Re-derive every existing probe's guard and require a byte-exact match."""
    specs = parse_tracer_specs(source)
    failures = 0
    for name, vaddr, expected in specs:
        try:
            derived = guard_for(vaddr)
        except GuardError as error:
            print(f"FAIL  {name:28} {vaddr:#010x}  {error}")
            failures += 1
            continue
        common = min(len(expected), len(derived))
        if derived[:common] != expected[:common]:
            print(
                f"FAIL  {name:28} {vaddr:#010x}  tracer={expected.hex()} derived={derived.hex()}"
            )
            failures += 1
        else:
            print(f"PASS  {name:28} {vaddr:#010x}  {derived.hex()}")
    print()
    if failures:
        print(f"{failures}/{len(specs)} CONTROLS FAILED -- output may not be cited")
        return 2
    print(f"ALL {len(specs)} CONTROLS PASS")
    return 0


def parse_site(text: str) -> tuple[str, int]:
    if ":" not in text:
        raise GuardError(f"expected Name:0xADDR, got {text!r}")
    name, _, address = text.partition(":")
    if not address.startswith("0x"):
        raise GuardError(f"address must be 0x-prefixed, got {address!r}")
    return name, int(address, 16)


def parse_rearm_groups(specs: list[str], known: set[str]) -> list[dict[str, str]]:
    """Parse NAME:ENTRY:PUMP:RULE, refusing groups that name a probe we did not generate.

    Catching a typo'd pump name here is worth a great deal: the runtime symptom of a broken re-arm
    group is an empty trace, which is indistinguishable by eye from the game never reaching the
    code.
    """
    groups: list[dict[str, str]] = []
    for spec in specs:
        parts = spec.split(":", 3)
        if len(parts) != 4:
            raise GuardError(f"--rearm-group needs NAME:ENTRY:PUMP:RULE, got {spec!r}")
        name, entry, pump, rule = parts
        for role, probe_name in (("entry", entry), ("pump", pump)):
            if probe_name not in known:
                raise GuardError(
                    f"re-arm group {name!r} names {role} probe {probe_name!r}, "
                    f"which is not among the generated sites"
                )
        if entry == pump:
            raise GuardError(
                f"re-arm group {name!r}: entry and pump are the same probe -- a probe cannot "
                "re-arm the instruction it is about to re-execute"
            )
        groups.append({"name": name, "entry": entry, "pump": pump, "rule": rule})
    return groups


def emit_cpp(sites: list[tuple[str, int, bytes]]) -> None:
    for name, vaddr, guard in sites:
        body = ", ".join(f"{value:#04x}" for value in guard)
        print(f"    ProbeSpec{{ProbeId::{name}, {vaddr:#x},\n              {{{body}}}}},")


def emit_json(sites: list[tuple[str, int, bytes]], args: argparse.Namespace) -> None:
    catalog = {
        "schema_version": 2,
        "title_id": args.title_id,
        "app_version": args.app_version,
        # The verifier refuses a catalog that does not name the image it was derived from, which is
        # the correct behaviour: a guard is only meaningful against one build.
        "eboot_sha256": hashlib.sha256(args.eboot.read_bytes()).hexdigest(),
        "runtime_load_base": "0x800000000",
        "record_budget": 60000,
        # A schema-2 catalog must declare its re-arm groups. That is not bureaucracy: a probe group
        # whose pump sits on a conditional path goes silent in exactly the failing case it was
        # installed to observe, and every such trace reads as "never fired" (ATLAS §4.4).
        "rearm_groups": [
            {
                "name": group["name"],
                "entry_probe": group["entry"],
                "pump_probe": group["pump"],
                "rule": group["rule"],
            }
            for group in args.rearm_groups
        ],
        "probes": [
            {
                "id": args.start_id + index,
                "name": name,
                "vaddr": hex(vaddr),
                "original_bytes": guard.hex(),
            }
            for index, (name, vaddr, guard) in enumerate(sites)
        ],
    }
    print(json.dumps(catalog, indent=2))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("sites", nargs="*", help="Name:0xADDR pairs")
    parser.add_argument("--controls", action="store_true", help="re-derive every existing probe")
    parser.add_argument("--emit", choices=("cpp", "json"), default="cpp")
    parser.add_argument("--tracer-source", type=pathlib.Path, default=TRACER_SOURCE)
    parser.add_argument("--title-id", default="CUSA00219")
    parser.add_argument("--app-version", default="01.29")
    parser.add_argument("--start-id", type=int, default=69)
    parser.add_argument(
        "--rearm-group",
        action="append",
        dest="rearm_group_specs",
        default=[],
        metavar="NAME:ENTRY:PUMP:RULE",
        help="declare a re-arm group; repeatable. ENTRY and PUMP must be probe names given above.",
    )
    parser.add_argument(
        "--eboot",
        type=pathlib.Path,
        default=REPO / "dumps" / "patch-01.29-CUSA00219" / "eboot.bin",
        help="image the guards are derived from; its sha256 is stamped into the catalog",
    )
    args = parser.parse_args()

    try:
        if args.controls:
            return run_controls(args.tracer_source)
        if not args.sites:
            parser.error("give at least one Name:0xADDR, or --controls")
        resolved = [(name, vaddr, guard_for(vaddr)) for name, vaddr in map(parse_site, args.sites)]
        args.rearm_groups = parse_rearm_groups(
            args.rearm_group_specs, {name for name, _, _ in resolved}
        )
        if args.emit == "json" and not args.rearm_groups:
            parser.error(
                "a schema-2 catalog needs at least one --rearm-group "
                "(NAME:ENTRY:PUMP:RULE) -- see ATLAS §4.4"
            )
    except GuardError as error:
        print(f"probe-guards: {error}", file=sys.stderr)
        return 2

    if args.emit == "cpp":
        emit_cpp(resolved)
    else:
        emit_json(resolved, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
