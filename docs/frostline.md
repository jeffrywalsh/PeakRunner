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
- `scripts/assets/frostline_beacon.py`, `frostline_flora.py`,
  `frostline_terrain.py`, `frostline_materials.py`.
- `scripts/test-frostline.py` — 19 tests.

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

## Known gaps

- Bots don't use the basement stair or tunnel.
- The roof hatch is a drop-in only; the walk-only route check doesn't count it.
- **Not playtested by a human:** skiing feel on 36° median slopes, how far the
  outpost is from the station, and how readable the fog is are all unverified.
- No real glow or bloom; the lit surfaces are bright textures.
