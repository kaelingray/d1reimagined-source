"""Audit the actual recorder header options and interrupted MOV/MP4 recovery.

The C++ fixture extracts the changed native block, including dictionary failure
and cleanup paths. Optional FFmpeg fixtures use only synthetic video/audio and
kill their own retained child process before a trailer can be written.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time


def configuration(source):
    if "AVDictionary* mux_options = nullptr;" not in source:
        if "avformat_write_header(av.format_context, nullptr)" not in source:
            raise ValueError("unsupported baseline recorder header")
        return "int header_error = avformat_write_header(av.format_context, nullptr);", {}
    start = source.index("AVDictionary* mux_options = nullptr;")
    end = source.index("if (int err = header_error;", start)
    actual = source[start:end]
    options = dict(re.findall(r'av_dict_set\(&mux_options, "([^"]+)", "([^"]+)", 0\)', actual))
    return actual, options


def generate(actual):
    return r'''
#include <iostream>
#include <map>
#include <string>
#include <string_view>
using AVDictionary = std::map<std::string, std::string>;
struct Format { const char* name; };
struct Context { const Format* oformat; };
int set_calls=0, free_calls=0, header_calls=0, fail_at=0, header_result=0;
AVDictionary captured;
int av_dict_set(AVDictionary** d,const char* key,const char* value,int) {
    if (++set_calls == fail_at) return -12;
    if (!*d) *d=new AVDictionary;
    (**d)[key]=value; return 0;
}
void av_dict_free(AVDictionary** d) { ++free_calls; delete *d; *d=nullptr; }
int avformat_write_header(Context*,AVDictionary** d) {
    ++header_calls; captured=d && *d ? **d : AVDictionary{}; return header_result;
}
int configure(const char* name,int failure=0,int header_failure=0) {
    set_calls=free_calls=header_calls=0; captured.clear();
    fail_at=failure; header_result=header_failure;
    Format format{name}; Context context{&format};
    struct { Context* format_context; } av{&context};
ACTUAL_CONFIGURATION
    return header_error;
}
int checks=0, failures=0;
void check(const char* label,bool valid) { ++checks; if (!valid) { ++failures; std::cerr<<"FAIL: "<<label<<"\n"; } }
int main() {
    for (auto name : {"mov","mp4"}) {
        check("MOV/MP4 header succeeds", configure(name)==0 && header_calls==1);
        check("interrupted container has initial metadata and bounded fragments",
            captured["movflags"]=="frag_keyframe+empty_moov+default_base_moof" && captured["frag_duration"]=="1000000");
        check("options are freed after header", free_calls==1);
    }
    for (auto name : {"matroska","avi","wav"}) {
        check("other muxer keeps default header", configure(name)==0 && header_calls==1 && captured.empty() && set_calls==0);
    }
    check("first option allocation failure propagates without writing header", configure("mov",1)==-12 && header_calls==0 && free_calls==1);
    check("second option allocation failure releases first option", configure("mp4",2)==-12 && header_calls==0 && free_calls==1);
    check("header failure propagates and releases options", configure("mov",0,-22)==-22 && header_calls==1 && free_calls==1);
    std::cout<<checks<<" recording-header checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
'''.replace("ACTUAL_CONFIGURATION", actual)


def ffmpeg_audit(ffmpeg, output, options):
    # Native recording uses MPEG-4 Part 2/AAC, 30 fps and a one-second GOP.
    flags = subprocess.CREATE_NO_WINDOW if hasattr(subprocess, "CREATE_NO_WINDOW") else 0
    base = [str(ffmpeg), "-hide_banner", "-nostdin", "-loglevel", "error"]
    inputs = ["-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30",
              "-f", "lavfi", "-i", "sine=frequency=400:sample_rate=48000"]
    codecs = ["-c:v", "mpeg4", "-b:v", "1000000", "-g", "30", "-bf", "0",
              "-c:a", "aac", "-b:a", "192000"]
    option_args = [word for key, value in options.items() for word in ("-" + key, value)]

    def run(args):
        return subprocess.run(base + args, capture_output=True, text=True,
                              timeout=30, creationflags=flags)

    def decode(path):
        result = run(["-xerror", "-i", str(path), "-map", "0:v:0", "-map", "0:a:0",
                      "-progress", "pipe:1", "-f", "null", "-"])
        frames = re.findall(r"^frame=(\d+)$", result.stdout, re.M)
        times = re.findall(r"^out_time_us=(\d+)$", result.stdout, re.M)
        return {"exit": result.returncode, "frames": int(frames[-1]) if frames else 0,
                "seconds": int(times[-1]) / 1e6 if times else 0,
                "error": result.stderr[-1500:]}

    results = []
    for format_name in ("mov", "mp4"):
        path = output / ("interrupted." + format_name)
        args = base + ["-re"] + inputs + codecs + option_args + ["-f", format_name, "-y", str(path)]
        with (output / (format_name + "-writer.log")).open("w") as log:
            child = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=log,
                                     creationflags=flags)
            try:
                deadline = time.monotonic() + 6
                while time.monotonic() < deadline:
                    if child.poll() is not None:
                        raise RuntimeError("synthetic writer exited before interruption")
                    time.sleep(.1)
            finally:
                if child.poll() is None:
                    child.kill()  # retained child handle; no trailer or broad process lookup
                child.wait(timeout=5)
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools/content_capture"))
        from fragmented_media import completed_fragment_prefix
        original_bytes = path.stat().st_size
        with path.open("rb") as stream:
            original_hash = hashlib.file_digest(stream, "sha256").hexdigest()
        prefix = completed_fragment_prefix(path)
        recovered = output / ("complete-prefix." + format_name)
        shutil.copyfile(path, recovered)
        if prefix:
            with recovered.open("r+b") as stream:
                stream.truncate(prefix["retained_bytes"])
        with path.open("rb") as stream:
            original_unchanged = hashlib.file_digest(stream, "sha256").hexdigest() == original_hash
        result = {"format": format_name, "prefix_recovery": prefix,
                  "original_unchanged": original_unchanged and path.stat().st_size == original_bytes,
                  "interrupted_decode": decode(recovered)}
        if options:
            remux = output / ("recovered-" + format_name + ".mp4")
            result["remux_exit"] = run(["-i", str(recovered), "-c", "copy", "-movflags", "+faststart", "-y", str(remux)]).returncode
            result["remux_decode"] = decode(remux)
        results.append(result)
    complete = output / "complete.mov"
    normal = run(inputs + codecs + option_args + ["-t", "3", "-f", "mov", "-y", str(complete)])
    results.append({"normal_writer_exit": normal.returncode, "normal_decode": decode(complete)})
    receipt = {"options_from_actual_source": options, "results": results}
    (output / "ffmpeg-recovery-result.json").write_text(json.dumps(receipt, indent=2) + "\n")
    interrupted_ok = all(r["original_unchanged"] and r["interrupted_decode"]["exit"] == 0 and r["interrupted_decode"]["frames"] >= 60
                         and r.get("remux_exit") == 0 and r["remux_decode"]["frames"] >= 60 for r in results[:2])
    normal_ok = normal.returncode == 0 and results[-1]["normal_decode"]["frames"] >= 85
    print(json.dumps(receipt))
    return interrupted_ok and normal_ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--ffmpeg", type=Path)
    args = parser.parse_args()
    source = args.source / ("media_utils.cpp" if args.baseline else "rpcs3/util/media_utils.cpp")
    actual, options = configuration(source.read_text(encoding="utf-8"))
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.out / "recording-header-audit.cpp"
    path.write_text(generate(actual), encoding="utf-8")
    print(path)
    if args.ffmpeg and not ffmpeg_audit(args.ffmpeg, args.out, options):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
