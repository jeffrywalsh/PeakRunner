# Skybreak Bastions — original floating-base CTF playtest

The user requested consolidating this development checkpoint and its multi-map
dependency onto main in the original checkout. It is not public-release ready;
remaining playtest and integration work below still applies.

Valley is removed from the playable menu and production server configuration.
Its procedural terrain/enum remains an internal regression-test fixture, not a
selectable replacement slot. Raindance is retained without changing its assets.

Skybreak is an original Broadside-inspired layout, not a converted Tribes map:
two floating fortresses 640 m apart, internal flag alcoves with solid roofs,
front hangars, side entrances, central roof jet hatches, landing wings and lower
recovery pads. Generators, inventory and repair stations use existing server
equipment behavior. Terrain, geometry, textures and sounds are PeakRunner's own.
The approved movement constants are untouched. Jet routes, bot navigation and
competitive balance still require playtesting; this is a first playable layout.

## Authoring and playing

- Source: `maps/skybreak-bastions.json`, `scripts/build-skybreak.py`.
- Compiler reuses the original mesh/material kit; no extracted files needed.
  `python3 scripts/build-skybreak.py /absolute/new/output-directory` refuses
  overwrites. Commit regenerated assets only after checking geometry and hashes.
- In a client built from this branch, choose **Skybreak Bastions** in the offline
  map menu. The usual Start match launches it.
- Dedicated server: `PEAKRUNNER_MATCH_MAP=skybreak-bastions`, or rotation
  `[{"map":"skybreak-bastions","mode":"ctf"},{"map":"raindance","mode":"ctf"}]`
  in `PEAKRUNNER_MATCH_ROTATION`. Server remains authoritative.
- `QA_LOCAL=1 QA_MAP=skybreak-bastions QA_CAPTURE_PATH=<absolute.png>
  cargo run --example launch_smoke` captures a real local client/server interior.
- `QA_SKYBREAK=1 cargo test -p peakrunner --lib render_grass_captures -- --ignored`
  adds an exterior capture (also regenerates the older grass QA captures).

## Compatibility and remaining work

Skybreak has a separate enum identity and immutable embedded pack; Raindance's
external pack override cannot replace it. Renderer switches geometry/textures
by active map. Shared collision and rendering use the same authored triangles.
Skybreak uses exact authored deck spawn points rather than outdoor spawn search.

Protocol marker `maps2` incorporates both pack fingerprints. Old `.4` clients
must not join this development server. Selected-map-only compatibility and the
general installed-pack registry remain unfinished. Skybreak is currently embedded
even in external-map builds; signed multi-pack launcher distribution is not done.
Do not publish this as a `.4` patch or update live hosts independently of clients.

Verified: solid spawn support and overhead flag roofs, floating clearance,
distinct pack fingerprints, real local client interior and GPU exterior renders,
and two local network clients receiving Skybreak from server rotation. Keep full
round/map-switch QA, traversal playtest, bot behavior and release packaging on
the remaining checklist. Windows compilation is not Windows runtime validation.

Checkpoint verification: 140 workspace library tests passed; all-target native,
Windows cross-compile and WebAssembly checks passed. Five Raindance compiler tests
and app-boundary checks passed. Rebuilding Skybreak into a fresh directory gave
byte-identical files. Captures: `skybreak-menu.png`, `skybreak-interior.png`,
`grass-look-skybreak-exterior.png` in `screenshots/`.
