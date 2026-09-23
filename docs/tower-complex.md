# Tower Complex

Original PeakRunner CTF map that replaces the Broadside Clone reference layout.
It keeps the `broadside-clone` key and `MapId::BroadsideClone`, so existing
rotation configs still resolve. Nothing here is extracted from Tribes.

**Status:** source only, on branch `map/reference-layouts`. It is not deployed.
The live server and published `0.1.0-private.20260921.1` clients still use the
old Broadside clone pack. Shipping it needs a full matching client and server
release, because gameplay compatibility changed (see below).

## Design

Each team base has five floating hulls over rolling hills. The bases are 408 m
apart (z 820 and 1228) and the flags sit about 88–95 m above the ground:

- **Central tower, front centre.** Three levels, entered by the front door.
  - Level 1: open area with cover blocks and ramps.
  - Level 2: the flag, on a plinth between two pillars.
  - Level 3: framed windows (collision panes act as glass) and inventory stations.
  - A gunmetal central shaft has one open face per level and floor holes at L2
    and L3. Hazard striping marks only its edges.
  - Above the playable block, stepped setbacks, spires and a team banner with a
    chevron emblem take the tower to about 52 m.
- **Two turret pods, front left and front right.** Each sits on an open bridge
  across open air.
- **Entry baffles.** Solid partitions 2 m inside the front wall stand behind
  both bridge doors and the front door. Players turn sideways through two gaps
  (|x| 4.2–6.0 m) into Level 1. The L1→L2 ramp foot sits behind the left baffle
  (local z −8.8). Before this, each pod turret looked straight down its bridge,
  through its door, along a walkway and into the rear tunnel. That let it track
  players inside the tower (570 standable interior points) and in the ship room
  (60), which felt like shooting through walls. Engine line of sight was correct.
- **Generator room, rear left, and ship platform, rear right.** Both connect to
  Level 1 through enclosed tunnels.
- **Keels.** Every hull has a tapered keel with thruster nozzles.

Blue mirrors Red. Each base has about 7,550 render and 2,070 collision triangles
(the collision budget was then 3,000; now 4,500, see `map-pipeline.md`).

### Terrain

`scripts/assets/tower_complex_terrain.py` generates the heightfield from
PeakRunner's own hash noise: a midfield basin between the bases, a warped
encircling ridge broken by four passes, a hill shoulder rising behind each
base (downhill start toward the basin), rolling fractal hills everywhere,
ridged detail on high ground and bounding hills at the map edge. Ground within
the complex footprint stays at least 20 m under the deepest keel tip. Splat
weights follow slope and height (rock on steep ground).

