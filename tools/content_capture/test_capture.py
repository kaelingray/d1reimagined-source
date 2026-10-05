import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch, Mock

from capture import Capture, atomic_json, digest, check_text, lock
from monitor import Detector, Monitor, observations


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.nas = self.root / "nas"
        self.nas.mkdir()
        self.config = self.root / "config.json"
        self.config.write_text(json.dumps({"CONTENT_LOCAL_PATH": str(self.root / "local"),
            "CONTENT_NAS_PATH": str(self.nas), "DEVELOPMENT_MACHINE_ID": "test-machine-a",
            "CURRENT_GAME_BUILD": "01.24", "CURRENT_SESSION": "test-pipeline",
            "CURRENT_FEATURE": "inventory", "CURRENT_DEVBLOG": None,
            "BUFFER_SECONDS": 180, "KNOWN_CAPABILITIES": {"world-entry": "OBSERVED"}}))
        self.capture = Capture(self.config)
        self.video = self.root / "source.mp4"
        self.video.write_bytes(b"a closed source fixture")
        self.validator = patch("capture.validate_video", return_value={"duration_seconds": 30, "contains_audio": True, "full_decode": "PASS"})
        self.validator.start()
        self.context = patch("capture.git_context", return_value={"git_commit": "abc", "git_branch": "master", "dirty_worktree": False, "repository": None})
        self.context.start()
        self.frames = patch("capture.subprocess.run", return_value=subprocess.CompletedProcess([], 0, "", ""))
        self.frames.start()

    def tearDown(self):
        self.frames.stop()
        self.context.stop()
        self.validator.stop()
        self.temp.cleanup()

    def metadata(self, **extra):
        return {"recorded_at": "2026-10-05T17:20:00+01:00", "description": "Resolved inventory slots are visible.",
                "event_type": "AFTER", "previous_state": "Materials 7/20", "current_state": "Materials 0/20", **extra}

    def package(self, reviewed=True):
        return self.capture.package(self.video, self.metadata(), reviewed)

    def test_atomic_transfer_and_original_retained(self):
        package = self.package()
        seen = []
        original = Capture.verify
        def observe(path, expected=None):
            if path.parent != self.capture.outbox:
                seen.append(path.name)
                self.assertFalse(any(self.nas.rglob("*.ready")))
            return original(path, expected)
        with patch.object(Capture, "verify", side_effect=observe):
            result = self.capture.sync()
        self.assertEqual(result[0]["status"], "SUBMITTED")
        self.assertTrue(any(n.endswith(".uploading") for n in seen))
        ready = next(self.nas.rglob("*.ready"))
        self.capture.verify(ready, self.capture.verify(package))
        self.assertTrue(self.video.exists())
        self.assertTrue(package.exists())
        self.assertEqual(self.capture.status()["NAS_CONNECTED"], True)

    def test_outage_retry_backoff_and_recovery(self):
        package = self.package()
        self.capture.cfg["CONTENT_NAS_PATH"] = str(self.root / "offline")
        first = self.capture.sync()
        self.assertEqual(first[0]["retry_in_seconds"], 30)
        self.assertEqual(self.capture.sync(), [])
        self.assertEqual(self.capture.sync(force=True)[0]["retry_in_seconds"], 60)
        self.capture.cfg["CONTENT_NAS_PATH"] = str(self.nas)
        self.assertEqual(self.capture.sync(force=True)[0]["status"], "SUBMITTED")
        self.assertEqual(self.capture.state(package.name)["attempts"], 2)

    def test_interrupted_copy_never_ready(self):
        package = self.package()
        real_copy = __import__("shutil").copyfileobj
        def fail(source, destination, *args):
            destination.write(source.read(3))
            raise OSError("Simulated interrupted connection")
        with patch("capture.shutil.copyfileobj", side_effect=fail):
            result = self.capture.sync()
        self.assertEqual(result[0]["status"], "WAITING_FOR_NAS")
        self.assertFalse(list(self.nas.rglob("*.ready")))
        self.assertTrue(list(self.nas.rglob("*.uploading")))
        with patch("capture.shutil.copyfileobj", side_effect=real_copy):
            self.capture.sync(force=True)
        self.assertEqual(len(list(self.nas.rglob("*.ready"))), 1)
        self.assertTrue(package.exists())

    def test_corruption_cannot_become_ready(self):
        self.package()
        real_copy = __import__("shutil").copyfileobj
        def corrupt(source, destination, *args):
            real_copy(source, destination, *args)
            if destination.name.endswith("clip.mp4"):
                destination.seek(0)
                destination.write(b"corrupt")
        with patch("capture.shutil.copyfileobj", side_effect=corrupt):
            self.capture.sync()
        self.assertFalse(list(self.nas.rglob("*.ready")))

    def test_idempotent_crash_after_ready_rename(self):
        package = self.package()
        ready = self.capture.transfer(package)
        self.capture.state(package.name, status="VERIFYING")
        self.assertEqual(self.capture.sync()[0]["destination"], str(ready))
        self.assertEqual(len(list(self.nas.rglob("*.ready"))), 1)

    def test_duplicate_media_and_multi_machine_isolation(self):
        package = self.package()
        self.assertEqual(self.package(), package)
        self.assertEqual(len(list(self.capture.outbox.iterdir())), 1)
        cfg = json.loads(self.config.read_text())
        cfg.update(DEVELOPMENT_MACHINE_ID="test-machine-b", CONTENT_LOCAL_PATH=str(self.root / "other"))
        other_config = self.root / "other.json"
        other_config.write_text(json.dumps(cfg))
        other = Capture(other_config)
        other.package(self.video, self.metadata(), True)
        self.capture.sync()
        other.sync()
        incoming = self.nas / "incoming"
        self.assertEqual({p.name for p in incoming.iterdir()}, {"test-machine-a", "test-machine-b"})

    def test_security_holds_uncertain_and_blocks_known_secrets(self):
        package = self.package(reviewed=False)
        self.assertEqual(self.capture.sync(), [])
        self.assertEqual(self.capture.state(package.name)["status"], "HOLD_FOR_REVIEW")
        for text in ("password=hidden", "Authorization: Bearer secret", "api_key: secret", "-----BEGIN PRIVATE KEY-----"):
            with self.assertRaises(ValueError):
                check_text(text)
        source = self.root / "logs.txt"
        source.write_text("access_token=hidden")
        self.video.write_bytes(b"different source")
        with self.assertRaises(ValueError):
            self.capture.package(self.video, self.metadata(), True, [source])
        self.assertFalse(list(self.capture.outbox.glob("*.building")))

    def test_auto_native_uncertain_transfers_for_downstream_review(self):
        meta = self.metadata(automatic_security_check={"target": "owned RPCS3 native game and audio only",
            "text_scan": "PASS", "publication_review_required": True})
        package = self.capture.package(self.video, meta, False)
        self.capture.state(package.name, status="WAITING_FOR_NAS")
        self.assertEqual(self.capture.sync()[0]["status"], "SUBMITTED")
        self.assertTrue(json.loads((package / "metadata.json").read_text())["contains_sensitive_information"])

    def test_metadata_cannot_override_truth_and_requires_timezone(self):
        for fields in ({"sha256": "fake"}, {"contains_sensitive_information": False}, {"recorded_at": "2026-10-05T12:00:00"}):
            with self.assertRaises(ValueError):
                self.capture.package(self.video, self.metadata(**fields), True)

    def test_source_must_be_closed_and_decode(self):
        with patch("capture.validate_video", side_effect=ValueError("moov atom missing")):
            with self.assertRaises(ValueError):
                self.package()
        self.assertFalse(list(self.capture.outbox.iterdir()))
        if os.name == "nt":
            with self.video.open("wb") as writer:
                writer.write(b"still open")
                writer.flush()
                with self.assertRaises(OSError):
                    self.package()

    def test_retention_requires_verified_destination(self):
        package = self.package()
        self.capture.sync()
        self.capture.cfg["SUBMITTED_RETENTION_DAYS"] = 0
        destination = Path(self.capture.state(package.name)["destination"])
        (destination / "clip.mp4").write_bytes(b"bad")
        with self.assertRaises(ValueError):
            self.capture.prune_submitted()
        self.assertTrue(package.exists())

    def test_path_traversal_and_lock_exclusion(self):
        package = self.package()
        manifest = json.loads((package / "hashes.json").read_text())
        manifest["files"]["../outside"] = {"size": 1, "sha256": "a"}
        atomic_json(package / "hashes.json", manifest)
        with self.assertRaises(ValueError):
            self.capture.verify(package)
        with lock(self.root / "test.lock"):
            with self.assertRaises(OSError):
                with lock(self.root / "test.lock"):
                    pass

    def test_explicit_events_only_and_no_repeated_ingestion(self):
        path = self.root / "events.jsonl"
        path.write_text(json.dumps({"event_id": "routine", "content_candidate": False}) + "\n" +
            json.dumps({"event_id": "meaningful", "content_candidate": True, "clip_path": str(self.video),
                        "metadata": self.metadata(), "sensitivity_reviewed": True}) + "\n{unfinished")
        receipt = self.capture.ingest_events(path)
        self.assertEqual(set(receipt), {"meaningful"})
        self.assertEqual(receipt, self.capture.ingest_events(path))

    def test_buffer_rotation_is_bounded(self):
        for i in range(8):
            self.capture.add_buffer(self.video, f"2026-10-05T17:{i:02d}:00+01:00")
        self.assertEqual(len(list(self.capture.buffer.glob("*.mp4"))), 6)
        self.assertTrue(self.video.exists())

    def test_known_states_and_bug_fix_comparison(self):
        detector = Detector(self.capture)
        self.assertIsNone(detector.observe("world-entry", "OBSERVED", {"summary": "world"}))
        before = detector.observe("fallen-spawn", "FAILED", {"summary": "No actors"})
        observed = detector.observe("fallen-spawn", "OBSERVED", {"summary": "One actor"})
        self.assertNotEqual(observed["event_type"], "BUG_FIXED")
        detector.observe("fallen-spawn", "FAILED", {"summary": "No actors"})
        after = detector.observe("fallen-spawn", "PASSED", {"summary": "Explicit encounter acceptance passes"})
        self.assertEqual(before["comparison_group_id"], after["comparison_group_id"])
        self.assertEqual(after["comparison_role"], "AFTER")
        self.assertEqual(after["event_type"], "BUG_FIXED")
        self.assertIsNone(Detector(self.capture).observe("fallen-spawn", "PASSED", {"summary": "again"}))

    def test_stub_and_engine_observations_do_not_claim_playability(self):
        sample = {"current": 36, "stage_unchanged_during_sample": True, "world": {"engine_state_89": 0}}
        events = observations(sample, "malformed fragment program not saved", "sent default reply for service 701 (result error_none)")
        self.assertEqual([e[1] for e in events], ["OBSERVED", "FAILED", "PARTIAL"])
        self.assertIn("unverified", events[-1][2]["summary"])
        self.assertEqual(observations({}, "normal echo", "heartbeat"), [])

    def test_temporary_engine_sample_does_not_stop_live_recorder(self):
        monitor = Monitor(self.config)
        manifest = self.root / "run-processes.json"
        manifest.write_text(json.dumps({"name": "RUN", "processes": [{"tag": "rpcs3", "executable": str(self.root / "rpcs3.exe")}]}))
        monitor.manifest = manifest
        monitor.reader = Mock()
        def alive(handle, output):
            output._obj.value = 259
            return 1
        monitor.reader.kernel.GetExitCodeProcess = alive
        with patch("rpcs3_final124_bootflow.sample", side_effect=ValueError("startup table is not initialized")), \
             patch.object(monitor, "stop_recorder") as stop:
            monitor.detect()
            stop.assert_not_called()
        self.assertEqual(self.capture.status()["SAMPLE_STATUS"], "TRANSITION_UNAVAILABLE")
        monitor.pool.shutdown()

    def test_build_change_does_not_reuse_previous_acceptance(self):
        detector = Detector(self.capture)
        detector.observe("inventory", "PASSED", {"summary": "01.24 inventory accepted"})
        self.capture.cfg["CURRENT_GAME_BUILD"] = "different-build"
        event = detector.observe("inventory", "OBSERVED", {"summary": "New build observed"})
        self.assertEqual(event["previous_state"], "UNKNOWN")
        self.assertEqual(event["event_type"], "FEATURE_PARTIAL")


if __name__ == "__main__":
    unittest.main()
