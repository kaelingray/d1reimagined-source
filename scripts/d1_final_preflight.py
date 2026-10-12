"""Read-only checks before the first BLES01857 01.24 run.

The data-only mode checks the saved EBOOT, service table, stand-in payloads and
mission scripts. With --rpcs3-dir and --game-usrdir it also checks
the installed update and the patched RPCS3 copy. It never starts the game.
"""

import argparse
import hashlib
import ipaddress
import json
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "scripts"))
sys.path.insert(0, str(REPO / "server"))
import d1_build_preflight  # noqa: E402
import game_ca_patch  # noqa: E402
from d1_eboot import Eboot  # noqa: E402
from d1_ppu_hash import ppu_hash  # noqa: E402
from ps3_update import param_sfo  # noqa: E402

FINAL_ELF_SHA256 = "e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206"
FINAL_EBOOT_BIN_SHA256 = "cd612e9b448e28e05c06d17ecc715996722fe6eea31465080a3ba1b21cc67a8c"
FINAL_EBOOT_BIN_SIZE = 26183840
CA_ADDR_0100 = 0x011B7218
CA_ADDR_0124 = 0x012D3740
MANIFEST_FLAG_0124 = 0x018DCA64
CA_LENGTH = 2390
PPU_PATTERN = re.compile(r"PPU-[0-9a-fA-F]{40}")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_data(data_root):
    elf = data_root / "ps3_updates/01.24/USRDIR/EBOOT.elf"
    old_elf = data_root / "eboot/EBOOT.elf"
    types = data_root / "ps3_updates/types_01.24.json"
    services = data_root / "ps3_updates/services_01.24.json"
    inventory_file = data_root / "ps3_updates/package_inventory_01.24.json"
    disc_list = data_root / "package_names.txt"
    old_services = data_root / "schema/investment_schema.json"
    for path in (elf, old_elf, types, services, inventory_file, disc_list, old_services):
        require(path.is_file(), f"missing data file: {path}")

    digest = hashlib.sha256(elf.read_bytes()).hexdigest()
    require(digest == FINAL_ELF_SHA256, f"01.24 EBOOT SHA-256 differs: {digest}")
    old, final = Eboot(old_elf), Eboot(elf)
    require(final.toc == 0x1891798, f"01.24 EBOOT has unexpected TOC {final.toc:#x}")
    ca_old, ca_final = old.read(CA_ADDR_0100, CA_LENGTH), final.read(CA_ADDR_0124, CA_LENGTH)
    require(ca_old is not None and ca_final == ca_old, "01.24 embedded CA differs from 01.00")
    require(ca_final.startswith(b"-----BEGIN CERTIFICATE-----"), "CA patch site is not a PEM")
    require(final.read(MANIFEST_FLAG_0124, 4) == b"\x01\x00\x00\x00",
            "01.24 manifest patch site has unexpected bytes")
    require(ppu_hash(old_elf) == game_ca_patch.PPU_HASH,
            "PPU hash algorithm does not reproduce the known 01.00 hash")
    require(ppu_hash(elf) == game_ca_patch.build_sites("01.24")["ppu_hash"],
            "01.24 PPU hash differs from the saved patch identity")
    print("PASS 01.24 EBOOT identity, PPU hash and both RPCS3 patch sites")

    schema = json.loads(types.read_text(encoding="utf-8"))
    mapping = json.loads(services.read_text(encoding="utf-8"))
    earlier = json.loads(old_services.read_text(encoding="utf-8"))["services"]
    current = mapping["services"]
    require(len(current) == 94 and len(mapping["name_hashes"]) == 94,
            "01.24 service map must contain 94 IDs and name hashes")
    require(all(current.get(wire) == pair for wire, pair in earlier.items()),
            "an existing service ID or type handle changed")
    require(all(handle in schema for pair in current.values() for handle in pair),
            "a service type handle is absent from the 01.24 schema")
    print("PASS 94 services, including all 66 stable older IDs and handles")

    inventory = json.loads(inventory_file.read_text(encoding="utf-8"))
    required = {name: meta for name, meta in inventory["packages"].items()
                if name not in inventory["stale"]}
    require(inventory["title"] == "BLES01857" and len(inventory["packages"]) == 1408
            and len(inventory["stale"]) == 374 and len(required) == 1034
            and set(inventory["stale"]) <= set(inventory["packages"])
            and sum(meta["size"] for meta in required.values()) == 10305603584
            and all(meta["path"].endswith("/" + name) for name, meta in required.items()),
            "01.24 package inventory is incomplete")
    print("PASS final update inventory: 1,034 required package files")
    disc_names = [name for name in disc_list.read_text(encoding="utf-8").splitlines() if name]
    require(len(disc_names) == 247 and all(name.endswith(".pkg") for name in disc_names),
            "disc package name list is incomplete")

    missions = [REPO / "missions/a_guardian_rises.json", REPO / "missions/tower_npcs.json"]
    result = d1_build_preflight.main([
        "--activity-schema", str(types), "--investment-schema", str(types),
        "--services-schema", str(services), *(str(path) for path in missions),
    ])
    require(result == 0, "01.24 build preflight failed")
    return inventory, disc_names


def check_packages(installed, inventory):
    stale = set(inventory["stale"])
    missing, wrong_size = [], []
    for name, meta in inventory["packages"].items():
        if name in stale:
            continue
        path = installed / meta["path"]
        if not path.is_file():
            missing.append(name)
        elif path.stat().st_size != meta["size"]:
            wrong_size.append(name)
    require(not missing and not wrong_size,
            f"01.24 update package set incomplete: {len(missing)} missing, "
            f"{len(wrong_size)} wrong-size; examples: {(missing + wrong_size)[:4]}")
    print(f"PASS all {len(inventory['packages']) - len(stale):,} required update packages "
          "present at the expected sizes")


