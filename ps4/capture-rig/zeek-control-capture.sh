#!/bin/bash
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
#
# zeek-control-capture.sh - manufacture the POSITIVE CONTROL pcap for zeek-logs.py.
#
# WHY THIS EXISTS
# ---------------
# "The pipeline ran and produced logs" is not evidence the pipeline works: an empty conn.log
# and a broken conn.log look identical.  ATLAS §4.2 - a scan without a positive control is not
# evidence of absence.  So before any real capture is analysed, we hand Zeek a pcap whose
# contents we chose, and assert it finds them.
#
# WHAT IT CAPTURES - read this, it matters for the containment rules
# ------------------------------------------------------------------
# NOTHING off this box, and nothing of the operator's network.  The entire control runs inside
# an unprivileged user+network namespace (`unshare -r -n`) that has exactly one interface:
# a fresh loopback.  There is no route out of it.  The DNS server, the TLS server, the client
# and tcpdump are all inside it.  No sudo, no host interface, no packet leaves.
#
# WHAT IT PUTS ON THE WIRE (all 127.0.0.1)
#   1. a well-formed DNS A query + answer for `www4.signon.deadorbit.net`   -> dns.log
#      (a real D1 signon host name, used as a literal only; nothing is resolved for real)
#   2. a TLS 1.2 handshake with SNI `www4.signon.deadorbit.net`             -> ssl.log
#   3. a self-signed certificate for that name, presented in the clear      -> x509.log
#   4. both of the above as ordinary flows                                  -> conn.log
#
# ⚠ TLS 1.2 IS DELIBERATE.  TLS 1.3 encrypts the server Certificate message, so a 1.3 control
# would produce an EMPTY x509.log and we would have "proved" the pipeline works while the
# certificate path was silently dead.  That is exactly the false negative this project keeps
# paying for.  D1's own client is 2016-era and negotiates =<1.2, so 1.2 is also the honest
# rehearsal for the retail capture.
#
# Usage:  zeek-control-capture.sh <output.pcap>
set -euo pipefail

OUT="${1:?usage: zeek-control-capture.sh <output.pcap>}"
OUT="$(readlink -f "$OUT")"
mkdir -p "$(dirname "$OUT")"

for tool in unshare tcpdump openssl python3; do
  command -v "$tool" >/dev/null || { echo "zeek-control-capture: missing $tool" >&2; exit 2; }
done

WORK="$(mktemp -d /tmp/d1-zeek-control.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

# The name the whole control is keyed on.  Every assertion in zeek-logs.py --self-test looks
# for this exact string, in three different logs.
NAME=www4.signon.deadorbit.net

openssl req -x509 -newkey rsa:2048 -nodes -days 2 \
  -keyout "$WORK/key.pem" -out "$WORK/cert.pem" \
  -subj "/CN=$NAME/O=D1 Preservation Zeek Control" \
  -addext "subjectAltName=DNS:$NAME" >/dev/null 2>&1

cat > "$WORK/dnsd.py" <<'PYEOF'
"""A four-record DNS server. Not a resolver: it answers one name and exits."""
import socket, struct, sys

NAME, ADDR, PORT = sys.argv[1], sys.argv[2], int(sys.argv[3])
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("127.0.0.1", PORT))
sock.settimeout(20)
print("ready", flush=True)
try:
    query, peer = sock.recvfrom(4096)
except socket.timeout:
    sys.exit(3)
qname = b"".join(bytes([len(p)]) + p.encode() for p in NAME.split(".")) + b"\x00"
answer = (
    query[:2]                                   # same transaction id
    + b"\x81\x80"                               # QR=1 RD=1 RA=1 rcode=0
    + b"\x00\x01\x00\x01\x00\x00\x00\x00"       # 1 question, 1 answer
    + qname + b"\x00\x01\x00\x01"               # the question, echoed
    + b"\xc0\x0c"                               # name pointer to offset 12
    + b"\x00\x01\x00\x01"                       # type A, class IN
    + struct.pack(">I", 60)                     # ttl
    + b"\x00\x04" + socket.inet_aton(ADDR)      # rdlength 4 + the address
)
sock.sendto(answer, peer)
PYEOF

