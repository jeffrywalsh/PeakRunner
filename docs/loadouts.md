# Loadouts, the repair tool, the mortar and deployables

Everything in this doc applies in CTF and Capture & Hold. Football has its own
armor and no weapons (`docs/football.md`). Compatibility markers: `loadout1`,
and `throw1` for grenades and mines.
Not deployed.

Base Tribes values used as references were read privately from the base game's
scripts in a local install (statistics only).

## Armor (`crates/core/src/loadout.rs`)

Two armors, chosen at an inventory station and kept across deaths.

| | Light | Heavy |
|---|---|---|
| Damage taken | 1x | 0.5x (twice the armor, base Tribes `maxDamage` 1.32 vs 0.66) |
| Walk speed | 1x (approved movement) | 0.55x (base Tribes run 5 vs 11) |
| Jet | approved standard (37.3 m/s², drain 15/s, recharge 12/s, adds speed to 72 km/h) | 23.5 m/s² (3.5 net of gravity), 8 m/s² sideways, adds speed only to 36 km/h, drain 11.3/s, recharge 6.5/s: hard to fly, barely climbs |
| Jump | standard | the same (base Tribes jump speed is equal) |
| Mass (shoves, blast kick) | 1x | 2x |
| Third weapon | grenade launcher | mortar |
| Ammo: disc / chaingun / third | 15 / 100 / 10 (base Tribes light) | 25 / 200 / 10 |

Heavy's jet barely out-pulls gravity, as base Tribes' harmor did (jetForce 385
on mass 18). It lifts you onto a ledge or slows a fall; it doesn't fly you
across the map. Skiing is heavy's real speed: momentum from a slope is kept
(`Armor::air_cap` only limits what the jet adds). Heavy armor is drawn bigger
all round.

## Ammunition

Every weapon has limited rounds (`Player::ammo`). Each shot spends one; an
empty weapon clicks (`dry`) and waits 0.35 s before trying again. Online
prediction spends rounds too, so an empty weapon never predicts a shot (it
used to cycle its reload animation). An empty disc launcher shows no disc in
its tray and a dark charge bar; an empty grenade launcher has no lit rounds. Rounds refill
on respawn, and while using your team's powered inventory station (hold E).
Bots restock just by reaching their station, and fall back to it when out of
disc and chaingun rounds.

**Death packs** (`World::drop_loot`, marker `loot2`): a dying player
drops a pack drawn from what they carried. Anyone who runs over it takes
what fits, and it lasts 20 s. The pack holds:

- 1–5 discs at random, never more than they had;
- all their chaingun rounds, so a picker fills up to their own max;
- 1–5 third-weapon rounds at random, never more than they had;
- 1–5 hand grenades: at least one, even if they had none, otherwise never
  more than they had;
- no mines and no railgun slugs.

Only matching kit is taken: grenade-launcher rounds go only to light armor,
and mortar shells only to heavy. Test:
`the_dead_drop_a_roll_of_what_they_carried_and_it_vanishes_after_20_s`.

## Mortar (heavy armor's third weapon)

A long green lob (65 m/s, half the thrower's velocity inherited, full
gravity): about 150 m at 20° elevation and 210 m at 45° (it was 66 m at 20°
at 42 m/s). It explodes on impact. Like the grenade launcher's shell it has a
launch safety: for its first 0.35 s (`MORTAR_ARM`) it bounces off whatever it
meets, so a short shot inside a building rattles round the room first; after
that the next contact sets it off. A shell that never lands goes off after
20 s. Blast: concentrated, `125 / (1 + (d / 0.6)²)` with d the distance to
the body: a near-direct hit (within ~0.25 m) kills light armor, 0.5 m off it
takes two (74), a metre off 33, two metres 12. The shock is intense: kick 5000
reaching 20 m, in green flames. Reload 2 s. `combat::MORTAR`, `Disc::kind` 5.
The grenade launcher's shell uses the same shape at 0.4 strength
(`GRENADE_SHARE`: 50 at the body, kick 2000). Against equipment and
deployables both keep their earlier linear splash (`structure_blast`) so the
shield time-to-break balance in `docs/weapon-damage.md` is unchanged. Any
blast on an airborne enemy kicks 1.5x (`AIR_HIT_KICK`): a mid-air disc throws
them hard; your own disc jumps are unchanged.

## Repair tool (hold Q; replaces the repair kit)

