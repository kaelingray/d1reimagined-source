"""Summarize decoded client activity incidents from a private AHL log.

Use the corrected 01.24 type export with --schema. The incident type chain
and its opaque byte body are separate fields: an AH message's type 18 is
not itself an incident ID. Counts and repeated bodies do not establish
retransmission or acknowledgement semantics. Output contains timestamps,
IDs and SHA-256 digests, never raw bodies; save reports outside Git or in
the repository's ignored local/ directory.
"""

import argparse
from collections import Counter
import datetime as dt
import hashlib
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
import investment  # noqa: E402

INCIDENT_HANDLE = 0x8080097D
MAX_LINE = 65536
MAX_PAYLOAD = 8192
MAX_TYPES = 256
MAX_CHAINS = 64
MAX_DIGESTS = 512
CLIENT = re.compile(r"^(\S+) \[\S+\] .*?c->ah not host (0x[0-9a-fA-F]{1,16}) "
                    r"type (\d{1,3}) len (\d{1,6}) body (\S*)\s*$")


def bounded_lines(stream, limit=MAX_LINE):
    """Yield a line or None for an oversized line, without retaining it."""
    while True:
        line = stream.readline(limit + 1)
        if not line:
            return
        if len(line) > limit:
            while not line.endswith("\n"):
                line = stream.readline(limit + 1)
                if not line:
                    break
            yield None
        else:
            yield line


def validate_schema(schema, handle):
    """Require the measured list/list/byte-body layout, not a guessed schema."""
    root = schema.get(f"{handle:#x}", {})
    if [f.get("off") for f in root.get("fields", [])] != [0, 104, 212]:
        raise ValueError("incident schema must contain the 01.24 fields at 0, 104 and 212")
    for field in root["fields"]:
        if field.get("kind") != 1 or field.get("arr") != 0:
            raise ValueError("incident schema has an unexpected nested-list layout")
        nested = schema.get(f"{field['nested']:#x}", {}).get("fields", [])
        if len(nested) != 2 or [f.get("off") for f in nested] != [0, 4]:
            raise ValueError("incident list schema must contain count and slots")
        count, array = nested
        if (count.get("kind") != 5 or not 1 <= count.get("width", 0) <= 32
                or count.get("min") != 0 or count.get("opt")
                or array.get("kind") != 1 or array.get("arr") != 1
                or array.get("width") != 0 or array.get("opt")):
            raise ValueError("incident list schema has an unexpected count or array")
        slots = schema.get(f"{array['nested']:#x}", {}).get("fields", [])
        if not 1 <= len(slots) <= MAX_PAYLOAD:
            raise ValueError("incident list capacity is missing or exceeds the parser bound")
    _validate_schema_tree(schema, handle)


def _validate_schema_tree(schema, handle, ancestors=()):
    if handle in ancestors or len(ancestors) >= 8:
        raise ValueError("incident schema is recursive or exceeds the nesting bound")
    fields = schema.get(f"{handle:#x}", {}).get("fields", [])
    if not fields or len(fields) > MAX_PAYLOAD:
        raise ValueError("incident schema has missing or excessive fields")
    for field in fields:
        if field["kind"] == 1:
            if field["arr"] not in (0, 1):
                raise ValueError("unsupported incident array mode")
            _validate_schema_tree(schema, field["nested"], (*ancestors, handle))
        elif (field["kind"] not in (2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0x23)
              or (field["kind"] not in (2, 11, 0x23) and not 1 <= field["width"] <= 64)):
            raise ValueError("incident schema contains an unsupported primitive")


def _validate_values(schema, handle, values):
    for field in schema[f"{handle:#x}"]["fields"]:
        if field["kind"] != 1 or field["off"] not in values:
            continue
        value = values[field["off"]]
        if field["arr"] == 1:
            slots = schema[f"{field['nested']:#x}"]["fields"]
            if values.get(field["width"]) != len(value):
                raise ValueError("incident count exceeds schema slot capacity")
            _validate_values(schema, field["nested"], {f["off"]: v for f, v in zip(slots, value)})
        else:
            _validate_values(schema, field["nested"], value)


def decode_incident(schema, handle, payload):
    reader = investment.BitReader(payload)
    values = investment.decode_from(reader, schema, handle)
    _validate_values(schema, handle, values)
    remainder = len(payload) * 8 - reader.position
    if remainder >= 8 or (remainder and reader.get(remainder)):
        raise ValueError("incident payload has unconsumed data or nonzero padding")
    types = tuple(values[0][4])
    if any(not isinstance(value, int) or value < 0 for value in types):
        raise ValueError("incident type IDs must be nonnegative integers")
    return types, bytes(values[212][4])


