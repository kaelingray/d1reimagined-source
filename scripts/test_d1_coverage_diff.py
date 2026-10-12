import tempfile
import unittest
from pathlib import Path

import d1_coverage_diff as cov

STUCK = """\
·W 0:00:10.000000 {PPU[0x1000000] Thread (main_thread) [0x00100000]} PPU: D1C 1 pc=0x100000 lr=0x200000
·W 0:01:30.500000 {PPU[0x1000009] Thread (async_job #0/2) [0x0014e864]} PPU: D1C 2 pc=0x14e864 lr=0x532718
·W 0:01:31.000000 {PPU[0x1000009] Thread (async_job #0/2) [0x0014e864]} PPU: D1C 3 pc=0x14e864 lr=0x999999
·W 0:01:32.000000 {RSX [0x0000000]} D1: D1MEM surface=0M
"""
PASS = """\
·W 0:00:10.000000 {PPU[0x1000000] Thread (main_thread) [0x00100000]} PPU: D1C 1 pc=0x100000 lr=0x200000
·W 0:01:40.250000 {PPU[0x100000a] Thread (async_job #1/2) [0x00531e78]} PPU: D1C 2 pc=0x531e78 lr=0x532050
"""


class CoverageDiffTest(unittest.TestCase):
    def logs(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        stuck, passing = Path(tmp.name, "stuck.log"), Path(tmp.name, "pass.log")
        stuck.write_text(STUCK, encoding="utf-8")
        passing.write_text(PASS, encoding="utf-8")
        return cov.first_calls(stuck), cov.first_calls(passing)

    def test_first_call_wins_and_non_d1c_lines_are_ignored(self):
        stuck, _ = self.logs()
        self.assertEqual(set(stuck), {0x100000, 0x14E864})
        t, thread, lr = stuck[0x14E864]
        self.assertAlmostEqual(t, 90.5)
        self.assertEqual(thread, "PPU[0x1000009] Thread (async_job #0/2)")
        self.assertEqual(lr, 0x532718)

    def test_only_in_respects_after_and_range(self):
        stuck, passing = self.logs()
        self.assertEqual([row[1] for row in cov.only_in(passing, stuck)], [0x531E78])
        self.assertEqual(cov.only_in(passing, stuck, after=cov.seconds("0:02:00")), [])
        self.assertEqual(cov.only_in(stuck, passing, low=0x14E000, high=0x14EFFF)[0][1], 0x14E864)


if __name__ == "__main__":
    unittest.main()
