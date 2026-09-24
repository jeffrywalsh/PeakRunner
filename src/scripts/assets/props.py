"""Original procedural scenery props, scattered deterministically over a
map's terrain after its structures are built and baked.

Props are PeakRunner's own low-poly shapes built with the kit's `Mesh`
primitives. They never come from another game. Each map picks a theme (rocks,
tufts, ferns, logs, standing stones, ice, scrub, bones...) made from the
map's existing painted materials.

Rules (docs/map-pipeline.md):

* Big props (boulders, outcrops, standing stones, logs, large ice shards) are
  solid, simple and sunk into the ground so no edge hovers. They are placed in
  mirrored pairs through the midpoint of the two flags, keep off the ski lanes
  (flag to flag, flag to each control point), stay `BASE_CLEAR` from each flag
  so base approaches stay open, and never exceed
  `PROP_COLLISION_TRIS` per map.
* Small props (tufts, ferns, heather, scrub, saplings, bones) are render-only.
* Nothing lands in protected space: under or near any existing collision
  geometry (bases, towers, bridges, pads, trees), terrain holes (bunker cuts,
  sewer and cavern mouths, trenches) and their neighbours, around flags, spawn
  points and control-point rings, below water, or off the map edge.
* Density thins with distance from the flags and control points.

Props are appended after the structure lightmap bake, keeping the kit's live
lighting path (vertex light layer -1). They do not add lightmap pages, and the
terrain shade map (terrain_shade.py) shades them where they stand.
"""
import math
import random

import numpy as np

SOURCE_VERSION = 4
# Solid triangles all of a map's props may add (outside the per-base budget).
PROP_COLLISION_TRIS = 2500
LANE_HALF_WIDTH = 28.0
MARGIN_BIG = 8.0
MARGIN_SMALL = 4.0
FLAG_CLEAR = 25.0
# Big props also keep off each base's approaches, where players and bots path in.
BASE_CLEAR = 80.0
SPAWN_CLEAR = 12.0
RING_CLEAR = 8.0
EDGE = 48.0
OCC_CELL = 2.0

# kind: (material(s), count, size range, solid). Counts are targets; big
# counts are pairs (each placement is mirrored).
THEMES = {
    'old-holler': dict(
        big=[('boulder', 'rock', 24, (1.3, 3.0)), ('log', 'bark', 8, (3.0, 6.0))],
        small=[('tuft', 'moss', 4200, (.55, 1.1)), ('fern', 'leaf', 900, (.8, 1.5))]),
    'tower-complex': dict(
        big=[('outcrop', 'rock', 22, (2.2, 4.6))],
        small=[('scrub', 'leaf', 700, (.8, 1.6)), ('tuft', 'moss', 3200, (.5, 1.0))]),
    'cairnhold': dict(
        big=[('standing_stone', 'concrete', 12, (3.0, 5.5)), ('boulder', 'rock', 20, (1.2, 2.6))],
        small=[('heather', 'moss', 1300, (.7, 1.4)), ('tuft', 'leaf', 2400, (.5, 1.0))]),
    'frostline': dict(
        big=[('snow_rock', ('rock', 'meadow'), 22, (1.5, 3.4)), ('ice_shard', 'soil', 12, (2.6, 5.0))],
        small=[('sapling', ('leaf', 'bark'), 200, (2.0, 4.0)), ('ice_shard', 'soil', 260, (.6, 1.6)),
               ('tuft', 'moss', 1800, (.45, .9))]),
    'dustreach': dict(
        big=[('boulder', 'rock', 24, (1.4, 3.4))],
        small=[('scrub', 'moss', 700, (.7, 1.4)), ('tuft', 'soil', 3200, (.6, 1.2)),
               ('bones', 'bark', 40, (1.2, 2.2))]),
}


