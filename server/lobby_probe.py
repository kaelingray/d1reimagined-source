"""Loopback Destiny lobby probe: header negotiation and lobby authentication.

The 28-byte client greeting starts with two 200 markers, minimum/maximum
protocol 210, a receive limit and an eight-byte nonce. Reply framing:
u32le payload length, AB 81, u32le selected protocol, u64le connection ID,
eight-byte server nonce. Verified against the client's 0xa3ad10 parser.
Format cross-reference (no implementation copied): project-bo4/shield-development
source/proxy-dll/demonware/servers/lobby_server.cpp, server_header_ack.

client_auth (AB 82) is an LSB-first bdBitBuffer holding the title ID, auth3
iv_seed and 128-byte server ticket, followed by an 8-byte challenge. The
client (0xa3af98) computes H = SHA1(transcript), PRK = HMAC-SHA1(H, auth3
session key), then HKDF-Expand(PRK, "CLIENTCHAL", 16). It sends bytes 0-7;
server_auth_done (AB 83) must return bytes 8-15 (compared at 0xa3a4e0).
HKDF-Expand(PRK, "BDDATA", 72) keys the encrypted frames that follow: bytes 0-19 and
20-39 are the client and server HMAC-SHA1 keys, 40-55 and 56-71 the client and server
AES-128 keys. An encrypted frame (send path 0xa391bc, receive 0xa3a640) is
  u32le length | AB 85 | u32le sequence | IV[16] | AES-CBC(u32le msg length | msg | zero pad)
  | first 8 bytes of HMAC-SHA1(mac key, everything before them, length prefix included).
Lobby service requests start 86 <service> (e.g. 86 8a: bdMatchMaking) and continue as a
typed bdByteBuffer (server/bdbuffer.py).
"""

import argparse
import sys
import base64
import datetime
import hashlib
import hmac
import ipaddress
import itertools
import json
import secrets
import socket
import struct
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bdbuffer  # noqa: E402
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes  # noqa: E402

from local_paths import SIGNON  # noqa: E402

ROOT = SIGNON / "captures"
MAX_FRAME = 1 << 20


def exact(conn, count):
    data = bytearray()
    while len(data) < count:
        chunk = conn.recv(count - len(data))
        if not chunk:
            raise EOFError(f"closed with {len(data)}/{count} bytes")
        data.extend(chunk)
    return bytes(data)


def header_reply(hello, connection_id, nonce):
    if len(hello) != 28 or len(nonce) != 8:
        raise ValueError("invalid greeting/nonce size")
    marker1, marker2, minimum, maximum, limit = struct.unpack_from("<5I", hello)
    if (marker1, marker2) != (200, 200) or not minimum <= 210 <= maximum:
        raise ValueError("unsupported client greeting")
    return struct.pack("<IBBIQ8s", 22, 0xAB, 0x81, 210, connection_id, nonce)


def hkdf_expand(prk, info, size):
    """RFC 5869 expand with HMAC-SHA1, as implemented at 0xa37308."""
    out, block, counter = b"", b"", 1
    while len(out) < size:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha1).digest()
        out += block
        counter += 1
    return out[:size]


def derive_auth(hello, server_hello, frame, session_key):
    """Recompute the client's lobby-auth values (0xa3b174-0xa3ba48).

    The transcript is the greeting after its two markers, the whole
    server_header_ack frame, and client_auth without its challenge.
    """
    transcript = hello[8:] + server_hello + frame[:-8]
    prk = hmac.new(hashlib.sha1(transcript).digest(), session_key, hashlib.sha1).digest()
    challenge = hkdf_expand(prk, b"CLIENTCHAL", 16)
    data = hkdf_expand(prk, b"BDDATA", 72)
    # Stored at object offsets 0x200 and 0x214 (20 bytes each), then two
    # 16-byte keys for the ciphers at 0x1f8 and 0x1fc. Roles not yet traced.
    return {"challenge": challenge[:8], "verify": challenge[8:],
            "bddata_20": [data[:20], data[20:40]], "bddata_16": [data[40:56], data[56:]]}


def read_bits(data, start, count):
    """Read `count` bytes from an LSB-first bit stream starting at bit `start`."""
    out = bytearray(count)
    for bit in range(count * 8):
        pos = start + bit
        out[bit >> 3] |= ((data[pos >> 3] >> (pos & 7)) & 1) << (bit & 7)
    return bytes(out)