cat > "$WORK/dnsq.py" <<'PYEOF'
"""Send one A query and wait for the answer, so the capture contains a complete transaction."""
import socket, sys
NAME, PORT = sys.argv[1], int(sys.argv[2])
qname = b"".join(bytes([len(p)]) + p.encode() for p in NAME.split(".")) + b"\x00"
packet = b"\xd1\x01\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00" + qname + b"\x00\x01\x00\x01"
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(10)
sock.sendto(packet, ("127.0.0.1", PORT))
reply, _ = sock.recvfrom(4096)
print(f"dns answer {len(reply)} bytes", flush=True)
PYEOF

# ---------------------------------------------------------------------------------------
# Everything below this line runs INSIDE the sealed namespace.  `unshare -r` maps us to uid 0
# in a new user namespace (no sudo, no setuid), and `-n` gives that namespace its own network
# stack whose only device is a down loopback.
# ---------------------------------------------------------------------------------------
env WORK="$WORK" OUT="$OUT" NAME="$NAME" unshare -r -n bash -euo pipefail -c '
  ip link set lo up

  # tcpdump would drop privileges to a "tcpdump" user that does not exist in this user
  # namespace; -Z root keeps it as the namespace root it already is.
  tcpdump -i lo -s0 -U -Z root -w "$OUT" >"$WORK/tcpdump.log" 2>&1 &
  TCPDUMP=$!
  for _ in $(seq 60); do [ -s "$OUT" ] && break; sleep 0.1; done

  python3 -u "$WORK/dnsd.py" "$NAME" 127.0.0.1 53 >"$WORK/dnsd.log" 2>&1 &
  DNSD=$!
  for _ in $(seq 60); do grep -q ready "$WORK/dnsd.log" 2>/dev/null && break; sleep 0.1; done

  # No -naccept: a readiness probe would consume the single accept and leave the real client
  # talking to a dead port, which is how the first draft of this script produced a 1 KB pcap
  # and an empty ssl.log.
  openssl s_server -accept 443 -tls1_2 -www \
    -cert "$WORK/cert.pem" -key "$WORK/key.pem" >"$WORK/sserver.log" 2>&1 &
  SSERVER=$!
  for _ in $(seq 100); do
    grep -q ACCEPT "$WORK/sserver.log" 2>/dev/null && break
    sleep 0.1
  done

  python3 -u "$WORK/dnsq.py" "$NAME" 53
  wait $DNSD || true

  # The cert is self-signed on purpose (no CA anywhere in this namespace), so verification
  # fails and that is fine - Zeek reads the certificate off the wire either way.  What must
  # NOT happen is the client aborting before the Certificate message, hence no
  # -verify_return_error.
  printf "GET / HTTP/1.0\r\n\r\n" | \
    openssl s_client -tls1_2 -servername "$NAME" -connect 127.0.0.1:443 \
      -no_ign_eof >"$WORK/sclient.log" 2>&1 || true
  kill $SSERVER 2>/dev/null || true
  wait $SSERVER 2>/dev/null || true

  sleep 0.7
  kill -INT $TCPDUMP 2>/dev/null || true
  wait $TCPDUMP 2>/dev/null || true
  cat "$WORK/tcpdump.log" >&2
'

# tcpdump reports "N packets captured" on stderr; a control pcap with no packets is a failure
# we refuse to hand downstream rather than let it look like an empty result.
if [ ! -s "$OUT" ]; then
  echo "zeek-control-capture: $OUT is empty - the control did not capture" >&2
  exit 2
fi
BYTES=$(stat -c %s "$OUT")
if [ "$BYTES" -lt 2000 ]; then
  echo "zeek-control-capture: $OUT is only $BYTES bytes - TLS handshake almost certainly missing" >&2
  exit 2
fi
echo "control pcap: $OUT ($BYTES bytes)"
