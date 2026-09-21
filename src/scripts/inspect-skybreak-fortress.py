#!/usr/bin/env python3
"""Render the same kind of floor-plan/profile cuts as inspect-broadside-inengine.py,
but for OUR OWN compiled Skybreak pack, so the two can be compared side by side.
Not a private diagnostic (this is our original asset); output can go anywhere.
"""
import argparse
import json
import math
from pathlib import Path
import struct
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent


def load_triangles(pack_dir, origin, yaw_deg):
    raw = (pack_dir / 'collision.bin').read_bytes()
    floats = struct.unpack(f'<{len(raw)//4}f', raw)
    pts = np.array(floats).reshape(-1, 3)
    ox, oy, oz = origin
    yaw = math.radians(yaw_deg)
    c, s = math.cos(yaw), math.sin(yaw)
    dx, dy, dz = pts[:, 0]-ox, pts[:, 1]-oy, pts[:, 2]-oz
    lx = dx*c - dz*s
    lz = dx*s + dz*c
    local = np.stack([lx, lz, dy], axis=1)
    tris = local.reshape(-1, 3, 3)
    centroid = tris.mean(axis=1)
    near = (np.abs(centroid[:, 0]) < 70) & (np.abs(centroid[:, 1]) < 70) & (centroid[:, 2] > -80) & (centroid[:, 2] < 90)
    return tris[near]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--pack', type=Path, default=ROOT/'assets/maps/skybreak-bastions')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    spec = json.loads((ROOT/'maps/skybreak-bastions.json').read_text())
    base = spec['bases'][0]
    tris = load_triangles(args.pack, base['position'], base['yaw'])
    print(f'{len(tris)} collision triangles near local origin')
    flat = tris.reshape(-1, 3)
    print('local bounds (x,y,height)', flat.min(axis=0), flat.max(axis=0))

    args.output.mkdir(parents=True, exist_ok=True)
    heights = [-8, 0, 7, 14, 21, 28, 35, 42, 49, 56, 63]
    cols, sheet_rows = 3, 4
    per_sheet = cols*sheet_rows
    for sheet_i in range(0, len(heights), per_sheet):
        batch = heights[sheet_i:sheet_i+per_sheet]
        sheet = Image.new('RGB', (1500, 600*((len(batch)+cols-1)//cols)), '#101820')
        draw = ImageDraw.Draw(sheet)
        for index, h in enumerate(batch):
            ox = (index % cols)*500
            oy = (index//cols)*600

            def p(v, ox=ox, oy=oy):
                return (ox+250+v[0]*3.2, oy+300-v[1]*3.2)
            draw.text((ox+15, oy+15), f'Local height={h} m; wall cut at +2 m', fill='white')
            for tri in tris:
                n = np.cross(tri[1]-tri[0], tri[2]-tri[0])
                norm = np.linalg.norm(n)
                if norm < 1e-9:
                    continue
                n = n/norm
                if abs(n[2]) > 0.5 and tri[:, 2].min() >= h-.25 and tri[:, 2].max() <= h+.25:
                    draw.polygon([p(v) for v in tri], fill='#405971')
                cut = h+2
                pts2 = []
                for a, b in zip(tri, np.roll(tri, -1, axis=0)):
                    if (a[2] <= cut < b[2]) or (b[2] <= cut < a[2]):
                        t = (cut-a[2])/(b[2]-a[2])
                        pts2.append(a+(b-a)*t)
                if len(pts2) == 2:
                    draw.line([p(v) for v in pts2], fill='#ffe19b', width=2)
            for value in range(-40, 41, 10):
                draw.text(p((value, 0, 0)), str(value), fill='#6ccbd5')
        sheet.save(args.output/f'floor-sections-{sheet_i}.png')

    side = Image.new('RGB', (1600, 1400), '#101820')
    draw = ImageDraw.Draw(side)
    for index, (axis, cut) in enumerate([(0, 0), (1, 0)]):
        def p(v, index=index, axis=axis):
            horiz = v[1] if axis == 0 else v[0]
            return (index*800+400+horiz*4, 950-v[2]*6)
        draw.text((index*800+20, 20), f'Central section {"X" if axis==0 else "Y"}=0', fill='white')
        for tri in tris:
            pts2 = []
            for a, b in zip(tri, np.roll(tri, -1, axis=0)):
                if (a[axis] <= cut+.01 < b[axis]) or (b[axis] <= cut+.01 < a[axis]):
                    t = (cut+.01-a[axis])/(b[axis]-a[axis])
                    pts2.append(a+(b-a)*t)
            if len(pts2) == 2:
                draw.line([p(v) for v in pts2], fill='#ffe19b', width=2)
        for hh in range(-90, 81, 10):
            draw.text((index*800+10, 950-hh*6), str(hh), fill='#6ccbd5')
    side.save(args.output/'vertical-sections.png')
    print('wrote', args.output)


if __name__ == '__main__':
    main()
