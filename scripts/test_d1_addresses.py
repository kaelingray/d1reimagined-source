import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_addresses  # noqa: E402


class AddressIndexTest(unittest.TestCase):
    def test_every_label_is_backed_by_a_mention(self):
        index = d1_addresses.build()
        for addr, label in d1_addresses.load_labels().items():
            self.assertTrue(index[addr]["mentions"], f"0x{addr:x} {label['name']} has no mention")

    def test_labels_are_unique_identifiers(self):
        names = [label["name"] for label in d1_addresses.load_labels().values()]
        self.assertEqual(len(names), len(set(names)))
        for name in names:
            self.assertRegex(name, r"^[a-z_][a-z0-9_]*$")

    def test_sentence_context(self):
        text = "First fact. The loader `0x13f8ec` loads zone sets. Next fact."
        at = text.index("0x13f8ec")
        self.assertEqual(d1_addresses.sentence(text, at, at + 8), "The loader `0x13f8ec` loads zone sets")


if __name__ == "__main__":
    unittest.main()
