import copy
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_incident_summary as summary  # noqa: E402


def field(offset, kind=5, width=8, nested=0, arr=0):
    return {"off": offset, "kind": kind, "width": width, "nested": nested,
            "arr": arr, "min": 0, "opt": 0}


def schema_fixture():
    schema = {f"{summary.INCIDENT_HANDLE:#x}": {"fields": [
        field(0, 1, 0, 1), field(104, 1, 0, 2), field(212, 1, 0, 3)]}}
    for handle, capacity, count_width, item_width in ((1, 4, 3, 12), (2, 1, 1, 8), (3, 16, 5, 8)):
        schema[hex(handle)] = {"fields": [field(0, width=count_width), field(4, 1, 0, handle + 10, 1)]}
        schema[hex(handle + 10)] = {"fields": [field(i * 4, width=item_width) for i in range(capacity)]}
    return schema


def payload(schema, types, body, participants=()):
    return summary.investment.encode(schema, summary.INCIDENT_HANDLE,
                                     {0: {0: len(types), 4: list(types)},
                                      104: {0: len(participants), 4: list(participants)},
                                      212: {0: len(body), 4: list(body)}})


def log_line(stamp, body, ah_type=18):
    return f"{stamp} [test] c->ah not host 0x1 type {ah_type} len {len(body)} body {body.hex()}\n"


class IncidentSummaryTest(unittest.TestCase):
    def setUp(self):
        self.schema = schema_fixture()

    def test_chain_occurrences_are_distinct_from_message_counts(self):
        lines = [log_line("2026-10-02T18:00:00.000", payload(self.schema, [469, 2841, 1623], b"\0\0\0\1abcd")),
                 log_line("2026-10-02T18:00:00.250", payload(self.schema, [2841, 1623, 2841, 1623], b"\0\0\0\2abcd")),
                 log_line("2026-10-02T18:00:00.300", b"\0", 15)]
        report = summary.summarize(io.StringIO("".join(lines)), self.schema)
        self.assertEqual(report["ah_message_counts"], {15: 1, 18: 2})
        self.assertEqual(report["ah_messages"][0]["last"], "2026-10-02T18:00:00.300")
        types = {row["type"]: row for row in report["types"]}
        self.assertEqual((types[2841]["messages"], types[2841]["occurrences"]), (2, 3))
        self.assertEqual(types[2841]["body_lengths"], {8: 2})
        self.assertEqual(types[2841]["body_variants"]["distinct"], 2)
        self.assertEqual(types[2841]["body_after_leading_u32_variants"]["distinct"], 1)
        self.assertEqual(types[2841]["interval_seconds"]["mean"], 0.25)
        self.assertNotIn(2665, types)

    def test_repeated_body_and_chain_keep_timing_without_raw_payload(self):
        data = payload(self.schema, [2120], b"unique raw body")
        report = summary.summarize(io.StringIO(log_line("2026-10-02T18:00:00", data)
                                              + log_line("2026-10-02T18:00:01", data)), self.schema)
        row = report["chains"][0]
        self.assertEqual(row["body_variants"]["distinct"], 1)
        self.assertEqual(list(row["body_variants"]["digests"].values()), [2])
        self.assertEqual(row["interval_seconds"]["samples"], 1)
        self.assertNotIn("unique raw body", str(report))
        out = io.StringIO()
        summary.render(report, out=out)
        self.assertIn("do not prove", out.getvalue())
        self.assertNotIn(data.hex(), out.getvalue())

    def test_malformed_length_hex_padding_and_truncation_are_counted(self):
        data = payload(self.schema, [2120], b"a")
        valid = log_line("2026-10-02T18:00:00", data)
        invalid = valid.replace(" body ", " body z")
        wrong_length = valid.replace(f"len {len(data)}", f"len {len(data) + 1}")
        lines = invalid + wrong_length + log_line("2026-10-02T18:00:01", data[:-1])
        lines += log_line("2026-10-02T18:00:02", data + b"\0")
        lines += log_line("2026-10-02T18:00:03", data[:-1] + bytes([data[-1] | 1])) + valid
        report = summary.summarize(io.StringIO(lines), self.schema)
        self.assertEqual(report["counters"]["malformed_incidents"], 5)
        self.assertEqual(report["counters"]["decoded_messages"], 1)

    def test_over_capacity_count_does_not_silently_decode(self):
        writer = summary.investment.BitWriter()
        writer.put(5, 3)  # five types, while the schema has only four slots
        for _ in range(5):
            writer.put(1, 12)
        writer.put(0, 1)
        writer.put(0, 5)
        with self.assertRaises(ValueError):
            summary.decode_incident(self.schema, summary.INCIDENT_HANDLE, writer.data())

    def test_oversized_lines_are_discarded_and_following_record_survives(self):
        data = payload(self.schema, [734], b"a")
        stream = io.StringIO("x" * (summary.MAX_LINE * 3) + "\n"
                             + log_line("2026-10-02T18:00:00", data))
        report = summary.summarize(stream, self.schema)
        self.assertEqual(report["counters"]["oversized_lines"], 1)
        self.assertEqual(report["counters"]["decoded_messages"], 1)

    def test_variant_and_chain_limits_report_incomplete_counts(self):
        lines = "".join(log_line(f"2026-10-02T18:00:0{i}", payload(self.schema, [i], bytes([i])))
                        for i in range(4))
        with patch.object(summary, "MAX_DIGESTS", 2), patch.object(summary, "MAX_CHAINS", 2):
            report = summary.summarize(io.StringIO(lines), self.schema)
        variants = report["all_incidents"]["body_variants"]
        self.assertIsNone(variants["distinct"])
        self.assertEqual(variants["distinct_at_least"], 3)
        self.assertEqual(variants["untracked_observations"], 2)
        self.assertEqual(report["counters"]["untracked_chain_messages"], 2)

    def test_midnight_and_backward_timestamp_intervals(self):
        data = payload(self.schema, [734], b"a")
        report = summary.summarize(io.StringIO(log_line("23:59:59.750", data)
                                              + log_line("00:00:00.000", data)), self.schema)
        self.assertEqual(report["all_incidents"]["interval_seconds"]["mean"], 0.25)
        report = summary.summarize(io.StringIO(log_line("2026-10-02T18:00:02", data)
                                              + log_line("2026-10-02T18:00:01", data)), self.schema)
        self.assertEqual(report["all_incidents"]["interval_seconds"]["nonmonotonic"], 1)

    def test_wrong_schema_and_recursive_schema_fail_before_decoding(self):
        wrong = copy.deepcopy(self.schema)
        wrong[f"{summary.INCIDENT_HANDLE:#x}"]["fields"][1]["off"] = 100
        with self.assertRaisesRegex(ValueError, "01.24"):
            summary.summarize(io.StringIO(""), wrong)
        wrong = copy.deepcopy(self.schema)
        wrong["0xb"]["fields"][0] = field(0, 1, 0, 11)
        with self.assertRaisesRegex(ValueError, "recursive"):
            summary.summarize(io.StringIO(""), wrong)

    def test_output_is_resolved_before_repository_privacy_check(self):
        repo = Path(__file__).resolve().parents[1]
        with self.assertRaisesRegex(ValueError, "outside"):
            summary.private_output(repo / "notes" / "incident-report.json")
        with self.assertRaises(ValueError):
            summary.private_output(repo / "local" / ".." / "incident-report.json")
        self.assertEqual(summary.private_output(repo / "local" / "reports" / "incidents.json"),
                         (repo / "local" / "reports" / "incidents.json").resolve())


if __name__ == "__main__":
    unittest.main()
