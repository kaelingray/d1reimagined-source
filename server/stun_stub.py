"""Loopback-only stand-in for Demonware's UDP "stun" host (dev-stun.demonware.net:3074).

Before sign-on, Destiny sends small UDP probes from its port 3074 to the STUN host.
With no answer it shows error marionberry. This stub logs every probe as hex and, unless
--log-only is given, answers the two request types seen in public Demonware emulators:

  type 30 (IP discovery)   -> type 31: type, version, 0, mapped IPv4, mapped port
  type 20 (NAT discovery)  -> type 21: the same, plus the server's IPv4 and port

Layout reference: project-bo4/shield-development, source/proxy-dll/demonware/servers/
stun_server.cpp (GPL-3.0; format only, no code copied). That is a Black Ops 4 PC client,
so the byte order of the address fields is a parameter (--byteorder) until Destiny's own
parsing is confirmed. The mapped address is the probe's source as seen by the stub.

Run dns_sinkhole.py with --override dev-stun.demonware.net=127.0.0.2 and bind this stub
to 127.0.0.2, because RPCS3 holds the game's own 0.0.0.0:3074.
"""

import argparse
import datetime
import ipaddress
import socket
import struct
import sys


def address_fields(ip, port, byteorder):
    packed_ip = ipaddress.IPv4Address(ip).packed  # network order: 127.0.0.1 -> 7f 00 00 01
    if byteorder == "little":
        # BO4 writes the uint32 0x0100007f little-endian (same bytes as network order) and
        # the port as a little-endian uint16.
        return packed_ip + struct.pack("<H", port)
    return packed_ip + struct.pack(">H", port)


def build_reply(packet, peer, server, args):
    if len(packet) < 2:
        return None
    ptype, version = packet[0], packet[1]
    reply_version = version if args.echo_version else 2
    mapped = address_fields(peer[0], peer[1], args.byteorder)
    if ptype == 30:
        return bytes([31, reply_version, 0]) + mapped
    if ptype == 20:
        return bytes([21, reply_version, 0]) + mapped + address_fields(server[0], server[1], args.byteorder)
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.0.0.2")
    ap.add_argument("--port", type=int, default=3074)
    ap.add_argument("--log-only", action="store_true", help="record probes without answering")
    ap.add_argument("--byteorder", choices=["little", "big"], default="little",
                    help="port byte order in replies (the IPv4 is always network order)")
    ap.add_argument("--echo-version", action="store_true", help="reply with the request's version byte instead of 2")
    args = ap.parse_args()
    if not ipaddress.ip_address(args.bind).is_loopback:
        sys.exit("refusing to bind a non-loopback address")

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((args.bind, args.port))
    server = sock.getsockname()
    mode = "log-only" if args.log_only else f"answering, byteorder={args.byteorder}, echo_version={args.echo_version}"
    print(f"STUN stub on {server[0]}:{server[1]} ({mode})", flush=True)

    while True:
        packet, peer = sock.recvfrom(2048)
        stamp = datetime.datetime.now().isoformat(timespec="milliseconds")
        print(f"{stamp} <- {peer[0]}:{peer[1]} len={len(packet)} {packet.hex(' ')}", flush=True)
        if args.log_only:
            continue
        reply = build_reply(packet, peer, server, args)
        if reply is None:
            print(f"{stamp}    (no reply: unknown type {packet[:1].hex()})", flush=True)
            continue
        sock.sendto(reply, peer)
        print(f"{stamp} -> {peer[0]}:{peer[1]} len={len(reply)} {reply.hex(' ')}", flush=True)


if __name__ == "__main__":
    main()
