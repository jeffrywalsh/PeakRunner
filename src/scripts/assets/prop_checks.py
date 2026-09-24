"""Shared checks every map suite runs on its committed pack's props
(assets/props.py) and terrain shade map (assets/terrain_shade.py)."""
import hashlib
import json
import math
from pathlib import Path

import numpy as np

from assets import props, terrain_shade


def check_pack(test, pack_dir):
    pack = Path(pack_dir)
    m = json.loads((pack/'map.json').read_text())
    p = m['props']
    flags = m['flags']; cps = m.get('control_points', [])
    spawns = [s for team in m.get('spawn_points', []) for s in team]
    protect = props.Protection(b'', flags, m.get('spawn_points', []), cps)
    terrain = props.Terrain((pack/'height.bin').read_bytes(), (pack/'weights.rgba').read_bytes(), m['holes'])

    # Budget: props' solid triangles are the tail of collision.bin.
    test.assertLessEqual(p['solid_triangles'], props.PROP_COLLISION_TRIS)
    collision = np.frombuffer((pack/'collision.bin').read_bytes(), '<f4').reshape(-1, 3, 3)
    solid = collision[len(collision)-p['solid_triangles']:] if p['solid_triangles'] else collision[:0]

    # Big props: clear of flags, spawns, rings and ski lanes, sunk, mirrored.
    big = p['big']
    test.assertGreater(len(big), 0)
    test.assertEqual(len(big) % 2, 0)
    cx = (flags[0][0]+flags[1][0])/2; cz = (flags[0][2]+flags[1][2])/2
    for x, y, z, kind, size in big:
        foot = props.FOOT[kind]*size
        for f in flags: test.assertGreaterEqual(math.hypot(x-f[0], z-f[2]), props.BASE_CLEAR, (kind, x, z))
        for s in spawns: test.assertGreaterEqual(math.hypot(x-s[0], z-s[2]), props.SPAWN_CLEAR, (kind, x, z))
        for c in cps:
            test.assertGreaterEqual(math.hypot(x-c['pos'][0], z-c['pos'][2]), c.get('radius', 12)+props.RING_CLEAR, (kind, x, z))
        test.assertGreaterEqual(protect.lane_distance(x, z), props.LANE_HALF_WIDTH, (kind, x, z))
        test.assertLessEqual(y, terrain.height(x, z)+1e-3, f'{kind} at {x},{z} would hover')
        test.assertFalse(terrain.near_hole(x, z), (kind, x, z))
    for (x, _, z, kind, size), (mx, _, mz, mkind, msize) in zip(big[0::2], big[1::2]):
        test.assertEqual((kind, size), (mkind, msize))
        test.assertAlmostEqual(x+mx, 2*cx, delta=.05); test.assertAlmostEqual(z+mz, 2*cz, delta=.05)
    # Every solid prop triangle stays off the ski lanes.
    for tri in solid:
        c = tri.mean(0)
        test.assertGreaterEqual(protect.lane_distance(float(c[0]), float(c[2])), props.LANE_HALF_WIDTH-4, c)

    check_cover(test, pack, m, protect, terrain)
    # Ground layer: the grass budget is spent, and never exceeded.
    test.assertGreaterEqual(p['ground_triangles'], props.GROUND_TRIS*.9)
    test.assertLessEqual(p['ground_triangles'], props.GROUND_TRIS+64)

    # Shade map: present, hashed, right size, with real shadow and occlusion.
    shade = (pack/'shade.rg').read_bytes()
    test.assertEqual(m['files']['shade.rg'], hashlib.sha256(shade).hexdigest())
    a = np.frombuffer(shade, np.uint8).reshape(terrain_shade.SIZE, terrain_shade.SIZE, 2)
    test.assertLess(int(a[..., 0].min()), 40, 'no fully shadowed texel')
    # Steep maps under a 35 degree sun shade a quarter of their ground (Frostline).
    test.assertGreater(float(a[..., 0].mean()), 170, 'most ground is lit')
    test.assertLess(int(a[..., 1].min()), 245, 'no ambient occlusion anywhere')
    test.assertEqual(m['terrain_shade']['size'], terrain_shade.SIZE)


