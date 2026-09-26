# Football and body checks

Football is PeakRunner's third mode, an original take on the classic community
football mods: no weapons, one ball, passes, fumbles, tackles and touchdowns.
Its maps are **Longfield**, a floodlit stadium, and **Highgoal**, a walled
arena whose goals stand on platforms you have to jump and jet onto. Body checks (player-to-player
collision, teammates included) come with it and apply in **every** mode.

Compatibility markers: `bump1` (collisions), `ball1` (the mode, its ball
and its armor), `maps8` (Longfield's and Highgoal's fingerprints join the
five CTF maps). Not deployed.

## Body checks (`crates/core/src/bump.rs`, all modes)

- Enemies collide as upright cylinders the size of the base game's light
  armor box (0.5 m each side of the centre, 2.3 m tall); centres within 2.3 m
  vertically touch, so a drop onto someone's head connects. Hits are swept
  across each physics step, so skiers closing at 100+ m/s can't pass through
  each other between ticks (they did before: a third to half the time at
  60–80 m/s).
  Teammates block each other too (as bodies did in the Tribes engine) but
  only push apart: no damage, no fumbles, no tackles. A player who is down
  still blocks.
- Overlapping bodies are pushed apart, never through a wall.
- Speed along the line of impact passes almost whole from one body to the other
  (restitution 0.8): a blocker stops a skier dead and is knocked away. This is
  how defenders body-block a flag carrier.
- Light damage outside Football, from each player's velocity change `dv`:
  `floor(min((dv - 8) * 0.45, 20))`. A walking bump does nothing, a running
  head-on collision about 5, a full-speed hit 20. A fatal check credits the other
  player ("Body check").
- Server and offline only; client prediction doesn't resolve hits against other
  players, so the server's result arrives in the next snapshot.

## Reference values from the base game

Read from the base Tribes scripts (private CrossOver install, statistics only):
light armor `jetForce 236`, `jetEnergyDrain 0.8`, `maxJetForwardVelocity 22`,
`jumpImpulse 75`, `mass 9`, `maxEnergy 60`, recharge 8/s; collision box
`boxWidth 0.5`, `boxDepth 0.5`, `boxNormalHeight 2.3` (medium 0.7 x 2.4, heavy
0.8 x 2.6); `respawnTime 2`, `warmupTime 20`. The football mod overrides the
armor to 400 / 4.0 / 250 / jump 50 and the server to 1 s respawns. The 1v1
football mod uses the same collision and pass code, ships no armor data, and
plays first to 10 touchdowns, win by 2.

## Football armor (after the classic mod's physics)

The classic football mod's light armor (studied privately) is `jetForce 400`
on `mass 9` against gravity 20, `jetEnergyDrain 4.0` per 32 ms engine tick
against a 60-energy tank, and recharge 8/s. That's a thrust of 44.4 m/s² that
empties the tank in about half a second, then 7.5 s to refill. Its jump is
`jumpImpulse 50` on mass 9, 5.56 m/s. Base Tribes light armor was a slow
2-second climb (force 236, drain 0.8); the mod deliberately made the jet a
short, hard burst.

`football::FOOTBALL_ARMOR` reproduces it in our movement code, in Football
only (`sim::armor_for(mode)`; CTF and Capture & Hold keep the approved
`STANDARD_ARMOR`, bit for bit):

| | Standard (CTF) | Football |
|---|---|---|
| Jet thrust (up) | 37.3 m/s², fading near 16–20 m/s climb | 44.4 m/s², no fade |
| Jet thrust (WASD) | 22 m/s² | 35.5 m/s² (0.8 of up) |
| Drain / recharge | 15/s / 12/s | 120/s / 8/s |
| Full tank lasts | 3.8 s | 0.48 s |
| Jump | 8.34 m/s | 5.56 m/s |

Measured in the sim (`football_jet_is_a_short_hard_burst`), the body centre
rises 0.7 m on a jump alone, 7.8 m on the jet alone, 15.8 m on a jump and jet
together, and 5.8 m on a jet at the top of a jump. Holding jet on an empty tank
flickers it as recharge trickles in, as the original did. Bot navigation plans
jet links on football maps with this armor.

## Football rules (`crates/core/src/football.rs`)

- **No weapons.** Fire passes while you carry the ball and does nothing
  otherwise.
- **Ctrl+K** (every mode) kills you for a quick respawn, as in Tribes; a
  carried ball or flag drops. One press, one death; the server applies it
  (`kill1`). Turrets and sensors stand idle. Repair kits still work.
- **The ball** appears at a kickoff spot on one team's side. Touch it to pick
  it up. The carrier walks at 72% speed; skiing and jets are unchanged.
