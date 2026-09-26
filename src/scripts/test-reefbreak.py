#!/usr/bin/env python3
"""Tests for the original Reefbreak map (docs/reefbreak.md,
docs/map-pipeline.md step 5). Run from src/ with the numpy venv.

Pack-level checks read REEFBREAK_PACK (a built pack directory) when set,
else the embedded assets/maps/reefbreak."""
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import unittest

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from assets import reefbreak_base as base
from assets import reefbreak_terrain as terrain


def _load(name, file):
    loader = importlib.util.spec_from_file_location(name, HERE/file)
    module = importlib.util.module_from_spec(loader); loader.loader.exec_module(module)
    return module
build = _load('reefbreak_build', 'build-reefbreak.py')
test_oz = _load('test_oz', 'test-ozarktic-blast.py')
Soup = test_oz.Soup
PACK = Path(os.environ.get('REEFBREAK_PACK') or HERE.parent/'assets/maps/reefbreak')


class Terrain(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = build.spec()
        cls.grid = build.terrain_grid(cls.definition)

    def test_point_symmetric(self):
        h = terrain.natural(np.arange(0, 2048, 8.0)[None, :].repeat(256, 0),
                            np.arange(0, 2048, 8.0)[:, None].repeat(256, 1), self.definition['seed'])
        self.assertLess(np.abs(h[1:, 1:]-h[1:, 1:][::-1, ::-1]).max(), 1e-6)

    def test_shallows_are_wadeable_and_there_is_land(self):
        z, x = np.mgrid[0:256, 0:256]*8.0
        r = np.hypot(x-1024, z-1024); play = r < 650
        wet = self.grid < terrain.SEA
        shallow = wet & (self.grid > terrain.SEA-.6)
        self.assertGreater(shallow[play].mean(), .4, 'most water in play is ankle deep')
        self.assertGreater((~wet)[play].mean(), .2, 'enough dry land')
        self.assertTrue((self.grid < terrain.SEA-5)[play].any(), 'deep blue holes exist')

    def test_lagoon_channels_are_off_the_flag_axis(self):
        for c in terrain.CHANNELS:
            self.assertGreater(min(abs(c), abs(180-c)), 30)


class Pack(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.m = json.loads((PACK/'map.json').read_text())
        cls.soup = Soup(np.frombuffer((PACK/'collision.bin').read_bytes(), '<f4'))

    def test_flags_are_on_the_reef_600_m_apart(self):
        a, b = (np.array(f) for f in self.m['flags'])
        self.assertAlmostEqual(np.linalg.norm(a-b), 600.0, delta=2.0)
        for f in (a, b):
            self.assertGreater(f[1]-terrain.SEA, 5.0, 'flag pads stand clear of the sea')

    def test_the_ships_float_with_room_to_ski_beneath(self):
        for b in build.spec()['bases']:
            keel = b['position'][1]+base.KEEL-2.2
            self.assertGreater(keel-terrain.SEA, 8.0)
            # Straight down from the middle of the hull: the keel, then open air to the sea.
            o = np.array(b['position'])+np.array([4.0, -4.0, 0.0])
            t = self.soup.first(o, np.array([0.0, -1.0, 0.0]), 40.0)
            self.assertTrue(math.isinf(t) or o[1]-t < terrain.SEA, 'nothing solid under the ship above the sea')

    def test_every_spawn_stands_over_a_floor(self):
        for team in self.m['spawn_points']:
            self.assertEqual(len(team), len(base.SPAWNS)+len(base.OUT_SPAWNS))
            for x, y, z, _ in team:
                t = self.soup.first(np.array([x, y, z]), np.array([0.0, -1.0, 0.0]), 3.0)
                self.assertAlmostEqual(t, base.SPAWN_LIFT, delta=.05, msg=f'spawn at {x:.1f},{y:.1f},{z:.1f}')

    def test_generator_room_has_exactly_two_ways_in(self):
        # The bulkhead door and the stern hatch; nothing else opens the engine room.
        s0, s1 = base.BULKHEAD
        for name in ('PORT_HATCH', 'STAR_HATCH', 'BREACH'):
            z0, z1 = getattr(base, name)[:2]
            self.assertLess(z1, s0, f'{name} opens into the hold')
        self.assertLess(base.DECK_HATCH[3], s0, 'the deck hatch opens into the hold')

    def test_each_team_has_its_stations_and_turrets(self):
        kinds = {}
        for e in self.m['entities']:
            kinds.setdefault((e['team'], e['kind']), 0); kinds[(e['team'], e['kind'])] += 1
        for team in (0, 1):
            self.assertEqual(kinds[(team, 'inventory')], 5, 'three in the ship, two in the outpost')
            self.assertEqual(kinds[(team, 'turret')], 2)
            self.assertEqual(kinds[(team, 'generator')], 1)
            self.assertEqual(kinds[(team, 'sensor')], 1)

    def test_one_sea_volume_covers_the_map(self):
        v = self.m['water_volumes']
        self.assertEqual(len(v), 1)
        self.assertEqual(v[0]['rect'], [0.0, 0.0, 2048.0, 2048.0])
        self.assertEqual(v[0]['surface'], terrain.SEA)

    def test_gangway_climbs_gently(self):
        gx0, gx1, gz0, gz1 = base.GANGWAY
        slope = math.degrees(math.atan2(base.DECK_Y-base.PAD_Y, abs(gz1-gz0)))
        self.assertLess(slope, 20.0)


if __name__ == '__main__':
    unittest.main()
