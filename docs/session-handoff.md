# Session handoff — map remake and gameplay overhaul (2026-09-21 → 2026-09-24)

Read this first when resuming. It records where the project stands, how every
system added in this session works, how it is tested and verified, how the
agent workflow was run, what the user prefers, and what is left. AGENTS.md
remains the project contract; this document is the working memory of the
session that followed the `0.1.0-private.20260921.1` release.

---

## 1. Resume checklist

1. `cd ~/workspace/peakrunner && git status && git log --oneline -5`
   Expect a clean tree on `cleanup/clipping-textures`, level with `main`.
2. `git fetch origin && git log --oneline origin/main -1` — `main` is pushed
   after every verified batch; local `main` is moved with
   `git fetch . cleanup/clipping-textures:main` (fast-forward only), never by
   switching branches while agents run.
3. `git worktree list` — should show only the main checkout. Remove stray
   temporary worktrees (`../peakrunner-capture`, `../peakrunner-survey` were
   used and removed).
4. Check for orphaned processes before starting captures:
   `ps -eo pid,etime,command | rg "launch_smoke|capture.sh|until \[|cargo test"`.
   Agents occasionally leave a capture script or a `launch_smoke` window
   running for hours. The auto-mode classifier blocks the assistant from
   `kill`; ask the user to run `! kill <pids>`.
5. Re-read §9 (user preferences) before any design work, and §11 (backlog).
6. Nothing in this session is deployed (§12).

---

## 2. Repository and branch state

- Repository: `~/workspace/peakrunner`, remote `github.com:jeffrywalsh/PeakRunner`.
  Build workspace is `src/`; run cargo, scripts and Python from there.
