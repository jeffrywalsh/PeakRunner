"""Original steep snow-mountain terrain for Frostline.

Generated only from PeakRunner's own hash noise (shared with Tower Complex's
terrain module) and an authored profile; no heightfield from any other game
is read, resampled or fitted. The layout is exactly point-symmetric about the
map centre (1024, 1024), so neither team has a better half.

Along the flag axis (world Z, red at low Z): each station sits on a shelf high
on its home massif, which climbs to peaks behind it. In front of the shelf the
ground drops steeply into a deep valley, then climbs a long snowfield to the
central ridge and its beacon. Broken side ridges with passes give flank
routes, and ridged fractal detail keeps most of the ground steep.
"""
from functools import lru_cache

import numpy as np

from assets.tower_complex_terrain import _fbm, _smooth, slope_degrees, N, STEP

CENTRE = 1024.0
# Authored profile along the axis (m from centre -> height).
RIDGE, VALLEY, SHELF, PEAKS = 262.0, 168.0, 230.0, 330.0
VALLEY_D, SHELF_D0, SHELF_D1, PEAK_D = 215.0, 345.0, 455.0, 585.0
# Detail amplitudes (m) tuned for a ~36 degree median slope.
RIDGED, ROLL, FINE = 110, 40, 13
AXIS_DAMP, AXIS_WIDTH = .72, 60.0


def _profile(d):
    ridge_to_valley = RIDGE+(VALLEY-RIDGE)*_smooth(0, VALLEY_D, d)
    valley_to_shelf = VALLEY+(SHELF-VALLEY)*_smooth(VALLEY_D, SHELF_D0, d)
    shelf_to_peaks = SHELF+(PEAKS-SHELF)*_smooth(SHELF_D1, PEAK_D, d)
    return np.where(d < VALLEY_D, ridge_to_valley, np.where(d < SHELF_D1, valley_to_shelf, shelf_to_peaks))


def _half(x, z, seed):
    """One unsymmetrised copy of the side ridges and mountain detail."""
    X, Z = x-CENTRE, z-CENTRE
    h = np.zeros_like(x)
    wx = X+50*(_fbm(x+700, z, 320, 3, seed+31)-.5)*2
    for side in (-1, 1):
        ridge = 95*np.exp(-((wx-side*250)/90)**2)
        spur = .5+.5*_fbm(x, z+side*900, 240, 3, seed+33)
        passes = 1-.6*np.exp(-((np.abs(Z)-150)/60)**2)
        h = h+ridge*spur*passes
    h = h+RIDGED*_fbm(x+200, z+700, 170, 5, seed+7, ridged=True)
    h = h+ROLL*(_fbm(x, z, 120, 4, seed+2)-.5)*2
    h = h+FINE*(_fbm(x+500, z+300, 36, 2, seed+9)-.5)*2
    return h


def _detail(x, z, seed):
    d = (_half(x, z, seed)+_half(2*CENTRE-x, 2*CENTRE-z, seed))/2
    return d*(1-AXIS_DAMP*np.exp(-((x-CENTRE)/AXIS_WIDTH)**2))


@lru_cache(maxsize=8)
def _offset(seed):
    """Mean detail over the flag-axis corridor, so the authored profile
    heights hold there on average."""
    z, x = np.mgrid[560:1489:8, 960:1089:8].astype(np.float64)
    return float(_detail(x, z, seed).mean())


def natural(x, z, seed):
    """Point-symmetric natural ground before any structure site."""
    h = _profile(np.abs(z-CENTRE))+_detail(x, z, seed)-_offset(seed)
    edge = np.maximum(np.abs(x-CENTRE), np.abs(z-CENTRE))
    return h+90*_smooth(700, 1010, edge)


def heights(seed, sites=()):
    """256x256 heights in metres (row = z cell, column = x cell). `sites`
    are (surface(x,z)->height, weight(x,z)->0..1) pairs over world grids."""
    z, x = np.mgrid[0:N, 0:N].astype(np.float64)*STEP
    h = natural(x, z, seed)
    for surface, weight in sites:
        w = weight(x, z)
        h = h*(1-w)+surface(x, z)*w
    return np.clip(h, 2, 2040)


def weights(h):
    """Four splat channels painted over the kit's terrain slots:
    fresh snow, bare granite, wind-scoured ice crust, snowy scree."""
    s = slope_degrees(h)
    rock = np.clip((s-44)/16, 0, 1)*.85+.02
    scree = np.clip(1-rock, 0, 1)*np.clip((s-30)/20, 0, .55)
    ice = np.clip(1-rock-scree, 0, 1)*np.clip((190-h)/30, 0, .8)
    snow = np.clip(1-rock-scree-ice, 0, 1)
    out = np.stack([snow, rock, ice, scree], axis=-1)
    return np.round(out*255).astype(np.uint8)


__all__ = ['heights', 'natural', 'weights', 'slope_degrees', 'N', 'STEP', 'CENTRE']