While held, a beam repairs the nearest friendly thing within 12 m under your
crosshair and in sight (a teammate, your team's equipment or deployables), or
yourself. It spends 10 energy per second (the base Tribes repair gun's rate)
and needs 3 to run. Rates: 6 health per second on players (a full heal takes
about 16 s), 20 hull per second on equipment and deployables. Your weapon is
lowered and can't fire while repairing. Wrecked equipment comes back online
past half its hull, as before. The old E-to-repair at equipment is gone; E
still uses stations. The beam's end travels in snapshots (`Player::repair_beam`)
for rendering and the repair hum. Commands still accept the old `kit` field.

## Inventory stations

At your team's powered inventory station, **press E** to open the inventory
screen (laid out like base Tribes' station menu: Armor, Weapons, Packs,
Miscellany). While it's open the mouse is free, and standing there heals and
recharges you and restocks rounds, grenades and mines. Click Take, or press:

| Key | Takes |
|---|---|
| 1 | Light armor (restocks for it) |
| 2 | Heavy armor (restocks for it) |
| 3 | Turret pack |
| 4 | Wall pack |
| 5 | Force field pack |
| 6 | Ammo station pack |

The screen shows each weapon's rounds, how many of each deployable your team
has out, and your grenades and mines. E or Esc closes it; stepping off the
station or dying closes it too. A pack is carried until deployed or you die.
Purchases are server-validated intents (`Command::buy`). There is no money.

## Rifles (`crates/core/src/rifles.rs`, marker `rifle1`)

Weapon slot 4 (key **4**), bought at an inventory station (the screen's
"Laser rifle" / "Railgun" row, or key 7 there), never carried by default,
lost on death. Which rifle you hold follows your armor:

| | Laser rifle (light only) | Railgun (heavy only) |
|---|---|---|
| Shot | a straight beam from the eye along the crosshair, instant, to 2.6 km (the whole map) | a slug at 640 m/s: straight, very fast, not instant |
| Cost | 24 energy per shot (you need 24 to fire) | 20 slugs, restocked at stations |
| Damage | 50 | 78 (a heavy target takes half) |
| Reload | 1.1 s | 1.6 s |
| Motion | none | keeps 15% of your velocity |

Both damage enemy players, equipment and deployables, and can be fired from
the hip.

- **Zoom:** hold **E** away from a station, as in Tribes. The view narrows to
  22° with a sight overlay, and the look slows to match. At a station, E
  still uses the station.
- **Seeing shots:** a laser shot reaches every client as a short-lived beam.
- **Checks:** the server refuses the rifle slot until a rifle is bought. The
  weapon command accepts slot 3.
- **Sounds:** new synthesized laser and railgun cues.

## Views and stations

- **R:** toggles a third-person view, with the camera behind and above your
  body and pulled in off walls. While placing a pack, R still turns it.
- **Inventory screen:** opens by itself when you step onto your inventory
  station. After you close it, E opens it again.

## Hand grenades and mines (`crates/core/src/throwables.rs`)

Hold **G** (grenade) or **M** (mine) to wind up, release to throw: a tap
throws at 0.3 of full strength, a one-second hold at full (base Tribes'
`throwStrength`, 0.3 + 0.7 x wind-up). A bar under the crosshair shows the
wind-up. One throw per half second (`throwTime`).

| | Hand grenade | Mine |
|---|---|---|
| Carried (light / heavy) | 5 / 8 | 3 / 3 |
| Full throw | 20 m/s | 24 m/s (base Tribes throws mines harder, 15 vs 9) |
| Goes off | 2 s after the throw | when an enemy comes within 2.5 m, once armed |
| Blast | 10 m, 76 peak (damageValue 0.5) | 10 m, 98 peak (damageValue 0.65) |

Both bounce dully (elasticity 0.15) and come to rest. A mine arms 1 s after
it settles (its light pulses in its team colour once armed). Teammates never
set it off, and team splash rules still apply, so your own mines can't hurt
teammates. An enemy blast within 4 m sets a mine off (a chain can follow). A
team can have 12 mines out (base Tribes allows 35 for bigger teams); a mine
that never settles within 8 s fizzles, and one lies out at most 15 minutes.
Respawns, inventory stations and ammo stations restock both. Kill feed:
"Grenade", "Mine". Bots don't throw them yet.

## Controls screen (`src/keybinds.rs`)

Main menu **Controls…** or the pause menu **Controls** lists every keyboard
action. Click one and press its new key; a key already in use swaps onto the
other action. Escape, Tab and Enter are reserved. Bindings save with the
client preferences (`client.json`, `keys`); unknown or bad entries fall back
to the defaults. The mouse (fire, jet) and arrow keys (movement) stay fixed.
Hints on the HUD and menu follow the bindings.

One-shot keys (deploy, suicide, throws, purchases) latch until a simulation
tick sends them (`Input::clear_once`), so a press on a frame that runs no tick
isn't lost, and one press acts once.

## Deployables (`crates/core/src/deploy.rs`)

Press **B** (was G, now the grenade) to bring up the deployer: it replaces
your weapon, and a hologram of the unit sits where your aim meets a surface
within 12 m (`DEPLOY_RANGE`). It's in your team's colour where it fits and
gray where it can't go (the reason shows under the crosshair), and vanishes
when nothing is in reach ("TOO FAR AWAY"). The wheel or **R** turns it 15° a
step; click places it; B, a weapon key or the repair tool puts the deployer
away. Client and server run the same check (`World::aim_placement`), and the
command carries the turn (`deploy_turn`), so what you see is what you get.

| | Turret | Wall | Force field |
|---|---|---|---|
| Per team at once | 4 | 6 | 4 |
| Hull | 150 | 400 | 250 |
| Shape | tripod post, 0.7 m radius | 4 x 3.2 x 0.36 m panel | 4 x 3.2 m panel between posts |
| Blocks | players, shots | everyone and every shot | only the other team and its shots |
| Does | chaingun rounds at enemies in sight, 70 m, 0.25 s | cover | a door your team walks through |

Placement needs flat ground where you aim (at any height in reach), room for
the unit and the whole panel, and keeps 12 m from flag stands, 6 m from spawns and 3 m from
other deployables. The server says why when it refuses ("NO ROOM THERE", "YOUR
TEAM HAS THE MAXIMUM OF THOSE DEPLOYED", ...). Deployables take bullet and
splash damage from the other team, can be repaired with the repair tool, and
blow up (harmlessly) at zero. Walls and fields block player movement and
projectiles; turret and repair sight lines see through your own fields.
Snapshots carry them whole (`Snapshot::deployables`).

## Controls added

Hold Q repair · E at a station opens the inventory · B deploy · G grenade ·
M mine · Ctrl+K respawn. All rebindable (Controls screen).

## Main menu and server rotations

- The main menu picks **mode first** (Capture the Flag, Capture & Hold,
  Football), then lists only maps that host it (`map_catalog::supports`,
  `maps_for`): stadiums host only Football; CTF maps host CTF, and Capture &
  Hold when they place at least two capture points.
- Server rotations accept playlists besides `{"map":...,"mode":...}` entries:
  `{"playlist":"ctf_cnh"}` (every CTF map in CTF, then every Capture & Hold map
  in Capture & Hold), `{"playlist":"football"}`, `"ctf"`, `"capture_and_hold"`.
  They mix with explicit entries (up to 64 matches). The VPS compose now uses
  `[{"playlist":"ctf_cnh"}]`; a football server uses `[{"playlist":"football"}]`.
- Explicit entries are checked the same way: CTF on a stadium, Football
  without a field, or Capture & Hold without two points are refused at startup.
- The server's status label now includes the mode ("Old Holler - CTF",
  "Longfield - Football"); the website shows it.

## QA

`QA_DEPLOY="turret:0:x,y,z,yaw;wall:1:...;field:0:..."`, `QA_HEAVY=Name,me`,
`QA_MORTAR=x,y,z`, `QA_MORTAR_BLAST=x,y,z,age`, `QA_KIT=heal` (the local repair beam),
`QA_THROWN="mine:0:x,y,z;grenade:1:x,y,z"` (resting, mines armed), `QA_SHOP=1`
(inventory screen), `QA_EMPTY=1` (empty weapons), `QA_CONTROLS=1` (controls
screen over the menu). Captures:
`research/screenshots/loadout-v1-deployables.png`, `menu-v2.png`,
`throw-v1-mines.png`, `throw-v1-shop.png`, `throw-v1-empty-disc.png`,
`throw-v1-controls.png`, `mortar-v2-8.png`, `mortar-v2-blast-0.25.png`.
Rifles: `QA_RIFLE=1` (or `=heavy`) buys and raises the rifle through the
server at a station, and `QA_FIRE=1` holds the trigger. The GPU capture test
`render_weapon_captures` writes `weapons-after-vm-{laser,railgun}-{idle,firing}.png`.

## Open items

- Bots don't buy heavy armor or packs, or deploy; they repair themselves and
  restock. In a soak run they fired about half as often (ammo limits and
  trips to restock).
- No first-person repair tool model yet (the weapon lowers and the beam shows).
- Deployables have no hit bars yet; they dull as they're damaged.
- Bots don't throw grenades or lay mines.
- Human playtest: armor balance, ammo counts, mortar power, deploy limits,
  throw distances and mine limits.