**No Broadside (or any other game's) height data is used.** Broadside's
private clone pack was studied only for shape statistics (kept in ignored
`research/terrain-study/`): its interior has a median slope of 19.7°, 90th
percentile 42°, 7% of cells under 3°, flags 68–82 m above ground and a ~50 m
drop between the bases. The new terrain measures median 19.0°, 90th percentile
35°, 2% under 3°, flags 88–95 m up and a 41–47 m midfield dip. The v4 canyon it
replaces was 30% flat with a 5° median.

Five midfield peak pairs (`PEAKS` in the terrain module) are mirrored through
the field centre so neither half is favoured. They stand off the
flag-to-flag line, which stays a low lane, and add 25–85 m summits between
the passes for cover and jet launch points. With them the interior measures
median 20.3°, 90th percentile 40°, 1.7% under 3°, and the midfield dip on the
flag line is 32 m (35 m without the peaks).

### Landing pads

`scripts/assets/landing_pad.py` builds one open floating pad per team half
(`pads` in `maps/tower-complex.json`): red at (1119, 190, 904), blue mirrored
at (929, 190, 1144). The pad has a 24 m square solid deck at y 190, 50 m below
the base decks. A 0.5 m lip runs round the edge with a 6 m walk-off gap
centred on each side. The underside has a tapered keel 16 m deep, a central
thruster, four engine pods, team trim and corner light posts included in the
bake. Markings and posts are render-only, so the deck is one flat walkable
plane. All armors share the 0.52 m body, so a heavy fits with room to spare.
The terrain is held at least 34 m under each deck; the ground sits about 41 m
under the red pad and 63 m under the blue one, since the fractal ground is
not mirrored. Nearby peaks rise to about deck height as jet launch points.

The pads have no equipment. Four marked `deploy_slots` per pad, recorded in
the manifest's informational `instances` anchors, reserve spots for future
placed turrets or vehicles. The Rust manifest schema is unchanged.

### Spawns

Each team has eight authored spawn points (`spawn_points` in `map.json`,
`[x, y, z, yaw]`, player centre 1.2 m above the floor): two on each tower
level, one in the generator room and one in the ship platform room. The server
picks one at random on every respawn, never the same point twice in a row for
a team, and avoids points with a live enemy within 30 m when another choice
exists. Offline bots use the same points. Maps without `spawn_points` keep the
old single spawn path unchanged.

### Slick spawn fix

The v4 spawn anchor was only 0.2 m above the 1 m floor slab. The engine finds a
standing player's floor with a ray from 0.15 m above the feet, so it started
inside the slab, hit its underside and treated the player as airborne: no
ground braking, so walking coasted like ice until the next jump. Spawns now use
the 1.2 m convention every other pack uses. Probing also showed players could
walk under the low ends of both switchback ramps and get popped upward into a
coast, so solid skirts now close the space under each ramp wherever its
underside is lower than 2.4 m.

Regression tests: the Python suite checks that the support ray finds the
up-facing floor top on every standable cell, that tilted walkable faces exist
only on ramps, the flag plinth and kit equipment, that every spawn stands on
the floor with room around it, and that the under-ramp wedges are closed. The
Rust suite settles a player on every spawn point, walks and releases, and
asserts it is grounded and stops within 2.2 m.

`pod_turrets_cannot_see_into_tower_rooms` samples standable points on a 1 m
grid in the tower (all levels), both tunnels and both rear rooms. It asserts no
pod turret has a clear line to any of them. The only exception is the 2 m entry
vestibule behind the front wall. `tower_entries_route_around_the_baffles` walks a
body from each front opening around the baffles, and on to the ramp for the left
door. It also checks that walking straight on from each opening is blocked.

## Build and test

Run these from `src/` with the numpy venv:

```sh
../research/local-assets/tools/venv/bin/python scripts/build-tower-complex.py            # -> assets/maps/tower-complex
../research/local-assets/tools/venv/bin/python scripts/build-tower-complex.py OUT --no-bake
../research/local-assets/tools/venv/bin/python scripts/test-tower-complex.py            # 17 tests
```

The builder refuses to overwrite. Two builds produce byte-identical packs,
including `map.json`, so the fingerprint is stable. The wall-clock bake time is
printed and is not written into the manifest.

| File | Size |
| --- | --- |
| `vertices.bin` | 2.2 MB |
| `collision.bin` | 0.15 MB |
| `height.bin` | 0.13 MB |
| `weights.rgba` | 0.26 MB |
| `textures.rgba` | 10.5 MB |
| `ambient.f32` | 0.7 MB |
| **Total** | **~13.9 MB** |

The pack is embedded with `include_bytes!` for both native and WASM builds. The
macOS release binary grew from 35.9 MB to 49.2 MB.

Sources:

- Geometry: `scripts/assets/tower_complex.py`
- Materials: `scripts/assets/tower_complex_materials.py`
- Lightmap bake: `scripts/assets/lightmap_bake.py`
- Terrain: `scripts/assets/tower_complex_terrain.py`
- Map definition: `maps/tower-complex.json`
- Section views: `scripts/inspect-tower-complex.py`

## Baked lighting

`lightmap_bake.py` bakes, per texel:

- sky and ground ambient, times ambient occlusion
- the renderer's fixed sun, with shadow rays
- 108 lamp samples from the ceiling light strips

The result is 10 pages (30 texture layers), written to the existing lightmap path
(`layer.y >= 0` in `src/map.wgsl`). There is no shader change. Baking takes
about 15 s on 13 processes.

## Compatibility

`game_protocol()` now uses `maps5:<raindance>:<tower-complex>:<cairnhold>`
fingerprints (Tower Complex introduced `maps3`, Cairnhold made it `maps4`, and
removing Skybreak made it `maps5`).
Broadside and Stonehenge no longer contribute `private1` segments. Only
Snowblind and Desert of Death do, and only when their packs are installed. Old clients and servers are therefore incompatible
with this source.

## Validation done

- Rust:
  - `cargo test --workspace --lib`: 155 passed. The tests check that the
    map is embedded, listed and non-private, that every spawn point is on its
    own team's deck with room around it, that players settle and brake on
    each spawn, that respawns vary among the team's points, that the flags
    have a deck and roof, and that they sit 60–120 m above the terrain.
  - The all-targets, WASM (existing `Lobby` warning only) and release builds
    succeed, and the app-boundary check passes.
  - The ignored connected-rotation test, with the three private packs, rotated
    two clients through all six maps and back.
- Rendered through the normal embedded path (`QA_LOCAL=1 QA_MAP=broadside-clone`),
  plus `QA_FLYCAM` views of the interiors and exterior.

## Known gaps

- No human playtest yet: traversal, balance and the fall risk are all unproven.
- The flag is the engine's flat red square, and the turret heads are the kit's
  boxes. Both are shared code, so changing them affects every map.
- There is no emissive or bloom. "Lights" are just bright textures.
- The ship platform has no gameplay role yet.
- The landing pads' deploy slots are placeholders; nothing can be placed on
  them yet. Whether a heavy can actually reach a pad by jetting from the
  ground or a peak is unverified in play.
- Red's front faces away from the fixed sun, so the exterior reads dark.
- Skiing feel on the new terrain is unverified; slope statistics are not a
  substitute for a human run.
- The client keeps its own view yaw after a server respawn; whether the new
  per-point facing is applied on a networked client is unverified.
