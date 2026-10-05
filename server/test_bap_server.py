import json
import struct
import tempfile
import unittest
from pathlib import Path

import bap_server as bap


class ClientZoneRequest(unittest.TestCase):
    def test_cleared_target_does_not_request_skywatch(self):
        self.assertIsNone(bap.requested_client_zone({"target": (0, 0)}))
        self.assertEqual(bap.requested_client_zone({"target": (0, 0),
                                                   "current": (112, 0x811C9DC5)}), 112)

    def test_actual_zone_zero_and_named_target_are_preserved(self):
        self.assertEqual(bap.requested_client_zone({"target": (0, 0x811C9DC5)}), 0)
        self.assertEqual(bap.requested_client_zone({"current": (112, 0x811C9DC5),
                                                   "target": (24, 0x68935472)}), 24)
        for record in ({}, {"state": 4}, {"target": (-1, 0x811C9DC5)},
                       {"target": (None, 0x811C9DC5)}):
            self.assertIsNone(bap.requested_client_zone(record))


class UserMessageReply(unittest.TestCase):
    def test_default_is_required_field_four_only(self):
        self.assertEqual(bap.user_msg_body(""), bytes.fromhex("2000"))

    def test_text_fields_and_integer(self):
        body = bap.user_msg_body("1=BODY|2=TITLE|4=1")
        self.assertEqual(body, bytes.fromhex("0a04424f445912055449544c452001"))
        with self.assertRaises(ValueError):
            bap.user_msg_body("5=x")


class ShelloReply(unittest.TestCase):
    key, mac_key = b"k" * 16, b"m" * 16

    def test_sealed_reply_fits_client_limits(self):
        blob = bap.shello_rsp_body(self.key, self.mac_key, b"n" * 12, b"s" * 16, b"i" * 16)
        self.assertEqual(len(blob), 84)
        self.assertEqual(struct.unpack_from(">I", blob)[0], 80)  # codec: first u32 == size - 4
        self.assertEqual(blob[4:20], b"i" * 16)
        self.assertEqual(bap.unseal(self.key, self.mac_key, blob), b"n" * 12 + b"s" * 16)
        tampered = blob[:30] + bytes([blob[30] ^ 1]) + blob[31:]
        with self.assertRaises(ValueError):
            bap.unseal(self.key, self.mac_key, tampered)

    def test_plain_frame_layout(self):
        frame = bap.plain_frame(bap.SHELLO_RSP, 0, 200, b"xyz")
        self.assertEqual(frame[:6], bytes.fromhex("01020000000b"))
        self.assertEqual(frame[6:14], bytes.fromhex("001a0000000000c8"))
        self.assertEqual(frame[14:], b"xyz")


    def test_gcm_channels_mirror_the_client(self):
        nonce, key = bytes(range(12)), b"s" * 16
        server_rx, server_tx = bap.channels(nonce, key)
        client_tx = bap.GcmChannel(key, nonce[:11] + bytes([nonce[11] ^ 1]))
        for msg in (bytes.fromhex("007900000001"), b"second"):
            sealed = client_tx.seal(msg)
            self.assertEqual(len(sealed), len(msg) + 16)
            self.assertEqual(server_rx.open(sealed), msg)
        channel = bap.GcmChannel(key, bytes.fromhex("ffff") + bytes(10))
        channel.seal(b"")
        self.assertEqual(bytes(channel.nonce), bytes.fromhex("000001") + bytes(9))  # LE carry
        frame = bap.encrypted_frame(server_tx, bap.QZ_REG_RSP, 1, 200, b"")
        self.assertEqual(frame[:6], bytes.fromhex("010100000018"))


class ServiceSchema(unittest.TestCase):
    def test_update_type_export_uses_saved_service_ids(self):
        target = {"0x80801825": {"fields": []}, "0x80801774": {"fields": []}}
        source = {"services": {"503": ["0x80801774", "0x80801825"]}}
        merged = bap.with_service_map(target, source)
        self.assertEqual(merged["services"], source["services"])
        self.assertIsNone(bap.with_service_map(None, source))
        self.assertIs(bap.with_service_map(merged, {}), merged)
        with self.assertRaisesRegex(ValueError, "absent type"):
            bap.with_service_map({"0x80801825": {"fields": []}}, source)


