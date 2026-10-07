"""Frozen descriptor dump parsing, separate from any native Vulkan acceptance."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('descriptor_ring_decode', Path(__file__).parents[1] / 'scripts/rpcs3_descriptor_ring_decode.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

def fixture(overwritten=0):
    rows = []
    for kind in range(1,9):
        values = [11,22,33,44,55,66]
        if kind == 2:
            values[4] = (1 << 64)-4
        fields = ' '.join(f'{name}={value:016x}' for name,value in zip('abcdef',values))
        rows.append(f'D1RING serial={kind+overwritten} time_us={1000+kind} kind={kind} {fields}')
    return (f'D1RING begin retained=8 last={overwritten+8} overwritten={overwritten}\n' + '\n'.join(rows) +
            f'\nD1RING end last={overwritten+8}\n' +
            'Synthetic: vkResetDescriptorPool(): VkDescriptorPool 0x1 pending VkCommandBuffer 0x2\n')

class RingDecodeTests(unittest.TestCase):
    def test_all_event_kinds_and_unsigned_failure_result(self):
        result = MODULE.decode(fixture())
        self.assertEqual(len(result['records']),8)
        self.assertEqual([r['kind'] for r in result['records']],list(range(1,9)))
        self.assertIn('result=-4',result['records'][1]['decoded'])
        self.assertIn('path=poke',result['records'][2]['decoded'])
        self.assertIn('path=wait',result['records'][3]['decoded'])
        self.assertEqual(result['records'][4]['decoded'],'retire object=000000000000000b pool=0000000000000016 index=33 current=44 completed=55')
        self.assertIn('scope=22',result['records'][5]['decoded'])
        self.assertIn('cutoff=11',result['records'][6]['decoded'])
        self.assertIn('completed=44',result['records'][7]['decoded'])

    def test_overwritten_suffix_keeps_actual_host_times(self):
        result = MODULE.decode(fixture(100))
        self.assertEqual(result['overwritten'],100)
        self.assertEqual(result['records'][0]['serial'],101)
        self.assertEqual(result['records'][0]['host_time_us'],1001)

    def test_truncated_dump_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('D1RING end last=8',''))

    def test_incorrect_end_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('D1RING end last=8','D1RING end last=9'))

    def test_count_mismatch_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('retained=8','retained=7'))

    def test_duplicate_serial_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('serial=8','serial=7'))

    def test_gap_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('serial=3','serial=9'))

    def test_unknown_kind_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture().replace('kind=8','kind=99'))

    def test_second_dump_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.decode(fixture()+fixture())

    def test_later_errors_are_explicitly_uncaptured(self):
        result = MODULE.decode(fixture()+'Synthetic: vkResetDescriptorPool(): VkDescriptorPool 0x3 pending VkCommandBuffer 0x4\n')
        self.assertEqual(result['later_uncaptured_callback_count'],1)
        self.assertIn('VkDescriptorPool 0x1',result['first_validation_callback'])

if __name__ == '__main__':
    unittest.main()
