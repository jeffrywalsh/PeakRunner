"""Compile a private Stonehenge scene using only the fresh native readers.

Source geometry and textures are source-derived, not independently authored art.
Native scripts are never executed. Output retains editable mission and meshes.
"""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import struct
import shutil
import numpy as np
from PIL import Image
import interior_file
import shape_file
import terrain_file
import mission

# Literal shapeFile values inspected in the source station/staticShape/turret and
# repairPack/sentryTurret datablocks. No source scripts are evaluated.
EQUIPMENT_SHAPES={'StationInventory':'station_inv_human.dts',
                  'GeneratorLarge':'station_generator_large.dts','SensorLargePulse':'sensor_pulse_large.dts',
                  'TurretBaseLarge':'turret_base_large.dts','SentryTurret':'turret_sentry.dts',
                  'RepairPack':'pack_upgrade_repair.dts'}

PROFILES = {
    'stonehenge': ('Stonehenge_nef', 'Stonehenge Clone'),
    'snowblind': ('Snowblind_nef', 'Snowblind Clone'),
    'desert-of-death': ('DesertofDeath_nef', 'Desert of Death Clone'),
}


def donut_wall(d, flag_local, size):
    """Choose a vertical triangle with room for the whole poster, near a flag.

    Requiring every corner inside one source triangle avoids spanning doorways.
    The placement is exported and visually checked before installation.
    """
    candidates=[]
    for tri in interior_file.triangles(d):
        n=np.array(tri['corners'][0]['normal'])
        if abs(n[2])>.01: continue
        pts=np.array([c['position'] for c in tri['corners']])
        center=pts.mean(axis=0); tangent=np.cross([0,0,1],n)
        a,b=pts[1]-pts[0],pts[2]-pts[0]
        basis=np.column_stack([a,b]); inv=np.linalg.pinv(basis)
        fits=True
        for x,y in [(-1,-1),(1,-1),(1,1),(-1,1)]:
            uv=inv@(center+tangent*x*size/2+np.array([0,0,y*size/2])-pts[0])
            if min(uv)<.02 or sum(uv)>.98: fits=False
        if fits:
            score=np.linalg.norm(center-flag_local)
            candidates.append((score,center,n))
    if not candidates: raise ValueError('No source wall can contain donut poster')
    _,center,n=min(candidates,key=lambda c:c[0])
    return center,n


def json_write(path, value):
    path.write_text(json.dumps(value, allow_nan=False, separators=(',',':'))+'\n')


def rotation(axis_angle):
    axis = np.array(axis_angle[:3], dtype=float)
    angle = np.deg2rad(axis_angle[3])
    if abs(angle)<1e-10: return np.eye(3)
    length = np.linalg.norm(axis)
    if length < 1e-8: raise ValueError('Rotation needs a nonzero axis')
    x,y,z = axis/length
    # Torque AngAxis is clockwise (QuatF::setMatrix convention).
    skew = np.array([[0,-z,y],[z,0,-x],[-y,x,0]])
    return np.eye(3)+np.sin(-angle)*skew+(1-np.cos(angle))*(skew@skew)


def node_matrix(d, node):
    if node < 0: return np.eye(4)
    chain=[]
    while node >= 0:
        if node in chain or node >= len(d['nodes']): raise ValueError('Invalid node hierarchy')
        chain.append(node); node=d['nodes'][node][1]
    transform=np.eye(4)
    for n in reversed(chain):
        q=np.array(d['rotations'][n],dtype=float)/32767
        q/=np.linalg.norm(q)
        x,y,z,w=q
        rot=np.array([[1-2*(y*y+z*z),2*(x*y+z*w),2*(x*z-y*w)],
                      [2*(x*y-z*w),1-2*(x*x+z*z),2*(y*z+x*w)],
                      [2*(x*z+y*w),2*(y*z-x*w),1-2*(x*x+y*y)]])
        local=np.eye(4);local[:3,:3]=rot;local[:3,3]=d['translations'][n]
        transform=transform@local
    return transform


def transform(fields):
    floats=lambda k,default: [float(v) for v in fields.get(k,default).split()]
    m=np.eye(4)
    m[:3,:3]=rotation(floats('rotation','1 0 0 0'))@np.diag(floats('scale','1 1 1'))
    m[:3,3]=floats('position','0 0 0')
    return m


