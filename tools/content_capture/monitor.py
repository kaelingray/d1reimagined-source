"""Zero-touch native recording, conservative observation changes, and NAS retry worker.

No desktop, credentials, guest writes, debugger commands, or unsupported gameplay claims.
Only exact emulator identities published by the project's launchers are eligible.
"""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import uuid

from capture import Capture, REPO, atomic_json, check_text, closed_source_hashes, digest, git_context, lock, now

sys.path.insert(0, str(REPO / "scripts"))
from rpcs3_shared_log import open_shared_read


class Detector:
    """Evidence-driven transitions; FAILED -> OBSERVED cannot be labelled fixed."""
    def __init__(self, capture):
        self.capture = capture
        self.path = capture.root / "project-state.json"
        self.state = json.loads(self.path.read_text()) if self.path.exists() else {
            "capabilities": dict(capture.cfg.get("KNOWN_CAPABILITIES", {})), "comparisons": {}, "seen": [],
            "build": capture.cfg.get("CURRENT_GAME_BUILD")}

    def observe(self, feature, value, evidence, maturity="Experimental", *, run=None, observed_at=None):
        with lock(self.capture.root / "project-state.lock"):
            if self.path.exists():
                self.state = json.loads(self.path.read_text())
            build = self.capture.cfg.get("CURRENT_GAME_BUILD")
            if self.state.get("build", build) != build:
                archive = self.path.with_name("project-state-" + uuid.uuid4().hex + ".json")
                atomic_json(archive, self.state)
                self.state = {"capabilities": dict(self.capture.cfg.get("KNOWN_CAPABILITIES", {})),
                              "comparisons": {}, "seen": [], "build": build}
                self.state["unsubmitted_events"] = json.loads(archive.read_text()).get("unsubmitted_events", [])
            self.state["build"] = build
            return self._observe(feature, value, evidence, maturity, run, observed_at)

    def _observe(self, feature, value, evidence, maturity, run=None, observed_at=None):
        previous = self.state["capabilities"].get(feature, "UNKNOWN")
        if previous == value:
            return None
        if value not in {"OBSERVED", "FAILED", "PASSED", "PARTIAL"}:
            raise ValueError("Unknown observation status")
        check_text(json.dumps(evidence))
        comparison = self.state["comparisons"].get(feature)
        if value == "FAILED":
            comparison = comparison or self.capture.machine + "-" + uuid.uuid4().hex
            self.state["comparisons"][feature] = comparison
            kind, importance, role = ("REGRESSION" if previous == "PASSED" else "BUG"), "MAJOR", "BEFORE"
            description = f"{feature}: failure observed. {evidence['summary']}"
        elif previous == "FAILED" and value == "PASSED":
            kind, importance, role = "BUG_FIXED", "MAJOR", "AFTER"
            description = f"{feature}: explicit validation now passes after a recorded failure. {evidence['summary']}"
        else:
            kind, importance, role = ("FEATURE_WORKING" if value == "PASSED" else "FEATURE_PARTIAL"), "USEFUL", None
            description = f"{feature}: new {value.lower()} evidence. {evidence['summary']}"
        event = {"event_id": uuid.uuid4().hex, "at": observed_at or now(), "feature": feature,
                 "game_build": self.capture.cfg.get("CURRENT_GAME_BUILD"),
                 "event_type": kind, "importance": importance, "description": description,
                 "previous_state": previous, "current_state": value,
                 "maturity": maturity, "evidence": evidence,
                 "comparison_group_id": comparison, "comparison_role": role,
                 "remaining_issue": "An observed engine state or stub response does not establish playable gameplay.",
                 "milestone_group": f"{self.capture.cfg.get('CURRENT_GAME_BUILD')}/{feature}"}
        if run is not None:
            event["run"] = run
        self.state["capabilities"][feature] = value
        # Commit the transition and its capture intent together. Pending queue contention
        # or a worker restart must not leave a deduplicated transition with no footage job.
        if run is not None:
            self.state.setdefault("unsubmitted_events", []).append(event)
        atomic_json(self.path, self.state)
        self.capture.event("REGRESSED" if kind == "REGRESSION" else "DISCOVERED", feature, description,
                           build=self.capture.cfg.get("CURRENT_GAME_BUILD"), event_type=kind,
                           importance=importance, previous_state=previous, new_state=value,
                           associated_clip_ids=[], observation_id=event["event_id"])
        return event

    def unsubmitted_events(self):
        with lock(self.capture.root / "project-state.lock"):
            state = json.loads(self.path.read_text()) if self.path.exists() else self.state
            return state.get("unsubmitted_events", [])

    def acknowledge_events(self, identifiers):
        with lock(self.capture.root / "project-state.lock"):
            self.state = json.loads(self.path.read_text())
            self.state["unsubmitted_events"] = [event for event in self.state.get("unsubmitted_events", [])
                                                if event["event_id"] not in identifiers]
            atomic_json(self.path, self.state)


