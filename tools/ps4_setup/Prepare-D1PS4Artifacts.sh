#!/usr/bin/env bash
# BOOTSTRAP.md 3a/3b/4: use existing generators and controls, never substitute a build.
# The build comes from capture-rig/builds.json (D1_BUILD, else the registry default). Only a build
# whose fixed addresses are valid runs the address-bearing generators; for any other build the
# manifest is generated and recorded, and the remaining steps are reported as not yet re-derived.
set -euo pipefail
repo=/mnt/f/d1/d1preservation-main
cd "$repo"
export D1_REPO="$repo"
export PYTHONPATH="/opt/d1-tools${PYTHONPATH:+:$PYTHONPATH}"
eval "$(python3 capture-rig/d1build.py shell)"
export D1_BUILD="$D1_BUILD_ID"
eboot="$D1_BUILD_EBOOT"
[[ -f "$eboot" ]] || { echo "REFUSING: missing $D1_BUILD_ID executable: $eboot" >&2; exit 2; }
[[ "$(stat -c %s "$eboot")" == "$D1_BUILD_EBOOT_BYTES" && "$(sha256sum "$eboot" | cut -d ' ' -f 1)" == "$D1_BUILD_EBOOT_SHA256" ]] || {
    echo "REFUSING: executable does not match the registered $D1_BUILD_ID build" >&2; exit 2;
}
packages="$(python3 -c 'import sys; sys.path.insert(0, "capture-rig"); import d1build; print(d1build.path(d1build.selected(), "packages"))')"
[[ -d "$packages" && -n "$(find "$packages" -maxdepth 1 -type f -name '*.pkg' -print -quit)" ]] || {
    echo 'REFUSING: installed Bungie content packages are missing' >&2; exit 2;
}
bash /mnt/f/d1/tools/Install-D1WSLEntrypoints.sh
python3 -c 'import capstone, unicorn'
bin="$repo/d1-server/target/release"
manifest="$D1_BUILD_MANIFEST"
mkdir -p "$(dirname "$manifest")" capture-rig/schemas
if [[ "$D1_BUILD_ADDRESSES_VALID" != 1 ]]; then
    [[ -e "$manifest" ]] && { echo "REFUSING: existing manifest $manifest (record or move it first)" >&2; exit 2; }
    "$bin/d1-pkg" manifest -o "$manifest"
    echo "Recorded $D1_BUILD_ID manifest: $(stat -c %s "$manifest") bytes, sha256 $(sha256sum "$manifest" | cut -d ' ' -f 1)"
    echo "Pin it in capture-rig/builds.json (manifest_sha256/manifest_bytes) after review."
    echo "NOT RUN for $D1_BUILD_ID: schema 0x80A5E37A extraction, d1-eboot/d1-oracle/d1-activity-host controls"
    echo "and the eboot atlas. They use fixed 01.29 addresses (Oodle entry points, codec tables) that"
    echo "must be re-derived for this build first."
    exit 3
fi
"$bin/d1-pkg" controls
"$bin/d1-pkg" manifest -o "$manifest"
[[ "$(stat -c %s "$manifest")" == "$D1_BUILD_MANIFEST_BYTES" &&
   "$(sha256sum "$manifest" | cut -d ' ' -f 1)" == "$D1_BUILD_MANIFEST_SHA256" ]] || {
    echo 'REFUSING: generated manifest differs from the pinned full English manifest' >&2; exit 2;
}
python3 -B capture-rig/oodle/extract-entry.py 0x80A5E37A -o capture-rig/schemas/investment-message-definitions.bin
"$bin/d1-eboot" controls
"$bin/d1-oracle" control
"$bin/d1-activity-host" controls
python3 -B capture-rig/eboot-atlas.py
echo 'Static preparation completed. This is not a native boot or gameplay acceptance.'
echo 'Continue the remaining fixture generators and canonical Windows/WSL launcher work in BOOTSTRAP.md.'
