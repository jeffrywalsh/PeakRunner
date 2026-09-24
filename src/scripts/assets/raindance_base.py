"""Old Holler base (asset raindance-base-v4; the map key stays `raindance`).

Same layout as the original kit base (basement hall, atrium, wide ski ramps,
front roof deck) with the pipeline checklist applied, and the flag in a
bishop-shaped tower behind the deck (v4, replacing the solid service spire
and the exposed roof flag stand):

- The tower's lower half is solid; its chamber floor is 9 m above the roof,
  with the flag on it. Three ways in: a front door (-Z), a side door (+X),
  and a slit cut diagonally through the mitre's back-left face. An L-shaped
  baffle inside the doors blocks turret sightlines across the chamber.
- The chamber is reached by jetting: from the front deck up to the ledge
  round the collar, or over the mitre and in through the slit.

The earlier cleanup items still hold:

- Walls meet at the corners instead of overlapping, and the roof sits on
  the walls with eaves, so no two visible faces share a plane (z-fighting).
- Each hall ramp climbs to an opening in its roof half instead of into the
  roof slab, and the outer shoulder ramps end flush with the roof edge.
- Where a ramp's underside is lower than 2.4 m above the floor beneath it,
  a solid closure stops players walking under it.
- The flag tower is sealed underneath, so it cannot be entered from below.
- Hall walls get liners, baseboards, cornices, pilasters and a team stripe;
  lit strips under the roof replace the old floating wall lights.
- Eight spawn points per team, 1.2 m above solid floor, facing open space.
- The generator sits in a basement under the hall with exactly two ways in:
  a stair down from the atrium floor, and a service stair that climbs from
  a passage behind the basement to a shed against the hall's back wall.
  Everything underground stays inside the hall's existing terrain cut
  (local |x| < 32, -56 < z < 32; the 8 m cells align with the base origin).

Local coordinates: Y up, the entrance faces local -Z. Hall floor at -10,
roof top at 8.6, ground level at 0.
"""
import math

from assets.structure_kit import Builder

ASSET_ID = 'raindance-base-v4'
FLOOR, WALL_TOP, ROOF_TOP, APRON_TOP = -10.0, 7.4, 8.6, .03
LIFT = 1.2
UNDER, HEADROOM = .6, 2.4
# Openings in each roof half above the upper end of its hall ramp.
HOLE_X, HOLE_Z = (15.5, 24.5), (10.0, 19.0)
# Basement: floor top, ceiling (the hall slab's underside) and interior box.
B_FLOOR, B_CEIL = -18.0, FLOOR-1.2
B_X, B_Z0, B_Z1 = 11.0, -6.0, 24.0
# Atrium stair: 4 m wide at x = 0, down from the hall floor to the basement.
STAIR_W, STAIR_Z1 = 4.0, 8.4
OPEN_Z1 = 1.5            # hall-floor opening ends where stair headroom passes 2.6 m
# Service route: door in the basement's back wall, passage and landing,
# then a stair along +x in the rear strip up to a shed at ground level.
DOOR_X, DOOR_TOP = (-5.0, -1.0), -13.5
STRIP_Z = (28.0, 31.6)
SERVICE_X = (-1.0, 30.2)
TUNNEL_CLEAR = 4.5
CEIL_END = 19.6          # where the rear apron ends and the shed begins
SHED_X1, SHED_ROOF, DOOR_LINTEL = 32.0, 3.4, 3.0
# Bishop flag tower: a lathed body on the roof behind the front deck. Heights
# are above ROOF_TOP; the lower half is solid and the chamber above it is hollow.
TZ, SIDES, PHASE = 20.0, 20, math.pi/20      # panel 14 faces -Z, panel 19 faces +X
CH_FLOOR = ROOF_TOP+9.0                      # chamber floor and outer ledge top
R_OUT, R_IN, R_LEDGE = 5.8, 5.2, 7.4         # stem outer / inner radius at the floor, ledge rim
DOOR_PANELS = (14, 19)                       # front (-Z) and side (+X) doors
CH_LINTEL = CH_FLOOR+3.4                     # both doors are one panel (1.6 m) wide, 3.4 m tall
BAFFLE = 3.0                                 # the L-shaped baffle's walls sit 3 m from the axis
BAFFLE_TOP = CH_FLOOR+4.2
R_BULB = 6.5                                 # widest part of the mitre
SLIT_DIR = (-math.sqrt(.5), math.sqrt(.5))   # the mitre slit faces back-left (-X, +Z)
SLIT_Y, SLIT_W, SLIT_TILT = ROOF_TOP+18.2, 3.6, math.radians(40)


