# Frostline

Original PeakRunner CTF map that replaces the private Snowblind Clone reference
pack. Built following `docs/map-pipeline.md`, it is embedded in the binary from
`src/assets/maps/frostline/` and holds the `snowblind-clone` rotation slot (key
kept so rotation configs keep working). Not deployed: the live server still
runs the old clone.

## Originality

Everything is original: station, outpost, emplacement, beacon and pine
geometry; procedural terrain from PeakRunner's own noise; procedural
textures and sky. The Snowblind reference was studied privately for
statistics and design intent only (notes in ignored `research/snowblind-study/`).
No Snowblind heights, geometry, textures, positions or audio are copied,
traced, resampled or fitted.

## Concept (auto-approved by the user; recommended choices taken)

The idea kept from the reference: a whiteout on steep snow mountains, two
far-apart mountain bases, a forward outpost that is a real second spawn, one
plasma emplacement per side, and pine cover.

What's new and ours:

- **Name:** Frostline. The other candidates were Whitecap Station and Hollowdrift.
- **Bases:** on the ground. Each team has a polar research station on a
  mountain shelf, raised on an insulated plinth.
  - Level 1 has the spawn hall with two inventory stations and a rear hall.
  - Level 2 is the command deck with the flag, glazed window bands and a roof
    hatch for attackers.
  - Ramps along the west and east walls join the levels (see the anti-camp
    pass below).
  - Every door has a baffle wall inside; the front one is also a porch.
- **Relay outpost:** a hut 20 m above the station floor on the right-flank
  ridge shoulder, with an inventory station, a repair pad, and a turret and
  sensor on the roof. Three of the eight team spawns are here.
- **Plasma emplacement:** an octagonal bastion 26 m up on the left-flank
  shoulder, with an access ramp.
- **Vehicle apron:** a flat deck beside each station with four marked
  `deploy_slots` for future player-placed turrets and vehicles.
- **Beacon:** a lit lattice mast on the central ridge, the one landmark you
  can find in the fog. It has a 12 m perch and four windbreak walls for cover.
- **Weather:** fog starts at 220 m and visibility ends at 700 m, against the
  reference's 150/450, so it's still a whiteout but playable.
- **Layout:** exactly point-symmetric. Flags are 816 m apart, shorter than the
  reference's 970 m but still the longest map.
- **Power:** a generator circuit per team. The reference had none.

## Files

- `maps/frostline.json` — bases and beacon placement.
- `scripts/build-frostline.py` — builds the pack; `--no-bake` skips lighting.
- `scripts/assets/frostline_station.py` — station, outpost, emplacement, apron.
- `scripts/assets/frostline_beacon.py`, `frostline_cavern.py` (the ice
  cavern), `frostline_flora.py`, `frostline_terrain.py`, `frostline_materials.py`.
- `scripts/test-frostline.py` — 28 tests.

Shared modules are used read-only: the kit `Mesh`, `structure_kit`,
`pack_writer`, `lightmap_bake`, and the noise helpers in `tower_complex_terrain`.

## Commands (from `src/`)

```sh
../research/local-assets/tools/venv/bin/python scripts/build-frostline.py [OUTPUT] [--no-bake]
../research/local-assets/tools/venv/bin/python scripts/test-frostline.py
```

The default output is `local-assets/frostline`. The build refuses to overwrite,
takes about 20 s with the bake, and two builds are byte-identical.

## Numbers

| | Frostline | Snowblind reference |
|---|---|---|
| Median slope | 35.8° | 39.8° |
| 90th percentile | 52.8° | 58.7° |
| Under 3° | 0.8% | 0.6% |
| Over 45° | 26% | 38% |
| Flag distance | 816 m | 970 m |

- **Collision:** 1,610 triangles per base (budget then 3,000; now 4,500, see `map-pipeline.md`), 124 for the beacon,
  about 2,000 for the 112 pines' trunks.
- **Render:** 17,668 triangles in total.
- **Pack:** 11 lightmap pages, about 14 MB.

## Validation done

- **`test-frostline.py`, 19 tests:**
  - floors, headroom, ramps and stairs, including closed undersides
  - engine support ray on more than 2,500 standing cells
  - no accidental slopes and no overlapping floor plates
  - 8 spawns per team: 1.2 m above the floor, clear all round, facing open space
  - flag on its plinth
  - transform invariance, budget, equipment kinds
  - unique IDs and independent circuits
  - materials and sky: deterministic, distinct from the other maps, horizon
    matching the fog
  - turret sightlines: none of the 6 turrets sees any of about 1,500+
    standing points inside rooms
  - terrain: symmetry, stats and profile
  - level ground under every structure
  - pine symmetry and clearance
  - beacon symmetry and perch
  - bake determinism
  - byte-identical rebuilds
