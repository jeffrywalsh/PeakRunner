#!/usr/bin/env python3
"""Original Frostline CTF map: two polar research stations on mountain
shelves in a whiteout, each with a relay outpost, a plasma emplacement and a
vehicle apron, a lit navigation beacon on the central ridge and snow pines,
over original steep snow terrain. No extracted assets, no external height data.

Usage (from src/, numpy venv): build-frostline.py [OUTPUT] [--no-bake]
Default output is the embedded assets/maps/frostline; refuses to overwrite.
"""
import importlib.util
import json
import math
from pathlib import Path
import struct
import sys

import numpy as np

from assets import frostline_beacon
from assets import frostline_flora
from assets import frostline_materials
from assets import frostline_station
from assets import frostline_terrain
from assets import pack_writer
from assets import structure_kit

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)
STEP = frostline_terrain.STEP
FOG = {'visibleDistance': '700', 'fogDistance': '220'}


def spec():
    return json.loads((ROOT/'maps/frostline.json').read_text())


def to_local(base, x, z):
    """World (x, z) -> base-local (x, z); inverse of kit Mesh.point."""
    ox, _, oz = base['position']; a = math.radians(base['yaw'])
    c, s = math.cos(a), math.sin(a)
    dx, dz = x-ox, z-oz
    return dx*c-dz*s, dx*s+dz*c


def to_world(base, x, z):
    ox, _, oz = base['position']; a = math.radians(base['yaw'])
    c, s = math.cos(a), math.sin(a)
    return ox+x*c+z*s, oz-x*s+z*c


def _shape_distance(shape, x, z):
    if shape[0] == 'rect':
        _, x0, x1, z0, z1 = shape
        return np.hypot(np.maximum(np.maximum(x0-x, x-x1), 0), np.maximum(np.maximum(z0-z, z-z1), 0))
    _, cx, cz, r = shape
    return np.maximum(np.hypot(x-cx, z-cz)-r, 0)


def terrain_sites(spec):
    smooth = frostline_terrain._smooth
    out = []
    for base in spec['bases']:
        oy = base['position'][1]
        for _, shape, level, falloff in frostline_station.sites():
            def site(shape=shape, level=level, falloff=falloff, base=base, oy=oy):
                def surf(x, z): return np.full_like(x, level+oy)
                def weight(x, z):
                    lx, lz = to_local(base, x, z)
                    return 1-smooth(0, falloff, _shape_distance(shape, lx, lz))
                return surf, weight
            out.append(site())
    bx, by, bz = spec['beacon']['position']
    out.append((lambda x, z: np.full_like(x, by-.06),
                lambda x, z: 1-smooth(0, 30, _shape_distance(('disc', bx, bz, frostline_beacon.SITE_R), x, z))))
    return out


def terrain_grid(spec):
    return frostline_terrain.heights(spec['seed'], terrain_sites(spec))


def terrain_height(x, z, grid):
    return pack_writer.sample_height(grid, x, z)


def tree_exclusions(spec):
    out = []
    for base in spec['bases']:
        for (lx, lz), r in (((-8, 0), 62), (frostline_station.OUTPOST, 26), (frostline_station.EMPLACEMENT, 22)):
            out.append((*to_world(base, lx, lz), r))
    bx, _, bz = spec['beacon']['position']
    out.append((bx, bz, 34))
    return out


def trees(spec, grid):
    return frostline_flora.positions(spec['seed'], lambda x, z: terrain_height(x, z, grid),
                                     frostline_flora.slope_sampler(grid), tree_exclusions(spec), kit.noise)


def paint_sky(textures, count, layers, seed):
    """Replace the six sky cube layers' whole mip chains with the whiteout."""
    for face, layer in enumerate(layers[:6]):
        img = frostline_materials.sky(face, seed+face, kit.noise, kit.value_noise)
        side, offset = 256, 0
        while True:
            size = side*side*4
            textures[offset+layer*size:offset+(layer+1)*size] = img
            offset += count*size
            if side == 1: break
            img = kit.downsample(img, side); side //= 2
    return textures


