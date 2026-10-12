#!/usr/bin/env bash
# The canonical boot run: `run-signon-experiment.sh` plus THE WHOLE KNOWN-GOOD ENVIRONMENT.
#
# WHY THIS EXISTS
#
# `run-signon-experiment.sh` takes its inputs from the environment, and every one of them defaults to
# "unset" rather than to the last-known-good value.  On 2026-08-07 a run launched without
# `D1_MANIFEST_FILE` served a 2-byte manifest, and the client died at `bootflow:bap_signin` with
#
#     halt information:  failed to load bootstrap_patchable
#
# which reads exactly like a regression in the patch being tested.  It was not.  It was a missing
# environment variable.  **A run's environment is part of the experiment; if it is not pinned in a
# file, it is not reproducible, and a forgotten variable produces a false negative that points at the
# wrong cause.**  (status/LESSONS.md; historicaldocs/design/ATLAS.md §4.6.)
#
# So: every input that the boot needs to reach `investment_signin` lives here, as a default that can
# be overridden per-run but never silently forgotten.
#
#   ./run-boot.sh <run-name> [patch.xml]
#
#   ./run-boot.sh baseline                                        # the reference boot
#   ./run-boot.sh ready-bypass patches/CUSA00219-force-account-ready.xml
#
# Overridable: DIAG_PATCH, D1_206_BODY, D1_MANIFEST_FILE, GAME, SHADPS4_BIN, PRESS (cross taps).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export D1_LAUNCH_MODE=native
case "${1:-}" in
    --plan) export D1_LAUNCH_MODE=plan; shift ;;
    --server-check) export D1_LAUNCH_MODE=server-check; shift ;;
esac
RUN_NAME="${1:?usage: run-boot.sh <run-name> [patch.xml]}"
PATCH_ARG="${2:-}"
[[ "$RUN_NAME" =~ ^[A-Za-z0-9_-]{1,64}$ ]] || { echo "Invalid run name" >&2; exit 2; }
if [[ "$D1_LAUNCH_MODE" != native ]]; then
    export OPERATOR_PAD=1 PRESS=0 D1_ACTIVITY_HOST=0
fi

# ── The pinned environment ────────────────────────────────────────────────────────────────────────
# Each line is a gate that is already passed.  Removing one re-closes that gate.

# The content manifest.  Without it the responder serves a 2-byte body and the client halts at
# bap_signin with `failed to load bootstrap_patchable`.  Requires the unsigned-manifest lab patch.
export D1_MANIFEST_FILE="${D1_MANIFEST_FILE:-$REPO/capture-rig/manifests/full_en.bin}"

# The investment message-206 (`fetch_family`) RESPONSE BODY.
#
# ⭐⭐⭐ 2026-08-10: **THIS PIN IS GONE, DELIBERATELY** - same reason, same week, as the 503 pin below.
# The constant that used to sit here is a **family-4 baseline with flags=0x01, object_count=1**, and
# it was sent in answer to EVERY 206 regardless of what was asked. A census of all 249 responder logs
# says the client has never once asked for family 4: **195 arrivals across 162 runs, every one
# byte-identical `4d 10 00 00 00 00 00 00 10` = `fetch_family{family = 3, key = account}`.**
# (The 4-bit family leaf is biased +1, so `4` on the wire is family 3 - `historicaldocs/design/CREATION-BUILD-ORDER.md`
# §3a's `family = v >> 68` and its "boot=4 / finish=3" table are both wrong and now retracted.)
#
# ⛔ WHY IT IS A HAZARD AND NOT MERELY WRONG. `FUN_0056AF60` treats a delivery whose family is at
# replication state 3 with `flags & 1` as a PURGE to the delivered keep-list (0x56AFE3 `cmp eax,3`,
# 0x56AFE8 `test byte [r15],1`, 0x56B023 `call 0x56B2D0`). At BOOT family 4 is at state 1 so the
# constant is discarded (harmless). At character-creation FINISH - which sends 501 then 206 back to
# back from `cui::c_character_creation_selection_data_component::vfunc[14]` - family 4 is at state 3,
# so the SAME constant is ACCEPTED and trims family 4 to one 9-byte object with `+0x08 = 0` (zero
# selection cards) and `+0x68 = 0` (slot B dead), deleting the profile rung 33 and rung 34 read.
#
# `d1-responder` now DECODES the request and answers the family the client actually asked for, built
# by the same `build_family_body` every other delivery path uses. `D1_206_BODY` still overrides, but
# it is REFUSED with a loud ⛔⛔ line unless the span it carries is for the requested family - so
# re-adding an `export` here would be inert rather than dangerous, and would say so.
# `D1_206_SPAN=0` is the bisect lever: it drops the span entirely. ⚠ It is a BISECT shape, not a
# careful one - over all 249 responder logs, the 144 runs that answered 206 WITH a span produced 82
# arrivals at `setup:orbit` and the 15 that answered without one produced ZERO. A span-carrying 206
# is the proven shape; this change alters which FAMILY the span delivers, not whether there is one.

