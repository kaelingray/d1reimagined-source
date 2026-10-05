from concurrent.futures import ThreadPoolExecutor
import errno
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch

from capture import Capture, atomic_json


class AtomicStatusTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.path = self.root / "state.json"
        atomic_json(self.path, {"old": True})

    def tearDown(self):
        self.temp.cleanup()

    @staticmethod
    def windows_error(code):
        error = PermissionError(errno.EACCES, "fixture: destination temporarily open")
        error.winerror = code
        return error

    def test_transient_replace_keeps_old_file_and_recovers(self):
        original = os.replace
        attempts = []
        def replace(source, target):
            attempts.append(source)
            self.assertEqual(json.loads(self.path.read_text()), {"old": True})
            self.assertEqual(json.loads(Path(source).read_text()), {"new": True})
            if len(attempts) < 3:
                raise self.windows_error(32)
            original(source, target)
        with patch("capture.os.replace", side_effect=replace), patch("capture.time.sleep") as sleep:
            atomic_json(self.path, {"new": True})
        self.assertEqual(json.loads(self.path.read_text()), {"new": True})
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [0.02, 0.05])
        self.assertEqual(list(self.root.glob("*.tmp")), [])

    def test_permanent_access_failure_is_bounded_and_preserves_old_file(self):
        with patch("capture.os.replace", side_effect=self.windows_error(5)) as replace:
            with patch("capture.time.sleep") as sleep:
                with self.assertRaises(PermissionError):
                    atomic_json(self.path, {"new": True})
        self.assertEqual(replace.call_count, 6)
        self.assertAlmostEqual(sum(call.args[0] for call in sleep.call_args_list), 0.77)
        self.assertEqual(json.loads(self.path.read_text()), {"old": True})
        self.assertEqual(list(self.root.glob("*.tmp")), [])

    def test_other_io_error_is_not_retried(self):
        with patch("capture.os.replace", side_effect=OSError(errno.ENOSPC, "fixture: full disk")) as replace:
            with patch("capture.time.sleep") as sleep:
                with self.assertRaises(OSError):
                    atomic_json(self.path, {"new": True})
        self.assertEqual(replace.call_count, 1)
        sleep.assert_not_called()
        self.assertEqual(json.loads(self.path.read_text()), {"old": True})

    def test_concurrent_observer_and_transfer_status_changes_are_preserved(self):
        config = self.root / "config.json"
        atomic_json(config, {"CONTENT_LOCAL_PATH": str(self.root / "local"),
                            "CONTENT_NAS_PATH": str(self.root / "nas"),
                            "DEVELOPMENT_MACHINE_ID": "atomic-status-fixture"})
        writers = [Capture(config) for _ in range(3)]
        with ThreadPoolExecutor(max_workers=6) as pool:
            results = [pool.submit(writers[i % 3].status, **{f"OBSERVATION_{i}": i}) for i in range(30)]
            for result in results:
                result.result(timeout=5)
        saved = json.loads((writers[0].root / "machine-status.json").read_text())
        self.assertTrue(all(saved[f"OBSERVATION_{i}"] == i for i in range(30)))

    @unittest.skipUnless(os.name == "nt", "Windows sharing modes")
    def test_real_windows_reader_releases_destination_for_atomic_replace(self):
        import ctypes
        from ctypes import wintypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                      wintypes.LPVOID, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
        kernel.CreateFileW.restype = wintypes.HANDLE
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = kernel.CreateFileW(str(self.path), 0x80000000, 3, None, 3, 0x80, None)
        self.assertNotEqual(handle, ctypes.c_void_p(-1).value)
        released = threading.Event()
        def release():
            kernel.CloseHandle(handle)
            released.set()
        timer = threading.Timer(0.15, release)
        timer.start()
        try:
            atomic_json(self.path, {"new": True})
            self.assertTrue(released.is_set())
            self.assertEqual(json.loads(self.path.read_text()), {"new": True})
        finally:
            timer.join(timeout=1)


if __name__ == "__main__":
    unittest.main()
