#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Turn a retail-console pcap into answers, by diffing it against what our eboot EXPECTS.

WHY THIS EXISTS
---------------
`historicaldocs/design/THE-BLANKS.md` §0.1 is blunt about the limit: `/SignOn` is HTTPS and the BAP front door is
AES-128-GCM, so a passive tap of a retail console yields **ciphertext**. Reading packets one by one
is therefore a waste of a capture.

What a capture *does* answer is **which blanks in our model are wrong** - and the way to get that
is not to read the pcap, it is to **diff it against the host list compiled into our own eboot**:

    EXPECTED and SEEN      our model is right about this endpoint
    EXPECTED but ABSENT    1.29 names a host the live client no longer uses
    SEEN but UNEXPECTED    ⭐ the live client talks to something our eboot never mentions

That third row is the whole point. `www4.signon.deadorbit.net` - the one signon host that is
actually alive in 2026 - does **not** appear in the 1.29 eboot at all, while the two that do
(`signon.deadorbit.net`, `signon.gravityshavings.net`) are NXDOMAIN. So the runtime host list is
built from something other than these literals, and finding the rest of that gap is exactly what a
capture is for.

WHAT IT WILL AND WILL NOT TELL YOU
----------------------------------
Will:   hostnames, resolved IPs, TLS SNI, per-peer conversations, plaintext-vs-TLS split,
        first-contact order (the boot sequence as the network sees it), UDP/STUN activity.
Won't:  any message body. Do not ask it to.

SEE ALSO - `capture-rig/zeek-logs.py`, which owns a DIFFERENT question
---------------------------------------------------------------------
    THIS TOOL      "is our MODEL of the client right?"  - one capture, judged against the
                   hostname literals compiled into the 1.29 eboot.  Owns
                   seen-but-not-in-eboot / expected-and-seen / compiled-but-never-queried.

    zeek-logs.py   "what CHANGED between run A and run B?" - pcap -> Zeek conn/dns/ssl/x509
                   logs, and a diff of two log sets.  Owns connection STRUCTURE: endpoints,
                   ports, transport, service, first-contact order, TLS parameters, certs.
                   Knows nothing about the eboot, by design.

