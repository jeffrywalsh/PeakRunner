#!/usr/bin/env python3
"""Create a private development baseline, preserving the converted source pack.

No simplification, inferred rooms, texture replacement or geometry regeneration.
The reference stays immutable; modifications belong in a later workshop revision.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
FILES=('vertices.bin','collision.bin','height.bin','weights.rgba','textures.rgba','ambient.f32')

def digest(data):return hashlib.sha256(data).hexdigest()

def build(source,output):
    if output.exists():raise ValueError('Refusing to overwrite an existing workshop')
    manifest_bytes=(source/'map.json').read_bytes()
    manifest=json.loads(manifest_bytes)
    if manifest.get('private_reference') is not True:
        raise ValueError('Expected a private source-reference pack')
    payload={name:(source/name).read_bytes() for name in FILES}
    hashes={name:digest(data) for name,data in payload.items()}
    for name in FILES:
        if manifest['files'].get(name)!=hashes[name]:
            raise ValueError(f'Reference hash mismatch: {name}')
    # Only identification/provenance changes. All geometry and runtime fields
    # (including flags, spawns, equipment, transforms and sky) are retained.
    manifest['name']='Broadside Workshop — source baseline'
    manifest['id']='broadside-workshop-private'
    manifest['workshop_baseline']={'source_manifest_sha256':digest(manifest_bytes),
        'payload_sha256':hashes,'revision':0,'changes':[],
        'distribution':'private source-derived development pack; not original assets'}
    output.mkdir(parents=True)
    for name,data in payload.items():
        (output/name).write_bytes(data)
        if digest((output/name).read_bytes())!=hashes[name]:
            raise ValueError(f'Written payload mismatch: {name}')
    (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
    return hashes

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,default=ROOT/'local-assets/broadside-reference')
    parser.add_argument('--output',type=Path,default=ROOT/'local-assets/broadside-workshop')
    args=parser.parse_args(); output=args.output.resolve()
    if not output.is_relative_to(ROOT/'local-assets'):
        parser.error('Workshop output must remain under ignored local-assets/')
    hashes=build(args.source.resolve(),output)
    print(f'Created {output}; all {len(hashes)} payloads byte-identical to reference.')
    print('No procedural fortress geometry used. Private development only.')

if __name__=='__main__':main()
