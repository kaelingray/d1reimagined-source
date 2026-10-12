#!/usr/bin/env bash
# The signon experiment: answer the signon names locally and watch what Destiny does.
#
# WHY THIS SHAPE
#
# `sku_config` supports signon URL overrides, but the file is RSA-2048 signed, so we cannot edit it.
# We do not have to: the eboot carries `signon.deadorbit.net` / `signon.gravityshavings.net` as string
# literals, and **inside the sealed namespace we own name resolution.** Point those names at ourselves
# and the client redirects with no file edit and no signature to forge.
#
# Name resolution is done with a bind-mounted /etc/hosts rather than a DNS server, because the
# namespace's nsswitch is already `hosts: files dns` - so /etc/hosts is consulted first and needs no
# daemon. Simpler thing that works.
#
# CONTAINMENT IS UNCHANGED. Answering is not egress: everything still lives in an unprivileged
# net+mount namespace with no route off the box, and the assertion still runs before anything starts.
# What changes is that inside the sealed box, something is now listening.
#
# REACH TAGGING: anything learned here is `synthetic-minimal` at best, and everything downstream of
# it inherits `synthetic_ancestor`. A TLS listener of ours answering is not evidence about Bungie's
# signon service - it is evidence about what our client will *attempt*.
set -euo pipefail

GAME="${1:?usage: run-signon-experiment.sh <eboot.bin path> [run-name]}"
RUN_NAME="${2:-signon}"

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EMU_DIR="${SHADPS4_DIR:-$HOME/dev/shadPS4-main}"
EMU_BIN="${SHADPS4_BIN:-$EMU_DIR/build/shadps4}"
SERVER_BIN="${SHADNET_BIN:-$REPO/d1-server/target/release/shadnet-server}"
PATCH="${DIAG_PATCH:-$REPO/patches/CUSA00219-guest-diagnostics.xml}"

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${RUN_OUT:-$REPO/runs/$STAMP-$RUN_NAME}"
mkdir -p "$OUT"

MODE="${D1_LAUNCH_MODE:-native}"
[[ "$MODE" == native || "$MODE" == server-check ]] || { echo "Invalid delegate mode" >&2; exit 2; }
required=("$SERVER_BIN" "$REPO/d1-server/target/release/d1-responder")
if [[ "$MODE" == native ]]; then
    required+=("$EMU_BIN" "$GAME")
else
    required+=("$REPO/d1-server/target/release/d1-platform")
fi
for f in "${required[@]}"; do
    [[ -e "$f" ]] || { echo "missing: $f" >&2; exit 1; }
done
# Close any previous instance and WAIT for it to actually exit.
#
# ⚠ `pgrep -x shadps4` DOES NOT WORK and silently found nothing. shadPS4 renames its main thread, so
# the process comm is **`shadPS4:Main`** -- verified with `ps -eo comm` against a live instance. That
# is why two emulators were alive at once earlier: the guard was a no-op, not a race.
#
# Match the command line instead. The `[s]` character class keeps the pattern from matching the shell
# that is evaluating it -- the same trap that made an earlier `pkill -f "shadps4 -g"` kill its own
# shell and swallow the output.
emu_pids() { pgrep -f "[s]hadps4 -p " 2>/dev/null || true; }
if [[ "$MODE" == native && -n "$(emu_pids)" ]]; then
    echo "closing previous shadps4 instance ($(emu_pids | tr '\n' ' '))"
    kill $(emu_pids) 2>/dev/null || true
    for _ in $(seq 1 40); do [[ -z "$(emu_pids)" ]] && break; sleep 0.5; done
    [[ -n "$(emu_pids)" ]] && { kill -9 $(emu_pids) 2>/dev/null || true; sleep 2; }
    [[ -n "$(emu_pids)" ]] && { echo "could not close previous instance" >&2; exit 1; }
    echo "previous instance closed"
fi

