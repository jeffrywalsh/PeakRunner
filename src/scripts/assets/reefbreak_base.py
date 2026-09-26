"""Original Reefbreak team base: a freighter hovering HOVER metres over a
sandbar outside the reef, with its flag on a pad on the reef crest in front
of the bow (docs/reefbreak.md). Both teams get the same ship; the map's
point symmetry places them. Each team also has a ground outpost
(build_outpost) on an outer island behind its ship.

Local coordinates (X/Z horizontal, Y up, the hold floor Y=0, -Z is the bow
and faces the field, +X is starboard facing the field):
- Hull (HULL_X, BOW_Z..STERN_Z, a pointed bow to BOW_TIP): steel sides from
  the closed keel at KEEL to the open top deck at DECK_Y, with a keel fin
  and four glowing hover thrusters underneath. Players can ski beneath it.
- Hold: the long front compartment, lit, with two inventory stations and the
  eight spawns. Ways in: the port and starboard hatches and a torn breach in
  the port bow (each with a landing ledge, reached by jetting) and the deck
  hatch ramp from the top deck.
- Engine room behind the bulkhead: the generator, and exactly two ways in,
  the bulkhead door from the hold and the stern hatch (a ledge, by jet).
- Top deck: reached by the bow gangway, a ramp climbing from the flag pad
  on the reef crest up to the bow (the walking route, for heavy armor too),
  or up the deck hatch ramp from the hold; low gunwales round it.
- Bridge: a deckhouse over the engine room with the third inventory
  station; its roof, reached by an outside ramp on the port side, carries the
  turret and the sensor mast, with a funnel behind.
- The flag pad (FLAG) on the reef crest; the build places the reef beacon
  (build_beacon) beside it separately.

Nothing touches the ground but the gangway's legs, so no terrain holes
are needed. Dressing is
render-only and stays within the 0.52 m player radius of a solid surface.
If the caller's mesh has a `lamps` list, light fixtures add bake samples. No
source geometry is used.
"""
import math

from assets.structure_kit import Builder

ASSET_ID = 'reefbreak-ship-v1'
BEACON_ID = 'reefbreak-beacon-v1'
OUTPOST_ID = 'reefbreak-outpost-v1'

W = .8
HOVER = 14.0                             # hold floor above the sea
KEEL = -3.4                              # hull bottom
SAND = -HOVER-.25                        # the sandbar under the ship, just awash
PAD_Y = -6.0                             # flag pad on the reef crest, below the hold floor
DECK_Y, DECK_T = 7.0, 1.0
HULL_X = 11.0
BOW_Z, STERN_Z, BOW_TIP = -30.0, 34.0, -42.0
BULKHEAD = (13.2, 14.0)
HOLD_CEIL = DECK_Y-DECK_T
PORT_HATCH = (-18.0, -12.0, 4.6)         # z0, z1, top (port side, x = -HULL_X)
STAR_HATCH = (-16.0, -10.0, 4.6)         # starboard side
BREACH = (-27.0, -21.5, .4, 5.0)         # port bow: z0, z1, y0, y1
BULK_DOOR = (-2.2, 2.2, 4.6)
STERN_HATCH = (-3.0, 3.0, 4.6)
DECK_HATCH = (4.2, 9.4, -9.0, 8.8)       # x0, x1, z0, z1: open over the hatch ramp
HATCH_RAMP = (4.6, 9.0, -8.6, 8.4)       # x0, x1, z at the deck, z at the hold floor
LEDGE = 2.6                              # landing ledges outside the hatches
BRIDGE = (-8.0, 8.0, 18.0, 32.0)         # deckhouse outer x0, x1, z0, z1
BRIDGE_TOP, BRIDGE_ROOF_T = 13.6, .8
BRIDGE_DOOR = (-2.4, 2.4, DECK_Y+4.2)
ROOF_RAMP = (-10.8, -8.4, 5.0, 24.0)     # x0, x1, z at the deck, z at the roof
ROOF_LANDING = (-11.0, -8.0, 24.0, 32.0)
TURRET = (2.0, BRIDGE_TOP, 27.0)
SENSOR = (-4.0, BRIDGE_TOP, 29.0)
FUNNEL = (4.5, 30.5)
GEN = (0.0, 0.0, 24.0)
INVENTORIES = ((-7.0, 0.0, 10.0), (7.0, 0.0, 10.0), (-4.0, DECK_Y, 29.0))
SPAWNS = [(x, z) for z in (-20.0, -13.0) for x in (-6.0, -2.0, 2.0, 6.0)]
SPAWN_LIFT = 1.2
GANGWAY = (-2.0, 2.0, BOW_TIP+.5, -84.0)  # x0, x1, z at the bow (deck), z at the pad (PAD_Y)
FLAG_PAD_R = 7.0
FLAG = (0.0, PAD_Y, -92.0)
BEACON_AT = (30.0, -96.0)                # local (x, z) of the reef beacon's foot
# Beacon, in its own frame (ground Y=0 at its foot).
BEACON_R0, BEACON_R1, BEACON_TOP = 3.2, 2.2, 16.0
BEACON_LEDGE, BEACON_LEDGE_Y = 5.5, 11.0