def close_under(mesh, x, width, z0, z1, y0, y1, floor, mat):
    """Solid closure under an ascending ramp wherever its underside is lower
    than HEADROOM above `floor`: two side panels and an end wall."""
    rise = (y1-y0)/(z1-z0)
    under = lambda z: y0+rise*(z-z0)-UNDER
    za = max(z0, z0+(floor-(y0-UNDER))/rise)
    zb = min(z1, z0+(floor+HEADROOM-(y0-UNDER))/rise)
    if zb <= za: return
    for sx in (x-width/2, x+width/2):
        mesh.triangle([(sx, floor, za), (sx, floor, zb), (sx, under(zb), zb)], mat)
    xa, xb = x-width/2, x+width/2
    mesh.quad((xa, floor, zb), (xb, floor, zb), (xb, under(zb), zb), (xa, under(zb), zb), mat)


def roof_half(mesh, side):
    """One roof half (x 12..32.6 mirrored) with its stairwell opening."""
    x0, x1 = sorted((side*12, side*32.6))
    hx0, hx1 = sorted((side*HOLE_X[0], side*HOLE_X[1]))
    y = (WALL_TOP+ROOF_TOP)/2; t = ROOF_TOP-WALL_TOP

    def slab(a, b, c, d):
        mesh.box(((a+b)/2, y, (c+d)/2), (b-a, t, d-c), 'panel')
    slab(x0, x1, -28, HOLE_Z[0])
    slab(x0, x1, HOLE_Z[1], 28.6)
    slab(x0, hx0, HOLE_Z[0], HOLE_Z[1])
    slab(hx1, x1, HOLE_Z[0], HOLE_Z[1])


def cap(mesh, p, r, sides, mat):
    x, y, z = p
    for i in range(sides):
        a, b = i*math.tau/sides, (i+1)*math.tau/sides
        mesh.triangle([(x, y, z), (x+r*math.cos(b), y, z+r*math.sin(b)), (x+r*math.cos(a), y, z+r*math.sin(a))], mat)


def ramp_x(mesh, z0, z1, x0, x1, y0, y1, mat, solid=True):
    """A ramp climbing along +x (the kit's ramp runs along z): walking
    surface, a 0.6 m slab beneath it and closed long sides."""
    a, b, c, d = (x0, y0, z0), (x0, y0, z1), (x1, y1, z1), (x1, y1, z0)
    aa, bb, cc, dd = [(p[0], p[1]-UNDER, p[2]) for p in (a, b, c, d)]
    mesh.quad(a, b, c, d, mat, solid)
    mesh.quad(aa, dd, cc, bb, 'trim', solid)
    mesh.quad(a, d, dd, aa, 'trim', solid); mesh.quad(b, bb, cc, c, 'trim', solid)


def service_y(x):
    """Walking surface of the service stair, from B_FLOOR up to ground."""
    return B_FLOOR+(0-B_FLOOR)*(x-SERVICE_X[0])/(SERVICE_X[1]-SERVICE_X[0])