# ── Fail-closed resolver files, plus the hosts file that does the redirect ────────────────────────
printf 'nameserver 127.0.0.1\noptions timeout:1 attempts:1\n' > "$OUT/resolv.conf"
sed 's/^hosts:.*/hosts: files dns/' /etc/nsswitch.conf > "$OUT/nsswitch.conf"
{
    echo "127.0.0.1   localhost"
    echo "::1         localhost"
    # Redirect to our non-loopback address, not 127.0.0.1: the client validates that these look like
    # real remote hosts, and rejects loopback ("none were valid").
    IP="${D1_IP:-10.77.0.2}"
    # The two signon domains, their web tier, and the STUN names. All of these are literals in the
    # eboot; pointing them here is the entire redirect.
    for d in deadorbit gravityshavings; do
        echo "$IP   signon.$d.net"
        echo "$IP   signon2.$d.net"
        echo "$IP   cdn.$d.net"
        echo "$IP   upload.$d.net"
        for i in 1 2 3 4 5 6 7 8; do
            echo "$IP   www$i.signon.$d.net"
            echo "$IP   stun$i.signon.$d.net"
        done
    done
    for r in us eu jp au; do echo "$IP   stun.$r.demonware.net"; done
    # Discovered from the client's own HTTP calls:
    #   auth.api.np.ac.playstation.net  -> PSN OAuth token exchange (retrieveSCEAccessToken -> 715)
    #   srv.shadps4.net                 -> shadNet's HTTP WebAPI, separate from our socket protocol
    echo "$IP   auth.api.np.ac.playstation.net"
    echo "$IP   auth.api.sp-int.playstation.net"
    echo "$IP   srv.shadps4.net"
    # The REST of the eboot's service hostname literals. There are 26 in total; we were covering 22,
    # and the missing `dm-partnernet.upload.deadorbit.net` cost a whole diagnosis detour -- the game
    # POSTs its own crash report to :32556/ticket_drop, and when that host did not resolve the upload
    # failed with `bdHTTPWrapper-ps4.cpp:665 Failed to get HTTP status code`, which looked like the
    # cause of the halt when it was only a consequence. Redirect ALL of them.
    #   enumerate with:  strings-style scan for [host].(net|com) over eboot.bin
    echo "$IP   dm-partnernet.upload.deadorbit.net"
    echo "$IP   help.bungie.net"
    echo "$IP   www.bungie.net"
    # Not an eboot literal -- we invent it, and the client accepts it because the manifest URL is
    # entirely server-chosen (signon f4.3 key 100). Kept here so the CDN fetch resolves.
    echo "$IP   cdn.deadorbit.net"
} > "$OUT/hosts"

# ── A throwaway self-signed cert. Whether the client accepts it IS part of the experiment ─────────
if [[ ! -f "$OUT/tls.pem" ]]; then
    openssl req -x509 -newkey rsa:2048 -nodes -keyout "$OUT/tls.key" -out "$OUT/tls.crt" \
        -days 2 -subj "/CN=signon.deadorbit.net" \
        -addext "subjectAltName=DNS:signon.deadorbit.net,DNS:signon.gravityshavings.net,DNS:www1.signon.deadorbit.net,DNS:www1.signon.gravityshavings.net" \
        >/dev/null 2>&1
    cat "$OUT/tls.crt" "$OUT/tls.key" > "$OUT/tls.pem"
fi

{
    echo "run:             $STAMP-$RUN_NAME"
    echo "experiment:      signon redirect via /etc/hosts + local TLS listener"
    echo "game:            $GAME"
    echo "emulator:        $EMU_BIN"
    echo "emulator commit: $(git -C "$EMU_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "diag patch:      $PATCH"
    echo "reach:           synthetic-minimal (everything downstream: synthetic_ancestor)"
    echo "bap-advertised:  ${D1_BAP_IP:-10.77.0.2}:${D1_BAP_PORT:-3075}  (NOT 3074 -- the guest binds that itself)"
    echo "mode:            $MODE (server-check never starts a game or emulator)"
    echo "containment:     unshare -rnm, loopback plus isolated dummy, no default route, DNS fail-closed"
    echo "core-dumps:      disabled (ulimit -c 0)"
    echo "loadavg-pre:     $(cut -d' ' -f1-3 /proc/loadavg)"
    echo "psi-cpu-pre:     $(awk '/^some/{print $2,$3,$4}' /proc/pressure/cpu)"
    echo "psi-io-pre:      $(awk '/^some/{print $2,$3,$4}' /proc/pressure/io)"
    echo "psi-mem-pre:     $(awk '/^some/{print $2,$3,$4}' /proc/pressure/memory)"
} > "$OUT/RUN-INFO.txt"

