#!/bin/bash
# Switch the capture rig between two topologies.
#
#   live       enp194s0 joins br0 with the uplink. Console is transparently on
#              the LAN + internet. For A/B (current firmware) and Lane B capture.
#
#   contained  enp194s0 is a standalone, isolated segment (10.99.99.1/24), NOT
#              bridged to the uplink, with an explicit forward-drop. Console can
#              reach ONLY this host, never the internet. For C/D (10.00/10.01
#              CFW) and Lane A. THE POINT: a 10.0x console physically cannot pull
#              a system update, because no egress path exists. Update past 13.00
#              is irreversible, so containment is by topology, not by a rule that
#              could be toggled off.
#
# Usage: sudo ./rig-mode.sh {live|contained|status|selftest}
#
# ⭐ EXIT CODES - this script is the containment ASSERTION named in CLAUDE.md, so it must be
# usable as one. Before 2026-08-11 every path exited 0, including INDETERMINATE, so any caller
# checking the exit code got a false green on an uncontained rig. Now:
#
#   status     0 = CONTAINED   1 = LIVE   2 = INDETERMINATE / cannot tell
#   contained  0 = reached CONTAINED, nonzero otherwise
#   live       0 = reached LIVE, nonzero otherwise
#
# So `sudo rig-mode.sh status && <thing that must not touch the internet>` is now correct, and
# CANNOT-TELL is the worst code rather than a pass. `selftest` exercises the verdict logic with
# synthetic inputs and needs neither root nor a rig.
set -u
CONSOLE=enp194s0
CONSUBNET=10.99.99.1/24
GUARD_TABLE=d1guard

# Pure verdict, separated from measurement so it can be tested without root or a live rig.
# Args: <master> <ip> <guard: yes|no|unknown>. Echoes the verdict, returns the exit code.
_verdict() {
  local inbr0="$1" hasip="$2" guard="$3"
  if [ "$inbr0" = br0 ]; then echo "  => LIVE (console reaches internet)"; return 1
  elif [ -n "$hasip" ] && [ "$guard" = yes ]; then echo "  => CONTAINED (console isolated)"; return 0
  else echo "  => INDETERMINATE - cannot assert containment; set a mode explicitly"; return 2; fi
}

status() {
  echo "== rig mode =="
  local inbr0 hasip guard guard_why nftout
  # -e the symlink first; readlink -f echoes a canonical path even for a nonexistent target.
  inbr0=$([ -e /sys/class/net/$CONSOLE/master ] && basename "$(readlink -f /sys/class/net/$CONSOLE/master)")
  hasip=$(ip -4 -br addr show $CONSOLE 2>/dev/null | awk '{print $3}')
  # Verify the DROP RULES exist, not merely the table - an empty table is a false green.
  # And distinguish "no rules" from "could not read": without root nft fails, and reporting that
  # as an absent guard would be a false RED that trains people to ignore this output.
  guard_why=""
  if nftout=$(nft -a list chain inet $GUARD_TABLE guard 2>/dev/null); then
    local ndrop; ndrop=$(printf '%s' "$nftout" | grep -c 'drop')
    if [ "${ndrop:-0}" -ge 2 ]; then guard=yes
    else guard=no; guard_why=" (table present, $ndrop drop rules, need 2)"; fi
  elif [ "$(id -u)" -ne 0 ]; then
    guard=unknown; guard_why=" (cannot read nftables without root - RUN WITH sudo)"
  else
    guard=no; guard_why=" (guard table $GUARD_TABLE absent)"
  fi
  echo "  $CONSOLE master:      ${inbr0:-<none>}"
  echo "  $CONSOLE ip:          ${hasip:-<none>}"
  echo "  forward-drop guard:   ${guard}${guard_why}"
  echo "  ip_forward:           $(sysctl -n net.ipv4.ip_forward 2>/dev/null || echo '?')"
  echo "  $CONSOLE carrier:     $(cat /sys/class/net/$CONSOLE/carrier 2>/dev/null || echo n/a)"
  _verdict "$inbr0" "$hasip" "$guard"
}

