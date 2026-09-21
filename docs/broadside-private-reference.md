# Private Broadside reference build

At the user's request, the installed Tribes 2 Classic `Broadside_nef.mis`
can now be imported directly for local comparison. This is separate from the
original Skybreak assets and is not a public release or redistributable pack.

## Reproduce

Use the existing local source installation and parser dependencies documented
in `raindance-import.md`:

```sh
local-assets/tools/venv/bin/python scripts/convert-raindance.py \
  --mission Broadside_nef.mis --output local-assets/broadside-reference
cargo build --locked --release -p peakrunner --bin peakrunner
sh scripts/bundle-broadside-reference.sh
open PeakRunner-Broadside-Reference.app
```

Both import and bundling refuse to overwrite existing outputs. The app contains
its own private pack, uses separate preferences, and does not automatically
join a public server. Select the private Broadside map and start a local match.
Alternatively set `PEAKRUNNER_MAP_PACK` to the absolute pack path when running
the normal binary. This privately overrides the Raindance map slot; it does not
add a public map ID. Content fingerprints remain enforced for network joins.

## What is imported

134 placed objects, 140,820 render triangles, 35,676 collision triangles and
97 texture layers: source interior geometry, transforms, texture coordinates,
interior baked lighting, terrain heights/material weights, and static scenery
and equipment meshes. These source assets remain ignored under `local-assets/`
and inside the ignored reference app. Never upload either to public downloads
or commit them to Git.

The non-interactive measurement overlay shows source XYZ coordinates, nearest
base-local XY, player-center height above its deck, and distance along a
player-center look-direction ray to structural geometry (500 m maximum).
Source Z is vertical; engine Y is vertical. Heights are not feet clearance.

## Fidelity limits

- This is the installed T2 Classic Broadside variant, not proven identical to T1.
- Source geometry is used directly, but rendering remains PeakRunner's.
- Textures are normalized to 256 pixels; sky, fog and lighting differ.
- Terrain uses raw heights and the mission's declared 9 m square size. The
  declared TerrainBlock Z translation is not applied: doing so buries the bases.
  Source-engine terrain transform behavior still needs verification.
- Eight source forcefields are not imported. Equipment is static reference
  geometry, not original station/turret logic; animations are not reproduced.
- Multiple original ambient streams are silent rather than incorrectly merged.
- Characters, weapons, HUD, physics and match rules remain PeakRunner's.

Do not describe this as measured 100% fidelity. Use it to inspect and measure
the real building before improving the independently authored reusable asset.

## Live interior survey

The overlay's structure distance is `MapPack::sweep` with radius 0 from the
player center, capped at 500 m. `PEAKRUNNER_INTERIOR_SURVEY` runs that same ray
on a 1 m grid of standing positions and exits before opening a window. It is
the game binary loading a map pack, not a separate triangle reader.

Reference, using this app's own map files (do not point it at Raindance):

```sh
PEAKRUNNER_MAP_PACK="$PWD/PeakRunner-Broadside-Reference.app/Contents/Resources/map" \
PEAKRUNNER_CONFIG_DIR="$PWD/local-assets/survey-config-reference" \
PEAKRUNNER_SURVEY_MAP=reference \
PEAKRUNNER_INTERIOR_SURVEY="$PWD/local-assets/interior-survey-reference.json" \
cargo run --bin peakrunner
```

Skybreak uses the same binary and its embedded pack. Leave `PEAKRUNNER_MAP_PACK`
unset so Raindance is not replaced:

```sh
PEAKRUNNER_CONFIG_DIR="$PWD/local-assets/survey-config-skybreak" \
PEAKRUNNER_SURVEY_MAP=skybreak \
PEAKRUNNER_INTERIOR_SURVEY="$PWD/local-assets/interior-survey-skybreak.json" \
cargo run --bin peakrunner
```

`scripts/compare-interior-survey.py` draws original difference plans from the
two JSON files. Those plans are measurements, not source renders. The JSON
stays under ignored `local-assets/`. A missing flag near local (0, -12) aborts
the survey instead of writing a shifted grid.

## Verification

Workspace library tests, converter tests, private-pack validation and GPU
reference captures passed. Capture with `QA_REFERENCE=1` and
`PEAKRUNNER_MAP_PACK` using the ignored `render_grass_captures` test.
Screenshots use ignored `screenshots/broadside-reference-*.png` paths.
The reference app is locally ad-hoc signed, not notarized for distribution.
The native `launch_smoke` screenshot callback did not complete in debug or
release and those QA processes were stopped. Sampling showed the release
client actively rendering; this is not a passed native-HUD visual check.
Interior and exterior GPU captures were inspected, but interactive traversal
and the measurement overlay still need a full-window visual check.
