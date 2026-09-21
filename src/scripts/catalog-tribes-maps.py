#!/usr/bin/env python3
"""Extract local T2 ZIP packs separately and inventory T1/T2 mission text.

Never executes mission scripts. Output contains copyrighted local assets and
must remain outside version control. Existing extraction destinations are refused.
"""
import argparse
import hashlib
import json
import re
import shutil
import stat
import zipfile
from collections import Counter
from pathlib import Path, PurePosixPath


def safe_members(archive):
    members = archive.infolist()
    if sum(m.file_size for m in members) > 2_000_000_000:
        raise ValueError("Archive exceeds extraction limit")
    names = set()
    for member in members:
        path = PurePosixPath(member.filename.replace('\\', '/'))
        if path.is_absolute() or '..' in path.parts or ':' in str(path):
            raise ValueError(f"Unsafe archive path: {path}")
        if stat.S_ISLNK(member.external_attr >> 16):
            raise ValueError(f"Archive symlink: {path}")
        key = str(path).casefold()
        if key in names:
            raise ValueError(f"Duplicate archive path: {path}")
        names.add(key)
        yield member, path


def mission(path, root):
    raw = path.read_bytes()
    text = raw.decode('cp1252', errors='replace')
    objects = Counter(re.findall(r'\b(?:instant|new)\s+(\w+)', text))
    references = sorted(set(re.findall(
        r'"([^"\r\n]+\.(?:ter|ted|vol|dif|dis|dts|dml))"', text, re.I)))
    return dict(path=str(path.relative_to(root)), sha256=hashlib.sha256(raw).hexdigest(),
                objects=dict(objects), references=references)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--t1-base', type=Path, required=True)
    parser.add_argument('--t2-base', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Output must be a new directory (no overwrites)')
    packs = sorted(args.t2_base.glob('*.vl2'))
    if not packs or not args.t1_base.is_dir():
        parser.error('Missing source directories or T2 archives')
    # Validate every pack before writing any extracted data.
    for pack in packs:
        with zipfile.ZipFile(pack) as archive:
            list(safe_members(archive))
    args.output.mkdir(parents=True)
    provenance = []
    for pack in packs:
        destination = args.output / 'tribes2' / pack.stem
        with zipfile.ZipFile(pack) as archive:
            for member, relative in safe_members(archive):
                target = destination / str(relative)
                if member.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with archive.open(member) as source, target.open('xb') as output:
                        shutil.copyfileobj(source, output)
        provenance.append(dict(source=str(pack), sha256=hashlib.sha256(pack.read_bytes()).hexdigest()))
    games = {}
    for name, root in [('tribes1', args.t1_base), ('tribes2', args.output / 'tribes2')]:
        files = sorted(p for p in root.rglob('*') if p.is_file())
        missions = [mission(p, root) for p in files if p.suffix.lower() == '.mis']
        games[name] = dict(root=str(root.resolve()), file_types=dict(Counter(p.suffix.lower() for p in files)),
                           mission_count=len(missions), unique_mission_contents=len({m['sha256'] for m in missions}),
                           missions=missions)
    catalog = dict(archives=provenance, games=games,
                   notes=['Counts are mission definitions, including training and mod variants, not unique terrain layouts.',
                          'Reference listing is lexical, not script execution or complete dependency resolution.',
                          'T1 PVOL containers are preserved, not internally decoded. No PeakRunner conversion performed.'])
    (args.output / 'catalog.json').write_text(json.dumps(catalog, indent=2) + '\n')
    for name, game in games.items():
        print(name, game['mission_count'], 'missions;', game['unique_mission_contents'], 'distinct contents;', game['file_types'])


if __name__ == '__main__':
    main()
