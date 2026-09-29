"""Destiny PS3 sign-on messages (POST /SignOn), as read from the game's nanopb descriptors.

The client (BLES01857 01.00) serialises with nanopb 0.1.x. The pb_field_t tables are
packed 19-byte records (tag u32, type u8, data_offset u8, size_offset s8, data_size u32,
array_size u32, ptr u32). The request descriptor is at EBOOT 0x011ca1c0, the response
descriptor at 0x011ca300. Field names are ours; only numbers, wire types and size limits
come from the tables. The response handler is at 0x126744 and the success-payload copy
at 0x12655c.

Response (HTTP 2xx, application/octet-stream):
  1  result   int32, required. 0 success (field 2), 1 queued (field 3),
              2 error (field 7), 3 user_action_required. Anything else is ignored.
  2  success  message:
       1-3  bytes[16] each, required. 2 and 3 are copied as two 16-byte session keys.
       4    bytes[<=512], required. Token; the game allocates a copy, and an empty
            token fails the sign-on.
       5    uint64, required. Expiry, absolute seconds (sys_time).
       6    uint32, required. IPv4 address, stored byte-swapped.
       7    uint32, required. Port (low 16 bits).
       9    message {1,2,3: fixed32 float}, optional.
       11   uint64, optional. Expiry relative to now, in seconds; overrides 5.
  3  queued   message: 1-3 int32, 4 bytes[256], 5 bool (all required); 6 int32, 7 bytes[256].
  4  message: 1 uint64 (-> +0x4460), 2 uint32 (-> +0x4468), required; 3 bytes, decoded by
     the callback at 0x126fec/0x1214e8 as the title configuration (descriptor 0x11c7677,
     58 submessages). Its field 1 = {1 bytes[4096], 2 string[256] content manifest URL,
     3 string[37] manifest GUID, 4 string[128]}; the bootflow (0x32bb40) reads the URL via
     0xb6ca90. The decoder skips required-field checks for submessages and for fields
     with defaults, so every other configuration field may be omitted.
  5  uint64, optional. Shown as silo_id by the debug routine at 0xb6ba4c.
  6  string[64], optional. signon_server_reported_content_version.
  7  error    message: 1 code int32 required (1 suspended, 2-3 denied, 4 prelaunch,
              5 platform, 6 configuration, 7 commercialization, 8 version mismatch,
              other denied); 2 int32; 3 uint64; 4 string[200]; 5 string[50].
  8  int32, optional.   10 message {1: repeated int32 [64]}.   12 uint64, optional.

The title updates (01.24; notes/final-build.md) add optional response fields 13 (string[64])
and 14 (u32). Field 13 is a serialized blob stored as {u64 pointer, u32 length} at sign-on
singleton +0x4460/+0x4468 (not +0x47cc, an earlier reading). Its only consumer parses a
0x178-byte record once per region and copes with an empty field (length 0 -> defaults), so
omitting it is safe; its contents are still undecoded. Field 14 goes as an IPv4 address record
to the network_demonware module (+0x184, 0x132cb4), which compares it with its own address
(+0x154) at 0x132cf4: LIKELY the client's public address as the server saw it. The byte order is
not confirmed; `public_ip` sends it as a big-endian u32. Runs 007-009 sent 127.0.0.1 and then
stalled before BAP, so treat it as a suspect and A/B it with `Start-FinalBuildSmoke.ps1
-PublicIp ''`. The request adds optional fields 20 (8 integers) and 21 (five strings). The
success message (field 2) is the same.

HTTP 500 maps to _internal_server_error, 503/509 to _server_unavailable, and any other
non-2xx status to _retriable_error.
"""

import socket
import struct

VARINT, FIXED64, BYTES, FIXED32 = 0, 1, 2, 5

# Fixed keys in the local SignOn success reply (fields 2.1-2.3). The client uses key 2 as
# the AES-128 key and key 3 as the HMAC-SHA256 key for the BAP shello reply (bap_server.py).
TEST_SESSION_KEYS = (b"d1xenia-key-0001", b"d1xenia-key-0002", b"d1xenia-key-0003")


def varint(n):
    n &= (1 << 64) - 1
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def field(num, wire, value):
    key = varint(num << 3 | wire)
    if wire == VARINT:
        return key + varint(value)
    if wire == BYTES:
        return key + varint(len(value)) + value
    if wire == FIXED32:
        return key + value
    raise ValueError(wire)


def ipv4_symmetric(addr):
    """uint32 for an address that reads the same in either byte order (e.g. 127.1.1.127)."""
    parts = [int(x) for x in addr.split(".")]
    if parts != parts[::-1]:
        raise ValueError(f"{addr} is not byte-order symmetric; the game byte-swaps this field")
    return struct.unpack(">I", bytes(parts))[0]


def title_config(manifest_url, manifest_guid):
    """Minimal title configuration: only the content manifest block (field 1)."""
    if not 0 < len(manifest_url) < 256 or len(manifest_guid) > 36:
        raise ValueError("manifest URL must be 1-255 bytes, GUID at most 36")
    return field(1, BYTES, field(2, BYTES, manifest_url) + field(3, BYTES, manifest_guid))


