#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""The executable-build registry, ``capture-rig/builds.json`` (Python side of ``d1-build``).

``$D1_BUILD`` selects a build, otherwise the registry ``default``. The ``reference`` build (01.29)
is the one every fixed address in ``design/`` and the probe-site catalogues was measured against;
a build whose ``addresses_valid`` is false has none yet, and address-bearing tools must refuse it.

    python3 capture-rig/d1build.py shell     # export lines for bash launchers
    python3 capture-rig/d1build.py show      # the selected entry as JSON
"""
import json
import os
import shlex
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REGISTRY = Path(os.environ.get("D1_BUILDS") or HERE / "builds.json")
REPO = REGISTRY.resolve().parent.parent


def load(build_id=None):
    doc = json.loads(REGISTRY.read_text(encoding="utf-8"))
    if doc.get("schema") != 1:
        raise SystemExit(f"build registry schema {doc.get('schema')} is not 1")
    want = build_id or os.environ.get("D1_BUILD") or doc["default"]
    if want not in doc["builds"]:
        raise SystemExit(f"unknown build {want!r} (D1_BUILD); registry has: {', '.join(doc['builds'])}")
    entry = dict(doc["builds"][want], id=want, reference=doc["reference"])
    entry["is_reference"] = want == doc["reference"]
    return entry


def selected():
    return load()


def path(entry, key):
    """Absolute path for a registry-relative field, or None when the field is unset."""
    rel = entry.get(key)
    return None if rel is None else REPO / rel


def shell_exports(entry):
    rows = {
        "D1_BUILD_ID": entry["id"],
        "D1_BUILD_LABEL": entry["build_label"],
        "D1_BUILD_ADDRESSES_VALID": "1" if entry["addresses_valid"] else "0",
        "D1_BUILD_GAME_DIR": path(entry, "game_dir"),
        "D1_BUILD_EBOOT": path(entry, "eboot"),
        "D1_BUILD_EBOOT_SHA256": entry["eboot_sha256"],
        "D1_BUILD_EBOOT_BYTES": entry["eboot_bytes"],
        "D1_BUILD_LAUNCH": path(entry, "launch"),
        "D1_BUILD_LAUNCH_SHA256": entry["launch_sha256"],
        "D1_BUILD_LAUNCH_BYTES": entry["launch_bytes"],
        "D1_BUILD_MANIFEST": path(entry, "manifest"),
        "D1_BUILD_MANIFEST_SHA256": entry.get("manifest_sha256") or "",
        "D1_BUILD_MANIFEST_BYTES": entry.get("manifest_bytes") or "",
        "D1_BUILD_PATCH": path(entry, "diag_patch") or "",
    }
    return "\n".join(f"export {k}={shlex.quote(str(v))}" for k, v in rows.items())


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "show"
    build_id = argv[2] if len(argv) > 2 else None
    entry = load(build_id)
    if cmd == "shell":
        print(shell_exports(entry))
    elif cmd == "show":
        print(json.dumps(entry, indent=2))
    else:
        raise SystemExit("usage: d1build.py [show|shell] [build-id]")


if __name__ == "__main__":
    main(sys.argv)
