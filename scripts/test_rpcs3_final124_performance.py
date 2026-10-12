import struct
import unittest

import rpcs3_final124_performance as probe


class Memory:
    def __init__(self):
        self.bytes = {}

    def put(self, address, data):
        self.bytes.update({address + index: value for index, value in enumerate(data)})

    def word(self, address, value):
        self.put(address, struct.pack(">I", value))

    def read(self, address, size):
        try:
            return bytes(self.bytes[address + index] for index in range(size))
        except KeyError as error:
            raise OSError(f"unmapped synthetic read {address:#x}+{size}") from error


def fixture():
    memory = Memory()
    registry, table, manager = 0x10000, 0x20000, 0x50000
    memory.word(probe.RESOURCE_REGISTRY, registry)
    memory.word(registry, table)
    memory.word(probe.MANAGER_POINTER, manager)
    memory.put(manager, bytes(0x1850))
    # Hand-calculated native registry indexes include the tag bank (0x400);
    # they are not package-file IDs. Positive salted handles retain ten bits.
    handles = ((0x80AA9A07, 0x554, 0x100000),
               (0x80CB8D5B, 0x65C, 0x200000),
               (0x72FAC3FD, 0x3D6, 0x300000))
    roots = []
    for handle, index, root in handles:
        entry = table + index * 64
        metadata = bytearray(0x38)
        # The resource record is 16 bytes beyond its adjusted root.
        record = root + 16
        struct.pack_into(">I", metadata, 0x10, record - (handle & 0x1FFF) * 32)
        struct.pack_into(">II", metadata, 0x30, 32, 0xFF)
        memory.put(entry, metadata)
        memory.put(record, struct.pack(">III", 0, 0, 16))
        roots.append(root)
    definition, authored_root, instance_root = roots
    memory.word(definition + 0x10, 0x100)
    memory.word(definition + 0x100 + 0x38, 0x60000)
    memory.word(definition + 0x100 + 0x3C, 0)
    memory.word(0x60000, probe.START_ROUTINE)
    instance = instance_root + 0x700
    header = bytearray(0xD0)
    struct.pack_into(">III", header, 0, 0x80CB8D5B, 0x1000, 0x808028A1)
    header[0x21] = 2
    struct.pack_into(">i", header, 0x28, 0)
    struct.pack_into(">II", header, 0x34, 1, 0x200)
    memory.put(instance, header)
    authored = bytearray(0xC0)
    struct.pack_into(">III", authored, 0, 0x80CB8D5B, 0x50, probe.PERFORMANCE_TYPE)
    struct.pack_into(">II", authored, 0xAC, 1, 0x200)
    memory.put(authored_root + 0x1000, authored)
    memory.put(authored_root + 0x12C0, bytes([0, 17]))
    memory.put(instance + 0x200, bytes(0x90))
    memory.put(instance + 0x248, struct.pack(">II", 0x80CB8D5B, 0x1200))
    memory.put(authored_root + 0x1200, bytes(0xC0))
    record = struct.pack(">8I", 0, 0x1234, 0x80CB8D5B, 0x777,
                         probe.PERFORMANCE_CLASS, 0x72FAC3FD, 0x700,
                         probe.PERFORMANCE_TYPE)
    memory.word(manager + 0x1350, 1)
    memory.put(manager + 0x1354, record)
    return memory, manager, instance


