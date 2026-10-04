"""Real-time walker for the owned 01.24 test session (reverse-engineering tool).

Builds on d1_traverse (player position at entity +0x40, keyboard pad): reads the player's
facing from memory, steers while W stays held, recovers from obstacles, avoids drops, and keeps a
per-slice-set grid of blocked/hazard cells (JSON) that A* plans around, so every attempt improves
the next.

Facing (JOIN71): unit vectors (cos, sin, 0) in the game heap turn with the camera keys
(L clockwise, J counter-clockwise, ~115 deg/s); some copies point along the walking direction,
some backwards, and the forward ones can go stale. `find_facing` locates them each run: a probe
step gives the heading, a scan finds matching unit vectors (both signs), two separate turns
confirm the ones that follow the camera; the walker searches again if its copy stops moving.

    python scripts/d1_walker.py --manifest <run>-processes.json go X Y [--map maps/112.json]
"""
import argparse
import heapq
import json
import math
import struct
import time
from pathlib import Path

import numpy as np

from d1_traverse import Pad, Player, angle_diff, find_player_entity, heading
from rpcs3_owned_memory import OwnedMemory, emulator_identity
from rpcs3_position_scan import guest_regions

CELL = 2.0  # grid cell size (world units)

# Pointer paths to the forward facing vector (JOIN73; static roots in the EBOOT data segment):
# *0x1adbce8 + 0x92c, *0x1b27fe8 + 0x18c, *(*0x1b28210 + 4) + 0x3c - all three led to the same
# vector and followed the camera.
FACING_PATHS = ((0x1ADBCE8, (0x92C,)), (0x1B27FE8, (0x18C,)), (0x1B28210, (0x4, 0x3C)))


def facing_from_pointers(mem):
    """Address of the forward facing vector through the static pointer paths, or None."""
    for root, offsets in FACING_PATHS:
        try:
            address = struct.unpack(">I", mem.read(root, 4))[0]
            for offset in offsets[:-1]:
                address = struct.unpack(">I", mem.read(address + offset, 4))[0]
            address += offsets[-1]
            c, s, z = struct.unpack(">3f", mem.read(address, 12))
        except (OSError, ValueError, struct.error):
            continue
        if abs(z) < 0.01 and abs(c * c + s * s - 1) < 0.02:
            return address
    return None


def find_facing(mem, player, pad, low=0x30000000, high=0x38000000, log=print):
    """(guest address, sign) of a unit vector (cos, sin, 0) that follows the camera; sign -1
    means it points backwards (JOIN71: the backward copy at 0x31ea6810 stayed valid for the
    whole session, while forward copies went stale)."""
    def probe():
        """Facing from a short move: forward, backward, or a strafe (D = right of facing)."""
        for key, offset in (("W", 0.0), ("S", math.pi), ("D", math.pi / 2), ("A", -math.pi / 2)):
            a = player.position()
            pad.hold([key], 350)
            time.sleep(0.25)
            b = player.position()
            if math.dist(a[:2], b[:2]) > 0.15:
                return heading(a, b) + offset
        return None

    h = probe()
    if h is None:
        pad.hold(["X", "W"], 500)  # wedged: hop and retry
        h = probe()
    if h is None:
        raise RuntimeError("cannot measure the facing (blocked in every direction)")
    regions = [(s, n) for s, n in guest_regions(mem) if s < high and s + n > low]
    candidates = []
    for sign in (1, -1):
        want = (sign * math.cos(h), sign * math.sin(h))
        for start, size in regions:
            for off in range(0, size, 16 << 20):
                n = min(16 << 20, size - off)
                try:
                    data = np.frombuffer(mem.read(start + off, n), dtype=">f4")
                except OSError:
                    continue
                with np.errstate(all="ignore"):
                    hit = ((np.abs(data[:-2] - want[0]) < 0.12) & (np.abs(data[1:-1] - want[1]) < 0.12)
                           & (data[2:] == 0))
                for i in np.flatnonzero(hit):
                    if abs(float(data[i]) ** 2 + float(data[i + 1]) ** 2 - 1) < 0.02:
                        candidates.append((start + off + 4 * int(i), sign))
    log(f"facing: probe heading {math.degrees(h):.0f}, {len(candidates)} candidate vectors")

    def read(a, sign):
        c, s = struct.unpack(">2f", mem.read(a, 8))
        return math.atan2(sign * s, sign * c) if c or s else None

    for key in ("L", "J"):  # two separate turns: a candidate must follow both
        before = {cand: read(*cand) for cand in candidates}
        pad.hold([key], 250)
        time.sleep(0.3)
        now = probe()
        if now is None:
            raise RuntimeError("cannot confirm the facing")
        kept = []
        for cand in candidates:
            value = read(*cand)
            if (value is not None and before[cand] is not None
                    and abs(angle_diff(before[cand], value)) > math.radians(5)
                    and abs(angle_diff(value, now)) < math.radians(15)):
                kept.append(cand)
        candidates = kept
    if not candidates:
        raise RuntimeError("no candidate followed the camera")
    candidates.sort(key=lambda c: c[1])  # backward copies first (they stayed valid)
    log(f"facing vector at {candidates[0][0]:#x} sign {candidates[0][1]:+d} ({len(candidates)} copies)")
    return candidates[0]


