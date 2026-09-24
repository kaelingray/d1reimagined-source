"""Loopback Destiny BAP (Bungie session server) stand-in: answers the session hello.

After content registration the client connects to the address from the SignOn reply
(127.1.1.127:12850) and sends `shello req`. Framing, from BLES01857 01.00:

  outer   u8 1 (version), u8 kind, u32be length of the rest            (0x7c413c)
          kind 2 = plaintext; kind 1 = encrypted with the shello session key (0x7c3fbc)
  header  kind 1 (client->server): u16be type, u32be id                (size 6, 0x81684)
          kind 2 (server->client): u16be type, u32be id, u16be status  (size 8, 0x82e30)
          The message codec's method 1 picks the header kind (0x41501c table).
  body    message payload.

Message names come from 0x7c6ef4: 0x19 "shello req", 0x1a "shello rsp", 0x12/0x13 client
config request/response, and so on. shello req (descriptor 0x11f3e00) is protobuf
{1 bytes SignOn token, 2 int32 mode, 3 optional uint64, 4 optional bool} (0x7c648c).

shello rsp (codec 0x41676c) is u32be N (N <= 0x5c) followed by N bytes, sealed as in
0x7c7794 and opened by 0x7c7660:
  u32be N | IV[16] | AES-128-CBC(key, IV, plaintext + PKCS#7) | HMAC-SHA256(mac_key, u32|IV|ct)
The AES key is SignOn session key 2 and the MAC key session key 3 (0x7c75f8). The
plaintext is nonce[12] || session_key[16] (0xb0cd30); the reply status must be 200.

Kind 1 frames (after shello) are AES-128-GCM with that session key, no AAD, 16-byte tag
first: tag[16] || ciphertext(inner header || payload) (0xb0ce4c / 0xb0cf58, 0x7c743c).
Each direction has its own 12-byte nonce: the client sends with the reply nonce's last byte
XOR 1 and receives with the nonce unchanged (0xb0cd30 with direction 0); after each message
the nonce is incremented as a little-endian counter from byte 0.

Encrypted requests seen so far: 0x79 "qz_reg req" (empty payload), then 0x12e "rrc req"
(40 bytes of address data: LAN IPv4, port 3074 little-endian, masks, the STUN-reported
address and a u64). Their replies have header kind 2 and decoders that ignore the payload.
Then 0x0a "c->ws req": a web-service call (codec 0xb4e208/0xb4e444) encoded with the
engine's bit-packed type serializer: 16-bit service id (503 = 0x01f7 in the first call),
the service's request structure, then a flag bit for an optional blob. The reply 0x0b
uses the same layout with the service's response type (descriptor +0x10). Without the
type definitions, --ws-zero-bytes answers with the service id followed by zero bytes,
which deserializes as an all-zero response structure and no blob.
"""

import argparse
import datetime
import hashlib
import hmac
import ipaddress
import json
import secrets
import socket
import struct
import sys
import threading
from pathlib import Path

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

sys.path.insert(0, str(Path(__file__).resolve().parent))
import signon_proto  # noqa: E402

CAPTURE_DIR = Path(r"D:\Destiny360\SIGNON_LOCAL\captures\bap")
SHELLO_REQ, SHELLO_RSP = 0x19, 0x1A
QZ_REG_REQ, QZ_REG_RSP = 0x79, 0x7A
ECHO_REQ, ECHO_RSP, RRC_REQ, RRC_RSP = 0xFA, 0xFB, 0x12E, 0x12F
WS_REQ, WS_RSP = 0x0A, 0x0B
# The client's reply queue (0x7c6bc0) expects exactly four reply types: shello rsp, echo
# rsp, qz_reg rsp and rrc rsp. The last three decode with 0xa62a0, which ignores the
# payload, so an empty status-200 reply satisfies them. Request -> reply type.
EMPTY_REPLIES = {QZ_REG_REQ: QZ_REG_RSP, ECHO_REQ: ECHO_RSP, RRC_REQ: RRC_RSP}
NAMES = {0x03: "goodbye not", 0x06: "c->ah mgr req", 0x07: "c->ah mgr rsp", 0x08: "c->ah not",
         0x09: "b->c not", 0x0A: "c->ws req", 0x0B: "c->ws rsp", 0x0C: "sub req", 0x0D: "sub rsp",
         0x0E: "unsubscribe req", 0x0F: "unsubscribe rsp", 0x10: "ahp req", 0x11: "ahp rsp",
         0x12: "client config req", 0x13: "client config rsp", 0x14: "shutdown not",
         0x15: "upo req", 0x16: "upo rsp", 0x17: "acct id trans req", 0x18: "acct id trans rsp",
         SHELLO_REQ: "shello req", SHELLO_RSP: "shello rsp", QZ_REG_REQ: "qz_reg req",
         QZ_REG_RSP: "qz_reg rsp", 0x7B: "qz->c update not", 0x7C: "qz->c sub lost not",
         ECHO_REQ: "echo req", ECHO_RSP: "echo rsp", 0x12C: "c->b irc not", 0x12D: "b->c rrc not",
         RRC_REQ: "rrc req", RRC_RSP: "rrc rsp"}


