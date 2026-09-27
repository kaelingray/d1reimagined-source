"""Loopback catch-all for the server address handed out in the SignOn reply.

signon_probe.py --mode signon tells Destiny to continue at --session-ip:--session-port
(default 127.1.1.127:12850). The protocol spoken there is not yet known, so this listener
accepts TCP and UDP on that address and port. It logs and saves what arrives, and sends
nothing back. Captures go to a private folder outside Git.
"""

import argparse
import datetime
import ipaddress
import socket
import sys
import threading
from pathlib import Path

from local_paths import SIGNON  # noqa: E402

CAPTURE_DIR = SIGNON / "captures" / "session"
_lock = threading.Lock()


def log(msg):
    stamp = datetime.datetime.now().isoformat(timespec="milliseconds")
    with _lock:
        print(f"{stamp} {msg}", flush=True)


def save(args, tag, data):
    args.capture_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    (args.capture_dir / f"{stamp}_{tag}.bin").write_bytes(data)


def tcp_client(conn, peer, args):
    tag = f"tcp_{peer[1]}"
    log(f"[{tag}] connection from {peer[0]}:{peer[1]}")
    conn.settimeout(args.timeout)
    total = b""
    try:
        while len(total) < 1 << 20:
            chunk = conn.recv(65536)
            if not chunk:
                break
            total += chunk
            log(f"[{tag}] {len(chunk)} bytes: {chunk[:96].hex(' ')}")
    except socket.timeout:
        log(f"[{tag}] idle {args.timeout}s, closing")
    except OSError as exc:
        log(f"[{tag}] {exc!r}")
    finally:
        conn.close()
        if total:
            save(args, tag, total)
        log(f"[{tag}] closed after {len(total)} bytes")


def serve_tcp(args):
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.bind((args.bind, args.port))
    srv.listen(8)
    log(f"TCP listening on {args.bind}:{args.port}")
    while True:
        conn, peer = srv.accept()
        threading.Thread(target=tcp_client, args=(conn, peer, args), daemon=True).start()


def serve_udp(args):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((args.bind, args.port))
    log(f"UDP listening on {args.bind}:{args.port}")
    while True:
        data, peer = sock.recvfrom(65536)
        log(f"[udp {peer[0]}:{peer[1]}] {len(data)} bytes: {data[:96].hex(' ')}")
        save(args, f"udp_{peer[1]}", data)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.1.1.127")
    ap.add_argument("--port", type=int, default=0x3232)
    ap.add_argument("--timeout", type=float, default=30.0)
    ap.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    args = ap.parse_args()
    if not ipaddress.ip_address(args.bind).is_loopback:
        sys.exit("refusing to bind a non-loopback address")
    threads = [threading.Thread(target=f, args=(args,), daemon=True) for f in (serve_tcp, serve_udp)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


if __name__ == "__main__":
    main()
