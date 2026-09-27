"""List the replicated objects of the running local RPCS3 test instance (read-only guest memory
reads through rpcs3_guest_read.py) in the `--ah-baselines` format of server/bap_server.py.

The client applies received object state only once every replication record of the active
bubbles has had a baseline (0x1ce1b8 checks record +0x15), so the activity-host stand-in must
send one for each record. Records (0x48 bytes) live in the simulation's record pool
(pool header +8 data, +0x14 count): +0 object id (type 0x808004b1: u32 entity group key, u8 kind,
u16 index), +8 state B type (-1 = none), +0xc state A type, +0x15 awaiting baseline, +0x3c bubble
(-1 = always). The simulation pointer is the first argument of 0x1ce4a4 (host type-5 packets);
it is found here through the host session's simulation object (SIM argument, default: the
TEST184 address).

Usage: python scripts/rpcs3_replicated_objects.py [SIM] > baselines.txt
Output: KEY/KIND/INDEX[/1],... on one line (/1 = the record has a state B type), plus a summary
on stderr.

Optional --json-out PATH writes one record per object with state A/B type handles and bubble;
--schema-out PATH exports the live type trees for every distinct state type. These outputs
help identify which default baselines need real replicated state to drive the mission.
"""

import argparse
import json
import struct
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
_argv, sys.argv = sys.argv, sys.argv[:1]
import rpcs3_guest_read as g  # noqa: E402
sys.argv = _argv

B = g.find_base()
RECORD = 0x48


def u32(a):
    return struct.unpack(">I", g.rpm(B + a, 4))[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("sim", nargs="?", type=lambda v: int(v, 0), default=0x70225618)
    ap.add_argument("--json-out", type=Path, help="write the replication record metadata as JSON")
    ap.add_argument("--schema-out", type=Path, help="export the live state A/B type trees as JSON")
    args = ap.parse_args()
    sim = args.sim
    pool = u32(sim)
    header = g.rpm(B + pool, 0x20)
    data, = struct.unpack(">I", header[8:12])
    count, = struct.unpack(">I", header[0x14:0x18])
    blob = g.rpm(B + data, RECORD * count)
    entries, summary, records, types = [], Counter(), [], set()
    for i in range(count):
        r = blob[i * RECORD:(i + 1) * RECORD]
        key, kind, index = struct.unpack(">IBxH", r[:8])
        state_b, = struct.unpack(">I", r[8:12])
        state_a, = struct.unpack(">I", r[0xC:0x10])
        bubble, = struct.unpack(">i", r[0x3C:0x40])
        has_b = state_b != 0xFFFFFFFF
        entries.append(f"{key:#x}/{kind:#x}/{index}" + ("/1" if has_b else ""))
        summary[(f"{key:#010x}", kind, bubble, r[0x15])] += 1
        records.append(dict(record=f"{data + i * RECORD:#x}", group=f"{key:#x}",
                            kind=kind, index=index, bubble=bubble, awaiting_baseline=r[0x15],
                            state_a=f"{state_a:#x}", state_b=f"{state_b:#x}" if has_b else None))
        types.add(state_a)
        if has_b:
            types.add(state_b)
    print(",".join(entries))
    for (key, kind, bubble, pending), n in sorted(summary.items()):
        print(f"{n:4} group {key} kind {kind:#x} bubble {bubble} awaiting {pending}", file=sys.stderr)
    if args.json_out:
        args.json_out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    if args.schema_out:
        import rpcs3_tiger_types as tiger
        valid_types = sorted(types - {0, 0xFFFFFFFF})
        args.schema_out.write_text(json.dumps(tiger.schema(valid_types), indent=2) + "\n",
                                   encoding="utf-8")


if __name__ == "__main__":
    main()
