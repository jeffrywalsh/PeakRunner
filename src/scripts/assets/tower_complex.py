"""Original small floating base: a 3-level central tower, two rear support
pods (generator room, ship platform) on enclosed tunnels, and two front
turret pods on open bridges. Every structure is its own hovering hull with a
tapered keel and thruster nozzles underneath.

No source mesh, texture, lightmap or parser is used by this builder. Local
X/Z are horizontal, Y is up. -Z is the front (field-facing) side; +Z is the
rear. Caller supplies placement and ownership through mesh.origin/mesh.yaw.

The central tube runs from the keel level up to the roof. Each level gets
one open wall face into it (keel -x, L1 front, L2 toward the flag, L3 the far
side); L1, L2 and L3 have floor holes inside it, so it is a fast drop down
and a jet route up.

The generator sits on the keel level: a room inside the top of the tower's
keel, below Level 1, with exactly two entrances. One is the tube (drop in
from Level 1, jet back up). The other is an exterior hatch on the keel's +x
flank, opening onto a ledge a jetting player can reach from the bridges or
the field; an airlock housing and an inner baffle keep every outside line of
sight out of the room. The old rear-left generator pod is now the armory: an
inventory station and a repair pad at the end of its tunnel.

Everything above ROOF (stepped setbacks, spine, fins, spires) and below the
keel level (lower keel, engine pods) is exterior massing. It is solid where a
jetting player could plausibly touch it and non-solid for thin decoration,
and none of it encloses a space a player can enter.

Interior dressing (wall liners, baseboards, cornices, pilasters, light
fixtures, frames) is render-only and sits at most 0.2 m proud of a solid
surface, inside the 0.52 m player radius, so it never needs collision.
Doorways and Level 3 windows are open: players and shots pass straight
through. Nothing stands behind the front openings; the pod turrets face the
field with a limited field of fire (turret_arcs.py) instead of being walled
off from the rooms. Only the keel hatch into the generator room keeps its
airlock baffle.

If the caller's mesh has a `lamps` list, every light fixture appends
(world_position, intensity) point samples to it for the offline light bake.
"""
import math

from assets.fortress_rooms import shaft

ASSET_ID = 'tower-complex-v6'

TOWER_HALF = 12
WALL = .4             # half-thickness of the tower perimeter walls
INNER = TOWER_HALF-WALL
SHAFT_HALF = 2.5
SHAFT_WALL = .6
L1, L2, L3, ROOF = 0, 7, 14, 21
KEEL = -8             # generator level floor; its ceiling is the L1 slab (-1)
KEEL_IN = 8.1         # keel room inner wall faces at |x|, |z| = KEEL_IN
KEEL_WALL = .4
KEEL_CEILING = L1-1
HATCH_HALF = 2.2      # hatch passage half-width (z)
HATCH_TOP = KEEL+4.5
HATCH_OUT = 13.4      # outer end of the airlock housing (+x)
LEDGE_OUT = 16.5
KEEL_BAFFLE = (5.9, 6.3, -4.0, 4.0)   # x0, x1, z0, z1 inside the hatch
GENERATOR = (-5.2, KEEL, -5.2)
STOREY = 6            # floor top to the underside of the next slab
SHAFT_OPENINGS = [('-x',KEEL,KEEL+3.2),('-z',L1,L1+3.2),('+x',L2,L2+3.2),('-x',L3,L3+3.2)]
DOOR_TOP = 4.5        # headers close L1 wall gaps above this height
POD_Z = -30
REAR_Z0, REAR_Z1 = 26, 36
ROOM_H = 7            # rear rooms clear the 5.8 m kit generator
FLAG_X = 5.5
# Spawn anchors are the player's centre, not the floor: the engine's support
# ray starts 0.15 m above the feet (centre - 0.52 m radius), so a centre only
# 0.2 m above a 1 m slab starts inside it and reads the slab's underside,
# leaving the player "airborne" and frictionless (the v3/v4 slick spawn).
SPAWN_LIFT = 1.2   # same convention as every other pack: centre 1.2 m above the floor
RAMP_HEADROOM = 2.4   # under-ramp space lower than this is closed off
PLINTH = .3
SILL, HEAD = 1.1, 3.9  # Level 3 window aperture, above the L3 floor
MULLION_STEP = 3
# Window apertures per wall (along-wall ranges). The front keeps its centre
# solid behind the banner strip and accent bands on the outer face.
WINDOWS = {'-z': [(-9,-5.8),(5.8,9)], '+z': [(-9,9)], '-x': [(-9,9)], '+x': [(-9,9)]}
# Solid wall spans on Level 1 (gaps: front door, two bridge doors, two rear
# tunnel mouths).
L1_GAPS = {'-z': [(-11,-7),(-2,2),(7,11)], '+z': [(-10,-6),(6,10)], '-x': [], '+x': []}
# Front openings (door, two bridge doors) are open straight into Level 1.
# The L1->L2 ramp foot stays set back from the left bridge door.
RAMP1_Z0 = -8.8

# Material roles. 'bark' is a kit slot this pack repaints as the interior
# wall panel; it is never used for trees here.
HULL, INTERIOR, DECK, HAZARD, METAL, GLOW = 'concrete', 'bark', 'panel', 'grate', 'trim', 'light'


def spans(lo, hi, gaps):
    out, u = [], lo
    for a, b in sorted(gaps):
        if a > u: out.append((u, a))
        u = max(u, b)
    if hi > u: out.append((u, hi))
    return out


