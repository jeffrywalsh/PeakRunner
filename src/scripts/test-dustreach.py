#!/usr/bin/env python3
"""Test the Dustreach builders without reading source-game assets.
Run from src/ with the numpy venv: scripts/test-dustreach.py"""
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
from assets import dustreach_citadel as base
from assets import dustreach_gate as gate
from assets import dustreach_materials
from assets import dustreach_terrain

HERE = Path(__file__).parent
def _load(name, file):
    loader = importlib.util.spec_from_file_location(name, HERE/file)
    module = importlib.util.module_from_spec(loader); loader.loader.exec_module(module)
    return module
kit = _load('kit', 'build-original-map.py')
build = _load('dustreach_build', 'build-dustreach.py')

LIFT = base.SPAWN_LIFT
TER = base.TER


class Soup:
    """Vectorised ray queries against a collision triangle soup."""
    def __init__(self, collision):
        t = np.array(collision, np.float64).reshape(-1, 3, 3)
        self.a = t[:, 0]; self.e1 = t[:, 1]-t[:, 0]; self.e2 = t[:, 2]-t[:, 0]
        n = np.cross(self.e1, self.e2); self.n = n/(np.linalg.norm(n, axis=1, keepdims=True)+1e-12)
        self.lo = t.min(1); self.hi = t.max(1); self.tris = t

    def hits(self, o, d, tmax=np.inf):
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


class DustreachTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mesh, cls.anchors = one_base()
        cls.soup = Soup(cls.mesh.collision)

    # --- Floors, headroom and standing support ----------------------------
    def test_floors_have_floor_and_headroom(self):
        samples = [(x, z, TER) for x, z in ((-6, -11), (5, -11), (-5, -2), (5, -2), (12, -11), (12, -1.5))]   # hall
        samples += [(x, z, TER) for x in (-11, 11) for z in (6, 12, 24)]+[(x, z, TER) for x in (-3, 3) for z in (6, 27.5)]  # courtyard
        samples += [(s*18.2, z, TER) for s in (-1, 1) for z in (6, 15.5, 25)]           # arcades
        samples += [(x, z, base.PIT_FLOOR) for x in (-5, 0, 5) for z in (14.8, 19.2)]   # flag court
        samples += [(x, -17.4, TER) for x in (-10, 0, 10)]                              # front ledge
        for x, z, y in samples:
            t, ny = self.soup.hits((x, y+3, z), (0, -1, 0))
            self.assertTrue(len(t) and abs(3-t[0]) < 1e-6 and ny[0] > .999, (x, z, 'floor'))
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'headroom'))

    def test_ramps_climb_with_open_sky_and_closed_undersides(self):
        for ramp, fn in [(base.FRONT_RAMP, base.front_ramp_y), (base.BACK_RAMP, base.back_ramp_y)]:
            x0, x1, zg, zt = ramp
            lo, hi = sorted((zg, zt))
            for z in np.arange(lo+.4, hi, 1.0):
                for x in (x0+.8, (x0+x1)/2, x1-.8):
                    y = fn(z)
                    t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
                    self.assertLess(abs(1-t[0]), .02, (ramp, x, z))
                    self.assertGreater(abs(ny[0]), .85, (ramp, z))
                    self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 3.0, (ramp, z, 'headroom'))
            self.assertLess(math.degrees(math.atan(TER/(hi-lo))), 20)
            # Nothing can be walked into underneath either edge.
            for z in np.arange(lo+1, hi-1, 1.5):
                y = fn(z)
                if y-.9 < .3: continue
                for edge, sx in ((x0, -1), (x1, 1)):
                    self.assertLess(self.soup.first((edge+sx, .3, z), (-sx, 0, 0)), 1.01, (ramp, z, sx))
        # The flag court ramps.
        for z in np.arange(base.PIT_Z[0]+.3, base.PIT_Z[1], 1.0):
            for x in (-5, 0, 5):
                if abs(x-base.FLAG[0]) < 2.2 and abs(z-base.FLAG[1]) < 2.2: continue   # flag plinth
                y = base.pit_y(z)
                t, _ = self.soup.hits((x, y+1, z), (0, -1, 0))
                self.assertLess(abs(1-t[0]), .02, (x, z, 'pit'))
                self.assertEqual(self.soup.first((x, y+.2, z), (0, 1, 0)), np.inf, (x, z, 'open sky'))

    def test_watch_ruin_ramp_reaches_the_deck(self):
        r0, r1, zg, zt = base.ruin_ramp()
        slope = math.degrees(math.atan(base.RUIN_TOP/(zg-zt)))
        self.assertLess(slope, 30)
        for z in np.arange(zt+.3, zg-.3, 1.0):
            y = base.ramp_y((r0, r1, zg, zt), z, 0, base.RUIN_TOP)
            t, _ = self.soup.hits(((r0+r1)/2, y+1, z), (0, -1, 0))
            self.assertLess(abs(1-t[0]), .02, z)
        cx, cz = base.RUIN
        t, ny = self.soup.hits((cx, base.RUIN_TOP+5.5, cz+3.5), (0, -1, 0))
        self.assertAlmostEqual(base.RUIN_TOP+5.5-t[0], base.RUIN_TOP, places=4)

    def test_standing_support_is_the_floor_top_everywhere(self):
        """The engine's support ray starts 0.15 m above the feet. Wherever a
        player can stand, its first hit must be the up-facing floor top."""
        regions = [(-13, 13, -15, 3, TER), (-20.5, 20.5, 4.5, 28.4, TER), (-20.5, 20.5, -17.8, -16.2, TER),
                   (-6.8, 6.8, 14.2, 19.8, base.PIT_FLOOR), (-13.8, 13.8, -15.8, 3.8, base.ROOF)]
        for s in (-1, 1):
            a, c = sorted((s*15.9, s*19.9)); regions.append((a, c, 4.1, 28.7, base.ARCADE_ROOF))
            a, c = sorted((s*14.9, s*19.1)); regions.append((a, c, -18.3, -12.7, base.PYLON_TOP))
        wx0, wx1, wz0, wz1 = base.TOWER
        regions.append((wx0+.6, wx1-.6, wz0+.6, wz1-.6, base.TOWER_TOP))
        px, pz = base.PAD; H = base.PAD_HALF-.6
        regions.append((px-H, px+H, pz-H, pz+H, base.PAD_TOP))
        cx, cz = base.RUIN; r = base.RUIN_R*.8
        regions.append((cx-r, cx+r, cz-r, cz+r, base.RUIN_TOP))
        checked = 0
        for x0, x1, z0, z1, level in regions:
            for x in np.arange(x0, x1+.01, .8):
                for z in np.arange(z0, z1+.01, .8):
                    t, ny = self.soup.hits((x, level+2.6, z), (0, -1, 0))
                    if not len(t) or abs(2.6-t[0]) > .02 or np.any((np.abs(t-t[0]) < .01) & (ny < .99)): continue
                    first, fy = self.soup.hits((x, level+.15, z), (0, -1, 0))
                    checked += 1
                    self.assertTrue(len(first) and first[0] < .17 and fy[0] > .99, (x, z, level, first[:2]))
        self.assertGreater(checked, 3000)

    def test_no_accidental_slopes_on_walkable_surfaces(self):
        n, cen = self.soup.n, self.soup.tris.mean(1)
        ents = [e['position'] for e in self.mesh.entities]
        cx, cz = base.RUIN
        ramps = [base.FRONT_RAMP, base.BACK_RAMP, base.ruin_ramp(), (-base.PIT_X, base.PIT_X, *base.PIT_Z),
                 base.STAIR, base.TOWER_RAMP, base.STORE_RAMP]
        for i in np.where((n[:, 1] > .3) & (n[:, 1] < .9999))[0]:
            x, y, z = cen[i]
            if any(r[0]-.01 <= x <= r[1]+.01 and min(r[2:])-.01 <= z <= max(r[2:])+.01 for r in ramps): continue
            if math.hypot(x-cx, z-cz) < base.RUIN_R+1: continue                        # ruin chamfer
            if math.hypot(x-base.FLAG[0], z-base.FLAG[1]) < 2.1: continue              # flag plinth
            if any(math.hypot(x-e[0], z-e[2]) < 3.3 for e in ents): continue          # kit equipment
            if any(math.hypot(x-dx, z-dz) < r+.5 for dx, dz, r in base.DRUMS): continue
            if abs(x) > base.PIT_X+.8 and abs(x) < base.PIT_X+2.2 and y > TER: continue   # braziers
            if any(math.hypot(x-s*base.ARCADE_X, z-az) < 1 for s in (-1, 1) for az in base.ARCADE_Z): continue
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

    def test_flag_sits_in_the_sunken_court_open_to_the_sky(self):
        fx, fy, fz = self.anchors['flag']
        top = base.PIT_FLOOR+base.PLINTH
        self.assertEqual(TER-base.PIT_FLOOR, 3.0, 'the court is sunk 3 m below the terrace')
        t, _ = self.soup.hits((fx, fy+1, fz), (0, -1, 0))
        self.assertAlmostEqual(fy+1-t[0], top, places=4)
        self.assertAlmostEqual(fy, top+.05, places=6)
        self.assertEqual(self.soup.first((fx, fy+.1, fz), (0, 1, 0)), np.inf, 'flag must be open to the sky')
        # The court's side walls stand between the flag and the arcades.
        for s in (-1, 1):
            self.assertLess(self.soup.first((0, base.PIT_FLOOR+1, fz), (s, 0, 0)), base.PIT_X+.01)

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
        self.assertEqual(kinds, ['generator', 'inventory', 'inventory', 'inventory', 'sensor', 'turret', 'turret', 'turret'])
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

    def test_materials_and_sky_are_deterministic_opaque_and_distinct(self):
        tex = {}
        for name in dustreach_materials.MATERIALS:
            a = dustreach_materials.texture(name, 7, kit.noise, kit.value_noise)
            self.assertEqual(a, dustreach_materials.texture(name, 7, kit.noise, kit.value_noise), name)
            self.assertEqual(len(a), 256*256*4)
            self.assertTrue(all(a[i] == 255 for i in range(3, len(a), 4)), name)
            tex[name] = a
        names = list(tex)
        for i, a in enumerate(names):
            for b in names[i+1:]: self.assertNotEqual(tex[a], tex[b], (a, b))
        from assets import tower_complex_materials, cairnhold_materials
        for name in ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark']:
            self.assertNotEqual(tex[name], tower_complex_materials.texture(name, 7, kit.noise, kit.value_noise), name)
            self.assertNotEqual(tex[name], cairnhold_materials.texture(name, 7, kit.noise, kit.value_noise), name)
        face = dustreach_materials.sky(4, kit.unit, kit.smooth, kit.cloud_noise)
        self.assertEqual(face, dustreach_materials.sky(4, kit.unit, kit.smooth, kit.cloud_noise))
        self.assertEqual(len(face), 256*256*4)
        self.assertNotEqual(face, kit.sky(4))

    # --- Underground level and storehouse (v2) -------------------------------
    def test_underground_and_storehouse_floors_have_floor_and_headroom(self):
        C, S = base.CIS_FLOOR, base.STORE_UP
        tw = (base.TUNNEL_W[2]+base.TUNNEL_W[3])/2
        # (x, z, floor, minimum headroom)
        samples = [(x, z, C, 5.5) for x, z in ((-10, -14), (-10, 4), (4, -14), (-4, 3), (3, 4), (-11, -3))]
        samples += [(-12.0, z, C, 4.2) for z in np.arange(7.5, 21.6, 2.0)]                 # tunnel north
        samples += [(x, tw, C, 4.2) for x in np.arange(-23.5, -15.9, 1.5)]                # tunnel west
        samples += [(-29.0, 18.0, C, 6.0), (-25.5, 20.0, C, 6.0), (-29.5, 28.5, base.LANDING, 4.2)]   # tower room
        samples += [(x, z, 0.0, 3.5) for x, z in ((33, 4), (45, 4), (38, 20), (33, 20), (40, 8))]    # store, ground
        samples += [(x, z, S, 3.5) for x, z in ((33, 4), (40, 20), (46, 5), (35, 12))]              # store, upper
        for x, z, y, room in samples:
            t, ny = self.soup.hits((x, y+3, z), (0, -1, 0))
            self.assertTrue(len(t) and abs(3-t[0]) < 1e-6 and ny[0] > .999, (x, z, y, 'floor', t[:1]))
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), room, (x, z, y, 'headroom'))
        # The tunnel keeps its 4.5 m headroom under a lid, not open to the terrace.
        self.assertAlmostEqual(self.soup.first((-12.0, C+.2, 12.0), (0, 1, 0)), base.TUN_CEIL-C-.2, places=4)

    def test_no_spawn_camps_the_generator_stair(self):
        """No spawn in the keep hall or cistern, and every spawn at least 15 m
        (straight line, so walking is further) from the stair head."""
        hx0, hx1, _, hz1 = base.HALL_HOLE
        head = ((hx0+hx1)/2, hz1)
        ix, iz0, iz1 = base.KX, base.KZ0, base.KZ1
        for x, y, z, _ in self.anchors['spawn_points']:
            self.assertFalse(-ix < x < ix and iz0 < z < iz1 and y > base.TER, (x, z, 'in the keep hall'))
            self.assertGreaterEqual(math.hypot(x-head[0], z-head[1]), 15.0, (x, z))

    def test_tower_back_door_porch_hides_the_exit_ramp(self):
        """Nobody outside the porch sees into the tower room or up its ramp:
        rays from a ring of outside viewpoints behind and beside the tower to
        points in the room, on the ramp and on the landing are all blocked.
        The porch is still open to the east for the route out."""
        wx0, wx1, wz0, wz1 = base.TOWER
        qx0, qx1, qz0, qz1 = base.PORCH
        targets = []
        r0, r1, rz0, rz1 = base.TOWER_RAMP
        for z in np.arange(rz0+.4, rz1, 1.0):
            for x in (r0+.6, (r0+r1)/2, r1-.6):
                targets.append((x, base.tower_ramp_y(z)+1.0, z))
        for x in np.arange(wx0+1.5, base.TOWER_HOLE[1]-1.0, 1.5):
            for z in np.arange(wz0+1.5, rz0, 1.5):
                targets.append((x, base.CIS_FLOOR+1.0, z))
            targets.append((x, base.LANDING+1.0, (rz1+wz1)/2))
        viewers = []
        for ang in np.linspace(-.25*np.pi, 1.25*np.pi, 25):   # behind (+Z) and to both sides
            for dist in (6.0, 15.0, 40.0):
                for h in (1.7, 6.0, 14.0):
                    cx, cz = (qx0+qx1)/2, qz1
                    x, z = cx+dist*math.cos(ang), cz+dist*math.sin(ang)
                    if z < wz1+.5 and wx0-1 < x < wx1+1: continue
                    if qx0 <= x <= qx1+2 and wz1 <= z <= qz1: continue   # inside the porch itself
                    viewers.append((x, h, z))
        starts = np.array([v for v in viewers for _ in targets], float)
        ends = np.array([t for _ in viewers for t in targets], float)
        blocked = self.soup.blocked_many(starts, ends)
        self.assertTrue(blocked.all(), [tuple(starts[i]) + tuple(ends[i]) for i in np.flatnonzero(~blocked)[:3]])
        # East opening: open from the ground outside, through the porch, to the door.
        y = base.LANDING+1.0
        for a, c in [((qx1+4, y, (wz1+qz0)/2), ((base.TOWER_DOOR[0]+base.TOWER_DOOR[1])/2, y, (wz1+qz0)/2)),
                     (((base.TOWER_DOOR[0]+base.TOWER_DOOR[1])/2, y, (wz1+qz0)/2),
                      ((base.TOWER_DOOR[0]+base.TOWER_DOOR[1])/2, y, wz1-1.0))]:
            d = np.subtract(c, a); n = np.linalg.norm(d)
            self.assertEqual(self.soup.first(a, d/n, n), np.inf, (a, c))

    def test_new_ramps_climb_with_headroom_and_closed_undersides(self):
        cases = [(base.STAIR, base.stair_y, base.CIS_FLOOR, 1),        # open (railed) side at x0, probe from -x
                 (base.TOWER_RAMP, base.tower_ramp_y, base.CIS_FLOOR, -1),
                 (base.STORE_RAMP, base.store_ramp_y, 0.0, 1)]
        for ramp, fn, bottom, side in cases:
            x0, x1, za, zb = ramp
            lo, hi = sorted((za, zb))
            rise = abs(fn(zb)-fn(za))
            self.assertLess(math.degrees(math.atan(rise/(hi-lo))), 30.0, ramp)
            for z in np.arange(lo+.3, hi-.2, .8):
                for x in (x0+.6, (x0+x1)/2, x1-.6):
                    y = fn(z)
                    t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
                    self.assertLess(abs(1-t[0]), .02, (ramp, x, z))
                    self.assertGreater(abs(ny[0]), .85, (ramp, z))
                    self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (ramp, x, z, 'headroom'))
            # Nothing can be walked into underneath the open edge.
            edge = x0 if side > 0 else x1
            for z in np.arange(lo+.5, hi-.5, .8):
                if fn(z)-.9 < bottom+.3: continue
                self.assertLess(self.soup.first((edge-side, bottom+.3, z), (side, 0, 0)), 1.01, (ramp, z))
        # The stair's head meets the hall floor flush, and the hall floor
        # resumes over the stair's low end (the opening is railed round).
        hx0, hx1, hz0, hz1 = base.HALL_HOLE
        t, ny = self.soup.hits(((hx0+hx1)/2, TER+1, hz1+.3), (0, -1, 0))
        self.assertAlmostEqual(t[0], 1.0, places=4); self.assertGreater(ny[0], .999)
        t, _ = self.soup.hits(((hx0+hx1)/2, TER+1, hz1-.3), (0, -1, 0))
        self.assertAlmostEqual(TER+1-t[0], base.stair_y(hz1-.3), delta=.02)
        for x in (hx0-.2, (hx0+hx1)/2):
            self.assertLess(self.soup.first((x, TER+.5, hz0-1.0), (0, 0, 1)), 1.01, 'south rail')

    def test_generator_room_has_exactly_two_ways_in(self):
        """The cistern's walls have one opening (the tunnel door); the only
        other way in is the stair from the hall. No view out through its
        ceiling, and the generator sits on its floor."""
        C = base.CIS_FLOOR
        cx0, cx1, cz0, cz1 = base.CIS
        gx, gy, gz = self.anchors['generator']
        self.assertTrue(cx0 < gx < cx1 and cz0 < gz < cz1 and gy == C)
        runs = {}
        step = .25
        sides = {'south': [((x, cz0+.3), (0, 0, -1)) for x in np.arange(cx0+.3, cx1-.29, step)],
                 'north': [((x, cz1-.3), (0, 0, 1)) for x in np.arange(cx0+.3, cx1-.29, step)],
                 'west': [((cx0+.3, z), (-1, 0, 0)) for z in np.arange(cz0+.3, cz1-.29, step)],
                 'east': [((cx1-.3, z), (1, 0, 0)) for z in np.arange(cz0+.3, cz1-.29, step)
                          if not base.STAIR[2]-.4 <= z <= base.STAIR[3]+.4]}
        for name, probes in sides.items():
            open_ = [all(self.soup.first((x, C+h, z), d, 2.0) == np.inf for h in (.6, 1.6)) for (x, z), d in probes]
            n, width, widths = 0, 0, []
            for o in open_ + [False]:
                if o: width += step
                elif width: widths.append(width); width = 0
            runs[name] = widths
        self.assertEqual(runs['south'], []); self.assertEqual(runs['west'], []); self.assertEqual(runs['east'], [])
        self.assertEqual(len(runs['north']), 1, runs['north'])
        self.assertAlmostEqual(runs['north'][0], base.TUN_DOOR[1]-base.TUN_DOOR[0]-.3, delta=.6)
        for x in np.arange(cx0+.5, cx1, 1.0):
            for z in np.arange(cz0+.5, cz1, 1.0):
                self.assertLess(self.soup.first((x, C+.3, z), (0, 1, 0)), 7.0, (x, z, 'open ceiling'))

    def test_holes_are_covered_on_the_grid_and_cut_edges_stay_flat(self):
        spec, _, soup, grid = self.whole_map()
        cells = build.holes(spec)
        per_base = sum((x1-x0)*(z1-z0)/64 for x0, x1, z0, z1 in base.HOLES.values())
        self.assertEqual(len(cells), 2*per_base, 'hole rects overlap or leave the grid')
        for b in spec['bases']:
            oy = b['position'][1]
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for name, (x0, x1, z0, z1) in base.HOLES.items():
                for x in np.arange(x0+1.5, x1, 3.0):
                    for z in np.arange(z0+1.5, z1, 3.0):
                        p = m.point((x, 2.3, z))
                        t, _ = soup.hits(p, (0, -1, 0))
                        floor = 2.3-t[0] if len(t) else -1e9
                        self.assertTrue(base.CIS_FLOOR-.01 <= floor < 2.3, (name, x, z, floor))   # a floor, stair or landing
                        self.assertLess(soup.first(p, (0, 1, 0)), 30, (name, x, z, 'hole open to the sky'))
                # Grid vertices on the cut's edge keep the flat site height: no seam.
                for x in np.arange(x0, x1+.1, 8.0):
                    for z in (z0, z1):
                        wx, wz = build.to_world(b, x, z)
                        self.assertAlmostEqual(build.terrain_height(wx, wz, grid)-oy, -.1, delta=1/32, msg=(name, x, z))
                for z in np.arange(z0, z1+.1, 8.0):
                    for x in (x0, x1):
                        wx, wz = build.to_world(b, x, z)
                        self.assertAlmostEqual(build.terrain_height(wx, wz, grid)-oy, -.1, delta=1/32, msg=(name, x, z))

    def test_storehouse_bridge_route_is_open_through_the_gate(self):
        z = (base.GATE_Z[0]+base.GATE_Z[1])/2
        for x in (17.5, 20.6, base.TX+1, (base.TX+base.STORE[0])/2, base.STORE[0]+.4, base.STORE[0]+1.6, 33.0):
            t, ny = self.soup.hits((x, TER+1, z), (0, -1, 0))
            self.assertAlmostEqual(t[0], 1.0, places=4, msg=x); self.assertGreater(ny[0], .999, x)
            self.assertGreater(self.soup.first((x, TER+.2, z), (0, 1, 0)), 3.5, (x, 'headroom'))
        # Straight through the gate and the door: the first thing in the way
        # is the storehouse baffle, not the curtain or the wall.
        baffle = base.STORE[0]+base.WALL+2.4
        self.assertAlmostEqual(self.soup.first((17.0, TER+1.2, z), (1, 0, 0)), baffle-17.0, places=3)

    # --- Whole map ----------------------------------------------------------
    @classmethod
    def whole_map(cls):
        if not hasattr(cls, '_map'):
            spec = build.spec(); mesh = kit.Mesh()
            build.build_structures(spec, mesh)
            grid = build.terrain_grid(spec)
            cls._map = spec, mesh, Soup(mesh.collision), grid
        return cls._map

    # Rooms no turret may see into: (name, x0, x1, z0, z1, floor), local. The
    # vestibules between each door and its baffle lie outside these boxes.
    ROOMS = [('keep hall', -(base.KX-base.WALL)+.6, base.KX-base.WALL-.6,
              base.FRONT_BAFFLE_Z[1]+.6, base.BACK_BAFFLE_Z[0]-.6, TER),
             ('cistern', base.CIS[0]+.6, base.CIS[1]-.6, base.CIS[2]+.6, base.CIS[3]-.6, base.CIS_FLOOR),
             ('tunnel north', base.TUNNEL_N[0]+1.4, base.TUNNEL_N[1]-1.4, base.TUNNEL_N[2]+.6, base.TUNNEL_W[3]-1.4, base.CIS_FLOOR),
             ('tunnel west', base.TUNNEL_W[0]+.6, base.TUNNEL_W[1]+.6, base.TUNNEL_W[2]+1.4, base.TUNNEL_W[3]-1.4, base.CIS_FLOOR),
             ('tower room', base.TOWER[0]+1.4, base.TOWER_HOLE[1]-1.4, base.TOWER[2]+1.4, base.TOWER_RAMP[2], base.CIS_FLOOR),
             ('store ground', base.STORE[0]+base.WALL+3.4, base.STORE[1]-base.WALL-.6,
              base.STORE[2]+base.WALL+.6, base.STORE[3]-base.WALL-3.4, 0.0),
             ('store upper', base.STORE[0]+base.WALL+3.4, base.STORE_HOLE[0]-1.0,
              base.STORE[2]+base.WALL+.6, base.STORE[3]-base.WALL-.6, base.STORE_UP)]

    def test_turrets_cannot_see_into_rooms(self):
        """No turret has a clear line from its barrel to a player's chest
        anywhere in the spawn hall, the cistern, the tunnel, the tower room or
        the storehouse, at sensor-extended range."""
        spec, mesh, soup, _ = self.whole_map()
        targets, counts = [], {}
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for name, x0, x1, z0, z1, floor in self.ROOMS:
                for x in np.arange(x0, x1+.01, 1.0):
                    for z in np.arange(z0, z1+.01, 1.0):
                        if soup.first(m.point((x, floor+.1, z)), (0, 1, 0), 2.3) < 2.3: continue   # inside a prop
                        t, _ = soup.hits(m.point((x, floor+1, z)), (0, -1, 0))
                        if not len(t) or abs(t[0]-1) > .02: continue                              # not this floor
                        targets.append(m.point((x, floor+LIFT+.8, z))); counts[name] = counts.get(name, 0)+1
        targets = np.array(targets)
        turrets = [e for e in mesh.entities if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        for e in turrets:
            p = np.array(e['position']); d = targets-p; dist = np.linalg.norm(d, axis=1)
            near = dist < 150
            starts = p+d[near]/dist[near, None]*(e['radius']+.6)
            clear = ~soup.blocked_many(starts, targets[near])
            self.assertEqual(int(clear.sum()), 0, (e['id'], targets[near][clear][:5]))
        for name, *_ in self.ROOMS: self.assertGreater(counts.get(name, 0), 20, (name, counts))
        self.assertGreater(len(targets), 1500)

    def test_terrain_is_symmetric_dune_like_and_matches_targets(self):
        spec, _, _, grid = self.whole_map()
        self.assertTrue(np.array_equal(grid, build.terrain_grid(spec)), 'terrain must be deterministic')
        self.assertLess(np.abs(grid[1:, 1:]-grid[1:, 1:][::-1, ::-1]).max(), 1e-6, 'exact 180 degree symmetry')
        slope = dustreach_terrain.slope_degrees(grid)
        rows, cols = slice(int((1024-420)/8), int((1024+420)/8)), slice(int((1024-380)/8), int((1024+380)/8))
        play = slope[rows, cols]
        self.assertTrue(14 <= np.median(play) <= 19, np.median(play))
        self.assertTrue(26 <= np.percentile(play, 90) <= 34, np.percentile(play, 90))
        self.assertLess((play < 3).mean(), .06)
        # The saddle at the gate is high ground; basins lie in front of each citadel.
        prof = lambda z: build.terrain_height(1024, z, grid)
        for sgn in (-1, 1):
            self.assertGreater(prof(1024)-prof(1024+sgn*240), 25)
            self.assertGreater(prof(1024+sgn*345)-prof(1024+sgn*240), 10)
        flags = []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            flags.append(m.point((base.FLAG[0], 0, base.FLAG[1])))
        self.assertTrue(700 <= math.dist(flags[0][::2], flags[1][::2]) <= 750)

    def test_ground_stays_under_structures(self):
        spec, _, _, grid = self.whole_map()
        h = lambda x, z: build.terrain_height(x, z, grid)
        for b in spec['bases']:
            oy = b['position'][1]
            local = lambda x, z: h(*build.to_world(b, x, z))-oy
            # Under the terrace, the tower, the pad and the ruin: at or under the ground line.
            for x in np.arange(-base.TX, base.TX+.1, 2):
                for z in np.arange(base.TZ0, base.TZ1+.1, 2):
                    self.assertLess(local(x, z), .02, ('terrace', x, z))
            sx0, sx1, sz0, sz1 = base.STORE
            for x in np.arange(sx0, sx1+.1, 2):
                for z in np.arange(sz0, sz1+.1, 2):
                    self.assertLess(local(x, z), .02, ('storehouse', x, z))
            px, pz = base.PAD; H = base.PAD_HALF
            for x in np.arange(px-H, px+H+.1, 2):
                for z in np.arange(pz-H, pz+H+.1, 2):
                    self.assertLess(local(x, z), .02, ('pad', x, z))
            cx, cz = base.RUIN
            for r in np.arange(0, base.RUIN_R+.8, 1):
                for a in np.linspace(0, math.tau, 12, endpoint=False):
                    self.assertLess(local(cx+r*math.cos(a), cz+r*math.sin(a)), .02, ('ruin', r))
            # Ramp feet meet the ground: level at the foot, below the surface along it.
            for ramp, fn in [(base.FRONT_RAMP, base.front_ramp_y), (base.BACK_RAMP, base.back_ramp_y),
                             (base.ruin_ramp(), lambda z, r=base.ruin_ramp(): base.ramp_y(r, z, 0, base.RUIN_TOP))]:
                x0, x1, zg, zt = ramp
                self.assertAlmostEqual(local((x0+x1)/2, zg), -.1, delta=.05)
                for z in np.linspace(zg, zt, 9):
                    for x in (x0, x1):
                        self.assertLess(local(x, z), fn(z)+.02, (ramp, x, z))
        gx, gy, gz = spec['gate']['position']
        # The gate's paving (to 22 m out) sits on level ground: exact where the
        # 8 m grid vertices are all inside the flattened disc.
        for r in np.arange(0, gate.SITE_R-12, 2.0):
            for a in np.linspace(0, math.tau, 16, endpoint=False):
                self.assertAlmostEqual(h(gx+r*math.cos(a), gz+r*math.sin(a)), gy-.06, delta=1/32)
        # Midfield ruins stand in their ground: within a metre of their base
        # under the footprint (their walls run 2 m below it).
        for wx, y, wz, _, _ in build.pieces(spec):
            for dx in np.arange(-6, 6.1, 2.0):
                for dz in np.arange(-4, 4.1, 2.0):
                    self.assertLess(abs(h(wx+dx, wz+dz)-(y-.1)), 1.0, (wx, wz, dx, dz))

    def test_gate_is_symmetric_and_the_lane_runs_through_it(self):
        mesh = kit.Mesh(); anchors = gate.build(mesh)
        soup = Soup(mesh.collision)
        tris = np.array(mesh.collision, np.float64).reshape(-1, 3)
        mirrored = tris*np.array([-1, 1, -1])
        key = lambda a: set(map(tuple, np.round(a, 4)))
        self.assertEqual(key(tris), key(mirrored), 'the gate must match its 180 degree rotation')
        for x in (-5, 0, 5):
            for y in (.3, 1.5, 8, 16):
                self.assertEqual(soup.first((x, y, -60), (0, 0, 1)), np.inf, (x, y, 'lane blocked'))
        for x, z in ((0, 0), (8, 0), (-8, 0)):
            t, ny = soup.hits((x, gate.ATTIC_TOP+2 if abs(x) < gate.ATTIC_X else gate.LINTEL_TOP+2, z), (0, -1, 0))
            self.assertLess(t[0], 2.01); self.assertGreater(ny[0], .999)
        self.assertLess(len(mesh.collision)//9, 800)
        pieces = build.pieces(build.spec())
        self.assertEqual(len(pieces), 2*len(gate.SCATTER))
        for (x, y, z, yaw, kind), (x2, y2, z2, yaw2, kind2) in zip(pieces[::2], pieces[1::2]):
            self.assertAlmostEqual(x+x2, 2048); self.assertAlmostEqual(z+z2, 2048)
            self.assertAlmostEqual(y, y2, places=2); self.assertEqual(kind, kind2)

    # --- Pack --------------------------------------------------------------
    def test_lightmap_bake_is_deterministic(self):
        from assets import lightmap_bake
        m = kit.Mesh(); m.lamps = []; gate.build(m)
        verts = np.array(m.vertices, np.float32).reshape(-1, 12)
        sun = (-.57735, .57735, -.57735); light = kit.MATERIALS.index('light')
        a, pa = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=1)
        b, pb = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=3)
        self.assertTrue(np.array_equal(a, b)); self.assertEqual(pa, pb)
        self.assertTrue(np.all((a[:, 8] >= 0) & (a[:, 8] <= 1) & (a[:, 9] >= 0) & (a[:, 9] <= 1)))

    def test_wind_loop_is_deterministic_and_bounded(self):
        a = build.wind(5); self.assertEqual(a, build.wind(5))
        samples = np.frombuffer(a, '<f4')
        self.assertEqual(len(samples), 44100*4)
        self.assertLess(np.abs(samples).max(), 1.0); self.assertGreater(np.abs(samples).max(), .01)

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
        pack = Path(__file__).resolve().parent.parent/'assets/maps/dustreach'
        self.assertEqual(spawn_checks.problems(pack), [])


if __name__ == '__main__':
    unittest.main()
