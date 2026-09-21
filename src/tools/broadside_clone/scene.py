#!/usr/bin/env python3
"""New standalone editable scene codec; no imports from prior PeakRunner tools.

Source-derived/private. Input is the exact approved app's runtime asset data,
not a fresh Torque DIF decoder. The engine's binary ABI is the output contract.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
from PIL import Image

FIELDS = (('position', 3), ('normal', 3), ('uv', 2), ('light_uv', 2),
          ('material', 1), ('lighting', 1))
FILES = {'vertices.bin', 'collision.bin', 'height.bin', 'weights.rgba',
         'textures.rgba', 'ambient.f32'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save_json(path, value):
    path.write_text(json.dumps(value, separators=(',', ':'), allow_nan=False)+'\n')


def vertex(values):
    result = {}; cursor = 0
    for name, size in FIELDS:
        result[name] = list(values[cursor:cursor+size]) if size > 1 else values[cursor]
        cursor += size
    return result


def flatten(record):
    if set(record) != {name for name, _ in FIELDS}:
        raise ValueError('Missing or unknown vertex fields')
    result = []
    for name, size in FIELDS:
        values = record[name] if size > 1 else [record[name]]
        if len(values) != size or not all(math.isfinite(v) for v in values):
            raise ValueError('Invalid vertex values')
        result.extend(values)
    return result


def decode(pack, destination):
    if destination.exists():
        raise ValueError('Refusing to overwrite editable scene')
    manifest = json.loads((pack/'map.json').read_text())
    if not manifest.get('private_reference') or set(manifest['files']) != FILES:
        raise ValueError('Expected approved private reference ABI')
    payload = {name: (pack/name).read_bytes() for name in FILES}
    for name, data in payload.items():
        if sha(data) != manifest['files'][name]:
            raise ValueError('Reference hash mismatch: '+name)
    destination.mkdir(parents=True)
    save_json(destination/'world.json', manifest)
    save_json(destination/'provenance.json', {'format': 'broadside-clone-scene-1',
        'reference_manifest_sha256': sha((pack/'map.json').read_bytes()),
        'reference_payload_sha256': manifest['files'],
        'coordinates': 'engine world XYZ, metres, Y up; do not transpose',
        'ownership': 'private source-derived geometry/textures, not original art'})
    with (destination/'surfaces.jsonl').open('w') as out:
        for i, values in enumerate(struct.iter_unpack('<36f', payload['vertices.bin'])):
            out.write(json.dumps({'id': i, 'corners': [vertex(values[j:j+12])
                for j in (0,12,24)]}, separators=(',', ':'), allow_nan=False)+'\n')
    with (destination/'collision.jsonl').open('w') as out:
        for i, values in enumerate(struct.iter_unpack('<9f', payload['collision.bin'])):
            out.write(json.dumps({'id': i, 'points': [values[j:j+3] for j in (0,3,6)]},
                                separators=(',', ':'), allow_nan=False)+'\n')
    heights = [v[0] for v in struct.iter_unpack('<H', payload['height.bin'])]
    save_json(destination/'terrain.json', {'width':256, 'height':256,
        'units_per_metre':32, 'rows':[heights[i:i+256] for i in range(0,len(heights),256)]})
    Image.frombytes('RGBA', (256,256), payload['weights.rgba']).save(destination/'terrain-weights.png')
    save_json(destination/'audio.json', [v[0] for v in struct.iter_unpack('<f', payload['ambient.f32'])])
    textures = destination/'materials'; textures.mkdir()
    offset = 0
    for level in range(9):
        side = 256 >> level; length = side*side*4
        for layer in range(manifest['texture_count']):
            image = Image.frombytes('RGBA',(side,side),payload['textures.rgba'][offset:offset+length])
            image.save(textures/f'layer-{layer:03d}-mip-{level}.png'); offset += length
    if offset != len(payload['textures.rgba']):
        raise ValueError('Unexpected texture bytes')
    print(f'Decoded editable scene: {destination}')


def encode(scene, output):
    if output.exists():
        raise ValueError('Refusing to overwrite compiled map')
    provenance = json.loads((scene/'provenance.json').read_text())
    if provenance['format'] != 'broadside-clone-scene-1':
        raise ValueError('Unsupported scene format')
    manifest = json.loads((scene/'world.json').read_text())
    data = {}
    for source, target in [('surfaces.jsonl','vertices.bin'),('collision.jsonl','collision.bin')]:
        binary = bytearray()
        with (scene/source).open() as stream:
            for index, line in enumerate(stream):
                record = json.loads(line)
                if record['id'] != index:
                    raise ValueError('Triangle IDs must be contiguous and ordered')
                rows = record['corners'] if source == 'surfaces.jsonl' else record['points']
                if len(rows) != 3:
                    raise ValueError('Expected triangles')
                for row in rows:
                    values = flatten(row) if source == 'surfaces.jsonl' else row
                    size = 12 if source == 'surfaces.jsonl' else 3
                    if len(values) != size or not all(math.isfinite(v) for v in values):
                        raise ValueError('Invalid triangle data')
                    binary.extend(struct.pack('<'+str(size)+'f', *values))
        data[target] = bytes(binary)
    terrain = json.loads((scene/'terrain.json').read_text())
    if terrain['width'] != 256 or terrain['height'] != 256 or len(terrain['rows']) != 256:
        raise ValueError('Expected 256-square terrain')
    if any(len(row) != 256 for row in terrain['rows']):
        raise ValueError('Invalid terrain row')
    data['height.bin'] = b''.join(struct.pack('<256H', *row) for row in terrain['rows'])
    with Image.open(scene/'terrain-weights.png') as image:
        if image.size != (256,256) or image.mode != 'RGBA':
            raise ValueError('Expected RGBA terrain weights')
        data['weights.rgba'] = image.tobytes()
    samples = json.loads((scene/'audio.json').read_text())
    data['ambient.f32'] = b''.join(struct.pack('<f', value) for value in samples)
    texture_bytes = bytearray()
    for level in range(9):
        side = 256 >> level
        for layer in range(manifest['texture_count']):
            with Image.open(scene/'materials'/f'layer-{layer:03d}-mip-{level}.png') as image:
                if image.size != (side,side) or image.mode != 'RGBA':
                    raise ValueError('Texture dimensions/mode changed')
                texture_bytes.extend(image.tobytes())
    data['textures.rgba'] = bytes(texture_bytes)
    hashes = {name:sha(value) for name,value in data.items()}
    report = {'format':'broadside-clone-validation-1',
        'payload_identity':{name: hashes[name] == provenance['reference_payload_sha256'][name] for name in FILES},
        'render_triangles':len(data['vertices.bin'])//144,
        'collision_triangles':len(data['collision.bin'])//36,
        'texture_layers':manifest['texture_count']}
    manifest['files'] = hashes
    manifest['name'] = 'broadside-clone'
    manifest['id'] = 'broadside-clone'
    output.mkdir(parents=True)
    for name,value in data.items():
        (output/name).write_bytes(value)
    save_json(output/'map.json', manifest)
    save_json(output/'validation.json', report)
    print(json.dumps(report, indent=2))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['decode','encode'])
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = (Path(__file__).resolve().parents[2]/'local-assets').resolve()
    if not args.output.resolve().is_relative_to(root):
        parser.error('Source-derived output must remain under ignored local-assets/')
    (decode if args.action == 'decode' else encode)(args.source,args.output)
