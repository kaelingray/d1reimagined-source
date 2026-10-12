#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""zeek-logs.py - pcap -> structured, DIFFABLE Zeek logs, in a container, with controls.

WHY THIS EXISTS
---------------
`historicaldocs/design/observability-tooling.md` §3 makes the argument in one line:

    "Diffing two Zeek log sets between runs is a real answer to 'what changed.'
     Diffing two pcaps is not."

Run-A-vs-run-B comparison is this project's scientific instrument, so the substrate has to be
structured logs and the pcap stays the raw artifact underneath.  That is the whole job of this
tool: turn a capture into `conn.log` / `dns.log` / `ssl.log` / `x509.log`, and make two of those
log sets comparable in one command.

⛔ SCOPE BOUNDARY - settled in `design/format-description.md`, do not re-open
---------------------------------------------------------------------------
**Zeek is for STANDARD protocols only.  No Spicy grammar, no D1-specific analyzer, ever.**
  * Spicy's unique value is D1-specific analyzers inside Zeek - we are explicitly not doing that.
  * What we want here needs *zero* Spicy; these four analyzers ship with Zeek.
  * Spicy emits C++; using it in the Rust server means FFI across our most safety-critical
    boundary.
  * D1-specific parsing belongs in a Rust `pcap -> JSONL` tool that **links the same crate the
    server uses** - because if the analyser and the server use different parsers, a mismatch is
    a *silent bug in our understanding*, not a parse error.
If you find yourself wanting to describe a D1 message format here: stop.  Wrong tool.

WHO OWNS WHICH QUESTION  (this tool vs `pcap-summary.py`)
--------------------------------------------------------
    pcap-summary.py   "is our MODEL of the client right?"    - diffs a capture against the host
                      list compiled into the 1.29 eboot: seen-but-not-in-eboot / expected-and-
                      seen / compiled-but-never-queried.  One capture, judged against static
                      knowledge.  Owns hostnames-vs-eboot, and only that.

    zeek-logs.py      "what CHANGED between run A and run B?"  - turns captures into typed logs
                      and diffs two log sets.  Owns connection structure: endpoints, ports,
                      transport, service, first-contact ORDER, TLS parameters, certificates.
                      Knows nothing about the eboot and must not learn.

They overlap on "which hostnames appeared" and that is fine - the overlap is a cross-check, not
a duplication, because the two derive it from different places (tshark fields vs Zeek's DNS
analyzer).  If they ever disagree, that disagreement is a finding.

⭐ THE CONTROL, AND THE TRAP IT EXISTS TO CATCH
----------------------------------------------
A pipeline that silently emits empty logs is the exact failure mode this project keeps hitting,
and Zeek has a spectacular instance of it: **by default Zeek DISCARDS every packet with a bad
checksum**, and NIC checksum offloading means a locally-taken capture is *full* of bad
checksums.  Measured here on the control pcap:

    zeek -r control.pcap      ->  conn.log, weird.log, reporter.log ... and NO dns.log, NO
                                  ssl.log, NO x509.log.  Exit status 0.  Looks like a clean run.
    zeek -C -r control.pcap   ->  all four logs, fully populated.

So this tool passes `-C` by default and asserts *both directions* as controls: with `-C` the
four logs are present and correct, and without it they are absent.  The negative control is what
makes the positive one mean something.  See `capture-rig/zeek-control-capture.sh` for what the
control pcap contains and why it is TLS 1.2 (1.3 encrypts the certificate, so a 1.3 control
would "pass" with a permanently dead x509 path).

USAGE
-----
    capture-rig/zeek-logs.py --self-test                 # controls only; run this first
    capture-rig/zeek-logs.py captures/<run>/x.pcapng     # analyse a capture
    capture-rig/zeek-logs.py x.pcapng -o /tmp/runA --json
    capture-rig/zeek-logs.py --diff /tmp/runA /tmp/runB  # ⭐ the instrument
    capture-rig/zeek-logs.py --pull                      # (re)fetch the pinned image

