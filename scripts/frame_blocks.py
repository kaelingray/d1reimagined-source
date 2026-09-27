"""Score in-mission F12 screenshots (1280x720) for the known RPCS3 rendering fault: fixed blocks
in the left 800 px (rows 0-145 and 360-592) that turn black or magenta, and the right side
overexposing after movement. A black block must show its hard edge (x = 800 / y = 145). Per image: fraction of near-black and magenta pixels in each block,
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
    # The blocks have hard straight edges (x = 800 for the bottom block, y = 145 for the top one);
    # dark scenery does not, so a black block only counts with a step in black fraction there.
    edge_bottom = stats(img, (760, 380, 800, 580))[0] - stats(img, (800, 380, 840, 580))[0]
    edge_top = stats(img, (100, 125, 700, 145))[0] - stats(img, (100, 145, 700, 165))[0]
    black_block = (out["bottom"][0] > 0.35 and edge_bottom > 0.5) or (out["top"][0] > 0.35 and edge_top > 0.5)
    # The dark interior renders at a mean of ~10-20; blown-out regions are > 90.
    bad = black_block or any(m > 0.3 for _, m in out.values()) or left > 90 or right > 90
    parts = " ".join(f"{n} black {b:.2f} magenta {m:.2f}" for n, (b, m) in out.items())
    return (f"{'BAD ' if bad else 'ok  '}{parts} edges {edge_top:.2f}/{edge_bottom:.2f} "
            f"left {left:.0f} right {right:.0f}")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        print(score(p), p.rsplit("\\", 1)[-1].rsplit("/", 1)[-1])
