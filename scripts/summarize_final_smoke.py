"""Index a local RPCS3 smoke run without copying private captures into Git.

Reads the preserved per-run logs and named screenshots, then writes a JSON index
under local/. Raw requests, account fields, and package contents are not copied.
"""

import argparse
import collections
import hashlib
import json
import re
from pathlib import Path


PACKAGE_OPEN = re.compile(r"sys_fs_open\(path=[“\"]([^”\"]+\.pkg)[”\"]")
STUN_PACKET = re.compile(r"<-\s+\S+\s+len=(\d+)\s+([\da-f ]+)$")
HTTP_REQUEST = re.compile(r"HTTP '((?:GET|POST|PUT|DELETE|HEAD)\s+[^']+)' host='([^']*)'")
DNS_REPLY = re.compile(r"\s+(?:A|AAAA)\s+(\S+)\s+->\s+(\S+)")


def fingerprint(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return {"name": path.name, "bytes": path.stat().st_size, "sha256": digest.hexdigest()}


def summarize(logs_dir, run, screenshots):
    files = {}
    for path in sorted(logs_dir.glob(f"{run}-*")):
        if path.is_file():
            files[path.name.removeprefix(run + "-")] = fingerprint(path)
    result = {"run": run, "files": files, "screenshots": [fingerprint(p) for p in screenshots]}

    rpc = logs_dir / f"{run}-rpcs3.log"
    if rpc.exists():
        package_opens = collections.Counter()
        socket_errors = collections.Counter()
        fatals = []
        with rpc.open("r", encoding="utf-8", errors="replace") as stream:
            for line in stream:
                match = PACKAGE_OPEN.search(line)
                if match:
                    package_opens[match.group(1)] += 1
                if "sys_net: Socket error " in line:
                    socket_errors[line.split("sys_net: Socket error ", 1)[1].strip()] += 1
                if "Access violation" in line or "Emulation has been frozen" in line:
                    fatals.append(line.strip()[:500])
        result["rpcs3"] = {
            "package_open_events": sum(package_opens.values()),
            "package_paths": dict(sorted(package_opens.items())),
            "socket_errors": dict(sorted(socket_errors.items())),
            "fatal_events": fatals[-20:],
        }

    dns = logs_dir / f"{run}-dns-queries.log"
    if dns.exists():
        queries = collections.Counter()
        with dns.open("r", encoding="utf-8", errors="replace") as stream:
            for line in stream:
                match = DNS_REPLY.search(line)
                if match:
                    queries[(match.group(1), match.group(2))] += 1
        result["dns"] = [{"host": host, "answer": answer, "count": count}
                         for (host, answer), count in sorted(queries.items())]

    signon = logs_dir / f"{run}-signon.log"
    if signon.exists():
        requests, replies = [], []
        with signon.open("r", encoding="utf-8", errors="replace") as stream:
            for line in stream:
                match = HTTP_REQUEST.search(line)
                if match:
                    requests.append({"line": match.group(1), "host": match.group(2)})
                if "replied 200 SignOn" in line or "replied 200 content manifest" in line:
                    replies.append(line.strip())
        result["signon"] = {"requests": requests, "replies": replies}

    stun = logs_dir / f"{run}-stun.log"
    if stun.exists():
        packets = collections.Counter()
        with stun.open("r", encoding="utf-8", errors="replace") as stream:
            for line in stream:
                match = STUN_PACKET.search(line)
                if match:
                    packets[(int(match.group(1)), match.group(2).strip())] += 1
        result["stun"] = [{"bytes": size, "hex": payload, "count": count}
                          for (size, payload), count in sorted(packets.items())]

    for service in ("bap", "lobby"):
        path = logs_dir / f"{run}-{service}.log"
        if path.exists():
            with path.open("r", encoding="utf-8", errors="replace") as stream:
                lines = sum(1 for _ in stream)
            result[service] = {"log_lines": lines}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logs-dir", type=Path, required=True)
    parser.add_argument("--run", required=True)
    parser.add_argument("--screenshot", type=Path, action="append", default=[])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    data = summarize(args.logs_dir, args.run, args.screenshot)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out}; {len(data['files'])} logs, "
          f"{data.get('rpcs3', {}).get('package_open_events', 0)} package opens")


if __name__ == "__main__":
    main()
