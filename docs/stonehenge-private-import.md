# Stonehenge private import — 2026-09-21

The first additional collection map is playable in the normal client as
**Stonehenge Clone**. Select it and Start match. Run the normal game from this
checkout with `cargo run --locked --release -p peakrunner --bin peakrunner`.
No reference-app executable or map override is required. Broadside remains
separate and its approved assets were not replaced by this work.

This is a private source-derived study, not newly authored Tribes-free artwork.
Re-encoding geometry and textures does not change their origin. Outputs stay
in ignored `local-assets/`; neither public packaging nor deployment is included.
Dedicated-server startup and rotation reject the private map IDs.

## Independent pipeline and editable source

New code in `tools/broadside_clone/` reads mission text without execution,
TER v3, DIF resource44/interior0, and static DTS v19–23. It does not call the old
converter, old DIF/DTS readers, or procedural fortress code. Existing PeakRunner
rendering, gameplay and runtime pack format are reused intentionally.

The compiler keeps decoded geometry, material references, UV planes, source
transforms, static meshes, terrain and poster settings as editable JSON/PNG.
Uninterpreted DIF tail bytes remain opaque data. Original scripts are never run.
Archive lookup gives Classic_maps_v1 explicit precedence and rejects ambiguous
remaining duplicates rather than silently picking an arbitrary file.

From the project directory, using the existing private numpy/Pillow venv:

```sh
local-assets/tools/venv/bin/python tools/broadside_clone/stonehenge.py local-assets/tribes-map-catalog/tribes2 local-assets/stonehenge-clone/compiled-next
local-assets/tools/venv/bin/python tools/broadside_clone/stonehenge.py --editable local-assets/stonehenge-clone/installed/editable local-assets/stonehenge-clone/edited-next
local-assets/tools/venv/bin/python tools/broadside_clone/verify_stonehenge.py local-assets/stonehenge-clone/installed
```

Use new output directories: the compiler refuses existing destinations. The
normal client loads `local-assets/stonehenge-clone/installed`. Preserve the prior
installed directory before explicitly replacing it with a reviewed new build.
Current installed pack is the v6 rebuild from the previous editable export;
earlier outputs are intermediate diagnostics, not the current reference.

`editable/poster.json` controls the original donut poster in the first bunker's
upper gallery. Changing size is a demonstrated decorative edit, not a claim
that arbitrary topology edits automatically repair collision or baked lighting.

## What was validated

- Eighteen Python tests pass, including material alpha, malformed/truncated input, index bounds,
  strips/fans, transform conventions, hierarchy cycles and BSP clipping.
- Real TER decode/editable/re-encode reproduces all 460,306 source bytes.
- Four native DIFs decode; surface vertices lie on their recorded planes within
  0.01 m. Source strips produce the expected count-minus-two triangles.
- Native scenery and equipment readers complete their guarded binary streams.
  Seven building instances, 100 scenery instances and 18 equipment instances
  produce **88,925 render triangles and 3,603 collision triangles**.
- An inventory close-up caught opaque station metal being cut out by texture
  alpha. DTS material flags now control that decision: opaque layers get full
  alpha, while translucent layers retain it. The original alpha can represent
  environment-map strength, not holes. The prior pack is preserved as
  `installed-before-material-flags`.
- Terrain is clipped against source interior BSP boundaries in **217 cells**;
  retained exterior fragments have both rendering and collision. This fixes
  terrain protruding into lower rooms without blanket rectangular terrain holes.
- The installed pack rebuilds from editable files with all six runtime payloads
  byte-identical, without original native asset files. Reducing poster size from
  2.5 to 2.0 changes exactly six corner positions; collision, terrain, texture and
  audio payloads remain identical.
- The opt-in core test verifies both spawns have floor support and body clearance,
  and the donut gallery has the expected approach, wall, floor and ceiling hits.
- Native GPU captures cover exterior bases, bunker, flag platform and donut.
  A real normal-client menu selection starts a match, with HUD and game scene
  captured as `qa-native-match.png`. Screenshots are private local diagnostics.
- Workspace library tests passed (150 passed, four ignored); private collision
  QA is separately run, not silently counted as covered by the ignored suite.

Useful repeat checks:

```sh
local-assets/tools/venv/bin/python -m unittest discover -s tools/broadside_clone -p 'test_*.py'
cargo test --locked -p peakrunner-core stonehenge_private_pack_spawn_and_wall_checks -- --ignored --nocapture
QA_STONEHENGE=1 QA_INSTALLED_CLONE=1 cargo test --locked -p peakrunner render_grass_captures -- --ignored --nocapture
```

GPU/native window checks need access to the macOS graphics session. Following
the game skill's visual-QA guidance, checks include rendered views and an actual
menu-to-match transition, not just successful compilation.

## Known differences — not a 100% fidelity claim

- Equipment uses static default poses; animations, damage variants and animated
  IFL materials are not played. Dynamic turret barrels and equipment behavior
  remain PeakRunner's. Source navigation, scripted weather and positional ambient
  sounds are not imported.
- Scenery uses its highest visible LOD and alpha testing. Plants have no added
  collision. Sorted meshes are rendered statically, not via original sort trees.
- Collision uses decoded surface/null polygons and equipment collision meshes,
  not the unparsed native hull tail. BSP is decoded and used for terrain clipping.
- Textures are resampled to the current runtime's 256-square layer format.
  Lighting, fog and shading use PeakRunner, not the original renderer.
- No original-engine screenshot pair or numerical image similarity study was
  completed; do not claim 90%/100% visual fidelity or exhaustive collision proof.
- This finishes the first additional private playable import, not all 84 listed
  missions. Other format variants must fail explicitly until supported.

## Format research

Fresh readers were informed by upstream format implementations, not copied from
the previous project conversion pipeline:

- [OpenMBU interior IO](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/interior/interiorIO.cpp)
- [Interior geometry/BSP](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/interior/interior.cpp)
- [Shape serialization](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/ts/tsShape.cpp)
- [Mesh serialization](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/ts/tsMesh.cpp)
- [Material flags](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/ts/tsShape.h)
- [Sorted meshes](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/ts/tsSortedMesh.cpp)
- [Legacy shape records](https://github.com/MBU-Team/OpenMBU/blob/master/engine/source/ts/tsShapeOldRead.cpp)
