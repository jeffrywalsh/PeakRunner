# Old Holler (formerly Raindance) and the original map kit

This version uses **PeakRunner-authored procedural assets**, including the
heightfield. It does not load, sample, trace, or convert Tribes installation
files. The older extracted-reference application is separate and remains private.
This is an original interpretation of the rainy highland/base-assault design,
not a pixel-identical reconstruction or a verified percentage match.

## Play

On macOS, the separate bundle is `PeakRunner-Original.app`:

```sh
open PeakRunner-Original.app
```

Choose **Old Holler** and local play. Its internal key stays `raindance`. The ordinary development client and match
server also embed this original pack. The previously built regular app and the
public server are not updated by editing this repository.

Movement is unchanged: WASD, Space to jump/ski, right mouse to jet, left mouse
to fire, 1/2/3 for disc/chaingun/grenade. At friendly equipment, hold **E** to
refit or repair. Touch mode provides a contextual Use / repair button.

## Assets and layout

The map has two multi-level bases approximately one kilometer apart, a central
ravine with a traversable bridge, two defense towers, two forward bunkers, two
landing pads, exposed flag decks, lower service halls, and interior/exterior
roof ramps. Scenery includes 115 original trees (conifer and broadleaf variants)
and 80 rock formations. Terrain materials, structural cladding, equipment
materials, six-face cloudy sky, rain and wind ambience are generated locally.

The reusable kit contains 12 asset types:

| Asset | Function |
| --- | --- |
| `base` | Basement hall, atrium, ramp routes, roof, flag deck and service spire |
| `bridge` | Deck, approach ramps, edge rails and supports |
| `tower` | Elevated turret platform and sensor mast |
| `bunker` | Covered forward position |
| `landing_pad` | Original landing-pad model; no vehicle spawning yet |
| `inventory` | Powered, team-gated health and energy refit while holding E |
| `generator` | Destructible, repairable power source for its named circuit |
| `sensor` | Detects visible enemies; powered team sensors extend turret range |
| `turret` | Server-selected targets and rotating barrels; base plasma cannons and tower bullet guns |
| `repair` | Powered, team-gated passive repair/refuel pad |
| `tree` | Seeded trunk and canopy geometry; solid trunk, nonsolid foliage |
| `rock` | Seeded solid rock formation |

Base turrets use `"weapon": "plasma"`: 0.45 m-radius plasma balls at 80 m/s,
one shot every 1.2 seconds. A direct hit deals 55 health damage (two hits kill
a full-health player). Splash tapers to zero over 6 m, respects solid cover,
and adds at most 3.5 m/s of knockback. Friendly players are protected. Tower
turrets keep their nonexplosive bullets; omitted `weapon` defaults to `bullet`.

Player collision covers the full eye height plus a head margin. Camera shake,
bob and network smoothing are swept against map geometry independently so
visual offsets cannot expose the other side of a ceiling or wall.

Inventory currently services PeakRunner's existing three-weapon kit. It is not
the Tribes armor/loadout purchase interface. Stations and defensive equipment
can take enemy damage. Hold E near damaged friendly equipment to spend energy
repairing it; a destroyed object comes back online only past half its hull,
so a generator restores its circuit at 50%. Destroyed equipment remains as a
solid disabled chassis; a wrecked generator smokes and sparks. Press Q to use
your repair kit (one per life, refilled at inventory stations; see
`docs/weapon-damage.md`). State indicators show online/offline/ruined
status. Server snapshots replicate health, power and turret aim. Empty servers
and round restarts restore equipment.

## Create another map

`maps/raindance.json` is the editable map definition;
`scripts/build-original-map.py` is the reusable procedural asset compiler.
It requires only Python's standard library—no Blender, game installation,
downloaded models, NumPy or image packages.

```sh
python3 scripts/build-original-map.py --definition maps/my-map.json --output build/my-map
PEAKRUNNER_MAP_PACK="$PWD/build/my-map" cargo run --release
```

