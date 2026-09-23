#!/usr/bin/env python3
"""Test the Frostline builders without reading source-game assets.
Run from src/ with the numpy venv: scripts/test-frostline.py"""
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
from assets import frostline_beacon as beacon
from assets import frostline_flora as flora
from assets import frostline_materials as materials
from assets import frostline_station as st
from assets import frostline_terrain as terrain

HERE = Path(__file__).parent
def _load(name, file):
    loader = importlib.util.spec_from_file_location(name, HERE/file)
    module = importlib.util.module_from_spec(loader); loader.loader.exec_module(module)
    return module
kit = _load('kit', 'build-original-map.py')
build = _load('frostline_build', 'build-frostline.py')
LIFT = st.SPAWN_LIFT
OX, OZ = st.OUTPOST
EX, EZ = st.EMPLACEMENT
AX, AZ = st.APRON


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
        lo = np.minimum(starts, ends).min(0)-1; hi = np.maximum(starts, ends).max(0)+1
        m = np.all(self.hi >= lo, 1) & np.all(self.lo <= hi, 1)
        A, E1, E2 = self.a[m][None], self.e1[m][None], self.e2[m][None]
        for i in range(0, len(starts), 128):
            s = starts[i:i+128][:, None]; d = ends[i:i+128][:, None]-s
            h = np.cross(d, E2); det = (E1*h).sum(-1)
            ok = np.abs(det) > 1e-9; inv = np.where(ok, 1/np.where(ok, det, 1), 0)
            tv = s-A; u = (tv*h).sum(-1)*inv; q = np.cross(tv, E1)
            v = (d*q).sum(-1)*inv; t = (E2*q).sum(-1)*inv
            out[i:i+128] = (ok & (u >= 0) & (v >= 0) & (u+v <= 1) & (t > 1e-4) & (t < 1-1e-4)).any(1)
        return out


def one_base(team=0, circuit='one', origin=(0, 0, 0), yaw=0.0):
    mesh = kit.Mesh(); mesh.origin = origin; mesh.yaw = yaw
    anchors = st.build(mesh, team, circuit)
    return mesh, anchors


_WHOLE = {}
def whole_map():
    """(spec, mesh, soup, grid) for the complete map, built once."""
    if not _WHOLE:
        spec = build.spec(); mesh = kit.Mesh()
        for b in spec['bases']:
            mesh.origin = tuple(b['position']); mesh.yaw = math.radians(b['yaw'])
            st.build(mesh, b['team'], f"base-{b['team']}")
        mesh.origin = tuple(spec['beacon']['position']); mesh.yaw = 0
        beacon.build(mesh)
        grid = build.terrain_grid(spec)
        mesh.origin = (0, 0, 0)
        flora.build(mesh, build.trees(spec, grid))
        _WHOLE.update(spec=spec, mesh=mesh, soup=Soup(mesh.collision), grid=grid)
    return _WHOLE['spec'], _WHOLE['mesh'], _WHOLE['soup'], _WHOLE['grid']


class FrostlineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mesh, cls.anchors = one_base()
        cls.soup = Soup(cls.mesh.collision)

    # --- Floors, headroom, routes -----------------------------------------
    def test_room_floors_have_floor_and_headroom(self):
        samples = [(x, z, st.L1) for x, z in ((-8, -7), (0, -8), (10, -8), (10, 4), (-8, 0), (0, -12), (-9, -12), (9, -12))]
        samples += [(x, z, st.L1) for x, z in ((-8, 10), (8, 10))]
        samples += [(x, z, st.L2) for x, z in ((-5, -5), (5, -9), (0, 3), (-8, 10), (12, 0))]
        samples += [(OX+x, OZ+z, st.OG) for x, z in ((-5, -5), (5.5, 2), (0, 5), (-4, 2))]
        for x, z, y in samples:
            t, ny = self.soup.hits((x, y+3, z), (0, -1, 0))
            self.assertTrue(len(t) and abs(3-t[0]) < 1e-6 and ny[0] > .999, (x, z, y, 'floor'))
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, y, 'headroom'))

    def test_west_ramp_climbs_with_headroom_and_a_closed_underside(self):
        x = sum(st.RAMP_X)/2
        for z in np.arange(st.RAMP_TOP_Z+.5, st.RAMP_FOOT_Z, 1.0):
            y = st.ramp_surface(z)
            t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
            self.assertLess(abs(1-t[0]), .02, (z, 'ramp surface'))
            self.assertGreater(ny[0], .85, z)
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (z, 'ramp headroom'))
        for z in np.arange(st.RAMP_TOP_Z+.5, 2.5, 1.0):
            self.assertLess(self.soup.first((st.RAMP_X[1]+1, st.L1+.5, z), (-1, 0, 0)), 1.01, z)
        self.assertLess(math.degrees(math.atan((st.L2-st.L1)/(st.RAMP_FOOT_Z-st.RAMP_TOP_Z))), 27)

    def test_stairs_and_emplacement_ramp_are_continuous(self):
        routes = [((-3, 0, 3), st.STAIR_FOOT_Z+.3, st.PORCH_Z-.3, st.stair_surface),
                  ((sum(st.REAR_DOOR)/2,), st.SZ1+.3, st.REAR_STAIR_FOOT_Z-.3, st.rear_stair_surface)]
        top = st.EG+st.E_H
        e_surface = lambda z: top+(st.EG-top)*min(max((z-EZ-st.E_RAMP[0])/(st.E_RAMP[1]-st.E_RAMP[0]), 0), 1)
        routes.append(((EX,), EZ+st.E_RAMP[0]+.3, EZ+st.E_RAMP[1]-.3, e_surface))
        for xs, z0, z1, surface in routes:
            for x in xs:
                for z in np.arange(z0, z1, .8):
                    y = surface(z)
                    t, _ = self.soup.hits((x, y+1, z), (0, -1, 0))
                    self.assertLess(abs(1-t[0]), .03, (x, z))
                    self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'stair headroom'))
            self.assertLess(math.degrees(math.atan(abs(surface(z1)-surface(z0))/(z1-z0))), 27)

    def test_standing_support_is_the_floor_top_everywhere(self):
        """The engine's support ray starts 0.15 m above the feet: wherever a
        player can stand, its first hit must be the up-facing floor top."""
        regions = [(st.RAMP_X[1]+.4, st.IX-.4, st.IZ0+.4, st.IZ1-.4, st.L1),
                   (-st.IX+.4, st.IX-.4, st.IZ0+.4, st.IZ1-.4, st.L2),
                   (-st.SX+.9, st.SX-.9, st.SZ0+.9, st.SZ1-.9, st.ROOF),
                   (-st.PORCH_X+.3, st.PORCH_X-.3, st.PORCH_Z+.3, st.SZ0-.1, st.L1),
                   (AX-st.APRON_HALF[0]+.4, AX+st.APRON_HALF[0]-.4, AZ-st.APRON_HALF[1]+.4, AZ+st.APRON_HALF[1]-.4, st.APRON_TOP),
                   (OX-st.OH+.9, OX+st.OH-.9, OZ-st.OH+.9, OZ+st.OH-.9, st.OG),
                   (OX-st.O_PORCH_X+.3, OX+st.O_PORCH_X-.3, OZ+st.OH+.2, OZ+st.OH+st.O_PORCH_D-.2, st.OG),
                   (OX-st.OH+.8, OX+st.OH-.8, OZ-st.OH+.8, OZ+st.OH-.8, st.OG+st.O_ROOF),
                   (EX-4, EX+4, EZ-4, EZ+4, st.EG+st.E_H)]
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
        rx0, rx1 = st.REAR_DOOR
        for i in np.where((n[:, 1] > .3) & (n[:, 1] < .9999))[0]:
            x, y, z = cen[i]
            if st.RAMP_X[0] <= x <= st.RAMP_X[1] and st.RAMP_TOP_Z <= z <= st.RAMP_FOOT_Z: continue
            if abs(x) <= st.STAIR_X and st.STAIR_FOOT_Z <= z <= st.PORCH_Z: continue
            if rx0 <= x <= rx1 and st.SZ1 <= z <= st.REAR_STAIR_FOOT_Z: continue
            if abs(x-EX) <= 1.6 and EZ+st.E_RAMP[0] <= z <= EZ+st.E_RAMP[1]: continue
            if any(math.hypot(x-e[0], z-e[2]) < 3.3 for e in ents): continue
            self.fail(('accidental slope', round(float(n[i, 1]), 4), cen[i]))

    def test_no_overlapping_coplanar_floor_plates(self):
        class Recording(kit.Mesh):
            def __init__(self):
                super().__init__(); self.plates = []
            def box(self, p, size, mat='concrete', solid=True):
                if solid and size[1] <= 1.25: self.plates.append((p[1]+size[1]/2, p, size))
                super().box(p, size, mat, solid)
        mesh = Recording(); st.build(mesh, 0, 'one')
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
        self.assertEqual(sum(abs(p[0]-OX) < 10 and abs(p[2]-OZ) < 10 for p in points), 3, 'three outpost spawns')
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

    def test_flag_stands_on_its_plinth_on_the_command_deck(self):
        fx, fy, fz = self.anchors['flag']
        t, ny = self.soup.hits((fx, fy+1, fz), (0, -1, 0))
        self.assertAlmostEqual(fy+1-t[0], st.L2+st.PLINTH, places=4)
        self.assertAlmostEqual(fy, st.L2+st.PLINTH+.05, places=6)
        self.assertGreater(self.soup.first((fx, fy+.1, fz), (0, 1, 0)), 5.0, 'room over the flag')

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
        self.assertEqual(kinds, ['generator', 'inventory', 'inventory', 'inventory', 'repair', 'sensor',
                                 'turret', 'turret', 'turret'])
        self.assertEqual(sorted(e['weapon'] for e in b.entities if e['kind'] == 'turret'), ['bullet', 'bullet', 'plasma'])
        self.assertLess(len(b.collision)//9, budgets.COLLISION_TRIS_PER_BASE)
        self.assertEqual(len(anchors['deploy_slots']), 4)

    def test_two_instances_have_unique_ids_and_independent_circuits(self):
        mesh = kit.Mesh(); st.build(mesh, 0, 'red')
        mesh.origin = (2048, 0, 2048); mesh.yaw = math.pi; st.build(mesh, 1, 'blue')
        ids = [e['id'] for e in mesh.entities]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual({e['circuit'] for e in mesh.entities}, {'red', 'blue'})
        for team in (0, 1):
            self.assertEqual(sum(e['kind'] == 'generator' and e['team'] == team for e in mesh.entities), 1)

    def test_materials_are_deterministic_opaque_and_distinct(self):
        names = ['meadow', 'rock', 'soil', 'moss', 'concrete', 'panel', 'grate', 'trim', 'ember', 'glacier',
                 'light', 'bark', 'leaf']
        tex = {}
        for name in names:
            a = materials.texture(name, 7, kit.noise, kit.value_noise)
            self.assertEqual(a, materials.texture(name, 7, kit.noise, kit.value_noise), name)
            self.assertEqual(len(a), 256*256*4)
            self.assertTrue(all(a[i] == 255 for i in range(3, len(a), 4)), name)
            tex[name] = a
        for i, a in enumerate(names):
            for b in names[i+1:]: self.assertNotEqual(tex[a], tex[b], (a, b))
        from assets import cairnhold_materials, tower_complex_materials
        for other in (cairnhold_materials, tower_complex_materials):
            for name in ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark']:
                self.assertNotEqual(tex[name], other.texture(name, 7, kit.noise, kit.value_noise), (other, name))
        # Fresh snow is bright.
        snow = np.frombuffer(tex['meadow'], np.uint8).reshape(-1, 4)[:, :3]
        self.assertGreater(snow.mean(), 200)

    def test_sky_meets_the_fog_at_the_horizon(self):
        for face in range(6):
            a = materials.sky(face, 3, kit.noise, kit.value_noise)
            self.assertEqual(a, materials.sky(face, 3, kit.noise, kit.value_noise))
            img = np.frombuffer(a, np.uint8).reshape(256, 256, 4)
            if face < 4:
                self.assertLess(abs(img[127:129, :, :3].mean()-materials.FOG_GREY), 6, face)
                self.assertGreater(img[:8, :, :3].mean(), materials.FOG_GREY+15, face)

    # --- Sightlines ---------------------------------------------------------
    def test_turrets_cannot_see_into_rooms(self):
        """No turret on the map has a clear line from its barrel to a player's
        chest anywhere inside the station hall, generator room, command deck
        or the outpost behind its baffle. Allowed: the airlock vestibules
        between each door and its baffle."""
        spec, mesh, soup, _ = whole_map()
        rooms = [(st.RAMP_X[1]+.6, st.IX-.6, st.BAFFLE_Z[1]+.6, st.PARTITION_Z[0]-.6, st.L1),
                 (-st.IX+.6, st.IX-.6, st.PARTITION_Z[1]+.6, st.IZ1-.6, st.L1),
                 (-st.IX+.6, st.IX-.6, st.IZ0+.6, st.IZ1-.6, st.L2),
                 (OX-st.OH+st.OW+.6, OX+st.OH-st.OW-.6, OZ-st.OH+st.OW+.6, OZ+st.O_BAFFLE_Z[0]-.6, st.OG)]
        targets = []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for x0, x1, z0, z1, y in rooms:
                for x in np.arange(x0, x1+.01, 1.0):
                    for z in np.arange(z0, z1+.01, 1.0):
                        if y == st.L2 and st.RAMP_X[0] <= x <= st.RAMP_X[1] and st.OPENING_Z[0] <= z <= st.OPENING_Z[1]:
                            continue          # the ramp opening has no floor
                        if soup.first(m.point((x, y+.1, z)), (0, 1, 0), 2.3) < 2.3: continue   # inside a prop
                        targets.append(m.point((x, y+LIFT+.8, z)))
        targets = np.array(targets)
        turrets = [e for e in mesh.entities if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        for e in turrets:
            p = np.array(e['position']); d = targets-p; dist = np.linalg.norm(d, axis=1)
            near = dist < 150
            starts = p+d[near]/dist[near, None]*(e['radius']+.6)
            clear = ~soup.blocked_many(starts, targets[near])
            self.assertEqual(int(clear.sum()), 0, (e['id'], targets[near][clear][:5]))
        self.assertGreater(len(targets), 1500)

    # --- Terrain, sites, flora ----------------------------------------------
    def test_terrain_is_symmetric_steep_and_matches_targets(self):
        spec, _, _, grid = whole_map()
        again = build.terrain_grid(spec)
        self.assertTrue(np.array_equal(grid, again), 'terrain must be deterministic')
        self.assertLess(np.abs(grid[1:, 1:]-grid[1:, 1:][::-1, ::-1]).max(), 1e-6, 'exact 180 degree symmetry')
        slope = terrain.slope_degrees(grid)[12:-12, 12:-12]
        self.assertTrue(33 <= np.median(slope) <= 39, np.median(slope))
        self.assertLess((slope < 3).mean(), .02)
        self.assertGreater((slope > 45).mean(), .15)
        prof = lambda z: build.terrain_height(1024, z, grid)
        shelf, valley, ridge = prof(624), prof(1024-215), prof(1024)
        self.assertGreater(shelf-valley, 40); self.assertGreater(ridge-valley, 60)
        flags = []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            flags.append(m.point((st.FLAG[0], 0, st.FLAG[1])))
        self.assertTrue(800 <= math.dist(flags[0][::2], flags[1][::2]) <= 830)

    def test_ground_is_level_under_every_structure(self):
        spec, _, _, grid = whole_map()
        h = lambda x, z: build.terrain_height(x, z, grid)
        for b in spec['bases']:
            oy = b['position'][1]
            checks = [(x, z, st.G) for x in np.arange(-st.SX, st.SX+.1, 2) for z in np.arange(st.STAIR_FOOT_Z-4, st.REAR_STAIR_FOOT_Z+4, 2)]
            checks += [(x, z, st.G) for x in np.arange(AX-st.APRON_HALF[0], AX+st.APRON_HALF[0]+.1, 2)
                       for z in np.arange(AZ-st.APRON_HALF[1], AZ+st.APRON_HALF[1]+.1, 2)]
            checks += [(x, z, st.OG) for x in np.arange(OX-st.OH, OX+st.OH+.1, 2) for z in np.arange(OZ-st.OH, OZ+st.OH+st.O_PORCH_D+.1, 1.5)]
            checks += [(EX+r*math.cos(a), EZ+r*math.sin(a), st.EG) for r in np.arange(0, st.E_R0+1, 1.5)
                       for a in np.linspace(0, math.tau, 16, endpoint=False)]
            checks += [(EX+dx, z, st.EG) for dx in (-1.5, 0, 1.5) for z in np.arange(EZ+st.E_RAMP[0], EZ+st.E_RAMP[1]+2, 1)]
            for x, z, level in checks:
                self.assertAlmostEqual(h(*build.to_world(b, x, z))-oy, level-.06, delta=1/32, msg=(b['team'], x, z))
        bx, by, bz = spec['beacon']['position']
        for r in np.arange(0, beacon.BREAK_R+2, 1.0):
            for a in np.linspace(0, math.tau, 24, endpoint=False):
                self.assertAlmostEqual(h(bx+r*math.cos(a), bz+r*math.sin(a)), by-.06, delta=1/32)

    def test_pines_are_symmetric_clear_and_on_holdable_slopes(self):
        spec, _, _, grid = whole_map()
        pines = build.trees(spec, grid)
        self.assertTrue(100 <= len(pines) <= 240, len(pines))
        slope = flora.slope_sampler(grid)
        keep = build.tree_exclusions(spec)
        for (x, y, z, hh), (x2, y2, z2, hh2) in zip(pines[::2], pines[1::2]):
            self.assertAlmostEqual(x+x2, 2048, places=6); self.assertAlmostEqual(z+z2, 2048, places=6)
            self.assertEqual(hh, hh2)
            for px, pz in ((x, z), (x2, z2)):
                self.assertGreaterEqual(abs(px-1024), flora.LANE_HALF)
                self.assertLessEqual(slope(px, pz), flora.MAX_SLOPE)
                for ex, ez, r in keep: self.assertGreaterEqual(math.hypot(px-ex, pz-ez), r)

    def test_beacon_is_symmetric_with_a_standable_perch(self):
        mesh = kit.Mesh(); anchors = beacon.build(mesh)
        soup = Soup(mesh.collision)
        c = np.array(mesh.collision).reshape(-1, 3)
        self.assertTrue(np.allclose(np.sort(c[:, 0]), np.sort(-c[:, 0]), atol=1e-4))
        self.assertTrue(np.allclose(np.sort(c[:, 2]), np.sort(-c[:, 2]), atol=1e-4))
        for x, z in ((0, 0), (2.5, 0), (-2.5, 2.5)):
            if abs(x) > beacon.leg_offset(beacon.PERCH_Y)-.5 or abs(z) > beacon.leg_offset(beacon.PERCH_Y)-.5 or (x, z) == (0, 0):
                t, ny = soup.hits((x, beacon.PERCH_Y+2, z), (0, -1, 0))
                self.assertAlmostEqual(2-t[0], 0, places=4); self.assertGreater(ny[0], .999)
        self.assertLess(len(mesh.collision)//9, 400)
        self.assertEqual(len(anchors['windbreaks']), 4)

    # --- Pack ----------------------------------------------------------------
    def test_lightmap_bake_is_deterministic(self):
        from assets import lightmap_bake
        m = kit.Mesh(); m.lamps = []; beacon.build(m)
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
        pack = Path(__file__).resolve().parent.parent/'assets/maps/frostline'
        self.assertEqual(spawn_checks.problems(pack), [])


if __name__ == '__main__':
    unittest.main()