def basement(mesh, b, lit):
    """Generator basement under the hall and its two routes (see module doc)."""
    wall = lambda x0, x1, z0, z1, y0, y1, m='concrete': b.wall(x0, x1, z0, z1, y0, y1, m)
    # Room: floor slab, side walls on the slab, north and back walls between.
    wall(-B_X-1, B_X+1, B_Z0-1, B_Z1+1, B_FLOOR-1, B_FLOOR, 'grate')
    for s in (-1, 1): wall(s*B_X, s*(B_X+1), B_Z0-1, B_Z1+1, B_FLOOR, B_CEIL)
    wall(-B_X, B_X, B_Z0-1, B_Z0, B_FLOOR, B_CEIL)
    wall(-B_X, DOOR_X[0], B_Z1, B_Z1+1, B_FLOOR, B_CEIL)
    wall(DOOR_X[1], B_X, B_Z1, B_Z1+1, B_FLOOR, B_CEIL)
    wall(*DOOR_X, B_Z1, B_Z1+1, DOOR_TOP, B_CEIL, 'trim')
    # Atrium stair, sealed underneath (the wedge meets the north wall).
    h = STAIR_W/2
    mesh.ramp(0, STAIR_W, B_Z0, STAIR_Z1, FLOOR, B_FLOOR)
    toe = B_Z0+(STAIR_Z1-B_Z0)*(FLOOR-UNDER-B_FLOOR)/(FLOOR-B_FLOOR)
    mesh.triangle([(-h, B_FLOOR, B_Z0), (-h, B_FLOOR, toe), (-h, FLOOR-UNDER, B_Z0)], 'trim')
    mesh.triangle([(h, B_FLOOR, B_Z0), (h, FLOOR-UNDER, B_Z0), (h, B_FLOOR, toe)], 'trim')
    # Rails round the atrium opening (the stair side is open).
    for s in (-1, 1): wall(s*h, s*(h+.2), B_Z0, OPEN_Z1, FLOOR, FLOOR+1, 'trim')
    wall(-h-.2, h+.2, OPEN_Z1, OPEN_Z1+.2, FLOOR, FLOOR+1, 'trim')
    # Passage out of the back door, then a landing at the service stair's foot.
    px0, px1 = DOOR_X[0]-.4, SERVICE_X[0]
    wall(px0, px1, B_Z1+1, STRIP_Z[1], B_FLOOR-1, B_FLOOR, 'grate')
    wall(px0, DOOR_X[0], B_Z1+1, STRIP_Z[1], B_FLOOR, B_CEIL)
    wall(px1, px1+.4, B_Z1+1, STRIP_Z[0]-.4, B_FLOOR, B_CEIL)
    wall(*DOOR_X, STRIP_Z[0]-.4, STRIP_Z[0], DOOR_TOP, B_CEIL, 'trim')
    wall(*DOOR_X, *STRIP_Z, DOOR_TOP, DOOR_TOP+.6, 'trim')
    # Service stair under a sloped ceiling, walled on both long sides.
    x0, x1 = SERVICE_X
    ramp_x(mesh, *STRIP_Z, x0, x1, B_FLOOR, 0, 'grate')
    ramp_x(mesh, *STRIP_Z, x0, CEIL_END, B_FLOOR+TUNNEL_CLEAR+UNDER, service_y(CEIL_END)+TUNNEL_CLEAR+UNDER, 'panel')
    wall(x0, CEIL_END, STRIP_Z[0]-.4, STRIP_Z[0], B_FLOOR-1, B_CEIL)
    wall(px0, CEIL_END, STRIP_Z[1], 32, B_FLOOR-1, APRON_TOP-.6)
    # Shed over the stair's top end, its door facing away from the hall.
    wall(CEIL_END, CEIL_END+.4, *STRIP_Z, service_y(CEIL_END)+TUNNEL_CLEAR, SHED_ROOF)
    wall(CEIL_END, SHED_X1, STRIP_Z[1], 32, B_FLOOR-1, SHED_ROOF)
    wall(CEIL_END, SHED_X1+.3, STRIP_Z[0], 32.3, SHED_ROOF, SHED_ROOF+.5, 'panel')
    wall(SHED_X1-.4, SHED_X1, *STRIP_Z, DOOR_LINTEL, SHED_ROOF, 'trim')
    wall(x1, SHED_X1, *STRIP_Z, -UNDER, 0, 'grate')
    mesh.box(((CEIL_END+SHED_X1)/2, SHED_ROOF-.6, 32.05), (SHED_X1-CEIL_END, .5, .1), lit.accent, False)
    # Dressing and light: liners on the basement walls, strips under the
    # ceilings, and a lit strip down the stair's sloped ceiling.
    lit.dress('x', -B_X, 1, [(B_Z0+.2, B_Z1-.2)], B_FLOOR, B_CEIL)
    lit.dress('x', B_X, -1, [(B_Z0+.2, B_Z1-.2)], B_FLOOR, B_CEIL)
    lit.dress('z', B_Z0, 1, [(-B_X, -h), (h, B_X)], B_FLOOR, B_CEIL)
    lit.dress('z', B_Z1, -1, [(-B_X, DOOR_X[0]), (DOOR_X[1], B_X)], B_FLOOR, B_CEIL)
    for x in (-7, 7): lit.ceiling_strip('z', x, B_Z0+2, B_Z1-2, B_CEIL, 1.1)
    lit.ceiling_strip('z', sum(DOOR_X)/2, B_Z1+1.3, STRIP_Z[0]-.8, B_CEIL, .7, spacing=2)
    lit.ceiling_strip('x', sum(STRIP_Z)/2, DOOR_X[0]+.4, DOOR_X[1]-.4, DOOR_TOP, .7, spacing=2)
    zc, w = sum(STRIP_Z)/2, .45
    y = lambda x: service_y(x)+TUNNEL_CLEAR-.07
    xs = (x0+.4, CEIL_END-.4)
    mesh.quad((xs[0], y(xs[0]), zc-w/2), (xs[1], y(xs[1]), zc-w/2), (xs[1], y(xs[1]), zc+w/2),
              (xs[0], y(xs[0]), zc+w/2), 'light', False)
    for x in range(1, int(CEIL_END), 4): lit.lamp((x+.5, y(x+.5)-.3, zc), .8)
    lit.ceiling_strip('x', zc, CEIL_END+1, SHED_X1-1, SHED_ROOF, .8)