The compiler refuses to overwrite an existing directory. Use a new output
directory for each revision. Client and server must load the identical compiled
pack; the handshake rejects differences. Packs are not downloaded automatically.
The built-in original map is used when no override is selected. Bundled resource
packs take precedence over the embedded default; the explicit environment
override takes precedence over both.

Authoring rules for this first format:

- Coordinates are meters, Y-up; terrain is 256 samples at 8 m spacing (2,040 m).
- Define exactly two bases with teams 0 and 1, unique IDs, and coordinates on
  the 8 m X/Z grid. Basement cutouts currently support only 0°/180° base rotation.
- `base_equipment` contains local positions and behavior kinds shared by both
  base prefabs. The base ID becomes its power-circuit name.
- `objects` places any non-base kit asset with position, optional yaw and team.
  Equipment may name a `circuit`; it must reference a same-team generator.
  `grounded: true` puts a structure on the generated ground. Bunker/tower sites
  receive a smooth terrain foundation automatically.
- `terrain`, `seed`, `scenery` and `environment` control authored terrain rules,
  deterministic variation, foliage budgets, visibility and water height.
- Mesh recipes and material functions are ordinary Python functions; behaviors
  are Rust code in `crates/core/src/equipment.rs` and the authoritative simulation.
- Custom packs currently occupy the `raindance` (Old Holler) map slot. Arbitrary map dimensions,
  an in-game editor, multiple custom-map slots and glTF importing are not present.

The shipped Old Holler pack is no longer the kit's own output: it is built by
`scripts/build-raindance.py` (below), which uses this kit's terrain, scenery
placement and field assets but the cleaned base. The kit build is kept as the
reference for custom definitions and for `test-original-map.py`. Never mix files
from different builds. `map.json` records the definition and source hashes plus
every binary asset's SHA-256 checksum.

## Raindance cleanup (September 23, 2026)

Raindance was re-checked with the map pipeline (`docs/map-pipeline.md`) and
rebuilt as `raindance-base-v2` by `scripts/build-raindance.py`, with
`scripts/assets/raindance_base.py`, `raindance_structures.py` and
`raindance_materials.py`. The layout, terrain, holes, trees and rocks,
flags and equipment are unchanged; `test-raindance.py` proves the heightfield,
weights, ambience, holes and scenery are byte-identical to the kit build.
The audit (private notes and before/after captures under ignored `research/`)
found and fixed:

- **Z-fighting:** hall walls, the roof and the bunker walls overlapped at their
  outer faces (385 visible coplanar pairs, about 1,280 m²). Walls now meet at
  the corners, the roof sits on the walls with eaves, and bunker walls stand on
  their floor. No two structure boxes share a same-facing face any more.
- **Dead-end ramps:** both hall ramps climbed into the solid roof and stopped
  about 6 m short. They now rise through roof openings and end flush with the
  roof; the roof sensor moved 5 m sideways, out of the opening. The outer
  shoulder ramps end flush with the roof edge instead of a 0.3 m lip.
- **Walk-under pockets:** the low ends of all four ramps are sealed where the
  underside is lower than 2.4 m (about 1,200 sampled points before, none now).
- **Hollow spire:** the service spire had no base and hung over a roof gap, so
  a player could jet up inside it. It is now capped.
- **Spawns:** 8 authored spawn points per team (hall, corridors, roof), 1.2 m
  above the floor; the server picks among them.
- **Look:** Raindance's own rain-streaked concrete, slate, grating, steel and
  team enamel; dressed and lit hall walls in place of floating wall lights;
  baked lighting. Zero-area canopy triangles are dropped before the bake.

Turrets could not see into the halls before and still cannot (Python and Rust
tests). Every other map used to start from Raindance's committed textures and
ambience; they now take the identical bytes from `kit.base_pack()`, so changing
Raindance cannot change another map. All four other packs rebuilt byte-identical.

```sh
../research/local-assets/tools/venv/bin/python scripts/build-raindance.py [OUTPUT] [--no-bake]
../research/local-assets/tools/venv/bin/python scripts/test-raindance.py
```

