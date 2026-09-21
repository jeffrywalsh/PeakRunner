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