def engine(p):
    return [float(p[0]+1024),float(p[2]),float(p[1]+1024)]


class Assets:
    def __init__(self, root):
        self.root=root
        self.files=[p for p in root.rglob('*') if p.is_file()]
        self.used={}
    def find(self, reference, texture=False):
        ref=reference.replace('\\','/').lower()
        suffix=ref if not texture or ref.endswith(('.png','.jpg','.jpeg')) else ref+'.png'
        matches=[p for p in self.files if str(p).lower().endswith('/'+suffix)]
        if not matches:
            matches=[p for p in self.files if p.name.lower()==suffix.rsplit('/',1)[-1]]
        if not matches and texture:
            # Literal IFL image list, first/default frame only; never a script.
            ifls=[p for p in self.files if p.name.lower()==ref.rsplit('/',1)[-1]+'.ifl']
            if ifls:
                listing=self.find(ref+'.ifl')
                first=next((line.strip().split()[0] for line in listing.read_text().splitlines()
                            if line.strip() and not line.lstrip().startswith('//')),None)
                if not first or first.lower()==ref.rsplit('/',1)[-1]:
                    raise ValueError('Invalid IFL first frame '+ref)
                if Path(first).suffix.lower() not in ('','.png','.jpg') or '..' in first:
                    raise ValueError('Invalid IFL image path')
                return self.find(first,texture=True)
        classic=[p for p in matches if 'Classic_maps_v1' in p.parts]
        if classic: matches=classic
        if not matches: raise ValueError('Missing asset '+reference)
        hashes={hashlib.sha256(p.read_bytes()).hexdigest() for p in matches}
        if len(hashes)>1: raise ValueError('Ambiguous asset '+reference+': '+str(matches))
        path=sorted(matches)[0]
        self.used[str(path.relative_to(self.root))]=next(iter(hashes))
        return path


def material_pixels(image, opaque=False):
    rgba=image.convert('RGBA')
    # DTS opaque alpha may store environment-map strength, not transparency.
    if opaque: rgba.putalpha(255)
    return rgba