def _pt(r, y, i, sides=SIDES, phase=PHASE):
    a = i*math.tau/sides+phase
    return (r*math.cos(a), y, TZ+r*math.sin(a))


def lathe(mesh, prof, mat, solid=True, inward=False, skip=None, sides=SIDES, phase=PHASE):
    """Surface of revolution about the tower axis through (y, r) profile rings.
    `skip(i, k)` leaves out panel i of band k (doors, the slit)."""
    for k in range(len(prof)-1):
        (y0, r0), (y1, r1) = prof[k], prof[k+1]
        for i in range(sides):
            if skip and skip(i, k): continue
            q, w = _pt(r0, y0, i, sides, phase), _pt(r0, y0, i+1, sides, phase)
            f, e = _pt(r1, y1, i, sides, phase), _pt(r1, y1, i+1, sides, phase)
            if r1 == 0: tri = [q, f, w]
            elif r0 == 0: tri = [q, f, e]
            else:
                if inward: mesh.quad(q, w, e, f, mat, solid)
                else: mesh.quad(q, f, e, w, mat, solid)
                continue
            mesh.triangle(tri if not inward else tri[::-1], mat, solid)


def annulus(mesh, y, r0, r1, mat, up, solid=True, only=None):
    """Flat ring at height y between radii r0 < r1 (a disk when r0 == 0)."""
    for i in range(SIDES):
        if only is not None and i not in only: continue
        a, b = _pt(r1, y, i), _pt(r1, y, i+1)
        if r0 == 0:
            c = (0, y, TZ)
            mesh.triangle([c, b, a] if up else [c, a, b], mat, solid)
        else:
            ai, bi = _pt(r0, y, i), _pt(r0, y, i+1)
            if up: mesh.quad(ai, bi, b, a, mat, solid)
            else: mesh.quad(ai, a, b, bi, mat, solid)


def _slit_cut(prof):
    """Panels of the mitre removed for the slit: a band SLIT_W wide across the
    back-left face, tilted SLIT_TILT from level, reaching in to the hollow."""
    dx, dz = SLIT_DIR
    ax, az = -dz, dx                              # level axis across the slit
    t = (ax*math.cos(SLIT_TILT), math.sin(SLIT_TILT), az*math.cos(SLIT_TILT))
    n = (t[1]*dz, t[2]*dx-t[0]*dz, -t[1]*dx)     # cross(dir, t)
    ln = math.sqrt(sum(v*v for v in n)); n = tuple(v/ln for v in n)
    cut = set()
    for k in range(len(prof)-1):
        (y0, r0), (y1, r1) = prof[k], prof[k+1]
        for i in range(SIDES):
            a = (i+.5)*math.tau/SIDES+PHASE; r = (r0+r1)/2
            x, y, z = r*math.cos(a), (y0+y1)/2-SLIT_Y, r*math.sin(a)
            if x*dx+z*dz > .35*r and abs(x*n[0]+y*n[1]+z*n[2]) < SLIT_W/2: cut.add((i, k))
    return cut