def seal(key, mac_key, plaintext, iv):
    pad = 16 - len(plaintext) % 16
    padded = plaintext + bytes([pad]) * pad
    encryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    body = iv + encryptor.update(padded) + encryptor.finalize()
    head = struct.pack(">I", len(body) + 32)
    return head + body + hmac.new(mac_key, head + body, hashlib.sha256).digest()


def unseal(key, mac_key, blob):
    (length,) = struct.unpack_from(">I", blob)
    if length + 4 != len(blob) or length < 0x30:
        raise ValueError("bad sealed length")
    mac = blob[-32:]
    if not hmac.compare_digest(mac, hmac.new(mac_key, blob[:-32], hashlib.sha256).digest()):
        raise ValueError("MAC mismatch")
    decryptor = Cipher(algorithms.AES(key), modes.CBC(blob[4:20])).decryptor()
    padded = decryptor.update(blob[20:-32]) + decryptor.finalize()
    pad = padded[-1]
    if not 1 <= pad <= 16 or padded[-pad:] != bytes([pad]) * pad:
        raise ValueError("bad padding")
    return padded[:-pad]


def shello_rsp_body(key, mac_key, nonce, session_key, iv):
    if len(nonce) != 12 or len(session_key) != 16:
        raise ValueError("nonce is 12 bytes, session key 16")
    blob = seal(key, mac_key, nonce + session_key, iv)
    if len(blob) - 4 > 0x5C:
        raise ValueError("sealed shello reply exceeds the client's 0x5c limit")
    return blob


class GcmChannel:
    """One direction of the post-shello encrypted stream."""

    def __init__(self, key, nonce):
        self.aead, self.nonce = AESGCM(key), bytearray(nonce)

    def _advance(self):
        for i in range(12):
            self.nonce[i] = (self.nonce[i] + 1) & 0xFF
            if self.nonce[i]:
                break

    def open(self, body):
        plain = self.aead.decrypt(bytes(self.nonce), body[16:] + body[:16], None)
        self._advance()
        return plain

    def seal(self, plain):
        out = self.aead.encrypt(bytes(self.nonce), plain, None)
        self._advance()
        return out[-16:] + out[:-16]


def channels(nonce, session_key):
    """(server receive, server send) channels for a session."""
    client_send = nonce[:11] + bytes([nonce[11] ^ 1])
    return GcmChannel(session_key, client_send), GcmChannel(session_key, nonce)


def encrypted_frame(channel, msg_type, msg_id, status, payload):
    body = channel.seal(struct.pack(">HIH", msg_type, msg_id, status) + payload)
    return struct.pack(">BBI", 1, 1, len(body)) + body


def plain_frame(msg_type, msg_id, status, body):
    inner = struct.pack(">HIH", msg_type, msg_id, status) + body
    return struct.pack(">BBI", 1, 2, len(inner)) + inner


def exact(conn, count):
    data = bytearray()
    while len(data) < count:
        chunk = conn.recv(count - len(data))
        if not chunk:
            raise EOFError(f"closed with {len(data)}/{count} bytes")
        data.extend(chunk)
    return bytes(data)


