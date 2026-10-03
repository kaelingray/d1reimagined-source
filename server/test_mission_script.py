import json
import unittest
from pathlib import Path

import bap_server as bap
from mission_script import ReactionQueue, reaction_scope


class ReactionScopes(unittest.TestCase):
    def test_scopes_require_an_explicit_pair_and_reactive_state_changes(self):
        self.assertIsNone(reaction_scope({"at": 1, "set": {}}))
        step = {"on": {"type": 18}, "set": {"objective": {"0": 5}}}
        self.assertEqual(reaction_scope({**step, "lane": "objectives", "phase": "weapon"}),
                         ("objectives", "weapon"))
        for invalid in ({"lane": "objectives"}, {"phase": "weapon"},
                        {"lane": "", "phase": "weapon"}, {"lane": "objectives", "phase": 1},
                        {"lane": "objectives", "phase": "weapon", "at": 1},
                        {"lane": "objectives", "phase": "weapon", "set": {}}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                reaction_scope({**step, **invalid})

    def test_new_phase_cancels_only_its_lane_and_keeps_parallel_siblings(self):
        queue = ReactionQueue()
        queue.schedule(10, "old objective", "first", ("objectives", "investigate"))
        queue.schedule(11, "Ghost dialogue", "parallel", ("dialogue", "ghost_light"))
        queue.schedule(12, "unscoped squad", "legacy")
        self.assertEqual(queue.schedule(4, "complete", "next", ("objectives", "weapon")), 1)
        self.assertEqual(queue.schedule(8, "clear/start", "next", ("objectives", "weapon")), 0)
        self.assertEqual(queue.schedule(14, "activate", "next", ("objectives", "weapon")), 0)
        self.assertEqual([spec for _, spec, _ in queue.pop_due(20)],
                         ["Ghost dialogue", "unscoped squad", "complete", "clear/start", "activate"])
        self.assertFalse(queue.has_due(20))

    def test_lane_state_is_per_session(self):
        one, two = ReactionQueue(), ReactionQueue()
        one.schedule(10, "first session", "one", ("objectives", "investigate"))
        two.schedule(10, "second session", "two", ("objectives", "investigate"))
        one.schedule(4, "next phase", "one", ("objectives", "weapon"))
        self.assertEqual([spec for _, spec, _ in one.pop_due(10)], ["next phase"])
        self.assertEqual([spec for _, spec, _ in two.pop_due(10)], ["second session"])


class RapidObjectiveHandover(unittest.TestCase):
    """Successive trigger reports must not reactivate an objective from an earlier phase."""

    def test_guardian_script_rapid_wall_weapon_reports_keep_only_latest_objective_active(self):
        from test_bap_server import MissionScript

        mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                              "a_guardian_rises.json").read_text(encoding="utf-8"))
        _, reactions = bap.expand_mission(mission, MissionScript.SCHEMA)
        queue, states = ReactionQueue(), {}

        def apply(now):
            for _, spec, _ in queue.pop_due(now):
                states[tuple(spec[0][off] for off in (0, 4, 6))] = spec[2]

        def report(now, matches):
            for rule in reactions:
                if matches(rule):
                    _, _, delay, spec = rule
                    queue.schedule(now + delay, spec, "mission", getattr(rule, "scope", None))
            apply(now)

        def object_id(name):
            return tuple(bap.object_spec(mission["objects"][name] + "=")[0][off] for off in (0, 4, 6))

        def trigger(name):
            ident = object_id(name)
            return lambda rule: isinstance(rule[1], bap.ObjectIdMatcher) and rule[1].ident == ident

        # The next volume is reached just after each four-second handover arms its trigger,
        # before the preceding phase's delayed active-state update can run.
        report(0, lambda rule: isinstance(rule[1], bap.SliceSetMatcher) and rule[1].index == 112)
        apply(20)
        report(21, trigger("trig_enter_wall"))
        apply(25)
        report(25.1, trigger("trig_investigate"))
        apply(29.1)
        report(29.2, trigger("trig_get_weapon"))
        apply(40)
        self.assertEqual({name: states[object_id(name)][0] for name in
                          ("obj_enter_wall", "obj_investigate", "obj_get_weapon", "obj_graveyard")},
                         {"obj_enter_wall": 0, "obj_investigate": 0,
                          "obj_get_weapon": 0, "obj_graveyard": 5})

    def test_encrypted_session_rapid_triggers_cancel_stale_activation(self):
        from test_ah_session import ActivityHostSession, objects_in

        fixture = ActivityHostSession()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        mission = {"objects": {name: f"0x10:3:{index}:0x80809002"
                               for index, name in enumerate(("wall", "investigate", "weapon", "fight"))},
                   "steps": []}
        for code, old, new in (("41", "wall", "investigate"),
                               ("42", "investigate", "weapon"), ("43", "weapon", "fight")):
            common = {"on": {"type": 15, "regex": f"^{code}"},
                      "lane": "objectives", "phase": new}
            mission["steps"] += [
                {**common, "set": {old: {"0": 3}}},
                {**common, "delay": 0.15, "set": {old: {"0": 0}, new: {"0": 1}}},
                {**common, "delay": 0.4, "set": {new: {"0": 5}}}]
        path = Path(fixture.tmp.name) / "mission.json"
        path.write_text(json.dumps(mission), encoding="utf-8")
        client = fixture.start("--ah-heartbeat", "0.025:-1", "--ah-mission", str(path),
                               "--ah-baselines", "0x10/3/0,0x10/3/1,0x10/3/2,0x10/3/3")
        pushes = []
        for report, wait in ((b"A", 0.21), (b"B", 0.21), (b"C", 0.5)):
            client.notify(15, report)
            pushes += fixture.run_client(client, wait)[1]
        states = [objects_in(fixture.schema, data) for _, subtype, data in pushes
                  if subtype == bap.AH_SIMULATION]
        self.assertTrue(states)
        self.assertEqual(states[-1], {(0x10, 3, 0): {0: 0}, (0x10, 3, 1): {0: 0},
                                      (0x10, 3, 2): {0: 0}, (0x10, 3, 3): {0: 5}})
        self.assertFalse(any(state[(0x10, 3, 1)] == {0: 5} or state[(0x10, 3, 2)] == {0: 5}
                             for state in states))
        self.assertTrue(any("cancelled" in line for line in fixture.logs))


if __name__ == "__main__":
    unittest.main()
