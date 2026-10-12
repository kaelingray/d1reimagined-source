#!/usr/bin/env bash
# d1-ghidra.sh - the one entry point for Ghidra on the Destiny 1 eboot.
#
# Ghidra is a *tool we run*, not code we ship: it is Apache-2.0, lives outside this
# repo, and nothing from it is linked into d1-server.  See historicaldocs/design/ghidra.md.
#
#   ./ghidra/d1-ghidra.sh build       import + seed from the atlas + auto-analyse + verify (~6 min)
#   ./ghidra/d1-ghidra.sh atlas       re-import the atlas only (46 s cold, 2 s warm; idempotent)
#   ./ghidra/d1-ghidra.sh verify      addressing positive controls - run before citing anything (4 s)
#   ./ghidra/d1-ghidra.sh structs     define the critical-path structs + type `this` (~15 s)
#                                     input: ghidra/structs/d1-structs.tsv (python3 -B ghidra/gen-structs.py)
#   ./ghidra/d1-ghidra.sh options     print the auto-analysis option set
#   ./ghidra/d1-ghidra.sh decompile OUTDIR ADDRS [THREADS] [TIMEOUT]
#                                     500 fns @16 threads = 2 s; all 47,306 @24 = 191 s
#   ./ghidra/d1-ghidra.sh gui         open the project in the GUI (needs a display; see §9)
#
# ⚠ Run `atlas` after `build`: auto-analysis flow-repair removes ~231 atlas function starts,
#   and re-running restores them. It costs 2 seconds.
#
# ⚠ Only ONE process may hold the project at a time.  A second one fails with a lock
#   error; that is Ghidra protecting the database, not a bug.
set -euo pipefail

GHIDRA_HOME="${GHIDRA_HOME:-$HOME/dev/ghidra_12.1.2_PUBLIC}"
PROJ_DIR="${D1_GHIDRA_PROJ_DIR:-$HOME/dev/ghidra-projects}"
PROJ_NAME="${D1_GHIDRA_PROJ_NAME:-D1}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EBOOT="$REPO/dumps/patch-01.29-CUSA00219/eboot.bin"
SCRIPTS="$REPO/ghidra"

# Ghidra 12.1.2 requires a JDK 21.  Fedora's default is 25, which the launcher rejects.
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk}"
# The stock headless default is 2G. Seeding 47k functions peaks near 9G; analysis near 20G.
export GHIDRA_HEADLESS_MAXMEM="${GHIDRA_HEADLESS_MAXMEM:-48G}"

HL="$GHIDRA_HOME/support/analyzeHeadless"
[ -x "$HL" ] || { echo "no analyzeHeadless at $HL (set GHIDRA_HOME)" >&2; exit 1; }

cmd="${1:-}"; shift || true

case "$cmd" in
  build)
    [ -f "$EBOOT" ] || { echo "missing $EBOOT" >&2; exit 1; }
    echo "== import (no analysis) =="
    "$HL" "$PROJ_DIR" "$PROJ_NAME" -import "$EBOOT" -noanalysis -overwrite
    echo "== seed from atlas =="
    "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Atlas.java
    echo "== auto-analyse =="
    "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -max-cpu 24 -analysisTimeoutPerFile 10800 \
        -scriptPath "$SCRIPTS" -preScript D1AnalysisOptions.java tune
    echo "== re-seed (analysis flow-repair drops ~231 atlas starts) =="
    "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Atlas.java
    echo "== verify =="
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Verify.java
    ;;
  atlas)
    # Re-applies names/comments over an existing database.  Safe to re-run: names are
    # SourceType.IMPORTED and are overwritten, Ghidra's own ANALYSIS names are not touched.
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Atlas.java "$@"
    ;;
  verify)
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Verify.java
    ;;
  structs)
    # Define the critical-path objects and type `this`.  Run AFTER `atlas` (it appends to the
    # plate comments D1Atlas writes) and re-run `decompile` afterwards to refresh the corpus.
    # Regenerate the input first:  python3 -B ghidra/gen-structs.py
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1Structs.java "$@"
    ;;
  options)
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis \
        -scriptPath "$SCRIPTS" -postScript D1AnalysisOptions.java "$@"
    ;;
  decompile)
    [ $# -ge 2 ] || { echo "usage: $0 decompile OUTDIR ADDRS [THREADS] [TIMEOUT]" >&2; exit 1; }
    exec "$HL" "$PROJ_DIR" "$PROJ_NAME" -process eboot.bin -noanalysis -readOnly \
        -scriptPath "$SCRIPTS" -postScript D1Decompile.java "$@"
    ;;
  gui)
    # ⛔ The operator's displays :0 :1 :99 :1003 are in use by the rig.  Do not steal them.
    echo "GUI: DISPLAY=${DISPLAY:-<unset>}  project=$PROJ_DIR/$PROJ_NAME.gpr"
    exec "$GHIDRA_HOME/ghidraRun" "$PROJ_DIR/$PROJ_NAME.gpr"
    ;;
  *)
    sed -n '2,20p' "${BASH_SOURCE[0]}"
    exit 1
    ;;
esac
