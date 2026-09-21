#!/usr/bin/env python3
"""Add original decorative entrance markers to the PRIVATE approved baseline.

No fortress regeneration. All existing vertices, collision and mip layers survive.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(spec); spec.loader.exec_module(kit)


def digest(value):
    return hashlib.sha256(value).hexdigest()


def build(source, output):
    if output.exists():
        raise ValueError('Refusing to overwrite existing pack')
    manifest = json.loads((source/'map.json').read_text())
    if manifest.get('private_reference') is not True:
        raise ValueError('Private Broadside baseline required')
    data = {name:(source/name).read_bytes() for name in manifest['files']}
    baseline_hashes = dict(manifest['files'])
    for name, value in data.items():
        if digest(value) != baseline_hashes[name]:
            raise ValueError('Baseline hash mismatch: '+name)
    count = manifest['texture_count']
    additions=[]
    # Original solid-color material layers: casing, amber marker, cyan marker.
    colors=[(30,38,45,255),(255,133,34,255),(46,219,247,255)]
    for base in manifest['reference_bases']:
        mesh=kit.Mesh()
        team_layer=1 if base['name']=='Base 1' else 2
        # Two slender markers at the mouth of the existing entry corridor.
        # kit uses Y-up; source local axes are right/forward/up.
        for x in (-6.5,6.5):
            mesh.box((x,1.6,-35.5),(.32,3.2,.32),'trim',solid=False)
            mesh.box((x,1.9,-35.69),(.18,2.1,.06),'light',solid=False)
        rows=np.array(mesh.vertices,dtype='<f4').reshape(-1,12)
        basis=np.linalg.inv(np.array(base['world_to_local']))
        rows[:,:3]=rows[:,[0,2,1]]@basis.T+np.array(base['position'])
        rows[:,3:6]=rows[:,[3,5,4]]@np.linalg.inv(basis)
        rows[:,3:6]/=np.linalg.norm(rows[:,3:6],axis=1)[:,None]
        rows[:,10]=np.where(rows[:,10]==kit.MATERIALS.index('light'),count+team_layer,count)
        additions.append(rows.tobytes())
    old_vertices=data['vertices.bin']
    data['vertices.bin']+=b''.join(additions)
    # Texture storage is mip-major: append new layers at EACH mip, never move
    # old layer IDs or regenerate original pixels.
    old_textures=data['textures.rgba'];offset=0;packed=[]
    for side in (256,128,64,32,16,8,4,2,1):
        length=count*side*side*4
        packed.append(old_textures[offset:offset+length]);offset+=length
        packed.extend(Image.new('RGBA',(side,side),color).tobytes() for color in colors)
    if offset!=len(old_textures):
        raise ValueError('Unexpected mip-chain size')
    data['textures.rgba']=b''.join(packed)
    manifest['texture_count']=count+len(colors)
    manifest['name']='Broadside Workshop — entrance markers'
    manifest['entrance_marker_revision']={'baseline_sha256':baseline_hashes,
        'original_vertex_bytes':len(old_vertices),'original_texture_layers':count,
        'added_triangles':sum(map(len,additions))//144,
        'description':'Four original non-colliding decorative markers; all existing map assets preserved'}
    manifest['files']={name:digest(value) for name,value in data.items()}
    # Verification fails before writing if any old payload or texel changed.
    assert data['vertices.bin'][:len(old_vertices)]==old_vertices
    for name,value in data.items():
        if name not in ('vertices.bin','textures.rgba'):
            assert digest(value)==baseline_hashes[name]
    a=b=0
    for side in (256,128,64,32,16,8,4,2,1):
        size=count*side*side*4
        assert old_textures[a:a+size]==data['textures.rgba'][b:b+size]
        a+=size;b+=(count+3)*side*side*4
    output.mkdir(parents=True)
    for name,value in data.items():
        (output/name).write_bytes(value)
    (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('Verified: existing geometry, collision, all original mip texels and other payloads unchanged.')
    print(f'Added {sum(map(len,additions))//144} decorative triangles to {output}')


if __name__=='__main__':
    output=Path(sys.argv[1]).resolve()
    if not output.is_relative_to(ROOT/'local-assets'):
        raise ValueError('Private outputs must stay in local-assets')
    build(ROOT/'local-assets/broadside-workshop',output)
