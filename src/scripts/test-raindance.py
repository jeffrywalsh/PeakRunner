#!/usr/bin/env python3
"""Test the Old Holler build (map key `raindance`) without reading source-game assets.
Run from src/ with the numpy venv: scripts/test-raindance.py"""
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from assets import sightline_checks
from assets import budgets
from assets import raindance_base as base
from assets import raindance_materials
from assets import raindance_structures

HERE = Path(__file__).parent
ROOT = HERE.parent


def _load(name, file):
    loader = importlib.util.spec_from_file_location(name, HERE/file)
    module = importlib.util.module_from_spec(loader); loader.loader.exec_module(module)
    return module


kit = _load('kit', 'build-original-map.py')
build = _load('raindance_build', 'build-raindance.py')
PACK = ROOT/'assets/maps/raindance'
EQUIPMENT = json.loads((ROOT/'maps/raindance.json').read_text())['base_equipment']


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
        for i in range(0, len(starts), 128):
            s = starts[i:i+128][:, None]; d = ends[i:i+128][:, None]-s
            e1, e2, a = self.e1[None], self.e2[None], self.a[None]
            h = np.cross(d, e2); det = (e1*h).sum(-1)
            ok = np.abs(det) > 1e-9; inv = np.where(ok, 1/np.where(ok, det, 1), 0)
            tv = s-a; u = (tv*h).sum(-1)*inv; q = np.cross(tv, e1)
            v = (d*q).sum(-1)*inv; t = (e2*q).sum(-1)*inv
            out[i:i+128] = (ok & (u >= 0) & (v >= 0) & (u+v <= 1) & (t > 1e-4) & (t < 1-1e-4)).any(1)
        return out


class Recording(kit.Mesh):
    """Records every box outside equipment models."""
    def __init__(self):
        super().__init__(); self.boxes = []; self.depth = 0

    def equipment(self, *a, **k):
        self.depth += 1
        try: return super().equipment(*a, **k)
        finally: self.depth -= 1

    group = 'base'

    def box(self, p, size, mat='concrete', solid=True):
        if not self.depth: self.boxes.append((tuple(p), tuple(size), mat, solid, self.group))
        super().box(p, size, mat, solid)


def one_base(team=0, circuit='one', origin=(0, 0, 0), yaw=0.0, mesh=None):
    mesh = mesh or kit.Mesh(); mesh.origin = origin; mesh.yaw = yaw
    return mesh, base.build(mesh, team, circuit, EQUIPMENT)


def box_faces(p, s):
    """(axis, plane, direction, rect) for the six faces of an axis box."""
    lo = [p[k]-s[k]/2 for k in range(3)]; hi = [p[k]+s[k]/2 for k in range(3)]
    for k in range(3):
        u, v = [j for j in range(3) if j != k]
        rect = (lo[u], hi[u], lo[v], hi[v])
        yield k, lo[k], -1, rect
        yield k, hi[k], 1, rect


class RaindanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mesh, cls.anchors = one_base()
        cls.soup = Soup(cls.mesh.collision)
        cls.tmp = tempfile.TemporaryDirectory()
        cls.pack = Path(cls.tmp.name)/'rd'
        build.build(cls.pack, bake=False)
        cls.man = json.loads((cls.pack/'map.json').read_text())
        cls.world = Soup(np.frombuffer((cls.pack/'collision.bin').read_bytes(), '<f4'))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    # --- Routes, floors, headroom --------------------------------------------
    def test_hall_floor_and_roof_have_floor_and_headroom(self):
        for x in np.arange(-28, 28.1, 2):
            for z in np.arange(-24, 24.1, 2):
                if abs(abs(x)-20) < 5.5 and -19.5 < z < base.HOLE_Z[1]+.5: continue   # hall ramps
                if abs(x) < base.STAIR_W/2+.3 and base.B_Z0-.3 < z < base.OPEN_Z1+.3: continue   # atrium stair
                if any(math.hypot(x-e['position'][0], z-e['position'][2]) < 4 for e in EQUIPMENT
                       if abs(e['position'][1]-base.FLOOR) < .01):
                    continue
                t, ny = self.soup.hits((x, base.FLOOR+3, z), (0, -1, 0))
                self.assertAlmostEqual(3-t[0], 0, places=3, msg=(x, z)); self.assertGreater(ny[0], .999)
                self.assertGreater(self.soup.first((x, base.FLOOR+.1, z), (0, 1, 0)), 2.6, (x, z))

    def test_hall_ramps_climb_through_the_roof_openings(self):
        for side in (-1, 1):
            x = side*20
            for z in np.arange(-18, base.HOLE_Z[1]-.3, .5):
                y = base.FLOOR+(base.ROOF_TOP-base.FLOOR)*(z+19)/(base.HOLE_Z[1]+19)
                t, ny = self.soup.hits((x, y+2, z), (0, -1, 0))
                self.assertAlmostEqual(y+2-t[0], y, places=2, msg=(x, z))
                self.assertGreater(self.soup.first((x, y+.1, z), (0, 1, 0)), 2.4, (x, z, 'headroom'))
            # The ramp tops out flush with the roof beyond the opening.
            t, _ = self.soup.hits((x, base.ROOF_TOP+1, base.HOLE_Z[1]+.5), (0, -1, 0))
            self.assertAlmostEqual(base.ROOF_TOP+1-t[0], base.ROOF_TOP, places=3)

    def test_shoulder_ramps_end_flush_with_the_roof_edge(self):
        for x in (-22, 22):
            t, _ = self.soup.hits((x, base.ROOF_TOP+1, -28.05), (0, -1, 0))
            self.assertLess(abs(base.ROOF_TOP+1-t[0]-base.ROOF_TOP), .02)
            t, _ = self.soup.hits((x, base.ROOF_TOP+1, -27.95), (0, -1, 0))
            self.assertAlmostEqual(base.ROOF_TOP+1-t[0], base.ROOF_TOP, places=3)

    def test_nobody_walks_under_a_low_ramp(self):
        """Wherever a ramp's underside is lower than 2.4 m above the floor
        below it, that pocket is sealed on every side."""
        ramps = [(x, 9, -19, base.HOLE_Z[1], base.FLOOR, base.ROOF_TOP, base.FLOOR) for x in (-20, 20)]
        ramps += [(x, 18, -52, -28, base.APRON_TOP, base.ROOF_TOP, base.APRON_TOP) for x in (-22, 22)]
        ramps += [(0, base.STAIR_W, base.B_Z0, base.STAIR_Z1, base.FLOOR, base.B_FLOOR, base.B_FLOOR)]
        sealed = 0
        for x, w, z0, z1, y0, y1, floor in ramps:
            for zz in np.arange(z0+.3, z1, .4):
                under = y0+(y1-y0)*(zz-z0)/(z1-z0)-base.UNDER
                gap = under-floor
                if not .6 < gap < base.HEADROOM-.05: continue
                for xx in np.arange(x-w/2+.3, x+w/2, .9):
                    p = (xx, floor+min(.5, gap/2), zz)
                    for a in np.linspace(0, math.tau, 8, endpoint=False):
                        self.assertLess(self.soup.first(p, (math.cos(a+.1), 0, math.sin(a+.1))), w+30, (p, a))
                    sealed += 1
        self.assertGreater(sealed, 100)

    def test_standing_support_is_the_floor_top_everywhere(self):
        """The support ray starts 0.15 m above the feet; its first hit must be
        the up-facing floor top wherever a player can stand."""
        regions = [(-29, 29, -25, 25, base.FLOOR), (12.2, 32.3, -27.8, 28.3, base.ROOF_TOP),
                   (-32.3, -12.2, -27.8, 28.3, base.ROOF_TOP), (-11.8, 11.8, 4.2, 19.8, base.ROOF_TOP),
                   (-10.8, 10.8, base.B_Z0+.2, base.B_Z1-.2, base.B_FLOOR),
                   (base.DOOR_X[0]+.2, base.DOOR_X[1]-.2, base.B_Z1+1.2, base.STRIP_Z[1]-.2, base.B_FLOOR)]
        checked = 0
        for x0, x1, z0, z1, level in regions:
            for x in np.arange(x0, x1+.01, .8):
                for z in np.arange(z0, z1+.01, .8):
                    if level == base.ROOF_TOP and math.hypot(x, z-base.TZ) < 7.7: continue   # inside the tower
                    t, ny = self.soup.hits((x, level+2.6, z), (0, -1, 0))
                    if not len(t) or abs(2.6-t[0]) > .02: continue
                    first, fy = self.soup.hits((x, level+.15, z), (0, -1, 0))
                    checked += 1
                    self.assertTrue(len(first) and first[0] < .17 and fy[0] > .99, (x, z, level, first[:2]))
        self.assertGreater(checked, 2500)

    def test_flag_tower_is_sealed_from_below(self):
        for dx, dz in [(0, 2), (3, 3), (-2, 1), (4, 5), (-6, 4)]:     # beyond the front deck roof (z > 20)
            t = self.soup.first((dx, 0, base.TZ+dz), (0, 1, 0))
            self.assertAlmostEqual(t, base.ROOF_TOP, places=3, msg=(dx, dz))

    # --- Bishop flag tower ----------------------------------------------------
    def test_chamber_floor_and_ledge_are_standable_with_headroom(self):
        """The whole chamber floor and the ledge round the collar are the
        floor top, with room to stand."""
        checked = 0
        for x in np.arange(-6.8, 6.81, .4):
            for z in np.arange(-6.8, 6.81, .4):
                r = math.hypot(x, z)
                inside = r < base.R_IN-.3
                ledge = base.R_OUT+.3 < r < base.R_LEDGE-.1
                if not (inside or ledge): continue
                p = (x, base.CH_FLOOR+.15, base.TZ+z)
                first, fy = self.soup.hits(p, (0, -1, 0))
                self.assertTrue(len(first) and first[0] < .17 and fy[0] > .99, (x, z, first[:2]))
                self.assertGreater(self.soup.first((x, base.CH_FLOOR+.1, base.TZ+z), (0, 1, 0)), 2.6, (x, z))
                checked += 1
        self.assertGreater(checked, 500)

    def test_opposite_doors_line_up_straight_through(self):
        """Four doors (front -Z, east +X, back +Z, west -X), 3 m wide and 4.5 m
        tall: a body band enters one door, crosses the chamber floor over the
        flag and leaves through the opposite door with nothing in the way,
        so a jetting player can fly straight through and grab the flag."""
        for (x, z), (dx, dz) in (((0, -12), (0, 1)), ((12, 0), (-1, 0)), ((0, 12), (0, -1)), ((-12, 0), (1, 0))):
            for h in (.3, 1., 1.8, 2.5, 3.2, 4.1):
                for off in (-.8, 0, .8):
                    o = (x+off*abs(dz), base.CH_FLOOR+h, base.TZ+z+off*abs(dx))
                    self.assertGreater(self.soup.first(o, (dx, 0, dz), 24), 24-1e-3, msg=(x, z, h, off))
            o = (x, base.CH_LINTEL+.15, base.TZ+z)                       # lintel
            self.assertLess(self.soup.first(o, (dx, 0, dz), 12), 12-base.R_OUT_L+.02)
        self.assertEqual(len(base.DOORS), 4)

    def test_slit_has_a_lane_for_a_standing_body(self):
        """A 1.04 m wide, 2.56 m tall body box flies straight in through the
        mitre slit along its facing and on to 1.5 m from the axis."""
        sx, sz = base.SLIT_DIR; ax, az = -sz, sx
        lanes = []
        for a in np.arange(-2, 2.01, .25):
            for feet in np.arange(14.5, 21, .25):
                ok = all(self.soup.first((sx*9+ax*(a+off), base.ROOF_TOP+feet+dh, base.TZ+sz*9+az*(a+off)), (-sx, 0, -sz), 7.5) > 7.49
                         for off in np.linspace(-.52, .52, 5) for dh in np.linspace(0, 2.56, 6))
                if ok: lanes.append((a, feet))
        self.assertGreater(len(lanes), 12, lanes)
        # The lane the movement test flies (sim.rs): 0.85 m along the slit, feet 16.5 m up.
        self.assertIn((-.75, 16.5), [(round(a, 2), round(f, 2)) for a, f in lanes])

    def test_the_flag_tower_reads_as_a_bishop(self):
        """Silhouette: wide plinth, narrow stem, collar, wider mitre, taller
        than the old 18 m spire, sealed at the top apart from the slit."""
        prof = {}
        for h in (.5, 5, 8.6, 12, 17.3, 22):
            rs = [self.soup.first((math.cos(a)*12, base.ROOF_TOP+h, base.TZ+math.sin(a)*12), (-math.cos(a), 0, -math.sin(a)), 12)
                  for a in np.linspace(0, math.pi, 7)]                # front half, away from the slit
            prof[h] = 12-min(rs)
        self.assertGreater(prof[.5], prof[5]); self.assertGreater(prof[8.6], prof[5])      # plinth, collar
        self.assertGreater(prof[17.3], prof[12]); self.assertGreater(prof[17.3], prof[22])  # mitre bulge
        t = self.soup.first((0, base.CH_FLOOR+1, base.TZ), (0, 1, 0))
        self.assertGreater(base.CH_FLOOR+1+t-base.ROOF_TOP, 18.0)          # taller than the old spire

    # --- Generator basement ---------------------------------------------------
    def test_basement_floor_has_headroom_over_the_generator(self):
        gx, _, gz = next(e['position'] for e in EQUIPMENT if e['kind'] == 'generator')
        checked = 0
        for x in np.arange(-10, 10.1, 1.5):
            for z in np.arange(base.B_Z0+1, base.B_Z1-.9, 1.5):
                if abs(x) < base.STAIR_W/2+.3 and z < base.STAIR_Z1+.3: continue   # stair wedge
                if abs(x-gx) < 3 and abs(z-gz) < 2.5: continue                        # generator plinth
                t, ny = self.soup.hits((x, base.B_FLOOR+3, z), (0, -1, 0))
                self.assertAlmostEqual(3-t[0], 0, places=3, msg=(x, z)); self.assertGreater(ny[0], .999)
                self.assertGreater(self.soup.first((x, base.B_FLOOR+.1, z), (0, 1, 0)), 7.9, (x, z))
                checked += 1
        self.assertGreater(checked, 150)
        # 8 m clear: flyable, and the generator (5.8 m) clears the ceiling.
        self.assertAlmostEqual(base.B_CEIL-base.B_FLOOR, 8.0, places=4)

    def test_both_stairs_are_walkable_with_headroom(self):
        for z in np.arange(base.B_Z0+.3, base.STAIR_Z1-.2, .5):         # atrium stair
            y = base.FLOOR+(base.B_FLOOR-base.FLOOR)*(z-base.B_Z0)/(base.STAIR_Z1-base.B_Z0)
            for x in (-1.3, 0, 1.3):
                t, ny = self.soup.hits((x, y+2, z), (0, -1, 0))
                self.assertAlmostEqual(y+2-t[0], y, places=2, msg=(x, z)); self.assertGreater(ny[0], .8)
                self.assertGreater(self.soup.first((x, y+.1, z), (0, 1, 0)), 2.6, (x, z, 'headroom'))
        zc = sum(base.STRIP_Z)/2
        for x in np.arange(base.SERVICE_X[0]+.2, base.SERVICE_X[1]-.1, .5):   # service stair
            y = base.service_y(x)
            for z in (zc-1.2, zc, zc+1.2):
                t, ny = self.soup.hits((x, y+2, z), (0, -1, 0))
                self.assertAlmostEqual(y+2-t[0], y, places=2, msg=(x, z)); self.assertGreater(ny[0], .8)
                self.assertGreater(self.soup.first((x, y+.1, z), (0, 1, 0)), 2.6, (x, z, 'headroom'))
        # Its top lands on the shed floor at ground level, and the door is open.
        t, _ = self.soup.hits((31, 1, zc), (0, -1, 0)); self.assertAlmostEqual(1-t[0], 0, places=3)
        self.assertEqual(self.soup.first((31, 1.2, zc), (1, 0, 0)), np.inf)
        self.assertGreater(self.soup.first((31, .1, zc), (0, 1, 0)), 2.8)

    def test_underground_spaces_do_not_leak_into_the_void(self):
        """Below ground, every sideways or downward ray from the basement,
        passage and service stair meets geometry: nothing opens onto the
        empty cut beneath the hall (terrain is absent in cut cells)."""
        dirs = [(math.cos(a)*c, -s, math.sin(a)*c) for s, c in ((0, 1), (.5, .866), (1, 0))
                for a in np.linspace(0, math.tau, 12, endpoint=False)]
        zc = sum(base.STRIP_Z)/2
        points = [(x, base.B_FLOOR+h, z) for x in (-9, -4, 4, 9) for z in (-4, 12, 22) for h in (1, 4)]
        points += [(-3, base.B_FLOOR+1, z) for z in (26, 30)]
        points += [(x, base.service_y(x)+1.5, zc) for x in (2, 8, 14, 19)]
        for p in points:
            for d in dirs:
                self.assertLess(self.soup.first(p, d), 80, (p, d))

    def test_generators_sit_in_cut_cells_on_the_basement_floor(self):
        holes = set(self.man['holes'])
        gens = [e for e in self.man['entities'] if e['kind'] == 'generator']
        self.assertEqual(len(gens), 2)
        for e, b in zip(gens, sorted(build.spec()['bases'], key=lambda b: b['team'])):
            x, y, z = e['position']
            self.assertIn(int(z//8)*256+int(x//8), holes, e['id'])
            t, _ = self.world.hits((x, y-1, z), (0, -1, 0))
            self.assertAlmostEqual(y-1-t[0], b['position'][1]+base.B_FLOOR+1, places=2, msg=e['id'])

    # --- Z-fighting ----------------------------------------------------------
    def test_no_two_boxes_share_a_visible_face(self):
        """No two structure boxes (equipment models aside) put same-facing
        faces in one plane over a shared area: that is what z-fights."""
        mesh = Recording(); mesh.lamps = []
        base.build(mesh, 0, 'one', EQUIPMENT)
        mesh.group = 'bunker'; raindance_structures.bunker(mesh)
        mesh.group = 'pad'; raindance_structures.landing_pad(mesh, 0)
        faces = [f+(i,) for i, (p, s, _, _, _) in enumerate(mesh.boxes) for f in box_faces(p, s)]
        by_plane = {}
        for k, plane, d, rect, i in faces:     # structures stand apart; compare within each
            by_plane.setdefault((mesh.boxes[i][4], k, round(plane, 4), d), []).append((rect, i))
        for key, items in by_plane.items():
            for a in range(len(items)):
                for b in range(a+1, len(items)):
                    (ra, ia), (rb, ib) = items[a], items[b]
                    if ia == ib: continue
                    du = min(ra[1], rb[1])-max(ra[0], rb[0]); dv = min(ra[3], rb[3])-max(ra[2], rb[2])
                    self.assertFalse(du > 1e-4 and dv > 1e-4, (key, mesh.boxes[ia], mesh.boxes[ib]))

    # --- Spawns, flag, equipment ----------------------------------------------
    def test_spawn_points_stand_on_floor_with_room_and_face_open_space(self):
        points = self.anchors['spawn_points']
        self.assertEqual(len(points), 8)
        for x, y, z, yaw in points:
            floor = y-base.LIFT
            t, ny = self.soup.hits((x, y-.37, z), (0, -1, 0))
            self.assertAlmostEqual(y-.37-t[0], floor, places=3); self.assertGreater(ny[0], .999)
            first, _ = self.soup.hits((x, floor+.15, z), (0, -1, 0))
            self.assertLess(first[0], .17, (x, z, 'support ray starts above the floor top'))
            self.assertGreater(self.soup.first((x, floor+.2, z), (0, 1, 0)), 2.4, (x, z, 'headroom'))
            for ang in np.linspace(0, 2*np.pi, 8, endpoint=False):
                for h in (.3, 1., 1.8):
                    self.assertGreater(self.soup.first((x, floor+h, z), (np.cos(ang), 0, np.sin(ang))), .7, (x, z, h))
            facing = (-math.sin(yaw), 0, -math.cos(yaw))
            self.assertGreater(self.soup.first((x, floor+1.6, z), facing), 5., (x, z, 'faces a wall'))

    def test_manifest_spawn_points_are_world_space_per_team(self):
        self.assertEqual([len(t) for t in self.man['spawn_points']], [8, 8])
        bases = sorted(build.spec()['bases'], key=lambda b: b['team'])
        for team, b in enumerate(bases):
            for x, y, z, yaw in self.man['spawn_points'][team]:
                self.assertLess(math.hypot(x-b['position'][0], z-b['position'][2]), 40)   # at their own base
                self.assertTrue(0 <= yaw < math.tau)

    def test_flag_stands_in_the_bishop_chamber(self):
        """On the chamber floor at the tower's axis, under the hollow mitre, and
        away from both doors and the slit (no spawn inside the tower)."""
        fx, fy, fz = self.anchors['flag']
        self.assertEqual((fx, fz), (0, base.TZ))
        t, ny = self.soup.hits((fx, fy+1, fz), (0, -1, 0))
        self.assertAlmostEqual(fy+1-t[0], base.CH_FLOOR, places=3); self.assertGreater(ny[0], .999)
        self.assertGreater(self.soup.first((fx, fy+.1, fz), (0, 1, 0)), 12)
        for x, y, z, _ in self.anchors['spawn_points']:
            self.assertLess(y, base.CH_FLOOR-5, 'spawns stay out of the flag tower')

    def test_equipment_is_unchanged_except_the_roof_sensor(self):
        with tempfile.TemporaryDirectory() as tmp:
            spec = json.loads((ROOT/'maps/raindance.json').read_text())
            spec['base_equipment'] = [dict(e, position=[-23, 8.6, 18]) if e['kind'] == 'sensor' else e
                                      for e in spec['base_equipment']]
            old = kit.build(spec, Path(tmp)/'kit')
        new = self.man['entities']
        self.assertEqual(len(new), 18)
        self.assertEqual([(e['id'], e['kind'], e['team'], e['weapon']) for e in old['entities']],
                         [(e['id'], e['kind'], e['team'], e['weapon']) for e in new])
        moved = [(a['id'], a['position'], b['position']) for a, b in zip(old['entities'], new) if list(a['position']) != list(b['position'])]
        self.assertEqual([m[0] for m in moved], ['sensor-4', 'sensor-11'])
        for _, a, b in moved: self.assertAlmostEqual(math.dist(a, b), 5, places=4)
        # The flags moved from the exposed roof stand into the bishop towers.
        for b, flag in zip(sorted(build.spec()['bases'], key=lambda b: b['team']), self.man['flags']):
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for got, want in zip(flag, m.point(self.anchors['flag'])): self.assertAlmostEqual(got, want, places=4)

    def test_terrain_holes_and_scenery_match_the_original_kit(self):
        with tempfile.TemporaryDirectory() as tmp:
            old = kit.build(json.loads((ROOT/'maps/raindance.json').read_text()), Path(tmp)/'kit')
            for name in ('height.bin', 'weights.rgba', 'ambient.f32'):
                self.assertEqual((Path(tmp)/'kit'/name).read_bytes(), (self.pack/name).read_bytes(), name)
        self.assertEqual(old['holes'], self.man['holes'])
        scenery = lambda m: [(i['asset'], list(i['position']), i['yaw']) for i in m['instances'] if i['asset'] in ('tree', 'rock')]
        self.assertEqual(scenery(old), scenery(self.man))
        self.assertEqual(len(scenery(self.man)), 195)

    def test_turrets_cannot_see_into_the_halls(self):
        """No turret engages a player (field of fire, range and a clear line
        from its barrel start to the chest, as the server does) anywhere in
        either hall, basement, service passage or flag chamber, beyond the
        doorway depth of an open opening (sightline_checks.DOOR_DEPTH)."""
        targets, openings = [], []
        for b in json.loads((ROOT/'maps/raindance.json').read_text())['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for x in np.arange(-29, 29.1, 1.5):
                for z in np.arange(-25, 25.1, 1.5):
                    targets.append(m.point((x, base.FLOOR+base.LIFT+.8, z)))
            # Basement, passage and the service stair below the shed.
            for x in np.arange(-10, 10.1, 1.5):
                for z in np.arange(base.B_Z0+.5, base.B_Z1, 1.5):
                    targets.append(m.point((x, base.B_FLOOR+base.LIFT+.8, z)))
            for z in np.arange(base.B_Z1+1.5, base.STRIP_Z[1], 1.5):
                targets.append(m.point((-3, base.B_FLOOR+base.LIFT+.8, z)))
            for x in np.arange(base.SERVICE_X[0]+.5, base.CEIL_END, 1.5):
                targets.append(m.point((x, base.service_y(x)+base.LIFT+.8, sum(base.STRIP_Z)/2)))
            # The whole flag chamber floor; its four doors are open.
            for x in np.arange(-4.6, 4.61, .5):
                for z in np.arange(-4.6, 4.61, .5):
                    if math.hypot(x, z) < base.R_IN-.55:
                        targets.append(m.point((x, base.CH_FLOOR+base.LIFT+.8, base.TZ+z)))
            pt = lambda x, z: tuple(m.point((x, 0, z))[i] for i in (0, 2))
            w = 1.35                                                    # half a door at the inner face
            openings += [(pt(-w, base.TZ-base.R_IN), pt(w, base.TZ-base.R_IN)),
                         (pt(base.R_IN, base.TZ-w), pt(base.R_IN, base.TZ+w)),
                         (pt(-w, base.TZ+base.R_IN), pt(w, base.TZ+base.R_IN)),
                         (pt(-base.R_IN, base.TZ-w), pt(-base.R_IN, base.TZ+w))]
        targets = np.array(targets)
        allowed = sightline_checks.near_openings(targets, openings)
        turrets = [e for e in self.man['entities'] if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        for e in turrets:
            self.assertLess(e['arc'], 360, e['id'])
            seen = sightline_checks.visible(self.world, e, targets) & ~allowed
            self.assertEqual(int(seen.sum()), 0, (e['id'], targets[seen][:5]))

    def test_transform_invariance_and_budget(self):
        a, anchors = one_base(0, 'one')
        b, moved = one_base(1, 'two', (321, 75, 987), .73)
        self.assertEqual(anchors, moved)
        self.assertEqual(len(a.collision), len(b.collision))
        for i in range(0, len(a.collision), 3):
            expected = b.point(tuple(a.collision[i:i+3]))
            for x, y in zip(expected, b.collision[i:i+3]): self.assertAlmostEqual(x, y, places=3)
        self.assertTrue(all(e['team'] == 1 and e['circuit'] == 'two' for e in b.entities))
        self.assertLess(len(b.collision)//9, budgets.COLLISION_TRIS_PER_BASE)

    def test_two_instances_have_unique_ids_and_independent_circuits(self):
        mesh = kit.Mesh(); base.build(mesh, 0, 'red', EQUIPMENT)
        mesh.origin = (2048, 0, 2048); mesh.yaw = math.pi; base.build(mesh, 1, 'blue', EQUIPMENT)
        ids = [e['id'] for e in mesh.entities]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual({e['circuit'] for e in mesh.entities}, {'red', 'blue'})
        for team in (0, 1):
            self.assertEqual(sum(e['kind'] == 'generator' and e['team'] == team for e in mesh.entities), 1)

    # --- Materials, mips, bake, determinism -----------------------------------
    def test_materials_are_deterministic_opaque_and_distinct(self):
        names = build.PAINTED
        tex = {}
        for name in names:
            a = raindance_materials.texture(name, 7, kit.noise, kit.value_noise)
            self.assertEqual(a, raindance_materials.texture(name, 7, kit.noise, kit.value_noise), name)
            self.assertEqual(len(a), 256*256*4)
            self.assertTrue(all(a[i] == 255 for i in range(3, len(a), 4)), name)
            tex[name] = a
        for i, a in enumerate(names):
            for b in names[i+1:]: self.assertNotEqual(tex[a], tex[b], (a, b))
            self.assertNotEqual(tex[a], kit.texture(a, 7), a)

    def test_every_texture_layer_has_a_full_mip_chain(self):
        count = self.man['texture_count']
        data = (self.pack/'textures.rgba').read_bytes()
        self.assertEqual(len(data), count*349524)
        for name in build.PAINTED:
            layer = kit.MATERIALS.index(name)
            top = data[layer*65536*4:(layer+1)*65536*4]
            mip = data[count*65536*4+layer*16384*4:count*65536*4+(layer+1)*16384*4]
            self.assertEqual(kit.downsample(top, 256), mip, name)
            self.assertEqual(top, raindance_materials.texture(name, 84271+layer, kit.noise, kit.value_noise), name)

    def test_lightmap_bake_is_deterministic(self):
        from assets import lightmap_bake
        m = kit.Mesh(); m.lamps = []; base.build(m, 0, 'one', EQUIPMENT)
        verts = np.array(m.vertices, np.float32).reshape(-1, 12)
        sun = (-.57735, .57735, -.57735); light = kit.MATERIALS.index('light')
        a, pa = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=1)
        b, pb = lightmap_bake.bake(verts, m.lamps, sun, light, 20, processes=3)
        self.assertTrue(np.array_equal(a, b)); self.assertEqual(pa, pb)
        self.assertTrue(np.all((a[:, 8] >= 0) & (a[:, 8] <= 1) & (a[:, 9] >= 0) & (a[:, 9] <= 1)))
        self.assertTrue(np.all(np.isfinite(a)))

    def test_committed_pack_lists_its_hashes_and_provenance(self):
        man = json.loads((PACK/'map.json').read_text())
        self.assertIn('no extracted assets', man['provenance'])
        for name, digest in man['files'].items():
            self.assertEqual(hashlib.sha256((PACK/name).read_bytes()).hexdigest(), digest, name)
        self.assertEqual(man['base_asset_sha256'], hashlib.sha256(Path(base.__file__).read_bytes()).hexdigest())
        self.assertEqual(man['definition_sha256'], hashlib.sha256((ROOT/'maps/raindance.json').read_bytes()).hexdigest())

    def test_unbaked_rebuilds_are_byte_identical(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)/'again'
            build.build(out, bake=False)
            for f in self.pack.iterdir():
                self.assertEqual(f.read_bytes(), (out/f.name).read_bytes(), f.name)

    # --- Field structures, render z-fighting, look (v5) -----------------------
    def test_bunker_is_tall_with_two_openings(self):
        """Ceiling 7 m over the floor, open at the front ramp and through the
        6 m back opening, with a ramp down to the ground on each side."""
        m = kit.Mesh(); raindance_structures.bunker(m); soup = Soup(m.collision)
        top = raindance_structures.B_FLOOR_TOP
        for x in (-6, 0, 6):
            for z in (-6, 0, 5):
                t, ny = soup.hits((x, top+2, z), (0, -1, 0))
                self.assertAlmostEqual(2-t[0], 0, places=3); self.assertGreater(ny[0], .999)
                self.assertAlmostEqual(soup.first((x, top+.1, z), (0, 1, 0)), 6.9, places=3)
        for h in (.5, 2, 4, 5.2):
            for x in (-2.4, 0, 2.4):
                self.assertEqual(soup.first((x, top+h, 0), (0, 0, 1), 30), np.inf, ('back', x, h))
                self.assertEqual(soup.first((x, top+h, 0), (0, 0, -1), 30), np.inf, ('front', x, h))

    def test_pads_and_bunkers_reach_below_the_ground(self):
        """No field structure edge hovers over the terrain: each landing pad's
        foundation and each bunker's slab reach below the ground at their rims."""
        d = build.spec()
        for o in d['objects']:
            if o['asset'] not in ('landing_pad', 'bunker'): continue
            x, y, z = o['position']
            if o.get('grounded'): y = kit.height(x, z, d)
            bottom = y-4 if o['asset'] == 'landing_pad' else y-2.8
            r = 10.6 if o['asset'] == 'landing_pad' else 14.9
            for a in np.linspace(0, math.tau, 24, endpoint=False):
                ground = kit.height(x+r*math.cos(a), z+r*math.sin(a), d)
                self.assertLess(bottom, ground-.5, (o['id'], a, ground))

    def test_render_mesh_has_no_same_facing_overlaps(self):
        """No two visible structure triangles of different materials lie in
        one plane, facing the same way, over each other (they would z-fight).
        Equipment models are the kit's and are left out."""
        class Tracked(kit.Mesh):
            def __init__(self): super().__init__(); self.skip = []; self.depth = 0
            def equipment(self, *a, **k):
                start = len(self.vertices); self.depth += 1
                try: return super().equipment(*a, **k)
                finally:
                    self.depth -= 1
                    if not self.depth: self.skip.append((start, len(self.vertices)))
        m = Tracked(); m.lamps = []
        base.build(m, 0, 'one', EQUIPMENT)
        m.origin = (200, 0, 0); raindance_structures.bunker(m)
        m.origin = (-200, 0, 0); raindance_structures.landing_pad(m, 0)
        v = np.array(m.vertices, np.float64).reshape(-1, 3, 12)
        keep = np.ones(len(v), bool)
        for a, b in m.skip: keep[a//36:b//36] = False
        v = v[keep]
        p = v[:, :, :3]; mat = v[:, 0, 10]
        n = np.cross(p[:, 1]-p[:, 0], p[:, 2]-p[:, 0]); ln = np.linalg.norm(n, axis=1)
        ok = np.nonzero(ln > 1e-6)[0]; n = n[ok]/ln[ok, None]
        d = np.einsum('ij,ij->i', n, p[ok, 0])
        groups = {}
        for k, i in enumerate(ok):
            groups.setdefault((tuple(np.round(n[k], 3)), round(float(d[k])/.004)), []).append(i)
        def inside(pt, tri, keep):
            a, b, c = tri[:, keep]; v0, v1, v2 = c-a, b-a, pt-a
            d00, d01, d11, d20, d21 = v0@v0, v0@v1, v1@v1, v2@v0, v2@v1
            den = d00*d11-d01*d01
            if abs(den) < 1e-12: return False
            u = (d11*d20-d01*d21)/den; w = (d00*d21-d01*d20)/den
            return u > .02 and w > .02 and u+w < .98
        bad = []
        for (normal, _), items in groups.items():
            if len(items) < 2: continue
            keep_axes = [c for c in range(3) if c != int(np.abs(normal).argmax())]
            for a in range(len(items)):
                for b in range(a+1, len(items)):
                    i, j = items[a], items[b]
                    if mat[i] == mat[j]: continue
                    if inside(p[j].mean(0)[keep_axes], p[i], keep_axes) or inside(p[i].mean(0)[keep_axes], p[j], keep_axes):
                        bad.append((p[i].mean(0).round(2).tolist(), int(mat[i]), int(mat[j])))
        self.assertEqual(bad, [])

    def test_capture_and_hold_points(self):
        """Three towers (docs/capture-and-hold.md): the knolls mirror each
        other through the map centre on flat, tree-free ground; the Crossing
        stands on a platform level with the bridge deck, touching its west
        side at mid-span. None runs in CTF."""
        from assets import cnh_tower
        d = build.spec(); pts = self.man['control_points']
        self.assertEqual([p['id'] for p in pts], ['crossing', 'west-knoll', 'east-knoll'])
        self.assertTrue(all(p['radius'] == cnh_tower.RING and not p['ctf_active'] for p in pts))
        (wx, wy, wz), (ex, ey, ez) = pts[1]['pos'], pts[2]['pos']
        self.assertAlmostEqual(wx+ex, 1960); self.assertAlmostEqual(wz+ez, 1880)
        scen = np.array([i['position'][::2] for i in self.man['instances'] if i['asset'] in ('tree', 'rock')])
        for p in pts:
            x, y, z = p['pos']
            self.assertGreater(np.min(np.hypot(*(scen-[x, z]).T)), 16, p['id'])
            if p['id'] == 'crossing': continue
            rim = [kit.height(x+3.8*math.cos(a), z+3.8*math.sin(a), d) for a in np.linspace(0, math.tau, 24, endpoint=False)]
            self.assertLess(max(rim)-min(rim), cnh_tower.MAX_TILT, p['id'])
            self.assertAlmostEqual(y, kit.height(x, z, d), places=3)
        x, y, z = pts[0]['pos']
        self.assertAlmostEqual(y, build.DECK_TOP)
        self.assertAlmostEqual(x+build.PLATFORM_R, 1000-6.5)             # bridge's west edge
        for r in (6, 9, 11.8):                                         # the ring is standable deck
            for a in np.linspace(0, math.tau, 12, endpoint=False):
                t, ny = self.world.hits((x+r*math.cos(a), y+2, z+r*math.sin(a)), (0, -1, 0))
                self.assertAlmostEqual(y+2-t[0], y, places=2); self.assertGreater(ny[0], .99)

    def test_old_holler_has_its_own_look(self):
        """An overcast procedural sky and valley mist. The sun keeps the
        direction the lightmaps were baked with."""
        look = self.man['look']
        self.assertNotIn('sun_direction', look)
        self.assertGreater(look['sky']['cloud_cover'], .7)
        self.assertLess(look['sun_disc'], .5)
        self.assertGreater(look['height_fog']['density'], 0)
        self.assertIn('fogColor', self.man['sky'])
        for other in ('tower-complex', 'stonehenge-clone', 'cairnhold'):
            f = ROOT/'assets/maps'/other/'map.json'
            if f.exists():
                self.assertNotEqual(json.loads(f.read_text()).get('look', {}).get('sky'), look['sky'], other)


class RouteCounts(unittest.TestCase):
    """Routes on the committed pack (assets/route_checks.py). Walking: more
    than one way into each hall, and exactly two into the generator basement
    (the atrium stair and the service stair). Jetting: five ways into each
    bishop flag chamber: the four doors and the mitre slit.
    The chamber is round, so it is counted with a round region; entries at
    floor height are the doors, higher ones come in over the top through the
    slit (the over-the-top hop, or a drop from the slit's lip)."""
    def test_base_routes(self):
        from assets import route_checks as rc
        pack = rc.Pack(PACK)
        for b in build.spec()['bases']:
            ox, oy, oz = b['position']; a = math.radians(b['yaw']); c, s = math.cos(a), math.sin(a)
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = a
            def box(lx0, lx1, ly0, ly1, lz0, lz1):
                (ax, az), (bx, bz) = [(ox+lx*c+lz*s, oz-lx*s+lz*c) for lx, lz in ((lx0, lz0), (lx1, lz1))]
                return (min(ax, bx), max(ax, bx), oy+ly0, oy+ly1, min(az, bz), max(az, bz))
            found = rc.base_entries(pack, (ox, oz), {
                'hall': box(-29.5, 29.5, base.FLOOR-1, base.FLOOR+1.5, -25.5, 25.5),
                'generator': box(-base.B_X, base.B_X, base.B_FLOOR-.5, base.B_FLOOR+1.5, base.B_Z0, base.B_Z1)})
            self.assertGreaterEqual(len(found['hall']), 2, (b['id'], found['hall']))
            self.assertEqual(len(found['generator']), 2, (b['id'], found['generator']))
            # The flag chamber, airborne model.
            g = rc.Graph(pack, ox-64, ox+64, oz-64, oz+64, airborne=True)
            seeds = list(rc.open_ground(g, ox, oz, 48.0)) + [i for i in range(len(g.nodes)) if g.structure(i) and g.open_sky(i)]
            cx, _, cz = m.point((0, 0, base.TZ)); floor = oy+base.CH_FLOOR
            n = g.nodes
            inside = (np.hypot(n[:, 0]-cx, n[:, 2]-cz) < base.R_IN) & (np.abs(n[:, 1]-floor) < .5)
            vol = (cx-base.R_IN, cx+base.R_IN, floor-.5, floor+1.5+rc.HEADROOM, cz-base.R_IN, cz+base.R_IN)
            # The slit drops players onto the floor near the back door, so its
            # crossings and the back door's cluster together; count the doors
            # without the over-the-top hops, then find the slit with them.
            doors = [e for e in rc.entries(g, inside, seeds, vol) if e[1]-floor < .5]
            entries = rc.entries(g, inside, seeds, vol, overhead=(7, 8, 9, 10))
            mitre = [e for e in entries if e[1]-floor >= .5]
            want = [m.point(p) for p in self.door_points()]
            self.assertEqual(len(doors), 4, (b['id'], entries))
            for w in want:
                self.assertLess(min(math.hypot(e[0]-w[0], e[2]-w[2]) for e in doors), 1.5, (b['id'], w, doors))
            self.assertGreaterEqual(len(mitre), 1, (b['id'], entries))
            sx, _, sz = [p-q for p, q in zip(m.point((base.SLIT_DIR[0], 0, base.TZ+base.SLIT_DIR[1])), (cx, 0, cz))]
            for e in mitre: self.assertGreater((e[0]-cx)*sx+(e[2]-cz)*sz, 0, (b['id'], 'mitre entry not at the slit', e))

    def test_distinct_flag_routes(self):
        """route_checks.flag_routes on the bishop chamber (airborne): each of
        the four doors and the mitre slit lands straight in the flag's zone,
        so each is one route. Five per flag today, a regression floor; the
        pipeline's target is about ten (docs/map-pipeline.md)."""
        from assets import route_checks as rc
        pack = rc.Pack(PACK)
        for b in build.spec()['bases']:
            ox, oy, oz = b['position']
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            cx, _, cz = m.point((0, 0, base.TZ)); floor = oy+base.CH_FLOOR; r = base.R_IN
            found = rc.flag_routes(pack, (cx, cz), (cx-r, cx+r, floor-.5, floor+14, cz-r, cz+r),
                                   (cx-2.5, cx+2.5, floor-.5, floor+.8, cz-2.5, cz+2.5), airborne=True)
            self.assertGreaterEqual(found['routes'], 5, (b['id'], [len(a) for a in found['approaches']]))

    @staticmethod
    def door_points():
        """Where each door's crossing lands: just inside the inner wall."""
        r = base.R_IN-.2
        return [(0, base.CH_FLOOR, base.TZ-r), (r, base.CH_FLOOR, base.TZ),
                (0, base.CH_FLOOR, base.TZ+r), (-r, base.CH_FLOOR, base.TZ)]


class SpawnForwardClearance(unittest.TestCase):
    """Every committed spawn faces open floor: a clear body-width view for
    6 m and the same floor for a half-second walk (docs/map-pipeline.md)."""
    def test_committed_spawns_face_open_floor(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/raindance'
        self.assertEqual(spawn_checks.problems(pack), [])

    def test_no_indoor_spawn_shows_through_an_opening_from_the_field(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/raindance'
        self.assertEqual(spawn_checks.exposed(pack), [])


if __name__ == '__main__':
    unittest.main()