MIN_COVER = 12


def _local(c, lx, lz):
    """World (x, z) of a point in a cover piece's local frame (long axis x)."""
    yaw = c[6]; co, s = math.cos(yaw), math.sin(yaw)
    return c[0]+lx*co+lz*s, c[2]-lx*s+lz*co


def check_cover(test, pack_dir, m, protect, terrain):
    """Deliberate cover (props.place_cover): enough pieces, mirrored, clear of
    lanes and protected space, blocks a crouching shot, and never traps a
    player standing beside it."""
    from assets import route_checks
    cover = m['props']['cover']
    flags = m['flags']; cps = m.get('control_points', [])
    spawns = [s for team in m.get('spawn_points', []) for s in team]
    test.assertGreaterEqual(len(cover), MIN_COVER)
    test.assertEqual(len(cover) % 2, 0)
    cx = (flags[0][0]+flags[1][0])/2; cz = (flags[0][2]+flags[1][2])/2
    for a, b in zip(cover[0::2], cover[1::2]):
        test.assertEqual((a[3], a[4], a[5]), (b[3], b[4], b[5]))
        test.assertAlmostEqual(a[0]+b[0], 2*cx, delta=.05); test.assertAlmostEqual(a[2]+b[2], 2*cz, delta=.05)
    pk = route_checks.Pack(pack_dir)
    for c in cover:
        x, _, z, kind, length, height, yaw, hx, hz = c
        for lx in (-hx, 0.0, hx):
            px, pz = _local(c, lx, 0)
            test.assertGreaterEqual(protect.lane_distance(px, pz), props.LANE_HALF_WIDTH, c)
            for f in flags: test.assertGreaterEqual(math.hypot(px-f[0], pz-f[2]), props.BASE_CLEAR, c)
            for s in spawns: test.assertGreaterEqual(math.hypot(px-s[0], pz-s[2]), props.SPAWN_CLEAR, c)
            for q in cps:
                test.assertGreaterEqual(math.hypot(px-q['pos'][0], pz-q['pos'][2]), q.get('radius', 12)+props.RING_CLEAR, c)
            test.assertFalse(terrain.near_hole(px, pz), c)
        # A shot at crouch height across the piece's short axis is stopped.
        g = pk.terrain(x, z)
        for lz in (-(hz+3.0),):
            ax, az = _local(c, 0, lz); bx, bz = _local(c, 0, -lz)
            starts = np.array([[ax, g+.9, az]]); ends = np.array([[bx, g+.9, bz]])
            test.assertTrue(pk.blocked(starts, ends, r=2)[0], f'{kind} at {x:.0f},{z:.0f} does not stop a shot')
        # Nobody standing beside it is trapped: every standable spot one metre
        # outside its footprint can walk three metres straight away from it.
        for k in range(16):
            a = k*math.tau/16
            ox, oz = math.cos(a), math.sin(a)
            # A point on the footprint's outline, pushed a metre out along the
            # face (or corner) normal: where a player can stand against it.
            scale = 1/max(abs(ox)/max(hx, .1), abs(oz)/max(hz, .1))
            bx, bz = ox*scale, oz*scale
            nx = math.copysign(1, ox) if abs(bx) >= hx-1e-6 else 0.0
            nz = math.copysign(1, oz) if abs(bz) >= hz-1e-6 else 0.0
            nl = math.hypot(nx, nz); nx, nz = nx/nl, nz/nl
            lx, lz = bx+nx*1.0, bz+nz*1.0
            px, pz = _local(c, lx, lz); qx, qz = _local(c, lx+nx*3, lz+nz*3)
            ground = pk.terrain(px, pz)
            ys = [y for y in route_checks.floors(pk, px, pz) if abs(y-ground) < 1.5]
            for y in ys:
                qy = pk.terrain(qx, qz)
                test.assertTrue(route_checks.path_clear(pk, [(px, y, pz), (qx, max(y, qy), qz)]),
                                f'player trapped beside {kind} at {px:.1f},{pz:.1f}')
