"""Independent static DTS v19–23 reader with three-stream guard validation.

Format research: OpenMBU tsShape.cpp, tsMesh.cpp, tsShapeAlloc.cpp.
Standard/sorted meshes and decal metadata are decoded in the default pose.
Skin meshes are unsupported and fail loudly; animation metadata is not played.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
from interior_file import Reader, InteriorError


def decode(data):
    r = Reader(data)
    version = r.one('I')
    version &= 255
    if version not in (19,20,21,22,23):
        raise InteriorError('Only DTS v19–23 supported')
    size, u16, u8 = r.values('III')
    if not 0 <= u16 <= u8 <= size <= 32_000_000:
        raise InteriorError('Invalid DTS stream offsets')
    raw = r.raw(size*4)
    a, b, c = Reader(raw[:u16*4]), Reader(raw[u16*4:u8*4]), Reader(raw[u8*4:])
    guard_id = 0
    def guard():
        nonlocal guard_id
        actual = (a.one('I'), b.one('H'), c.one('B'))
        expected = (guard_id, guard_id & 65535, guard_id & 255)
        if actual != expected:
            raise InteriorError(f'DTS guard {guard_id}: {actual} != {expected}')
        guard_id += 1
    counts = a.values('15I' if version == 23 else '16I' if version == 22 else '12I')
    if version == 23: counts.append(0)
    if version < 22:
        ntrans = counts[5]-counts[0]
        if ntrans < 0: raise InteriorError('Invalid legacy transform count')
        counts = counts[:5]+[ntrans,ntrans,0,0,0]+counts[6:]
    (nn,no,nd,ns,ni,nr,nt,nu,na,narb,nos,nds,ntr,ndl,nm,nskin) = counts
    if max(counts) > 100000 or nskin:
        raise InteriorError(f'Unsupported skins or oversized shape: {counts}')
    nname, smallest, smallest_dl = a.values('III')
    if nname > 100000:
        raise InteriorError('Excessive names')
    guard()
    bounds = a.values('11f'); guard()
    nodes = [a.values('5i') for _ in range(nn)]; guard()
    objects = [a.values('6i') for _ in range(no)]; guard()
    decals=[a.values('5i') for _ in range(nd)];guard()
    ifl=[a.values('5i') for _ in range(ni)];guard()
    first = a.values(f'{ns*3}i'); guard()
    nums = a.values(f'{ns*3}i'); guard()
    rotations = [b.values('4h') for _ in range(nn)]
    translations = [a.values('3f') for _ in range(nn)]
    node_translations=[a.values('3f') for _ in range(nt)]
    node_rotations=[b.values('4h') for _ in range(nr)]; guard()
    scales={}
    if version >= 22:
        scales={'uniform':a.values(f'{nu}f'),'aligned':[a.values('3f') for _ in range(na)],
                'arbitrary':[a.values('3f') for _ in range(narb)],
                'rotations':[b.values('4h') for _ in range(narb)]}; guard()
    states = [a.values('fii') for _ in range(nos)]; guard()
    a.raw(nds*4); guard()
    a.raw(ntr*8); guard()
    details = [a.values('iiifffi') for _ in range(ndl)]; guard()
    meshes = []
    for mesh_id in range(nm):
        kind = a.one('I')
        if kind == 4:
            meshes.append(None); continue
        if kind == 2:
            if version<20: guard();a.raw(60)
            n=a.one('I');spans=[b.values('HH') for _ in range(n)];mat=a.values(f'{n}I')
            n=a.one('I');idx=b.values(f'{n}H')
            if version<20: a.raw(12);guard()
            n=a.one('I');starts=a.values(f'{n}I');texs=a.values(f'{n*4}f');text=a.values(f'{n*4}f')
            material=a.one('I');guard()
            meshes.append(dict(kind=2,primitives=[[*s,m] for s,m in zip(spans,mat)],indices=idx,
                               starts=starts,texgen_s=texs,texgen_t=text,material=material))
            continue
        if kind not in (0,3):
            raise InteriorError(f'Unsupported mesh type {kind}')
        guard()
        frames, matframes, parent = a.values('iii')
        mesh_bounds = a.values('10f')
        if parent >= mesh_id or parent < -1:
            raise InteriorError('Invalid shared mesh parent')
        parent_mesh = meshes[parent] if parent >= 0 else None
        def shared(key, width):
            count = a.one('I')
            if count > 1_000_000:
                raise InteriorError('Oversized mesh')
            if parent_mesh:
                if count > len(parent_mesh[key]):
                    raise InteriorError('Shared mesh data overflow')
                return parent_mesh[key][:count]
            return [a.values(f'{width}f') for _ in range(count)]
        points = shared('points', 3)
        uv = shared('uv', 2)
        normals = parent_mesh['normals'][:len(points)] if parent_mesh else [a.values('3f') for _ in points]
        if not parent_mesh and version >= 22:
            c.raw(len(points))  # encoded normals; explicit float normals retained
        nprim = a.one('I')
        if nprim > 1_000_000:
            raise InteriorError('Oversized primitive table')
        spans = [b.values('HH') for _ in range(nprim)]
        materials = a.values(f'{nprim}I')
        nidx = a.one('I')
        if nidx > 1_000_000:
            raise InteriorError('Oversized mesh indices')
        indices = b.values(f'{nidx}H')
        nmerge = a.one('I'); b.raw(nmerge*2)
        perframe, flags = a.values('II'); guard()
        sorted_data = None
        if kind == 3:
            # Leaf clusters can contain uninitialized plane floats (including
            # NaN). Preserve their exact 32-bit words, not invalid JSON floats.
            sorted_data = {'cluster_words': a.array('8I')}
            for key in ('start_cluster','first_verts','num_verts','first_tverts'):
                sorted_data[key] = a.indices('i')
            sorted_data['always_write_depth'] = a.one('I')
            guard()
        meshes.append(dict(kind=kind, sorted_data=sorted_data, frames=frames, matframes=matframes, parent=parent,
                           bounds=mesh_bounds, points=points, uv=uv, normals=normals,
                           primitives=[[*span,m] for span,m in zip(spans,materials)],
                           indices=indices, perframe=perframe, flags=flags))
    guard()
    names = []
    for _ in range(nname):
        name = bytearray()
        while True:
            v = c.one('B')
            if not v: break
            name.append(v)
            if len(name)>4096: raise InteriorError('Oversized shape name')
        names.append(name.decode('latin-1'))
    guard()
    if version<23:
        a.raw(ndl*8); guard()  # pre-v23 empty skin detail arrays
        guard()  # end of empty legacy skin list
    # Alignment padding belongs to each packed stream, not to individual records.
    if len(a.data)-a.offset > 3 or len(b.data)-b.offset > 3 or len(c.data)-c.offset > 3:
        raise InteriorError(f'Unexpected remaining shape stream data: {[(len(s.data)-s.offset,s.data[s.offset:s.offset+24].hex()) for s in (a,b,c)]}')
    sequence_count = r.one('I')
    if sequence_count>10000: raise InteriorError('Too many animation sequences')
    sequences=[]
    for _ in range(sequence_count):
        seq={'name':r.one('i'),'flags':r.one('I') if version>21 else 0,
             'keyframes':r.one('i'),'duration':r.one('f')}
        if version<22: seq['legacy_flags']=r.values('3B')
        seq['priority'],seq['first_ground'],seq['ground_count']=r.values('3i')
        seq['bases']=r.values('5i' if version>21 else '3i')
        seq['first_trigger'],seq['trigger_count'],seq['tool_begin']=r.values('iif')
        seq['membership']=[]
        for _ in range(8 if version>21 else 6):
            unused=r.one('I');count=r.one('I')
            if count>1024: raise InteriorError('Oversized animation membership')
            seq['membership'].append(r.values(f'{count}I'))
        sequences.append(seq)
    if r.one('B') != 1:
        raise InteriorError('Unsupported shape material list')
    nmat, _ = r.count()
    materials = [r.raw(r.one('B')).decode('latin-1') for _ in range(nmat)]
    material_flags = r.values(f'{nmat}I')
    material_maps = [r.values(f'{nmat}I') for _ in range(3)]
    detail_scales = r.values(f'{nmat}f')
    reflection = r.values(f'{nmat}f') if version > 20 else [1.0]*nmat
    if r.offset != len(data):
        raise InteriorError('Unexpected DTS trailing bytes')
    d = dict(schema=1, source_sha256=hashlib.sha256(data).hexdigest(),
             bounds=bounds, nodes=nodes, objects=objects, rotations=rotations,
             translations=translations, details=details, meshes=meshes, names=names,
             materials=materials, material_flags=material_flags, object_states=states,
             material_maps=material_maps, detail_scales=detail_scales, reflection=reflection,
             sequences=sequences,node_translations=node_translations,node_rotations=node_rotations,scales=scales,ifl=ifl,decals=decals,
             subshape_first=first, subshape_counts=nums, guard_count=guard_id)
    # Validate all primitives, including lower-detail meshes, before selecting a LOD.
    for mesh in meshes:
        if mesh and mesh['kind'] != 2:
            list(mesh_triangles(mesh, len(materials)))
    return d


def mesh_triangles(mesh, material_count):
    for start, count, flags in mesh['primitives']:
        if not flags & 0x20000000 or start+count > len(mesh['indices']):
            raise InteriorError('Nonindexed primitive or invalid span')
        material = flags & 0xfffffff
        if not flags & 0x10000000 and material >= material_count:
            raise InteriorError('Invalid mesh material')
        ids = mesh['indices'][start:start+count]
        kind = flags >> 30
        if kind == 0:
            if count % 3: raise InteriorError('Incomplete triangle list')
            triples = [ids[i:i+3] for i in range(0,count,3)]
        elif kind == 1:
            triples = [[ids[i],ids[i+1],ids[i+2]] if i%2==0 else
                       [ids[i+1],ids[i],ids[i+2]] for i in range(count-2)]
        elif kind == 2:
            triples = [[ids[0],ids[i],ids[i+1]] for i in range(1,count-1)]
        else:
            raise InteriorError('Unsupported primitive type')
        for tri in triples:
            if any(i >= len(mesh['points']) for i in tri):
                raise InteriorError('Invalid mesh vertex index')
            if len(set(tri)) < 3: continue
            yield tri, material, bool(flags & 0x10000000)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path); p.add_argument('output',type=Path)
    args=p.parse_args()
    if not args.output.resolve().is_relative_to((Path(__file__).resolve().parents[2]/'local-assets').resolve()):
        p.error('Use private local-assets output')
    if args.output.exists(): p.error('Refusing overwrite')
    d=decode(args.source.read_bytes())
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(d,allow_nan=False)+'\n')
    print(json.dumps({k:d[k] for k in ('materials','names','objects','details','guard_count')},indent=2))
