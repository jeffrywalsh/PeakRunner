#!/usr/bin/env python3
"""Synthetic measurement tests: no proprietary pack required."""
import importlib.util
from pathlib import Path
import unittest
import numpy as np

def load(name,file):
    s=importlib.util.spec_from_file_location(name,Path(__file__).with_name(file))
    m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m

audit=load('audit','audit-fortress-interior.py')
panels=load('panels','measure-broadside.py')

class MeasurementTests(unittest.TestCase):
    def test_nearest_two_sided_ray_and_miss(self):
        t=np.array([[[0,0,0],[4,0,0],[0,4,0]],[[0,0,-2],[4,0,-2],[0,4,-2]]],dtype=float)
        self.assertAlmostEqual(audit.ray(t,[1,1,3],[0,0,-1]),3)
        self.assertAlmostEqual(audit.ray(t[:,::-1],[1,1,3],[0,0,-1]),3)
        self.assertIsNone(audit.ray(t,[5,5,3],[0,0,-1]))
        self.assertIsNone(audit.ray(t,[1,1,3],[1,0,0]))
        self.assertIsNone(audit.ray(t,[1,1,3],[0,0,-1],limit=2))

    def test_point_contact_is_not_edge_adjacency(self):
        tris=np.array([[[0,0,0],[1,0,0],[0,1,0]],
                       [[0,0,0],[-1,0,0],[0,-1,0]],
                       [[1,0,0],[1,1,0],[0,1,0]]],dtype=float)
        self.assertEqual(sorted(map(len,panels.connected_components(tris))),[1,2])

if __name__=='__main__':unittest.main()
