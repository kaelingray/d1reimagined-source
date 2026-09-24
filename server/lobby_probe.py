"""Loopback Destiny lobby negotiation probe; does not complete lobby auth.

The 28-byte client greeting starts with two 200 markers, minimum/maximum
protocol 210, a receive limit and an eight-byte nonce. Reply framing:
u32le payload length, AB 81, u32le selected protocol, u64le connection ID,
eight-byte server nonce. Verified against the client's 0xa3ad0c parser.
Format cross-reference (no implementation copied): project-bo4/shield-development
source/proxy-dll/demonware/servers/lobby_server.cpp, server_header_ack.
"""

import argparse
import base64
import datetime
import ipaddress
import json
import secrets
import socket
import struct
import threading
from pathlib import Path

ROOT = Path(r"D:\Destiny360\SIGNON_LOCAL\captures")
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


def log(message):
    print(f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {message}", flush=True)


def match_session(payload, directory):
    """Identify an opaque server ticket; never print its value or session keys."""
    for file in sorted(directory.glob("*.session.json"), reverse=True):
        try:
            session = json.loads(file.read_text(encoding="utf-8"))
            ticket = base64.b64decode(session["server_ticket"], validate=True)
            if len(ticket) == 128 and ticket in payload:
                return file.name, payload.index(ticket)
        except (ValueError, KeyError, OSError):
            continue
    return None


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
        for number in range(3, 11):
            size_bytes = exact(conn, 4)
            size = struct.unpack("<I", size_bytes)[0]
            if size > MAX_FRAME:
                raise ValueError("frame exceeds 1 MiB limit")
            frame = size_bytes + exact(conn, size)
            folder.joinpath(f"{number:02}-client-frame.bin").write_bytes(frame)
            kind = frame[4:6].hex() if size >= 2 else "empty"
            match = match_session(frame, args.session_dir)
            log(f"[{tag}] received {len(frame)} bytes, type={kind}, matched_auth_session={bool(match)}")
            if match:
                folder.joinpath("session-reference.json").write_text(
                    json.dumps({"file": match[0], "ticket_offset": match[1]}), encoding="utf-8")
            # Auth-done needs the correct transcript-derived proof; remain
            # silent here until that derivation is established from the client.
    except (OSError, EOFError, ValueError) as exc:
        log(f"[{tag}] {exc}")
    finally:
        conn.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bind", default="127.0.3.1")
    ap.add_argument("--port", type=int, default=3074)
    ap.add_argument("--timeout", type=float, default=15)
    ap.add_argument("--capture-dir", type=Path, default=ROOT / "lobby210")
    ap.add_argument("--session-dir", type=Path, default=ROOT)
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
