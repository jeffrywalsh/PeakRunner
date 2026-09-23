#!/usr/bin/env python3
"""Mine wall/ramp/doorway geometry from the decoded T2 Broadside_nef DIF.

Deeper than inspect-broadside-reference.py's floor-area histogram: this
clusters actual triangles into wall segments, ramp runs and floor polygons
per level so authored geometry (formerly the retired Skybreak fortress) can be
checked against real connectivity, not just floor heights. Same rules as the
sibling script: user-owned DIF only, no source triangles/textures ever leave
this PRIVATE report, output must be a new directory under ignored local-assets/.
"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import sys
import numpy as np

ROOT = Path(__file__).resolve().parent.parent


def decode(path):
    sys.path.insert(0, str(ROOT / 'local-assets/tools/io_dif/blender_plugin/io_dif'))
    from hxDif import Dif
    interior = Dif.Load(str(path)).interiors[0]
    result = []
    for surface in interior.surfaces:
        material = interior.materialList[surface.textureIndex]
        if material.rsplit('/', 1)[-1].upper() in ('NULL', 'ORIGIN', 'TRIGGER', 'FORCEFIELD'):
            continue
        indices = interior.windings[surface.windingStart:surface.windingStart + surface.windingCount]
        normal = interior.normals[interior.planes[surface.planeIndex & 0x7fff].normalIndex]
        normal = np.array([normal.x, normal.y, normal.z]) * (-1 if surface.planeFlipped else 1)
        points = [interior.points[i] for i in indices]
        poly = np.array([[p.x, p.y, p.z] for p in points])
        if len(poly) < 3:
            continue
        result.append((poly, normal, material))
    return result


def polygon_kind(normal):
    nz = normal[2]
    if nz > 0.9:
        return 'floor'
    if nz < -0.9:
        return 'ceiling'
    if abs(nz) < 0.25:
        return 'wall'
    return 'ramp'


def round_level(z, levels, tol=1.0):
    for lv in levels:
        if abs(z - lv) <= tol:
            return lv
    return round(z * 2) / 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dif', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'local-assets') or out.exists():
        parser.error('Output must be a NEW directory under ignored local-assets/')

    polys = decode(args.dif)
    levels = [-42, -34, -18, -6, 0, 7, 14, 25, 31, 38, 45, 56, 57, 67]

    walls = []
    ramps = []
    floors = defaultdict(float)
    for poly, normal, material in polys:
        kind = polygon_kind(normal)
        zmin, zmax = poly[:, 2].min(), poly[:, 2].max()
        area = 0.0
        for i in range(1, len(poly) - 1):
            area += np.linalg.norm(np.cross(poly[i] - poly[0], poly[i + 1] - poly[0])) / 2
        if kind == 'floor':
            floors[round_level(poly[0, 2], levels)] += area
        elif kind == 'wall':
            if area < 0.4:
                continue
            xs, ys = poly[:, 0], poly[:, 1]
            walls.append(dict(
                x=[round(float(xs.min()), 2), round(float(xs.max()), 2)],
                y=[round(float(ys.min()), 2), round(float(ys.max()), 2)],
                z=[round(float(zmin), 2), round(float(zmax), 2)],
                area=round(float(area), 2),
                material=material,
            ))
        elif kind == 'ramp':
            if area < 0.4:
                continue
            xs, ys = poly[:, 0], poly[:, 1]
            horiz = max(float(xs.max() - xs.min()), float(ys.max() - ys.min()))
            rise = float(zmax - zmin)
            if horiz < 0.5 or rise < 0.5:
                continue
            ramps.append(dict(
                x=[round(float(xs.min()), 2), round(float(xs.max()), 2)],
                y=[round(float(ys.min()), 2), round(float(ys.max()), 2)],
                z=[round(float(zmin), 2), round(float(zmax), 2)],
                slope=round(rise / horiz, 3), area=round(float(area), 2),
                material=material,
            ))

    def cluster(items, keys, tol):
        clusters = []
        for item in sorted(items, key=lambda it: [it[k][0] for k in keys]):
            placed = False
            for c in clusters:
                if all(min(abs(item[k][0] - c[k][0]), abs(item[k][1] - c[k][1])) < tol for k in keys):
                    for k in keys:
                        c[k] = [min(c[k][0], item[k][0]), max(c[k][1], item[k][1])]
                    c['area'] += item['area']
                    c['count'] += 1
                    placed = True
                    break
            if not placed:
                clusters.append(dict(item, count=1))
        return clusters

    wall_clusters = cluster(walls, ['x', 'y', 'z'], 1.5)
    ramp_clusters = cluster(ramps, ['x', 'y'], 2.0)
    wall_clusters.sort(key=lambda c: -c['area'])
    ramp_clusters.sort(key=lambda c: -c['area'])

    out.mkdir(parents=True)
    report = dict(
        source_sha256=hashlib.sha256(args.dif.read_bytes()).hexdigest(),
        coordinates='DIF local X/Y horizontal, Z up, metres',
        floors=sorted(floors.items(), key=lambda kv: -kv[1]),
        wall_segments=wall_clusters,
        ramp_segments=[r for r in ramp_clusters if r['count'] >= 2],
        warning='PRIVATE source-game diagnostic; not a distributable original asset',
    )
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f"{len(wall_clusters)} wall clusters, {len(report['ramp_segments'])} ramp clusters (>=2 tris)")
    print('Top 25 wall clusters by area:')
    for c in wall_clusters[:25]:
        print(f"  x={c['x']} y={c['y']} z={c['z']} area={c['area']:.1f} n={c['count']} mat={c['material']}")
    print('Top 25 ramp clusters by area:')
    for c in report['ramp_segments'][:25]:
        print(f"  x={c['x']} y={c['y']} z={c['z']} slope={c['slope']} area={c['area']:.1f} n={c['count']}")


if __name__ == '__main__':
    main()