class Grid:
    """Blocked and hazard cells for one slice set, persisted as JSON."""

    def __init__(self, path=None):
        self.path = Path(path) if path else None
        self.blocked, self.hazard = {}, set()
        if self.path and self.path.exists():
            data = json.loads(self.path.read_text(encoding="utf-8"))
            self.blocked = {tuple(k): v for k, v in data.get("blocked", [])}
            self.hazard = {tuple(c) for c in data.get("hazard", [])}

    @staticmethod
    def cell(xy):
        return (int(math.floor(xy[0] / CELL)), int(math.floor(xy[1] / CELL)))

    def mark_blocked(self, xy):
        c = self.cell(xy)
        self.blocked[c] = self.blocked.get(c, 0) + 1

    def mark_hazard(self, xy):
        self.hazard.add(self.cell(xy))

    def save(self):
        if self.path:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.path.write_text(json.dumps({"cell": CELL, "blocked": [[list(k), v] for k, v in self.blocked.items()],
                                             "hazard": [list(c) for c in self.hazard]}), encoding="utf-8")

    def cost(self, c):
        if c in self.hazard or self.blocked.get(c, 0) >= 2:
            return None
        return 1.0 + 2.0 * self.blocked.get(c, 0)

    def plan(self, start_xy, goal_xy, limit=40000):
        """A* over 8-connected cells; unknown cells are free. Returns world waypoints."""
        start, goal = self.cell(start_xy), self.cell(goal_xy)
        frontier = [(0.0, start)]
        came, cost = {start: None}, {start: 0.0}
        while frontier and len(came) < limit:
            _, cur = heapq.heappop(frontier)
            if cur == goal:
                break
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    if dx == dy == 0:
                        continue
                    nxt = (cur[0] + dx, cur[1] + dy)
                    step = self.cost(nxt)
                    if step is None and nxt != goal:
                        continue
                    new = cost[cur] + (step or 1.0) * math.hypot(dx, dy)
                    if nxt not in cost or new < cost[nxt]:
                        cost[nxt], came[nxt] = new, cur
                        heapq.heappush(frontier, (new + math.dist(nxt, goal), nxt))
        if goal not in came:
            return [goal_xy]
        path, c = [], goal
        while c is not None:
            path.append(((c[0] + 0.5) * CELL, (c[1] + 0.5) * CELL))
            c = came[c]
        path.reverse()
        # keep a waypoint every ~5 cells and wherever the path turns
        keep = [p for i, p in enumerate(path) if i % 5 == 0]
        return keep[1:] + [goal_xy]