# The investment-message-503 (signon) RESPONSE BODY.
#
# ⭐⭐⭐ 2026-08-10: **THIS PIN IS GONE, DELIBERATELY.** The value that used to sit here was the
# 45-byte flagless baseline - account SOID + family-5 root and an unlock array of `count = 0`. It
# was the reason every run in project history shipped ZERO unlock flags, and the reason rung 33's
# two walls (baboon `missing_dlc`, urchin `missing_permissions`) had to be cleared by poking ten
# bytes into the derived plane at `[0x26A34E8]+0xC+idx = 0x02`.
#
# `d1-responder` now serves the **ten-flag** body by DEFAULT
# ({98,308,419,440,834,2667,3389,5015,5987,6018} at value 2, `d1_bap::INVESTMENT_503_UNLOCKS_RAW`),
# and splices `D1_ACCOUNT_SOID` into it, so the identity that used to be hardcoded here now follows
# the run. **An `export D1_503_RAW=…` here would silently override that and restore the old
# no-flags behaviour** - env beats default, and the receipt would still look healthy.
#
# ⚠ To go back to the flagless baseline for a bisect, set BOTH halves - 205 lands last and wins:
#   D1_503_RAW=180000000d1000000000000010000000000000000bfffffffffffffffc00000000000000020810220000000100 \
#   D1_205_RAW=1bfffffffffffffffc00000000000000020810220000000100  capture-rig/run-boot.sh …
# The responder prints the decoded flag list at startup either way; read it, do not assume it.
#
# `D1_503_SOID` no longer gates the arm (unset = serve the default); `D1_503_SOID=0` is the explicit
# "answer 503 with nothing" opt-out, kept for bisecting.
export D1_503_SOID="${D1_503_SOID:-1}"
# The account and character SOIDs. Everything downstream is keyed by these: the 503 body above, the
# 206 span body, the registry lookup 0x583450 does, and the QueueZ push. They must all agree.
export D1_ACCOUNT_SOID="${D1_ACCOUNT_SOID:-0xD100000000000001}"
export D1_CHARACTER_SOID="${D1_CHARACTER_SOID:-0xD100000000000101}"

# The QueueZ object push - ON, at `echo`. This is a GATE, not an experiment.
#
# ⚠ These three lines used to read `D1_PUSH_CHARACTER=0` / `D1_PUSH_AT=investment`, justified by a
# measurement (§26.42/§26.43) that is **RETRACTED** - the short BAP transcripts it rested on were
# caused by a missing D1_503_SOID/D1_503_RAW, not by the push (§26.45). Leaving the push off meant
# `investment_signin` had nothing to satisfy it, and the run died at `error.no_investment_connection`
# having passed all ten already-won gates. That looks exactly like a regression and is not one.
#
# TIMING IS THE WHOLE THING (§26.47):
#   relay / investment  push BEFORE the client runs 0x584EB0, so it holds no subscription for the
#                       family root and an unasked-for object is fatal
#   echo                waits for the keepalive loop, which begins only after 503, msg 171, the 206
#                       request and msg 12 - i.e. after the client has SUBSCRIBED and is idle-waiting
#                       for exactly this object. This is the one that clears the gate.
#
#   D1_PUSH_FAMILY=4  the ACCOUNT object -- 0x583450 looks it up to set the ready byte (§26.41),
#                     and it carries the CHARACTER LIST the card builder counts (§26.66)
#   D1_PUSH_FAMILY=3  the ROSTER (family root) + the CHARACTER, as one baseline
#   Order is not free: 0x584EB0 subscribes class 4 first, class 3 second. Push 4 then 3.
export D1_PUSH_CHARACTER="${D1_PUSH_CHARACTER:-1}"
export D1_PUSH_AT="${D1_PUSH_AT:-echo}"
export D1_PUSH_FAMILY="${D1_PUSH_FAMILY:-4,3}"