# A control that can FAIL: the verdict table, including the inversions. No root, no rig, no
# network. Every row states the axis it protects.
selftest() {
  local fails=0
  _check() { # <want-rc> <master> <ip> <guard> <why>
    local want="$1" out rc; out=$(_verdict "$2" "$3" "$4"); rc=$?
    if [ "$rc" = "$want" ]; then echo "  PASS  rc=$rc  $5"
    else echo "  FAIL  rc=$rc want=$want  $5 -> $out"; fails=$((fails+1)); fi
  }
  echo "== rig-mode verdict selftest =="
  _check 0 ""    "10.99.99.1/24" yes     "CONTAINED: isolated ip + 2 drop rules"
  _check 1 br0   "10.99.99.1/24" yes     "LIVE wins over everything: bridged is bridged"
  _check 1 br0   ""              no      "LIVE: bridged, no ip"
  _check 2 ""    "10.99.99.1/24" no      "INVERTED: ip but guard rules MISSING must not pass"
  _check 2 ""    "10.99.99.1/24" unknown "INVERTED: guard unreadable (no root) must not pass"
  _check 2 ""    ""              yes     "INVERTED: guard up but iface unconfigured must not pass"
  _check 2 ""    ""              no      "INDETERMINATE: nothing configured"
  if [ "$fails" -ne 0 ]; then echo "  $fails FAILED"; return 2; fi
  echo "  all 7 pass"; return 0
}

guard_up() {   # drop any forwarding to/from the console iface, regardless of ip_forward
  nft list table inet $GUARD_TABLE >/dev/null 2>&1 && nft delete table inet $GUARD_TABLE
  # 'fwd' is a reserved nft keyword; the chain must not be named that.
  nft add table inet $GUARD_TABLE
  nft "add chain inet $GUARD_TABLE guard { type filter hook forward priority -300 ; policy accept ; }"
  nft "add rule inet $GUARD_TABLE guard iifname \"$CONSOLE\" drop"
  nft "add rule inet $GUARD_TABLE guard oifname \"$CONSOLE\" drop"
}
guard_down() { nft list table inet $GUARD_TABLE >/dev/null 2>&1 && nft delete table inet $GUARD_TABLE; }

case "${1:-status}" in
  contained)
    echo "== -> CONTAINED =="
    nmcli con mod br0-console connection.autoconnect no 2>/dev/null
    nmcli con down br0-console 2>/dev/null
    # Standalone isolated profile: static IP, no gateway, no DNS, never default route.
    if ! nmcli -t -f NAME con show | grep -qx d1-console-iso; then
      nmcli con add type ethernet ifname $CONSOLE con-name d1-console-iso \
        ipv4.method manual ipv4.addresses $CONSUBNET ipv4.never-default yes \
        ipv6.method disabled connection.autoconnect yes
    fi
    nmcli con mod d1-console-iso connection.autoconnect yes
    nmcli con up d1-console-iso 2>/dev/null
    guard_up
    /usr/local/sbin/d1-capture-offloads.sh
    echo; status; rc=$?
    # Assert we ARRIVED. Switching and then reporting whatever happened is how a failed switch
    # reads as success; the whole point of this mode is that the console cannot reach the internet.
    if [ "$rc" -ne 0 ]; then
      echo "⛔ FAILED to reach CONTAINED (verdict rc=$rc). The console may still have egress." >&2
      echo "   Do NOT power on a CFW console until this says CONTAINED." >&2
      exit "$rc"
    fi
    exit 0 ;;
  live)
    echo "== -> LIVE =="
    nmcli con mod d1-console-iso connection.autoconnect no 2>/dev/null
    nmcli con down d1-console-iso 2>/dev/null
    guard_down
    nmcli con mod br0-console connection.autoconnect yes
    nmcli con up br0-console 2>/dev/null
    /usr/local/sbin/d1-capture-offloads.sh
    echo; status; rc=$?
    if [ "$rc" -ne 1 ]; then
      echo "⛔ FAILED to reach LIVE (verdict rc=$rc)." >&2
      exit 2
    fi
    exit 0 ;;
  status) status; exit $? ;;
  selftest) selftest; exit $? ;;
  *) echo "usage: sudo $0 {live|contained|status|selftest}"; exit 2 ;;
esac
