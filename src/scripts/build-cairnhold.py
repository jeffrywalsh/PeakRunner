#!/usr/bin/env python3
"""Original Cairnhold CTF map: dug-in stone bunkers, covered trenches up to
exposed flag stands, flank plasma batteries, landing/deploy ledges and the
Ring on a central mesa, over original rugged terrain. No extracted assets,
no external height data.

Usage (from src/, numpy venv): build-cairnhold.py [OUTPUT] [--no-bake]
Default output is the embedded assets/maps/cairnhold; refuses to overwrite.
"""
import importlib.util
import json
import math
from pathlib import Path
import struct
import sys

import numpy as np

from assets import cairnhold_base
from assets import cairnhold_materials
from assets import cairnhold_ring
from assets import cairnhold_terrain
from assets import pack_writer
from assets import structure_kit

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.util.spec_from_file_location('kit', ROOT/'scripts/build-original-map.py')
kit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(kit)
STEP = cairnhold_terrain.STEP


def spec():
    return json.loads((ROOT/'maps/cairnhold.json').read_text())


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
    """(surface, weight) pairs over world grids for every structure site."""
    out = []
    smooth = cairnhold_terrain._smooth
    for base in spec['bases']:
        oy = base['position'][1]
        for _, shape, surface, falloff in cairnhold_base.sites():
            def site(shape=shape, surface=surface, falloff=falloff, base=base, oy=oy):
                def local(x, z): return to_local(base, x, z)
                def surf(x, z):
                    lx, lz = local(x, z)
                    return np.vectorize(surface, otypes=[float])(lx, lz)+oy
                def weight(x, z):
                    lx, lz = local(x, z)
                    return 1-smooth(0, falloff, _shape_distance(shape, lx, lz))
                return surf, weight
            out.append(site())
    rx, ry, rz = spec['ring']['position']
    out.append((lambda x, z: np.full_like(x, ry-.06),
                lambda x, z: 1-smooth(0, 26, _shape_distance(('disc', rx, rz, cairnhold_ring.SITE_R), x, z))))
    return out


def holes_and_rings(spec):
    """Terrain cells cut under the dug-in structures, and the pinned heights
    of every grid vertex on the boundary of the cut."""
    cells, region_of = set(), {}
    for bi, base in enumerate(spec['bases']):
        for name, (x0, x1, z0, z1) in cairnhold_base.HOLES.items():
            (ax, az), (bx, bz) = to_world(base, x0, z0), to_world(base, x1, z1)
            wx0, wx1, wz0, wz1 = min(ax, bx), max(ax, bx), min(az, bz), max(az, bz)
            for v in (wx0, wx1, wz0, wz1):
                if abs(v/STEP-round(v/STEP)) > 1e-9: raise ValueError(f'{name} is not on the 8 m grid')
            for iz in range(round(wz0/STEP), round(wz1/STEP)):
                for ix in range(round(wx0/STEP), round(wx1/STEP)):
                    cells.add((ix, iz)); region_of.setdefault((ix, iz), []).append((bi, name))
    rings = {}
    for ix, iz in cells:
        for vx, vz in ((ix, iz), (ix+1, iz), (ix, iz+1), (ix+1, iz+1)):
            touching = [(vx-dx, vz-dz) for dx in (0, 1) for dz in (0, 1)]
            if all(c in cells for c in touching) or (vx, vz) in rings: continue
            heights = []
            for c in touching:
                for bi, name in region_of.get(c, []):
                    base = spec['bases'][bi]
                    lx, lz = to_local(base, vx*STEP, vz*STEP)
                    heights.append(cairnhold_base.ring_height(name, lx, lz)+base['position'][1])
            rings[(vx, vz)] = min(heights)
    holes = sorted(iz*256+ix for ix, iz in cells)
    return holes, rings


def terrain_grid(spec):
    holes, rings = holes_and_rings(spec)
    return cairnhold_terrain.heights(spec['seed'], terrain_sites(spec), rings), holes


def terrain_height(x, z, grid):
    return pack_writer.sample_height(grid, x, z)