def title_config_0124(manifest_url, manifest_guid):
    """01.24 binary overrides: magic, (id, varint length, bytes)*, zero id.

    The 01.24 client dispatches override 0x65 to the 37-byte manifest GUID
    buffer and 0x66 to the manifest URL buffer (EBOOT 0xe196e8/0xe1973c).
    """
    if not 0 < len(manifest_url) < 128 or not 0 < len(manifest_guid) < 37:
        raise ValueError("01.24 manifest URL must be 1-127 bytes, GUID 1-36 bytes")
    return (struct.pack("<I", 0x123456)
            + struct.pack("<I", 0x65) + varint(len(manifest_guid)) + manifest_guid
            + struct.pack("<I", 0x66) + varint(len(manifest_url)) + manifest_url
            + struct.pack("<I", 0))


def success_response(token, key1, key2, key3, expires_at, ip, port, expires_in=None,
                     config=None, config_ids=(1, 1), public_ip=None):
    for k in (key1, key2, key3):
        if len(k) != 16:
            raise ValueError("session keys are 16 bytes")
    if not 0 < len(token) <= 512:
        raise ValueError("token must be 1-512 bytes")
    body = (field(1, BYTES, key1) + field(2, BYTES, key2) + field(3, BYTES, key3)
            + field(4, BYTES, token) + field(5, VARINT, expires_at)
            + field(6, VARINT, ip) + field(7, VARINT, port))
    if expires_in is not None:
        body += field(11, VARINT, expires_in)
    reply = field(1, VARINT, 0) + field(2, BYTES, body)
    if config is not None:
        reply += field(4, BYTES, field(1, VARINT, config_ids[0]) + field(2, VARINT, config_ids[1])
                       + field(3, BYTES, config))
    if public_ip is not None:  # field 14: only the title updates read it (01.00 skips the tag)
        reply += field(14, VARINT, struct.unpack(">I", socket.inet_aton(public_ip))[0])
    return reply


def content_manifest(guid, packages=()):
    """Minimal content manifest (descriptor 0x12a2e7d), fetched from the title-config URL.

    Top level: 1 repeated {1 int32, 2 string[129], 3 int32, 4 optional string[129]} [64];
    2 repeated {1, 2 int32, 3 message (hashes etc.), 4 int32, 5 bool} [64];
    3 repeated {1 int32, 2 string[257], 3 optional int32, 4 optional uint64} [800];
    4 repeated {1 optional string[257], 2 optional int32} [256];
    5 optional string[37] GUID. The bootflow requires field 5 to equal the config's GUID
    (0xb6f7b0).
    The bootflow (0x32d450) registers every field-3 entry as a patchable package: field 3 is
    the package id (int16, 10-6143 when present), field 2 its path, and field 1 an offer
    reference (0 = none). With no entries, loading client_bootstrap_patchable halts the game.
    Field 4 must equal the u64 at offset 8 of the package header; registration (0xe5c2c)
    fails with -90 otherwise.
    `packages` is a list of (id, path, header_u64) triples.
    """
    if len(guid) > 36:
        raise ValueError("GUID is at most 36 bytes")
    body = b""
    for package_id, path, header_value in packages:
        if not 10 <= package_id <= 6143 or not 0 < len(path) <= 256:
            raise ValueError(f"bad package entry {package_id} {path!r}")
        body += field(3, BYTES, field(1, VARINT, 0) + field(2, BYTES, path)
                      + field(3, VARINT, package_id) + field(4, VARINT, header_value))
    return body + field(5, BYTES, guid)


def disc_packages(folder, language="en"):
    """(id, name, header_u64) for each patchable package in a PS3_GAME/USRDIR/packages folder.

    Names look like ps3_bootstrap_01a3_0.pkg or ps3_<name>_0204_en_0.pkg; only the
    requested language variant of localized packages is listed. The header starts
    u16 0x18, u16 4, u16 id, u16 1, u64 (big-endian); only these 16 bytes are read.
    """
    import re
    pattern = re.compile(r"^ps3_.+?_([0-9a-f]{4})(?:_([a-z]{2,3}))?_\d+\.pkg$")
    out = []
    for item in sorted(folder.iterdir()):
        match = pattern.match(item.name)
        if match and match.group(2) in (None, language):
            with item.open("rb") as f:
                magic, _, header_id, _, value = struct.unpack(">4HQ", f.read(16))
            if magic != 0x18 or header_id != int(match.group(1), 16):
                raise ValueError(f"{item.name}: unexpected header")
            out.append((header_id, item.name[:-4].encode(), value))
    return out


def error_response(code, message=b""):
    body = field(1, VARINT, code)
    if message:
        body += field(4, BYTES, message[:199])
    return field(1, VARINT, 2) + field(7, BYTES, body)


def decode(buf):
    """Generic wire decode into [(field, wire, value)], for logging requests."""
    out, p = [], 0
    while p < len(buf):
        key, p = _read_varint(buf, p)
        num, wire = key >> 3, key & 7
        if wire == VARINT:
            val, p = _read_varint(buf, p)
        elif wire == BYTES:
            n, p = _read_varint(buf, p)
            val, p = buf[p:p + n], p + n
        elif wire == FIXED32:
            val, p = buf[p:p + 4], p + 4
        elif wire == FIXED64:
            val, p = buf[p:p + 8], p + 8
        else:
            raise ValueError(f"wire type {wire} at {p}")
        out.append((num, wire, val))
    return out


def _read_varint(buf, p):
    n = shift = 0
    while True:
        b = buf[p]
        p += 1
        n |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return n, p
