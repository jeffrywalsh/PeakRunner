#!/usr/bin/env python3
"""Build the Raindance pack: the original kit layout with the cleaned base
and field structures, Raindance materials and baked lighting.

Usage: build-raindance.py [OUTPUT] [--no-bake]
Default OUTPUT is assets/maps/raindance; refuses to overwrite. Terrain,
scenery placement and equipment come from maps/raindance.json and the kit
(scripts/build-original-map.py), exactly as the original kit build placed them.
"""
import importlib.util
import json
import math
from pathlib import Path
import random
import struct
import sys

from assets import pack_writer
from assets import raindance_base
from assets import raindance_materials
from assets import raindance_structures

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)

PAINTED = ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light']


def spec():
    definition = json.loads((ROOT/'maps/raindance.json').read_text())
    kit.validate(definition)
    return definition


def render_triangles(vertices):
    """Drop zero-area render triangles (the kit's broadleaf canopies collapse
    to a point at each pole). They draw nothing, and a lightmap chart for a
    triangle without a normal would bake NaN texels."""
    import numpy as np
    tris = np.frombuffer(vertices, '<f4').reshape(-1, 3, 12)
    p = tris[:, :, :3].astype(np.float64)
    area = np.linalg.norm(np.cross(p[:, 1]-p[:, 0], p[:, 2]-p[:, 0]), axis=1)
    return tris[area > 1e-9].tobytes()


def place(mesh, obj, definition):
    """One authored field object, as the kit places it."""
    team = obj.get('team', 0)
    circuit = next(b['id'] for b in definition['bases'] if b['team'] == team)
    asset = obj['asset']
    if asset == 'tower': mesh.tower(team, circuit)
    elif asset == 'bunker': raindance_structures.bunker(mesh)
    elif asset == 'landing_pad': return raindance_structures.landing_pad(mesh, team)
    elif asset == 'bridge': mesh.bridge()
    elif asset in ['inventory', 'generator', 'sensor', 'turret', 'repair']:
        mesh.equipment(asset, (0, 0, 0), team, obj.get('circuit', circuit), obj.get('weapon', 'bullet'))
    else: raise ValueError(f'Unknown field asset {asset}')
    return {}


