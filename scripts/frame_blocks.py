"""Score in-mission F12 screenshots (1280x720) for the known RPCS3 rendering fault: fixed blocks
in the left 800 px (rows 0-145 and 360-592) that turn black or magenta, and the right side
overexposing after movement. Per image: fraction of near-black and magenta pixels in each block,
mean brightness left/right of x=800 over rows 145-360 (a row band that is never covered by the
blocks), and a verdict.

Usage: python scripts/frame_blocks.py SHOT.png [...]
"""

import sys

from PIL import Image

BLOCKS = {"top": (0, 0, 800, 145), "bottom": (0, 360, 800, 592)}


def stats(img, box):
    px = list(img.crop(box).get_flattened_data())
    black = sum(1 for r, g, b in px if r < 8 and g < 8 and b < 8) / len(px)
    magenta = sum(1 for r, g, b in px if r > 150 and b > 150 and g < 90) / len(px)
    return black, magenta


def mean(img, box):
    px = list(img.crop(box).get_flattened_data())
    return sum(r + g + b for r, g, b in px) / (3 * len(px))


def score(path):
    img = Image.open(path).convert("RGB").resize((1280, 720))
    out = {name: stats(img, box) for name, box in BLOCKS.items()}
    left, right = mean(img, (0, 145, 800, 360)), mean(img, (800, 145, 1280, 360))
    # The dark interior renders at a mean of ~10-20; blown-out regions are > 90.
    bad = any(b > 0.35 or m > 0.3 for b, m in out.values()) or left > 90 or right > 90
    parts = " ".join(f"{n} black {b:.2f} magenta {m:.2f}" for n, (b, m) in out.items())
    return f"{'BAD ' if bad else 'ok  '}{parts} left {left:.0f} right {right:.0f}"


if __name__ == "__main__":
    for p in sys.argv[1:]:
        print(score(p), p.rsplit("\\", 1)[-1].rsplit("/", 1)[-1])
