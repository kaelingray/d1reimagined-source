"""Scan a one-frame render-target dump written by the instrumented RPCS3 build (marker file
C:/dev/d1dump.on; files <seq>_<c|z>_<addr>_p<pitch>_<w>x<h>_f<format>.bin, the guest-memory
contents of each target when the RSX switched away from it). Per target: NaN/Inf/zero fractions
and mean value inside the known block region (left 62.5 %, rows 0-20 % and 50-82 %) versus the
rest, so the first pass with bad data in the blocks stands out. --png SEQ writes a preview
(float formats tone-mapped, NaN magenta, Inf white).

Usage: python scripts/d1dump_scan.py DUMPDIR [--png SEQ[,SEQ...]]
"""

import re
import sys
from pathlib import Path

import numpy as np

NAME = re.compile(r"(\d+)_([cz])_([0-9a-f]+)_p(\d+)_(\d+)x(\d+)_f(\d+)\.bin")
COLOR = {8: "argb8", 11: "rgba16f", 12: "rgba32f", 13: "x32f", 9: "b8", 10: "g8b8", 5: "xrgb8", 4: "xrgb8"}


def decode(path, kind, pitch, w, h, fmt):
    raw = np.fromfile(path, dtype=np.uint8)
    rows = raw[: pitch * h].reshape(h, pitch)
    if kind == "z":
        if fmt == 1:
            return rows[:, : w * 2].copy().view(">u2").astype(np.float32)[:, :w] / 65535
        v = rows[:, : w * 4].copy().view(">u4")[:, :w]
        return ((v >> 8).astype(np.float32) / 0xFFFFFF)
    name = COLOR.get(fmt)
    if name == "rgba16f":
        return rows[:, : w * 8].copy().view(">f2").astype(np.float32).reshape(h, w, 4)
    if name == "rgba32f":
        return rows[:, : w * 16].copy().view(">f4").reshape(h, w, 4)
    if name == "x32f":
        return rows[:, : w * 4].copy().view(">f4")[:, :w]
    if name in ("argb8", "xrgb8"):
        return rows[:, : w * 4].reshape(h, w, 4)[:, :, [1, 2, 3, 0]].astype(np.float32) / 255
    if name == "g8b8":
        return rows[:, : w * 2].reshape(h, w, 2).astype(np.float32) / 255
    if name == "b8":
        return rows[:, :w].astype(np.float32) / 255
    return None


def masks(h, w):
    yy, xx = np.mgrid[0:h, 0:w]
    left = xx < w * 0.625
    block = left & ((yy < h * 0.2) | ((yy >= h * 0.5) & (yy < h * 0.82)))
    return block, ~block


def stats(a, m):
    v = a[m]
    n = max(v.size, 1)
    fin = v[np.isfinite(v)]
    return np.isnan(v).sum() / n, np.isinf(v).sum() / n, (v == 0).sum() / n, float(fin.mean()) if fin.size else float("nan")


def preview(a, out):
    from PIL import Image
    if a.ndim == 2:
        a = np.repeat(a[:, :, None], 3, axis=2)
    rgb = a[:, :, :3].astype(np.float32)
    nan, inf = np.isnan(rgb).any(axis=2), np.isinf(rgb).any(axis=2)
    rgb = np.nan_to_num(rgb, nan=0, posinf=0, neginf=0)
    if rgb.max() > 1.001 or rgb.min() < 0:
        rgb = np.clip(rgb, 0, None)
        rgb = rgb / (1 + rgb)
    img = (np.clip(rgb, 0, 1) * 255).astype(np.uint8)
    img[nan] = (255, 0, 255)
    img[inf] = (255, 255, 255)
    Image.fromarray(img).save(out)


def main():
    d = Path(sys.argv[1])
    want = set()
    if "--png" in sys.argv:
        want = {int(x) for x in sys.argv[sys.argv.index("--png") + 1].split(",")}
    for p in sorted(d.glob("*.bin")):
        m = NAME.fullmatch(p.name)
        if not m:
            continue
        seq, kind, addr, pitch, w, h, fmt = int(m[1]), m[2], int(m[3], 16), int(m[4]), int(m[5]), int(m[6]), int(m[7])
        a = decode(p, kind, pitch, w, h, fmt)
        label = f"{seq:4} {kind} 0x{addr:08x} {w}x{h} f{fmt}"
        if a is None:
            print(label, "(format not decoded)")
            continue
        inside, outside = masks(h, w)
        si, so = stats(a, inside), stats(a, outside)
        flag = " <-- NaN/Inf" if si[0] + si[1] > 0.001 or so[0] + so[1] > 0.001 else ""
        print(f"{label:34} block nan {si[0]:.3f} inf {si[1]:.3f} zero {si[2]:.2f} mean {si[3]:9.4g} | "
              f"rest nan {so[0]:.3f} inf {so[1]:.3f} zero {so[2]:.2f} mean {so[3]:9.4g}{flag}")
        if seq in want:
            preview(a, d / f"{p.stem}.png")


if __name__ == "__main__":
    main()
