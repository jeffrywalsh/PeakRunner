# Raindance conversion — local playtest

**Historical private-reference workflow.** The current default is the original
PeakRunner map described in [original-map-kit.md](original-map-kit.md).
`scripts/bundle-raindance.sh` now builds `PeakRunner-Original.app` from original
assets; the already-built `PeakRunner-Raindance.app` remains the private imported
reference. Instructions below describe that earlier conversion, not the release.

This is the **Tribes 2 Classic `Raindance_nef` mission**, not a claim of a
pixel-perfect Tribes 1 port. Original game assets remain ignored local files;
PeakRunner's code license does not grant redistribution rights to those assets.
The ordinary app and public servers remain unchanged unless explicitly opted in.

## Implemented

- Original heightfield, checkerboard terrain triangles, four painted material
  layers, and 58 terrain holes (including underground entrances).
- 178 placed mesh instances: bases, bridge, towers, scenery, equipment and
  mounted turret barrels. Source transforms, UVs and interior lightmaps retained.
- 145,907 imported render triangles and 20,731 solid collision triangles.
  Terrain and water rendering are additional geometry.
- Original six-face sky, fog distances, water placement, rain, and frog ambience.
- Original flag coordinates; clear spawn positions selected within the original
  spawn spheres. Shared client/server structure collision and projectile sweeps.
- SHA-256 checked pack files and a map fingerprint in the gameplay handshake.
  Mismatched clients are rejected before receiving a player slot.

Approved movement constants and input mappings are retained. This is a fidelity
foundation, **not yet a verified 90–100% recreation**.

## Remaining fidelity work

Equipment is static: turret AI, power, inventories, vehicle spawning, repair
pickups and DTS animation are not converted. Original AI navigation is absent;
bots can struggle with buildings. Water has no original swimming/drag behavior.
Terrain remains one 2,040 m playable tile rather than the original repeating
2,048 m terrain; original mission-area restrictions are recorded but not applied.
Animated cloud layers, native weather density, exact lighting and material
effects, LOD selection and visibility culling still need work. Textures are
normalized to 256 pixels with mipmaps. Ambient emitters share one attenuated
stream rather than four independently spatialized streams. Collision uses
imported solid render triangles and a swept three-sphere player approximation,
not Torque's original collision hulls.

## Reproduce from a user-owned installation

The converter reads extracted `.vl2` contents under
`local-assets/tribes-map-catalog/tribes2`, including `Classic_maps_v1`.
It parses the mission's declarative object fields; it does not execute TorqueScript.

Local parser dependencies (not vendored into this repository):

- [io_dif](https://github.com/RandomityGuy/io_dif), commit
  `b64ea34ebd0ef0638df00532a85fbe943eab23c8`, at `local-assets/tools/io_dif`.
- [io_scene_dts](https://github.com/qoh/io_scene_dts), commit
  `e4e2a7e3a33bec7be4ed09b6c92cf93dde95e1c0`, at `local-assets/tools/io_scene_dts`.
  In `DtsShape.py`, the material reflectance read must use
  `unpack("f", fd.read(4))[0] if stream.dtsVersion > 20 else 1.0`;
  old DTS files do not contain that trailing float.
- Python environment with NumPy and Pillow (tested with 2.5.3 and 12.3.0).

Format interpretation was cross-checked against the
[Torque3D engine source](https://github.com/GarageGames/Torque3D/tree/development/Engine/source),
particularly terrain, TS mesh, material, shape and quaternion readers.

```sh
local-assets/tools/venv/bin/python scripts/convert-raindance.py --output local-assets/raindance-playtest
cargo build --offline --release --workspace --bins
sh scripts/bundle-raindance.sh
```

Both conversion and bundling refuse existing output destinations. The resulting
`PeakRunner-Raindance.app` is a separate private playtest application. Select
Raindance for local play. To run a dedicated test match, both native client and
server need `PEAKRUNNER_MAP_PACK` set to an **absolute** directory containing the
same pack. It intentionally cannot join the stock public server. Never upload
the app or pack as ordinary open-source release artifacts.

## Verification

Core tests with the imported pack cover terrain holes, platform heights, clear
spawns, continuous collision and existing movement/combat regressions. GPU
capture tests exercise desktop/portrait and all three weapons. The directory
remains independent of the gameplay core. Automated checks do not establish
visual percentage fidelity or replace a traversability playtest of every room.

September 19, 2026 checks: release workspace binaries built; 79 ordinary
workspace unit tests passed; all 56 core tests and 13 server tests passed with
the imported pack enabled. The separate eight-client, 12-second local load test
completed 723 ticks with all weapons active. Five converter regressions passed.
Native GPU captures of the exterior and underground base rendered successfully
and were visually inspected at desktop and portrait sizes. WebAssembly compile
and application dependency-boundary checks passed (the local asset pack itself
is native-only). No public server deployment was performed.
