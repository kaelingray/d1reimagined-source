"""Read the stand-in's structured run log (<capture folder>/events.jsonl, written by
bap_server.py for every connection): one JSON object per line, either a log line
{"t", "log"}, a client frame {"t", "ev": "frame", "n", "kind", "type", "name", "id", "payload"}
or a client activity-host message {"t", "ev": "ah", "n", "host", "ah_type", "body"}.

  python server/run_log.py summary EVENTS.jsonl             # counts per frame and host message type
  python server/run_log.py ah EVENTS.jsonl --type 6 [--distinct] [--since HH:MM:SS]
  python server/run_log.py bits EVENTS.jsonl --type 6       # which bits vary, per body length
  python server/run_log.py timeline EVENTS.jsonl [--grep rule]

`bits` is for decoding headers such as the type-6 upstream objects: bodies of one length are
compared bit by bit and every bit that ever changes is listed with how many values it took.
A folder argument means its events.jsonl; several files are read in order.
"""

import argparse
import collections
import json
import re
import sys
from pathlib import Path


def events(paths):
    for path in paths:
        path = Path(path)
        if path.is_dir():
            path = path / "events.jsonl"
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    yield json.loads(line)


def clock(event):
    return event["t"].split("T")[-1]


def summary(evs):
    frames, ah, rules, first, last = collections.Counter(), collections.Counter(), [], None, None
    for e in evs:
        first, last = first or e["t"], e["t"]
        if e.get("ev") == "frame":
            frames[e["name"]] += 1
        elif e.get("ev") == "ah":
            ah[e["ah_type"]] += 1
        elif "--ah-on rule" in e.get("log", ""):
            rules.append(f"{clock(e)} {e['log'].split('] ', 1)[-1]}")
    out = [f"from {first} to {last}", "client frames:"]
    out += [f"  {count:6} {name}" for name, count in frames.most_common()]
    out.append("client activity-host messages:")
    out += [f"  {count:6} type {kind}" for kind, count in sorted(ah.items())]
    if rules:
        out.append("rules fired:")
        out += [f"  {r}" for r in rules]
    return out


def host_messages(evs, kind, since=None):
    for e in evs:
        if e.get("ev") == "ah" and (kind is None or e["ah_type"] == kind) and (since is None or clock(e) >= since):
            yield e


def varying_bits(bodies):
    """{length: (message count, [(bit, number of bodies with it set)])}: for each body length,
    the bit positions (MSB first) that are not the same in every body."""
    by_length = collections.defaultdict(list)
    for body in bodies:
        by_length[len(body)].append(body)
    out = {}
    for length, group in sorted(by_length.items()):
        varying = []
        for bit in range(length * 8):
            values = {(b[bit // 8] >> (7 - bit % 8)) & 1 for b in group}
            if len(values) > 1:
                ones = sum((b[bit // 8] >> (7 - bit % 8)) & 1 for b in group)
                varying.append((bit, ones))
        out[length] = (len(group), varying)
    return out


def ranges(bits):
    """[(first, last)] runs of consecutive bit numbers."""
    runs = []
    for bit in bits:
        if runs and bit == runs[-1][1] + 1:
            runs[-1][1] = bit
        else:
            runs.append([bit, bit])
    return [tuple(r) for r in runs]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("summary", "ah", "bits", "timeline"):
        s = sub.add_parser(name)
        s.add_argument("paths", nargs="+")
        if name in ("ah", "bits"):
            s.add_argument("--type", type=int, required=name == "bits")
            s.add_argument("--since", help="HH:MM:SS[.mmm]: only messages from then on")
        if name == "ah":
            s.add_argument("--distinct", action="store_true", help="each body once, with its count")
        if name == "timeline":
            s.add_argument("--grep", help="regex on the log text")
    a = ap.parse_args(argv)
    evs = events(a.paths)
    if a.cmd == "summary":
        print("\n".join(summary(evs)))
    elif a.cmd == "ah":
        msgs = host_messages(evs, a.type, a.since)
        if a.distinct:
            seen = collections.OrderedDict()
            for e in msgs:
                key = (e["ah_type"], e["body"])
                seen.setdefault(key, [clock(e), 0])[1] += 1
            for (kind, body), (first, count) in seen.items():
                print(f"{first} type {kind} x{count} {body}")
        else:
            for e in msgs:
                print(f"{clock(e)} #{e['n']} type {e['ah_type']} {e['body']}")
    elif a.cmd == "bits":
        bodies = [bytes.fromhex(e["body"]) for e in host_messages(evs, a.type, a.since)]
        for length, (count, varying) in varying_bits(bodies).items():
            print(f"{length} bytes, {count} messages: {len(varying)} varying bits")
            for first, last in ranges([bit for bit, _ in varying]):
                ones = {bit: n for bit, n in varying}
                share = ", ".join(f"{ones[b]}/{count}" for b in range(first, min(last, first + 3) + 1))
                print(f"  bits {first}-{last} (bytes {first // 8}-{last // 8}): ones {share}"
                      f"{' ...' if last - first > 3 else ''}")
    else:
        pattern = re.compile(a.grep) if a.grep else None
        for e in evs:
            text = e.get("log")
            if text and (pattern is None or pattern.search(text)):
                print(f"{clock(e)} {text}")


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(0)
