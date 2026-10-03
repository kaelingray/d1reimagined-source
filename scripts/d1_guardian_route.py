"""Walk the recorded 01.24 route to the Wall on one exactly owned test emulator.

python scripts/d1_guardian_route.py --manifest <RUN>-processes.json

Waits for the client's actual slice 112 and an idle in-world engine, observes a fresh player
hook, then follows every third route point and its endpoint. Stops on loading, cleanup,
rejoin, a new slice request or a position jump. Only pad input is sent; guest memory is read
only. Diagnostic watch lines are preserved and private audit/state/result/grid files are
written beside the manifest by default. This is a route test, not proof of mission completion.
"""

import argparse
import ctypes
import datetime
import json
import math
import os
import re
import struct
import time
from contextlib import ExitStack
from pathlib import Path

from d1_traverse import Pad, Player
from d1_walker import Grid, Walker, facing_from_pointers
from rpcs3_owned_memory import OwnedMemory, emulator_identity

ROOT = Path(__file__).resolve().parents[1]
HOOK = re.compile(rb"\bpc=0x138e24\s+lr=0x352a4c\b.*?\br4=0x([0-9a-f]+)\b", re.I)
HOOK_LINE = re.compile(r"^\s*(?:0x)?138e24(?:\s|$)", re.I)
SLICE = re.compile(rb"\b(current|target) (-?\d+):0x([0-9a-f]+)", re.I)


class RouteStopped(RuntimeError):
    pass


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def route_points(path, every=3):
    points = json.loads(Path(path).read_text(encoding="utf-8-sig"))
    if not points or every < 1 or any(not isinstance(p, list) or len(p) != 3 or
                                    any(not isinstance(v, (int, float)) or not math.isfinite(v)
                                        for v in p) for p in points):
        raise ValueError("route must contain finite x,y,z triples and a positive point interval")
    selected = [(i, points[i]) for i in range(0, len(points), every)]
    if selected[-1][0] != len(points) - 1:
        selected.append((len(points) - 1, points[-1]))
    return selected


class SharedTail:
    """Bounded incremental reads, including RPCS3's exclusively held Windows log."""

    def __init__(self, path, fresh=False):
        self.path = Path(path)
        self.offset = self.path.stat().st_size if fresh and self.path.exists() else 0
        self.partial = b""

    def lines(self):
        if not self.path.exists():
            return []
        if os.name == "nt":
            import msvcrt
            from ctypes import wintypes as wt
            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel.CreateFileW.argtypes = [wt.LPCWSTR, wt.DWORD, wt.DWORD, ctypes.c_void_p,
                                          wt.DWORD, wt.DWORD, wt.HANDLE]
            kernel.CreateFileW.restype = wt.HANDLE
            handle = kernel.CreateFileW(str(self.path), 0x80000000, 7, None, 3, 0x80, None)
            if handle in (None, wt.HANDLE(-1).value):
                raise ctypes.WinError(ctypes.get_last_error())
            stream = os.fdopen(msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY), "rb")
        else:
            stream = self.path.open("rb")
        with stream:
            size = stream.seek(0, 2)
            if size < self.offset:
                raise RouteStopped(f"log rotated or truncated: {self.path.name}")
            stream.seek(self.offset)
            data = stream.read(min(1 << 20, size - self.offset))
            self.offset += len(data)
        combined = self.partial + data
        parts = combined.split(b"\n")
        self.partial = parts.pop()
        return parts


class ClientWorld:
    def __init__(self):
        self.current = self.target = None
        self.joins = 0

    def update(self, lines):
        for line in lines:
            if b"host join request" in line:
                self.joins += 1
            if b"client slice set:" not in line:
                continue
            for name, index, name_hash in SLICE.findall(line):
                value = (int(index), int(name_hash, 16))
                if name.lower() == b"current":
                    self.current = value
                else:
                    self.target = None if value == (0, 0) or value[0] < 0 else value

    def ready(self):
        return self.current is not None and self.current[0] == 112 and self.current[1] != 0 and (
            self.target is None or self.target[0] == 112)


def run_state(read):
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]
    boot, engine = u32(0x01ABC89C), u32(0x01ABC8AC)
    if not boot or not engine:
        return {"current": None, "engine": None, "stable": False}
    fields = read(boot + 0x118, 0x1C)
    state = struct.unpack(">b", read(engine + 0x89, 1))[0]
    stable = (u32(0x01ABC89C) == boot and u32(0x01ABC8AC) == engine and
              read(boot + 0x118, 0x1C) == fields and read(engine + 0x89, 1) == bytes([state & 255]))
    return {"current": struct.unpack_from(">I", fields)[0], "engine": state, "stable": stable}


