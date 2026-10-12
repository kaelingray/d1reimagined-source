import unittest
from rpcs3_fragment_binding_summary import summarize


class BindingSummaryTests(unittest.TestCase):
    def test_zero_binding_and_synthetic_source_are_distinct(self):
        base = "D1FPBIND offset=0x0 location=0 start=0x1f1c20 length=0x20 constants=0x10 before=bf800000 prefix=bf800000 code=3f99999a3ee666673f7f18b9 ctrl=0x3000000 color=0x0 mrt=0x0 synthetic=0"
        fixed = base.replace("start=0x1f1c20", "start=0x0").replace("length=0x20", "length=0x10").replace("constants=0x10", "constants=0x0").replace("code=3f99999a3ee666673f7f18b9", "code=00010000000000000000000000000000").replace("synthetic=0", "synthetic=1")
        result = summarize([base, fixed])
        self.assertEqual(result["zero_local"], 2)
        self.assertEqual(result["known_float_records"], 1)
        self.assertEqual([s["synthetic"] for s in result["zero_local_states"]], [0, 1])

    def test_incomplete_lines_are_reported(self):
        result = summarize(["unrelated", "D1FPBIND offset=0x0 location=0 start=", "D1FPBIND offset=invalid"])
        self.assertEqual(result["records"], 0)
        self.assertEqual(result["incomplete_records"], 2)


if __name__ == "__main__":
    unittest.main()
