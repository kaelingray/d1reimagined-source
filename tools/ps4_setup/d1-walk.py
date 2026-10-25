"""Walk the Guardian along a recorded route on the 56578 client (shadPS4), by pad input only.

Position: the client keeps many float triples that track the player (rendering and simulation
copies). `--scan` finds them: candidates near a known point, kept when they move with the player
and stay still when it does; the position is the median of the tracked set. No memory writes.

Steering: heading from the last movement, camera turns with the right stick (J/L), walking with
the left stick (W). Turn rate and sign are calibrated on the first turn.

  d1-walk.py --scan 383,-962,12 --out tracked.json      (needs the player able to move)
  d1-walk.py --tracked tracked.json --route missions/routes/enter_the_wall.json [--until-incident ...]
"""
import argparse, ctypes, ctypes.wintypes as wt, json, math, os, statistics, struct, subprocess, sys, time

src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py')).read()
wg = {'__name__': 'lib'}
exec(compile(src, 'd1-winguest.py', 'exec'), wg)
PAD = r'F:\d1\tools\d1-pad.ps1'


def pad(*keys, gap=150):
    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', ','.join(keys), '-GapMilliseconds', str(gap)],
                   capture_output=True)


class Client:
    def __init__(self):
        self.p, self.base = wg['open_client']()

    def triple(self, a):
        try:
            return struct.unpack('<3f', self.p.read(a, 12))
        except OSError:
            return None

    def scan(self, centre, radius):
        k32 = ctypes.WinDLL('kernel32', use_last_error=True)

        class MBI(ctypes.Structure):
            _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p),
                        ('AllocationProtect', wt.DWORD), ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t),
                        ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]
        k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
        import numpy as np
        out, addr, mbi = [], 0x100000000, MBI()
        while addr < 0x1000000000 and k32.VirtualQueryEx(self.p.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
            b0, sz = mbi.BaseAddress or 0, mbi.RegionSize
            if mbi.State == 0x1000 and mbi.Protect in (0x04, 0x40) and sz < (1 << 31):
                for c in range(0, sz, 1 << 24):
                    try:
                        buf = self.p.read(b0 + c, min(1 << 24, sz - c))
                    except OSError:
                        continue
                    a = np.frombuffer(buf[:len(buf) // 4 * 4], dtype='<f4')
                    m = ((abs(a[:-2] - centre[0]) < radius) & (abs(a[1:-1] - centre[1]) < radius)
                         & (abs(a[2:] - centre[2]) < radius))
                    out += [b0 + c + 4 * int(i) for i in np.nonzero(m)[0]]
            addr = b0 + sz
        return out


def keep(client, addrs, moved):
    """Re-read; keep addresses that moved 1..15 units (moved=True) or stayed within 0.05."""
    before = {a: client.triple(a) for a in addrs}
    return before


def filter_step(client, before, moved):
    kept = []
    for a, v in before.items():
        n = client.triple(a)
        if not v or not n:
            continue
        d = math.dist(v[:2], n[:2])
        if (moved and 1 <= d <= 15 and abs(n[2] - v[2]) < 3) or (not moved and d <= 0.05):
            kept.append(a)
    return kept


def position(client, tracked, near=None):
    pts = [t for t in (client.triple(a) for a in tracked) if t and all(math.isfinite(x) for x in t)]
    if near:
        pts = [t for t in pts if math.dist(t[:2], near[:2]) < 25] or pts
    if not pts:
        return None
    return tuple(statistics.median(t[k] for t in pts) for k in range(3))


def wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--scan')
    ap.add_argument('--out')
    ap.add_argument('--tracked')
    ap.add_argument('--route')
    ap.add_argument('--tolerance', type=float, default=2.5)
    ap.add_argument('--max-seconds', type=float, default=600)
    a = ap.parse_args()
    c = Client()
    if a.scan:
        centre = [float(v) for v in a.scan.split(',')]
        addrs = c.scan(centre, 25)
        print(len(addrs), 'candidates')
        for moved, keys in ((True, ['Hold:W:1500']), (False, []), (True, ['Hold:S:1500']), (False, [])):
            before = keep(c, addrs, moved)
            if keys:
                pad(*keys)
            else:
                time.sleep(2)
            addrs = filter_step(c, before, moved)
            print(('moved' if moved else 'still'), len(addrs))
        json.dump(addrs, open(a.out, 'w'))
        print('position', position(c, addrs))
        return
    tracked = json.load(open(a.tracked))
    route = json.load(open(a.route))
    here = position(c, tracked)
    print('start', here)
    rate, sign = 0.0035, 1.0     # rad per ms of right-stick hold, and its direction (calibrated below)
    heading, calibrated = None, False
    t_end = time.time() + a.max_seconds
    i = min(range(len(route)), key=lambda k: math.dist(route[k][:2], here[:2]))
    stuck = 0
    best, since_best = None, 0
    while i < len(route) and time.time() < t_end:
        target = route[i]
        d = math.dist(here[:2], target[:2])
        if d < a.tolerance:
            i += 1
            best, since_best = None, 0
            continue
        # No progress toward this waypoint for a while (a railing, a car): back off, side-step
        # and take the next waypoint instead.
        if best is None or d < best - 0.5:
            best, since_best = d, 0
        else:
            since_best += 1
        if since_best >= 8:
            pad('Hold:S:600', 'Hold:A:500' if (i % 2) else 'Hold:D:500', 'NumPad2')
            i = min(i + 1, len(route) - 1)
            best, since_best, heading = None, 0, None
            here = position(c, tracked, here) or here
            print(f'skip to wp {i}')
            continue
        want = math.atan2(target[1] - here[1], target[0] - here[0])
        if heading is not None:
            err = wrap(want - heading)
            if abs(err) > 0.3:
                ms = int(min(900, abs(err) / rate))
                key = 'L' if (err < 0) == (sign > 0) else 'J'
                h0 = heading
                pad(f'Hold:{key}:{ms}')
                if not calibrated:
                    # measure the turn: one short forward pulse gives the new heading
                    p0 = position(c, tracked, here)
                    pad('Hold:W:500')
                    p1 = position(c, tracked, here)
                    if p0 and p1 and math.dist(p0[:2], p1[:2]) > 0.3:
                        h1 = math.atan2(p1[1] - p0[1], p1[0] - p0[0])
                        turned = wrap(h1 - h0)
                        if abs(turned) > 0.05:
                            sign = (1.0 if turned * (-1 if key == 'L' else 1) > 0 else -1.0)
                            rate = max(0.0008, min(0.01, abs(turned) / ms))
                            calibrated = True
                            print(f'calibrated: {rate * 1000:.2f} rad/s per key, sign {sign:+.0f}')
                        heading, here = h1, p1
                    continue
                heading = None
        p0 = here
        pad('Hold:W:700')
        here = position(c, tracked, p0) or p0
        step = math.dist(p0[:2], here[:2])
        if step > 0.3:
            heading = math.atan2(here[1] - p0[1], here[0] - p0[0])
            stuck = 0
        else:
            stuck += 1
            heading = None
            pad('NumPad2', 'Hold:A:400' if stuck % 2 else 'Hold:D:400')   # jump and side-step out
        print(f'wp {i}/{len(route)} at ({here[0]:.1f}, {here[1]:.1f}, {here[2]:.1f}) -> ({target[0]:.1f}, {target[1]:.1f})')
    print('done' if i >= len(route) else 'stopped', here)


if __name__ == '__main__':
    main()
