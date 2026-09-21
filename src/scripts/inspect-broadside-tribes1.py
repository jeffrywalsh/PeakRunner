#!/usr/bin/env python3
"""Decode Tribes 1's plaintext Broadside.MIS into a PRIVATE measurement report.

Tribes 1 missions are declarative TorqueScript-like text, not compiled bytecode;
this only reads position/rotation/dataBlock fields for named objects, never
executes anything and never touches the paired binary .dis interior geometry.
Output must be a new directory beneath ignored local-assets/, mirroring the
Tribes 2 DIF study documented in docs/broadside-reference-study.md.
"""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

OBJECT_RE = re.compile(
    r'instant\s+\w+\s+"([^"]*)"\s*\{([^{}]*)\}', re.DOTALL
)
FIELD_RE = re.compile(r'(\w+)\s*=\s*"([^"]*)"')


def parse_objects(text):
    """Yield (name, fields) for every innermost {...} block, in file order."""
    # Iteratively resolve innermost blocks so nested SimGroups don't break the
    # regex; each pass only matches blocks with no remaining nested braces.
    remaining = text
    while True:
        matches = list(OBJECT_RE.finditer(remaining))
        if not matches:
            break
        for m in matches:
            fields = dict(FIELD_RE.findall(m.group(2)))
            if fields:
                yield m.group(1), fields
        remaining = OBJECT_RE.sub('', remaining)


def vec3(s):
    return tuple(float(v) for v in s.split())


def local_frame(origin, yaw):
    ox, oy, oz = origin
    cy, sy = math.cos(-yaw), math.sin(-yaw)

    def to_local(pos):
        x, y, z = pos[0] - ox, pos[1] - oy, pos[2] - oz
        return (x * cy - y * sy, x * sy + y * cy, z)

    return to_local


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mis', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'local-assets') or out.exists():
        parser.error('Output must be a NEW directory under ignored local-assets/')

    text = args.mis.read_text(errors='replace')
    objects = list(parse_objects(text))

    towers = {}
    for name, fields in objects:
        if fields.get('fileName', '').lower().startswith('acommand.'):
            index = fields['fileName'].split('.')[1]
            towers[index] = dict(
                position=vec3(fields['position']),
                rotation=vec3(fields['rotation']),
            )

    tower_list = [towers[k] for k in sorted(towers)]
    separation = None
    if len(tower_list) == 2:
        (ax, ay, az), (bx, by, bz) = (t['position'] for t in tower_list)
        separation = math.dist((ax, ay), (bx, by))

    items_by_tower = {k: [] for k in towers}
    item_types = (
        'Generator', 'SolarPanel', 'AmmoStation', 'InventoryStation',
        'CommandStation', 'IndoorTurret', 'PlasmaTurret', 'MortarTurret',
        'PulseSensor', 'Flag',
    )
    for name, fields in objects:
        block = fields.get('dataBlock', '')
        if block not in item_types and name != 'flag':
            continue
        if 'position' not in fields:
            continue
        pos = vec3(fields['position'])
        nearest = min(
            towers, key=lambda k: math.dist(pos[:2], towers[k]['position'][:2])
        )
        items_by_tower[nearest].append(dict(name=name, dataBlock=block, world=pos))

    for index, tower in towers.items():
        to_local = local_frame(tower['position'], tower['rotation'][2])
        for item in items_by_tower[index]:
            item['local'] = [round(v, 3) for v in to_local(item['world'])]

    out.mkdir(parents=True)
    report = dict(
        source_sha256=hashlib.sha256(args.mis.read_bytes()).hexdigest(),
        coordinates='Tribes 1 mission Z-up metres; local frame is tower-relative, yaw-corrected',
        towers=towers,
        base_separation_m=separation,
        items_by_tower=items_by_tower,
        warning='PRIVATE source-game diagnostic; not a distributable original asset',
    )
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
