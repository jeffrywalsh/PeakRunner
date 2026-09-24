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

    # Shade map: present, hashed, right size, with real shadow and occlusion.
    shade = (pack/'shade.rg').read_bytes()
    test.assertEqual(m['files']['shade.rg'], hashlib.sha256(shade).hexdigest())
    a = np.frombuffer(shade, np.uint8).reshape(terrain_shade.SIZE, terrain_shade.SIZE, 2)
    test.assertLess(int(a[..., 0].min()), 40, 'no fully shadowed texel')
    # Steep maps under a 35 degree sun shade a quarter of their ground (Frostline).
    test.assertGreater(float(a[..., 0].mean()), 170, 'most ground is lit')
    test.assertLess(int(a[..., 1].min()), 245, 'no ambient occlusion anywhere')
    test.assertEqual(m['terrain_shade']['size'], terrain_shade.SIZE)
