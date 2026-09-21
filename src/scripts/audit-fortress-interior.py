#!/usr/bin/env python3
"""Private, reproducible two-sided collision-ray audit; never exports geometry.

All coordinates in reports are base-local [right, forward, height] metres.
Run with the local reference Python (numpy). Reports remain in local-assets.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parent.parent

def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT/'scripts'/filename)
    mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
    return mod

def ray(tris, origin, direction, limit=150):
    """Double-sided Moller-Trumbore; nearest positive hit, with no hull inference."""
    origin=np.asarray(origin,dtype=float); direction=np.asarray(direction,dtype=float)
    direction=direction/np.linalg.norm(direction)
    e1=tris[:,1]-tris[:,0]; e2=tris[:,2]-tris[:,0]
    h=np.cross(direction,e2); det=np.einsum('ij,ij->i',e1,h)
    valid=np.abs(det)>1e-9
    inv=np.zeros_like(det); inv[valid]=1/det[valid]
    s=origin-tris[:,0]; u=inv*np.einsum('ij,ij->i',s,h)
    q=np.cross(s,e1); v=inv*(q@direction)
    t=inv*np.einsum('ij,ij->i',e2,q)
    valid &= (u>=-1e-7)&(v>=-1e-7)&(u+v<=1+1e-7)&(t>1e-4)&(t<=limit)
    return float(t[valid].min()) if valid.any() else None

PROBES = [('entry', [0,-29,2]), ('lower_inventory',[0,2,-4]),
          ('hall',[0,10,9]), ('flag',[0,-12,16]), ('gallery',[-15,18,16]),
          ('armory',[0,-12,27]), ('spine',[6,12,33]),
          ('generator',[8,15,40]), ('defence',[8,0,47]), ('roof',[8,0,59])]
DIRECTIONS={'left':[-1,0,0],'right':[1,0,0],'back':[0,-1,0],
            'front':[0,1,0],'floor':[0,0,-1],'ceiling':[0,0,1]}

def measurements(tris):
    return [dict(name=name,point=point,distances={key:ray(tris,point,d)
            for key,d in DIRECTIONS.items()}) for name,point in PROBES]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(); out=args.output.resolve()
    if not out.is_relative_to(ROOT/'local-assets') or out.exists():
        parser.error('output must be a NEW directory under ignored local-assets')
    ref=module('reference_measure','measure-broadside.py')
    own=module('own_measure','inspect-skybreak-fortress.py')
    pack=ROOT/'local-assets/broadside-reference'
    manifest=json.loads((pack/'map.json').read_text())
    authored=ROOT/'assets/maps/skybreak-bastions'
    own_manifest=json.loads((authored/'map.json').read_text())
    placements=json.loads((ROOT/'maps/skybreak-bastions.json').read_text())['bases']
    report={'units':'metres; local [right, forward, height]',
            'method':'nearest double-sided ray hit; includes static equipment; no room-volume or traversability inference',
            'sha256':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in [pack/'collision.bin',pack/'map.json',authored/'collision.bin',authored/'map.json']},
            'bases':[]}
    for index,base in enumerate(placements):
        rt,_=ref.load_local_triangles(pack,index)
        ot=own.load_triangles(authored,base['position'],base['yaw'])
        rows=[]
        for r,o in zip(measurements(rt),measurements(ot)):
            rows.append(dict(name=r['name'],point=r['point'],reference=r['distances'],
                authored=o['distances'],delta={k:None if r['distances'][k] is None or o['distances'][k] is None
                else o['distances'][k]-r['distances'][k] for k in DIRECTIONS}))
        # Corridor cross-section samples at identical local coordinates.
        corridor=[]
        for z in [-29,-20,-12,-2,2,6,14,22]:
            # Probe from just above the expected corridor slope.
            h=0 if z<-20 else -(z+20)/3 if z<-2 else -6 if z<6 else -6+(z-6)*.375
            p=[0,z,h+1.2]
            corridor.append(dict(point=p,reference={k:ray(rt,p,d) for k,d in DIRECTIONS.items()},
                                  authored={k:ray(ot,p,d) for k,d in DIRECTIONS.items()}))
        report['bases'].append(dict(index=index,probes=rows,corridor=corridor))
    # Equipment mission positions -> engine -> source base-local, not raw
    # source XYZ minus an engine origin (different axes and map offset).
    report['reference_equipment']=[]
    for obj in manifest['objects']:
        data=obj.get('fields',obj)
        block=data.get('dataBlock','')
        if block not in ['StationInventory','GeneratorLarge','SentryTurret','SensorLargePulse','FLAG']:
            continue
        p=np.array([float(v) for v in data['position'].split()])
        engine=np.array([p[0]+1024,p[2],p[1]+1024])
        idx=min(range(len(manifest['reference_bases'])),key=lambda i:np.linalg.norm(engine-manifest['reference_bases'][i]['position']))
        b=manifest['reference_bases'][idx]
        local=np.array(b['world_to_local'])@(engine-np.array(b['position']))
        report['reference_equipment'].append(dict(base=idx,kind=block,position=local.tolist()))
    report['authored_equipment']=own_manifest['entities']
    out.mkdir(parents=True); (out/'audit.json').write_text(json.dumps(report,indent=2)+'\n')
    for row in report['bases'][0]['probes']:
        print(row['name'], 'ref', {k:None if v is None else round(v,2) for k,v in row['reference'].items()},
              'own', {k:None if v is None else round(v,2) for k,v in row['authored'].items()})
    print('equipment',json.dumps(report['reference_equipment']))
    print(out/'audit.json')

if __name__=='__main__':main()
