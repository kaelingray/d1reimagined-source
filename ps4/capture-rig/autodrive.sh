#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Drive the boot flow all the way to `character:signin` with NO OPERATOR AT THE MACHINE.
#
# WHY THIS EXISTS
# ---------------
# `autopress.sh` (archived 2026-08-11 to historicaldocs/python-capture/; this script is its strict
# superset) pressed cross on `bootflow:start` and deliberately refused `bootflow:profile_setup`,
# because that state is a THREE-SCREEN CURSOR MENU (screen boundaries -> preferred brightness ->
# language/subtitles) and blind X presses cannot navigate it. That refusal is correct, and it is also
# the single reason every run so far has needed a human in the chair. Which in turn is why the
# project has only ever had a handful of full-ladder runs: the experiment loop was gated on a person.
#
# THE TWO FACTS THAT MAKE THIS POSSIBLE
# -------------------------------------
# 1. ⚠ The brightness screen (ui substage `ENUM(21)`) IGNORES EVERY DIGITAL INPUT -- cross, circle,
#    square, triangle, start, L1, R1 and all four d-pad directions, measured one press at a time
#    across two runs (`virtual-pad.py` docstring). A slider cannot be expressed as a button. So the
#    driver must reach for `axis` there, and `press` alone will look exactly like a broken pad.
# 2. The client narrates its own menu position: `world_controller:ui_stage: Substage goal changed
#    from 'ENUM(a)' to 'ENUM(b)'`. So the driver never has to guess from wall-clock time -- it reads
#    where it actually is. Sleep-based driving is what made earlier attempts flaky.
#
# THE DESIGN RULE: ADAPTIVE, NOT SCRIPTED
# ---------------------------------------
# We do NOT hardcode "substage 20 wants cross, 21 wants the stick". That mapping was not stable
# across runs (`162724` oscillated 21<->22 for its whole life), and a fixed script that guesses wrong
# stalls silently -- the worst failure mode this project has. Instead: while the substage is
# UNCHANGED, cycle through a repertoire of inputs (stick nudge, then confirm) until it moves. The
# moment it moves, reset. That converges without knowing which screen is which, and it self-heals
# when a press lands before a screen is interactive.
#
# ⚠ SAFETY: this presses cross, which is CONFIRM everywhere -- including character creation. The
# driver therefore STOPS the moment `character:signin` is reached and never presses again. Selecting
# or creating a character is a decision with consequences for the experiment, so it stays manual
# unless `--through-select` is passed explicitly.
#
#   ./autodrive.sh <run-dir> [--through-select]
#
# Start it AFTER the run is launched; it exits when the emulator does.
set -euo pipefail

RUN="${1:?usage: autodrive.sh <run-dir> [--through-select]}"
shift || true
THROUGH_SELECT=0
[[ "${1:-}" == "--through-select" ]] && THROUGH_SELECT=1

PAD="${PAD_FIFO:-/tmp/d1pad}"
LOG="$RUN/emulator.log"

[[ -p "$PAD" ]] || { echo "autodrive: no pad FIFO at $PAD - start capture-rig/virtual-pad.py FIRST" >&2; exit 1; }

pad() { echo "$*" > "$PAD" 2>/dev/null || true; }

# Where is the client right now? The last state and the last substage it announced.
cur_state()    { grep -ao "bootflow:[a-z_]*\|character:[a-z_]*" "$LOG" 2>/dev/null | tail -1 || true; }
cur_substage() { grep -ao "Substage goal changed from 'ENUM([-0-9]*)' to 'ENUM([-0-9]*)'" "$LOG" 2>/dev/null \
                   | tail -1 | grep -oE "to 'ENUM\([-0-9]*\)'" | grep -oE '[-0-9]+' || true; }

# The repertoire, in the order we try it. `axis` first because the screen that actually blocks is the
# one that only answers to a stick, and a wasted stick nudge on a button screen is harmless while a
# wasted press on the slider screen looks like a hang.
REPERTOIRE=(
  "axis ly -0.9 0.4"     # stick up    - moves a slider / list cursor
  "press a"              # confirm
  "axis lx 0.9 0.4"      # stick right - the brightness slider is horizontal on some screens
  "press a"
  "axis ly 0.9 0.4"      # stick down
  "press a"
)

echo "autodrive: watching $RUN  (stop at character:signin: $([[ $THROUGH_SELECT == 1 ]] && echo no || echo YES))"
last_key=""
idx=0
stuck=0

while pgrep -f "[s]hadps4 -p " >/dev/null; do
    state="$(cur_state)"
    sub="$(cur_substage)"
    key="$state/$sub"

    # Reached the character screen: our job is done, and pressing cross here is a real decision.
    if [[ "$state" == character:* && $THROUGH_SELECT == 0 ]]; then
        echo "autodrive: reached $state - stopping (cross here selects/creates a character)"
        exit 0
    fi

    if [[ "$key" != "$last_key" ]]; then
        # It moved. Reset the repertoire so the next screen starts from the top.
        [[ -n "$last_key" ]] && echo "autodrive: advanced $last_key -> $key"
        last_key="$key"; idx=0; stuck=0
        sleep 2   # let the new screen become interactive before poking it
        continue
    fi

    case "$state" in
        bootflow:start)
            # The title screen only ever wants cross.
            pad "press a"
            ;;
        bootflow:profile_setup)
            # The three-screen cursor menu. Cycle the repertoire until the substage moves.
            act="${REPERTOIRE[$((idx % ${#REPERTOIRE[@]}))]}"
            echo "autodrive: $key unchanged (${stuck}) - $act"
            pad "$act"
            idx=$((idx + 1))
            ;;
        *)
            # Every other state advances on its own; do not poke it.
            ;;
    esac

    stuck=$((stuck + 1))
    sleep 3
done

echo "autodrive: emulator exited"
