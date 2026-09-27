"""Summarize the client -> activity-host traffic in a bap_server.py log (the proxy's
`<NAME>-ahl.log` from Invoke-JoinCycle.ps1).

The client sends type 15 about 30 times a second, so a new message type or body is hard to spot in
the raw log. This prints, in time order, only the first occurrence of each (type, body) pair,
interleaved with the stand-in's own pushes (host messages, object steps, zone schedules), then a
count per type. A sensor report (sense state) sent when the guardian walks into a trigger volume,
or a transition request at a z-leg, shows up as a new line after the join
(notes/public-research.md, predictions 1 and 2).

  python scripts/ah_log_summary.py local\\SIGNON_LOCAL\\logs\\TEST367-ahl.log [--since HH:MM:SS]
      [--body-bytes N]   (compare only the first N body bytes; default: whole body)
      [--no-pushes]      (client messages only)
"""

import argparse
import re
import sys
from collections import Counter, defaultdict

LINE = re.compile(r"^(\S+) \[\S+\] (.*)$")
CLIENT = re.compile(r"c->ah not host (0x[0-9a-f]+) type (\d+) len (\d+) body ([0-9a-f]*)")


def summarize(lines, since=None, body_bytes=None, pushes=True, out=sys.stdout):
    seen = set()
    counts = Counter()
    first_time, last_time = {}, {}
    bodies = defaultdict(set)
    for raw in lines:
        m = LINE.match(raw.rstrip("\n"))
        if not m:
            continue
        stamp, message = m.groups()
        clock = stamp[11:23] if "T" in stamp else stamp
        if since and clock < since:
            continue
        c = CLIENT.search(message)
        if c:
            host, ah_type, length, body = c.group(1), int(c.group(2)), int(c.group(3)), c.group(4)
            key_body = body[:2 * body_bytes] if body_bytes is not None else body
            counts[ah_type] += 1
            bodies[ah_type].add(key_body)
            first_time.setdefault(ah_type, clock)
            last_time[ah_type] = clock
            if (ah_type, key_body) not in seen:
                seen.add((ah_type, key_body))
                shown = body if len(body) <= 96 else body[:96] + f"... ({len(body) // 2} bytes)"
                print(f"{clock} NEW  type {ah_type:3d} len {length:5d} host {host} {shown}", file=out)
        elif pushes and ("pushed" in message or "zone schedule" in message or "object" in message):
            print(f"{clock}      {message[:160]}", file=out)
    print("\nper type: count, distinct bodies, first .. last", file=out)
    for ah_type in sorted(counts):
        print(f"  type {ah_type:3d}: {counts[ah_type]:7d} x, {len(bodies[ah_type]):4d} bodies, "
              f"{first_time[ah_type]} .. {last_time[ah_type]}", file=out)
    return counts


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("log")
    ap.add_argument("--since", help="skip lines before this clock time (HH:MM:SS)")
    ap.add_argument("--body-bytes", type=int)
    ap.add_argument("--no-pushes", action="store_true")
    a = ap.parse_args(argv)
    with open(a.log, encoding="utf-8", errors="replace") as f:
        summarize(f, a.since, a.body_bytes, not a.no_pushes)


if __name__ == "__main__":
    main()