def build(mesh, team, circuit):
    if team not in (0,1) or not circuit: raise ValueError('team and circuit required')
    accent = ['ember','glacier'][team]
    lamps = getattr(mesh, 'lamps', None)

    def lamp(p, power):
        if lamps is not None: lamps.append((mesh.point(p), power))

    def slab(x0,x1,z0,z1,y,mat=DECK):
        mesh.box(((x0+x1)/2,y-.5,(z0+z1)/2),(x1-x0,1,z1-z0),mat)
    def wall(x0,x1,z0,z1,y0,y1,mat=HULL,solid=True):
        if x0>x1: x0,x1=x1,x0
        if z0>z1: z0,z1=z1,z0
        if y0>y1: y0,y1=y1,y0
        mesh.box(((x0+x1)/2,(y0+y1)/2,(z0+z1)/2),(x1-x0,y1-y0,z1-z0),mat,solid)
    def floor_ring(y,mat=DECK,holes=(),shaft_hole=True):
        h=SHAFT_HALF; T=TOWER_HALF
        holes=[*([(-h,h,-h,h)] if shaft_hole else []),*holes]
        xs=sorted({-T,T,*(v for hx0,hx1,_,_ in holes for v in (hx0,hx1))})
        for xa,xb in zip(xs,xs[1:]):
            cover=sorted((hz0,hz1) for hx0,hx1,hz0,hz1 in holes if hx0<=xa and hx1>=xb)
            z=-T
            for hz0,hz1 in cover+[(T,T)]:
                if hz0>z: slab(xa,xb,z,hz0,y,mat)
                z=max(z,hz1)
    def hull(cx,cz,bottom,top,y0,y1,mat,solid=True):
        bx,bz=bottom; tx,tz=top
        for s in (-1,1):
            mesh.quad((cx-bx,y0,cz+s*bz),(cx+bx,y0,cz+s*bz),(cx+tx,y1,cz+s*tz),(cx-tx,y1,cz+s*tz),mat,solid)
            mesh.quad((cx+s*bx,y0,cz-bz),(cx+s*bx,y0,cz+bz),(cx+s*tx,y1,cz+tz),(cx+s*tx,y1,cz-tz),mat,solid)
    def thruster(x,y,z,r):
        mesh.column((x,y-r*1.4,z),r*.75,r*1.4,GLOW,10,top=r,solid=False)
    def keel(cx,cz,half,y_top,depth,r_tip):
        hull(cx,cz,(half[0]*.72,half[1]*.72),half,y_top-depth*.3,y_top,METAL)
        hull(cx,cz,(r_tip,r_tip),(half[0]*.72,half[1]*.72),y_top-depth,y_top-depth*.3,HULL)
        thruster(cx,y_top-depth,cz,r_tip)

    # --- Surface dressing helpers --------------------------------------------
    # A "face" is an axis-aligned wall plane described by the side of the room
    # it bounds, the plane coordinate and which way the room lies. Along-wall
    # coordinate u is x for z-walls and z for x-walls.
    def at(axis, plane, u, y):
        return (u, y, plane) if axis == 'z' else (plane, y, u)
    def face_quad(axis, plane, into, u0, u1, y0, y1, mat, off=.03):
        p = plane+into*off
        a,b,c,d = at(axis,p,u0,y0),at(axis,p,u1,y0),at(axis,p,u1,y1),at(axis,p,u0,y1)
        # Wind so the normal points into the room.
        flip = (axis == 'z') == (into < 0)
        mesh.quad(*((a,d,c,b) if flip else (a,b,c,d)), mat, False)
    def face_box(axis, plane, into, u0, u1, y0, y1, depth, mat, solid=False):
        centre = at(axis, plane+into*depth/2, (u0+u1)/2, (y0+y1)/2)
        size = (u1-u0, y1-y0, depth) if axis == 'z' else (depth, y1-y0, u1-u0)
        mesh.box(centre, size, mat, solid)
    def dress(axis, plane, into, runs, y0, y1, stripe=True, pilasters=True):
        """Liner, baseboard, cornice, accent stripe and pilasters over the
        solid runs of one wall face between floor y0 and ceiling y1."""
        for u0,u1 in runs:
            if u1-u0 < .05: continue
            face_quad(axis,plane,into,u0,u1,y0,y1,INTERIOR)
            face_box(axis,plane,into,u0,u1,y0,y0+.3,.12,METAL)
            face_box(axis,plane,into,u0,u1,y1-.28,y1,.18,METAL)
            if stripe: face_quad(axis,plane,into,u0,u1,y0+.7,y0+.95,accent,.045)
            if pilasters:
                for u in range(-9,10,6):
                    if u0+.4 <= u <= u1-.4: face_box(axis,plane,into,u-.22,u+.22,y0+.3,y1-.28,.15,METAL)
    def ceiling_strip(x, z0, z1, y_ceiling, power, spacing=3):
        mesh.box((x,y_ceiling-.035,(z0+z1)/2),(.8,.07,z1-z0+.4),METAL,False)
        mesh.box((x,y_ceiling-.07,(z0+z1)/2),(.45,.06,z1-z0),GLOW,False)
        n = max(1, round((z1-z0)/spacing))
        for i in range(n): lamp((x,y_ceiling-.3,z0+(i+.5)*(z1-z0)/n), power)

    tower_faces = {'-z': ('z',-INNER,1), '+z': ('z',INNER,-1), '-x': ('x',-INNER,1), '+x': ('x',INNER,-1)}

    # --- Central tower: traversable L1-L3 block ------------------------------
    # Ramp openings keep ~2.2 m headroom over each climbing ramp: L2 is cut
    # above the L1->L2 ramp (x=-9), L3 above the L2->L3 ramp (x=+9).
    floor_ring(L1)
    floor_ring(L2,holes=[(-11,-7,-1,11)])
    floor_ring(L3,holes=[(7,11,-11,1)])
    slab(-TOWER_HALF,TOWER_HALF,-TOWER_HALF,TOWER_HALF,ROOF)
    # The tube: gunmetal walls; hazard striping only frames the open faces.
    shaft(mesh,0,0,SHAFT_HALF,KEEL,ROOF,mat=METAL,thickness=SHAFT_WALL,openings=SHAFT_OPENINGS)
    edge = SHAFT_HALF+SHAFT_WALL
    for side,ya,yb in SHAFT_OPENINGS:
        axis = 'x' if side[1] == 'x' else 'z'; sgn = -1 if side[0] == '-' else 1
        mid = sgn*(SHAFT_HALF+SHAFT_WALL/2)
        # x-faces own the tube corners; z-faces sit between the x-walls.
        reach = edge if axis == 'x' else SHAFT_HALF
        def tube_box(n0,n1,u0,u1,y0,y1,mat,solid):
            # n: along the opening's outward normal, u: along the face.
            if axis == 'x': wall(n0,n1,u0,u1,y0,y1,mat,solid)
            else: wall(u0,u1,n0,n1,y0,y1,mat,solid)
        for s in (-1,1):   # solid hazard jambs in the open corners
            tube_box(mid-SHAFT_WALL/2,mid+SHAFT_WALL/2,s*(reach-.35),s*reach,ya,yb,HAZARD,True)
        # Hazard band on the lintel and a lip strip across the threshold.
        tube_box(mid-SHAFT_WALL/2-.02,mid+SHAFT_WALL/2+.04,-reach,reach,yb+.02,yb+.35,HAZARD,False)
        tube_box(mid-SHAFT_WALL/2,mid+SHAFT_WALL/2,-(reach-.35),reach-.35,ya,ya+.02,HAZARD,False)
    # A lit ring inside the tube above each opening, and a landing square on
    # the tube's keel-level floor.
    for level in (KEEL,L1,L2,L3):
        y = level+(4.4 if level != KEEL else 5.4)
        for s in (-1,1):
            mesh.box((s*(SHAFT_HALF-.03),y,0),(.06,.18,2*SHAFT_HALF),GLOW,False)
            mesh.box((0,y,s*(SHAFT_HALF-.03)),(2*SHAFT_HALF,.18,.06),GLOW,False)
        lamp((0,y-.4,0),.9)
    for s in (-1,1):
        mesh.box((0,KEEL+.015,s*(SHAFT_HALF-.2)),(2*SHAFT_HALF,.03,.4),HAZARD,False)
        mesh.box((s*(SHAFT_HALF-.2),KEEL+.016,0),(.4,.03,2*SHAFT_HALF-.8),HAZARD,False)
    # Hazard lip round the tube's Level 1 floor hole (the drop to the keel).
    for s in (-1,1):
        wall(-SHAFT_HALF,SHAFT_HALF,s*SHAFT_HALF-.02,s*SHAFT_HALF+.02,L1-.3,L1+.01,HAZARD,False)
        wall(s*SHAFT_HALF-.02,s*SHAFT_HALF+.02,-SHAFT_HALF,SHAFT_HALF,L1-.3,L1+.01,HAZARD,False)

    # --- Keel level: the generator room ------------------------------------
    # A room inside the top of the keel, under the L1 slab. Exactly two ways
    # in: the tube's keel-level face (-x) and the hatch passage on the +x
    # flank. The baffle just inside the hatch turns the entry sideways.
    K, KW = KEEL_IN, KEEL_WALL
    slab(-K-KW,K,-K-KW,K+KW,KEEL)                       # floor (the ledge slab carries x > K)
    keel_faces = {'-z': ('z',-K,1), '+z': ('z',K,-1), '-x': ('x',-K,1), '+x': ('x',K,-1)}
    for side,(axis,plane,into) in keel_faces.items():
        outer = plane-into*KW; lo, hi = min(plane,outer), max(plane,outer)
        gaps = [(-HATCH_HALF,HATCH_HALF)] if side == '+x' else []
        for u0,u1 in spans(-K-KW,K+KW,gaps):
            if axis == 'z': wall(u0,u1,lo,hi,KEEL,KEEL_CEILING)
            else: wall(lo,hi,u0,u1,KEEL,KEEL_CEILING)
        for u0,u1 in gaps:
            wall(lo,hi,u0,u1,HATCH_TOP,KEEL_CEILING)
            face_quad(axis,plane,into,u0,u1,HATCH_TOP,KEEL_CEILING,INTERIOR)
        dress(axis,plane,into,spans(-K,K,gaps),KEEL,KEEL_CEILING)
    bx0,bx1,bz0,bz1 = KEEL_BAFFLE
    wall(bx0,bx1,bz0,bz1,KEEL,KEEL_CEILING)
    dress('x',bx1,1,[(bz0,bz1)],KEEL,KEEL_CEILING,pilasters=False)
    dress('x',bx0,-1,[(bz0,bz1)],KEEL,KEEL_CEILING,pilasters=False)
    for z in (bz0,bz1): wall(bx0-.06,bx1+.06,z-.06,z+.06,KEEL,KEEL+DOOR_TOP,HAZARD,False)
    for x in (-5.5,5.5): ceiling_strip(x,-7,7,KEEL_CEILING,.8)
    mesh.equipment('generator',GENERATOR,team,circuit)
    gxk,_,gzk = GENERATOR
    mesh.box((gxk,KEEL+.015,gzk),(6.4,.03,5.4),HAZARD,False)
    # Hatch passage and airlock housing: floor on the ledge slab, solid side
    # walls and roof from the room wall out past the keel flank.
    slab(K,LEDGE_OUT,-HATCH_HALF-2.3,HATCH_HALF+2.3,KEEL)              # passage floor + ledge
    for s in (-1,1):
        wall(K+KW,HATCH_OUT,s*HATCH_HALF,s*(HATCH_HALF+.4),KEEL,HATCH_TOP+1)
        dress('z',s*HATCH_HALF,-s,[(K+KW,HATCH_OUT)],KEEL,HATCH_TOP,pilasters=False)
    slab(K+KW,HATCH_OUT,-HATCH_HALF-.4,HATCH_HALF+.4,HATCH_TOP+1,HULL)    # roof
    mesh.box(((K+KW+HATCH_OUT)/2,HATCH_TOP-.035,0),(HATCH_OUT-K-KW-.6,.07,.8),METAL,False)
    mesh.box(((K+KW+HATCH_OUT)/2,HATCH_TOP-.07,0),(HATCH_OUT-K-KW-1,.06,.45),GLOW,False)
    for x in (K+1.5,HATCH_OUT-1.5): lamp((x,HATCH_TOP-.3,0),.5)
    # Outer frame, hazard lintel and ledge edge markings (render only).
    for s in (-1,1):
        mesh.box((HATCH_OUT+.07,(KEEL+HATCH_TOP)/2,s*(HATCH_HALF+.2)),(.14,HATCH_TOP-KEEL,.5),METAL,False)
        mesh.box(((HATCH_OUT+LEDGE_OUT)/2,KEEL+.015,s*(HATCH_HALF+2.1)),(LEDGE_OUT-HATCH_OUT,.03,.3),HAZARD,False)
    mesh.box((HATCH_OUT+.07,HATCH_TOP+.2,0),(.16,.4,2*HATCH_HALF+.9),HAZARD,False)
    mesh.box((LEDGE_OUT-.15,KEEL+.015,0),(.3,.03,2*HATCH_HALF+4.6),HAZARD,False)
    for s in (-1,1):
        mesh.column((LEDGE_OUT-.4,KEEL,s*(HATCH_HALF+2)),.12,1.6,METAL,6,top=.05,solid=False)
        mesh.box((LEDGE_OUT-.4,KEEL+1.7,s*(HATCH_HALF+2)),(.2,.2,.2),GLOW,False)
        lamp((LEDGE_OUT-.4,KEEL+2.2,s*(HATCH_HALF+2)),.3)

    # Level 1 shell: door gaps capped by solid headers above DOOR_TOP.
    T, W = TOWER_HALF, WALL
    for side,(axis,plane,into) in tower_faces.items():
        outer = plane-into*2*W  # far face of the wall
        lo, hi = min(plane,outer), max(plane,outer)
        for u0,u1 in spans(-T,T,L1_GAPS[side]):
            if axis == 'z': wall(u0,u1,lo,hi,L1,L2)
            else: wall(lo,hi,u0,u1,L1,L2)
        for u0,u1 in L1_GAPS[side]:
            if axis == 'z': wall(u0,u1,lo,hi,DOOR_TOP,L2)
            else: wall(lo,hi,u0,u1,DOOR_TOP,L2)
            # Frames on both faces and a hazard band under the header.
            for face,direction in ((plane,into),(outer,-into)):
                face_box(axis,face,direction,u0-.3,u0,L1,DOOR_TOP,.14,METAL)
                face_box(axis,face,direction,u1,u1+.3,L1,DOOR_TOP,.14,METAL)
                face_box(axis,face,direction,u0-.3,u1+.3,DOOR_TOP,DOOR_TOP+.3,.14,METAL)
            if axis == 'z': wall(u0,u1,lo-.06,hi+.06,DOOR_TOP-.15,DOOR_TOP,HAZARD,False)
        dress(axis,plane,into,spans(-INNER,INNER,L1_GAPS[side]),L1,L1+STOREY)
        for u0,u1 in L1_GAPS[side]:
            face_quad(axis,plane,into,u0,u1,DOOR_TOP,L1+STOREY,INTERIOR)
    # Level 2: solid perimeter.
    for side,(axis,plane,into) in tower_faces.items():
        outer = plane-into*2*W; lo, hi = min(plane,outer), max(plane,outer)
        if axis == 'z': wall(-T,T,lo,hi,L2,L3)
        else: wall(lo,hi,-T,T,L2,L3)
        dress(axis,plane,into,[(-INNER,INNER)],L2,L2+STOREY)
    # Level 3: sill, head and piers around open window apertures; the
    # mullions are decoration, so shots and players pass between them.
    for side,(axis,plane,into) in tower_faces.items():
        outer = plane-into*2*W; lo, hi = min(plane,outer), max(plane,outer)
        def piece(u0,u1,y0,y1):
            if axis == 'z': wall(u0,u1,lo,hi,y0,y1)
            else: wall(lo,hi,u0,u1,y0,y1)
        piece(-T,T,L3,L3+SILL); piece(-T,T,L3+HEAD,ROOF)
        piers = spans(-T,T,WINDOWS[side])
        for u0,u1 in piers: piece(u0,u1,L3+SILL,L3+HEAD)
        mid = (lo+hi)/2
        for u0,u1 in WINDOWS[side]:
            y0,y1 = L3+SILL,L3+HEAD
            # Frame (proud both faces) and mullions.
            face_box(axis,outer,-into,u0,u1,y0-.12,y0,.16,METAL)
            for face,direction in ((plane,into),(outer,-into)):
                face_box(axis,face,direction,u0,u1,y1,y1+.12,.16,METAL)
            n = max(1, round((u1-u0)/MULLION_STEP))
            for i in range(n+1):
                u = u0+(u1-u0)*i/n
                if axis == 'z': wall(u-.1,u+.1,mid-.2,mid+.2,y0,y1,METAL,False)
                else: wall(mid-.2,mid+.2,u-.1,u+.1,y0,y1,METAL,False)
            wall(*((u0,u1,mid-.03,mid+.03) if axis == 'z' else (mid-.03,mid+.03,u0,u1)),y0+1.3,y0+1.36,METAL,False)
        inner_runs = [(max(u0,-INNER),min(u1,INNER)) for u0,u1 in piers]
        dress(axis,plane,into,[(-INNER,INNER)],L3,L3+SILL,stripe=True,pilasters=False)
        for u0,u1 in inner_runs: face_quad(axis,plane,into,u0,u1,L3+SILL,L3+HEAD,INTERIOR)
        face_quad(axis,plane,into,-INNER,INNER,L3+HEAD,L3+STOREY,INTERIOR)
        face_box(axis,plane,into,-INNER,INNER,L3+STOREY-.28,L3+STOREY,.18,METAL)

    # Ceiling light strips, clear of the shaft and the ramp openings.
    for level in (L1,L2,L3):
        for x in (-5,5): ceiling_strip(x,-9,9,level+STOREY,.75)

    # Level 1: open area cover and switchback ramps up the outer walls.
    for x,z in [(-6,-4),(6,-4),(-6,4),(6,4)]:
        mesh.box((x,1.5,z),(1.5,3,1.5),HULL)
        mesh.box((x,3.07,z),(1.78,.14,1.78),METAL,False)
        mesh.box((x,.18,z),(1.68,.36,1.68),METAL,False)
        mesh.box((x,2.3,z),(1.56,.16,1.56),accent,False)
        for dx in (-.76,.76):
            for dz in (-.76,.76): mesh.box((x+dx,1.5,z+dz),(.1,2.6,.1),HAZARD,False)
    mesh.ramp(-9,4,RAMP1_Z0,TOWER_HALF-1,L1,L2)
    mesh.ramp(9,4,TOWER_HALF-1,-TOWER_HALF+1,L2,L3)
    for x,z0,z1,y0,y1 in [(-9,RAMP1_Z0,TOWER_HALF-1,L1,L2),(9,TOWER_HALF-1,-TOWER_HALF+1,L2,L3)]:
        for s in (-1,1):   # rails and posts
            mesh.quad((x+s*2,y0+.9,z0),(x+s*2,y1+.9,z1),(x+s*2,y1+1,z1),(x+s*2,y0+1,z0),METAL,False)
            for i in range(6):
                t = (i+.5)/6
                mesh.box((x+s*2,y0+t*(y1-y0)+.5,z0+t*(z1-z0)),(.08,1,.08),METAL,False)

    # Close the low wedge under each ramp: a player could walk in beneath the
    # rising underside and get pinched between it and the floor, which pops
    # them up into a frictionless coast. Solid skirt on the room side from
    # where the underside leaves the floor to where it clears RAMP_HEADROOM,
    # plus an end cap across the ramp and the wall trench.
    def skirt(x_in, x_wall, z_floor, z_open, y):
        top = y+RAMP_HEADROOM
        for a,b,c in (((x_in,y,z_floor),(x_in,y,z_open),(x_in,top,z_open)),):
            mesh.triangle([a,b,c],METAL); mesh.triangle([a,c,b],METAL)
        lo, hi = min(x_in,x_wall), max(x_in,x_wall)
        quad = ((lo,y,z_open),(hi,y,z_open),(hi,top,z_open),(lo,top,z_open))
        mesh.quad(*quad,METAL); mesh.quad(*quad[::-1],METAL)
    rise = STOREY+1   # ramp climb per level
    run1, run2 = TOWER_HALF-1-RAMP1_Z0, 2*TOWER_HALF-2
    skirt(-7,-INNER,RAMP1_Z0+.6*run1/rise,RAMP1_Z0+(RAMP_HEADROOM+.6)*run1/rise,L1)
    skirt(7,INNER,TOWER_HALF-1-.6*run2/rise,TOWER_HALF-1-(RAMP_HEADROOM+.6)*run2/rise,L2)

    # Entry hall light strip just inside the front wall.
    vz = -INNER+1.2
    mesh.box((0,L1+STOREY-.035,vz),(2*INNER-.4,.07,.8),METAL,False)
    mesh.box((0,L1+STOREY-.07,vz),(2*INNER-.8,.06,.45),GLOW,False)
    for x in range(-9,10,3): lamp((x,L1+STOREY-.3,vz),.5)

    # Level 2: the flag on a low chamfered plinth under two ramp supports,
    # with the team banner on the rear wall.
    flag = (FLAG_X,L2+PLINTH+.05,0)
    mesh.column((FLAG_X,L2,0),1.7,PLINTH,METAL,12,top=1.45)
    mesh.column((FLAG_X,L2+PLINTH,0),1.45,.012,accent,12,top=1.45,solid=False)
    mesh.column((FLAG_X,L2+PLINTH+.012,0),1.05,.012,DECK,12,top=1.05,solid=False)
    mesh.column((FLAG_X,L2+PLINTH+.024,0),.35,.012,GLOW,12,top=.35,solid=False)
    for z in (-2.4,2.4):
        ramp_under = L2+7*(TOWER_HALF-1-z)/(2*TOWER_HALF-2)-.62
        wall(7.05,7.55,z-.25,z+.25,L2,ramp_under,METAL)
        wall(7.02,7.58,z-.28,z+.28,L2+2.1,L2+2.3,accent,False)
    bz = INNER-.12
    mesh.quad((-2.2,L2+1.2,bz),(-2.2,L2+5.4,bz),(2.2,L2+5.4,bz),(2.2,L2+1.2,bz),accent,False)
    mesh.triangle([(-2.2,L2+1.2,bz),(2.2,L2+1.2,bz),(0,L2+.2,bz)],accent,False)
    for base_y in (2.6,3.8):
        for s in (-1,1):
            mesh.quad((s*1.5,L2+base_y,bz-.02),(0,L2+base_y+1.2,bz-.02),(0,L2+base_y+1.6,bz-.02),(s*1.5,L2+base_y+.4,bz-.02),GLOW,False)
    lamp((FLAG_X,L2+3,0),.6)

    # Level 3: paired inventory stations.
    for x in (-5.2,5.2): mesh.equipment('inventory',(x,L3,-6),team,circuit)

    # --- Central tower: exterior massing ------------------------------------
    tiers = [(10.2,ROOF,32),(7.6,32,42),(5.2,42,50)]
    for half,y0,y1 in tiers: wall(-half,half,-half,half,y0,y1)
    wall(-5.8,5.8,-5.8,5.8,50,51.2,METAL)
    for half,_,y1 in tiers:   # setback cornices
        wall(-half-.25,half+.25,-half-.25,half+.25,y1-.4,y1,METAL,False)
    wall(-3,3,-11,-5,ROOF,47,DECK)
    front=-11.2
    mesh.quad((-3.4,23,-11.08),(3.4,23,-11.08),(3.4,46,-11.08),(-3.4,46,-11.08),METAL,False)
    mesh.quad((-2.3,31,front),(2.3,31,front),(2.3,45,front),(-2.3,45,front),accent,False)
    mesh.triangle([(-2.3,31,front),(0,28.6,front),(2.3,31,front)],accent,False)
    # Emblem: two stacked upward chevrons (a peak), original to PeakRunner.
    for base_y in (36.2,39.4):
        for s in (-1,1):
            mesh.quad((s*1.7,base_y,front-.12),(s*1.7,base_y+.8,front-.12),
                      (0,base_y+2.6,front-.12),(0,base_y+1.8,front-.12),GLOW,False)
    mesh.quad((-.22,22,front-.12),(.22,22,front-.12),(.22,29.5,front-.12),(-.22,29.5,front-.12),GLOW,False)
    fz=-TOWER_HALF-.55
    mesh.quad((-.22,8,fz),(.22,8,fz),(.22,19.5,fz),(-.22,19.5,fz),GLOW,False)
    for s in (-1,1):
        mesh.quad((s*4,9,fz),(s*5.2,9,fz),(s*5.2,19,fz),(s*4,19,fz),accent,False)
    # Belt courses mark each floor line on the block (the front keeps its
    # centre clear for the strip and bands).
    for y in (L2,L3):
        for side,(axis,plane,into) in tower_faces.items():
            outer = plane-into*2*W
            runs = [(-T-.2,-5.6),(5.6,T+.2)] if side == '-z' else [(-T-.2,T+.2)]
            for u0,u1 in runs: face_box(axis,outer,-into,u0,u1,y-.2,y+.2,.2,METAL)
    for sx in (-1,1):
        for sz in (-1,1):
            wall(sx*12.3,sx*13.7,sz*12.3,sz*13.7,-3,28)
            wall(sx*9.9,sx*11,sz*9.9,sz*11,ROOF,37,METAL)
            mesh.column((sx*6.6,42,sz*6.6),.35,9,METAL,6,top=.08,solid=False)
            mesh.box((sx*13,27,sz*13),(.3,.3,.3),GLOW,False)
    mesh.column((0,51.2,0),.5,9,METAL,6,top=.06,solid=False)
    mesh.box((0,60.2,0),(.5,.5,.5),GLOW,False)

    # Upper keel band around the keel level. Its +x face is split round the
    # hatch housing (z within the housing's outer walls, below its roof).
    kb, kt = 9.5, 12.4
    hz, hy = HATCH_HALF+.4, HATCH_TOP+1
    at_y = lambda y: kb+(kt-kb)*(y-KEEL)/(KEEL_CEILING-KEEL)
    for s in (-1,1):
        mesh.quad((-kb,KEEL,s*kb),(kb,KEEL,s*kb),(kt,KEEL_CEILING,s*kt),(-kt,KEEL_CEILING,s*kt),METAL)
    mesh.quad((-kb,KEEL,-kb),(-kb,KEEL,kb),(-kt,KEEL_CEILING,kt),(-kt,KEEL_CEILING,-kt),METAL)
    mesh.quad((kb,KEEL,-kb),(kb,KEEL,-hz),(kt,KEEL_CEILING,-hz),(kt,KEEL_CEILING,-kt),METAL)
    mesh.quad((kb,KEEL,hz),(kb,KEEL,kb),(kt,KEEL_CEILING,kt),(kt,KEEL_CEILING,hz),METAL)
    mesh.quad((at_y(hy),hy,-hz),(at_y(hy),hy,hz),(kt,KEEL_CEILING,hz),(kt,KEEL_CEILING,-hz),METAL)
    hull(0,0,(4.5,4.5),(9.5,9.5),-22,-8,HULL)
    hull(0,0,(1.3,1.3),(4.5,4.5),-34,-22,METAL)
    thruster(0,-34,0,1.3)
    for sx in (-1,1):
        for sz in (-1,1):
            # Engine pods sit fully below the keel-level floor.
            mesh.column((sx*8,-15,sz*8),1.3,5.5,METAL,8,top=2.1)
            thruster(sx*8,-15,sz*8,1.3)

    # --- Rear tunnels + support pods --------------------------------------
    def corridor_z(x,width,z0,z1,y0,y1,mat=HULL):
        slab(x-width/2,x+width/2,z0,z1,y0,DECK)
        slab(x-width/2,x+width/2,z0,z1,y1,mat)
        wall(x-width/2-.3,x-width/2+.3,z0,z1,y0,y1,mat)
        wall(x+width/2-.3,x+width/2+.3,z0,z1,y0,y1,mat)
        for s in (-1,1):
            dress('x',x+s*(width/2-.3),-s,[(z0+.4,z1)],y0,y1-1,pilasters=False)
        mesh.box((x,y1-1.035,(z0+z1)/2),(.7,.07,z1-z0),METAL,False)
        mesh.box((x,y1-1.07,(z0+z1)/2),(.35,.06,z1-z0-.6),GLOW,False)
        for z in (z0+3,(z0+z1)/2,z1-3): lamp((x,y1-1.3,z),.6)
    corridor_z(-8,4,TOWER_HALF,REAR_Z0,L1,L1+5)
    corridor_z(8,4,TOWER_HALF,REAR_Z0,L1,L1+5)
    rear_mid=(REAR_Z0+REAR_Z1)/2

    def rear_room(x0,x1,door):
        slab(x0,x1,REAR_Z0,REAR_Z1,L1); slab(x0,x1,REAR_Z0,REAR_Z1,L1+ROOM_H,HULL)
        wall(x0,x1,REAR_Z1-.4,REAR_Z1+.4,L1,L1+ROOM_H)
        wall(x0-.4,x0+.4,REAR_Z0,REAR_Z1,L1,L1+ROOM_H)
        wall(x1-.4,x1+.4,REAR_Z0,REAR_Z1,L1,L1+ROOM_H)
        for a,b in [(x0,door[0]),(door[1],x1)]: wall(a,b,REAR_Z0-.4,REAR_Z0+.4,L1,L1+ROOM_H)
        wall(door[0],door[1],REAR_Z0,REAR_Z0+.4,L1+4,L1+ROOM_H)   # header over the tunnel mouth
        cx=(x0+x1)/2
        dress('z',REAR_Z1-.4,-1,[(x0+.4,x1-.4)],L1,L1+ROOM_H-1,pilasters=False)
        dress('x',x0+.4,1,[(REAR_Z0+.4,REAR_Z1-.4)],L1,L1+ROOM_H-1,pilasters=False)
        dress('x',x1-.4,-1,[(REAR_Z0+.4,REAR_Z1-.4)],L1,L1+ROOM_H-1,pilasters=False)
        dress('z',REAR_Z0+.4,1,[(x0+.4,door[0]),(door[1],x1-.4)],L1,L1+ROOM_H-1,pilasters=False)
        for dx in (-2.5,2.5):
            mesh.box((cx+dx,L1+ROOM_H-1.035,rear_mid),(.7,.07,8),METAL,False)
            mesh.box((cx+dx,L1+ROOM_H-1.07,rear_mid),(.35,.06,7.6),GLOW,False)
            for dz in (-2.5,2.5): lamp((cx+dx,L1+ROOM_H-1.3,rear_mid+dz),.7)
        keel(cx,rear_mid,((x1-x0)/2+.4,(REAR_Z1-REAR_Z0)/2+.4),-1,13,1.1)
        for s in (-1,1):
            mesh.quad((cx+s*((x1-x0)/2+.45),2.2,REAR_Z0),(cx+s*((x1-x0)/2+.45),2.2,REAR_Z1),
                      (cx+s*((x1-x0)/2+.45),3.0,REAR_Z1),(cx+s*((x1-x0)/2+.45),3.0,REAR_Z0),accent,False)
        mesh.quad((x0,2.2,REAR_Z1+.45),(x1,2.2,REAR_Z1+.45),(x1,3.0,REAR_Z1+.45),(x0,3.0,REAR_Z1+.45),accent,False)
        return cx

    # Armory, rear-left (the generator's old pod): an inventory station facing
    # the tunnel mouth and a repair pad, so the left tunnel still leads
    # somewhere worth going. Exhaust stacks on the roof.
    gx=rear_room(-13,-3,(-10,-6))
    mesh.equipment('inventory',(-10.5,L1,rear_mid-2.2),team,circuit)
    mesh.equipment('repair',(-6,L1,rear_mid-2.2),team,circuit)
    for dx,h in [(-2.5,6),(0,8),(2.5,6)]:
        mesh.column((gx+dx,L1+ROOM_H,REAR_Z1-2.5),.9,h,METAL,8,top=.7)
        mesh.column((gx+dx,L1+ROOM_H+h,REAR_Z1-2.5),.72,.12,GLOW,8,top=.72,solid=False)

    # Ship platform, rear-right. Its gameplay role beyond a transit point is
    # not settled yet: kept as a light-cover room. The roof reads as the
    # landing pad (markers only, no new collision).
    sx_=rear_room(3,13,(6,10))
    for x,z in [(6,rear_mid-2),(10,rear_mid)]:
        mesh.box((x,1.5,z),(1.2,3,1.2),HULL)
        mesh.box((x,3.07,z),(1.44,.14,1.44),METAL,False)
    for dx in (-4,4):
        for dz in (-4,4): mesh.box((sx_+dx,L1+ROOM_H+.08,rear_mid+dz),(.8,.1,.8),GLOW,False)
    mesh.column((sx_,L1+ROOM_H+.02,rear_mid),3.2,.06,accent,12,top=3.2,solid=False)
    mesh.column((sx_,L1+ROOM_H+.06,rear_mid),2.6,.03,DECK,12,top=2.6,solid=False)

    # --- Front turret pods on open bridges --------------------------------
    for side in (-1,1):
        px = side*9
        mesh.ramp(px,4,-TOWER_HALF,POD_Z+4.2,L1,L1,HAZARD)
        for s in (-1,1):   # bridge rails and posts
            mesh.box((px+s*2.05,L1+1,(-TOWER_HALF+POD_Z+4.2)/2),(.08,.08,-POD_Z-4.2-TOWER_HALF),METAL,False)
            for z in range(-14,POD_Z+4,-4): mesh.box((px+s*2.05,L1+.5,z),(.1,1,.1),METAL,False)
        mesh.column((px,L1-.63,POD_Z),5,.6,METAL,8)
        mesh.column((px,L1-.95,POD_Z),5.1,.3,accent,8,solid=False)
        mesh.column((px,L1-3.2,POD_Z),3.6,2.6,HULL,8,top=5)
        mesh.column((px,L1-11,POD_Z),1,7.8,METAL,8,top=3.6)
        thruster(px,L1-11,POD_Z,1)
        for dx in (-3.4,3.4): mesh.column((px+dx,L1,POD_Z+2.5),.22,4,METAL,6,top=.05,solid=False)
        for dx in (-3.4,3.4): mesh.box((px+dx,L1+4.05,POD_Z+2.5),(.2,.2,.2),GLOW,False)
        mesh.equipment('turret',(px,L1,POD_Z),team,circuit)
        # Visual turret mount around the kit pedestal (r=2): armoured base
        # ring, bolt ring, accent collar and cable boxes. Behaviour unchanged.
        mesh.column((px,L1,POD_Z),2.6,.28,METAL,12,top=2.25,solid=False)
        for a in range(12):
            ang = a*math.tau/12
            mesh.box((px+2.12*math.cos(ang),L1+.3,POD_Z+2.12*math.sin(ang)),(.14,.06,.14),GLOW if a%3==0 else METAL,False)
        mesh.column((px,L1+1.12,POD_Z),2.04,.14,accent,12,solid=False)
        mesh.column((px,L1+1.2,POD_Z),1.25,.12,METAL,10,solid=False)
        for dx in (-1.6,1.6): mesh.box((px+dx,L1+.45,POD_Z-1.9),(.7,.9,.5),METAL,False)

    return {
        'flag': flag,
        'spawn': (-4.2,L1+SPAWN_LIFT,8.5),
        # (x, y, z, local yaw): yaw 0 faces the base front (-Z), pi/2 faces -X.
        'spawn_points': [(-4.2,L1+SPAWN_LIFT,8.5,0), (4.2,L1+SPAWN_LIFT,8.5,0),
                         (-3.5,L2+SPAWN_LIFT,-7.5,-math.pi/2), (4.2,L2+SPAWN_LIFT,-7.5,math.pi/2),
                         # Level 3's windows are open, so no spawn up there.
                         (4,L2+SPAWN_LIFT,8,0), (8,L1+SPAWN_LIFT,30,0),
                         (-11,L1+SPAWN_LIFT,34,-math.pi/2), (5,L1+SPAWN_LIFT,34,-math.pi/2)],
        'entrances': [(0,.2,-TOWER_HALF),(-8,.2,TOWER_HALF),(8,.2,TOWER_HALF),(-9,.2,POD_Z),(9,.2,POD_Z)],
        'generator': GENERATOR,
        'armory': (-8,L1,rear_mid),
        'keel_hatch': (HATCH_OUT+1.5,KEEL,0),
        'keel_view': (6.5,KEEL+.2,6.5),
        'ship_platform': (8,L1,rear_mid),
        'turret_left': (-9,L1,POD_Z),
        'turret_right': (9,L1,POD_Z),
        'level1_view': (4,L1+.2,-2),
        'level2_view': (-2,L2+.2,-3),
        'level3_view': (0,L3+.2,-4),
        'shaft_view': (4,L1+.4,0),
        'bridge_left_view': (-9,L1+.4,-18),
    }
