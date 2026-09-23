#!/usr/bin/env python3
"""Test the tower-complex asset builder without reading source-game assets."""
import importlib.util
import math
from pathlib import Path
import unittest
from assets import tower_complex
from assets import tower_complex_materials

loader=importlib.util.spec_from_file_location('kit',Path(__file__).with_name('build-original-map.py'))
kit=importlib.util.module_from_spec(loader);loader.loader.exec_module(kit)


def floor_heights_at(collision, x, z):
    """All triangle intersections of a vertical ray at (x,z) against the
    collision soup, sorted low to high."""
    import numpy as np
    triangles = np.array(collision).reshape(-1,3,3)
    heights=[]
    for a,b,c in triangles:
        mat=np.array([[b[0]-a[0],c[0]-a[0]],[b[2]-a[2],c[2]-a[2]]])
        if abs(np.linalg.det(mat))<1e-8: continue
        u,v=np.linalg.solve(mat,[x-a[0],z-a[2]])
        if u>=-1e-6 and v>=-1e-6 and u+v<=1+1e-6:
            heights.append(a[1]+u*(b[1]-a[1])+v*(c[1]-a[1]))
    return sorted(heights)


class TowerComplexTests(unittest.TestCase):
    def test_level_floors_and_headroom_away_from_shaft(self):
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        # Sample points clear of the shaft hole (|x| or |z| > SHAFT_HALF) and
        # clear of the switchback ramp lanes (x != +-9).
        for y in (tower_complex.L1, tower_complex.L2, tower_complex.L3):
            for x,z in [(6,6),(-6,-6),(6,-6),(-6,6)]:
                heights=floor_heights_at(mesh.collision,x,z)
                self.assertTrue(any(abs(h-y)<.6 for h in heights),(x,z,y,'floor'))
                self.assertFalse(any(y+.6<h<y+2.5 for h in heights),(x,z,y,'headroom'))

    def test_ramps_keep_headroom_up_to_the_next_floor(self):
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        T=tower_complex.TOWER_HALF
        Z0=tower_complex.RAMP1_Z0
        ramps=[(-9,lambda z:7*(z-Z0)/(T-1-Z0),range(int(Z0)+1,11)),(9,lambda z:7+7*(T-1-z)/(2*T-2),range(-10,11))]
        for x,surface,zs in ramps:
            for z in zs:
                y=surface(z)
                heights=floor_heights_at(mesh.collision,x,z)
                self.assertTrue(any(abs(h-y)<.05 for h in heights),(x,z,'ramp surface'))
                self.assertFalse(any(y+.2<h<y+2.2 for h in heights),(x,z,y,'ramp headroom'))

    def test_shaft_is_a_drop_to_l1_with_one_open_face_per_level(self):
        import numpy as np
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        heights=floor_heights_at(mesh.collision,0,0)
        self.assertTrue(any(abs(h-tower_complex.L1)<.6 for h in heights),'tube needs its L1 floor')
        for y in (tower_complex.L2, tower_complex.L3):
            self.assertFalse(any(abs(h-y)<.6 for h in heights),(y,'shaft should be open'))
        tris=np.array(mesh.collision).reshape(-1,3,3)
        def blocked(a,b):
            a,b=np.array(a,float),np.array(b,float); d=b-a
            for p0,p1,p2 in tris:
                e1,e2=p1-p0,p2-p0; h=np.cross(d,e2); det=e1@h
                if abs(det)<1e-9: continue
                s=a-p0; u=(s@h)/det; q=np.cross(s,e1); v=(d@q)/det; t=(e2@q)/det
                if u>=0 and v>=0 and u+v<=1 and 0<=t<=1: return True
            return False
        faces={'-z':(0,-5),'+z':(0,5),'-x':(-5,0),'+x':(5,0)}
        for side,ya,yb in tower_complex.SHAFT_OPENINGS:
            level=ya
            for other,(fx,fz) in faces.items():
                ray=blocked((0,level+1.5,0),(fx,level+1.5,fz))
                self.assertEqual(ray, other!=side, (level,other,'open face' if other==side else 'closed face'))

    def test_only_uses_valid_kit_materials(self):
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        # Mesh.triangle appends pos3, normal3, uv2, 0, 0, material_index, -1
        # per vertex (12 floats); material index is the 11th value (offset 10).
        vstride=12
        indices={int(mesh.vertices[i+10]) for i in range(0,len(mesh.vertices),vstride)}
        self.assertTrue(indices.issubset(set(range(len(kit.MATERIALS)))))

    def test_no_overlapping_coplanar_floor_or_roof_plates(self):
        class RecordingMesh(kit.Mesh):
            def __init__(self):
                super().__init__(); self.floor_rects=[]
            def box(self,p,size,mat='concrete',solid=True):
                floor_ys={tower_complex.L1,tower_complex.L2,tower_complex.L3,tower_complex.ROOF,
                          tower_complex.L1+5,19,29}
                # Only thin floor/roof plates (slab's 1 m boxes), not tall walls
                # whose top happens to land on the same level height.
                if solid and mat in ('panel','grate','concrete') and size[1]<=1.2 and any(abs(p[1]+size[1]/2-y)<1e-6 for y in floor_ys):
                    self.floor_rects.append((p,size))
                super().box(p,size,mat,solid)
        mesh=RecordingMesh(); tower_complex.build(mesh,0,'one')
        for i,(a,sa) in enumerate(mesh.floor_rects):
            for b,sb in mesh.floor_rects[i+1:]:
                if abs(a[1]-b[1])>1e-6: continue
                overlaps=all(min(a[k]+sa[k]/2,b[k]+sb[k]/2)-
                             max(a[k]-sa[k]/2,b[k]-sb[k]/2)>1e-6 for k in [0,2])
                self.assertFalse(overlaps, f'Coplanar floor overlap: {a} / {b}')

    def test_arbitrary_map_transform_and_anchors(self):
        a,b=kit.Mesh(),kit.Mesh()
        b.origin=(321,75,987); b.yaw=.73
        anchors=tower_complex.build(a,0,'one')
        moved=tower_complex.build(b,1,'two')
        self.assertEqual(anchors,moved)
        self.assertEqual(len(a.collision),len(b.collision))
        for i in range(0,len(a.collision),3):
            expected=b.point(tuple(a.collision[i:i+3]))
            for x,y in zip(expected,b.collision[i:i+3]): self.assertAlmostEqual(x,y,places=3)
        self.assertTrue(all(e['team']==1 and e['circuit']=='two' for e in b.entities))
        self.assertEqual(sum(e['kind']=='turret' for e in b.entities),2)
        self.assertEqual(sum(e['kind']=='generator' for e in b.entities),1)
        self.assertEqual(len(moved['entrances']),5)
        self.assertLess(len(b.collision)//9,3000)

    def test_two_instances_have_unique_equipment_ids_and_independent_power(self):
        mesh=kit.Mesh();tower_complex.build(mesh,0,'red')
        mesh.origin=(1000,240,1400);mesh.yaw=math.pi
        tower_complex.build(mesh,1,'blue')
        ids=[e['id'] for e in mesh.entities]
        self.assertEqual(len(ids),len(set(ids)))
        self.assertEqual({e['circuit'] for e in mesh.entities},{'red','blue'})
        for team in [0,1]:
            self.assertEqual(sum(e['kind']=='generator' and e['team']==team for e in mesh.entities),1)
            self.assertEqual(sum(e['kind']=='turret' and e['team']==team for e in mesh.entities),2)

    def test_materials_are_deterministic_opaque_and_distinct(self):
        names = ['concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark']
        for name in names:
            a = tower_complex_materials.texture(name, 7, kit.noise, kit.value_noise)
            b = tower_complex_materials.texture(name, 7, kit.noise, kit.value_noise)
            self.assertEqual(a, b, f'{name} texture is not deterministic')
            self.assertEqual(len(a), 256*256*4)
            self.assertTrue(all(a[i] == 255 for i in range(3, len(a), 4)), f'{name} has transparent texels')
        textures = {name: tower_complex_materials.texture(name, 7, kit.noise, kit.value_noise) for name in names}
        for i, a in enumerate(names):
            for b in names[i+1:]:
                self.assertNotEqual(textures[a], textures[b], f'{a} and {b} render identically')

    def test_every_structure_hangs_a_keel_below_its_deck(self):
        import numpy as np
        mesh=kit.Mesh(); anchors=tower_complex.build(mesh,0,'one')
        pts=np.array(mesh.collision).reshape(-1,3)
        centres={'tower':(0,0),'generator':anchors['generator'][::2],
                 'ship_platform':anchors['ship_platform'][::2],
                 'turret_left':anchors['turret_left'][::2],'turret_right':anchors['turret_right'][::2]}
        for name,(cx,cz) in centres.items():
            near=pts[(abs(pts[:,0]-cx)<6)&(abs(pts[:,2]-cz)<6)]
            self.assertLess(near[:,1].min(),-9,f'{name} has no keel under its deck')
        # Bridges and tunnels cross open air: nothing solid directly under
        # their midpoints for at least 20 m.
        for x,z in [(-9,(-tower_complex.TOWER_HALF+tower_complex.POD_Z)/2),
                    (-8,(tower_complex.TOWER_HALF+tower_complex.REAR_Z0)/2)]:
            below=[h for h in floor_heights_at(mesh.collision,x,z) if h<-1.5]
            self.assertFalse(any(h>-20 for h in below),(x,z,below))

    def test_flag_stands_on_its_plinth(self):
        mesh=kit.Mesh(); anchors=tower_complex.build(mesh,0,'one')
        fx,fy,fz=anchors['flag']
        top=tower_complex.L2+tower_complex.PLINTH
        heights=floor_heights_at(mesh.collision,fx,fz)
        self.assertTrue(any(abs(h-top)<.02 for h in heights),heights)
        self.assertAlmostEqual(fy,top+.05,places=6)
        self.assertFalse(any(top+.1<h<top+2.5 for h in heights),'flag needs headroom')

    def test_level3_windows_are_glazed_openings(self):
        import numpy as np
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        def hits(tris,a,b):
            a,b=np.array(a,float),np.array(b,float); d=b-a; n=0
            for p0,p1,p2 in tris:
                e1,e2=p1-p0,p2-p0; h=np.cross(d,e2); det=e1@h
                if abs(det)<1e-9: continue
                s=a-p0; u=(s@h)/det; q=np.cross(s,e1); v=(d@q)/det; t=(e2@q)/det
                if u>=0 and v>=0 and u+v<=1 and 0<=t<=1: n+=1
            return n
        solid=np.array(mesh.collision).reshape(-1,3,3)
        render=np.array(mesh.vertices).reshape(-1,3,12)[:,:,:3]
        y=tower_complex.L3+2.2
        # Mid-bay on each side wall: glass blocks, nothing drawn in the way.
        for a,b in [((1.5,y,10),(1.5,y,14)),((10,y,1.5),(14,y,1.5)),((-10,y,-1.5),(-14,y,-1.5)),((7.4,y,-10),(7.4,y,-14))]:
            self.assertGreater(hits(solid,a,b),0,(a,'pane'))
            self.assertEqual(hits(render,a,b),0,(a,'view blocked'))
        # The front banner strip stays solid wall.
        self.assertGreater(hits(render,(0,y,-10),(0,y,-14)),0)

    def test_collision_budget(self):
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        self.assertLess(len(mesh.collision)//9,3000)

    def test_lightmap_bake_is_deterministic_valid_and_shadows(self):
        import numpy as np
        from assets import lightmap_bake
        m=kit.Mesh(); m.lamps=[]
        # Two floor tiles: one under a roof slab, one in the open; a lamp strip.
        m.quad((-2,0,-2),(2,0,-2),(2,0,2),(-2,0,2),'panel')
        m.quad((13,0,-2),(17,0,-2),(17,0,2),(13,0,2),'panel')
        m.box((0,2.6,0),(9,.4,9),'concrete')
        m.box((0,4,12),(.4,.1,3),'light',False)
        verts=np.array(m.vertices,np.float32).reshape(-1,12)
        sun=(-.57735,.57735,-.57735); light=kit.MATERIALS.index('light'); base=20
        a,pa=lightmap_bake.bake(verts,m.lamps,sun,light,base,processes=1)
        b,pb=lightmap_bake.bake(verts,m.lamps,sun,light,base,processes=3)
        self.assertTrue(np.array_equal(a,b)); self.assertEqual(pa,pb)
        self.assertTrue(np.all((a[:,11]>=base)&(a[:,11]<base+len(pa))))
        self.assertTrue(np.all((a[:,8]>=0)&(a[:,8]<=1)&(a[:,9]>=0)&(a[:,9]<=1)))
        self.assertTrue(np.array_equal(a[:,:8],verts[:,:8]))   # geometry untouched
        def mean(tri):
            uv=a[tri*3:tri*3+6,8:10]; lay=int(a[tri*3,11])-base
            x0,y0=(uv.min(0)*256).astype(int); x1,y1=np.ceil(uv.max(0)*256).astype(int)
            page=np.frombuffer(pa[lay],np.uint8).reshape(256,256,4)
            return page[y0:y1,x0:x1,:3].mean()
        self.assertLess(mean(0),.7*mean(2),'shadowed floor should bake darker')
        glow=a[verts[:,10]==light]
        self.assertTrue(np.all(glow[:,11]==base))
        page=np.frombuffer(pa[0],np.uint8).reshape(256,256,4)
        self.assertTrue(np.all(page[:4,:4,:3]==255))

    def _build_module(self):
        loader=importlib.util.spec_from_file_location('tc_build',Path(__file__).with_name('build-tower-complex.py'))
        build=importlib.util.module_from_spec(loader);loader.loader.exec_module(build)
        import json
        spec=json.loads((Path(__file__).parent.parent/'maps/tower-complex.json').read_text())
        return build,spec

    def test_terrain_clears_every_hull_and_rolls_between_bases(self):
        import numpy as np
        from assets import tower_complex_terrain as terrain
        build,spec=self._build_module()
        grid=build.terrain_grid(spec)
        self.assertTrue(np.array_equal(grid,build.terrain_grid(spec)),'terrain must be deterministic')
        for base in spec['bases']:
            bx,by,bz=base['position']
            # Deepest keel tip is 34 m below the deck; keep 20 m of air under it
            # across the whole complex (tower, pods 30 m out, rear rooms 36 m).
            for dx in range(-16,17,4):
                for dz in range(-40,41,4):
                    self.assertLess(build.terrain_height(bx+dx,bz+dz,grid),by-54,(base['team'],dx,dz))
        # Flow: rolling ground, not a flat floor with mounds (compare the
        # original v4 canyon: 30% of cells under 3 degrees, median 5 degrees).
        play=grid[40:216,40:216]
        slope=terrain.slope_degrees(grid)[40:216,40:216]
        self.assertLess((slope<3).mean(),.08)
        self.assertTrue(12<np.median(slope)<26,np.median(slope))
        self.assertGreater(np.percentile(slope,90),28)
        self.assertGreater(np.ptp(play),120)
        # The straight line between flags dips into a midfield basin.
        (rx,_,rz),(bx,_,bz)=[b['position'] for b in spec['bases']]
        profile=[build.terrain_height(rx+(bx-rx)*t,rz+(bz-rz)*t,grid) for t in np.linspace(0,1,41)]
        self.assertGreater(min(profile[0],profile[-1])-min(profile),25)

    def test_standing_support_is_the_floor_top_everywhere(self):
        """The engine finds a standing player's floor with a ray from 0.15 m
        above the feet. Wherever a player can stand, that ray's first hit must
        be the up-facing floor top, not a slab underside or a coplanar
        down-facing face (which leaves the player airborne and frictionless)."""
        import numpy as np
        mesh=kit.Mesh(); anchors=tower_complex.build(mesh,0,'one')
        tris=np.array(mesh.collision).reshape(-1,3,3)
        a,b,c=tris[:,0],tris[:,1],tris[:,2]
        n=np.cross(b-a,c-a); n/=np.linalg.norm(n,axis=1,keepdims=True)+1e-12
        def down_hits(x,z,y0):
            # Vertical ray straight down from (x,y0,z): (height, normal.y) sorted high to low.
            e1,e2=b-a,c-a; d=np.array([0.,-1.,0.])
            h=np.cross(d,e2); det=np.einsum('ij,ij->i',e1,h)
            ok=np.abs(det)>1e-9; s=np.array([x,y0,z])-a
            u=np.einsum('ij,ij->i',s,h)/np.where(ok,det,1)
            q=np.cross(s,e1); v=(q@d)/np.where(ok,det,1); t=np.einsum('ij,ij->i',e2,q)/np.where(ok,det,1)
            m=ok&(u>=-1e-6)&(v>=-1e-6)&(u+v<=1+1e-6)&(t>=0)
            order=np.argsort(t[m]); return list(zip((y0-t[m])[order],n[m][order,1]))
        checked=0
        regions=[(-11.4,11.4,-11.4,11.4,y) for y in (tower_complex.L1,tower_complex.L2,tower_complex.L3)]
        regions+=[(-12.4,-3.6,26.6,35.4,0),(3.6,12.4,26.6,35.4,0),(-9.6,-6.4,12.2,25.8,0),(6.4,9.6,12.2,25.8,0)]
        for x0,x1,z0,z1,level in regions:
            for x in np.arange(x0,x1+.01,.8):
                for z in np.arange(z0,z1+.01,.8):
                    above=down_hits(x,z,level+2.6)
                    # Only cells whose top surface is this level's floor with
                    # standing room (no solid between floor+0.2 and floor+2.4).
                    # Skip cells inside a solid resting on the floor: its
                    # bottom face ties with the floor top at the same height.
                    if not above or abs(above[0][0]-level)>.02 or any(
                        hy<.99 for hh,hy in above if abs(hh-above[0][0])<.01): continue
                    first=down_hits(x,z,level+.15)
                    checked+=1
                    self.assertTrue(first and abs(first[0][0]-level)<.02 and first[0][1]>.99,
                                    (x,z,level,first[:2]))
        self.assertGreater(checked,600)
        cen=tris.mean(axis=1)
        # No accidental slopes on the floors: tilted walkable solid faces sit
        # only on the ramps, the flag plinth chamfer and kit equipment.
        ents=[e['position'] for e in mesh.entities]
        for i in np.where((n[:,1]>.3)&(n[:,1]<.9999))[0]:
            x,y,z=cen[i]
            if y<tower_complex.L1-.5 or y>tower_complex.ROOF+.5: continue   # keels, exterior massing
            if abs(abs(x)-9)<=2.01 and abs(z)<=tower_complex.TOWER_HALF: continue  # ramps
            if math.hypot(x-tower_complex.FLAG_X,z)<1.8 and abs(y-tower_complex.L2-.15)<.3: continue
            if any(math.hypot(x-e[0],z-e[2])<3.3 for e in ents): continue
            self.fail(('accidental slope',round(float(n[i,1]),4),cen[i]))

    def test_spawn_points_stand_on_floor_with_room_and_face_open_space(self):
        import numpy as np
        mesh=kit.Mesh(); anchors=tower_complex.build(mesh,0,'one')
        points=anchors['spawn_points']
        self.assertGreaterEqual(len(points),6)
        self.assertEqual(tuple(anchors['spawn']),tuple(points[0][:3]))
        levels={round(p[1]-tower_complex.SPAWN_LIFT) for p in points}
        self.assertEqual(levels,{tower_complex.L1,tower_complex.L2,tower_complex.L3})
        tris=np.array(mesh.collision).reshape(-1,3,3)
        def first_hit(o,d):
            o,d=np.array(o,float),np.array(d,float); a,b,c=tris[:,0],tris[:,1],tris[:,2]
            e1,e2=b-a,c-a; h=np.cross(d,e2); det=np.einsum('ij,ij->i',e1,h)
            ok=np.abs(det)>1e-9; s=o-a; det=np.where(ok,det,1)
            u=np.einsum('ij,ij->i',s,h)/det; q=np.cross(s,e1); v=(q@d)/det; t=np.einsum('ij,ij->i',e2,q)/det
            m=ok&(u>=0)&(v>=0)&(u+v<=1)&(t>=0)
            return t[m].min() if m.any() else np.inf
        for x,y,z,yaw in points:
            floor=y-tower_complex.SPAWN_LIFT
            self.assertAlmostEqual(y-first_hit((x,y-.37,z),(0,-1,0))-.37,floor,places=3)
            self.assertGreater(first_hit((x,floor+.2,z),(0,1,0)),2.4,(x,z,'headroom'))
            for ang in np.linspace(0,2*np.pi,8,endpoint=False):
                for h in (.3,1.,1.8):
                    self.assertGreater(first_hit((x,floor+h,z),(np.cos(ang),0,np.sin(ang))),.7,(x,z,h,'clearance'))
            facing=(-math.sin(yaw),0,-math.cos(yaw))
            self.assertGreater(first_hit((x,floor+1.6,z),facing),3.,(x,z,'faces a wall'))

    def test_low_space_under_ramps_is_closed(self):
        import numpy as np
        mesh=kit.Mesh(); tower_complex.build(mesh,0,'one')
        tris=np.array(mesh.collision).reshape(-1,3,3)
        def first_x(o,d):
            o,d=np.array(o,float),np.array(d,float); a,b,c=tris[:,0],tris[:,1],tris[:,2]
            e1,e2=b-a,c-a; h=np.cross(d,e2); det=np.einsum('ij,ij->i',e1,h)
            ok=np.abs(det)>1e-9; s=o-a; det=np.where(ok,det,1)
            u=np.einsum('ij,ij->i',s,h)/det; q=np.cross(s,e1); v=(q@d)/det; t=np.einsum('ij,ij->i',e2,q)/det
            m=ok&(u>=0)&(v>=0)&(u+v<=1)&(t>=0)
            return o[0]+d[0]*t[m].min() if m.any() else None
        L1,L2=tower_complex.L1,tower_complex.L2
        T=tower_complex.TOWER_HALF
        Z0=tower_complex.RAMP1_Z0
        under1=lambda z: 7*(z-Z0)/(T-1-Z0)-.6      # ramp underside above its floor
        under2=lambda z: 7*(T-1-z)/(2*T-2)-.6
        tested=0
        for h in (.3,1.,1.8):
            for z in np.arange(Z0+.3,Z0+(tower_complex.RAMP_HEADROOM+.6)*(T-1-Z0)/7,.5):   # under the L1->L2 ramp's low end
                if h>=under1(z): continue
                x=first_x((-4,L1+h,z),(-1,0,0)); self.assertIsNotNone(x); self.assertGreater(x,-7.06,(z,h)); tested+=1
            for z in np.arange(1.6,8.6,.5):       # under the L2->L3 ramp's low end
                if h>=under2(z): continue
                x=first_x((4,L2+h,z),(1,0,0)); self.assertIsNotNone(x); self.assertLess(x,7.06,(z,h)); tested+=1
        self.assertGreater(tested,20)

    def _pad(self):
        import numpy as np
        from assets import landing_pad
        mesh=kit.Mesh(); anchors=landing_pad.build(mesh,0)
        tris=np.array(mesh.collision).reshape(-1,3,3)
        return landing_pad,mesh,anchors,tris

    @staticmethod
    def _down(tris,x,z,y0):
        """(height, normal.y) of every solid hit straight down from (x,y0,z), nearest first."""
        import numpy as np
        a,b,c=tris[:,0],tris[:,1],tris[:,2]; e1,e2=b-a,c-a
        n=np.cross(e1,e2); n/=np.linalg.norm(n,axis=1,keepdims=True)+1e-12
        d=np.array([0.,-1.,0.]); h=np.cross(d,e2); det=np.einsum('ij,ij->i',e1,h)
        ok=np.abs(det)>1e-9; det=np.where(ok,det,1); s=np.array([x,y0,z])-a
        u=np.einsum('ij,ij->i',s,h)/det; q=np.cross(s,e1); v=(q@d)/det; t=np.einsum('ij,ij->i',e2,q)/det
        m=ok&(u>=-1e-6)&(v>=-1e-6)&(u+v<=1+1e-6)&(t>=0)
        order=np.argsort(t[m]); return list(zip((y0-t[m])[order],n[m][order,1]))

    def test_landing_pad_deck_is_flat_standable_with_open_sky(self):
        import numpy as np
        pad,mesh,anchors,tris=self._pad()
        inner=pad.HALF-pad.LIP_T-.3
        checked=0
        for x in np.arange(-inner,inner+.01,.75):
            for z in np.arange(-inner,inner+.01,.75):
                # The support ray from 0.15 m above the feet meets the deck top.
                first=self._down(tris,x,z,.15)
                self.assertTrue(first and abs(first[0][0])<1e-4 and first[0][1]>.9999,(x,z,first[:2]))
                # Nothing solid over the deck: open landing from above.
                self.assertEqual([hh for hh,_ in self._down(tris,x,z,40.) if hh>1e-3],[],(x,z))
                checked+=1
        self.assertGreaterEqual(checked,900)
        # A heavy (same 0.52 m body as every armor) has room to spare.
        self.assertGreaterEqual(2*inner,20)
        # No accidental slopes on or above the deck; tilted solids are underside only.
        a,b,c=tris[:,0],tris[:,1],tris[:,2]
        n=np.cross(b-a,c-a); n/=np.linalg.norm(n,axis=1,keepdims=True)+1e-12
        tilted=(n[:,1]>.3)&(n[:,1]<.9999)
        self.assertTrue((tris[tilted][:,:,1].max(axis=1)<=-.99).all())

    def test_landing_pad_deploy_slots_edges_and_keel(self):
        pad,mesh,anchors,tris=self._pad()
        slots=anchors['deploy_slots']
        self.assertGreaterEqual(len(slots),3)
        for x,y,z in slots:
            self.assertEqual(y,0)
            self.assertLess(max(abs(x),abs(z))+pad.SLOT_HALF,pad.HALF-pad.LIP_T)
            self.assertAlmostEqual(self._down(tris,x,z,.15)[0][0],0,places=4)
        for (ax,_,az),(bx,_,bz) in [(a,b) for i,a in enumerate(slots) for b in slots[i+1:]]:
            self.assertGreater(math.hypot(ax-bx,az-bz),2*pad.SLOT_HALF+2)
        # Every side's lip has a walk-off gap at its midpoint and is solid either side of it.
        for gx,_,gz in anchors['edge_gaps']:
            ox,oz=gx*.97,gz*.97
            self.assertEqual([h for h,_ in self._down(tris,ox,oz,1.) if h>1e-3],[],(gx,gz))
            sx,sz=(ox+(pad.GAP+1)*(gz!=0),oz+(pad.GAP+1)*(gx!=0))
            self.assertAlmostEqual(self._down(tris,sx,sz,1.)[0][0],pad.LIP,places=4)
        self.assertEqual(anchors['keel_tip'][1],-1-pad.KEEL)
        self.assertEqual(mesh.entities,[])
        self.assertLess(len(mesh.collision)//9,400)

    def test_pads_float_over_terrain_one_per_team_half(self):
        import json, numpy as np
        build,spec=self._build_module()
        from assets import landing_pad, tower_complex_terrain as terrain
        grid=build.terrain_grid(spec)
        pads=spec['pads']
        self.assertEqual(sorted(p['team'] for p in pads),[0,1])
        (rx,_,rz),(bx,_,bz)=[b['position'] for b in spec['bases']]
        mid=((rx+bx)/2,(rz+bz)/2)
        for p in pads:
            x,y,z=p['position']; home=spec['bases'][p['team']]['position']
            # On its own team's half: nearer its own base than the enemy's.
            other=spec['bases'][1-p['team']]['position']
            self.assertLess(math.hypot(x-home[0],z-home[2]),math.hypot(x-other[0],z-other[2]))
            for dx in range(-14,15,2):
                for dz in range(-14,15,2):
                    self.assertLess(build.terrain_height(x+dx,z+dz,grid),y-landing_pad.KEEL-8,(p['team'],dx,dz))
            self.assertLessEqual(y-build.terrain_height(x,z,grid),80,'reachable by jetting from the ground')

if __name__=='__main__':unittest.main()
