"""Walk-graph route checks on a built pack (collision.bin + height.bin + holes).

Counts how many independent walkable ways lead from open ground into a
region (a station's floors, a flag deck, a generator room). A base with one
way in is easy to camp; docs/map-pipeline.md asks for at least three into a
main building, two onto a flag deck and exactly two into a generator room.

Model (walking only; no jumps, jets or drops): standing nodes sit on a 1 m
grid on every floor with 2.6 m of clear headroom (the body's four 0.52 m
spheres reach floor+2.56 m). Neighbours (8-connected) join when the floor
changes by at most 0.75 m and a 0.9 m wide band is clear between them at
knee, chest and head height. An entry is a cluster of graph edges crossing
from open ground's reachable component into the region; crossings within
ENTRY_MERGE metres of each other are one entry.
"""
import json
import math
from collections import deque
from pathlib import Path

import numpy as np

GRID = 1.0
HEADROOM = 2.6
MAX_RISE = 0.75
EDGE_HEIGHTS = (0.45, 1.2, 2.2)
LATERAL = 0.45
BUCKET = 4.0
ENTRY_MERGE = 3.0


class Pack:
    def __init__(self, pack):
        pack = Path(pack)
        self.manifest = json.loads((pack/'map.json').read_text())
        t = np.frombuffer((pack/'collision.bin').read_bytes(), '<f4').astype(np.float64).reshape(-1, 3, 3)
        self.t = t
        self.a = t[:, 0]; self.e1 = t[:, 1]-t[:, 0]; self.e2 = t[:, 2]-t[:, 0]
        self.lo = t.min(1); self.hi = t.max(1)
        n = np.cross(self.e1, self.e2); self.ny = n[:, 1]/(np.linalg.norm(n, axis=1)+1e-12)
        self.h = np.frombuffer((pack/'height.bin').read_bytes(), '<u2').reshape(256, 256)/32.0
        self.step = self.manifest.get('terrain_step', 8.0)
        self.holes = set(self.manifest.get('holes', []))
        self.buckets = {}
        b0 = np.floor(self.lo[:, [0, 2]]/BUCKET).astype(int); b1 = np.floor(self.hi[:, [0, 2]]/BUCKET).astype(int)
        for i in range(len(t)):
            for bx in range(b0[i, 0], b1[i, 0]+1):
                for bz in range(b0[i, 1], b1[i, 1]+1):
                    self.buckets.setdefault((bx, bz), []).append(i)
        self.buckets = {k: np.array(v) for k, v in self.buckets.items()}

    def hole(self, x, z):
        ix, iz = int(math.floor(x/self.step)), int(math.floor(z/self.step))
        return iz*256+ix in self.holes

    def terrain(self, x, z):
        fx = min(max(x/self.step, 0), 255); fz = min(max(z/self.step, 0), 255)
        x0, z0 = int(fx), int(fz); x1, z1 = min(x0+1, 255), min(z0+1, 255)
        tx, tz = fx-x0, fz-z0
        a = self.h[z0, x0]+(self.h[z0, x1]-self.h[z0, x0])*tx
        b = self.h[z1, x0]+(self.h[z1, x1]-self.h[z1, x0])*tx
        return a+(b-a)*tz

    def near(self, x, z, r=1):
        bx, bz = int(math.floor(x/BUCKET)), int(math.floor(z/BUCKET))
        parts = [self.buckets[k] for k in ((bx+i, bz+j) for i in range(-r, r+1) for j in range(-r, r+1)) if k in self.buckets]
        return np.unique(np.concatenate(parts)) if parts else np.zeros(0, int)

    def column(self, x, z):
        """Heights of every non-vertical collision surface over (x, z)."""
        idx = self.near(x, z, 0)
        idx = idx[np.abs(self.ny[idx]) > 1e-3] if len(idx) else idx
        if not len(idx): return np.zeros(0)
        a, e1, e2 = self.a[idx], self.e1[idx], self.e2[idx]
        px, pz = x-a[:, 0], z-a[:, 2]
        det = e1[:, 0]*e2[:, 2]-e1[:, 2]*e2[:, 0]
        ok = np.abs(det) > 1e-12; det = np.where(ok, det, 1)
        u = (px*e2[:, 2]-pz*e2[:, 0])/det; v = (e1[:, 0]*pz-e1[:, 2]*px)/det
        k = ok & (u >= -1e-7) & (v >= -1e-7) & (u+v <= 1+1e-7)
        return np.sort(a[k, 1]+u[k]*e1[k, 1]+v[k]*e2[k, 1])

    def blocked(self, starts, ends):
        """Segment-vs-soup test for rays that all lie near one another."""
        mid = (starts.mean(0)+ends.mean(0))/2
        idx = self.near(mid[0], mid[2])
        if not len(idx): return np.zeros(len(starts), bool)
        A, E1, E2 = self.a[idx][None], self.e1[idx][None], self.e2[idx][None]
        s = starts[:, None]; d = ends[:, None]-s
        h = np.cross(d, E2); det = (E1*h).sum(-1)
        ok = np.abs(det) > 1e-9; inv = np.where(ok, 1/np.where(ok, det, 1), 0)
        tv = s-A; u = (tv*h).sum(-1)*inv; q = np.cross(tv, E1)
        v = (d*q).sum(-1)*inv; t = (E2*q).sum(-1)*inv
        return (ok & (u >= 0) & (v >= 0) & (u+v <= 1) & (t > 1e-4) & (t < 1-1e-4)).any(1)