# ⭐⭐ D1_EMPTY_ROSTER=1 -- THE CHARACTER-CREATION RUN. Default OFF; this is the whole switch.
#
# With it set, `d1-responder` serves family 3 as a roster with ZERO characters and family 4 as an
# **account-only** delivery: `object_count = 1`, no family-4/type-1 profile object, and account
# field 3 (`+0x68`) ABSENT. That last part is what is new on 2026-08-10 and it is the load-bearing
# half - `0x585820` fills `character:signin`'s slot B from `*(account + 0x68)` and a filled slot B
# makes ladder `0x78C5F0` return 6, i.e. "signin is finished", pulling the state machine out from
# under the creator. Every historical creation run (CREATE-ROSTER, CREATE-ANSWER, HOLD-SELECT) died
# exactly there, at `Entering state 'character:signin'`, because we were still shipping the profile.
#
# ⛔ A CREATION ATTEMPT COSTS A BOOT, NOT A POKE. `character:signin` is rung 23, upstream of orbit;
# there is no recovery path, and a failed attempt ends in `error.lost_bap_connection ->
# kick_to_bootflow_start`. Budget ~90 s per attempt and read the STATE line, not the substage.
#
# ⚠ The profile is deliberately WITHHELD while this is on, and the profile is what carries
# `last_orbited_destination` (rung 33) and what rung 34's task 7 reads. It comes back the instant a
# 501 is answered (`CHARACTER_CREATED` flips `serves_no_characters` to false). So an
# `D1_EMPTY_ROSTER=1` run that never completes creation is NOT a run that can reach orbit - do not
# A/B it against a rung-33 change.
#
# ⛔ DELIBERATELY NOT EXPORTED HERE. `export D1_EMPTY_ROSTER="${D1_EMPTY_ROSTER:-}"` - the idiom every
# other optional knob in this file uses - sets it to the EMPTY STRING, and a boolean read of
# `v != "0"` calls that TRUE. That would turn every ordinary boot into a creation run. The responder
# now treats empty as unset (asserted by `d1-responder controls`), but the export is still wrong to
# add: pass it on the command line for the runs that want it.
#
#   D1_EMPTY_ROSTER=1 capture-rig/run-boot.sh CREATE-1

# The character LIFECYCLE replies - investment 501 create / 502 delete / 505 signout / 506 customize.
# **Default ON in d1-responder since 2026-08-10**; set to 0 only to reproduce the old stall.
# Measured safe: across all 249 responder logs, 501 arrived in 8 runs and 505 in 2, and not one of
# those ten contains a single `Entering state 'orbit…'` line - the proven route never sends them.
# It is also the only exit from the creator D1_EMPTY_ROSTER opens.
export D1_CHARACTER_LIFECYCLE="${D1_CHARACTER_LIFECYCLE:-1}"

# ⭐ D1_ANSWER_FAMILIES -- answer a family SUBSCRIPTION, per subscription, with an empty family
# body (object_count=0), written right after the 0x000D ACK.
#
# WHY. Measured on run 20260808-011129: classes 0 and 2 subscribe 56 times EACH, we ACK all 112 and
# deliver nothing, and the client logs
#     networking:investment:peer: Failed peer-subscription validation [error error_timeout]
# 548 times. Its own replication table then reads:
#     [ 0] class=0 state=4 LOST/TIMED-OUT (update REFUSED)
#     [21] class=2 state=4 LOST/TIMED-OUT (update REFUSED)
#     [252..254] class=3,4,5 state=3 REPLICATING (update accepted)
# State 4 REFUSES updates, so nothing delivered later -- and no memory poke -- can recover it.
#
# ⚠ D1_PUSH_FAMILY CANNOT DO THIS. That push is one-shot on a trigger and fires at seq 7; classes
# 0 and 2 do not subscribe until seq 20/21. Listing 0,2 there looks wired and does nothing.
#
# Empty first, deliberately: it separates "the subscription needs an answer at all" from "the
# answer needs contents". We do not yet know either family's instance types.
export D1_ANSWER_FAMILIES="${D1_ANSWER_FAMILIES:-}"

# ⭐ D1_UNLOCKS -- the family-4/type-4 object the director's activity enumerators look up
# (0x574220/0x574340 -> find(family=4, type=4, key=0x7FFFFFFFFFFFFFFE)). We have never sent it.
#   key      key-only, 9 bytes (bfffffffffffffff00 -> 0x7FFFFFFFFFFFFFFE, oracle round-tripped)
#   <path>   a payload file from `d1-oracle object --tag 0x8080102E --obj <3984-byte C struct>`
# Poking the key into a live client (2026-08-08) made the lookup resolve and wired the selection
# manager's +0x20 -- and Earth STILL was not selectable. So this is necessary-or-not-yet-known,
# not sufficient. Default off: the route that reached orbit ran with the object absent.
export D1_UNLOCKS="${D1_UNLOCKS:-}"

# ⭐ D1_FAMILY42 -- family 4/type 2 (schema 0x80800FB2, 72 B, cap 1). Its absence is a MEASURED
# NULL at selection-manager +0x28 on a live orbit client; delivering it fills the slot (verified
# 2026-08-08: +0x28 0x0 -> 0x37f92e2f8). Both enumerator early-exits were replayed and both pass,
# so the manager was being handed a hole rather than bailing out.
export D1_FAMILY42="${D1_FAMILY42:-}"