- **Passing:** the receiver is the player under your crosshair, else the
  nearest within 0.15 rad of your facing (the classic selection, up to 8 km).
  As in the mod, the receiver only sets the power (`6.454 * sqrt(distance)`,
  uncapped, which lands about that far at a level look, so a pass can cross
  the field); the ball always goes along your look plus the classic 0.25 rad
  loft, and keeps none of your own velocity (the mod zeroed the thrower's
  velocity around its throw; `PASS_INHERIT` is 0). With nobody to aim at it's
  the classic 35 m/s throw. Steep throws lean forward: the elevation (look
  plus loft) passes unchanged up to 0.8 rad, then eases so a throw aimed
  straight up leaves at 65°, 25° forward along your facing
  (`throw_elevation`, `MAX_THROW_ELEVATION`). Leading a runner or lofting onto
  a ledge is your aim. An earlier automatic lead-arc (`lead_pass`) was removed to match the
  mod. Tests: `passes_reach_the_receiver_you_aim_at_near_and_across_the_field`,
  `throws_follow_the_mod_aim_only_no_carried_momentum`.
- **Skiing bleeds speed on the flat** in Football (`FOOTBALL_SKI_DRAG`, 1 m/s²,
  fading on slopes), so a runner has to keep boosting, as in Tribes football.
  CTF skiing is unchanged (the approved movement has no flat drag).
- **Catches** use the mod's wording: a ball nobody threw or dropped, or one
  moving under 1.1 m/s, is picked up; a moving ball last thrown or dropped by an
  enemy is intercepted; anything else is caught. Catching pushes you along the
  ball's flight (the mod's impulse, 2 x ball speed x 6.5 / 9 on mass 9: about
  0.16 x ball speed). The thrower can't re-catch for 0.5 s. Passes pick their
  power from players up to 8 km away, as the mod's sight check did.
- **Hits follow the classic mod exactly** (`Player::onCollision` in the 4.6
  tasermod and the 1v1 mod; enemies only):
  - The two bodies swap velocities outright.
  - Speeds are rounded to whole m/s. The slower player loses; on a tie both do.
  - A losing carrier fumbles when the closing speed is 2–14 m/s. The ball
    pops out 10 m/s sideways, and the fumbler can't re-catch it for 0.75 s
    (`FUMBLER_NO_CATCH`). Before this fix a fumble set no guard and the ball
    started inside the carrier's pickup reach, so the carrier took it straight
    back the next tick: knock-loose hits did nothing, and only tackles (which
    stun) cost the ball. Test:
    `a_fumbling_carrier_cannot_grab_the_ball_straight_back`. With real fumbles,
    Highgoal bots need longer to score, so that test now runs 10 minutes.
  - At 15 m/s or more the loser is **tackled**: the ball comes loose, 30
    damage, 2 s down (no movement, jets or throws) in the collapse pose with the
    camera orbiting 8 m out, getting up with an empty tank. The tackler scores 1
    when the loser carried the ball.
  - Nobody hits a player who is down. No tackles while no ball is in play
    (the tasermod rule; the 1v1 mod allowed them).
  - "Knee to the face" is emergent: diving onto someone is how you arrive
    faster than them.
- **Touchdown:** carry the ball into the enemy end zone's sphere. The team scores
  1 and the scorer 5 points. Everyone regroups at spawn after 6 s; a new ball
  appears on the conceding team's side after 10 s (the mod's timings).
- **Match:** two 15-minute halves (the tasermod server config), a 20 s warmup,
  1 s respawns. The score limit is 40 touchdowns (the missions' usual value),
  so games effectively run on time.
- **Half time** at 15:00: a 10 s break, everyone regroups, and the side that
  didn't start gets the ball.
- **Resets:** a ball that sits still for 20 s is reset at once, and one that
  leaves the field's bounds after 2 s. Either way it goes to the side opposite
  the last team to touch it, and everyone is sent back to spawn, as in the mod.
- **Out of bounds:** outside the field's bounds you lose 0.065 damage level
  (9.8% of light armor) each second, "YOU'RE OUT OF BOUNDS!", until you
  return or die. On Longfield that's the stands; on Highgoal the walls keep
  you in.
- Personal points use the frag counter (the HUD and roster say "pts").

## The field

A map offers Football only when its manifest declares a field:

```json
"football": {
  "end_zones": [[1024, 101.2, 899], [1024, 101.2, 1149]],
  "kickoff":   [[1024, 101.0, 986.5], [1024, 101.0, 1061.5]],
  "radius": 9,
  "bounds": [896, 799, 1152, 1249]
}
```

Index 0 is Ember's. End zones are the sphere centres a carrier (body centre
1.2 m above the turf) must enter. The rotation rejects `"mode":"football"` on a map
without a field, and the offline menu disables the button. Tests and QA without a
field fall back to the flag stands.

## Longfield (`src/assets/maps/longfield/`)

Built by `scripts/build-longfield.py` from `maps/longfield.json` and
`scripts/assets/longfield_{terrain,stadium,materials}.py`; tested by
`scripts/test-longfield.py`. Key `longfield`, `MapId::Longfield`.

- A dead-flat turf field 140 x 340 m with mown stripes. End zones 250 m apart,
  9 m spheres, painted team rings. Chalk sidelines, end lines, a line every 25 m,
  and a centre line and circle.
- A point-symmetric bowl: banks rise 42 m (median slope over 25°, never over
  60°), so you can ski down them to build speed for a tackle, to a level rim.
