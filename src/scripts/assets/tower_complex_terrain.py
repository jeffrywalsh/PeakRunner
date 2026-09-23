"""Original rolling terrain for Tower Complex.

Generated only from PeakRunner's own hash noise, ridge functions and base
placement; no heightfield from any other game is read, resampled or fitted.

Shape: a basin between the two floating bases, ringed by broken ridges with
passes, each base sitting over a hill shoulder that falls toward the basin, so
a player leaving either base has a long downhill run, a climb, and several
routes around the ring. Rolling fractal detail keeps most of the ground on a
skiable slope; there are few flats.
"""
import numpy as np

N, STEP = 256, 8.0
PAD_CLEARANCE = 34   # deck to ground under a landing pad; keel tip 16 m down
# Midfield peak pairs: (u along the base axis, w across it, height, radius), m.
# Each is mirrored through the field centre.
PEAKS = ((35, 85, 86, 46), (-62, 50, 62, 36), (112, -58, 52, 30), (5, 158, 72, 42), (-150, 118, 48, 38))


def _hash(ix, iz, seed):
    n = (ix.astype(np.uint64)*np.uint64(374761393) + iz.astype(np.uint64)*np.uint64(668265263)
         + np.uint64(seed)*np.uint64(1274126177)) & np.uint64(0xffffffff)
    n = ((n ^ (n >> np.uint64(13)))*np.uint64(1274126177)) & np.uint64(0xffffffff)
    return ((n ^ (n >> np.uint64(16))) & np.uint64(65535)).astype(np.float64)/65535.0


def _value(x, z, seed):
    ix, iz = np.floor(x), np.floor(z)
    u, v = x-ix, z-iz
    u, v = u*u*(3-2*u), v*v*(3-2*v)
    ix, iz = ix.astype(np.int64)+4096, iz.astype(np.int64)+4096
    a, b = _hash(ix, iz, seed), _hash(ix+1, iz, seed)
    c, d = _hash(ix, iz+1, seed), _hash(ix+1, iz+1, seed)
    return (a*(1-u)+b*u)*(1-v)+(c*(1-u)+d*u)*v


def _fbm(x, z, wavelength, octaves, seed, ridged=False):
    total, amp, norm = 0.0, 1.0, 0.0
    for o in range(octaves):
        n = _value(x/wavelength, z/wavelength, seed+o*101)
        if ridged: n = 1-np.abs(2*n-1); n = n*n
        total = total+amp*n; norm += amp
        amp *= .5; wavelength /= 2.03
    return total/norm


def _smooth(a, b, t):
    t = np.clip((t-a)/(b-a), 0, 1)
    return t*t*(3-2*t)


def heights(bases, seed, deck_y=240.0, pads=()):
    """256x256 heights in metres (row = z cell, column = x cell).

    `pads` are (x, z, deck_y) landing pads; the ground under each is held
    PAD_CLEARANCE below its deck so the keel floats and a jet can reach it."""
    z, x = np.mgrid[0:N, 0:N].astype(np.float64)*STEP
    (rx, rz), (bx, bz) = [(b[0], b[2]) for b in bases]
    cx, cz = (rx+bx)/2, (rz+bz)/2
    axis = np.array([bx-rx, bz-rz], dtype=np.float64); half = np.linalg.norm(axis)/2; axis /= 2*half
    # Coordinates along (u) and across (w) the base-to-base line.
    u = (x-cx)*axis[0]+(z-cz)*axis[1]
    w = -(x-cx)*axis[1]+(z-cz)*axis[0]
    # Warp so ridges and valleys curve instead of running straight.
    wu = u+38*(_fbm(x+900, z, 420, 3, seed+7)-.5)*2
    ww = w+46*(_fbm(x, z+900, 380, 3, seed+11)-.5)*2
    r = np.hypot(wu/(half+230), ww/300)

    basin = 112+84*(1-np.exp(-1.3*r*r))              # bowl between the bases
    ring = 78*np.exp(-((r-1.05)/.26)**2)             # encircling ridge
    angle = np.arctan2(ww, wu)
    passes = 1-.78*np.maximum.reduce([np.exp(-((np.angle(np.exp(1j*(angle-a))))/.2)**2)
                                     for a in (1.1, 2.05, -1.1, -2.05)])
    h = basin+ring*passes
    # Each base sits over a shoulder that falls toward the basin, rising
    # behind the base for a downhill start.
    for sx, sz, sign in ((rx, rz, -1), (bx, bz, 1)):
        du = (x-sx)*axis[0]+(z-sz)*axis[1]
        dw = -(x-sx)*axis[1]+(z-sz)*axis[0]
        behind = du*sign
        shoulder = 58*np.exp(-(dw/170)**2)*_smooth(-90, 170, behind)
        h = h+shoulder
    # Midfield peaks: point-symmetric pairs (u,w) / (-u,-w) so neither team's
    # half is favoured. They stand off the flag-to-flag axis (|w| >= 45 m or
    # far forward of a base), leaving a clear line down the middle, and sit
    # clear of the passes and the landing pads. Profiles sharpen toward the
    # summit so they read as peaks, with a warped outline so no two match.
    for pu, pw, ph, pr in PEAKS:
        for s in (1, -1):
            du, dw = wu-s*pu, ww-s*pw
            d = np.hypot(du, dw)*(1+.18*(_fbm(x+s*pu*3, z+s*pw*3, 70, 2, seed+17)-.5)*2)
            h = h+ph*np.exp(-(d/pr)**1.9)
    # Rolling fractal hills everywhere plus ridged detail on the high ground.
    h = h+34*(_fbm(x, z, 240, 4, seed)-.5)*2
    h = h+14*(_fbm(x+700, z+200, 110, 3, seed+9)-.5)*2
    h = h+22*_fbm(x+300, z+500, 190, 3, seed+3, ridged=True)*_smooth(150, 210, h)
    h = h+7*(_fbm(x, z, 48, 2, seed+5)-.5)*2
    # Outer edge: climb into bounding hills beyond the play space.
    edge = np.maximum(np.abs(x-1024), np.abs(z-1024))
    h = h+60*_smooth(800, 1010, edge)
    # Keep every hull clear: ground under each base stays well below the keels.
    for sx, sz in ((rx, rz), (bx, bz)):
        d = np.hypot(x-sx, z-sz)
        cap = deck_y-62+np.maximum(d-48, 0)*.9
        h = np.minimum(h, cap)
    for px, pz, py in pads:
        d = np.hypot(x-px, z-pz)
        h = np.minimum(h, py-PAD_CLEARANCE+np.maximum(d-26, 0)*.8)
    return np.clip(h, 2, 2040)


def slope_degrees(h):
    gz, gx = np.gradient(h, STEP)
    return np.degrees(np.arctan(np.hypot(gx, gz)))


def weights(h):
    """Meadow/rock/soil splat per cell: rock on steep ground, soil on the
    middle slopes and high ground, meadow on gentle low ground."""
    s = slope_degrees(h)
    rock = np.clip((s-24)/14, 0, 1)*.9+.08
    soil = np.clip(1-rock, 0, 1)*np.clip(.25+(s-8)/30+(h-170)/160, 0, .85)
    meadow = np.clip(1-rock-soil, 0, 1)
    out = np.stack([meadow, rock, soil, np.zeros_like(h)], axis=-1)
    return np.round(out*255).astype(np.uint8)