They overlap on "which hostnames appeared" deliberately - the two derive it from different
places (tshark display fields vs Zeek's DNS analyzer), so a **disagreement is a finding**.
Run both on a retail capture.  Details: `historicaldocs/design/zeek-pipeline.md` §9.

USAGE
-----
    sudo capture-rig/rig-mode.sh live            # bridge the console iface -- SEE THE WARNING
    sudo tcpdump -i br0 -s0 -w runs/retail.pcap 'host <console-ip>'
    python3 -B capture-rig/pcap-summary.py runs/retail.pcap
    python3 -B capture-rig/pcap-summary.py runs/retail.pcap --json

⚠ `rig-mode.sh live` bridges the console interface to the internet. **The CFW console must never
be the thing plugged into it in that mode** - a firmware update past 13.00 is irreversible.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import shutil
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_EBOOT = REPO / "dumps" / "patch-01.29-CUSA00219" / "eboot.bin"
# Deliberately the same shape used to build the baseline by hand, so the two cannot drift.
HOST_PATTERN = re.compile(rb"\b[a-z0-9][a-z0-9._-]*\.(?:net|com|org|io|tv)\b")
# Hosts every PS4 contacts regardless of title. Not evidence about Destiny; muted by default so
# they cannot drown the interesting rows.
PLATFORM_NOISE = re.compile(
    r"(playstation\.(net|com)|sony|scea|akamai|cloudflare|ntp\.|pool\.ntp|"
    r"googleapis|gstatic|microsoft|windows)"
)


class PcapError(RuntimeError):
    pass


def need_tshark() -> str:
    path = shutil.which("tshark")
    if path is None:
        raise PcapError("tshark is not installed (dnf install wireshark-cli)")
    return path


def tshark_fields(pcap: pathlib.Path, display_filter: str, fields: list[str]) -> list[list[str]]:
    command = [need_tshark(), "-r", str(pcap), "-Y", display_filter, "-T", "fields"]
    for field in fields:
        command += ["-e", field]
    command += ["-E", "separator=\t", "-E", "occurrence=a"]
    done = subprocess.run(command, capture_output=True, text=True)
    if done.returncode != 0:
        raise PcapError(done.stderr.strip() or "tshark failed")
    rows = []
    for line in done.stdout.splitlines():
        if line.strip():
            rows.append(line.split("\t"))
    return rows


def eboot_hosts(eboot: pathlib.Path) -> set[str]:
    """The hostnames compiled into the client. This is the model a capture is tested against."""
    if not eboot.exists():
        raise PcapError(f"{eboot} not found - needed as the expectation baseline")
    blob = eboot.read_bytes()
    return {m.decode().lower() for m in HOST_PATTERN.findall(blob)}


def collect(pcap: pathlib.Path, eboot: pathlib.Path) -> dict:
    expected = eboot_hosts(eboot)

    dns_rows = tshark_fields(
        pcap, "dns.flags.response==1", ["dns.qry.name", "dns.a", "dns.cname"]
    )
    resolved: dict[str, set[str]] = collections.defaultdict(set)
    for row in dns_rows:
        name = (row[0] if row else "").lower()
        if not name:
            continue
        addresses = row[1].split(",") if len(row) > 1 and row[1] else []
        resolved[name].update(a for a in addresses if a)

    queried = {
        (row[0] or "").lower()
        for row in tshark_fields(pcap, "dns.flags.response==0", ["dns.qry.name"])
        if row and row[0]
    }
    queried |= set(resolved)

    sni_rows = tshark_fields(
        pcap,
        "tls.handshake.type==1",
        ["tls.handshake.extensions_server_name", "ip.dst", "tcp.dstport"],
    )
    sni: dict[str, set[str]] = collections.defaultdict(set)
    for row in sni_rows:
        if row and row[0]:
            peer = f"{row[1] if len(row) > 1 else '?'}:{row[2] if len(row) > 2 else '?'}"
            sni[row[0].lower()].add(peer)

    first_seen: dict[str, float] = {}
    for row in tshark_fields(pcap, "dns.flags.response==0", ["frame.time_relative", "dns.qry.name"]):
        if len(row) >= 2 and row[1]:
            name = row[1].lower()
            when = float(row[0]) if row[0] else 0.0
            first_seen.setdefault(name, when)

    conversations: dict[tuple[str, str], dict] = {}
    for row in tshark_fields(
        pcap, "tcp", ["ip.dst", "tcp.dstport", "frame.len", "tls.record.content_type"]
    ):
        if len(row) < 3 or not row[0]:
            continue
        key = (row[0], row[1])
        entry = conversations.setdefault(key, {"packets": 0, "bytes": 0, "tls": False})
        entry["packets"] += 1
        entry["bytes"] += int(row[2]) if row[2].isdigit() else 0
        if len(row) > 3 and row[3]:
            entry["tls"] = True

    udp: dict[tuple[str, str], int] = collections.Counter()
    for row in tshark_fields(pcap, "udp && !dns", ["ip.dst", "udp.dstport"]):
        if len(row) >= 2 and row[0]:
            udp[(row[0], row[1])] += 1

    return {
        "expected_hosts": sorted(expected),
        "queried": sorted(queried),
        "resolved": {k: sorted(v) for k, v in resolved.items()},
        "sni": {k: sorted(v) for k, v in sni.items()},
        "first_seen": first_seen,
        "tcp": {f"{k[0]}:{k[1]}": v for k, v in conversations.items()},
        "udp": {f"{k[0]}:{k[1]}": v for k, v in udp.items()},
    }


def report(data: dict, show_noise: bool) -> None:
    expected = set(data["expected_hosts"])
    queried = set(data["queried"])

    def interesting(host: str) -> bool:
        return show_noise or not PLATFORM_NOISE.search(host)

    seen_expected = sorted(h for h in queried & expected if interesting(h))
    unexpected = sorted(h for h in queried - expected if interesting(h))
    absent = sorted(h for h in expected - queried if interesting(h))

    print("=" * 78)
    print("HOSTNAMES - the client's live list vs the 1.29 eboot's compiled list")
    print("=" * 78)

    print(f"\n⭐ SEEN but NOT IN THE EBOOT  ({len(unexpected)}) - the model is incomplete here")
    for host in unexpected:
        addresses = ", ".join(data["resolved"].get(host, [])) or "(no A record)"
        print(f"    {host:<48} {addresses}")
    if not unexpected:
        print("    (none - every host the client contacted is a compiled-in literal)")

    print(f"\n✅ EXPECTED and SEEN  ({len(seen_expected)})")
    for host in seen_expected:
        addresses = ", ".join(data["resolved"].get(host, [])) or "(no A record)"
        print(f"    {host:<48} {addresses}")

    print(f"\n⚠ IN THE EBOOT but NEVER QUERIED  ({len(absent)}) - dead literals or unused paths")
    for host in absent:
        print(f"    {host}")

    if data["sni"]:
        print("\n" + "=" * 78)
        print("TLS SNI - what the client asked each connection to be")
        print("=" * 78)
        for host, peers in sorted(data["sni"].items()):
            print(f"    {host:<48} {', '.join(sorted(peers))}")

    if data["first_seen"]:
        print("\n" + "=" * 78)
        print("BOOT ORDER - first DNS query per host (seconds into the capture)")
        print("=" * 78)
        for host, when in sorted(data["first_seen"].items(), key=lambda kv: kv[1]):
            if interesting(host):
                print(f"    {when:8.2f}s  {host}")

    print("\n" + "=" * 78)
    print("CONVERSATIONS - ⭐ PLAINTEXT rows are the ones worth opening")
    print("=" * 78)
    for peer, info in sorted(data["tcp"].items(), key=lambda kv: -kv[1]["bytes"])[:30]:
        kind = "TLS      " if info["tls"] else "PLAINTEXT"
        print(f"    {kind} {peer:<28} {info['packets']:>6} pkts  {info['bytes']:>10} B")

    if data["udp"]:
        print("\n    UDP (STUN/P2P candidates):")
        for peer, count in sorted(data["udp"].items(), key=lambda kv: -kv[1])[:15]:
            print(f"      {peer:<28} {count:>6} pkts")

    print("\n" + "-" * 78)
    print("Reminder: bodies are encrypted. This tool answers WHICH endpoints and IN WHAT ORDER,")
    print("not what was said. See historicaldocs/design/THE-BLANKS.md §0.1.")


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("pcap", type=pathlib.Path)
    parser.add_argument("--eboot", type=pathlib.Path, default=DEFAULT_EBOOT)
    parser.add_argument("--show-platform-noise", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    try:
        if not args.pcap.exists():
            raise PcapError(f"{args.pcap} not found")
        data = collect(args.pcap, args.eboot)
    except PcapError as error:
        print(f"pcap-summary: {error}", file=sys.stderr)
        return 2

    if args.json:
        print(json.dumps(data, indent=2, sort_keys=True))
    else:
        report(data, args.show_platform_noise)
    return 0


if __name__ == "__main__":
    sys.exit(main())
