#!/usr/bin/env python3
"""Convert a user-owned T2 Classic Raindance into a local PeakRunner map pack.

Never evaluates TorqueScript. Requires Pillow/numpy plus pinned local copies of
io_dif and io_scene_dts (see docs/raindance-import.md). Assets are not distributable
under PeakRunner's code license. Unsupported gameplay is recorded, not invented.
"""
import argparse
import hashlib
import importlib
import io
import json
import math
import re
import struct
import sys
import types
import wave
from collections import Counter
from pathlib import Path

import numpy as np
from PIL import Image


def objects(text):
    # Mission writer's declarative subset. Strings/comments are tokens so braces
    # inside them cannot change nesting. No eval, exec, or mission script imports.
    tokens = re.findall(r'//[^\n]*|"(?:\\.|[^"\\])*"|[A-Za-z_][\w\[\]]*|[{}();=]|[^\s]', text)
    tokens = [t for t in tokens if not t.startswith('//')]
    stack, result, i = [], [], 0
    while i < len(tokens):
        if tokens[i] == 'new':
            kind = tokens[i+1]
            j = i+3
            names = []
            while tokens[j] != ')':
                names.append(tokens[j]); j += 1
            if tokens[j+1] != '{':
                raise ValueError('Expected declarative object')
            obj = dict(kind=kind, name=''.join(names), fields={}, parents=[x['name'] for x in stack])
            result.append(obj); stack.append(obj); i = j+2
        elif tokens[i] == '}':
            if not stack: raise ValueError('Unbalanced mission')
            stack.pop(); i += 1
        elif stack and i+2 < len(tokens) and tokens[i+1] == '=':
            key = tokens[i]; j = i+2; value = []
            while tokens[j] != ';': value.append(tokens[j]); j += 1
            if len(value) != 1 or not value[0].startswith('"'):
                raise ValueError(f'Nonliteral mission property: {key}')
            stack[-1]['fields'][key] = value[0][1:-1]; i = j+1
        else:
            i += 1
    if stack: raise ValueError('Unclosed mission objects')
    return result


def vec(s): return np.array([float(x) for x in s.split()], dtype=float)


def rotation(axis, angle):
    length = np.linalg.norm(axis)
    if length < 1e-10 or abs(angle) < 1e-10: return np.eye(3)
    x, y, z = axis / length
    c, s = math.cos(angle), math.sin(angle)
    k = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]])
    return np.eye(3)*c + (1-c)*np.outer([x,y,z], [x,y,z]) + s*k


def transform(fields):
    rot = vec(fields.get('rotation', '1 0 0 0'))
    # Torque AngAxisF uses the opposite rotation sign to conventional Rodrigues.
    r = rotation(rot[:3], -math.radians(rot[3]))
    return r @ np.diag(vec(fields.get('scale', '1 1 1'))), vec(fields.get('position', '0 0 0'))


def point(p): return np.array([p[0]+1024, p[2], p[1]+1024])
def direction(p): return np.array([p[0], p[2], p[1]])
def xyz(p): return np.array([p.x, p.y, p.z])