def check_runtime(rpcs3_dir, game_usrdir, ppu_hash, inventory, disc_names, local_root):
    require(PPU_PATTERN.fullmatch(ppu_hash or ""),
            "invalid PPU hash; expected PPU-<40 hex digits>")
    require(ppu_hash == game_ca_patch.build_sites("01.24")["ppu_hash"],
            "selected PPU hash differs from the saved 01.24 decrypted EBOOT")
    require((rpcs3_dir / "rpcs3.exe").is_file(), f"missing rpcs3.exe in {rpcs3_dir}")
    require((game_usrdir / "EBOOT.BIN").is_file(), f"missing disc EBOOT.BIN in {game_usrdir}")
    missing_disc = [name for name in disc_names if not (game_usrdir / "packages" / name).is_file()]
    require(not missing_disc,
            f"disc package folder is incomplete: {len(missing_disc)} missing; examples: {missing_disc[:4]}")
    print(f"PASS all {len(disc_names)} disc package filenames present")
    installed = rpcs3_dir / "dev_hdd0/game/BLES01857"
    sfo = installed / "PARAM.SFO"
    require(sfo.is_file(), f"01.24 update is not installed in this RPCS3 copy: {sfo}")
    info = param_sfo(sfo.read_bytes())
    require(info.get("TITLE_ID") == "BLES01857" and info.get("APP_VER") == "01.24",
            f"installed update is not BLES01857 01.24: {info.get('TITLE_ID')} {info.get('APP_VER')}")
    installed_eboot = installed / "USRDIR/EBOOT.BIN"
    require(installed_eboot.is_file(), "installed 01.24 update has no EBOOT.BIN")
    require(installed_eboot.stat().st_size == FINAL_EBOOT_BIN_SIZE and
            hashlib.sha256(installed_eboot.read_bytes()).hexdigest() == FINAL_EBOOT_BIN_SHA256,
            "installed update EBOOT.BIN differs from Sony's BLES01857 01.24 file")
    print("PASS Sony's exact 01.24 update EBOOT installed in selected RPCS3 copy; disc EBOOT present")
    check_packages(installed, inventory)

    patch = rpcs3_dir / "patches/imported_patch.yml"
    config = rpcs3_dir / "config/patch_config.yml"
    require(patch.is_file() and config.is_file(), "local CA and manifest patches are missing")
    patch_text, config_text = (path.read_text(encoding="utf-8") for path in (patch, config))
    require(ppu_hash in patch_text and ppu_hash in config_text,
            "RPCS3 patch files do not use the 01.24 PPU hash")
    require("0x012d3740" in patch_text.lower() and "0x018dca64" in patch_text.lower(),
            "RPCS3 patch files do not contain both 01.24 patch addresses")
    require(config_text.count("Enabled: true") >= 2,
            "both the local CA and unsigned-manifest patches must be enabled")
    ca = local_root / "SIGNON_LOCAL/ca_v4/ca.cert.pem"
    require(ca.is_file(), f"stand-in CA certificate is missing: {ca}")
    pem = ca.read_text(encoding="ascii").replace("\r\n", "\n")
    require(game_ca_patch.yaml_quote(pem) in patch_text,
            "RPCS3 embedded CA patch does not match the stand-in's certificate")
    print("PASS both 01.24 RPCS3 patches enabled for the 01.24 PPU hash")

    require((local_root / "RPCN_LOCAL/rpcn.exe").is_file(),
            f"loopback RPCN service is missing in {local_root}")
    require((local_root / "SIGNON_LOCAL/logs").is_dir(),
            f"stand-in log folder is missing in {local_root / 'SIGNON_LOCAL/logs'}")
    print("PASS private loopback services and CA match the selected patch")

    cfg = rpcs3_dir / "config/config.yml"
    require(cfg.is_file(), "RPCS3 config.yml is missing")
    match = re.search(r"^\s*DNS address:\s*['\"]?([^'\"\s#]+)", cfg.read_text(encoding="utf-8"), re.M)
    require(match is not None, "RPCS3 DNS address is not set")
    require(ipaddress.ip_address(match.group(1)).is_loopback,
            "RPCS3 DNS address must point to the loopback stand-in")
    print("PASS RPCS3 DNS points to loopback")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data-root", type=Path, required=True, help="d1reimagined-data checkout")
    ap.add_argument("--rpcs3-dir", type=Path, help="01.24 RPCS3 copy to inspect")
    ap.add_argument("--game-usrdir", type=Path, help="disc PS3_GAME/USRDIR folder")
    ap.add_argument("--ppu-hash", help="override the calculated 01.24 PPU hash")
    ap.add_argument("--local-root", type=Path,
                    help="private runtime folder (default: D1_LOCAL or repo/local)")
    args = ap.parse_args(argv)
    if bool(args.rpcs3_dir) != bool(args.game_usrdir):
        ap.error("--rpcs3-dir and --game-usrdir must be provided together")
    try:
        inventory, disc_names = check_data(args.data_root)
        if args.rpcs3_dir:
            local_root = args.local_root or Path(os.environ.get("D1_LOCAL") or REPO / "local")
            check_runtime(args.rpcs3_dir, args.game_usrdir,
                          args.ppu_hash or game_ca_patch.build_sites("01.24")["ppu_hash"],
                          inventory, disc_names, local_root)
    except (OSError, KeyError, TypeError, ValueError) as exc:
        print(f"FAIL {exc}", file=sys.stderr)
        return 1
    print("READY for a first 01.24 run" if args.rpcs3_dir else
          "Data checks passed; supply the RPCS3 copy and disc paths to check launch readiness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
