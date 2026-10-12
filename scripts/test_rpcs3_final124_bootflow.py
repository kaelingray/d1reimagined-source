import struct
import unittest
from unittest.mock import patch

import rpcs3_final124_bootflow as reader


class BootflowTest(unittest.TestCase):
    def test_engine_and_double_buffered_controller_states(self):
        values = {(0x1ABC8AC, 4): struct.pack(">I", 0x2000),
                  (0x2089, 1): bytes([13]),
                  (0x1ABCA14, 4): struct.pack(">I", 0x3000),
                  (0x3000, 1): bytes([1])}
        for index, selector in ((1, 0), (2, 1)):
            record = 0x3000 + index * 0x12208
            values[record + 8, 4] = struct.pack(">I", selector)
            values[record + selector * 0x9100 + 0x10 + 0x8FEC, 4] = struct.pack(">I", index + 2)
        result = reader.world_snapshot(lambda address, size: values[address, size])
        self.assertEqual(result["engine_state_89"], 13)
        self.assertEqual([c["status_8fec"] for c in result["controllers"]], [3, 4])
        values[0x3000 + 0x12208 + 8, 4] = struct.pack(">I", 2)
        with self.assertRaisesRegex(ValueError, "selector"):
            reader.world_snapshot(lambda address, size: values[address, size])

    def fixture(self, current=19, stage=12):
        fields = struct.pack(">7I", current, 0, 0, 0, current, 0, 0)
        values = {
            (reader.BOOTFLOW_PTR, 4): struct.pack(">I", 0x30000000),
            (0x30000118, 0x1C): fields,
            (0x30000080 + current * 4, 4): struct.pack(">I", 0x01ABDCA0),
            (0x01ABDCD0, 0x10): struct.pack(">4I", stage, 0, 0, 0),
            (0x01ABDCA0, 4): struct.pack(">I", 0x013E8D58),
            (0x01ABDCDC, 8): struct.pack(">II", 5, 5),
            (reader.RESOURCE_STATES, 96): b"".join(struct.pack(">III", 1, 0x01878002, 0)
                                                 for _ in range(8)),
            (reader.RESOURCE_STATES + 0x60, 4): struct.pack(">I", 1),
            (reader.RESOURCE_NAMES, 32): struct.pack(">8I", *([0] * 8)),
            (reader.BOOTFLOW_NAMES + current * 4, 4): struct.pack(">I", 0x1300000),
            (0x1300000, 96): b"bootflow:bap_signin\0".ljust(96, b"\0"),
            (reader.AUXILIARY_PTR, 4): bytes(4),
            (reader.READINESS_TABLE, 0xEC): bytes(0xEC),
        }
        return lambda addr, size: values[(addr, size)]

    def test_stage_fields_big_endian_and_resource_pending(self):
        result = reader.sample(self.fixture())
        self.assertEqual(result["current"], 19)
        self.assertEqual(result["stage_state_30"], 12)
        self.assertEqual(result["resources"][3]["handle"], "0x01878002")
        self.assertEqual(result["resources"][3]["state"], 1)
        self.assertTrue(result["stage_unchanged_during_sample"])
        self.assertFalse(result["atomic"])

    def test_uninitialized_and_out_of_range_do_not_index_stage_table(self):
        with self.assertRaisesRegex(ValueError, "not initialized"):
            reader.sample(lambda addr, size: bytes(size))
        with self.assertRaisesRegex(ValueError, "unexpected"):
            reader.sample(self.fixture(current=38))

    def test_other_stage_layout_is_preserved_without_calling_a_pointer_a_substate(self):
        result = reader.sample(self.fixture(current=21, stage=0x013E8B74))
        self.assertEqual(result["stage_word_30"], 0x013E8B74)
        self.assertTrue(result["stage_fields_30_3f"].startswith("013e8b74"))
        self.assertIsNone(result["stage_state_30"])
        self.assertIsNone(result["stage_state_34"])

    def test_reports_stage_changed_during_read(self):
        fixture = self.fixture()
        count = 0

        def read(addr, size):
            nonlocal count
            if addr == 0x30000118:
                count += 1
                if count > 1:
                    return struct.pack(">7I", 20, 0, 0, 0, 20, 0, 0)
            return fixture(addr, size)

        self.assertFalse(reader.sample(read)["stage_unchanged_during_sample"])

    def test_character_signin_layout_is_vtable_checked_and_rechecked(self):
        fixture = self.fixture(current=23)
        result = reader.sample(fixture)
        self.assertEqual(result["character_signin"], {
            "vtable": "0x013e8d58", "current": 5, "target": 5,
            "unchanged_during_sample": True})
        reads = 0

        def changing(address, size):
            nonlocal reads
            if address == 0x01ABDCDC:
                reads += 1
                if reads > 1:
                    return struct.pack(">II", 5, 7)
            return fixture(address, size)

        self.assertFalse(reader.sample(changing)["character_signin"]["unchanged_during_sample"])

        def wrong_layout(address, size):
            if address == 0x01ABDCA0:
                return struct.pack(">I", 0x013E8B74)
            if address == 0x01ABDCDC:
                self.fail("must not read an unverified stage layout")
            return fixture(address, size)

        self.assertNotIn("character_signin", reader.sample(wrong_layout))
        self.assertNotIn("character_signin", reader.sample(self.fixture(current=24)))

    def test_memory_rejects_short_reads_and_daemon_errors(self):
        for reply in (b"0000\n.\n", b"error: TimeoutError()\n.\n", b""):
            with self.subTest(reply=reply), patch.object(reader.socket, "create_connection") as connect:
                conn = connect.return_value.__enter__.return_value
                conn.recv.return_value = reply
                with self.assertRaises((ValueError, EOFError)):
                    reader.memory(0x1234, 4)

    def test_fragmented_control_reply(self):
        with patch.object(reader.socket, "create_connection") as connect:
            conn = connect.return_value.__enter__.return_value
            conn.recv.side_effect = [b"0000", b"0013\n.", b"\n"]
            self.assertEqual(reader.memory(0x1234, 4), struct.pack(">I", 19))
            conn.sendall.assert_called_once_with(b"mem 1234 4\n")

    def test_resolves_handle_with_alignment_and_pending_parent(self):
        descriptor = bytearray(0x38)
        struct.pack_into(">I", descriptor, 0x10, 0x30000000)
        struct.pack_into(">II", descriptor, 0x30, 0x100, 0xF)
        pending = bytearray(0x44)
        pending[6:8] = bytes((2, 1))
        struct.pack_into(">I", pending, 0xC, 0x01878003)
        finished = bytearray(0x44)
        finished[6:8] = bytes((1, 2))
        values = {
            (reader.REQUEST_TABLE_PTR, 4): struct.pack(">I", 0x20000000),
            (0x20000000, 4): struct.pack(">I", 0x21000000),
            (0x21000000 + 0x3C * 0x40, 0x38): bytes(descriptor),
            (0x30000208, 4): struct.pack(">I", 4),
            (0x300001FC, 0x44): bytes(pending),
            (0x30000308, 4): struct.pack(">I", 0),
            (0x30000300, 0x44): bytes(finished),
        }
        chain = reader.request_chain(0x01878002, lambda addr, size: values[(addr, size)])
        self.assertEqual([r["status_byte_7"] for r in chain], [1, 2])
        self.assertEqual(chain[0]["address"], "0x300001fc")
        self.assertEqual(chain[1]["address"], "0x30000300")
        self.assertEqual(reader.request_chain(0xFFFFFFFF, lambda *_: self.fail("invalid read")), [])


