# Making an original map

This is the process used for Tower Complex, Cairnhold, Frostline and Dustreach,
which replaced the Broadside, Stonehenge, Snowblind and Desert of Death clones. Follow it for every new or replacement
map. Commands run from `src/`; Python uses the numpy venv at
`../research/local-assets/tools/venv/bin/python`.

## 0. Rules that never bend

- **Original only.** Every rotation slot is now an original embedded map; no
  slot loads a private pack. A private reference pack in ignored `research/`
  or `local-assets/` may still be studied for statistics and
  design intent. Never copy, trace, resample, blend or fit its heights,
  geometry, textures, positions or audio. Say so in the map's doc.
- Study notes, reference renders and captures live in ignored `research/`.
  Never put them under `src/` or `docs/`, never embed them, never publish them.
- Don't change approved movement physics or shaders to make a map work.
  Fix the geometry.
- Other maps' builds must stay byte-identical. If you touch a shared module
  (`structure_kit.py`, `pack_writer.py`, `lightmap_bake.py`, `landing_pad.py`,
  `fortress_rooms.py`, `build-original-map.py`), rebuild every map that uses it
  and compare all payload hashes. When several agents build maps at the same
  time, treat shared modules as read-only and keep helpers map-local.

## 1. Study the reference (private)

Save to `research/<map>-study/`:

- What players come to the map for: the idea, not the shapes.
- Base types, footprints, positions; what's floating vs. on the ground.
- Flag placement and exposure; flag-to-flag and base-to-landmark distances.
- Spawn regions and weights; equipment (gens, inventories, turrets, sensors)
  and what each covers.
- Terrain: height range, slope histogram (median, 90th percentile, % under 3°,
  % over 30°), the profile along the flag line, ski runs, routes, chokepoints.

## 2. Concept

Publish a concept page (site plan, base plan or section, a terrain sketch of
*our* design, "what we keep / what's new" in words, open questions, 2–3
original names; no Tribes names, no reference images). Get the user's call on
the name and the big choices (ground vs. floating bases, flag distance, what
the landmark does, which slot it replaces). When the user says to proceed
without a review, take the concept's recommended answers and say which.

## 3. Build

Files, by analogy with `tower-complex` and `cairnhold`:

- `maps/<id>.json`: id, name, seed, bases (team, position, yaw).
- `scripts/build-<id>.py`: imports `build-original-map.py` as `kit`, starts
  from `kit.base_pack()` (shared textures, ambience, layer layout — never
  another map's committed pack), and uses `pack_writer` for materials, the
  lightmap bake and the pack. Refuses to overwrite; `--no-bake` skips baking.
- `scripts/assets/<id>_*.py`: base, landmark, terrain, materials modules. Each
  `build(mesh, team, circuit)` returns local-space anchors (flag, spawn points,
  entrances, rooms, deploy slots).
- `scripts/test-<id>.py`: see step 5.
- Build into ignored `local-assets/<id>/` until the user has seen it.

Design checklist (each item has bitten us once):

- **Spawns:** 8 per team via manifest `spawn_points` (`[x,y,z,yaw]`), each
  centre **1.2 m above a solid floor** with headroom, facing open space. Lower
  spawns start the support ray inside the floor slab and the player slides
  frictionless.
- **Spawn forward clearance:** from each spawn the body's width must see 6 m
  of clear space ahead, and a half-second walk forward must stay on the same
  floor (no deck edge or ramp top). `scripts/assets/spawn_checks.py` checks the
  built pack's whole world (other structures and terrain included); every
  map's suite runs it, and the Rust spawn test walks it.
- **Open doorways and windows:** doors, bridge doors and windows are open, so
  players can shoot and fly straight in. Don't glaze windows or wall off
  doors. Baffles stay only on generator rooms and spawn rooms. Size each
  window as either an entry (taller than a 2.56 m body; count it in the route
  checks) or a firing slit.
- **Turret sightlines:** fix turrets, not openings. Every fixed turret gets a
  `facing` and `arc` (`scripts/assets/turret_arcs.py`: toward the enemy
  flag, 200 degrees). Beyond `equipment::ALL_ROUND_RANGE` (15 m) the server
  only engages targets inside that field of fire; closer in it covers every
  direction, so a turret still guards its own bridge or ramp. No turret may
  engage a standable point more than 3 m (`sightline_checks.DOOR_DEPTH`)
  past an opening's inner face. When a turret still lines up with an opening,
  move it (as the Dustreach sentry was) rather than adding a wall. Tests use
  `sightline_checks.visible` in Python and the same rule in `sim.rs`.
- **Spawns and openings:** no spawn in a room may be visible from 40-100 m
  out in the field through an open door or window
  (`spawn_checks.exposed`). Open colonnades and outdoor spawns are exempt.
- **Walkable routes:** ramps need headroom and a floor opening above them, and
  closed undersides so players can't walk under a low ramp end. Shafts need an
  open face per level. Check every route with body sweeps. The engine's body
  sweep reaches 2.64 m above a surface, so any ceiling edge over a ramp needs
  about 2.8 m of clearance (Cairnhold's stair opening had to grow for this).
- **Generator rooms:** the kit generator is 5.8 m tall and its hit bar hangs
  6.4 m over its floor, so a generator room needs about 7 m of clear height (or
  a well, as in Cairnhold). With less, the bar hangs beside the generator
  (Dustreach's cistern). Keep spawns well away from the only ways in.
- **Decoration is non-solid.** Trim, liners, markings, lamps and landing
  circles use `solid=False`, so nothing slick or snaggy sits on a floor.
- **Budget:** at most 4500 collision triangles per base, including its
  outbuildings, tunnels and pads (`scripts/assets/budgets.py`, checked by
  every map's suite); report render counts. Raised from 3000 for Dustreach's
  cistern and storehouse after measuring. At 3442 per base, Dustreach's sim tick
  averaged 141 µs (was 112 µs at 2056), worst tick ~310 µs either way,
  against a 16 667 µs tick. A mixed collision query averaged 6.2 µs (was 3.4 µs),
  because the base's 32 m buckets got denser. The densest bucket (1029
  triangles) matches Cairnhold's (1066). The eight-client load test sustained
  726 ticks in 12 s before and after. Re-measure before going past 4500 with
  the ignored `collision_budget_timing` probe in `sim.rs`, and with
  `PEAKRUNNER_LOAD_MAP=<key>` on the load test.
- **Underground:** tunnels and basements sit in terrain holes (whole 8 m
  cells, listed in the manifest's `holes`). Keep every hole cell under a
  structure, so no lid is exposed and the cut's edge keeps the flat site
  height. Otherwise the lid must meet the terrain exactly, like Cairnhold's
  trench. A generator room gets exactly two entrances and no spawn points.
- **Terrain:** original procedural. Match the study's slope *profile*, keep
  bases exactly mirrored (180° symmetry), keep ski runs and more than one
  route, avoid large flats. Where terrain is cut for trenches or bunkers,
  holes and collision must match with no fall-through seams.
- **Materials:** a map-specific material module with team variants (banner,
  trim). Every texture replacement must write **all mip levels**; an early
  build only wrote level 0, so distant surfaces showed another map's textures.
- **Lighting:** bake lightmaps (AO, sun shadows, lamp strips) through
  `pack_writer.bake_lightmaps`. Never write build time or other non-deterministic
  values into `map.json`: it changes the fingerprint on every build.
- **Fog colour:** optional manifest `sky.fogColor` (`"r g b"`, 0–1) tints
  distance fog and the sky horizon. Omit it for the default grey (0.62). Pick
  it to match the map's sky (warm haze for desert, near-white for snow).
- **Look (optional `look` object in `map.json`, `peakrunner_core::look`):**
  renderer-only, so it never touches gameplay. Every field is optional and
  unknown keys are rejected:
  - `sun_direction` `[x,y,z]` toward the sun, and `sun_color` `[r,g,b]`.
    Lightmaps are baked with the fixed sun `[-1,1,-1]`, so a moved sun needs a
    rebake. A core test fails if it's more than 3° from the bake.
  - `sun_disc`: brightness of the visible sun in the sky (default 1, 0 hides it).
  - `exposure`: 0.25–4, default 1.
  - `ambient_sky` / `ambient_ground`: hemisphere ambient; the defaults average
    the old flat 0.55.
  - `height_fog` `{density, base, falloff}`: exponential fog that pools below
    `base` height, e.g. valley mist or a whiteout.
  - `sky` `{zenith, horizon, cloud_cover 0–1, cloud_color, cloud_scale,
    sun_size}`: a procedural gradient-and-cloud sky that replaces the cubemap
    faces. Use it to give each map a distinct sky.

  Preview a look without rebuilding: set `QA_LOOK='{"sky":{...}}'` (the same
  JSON object) on a `QA_LOCAL` capture. It applies to every map, and invalid
  JSON is ignored.

  Global renderer changes apply to every map regardless: 4× MSAA when the GPU
  supports it (a settings toggle), a close-range terrain detail layer, softer
  terrain blending from the air, hemisphere ambient, a gentle highlight
  shoulder, and a sun disc at the lighting direction.
- **Control points:** Capture & Hold towers go in the manifest's
  `control_points` (id, name, pos on the floor at the ring centre, radius, optional
  `ctf_active` and `drain`). Give every map at least 2 (centre plus one per side,
  mirrored), keep each ring on open, walkable ground with more than one approach,
  and keep spawns outside rings. See `docs/capture-and-hold.md`.
- **Water:** the legacy `water` plane is render-only. Water that slows players
  goes in the manifest's `water_volumes` (up to 32): each has `surface` (world
  y), exactly one footprint (`rect` `[x0, z0, x1, z1]` or `polygon`
  `[[x, z], ...]`, 3–64 points, convex or star-shaped about its centroid: the
  surface is fanned from the centroid), optional
  `depth` (volume ends that far below the surface), `flow` `[x, z]` m/s (a
  current, at most 30), and `color` (linear RGB 0–1, used for the underwater
  tint). See `crates/core/src/water.rs`. In the shared sim, horizontal drag
  scales with immersion^1.5 (3/s fully submerged), skiing in water adds up to
  1.5/s, walking top speed drops by 60% x immersion, buoyancy floats a swimmer
  with the eye near the surface, and jetting while waist-deep costs 1.5x energy.
  Chaingun rounds and turret plasma fizzle underwater; discs and grenades are
  dragged hard (5/s). Measured on a flat patch, a skier entering at 30 m/s keeps
  29.7 m/s after 1 s dry, 20.8 ankle-deep, 2.7 waist-deep and 1.7 swimming; walking
  top speed is 11.2 dry and 7.0 waist-deep; jetting out of deep water takes 0.43 s.
  Keep water off spawns and capture rings, keep ski lanes dry unless the
  slowdown is the point, and make sure every basin has a shore a wader can walk
  out of. `QA_WATER=surface,x0,z0,x1,z1[,flow_x,flow_z];...` stages volumes in
  a local match for previews.
  **Placing ponds:** declare bodies in the map definition's `"water"` entry
  (`centre`, `mirror`, splat `channel` for the wet bank, and `bodies`: ellipses
  with `x`, `z`, `rx`, `rz`, optional `yaw`, `depth`, `flow`, `color`), then call
  `water_bodies.apply(definition, grid)` right after the terrain grid is made,
  `water_bodies.wet_banks(...)` on the splat weights, write the returned
  `water_volumes` into the manifest with `water_enabled: false`, and pass them to
  `add_props_and_shade(..., water=...)` so no prop lands in the water. The
  carver sets the surface from the ground round the ellipse, shelves the bed
  from ankle depth at the shore through waist depth to the full depth, raises a
  crest at least one grid step wide outside the shore, and traces the outline
  along the finished shoreline so no water hangs past the bank. Put ponds off
  the ski lanes where they make a choice (existing ponds sit 77–125 m from the
  lanes). Suites call `water_checks.assert_ponds(...)` (depth, dry edge,
  clearance from flags, spawns, rings, holes and lanes, mirrored size) and the
  core test `every_maps_water_slows_skiers_who_can_jet_out` skis into every
  declared volume with the real movement code.
- **Pads:** optional `landing_pad` with named `deploy_slots` anchors for future
  player-placed turrets and vehicles.
- **Props and terrain shade:** after the lightmap bake, call
  `pack_writer.add_props_and_shade(kit, files, manifest, theme, seed, holes,
  flags, spawn_points, control_points)`. It scatters the map's prop theme
  (`scripts/assets/props.py`) and bakes `shade.rg` (`terrain_shade.py`).
  - Props are original low-poly shapes built from the map's own materials.
    Small props (tufts, ferns, heather, scrub, saplings, bones) are
    render-only. Big props (boulders, outcrops, standing stones, logs, large
    ice shards) are solid and sunk into the ground. They come in mirrored
    pairs through the flag midpoint, stay at least 28 m off the ski lanes
    (flag to flag, flag to each control point), and take at most
    `props.SCENERY_COLLISION_TRIS` (2500) solid triangles. Scenery and cover
    together stay under `props.PROP_COLLISION_TRIS` (4000) per map, outside
    the per-base budget.
  - **Cover** (`place_cover`, the theme's `cover` list): deliberate solid
    pieces that block movement and shots, such as dry-stone walls, log piles,
    rock clusters, sandstone ruins, crates, ice ridges and plating debris.
    They stand 36–62 m to either side of each ski lane and in a ring
    34–46 m around each control point, mirrored through the flag midpoint.
    Heights are crouch cover (1.25–1.5 m) or full cover (2.6–3.6 m). Walls
    are built in 2.5 m segments that follow the ground and reach 0.5 m below
    it, so nothing hovers or leaves a gap underneath. Rock clusters overlap
    their rocks, so no wedge can trap a player. Cover keeps
    `BASE_CLEAR` off each flag (flag routes are unchanged), off lanes, rings,
    spawns and holes, 30 m apart and 4 m from any other solid prop. Each map
    gets 16–22 pieces.
  - **Ground layer** (`ground_layer`): up to `props.GROUND_TRIS` (80,000)
    render-only triangles of grass clumps (8–12 blades each). Two thirds go
    into dense meadow patches 4–9 m across, weighted towards ski lanes, flags
    and control points; the rest is scattered, thinner away from them.
  - **Instanced props (`props.bin`):** every render-only prop (grass clumps,
    tufts, ferns, heather, scrub, saplings, small ice shards, bones) is a GPU
    instance, not baked vertices. `props.Instancer` builds
    `props.INSTANCE_VARIANTS` (24) shapes per kind at size 1; each placement
    picks a variant by its shape seed and keeps its own position, yaw and
    size as a uniform scale. The renderer (`map.wgsl` `vs_prop`) applies the
    kit's `Mesh.point` yaw, recomputes the kit's planar world UVs (world / 4
    on the two axes other than the dominant normal) and shares `fs_map`, so
    lighting, terrain shade, fog, look and bloom match baked geometry. Solid
    props (big scenery, cover) stay baked: their collision is in
    `collision.bin` and they cast into `shade.rg`. Layout: header `PRP1`,
    mesh count, instance count, a mesh table (first vertex, vertex count,
    first instance, instance count), 8-float vertices (local position,
    normal, material) and 8-float instances (x, y, z, yaw, scale, mesh
    index), sorted by mesh, so each mesh is one instanced draw.
    `map_pack::PropSet` validates it at load. The payload is optional (older
    packs load without it) and is compressed by `crates/core/build.rs` and
    copied by the bundle scripts. The grass budget counts instance triangles,
    so it no longer grows the pack: Tower Complex went from 19.1 MB of
    vertices to 3.7 MB plus 0.4 MB of instances.
  - Nothing is placed near existing collision geometry, terrain holes and
    their neighbour cells, flags, spawns, capture rings, water, or the map
    edge. Density thins away from the flags and points.
  - Props keep the live-lighting path (light layer -1), so they add no
    lightmap pages. Each pack's `props` block in `map.json` records the
    counts and every big prop.
  - `shade.rg` is a 1024x1024 map (2 m texels), two bytes per texel: sun
    visibility and ambient occlusion. It comes from a soft heightfield
    ray-march, an orthographic sun depth map of every opaque structure and big
    prop (so floating hulls shadow the ground below), and overhead and concavity
    occlusion. The terrain shader and props multiply sun light by R and
    ambient by G. Bake it with the map's own sun, the same as the lightmaps.
    The payload is optional: packs without it render unshaded. It is compressed
    by `crates/core/build.rs` and copied by the bundle scripts.
  - Each map suite runs `assets/prop_checks.py` on its committed pack. It
    checks at least 12 mirrored cover pieces clear of lanes, flags, spawns,
    rings and holes, that each piece stops a shot at crouch height, that
    a player standing a metre out from any face can walk three metres away,
    that the ground layer spends its triangle budget, and that `props.bin`
    matches its manifest hash and summary with instances on the ground and
    clear of flags and rings (`check_instances`).

## 4. Look at it before wiring

Before a map is embedded, preview it in the `raindance` (Old Holler) slot. There are no
private-pack slots any more; `PEAKRUNNER_MAP_PACK` loads any pack there through
the same validating `MapPack::load`, with no manifest patching:

1. Build into ignored `local-assets/<id>/`.
2. `PEAKRUNNER_MAP_PACK=<absolute path to local-assets/<id>> QA_LOCAL=1
   QA_MAP=raindance`, plus `QA_CAPTURE_PATH` and
   `QA_FLYCAM=x,y,z,yaw,pitch,fov` from `examples/launch_smoke.rs`.
3. Capture: aerial, landmark, base exterior, base interior, flag area, each
   turret emplacement, ground-level slopes, one spawn. Compute cameras from
   world-space anchors: a room corner at eye height looking across, never
   into a wall. View every image and re-aim bad ones.
4. Captures time out once the display sleeps. Run them straight after the
   build, and stop and report missing views instead of retrying for long.
5. Screenshots go to `research/screenshots/`.

## 5. Tests

`scripts/test-<id>.py` must cover:

- floors and headroom along every route; no overlapping coplanar floors
- the support-ray check for every spawn
- no slick surfaces
- sightlines into rooms
- route counts with `scripts/assets/route_checks.py` on the committed pack:
  at least 3 ways into the main building, 2 onto the flag deck, exactly 2
  into the generator room. Ground maps use the walking model; floating bases
  pass `airborne=True` (open-sky decks, drops and short jet hops, see the
  module doc). Define room-shaped regions; a box around an open area scores 1.
- transform/anchor invariance
- unique equipment IDs and per-team circuits
- terrain-stat bounds and cut-seam checks
- bake determinism
- byte-identical rebuilds

Also run the other maps' suites and confirm their builds are unchanged.

## 6. Wire it in (embed)

Mirror the Tower Complex and Cairnhold commits:

- Build into `src/assets/maps/<id>/`. Two builds must be byte-identical.
- `crates/core/src/map_pack.rs`: add an `embedded_map!` line, and add the id
  to `MAPS` in `crates/core/build.rs`. The build script zlib-compresses the
  six payloads into `OUT_DIR`; `map.json` stays raw, and the manifest hashes
  (and so the fingerprint) remain over the uncompressed bytes. Commit only the
  raw pack; never commit compressed copies.
- `crates/core/src/terrain.rs`: always listed; `MapInfo` with the real name
  and base positions. A new slot needs a new `MapId` variant and key.
- Update tests in `map_catalog.rs`, `rotation.rs` and `server/src/lib.rs`.
  Add acceptance tests: spawns grounded, flags on a deck, and turrets can't
  see into rooms (the `sim.rs` sightline tests).
- `crates/protocol/src/lib.rs`: bump the `mapsN` marker and add the new
  fingerprint.
- Add the pack to the server, client and launcher Dockerfiles
  (`COPY assets/maps/<id> ...`), or Docker builds fail on `include_bytes!`.
- Update AGENTS.md, `docs/multi-map-system.md`, the package release-notes
  input and the map's own doc. Never edit records of a published release.
- Verify with the Build and verification list in AGENTS.md, plus the ignored
  connected-rotation test (no private packs needed). Capture spawn and aerial
  shots through the normal path with no `PEAKRUNNER_MAP_PACK`.

## 7. Commit

Stage map sources, the embedded pack, Rust, and docs by name. Never stage
`research/` or `local-assets/`. Commit the build scripts first, then the
wiring. Nothing is deployed until a full matching client and server release
(see "Safe release procedure" in AGENTS.md).
