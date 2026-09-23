"""Raindance field structures, cleaned (asset raindance-structures-v2).

The kit's bunker walls overlapped each other and the floor slab at their
outer faces (visible z-fighting); here they sit on the floor and meet at the
corners. The landing pad's cross marking no longer overlaps its long bars,
and the pad reports deploy slots for future player-placed items. The bridge
and lookout tower are unchanged kit assets.
"""
ASSET_ID = 'raindance-structures-v2'


def bunker(mesh):
    mesh.box((0, 0, 0), (22, .8, 20), 'grate')
    for x in (-10, 10): mesh.box((x, 2.95, -1), (2, 5.1, 18), 'concrete')
    mesh.box((0, 2.95, 9), (22, 5.1, 2), 'concrete')
    mesh.box((0, 6, 0), (24, 1, 22), 'panel')
    mesh.ramp(0, 18, -15, -10, -2, .4)
    for x in (-7, 7): mesh.box((x, 5.3, -9), (.6, .2, 2), 'light', False)


def landing_pad(mesh, team):
    accent = 'ember' if team == 0 else 'glacier'
    mesh.column((0, 0, 0), 11, .35, 'trim', 12)
    mesh.column((0, .35, 0), 9.5, .12, 'grate', 12)
    for x in (-4, 4): mesh.box((x, .5, 0), (.5, .06, 10), accent, False)
    mesh.box((0, .5, 0), (7.5, .06, .5), accent, False)
    mesh.box((8, 1.1, 8), (1.6, 2.2, 1.2), 'panel')
    mesh.box((8, 1.8, 7.37), (1.1, .5, .04), 'light', False)
    deck = .47
    return {'deck': (0, deck, 0),
            'deploy_slots': [(-5.5, deck, -5.5), (5.5, deck, -5.5), (-5.5, deck, 5.5), (4.5, deck, 4.5)]}