class CharacterGear(unittest.TestCase):
    def test_gear_spec(self):
        gear = bap.gear_spec("7=0x64/2:5/3:6, 12=200")
        self.assertEqual(gear, {7: {"item": 100, "id": 0x107, "dyes": [(2, 5), (3, 6)]},
                                12: {"item": 200, "id": 0x10c, "dyes": []}})
        with self.assertRaises(ValueError):
            bap.gear_spec("20=1")

    def test_starter_kit_with_override(self):
        gear = bap.gear_spec("starter:titan,6=2008")
        self.assertEqual({slot: g["item"] for slot, g in gear.items()},
                         {**bap.STARTER_GEAR["titan"], 6: 2008})
        self.assertEqual(gear[1]["id"], 0x101)
        with self.assertRaises(ValueError):
            bap.gear_spec("starter:guardian")

    def test_final_starter_kit_uses_final_indexes(self):
        gear = bap.gear_spec("starter:01.24:titan,6=123")
        self.assertEqual({slot: entry["item"] for slot, entry in gear.items()},
                         {0: 710, 1: 204, 2: 198, 3: 200, 4: 206, 5: 202, 11: 862, 6: 123})
        self.assertEqual(bap.gear_spec("starter:titan")[0]["item"], 776)
        self.assertEqual(bap.gear_spec("starter:01.24:hunter")[0]["item"], 709)
        self.assertEqual(bap.gear_spec("starter:01.24:warlock")[0]["item"], 713)

    def test_final_armour_summaries_include_model_arrangements(self):
        expected = {"titan": [191, 188, 189, 192, 190],
                    "hunter": [8, 5, 6, 9, 7],
                    "warlock": [374, 371, 372, 375, 373]}
        layout = {**bap.LAYOUT_0100, "progression_items": 0x1e8,
                  "slot_size": 0x30, "slot_dyes": 0x18}
        for guardian, arrangements in expected.items():
            gear = bap.gear_spec(f"starter:01.24:{guardian}")
            extras = bap.gear_objects(gear, layout)
            for family, off in (("peer", 0x30), ("select", 0x68)):
                slots = extras[family][off][0x1e8]
                self.assertEqual([slots[s * 0x30][0xc] for s in range(1, 6)], arrangements)
                self.assertEqual([slots[s * 0x30][8] for s in range(1, 6)],
                                 [gear[s]["item"] for s in range(1, 6)])
            self.assertNotIn("arrangement", gear[0])  # subclass has no such model
            self.assertEqual(extras["self"][0x1838][8], 0x101)
        self.assertTrue(all("arrangement" not in entry for entry in
                            bap.gear_spec("starter:titan").values()))

    def test_changed_item_does_not_inherit_another_items_model(self):
        gear = bap.gear_spec("starter:01.24:titan,1=123,2=198")
        self.assertNotIn("arrangement", gear[1])
        self.assertEqual(gear[2]["arrangement"], 188)

    def test_custom_arrangement_accepts_zero_and_rejects_invalid_indexes(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "gear.json"
            for value in (0, 123, -1, 32767):
                path.write_text(json.dumps({"1": {"item": 204, "arrangement": value}}))
                gear = bap.gear_spec(f"@{path}")
                self.assertEqual(bap.gear_objects(gear)["peer"][0x30][0x138][0x28][0xc], value)
            for value in (-2, 32768, True, 1.5, "191", None):
                path.write_text(json.dumps({"1": {"item": 204, "arrangement": value}}))
                with self.assertRaisesRegex(ValueError, "arrangement"):
                    bap.gear_spec(f"@{path}")

    def test_arrangement_is_sent_as_its_own_optional_signed_wire_field(self):
        # The relevant fields of 01.24 type 0x8080183c. +8 is the item
        # index, +0xa is a different index, and +0xc selects model art.
        fields = [{"off": off, "kind": kind, "opt": 1, "arr": 0,
                   "nested": 0xffffffff, "min": minimum, "width": width}
                  for off, kind, minimum, width in ((0, 6, 0, 64), (8, 4, 32768, 16),
                                                   (10, 4, 32768, 16), (12, 4, 32768, 16))]
        schema = {"0x8080183c": {"fields": fields, "size": 0x30}}
        gear = bap.gear_spec("starter:01.24:titan")
        slots = bap.gear_objects(gear)["peer"][0x30][0x138]
        helmet = slots[0x28]
        encoded = bap.investment.encode(schema, 0x8080183c, helmet)
        decoded = bap.investment.decode(schema, 0x8080183c, encoded)
        self.assertEqual(decoded, {0: 0x101, 8: 204, 12: 191})
        self.assertNotIn(10, decoded)
        omitted = {key: value for key, value in helmet.items() if key != 12}
        self.assertNotIn(12, bap.investment.decode(
            schema, 0x8080183c, bap.investment.encode(schema, 0x8080183c, omitted)))

    def test_gear_objects_fill_all_three_character_objects(self):
        extras = bap.character_extras({7: {"item": 100, "id": 0x107, "dyes": [(2, 5)]},
                                       12: {"item": 200, "id": 0x10c, "dyes": []}})
        slot7 = {0: 0x107, 8: 100, 0xe: {0: {0: 2, 2: 5}}}
        self.assertEqual(extras["peer"][0x30][0x138][7 * 0x28], slot7)
        self.assertEqual(extras["select"][0x68][0x138][12 * 0x28], {0: 0x10c, 8: 200})
        self.assertEqual(extras["self"][0x30], {0: 2, 8: {0: {0: 100, 8: 0x107, 0x10: 1},
                                                          0x18: {0: 200, 8: 0x10c, 0x10: 1}}})
        self.assertEqual(extras["self"][0x1838], {7 * 8: 0x107, 12 * 8: 0x10c})

    def test_layout_follows_the_schema(self):
        # a 01.24-shaped schema: same handles, grown types (slot 0x30 with dyes at +0x18, items at
        # +0x1e8, equipped ids at +0x2900, inventory entries of 0x20 bytes)
        def struct(off, nested):
            return {"off": off, "kind": 1, "opt": 1, "nested": int(nested, 16)}
        schema = {"0x80801855": {"size": 0x8c8, "fields": [struct(0x10, "0x80805513"), struct(0x1e8, "0x808045d9")]},
                  "0x808017fd": {"size": 0x908, "fields": [struct(0x30, "0x80801855")]},
                  "0x80801954": {"size": 0x940, "fields": [struct(0x68, "0x80801855")]},
                  "0x80801714": {"size": 0x6db8, "fields": [struct(0xc, "0x8080181a"), struct(0x30, "0x80804a18"),
                                                            struct(0x2900, "0x808045da")]},
                  "0x80804a18": {"size": 0x2808, "fields": [{"off": 0, "kind": 5}, struct(8, "0x8080547d")]},
                  "0x8080183c": {"size": 0x30, "fields": [struct(0x18, "0x808046bc")]},
                  "0x808016e0": {"size": 0x20, "fields": []}}
        layout = bap.character_layout(schema)
        self.assertEqual(bap.character_layout(None), bap.LAYOUT_0100)
        self.assertEqual((layout["progression_items"], layout["self_equipped"], layout["slot_size"],
                          layout["slot_dyes"], layout["inventory_entry"]), (0x1e8, 0x2900, 0x30, 0x18, 0x20))
        extras = bap.character_extras({7: {"item": 100, "id": 0x107, "dyes": [(2, 5)]},
                                       12: {"item": 200, "id": 0x10c, "dyes": []}}, layout=layout)
        self.assertEqual(extras["peer"][0x30][0x1e8][7 * 0x30], {0: 0x107, 8: 100, 0x18: {0: {0: 2, 2: 5}}})
        self.assertEqual(extras["self"][0x30][8][0x20], {0: 200, 8: 0x10c, 0x10: 1})
        self.assertEqual(extras["self"][0x2900], {7 * 8: 0x107, 12 * 8: 0x10c})

    def test_overlay_merges_over_gear(self):
        overlay = bap.overlay_spec('{"peer": {"0x30": {"0": 1, "0x138": {"0": {"8": 5}}}}}')
        extras = bap.character_extras({0: {"item": 9, "id": 0x100, "dyes": []}}, overlay)
        self.assertEqual(extras["peer"][0x30], {0: 1, 0x138: {0: {0: 0x100, 8: 5}}})
        self.assertEqual(extras["select"][0x68][0x138][0], {0: 0x100, 8: 9})
        with self.assertRaises(ValueError):
            bap.overlay_spec('{"player": {}}')


class UnlockFlags(unittest.TestCase):
    @staticmethod
    def schema(a_offset, a_handle, a_count, b_offset, b_inner, c_offset, c_handle, c_count):
        def nested(off, handle):
            return {"off": off, "kind": 1, "opt": 1, "nested": handle, "arr": 0, "min": 0, "width": 0}

        def table(count):
            return {"size": count, "fields": [
                {"off": i, "kind": 3, "opt": 1, "nested": -1, "arr": 0, "min": 1, "width": 2}
                for i in range(count)]}

        return {"0x808018c7": {"fields": [nested(a_offset, a_handle), nested(b_offset, 0x80801807)]},
                "0x80801807": {"fields": [nested(b_inner, 0x808047d8)]},
                "0x80801714": {"fields": [nested(c_offset, c_handle)]},
                f"{a_handle:#x}": table(a_count), "0x808047d8": table(256),
                f"{c_handle:#x}": table(c_count)}

    def test_flags_follow_old_and_new_schema_layouts(self):
        cases = ((0x12a8, 0x80804595, 1024, 0x16a8, 1, 0x3bc0, 0x80804595, 1024),
                 (0x383c, 0x80804b0c, 2560, 0x4f78, 8, 0x5000, 0x80804c21, 2048))
        for args in cases:
            schema = self.schema(*args)
            layout = bap.flag_layout(schema)
            self.assertEqual(tuple(layout[t]["count"] for t in ("A", "B", "C")),
                             (args[2], 256, args[7]))
            values = bap.account_flag_values("2", {t: layout[t]["count"] for t in layout})
            self.assertEqual(tuple(len(values[t]) for t in ("A", "B", "C")), (args[2], 256, args[7]))
            root = bap.account_flags(schema, "2", layout, values)
            self.assertEqual(root[args[0]][args[2] - 1], 2)
            self.assertEqual(root[args[3]][args[4]][255], 2)
            self.assertEqual(values["C"][args[7] - 1], 2)
            self.assertNotIn(0, root[args[3]])  # old bool defaults false; new u64 is optional

    def test_explicit_ranges_use_the_selected_build_capacity(self):
        counts = {"A": 2560, "B": 256, "C": 2048}
        values = bap.account_flag_values("1;2@A2559,B0,C2047", counts)
        self.assertEqual(values["A"][2559], 2)
        self.assertEqual(values["B"][0], 2)
        self.assertEqual(values["C"][2047], 2)
        with self.assertRaisesRegex(ValueError, "outside"):
            bap.account_flag_values("2@C2048", counts)


class MissionScript(unittest.TestCase):
    F = lambda off, kind, width, opt=0, minimum=0: {"off": off, "kind": kind, "opt": opt, "nested": -1,
                                                    "arr": 0, "min": minimum, "width": width, "extra": 0}
    SCHEMA = {"0x808004b1": {"size": 8, "fields": [F(0, 9, 32), F(4, 3, 6, 0, 1), F(6, 4, 16, 0, 0x8000)]},
              "0x80800463": {"size": 8, "fields": [F(0, 2, 0), F(4, 5, 32, 0, 0x80000000)]},
              "0x8080071e": {"size": 0x60, "fields": [F(0x44, 5, 31), F(0x5c, 3, 2, minimum=1)]}}
    MISSION = {"objects": {"trig": "0x992bb471:0x1d:30:0x80800463", "other": "0x992bb471:0x1d:31:0x80800463"},
               "steps": [{"at": 5, "set": {"trig": {"0": True, "4": "$now"}}},
                         {"on": {"type": 6, "object": "trig"}, "delay": 1, "set": {"other": {"4": "$player"}}},
                         {"on": {"type": 21, "regex": "^b8"}, "set": {"other": {"0": True}}}]}

    def test_expand_and_resolve(self):
        timed, reactive = bap.expand_mission(self.MISSION, self.SCHEMA)
        self.assertEqual(len(timed), 1)
        at, spec = timed[0]
        self.assertEqual((at, spec[0], spec[1]), (5.0, {0: 0x992bb471, 4: 0x1d, 6: 30}, 0x80800463))
        self.assertEqual(bap.resolve_spec(spec, {"now": 90, "player": 7})[2], {0: True, 4: 90})
        (t1, m1, d1, s1), (t2, m2, d2, s2) = reactive
        self.assertEqual((t1, d1, s1[0][6]), (6, 1.0, 31))
        self.assertEqual(bap.resolve_spec(s1, {"now": 0, "player": 7})[2], {4: 7})
        self.assertTrue(m2.search("b80000") and not m2.search("00b8"))
        self.assertEqual(bap.resolve_values(["$now+30", "$now-5"], {"now": 100}), [130, 95])
        any_rule = bap.expand_mission({"objects": self.MISSION["objects"], "steps": [
            {"on": {"type": "any", "object": "trig"}, "set": {"other": {"0": True}}}]}, self.SCHEMA)[1][0]
        self.assertEqual(any_rule[0], bap.ANY_MESSAGE)
        with self.assertRaises(ValueError):
            bap.resolve_values("$later", {"now": 0})
        with self.assertRaises(ValueError):
            bap.expand_mission({"objects": {}, "steps": [{"at": 1, "set": {"nope": {}}}]}, self.SCHEMA)

    def test_object_matcher_finds_the_id_at_any_bit(self):
        matcher = bap.ObjectIdMatcher(self.SCHEMA, 0x992bb471, 0x1d, 30)
        for lead in (0, 3, 11):
            w = bap.investment.BitWriter()
            w.put(1, lead + 1)
            bap.investment.encode_into(w, self.SCHEMA, bap.OBJECT_ID_TYPE, {0: 0x992bb471, 4: 0x1d, 6: 30})
            w.put(0, 9)
            self.assertTrue(matcher.search(w.data().hex()))
        w = bap.investment.BitWriter()
        bap.investment.encode_into(w, self.SCHEMA, bap.OBJECT_ID_TYPE, {0: 0x992bb471, 4: 0x1d, 6: 31})
        self.assertFalse(matcher.search(w.data().hex()))

    def test_shipped_scripts_expand(self):
        folder = Path(__file__).resolve().parents[1] / "missions"
        timed, _ = bap.expand_mission(json.loads((folder / "tower_npcs.json").read_text(encoding="utf-8")),
                                      self.SCHEMA)
        self.assertEqual({spec[0][4] for _, spec in timed}, {1})
        mission = json.loads((folder / "a_guardian_rises.json").read_text(encoding="utf-8"))
        timed, reactive = bap.expand_mission(mission, self.SCHEMA)
        self.assertTrue(timed and reactive)
        self.assertTrue(all(isinstance(m, (bap.ObjectIdMatcher, bap.SliceSetMatcher)) for _, m, _, _ in reactive))
        self.assertTrue(any(isinstance(m, bap.SliceSetMatcher) and m.index == 24 for _, m, _, _ in reactive))

    def test_squad_field_resolves_for_both_build_layouts(self):
        mission = {"objects": {"squad": "0x992bb471:1:3:0x8080071e"},
                   "steps": [{"at": 1, "set": {"squad": {"0x44": 1, "squad_existing_members": -1}}}]}
        for size, offset in ((0x60, 0x5c), (0x7c, 0x74)):
            schema = dict(self.SCHEMA)
            schema["0x8080071e"] = {"size": size, "fields": [MissionScript.F(0x44, 5, 31),
                                                                 MissionScript.F(offset, 3, 2, minimum=1)]}
            timed, reactive = bap.expand_mission(mission, schema)
            self.assertEqual(timed[0][1][2], {0x44: 1, offset: -1})
            self.assertEqual(bap.validate_mission_states(schema, timed, reactive), 1)

    def test_preflight_rejects_a_dropped_field(self):
        schema = dict(self.SCHEMA)
        schema["0x8080071e"] = {"size": 0x7c, "fields": [MissionScript.F(0x44, 5, 31),
                                                           MissionScript.F(0x74, 3, 2, minimum=1)]}
        mission = {"objects": {"squad": "0x992bb471:1:3:0x8080071e"},
                   "steps": [{"at": 1, "set": {"squad": {"0x5c": -1}}}]}
        timed, reactive = bap.expand_mission(mission, schema)
        with self.assertRaisesRegex(ValueError, "0x5c: field is absent"):
            bap.validate_mission_states(schema, timed, reactive)


if __name__ == "__main__":
    unittest.main()


class ClientConfigReply(unittest.TestCase):
    def test_fields_are_protobuf_varints(self):
        self.assertEqual(bap.client_config_reply(), bytes.fromhex("08011001"))
        self.assertEqual(bap.client_config_reply(0, 1), bytes.fromhex("08001001"))
        self.assertEqual(bap.client_config_reply(300, 2), bytes.fromhex("08ac021002"))


class ReactiveObjects(unittest.TestCase):
    def test_rule_parsing(self):
        on_type, on_body, delay, spec = bap.reactive_object_spec(
            '15/^41=>2.5@0xecf574b5:3:0:0x808004fa={"0": {"0": 2}}')
        self.assertEqual((on_type, delay), (15, 2.5))
        self.assertTrue(on_body.search("4100") and not on_body.search("0041"))
        self.assertEqual(spec[0], {0: 0xecf574b5, 4: 3, 6: 0})
        self.assertEqual(spec[1], 0x808004fa)
        self.assertEqual(spec[2], {0: {0: 2}})
        self.assertIsNone(bap.reactive_object_spec("0x14=>0@1:2:3:0x80800001")[1])
        with self.assertRaises(ValueError):
            bap.reactive_object_spec("15@1:2:3:0x80800001")

    def test_replace_object_keeps_one_state_per_object(self):
        objects = [bap.default_baseline(1, 3, 0), bap.default_baseline(1, 4, 0)]
        spec = bap.object_spec('1:3:0:0x808004fa={"0": {"0": 1}}')
        self.assertEqual(bap.replace_object(objects, spec), (1, 3, 0))
        self.assertEqual(len(objects), 2)
        self.assertEqual(objects[-1], spec)


class StartRequest(unittest.TestCase):
    """01.24 opens the BAP session with start req (0x1e, plaintext); FINAL124_ALL_CHAINS."""

    def test_start_req_gets_a_plain_status_200_start_rsp(self):
        import socket
        import tempfile
        import threading
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        logs = []
        self.addCleanup(setattr, bap, "log", bap.log)
        bap.log = logs.append
        args = bap.parse_args(["--bind", "127.0.0.1", "--capture-dir", tmp.name,
                               "--schema", str(Path(tmp.name) / "none.json")])
        server, client = socket.socketpair()
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(client.close)
        payload = b"DESTINY\x01" + bytes(range(120))
        inner = struct.pack(">HI", bap.START_REQ, 0) + payload
        client.sendall(struct.pack(">BBI", 1, 2, len(inner)) + inner)
        client.settimeout(2)
        expected = bap.plain_frame(bap.START_RSP, 0, 200, payload)  # echo: the decoder takes exactly 0x80 bytes
        reply = b""
        while len(reply) < len(expected):
            chunk = client.recv(len(expected) - len(reply))
            if not chunk:
                break
            reply += chunk
        self.assertEqual(reply, expected)
        self.assertTrue(any("start req magic" in line and "DESTINY" in line for line in logs), logs)


class ProfileRequests(unittest.TestCase):
    def test_profile_replies_preserve_ids_order_and_encrypted_channel(self):
        import socket
        import tempfile
        import threading

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        logs = []
        self.addCleanup(setattr, bap, "log", bap.log)
        bap.log = logs.append
        args = bap.parse_args(["--capture-dir", tmp.name,
                               "--schema", str(Path(tmp.name) / "none.json")])
        server, client = socket.socketpair()
        client.settimeout(2)
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(client.close)

        def receive():
            version, kind, length = struct.unpack(">BBI", bap.exact(client, 6))
            self.assertEqual(version, 1)
            return kind, bap.exact(client, length)

        hello = struct.pack(">HI", bap.SHELLO_REQ, 0)
        client.sendall(struct.pack(">BBI", 1, 2, len(hello)) + hello)
        kind, reply = receive()
        self.assertEqual(kind, 2)
        plaintext = bap.unseal(args.key, args.mac_key, reply[8:])
        nonce, key = plaintext[:12], plaintext[12:]
        tx = bap.GcmChannel(key, nonce[:11] + bytes([nonce[11] ^ 1]))
        rx = bap.GcmChannel(key, nonce)
        requests = [(bap.USER_MSG_REQ, 17, b""), (bap.UPO_REQ, 18, b"\x08\x01"),
                    (bap.ECHO_REQ, 19, b"")]
        frames = []
        for msg_type, msg_id, body in requests:
            encrypted = tx.seal(struct.pack(">HI", msg_type, msg_id) + body)
            frames.append(struct.pack(">BBI", 1, 1, len(encrypted)) + encrypted)
        client.sendall(b"".join(frames))
        for msg_type, msg_id, body in [(bap.USER_MSG_RSP, 17, b"\x20\x00"),
                                       (bap.UPO_RSP, 18, b""), (bap.ECHO_RSP, 19, b"")]:
            kind, reply = receive()
            self.assertEqual(kind, 1)
            self.assertEqual(rx.open(reply), struct.pack(">HIH", msg_type, msg_id, 200) + body)


class ActiveCharacter(unittest.TestCase):
    def test_self_family_keeps_full_soid_for_both_account_layouts(self):
        def field(off, kind, width=0, nested=-1, arr=0):
            return {"off": off, "kind": kind, "width": width, "nested": nested,
                    "arr": arr, "opt": 1, "min": 0}

        character = 0x123456789ABCDEF0
        for active_offset in (0x60, 0x68):
            with self.subTest(active_offset=active_offset):
                account = [field(0, 6, 64), field(8, 1, nested=0x80804621)]
                if active_offset == 0x68:
                    account.append(field(0x60, 9, 32))
                account.append(field(active_offset, 6, 64))
                schema = {
                    "0x80801803": {"fields": [field(0, 1, nested=0x808018C7)]},
                    "0x808018c7": {"fields": account},
                    "0x80804621": {"fields": [field(0, 5, 32),
                                              field(8, 1, nested=0x80804620, arr=1)]},
                    "0x80804620": {"fields": [field(0, 6, 64)]},
                }
                payload = bap.family_blob(schema, 4, 7, character_id=character,
                                          active_id=character)
                # One family, one root object. Decode its real wire payload:
                # the new +0x60 u32 must not truncate the active SOID.
                self.assertEqual(struct.unpack_from(">I", payload, 21)[0], 1)
                length = struct.unpack_from(">I", payload, 37)[0]
                decoded = bap.investment.decode(schema, 0x80801803, payload[41:41 + length])[0]
                self.assertEqual(decoded[active_offset], character)
                self.assertEqual(decoded[8][8], [character])
                if active_offset == 0x68:
                    self.assertNotIn(0x60, decoded)

    def test_unknown_account_layout_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "active-character layout"):
            bap.active_character_offset({"0x808018c7": {"fields": [{"kind": 9}]}})


