"""Offline reader for Destiny 1 Tiger packages (.pkg, format version 24) as shipped on the PS3
disc (big-endian, platform 4). Reads package headers, file entries, block tables and the named
tag table without the game running; extracts entries when their blocks are stored uncompressed
or when a user-supplied Oodle 2 runtime is given (--oodle, `oo2core_3_win64.dll`; never commit or
download one).

Layout (tiger-pkg `d1_legacy`, cross-checked against a PS4 v24 package study in
github.com/lizardpeter/destiny-1 spec/D1_TIGER_PACKAGE_v24.md; PS3 byte order and the flag bit
from tiger-pkg; the package id at +4 and the u64 at +8 match what the client itself checks,
notes/ps3-route.md 021-022):

  header  +0x00 u16 version (24)  +0x02 u16 platform (4 = PS3)  +0x04 u16 package id
          +0x08 u64 registration value  +0x10 u64 build time  +0x20 u16 patch id
          +0x22 u16 language  +0x24 char[128] tool string  +0xb0 u32 signature offset
          +0xb4/+0xb8/+0xbc entry table count, offset, SHA-1
          +0xd0/+0xd4/+0xd8 block table count, offset, SHA-1
          +0xec/+0xf0/+0xf4 named tag table count, offset, SHA-1   +0x13c u32 file size
  entry   16 bytes: u32 reference, u32 type word, u64 block info
          (start block = bits 0-13, start offset = bits 14-27 * 16, size = bits 28-57)
  block   32 bytes: u32 offset, u32 stored size, u16 patch id, u16 flags, SHA-1 of stored bytes;
          blocks are 0x40000 bytes once decompressed; a block with a different patch id lives in
          <name>_<patch>.pkg next to this one
  named   68 bytes: u32 tag hash, u32 class hash, char[60] name

Tag hashes: 0x80800000 + (package id << 13) + entry index for ids below 0x400; ids up to 0xbff
move to the next banks (bits 23-24 hold 1 + id // 0x400).

Usage:
  python scripts/d1_pkg.py info PKG
  python scripts/d1_pkg.py entries PKG [--type T] [--subtype S]
  python scripts/d1_pkg.py blocks PKG [--verify]
  python scripts/d1_pkg.py named [DIR|PKG ...] [--grep TEXT]   (default DIR: the disc packages)
  python scripts/d1_pkg.py locate TAGHASH [DIR]                (which package holds a tag)
  python scripts/d1_pkg.py extract PKG INDEX|TAGHASH OUT [--oodle DLL]
  python scripts/d1_pkg.py scan [DIR] --find TEXT|--hex HEX [--oodle DLL]  (search entry data)
"""

import argparse
import ctypes
import hashlib
import struct
import sys
from pathlib import Path

BLOCK_SIZE = 0x40000
PLATFORMS = ["tool32", "win32", "win64", "x360", "ps3", "tool64", "win64v1", "ps4", "xboxone"]
LANGUAGES = ["none", "en", "fr", "it", "de", "es", "ja", "pt", "ru", "pl", "zh-hans", "zh-hant",
             "es-mx", "ko"]
FLAG_COMPRESSED = 0x100  # tiger-pkg d1_legacy (big-endian builds)
# Oodle LZ block header byte 1 (low 7 bits) -> codec, as decoded by the open-source ooz
# (github.com/powzix/ooz); ids outside this table are older codecs ooz cannot read.
OODLE_DECODERS = {5: "LZNA", 6: "Kraken", 10: "Mermaid/Selkie", 11: "BitKnit", 12: "Leviathan"}


def default_dir():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
    from local_paths import LOCAL
    return LOCAL / "01_WORKING" / "ps3_destiny" / "PS3_GAME" / "USRDIR" / "packages"


