"""Local, verified source packages for the separate Destiny content machine.

Uses only the standard library and a configured FFmpeg for media validation/remux.
NAS paths and machine identifiers belong in private config, never in source code.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import uuid

REPO = Path(__file__).resolve().parents[2]
EVENT_TYPES = {
    "BREAKTHROUGH", "FEATURE_WORKING", "FEATURE_PARTIAL", "BUG", "BUG_FIXED",
    "FAILED_ATTEMPT", "TECHNICAL_DISCOVERY", "REGRESSION", "BEFORE", "AFTER",
    "GAMEPLAY", "DEBUGGING", "SERVER_WORK", "NETWORKING", "MISSION_PROGRESS",
    "UI_PROGRESS", "CHARACTER_PROGRESS", "AI_PROGRESS", "CONTENT_RESTORATION",
    "PERFORMANCE", "MILESTONE", "GENERAL_PROGRESS",
}
IMPORTANCE = {"CRITICAL", "MAJOR", "USEFUL", "SUPPORTING", "LOW"}
SECRET = re.compile(
    r"(?im)(?:password|passwd|api[_-]?key|access[_-]?token|refresh[_-]?token|"
    r"authorization|client[_-]?secret|nas[_-]?credential)\s*[\"']?\s*[:=]\s*\S+"
    r"|-----BEGIN (?:\w+ )?PRIVATE KEY-----|\b(?:gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,})\b"
)


def now():
    return datetime.now(timezone.utc).isoformat()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    try:
        with temporary.open("x", encoding="utf-8", newline="\n") as f:
            json.dump(value, f, indent=2, ensure_ascii=False, allow_nan=False)
            f.write("\n")
            f.flush()
            os.fsync(f.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


@contextmanager
def lock(path):
    """A process lock released by the OS even if a worker crashes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as f:
        f.seek(0, 2)
        if not f.tell():
            f.write(b"0")
            f.flush()
        f.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            yield
        finally:
            f.seek(0)
            if os.name == "nt":
                msvcrt.locking(f.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(f, fcntl.LOCK_UN)


@contextmanager
def closed_source(path):
    """On Windows, deny concurrent writers/deletion while copying a recording."""
    if path.is_symlink() or not path.is_file():
        raise ValueError("Source must be a regular, closed local file")
    if str(path).startswith(("\\\\", "//")):
        raise ValueError("Record and finish media locally before packaging")
    if os.name != "nt":
        with path.open("rb") as f:
            yield f
        return
    import ctypes
    from ctypes import wintypes
    import msvcrt
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                  wintypes.LPVOID, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    kernel.CreateFileW.restype = wintypes.HANDLE
    handle = kernel.CreateFileW(str(path), 0x80000000, 1, None, 3, 0x80, None)
    if handle == ctypes.c_void_p(-1).value:
        raise OSError(ctypes.get_last_error(), "Source is unavailable or still open for writing")
    with os.fdopen(msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY), "rb") as f:
        yield f


def copy_closed(source, destination, max_bytes=10 * 1024**3):
    source = source.absolute()
    ensure_local(source)
    with closed_source(source) as f:
        before = os.fstat(f.fileno())
        if not 0 < before.st_size <= max_bytes:
            raise ValueError("Source is empty or exceeds the configured package limit")
        with destination.open("xb") as out:
            shutil.copyfileobj(f, out, 1024 * 1024)
            out.flush()
            os.fsync(out.fileno())
        after = os.fstat(f.fileno())
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ValueError("Source changed during packaging")


def check_text(text):
    if SECRET.search(text):
        raise ValueError("Possible credentials in package text; redact locally before packaging")


def ensure_local(path):
    if str(path).startswith(("\\\\", "//")):
        raise ValueError("Recording and outbox paths must be local")
    if os.name == "nt":
        import ctypes
        kernel = ctypes.WinDLL("kernel32")
        kernel.GetDriveTypeW.argtypes = [ctypes.c_wchar_p]
        if kernel.GetDriveTypeW(path.anchor) == 4:
            raise ValueError("Mapped NAS drives cannot be used for recording or outbox storage")


