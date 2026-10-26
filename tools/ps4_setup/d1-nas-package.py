"""Build one evidence package and hand it to the publisher's NAS inbox.

    d1-nas-package.py --spec package.json --outbox DIR --inbox \\\\NAS\\share\\incoming [--dry-run]

The spec (JSON) gives the package's own metadata fields (topic, feature, event_type, importance,
description, previous_state, current_state, remaining_issue, next_task, git_commit, game_build,
comparison_group_id/role, ...), the machine name, and the files:

    {"machine": "...", "report": "daily-report.md", "events": "events.json",
     "clips": ["clip.mp4", ...], "screenshots": ["a.png", ...], "metadata": {...}}

Layout and handoff follow the publisher's NAS package format (schema_version 1): clips/,
screenshots/, daily-report.md, events.json, metadata.json and hashes.json (size and lowercase
SHA-256 of every other file, metadata.json included). The package is built in the local outbox
(kept), copied to <inbox>/<machine>/<yyyy>/<mm>/<dd>/<clip_id>.uploading, every copied file is
re-hashed, and only then is the folder renamed to .ready. Text is scanned for local paths, e-mail
addresses and credentials first; a hit stops the handoff. Videos are probed and fully decoded
before they are packaged.
"""
import argparse, datetime as dt, hashlib, json, os, re, shutil, subprocess, sys, uuid

FFMPEG = os.environ.get('D1_FFMPEG') or shutil.which('ffmpeg') or os.path.expandvars(
    r'%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe'
    r'\ffmpeg-9.0.2-full_build\bin\ffmpeg.exe')
FFPROBE = os.path.join(os.path.dirname(FFMPEG), 'ffprobe.exe' if FFMPEG.endswith('.exe') else 'ffprobe')

# Things that must never reach a public package's text.
FORBIDDEN = [
    (r'[A-Za-z]:\\', 'a Windows path'), (r'(?i)\\\\[0-9a-z.-]+\\', 'a network path'),
    (r'(?<![\w.])/(?:home|Users|mnt|f|c)/', 'a local path'),
    (r'[\w.+-]+@[\w-]+\.[\w.]+', 'an e-mail address'),
    (r'(?i)\b(?:password|passwd|api[_-]?key|secret|token|webhook)\b\s*[:=]', 'a credential'),
    (r'(?i)\b(?:ghp|gho|github_pat|sk|xox[bp])_[A-Za-z0-9]{8,}', 'a token'),
    (r'\b(?:\d{1,3}\.){3}\d{1,3}\b', 'an IP address'),
    (r'(?i)\b(?:claude|anthropic|chatgpt|openai|copilot|codex)\b', 'an assistant or tool name'),
]


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def scan(name, text, allow):
    hits = []
    for pattern, what in FORBIDDEN:
        for m in re.finditer(pattern, text):
            if m.group(0) not in allow:
                hits.append(f'{name}: {what}: {m.group(0)!r}')
    return hits