HULL_MAT, DECKING, GRATE, METAL, GLOW, WOOD = 'concrete', 'panel', 'grate', 'trim', 'light', 'bark'


def ramp_y(ramp, z, y_a, y_b):
    _, _, za, zb = ramp
    t = min(max((z-za)/(zb-za), 0.0), 1.0)
    return y_a+(y_b-y_a)*t


def sites():
    """Terrain to blend toward: (shape, surface(x, z) -> local height,
    falloff). The sand is levelled round the hull; the flag pad and its
    approach are levelled on the reef crest."""
    sand = lambda x, z: SAND
    pad = lambda x, z: PAD_Y-.1
    return [(('rect', -HULL_X-8, HULL_X+8, BOW_TIP-4, STERN_Z+10), sand, 22),
            (('disc', FLAG[0], FLAG[2], FLAG_PAD_R+4), pad, 18)]


def _runs(lo, hi, gaps):
    out, u = [], lo
    for a, c in sorted(gaps):
        if a > u: out.append((u, a))
        u = max(u, c)
    if hi > u: out.append((u, hi))
    return out


def build(mesh, team, circuit):
    if team not in (0, 1) or not circuit: raise ValueError('team and circuit required')
    accent = ['ember', 'glacier'][team]
    b = Builder(mesh, accent, interior=WOOD, metal=METAL, glow=GLOW)
    wall, slab = b.wall, b.slab

    def panel_wall(axis, plane0, plane1, u0, u1, y0, y1, openings=(), mat=HULL_MAT):
        cuts = sorted({u0, u1, *(v for o in openings for v in o[:2] if u0 < v < u1)})
        for a, c in zip(cuts, cuts[1:]):
            mid = (a+c)/2
            gaps = [(o[2], o[3]) for o in openings if o[0] <= mid <= o[1]]
            for ya, yb in _runs(y0, y1, gaps):
                if axis == 'z': wall(a, c, plane0, plane1, ya, yb, mat)
                else: wall(plane0, plane1, a, c, ya, yb, mat)

    def closed_side(x, z0, z1, surface, bottom):
        u0, u1 = surface(z0)-.6, surface(z1)-.6
        if u0 < bottom:
            z0 = z0+(bottom-u0)*(z1-z0)/(u1-u0); u0 = bottom
        if u1 < bottom:
            z1 = z1+(bottom-u1)*(z0-z1)/(u0-u1); u1 = bottom
        pts = [(x, bottom, z0), (x, u0, z0), (x, u1, z1), (x, bottom, z1)]
        pts = [q for i, q in enumerate(pts) if q != pts[i-1]]
        if len(pts) == 3: mesh.triangle(pts, HULL_MAT); mesh.triangle(pts[::-1], HULL_MAT)
        else: mesh.quad(*pts, HULL_MAT); mesh.quad(*pts[::-1], HULL_MAT)

    def ramp(r, y_a, y_b, bottom, mat=GRATE, sides=True):
        x0, x1, za, zb = r
        mesh.ramp((x0+x1)/2, x1-x0, za, zb, y_a, y_b, mat)
        if sides:
            f = lambda z: ramp_y(r, z, y_a, y_b)
            for x in (x0, x1): closed_side(x, za, zb, f, bottom)

    hx = HULL_X; ix = hx-W
    s0, s1 = BULKHEAD

    # --- Hull sides, stern and the pointed bow ------------------------------
    panel_wall('x', -hx, -ix, BOW_Z, STERN_Z, KEEL, DECK_Y,
               [(PORT_HATCH[0], PORT_HATCH[1], 0.0, PORT_HATCH[2]), (BREACH[0], BREACH[1], BREACH[2], BREACH[3])])
    panel_wall('x', ix, hx, BOW_Z, STERN_Z, KEEL, DECK_Y, [(STAR_HATCH[0], STAR_HATCH[1], 0.0, STAR_HATCH[2])])
    panel_wall('z', STERN_Z-W, STERN_Z, -ix, ix, KEEL, DECK_Y, [(STERN_HATCH[0], STERN_HATCH[1], 0.0, STERN_HATCH[2])])
    for sx in (-1, 1):
        b.beam((sx*hx+(-sx*W/2), BOW_Z), (0.0, BOW_TIP), W, KEEL, DECK_Y, HULL_MAT)
    # The bow's forward wall closes the hold at BOW_Z; the bow point is a
    # solid forepeak with the forecastle deck on top.
    wall(-ix, ix, BOW_Z, BOW_Z+W, KEEL, DECK_Y, HULL_MAT)
    for side in (-1, 1):
        tri = [(0.0, DECK_Y, BOW_TIP), (side*hx, DECK_Y, BOW_Z), (0.0, DECK_Y, BOW_Z)]
        mesh.triangle(tri, DECKING); mesh.triangle(tri[::-1], DECKING)
    # Closed keel, a keel fin and four hover thrusters.
    slab(-hx, hx, BOW_Z, STERN_Z, KEEL+.8, HULL_MAT, .8)
    for side in (-1, 1):
        tri = [(0.0, KEEL, BOW_TIP), (side*hx, KEEL, BOW_Z), (0.0, KEEL, BOW_Z)]
        mesh.triangle(tri, HULL_MAT); mesh.triangle(tri[::-1], HULL_MAT)
    b.wall(-.6, .6, BOW_Z+4.0, STERN_Z-6.0, KEEL-2.2, KEEL, METAL)
    for x, z in ((-7.0, -20.0), (7.0, -20.0), (-7.0, 22.0), (7.0, 22.0)):
        b.prism(x, z, 2.2, 2.6, KEEL-1.2, KEEL, 12, METAL)
        b.prism(x, z, 1.7, 1.7, KEEL-1.25, KEEL-1.2, 12, GLOW, solid=False)
        b.lamp((x, KEEL-2.5, z), 1.0)
    # Team band and a band along both sides.
    for x, into in ((-hx, -1), (hx, 1)):
        runs = _runs(BOW_Z, STERN_Z, [PORT_HATCH[:2], BREACH[:2]] if x < 0 else [STAR_HATCH[:2]])
        for u0, u1 in runs:
            b.face_quad('x', x, into, u0, u1, DECK_Y-1.6, DECK_Y-.8, accent, off=.05)
            b.face_box('x', x, into, u0, u1, -.4, -.1, .1, METAL)
    # Torn plate round the breach: a few bent render-only shards.
    z0, z1, y0, y1 = BREACH
    for k, (zz, yy, dz, dy) in enumerate(((z0, y0+1.0, -.8, .6), (z1, y1-.8, .9, -.5), ((z0+z1)/2, y1, .2, .8), (z0+.6, y1-.3, -.5, .7))):
        mesh.quad((-hx-.06, yy, zz), (-hx-.06-.6, yy+dy, zz+dz), (-hx-.06-.6, yy+dy+.9, zz+dz*.5), (-hx-.06, yy+1.0, zz-.2), METAL, False)

    # --- Hold ------------------------------------------------------------
    slab(-ix, ix, BOW_Z+W, s0, 0.0, DECKING)
    panel_wall('z', s0, s1, -ix, ix, 0.0, HOLD_CEIL, [(BULK_DOOR[0], BULK_DOOR[1], 0.0, BULK_DOOR[2])])
    b.dress('x', -ix, 1, _runs(BOW_Z+W, s0, [PORT_HATCH[:2], BREACH[:2]]), 0.0, HOLD_CEIL)
    b.dress('x', ix, -1, _runs(BOW_Z+W, s0, [STAR_HATCH[:2]]), 0.0, HOLD_CEIL)
    b.dress('z', BOW_Z+W, 1, [(-ix, ix)], 0.0, HOLD_CEIL)
    b.dress('z', s0, -1, _runs(-ix, ix, [BULK_DOOR[:2]]), 0.0, HOLD_CEIL)
    for x in (-5.0, 0.0): b.ceiling_strip('z', x, BOW_Z+2, s0-1, HOLD_CEIL, .9)
    for x, y, z in INVENTORIES[:2]: mesh.equipment('inventory', (x, y, z), team, circuit)
    # Cargo: a few containers along the port side for cover.
    for z, h in ((-6.0, 2.6), (1.5, 2.6)):
        b.wall(-ix+.4, -ix+3.0, z-2.4, z+2.4, 0.0, h, accent if z < 0 else METAL)
    # Hatch ramp from the deck down into the hold.
    ramp(HATCH_RAMP, DECK_Y, 0.0, 0.0)
    for x0, x1, z0, z1 in ((DECK_HATCH[0]-.2, DECK_HATCH[0], DECK_HATCH[2], DECK_HATCH[3]),
                           (DECK_HATCH[1], DECK_HATCH[1]+.2, DECK_HATCH[2], DECK_HATCH[3])):
        wall(x0, x1, z0, z1, DECK_Y, DECK_Y+1.0, METAL)

    # --- Engine room -----------------------------------------------------
    slab(-ix, ix, s1, STERN_Z-W, 0.0, DECKING)
    b.dress('x', -ix, 1, [(s1, STERN_Z-W)], 0.0, HOLD_CEIL)
    b.dress('x', ix, -1, [(s1, STERN_Z-W)], 0.0, HOLD_CEIL)
    b.dress('z', STERN_Z-W, -1, _runs(-ix, ix, [STERN_HATCH[:2]]), 0.0, HOLD_CEIL)
    mesh.equipment('generator', GEN, team, circuit)
    b.ceiling_strip('z', -6.0, s1+1, STERN_Z-2, HOLD_CEIL, .9)
    b.ceiling_strip('z', 6.0, s1+1, STERN_Z-2, HOLD_CEIL, .9)
    # Pipes along the engine room walls.
    for x in (-ix+.35, ix-.35):
        mesh.box((x, 4.6, (s1+STERN_Z)/2), (.3, .3, STERN_Z-s1-2), METAL, False)

    # --- Landing ledges outside the hatches (reached by jetting) ---------
    for x0, x1, (z0, z1, *_), y in ((-hx-LEDGE, -hx, PORT_HATCH, 0.0), (hx, hx+LEDGE, STAR_HATCH, 0.0),
                                   (-hx-LEDGE, -hx, BREACH, BREACH[2])):
        slab(x0, x1, z0-1.0, z1+1.0, y, GRATE, .4)
        edge = x0 if x0 < 0 else x1
        mesh.box((edge, y+.08, (z0+z1)/2), (.08, .06, z1-z0+2.0), GLOW, False)
    slab(STERN_HATCH[0]-1.0, STERN_HATCH[1]+1.0, STERN_Z, STERN_Z+LEDGE+.6, 0.0, GRATE, .4)
    mesh.box(((STERN_HATCH[0]+STERN_HATCH[1])/2, .08, STERN_Z+LEDGE+.6), (STERN_HATCH[1]-STERN_HATCH[0]+2.0, .06, .08), GLOW, False)

    # --- Top deck, gunwales and the starboard gangway ---------------------
    dx0, dx1, dz0, dz1 = DECK_HATCH
    # The deck, open over the hatch ramp.
    slab(-hx, hx, BOW_Z, dz0, DECK_Y, DECKING, DECK_T)
    slab(-hx, hx, dz1, STERN_Z, DECK_Y, DECKING, DECK_T)
    slab(-hx, dx0, dz0, dz1, DECK_Y, DECKING, DECK_T)
    slab(dx1, hx, dz0, dz1, DECK_Y, DECKING, DECK_T)
    gun = .9
    wall(-hx, -hx+.3, BOW_Z, STERN_Z, DECK_Y, DECK_Y+gun, METAL)
    wall(hx-.3, hx, BOW_Z, STERN_Z, DECK_Y, DECK_Y+gun, METAL)
    wall(-hx, hx, STERN_Z-.3, STERN_Z, DECK_Y, DECK_Y+gun, METAL)
    for x, z in ((-hx, -10.0), (-hx, 10.0), (hx, -20.0), (hx, 20.0)):
        b.lamp((x*.8, DECK_Y+2.5, z), .8)

    # --- Bridge deckhouse --------------------------------------------------
    bx0, bx1, bz0, bz1 = BRIDGE
    top = BRIDGE_TOP-BRIDGE_ROOF_T
    panel_wall('z', bz0, bz0+W, bx0, bx1, DECK_Y, top, [(BRIDGE_DOOR[0], BRIDGE_DOOR[1], DECK_Y, BRIDGE_DOOR[2])])
    panel_wall('z', bz1-W, bz1, bx0, bx1, DECK_Y, top)
    panel_wall('x', bx0, bx0+W, bz0+W, bz1-W, DECK_Y, top)
    panel_wall('x', bx1-W, bx1, bz0+W, bz1-W, DECK_Y, top)
    slab(bx0, bx1, bz0, bz1, BRIDGE_TOP, HULL_MAT, BRIDGE_ROOF_T)
    # Bridge windows: a dark glazed band on the front and sides (render-only).
    for axis, plane, into, u0, u1 in (('z', bz0, -1, bx0+.8, BRIDGE_DOOR[0]-.4), ('z', bz0, -1, BRIDGE_DOOR[1]+.4, bx1-.8),
                                      ('x', bx0, -1, bz0+.8, bz1-.8), ('x', bx1, 1, bz0+.8, bz1-.8)):
        b.face_quad(axis, plane, into, u0, u1, top-2.0, top-.6, METAL, off=.04)
        b.face_box(axis, plane, into, u0, u1, top-2.1, top-1.95, .06, GLOW)
    b.ceiling_strip('x', (bz0+bz1)/2, bx0+1, bx1-1, top, .8)
    mesh.equipment('inventory', INVENTORIES[2], team, circuit)
    b.face_quad('z', bz0, -1, BRIDGE_DOOR[0]-1.2, BRIDGE_DOOR[1]+1.2, BRIDGE_DOOR[2]+.3, top-2.2, accent, off=.05)
    # Roof ramp, landing, turret, sensor mast and funnel.
    ramp(ROOF_RAMP, DECK_Y, BRIDGE_TOP, DECK_Y)
    lx0, lx1, lz0, lz1 = ROOF_LANDING
    slab(lx0, lx1, lz0, lz1, BRIDGE_TOP, GRATE, .6)
    mesh.equipment('turret', TURRET, team, circuit)
    sx, sy, sz = SENSOR
    b.prism(sx, sz, .35, .25, sy, sy+4.5, 6, METAL)
    mesh.equipment('sensor', (sx, sy+4.5, sz), team, circuit)
    fx, fz = FUNNEL
    b.prism(fx, fz, 2.0, 1.7, BRIDGE_TOP, BRIDGE_TOP+5.0, 10, HULL_MAT)
    b.prism(fx, fz, 2.02, 1.93, BRIDGE_TOP+3.4, BRIDGE_TOP+4.3, 10, accent, solid=False, cap=False)
    b.lamp((0.0, BRIDGE_TOP+1.5, bz0-1.0), 1.0)

    # --- Bow gangway: a ramp from the flag pad up to the bow deck --------
    gx0, gx1, gz0, gz1 = GANGWAY
    ramp(GANGWAY, DECK_Y, PAD_Y, 0.0, sides=False)
    gy = lambda z: ramp_y(GANGWAY, z, DECK_Y, PAD_Y)
    for x in (gx0-.1, gx1+.1):
        for k in range(8):
            za, zb = gz0+(gz1-gz0)*k/8, gz0+(gz1-gz0)*(k+1)/8
            mesh.quad((x-.05, gy(za)+1.0, za), (x+.05, gy(za)+1.0, za), (x+.05, gy(zb)+1.0, zb), (x-.05, gy(zb)+1.0, zb), METAL, False)
            mesh.quad((x-.05, gy(za)+.05, za), (x+.05, gy(za)+.05, za), (x+.05, gy(zb)+.05, zb), (x-.05, gy(zb)+.05, zb), GLOW, False)
    for z in (gz0-8.0, (gz0+gz1)/2, gz1+8.0):
        wall(-.5, .5, z-.5, z+.5, SAND-2.0, gy(z)-.7, METAL)
    fx, fy, fz = FLAG
    b.prism(fx, fz, FLAG_PAD_R, FLAG_PAD_R, fy-1.2, fy, 12, DECKING)
    b.prism(fx, fz, FLAG_PAD_R+.05, FLAG_PAD_R+.05, fy-.25, fy-.1, 12, GLOW, solid=False, cap=False)
    b.prism(fx, fz, 2.4, 2.2, fy, fy+.04, 12, accent, solid=False)
    b.lamp((fx, fy+2.0, fz), 1.0)

    spawn_points = [((x, SPAWN_LIFT, z), 0.0) for x, z in SPAWNS]
    return dict(flag=FLAG, spawn=spawn_points[0][0], spawn_points=[(*p, yaw) for p, yaw in spawn_points],
                generator=GEN, hold=(0.0, 0.0, -8.0), deck=(0.0, DECK_Y, 0.0),
                bridge=(0.0, DECK_Y, (bz0+bz1)/2), roof=(0.0, BRIDGE_TOP, 26.0),
                port_hatch=(-hx-1.3, 0.0, sum(PORT_HATCH[:2])/2), star_hatch=(hx+1.3, 0.0, sum(STAR_HATCH[:2])/2),
                breach=(-hx-1.3, BREACH[2], sum(BREACH[:2])/2), stern_hatch=(0.0, 0.0, STERN_Z+1.5),
                gangway=(0.0, ramp_y(GANGWAY, (gz0+gz1)/2, DECK_Y, PAD_Y), (gz0+gz1)/2),
                beacon=(BEACON_AT[0], 0.0, BEACON_AT[1]))


