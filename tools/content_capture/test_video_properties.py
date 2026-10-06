"""Media inspection fixtures; live native decoding is a separate acceptance check."""
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

from capture import inspect_video_properties, validate_video


class VideoPropertiesTests(unittest.TestCase):
    def test_native_average_rate_is_distinct_from_base_rate_and_output(self):
        log = """Input #0, mov, from 'native.mp4':
  Duration: 00:00:25.93, start: 0.000000, bitrate: 2323 kb/s
  Stream #0:0[0x1]: Video: mpeg4 (Advanced Simple Profile), yuv420p, 1280x720 [SAR 1:1 DAR 16:9], 2166 kb/s, 9 fps, 30 tbr, 15360 tbn (default)
  Stream #0:1[0x2]: Audio: aac (LC), 48000 Hz, stereo
Output #0, null, to 'pipe:':
  Stream #0:0: Video: wrapped_avframe, yuv420p, 1920x1080, 30 fps, 30 tbn
"""
        result = subprocess.CompletedProcess([], 0, "", log)
        with patch("capture.subprocess.run", return_value=result) as run:
            media = validate_video(Path("native.mp4"), "configured-ffmpeg")
        properties = media["video_properties"]
        self.assertEqual((properties["width"], properties["height"]), (1280, 720))
        self.assertEqual(properties["average_fps_reported"], 9)
        self.assertEqual(properties["base_fps_reported"], 30)
        self.assertIn("Rounded", properties["frame_rate_precision"])
        self.assertEqual(media["duration_seconds"], 25.93)
        self.assertTrue(media["contains_audio"])
        self.assertEqual(media["full_decode"], "PASS")
        self.assertEqual(run.call_count, 1)
        self.assertIn("-xerror", run.call_args[0][0])

    def test_fractional_and_scaled_rates_do_not_parse_time_base_as_fps(self):
        log = "  Stream #0:3(eng): Video: h264, yuv420p(tv, bt709), 1920x1080, 29.97 fps, 1k tbr, 90k tbn"
        properties = inspect_video_properties(log)
        self.assertEqual(properties["average_fps_reported"], 29.97)
        self.assertEqual(properties["base_fps_reported"], 1000)
        self.assertEqual((properties["width"], properties["height"]), (1920, 1080))

    def test_unknown_input_properties_cannot_be_inferred_from_output_or_audio(self):
        log = """  Stream #0:0: Audio: pcm_s16le, 48000 Hz
Output #0, null, to 'pipe:':
  Stream #0:1: Video: wrapped_avframe, yuv420p, 1280x720, 60 fps
"""
        properties = inspect_video_properties(log)
        self.assertEqual(properties["inspection"], "NOT_DETERMINED")
        for field in ("width", "height", "average_fps_reported", "base_fps_reported"):
            self.assertIsNone(properties[field])

    def test_decode_failure_cannot_receive_verified_media_properties(self):
        log = "Duration: 00:00:30.00\n  Stream #0:0: Video: h264, yuv420p, 1280x720, 30 fps"
        with patch("capture.subprocess.run", return_value=subprocess.CompletedProcess([], 1, "", log)):
            with self.assertRaisesRegex(ValueError, "failed full decode"):
                validate_video(Path("failed.mp4"), "configured-ffmpeg")


if __name__ == "__main__":
    unittest.main()
