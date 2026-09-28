import json
import tempfile
import unittest
from pathlib import Path

import run_log


class RunLog(unittest.TestCase):
    def events(self):
        return [{"t": "2026-09-28T10:00:00.000", "ev": "frame", "n": 1, "kind": 2, "type": 25, "name": "shello req",
                 "id": 1, "payload": ""},
                {"t": "2026-09-28T10:00:01.000", "ev": "ah", "n": 2, "host": "0x1", "ah_type": 6, "body": "80ff00"},
                {"t": "2026-09-28T10:00:02.000", "ev": "ah", "n": 3, "host": "0x1", "ah_type": 6, "body": "81ff01"},
                {"t": "2026-09-28T10:00:03.000", "ev": "ah", "n": 4, "host": "0x1", "ah_type": 6, "body": "80ff00"},
                {"t": "2026-09-28T10:00:04.000", "log": "[x] --ah-on rule 0 matched type 6: object 0x10:0x3:1 in 0 s"}]

    def test_summary_counts_and_rules(self):
        text = "\n".join(run_log.summary(self.events()))
        self.assertIn("     1 shello req", text)
        self.assertIn("     3 type 6", text)
        self.assertIn("10:00:04.000 --ah-on rule 0 matched", text)

    def test_varying_bits(self):
        bodies = [bytes.fromhex(e["body"]) for e in run_log.host_messages(self.events(), 6)]
        count, varying = run_log.varying_bits(bodies)[3]
        self.assertEqual(count, 3)
        self.assertEqual(varying, [(7, 1), (23, 1)])
        self.assertEqual(run_log.ranges([7, 8, 9, 23]), [(7, 9), (23, 23)])

    def test_since_and_folder_argument(self):
        with tempfile.TemporaryDirectory() as folder:
            Path(folder, "events.jsonl").write_text("\n".join(json.dumps(e) for e in self.events()), encoding="utf-8")
            later = list(run_log.host_messages(run_log.events([folder]), 6, since="10:00:02"))
        self.assertEqual([e["n"] for e in later], [3, 4])


if __name__ == "__main__":
    unittest.main()
