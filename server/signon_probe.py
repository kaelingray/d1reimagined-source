"""Loopback-only listener that records what Destiny's sign-on client sends.

Used with dns_sinkhole.py so the game's sign-on hostnames resolve to 127.0.0.1.

Every connection's TLS ClientHello is peeked and summarised (record/client version,
SNI, cipher suites, extensions) before anything else happens. Then, per --mode:

  hello      close the connection after logging the ClientHello
  terminate  complete the handshake with a leaf certificate issued by local_ca.py,
             log the HTTP request, and reply with --status
  signon     as terminate, but answer POST /SignOn with a success message
             (signon_proto.py) that points the game at --session-ip:--session-port
  auth3      as signon, plus a synthetic local PS3 auth3 success reply

The certificate's hostname comes from the SNI name if sent, else from the loopback address
the client dialled (--destiny-hosts listens on every address in hosts.py; the PS3 client
sends no SNI). SSLv3-only clients are served by sslv3.py, because current OpenSSL builds
no longer implement SSL 3.0.

Handshake failures are logged with OpenSSL's reason, e.g. the client's alert when it
rejects the certificate. Raw requests are written only under --capture-dir, which
defaults to a private folder outside Git.
"""

import argparse
import datetime
import ipaddress
import json
import socket
import ssl
import struct
import sys
import threading
import urllib.parse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import hosts  # noqa: E402
import auth3_proto  # noqa: E402
import local_ca  # noqa: E402
import signon_proto  # noqa: E402
import sslv3  # noqa: E402

from cryptography import x509  # noqa: E402
from cryptography.hazmat.primitives import serialization  # noqa: E402

from local_paths import SIGNON  # noqa: E402

CAPTURE_DIR = SIGNON / "captures"

CIPHERS = {
    0x0004: "RSA_RC4_128_MD5", 0x0005: "RSA_RC4_128_SHA", 0x000A: "RSA_3DES_EDE_CBC_SHA",
    0x0009: "RSA_DES_CBC_SHA", 0x0003: "RSA_EXPORT_RC4_40_MD5", 0x0006: "RSA_EXPORT_RC2_CBC_40_MD5",
    0x0008: "RSA_EXPORT_DES40_CBC_SHA", 0x0012: "DHE_DSS_DES_CBC_SHA", 0x0013: "DHE_DSS_3DES_EDE_CBC_SHA",
    0x0015: "DHE_RSA_DES_CBC_SHA", 0x0016: "DHE_RSA_3DES_EDE_CBC_SHA", 0x002F: "RSA_AES_128_CBC_SHA",
    0x0032: "DHE_DSS_AES_128_CBC_SHA", 0x0033: "DHE_RSA_AES_128_CBC_SHA", 0x0035: "RSA_AES_256_CBC_SHA",
    0x0038: "DHE_DSS_AES_256_CBC_SHA", 0x0039: "DHE_RSA_AES_256_CBC_SHA", 0x003C: "RSA_AES_128_CBC_SHA256",
    0x003D: "RSA_AES_256_CBC_SHA256", 0x009C: "RSA_AES_128_GCM_SHA256", 0x009D: "RSA_AES_256_GCM_SHA384",
    0x00FF: "EMPTY_RENEGOTIATION_INFO_SCSV", 0x5600: "FALLBACK_SCSV", 0xC013: "ECDHE_RSA_AES_128_CBC_SHA",
    0xC014: "ECDHE_RSA_AES_256_CBC_SHA", 0xC02F: "ECDHE_RSA_AES_128_GCM_SHA256",
}
EXTENSIONS = {0: "server_name", 5: "status_request", 10: "supported_groups", 11: "ec_point_formats",
              13: "signature_algorithms", 15: "heartbeat", 16: "alpn", 22: "encrypt_then_mac",
              23: "extended_master_secret",
              35: "session_ticket", 43: "supported_versions", 65281: "renegotiation_info"}
VERSIONS = {0x0002: "SSLv2", 0x0300: "SSLv3", 0x0301: "TLS1.0", 0x0302: "TLS1.1", 0x0303: "TLS1.2", 0x0304: "TLS1.3"}

_lock = threading.Lock()
_counter = 0


def log(conn_id, msg):
    stamp = datetime.datetime.now().isoformat(timespec="milliseconds")
    with _lock:
        print(f"{stamp} [{conn_id}] {msg}", flush=True)


