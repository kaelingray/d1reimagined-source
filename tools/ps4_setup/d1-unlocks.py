"""Read or set the client's derived unlock-flag plane (56578).

The evaluator 0x28CFB0(obj, idx) tests `[plane + 4 + idx] == 2` for idx <= 0x1FFF, where `plane`
comes from 0x2601F0(obj). Find the plane once with
`d1-watchwrite.py --exec 0x28cfca` (rax = plane) and pass it with --plane.

  d1-unlocks.py --plane 0x37f8bd3c8                      list set flags
  d1-unlocks.py --plane P --save FILE                    save the 0x2000-byte plane
  d1-unlocks.py --plane P --set 1,2,3 --confirm          set flags to 2
  d1-unlocks.py --plane P --all --except 306,3872 --confirm   set every flag except these
  d1-unlocks.py --plane P --restore FILE --confirm       write a saved plane back
SYNTHETIC: a poked plane is not served state.
"""
import argparse, os, sys
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py')).read()
wg = {'__name__': 'd1_winguest_lib'}
exec(compile(src, 'd1-winguest.py', 'exec'), wg)

N = 0x2000
POISON = {306, 3872, 3876, 3877}

ap = argparse.ArgumentParser()
ap.add_argument('--plane', required=True)
ap.add_argument('--save'); ap.add_argument('--restore')
ap.add_argument('--set'); ap.add_argument('--all', action='store_true')
ap.add_argument('--except', dest='exc', default='')
ap.add_argument('--value', type=int, default=2)
ap.add_argument('--confirm', action='store_true')
a = ap.parse_args()

p, base = wg['open_client']()
plane = int(a.plane, 16)
flags = plane + 4
cur = p.read(flags, N)
print(f'plane {plane:#x}: valid={p.read(plane,1)[0]} dirty={p.read(plane+2,1)[0]}  flags==2: {sum(1 for b in cur if b == 2)}  nonzero: {sum(1 for b in cur if b)}')
if a.save:
    open(a.save, 'wb').write(cur); print('saved', a.save)
new = None
if a.restore:
    new = bytearray(open(a.restore, 'rb').read()); assert len(new) == N
elif a.set or a.all:
    new = bytearray(cur)
    exc = {int(x) for x in a.exc.split(',') if x} | (POISON if a.all else set())
    idx = range(N) if a.all else [int(x) for x in a.set.split(',')]
    for i in idx:
        if i not in exc: new[i] = a.value
if new is None:
    print('set:', [i for i, b in enumerate(cur) if b == 2][:200])
    vals = {}
    for b in cur: vals[b] = vals.get(b, 0) + 1
    print('value histogram:', dict(sorted(vals.items())))
    sys.exit(0)
if not a.confirm:
    sys.exit(f'would change {sum(1 for x, y in zip(cur, new) if x != y)} bytes; pass --confirm')
p.write(flags, bytes(new))
print(f'wrote; flags==2 now {sum(1 for b in p.read(flags, N) if b == 2)}')