def floors(pack, x, z):
    """Standing heights over (x, z): structure floors and open terrain, each
    with at least HEADROOM clear above."""
    ys = pack.column(x, z)
    cand = list(ys)
    if not pack.hole(x, z): cand.append(pack.terrain(x, z))
    cand = sorted(set(round(float(y), 3) for y in cand))
    out = []
    ground = None if pack.hole(x, z) else pack.terrain(x, z)
    for y in cand:
        if ground is not None and y < ground-.05: continue      # under the ground
        above = ys[ys > y+.05]
        if len(above) and above[0] < y+HEADROOM: continue
        if out and y-out[-1] < .3: out[-1] = y; continue
        out.append(y)
    return out


class Graph:
    def __init__(self, pack, x0, x1, z0, z1):
        self.pack = pack
        self.nodes = []                      # (x, y, z)
        self.at = {}                         # (ix, iz) -> [node ids]
        for ix in range(int(math.floor(x0/GRID)), int(math.ceil(x1/GRID))+1):
            for iz in range(int(math.floor(z0/GRID)), int(math.ceil(z1/GRID))+1):
                x, z = ix*GRID+.5*GRID, iz*GRID+.5*GRID
                for y in floors(pack, x, z):
                    self.at.setdefault((ix, iz), []).append(len(self.nodes))
                    self.nodes.append((x, y, z))
        self.nodes = np.array(self.nodes)
        self.adj = [[] for _ in range(len(self.nodes))]
        self._edges()

    def _edges(self):
        pairs = []
        for (ix, iz), ids in self.at.items():
            for dx, dz in ((1, 0), (0, 1), (1, 1), (1, -1)):
                for j in self.at.get((ix+dx, iz+dz), ()):
                    for i in ids:
                        if abs(self.nodes[i, 1]-self.nodes[j, 1]) <= MAX_RISE*(1.42 if dx and dz else 1): pairs.append((i, j))
        if not pairs: return
        pairs = np.array(pairs)
        a, b = self.nodes[pairs[:, 0]], self.nodes[pairs[:, 1]]
        d = b-a; flat = d.copy(); flat[:, 1] = 0
        side = np.stack([-flat[:, 2], np.zeros(len(d)), flat[:, 0]], 1)
        side /= np.linalg.norm(side, axis=1, keepdims=True)
        ok = np.ones(len(pairs), bool)
        order = np.lexsort((np.floor(a[:, 2]/BUCKET), np.floor(a[:, 0]/BUCKET)))
        keys = np.floor(a[order][:, [0, 2]]/BUCKET).astype(int)
        cuts = np.flatnonzero(np.any(np.diff(keys, axis=0) != 0, 1))+1
        for grp in np.split(order, cuts):
            starts, ends = [], []
            for h in EDGE_HEIGHTS:
                for off in (-LATERAL, 0.0, LATERAL):
                    o = side[grp]*off+np.array([0, h, 0])
                    starts.append(a[grp]+o); ends.append(b[grp]+o)
            hit = self.pack.blocked(np.concatenate(starts), np.concatenate(ends)).reshape(len(EDGE_HEIGHTS)*3, len(grp))
            ok[grp] = ~hit.any(0)
        for (i, j), good in zip(pairs, ok):
            if good: self.adj[i].append(j); self.adj[j].append(i)

    def reach(self, seeds, blocked):
        seen = np.zeros(len(self.nodes), bool); q = deque()
        for s in seeds:
            if not blocked[s]: seen[s] = True; q.append(s)
        while q:
            i = q.popleft()
            for j in self.adj[i]:
                if not seen[j] and not blocked[j]: seen[j] = True; q.append(j)
        return seen


def entries(graph, inside, outside_seeds):
    """Clusters of edges from the region's outside (reached from the seeds
    without entering the region) into the region. Returns cluster centres."""
    outside = graph.reach(outside_seeds, inside)
    cross = [(graph.nodes[i]+graph.nodes[j])/2 for i in np.flatnonzero(outside) for j in graph.adj[i] if inside[j]]
    clusters = []
    for p in cross:
        for c in clusters:
            if min(np.linalg.norm(p-q) for q in c) < ENTRY_MERGE: c.append(p); break
        else: clusters.append([p])
    # Merge clusters that grew into each other.
    merged = True
    while merged:
        merged = False
        for i in range(len(clusters)):
            for j in range(i+1, len(clusters)):
                if min(np.linalg.norm(p-q) for p in clusters[i] for q in clusters[j]) < ENTRY_MERGE:
                    clusters[i] += clusters.pop(j); merged = True; break
            if merged: break
    return [np.mean(c, 0) for c in clusters]


def box_region(graph, x0, x1, y0, y1, z0, z1):
    n = graph.nodes
    return (n[:, 0] >= x0) & (n[:, 0] <= x1) & (n[:, 1] >= y0) & (n[:, 1] <= y1) & (n[:, 2] >= z0) & (n[:, 2] <= z1)


def base_entries(pack, centre, regions, extent=64.0, seed_radius=48.0):
    """Entries into each named world-space region box (x0, x1, y0, y1, z0, z1)
    around one base: {name: [entry centres]}. Open ground is every terrain
    node at least seed_radius from the base centre, inside a square of
    half-size `extent`. Pass a Pack or a pack directory."""
    pack = pack if isinstance(pack, Pack) else Pack(pack)
    cx, cz = centre
    g = Graph(pack, cx-extent, cx+extent, cz-extent, cz+extent)
    seeds = open_ground(g, cx, cz, seed_radius)
    return {name: entries(g, box_region(g, *box), seeds) for name, box in regions.items()}


def open_ground(graph, cx, cz, radius):
    """Seed nodes: terrain-level nodes on a ring at least `radius` from the centre."""
    n = graph.nodes
    far = np.hypot(n[:, 0]-cx, n[:, 2]-cz) >= radius
    ground = np.array([abs(y-graph.pack.terrain(x, z)) < .3 for x, y, z in n])
    return np.flatnonzero(far & ground)