def peek_record(sock, timeout=10.0):
    """Peek (without consuming) until one whole TLS record is buffered."""
    sock.settimeout(timeout)
    data = b""
    for _ in range(50):
        data = sock.recv(16384, socket.MSG_PEEK)
        if not data:
            return data
        if data[0] & 0x80:  # SSLv2-format hello: 2-byte length header
            need = 2 + (((data[0] & 0x7F) << 8) | data[1]) if len(data) >= 2 else 3
        else:
            need = 5 + struct.unpack(">H", data[3:5])[0] if len(data) >= 5 else 6
        if len(data) >= need:
            return data[:need]
        threading.Event().wait(0.05)
    return data


def describe_hello(rec):
    if not rec:
        return {"error": "connection closed before any data"}
    if rec[0] & 0x80:
        # SSLv2-compatible ClientHello: msg_type, version, cipher_spec_len, session_id_len, challenge_len
        body = rec[2:]
        ver, cs_len, sid_len, ch_len = struct.unpack(">HHHH", body[1:9])
        specs = [int.from_bytes(body[9 + i:12 + i], "big") for i in range(0, cs_len, 3)]
        return {"format": "SSLv2-compatible", "client_version": VERSIONS.get(ver, hex(ver)),
                "ciphers": [CIPHERS.get(s & 0xFFFF, hex(s)) if s >> 16 == 0 else f"v2:{s:06x}" for s in specs]}
    if rec[0] != 22:
        return {"error": f"not a TLS handshake record (first byte {rec[0]:#x})", "head": rec[:16].hex()}
    rec_ver = struct.unpack(">H", rec[1:3])[0]
    hs = rec[5:]
    if hs[0] != 1:
        return {"error": f"handshake type {hs[0]} is not ClientHello"}
    p = 4
    client_ver = struct.unpack(">H", hs[p:p + 2])[0]
    p += 2 + 32
    p += 1 + hs[p]
    cs_len = struct.unpack(">H", hs[p:p + 2])[0]
    suites = [struct.unpack(">H", hs[p + 2 + i:p + 4 + i])[0] for i in range(0, cs_len, 2)]
    p += 2 + cs_len
    comp = list(hs[p + 1:p + 1 + hs[p]])
    p += 1 + hs[p]
    info = {"format": "TLS", "record_version": VERSIONS.get(rec_ver, hex(rec_ver)),
            "client_version": VERSIONS.get(client_ver, hex(client_ver)),
            "ciphers": [CIPHERS.get(s, f"{s:#06x}") for s in suites], "compression": comp,
            "extensions": [], "sni": None}
    if p + 2 <= len(hs):
        ext_end = p + 2 + struct.unpack(">H", hs[p:p + 2])[0]
        p += 2
        while p + 4 <= ext_end:
            etype, elen = struct.unpack(">HH", hs[p:p + 4])
            edata = hs[p + 4:p + 4 + elen]
            info["extensions"].append(EXTENSIONS.get(etype, str(etype)))
            if etype == 0 and len(edata) > 5:
                info["sni"] = edata[5:5 + struct.unpack(">H", edata[3:5])[0]].decode("ascii", "replace")
            p += 4 + elen
    return info


class CertStore:
    """SSL contexts per SNI name, each with a leaf issued by the local CA."""

    def __init__(self, ca_dir, digest, default_host):
        self.ca_dir, self.digest, self.contexts = ca_dir, digest, {}
        self.default = self.context_for(default_host)
        self.default.sni_callback = self._on_sni

    def context_for(self, host):
        if host not in self.contexts:
            cert, key = local_ca.issue_leaf(host, self.ca_dir, self.digest)
            ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            ctx.minimum_version = ssl.TLSVersion.MINIMUM_SUPPORTED  # the PS3 stack predates TLS 1.2
            ctx.set_ciphers("ALL:@SECLEVEL=0")
            ctx.load_cert_chain(cert, key)
            self.contexts[host] = ctx
        return self.contexts[host]

    def sslv3_material(self, host):
        """(chain as DER list, private key) for the pure-Python SSLv3 server."""
        chain_path, key_path = local_ca.issue_leaf(host, self.ca_dir, self.digest)
        chain = [c.public_bytes(serialization.Encoding.DER)
                 for c in x509.load_pem_x509_certificates(chain_path.read_bytes())]
        return chain, serialization.load_pem_private_key(key_path.read_bytes(), None)

    def _on_sni(self, sslobj, name, _ctx):
        if name:
            sslobj.context = self.context_for(name)


