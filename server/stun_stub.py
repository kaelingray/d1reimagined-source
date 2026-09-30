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
import select
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


# ---------------------------------------------------------------- 01.24 probes
#
# 01.24 does not use the 01.00 fixed request types. Two shapes were captured on
# this machine (local/SIGNON_LOCAL/logs/FINAL124_S01-stun.log):
#
#   len=4   00 01 00 01
#   len=29  0e 02 00 00 ... 00 ff 00 ff 00 00 00 ff 00 ff 00 00
#
# The 29-byte one is MessagePack: byte 0 = 0x0e is a fixmap of 14 elements, i.e.
# seven integer key/value pairs, all of the rest being MessagePack integers. The
# client is reporting its own socket table before binding, so the -1 values are
# "not bound yet" port fields.
#
# The reply shape is still inferred, not confirmed from the client's parser. What
# is known about the parser (01.24 EBOOT): the STUN response handler is 0xb36fd8,
# called from the transport state machine at 0xb33910/0xb339c4; at 0xb3707c it
# loads the reply's first word and bounds-checks it against 0x13 (0..19), then at
# 0xb370e0 indexes a jump table at 0xb370fc with that value. So the reply is
# dispatched on a small integer index taken from the front of the packet, and
# indices 0..19 are valid.
#
# --reply-index picks that first index (-1 sends none). The msgpack reply echoes
# the request's own shape and fills the mapped address in, which is the least
# assumption that still tells the client what address the server saw it at.


def _mp_int(value):
    """Encode a small integer as MessagePack, shortest form.

    Ports reach 65535, so uint16 (0xcd) is required as well as the fixint forms.
    The reader must be able to walk the value back, so the width actually used
    has to match what the client expects for that field; 0xcd is the widest form
    used here on purpose (the captured request only ever holds -1..127).
    """
    if 0 <= value <= 127:
        return bytes([value])
    if -32 <= value < 0:
        return bytes([value + 0x100])
    if 0 <= value <= 0xFF:
        return b"\xcc" + bytes([value])
    if -128 <= value < 0:
        return b"\xd0" + struct.pack("b", value)
    if 0 <= value <= 0xFFFF:
        return b"\xcd" + struct.pack(">H", value)
    return b"\xce" + struct.pack(">I", value)


def _mp_fixmap_header(pairs):
    """Header for a map of `pairs` entries; use map16 past 15."""
    if pairs <= 15:
        return bytes([pairs])
    return b"\x8f" + struct.pack(">H", pairs)


def parse_mp_map(packet):
    """[(key, value)] from a MessagePack int->int map. None if it is not one."""
    if not packet or packet[0] & 0xF0 != 0x00 or packet[0] > 0x0F:
        return None
    pairs = packet[0] & 0x0F
    pos, out = 1, []
    for _ in range(pairs):
        key, pos = _read_mp_int(packet, pos)
        value, pos = _read_mp_int(packet, pos)
        if key is None or value is None:
            return None
        out.append((key, value))
    return out


def _read_mp_int(buf, pos):
    if pos >= len(buf):
        return None, pos
    b = buf[pos]
    if b <= 0x7F:
        return b, pos + 1
    if b >= 0xE0:
        return b - 0x100, pos + 1
    if b == 0xCC and pos + 2 <= len(buf):
        return buf[pos + 1], pos + 2
    if b == 0xD0 and pos + 2 <= len(buf):
        return struct.unpack("b", buf[pos + 1:pos + 2])[0], pos + 2
    if b == 0xCD and pos + 3 <= len(buf):
        return struct.unpack(">H", buf[pos + 1:pos + 3])[0], pos + 3
    return None, pos


