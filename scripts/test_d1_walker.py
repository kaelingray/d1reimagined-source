import math
import unittest
from unittest.mock import patch

from d1_traverse import heading
from d1_walker import Walker


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


if __name__ == "__main__":
    unittest.main()