def probe_video(path):
    out = subprocess.run([FFPROBE, '-v', 'error', '-show_entries',
                          'stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate,'
                          'sample_rate,channels:format=duration,size', '-of', 'json', path],
                         capture_output=True, text=True, check=True).stdout
    info = json.loads(out)
    dec = subprocess.run([FFMPEG, '-v', 'error', '-i', path, '-f', 'null', '-'], capture_output=True, text=True)
    if dec.returncode or dec.stderr.strip():
        sys.exit(f'{path}: does not decode cleanly: {dec.stderr.strip()[:300]}')
    v = next(s for s in info['streams'] if s['codec_type'] == 'video')
    a = [s for s in info['streams'] if s['codec_type'] == 'audio']
    num, den = (int(x) for x in v['avg_frame_rate'].split('/'))
    return {'duration_seconds': round(float(info['format']['duration']), 3),
            'width': v['width'], 'height': v['height'], 'video_codec': v['codec_name'],
            'average_frame_rate': round(num / den, 2) if den else None,
            'audio': [{'codec': s['codec_name'], 'sample_rate': int(s['sample_rate']),
                       'channels': s['channels']} for s in a],
            'decoded_fully': True}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--spec', required=True)
    ap.add_argument('--outbox', required=True)
    ap.add_argument('--inbox', required=True)
    ap.add_argument('--dry-run', action='store_true')
    a = ap.parse_args()
    spec = json.load(open(a.spec, encoding='utf-8'))
    base = os.path.dirname(os.path.abspath(a.spec))
    rel = lambda p: p if os.path.isabs(p) else os.path.join(base, p)
    machine = spec['machine']
    now = dt.datetime.now(dt.timezone.utc)
    clip_id = f'{machine}-{now:%Y%m%dT%H%M%S}-{uuid.uuid4().hex}'
    pkg = os.path.join(a.outbox, clip_id)
    os.makedirs(pkg)
    files = []
    clips_meta = []
    for k, c in enumerate(spec.get('clips', []), 1):
        name = 'clips/clip.mp4' if len(spec['clips']) == 1 else f'clips/clip{k:02d}.mp4'
        os.makedirs(os.path.join(pkg, 'clips'), exist_ok=True)
        shutil.copyfile(rel(c), os.path.join(pkg, name))
        clips_meta.append({'file': name, **probe_video(os.path.join(pkg, name))})
        files.append(name)
    for k, s in enumerate(spec.get('screenshots', []), 1):
        name = f'screenshots/{k:02d}.png'
        os.makedirs(os.path.join(pkg, 'screenshots'), exist_ok=True)
        shutil.copyfile(rel(s), os.path.join(pkg, name))
        files.append(name)
    for key, name in (('report', 'daily-report.md'), ('events', 'events.json')):
        if spec.get(key):
            shutil.copyfile(rel(spec[key]), os.path.join(pkg, name))
            files.append(name)
    meta = {
        'schema_version': 1, 'source_machine': machine, 'clip_id': clip_id,
        'project': 'destiny1-preservation', 'recorded_at': now.isoformat(), 'devblog': None,
        'contains_voice': False, 'contains_microphone_audio': False,
        'importance': 'USEFUL',
        'clips': clips_meta, 'new_video_count': len(clips_meta),
        'new_screenshot_count': len(spec.get('screenshots', [])),
    }
    meta.update(spec.get('metadata', {}))
    meta['clip_id'] = clip_id
    with open(os.path.join(pkg, 'metadata.json'), 'w', encoding='utf-8', newline='\n') as f:
        json.dump(meta, f, indent=2, ensure_ascii=False)
        f.write('\n')
    files.append('metadata.json')
    allow = set(spec.get('scan_allow', []))
    hits = []
    for name in files:
        if name.endswith(('.md', '.json')):
            text = open(os.path.join(pkg, name), encoding='utf-8').read()
            hits += scan(name, text.replace(clip_id, ''), allow)
    if hits:
        sys.exit('privacy scan failed, nothing sent:\n  ' + '\n  '.join(hits))
    hashes = {'schema_version': 1, 'files': {
        n: {'size': os.path.getsize(os.path.join(pkg, n)), 'sha256': sha256(os.path.join(pkg, n))}
        for n in sorted(files)}}
    with open(os.path.join(pkg, 'hashes.json'), 'w', encoding='utf-8', newline='\n') as f:
        json.dump(hashes, f, indent=2)
        f.write('\n')
    print('built', pkg)
    if a.dry_run:
        return
    day = os.path.join(a.inbox, machine, f'{now:%Y}', f'{now:%m}', f'{now:%d}')
    up = os.path.join(day, clip_id + '.uploading')
    shutil.copytree(pkg, up)
    for n, h in hashes['files'].items():
        p = os.path.join(up, n)
        if os.path.getsize(p) != h['size'] or sha256(p) != h['sha256']:
            sys.exit(f'copy check failed for {n}; left as {up}')
    if sha256(os.path.join(up, 'hashes.json')) != sha256(os.path.join(pkg, 'hashes.json')):
        sys.exit(f'copy check failed for hashes.json; left as {up}')
    ready = os.path.join(day, clip_id + '.ready')
    os.rename(up, ready)
    print('ready', ready)


if __name__ == '__main__':
    main()