def build_0124_reply(packet, peer, server, args):
    """Reply to a 01.24 probe: the 4-byte ack, or a msgpack map mirroring it."""
    if len(packet) == 4 and packet[:2] == b"\x00\x01":
        # the short probe. The client's dispatch index comes from the first word,
        # so answer with index (default 0) followed by the mapped address.
        if args.reply_index < 0:
            return None
        return _mp_int(args.reply_index) + address_fields(peer[0], peer[1], args.byteorder)

    pairs = parse_mp_map(packet)
    if pairs is None:
        return None
    if args.reply_index < 0:
        return None

    # Mirror the request's keys. The -1 values are the client's unbound port
    # fields; fill them with the address this server saw the probe come from, so
    # the client has a mapped address to report onward.
    #
    # --fill all|one|none chooses how many of them to fill. "all" is the default:
    # the client reports several sockets at once and leaving any of them unbound
    # leaves it retrying (observed: it stops the 4-byte probe and then repeats the
    # map forever). "one" is the earlier, minimal behaviour and "none" echoes the
    # request unchanged.
    if args.fill == "none":
        values = [v for _, v in pairs]
    elif args.fill == "one":
        values, done = [], False
        for _, v in pairs:
            if v == -1 and not done:
                values.append(peer[1])
                done = True
            else:
                values.append(v)
    else:
        values = [peer[1] if v == -1 else v for _, v in pairs]

    out = _mp_fixmap_header(len(pairs))
    for (key, _), value in zip(pairs, values):
        out += _mp_int(key) + _mp_int(value)
    return out


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
    reply = build_0124_reply(packet, peer, server, args)
    if reply is not None:
        return reply
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.0.0.2")
    ap.add_argument("--bind-count", type=int, default=0,
                    help="answer on this many consecutive loopback addresses starting at "
                         "--bind (01.24 walks a 20-name STUN list and may need each name to "
                         "look like a separate server)")
    ap.add_argument("--port", type=int, default=3074)
    ap.add_argument("--log-only", action="store_true", help="record probes without answering")
    ap.add_argument("--byteorder", choices=["little", "big"], default="little",
                    help="port byte order in replies (the IPv4 is always network order)")
    ap.add_argument("--echo-version", action="store_true", help="reply with the request's version byte instead of 2")
    ap.add_argument("--reply-index", type=int, default=0,
                    help="01.24 dispatch index to answer 01.24 probes with (-1 = stay silent); "
                         "the client reads this from the reply's first word and accepts 0..19")
    ap.add_argument("--fill", choices=["all", "one", "none"], default="all",
                    help="how many of the request's unbound (-1) port fields to fill with the "
                         "mapped port in a 01.24 map reply")
    args = ap.parse_args()
    if not ipaddress.ip_address(args.bind).is_loopback:
        sys.exit("refusing to bind a non-loopback address")

    # 01.24 walks a 20-name STUN list, so it may need each name to look like a
    # different server; --bind-count answers on that many consecutive loopback
    # addresses from one process (127.0.2.2, 127.0.2.3, ...). A single address
    # is the 01.00 behaviour and still the default.
    addresses = [args.bind]
    if args.bind_count > 0:
        first = ipaddress.IPv4Address(args.bind)
        octets = str(first).rsplit(".", 1)[0]
        addresses = [f"{octets}.{first.packed[-1] + n}" for n in range(args.bind_count)]
        for a in addresses:
            if not ipaddress.ip_address(a).is_loopback:
                sys.exit(f"refusing to bind a non-loopback address: {a}")

    socks = []
    for a in addresses:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.bind((a, args.port))
        socks.append(s)
    mode = "log-only" if args.log_only else f"answering, byteorder={args.byteorder}, echo_version={args.echo_version}"
    print(f"STUN stub on {', '.join(s.getsockname()[0] for s in socks)}:{args.port} ({mode})", flush=True)

    # one select() over every bound socket, so several addresses share the log
    readable, _, _ = select.select(socks, [], [])
    while True:
        for sock in readable:
            packet, peer = sock.recvfrom(2048)
            server = sock.getsockname()
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
        readable, _, _ = select.select(socks, [], [])


if __name__ == "__main__":
    main()
