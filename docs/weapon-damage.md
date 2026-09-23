# Disc and grenade damage reference

Research and tuning: 2026-09-19. This is an original implementation of numeric
gameplay rules, not imported Tribes code or assets.

## Primary evidence

Read the user's installed T2 archive directly (without executing its scripts):

`~/Library/Application Support/CrossOver/Bottles/winxp-dos/drive_c/Dynamix/Tribes2/GameData/base/scripts.vl2`

The corresponding loose scripts under `GameData/Classic/scripts/` were also
checked. Both installed rulesets agree on the damage/radius values below and
the light-armour damage multipliers. They differ on several flight and impulse
settings; those are not part of this damage-only adjustment.

| Script / definition | Verified value |
| --- | --- |
| `weapons/disc.cs`, `DiscProjectile` | No separate direct damage; 0.50 splash; 7.5 m radius |
| `weapons/grenadeLauncher.cs`, `BasicGrenade` | No separate direct damage; 0.40 splash; 15 m radius |
| `player.cs`, `LightMaleHumanArmor` | Maximum damage capacity 0.66 |
| `damageTypes.cs`, `LightPlayerDamageProfile` | Disc multiplier 1.0; grenade multiplier 1.2 |
| `projectiles.cs`, `RadiusExplosion` | Linear damage falloff retaining 12% at the radius; coverage gates damage |

Additional online cross-checks (not substitutes for the installed scripts):

- [Projectile datablocks reproduced in the T2 modding handbook](https://modding.tribes2wiki.com/03-content-recipes/projectiles)
- [Damage, coverage and impulse reference](https://modding.tribes2wiki.com/03-content-recipes/damage-and-typemasks)
- [Community spinfusor comparison across Base and Classic](https://tribes2wiki.com/weapons/spinfusor/)

The key distinction: the grenade has twice the radius, not a larger raw peak
than the disc. The light-armour multiplier brings its player damage close to
the disc. Reading 0.50 as 50% health would miss T2's 0.66 light-armour capacity.

## PeakRunner mapping

Use the existing single, fast player class as the light-armour baseline, with
100 health representing T2's 0.66. Do not introduce armour classes in this pass.

| Weapon | Previous theoretical peak | New peak vs player | Radius, previous → new | Half-radius damage | Edge damage |
| --- | ---: | ---: | --- | ---: | ---: |
| Disc | 64 | 75.76 | 7.5 → 7.5 m | 42.42 | 9.09 |
| Grenade launcher | 80 | 72.73 | 9 → 15 m | 40.73 | 8.73 |

Disc peak = `100 × 0.50 / 0.66`.
Grenade peak = `100 × 0.40 × 1.2 / 0.66`.
Within the radius, damage = `peak × (1 − 0.88 × distance / radius)`.
Outside it, damage is zero. The discontinuity at the boundary is deliberate
T2 behavior, replacing PeakRunner's previous quadratic falloff to zero.

A confirmed direct impact applies the peak **once as splash**, not direct plus
splash damage. Both weapons kill an unhealed full-health enemy in two direct
hits. Near misses are weaker and do not automatically become two-hit kills.

For other players, distance is to the surface of PeakRunner's vertical body
capsule, instead of the old point near the feet. This is our geometry adaptation,
not a claim that T2's engine uses an identical capsule. Equipment distance uses
its existing collision sphere. Equipment gets the same normalized blast curve;
its custom health totals and lack of T2 material multipliers are unchanged.

Solid cover still blocks damage and knockback through the existing line-of-sight
test. This is binary cover, not a reproduction of T2's partial-coverage routine.

## Deliberate differences retained

- Existing 40% self-damage multiplier and friendly-fire-off rules remain.
  These are PeakRunner choices, not claims about unmodified T2 self-damage.
- Approved skiing, gravity, steering and jetpack settings remain unchanged.
- Blast impulses are now 2000 for discs and 1500 for grenades: maximum velocity
  changes of 22.22 and 16.67 m/s at PeakRunner's 90-unit player mass. Grenades
  previously pushed only 6 m/s. These are our selected combat tuning values.
  Both decline linearly to zero at the radius using body contact distance, push
  away from the impact, and release ground contact for an upward blast jump.
  Self-damage remains reduced, but self-knockback is not reduced.
- Weapon reloads, projectile speeds, inheritance, grenade arming/bounce/fuse
  behavior, and smoke trails are unchanged. This is not a full T2 weapon port.
- Main turret plasma remains 55 direct damage, 6 m splash, slight knockback.
- Walls: the viewmodel muzzle sits up to ~1.5 m (more at wide FOV) ahead of the
  eye. Player and bot shots now start no farther than the first surface on the
  eye-to-muzzle segment, so a wall-hugging shot hits the wall on the shooter's
  side instead of spawning outside (`muzzle1` compatibility, source only).
- Turrets and sensors acquire a target only with a clear segment from the muzzle
  (radius + 0.6 m out) to the target's chest, re-checked every tick, against
  terrain, pillars and the map collision mesh; the shot is re-checked before
  firing and bolts collide with the same geometry. Aligned open doorways are real
  sight lines: fix those in map layout, not by blinding turrets.
- No Ascend damage numbers are mixed into this light-armour conversion.

Tuning lives in `crates/core/src/combat.rs`; the shared authoritative explosion
path is in `sim.rs`. The gameplay compatibility marker includes `blast3:chat1`, so
old clients/servers cannot silently play together with different damage rules.
The directory protocol is unchanged. Live deployments require matching builds.

## Turret targeting

Every sensor and turret uses one rule, `equipment::acquire_target` in
`crates/core/src/equipment.rs`. It picks the nearest living enemy within range
whose chest (`CHEST_HEIGHT` 0.8 m) is visible from the barrel start, which is
body radius + `MUZZLE_GAP` 0.6 m toward the target. Leading weapons aim at the
constant-velocity intercept point. Before firing, the caller re-checks the line
from the muzzle to the aim point. Line of sight is a caller-supplied closure;
`step_equipment` passes terrain, pillars and the map collision mesh.

Numbers live in one table, `equipment::profile(kind, weapon)`:

| Profile | Range | With powered sensor | Speed | Life | Cooldown | Leads |
| --- | --- | --- | --- | --- | --- | --- |
| Sensor (detects only) | 260 m | 260 m | — | — | — | no |
| Bullet turret | 80 m | 150 m | 420 m/s | 1 s | 0.22 s | no |
| Plasma turret | 80 m | 150 m | 80 m/s | 3 s | 1.2 s | yes |

The table reproduces the numbers the inline code used; a test compares the old
rule with the shared one across Tower Complex, Skybreak and Raindance. Future
player-placed turrets should call the same function with their own profile row.
They will also need placed objects added to the line-of-sight closure, so a
deployed shield or turret blocks sight, and placement validation (clearance, no
placing through walls or with a line straight into an enemy room). Neither
exists yet.

## Verification

Regression tests cover exact peaks, half-radius and edge damage, outside-radius
immunity, monotonic falloff, direct-hit single application, two-hit kills,
multiple splash victims, wider grenade reach, building cover, equipment damage,
self/friendly damage, and preservation of the custom plasma behavior.
