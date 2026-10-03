import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from d1_guardian_route import BREACH_SLICE, WALL_SLICE, ClientWorld, Guard, GuardedPad, GuardedPlayer, HOOK, HookLease, RouteStopped, SharedTail, WALL_ROUTE, route_points, run_state


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

    def test_wall_route_retains_successful_climb_and_approach(self):
        # JOIN88 used these 29 targets; point zero is also kept to check the initial position.
        targets = [1, 2, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42,
                   44, 45, 46, 47, 48, 49, 50, 51, 54, 57, 60, 63, 66]
        selected = route_points(WALL_ROUTE)
        self.assertEqual([i for i, _ in selected], [0, *targets])
        recorded = json.loads(WALL_ROUTE.read_text())
        self.assertEqual(dict(selected)[47], recorded[47])
        # An explicit lower point density still cannot omit the narrow climb's turn.
        sparse = dict(route_points(WALL_ROUTE, every=8))
        self.assertTrue(all(i in sparse for i in range(44, 52)))

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
                  0x2118: fields, 0x3089: b"\0", 0x3090: struct.pack(">iI", *WALL_SLICE),
                  0x3130: struct.pack(">iI", -1, 0x811C9DC5)}
        read = lambda address, size: values[address]
        self.assertEqual(run_state(read), {"current": 36, "engine": 0, "stable": True,
                                          "native_slice_current": WALL_SLICE, "native_slice_target": None})
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
        reads = 0
        def changing_target(address, size):
            nonlocal reads
            if address == 0x3130:
                reads += 1
                if reads == 2:
                    return struct.pack(">iI", *BREACH_SLICE)
            return values[address]
        self.assertFalse(run_state(changing_target)["stable"])

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

    def preload_guard(self):
        class Memory:
            read = None
        class Tail:
            pending = []
            def lines(self):
                lines, self.pending = self.pending, []
                return lines
        guard = Guard(Memory(), Tail(), self.root, lambda **event: None, allow_breach_preload=True)
        guard.world.current = WALL_SLICE
        guard.walking = True
        guard.join_at_start = guard.world.joins = 1
        return guard

    def test_opt_in_preload_uses_native_target_before_its_log_report(self):
        guard = self.preload_guard()
        state = {"current": 36, "engine": 3, "stable": True,
                 "native_slice_current": WALL_SLICE, "native_slice_target": BREACH_SLICE}
        with patch("d1_guardian_route.run_state", return_value=state):
            # JOIN89's engine3 preceded its AHL target24 report by about 51 seconds.
            self.assertIsNone(guard.world.target)
            guard.check()
            guard.world.target = BREACH_SLICE
            guard.check()
            guard.allow_breach_preload = False
            with self.assertRaisesRegex(RouteStopped, "engine 3"):
                guard.check()

    def test_preload_rejects_unexpected_targets_hashes_source_and_engine_states(self):
        valid = {"current": 36, "engine": 3, "stable": True,
                 "native_slice_current": WALL_SLICE, "native_slice_target": BREACH_SLICE}
        cases = [{"engine": 6}, {"engine": 2}, {"current": 25}, {"stable": False},
                 {"native_slice_current": (-1, 0x811C9DC5)}, {"native_slice_current": BREACH_SLICE},
                 {"native_slice_target": None}, {"native_slice_target": (24, 0x811C9DC5)},
                 {"native_slice_target": (104, 0x811C9DC5)}]
        for changes in cases:
            with self.subTest(changes=changes):
                guard = self.preload_guard()
                with patch("d1_guardian_route.run_state", return_value={**valid, **changes}):
                    with self.assertRaises(RouteStopped):
                        guard.check()
        for reported in ((24, 0x811C9DC5), (104, 0x811C9DC5)):
            guard = self.preload_guard()
            guard.world.target = reported
            with patch("d1_guardian_route.run_state", return_value=valid):
                with self.assertRaisesRegex(RouteStopped, "slice changed"):
                    guard.check()
        guard = self.preload_guard()
        guard.world.joins += 1
        with patch("d1_guardian_route.run_state", return_value=valid):
            with self.assertRaisesRegex(RouteStopped, "rejoined"):
                guard.check()

    def test_preload_fresh_player_hook_stops_on_replacement_or_staleness(self):
        guard = self.preload_guard()
        class Trace:
            pending = []
            def lines(self):
                lines, self.pending = self.pending, []
                return lines
        trace = Trace()
        with patch("d1_guardian_route.time.monotonic", return_value=100):
            guard.bind_player(trace, 0x60000000)
        trace.pending = [b"pc=0x138e24 lr=0x352a4c r4=0x60000000"]
        with patch("d1_guardian_route.time.monotonic", return_value=102):
            guard.check_player()
        with patch("d1_guardian_route.time.monotonic", return_value=106):
            with self.assertRaisesRegex(RouteStopped, "no longer fresh"):
                guard.check_player()
        trace.pending = [b"pc=0x138e24 lr=0x352a4c r4=0x62000000"]
        with self.assertRaisesRegex(RouteStopped, "entity changed"):
            guard.check_player()

    def test_guarded_player_stops_on_respawn_position_jump(self):
        class Memory:
            position = (383, -962, 12)
            def read(self, address, size):
                return struct.pack(">fff", *self.position)
        class ReadyGuard:
            def check(self):
                pass
            def audit(self, **unused):
                pass
        memory = Memory()
        player = GuardedPlayer(memory, 0x60000000, ReadyGuard())
        player.position()
        memory.position = (336, -898, 4)
        with self.assertRaisesRegex(RouteStopped, "position jumped"):
            player.position()


if __name__ == "__main__":
    unittest.main()
