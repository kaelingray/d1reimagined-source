#!/bin/bash
# Start a capture. One dumpcap per bridge member + br0 as cross-check.
# Usage: ./capture.sh <run-label>      e.g. ./capture.sh stage0-solo-01.33
# Ctrl-C to stop. READ THE DROP COUNTS IT PRINTS: a run with drops is a run to redo.
# Ref: historicaldocs/destiny1/docs/capture-rig-bridge.md
set -u

# dumpcap is 0750 root:wireshark. usermod adds the group but existing shells do
# not have it until re-login, so pick it up here rather than failing mid-session.
if ! dumpcap --version >/dev/null 2>&1; then
  exec sg wireshark -c "$(printf '%q ' "$0" "$@")"
fi

LABEL="${1:?usage: capture.sh <run-label>}"
OUT=$HOME/dev/destiny1-preservation/captures
RIG="$(dirname "$(readlink -f "$0")")"

"$RIG/preflight.sh" || { echo; echo "ABORT: preflight failed."; exit 1; }

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
DIR="$OUT/${STAMP}-${LABEL}"
mkdir -p "$DIR"

# Environment manifest - captures are only comparable if "comparable" is written down.
{
  echo "run_label:     $LABEL"
  echo "started_utc:   $(date -u +%FT%TZ)"
  echo "host:          $(hostname) ($(ip -4 -br addr show br0 | awk '{print $3}'))"
  echo "kernel:        $(uname -r)"
  echo "dumpcap:       $(dumpcap --version 2>/dev/null | head -1)"
  echo "clock_source:  $(chronyc tracking 2>/dev/null | awk '/Reference ID/{print $4,$5}')"
  echo "clock_offset:  $(chronyc tracking 2>/dev/null | awk '/System time/{print $4,$5,$6}')"
  echo "bridge:        stp=$(cat /sys/class/net/br0/bridge/stp_state) fwd_delay=$(cat /sys/class/net/br0/bridge/forward_delay)"
  for i in enp193s0 enp194s0; do
    echo "ethtool_$i:  $(ethtool -k $i | grep -E '^(generic-receive|tcp-segmentation|generic-segmentation|rx-vlan)' | tr '\n' ' ')"
  done
  echo
  echo "# TODO fill in per run: game version, console firmware + serial, account IDs,"
  echo "# console role (A/B/C/D), lane (A/B1/B2/B3), and the wall-clock action log."
} > "$DIR/manifest.txt"

echo "capturing -> $DIR   (Ctrl-C to stop)"
pids=()
for i in enp194s0 enp193s0 br0; do
  dumpcap -i "$i" -s 0 -B 512 -w "$DIR/${LABEL}-${i}.pcapng" > "$DIR/dumpcap-${i}.log" 2>&1 &
  pids+=($!)
  echo "  dumpcap on $i (pid ${pids[-1]})"
done

trap 'echo; echo "stopping..."; kill -INT "${pids[@]}" 2>/dev/null' INT TERM
wait "${pids[@]}" 2>/dev/null

echo
echo "== DROP COUNTS (any non-zero => redo the run, today, not in analysis) =="
for i in enp194s0 enp193s0 br0; do
  printf '  %-10s %s\n' "$i" "$(grep -oE 'received/dropped on interface .*' "$DIR/dumpcap-${i}.log" | tail -1)"
done
echo
echo "== files =="
ls -la "$DIR"/*.pcapng 2>/dev/null | awk '{printf "  %-50s %s\n", $NF, $5}'
echo "  (enp194s0 = console-only traffic, the primary artifact)"
echo "  (enp193s0 = console + this host's own traffic)"
echo
echo "hash them before doing anything else:"
echo "  cd '$DIR' && sha256sum *.pcapng > SHA256SUMS"
