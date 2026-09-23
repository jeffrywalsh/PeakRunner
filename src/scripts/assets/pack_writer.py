"""Shared assembly for original map packs: terrain sampling, material
overrides, the lightmap bake and the six-file pack write.

Used by build-tower-complex.py and build-cairnhold.py. Output layout matches
what crates/core/src/map_pack.rs validates: six payloads plus map.json with
their SHA-256 digests.
"""
import hashlib
import json
from pathlib import Path

# The renderer's fixed map sun (src/map_scene.rs uniform), so baked shadows on
# the structures agree with the terrain's live lighting.
MAP_SUN = (-0.57735, 0.57735, -0.57735)


def sample_height(grid, x, z, step=8):
    """Bilinear sample of a 256x256 height grid, quantised like height.bin,
    matching the engine's heightfield lookup."""
    fx, fz = min(max(x/step, 0), 255), min(max(z/step, 0), 255)
    x0, z0 = int(fx), int(fz); x1, z1 = min(x0+1, 255), min(z0+1, 255)
    tx, tz = fx-x0, fz-z0
    q = lambda ix, iz: round(float(grid[iz, ix])*32)/32
    a = q(x0, z0)+(q(x1, z0)-q(x0, z0))*tx; b = q(x0, z1)+(q(x1, z1)-q(x0, z1))*tx
    return a+(b-a)*tz


def paint_materials(textures, count, materials, texture, seed, kit):
    """Replace each named kit material's whole mip chain in a level-major
    textures.rgba (every layer at 256, then every layer at 128, ...).
    Replacing only level 0 would leave distant surfaces on the shared map's
    texture."""
    for material in materials:
        layer = kit.MATERIALS.index(material)
        img = texture(material, seed+layer, kit.noise, kit.value_noise)
        side, offset = 256, 0
        while True:
            size = side*side*4
            textures[offset+layer*size:offset+(layer+1)*size] = img
            offset += count*size
            if side == 1: break
            img = kit.downsample(img, side); side //= 2
    return textures


def bake_lightmaps(vertices, textures, count, lamps, kit):
    """Bake lightmap pages for the render vertices and append them as new
    texture layers, keeping the level-major mip layout.
    Returns (vertex bytes, textures, new texture count, manifest metadata)."""
    import numpy as np
    from assets import lightmap_bake
    baked, pages = lightmap_bake.bake(np.frombuffer(vertices, '<f4'), lamps, MAP_SUN,
                                      kit.MATERIALS.index('light'), count)
    mips = [[page] for page in pages]
    for chain in mips:
        side = 256
        while side > 1: chain.append(kit.downsample(chain[-1], side)); side //= 2
    merged, side, offset = bytearray(), 256, 0
    for level in range(9):
        size = side*side*4
        merged += textures[offset:offset+count*size]
        for chain in mips: merged += chain[level]
        offset += count*size; side //= 2
    meta = dict(pages=len(pages), first_layer=count, texel_m=.5, sun=list(MAP_SUN),
                lamps=len(lamps),
                baker_sha256=hashlib.sha256(Path(lightmap_bake.__file__).read_bytes()).hexdigest())
    return baked.astype('<f4').tobytes(), merged, count+len(pages), meta


def source_hash(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_pack(output, files, manifest):
    """Record every payload digest in the manifest, then write the pack."""
    manifest['files'] = {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}
    output.mkdir(parents=True)
    for name, data in files.items(): (output/name).write_bytes(data)
    (output/'map.json').write_text(json.dumps(manifest, indent=2)+'\n')
