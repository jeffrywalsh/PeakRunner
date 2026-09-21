from pathlib import Path
import unittest
import numpy as np
import interior_file as dif
import shape_file as dts
from stonehenge import rotation, node_matrix, material_pixels, donut_wall
from PIL import Image


class NativeGeometryTests(unittest.TestCase):
    def test_opaque_material_alpha_is_not_a_cutout(self):
        source=Image.new('RGBA',(1,1),(12,34,56,0))
        self.assertEqual(material_pixels(source,True).getpixel((0,0)),(12,34,56,255))
        self.assertEqual(material_pixels(source,False).getpixel((0,0)),(12,34,56,0))
        self.assertEqual(source.getpixel((0,0)),(12,34,56,0))

    def surface(self):
        return dict(normals=[[0,0,1]],planes=[[0,0]],points=[[0,0,0],[0,1,0],[1,0,0],[1,1,0]],
                    texgen=[[1,0,0,0,0,1,0,0]],materials=['wall'],windings=[0,1,2,3],
                    normal_lightmaps=[255],lightmaps=[],surfaces=[dict(start=0,count=4,plane=0,
                    material=0,texgen=0,light_word=0,light_offset=[0,0])])
    def test_dif_strip_not_fan(self):
        tris=list(dif.triangles(self.surface()))
        self.assertEqual([c['position'] for c in tris[1]['corners']],[[1,0,0],[0,1,0],[1,1,0]])
        area=0
        for t in tris:
            a,b,c=[np.array(v['position']) for v in t['corners']]
            area+=np.linalg.norm(np.cross(b-a,c-a))/2
        self.assertEqual(area,1)
    def test_donut_fits_source_wall_and_rejects_small_faces(self):
        d=self.surface();d['normals']=[[0,-1,0]]
        d['points']=[[0,0,0],[0,0,10],[10,0,0],[10,0,10]]
        center,n=donut_wall(d,np.array([3,2,3]),1.5)
        self.assertEqual(center[1],0);np.testing.assert_allclose(n,[0,-1,0])
        with self.assertRaises(ValueError): donut_wall(d,np.zeros(3),100)
    def test_invalid_indices_and_nonfinite_fail(self):
        d=self.surface();d['windings'][2]=999
        with self.assertRaises(dif.InteriorError):list(dif.triangles(d))
        d=self.surface();d['points'][0][0]=float('nan')
        with self.assertRaises(dif.InteriorError):list(dif.triangles(d))
    def test_mesh_strip_fan_and_list(self):
        mesh=dict(points=[[0,0,0]]*4,indices=[0,1,2,3],primitives=[[0,4,0x60000000]])
        self.assertEqual([t[0] for t in dts.mesh_triangles(mesh,1)],[[0,1,2],[2,1,3]])
        mesh['primitives'][0][2]=0xa0000000
        self.assertEqual([t[0] for t in dts.mesh_triangles(mesh,1)],[[0,1,2],[0,2,3]])
        mesh['primitives'][0]=[0,3,0x20000000]
        self.assertEqual([t[0] for t in dts.mesh_triangles(mesh,1)],[[0,1,2]])
    def test_axis_and_node_transforms_agree(self):
        rot=rotation([0,0,1,90])
        np.testing.assert_allclose(rot@[1,0,0],[0,-1,0],atol=1e-7)
        self.assertAlmostEqual(np.linalg.det(rot),1)
        d=dict(nodes=[[0,-1],[0,0]],rotations=[[0,0,0,32767]]*2,translations=[[1,2,3],[4,5,6]])
        np.testing.assert_allclose(node_matrix(d,1)[:3,3],[5,7,9])
        d['nodes'][0][1]=1
        with self.assertRaises(ValueError):node_matrix(d,1)
    def test_truncated_native_headers_fail(self):
        for reader in (dif.decode,dts.decode):
            for data in (b'',b'\0'*4,b'\0'*16):
                with self.assertRaises(dif.InteriorError): reader(data)
    def test_bsp_clips_only_interior_side(self):
        d=dict(normals=[[1,0,0]],planes=[[0,0]],bsp_nodes=[[0,0x8000,0x8001]])
        parts=dif.exterior_fragments(d,[[-1,-1,0],[1,-1,0],[1,1,0],[-1,1,0]])
        self.assertEqual(len(parts),1)
        self.assertTrue(all(p[0]>=0 for p in parts[0]))
        self.assertEqual(len(parts[0]),4)
        d['bsp_nodes'][0][1]=0
        with self.assertRaises(dif.InteriorError):
            dif.exterior_fragments(d,[[1,0,0],[2,0,0],[1,1,0]])

    def test_installed_source_geometry_when_available(self):
        root=Path(__file__).resolve().parents[2]/'local-assets/tribes-map-catalog/tribes2'
        if not root.exists(): self.skipTest('private source files not installed')
        for path in sorted((root/'Classic_maps_v1/interiors').glob('*stonehenge*.dif')):
            d=dif.decode(path.read_bytes())
            tris=list(dif.triangles(d))
            self.assertEqual(len(tris),sum(s['count']-2 for s in d['surfaces']))
            for tri in tris:
                s=d['surfaces'][tri['surface']]
                ni,dist=d['planes'][s['plane']&0x7fff]
                for c in tri['corners']:
                    self.assertLess(abs(np.dot(c['position'],d['normals'][ni])+dist),.01)
        for name in ('borg1','borg5'):
            d=dts.decode((root/'shapes/shapes'/f'{name}.dts').read_bytes())
            for obj in d['objects']:
                m=node_matrix(d,obj[3]); mesh=d['meshes'][obj[2]]
                points=np.array(mesh['points'])
                # Per-mesh bounds describe the actual vertices. Shape-level bounds
                # are authored envelopes (borg1 extends 0.16m below its envelope).
                np.testing.assert_allclose(points.min(axis=0),mesh['bounds'][:3],atol=.01)
                np.testing.assert_allclose(points.max(axis=0),mesh['bounds'][3:6],atol=.01)
                self.assertAlmostEqual(np.linalg.det(m[:3,:3]),1)


if __name__=='__main__':unittest.main()
