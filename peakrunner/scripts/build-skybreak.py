#!/usr/bin/env python3
"""Original floating-fortress CTF map, using PeakRunner's original material kit."""
import hashlib
import importlib.util
import json
from pathlib import Path
import math
import struct
import sys

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)

def build(output):
    if output.exists(): raise ValueError('Refusing to overwrite existing pack')
    spec = json.loads((ROOT/'maps/skybreak-bastions.json').read_text())
    mesh = kit.Mesh()
    flags, spawns = [], []
    for base in spec['bases']:
        team = base['team']; color = ['ember', 'glacier'][team]
        mesh.origin = tuple(base['position']); mesh.yaw = math.radians(base['yaw'])
        # Main deck and lower armored hull; all surfaces share render/collision.
        mesh.box((0,-2,0),(88,4,112),'panel')
        mesh.box((0,-7,0),(68,6,88),'trim')
        mesh.box((0,-12,0),(46,4,64),'panel')
        # Side walls have broad entrances, and front has a recessed hangar mouth.
        for x in [-38,38]:
            for z in [-32,34]: mesh.box((x,7,z),(3,14,34),'concrete')
            mesh.box((x,13,0),(3,2,30),color)
        mesh.box((0,7,49),(78,14,3),'concrete')
        for x in [-28,28]: mesh.box((x,7,-45),(22,14,3),'concrete')
        mesh.box((0,13,-45),(34,2,3),color)
        # Roof split around a central jet hatch. Flag is under solid rear roof.
        for x in [-27,27]: mesh.box((x,15,0),(26,2,100),'panel')
        for z in [-34,34]: mesh.box((0,15,z),(28,2,32),'panel')
        for x in [-39,39]: mesh.box((x,17,0),(2,2,100),color)
        # Interior flag alcove is screened from the front, accessible both sides.
        mesh.box((0,5,5),(26,10,2),'concrete')
        for x in [-16,16]: mesh.box((x,5,24),(2,10,22),'panel')
        mesh.box((0,.3,27),(10,.6,10),color)
        flags.append(mesh.point((0,1.05,27)))
        spawns.append(mesh.point((-26,1.2,31)))
        # Exterior landing wings and ascending pads provide recovery routes.
        for x in [-51,51]:
            mesh.box((x,-.5,0),(20,1,26),'grate')
            mesh.box((x,-16,-20),(20,2,22),'panel')
            mesh.box((x,-32,-44),(22,2,24),'panel')
            mesh.column((x,-9,0),5,5,color,top=3)
        for x in [-25,25]:
            mesh.column((x,-22,25),9,8,'trim',top=6)
            mesh.column((x,-23,25),6,1,'light',solid=False)
        circuit = f'base-{team}'
        for kind,p in [('generator',(26,0,32)),('inventory',(-26,0,16)),('repair',(26,0,16))]:
            mesh.equipment(kind,p,team,circuit)
    heights = bytearray()
    for z in range(256):
        for x in range(256):
            xx,zz=x*8,z*8
            h=110+18*math.sin(xx/130)*math.cos(zz/160)+8*math.sin((xx+zz)/61)
            heights.extend(struct.pack('<H',round(h*32)))
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    shared = ROOT/'assets/maps/raindance'
    files = {'height.bin':bytes(heights), 'vertices.bin':mesh.vertices.tobytes(),
             'collision.bin':mesh.collision.tobytes(),
             'weights.rgba':bytes([75,150,30,0])*(256*256),
             'textures.rgba':(shared/'textures.rgba').read_bytes(),
             'ambient.f32':(shared/'ambient.f32').read_bytes()}
    old=json.loads((shared/'map.json').read_text())
    manifest={k:old[k] for k in ['texture_count','terrain_layers','sky_layers','water_layer','water']}
    manifest.update(version=1, id=spec['id'], name=spec['name'], flags=flags, spawns=spawns,
        exact_spawns=True, holes=[], entities=mesh.entities, ambient_emitters=[],
        sky={'visibleDistance':'2000','fogDistance':'900'},
        provenance='PeakRunner original floating fortress geometry and original material kit; no extracted assets',
        definition_sha256=hashlib.sha256((ROOT/'maps/skybreak-bastions.json').read_bytes()).hexdigest(),
        files={name:hashlib.sha256(data).hexdigest() for name,data in files.items()})
    output.mkdir(parents=True)
    for name,data in files.items(): (output/name).write_bytes(data)
    (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Built {spec["name"]}: {len(mesh.collision)//9} solid triangles')

if __name__ == '__main__':
    build(Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'assets/maps/skybreak-bastions')