def native_exception_observation(exit_code):
    # Known native exception statuses; ordinary/forced exits alone cannot prove a crash.
    if exit_code not in {0xC0000005, 0xC000001D, 0xC0000374, 0xC0000409, 0xC00000FD, 0x80000003}:
        return None
    return ("native-process-exception", "FAILED", {
        "summary": f"The exact owned emulator exited with native exception status 0x{exit_code:08x}; the faulting subsystem remains unverified.",
        "source": "owned process handle", "exception_status": f"0x{exit_code:08x}"}, "Known Broken")


class ValidationErrors:
    """Read bounded callback continuations without retaining private diagnostic text."""
    def __init__(self):
        self.remaining = 0

    def observe(self, text):
        errors, identifiers = 0, set()
        for line in text.splitlines():
            is_error = bool(re.search(
                r"(?:Validation Error:|RSX: ERROR: \[Validation\] Code -?\d+ :)", line))
            if is_error:
                errors += 1
                self.remaining = 64
            elif re.search(r"^(?:·[!A-Za-z]|Validation (?:Warning|Info):)", line):
                self.remaining = 0
            elif self.remaining:
                self.remaining -= 1
            else:
                continue
            if is_error or self.remaining:
                identifiers.update(re.findall(r"\b(VUID-[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)+)", line))
        return errors, sorted(identifiers)


class CheckpointEvidence:
    """Normalize bounded device-loss checkpoint continuations; retain no raw handles."""
    OPERATIONS = {"command-buffer.begin", "command-buffer.end", "graphics-draw.before",
                  "graphics-draw.after", "compute-dispatch.before", "compute-dispatch.after"}

    def __init__(self):
        self.remaining = 0
        self.queue = None

    def observe(self, text):
        hints = []
        for line in text.splitlines():
            if line.strip() == "NVIDIA diagnostic checkpoints (bounded command-region hints):":
                self.remaining, self.queue = 132, None
                continue
            if not self.remaining:
                continue
            if re.match(r"^·[!A-Za-z]", line):
                self.remaining, self.queue = 0, None
                continue
            self.remaining -= 1
            queue = re.fullmatch(r"\s+queue=([01]) records=(\d{1,10}) capacity=(\d{1,10})", line)
            if queue:
                index, count, capacity = map(int, queue.groups())
                self.queue = index if 0 < count <= capacity <= 64 else None
                continue
            record = re.match(r"\s+stage=0x([0-9a-fA-F]{1,8}) marker=[1-9]\d{0,19} operation=([^\s]+)\s", line)
            if record and self.queue is not None and int(record[1], 16) and record[2] in self.OPERATIONS:
                hint = {"queue": self.queue, "stage": f"0x{int(record[1], 16):x}", "operation": record[2]}
                if hint not in hints:
                    hints.append(hint)
        return hints