# Destiny writes its OWN crash report on RELEASE_VHALT -- halt reason in prose, engine state, and an
# active_packages.txt -- into /temp0/, which shadPS4 maps to its temp dir on the host. That is the
# cheapest cause-of-death signal in the project (it named `failed to load bootstrap_patchable`
# outright, where lowering diagnostic thresholds to T=2 produced nothing). Preserve it per run:
# shadPS4 may clean temp, and `d1` parses runs/<run>/guest-crash-report/crash_info.txt automatically.
preserve_guest_crash_report() {
    local base="$HOME/.local/share/shadPS4/temp/CUSA00219/temp/crash_folder/orbis"
    [[ -d "$base" ]] || return 0
    local newest
    newest="$(ls -td "$base"/*/ 2>/dev/null | head -1)"
    [[ -n "$newest" ]] || return 0
    # Only if it is from THIS run, so a stale report is never attributed to a run that did not halt.
    if [[ -n "$(find "$newest" -maxdepth 1 -newer "$OUT/RUN-INFO.txt" -print -quit 2>/dev/null)" ]]; then
        mkdir -p "$OUT/guest-crash-report"
        cp -r "$newest"/* "$OUT/guest-crash-report/" 2>/dev/null || true
        echo "guest crash report preserved -> $OUT/guest-crash-report/"
        grep -A1 "^halt information:" "$OUT/guest-crash-report/crash_info.txt" 2>/dev/null | tail -1
    fi
}

record_post() {
    if [[ "$MODE" == native ]]; then preserve_guest_crash_report; fi
    {
        echo "loadavg-post:    $(cut -d' ' -f1-3 /proc/loadavg)"
        echo "psi-cpu-post:    $(awk '/^some/{print $2,$3,$4}' /proc/pressure/cpu)"
        echo "psi-io-post:     $(awk '/^some/{print $2,$3,$4}' /proc/pressure/io)"
        echo "psi-mem-post:    $(awk '/^some/{print $2,$3,$4}' /proc/pressure/memory)"
    } >> "$OUT/RUN-INFO.txt"
}
trap record_post EXIT INT TERM

# ⚠⚠ REFUSE A RUN WITH NO CONTENT MANIFEST (added 2026-08-08, after it cost a run for the SECOND
# time - the first is already written up in status/LESSONS.md).
#
# With D1_MANIFEST_FILE unset, d1-responder falls back to a 2-byte manifest. The client accepts it,
# registers zero packages, and then dies ~90 s later in bootflow:bap_signin with
#     halt: RELEASE_VHALT      halt information: failed to load bootstrap_patchable
# That reads as a package/BAP regression and is actually a missing environment variable. The 2-byte
# body is only ever wanted as a deliberate ablation, so make it opt-in and loud.
#
# ⇒ Prefer capture-rig/run-boot.sh, which pins this and every other known-good knob. This guard
#   exists because this script is the one people reach for when they want to vary ONE thing.
#
# ⚠⚠ THIS BLOCK MUST STAY ABOVE THE `unshare` BELOW, AND MUST CONTAIN NO APOSTROPHES.
# The inner script is a single-quoted `bash -c` string. An apostrophe placed inside it terminates
# the quote, shifts every positional argument, and the failure surfaces as a bind-mount error
# naming a path fragment of your own comment text. That cost a run on 2026-08-08.
if [[ -z "${D1_MANIFEST_FILE:-}" && "${D1_ALLOW_EMPTY_MANIFEST:-0}" != "1" ]]; then
    DEFAULT_MANIFEST="$REPO/capture-rig/manifests/full_en.bin"
    if [[ -r "$DEFAULT_MANIFEST" ]]; then
        export D1_MANIFEST_FILE="$DEFAULT_MANIFEST"
        echo "D1_MANIFEST_FILE was unset -> defaulting to $DEFAULT_MANIFEST ($(stat -c%s "$DEFAULT_MANIFEST") B)"
    else
        echo "REFUSING: D1_MANIFEST_FILE is unset and $DEFAULT_MANIFEST is missing." >&2
        echo "  A 2-byte manifest halts the client in bap_signin on bootstrap_patchable." >&2
        echo "  Build one with 'd1-pkg manifest', or set D1_ALLOW_EMPTY_MANIFEST=1." >&2
        exit 1
    fi
fi
if [[ -n "${D1_MANIFEST_FILE:-}" && ! -r "$D1_MANIFEST_FILE" ]]; then
    echo "REFUSING: D1_MANIFEST_FILE=$D1_MANIFEST_FILE is not readable." >&2
    exit 1
fi

echo "run dir: $OUT"
export D1_OUTER_NETNS="$(readlink /proc/self/ns/net)"

set +e
unshare -rnm bash -c '
set -euo pipefail
OUT="$1"; SERVER_BIN="$2"; EMU_DIR="$3"; EMU_BIN="$4"; GAME="$5"; PATCH="$6"; REPO="$7"
OWNED=()
cleanup_owned() {
    local pid
    for pid in "${OWNED[@]}"; do
        if [[ "$(readlink /proc/$pid/ns/net 2>/dev/null)" == "$(readlink /proc/self/ns/net)" ]]; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    for _ in $(seq 1 20); do
        local alive=0
        for pid in "${OWNED[@]}"; do kill -0 "$pid" 2>/dev/null && alive=1; done
        [[ "$alive" == 0 ]] && break
        sleep 0.1
    done
    for pid in "${OWNED[@]}"; do
        if [[ "$(readlink /proc/$pid/ns/net 2>/dev/null)" == "$(readlink /proc/self/ns/net)" ]]; then
            kill -9 "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
    done
}
trap cleanup_owned EXIT
trap "exit 130" INT
trap "exit 143" TERM

mount --bind "$OUT/resolv.conf"   /etc/resolv.conf
mount --bind "$OUT/nsswitch.conf" /etc/nsswitch.conf
mount --bind "$OUT/hosts"         /etc/hosts
ip link set lo up

# ── A plausible non-loopback network ──────────────────────────────────────────────────────────────
# WHY: with loopback only, the client reported `Client local address: [127.0.0.1:65535]`, rejected the
# STUN addresses ("none were valid"), and the boot flow died with `error.transport_method_invalid` at
# bootflow:platform_signin -- which is the Marionberry the operator sees. The client wants a real
# interface with a routable-looking address before it considers its transport usable.
#
# CONTAINMENT IS UNAFFECTED: this namespace has no veth to the host and no physical interface, so
# there is still literally no path off the box. The default route points at a gateway that does not
# exist; packets to it are discarded locally. Asserted below regardless.
ip link add d1net type dummy
ip addr add "${D1_IP:-10.77.0.2}/24" dev d1net
ip link set d1net up
# NO default route, deliberately. A dummy interface accepts and discards, so a default route through
# a nonexistent gateway makes off-subnet connects hang until timeout instead of failing fast with
# ENETUNREACH -- which stalled the boot flow into a black screen on the first attempt. Fail-fast is
# what we want: the client should learn immediately that a host is unreachable, and reach OUR listeners
# on-subnet instantly.

# Containment first, and it still has to pass. Note the redirect target is loopback, so a
# *resolution* now succeeds by design; what must remain impossible is a route off the box.
{
    echo -n "tcp-out:      "; timeout 3 bash -c "echo > /dev/tcp/1.1.1.1/443" 2>/dev/null && echo "REACHABLE - ABORT" || echo "no route"
    echo -n "dns-upstream: "; timeout 3 getent hosts one.one.one.one >/dev/null 2>&1 && echo "RESOLVED - ABORT" || echo "fails closed"
    echo -n "signon-name:  "; getent hosts signon.deadorbit.net | head -1
    echo -n "local-addr:   "; ip -4 -br addr show dev d1net | awk "{print \$3}"
    echo -n "interfaces:   "; ip -br link | awk "{printf \"%s \", \$1}"; echo
} | tee "$OUT/containment.txt"
grep -q ABORT "$OUT/containment.txt" && { echo "CONTAINMENT FAILED - refusing" >&2; exit 1; }

ulimit -c 0

"$SERVER_BIN" --bind 127.0.0.1:31313 > "$OUT/shadnet.log" 2>&1 &
SHAD_PID=$!; OWNED+=("$SHAD_PID")

# The answering surface: logs every request and answers the two endpoints the client actually calls
# (PSN OAuth on TLS 443, shadNet WebAPI on 31315), 404-ing everything else so an unimplemented
# endpoint becomes a NAMED GATE rather than a hang. Reach tag: synthetic-generic.
D1_SIGNON_HEX="${D1_SIGNON_HEX:-}" D1_MANIFEST_FILE="${D1_MANIFEST_FILE:-}" \
    D1_UPLOAD_DIR="${D1_UPLOAD_DIR:-$OUT/uploads}" \
    "$REPO/d1-server/target/release/d1-responder" --bind 0.0.0.0 \
    --cert "$OUT/tls.crt" --key "$OUT/tls.key" > "$OUT/responder.log" 2>&1 &
RESPONDER_PID=$!; OWNED+=("$RESPONDER_PID")

if [[ "${D1_LAUNCH_MODE:-native}" == server-check ]]; then
    "$REPO/d1-server/target/release/d1-platform" verify-servers --out "$OUT" \
        --outer-netns "$D1_OUTER_NETNS" --shad-pid "$SHAD_PID" --responder-pid "$RESPONDER_PID"
    exit $?
fi

# ── THE ACTIVITY HOST -- rung 30 stalls forever without it ────────────────────────────────────────
# setup:activity_host_setup sends BAP message 8 (the join request, carrying a nonce) and waits for a
# join_result. d1-activity-host answers exactly that: 25 controls pass, including
# "join_request -> join_result echoes the nonce". It is a SEPARATE listener and must live inside this
# namespace to be reachable at the address BAP message 17 hands out.
# OPT-IN, and deliberately so: every run that reached orbit did it without this process, so a boot
# regression must never be attributable to a component the proven route never had.
# ⛔ CEILING, from the crate control that states it: a stub Activity Host yields a LOADED BUT EMPTY
# world -- no spawns, no encounters, no objectives. The mission script is cloud-side content.
if [[ "${D1_ACTIVITY_HOST:-0}" == "1" ]]; then
    # D1_AH_RAW_CODEC=1 -- an EXPERIMENT the crate provides on purpose, not the protocol.
    # AH payloads are schema-serialised through FUN_002A4090; the default codec REFUSES rather than
    # fabricate a raw-struct encoding. The raw codec exists to falsify that in one run: if the
    # client accepts a raw struct, the schema requirement is wrong; if it rejects it, we have paid
    # one boot to learn the codec is mandatory. Either way the run log says which.
    AH_CODEC_FLAG=""
    if [[ "${D1_AH_RAW_CODEC:-0}" == "1" ]]; then AH_CODEC_FLAG="--raw-struct-codec"; fi
    "$REPO/d1-server/target/release/d1-activity-host" serve \
        --bind "0.0.0.0:${D1_AH_BIND_PORT:-4110}" $AH_CODEC_FLAG \
        > "$OUT/activity-host.log" 2>&1 &
    OWNED+=("$!")
    echo "activity-host:   ENABLED on port ${D1_AH_BIND_PORT:-4110}" >> "$OUT/RUN-INFO.txt"
fi
# Record which local sockets appear, as a cheap stand-in for pcap (CAP_NET_RAW is usually absent in
# an unprivileged userns, so tcpdump would silently record nothing).
( for _ in $(seq 1 60); do ss -tuanp 2>/dev/null; echo "--- $(date +%s)"; sleep 5; done ) > "$OUT/sockets.log" 2>&1 &
OWNED+=("$!")

for _ in $(seq 1 50); do (exec 3<>/dev/tcp/127.0.0.1/31313) 2>/dev/null && break; sleep 0.1; done

cd "$EMU_DIR"
# Run on the virtual display when DRIVE=1 so xdotool can press buttons and `import` can grab frames.
# Wayland has no equivalent no-privilege input-injection path, and this is what makes the loop
# autonomous: press X, screenshot, read, decide.
if [[ "${DRIVE:-0}" == "1" ]]; then
    # THE PAD LAW (hindsight skills/sdl-controller-remap): an agent-driven emulator must NOT claim
    # the physical controller belonging to the operator -- it would forge input verdicts and hijack a
    # live session. The 0xFFFF/0xFFFF convention is a no-op (it is a VID/PID match list), so build the
    # list from real devices. Keyboard input via xdotool needs no pad at all.
    # NOTE: no apostrophes anywhere below -- this block is inside bash -c with single quotes.
    DISPLAY="${D1_DISPLAY:-:99}" SDL_VIDEODRIVER=x11 \
        SDL_GAMECONTROLLER_IGNORE_DEVICES="${SDL_IGNORE_PADS:-}" \
        "$EMU_BIN" -p "$PATCH" -g "$GAME" > "$OUT/emulator.log" 2>&1 &
    EMU_PID=$!
    OWNED+=("$EMU_PID")
    # Boot takes ~40s to the title screen; then press X (Enter) on a cadence and grab a frame each
    # time so the sequence is auditable after the fact.
    sleep 55
    for i in 1 2 3 4 5 6; do
        DISPLAY="${D1_DISPLAY:-:99}" import -window root "$OUT/frame-$i-pre.png" 2>/dev/null || true
        DISPLAY="${D1_DISPLAY:-:99}" xdotool key --clearmodifiers Return 2>/dev/null || true
        sleep 12
        DISPLAY="${D1_DISPLAY:-:99}" import -window root "$OUT/frame-$i-post.png" 2>/dev/null || true
        kill -0 $EMU_PID 2>/dev/null || break
    done
    wait $EMU_PID 2>/dev/null || true
else
    # Visible on the operator monitor, and driven by the uinput virtual pad rather than xdotool.
    # WHY the pad: xdotool key needs a focus target, and Xvfb has no window manager (no
    # _NET_ACTIVE_WINDOW) while Wayland has no unprivileged injection path at all -- the earlier
    # autonomous attempt produced zero SignOn requests for exactly that reason. /dev/uinput creates a
    # real kernel evdev device, so SDL sees it on Wayland, on X11 and headless. Start
    # capture-rig/virtual-pad.py BEFORE this script: udev hotplug is netlink, and netlink IS
    # network-namespaced, so a pad created after we enter the netns is never announced to the
    # emulator. A pre-existing device is found via sysfs, which is not namespaced.
    #
    # THE PAD LAW: ignore every PHYSICAL pad by real VID/PID so the operator keeps their controller
    # and slot 0 goes to ours. `virtual-pad.py --print-ignore` generates this value.
    # RENDERDOC needs XWAYLAND -- and the desktop session is NOT the thing that changes.
    # RenderDoc does not support NATIVE WAYLAND windowing: its own CMake gates it behind
    # ENABLE_UNSUPPORTED_EXPERIMENTAL_POSSIBLY_BROKEN_WAYLAND, off by default, and shadPS4 warns
    # about it in vk_platform.cpp. The operator stays logged into Wayland; only THIS window is an
    # X11 client under XWayland, which the compositor displays normally.
    # D1_RENDERDOC=1 selects that path. It must be set AT LAUNCH: Vulkan capture is a LAYER and the
    # layer chain is fixed at vkCreateInstance, so an already-running client can never be injected
    # into afterwards -- renderdoccmd inject cannot retrofit it. Measured 2026-08-10.
    # TWO INDEPENDENT DECISIONS, TWO KNOBS. They were welded together until 2026-08-10 and that was
    # a design error: D1_RENDERDOC=1 was the ONLY way to reach XWayland, so anyone who wanted the
    # X11 path had to also load a capture layer they did not want.
    #   D1_X11=1        -- run as an X11 client under XWayland. Nothing else changes.
    #   D1_RENDERDOC=1  -- load the RenderDoc capture layer. IMPLIES D1_X11 (RenderDoc does not
    #                      support native Wayland windowing; its own CMake gates that behind
    #                      ENABLE_UNSUPPORTED_EXPERIMENTAL_POSSIBLY_BROKEN_WAYLAND, off by default).
    # ⭐ WHY ANYONE WOULD WANT D1_X11 ALONE: the operator reports the XWayland path is noticeably
    #   more responsive, with or without RenderDoc attached. [O] -- observed, not measured; no frame
    #   timing has been taken either way. It is recorded as a preference, not as a benchmark.
    # ⚠ The desktop session is UNCHANGED either way. This makes shadPS4 an X11 client under
    #   XWayland; the operator stays logged into Wayland.
    USE_X11="${D1_X11:-0}"
    if [[ "${D1_RENDERDOC:-0}" == "1" ]]; then USE_X11=1; fi

    if [[ "$USE_X11" == "1" ]]; then
        XAUTH=$(ls /run/user/1001/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
        RDOC_LIB=""
        if [[ "${D1_RENDERDOC:-0}" == "1" ]]; then
            # Fedora ships librenderdoc.so in /usr/lib64/renderdoc, which is NOT in the ldconfig
            # cache; shadPS4 does a plain dlopen(librenderdoc.so) and fails with
            # "cannot open shared object file". Measured 2026-08-10. Hence LD_LIBRARY_PATH.
            RDOC_LIB="/usr/lib64/renderdoc:"
            echo "video:    XWAYLAND DISPLAY=${D1_X_DISPLAY:-:0} + RENDERDOC layer (desktop session unchanged)"
        else
            echo "video:    XWAYLAND DISPLAY=${D1_X_DISPLAY:-:0} (D1_X11=1; no capture layer; desktop session unchanged)"
        fi
        DISPLAY="${D1_X_DISPLAY:-:0}" XAUTHORITY="$XAUTH" SDL_VIDEODRIVER=x11 \
            LD_LIBRARY_PATH="${RDOC_LIB}${LD_LIBRARY_PATH:-}" \
            XDG_RUNTIME_DIR=/run/user/1001 \
            SDL_GAMECONTROLLER_IGNORE_DEVICES="${SDL_IGNORE_PADS:-}" \
            "$EMU_BIN" -p "$PATCH" -g "$GAME" ${D1_FULLSCREEN:+-f true} \
                > "$OUT/emulator.log" 2>&1 || true
    else
        echo "video:    NATIVE WAYLAND (set D1_X11=1 for the XWayland path)"
        XDG_RUNTIME_DIR=/run/user/1001 WAYLAND_DISPLAY=wayland-0 SDL_VIDEODRIVER=wayland \
            SDL_GAMECONTROLLER_IGNORE_DEVICES="${SDL_IGNORE_PADS:-}" \
            "$EMU_BIN" -p "$PATCH" -g "$GAME" ${D1_FULLSCREEN:+-f true} \
                > "$OUT/emulator.log" 2>&1 || true
    fi
fi

' _ "$OUT" "$SERVER_BIN" "$EMU_DIR" "$EMU_BIN" "$GAME" "$PATCH" "$REPO"
rc=$?
set -e
echo "exit-status:     $rc" >> "$OUT/RUN-INFO.txt"
exit "$rc"
