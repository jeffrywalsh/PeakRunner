# Offline bots

Bots appear only in offline matches. The server has no bots. All bot state on `Player` is `#[serde(skip)]`, so snapshots and the protocol are unchanged and no compatibility bump is needed.

## Route-following (`crates/core/src/bot_nav.rs`)

When a map is first played, a navigation graph is built from its collision triangles and terrain. On native builds this runs in a background thread and takes 1–5 s. Until the graph is ready, and on the web build, bots steer straight at their goals.

**Waypoints**
- Terrain waypoints sit on a 16 m grid.
- Floor waypoints are sampled on two interleaved 3 m lattices over walkable triangles.
- A floor waypoint is rejected if it is buried, sits under a slab, lacks headroom, or is pressed into a wall.

**Links**
- **Ski:** between terrain neighbours.
- **Walk:** between floor neighbours, and hops over steps and plinths.
- **Floor-to-terrain:** doorway probes.
- **Jet:** simulated with the real jet constants on 85% of a tank, including run-in speed from downhill terrain. Each arc is checked for clearance.
- **Drop:** falls of up to 160 m. Landing damage is capped.

**Pathfinding**
- Isolated pockets of fewer than 30 nodes are never snapped to.
- A* is limited to 40k nodes, and at most two plans run per tick.
- A stuck bot bans the link it failed on and replans. If no route remains, it forgets the bans.
- A route starts from a waypoint the bot can reach in a straight line.
- Bots save energy on the approach to a jet link and wait for a tank before launching.
- During a jet, a bot keeps thrusting until a coast would carry it over the target.

## Personalities

| Archetype | Aim error | Reaction | Style / role |
| --- | --- | --- | --- |
| Rookie | 0.11 rad | 0.75 s | Ground |
| Grunt | default | default | Ground |
| Rider | — | — | Skier |
| Skirmisher | — | — | Jetter, hunter |
| Anchor | — | — | Defense |
| Ace | 0.025 rad | 0.2 s | Jetter, hunter |

Profiles also set lead, aggression (chase range 20–80 m), retreat health, kit use and route weights per link type.

**Difficulty** is chosen in the offline menu's BOTS row and saved as `bot_difficulty` in `client.json`. The options are Easy, Normal (the default), Hard and Mixed. An unknown value reads as Normal.

The line-of-sight and reaction rules are unchanged. A bot fires only at a target it can see, after its reaction time has passed.

## Tests

- `nav_graph_connects_every_spawn_to_the_enemy_flag`
- `a_lone_bot_reaches_the_enemy_flag_on_every_map`: both teams on all five maps, within 240 s.
- `difficulty_picks_are_deterministic_and_bounded`
- Soak report (ignored by default):
  `cargo test --release -p peakrunner-core --lib bot_navigation_soak_report -- --ignored --nocapture`

## Known limits

Two cases are slow but do not fail:
- On Old Holler, a lone Glacier bot takes about 165 s: it reaches the enemy base in about 60 s, then struggles with the interior stairs and a jet under a slab.
- On Tower Complex, the long climbs into the floating bases depend on skiing speed before launch.
