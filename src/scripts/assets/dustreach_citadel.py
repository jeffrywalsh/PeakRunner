"""Original Dustreach team base: a ruined sandstone citadel on a raised
terrace. A keep with twin pylon towers holds the spawn hall, inventories and
the generator; behind it an arcaded courtyard surrounds the sunken flag court,
open to the sky. A watch tower carries the sentry turret and sensor, a broken
watch ruin on the right flank carries the plasma turret, and a raised caravan
dais on the left flank is the landing and deploy pad.

No source mesh, texture, lightmap or parser is used by this builder. Local
X/Z are horizontal, Y is up, the surrounding ground is Y=0. -Z is the front
(field-facing) side; facing the field, the right hand is +X.

Nothing is dug into the terrain: the build blends the ground to Y=-0.1 under
every structure and every outer wall runs 3 m below that.

Sightlines: both keep doors open into a vestibule closed by a baffle, so no
turret outside has a straight line into the spawn hall.

Dressing is render-only and stays within the 0.52 m player radius of a solid
surface. If the caller's mesh has a `lamps` list, light fixtures add bake
samples.
"""
import math

from assets.structure_kit import Builder

ASSET_ID = 'dustreach-citadel-v1'

TER = 5.0                                   # terrace top
TX, TZ0, TZ1 = 21.2, -18.0, 30.0            # terrace half-width and depth
FOOT = -3.0                                 # outer walls run this far below ground
# Keep: walls, doors, vestibule baffles.
KX, KZ0, KZ1 = 14.0, -16.0, 4.0
WALL = .8
CEIL, ROOF = 11.0, 12.0
DOOR, DOOR_TOP = (-2.5, 2.5), TER+4.5
FRONT_BAFFLE_Z, BACK_BAFFLE_Z = (-12.8, -12.4), (0.0, 0.4)
BAFFLE_X = 8.5
# Pylon towers flanking the keep front.
PYLON = (14.0, 20.0, -18.5, -12.5)          # |x| from, to, z from, to (meets the curtain)
PYLON_TOP, PYLON_CAP = 19.0, 21.0
# Courtyard: arcades along both sides, curtain walls with broken tops.
ARCADE_X, ARCADE_Z = 16.5, (8.0, 13.0, 18.0, 23.0, 27.2)
ARCADE_ROOF = 10.6
CURTAIN = 20.0                              # inner face of the side curtains
BACK_WALL = (28.8, TZ1)
BREACH = (-4.0, 4.0)
CURTAIN_TOPS = (12.4, 13.6, 11.6, 13.0, 12.2)   # broken tops, front to back
# Sunken flag court.
PIT_X, PIT_Z = 7.0, (8.0, 26.0)
PIT_FLOOR, PIT_DOWN = 2.0, (14.0, 20.0)
FLAG = (0.0, 17.0)
PLINTH = .3
# Access ramps from the ground: front to the keep door, back through the breach.
FRONT_RAMP = (-4.0, 4.0, -34.0, TZ0)        # x0, x1, z at ground, z at terrace
BACK_RAMP = (-4.0, 4.0, 46.0, TZ1)
# Watch tower at the back-left corner.
TOWER = (-30.0, -TX, 20.0, 29.5)
TOWER_TOP = 20.0
SENTRY, SENSOR = (-24.2, 26.0), (-27.0, 22.8)
# Watch ruin on the right flank, forward.
RUIN = (72.0, -64.0)
RUIN_R, RUIN_TOP = 5.5, 7.0
RUIN_RAMP_LEN = 14.0
# Caravan dais on the left flank.
PAD = (-84.0, -26.0)
PAD_TOP, PAD_HALF, PAD_LIP, PAD_GAP = 4.0, 12.0, .5, 3.0
PAD_SLOTS = ((-6.5, -6.5), (6.5, -6.5), (-6.5, 6.5), (6.5, 6.5))
# Fallen drums and a broken wall in front of the citadel: cover on the apron.
DRUMS = ((-26.0, -46.0, 1.3), (-21.0, -50.0, 1.1), (30.0, -40.0, 1.2))
FRONT_RUIN = (18.0, -52.0, 10.0)            # centre x, z, length along x
SPAWN_LIFT = 1.2