def tag_hash(pkg_id, index):
    if pkg_id >= 0xc00:
        raise ValueError(f"package id 0x{pkg_id:x} has no tag hash (banks 1-3 cover 0-0xbff)")
    return 0x80000000 | ((pkg_id // 0x400 + 1) << 23) | ((pkg_id % 0x400) << 13) | (index & 0x1fff)


def split_tag_hash(value):
    """(package id, entry index) of a tag hash, or None for a value outside the tag namespace."""
    if not value & 0x80000000 or not (value >> 23) & 3:
        return None
    return ((value >> 13) & 0x3ff) + (((value >> 23) & 3) - 1) * 0x400, value & 0x1fff


def parse_int(text):
    """0x-prefixed or 8-digit values are hex (tag hashes), short digit strings decimal (indices)."""
    if text.lower().startswith("0x") or len(text) == 8 or not text.isdigit():
        return int(text, 16)
    return int(text)


class Package:
    def __init__(self, path):
        self.path = Path(path)
        self.data = self.path.read_bytes()
        d = self.data
        (self.version, self.platform, self.pkg_id, self.unk6, self.reg_value, self.build_time,
         self.build_id, self.unk1c, self.patch_id, self.language) = struct.unpack_from(">HHHHQQIIHH", d, 0)
        if self.version != 24:
            raise ValueError(f"{self.path.name}: package version {self.version}, expected 24")
        self.tool = d[0x24:0xa4].split(b"\0")[0].decode("latin-1")
        self.signature_offset = struct.unpack_from(">I", d, 0xb0)[0]
        self.tables = {}
        for name, off in (("entries", 0xb4), ("blocks", 0xd0), ("named", 0xec)):
            count, offset = struct.unpack_from(">II", d, off)
            self.tables[name] = (count, offset, d[off + 8:off + 28])
        self.file_size = struct.unpack_from(">I", d, 0x13c)[0]

        count, offset, _ = self.tables["entries"]
        self.entries = []
        for i in range(count):
            ref, word, info = struct.unpack_from(">IIQ", d, offset + 16 * i)
            self.entries.append({
                "index": i, "reference": ref, "word": word,
                "type": (word >> 16) & 0xff, "subtype": word & 0xff,
                "block": info & 0x3fff, "offset": ((info >> 14) & 0x3fff) << 4,
                "size": (info >> 28) & 0x3fffffff})
        count, offset, _ = self.tables["blocks"]
        self.blocks = []
        for i in range(count):
            off, size, patch, flags = struct.unpack_from(">IIHH", d, offset + 32 * i)
            self.blocks.append({"index": i, "offset": off, "size": size, "patch": patch,
                                "flags": flags, "sha1": d[offset + 32 * i + 12:offset + 32 * i + 32]})
        count, offset, _ = self.tables["named"]
        self.named = []
        for i in range(count):
            h, cls = struct.unpack_from(">II", d, offset + 68 * i)
            name = d[offset + 68 * i + 8:offset + 68 * i + 68].split(b"\0")[0].decode("latin-1")
            self.named.append({"hash": h, "class": cls, "name": name})

    def table_hash_ok(self, name):
        count, offset, stored = self.tables[name]
        size = {"entries": 16, "blocks": 32, "named": 68}[name] * count
        if not count:
            return stored == bytes(20) or stored == hashlib.sha1(b"").digest()
        return hashlib.sha1(self.data[offset:offset + size]).digest() == stored

    def stored_block(self, block):
        if block["patch"] == self.patch_id:
            return self.data[block["offset"]:block["offset"] + block["size"]]
        stem = self.path.name.rsplit("_", 1)[0]
        other = self.path.with_name(f"{stem}_{block['patch']}.pkg")
        with open(other, "rb") as f:
            f.seek(block["offset"])
            return f.read(block["size"])

    def block_data(self, index, oodle=None):
        block = self.blocks[index]
        raw = self.stored_block(block)
        if not block["flags"] & FLAG_COMPRESSED:
            return raw
        if oodle is None:
            raise RuntimeError(f"block {index} is compressed ({oodle_codec(raw)}); pass --oodle")
        return oodle.decompress(raw, BLOCK_SIZE)

    def entry_data(self, index, oodle=None):
        e = self.entries[index]
        out = bytearray()
        block, skip = e["block"], e["offset"]
        while len(out) < e["size"]:
            chunk = self.block_data(block, oodle)[skip:]
            out += chunk[:e["size"] - len(out)]
            block, skip = block + 1, 0
        return bytes(out)


def oodle_codec(raw):
    if len(raw) < 2 or raw[0] & 0x0f != 0x0c:
        return f"no Oodle LZ header ({raw[:2].hex()})"
    kind = raw[1] & 0x7f
    return f"Oodle decoder {kind} ({OODLE_DECODERS.get(kind, 'not in ooz')})"


class Oodle:
    """OodleLZ_Decompress from a user-supplied oo2core runtime (Windows DLL, or a Linux shim)."""

    def __init__(self, path):
        self.lib = ctypes.CDLL(str(path))
        f = self.lib.OodleLZ_Decompress
        f.restype = ctypes.c_int64
        f.argtypes = [ctypes.c_char_p, ctypes.c_int64, ctypes.c_char_p, ctypes.c_int64,
                      ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p,
                      ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
        self.fn = f

    def decompress(self, raw, size):
        out = ctypes.create_string_buffer(size)
        # fuzz-safe, no CRC check, minimal verbosity, all thread phases (as tiger-pkg does)
        n = self.fn(raw, len(raw), out, size, 1, 0, 1, None, None, None, None, None, None, 3)
        if n <= 0:
            raise RuntimeError(f"OodleLZ_Decompress failed ({n}) on a {len(raw)}-byte block")
        return out.raw[:n]


def packages(args_paths):
    paths = [Path(p) for p in args_paths] or [default_dir()]
    for p in paths:
        if p.is_dir():
            yield from sorted(p.glob("*.pkg"))
        else:
            yield p


def cmd_info(a):
    p = Package(a.pkg)
    plat = PLATFORMS[p.platform] if p.platform < len(PLATFORMS) else p.platform
    lang = LANGUAGES[p.language] if p.language < len(LANGUAGES) else p.language
    print(f"{p.path.name}: version {p.version}, platform {plat}, package id 0x{p.pkg_id:04x}, "
          f"patch {p.patch_id}, language {lang}")
    print(f"registration value 0x{p.reg_value:016x}, build time 0x{p.build_time:016x}, "
          f"build id 0x{p.build_id:08x}")
    print(f"tool: {p.tool}")
    for name, (count, offset, _) in p.tables.items():
        print(f"{name}: {count} at 0x{offset:x}, SHA-1 {'ok' if p.table_hash_ok(name) else 'MISMATCH'}")
    print(f"file size field {p.file_size}, actual {len(p.data)}")
    flags = {}
    for b in p.blocks:
        flags[b["flags"]] = flags.get(b["flags"], 0) + 1
    print("block flags: " + ", ".join(f"0x{k:04x} x{v}" for k, v in sorted(flags.items())))
    first = next((b for b in p.blocks if b["flags"] & FLAG_COMPRESSED and b["patch"] == p.patch_id), None)
    if first:
        print(f"first compressed block: {oodle_codec(p.stored_block(first))}")
    types = {}
    for e in p.entries:
        types[(e["type"], e["subtype"])] = types.get((e["type"], e["subtype"]), 0) + 1
    print("entry type:subtype: " + ", ".join(f"{t}:{s} x{n}" for (t, s), n in sorted(types.items())))


def cmd_entries(a):
    p = Package(a.pkg)
    for e in p.entries:
        if (a.type is None or e["type"] == a.type) and (a.subtype is None or e["subtype"] == a.subtype):
            print(f"{e['index']:5d} 0x{tag_hash(p.pkg_id, e['index']):08x} ref 0x{e['reference']:08x} "
                  f"word 0x{e['word']:08x} type {e['type']}:{e['subtype']} block {e['block']}"
                  f"+0x{e['offset']:x} size {e['size']}")


def cmd_blocks(a):
    p = Package(a.pkg)
    bad = 0
    for b in p.blocks:
        line = (f"{b['index']:5d} off 0x{b['offset']:08x} size {b['size']:7d} patch {b['patch']} "
                f"flags 0x{b['flags']:04x}")
        if a.verify:
            try:
                ok = hashlib.sha1(p.stored_block(b)).digest() == b["sha1"]
            except OSError as exc:
                ok, line = False, line + f" ({exc.strerror})"
            bad += not ok
            line += " sha1 ok" if ok else " sha1 MISMATCH"
        print(line)
    if a.verify:
        print(f"{len(p.blocks) - bad}/{len(p.blocks)} blocks match their SHA-1")


def cmd_named(a):
    needle = a.grep.lower() if a.grep else None
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error) as exc:
            print(f"# {path.name}: {exc}", file=sys.stderr)
            continue
        for n in p.named:
            if needle is None or needle in n["name"].lower():
                print(f"{path.name} 0x{n['hash']:08x} class 0x{n['class']:08x} {n['name']}")


def cmd_locate(a):
    value = parse_int(a.hash)
    where = split_tag_hash(value)
    if where is None:
        sys.exit(f"0x{value:08x} is not a package tag hash")
    pkg_id, index = where
    print(f"0x{value:08x} = package 0x{pkg_id:04x}, entry {index}")
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error):
            continue
        if p.pkg_id == pkg_id and index < len(p.entries):
            e = p.entries[index]
            print(f"  {path.name}: type {e['type']}:{e['subtype']}, size {e['size']}, "
                  f"reference 0x{e['reference']:08x}")


def resolve_entry(p, text):
    value = parse_int(text)
    if value < len(p.entries):
        return value
    where = split_tag_hash(value)
    if not where or where[0] != p.pkg_id:
        sys.exit(f"{text} is neither an entry index nor a tag of package 0x{p.pkg_id:04x}")
    return where[1]


def cmd_extract(a):
    p = Package(a.pkg)
    oodle = Oodle(a.oodle) if a.oodle else None
    data = p.entry_data(resolve_entry(p, a.entry), oodle)
    Path(a.out).write_bytes(data)
    print(f"wrote {len(data)} bytes to {a.out}")


def cmd_scan(a):
    needle = bytes.fromhex(a.hex) if a.hex else a.find.encode()
    oodle = Oodle(a.oodle) if a.oodle else None
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error):
            continue
        for e in p.entries:
            try:
                data = p.entry_data(e["index"], oodle)
            except (RuntimeError, OSError):
                continue
            pos = data.find(needle)
            while pos >= 0:
                ctx = data[max(0, pos - 16):pos + len(needle) + 16]
                print(f"{path.name} entry {e['index']} (0x{tag_hash(p.pkg_id, e['index']):08x}, "
                      f"type {e['type']}:{e['subtype']}) +0x{pos:x}: {ctx.hex()}")
                pos = data.find(needle, pos + 1)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("info"); s.add_argument("pkg"); s.set_defaults(fn=cmd_info)
    s = sub.add_parser("entries"); s.add_argument("pkg")
    s.add_argument("--type", type=int); s.add_argument("--subtype", type=int); s.set_defaults(fn=cmd_entries)
    s = sub.add_parser("blocks"); s.add_argument("pkg"); s.add_argument("--verify", action="store_true")
    s.set_defaults(fn=cmd_blocks)
    s = sub.add_parser("named"); s.add_argument("paths", nargs="*"); s.add_argument("--grep")
    s.set_defaults(fn=cmd_named)
    s = sub.add_parser("locate"); s.add_argument("hash"); s.add_argument("paths", nargs="*")
    s.set_defaults(fn=cmd_locate)
    s = sub.add_parser("extract"); s.add_argument("pkg"); s.add_argument("entry"); s.add_argument("out")
    s.add_argument("--oodle"); s.set_defaults(fn=cmd_extract)
    s = sub.add_parser("scan"); s.add_argument("paths", nargs="*")
    g = s.add_mutually_exclusive_group(required=True); g.add_argument("--find"); g.add_argument("--hex")
    s.add_argument("--oodle"); s.set_defaults(fn=cmd_scan)
    a = ap.parse_args(argv)
    a.fn(a)


if __name__ == "__main__":
    main()