class ObjectHashes(unittest.TestCase):
    """qz object type hashes follow the schema's build (type size); 01.24 grew four types."""

    def test_hash_follows_the_type_size_and_falls_back_to_0100(self):
        old = {"0x80801803": {"size": 8184}, "0x80801743": {"size": 21936}}
        new = {"0x80801803": {"size": 28752}, "0x80801743": {"size": 28088}, "0x80801951": {"size": 160}}
        self.assertEqual(bap.object_hash(old, 0x88190FE1, 0x80801803), 0x88190FE1)
        self.assertEqual(bap.object_hash(new, 0x88190FE1, 0x80801803), 0xB5DF2907)
        self.assertEqual(bap.object_hash(new, 0x242B2F61, 0x80801743), 0xEB4B8439)
        self.assertEqual(bap.object_hash(new, 0xA2AC2C74, 0x80801951), 0xA2AC2C74)   # unchanged type
        self.assertEqual(bap.object_hash(None, 0x88190FE1, 0x80801803), 0x88190FE1)
        self.assertEqual(bap.object_hash({"0x80801803": {"size": 1}}, 0x88190FE1, 0x80801803), 0x88190FE1)


class UniverseService(unittest.TestCase):
    """01.00 replies to 104 with the universe object; 01.24's 104 is a bool and 205 has it."""

    @staticmethod
    def schema(universe_wire):
        other = "205" if universe_wire == "104" else "104"
        return {"services": {universe_wire: ["0x1", "0x10"], other: ["0x2", "0x20"]},
                "0x10": {"size": 952, "fields": [{"off": 0, "kind": 3}, {"off": 8, "kind": 1,
                                                                          "nested": bap.UNIVERSE_TYPE}]},
                "0x20": {"size": 12, "fields": [{"off": 0, "kind": 1}, {"off": 8, "kind": 2}]}}

    def args(self, schema):
        return type("Args", (), {"schema": schema, "universe_time": "s", "universe_flags": [],
                                 "universe_values": []})()

    def test_0100_layout_answers_104(self):
        args = self.args(self.schema("104"))
        self.assertEqual(bap.universe_service(args.schema), 104)
        self.assertEqual(bap.ws_reply_values(args, 104)[8][0], bap.UNIVERSE_SOID)
        self.assertIsNone(bap.ws_reply_values(args, 205))

    def test_0124_layout_answers_205_and_leaves_the_104_bool_unset(self):
        args = self.args(self.schema("205"))
        self.assertEqual(bap.universe_service(args.schema), 205)
        self.assertEqual(bap.ws_reply_values(args, 205)[8][0], bap.UNIVERSE_SOID)
        self.assertIsNone(bap.ws_reply_values(args, 104))
        self.assertEqual(bap.universe_service(None), 104)