class Terrain:
    """The pack's 256x256 heightfield (height.bin), sampled like the engine."""

    def __init__(self, height_bytes, weights_bytes, holes, step=8.0, water=None):
        self.h = np.frombuffer(height_bytes, '<u2').reshape(256, 256).astype(np.float64)/32
        self.w = np.frombuffer(weights_bytes, np.uint8).reshape(256, 256, 4).astype(np.float64)/255
        self.step = step; self.water = water
        cut = np.zeros((256, 256), bool)
        for i in holes: cut[i//256, i % 256] = True
        # Holes plus one cell around them: bunker cuts, sewer and cavern
        # mouths, trenches.
        near = cut.copy()
        for dz in (-1, 0, 1):
            for dx in (-1, 0, 1): near |= np.roll(np.roll(cut, dz, 0), dx, 1)
        self.cut = near

    def height(self, x, z):
        fx = min(max(x/self.step, 0), 255); fz = min(max(z/self.step, 0), 255)
        x0, z0 = int(fx), int(fz); x1, z1 = min(x0+1, 255), min(z0+1, 255); tx, tz = fx-x0, fz-z0
        a = self.h[z0, x0]+(self.h[z0, x1]-self.h[z0, x0])*tx
        b = self.h[z1, x0]+(self.h[z1, x1]-self.h[z1, x0])*tx
        return a+(b-a)*tz

    def slope(self, x, z):
        d = 2.0
        gx = (self.height(x+d, z)-self.height(x-d, z))/(2*d); gz = (self.height(x, z+d)-self.height(x, z-d))/(2*d)
        return math.degrees(math.atan(math.hypot(gx, gz)))

    def weight(self, x, z):
        ix = min(max(int(x/self.step), 0), 255); iz = min(max(int(z/self.step), 0), 255)
        return self.w[iz, ix]

    def near_hole(self, x, z):
        ix = min(max(int(x/self.step), 0), 255); iz = min(max(int(z/self.step), 0), 255)
        return bool(self.cut[iz, ix])


class Protection:
    """Everywhere a prop may not stand."""

    def __init__(self, collision, flags, spawn_points, control_points, extra=()):
        tris = np.frombuffer(bytes(collision), '<f4').reshape(-1, 3, 3) if len(collision) else np.zeros((0, 3, 3), np.float32)
        n = int(2048/OCC_CELL)
        lo = tris.min(1); hi = tris.max(1)
        occ = np.zeros((n, n), bool)
        for (x0, _, z0), (x1, _, z1) in zip(lo, hi):
            i0 = max(int(x0//OCC_CELL), 0); i1 = min(int(x1//OCC_CELL), n-1)
            j0 = max(int(z0//OCC_CELL), 0); j1 = min(int(z1//OCC_CELL), n-1)
            if i0 <= i1 and j0 <= j1: occ[j0:j1+1, i0:i1+1] = True
        self.cells = occ
        self.circles = [(f[0], f[2], FLAG_CLEAR) for f in flags]
        self.circles += [(p[0], p[2], SPAWN_CLEAR) for team in spawn_points for p in team]
        self.circles += [(c['pos'][0], c['pos'][2], c.get('radius', 12)+RING_CLEAR) for c in control_points]
        self.circles += list(extra)
        self.interest = [(f[0], f[2]) for f in flags]+[(c['pos'][0], c['pos'][2]) for c in control_points]
        self.lanes = []
        for i, a in enumerate(flags):
            for b in flags[i+1:]: self.lanes.append(((a[0], a[2]), (b[0], b[2])))
            for c in control_points: self.lanes.append(((a[0], a[2]), (c['pos'][0], c['pos'][2])))

    def occupied(self, x, z, margin):
        r = int(math.ceil(margin/OCC_CELL)); n = self.cells.shape[0]
        i = int(x//OCC_CELL); j = int(z//OCC_CELL)
        return bool(self.cells[max(j-r, 0):min(j+r+1, n), max(i-r, 0):min(i+r+1, n)].any())

    def in_circle(self, x, z, pad=0.0):
        return any(math.hypot(x-cx, z-cz) < r+pad for cx, cz, r in self.circles)

    def lane_distance(self, x, z):
        best = math.inf
        for (ax, az), (bx, bz) in self.lanes:
            dx, dz = bx-ax, bz-az; t = ((x-ax)*dx+(z-az)*dz)/max(dx*dx+dz*dz, 1e-9)
            t = min(max(t, 0), 1); best = min(best, math.hypot(x-(ax+t*dx), z-(az+t*dz)))
        return best

    def flag_distance(self, x, z):
        return min(math.hypot(x-fx, z-fz) for fx, fz, _ in self.circles[:2])

    def interest_distance(self, x, z):
        return min((math.hypot(x-a, z-b) for a, b in self.interest), default=0.0)


def _rot(p, yaw, tilt=0.0, axis_yaw=0.0):
    """Tilt a local point about a horizontal axis, then turn it by yaw."""
    x, y, z = p
    if tilt:
        ca, sa = math.cos(axis_yaw), math.sin(axis_yaw)
        u = x*ca+z*sa; v = -x*sa+z*ca           # into the tilt axis frame
        c, s = math.cos(tilt), math.sin(tilt)
        u, y = u*c-y*s, u*s+y*c
        x = u*ca-v*sa; z = u*sa+v*ca
    c, s = math.cos(yaw), math.sin(yaw)
    return (x*c+z*s, y, -x*s+z*c)


def _hull(mesh, rng, r, height, mat, solid, lon=6, sink=0.0, squash=1.0):
    """Irregular low-poly rock: a lat/long hull with jittered radii."""
    rings = [(-.35, .95), (.25, 1.0), (.75, .72)]
    pts = []
    for ry, rr in rings:
        row = []
        for k in range(lon):
            a = (k+rng.uniform(-.18, .18))*math.tau/lon
            j = rng.uniform(.78, 1.18)
            row.append((r*rr*j*math.cos(a)*squash, height*ry-sink, r*rr*j*math.sin(a)))
        pts.append(row)
    top = (rng.uniform(-.15, .15)*r, height*rng.uniform(.95, 1.1)-sink, rng.uniform(-.15, .15)*r)
    bottom = (0, -height*.6-sink, 0)
    tris = []
    for k in range(lon):
        n = (k+1) % lon
        tris.append((bottom, pts[0][n], pts[0][k]))
        for i in range(len(rings)-1):
            a, b, c, d = pts[i][k], pts[i][n], pts[i+1][n], pts[i+1][k]
            tris += [(a, b, c), (a, c, d)]
        tris.append((pts[-1][k], pts[-1][n], top))
    for t in tris: mesh.triangle(list(t), mat, solid)
    return len(tris)


def boulder(mesh, rng, size, mat, solid=True):
    return _hull(mesh, rng, size, size*rng.uniform(.55, .8), mat, solid, sink=size*.18)


def outcrop(mesh, rng, size, mat, solid=True):
    n = 0
    for k in range(rng.choice([2, 3])):
        s = size*rng.uniform(.55, 1.0)
        ox, oz = rng.uniform(-.8, .8)*size, rng.uniform(-.8, .8)*size
        saved = mesh.origin
        mesh.origin = (saved[0]+ox, saved[1], saved[2]+oz)
        n += _hull(mesh, rng, s, s*rng.uniform(.8, 1.3), mat, solid, sink=s*.25)
        mesh.origin = saved
    return n


def snow_rock(mesh, rng, size, mats, solid=True):
    rock, snow = mats
    h = size*rng.uniform(.55, .8)
    n = _hull(mesh, rng, size, h, rock, solid, sink=size*.18)
    n += _hull(mesh, rng, size*.62, h*.28, snow, False, lon=6, sink=-(h*.78-size*.18))
    return n


def standing_stone(mesh, rng, size, mat, solid=True):
    r = size*rng.uniform(.16, .22); t = r*rng.uniform(.55, .8)
    lean = rng.uniform(0, math.radians(9)); lean_dir = rng.uniform(0, math.tau)
    sides = 5; base = []; head = []
    for k in range(sides):
        a = (k+rng.uniform(-.12, .12))*math.tau/sides
        base.append(_rot((r*math.cos(a), -1.2, r*math.sin(a)*.7), 0, lean, lean_dir))
        head.append(_rot((t*math.cos(a), size, t*math.sin(a)*.7), 0, lean, lean_dir))
    cap = _rot((0, size+t*.6, 0), 0, lean, lean_dir)
    for k in range(sides):
        n = (k+1) % sides
        mesh.quad(base[k], head[k], head[n], base[n], mat, solid)
        mesh.triangle([head[k], cap, head[n]], mat, solid)
    return sides*3


def log(mesh, rng, size, mat, solid=True):
    r = rng.uniform(.3, .45)
    mesh.box((0, r*.55, 0), (size, r*1.7, r*1.7), mat, solid)
    return 12


def ice_shard(mesh, rng, size, mat, solid=True):
    r = size*rng.uniform(.14, .22); lean = rng.uniform(math.radians(4), math.radians(22))
    d = rng.uniform(0, math.tau); sides = 4
    base = [_rot((r*math.cos(k*math.tau/sides), -.6, r*math.sin(k*math.tau/sides)), 0, lean, d) for k in range(sides)]
    tip = _rot((0, size, 0), 0, lean, d)
    for k in range(sides):
        mesh.triangle([base[k], tip, base[(k+1) % sides]], mat, solid)
    return sides


def tuft(mesh, rng, size, mat, solid=False):
    blades = rng.randint(3, 5)
    for _ in range(blades):
        a = rng.uniform(0, math.tau); off = rng.uniform(0, size*.25)
        bx, bz = off*math.cos(a), off*math.sin(a)
        lean = rng.uniform(.1, .5); h = size*rng.uniform(.7, 1.1); w = size*.07
        tx, tz = bx+lean*h*math.cos(a), bz+lean*h*math.sin(a)
        px, pz = -math.sin(a)*w, math.cos(a)*w
        mesh.triangle([(bx-px, -.05, bz-pz), (bx+px, -.05, bz+pz), (tx, h, tz)], mat, False)
    return blades


def fern(mesh, rng, size, mat, solid=False):
    fronds = 6
    for k in range(fronds):
        a = (k+rng.uniform(-.2, .2))*math.tau/fronds
        ca, sa = math.cos(a), math.sin(a); w = size*.12
        mid = (ca*size*.5, size*.45, sa*size*.5); tip = (ca*size, size*.15, sa*size)
        px, pz = -sa*w, ca*w
        base = (0, -.05, 0)
        left = (mid[0]-px, mid[1], mid[2]-pz); right = (mid[0]+px, mid[1], mid[2]+pz)
        mesh.triangle([base, right, left], mat, False)
        mesh.triangle([left, right, tip], mat, False)
    return fronds*2


def heather(mesh, rng, size, mat, solid=False):
    return _hull(mesh, rng, size*.5, size*.35, mat, False, lon=4, sink=size*.1, squash=1.2)


def scrub(mesh, rng, size, mat, solid=False):
    n = _hull(mesh, rng, size*.45, size*.6, mat, False, lon=4, sink=-size*.25)
    for _ in range(3):
        a = rng.uniform(0, math.tau); w = size*.05
        tip = (math.cos(a)*size*.35, size*.5, math.sin(a)*size*.35)
        mesh.triangle([(-w, -.05, 0), (w, -.05, 0), tip], mat, False)
    return n+3


def sapling(mesh, rng, size, mats, solid=False):
    leaf, bark = mats
    mesh.column((0, -.2, 0), size*.05, size*.35, bark, 4, top=size*.03, solid=False)
    mesh.column((0, size*.2, 0), size*.3, size*.5, leaf, 6, top=.02, solid=False)
    mesh.column((0, size*.52, 0), size*.2, size*.48, leaf, 6, top=.02, solid=False)
    return 4*3+6*3*2


def bones(mesh, rng, size, mat, solid=False):
    n = 0; ribs = 5; spine = size
    mesh.box((0, .08, 0), (spine, .12, .14), mat, False); n += 12
    for k in range(ribs):
        x = (k/(ribs-1)-.5)*spine*.8; r = size*.28*(1-.35*abs(k/(ribs-1)-.5)*2)
        for side in (-1, 1):
            prev = (x, .1, 0)
            for s in range(1, 4):
                a = s/3*math.pi*.5
                p = (x, .1+math.sin(a)*r*.9, side*(1-math.cos(a)+.3)*r)
                mesh.triangle([prev, (x+.05, prev[1], prev[2]), p], mat, False); n += 1
                prev = p
    return n


BUILDERS = dict(boulder=boulder, outcrop=outcrop, snow_rock=snow_rock, standing_stone=standing_stone,
                log=log, ice_shard=ice_shard, tuft=tuft, fern=fern, heather=heather, scrub=scrub,
                sapling=sapling, bones=bones)
# Rough footprint radius factor (x size) used for sinking and spacing.
FOOT = dict(boulder=1.1, outcrop=1.9, snow_rock=1.1, standing_stone=.35, log=.55, ice_shard=.35,
            tuft=.4, fern=1.0, heather=.7, scrub=.6, sapling=.4, bones=.8)


def _ground(terrain, x, z, radius):
    """Lowest terrain under a footprint, so no edge hovers."""
    samples = [terrain.height(x, z)]
    for k in range(8):
        a = k*math.tau/8
        samples.append(terrain.height(x+radius*math.cos(a), z+radius*math.sin(a)))
    return min(samples)


def _slope_ok(kind, slope):
    if kind in ('standing_stone', 'log'): return slope < 18
    if kind in ('tuft', 'fern', 'heather', 'sapling', 'bones'): return slope < 30
    if kind == 'scrub': return slope < 34
    return slope < 42


def _terrain_ok(kind, terrain, x, z):
    w = terrain.weight(x, z)   # [layer0 (grass/snow/sand), rock, soil, moss]
    if kind in ('tuft', 'fern', 'heather', 'bones'): return w[1] < .55
    if kind in ('boulder', 'outcrop', 'snow_rock'): return True
    return w[1] < .8


def scatter(kit, theme, seed, terrain, protect):
    """Build the theme's props into a fresh kit Mesh. Returns
    (render vertex bytes, collision bytes, caster vertex bytes, summary)."""
    spec = THEMES[theme]
    mesh = kit.Mesh(); casters = kit.Mesh()
    # Big props mirror through the midpoint of the two flags (the first two
    # interest points), which is each map's symmetry centre.
    (ax, az), (bx, bz) = protect.interest[:2]
    cx, cz = (ax+bx)/2, (az+bz)/2
    placed, big = {}, []
    rng = random.Random(seed*7919+SOURCE_VERSION)

    def valid(kind, x, z, size, is_big):
        if not (EDGE <= x <= 2048-EDGE and EDGE <= z <= 2048-EDGE): return None
        foot = FOOT[kind]*size
        if terrain.near_hole(x, z): return None
        if protect.occupied(x, z, (MARGIN_BIG if is_big else MARGIN_SMALL)+foot): return None
        if protect.in_circle(x, z, foot): return None
        if is_big and protect.lane_distance(x, z) < LANE_HALF_WIDTH+foot: return None
        if is_big and protect.flag_distance(x, z) < BASE_CLEAR+foot: return None
        if not _slope_ok(kind, terrain.slope(x, z)) or not _terrain_ok(kind, terrain, x, z): return None
        y = _ground(terrain, x, z, foot)
        if terrain.water is not None and y < terrain.water+1.0: return None
        return y

    def build(target, kind, mats, x, y, z, size, yaw, solid, shape):
        # Each prop's shape comes from its own seed, drawn from the scatter's
        # generator, so builds are deterministic and mirrored pairs match.
        target.origin = (x, y, z); target.yaw = yaw
        BUILDERS[kind](target, random.Random(shape), size, mats, solid)

    for kind, mats, pairs, (smin, smax) in spec['big']:
        count = 0
        for _ in range(pairs*60):
            if count >= pairs: break
            x, z = rng.uniform(EDGE, 2048-EDGE), rng.uniform(EDGE, 2048-EDGE)
            size = rng.uniform(smin, smax); yaw = rng.uniform(0, math.tau); shape = rng.getrandbits(32)
            if rng.random() > .3+.7*math.exp(-protect.interest_distance(x, z)/260): continue
            mx, mz = 2*cx-x, 2*cz-z
            if math.hypot(x-mx, z-mz) < 40: continue
            ya = valid(kind, x, z, size, True); yb = valid(kind, mx, mz, size, True)
            if ya is None or yb is None: continue
            if any(math.hypot(x-px, z-pz) < 10 or math.hypot(mx-px, mz-pz) < 10 for px, pz, *_ in big): continue
            pair = kit.Mesh()
            build(pair, kind, mats, x, ya, z, size, yaw, True, shape)
            build(pair, kind, mats, mx, yb, mz, size, yaw+math.pi, True, shape)
            # The budget is a hard limit: a pair that would exceed it is dropped.
            if (len(mesh.collision)+len(pair.collision))//9 > PROP_COLLISION_TRIS: break
            mesh.vertices.extend(pair.vertices); mesh.collision.extend(pair.collision)
            casters.vertices.extend(pair.vertices)
            big += [(x, z, kind, size, ya), (mx, mz, kind, size, yb)]
            count += 1
        placed[kind] = placed.get(kind, 0)+count*2
    for kind, mats, count, (smin, smax) in spec['small']:
        # Small props grow in patches, so a player at ground level sees
        # vegetation around them rather than one tuft per hectare.
        n = 0
        for _ in range(count*6):
            if n >= count: break
            px, pz = rng.uniform(EDGE, 2048-EDGE), rng.uniform(EDGE, 2048-EDGE)
            if rng.random() > .2+.8*math.exp(-protect.interest_distance(px, pz)/400): continue
            if valid(kind, px, pz, smax, False) is None: continue
            radius = rng.uniform(3.5, 8.0)
            for _ in range(rng.randint(5, 12)):
                if n >= count: break
                a = rng.uniform(0, math.tau); r = radius*math.sqrt(rng.random())
                x, z = px+r*math.cos(a), pz+r*math.sin(a)
                size = rng.uniform(smin, smax); yaw = rng.uniform(0, math.tau); shape = rng.getrandbits(32)
                y = valid(kind, x, z, size, False)
                if y is None: continue
                build(mesh, kind, mats, x, y, z, size, yaw, False, shape)
                n += 1
        placed[kind] = placed.get(kind, 0)+n
    summary = dict(theme=theme, version=SOURCE_VERSION, counts=placed,
                   render_triangles=len(mesh.vertices)//36, solid_triangles=len(mesh.collision)//9,
                   big=[[round(x, 2), round(y, 2), round(z, 2), kind, round(size, 2)] for x, z, kind, size, y in big])
    return mesh.vertices.tobytes(), mesh.collision.tobytes(), casters.vertices.tobytes(), summary