def read_http_request(conn, timeout):
    conn.settimeout(timeout)
    data = b""
    while b"\r\n\r\n" not in data:
        chunk = conn.recv(65536)
        if not chunk:
            return data, None
        data += chunk
    head, _, body = data.partition(b"\r\n\r\n")
    headers = {}
    for line in head.split(b"\r\n")[1:]:
        k, _, v = line.decode("latin-1").partition(":")
        headers[k.strip().lower()] = v.strip()
    length = int(headers.get("content-length", "0") or 0)
    while len(body) < length:
        chunk = conn.recv(65536)
        if not chunk:
            break
        body += chunk
    return head + b"\r\n\r\n" + body, (head.split(b"\r\n")[0].decode("latin-1"), headers, body)


def handle(raw, peer, port, args, certs, bound_host):
    global _counter
    with _lock:
        _counter += 1
        conn_id = f"{port}#{_counter}"
    log(conn_id, f"connection from {peer[0]}:{peer[1]}")
    try:
        if port == args.plain_port:
            raw.settimeout(args.timeout)
            log(conn_id, f"plaintext first bytes: {raw.recv(120, socket.MSG_PEEK)!r}")
            info = None
        else:
            info = describe_hello(peek_record(raw))
            log(conn_id, f"ClientHello: {info}")
        if args.mode == "hello":
            return
        conn = raw
        host = (info or {}).get("sni") or bound_host or args.default_host
        if info is not None and info.get("client_version") == "SSLv3":
            chain, key = certs.sslv3_material(host)
            conn = sslv3.SSLv3Server(raw, chain, key)
            try:
                suite = conn.handshake()
            except (sslv3.SSLv3Error, OSError, ValueError) as exc:
                log(conn_id, f"SSLv3 handshake failed (cert for {host}): {exc!r}")
                return
            log(conn_id, f"SSLv3 established with cert for {host}: {suite}")
        elif info is not None:
            try:
                conn = certs.context_for(host).wrap_socket(raw, server_side=True)
            except (ssl.SSLError, OSError) as exc:
                log(conn_id, f"TLS handshake failed: {exc!r}")
                return
            log(conn_id, f"TLS established with cert for {host}: {conn.version()} {conn.cipher()[0]}")
        raw_req, parsed = read_http_request(conn, args.timeout)
        if parsed is None:
            log(conn_id, f"connection closed before a full HTTP header ({len(raw_req)} bytes)")
            return
        request_line, headers, body = parsed
        log(conn_id, f"HTTP {request_line!r} host={headers.get('host')!r} "
                     f"content-type={headers.get('content-type')!r} body={len(body)} bytes "
                     f"header names={sorted(headers)}")
        args.capture_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
        out = args.capture_dir / f"{stamp}_{conn_id.replace('#', '_')}_{host}.http"
        out.write_bytes(raw_req)
        log(conn_id, f"raw request saved to {out}")
        try:
            log(conn_id, f"body fields: {[(n, w, v if w == 0 else len(v)) for n, w, v in signon_proto.decode(body)]}")
        except (ValueError, IndexError) as exc:
            log(conn_id, f"body is not protobuf: {exc}")
        if args.mode in ("signon", "auth3") and request_line.startswith("POST /SignOn"):
            reply = signon_response(args)
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n"
                         + f"Content-Length: {len(reply)}\r\nConnection: close\r\n\r\n".encode() + reply)
            log(conn_id, f"replied 200 SignOn success ({len(reply)} bytes): session "
                         f"{args.session_ip}:{args.session_port}")
            return
        if (args.mode == "auth3" and request_line.split()[:2] == ["POST", "/auth/"]
                and host in hosts.DEMONWARE_HOSTS and "-auth3." in host):
            try:
                reply, session = auth3_proto.make_response(body)
            except (ValueError, TypeError) as exc:
                log(conn_id, f"auth3 request rejected: {exc}")
                conn.sendall(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                return
            # Keep the key and opaque server ticket together for a future local
            # lobby handler. All captures and session material stay outside Git.
            out.with_suffix(".session.json").write_text(json.dumps(session, indent=2), encoding="utf-8")
            out.with_suffix(".response.json").write_bytes(reply)
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n"
                         + f"Content-Length: {len(reply)}\r\nConnection: close\r\n\r\n".encode() + reply)
            log(conn_id, "replied 200 auth3 task=19 code=700 client_ticket=145 server_ticket=128")
            return
        if (args.manifest_url and request_line.split()[:2]
                == ["GET", urllib.parse.urlsplit(args.manifest_url).path]):
            packages = signon_proto.disc_packages(args.manifest_packages) if args.manifest_packages else []
            reply = signon_proto.content_manifest(args.manifest_guid.encode(), packages)
            conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n"
                         + f"Content-Length: {len(reply)}\r\nConnection: close\r\n\r\n".encode() + reply)
            log(conn_id, f"replied 200 content manifest ({len(reply)} bytes, unsigned, "
                         f"{len(packages)} packages)")
            return
        reason = {200: "OK", 404: "Not Found", 503: "Service Unavailable"}.get(args.status, "Status")
        conn.sendall(f"HTTP/1.1 {args.status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".encode())
        log(conn_id, f"replied {args.status}")
    except Exception as exc:  # keep the listener alive whatever the client does
        log(conn_id, f"error: {exc!r}")
    finally:
        try:
            raw.close()
        except OSError:
            pass