Exit status 2 means a control FAILED (or the run errored) and the output must not be cited.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import socket
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parent.parent
RIG = REPO / "capture-rig"
CONTROL_CAPTURE = RIG / "zeek-control-capture.sh"

# The image is PINNED by digest, exactly like the eboot's sha256, and for the same reason: a
# silently different analyser is a silently different answer.  `--pull` re-fetches the tag; if
# upstream has moved, the digest control FAILS and you update this constant deliberately.
IMAGE_REPO = "docker.io/zeek/zeek"
IMAGE_TAG = "latest"
IMAGE_DIGEST = "sha256:6ce464c9cc63185ac0ef40b86f1150842350895496520d7e6522c1756d07c755"
IMAGE_ZEEK_VERSION = "8.2.1"  # what that digest reports; asserted as a control
IMAGE_REF = f"{IMAGE_REPO}@{IMAGE_DIGEST}"

# Zeek is BSD-3-Clause (see design/licensing.md - compatible with our GPL-2.0-or-later, and it
# is a separate batch process besides).  The image also ships Spicy; we do not use it. See the
# scope boundary above.

# The name every control assertion is keyed on. A real D1 signon host, used here as a literal.
CONTROL_NAME = "www4.signon.deadorbit.net"

# Logs we consider load-bearing. Anything else Zeek writes is kept but not asserted.
CORE_LOGS = ["conn.log", "dns.log", "ssl.log", "x509.log"]


class ZeekError(RuntimeError):
    pass


# ==========================================================================================
# container plumbing
# ==========================================================================================
def need(tool: str, hint: str) -> str:
    path = shutil.which(tool)
    if path is None:
        raise ZeekError(f"{tool} is not installed ({hint})")
    return path


def image_present() -> bool:
    done = subprocess.run(
        [need("podman", "dnf install podman"), "image", "exists", IMAGE_REF],
        capture_output=True, text=True,
    )
    return done.returncode == 0


def pull_image() -> str:
    podman = need("podman", "dnf install podman")
    print(f"pulling {IMAGE_REPO}:{IMAGE_TAG} ...", file=sys.stderr)
    done = subprocess.run([podman, "pull", f"{IMAGE_REPO}:{IMAGE_TAG}"],
                          capture_output=True, text=True)
    if done.returncode != 0:
        raise ZeekError(done.stderr.strip() or "podman pull failed")
    digest = subprocess.run(
        [podman, "image", "inspect", f"{IMAGE_REPO}:{IMAGE_TAG}", "--format", "{{.Digest}}"],
        capture_output=True, text=True,
    ).stdout.strip()
    if digest != IMAGE_DIGEST:
        print(f"\n⚠ pulled digest {digest}\n  does not match the pin in {__file__}:"
              f"\n  {IMAGE_DIGEST}\n  Update IMAGE_DIGEST/IMAGE_ZEEK_VERSION deliberately, then "
              f"re-run --self-test.\n", file=sys.stderr)
    return digest


def zeek_version() -> str:
    done = run_in_image(["zeek", "--version"], mounts=[])
    return done.stdout.strip().splitlines()[0] if done.stdout.strip() else ""


def run_in_image(argv: list[str], mounts: list[str], workdir: str | None = None
                 ) -> subprocess.CompletedProcess:
    """One place that knows how we invoke the container.

    `--network=none`  the batch step has no business touching the network, and this proves it.
    `--security-opt label=disable`  the honest trade: the alternative (`:z`) would REWRITE the
        SELinux label of the operator's capture files on disk.  We refuse to mutate an artifact
        in order to read it.  The container is still rootless-userns-confined, the pcap is
        mounted read-only, and there is no network.
    """
    command = [need("podman", "dnf install podman"), "run", "--rm",
               "--network=none", "--security-opt", "label=disable"]
    for mount in mounts:
        command += ["-v", mount]
    if workdir:
        command += ["-w", workdir]
    command += [IMAGE_REF] + argv
    return subprocess.run(command, capture_output=True, text=True)