class AccountUserData(unittest.TestCase):
    """Service 701 uploads the account user data (settings) that the self account carries."""

    schema = {"services": {"701": ["0x1", "0x2"]},
              "0x1": {"size": 1088, "fields": [{"off": 0, "kind": 1, "nested": 0x80801716}]},
              "0x808018c7": {"size": 28752, "fields": [
                  {"off": 8, "kind": 1, "nested": 0x80804621},
                  {"off": 0x80, "kind": 1, "nested": 0x80801716}]}}

    def test_offset_follows_the_701_request_type(self):
        self.assertEqual(bap.account_data_offset(self.schema), 0x80)
        self.assertIsNone(bap.account_data_offset({"services": {}}))
        self.assertIsNone(bap.account_data_offset(None))

    def test_store_keeps_character_and_account_data_side_by_side(self):
        import tempfile
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "store.json"
            bap.save_account_data(path, 0x10, b"\x01\x02")
            bap.save_character(path, 0x10, 0x11, b"\xaa")
            bap.save_account_data(path, 0x10, b"\x03")
            self.assertEqual(bap.load_store(path)["0x10"], {"id": 0x11, "request": "aa", "account_data": "03"})

    def test_mm_req_gets_an_empty_mm_rsp(self):
        # 01.24 mm rsp (descriptor 0x01306900) has only optional fields
        self.assertEqual(bap.EMPTY_REPLIES[bap.MM_REQ], bap.MM_RSP)

    def test_mm_rsp_fields(self):
        self.assertEqual(bap.mm_rsp_body("1=0,4="), bytes.fromhex("0800") + bytes.fromhex("2200"))
        self.assertEqual(bap.mm_rsp_body("4=0801"), bytes.fromhex("22020801"))
        with self.assertRaises(ValueError):
            bap.mm_rsp_body("7=x")