- **Sightline test proven to bite:** it fails when the porch walls are removed
  with a 1 m baffle, and when the window glazing is removed.
- **Shared code untouched:** Tower Complex and Cairnhold rebuilt from scratch
  match their committed game payloads. Tower Complex's `map.json` differs only
  in two stale source-hash provenance fields, left by an earlier commit that
  edited its sources without rebuilding.
- **Visual QA:** real client captures of the aerial, beacon, station exterior
  and interior, flag deck, emplacement, outpost, slopes and a spawn, in
  `research/screenshots/frostline-v3-*`. v1 and v2 show the fixes: the camo
  rock texture, the emplacement sunk in a pit, and the plain outpost.

## Cleanup pass

- A baffle wall stands 1.4 m inside the rear door (walk round its west end or
  east side), so the back door is an airlock like the front.
- Ambient audio is Frostline's own synthesized polar wind (`polar_wind` in
  `build-frostline.py`), not Raindance's loop.
- `sky.fogColor` is a pale whiteout (0.84 0.87 0.90).
- Two outpost spawns were re-aimed or moved to face open floor
  (`scripts/assets/spawn_checks.py`).

## Anti-camp pass (station v3)

The user found the station "easy to camp in. one way in and out". A walk-graph
check (`scripts/assets/route_checks.py`) confirmed it: two ways into the
floors (front airlock, and a rear door that led straight through the
generator room), **one** approach to the flag deck (the west ramp) and a
generator room on the rear route. Changes, per base:

- **East side door** into the rear hall, with a baffle inside and a stair down
  to the shelf. The station now has doors on three sides.
- **East ramp**, the mirror of the west one. The flag deck has two approaches
  from opposite sides, and you can leave by the other ramp.
- **Basement generator room** under the hall, 7 m below the hall floor
  (the kit generator is 5.8 m tall), in cut terrain cells (6 per base). It has
  exactly two ways in: a railed stair down from the hall, and a tunnel door
  behind a baffle.
- **Tunnel** (6.4 m wide, 3.6 m headroom) east under the shelf to a sunken
  stair inside a new **service shed**, whose south door has a baffle. A cable
  duct covers the short lid between the station and the shed, so no cut cell
  is exposed.
- **Spawns** moved off the old choke: two in the front hall, one in the rear
  hall, two on the deck, three at the outpost. None in the basement.

Route counts on the committed pack, both teams (walking only, no jets):

| Region | Before | After |
|---|---|---|
| Ways into the station floors | 2 | 4 (front, rear, east door, up from the basement via the shed) |
| Approaches to the flag deck | 1 | 2 (west and east ramps) |
| Ways into the generator room | 2 (on the rear route) | exactly 2 (hall stair, tunnel) |

Collision is 2,366 triangles per base (budget 4,500); 19,976 render triangles
for the map; 12 lightmap pages. `test-frostline.py` checks the route counts,
the covered cut cells, the new floors, stairs and ramps, and that no turret can
see into the basement, tunnel or halls.

## Ice cavern (v5)

From the approved Ascend proposal: a skiable passage **through** the central
beacon ridge, a second lane beside the climb over the crest. It connects to
neither base. `scripts/assets/frostline_cavern.py`, both halves point-symmetric.

- **Line:** straight along the flag axis, portals at z 976 and 1072 (48 m either
  side of the centre), with an open trench 32 m long outside each portal. A
  skier turns at only v²/28 m/s² (a 128 m radius at 60 m/s), so a bent cavern
  would put skiers into the wall. On the axis, one straight cavern keeps the map
  point-symmetric and its mouths on the 8 m grid.
- **Size:** 24 m wide (three skiers abreast of a 0.52 m body), vertical ice walls
  8 m tall and a vault to 13 m at the crown, for a jet hop over another
  player and disc arcs. The trenches are 32 m wide between granite walls.
- **Floor:** 219 m at the portals, dipping on a cosine to 214 m in the
  middle (at most about 6°): you roll in and carry through, with no flat
  dead zone. The trench floor meets the slope at about 220 m.