def bishop_tower(mesh, b, accent):
    """Each team's flag tower, shaped like a chess bishop: ringed plinth,
    flared foot, stem, a collar whose top is the chamber floor and an outer
    landing ledge, a hollow upper stem and mitre, and a ball finial. Three
    ways into the chamber: a front door (-Z), a side door (+X) and the slit
    cut diagonally through the mitre's back-left face. An L-shaped baffle
    inside the doors keeps turrets from seeing across the chamber floor."""
    lift = lambda prof: [(ROOF_TOP+h, r) for h, r in prof]
    # Solid lower half: sealed underneath, stepped rings, flare, stem, collar.
    annulus(mesh, ROOF_TOP, 0, 7.6, 'trim', False)
    lathe(mesh, lift([(0, 7.6), (.9, 7.6)]), 'trim')
    annulus(mesh, ROOF_TOP+.9, 7.0, 7.6, 'trim', True)
    lathe(mesh, lift([(.9, 7.0), (1.6, 7.0)]), 'concrete')
    annulus(mesh, ROOF_TOP+1.6, 6.8, 7.0, 'concrete', True)
    lathe(mesh, lift([(1.6, 6.8), (2.4, 6.5), (3.2, 6.1), (4.0, 5.8), (8.2, 5.5)]), 'concrete')
    lathe(mesh, lift([(3.8, 5.87), (4.2, 5.83)]), accent, solid=False)
    annulus(mesh, ROOF_TOP+8.2, 5.5, R_LEDGE, 'trim', False)
    lathe(mesh, lift([(8.2, R_LEDGE), (9.0, R_LEDGE)]), 'trim')
    annulus(mesh, CH_FLOOR, 0, R_LEDGE, 'grate', True)
    # Hollow stem with the two doors cut through both shells, then the neck.
    door = lambda i, k: k == 0 and i in DOOR_PANELS
    outer = lift([(9, R_OUT), (12.4, 5.7), (14.4, 5.6)])
    inner = lift([(9, R_IN), (12.4, 5.1), (14.4, 5.0)])
    lathe(mesh, outer, 'concrete', skip=door)
    lathe(mesh, inner, 'panel', inward=True, skip=door)
    for i in DOOR_PANELS:
        for j, s in ((i, 1), (i+1, -1)):         # jambs close the wall's thickness
            a, bb = _pt(R_OUT, CH_FLOOR, j), _pt(R_IN, CH_FLOOR, j)
            c, d = _pt(5.1, CH_LINTEL, j), _pt(5.7, CH_LINTEL, j)
            mesh.quad(a, bb, c, d, 'trim') if s > 0 else mesh.quad(a, d, c, bb, 'trim')
        annulus(mesh, CH_LINTEL, 5.1, 5.7, 'trim', False, only={i})
    annulus(mesh, ROOF_TOP+14.4, 5.6, 6.1, 'trim', False)
    lathe(mesh, lift([(14.4, 6.1), (15.2, 6.1)]), 'trim')
    lathe(mesh, lift([(14.95, 6.13), (15.1, 6.13)]), accent, solid=False)
    annulus(mesh, ROOF_TOP+15.2, 5.7, 6.1, 'trim', True)
    lathe(mesh, lift([(14.4, 5.0), (15.2, 5.1)]), 'panel', inward=True)
    # Mitre: outer and inner shells share band heights so the slit cuts both.
    heights = [15.2, 15.8, 16.4, 17.0, 17.6, 18.2, 18.8, 19.4, 20.0, 20.6, 21.2, 21.8, 22.4, 23.0, 23.5, 23.7]
    radii = [5.7, 6.1, 6.35, R_BULB, R_BULB, 6.35, 6.1, 5.75, 5.3, 4.75, 4.1, 3.35, 2.5, 1.5, .6, 0]
    bulb = lift(list(zip(heights, radii)))
    hollow = [(y, max(0, r-.5) if r > 0 else 0) for y, r in bulb]
    hollow[0] = (bulb[0][0], 5.1)
    cut = _slit_cut(bulb)
    slit = lambda i, k: (i, k) in cut
    lathe(mesh, bulb, 'concrete', skip=slit)
    lathe(mesh, hollow, 'panel', inward=True, skip=slit)
    lathe(mesh, lift([(23.6, .35), (24.0, .35), (24.3, .75), (24.7, .75), (25.0, .45), (25.2, 0)]), 'trim', solid=False, sides=8)
    # L-shaped baffle: a wall across the front door and one across the side
    # door, joined at the corner so the two vestibules open only at their far
    # ends (the -X end in front, the +Z end at the side).
    b.wall(-2.2, BAFFLE+.2, TZ-BAFFLE-.2, TZ-BAFFLE+.2, CH_FLOOR, BAFFLE_TOP, 'trim')
    b.wall(BAFFLE-.2, BAFFLE+.2, TZ-BAFFLE+.2, TZ+2.2, CH_FLOOR, BAFFLE_TOP, 'trim')
    # Team trim and light: flag ring on the floor, glow strips on the inner
    # wall between the doors, a lit ring under the neck.
    lathe(mesh, [(CH_FLOOR+.02, 1.6), (CH_FLOOR+.02, 1.2)], accent, solid=False)
    for i in (3, 7, 10):
        a = (i+.5)*math.tau/SIDES+PHASE; r = 5.05
        cx, cz, tx, tz = r*math.cos(a), TZ+r*math.sin(a), -math.sin(a)*.22, math.cos(a)*.22
        y0, y1 = CH_FLOOR+1, CH_FLOOR+5
        mesh.quad((cx-tx, y0, cz-tz), (cx+tx, y0, cz+tz), (cx+tx, y1, cz+tz), (cx-tx, y1, cz-tz), 'light', False)
        b.lamp((cx*.9, CH_FLOOR+3, TZ+(cz-TZ)*.9), 1.0)
    lathe(mesh, [(ROOF_TOP+14.5, 4.97), (ROOF_TOP+14.7, 4.97)], 'light', solid=False)
    b.lamp((0, ROOF_TOP+14, TZ), 1.4)
    b.lamp((0, CH_FLOOR+2.5, TZ), .8)


