#!/usr/bin/env python3
"""Original tower/generator/ship-platform/turret-pod CTF base, using
PeakRunner's original material kit. Modeled on build-skybreak.py; this
replaces the concept the Broadside Clone reference layout is being retired
in favor of, so it stays fully original (no Torque/DIF-derived geometry).
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import math
import struct
import sys
from assets import tower_complex
from assets import tower_complex_materials
from assets import lightmap_bake
from assets import tower_complex_terrain
from assets import landing_pad

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)

def terrain_grid(spec):
    """Original rolling terrain (see assets/tower_complex_terrain.py)."""
    return tower_complex_terrain.heights([b['position'] for b in spec['bases']], spec['seed'],
                                         pads=[(p['position'][0], p['position'][2], p['position'][1])
                                               for p in spec.get('pads', [])])

def terrain_height(x, z, grid):
    """Bilinear sample of the grid, matching the engine's heightfield lookup."""
    fx, fz = min(max(x/8, 0), 255), min(max(z/8, 0), 255)
    x0, z0 = int(fx), int(fz); x1, z1 = min(x0+1, 255), min(z0+1, 255)
    tx, tz = fx-x0, fz-z0
    q = lambda ix, iz: round(float(grid[iz, ix])*32)/32
    a = q(x0,z0)+(q(x1,z0)-q(x0,z0))*tx; b = q(x0,z1)+(q(x1,z1)-q(x0,z1))*tx
    return a+(b-a)*tz

# The renderer's fixed map sun (src/map_scene.rs uniform), so baked shadows on
# the structures agree with the terrain's live lighting.
MAP_SUN = (-0.57735, 0.57735, -0.57735)


def build(output, bake=True):
    if output.exists(): raise ValueError('Refusing to overwrite existing pack')
    spec = json.loads((ROOT/'maps/tower-complex.json').read_text())
    mesh = kit.Mesh(); mesh.lamps = []
    flags, spawns, spawn_points = [], [], []
    instances = []
    for base in spec['bases']:
        team = base['team']
        mesh.origin = tuple(base['position']); mesh.yaw = math.radians(base['yaw']); base_yaw = mesh.yaw
        anchors = tower_complex.build(mesh, team, f'base-{team}')
        flags.append(mesh.point(anchors['flag']))
        spawns.append(mesh.point(anchors['spawn']))
        spawn_points.append([[*mesh.point(p), (yaw+base_yaw) % (2*math.pi)] for *p, yaw in anchors['spawn_points']])
        instances.append(dict(asset=tower_complex.ASSET_ID, **base,
            anchors={name: [mesh.point(p) for p in value] if name == 'entrances' else mesh.point(value)
                     for name,value in anchors.items() if name != 'spawn_points'}))
    for pad in spec.get('pads', []):
        mesh.origin = tuple(pad['position']); mesh.yaw = math.radians(pad['yaw'])
        anchors = landing_pad.build(mesh, pad['team'])
        instances.append(dict(asset=landing_pad.ASSET_ID, **pad,
            anchors={name: [mesh.point(p) for p in value] if isinstance(value, list) else mesh.point(value)
                     for name, value in anchors.items()}))
    grid = terrain_grid(spec)
    heights = bytearray(struct.pack('<65536H', *[round(float(v)*32) for v in grid.ravel()]))
    weights = bytearray(tower_complex_terrain.weights(grid).tobytes())
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    shared = ROOT/'assets/maps/raindance'
    textures = bytearray((shared/'textures.rgba').read_bytes())
    # textures.rgba is level-major (every layer at 256, then every layer at
    # 128, ...). Replace the whole mip chain, not just level 0, or distant
    # surfaces sample the shared Raindance texture instead.
    count = json.loads((shared/'map.json').read_text())['texture_count']
    for material in ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark']:
        layer = kit.MATERIALS.index(material)
        img = tower_complex_materials.texture(material, spec['seed']+layer, kit.noise, kit.value_noise)
        side, offset = 256, 0
        while True:
            size = side*side*4
            textures[offset+layer*size:offset+(layer+1)*size] = img
            offset += count*size
            if side == 1: break
            img = kit.downsample(img, side); side //= 2
    vertices = mesh.vertices.tobytes()
    lightmap = None
    if bake:
        import time
        import numpy as np
        started = time.time()
        baked, pages = lightmap_bake.bake(np.frombuffer(vertices, '<f4'), mesh.lamps, MAP_SUN,
                                          kit.MATERIALS.index('light'), count)
        vertices = baked.astype('<f4').tobytes()
        # Append the pages as new layers, keeping the level-major mip layout.
        mips = [[page] for page in pages]
        for chain in mips:
            side = 256
            while side > 1: chain.append(kit.downsample(chain[-1], side)); side //= 2
        merged, side, offset = bytearray(), 256, 0
        for level in range(9):
            size = side*side*4
            merged += textures[offset:offset+count*size]
            for chain in mips: merged += chain[level]
            offset += count*size; side //= 2
        textures = merged
        lightmap = dict(pages=len(pages), first_layer=count, texel_m=.5, sun=list(MAP_SUN),
                        lamps=len(mesh.lamps),
                        baker_sha256=hashlib.sha256(Path(lightmap_bake.__file__).read_bytes()).hexdigest())
        bake_seconds = round(time.time()-started, 1)
        count += len(pages)
    files = {'height.bin':bytes(heights), 'vertices.bin':vertices,
             'collision.bin':mesh.collision.tobytes(),
             'weights.rgba':bytes(weights),
             'textures.rgba':bytes(textures),
             'ambient.f32':(shared/'ambient.f32').read_bytes()}
    old=json.loads((shared/'map.json').read_text())
    manifest={k:old[k] for k in ['terrain_layers','sky_layers','water_layer','water']}
    manifest['texture_count']=count
    if lightmap: manifest['lightmap']=lightmap
    manifest.update(version=1, id=spec['id'], name=spec['name'], flags=flags, spawns=spawns,
        exact_spawns=True, spawn_points=spawn_points, holes=[], entities=mesh.entities, instances=instances, ambient_emitters=[],
        sky={'visibleDistance':'2500','fogDistance':'1500'},
        asset_sha256=hashlib.sha256(Path(tower_complex.__file__).read_bytes()).hexdigest(),
        pad_asset_sha256=hashlib.sha256(Path(landing_pad.__file__).read_bytes()).hexdigest(),
        terrain_source_sha256=hashlib.sha256(Path(tower_complex_terrain.__file__).read_bytes()).hexdigest(),
        material_source_sha256=hashlib.sha256(Path(tower_complex_materials.__file__).read_bytes()).hexdigest(),
        provenance='PeakRunner original tower-complex geometry, original procedural terrain and original material kit; no extracted assets or external height data',
        definition_sha256=hashlib.sha256((ROOT/'maps/tower-complex.json').read_bytes()).hexdigest(),
        files={name:hashlib.sha256(data).hexdigest() for name,data in files.items()})
    output.mkdir(parents=True)
    for name,data in files.items(): (output/name).write_bytes(data)
    (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Built {spec["name"]}: {len(mesh.collision)//9} solid triangles'
          + (f', {lightmap["pages"]} lightmap pages in {bake_seconds} s' if lightmap else ', unbaked'))

if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if a != '--no-bake']
    build(Path(args[0]) if args else ROOT/'assets/maps/tower-complex', bake='--no-bake' not in sys.argv)
