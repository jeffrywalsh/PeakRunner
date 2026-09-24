# Capture & Hold

Control points ("capture towers") and the Capture & Hold mode. Engine-only for
now: no shipped map declares control points yet, so the mode cannot be put in a
rotation until the map passes place towers. Compatibility marker: `cnh1`.

## Rules (`crates/core/src/control.rs`)

- A point is a ring (default radius 12 m) over a floor position. Presence is a
  cylinder: within the radius horizontally, from 2 m below to 10 m above the ring.
- A point flips to a team after **10 s** with only that team's living players
  inside. Presence count doesn't speed it up.
- Both teams inside: **contested**. Progress pauses.
- A team on a point with the other team's partial progress first undoes that
  progress at the same rate, then starts its own. Defenders on their own point
  also clear enemy progress.
- Nobody inside: progress decays at half the capture rate (20 s from full).
- Points start neutral and reset on map change, round restart and warmup. Warmup
  and countdown never capture or score.
- The server owns all of it. Snapshots carry each point's owner, progress,
  capturing team and contested flag. Clients announce owner changes by diffing
  frames, so a replayed snapshot announces nothing.

## Drain field

A point may declare `drain` (default radius 60 m, rate 10 energy/s). While the
point is held, the holder's **enemies** inside the radius lose energy. The
holders are unaffected; a neutral point drains nobody. It runs in the shared sim,
so client prediction of your own energy matches the server.

Maths, with jetting at 15 energy/s, regen at 12/s, a 60-energy tank and a
3-energy minimum to jet:

| | Normal | In an enemy drain field |
|---|---|---|
| Continuous jet from a full tank | 57 / 15 = 3.8 s | 57 / 25 = 2.3 s |
| Recharge on the ground | 12/s (4.75 s to full) | 2/s (28.5 s to full) |

So a drained player gets one burst, then short hops every few seconds; sustained
flight is impossible. Covered by `drain_field_allows_hops_but_not_sustained_flight`.

## Capture & Hold mode

- Mode key `capture_and_hold` (`SupportedMode::CaptureAndHold`). Flags are
  inactive and hidden, with no flag markers.
- Each held point scores **1 point per second**; the first team to **300** wins.
  The existing match timer still ends the round, and the higher score wins.
- In CTF, only points with `ctf_active` run (e.g. Frostline's centre); in Capture &
  Hold, every point runs.
- Server rotation: `{"map":"...","mode":"capture_and_hold"}`. Rotation rejects it on
  a map with fewer than 2 control points, with a clear error. It is not in the
  default rotation.
- Offline menu: a MODE row (Capture the Flag / Capture & Hold). Capture & Hold is
  disabled until the selected map has at least 2 points.
- Bots (minimal): defenders guard their nearest held point; everyone else heads for
  the nearest point not yet theirs. Full personalities come later.

## Manifest schema

```json
"control_points": [
  {"id": "beacon", "name": "Beacon", "pos": [1024, 210, 1024],
   "radius": 12, "ctf_active": true, "drain": {"radius": 60, "rate": 10}}
]
```

- `id`: lowercase key; `name`: 1–24 letters, digits, spaces or hyphens.
- `radius`: 2–40 m.
- `drain.radius`: from the ring radius to 200 m; `drain.rate`: 0–40/s.
- At most 8 points. The optional field is absent on all shipped packs, so their
  hashes are unchanged.

## HUD

- A world ring at each active point, coloured by owner (neutral grey, Ember red,
  Glacier cyan), with an arc showing capture progress in the capturing team's
  colour, and a name, state and distance label.
- Standing in a ring: a progress bar with "CAPTURING BEACON · 6.0 s",
  "HOLDING", "CLEARING ENEMY PROGRESS" or "CONTESTED".
- Inside an enemy drain field: a red outline showing the field (only to those it
  hurts), a pulsing "ENERGY DRAIN −10/s" warning and a red screen edge.
- Capture & Hold: edge-of-screen markers for off-screen points, owner chips under
  the score, the "Hold the points · first to 300" caption, and a Capture & Hold
  line in the Tab roster.
- Announcements through the shared announcer: "Your team captured the Beacon",
  "Your team lost the Beacon", "Ember captured the Beacon". Captures use the flag
  chime; a point becoming contested uses the drop cue.

## QA staging

`QA_POINTS="Beacon:1100,123.7,700:drain;West:1040,120.1,680"` stages temporary
points (`name:x,y,z[:drain][:rRADIUS]`) on any map, offline and in the client
view. `QA_POINT_STATE="0=1/0.4/0/c"` forces point `index=owner/progress/capturing[/c]`
(`-` = none). `QA_MODE=cnh QA_SCORE=120,45` shows the Capture & Hold HUD.
Screenshots: `research/screenshots/cnh-*.png`.
