#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""A kernel-level virtual gamepad, so an agent can drive the emulator on Wayland.

WHY THIS EXISTS
---------------
The previous autonomous-input attempt used `xdotool key` against Xvfb and produced **zero** SignOn
requests. Cause: Xvfb has no window manager, so there is no `_NET_ACTIVE_WINDOW` and `xdotool key`
has nothing to deliver to. Wayland has no unprivileged key-injection path at all, so moving to the
real display does not fix it either.

`/dev/uinput` sidesteps the display server entirely: it creates a **real evdev device in the kernel**.
SDL enumerates it like any other pad, so it works on Wayland, on X11, and headless - and it is
readable from inside the emulator's network namespace because device nodes are not netns-scoped.

`/dev/uinput` on this box is `crw-rw----+ root root` with a POSIX ACL granting `the operator` rw, so
this needs **no sudo**.

TWO ORDERING CONSTRAINTS, both load-bearing
-------------------------------------------
1. **Create the pad BEFORE launching the emulator.** udev hotplug arrives over
   `NETLINK_KOBJECT_UEVENT`, and netlink *is* network-namespaced - so a pad created after the
   emulator is inside `unshare -rn` will never be announced to it. Initial enumeration reads sysfs
   (not namespaced) and does find a pre-existing device.
2. **The process must stay alive.** The device disappears when the fd closes. Hence the FIFO control
   interface rather than a fire-and-forget invocation.

THE PAD LAW
-----------
Per `~/dev/hindsight/.claude/skills/sdl-controller-remap`: an agent-driven emulator must NOT claim a
physical pad belonging to the operator. This tool prints the exact
`SDL_GAMECONTROLLER_IGNORE_DEVICES` value to use, built from the **real** VID/PIDs in
`/proc/bus/input/devices` - the `0xFFFF/0xFFFF` convention is a no-op because it is a VID/PID match
list and `0xFFFF` matches no real device.

Identity: it reports as an Xbox 360 pad (`045e:028e`, bus USB, version `0x0110`) because SDL ships a
built-in mapping for that GUID. No `SDL_GAMECONTROLLERCONFIG` string needed.

USAGE
-----
    ./virtual-pad.py --fifo /tmp/d1pad            # creates the pad, blocks, reads commands
    echo 'press a'   > /tmp/d1pad                 # cross / X on a DualShock
    echo 'press start' > /tmp/d1pad
    echo 'tap a 3 1.5' > /tmp/d1pad               # 3 presses, 1.5s apart
    echo 'hold a 1.5'  > /tmp/d1pad               # press and HOLD for 1.5s
    echo 'axis ly -1'  > /tmp/d1pad               # left stick fully up, then re-centre
    echo 'axis lx 0.6 0.8' > /tmp/d1pad           # 60% right for 0.8s
    echo quit > /tmp/d1pad

⚠ `bootflow:profile_setup`'s BRIGHTNESS screen (ui stage ENUM(21)) ignores every DIGITAL input -
cross, circle, square, triangle, start, L1, R1 and all four d-pad directions, measured one press at
a time across two runs. If a screen swallows every button, reach for `axis` before concluding the
pad is broken; `press` alone cannot express a slider.

