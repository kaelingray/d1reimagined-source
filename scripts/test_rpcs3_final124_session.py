import struct
import unittest

import rpcs3_final124_session as reader


class Memory:
    def __init__(self):
        self.bytes = {}

    def put(self, address, data):
        self.bytes.update({address + i: value for i, value in enumerate(data)})

    def word(self, address, value):
        self.put(address, struct.pack(">I", value))

    def read(self, address, size):
        try:
            return bytes(self.bytes[address + i] for i in range(size))
        except KeyError as error:
            raise OSError(f"unmapped fixture read {address:#x}+{size}") from error


def context_fixture(index=7, flags=2, generation=0x34):
    memory = Memory()
    descriptor = bytearray(0x34)
    struct.pack_into(">I", descriptor, 8, 0x10000000)
    struct.pack_into(">III", descriptor, 0x18, 0x100, 4, 0xFFFFFFFF)
    struct.pack_into(">I", descriptor, 0x30, flags)
    memory.put(reader.CONTEXT_DESCRIPTOR, descriptor)
    memory.word(0x10000100 + index * 4, generation)
    memory.word(reader.DATUM_TABLE_PTR, 0x11000000)
    memory.word(0x11000000, 0x12000000)
    datum_descriptor = bytearray(0x38)
    struct.pack_into(">I", datum_descriptor, 0x10, 0x20000000)
    struct.pack_into(">II", datum_descriptor, 0x30, 0x1000, 0xFF)
    memory.put(0x12000080, datum_descriptor)  # native packed group 2
    memory.word(0x20000000 + index * 0x1000 + 8, 0x120)
    return memory


class ContextLookupTests(unittest.TestCase):
    def test_native_normal_and_high_bit_packing_vectors(self):
        # Hand-calculated outputs of the two native rlwimi branches. Generation
        # bits differ; the datum group/index and adjusted address stay the same.
        for flags, generation, expected in ((2, 0x34, 0x1A004007),
                                             (0x40000002, 3, 0x98004007)):
            with self.subTest(flags=flags):
                result = reader.context_address(7, context_fixture(flags=flags, generation=generation).read)
                self.assertEqual(result["packed"], expected)
                self.assertEqual(result["group"], 2)
                self.assertEqual(result["address"], 0x20006FE0)
                self.assertTrue(result["unchanged_during_lookup"])

    def test_zero_is_a_valid_index_but_all_ones_is_null(self):
        self.assertEqual(reader.context_address(0, context_fixture(index=0).read)["address"], 0x1FFFFFE0)
        self.assertIsNone(reader.context_address(0xFFFFFFFF, lambda *_: self.fail("null must not read")))

    def test_generation_change_is_reported(self):
        memory = context_fixture()
        calls = 0

        def read(address, size):
            nonlocal calls
            if address == 0x1000011C:
                calls += 1
                if calls == 2:
                    return struct.pack(">I", 0x35)
            return memory.read(address, size)

        self.assertFalse(reader.context_address(7, read)["unchanged_during_lookup"])

    def test_uninitialized_descriptor_is_rejected_before_dereference(self):
        with self.assertRaisesRegex(ValueError, "descriptor"):
            reader.context_address(0, lambda address, size: bytes(size))

    def test_session_handle_comes_from_slot_map_and_pending_fields_are_preserved(self):
        memory = context_fixture()
        engine, manager, mapping, slices = 0x30000000, 0x31000000, 0x32000000, 0x33000000
        memory.word(reader.UPDATE_GATE_FLAGS_PTR, 0)
        memory.word(reader.UPDATE_GATE_COUNT_PTR, 0)
        memory.word(reader.ACTIVITY_CLOCK_PTR, 0x34000000)
        memory.word(0x3400000C, 5848)
        for pointer, value in ((reader.ENGINE_PTR, engine), (reader.MANAGER_PTR, manager),
                               (reader.SLOT_MAP_PTR, mapping), (reader.SLICE_TABLE_PTR, slices)):
            memory.word(pointer, value)
        memory.put(engine + 0x88, bytes(0x4E0))
        memory.put(engine + 0x80, bytes(2))
        memory.put(mapping, bytes(16))
        memory.word(mapping + 0x14, 7)
        memory.word(manager + 0x4E0, 6)
        memory.word(manager + 0x4F4, 0xFFFFFFFF)  # old, wrong layout decoy
        memory.word(manager + 0x550, 0x1234)
        context = bytearray(0x34)
        context[4], context[0x18], context[0x20] = 1, 4, 2
        struct.pack_into(">I", context, 0x1C, 123)
        struct.pack_into(">I", context, 0x28, 0x120)  # embedded datum relocation tag
        struct.pack_into(">I", context, 0x30, 456)
        memory.put(0x20006FE0, context)
        memory.put(0x2000C058, bytes.fromhex("01040000000000000203000000000000"))
        memory.put(slices + 0x110C, struct.pack(">2i16I", -1, -1, *([0, 0, 0, 0x10000] + [0] * 12)))
        result = reader.snapshot(memory.read)
        session = result["sessions"][0]
        self.assertEqual(session["context_handle"], 7)
        self.assertEqual(session["context"]["pending_18"], 4)
        self.assertEqual(session["context"]["pending_20"], 2)
        self.assertEqual(session["context"]["pending_since_30"], 456)
        self.assertEqual(result["active_loading"], [-1, -1])
        self.assertEqual(result["loaded_words"][3], 0x10000)
        self.assertEqual(result["activity_clock"]["tick_0c"], 5848)
        self.assertTrue(result["pointers_unchanged"])
        self.assertFalse(result["atomic"])


if __name__ == "__main__":
    unittest.main()