class Walker:
    def __init__(self, mem, player, pad, facing, grid=None, log=print):
        self.mem, self.player, self.pad = mem, player, pad
        self.facing, self.sign = facing
        self.grid, self.log = grid or Grid(), log
        self.rate = math.radians(115) / 1000  # camera turn, radians per ms (adapted below)

    def yaw(self):
        live = facing_from_pointers(self.mem)
        if live is not None:
            self.facing, self.sign = live, 1
        c, s = struct.unpack(">2f", self.mem.read(self.facing, 8))
        if not (c or s):
            self.refind()
            return self.yaw()
        return math.atan2(self.sign * s, self.sign * c)

    def refind(self):
        live = facing_from_pointers(self.mem)
        if live is not None and live != self.facing:
            self.facing, self.sign = live, 1
            return
        self.log("facing vector went stale; searching again")
        try:
            self.facing, self.sign = find_facing(self.mem, self.player, self.pad, log=self.log)
        except RuntimeError as exc:
            self.log(f"  facing search failed ({exc}); keeping the current vector")

    def face(self, target_heading, tolerance=math.radians(6), tries=12, arrived=None):
        still = 0
        for _ in range(tries):
            # Released movement keys can still leave the player coasting toward the goal.
            # Observe that arrival before spending the bounded entry window turning back.
            if arrived is not None and arrived():
                return True
            error = angle_diff(self.yaw(), target_heading)
            if abs(error) <= tolerance:
                return True
            ms = max(25, min(300, abs(error) / self.rate * 0.8))
            before = self.yaw()
            self.pad.hold(["J" if error > 0 else "L"], ms)  # J turns counter-clockwise
            time.sleep(0.06)
            turned = abs(angle_diff(before, self.yaw()))
            if turned > 0.02:
                self.rate = 0.7 * self.rate + 0.3 * (turned / ms)
                still = 0
            elif ms >= 150:  # short taps barely move the camera (stick acceleration)
                still += 1
                if still >= 2:  # the vector stopped following the camera
                    self.refind()
                    still = 0
        return False

    def press(self, keys, down):
        for key in keys:
            self.pad._post(self.pad.VK[key], down)

    def stride(self, target, seconds=4.0, sprint=True, arrived=None):
        """Hold W toward the target, steering on the fly. Returns 'arrived', 'stuck', 'drop' or
        'time', or 'goal' when the actual goal is reached between waypoint checks."""
        if arrived is not None and arrived():
            return "goal"
        keys = ["W", "G"] if sprint else ["W"]
        self.press(keys, True)
        try:
            start = time.time()
            last_pos, last_check = self.player.position(), time.time()
            while time.time() - start < seconds:
                time.sleep(0.1)
                here = self.player.position()
                if arrived is not None and arrived(here):
                    return "goal"
                if math.dist(here[:2], target[:2]) < 2.0:
                    return "arrived"
                if here[2] < last_pos[2] - 3.0:
                    return "drop"
                error = angle_diff(self.yaw(), heading(here, target))
                if abs(error) > math.radians(8):
                    self.pad.hold(["J" if error > 0 else "L"], max(20, min(120, abs(error) / self.rate * 0.6)))
                if time.time() - last_check >= 1.0:
                    if math.dist(here[:2], last_pos[:2]) < 0.6:
                        return "stuck"
                    last_pos, last_check = here, time.time()
            return "time"
        finally:
            self.press(keys, False)

    def recover(self, target, log=None):
        """Back off, then try side headings with jumps until progress toward the target."""
        log = log or self.log
        self.pad.hold(["S"], 350)
        here = self.player.position()
        bearing = heading(here, target)
        for offset in (50, -50, 90, -90, 135, -135):
            self.face(bearing + math.radians(offset), tolerance=math.radians(12), tries=6)
            a = self.player.position()
            self.pad.hold(["W", "G", "X"], 900)
            self.pad.hold(["W", "G"], 600)
            time.sleep(0.15)
            b = self.player.position()
            if b[2] < a[2] - 4:
                self.grid.mark_hazard(b)
                self.pad.hold(["S"], 700)
                continue
            if math.dist(a[:2], b[:2]) > 1.5:
                log(f"  recovered via {offset:+d} deg -> {[round(v, 1) for v in b]}")
                return True
            self.grid.mark_blocked((a[0] + 1.5 * math.cos(self.yaw()), a[1] + 1.5 * math.sin(self.yaw())))
        return False

    def go(self, goal, radius=3.0, budget=600.0):
        """Walk to goal (x, y) re-planning on the grid; returns True when within radius."""
        start = time.time()
        arrival = None

        def arrived(position=None):
            nonlocal arrival
            if arrival is None:
                position = self.player.position() if position is None else position
                if math.dist(position[:2], goal[:2]) <= radius:
                    arrival = position
            return arrival is not None

        def reached():
            self.log(f"reached {goal} at {[round(v, 1) for v in arrival]} in {time.time() - start:.0f} s")
            self.grid.save()
            return True

        while time.time() - start < budget:
            here = self.player.position()
            if arrived(here):
                return reached()
            waypoints = self.grid.plan(here[:2], goal)
            target = waypoints[0]
            self.face(heading(here, target), arrived=arrived)
            if arrived():
                return reached()
            result = self.stride(target, arrived=arrived)
            if arrival is not None:
                return reached()
            now = self.player.position()
            self.log(f"{result:7s} at {[round(v, 1) for v in now]} -> wp {[round(v, 1) for v in target]} "
                     f"(goal {math.dist(now[:2], goal[:2]):.0f})")
            if result == "drop":
                self.grid.mark_hazard(now)
                self.pad.hold(["S"], 800)
            elif result == "stuck":
                yaw = self.yaw()
                for spread in (-0.6, 0.0, 0.6):  # left-front, front, right-front: impassable at once
                    ahead = (now[0] + 1.8 * math.cos(yaw + spread), now[1] + 1.8 * math.sin(yaw + spread))
                    self.grid.mark_blocked(ahead)
                    self.grid.mark_blocked(ahead)
                self.recover(target)
            if math.dist(now[:2], here[:2]) > 25:
                self.log("  position jumped (respawn?)")
            self.grid.save()
        self.log("budget exhausted")
        return False


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--manifest", required=True, type=Path)
    ap.add_argument("--entity", type=lambda v: int(v, 0))
    ap.add_argument("--facing", help="ADDRESS[:SIGN] of the facing vector (skip the search; sign -1 = backward copy)")
    ap.add_argument("--map", type=Path, help="grid JSON (blocked/hazard cells), read and updated")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("facing")
    g = sub.add_parser("go")
    g.add_argument("points", nargs="+", help="X,Y goals in order")
    g.add_argument("--radius", type=float, default=3.0)
    g.add_argument("--budget", type=float, default=600.0, help="seconds per goal")
    args = ap.parse_args()
    pid = emulator_identity(json.loads(args.manifest.read_text(encoding="utf-8-sig")))["id"]
    with OwnedMemory(args.manifest) as mem:
        player = Player(mem, args.entity or find_player_entity())
        pad = Pad(pid)
        if args.facing:
            address, _, sign = args.facing.partition(":")
            facing = (int(address, 0), int(sign or "1"))
        elif facing_from_pointers(mem) is not None:
            facing = (facing_from_pointers(mem), 1)
            print(f"facing vector via static pointers at {facing[0]:#x}")
        else:
            facing = find_facing(mem, player, pad)
        if args.cmd == "facing":
            return
        walker = Walker(mem, player, pad, facing, Grid(args.map))
        for point in args.points:
            goal = tuple(float(v) for v in point.split(","))
            if not walker.go(goal, args.radius, args.budget):
                break


if __name__ == "__main__":
    main()