def validate_video(path, ffmpeg):
    result = subprocess.run(
        [ffmpeg, "-hide_banner", "-nostdin", "-xerror", "-threads", "2", "-i", str(path),
         "-map", "0:v:0", "-map", "0:a?", "-f", "null", "-"],
        capture_output=True, text=True, errors="replace", timeout=600,
    )
    match = re.search(r"Duration: (\d+):(\d+):(\d+(?:\.\d+)?)", result.stderr)
    if result.returncode or not match:
        raise ValueError("Recording failed full decode or has no usable duration")
    h, m, s = map(float, match.groups())
    return {"duration_seconds": h * 3600 + m * 60 + s,
            "contains_audio": "Audio:" in result.stderr,
            "full_decode": "PASS"}


def git_context():
    def git(*args):
        r = subprocess.run(["git", "-C", str(REPO), *args], capture_output=True,
                           text=True, timeout=10)
        return r.stdout.strip() if r.returncode == 0 else None
    remote = git("remote", "get-url", "origin")
    if remote and ("@" in remote.split("://")[-1] or SECRET.search(remote)):
        remote = None  # never include authenticated remote URLs
    return {"git_commit": git("rev-parse", "HEAD"),
            "git_branch": git("branch", "--show-current"),
            "dirty_worktree": bool(git("status", "--porcelain")), "repository": remote}


