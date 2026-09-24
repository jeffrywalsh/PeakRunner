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
landing pads, bishop flag towers, lower service halls, and interior/exterior
roof ramps. Scenery includes 115 original trees (conifer and broadleaf variants)
and 80 rock formations. Terrain materials, structural cladding, equipment
materials, six-face cloudy sky, rain and wind ambience are generated locally.

The reusable kit contains 12 asset types:

| Asset | Function |
| --- | --- |
| `base` | Basement hall, atrium, ramp routes, roof and the bishop flag tower |
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

## Old Holler: bishop flag towers (September 23, 2026)

Each base (asset `raindance-base-v4`) replaces its solid service spire and the
exposed roof flag stand with a flag tower shaped like a chess bishop, behind
the front roof deck at local (0, 20). It is lathed from 20-sided rings, and the
flag moves into it.

| | Old spire | Bishop tower |
| --- | --- | --- |
| Height above the roof | 18 m (antenna to 23 m) | 23.7 m (finial to 25.2 m) |
| Widest | 11.6 m (base) | 15.2 m plinth, 14.8 m collar ledge, 13 m mitre |

- **Shape:** ringed plinth, flared foot, stem, a wide collar, hollow upper
  stem, neck ring, bulbous mitre and ball finial. The lower half is solid and
  sealed underneath.
- **Chamber:** its floor is 9 m above the roof (17.6 m above the base origin),
  10.4 m across inside, with the flag on the axis and 14 m of open space up
  into the mitre. The collar's top is also a 1.6 m ledge round the outside.
- **Three ways in** (players jet up; nothing walks up to the chamber):
  - a **front door** (-Z) and a **side door** (+X), each 1.6 m wide and 3.4 m
    tall at chamber-floor level, 90° apart so there is no straight shot through;
  - the **mitre slit**, a 3.6 m band cut diagonally (40°) through both shells
    of the mitre's back-left face, about 16–20 m above the roof. A standing
    body fits through it in a lane about 0.5 m wide and 1.5 m tall; from inside,
    players drop to the flag.
- **L baffle:** two 4.2 m walls 3 m from the axis, one across each door, joined
  at the corner. Each door opens into a vestibule that leads round the far end
  of its wall. No turret can see the chamber floor past the baffle; the two
  vestibules are the stated tolerance, as on the other maps.
- **Spawns** are unchanged and all stay on or below the roof, well away from
  the tower's entries.
- **Collision:** 2,992 triangles per base (was 1,302; budget 4,500). The
  baked pack has 25 lightmap pages; chamber light strips and a lit neck ring
  are baked.

Routes (`route_checks.py`): hall 4 and generator 2 (walking, unchanged). The
chamber is counted with a round region in the jet model, plus an opt-in
over-the-top hop (`overhead=`) for the slit: two door entries and the slit
(one or two clusters: the over-the-top hop and drops from the slit's lip).
The old roof flag deck had 2 walking routes.

Movement: `old_holler_flag_tower_entries_and_exits_are_flyable` (sim.rs) flies
a player, using only facing, W and jet, from the roof in through each door and
through the slit to stand on the chamber floor, and back out each way, for
both teams. `old_holler_flags_sit_in_the_bishop_tower_chambers` (terrain.rs)
checks each flag's deck and headroom, and `raindance_turrets_cannot_see_into_the_halls`
now samples the chamber. `test-raindance.py` adds chamber-floor support, the
doors, the slit's body lane, the bishop silhouette and the round-region route
counts.

Not playtested: whether the jet-only chamber is too hard to defend or to cap
from, and whether the slit is readable from the field. Bots head for the flag
position but cannot fly these routes.

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

### Open chamber doors (2026-09-24)

Supersedes the L baffle above: the bishop tower's chamber doors are open
straight through to the flag. Turrets face the field with a 200 degree field
of fire beyond 15 m (`turret_arcs.py`), and none of them engages the chamber
floor.

## Old Holler rework (2026-09-24)

Asset `raindance-base-v5`, `raindance-structures-v3`.

- **Four-door flag cabin.** The bishop chamber now has four doors, one per
  side (front -Z, east +X, back +Z, west -X), each 3.0 m wide at the outer
  face (2.7 m inside) and 4.5 m tall, plus the mitre slit. The tower is lathed
  with 24 sides so each door is two whole panels centred on its axis, and
  opposite doors line up through the flag. `old_holler_flag_chamber_fly_through_grabs_the_flag_at_speed`
  (sim.rs) flies an enemy straight through at 20, 30 and 40 m/s on both
  towers and both axes, jetting only to hold height: it grabs the flag and
  never drops below 90% of its entry speed. Walk-and-jet routes through every
  door and the slit are checked by `old_holler_flag_tower_entries_and_exits_are_flyable`.
- **Flag access** (`route_checks.py`, jet mode): 5 ways into each chamber (4
  doors and the slit, was 3); hall 4 walking entries; generator exactly 2.
- **Basement** floor lowered 1.2 m to 19.2 m below the base deck: 8.0 m clear,
  flyable. The atrium stair is 16.6 m long (29 degrees); the generator stays at
  exactly two entrances.
- **Field bunkers**: 7.0 m ceiling (was 5.1 m), a second 6 m wide, 5.5 m tall
  opening through the back wall with a ramp down to the ground, and a
  foundation reaching 2.8 m below the floor top. Landing pads stand on a 4 m
  concrete plinth, so no edge hovers over sloping ground (the audit counted
  180 hovering pad edges).
- **Capture & Hold**: three `cnh_tower` points. West Knoll (906, 780) and East
  Knoll (1054, 1100) mirror each other through the map centre (980, 940) on
  flat, tree-free ground, 393 m from their nearer base. The centre lies over
  the flooded ravine, so the Crossing tower stands on a 13 m platform on a pier
  from the ravine floor, joined to the west side of the bridge at mid-span
  with its deck level with the bridge deck (494 m from both flags). None is
  CTF-active. Capture & Hold is playable on Old Holler.
- **Look**: overcast procedural sky (heavy cloud, faint sun disc), cooler
  sun colour at the baked direction, sky and ground ambient, valley mist
  (height fog base 70 m) and a matching fog colour.
- **Materials**: Old Holler's own meadow and moss (isotropic, no directional
  streaks; the kit meadow banded), and a smooth lit diffuser for light strips
  (the old one read as a checkerboard).
