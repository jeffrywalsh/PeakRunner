# Scoreboard, flag bearings and distance audio

Release update: `.20260919.3` and its matching server are now public, together with
the native launcher. The previously staged FOV/capture/turret changes below shipped
in this coordinated release. See `docs/launcher.md` for verification and caveats.

Deployed as `0.1.0-raindance.20260919.2` to Springdale Central and the Mac,
Windows and Linux downloads on 2026-09-19. See `deploy/client-release-staging.json`
for immutable archive hashes and platform verification limits.

- Hold Tab: each connected player's server-measured QUIC RTT appears in ms.
  It is not client-reported or the input-ack latency. Unavailable transports
  (local TCP QA) display an em dash. Connection cleanup removes its RTT entry.
- Off-screen flag arrows use the scene camera basis and FOV, team colors and
  distance in world meters. Behind-camera targets stay on the HUD boundary.
  Carrying the enemy flag replaces its self-pointer with our capture-base target.
- Remote shots and explosions have position-based volume. Full volume within
  6 m, smooth squared falloff to silence at 120 m for shots, 180 m for explosions.
  Offline bot shots follow the same path. Local weapon/reload/hit/death feedback
  and match/flag announcements intentionally remain non-positional. Ambient
  emitters retain their existing distance handling. This adds attenuation, not
  wall occlusion, HRTF, or simulated sound travel time.
- Snapshot ordering prevents repeated snapshot audio. First join/new round does
  not play historical remote shots/explosions. Sounds are not gameplay authority.

Wire compatibility is now `ping1`; old clients and servers deliberately reject
each other. Future releases must assign a new release version and rebuild/deploy
matching server and client archives. The directory itself needs no protocol code.
Do not publish these under the existing immutable release filenames.

Verification: workspace library tests (including encrypted RTT transport), native
all-target compilation, wasm library check, dependency-boundary checks, and the
real native local-match screenshot in `screenshots/ping-flag-hud.png`.
The screenshot uses local TCP and therefore correctly shows unavailable ping.
Subjective multi-machine audio balancing remains a playtest task.
Public verification: two encrypted clients for 10 seconds, maximum snapshot gap
100 ms and input-ack gap 9 ticks. Native WAN screenshot
`screenshots/public2-wan-ping.png` shows the server-measured 40 ms RTT.

Reproduce the isolated HUD capture:

```sh
QA_LOCAL=1 QA_SCOREBOARD=1 QA_CAPTURE_PATH=screenshots/ping-flag-hud.png cargo run --example launch_smoke
```

This creates an ephemeral loopback match and closes only its own QA window.

## Enemy arrows, carrier markers and flag announcements (source only, not deployed)

Code: `src/src/world_overlay.rs` (markers) and `src/src/flag_announce.rs`
(announcements). Client-only presentation built from snapshot fields the server
owns (player positions, teams, names, flag carriers, score); no wire change and
no compatibility marker change.

- **Red arrow over enemies.** A downward chevron in the enemy name-tag red, over
  every living enemy within `ENEMY_ARROW_RANGE` (250 m) that is on screen AND in
  line of sight (the same map/terrain ray as name tags). Never drawn through
  terrain or walls. It shrinks from 18 px wide close up to 11 px far out and fades
  over the last 20% of range. Enemies within 80 m also show their red name above
  it. Teammates never get an arrow: only their blue name within 150 m.