def observations(sample, native_lines, backend_lines, validation=None, checkpoints=None):
    result = []
    for line in native_lines.splitlines():
        if "D1FPBIND " not in line:
            continue
        fields = dict(re.findall(r"(\w+)=([^\s]+)", line.split("D1FPBIND ", 1)[1]))
        if all(fields.get(k) == v for k, v in {"offset": "0x0", "location": "0", "start": "0x0",
                "length": "0x10", "constants": "0x0", "synthetic": "1"}.items()):
            result.append(("zero-local-depth-fragment", "OBSERVED", {
                "summary": "Experimental zero-local depth path used a bounded 16-byte END NOP instead of scanning vertex memory. Complete renderer stability remains unverified.",
                "source": "D1FPBIND", "canonical_length": 16}, "Experimental"))
            break
    if (sample.get("current") == 36 and sample.get("stage_unchanged_during_sample")
            and sample.get("world", {}).get("engine_state_89") == 0):
        result.append(("world-entry", "OBSERVED", {"summary": "01.24 reached world bootflow 36 with engine state 0.",
                                                   "atomic": False}, "Partially Working"))
    if re.search(r"(?:·F[^\n]*(?:\{RSX|RSX:|Vulkan|VK_)|VK_ERROR_DEVICE_LOST|Device lost)", native_lines):
        result.append(("renderer-device-loss", "FAILED", {"summary": "The owned run wrote a fatal renderer/device-loss diagnostic."}, "Known Broken"))
    if "malformed fragment program not saved" in native_lines:
        result.append(("live-fragment-validation", "FAILED", {"summary": "Live fragment program failed validation; cache refusal alone does not fix its source."}, "Known Broken"))
    validation_errors, validation_ids = (validation or ValidationErrors()).observe(native_lines)
    if validation_errors or validation_ids:
        reported = ", ".join(validation_ids[:3]) if validation_ids else "an error through the native callback"
        result.append(("renderer-vulkan-validation", "FAILED", {
            "summary": "The owned Vulkan validation layer reported " + reported +
                       ". The originating game defect and complete graphics acceptance remain unverified.",
            "source": "Vulkan validation", "validation_ids": validation_ids[:3],
            "distinct_ids_in_observation": len(validation_ids),
            "error_lines_in_observation": validation_errors}, "Known Broken"))
    hints = (checkpoints or CheckpointEvidence()).observe(native_lines)
    if hints:
        result.append(("gpu-command-checkpoints", "OBSERVED", {
            "summary": "The owned device-loss report returned GPU checkpoint stage/operation hints. They narrow a command region; the faulting shader/resource and renderer fix remain unverified.",
            "source": "native lost-device checkpoint query", "checkpoint_hints": hints[:4],
            "distinct_hints_in_observation": len(hints)}, "Experimental"))
    for service in set(re.findall(r"sent default reply for service (\d+) \(result error_none\)", backend_lines)):
        result.append(("service-" + service + "-stub", "PARTIAL",
                       {"summary": f"Local backend sent its default success-shaped reply for service {service}; client acceptance is unverified."}, "Stubbed / Mocked"))
    return result


class Tail:
    def __init__(self):
        self.offsets = {}
        self.partial = {}

    def read(self, path):
        try:
            with open_shared_read(path) as f:
                size = os.fstat(f.fileno()).st_size
                offset = self.offsets.get(str(path), 0)
                if size < offset:
                    offset = 0
                    self.partial.pop(str(path), None)
                # Drain bounded chunks in order: jumping to the latest bytes can
                # silently lose a brief fatal or probe burst. Raw logs stay local.
                f.seek(offset)
                data = f.read(4 * 1024 * 1024)
                self.offsets[str(path)] = f.tell()
                combined = self.partial.get(str(path), b"") + data
                lines = combined.split(b"\n")
                self.partial[str(path)] = lines.pop()
                return b"\n".join(lines).decode("utf-8", errors="replace")
        except OSError:
            return ""