def client_auth_fields(frame):
    """Title ID, iv_seed and server ticket at the bit offsets seen in capture 017b."""
    bits = frame[6:-8]
    if frame[4:6] != b"\xab\x82" or len(bits) * 8 < 83 + 1024:
        raise ValueError("not a client_auth frame")
    title, seed = (struct.unpack("<I", read_bits(bits, pos, 4))[0] for pos in (14, 51))
    return title, seed, read_bits(bits, 83, 128)


def auth_done(verify):
    if len(verify) != 8:
        raise ValueError("verify must be 8 bytes")
    return struct.pack("<IBB8s", 10, 0xAB, 0x83, verify)


class LobbyCrypto:
    """Post-auth lobby frame envelope (see module docstring). Server side: opens client
    frames with the client keys and seals replies with the server keys."""

    def __init__(self, keys):
        self.client_mac, self.server_mac = keys["bddata_20"]
        self.client_aes, self.server_aes = keys["bddata_16"]
        self.seq = 0

    def open(self, frame):
        if frame[4:6] != b"\xab\x85":
            raise ValueError("not an encrypted lobby frame")
        tag = hmac.new(self.client_mac, frame[:-8], hashlib.sha1).digest()[:8]
        if not hmac.compare_digest(tag, frame[-8:]):
            raise ValueError("lobby frame HMAC mismatch")
        seq = struct.unpack_from("<I", frame, 6)[0]
        iv, ct = frame[10:26], frame[26:-8]
        dec = Cipher(algorithms.AES(self.client_aes), modes.CBC(iv)).decryptor()
        pt = dec.update(ct) + dec.finalize()
        size = struct.unpack_from("<I", pt)[0]
        return seq, pt[4:4 + size]

    def seal(self, message):
        self.seq += 1
        pt = struct.pack("<I", len(message)) + message
        pt += bytes(-len(pt) % 16)
        iv = secrets.token_bytes(16)
        enc = Cipher(algorithms.AES(self.server_aes), modes.CBC(iv)).encryptor()
        body = b"\xab\x85" + struct.pack("<I", self.seq) + iv + enc.update(pt) + enc.finalize()
        frame = struct.pack("<I", len(body) + 8) + body
        return frame + hmac.new(self.server_mac, frame, hashlib.sha1).digest()[:8]


def task_reply(msg_type, task_id, error=0, results=b"", count=0, transaction=0x8000000000000000):
    """Experimental Demonware task reply: type byte, then typed uint64 transaction id,
    uint32 error, uint8 task id, uint32 result count, uint32 total, results."""
    w = bdbuffer.Writer()
    w.fixed(bdbuffer.UINT64, transaction).fixed(bdbuffer.UINT32, error).fixed(bdbuffer.UINT8, task_id)
    w.fixed(bdbuffer.UINT32, count).fixed(bdbuffer.UINT32, count)
    return bytes([msg_type]) + w.data() + results


