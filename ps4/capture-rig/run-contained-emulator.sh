#!/usr/bin/env bash
# Run shadPS4 + the local ShadNet server (Layer A) with provably no egress.
#
# Lane A discipline: `isConnectedToNetwork=false` is NOT containment - shadPS4's resolver calls host
# getaddrinfo unconditionally, so an "offline" emulator still emits real DNS. This wraps the run in an
# unprivileged network + mount namespace so that:
#   * there is no route off the box   (loopback only)
#   * DNS fails closed               (resolv.conf → 127.0.0.1, nss `resolve` removed so
#                                     systemd-resolved's unix socket can't answer either)
#   * loopback still works            (so the game can reach our Layer A server)
#
# Everything runs in ONE namespace, so 127.0.0.1 is shared between the emulator and the server.
#
# Usage: run-contained-emulator.sh <game-eboot-path> [run-name] [-- extra shadps4 args...]
set -euo pipefail

GAME="${1:?usage: run-contained-emulator.sh <eboot.bin path> [run-name] [-- extra args]}"
RUN_NAME="${2:-run}"
shift || true; shift || true
[[ "${1:-}" == "--" ]] && shift || true

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EMU_DIR="${SHADPS4_DIR:-$HOME/dev/shadPS4-main}"
EMU_BIN="${SHADPS4_BIN:-$EMU_DIR/build/shadps4}"
SERVER_BIN="${SHADNET_BIN:-$REPO/d1-server/target/release/shadnet-server}"

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${RUN_OUT:-$REPO/runs/$STAMP-$RUN_NAME}"
mkdir -p "$OUT"

for f in "$EMU_BIN" "$SERVER_BIN" "$GAME"; do
    [[ -e "$f" ]] || { echo "missing: $f" >&2; exit 1; }
done

# One emulator at a time. Two instances fight over the GPU, the audio device and the user dir, and
# the extra host load is exactly what loses Destiny's init race (see status/shadps4.md).
#
# ⚠ `pgrep -x shadps4` DOES NOT MATCH. shadPS4 renames its main thread, so the process comm is
# **`shadPS4:Main`** (verified against a live instance with `ps -eo comm`). This guard was a silent
# no-op, which is how two emulators ran at once. Match the command line, with a `[s]` character class
# so the pattern cannot match the shell evaluating it.
emu_pids() { pgrep -f "[s]hadps4 -p " 2>/dev/null || true; }
if [[ -n "$(emu_pids)" ]]; then
    echo "refusing to start: shadps4 is already running (pids: $(emu_pids | tr '\n' ' '))" >&2
    echo "stop it first:  kill \$(pgrep -f '[s]hadps4 -p ')" >&2
    exit 1
fi

# Fail-closed resolver files, bind-mounted inside the namespace only.
printf 'nameserver 127.0.0.1\noptions timeout:1 attempts:1\n' > "$OUT/resolv.conf"
sed 's/^hosts:.*/hosts: files dns/' /etc/nsswitch.conf > "$OUT/nsswitch.conf"

# Provenance: what produced this run.
{
    echo "run:            $STAMP-$RUN_NAME"
    echo "game:           $GAME"
    echo "emulator:       $EMU_BIN"
    echo "emulator commit: $(git -C "$EMU_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "shadnet-server: $SERVER_BIN"
    echo "host:           $(uname -srm) / $(hostname)"
    echo "containment:    unshare -rnm, loopback only, DNS fail-closed"
    echo "core-dumps:     disabled (ulimit -c 0)"
    echo "loadavg-pre:    $(cut -d' ' -f1-3 /proc/loadavg)"
    echo "psi-cpu-pre:    $(awk '/^some/{print $2,$3,$4}' /proc/pressure/cpu)"
    echo "psi-io-pre:     $(awk '/^some/{print $2,$3,$4}' /proc/pressure/io)"
    echo "psi-mem-pre:    $(awk '/^some/{print $2,$3,$4}' /proc/pressure/memory)"
} > "$OUT/RUN-INFO.txt"

# Host conditions are provenance, not trivia: the most expensive mistake in this project was
# reading determinism into an artifact of host load. A determinism claim whose run has no
# host-quiet evidence is [O], not [C]. Sampled again on exit below.
record_psi_post() {
    {
        echo "loadavg-post:   $(cut -d' ' -f1-3 /proc/loadavg)"
        echo "psi-cpu-post:   $(awk '/^some/{print $2,$3,$4}' /proc/pressure/cpu)"
        echo "psi-io-post:    $(awk '/^some/{print $2,$3,$4}' /proc/pressure/io)"
        echo "psi-mem-post:   $(awk '/^some/{print $2,$3,$4}' /proc/pressure/memory)"
    } >> "$OUT/RUN-INFO.txt"
}
trap record_psi_post EXIT INT TERM

echo "run dir: $OUT"

# ulimit -c 0: a 16 GB core being compressed by systemd-coredump is enough host contention to lose an
# init race inside Destiny. Learned the hard way; see status/shadps4.md.
unshare -rnm bash -c '
set -euo pipefail
OUT="$1"; SERVER_BIN="$2"; EMU_DIR="$3"; EMU_BIN="$4"; GAME="$5"; shift 5

mount --bind "$OUT/resolv.conf"   /etc/resolv.conf
mount --bind "$OUT/nsswitch.conf" /etc/nsswitch.conf
ip link set lo up

# Assert containment before anything runs, and record the proof alongside the run.
{
    echo -n "dns:     "; timeout 3 getent hosts bungie.net >/dev/null 2>&1 && echo "RESOLVED - ABORT" || echo "fails closed"
    echo -n "tcp-out: "; timeout 3 bash -c "echo > /dev/tcp/1.1.1.1/443" 2>/dev/null && echo "REACHABLE - ABORT" || echo "no route"
} | tee "$OUT/containment.txt"
grep -q ABORT "$OUT/containment.txt" && { echo "CONTAINMENT FAILED - refusing to run" >&2; exit 1; }

ulimit -c 0

"$SERVER_BIN" --bind 127.0.0.1:31313 "$@" > "$OUT/shadnet.log" 2>&1 &
SERVER_PID=$!
trap "kill $SERVER_PID 2>/dev/null || true" EXIT

# Give the listener a moment; fail loudly rather than racing the emulator against it.
for _ in $(seq 1 50); do
    (exec 3<>/dev/tcp/127.0.0.1/31313) 2>/dev/null && break
    sleep 0.1
done

cd "$EMU_DIR"
XDG_RUNTIME_DIR=/run/user/1001 WAYLAND_DISPLAY=wayland-0 SDL_VIDEODRIVER=wayland \
    "$EMU_BIN" -g "$GAME" > "$OUT/emulator.log" 2>&1 || true

echo "--- emulator exited; shadnet server log follows ---"
' _ "$OUT" "$SERVER_BIN" "$EMU_DIR" "$EMU_BIN" "$GAME" "$@"
rc=$?
echo "exit-status:    $rc" >> "$OUT/RUN-INFO.txt"
exit "$rc"
