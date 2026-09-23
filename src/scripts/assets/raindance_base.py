"""Raindance base, cleaned (asset raindance-base-v2).

Same layout as the original kit base (basement hall, atrium, wide ski ramps,
exposed flag deck, service spire) with the pipeline checklist applied:

- Walls meet at the corners instead of overlapping, and the roof sits on
  the walls with eaves, so no two visible faces share a plane (z-fighting).
- Each hall ramp climbs to an opening in its roof half instead of into the
  roof slab, and the outer shoulder ramps end flush with the roof edge.
- Where a ramp's underside is lower than 2.4 m above the floor beneath it,
  a solid closure stops players walking under it.
- The service spire has a solid base, so it cannot be entered from below.
- Hall walls get liners, baseboards, cornices, pilasters and a team stripe;
  lit strips under the roof replace the old floating wall lights.
- Eight spawn points per team, 1.2 m above solid floor, facing open space.

Local coordinates: Y up, the entrance faces local -Z. Hall floor at -10,
roof top at 8.6, ground level at 0.
"""
import math

from assets.structure_kit import Builder

ASSET_ID = 'raindance-base-v2'
FLOOR, WALL_TOP, ROOF_TOP, APRON_TOP = -10.0, 7.4, 8.6, .03
LIFT = 1.2
UNDER, HEADROOM = .6, 2.4
# Openings in each roof half above the upper end of its hall ramp.
HOLE_X, HOLE_Z = (15.5, 24.5), (10.0, 19.0)


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


def build(mesh, team, circuit, equipment):
    accent = 'ember' if team == 0 else 'glacier'
    b = Builder(mesh, accent, interior='panel', metal='trim', glow='light')
    # Hall floor and walls; side walls fit between the front and back walls.
    mesh.box((0, FLOOR-.6, 0), (64, 1.2, 56), 'grate')
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
    # Faceted service spire behind the exposed flag deck; solid base.
    mesh.column((0, ROOF_TOP, 14), 5, 1.1, 'trim', 8)
    mesh.column((0, ROOF_TOP+1.1, 14), 4.7, .16, accent, 8, solid=False)
    mesh.column((0, ROOF_TOP, 22), 5.8, 18, 'concrete', 8, top=4.4)
    cap(mesh, (0, ROOF_TOP, 22), 5.8, 8, 'concrete')
    mesh.column((0, 26.6, 22), 4.8, 1.2, 'trim', 8)
    mesh.column((0, 24.7, 22), 4.6, .4, accent, 8, solid=False)
    mesh.column((0, 27.8, 22), .5, 4, 'panel', 6)
    for x in (-4, 4): mesh.box((x, 16.5, 18.8), (.6, 12, .6), 'trim', False)
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
        'flag': (0, 10.05, 14),
        'spawn': (48, 0, -65),
        # (x, y, z, local yaw): yaw 0 faces the entrance (-Z), pi faces +Z.
        'spawn_points': [(-10, hall, -18, math.pi), (10, hall, -18, math.pi),
                         (-12, hall, 14, 0), (12, hall, 14, 0),
                         (-27, hall, -23, math.pi), (27, hall, -23, math.pi),
                         (-27, roof, 4, 0), (27, roof, 4, 0)],
        'entrances': [(0, 0, -52), (-22, APRON_TOP, -52), (22, APRON_TOP, -52)],
        'hall': (0, hall, 0),
        'roof_openings': [(-20, ROOF_TOP, sum(HOLE_Z)/2), (20, ROOF_TOP, sum(HOLE_Z)/2)],
    }
