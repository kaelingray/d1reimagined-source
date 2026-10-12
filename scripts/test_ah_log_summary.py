import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ah_log_summary  # noqa: E402

LOG = """2026-09-27T20:00:00.000 [a] c->ah not host 0x1 type 3 len 4 body 01020304
2026-09-27T20:00:00.100 [a] pushed host join response from host 0x1 (40 bytes)
2026-09-27T20:00:01.000 [a] c->ah not host 0x1 type 15 len 1 body 00
2026-09-27T20:00:01.033 [a] c->ah not host 0x1 type 15 len 1 body 00
2026-09-27T20:00:02.000 [a] c->ah not host 0x1 type 15 len 1 body 41
2026-09-27T20:00:03.000 [a] c->ah not host 0x1 type 15 len 1 body 00
"""


class SummaryTest(unittest.TestCase):
    def test_new_pairs_only_and_counts(self):
        out = io.StringIO()
        counts = ah_log_summary.summarize(LOG.splitlines(True), out=out)
        text = out.getvalue()
        self.assertEqual(counts, {3: 1, 15: 4})
        self.assertEqual(text.count("NEW  type  15"), 2)  # 00 and 41, once each
        self.assertIn("pushed host join response", text)
        self.assertIn("type  15:       4 x,    2 bodies, 20:00:01.000 .. 20:00:03.000", text)

    def test_since_and_prefix(self):
        out = io.StringIO()
        counts = ah_log_summary.summarize(LOG.splitlines(True), since="20:00:01.5", body_bytes=0,
                                          pushes=False, out=out)
        self.assertEqual(counts, {15: 2})
        self.assertEqual(out.getvalue().count("NEW"), 1)

    def test_transition_and_connection_lifecycle_remain_visible(self):
        lines = [
            "2026-10-06T08:00:00.000 [a] world state: teleport to 112 reached (state 3)\n",
            "2026-10-06T08:00:02.000 [a] world state: finished teleport request dropped\n",
            "2026-10-06T08:00:04.000 [a] [WinError 10054] An existing connection was forcibly closed by the remote host\n",
            "2026-10-06T08:00:06.000 [b] host join request 0x123 / 0x1 ids {}\n",
        ]
        out = io.StringIO()
        self.assertEqual(ah_log_summary.summarize(lines, out=out), {})
        text = out.getvalue()
        for expected in ("state 3", "request dropped", "connection was forcibly closed", "host join request"):
            self.assertIn(expected, text)
        quiet = io.StringIO()
        ah_log_summary.summarize(lines, pushes=False, out=quiet)
        self.assertNotIn("world state", quiet.getvalue())


if __name__ == "__main__":
    unittest.main()
