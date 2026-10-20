"""Verify or rebuild the two split PS3 packages in this repository."""

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent
BLOCK_SIZE = 1024 * 1024


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(BLOCK_SIZE), b""):
            digest.update(block)
    return digest.hexdigest()


def process_package(package, output_dir):
    name = package["filename"]
    if Path(name).name != name:
        raise ValueError(f"Invalid package filename: {name}")
    destination = output_dir / name if output_dir is not None else None
    if destination is not None and destination.exists():
        if (destination.stat().st_size == package["size"]
                and sha256_file(destination) == package["sha256"]):
            print(f"Already verified: {destination}")
            return
        raise FileExistsError(f"Existing package differs: {destination}")

    aggregate = hashlib.sha256()
    total = 0
    temporary = None
    try:
        if destination is not None:
            with tempfile.NamedTemporaryFile(
                mode="wb", dir=output_dir, prefix=f".{name}.",
                suffix=".tmp", delete=False
            ) as output:
                temporary = Path(output.name)
                for part in package["parts"]:
                    total += copy_verified_part(part, aggregate, output)
        else:
            for part in package["parts"]:
                total += copy_verified_part(part, aggregate, None)
        if total != package["size"] or aggregate.hexdigest() != package["sha256"]:
            raise ValueError(f"Original package checksum mismatch: {name}")
        if destination is not None:
            os.replace(temporary, destination)
            temporary = None
            print(f"Rebuilt and verified: {destination}")
        else:
            print(f"Verified parts for: {name}")
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def copy_verified_part(part, aggregate, output):
    path = (ROOT / part["path"]).resolve()
    try:
        path.relative_to(ROOT)
    except ValueError as exc:
        raise ValueError(f"Part outside repository: {path}") from exc
    if path.stat().st_size != part["size"]:
        raise ValueError(f"Part size mismatch: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(BLOCK_SIZE), b""):
            digest.update(block)
            aggregate.update(block)
            if output is not None:
                output.write(block)
    if digest.hexdigest() != part["sha256"]:
        raise ValueError(f"Part checksum mismatch: {path}")
    return part["size"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--verify-only", action="store_true")
    action.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    output_dir = args.output_dir
    if output_dir is not None:
        output_dir.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((ROOT / "split_packages.json").read_text(encoding="utf-8"))
    for package in manifest["packages"]:
        process_package(package, output_dir)


if __name__ == "__main__":
    main()
