"""Build the D1_AH_MISSION_STEPS files for A Guardian Rises' opening (56578, activity index 164).

The order is the 01.24 mission script (`missions/a_guardian_rises.json`): the revival cinematic
`pf_104_res`, then the first-person `pf_104_res_fp`, both stopped, then the host teleport from the
intro slice set 113 to gameplay 112 with "Enter the Wall" active.

Inputs are sensor-client lists captured with `d1-winguest.py sensor-clients --all --bundle` while
the client waits in each region (113 and 112), plus a list holding the objective states (the
`obj3s` list: objective 3 state 5 with its waypoint, incoming sequence 4).

Cinematic state `0x8080254D` (212 bytes on 56578, handler 0x403140):
  +0x04 play counter (a change starts/stops), +0x08 playing, +0x0C optional object ref
  ({0x811C9DC5, 0xFF, 0xFFFF} = absent; all-zero is "present" and the failed resolve crashes at
  0x126CE5), +0x14 team filter (-1 = off), +0x9C cast count, +0xA8 cast[0] = the local player's
  identity (kind 0x23, the spawn bundle's +0x40 bytes), not a machine id.

  d1-agr-steps.py --r113 LIST --r112 LIST --objectives LIST --out-dir DIR
"""
import argparse, os, struct

ABSENT = (0x811C9DC5, 0xFF, 0xFFFF)
C1, C2 = ('0x992bb471', '0x6', '1'), ('0x992bb471', '0x6', '2')
CINE_TYPE = '0x8080254D'
STORY_GROUP = '0x4786c0e0'


def identity(lines):
    for l in lines:
        p = l.split()
        if len(p) > 6 and p[5].lower() == '0x808022a5':
            return bytes.fromhex(p[6])[0x40:0x48]
    raise SystemExit('no spawn bundle (A 0x808022A5) in the list')


def cine(counter, playing, ident):
    img = bytearray(0xD4)
    struct.pack_into('<I', img, 0x04, counter)
    img[0x08] = 1 if playing else 0
    struct.pack_into('<IBxH', img, 0x0C, *ABSENT)
    img[0x14] = 0xFF
    if playing:
        struct.pack_into('<I', img, 0x9C, 1)
        img[0xA8:0xB0] = ident
    return img.hex()


def patched(lines, states):
    out, hit = [], set()
    for l in lines:
        p = l.split()
        key = tuple(p[:3])
        if key in states:
            t, h = states[key]
            l = ' '.join(p[:5] + [t, h])
            hit.add(key)
        out.append(l)
    missing = set(states) - hit
    if missing:
        raise SystemExit(f'objects not in the list: {sorted(missing)}')
    return '\n'.join(out) + '\n'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--r113', required=True)
    ap.add_argument('--r112', required=True)
    ap.add_argument('--objectives', required=True)
    ap.add_argument('--out-dir', required=True)
    a = ap.parse_args()
    r113 = open(a.r113).read().splitlines()
    r112 = open(a.r112).read().splitlines()
    ident = identity(r113)
    objectives = {tuple(p[:3]): (p[5], p[6]) for p in (l.split() for l in open(a.objectives))
                  if len(p) > 6 and p[0] == '0x992bb471'}
    files = {
        'agr-step0.txt': patched(r113, {C1: (CINE_TYPE, cine(1, True, ident))}),
        'agr-step1.txt': patched(r113, {C1: (CINE_TYPE, cine(2, False, ident)), C2: (CINE_TYPE, cine(1, True, ident))}),
        'agr-step2.txt': patched(r113, {C1: (CINE_TYPE, cine(2, False, ident)), C2: (CINE_TYPE, cine(2, False, ident))}),
        'agr-step3.txt': patched(r112, objectives),
    }
    os.makedirs(a.out_dir, exist_ok=True)
    for name, text in files.items():
        open(os.path.join(a.out_dir, name), 'w').write(text)
    d = os.path.abspath(a.out_dir).replace('\\', '/')
    steps = ('# A Guardian Rises (164): revival, first person, teleport 113 -> 112 (01.24 script order)\n'
             f'164  0    {STORY_GROUP}  {d}/agr-step0.txt\n'
             f'164  70   {STORY_GROUP}  {d}/agr-step1.txt\n'
             f'164  115  {STORY_GROUP}  {d}/agr-step2.txt\n'
             f'164  120  {STORY_GROUP}  {d}/agr-step3.txt  112\n')
    open(os.path.join(a.out_dir, 'agr-steps.txt'), 'w').write(steps)
    print(f'wrote {len(files) + 1} files to {d}; objectives {sorted(objectives)}')


if __name__ == '__main__':
    main()