def build(output, bake=True):
    if output.exists(): raise ValueError(f'Refusing to overwrite {output}; use a new output directory')
    d = spec()
    rng = random.Random(d['seed'])
    mesh = kit.Mesh(); mesh.lamps = []
    flags, spawns, spawn_points = [], [], []
    for b in sorted(d['bases'], key=lambda b: b['team']):
        mesh.origin = tuple(b['position']); mesh.yaw = math.radians(b.get('yaw', 0))
        anchors = raindance_base.build(mesh, b['team'], b['id'], d['base_equipment'])
        flags.append(mesh.point(anchors['flag'])); spawns.append(mesh.point(anchors['spawn']))
        spawn_points.append([[*mesh.point(p), (yaw+mesh.yaw) % math.tau] for *p, yaw in anchors['spawn_points']])
        mesh.instances.append(dict(asset='base', **b, anchors={
            k: [mesh.point(p) for p in v] if isinstance(v, list) else mesh.point(v)
            for k, v in anchors.items() if k != 'spawn_points'}))
    base_triangles = len(mesh.collision)//9
    for obj in d['objects']:
        p = list(obj['position'])
        if obj.get('grounded'): p[1] = kit.height(p[0], p[2], d)
        mesh.origin = tuple(p); mesh.yaw = math.radians(obj.get('yaw', 0))
        anchors = place(mesh, obj, d)
        mesh.instances.append(dict(obj, position=p, **({'anchors': {
            k: [mesh.point(q) for q in v] if isinstance(v, list) else mesh.point(v)
            for k, v in anchors.items()}} if anchors else {})))
    # Scenery: the kit's seeded trees and rocks, placed exactly as before.
    for asset, count in [('tree', d['scenery']['trees']), ('rock', d['scenery']['rocks'])]:
        placed = 0
        for _ in range(count*40):
            x, z = rng.uniform(180, 1860), rng.uniform(200, 1840)
            y = kit.height(x, z, d)
            if y < 65 or any(math.hypot(x-b['position'][0], z-b['position'][2]) < 140 for b in d['bases']): continue
            if any(math.hypot(x-o['position'][0], z-o['position'][2]) < 35 for o in d['objects']): continue
            if abs(x-1000) < 35 and abs(z-940) < 150: continue
            mesh.origin = (x, y, z); mesh.yaw = rng.random()*math.tau
            getattr(mesh, asset)(rng); mesh.instances.append(dict(asset=asset, position=mesh.origin, yaw=math.degrees(mesh.yaw)))
            placed += 1
            if placed == count: break
    heights, weights, holes = bytearray(), bytearray(), []
    for iz in range(256):
        for ix in range(256):
            x, z = ix*8, iz*8; h = kit.height(x, z, d)
            heights.extend(struct.pack('<H', round(h*32)))
            slope = math.hypot(kit.height(x+2, z, d)-kit.height(x-2, z, d), kit.height(x, z+2, d)-kit.height(x, z-2, d))/4
            rock = kit.smooth((slope-.25)/.8); soil = (1-rock)*(.2+.16*math.sin(x/70)*math.cos(z/100))
            moss = (1-rock-soil)*(.2+.15*math.sin((x+z)/90)); grass = 1-rock-soil-moss
            weights.extend(round(w*255) for w in [grass, rock, soil, moss])
            cx, cz = x+4, z+4
            for b in d['bases']:
                bx, by, bz = b['position']; a = -math.radians(b.get('yaw', 0))
                lx = (cx-bx)*math.cos(a)+(cz-bz)*math.sin(a); lz = -(cx-bx)*math.sin(a)+(cz-bz)*math.cos(a)
                lx, lz = round(lx, 6), round(lz, 6)
                if abs(lx) < 36 and -56 < lz < 32:
                    holes.append(iz*256+ix); break
    # Aprons covering the grid-snapped edge of each basement cut.
    for b in d['bases']:
        mesh.origin = tuple(b['position']); mesh.yaw = math.radians(b.get('yaw', 0))
        for x in [-34, 34]: mesh.box((x, -.27, -12), (4, .6, 88), 'concrete')
        # The rear strip's east end is the service shed (raindance_base).
        rear_end = raindance_base.CEIL_END
        mesh.box(((rear_end-32)/2, -.27, 30), (rear_end+32, .6, 4), 'concrete')
        for x in [-21, 21]: mesh.box((x, -.27, -40), (22, .6, 24), 'concrete')
        mesh.box((0, -.27, -54), (64, .6, 4), 'grate')
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    textures = bytearray(kit.base_textures(d['seed']))
    manifest = kit.layer_manifest(d['environment']['water_height'])
    count = manifest['texture_count']
    pack_writer.paint_materials(textures, count, PAINTED, raindance_materials.texture, d['seed'], kit)
    vertices, lightmap = render_triangles(mesh.vertices.tobytes()), None
    if bake:
        vertices, textures, count, lightmap = pack_writer.bake_lightmaps(vertices, textures, count, mesh.lamps, kit)
        manifest['texture_count'] = count; manifest['lightmap'] = lightmap
    files = {'height.bin': bytes(heights), 'weights.rgba': bytes(weights), 'vertices.bin': vertices,
             'collision.bin': mesh.collision.tobytes(), 'textures.rgba': bytes(textures),
             'ambient.f32': kit.base_ambient(d['seed'])}
    e = d['environment']
    manifest.update(version=1, name=d['name'], flags=flags, spawns=spawns, spawn_points=spawn_points,
        holes=sorted(set(holes)), sky={'visibleDistance': str(e['visibility']), 'fogDistance': str(e['fog_start'])},
        ambient_emitters=[[1000, 100, 940, .35, 200, 2000]], entities=mesh.entities, instances=mesh.instances,
        materials=list(kit.MATERIALS), asset_catalog=list(kit.ASSETS),
        provenance='PeakRunner original procedural kit v2 (cleaned Raindance); no extracted assets',
        base_asset=raindance_base.ASSET_ID, structures_asset=raindance_structures.ASSET_ID,
        base_asset_sha256=pack_writer.source_hash(raindance_base.__file__),
        structures_asset_sha256=pack_writer.source_hash(raindance_structures.__file__),
        material_source_sha256=pack_writer.source_hash(raindance_materials.__file__),
        definition_sha256=pack_writer.source_hash(ROOT/'maps/raindance.json'))
    pack_writer.write_pack(output, files, manifest)
    print(f'Built {d["name"]}: {len(mesh.collision)//9} solid triangles ({base_triangles//2} per base), '
          f'{len(files["vertices.bin"])//144} render triangles, {len(mesh.entities)} equipment objects'
          + (f', {lightmap["pages"]} lightmap pages' if lightmap else ', unbaked'))


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if a != '--no-bake']
    build(Path(args[0]) if args else ROOT/'assets/maps/raindance', bake='--no-bake' not in sys.argv)
