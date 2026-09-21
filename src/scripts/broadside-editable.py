#!/usr/bin/env python3
"""Private fixed-topology OBJ exchange. Never modifies the source pack."""
import argparse
from collections import defaultdict, deque
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parent.parent


def sha(data):
    return hashlib.sha256(data).hexdigest()


def payloads(source):
    manifest = json.loads((source / 'map.json').read_text())
    if manifest.get('private_reference') is not True:
        raise ValueError('Private reference pack required')
    data = {name: (source / name).read_bytes() for name in manifest['files']}
    for name, value in data.items():
        if sha(value) != manifest['files'][name]:
            raise ValueError('Hash mismatch: ' + name)
    return manifest, data


def export(source, output, base=0):
    if output.exists():
        raise ValueError('Output already exists')
    manifest, data = payloads(source)
    instances = [i for i in manifest['instances'] if i['name'] == 'dbase_broadside_nef.dif']
    inst = instances[base]
    records = list(struct.iter_unpack('<12f', data['vertices.bin']))
    lookup = defaultdict(deque)
    for start in range(0, len(records), 3):
        key = struct.pack('<9f', *(v for r in records[start:start+3] for v in r[:3]))
        lookup[key].append(start // 3)
    ids = []
    for ci in range(inst['first'], inst['first'] + inst['count']):
        key = data['collision.bin'][ci*36:(ci+1)*36]
        if not lookup[key]:
            raise ValueError(f'No exact render match for collision triangle {ci}')
        ids.append(lookup[key].popleft())
    output.mkdir(parents=True)
    # Engine Y-up coordinates are retained: no lossy translation/axis conversion.
    lines = ['# PeakRunner private fixed-topology mesh; engine Y-up, meters', 'mtllib fortress.mtl']
    layers = set()
    for face, tri in enumerate(ids):
        rows = records[tri*3:tri*3+3]
        layer = int(rows[0][10]); layers.add(layer)
        lines.extend([f'g triangle_{face}', f'usemtl layer_{layer}'])
        for r in rows:
            lines.append('v ' + ' '.join(repr(x) for x in r[:3]))
            lines.append('vt ' + ' '.join(repr(x) for x in r[6:8]))
            lines.append('vn ' + ' '.join(repr(x) for x in r[3:6]))
        indices = range(face*3+1, face*3+4)
        lines.append('f ' + ' '.join(f'{i}/{i}/{i}' for i in indices))
    (output / 'fortress.obj').write_text('\n'.join(lines) + '\n')
    from PIL import Image
    mtl = []
    for layer in sorted(layers):
        pixels = data['textures.rgba'][layer*256*256*4:(layer+1)*256*256*4]
        Image.frombytes('RGBA', (256, 256), pixels).save(output / f'layer_{layer}.png')
        mtl.extend([f'newmtl layer_{layer}', 'Kd 1 1 1', f'map_Kd layer_{layer}.png'])
    (output / 'fortress.mtl').write_text('\n'.join(mtl) + '\n')
    metadata = dict(schema=1, source=str(source.resolve()), source_hashes=manifest['files'],
                    instance=inst, render_triangles=ids,
                    contract='Fixed topology, face and corner order. Preserve groups, UVs and normals. '
                    'Sidecar retains light UVs and light layers; textures are preview-only. '
                    'Collision follows edited positions. Geometry edits require rebaking lighting later.')
    (output / 'exchange.json').write_text(json.dumps(metadata, indent=2) + '\n')
    return len(ids)


def read_obj(path):
    positions, uvs, normals, faces = [], [], [], []
    group = material = None
    for line in path.read_text().splitlines():
        words = line.split()
        if not words or words[0].startswith('#'):
            continue
        tag, values = words[0], words[1:]
        if tag in ('v', 'vt', 'vn'):
            row = tuple(map(float, values))
            if len(row) != (2 if tag == 'vt' else 3) or not all(map(math.isfinite, row)):
                raise ValueError('Invalid vertex attribute')
            {'v': positions, 'vt': uvs, 'vn': normals}[tag].append(row)
        elif tag == 'g':
            group = ' '.join(values)
        elif tag == 'usemtl':
            material = ' '.join(values)
        elif tag == 'f':
            if len(values) != 3 or group != f'triangle_{len(faces)}':
                raise ValueError('Face order/groups/topology changed; unsupported in revision 1')
            rows = []
            for value in values:
                indices = list(map(int, value.split('/')))
                if len(indices) != 3 or min(indices) <= 0:
                    raise ValueError('Expected positive position/UV/normal indices')
                p, uv, n = indices
                rows.append((*positions[p-1], *normals[n-1], *uvs[uv-1]))
            faces.append((rows, material))
    return faces


def import_mesh(exchange, output):
    if output.exists():
        raise ValueError('Output already exists')
    meta = json.loads((exchange / 'exchange.json').read_text())
    if meta['schema'] != 1:
        raise ValueError('Unsupported exchange schema')
    source = Path(meta['source'])
    manifest, data = payloads(source)
    if manifest['files'] != meta['source_hashes']:
        raise ValueError('Baseline changed')
    faces = read_obj(exchange / 'fortress.obj')
    if len(faces) != len(meta['render_triangles']):
        raise ValueError('Triangle count changed')
    vertices = bytearray(data['vertices.bin']); collision = bytearray(data['collision.bin'])
    for face, (rows, material) in enumerate(faces):
        tri = meta['render_triangles'][face]
        for corner, row in enumerate(rows):
            offset = (tri*3+corner)*48
            original = struct.unpack_from('<12f', vertices, offset)
            if material != f'layer_{int(original[10])}':
                raise ValueError('Material reassignment unsupported in revision 1')
            struct.pack_into('<12f', vertices, offset, *row, *original[8:])
        ci = meta['instance']['first'] + face
        struct.pack_into('<9f', collision, ci*36, *(v for row in rows for v in row[:3]))
    data['vertices.bin'] = bytes(vertices); data['collision.bin'] = bytes(collision)
    output.mkdir(parents=True)
    for name, value in data.items():
        (output / name).write_bytes(value)
    manifest['files'] = {name: sha(value) for name, value in data.items()}
    manifest['name'] = 'Broadside Workshop — OBJ round trip'
    manifest['editable_exchange'] = dict(schema=1, source_hashes=meta['source_hashes'],
                                       triangles=len(faces), private=True)
    (output / 'map.json').write_text(json.dumps(manifest, indent=2) + '\n')
    return {name: sha(value) == meta['source_hashes'][name] for name, value in data.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['export', 'import'])
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--base', type=int, choices=[0, 1], default=0)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(ROOT / 'local-assets'):
        parser.error('Private outputs must stay under local-assets/')
    result = (export(args.source, args.output, args.base) if args.action == 'export'
              else import_mesh(args.source, args.output))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
