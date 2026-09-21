# Original Raindance-inspired map and creator kit

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

Choose **Raindance** and local play. The ordinary development client and match
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
repairing it; restoring a generator restores its circuit. Destroyed equipment
remains as a solid disabled chassis. State indicators show online/offline/ruined
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
- Custom packs currently occupy the Raindance map slot. Arbitrary map dimensions,
  an in-game editor, multiple custom-map slots and glTF importing are not present.

To replace the bundled original pack, compile into a fresh directory, run the
checks, then copy its seven output files to `assets/maps/raindance` and rebuild.
Never mix files from different builds. `map.json` records the definition/compiler
hashes plus every binary asset's SHA-256 checksum.

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