# ⭐⭐ D1_ACTIVITY_HOST -- the rung-30 deliverable. Launches d1-activity-host inside the namespace
# (see run-signon-experiment.sh) and points BAP messages 7 and 17 at it.
#
# Rung 30 `setup:activity_host_setup` sends BAP msg 8 -- the join request, carrying a nonce -- and
# waits for a join_result that has never existed. Measured twice (runs 20260808-195418 and
# 20260808-203643): the client connects, sends 4 messages, receives 3, then times out into
# error.starting_private_activity_instance_timed_out_waiting_for_join_response (on screen: ELDERBERRY).
#
# ⚠ THE THREE VALUES BELOW MUST AGREE WITH THE AH PROCESS OR THE FAILURE IS SILENT:
#   D1_AH_HOST_TOKEN must equal d1-activity-host FIRST_ACTIVITY_HOST_ID (0xD1A0000000000001).
#     A mismatched echo is dropped without a diagnostic at 0x594E70. The old default
#     (0xD100000000000A01) was invented before the AH crate existed and does NOT match.
#   D1_AH_PORT must be the AH listener port, not the primary BAP port.
#   D1_AH_HOST_TEXT is the SESSION ID, not an address -- d1-activity-host control:
#     "message 7 host_text is the session id, not an address" (udt 10 = {u64 ah_id, char session_id[]}).
#     The responder default formats <ip>:<port> there, which that control says is wrong.
# ⛔ CEILING: a stub AH gives a LOADED BUT EMPTY world -- no spawns, encounters or objectives.
export D1_ACTIVITY_HOST="${D1_ACTIVITY_HOST:-0}"
export D1_AH_BIND_PORT="${D1_AH_BIND_PORT:-4110}"
# ⚠ An EXPERIMENT, not the protocol -- see run-signon-experiment.sh. Off by default.
export D1_AH_RAW_CODEC="${D1_AH_RAW_CODEC:-0}"
if [[ "$D1_ACTIVITY_HOST" == "1" ]]; then
    export D1_AH_HOST_TOKEN="${D1_AH_HOST_TOKEN:-0xD1A0000000000001}"
    export D1_AH_HOST_TEXT="${D1_AH_HOST_TEXT:-d1ah-d1a0000000000001}"
    export D1_AH_PORT="${D1_AH_PORT:-$D1_AH_BIND_PORT}"
fi

# ⭐ The two oracle-encoded full objects. NOT optional in practice:
#  - D1_PROFILE_FILE: without it the responder falls back to the SPARSE profile encoder, which is
#    a DOCUMENTED NEGATIVE (under-length, §26.67 -- the decode "succeeds" and reads zeros past the
#    end, so last_orbited_destination silently lands as 0). Every run on 2026-08-08 before this
#    line existed shipped 126 B instead of 20,403 B and nobody noticed.
#  - D1_CHARACTER_FILE: without it the character is identity-only -- no level, no light, no gear,
#    which is exactly what a blank character on screen looks like.
export D1_PROFILE_FILE="${D1_PROFILE_FILE:-$REPO/capture-rig/profiles/profile-old-russia.bin}"
export D1_CHARACTER_FILE="${D1_CHARACTER_FILE:-}"

# ⚠ THE WEASEL CURE IS NO LONGER SILENCE - THIS DEFAULT IS NOW EMPTY (2026-08-07).
#
# §26.49/§26.51 read `error.lost_bap_connection` as "our header-only reply to 701 caused it" and
# pinned `D1_INV_SILENT=701`. The measurement was real (silence roughly doubled survival) but the
# explanation was wrong, and keeping the silence now BLOCKS the boot.
#
# The client keeps two IN-FLIGHT REQUEST QUEUES (conn+0x1b80, stride 0x198). An entry lives until a
# reply matching `(expected_reply_type, sequence)` arrives, only the queue HEAD is examined, and a
# queue holding a timed-out entry kills the connection (`0xB2D500` ->
# `_connection_failure_timed_out`). 701 rides envelope type 0x000A and policy families 0 and 1 both
# map 0x000A -> 0x000B, so the client ALWAYS has an entry waiting for it. Silence never discharged
# that entry; it only deferred the death - which is why the §26.51 run "held at prepare_for_orbit"
# and then died anyway.
#
# Every message in the queue now has a body derived from the client's own tables:
#   0x000C -> 0x000D   header-only          (policy 13, response payload size 0)
#   0x0015 -> 0x0016   header-only          (protobuf 0x15AB3E0, no required field)
#   0x0020 -> 0x0021   body `20 00`         (protobuf 0x15AB800, field #4 REQUIRED)
#   701    -> 0x000B   body `1800000000`    (RSAT 0x8080189B, via the native encoder)
#   504    -> 0x000B   body `1800000000`    (RSAT 0x80801845, via the native encoder)
# Set D1_INV_SILENT=701,504 to restore the old behaviour and reproduce the negative.
export D1_INV_SILENT="${D1_INV_SILENT:-}"
# ⚠ D1_ACCOUNT_RSAT IS DELIBERATELY NOT SET HERE ANY MORE (2026-08-07).
#
# It used to pin `e88000000000000080` -- presence bit + u64be SOID, and NOTHING ELSE. That payload
# leaves the account object's character list EMPTY, and `0xA110F0` (the selection-card builder)
# reads its count out of THAT object:
#     0a112cd  cmp dword [rbx+8], 0
#     0a112d1  jle 0xa116bb          ; count <= 0 -> build ZERO cards
# which is exactly the "D1Guardian, but no guardian to select" screen. d1-bap now DERIVES the
# payload from the schema (`account_rsat`), carrying the character list, so pinning the old value
# here would silently re-break it. Set it only to override for a specific experiment.
#
#   D1_ACCOUNT_RSAT=<hex>  override the derived account payload
#   D1_NO_ROSTER=1         family 3 sends the character only, not roster+character (the old A/B)

