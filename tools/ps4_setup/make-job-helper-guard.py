#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build the 56578 null-worker guard as a shadPS4 patch XML (synthetic client modification).

0xFAAC0 (run one pending job on the current worker) reads the thread's job-worker pointer from
TLS (+0x58) and dereferences it. Only job-worker threads set it; the Graphics Heartbeat thread
calls it while its frame counter lags and faults at 0xFAAF4. The guard returns the function's own
"no job run" result (ebx=2 -> al=0 at 0xFACF6) when the pointer is null; the caller then takes
its existing 300 us sleep path. The non-null path is byte-for-byte the original.

Usage: make-job-helper-guard.py <eboot.elf> <out.xml> [--unsigned-manifest] [--diagnostics]
       (refuses any other image)

--unsigned-manifest adds the 56578 equivalent of the 01.29 lab byte (0x18c4974 there): the
cdn_content_manifest handler 0x2B0980 reads one flag at 0x18DC974 (shipped 0x01, its only
reference is 0x2B09F0: `cmp byte [rax],0; je 0x2B0AF2`). 0 skips the RSA envelope and parses
the body directly, so our unsigned manifest is accepted. Nothing is forged; it is a lab-only
client modification and everything past it is synthetic_ancestor.

--diagnostics adds the 56578 mapping of the 01.29 guest-diagnostics recipe: retail_log_emit
0xE4AA0 (same 1124 x 0x150 ring at +0x19860 and 0x140 memcpy as 01.29's 0xE4AD0) tail-calls
printf("%s<newline>", text) via the printf stub 0x12AAB90 and the "%s<newline>" string at
0x16136A0, and the log check
0xE4D70 (threshold[system] <= level; 1,636 call sites) becomes a fixed threshold of 1. Output
only: no check is altered.
"""
import hashlib
import struct
import sys

ELF_SHA = "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833"
BIAS = 0x4000            # file offset - vaddr for the text LOAD segment
SHADPS4 = 0x400000       # shadPS4 XML: Address = vaddr + 0x400000
HOOK, CAVE = 0xFAAE7, 0x290992
GETTER, RESUME, EXIT = 0xFC520, 0xFAAF4, 0xFACF6
SIGNED_FLAG, SIGNED_FLAG_FILE = 0x18DC974, 0x18DC974 - 0x18D8000 + 0x18DC000
EMIT, PRINTF, FMT_S_NL, CHECK = 0xE4AA0, 0x12AAB90, 0x16136A0, 0xE4D70
EMIT_ORIG = bytes.fromhex("554889e5415653" "83ffff0f84ac000000")
CHECK_ORIG = bytes.fromhex("488d05392e4a02")
HOOK_ORIG = bytes.fromhex("e8341a000048c745b0ffffffff")
CAVE_ORIG = bytes.fromhex("66666666662e0f1f840000000000" "90" "6666666666662e0f1f840000000000"
                          "90" "6666666666662e0f1f840000000000")


def rel(src_next, dst):
    return struct.pack("<i", dst - src_next)


def build():
    c = bytearray()
    c += b"\xe8" + rel(CAVE + 5, GETTER)              # call worker getter
    c += b"\x48\x85\xc0"                              # test rax, rax
    c += b"\x74\x0d"                                  # jz none
    c += b"\x48\xc7\x45\xb0\xff\xff\xff\xff"          # mov qword [rbp-0x50], -1 (original)
    c += b"\xe9" + rel(CAVE + len(c) + 5, RESUME)     # jmp back to mov rsi,[rax+8]
    assert len(c) == 0x17
    c += b"\xbb\x02\x00\x00\x00"                      # none: mov ebx, 2
    c += b"\xe9" + rel(CAVE + len(c) + 5, EXIT)       # jmp to the function's own exit
    hook = b"\xe9" + rel(HOOK + 5, CAVE) + b"\x90" * 8
    assert len(hook) == len(HOOK_ORIG) and len(c) <= len(CAVE_ORIG)
    return bytes(hook), bytes(c)


def main(elf, out, *flags):
    data = open(elf, "rb").read()
    if hashlib.sha256(data).hexdigest() != ELF_SHA:
        raise SystemExit("REFUSING: not the build 56578 ELF")
    if data[HOOK + BIAS:HOOK + BIAS + len(HOOK_ORIG)] != HOOK_ORIG:
        raise SystemExit("REFUSING: hook bytes differ")
    if data[CAVE + BIAS:CAVE + BIAS + len(CAVE_ORIG)] != CAVE_ORIG:
        raise SystemExit("REFUSING: cave is not the expected padding")
    hook, cave = build()
    unsigned = "--unsigned-manifest" in flags
    diagnostics = "--diagnostics" in flags
    diag_entry = ""
    if diagnostics:
        if data[EMIT + BIAS:EMIT + BIAS + len(EMIT_ORIG)] != EMIT_ORIG:
            raise SystemExit("REFUSING: retail_log_emit bytes differ")
        if data[CHECK + BIAS:CHECK + BIAS + len(CHECK_ORIG)] != CHECK_ORIG:
            raise SystemExit("REFUSING: retail_log_check bytes differ")
        if data[FMT_S_NL + BIAS:FMT_S_NL + BIAS + 4] != b"%s\n\x00":
            raise SystemExit("REFUSING: format string differs")
        sink = (b"\x48\x8d\x3d" + rel(EMIT + 7, FMT_S_NL) + b"\x30\xc0"
                + b"\xe9" + rel(EMIT + 14, PRINTF))
        threshold = bytes.fromhex("83fe010f9dc0c3")
        diag_entry = f"""  <Metadata Name="D1 56578 diagnostics: retail_log sink to stdout, threshold T=1"
            isEnabled="true" AppVer="01.00" AppElf="eboot.bin" Author="d1reimagined" PatchVer="1.0">
    <PatchList>
      <Line Type="bytes" Address="{EMIT + SHADPS4:X}" Value="{sink.hex().upper()}"/>
      <Line Type="bytes" Address="{CHECK + SHADPS4:X}" Value="{threshold.hex().upper()}"/>
    </PatchList>
  </Metadata>
"""
    if unsigned and data[SIGNED_FLAG_FILE] != 1:
        raise SystemExit("REFUSING: manifest-signature flag is not the shipped 0x01")
    manifest_entry = f"""  <Metadata Name="D1 56578 lab: accept UNSIGNED cdn content manifest (synthetic)"
            isEnabled="true" AppVer="01.00" AppElf="eboot.bin" Author="d1reimagined" PatchVer="1.0">
    <PatchList>
      <Line Type="bytes" Address="{SIGNED_FLAG + SHADPS4:X}" Value="00"/>
    </PatchList>
  </Metadata>
""" if unsigned else ""
    # shadPS4 0.19 matches AppVer against param.sfo APP_VER (the repack's 01.00) and silently skips
    # an entry whose AppElf is not the loaded executable's name.
    entries = "".join(f"""  <Metadata Name="D1 56578: null job-worker guard (synthetic)"
            isEnabled="true" AppVer="{ver}" AppElf="eboot.bin" Author="d1reimagined" PatchVer="1.0">
    <PatchList>
      <Line Type="bytes" Address="{CAVE + SHADPS4:X}" Value="{cave.hex().upper()}"/>
      <Line Type="bytes" Address="{HOOK + SHADPS4:X}" Value="{hook.hex().upper()}"/>
    </PatchList>
  </Metadata>
""" for ver in ("01.00",))
    xml = f"""<?xml version="1.0" encoding="utf-8"?>
<!--
  Destiny 1 PS4 CUSA00219, executable build 56578.17.04.12 (ELF sha256 {ELF_SHA[:16]}...).
  SYNTHETIC CLIENT MODIFICATION - anything reached through it is synthetic_ancestor.
  Null job-worker guard for 0xFAAC0 (fault at 0xFAAF4 in Graphics Heartbeat). Generated by
  make-job-helper-guard.py, which verifies the original bytes. AppVer is the repack's APP_VER.
-->
<Patch>
{entries}{manifest_entry}{diag_entry}</Patch>
"""
    with open(out, "x", encoding="utf-8") as f:
        f.write(xml)
    print(f"cave {cave.hex()}\nhook {hook.hex()}\nwrote {out}")


if __name__ == "__main__":
    main(*sys.argv[1:])
