# Snowblind / Desert of Death private collection — 2026-09-21

Both maps are installed in the **normal local native game**, not separate frozen
reference apps. Select **Snowblind Clone** or **Desert of Death Clone**, then
Start match. The menu wraps onto two rows at the tested 1280×800 window size.
Existing approved map packs and movement physics were not changed.

These are the installed T2 Classic missions `Snowblind_nef` and
`DesertofDeath_nef`, not claims of binary-identical Tribes 1 maps. Geometry,
textures and imported objects remain Tribes-derived private study assets.
The donut artwork is original. There is no public deployment/download update,
server admission, redistribution or independently authored-art claim.

## Installed results

| | Snowblind | Desert of Death |
|---|---:|---:|
| Building instances | 6 | 20 |
| Scenery instances | 142 | 354 |
| Imported equipment models | 22 | 9 |
| Gameplay equipment definitions | 22 | 8 |
| Render triangles | 23,828 | 47,727 |
| Collision triangles | 4,724 | 7,102 |
| Terrain cells clipped to interior BSP | 86 | 223 |

Pack roots are `local-assets/snowblind-clone/installed` and
`local-assets/desert-of-death-clone/installed`. Prior iterations were preserved
in sibling directories, never overwritten. Final report-only rebuilds were
verified to have identical runtime hashes to the visually tested packs.

Snowblind's donut is on the first bunker exterior wall. Desert of Death's is
inside the first flag alcove, raised above the flag cloth after visual QA.
Each is a 1.5 m square, offset 0.025 m from its supporting wall. Exact local
anchors and camera positions are recorded in each `validation.json` and
`editable/poster.json`. These are tested decorative edits, not approximate
replacement buildings.

## Pipeline / rebuild

`tools/broadside_clone/stonehenge.py` retains its original filename for existing
commands but now selects an explicit mission profile. It uses only the fresh
mission/TER/DIF/DTS readers, not the previous converter or procedural map tools.
The default remains Stonehenge. The runtime pack ABI and renderer are shared.

```sh
local-assets/tools/venv/bin/python tools/broadside_clone/stonehenge.py --profile snowblind local-assets/tribes-map-catalog/tribes2 local-assets/snowblind-clone/new-build
local-assets/tools/venv/bin/python tools/broadside_clone/stonehenge.py --profile desert-of-death local-assets/tribes-map-catalog/tribes2 local-assets/desert-of-death-clone/new-build
local-assets/tools/venv/bin/python tools/broadside_clone/stonehenge.py --editable local-assets/snowblind-clone/installed/editable local-assets/snowblind-clone/new-edit
```

Destinations must not already exist. Rebuilds infer profile from the editable
export. Review a new pack before moving it into `installed`; preserve the old
directory. Normal client loading is local and optional, not a download mechanism.
The public binaries do not embed these packs.

Three active source terrain layers become three weighted channels plus one
zero-weight padding channel. Sorted DTS leaf clusters can contain uninitialized
plane floats; the reader now preserves the exact eight integer words rather
than attempting to serialize NaN into JSON. Rendering still uses the static
mesh primitives, not the source dynamic sorting tree.

Equipment uses explicit `always-on` circuits where the mission has no generator.
Destroyed equipment stays unpowered. Existing generator-controlled circuits
retain their previous behavior; a missing generator is still invalid for those.

## Validation actually performed

- Nineteen Python tests pass, including the source-wall poster fit check.
- All workspace library tests pass; all-targets check and application dependency
  boundary checks pass. Release client rebuilt.
- Private core checks verify both teams' spawns have building support within
  0.25 m of the expected 1.2 m offset, head clearance and at least one clear
  two-meter exit direction. Donut rays reach solid backing with an open approach.
- GPU captures for each map cover donut, both exterior bases and spawn view.
  Images were inspected, not merely generated. Desert donut was moved upward
  because its first position was partially obscured by the flag.
- Actual native menu clicks selected each map and started a running match.
  Private captures: `qa-menu.png` (Snowblind folder) and `qa-native-match.png`
  in each map folder. Isolated preference directories avoided user settings.
- The GPU test also executes flag pickup/capture using each map's imported
  homes, checks the score, switches away/back and verifies reset/home positions.
- Each installed pack independently rebuilds all six runtime payloads
  byte-for-byte using only its editable JSON/PNG, without native assets.
  Reducing poster size to 80% changes exactly six render corner positions;
  collision, terrain, textures and audio remain unchanged.
- Dedicated-server rotation rejects both private map IDs, tested alongside
  existing clone rejection. This work does not enable private multiplayer.

Repeat checks:

```sh
cargo test --locked -p peakrunner-core collection_private_spawns_and_donuts -- --ignored --nocapture
QA_COLLECTION=snowblind-clone cargo test --locked -p peakrunner render_grass_captures -- --ignored --nocapture
QA_COLLECTION=desert-of-death-clone cargo test --locked -p peakrunner render_grass_captures -- --ignored --nocapture
local-assets/tools/venv/bin/python tools/broadside_clone/verify_stonehenge.py local-assets/snowblind-clone/installed
local-assets/tools/venv/bin/python tools/broadside_clone/verify_stonehenge.py local-assets/desert-of-death-clone/installed
```

The two native launch checks use `examples/launch_smoke.rs` with
`QA_COLLECTION_PLAY` set to the map key and `QA_CAPTURE_PATH` to a private PNG.
Its click coordinates are specific to the verified 1280×800 layout.

## Explicit differences / omissions

Normal PeakRunner CTF and weapons are integrated; the original game's complete
scripted rules, arsenal and inventory system are not. `validation.json` lists
unimplemented datablocks rather than implying every parsed mission object works.

- Snowblind: Honor/Strength banner objects and scripted snowfall are not imported.
- Desert: source ammo/weapon pickups, energy pack and repair kit/patch scripts
  are not implemented. One neutral RepairPack model is visual-only; it was not
  assigned to an invented team. Team-owned supported equipment is functional.
- Both retain the earlier static-pose/lighting/texture-resolution limitations
  described in `stonehenge-private-import.md`. Terrain uses decoded TER holes
  and BSP clipping; separate mission `emptySquares` overrides are not interpreted.
- No exhaustive human route/bot navigation/balance playtest, original-engine
  image similarity metric or cross-platform runtime test was completed.

Thus the local game integration is tested, but this is not a claim of exact
original-engine gameplay or production/public release readiness.