# The emulator: the d1trace fork, which carries the UD2 probe tracer.
export SHADPS4_BIN="${SHADPS4_BIN:-$HOME/dev/shadPS4-d1trace/build-d1trace/shadps4}"

# ⚠ The tracer is INERT unless `D1_ACCOUNT_TRACE_OUT` is set -- d1_account_trace.cpp:403 returns
# early on an unset/empty path and arms nothing, silently. A run without it looks identical to a run
# whose probes never fired. Default it ON, into the run directory, so tracing is never accidentally
# absent. The run directory is stamped at launch time by run-signon-experiment.sh, so resolve it the
# same way here.
export D1_ACCOUNT_TRACE_STAMP="$(date +%Y%m%d-%H%M%S)"
export RUN_OUT="${RUN_OUT:-$REPO/runs/$D1_ACCOUNT_TRACE_STAMP-$RUN_NAME}"
if [[ "$D1_LAUNCH_MODE" != native && -e "$RUN_OUT" ]]; then
    echo "REFUSING existing preparation run: $RUN_OUT" >&2; exit 2
fi
mkdir -p "$RUN_OUT"
export D1_ACCOUNT_TRACE_OUT="${D1_ACCOUNT_TRACE_OUT:-$RUN_OUT/account-trace.bin}"

# The patch.  Default is the lab baseline: diagnostics T=1 + unsigned manifest, no experiment.
export DIAG_PATCH="${DIAG_PATCH:-${PATCH_ARG:-$REPO/patches/CUSA00219-lab-manifest-T1.xml}}"
[[ -n "$PATCH_ARG" ]] && DIAG_PATCH="$(cd "$(dirname "$PATCH_ARG")" && pwd)/$(basename "$PATCH_ARG")"

