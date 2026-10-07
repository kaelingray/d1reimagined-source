"""Decode one frozen descriptor trace without inventing native timestamps."""
from pathlib import Path
import argparse
import hashlib
import json
import re

BEGIN = re.compile(r'D1RING begin retained=(\d+) last=(\d+) overwritten=(\d+)')
ROW = re.compile(r'D1RING serial=(\d+) time_us=(\d+) kind=(\d+) a=([0-9a-f]{16}) b=([0-9a-f]{16}) c=([0-9a-f]{16}) d=([0-9a-f]{16}) e=([0-9a-f]{16}) f=([0-9a-f]{16})')
END = re.compile(r'D1RING end last=(\d+)')

def decode(text):
    begins, ends = list(BEGIN.finditer(text)), list(END.finditer(text))
    if len(begins) != 1 or len(ends) != 1 or ends[0].start() <= begins[0].end():
        raise ValueError('Expected one complete frozen dump')
    count, last, overwritten = map(int, begins[0].groups())
    records = []
    for match in ROW.finditer(text, begins[0].end(), ends[0].start()):
        serial, time_us, kind = map(int, match.groups()[:3])
        fields = [int(x, 16) for x in match.groups()[3:]]
        a,b,c,d,e,f = fields
        if kind == 1:
            payload = f'tag eid={a} cmd={b:016x} fence={c:016x}'
        elif kind == 2:
            result = e - (1 << 64) if e >= (1 << 63) else e
            payload = f'submit order={a} queue={b:016x} cmd={c:016x} fence={d:016x} result={result}'
        elif kind in (3,4):
            payload = f'completed path={"poke" if kind == 3 else "wait"} eid={a} cmd={b:016x} fence={c:016x}'
        elif kind == 5:
            payload = f'retire object={a:016x} pool={b:016x} index={c} current={d} completed={e}'
        elif kind == 6:
            payload = f'dispose object={a:016x} scope={b} current={c} completed={d}'
        elif kind == 7:
            payload = f'collect cutoff={a} first={b} last={c}'
        elif kind == 8:
            payload = f'reset pool={a:016x} index={b} current={c} completed={d}'
        else:
            raise ValueError('Unknown descriptor event kind')
        records.append({'serial': serial, 'host_time_us': time_us, 'kind': kind, 'fields': fields, 'decoded': payload})
    if count != len(records) or last != overwritten+count or int(ends[0].group(1)) != last:
        raise ValueError('Dump count or overwrite accounting mismatch')
    if [r['serial'] for r in records] != list(range(overwritten+1,last+1)):
        raise ValueError('Dump suffix has gaps, duplicates or reordered records')
    # Only the first callback after this one-shot dump has captured lead-up.
    failures = [line for line in text[ends[0].end():].splitlines()
                if 'vkResetDescriptorPool()' in line and 'VkCommandBuffer' in line]
    return {'retained': count, 'last_serial': last, 'overwritten': overwritten,
            'records': records, 'first_validation_callback': failures[0] if failures else None,
            'later_uncaptured_callback_count': max(0,len(failures)-1),
            'scope': 'Serialized memory-trace suffix; host timestamps are original fields, not native log timestamps. Older overwritten history is unavailable.'}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--log', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True, help='New private result directory')
    args = parser.parse_args()
    raw = args.log.read_bytes()
    result = decode(raw.decode('utf-8', errors='replace'))
    result['source_sha256'] = hashlib.sha256(raw).hexdigest()
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / 'descriptor-ring.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    lines = ['# Decoded frozen memory trace. No synthetic native timestamps.']
    lines += [f'# serial={r["serial"]} host_time_us={r["host_time_us"]} D1DESC {r["decoded"]}' for r in result['records']]
    if result['first_validation_callback']:
        lines.append(result['first_validation_callback'])
    (args.out / 'descriptor-ring-decoded.log').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k != 'records'}, indent=2))

if __name__ == '__main__':
    main()
