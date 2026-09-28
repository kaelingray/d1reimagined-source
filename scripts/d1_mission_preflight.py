"""Check mission scripts against a client's exported activity type schema without running it."""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
import bap_server  # noqa: E402
import investment  # noqa: E402


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--schema", required=True, type=Path,
                    help="activity schema from a running client or d1_types.py schema")
    ap.add_argument("missions", nargs="+", type=Path)
    args = ap.parse_args(argv)
    schema = investment.load_schema(args.schema)
    failed = False
    for path in args.missions:
        try:
            mission = json.loads(path.read_text(encoding="utf-8"))
            timed, reactive = bap_server.expand_mission(mission, schema)
            bap_server.validate_mission_states(schema, timed, reactive)
        except (OSError, ValueError, KeyError, TypeError) as exc:
            print(f"FAIL {path}: {exc}", file=sys.stderr)
            failed = True
        else:
            print(f"PASS {path}: {len(timed)} timed, {len(reactive)} reactive state changes")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
