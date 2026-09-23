"""Summarize console packet metadata without displaying captured payloads."""

from collections import Counter, defaultdict
from pathlib import Path
import argparse
import ipaddress
import struct


def dns_name(data: bytes, offset: int, depth: int = 0):
    if depth > 10:
        return None, offset
    labels = []
    end = None
    while offset < len(data):
        length = data[offset]
        if length == 0:
            offset += 1
            break
        if length & 0xC0 == 0xC0:
            if offset + 1 >= len(data):
                return None, offset
            target = ((length & 0x3F) << 8) | data[offset + 1]
            part, _ = dns_name(data, target, depth + 1)
            if part:
                labels.append(part)
            offset += 2
            break
        if length > 63 or offset + 1 + length > len(data):
            return None, offset
        labels.append(data[offset + 1 : offset + 1 + length].decode("ascii", "replace"))
        offset += length + 1
    return ".".join(labels).lower(), end or offset


def dns_details(payload: bytes):
    if len(payload) < 12:
        return [], []
    flags, qcount, acount = struct.unpack_from("!HHH", payload, 2)
    offset = 12
    questions = []
    answers = []
    for _ in range(qcount):
        name, offset = dns_name(payload, offset)
        if not name or offset + 4 > len(payload):
            return questions, answers
        qtype, _qclass = struct.unpack_from("!HH", payload, offset)
        offset += 4
        questions.append((name, qtype))
    if not flags & 0x8000:
        return questions, answers
    for _ in range(acount):
        name, offset = dns_name(payload, offset)
        if not name or offset + 10 > len(payload):
            break
        atype, _aclass, _ttl, length = struct.unpack_from("!HHIH", payload, offset)
        offset += 10
        if offset + length > len(payload):
            break
        if atype == 1 and length == 4:
            answers.append((name, str(ipaddress.IPv4Address(payload[offset : offset + 4]))))
        offset += length
    return questions, answers


def blocks(path: Path):
    data = path.read_bytes()
    offset = 0
    endian = "<"
    while offset + 12 <= len(data):
        block_type = struct.unpack_from(endian + "I", data, offset)[0]
        if block_type == 0x0A0D0D0A:
            magic = data[offset + 8 : offset + 12]
            endian = "<" if magic == b"\x4d\x3c\x2b\x1a" else ">"
        size = struct.unpack_from(endian + "I", data, offset + 4)[0]
        if size < 12 or offset + size > len(data):
            raise ValueError(f"Invalid pcapng block at byte {offset}")
        yield block_type, data[offset + 8 : offset + size - 4], endian
        offset += size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("--console-ip", default="192.168.137.11")
    args = parser.parse_args()
    counts = Counter()
    directions = Counter()
    queries = Counter()
    answers = defaultdict(set)
    earliest = {}
    for block_type, body, endian in blocks(args.capture):
        if block_type != 6 or len(body) < 20:
            continue
        _interface, high, low, captured, original = struct.unpack_from(endian + "IIIII", body)
        frame = body[20 : 20 + captured]
        if len(frame) < 34 or frame[12:14] != b"\x08\x00":
            continue
        ip_offset = 14
        ip_length = (frame[ip_offset] & 15) * 4
        if ip_length < 20 or len(frame) < ip_offset + ip_length + 4:
            continue
        source = str(ipaddress.IPv4Address(frame[ip_offset + 12 : ip_offset + 16]))
        target = str(ipaddress.IPv4Address(frame[ip_offset + 16 : ip_offset + 20]))
        if args.console_ip not in (source, target):
            continue
        protocol = frame[ip_offset + 9]
        if protocol not in (6, 17):
            continue
        transport = ip_offset + ip_length
        source_port, target_port = struct.unpack_from("!HH", frame, transport)
        outgoing = source == args.console_ip
        remote = target if outgoing else source
        port = target_port if outgoing else source_port
        key = ("TCP" if protocol == 6 else "UDP", remote, port)
        counts[key] += 1
        directions[(key, "out" if outgoing else "in")] += 1
        timestamp = (high << 32) | low
        earliest[key] = min(earliest.get(key, timestamp), timestamp)
        if protocol == 17 and (source_port == 53 or target_port == 53):
            udp_length = struct.unpack_from("!H", frame, transport + 4)[0]
            payload = frame[transport + 8 : transport + min(udp_length, len(frame) - transport)]
            q, a = dns_details(payload)
            for name, qtype in q:
                queries[(name, qtype)] += 1
            for name, address in a:
                answers[address].add(name)
    print("Console flows (first seen order; truncated packet contents never printed):")
    for key in sorted(counts, key=lambda k: earliest[k]):
        protocol, remote, port = key
        names = ", ".join(sorted(answers.get(remote, ())))
        print(f"  {protocol:3} {remote:15}:{port:<5} out={directions[key, 'out']:<4} in={directions[key, 'in']:<4} {names}")
    print("DNS questions:")
    for (name, qtype), count in queries.most_common():
        print(f"  {count:3} type={qtype:<2} {name}")


if __name__ == "__main__":
    main()