class Variants:
    def __init__(self):
        self.counts = Counter()
        self.untracked = 0

    def add(self, body):
        digest = hashlib.sha256(body).hexdigest()
        if digest in self.counts or len(self.counts) < MAX_DIGESTS:
            self.counts[digest] += 1
        else:
            self.untracked += 1

    def report(self):
        return {"distinct": len(self.counts) if not self.untracked else None,
                "distinct_at_least": len(self.counts) + bool(self.untracked),
                "untracked_observations": self.untracked,
                "digests": dict(sorted(self.counts.items()))}


class Group:
    def __init__(self):
        self.count = 0
        self.first = self.last = self.previous = None
        self.interval_count = self.nonmonotonic = 0
        self.interval_total = 0.0
        self.interval_min = self.interval_max = None
        self.lengths = Counter()
        self.bodies = Variants()
        self.after_u32 = Variants()
        self.leading_u32 = Variants()

    def add(self, stamp, instant, body):
        self.count += 1
        if self.first is None:
            self.first = stamp
        self.last = stamp
        if self.previous is not None:
            interval = (instant - self.previous).total_seconds()
            if interval < 0:
                self.nonmonotonic += 1
            else:
                self.interval_count += 1
                self.interval_total += interval
                self.interval_min = interval if self.interval_min is None else min(self.interval_min, interval)
                self.interval_max = interval if self.interval_max is None else max(self.interval_max, interval)
        self.previous = instant
        self.lengths[len(body)] += 1
        self.bodies.add(body)
        if len(body) >= 4:
            # This comparison is useful when only a leading counter changes.
            # Its meaning has not been established for every incident family.
            self.after_u32.add(body[4:])
            self.leading_u32.add(body[:4])

    def report(self):
        return {"messages": self.count, "first": self.first, "last": self.last,
                "interval_seconds": {"samples": self.interval_count,
                                     "min": self.interval_min, "max": self.interval_max,
                                     "mean": self.interval_total / self.interval_count if self.interval_count else None,
                                     "nonmonotonic": self.nonmonotonic},
                "body_lengths": dict(sorted(self.lengths.items())),
                "body_variants": self.bodies.report(),
                "body_after_leading_u32_variants": self.after_u32.report(),
                "leading_u32_variants": self.leading_u32.report()}


