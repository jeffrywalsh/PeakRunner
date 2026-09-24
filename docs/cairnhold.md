# Cairnhold (v2, local build only)

Cairnhold is the original PeakRunner map intended to replace the Stonehenge
Clone reference layout. It keeps the design idea a private study identified
(ground bunkers dug into rugged highland, an exposed flag stand behind each
bunker guarded from above, a plasma battery on one flank, a neutral landmark on
the high middle ground between two valleys) and nothing else.

**Originality.** Every structure, the terrain and every texture are authored
here. No Stonehenge geometry, heights, textures, positions or spawn data are
copied, traced, resampled or fitted. The study (ignored
`research/stonehenge-study/`) was used only for statistics and design intent:
slope distribution, distances and what each base element is for.

**Status.** v2 is embedded in source from `src/assets/maps/cairnhold/` and holds
the `stonehenge-clone` slot (`MapId::StonehengeClone`, menu name "Cairnhold"),
always listed, with no private pack. Gameplay compatibility is `maps5`, which
includes its fingerprint. Not deployed: the live server still runs the old
Stonehenge clone. The v1 pack is kept in ignored `local-assets/cairnhold-v1/`.

**v2 base pass.** The bunker front is a stepped, crenellated gatehouse facade
(gatehouse 17 m, middle tiers 14 m, outer tiers 11.5 m over the 0 m floor) with a
recessed, bronze-lined portal under a lit lintel, a tall team banner with the
cairn emblem, and stone wing blocks retaining the hillside. The roof turret
now stands on the gatehouse. The flag sits on a real tower (platform 41 m, 13.5 m
over the knoll) rising from the guard hut, reached inside by the hut ramp, two
tower ramps and a roof hatch, and from outside by an attack ramp up the knoll to
a bridge onto the platform. The trench roof now climbs from the bunker roof to
the knoll with the ground held flush 10 m to either side, stone kerbs and light
slots on top. Turrets, the battery and the sensor have render-only stone and
bronze collars.

## Layout

180-degree rotational symmetry about the map centre (1024, 1024). Red at low Z
faces +Z; blue is red rotated.

| Element | Where (red, local; bunker floor = 0) | Notes |
| --- | --- | --- |
| Bunker | 32 x 40 m, floor 191 m world | Dug into the knoll's forward slope; roof flush with the hill behind |
| Facade | front 7 m of the roof, wings to x ±24 | Stepped tiers 17 / 14 / 11.5 m, crenellations, buttresses; gatehouse projects 2 m round a 7.4 x 6.2 m portal |
| Front vestibule + baffle | baffle 4 m inside the door, x ±10.5 | Blocks every turret/battery line through the portal and door |
| Hall, inventory room (2), back room | behind the baffle | Bronze-framed doors; no spawns in the hall (v3) |
| Vault (v3) | under the hall: x 3.2–15.2, z −9.8–12, floor −6, ceiling −1 | Stone piers, bronze ribs. Exactly two ways in: a 29.3° stair from the hall's east side (railed opening, x 11.2–15.2, z −9 to −2) and the tunnel door (6.4 x 4.5 m) in its east wall |
| Generator well (v3) | x 3.2–10.4, z 3.4–12, floor −8.2 | The kit generator is 5.8 m tall and its hit bar hangs 6.4 m over the floor, so it stands in a well reached by a 28.8° ramp; a walkway runs from the stair foot to the tunnel door along its east side |
| Sally port (v3) | east x 16–88 (z 4–12), then south z 4 → −36 (x 80–88) | 6.4 m wide, 4.5 m headroom, sconces every 8 m. Floor climbs −6 → 3 (20.6°), runs level, then 3 → 8 (17.4°). Its stone lid is flush with the hillside it runs under |
| Exit house (v3) | x 80–88, z −44 to −36, on the battery bench | Door in its east wall behind a dog-leg vestibule (baffle 1.8 m inside, joined to the wall at its north end), at the foot of the battery ramp |
| Roof turret (bullet) | top of the gatehouse, 17 m | Solid gatehouse mass below it |
| Covered trench | x 0–8, z 20–76, floor climbs 22 m (21.4°) | Roof 7 → 27.5 m; ground flush with it 10 m to each side |
| Guard hut | 24 x 24 m, floor +22 | Dug into the knoll (ground 27.5); its roof is a terrace round the tower |
| Flag tower | 14 x 20 m on the hut's east half, platform +41 | Hut ramp → floor 29 → ramp → landing 35 → ramp → roof hatch; crenellated, buttressed, bronze ring |
| Attack ramp + bridge | x −5–0, z 106 → 79 (26.6°) | From the knoll behind the hut to a bridge onto the platform's west edge |
| Sentry mast (bullet) + sensor | on the tower platform | Sentry about 10 m over the platform |
| Plasma battery | right flank (+X), bench +8 m | Octagonal bastion, parapet, ramp toward home |
| Landing/deploy ledge | left flank (−X), deck +5 m | 24 m paved deck, four marked `deploy_slots` |
| The Ring (neutral) | map centre, 240 m | Octagonal dais, four axis ramps, eight bronze-seamed pylons; landmark and cover only |

Flags are 532 m apart. Each team has eight `spawn_points` (two in the
inventory room, two in the back room, one in the guard hut, one on the battery
deck, two on the ledge), every one 1.2 m above its floor. None is in the hall,
vault or tunnel; the nearest is 18 m on foot from the stair head
(`test_no_spawn_camps_the_generator`).

