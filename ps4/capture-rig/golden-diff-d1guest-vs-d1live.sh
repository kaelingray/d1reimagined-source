#!/bin/bash
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
#
# GOLDEN DIFF: the READ SIDE of capture-rig/d1guest.py vs `d1-live`, over ONE target process.
#
# WHY THIS EXISTS
# ===============
# `d1-live read|obj|tag|descriptors|syms` is a port of `d1guest.py`'s read side, and a port is only
# a port if somebody ran both over the same input and compared the bytes. There is no live client
# most of the time, so this drives `d1-live __mem-target`: a real Linux process holding the REAL
# eboot mapped at a base, plus an object registry laid out from `guest-symbols.json`, a package
# registry with a real signed back-offset, and a descriptor table with claimed rows.
#
# ⚠ The registry CONTENTS are ours. Nothing this prints is evidence about Destiny (reach tag
# `synthetic-minimal`). What it measures is whether two readers of the same bytes agree.
#
# THE FOUR CONFIGURATIONS, and why the last three matter
# ======================================================
#   none              both tools must produce the same dumps
#   family-stride     one bit of stride disagreement: both must refuse the same way
#   verify-window     one flipped byte in the verify window: both base controls must fail
#   duplicate-anchor  two candidate bases: both must say "need EXACTLY one"
# Agreeing on the happy path is cheap. Agreeing on every refusal is the port.
#
# USAGE
#   capture-rig/golden-diff-d1guest-vs-d1live.sh [none|family-stride|verify-window|duplicate-anchor]
#
# ⛔ Run it with the emulator CLOSED. The fixture refuses to start beside a real client, because it
#    is discoverable by the same `pgrep` an instrument uses to find that client.
#
# ⚠ The decoy words live in THIS FILE and must never be typed at a shell: `pgrep -f` matches the
#   whole command line, so a shell that mentions them becomes a second "emulator" and `d1guest.py`
#   refuses to guess between them. Same family as the `pkill -f matches its own shell` trap.
set -u
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO=$(dirname "$HERE")
B=$REPO/d1-server/target/release/d1-live
PY="python3 -B $REPO/capture-rig/d1guest.py"
DECOY="shad""ps4 -p GOLDEN-DIFF-FIXTURE"
OUT=${OUT:-/tmp/d1-golden-diff}
PERTURB=${1:-none}

[ -x "$B" ] || { echo "build it first:  (cd $REPO/d1-server && cargo build --release)"; exit 9; }
rm -rf "$OUT"; mkdir -p "$OUT"

# ⚠ No `setsid`: it reparents the fixture to init, and the fixture identifies the shells that
# launched it by walking PPid so it does not refuse against its own reflection. Plain backgrounding
# keeps that chain intact.
$B __mem-target --hold 300 --perturb "$PERTURB" $DECOY > "$OUT/banner.json" 2>"$OUT/banner.err" &
for _ in $(seq 1 120); do [ -s "$OUT/banner.json" ] && break; sleep 0.5; done
PID=$(python3 -c "import json;print(json.load(open('$OUT/banner.json'))['pid'])" 2>/dev/null)
if [ -z "${PID:-}" ]; then echo "FIXTURE FAILED:"; cat "$OUT/banner.err"; exit 9; fi
N=$(pgrep -f "[s]hadps4 -p " | wc -l)
echo "fixture pid=$PID  perturb=$PERTURB  pgrep matches=$N  (d1guest.py refuses on anything but 1)"
if [ "$N" != 1 ]; then echo "ABORT: the Python instrument cannot be aimed"; kill "$PID"; exit 9; fi

# Both tools cache a recovered base. Clear both, and pass --no-cache, so the ANCHOR SCAN itself is
# compared and not just the two cheap controls that re-run either way.
rm -f /tmp/d1guest-base.json /tmp/d1-live-base.json
FAIL=0

run() {                       # run <name> <args...>
  local name=$1; shift
  # d1guest.py puts the shared flags on its TOP-LEVEL parser; d1-live puts them on the subcommand.
  # Same flags, different position: the one structural difference between the two CLIs.
  $PY --no-cache "$@"             > "$OUT/$name.py.txt" 2>"$OUT/$name.py.err"; local pe=$?
  $B "$@" --no-cache --pid "$PID" > "$OUT/$name.rs.txt" 2>"$OUT/$name.rs.err"; local re=$?
  # The ONLY normalisation is the wall-clock stamp `descriptors` prints, which cannot match across
  # two invocations. Addresses, control lines and byte columns are compared raw.
  sed -E 's/^[0-9]{2}:[0-9]{2}:[0-9]{2}$/HH:MM:SS/' "$OUT/$name.py.txt" > "$OUT/$name.py.n"
  sed -E 's/^[0-9]{2}:[0-9]{2}:[0-9]{2}$/HH:MM:SS/' "$OUT/$name.rs.txt" > "$OUT/$name.rs.n"
  if [ "$pe" != "$re" ]; then
      echo "  *** EXIT CODES DIFFER $name: py=$pe rs=$re"; FAIL=1
  fi
  if diff -u "$OUT/$name.py.n" "$OUT/$name.rs.n" > "$OUT/$name.diff"; then
      echo "  IDENTICAL  $(printf '%-14s' "$name") exit py=$pe rs=$re  $(wc -c < "$OUT/$name.py.n") bytes  md5 $(md5sum < "$OUT/$name.py.n" | cut -c1-12)"
  else
      echo "  *** DIFFERS $(printf '%-14s' "$name") exit py=$pe rs=$re"; sed -n '1,40p' "$OUT/$name.diff"
      FAIL=1
  fi
}

run read-sym        read profile_setup_kind
run read-vaddr      read 0x263D0F8 --len 0x270
run read-struct     read character_selection_manager --deref --struct character_selection_node
run read-reloc      read boot_state_names --len 320 --as ptr
run read-u32        read 0x1324060 --len 64 --as u32
run read-u64        read 0x1324060 --len 64 --as u64
run read-abs        read 0x800798050 --abs --len 48
run read-bad-sym    read no_such_symbol_at_all
run obj-account     obj 4:0
run obj-index       obj 3:1:2 --len 64
run obj-key         obj 3:1 --key 0xD20000000000002A
run obj-nokey       obj 3:1 --key 0xDEADBEEF
run obj-overcap     obj 3:1:99
run tag-one         tag 0x80801825
run tag-many        tag 0x80801825 0x808018B7 0x00000001 --len 32
run descriptors     descriptors

kill "$PID" 2>/dev/null

# `syms` needs no target at all, so it is compared without the fixture.
for pat in "" profile signon schema nosuch; do
  $PY syms $pat > "$OUT/syms.py.txt" 2>&1
  $B  syms $pat > "$OUT/syms.rs.txt" 2>&1
  if diff -q "$OUT/syms.py.txt" "$OUT/syms.rs.txt" > /dev/null; then
      echo "  IDENTICAL  $(printf '%-14s' "syms:${pat:-all}") $(wc -c < "$OUT/syms.py.txt") bytes"
  else
      echo "  *** DIFFERS syms:${pat:-all}"; diff -u "$OUT/syms.py.txt" "$OUT/syms.rs.txt" | head -20; FAIL=1
  fi
done

echo "GOLDEN DIFF ($PERTURB): $([ $FAIL = 0 ] && echo ALL IDENTICAL || echo DIFFERENCES FOUND)   artifacts in $OUT"
exit $FAIL