class SchedulerTest(unittest.TestCase):
    def fixture(self, token=1, manager=0x02000000, offset=0x20, stride=0x40, capacity=2):
        table = bytearray(0x20)
        struct.pack_into(">I", table, 0, offset)
        struct.pack_into(">I", table, 8, stride)
        struct.pack_into(">H", table, 0xC, capacity)
        record = bytearray(0x40)
        struct.pack_into(">i", record, 0, -72)
        values = {
            (reader.RESOURCE_MANAGER_PTR, 4): struct.pack(">I", manager),
            (reader.RESOURCE_SCHEDULER, 12): struct.pack(">H2xII", token, 2, 15),
            (0x0201405C, 0x1C): struct.pack(">7I", 0x01FA0002, 0, 1,
                                                     0x01FA0003, 0xFFFFFFFF,
                                                     0x01FA0002, 4),
            (reader.BATCH_REQUEST_TABLE, 0x20): bytes(table),
            (reader.BATCH_REQUEST_TABLE + 0x60, 0x40): bytes(record),
        }
        reads = []

        def read(address, size):
            reads.append((address, size))
            return values[(address, size)]

        return read, reads

    def test_resolves_active_token_and_distinguishes_inline_retry_slots(self):
        read, reads = self.fixture()
        result = reader.scheduler_snapshot(read)
        self.assertEqual(result["active_token"], "0x0001")
        self.assertEqual(result["job_state_4"], 2)
        self.assertEqual(result["deferred_event_8"], 15)
        fields = result["manager_fields"]
        self.assertEqual(fields["queue_head"], "0x01fa0002")
        self.assertEqual(fields["retry_count"], 1)
        self.assertEqual(fields["retry_handle_slots"], ["0x01fa0003", "0xffffffff"])
        self.assertEqual(fields["active_handle"], "0x01fa0002")
        record = result["active_batch_record"]
        self.assertEqual(record["address"], f"0x{reader.BATCH_REQUEST_TABLE + 0x60:08x}")
        self.assertEqual(record["status_word_0"], -72)
        self.assertEqual(record["capacity"], 2)
        self.assertIn((reader.BATCH_REQUEST_TABLE + 0x60, 0x40), reads)

    def test_inactive_token_and_null_manager_do_not_dereference(self):
        read, reads = self.fixture(token=0xFFFF, manager=0)
        result = reader.scheduler_snapshot(read)
        self.assertIsNone(result["manager_fields"])
        self.assertIsNone(result["active_batch_record"])
        self.assertEqual(reads, [(reader.RESOURCE_MANAGER_PTR, 4),
                                 (reader.RESOURCE_SCHEDULER, 12)])

    def test_invalid_table_never_reads_an_active_record(self):
        for changes in ({"token": 2}, {"stride": 0}, {"stride": 0x41},
                        {"offset": 0}, {"capacity": 0}, {"offset": 0xFFFFFFFF}):
            with self.subTest(changes=changes):
                read, reads = self.fixture(**changes)
                with self.assertRaisesRegex(ValueError, "invalid scheduler"):
                    reader.scheduler_snapshot(read)
                self.assertFalse(any(size == 0x40 for _, size in reads))

    def test_overflowing_manager_and_short_reads_are_rejected(self):
        read, reads = self.fixture(manager=0xFFFF0000)
        with self.assertRaisesRegex(ValueError, "invalid scheduler memory range"):
            reader.scheduler_snapshot(read)
        self.assertEqual(len(reads), 2)
        with self.assertRaisesRegex(ValueError, "short scheduler read"):
            reader.scheduler_snapshot(lambda address, size: bytes(size - 1))

    def test_sample_collects_scheduler_only_when_requested(self):
        fixture = BootflowTest().fixture()
        with patch.object(reader, "request_chain", return_value=[{"kind_byte_6": 3}]) as chain, \
                patch.object(reader, "scheduler_snapshot", return_value={"active_token": "0xffff"}) as scheduler:
            plain = reader.sample(fixture)
            self.assertNotIn("resource_scheduler", plain)
            chain.assert_not_called()
            scheduler.assert_not_called()
            detailed = reader.sample(fixture, include_request=True)
            self.assertEqual(detailed["resource_scheduler"]["active_token"], "0xffff")
            chain.assert_called_once_with(0x01878002, fixture)
            scheduler.assert_called_once_with(fixture)


