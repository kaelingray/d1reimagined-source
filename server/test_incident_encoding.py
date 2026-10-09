import json
import tempfile
import unittest
from pathlib import Path

import bap_server as bap
from incident_codec import INCIDENT_HANDLE, decode_incident, encode_incident
from test_cinematic_completion import add_incident_schema


class HostIncidentEncoding(unittest.TestCase):
    def setUp(self):
        self.schema = add_incident_schema({})

    def test_independent_bit_fixture_and_empty_secondary_list(self):
        bits = "001" + f"{1504:012b}" + "0" + f"{2:07b}" + "1010101001010101"
        expected = int(bits + "0", 2).to_bytes(5, "big")
        encoded = encode_incident(self.schema, [1504], bytes.fromhex("aa55"))
        self.assertEqual(encoded, expected)
        self.assertEqual(decode_incident(self.schema, INCIDENT_HANDLE, expected),
                         ((1504,), bytes.fromhex("aa55")))

    def test_chain_slot_range_and_body_limits(self):
        for types, body in (([], b""), ([1] * 5, b""), ([-1], b""),
                            ([4096], b""), ([True], b""), ([1], "aa"),
                            ([1], bytes(65))):
            with self.subTest(types=types), self.assertRaises(ValueError):
                encode_incident(self.schema, types, body)
        self.assertEqual(decode_incident(self.schema, INCIDENT_HANDLE,
                                        encode_incident(self.schema, [1, 2], b"")),
                         ((1, 2), b""))

    def test_count_field_overflow_is_rejected_before_encoding(self):
        self.schema['0x1']['fields'][0]['width'] = 1
        with self.assertRaises(ValueError):
            encode_incident(self.schema, [1, 2], b"")

    def test_control_consumes_once_and_waits_for_a_complete_line(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "control.jsonl"
            line = json.dumps({"incident": {"types": [1504], "body": "aa55"}})
            path.write_text(line)
            changes, _, offset, errors = bap.control_steps(path, 0, {}, self.schema)
            self.assertEqual((changes.incidents, offset, errors), ([], 0, []))
            path.write_text(line + "\n")
            changes, teleports, offset, errors = bap.control_steps(path, offset, {}, self.schema)
            self.assertEqual((len(changes.incidents), teleports, errors), (1, [], []))
            self.assertEqual(decode_incident(self.schema, INCIDENT_HANDLE, changes.incidents[0]),
                             ((1504,), bytes.fromhex("aa55")))
            repeated, _, _, _ = bap.control_steps(path, offset, {}, self.schema)
            self.assertEqual(repeated.incidents, [])

    def test_invalid_control_does_not_queue_an_event(self):
        steps = [{"incident": {"types": [1], "body": "zz"}},
                 {"incident": {"types": [], "body": ""}},
                 {"incident": {"types": [1], "body": "", "other": 0}},
                 {"incident": {"types": [1], "body": ""}, "set": {}},
                 {"incident": {"types": [1], "body": "a" * 16385}}]
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "control.jsonl"
            path.write_text("\n".join(map(json.dumps, steps)) + "\n")
            changes, teleports, offset, errors = bap.control_steps(path, 0, {}, self.schema)
            self.assertEqual((changes.incidents, list(changes), teleports), ([], [], []))
            self.assertEqual(len(errors), len(steps))
            self.assertEqual(offset, path.stat().st_size)


if __name__ == "__main__":
    unittest.main()
