#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Set shadPS4's render and window resolution, safely.
#
# WHY THIS IS A SCRIPT AND NOT "just edit the json"
# -------------------------------------------------
# ⚠ shadPS4 REWRITES config.json ON EXIT from its in-memory state. Editing the file while the
# emulator is running looks like it worked -- the file changes -- and is then silently reverted the
# moment the emulator closes. You lose the change AND the run you were about to do. This script
# refuses to touch the file while shadps4 is alive, which is the only reliable guard.
#
# ⚠ `pgrep -x shadps4` MATCHES NOTHING. The comm is `shadPS4:Main`. Use the -f pattern below and
# keep the [s] character class, or the pattern matches the shell evaluating it (CLAUDE.md).
#
# WHAT THE TWO PAIRS MEAN -- they are not the same thing
# -----------------------------------------------------
#   /GPU/internal_screen_*  the RENDER resolution the guest presents at.  Destiny on PS4 is 1080p,
#                           so 1920x1080 is the AUTHENTIC value; shadPS4 defaults to 1280x720,
#                           which is the emulator's choice, not the console's.
#   /GPU/window_*           the desktop window size. Cosmetic. Fullscreen (`D1_FULLSCREEN=1` in
#                           run-boot.sh) scales whatever is rendered up to the panel.
#
# ⚠ Changing internal_screen_* changes every screenshot from here on. Pixel comparisons ACROSS this
# boundary are invalid -- earlier runs are 720p. Record which runs are which.
#
# Usage:
#   capture-rig/emu-display.sh                 # show current values
#   capture-rig/emu-display.sh 1920 1080       # set both pairs, with a backup
#   capture-rig/emu-display.sh 1280 720        # back to the shadPS4 default

set -euo pipefail

CONFIG="${SHADPS4_CONFIG:-$HOME/.local/share/shadPS4/config.json}"
CUSTOM_DIR="$(dirname "$CONFIG")/custom_configs"

[[ -f "$CONFIG" ]] || { echo "no config at $CONFIG" >&2; exit 2; }

# A per-game config silently overrides the global one, so editing the global would do nothing.
if compgen -G "$CUSTOM_DIR/*" > /dev/null 2>&1; then
    echo "⚠ per-game configs exist in $CUSTOM_DIR - the global file may be overridden:" >&2
    ls -1 "$CUSTOM_DIR" >&2
    echo "  Edit the per-game file instead, or remove it." >&2
    exit 2
fi

show() {
    python3 - "$CONFIG" <<'PY'
import json, sys
gpu = json.load(open(sys.argv[1])).get("GPU", {})
print("  render (internal_screen)  {}x{}".format(
    gpu.get("internal_screen_width"), gpu.get("internal_screen_height")))
print("  window                    {}x{}".format(
    gpu.get("window_width"), gpu.get("window_height")))
print("  full_screen               {}  ({})".format(
    gpu.get("full_screen"), gpu.get("full_screen_mode")))
PY
}

if [[ $# -eq 0 ]]; then
    echo "current ($CONFIG):"
    show
    exit 0
fi

[[ $# -eq 2 ]] || { echo "usage: $0 [WIDTH HEIGHT]" >&2; exit 2; }
WIDTH="$1"; HEIGHT="$2"
[[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || { echo "WIDTH/HEIGHT must be integers" >&2; exit 2; }

# THE GUARD. See the header.
if pgrep -f "[s]hadps4 -p " > /dev/null 2>&1; then
    echo "⛔ shadPS4 is RUNNING. It rewrites config.json on exit and would revert this edit." >&2
    echo "   Close the emulator, then run this again." >&2
    exit 2
fi

BACKUP="$CONFIG.bak-$(date +%Y%m%d-%H%M%S)"
cp -p "$CONFIG" "$BACKUP"

echo "before:"; show

python3 - "$CONFIG" "$WIDTH" "$HEIGHT" <<'PY'
import json, sys
path, width, height = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
with open(path) as handle:
    config = json.load(handle)
gpu = config.setdefault("GPU", {})
gpu["internal_screen_width"] = width
gpu["internal_screen_height"] = height
gpu["window_width"] = width
gpu["window_height"] = height
with open(path, "w") as handle:
    json.dump(config, handle, indent=2)
    handle.write("\n")
PY

echo "after:"; show
echo
echo "backup: $BACKUP"
echo "⚠ Screenshots from runs before this point are a DIFFERENT resolution. Do not pixel-compare"
echo "  across the boundary."
