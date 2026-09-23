"""Original rugged highland terrain for Cairnhold.

Generated only from PeakRunner's own hash noise (shared with Tower Complex's
terrain module) and an authored profile; no heightfield from any other game
is read, resampled or fitted. The layout is exactly point-symmetric about the
map centre (1024, 1024), so neither team has a better half.

Along the flag axis (world Z, red at low Z): a flag knoll behind each bunker,
a transverse valley in front of it, and a raised central mesa carrying the
Ring. Broken ridges run down both sides of the field for long ski runs.
Ridged fractal detail keeps almost all of the ground steep.

Structures sit in "sites": the ground is blended toward an authored surface
around each one, and cells a structure is dug into become terrain holes whose
boundary vertices are pinned to heights the structure's walls cover.
"""
from functools import lru_cache

import numpy as np

from assets.tower_complex_terrain import _fbm, _smooth, slope_degrees, N, STEP

CENTRE = 1024.0
# Authored profile along the axis (m): knoll behind each bunker, valley in
# front of it, central mesa.
KNOLL, VALLEY, MESA = 226.0, 186.0, 246.0
KNOLL_Z, VALLEY_Z = 260.0, 110.0
# Detail amplitudes (m) tuned for a 26-30 degree median slope.
RIDGED, ROLL, FINE = 52.0, 25.0, 8.0


def _profile(Z):
    """Smooth piecewise profile through mesa (Z=0), valleys (|Z|=VALLEY_Z)
    and knolls (|Z|=KNOLL_Z), level at each extreme."""
    d = np.abs(Z)
    inner = MESA+(VALLEY-MESA)*_smooth(0, VALLEY_Z, d)
    outer = VALLEY+(KNOLL-VALLEY)*_smooth(VALLEY_Z, KNOLL_Z, d)
    return np.where(d < VALLEY_Z, inner, outer)


def _half(x, z, seed):
    """One unsymmetrised copy of the rugged detail and side ridges."""
    X, Z = x-CENTRE, z-CENTRE
    h = np.zeros_like(x)
    # Side ridges, broken into spurs, with a pass on each side of the mesa.
    wx = X+40*(_fbm(x+500, z, 300, 3, seed+21)-.5)*2
    for side in (-1, 1):
        ridge = 58*np.exp(-((wx-side*230)/95)**2)
        spur = .55+.45*_fbm(x, z+side*800, 260, 3, seed+23)
        passes = 1-.55*np.exp(-((Z-side*60)/70)**2)
        h = h+ridge*spur*passes
    # Rugged detail: ridged fractal at two scales plus rolling fbm.
    h = h+RIDGED*_fbm(x+300, z+500, 150, 4, seed+3, ridged=True)
    h = h+ROLL*(_fbm(x, z, 110, 4, seed)-.5)*2
    h = h+FINE*(_fbm(x+900, z+100, 40, 2, seed+5)-.5)*2
    return h


# The rugged detail is damped inside the flag-axis corridor so the authored
# knoll -> valley -> mesa sequence reads along the main lane.
AXIS_DAMP, AXIS_WIDTH = .8, 70.0


def _detail(x, z, seed):
    d = (_half(x, z, seed)+_half(2*CENTRE-x, 2*CENTRE-z, seed))/2
    return d*(1-AXIS_DAMP*np.exp(-((x-CENTRE)/AXIS_WIDTH)**2))


@lru_cache(maxsize=8)
def _offset(seed):
    """Mean detail over the flag-axis corridor, from a fixed sample grid, so
    the authored profile heights hold there on average."""
    z, x = np.mgrid[694:1355:8, 944:1105:8].astype(np.float64)
    return float(_detail(x, z, seed).mean())


def natural(x, z, seed):
    """Point-symmetric natural ground before any structure site."""
    Z = z-CENTRE
    h = _profile(np.clip(Z, -KNOLL_Z, KNOLL_Z))
    # Beyond each knoll the ground climbs into back-country hills.
    h = h+30*_smooth(KNOLL_Z, KNOLL_Z+140, np.abs(Z))
    h = h+_detail(x, z, seed)-_offset(seed)
    edge = np.maximum(np.abs(x-CENTRE), np.abs(z-CENTRE))
    return h+70*_smooth(760, 1010, edge)


def heights(seed, sites=(), rings=None):
    """256x256 heights in metres (row = z cell, column = x cell).

    `sites` are (surface(x,z)->height or None, weight(x,z)->0..1) pairs over
    world grids; `rings` maps (ix, iz) grid vertices to pinned heights."""
    z, x = np.mgrid[0:N, 0:N].astype(np.float64)*STEP
    h = natural(x, z, seed)
    for surface, weight in sites:
        w = weight(x, z)
        h = h*(1-w)+surface(x, z)*w
    for (ix, iz), y in (rings or {}).items():
        h[iz, ix] = y
    return np.clip(h, 2, 2040)


def weights(h):
    """Meadow/rock/soil/moss splat: bare rock on the steep faces, soil on the
    middle slopes, meadow in sheltered low ground, moss on high benches."""
    s = slope_degrees(h)
    rock = np.clip((s-28)/16, 0, 1)*.88+.08
    soil = np.clip(1-rock, 0, 1)*np.clip(.2+(s-10)/34, 0, .8)
    moss = np.clip(1-rock-soil, 0, 1)*np.clip((h-238)/20, 0, 1)
    meadow = np.clip(1-rock-soil-moss, 0, 1)
    out = np.stack([meadow, rock, soil, moss], axis=-1)
    return np.round(out*255).astype(np.uint8)
