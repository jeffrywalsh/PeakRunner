#!/usr/bin/env python3
"""Original tower/generator/ship-platform/turret-pod CTF base, using
PeakRunner's original material kit. Modeled on the retired Skybreak build; this
replaces the concept the Broadside Clone reference layout is being retired
in favor of, so it stays fully original (no Torque/DIF-derived geometry).
"""
import importlib.util
import json
from pathlib import Path
import math
import struct
import sys
from assets import tower_complex
from assets import tower_complex_materials
from assets import tower_complex_terrain
from assets import landing_pad
from assets import pack_writer

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
    return pack_writer.sample_height(grid, x, z)


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
    shared = kit.base_pack(); old = shared['manifest']
    textures = bytearray(shared['textures'])
    count = old['texture_count']
    pack_writer.paint_materials(textures, count, ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark'],
                                tower_complex_materials.texture, spec['seed'], kit)
    vertices = mesh.vertices.tobytes()
    lightmap = None
    if bake:
        import time
        started = time.time()
        vertices, textures, count, lightmap = pack_writer.bake_lightmaps(vertices, textures, count, mesh.lamps, kit)
        bake_seconds = round(time.time()-started, 1)
    files = {'height.bin':bytes(heights), 'vertices.bin':vertices,
             'collision.bin':mesh.collision.tobytes(),
             'weights.rgba':bytes(weights),
             'textures.rgba':bytes(textures),
             'ambient.f32':shared['ambient']}
    manifest={k:old[k] for k in ['terrain_layers','sky_layers','water_layer','water']}
    manifest['texture_count']=count
    if lightmap: manifest['lightmap']=lightmap
    manifest.update(version=1, id=spec['id'], name=spec['name'], flags=flags, spawns=spawns,
        exact_spawns=True, spawn_points=spawn_points, holes=[], entities=mesh.entities, instances=instances, ambient_emitters=[],
        sky={'visibleDistance':'2500','fogDistance':'1500'},
        asset_sha256=pack_writer.source_hash(tower_complex.__file__),
        pad_asset_sha256=pack_writer.source_hash(landing_pad.__file__),
        terrain_source_sha256=pack_writer.source_hash(tower_complex_terrain.__file__),
        material_source_sha256=pack_writer.source_hash(tower_complex_materials.__file__),
        provenance='PeakRunner original tower-complex geometry, original procedural terrain and original material kit; no extracted assets or external height data',
        definition_sha256=pack_writer.source_hash(ROOT/'maps/tower-complex.json'))
    pack_writer.write_pack(output, files, manifest)
    print(f'Built {spec["name"]}: {len(mesh.collision)//9} solid triangles'
          + (f', {lightmap["pages"]} lightmap pages in {bake_seconds} s' if lightmap else ', unbaked'))

if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if a != '--no-bake']
    build(Path(args[0]) if args else ROOT/'assets/maps/tower-complex', bake='--no-bake' not in sys.argv)