- **Flag carrier marker.** Replaces the arrow and name tag on a carrier: a larger
  chevron and pennant in the carried flag's team colour, the carrier's name (blue
  for your team, red for the enemy), and "FLAG CARRIER" or "HAS YOUR FLAG" with
  the distance beyond 60 m. It shows out to `CARRIER_MARKER_RANGE` (1500 m, past
  every map's flag distance) on screen, **through terrain**: the edge-of-screen
  flag bearing already reveals every flag's position, so this adds no information.
- **Centred announcements.** Large pale text with a dark outline, a quarter of the
  way down the screen, for 3 s with a 0.6 s fade: "Budster has the enemy flag",
  "Echo has your flag", "You have the enemy flag", "… dropped …", "… captured …",
  "Your flag was returned", "The enemy flag was returned". Wording is relative to
  the viewer. They queue: up to 3 wait (the oldest waiting one drops on overflow)
  and, while others wait, the one on screen yields after 1.2 s. They come from
  diffing flag carriers, flag-at-home and score between frames, so a replayed
  snapshot announces nothing and a score reset (new round) rebaselines silently.
  The simulation no longer writes its own flag text into the top HUD line;
  VICTORY/DEFEAT still appear there.
- QA: a trailing `*` on a `QA_PLAYERS` name hands that stand-in the enemy flag
  after `QA_CARRY_AT` seconds (default 6), so the real announcement path fires
  before the 8 s capture. Captures: `research/screenshots/markers-*.png`.

## Sound engine (source only, not deployed)

Code: `src/src/sound.rs` (palette, mixer, director, tests) and `src/src/audio.rs`
(output only). This supersedes the distance note above: world sounds now have
pan, distance dulling and, for explosions, travel delay. Client-only; no wire or
compatibility change, and no gameplay change.

- **All original, all synthesized.** Every sound is generated at startup from
  deterministic noise, oscillators, resonators and FM; no samples from any
  game or library. 27 one-shot cues (many with 2–6 variants so rapid fire never
  repeats one clip) and 9 loops.
- **Deep, layered design (v2).** The first palette sounded thin and "DOS": bare
  sine sweeps and FM bells with no low end (shield hit, shield down and repair kit
  had 0% of their energy below 250 Hz), hiss-only ski and jet loops (56% and 16%
  above 6 kHz), and short dry tails under 0.5 s. Every cue is now layered: a sub
  (30–80 Hz) for weight, a saturated body, filtered pink-noise texture and a
  short transient. Oscillators are band-limited (polyBLEP saws), filters are
  zero-delay state-variable filters, envelopes rise and fall smoothly, and a
  gentle tanh saturation glues the layers. Flag and match cues are warm
  detuned-saw stings an octave lower instead of bells. Measured change, spectral
  centroid before → after: shield hit 1559 → 291 Hz, ski 7971 → 438 Hz, jet
  3254 → 180 Hz, repair kit 1913 → 286 Hz; tails now 0.3–3.3 s. Tests keep the
  weight (minimum energy below 150 Hz per cue) and forbid aliasing buzz (under 1%
  above 12 kHz for tonal sounds).
- **Held-weapon idle hums.** The weapon in your hands hums quietly: the disc
  launcher a deep throbbing electric hum (55 Hz with a 3 Hz throb) with a
  spinning whir inside it, the chaingun a low motor tick, the grenade launcher a
  mechanical settle. Only your own weapon; ducked to 30% while it fires; the
  mixer crossfades on a switch.
- **Master bus.** A stereo feedback-delay-network reverb (pre-delay, four damped
  lines, Householder mix) replaces the small metallic room: long and dark
  outdoors (RT60 2.4 s), shorter and brighter under a roof (0.9 s). Its input is
  high-passed at 140 Hz so the low end stays mono and tight. A soft-knee 20:1
  limiter at -2 dBFS keeps pile-ups from clipping.
- **Your own sounds.** Put WAV files (8/16/24/32-bit PCM or 32-bit float, any
  rate, mono or stereo) in a sounds folder and they replace the synthesized cue
  at startup; anything missing stays synthesized. Folder: `PEAKRUNNER_SOUND_DIR`,
  else `sounds/` beside the executable (macOS: `Contents/Resources/sounds/`),
  else `assets/sounds/` when run from `src/`. Names: `disc-fire.wav`, with
  optional variants `disc-fire-2.wav` … `-8.wav`; loops use `loop-idle-disc.wav`,
  `loop-jet.wav` and so on (the full list is `Cue::file_stem` and
  `Loop::file_stem` in `sound.rs`). Desktop only; the browser build always uses
  synthesis. Packaging doesn't copy a sounds folder yet. Shipped sounds must stay
  original: never drop in audio taken from another game.
- **One mixer on both platforms.** Native renders it through rodio in 512-frame
  blocks; the browser runs the same mixer in a WebAudio script node created on
  the first user gesture. A missing device (e.g. Linux "ALSA no device") leaves
  the game silent, never crashed.
- **Voices.** 32 one-shot voices plus 9 loops and the map ambient bed. Per-cue
  caps (chaingun and turret bullets 8, footsteps 4, shield and hull hits 4,
  explosions 6, others 3). When full, the least important, most finished voice is
  stolen; a newcomer never steals from a higher-priority voice. Priorities, from
  high to low: flag, capture and match cues; generator blast, hit marker, pain, kit,
  shield down; own weapons; explosions and plasma; bullets and impacts; footsteps and bounces.
  Playing a sound and rendering never allocate after startup.
- **Spatial.** Smooth squared falloff to each cue's range (footsteps 38 m, shots
  140–150 m, near explosions 180 m, far explosions 520 m, generator blast 420 m).
  Equal-power stereo pan from the camera. A one-pole low-pass makes distant sounds
  duller, and sounds behind you are slightly quieter and duller. Explosions beyond
  40 m arrive late at 340 m/s; gunfire is not delayed, so feedback stays instant.
  Explosions beyond 75 m switch to a rolling low "far" layer.