class ReadinessTest(unittest.TestCase):
    def fixture(self, mode=2, readiness_mask=0, initialized=1, auxiliary=0x30751900, descriptor_count=0):
        table = bytearray(0xEC)
        struct.pack_into(">10H", table, 0, 1, 5, 2, 0x1A, 0x2A, 0xAA, 0x6A, 0x12A, 0x22A, 0x42A)
        struct.pack_into(">H", table, 0xBA, readiness_mask)
        table[0xE8] = initialized
        struct.pack_into(">6I", table, 0x14, 1, 0x80814000, 0x01B5EF00,
                         0x01B55B10, 0xFFFFFFFF, 0)
        struct.pack_into(">I", table, 0x8C, descriptor_count)
        struct.pack_into(">I", table, 0xA4, 1)
        struct.pack_into(">i", table, 0xB0, -1)
        values = {
            (reader.AUXILIARY_PTR, 4): struct.pack(">I", auxiliary),
            (0x30751980, 8): struct.pack(">iI", mode, 19),
            (reader.READINESS_TABLE, 0xEC): bytes(table),
        }
        return lambda address, size: values[(address, size)]

    def test_mode_two_waits_for_its_exact_bit_and_ignores_other_bits(self):
        waiting = reader.readiness_snapshot(self.fixture(readiness_mask=1))
        self.assertEqual(waiting["mode_80"], 2)
        self.assertEqual(waiting["required_mode_mask"], 2)
        self.assertEqual(waiting["missing_mode_mask"], 2)
        self.assertFalse(waiting["poll_ready"])
        ready = reader.readiness_snapshot(self.fixture(readiness_mask=0x802))
        self.assertEqual(ready["missing_mode_mask"], 0)
        self.assertTrue(ready["poll_ready"])

    def test_null_auxiliary_uninitialized_table_and_invalid_mode_are_not_indexed(self):
        for changes in ({"auxiliary": 0}, {"initialized": 0}, {"mode": 10}, {"mode": -2}):
            with self.subTest(changes=changes):
                result = reader.readiness_snapshot(self.fixture(**changes))
                self.assertIsNone(result["required_mode_mask"])
                self.assertIsNone(result["missing_mode_mask"])
                self.assertIsNone(result["poll_ready"])
        self.assertTrue(reader.readiness_snapshot(self.fixture(mode=-1, initialized=0))["poll_ready"])

    def test_readiness_rejects_pointer_overflow_and_short_reads(self):
        with self.assertRaisesRegex(ValueError, "invalid readiness memory range"):
            reader.readiness_snapshot(self.fixture(auxiliary=0xFFFFFFF0))
        with self.assertRaisesRegex(ValueError, "short readiness read"):
            reader.readiness_snapshot(lambda address, size: bytes(size - 1))

    def test_descriptor_summaries_use_only_captured_table_and_cap_bad_count(self):
        result = reader.readiness_snapshot(self.fixture(descriptor_count=1))
        self.assertEqual(result["descriptor_count_8c"], 1)
        self.assertEqual(result["requested_count_a4"], 1)
        self.assertEqual(result["loading_word_b0"], -1)
        self.assertEqual(result["descriptors"][0]["bit_index"], 1)
        self.assertEqual(result["descriptors"][0]["content_handle"], "0x80814000")
        self.assertEqual(result["descriptors"][0]["loaded_pointer"], "0x01b5ef00")
        self.assertFalse(result["descriptors_truncated"])
        capped = reader.readiness_snapshot(self.fixture(descriptor_count=0xFFFFFFFF))
        self.assertEqual(len(capped["descriptors"]), 5)
        self.assertTrue(capped["descriptors_truncated"])

    def test_stage_guard_runs_after_optional_scheduler_reads(self):
        fixture = BootflowTest().fixture()
        scheduler_ran = False

        def scheduler(read):
            nonlocal scheduler_ran
            scheduler_ran = True
            return {}

        def read(address, size):
            if scheduler_ran and address == 0x30000118:
                return struct.pack(">7I", 20, 0, 0, 0, 20, 0, 0)
            return fixture(address, size)

        with patch.object(reader, "request_chain", return_value=[]), \
                patch.object(reader, "scheduler_snapshot", side_effect=scheduler):
            self.assertFalse(reader.sample(read, include_request=True)["stage_unchanged_during_sample"])

    def test_package_records_use_id_map_pool_stride_and_report_unregistered(self):
        # Shape seen live in FINAL124_NEXT2: pool 0x316a1e80, records at +0x20, stride 0x1a8,
        # package 0x0172 at index 0x49 (only the _2 overlay registered).
        pool, index = 0x316A1E80, 0x49
        record = bytearray(0x180)
        struct.pack_into(">H", record, 4, 0x0172)
        struct.pack_into(">Q", record, 0x10, 0x56FCAA7C)
        struct.pack_into(">I", record, 0xB4, 0x3B5)
        struct.pack_into(">I", record, 0xEC, 1)
        record[0x140:0x140 + 48] = b"packages/ps3_client_bootstrap_patchabl_0172_2.pkg"
        struct.pack_into(">II", record, 0x178, 0x560, 0x019BAD40)
        values = {
            (reader.PACKAGE_POOL_PTR, 4): struct.pack(">I", pool),
            (pool, 12): struct.pack(">III", 0x20, 0x892A4, 0x1A8),
            (reader.PACKAGE_LIST_HEAD, 2): struct.pack(">H", 3),
            (reader.PACKAGE_LIST_COUNT, 4): struct.pack(">I", 314),
            (reader.PACKAGE_INDEX_BY_ID + 0x172 * 2, 2): struct.pack(">H", index),
            (reader.PACKAGE_INDEX_BY_ID + 0x999 * 2, 2): struct.pack(">H", 0xFFFF),
            (pool + 0x20 + index * 0x1A8, 0x180): bytes(record),
        }
        result = reader.package_records([0x172, 0x999], lambda addr, size: values[(addr, size)])
        self.assertEqual(result["count"], 314)
        found, missing = result["packages"]
        self.assertEqual(found["record"], "0x316a9788")
        self.assertEqual(found["record_id"], "0x0172")
        self.assertEqual(found["build_time_10"], "0x0000000056fcaa7c")
        self.assertEqual(found["path_140"], "packages/ps3_client_bootstrap_patchabl_0172_2.pkg")
        self.assertEqual(found["header_block_17c"], "0x019bad40")
        self.assertIsNone(missing["index"])
        self.assertNotIn("record", missing)


if __name__ == "__main__":
    unittest.main()
