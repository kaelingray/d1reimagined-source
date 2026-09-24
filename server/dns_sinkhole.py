"""Loopback-only DNS sinkhole for RPCS3 tests.

Point RPCS3's Net -> "DNS address" at 127.0.0.1 and every name the game resolves is
answered locally with --answer (default 127.0.0.1). Nothing is forwarded upstream, so
the game cannot reach live Bungie/Demonware hosts by name. Every query is logged.

Only A queries get an answer; AAAA and other types get an empty NOERROR reply.
--override name=ip sends one name elsewhere (e.g. the STUN host to 127.0.0.2, so its stub
does not collide with the game's own UDP port 3074 on 127.0.0.1).
"""

import argparse
import datetime
import ipaddress
import socket
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import hosts  # noqa: E402

QTYPES = {1: "A", 5: "CNAME", 12: "PTR", 15: "MX", 16: "TXT", 28: "AAAA", 33: "SRV", 65: "HTTPS"}


def parse_question(packet):
    """Return (name, qtype, qclass, end_offset) of the first question."""
    labels, pos = [], 12
    while True:
        length = packet[pos]
        if length == 0:
            pos += 1
            break
        if length & 0xC0:
            raise ValueError("compressed name in question")
        labels.append(packet[pos + 1:pos + 1 + length].decode("ascii", "replace"))
        pos += 1 + length
    qtype, qclass = struct.unpack_from(">HH", packet, pos)
    return ".".join(labels), qtype, qclass, pos + 4


def build_reply(query, answer_ip, ttl, overrides=None):
    txid, flags, qdcount = struct.unpack_from(">HHH", query, 0)
    if qdcount < 1:
        return None, None, None, None
    name, qtype, qclass, qend = parse_question(query)
    answer_ip = (overrides or {}).get(name.lower(), answer_ip)
    # QR=1, keep opcode and RD, set AA and RA, RCODE=0.
    reply_flags = 0x8000 | (flags & 0x7900) | 0x0400 | 0x0080
    answer = b""
    if qtype == 1 and qclass == 1:
        answer = struct.pack(">HHHIH", 0xC00C, 1, 1, ttl, 4) + answer_ip
    header = struct.pack(">HHHHHH", txid, reply_flags, 1, 1 if answer else 0, 0, 0)
    return header + query[12:qend] + answer, name, qtype, answer_ip


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=53)
    ap.add_argument("--answer", default="127.0.0.1", help="IPv4 address returned for every A query")
    ap.add_argument("--ttl", type=int, default=30)
    ap.add_argument("--override", action="append", default=[], metavar="NAME=IP",
                    help="answer NAME with IP instead of --answer (repeatable; IP must be loopback)")
    ap.add_argument("--destiny-hosts", action="store_true",
                    help="apply hosts.ALL_OVERRIDES (STUN, per-host sign-on and Demonware loopback addresses)")
    ap.add_argument("--log", type=Path, help="append query log to this file as well as stdout")
    args = ap.parse_args()

    if not ipaddress.ip_address(args.bind).is_loopback:
        sys.exit("refusing to bind a non-loopback address")
    answer_ip = ipaddress.IPv4Address(args.answer).packed
    overrides = {}
    items = list(args.override)
    if args.destiny_hosts:
        items = [f"{k}={v}" for k, v in hosts.ALL_OVERRIDES.items()] + items
    for item in items:
        name, _, ip = item.partition("=")
        addr = ipaddress.IPv4Address(ip)
        if not addr.is_loopback:
            sys.exit(f"override {item!r} is not a loopback address")
        overrides[name.lower().rstrip(".")] = addr.packed

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((args.bind, args.port))
    log = args.log.open("a", encoding="utf-8") if args.log else None
    print(f"DNS sinkhole on {args.bind}:{args.port}, answering A with {args.answer}; {len(overrides)} overrides", flush=True)

    while True:
        query, peer = sock.recvfrom(4096)
        try:
            reply, name, qtype, answered_ip = build_reply(query, answer_ip, args.ttl, overrides)
        except (ValueError, IndexError, struct.error) as exc:
            print(f"malformed query from {peer}: {exc}", flush=True)
            continue
        if reply is None:
            continue
        sock.sendto(reply, peer)
        answered = str(ipaddress.IPv4Address(answered_ip)) if qtype == 1 else "(empty)"
        line = f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {QTYPES.get(qtype, qtype)} {name} -> {answered}"
        print(line, flush=True)
        if log:
            log.write(line + "\n")
            log.flush()


if __name__ == "__main__":
    main()