class GroupSwitch(unittest.TestCase):
    """Group record 1 carries a slice-set switch in progress (--ah-group-switch)."""

    def test_target_goes_to_group_record_one(self):
        path = Path(__file__).resolve().parents[2] / "d1xenia-data" / "ps3_updates" / "types_01.24.json"
        if not path.exists():
            self.skipTest("01.24 schema not available")
        schema = json.loads(path.read_text(encoding="utf-8"))

        def groups(data):
            reader = bap.investment.BitReader(data)
            reader.get(1)
            return bap.investment.decode_from(reader, schema, bap.WORLD_STATE_TYPE)[0x2E48][0]

        switching = groups(bap.world_state_delta(schema, {0x10: {}, 0x38: {}}, 113, 1, 0, target=112))
        self.assertEqual((switching[0][0], switching[0xD8][0], switching[0xD8][8][1], switching[0xD8][8][2]),
                         (113, 112, 1, 0))
        settled = groups(bap.world_state_delta(schema, {0x10: {}, 0x38: {}}, 112, 1, 0))
        self.assertEqual((settled[0][0], settled[0xD8][0]), (112, -1))


class ClientSliceRecord(unittest.TestCase):
    def test_peer_byte_is_reported(self):
        # JOIN38: target 112 with the client's i8 @0x140 (engine +0x1d0) = 2
        path = Path(__file__).resolve().parents[2] / "d1xenia-data" / "ps3_updates" / "types_01.24.json"
        if not path.exists():
            self.skipTest("01.24 schema not available")
        schema = json.loads(path.read_text(encoding="utf-8"))
        writer = bap.investment.BitWriter()
        writer.put(1, 1)
        bap.investment.encode_into(writer, schema, bap.CLIENT_STATE_TYPE,
                                   {0xB8: {0xA0: {0: 112, 4: 0x811C9DC5}, 0x140: 2, 0x142: 2}})
        record = bap.client_slice_record(schema, writer.data())
        self.assertEqual((record["target"], record["peer_byte"], record["state"]), ((112, 0x811C9DC5), 2, 2))
        self.assertIn("peer byte 2", bap.client_slice_state(schema, writer.data()))


