"""Summarize bounded D1FPBIND diagnostics; snapshots and output stay private."""
import argparse
from collections import Counter
import json
from pathlib import Path
import re

NUMERIC = {"offset", "location", "address", "start", "length", "constants", "ctrl", "color", "mrt",
           "varyings", "alpha", "a2c", "stipple", "texture_kill", "depth_test", "depth_write", "stencil", "synthetic"}
STATE = ("ctrl", "color", "mrt", "varyings", "alpha", "a2c", "stipple", "texture_kill", "depth_test", "depth_write", "stencil", "synthetic")


def summarize(lines):
    rows = []
    malformed = 0
    for line in lines:
        if "D1FPBIND " not in line:
            continue
        fields = dict(re.findall(r"(\w+)=([^\s]+)", line.split("D1FPBIND ", 1)[1]))
        try:
            for key in NUMERIC & fields.keys():
                fields[key] = int(fields[key], 0)
            if not {"offset", "location", "start", "length", "constants", "before", "prefix", "code"} <= fields.keys():
                raise ValueError("Incomplete diagnostic")
            fields["changed_during_parse"] = fields["before"] != fields["prefix"][:32]
            fields["known_float_prefix"] = fields["code"].startswith("3f99999a3ee666673f7f18b9")
            fields["time"] = line.split(" {", 1)[0].strip()
            rows.append(fields)
        except ValueError:
            malformed += 1
    zero = [r for r in rows if r["offset"] == 0 and r["location"] == 0]
    states = Counter(tuple(r.get(k) for k in STATE) for r in zero)
    return {"records": len(rows), "incomplete_records": malformed, "zero_local": len(zero),
            "known_float_records": sum(r["known_float_prefix"] for r in rows),
            "changed_during_parse": sum(r["changed_during_parse"] for r in rows),
            "zero_local_states": [{**dict(zip(STATE, key)), "count": value} for key, value in states.items()],
            "bindings": rows}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--log", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    with args.log.open(encoding="utf-8", errors="replace") as source:
        result = summarize(source)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "bindings"}, indent=2))


if __name__ == "__main__":
    main()