Terrain sites are blended in order bunker (with the wings), apron, hut, ramp
foot, then trench last, so the trench banks stay exactly flush with its roof.

Dug-in structures cut terrain cells (102 holes, including the sally port's
cells). Their outer walls lie exactly on
the 8 m cell lines, and every boundary vertex is pinned to a height its walls
cover, so there is no gap between ground and wall. Other structures sit on
flattened benches that reach at least one cell beyond them.

## Terrain

`scripts/assets/cairnhold_terrain.py`: an authored axis profile (knoll 226,
valley 186, mesa 246 m) plus point-symmetric ridged and rolling detail from
PeakRunner's own hash noise (shared with Tower Complex), damped along the flag
axis so the knoll → valley → mesa sequence reads.

| Play-area terrain cells | Cairnhold | Private study |
| --- | --- | --- |
| Median slope | 29.7° | 31.1° |
| 90th percentile | 43.6° | 50.7° |
| Under 3° | 2.7% | 0.8% |
| Over 30° | 49% | 53% |

Along the red flag axis: knoll about 217 m, bunker front 191 m, valley 181 m,
mesa 240 m. The mesa is higher than the knolls, unlike the reference.

## Build and test

From `src/` with the numpy venv:

```sh
../research/local-assets/tools/venv/bin/python scripts/build-cairnhold.py [OUTPUT] [--no-bake]
../research/local-assets/tools/venv/bin/python scripts/test-cairnhold.py
```

Default output is the embedded `assets/maps/cairnhold`, and the build refuses to overwrite.
It bakes lightmaps (10 pages, about 25 s). Baked builds are byte-identical.
Pack assembly is shared with Tower Complex in `scripts/assets/pack_writer.py`,
and Tower Complex still builds byte-identical to its committed pack. Building
helpers live in `scripts/assets/structure_kit.py`.

`test-cairnhold.py` (24 tests) covers:
- floors and headroom, the trench, the hut ramp and both tower ramps
- the facade: stepped tiers, an open portal through to the baffle, the 2 m recess
- the attack ramp (slope, open sky, closed underside) and the bridge onto the platform
- no fall-through anywhere over a dug-in footprint, the trench banks flush with its roof,
  and the wing blocks and ramp foot meeting the ground
- the standing-support floor ray, no accidental slopes, no overlapping floor plates
- spawn validity, the flag plinth, transform invariance and the collision budget
- unique equipment IDs and circuits, and deterministic, distinct materials
- a whole-map turret/battery sightline check into every room
- terrain symmetry, slope bounds and the forward-slope layout
- holes and wall coverage of the pinned boundary, ground under decks and the Ring
- Ring symmetry, bake determinism and byte-identical rebuilds

Collision: 2,336 triangles per base (budget then 3,000; now 4,500, see `map-pipeline.md`), 256 for the Ring; about 6,250 render triangles per base.

## Validation done, and gaps

- **Rendering:** checked in a real local match through a temporary pack
  redirect into the `stonehenge-clone` slot (scratch copy with
  `private_reference` added, since removed). Screenshots are in
  `research/screenshots/cairnhold-v1-*`: aerial, Ring, bunker exterior and
  bunker interior. v2 was captured the same way: `research/screenshots/cairnhold-v2-*`
  (aerial, Ring, bunker exterior, bunker interior, flag tower and trench,
  battery, slopes, a battery-deck spawn view). The v1 camera positions were not
  saved, so the v1/v2 pairs are matched by eye, not exactly.
- **Rust acceptance tests:** `cairnhold_is_embedded_with_grounded_spawns_and_flag_decks`
  (terrain.rs) and `cairnhold_turrets_cannot_see_into_base_rooms` (sim.rs), which
  runs every turret through `equipment::acquire_target` at sensor range against
  standable points in the bunker, trench, hut and tower floors. Tolerance: the
  entry vestibule in front of each baffle and the columns under the open roof hatches.
- **Not done:** human traversal, ski feel, balance, bot pathing, audio, and
  Windows/Linux runtime.
- **Known gaps:** kit turret heads and the sensor are still shared placeholders
  inside render-only collars. The flag is the engine's. There is no bloom, so
  "glow" is a bright texture. The Ring has no mechanic yet. The attack ramp's
  closed side reads as a large plain stone wedge from the west. The interior is
  unchanged from v1. Nobody has walked or skied the new routes yet.
- **v3 vault and sally port:** 3,846 collision triangles per base (budget
  4,500). Python tests cover the vault's two ways in, the stair, the well, the
  whole tunnel (floor, headroom, walls, lids flush with the pinned ground) and
  the exit vestibule (the battery's barrel sees nothing past it; nobody on the
  bench sees up the tunnel). Rust tests walk the full body from the hall down
  the stair, through the vault, the length of the tunnel and out onto the
  battery bench, check the generators sit in cut cells 8.2 m down, and keep
  every turret's sightlines out of the vault, well, walkway, tunnel and exit
  house. The tunnel is 112 m long, so it is a defenders' and infiltrators'
  route, not a fast one; whether the stair + tunnel pair is too easy to hold
  needs a playtest. Bots don't use either.

## Open doorways (2026-09-24)

Supersedes the front-vestibule and exit-house dog-leg rows above. The bunker's
front door opens straight into the hall behind its recessed portal, and the
exit house's door opens straight in. Turrets and the battery face the field
with a 200 degree field of fire beyond 15 m, so neither door gives a turret a
line into the rooms. The route and sightline tests use the new rule.