- Branches:
  - `main` — pushed; every commit of this session lands here via fast-forward.
  - `cleanup/clipping-textures` — the working branch; equal to `main` after
    each batch.
  - `map/reference-layouts` — fully merged into `main`; kept, not deleted
    (deleting branches needs the user's request).
- The live game is still `0.1.0-private.20260921.1` (launcher `0.1.0-r1`,
  feed `2026092102`). The VPS and published clients run the old six-map clone
  rotation. See §12.

### Commits of this session (oldest → newest, all on `main`)

| Commit | What |
|---|---|
| 9b50810 | Tower Complex replaces Broadside Clone (embedded original map) |
| 949e9aa / 38a1e10 | Cairnhold build, then embedded in the `stonehenge-clone` slot |
| 0a51b56 | Skybreak removed (derived from Broadside measurements); `docs/map-pipeline.md` |
| 1359b2a / 8b87136 | Frostline and Dustreach builds, embedded; no private packs remain |
| 39179dc | Raindance cleanup via the pipeline |
| e4d142d | Blocked-spawn fixes, turret/flag models, per-map fog colour |
| 1082d74 | Equipment shields, hit bars, name tags, Dustreach underground |
| cc63137, 29c3298, 1961656, bcb082d | Underground generator rooms (Dustreach, Cairnhold, Frostline, Tower Complex), Frostline cavern |
| 7f95679 | Embedded packs compressed in the binary (93 → 29 MB) |
| ce90ca9 | Raindance displayed as **Old Holler** (key stays `raindance`), basement |
| 5713ef1 | Enemy arrows, flag carrier marker and announcements, exploding generators, repair kits |
| b7487ed | LAN directory accepts every map label |
| 0ffff1b | Weapon models, animation, effects and the smoke pipeline |
| 5f4f2eb / cbea4e8 | Dustreach cross-map sewer; Old Holler bishop flag tower |
| 07e2bd1 | Sound overhaul, articulated player models |
| 25506e1 | Open doorways, `arc1` turret arcs, bot line of sight, Capture & Hold, renderer upgrade, deeper sound |
| aebec56 / 14869e9 | Sound tuned against reference recordings; single-hit chaingun/steps, short explosions |
| d7cd3be | Tower Complex and Old Holler reworks, C&H towers |
| 1e47ff9 / 0d5ed33 | Cairnhold/Frostline/Dustreach passes; bot navigation and personalities |
| 6ec9151 | Menu text no longer references other games |
| f8eed55 | Old Holler flag routes 7 → 11–12; z-fight checker fix |
| cf6cca7 | Props and baked terrain shadows |
| fbfb4cb / f028478 | Ground cover and blocking cover; water physics, bloom, flyer bots, landing/switch animations |
| a7c8345 | Real water on every map |
| 4d66b9d | GPU instancing for grass and small props (binary 59 → 36 MB) |
| c921216 / d6d1c3f | Tunnel lids excluded from terrain shadow bake; taller Tower Complex summits |

---

## 3. The five maps

All are original, embedded in the binary (`src/assets/maps/<id>/`), built by
Python from `src/maps/<id>.json` + `src/scripts/build-<id>.py` +
`src/scripts/assets/<id>_*.py`, and tested by `src/scripts/test-<id>.py`.
Keys kept for rotation compatibility:

| Key (MapId) | Display name | Character | Doc |
|---|---|---|---|
| `raindance` (Raindance) | Old Holler | Rainy highland hollows, flooded flowing ravine (bridge/jet/swim), sunken halls over basements, flags in bishop-shaped towers with four doors plus a mitre slit | `docs/original-map-kit.md` |
| `broadside-clone` (BroadsideClone) | Tower Complex | Floating tower bases over rolling eroded hills with eight tall summits; two-storey atrium, keel generator room, turret pods on bridges, armory, landing pads, tarns | `docs/tower-complex.md` |
| `stonehenge-clone` (StonehengeClone) | Cairnhold | Hillside stone bunkers, two-level hall with gallery, vault generator with sally-port tunnel and skylights, open flag tower, the Ring monument | `docs/cairnhold.md` |
| `snowblind-clone` (SnowblindClone) | Frostline | Polar stations on steep snow, basement generator, two-level void before the flag, ski cavern under the beacon ridge, CTF-active Beacon capture point with drain field, meltwater pools | `docs/frostline.md` |
| `desert-of-death-clone` (DesertOfDeathClone) | Dustreach | Sandstone citadels, cistern generator, storehouse hall with mezzanine, cross-map sewer with flank and midfield shafts, Sun Gate, oases | `docs/dustreach.md` |

Valley remains an internal test fixture. Skybreak Bastions is removed from
source; a `skybreak-bastions` rotation entry fails at startup with a clear
message. Reference studies (Broadside, Stonehenge, Snowblind, Desert of Death)
live only in ignored `research/*-study/` and were used for statistics and
design intent — never copied (§8).

---

## 4. Systems added this session

### Gameplay (shared sim, `crates/core`)
- **Spawns:** optional manifest `spawn_points` `[x,y,z,yaw]`, 8 per team, centre
  1.2 m above the floor (lower starts the support ray inside the slab →
  frictionless "slick" spawn). Server picks randomly, avoids enemies within 30 m
  and the last point.
- **Player shots** are clamped to the shooter's side of walls (`muzzle1`).
- **Turrets:** shared `equipment::acquire_target` (line of sight + per-type
  `profile()` range: sensor 260, bullet/plasma 80, 150 with a powered sensor);
  **`arc1`** field of fire — 200° facing the enemy side, all-round within 15 m.
- **Shields:** sensors 300, turrets 450; powered shields regenerate
  **continuously at 110/s** (one attacker can never break one; two can);
  bullets do half to shields; generator down → shields 0 and equipment offline.
- **Generators** explode (visual, no damage) and leave a wreck; anything
  destroyed stays offline until repaired past **50%**.
- **Repair kit:** Q, one per life, 60 armor over 2 s, refilled at inventories.
- **Capture & Hold** (`control.rs`, `docs/capture-and-hold.md`): points flip after
  10 s uncontested; contested pauses; empty decays at half rate. Drain field
  (default 60 m, 10 energy/s) hits only the holder's enemies (2.3 s of jet vs
  3.8 s). Mode `capture_and_hold`: 1 point/s per held point, first to 300.
  Configurable, not in default rotation. Three points per map; Frostline's
  Beacon also runs in CTF with the drain.
- **Water** (`water.rs`): manifest `water_volumes` (rect/polygon, surface,
  depth, flow, colour). Drag by immersion, skiers braked hard, jet costs 1.5×
  waist-deep, buoyancy, bullets/plasma fizzle, discs/grenades drag. Dry
  movement bit-identical (tested).
- **Bots** (`bot_nav.rs`, `docs/bots.md`): line of sight required to chase/aim/
  fire, 2.5 s last-seen memory; navigation graph built at map load (walk, hop,
  ski, jet, drop links); archetypes Rookie, Grunt, Rider, Skirmisher, Anchor,
  Ace, Hawk (flyer); difficulty Easy/Normal/Hard/Mixed (preference
  `bot_difficulty`). Bots are offline-only; no protocol impact.

### HUD and client
- World-space hit bars (shield strip + hull) above generators/turrets/sensors;
  OFFLINE/DESTROYED states.
- Name tags: teammates blue ≤150 m, enemies red ≤80 m, line of sight only.
- Red enemy arrows ≤250 m, line of sight only; flag-carrier marker visible to
  1,500 m through terrain; centred flag announcements (derived client-side,
  now also reach online players); generator announcements.
