import copy
import json
import unittest
from pathlib import Path

import bap_server as bap
import investment
from incident_codec import INCIDENT_HANDLE
from mission_script import ReactionQueue, TeleportReaction
import test_bap_server as fixtures


def field(off, kind=5, width=8, nested=0, arr=0):
    return dict(off=off, kind=kind, width=width, nested=nested, arr=arr, min=0, opt=0)


def add_incident_schema(schema):
    schema = copy.deepcopy(schema)
    schema[hex(INCIDENT_HANDLE)] = {"fields": [field(0, 1, 0, 1),
                                             field(104, 1, 0, 2), field(212, 1, 0, 3)]}
    for handle, capacity, count_width, item_width in ((1, 4, 3, 12), (2, 1, 1, 8), (3, 64, 7, 8)):
        schema[hex(handle)] = {"fields": [field(0, width=count_width),
                                        field(4, 1, 0, handle + 10, 1)]}
        schema[hex(handle + 10)] = {"fields": [field(i * 4, width=item_width) for i in range(capacity)]}
    return schema


def schema_fixture():
    return add_incident_schema(fixtures.MissionScript.SCHEMA)


def opaque(index=1):
    # Independent native bit layout: 303 common bits, group32/kind6/index16,
    # then two words and three byte-padding bits. No codec round-trip oracle.
    bits = '0' * 303 + f'{0x992BB471:032b}{7:06b}{index + 0x8000:016b}' + '0' * 67
    return int(bits, 2).to_bytes(53, 'big')


def envelope(schema, body=None, types=(734,)):
    body = opaque() if body is None else body
    return investment.encode(schema, INCIDENT_HANDLE,
                             {0: {0: len(types), 4: list(types)},
                              104: {0: 0, 4: []}, 212: {0: len(body), 4: list(body)}})


class CinematicCompletion(unittest.TestCase):
    def setUp(self):
        self.schema = schema_fixture()
        self.matcher = bap.CinematicCompletionMatcher(self.schema, {0: 0x992BB471, 4: 6, 6: 1})

    def test_completion_matches_only_the_named_cinematic(self):
        self.assertTrue(self.matcher.search(envelope(self.schema).hex()))
        self.assertFalse(self.matcher.search(envelope(self.schema, opaque(2)).hex()))

    def test_started_and_other_incident_chains_do_not_advance(self):
        for types in ((2120,), (469,), (734, 2120), (734, 734), ()):
            with self.subTest(types=types):
                self.assertFalse(self.matcher.search(envelope(self.schema, types=types).hex()))

    def test_an_object_id_in_an_unrelated_body_field_does_not_match(self):
        body = bytearray(opaque(2))
        body[:8] = bytes.fromhex('992bb4711e000400')
        self.assertFalse(self.matcher.search(envelope(self.schema, body).hex()))

    def test_malformed_body_envelope_and_padding_are_rejected(self):
        valid = envelope(self.schema)
        invalid = ('not hex', valid[:-1].hex(), (valid + b'\0').hex(), (valid[:-1] + b'\xff').hex())
        for value in invalid:
            with self.subTest(value=value):
                self.assertFalse(self.matcher.search(value))
        self.assertFalse(self.matcher.search(envelope(self.schema, opaque()[:-1]).hex()))
        body = bytearray(opaque()); body[-1] |= 1
        self.assertFalse(self.matcher.search(envelope(self.schema, body).hex()))

    def test_missing_schema_and_non_cinematic_object_fail_before_launch(self):
        with self.assertRaises(ValueError):
            bap.CinematicCompletionMatcher({}, {0: 1, 4: 6, 6: 1})
        with self.assertRaises(ValueError):
            bap.CinematicCompletionMatcher(self.schema, {0: 1, 4: 29, 6: 1})

    def test_completion_rule_and_teleport_are_reactive_and_preflightable(self):
        mission = {"objects": {"cine": '0x992bb471:6:1:0x80800463'}, "steps": [
            {"on": {"type": 18, "object": "cine", "cinematic_complete": True},
             "set": {"cine": {"0": False}}},
            {"on": {"type": 18, "object": "cine", "cinematic_complete": True},
             "delay": 5, "teleport": 112}]}
        timed, reactions = bap.expand_mission(mission, self.schema)
        self.assertEqual(timed, [])
        self.assertEqual(bap.mission_teleports(mission), [])
        self.assertEqual(bap.validate_mission_states(self.schema, timed, reactions), 2)
        self.assertTrue(all(rule[1].search(envelope(self.schema).hex()) for rule in reactions))
        self.assertEqual(reactions[1][3], TeleportReaction((112, None)))
        queue = ReactionQueue()
        for kind, matcher, delay, action in reactions:
            queue.schedule(10 + delay, action, 'completion')
        self.assertEqual(len(queue.pop_due(10)), 1)
        self.assertFalse(queue.has_due(14.99))
        self.assertEqual(queue.pop_due(15)[0][1], TeleportReaction((112, None)))

    def test_completion_configuration_cannot_fall_back_to_generic_id_matching(self):
        for on in ({"type": 21, "object": "cine", "cinematic_complete": True},
                   {"type": 18, "cinematic_complete": True},
                   {"type": 18, "object": "cine", "cinematic_complete": False},
                   {"type": 18, "object": "cine", "cinematic_complete": 'false'}):
            with self.subTest(on=on), self.assertRaises(ValueError):
                bap.expand_mission({"objects": {"cine": '1:6:1:0x80800463'},
                                    "steps": [{"on": on, "teleport": 112}]}, self.schema)
        with self.assertRaises(ValueError):
            bap.expand_mission({"objects": {"cine": '1:6:1:0x80800463'},
                                "steps": [{"on": {"type": 18},
                                           "set": {"cine": {"0": False}}, "teleport": 112}]}, self.schema)

    def test_shipped_revival_does_not_stop_or_teleport_on_a_host_timer(self):
        mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                              "a_guardian_rises.json").read_text(encoding="utf-8"))
        timed, reactions = bap.expand_mission(mission, self.schema)
        self.assertEqual([(at, spec[0][6]) for at, spec in timed], [(20, 1)])
        self.assertEqual(bap.mission_teleports(mission), [])
        completions = [rule for rule in reactions if isinstance(rule[1], bap.CinematicCompletionMatcher)]
        self.assertEqual([(rule[1].ident[2], rule[2]) for rule in completions],
                         [(1, 0), (1, 0), (2, 0), (2, 5)])
        teleport = [rule for rule in completions if isinstance(rule[3], TeleportReaction)]
        self.assertEqual(len(teleport), 1)
        self.assertFalse(teleport[0][1].search(envelope(self.schema, opaque(1)).hex()))
        self.assertTrue(teleport[0][1].search(envelope(self.schema, opaque(2)).hex()))


if __name__ == '__main__':
    unittest.main()
