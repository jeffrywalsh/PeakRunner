"""Deterministic original Cairnhold surfaces: highland stone and bronze.
No reference images are sampled.

Kit slots are repainted for this pack:
  concrete  weathered ashlar masonry for the dug-in walls and plinths
  panel     worn flagstone floors
  grate     bronze tread plate for ramps, thresholds and hazard edges
  trim      dark patinated bronze for frames, bands and fittings
  ember / glacier  heavy woven team banners with a gold hem
  light     warm lamp and seam glow (the map shader has no emissive term)
  bark      pale dressed limestone for interior walls
"""

TEAM_CLOTH = {'ember': (138, 34, 26), 'glacier': (34, 70, 142)}
GOLD = (196, 152, 70)


def texture(name, seed, noise, value_noise):
    palettes = {'concrete': (112, 106, 95), 'panel': (92, 88, 81), 'grate': (142, 101, 54),
                'trim': (60, 55, 47), 'light': (255, 222, 164), 'bark': (156, 148, 132),
                **TEAM_CLOTH}
    data = bytearray()
    for y in range(256):
        for x in range(256):
            grain = (noise(x, y, seed)-.5)*24
            mottling = (value_noise(x/16, y/16, 16, seed)-.5)*38
            rgb = palettes[name]
            if name in TEAM_CLOTH:
                # Coarse basket weave with a gold hem band.
                value = grain*.3+mottling*.3+(7 if (x//3+y//3) % 2 else -7)
                if y % 128 < 8:
                    rgb, value = GOLD, grain*.3-8
            elif name == 'light':
                value = grain*.12+(5 if (x+y) % 24 < 12 else 0)
            elif name == 'concrete':
                # Coursed ashlar: 1 m courses of 2 m / 1.4 m blocks, each block
                # its own tone, rough faces and deep mortar joints.
                course = y//64
                shift = (course*37) % 128
                width = 128 if course % 2 else 90
                bx = (x+shift) % width
                block = int(noise((x+shift)//width, course, seed+11)*9)
                value = grain*.8+mottling*.9+(block-4)*4+(noise(x//5, y//5, seed+3)-.5)*14
                if y % 64 < 3 or bx < 3: value -= 42
                elif y % 64 < 5 or bx < 5: value -= 12
            elif name == 'bark':
                # Pale dressed limestone: tight 0.5 m courses, fine tooling.
                bx = (x+(32 if (y//32) % 2 else 0)) % 96
                value = grain*.35+mottling*.35+(3 if x % 3 == 0 else 0)
                if y % 32 < 2 or bx < 2: value -= 30
            elif name == 'panel':
                # Flagstones: 1 m squares offset every other row, each stone a
                # different tone, with wear in the middle of the room.
                row = y//64
                bx = (x+(32 if row % 2 else 0)) % 64
                stone = int(noise((x+(32 if row % 2 else 0))//64, row, seed+7)*7)
                value = grain*.55+mottling*.7+(stone-3)*5
                if y % 64 < 3 or bx < 3: value -= 34
            elif name == 'grate':
                # Bronze tread plate: raised chevrons over hammered bronze.
                chevron = ((x+abs((y % 32)-16)) % 16) < 4
                value = grain*.5+mottling*.5+(18 if chevron else -6)
                if y % 64 < 2: value -= 30
            else:  # trim
                # Patinated bronze: verdigris streaks in the recesses, rivets.
                patina = value_noise(x/10, y/40, 16, seed+5)
                value = grain*.4+mottling*.3
                if patina > .62: rgb = (52, 78, 66)
                if y % 32 < 2: value -= 20
                if y % 32 == 10 and x % 16 < 3: value += 30
            data.extend(max(0, min(255, round(c+value))) for c in rgb)
            data.append(255)
    return bytes(data)