class PerformanceProbe(unittest.TestCase):
    def test_startup_null_manager_does_not_read_through_zero(self):
        memory = Memory()
        memory.word(probe.MANAGER_POINTER, 0)
        result = probe.snapshot(memory.read, True, True)
        self.assertFalse(result["initialized"])
        self.assertIsNone(result["count"])
        self.assertTrue(result["unchanged_during_read"])
        self.assertNotIn("actor_slots", result)

    def test_class_dispatch_and_salted_instance_are_distinct_from_asset(self):
        memory, _, instance = fixture()
        result = probe.snapshot(memory.read, True, True)
        row = result["performances"][0]
        self.assertFalse(result["atomic"])
        self.assertTrue(result["unchanged_during_read"])
        self.assertEqual(row["instance"], instance)
        self.assertEqual(row["target"], probe.START_ROUTINE)
        self.assertEqual(row["authored_definition"], 0x201000)
        self.assertEqual((row["phase_raw"], row["scene_index"], row["scene_count"]), (2, 0, 1))
        self.assertEqual(row["scenes"][0]["definition"], 0x201200)
        self.assertEqual(row["bindings"], [{"role": 0, "type": 17}])
        self.assertEqual(row["cast_source"], result["manager"])
        self.assertEqual(len(result["actor_slots"]), 6)

    def test_native_tag_and_dynamic_handle_lookup_vectors(self):
        memory, _, _ = fixture()
        resources = probe.Resources(memory.read)
        for handle, expected in ((0x80AA9A07, 0x100000),
                                 (0x80CB8D5B, 0x200000), (0x72FAC3FD, 0x300000)):
            self.assertEqual(resources.resolve(handle), expected)

    def test_rejects_bad_stride_and_null_handle(self):
        memory, _, _ = fixture()
        resources = probe.Resources(memory.read)
        with self.assertRaisesRegex(ValueError, "null"):
            resources.resolve(0xFFFFFFFF)
        memory.word(0x20000 + 0x554 * 64 + 0x30, 0)
        with self.assertRaisesRegex(ValueError, "stride"):
            resources.resolve(probe.PERFORMANCE_CLASS)

    def test_unsupported_dispatch_does_not_interpret_scene_state(self):
        memory, _, _ = fixture()
        memory.word(0x60000, 0x123456)
        row = probe.snapshot(memory.read)["performances"][0]
        self.assertIn("dispatch", row["error"])
        self.assertNotIn("scene_count", row)

    def test_virtual_dispatch_uses_adjusted_instance_and_vtable_slot(self):
        memory, _, instance = fixture()
        memory.word(0x100000 + 0x100 + 0x38, 9)
        memory.word(0x100000 + 0x100 + 0x3C, 0x100)
        memory.word(instance + 0x100, 0x70000)
        memory.word(0x70008, 0x60000)
        row = probe.snapshot(memory.read)["performances"][0]
        self.assertEqual(row["target"], probe.START_ROUTINE)
        self.assertNotIn("error", row)

    def test_unsupported_class_is_reported_before_resource_access(self):
        memory, manager, _ = fixture()
        memory.word(manager + 0x1354 + 0x10, 0x12345678)
        row = probe.snapshot(memory.read)["performances"][0]
        self.assertIn("class", row["error"])
        self.assertNotIn("instance", row)

    def test_wrong_instance_identity_does_not_interpret_scene_state(self):
        memory, _, instance = fixture()
        memory.word(instance, 0x80CB8D65)
        row = probe.snapshot(memory.read)["performances"][0]
        self.assertIn("identity", row["error"])
        self.assertNotIn("scene_count", row)

    def test_record_count_and_scene_count_are_bounded(self):
        memory, manager, instance = fixture()
        memory.word(manager + 0x1350, 33)
        with self.assertRaisesRegex(ValueError, "count"):
            probe.snapshot(memory.read)
        memory.word(manager + 0x1350, 1)
        memory.word(instance + 0x34, 1025)
        self.assertIn("count", probe.snapshot(memory.read)["performances"][0]["error"])

    def test_change_during_record_read_is_exposed(self):
        memory, manager, _ = fixture()
        calls = 0
        def read(address, size):
            nonlocal calls
            if address == manager + 0x1354:
                calls += 1
                if calls == 2:
                    return bytes(size)
            return memory.read(address, size)
        self.assertFalse(probe.snapshot(read)["unchanged_during_read"])

    def test_repeated_lookup_keeps_first_resource_signature(self):
        memory, _, _ = fixture()
        resources = probe.Resources(memory.read)
        resources.resolve(probe.PERFORMANCE_CLASS)
        memory.word(0x20000 + 0x554 * 64 + 4, 1)
        resources.resolve(probe.PERFORMANCE_CLASS)
        self.assertFalse(resources.unchanged())

    def test_binding_roles_are_signed_and_alternate_cast_is_relative(self):
        memory, _, instance = fixture()
        memory.put(0x2012C0, bytes([255, 16]))
        memory.word(instance + 0xC0, 0x1000)
        row = probe.snapshot(memory.read)["performances"][0]
        self.assertEqual(row["bindings"], [{"role": -1, "type": 16}])
        self.assertEqual(row["cast_source"], instance + 0x10C0)

    def test_authored_identity_and_binding_count_are_guarded(self):
        memory, _, _ = fixture()
        memory.word(0x201008, 0)
        self.assertIn("authored", probe.snapshot(memory.read)["performances"][0]["error"])
        memory.word(0x201008, probe.PERFORMANCE_TYPE)
        memory.word(0x2010AC, 65)
        self.assertIn("binding count", probe.snapshot(memory.read)["performances"][0]["error"])

    def test_authored_binding_change_during_read_is_exposed(self):
        memory, _, _ = fixture()
        calls = 0
        def read(address, size):
            nonlocal calls
            if address == 0x2012C0:
                calls += 1
                if calls == 2:
                    return bytes(size)
            return memory.read(address, size)
        self.assertFalse(probe.snapshot(read)["unchanged_during_read"])


if __name__ == "__main__":
    unittest.main()
