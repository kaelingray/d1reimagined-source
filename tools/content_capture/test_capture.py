import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch, Mock

from capture import Capture, atomic_json, digest, check_text, lock
from monitor import Detector, Monitor, Tail, ValidationErrors, native_exception_observation, observations


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
        with patch("rpcs3_final124_bootflow.sample", return_value={}), \
             patch("rpcs3_final124_bootflow.world_snapshot", return_value={}):
            monitor.detect()
        self.assertEqual(self.capture.status()["SAMPLE_STATUS"], "AVAILABLE")
        self.assertIsNone(self.capture.status()["SAMPLE_ERROR"])
        monitor.pool.shutdown()

    def test_build_change_does_not_reuse_previous_acceptance(self):
        detector = Detector(self.capture)
        detector.observe("inventory", "PASSED", {"summary": "01.24 inventory accepted"})
        self.capture.cfg["CURRENT_GAME_BUILD"] = "different-build"
        event = detector.observe("inventory", "OBSERVED", {"summary": "New build observed"})
        self.assertEqual(event["previous_state"], "UNKNOWN")
        self.assertEqual(event["event_type"], "FEATURE_PARTIAL")

    def test_canonical_depth_observation_is_experimental(self):
        line = "D1FPBIND offset=0x0 location=0 start=0x0 length=0x10 constants=0x0 synthetic=1"
        events = observations({}, line, "")
        self.assertEqual(events[0][0:2], ("zero-local-depth-fragment", "OBSERVED"))
        self.assertEqual(events[0][3], "Experimental")
        self.assertEqual(observations({}, line.replace("synthetic=1", "synthetic=0"), ""), [])

    def test_vulkan_validation_failure_has_only_normalized_evidence(self):
        line = "Validation Error: [ VUID-vkCmdDispatch-None-08613 ] | Object = 0x1234 private context"
        events = observations({}, line + "\n" + line, "")
        self.assertEqual(events[0][0:2], ("renderer-vulkan-validation", "FAILED"))
        self.assertEqual(events[0][3], "Known Broken")
        evidence = events[0][2]
        self.assertEqual(evidence["validation_ids"], ["VUID-vkCmdDispatch-None-08613"])
        self.assertNotIn("private context", json.dumps(evidence))
        self.assertNotIn("0x1234", json.dumps(evidence))
        detector = Detector(self.capture)
        event = detector.observe(*events[0])
        self.assertEqual(event["event_type"], "BUG")
        self.assertEqual(event["comparison_role"], "BEFORE")
        self.assertIsNone(detector.observe(*events[0]))

    def test_vulkan_validation_warning_or_configuration_is_not_a_bug_event(self):
        self.assertEqual(observations({}, "Validation Warning: [ VUID-vkCmdDispatch-None-08613 ]", ""), [])
        self.assertEqual(observations({}, "enabled Vulkan validation; GPU assisted false", ""), [])

    def test_native_validation_error_without_vuid_is_preserved(self):
        line = "·E 0:03:00.000000 RSX: ERROR: [Validation] Code 0 : vkCmdDraw(): Object 0xdead private shader text"
        event = observations({}, line, "")[0]
        self.assertEqual(event[0:2], ("renderer-vulkan-validation", "FAILED"))
        self.assertEqual(event[2]["validation_ids"], [])
        self.assertEqual(event[2]["error_lines_in_observation"], 1)
        self.assertNotIn("private shader", json.dumps(event[2]))
        self.assertNotIn("0xdead", json.dumps(event[2]))

    def test_native_validation_warning_does_not_supply_error_ids(self):
        lines = ("·W RSX: WARNING: [Validation] Code 0 : VUID-vkCmdDraw-warning-12345\n"
                 "·E RSX: ERROR: [Validation] Code -1 : VUID-vkCmdDraw-None-08613")
        event = observations({}, lines, "")[0]
        self.assertEqual(event[2]["validation_ids"], ["VUID-vkCmdDraw-None-08613"])
        self.assertEqual(event[2]["error_lines_in_observation"], 1)

    def test_native_non_validation_error_is_not_a_validator_event(self):
        self.assertEqual(observations({}, "·E RSX: ERROR: [Other] Code 0 : private error", ""), [])
        self.assertEqual(observations({}, "·W RSX: WARNING: [Validation] Code 0 : compiler warning", ""), [])

    def test_native_validation_continuations_supply_normalized_ids(self):
        lines = ("·E 0:03:15.000000 RSX: ERROR: [Validation] Code 0 : private object 0xdead\n"
                 "Shader private-path with an invalid resource\n"
                 "The Vulkan spec states: see #VUID-vkCmdDrawIndexed-viewType-07752)\n"
                 "·W RSX: WARNING: [Validation] Code 0 : ignored\n"
                 "See #VUID-vkCmdDrawIndexed-warning-12345)\n"
                 "·N RSX: unrelated event\n"
                 "VUID-vkCmdDrawIndexed-unrelated-54321")
        evidence = observations({}, lines, "")[0][2]
        self.assertEqual(evidence["validation_ids"], ["VUID-vkCmdDrawIndexed-viewType-07752"])
        self.assertEqual(evidence["error_lines_in_observation"], 1)
        self.assertNotIn("0xdead", json.dumps(evidence))
        self.assertNotIn("private-path", json.dumps(evidence))

    def test_validation_continuation_spans_reads_and_ends_at_next_log_entry(self):
        parser = ValidationErrors()
        observations({}, "·E RSX: ERROR: [Validation] Code 0 : bad resource", "", parser)
        event = observations({}, "The spec: #VUID-vkCmdDrawIndexed-format-07753)", "", parser)[0]
        self.assertEqual(event[2]["validation_ids"], ["VUID-vkCmdDrawIndexed-format-07753"])
        self.assertEqual(event[2]["error_lines_in_observation"], 0)
        self.assertEqual(observations({}, "·N RSX: normal entry\nVUID-vkCmdDraw-unrelated-12345", "", parser), [])

    def test_validation_continuations_are_bounded(self):
        parser = ValidationErrors()
        observations({}, "Validation Error: bad resource", "", parser)
        self.assertEqual(observations({}, "continuation\n" * 65 + "VUID-vkCmdDraw-unrelated-12345", "", parser), [])

    def test_debugger_teardown_fatal_is_not_device_loss(self):
        line = "·F 0:11:47.588403 {GDB Server} SIG: Thread terminated due to fatal error: Error during socket read"
        self.assertEqual(observations({}, line, ""), [])

    def test_device_loss_is_detected_on_reporting_thread(self):
        for thread in ("RSX", "PPU", "SPU"):
            line = ("·F 0:03:19.887520 {" + thread + "} SIG: Thread terminated due to fatal error: "
                    "Assertion Failed! Vulkan API call failed with unrecoverable error: Device lost (VK_ERROR_DEVICE_LOST)")
            self.assertEqual(observations({}, line, "")[0][0:2], ("renderer-device-loss", "FAILED"))

    def test_native_exception_status_is_separate_from_gpu_loss(self):
        for code in (0xC0000005, 0xC000001D, 0xC0000374, 0xC0000409, 0xC00000FD, 0x80000003):
            event = native_exception_observation(code)
            self.assertEqual(event[0:2], ("native-process-exception", "FAILED"))
            self.assertEqual(event[3], "Known Broken")
        for code in (0, 1, 259, 0xFFFFFFFF):
            self.assertIsNone(native_exception_observation(code))

    @unittest.skipUnless(os.name == "nt", "Windows retained process handle")
    def test_real_windows_exception_status_retains_capture_intent_before_handle_close(self):
        import ctypes
        import sys
        monitor = Monitor(self.config)
        child = subprocess.Popen([sys.executable, "-c",
            "import ctypes; ctypes.WinDLL('kernel32').ExitProcess(0xc0000005)"],
            creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            self.assertEqual(child.wait(timeout=10), 0xC0000005)
            manifest = self.root / "owned-run.json"
            manifest.write_text(json.dumps({"name": "owned-run"}))
            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            def before_close():
                self.assertEqual(monitor.detector.unsubmitted_events()[0]["run"], "owned-run")
            reader = SimpleNamespace(kernel=kernel, handle=int(child._handle), close=Mock(side_effect=before_close))
            monitor.manifest, monitor.reader = manifest, reader
            monitor.detect()
            reader.close.assert_called_once()
            event = json.loads(monitor.pending_path.read_text())[0]
            self.assertEqual(event["evidence"]["exception_status"], "0xc0000005")
            self.assertEqual(event["comparison_role"], "BEFORE")
            self.assertIsNone(monitor.reader)
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=10)
            monitor.pool.shutdown()

    def test_failed_exit_query_preserves_live_reader_and_recorder(self):
        monitor = Monitor(self.config)
        try:
            monitor.manifest = self.root / "owned-run.json"
            reader = SimpleNamespace(kernel=Mock(), handle=123, close=Mock())
            reader.kernel.GetExitCodeProcess.return_value = 0
            recorder = Mock()
            monitor.reader, monitor.recorder = reader, recorder
            with self.assertRaisesRegex(OSError, "exit is unproven"):
                monitor.detect()
            self.assertIs(monitor.reader, reader)
            self.assertIs(monitor.recorder, recorder)
            reader.close.assert_not_called()
            recorder.terminate.assert_not_called()
        finally:
            monitor.pool.shutdown()

    def test_observation_capture_intent_survives_queue_failure_and_restart(self):
        monitor = Monitor(self.config)
        try:
            event = monitor.detector.observe(*native_exception_observation(0xC0000005), run="owned-run")
            with patch("monitor.atomic_json", side_effect=PermissionError("pending queue busy")):
                with self.assertRaises(PermissionError):
                    monitor.queue_observations()
            recovered = Detector(self.capture)
            self.assertEqual(recovered.unsubmitted_events()[0]["event_id"], event["event_id"])
            self.assertIsNone(recovered.observe(*native_exception_observation(0xC0000005), run="owned-run"))
            monitor.detector = recovered
            monitor.queue_observations()
            pending = json.loads(monitor.pending_path.read_text())
            self.assertEqual([(e["event_id"], e["run"]) for e in pending], [(event["event_id"], "owned-run")])
            self.assertEqual(recovered.unsubmitted_events(), [])
        finally:
            monitor.pool.shutdown()

    def test_failed_journal_acknowledgement_does_not_duplicate_queue(self):
        monitor = Monitor(self.config)
        try:
            event = monitor.detector.observe(*native_exception_observation(0xC0000005), run="owned-run")
            original = atomic_json
            def fail_ack(path, data):
                if Path(path) == monitor.detector.path:
                    raise PermissionError("state file busy")
                return original(path, data)
            with patch("monitor.atomic_json", side_effect=fail_ack):
                with self.assertRaises(PermissionError):
                    monitor.queue_observations()
            monitor.queue_observations()
            self.assertEqual([e["event_id"] for e in json.loads(monitor.pending_path.read_text())], [event["event_id"]])
            self.assertEqual(monitor.detector.unsubmitted_events(), [])
        finally:
            monitor.pool.shutdown()

    def test_journal_acknowledgement_preserves_newer_observations(self):
        detector = Detector(self.capture)
        first = detector.observe("first", "FAILED", {"summary": "first"}, run="owned-run")
        second = detector.observe("second", "FAILED", {"summary": "second"}, run="owned-run")
        detector.acknowledge_events({first["event_id"]})
        self.assertEqual([e["event_id"] for e in detector.unsubmitted_events()], [second["event_id"]])

    def test_build_change_preserves_previous_capture_intent_and_build(self):
        detector = Detector(self.capture)
        first = detector.observe("first", "FAILED", {"summary": "first"}, run="old-run")
        self.capture.cfg["CURRENT_GAME_BUILD"] = "next-build"
        second = detector.observe("second", "FAILED", {"summary": "second"}, run="new-run")
        events = detector.unsubmitted_events()
        self.assertEqual([e["event_id"] for e in events], [first["event_id"], second["event_id"]])
        self.assertEqual([e["game_build"] for e in events], ["01.24", "next-build"])

    def test_log_tail_preserves_probe_bursts_partial_lines_and_rotation(self):
        path = self.root / "RPCS3.log"
        path.write_bytes(b"D1FPBIND " + b"x" * (300 * 1024) + b"\nVK_ERROR_DEVICE")
        tail = Tail()
        first = tail.read(path)
        self.assertTrue(first.startswith("D1FPBIND "))
        self.assertNotIn("VK_ERROR_DEVICE", first)
        with path.open("ab") as stream:
            stream.write(b"_LOST\n")
        self.assertEqual(tail.read(path), "VK_ERROR_DEVICE_LOST")
        self.assertEqual(tail.read(path), "")
        path.write_bytes(b"new run\n")
        self.assertEqual(tail.read(path), "new run")

    def test_log_tail_does_not_skip_earlier_failure_in_large_backlog(self):
        path = self.root / "RPCS3.log"
        path.write_bytes(b"VK_ERROR_DEVICE_LOST\n" + b"ordinary\n" * 600000)
        tail = Tail()
        first = tail.read(path)
        self.assertTrue(first.startswith("VK_ERROR_DEVICE_LOST\n"))
        self.assertLessEqual(len(first.encode()), 4 * 1024 * 1024)
        self.assertTrue(tail.read(path).startswith("ordinary"))
        self.assertEqual(tail.read(path), "")

    @unittest.skipUnless(os.name == "nt", "Windows native sharing semantics")
    def test_log_tail_reads_while_windows_writer_is_open(self):
        import ctypes
        from ctypes import wintypes as wt
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [wt.LPCWSTR, wt.DWORD, wt.DWORD, ctypes.c_void_p,
                                      wt.DWORD, wt.DWORD, wt.HANDLE]
        kernel.CreateFileW.restype = wt.HANDLE
        kernel.CloseHandle.argtypes = [wt.HANDLE]
        path = self.root / "locked.log"
        path.write_bytes(b"VK_ERROR_DEVICE_LOST\n")
        # Writer permits readers, but a CRT reader that denies writing conflicts.
        handle = kernel.CreateFileW(str(path), 0x40000000, 1, None, 3, 0x80, None)
        self.assertNotIn(handle, (None, wt.HANDLE(-1).value))
        try:
            self.assertEqual(Tail().read(path), "VK_ERROR_DEVICE_LOST")
        finally:
            kernel.CloseHandle(handle)


if __name__ == "__main__":
    unittest.main()
