"""Render captioned devlogs from recorded gameplay, with footage in every section.

Requires Pillow and FFmpeg (--ffmpeg or imageio-ffmpeg). Longer edits use Windows
speech or a text-matched narration cache. All media and render outputs stay private.
"""
import argparse
import json
import math
import re
import shutil
import subprocess
import wave
from pathlib import Path


def positive(value, label):
    value = float(value)
    if not math.isfinite(value) or value <= 0:
        raise ValueError(f"{label} must be positive and finite")
    return value


def media_path(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise ValueError(f"Footage must be an existing file within --footage: {name}")
    return path


def probe(ffmpeg, path):
    result = subprocess.run([ffmpeg, "-hide_banner", "-i", str(path)],
                            capture_output=True, text=True, errors="replace")
    match = re.search(r"Duration: (\d+):(\d+):(\d+(?:\.\d+)?)", result.stderr)
    if not match or not re.search(r"Stream #.*Video:", result.stderr):
        raise ValueError(f"Cannot read video duration/stream: {path.name}")
    h, m, s = map(float, match.groups())
    return {"duration": h * 3600 + m * 60 + s,
            "audio": bool(re.search(r"Stream #.*Audio:", result.stderr))}


def footage_spec(scene, root, ffmpeg):
    spec = scene.get("footage")
    if not isinstance(spec, dict) or not spec.get("file"):
        raise ValueError(f"Every section needs recorded footage: {scene['title']}")
    path = media_path(root, spec["file"])
    info = probe(ffmpeg, path)
    start = float(spec.get("start", 0))
    duration = positive(spec["duration"], "Footage duration")
    if not math.isfinite(start) or start < 0 or start + duration > info["duration"] + .03:
        raise ValueError(f"Footage trim exceeds its source: {path.name}")
    if type(spec.get("loop", False)) is not bool:
        raise ValueError("Footage loop must be an explicit Boolean")
    return {**spec, "path": path, "start": start, "duration": duration,
            "audio": info["audio"], "source_duration": info["duration"]}


def wrapped(draw, text, font, width):
    lines, line = [], ""
    for word in text.split():
        candidate = f"{line} {word}".strip()
        if draw.textlength(candidate, font=font) > width and line:
            lines.append(line)
            line = word
        else:
            line = candidate
    return lines + ([line] if line else [])


def text_block(draw, text, xy, font, width, max_lines, color):
    lines = wrapped(draw, text, font, width)
    if len(lines) > max_lines:
        raise ValueError(f"Text exceeds {max_lines} lines: {text}")
    draw.multiline_text(xy, "\n".join(lines), font=font, fill=color, spacing=12)


def overlay(scene, caption, mode, episode, fonts):
    from PIL import Image, ImageDraw

    short = mode == "short"
    w, h = (1080, 1920) if short else (1920, 1080)
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    accent, white, muted = "#68e5eb", "#f3f7fa", "#b9c6d3"
    panel = (9, 19, 30, 235)
    if short:
        draw.rectangle((0, 0, w, 470), fill=panel)
        draw.rectangle((0, 1220, w, h), fill=panel)
        draw.rectangle((60, 92, 188, 100), fill=accent)
        draw.text((60, 130), f"DESTINY 1 / DEVLOG {episode['number']}", font=fonts[30], fill=muted)
        text_block(draw, scene["title"], (60, 205), fonts[64], 960, 3, white)
        draw.text((60, 434), scene["status"].upper(), font=fonts[30], fill=accent)
        text_block(draw, caption, (60, 1250), fonts[48], 940, 6, white)
        draw.text((60, 1650), "RECORDED GAMEPLAY / PS3 01.24", font=fonts[30], fill=muted)
        draw.text((60, 1710), "Edited local test footage", font=fonts[30], fill=muted)
        draw.line((60, 1780, 1020, 1780), fill=accent, width=3)
        draw.text((60, 1820), episode["date"], font=fonts[30], fill=muted)
    else:
        draw.rectangle((0, 0, w, 178), fill=panel)
        draw.rectangle((0, 827, w, h), fill=panel)
        draw.text((70, 27), f"DESTINY 1 / DEVLOG {episode['number']} / PS3 01.24", font=fonts[30], fill=accent)
        text_block(draw, scene["title"], (70, 76), fonts[48], 1760, 1, white)
        draw.text((70, 846), scene["status"].upper(), font=fonts[30], fill=accent)
        text_block(draw, caption, (70, 901), fonts[48], 1750, 2, white)
        draw.text((70, 1030), "EDITED TEST FOOTAGE / SYNTHETIC DRAFT NARRATION", font=fonts[24], fill=muted)
    return canvas


def srt_time(seconds):
    ms = round(seconds * 1000)
    hours, ms = divmod(ms, 3600000)
    minutes, ms = divmod(ms, 60000)
    secs, ms = divmod(ms, 1000)
    return f"{hours:02}:{minutes:02}:{secs:02},{ms:03}"


def concat_line(path):
    return "file '" + str(path.resolve()).replace("\\", "/").replace("'", "'\\''") + "'\n"


def run(ffmpeg, args):
    subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "warning", "-n", *args], check=True)


