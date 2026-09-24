"""Make an isolated RPCS3 copy for a local sign-on test.

Copies a baseline RPCS3 folder (never modified) to a new folder, then:
  * sets Net -> "DNS address" so every guest DNS lookup goes to dns_sinkhole.py;
  * with --log sys_net=Trace (repeatable), sets per-channel RPCS3 log levels;
  * with --install-ca, replaces dev_flash/data/cert/CAnn.cer files with the local test CA
    (all of them, or only --ca-slots). Destiny loads CA01-CA05 only. The originals are
    kept in <dest>/dev_flash_cert_original.

The copy's rpcn.yml is left as-is (it should already point at the loopback RPCN).
"""

import argparse
import ipaddress
import re
import shutil
import sys
from pathlib import Path


def set_yaml_value(text, key, value):
    pattern = re.compile(rf"^(\s*{re.escape(key)}:).*$", re.M)
    if not pattern.search(text):
        raise KeyError(f"{key!r} not found in config.yml")
    return pattern.sub(lambda m: f"{m.group(1)} {value}", text, count=1)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("source", type=Path)
    ap.add_argument("dest", type=Path)
    ap.add_argument("--dns", default="127.0.0.1")
    ap.add_argument("--log", action="append", default=[], metavar="CHANNEL=LEVEL",
                    help="RPCS3 log channel level, e.g. sys_net=Trace (repeatable)")
    ap.add_argument("--install-ca", type=Path, help="PEM CA certificate to install over the firmware roots")
    ap.add_argument("--ca-slots", default="",
                    help="comma-separated root files to replace, e.g. CA01,CA02 (default: all CAnn.cer)")
    args = ap.parse_args()

    if not ipaddress.ip_address(args.dns).is_loopback:
        sys.exit("DNS address must be loopback so the game cannot resolve live hosts")
    if args.dest.exists():
        sys.exit(f"{args.dest} already exists; use a new folder per test")
    if not (args.source / "rpcs3.exe").exists():
        sys.exit(f"{args.source} does not look like an RPCS3 folder")

    shutil.copytree(args.source, args.dest)
    cfg = args.dest / "config" / "config.yml"
    text = cfg.read_text(encoding="utf-8")
    text = set_yaml_value(text, "DNS address", args.dns)
    text = set_yaml_value(text, "IP swap list", '""')
    if args.log:
        empty = "Log:\n  {}\n"
        if empty not in text:
            sys.exit("baseline config.yml already has log channel overrides; edit it by hand")
        channels = "".join(f"  {c.partition('=')[0]}: {c.partition('=')[2]}\n" for c in args.log)
        text = text.replace(empty, "Log:\n" + channels, 1)
    cfg.write_text(text, encoding="utf-8")
    print(f"copied {args.source} -> {args.dest}; DNS address = {args.dns}")

    if args.install_ca:
        # Firmware roots use LF line endings; keep installed files in the same form.
        pem = args.install_ca.read_bytes().replace(b"\r\n", b"\n")
        if b"BEGIN CERTIFICATE" not in pem:
            sys.exit("--install-ca must be a PEM certificate")
        cert_dir = args.dest / "dev_flash" / "data" / "cert"
        backup = args.dest / "dev_flash_cert_original"
        shutil.copytree(cert_dir, backup)
        slots = [x.strip() for x in args.ca_slots.split(",") if x.strip()]
        roots = [cert_dir / f"{x}.cer" for x in slots] if slots else sorted(cert_dir.glob("CA*.cer"))
        missing = [r.name for r in roots if not r.exists()]
        if missing:
            sys.exit(f"missing firmware roots: {missing}")
        for root in roots:
            root.write_bytes(pem)
        print(f"replaced {len(roots)} firmware roots with {args.install_ca}; originals in {backup}")


if __name__ == "__main__":
    main()