def build(output, bake=True):
    if output.exists(): raise ValueError('Refusing to overwrite existing pack')
    definition = spec()
    mesh = kit.Mesh(); mesh.lamps = []
    flags, spawns, spawn_points, instances = [], [], [], []
    for base in definition['bases']:
        team = base['team']
        mesh.origin = tuple(base['position']); mesh.yaw = math.radians(base['yaw'])
        anchors = frostline_station.build(mesh, team, f'base-{team}')
        flags.append(mesh.point(anchors['flag']))
        spawns.append(mesh.point(anchors['spawn']))
        spawn_points.append([[*mesh.point(p), (yaw+mesh.yaw) % (2*math.pi)] for *p, yaw in anchors['spawn_points']])
        instances.append(dict(asset=frostline_station.ASSET_ID, **base,
            anchors={name: [mesh.point(p) for p in value] if isinstance(value, list) else mesh.point(value)
                     for name, value in anchors.items() if name != 'spawn_points'}))
    base_triangles = len(mesh.collision)//9
    mesh.origin = tuple(definition['beacon']['position']); mesh.yaw = 0
    anchors = frostline_beacon.build(mesh)
    instances.append(dict(asset=frostline_beacon.ASSET_ID, **definition['beacon'],
                          anchors={k: [mesh.point(p) for p in v] if isinstance(v, list) else mesh.point(v)
                                   for k, v in anchors.items()}))
    beacon_triangles = len(mesh.collision)//9-base_triangles
    grid = terrain_grid(definition)
    mesh.origin = (0, 0, 0); mesh.yaw = 0
    pines = trees(definition, grid)
    frostline_flora.build(mesh, pines)
    heights = bytearray(struct.pack('<65536H', *[round(float(v)*32) for v in grid.ravel()]))
    weights = frostline_terrain.weights(grid).tobytes()
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    shared = ROOT/'assets/maps/raindance'
    old = json.loads((shared/'map.json').read_text())
    textures = bytearray((shared/'textures.rgba').read_bytes())
    count = old['texture_count']
    pack_writer.paint_materials(textures, count, ['meadow', 'rock', 'soil', 'moss', 'concrete', 'panel', 'grate', 'trim',
                                                  'ember', 'glacier', 'light', 'bark', 'leaf'],
                                frostline_materials.texture, definition['seed'], kit)
    paint_sky(textures, count, old['sky_layers'], definition['seed']+500)
    vertices = mesh.vertices.tobytes()
    lightmap = None
    if bake:
        vertices, textures, count, lightmap = pack_writer.bake_lightmaps(vertices, textures, count, mesh.lamps, kit)
    files = {'height.bin': bytes(heights), 'vertices.bin': vertices,
             'collision.bin': mesh.collision.tobytes(), 'weights.rgba': bytes(weights),
             'textures.rgba': bytes(textures), 'ambient.f32': (shared/'ambient.f32').read_bytes()}
    manifest = {k: old[k] for k in ['terrain_layers', 'sky_layers', 'water_layer', 'water']}
    manifest['texture_count'] = count
    if lightmap: manifest['lightmap'] = lightmap
    manifest.update(version=1, id=definition['id'], name=definition['name'], flags=flags, spawns=spawns,
        exact_spawns=True, spawn_points=spawn_points, holes=[], entities=mesh.entities,
        instances=instances, ambient_emitters=[], sky=dict(FOG), trees=len(pines),
        asset_sha256=pack_writer.source_hash(frostline_station.__file__),
        beacon_asset_sha256=pack_writer.source_hash(frostline_beacon.__file__),
        flora_source_sha256=pack_writer.source_hash(frostline_flora.__file__),
        structure_kit_sha256=pack_writer.source_hash(structure_kit.__file__),
        terrain_source_sha256=pack_writer.source_hash(frostline_terrain.__file__),
        material_source_sha256=pack_writer.source_hash(frostline_materials.__file__),
        provenance='PeakRunner original Frostline geometry, original procedural terrain, flora and material kit; '
                   'no extracted assets or external height data',
        definition_sha256=pack_writer.source_hash(ROOT/'maps/frostline.json'))
    pack_writer.write_pack(output, files, manifest)
    print(f'Built {definition["name"]}: {len(mesh.collision)//9} solid triangles '
          f'({base_triangles//2} per base, {beacon_triangles} beacon, {len(pines)} pines), '
          f'{len(mesh.vertices)//36} render triangles'
          + (f', {lightmap["pages"]} lightmap pages' if lightmap else ', unbaked'))


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if a != '--no-bake']
    build(Path(args[0]) if args else ROOT/'assets/maps/frostline', bake='--no-bake' not in sys.argv)
