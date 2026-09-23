> **Retired:** Skybreak Bastions and its fortress builders were removed from source because they derived from Broadside measurements. Kept as history only.

# Skybreak Bastions — original floating-base CTF playtest

The user requested consolidating this development checkpoint and its multi-map
dependency onto main in the original checkout. It is not public-release ready;
remaining playtest and integration work below still applies.

Valley is removed from the playable menu and production server configuration.
Its procedural terrain/enum remains an internal regression-test fixture, not a
selectable replacement slot. Raindance is retained without changing its assets.

Skybreak is an original Broadside-inspired layout, not a converted Tribes map:
two floating fortresses 298 m apart (measured from Tribes 1's own Broadside
mission, see below), internal flag alcoves with solid roofs,
opposed deck entrances, sunken entrance ramps, split loft ramps, a large
equipment hall, exterior plasma turrets and lower recovery pads. Tall mountain
walls enclose a central valley. The taller tapered fortresses are instances of
the reusable asset described in `floating-fortress.md`.
Generators, inventory and repair stations use existing server
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
  captures the exterior, entrance, hall and loft without rewriting Raindance captures.

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

Fortress revision (`map/skybreak-fortress`): 141 workspace library tests passed,
native all-target check passed, two prefab and five original-map compiler tests
passed, and app boundaries passed. A fresh compilation was byte-identical to the
checked-in pack. Exterior, entrance, hall and loft GPU captures were inspected.
Ramp tests cover both rotated bases, including side/rear entry and head clearance.
This revision has not been deployed or manually tested through a full match.

Enclosed-storey follow-up: v3 replaces the atrium with solid floor/ceiling
separation and upper room partitions. Removed coplanar hall/landing and
rear/side gallery overlaps. 142 workspace library tests and three prefab tests
pass; native all-target check and fresh entrance/hall/upper-floor GPU captures
pass. Captures were visually inspected. Full human traversal remains pending.

Measured v4 follow-up: decoded the private T2 Classic Broadside DIF and rebuilt
the prefab with an 88 x 104 m deck, -6/7/14/25/31/38/45/57 m circulation levels,
front flag room and deep keel. See `broadside-reference-study.md` for source
measurements and explicit remaining differences; this is not a verified 90% match.
Only our authored geometry/materials are compiled into the game. Reference
sections remain ignored local files. Both rotated instances pass sampled ramp
support/headroom and flag-corridor checks. Principal-level GPU captures are under
`screenshots/grass-look-fortress-*.png`. No public deployment or full-match QA.

Base-distance correction: Tribes 1's own `Broadside.MIS` (plaintext, no
geometry decode needed) puts its two base towers 297.8 m apart; the 640 m gap
here was an unmeasured guess. `maps/skybreak-bastions.json` now places the two
bases 298 m apart, and `crates/core/src/terrain.rs`'s `ember`/`glacier`
constants plus the `fortress_*` test fixtures were updated to match. The GPU
exterior capture now shows both fortresses in frame from the original vantage
point. See `broadside-reference-study.md` for the Tribes 1 measurement method
and the storey-height cross-check against Tribes 2. 142 workspace library
tests and the floating-fortress prefab tests still pass after the move.