def run_zeek(pcap: pathlib.Path, outdir: pathlib.Path, *, checksums: bool,
             json_logs: bool = True, extra: list[str] | None = None) -> dict:
    """Batch one pcap into `outdir`. Returns provenance, raises on hard failure."""
    pcap = pcap.resolve()
    if not pcap.exists():
        raise ZeekError(f"{pcap} not found")
    if pcap.stat().st_size == 0:
        raise ZeekError(f"{pcap} is empty")
    outdir.mkdir(parents=True, exist_ok=True)

    argv = ["zeek"]
    if not checksums:
        argv.append("-C")          # ⭐ see the module docstring: without this, silence
    argv += ["-r", "/input/capture.pcap"]
    if json_logs:
        argv.append("LogAscii::use_json=T")
    argv += extra or []

    done = run_in_image(
        argv,
        mounts=[f"{pcap}:/input/capture.pcap:ro", f"{outdir.resolve()}:/work"],
        workdir="/work",
    )
    if done.returncode != 0:
        raise ZeekError(f"zeek exited {done.returncode}: {done.stderr.strip()[:400]}")

    (outdir / "ZEEK-INFO.txt").write_text(
        "\n".join([
            f"pcap:          {pcap}",
            f"pcap_bytes:    {pcap.stat().st_size}",
            f"image:         {IMAGE_REF}",
            f"zeek_version:  {IMAGE_ZEEK_VERSION} (pinned)",
            f"argv:          {' '.join(argv)}",
            f"checksums:     {'validated' if checksums else 'IGNORED (-C)'}",
            "",
        ])
    )
    return {"argv": argv, "stderr": done.stderr.strip(),
            "logs": sorted(p.name for p in outdir.glob("*.log"))}


