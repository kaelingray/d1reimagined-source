#!/usr/bin/env bash
# Save each actual tool exit code; the overall run fails if any required control fails.
set -uo pipefail
cd /mnt/f/d1/d1preservation-main
export D1_REPO="$PWD"
out=cfw-staging/2026-10-11
mkdir -p "$out"
: > "$out/controls-exits.tsv"
fail=0
for tool in d1-responder shadnet-server d1; do
    /opt/d1-target/d1-server/release/"$tool" controls > "$out/$tool-controls.log" 2>&1
    rc=$?
    printf '%s exit=%s\n' "$tool" "$rc"
    tail -n 7 "$out/$tool-controls.log"
    printf '%s\t%s\n' "$tool" "$rc" >> "$out/controls-exits.tsv"
    if [[ "$rc" != 0 ]]; then fail=1; fi
done
exit "$fail"
