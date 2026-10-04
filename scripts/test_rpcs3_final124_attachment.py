import struct
import unittest

import rpcs3_final124_attachment as attachment


class Memory:
    def __init__(self):
        self.data = {}

    def put(self, address, data):
        self.data.update((address + i, byte) for i, byte in enumerate(data))

    def words(self, address, *words):
        self.put(address, struct.pack(f">{len(words)}I", *words))

    def read(self, address, size):
        try:
            return bytes(self.data[address + i] for i in range(size))
        except KeyError as exc:
            raise OSError("fixture does not map that range") from exc


class AttachmentSnapshot(unittest.TestCase):
    def setUp(self):
        self.mem = Memory()
        self.base, self.runtime = 0x60000000, 0x61000000
        self.owner, self.component = (0x3D6 << 13) | 1, (0x3D6 << 13) | 2
        self.target, self.binding = (0x3DC << 13) | 7, (0x3DF << 13) | 4
        self.asset = 0x80A3D8AD
        self.mem.put(attachment.COMPONENT_POOL, bytes(0x40))
        for offset, value in ((8, self.base), (12, 24), (16, 8), (0x30, 0x3D6)):
            self.mem.words(attachment.COMPONENT_POOL + offset, value)
        self.mem.words(attachment.UNIT_POOL + 0x10, 32)
        self.mem.words(attachment.UNIT_POOL + 0x30, 0x3DC)
        self.mem.words(attachment.UNIT_MASK, 1 << 7)
        owner_row = self.base + 24
        component_row = self.base + 48
        self.mem.words(owner_row, 0xC0000014, 0, (owner_row - self.runtime) & 0xFFFFFFFF,
                       0xFFFFFFFF, self.component, 0)
        self.mem.words(self.runtime, 0, self.asset, 1, 0x80800906, 0)
        self.component_runtime = self.runtime + 0x40
        self.mem.words(component_row, 0xC0000054, 0,
                       (component_row - self.component_runtime) & 0xFFFFFFFF,
                       self.owner, 0xFFFFFFFF, attachment.NETWORK_COMPONENT)
        self.mem.put(self.component_runtime, bytes(0x54))
        self.mem.words(self.component_runtime + 8, 0x80AAD033)
        self.mem.words(self.component_runtime + 0x1C, self.target, self.binding)
        self.mem.words(0x1896D50, 0x2000)
        self.mem.words(0x2000, 0x10000)
        descriptor = 0x10000 + 0x3DF * 0x40
        self.mem.words(descriptor + 0x10, 0x62000000)
        self.mem.words(descriptor + 0x30, 0x120, 0xFFFFFFFF)
        self.binding_runtime = 0x63000000
        binding_row = 0x62000000 + 4 * 0x120
        self.mem.words(binding_row + 8, (binding_row - self.binding_runtime) & 0xFFFFFFFF)
        self.mem.words(self.binding_runtime + 0x104, 0xFFFFFFFF, self.target)

    def snapshot(self):
        return attachment.snapshot(self.mem.read, self.owner, self.target, self.asset)

    def test_component_and_binding_targets_are_verified_separately(self):
        result = self.snapshot()
        self.assertEqual(result["component_count"], 1)
        binding = result["components"][0]["binding"]
        self.assertTrue(binding["matches_selected_target"])
        self.assertTrue(binding["unit_membership"]["unit_mask_bit"])
        self.assertFalse(result["owner_unit_membership"]["unit_bank_matches"])

    def test_different_binding_unit_is_reported_without_assuming_the_parent_target(self):
        self.mem.words(self.binding_runtime + 0x108, self.owner)
        binding = self.snapshot()["components"][0]["binding"]
        self.assertFalse(binding["matches_selected_target"])
        self.assertFalse(binding["unit_membership"]["unit_bank_matches"])
        self.assertFalse(binding["unit_membership"]["unit_mask_bit"])

    def test_out_of_range_unit_index_never_reads_past_the_bitmap(self):
        membership = attachment.unit_membership(self.mem.read, (0x3DC << 13) | 99)
        self.assertFalse(membership["unit_index_in_range"])
        self.assertIsNone(membership["unit_mask_bit"])

    def test_stale_asset_wrong_owner_and_wrong_target_are_rejected(self):
        changes = ((self.runtime + 4, 0x80A3D8AE, "asset"),
                   (self.base + 48 + 12, self.owner + 1, "belong"),
                   (self.component_runtime + 0x1C, self.target + 1, "target"))
        for address, value, error in changes:
            with self.subTest(error=error):
                old = self.mem.read(address, 4)
                self.mem.words(address, value)
                with self.assertRaisesRegex(ValueError, error):
                    self.snapshot()
                self.mem.put(address, old)

    def test_chain_count_and_pool_bounds_limit_reads(self):
        self.mem.words(self.base + 48 + 16, self.component)
        with self.assertRaisesRegex(ValueError, "exceeds"):
            self.snapshot()
        self.mem.words(self.runtime + 8, 2)
        with self.assertRaisesRegex(ValueError, "cycle"):
            self.snapshot()
        self.mem.words(self.runtime + 8, 65)
        with self.assertRaisesRegex(ValueError, "layout"):
            self.snapshot()
        self.mem.words(self.base + 24 + 16, (0x3D6 << 13) | 9)
        self.mem.words(self.runtime + 8, 1)
        with self.assertRaisesRegex(ValueError, "outside"):
            self.snapshot()

    def test_structure_changing_during_sample_is_rejected(self):
        reads = 0

        def read(address, size):
            nonlocal reads
            if address == self.base + 48:
                reads += 1
                if reads == 2:
                    self.mem.words(address + 12, self.owner + 1)
            return self.mem.read(address, size)

        with self.assertRaisesRegex(ValueError, "changed"):
            attachment.snapshot(read, self.owner, self.target, self.asset)

    def test_link_changing_without_a_pool_row_change_is_rejected(self):
        for address, size, change_address, value in (
                (self.component_runtime, 0x20, self.component_runtime + 0x1C, self.target + 1),
                (self.component_runtime + 0x20, 4, self.component_runtime + 0x20, self.binding + 1),
                (self.binding_runtime + 0x104, 8, self.binding_runtime + 0x108, self.target + 1)):
            with self.subTest(address=address):
                reads = 0
                old = self.mem.read(change_address, 4)

                def read(at, length):
                    nonlocal reads
                    if at == address and length == size:
                        reads += 1
                        if reads == 2:
                            self.mem.words(change_address, value)
                    return self.mem.read(at, length)

                with self.assertRaisesRegex(ValueError, "changed"):
                    attachment.snapshot(read, self.owner, self.target, self.asset)
                self.mem.put(change_address, old)

    def test_network_binding_field_must_fit_inside_its_allocation(self):
        self.mem.words(self.base + 48, 0xC0000020)
        with self.assertRaisesRegex(ValueError, "allocation"):
            self.snapshot()


if __name__ == "__main__":
    unittest.main()
