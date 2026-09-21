# Broadside interior audit — v7, 2026-09-20

## v8 follow-up: gaps and circulation redesign

After the user's follow-up about large gaps, checking the layout rather than
individual ramps exposed overlapping lower/upper stair footprints. A floor
under a sampled point did not prove the player could move to the next landing.

Changed:

- Lower gallery ramps: x=±18, width 4 m; armory ramps: x=±22, width 4 m.
  These are adjacent, not overlapping. Gallery doors at z=10..14 connect the
  lower landing. A rear crossover at z=23.5 leads into the armory stair portal.
- Gallery corridors have explicit ceilings and inner partitions rather than
  exposing the multi-storey hall void. Their clear width is still narrower than
  the reference; closure is not counted as a dimensional fidelity match.
- Armory, spine, defence and roof ascending ramps use a reusable original
  enclosed stairwell primitive: floor, side walls, following roof, two end
  portals. This closes side views into unused shaft spaces. End portals and
  the floor apertures occupied by these stairs are intentional openings.
- Filled the unused front portion of the 31 m spine floor aperture (z=0..6).
- Added a 45 m roof-stair approach landing, full-width 28 m cross-ramp
  landings, and shortened the entrance's side walls by 1 m at the hall-ramp
  turn. Complete-route sweeps exposed these additional missing/pinched links.
- Compiler records the new room primitive's source hash. Source geometry is not
  a build dependency. Exterior and terrain are unchanged.

Validated:

- `fortress_entry_to_roof_connections_have_no_gaps_or_blocked_portals`
  samples a complete authored entrance-to-roof route (including the flag-gallery
  detour), at 101 positions per segment, on both sides of
  both rotated bases: floor error <0.08 m, radius-0.52 m swept body clearance,
  and overhead clearance. This includes lateral door crossings and landings.
- Existing ramp support/headroom, flag doors, hall/flag measurement and storey
  checks still pass. Python prefab checks still reject overlapping coplanar
  floor boxes and check transformed instances and equipment ownership.
- The final v8 audit is `local-assets/broadside-interior-audit-v8-complete/audit.json`, with new
  input hashes. The v7 before/after table below remains historical, not v8 data.

Still pending: whole-building gap/portal enumeration, a human entry-to-roof
traversal, reference gallery/shaft/armory shape equivalence, bot navigation and
human match play. Neither unit tests nor selected screenshots justify saying
all gaps are gone. The remaining upper-floor layout may need further redesign.

Final v8 verification: 145 workspace library tests, four prefab/material tests,
two synthetic measurement tests, all-target native check, app-boundary checks,
release build and GPU capture test passed. Gallery, stairwell, flag chamber,
armory and generator captures were visually inspected. No deployment, commit,
or update to the private reference app was performed. Play this original
rebuild with the normal client, selecting Skybreak Bastions.

Purpose: compare the installed T2 Classic Broadside reference with the original
Skybreak building numerically, including what already matches, not just fixes.
This supersedes conflicting historical assertions in `broadside-reference-study.md`.
It does **not** establish 90% overall fidelity or an exact T1 reconstruction.

## Reproduce and provenance

```sh
local-assets/tools/venv/bin/python scripts/audit-fortress-interior.py \
  --output local-assets/broadside-interior-audit-new
local-assets/tools/venv/bin/python scripts/test-interior-audit.py
python3 scripts/test-floating-fortress.py
cargo test --locked -p peakrunner-core fortress_
```

Output must be a new ignored local directory. Before: `local-assets/broadside-interior-audit-20260920/audit.json`.
After: `local-assets/broadside-interior-audit-v7/audit.json`. Reports contain both
bases, ten six-direction interior probes and eight entry-route samples per base,
mission equipment positions, and SHA-256 of both input manifests/collision files.
The private reference app and its source pack were not changed.

Measurements use the collision triangles actually loaded by the game, not
screenshot pixel estimates. All points below are **[right, forward, height]**
metres relative to the base deck; they are not engine XYZ. Source mission
positions are first converted to engine `[X+1024,Z,Y+1024]`, then transformed
by the base's `world_to_local` matrix. Original Skybreak vertices are transformed
by the inverse of their authored placement. Reports retain raw float precision;
tables round to centimetres. Rounding is not a guarantee of physical accuracy.

Rays are double-sided nearest positive triangle intersections, capped at 150 m.
They include equipment; a hit is not automatically a structural wall or floor.
No hit means no intersection within range, not a proven doorway. A probe can
occupy a non-walkable volume. These measurements do not establish room-volume
equivalence or route connectivity. Both bases are measured, not assumed mirrored.

## Validated unchanged (base 0 probes)