- Articulated player models (`player_model.rs`), held weapons, LOD beyond 80 m,
  landing squash and weapon-switch animation derived client-side.
- Weapons: rebuilt chaingun/grenade launcher, polished disc launcher,
  per-weapon third-person models, switch/bob/recoil, layered effects
  (`effects.rs`) with a darkening smoke pipeline.

### Rendering
- 4× MSAA (preference `antialiasing`), terrain detail layer, tone mapping,
  hemisphere ambient, sun disc, optional manifest `look` (sun, exposure,
  ambient, height fog, procedural sky; `crate::look`), per-map fog colour.
- **Bloom** (`bloom.wgsl`, preference `bloom` Off/Low/High, default Low): masked
  via scene alpha; only light materials, thrusters, flames, plasma, visors and
  the sun glow; HUD never glows.
- Baked lightmaps for structures (`lightmap_bake.py`); baked terrain sun
  shadow + AO in `shade.rg` (`terrain_shade.py`; tunnel lids are not casters).
- Props: `props.py`; render-only props instanced from `props.bin`
  (`vs_prop`); solid cover baked with collision.
- Embedded packs zlib-compressed by `crates/core/build.rs`; hashes are over
  uncompressed bytes.

### Sound (`sound.rs`, `audio.rs`, `docs/hud-audio.md`)
Original layered synthesis (band-limited oscillators, sub + body + texture,
saturation, FDN reverb, limiter), 32 one-shot voices + 6 loops, pan/distance
dulling, delayed far explosions, idle weapon hums, single-hit chaingun and
footsteps, short ski release, short deep explosions, water splash/wade.
Tuned against a **private** Tribes 2 reference extracted to
`research/audio-reference/` (never shipped). Custom WAV overrides: files in
`PEAKRUNNER_SOUND_DIR`, `sounds/` beside the executable, or `assets/sounds/`
(desktop only).

### Compatibility marker
`{PROTOCOL}:equipment5:blast3:chat2:names1:ping1:fov1:muzzle1:kit1:cnh1:arc1:water1:maps6:<raindance>:<tower-complex>:<cairnhold>:<frostline>:<dustreach>`
Each map fingerprint changes whenever its `map.json` changes. Bump a named
segment in the existing style whenever wire layout or authoritative sim
behaviour changes; client-only visuals and offline-only bot changes don't.

---

## 5. How testing is performed

Run everything from `src/`. Python uses the numpy venv:
`PY=../research/local-assets/tools/venv/bin/python`.

### Full verification checklist (run before every commit batch)
```sh
cargo test --workspace --lib
cargo check --workspace --all-targets
cargo check --target wasm32-unknown-unknown -p peakrunner --lib
node scripts/check-app-boundaries.mjs
cargo build --locked --release -p peakrunner --bin peakrunner   # report binary size
for m in original-map tower-complex raindance cairnhold frostline dustreach; do
  $PY scripts/test-$m.py >/dev/null 2>&1; echo "$m exit=$?"
done
```
- Python suites print colour codes; use exit codes, not `rg ^OK`.
- The six suites take several minutes each now; run them in the background
  (`run_in_background`) rather than a single 10-minute foreground call.
- **The connected rotation test** (ignored by default) cycles all 5 maps with
  connected clients, late join and empty reset; run it for anything touching
  maps, rotation or protocol. No private packs are needed any more.
- **Eight-client load test:**
  `cargo test -p peakrunner-server --lib eight_clients_sustain_movement_and_all_weapons -- --ignored`
  (`PEAKRUNNER_LOAD_MAP` selects the map).
- Byte-identical rebuilds: build a pack twice into scratch dirs and compare
  all payload hashes; when a shared module changes, rebuild every map and
  confirm unchanged maps stay identical.

### What the map suites check (shared helpers in `scripts/assets/`)
| Helper | Checks |
|---|---|
| `spawn_checks.py` | 1.2 m support ray, stand-and-stop, 6 m forward clearance, not exposed from the field at 40–100 m |
| `route_checks.py` | walkable/jet entries per region; `flag_routes` distinct (entry, approach) routes — minimum ~10 per flag; generator rooms exactly 2 entrances |
| `sightline_checks.py`, `turret_arcs.py` | no turret sees standable floor deeper than 3 m inside rooms, under the `arc1` rule |
| `surface_checks.py` | no visible coplanar different-material overlaps (true coplanarity, float32-tilt safe), no hovering edges |
| `prop_checks.py` | props clear of flags/lanes/doors/rings/spawns/mouths; cover mirrored, blocks shots, never traps; lid shade ≥ bare terrain; instances valid |
| `water_checks.py` | volumes on carved beds, intended depth, protected-zone clearance |
| `budgets.py` | collision ≤ 4,500 triangles per base (measured tick cost justified the raise from 3,000); props share |

