#!/usr/bin/env python3
"""Original floating-fortress CTF map, using PeakRunner's original material kit."""
import hashlib
import importlib.util
import json
from pathlib import Path
import math
import struct
import sys
from assets import floating_fortress as fortress
from assets import fortress_materials
from assets import fortress_rooms
from assets import docking_bay

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)

def terrain_height(x,z):
    ridge=1-math.exp(-((x-1024)/340)**2)
    endcaps=1-math.exp(-((z-1024)/780)**6)
    rugged=0.66+0.22*math.sin(z/91+x/173)+0.12*math.cos((x-z)/47)
    return 95+330*ridge*rugged+160*endcaps+12*math.sin(z/130)*math.cos(x/170)

def build(output, docking=False):
    if output.exists(): raise ValueError('Refusing to overwrite existing pack')
    spec = json.loads((ROOT/'maps/skybreak-bastions.json').read_text())
    mesh = kit.Mesh()
    flags, spawns = [], []
    instances = []
    for base in spec['bases']:
        team = base['team']
        mesh.origin = tuple(base['position']); mesh.yaw = math.radians(base['yaw'])
        anchors = fortress.build(mesh, team, f'base-{team}')
        if docking:
            anchors['docking_bay'] = docking_bay.build(mesh, team)
        flags.append(mesh.point(anchors['flag']))
        spawns.append(mesh.point(anchors['spawn']))
        instances.append(dict(asset=fortress.ASSET_ID, **base,
            anchors={name: [mesh.point(p) for p in value] if name == 'entrances' else mesh.point(value)
                     for name,value in anchors.items()}))
    heights = bytearray(); weights = bytearray()
    for z in range(256):
        for x in range(256):
            xx,zz=x*8,z*8
            # A long central valley with rugged mountain walls, not uniform hills.
            h=terrain_height(xx,zz)
            heights.extend(struct.pack('<H',round(h*32)))
            slope=math.hypot(terrain_height(xx+2,zz)-terrain_height(xx-2,zz),
                             terrain_height(xx,zz+2)-terrain_height(xx,zz-2))/4
            rock=max(0.12,min(.85,(slope-.2)*.65+(h-140)/650))
            soil=.12+.05*math.sin(xx/81)*math.cos(zz/97)
            cover=max(0.,1-rock-soil)
            weights.extend(round(v*255) for v in [cover*.85,rock,soil,cover*.15])
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    shared = ROOT/'assets/maps/raindance'
    textures = bytearray((shared/'textures.rgba').read_bytes())
    layer_bytes = 256*256*4
    for material in ['concrete', 'panel', 'grate', 'trim']:
        layer = kit.MATERIALS.index(material)
        textures[layer*layer_bytes:(layer+1)*layer_bytes] = fortress_materials.texture(
            material, spec['seed']+layer, kit.noise, kit.value_noise)
    files = {'height.bin':bytes(heights), 'vertices.bin':mesh.vertices.tobytes(),
             'collision.bin':mesh.collision.tobytes(),
             'weights.rgba':bytes(weights),
             'textures.rgba':bytes(textures),
             'ambient.f32':(shared/'ambient.f32').read_bytes()}
    old=json.loads((shared/'map.json').read_text())
    manifest={k:old[k] for k in ['texture_count','terrain_layers','sky_layers','water_layer','water']}
    manifest.update(version=1, id=spec['id'], name=spec['name'], flags=flags, spawns=spawns,
        exact_spawns=True, holes=[], entities=mesh.entities, instances=instances, ambient_emitters=[],
        sky={'visibleDistance':'2500','fogDistance':'1500'},
        asset_sha256=hashlib.sha256(Path(fortress.__file__).read_bytes()).hexdigest(),
        material_source_sha256=hashlib.sha256(Path(fortress_materials.__file__).read_bytes()).hexdigest(),
        rooms_source_sha256=hashlib.sha256(Path(fortress_rooms.__file__).read_bytes()).hexdigest(),
        provenance='PeakRunner original floating fortress geometry and original material kit; no extracted assets',
        definition_sha256=hashlib.sha256((ROOT/'maps/skybreak-bastions.json').read_bytes()).hexdigest(),
        files={name:hashlib.sha256(data).hexdigest() for name,data in files.items()})
    if docking:
        manifest['name'] += ' — docking prototype'
        manifest['docking_source_sha256'] = hashlib.sha256(Path(docking_bay.__file__).read_bytes()).hexdigest()
    output.mkdir(parents=True)
    for name,data in files.items(): (output/name).write_bytes(data)
    (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Built {spec["name"]}: {len(mesh.collision)//9} solid triangles')

if __name__ == '__main__':
    build(Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'assets/maps/skybreak-bastions', '--docking' in sys.argv)
