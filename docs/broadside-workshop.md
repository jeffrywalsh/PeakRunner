# Broadside Workshop: clone first, modify second

The user's latest direction supersedes the approximation-first workflow for
Broadside development. Start with the working imported reference, preserve its
layout, then make deliberate modifications. Do not run the procedural fortress
builder to generate this variant. The original Skybreak remains separate.

## Play

```sh
open PeakRunner-Broadside-Workshop.app
```

Choose **Broadside Workshop — source baseline** and Start match. This is a
separate local app with separate preferences; the Reference app stays unchanged.
Ordinary `cargo run` still uses the original maps, not this workshop. To run the
workshop through Cargo explicitly:

```sh
PEAKRUNNER_MAP_PACK="$PWD/local-assets/broadside-workshop" \
  cargo run --locked --release -p peakrunner --bin peakrunner
```

## Build and verify

```sh
python3 scripts/test-broadside-workshop.py
python3 scripts/build-broadside-workshop.py
cargo build --locked --release -p peakrunner --bin peakrunner
sh scripts/bundle-broadside-reference.sh --workshop
codesign --verify --deep --strict PeakRunner-Broadside-Workshop.app
```

Builder and bundler refuse existing destinations. The builder accepts a new
`--output` under ignored `local-assets/`; do not remove existing revisions just
to reuse their names. The pack's `workshop_baseline` metadata records source
manifest SHA-256, six payload hashes, revision 0 and an empty change list.
Only map identification/provenance changes. All source runtime fields remain
unchanged: transforms, spawns, flags, sky, equipment data and terrain parameters.

Validated this turn:

- All six map payloads match the converted reference byte-for-byte, including
  vertices (normals/UV/lightmap coordinates), collision, texture/lightmap layers,
  terrain heights/weights and ambient data.
- Synthetic tests check metadata preservation, unchanged source manifest,
  overwrite refusal and rejection of corrupt input before creating output.
- Release client builds; workshop pack passes the runtime pack test (including
  flags and spawn clearance); bundled app passes strict recursive signature check.
- GPU capture test passed against the workshop pack and its entry interior was
  visually inspected. All six payloads inside the finished app were additionally
  compared directly against the reference and match byte-for-byte.

Exact identity is to **our converted T2 Classic Broadside reference**, not an
assertion of perfect original-game reproduction. Existing converter omissions
(forcefields, some ambient/animation/gameplay behavior, texture normalization and
terrain-transform uncertainty) remain documented in `broadside-private-reference.md`.

## Subsequent changes

### 2026-09-21 — exact approved application, not a rebuilt client

The user confirmed `PeakRunner-Broadside-Reference.app` is the correct-looking
version. `PeakRunner-Broadside-Working-Copy.app` is now a complete unchanged
`ditto` copy, including executable, launcher, map manifest, assets and signature.
Recursive `diff -rq` reports no differences and strict deep codesign verification
passes on both. The source app was not edited.

The approved executable SHA256 is
`a4b69054255fca7dfda70d570db755c9454e70a535121df5a26aec108a049a69`.
Current release executable SHA256 at this check was
`46d5b219221c34363eb8bafd2962f692abf6ca7d4d05233b99f6e1641b2ee621`.
All six map payloads match the previous workshop; manifest differences are
identity/provenance metadata. Thus map-only equivalence did not establish
equivalence of the applications. The binary difference is verified, but the
specific rendering difference and original source revision are not identified.

Do not rebuild or re-sign the unchanged copy. It retains the same bundle ID,
display name and reference preferences path deliberately. Launch by explicit
path (`open -n ./PeakRunner-Broadside-Working-Copy.app`). No markers/docking
additions are included. File identity is validated; a visual gameplay comparison
with this exact executable remains pending. This supersedes earlier playtest
directions using `cargo run` and workshop pack overrides.

### Approved-baseline entrance-marker experiment

