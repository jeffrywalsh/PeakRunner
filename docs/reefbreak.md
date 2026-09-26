# Reefbreak

An original CTF map (key `reefbreak`; also runs Capture & Hold and both
deathmatch modes): a tropical atoll.

The layout idea follows a privately studied reference, Tribes: Ascend's
Crossfire. That study recorded statistics only, in the ignored
`research/crossfire-study/NOTES.md`. No heights, geometry, positions,
textures, names or audio were copied (`docs/map-pipeline.md` rule 0), and
everything here is our own shapes at our own scale.

## Layout (world metres; flag axis Z, Ember at low Z; point-symmetric about the centre)

- **Reef ring:** radius 300 m, a broad, low crest about 8 m over the sea.
  - Four shallow channels, at 55°, 125°, 235° and 305° from the flag axis.
  - The **flags** stand on pads on the crest at z = 724 and 1324, 600 m apart.
- **Central island:** radius 135 m, about 26 m high, with the **lighthouse**.
  - A railed gallery at 24 m is the sniping spot, reached by jetting.
  - A keeper's yard wall with two gaps gives cover.
  - Sand spits run east and west to the ring, carrying the two Capture &
    Hold points: East Spit and West Spit.
- **Water:** one sea volume covers the map, with the surface at 60 m.
  - The lagoon and the shelf out to the islands are ankle-deep: skiable,
    with a little water drag.
  - Blue holes and the open sea beyond radius 640 m are deep.
- **Outer islands:** a ring of them, including a C-shaped pair on the
  flanks.
- **Boundary:** sea cliffs from radius 700 m.

## Bases (`scripts/assets/reefbreak_base.py`)

Each team has a freighter that **hovers 14 m over the sea**
(`HOVER`), above a sandbar outside the reef behind its flag.

- **Under the ship:** there are about 10 m of clearance under the keel, so
  you can ski beneath it. The keel carries a fin and four glowing
  thrusters.
- **Hold:** eight spawns and two inventory stations. Four ways in:
  - the port and starboard hatches, each with a landing ledge;
  - a torn breach in the port bow;
  - the deck hatch ramp.
- **Engine room:** holds the generator, with exactly two ways in: the
  bulkhead door from the hold and the stern hatch, which has a ledge.
- **Top deck:** reached by the **bow gangway**, a 17° ramp from the flag pad
  up to the bow. It's the walking route, so heavy armor gets in too.
  - A bridge deckhouse holds the third station.
  - The roof carries the turret and the sensor mast, reached by an outside
    ramp.
- **Reef beacon:** beside the flag, with the second turret.
- **Ground outpost:** on the outer island behind the ship
  (`build_outpost`): a bunker with front and side doors, two stations and
  four more spawns.

## Build and test (from `src/`, numpy venv)

- Build with `scripts/build-reefbreak.py [OUTPUT] [--no-bake]`.
  - The default output is the embedded `assets/maps/reefbreak`, and the
    script refuses to overwrite.
  - Builds are byte-identical. That was verified by rebuilding and
    comparing every file.
- Test with `scripts/test-reefbreak.py` (10 tests; set `REEFBREAK_PACK`
  to test a built pack):
  - point symmetry;
  - how much water is wadeable against how much is dry land;
  - channels off the flag axis;
  - flags 600 m apart;
  - ships floating clear;
  - spawns 1.2 m over floors;
  - the generator room's two ways in;
  - stations, turrets and sensors per team;
  - the sea volume;
  - the gangway slope.
- Props use the `tower-complex` theme (outcrops, rock clusters, wreck
  debris), so `props.py` is unchanged.

## Verified

- All workspace tests pass with Reefbreak embedded.
- The compatibility marker is `maps10`.
- The connected rotation test passes.
- Captures checked visually: GPU captures of the unbaked pack, and a real
  local match on the embedded baked pack (the spawn in the hold, the
  floating ship, the overview).

## Pending

- Human traversal and balance.
- Bot flag play. A headless report (`bot_flag_play_report`, ignored) shows
  no flag grabs on any map, Reefbreak included. Bots leave the ships and
  fight.
