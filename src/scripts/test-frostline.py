#!/usr/bin/env python3
"""Test the Frostline builders without reading source-game assets.
Run from src/ with the numpy venv: scripts/test-frostline.py"""
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
from assets import cnh_tower
from assets import frostline_beacon as beacon
from assets import frostline_cavern as cavern
from assets import frostline_flora as flora
from assets import frostline_materials as materials
from assets import frostline_station as st
from assets import frostline_terrain as terrain
from assets import sightline_checks
from assets import turret_arcs

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
        flags = [None, None]
        for b in spec['bases']:
            mesh.origin = tuple(b['position']); mesh.yaw = math.radians(b['yaw'])
            anchors = st.build(mesh, b['team'], f"base-{b['team']}")
            flags[b['team']] = mesh.point(anchors['flag'])
        turret_arcs.assign(mesh.entities, flags)
        mesh.origin = tuple(spec['beacon']['position']); mesh.yaw = 0
        beacon.build(mesh)
        grid = build.terrain_grid(spec)
        cavern.build(mesh, grid)
        mesh.collision.extend(cavern.lid(grid)[1])
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
        samples += [(x, z, st.L2) for x, z in ((-6, -5), (5, -9), (-7, 3), (-8, 10), (12, 0), (3, 10.5), (0, -6))]
        samples += [(OX+x, OZ+z, st.OG) for x, z in ((-5, -5), (5.5, 2), (0, 5), (-4, 2))]
        samples += [(x, z, st.B_FLOOR) for x, z in ((-2, -5), (3, -5), (-3, 6), (6.4, -4), (6.4, 5))]
        samples += [(x, -4, st.B_FLOOR) for x in (9, 12, 15)]
        samples += [(x, z, st.G) for x, z in ((28, -5), (30, -1), (24.5, -2))]
        for x, z, y in samples:
            t, ny = self.soup.hits((x, y+3, z), (0, -1, 0))
            self.assertTrue(len(t) and abs(3-t[0]) < 1e-6 and ny[0] > .999, (x, z, y, 'floor'))
            self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, y, 'headroom'))

    def test_both_ramps_climb_with_headroom_and_a_closed_underside(self):
        for (x0, x1), side in ((st.RAMP_X, 1), (st.E_RAMP_X, -1)):
            x = (x0+x1)/2
            for z in np.arange(st.RAMP_TOP_Z+.5, st.RAMP_FOOT_Z, 1.0):
                y = st.ramp_surface(z)
                t, ny = self.soup.hits((x, y+1, z), (0, -1, 0))
                self.assertLess(abs(1-t[0]), .02, (x, z, 'ramp surface'))
                self.assertGreater(ny[0], .85, z)
                self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'ramp headroom'))
            inner = x1 if side > 0 else x0
            for z in np.arange(st.RAMP_TOP_Z+.5, 2.5, 1.0):
                self.assertLess(self.soup.first((inner+side, st.L1+.5, z), (-side, 0, 0)), 1.01, (x, z))
        self.assertLess(math.degrees(math.atan((st.L2-st.L1)/(st.RAMP_FOOT_Z-st.RAMP_TOP_Z))), 27)

    def test_stairs_and_emplacement_ramp_are_continuous(self):
        routes = [((-3, 0, 3), st.STAIR_FOOT_Z+.3, st.PORCH_Z-.3, st.stair_surface),
                  ((sum(st.REAR_DOOR)/2,), st.SZ1+.3, st.REAR_STAIR_FOOT_Z-.3, st.rear_stair_surface)]
        top = st.EG+st.E_H
        e_surface = lambda z: top+(st.EG-top)*min(max((z-EZ-st.E_RAMP[0])/(st.E_RAMP[1]-st.E_RAMP[0]), 0), 1)
        routes.append(((EX,), EZ+st.E_RAMP[0]+.3, EZ+st.E_RAMP[1]-.3, e_surface))
        routes.append(((sum(st.B_STAIR[:2])/2,), st.B_STAIR[2]+.3, st.B_STAIR[3]-.3, st.basement_stair_surface))
        # The basement stair descends to the 6.5 m generator room at about
        # 28 degrees; the engine only counts ground as steep past 35 degrees
        # (normal.y < 0.82), and Cairnhold's vault stair is 29.3 degrees.
        # The shed's exit stair climbs from that same lowered floor (29.4).
        limits = {st.basement_stair_surface: 30, st.exit_stair_surface: 30}
        for xs, z0, z1, surface in routes:
            for x in xs:
                for z in np.arange(z0, z1, .8):
                    y = surface(z)
                    t, _ = self.soup.hits((x, y+1, z), (0, -1, 0))
                    self.assertLess(abs(1-t[0]), .03, (x, z))
                    self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'stair headroom'))
            self.assertLess(math.degrees(math.atan(abs(surface(z1)-surface(z0))/(z1-z0))), limits.get(surface, 27))
        # Stairs that climb along x: the east door's and the shed's sunken one.
        for z, x0, x1, surface in ((sum(st.EAST_DOOR)/2, st.SX+.3, st.EAST_STAIR_FOOT_X-.3, st.east_stair_surface),
                                   (sum(st.T_IN)/2, st.X_STAIR[0]+.3, st.X_STAIR[1]-.3, st.exit_stair_surface)):
            for x in np.arange(x0, x1, .8):
                y = surface(x)
                t, _ = self.soup.hits((x, y+1, z), (0, -1, 0))
                self.assertLess(abs(1-t[0]), .03, (x, z))
                self.assertGreater(self.soup.first((x, y+.2, z), (0, 1, 0)), 2.4, (x, z, 'stair headroom'))
            self.assertLess(math.degrees(math.atan(abs(surface(x1)-surface(x0))/(x1-x0))), limits.get(surface, 27))

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
                   (EX-4, EX+4, EZ-4, EZ+4, st.EG+st.E_H),
                   (st.B_IN[0]+.4, st.B_IN[1]-.4, st.B_IN[2]+.4, st.B_IN[3]-.4, st.B_FLOOR),
                   (st.T_CELLS[0]+.4, st.X_STAIR[0]-.4, st.T_IN[0]+.4, st.T_IN[1]-.4, st.B_FLOOR),
                   (st.X_STAIR[1]+.4, st.ANNEX[1]-1.0, st.ANNEX[2]+1.0, st.ANNEX[3]-1.0, st.G)]
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
            if st.E_RAMP_X[0] <= x <= st.E_RAMP_X[1] and st.RAMP_TOP_Z <= z <= st.RAMP_FOOT_Z: continue
            if st.B_STAIR[0] <= x <= st.B_STAIR[1] and st.B_STAIR[2] <= z <= st.B_STAIR[3]: continue
            if st.SX <= x <= st.EAST_STAIR_FOOT_X and st.EAST_DOOR[0] <= z <= st.EAST_DOOR[1]: continue
            if st.X_STAIR[0] <= x <= st.X_STAIR[1] and st.T_IN[0] <= z <= st.T_IN[1]: continue
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
        """No turret engages a player (field of fire, range, clear line from
        its barrel to the chest, as the server does) anywhere inside the
        station hall, rear hall, command deck, the basement generator room,
        its tunnel, or the outpost behind its baffle, beyond the doorway depth
        (sightline_checks.DOOR_DEPTH) of an open door or window."""
        spec, mesh, soup, _ = whole_map()
        rooms = [(st.RAMP_X[1]+.6, st.E_RAMP_X[0]-.6, st.IZ0+.6, st.PARTITION_Z[0]-.6, st.L1),
                 (-st.IX+.6, st.IX-.6, st.PARTITION_Z[1]+.6, st.IZ1-.6, st.L1),
                 (-st.IX+.6, st.IX-.6, st.IZ0+.6, st.IZ1-.6, st.L2),
                 (st.B_IN[0]+.6, st.B_BAFFLE[0]-.6, st.B_IN[2]+.6, st.B_IN[3]-.6, st.B_FLOOR),
                 (st.B_BAFFLE[0], st.B_IN[1]-.6, st.B_BAFFLE[3]+.6, st.B_IN[3]-.6, st.B_FLOOR),
                 (st.T_CELLS[0]+.6, st.X_STAIR[0]-.6, st.T_IN[0]+.6, st.T_IN[1]-.6, st.B_FLOOR),
                 (OX-st.OH+st.OW+.6, OX+st.OH-st.OW-.6, OZ-st.OH+st.OW+.6, OZ+st.O_BAFFLE_Z[0]-.6, st.OG)]
        # Inner faces of every open door and window: (x0, z0, x1, z1).
        local_openings = [(*st.DOOR, st.IZ0), (*st.REAR_DOOR, st.IZ1)]
        local_openings = [(x0, z, x1, z) for x0, x1, z in local_openings]
        local_openings += [(x0, z, x1, z) for x0, x1 in st.FRONT_WINDOWS for z in (st.IZ0, st.IZ1)]
        local_openings += [(x, z0, x, z1) for z0, z1 in st.SIDE_WINDOWS for x in (-st.IX, st.IX)]
        local_openings += [(st.IX, st.EAST_DOOR[0], st.IX, st.EAST_DOOR[1])]
        def no_floor(x, y, z):
            if y == st.L2:
                return any(x0 <= x <= x1 and st.OPENING_Z[0] <= z <= st.OPENING_Z[1] for x0, x1 in (st.RAMP_X, st.E_RAMP_X))
            if y == st.L1:
                bx0, bx1, bz0, bz1 = st.B_OPENING
                return bx0-.3 <= x <= bx1+.3 and bz0 <= z <= bz1+.3
            return False
        targets, openings = [], []
        for b in spec['bases']:
            m = kit.Mesh(); m.origin = tuple(b['position']); m.yaw = math.radians(b['yaw'])
            for x0, x1, z0, z1, y in rooms:
                for x in np.arange(x0, x1+.01, 1.0):
                    for z in np.arange(z0, z1+.01, 1.0):
                        if no_floor(x, y, z): continue   # ramp and stair openings have no floor
                        if soup.first(m.point((x, y+.1, z)), (0, 1, 0), 2.3) < 2.3: continue   # inside a prop
                        targets.append(m.point((x, y+LIFT+.8, z)))
            pt = lambda x, z: tuple(m.point((x, 0, z))[i] for i in (0, 2))
            openings += [(pt(x0, z0), pt(x1, z1)) for x0, z0, x1, z1 in local_openings]
        targets = np.array(targets)
        allowed = sightline_checks.near_openings(targets, openings)
        turrets = [e for e in mesh.entities if e['kind'] == 'turret']
        self.assertEqual(len(turrets), 6)
        for e in turrets:
            seen = sightline_checks.visible(soup, e, targets) & ~allowed
            self.assertEqual(int(seen.sum()), 0, (e['id'], targets[seen][:5]))
        self.assertGreater(len(targets), 1500)

    def test_generators_are_underground_in_covered_cut_cells(self):
        spec, mesh, soup, grid = whole_map()
        cells = set(build.holes(spec))
        cave = set(cavern.holes())
        self.assertEqual(len(cells), 2*sum((x1-x0)*(z1-z0)/64 for x0, x1, z0, z1 in st.HOLES.values())+len(cave))
        gens = [e for e in mesh.entities if e['kind'] == 'generator']
        self.assertEqual(len(gens), 2)
        for e, b in zip(gens, spec['bases']):
            x, y, z = e['position']
            self.assertIn(int(z//8)*256+int(x//8), cells, e['id'])
            self.assertLess(y, b['position'][1]+st.G, 'generator is below the shelf')
        # Every base cut cell is roofed at or above ground: no exposed hole or
        # lid. (The cavern's cells have their own test below.)
        for c in cells-cave:
            ix, iz = c % 256, c//256
            for fx in (.1, .5, .9):
                for fz in (.1, .5, .9):
                    x, z = (ix+fx)*8, (iz+fz)*8
                    ground = build.terrain_height(x, z, grid)
                    t = soup.first((x, ground+60, z), (0, -1, 0))
                    self.assertGreater(ground+60-t, ground-.1, (c, x, z, 'exposed cut'))

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
            checks += [(x, z, st.G) for x in np.arange(st.SX, st.ANNEX[1]+2.1, 2) for z in np.arange(st.ANNEX[2]-2, st.EAST_DOOR[1]+3, 2)]
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
        # The perch is a ring round the Capture & Hold pylon: standable
        # between the hole and the rail on every side, pylon in the middle.
        for x, z in ((3.1, 0), (0, -3.1), (-3.1, 0), (0, 3.1), (2.2, -3.2)):
            t, ny = soup.hits((x, beacon.PERCH_Y+2, z), (0, -1, 0))
            self.assertAlmostEqual(2-t[0], 0, places=4); self.assertGreater(ny[0], .999)
        self.assertLess(soup.first((beacon.PERCH_HOLE-.05, beacon.PERCH_Y+.5, 0), (-1, 0, 0)), .6, 'the pylon fills the perch hole')
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

class CavernTests(unittest.TestCase):
    """The ice cavern through the beacon ridge (assets/frostline_cavern.py)."""
    LANES = (1021.0, 1024.0, 1027.0)      # clear of both ice boulders

    def test_roof_copies_the_ridge_and_every_cut_cell_has_support(self):
        spec, mesh, soup, grid = whole_map()
        q = cavern.quantize(grid)
        for c in cavern.holes():
            ix, iz = c % 256, c//256
            for fx in (.02, .5, .98):
                for fz in (.02, .5, .98):
                    x, z = (ix+fx)*8, (iz+fz)*8
                    if math.hypot(x-cavern.CX, z-cavern.CX) < 15: continue   # beacon pad and legs
                    ground = cavern.surface(q, x, z)
                    top = ground+80-soup.first((x, ground+80, z), (0, -1, 0))
                    if cavern.roofed(ix, iz):
                        self.assertAlmostEqual(top, ground, delta=.02, msg=('roof is not the ridge', x, z))
                    else:
                        self.assertTrue(210 < top < ground+.02, ('trench floor', x, z, top, ground))

    def test_roof_renders_as_terrain(self):
        spec, _, _, grid = whole_map()
        render, collision = cavern.lid(grid)
        v = np.array(render).reshape(-1, 12)
        roofed = [c for c in cavern.cells() if cavern.roofed(*c)]
        self.assertEqual(len(v), 6*len(roofed)); self.assertEqual(len(collision), 3*len(v))
        self.assertTrue(np.all(v[:, 11] == -2) and np.all(v[:, 10] == 0), 'terrain shading path')
        self.assertTrue(np.allclose(v[:, 6], v[:, 0]/8) and np.allclose(v[:, 7], v[:, 2]/8))
        self.assertTrue(np.allclose(v[:, 8], (v[:, 0]/8+.5)/256) and np.allclose(v[:, 9], (v[:, 2]/8+.5)/256))
        self.assertTrue(np.allclose(np.linalg.norm(v[:, 3:6], axis=1), 1))
        self.assertTrue(np.all(v[:, 0] % 8 == 0) and np.all(v[:, 2] % 8 == 0))
        q = cavern.quantize(grid)
        self.assertTrue(np.array_equal(v[:, 1], q[(v[:, 2]/8).astype(int), (v[:, 0]/8).astype(int)]))

    def test_lanes_are_walkable_from_trench_end_to_trench_end(self):
        """Body-band sweep along three lanes over the whole passage. Outside
        the trench ends the approach is Frostline's usual 35-40 degree snow,
        skied or jetted like everywhere else, so the walk starts at the ends."""
        from assets import route_checks
        pack = route_checks.Pack(Path(__file__).resolve().parent.parent/'assets/maps/frostline')
        for x in self.LANES:
            pts = []
            for z in np.arange(944.0, 1104.1, 2.0):
                ys = route_checks.floors(pack, x, z)
                self.assertTrue(ys, (x, z))
                expect = cavern.floor_at(z-cavern.CX)
                pts.append((x, min(ys, key=lambda y: abs(y-expect)), z))
            self.assertLess(max(abs(b[1]-a[1]) for a, b in zip(pts, pts[1:])), .75, 'no step taller than a walk')
            self.assertTrue(route_checks.path_clear(pack, pts), x)

    def test_mouths_are_open_across_their_width(self):
        """Through each portal, the whole opening (wall to wall, floor to
        vault) is clear from the trench into the cavern; and at 3.5 m, above
        the boulders, the cavern is clear end to end across its width."""
        _, _, soup, _ = whole_map()
        xs = cavern._vault_xs(); ys = [cavern.vault(x) for x in xs]   # the built polygon
        for sign in (-1, 1):
            portal = cavern.CX+sign*cavern.PORTAL_D
            for dx in np.arange(-cavern.HALF_W+.6, cavern.HALF_W-.59, 1.0):
                for h in np.arange(.5, np.interp(dx, xs, ys)-.4, 1.0):
                    o = (cavern.CX+dx, cavern.FLOOR_MOUTH+h, portal+sign*2.0)   # the trench floor is flat here
                    self.assertEqual(soup.first(o, (0, 0, -sign), 4.0), np.inf, (sign, dx, h))
        for x in np.arange(cavern.CX-cavern.HALF_W+1, cavern.CX+cavern.HALF_W-.9, 1.0):
            y = cavern.FLOOR_MOUTH+3.5
            self.assertEqual(soup.first((x, y, 950.0), (0, 0, 1), 1098.0-950.0), np.inf, x)

    def test_no_turret_or_sensor_can_see_inside_and_no_one_spawns_there(self):
        spec, mesh, soup, _ = whole_map()
        targets = np.array([(x, cavern.floor_at(z-cavern.CX)+LIFT+.8, z)
                            for x in np.arange(cavern.CX-11, cavern.CX+11.1, 2.0)
                            for z in np.arange(cavern.CX-47, cavern.CX+47.1, 2.0)])
        for e in mesh.entities:
            if e['kind'] not in ('turret', 'sensor'): continue
            p = np.array(e['position']); d = targets-p; dist = np.linalg.norm(d, axis=1)
            near = dist < 260
            if not near.any(): continue
            starts = p+d[near]/dist[near, None]*(e['radius']+.6)
            self.assertFalse((~soup.blocked_many(starts, targets[near])).any(), e['id'])
        import json
        manifest = json.loads((Path(__file__).resolve().parent.parent/'assets/maps/frostline/map.json').read_text())
        for team in manifest['spawn_points']:
            for x, y, z, _ in team:
                self.assertFalse(1000 <= x <= 1048 and 936 <= z <= 1112, (x, z))

    def test_geometry_has_no_degenerate_triangles_and_fits_its_budget(self):
        spec, _, _, grid = whole_map()
        m = kit.Mesh(); m.lamps = []; cavern.build(m, grid)
        t = np.array(m.vertices, np.float64).reshape(-1, 3, 12)[:, :, :3]
        area = np.linalg.norm(np.cross(t[:, 1]-t[:, 0], t[:, 2]-t[:, 0]), axis=1)/2
        self.assertGreater(area.min(), 1e-4)
        solid = len(m.collision)//9+len(cavern.lid(grid)[1])//9
        self.assertLess(solid, 600, solid)
        self.assertGreaterEqual(len(m.lamps), 16)


class RouteCounts(unittest.TestCase):
    """Walking routes on the committed pack (assets/route_checks.py): the
    station is not a one-door camp. At least three independent ways into its
    floors, two onto the flag deck, and exactly two into the generator room."""
    def test_station_routes(self):
        from assets import route_checks
        pack = route_checks.Pack(Path(__file__).resolve().parent.parent/'assets/maps/frostline')
        for base in build.spec()['bases']:
            ox, oy, oz = base['position']
            def box(lx0, lx1, ly0, ly1, lz0, lz1):
                (ax, az), (bx, bz) = build.to_world(base, lx0, lz0), build.to_world(base, lx1, lz1)
                return (min(ax, bx), max(ax, bx), oy+ly0, oy+ly1, min(az, bz), max(az, bz))
            found = route_checks.base_entries(pack, (ox, oz), {
                'station': box(-st.SX, st.SX, st.L1-1, st.ROOF-1, st.SZ0, st.SZ1),
                'deck': box(-st.SX, st.SX, st.L2-1, st.L2+1.5, st.SZ0, st.SZ1),
                'generator': box(*st.B_IN[:2], st.B_FLOOR-.5, st.B_FLOOR+1.5, *st.B_IN[2:])})
            self.assertGreaterEqual(len(found['station']), 3, (base['team'], found['station']))
            self.assertGreaterEqual(len(found['deck']), 2, (base['team'], found['deck']))
            self.assertEqual(len(found['generator']), 2, (base['team'], found['generator']))


class FlagRoutes(unittest.TestCase):
    """docs/map-pipeline.md: "two main entrances but ~10 ways of getting to
    the flag". route_checks.flag_routes counts (entry, approach) pairs: every
    way from the field into the station (walking, drops and jet hops,
    including the command deck's open windows) times every way from that
    entry onto the flag's stretch of the deck (either ramp, or a jet up
    through the two-level void)."""
    def test_flag_has_at_least_ten_routes(self):
        from assets import route_checks
        pack = route_checks.Pack(Path(__file__).resolve().parent.parent/'assets/maps/frostline')
        fx, fz = st.FLAG
        for base in build.spec()['bases']:
            ox, oy, oz = base['position']
            def box(lx0, lx1, ly0, ly1, lz0, lz1):
                (ax, az), (bx, bz) = build.to_world(base, lx0, lz0), build.to_world(base, lx1, lz1)
                return (min(ax, bx), max(ax, bx), oy+ly0, oy+ly1, min(az, bz), max(az, bz))
            r = route_checks.flag_routes(pack, (ox, oz), box(-st.SX, st.SX, st.L1-1, st.ROOF-1, st.SZ0, st.SZ1),
                                         box(fx-5, fx+5, st.L2-.5, st.L2+.8, fz-4, st.IZ1), airborne=True)
            self.assertGreaterEqual(len(r['entries']), 6, (base['team'], r['entries']))
            self.assertGreaterEqual(r['routes'], 10, (base['team'], [len(a) for a in r['approaches']]))


class ControlPoints(unittest.TestCase):
    """Capture & Hold (docs/capture-and-hold.md): the beacon is the centre
    point, active in CTF with a drain field; the West and East Cols are
    Capture & Hold only and mirror each other through the map centre."""
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parent.parent/'assets/maps/frostline'
        cls.manifest = json.loads((root/'map.json').read_text())
        cls.heights = np.frombuffer((root/'height.bin').read_bytes(), '<u2').reshape(256, 256)/32

    def test_centre_is_ctf_active_with_a_drain_and_the_cols_mirror(self):
        pts = self.manifest['control_points']
        self.assertEqual([p['id'] for p in pts], ['beacon', 'west-col', 'east-col'])
        centre, west, east = pts
        self.assertEqual(centre['pos'], build.spec()['beacon']['position'])
        self.assertTrue(centre['ctf_active'])
        self.assertEqual(centre['drain'], {'radius': 60, 'rate': 10})
        for p in (west, east):
            self.assertFalse(p['ctf_active']); self.assertNotIn('drain', p)
        self.assertEqual((west['pos'][0]+east['pos'][0], west['pos'][2]+east['pos'][2]), (2048, 2048))
        self.assertEqual(west['pos'][1], east['pos'][1])
        # The drain covers both cavern portals (48 m) but not the trench ends.
        self.assertGreater(centre['drain']['radius'], cavern.PORTAL_D)
        self.assertLess(centre['drain']['radius'], cavern.PORTAL_D+cavern.TRENCH_L)
        for p in pts: self.assertEqual(p['radius'], 12.0)

    def test_col_rings_are_level_ground_clear_of_spawns(self):
        for p in self.manifest['control_points'][1:]:
            x, y, z = p['pos']
            for dx in range(-12, 13, 4):
                for dz in range(-12, 13, 4):
                    if dx*dx+dz*dz > 144: continue
                    h = self.heights[int((z+dz)//8), int((x+dx)//8)]
                    self.assertLess(abs(h-y), cnh_tower.MAX_TILT, (p['id'], dx, dz, h))
        for p in self.manifest['control_points']:
            for team in self.manifest['spawn_points']:
                for s in team:
                    self.assertGreater(math.hypot(s[0]-p['pos'][0], s[2]-p['pos'][2]), p['radius']+2, (p['id'], s))


class Surfaces(unittest.TestCase):
    """No visible coplanar faces of different materials (they flicker). The
    only pairs left belong to the shared kit turret mount (its team band sits
    flush on its collar), which build-original-map.py owns."""
    def test_no_z_fighting_outside_the_kit_turret_mounts(self):
        from assets import surface_checks
        root = Path(__file__).resolve().parent.parent/'assets/maps/frostline'
        v = np.fromfile(root/'vertices.bin', '<f4'); c = np.fromfile(root/'collision.bin', '<f4')
        turrets = [e['position'] for e in json.loads((root/'map.json').read_text())['entities'] if e['kind'] == 'turret']
        left = [e for e in surface_checks.z_fighting(v, c)
                if not any(math.dist(e[1], t) < 3.0 for t in turrets)]
        self.assertEqual(left, [])


class SpawnForwardClearance(unittest.TestCase):
    """Every committed spawn faces open floor: a clear body-width view for
    6 m and the same floor for a half-second walk (docs/map-pipeline.md)."""
    def test_committed_spawns_face_open_floor(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/frostline'
        self.assertEqual(spawn_checks.problems(pack), [])

    def test_no_indoor_spawn_shows_through_an_opening_from_the_field(self):
        from assets import spawn_checks
        pack = Path(__file__).resolve().parent.parent/'assets/maps/frostline'
        self.assertEqual(spawn_checks.exposed(pack), [])


if __name__ == '__main__':
    unittest.main()
