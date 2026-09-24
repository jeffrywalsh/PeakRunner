"""Raindance field structures, cleaned (asset raindance-structures-v3).

The kit's bunker walls overlapped each other and the floor slab at their
outer faces (visible z-fighting); here they sit on the floor and meet at the
corners. The landing pad's cross marking no longer overlaps its long bars,
and the pad reports deploy slots for future player-placed items. The bridge
and lookout tower are unchanged kit assets.

v3: the bunker's ceiling is 7 m above its floor (was 5.1 m) and its back
wall has a second 6 m opening with a ramp down to the ground, so it is no
longer a one-door box. Its floor slab and each landing pad stand on a
foundation that reaches below the ground, so no edge hovers over sloping
terrain (the pads sit at a fixed height; the terrain dips up to 0.7 m under
them).
"""
ASSET_ID = 'raindance-structures-v3'
B_FLOOR_TOP, B_CEIL = .4, 7.4
B_OPEN_W, B_OPEN_TOP = 6.0, 5.9


def bunker(mesh):
    # Floor slab on a foundation reaching 2.8 m below the floor top.
    mesh.box((0, -1.2, 0), (22, 3.2, 20), 'grate')
    wy, wh = (B_FLOOR_TOP+B_CEIL)/2, B_CEIL-B_FLOOR_TOP
    for x in (-10, 10): mesh.box((x, wy, -1), (2, wh, 18), 'concrete')
    # Back wall with a 6 m opening and a lintel over it.
    side = (22-B_OPEN_W)/2
    for x in (-(B_OPEN_W+side)/2, (B_OPEN_W+side)/2): mesh.box((x, wy, 9), (side, wh, 2), 'concrete')
    mesh.box((0, (B_OPEN_TOP+B_CEIL)/2, 9), (B_OPEN_W, B_CEIL-B_OPEN_TOP, 2), 'trim')
    mesh.box((0, B_CEIL+.5, 0), (24, 1, 22), 'panel')
    mesh.ramp(0, 18, -15, -10, -2, B_FLOOR_TOP)
    mesh.ramp(0, B_OPEN_W, 10, 15, B_FLOOR_TOP, -2)
    for x in (-7, 7): mesh.box((x, B_CEIL-.7, -9), (.6, .2, 2), 'light', False)
    mesh.box((0, B_CEIL-.7, 7), (4, .2, .6), 'light', False)


def landing_pad(mesh, team):
    accent = 'ember' if team == 0 else 'glacier'
    mesh.column((0, -4, 0), 10.6, 4, 'concrete', 12)
    mesh.column((0, 0, 0), 11, .35, 'trim', 12)
    mesh.column((0, .35, 0), 9.5, .12, 'grate', 12)
    for x in (-4, 4): mesh.box((x, .5, 0), (.5, .06, 10), accent, False)
    mesh.box((0, .5, 0), (7.5, .06, .5), accent, False)
    mesh.box((8, 1.1, 8), (1.6, 2.2, 1.2), 'panel')
    mesh.box((8, 1.8, 7.37), (1.1, .5, .04), 'light', False)
    deck = .47
    return {'deck': (0, deck, 0),
            'deploy_slots': [(-5.5, deck, -5.5), (5.5, deck, -5.5), (-5.5, deck, 5.5), (4.5, deck, 4.5)]}
