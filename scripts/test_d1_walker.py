import math
import struct
import unittest
from unittest.mock import patch

from d1_traverse import heading
from d1_walker import FACING_PATHS, Walker, facing_from_pointers


class FacingMemory:
    def __init__(self, vector):
        self.address = 0x306E8CFC
        self.vector = vector

    def read(self, address, size):
        if address == FACING_PATHS[0][0]:
            return struct.pack(">I", self.address - FACING_PATHS[0][1][0])
        if address == self.address:
            return struct.pack(">3f", *self.vector)[:size]
        raise OSError("unavailable pointer path")


class CameraFacing(unittest.TestCase):
    def test_join102_pitched_camera_keeps_its_horizontal_heading(self):
        mem = FacingMemory((-0.00490868697, 0.9975581169, -0.0696500242))
        self.assertEqual(facing_from_pointers(mem), mem.address)
        walker = Walker(mem, None, PadReplay(), (0, -1))
        self.assertAlmostEqual(walker.yaw(), math.atan2(mem.vector[1], mem.vector[0]), places=6)
        self.assertEqual((walker.facing, walker.sign), (mem.address, 1))

    def test_level_and_steep_pitched_unit_vectors_are_valid(self):
        for vector in ((1, 0, 0), (0.6, 0, 0.8), (0, -0.6, -0.8)):
            with self.subTest(vector=vector):
                mem = FacingMemory(vector)
                self.assertEqual(facing_from_pointers(mem), mem.address)

    def test_refind_keeps_a_valid_same_address_without_movement_probes(self):
        mem = FacingMemory((0.6, 0, 0.8))
        walker = Walker(mem, None, PadReplay(), (mem.address, -1))
        with patch("d1_walker.find_facing", side_effect=AssertionError("unexpected movement probe")):
            walker.refind()
        self.assertEqual((walker.facing, walker.sign), (mem.address, 1))

    def test_invalid_or_near_vertical_vectors_are_rejected(self):
        vectors = ((0, 0, 0), (1, 1, 0), (0, 0, 1), (0.001, 0, 0.9999995),
                   (math.nan, 0, 0), (1, math.inf, 0), (1, 0, math.nan))
        for vector in vectors:
            with self.subTest(vector=vector):
                self.assertIsNone(facing_from_pointers(FacingMemory(vector)))


class PlayerReplay:
    def __init__(self, positions):
        self.positions = iter(positions)
        self.last = None
        self.reads = 0

    def position(self):
        self.reads += 1
        self.last = next(self.positions)
        return self.last


class PadReplay:
    VK = {"W": 1, "G": 2}

    def __init__(self):
        self.events = []

    def _post(self, key, down):
        self.events.append((key, down))

    def hold(self, keys, milliseconds):
        raise AssertionError("arrival must stop further camera or movement input")


class GridReplay:
    def __init__(self):
        self.saved = 0

    def plan(self, start, goal):
        return [goal]

    def save(self):
        self.saved += 1


class WalkerArrival(unittest.TestCase):
    def test_stride_latches_join92_goal_before_the_next_position_overshoots(self):
        goal = (353.7513732910156, -796.29150390625)
        before = (352.4, -797.1, 14.0)
        # The live run observed this valid arrival, then read outside the 1.5 radius
        # after movement-key release and spent the final entry deadline turning back.
        at_goal = (354.3870849609375, -796.1572875976562, 14.048073768615723)
        overshoot = (355.595, -795.8, 14.1)
        self.assertLess(math.dist(at_goal[:2], goal), 1.5)
        self.assertGreater(math.dist(overshoot[:2], goal), 1.5)
        player = PlayerReplay([before] * 5 + [at_goal, overshoot])
        pad, grid, messages = PadReplay(), GridReplay(), []
        walker = Walker(None, player, pad, (0, 1), grid, messages.append)
        walker.yaw = lambda: heading(player.last, goal)
        with patch("d1_walker.time.sleep"):
            self.assertTrue(walker.go(goal, radius=1.5, budget=20))
        self.assertEqual(player.reads, 6)
        self.assertEqual(pad.events, [(1, True), (2, True), (1, False), (2, False)])
        self.assertEqual(grid.saved, 1)
        self.assertIn("at [354.4, -796.2, 14.0]", messages[-1])

    def test_arrival_while_coasting_cancels_facing_without_movement_input(self):
        goal = (10, 0)
        player = PlayerReplay([(8, 0, 1), (9, 0, 1)])
        pad, grid = PadReplay(), GridReplay()
        walker = Walker(None, player, pad, (0, 1), grid, lambda unused: None)
        walker.yaw = lambda: self.fail("arrival should be checked before turning")
        self.assertTrue(walker.go(goal, radius=1.5, budget=20))
        self.assertEqual(player.reads, 2)
        self.assertEqual(pad.events, [])
        self.assertEqual(grid.saved, 1)

    def test_arrival_check_does_not_hide_a_guard_failure(self):
        class Player:
            reads = 0

            def position(self):
                self.reads += 1
                if self.reads == 1:
                    return (8, 0, 1)
                raise RuntimeError("client entered cleanup")

        pad = PadReplay()
        walker = Walker(None, Player(), pad, (0, 1), GridReplay(), lambda unused: None)
        with self.assertRaisesRegex(RuntimeError, "cleanup"):
            walker.go((10, 0), radius=1.5, budget=20)
        self.assertEqual(pad.events, [])


class WalkerPrecision(unittest.TestCase):
    def test_close_goal_keeps_moving_until_a_slow_pad_poll_consumes_input(self):
        # A 100 ms press/release can fall entirely between game input polls.
        # The final goal is inside the intermediate two-unit waypoint tolerance,
        # but outside the caller's one-unit arrival tolerance.
        class Clock:
            now = 0.0

            def time(self):
                self.now += 0.01
                return self.now

            def sleep(self, seconds):
                self.now += seconds

        clock = Clock()

        class Pad:
            VK = {"W": "W", "G": "G"}
            down_at = None

            def _post(self, key, down):
                if key == "W":
                    self.down_at = clock.now if down else None

        pad = Pad()

        class Player:
            x = 0.0

            def position(self):
                if pad.down_at is not None and clock.now - pad.down_at >= 0.25:
                    self.x += 0.4
                return (self.x, 0.0, 0.0)

        player = Player()
        walker = Walker(None, player, pad, (0, 1), log=lambda _: None)
        walker.yaw = lambda: 0.0
        with patch("d1_walker.time.time", clock.time), patch("d1_walker.time.sleep", clock.sleep):
            self.assertTrue(walker.go((1.5, 0.0), radius=1.0, budget=2.0))
        self.assertGreaterEqual(player.x, 0.5)
        self.assertIsNone(pad.down_at)


if __name__ == "__main__":
    unittest.main()