- **Z-fighting**: the render mesh has no same-facing coplanar overlaps of
  different materials outside kit equipment models (tested). The audit's 28
  pairs were hidden equipment underside faces.

Collision: 3,316 triangles per base (budget 4,500); 12,983 for the map.
Not playtested: whether four doors make the flag too easy to take, and the
Crossing's reach from the bridge (players hop the 1 m bridge rail).

## Old Holler flag routes (2026-09-24)

Asset `raindance-base-v6`. The pipeline asks for "two main entrances but ~10
ways of getting to the flag"; the bishop chamber alone had 5 (four doors and
the slit), all jet-only.

- **Walkable tower ramps.** Each roof half gets a 3.6 m ramp that climbs 9 m
  over 16 m (29 degrees, inside the 35 degree walk limit and the bots' 0.62
  rise-per-metre link limit) from behind its stairwell opening to a landing
  beside the east or west door. The landing's inner edge follows the ledge's
  24-sided rim vertex for vertex, so the two meet with no gap and no overlap.
  Where a ramp's underside is lower than 2.4 m over the roof, a solid closure
  stops players walking under it.
- **Wider ledge.** The ledge round the collar is now a 2.4 m walkway (rim
  radius 8.2 m, was 7.4). The 1.6 m ring could not be walked round: its
  straight 3 m nav segments cut outside the edge and bots fell off. A 3.0 m
  ledge overhung the deck so far that the jet hops up onto its front failed;
  2.4 m keeps both. From either landing a player reaches all four doors on foot.
- **Routes** (`route_checks.flag_routes`, airborne, over the chamber, ledge
  and landings from chamber-floor height up): 11 (ember) and 12 (glacier)
  per flag, was 7 in the same volume; the chamber on its own still has its 5 entries. `test-raindance.py`
  requires 10.
- **Tests.** `test_tower_ramps_walk_up_to_the_ledge` (slope, headroom, landing
  floor to the door, sealed underside). In sim.rs the east door route now walks
  up the ramp and in; the west and back routes climb outside the ledge rim
  first. The fly-through grab at 20, 30 and 40 m/s is unchanged and passes.
- **Bots.** A lone bot reaches the enemy flag in 80 s (ember) and 91 s
  (glacier), was 90 s and 165 s. The new ramps made the east roof a cheaper
  route, which exposed bots pinning themselves under the hall's 0.6 m eave on
  jet links; bots now back away from a wall when the climb straight up is
  blocked (sim.rs, jet steering).

Collision: 3,384 triangles per base (budget 4,500). Not playtested: whether a
walking route makes the flag too easy to reach.

## Props and terrain shade

Old Holler scatters original props from `scripts/assets/props.py` (theme `old-holler`): meadow boulders and fallen logs (solid), moss tufts and ferns (render-only). The committed pack has 48 boulder, 16 log, 4200 tuft, 900 fern. That adds 29,560 render triangles and 1,920 solid ones (prop budget 2,500). Big props come in mirrored pairs, stay at least 80 m from each flag and 28 m off the ski lanes, and sink into the ground. The baked terrain shade map `shade.rg` (`scripts/assets/terrain_shade.py`) shadows the ground under structures, hulls and big props for the map's sun, plus terrain self-shadow and occlusion. About 0.9% of the tile is in shadow. See `docs/map-pipeline.md`.

**Cover and ground layer.** 22 mirrored cover pieces (8 log pile, 6 stone wall, 8 rock cluster; 14 crouch-height, 8 full-height) stand 36–62 m beside the ski lanes and around the control points, blocking movement and shots. The grass layer adds 8003 clumps in 95 meadow patches (80,007 render-only triangles). Scenery: 48 boulder, 16 log, 1300 fern. In total props add 99,567 render and 3,288 solid triangles (budget 4,000).