def log(message):
    print(f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {message}", flush=True)


def match_session(hello, server_hello, frame, directory):
    """Find the auth3 session whose key reproduces the client's challenge.

    Never print the ticket, session key or derived keys.
    """
    _, _, ticket = client_auth_fields(frame)
    for file in sorted(directory.glob("*.session.json"), reverse=True):
        try:
            session = json.loads(file.read_text(encoding="utf-8"))
            if base64.b64decode(session["server_ticket"], validate=True) != ticket:
                continue
            key = base64.b64decode(session["session_key"], validate=True)
        except (ValueError, KeyError, OSError):
            continue
        keys = derive_auth(hello, server_hello, frame, key)
        if hmac.compare_digest(keys["challenge"], frame[-8:]):
            return file.name, keys
    return None


def save_keys(path, session_file, keys):
    encode = lambda v: [encode(x) for x in v] if isinstance(v, list) else base64.b64encode(v).decode()
    record = {"session_file": session_file, **{k: encode(v) for k, v in keys.items()}}
    path.write_text(json.dumps(record), encoding="utf-8")


def handle(conn, peer, args):
    tag = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    folder = args.capture_dir / tag
    folder.mkdir(parents=True)
    conn.settimeout(args.timeout)
    try:
        hello = exact(conn, 28)
        folder.joinpath("01-client-hello.bin").write_bytes(hello)
        reply = header_reply(hello, secrets.randbits(64), secrets.token_bytes(8))
        conn.sendall(reply)
        folder.joinpath("02-server-hello.bin").write_bytes(reply)
        log(f"[{tag}] accepted greeting; sent protocol 210 header")
        authenticated = False
        # Keepalives also advance the counter, so it must not be bounded: a fixed limit
        # closed the lobby after ~65 idle minutes (caterpillar, comparison 054).
        for number in itertools.count(3):
            size_bytes = exact(conn, 4)
            size = struct.unpack("<I", size_bytes)[0]
            if size > MAX_FRAME:
                raise ValueError("frame exceeds 1 MiB limit")
            if size == 0 and authenticated:
                # The client sends an empty frame every 40 seconds while idle.
                # Acknowledge it so the lobby connection remains live.
                conn.sendall(size_bytes)
                log(f"[{tag}] replied to empty keepalive frame")
                continue
            frame = size_bytes + exact(conn, size)
            folder.joinpath(f"{number:02}-client-frame.bin").write_bytes(frame)
            kind = frame[4:6].hex() if size >= 2 else "empty"
            log(f"[{tag}] received {len(frame)} bytes, type={kind}")
            if authenticated and kind == "ab85":
                seq, message = crypto.open(frame)
                folder.joinpath(f"{number:02}-client-message.bin").write_bytes(message)
                log(f"[{tag}] message seq={seq} len={len(message)} head={message[:2].hex()} "
                    f"values={bdbuffer.Reader(message[2:]).values()[:6]}")
                if args.reply_type is not None and message[:1] == b"\x86":
                    task_id = message[3] if len(message) > 3 and message[2] == bdbuffer.UINT8 else 0
                    results, count = b"", 0
                    if task_id == 1 and args.session_result:
                        # createSession: one bdSessionID result (8-byte blob, experimental)
                        results = bdbuffer.Writer().blob(bytes.fromhex("5e5510c0de0d1e55")).data()
                        count = 1
                    out = crypto.seal(task_reply(args.reply_type, task_id, error=args.reply_error,
                                                 results=results, count=count))
                    conn.sendall(out)
                    folder.joinpath(f"{number:02}-server-frame.bin").write_bytes(out)
                    log(f"[{tag}] sent experimental task reply type {args.reply_type:#x} task {task_id}")
                continue
            if authenticated or kind != "ab82":
                continue
            title, _, _ = client_auth_fields(frame)
            match = match_session(hello, reply, frame, args.session_dir)
            log(f"[{tag}] client_auth title={title} challenge_verified={bool(match)}")
            if not match:
                continue  # Stay silent; the client times out with cattle.
            save_keys(folder / "lobby-keys.json", *match)
            keys = match[1]
            crypto = LobbyCrypto({"bddata_20": keys["bddata_20"], "bddata_16": keys["bddata_16"]})
            done = auth_done(match[1]["verify"])
            conn.sendall(done)
            folder.joinpath(f"{number:02}-server-auth-done.bin").write_bytes(done)
            authenticated = True
            log(f"[{tag}] sent server_auth_done ({match[0]})")
    except (OSError, EOFError, ValueError) as exc:
        log(f"[{tag}] {exc}")
    finally:
        conn.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bind", default="127.0.3.1")
    ap.add_argument("--port", type=int, default=3074)
    ap.add_argument("--timeout", type=float, default=60)
    ap.add_argument("--capture-dir", type=Path, default=ROOT / "lobby210")
    ap.add_argument("--session-dir", type=Path, default=ROOT)
    ap.add_argument("--reply-type", type=lambda v: int(v, 0),
                    help="experiment: answer lobby service requests with this reply type byte")
    ap.add_argument("--reply-error", type=lambda v: int(v, 0), default=0)
    ap.add_argument("--session-result", action="store_true",
                    help="experiment: answer createSession with one 8-byte session id blob")
    args = ap.parse_args()
    if not ipaddress.IPv4Address(args.bind).is_loopback:
        ap.error("listener must be loopback")
    with socket.socket() as server:
        server.bind((args.bind, args.port))
        server.listen(8)
        log(f"lobby probe listening on {args.bind}:{args.port}")
        while True:
            conn, peer = server.accept()
            threading.Thread(target=handle, args=(conn, peer, args), daemon=True).start()


if __name__ == "__main__":
    main()
