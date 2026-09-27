"""Wait until a UTF-16BE text appears in RPCS3 guest memory (read-only), then append a marker
line to the instrumented build's watch file (C:/dev/d1ppu.txt), so its next reload logs a
timestamp ("D1P watch list reloaded at ..."). Used to line up first-call coverage (D1C lines)
with the moment a UI text is created.

Usage: python scripts/rpcs3_text_marker.py "TEXT" [START END] [TIMEOUT_S]
"""

import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0].rsplit("/", 1)[0])
_argv, sys.argv = sys.argv, sys.argv[:1]
import rpcs3_guest_read as guest  # noqa: E402

sys.argv = _argv
WATCH = "C:/dev/d1ppu.txt"


def main():
    needle = sys.argv[1].encode("utf-16-be")
    start = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0x40000000
    end = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0x48000000
    timeout = float(sys.argv[4]) if len(sys.argv) > 4 else 1800
    base = guest.find_base()
    while base is None:  # the game image is not loaded yet
        time.sleep(5)
        base = guest.find_base()
    deadline = time.time() + timeout
    while time.time() < deadline:
        for addr in range(start, end, 0x100000):
            data = guest.rpm(base + addr, 0x100000 + len(needle))
            if data and (i := data.find(needle)) != -1:
                with open(WATCH, "a", encoding="ascii") as f:
                    f.write(f"# marker {time.strftime('%H:%M:%S')} text at {addr + i:#x}\n")
                print(f"{time.strftime('%H:%M:%S')} found at {addr + i:#x}", flush=True)
                return
        time.sleep(0.2)
    print("not found", flush=True)


if __name__ == "__main__":
    main()