- **Terrain:** heights and splat weights are unchanged, so the slope
  statistics are unchanged (median 35.7°, 90th percentile 52.8°, 0.89% under
  3°). 80 cells are cut (48 roofed, 32 open trench). The roof is an exact copy
  of the terrain surface over the roofed cells, with the client's quantized
  heights, alternating diagonals, normals, UVs and splat path (`layer.y = -2`),
  appended after the bake. It collides and renders as ridge.
- **Look:** blue ice-crust walls and vault, a snow floor, a granite sill,
  amber crown and wall light strips (16 bake lamps), icicles, and one ice
  boulder per half beside the ski line as cover.
- **Bake note:** there's no terrain behind surfaces in cut cells, and the bake
  lights whichever side of a surface looks more open. So every cavern
  surface has a render-only backing plate 0.4 m behind it. Without them, floor
  near the walls baked dim and brown, lit from below. The trench and portal
  backs sit 1 m lower so they stay under the ground. Fronts and backs are
  emitted in separate pairs so each quad keeps a single lightmap chart.
- **Counts:** 492 collision triangles (including the 96-triangle roof), 15
  lightmap pages (12 before).

Tests:
- `test-frostline.py` (28):
  - the roof equals the ridge
  - every trench cell is floored
  - the roof renders on the terrain path
  - body-band walk along three lanes from trench end to trench end
  - both portals open across their full cross-section
  - no turret or sensor can see inside
  - no spawn in or near it
  - no degenerate triangles
  - budget
- Rust:
  - `frostline_cavern_is_seamless_and_open_mouth_to_mouth` (terrain.rs)
  - `frostline_cavern_skis_through_mouth_to_mouth` (sim.rs): the real movement code, holding ski with no steering, from either end. At 30 m/s in, the skier never drops below 26.7 inside and exits the far trench at 24.8. At 16 m/s in, it's 15.6 inside and 14.0 out.

Screenshots: `research/screenshots/frostline-v5-{ridge,mouth-red,mouth-blue,inside,mid}.png`.

## Known gaps

- **Cavern not playtested:** whether it becomes the default capping lane
  instead of an alternative, and how the approach up the 36–40° slope to
  each trench plays. The approach can't be walked: you ski or jet up, as
  everywhere on Frostline.
- Bots don't use the cavern.
- Bots don't use the basement stair or tunnel.
- The roof hatch is a drop-in only; the walk-only route check doesn't count it.
- **Not playtested by a human:** skiing feel on 36° median slopes, how far the
  outpost is from the station, and how readable the fog is are all unverified.
- No real glow or bloom; the lit surfaces are bright textures.

## Open doorways and windows (2026-09-24)

Supersedes the baffle and glazing notes above. The front door (still under its
porch roof), the rear door, the east door, the service shed door and the
command deck's window bands are open. The windows (2.6 m tall) are jet-in
entries onto the flag deck. Baffles remain at the basement's tunnel door
(generator room) and inside the relay outpost (a spawn room). The open windows
and the now open-ended rear hall exposed spawns on the command deck and in the
rear hall, so all five station spawns now stand in the front hall, near the
front wall or down its sides, where no line through an opening reaches them.

## Capture & Hold, flag access and look pass (station v4)

Supersedes earlier door, basement, sky and fog numbers above.

- **Centre point, active in CTF.** The shared C&H tower (`cnh_tower.py`)
  stands on the beacon's origin: its pylon rises inside the lattice legs, so
  the 12.4 m perch is now a ring round it (the `Beacon` point, 12 m ring at
  (1024, 262, 1024)). It flips after 10 s with only one team inside, and while
  held its **drain field** (60 m, 10 energy/s, horizontal distance) saps the
  holder's enemies only. 60 m covers the ridge crossing, both cavern portals
  (48 m) and the whole cavern beneath; the trench ends (60–80 m) and the
  valleys stay clear, so attackers reach the field's edge with a full tank.
  A render-only **drain emitter** round the perch (four lit projector heads
  angled at the field and a halo under the lantern) reads as the thing
  draining you.
- **Flank points (Capture & Hold only):** `West Col` (896, ~286, 1024) and
  `East Col` (1152, ~286, 1024), mirrored through the centre, each 420 m from
  both flags, on shelves levelled 16 m round the ring and kept clear of pines.
- **Doors:** front, rear and east doors are 6 m wide and 5.5 m tall (the porch
  widened to match). The hall partition has two 6 m doorways.