def summarize(stream, schema, handle=INCIDENT_HANDLE):
    validate_schema(schema, handle)
    counters = Counter(lines=0, oversized_lines=0, incident_messages=0,
                       decoded_messages=0, malformed_incidents=0,
                       untracked_type_messages=0, untracked_chain_messages=0)
    ah_counts, ah_first, ah_last = Counter(), {}, {}
    occurrences, types, chains = Counter(), {}, {}
    all_incidents = Group()
    previous_clock, clock_day = None, 0
    for raw in bounded_lines(stream):
        counters["lines"] += 1
        if raw is None:
            counters["oversized_lines"] += 1
            continue
        match = CLIENT.match(raw.rstrip("\r\n"))
        if not match:
            continue
        stamp, _, ah_type, declared_length, text = match.groups()
        ah_type = int(ah_type)
        ah_counts[ah_type] += 1
        ah_first.setdefault(ah_type, stamp)
        ah_last[ah_type] = stamp
        if ah_type != 18:
            continue
        counters["incident_messages"] += 1
        try:
            length = int(declared_length)
            if length > MAX_PAYLOAD or len(text) != length * 2 or not re.fullmatch(r"[0-9a-fA-F]*", text):
                raise ValueError("invalid or oversized incident hex body")
            try:
                instant = dt.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
            except ValueError:
                # Time-only log stamps need a date for midnight intervals.
                clock = dt.time.fromisoformat(stamp)
                if previous_clock is not None and clock < previous_clock:
                    if (dt.datetime.combine(dt.date.min, previous_clock)
                            - dt.datetime.combine(dt.date.min, clock)).total_seconds() > 43200:
                        clock_day += 1
                previous_clock = clock
                instant = dt.datetime.combine(dt.date(2000, 1, 1) + dt.timedelta(days=clock_day), clock)
            if instant.tzinfo is not None:
                instant = instant.astimezone(dt.timezone.utc).replace(tzinfo=None)
            chain, body = decode_incident(schema, handle, bytes.fromhex(text))
        except (ValueError, KeyError, TypeError, OverflowError):
            counters["malformed_incidents"] += 1
            continue
        counters["decoded_messages"] += 1
        all_incidents.add(stamp, instant, body)
        chain_counts = Counter(chain)
        untracked_type = False
        for type_id, number in chain_counts.items():
            if type_id not in types and len(types) >= MAX_TYPES:
                untracked_type = True
                continue
            types.setdefault(type_id, Group()).add(stamp, instant, body)
            occurrences[type_id] += number
        counters["untracked_type_messages"] += untracked_type
        if chain not in chains and len(chains) >= MAX_CHAINS:
            counters["untracked_chain_messages"] += 1
        else:
            chains.setdefault(chain, Group()).add(stamp, instant, body)
    return {"schema_handle": f"{handle:#x}", "counters": dict(counters),
            "ah_message_counts": dict(sorted(ah_counts.items())),
            "ah_messages": [{"type": type_id, "messages": count,
                             "first": ah_first[type_id], "last": ah_last[type_id]}
                            for type_id, count in sorted(ah_counts.items())],
            "all_incidents": all_incidents.report(),
            "types": [{"type": type_id, "occurrences": occurrences[type_id], **group.report()}
                      for type_id, group in sorted(types.items())],
            "chains": [{"types": list(chain), **group.report()} for chain, group in sorted(chains.items())],
            "limits": {"line_chars": MAX_LINE, "payload_bytes": MAX_PAYLOAD,
                       "types": MAX_TYPES, "chains": MAX_CHAINS, "digests_per_group": MAX_DIGESTS},
            "interpretation": "Repeated IDs or bodies do not prove retransmission, acknowledgement, or disconnect cause."}


def private_output(destination, repo=None):
    repo = (repo or Path(__file__).resolve().parents[1]).resolve()
    destination = Path(destination).resolve()
    if destination.is_relative_to(repo) and not destination.is_relative_to(repo / "local"):
        raise ValueError("incident report output must be outside the tracked repository or under ignored local/")
    return destination


def render(report, limit=20, out=None):
    out = out or sys.stdout
    counters = report["counters"]
    print(f"Incidents: {counters['decoded_messages']} decoded / {counters['incident_messages']} messages; "
          f"{counters['malformed_incidents']} malformed, {counters['oversized_lines']} oversized lines", file=out)
    print("Incident ID: messages / occurrences; body lengths; full-body variants; first .. last", file=out)
    for row in report["types"][:limit]:
        variants = row["body_variants"]
        distinct = variants["distinct"] if variants["distinct"] is not None else f">={variants['distinct_at_least']}"
        print(f"  {row['type']}: {row['messages']} / {row['occurrences']}; "
              f"{row['body_lengths']}; {distinct}; {row['first']} .. {row['last']}", file=out)
    print("Type chains: messages; interval mean/min/max in seconds", file=out)
    for row in report["chains"][:limit]:
        interval = row["interval_seconds"]
        print(f"  {row['types']}: {row['messages']}; "
              f"{interval['mean']} / {interval['min']} / {interval['max']}", file=out)
    if len(report["types"]) > limit or len(report["chains"]) > limit:
        print(f"Display limited to {limit} types/chains; --out preserves all bounded groups.", file=out)
    if counters["untracked_type_messages"] or counters["untracked_chain_messages"]:
        print("Group capacity reached; see counters for untracked observations.", file=out)
    print(report["interpretation"], file=out)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--schema", required=True, type=Path)
    parser.add_argument("--handle", default=INCIDENT_HANDLE, type=lambda value: int(value, 0))
    parser.add_argument("--out", type=Path, help="write bounded JSON metadata privately")
    parser.add_argument("--limit", type=int, default=20, help="maximum displayed types and chains (1..100)")
    args = parser.parse_args(argv)
    if not 1 <= args.limit <= 100:
        parser.error("--limit must be between 1 and 100")
    try:
        destination = private_output(args.out) if args.out else None
        schema = investment.load_schema(args.schema)
        with args.log.open(encoding="utf-8", errors="replace") as stream:
            report = summarize(stream, schema, args.handle)
        if destination:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        render(report, args.limit)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        parser.error(str(exc))


if __name__ == "__main__":
    main()