def signon_response(args):
    now = int(datetime.datetime.now(datetime.timezone.utc).timestamp())
    return signon_proto.success_response(
        token=b"d1xenia-local-signon-token-" + b"0" * 37,
        key1=signon_proto.TEST_SESSION_KEYS[0], key2=signon_proto.TEST_SESSION_KEYS[1],
        key3=signon_proto.TEST_SESSION_KEYS[2],
        expires_at=now + 86400, expires_in=86400,
        ip=signon_proto.ipv4_symmetric(args.session_ip), port=args.session_port,
        config=signon_proto.title_config(args.manifest_url.encode(), args.manifest_guid.encode())
        if args.manifest_url else None, public_ip=args.public_ip)


def serve(addr, port, args, certs, bound_host=None):
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE if hasattr(socket, "SO_EXCLUSIVEADDRUSE")
                   else socket.SO_REUSEADDR, 1)
    srv.bind((addr, port))
    srv.listen(16)
    kind = "plain" if port == args.plain_port else "TLS"
    log("-", f"listening on {addr}:{port} ({kind}, mode={args.mode}{', host=' + bound_host if bound_host else ''})")
    while True:
        conn, peer = srv.accept()
        threading.Thread(target=handle, args=(conn, peer, port, args, certs, bound_host), daemon=True).start()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.0.0.1")
    ap.add_argument("--tls-port", type=int, default=443)
    ap.add_argument("--plain-port", type=int, default=80, help="plain HTTP listener; 0 disables")
    ap.add_argument("--mode", choices=["hello", "terminate", "signon", "auth3"], default="hello")
    ap.add_argument("--status", type=int, default=503, help="HTTP status returned in terminate mode")
    ap.add_argument("--digest", choices=["sha1", "sha256"], default="sha1")
    ap.add_argument("--default-host", default="www1.signon.deadorbit.net",
                    help="certificate used on --bind when no SNI is sent")
    ap.add_argument("--destiny-hosts", action="store_true",
                    help="also listen on each sign-on and Demonware host's loopback address from hosts.py")
    ap.add_argument("--ca-dir", type=Path, default=local_ca.DEFAULT_DIR)
    ap.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    ap.add_argument("--session-ip", default="127.1.1.127",
                    help="address returned in the SignOn reply; must be loopback and byte-order symmetric")
    ap.add_argument("--session-port", type=int, default=0x3232,
                    help="port returned in the SignOn reply (default 12850 = 0x3232)")
    ap.add_argument("--manifest-url", default="",
                    help="content manifest URL placed in the SignOn title configuration (field 4)")
    ap.add_argument("--manifest-guid", default="00000000-0000-0000-0000-000000000001")
    ap.add_argument("--manifest-packages", type=Path,
                    help="PS3_GAME/USRDIR/packages folder whose file names are listed in the manifest")
    ap.add_argument("--public-ip", help="01.24: report this client address in SignOn field 14 "
                    "(LIKELY its public IPv4; notes/final-build.md)")
    ap.add_argument("--timeout", type=float, default=15.0)
    args = ap.parse_args()
    if not ipaddress.ip_address(args.bind).is_loopback:
        sys.exit("refusing to bind a non-loopback address")
    if not ipaddress.ip_address(args.session_ip).is_loopback:
        sys.exit("--session-ip must be loopback")

    certs = CertStore(args.ca_dir, args.digest, args.default_host)
    ports = [args.tls_port] + ([args.plain_port] if args.plain_port else [])
    listeners = [(args.bind, None)]
    if args.destiny_hosts:
        listeners += [(ip, name) for name, ip in hosts.HTTPS_HOSTS.items()]
    threads = [threading.Thread(target=serve, args=(addr, port, args, certs, name), daemon=True)
               for addr, name in listeners for port in ports]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


if __name__ == "__main__":
    main()