- On the rim: five-tier stands down both long sides and behind each end,
  back walls with team banners, lit canopies, two scoreboards and four
  floodlight masts. An open goal gate (pylons, crossbar, banner, lamp strip)
  stands behind each end zone; every approach from the field and the flanks is
  clear.
- 8 spawns per team behind their own end zone, facing up the field. No
  equipment, water or props. The terrain shade is baked directly.
- 1152 collision and 2290 render triangles, 5 lightmap pages, byte-identical
  rebuilds.
- Originality: the classic community stadiums in `~/Downloads/classic_football_maps`
  were parsed privately for statistics only (47 missions: end zones a median
  257 m apart, typical zone radius 6–10 m, walled flat fields). No geometry,
  positions, textures or names are used.

## Highgoal (`src/assets/maps/highgoal/`)

Built by `scripts/build-highgoal.py` from `maps/highgoal.json` and
`scripts/assets/highgoal_{terrain,arena,materials}.py`; tested by
`scripts/test-highgoal.py`. Key `highgoal`, `MapId::Highgoal`.

- An enclosed sand arena, 110 x 220 m, inside 32 m walls of carved
  medallion tiles, with lamp strips along the tops and team banners on the end walls.
- **Raised goals:** at each end, a glowing team-glass slab (15 x 8 m) on two
  tapered basalt pillars, its top `RAISED_GOAL_HEIGHT` = 7 m up; the end zone is
  a 4 m sphere on it. The jet alone (7.8 m) only just gets you up, launched
  right at the slab's edge; a jump and jet clears it from anywhere 4.6–10 m
  out (`the_jet_alone_only_just_reaches_the_raised_goal`). Climb straight up,
  let go of the jet once above the top, and drift onto it: jetting in at a run,
  the side thrust flings you over or under it.
  You can walk under the slab between the pillars; defenders can stand on it.
- 8 spawns per team between their goal and end wall. No equipment, water or
  props. 152 collision and 726 render triangles, 2 lightmap pages,
  byte-identical rebuilds.
- Bot carriers walk the last 12 m to a launch spot 6.5 m out without jetting,
  wait there for a full tank (open to tackles), then leap, cutting the jet once
  above the top
  (`bots_leap_onto_raised_goals_and_score_on_highgoal`).
- Originality: the arena idea follows the classic community arenas with raised
  goals; the geometry, textures and name are original.

## Client

- `src/football_hud.rs`: SCORE HERE / YOUR END ZONE labels (your own only
  within 80 m; rings drawn only on maps without painted ones), a ball or carrier
  marker, screen-edge arrows (to the ball, or to their end zone while you carry
  it), and a status line (YOU HAVE THE BALL · FIRE TO PASS, TACKLED — GETTING UP,
  NEXT BALL IN n).
- Announcements come from diffing state (TOUCHDOWN EMBER, HALF TIME, YOU HAVE THE
  BALL, X INTERCEPTS, TACKLED), so a replayed snapshot announces nothing. Plays
  are server feed entries (`feed::Entry::Play`), shown in gold as PLAY.
- The ball is a glowing amber ball with dark seams and a soft halo. A loose
  ball has a faint beacon column. Carriers hold it in front, and your own shows
  in first person. Nobody holds a weapon.
- **Stadium sound** (original synthesis, `src/sound.rs`): referee whistle (new
  ball), deep stadium horn (half time, resets), crowd cheer for your touchdown and
  groan for theirs, tackle thud, body-check bump, pass whoosh, catch slap. A
  crowd-murmur loop swells as a carrier nears an end zone. Online clients derive
  the same cues from snapshots (`football_hud::network_events`).

## Bots (offline)

Carriers run for the enemy end zone and pass to a visible teammate further up
the field when an enemy is within 12 m. Teammates run 15 m ahead of their
carrier to block. Everyone chases a loose ball. Within 5–14 m of an enemy
carrier a bot pounces: it jumps and jets only while its predicted height on
arrival is under about 1.8 m above them, then dives in, arriving faster than
them (12 tackles in the 4-minute Longfield bot match). Defenders hang back when the play is far from home.
`bots_carry_pass_tackle_and_score_on_longfield` checks a 4-minute bot match
has carries and touchdowns.

## QA

- Local stadium match: `QA_LOCAL=1 QA_MAP=longfield QA_MODE=football` (or
  `QA_MAP=highgoal`), plus
  `QA_CAPTURE_PATH` and optional `QA_FLYCAM`.
- `QA_MODE=football` on an offline start plays Football on any map, with end
  zones at the flag stands.
- Captures: `research/screenshots/longfield-v2-{spawn,sideline,aerial}.png`,
  `highgoal-v1-{goal,leap,aerial}.png`.

## Open items

- Human playtest: tackle speeds, carrier slowdown, pass feel, score target,
  and the football armor's burst (and whether the lighter jump should stay).
- More stadiums, and out-of-bounds damage for players (the classic mods had
  it; the ball resets but players don't).
- Field lines in the stands' evening shadow read dim.