- **Room feel.** Under a roof or below the terrain surface (checked four times a
  second) adds a short room reverb, cuts speed wind to 20% and the map ambience to 35%.
- **What plays when.**
  - *Weapons:* layered disc, chaingun and grenade shots with per-shot variation;
    chaingun motor whine follows the same spin-up and coast-down as the barrels.
  - *Turrets:* their own bullet and plasma shots, spatialized.
  - *Movement:* footsteps at stride rate for you and the three nearest walkers;
    landing thumps scaled by impact; ski hiss and wind by speed; a jet loop
    whose pitch follows climb and speed.
  - *Equipment:* shield pings at the struck turret or sensor, a falling sweep
    when a shield collapses, hull clanks once it is down, a hum near a running
    generator (silent once offline), and the big generator blast.
  - *Other:* grenade bounces, the hum and Doppler of a disc passing within 26 m,
    weapon-switch clicks, the repair-kit shimmer, and new chimes for flag
    taken/dropped/returned and match start/end.
- **Tests.** Every cue and variant is deterministic, finite, bounded and
  click-free at 44.1 and 48 kHz. Loops wrap seamlessly. Voice caps and stealing
  are checked, as are pan, distance and filter maths, delay and far/generator
  layer choice, and the director's footsteps, landings, switches, kits, shields,
  hum and chaingun spin. A source scan fails the build if the sim or network code
  emits an event name with no cue.
- **Listen.** `cargo test -p peakrunner --lib render_audio_samples -- --ignored`
  writes WAVs, including the weapon idles and a `demo.wav` walkthrough, to
  ignored `research/audio-samples/v2/after/`; the pre-v2 renders are in
  `research/audio-samples/v2/before/`.
- Not done: wall occlusion, HRTF, Doppler on anything but passing discs,
  third-person jet loops for other players, bullet ricochets. Mix balance still
  needs a human ear.

## Published revision: 20260919.3

- FOV stays at 76 degrees through 20 m/s. A smoothstep speed curve widens it
  to 88 degrees at 120 m/s; the camera and authoritative muzzle use the same
  curve. This is speed-based easing, not a delayed camera lens or physics change.
- Original synthesized 2.4-second capture motifs: rising victory sequence for
  the scoring team, descending warning for opponents. Native and WebAudio share
  PCM synthesis. Capture cues are global and do not fade with distance. Duplicate
  snapshots, initial joins and score resets do not trigger capture stings.
- Plasma turrets solve constant-velocity interception at 80 m/s, including
  muzzle offset and the three-second projectile lifetime. The earliest positive
  solution wins; unreachable targets fall back to direct aim. Acquisition still
  requires a visible enemy, and the predicted firing path must also be clear.
  Projectiles are not homing: a player changing velocity after firing can evade.
  Damage, splash, knockback, cooldowns, range and movement remain unchanged.
- Compatibility adds `equipment3` and `fov1` because authoritative aiming and
  muzzle behavior changed. Deploy matching clients and server together.