def build(output, bake=True):
    if output.exists(): raise ValueError('Refusing to overwrite existing pack')
    definition = spec()
    mesh = kit.Mesh(); mesh.lamps = []
    flags, spawns, spawn_points, instances = [], [], [], []
    for base in definition['bases']:
        team = base['team']
        mesh.origin = tuple(base['position']); mesh.yaw = math.radians(base['yaw'])
        anchors = cairnhold_base.build(mesh, team, f'base-{team}')
        flags.append(mesh.point(anchors['flag']))
        spawns.append(mesh.point(anchors['spawn']))
        spawn_points.append([[*mesh.point(p), (yaw+mesh.yaw) % (2*math.pi)] for *p, yaw in anchors['spawn_points']])
        instances.append(dict(asset=cairnhold_base.ASSET_ID, **base,
            anchors={name: [mesh.point(p) for p in value] if isinstance(value, list) else mesh.point(value)
                     for name, value in anchors.items() if name != 'spawn_points'}))
    base_triangles = len(mesh.collision)//9
    mesh.origin = tuple(definition['ring']['position']); mesh.yaw = 0
    anchors = cairnhold_ring.build(mesh)
    instances.append(dict(asset=cairnhold_ring.ASSET_ID, **definition['ring'],
                          anchors={k: [mesh.point(p) for p in v] if isinstance(v, list) else mesh.point(v)
                                   for k, v in anchors.items()}))
    grid, holes = terrain_grid(definition)
    heights = bytearray(struct.pack('<65536H', *[round(float(v)*32) for v in grid.ravel()]))
    weights = cairnhold_terrain.weights(grid).tobytes()
    if sys.byteorder != 'little': mesh.vertices.byteswap(); mesh.collision.byteswap()
    shared = ROOT/'assets/maps/raindance'
    textures = bytearray((shared/'textures.rgba').read_bytes())
    count = json.loads((shared/'map.json').read_text())['texture_count']
    pack_writer.paint_materials(textures, count, ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark'],
                                cairnhold_materials.texture, definition['seed'], kit)
    vertices = mesh.vertices.tobytes()
    lightmap = None
    if bake:
        import time
        started = time.time()
        vertices, textures, count, lightmap = pack_writer.bake_lightmaps(vertices, textures, count, mesh.lamps, kit)
        bake_seconds = round(time.time()-started, 1)
    files = {'height.bin': bytes(heights), 'vertices.bin': vertices,
             'collision.bin': mesh.collision.tobytes(), 'weights.rgba': bytes(weights),
             'textures.rgba': bytes(textures), 'ambient.f32': (shared/'ambient.f32').read_bytes()}
    old = json.loads((shared/'map.json').read_text())
    manifest = {k: old[k] for k in ['terrain_layers', 'sky_layers', 'water_layer', 'water']}
    manifest['texture_count'] = count
    if lightmap: manifest['lightmap'] = lightmap
    manifest.update(version=1, id=definition['id'], name=definition['name'], flags=flags, spawns=spawns,
        exact_spawns=True, spawn_points=spawn_points, holes=holes, entities=mesh.entities,
        instances=instances, ambient_emitters=[],
        sky={'visibleDistance': '2500', 'fogDistance': '1500'},
        asset_sha256=pack_writer.source_hash(cairnhold_base.__file__),
        ring_asset_sha256=pack_writer.source_hash(cairnhold_ring.__file__),
        structure_kit_sha256=pack_writer.source_hash(structure_kit.__file__),
        terrain_source_sha256=pack_writer.source_hash(cairnhold_terrain.__file__),
        material_source_sha256=pack_writer.source_hash(cairnhold_materials.__file__),
        provenance='PeakRunner original Cairnhold geometry, original procedural terrain and original material kit; '
                   'no extracted assets or external height data',
        definition_sha256=pack_writer.source_hash(ROOT/'maps/cairnhold.json'))
    pack_writer.write_pack(output, files, manifest)
    print(f'Built {definition["name"]}: {len(mesh.collision)//9} solid triangles '
          f'({base_triangles//2} per base, {len(mesh.collision)//9-base_triangles} Ring), {len(holes)} terrain holes'
          + (f', {lightmap["pages"]} lightmap pages in {bake_seconds} s' if lightmap else ', unbaked'))


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if a != '--no-bake']
    build(Path(args[0]) if args else ROOT/'assets/maps/cairnhold', bake='--no-bake' not in sys.argv)