def load_parsers(tools):
    sys.path.insert(0, str(tools/'io_dif/blender_plugin/io_dif'))
    from hxDif import Dif
    # The DTS binary reader only needs value constructors, not Blender itself.
    m = types.ModuleType('mathutils')
    m.Vector = lambda v=(0,0,0): np.array(v, dtype=float)
    m.Quaternion = lambda q: np.array(q, dtype=float)
    m.Matrix = lambda: np.eye(4)
    m.Euler = None
    sys.modules['mathutils'] = m
    package = types.ModuleType('local_dts')
    package.__path__ = [str(tools/'io_scene_dts')]
    sys.modules['local_dts'] = package
    from local_dts.DtsShape import DtsShape
    from local_dts.DtsTypes import Mesh, Primitive
    class DecalRecord:
        @staticmethod
        def read(stream): return [stream.read32() for _ in range(5)]
    importlib.import_module('local_dts.DtsShape').Decal = DecalRecord
    def read_mesh(cls, stream):
        # T2 foliage uses TSSortedMesh: the standard mesh followed by sorting
        # clusters (8 words each), four index arrays, depth flag, guard.
        kind = stream.read32() & 7
        mesh = cls(kind)
        previous = getattr(stream, 'map_meshes', [])
        if kind not in (0,2,3,4): raise ValueError(f'Unsupported animated mesh type {kind}')
        if kind == 2:
            if stream.dtsVersion<20:
                stream.guard()
                for _ in range(15):stream.read32()
            for _ in range(stream.read32()): Primitive.read(stream)
            for _ in range(stream.read32()):stream.read16()
            if stream.dtsVersion<20:
                for _ in range(3):stream.read32()
                stream.guard()
            n=stream.read32()
            for _ in range(n*9):stream.read32()
            stream.read32();stream.guard()
        if kind in (0,3):
            if not 19 <= stream.dtsVersion <= 24: raise ValueError('Unsupported DTS version')
            stream.guard()
            mesh.numFrames=stream.read32();mesh.numMatFrames=stream.read32();mesh.parent=stream.read32()
            mesh.bounds=stream.read_box();mesh.center=stream.read_vec3();mesh.radius=stream.read_float()
            parent=previous[mesh.parent] if mesh.parent>=0 else None
            n=stream.read32();mesh.verts=parent.verts[:n] if parent else [stream.read_vec3() for _ in range(n)]
            nt=stream.read32();mesh.tverts=parent.tverts[:nt] if parent else [stream.read_vec2() for _ in range(nt)]
            mesh.normals=parent.normals[:n] if parent else [stream.read_vec3() for _ in range(n)]
            if not parent and stream.dtsVersion>21:
                for _ in range(n):stream.read8()
            mesh.primitives=[Primitive.read(stream) for _ in range(stream.read32())]
            mesh.indices=[stream.read16() for _ in range(stream.read32())]
            mesh.mindices=[stream.read16() for _ in range(stream.read32())]
            mesh.vertsPerFrame=stream.read32();mesh.set_flags(stream.read32());stream.guard()
        if kind == 3:
            for width in [8,1,1,1,1]:
                n = stream.read32()
                if not 0 <= n <= 1_000_000: raise ValueError('Invalid sorted-mesh count')
                for _ in range(n*width): stream.read32()
            stream.read32(); stream.guard()
        previous.append(mesh);stream.map_meshes=previous
        return mesh
    Mesh.read = classmethod(read_mesh)
    return Dif, DtsShape


