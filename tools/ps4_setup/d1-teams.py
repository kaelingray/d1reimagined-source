#!/usr/bin/env python3
"""d1-teams - read (and optionally set) the session team bytes on build 56578.

The client picks teams itself in the matchmaking state (0x7AB270 -> 0xB9A040 -> 0xB9A140, team
picker 0x418360). The lab launch poke sets the goal straight to in_world (0x24), which the client
reads as "the activity-session host finished matchmaking" (0x7AB5D0), so the pick never runs and
every player keeps team -1.

Membership of session i (accessor 0x7756E0 / 0x775180):
    G   = [eboot + 0x26CA4E0]                 (slot read by 0x7756E6)
    S   = G + i * 0x131C8
    M   = S + [S + 8] * 0x98E0 + 0x10 + 0x620 (0xB74340)
    peers: valid mask M+0x2C, entries M+0x5C + p*0x168 (state != 3), player index at +0x18C - 0x5C
    team byte: M + player * 0xE8 + 0x1D08

    d1-teams.py                 # show sessions 0 and 1
    d1-teams.py --set 0         # set every assigned player's team to 0 in both sessions
"""
import argparse, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(HERE, 'd1-winguest.py'), encoding='utf-8').read().split('def main():')[0]
wg = {}
exec(compile(src, 'd1-winguest.py', 'exec'), wg)

G_SLOT = 0x26CA4E0


def membership(p, base, i):
    g = p.u(base + G_SLOT, 'Q')
    if not g or p.read(g, 1)[0] == 0:
        return None
    s = g + i * 0x131C8
    k = struct.unpack('<i', p.read(s + 8, 4))[0]
    return s + k * 0x98E0 + 0x10 + 0x620


def players(p, m):
    mask = p.u(m + 0x2C, 'I')
    out = []
    for peer in range(20):
        if not mask >> peer & 1:
            continue
        e = m + 0x5C + peer * 0x168
        state = p.u(e, 'I')
        if state == 3:
            continue
        idx = struct.unpack('<i', p.read(m + peer * 0x168 + 0x18C, 4))[0]
        out.append((peer, state, idx))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--set', type=int, help='team index to write for every assigned player')
    a = ap.parse_args()
    p, base = wg['open_client']()
    for i in (0, 1):
        m = membership(p, base, i)
        if m is None:
            print(f'session {i}: none'); continue
        print(f'session {i}: membership {m:#x}')
        for peer, state, idx in players(p, m):
            if idx < 0:
                print(f'  peer {peer} state {state} no player'); continue
            t = m + idx * 0xE8 + 0x1D08
            team = struct.unpack('<b', p.read(t, 1))[0]
            line = f'  peer {peer} state {state} player {idx} team {team} @ {t:#x}'
            if a.set is not None and team != a.set:
                p.write(t, struct.pack('<b', a.set))
                line += f' -> {struct.unpack("<b", p.read(t, 1))[0]}'
            print(line)


if __name__ == '__main__':
    main()