def build(mesh, team, circuit, equipment):
    accent = 'ember' if team == 0 else 'glacier'
    b = Builder(mesh, accent, interior='panel', metal='trim', glow='light')
    under = Builder(mesh, accent, interior='concrete', metal='trim', glow='light')
    # Hall floor and walls; side walls fit between the front and back walls.
    # The floor leaves the atrium stair's opening.
    h = STAIR_W/2
    b.slab(-32, 32, -28, B_Z0, FLOOR, 'grate', 1.2)
    b.slab(-32, 32, OPEN_Z1, 28, FLOOR, 'grate', 1.2)
    for s in (-1, 1): b.slab(s*h, s*32, B_Z0, OPEN_Z1, FLOOR, 'grate', 1.2)
    basement(mesh, b, under)
    wy, wh = (FLOOR+WALL_TOP)/2, WALL_TOP-FLOOR
    for x in (-31, 31): mesh.box((x, wy, 0), (2, wh, 52), 'concrete')
    mesh.box((0, wy, 27), (64, wh, 2), 'concrete')
    for x in (-21, 21): mesh.box((x, wy, -27), (22, wh, 2), 'concrete')
    mesh.box((0, (4+WALL_TOP)/2, -27), (20, WALL_TOP-4, 2), 'panel')
    # Roof halves with ramp openings, eaves on the sides and back; the centre
    # roof carries the flag deck. The atrium stays open to the sky.
    for side in (-1, 1): roof_half(mesh, side)
    mesh.box((0, (WALL_TOP+ROOF_TOP)/2, 12), (24, ROOF_TOP-WALL_TOP, 16), 'panel')
    # Entrance ramp down into the hall, its cheeks, and the retaining walls.
    mesh.ramp(0, 18, -52, -27, 0, FLOOR)
    for x in (-9.5, 9.5): mesh.box((x, -5, -40), (1, 10, 24), 'concrete')
    for x in (-31, 31): mesh.box((x, -5, -40.5), (2, 10, 25), 'concrete')
    for x in (-20, 20): mesh.box((x, -5, -51), (20, 10, 2), 'concrete')
    # Hall ramps to the roof openings, closed underneath where too low.
    for x in (-20, 20):
        mesh.ramp(x, 9, -19, HOLE_Z[1], FLOOR, ROOF_TOP)
        close_under(mesh, x, 9, -19, HOLE_Z[1], FLOOR, ROOF_TOP, FLOOR, 'trim')
    # Outer shoulders: ski ramps from the apron up to the roof's front edge.
    for x in (-22, 22):
        mesh.ramp(x, 18, -52, -28, APRON_TOP, ROOF_TOP, 'concrete')
        close_under(mesh, x, 18, -52, -28, APRON_TOP, ROOF_TOP, APRON_TOP, 'concrete')
    # The bishop flag tower behind the front roof deck (see bishop_tower).
    bishop_tower(mesh, b, accent)
    # Exterior ribbed cladding and team band above ground on the side walls.
    for x in (-32.05, 32.05):
        for z in range(-24, 26, 8): mesh.box((x, WALL_TOP/2, z), (.15, WALL_TOP-.2, .7), 'trim', False)
        mesh.box((x, 6, 0), (.2, .6, 50), accent, False)
    # Interior: dressed hall walls and lit strips under the roof.
    # Side runs stop short of the front and back trim so corners never overlap.
    for side in (-1, 1):
        b.dress('x', side*30, -side, [(-25.8, 25.8)], FLOOR, WALL_TOP)
    b.dress('z', 26, -1, [(-30, 30)], FLOOR, WALL_TOP)
    b.dress('z', -26, 1, [(-30, -10), (10, 30)], FLOOR, WALL_TOP)
    for side in (-1, 1):
        b.ceiling_strip('z', side*27, -24, 24, WALL_TOP, 1.1)
        b.ceiling_strip('z', side*13.7, -24, 24, WALL_TOP, .9)
    b.ceiling_strip('x', 12, -10, 10, WALL_TOP, .9)
    for item in equipment:
        mesh.equipment(item['kind'], item['position'], team, circuit, item.get('weapon', 'bullet'))
    hall, roof = FLOOR+LIFT, ROOF_TOP+LIFT
    return {
        'flag': (0, CH_FLOOR+.35, TZ),
        'flag_tower': (0, CH_FLOOR, TZ),
        # Front door, side door and the mitre slit, each just outside the tower.
        'tower_entries': [(0, CH_FLOOR, TZ-R_OUT-.8), (R_OUT+.8, CH_FLOOR, TZ),
                          (SLIT_DIR[0]*(R_BULB+1), SLIT_Y, TZ+SLIT_DIR[1]*(R_BULB+1))],
        'spawn': (48, 0, -65),
        # (x, y, z, local yaw): yaw 0 faces the entrance (-Z), pi faces +Z.
        'spawn_points': [(-10, hall, -18, math.pi), (10, hall, -18, math.pi),
                         (-12, hall, 14, 0), (12, hall, 14, 0),
                         (-27, hall, -23, math.pi), (27, hall, -23, math.pi),
                         (-27, roof, 4, 0), (27, roof, 4, 0)],
        'entrances': [(0, 0, -52), (-22, APRON_TOP, -52), (22, APRON_TOP, -52)],
        'hall': (0, hall, 0),
        'roof_openings': [(-20, ROOF_TOP, sum(HOLE_Z)/2), (20, ROOF_TOP, sum(HOLE_Z)/2)],
        'generator_room': (0, B_FLOOR+LIFT, 16),
        'stair_head': (0, FLOOR, B_Z0),
        'service_door': (SHED_X1, 0, sum(STRIP_Z)/2),
    }
