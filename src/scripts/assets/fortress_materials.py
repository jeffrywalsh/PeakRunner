"""Deterministic original fortress surfaces. No reference images are sampled."""

def texture(name, seed, noise, value_noise):
    palettes = {'concrete': (123, 121, 115), 'panel': (102, 119, 123),
                'grate': (83, 99, 103), 'trim': (39, 49, 54)}
    data = bytearray()
    for y in range(256):
        for x in range(256):
            grain = (noise(x, y, seed) - .5) * 27
            mottling = (value_noise(x/16, y/16, 16, seed) - .5) * 39
            value = grain + mottling
            if name == 'concrete':
                # Coarse aggregate without a checkerboard on every wall.
                value += (noise(x//2, y//2, seed+3) - .5) * 19
                if x < 2 or y < 2: value -= 12
            elif name == 'panel':
                # Long brushed deck plates, recessed seams and small rivets.
                value = grain*.65 + mottling*.3
                if x % 64 < 3: value -= 45
                elif x % 64 < 5: value += 23
                if y < 3: value -= 30
                if (x % 64-9)**2 + (y % 128-10)**2 < 6: value -= 45
            elif name == 'grate':
                # Ribbed anti-slip ramps, not an intersecting diamond grid.
                value = grain*.7 + (19 if y % 16 < 3 else -8)
                if x % 64 < 3: value -= 22
            else:
                value = grain*.4 + (12 if x % 32 < 4 else 0)
            data.extend(max(0, min(255, round(c+value))) for c in palettes[name])
            data.append(255)
    return bytes(data)
