"""Calculate RPCS3's PPU executable patch key from a decrypted PS3 ELF64.

Matches ppu_load_exec in RPCS3's Emu/Cell/PPUModule.cpp: hash every program
header's big-endian type and flags, then each nonempty LOAD segment's virtual
address, memory size and file bytes. This is the pre-patch executable hash.
"""

import argparse
import hashlib
import struct
from pathlib import Path


def ppu_hash(path: Path) -> str:
    data = path.read_bytes()
    if data[:6] != b"\x7fELF\x02\x02":
        raise ValueError(f"{path}: expected a decrypted big-endian ELF64")
    phoff = struct.unpack_from(">Q", data, 0x20)[0]
    phentsize, phnum = struct.unpack_from(">HH", data, 0x36)
    if phentsize < 56 or phoff + phentsize * phnum > len(data):
        raise ValueError(f"{path}: invalid program-header table")
    sha = hashlib.sha1()
    for index in range(phnum):
        off = phoff + index * phentsize
        typ, _, file_off, _, _, file_size, mem_size, _ = struct.unpack_from(">IIQQQQQQ", data, off)
        sha.update(data[off:off + 8])
        if typ == 1 and mem_size:
            if file_size > mem_size or file_off + file_size > len(data):
                raise ValueError(f"{path}: invalid LOAD segment {index}")
            sha.update(data[off + 16:off + 24])
            sha.update(data[off + 40:off + 48])
            sha.update(data[file_off:file_off + file_size])
    return "PPU-" + sha.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", type=Path, help="decrypted EBOOT.elf")
    args = parser.parse_args()
    print(ppu_hash(args.elf))


if __name__ == "__main__":
    main()