HULL, INTERIOR, DECK, TIMBER, METAL, GLOW = 'concrete', 'bark', 'panel', 'grate', 'trim', 'light'


def _clamp01(v):
    return min(max(v, 0), 1)


def ramp_y(ramp, z, y_a, y_b):
    """Surface height of a straight ramp (x0, x1, z_a, z_b) at z."""
    _, _, za, zb = ramp
    return y_a+(y_b-y_a)*_clamp01((z-za)/(zb-za))


def front_ramp_y(z): return ramp_y(FRONT_RAMP, z, 0.0, TER)
def back_ramp_y(z): return ramp_y(BACK_RAMP, z, 0.0, TER)


def pit_y(z):
    """Floor height over the sunken flag court and its two ramps."""
    z0, z1 = PIT_Z; d0, d1 = PIT_DOWN
    if z <= d0: return TER+(PIT_FLOOR-TER)*_clamp01((z-z0)/(d0-z0))
    if z >= d1: return TER+(PIT_FLOOR-TER)*_clamp01((z1-z)/(z1-d1))
    return PIT_FLOOR


def ruin_ramp():
    """The watch ruin's back ramp: x0, x1, z at the ground, z at the deck."""
    cx, cz = RUIN
    apothem = RUIN_R*math.cos(math.pi/8)
    return (cx-2.0, cx+2.0, cz+apothem+RUIN_RAMP_LEN, cz+apothem)


def sites():
    """Terrain surfaces to blend toward around the structures:
    (kind, shape, surface(x, z) -> local height, falloff metres). Shapes are
    ('rect', x0, x1, z0, z1) or ('disc', x, z, r) in local coordinates."""
    px, pz = PAD; rx, rz = RUIN
    flat = lambda x, z: -.1
    out = [('citadel', ('rect', TOWER[0]-4, TX+4, FRONT_RAMP[2]-4, BACK_RAMP[2]+4), flat, 20),
           ('ruin', ('disc', rx, rz, RUIN_R+9), flat, 14),
           ('ruin ramp', ('rect', rx-8, rx+8, rz, ruin_ramp()[2]+6), flat, 12),
           ('pad', ('rect', px-PAD_HALF-6, px+PAD_HALF+6, pz-PAD_HALF-6, pz+PAD_HALF+6), flat, 14),
           ('apron ruin', ('rect', FRONT_RUIN[0]-FRONT_RUIN[2]/2-5, FRONT_RUIN[0]+FRONT_RUIN[2]/2+5,
                           FRONT_RUIN[1]-6, FRONT_RUIN[1]+6), flat, 10)]
    for x, z, r in DRUMS:
        out.append(('drum', ('disc', x, z, r+5), flat, 8))
    return out


def _runs_without(lo, hi, gaps):
    out, u = [], lo
    for a, c in sorted(gaps):
        if a > u: out.append((u, a))
        u = max(u, c)
    if hi > u: out.append((u, hi))
    return out


