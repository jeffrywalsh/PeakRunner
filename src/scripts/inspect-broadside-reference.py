#!/usr/bin/env python3
"""Decode a user-owned DIF into PRIVATE diagnostic sections, never a game pack.

Requires the pinned local io_dif reader, NumPy/Pillow environment documented in
docs/raindance-import.md. Does not execute mission scripts or import textures.
Output must be a new directory beneath ignored local-assets/.
"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import sys
import numpy as np
from PIL import Image, ImageDraw

ROOT=Path(__file__).resolve().parent.parent

def decode(path):
    sys.path.insert(0,str(ROOT/'local-assets/tools/io_dif/blender_plugin/io_dif'))
    from hxDif import Dif
    interior=Dif.Load(str(path)).interiors[0]
    result=[]
    for surface in interior.surfaces:
        material=interior.materialList[surface.textureIndex]
        if material.rsplit('/',1)[-1].upper() in ('NULL','ORIGIN','TRIGGER','FORCEFIELD'): continue
        indices=interior.windings[surface.windingStart:surface.windingStart+surface.windingCount]
        normal=interior.normals[interior.planes[surface.planeIndex & 0x7fff].normalIndex]
        normal=np.array([normal.x,normal.y,normal.z])*(-1 if surface.planeFlipped else 1)
        for j in range(2,len(indices)):
            points=[interior.points[indices[k]] for k in (j-2,j-1,j)]
            tri=np.array([[p.x,p.y,p.z] for p in points])
            if np.linalg.norm(np.cross(tri[1]-tri[0],tri[2]-tri[0]))<1e-6: continue
            result.append((tri,normal,material))
    return interior,result

def section(tri,height,axis):
    points=[]
    for a,b in zip(tri,np.roll(tri,-1,axis=0)):
        if (a[axis]<=height<b[axis]) or (b[axis]<=height<a[axis]):
            points.append(a+(b-a)*(height-a[axis])/(b[axis]-a[axis]))
    return points

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dif',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to(ROOT/'local-assets') or out.exists():
        parser.error('Output must be a NEW directory under ignored local-assets/')
    interior,triangles=decode(args.dif)
    out.mkdir(parents=True)
    floors=defaultdict(float)
    for tri,n,_ in triangles:
        if n[2]>.98 and np.ptp(tri[:,2])<.02:
            floors[round(float(tri[0,2]),2)]+=float(np.linalg.norm(np.cross(tri[1]-tri[0],tri[2]-tri[0]))/2)
    points=np.array([[p.x,p.y,p.z] for p in interior.points])
    report=dict(source_sha256=hashlib.sha256(args.dif.read_bytes()).hexdigest(),
                coordinates='DIF local X/Y horizontal, Z up, metres',
                points=len(points),surfaces=len(interior.surfaces),triangles=len(triangles),
                bounds=[points.min(axis=0).tolist(),points.max(axis=0).tolist()],
                floors=sorted(floors.items(),key=lambda item:-item[1]),
                warning='PRIVATE source-game diagnostic; not a distributable original asset')
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    heights=[-6,0,7,14,25,31,38,44,51,57]
    sheet=Image.new('RGB',(1500,2400),'#101820'); draw=ImageDraw.Draw(sheet)
    for index,h in enumerate(heights):
        ox=(index%3)*500; oy=(index//3)*600
        def p(v): return (ox+250+v[0]*4.2,oy+300-v[1]*4.2)
        draw.text((ox+15,oy+15),f'Floor Z={h} m; wall cut at +2 m',fill='white')
        for tri,n,_ in triangles:
            if n[2]>.5 and tri[:,2].min()>=h-.2 and tri[:,2].max()<=h+.2:
                draw.polygon([p(v) for v in tri],fill='#405971')
            line=section(tri,h+2,2)
            if len(line)==2: draw.line([p(v) for v in line],fill='#ffe19b',width=2)
        for value in range(-40,41,10):
            draw.text(p((value,0)),str(value),fill='#6ccbd5')
        draw.text((ox+20,oy+570),'X right; +Y up. 10 m = 42 pixels',fill='#99acba')
    sheet.save(out/'floor-sections.png')
    side=Image.new('RGB',(1600,1000),'#101820');draw=ImageDraw.Draw(side)
    for index,(axis,cut) in enumerate([(0,0),(1,0)]):
        def p(v):return (index*800+400+v[1-axis]*5,440-v[2]*5)
        draw.text((index*800+20,20),f'Central section {"X" if axis==0 else "Y"}=0',fill='white')
        for tri,_,_ in triangles:
            line=section(tri,cut+.01,axis)
            if len(line)==2:draw.line([p(v) for v in line],fill='#ffe19b',width=2)
        for h in range(-60,71,10):draw.text((index*800+10,440-h*5),str(h),fill='#6ccbd5')
    side.save(out/'vertical-sections.png')
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