Notable Rust tests: turret sightlines per map, `acquire_target` equivalence,
muzzle-through-wall, shields/regen/generator/kit, C&H timing and drain,
water tables and dry-replay identity, `a_lone_bot_reaches_the_enemy_flag_on_every_map`,
`flyers_cross_every_map_mostly_airborne`, menu orbit clearance, embedded pack
hash/compression, manifest validation (`look`, `water_volumes`,
`control_points`, `spawn_points`).

---

## 6. Visual QA — how captures are made and verified

Captures run the real client via `examples/launch_smoke.rs` in a local match.

Common environment:
- `QA_LOCAL=1 QA_MAP=<key>` — local match on a map; `QA_MODE` selects mode.
- `QA_CAPTURE_PATH=<png>` — write one frame; `QA_CAPTURE_AT` time; `QA_PAUSE=1`.
- `QA_FLYCAM=x,y,z,yaw_deg,pitch_deg,fov_deg` — detached render camera
  (`src/drawlist.rs`); does not affect physics.
- Staging hooks (QA-only, env-gated): `QA_PLAYERS` (place players, pose field),
  `QA_EQUIPMENT` (health/shield), `QA_POINTS`/`QA_POINT_STATE` (C&H),
  `QA_WATER` (stage volumes), `QA_LOOK` (preview a look), `QA_BLOOM`,
  `QA_KIT`, `QA_CARRY_AT`, `QA_BOOM_AT`, `QA_SCORE`, `QA_SCOREBOARD`,
  `QA_MENU_CLIP`, `QA_LID_PROBE`, `QA_CLICKS`, `QA_CHAT`.
- `PEAKRUNNER_MAP_PACK=<abs path>` loads an unembedded pack for preview in the
  Raindance slot (pipeline step 4); `PEAKRUNNER_SOUND_DIR` for sound overrides.

Rules that were learned the hard way:
- Compute cameras from world-space anchors; a room corner at eye height looking
  across. View every PNG with the Read tool and re-aim bad shots (several
  captures were a camera pressed against a wall).
- Captures fail when the display sleeps: keep the machine awake
  (`caffeinate -d -i -t <s>` in the background, then stop it). Don't leave it
  running.
- When Rust is being edited by other agents, build the capture harness in a
  **temporary clean worktree** (`git worktree add --detach ../peakrunner-capture <commit>`)
  and remove it afterwards; serialize parallel captures with a lock
  (`mkdir /tmp/peakrunner-capture.lock`).
- Screenshots go to ignored `research/screenshots/` with a descriptive prefix
  (`tc-rework-*`, `water-maps-*`, `bloom-{off,on}-*` …). Before/after pairs
  use identical cameras.
- Audio "captures" are WAV renders in `research/audio-samples/` and A/B pairs
  in `research/audio-reference/ab/`.

---

## 7. How the agent workflow was run

The main session orchestrated; most implementation was done by **fork**
agents (they inherit the conversation context). Patterns that worked:

1. **Survey or concept first, then build.** Big design changes started with a
   read-only survey/concept (published as a claude.ai artifact for the user),
   then a build agent. Examples: the Tower Complex concept pages, the Ascend
   research proposal, the playability survey, the visual audit, the weapons
   audit.
2. **Explicit file ownership per agent.** Every prompt listed what the agent
   owns, what parallel agents own, and which shared modules are read-only.
   Shared Rust files (`sim.rs`, `map_pack.rs`, `terrain.rs`) got *targeted
   edits only*; "if a build breaks from others' in-progress work, wait and
   retry".
3. **Sequence anything that rebuilds packs.** Two agents rebuilding the same
   map packs collide; pack-rebuilding work was serialized or split by map.
   Engine-only work ran in parallel with map work.
4. **Agents never commit.** The main session re-ran the full checklist on the
   combined tree, reviewed diffs (`println!`/`dbg!`, private-data references,
   sim.rs removed lines), then committed in logical commits. Mixed files were
   staged together; one agent saved its part as a patch
   (`git apply --cached`), which let it be committed separately.
5. **Merging to main** without disturbing running agents:
   `git fetch . cleanup/clipping-textures:main && git push origin main`.
6. **Interim notifications** ("stopped with background work still running") are
   not completion; wait for the final report. Agents that hit the 200-turn
   limit were resumed with `SendMessage` and a narrow "wrap up" brief.