class MissionTeleports(unittest.TestCase):
    def test_teleport_steps(self):
        mission = {"objects": {}, "steps": [{"at": 175, "teleport": 112}, {"at": 20, "set": {}},
                                            {"at": 400, "teleport": "0x18"}]}
        self.assertEqual(bap.mission_teleports(mission), [((112, None), 175.0), ((24, None), 400.0)])
        self.assertEqual(bap.parse_slice_set("24:0x68935472"), (24, 0x68935472))
        timed, reactive = bap.expand_mission(mission, {})
        self.assertEqual((timed, reactive), ([], []))

    def test_control_steps(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "control.jsonl"
            path.write_text('{"set": {"obj": {"0": 1}}}\n# note\n{"bad": 1}\n{"set": {"obj": {"0": 3}}}',
                            encoding="utf-8")
            objects = {"obj": "0x992bb471:0x20:3:0x80802a1b"}
            specs, teleports, offset, errors = bap.control_steps(path, 0, objects, {})
            self.assertEqual(len(specs), 1)            # the last line is not complete yet
            self.assertEqual(len(errors), 1)           # {"bad": 1}
            with path.open("a", encoding="utf-8") as f:
                f.write("\n")
            more, _, offset2, _ = bap.control_steps(path, offset, objects, {})
            self.assertEqual(len(more), 1)
            self.assertEqual(bap.control_steps(path, offset2, objects, {})[0], [])
            with path.open("a", encoding="utf-8") as f:
                f.write('{"teleport": "0x18"}\n')
            self.assertEqual(bap.control_steps(path, offset2, objects, {})[1], [(24, None)])

    def test_mission_clock(self):
        self.assertEqual(bap.mission_clock({}), "world")
        self.assertEqual(bap.mission_clock({"clock": "ready"}), "ready")
        with self.assertRaises(ValueError):
            bap.mission_clock({"clock": "arrival"})
        # type 17 {counter, status}: loading vs in world (JOIN53)
        self.assertFalse(bap.client_in_world(bytes.fromhex("000000007fffffff")))
        self.assertTrue(bap.client_in_world(bytes.fromhex("000000018000000e")))
        self.assertFalse(bap.client_in_world(bytes(2)))


class TeleportCompletion(unittest.TestCase):
    """A completed host teleport settles, expires, and releases the next queued request."""

    def complete_and_queue_next(self, *, delay=0, follow=False, repeat_report=False):
        import socket
        import tempfile
        import threading
        import time
        from unittest.mock import patch
        from test_ah_session import FakeClient, PEER, schema

        # Exercise the real encrypted session and scheduling loop. World/client-state codecs
        # are replaced here so this protocol regression needs no private exported schema.
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        activity_schema = schema()
        path = Path(tmp.name) / "schema.json"
        path.write_text(json.dumps(activity_schema), encoding="utf-8")
        args = bap.parse_args(["--capture-dir", tmp.name, "--schema", str(Path(tmp.name) / "none.json"),
                               "--activity-schema", str(path), "--ah-join", "--ah-state",
                               "--ah-world-state", "113" if follow else "112",
                               "--ah-heartbeat", "0.01:world",
                               "--ah-teleport-finish-delay", str(delay),
                               *(["--ah-follow-client-zone"] if follow else [])])
        args.ah_teleports = [((112, 0x811C9DC5), 0), ((24, 0x68935472), 0)]
        settle = 0.12

        def world_data(_schema, _request, zone, *unused, **fields):
            return json.dumps({"zone": zone, "sent_at": time.monotonic(), **fields}).encode()

        for mock in (patch.object(bap, "world_state_delta", side_effect=world_data),
                     patch.object(bap, "activity_state", return_value=b"activity"),
                     patch.object(bap, "client_slice_record", side_effect=lambda _s, body: json.loads(body)),
                     patch.object(bap, "client_slice_state", return_value="test report"),
                     patch.object(bap, "TELEPORT_SETTLE_SECONDS", settle),
                     patch.object(bap, "log")):
            mock.start()
            self.addCleanup(mock.stop)
        server, client = socket.socketpair()
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(client.close)
        c = FakeClient(client)
        c.notify(bap.AH_JOIN_REQUEST, bap.investment.encode(activity_schema, bap.JOIN_REQUEST_TYPE,
                                                          {0: 7, 8: 9, 0x10: {0: PEER}}))
        report = ({"target": [112, 0x811C9DC5], "state": 4} if delay
                  else {"current": [112, 0x811C9DC5], "state": 0})
        reported, worlds = False, []
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            c.notify(bap.AH_CLIENT_STATE if reported and repeat_report else 15,
                     json.dumps(report).encode() if reported and repeat_report else b"\0")
            for arrival, subtype, data in c.pushes(0.005):
                if subtype != bap.AH_ESTABLISH:
                    continue
                world = json.loads(data)
                worlds.append((arrival, world))
                if world.get("teleport") == [1, 112, 0x811C9DC5] and not reported:
                    c.notify(bap.AH_CLIENT_STATE, json.dumps(report).encode())
                    reported = True
            if any(w.get("teleport") == [1, 24, 0x68935472] for _, w in worlds):
                break

        finished = [(t, w) for t, w in worlds if w.get("teleport") == [3, 112, 0x811C9DC5]]
        dropped = [(t, w) for t, w in worlds if finished and t > finished[0][0]
                   and w.get("teleport") is None]
        transitions = []
        for _, world in worlds:
            value = {k: v for k, v in world.items() if k != "sent_at"}
            if not transitions or value != transitions[-1]:
                transitions.append(value)
        self.assertTrue(finished, transitions)
        self.assertTrue(dropped, transitions)
        self.assertGreaterEqual(dropped[0][1]["sent_at"] - finished[0][1]["sent_at"], settle - 0.02)
        self.assertTrue(any(w.get("teleport") == [1, 24, 0x68935472] for _, w in worlds), transitions)
        # State 3 reuses the request version; removing it publishes a new version.
        started = next(w for _, w in worlds if w.get("teleport") == [1, 112, 0x811C9DC5])
        self.assertEqual(finished[0][1]["version"], started["version"])
        self.assertGreater(dropped[0][1]["version"], finished[0][1]["version"])

    def test_already_current_teleport_expires_despite_repeated_arrival_reports(self):
        self.complete_and_queue_next(repeat_report=True)

    def test_follow_mode_teleport_expires(self):
        self.complete_and_queue_next(follow=True)

    def test_timer_finished_teleport_expires_without_further_slice_reports(self):
        self.complete_and_queue_next(delay=0.06)