def build_beacon(mesh, team, circuit):
    """A short reef beacon beside the flag: a tapered tower with a turret
    ledge. Local ground Y=0 at its foot; the tower runs 3 m below it."""
    accent = ['ember', 'glacier'][team]
    b = Builder(mesh, accent, interior=WOOD, metal=METAL, glow=GLOW)
    b.prism(0, 0, BEACON_R0, BEACON_R1, -3.0, BEACON_TOP, 8, HULL_MAT, phase=math.pi/8)
    b.prism(0, 0, BEACON_LEDGE, BEACON_LEDGE, BEACON_LEDGE_Y-.7, BEACON_LEDGE_Y, 12, METAL)
    b.prism(0, 0, BEACON_LEDGE+.05, BEACON_LEDGE+.05, BEACON_LEDGE_Y-.3, BEACON_LEDGE_Y-.18, 12, GLOW, solid=False, cap=False)
    taper = lambda y: BEACON_R0+(BEACON_R1-BEACON_R0)*(y+3.0)/(BEACON_TOP+3.0)
    for y0, y1, mat in ((3.0, 4.2, accent), (8.0, 8.2, GLOW), (13.5, 14.7, accent)):
        r = taper(y0)+.04
        b.prism(0, 0, r, r, y0, y1, 8, mat, phase=math.pi/8, solid=False, cap=False)
    b.prism(0, 0, BEACON_R1+.4, BEACON_R1+.4, BEACON_TOP, BEACON_TOP+.6, 8, METAL, phase=math.pi/8)
    b.prism(0, 0, 1.0, .8, BEACON_TOP+.6, BEACON_TOP+1.8, 8, GLOW, solid=False)
    mesh.equipment('turret', (0.0, BEACON_LEDGE_Y, -(BEACON_LEDGE-1.2)), team, circuit)
    b.lamp((0.0, BEACON_TOP+1.2, 0.0), 1.2)
    return dict(ledge=(0.0, BEACON_LEDGE_Y, BEACON_LEDGE-1.5), top=(0.0, BEACON_TOP+.6, 0.0))


