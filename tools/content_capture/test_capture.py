import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch, Mock

from capture import Capture, atomic_json, closed_source_hashes, digest, check_text, lock
from test_fragmented_media import box
from monitor import CheckpointEvidence, Detector, Monitor, Tail, ValidationErrors, native_exception_observation, observations


class PipelineTests(unittest.TestCase):
    def test_status_lock_contention_defers_telemetry_and_retries_all_changes(self):
        initial = self.capture.status(RECORDER_STATUS="IDLE")
        status_path = self.capture.root / "machine-status.json"
        original = status_path.read_bytes()
        native_lock = lock
        with native_lock(self.capture.root / "status.lock"):
            with patch("capture.lock", side_effect=lambda path, timeout=0: native_lock(path, timeout=0)):
                first = self.capture.status(RECORDER_STATUS="RECORDING", ACTIVE_RUN="owned-run")
                second = self.capture.status(PROCESSING_ERROR="pending report")
        self.assertTrue(first["STATUS_UPDATE_DEFERRED"] and second["STATUS_UPDATE_DEFERRED"])
        self.assertEqual(first["updated_at"], initial["updated_at"])
        self.assertEqual(first["RECORDER_STATUS"], "IDLE")
        self.assertEqual(status_path.read_bytes(), original)
        retried = self.capture.status(MONITOR_ERROR=None)
        self.assertEqual(retried["RECORDER_STATUS"], "RECORDING")
        self.assertEqual(retried["ACTIVE_RUN"], "owned-run")
        self.assertEqual(retried["PROCESSING_ERROR"], "pending report")
        self.assertFalse(retried["STATUS_UPDATE_DEFERRED"])
        self.assertIsNone(retried["STATUS_UPDATE_ERROR"])

    def test_concurrent_deferred_status_updates_keep_both_fields(self):
        native_lock = lock
        results = []
        with native_lock(self.capture.root / "status.lock"):
            with patch("capture.lock", side_effect=lambda path, timeout=0: native_lock(path, timeout=0)):
                threads = [threading.Thread(target=lambda value=value: results.append(self.capture.status(**value)))
                           for value in ({"RECORDER_STATUS": "RECORDING"}, {"NAS_CONNECTED": True})]
                for worker in threads:
                    worker.start()
                for worker in threads:
                    worker.join(timeout=3)
                self.assertTrue(all(not worker.is_alive() for worker in threads))
        self.assertEqual(len(results), 2)
        self.assertTrue(all(result["STATUS_UPDATE_DEFERRED"] for result in results))
        retried = self.capture.status()
        self.assertEqual(retried["RECORDER_STATUS"], "RECORDING")
        self.assertTrue(retried["NAS_CONNECTED"])

    def test_status_io_failure_preserves_pending_fields_for_next_update(self):
        initial = self.capture.status(RECORDER_STATUS="IDLE")
        with patch("capture.atomic_json", side_effect=OSError(28, "full disk")):
            deferred = self.capture.status(RECORDER_STATUS="RECORDING")
        self.assertTrue(deferred["STATUS_UPDATE_DEFERRED"])
        self.assertEqual(deferred["STATUS_UPDATE_ERROR"], 28)
        self.assertEqual(deferred["updated_at"], initial["updated_at"])
        self.assertEqual(self.capture.status()["RECORDER_STATUS"], "RECORDING")

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

    def test_importer_importance_vocabulary_and_legacy_alias(self):
        for supplied, expected in (("LOW", "LOW_VALUE"), ("LOW_VALUE", "LOW_VALUE"), ("REJECT", "REJECT")):
            with self.subTest(importance=supplied):
                self.video.write_bytes(supplied.encode())
                package = self.capture.package(self.video, self.metadata(importance=supplied), False)
                self.assertEqual(json.loads((package / "metadata.json").read_text())["importance"], expected)

    def test_engineering_report_reuses_verified_outbox_without_media_or_publication_clearance(self):
        report = self.root / "daily-report.md"
        report.write_text("Graphics fault persists. CPU diagnostics pass; gameplay acceptance is not established.")
        events = [{"id": "observed-event", "maturity": "Experimental", "earlier_package_id": "already-delivered"}]
        package = self.capture.package_report(report, events, self.metadata())
        meta = json.loads((package / "metadata.json").read_text())
        self.assertEqual(meta["recorded_at"], "2026-10-05T16:20:00+00:00")
        self.assertIsNone(meta["devblog"])
        self.assertTrue(meta["contains_sensitive_information"] and meta["requires_manual_review"])
        self.assertNotIn("full_decode", meta)
        self.assertEqual(set(Capture.verify(package)["files"]), {"metadata.json", "daily-report.md", "events.json"})
        self.assertFalse((package / "clip.mp4").exists())
        original = report.read_bytes()
        result = self.capture.sync()[0]
        self.assertEqual(result["status"], "SUBMITTED")
        ready = Path(result["destination"])
        self.assertEqual(Capture.verify(ready), Capture.verify(package))
        self.assertEqual(report.read_bytes(), original)
        with self.assertRaisesRegex(ValueError, "immutable"):
            self.capture.review(package.name)

    def test_engineering_report_rejects_secrets_empty_payloads_and_traceability_overrides(self):
        report = self.root / "daily-report.md"
        for text in ("", "api_key=secret"):
            report.write_text(text)
            with self.assertRaises(ValueError):
                self.capture.package_report(report, [], self.metadata())
        report.write_text("Known failure; fix acceptance remains open.")
        with self.assertRaises(ValueError):
            self.capture.package_report(report, {}, self.metadata())
        with self.assertRaises(ValueError):
            self.capture.package_report(report, [], self.metadata(git_commit="invented"))
        with self.assertRaises(ValueError):
            self.capture.package_report(report, [], self.metadata(full_decode="PASS"))
        self.assertFalse(list(self.capture.outbox.glob("*.building")))

    def test_report_clearance_cannot_submit_unreviewed_visual_payload(self):
        report = self.root / "daily-report.md"
        report.write_text("Private engineering notes.")
        package = self.capture.package_report(report, [], self.metadata())
        (package / "unreviewed.mp4").write_bytes(b"not reviewed footage")
        self.capture.make_hashes(package)
        result = self.capture.sync()[0]
        self.assertEqual(result["status"], "WAITING_FOR_NAS")
        self.assertIn("Review footage", self.capture.state(package.name)["error"])
        self.assertFalse(list(self.nas.rglob("*.ready")))

    def test_report_uses_reporting_day_without_changing_event_timestamp(self):
        report = self.root / "daily-report.md"
        report.write_text("Late London event retains its original UTC timestamp.")
        package = self.capture.package_report(report, [], self.metadata(
            recorded_at="2026-10-05T23:20:00+00:00", reporting_date="2026-10-06", reporting_timezone="Europe/London"))
        ready = Path(self.capture.sync()[0]["destination"])
        self.assertEqual(ready.parent.relative_to(self.nas).as_posix(), "incoming/test-machine-a/2026/10/06")
        self.assertEqual(json.loads((package / "metadata.json").read_text())["recorded_at"], "2026-10-05T23:20:00+00:00")

    def test_insufficient_nas_space_keeps_durable_source_and_retry(self):
        package = self.package()
        original = (package / "clip.mp4").read_bytes()
        import shutil
        disk_usage = shutil.disk_usage
        with patch("capture.shutil.disk_usage", side_effect=lambda path:
                   SimpleNamespace(free=1) if Path(path) == self.nas else disk_usage(path)):
            self.assertEqual(self.capture.sync()[0]["status"], "WAITING_FOR_NAS")
        status = self.capture.status()
        self.assertEqual(status["AVAILABLE_NAS_SPACE"], 1)
        self.assertIn("Insufficient", status["NAS_CAPACITY_ERROR"])
        self.assertEqual((package / "clip.mp4").read_bytes(), original)
        self.assertGreater(self.capture.state(package.name)["next_attempt"], 0)
        self.assertFalse(list(self.nas.rglob("*.ready")))

    def test_fragment_recovery_changes_only_private_buffer_copy(self):
        prefix = box(b"ftyp") + box(b"moov") + box(b"moof") + box(b"mdat", b"complete")
        source = prefix + box(b"moof") + b"unfinished"
        self.video.write_bytes(source)
        info = self.capture.add_buffer(self.video, "2026-10-05T17:20:00+01:00")
        self.assertEqual(self.video.read_bytes(), source)
        self.assertEqual((self.capture.buffer / info["file"]).read_bytes(), prefix)
        self.assertEqual(info["source_recovery"]["retained_bytes"], len(prefix))
        self.assertEqual(info["source_recovery"]["discarded_tail_bytes"], len(source) - len(prefix))

    def native_receipt(self, monitor):
        receipt = monitor.segments / "segment.json"
        atomic_json(receipt, {"file": str(self.video), "run": "owned-run",
                             "recorded_at": "2026-10-05T16:20:00+00:00",
                             "ended_at": "2026-10-05T16:20:40+00:00"})
        return receipt

    def test_pending_source_waits_for_controller_lock_release(self):
        monitor = Monitor(self.config)
        try:
            pending = monitor.segments / "pending.pending"
            atomic_json(pending, {"file": str(self.video)})
            with lock(monitor.segments / "recorder.lock"):
                monitor.recover_pending_segments()
            self.assertTrue(pending.exists())
            self.assertFalse(pending.with_suffix(".json").exists())
            self.assertEqual(self.video.read_bytes(), b"a closed source fixture")
        finally:
            monitor.pool.shutdown()

    def test_pending_source_waits_for_writer_without_decoding(self):
        monitor = Monitor(self.config)
        try:
            pending = monitor.segments / "pending.pending"
            atomic_json(pending, {"file": str(self.video)})
            with patch("monitor.closed_source", side_effect=OSError("still writing")), patch("capture.validate_video") as decode:
                monitor.process()
                decode.assert_not_called()
            self.assertTrue(pending.exists())
            self.assertFalse(pending.with_suffix(".json").exists())
        finally:
            monitor.pool.shutdown()

    def test_pending_source_recovery_uses_duration_instead_of_stop_request(self):
        monitor = Monitor(self.config)
        try:
            pending = monitor.segments / "pending.pending"
            atomic_json(pending, {"file": str(self.video), "run": "owned-run",
                                 "recorded_at": "2026-10-05T16:20:00+00:00",
                                 "stop_requested_at": "2026-10-05T16:20:10+00:00"})
            with patch("monitor.now", return_value="2026-10-05T16:25:00+00:00"):
                monitor.process()
            self.assertFalse(pending.exists())
            receipt = pending.with_suffix(".json")
            self.assertTrue(receipt.with_suffix(".indexed").exists())
            entry = json.loads(next(monitor.capture.buffer.glob("*.json")).read_text())
            self.assertEqual(entry["ended_at"], "2026-10-05T16:20:30+00:00")
            self.assertEqual(entry["stop_requested_at"], "2026-10-05T16:20:10+00:00")
            self.assertEqual(entry["closure_observed_at"], "2026-10-05T16:25:00+00:00")
            self.assertTrue(entry["controller_recovery"])
            monitor.process()
            self.assertEqual(len(list(monitor.capture.buffer.glob("*.mp4"))), 1)
        finally:
            monitor.pool.shutdown()

    def test_pending_receipt_publication_survives_interruption(self):
        monitor = Monitor(self.config)
        try:
            receipt = self.native_receipt(monitor)
            pending = receipt.with_suffix(".pending")
            atomic_json(pending, {"file": str(self.video)})
            original = receipt.read_bytes()
            monitor.recover_pending_segments()
            self.assertFalse(pending.exists())
            self.assertEqual(receipt.read_bytes(), original)
        finally:
            monitor.pool.shutdown()

    def test_interrupted_segment_is_verified_indexed_and_original_retained(self):
        prefix = box(b"ftyp") + box(b"moov") + box(b"moof") + box(b"mdat", b"complete")
        original = prefix + box(b"moof") + b"unfinished"
        self.video.write_bytes(original)
        monitor = Monitor(self.config)
        try:
            receipt = self.native_receipt(monitor)
            monitor.process()
            self.assertEqual(self.video.read_bytes(), original)
            self.assertFalse(receipt.with_suffix(".failed").exists())
            proof = json.loads(receipt.with_suffix(".indexed").read_text())
            self.assertEqual(proof["source_sha256"], digest(self.video))
            copy = next(monitor.capture.buffer.glob("*.mp4"))
            self.assertEqual(copy.read_bytes(), prefix)
            self.assertEqual(proof["retained_sha256"], digest(copy))
            entry = json.loads(copy.with_suffix(".json").read_text())
            self.assertEqual(entry["ended_at"], "2026-10-05T16:20:30+00:00")
            self.assertEqual(entry["source_ended_at"], "2026-10-05T16:20:40+00:00")
            event = json.loads(monitor.pending_path.read_text())[0]
            self.assertEqual(event["feature"], "native-recording-recovery")
            self.assertEqual(event["maturity"], "Experimental")
            self.assertEqual(event["at"], entry["ended_at"])
            monitor.process()
            self.assertEqual(len(list(monitor.capture.buffer.glob("*.mp4"))), 1)
        finally:
            monitor.pool.shutdown()

    def test_corrupt_buffer_copy_is_retryable_and_never_indexed(self):
        monitor = Monitor(self.config)
        try:
            receipt = self.native_receipt(monitor)
            add = monitor.capture.add_buffer
            def corrupt(*args):
                entry = add(*args)
                (monitor.capture.buffer / entry["file"]).write_bytes(b"corrupted copy")
                return entry
            with patch.object(monitor.capture, "add_buffer", side_effect=corrupt):
                monitor.process()
            self.assertTrue(self.video.exists())
            self.assertFalse(receipt.with_suffix(".indexed").exists())
            failure = json.loads(receipt.with_suffix(".failed").read_text())
            self.assertIn("differs", failure["error"])
            self.assertGreater(failure["next_attempt"], 0)
            self.assertFalse(monitor.pending_path.exists())
            self.assertEqual(list(monitor.capture.buffer.glob("*.json")), [])
            atomic_json(receipt.with_suffix(".failed"), {**failure, "next_attempt": 0})
            monitor.process()
            self.assertTrue(receipt.with_suffix(".indexed").exists())
            self.assertFalse(self.video.exists())
        finally:
            monitor.pool.shutdown()

    def test_finished_segment_is_byte_identical_before_original_retirement(self):
        original = self.video.read_bytes()
        monitor = Monitor(self.config)
        try:
            receipt = self.native_receipt(monitor)
            monitor.process()
            self.assertFalse(self.video.exists())
            copy = next(monitor.capture.buffer.glob("*.mp4"))
            self.assertEqual(copy.read_bytes(), original)
            proof = json.loads(receipt.with_suffix(".indexed").read_text())
            self.assertEqual(proof["source_sha256"], digest(copy))
            self.assertIsNone(proof["source_recovery"])
        finally:
            monitor.pool.shutdown()

    def test_closed_source_prefix_hash_crosses_chunk_boundary(self):
        import hashlib
        original = b"a" * (1024 * 1024 + 37) + b"discarded tail"
        self.video.write_bytes(original)
        length = 1024 * 1024 + 37
        proof = closed_source_hashes(self.video, length)
        self.assertEqual(proof["source_sha256"], hashlib.sha256(original).hexdigest())
        self.assertEqual(proof["retained_sha256"], hashlib.sha256(original[:length]).hexdigest())
        with self.assertRaisesRegex(ValueError, "exceeds"):
            closed_source_hashes(self.video, len(original) + 1)

    def test_event_window_does_not_repeat_retried_recording_segment(self):
        monitor = Monitor(self.config)
        try:
            common = {"run": "owned-run", "recorded_at": "2026-10-05T16:20:00+00:00",
                      "duration_seconds": 30, "contains_audio": True, "full_decode": "PASS"}
            stale = {**common, "file": "stale.mp4", "ended_at": "2026-10-05T16:20:40+00:00",
                     "source_recovery": {"retained_bytes": 100}}
            verified = {**common, "file": "verified.mp4", "ended_at": "2026-10-05T16:20:30+00:00",
                        "source_recovery": {"retained_bytes": 100, "retained_sha256": "proof"}}
            for entry in (stale, verified):
                atomic_json(monitor.capture.buffer / Path(entry["file"]).with_suffix(".json"), entry)
            atomic_json(monitor.pending_path, [{"event_id": "recovery", "run": "owned-run",
                                               "at": "2026-10-05T16:20:30+00:00"}])
            with patch.object(monitor, "package_window", return_value="package") as package:
                monitor.finish_events()
                self.assertEqual(monitor.job.result(timeout=5), "package")
            selected, metadata = package.call_args.args
            self.assertEqual([e["file"] for e in selected], ["verified.mp4"])
            self.assertEqual(metadata["capture_window"]["actual_end"], verified["ended_at"])
            self.assertTrue(metadata["capture_window"]["post_event_truncated"])
        finally:
            monitor.pool.shutdown()

    def test_gpu_checkpoint_evidence_is_experimental_and_excludes_raw_handles(self):
        text = ("NVIDIA diagnostic checkpoints (bounded command-region hints):\n"
                "  queue=0 records=2 capacity=2\n"
                "    stage=0x1 marker=123 operation=graphics-draw.before command_buffer=0xsecret pipeline=0xprivate\n"
                "    stage=0x2000 marker=122 operation=compute-dispatch.after command_buffer=0xother pipeline=0xhidden\n")
        events = observations({}, text, "")
        self.assertEqual(len(events), 1)
        feature, status, evidence, maturity = events[0]
        self.assertEqual((feature, status, maturity), ("gpu-command-checkpoints", "OBSERVED", "Experimental"))
        self.assertEqual(evidence["checkpoint_hints"][1], {"queue": 0, "stage": "0x2000", "operation": "compute-dispatch.after"})
        self.assertNotIn("pipeline", json.dumps(evidence))
        self.assertNotIn("marker", json.dumps(evidence))
        detector = Detector(self.capture)
        event = detector.observe(*events[0], run="owned-run")
        self.assertNotEqual(event["event_type"], "BUG_FIXED")
        self.assertIsNone(detector.observe(*events[0], run="owned-run"))

    def test_checkpoint_context_survives_split_read_and_ends_at_next_log_entry(self):
        parser = CheckpointEvidence()
        self.assertEqual(parser.observe("NVIDIA diagnostic checkpoints (bounded command-region hints):\n  queue=1 records=1 capacity=1"), [])
        line = "    stage=0x2000 marker=321 operation=compute-dispatch.before command_buffer=0x1 pipeline=0x2"
        self.assertEqual(parser.observe(line), [{"queue": 1, "stage": "0x2000", "operation": "compute-dispatch.before"}])
        self.assertEqual(parser.observe("·! unrelated log entry\n" + line), [])
        self.assertEqual(CheckpointEvidence().observe(line), [])

    def test_dma_checkpoint_context_keeps_only_normalized_region_hints(self):
        text = ("NVIDIA diagnostic checkpoints (bounded command-region hints):\n"
                "  queue=0 records=3 capacity=3\n")
        for operation in ("before", "mapped", "after"):
            text += (f"    stage=0x2000 marker=123 operation=dma-readback.{operation} "
                     "command_buffer=0xsecret pipeline=0xhidden graphics_pipeline=0xprivate "
                     "compute_pipeline=0xother readback_image=0xsensitive readback_buffer=0xraw "
                     "readback_offset=64 readback_length=256 readback_capacity=512\n")
        event = observations({}, text, "")[0]
        self.assertEqual((event[0], event[1], event[3]), ("gpu-command-checkpoints", "OBSERVED", "Experimental"))
        self.assertEqual(event[2]["checkpoint_hints"], [
            {"queue": 0, "stage": "0x2000", "operation": f"dma-readback.{operation}"}
            for operation in ("before", "mapped", "after")])
        self.assertNotRegex(json.dumps(event[2]), r"private|secret|hidden|sensitive|pipeline|readback_buffer|readback_offset")

    def test_checkpoint_expired_or_invalid_records_never_claim_a_command_region(self):
        header = "NVIDIA diagnostic checkpoints (bounded command-region hints):\n"
        expired = "  queue=0 records=1 capacity=1\n    stage=0x1 marker=99 expired-or-unknown"
        self.assertEqual(CheckpointEvidence().observe(header + expired), [])
        for queue, record in (("  queue=0 records=100 capacity=100", "stage=0x1 marker=1 operation=graphics-draw.before"),
                              ("  queue=0 records=1 capacity=1", "stage=0x0 marker=1 operation=graphics-draw.before"),
                              ("  queue=0 records=1 capacity=1", "stage=0x1 marker=0 operation=graphics-draw.before"),
                              ("  queue=0 records=1 capacity=1", "stage=0x1 marker=1 operation=arbitrary-secret")):
            self.assertEqual(CheckpointEvidence().observe(header + queue + "\n    " + record + " pipeline=0x1"), [])

    def test_checkpoint_continuation_length_is_bounded(self):
        parser = CheckpointEvidence()
        parser.observe("NVIDIA diagnostic checkpoints (bounded command-region hints):\n  queue=0 records=1 capacity=1")
        parser.observe("\n".join("continuation" for _ in range(132)))
        self.assertEqual(parser.observe("    stage=0x1 marker=1 operation=graphics-draw.before pipeline=0x2"), [])

    def test_validation_context_ends_at_native_important_log_entry(self):
        parser = ValidationErrors()
        self.assertEqual(parser.observe("Validation Error: callback"), (1, []))
        self.assertEqual(parser.observe("·! unrelated native entry\nSee VUID-vkCmdDraw-None-08613"), (0, []))

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