| Probe | Reference | Original | Validation scope |
| --- | --- | --- | --- |
| Entry (0,-29,2), left/right | 5 / 5 m | 5 / 5 m | 10 m clear width at this section, not 11 m |
| Entry floor/ceiling distances | 2 / 4 m | 2 / 4 m | 6 m vertical clearance at this section |
| Lower inventory (0,2,-4), floor | 2 m | 2 m | Same -6 m support at sample |
| Lower inventory ceiling | 3.88 m | 4 m | 0.12 m difference, not exact |
| Hall (0,10,9), floor/ceiling | 2 / 14 m | 2 / 14 m | Same sampled floor and ceiling |
| Armory (0,-12,27), floor/ceiling | 2 / 4 m | 2 / 4 m | Height match only; room width differs |
| Generator (8,15,40), floor/ceiling | 2 / 4 m | 2 / 4 m | Floor 38, ceiling 44 at sample |

Inventory counts are six per base, with two at each of approximately -6, 7 and
25 m. Heights differ by up to about 0.021 m; horizontal placement is approximate,
not exact: base-0 lower stations are near (+13.69,2.46) and (-13.68,2.51), versus
our (+13,2) and (-13,2). Hall stations are near (±8.84,10.34), versus (±9,10).
Upper stations are near (-12.41,-13.29)/(12.55,-13.28), versus (±12,-13).

## Measured corrections in original v7

| Probe/direction | Reference | Before | After |
| --- | --- | --- | --- |
| Hall left / right | 11 / 11 | 12 / 12 | 11 / 11 |
| Hall back / front | 17.5 / 12.5 | 44.38 / 27.75 | 17.5 / 12.5 |
| Flag (0,-12,16), back / front | 4 / 4 | 5.5 / 48 | 4 / 4 |
| Flag ceiling | 4 | 5 | 4 |
| Generator left / right | 20 / 4 | 32 / 16 | 19.5 / 3.5 |
| Generator back | 17.5 | 35 | 17.5 |

The hall now has actual bounding partitions. The flag-room central front wall
is restored; side routes at x=±15 remain open. Reference forward rays from the
flag-room center stop at z=-8, while rays at x=±14/16 continue over 42 m. This
**disproves** the earlier inference that there should be a single wide central
opening. Tests now require central solidity and side openings, rather than
preserving a mistaken expectation. The flag ceiling is lowered by one metre.
Generator gallery walls reduce oversized sightlines without moving its floor.
Exterior geometry, turrets, terrain and approved movement were not changed.

## Unresolved, not counted as validated matches

- Gallery probe (-15,18,16): reference left/right 9/3 m versus 1/31 m; ceiling
  4 m versus 10.8 m. This needs a coordinated gallery/ramp redesign, not isolated
  walls that would block the existing armory ramp.
- Armory center left/right: 3/3 m versus 10.07/10.08 m. Equipment can affect these
  rays. The actual alcove partitions and access openings need additional cuts.
- Spine (6,12,33): reference left/right 5.69/1.90 versus 18/6 m. Shaft geometry
  and surrounding corridors differ substantially.
- Defence and roof probes also differ; detailed distances are in both reports.
- Flag center pedestal hit differs by 0.32 m; this is not a floor-height error.
- Reference roof sensor is near (0.06,6.62,66.94), ours (0,0,67): elevation
  agrees, horizontal position does not. Base 1 has a slightly different offset.
- Reference sentry is near (0,27.46,51.05), **one** per base. Our two exterior
  plasma turrets at (±30,6,51) are not matched equipment placement. Matching
  only their height was insufficient evidence; no turret change in this pass.
- Generator model origins around 41 m cannot be substituted for room floor
  elevation. The room-floor probe confirms 38 m at one location; original model
  pivot/pedestal semantics remain unverified.

## Audit-tool corrections and validation boundaries

`measure-broadside.py` formerly merged triangles touching at just one vertex,
despite calling that a shared edge. It now requires two shared snapped endpoints,
with 1 mm snapping, normal-dot tolerance 1e-6 and plane tolerance 2 mm. Synthetic
tests distinguish point-touching from edge-adjacent triangles. This is still
approximate surface grouping: T-junctions, opposite winding and topology can
split panels; original authoring panels and doorway shapes are NOT recovered.
Bounding boxes spanning a gap do not prove that the gap is open.

`inspect-skybreak-fortress.py` also had its height and forward-axis bounds swapped;
the filter now matches the reference tool. Contrary to an earlier note, it does
not perform panel grouping or a panel-for-panel comparison.

Synthetic ray tests cover winding reversal, nearest hit, misses, parallel rays
and maximum distance. Rust checks cover measured hall/flag distances on both
rotated bases plus existing ramp support, headroom, door solidity/openings and
ceiling collisions. These are regression tests, not a fidelity percentage or a
full human traversal/playtest. All source-derived reports remain private.