class HookLease:
    """Temporarily replace only the entity hook; refuse to overwrite concurrent diagnostics."""

    def __init__(self, path):
        self.path = Path(path)
        self.original = self.path.read_text(encoding="utf-8") if self.path.exists() else ""
        preserved = "".join("# guardian route saved: " + line if HOOK_LINE.match(line) else line
                            for line in self.original.splitlines(keepends=True))
        self.installed = preserved.rstrip("\r\n") + "\n# guardian route: fresh player only\n138e24 256\n"

    def __enter__(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        current = self.path.read_text(encoding="utf-8") if self.path.exists() else ""
        if current != self.original:
            raise RouteStopped("diagnostic watch changed before the player hook could be armed")
        self.path.write_text(self.installed, encoding="utf-8")
        return self

    def __exit__(self, *unused):
        if self.path.read_text(encoding="utf-8") == self.installed:
            self.path.write_text(self.original, encoding="utf-8")


class Guard:
    def __init__(self, memory, tail, output, audit):
        self.memory, self.tail, self.output, self.audit = memory, tail, output, audit
        self.world, self.join_at_start = ClientWorld(), None
        self.walking = False
        self.deadline = None

    def observe(self):
        self.world.update(self.tail.lines())
        state = run_state(self.memory.read)
        state.update(utc=utc_now(), slice_current=self.world.current, slice_target=self.world.target,
                     joins=self.world.joins)
        (self.output / "state.json").write_text(json.dumps(state) + "\n", encoding="utf-8")
        return state

    def check(self):
        state = self.observe()
        if self.deadline is not None and time.monotonic() >= self.deadline:
            raise RouteStopped("route time budget exhausted")
        if not (state["stable"] and state["current"] == 36 and state["engine"] == 0):
            raise RouteStopped(f"mission is loading or leaving: stage {state['current']}, engine {state['engine']}")
        if not self.world.ready():
            raise RouteStopped(f"client slice changed: current {self.world.current}, target {self.world.target}")
        if self.walking and self.world.joins != self.join_at_start:
            raise RouteStopped("client rejoined the activity")
        return state


class GuardedPad(Pad):
    def __init__(self, identity, memory, guard):
        self.identity, self.memory, self.guard = identity, memory, guard
        super().__init__(identity["id"])
        from ctypes import wintypes as wt
        self.u.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
        self.u.IsWindow.argtypes = [wt.HWND]
        memory.kernel.GetExitCodeProcess.argtypes = [wt.HANDLE, ctypes.POINTER(wt.DWORD)]
        self.down = set()

    def owned_window(self):
        from ctypes import wintypes as wt
        pid, status = wt.DWORD(), wt.DWORD()
        self.u.GetWindowThreadProcessId(self.window, ctypes.byref(pid))
        if (not self.u.IsWindow(self.window) or pid.value != self.identity["id"] or
                not self.memory.kernel.GetExitCodeProcess(self.memory.handle, ctypes.byref(status)) or
                status.value != 259):
            raise RouteStopped("owned emulator or game window has exited")

    def _post(self, vk, down):
        self.owned_window()
        if down:
            self.guard.check()
        super()._post(vk, down)
        (self.down.add if down else self.down.discard)(vk)

    def hold(self, keys, ms):
        self.guard.audit(ev="hold", keys=keys, milliseconds=ms)
        pressed = []
        try:
            for key in keys:
                vk = self.VK[key]
                self._post(vk, True)
                pressed.append(vk)
            end = time.monotonic() + ms / 1000
            while time.monotonic() < end:
                time.sleep(min(0.1, max(0, end - time.monotonic())))
                self.guard.check()
        finally:
            for vk in pressed:
                self._post(vk, False)

    def release(self):
        for vk in list(self.down):
            try:
                self._post(vk, False)
            except RouteStopped:
                break


class GuardedPlayer(Player):
    def __init__(self, memory, entity, guard):
        super().__init__(memory, entity)
        self.guard, self.last, self.logged = guard, None, 0

    def position(self):
        self.guard.check()
        position = super().position()
        if not all(math.isfinite(v) and abs(v) < 1e6 for v in position):
            raise RouteStopped("player position is invalid")
        if self.last is not None and math.dist(position, self.last) > 25:
            raise RouteStopped("player position jumped; a fresh entity is required after respawn")
        self.last = position
        if time.monotonic() - self.logged >= 0.5:
            self.guard.audit(ev="position", xyz=position)
            self.logged = time.monotonic()
        return position


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--manifest", required=True, type=Path)
    ap.add_argument("--route", type=Path, default=ROOT / "missions/routes/enter_the_wall.json")
    ap.add_argument("--out-dir", type=Path)
    ap.add_argument("--every", type=int, default=3)
    ap.add_argument("--wait", type=float, default=600, help="seconds to wait for slice112 and idle engine")
    ap.add_argument("--budget", type=float, default=600, help="total walking seconds")
    args = ap.parse_args(argv)
    identity = emulator_identity(json.loads(args.manifest.read_text(encoding="utf-8-sig")))
    if not args.manifest.name.endswith("-processes.json"):
        ap.error("manifest filename must be RUN-processes.json to identify its activity log")
    run = args.manifest.name.removesuffix("-processes.json")
    output = (args.out_dir or args.manifest.parent / f"{run}-guardian-route").resolve()
    if output.is_relative_to(ROOT) and not output.is_relative_to(ROOT / "local"):
        ap.error("route observations must stay outside tracked source or under ignored local/")
    if args.wait <= 0 or args.budget <= 0:
        ap.error("wait and budget must be positive")
    points = route_points(args.route, args.every)
    output.mkdir(parents=True, exist_ok=True)
    emulator_dir = Path(identity["executable"]).parent
    result, pad, grid = {"run": run, "started_utc": utc_now(), "status": "stopped"}, None, None
    with (output / "audit.jsonl").open("a", encoding="utf-8", buffering=1) as stream:
        def audit(**fields):
            stream.write(json.dumps({"utc": utc_now(), **fields}) + "\n")
        def log(message):
            print(message, flush=True)
            audit(ev="walker", message=message)
        try:
            with OwnedMemory(args.manifest) as memory, ExitStack() as cleanup:
                # Release held keys before the verified process handle closes on any exit.
                cleanup.callback(lambda: pad.release() if pad else None)
                guard = Guard(memory, SharedTail(args.manifest.parent / f"{run}-ahl.log"), output, audit)
                ready, deadline = 0, time.monotonic() + args.wait
                saw_world = False
                while time.monotonic() < deadline:
                    state = guard.observe()
                    saw_world |= state["current"] is not None and 33 <= state["current"] <= 36
                    if saw_world and state["current"] == 25:
                        raise RouteStopped("client entered cleanup while waiting")
                    ready = ready + 1 if (state["stable"] and state["current"] == 36 and
                                          state["engine"] == 0 and guard.world.ready()) else 0
                    if ready >= 3:
                        break
                    time.sleep(1)
                if ready < 3:
                    raise RouteStopped("client did not reach idle slice112 before the wait deadline")
                guard.walking, guard.join_at_start = True, guard.world.joins
                guard.deadline = time.monotonic() + args.budget
                # Tail starts before arming, so historical hook hits cannot supply an entity.
                trace = SharedTail(emulator_dir / "log/RPCS3.log", fresh=True)
                with HookLease(emulator_dir / "d1/d1ppu.txt"):
                    entity, deadline = None, time.monotonic() + 30
                    while time.monotonic() < deadline and entity is None:
                        guard.check()
                        hits = [match for line in trace.lines() if (match := HOOK.search(line))]
                        if hits:
                            entity = int(hits[-1][1], 16)
                        else:
                            time.sleep(0.1)
                if not entity or entity + 0x4C > 0x100000000:
                    raise RouteStopped("no fresh follow-player hook from this run")
                player = GuardedPlayer(memory, entity, guard)
                start_position = player.position()
                if math.dist(start_position, points[0][1]) > 25:
                    raise RouteStopped("player is away from the recorded route start")
                facing = facing_from_pointers(memory)
                if facing is None:
                    raise RouteStopped("verified facing pointer unavailable; no probe inputs were sent")
                pad = GuardedPad(identity, memory, guard)
                grid = Grid(output / "grid-112.json")
                walker = Walker(memory, player, pad, (facing, 1), grid, log)
                audit(ev="start", identity=identity, entity=f"{entity:#x}", xyz=start_position,
                      route=str(args.route.resolve()), points=len(points))
                for index, point in points:
                    guard.check()
                    audit(ev="waypoint", route_index=index, xyz=point)
                    if not walker.go(point[:2], radius=3, budget=max(0, guard.deadline - time.monotonic())):
                        raise RouteStopped(f"waypoint {index} was not reached")
                result.update(status="route_reached", final_xyz=player.position())
        except (RouteStopped, OSError, ValueError) as exc:
            result["reason"] = str(exc)
            log(f"stopped: {exc}")
        finally:
            if pad:
                pad.release()
            if grid:
                grid.save()
            result["finished_utc"] = utc_now()
            audit(ev="result", **result)
            (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"{result['status']}: {output / 'result.json'}", flush=True)
    return 0 if result["status"] == "route_reached" else 1


if __name__ == "__main__":
    raise SystemExit(main())
