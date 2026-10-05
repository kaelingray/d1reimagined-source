"""Render captioned review videos from an evidence manifest and private test captures.

Requires Pillow and an FFmpeg executable (--ffmpeg or imageio-ffmpeg). Narration
uses optional Windows desktop TTS, one caption per WAV for exact segment timing.
No capture, voice, video or private path is added to source control.
"""
import argparse
import json
import math
import subprocess
import wave
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageOps


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


def card(scene, caption, capture, mode, episode, fonts):
    short = mode == "short"
    w, h = (1080, 1920) if short else (1920, 1080)
    canvas = Image.new("RGB", (w, h), "#09131e")
    if capture is not None:
        area = (960, 680) if short else (w, h)
        source = ImageOps.contain(capture, area, Image.Resampling.LANCZOS)
        x = (w - source.width) // 2
        y = 490 + (680 - source.height) // 2 if short else (h - source.height) // 2
        canvas.paste(source, (x, y))
    draw = ImageDraw.Draw(canvas)
    accent, white, muted = "#68e5eb", "#f3f7fa", "#b9c6d3"
    if short:
        draw.rectangle((60, 92, 188, 100), fill=accent)
        draw.text((60, 130), f"DESTINY 1 / DEVLOG {episode['number']}", font=fonts[30], fill=muted)
        text_block(draw, scene["title"], (60, 205), fonts[64], 960, 3, white)
        draw.text((60, 434), scene["status"].upper(), font=fonts[30], fill=accent)
        text_block(draw, caption, (60, 1230), fonts[48], 940, 6, white)
        draw.text((60, 1630), "LOCAL TEST CAPTURES / PS3 01.24", font=fonts[30], fill=muted)
        draw.text((60, 1690), "Experimental preservation build", font=fonts[30], fill=muted)
        draw.line((60, 1770, 1020, 1770), fill=accent, width=3)
        draw.text((60, 1810), episode["date"], font=fonts[30], fill=muted)
    else:
        draw.rectangle((0, 0, w, 178), fill="#09131e")
        draw.rectangle((0, 827, w, h), fill="#09131e")
        draw.text((70, 27), f"DESTINY 1 / DEVLOG {episode['number']} / PS3 01.24", font=fonts[30], fill=accent)
        text_block(draw, scene["title"], (70, 76), fonts[48], 1760, 1, white)
        draw.text((70, 846), scene["status"].upper(), font=fonts[30], fill=accent)
        text_block(draw, caption, (70, 901), fonts[48], 1750, 2, white)
        draw.text((70, 1030), "TEST CAPTURE MONTAGE / SYNTHETIC DRAFT NARRATION", font=fonts[24], fill=muted)
    return canvas


def srt_time(seconds):
    ms = round(seconds * 1000)
    hours, ms = divmod(ms, 3600000)
    minutes, ms = divmod(ms, 60000)
    secs, ms = divmod(ms, 1000)
    return f"{hours:02}:{minutes:02}:{secs:02},{ms:03}"


def concat_line(path):
    # FFmpeg concat syntax is distinct from shell quoting. subprocess uses argv.
    return "file '" + str(path.resolve()).replace("\\", "/").replace("'", "'\\''") + "'\n"


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("manifest", type=Path)
    p.add_argument("--captures", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--mode", choices=("short", "long"), required=True)
    p.add_argument("--ffmpeg", type=Path)
    p.add_argument("--font", type=Path, default=Path("C:/Windows/Fonts/segoeuib.ttf"))
    p.add_argument("--voice", default="Microsoft Hazel Desktop")
    args = p.parse_args()
    out = args.out.resolve()
    repo = Path(__file__).resolve().parents[1]
    if out.is_relative_to(repo) and not out.is_relative_to(repo / "local"):
        p.error("Render private media outside tracked source or under ignored local/.")
    out.mkdir(parents=True, exist_ok=False)
    episode = json.loads(args.manifest.read_text(encoding="utf-8"))
    fonts = {size: ImageFont.truetype(str(args.font), size) for size in (24, 30, 48, 64)}
    parts, jobs = [], []
    for scene in episode[args.mode]:
        capture = None
        if scene.get("capture"):
            path = (args.captures / scene["capture"]).resolve()
            if not path.is_relative_to(args.captures.resolve()):
                raise ValueError("Capture must stay within its selected directory")
            with Image.open(path) as image:
                capture = image.convert("RGB")
        captions = scene["captions"] if args.mode == "long" else [scene["caption"]]
        for caption in captions:
            index = len(parts)
            frame, audio = out / f"frame-{index:03}.png", out / f"voice-{index:03}.wav"
            card(scene, caption, capture, args.mode, episode, fonts).save(frame)
            parts.append({"frame": frame, "audio": audio, "caption": caption,
                          "duration": scene.get("duration"), "title": scene["title"]})
            jobs.append({"text": caption, "destination": str(audio)})
    if args.mode == "long":
        job_path = out / "narration-jobs.json"
        job_path.write_text(json.dumps(jobs, ensure_ascii=False), encoding="utf-8")
        subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(Path(__file__).with_name(
            "Render-DevlogNarration.ps1")), "-JobsFile", str(job_path), "-Voice", args.voice], check=True)
        for part in parts:
            with wave.open(str(part["audio"]), "rb") as audio:
                part["duration"] = audio.getnframes() / audio.getframerate()
    ffmpeg = str(args.ffmpeg) if args.ffmpeg else None
    if ffmpeg is None:
        import imageio_ffmpeg
        ffmpeg = imageio_ffmpeg.get_ffmpeg_exe()
    frames, audios, captions, cursor = "", "", [], 0.0
    for i, part in enumerate(parts, 1):
        duration = float(part["duration"])
        if not math.isfinite(duration) or duration <= 0:
            raise ValueError("Segment duration must be positive and finite")
        frames += concat_line(part["frame"]) + f"duration {duration:.6f}\n"
        audios += concat_line(part["audio"])
        captions.append(f"{i}\n{srt_time(cursor)} --> {srt_time(cursor + duration)}\n{part['caption']}\n")
        cursor += duration
    frames += concat_line(parts[-1]["frame"])
    frame_list, audio_list = out / "frames.txt", out / "audio.txt"
    frame_list.write_text(frames, encoding="utf-8")
    audio_list.write_text(audios, encoding="utf-8")
    (out / "captions.srt").write_text("\n".join(captions), encoding="utf-8")
    command = [ffmpeg, "-hide_banner", "-loglevel", "warning", "-n", "-safe", "0", "-f", "concat", "-i", str(frame_list)]
    if args.mode == "long":
        command += ["-safe", "0", "-f", "concat", "-i", str(audio_list), "-c:a", "aac", "-b:a", "160k",
                    "-af", "loudnorm=I=-16:TP=-1.5:LRA=11", "-ar", "48000"]
    else:
        command += ["-an"]
    video = out / f"destiny1-devlog-{episode['number']}-{args.mode}.mp4"
    command += ["-t", f"{cursor:.6f}", "-r", "24", "-fps_mode", "cfr", "-c:v", "libx264",
                "-preset", "veryfast", "-crf", "22", "-pix_fmt", "yuv420p", "-threads", "2",
                "-movflags", "+faststart", str(video)]
    subprocess.run(command, check=True)
    report = {"duration_seconds": cursor, "segments": len(parts), "mode": args.mode,
              "dimensions": [1080, 1920] if args.mode == "short" else [1920, 1080],
              "video": str(video), "source_manifest": str(args.manifest.resolve()),
              "presentation": "test capture montage; no continuous gameplay footage",
              "narration": args.voice if args.mode == "long" else None}
    (out / "render.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