def _merlon_spans(u0, u1, width=1.0, gap=1.4):
    n = max(1, int((u1-u0+gap)//(width+gap)))
    if n == 1: return [((u0+u1-width)/2, (u0+u1+width)/2)]
    step = (u1-u0-width)/(n-1)
    return [(u0+i*step, u0+i*step+width) for i in range(n)]


def build(mesh, team, circuit):
    if team not in (0, 1) or not circuit: raise ValueError('team and circuit required')
    accent = ['ember', 'glacier'][team]
    b = Builder(mesh, accent, interior=INTERIOR, metal=METAL, glow=GLOW)
    wall, slab = b.wall, b.slab

    def closed_side(x, z0, z1, surface, bottom=-.3):
        """Double-faced panel under a ramp edge from `bottom` up to 0.6 m
        under the surface, so nothing can be walked into beneath it."""
        u0, u1 = surface(z0)-.6, surface(z1)-.6
        if u0 < bottom:
            z0 = z0+(bottom-u0)*(z1-z0)/(u1-u0); u0 = bottom
        if u1 < bottom:
            z1 = z1+(bottom-u1)*(z0-z1)/(u0-u1); u1 = bottom
        pts = [(x, bottom, z0), (x, u0, z0), (x, u1, z1), (x, bottom, z1)]
        pts = [q for i, q in enumerate(pts) if q != pts[i-1]]
        if len(pts) == 3: mesh.triangle(pts, HULL); mesh.triangle(pts[::-1], HULL)
        else: mesh.quad(*pts, HULL); mesh.quad(*pts[::-1], HULL)

    def crenels(axis, fixed0, fixed1, u0, u1, y, heights=None):
        for i, (a, c) in enumerate(_merlon_spans(u0, u1)):
            h = .9 if heights is None else heights[i % len(heights)]
            if h <= 0: continue
            if axis == 'x': wall(a, c, fixed0, fixed1, y, y+h, HULL)
            else: wall(fixed0, fixed1, a, c, y, y+h, HULL)

    def collar(x, y, z, r):
        b.prism(x, z, r+.15, r, y, y+1.2, 8, HULL, solid=False, cap=False)
        b.prism(x, z, r+.08, r+.08, y+.8, y+1.0, 8, METAL, solid=False, cap=False)
        # Iron shroud over the kit turret head (render only; barrels stay live).
        mesh.box((x, y+2.7, z), (3.16, 1.3, 2.26), METAL, False)

    # --- Terrace ------------------------------------------------------------
    # Outer walls from below the ground up to the terrace top, and the paving,
    # tiled without overlaps round the hole of the sunken flag court.
    wall(-TX, TX, TZ0, TZ0+1, FOOT, TER-1, HULL)
    for s in (-1, 1): wall(s*(TX-1), s*TX, TZ0+1, BACK_WALL[0], FOOT, TER-1, HULL)
    wall(-TX, TX, BACK_WALL[0], TZ1, FOOT, TER-1, HULL)
    z0, z1 = PIT_Z
    for x0, x1, za, zb in [(-TX, TX, TZ0, KZ1), (-TX, -PIT_X, KZ1, TZ1), (PIT_X, TX, KZ1, TZ1),
                           (-PIT_X, PIT_X, KZ1, z0), (-PIT_X, PIT_X, z1, TZ1)]:
        slab(x0, x1, za, zb, TER, DECK)

    # --- Sunken flag court ----------------------------------------------------
    d0, d1 = PIT_DOWN
    slab(-PIT_X, PIT_X, d0, d1, PIT_FLOOR, DECK)
    mesh.ramp(0, 2*PIT_X, z0, d0, TER, PIT_FLOOR, TIMBER)
    mesh.ramp(0, 2*PIT_X, z1, d1, TER, PIT_FLOOR, TIMBER)
    for s in (-1, 1): wall(s*PIT_X, s*(PIT_X+.6), z0, z1, PIT_FLOOR-1, TER-1, HULL)
    fx, fz = FLAG
    b.prism(fx, fz, 1.8, 1.5, PIT_FLOOR, PIT_FLOOR+PLINTH, 12, METAL)
    b.prism(fx, fz, 1.2, 1.2, PIT_FLOOR+PLINTH, PIT_FLOOR+PLINTH+.02, 12, accent, solid=False)
    b.prism(fx, fz, .4, .4, PIT_FLOOR+PLINTH+.02, PIT_FLOOR+PLINTH+.04, 12, GLOW, solid=False)
    b.lamp((fx, PIT_FLOOR+2.4, fz), .6)
    # Sun-disc inlay round the plinth and a bronze-dark band on the pit walls.
    b.prism(fx, fz, 3.0, 3.0, PIT_FLOOR+.005, PIT_FLOOR+.02, 20, METAL, solid=False)
    b.prism(fx, fz, 2.6, 2.6, PIT_FLOOR+.02, PIT_FLOOR+.03, 20, DECK, solid=False)
    for s in (-1, 1):
        b.face_box('x', s*PIT_X, -s, d0, d1, PIT_FLOOR, PIT_FLOOR+.3, .1, METAL)
        b.face_quad('x', s*PIT_X, -s, d0, d1, PIT_FLOOR+1.0, PIT_FLOOR+1.3, accent)
        b.face_box('x', s*PIT_X, -s, z0, z1, TER-.25, TER+.02, .14, METAL)
    # Corner braziers on the rim: bowls of glow, lit for the bake.
    for x in (-PIT_X-1.4, PIT_X+1.4):
        for z in (z0-1.4, z1+1.4):
            b.prism(x, z, .5, .38, TER, TER+.9, 8, METAL)
            b.prism(x, z, .62, .5, TER+.9, TER+1.2, 8, METAL, solid=False)
            b.prism(x, z, .42, .42, TER+1.15, TER+1.2, 8, GLOW, solid=False)
            b.lamp((x, TER+1.8, z), .5)

    # --- Keep -----------------------------------------------------------------
    ix, iz0, iz1 = KX-WALL, KZ0+WALL, KZ1-WALL
    for s in (-1, 1): wall(s*ix, s*KX, KZ0, KZ1, TER, CEIL, HULL)
    for zf, zb in [(KZ0, iz0), (iz1, KZ1)]:
        for x0, x1 in [(-ix, DOOR[0]), (DOOR[1], ix)]: wall(x0, x1, zf, zb, TER, CEIL, HULL)
        wall(DOOR[0], DOOR[1], zf, zb, DOOR_TOP, CEIL, HULL)
    slab(-KX, KX, KZ0, KZ1, ROOF, HULL)
    for z0b, z1b in (FRONT_BAFFLE_Z, BACK_BAFFLE_Z): wall(-BAFFLE_X, BAFFLE_X, z0b, z1b, TER, CEIL, HULL)
    # Dressing: plaster liners, iron base and cornice, team stripe.
    b.dress('z', iz0, 1, _runs_without(-ix, ix, [DOOR]), TER, CEIL)
    b.dress('z', iz1, -1, _runs_without(-ix, ix, [DOOR]), TER, CEIL)
    for s in (-1, 1): b.dress('x', s*ix, -s, [(iz0, iz1)], TER, CEIL)
    b.dress('z', FRONT_BAFFLE_Z[0], -1, [(-BAFFLE_X, BAFFLE_X)], TER, CEIL, pilasters=False)
    b.dress('z', FRONT_BAFFLE_Z[1], 1, [(-BAFFLE_X, BAFFLE_X)], TER, CEIL)
    b.dress('z', BACK_BAFFLE_Z[0], -1, [(-BAFFLE_X, BAFFLE_X)], TER, CEIL)
    b.dress('z', BACK_BAFFLE_Z[1], 1, [(-BAFFLE_X, BAFFLE_X)], TER, CEIL, pilasters=False)
    for z, into_list in [(iz0, (1,)), (iz1, (-1,))]:
        for into in into_list:
            b.face_box('z', z, into, DOOR[0]-.35, DOOR[0], TER, DOOR_TOP, .12, METAL)
            b.face_box('z', z, into, DOOR[1], DOOR[1]+.35, TER, DOOR_TOP, .12, METAL)
            b.face_box('z', z, into, DOOR[0]-.35, DOOR[1]+.35, DOOR_TOP, DOOR_TOP+.35, .12, METAL)
    for z0b, z1b in (FRONT_BAFFLE_Z, BACK_BAFFLE_Z):
        for u in (-BAFFLE_X, BAFFLE_X):
            wall(u-.08, u+.08, z0b-.05, z1b+.05, TER, DOOR_TOP, METAL, False)
    b.ceiling_strip('x', (iz0+FRONT_BAFFLE_Z[0])/2, -ix+1, ix-1, CEIL, .45)
    b.ceiling_strip('x', (BACK_BAFFLE_Z[1]+iz1)/2, -ix+1, ix-1, CEIL, .45)
    for x in (-6, 6): b.ceiling_strip('z', x, FRONT_BAFFLE_Z[1]+1, BACK_BAFFLE_Z[0]-1, CEIL, .7)
    # Hall cover, inventories and the generator.
    wall(-.8, .8, -6.8, -5.2, TER, TER+2.6, HULL)
    wall(-.9, .9, -6.9, -5.1, TER+2.6, TER+2.74, METAL, False)
    wall(-.84, .84, -6.84, -5.16, TER+1.6, TER+1.78, accent, False)
    for z in (-10.0, -4.5): mesh.equipment('inventory', (-10, TER, z), team, circuit)
    generator = (10.0, TER, -6.0)
    mesh.equipment('generator', generator, team, circuit)
    wall(generator[0]-3.0, generator[0]+3.0, -8.6, -3.4, TER, TER+.03, TIMBER, False)
    # Roof: parapet with broken crenels, and the roof turret.
    t = .5
    wall(-KX, KX, KZ0, KZ0+t, ROOF, ROOF+.6, HULL)
    wall(-KX, KX, KZ1-t, KZ1, ROOF, ROOF+.6, HULL)
    for s in (-1, 1): wall(s*(KX-t), s*KX, KZ0+t, KZ1-t, ROOF, ROOF+.6, HULL)
    crenels('x', KZ1-t, KZ1, -KX+.6, KX-.6, ROOF+.6, (.9, .9, 0, .9, .5, .9))
    roof_turret = (0.0, ROOF, -12.5)
    mesh.equipment('turret', roof_turret, team, circuit)
    collar(*roof_turret, 2.2)

    # --- Pylon towers and facade --------------------------------------------
    p0, p1, pz0, pz1 = PYLON
    for s in (-1, 1):
        a, c = sorted((s*p0, s*p1))
        wall(a, c, pz0, pz1, FOOT, PYLON_TOP, HULL)
        a2, c2 = sorted((s*(p0+.8), s*(p1-.8)))
        wall(a2, c2, pz0+.8, pz1-.8, PYLON_TOP, PYLON_CAP, HULL)
        crenels('x', pz0, pz0+.6, min(a, c)+.3, max(a, c)-.3, PYLON_TOP, (.9, 0, .9))
        # Iron bands, a tall banner and slit windows (render only).
        for y in (TER+.4, 13.0, PYLON_TOP-.6):
            b.face_box('z', pz0, -1, a, c, y, y+.35, .15, METAL)
        cx = (a+c)/2
        mesh.quad((cx-1.4, TER+2.0, pz0-.05), (cx-1.4, 16.8, pz0-.05), (cx+1.4, 16.8, pz0-.05),
                  (cx+1.4, TER+2.0, pz0-.05), accent, False)
        b.face_box('z', pz0, -1, cx-.2, cx+.2, 17.2, 18.4, .05, GLOW)
        b.lamp((cx, 15, pz0-2.5), .7)
    # Keep front between the pylons: a lintel band, sun emblem over the door.
    fz0 = KZ0
    b.face_box('z', fz0, -1, -KX, KX, CEIL-.4, CEIL, .2, METAL)
    b.face_box('z', fz0, -1, DOOR[0]-.5, DOOR[1]+.5, DOOR_TOP, DOOR_TOP+.5, .25, METAL)
    ey = CEIL-2.2
    for r, mat in [(1.9, METAL), (1.5, GLOW), (.9, accent)]:
        pts = [(r*math.cos(i*math.tau/16), ey+r*math.sin(i*math.tau/16), fz0-.06-(1.9-r)*.02) for i in range(16)]
        for i in range(16):
            mesh.triangle([(0, ey, pts[i][2]), pts[(i+1) % 16], pts[i]], mat, False)
    b.lamp((0, ey, fz0-2.5), .7)
    for x in (DOOR[0]-1.2, DOOR[1]+1.2):
        wall(x-.2, x+.2, fz0-.4, fz0, TER+3.0, TER+3.3, METAL, False)
        wall(x-.14, x+.14, fz0-.36, fz0-.04, TER+3.3, TER+3.8, GLOW, False)
        b.lamp((x, TER+3.6, fz0-.7), .5)

    # --- Courtyard: arcades and curtain walls ---------------------------------
    for s in (-1, 1):
        for z in ARCADE_Z:
            b.prism(s*ARCADE_X, z, .62, .55, TER, ARCADE_ROOF-.6, 8, HULL, phase=math.pi/8)
            b.prism(s*ARCADE_X, z, .8, .8, ARCADE_ROOF-1.0, ARCADE_ROOF-.6, 8, HULL, phase=math.pi/8, solid=False)
        a, c = sorted((s*(ARCADE_X-.7), s*CURTAIN))
        slab(a, c, KZ1, BACK_WALL[0], ARCADE_ROOF, HULL, .6)
        b.face_box('x', s*(ARCADE_X-.7), -s, KZ1, BACK_WALL[0], ARCADE_ROOF-.6, ARCADE_ROOF-.3, .1, METAL)
        b.ceiling_strip('z', s*(ARCADE_X+1.8), KZ1+2, BACK_WALL[0]-2, ARCADE_ROOF-.6, .45, spacing=5)
        # Curtain: runs from the keep's side to the back wall, broken tops.
        a, c = sorted((s*CURTAIN, s*TX))
        seg = (BACK_WALL[0]-KZ0)/len(CURTAIN_TOPS)
        for i, top in enumerate(CURTAIN_TOPS if s > 0 else CURTAIN_TOPS[::-1]):
            za, zb = KZ0+i*seg, KZ0+(i+1)*seg
            wall(a, c, za, zb, TER-1, top, HULL)
        b.dress('x', s*CURTAIN, -s, [(KZ1, BACK_WALL[0])], TER, ARCADE_ROOF-.6, pilasters=False)
    # Back wall with the breach, jagged either side of it.
    bz0, bz1 = BACK_WALL
    for x0, x1, top in [(-TX, -9.0, 12.0), (-9.0, BREACH[0], 9.5), (BREACH[1], 9.0, 10.5), (9.0, TX, 12.8)]:
        wall(x0, x1, bz0, bz1, TER-1, top, HULL)
    for x0, x1, y in [(BREACH[0]-1.2, BREACH[0], 7.4), (BREACH[1], BREACH[1]+1.0, 8.0)]:
        wall(x0, x1, bz0, bz1, TER, y, HULL)

    # --- Access ramps ---------------------------------------------------------
    for ramp, fn in [(FRONT_RAMP, front_ramp_y), (BACK_RAMP, back_ramp_y)]:
        x0, x1, zg, zt = ramp
        mesh.ramp((x0+x1)/2, x1-x0, zg, zt, 0.0, TER, TIMBER)
        for x in (x0, x1): closed_side(x, *sorted((zg, zt)), fn)
        for x in (x0-.3, x1+.3):   # low stone kerbs (render only)
            ya, yb = fn(zg), fn(zt)
            mesh.quad((x, ya, zg), (x, yb, zt), (x, yb+.35, zt), (x, ya+.35, zg), METAL, False)
            mesh.quad((x, ya+.35, zg), (x, yb+.35, zt), (x, yb, zt), (x, ya, zg), METAL, False)

    # --- Watch tower ----------------------------------------------------------
    wx0, wx1, wz0, wz1 = TOWER
    wall(wx0, wx1, wz0, wz1, FOOT, TOWER_TOP, HULL)
    t = .5
    for x0, x1, za, zb in [(wx0, wx1, wz0, wz0+t), (wx0, wx1, wz1-t, wz1), (wx0, wx0+t, wz0+t, wz1-t),
                           (wx1-t, wx1, wz0+t, wz1-t)]:
        wall(x0, x1, za, zb, TOWER_TOP, TOWER_TOP+.6, HULL)
    crenels('x', wz0, wz0+t, wx0+.5, wx1-.5, TOWER_TOP+.6)
    crenels('z', wx0, wx0+t, wz0+.8, wz1-.8, TOWER_TOP+.6)
    for y in (TER+.4, 12.5, TOWER_TOP-.6):
        b.face_box('z', wz0, -1, wx0, wx1, y, y+.35, .15, METAL)
        b.face_box('x', wx0, -1, wz0, wz1, y, y+.35, .15, METAL)
    mesh.quad((wx0-.05, 8.0, wz1-2.5), (wx0-.05, 17.5, wz1-2.5), (wx0-.05, 17.5, wz0+2.5),
              (wx0-.05, 8.0, wz0+2.5), accent, False)
    sentry = (SENTRY[0], TOWER_TOP, SENTRY[1])
    mesh.equipment('turret', sentry, team, circuit)
    collar(*sentry, 2.2)
    mesh.equipment('sensor', (SENSOR[0], TOWER_TOP, SENSOR[1]), team, circuit)
    b.prism(SENSOR[0], SENSOR[1], 1.95, 1.8, TOWER_TOP, TOWER_TOP+1.1, 8, HULL, solid=False, cap=False)

    # --- Watch ruin, right flank ----------------------------------------------
    cx, cz = RUIN
    b.prism(cx, cz, RUIN_R+.8, RUIN_R, FOOT, RUIN_TOP, 8, HULL, phase=math.pi/8)
    b.prism(cx, cz, RUIN_R+.86, RUIN_R+.7, 1.2, 1.6, 8, METAL, phase=math.pi/8, solid=False, cap=False)
    r0, r1, zg, zt = ruin_ramp()
    ruin_ramp_y = lambda z: ramp_y((r0, r1, zg, zt), z, 0.0, RUIN_TOP)
    mesh.ramp(cx, r1-r0, zg, zt, 0.0, RUIN_TOP, TIMBER)
    for x in (r0, r1): closed_side(x, zt, zg, ruin_ramp_y)
    apothem = RUIN_R*math.cos(math.pi/8)
    verts = [(cx+(RUIN_R-.3)*math.cos(math.pi/8+i*math.tau/8), cz+(RUIN_R-.3)*math.sin(math.pi/8+i*math.tau/8))
             for i in range(8)]
    broken = (1.6, .8, 1.9, 0, 1.2, 2.2, 0, 1.4)       # a ruined, uneven crown
    for i in range(8):
        a, c = verts[i], verts[(i+1) % 8]
        if (a[1]+c[1])/2-cz > apothem*.9 or broken[i] <= 0: continue
        b.beam(a, c, .6, RUIN_TOP, RUIN_TOP+broken[i], HULL)
    ruin_turret = (cx, RUIN_TOP, cz)
    mesh.equipment('turret', ruin_turret, team, circuit, 'plasma')
    collar(*ruin_turret, 2.2)
    b.lamp((cx, RUIN_TOP+2, cz+3), .4)

    # --- Caravan dais: landing and deploy pad, left flank ---------------------
    px, pz = PAD
    H, tl = PAD_HALF, .5
    wall(px-H, px+H, pz-H, pz+H, PAD_TOP-1.2, PAD_TOP, DECK)
    wall(px-H+.4, px+H-.4, pz-H+.4, pz+H-.4, FOOT, PAD_TOP-1.2, HULL)
    b.face_box('z', pz-H, -1, px-H, px+H, PAD_TOP-1.2, PAD_TOP-.9, .12, METAL)
    for s in (-1, 1):
        for a, c in ((-H, -PAD_GAP), (PAD_GAP, H)):
            wall(px+a, px+c, pz+s*(H-tl), pz+s*H, PAD_TOP, PAD_TOP+PAD_LIP, HULL)
        for a, c in ((-H+tl, -PAD_GAP), (PAD_GAP, H-tl)):
            wall(px+s*(H-tl), px+s*H, pz+a, pz+c, PAD_TOP, PAD_TOP+PAD_LIP, HULL)
    def decal(x0, x1, z0_, z1_, mat):
        y = PAD_TOP+.02
        mesh.quad((px+x0, y, pz+z0_), (px+x0, y, pz+z1_), (px+x1, y, pz+z1_), (px+x1, y, pz+z0_), mat, False)
    k, w = 9.5, .5
    decal(-k, k, -k, -k+w, METAL); decal(-k, k, k-w, k, METAL)
    decal(-k, -k+w, -k+w, k-w, METAL); decal(k-w, k, -k+w, k-w, METAL)
    b.prism(px, pz, 3.2, 3.2, PAD_TOP+.01, PAD_TOP+.03, 16, accent, solid=False)
    b.prism(px, pz, 2.2, 2.2, PAD_TOP+.03, PAD_TOP+.04, 16, INTERIOR, solid=False)
    slots = []
    for sx, sz in PAD_SLOTS:
        hs, tt = 1.6, .22
        decal(sx-hs, sx+hs, sz-hs, sz-hs+tt, METAL); decal(sx-hs, sx+hs, sz+hs-tt, sz+hs, METAL)
        decal(sx-hs, sx-hs+tt, sz-hs+tt, sz+hs-tt, METAL); decal(sx+hs-tt, sx+hs, sz-hs+tt, sz+hs-tt, METAL)
        slots.append((px+sx, PAD_TOP, pz+sz))
    for sx in (-1, 1):
        for sz in (-1, 1):
            x, z = px+sx*(H-tl/2), pz+sz*(H-tl/2)
            b.prism(x, z, .12, .12, PAD_TOP+PAD_LIP, PAD_TOP+PAD_LIP+1.1, 6, METAL, solid=False)
            b.prism(x, z, .2, .2, PAD_TOP+PAD_LIP+1.1, PAD_TOP+PAD_LIP+1.35, 8, GLOW, solid=False)
            b.lamp((x, PAD_TOP+PAD_LIP+1.3, z), .5)

    # --- Apron cover: fallen drums and a broken wall --------------------------
    for x, z, r in DRUMS:
        b.prism(x, z, r, r, FOOT+2, 1.4, 10, HULL)
        b.prism(x, z, r+.04, r+.04, .5, .7, 10, METAL, solid=False, cap=False)
    fx_, fz_, length = FRONT_RUIN
    wall(fx_-length/2, fx_-length/6, fz_-.7, fz_+.7, FOOT+2, 4.6, HULL)
    wall(fx_-length/6, fx_+length/6, fz_-.7, fz_+.7, FOOT+2, 3.0, HULL)
    wall(fx_+length/6, fx_+length/2, fz_-.7, fz_+.7, FOOT+2, 1.6, HULL)

    lift = SPAWN_LIFT
    return {
        'flag': (fx, PIT_FLOOR+PLINTH+.05, fz),
        'spawn': (-4.0, TER+lift, -10.5),
        # (x, y, z, local yaw): yaw 0 faces the base front (-Z). Hall spawns
        # face along the hall toward the far baffle, never the near one.
        'spawn_points': [(-4.0, TER+lift, -10.5, math.pi), (4.0, TER+lift, -10.5, math.pi),
                         (-4.0, TER+lift, -2.5, 0.0), (4.0, TER+lift, -2.5, 0.0),
                         (18.2, TER+lift, 15.5, math.pi/2), (-18.2, TER+lift, 15.5, -math.pi/2),
                         (px-5, PAD_TOP+lift, pz+4, 0.0), (px+5, PAD_TOP+lift, pz+4, 0.0)],
        'entrances': [(0.0, TER+.2, KZ0), (0.0, TER+.2, KZ1), (0.0, .2, FRONT_RAMP[2]), (0.0, .2, BACK_RAMP[2])],
        'generator': generator,
        'roof_turret': roof_turret,
        'sentry': sentry,
        'ruin_turret': ruin_turret,
        'pad_deck': (px, PAD_TOP, pz),
        'deploy_slots': slots,
        'hall_view': (-11.5, TER+1.7, -11.8),
        'court_view': (-17.5, TER+1.7, 26.5),
        'exterior_view': (-46.0, 16.0, -78.0),
    }

