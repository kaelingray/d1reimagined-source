#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build a read-only Demonware source-file attribution atlas from a PS4 eboot.

The tool treats direct references to compiler-emitted ``__FILE__`` strings as
confirmed evidence.  Functions between a file's first and last confirmed
function are reported separately as an inferred bracket, never silently
promoted to confirmed ownership.

It understands the ELF LOAD mappings, the PS4/SCE RELA table, and the
``PT_GNU_EH_FRAME`` binary-search table.  No section headers or fixed address
bias are assumed.
"""

from __future__ import annotations

import argparse
import bisect
import csv
import hashlib
import io
import json
import re
import struct
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_64, Cs
from capstone.x86_const import (
    X86_OP_IMM,
    X86_OP_MEM,
    X86_OP_REG,
    X86_REG_R8,
    X86_REG_R9D,
    X86_REG_RCX,
    X86_REG_RIP,
)


PT_LOAD = 1
PT_DYNAMIC = 2
PT_GNU_EH_FRAME = 0x6474E550
PT_SCE_DYNLIBDATA = 0x61000000

DT_SCE_RELA = 0x6100002F
DT_SCE_RELASZ = 0x61000031
DT_SCE_RELAENT = 0x61000033

PATH_RE = re.compile(
    br"c:\\bungie\\external_libraries\\DemonWare\\45\\[^\x00]+?\.cpp",
    re.IGNORECASE,
)
METHOD_TOKEN_RE = re.compile(
    br"bd[A-Za-z0-9_]*(?:<[^>\x00]{1,80}>)?::(?:operator\[\]|~?[A-Za-z_][A-Za-z0-9_]*)"
)
RTTI_NAME_RE = re.compile(br"([0-9]{1,3})(bd[A-Za-z_][A-Za-z0-9_]*)(?=\x00)")
LOGGED_FUNCTION_RE = re.compile(r"^[A-Za-z_~][A-Za-z0-9_:<>~]*$")


@dataclass(frozen=True)
class Segment:
    kind: int
    flags: int
    file_offset: int
    vaddr: int
    file_size: int
    mem_size: int

    def contains_file_offset(self, offset: int) -> bool:
        return self.file_offset <= offset < self.file_offset + self.file_size

    def contains_vaddr(self, vaddr: int) -> bool:
        return self.vaddr <= vaddr < self.vaddr + self.file_size

    def file_to_vaddr(self, offset: int) -> int:
        return self.vaddr + offset - self.file_offset

    def vaddr_to_file(self, vaddr: int) -> int:
        return self.file_offset + vaddr - self.vaddr


@dataclass(frozen=True)
class FunctionExtent:
    start: int
    end: int
    fde: int | None
    evidence: str


@dataclass
class FileRecord:
    path: str
    string_file_offset: int
    string_vaddr: int
    direct_xrefs: list[int]
    immediate_xrefs: list[int]
    rela_xrefs: list[int]
    confirmed_functions: list[FunctionExtent]
    bracket_start: int | None
    bracket_end: int | None
    bracket_function_count: int
    foreign_confirmed_functions: list[int]


@dataclass
class MethodStringRecord:
    text: str
    string_file_offset: int
    string_vaddr: int
    method_names: list[str]
    direct_xrefs: list[int]
    rela_xrefs: list[int]
    functions: list[FunctionExtent]
    source_attribution: list[dict[str, str]]


@dataclass
class BDLogRecord:
    call_vaddr: int
    function_start: int
    function_end: int
    logged_function_names: list[str]
    source_paths: list[str]
    source_lines: list[int]


@dataclass(frozen=True)
class Relocation:
    write_vaddr: int
    kind: int
    addend: int


@dataclass
class VTableRecord:
    base_vaddr: int
    typeinfo_vaddr: int
    offset_to_top: int
    slots: list[int]
    slot_sources: list[list[dict[str, str]]]


@dataclass
class BaseClassRecord:
    class_name: str
    typeinfo_vaddr: int
    layout: str
    offset_flags: int | None


@dataclass
class RTTIRecord:
    class_name: str
    encoded_name: str
    name_file_offset: int
    name_vaddr: int
    typeinfo_vaddrs: list[int]
    bases: list[BaseClassRecord]
    vtables: list[VTableRecord]


def parse_int(value: str) -> int:
    return int(value, 0)


def parse_segments(data: bytes) -> list[Segment]:
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        raise ValueError("expected a little-endian ELF64 file")
    phoff = struct.unpack_from("<Q", data, 0x20)[0]
    phentsize = struct.unpack_from("<H", data, 0x36)[0]
    phnum = struct.unpack_from("<H", data, 0x38)[0]
    if phentsize < 56:
        raise ValueError(f"unexpected program-header size {phentsize}")
    result = []
    for index in range(phnum):
        at = phoff + index * phentsize
        kind, flags, off, va, _pa, filesz, memsz, _align = struct.unpack_from(
            "<IIQQQQQQ", data, at
        )
        result.append(Segment(kind, flags, off, va, filesz, memsz))
    return result


def file_to_vaddr(segments: list[Segment], offset: int) -> int:
    matches = [s for s in segments if s.kind == PT_LOAD and s.contains_file_offset(offset)]
    if len(matches) != 1:
        raise ValueError(f"file offset {offset:#x} maps through {len(matches)} LOAD segments")
    return matches[0].file_to_vaddr(offset)


def vaddr_to_file(segments: list[Segment], vaddr: int) -> int:
    matches = [s for s in segments if s.kind == PT_LOAD and s.contains_vaddr(vaddr)]
    if len(matches) != 1:
        raise ValueError(f"vaddr {vaddr:#x} maps through {len(matches)} LOAD segments")
    return matches[0].vaddr_to_file(vaddr)


def extract_paths(data: bytes, segments: list[Segment]) -> list[tuple[int, int, str]]:
    records = []
    seen = set()
    for match in PATH_RE.finditer(data):
        file_offset = match.start()
        vaddr = file_to_vaddr(segments, file_offset)
        path = match.group().decode("ascii")
        key = (vaddr, path.lower())
        if key not in seen:
            seen.add(key)
            records.append((file_offset, vaddr, path))
    return sorted(records, key=lambda item: item[1])


def extract_method_strings(
    data: bytes, segments: list[Segment]
) -> list[tuple[int, int, str, list[str]]]:
    """Find NUL-terminated diagnostic strings containing bdClass::method tokens."""
    by_start = {}
    for match in METHOD_TOKEN_RE.finditer(data):
        start = data.rfind(b"\x00", 0, match.start()) + 1
        end = data.find(b"\x00", match.end())
        if end < 0 or end - start > 4096:
            continue
        raw = data[start:end]
        if not raw or any(byte < 0x09 or (0x0E <= byte < 0x20) for byte in raw):
            continue
        try:
            text = raw.decode("ascii")
            vaddr = file_to_vaddr(segments, start)
        except (UnicodeDecodeError, ValueError):
            continue
        names = sorted(
            {token.group().decode("ascii") for token in METHOD_TOKEN_RE.finditer(raw)}
        )
        by_start[start] = (start, vaddr, text, names)
    return sorted(by_start.values(), key=lambda item: item[1])


def extract_rtti_names(
    data: bytes, segments: list[Segment]
) -> list[tuple[int, int, str, str]]:
    """Extract simple global-scope Itanium names whose class begins with bd."""
    result = []
    for match in RTTI_NAME_RE.finditer(data):
        encoded_start = match.start(1)
        class_name = match.group(2).decode("ascii")
        if int(match.group(1)) != len(class_name):
            continue
        encoded_name = (match.group(1) + match.group(2)).decode("ascii")
        try:
            vaddr = file_to_vaddr(segments, encoded_start)
        except ValueError:
            continue
        result.append((encoded_start, vaddr, encoded_name, class_name))
    return sorted(set(result), key=lambda item: item[1])


def read_dynamic(data: bytes, segments: list[Segment]) -> dict[int, list[int]]:
    dynamic = next((s for s in segments if s.kind == PT_DYNAMIC), None)
    if dynamic is None:
        return {}
    tags: dict[int, list[int]] = defaultdict(list)
    for at in range(dynamic.file_offset, dynamic.file_offset + dynamic.file_size, 16):
        tag, value = struct.unpack_from("<qQ", data, at)
        if tag == 0:
            break
        tags[tag].append(value)
    return tags


def scan_relocations(
    data: bytes, segments: list[Segment], targets: set[int]
) -> tuple[dict[int, list[int]], Counter[int], list[Relocation]]:
    tags = read_dynamic(data, segments)
    dynlib = next((s for s in segments if s.kind == PT_SCE_DYNLIBDATA), None)
    required = (DT_SCE_RELA, DT_SCE_RELASZ, DT_SCE_RELAENT)
    if dynlib is None or any(tag not in tags for tag in required):
        return {}, Counter(), []
    rela_offset = dynlib.file_offset + tags[DT_SCE_RELA][0]
    rela_size = tags[DT_SCE_RELASZ][0]
    rela_entry = tags[DT_SCE_RELAENT][0]
    if rela_entry != 24 or rela_size % rela_entry:
        raise ValueError(
            f"unexpected SCE RELA shape: size={rela_size:#x}, entry={rela_entry:#x}"
        )
    if rela_offset + rela_size > len(data):
        raise ValueError("SCE RELA table extends beyond file")
    hits: dict[int, list[int]] = defaultdict(list)
    types: Counter[int] = Counter()
    relocations = []
    for at in range(rela_offset, rela_offset + rela_size, rela_entry):
        write_vaddr, info, addend = struct.unpack_from("<QQq", data, at)
        reloc_type = info & 0xFFFFFFFF
        types[reloc_type] += 1
        relocations.append(Relocation(write_vaddr, reloc_type, addend))
        if addend in targets:
            hits[addend].append(write_vaddr)
    return dict(hits), types, relocations


def decode_eh_frame_header(
    data: bytes, segments: list[Segment]
) -> tuple[list[FunctionExtent], dict[str, int]]:
    header = next((s for s in segments if s.kind == PT_GNU_EH_FRAME), None)
    if header is None:
        raise ValueError("PT_GNU_EH_FRAME is missing")
    at = header.file_offset
    version, ptr_enc, count_enc, table_enc = struct.unpack_from("<BBBB", data, at)
    expected = (1, 0x1B, 0x03, 0x3B)
    if (version, ptr_enc, count_enc, table_enc) != expected:
        raise ValueError(
            "unsupported eh_frame_hdr encodings: "
            f"{version:#x}/{ptr_enc:#x}/{count_enc:#x}/{table_enc:#x}"
        )
    eh_ptr_field_vaddr = header.vaddr + 4
    eh_frame_vaddr = eh_ptr_field_vaddr + struct.unpack_from("<i", data, at + 4)[0]
    count = struct.unpack_from("<I", data, at + 8)[0]
    table_at = at + 12
    if table_at + count * 8 > at + header.file_size:
        raise ValueError("eh_frame_hdr search table extends beyond its segment")

    extents: dict[int, FunctionExtent] = {}
    mismatched_starts = 0
    for index in range(count):
        start_rel, fde_rel = struct.unpack_from("<ii", data, table_at + index * 8)
        table_start = header.vaddr + start_rel
        fde_vaddr = header.vaddr + fde_rel
        fde_at = vaddr_to_file(segments, fde_vaddr)
        length = struct.unpack_from("<I", data, fde_at)[0]
        if length == 0 or length == 0xFFFFFFFF or length < 12:
            raise ValueError(f"unsupported FDE length {length:#x} at {fde_vaddr:#x}")
        initial_field_vaddr = fde_vaddr + 8
        initial = initial_field_vaddr + struct.unpack_from("<i", data, fde_at + 8)[0]
        size = struct.unpack_from("<I", data, fde_at + 12)[0]
        if initial != table_start:
            mismatched_starts += 1
        extent = FunctionExtent(initial, initial + size, fde_vaddr, "eh_frame")
        prior = extents.get(initial)
        if prior is None or extent.end > prior.end:
            extents[initial] = extent
    ordered = sorted(extents.values(), key=lambda item: item.start)
    meta = {
        "header_vaddr": header.vaddr,
        "eh_frame_vaddr": eh_frame_vaddr,
        "table_entries": count,
        "unique_extents": len(ordered),
        "mismatched_starts": mismatched_starts,
    }
    return ordered, meta


def find_function(extents: list[FunctionExtent], starts: list[int], addr: int) -> FunctionExtent | None:
    index = bisect.bisect_right(starts, addr) - 1
    if index < 0:
        return None
    candidate = extents[index]
    return candidate if addr < candidate.end else None


def scan_code_references(
    data: bytes,
    segments: list[Segment],
    targets: set[int],
    controls: dict[int, int],
    observed_call_targets: set[int],
) -> tuple[
    dict[int, list[int]],
    dict[int, list[int]],
    dict[int, tuple[str, int]],
    dict[int, list[int]],
]:
    rip_hits: dict[int, list[int]] = defaultdict(list)
    immediate_hits: dict[int, list[int]] = defaultdict(list)
    control_results: dict[int, tuple[str, int]] = {}
    calls_by_target: dict[int, list[int]] = defaultdict(list)
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    md.detail = True
    md.skipdata = True
    for segment in segments:
        if segment.kind != PT_LOAD or not (segment.flags & 1):
            continue
        blob = data[segment.file_offset : segment.file_offset + segment.file_size]
        for insn in md.disasm(blob, segment.vaddr):
            # Capstone represents skipped inline data as an instruction with id
            # zero; asking it for operands raises CS_ERR_SKIPDATA.
            if insn.id == 0:
                continue
            if insn.address in controls and insn.operands:
                operand = insn.operands[0]
                if operand.type == X86_OP_IMM:
                    control_results[insn.address] = (insn.mnemonic, operand.imm)
            if insn.mnemonic == "call" and insn.operands:
                operand = insn.operands[0]
                if operand.type == X86_OP_IMM and operand.imm in observed_call_targets:
                    calls_by_target[operand.imm].append(insn.address)
            for operand in insn.operands:
                if operand.type == X86_OP_MEM and operand.mem.base == X86_REG_RIP:
                    target = insn.address + insn.size + operand.mem.disp
                    if target in targets:
                        rip_hits[target].append(insn.address)
                # A PIE should materialize these addresses with RIP-relative LEA.
                # Only movabs has unambiguous absolute-address semantics here;
                # branch immediates that numerically land in rodata are not data
                # references and executable-segment data can disassemble as
                # plausible small immediates.
                elif (
                    operand.type == X86_OP_IMM
                    and insn.mnemonic == "movabs"
                    and operand.imm in targets
                ):
                    immediate_hits[operand.imm].append(insn.address)
    return dict(rip_hits), dict(immediate_hits), control_results, dict(calls_by_target)


def recover_prologue_extent(
    data: bytes,
    segments: list[Segment],
    xrefs: list[int],
) -> FunctionExtent | None:
    """Conservatively recover an xref-bearing function absent from eh_frame.

    This fallback requires the canonical frame-pointer prologue.  Its end is
    the first decoded RET after the final xref and before the next canonical
    prologue.  The evidence label keeps it distinct from an unwind-backed
    extent.
    """
    if not xrefs:
        return None
    first_xref = min(xrefs)
    executable = next(
        (
            segment
            for segment in segments
            if segment.kind == PT_LOAD and (segment.flags & 1) and segment.contains_vaddr(first_xref)
        ),
        None,
    )
    if executable is None or any(not executable.contains_vaddr(xref) for xref in xrefs):
        return None
    blob = data[executable.file_offset : executable.file_offset + executable.file_size]
    relative_xref = first_xref - executable.vaddr
    prologue = b"\x55\x48\x89\xE5"
    start_rel = blob.rfind(prologue, max(0, relative_xref - 0x4000), relative_xref + 1)
    if start_rel < 0:
        return None
    start = executable.vaddr + start_rel
    next_rel = blob.find(prologue, start_rel + len(prologue))
    bound = (
        executable.vaddr + next_rel
        if next_rel >= 0
        else executable.vaddr + executable.file_size
    )
    if max(xrefs) >= bound:
        return None
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    region = blob[start_rel : bound - executable.vaddr]
    for insn in md.disasm(region, start):
        if insn.address > max(xrefs) and insn.mnemonic.startswith("ret"):
            return FunctionExtent(start, insn.address + insn.size, None, "canonical_prologue")
    return None


def read_cstring_vaddr(data: bytes, segments: list[Segment], vaddr: int) -> str | None:
    try:
        offset = vaddr_to_file(segments, vaddr)
    except ValueError:
        return None
    end = data.find(b"\x00", offset, min(len(data), offset + 4096))
    if end < 0:
        return None
    try:
        return data[offset:end].decode("ascii")
    except UnicodeDecodeError:
        return None


def extract_bd_log_records(
    data: bytes,
    segments: list[Segment],
    extents: list[FunctionExtent],
    starts: list[int],
    callsites: list[int],
    logger_target: int,
    path_targets: set[int],
) -> list[BDLogRecord]:
    """Recover bdLog file/function/line arguments from simple static setup.

    The accepted ABI is independently controlled in D1: rcx must resolve to a
    discovered __FILE__ path, r8 to an identifier-like string, and the known
    call at 0x10421F2 must recover bdPlatformTrulyRandomInit.
    """
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    md.detail = True
    result = []
    for callsite in sorted(callsites):
        function = find_function(extents, starts, callsite)
        if function is None:
            continue
        function_offset = vaddr_to_file(segments, function.start)
        instructions = list(
            md.disasm(data[function_offset : function_offset + (callsite - function.start)], function.start)
        )
        previous_call_end = function.start
        for insn in instructions:
            if insn.mnemonic == "call":
                previous_call_end = insn.address + insn.size
        window_start = max(previous_call_end, callsite - 0x180)
        names = set()
        source_paths = set()
        lines = set()
        # Track RIP-relative string addresses through simple register moves.
        # This covers compiler setups such as
        #   lea r14, [rip + __FUNCTION__]
        #   ...
        #   mov r8, r14
        # without pretending to be a full data-flow engine.  The state begins
        # after the previous call and is invalidated whenever a destination is
        # overwritten by an untracked value.
        register_strings: dict[int, int] = {}
        for insn in instructions:
            if insn.address < window_start or not insn.operands:
                continue
            destination = insn.operands[0]
            if (
                insn.mnemonic == "lea"
                and len(insn.operands) == 2
                and destination.type != X86_OP_MEM
                and insn.operands[1].type == X86_OP_MEM
                and insn.operands[1].mem.base == X86_REG_RIP
            ):
                target = insn.address + insn.size + insn.operands[1].mem.disp
                register_strings[destination.reg] = target
                if destination.reg == X86_REG_RCX and target in path_targets:
                    path = read_cstring_vaddr(data, segments, target)
                    if path is not None:
                        source_paths.add(path)
                elif destination.reg == X86_REG_R8:
                    name = read_cstring_vaddr(data, segments, target)
                    if name is not None and LOGGED_FUNCTION_RE.fullmatch(name):
                        names.add(name)
            elif (
                insn.mnemonic == "mov"
                and len(insn.operands) == 2
                and destination.type == X86_OP_REG
                and insn.operands[1].type == X86_OP_REG
            ):
                target = register_strings.get(insn.operands[1].reg)
                if target is None:
                    register_strings.pop(destination.reg, None)
                else:
                    register_strings[destination.reg] = target
                    if destination.reg == X86_REG_RCX and target in path_targets:
                        path = read_cstring_vaddr(data, segments, target)
                        if path is not None:
                            source_paths.add(path)
                    elif destination.reg == X86_REG_R8:
                        name = read_cstring_vaddr(data, segments, target)
                        if name is not None and LOGGED_FUNCTION_RE.fullmatch(name):
                            names.add(name)
            elif (
                insn.mnemonic == "mov"
                and len(insn.operands) == 2
                and destination.reg == X86_REG_R9D
                and insn.operands[1].type == X86_OP_IMM
                and 0 <= insn.operands[1].imm <= 1_000_000
            ):
                lines.add(insn.operands[1].imm)
            elif destination.type == X86_OP_REG:
                register_strings.pop(destination.reg, None)
        result.append(
            BDLogRecord(
                call_vaddr=callsite,
                function_start=function.start,
                function_end=function.end,
                logged_function_names=sorted(names),
                source_paths=sorted(source_paths),
                source_lines=sorted(lines),
            )
        )
    return result


def build_atlas(
    data: bytes,
    tail_control: tuple[int, int],
    bd_log_target: int,
    bd_log_control_call: int,
) -> dict:
    segments = parse_segments(data)
    paths = extract_paths(data, segments)
    method_strings = extract_method_strings(data, segments)
    rtti_names = extract_rtti_names(data, segments)
    path_targets = {vaddr for _offset, vaddr, _path in paths}
    method_targets = {vaddr for _offset, vaddr, _text, _names in method_strings}
    rtti_name_targets = {vaddr for _offset, vaddr, _encoded, _name in rtti_names}
    code_targets = path_targets | method_targets
    rela_targets = code_targets | rtti_name_targets
    extents, eh_meta = decode_eh_frame_header(data, segments)
    starts = [item.start for item in extents]
    rip_hits, immediate_hits, control_results, calls_by_target = scan_code_references(
        data,
        segments,
        code_targets,
        {tail_control[0]: tail_control[1]},
        {bd_log_target},
    )
    rela_hits, rela_types, relocations = scan_relocations(data, segments, rela_targets)

    actual_control = control_results.get(tail_control[0])
    control_ok = actual_control is not None and actual_control[0] == "jmp" and actual_control[1] == tail_control[1]

    owners_by_function: dict[int, set[int]] = defaultdict(set)
    functions_by_target: dict[int, dict[int, FunctionExtent]] = defaultdict(dict)
    unmapped_xrefs = []
    # Only compiler-emitted __FILE__ targets establish source ownership.
    # Method diagnostics are resolved afterward and may inherit that ownership,
    # but can never become owners themselves.
    for target in path_targets:
        target_xrefs = rip_hits.get(target, []) + immediate_hits.get(target, [])
        pending = []
        for xref in target_xrefs:
            function = find_function(extents, starts, xref)
            if function is None:
                pending.append(xref)
                continue
            functions_by_target[target][function.start] = function
            owners_by_function[function.start].add(target)
        if pending:
            fallback = recover_prologue_extent(data, segments, pending)
            if fallback is None:
                unmapped_xrefs.extend(pending)
            else:
                functions_by_target[target][fallback.start] = fallback
                owners_by_function[fallback.start].add(target)

    all_extents_by_start = {function.start: function for function in extents}
    for functions in functions_by_target.values():
        for function in functions.values():
            prior = all_extents_by_start.get(function.start)
            if prior is None or function.end > prior.end:
                all_extents_by_start[function.start] = function
    attributed_extents = sorted(all_extents_by_start.values(), key=lambda item: item.start)
    attributed_starts = [item.start for item in attributed_extents]
    bd_log_records = extract_bd_log_records(
        data,
        segments,
        attributed_extents,
        attributed_starts,
        calls_by_target.get(bd_log_target, []),
        bd_log_target,
        path_targets,
    )

    records = []
    for file_offset, target, path in paths:
        confirmed = sorted(functions_by_target[target].values(), key=lambda item: item.start)
        bracket_start = confirmed[0].start if confirmed else None
        bracket_end = max(item.end for item in confirmed) if confirmed else None
        bracket_functions = []
        foreign = []
        if bracket_start is not None and bracket_end is not None:
            lo = bisect.bisect_left(attributed_starts, bracket_start)
            hi = bisect.bisect_left(attributed_starts, bracket_end)
            bracket_functions = [
                item
                for item in attributed_extents[lo:hi]
                if item.start >= bracket_start and item.end <= bracket_end
            ]
            for function in bracket_functions:
                if any(owner != target for owner in owners_by_function.get(function.start, set())):
                    foreign.append(function.start)
        records.append(
            FileRecord(
                path=path,
                string_file_offset=file_offset,
                string_vaddr=target,
                direct_xrefs=sorted(rip_hits.get(target, [])),
                immediate_xrefs=sorted(immediate_hits.get(target, [])),
                rela_xrefs=sorted(rela_hits.get(target, [])),
                confirmed_functions=confirmed,
                bracket_start=bracket_start,
                bracket_end=bracket_end,
                bracket_function_count=len(bracket_functions),
                foreign_confirmed_functions=sorted(set(foreign)),
            )
        )

    cross_owned = {
        start: sorted(targets_at_start)
        for start, targets_at_start in owners_by_function.items()
        if len(targets_at_start) > 1
    }
    code_order = sorted(
        records,
        key=lambda record: (
            record.bracket_start is None,
            record.bracket_start if record.bracket_start is not None else record.string_vaddr,
        ),
    )
    string_order_matches_code_order = [record.path for record in records] == [
        record.path for record in code_order
    ]
    bracket_pairs = [
        (record.bracket_start, record.bracket_end)
        for record in code_order
        if record.bracket_start is not None and record.bracket_end is not None
    ]
    non_overlapping_brackets = all(
        left_end <= right_start
        for (_left_start, left_end), (right_start, _right_end) in zip(
            bracket_pairs, bracket_pairs[1:]
        )
    )
    component_path_counts: Counter[str] = Counter()
    for record in records:
        component_path_counts[short_path(record.path).split("\\", 1)[0]] += 1
    method_records = []
    path_by_target = {record.string_vaddr: record.path for record in records}
    for file_offset, target, text, names in method_strings:
        target_xrefs = rip_hits.get(target, []) + immediate_hits.get(target, [])
        method_functions: dict[int, FunctionExtent] = {}
        pending = []
        for xref in target_xrefs:
            function = find_function(extents, starts, xref)
            if function is None:
                pending.append(xref)
            else:
                method_functions[function.start] = function
        if pending:
            fallback = recover_prologue_extent(data, segments, pending)
            if fallback is None:
                unmapped_xrefs.extend(pending)
            else:
                method_functions[fallback.start] = fallback

        attributions = set()
        for function in method_functions.values():
            direct_owners = owners_by_function.get(function.start, set())
            for owner in direct_owners:
                attributions.add((path_by_target[owner], "confirmed-file-xref"))
            if direct_owners:
                continue
            for record in records:
                if (
                    record.bracket_start is not None
                    and record.bracket_end is not None
                    and record.bracket_start <= function.start
                    and function.end <= record.bracket_end
                ):
                    attributions.add((record.path, "inferred-file-bracket"))
        method_records.append(
            MethodStringRecord(
                text=text,
                string_file_offset=file_offset,
                string_vaddr=target,
                method_names=names,
                direct_xrefs=sorted(rip_hits.get(target, [])),
                rela_xrefs=sorted(rela_hits.get(target, [])),
                functions=sorted(method_functions.values(), key=lambda item: item.start),
                source_attribution=[
                    {"path": path, "evidence": evidence}
                    for path, evidence in sorted(attributions)
                ],
            )
        )

    relocations_by_addend: dict[int, list[Relocation]] = defaultdict(list)
    relocations_by_write: dict[int, list[Relocation]] = defaultdict(list)
    for relocation in relocations:
        relocations_by_addend[relocation.addend].append(relocation)
        relocations_by_write[relocation.write_vaddr].append(relocation)

    def is_executable(vaddr: int) -> bool:
        return any(
            segment.kind == PT_LOAD
            and (segment.flags & 1)
            and segment.vaddr <= vaddr < segment.vaddr + segment.file_size
            for segment in segments
        )

    def sources_for_code(vaddr: int) -> list[dict[str, str]]:
        function = find_function(extents, starts, vaddr)
        function_start = function.start if function is not None else vaddr
        result = set()
        for owner in owners_by_function.get(function_start, set()):
            result.add((path_by_target[owner], "confirmed-file-xref"))
        if not result:
            for record in records:
                if (
                    record.bracket_start is not None
                    and record.bracket_end is not None
                    and record.bracket_start <= vaddr < record.bracket_end
                ):
                    result.add((record.path, "inferred-file-bracket"))
        return [
            {"path": path, "evidence": evidence}
            for path, evidence in sorted(result)
        ]

    typeinfo_to_class = {}
    for _file_offset, name_vaddr, _encoded_name, class_name in rtti_names:
        for write_vaddr in rela_hits.get(name_vaddr, []):
            if write_vaddr >= 8:
                typeinfo_to_class[write_vaddr - 8] = class_name

    rtti_records = []
    for file_offset, name_vaddr, encoded_name, class_name in rtti_names:
        typeinfos = sorted(
            {
                write_vaddr - 8
                for write_vaddr in rela_hits.get(name_vaddr, [])
                if write_vaddr >= 8
            }
        )
        vtables_by_base = {}
        for typeinfo in typeinfos:
            for typeinfo_reference in relocations_by_addend.get(typeinfo, []):
                typeinfo_slot = typeinfo_reference.write_vaddr
                vtable_base = typeinfo_slot - 8
                try:
                    base_file_offset = vaddr_to_file(segments, vtable_base)
                except ValueError:
                    continue
                offset_to_top = struct.unpack_from("<q", data, base_file_offset)[0]
                first_slot = typeinfo_slot + 8
                first_targets = [
                    relocation.addend
                    for relocation in relocations_by_write.get(first_slot, [])
                    if is_executable(relocation.addend)
                ]
                if len(first_targets) != 1:
                    continue
                slots = []
                slot = first_slot
                while len(slots) < 256:
                    executable_targets = [
                        relocation.addend
                        for relocation in relocations_by_write.get(slot, [])
                        if is_executable(relocation.addend)
                    ]
                    if len(executable_targets) != 1:
                        break
                    slots.append(executable_targets[0])
                    slot += 8
                if slots:
                    vtables_by_base[vtable_base] = VTableRecord(
                        base_vaddr=vtable_base,
                        typeinfo_vaddr=typeinfo,
                        offset_to_top=offset_to_top,
                        slots=slots,
                        slot_sources=[sources_for_code(target) for target in slots],
                    )
        rtti_records.append(
            RTTIRecord(
                class_name=class_name,
                encoded_name=encoded_name,
                name_file_offset=file_offset,
                name_vaddr=name_vaddr,
                typeinfo_vaddrs=typeinfos,
                bases=[],
                vtables=sorted(vtables_by_base.values(), key=lambda item: item.base_vaddr),
            )
        )

    for record in rtti_records:
        bases = {}
        for typeinfo in record.typeinfo_vaddrs:
            single_targets = [
                relocation.addend
                for relocation in relocations_by_write.get(typeinfo + 16, [])
                if relocation.addend in typeinfo_to_class
            ]
            if len(single_targets) == 1:
                target = single_targets[0]
                bases[(target, "single")] = BaseClassRecord(
                    class_name=typeinfo_to_class[target],
                    typeinfo_vaddr=target,
                    layout="single",
                    offset_flags=None,
                )
                continue

            try:
                vmi_header_offset = vaddr_to_file(segments, typeinfo + 16)
            except ValueError:
                continue
            _flags, count = struct.unpack_from("<II", data, vmi_header_offset)
            if count == 0 or count > 32:
                continue
            candidate_bases = []
            for index in range(count):
                base_slot = typeinfo + 24 + index * 16
                targets_at_slot = [
                    relocation.addend
                    for relocation in relocations_by_write.get(base_slot, [])
                    if relocation.addend in typeinfo_to_class
                ]
                if len(targets_at_slot) != 1:
                    candidate_bases = []
                    break
                offset_flags_offset = vaddr_to_file(segments, base_slot + 8)
                offset_flags = struct.unpack_from("<q", data, offset_flags_offset)[0]
                target = targets_at_slot[0]
                candidate_bases.append(
                    BaseClassRecord(
                        class_name=typeinfo_to_class[target],
                        typeinfo_vaddr=target,
                        layout="multiple",
                        offset_flags=offset_flags,
                    )
                )
            for base in candidate_bases:
                bases[(base.typeinfo_vaddr, base.layout)] = base
        record.bases = sorted(
            bases.values(), key=lambda item: (item.layout, item.class_name, item.typeinfo_vaddr)
        )

    method_slot_matches = []
    for method in method_records:
        for function in method.functions:
            for rtti in rtti_records:
                for vtable in rtti.vtables:
                    for slot_index, target in enumerate(vtable.slots):
                        target_function = find_function(extents, starts, target)
                        target_start = target_function.start if target_function is not None else target
                        if target_start != function.start:
                            continue
                        method_slot_matches.append(
                            {
                                "method_names": method.method_names,
                                "function_vaddr": function.start,
                                "class_name": rtti.class_name,
                                "vtable_vaddr": vtable.base_vaddr,
                                "slot_index": slot_index,
                                "slot_target": target,
                            }
                        )

    referenced_paths = sum(
        bool(record.direct_xrefs or record.immediate_xrefs or record.rela_xrefs)
        for record in records
    )
    rtti_control = next(
        (record for record in rtti_records if record.class_name == "bdAuthPS4"),
        None,
    )
    names_by_function: dict[int, set[str]] = defaultdict(set)
    paths_by_function: dict[int, set[str]] = defaultdict(set)
    for record in bd_log_records:
        names_by_function[record.function_start].update(record.logged_function_names)
        paths_by_function[record.function_start].update(record.source_paths)
    bodies_by_name: dict[str, set[int]] = defaultdict(set)
    bodies_by_file_name: dict[tuple[str, str], set[int]] = defaultdict(set)
    for start, names in names_by_function.items():
        for name in names:
            bodies_by_name[name].add(start)
            for path in paths_by_function[start]:
                bodies_by_file_name[(path, name)].add(start)
    singly_named = {start: next(iter(names)) for start, names in names_by_function.items() if len(names) == 1}
    bd_log_control = next(
        (record for record in bd_log_records if record.call_vaddr == bd_log_control_call),
        None,
    )
    bd_log_control_ok = bool(
        bd_log_control
        and bd_log_control.logged_function_names == ["bdPlatformTrulyRandomInit"]
        and any(path.endswith("bdPlatformTrulyRandom-ps4.cpp") for path in bd_log_control.source_paths)
    )
    laundered_control = next(
        (record for record in bd_log_records if record.call_vaddr == 0xF695D6),
        None,
    )
    laundered_control_ok = bool(
        laundered_control
        and "handleGetAccessTokenReply" in laundered_control.logged_function_names
        and any(path.endswith("bdAuthPS4.cpp") for path in laundered_control.source_paths)
    )
    summary = {
        "sha256": hashlib.sha256(data).hexdigest(),
        "path_count": len(records),
        "referenced_path_count": referenced_paths,
        "rip_relative_xrefs": sum(len(record.direct_xrefs) for record in records),
        "immediate_xrefs": sum(len(record.immediate_xrefs) for record in records),
        "rela_xrefs": sum(len(record.rela_xrefs) for record in records),
        "rela_entries": len(relocations),
        "rela_types": {str(kind): count for kind, count in sorted(rela_types.items())},
        "unmapped_xrefs": sorted(set(unmapped_xrefs)),
        "fallback_function_count": len(
            {
                function.start
                for functions in functions_by_target.values()
                for function in functions.values()
                if function.evidence == "canonical_prologue"
            }
        ),
        "cross_owned_functions": {f"0x{start:X}": values for start, values in cross_owned.items()},
        "confirmed_function_count": len(owners_by_function),
        "functions_inside_brackets": sum(record.bracket_function_count for record in records),
        "foreign_confirmed_inside_brackets": sum(
            len(record.foreign_confirmed_functions) for record in records
        ),
        "string_order_matches_code_order": string_order_matches_code_order,
        "non_overlapping_brackets": non_overlapping_brackets,
        "component_path_counts": dict(sorted(component_path_counts.items())),
        "method_string_count": len(method_records),
        "unique_method_name_count": len(
            {name for record in method_records for name in record.method_names}
        ),
        "method_strings_with_xrefs": sum(bool(record.direct_xrefs) for record in method_records),
        "method_strings_with_source_attribution": sum(
            bool(record.source_attribution) for record in method_records
        ),
        "rtti_class_count": len(rtti_records),
        "rtti_classes_with_typeinfo": sum(bool(record.typeinfo_vaddrs) for record in rtti_records),
        "rtti_typeinfo_count": sum(len(record.typeinfo_vaddrs) for record in rtti_records),
        "rtti_vtable_count": sum(len(record.vtables) for record in rtti_records),
        "rtti_virtual_slot_count": sum(
            len(vtable.slots) for record in rtti_records for vtable in record.vtables
        ),
        "rtti_slots_with_source_attribution": sum(
            bool(sources)
            for record in rtti_records
            for vtable in record.vtables
            for sources in vtable.slot_sources
        ),
        "rtti_inheritance_edge_count": sum(len(record.bases) for record in rtti_records),
        "rtti_classes_with_bases": sum(bool(record.bases) for record in rtti_records),
        "method_virtual_slot_match_count": len(method_slot_matches),
        "bd_log_target": bd_log_target,
        "bd_log_call_count": len(bd_log_records),
        "bd_log_calls_with_function_name": sum(
            bool(record.logged_function_names) for record in bd_log_records
        ),
        "bd_log_calls_with_source_path": sum(bool(record.source_paths) for record in bd_log_records),
        "bd_log_unique_function_name_count": len(
            {name for record in bd_log_records for name in record.logged_function_names}
        ),
        "bd_log_uniquely_named_function_count": sum(
            len(names) == 1 for names in names_by_function.values()
        ),
        "bd_log_ambiguous_function_count": sum(
            len(names) > 1 for names in names_by_function.values()
        ),
        "bd_log_shared_bare_name_count": sum(len(starts) > 1 for starts in bodies_by_name.values()),
        "bd_log_single_name_bodies_with_shared_bare_name": sum(
            len(bodies_by_name[name]) > 1 for name in singly_named.values()
        ),
        "bd_log_shared_file_name_pair_count": sum(
            len(starts) > 1 for starts in bodies_by_file_name.values()
        ),
        "bd_log_single_name_bodies_with_shared_file_name_pair": sum(
            any(len(bodies_by_file_name[(path, name)]) > 1 for path in paths_by_function[start])
            for start, name in singly_named.items()
        ),
        "bd_log_positive_control": {
            "call_vaddr": bd_log_control_call,
            "expected_name": "bdPlatformTrulyRandomInit",
            "ok": bd_log_control_ok,
        },
        "bd_log_register_laundering_control": {
            "call_vaddr": 0xF695D6,
            "expected_name": "handleGetAccessTokenReply",
            "expected_file_suffix": "bdAuthPS4.cpp",
            "ok": laundered_control_ok,
        },
        "rtti_positive_control": {
            "class_name": "bdAuthPS4",
            "found": rtti_control is not None,
            "typeinfo_found": bool(rtti_control and rtti_control.typeinfo_vaddrs),
            "vtable_found": bool(rtti_control and rtti_control.vtables),
        },
        "tail_control": {
            "site": tail_control[0],
            "expected_target": tail_control[1],
            "actual": actual_control,
            "ok": control_ok,
        },
        "eh_frame": eh_meta,
    }
    if not control_ok:
        raise RuntimeError(f"tail-call positive control failed: {summary['tail_control']}")
    if referenced_paths != len(records):
        raise RuntimeError(
            f"reference positive control failed: {referenced_paths}/{len(records)} paths referenced"
        )
    if eh_meta["mismatched_starts"]:
        raise RuntimeError(f"eh_frame start validation failed: {eh_meta}")
    if unmapped_xrefs:
        raise RuntimeError(f"referenced instructions lack a function extent: {unmapped_xrefs}")
    if not summary["rtti_positive_control"]["typeinfo_found"]:
        raise RuntimeError(f"RTTI positive control failed: {summary['rtti_positive_control']}")
    if not bd_log_control_ok:
        raise RuntimeError(f"bdLog positive control failed: {summary['bd_log_positive_control']}")
    if not laundered_control_ok:
        raise RuntimeError(
            "bdLog register-laundering control failed: "
            f"{summary['bd_log_register_laundering_control']}"
        )
    return {
        "summary": summary,
        "files": records,
        "method_strings": method_records,
        "rtti": rtti_records,
        "method_slot_matches": method_slot_matches,
        "bd_log_calls": bd_log_records,
    }


def hex_address(value: int | None) -> str | None:
    return None if value is None else f"0x{value:X}"


def serializable(atlas: dict) -> dict:
    """Return JSON data with addresses already rendered in hexadecimal.

    Human conversion of decimal addresses is an observed project failure
    surface, so address-bearing fields intentionally do not serialize as JSON
    numbers.
    """
    summary = dict(atlas["summary"])
    summary["unmapped_xrefs"] = [hex_address(value) for value in summary["unmapped_xrefs"]]
    summary["cross_owned_functions"] = {
        key: [hex_address(value) for value in values]
        for key, values in summary["cross_owned_functions"].items()
    }
    control = dict(summary["tail_control"])
    control["site"] = hex_address(control["site"])
    control["expected_target"] = hex_address(control["expected_target"])
    if control["actual"] is not None:
        control["actual"] = [control["actual"][0], hex_address(control["actual"][1])]
    summary["tail_control"] = control
    summary["bd_log_target"] = hex_address(summary["bd_log_target"])
    bd_log_control = dict(summary["bd_log_positive_control"])
    bd_log_control["call_vaddr"] = hex_address(bd_log_control["call_vaddr"])
    summary["bd_log_positive_control"] = bd_log_control
    laundered_control = dict(summary["bd_log_register_laundering_control"])
    laundered_control["call_vaddr"] = hex_address(laundered_control["call_vaddr"])
    summary["bd_log_register_laundering_control"] = laundered_control
    eh = dict(summary["eh_frame"])
    eh["header_vaddr"] = hex_address(eh["header_vaddr"])
    eh["eh_frame_vaddr"] = hex_address(eh["eh_frame_vaddr"])
    summary["eh_frame"] = eh

    files = []
    for record in atlas["files"]:
        files.append(
            {
                "path": record.path,
                "string_file_offset": hex_address(record.string_file_offset),
                "string_vaddr": hex_address(record.string_vaddr),
                "direct_xrefs": [hex_address(value) for value in record.direct_xrefs],
                "immediate_xrefs": [hex_address(value) for value in record.immediate_xrefs],
                "rela_xrefs": [hex_address(value) for value in record.rela_xrefs],
                "confirmed_functions": [
                    {
                        "start": hex_address(function.start),
                        "end": hex_address(function.end),
                        "fde": hex_address(function.fde),
                        "evidence": function.evidence,
                    }
                    for function in record.confirmed_functions
                ],
                "bracket_start": hex_address(record.bracket_start),
                "bracket_end": hex_address(record.bracket_end),
                "bracket_function_count": record.bracket_function_count,
                "foreign_confirmed_functions": [
                    hex_address(value) for value in record.foreign_confirmed_functions
                ],
            }
        )
    method_strings = []
    for record in atlas["method_strings"]:
        method_strings.append(
            {
                "text": record.text,
                "string_file_offset": hex_address(record.string_file_offset),
                "string_vaddr": hex_address(record.string_vaddr),
                "method_names": record.method_names,
                "direct_xrefs": [hex_address(value) for value in record.direct_xrefs],
                "rela_xrefs": [hex_address(value) for value in record.rela_xrefs],
                "functions": [
                    {
                        "start": hex_address(function.start),
                        "end": hex_address(function.end),
                        "fde": hex_address(function.fde),
                        "evidence": function.evidence,
                    }
                    for function in record.functions
                ],
                "source_attribution": record.source_attribution,
            }
        )
    rtti = []
    for record in atlas["rtti"]:
        rtti.append(
            {
                "class_name": record.class_name,
                "encoded_name": record.encoded_name,
                "name_file_offset": hex_address(record.name_file_offset),
                "name_vaddr": hex_address(record.name_vaddr),
                "typeinfo_vaddrs": [hex_address(value) for value in record.typeinfo_vaddrs],
                "bases": [
                    {
                        "class_name": base.class_name,
                        "typeinfo_vaddr": hex_address(base.typeinfo_vaddr),
                        "layout": base.layout,
                        "offset_flags": (
                            None if base.offset_flags is None else f"0x{base.offset_flags & ((1 << 64) - 1):X}"
                        ),
                    }
                    for base in record.bases
                ],
                "vtables": [
                    {
                        "base_vaddr": hex_address(vtable.base_vaddr),
                        "typeinfo_vaddr": hex_address(vtable.typeinfo_vaddr),
                        "offset_to_top": vtable.offset_to_top,
                        "slots": [
                            {
                                "target": hex_address(target),
                                "source_attribution": sources,
                            }
                            for target, sources in zip(vtable.slots, vtable.slot_sources)
                        ],
                    }
                    for vtable in record.vtables
                ],
            }
        )
    method_slot_matches = [
        {
            "method_names": match["method_names"],
            "function_vaddr": hex_address(match["function_vaddr"]),
            "class_name": match["class_name"],
            "vtable_vaddr": hex_address(match["vtable_vaddr"]),
            "slot_index": match["slot_index"],
            "slot_target": hex_address(match["slot_target"]),
        }
        for match in atlas["method_slot_matches"]
    ]
    bd_log_calls = [
        {
            "call_vaddr": hex_address(record.call_vaddr),
            "function_start": hex_address(record.function_start),
            "function_end": hex_address(record.function_end),
            "logged_function_names": record.logged_function_names,
            "source_paths": record.source_paths,
            "source_lines": record.source_lines,
        }
        for record in atlas["bd_log_calls"]
    ]
    return {
        "summary": summary,
        "files": files,
        "method_strings": method_strings,
        "rtti": rtti,
        "method_slot_matches": method_slot_matches,
        "bd_log_calls": bd_log_calls,
    }


def short_path(path: str) -> str:
    marker = "DemonWare\\45\\"
    return path.split(marker, 1)[-1]


def render_markdown(atlas: dict) -> str:
    summary = atlas["summary"]
    out = io.StringIO()
    print("# Demonware source-file attribution atlas", file=out)
    print(file=out)
    print(f"- Eboot SHA-256: `{summary['sha256']}`", file=out)
    print(
        f"- Paths: {summary['referenced_path_count']}/{summary['path_count']} referenced; "
        f"RIP-relative xrefs: {summary['rip_relative_xrefs']}; "
        f"immediate xrefs: {summary['immediate_xrefs']}; RELA xrefs: {summary['rela_xrefs']}",
        file=out,
    )
    print(
        f"- RELA positive control: {summary['rela_entries']} entries, "
        f"types {summary['rela_types']}",
        file=out,
    )
    eh = summary["eh_frame"]
    print(
        f"- Unwind map: {eh['unique_extents']} exact extents from {eh['table_entries']} FDE entries; "
        f"start mismatches: {eh['mismatched_starts']}",
        file=out,
    )
    print(
        f"- Canonical-prologue fallbacks for referenced functions omitted from unwind data: "
        f"{summary['fallback_function_count']}; unmapped xrefs: {len(summary['unmapped_xrefs'])}",
        file=out,
    )
    print(
        f"- Confirmed functions: {summary['confirmed_function_count']}; functions inside inferred "
        f"brackets: {summary['functions_inside_brackets']}; foreign confirmed functions inside "
        f"another file bracket: {summary['foreign_confirmed_inside_brackets']}",
        file=out,
    )
    print(
        f"- String order equals code-bracket order: {summary['string_order_matches_code_order']}; "
        f"brackets are non-overlapping: {summary['non_overlapping_brackets']}",
        file=out,
    )
    print(f"- Source-path components: {summary['component_path_counts']}", file=out)
    print(
        f"- Explicit method evidence: {summary['unique_method_name_count']} unique spellings in "
        f"{summary['method_string_count']} strings; {summary['method_strings_with_xrefs']} strings "
        f"referenced; {summary['method_strings_with_source_attribution']} source-attributed",
        file=out,
    )
    print(
        f"- bdLog ABI recovery: {summary['bd_log_call_count']} calls; "
        f"{summary['bd_log_calls_with_function_name']} with function strings; "
        f"{summary['bd_log_unique_function_name_count']} unique logged names; "
        f"{summary['bd_log_uniquely_named_function_count']} bodies carrying exactly one name; "
        f"{summary['bd_log_ambiguous_function_count']} ambiguous functions",
        file=out,
    )
    print(
        f"- Reverse ambiguity: {summary['bd_log_shared_bare_name_count']} bare names occur in "
        f"multiple bodies; {summary['bd_log_single_name_bodies_with_shared_bare_name']} singly-named "
        f"bodies share their bare name; {summary['bd_log_shared_file_name_pair_count']} `(file,name)` "
        f"pairs occur in multiple bodies; {summary['bd_log_single_name_bodies_with_shared_file_name_pair']} "
        f"singly-named bodies share their `(file,name)` pair",
        file=out,
    )
    bd_log_control = summary["bd_log_positive_control"]
    print(
        f"- bdLog positive control: `0x{bd_log_control['call_vaddr']:X}` -> "
        f"`{bd_log_control['expected_name']}`: {'PASS' if bd_log_control['ok'] else 'FAIL'}",
        file=out,
    )
    laundering_control = summary["bd_log_register_laundering_control"]
    print(
        f"- Register-laundering control: `0x{laundering_control['call_vaddr']:X}` -> "
        f"`{laundering_control['expected_name']}` in `{laundering_control['expected_file_suffix']}`: "
        f"{'PASS' if laundering_control['ok'] else 'FAIL'}",
        file=out,
    )
    print(
        f"- Itanium RTTI: {summary['rtti_class_count']} length-valid candidates; "
        f"{summary['rtti_classes_with_typeinfo']} RELA-confirmed typeinfo objects; "
        f"{summary['rtti_vtable_count']} accepted vtables; "
        f"{summary['rtti_virtual_slot_count']} virtual slots; "
        f"{summary['rtti_slots_with_source_attribution']} source-attributed slots; "
        f"{summary['rtti_inheritance_edge_count']} inheritance edges; "
        f"{summary['method_virtual_slot_match_count']} exact method-slot anchors",
        file=out,
    )
    rtti_control = summary["rtti_positive_control"]
    print(
        f"- RTTI positive control {rtti_control['class_name']}: "
        f"typeinfo={rtti_control['typeinfo_found']}, vtable={rtti_control['vtable_found']}",
        file=out,
    )
    control = summary["tail_control"]
    print(
        f"- Tail-call control: `{control['site']:#x}` -> `{control['expected_target']:#x}`: "
        f"{'PASS' if control['ok'] else 'FAIL'}",
        file=out,
    )
    print(file=out)
    print(
        "A confirmed function contains a direct reference to that file string. The bracket is an "
        "inference bounded by the first and last confirmed function; unreferenced functions inside "
        "it are not promoted to confirmed ownership.",
        file=out,
    )
    print(file=out)
    print("| Source file | String VA | Xrefs | Confirmed functions | Inferred bracket | Functions in bracket | Foreign confirmed |", file=out)
    print("|---|---:|---:|---:|---:|---:|---:|", file=out)
    ordered = sorted(
        atlas["files"],
        key=lambda record: (record.bracket_start is None, record.bracket_start or record.string_vaddr),
    )
    for record in ordered:
        xrefs = len(record.direct_xrefs) + len(record.immediate_xrefs) + len(record.rela_xrefs)
        bracket = (
            f"`0x{record.bracket_start:X}..0x{record.bracket_end:X}`"
            if record.bracket_start is not None and record.bracket_end is not None
            else "-"
        )
        print(
            f"| `{short_path(record.path)}` | `0x{record.string_vaddr:X}` | {xrefs} | "
            f"{len(record.confirmed_functions)} | {bracket} | {record.bracket_function_count} | "
            f"{len(record.foreign_confirmed_functions)} |",
            file=out,
        )
    print(file=out)
    print("## Explicit method-name evidence", file=out)
    print(file=out)
    print("| Method spelling(s) | String VA | Referencing functions | Source attribution | Diagnostic |", file=out)
    print("|---|---:|---:|---|---|", file=out)
    for record in atlas["method_strings"]:
        functions = ", ".join(f"0x{function.start:X}" for function in record.functions) or "-"
        sources = ", ".join(
            f"{short_path(item['path'])} ({item['evidence']})"
            for item in record.source_attribution
        ) or "-"
        diagnostic = record.text.replace("|", "\\|").replace("\n", " ")
        print(
            f"| `{', '.join(record.method_names)}` | `0x{record.string_vaddr:X}` | `{functions}` | "
            f"{sources} | {diagnostic} |",
            file=out,
        )
    print(file=out)
    print("## Function names recovered from the bdLog ABI", file=out)
    print(file=out)
    print(
        "D1 independently establishes the logger parameter order at 0xF78CA8..0xF78D45: rcx is "
        "the __FILE__ path, r8 is the compiler __FUNCTION__ string, r9d is the source line, and the "
        "first stack argument is the format string. No external source is needed for this ABI claim.",
        file=out,
    )
    print(
        "Names are published only as `(source file, logged name, machine-function extent)` triples. "
        "The unqualified `__FUNCTION__` spelling is not a globally unique symbol.",
        file=out,
    )
    print(file=out)
    print("| Function extent | Logged name(s) | Source file(s) | Line(s) | Callsite(s) |", file=out)
    print("|---:|---|---|---:|---:|", file=out)
    grouped_logs = {}
    for record in atlas["bd_log_calls"]:
        key = (record.function_start, record.function_end)
        group = grouped_logs.setdefault(
            key, {"names": set(), "paths": set(), "lines": set(), "calls": set()}
        )
        group["names"].update(record.logged_function_names)
        group["paths"].update(record.source_paths)
        group["lines"].update(record.source_lines)
        group["calls"].add(record.call_vaddr)
    for (start, end), group in sorted(grouped_logs.items()):
        if not group["names"]:
            continue
        names = ", ".join(f"`{name}`" for name in sorted(group["names"]))
        paths = ", ".join(f"`{short_path(path)}`" for path in sorted(group["paths"])) or "-"
        lines = ", ".join(str(line) for line in sorted(group["lines"])) or "-"
        calls = ", ".join(f"`0x{call:X}`" for call in sorted(group["calls"]))
        print(
            f"| `0x{start:X}..0x{end:X}` | {names} | {paths} | {lines} | {calls} |",
            file=out,
        )
    print(file=out)
    print("## Itanium RTTI and vtables", file=out)
    print(file=out)
    print("| Class | Base class(es) | Name VA | Typeinfo | Vtable (slot count) | Slot source files |", file=out)
    print("|---|---|---:|---:|---|---|", file=out)
    for record in atlas["rtti"]:
        bases = ", ".join(
            f"{base.class_name} ({base.layout})" for base in record.bases
        ) or "-"
        typeinfos = ", ".join(f"0x{value:X}" for value in record.typeinfo_vaddrs) or "-"
        vtables = ", ".join(
            f"0x{vtable.base_vaddr:X} ({len(vtable.slots)})" for vtable in record.vtables
        ) or "-"
        source_paths = sorted(
            {
                short_path(item["path"])
                for vtable in record.vtables
                for sources in vtable.slot_sources
                for item in sources
            }
        )
        sources = ", ".join(source_paths) or "-"
        print(
            f"| `{record.class_name}` | {bases} | `0x{record.name_vaddr:X}` | `{typeinfos}` | "
            f"{vtables} | {sources} |",
            file=out,
        )
    print(file=out)
    print("## Explicit-method to virtual-slot matches", file=out)
    print(file=out)
    if not atlas["method_slot_matches"]:
        print("No exact method-bearing function matched a recovered virtual slot target.", file=out)
    else:
        print("| Method | Function | Class | Vtable | Slot (zero-based) | Target |", file=out)
        print("|---|---:|---|---:|---:|---:|", file=out)
        for match in atlas["method_slot_matches"]:
            print(
                f"| `{', '.join(match['method_names'])}` | `0x{match['function_vaddr']:X}` | "
                f"`{match['class_name']}` | `0x{match['vtable_vaddr']:X}` | "
                f"{match['slot_index']} | `0x{match['slot_target']:X}` |",
                file=out,
            )
    return out.getvalue()


def render_csv(atlas: dict) -> str:
    out = io.StringIO()
    writer = csv.writer(out)
    writer.writerow(
        [
            "source_file",
            "string_vaddr",
            "xref_count",
            "confirmed_function_count",
            "bracket_start",
            "bracket_end",
            "bracket_function_count",
            "foreign_confirmed_count",
        ]
    )
    for record in atlas["files"]:
        writer.writerow(
            [
                record.path,
                f"0x{record.string_vaddr:X}",
                len(record.direct_xrefs) + len(record.immediate_xrefs) + len(record.rela_xrefs),
                len(record.confirmed_functions),
                "" if record.bracket_start is None else f"0x{record.bracket_start:X}",
                "" if record.bracket_end is None else f"0x{record.bracket_end:X}",
                record.bracket_function_count,
                len(record.foreign_confirmed_functions),
            ]
        )
    return out.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("eboot", type=Path)
    parser.add_argument("--format", choices=("markdown", "json", "csv"), default="markdown")
    parser.add_argument("--tail-control-site", type=parse_int, default=0x560D27)
    parser.add_argument("--tail-control-target", type=parse_int, default=0x56FFB0)
    parser.add_argument("--bd-log-target", type=parse_int, default=0xF78C90)
    parser.add_argument("--bd-log-control-call", type=parse_int, default=0x10421F2)
    args = parser.parse_args()

    atlas = build_atlas(
        args.eboot.read_bytes(),
        (args.tail_control_site, args.tail_control_target),
        args.bd_log_target,
        args.bd_log_control_call,
    )
    if args.format == "json":
        print(json.dumps(serializable(atlas), indent=2, sort_keys=True))
    elif args.format == "csv":
        print(render_csv(atlas), end="")
    else:
        print(render_markdown(atlas), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