# ==========================================================================================
# reading Zeek's output
# ==========================================================================================
def read_log(outdir: pathlib.Path, name: str) -> list[dict]:
    """Read a JSON-lines Zeek log. Missing file -> [] (the caller decides if that is a failure)."""
    path = outdir / name
    if not path.exists():
        return []
    rows = []
    for line in path.read_text(errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue          # a TSV log: not our format, skip rather than pretend
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return rows


def endpoint_timeline(conns: list[dict]) -> list[dict]:
    """⭐ The Activity-Host question, in table form.

    Predicted shape (client RTTI: `client_to_bap_get_activity_host_proxy_request`,
    `bap_to_activity_host_socket_handoff_request`): BAP connection exists -> client asks BAP for
    a host -> **a second connection appears to a DIFFERENT endpoint**.  So the thing to look at
    is not traffic volume, it is *which responder endpoint appeared, and when*.
    """
    if not conns:
        return []
    base = min(float(c.get("ts", 0)) for c in conns)
    first: dict[tuple, dict] = {}
    for conn in sorted(conns, key=lambda c: float(c.get("ts", 0))):
        key = (conn.get("proto"), conn.get("id.resp_h"), conn.get("id.resp_p"))
        entry = first.setdefault(key, {
            "proto": conn.get("proto"), "resp_h": conn.get("id.resp_h"),
            "resp_p": conn.get("id.resp_p"), "service": conn.get("service") or "",
            "first_seen": float(conn.get("ts", 0)) - base, "flows": 0,
            "orig_bytes": 0, "resp_bytes": 0, "pkts": 0,
        })
        entry["flows"] += 1
        entry["orig_bytes"] += int(conn.get("orig_bytes") or 0)
        entry["resp_bytes"] += int(conn.get("resp_bytes") or 0)
        entry["pkts"] += int(conn.get("orig_pkts") or 0) + int(conn.get("resp_pkts") or 0)
        if not entry["service"] and conn.get("service"):
            entry["service"] = conn["service"]
    return sorted(first.values(), key=lambda e: e["first_seen"])


def add_rdns(rows: list[dict]) -> None:
    """OPT-IN, and it TOUCHES THE NETWORK - one PTR lookup per distinct responder address.

    Included because it is the cheapest available signal on the question that decides whether we
    are writing a server or a peer: is the Activity Host a **datacentre** address or a
    **residential** one?  A PTR like `...bungie...`/`...amazonaws...` and one like
    `...dyn.comcast.net` answer that in one glance.  Never automatic; the operator asks for it.
    """
    cache: dict[str, str] = {}
    socket.setdefaulttimeout(2.0)
    for row in rows:
        address = row.get("resp_h")
        if not address:
            continue
        if address not in cache:
            try:
                cache[address] = socket.gethostbyaddr(address)[0]
            except OSError:
                cache[address] = ""
        row["rdns"] = cache[address]


def collect(outdir: pathlib.Path) -> dict:
    conns = read_log(outdir, "conn.log")
    dns = read_log(outdir, "dns.log")
    ssl = read_log(outdir, "ssl.log")
    x509 = read_log(outdir, "x509.log")
    return {
        "counts": {name: len(read_log(outdir, name)) for name in CORE_LOGS},
        "other_logs": sorted(p.name for p in outdir.glob("*.log") if p.name not in CORE_LOGS),
        "endpoints": endpoint_timeline(conns),
        "dns": [{"query": d.get("query"), "qtype": d.get("qtype_name"),
                 "rcode": d.get("rcode_name"), "answers": d.get("answers") or []}
                for d in dns],
        "ssl": [{"server_name": s.get("server_name"), "version": s.get("version"),
                 "cipher": s.get("cipher"), "established": s.get("established"),
                 "resp_h": s.get("id.resp_h"), "resp_p": s.get("id.resp_p")}
                for s in ssl],
        "x509": [{"subject": c.get("certificate.subject"), "issuer": c.get("certificate.issuer"),
                  "san": c.get("san.dns") or []} for c in x509],
        "weird": len(read_log(outdir, "weird.log")),
        "notice": len(read_log(outdir, "notice.log")),
    }


# ==========================================================================================
# controls  (ATLAS §4.2 / §4.4d - mandatory, printed, and fatal)
# ==========================================================================================
def controls(workdir: pathlib.Path) -> list[tuple[str, bool, str]]:
    """Manufacture a pcap whose contents we chose, and assert Zeek finds them.

    Both directions.  The negative control (`without -C the four logs go MISSING`) is not
    decoration: it is the proof that the failure mode we defend against is real and that our
    defence is the thing preventing it.
    """
    out: list[tuple[str, bool, str]] = []

    def c(name: str, ok: bool, detail: str = "") -> None:
        out.append((name, bool(ok), detail))

    # -- the analyser itself ---------------------------------------------------------------
    present = image_present()
    c(f"pinned image present ({IMAGE_DIGEST[:19]}...)", present,
      IMAGE_REF if present else "run --pull")
    if not present:
        return out
    version = zeek_version()
    c(f"image reports zeek {IMAGE_ZEEK_VERSION}", IMAGE_ZEEK_VERSION in version, version)

    # -- the control pcap ------------------------------------------------------------------
    pcap = workdir / "control.pcap"
    if not CONTROL_CAPTURE.exists():
        c("control capture script present", False, str(CONTROL_CAPTURE))
        return out
    made = subprocess.run(["bash", str(CONTROL_CAPTURE), str(pcap)],
                          capture_output=True, text=True)
    size = pcap.stat().st_size if pcap.exists() else 0
    c("control pcap generated in a sealed netns (loopback only, no sudo)",
      made.returncode == 0 and size > 2000,
      f"{size} B" if size else made.stderr.strip()[:200])
    if made.returncode != 0 or size <= 2000:
        return out

    # -- ⭐ THE POSITIVE CONTROL: with -C, all four logs, correct ---------------------------
    good = workdir / "with-C"
    try:
        run_zeek(pcap, good, checksums=False)
    except ZeekError as error:
        c("zeek -C ran", False, str(error))
        return out
    c("zeek -C ran", True, "")

    conns = read_log(good, "conn.log")
    dns = read_log(good, "dns.log")
    ssl = read_log(good, "ssl.log")
    x509 = read_log(good, "x509.log")

    udp53 = [x for x in conns if x.get("proto") == "udp" and x.get("id.resp_p") == 53]
    tcp443 = [x for x in conns if x.get("proto") == "tcp" and x.get("id.resp_p") == 443]
    c("⭐ conn.log has the udp/53 flow, service=dns",
      bool(udp53) and udp53[0].get("service") == "dns",
      f"{len(udp53)} flow(s), service={udp53[0].get('service') if udp53 else '-'}")
    c("⭐ conn.log has the tcp/443 flow, service=ssl",
      bool(tcp443) and tcp443[0].get("service") == "ssl",
      f"{len(tcp443)} flow(s), service={tcp443[0].get('service') if tcp443 else '-'}")
    c(f"⭐ dns.log has the A query for {CONTROL_NAME}",
      any(d.get("query") == CONTROL_NAME and d.get("qtype_name") == "A" for d in dns),
      f"{len(dns)} record(s)")
    c("...and it carries the answer we served (127.0.0.1)",
      any("127.0.0.1" in (d.get("answers") or []) for d in dns),
      str(next((d.get("answers") for d in dns), None)))
    c(f"⭐ ssl.log has SNI {CONTROL_NAME}, TLSv12, established",
      any(s.get("server_name") == CONTROL_NAME and s.get("version") == "TLSv12"
          and s.get("established") for s in ssl),
      str(next((s.get("cipher") for s in ssl), None)))
    c("⭐ x509.log has the certificate subject (proves the cert path is live)",
      any(CONTROL_NAME in (c_.get("certificate.subject") or "") for c_ in x509),
      next((c_.get("certificate.subject") for c_ in x509), "MISSING"))
    c("...with the SAN, so a name in a cert is recoverable and not just the SNI",
      any(CONTROL_NAME in (c_.get("san.dns") or []) for c_ in x509), "")
    c("logs are JSON lines (parseable without a Zeek TSV reader)",
      len(conns) >= 2 and len(dns) >= 1, f"conn={len(conns)} dns={len(dns)}")

    # -- ⭐ THE NEGATIVE CONTROL: without -C, the failure mode is REAL ----------------------
    # This is the assertion that earns the -C default. If this control ever starts FAILING
    # because Zeek stopped discarding bad-checksum packets, that is good news and the default
    # can be revisited - but it must be noticed, not assumed.
    bad = workdir / "without-C"
    try:
        run_zeek(pcap, bad, checksums=True)
        silent = not any((bad / name).exists() for name in ("dns.log", "ssl.log", "x509.log"))
        c("⭐ NEGATIVE: without -C, dns/ssl/x509 logs are ABSENT (checksum-offload trap)",
          silent, "silent" if silent else "logs appeared - re-read the -C rationale")
        c("...and conn.log is still written, so the run LOOKS successful",
          (bad / "conn.log").exists(), "exit 0, no error")
    except ZeekError as error:
        c("⭐ NEGATIVE: without -C, dns/ssl/x509 logs are ABSENT", False, str(error))
    return out


# ==========================================================================================
# diff - the reason this tool exists
# ==========================================================================================
def diff_key(kind: str, row: dict) -> str:
    if kind == "endpoints":
        return f"{row['proto']} {row['resp_h']}:{row['resp_p']}" + (
            f" [{row['service']}]" if row.get("service") else "")
    if kind == "dns":
        return f"{row['query']} {row['qtype']}"
    if kind == "ssl":
        return f"{row['server_name']} @ {row['resp_h']}:{row['resp_p']} {row['version']}"
    return str(row.get("subject"))


def diff(a: pathlib.Path, b: pathlib.Path) -> dict:
    da, db = collect(a), collect(b)
    result = {"a": str(a), "b": str(b), "sections": {}}
    for kind in ("endpoints", "dns", "ssl", "x509"):
        ka = {diff_key(kind, r) for r in da[kind]}
        kb = {diff_key(kind, r) for r in db[kind]}
        result["sections"][kind] = {
            "only_a": sorted(ka - kb), "only_b": sorted(kb - ka), "both": sorted(ka & kb),
        }
    result["counts"] = {"a": da["counts"], "b": db["counts"]}
    return result


def report_diff(data: dict) -> None:
    print("=" * 78)
    print("ZEEK LOG DIFF   A =", data["a"])
    print("                B =", data["b"])
    print("=" * 78)
    print(f"\n  record counts   A: {data['counts']['a']}\n                  B: {data['counts']['b']}")
    titles = {
        "endpoints": "⭐ CONNECTION ENDPOINTS - a row under 'only in B' at activity launch is "
                     "the Activity Host",
        "dns": "DNS QUERIES",
        "ssl": "TLS (SNI / endpoint / version)",
        "x509": "CERTIFICATE SUBJECTS",
    }
    for kind, title in titles.items():
        section = data["sections"][kind]
        if not (section["only_a"] or section["only_b"]):
            print(f"\n{title}\n    (identical - {len(section['both'])} shared)")
            continue
        print(f"\n{title}")
        for key in section["only_b"]:
            print(f"    + only in B   {key}")
        for key in section["only_a"]:
            print(f"    - only in A   {key}")
        print(f"      ({len(section['both'])} in both)")


# ==========================================================================================
# report
# ==========================================================================================
def report(data: dict, outdir: pathlib.Path) -> None:
    print("=" * 78)
    print(f"ZEEK LOGS - {outdir}")
    print("=" * 78)
    counts = ", ".join(f"{k}={v}" for k, v in data["counts"].items())
    print(f"    {counts}"
          + (f"   (+{', '.join(data['other_logs'])})" if data["other_logs"] else ""))

    print("\n" + "=" * 78)
    print("⭐ ENDPOINT TIMELINE - first contact per responder endpoint")
    print("   A NEW ROW appearing at activity launch, on a DIFFERENT ip than BAP, is the")
    print("   `bap_to_activity_host_socket_handoff` prediction coming true.")
    print("=" * 78)
    if not data["endpoints"]:
        print("    (no connections - if that is a surprise, re-read the -C note in --help)")
    for entry in data["endpoints"]:
        rdns = f"  {entry['rdns']}" if entry.get("rdns") else ""
        print(f"    {entry['first_seen']:9.3f}s  {entry['proto']:>3}  "
              f"{entry['resp_h']}:{entry['resp_p']:<6} {entry['service'] or '-':<8} "
              f"{entry['flows']:>4} flow  {entry['orig_bytes']:>9}↑ {entry['resp_bytes']:>9}↓"
              f"{rdns}")

    if data["dns"]:
        print("\n" + "=" * 78)
        print("DNS")
        print("=" * 78)
        seen: set[str] = set()
        for row in data["dns"]:
            key = f"{row['query']}/{row['qtype']}"
            if key in seen:
                continue
            seen.add(key)
            answers = ", ".join(row["answers"]) or f"({row['rcode']})"
            print(f"    {row['query']:<44} {row['qtype']:<6} {answers}")

    if data["ssl"]:
        print("\n" + "=" * 78)
        print("TLS - SNI, endpoint, version, cipher")
        print("=" * 78)
        for row in data["ssl"]:
            print(f"    {row['server_name'] or '(no SNI)':<44} "
                  f"{row['resp_h']}:{row['resp_p']:<6} {row['version'] or '?':<8} "
                  f"{'ok' if row['established'] else 'FAILED':<7} {row['cipher'] or ''}")

    if data["x509"]:
        print("\n" + "=" * 78)
        print("CERTIFICATES - who the server said it was")
        print("=" * 78)
        for row in data["x509"]:
            print(f"    subject {row['subject']}")
            print(f"    issuer  {row['issuer']}")
            if row["san"]:
                print(f"    san     {', '.join(row['san'])}")

    if data["weird"] or data["notice"]:
        print(f"\n    weird.log: {data['weird']} · notice.log: {data['notice']}")

    print("\n" + "-" * 78)
    print("This tool owns connection STRUCTURE and run-to-run DIFF. For 'does the live client")
    print("contact hosts our eboot never mentions?' use capture-rig/pcap-summary.py - different")
    print("question, different evidence, deliberately not duplicated here.")
    print("Compare two runs:  zeek-logs.py --diff <dirA> <dirB>")


# ==========================================================================================
def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pcap", nargs="?", type=pathlib.Path)
    parser.add_argument("-o", "--outdir", type=pathlib.Path,
                        help="where logs land (default: <pcap-dir>/zeek-<pcap-stem>/)")
    parser.add_argument("--self-test", action="store_true",
                        help="⭐ generate a control pcap and assert Zeek finds what we put in it")
    parser.add_argument("--diff", nargs=2, metavar=("DIR_A", "DIR_B"), type=pathlib.Path,
                        help="⭐ diff two Zeek log directories")
    parser.add_argument("--pull", action="store_true", help="(re)fetch the pinned image")
    parser.add_argument("--rdns", action="store_true",
                        help="PTR-lookup each responder address - TOUCHES THE NETWORK, opt-in. "
                             "Answers datacentre-vs-residential for the Activity Host.")
    parser.add_argument("--tsv", action="store_true", help="Zeek's native TSV instead of JSON")
    parser.add_argument("--validate-checksums", action="store_true",
                        help="do NOT pass -C. Almost always wrong: see --help. Kept so the "
                             "checksum behaviour can be re-measured rather than believed.")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    try:
        if args.pull:
            digest = pull_image()
            print(f"image digest: {digest}")
            return 0 if digest == IMAGE_DIGEST else 2

        if args.diff:
            for path in args.diff:
                if not path.is_dir():
                    raise ZeekError(f"{path} is not a directory of Zeek logs")
            data = diff(*args.diff)
            print(json.dumps(data, indent=2)) if args.json else report_diff(data)
            return 0

        if args.self_test:
            with tempfile.TemporaryDirectory(prefix="d1-zeek-selftest.") as tmp:
                results = controls(pathlib.Path(tmp))
            for name, ok, detail in results:
                print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  [{detail}]" if detail else ""))
            passed = bool(results) and all(ok for _, ok, _ in results)
            print("\n" + ("ALL CONTROLS PASS - Zeek output from this box may be cited"
                          if passed else
                          "*** CONTROL FAILURE - this pipeline's output must not be cited ***"))
            if args.json:
                print(json.dumps([{"name": n, "passed": o, "detail": d}
                                  for n, o, d in results], indent=2))
            return 0 if passed else 2

        if args.pcap is None:
            parser.print_usage(sys.stderr)
            raise ZeekError("give a pcap, or --self-test / --diff / --pull")

        outdir = args.outdir or args.pcap.resolve().parent / f"zeek-{args.pcap.stem}"
        provenance = run_zeek(args.pcap, outdir, checksums=args.validate_checksums,
                              json_logs=not args.tsv)
        if args.tsv:
            print(f"wrote TSV logs to {outdir}: {', '.join(provenance['logs'])}")
            print("(--tsv output is not parsed by this tool; drop --tsv for the report/diff)")
            return 0
        data = collect(outdir)
        if args.rdns:
            add_rdns(data["endpoints"])
        if args.json:
            print(json.dumps({"outdir": str(outdir), "provenance": provenance, **data}, indent=2))
        else:
            report(data, outdir)
    except ZeekError as error:
        print(f"zeek-logs: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