7. **Redirects mid-flight** used `SendMessage` (e.g. the shield design
   reversal, flank sewer shafts, explosion length correction).
8. **Permissions:** the auto-mode classifier blocked a sub-agent from reading
   the CrossOver bottle and the assistant from `kill`. The main session read
   the bottle itself (user-authorized) and extracted into `research/`; kills
   were handed to the user.
9. **Clean up** leftovers: orphan loops (one wait loop miscounted "exit"
   lines and never ended), stray capture scripts, temporary worktrees,
   keep-awake processes.
10. **Task list:** `TaskCreate`/`TaskUpdate` tracked the queue with
    blocked-by dependencies.

---

## 8. Originality and private references

- Everything shipped is original. Private reference packs (Broadside,
  Stonehenge, Snowblind, Desert of Death), the Tribes 2 audio, and Ascend web
  research were used only for statistics, design intent and A/B listening.
  They live in ignored `research/` (`*-study/`, `audio-reference/`,
  `ascend-study/`) and are never copied, traced, resampled, embedded or
  committed. Map docs state "no <reference> data is used".
- Player-facing text must not reference other games (the menu blurb was fixed).
- Map names are original; keys stay for compatibility.

---

## 9. User preferences (design rules that persist)

- **Open doorways** players can shoot and fly straight into; fix turret
  sightlines by moving/angling turrets (`arc1`), not walls. Baffles only for
  generator and spawn rooms.
- **Never a turtle:** "two main entrances but ~10 ways of getting to the flag";
  fly-through grabs possible; some flags fully open.
- **Flyable interiors:** ceilings ≥ ~8 m where fights happen, 6 m doors,
  two-storey rooms enterable from each level.
- **Shields** need teamwork or a downed generator to break.
- Sounds deep, weighty and short — not "DOS", not long tails.
- Terrain: natural rolling hills with some tall summits; no spiky peaks or flat
  plates.
- Ground cover dense at eye level with real cover that blocks shots/progress;
  water slows you.
- The user playtests and gives short feedback; act on it directly, show
  screenshots, commit, and merge to `main` when green. Keep momentum.
- Persisted in assistant memory at
  `~/.claude/projects/-Users-jeffrywalsh-workspace-peakrunner/memory/feedback_map_design.md`.

---

## 10. Gotchas

- Spawn centres at 1.2 m above the floor, or players skate frictionlessly.
- Texture replacement must write **all mip levels**.
- Never write non-deterministic values (build time) into `map.json`.
- Editing a builder source (even comments) changes source hashes in `map.json`;
  rebuild that pack.
- Tunnel/cavern lids must not cast terrain shadow.
- The z-fight checker must test true coplanarity (float32 normals tilt).
- Bash `cwd` persists; `cd ..` from the repo root leaves the repo. Use absolute
  paths.
- Pack size: dense props must be instanced, not baked (binary hit 59 MB once).
- `rg` output of Python suites contains colour codes.

---

## 11. Backlog and open items

- **Playtest** (human): rooms, atriums, flag routes, cover, water, Frostline
  drain, C&H, sound, bot difficulty, Old Holler ravine crossing.
- Water surfaces are flat (no ripples/reflections).
- First-person chaingun/grenade launcher detail (second weapon pass).
- Old Holler blue-side bots slow on interior stairs; bots don't use every
  shaft/tunnel optimally.
- C&H mode tuning (scores, point placement) after play.
- Selected-map-only admission and independent pack updates (older roadmap).
- **Scorch decals are invisible:** the fade in `vs_emit` uses the wrong
  transform on the flattened disc, so only a ~1 cm rim renders. Found while
  fixing the black lines; not yet fixed.
- The last fix of the session: incoming discs showed as black lines because
  the round's hub was dark and thicker than its rim (plus a NaN tracer at zero
  velocity). The hub is now self-lit; `projectile_draws_never_render_dark` and
  the ignored GPU probe `incoming_rounds_never_darken_the_view` guard it.
  `QA_ROUNDS` / `QA_BLASTS` stage rounds and explosions for captures.

---

## 12. Deployment status

Nothing from this session is deployed. The live VPS runs the September
image with the six-map clone rotation; `src/deploy/vps/compose.yaml` in source
now lists the five-map rotation. Because the compatibility marker changed, a
release needs a matching client and server build, packaging, signed launcher
feed (higher sequence than `2026092102`), and the website update, following
AGENTS.md "Safe release procedure": back up, build on dellcon, check occupancy,
update the server before exposing clients, verify over public HTTPS. Do not
start a deploy without the user's explicit go-ahead.