def log(message):
    print(f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {message}", flush=True)


def handle(conn, peer, args):
    tag = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    folder = args.capture_dir / tag
    folder.mkdir(parents=True)
    conn.settimeout(args.timeout)
    log(f"[{tag}] connection from {peer[0]}:{peer[1]}")
    rx = tx = None
    try:
        for number in range(1, 200):
            head = exact(conn, 6)
            version, kind, length = struct.unpack(">BBI", head)
            if version != 1 or length > 1 << 20:
                raise ValueError(f"unexpected outer header {head.hex()}")
            body = exact(conn, length)
            folder.joinpath(f"{number:03}-client-kind{kind}.bin").write_bytes(head + body)
            if kind == 1 and rx is not None:
                body = rx.open(body)
                folder.joinpath(f"{number:03}-client-plain.bin").write_bytes(body)
            elif kind != 2 or length < 6:
                log(f"[{tag}] frame {number}: kind {kind}, {length} bytes (captured)")
                continue
            msg_type, msg_id = struct.unpack_from(">HI", body)
            payload = body[6:]
            log(f"[{tag}] frame {number} (kind {kind}): {NAMES.get(msg_type, hex(msg_type))} "
                f"id={msg_id} payload={len(payload)} bytes {payload[:48].hex()}")
            if kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_zero_bytes:
                body = bytearray(args.ws_zero_bytes)
                body[0] = (args.ws_result & 0xF) << 4  # u4 result code, MSB-first (0 = error_none)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, payload[:2] + bytes(body))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-rsp.bin").write_bytes(reply)
                log(f"[{tag}] sent c->ws rsp for service {int.from_bytes(payload[:2], 'big')} "
                    f"(result {args.ws_result}, {args.ws_zero_bytes} bytes)")
            if kind == 1 and msg_type in EMPTY_REPLIES:
                reply = encrypted_frame(tx, EMPTY_REPLIES[msg_type], msg_id, 200, b"")
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-{EMPTY_REPLIES[msg_type]:02x}.bin").write_bytes(reply)
                log(f"[{tag}] sent {NAMES[EMPTY_REPLIES[msg_type]]} (empty, status 200)")
            if msg_type == SHELLO_REQ:
                fields = [(n, w, v if w == 0 else len(v)) for n, w, v in signon_proto.decode(payload)]
                log(f"[{tag}] shello req fields {fields}")
                nonce, session_key = secrets.token_bytes(12), secrets.token_bytes(16)
                reply = plain_frame(SHELLO_RSP, msg_id, 200, shello_rsp_body(
                    args.key, args.mac_key, nonce, session_key, secrets.token_bytes(16)))
                conn.sendall(reply)
                rx, tx = channels(nonce, session_key)
                folder.joinpath(f"{number:03}-server-shello-rsp.bin").write_bytes(reply)
                folder.joinpath("session.json").write_text(json.dumps(
                    {"nonce": nonce.hex(), "session_key": session_key.hex()}), encoding="utf-8")
                log(f"[{tag}] sent shello rsp ({len(reply)} bytes)")
    except (OSError, EOFError, ValueError, InvalidTag) as exc:
        log(f"[{tag}] {exc}")
    finally:
        conn.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.1.1.127")
    ap.add_argument("--port", type=int, default=12850)
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--ws-zero-bytes", type=int, default=1024,
                    help="answer c->ws requests with the service id and this many zero bytes (0: no reply)")
    ap.add_argument("--ws-result", type=int, default=0,
                    help="ws result code in the first 4 bits (0 error_none, 1 unknown, 2 no_content, "
                         "3 moved, 4 not_found, 5 service_unavailable, 6 unauthorized, 7 timeout)")
    ap.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    args = ap.parse_args()
    if not ipaddress.IPv4Address(args.bind).is_loopback:
        ap.error("listener must be loopback")
    args.key, args.mac_key = signon_proto.TEST_SESSION_KEYS[1], signon_proto.TEST_SESSION_KEYS[2]
    with socket.socket() as server:
        server.bind((args.bind, args.port))
        server.listen(8)
        log(f"BAP stand-in listening on {args.bind}:{args.port}")
        while True:
            conn, peer = server.accept()
            threading.Thread(target=handle, args=(conn, peer, args), daemon=True).start()


if __name__ == "__main__":
    main()
