import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from d1_guardian_route import ClientWorld, Guard, GuardedPad, HOOK, HookLease, RouteStopped, SharedTail, route_points, run_state


class GuardianRoute(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_decimation_includes_recorded_endpoint(self):
        path = self.root / "route.json"
        path.write_text(json.dumps([[i, i + 1, 2] for i in range(8)]))
        self.assertEqual([i for i, _ in route_points(path)], [0, 3, 6, 7])
        path.write_text('[[1, 2, NaN]]')
        with self.assertRaises(ValueError):
            route_points(path)

    def test_fresh_tail_rejects_historical_entity_and_wrong_caller(self):
        path = self.root / "RPCS3.log"
        path.write_bytes(b"D1P 1 pc=0x138e24 lr=0x352a4c r4=0x60000000\n")
        tail = SharedTail(path, fresh=True)
        self.assertEqual(tail.lines(), [])
        with path.open("ab") as stream:
            stream.write(b"D1P 2 pc=0x138e24 lr=0x11111 r4=0x61000000\n")
            stream.write(b"D1P 3 pc=0x138e24 lr=0x352a4c r4=0x62000000")
        self.assertFalse(any(HOOK.search(line) for line in tail.lines()))
        with path.open("ab") as stream:
            stream.write(b" thread=main\n")
        self.assertEqual(int(HOOK.search(tail.lines()[0])[1], 16), 0x62000000)
        path.write_bytes(b"")
        with self.assertRaises(RouteStopped):
            tail.lines()

    def test_hook_lease_preserves_other_hooks_and_concurrent_changes(self):
        path = self.root / "d1ppu.txt"
        original = "# diagnostics\n20f858 15 r3:4\n138e24 3 r4:16\n"
        path.write_text(original)
        with HookLease(path):
            self.assertIn("20f858 15 r3:4", path.read_text())
            self.assertIn("# guardian route saved: 138e24 3 r4:16", path.read_text())
        self.assertEqual(path.read_text(), original)
        with HookLease(path):
            path.write_text("# operator changed the diagnostics\n1df644 20\n")
        self.assertEqual(path.read_text(), "# operator changed the diagnostics\n1df644 20\n")

    def test_slice_target_does_not_prove_arrival_and_cleared_target_is_safe(self):
        world = ClientWorld()
        world.update([b"client slice set: target 112:0x811c9dc5, state 4"])
        self.assertFalse(world.ready())
        world.update([b"client slice set: current 112:0x811c9dc5, target 0:0x00000000"])
        self.assertTrue(world.ready())
        world.update([b"client slice set: target 24:0x68935472"])
        self.assertFalse(world.ready())
        world.update([b"host join request 1", b"host join request 2"])
        self.assertEqual(world.joins, 2)

    def test_boot_state_checks_engine_and_nonatomic_changes(self):
        fields = struct.pack(">7I", 36, 0, 0, 0, 36, 0, 0)
        values = {0x01ABC89C: struct.pack(">I", 0x2000), 0x01ABC8AC: struct.pack(">I", 0x3000),
                  0x2118: fields, 0x3089: b"\0"}
        read = lambda address, size: values[address]
        self.assertEqual(run_state(read), {"current": 36, "engine": 0, "stable": True})
        values[0x3089] = b"\xff"
        self.assertEqual(run_state(read)["engine"], -1)
        reads = 0
        def changing(address, size):
            nonlocal reads
            if address == 0x2118:
                reads += 1
                if reads == 2:
                    return struct.pack(">7I", 25, 0, 0, 0, 25, 0, 0)
            return values[address]
        self.assertFalse(run_state(changing)["stable"])

    def test_guard_stops_for_world_change_rejoin_and_loading(self):
        class Memory:
            read = None
        class Tail:
            def lines(self):
                return []
        guard = Guard(Memory(), Tail(), self.root, lambda **event: None)
        guard.world.current = (112, 0x811C9DC5)
        guard.world.joins = guard.join_at_start = 1
        guard.walking = True
        with patch("d1_guardian_route.run_state", return_value={"current": 36, "engine": 0, "stable": True}):
            guard.check()
            guard.world.target = (24, 0x68935472)
            with self.assertRaisesRegex(RouteStopped, "slice changed"):
                guard.check()
            guard.world.target = None
            guard.world.joins = 2
            with self.assertRaisesRegex(RouteStopped, "rejoined"):
                guard.check()
        with patch("d1_guardian_route.run_state", return_value={"current": 25, "engine": -1, "stable": True}):
            with self.assertRaisesRegex(RouteStopped, "loading or leaving"):
                guard.check()

    def test_guarded_hold_releases_keys_if_mission_changes_mid_press(self):
        class FailingGuard:
            def audit(self, **unused):
                pass
            def check(self):
                raise RouteStopped("loading")
        pad = GuardedPad.__new__(GuardedPad)
        pad.guard = FailingGuard()
        posts = []
        pad._post = lambda vk, down: posts.append((vk, down))
        with self.assertRaises(RouteStopped):
            pad.hold(["W", "G"], 5)
        self.assertEqual(posts, [(pad.VK["W"], True), (pad.VK["G"], True),
                                 (pad.VK["W"], False), (pad.VK["G"], False)])


if __name__ == "__main__":
    unittest.main()
