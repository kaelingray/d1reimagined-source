"""Install extracted BLES01857 01.24 files into an isolated RPCS3 copy.

The source package set comes from ps3_update.py --packages --inventory-file.
Every required file is validated against the inventory before the destination is
changed. Existing matching-size files can be reused after an interrupted copy.
"""

import argparse
import hashlib
import json
import shutil
from pathlib import Path


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--inventory", required=True, type=Path)
    ap.add_argument("--extracted", required=True, type=Path)
    ap.add_argument("--rpcs3", required=True, type=Path)
    args = ap.parse_args()

    inventory = json.loads(args.inventory.read_text(encoding="utf-8"))
    if inventory["title"] != "BLES01857":
        ap.error("expected the BLES01857 inventory")
    required = {name: meta for name, meta in inventory["packages"].items()
                if name not in inventory["stale"]}
    source_packages = args.extracted / "final/packages"
    prefix = "USRDIR/a0000001/packages/"
    for name, meta in required.items():
        if meta["path"] != prefix + name:
            ap.error(f"unexpected update package path: {meta['path']}")
        source = source_packages / name
        if not source.is_file() or source.stat().st_size != meta["size"]:
            ap.error(f"missing or wrong-sized source package: {name}")
    update_dir = args.extracted / "01.24"
    for relative in ("PARAM.SFO", "USRDIR/EBOOT.BIN"):
        if not (update_dir / relative).is_file():
            ap.error(f"missing {relative} in the extracted 01.24 update")
    eboot_digest = hashlib.sha256((update_dir / "USRDIR/EBOOT.BIN").read_bytes()).hexdigest()
    if eboot_digest != "cd612e9b448e28e05c06d17ecc715996722fe6eea31465080a3ba1b21cc67a8c":
        ap.error("01.24 EBOOT.BIN hash does not match the verified update")
    if not (args.rpcs3 / "rpcs3.exe").is_file():
        ap.error("destination is not an RPCS3 copy")

    installed = args.rpcs3 / "dev_hdd0/game/BLES01857"
    count = 0
    for name, meta in sorted(required.items()):
        source = source_packages / name
        target = installed / meta["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.is_file() or target.stat().st_size != meta["size"]:
            temporary = target.with_name(target.name + ".part")
            shutil.copyfile(source, temporary)
            temporary.replace(target)
        count += 1
        if count % 100 == 0 or count == len(required):
            print(f"installed {count}/{len(required)} update packages", flush=True)

    # Make the new executable and version visible only after the package copy.
    for relative in ("USRDIR/EBOOT.BIN", "PARAM.SFO"):
        source = update_dir / relative
        target = installed / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.with_name(target.name + ".part")
        shutil.copyfile(source, temporary)
        temporary.replace(target)
    print(f"installed BLES01857 01.24 in {installed}")


if __name__ == "__main__":
    main()
