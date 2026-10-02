"""Mission traversal for the owned 01.24 test session: read the local player's position and walk
it to world coordinates with the keyboard pad (W/A/S/D = left stick, J/L = camera turn).

Reverse-engineering tool. Reads guest memory of the run's own emulator (rpcs3_owned_memory) and
posts keys to that emulator's game window only. The player entity record comes from the PPU
call hook on 0x138e24 (r4; the world controller's follow-player step 0x352968 calls it every
frame), and the position is the float triple at record +0x40 (x, y, z; z up) - JOIN63.

    python scripts/d1_traverse.py --manifest <run>-processes.json where
    python scripts/d1_traverse.py --manifest <run>-processes.json goto X Y [--radius 2]
    python scripts/d1_traverse.py --manifest <run>-processes.json find 0x992bb471 0x2c 87
"""
import argparse
import ctypes
import json
import math
import re
import struct
import time
from ctypes import wintypes as wt
from pathlib import Path

from rpcs3_owned_memory import OwnedMemory, emulator_identity

LOCAL = Path(__file__).resolve().parents[2] / "lukespc-import" / "d1xenia" / "local"
RPCS3_DIR = LOCAL / "RPCS3_FINAL124_SMOKE001"
WATCH = RPCS3_DIR / "d1" / "d1ppu.txt"
LOG = RPCS3_DIR / "log" / "RPCS3.log"
ENTITY_HOOK = "138e24"
POSITION = 0x40


def read_shared(path):
    """RPCS3 keeps its log open with delete sharing; open it the same way."""
    import msvcrt
    import os
    k = ctypes.WinDLL("kernel32", use_last_error=True)
    k.CreateFileW.restype = wt.HANDLE
    h = k.CreateFileW(str(path), 0x80000000, 7, None, 3, 0x80, None)
    if h in (None, wt.HANDLE(-1).value):
        raise OSError(f"cannot open {path}")
    with os.fdopen(msvcrt.open_osfhandle(h, os.O_RDONLY | os.O_BINARY), "rb") as f:
        return f.read()


def find_player_entity(timeout=30.0):
    """Arm the entity hook and return the record address from the next hit."""
    WATCH.write_text(f"# d1_traverse: player entity\n{ENTITY_HOOK} 3\n", encoding="ascii")
    deadline = time.time() + timeout
    while time.time() < deadline:
        hits = re.findall(rb"pc=0x138e24 lr=0x352a4c .*? r4=0x([0-9a-f]+)", read_shared(LOG))
        if hits:
            return int(hits[-1], 16)
        time.sleep(1)
    raise TimeoutError("no 0x138e24 hit (is the session in a world?)")


class Pad:
    """Posts key presses to the owned emulator's game window (no focus needed)."""
    VK = {"W": 0x57, "A": 0x41, "S": 0x53, "D": 0x44, "J": 0x4A, "L": 0x4C, "I": 0x49, "K": 0x4B,
          "X": 0x58, "C": 0x43, "G": 0x47, "SPACE": 0x20}

    def __init__(self, pid):
        self.u = ctypes.WinDLL("user32", use_last_error=True)
        self.u.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
        self.u.MapVirtualKeyW.argtypes = [wt.UINT, wt.UINT]
        windows = []
        proc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

        def visit(h, _):
            owner = wt.DWORD()
            self.u.GetWindowThreadProcessId(h, ctypes.byref(owner))
            if owner.value == pid and self.u.IsWindowVisible(h):
                title = ctypes.create_unicode_buffer(256)
                self.u.GetWindowTextW(h, title, 256)
                if re.search("FPS|Destiny|BLES", title.value):
                    windows.append(h)
            return True
        self.u.EnumWindows(proc(visit), 0)
        if not windows:
            raise RuntimeError("Destiny game window not found for the run's emulator")
        self.window = windows[0]

    def _post(self, vk, down):
        scan = self.u.MapVirtualKeyW(vk, 0)
        lparam = 1 | (scan << 16)
        if not down:
            lparam |= (1 << 30) | (1 << 31)
        self.u.PostMessageW(self.window, 0x100 if down else 0x101, vk, lparam)

    def hold(self, keys, ms):
        vks = [self.VK[k] for k in keys]
        for vk in vks:
            self._post(vk, True)
        time.sleep(ms / 1000)
        for vk in vks:
            self._post(vk, False)


class Player:
    def __init__(self, mem, entity):
        self.mem, self.entity = mem, entity

    def position(self):
        return struct.unpack(">fff", self.mem.read(self.entity + POSITION, 12))


def heading(a, b):
    return math.atan2(b[1] - a[1], b[0] - a[0])


def angle_diff(a, b):
    return (b - a + math.pi) % (2 * math.pi) - math.pi


def facing_now(player, pad, ms=350):
    """Heading (radians) of a short forward step, or None if blocked."""
    a = player.position()
    pad.hold(["W"], ms)
    time.sleep(0.15)
    b = player.position()
    return heading(a, b) if math.dist(a[:2], b[:2]) > 0.15 else None