def main():
    from PIL import ImageFont

    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("manifest", type=Path)
    p.add_argument("--footage", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--mode", choices=("short", "long"), required=True)
    p.add_argument("--ffmpeg", type=Path)
    p.add_argument("--font", type=Path, default=Path("C:/Windows/Fonts/segoeuib.ttf"))
    p.add_argument("--voice", default="Microsoft Hazel Desktop")
    p.add_argument("--narration-cache", type=Path, help="Reuse WAVs only when their job text matches")
    args = p.parse_args()
    out = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if out.is_relative_to(repo) and not out.is_relative_to(repo / "local"):
        p.error("Render private media outside tracked source or under ignored local/.")
    if out.exists():
        p.error("Output directory must be new")
    ffmpeg = str(args.ffmpeg) if args.ffmpeg else None
    if ffmpeg is None:
        import imageio_ffmpeg
        ffmpeg = imageio_ffmpeg.get_ffmpeg_exe()
    episode = json.loads(args.manifest.read_text(encoding="utf-8"))
    scenes = episode[args.mode]
    if not scenes:
        p.error("Episode must contain sections")
    sources = [footage_spec(scene, args.footage, ffmpeg) for scene in scenes]
    out.mkdir(parents=True, exist_ok=False)
    fonts = {size: ImageFont.truetype(str(args.font), size) for size in (24, 30, 48, 64)}
    parts, jobs, cached = [], [], {}
    if args.narration_cache:
        cache_root = args.narration_cache.resolve()
        cache_report = json.loads((cache_root / "render.json").read_text(encoding="utf-8"))
        if cache_report.get("narration") != args.voice:
            raise ValueError("Narration cache voice does not match --voice")
        for job in json.loads((cache_root / "narration-jobs.json").read_text(encoding="utf-8")):
            path = Path(job["destination"]).resolve()
            if not path.is_relative_to(cache_root) or not path.is_file():
                raise ValueError("Narration cache WAV must stay within its directory")
            cached[job["text"]] = path
    for scene_index, scene in enumerate(scenes):
        captions = scene["captions"] if args.mode == "long" else [scene["caption"]]
        if not captions:
            raise ValueError("Section must contain captions")
        for caption in captions:
            index = len(parts)
            frame, audio = out / f"overlay-{index:03}.png", out / f"voice-{index:03}.wav"
            overlay(scene, caption, args.mode, episode, fonts).save(frame)
            parts.append({"overlay": frame, "audio": audio, "caption": caption,
                          "duration": scene.get("duration"), "scene": scene_index})
            jobs.append({"text": caption, "destination": str(audio)})
            if args.mode == "long" and caption in cached:
                shutil.copyfile(cached[caption], audio)
    if args.mode == "long":
        (out / "narration-jobs.json").write_text(json.dumps(jobs, ensure_ascii=False), encoding="utf-8")
        pending = [job for job in jobs if not Path(job["destination"]).exists()]
        if pending:
            job_path = out / "narration-pending.json"
            job_path.write_text(json.dumps(pending, ensure_ascii=False), encoding="utf-8")
            subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                            str(Path(__file__).with_name("Render-DevlogNarration.ps1")),
                            "-JobsFile", str(job_path), "-Voice", args.voice], check=True)
        for part in parts:
            with wave.open(str(part["audio"]), "rb") as audio:
                part["duration"] = audio.getnframes() / audio.getframerate()
    for part in parts:
        part["duration"] = math.ceil(positive(part["duration"], "Segment duration") * 24) / 24
    prepared = []
    for i, source in enumerate(sources):
        needed = sum(part["duration"] for part in parts if part["scene"] == i)
        if needed > source["duration"] + .03 and not source.get("loop", False):
            raise ValueError(f"Section needs {needed:.2f}s; supply more footage or explicitly allow loop: {scenes[i]['title']}")
        # PCM intermediates avoid AAC encoder delay at every loop and edit boundary.
        target = out / f"source-{i:02}.mov"
        command = ["-ss", str(source["start"]), "-i", str(source["path"])]
        if not source["audio"]:
            command += ["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]
        command += ["-t", str(source["duration"]), "-map", "0:v:0", "-map", "0:a:0" if source["audio"] else "1:a:0",
                    "-r", "24", "-fps_mode", "cfr", "-c:v", "libx264", "-preset", "veryfast", "-crf", "20",
                    "-pix_fmt", "yuv420p", "-c:a", "pcm_s16le", "-ar", "48000", "-ac", "2", "-threads", "2", str(target)]
        run(ffmpeg, command)
        prepared.append(target)
    videos, captions, timeline = "", [], []
    cursor, scene_cursors = 0.0, [0.0] * len(scenes)
    for i, part in enumerate(parts):
        si = part["scene"]
        source, local, duration = sources[si], scene_cursors[si], part["duration"]
        segment = out / f"segment-{i:03}.mov"
        offset = local % source["duration"]
        command = ["-stream_loop", "-1", "-i", str(prepared[si]),
                   "-loop", "1", "-i", str(part["overlay"])]
        # Seeking a looped input can reset audio timestamps at the first wrap. Decode
        # from the selected window's beginning, establish monotonic sample/frame clocks,
        # then trim. Both streams begin at zero and have the same measured duration.
        clocks = (f"[0:v]setpts=N/(24*TB),trim=start={offset}:duration={duration},setpts=PTS-STARTPTS[clip];"
                  f"[0:a]asetpts=N/SR/TB,atrim=start={offset}:duration={duration},asetpts=PTS-STARTPTS[a];")
        if args.mode == "short":
            layout = "[clip]split[bg][fg];[bg]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,setsar=1,boxblur=20:2,eq=brightness=-0.15[back];[fg]scale=1080:608,setsar=1[front];[back][front]overlay=0:540[game]"
        else:
            layout = "[clip]split[bg][fg];[bg]scale=1920:1080,setsar=1,boxblur=20:2,eq=brightness=-0.18[back];[fg]scale=1152:648,setsar=1[front];[back][front]overlay=384:178[game]"
        command += ["-filter_complex", clocks + layout + ";[game][1:v]overlay=0:0:format=auto,format=yuv420p[v]",
                    "-map", "[v]", "-map", "[a]", "-t", str(duration), "-r", "24", "-fps_mode", "cfr",
                    "-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-threads", "2",
                    "-filter_complex_threads", "2", "-c:a", "pcm_s16le", "-ar", "48000", "-ac", "2", str(segment)]
        run(ffmpeg, command)
        videos += concat_line(segment)
        captions.append(f"{i + 1}\n{srt_time(cursor)} --> {srt_time(cursor + duration)}\n{part['caption']}\n")
        timeline.append({"start": cursor, "duration": duration, "title": scenes[si]["title"],
                         "file": source["file"], "source_start": source["start"] + local % source["duration"],
                         "source_window": [source["start"], source["start"] + source["duration"]],
                         "repeats_b_roll": local + duration > source["duration"], "caption": part["caption"]})
        scene_cursors[si] += duration
        cursor += duration
        print(f"Rendered {i + 1}/{len(parts)}: {scenes[si]['title']}", flush=True)
    video_list, audio_list = out / "videos.txt", out / "audio.txt"
    video_list.write_text(videos, encoding="utf-8")
    (out / "captions.srt").write_text("\n".join(captions), encoding="utf-8")
    video = out / f"destiny1-devlog-{episode['number']}-{args.mode}.mp4"
    command = ["-safe", "0", "-f", "concat", "-i", str(video_list)]
    if args.mode == "long":
        padded = []
        for i, part in enumerate(parts):
            target = out / f"voice-padded-{i:03}.wav"
            run(ffmpeg, ["-i", str(part["audio"]), "-af", "apad", "-t", str(part["duration"]),
                         "-ar", "48000", "-ac", "2", str(target)])
            padded.append(target)
        audio_list.write_text("".join(concat_line(path) for path in padded), encoding="utf-8")
        command += ["-safe", "0", "-f", "concat", "-i", str(audio_list),
                    "-filter_complex", "[0:a]volume=0.16[game];[1:a]loudnorm=I=-16:TP=-1.5:LRA=11[voice];[game][voice]amix=inputs=2:normalize=0,alimiter=limit=0.95:level=false[a]",
                    "-map", "0:v:0", "-map", "[a]", "-c:a", "aac", "-b:a", "160k", "-ar", "48000"]
    else:
        command += ["-map", "0:v:0", "-map", "0:a:0", "-c:a", "aac", "-b:a", "160k", "-ar", "48000"]
    command += ["-t", str(cursor), "-c:v", "copy", "-movflags", "+faststart", str(video)]
    run(ffmpeg, command)
    report = {"duration_seconds": cursor, "segments": len(parts), "mode": args.mode,
              "dimensions": [1080, 1920] if args.mode == "short" else [1920, 1080],
              "video": str(video), "source_manifest": str(args.manifest.resolve()),
              "presentation": "edited recorded gameplay; repeated b-roll identified in timeline",
              "narration": args.voice if args.mode == "long" else None, "timeline": timeline}
    (out / "render.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in report.items() if k != "timeline"}, indent=2))


if __name__ == "__main__":
    main()
