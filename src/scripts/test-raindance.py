#!/usr/bin/env python3
"""Test the cleaned Raindance build without reading source-game assets.
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

    def box(self, p, size, mat='concrete', solid=True):
        if not self.depth: self.boxes.append((tuple(p), tuple(size), mat, solid))
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
                if any(math.hypot(x-e['position'][0], z-e['position'][2]) < 4 for e in EQUIPMENT if e['position'][1] < 0):
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
                   (-32.3, -12.2, -27.8, 28.3, base.ROOF_TOP), (-11.8, 11.8, 4.2, 19.8, base.ROOF_TOP)]
        checked = 0
        for x0, x1, z0, z1, level in regions:
            for x in np.arange(x0, x1+.01, .8):
                for z in np.arange(z0, z1+.01, .8):
                    t, ny = self.soup.hits((x, level+2.6, z), (0, -1, 0))
                    if not len(t) or abs(2.6-t[0]) > .02: continue
                    first, fy = self.soup.hits((x, level+.15, z), (0, -1, 0))
                    checked += 1
                    self.assertTrue(len(first) and first[0] < .17 and fy[0] > .99, (x, z, level, first[:2]))
        self.assertGreater(checked, 2500)

    def test_spire_is_sealed_from_below(self):
        for dx, dz in [(0, 0), (3, 1), (-2, -1), (4, 2)]:     # all beyond the flag-deck roof (z > 20)
            t = self.soup.first((dx, 0, 22+dz), (0, 1, 0))
            self.assertAlmostEqual(t, base.ROOF_TOP, places=3, msg=(dx, dz))

    # --- Z-fighting ----------------------------------------------------------
    def test_no_two_boxes_share_a_visible_face(self):
        """No two structure boxes (equipment models aside) put same-facing
        faces in one plane over a shared area: that is what z-fights."""
        mesh = Recording(); mesh.lamps = []
        base.build(mesh, 0, 'one', EQUIPMENT)
        raindance_structures.bunker(mesh); raindance_structures.landing_pad(mesh, 0)
        faces = [f+(i,) for i, (p, s, _, _) in enumerate(mesh.boxes) for f in box_faces(p, s)]
        by_plane = {}
        for k, plane, d, rect, i in faces: by_plane.setdefault((k, round(plane, 4), d), []).append((rect, i))
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
        for team, flag in enumerate(self.man['flags']):
            for x, y, z, yaw in self.man['spawn_points'][team]:
                self.assertLess(math.hypot(x-flag[0], z-flag[2]), 50)
                self.assertTrue(0 <= yaw < math.tau)

    def test_flag_stands_on_its_stand_open_to_the_sky(self):
        fx, fy, fz = self.anchors['flag']
        t, ny = self.soup.hits((fx, fy+1, fz), (0, -1, 0))
        self.assertAlmostEqual(fy+1-t[0], base.ROOF_TOP+1.1, places=3)
        self.assertEqual(self.soup.first((fx, fy+.1, fz), (0, 1, 0)), np.inf)

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
        self.assertEqual([list(f) for f in old['flags']], self.man['flags'])

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
        """No turret has a clear line from its barrel start to a player's
        chest anywhere in either hall (same rule as equipment::acquire_target)."""
        targets = []
        for b in json.loads((ROOT/'maps/raindance.json').read_text())['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for x in np.arange(-29, 29.1, 1.5):
                for z in np.arange(-25, 25.1, 1.5):
                    targets.append(m.point((x, base.FLOOR+base.LIFT+.8, z)))
        targets = np.array(targets)
        turrets = [e for e in self.man['entities'] if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        for e in turrets:
            p = np.array(e['position']); d = targets-p; dist = np.linalg.norm(d, axis=1)
            near = dist < 150
            if not near.any(): continue
            starts = p+d[near]/dist[near, None]*(e['radius']+.6)
            clear = ~self.world.blocked_many(starts, targets[near])
            self.assertEqual(int(clear.sum()), 0, (e['id'], targets[near][clear][:5]))

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

class SpawnForwardClearance(unittest.TestCase):
    """Every committed spawn faces open floor: a clear body-width view for
    6 m and the same floor for a half-second walk (docs/map-pipeline.md)."""
    def test_committed_spawns_face_open_floor(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/raindance'
        self.assertEqual(spawn_checks.problems(pack), [])


if __name__ == '__main__':
    unittest.main()