On a DualShock layout shadPS4 maps SDL south (`BTN_A`) to **cross**, which is the "press X to play"
button Destiny wants.
"""

import argparse
import fcntl
import os
import struct
import sys
import time

# ── uinput / evdev constants (linux/uinput.h, linux/input-event-codes.h) ──────────────────────────
# _IOW('U', n, int) = dir(write)<<30 | size(4)<<16 | 'U'<<8 | n
UI_SET_EVBIT = 0x40045564   # _IOW('U', 100, int)
UI_SET_KEYBIT = 0x40045565  # _IOW('U', 101, int)
UI_SET_ABSBIT = 0x40045567  # _IOW('U', 103, int)
UI_DEV_CREATE = 0x5501      # _IO('U', 1)
UI_DEV_DESTROY = 0x5502     # _IO('U', 2)

EV_SYN, EV_KEY, EV_ABS = 0x00, 0x01, 0x03
SYN_REPORT = 0

BUS_USB = 0x03

# Xbox 360 pad button layout - SDL's built-in mapping keys off the resulting GUID.
BTN = {
    "a": 0x130, "b": 0x131, "x": 0x133, "y": 0x134,
    "lb": 0x136, "rb": 0x137,
    "back": 0x13A, "start": 0x13B, "guide": 0x13C,
    "ls": 0x13D, "rs": 0x13E,
}
# Friendly aliases. Destiny is a PS4 title, so the operator thinks in DualShock names.
ALIAS = {
    "cross": "a", "circle": "b", "square": "x", "triangle": "y",
    "l1": "lb", "r1": "rb", "options": "start", "share": "back", "ps": "guide",
    "l3": "ls", "r3": "rs", "enter": "a", "accept": "a", "cancel": "b",
}

ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ = 0, 1, 2, 3, 4, 5
ABS_HAT0X, ABS_HAT0Y = 0x10, 0x11
# (code, min, max) - sticks are signed 16-bit, triggers 0..255, hats -1..1
AXES = [
    (ABS_X, -32768, 32767), (ABS_Y, -32768, 32767),
    (ABS_RX, -32768, 32767), (ABS_RY, -32768, 32767),
    (ABS_Z, 0, 255), (ABS_RZ, 0, 255),
    (ABS_HAT0X, -1, 1), (ABS_HAT0Y, -1, 1),
]
HAT = {"up": (ABS_HAT0Y, -1), "down": (ABS_HAT0Y, 1),
       "left": (ABS_HAT0X, -1), "right": (ABS_HAT0X, 1)}
# Named ANALOG axes, for screens the d-pad cannot drive. `bootflow:profile_setup`'s brightness
# screen (ui stage 0x15 = ENUM(21)) does not respond to cross, circle, square, triangle, start,
# L1/R1 or any d-pad direction - measured, twice, one press at a time with 5 s settle
# (runs 20260807-162724-subackhdr-1 and 20260807-164419-defaults-504). A screen that ignores every
# digital input is asking for a stick, and until this existed the FIFO could not produce one, so
# every unattended run stalled there forever.
STICK = {"lx": ABS_X, "ly": ABS_Y, "rx": ABS_RX, "ry": ABS_RY,
         "l2": ABS_Z, "r2": ABS_RZ}


class VirtualPad:
    def __init__(self, name="D1 Preservation Virtual Pad"):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_KEY)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_ABS)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_SYN)
        for code in BTN.values():
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, code)
        for code, _lo, _hi in AXES:
            fcntl.ioctl(self.fd, UI_SET_ABSBIT, code)

        # struct uinput_user_dev: char name[80]; input_id id; __u32 ff_effects_max;
        #                         __s32 absmax[64], absmin[64], absfuzz[64], absflat[64];
        absmax = [0] * 64
        absmin = [0] * 64
        for code, lo, hi in AXES:
            absmin[code] = lo
            absmax[code] = hi
        payload = struct.pack(
            "80sHHHHI" + "64i" * 4,
            name.encode()[:79],
            BUS_USB, 0x045E, 0x028E, 0x0110,  # the classic x360 GUID SDL already knows
            0,
            *absmax, *absmin, *([0] * 64), *([0] * 64),
        )
        os.write(self.fd, payload)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        # Centre the sticks. An uninitialised pad can read as a held stick, which in a menu shows up
        # as a phantom cursor drift and would look like the game ignoring us.
        time.sleep(0.3)
        for code, lo, hi in AXES:
            self._emit(EV_ABS, code, 0 if lo < 0 else lo)
        self._syn()

    def _emit(self, etype, code, value):
        # struct input_event { struct timeval time; __u16 type; __u16 code; __s32 value; }
        os.write(self.fd, struct.pack("llHHi", 0, 0, etype, code, value))

    def _syn(self):
        self._emit(EV_SYN, SYN_REPORT, 0)

    def press(self, name, hold=0.08):
        name = ALIAS.get(name, name)
        if name in HAT:
            code, val = HAT[name]
            self._emit(EV_ABS, code, val); self._syn()
            time.sleep(hold)
            self._emit(EV_ABS, code, 0); self._syn()
            return True
        code = BTN.get(name)
        if code is None:
            return False
        self._emit(EV_KEY, code, 1); self._syn()
        time.sleep(hold)
        self._emit(EV_KEY, code, 0); self._syn()
        return True

    def hold(self, name, seconds=1.0):
        """Press and HOLD. Some CUI screens act on a held button, not on a tap."""
        return self.press(name, hold=seconds)

    def axis(self, name, value, seconds=0.35):
        """Deflect an analog axis to `value` (-1.0..1.0, or 0..1 for triggers) then re-centre."""
        code = STICK.get(name)
        if code is None:
            return False
        lo, hi = next(((l, h) for c, l, h in AXES if c == code), (-32768, 32767))
        v = int(hi * value) if value >= 0 else int(-lo * value)
        self._emit(EV_ABS, code, max(lo, min(hi, v))); self._syn()
        time.sleep(seconds)
        self._emit(EV_ABS, code, 0 if lo < 0 else lo); self._syn()
        return True

    def close(self):
        try:
            fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        except OSError:
            pass
        os.close(self.fd)


def ignore_list():
    """The SDL_GAMECONTROLLER_IGNORE_DEVICES value covering every *physical* input device.

    Built from real VID/PIDs, per THE PAD LAW. Excludes our own virtual pad so it stays visible.
    """
    out, cur = [], None
    try:
        with open("/proc/bus/input/devices") as f:
            for line in f:
                if line.startswith("I:"):
                    parts = dict(p.split("=", 1) for p in line[2:].split() if "=" in p)
                    cur = (parts.get("Vendor"), parts.get("Product"))
                elif line.startswith("H: Handlers=") and cur:
                    if "js" in line and cur != ("045e", "028e"):
                        out.append(f"0x{cur[0]}/0x{cur[1]}")
                    cur = None
    except OSError:
        pass
    return ",".join(dict.fromkeys(out))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--fifo", default="/tmp/d1pad", help="control FIFO to create and read")
    ap.add_argument("--print-ignore", action="store_true",
                    help="print SDL_GAMECONTROLLER_IGNORE_DEVICES and exit")
    args = ap.parse_args()

    if args.print_ignore:
        print(ignore_list())
        return 0

    pad = VirtualPad()
    print(f"pad created; SDL_GAMECONTROLLER_IGNORE_DEVICES={ignore_list()}", flush=True)

    if os.path.exists(args.fifo):
        os.unlink(args.fifo)
    os.mkfifo(args.fifo, 0o600)
    print(f"control fifo: {args.fifo}  (commands: press <btn> | tap <btn> <n> <gap> | quit)",
          flush=True)

    try:
        while True:
            # Reopening per read is deliberate: a FIFO read returns EOF each time the last writer
            # closes, so a single long-lived handle would spin at EOF forever.
            with open(args.fifo) as f:
                for line in f:
                    parts = line.split()
                    if not parts:
                        continue
                    cmd = parts[0].lower()
                    if cmd in ("quit", "exit"):
                        return 0
                    if cmd == "press" and len(parts) >= 2:
                        ok = pad.press(parts[1].lower())
                        print(f"press {parts[1]}: {'ok' if ok else 'UNKNOWN BUTTON'}", flush=True)
                    elif cmd == "tap" and len(parts) >= 2:
                        n = int(parts[2]) if len(parts) > 2 else 1
                        gap = float(parts[3]) if len(parts) > 3 else 1.0
                        for i in range(n):
                            pad.press(parts[1].lower())
                            print(f"tap {parts[1]} {i + 1}/{n}", flush=True)
                            if i + 1 < n:
                                time.sleep(gap)
                    elif cmd == "hold" and len(parts) >= 2:
                        secs = float(parts[2]) if len(parts) > 2 else 1.0
                        ok = pad.hold(parts[1].lower(), secs)
                        print(f"hold {parts[1]} {secs}s: {'ok' if ok else 'UNKNOWN BUTTON'}",
                              flush=True)
                    elif cmd == "axis" and len(parts) >= 3:
                        secs = float(parts[3]) if len(parts) > 3 else 0.35
                        ok = pad.axis(parts[1].lower(), float(parts[2]), secs)
                        print(f"axis {parts[1]} {parts[2]} {secs}s: "
                              f"{'ok' if ok else f'UNKNOWN AXIS (have {sorted(STICK)})'}", flush=True)
                    elif cmd == "sleep" and len(parts) >= 2:
                        time.sleep(float(parts[1]))
                    else:
                        print(f"unknown command: {line.strip()}", flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        pad.close()
        if os.path.exists(args.fifo):
            os.unlink(args.fifo)
    return 0


if __name__ == "__main__":
    sys.exit(main())