- **Two-level void:** a 10 x 10 m opening in the command deck in front of the
  flag joins the hall and the deck into one space; enter it from either level
  or jet straight up from the hall to the flag. The roof hatch now sits over
  the void, so dropping in lands in the hall or on the deck edge.
- **Basement:** floor lowered to 6.5 m clear. Its stair and the shed's exit
  stair are now about 28–29° (the engine counts ground as steep only past 35°).
  The generator room keeps exactly two entrances.
- **Cover:** the hall's crates are 0.8 m thick instead of 1.8–2.6 m.
- **Flag routes** (`route_checks.flag_routes`, walking plus jet hops): 13 per
  flag, up from 6 (6 entries; either ramp or a jet up the void to the flag).
  `test-frostline.py` holds at least 10.
- **Flicker:** the hall floor ran out to the walls' outer faces, so its edges
  shared a plane with the wall faces (the dark line on the station waist
  band); it now stops at the inner faces. Two decorative legs under the front
  deck capped exactly in its walking surface; they stop at its underside.
  40 pairs before, 0 in Frostline's own geometry after. Three 0.04 m² pairs
  remain on the shared kit turret mount (team band flush on its collar), owned
  by `build-original-map.py`. `test-frostline.py` checks for any others.
- **Hovering edges:** no solid wall hovers on Frostline. The audit's examples
  were render-only bands and the kit turret on the emplacement's top cap.
- **Cave mouths:** the audit's "floating grey prism" was the red cavern
  portal: a bare granite face that read as a block in the whiteout. The top
  5 m of each portal face and the top 3 m of the trench walls are now snow,
  with a snow cornice over the portal lip.
- **Look:** a cold procedural overcast sky (`look.sky`), pale cool sun with a
  small dim disc (direction unchanged: lightmaps are baked with the shared
  fixed sun, so a truly low sun needs a bake change), cool ambient, and height
  fog pooling below 205 m in the valleys. Fog runs 180–900 m so the beacon
  ridge reads from both shelves.

Counts: 2,362 collision triangles per base, 376 for the beacon and centre
tower, 432 for both Col towers, 560 cavern; 8,072 total, 23,354 render,
16 lightmap pages. Screenshots: `research/screenshots/frostline-pass-*.png`.
Not playtested: the drain radius and the void's effect on flag defence need a
human match; bots don't use the void or the points' drain tactics.

## Props and terrain shade

Frostline scatters original props from `scripts/assets/props.py` (theme `frostline`): snow-capped rocks and large ice shards (solid), pine saplings, small ice shards and frost tufts (render-only). The committed pack has 44 snow rock, 284 ice shard, 200 sapling, 1800 tuft. That adds 21,089 render triangles and 1,680 solid ones (prop budget 2,500). Big props come in mirrored pairs, stay at least 80 m from each flag and 28 m off the ski lanes, and sink into the ground. The baked terrain shade map `shade.rg` (`scripts/assets/terrain_shade.py`) shadows the ground under structures, hulls and big props for the map's sun, plus terrain self-shadow and occlusion. About 24.7% of the tile is in shadow. See `docs/map-pipeline.md`.

**Cover and ground layer.** 16 mirrored cover pieces (8 ice ridge, 8 rock cluster; 8 crouch-height, 8 full-height) stand 36–62 m beside the ski lanes and around the control points, blocking movement and shots. The grass layer adds 7987 clumps in 132 meadow patches (80,000 render-only triangles). Scenery: 44 snow rock, 344 ice shard, 260 sapling. In total props add 99,368 render and 2,808 solid triangles (budget 4,000).

## Water

Two mirrored ponds, the **Meltwater Pool**, sit at (880, 752) and its mirror (1168, 1296): ellipses 20 × 16 m, turned 10°, carved
into the terrain by `scripts/assets/water_bodies.py` from the map definition's
`"water"` entry. The bed shelves from ankle depth at the shore through waist
depth to 2.5 m at the centre (about 1,090 m² each), a crest one grid step wide
holds the water in, and the banks take the ice crust splat. They are real
`water_volumes`: they slow walkers, brake skiers hard, float swimmers and cost
extra jet energy (see `docs/map-pipeline.md`). They are 77 m from the nearest lane, on each station's flank; two mirrored pines on the banks were dropped, so they are a
choice beside the route, not a block across it; flags, spawns, capture rings
and holes are all well clear (checked by `water_checks.assert_ponds`). The
underwater tint is glacial cyan. No prop, cover piece or bot route is placed in
the water, and the legacy render-only plane is off.

