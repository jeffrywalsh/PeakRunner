"""Original rolling dune terrain for Dustreach.

Generated only from PeakRunner's own hash noise (shared with Tower Complex's
terrain module) and an authored profile; no heightfield from any other game is
read, resampled or fitted. The layout is exactly point-symmetric about the map
centre (1024, 1024), so neither team has a better half.

Along the flag axis (world Z, red at low Z): a dune ridge behind each citadel
for a downhill start, the citadel's plateau, a basin in front of it, a
transverse dune crest, a dip, and the raised central saddle carrying the Sun
Gate. Long crescent dunes run diagonally across the whole field: a gentle
windward face and a steeper lee face, warped so no two crests line up.

Structures sit in "sites": the ground is blended toward an authored surface
around each one. The only cuts are the whole-cell holes under each citadel's
underground level (dustreach_citadel.HOLES); every one lies under the terrace
or the tower, so the heights here are never pinned.
"""
from functools import lru_cache

import numpy as np

from assets.tower_complex_terrain import _fbm, _smooth, slope_degrees, N, STEP

CENTRE = 1024.0
BASE_D = 345.0             # citadel origin distance from the centre along Z
BASE_Y = 130.0             # citadel plateau height
# Authored profile along the axis: (distance from centre, height).
PROFILE = ((0, 152), (85, 131), (150, 141), (240, 114), (BASE_D, BASE_Y), (430, 142), (560, 168))
DUNE, DUNE_WAVE, DUNE_ANGLE = 11.5, 124.0, .62
ROLL, FINE = 10.0, 2.4


def _profile(d):
    """Smoothstep interpolation through PROFILE; level at each control point."""
    h = np.full_like(d, float(PROFILE[0][1]))
    for (d0, h0), (d1, h1) in zip(PROFILE, PROFILE[1:]):
        h = np.where(d >= d0, h0+(h1-h0)*_smooth(d0, d1, d), h)
    return h


def _dunes(x, z, seed):
    """Crescent dunes: an asymmetric saw profile along a warped diagonal."""
    X, Z = x-CENTRE, z-CENTRE
    c, s = np.cos(DUNE_ANGLE), np.sin(DUNE_ANGLE)
    warp = (_fbm(x+700, z+200, 320, 3, seed+31)-.5)*1.4
    phase = (X*c+Z*s)/DUNE_WAVE+warp
    f = phase-np.floor(phase)
    # Windward face rises over 70 % of the wavelength, the lee drops over 30 %.
    rise = _smooth(0, .7, f)
    fall = 1-_smooth(.7, 1.0, f)
    shape = np.where(f < .7, rise, fall)
    strength = .45+.55*_fbm(x+100, z+900, 260, 2, seed+33)
    return DUNE*strength*(shape-.5)*2


def _half(x, z, seed):
    X = x-CENTRE
    h = _dunes(x, z, seed)
    # High dune banks along both sides of the field channel the crossings.
    for side in (-1, 1):
        bank = 30*np.exp(-((X-side*330)/120)**2)
        h = h+bank*(.6+.4*_fbm(x, z+side*600, 240, 3, seed+23))
    h = h+ROLL*(_fbm(x, z, 150, 4, seed)-.5)*2
    h = h+FINE*(_fbm(x+900, z+100, 36, 2, seed+5)-.5)*2
    return h


# Dune detail is damped inside the flag-axis corridor so the authored basin ->
# crest -> saddle sequence still reads along the main lane.
AXIS_DAMP, AXIS_WIDTH = .45, 60.0


def _detail(x, z, seed):
    d = (_half(x, z, seed)+_half(2*CENTRE-x, 2*CENTRE-z, seed))/2
    return d*(1-AXIS_DAMP*np.exp(-((x-CENTRE)/AXIS_WIDTH)**2))


@lru_cache(maxsize=8)
def _offset(seed):
    """Mean detail over the flag-axis corridor, from a fixed sample grid."""
    z, x = np.mgrid[680:1369:8, 960:1089:8].astype(np.float64)
    return float(_detail(x, z, seed).mean())


def natural(x, z, seed):
    """Point-symmetric natural ground before any structure site."""
    h = _profile(np.abs(z-CENTRE))
    h = h+_detail(x, z, seed)-_offset(seed)
    edge = np.maximum(np.abs(x-CENTRE), np.abs(z-CENTRE))
    return h+60*_smooth(760, 1010, edge)


def heights(seed, sites=()):
    """256x256 heights in metres (row = z cell, column = x cell).
    `sites` are (surface(x,z)->height, weight(x,z)->0..1) pairs over world grids."""
    z, x = np.mgrid[0:N, 0:N].astype(np.float64)*STEP
    h = natural(x, z, seed)
    for surface, weight in sites:
        w = weight(x, z)
        h = h*(1-w)+surface(x, z)*w
    return np.clip(h, 2, 2040)


def weights(h):
    """Sand/sandstone/ochre/hardpan splat: bare sandstone on steep faces,
    darker ochre sand on the middle slopes, gravel hardpan on the flats and
    pale wind-rippled sand everywhere else."""
    s = slope_degrees(h)
    rock = np.clip((s-27)/12, 0, 1)*.9+.04
    ochre = np.clip(1-rock, 0, 1)*np.clip((s-11)/22, 0, .7)
    hardpan = np.clip(1-rock-ochre, 0, 1)*np.clip((7-s)/6, 0, .8)
    sand = np.clip(1-rock-ochre-hardpan, 0, 1)
    out = np.stack([sand, rock, ochre, hardpan], axis=-1)
    return np.round(out*255).astype(np.uint8)