class Capture:
    def __init__(self, config_path):
        config_path = Path(config_path).resolve()
        self.cfg = json.loads(config_path.read_text(encoding="utf-8-sig"))
        for key in ("CONTENT_NAS_PATH", "DEVELOPMENT_MACHINE_ID", "CURRENT_SESSION",
                    "CURRENT_DEVBLOG", "CURRENT_GAME_BUILD", "CURRENT_FEATURE", "FFMPEG_PATH"):
            if key in os.environ:
                self.cfg[key] = os.environ[key]
        self.machine = self.cfg["DEVELOPMENT_MACHINE_ID"]
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}", self.machine):
            raise ValueError("Use a unique, safe DEVELOPMENT_MACHINE_ID")
        local = Path(self.cfg["CONTENT_LOCAL_PATH"])
        self.root = (local if local.is_absolute() else config_path.parent / local).absolute()
        ensure_local(self.root)
        self.outbox = self.root / "content-outbox"
        self.states = self.root / "state"
        self.buffer = self.root / "buffer"
        for p in (self.outbox, self.states, self.buffer):
            p.mkdir(parents=True, exist_ok=True)
        self.ffmpeg = self.cfg.get("FFMPEG_PATH", "ffmpeg")
        segment = float(self.cfg.get("SEGMENT_SECONDS", 30))
        needed = float(self.cfg.get("PRE_EVENT_BUFFER", 60)) + float(self.cfg.get("POST_EVENT_BUFFER", 30)) + 2 * segment
        if not 10 <= segment <= 120 or float(self.cfg.get("BUFFER_SECONDS", 180)) < needed:
            raise ValueError("Buffer must retain pre/post footage plus two native segments")

    def event(self, category, feature, description, **extra):
        check_text(description)
        event = {"timestamp": now(), "machine": self.machine, "category": category,
                 "feature": feature, "description": description,
                 "devblog": self.cfg.get("CURRENT_DEVBLOG"), **git_context(), **extra}
        check_text(json.dumps(event))
        with lock(self.root / "events.lock"):
            with (self.root / "development-events.jsonl").open("a", encoding="utf-8") as f:
                f.write(json.dumps(event, ensure_ascii=False, allow_nan=False) + "\n")
                f.flush()
                os.fsync(f.fileno())
        return event

    def state(self, clip_id, **values):
        path = self.states / (clip_id + ".json")
        value = json.loads(path.read_text()) if path.exists() else {}
        value.update(values)
        value["updated_at"] = now()
        atomic_json(path, value)
        return value

    def package(self, video, metadata, reviewed=False, supporting=()):
        if not self.cfg.get("CONTENT_CAPTURE_ENABLED", True):
            raise ValueError("Content capture is disabled")
        # Metadata cannot supply identifiers, hashes or claim verified media/transfer.
        reserved = {"clip_id", "source_machine", "sha256", "duration_seconds", "full_decode",
                    "schema_version", "project", "git_commit", "git_branch", "dirty_worktree", "repository",
                    "contains_sensitive_information", "requires_manual_review", "contains_audio"}
        if reserved & metadata.keys():
            raise ValueError("Metadata supplies reserved traceability fields")
        if not metadata.get("description", "").strip():
            raise ValueError("Describe what happened and why it matters")
        event_type = metadata.get("event_type", "GENERAL_PROGRESS").upper()
        importance = metadata.get("importance", "USEFUL").upper()
        if event_type not in EVENT_TYPES or importance not in IMPORTANCE:
            raise ValueError("Unknown event type or importance")
        recorded = datetime.fromisoformat(metadata["recorded_at"])
        if recorded.tzinfo is None:
            raise ValueError("recorded_at requires a timezone")
        if Path(video).suffix.lower() != ".mp4":
            raise ValueError("Supply a finalized MP4 recording")
        check_text(json.dumps(metadata))
        clip_id = self.machine + "-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex
        work = self.outbox / (clip_id + ".building")
        package = self.outbox / clip_id
        work.mkdir()
        try:
            copy_closed(Path(video), work / "clip.mp4")
            media = validate_video(work / "clip.mp4", self.ffmpeg)
            video_hash = digest(work / "clip.mp4")
            # Never resubmit identical media automatically; metadata updates stay local.
            for existing in self.outbox.glob("*/metadata.json"):
                if existing.parent == work or existing.parent.name.endswith(".building"):
                    continue
                old = json.loads(existing.read_text(encoding="utf-8"))
                if old.get("sha256") == video_hash:
                    self.event("DISCOVERED", metadata.get("feature", "content-capture"),
                               "Duplicate source clip retained in its existing package.", clip_id=old["clip_id"])
                    return existing.parent
            meta = {"schema_version": 1, "clip_id": clip_id, "project": "destiny1-preservation",
                    "source_machine": self.machine,
                    "development_session": self.cfg.get("CURRENT_SESSION"),
                    "devblog": self.cfg.get("CURRENT_DEVBLOG"),
                    "game_build": self.cfg.get("CURRENT_GAME_BUILD", "01.24"),
                    "feature": self.cfg.get("CURRENT_FEATURE"), "topic": self.cfg.get("CURRENT_SESSION"),
                    "previous_state": "", "current_state": "", "remaining_issue": "",
                    "suggested_use": ["youtube-devblog", "tiktok"], "contains_voice": False,
                    "contains_debug_information": True,
                    **metadata, **git_context(), **media,
                    "event_type": event_type, "importance": importance, "sha256": video_hash,
                    "contains_sensitive_information": not reviewed,
                    "requires_manual_review": not reviewed}
            check_text(json.dumps(meta))
            if len(supporting) > 20:
                raise ValueError("Select relevant support files, not entire logs")
            for index, source in enumerate(supporting):
                source = Path(source)
                suffix = source.suffix.lower()
                if suffix not in {".txt", ".md", ".json", ".jsonl", ".log", ".png", ".jpg"}:
                    raise ValueError("Unsupported support file type")
                dest = work / "support" / f"{index:02d}{suffix}"
                dest.parent.mkdir(exist_ok=True)
                copy_closed(source, dest, 20 * 1024**2)
                if suffix not in {".png", ".jpg"}:
                    check_text(dest.read_text(encoding="utf-8-sig", errors="strict"))
            atomic_json(work / "metadata.json", meta)
            event = {"timestamp": now(), "machine": self.machine, "build": meta["game_build"],
                     "git_commit": meta["git_commit"], "feature": meta["feature"],
                     "event_type": event_type, "importance": importance,
                     "previous_state": meta["previous_state"], "new_state": meta["current_state"],
                     "description": meta["description"], "associated_clip_ids": [clip_id]}
            atomic_json(work / "development-event.json", event)
            # Candidates are frames from the source, without titles or artwork.
            for index, fraction in enumerate((0.25, 0.65), 1):
                result = subprocess.run([self.ffmpeg, "-v", "error", "-nostdin", "-ss",
                                         str(media["duration_seconds"] * fraction), "-i", str(work / "clip.mp4"),
                                         "-frames:v", "1", "-threads", "1",
                                         str(work / f"screenshot-{index:02d}.png")],
                                        capture_output=True, timeout=30)
                if result.returncode:
                    (work / f"screenshot-{index:02d}.png").unlink(missing_ok=True)
            (work / "notes.md").write_text(
                f"{meta['description']}\n\nPrevious: {meta['previous_state']}\n\n"
                f"Current: {meta['current_state']}\n\nRemaining: {meta['remaining_issue']}\n",
                encoding="utf-8")
            self.make_hashes(work)
            work.rename(package)
            self.state(clip_id, status="WAITING_FOR_NAS" if reviewed else "HOLD_FOR_REVIEW",
                       attempts=0, next_attempt=0, destination=None)
            self.event("ADDED", meta["feature"], meta["description"], clip_id=clip_id,
                       importance=importance, reference=meta["git_commit"], build=meta["game_build"],
                       event_type=event_type, previous_state=meta["previous_state"],
                       new_state=meta["current_state"], associated_clip_ids=[clip_id])
            self.status()
            return package
        finally:
            if work.exists():
                # Only this invocation's exact local staging directory is removed.
                shutil.rmtree(work)

    @staticmethod
    def make_hashes(package):
        files = {}
        for path in sorted(package.rglob("*")):
            if path.is_file() and path.name != "hashes.json":
                files[path.relative_to(package).as_posix()] = {"size": path.stat().st_size, "sha256": digest(path)}
        atomic_json(package / "hashes.json", {"schema_version": 1, "files": files})

    @staticmethod
    def verify(package, expected=None):
        if package.is_symlink():
            raise ValueError("Symlink packages are not supported")
        manifest_path = package / "hashes.json"
        if manifest_path.is_symlink():
            raise ValueError("Symlink hash manifests are not supported")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if expected is not None and manifest != expected:
            raise ValueError("Destination hash manifest differs from the source")
        if not {"clip.mp4", "metadata.json", "notes.md"} <= manifest["files"].keys():
            raise ValueError("Required package files are missing")
        for relative, info in manifest["files"].items():
            if "\\" in relative or ":" in relative or relative.startswith("/") or ".." in Path(relative).parts:
                raise ValueError("Unsafe package-relative path")
            path = package / relative
            if any(p.is_symlink() for p in [path, *path.parents][:len(Path(relative).parts)]):
                raise ValueError("Symlinks are not supported")
            if not path.is_file() or path.stat().st_size != info["size"] or digest(path) != info["sha256"]:
                raise ValueError("Package size or SHA-256 verification failed: " + relative)
        actual = {p.relative_to(package).as_posix() for p in package.rglob("*") if p.is_file()}
        if actual != set(manifest["files"]) | {"hashes.json"}:
            raise ValueError("Package contains unexpected files")
        meta = json.loads((package / "metadata.json").read_text(encoding="utf-8"))
        if meta["sha256"] != manifest["files"]["clip.mp4"]["sha256"]:
            raise ValueError("Clip hash and metadata disagree")
        return manifest

    def review(self, clip_id):
        with lock(self.root / "sync.lock"):
            package = self.safe_package(clip_id)
            self.verify(package)
            state = self.state(clip_id)
            if state["status"] == "SUBMITTED":
                raise ValueError("Submitted packages are immutable")
            meta = json.loads((package / "metadata.json").read_text())
            meta.update(contains_sensitive_information=False, requires_manual_review=False)
            check_text(json.dumps(meta))
            for source in (package / "support").glob("*"):
                if source.suffix not in {".png", ".jpg"}:
                    check_text(source.read_text(encoding="utf-8-sig"))
            atomic_json(package / "metadata.json", meta)
            self.make_hashes(package)
            self.state(clip_id, status="WAITING_FOR_NAS", next_attempt=0)

    def safe_package(self, clip_id):
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,150}", clip_id):
            raise ValueError("Unsafe clip ID")
        package = self.outbox / clip_id
        if not package.is_dir() or package.is_symlink():
            raise ValueError("Unknown local package")
        return package

    def sync(self, force=False):
        results = []
        with lock(self.root / "sync.lock"):
            for state_path in sorted(self.states.glob("*.json")):
                state = json.loads(state_path.read_text())
                if state.get("status") not in {"WAITING_FOR_NAS", "FAILED_RETRYING", "TRANSFERRING", "VERIFYING"}:
                    continue
                if not force and state.get("next_attempt", 0) > time.time():
                    continue
                clip_id = state_path.stem
                try:
                    self.state(clip_id, status="TRANSFERRING", transfer_started_at=now())
                    dest = self.transfer(self.safe_package(clip_id))
                    self.state(clip_id, status="SUBMITTED", destination=str(dest),
                               submitted_at=now(), error=None, next_attempt=0)
                    results.append({"clip_id": clip_id, "status": "SUBMITTED", "destination": str(dest)})
                except (OSError, ValueError, KeyError) as error:
                    attempts = state.get("attempts", 0) + 1
                    delay = min(float(self.cfg.get("RETRY_MAX_SECONDS", 3600)),
                                float(self.cfg.get("RETRY_INITIAL_SECONDS", 30)) * 2**min(attempts - 1, 20))
                    # Keep error details local. Never leak local credentials into packages.
                    self.state(clip_id, status="FAILED_RETRYING", attempts=attempts, error=str(error), next_attempt=time.time() + delay)
                    results.append({"clip_id": clip_id, "status": "WAITING_FOR_NAS", "retry_in_seconds": delay})
        if results:
            self.status(NAS_CONNECTED=all(r["status"] == "SUBMITTED" for r in results))
        else:
            self.status()
        return results

    def transfer(self, package):
        manifest = self.verify(package)
        meta = json.loads((package / "metadata.json").read_text())
        if meta["source_machine"] != self.machine or meta["clip_id"] != package.name:
            raise ValueError("Package identity does not match this machine")
        uncertain = meta.get("requires_manual_review") or meta.get("contains_sensitive_information")
        auto_security = meta.get("automatic_security_check", {})
        native_review = (auto_security.get("target") == "owned RPCS3 native game and audio only"
                         and auto_security.get("text_scan") == "PASS"
                         and auto_security.get("publication_review_required") is True)
        if uncertain and not native_review:
            raise ValueError("Review footage and support files before NAS submission")
        check_text(json.dumps(meta))
        for source in (package / "support").glob("*"):
            if source.suffix not in {".png", ".jpg"}:
                check_text(source.read_text(encoding="utf-8-sig"))
        nas = self.cfg.get("CONTENT_NAS_PATH", "")
        if not nas:
            raise OSError("CONTENT_NAS_PATH is not configured")
        nas = Path(nas)
        if not nas.is_absolute() or not nas.is_dir():
            raise OSError("Configured NAS root is unavailable")
        recorded = datetime.fromisoformat(meta["recorded_at"])
        parent = nas / "incoming" / self.machine / recorded.strftime("%Y/%m/%d")
        parent.mkdir(parents=True, exist_ok=True)
        ready = parent / (package.name + ".ready")
        if ready.exists():
            self.verify(ready, manifest)
            return ready  # crash after rename, before local receipt: idempotent recovery
        staging = parent / (package.name + "." + uuid.uuid4().hex + ".uploading")
        staging.mkdir()  # unique, no overwriting another worker's partial transfer
        # On failure, leave uploading evidence ignored by the receiver; retry uses a new name.
        for relative in [*manifest["files"], "hashes.json"]:
            target = staging / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            with (package / relative).open("rb") as source, target.open("xb") as out:
                shutil.copyfileobj(source, out, 1024 * 1024)
                out.flush()
                os.fsync(out.fileno())
        self.state(package.name, status="VERIFYING")
        self.verify(staging, manifest)
        # Same-parent directory rename is the sole READY transition, with no partial READY marker.
        if ready.exists():
            self.verify(ready, manifest)
        else:
            staging.rename(ready)
        return ready

    def status(self, **changes):
        path = self.root / "machine-status.json"
        old = json.loads(path.read_text()) if path.exists() else {}
        states = [json.loads(p.read_text()) for p in self.states.glob("*.json")]
        submitted = [s["submitted_at"] for s in states if s.get("submitted_at")]
        value = {**old, "CONTENT_CAPTURE_ENABLED": self.cfg.get("CONTENT_CAPTURE_ENABLED", True),
                 "DEVELOPMENT_MACHINE_ID": self.machine,
                 "LOCAL_OUTBOX_COUNT": sum(s.get("status") in {"WAITING_FOR_NAS", "FAILED_RETRYING", "TRANSFERRING", "VERIFYING"} for s in states),
                 "LOCAL_REVIEW_COUNT": sum(s.get("status") == "HOLD_FOR_REVIEW" for s in states),
                 "LAST_SUCCESSFUL_TRANSFER": max(submitted) if submitted else None,
                 "CURRENT_DEVBLOG": self.cfg.get("CURRENT_DEVBLOG"),
                 "CURRENT_SESSION": self.cfg.get("CURRENT_SESSION"),
                 "AVAILABLE_BUFFER_SPACE": shutil.disk_usage(self.root).free,
            "NAS_CONNECTED": old.get("NAS_CONNECTED"), "updated_at": now(), **changes}
        atomic_json(path, value)
        return value

    def add_buffer(self, video, recorded_at):
        """Index a finalized native recorder segment, never the active recording."""
        with lock(self.root / "buffer.lock"):
            path = self.buffer / (uuid.uuid4().hex + ".mp4")
            copy_closed(Path(video), path, self.cfg.get("BUFFER_MAX_BYTES", 2 * 1024**3))
            try:
                media = validate_video(path, self.ffmpeg)
                recorded = datetime.fromisoformat(recorded_at)
                if recorded.tzinfo is None:
                    raise ValueError("Buffer timestamp requires a timezone")
                info = {"file": path.name, "recorded_at": recorded.isoformat(), **media}
                atomic_json(path.with_suffix(".json"), info)
                self.trim_buffer()
                return info
            except Exception:
                path.unlink(missing_ok=True)
                raise

    def trim_buffer(self):
        entries = sorted(self.buffer.glob("*.json"), key=lambda p: json.loads(p.read_text())["recorded_at"], reverse=True)
        seconds = 0
        size = 0
        limit = float(self.cfg.get("BUFFER_SECONDS", 180))
        byte_limit = int(self.cfg.get("BUFFER_MAX_BYTES", 2 * 1024**3))
        for entry in entries:
            info = json.loads(entry.read_text())
            path = self.buffer / info["file"]
            if seconds + info["duration_seconds"] > limit or size + path.stat().st_size > byte_limit:
                path.unlink()
                entry.unlink()
            else:
                seconds += info["duration_seconds"]
                size += path.stat().st_size

    def capture_buffer(self, metadata, seconds=60, reviewed=False):
        with lock(self.root / "buffer.lock"):
            selected = []
            total = 0
            for entry in sorted(self.buffer.glob("*.json"), reverse=True,
                                key=lambda p: json.loads(p.read_text())["recorded_at"]):
                info = json.loads(entry.read_text())
                selected.append(info)
                total += info["duration_seconds"]
                if total >= seconds:
                    break
            if not selected:
                raise ValueError("No finalized buffer segments are available")
            selected.reverse()
            # A source capture contains complete closed segments, not invented transition footage.
            metadata = {**metadata, "recorded_at": selected[0]["recorded_at"],
                        "buffer_segments": selected, "buffer_requested_seconds": seconds,
                        "buffer_available_seconds": total}
            token = uuid.uuid4().hex
            playlist = self.buffer / (token + ".txt")
            output = self.buffer / (token + "-capture.mp4")
            try:
                playlist.write_text("".join("file '" + s["file"] + "'\n" for s in selected), encoding="utf-8")
                result = subprocess.run([self.ffmpeg, "-v", "error", "-nostdin", "-f", "concat",
                                         "-safe", "1", "-i", str(playlist), "-c", "copy",
                                         "-movflags", "+faststart", str(output)], capture_output=True, timeout=120)
                if result.returncode:
                    raise ValueError("Buffer remux failed; original segments retained")
                return self.package(output, metadata, reviewed=reviewed)
            finally:
                playlist.unlink(missing_ok=True)
                output.unlink(missing_ok=True)

    def ingest_events(self, events_file):
        """Explicit opt-in finished-clip events; routine logs never generate clips."""
        with lock(self.root / "ingest.lock"):
            receipt = self.root / "ingested-events.json"
            seen = json.loads(receipt.read_text()) if receipt.exists() else {}
            for line in Path(events_file).read_text(encoding="utf-8-sig").splitlines():
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue  # an append may currently be incomplete
                if event.get("content_candidate") is not True or not event.get("event_id"):
                    continue
                key = str(event["event_id"])
                if key in seen:
                    continue
                try:
                    result = self.package(event["clip_path"], event["metadata"],
                                          reviewed=event.get("sensitivity_reviewed") is True)
                    seen[key] = {"package": result.name, "at": now()}
                    atomic_json(receipt, seen)
                except (OSError, ValueError, KeyError):
                    continue  # keep the event pending; development remains independent
            return seen

    def prune_submitted(self):
        """Explicit local retention command; always reverify the NAS copy first."""
        removed = []
        with lock(self.root / "sync.lock"):
            for path in self.states.glob("*.json"):
                state = json.loads(path.read_text())
                if state.get("status") != "SUBMITTED":
                    continue
                age = datetime.now(timezone.utc) - datetime.fromisoformat(state["submitted_at"])
                if age.total_seconds() < float(self.cfg.get("SUBMITTED_RETENTION_DAYS", 30)) * 86400:
                    continue
                package = self.safe_package(path.stem)
                manifest = self.verify(package)
                self.verify(Path(state["destination"]), manifest)
                shutil.rmtree(package)
                self.state(path.stem, status="RETAINED_ON_NAS")
                removed.append(path.stem)
        return removed


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--config", required=True, help="Private JSON config outside source control")
    sub = p.add_subparsers(dest="command", required=True)
    cap = sub.add_parser("capture", help="Package a closed native MP4 or recent closed buffer segments")
    cap.add_argument("event_type", choices=sorted(EVENT_TYPES), type=str.upper)
    cap.add_argument("description")
    src = cap.add_mutually_exclusive_group(required=True)
    src.add_argument("--video", type=Path)
    src.add_argument("--buffer", action="store_true")
    cap.add_argument("--seconds", type=float, default=60)
    cap.add_argument("--recorded-at")
    cap.add_argument("--metadata", type=Path, help="Additional context JSON (no secret values)")
    cap.add_argument("--support", type=Path, action="append", default=[])
    cap.add_argument("--reviewed", action="store_true", help="Footage AND supporting files inspected for sensitive information")
    cap.add_argument("--importance", type=str.upper, choices=sorted(IMPORTANCE), default="USEFUL")
    syn = sub.add_parser("sync", help="Transfer outside critical test windows")
    syn.add_argument("--force", action="store_true", help="Ignore retry delay, preserve verification")
    syn.add_argument("--watch", action="store_true")
    syn.add_argument("--events", type=Path, help="Optional explicit finished-clip events JSONL")
    rev = sub.add_parser("review", help="Mark a locally inspected package as safe to submit")
    rev.add_argument("clip_id")
    buf = sub.add_parser("buffer-add", help="Add a finalized native recording segment to the bounded local buffer")
    buf.add_argument("video", type=Path)
    buf.add_argument("--recorded-at", required=True)
    ev = sub.add_parser("event", help="Record a structured development changelog event")
    ev.add_argument("category", choices=["ADDED", "FIXED", "IMPROVED", "DISCOVERED", "REGRESSED", "IN_PROGRESS", "BLOCKED", "NEXT"])
    ev.add_argument("feature")
    ev.add_argument("description")
    sub.add_parser("status")
    sub.add_parser("prune-submitted")
    args = p.parse_args(argv)
    capture = Capture(args.config)
    if args.command == "capture":
        metadata = json.loads(args.metadata.read_text(encoding="utf-8-sig")) if args.metadata else {}
        metadata.update(description=args.description, event_type=args.event_type, importance=args.importance)
        if args.video:
            if not args.recorded_at:
                p.error("--recorded-at is required for a source file; do not guess its timestamp")
            metadata["recorded_at"] = args.recorded_at
            result = str(capture.package(args.video, metadata, args.reviewed, args.support))
        else:
            if args.support:
                p.error("Buffer captures do not accept support files; package a selected closed clip instead")
            result = str(capture.capture_buffer(metadata, args.seconds, args.reviewed))
    elif args.command == "sync":
        while True:
            if args.events:
                capture.ingest_events(args.events)
            result = capture.sync(force=args.force)
            print(json.dumps(result), flush=True)
            if not args.watch:
                return
            args.force = False
            time.sleep(5)
    elif args.command == "review":
        capture.review(args.clip_id)
        result = capture.status()
    elif args.command == "buffer-add":
        result = capture.add_buffer(args.video, args.recorded_at)
    elif args.command == "event":
        result = capture.event(args.category, args.feature, args.description)
    elif args.command == "prune-submitted":
        result = capture.prune_submitted()
    else:
        result = capture.status()
    print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
