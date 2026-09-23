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
- **Turret sightlines:** no turret, sensor or battery may see standable floor
  inside rooms. Aligned doors are the usual cause: put a baffle wall just
  inside each door so it opens sideways. Turrets use
  `equipment::acquire_target` (line of sight + per-type range profile).
- **Walkable routes:** ramps need headroom and a floor opening above them, and
  closed undersides so players can't walk under a low ramp end. Shafts need an
  open face per level. Check every route with body sweeps.
- **Decoration is non-solid.** Trim, liners, markings, lamps and landing
  circles use `solid=False`, so nothing slick or snaggy sits on a floor.
- **Budget:** at most 3000 collision triangles per base; report render counts.
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
- **Pads:** optional `landing_pad` with named `deploy_slots` anchors for future
  player-placed turrets and vehicles.

## 4. Look at it before wiring

Before a map is embedded, preview it in the Raindance slot. There are no
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
- transform/anchor invariance
- unique equipment IDs and per-team circuits
- terrain-stat bounds and cut-seam checks
- bake determinism
- byte-identical rebuilds

Also run the other maps' suites and confirm their builds are unchanged.

## 6. Wire it in (embed)

Mirror the Tower Complex and Cairnhold commits:

- Build into `src/assets/maps/<id>/`. Two builds must be byte-identical.
- `crates/core/src/map_pack.rs`: embed through the shared helper.
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
