"""Add a non-colliding picture to the measured Base 1 entrance-hall wall."""
import json
from pathlib import Path
import shutil
import numpy as np
from PIL import Image
import scene

ROOT=Path(__file__).resolve().parents[2]
source=ROOT/'local-assets/broadside-clone/source-v1'
target=ROOT/'local-assets/broadside-clone/source-donut-v1'
if target.exists(): raise ValueError('Refusing overwrite')
shutil.copytree(source,target)
world=json.loads((target/'world.json').read_text())
base=next(b for b in world['reference_bases'] if b['name']=='Base 1')
basis=np.linalg.inv(np.array(base['world_to_local']))
origin=np.array(base['position'])
layer=world['texture_count'];world['texture_count']+=1
scene.save_json(target/'world.json',world)
with Image.open(ROOT/'tools/broadside_clone/art/donut.png') as im:
    picture=im.convert('RGBA')
    for mip in range(9):
        side=256>>mip
        picture.resize((side,side),Image.Resampling.LANCZOS).save(target/'materials'/f'layer-{layer:03d}-mip-{mip}.png')
# Source-local wall plane forward=30.5, bottom=.5, top=6.0. Picture
# is 3m square, offset 3cm toward the viewer to prevent z-fighting.
points=[(-1.5,30.47,1.8),(1.5,30.47,1.8),(1.5,30.47,4.8),(-1.5,30.47,4.8)]
uv=[(0,1),(1,1),(1,0),(0,0)]
normal=basis@np.array([0.,-1.,0.])
path=target/'surfaces.jsonl'
with path.open() as stream: count=sum(1 for _ in stream)
with path.open('a') as stream:
    for delta,ids in enumerate(((0,1,2),(0,2,3))):
        corners=[scene.vertex([*(origin+basis@np.array(points[i])),*normal,*uv[i],0,0,layer,-1]) for i in ids]
        stream.write(json.dumps({'id':count+delta,'corners':corners},separators=(',',':'))+'\n')
scene.save_json(target/'donut-placement.json',{'base':'Base 1','center_local':[0,30.47,3.3],
    'size_metres':[3,3],'collision':'unchanged','wall_collision_ids':[12103,12104],
    'prompt':'Square poster: pink-frosted donut with colorful sprinkles, top-down, cream background; no text or frame.',
    'generation':'built-in image generation'})
report=scene.encode(target,ROOT/'local-assets/broadside-clone/compiled-donut-v1')
assert all(report['payload_identity'][name] for name in ('collision.bin','height.bin','weights.rgba','ambient.f32'))
old=(ROOT/'local-assets/broadside-clone/compiled-v1/vertices.bin').read_bytes()
new=(ROOT/'local-assets/broadside-clone/compiled-donut-v1/vertices.bin').read_bytes()
assert new[:len(old)]==old and len(new)-len(old)==288
print('Verified: exactly two added triangles; old vertices and all collision unchanged.')