## Old Holler: rename and generator basements (September 23, 2026)

Raindance is a Tribes map name, so the map is now shown as **Old Holler**. Only
player-facing text changed: the `MapInfo` name, the manifest `name` and the docs.
The key `raindance`, `MapId::Raindance`, the `build-raindance.py` and
`raindance_*.py` files, rotation configs and the default/reset map all stay.
Records of the published `.20260921.1` release still say Raindance, because
that is what shipped.

Each base (asset `raindance-base-v3`) moves its generator from the hall floor
to a basement under it. Everything underground sits inside the hall's existing
terrain cut, so the holes, heightfield, weights and scenery are still
byte-identical to the kit build.

- **Basement:** 22 × 30 m, floor 8 m below the hall floor, 6.8 m of headroom
  over the 5.8 m generator, concrete liners and lit ceiling strips.
- **Two ways in, exactly:**
  - a 4 m stair (29°) down from the open atrium floor, railed at the top and
    sealed underneath;
  - a door in the basement's back wall, a short passage and landing, then a
    3.6 m service stair (30°) that climbs 18 m along the rear strip under a
    sloped, lit ceiling to a shed against the hall's back wall. The shed door
    faces away from the hall, so no turret can see down it.
- **Anti-turtling:** no spawns in the basement or the passage. The nearest hall
  spawns are 15.6 m (straight line) from the atrium stair head.
- **Routes** (`route_checks.py`, walking): hall 3 → 4 (the basement stair is a
  new way up), flag roof 2, generator 1 → 2.

Tests: `test-raindance.py` checks the basement floor, both stairs, the sealed
stair underside, that nothing underground opens onto the empty cut, that the
generators sit in cut cells, that no turret can see the basement, passage or
service stair, and the route counts. The Rust tests
`raindance_generators_are_in_basements_in_cut_cells` and the extended
`raindance_turrets_cannot_see_into_the_halls` cover the same on the embedded pack.

Human traversal and balance of the new ramp exits are not playtested.

## Verification and boundaries

```sh
python3 scripts/test-original-map.py
cargo test --offline --workspace --lib
cargo test --offline -p peakrunner-server eight_clients_sustain_movement_and_all_weapons -- --ignored --nocapture
node scripts/check-app-boundaries.mjs
cargo build --offline --release --workspace --bins
sh scripts/bundle-raindance.sh
```

Checks cover original asset provenance, generated height samples, material
indices, file checksums, terrain/mesh agreement, fast projectiles, base traversal,
refit permissions, generator damage/repair, turret power gating, snapshots and
empty-server resets. GPU capture tests inspect exterior/interior/bridge views.
Existing control tests retain the approved movement tuning and A/D directions.

Water is currently a rendered surface, not a swimming simulation. Landing pads
do not spawn vehicles. Bot navigation is still the existing simple steering,
not a navigation mesh. No original Tribes animations, characters, audio recordings,
lightmaps, textures, terrain samples or meshes are dependencies of this kit.
The regular public server remains unchanged until an explicit deployment.

Verified September 19, 2026: 86 ordinary workspace tests and five asset-compiler
checks passed. The final eight-client local load run completed 723 ticks in
12 seconds (largest JSON snapshot 26,368 bytes). GPU exterior, interior and
bridge captures were rendered and visually inspected at desktop/portrait sizes.
Release binaries, native all-target checking, WebAssembly compilation and app
dependency boundaries passed. The packaged app's binary and map files were
compared against those tested build artifacts. This is not an internet latency,
long-duration soak, or low-end hardware performance certification.
The packaged macOS app was also launched and its actual menu window visually
checked. Gameplay interaction/traversal was tested through the shared simulator;
the GPU captures exercise the real gameplay renderer.

The code and generated original kit follow this project's MIT license declaration.
That does **not** extend to the private extracted Tribes reference files or old
reference bundle, which must not be included in releases.