class Converter:
    def __init__(self, root, tools):
        self.root = root
        self.Dif, self.Dts = load_parsers(tools)
        self.files = sorted(p for p in root.rglob('*') if p.is_file())
        self.images = [Image.new('RGBA', (256,256), (255,255,255,255))]
        self.layers = {'white':0}
        self.vertices, self.collision, self.instances = [], [], []
        self.dependencies = {}
        self.difs, self.shapes = {}, {}

    def resolve(self, name):
        name = name.replace('\\','/').lower().removeprefix('base/')
        matches = [p for p in self.files if str(p).lower().endswith('/'+name)]
        if not matches:
            matches = [p for p in self.files if p.name.lower() == name.rsplit('/',1)[-1]]
        if not matches: raise FileNotFoundError(name)
        # Only the selected Classic pack overrides stock resources, not TR2.
        matches = [p for p in matches if not p.relative_to(self.root).parts[0].lower().startswith('tr2')]
        matches.sort(key=lambda p: (p.relative_to(self.root).parts[0] != 'Classic_maps_v1', str(p)))
        chosen = matches[0]
        self.dependencies[str(chosen.relative_to(self.root))] = hashlib.sha256(chosen.read_bytes()).hexdigest()
        return chosen

    def texture(self, name):
        key = name.lower()
        if key in self.layers: return self.layers[key]
        path = None
        for ext in ['', '.png', '.jpg', '.bmp', '.ifl']:
            try: path = self.resolve(name+ext); break
            except FileNotFoundError: pass
        if path is None: raise FileNotFoundError(f'Texture {name}')
        if path.suffix.lower() == '.ifl':
            return self.texture(path.read_text().split()[0])
        img = Image.open(path).convert('RGBA')
        return self.image(key, img)

    def image(self, key, img):
        if key not in self.layers:
            self.layers[key] = len(self.images)
            self.images.append(img.resize((256,256), Image.Resampling.LANCZOS))
        return self.layers[key]

    def triangle(self, positions, normals, uvs, layer, light=-1, lightuv=None, collide=True):
        p = np.array(positions); n = np.array(normals)
        cross = np.cross(p[1]-p[0],p[2]-p[0])
        if np.linalg.norm(cross) < 1e-7: return
        order = [0,1,2] if np.dot(cross, n.mean(axis=0)) >= 0 else [0,2,1]
        lightuv = lightuv if lightuv is not None else [[0,0]]*3
        for k in order:
            self.vertices.append([*p[k], *n[k], *uvs[k], *lightuv[k], layer, light])
        if collide: self.collision.append(p[order].flatten().tolist())

    def interior(self, obj):
        f=obj['fields']; name=f['interiorFile']; start=len(self.collision)
        if name not in self.difs: self.difs[name] = self.Dif.Load(str(self.resolve(name))).interiors[0]
        interior=self.difs[name]; a,t=transform(f); normal_matrix=np.linalg.inv(a).T
        for index,surface in enumerate(interior.surfaces):
            mat=interior.materialList[surface.textureIndex]
            if mat.rsplit('/',1)[-1].upper() in ['NULL','ORIGIN','TRIGGER','FORCEFIELD']: continue
            layer=self.texture(mat)
            lm=interior.normalLMapIndices[index]
            light=-1
            if 0 <= lm < len(interior.lightMaps):
                blob=bytes(interior.lightMaps[lm].lightmap)
                light=self.image(f'{name}:light:{lm}',Image.open(io.BytesIO(blob)).convert('RGBA'))
            normal=xyz(interior.normals[interior.planes[surface.planeIndex & 0x7fff].normalIndex])
            if surface.planeFlipped: normal=-normal
            normal=direction(normal_matrix@normal); normal/=np.linalg.norm(normal)
            tg=interior.texGenEQs[surface.texGenIndex]
            uvplanes=[[*xyz(tg.planeX),tg.planeX.d],[*xyz(tg.planeY),tg.planeY.d]]
            word=surface.lightMapFinalWord
            axes=[(0,1),(0,2),(1,0),(1,2),(2,0),(2,1)][(word>>13)&7]
            scale=[2**(-((word>>6)&63)),2**(-(word&63))]
            offsets=[surface.lightMapTexGenXD,surface.lightMapTexGenYD]
            ids=interior.windings[surface.windingStart:surface.windingStart+surface.windingCount]
            # DIF windings are triangle strips, not polygon fans.
            for j in range(2,len(ids)):
                ps=[xyz(interior.points[ids[k]]) for k in [j-2,j-1,j]]
                uv=[[float(np.dot(v[:3],p)+v[3]) for v in uvplanes] for p in ps]
                luv=[[p[axes[k]]*scale[k]+offsets[k] for k in range(2)] for p in ps]
                shading=-4-light if light>=0 and surface.surfaceFlags & 16 else light
                self.triangle([point(a@p+t) for p in ps],[normal]*3,uv,layer,shading,luv)
        self.instances.append(dict(kind=obj['kind'],name=name,first=start,count=len(self.collision)-start,fields=f))

    def shape(self,obj,name,parent_transform=None):
        if name not in self.shapes:
            shape=self.Dts()
            with self.resolve(name).open('rb') as source: shape.load(source)
            self.shapes[name]=shape
        shape=self.shapes[name]; a,t=transform(obj['fields']); nodes=[]
        for i,node in enumerate(shape.nodes):
            w,x,y,z=shape.default_rotations[i]
            # Reader has already converted Torque quaternion handedness.
            r=np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],
                        [2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],
                        [2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])
            m=np.eye(4);m[:3,:3]=r;m[:3,3]=shape.default_translations[i]
            nodes.append(nodes[node.parent]@m if node.parent>=0 else m)
        if parent_transform is not None:
            mount=next((nodes[i] for i,n in enumerate(shape.nodes) if shape.names[n.name].lower()=='mountpoint'),np.eye(4))
            target=parent_transform@np.linalg.inv(mount)
            a=target[:3,:3];t=target[:3,3]
        detail=max((d for d in shape.detail_levels if d.size>0 and d.subshape>=0),key=lambda d:d.size)
        sub=shape.subshapes[detail.subshape]
        first=len(self.collision)
        for oi in range(sub.firstObject,sub.firstObject+sub.numObjects):
            ob=shape.objects[oi]
            if shape.objectstates[oi].vis<=0.0: continue
            if detail.objectDetail>=ob.numMeshes: continue
            mesh=shape.meshes[ob.firstMesh+detail.objectDetail]
            if not mesh.verts: continue
            mat=nodes[ob.node] if ob.node>=0 else np.eye(4)
            aa=a@mat[:3,:3];tt=a@mat[:3,3]+t
            nn=np.linalg.inv(aa).T
            for prim in mesh.primitives:
                material=shape.materials[prim.type & 0x0fffffff]
                layer=self.texture(material.name)
                ids=mesh.indices[prim.firstElement:prim.firstElement+prim.numElements] if prim.type & 0x20000000 else list(range(prim.firstElement,prim.firstElement+prim.numElements))
                ids=[i & 0xffff for i in ids]
                kind=prim.type & 0xc0000000
                tris=([ids[i-2:i+1] for i in range(2,len(ids))] if kind==0x40000000 else
                      [[ids[0],ids[i-1],ids[i]] for i in range(2,len(ids))] if kind==0x80000000 else
                      [ids[i:i+3] for i in range(0,len(ids),3)])
                for tri in tris:
                    ns=[direction(nn@mesh.normals[i]) for i in tri]
                    ns=[n/max(np.linalg.norm(n),1e-9) for n in ns]
                    # Alpha-cut foliage is visual only; solid meshes collide.
                    self.triangle([point(aa@mesh.verts[i]+tt) for i in tri],ns,
                                  [mesh.tverts[i] for i in tri],layer,
                                  collide=not(material.flags & 4))
        self.instances.append(dict(kind=obj['kind'],name=name,first=first,count=len(self.collision)-first,fields=obj['fields']))
        mounts={}
        for i,node in enumerate(shape.nodes):
            world=np.eye(4);world[:3,:3]=a;world[:3,3]=t
            mounts[shape.names[node.name].lower()]=world@nodes[i]
        return mounts

    def run(self, output, mission_name='Raindance_nef.mis'):
        if output.exists(): raise ValueError('Refusing to overwrite an existing pack')
        broadside=mission_name=='Broadside_nef.mis'
        mission=self.resolve(mission_name)
        items=objects(mission.read_text())
        by_kind=lambda k:[o for o in items if o['kind']==k]
        terrain=by_kind('TerrainBlock')[0]['fields']
        data=self.resolve(terrain['terrainFile']).read_bytes()
        if data[0]!=3: raise ValueError('Expected T2 terrain v3')
        heights=data[1:131073]; offset=196609; names=[]
        for _ in range(8):
            n=data[offset];offset+=1;names.append(data[offset:offset+n].decode());offset+=n
        names=[n for n in names if n]
        if len(names)!=4: raise ValueError('Expected four terrain layers')
        terrain_layers=[self.texture(n) for n in names]
        weights=np.frombuffer(data[offset:offset+4*65536],dtype=np.uint8).reshape(4,256,256).transpose(1,2,0).copy()
        # Mission emptySquares is run-length encoding: low 16 bits = start,
        # high 16 bits = run length. Preserve underground entrance cutouts.
        holes=[]
        for value in terrain.get('emptySquares','').split():
            n=int(value);holes.extend(range(n&65535,(n&65535)+(n>>16)))
        for obj in by_kind('InteriorInstance'): self.interior(obj)
        for obj in by_kind('TSStatic'): self.shape(obj,obj['fields']['shapeName'])
        equipment={'StationInventory':'station_inv_human.dts','StationVehiclePad':'vehicle_pad.dts',
                   'SensorLargePulse':'sensor_pulse_large.dts','GeneratorLarge':'station_generator_large.dts',
                   'TurretBaseLarge':'turret_base_large.dts','RepairPack':'pack_upgrade_repair.dts'}
        if broadside: equipment.update(InteriorFlagStand='int_flagstand.dts',SentryTurret='turret_sentry.dts')
        for obj in by_kind('StaticShape')+by_kind('Turret')+by_kind('Item'):
            block=obj['fields']['dataBlock']
            if block in equipment:
                mounts=self.shape(obj,equipment[block])
                if obj['kind']=='Turret' and block=='TurretBaseLarge':
                    barrel={'PlasmaBarrelLarge':'turret_fusion_large.dts','MissileBarrelLarge':'turret_missile_large.dts'}[obj['fields']['initialBarrel']]
                    self.shape(obj,barrel,mounts['mount0'])
        flags=[None,None];spawns=[None,None]
        for obj in items:
            team=0 if 'Team2' in obj['parents'] else 1 if 'Team1' in obj['parents'] else None
            if team is None:continue
            if obj['kind']=='Item' and obj['fields']['dataBlock']=='FLAG':flags[team]=point(vec(obj['fields']['position'])).tolist()
            if obj['kind']=='SpawnSphere':spawns[team]=point(vec(obj['fields']['position'])).tolist()
        sky=by_kind('Sky')[0]['fields']
        skylist=self.resolve(sky['materialList']).read_text().splitlines()
        sky_layers=[self.texture(n.strip()) for n in skylist if n.strip()][:6]
        waters=by_kind('WaterBlock')
        water=waters[0]['fields'] if waters else dict(position='0 0 -900',scale='1 1 1')
        water_layer=self.texture(water['surfaceTexture']) if waters else 0
        emitters=by_kind('AudioEmitter')
        samples=np.zeros(1,dtype='<f4')
        if broadside:
            # The runtime has one ambient stream, not a per-emitter sound bank.
            # Do not replace the different bird/engine sounds with a wrong loop.
            emitters=[]
        else:
            if len({o['fields']['fileName'] for o in emitters})!=1:raise ValueError('Expected shared Raindance ambience')
            with wave.open(str(self.resolve(emitters[0]['fields']['fileName']))) as wav:
                if wav.getnchannels()!=1 or wav.getsampwidth()!=2:raise ValueError('Expected PCM16 mono ambience')
                samples=np.frombuffer(wav.readframes(wav.getnframes()),dtype='<i2').astype(float)/32768
                samples=np.interp(np.arange(round(len(samples)*44100/wav.getframerate()))*wav.getframerate()/44100,
                                  np.arange(len(samples)),samples).astype('<f4')
        bases=[]
        if broadside:
            for obj in by_kind('InteriorInstance'):
                if obj['fields']['interiorFile']!='dbase_broadside_nef.dif':continue
                a,t=transform(obj['fields'])
                team=0 if 'Team2' in obj['parents'] else 1
                spawns[team]=point(a@np.array([20.,-45.,1.2])+t).tolist()
                # Matrix maps engine XYZ delta to original DIF XYZ (Z-up).
                bases.append(dict(name=f'Base {team+1}',position=point(t).tolist(),
                                  world_to_local=(np.linalg.inv(a)@np.array([[1,0,0],[0,0,1],[0,1,0]])).tolist()))
        output.mkdir(parents=True,exist_ok=False)
        np.asarray(self.vertices,dtype='<f4').tofile(output/'vertices.bin')
        np.asarray(self.collision,dtype='<f4').tofile(output/'collision.bin')
        (output/'height.bin').write_bytes(heights)
        (output/'weights.rgba').write_bytes(weights.tobytes())
        (output/'ambient.f32').write_bytes(samples.tobytes())
        (output/'textures.rgba').write_bytes(b''.join(
            img.resize((side,side),Image.Resampling.BOX).tobytes()
            for side in [256,128,64,32,16,8,4,2,1] for img in self.images))
        manifest=dict(version=1,name='Broadside — private T2 reference' if broadside else 'Raindance — T2 Classic',flags=flags,spawns=spawns,holes=holes,
                      terrain_layers=terrain_layers,sky_layers=sky_layers,water_layer=water_layer,
                      texture_count=len(self.images),sky=sky,water=water,
                      ambient_emitters=[[*point(vec(o['fields']['position'])),float(o['fields']['volume']),
                                         float(o['fields']['minDistance']),float(o['fields']['maxDistance'])] for o in emitters],
                      sun=by_kind('Sun')[0]['fields'],mission_area=by_kind('MissionArea')[0]['fields'],
                      objects=items,instances=self.instances,sources=self.dependencies,
                      limitations=['Equipment is static: no turret AI, power grid, inventory UI, vehicle spawning, repair pickups, or DTS animations.',
                                   'Original AI navigation is not converted; PeakRunner bots retain their existing steering. Legacy animated decal meshes are omitted.',
                                   'Water appearance imported; original swimming/drag physics are not implemented.',
                                   'Textures normalized to 256px; original interior baked lighting retained.'])
        if broadside:
            manifest.update(private_reference=True,exact_spawns=True,reference_bases=bases,
                            terrain_step=float(terrain.get('squareSize','8')),water_enabled=bool(waters))
            manifest['limitations'] += ['Ambient bird/engine emitters are recorded but silent; multi-stream ambience is not implemented.',
                'Terrain uses the declared squareSize and raw heights; legacy TerrainBlock Z translation is not applied.',
                'Dynamic flags, player characters, weapons, physics and HUD remain PeakRunner; this is not the Tribes 2 executable.']
        manifest['files']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(output.iterdir())}
        (output/'map.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(json.dumps(dict(objects=Counter(o['kind'] for o in items),instances=len(self.instances),
                             triangles=len(self.vertices)//3,collision_triangles=len(self.collision),textures=len(self.images),output=str(output)),indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,default=Path('local-assets/tribes-map-catalog/tribes2'))
    parser.add_argument('--tools',type=Path,default=Path('local-assets/tools'))
    parser.add_argument('--output',type=Path,default=Path('local-assets/raindance'))
    parser.add_argument('--mission',choices=['Raindance_nef.mis','Broadside_nef.mis'],default='Raindance_nef.mis')
    args=parser.parse_args()
    Converter(args.source,args.tools).run(args.output,args.mission)