# ⚠ ABSOLUTE path required: run-signon-experiment.sh `cd`s into the emulator directory before exec,
# so a relative game path becomes "Game ID or file path not found" and the run dies in 2 seconds.
GAME="${GAME:-$REPO/dumps/patch-01.29-CUSA00219/eboot.bin}"
[[ "$GAME" = /* ]] || { echo "GAME must be an absolute path" >&2; exit 1; }

# Input ownership. TWO MODES, and picking the wrong one wastes a boot.
#
#   OPERATOR_PAD=1  the OPERATOR drives with their real controller. Nothing is ignored and the
#                   virtual pad is not required. Use this for anything that needs a MENU -
#                   `bootflow:profile_setup` is a cursor menu (move down, then confirm, plus a
#                   circle-to-dismiss brightness prompt), and blind-pressing X at it just cycles
#                   the boot. A human with a controller does that in three seconds.
#   default         THE PAD LAW: ignore every PHYSICAL pad by real VID/PID so the agent's virtual
#                   pad gets slot 0 and cannot forge input verdicts on the operator's controller.
#                   Use for unattended runs that only need "press X at the title screen".
#
# ⚠ Do not drive the virtual pad while OPERATOR_PAD=1 - two input sources on one menu is how you get
# an unexplained selection.
#
# Display. `D1_FULLSCREEN=1` passes shadPS4's `-f true`.
#
# The stored config is a 1280x720 WINDOW (`/GPU/window_*` in
# ~/.local/share/shadPS4/config.json), which on a 4K panel is a postage stamp. Fullscreen scales
# that 720p output up to the panel; it does NOT change what the guest renders.
#
# ⚠ Do NOT reach for `/GPU/internal_screen_*` to make it sharper without thinking. That is the
# RENDER resolution the guest presents at - changing it introduces a new variable into every run
# and into any screenshot comparison, in the middle of an investigation where we are reading
# on-screen state as evidence. Fullscreen is free; render resolution is not.
export D1_FULLSCREEN="${D1_FULLSCREEN:-}"

# ── GPU: pin the real driver, or shadPS4 renders on the CPU ──────────────────────────────────────
# shadPS4's auto-picker (vk_instance.cpp, gpu_id -1) prefers discrete GPUs, then breaks ties on
# device-local heap size. This box's Radeon is INTEGRATED and llvmpipe (Mesa's CPU rasterizer)
# advertises all of host RAM as its heap, so llvmpipe wins the tiebreak and every non-RenderDoc
# run to date rendered in software (emulator.log: "GPU_Model: llvmpipe" - that was the in-game
# 3-7 fps). Hiding the llvmpipe ICD leaves only RADV to enumerate. Verify per run:
# `grep -m1 GPU_Model runs/<RUN>/emulator.log` must NOT say llvmpipe.
export VK_DRIVER_FILES="${VK_DRIVER_FILES:-/usr/share/vulkan/icd.d/radeon_icd.x86_64.json}"

if [[ "${OPERATOR_PAD:-0}" == "1" ]]; then
    export SDL_IGNORE_PADS=""
    echo "input:    OPERATOR (real controller drives; virtual pad NOT used)"
else
    export SDL_IGNORE_PADS="${SDL_IGNORE_PADS:-$("$REPO/d1-server/target/release/d1-harness" pad print-ignore 2>/dev/null || true)}"
fi

export PRESS="${PRESS:-3}"      # cross taps at the title screen
export PAD_FIFO="${PAD_FIFO:-/tmp/d1pad}"

# ── Preflight: fail loudly, never silently degrade ────────────────────────────────────────────────
fail=0
required=("$D1_MANIFEST_FILE" "$DIAG_PATCH")
if [[ "$D1_LAUNCH_MODE" == native ]]; then
    required+=("$SHADPS4_BIN" "$GAME")
else
    required+=("$REPO/d1-server/target/release/d1-platform")
fi
for f in "${required[@]}"; do
    [[ -e "$f" ]] || { echo "MISSING: $f" >&2; fail=1; }
done
python3 -c "import xml.etree.ElementTree as E,sys; E.parse(sys.argv[1])" "$DIAG_PATCH" \
    || { echo "PATCH IS NOT WELL-FORMED XML: $DIAG_PATCH" >&2; fail=1; }
# The pad must already exist: udev hotplug is netlink and netlink IS network-namespaced, so a pad
# created after the emulator enters the netns is never announced to it.
if [[ "${OPERATOR_PAD:-0}" != "1" ]]; then
    [[ -p "$PAD_FIFO" ]] || { echo "MISSING: $PAD_FIFO - start 'd1-harness pad run --fifo $PAD_FIFO &' FIRST" >&2; fail=1; }
fi
[[ $fail -eq 0 ]] || exit 1
if [[ "$D1_LAUNCH_MODE" != plan ]]; then
    [[ "$(stat -c%s "$D1_MANIFEST_FILE")" == 52797 && "$(sha256sum "$D1_MANIFEST_FILE" | cut -d' ' -f1)" == 0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be ]] || {
        echo "REFUSING: manifest does not match exact 01.29 English pin" >&2; exit 2;
    }
fi
if [[ "$D1_LAUNCH_MODE" == native ]]; then
    [[ "$(stat -c%s "$GAME")" == 29148264 && "$(sha256sum "$GAME" | cut -d' ' -f1)" == 7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954 ]] || {
        echo "REFUSING: executable does not match exact CUSA00219 01.29" >&2; exit 2;
    }
fi

echo "run dir:  $RUN_OUT"
echo "trace:    $D1_ACCOUNT_TRACE_OUT"
echo "patch:    $DIAG_PATCH"
echo "manifest: $D1_MANIFEST_FILE ($(stat -c%s "$D1_MANIFEST_FILE") B)"
# ⚠ `set -u` is on: expand through a default FIRST, or an unset variable aborts the launcher AFTER
# the run directory exists - which reads as a rig failure rather than as a missing export.
_body206="${D1_206_BODY:-}"
if [[ -n "$_body206" ]]; then
    echo "⛔ 206:    D1_206_BODY OVERRIDE SET ($((${#_body206}/2)) B) - the responder will REFUSE it"
    echo "          unless its span is for the family the client asks for. Read the ⛔⛔ line."
else
    echo "206:      fetch_family answered per REQUEST (family decoded, span built by build_family_body)"
    echo "          (D1_206_SPAN=0 restores the pre-2026-08-10 no-span 0x10 reply)"
fi
if [[ -n "${D1_EMPTY_ROSTER:-}" && "${D1_EMPTY_ROSTER}" != "0" ]]; then
    # ⛔⛔ THE BASELINE IS NOT THE ONLY PATH THAT CAN DELIVER A PROFILE, AND THIS BLOCK USED TO ONLY
    # *PRINT*. Measured 2026-08-10, run 20260810-191649-CREATE-1: the responder correctly served
    # "family 4 = account only, no profile" and the client STILL had acct+0x68 = 0xD100000000000201
    # with a live 4/1 object, so 0x585885 and 0x585893 both passed, 0x78C5F0 returned 6, and
    # character:signin exited to character:prologue on normal_flow BEFORE the creator could be used.
    # 501 never went out. The leak was THIS SCRIPT: D1_PROFILE_FILE stayed pinned and
    # D1_PUSH_FAMILY=4,3 pushed the profile back in at `at=echo`.
    # ⭐ THE RULE, the same one that ate rung-34 task 8: when one path withholds a field, EVERY other
    #   path that can assert it must withhold it too. Enumerate; do not spot-check. And a launcher
    #   banner describing intent is a RECEIPT - it proves we meant it, never that the client did not
    #   receive the object by another route.
    export D1_PROFILE_FILE=""    # no 4/1 profile object exists to serve OR to push
    export D1_PUSH_FAMILY="4"    # family 4 = the ACCOUNT (0x583450's ready byte). Drop 3: no character.
    echo "⭐⭐ roster: EMPTY - this is a CHARACTER-CREATION run. family 4 = account only, no profile"
    echo "          object and account+0x68 ABSENT, so character:signin's slot B stays unresolved."
    echo "          ⛔ This run CANNOT reach orbit unless creation completes (501 answered)."
    echo "          ⭐ FORCED for creation: D1_PROFILE_FILE='' and D1_PUSH_FAMILY=4 (was 4,3) - the"
    echo "            push would otherwise re-deliver the profile and close the creator."
else
    echo "roster:   1 character ($D1_CHARACTER_SOID) - the proven route. D1_EMPTY_ROSTER=1 to create."
fi
echo "lifecycle: 501/502/505/506 D1_CHARACTER_LIFECYCLE=$D1_CHARACTER_LIFECYCLE (default 1 since 2026-08-10)"
# ⚠ `set -u` is on: expand through a default FIRST. `${#D1_503_RAW}` on an unset variable aborts the
# whole launcher, and it would do it after the run directory exists - which reads as a rig failure.
_raw503="${D1_503_RAW:-}"; _raw205="${D1_205_RAW:-}"
if [[ -n "${_raw503}${_raw205}" ]]; then
    echo "⛔ 503/205: RAW OVERRIDE SET (503 $((${#_raw503}/2)) B, 205 $((${#_raw205}/2)) B) - the"
    echo "           ten-flag unlock default is NOT in play. Read the responder's decoded flag list."
else
    echo "503/205:  ten-flag unlock default {98,308,419,440,834,2667,3389,5015,5987,6018} @ value 2"
    echo "          (D1_503_SOID=$D1_503_SOID; =0 disables the 503 arm entirely)"
fi
echo "soids:    acct=$D1_ACCOUNT_SOID char=$D1_CHARACTER_SOID"
echo "push:     D1_PUSH_CHARACTER=$D1_PUSH_CHARACTER family=$D1_PUSH_FAMILY at=$D1_PUSH_AT"
echo "families: D1_ANSWER_FAMILIES=${D1_ANSWER_FAMILIES:-<none>}  D1_UNLOCKS=${D1_UNLOCKS:-<none>}  D1_FAMILY42=${D1_FAMILY42:-<none>}"
if [[ "$D1_ACTIVITY_HOST" == "1" ]]; then
    echo "⭐ AH:     ENABLED  port=$D1_AH_BIND_PORT  token=$D1_AH_HOST_TOKEN  host_text=$D1_AH_HOST_TEXT  msg17->:$D1_AH_PORT"
    # ⭐⭐ Since 2026-08-10 d1-activity-host DEFAULTS to pvp_moonbase / 407 / checksum 0x5966e491 /
    # 1 bubble / bubble 0 chosen / slice token 1 -- the only configuration ever walked to
    # activity:in_world. This block exists because the OPPOSITE used to be true and was invisible:
    # nothing here has ever exported D1_AH_ACTIVITY*, so the good values were the operator's shell
    # variables and runs 144332 / 145553 / 161501 silently shipped act_cosmo_m0_highway + index 0.
    # ⚠ A STALE EXPORT STILL WINS. Naming a world without its checksum/bubble count is the
    # cross-world misconfiguration; the AH now warns "OVERRIDE CONTRADICTS THE MEASURED WORLD".
    if [[ -n "${D1_AH_ACTIVITY:-}${D1_AH_ACTIVITY_INDEX:-}${D1_AH_ACTIVITY_CHECKSUM:-}${D1_AH_BUBBLE_COUNT:-}${D1_AH_BUBBLE_CHOSEN+set}${D1_AH_SLICE_TOKEN:-}" ]]; then
        echo "   ⛔ AH OVERRIDE SET: activity=${D1_AH_ACTIVITY:-<default>} index=${D1_AH_ACTIVITY_INDEX:-<default>} checksum=${D1_AH_ACTIVITY_CHECKSUM:-<from world>} bubbles=${D1_AH_BUBBLE_COUNT:-<from world>} chosen=${D1_AH_BUBBLE_CHOSEN-<from world>} token=${D1_AH_SLICE_TOKEN:-<default>}"
        echo "      -- read activity-host.log's own startup receipts; an override that contradicts"
        echo "         the named world is a rung-33 gate failure, and the AH names it there."
    else
        echo "   ⭐ AH world/mode: DEFAULT pvp_moonbase / 407 ELIMINATION / 0x5966e491 / 1 bubble, [0] chosen / token 1"
    fi
else
    echo "AH:       disabled (D1_ACTIVITY_HOST=1 to launch d1-activity-host; rung 30 stalls without it)"
fi
echo "objects:  profile=$(basename "${D1_PROFILE_FILE:-<none>}")  character=$(basename "${D1_CHARACTER_FILE:-<none>}")"
echo "enabled patch lines:"
python3 - "$DIAG_PATCH" <<'EOF'
import sys, xml.etree.ElementTree as E
for m in E.parse(sys.argv[1]).getroot():
    if m.get('isEnabled') == 'true':
        for pl in m:
            for ln in pl:
                print(f"  {ln.get('Address'):>8} = {ln.get('Value'):<18} | {m.get('Name')}")
EOF

if [[ "$D1_LAUNCH_MODE" == plan ]]; then
    "$REPO/d1-server/target/release/d1-platform" launch-contract --repo "$REPO" \
        --game "$GAME" --patch "$DIAG_PATCH" --emulator "$SHADPS4_BIN" --out "$RUN_OUT/launch-contract.json"
    echo "Prepared contract only: $RUN_OUT/launch-contract.json"
    exit 0
fi
if [[ "$D1_LAUNCH_MODE" == server-check ]]; then
    export D1_CHARACTER_TRANSCRIPT_DIR="$RUN_OUT/character-transcripts"
    exec bash "$REPO/capture-rig/run-signon-experiment.sh" "$GAME" "$RUN_NAME"
fi

# ── Launch, then press cross at the title screen ──────────────────────────────────────────────────
start_title_input() {
    TITLE_INPUT_PID=""
    # Respect the operator controller; only autonomous runs can use the virtual pad.
    [[ "${OPERATOR_PAD:-0}" != "1" && "${PRESS:-0}" != "0" ]] || return 0
    (
        child=""
        cleanup_input_child() {
            if [[ -n "$child" ]]; then
                kill "$child" 2>/dev/null || true
                wait "$child" 2>/dev/null || true
            fi
        }
        trap cleanup_input_child EXIT
        trap 'exit 130' INT
        trap 'exit 143' TERM
        sleep "${D1_TITLE_INPUT_DELAY_SECONDS:-60}" & child=$!
        wait "$child"; child=""
        for _ in $(seq 1 "$PRESS"); do
            # A disappearing FIFO reader must not leave this driver blocked forever.
            timeout 2 bash -c 'printf "press a\n" > "$1"' _ "$PAD_FIFO" & child=$!
            wait "$child" 2>/dev/null || true; child=""
            sleep 3 & child=$!
            wait "$child"; child=""
        done
    ) &
    TITLE_INPUT_PID=$!
}
stop_title_input() {
    if [[ -n "${TITLE_INPUT_PID:-}" ]]; then
        # The shell job table proves ownership even if an old PID has been reused.
        local running
        for running in $(jobs -pr); do
            [[ "$running" != "$TITLE_INPUT_PID" ]] || kill "$running" 2>/dev/null || true
        done
        wait "$TITLE_INPUT_PID" 2>/dev/null || true
    fi
}
trap stop_title_input EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
"$REPO/capture-rig/run-signon-experiment.sh" "$GAME" "$RUN_NAME" &
RUNNER=$!

# The title screen appears ~55s in; taps before that are simply dropped, so an early first tap is
# harmless and a late one costs a whole boot.
start_title_input

wait $RUNNER

# ── Regression check ──────────────────────────────────────────────────────────────────────────────
# Read this BEFORE reading anything else out of the run. A finding from a run that fails a gate the
# project has already passed is not a finding, it is a confound - three times on 2026-08-07 a
# forgotten environment variable produced a failure three gates earlier than the thing under test
# and read exactly like a regression in it (historicaldocs/design/character-and-orbit.md §26.45).
echo
"$REPO/d1-server/target/release/d1" gate "$RUN_OUT" || true
