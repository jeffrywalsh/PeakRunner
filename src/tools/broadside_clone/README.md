# broadside-clone — fresh editable scene pipeline

Private/source-derived. This is a new codec/compiler, with no imports or calls
to old converters, original-map builders, fortress generators or exchange tools.
It uses Python, struct, JSON and Pillow. The approved Reference app's runtime
asset data is the authoritative input. This is NOT a new direct Torque DIF
decoder and does not restore data omitted by the original reference conversion.

## Source and compilation

Generated scene lives in ignored `local-assets/broadside-clone/source-v1`:

- `surfaces.jsonl`: one ordered triangle per line; three corners with named
  position, normal, UV, light UV, material and lighting fields. No welded seams,
  remeshing, coordinate rounding, decimation or inferred room geometry.
- `collision.jsonl`: separately editable ordered collision triangles.
- `world.json`: equipment/object metadata, placement, terrain parameters,
  spawns, flags, source instance ranges and reference coordinate transforms.
- `terrain.json`: integer height samples; `terrain-weights.png`: RGBA blends.
- `materials/`: each texture/lightmap layer and ALL nine original mip levels
  represented as lossless RGBA PNG files.
- `audio.json`: explicit float samples.
- `provenance.json`: immutable reference payload hashes and coordinate contract.

The compiler reads only those editable sources, never the reference binaries.
All resulting assets are reconstructed. It reports exact hash agreement against
the approved baseline. Output map identity is `broadside-clone`; runtime fields
other than name/id/file hashes remain as supplied by world.json.

```sh
local-assets/tools/venv/bin/python tools/broadside_clone/scene.py decode PeakRunner-Broadside-Reference.app/Contents/Resources/map local-assets/broadside-clone/source-v1
local-assets/tools/venv/bin/python tools/broadside_clone/scene.py encode local-assets/broadside-clone/source-v1 local-assets/broadside-clone/compiled-v1
local-assets/tools/venv/bin/python tools/broadside_clone/test_scene.py
sh tools/broadside_clone/package.sh
open -n ./PeakRunner-Broadside-Clone.app
```

Existing source/output/app paths are refused. Use new revision directories for
further experiments (package.sh currently selects compiled-v1 intentionally).
Generated files and app stay ignored/private; code and documentation contain no
source-game geometry or textures. The packaged app retains the approved renderer
and launcher, including the reference preferences path and window title. It has
a distinct bundle ID and display name; no current-source rebuild is involved.

## Verified milestone

- 140,820 render triangles and 35,676 collision triangles reconstructed.
- All 97 texture layers and mip levels round-trip exactly through PNG.
- All SIX runtime payload hashes match the approved Reference app.
- Finished app executable is byte-identical to the approved executable.
- Finished app's six payloads independently compared with `cmp`; all identical.
- Strict recursive macOS signature verification passes.
- Three synthetic tests pass, including negative-zero float preservation,
  missing-attribute rejection, full scene roundtrip, overwrite rejection, and a
  real source-vertex edit affecting the compiled binary.

These establish exact converted asset data and the same renderer, not verified
100% fidelity to the original Tribes engine. A fresh visual gameplay comparison
of this app is still pending. Existing reference conversion defects are retained.

## Editing next

### Fresh mission-file parser (collection phase)

`mission.py` is a new lexer/recursive parser, not the old converter or a regex
object extractor. It reads nested `new Class(Name) { ... };` declarations,
literal fields and indexed fields; preserves parent IDs, raw transforms, all
field values, source paths and SHA256. Comments and braces inside quoted
strings are handled separately. Depth and object counts are bounded. It never
executes mission scripts. Executable text outside explicit object-block markers
is flagged; expressions inside declarations fail rather than being guessed.

```sh
local-assets/tools/venv/bin/python tools/broadside_clone/test_mission.py
local-assets/tools/venv/bin/python tools/broadside_clone/mission.py local-assets/tribes-map-catalog local-assets/broadside-clone/missions-v1
```

Validated: four synthetic tests pass (hierarchy/literals/arrays/comments,
expression/duplicate/malformed rejection, external-script warning and recursion
limit). Collection pass: **79 parsed, five explicitly unsupported, zero maps
converted by this parser**. Unsupported: Alcatraz duplicate `selfPower`, Katabatic
and Minotaur non-UTF8 source bytes, Training4/Training5 `%initialBarrel` variables.
Do not silently drop these fields or evaluate scripts to get past a failure.

Stonehenge_nef: 163 objects; seven interior instances, 100 TSStatic scenery
instances, eight SpawnSpheres, two FLAG items, six turrets. Four unique interior
files: `dbunk_stonehenge1.dif`, `dmisc_stonehenge1.dif`,
`dmisc_stonehenge2.dif`, `dmisc_stonehenge3.dif`. 114 direct dependency references
have a unique filename candidate; only `Stonehenge_nef.nav` is unresolved.
Filename candidates are NOT proof of correct archive override precedence.
Transitive dependencies (DIF textures, DTS materials, datablock-defined equipment
models) still need discovery. Rain/lightning/audio objects are retained as data,
not claimed implemented gameplay features.

Reports and complete editable mission trees are in ignored
`local-assets/broadside-clone/missions-v1/`. Stonehenge now has a fresh native
import and a separate normal-client menu entry. See
`../../docs/stonehenge-private-import.md` for the playable checkpoint.

