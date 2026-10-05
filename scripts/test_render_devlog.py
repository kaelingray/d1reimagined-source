"""Protect against blank sections and misleading/out-of-range media selection."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from render_devlog import footage_spec, media_path


class FootageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "game.mp4").touch()
        self.scene = {"title": "Outro", "footage": {
            "file": "game.mp4", "start": 3, "duration": 7, "loop": False}}
        self.probe = patch("render_devlog.probe", return_value={"duration": 10, "audio": True})
        self.probe.start()
        self.addCleanup(self.probe.stop)

    def test_every_section_requires_footage(self):
        with self.assertRaisesRegex(ValueError, "Every section"):
            footage_spec({"title": "Outro"}, self.root, "unused")

    def test_source_window_retains_trim_and_full_duration(self):
        result = footage_spec(self.scene, self.root, "unused")
        self.assertEqual((result["start"], result["duration"], result["source_duration"]), (3, 7, 10))

    def test_rejects_trim_beyond_source(self):
        self.scene["footage"]["duration"] = 8
        with self.assertRaisesRegex(ValueError, "exceeds"):
            footage_spec(self.scene, self.root, "unused")

    def test_rejects_invalid_timing(self):
        for field, value in [("start", -1), ("start", float("nan")),
                             ("duration", 0), ("duration", float("inf"))]:
            with self.subTest(field=field, value=value):
                scene = {"title": "Outro", "footage": {**self.scene["footage"], field: value}}
                with self.assertRaises(ValueError):
                    footage_spec(scene, self.root, "unused")

    def test_loop_requires_boolean(self):
        self.scene["footage"]["loop"] = "false"
        with self.assertRaisesRegex(ValueError, "Boolean"):
            footage_spec(self.scene, self.root, "unused")

    def test_media_must_stay_inside_selected_directory(self):
        with self.assertRaises(ValueError):
            media_path(self.root, "../outside.mp4")


if __name__ == "__main__":
    unittest.main()
