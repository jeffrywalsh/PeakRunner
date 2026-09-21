"""Original reusable fortress, authored from measured Broadside proportions.

No source mesh, texture, lightmap or parser is used by this builder. Local X/Z
are horizontal, Y is up; deck Y=0. Caller supplies placement and ownership.
"""
from assets.fortress_rooms import stairwell

ASSET_ID = 'floating-fortress-v9'

def shell(mesh,bottom,top,y0,y1,mat,doors=False,center0=0,center1=0):
    bx,bz=bottom;tx,tz=top
    def q(*points):
        mesh.quad(*[(x,y,z+center0+(center1-center0)*(y-y0)/(y1-y0)) for x,y,z in points],mat)
    for side in [-1,1]:
        if doors:
            f=7/(y1-y0); dx=bx+(tx-bx)*f;dz=bz+(tz-bz)*f
            for a,b in [(-1,-.16),(.16,1)]:
                q((bx*a,y0,side*bz),(bx*b,y0,side*bz),
                  (dx*b,y0+7,side*dz),(dx*a,y0+7,side*dz))
            q((-dx,y0+7,side*dz),(dx,y0+7,side*dz),
              (tx,y1,side*tz),(-tx,y1,side*tz))
        else:
            q((-bx,y0,side*bz),(bx,y0,side*bz),(tx,y1,side*tz),(-tx,y1,side*tz))
        q((side*bx,y0,-bz),(side*bx,y0,bz),(side*tx,y1,tz),(side*tx,y1,-tz))

