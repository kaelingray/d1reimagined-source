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
  4  message: 1 uint64, 2 uint32 (required); 3 bytes (callback).
  5  uint64 flags, optional.
  6  string[64], optional.
  7  error    message: 1 code int32 required (1 suspended, 2-3 denied, 4 prelaunch,
              5 platform, 6 configuration, 7 commercialization, 8 version mismatch,
              other denied); 2 int32; 3 uint64; 4 string[200]; 5 string[50].
  8  int32, optional.   10 message {1: repeated int32 [64]}.   12 uint64, optional.

HTTP 500 maps to _internal_server_error, 503/509 to _server_unavailable, and any other
non-2xx status to _retriable_error.
"""

import struct

VARINT, FIXED64, BYTES, FIXED32 = 0, 1, 2, 5


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


def success_response(token, key1, key2, key3, expires_at, ip, port, expires_in=None):
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
    return field(1, VARINT, 0) + field(2, BYTES, body)


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