### Fresh native terrain codec

`terrain_file.py` independently reads and writes TER version 3. Format research:
[upstream Torque terrain reader](https://raw.githubusercontent.com/GarageGames/Torque3D/development/Engine/source/terrain/terrFile.cpp),
`TerrainFile::_loadLegacy`. No previous project decoder is imported or invoked.
Editable output consists of integer height rows in JSON, lossless grayscale
material flags and alpha PNGs, and inert base64 editor-script bytes. Never execute
those scripts. Unknown versions, truncation and trailing bytes are rejected.

```sh
local-assets/tools/venv/bin/python tools/broadside_clone/test_terrain_file.py
local-assets/tools/venv/bin/python tools/broadside_clone/terrain_file.py local-assets/tribes-map-catalog/tribes2/Classic_maps_v1/terrains/Stonehenge_nef.ter local-assets/stonehenge-clone/terrain-v1
```

The second command refuses existing output; use a new revision directory to rerun.
Validated against the real file: **all 460,306 bytes reproduced exactly** after
decode → editable files → reload → encode. SHA256:
`d2bb26c148e66f4add44a46269680d5f2408e25f62f7c526bc3542f462ac1360`.
65,536 height samples, local height range 50–350 m, four material layers:
`GMD.GrassMixed`, `LushWorld.RockLight`, `LushWorld.RockMossy`,
`LushWorld.GrassMixed`. Texture/height editor blobs are 1,339/126 bytes.
Three synthetic tests cover roundtrip (including sparse material slots and opaque
script bytes), malformed input, and a single height edit affecting only its two
source bytes. `validation.json` records the real-file result.

This terrain codec test proves lossless terrain conversion, not visual fidelity
or gameplay by itself. Separate native-reader, collision, GPU and real-client
checks now cover the Stonehenge import; see the checkpoint document above.

### Snowblind and Desert of Death

The same fresh-native pipeline now supports `--profile snowblind` and
`--profile desert-of-death`. Both are playable private menu entries with separate
packs and donuts. Editable exports include `profile.json` and a fixed local
poster anchor in `poster.json`; rebuilding no longer needs native source files.
See [collection validation and limits](../../docs/collection-snowblind-desert.md).

### Normal client / collection integration

The normal client now has a separate `broadside-clone` map menu entry when the
local `compiled-donut-v1` directory is installed. Run from the workspace root;
no PEAKRUNNER_MAP_PACK override. Raindance and Skybreak stay separate. Assets
remain external and ignored rather than embedded in distributable binaries.
The new MapId is private/offline: dedicated-server startup and rotation reject
it until private multiplayer admission is implemented. This entry uses current
client rendering; the frozen clone apps remain unchanged for comparison.

`inventory.py` independently lists mission objects/dependencies without loading
or executing mission scripts. It found 84 extracted mission files: 24 Classic,
8 TR2 and 52 under the missions collection. See the private mission-inventory
JSON. This count does not assert completeness of either installed game, or
successful import. Stonehenge_nef is now playable privately, contrasting
Broadside's floating enclosed bases. The new `interior_file.py`, `shape_file.py`
and `stonehenge.py` decode native geometry independently; the older scene codec
still only ingests the approved converted runtime format.

### Donut proof edit

`add_donut.py` creates source-donut-v1 and compiled-donut-v1, adding exactly
two decorative triangles at Base 1 local center (0,30.47,3.3), 3m square.
Wall collision triangles 12103/12104 establish forward=30.5; 3cm offset avoids
z-fighting. All old render bytes and collision remain unchanged. One new
texture layer and its mips come from `art/donut.png`. The current-renderer GPU
diagnostic `QA_REFERENCE=1 QA_DONUT=1` was visually inspected and reproduces
the user's viewpoint with the picture between the two ramps. The finished
`PeakRunner-Broadside-Clone-Donut.app` retains the APPROVED executable instead;
its signature and executable/collision comparisons pass.

Generate using built-in imagegen (imagegen skill), prompt:
"Generate a square game wall-poster texture: one large delicious pink-frosted
donut with colorful sprinkles, viewed straight from above, centered on a pale
cream background. Cheerful polished food photography, donut fills 75 percent
of frame, soft shadow. No text, no frame, no environment, no watermark. This is
the flat picture itself to mount onto an in-game wall."

Reproduction: run `add_donut.py` with the existing Pillow/numpy Python environment,
then `sh tools/broadside_clone/package.sh compiled-donut-v1 PeakRunner-Broadside-Clone-Donut.app`.
Both refuse existing destinations. Baseline clone and Reference app unchanged.

First edit only a selected surface's named fields and recompile a NEW output.
For positional changes update matching collision triangles too. Source mission
instance `first`/`count` are collision ranges, NOT render ranges. Normals and
lightmaps are not automatically recomputed. Texture base-level edits require
intentional mip updates. Triangle IDs must remain contiguous; arbitrary topology
changes require repairing instance ranges/metadata. A room/object editor,
linked render/collision edits and lightmap rebaking are future work, not features
claimed by this first exact-data milestone. Copying/encoding geometry and artwork
does not change source ownership. No public release is authorized.