class Steering:
    """Camera-turn calibration: signed radians per ms of the L key (J turns the other way)."""

    def __init__(self, player, pad, log=print):
        self.player, self.pad, self.log, self.rate = player, pad, log, None

    def calibrate(self):
        for _ in range(4):
            h0 = facing_now(self.player, self.pad)
            self.pad.hold(["L"], 250)
            h1 = facing_now(self.player, self.pad)
            if h0 is not None and h1 is not None and abs(angle_diff(h0, h1)) > 0.05:
                self.rate = angle_diff(h0, h1) / 250
                self.log(f"camera: L turns {math.degrees(self.rate) * 1000:+.0f} deg/s")
                return
            self.pad.hold(["X", "W"], 500)  # blocked: hop forward and retry
        self.rate = -math.radians(120) / 1000
        self.log("camera: calibration failed, assuming L turns -120 deg/s")

    def turn(self, error):
        if self.rate is None:
            self.calibrate()
        key = "L" if error * self.rate > 0 else "J"
        ms = min(900, abs(error / self.rate))
        if ms >= 20:
            self.pad.hold([key], ms)


def goto(player, pad, target, radius=2.0, max_steps=200, step_ms=1200, sprint=True, log=print,
         steering=None):
    """Closed loop: step forward, measure heading from the motion, turn toward the target."""
    steering = steering or Steering(player, pad, log)
    unstick = [("hop", ["X", "W"], 0), ("left", None, 60), ("right", None, -120), ("back", ["S"], 0)]
    blocked = 0
    for step in range(max_steps):
        here = player.position()
        distance = math.dist(here[:2], target[:2])
        if distance <= radius:
            log(f"reached {target} at {[round(v, 1) for v in here]}")
            return True
        pad.hold(["W", "G"] if sprint else ["W"], min(step_ms, max(300, distance * 400)))
        time.sleep(0.15)
        after = player.position()
        moved = math.dist(here[:2], after[:2])
        if moved > 25:
            log(f"step {step}: jumped {moved:.0f} units to {[round(v, 1) for v in after]} (respawn?)")
            continue
        if moved < 0.3:
            name, keys, angle = unstick[blocked % len(unstick)]
            blocked += 1
            log(f"step {step}: blocked at {[round(v, 1) for v in after]}; {name}")
            if keys:
                pad.hold(keys, 700)
            else:
                if steering.rate is None:
                    steering.calibrate()
                steering.turn(math.radians(angle))
                pad.hold(["W", "G"], 900)
            continue
        blocked = 0
        facing = heading(here, after)
        error = angle_diff(facing, heading(after, target))
        log(f"step {step}: at {[round(v, 1) for v in after]} dist {distance:.1f} "
            f"facing {math.degrees(facing):.0f} error {math.degrees(error):+.0f}")
        if abs(error) > math.radians(10):
            steering.turn(error)
    return False


def route(player, pad, waypoints, radius=3.0, log=print):
    steering = Steering(player, pad, log)
    for point in waypoints:
        log(f"-> {point}")
        if not goto(player, pad, point, radius, log=log, steering=steering):
            log(f"gave up before {point}")
            return False
    return True


def find_object(mem, group, kind, index, low=0x30000000, high=0x80000000, chunk=16 << 20):
    """Guest addresses of the 8-byte id {group, kind << 24 | index} (content records carry their
    own id; e.g. the objective definition at +0x24)."""
    needle = struct.pack(">II", group, (kind << 24) | index)
    found = []
    for start in range(low, high, chunk):
        try:
            data = mem.read(start, chunk + 8)
        except (OSError, ValueError):
            continue
        at = data.find(needle)
        while at != -1:
            found.append(start + at)
            at = data.find(needle, at + 4)
    return sorted(set(found))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--manifest", required=True, type=Path)
    ap.add_argument("--entity", type=lambda v: int(v, 0), help="player entity record (skip the hook)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("where")
    g = sub.add_parser("goto")
    g.add_argument("x", type=float)
    g.add_argument("y", type=float)
    g.add_argument("--radius", type=float, default=2.0)
    g.add_argument("--step-ms", type=int, default=1200)
    g.add_argument("--no-sprint", action="store_true")
    r = sub.add_parser("route")
    r.add_argument("points", nargs="+", help="X,Y waypoints")
    r.add_argument("--radius", type=float, default=3.0)
    f = sub.add_parser("find")
    f.add_argument("group", type=lambda v: int(v, 0))
    f.add_argument("kind", type=lambda v: int(v, 0))
    f.add_argument("index", type=lambda v: int(v, 0))
    args = ap.parse_args()
    with OwnedMemory(args.manifest) as mem:
        if args.cmd == "find":
            for address in find_object(mem, args.group, args.kind, args.index):
                around = mem.read(address - 0x40, 0xc0)
                floats = struct.unpack(">48f", around)
                print(f"{address:#010x}", " ".join(f"{v:.2f}" for v in floats
                                                    if math.isfinite(v) and 1 < abs(v) < 50000))
            return
        entity = args.entity or find_player_entity()
        player = Player(mem, entity)
        print(f"entity {entity:#x} position {[round(v, 2) for v in player.position()]}")
        if args.cmd == "goto":
            pid = emulator_identity(json.loads(args.manifest.read_text(encoding="utf-8-sig")))["id"]
            goto(player, Pad(pid), (args.x, args.y), args.radius, step_ms=args.step_ms,
                 sprint=not args.no_sprint)
        if args.cmd == "route":
            pid = emulator_identity(json.loads(args.manifest.read_text(encoding="utf-8-sig")))["id"]
            route(player, Pad(pid), [tuple(float(v) for v in p.split(",")) for p in args.points], args.radius)


if __name__ == "__main__":
    main()