The user rejected the procedural docking prototype: keep the imported building
and make small additions only. `scripts/build-broadside-entrance-lights.py`
creates `local-assets/broadside-entrance-lights-v1` from the approved workshop,
adding four original decorative markers, two per entrance. No gameplay lights,
collision changes or building replacement: 96 new triangles and three opaque
solid-color material layers. Known baseline gaps/floating scenery remain.

The builder verifies the entire original vertex prefix, every existing texture
pixel at all nine mip levels, and byte identity of collision, terrain, weights
and ambient data. Six GPU views render successfully; the entrance approach was
visually inspected and shows the same building with the two added markers.
New captures are preserved inside the test pack's `captures/` directory.

Load with `PEAKRUNNER_MAP_PACK="$PWD/local-assets/broadside-entrance-lights-v1"`
and the normal client; select Raindance (the override slot). No public assets,
baseline packs or bundled apps are replaced. This remains source-derived and
private; the original markers do not change the provenance of the building.

### Editable exchange revision 1

`scripts/broadside-editable.py` exports a single fortress by matching its exact
collision-instance triangles to render triangles (not by a bounding-box crop).
It produces `fortress.obj`, textured MTL/PNG previews and `exchange.json` under
ignored `local-assets/broadside-editable-v1`. The first base contains **9,145
triangles** in this converted pack. Coordinates remain engine Y-up, in meters;
do not apply an axis conversion on export back to this pipeline.

Commands (using the existing Pillow-enabled Python environment):

```sh
local-assets/tools/venv/bin/python scripts/broadside-editable.py export local-assets/broadside-workshop local-assets/broadside-editable-v1
local-assets/tools/venv/bin/python scripts/broadside-editable.py import local-assets/broadside-editable-v1 local-assets/broadside-roundtrip-v1
local-assets/tools/venv/bin/python scripts/test-broadside-editable.py
```

Outputs must be new directories. The importer reconstructs positions, normals
and primary UVs from OBJ, retains secondary/lightmap UVs and material/light
layers from the hash-locked source, and updates matching collision positions.
All other map payloads remain unchanged. The companion file **depends on the
original workshop pack**; this is not a self-contained standalone asset yet.
PNG/MTL files are textured previews, not a baked-lighting Blender material.

Validated: the real first-fortress OBJ no-edit export/import produces six
byte-identical payloads, including render vertices and collision. Synthetic
tests additionally prove a vertex edit changes both render and collision while
preserving light attributes, and reject changed face groups and overwrites.
Both baseline and roundtrip passed the GPU capture test; all five camera images
(exterior, entry, hall, flag, armory) match pixel-for-pixel. Copies of roundtrip
captures are in `local-assets/broadside-roundtrip-v1/captures/`. Visual inspection
of the hall capture found its camera faces a nearby wall: these comparisons
prove unchanged rendering, not complete interior traversal or layout coverage.

Limitations: fixed triangle/corner order and `triangle_N` face groups are
required. Do not weld, decimate, triangulate again or reassign materials.
Texture edits are not imported. Geometry edits retain old lightmaps and may
need rebaking. A Blender import/save/export round trip has **not** been
validated (Blender was absent from the standard Applications location).
This is the first exchange milestone, not a finished arbitrary-topology editor.
Only base 1 is changed by default; use `--base 1` for a separate base 2 export.


Keep the reference immutable and retain baseline hashes. Make one explicitly
recorded change at a time to a new workshop revision, using the source importer
or a deliberate mesh-edit step rather than regenerating guessed rooms. Compare
changed geometry/collision together and capture the same camera views. Preserve
original layout unless a specific gameplay change is intended. Equipment meshes
being present does not imply original station/turret mechanics are implemented.

This pack and app contain source-derived assets; they are private development
outputs, ignored by Git and excluded from public releases. Renaming/retexturing
or calling it an homage does not change that provenance. Public distribution
remains a separate decision requiring suitable assets/permissions. This work
does not deploy, commit, publish or replace the public game's original assets.
