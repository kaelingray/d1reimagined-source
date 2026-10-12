#!/bin/bash
# Assert the capture rig is sane. Run before every session.
# Exits non-zero if anything that silently corrupts a capture is wrong.
# Ref: historicaldocs/destiny1/docs/capture-rig-bridge.md

UPLINK=enp193s0     # -> switch/router/internet
CONSOLE=enp194s0    # -> PS4
fail=0
ok()   { printf '  \033[32mPASS\033[0m  %s\n' "$1"; }
bad()  { printf '  \033[31mFAIL\033[0m  %s\n' "$1"; fail=1; }
warn() { printf '  \033[33mWARN\033[0m  %s\n' "$1"; }

echo "== bridge =="
[ -d /sys/class/net/br0/bridge ] && ok "br0 exists" || bad "br0 missing"
for i in $UPLINK $CONSOLE; do
  [ "$(cat /sys/class/net/$i/master/uplink 2>/dev/null; readlink -f /sys/class/net/$i/master 2>/dev/null | xargs -r basename)" = br0 ] \
    && ok "$i enslaved to br0" || bad "$i NOT enslaved to br0"
done
[ "$(cat /sys/class/net/br0/bridge/stp_state)" = 0 ] && ok "STP off" || bad "STP on (console may stall on link-up)"
[ "$(cat /sys/class/net/br0/bridge/forward_delay)" = 0 ] && ok "forward_delay 0" \
  || bad "forward_delay $(cat /sys/class/net/br0/bridge/forward_delay) (~15s stall; run d1-capture-offloads.sh)"

echo "== offloads (the silent capture-corrupter) =="
for i in $UPLINK $CONSOLE; do
  on=$(ethtool -k $i 2>/dev/null | grep -E '^(generic-receive-offload|tcp-segmentation-offload|generic-segmentation-offload|rx-vlan-offload)' | grep -c ': on')
  [ "$on" = 0 ] && ok "$i offloads off" || bad "$i has $on offload(s) STILL ON"
done

echo "== no netfilter on bridged frames =="
[ "$(lsmod | grep -c br_netfilter)" = 0 ] && ok "br_netfilter not loaded" \
  || warn "br_netfilter loaded - firewalld/nftables can now drop bridged frames"

echo "== console link =="
if [ "$(cat /sys/class/net/$CONSOLE/carrier 2>/dev/null)" = 1 ]; then
  ok "$CONSOLE has carrier (console connected)"
  st=$(bridge link show dev $CONSOLE 2>/dev/null | grep -o 'state [a-z]*' | awk '{print $2}')
  [ "$st" = forwarding ] && ok "$CONSOLE forwarding" || bad "$CONSOLE state=$st (not forwarding)"
else
  warn "$CONSOLE has no carrier - plug in the PS4"
fi

echo "== host =="
if ! command -v dumpcap >/dev/null; then
  bad "dumpcap missing"
elif dumpcap --version >/dev/null 2>&1; then
  ok "dumpcap runnable ($(dumpcap --version 2>/dev/null | head -1 | awk '{print $3}'))"
elif sg wireshark -c 'dumpcap --version' >/dev/null 2>&1; then
  warn "dumpcap needs the 'wireshark' group - capture.sh re-execs itself to get it."
  warn "  (log out and back in to make it permanent for interactive use)"
else
  bad "dumpcap not executable even via the wireshark group"
fi
chronyc tracking >/dev/null 2>&1 && ok "clock synced ($(chronyc tracking 2>/dev/null | awk '/Reference ID/{print $4}'))" || bad "clock not synced"
free=$(df --output=avail -BG $HOME/dev/destiny1-preservation/captures 2>/dev/null | tail -1 | tr -dc 0-9)
[ "${free:-0}" -gt 50 ] && ok "${free}G free for captures" || bad "only ${free}G free"
dl=$(pgrep -c -f 'download_ps4_patches|aria2c' 2>/dev/null || true)
[ "${dl:-0}" = 0 ] && ok "no patch downloads competing" || bad "$dl download(s) running - pause them (drops + polluted capture)"

echo
[ $fail = 0 ] && echo "RIG READY" || echo "RIG NOT READY - fix the FAILs above"
exit $fail