# Outpost, in its own frame (floor Y=0, -Z is the door side).
OUT_X, OUT_Z0, OUT_Z1, OUT_H = 10.0, -7.0, 7.0, 4.8
OUT_DOOR = (-2.8, 2.8, 3.6)
OUT_SIDE_DOOR = (-2.0, 2.0, 3.6)           # z range on the +X wall
OUT_INV = ((-5.0, 0.0, 3.4), (5.0, 0.0, 3.4))
OUT_SPAWNS = ((-5.0, -3.0), (0.0, -3.0), (5.0, -3.0), (0.0, 1.0))


def outpost_site():
    return ('rect', -OUT_X-6, OUT_X+6, OUT_Z0-8, OUT_Z1+5)


def build_outpost(mesh, team, circuit):
    """A ground outpost on an outer island: a low bunker with a front and a
    side door, two inventory stations and four spawns. The roof is reached
    by jetting."""
    accent = ['ember', 'glacier'][team]
    b = Builder(mesh, accent, interior=WOOD, metal=METAL, glow=GLOW)
    wall, slab = b.wall, b.slab
    x0, x1, z0, z1 = -OUT_X, OUT_X, OUT_Z0, OUT_Z1
    foot = -2.5
    for a, c in _runs(x0, x1, [OUT_DOOR[:2]]):
        wall(a, c, z0, z0+W, foot, OUT_H, HULL_MAT)
    wall(OUT_DOOR[0], OUT_DOOR[1], z0, z0+W, OUT_DOOR[2], OUT_H, HULL_MAT)
    wall(x0, x1, z1-W, z1, foot, OUT_H, HULL_MAT)
    wall(x0, x0+W, z0+W, z1-W, foot, OUT_H, HULL_MAT)
    for a, c in _runs(z0+W, z1-W, [OUT_SIDE_DOOR[:2]]):
        wall(x1-W, x1, a, c, foot, OUT_H, HULL_MAT)
    wall(x1-W, x1, OUT_SIDE_DOOR[0], OUT_SIDE_DOOR[1], OUT_SIDE_DOOR[2], OUT_H, HULL_MAT)
    slab(x0, x1, z0, z1, OUT_H+.7, HULL_MAT, .7)
    slab(x0+W, x1-W, z0+W, z1-W, 0.0, DECKING, .8)
    b.dress('z', z1-W, -1, [(x0+W, x1-W)], 0.0, OUT_H)
    b.dress('x', x0+W, 1, [(z0+W, z1-W)], 0.0, OUT_H)
    b.ceiling_strip('x', 0.0, x0+2, x1-2, OUT_H, .9)
    for p in OUT_INV: mesh.equipment('inventory', p, team, circuit)
    b.face_quad('z', z0, -1, OUT_DOOR[0]-1.5, OUT_DOOR[1]+1.5, OUT_DOOR[2]+.25, OUT_H-.2, accent, off=.05)
    for u in (OUT_DOOR[0], OUT_DOOR[1]):
        b.face_box('z', z0, -1, u-.12, u+.12, 0.0, OUT_DOOR[2], .06, GLOW)
    b.face_box('z', z0, -1, x0, x1, OUT_H+.5, OUT_H+.65, .06, GLOW)
    b.lamp((0.0, OUT_DOOR[2]+.6, z0-1.0), .8)
    spawns = [((x, SPAWN_LIFT, z), 0.0) for x, z in OUT_SPAWNS]
    return dict(inside=(0.0, 0.0, 0.0), door=(0.0, 0.0, z0-1.5), roof=(0.0, OUT_H+.7, 0.0),
                spawn_points=[(*p, yaw) for p, yaw in spawns])
