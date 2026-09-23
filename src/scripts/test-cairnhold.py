#!/usr/bin/env python3
"""Test the Cairnhold builders without reading source-game assets.
Run from src/ with the numpy venv: scripts/test-cairnhold.py"""
import hashlib
import importlib.util
import math
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from assets import budgets
from assets import cairnhold_base as base
from assets import cairnhold_materials
from assets import cairnhold_ring
from assets import cairnhold_terrain

HERE = Path(__file__).parent
def _load(name, file):
    loader = importlib.util.spec_from_file_location(name, HERE/file)
    module = importlib.util.module_from_spec(loader); loader.loader.exec_module(module)
    return module
kit = _load('kit', 'build-original-map.py')
build = _load('cairnhold_build', 'build-cairnhold.py')

LIFT = base.SPAWN_LIFT


class Soup:
    """Vectorised ray queries against a collision triangle soup."""
    def __init__(self, collision):
        t = np.array(collision, np.float64).reshape(-1, 3, 3)
        self.a = t[:, 0]; self.e1 = t[:, 1]-t[:, 0]; self.e2 = t[:, 2]-t[:, 0]
        n = np.cross(self.e1, self.e2); self.n = n/(np.linalg.norm(n, axis=1, keepdims=True)+1e-12)
        self.lo = t.min(1); self.hi = t.max(1); self.tris = t

    def hits(self, o, d, tmax=np.inf):
        """Distances and normal.y of every hit along a ray, nearest first."""
        o, d = np.asarray(o, float), np.asarray(d, float)
        end = o+d*min(tmax, 1e4)
        m = np.all(self.hi >= np.minimum(o, end)-1e-6, 1) & np.all(self.lo <= np.maximum(o, end)+1e-6, 1)
        a, e1, e2 = self.a[m], self.e1[m], self.e2[m]
        h = np.cross(d, e2); det = np.einsum('ij,ij->i', e1, h)
        ok = np.abs(det) > 1e-9; det = np.where(ok, det, 1); s = o-a
        u = np.einsum('ij,ij->i', s, h)/det; q = np.cross(s, e1)
        v = (q@d)/det; t = np.einsum('ij,ij->i', e2, q)/det
        k = ok & (u >= -1e-7) & (v >= -1e-7) & (u+v <= 1+1e-7) & (t >= 0) & (t <= tmax)
        order = np.argsort(t[k])
        return t[k][order], self.n[m][k][order, 1]

    def first(self, o, d, tmax=np.inf):
        t, _ = self.hits(o, d, tmax)
        return t[0] if len(t) else np.inf

    def blocked_many(self, starts, ends):
        """For segments start->end (N,3), whether any triangle is crossed."""
        out = np.zeros(len(starts), bool)
        for i in range(0, len(starts), 256):
            s = starts[i:i+256][:, None]; d = ends[i:i+256][:, None]-s
            e1, e2, a = self.e1[None], self.e2[None], self.a[None]
            h = np.cross(d, e2); det = (e1*h).sum(-1)
            ok = np.abs(det) > 1e-9; inv = np.where(ok, 1/np.where(ok, det, 1), 0)
            tv = s-a; u = (tv*h).sum(-1)*inv; q = np.cross(tv, e1)
            v = (d*q).sum(-1)*inv; t = (e2*q).sum(-1)*inv
            out[i:i+256] = (ok & (u >= 0) & (v >= 0) & (u+v <= 1) & (t > 1e-4) & (t < 1-1e-4)).any(1)
        return out


def one_base(team=0, circuit='one', origin=(0, 0, 0), yaw=0.0):
    mesh = kit.Mesh(); mesh.origin = origin; mesh.yaw = yaw
    anchors = base.build(mesh, team, circuit)
    return mesh, anchors


class CairnholdTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mesh, cls.anchors = one_base()
        cls.soup = Soup(cls.mesh.collision)

    # --- Floors, headroom and standing support ----------------------------
    def test_room_floors_have_floor_and_headroom(self):
        samples = [(x, z, 0) for x in (-12, -8, 8, 12) for z in (-13, -6, 2)]      # hall
        samples += [(x, z, 0) for x in (-12, 12) for z in (6, 10)]                  # inventory room
        samples += [(x, 16, 0) for x in (0, 10, 13)]                                # generator room
        samples += [(x, z, base.HUT_FLOOR) for x in (-5, 0, 6) for z in (base.HZ0+4, base.HZ0+12, base.HZ0+20)]  # hut
        samples += [(x, 78, base.HUT_ROOF) for x in (9, 12, 14)]                                    # tower floor
        samples += [(x, z, base.TOWER_MID) for x in (3.5, 9, 14) for z in (93, 94.5)]              # tower landing
        for x, z, y in samples:
            t, ny = self.soup.hits((x, y+3, z), (0, -1, 0))
            self.assertTrue(len(t) and abs(3-t[0]) < 1e-6 and ny[0] > .999, (x, z, 'floor'))
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'headroom'))

    def test_trench_climbs_with_headroom(self):
        for z in np.arange(base.TZ0+.5, base.TZ1, 2.0):
            for x in (2.2, 4, 5.8):
                y = base.trench_floor(z)
                t, ny = self.soup.hits((x, y+2, z), (0, -1, 0))
                self.assertLess(abs(2-t[0]), .02, (x, z))
                self.assertGreater(ny[0], .9, (x, z, 'trench slope'))
                self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 4.0, (x, z, 'trench headroom'))
        self.assertLess(math.degrees(math.atan(base.TRENCH_RISE/(base.TZ1-base.TZ0))), 23)

    def test_stand_ramp_reaches_the_roof_through_the_hatch(self):
        r0, r1, zl, zh = base.RAMP
        rise = base.HUT_ROOF-base.HUT_FLOOR
        for z in np.arange(zh+.5, zl, 1.5):
            y = base.HUT_FLOOR+rise*(zl-z)/(zl-zh)
            t, _ = self.soup.hits(((r0+r1)/2, y+1, z), (0, -1, 0))
            self.assertLess(abs(1-t[0]), .02, (z, 'ramp surface'))
            self.assertGreater(self.soup.first(((r0+r1)/2, y+.2, z), (0, 1, 0)), 2.4, (z, 'ramp headroom'))
        # Nothing can be walked into underneath the ramp: its sides are closed.
        for z in np.arange(zh+1, zl-1, 2.0):
            self.assertLess(self.soup.first((r0-1, base.HUT_FLOOR+.5, z), (1, 0, 0)), 1.01, z)

    def test_standing_support_is_the_floor_top_everywhere(self):
        """The engine's support ray starts 0.15 m above the feet. Wherever a
        player can stand, its first hit must be the up-facing floor top."""
        tx0, tx1, tz0, tz1 = base.TW
        regions = [(-15, 15, -19, 19, 0), (base.HX0+1, base.HX1-1, base.HZ0+1, base.HZ1-1, base.HUT_FLOOR),
                   (base.HX0+.5, base.HX1-.5, base.HZ0+.5, base.HZ1-.5, base.HUT_ROOF),
                   (-15.5, 15.5, -19.5, 19.5, base.ROOF),
                   (tx0+.6, tx1-.6, tz0+.6, tz1-.6, base.TOWER_TOP), (tx0+1, tx1-1, 92.2, 95, base.TOWER_MID),
                   (base.BRIDGE[0]+.6, base.BRIDGE[1], base.BRIDGE[2]+.6, base.BRIDGE[3]-.6, base.TOWER_TOP),
                   (-5.1, 5.1, -21.1, -13.9, base.TIERS[0][2])]
        for s in (-1, 1):
            for x0, x1, z0, z1, level in [(6.2, 10.8, -19.1, -13.2, base.TIERS[1][2]),
                                          (12.6, 15.8, -19.1, -13.2, base.TIERS[2][2]),
                                          (16.2, 23.8, -19.8, -12.2, base.ROOF)]:
                a, c = sorted((s*x0, s*x1)); regions.append((a, c, z0, z1, level))
        px, pz = base.PAD; H = base.PAD_HALF-.6
        regions.append((px-H, px+H, pz-H, pz+H, base.PAD_TOP))
        bx, bz = base.BATTERY; r = base.BATTERY_R*.9
        regions.append((bx-r, bx+r, bz-r, bz+r, base.BATTERY_GROUND+base.BATTERY_H))
        checked = 0
        for x0, x1, z0, z1, level in regions:
            for x in np.arange(x0, x1+.01, .8):
                for z in np.arange(z0, z1+.01, .8):
                    t, ny = self.soup.hits((x, level+2.6, z), (0, -1, 0))
                    if not len(t) or abs(2.6-t[0]) > .02 or np.any((np.abs(t-t[0]) < .01) & (ny < .99)): continue
                    first, fy = self.soup.hits((x, level+.15, z), (0, -1, 0))
                    checked += 1
                    self.assertTrue(len(first) and first[0] < .17 and fy[0] > .99, (x, z, level, first[:2]))
        self.assertGreater(checked, 2500)

    def test_no_accidental_slopes_on_walkable_surfaces(self):
        n, cen = self.soup.n, self.soup.tris.mean(1)
        ents = [e['position'] for e in self.mesh.entities]
        r0, r1, zl, zh = base.RAMP
        bx, bz = base.BATTERY
        for i in np.where((n[:, 1] > .3) & (n[:, 1] < .9999))[0]:
            x, y, z = cen[i]
            if base.TX0 <= x <= base.TX1 and base.TZ0 <= z <= base.TZ1: continue        # trench floor/roof
            if r0 <= x <= r1 and zh <= z <= zl: continue                                 # hut ramp
            if base.R1[0] <= x <= base.R1[1] and base.R1[2] <= z <= base.R1[3]: continue  # tower R1
            if base.R2[0] <= x <= base.R2[1] and base.R2[3] <= z <= base.R2[2]: continue  # tower R2
            if base.XR[0] <= x <= base.XR[1] and base.XR[3] <= z <= base.XR[2]: continue  # exterior ramp
            if math.hypot(x-bx, z-bz) < base.BATTERY_R+12: continue                      # battery plinth + ramp
            if math.hypot(x-base.FLAG[0], z-base.FLAG[1]) < 2.1: continue                # flag plinth chamfer
            if any(math.hypot(x-e[0], z-e[2]) < 3.3 for e in ents): continue            # kit equipment
            if abs(x-base.MAST[0]) < 1 and abs(z-base.MAST[1]) < 1: continue             # sentry mast
            self.fail(('accidental slope', round(float(n[i, 1]), 4), cen[i]))

    def test_no_overlapping_coplanar_floor_plates(self):
        class Recording(kit.Mesh):
            def __init__(self):
                super().__init__(); self.plates = []
            def box(self, p, size, mat='concrete', solid=True):
                if solid and size[1] <= 1.25:
                    self.plates.append((p[1]+size[1]/2, p, size))
                super().box(p, size, mat, solid)
        mesh = Recording(); base.build(mesh, 0, 'one')
        for i, (ta, a, sa) in enumerate(mesh.plates):
            for tb, b, sb in mesh.plates[i+1:]:
                if abs(ta-tb) > 1e-6: continue
                overlap = all(min(a[k]+sa[k]/2, b[k]+sb[k]/2)-max(a[k]-sa[k]/2, b[k]-sb[k]/2) > 1e-6 for k in (0, 2))
                self.assertFalse(overlap, f'coplanar plate overlap at y={ta}: {a} {sa} / {b} {sb}')

    # --- Spawns, flag, equipment ------------------------------------------
    def test_spawn_points_stand_on_floor_with_room_and_face_open_space(self):
        points = self.anchors['spawn_points']
        self.assertEqual(len(points), 8)
        self.assertEqual(tuple(self.anchors['spawn']), tuple(points[0][:3]))
        for x, y, z, yaw in points:
            floor = y-LIFT
            t, ny = self.soup.hits((x, y-.37, z), (0, -1, 0))
            self.assertAlmostEqual(y-.37-t[0], floor, places=3); self.assertGreater(ny[0], .999)
            self.assertGreater(self.soup.first((x, floor+.2, z), (0, 1, 0)), 2.4, (x, z, 'headroom'))
            for ang in np.linspace(0, 2*np.pi, 8, endpoint=False):
                for h in (.3, 1., 1.8):
                    self.assertGreater(self.soup.first((x, floor+h, z), (np.cos(ang), 0, np.sin(ang))), .7, (x, z, h))
            facing = (-math.sin(yaw), 0, -math.cos(yaw))
            self.assertGreater(self.soup.first((x, floor+1.6, z), facing), 3., (x, z, 'faces a wall'))

    def test_flag_stands_on_its_plinth_open_to_the_sky(self):
        fx, fy, fz = self.anchors['flag']
        top = base.TOWER_TOP+base.PLINTH
        self.assertTrue(12 <= base.TOWER_TOP-base.HUT_RING <= 18, 'the flag platform stands 12-18 m over the knoll')
        t, ny = self.soup.hits((fx, fy+1, fz), (0, -1, 0))
        self.assertAlmostEqual(fy+1-t[0], top, places=4)
        self.assertAlmostEqual(fy, top+.05, places=6)
        self.assertEqual(self.soup.first((fx, fy+.1, fz), (0, 1, 0)), np.inf, 'flag must be open to the sky')

    def test_transform_invariance_and_budget(self):
        a, anchors = one_base(0, 'one')
        b, moved = one_base(1, 'two', (321, 75, 987), .73)
        self.assertEqual(anchors, moved)
        self.assertEqual(len(a.collision), len(b.collision))
        for i in range(0, len(a.collision), 3):
            expected = b.point(tuple(a.collision[i:i+3]))
            for x, y in zip(expected, b.collision[i:i+3]): self.assertAlmostEqual(x, y, places=3)
        self.assertTrue(all(e['team'] == 1 and e['circuit'] == 'two' for e in b.entities))
        kinds = sorted(e['kind'] for e in b.entities)
        self.assertEqual(kinds, ['generator', 'inventory', 'inventory', 'sensor', 'turret', 'turret', 'turret'])
        self.assertEqual(sorted(e['weapon'] for e in b.entities if e['kind'] == 'turret'), ['bullet', 'bullet', 'plasma'])
        self.assertLess(len(b.collision)//9, budgets.COLLISION_TRIS_PER_BASE)

    def test_two_instances_have_unique_ids_and_independent_circuits(self):
        mesh = kit.Mesh(); base.build(mesh, 0, 'red')
        mesh.origin = (2048, 0, 2048); mesh.yaw = math.pi; base.build(mesh, 1, 'blue')
        ids = [e['id'] for e in mesh.entities]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual({e['circuit'] for e in mesh.entities}, {'red', 'blue'})
        for team in (0, 1):
            self.assertEqual(sum(e['kind'] == 'generator' and e['team'] == team for e in mesh.entities), 1)

    def test_materials_are_deterministic_opaque_and_distinct(self):
        names = ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark']
        tex = {}
        for name in names:
            a = cairnhold_materials.texture(name, 7, kit.noise, kit.value_noise)
            self.assertEqual(a, cairnhold_materials.texture(name, 7, kit.noise, kit.value_noise), name)
            self.assertEqual(len(a), 256*256*4)
            self.assertTrue(all(a[i] == 255 for i in range(3, len(a), 4)), name)
            tex[name] = a
        for i, a in enumerate(names):
            for b in names[i+1:]: self.assertNotEqual(tex[a], tex[b], (a, b))
        from assets import tower_complex_materials
        for name in names:
            self.assertNotEqual(tex[name], tower_complex_materials.texture(name, 7, kit.noise, kit.value_noise), name)

    # --- Facade, flag tower, sunken trench ----------------------------------
    def test_facade_steps_up_and_recesses_the_portal(self):
        tops = [t for _, _, t in base.TIERS]+[base.ROOF]
        self.assertEqual(tops, sorted(tops, reverse=True))
        self.assertGreaterEqual(base.TIERS[0][2], 2*base.ROOF, 'gatehouse rises well above the old roof line')
        # The portal is open from the apron through the door to the baffle.
        for y in (.3, 1.2, 2.2, 3.8):
            for x in (-1.8, 0, 1.8):
                t = self.soup.first((x, y, -30), (0, 0, 1))
                self.assertAlmostEqual(t, base.BAFFLE_Z[0]+30, delta=.05, msg=(x, y))
        # Recess: the jambs stand 2 m proud of the front wall either side of the door.
        self.assertAlmostEqual(self.soup.first((4.8, 2, -30), (0, 0, 1)), base.GATE_Z+30, delta=.01)
        self.assertAlmostEqual(self.soup.first((3.2, 2, -30), (0, 0, 1)), base.BZ0+30, delta=.01)
        self.assertAlmostEqual(self.soup.first((0, 5.5, -30), (0, 0, 1)), base.BZ0+30, delta=.01)
        # The roof turret stands on the gatehouse.
        turret = [e for e in self.mesh.entities if e['kind'] == 'turret']
        self.assertTrue(any(abs(e['position'][1]-base.TIERS[0][2]-2.8) < 1e-6 for e in turret))

    def test_flag_tower_route_climbs_from_the_hut_to_the_platform(self):
        def climb(ramp, y_a, y_b, lo, hi):
            x = (ramp[0]+ramp[1])/2
            for z in np.arange(lo, hi, 1.0):
                y = base.ramp_y(ramp, z, y_a, y_b)
                t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
                self.assertLess(abs(1-t[0]), .02, (ramp, z, 'surface'))
                self.assertGreater(abs(ny[0]), .85, (ramp, z))    # the engine's sweep is double-sided
                self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (ramp, z, 'headroom'))
        climb(base.R1, base.HUT_ROOF, base.TOWER_MID, base.R1[2]+.3, base.R1[3])
        climb(base.R2, base.TOWER_MID, base.TOWER_TOP, base.R2[3]+.3, base.R2[2])
        # The top hatch is open above R2; the hut hatch is open above the hut ramp.
        g0, g1, ga, gb = base.TOP_HATCH
        for x in np.arange(g0+.3, g1, 1.0):
            for z in np.arange(ga+.3, gb, 1.0):
                y = base.ramp_y(base.R2, z, base.TOWER_MID, base.TOWER_TOP)
                self.assertEqual(self.soup.first((x, y+.2, z), (0, 1, 0)), np.inf, (x, z, 'open hatch'))
        # Nothing hides under R1: its open side is closed down to the floor.
        for z in np.arange(base.R1[2]+1, base.R1[3], 2.0):
            self.assertLess(self.soup.first((base.R1[1]+1, base.HUT_ROOF+.4, z), (-1, 0, 0)), 1.01, z)
        # The platform stands 12-18 m over the knoll, with open sky over the deck.
        self.assertTrue(12 <= base.TOWER_TOP-base.HUT_RING <= 18)
        self.assertEqual(self.soup.first((base.FLAG[0], base.TOWER_TOP+.5, base.FLAG[1]+3), (0, 1, 0)), np.inf)

    def test_exterior_ramp_and_bridge_reach_the_platform(self):
        xa, xb, zg, zt = base.XR
        slope = math.degrees(math.atan((base.TOWER_TOP-base.HUT_RING)/(zg-zt)))
        self.assertLess(slope, 27)
        x = (xa+xb)/2
        for z in np.arange(zt+.3, zg-.3, 1.0):
            y = base.exterior_ramp_y(z)
            t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
            self.assertLess(abs(1-t[0]), .02, (z, 'surface'))
            self.assertEqual(self.soup.first((x, y+.2, z), (0, 1, 0)), np.inf, (z, 'open sky'))
        # Its underside is closed on both sides along its whole height.
        for z in np.arange(zt+1, zg-2, 2.0):
            for sx in (-1, 1):
                edge = xa if sx < 0 else xb
                y = max(base.HUT_ROOF if z < base.HZ1 else base.HUT_RING, base.exterior_ramp_y(z)-2)+.3
                if y > base.exterior_ramp_y(z)-.7: continue
                self.assertLess(self.soup.first((edge+sx, y, z), (-sx, 0, 0)), 1.01, (z, sx))
        # From the bridge a body walks straight onto the platform.
        for h in (.3, 1.0, 1.8):
            self.assertGreater(self.soup.first((base.BRIDGE[0]+1, base.TOWER_TOP+h, 77.8), (1, 0, 0)), 9, h)

    def test_hole_cells_never_fall_through(self):
        """Every point over a dug-in footprint has collision under it at or
        above that structure's lowest floor, so there is no seam to drop
        through between the cut terrain and the structure."""
        floors = {'bunker': lambda z: -.05, 'trench': lambda z: base.trench_floor(z)-.05,
                  'hut': lambda z: base.HUT_FLOOR-.05}
        for name, (x0, x1, z0, z1) in base.HOLES.items():
            xs = np.concatenate([np.arange(x0+.1, x1, 1.0), [x0+.02, x1-.02]])
            zs = np.concatenate([np.arange(z0+.1, z1, 1.0), [z0+.02, z1-.02]])
            for x in xs:
                for z in zs:
                    t = self.soup.first((x, 90, z), (0, -1, 0))
                    self.assertLess(t, np.inf, (name, x, z))
                    self.assertGreaterEqual(90-t, floors[name](z), (name, x, z, 90-t))

    def test_trench_is_sunk_flush_with_the_hillside(self):
        spec, _, _, grid, _ = self.whole_map()
        for b in spec['bases']:
            oy = b['position'][1]
            for z in np.arange(base.TZ0+2, base.TZ1-2, 3.0):
                roof = base.trench_roof(z)
                for x in (base.TX0-7.5, base.TX0-.3, base.TX1+.3, base.TX1+7.5):   # between flush grid lines
                    ground = build.terrain_height(*build.to_world(b, x, z), grid)-oy
                    self.assertAlmostEqual(ground, roof, delta=.05, msg=(b['team'], x, z))
        # The hut terrace sits just above the knoll, the platform well above it.
        self.assertLess(base.HUT_ROOF-base.HUT_RING, 2)
        self.assertAlmostEqual(base.trench_roof(base.TZ1), base.HUT_RING)
        self.assertAlmostEqual(base.trench_roof(base.TZ0), base.ROOF)

    def test_wing_blocks_and_ramp_foot_meet_the_ground(self):
        spec, _, _, grid, _ = self.whole_map()
        for b in spec['bases']:
            oy = b['position'][1]
            h = lambda x, z: build.terrain_height(*build.to_world(b, x, z), grid)-oy
            for s in (-1, 1):
                for x in np.arange(base.BX+.5, base.WING_X, 1.5):
                    for z in np.arange(base.BZ0+.5, base.WING_Z1, 1.5):
                        self.assertTrue(-.5 < h(s*x, z) <= base.ROOF+1e-6, (s*x, z))        # buried in the block
                    self.assertAlmostEqual(h(s*x, base.WING_Z1+.3), base.ROOF, delta=.06)  # flush behind it
                    self.assertAlmostEqual(h(s*x, base.BZ0-.5), -.1, delta=.25)          # apron in front
            xa, xb, zg, _ = base.XR
            for x in np.arange(xa, xb+.1, 1.0):
                for z in np.arange(base.HZ1+.5, zg+2, 1.0):
                    self.assertAlmostEqual(h(x, z), base.HUT_RING, delta=.05, msg=(x, z))

    # --- Whole map: sightlines, terrain, holes -----------------------------
    @classmethod
    def whole_map(cls):
        if not hasattr(cls, '_map'):
            spec = build.spec(); mesh = kit.Mesh()
            for b in spec['bases']:
                mesh.origin = tuple(b['position']); mesh.yaw = math.radians(b['yaw'])
                base.build(mesh, b['team'], f"base-{b['team']}")
            mesh.origin = tuple(spec['ring']['position']); mesh.yaw = 0
            cairnhold_ring.build(mesh)
            grid, holes = build.terrain_grid(spec)
            cls._map = spec, mesh, Soup(mesh.collision), grid, holes
        return cls._map

    def test_turrets_cannot_see_into_rooms(self):
        """No turret or the plasma battery has a clear line from its barrel to
        a player's chest anywhere inside a bunker room, the trench or the hut.
        Allowed: the 3.6 m vestibule behind each front door (outside the
        baffle) and the ramp directly under the stand's open roof hatch."""
        spec, mesh, soup, _, _ = self.whole_map()
        tx0, tx1, tz0, tz1 = base.TW
        rooms = [(-15, 15, base.BAFFLE_Z[1]+.6, 19, lambda z: 0.0),
                 (base.TX0+1.6, base.TX1-1.6, base.TZ0+.6, base.TZ1-.6, base.trench_floor),
                 (base.HX0+1.4, base.HX1-1.4, base.HZ0+1.4, base.HZ1-1.4, lambda z: base.HUT_FLOOR),
                 (tx0+1.4, tx1-1.4, tz0+1.4, tz1-1.4, lambda z: base.HUT_ROOF),
                 (tx0+1.4, tx1-1.4, base.LANDING_Z[0]+.4, base.LANDING_Z[1]-.3, lambda z: base.TOWER_MID)]
        r0, r1, zl, zh = base.RAMP; h0, h1, hz0, hz1 = base.HATCH
        targets = []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for x0, x1, z0, z1, floor in rooms:
                for x in np.arange(x0, x1+.01, 1.0):
                    for z in np.arange(z0, z1+.01, 1.0):
                        y = floor(z)
                        if y != base.TOWER_MID and h0-.5 <= x <= h1 and hz0 <= z <= hz1+2: continue   # hatch columns
                        if soup.first(m.point((x, y+.1, z)), (0, 1, 0), 2.3) < 2.3: continue   # inside a prop
                        targets.append(m.point((x, y+LIFT+.8, z)))
        targets = np.array(targets)
        turrets = [e for e in mesh.entities if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        seen = 0
        for e in turrets:
            p = np.array(e['position']); d = targets-p; dist = np.linalg.norm(d, axis=1)
            near = dist < 150
            starts = p+d[near]/dist[near, None]*(e['radius']+.6)
            clear = ~soup.blocked_many(starts, targets[near])
            seen += int(clear.sum())
            self.assertEqual(int(clear.sum()), 0, (e['id'], targets[near][clear][:5]))
        self.assertGreater(len(targets), 900)

    def test_terrain_is_symmetric_rugged_and_matches_targets(self):
        spec, _, _, grid, holes = self.whole_map()
        again, _ = build.terrain_grid(spec)
        self.assertTrue(np.array_equal(grid, again), 'terrain must be deterministic')
        self.assertLess(np.abs(grid[1:, 1:]-grid[1:, 1:][::-1, ::-1]).max(), 1e-6, 'exact 180 degree symmetry')
        # Slope over the play area's terrain cells (hole cells render no ground).
        slope = cairnhold_terrain.slope_degrees(grid)
        cut = np.zeros(65536, bool); cut[holes] = True; cut = cut.reshape(256, 256)
        rows, cols = slice(int((1024-330)/8), int((1024+330)/8)+1), slice(int((1024-280)/8), int((1024+280)/8)+1)
        play = slope[rows, cols][~cut[rows, cols]]
        self.assertTrue(26 <= np.median(play) <= 30, np.median(play))
        self.assertLess((play < 3).mean(), .03)
        # Two valleys and a raised middle along the flag axis.
        (_, _, rz), (_, _, bz) = [b['position'] for b in spec['bases']]
        prof = lambda z: build.terrain_height(1024, z, grid)
        self.assertGreater(prof(1024)-prof(1024-130), 20); self.assertGreater(prof(1024)-prof(1024+130), 20)
        # Each bunker sits on its knoll's forward slope: the ground falls away
        # in front of the apron into the valley, and rises behind to the stand.
        for b in spec['bases']:
            front = build.terrain_height(*build.to_world(b, 0, base.BZ0-3), grid)
            ahead = min(build.terrain_height(*build.to_world(b, 0, base.BZ0-d), grid) for d in (40, 50, 60))
            behind = build.terrain_height(*build.to_world(b, -6, base.HZ0+4), grid)
            self.assertGreater(front-ahead, 3, (b['team'], front, ahead))
            self.assertGreater(behind-front, 15, (b['team'], front, behind))
        flags = []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            flags.append(m.point((base.FLAG[0], 0, base.FLAG[1])))
        self.assertTrue(500 <= math.dist(flags[0][::2], flags[1][::2]) <= 540)

    def test_holes_are_exactly_the_dug_in_footprints_and_walls_cover_the_pins(self):
        spec, _, _, grid, holes = self.whole_map()
        hole_set = set(holes)
        expected = set()
        for b in spec['bases']:
            for x0, x1, z0, z1 in base.HOLES.values():
                for x in np.arange(x0+4, x1, 8):
                    for z in np.arange(z0+4, z1, 8):
                        wx, wz = build.to_world(b, x, z)
                        expected.add(int(wz//8)*256+int(wx//8))
        self.assertEqual(hole_set, expected)
        _, rings = build.holes_and_rings(spec)
        spans = {'bunker': lambda z: (-4, base.ROOF),
                 'trench': lambda z: (base.trench_wall_bottom(z), base.trench_roof(z)),
                 'hut': lambda z: (base.HUT_FLOOR-5, base.HUT_ROOF)}
        for (vx, vz), y in rings.items():
            self.assertEqual(grid[vz, vx], y)
            for b in spec['bases']:
                lx, lz = build.to_local(b, vx*8, vz*8)
                for name, (x0, x1, z0, z1) in base.HOLES.items():
                    if x0-1e-6 <= lx <= x1+1e-6 and z0-1e-6 <= lz <= z1+1e-6:
                        lo, hi = spans[name](lz)
                        self.assertTrue(lo < y-b['position'][1] <= hi+1e-6, (name, lx, lz, y))

    def test_ground_stays_under_decks_ramps_and_the_ring(self):
        spec, _, _, grid, _ = self.whole_map()
        h = lambda x, z: build.terrain_height(x, z, grid)
        for b in spec['bases']:
            oy = b['position'][1]
            px, pz = base.PAD; H = base.PAD_HALF
            for x in np.arange(px-H, px+H+.1, 2):
                for z in np.arange(pz-H, pz+H+.1, 2):
                    wx, wz = build.to_world(b, x, z)
                    self.assertTrue(base.PAD_TOP-1.2 < h(wx, wz)-oy < base.PAD_TOP-.03, ('pad', x, z))
            bx, bz = base.BATTERY
            for r in np.arange(0, base.BATTERY_R*math.cos(math.pi/8)+10, 1.5):
                for dx in (-1.9, 0, 1.9):
                    wx, wz = build.to_world(b, bx+dx, bz+r)
                    self.assertLessEqual(h(wx, wz)-oy, base.BATTERY_GROUND+1e-6, ('battery', r))
            # The front apron is level with the bunker floor.
            for x in (-4, 0, 4):
                wx, wz = build.to_world(b, x, base.BZ0-3)
                self.assertAlmostEqual(h(wx, wz)-oy, -.1, delta=1/32)   # height.bin quantisation
        rx, ry, rz = spec['ring']['position']
        for r in np.arange(0, cairnhold_ring.PYLON_R+2.5, 1.0):
            for a in np.linspace(0, math.tau, 24, endpoint=False):
                self.assertAlmostEqual(h(rx+r*math.cos(a), rz+r*math.sin(a)), ry-.06, delta=1/32)

    def test_ring_is_symmetric_and_open_on_the_axes(self):
        mesh = kit.Mesh(); anchors = cairnhold_ring.build(mesh)
        soup = Soup(mesh.collision)
        pts = np.array(anchors['pylons'])
        self.assertTrue(np.allclose(np.sort(pts[:, [0, 2]], 0), np.sort(-pts[:, [0, 2]], 0)))
        for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            # Each ramp lane leads from outside the pylons up onto the dais.
            ts, _ = soup.hits((dx*30, cairnhold_ring.DAIS_H+1.2, dz*30), (-dx, 0, -dz), 60)
            self.assertTrue(len(ts) == 0 or ts[0] > 28, (dx, dz, ts[:2]))
        self.assertLess(len(mesh.collision)//9, 400)

    # --- Pack --------------------------------------------------------------
    def test_lightmap_bake_is_deterministic(self):
        from assets import lightmap_bake
        m = kit.Mesh(); m.lamps = []; cairnhold_ring.build(m)
        verts = np.array(m.vertices, np.float32).reshape(-1, 12)
        sun = (-.57735, .57735, -.57735); light = kit.MATERIALS.index('light')
        a, pa = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=1)
        b, pb = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=3)
        self.assertTrue(np.array_equal(a, b)); self.assertEqual(pa, pb)
        self.assertTrue(np.all((a[:, 8] >= 0) & (a[:, 8] <= 1) & (a[:, 9] >= 0) & (a[:, 9] <= 1)))

    def test_unbaked_rebuilds_are_byte_identical(self):
        with tempfile.TemporaryDirectory() as tmp:
            digests = []
            for name in ('a', 'b'):
                out = Path(tmp)/name
                build.build(out, bake=False)
                digests.append({f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in out.iterdir()})
            self.assertEqual(digests[0], digests[1])
            self.assertEqual(len(digests[0]), 7)

class SpawnForwardClearance(unittest.TestCase):
    """Every committed spawn faces open floor: a clear body-width view for
    6 m and the same floor for a half-second walk (docs/map-pipeline.md)."""
    def test_committed_spawns_face_open_floor(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/cairnhold'
        self.assertEqual(spawn_checks.problems(pack), [])


if __name__ == '__main__':
    unittest.main()