class Monitor:
    def __init__(self, config):
        self.config = Path(config).resolve()
        self.capture = Capture(config)
        self.detector = Detector(self.capture)
        self.segments = self.capture.root / "native-segments"
        self.segments.mkdir(exist_ok=True)
        self.tail = Tail()
        self.validation = ValidationErrors()
        self.checkpoints = CheckpointEvidence()
        self.pending_path = self.capture.root / "pending-observations.json"
        self.pending = json.loads(self.pending_path.read_text()) if self.pending_path.exists() else []
        self.recorder = None
        self.reader = None
        self.manifest = None
        self.capture.status(RECORDER_STATUS="IDLE", ACTIVE_RUN=None, RECORDER_EXIT=None)
        self.stopfile = self.capture.root / "recorder.stop"
        self.pool = ThreadPoolExecutor(max_workers=1, thread_name_prefix="content-io")
        self.job = None
        self.last_git = git_context()["git_commit"]
        self.last_git_check = time.monotonic()
        self.inflight = None
        self.last_maintenance = 0

    def eligible_run(self):
        from rpcs3_owned_memory import OwnedMemory
        folder = Path(self.capture.cfg["RUN_MANIFEST_DIRECTORY"])
        if not folder.is_absolute():
            folder = self.config.parent / folder
        for path in sorted(folder.glob("*-processes.json"), key=lambda p: p.stat().st_mtime, reverse=True):
            try:
                m = json.loads(path.read_text(encoding="utf-8-sig"))
                if m.get("state") != "ready" or Path(str(path) + ".stop").exists():
                    continue
                reader = OwnedMemory(path)
                return path, reader
            except (OSError, ValueError, KeyError, json.JSONDecodeError):
                continue
        return None, None

    def start_recorder(self, manifest):
        self.stopfile.unlink(missing_ok=True)
        output = (self.capture.root / "native-recorder.log").open("a", encoding="utf-8")
        try:
            self.recorder = subprocess.Popen(
                ["pwsh", "-NoProfile", "-File", str(Path(__file__).with_name("Record-NativeBuffer.ps1")),
                 "-Config", str(self.config), "-ProcessManifest", str(manifest),
                 "-Segments", str(self.segments), "-StopFile", str(self.stopfile),
                 "-SegmentSeconds", str(self.capture.cfg.get("SEGMENT_SECONDS", 30))],
                stdout=output, stderr=output, creationflags=subprocess.CREATE_NO_WINDOW)
        finally:
            output.close()
        atomic_json(self.capture.root / "active-recorder.json", {"pid": self.recorder.pid, "manifest": str(manifest), "started_at": now()})

    def stop_recorder(self):
        self.stopfile.touch()
        if self.recorder:
            try:
                self.recorder.wait(timeout=20)
            except subprocess.TimeoutExpired:
                # Direct Popen handle, never a PID-wide kill. Native source stays recoverable locally.
                self.recorder.terminate()
                self.recorder.wait(timeout=5)
            self.recorder = None
        if self.reader:
            self.reader.close()
            self.reader = None
        self.manifest = None
        self.capture.status(RECORDER_STATUS="IDLE", ACTIVE_RUN=None, RECORDER_EXIT=None)

    def queue_observations(self):
        events = self.detector.unsubmitted_events()
        if not events:
            return
        identifiers = {event["event_id"] for event in events}
        with lock(self.capture.root / "pending.lock"):
            pending = json.loads(self.pending_path.read_text()) if self.pending_path.exists() else []
            existing = {event["event_id"] for event in pending}
            pending.extend(event for event in events if event["event_id"] not in existing)
            atomic_json(self.pending_path, pending)
        # A failed acknowledgement is harmless: the next pass merges by event identity.
        self.detector.acknowledge_events(identifiers)

    def detect(self):
        from rpcs3_final124_bootflow import sample, world_snapshot
        if self.manifest is None:
            self.manifest, self.reader = self.eligible_run()
            if self.manifest:
                self.tail = Tail()  # the shared native log belongs to this new run
                self.validation = ValidationErrors()
                self.checkpoints = CheckpointEvidence()
                self.start_recorder(self.manifest)
        if not self.manifest:
            return
        if Path(str(self.manifest) + ".stop").exists():
            self.stop_recorder()
            return
        import ctypes
        exit_code = ctypes.c_ulong()
        self.reader.kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        if not self.reader.kernel.GetExitCodeProcess(self.reader.handle, ctypes.byref(exit_code)):
            raise OSError("Could not query the retained emulator handle; process exit is unproven")
        if exit_code.value != 259:
            exception = native_exception_observation(exit_code.value)
            if exception:
                m = json.loads(self.manifest.read_text(encoding="utf-8-sig"))
                self.detector.observe(*exception, run=m["name"])
            self.stop_recorder()
            self.queue_observations()
            return
        try:
            s = sample(self.reader.read, include_request=False)
            s["world"] = world_snapshot(self.reader.read)
            self.capture.status(SAMPLE_STATUS="AVAILABLE", SAMPLE_ERROR=None)
        except (OSError, ValueError) as error:
            # Early engine tables and transitions are temporarily unreadable. The
            # still-live process keeps its recorder; this is not a process exit.
            s = {}
            self.capture.status(SAMPLE_STATUS="TRANSITION_UNAVAILABLE", SAMPLE_ERROR=str(error))
        m = json.loads(self.manifest.read_text(encoding="utf-8-sig"))
        emulator = next(p for p in m["processes"] if p["tag"] == "rpcs3")
        native = self.tail.read(Path(emulator["executable"]).parent / "log" / "RPCS3.log")
        backend = self.tail.read(self.manifest.parent / (m["name"] + "-bap.log"))
        for feature, value, evidence, maturity in observations(s, native, backend, self.validation, self.checkpoints):
            self.detector.observe(feature, value, evidence, maturity, run=m["name"])
        self.queue_observations()
        if self.recorder and self.recorder.poll() is not None:
            self.capture.status(RECORDER_STATUS="FAILED", RECORDER_EXIT=self.recorder.returncode)
            # No endless toggles after a recorder error. Next owned run retries.
        else:
            self.capture.status(RECORDER_STATUS="RECORDING", RECORDER_EXIT=None, ACTIVE_RUN=m["name"])

    def process(self):
        # Called on one low-priority worker: validation and SMB copies never block game monitoring.
        for receipt in sorted(self.segments.glob("*.json")):
            info = json.loads(receipt.read_text())
            done = receipt.with_suffix(".indexed")
            if done.exists():
                continue
            failure = receipt.with_suffix(".failed")
            if failure.exists() and json.loads(failure.read_text()).get("next_attempt", 0) > time.time():
                continue
            try:
                buffer_info = self.capture.add_buffer(info["file"], info["recorded_at"])
                buffer_info.update(ended_at=info["ended_at"], run=info["run"], source="native rendered game and audio",
                                   source_segment=receipt.stem)
                recovery = buffer_info.get("source_recovery")
                buffer_path = self.capture.buffer / buffer_info["file"]
                try:
                    proof = closed_source_hashes(info["file"], recovery["retained_bytes"] if recovery else None)
                    if (buffer_path.stat().st_size != proof["retained_bytes"]
                            or digest(buffer_path) != proof["retained_sha256"]
                            or (recovery and recovery["original_bytes"] != proof["original_bytes"])):
                        raise ValueError("Native buffer copy differs from its retained source bytes")
                except (OSError, ValueError):
                    # An unverified candidate cannot be selected for an event.
                    buffer_path.with_suffix(".json").unlink(missing_ok=True)
                    buffer_path.unlink(missing_ok=True)
                    raise
                if recovery:
                    # Termination time includes the discarded tail, which must not
                    # inflate the available post-event footage window.
                    decoded_end = datetime.fromisoformat(info["recorded_at"]) + timedelta(seconds=buffer_info["duration_seconds"])
                    buffer_info["source_ended_at"] = info["ended_at"]
                    buffer_info["ended_at"] = min(decoded_end, datetime.fromisoformat(info["ended_at"])).isoformat()
                    recovery.update(proof)
                atomic_json(self.capture.buffer / Path(buffer_info["file"]).with_suffix(".json"), buffer_info)
                feature = "native-recording-recovery" if recovery else "native-buffer-validation"
                summary = ("Interrupted RPCS3 recording recovered through complete fragments, fully decoded and verified against its original prefix; the unfinished original is preserved."
                           if recovery else "Automated RPCS3 recording finalized, fully decoded and copied with matching SHA-256.")
                self.detector.observe(feature, "PASSED",
                    {"summary": summary, "contains_audio": buffer_info["contains_audio"], "source_recovery": recovery},
                    "Experimental", run=info["run"], observed_at=buffer_info["ended_at"])
                self.queue_observations()
                atomic_json(done, {**proof, "source_recovery": recovery, "indexed_at": now()})
                if not recovery:
                    # Delete only the controller's byte-identical original. Keep
                    # interrupted originals intact for inspection or later recovery.
                    Path(info["file"]).unlink()
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                atomic_json(failure, {"error": str(error), "next_attempt": time.time() + 300})
                continue
        self.capture.ingest_events(self.capture.root / "automatic-events.jsonl") if (self.capture.root / "automatic-events.jsonl").exists() else None
        self.capture.sync()
        if not self.manifest and time.time() - self.last_maintenance > 3600:
            try:
                self.capture.prune_submitted()
            finally:
                self.last_maintenance = time.time()

    def finish_events(self):
        with lock(self.capture.root / "pending.lock"):
            self.pending = json.loads(self.pending_path.read_text()) if self.pending_path.exists() else []
        cutoff = datetime.now(timezone.utc) - timedelta(seconds=float(self.capture.cfg.get("POST_EVENT_BUFFER", 30)))
        for event in list(self.pending):
            when = datetime.fromisoformat(event["at"])
            if when > cutoff or (self.job and not self.job.done()):
                continue
            entries = [json.loads(p.read_text()) for p in self.capture.buffer.glob("*.json")]
            # Retried imports represent the same recorder segment. Prefer its
            # verified recovery metadata, without repeating footage in a clip.
            unique = {}
            for entry in entries:
                key = (entry.get("run"), datetime.fromisoformat(entry["recorded_at"]))
                quality = (bool(entry.get("source_recovery", {}).get("retained_sha256")),
                           bool(entry.get("source_segment")))
                prior = unique.get(key)
                if prior is None or quality > prior[0]:
                    unique[key] = (quality, entry)
            entries = [entry for quality, entry in unique.values()]
            start = when - timedelta(seconds=float(self.capture.cfg.get("PRE_EVENT_BUFFER", 60)))
            end = when + timedelta(seconds=float(self.capture.cfg.get("POST_EVENT_BUFFER", 30)))
            selected = [e for e in entries if e.get("run") == event.get("run")
                        and datetime.fromisoformat(e["recorded_at"]) <= end
                        and datetime.fromisoformat(e.get("ended_at", e["recorded_at"])) >= start]
            if not selected:
                continue  # preserve event until relevant footage is available
            latest = max(datetime.fromisoformat(e.get("ended_at", e["recorded_at"])) for e in selected)
            if latest < end and self.manifest:
                continue  # wait for finalized post-event footage
            event["capture_window"] = {"requested_start": start.isoformat(), "requested_end": end.isoformat(),
                                       "actual_start": min(e["recorded_at"] for e in selected), "actual_end": latest.isoformat(),
                                       "post_event_truncated": latest < end,
                                       "segmented_native_recording": True,
                                       "note": "Recorder rotation can introduce small gaps; clip contains finalized segments or verified complete fragments."}
            event["recorded_at"] = min(e["recorded_at"] for e in selected)
            event["development_session"] = self.capture.cfg.get("CURRENT_SESSION")
            event["suggested_content_types"] = ["youtube-devblog", "tiktok", "before-after"]
            self.job = self.pool.submit(self.package_window, selected, dict(event))
            self.inflight = event["event_id"]
            break

    def package_window(self, selected, metadata):
        # Hold buffer lock while remuxing to prevent retention from deleting selected segments.
        with lock(self.capture.root / "buffer.lock"):
            token = uuid.uuid4().hex
            playlist = self.capture.buffer / (token + ".txt")
            output = self.capture.buffer / (token + ".mp4")
            try:
                selected.sort(key=lambda e: e["recorded_at"])
                metadata["buffer_segments"] = selected
                playlist.write_text("".join(f"file '{s['file']}'\n" for s in selected), encoding="utf-8")
                r = subprocess.run([self.capture.ffmpeg, "-v", "error", "-nostdin", "-f", "concat", "-safe", "1",
                                    "-i", str(playlist), "-c", "copy", "-movflags", "+faststart", str(output)],
                                   capture_output=True, timeout=120)
                if r.returncode:
                    raise ValueError("Native segment remux failed")
                metadata["automatic_security_check"] = {"target": "owned RPCS3 native game and audio only",
                                                        "text_scan": "PASS", "visual_scan": "not available",
                                                        "publication_review_required": True}
                # Native game footage is transferred automatically but flagged for downstream review.
                package = self.capture.package(output, metadata, reviewed=False)
                self.capture.state(package.name, status="WAITING_FOR_NAS", automatic_game_source=True)
                self.capture.sync()
                return str(package)
            finally:
                playlist.unlink(missing_ok=True)
                output.unlink(missing_ok=True)

    def run(self):
        if os.name != "nt":
            raise OSError("Native automatic recording currently requires Windows")
        import ctypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetPriorityClass.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        if not kernel.SetPriorityClass(kernel.GetCurrentProcess(), 0x4000):
            self.capture.status(PRIORITY_ERROR=ctypes.get_last_error())
        stop = self.capture.root / "monitor.stop"
        try:
            while not stop.exists():
                try:
                    if self.capture.cfg.get("CONTENT_CAPTURE_ENABLED", True):
                        self.queue_observations()
                        self.detect()
                        if not self.job or self.job.done():
                            if self.job:
                                try:
                                    self.job.result()
                                    if self.inflight:
                                        with lock(self.capture.root / "pending.lock"):
                                            pending = json.loads(self.pending_path.read_text())
                                            atomic_json(self.pending_path, [e for e in pending if e["event_id"] != self.inflight])
                                    self.capture.status(PROCESSING_ERROR=None)
                                except Exception as error:
                                    self.capture.status(PROCESSING_ERROR=str(error))
                                self.inflight = None
                            self.finish_events()
                            if not self.job or self.job.done():
                                self.job = self.pool.submit(self.process)
                    if time.monotonic() - self.last_git_check >= 60:
                        self.last_git_check = time.monotonic()
                        current = git_context()["git_commit"]
                        if current != self.last_git:
                            self.capture.event("IMPROVED", "repository", "Repository revision changed; gameplay acceptance requires separate evidence.",
                                               previous_commit=self.last_git, new_commit=current)
                            self.last_git = current
                    self.capture.status(MONITOR_ERROR=None)
                except Exception as error:
                    self.capture.status(MONITOR_ERROR=str(error))
                time.sleep(2)
        finally:
            self.stop_recorder()
            self.pool.shutdown(wait=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--config", required=True)
    args = p.parse_args()
    monitor = Monitor(args.config)
    with lock(monitor.capture.root / "monitor.lock"):
        monitor.run()


if __name__ == "__main__":
    main()
