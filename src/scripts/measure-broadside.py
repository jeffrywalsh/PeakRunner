#!/usr/bin/env python3
"""Approximate coplanar surface grouping of the private Broadside reference.

Unlike inspect-broadside-inengine.py (which renders pictures to eyeball),
this groups collision surfaces using mesh TOPOLOGY: triangles that share an edge
(identical vertices, from the original authored surface) are grouped into one
connected component. This does not recover original authoring panels or rooms.
Two coplanar pieces that do NOT share an edge
(e.g. a wall split by a doorway) stay separate components, so doorways and
gaps fall out of the data directly as the space between panels, rather than
being read off a picture.

Output is plain-text/JSON measurements: panel type, plane, exact 2D extent,
height range, area. Private diagnostic; output must be a new directory under
ignored local-assets/, and this never reads textures or emits renderable
geometry.
"""
import argparse
import json
from pathlib import Path
import struct
import sys
import numpy as np

ROOT = Path(__file__).resolve().parent.parent


def load_local_triangles(pack_dir, base_index=0):
    manifest = json.loads((pack_dir / 'map.json').read_text())
    base = manifest['reference_bases'][base_index]
    origin = np.array(base['position'])
    rot = np.array(base['world_to_local'])
    raw = (pack_dir / 'collision.bin').read_bytes()
    floats = struct.unpack(f'<{len(raw)//4}f', raw)
    pts = np.array(floats).reshape(-1, 3)
    local = (pts - origin) @ rot.T
    # local axis order from base['world_to_local']: [x, y, height]
    tris = local.reshape(-1, 3, 3)
    centroid = tris.mean(axis=1)
    near = (np.abs(centroid[:, 0]) < 70) & (np.abs(centroid[:, 1]) < 70) & (centroid[:, 2] > -80) & (centroid[:, 2] < 90)
    return tris[near], base['name']


def triangle_normal(tri):
    cr = np.cross(tri[1] - tri[0], tri[2] - tri[0])
    norm = np.linalg.norm(cr)
    return cr / norm if norm > 1e-9 else None


def connected_components(tris, snap=0.001, normal_tol=0.000001, plane_tol=0.002):
    """Group edge-adjacent coplanar triangles (not original authoring panels).
    Two matching endpoints are required: point contact is NOT edge adjacency.
    Adjacency is approximate after snapping, AND coplanar (matching
    normal and plane offset). Plain vertex adjacency alone is not enough — a
    floor touches its bounding walls at every edge, so without the
    coplanarity check the whole building collapses into a few giant blobs
    instead of one component per wall/floor/ramp quad."""
    n = len(tris)
    parent = list(range(n))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    def union(a, b):
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[ra] = rb

    normals = [triangle_normal(tri) for tri in tris]
    offsets = [float(np.dot(normals[i], tris[i][0])) if normals[i] is not None else 0. for i in range(n)]

    edge_owner = {}
    for i, tri in enumerate(tris):
        if normals[i] is None:
            continue
        vertices=[tuple(np.round(v / snap).astype(int)) for v in tri]
        for a,b in zip(vertices,vertices[1:]+vertices[:1]):
            if a == b: continue
            key=tuple(sorted((a,b)))
            for j in edge_owner.get(key, []):
                if normals[j] is None:
                    continue
                if np.dot(normals[i], normals[j]) > 1 - normal_tol and abs(offsets[i] - offsets[j]) < plane_tol:
                    union(i, j)
            edge_owner.setdefault(key, []).append(i)
    groups = {}
    for i in range(n):
        if normals[i] is None:
            continue
        groups.setdefault(find(i), []).append(i)
    return list(groups.values())


def panel_stats(tris, indices):
    group = tris[indices]
    normals = []
    area = 0.0
    for tri in group:
        cr = np.cross(tri[1] - tri[0], tri[2] - tri[0])
        a = np.linalg.norm(cr) / 2
        if a < 1e-9:
            continue
        normals.append(cr / (2 * a))
        area += a
    if not normals:
        return None
    normal = np.mean(normals, axis=0)
    normal /= np.linalg.norm(normal)
    pts = group.reshape(-1, 3)
    kind = 'ceiling' if normal[2] < -0.7 else 'floor' if normal[2] > 0.7 else 'ramp' if abs(normal[2]) > 0.15 else 'wall'
    return dict(
        kind=kind,
        triangles=len(indices),
        area=round(float(area), 2),
        normal=[round(float(v), 3) for v in normal],
        x=[round(float(pts[:, 0].min()), 2), round(float(pts[:, 0].max()), 2)],
        y=[round(float(pts[:, 1].min()), 2), round(float(pts[:, 1].max()), 2)],
        height=[round(float(pts[:, 2].min()), 2), round(float(pts[:, 2].max()), 2)],
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pack', type=Path, default=ROOT / 'local-assets/broadside-reference')
    parser.add_argument('--base', type=int, default=0)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--min-area', type=float, default=1.0, help='drop panels smaller than this (m^2), mostly render noise/trim')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / 'local-assets') or out.exists():
        parser.error('Output must be a NEW directory under ignored local-assets/')

    tris, base_name = load_local_triangles(args.pack, args.base)
    print(f'{base_name}: {len(tris)} collision triangles near local origin', file=sys.stderr)
    components = connected_components(tris)
    print(f'{len(components)} connected panels before size filtering', file=sys.stderr)

    panels = []
    for indices in components:
        stats = panel_stats(tris, indices)
        if stats and stats['area'] >= args.min_area:
            panels.append(stats)
    panels.sort(key=lambda p: -p['area'])

    out.mkdir(parents=True)
    (out / 'panels.json').write_text(json.dumps(panels, indent=2) + '\n')
    by_kind = {}
    for p in panels:
        by_kind.setdefault(p['kind'], []).append(p)
    for kind, group in by_kind.items():
        print(f'{kind}: {len(group)} panels, total area {sum(p["area"] for p in group):.0f} m^2')
    print(f'wrote {len(panels)} panels (area>={args.min_area} m^2) to {out/"panels.json"}')


if __name__ == '__main__':
    main()