def build(mesh,team,circuit):
    if team not in (0,1) or not circuit: raise ValueError('team and circuit required')
    accent=['ember','glacier'][team]
    def slab(x0,x1,z0,z1,y,mat='panel'):
        mesh.box(((x0+x1)/2,y-.5,(z0+z1)/2),(x1-x0,1,z1-z0),mat)
    def wall(x0,x1,z0,z1,y0,y1,mat='concrete'):
        if x0>x1: x0,x1=x1,x0
        if z0>z1: z0,z1=z1,z0
        if y0>y1: y0,y1=y1,y0
        mesh.box(((x0+x1)/2,(y0+y1)/2,(z0+z1)/2),(x1-x0,y1-y0,z1-z0),mat)
    def cross_ramp(x0,x1,z,width,y0,y1):
        mesh.quad((x0,y0,z-width/2),(x1,y1,z-width/2),
                  (x1,y1,z+width/2),(x0,y0,z+width/2),'grate')
    def light(x,y,z):mesh.box((x,y,z),(3,.16,2),'light',solid=False)

    # 88 x 104 m flight deck. Disjoint rectangles leave the sunken route open.
    for a,b in [(-44,-32),(32,44)]: slab(a,b,-52,52,0)
    for a,b in [(-32,-5),(5,32)]: slab(a,b,-40,40,0)
    # South (entrance-side) deck edge steps back around the entrance housing
    # instead of staying flush; the reference's own wall-cuts show this notch
    # recurring at the deck, 14 m and 21 m levels (see
    # broadside-reference-study.md's in-engine-measurement section).
    slab(-32,-14,-52,-40,0);slab(14,32,-52,-40,0);slab(-14,14,-52,-44,0)
    slab(-32,32,40,52,0)
    slab(-5,5,-40,-20,0);slab(-5,5,22,40,0)
    # Exact keel dimensions come from scripts/measure-broadside.py's panel
    # reconstruction of the private in-engine reference (see
    # broadside-reference-study.md's exact-measurement section): a wide
    # sloped hull funnel from the deck edge down to about half its radius by
    # -24 m, wrapping a much narrower ~5.5-7 m internal shaft that continues
    # almost straight down to a near-point around -68 m. The previous keel
    # kept widening well past where the reference's own funnel stops.
    shell(mesh,(32,40),(44,52),-8,0,'trim')
    shell(mesh,(24,26),(32,40),-24,-8,'concrete')
    shell(mesh,(7,7),(24,26),-30,-24,'concrete')
    shell(mesh,(5.4,5.4),(7,7),-42,-30,'concrete')
    shell(mesh,(1,1),(5.4,5.4),-68,-42,'trim')
    slab(-1,1,-.5,.5,-68)
    shell(mesh,(32,40),(24,26),0,32,'concrete',doors=True,center1=6)
    # Central sections show the OUTER wall holding its width to the roof
    # terrace at 57 m. The narrower lines inside it are room walls, not an
    # exterior taper. Keep the roof housing distinct from that main tower.
    shell(mesh,(24,26),(24,26),32,57,'concrete',center0=6,center1=6)
    shell(mesh,(16,18),(12,13),57,67,'panel',center0=6,center1=6)
    slab(-12,12,-7,19,67,'trim')
    for x in [-24,24]:wall(x-.25,x+.25,-20,32,32,33,accent)
    for z in [-20,32]:wall(-24,24,z-.25,z+.25,32,33,accent)
    # Flanking turret pylons near the roof terrace. Earlier passes placed
    # these mid-tower by eye; the reference mission's own parsed SentryTurret
    # objects (scripts/measure-broadside.py's sibling equipment extraction,
    # see broadside-reference-study.md) measure at height ~51 m, not ~26 m —
    # near the crown, not the neck. Height corrected; the tower's radius here
    # is still the constant 24x26 m of the main shell, so x=30 stays outside it.
    for side in [-1,1]:mesh.box((side*30,51,6),(4,1,4),'trim')

    # Deck entrance: 0 -> -6 -> 0 with inventory alcoves at the low point.
    mesh.ramp(0,10,-20,-2,0,-6)
    slab(-16,16,-2,6,-6,'grate')
    mesh.ramp(0,10,6,22,-6,0)
    for x in [-13,13]:mesh.equipment('inventory',(x,-6,2),team,circuit)
    for x in [-5.5,5.5]:
        wall(x-.5,x+.5,-35,-20,0,6)
        wall(x-.5,x+.5,-20,-2,-6,6)
        # Stop before the turn so a full-radius body can cross onto the
        # paired hall ramps; ending at z=22 pinched the landing closed.
        wall(x-.5,x+.5,6,21,-6,6)
    slab(-5,5,-35,-18,7)
    # Enclosed descending and ascending entry tunnels, not ramps visible
    # through a multi-storey atrium. Roof slabs follow the passage gradient.
    mesh.ramp(0,10,-18,-2,6.33,1,'panel')
    slab(-5,5,-2,6,1,'panel')
    mesh.ramp(0,10,6,22,1,7,'panel')
    for side in [-1,1]:
        wall(side*16-.5,side*16+.5,-2,6,-6,0)
        light(side*11,-.8,2)
    # Rear turn and paired ramps bring the passage into the 7 m equipment hall.
    for side in [-1,1]:mesh.ramp(side*11,10,22,30,0,7)
    slab(-24,24,-18,-2,7)
    slab(-12,12,-2,18,7)
    slab(-24,24,18,22,7)
    slab(-24,-16,22,30,7);slab(16,24,22,30,7)
    slab(-6,6,22,30,7)
    slab(-24,24,30,32,7)
    for x in [-9,9]:mesh.equipment('inventory',(x,7,10),team,circuit)
    mesh.equipment('repair',(0,7,-12),team,circuit)
    for x in [-11.5,11.5]:wall(x-.5,x+.5,-2,14,7,13)
    # Direct reference rays from (0,10,9), local [x,z,height], hit
    # x=+-11, z=-7.5 and z=22.5. Bound the hall instead of leaving it
    # visually connected to distant outer shell walls.
    wall(-11,11,-8.5,-7.5,7,14)
    wall(-11,11,22.5,23.5,7,14)
    light(0,22.8,8)
    # Split ramps rise to flag-level perimeter circulation.
    for x in [-18,18]:mesh.ramp(x,4,-2,12,7,14)
    slab(-24,24,-18,-7,14)
    slab(-3,3,-15,-9,14.3,accent)
    # Central partition plus side connections: nearest-hit rays disprove
    # the previous bounding-box inference of a single wide central opening.
    wall(-12,12,-8,-7,14,20)
    for a,b in [(-24,-18),(18,24)]:wall(a,b,-7.5,-6.5,14,20)
    wall(-24,24,-7.5,-6.5,20,21,'trim')
    slab(-24,24,-18,-7,21)
    # Solid flag-room side/back walls and framed twin gallery doors.
    wall(-24,24,-18.5,-17.5,14,22)
    wall(-12,12,-17,-16,14,20)
    for side in [-1,1]:
        wall(side*24-.5,side*24+.5,-18,-7,14,22)
        mesh.box((side*15,19.8,-6.4),(6,.3,.3),'light',solid=False)
    # Low dark skirting and chamfered ceiling shoulders give the passages
    # an enclosed room scale without changing tested circulation routes.
    for side in [-1,1]:
        wall(side*12-.12,side*12+.12,-2,14,7,7.45,'trim')
        mesh.quad((side*12,21,-2),(side*10,23,-2),
                  (side*10,23,14),(side*12,21,14),'panel')
    light(0,19.8,-12)
    # Gallery bays are the full width from the hall edge to the inner tower
    # wall. A 4 m corridor here made the flag-gallery sample read 1 m / 3 m
    # instead of 9 m / 3 m. The hall void stays open between those edges.
    def span(x0,x1,z0,z1,y,mat='panel'):
        if x0>x1: x0,x1=x1,x0
        if z0>z1: z0,z1=z1,z0
        slab(x0,x1,z0,z1,y,mat)
    def plate(x0,x1,z0,z1,top,thick=0.4,mat='panel'):
        if x0>x1: x0,x1=x1,x0
        if z0>z1: z0,z1=z1,z0
        mesh.box(((x0+x1)/2,top-thick/2,(z0+z1)/2),(x1-x0,thick,z1-z0),mat)
    for side in [-1,1]:
        span(side*12,side*16,-7,22,14)
        span(side*16,side*20,-7,-2,14)
        span(side*16,side*20,12,22,14)
        span(side*20,side*24,-7,22,14)
        span(side*12,side*19,22,30,14)
        span(side*23,side*24,22,30,14)
        # Ceiling of the gallery is the next walkable loft. Underside at 20.
        span(side*12,side*24,-7,22,21)
        span(side*12,side*19,22,30,21)
        span(side*23,side*24,22,30,21)
        span(side*12,side*24,30,32,21)
        wall(side*24,side*24.4,-7,30,14,21)
        wall(side*12,side*11.6,-7,14,14,21)
        wall(side*12,side*11.6,16,20,14,21)
        light(side*18,19.8,8)
        stairwell(mesh,side*21,4,22,30,14,21,clearance=4)
    wall(-24,24,30.15,30.4,14,20)
    span(-12,12,21,28,14)
    span(-12,12,21,28,21)
    # Partial cover over the rear of the hall. The sample at z=10 still sees
    # the lid at 23, not a floor at 14.
    span(-10,10,13,18,14)
    span(-11,11,-6,22,24)
    # Front armory: a 6 m central bay between side rooms, then side halls.
    span(-20,20,-16,-6,25)
    span(-22,-18,-6,16,25)
    span(18,22,-6,16,25)
    for x in [-3.25,3.25]:
        wall(x-.25,x+.25,-16,-10,25,31)
    wall(-3,3,-4.7,-4.3,25,31)
    span(-20,20,-16,-6,32)
    for x in [-12,12]:mesh.equipment('inventory',(x,25,-13),team,circuit)
    for side in [-1,1]:
        mesh.ramp(side*16,4,10,-6,21,25)
        mesh.ramp(side*6,3,-6,-2,25,31)
    # Ring around the shaft. The spine sample sits in the side bay, not in
    # the hole, with the shaft wall a few metres inward.
    for side in [-1,1]:
        span(side*2,side*8,-2,5,31)
        span(side*2,side*8,5,18,31)
        wall(side*8.2,side*7.8,5,18,31,37)
        wall(side*1.3,side*1.7,6,16,31,37)
        wall(side*2,side*8,16.1,16.5,31,37)
        wall(side*2,side*8,-0.2,0.2,31,37)
    span(-2,2,-2,0,31)
    span(-8,8,18,26,31)
    stairwell(mesh,0,4,0,6,31,38,clearance=4)
    # Generator deck wraps a floor opening. The opening has no tall curb:
    # a standing ray crosses it and hits the far wall.
    for side in [-1,1]:
        span(side*2,side*22,-1,2,38)
        span(side*12,side*22,2,8,38)
        span(side*2,side*12,2,6,38)
        span(side*5,side*12,6,18,38)
        wall(side*12.15,side*11.85,6,18,38.2,44)
        span(side*2,side*12,18,26,38)
        span(side*2,side*12,26,28,38)
    span(-2,2,-1,0,38)
    span(-5,5,6,8,38)
    span(-2,2,18,20,38)
    wall(-12,12,-2.6,-2.2,38.2,43.4)
    wall(-12,12,27.1,27.5,38.2,43.4)
    for x in [-16,16]:mesh.equipment('generator',(x,38,5),team,circuit)
    stairwell(mesh,0,4,20,26,38,44,clearance=4)
    # Defence floor. Top at 44 so a probe 3 m up reads the reference floor.
    # The roof underside at 56 is this room's ceiling.
    for side in [-1,1]:
        plate(side*2,side*22,-2,28,44)
        wall(side*3.4,side*3.0,8,16,44,51)
        wall(side*5.15,side*4.85,-2,3,46,52)
    wall(-12,-6,3.6,4.0,46,52)
    wall(6,12,3.6,4.0,46,52)
    plate(-2,2,-2,8,44)
    plate(-2,2,16,20,44)
    plate(-2,2,26,28,44)
    stairwell(mesh,0,4,8,18,44,57,clearance=5)
    slab(-24,-4,-20,32,57);slab(4,24,-20,32,57)
    slab(-4,4,-20,-6,57);slab(-4,4,18,32,57)
    for x in [-7,7]:
        for z in [-6,14]:wall(x-1,x+1,z-1,z+1,57,63,'trim')
    # Small cap and mast above the crown, where the hull tapers back in to a
    # spire rather than staying wide all the way up.
    for y,z in [(23,-1),(37,0),(44,0)]:light(0,y,z)
    for x in [-35,35]:
        for z in [-34,34]:
            mesh.column((x,-15,z),4,7,'trim',top=5)
            mesh.column((x,-23,z),2,8,'glacier',top=3,solid=False)
    for x,y,z in [(-25,-34,-30),(25,-54,-30)]:slab(x-8,x+8,z-9,z+9,y)
    for side in [-1,1]:mesh.equipment('turret',(side*30,51,6),team,circuit,'plasma')
    mesh.equipment('sensor',(0,67,0),team,circuit)
    return {'flag':(0,15.05,-12),'spawn':(-9,15.2,-12),
            'entrances':[(0,1.2,-40),(0,1.2,40)],
            'hall':(0,8.2,8),'loft':(-15,15.2,18)}
