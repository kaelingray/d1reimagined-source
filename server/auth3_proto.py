"""Diagnostic Destiny PS3 auth3 reply, derived from BLES01857 01.00.

Request task 18 expects reply task 19 (0xa264f8). The JSON uint reader at
0xa2c7a8 accepts strings and numbers. 0xa24b94 base64-decodes the tickets
without decrypting them. 0xa2c1dc reads the packed 128-byte auth ticket in
little-endian order; its aligned destination object is *not* the wire layout.
The 24-byte key is at wire offset 0x61, object offset 0x68. PS3 adds 17 bytes.

This issues synthetic local test sessions, not verified PSN identities.
"""

import base64
import json
import secrets
import struct
import time

TITLE_ID = 5378
MAGIC = 0xEFBDADDE
# magic, type, title, issued, expires, license, user, name, key, reserved, hash
TICKET = struct.Struct("<IBIIIQQ64s24s3s4s")
PS3_INFO = struct.Struct("<IIBBB6x")


def uint32(value, name):
    if isinstance(value, bool) or not isinstance(value, (str, int)):
        raise ValueError(f"{name} must be a decimal uint32")
    if isinstance(value, str) and (not value.isascii() or not value.isdecimal()):
        raise ValueError(f"{name} must be a decimal uint32")
    n = int(value)
    if not 0 <= n <= 0xFFFFFFFF:
        raise ValueError(f"{name} is outside uint32 range")
    return n


def make_response(body, *, now=None, session_key=None, server_ticket=None):
    """Return (JSON bytes, private session record). Never log the record."""
    request = json.loads(body)
    if not isinstance(request, dict):
        raise ValueError("auth request must be an object")
    if uint32(request.get("auth_task"), "auth_task") != 18:
        raise ValueError("only PS3 task 18 is supported")
    if uint32(request.get("title_id"), "title_id") != TITLE_ID:
        raise ValueError("unexpected title_id")
    seed = uint32(request.get("iv_seed"), "iv_seed")
    issued = int(time.time()) if now is None else now
    key = secrets.token_bytes(24) if session_key is None else session_key
    opaque = secrets.token_bytes(128) if server_ticket is None else server_ticket
    if len(key) != 24 or len(opaque) != 128:
        raise ValueError("session key must be 24 bytes; server ticket must be 128 bytes")
    ticket = TICKET.pack(MAGIC, 0, TITLE_ID, issued, issued + 86400,
                         1, 1, b"d1localtest", key, bytes(3), bytes(4))
    # The PS3 deserializer consumes two u32s and three flags from the 17-byte
    # extension. Their meaning is not yet established; zero is a test value.
    client = ticket + PS3_INFO.pack(0, 0, 0, 0, 0)
    encode = lambda data: base64.b64encode(data).decode("ascii")
    response = {"auth_task": "19", "code": "700", "iv_seed": str(seed),
                "client_ticket": encode(client), "server_ticket": encode(opaque)}
    private = {"issued": issued, "expires": issued + 86400, "user_id": 1,
               "iv_seed": seed, "session_key": encode(key),
               "server_ticket": encode(opaque), "client_ticket": encode(client)}
    return json.dumps(response, separators=(",", ":")).encode("ascii"), private