def build(root, output, editable=None, profile='stonehenge'):
    if output.exists(): raise ValueError('Refusing overwrite')
    if editable and (editable/'profile.json').exists():
        profile=json.loads((editable/'profile.json').read_text())['profile']
    mission_name,display_name=PROFILES[profile]
    assets=Assets(editable/'resources' if editable else root)
    doc=json.loads((editable/'mission.json').read_text()) if editable else mission.parse(assets.find(mission_name+'.mis').read_text())
    objects=doc['objects']
    t=terrain_file.load_editable(editable/'terrain') if editable else terrain_file.decode(assets.find(mission_name+'.ter').read_bytes())
    terrain_obj=next(o for o in objects if o['kind']=='TerrainBlock')
    if terrain_obj['fields']['position']!='-1024 -1024 0' or terrain_obj['fields']['squareSize']!='8':
        raise ValueError('Terrain placement outside current runtime contract')
    textures=[]; texture_names=[]; texture_cache={}
    def layer(image, name):
        image=image.convert('RGBA').resize((256,256),Image.Resampling.LANCZOS)
        key=hashlib.sha256(image.tobytes()).hexdigest()
        if key in texture_cache: return texture_cache[key]
        index=len(textures);textures.append(image);texture_names.append(name);texture_cache[key]=index
        return index
    def texture(ref, opaque=False):
        with Image.open(assets.find(ref,texture=True)) as im:
            return layer(material_pixels(im,opaque),ref)
    terrain_layers=[texture(n) for n in t['material_names'] if n]
    if not 1<=len(terrain_layers)<=4: raise ValueError('Runtime supports one to four terrain layers')
    while len(terrain_layers)<4: terrain_layers.append(terrain_layers[0])
    sky=next(o['fields'] for o in objects if o['kind']=='Sky')
    sky_layers=[texture(n.strip()) for n in assets.find(sky['materialList']).read_text().splitlines()[:6]]
    vertices=[]; collision=[]; mesh_records=[]; interiors={}; shapes={}; report=[]; placements=[]
    poster=None
    poster_settings=json.loads((editable/'poster.json').read_text()) if editable else {'size':2.5 if profile=='stonehenge' else 1.5,'enabled':True}
    first_flag=np.array([float(v) for v in next(o['fields']['position'] for o in objects if o['fields'].get('dataBlock')=='FLAG').split()])
    # Select the building closest to the first flag; no substituted geometry.
    target_interior=min((o for o in objects if o['kind']=='InteriorInstance'),
        key=lambda o:np.linalg.norm(transform(o['fields'])[:3,3]-first_flag))['id']
    def emit(corners, mat, lighting, matrix, solid, lm_size=None):
        points=[]; out=[]
        normal_matrix=np.linalg.inv(matrix[:3,:3]).T
        for corner in corners:
            p=engine(matrix@np.array([*corner['position'],1.]))
            n=normal_matrix@np.array(corner['normal']);length=np.linalg.norm(n)
            n=n/length if length>1e-8 else np.array([0.,0.,1.])
            lm=corner.get('light_uv',[0,0])
            # Stored light coordinates already normalize to each PNG's own size.
            out.append(p+[float(n[0]),float(n[2]),float(n[1])]+corner['uv']+lm+[mat,lighting])
            points.append(p)
        area=np.linalg.norm(np.cross(np.array(points[1])-points[0],np.array(points[2])-points[0]))
        if area<1e-8: return
        vertices.extend(out)
        if solid: collision.append(points)
    for o in objects:
        f=o['fields'];start=len(vertices)//3
        if o['kind']=='InteriorInstance':
            name=f['interiorFile']
            if Path(name).name != name: raise ValueError('Interior must be a basename')
            if name not in interiors:
                interiors[name]=json.loads((editable/(name+'.json')).read_text()) if editable else interior_file.decode(assets.find(name).read_bytes())
            d=interiors[name];m=transform(f)
            placements.append((d,m,np.linalg.inv(m)))
            mats={i:texture(d['materials'][i]) for i in {s['material'] for s in d['surfaces']}}
            lights=[]
            for lm in d['lightmaps']:
                with Image.open(io.BytesIO(base64.b64decode(lm['png']))) as im:
                    lights.append((layer(im,name+' lightmap'),im.size))
            for tri in interior_file.triangles(d):
                lm=tri['lightmap'];lighting,size=lights[lm] if lm!=255 else (-1,None)
                if lm!=255 and d['surfaces'][tri['surface']]['flags'] & 16:
                    lighting=-4-lighting  # source outside-visible faces also receive sunlight
                emit(tri['corners'],mats[tri['material']],lighting,m,True,size)
            # Preserve null collision faces too: unlike rendered surfaces these are polygon fans.
            for winding,plane,flags,count in d['null_surfaces']:
                ids=d['windings'][winding:winding+count]
                for i in range(1,len(ids)-1):
                    pts=[engine(m@np.array([*d['points'][j],1.])) for j in (ids[0],ids[i],ids[i+1])]
                    if np.linalg.norm(np.cross(np.subtract(pts[1],pts[0]),np.subtract(pts[2],pts[0])))>1e-8:
                        collision.append(pts)
            report.append({'object':o['id'],'type':'interior','source':name,'source_surfaces':len(d['surfaces']),
                           'triangles':len(vertices)//3-start})
            # Donut is an explicit small addition to a vertical source wall, not a replacement.
            if poster_settings['enabled'] and poster is None and ((profile=='stonehenge' and name=='dbunk_stonehenge1.dif') or (profile!='stonehenge' and o['id']==target_interior)):
                candidates=[]
                for s in d['surfaces']:
                    ni,_=d['planes'][s['plane']&0x7fff]
                    n=np.array(d['normals'][ni])*(-1 if s['plane']&0x8000 else 1)
                    pts=np.array([d['points'][i] for i in d['windings'][s['start']:s['start']+s['count']]])
                    span=pts.max(axis=0)-pts.min(axis=0)
                    if abs(n[2])<.01 and span[2]>4 and max(span[:2])>5:
                        center=(pts.min(axis=0)+pts.max(axis=0))/2
                        # Main upper room walls, away from narrow entrance ramps.
                        if 16<center[2]<30 and -43<center[1]<-27:
                            candidates.append((float(np.prod(sorted(span)[1:])),center,n))
                if profile=='stonehenge':
                    if not candidates: raise ValueError('No verified donut wall candidate')
                    _,center,n=max(candidates,key=lambda v:v[0])
                else:
                    if 'local_center' in poster_settings:
                        center=np.array(poster_settings['local_center']);n=np.array(poster_settings['normal'])
                    else:
                        center,n=donut_wall(d,(np.linalg.inv(m)@np.array([*first_flag,1.]))[:3],float(poster_settings['size']))
                        if profile=='desert-of-death': center[2]+=3.0  # above the flag cloth
                        poster_settings.update(local_center=center.tolist(),normal=n.tolist())
                tangent=np.cross([0,0,1],n);up=np.array([0,0,1.]);center=center+n*.025
                size=float(poster_settings['size'])
                if not .1<=size<=4: raise ValueError('Poster size outside safe wall bounds')
                half=size/2
                poster={'object':o['id'],'local_center':center.tolist(),'normal':n.tolist(),'size':size}
                img=Image.open(editable/'donut.png' if editable else Path(__file__).parent/'art/donut.png');mat=layer(img,'original donut poster')
                white=layer(Image.new('RGBA',(256,256),'white'),'poster neutral light')
                corners=[dict(position=(center+tangent*x+up*y).tolist(),normal=n.tolist(),uv=uv)
                         for x,y,uv in [(-half,-half,[0,1]),(half,-half,[1,1]),(half,half,[1,0]),(-half,half,[0,0])]]
                for ids in [(0,1,2),(0,2,3)]: emit([corners[i] for i in ids],mat,white,m,False)
                poster['camera']=engine(m@np.array([*(center+n*7-np.array([0,0,.6])),1.]))
                direction=m[:3,:3]@(-n)
                poster['heading']=[float(direction[0]),float(direction[2]),float(direction[1])]
        elif o['kind']=='TSStatic' or f.get('dataBlock') in EQUIPMENT_SHAPES:
            name=f['shapeName'] if o['kind']=='TSStatic' else EQUIPMENT_SHAPES[f['dataBlock']]
            if Path(name).name != name: raise ValueError('Shape must be a basename')
            if name not in shapes:
                shapes[name]=json.loads((editable/(name+'.json')).read_text()) if editable else shape_file.decode(assets.find(name).read_bytes())
            d=shapes[name];instance=transform(f)
            mats={}
            detail=max((v for v in d['details'] if v[3]>0),key=lambda v:v[3])
            for oi,obj in enumerate(d['objects']):
                if d['object_states'][oi][0]<=0: continue
                _,nmesh,first,node,_,_=obj
                if detail[2]>=nmesh: continue
                mesh=d['meshes'][first+detail[2]]
                if mesh is None: continue
                m=instance@node_matrix(d,node)
                for ids,mat,no_material in shape_file.mesh_triangles(mesh,len(d['materials'])):
                    if no_material: raise ValueError('Visible scenery primitive has no material')
                    if mat not in mats:
                        mats[mat]=texture(d['materials'][mat],opaque=not(d['material_flags'][mat]&4))
                    corners=[dict(position=mesh['points'][i],normal=mesh['normals'][i],uv=mesh['uv'][i]) for i in ids]
                    emit(corners,mats[mat],-1,m,False)
            for detail in (v for v in d['details'] if v[3]<0):
                for oi,obj in enumerate(d['objects']):
                    if d['object_states'][oi][0]<=0: continue
                    _,nmesh,first,node,_,_=obj
                    if detail[2]>=nmesh: continue
                    mesh=d['meshes'][first+detail[2]]
                    if mesh is None: continue
                    m=instance@node_matrix(d,node)
                    for ids,_,_ in shape_file.mesh_triangles(mesh,len(d['materials'])):
                        pts=[engine(m@np.array([*mesh['points'][i],1.])) for i in ids]
                        if np.linalg.norm(np.cross(np.subtract(pts[1],pts[0]),np.subtract(pts[2],pts[0])))>1e-8:
                            collision.append(pts)
            report.append({'object':o['id'],'type':'scenery' if o['kind']=='TSStatic' else 'equipment',
                           'source':name,'triangles':len(vertices)//3-start})
        else: continue
        mesh_records.append({'object':o['id'],'first_triangle':start,'triangle_count':len(vertices)//3-start})
    # The source engine suppresses terrain inside interior zones. Clip exact
    # terrain triangles against those BSP boundaries, then retain exterior pieces.
    holes={i for i,f in enumerate(t['material_flags']) if f & 128}
    cells=set()
    for d,m,_ in placements:
        lo=d['bounds'][:3];hi=d['bounds'][3:]
        bounds=np.array([(m@np.array([x,y,z,1.]))[:3] for x in (lo[0],hi[0]) for y in (lo[1],hi[1]) for z in (lo[2],hi[2])])
        low=np.floor((bounds.min(axis=0)[:2]+1024)/8).astype(int)
        high=np.floor((bounds.max(axis=0)[:2]+1024)/8).astype(int)
        for y in range(max(0,low[1]),min(254,high[1])+1):
            for x in range(max(0,low[0]),min(254,high[0])+1): cells.add((x,y))
    clipped_count=0
    for x,y in sorted(cells):
        if y*256+x in holes: continue
        pts=[[xx*8-1024,yy*8-1024,t['heights'][yy*256+xx]/32]
             for xx,yy in ((x,y),(x+1,y),(x,y+1),(x+1,y+1))]
        ids=[(0,1,3),(0,3,2)] if (x^y)&1==0 else [(0,1,2),(1,3,2)]
        original=[[pts[j] for j in tri] for tri in ids]
        fragments=original
        for d,m,inverse in placements:
            next_fragments=[]
            for polygon in fragments:
                local=[(inverse@np.array([*p,1.]))[:3].tolist() for p in polygon]
                # Far-away polygons never need BSP traversal.
                bb=np.array(local)
                if any(bb[:,i].max()<d['bounds'][i] or bb[:,i].min()>d['bounds'][i+3] for i in range(3)):
                    next_fragments.append(polygon);continue
                for part in interior_file.exterior_fragments(d,local):
                    next_fragments.append([(m@np.array([*p,1.]))[:3].tolist() for p in part])
            fragments=next_fragments
        def area(polygons):
            return sum(np.linalg.norm(np.cross(np.subtract(p[i],p[0]),np.subtract(p[i+1],p[0])))/2
                       for p in polygons for i in range(1,len(p)-1))
        if area(original)-area(fragments)<1e-5: continue
        holes.add(y*256+x);clipped_count+=1
        for polygon in fragments:
            for i in range(1,len(polygon)-1):
                p=[engine(v) for v in (polygon[0],polygon[i],polygon[i+1])]
                n=np.cross(np.subtract(p[1],p[0]),np.subtract(p[2],p[0]));length=np.linalg.norm(n)
                if length<1e-8: continue
                n=n/length
                if n[1]<0: n=-n
                for v in p:
                    vertices.append(v+n.tolist()+[v[0]/8,v[2]/8,(v[0]/8+.5)/256,(v[2]/8+.5)/256,0,-2])
                collision.append(p)
    def team(o):
        while o['parent'] is not None:
            o=objects[o['parent']]
            if o['name'] in ('Team1','Team2'): return int(o['name'][-1])-1
            if o['name'].lower()=='team0': return None
        raise ValueError('Equipment or flag lacks team')
    flags=[None,None];entities=[]
    kinds={'StationInventory':'inventory','GeneratorLarge':'generator','SensorLargePulse':'sensor',
           'TurretBaseLarge':'turret','SentryTurret':'turret','RepairPack':'repair'}
    for o in objects:
        f=o['fields'];block=f.get('dataBlock')
        if block=='FLAG': flags[team(o)]=engine([float(v) for v in f['position'].split()])
        if block in kinds:
            tid=team(o)
            # Neutral props retain their source model; no invented team owner.
            if tid is None: continue
            entities.append(dict(id=f'{profile}-{o["id"]}',kind=kinds[block],team=tid,circuit=f'base-{tid}',
                                 position=engine([float(v) for v in f['position'].split()]),radius=1.5,
                                 weapon='plasma' if f.get('initialBarrel')=='PlasmaBarrelLarge' else 'bullet'))
    if any(p is None for p in flags): raise ValueError('Missing flag')
    for e in entities:
        if not any(g['kind']=='generator' and g['team']==e['team'] for g in entities):
            e['circuit']='always-on'
    spawns=[[p[0]+4,p[1]+1.2,p[2]] for p in flags]
    channels=[t['alpha'].get(str(j),bytes(65536)) for j in range(4)]
    terrain_weights=bytes(v for i in range(65536) for v in (channel[i] for channel in channels))
    data={'vertices.bin':np.asarray(vertices,dtype='<f4').tobytes(),
          'collision.bin':np.asarray(collision,dtype='<f4').tobytes(),
          'height.bin':struct.pack('<65536H',*t['heights']), 'weights.rgba':terrain_weights,
          'ambient.f32':b'',
          'textures.rgba':b''.join(im.resize((256>>level,256>>level),Image.Resampling.LANCZOS).tobytes()
                                  for level in range(9) for im in textures)}
    if poster_settings['enabled'] and poster is None: raise ValueError('Donut was not placed')
    manifest=dict(version=1,id=profile+'-clone',name=display_name,private_reference=True,
                  terrain_step=8,water_enabled=False,exact_spawns=True,flags=flags,spawns=spawns,
                  holes=sorted(holes),
                  texture_count=len(textures),terrain_layers=terrain_layers,sky_layers=sky_layers,
                  water_layer=0,water={'position':'0 0 0','scale':'1 1 1'},sky=sky,
                  ambient_emitters=[],entities=entities,
                  files={k:hashlib.sha256(v).hexdigest() for k,v in data.items()})
    output.mkdir(parents=True)
    source=output/'editable';source.mkdir()
    json_write(source/'profile.json',{'profile':profile})
    json_write(source/'mission.json',doc)
    json_write(source/'texture-index.json',texture_names)
    json_write(source/'instances.json',mesh_records)
    json_write(source/'poster.json',poster_settings)
    shutil.copyfile(editable/'donut.png' if editable else Path(__file__).parent/'art/donut.png',source/'donut.png')
    terrain_file.save_editable(t,source/'terrain')
    for path in assets.used:
        if Path(path).suffix.lower() not in ('.png','.jpg','.jpeg','.dml','.ifl'): continue
        dest=source/'resources'/path;dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(assets.root/path,dest)
    for name,d in interiors.items(): json_write(source/(name+'.json'),d)
    for name,d in shapes.items(): json_write(source/(name+'.json'),d)
    for name,value in data.items(): (output/name).write_bytes(value)
    json_write(output/'map.json',manifest)
    validation=dict(source_files=assets.used,objects=report,poster=poster,
                    terrain_cells_clipped_to_source_bsp=clipped_count,
                    render_triangles=len(vertices)//3,collision_triangles=len(collision),
                    known_differences=['Equipment models use source default poses; animation, damage variants and IFL playback are not active',
                                      'Equipment gameplay and dynamic turret barrels use PeakRunner behavior',
                                      'Source navigation, scripted weather and positional ambience not imported',
                                      'Scenery is rendered at highest LOD with alpha testing; no plant collision',
                                      'Remaining DIF hull/LOD tail preserved; collision uses decoded surfaces and null faces; BSP clips terrain',
                                      'Source texture dimensions resampled to current 256-square runtime ABI'])
    validation['unimplemented_datablocks']=sorted({o['fields']['dataBlock'] for o in objects
        if o['fields'].get('dataBlock') and o['fields']['dataBlock'] not in set(EQUIPMENT_SHAPES)|{'FLAG','SpawnSphereMarker','Observer','WayPointMarker'}})
    validation['neutral_props_visual_only']=[o['id'] for o in objects
        if o['fields'].get('dataBlock') in kinds and team(o) is None]
    json_write(output/'validation.json',validation)
    print(json.dumps({k:validation[k] for k in ('render_triangles','collision_triangles','poster','known_differences')},indent=2))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--editable',action='store_true',help='Rebuild from exported JSON/PNG, without original native assets')
    p.add_argument('--profile',choices=PROFILES,default='stonehenge')
    args=p.parse_args()
    if not args.output.resolve().is_relative_to((Path(__file__).resolve().parents[2]/'local-assets').resolve()):
        p.error('Output must stay private under local-assets')
    build(args.source,args.output,args.source if args.editable else None,args.profile)
