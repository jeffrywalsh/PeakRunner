#!/usr/bin/env python3
"""Test reusable asset transforms without reading source-game assets."""
import importlib.util
import math
from pathlib import Path
import unittest
from assets import floating_fortress as fortress
from assets import fortress_materials
from assets import docking_bay

loader=importlib.util.spec_from_file_location('kit',Path(__file__).with_name('build-original-map.py'))
kit=importlib.util.module_from_spec(loader);loader.loader.exec_module(kit)

class FortressTests(unittest.TestCase):
    def test_docking_route_floor_and_headroom(self):
        import numpy as np
        mesh=kit.Mesh(); fortress.build(mesh,0,'one'); docking_bay.build(mesh,0)
        triangles=np.array(mesh.collision).reshape(-1,3,3)
        # Vertical triangle intersections at three lanes along the walking route.
        for z in range(-92,-51):
            floor=-12 if z<=-78 else min(0,(z+78)*.5-12)
            for x in (-2,0,2):
                heights=[]
                for a,b,c in triangles:
                    mat=np.array([[b[0]-a[0],c[0]-a[0]],[b[2]-a[2],c[2]-a[2]]])
                    if abs(np.linalg.det(mat))<1e-8: continue
                    u,v=np.linalg.solve(mat,[x-a[0],z-a[2]])
                    if u>=-1e-6 and v>=-1e-6 and u+v<=1+1e-6:
                        heights.append(a[1]+u*(b[1]-a[1])+v*(c[1]-a[1]))
                self.assertTrue(any(abs(y-floor)<.01 for y in heights),(x,z,'floor'))
                self.assertFalse(any(floor+.05<y<floor+2.5 for y in heights),(x,z,'headroom'))

    def test_original_fortress_materials_are_deterministic_opaque_layers(self):
        layers=[]
        for material in ['concrete','panel','grate','trim']:
            first=fortress_materials.texture(material,41023,kit.noise,kit.value_noise)
            self.assertEqual(first,fortress_materials.texture(material,41023,kit.noise,kit.value_noise))
            self.assertEqual(len(first),256*256*4)
            self.assertEqual(set(first[3::4]),{255})
            layers.append(first)
        self.assertEqual(len(set(layers)),4)

    def test_hall_and_grate_landing_do_not_overlap(self):
        class RecordingMesh(kit.Mesh):
            def __init__(self):
                super().__init__(); self.floor_rects=[]
            def box(self,p,size,mat='concrete',solid=True):
                if solid and mat in ('panel','grate') and any(abs(p[1]+size[1]/2-y)<1e-6 for y in [0,7,14,25,31,38,45,57]):
                    self.floor_rects.append((p,size))
                super().box(p,size,mat,solid)
        mesh=RecordingMesh(); fortress.build(mesh,0,'one')
        for i,(a,sa) in enumerate(mesh.floor_rects):
            for b,sb in mesh.floor_rects[i+1:]:
                if abs(a[1]+sa[1]/2-b[1]-sb[1]/2)>1e-6: continue
                overlaps=all(min(a[k]+sa[k]/2,b[k]+sb[k]/2)-
                             max(a[k]-sa[k]/2,b[k]-sb[k]/2)>1e-6 for k in [0,2])
                self.assertFalse(overlaps, f'Coplanar floor overlap: {a} / {b}')

    def test_arbitrary_map_transform_and_anchors(self):
        a,b=kit.Mesh(),kit.Mesh()
        b.origin=(321,75,987); b.yaw=.73
        anchors=fortress.build(a,0,'one')
        moved=fortress.build(b,1,'two')
        self.assertEqual(anchors,moved)
        self.assertEqual(len(a.collision),len(b.collision))
        for i in range(0,len(a.collision),3):
            expected=b.point(tuple(a.collision[i:i+3]))
            for x,y in zip(expected,b.collision[i:i+3]): self.assertAlmostEqual(x,y,places=3)
        self.assertTrue(all(e['team']==1 and e['circuit']=='two' for e in b.entities))
        self.assertEqual(sum(e['kind']=='turret' and e['weapon']=='plasma' for e in b.entities),2)
        self.assertEqual(len(moved['entrances']),2)
        self.assertLess(len(b.collision)//9,3000)

    def test_two_instances_have_unique_equipment_ids_and_independent_power(self):
        mesh=kit.Mesh();fortress.build(mesh,0,'red')
        mesh.origin=(1000,240,1400);mesh.yaw=math.pi
        fortress.build(mesh,1,'blue')
        ids=[e['id'] for e in mesh.entities]
        self.assertEqual(len(ids),len(set(ids)))
        self.assertEqual({e['circuit'] for e in mesh.entities},{'red','blue'})
        for team in [0,1]:
            self.assertEqual(sum(e['kind']=='generator' and e['team']==team for e in mesh.entities),2)

if __name__=='__main__':unittest.main()
