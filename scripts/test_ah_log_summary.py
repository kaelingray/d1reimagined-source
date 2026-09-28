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


if __name__ == "__main__":
    unittest.main()
