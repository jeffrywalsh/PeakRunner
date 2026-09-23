"""Deterministic original Dustreach surfaces: sun-baked sandstone, terracotta,
bleached timber and dark iron, plus desert terrain layers and a clear desert
sky. No reference images are sampled.

Kit slots are repainted for this pack:
  concrete  large sandstone ashlar with worn arrises and wind-scoured faces
  panel     terracotta floor tiles in a herringbone of squares
  grate     sun-bleached timber planking for ramps, lintels and treads
  trim      dark blued iron: bands, frames and fittings
  ember / glacier  dyed banners with an ivory border and a woven sun motif
  light     warm lamp glow (the map shader has no emissive term)
  bark      whitewashed lime plaster for interior walls
  meadow    pale wind-rippled sand (terrain layer 0)
  rock      layered sandstone strata (terrain layer 1)
  soil      darker ochre sand on the middle slopes (terrain layer 2)
  moss      gravel hardpan on the flats (terrain layer 3)
"""
import math

TEAM_CLOTH = {'ember': (150, 40, 28), 'glacier': (30, 78, 150)}
IVORY = (226, 212, 180)
MATERIALS = ('concrete', 'panel', 'grate', 'trim', 'ember', 'glacier', 'light', 'bark',
             'meadow', 'rock', 'soil', 'moss')


def texture(name, seed, noise, value_noise):
    palettes = {'concrete': (184, 146, 100), 'panel': (160, 86, 56), 'grate': (170, 150, 118),
                'trim': (46, 50, 58), 'light': (255, 226, 170), 'bark': (214, 206, 188),
                'meadow': (214, 180, 128), 'rock': (170, 118, 78), 'soil': (178, 128, 80),
                'moss': (150, 132, 108), **TEAM_CLOTH}
    data = bytearray()
    for y in range(256):
        for x in range(256):
            grain = (noise(x, y, seed)-.5)*22
            mottling = (value_noise(x/16, y/16, 16, seed)-.5)*30
            rgb = palettes[name]
            if name in TEAM_CLOTH:
                # Plain weave with an ivory border and a woven sun disc motif.
                value = grain*.3+mottling*.25+(5 if (x+y) % 4 < 2 else -5)
                cx, cy = x % 128-64, y % 128-64
                r = math.hypot(cx, cy)
                if y % 128 < 10 or 22 < r < 28 or r < 12:
                    rgb, value = IVORY, grain*.3-6
            elif name == 'light':
                value = grain*.1+(4 if (x//8+y//8) % 2 else 0)
            elif name == 'concrete':
                # Big sandstone blocks: 1.5 m courses, weathered edges, scour lines.
                course = y//96
                shift = (course*53) % 160
                bx = (x+shift) % 160
                block = int(noise((x+shift)//160, course, seed+11)*9)
                scour = (value_noise(x/40, y/6, 16, seed+4)-.5)*16
                value = grain*.6+mottling*.8+(block-4)*5+scour
                if y % 96 < 3 or bx < 3: value -= 36
                elif y % 96 < 7 or bx < 7: value -= 10
            elif name == 'bark':
                # Lime plaster: soft trowel marks, faint cracks.
                value = grain*.25+mottling*.35+(value_noise(x/30, y/12, 16, seed+2)-.5)*10
                if (noise(x//3, y, seed+8) > .995): value -= 30
            elif name == 'panel':
                # Terracotta tiles: 0.5 m squares with lime grout, each tile its own fire.
                tile = int(noise(x//32, y//32, seed+7)*7)
                value = grain*.5+mottling*.5+(tile-3)*6
                if x % 32 < 2 or y % 32 < 2: rgb, value = (196, 184, 160), grain*.3
            elif name == 'grate':
                # Bleached planks with dark joints, end grain and nail heads.
                plank = y//24
                value = grain*.5+mottling*.4+(noise(plank, 0, seed+3)-.5)*18
                value += (value_noise(x/48, y/3, 16, seed+6)-.5)*14
                if y % 24 < 2: value -= 40
                if (x+plank*37) % 128 < 2: value -= 30
                if y % 24 in (6, 17) and (x+plank*37) % 128 in (8, 9): value -= 45
            elif name == 'trim':
                # Blued iron: hammer marks, rust blooms, rivet rows.
                value = grain*.4+mottling*.3
                rust = value_noise(x/5, y/5, 64, seed+5)
                if rust > .8: rgb, value = (72, 58, 52), value+(rust-.8)*40
                if y % 32 < 2: value -= 16
                if y % 32 == 9 and x % 14 < 3: value += 34
            elif name == 'meadow':
                # Pale sand with fine wind ripples.
                ripple = math.sin((x*.9+y*.45+value_noise(x/20, y/20, 16, seed+9)*18)*math.tau/9)
                value = grain*.35+mottling*.5+ripple*6
            elif name == 'rock':
                # Horizontal sandstone strata with darker bands and cracks.
                band = math.sin((y+value_noise(x/24, y/24, 16, seed+3)*20)*math.tau/22)
                value = grain*.7+mottling*.8+band*14
                if noise(x//4, y//2, seed+12) > .985: value -= 40
            elif name == 'soil':
                # Ochre sand, coarser than the pale layer, wind streaks.
                value = grain*.55+mottling*.7+(value_noise(x/60, y/8, 16, seed+13)-.5)*16
            else:  # moss slot: gravel hardpan
                pebble = noise(x//3, y//3, seed+14)
                value = grain*.6+mottling*.5+(pebble-.5)*34
                if pebble > .93: rgb = (118, 104, 88)
            data.extend(max(0, min(255, round(c+value))) for c in rgb)
            data.append(255)
    return bytes(data)


def sky(face, unit, smooth, cloud_noise):
    """Clear desert sky cube face: warm dusty horizon, deep blue zenith and a
    few high streaks of cloud. Same face order as the kit sky."""
    data = bytearray()
    for y in range(256):
        for x in range(256):
            u, v = x/255*2-1, y/255*2-1
            d = [(u, -v, -1), (1, -v, u), (-u, -v, 1), (-1, -v, -u), (u, 1, -v), (u, -1, v)][face]
            dx, dy, dz = unit(d)
            streak = sum((cloud_noise(dx*f*3+5, dy*f+2, dz*f*.6+9, 71)-.5)/2**i for i, f in enumerate([3, 7, 16]))
            cloud = smooth((streak+.02)/.34)*.55
            horizon = smooth(dy/.5) if dy > 0 else 0
            haze = (232, 206, 164)
            zenith = (70, 122, 188)
            for low, high in zip(haze, zenith):
                c = low*(1-horizon)+high*horizon
                c = c*(1-cloud*horizon)+248*cloud*horizon
                data.append(max(0, min(255, int(c))))
            data.append(255)
    return bytes(data)
