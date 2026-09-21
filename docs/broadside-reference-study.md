# Broadside measured reference study

## Current audit takes precedence

See [the reproducible interior audit](broadside-interior-audit.md) for the
2026-09-20 v7 results, including validated unchanged geometry, before/after
measurements and unresolved differences. It supersedes the later historical
claims in this file about a wide central flag-room doorway, exact equipment
placement and reconstructing exact authored panels. Those claims were not
supported by sufficient measurements. The exterior is unchanged in v7.

Reference: **Tribes 2 Classic Broadside_nef**, not proof of an exact Tribes 1
Broadside layout. The user requested decoding this locally installed reference
and approaching its layout/proportions with reusable, newly authored assets.

## Reproduce privately

Use the existing pinned io_dif reader documented in `raindance-import.md`:

```sh
local-assets/tools/venv/bin/python scripts/inspect-broadside-reference.py \
  --dif local-assets/tribes-map-catalog/tribes2/Classic_maps_v1/interiors/dbase_broadside_nef.dif \
  --output local-assets/reference-broadside-new
```

Output must be a new directory under ignored `local-assets/`. The first study is
`local-assets/reference-broadside-decoded/`: JSON dimensions/floor areas, horizontal
sections and two vertical cuts. These are source-derived PRIVATE diagnostics;
do not publish/package them. No textures are loaded. Mission fields are parsed
declaratively by the existing converter, never executed as scripts.

Highest-detail interior: 6,963 points, 3,612 surfaces and 9,129 nondegenerate
visible triangles after excluding NULL/ORIGIN/TRIGGER/FORCEFIELD materials.
Source SHA-256: `ae381077d2655b5c44ac86cd6bb1924f78ee2b9ec8fbcd942aa8efce5f34e55e`.
DIF uses Z-up; PeakRunner uses Y-up. Heights below are relative to flight deck.

## Measured targets versus authored v4

| Feature | Reference | Rebuilt asset |
| --- | --- | --- |
| Deck extent | 88 x 104 m | 88 x 104 m |
| Crown height | 67 m | 67 m |
| Lowest geometry | -67.875 m | -68 m |
| Major horizontal levels | -6, 0, 7, 14, 25, 31, 38, 45, 57 m | Same selected elevations |
| Flag anchor | approximately (0.14, -11.71), height 14 m | (0, -12), pedestal on 14 m floor |
| Inventory levels | -6, 7, 25 m | -6, 7, 25 m |
| Generator positions | two at rear, object origins around 41 m | two grounded on 38 m level |

These are dimensional comparisons, **not a 90% fidelity score**. Diagnostic
surface areas also include ledges and structural surfaces, not just walkable
rooms. The authored model is built from fresh primitive geometry and our original
material kit; it is not a DIF conversion with replacement textures.

## Remaining differences / acceptance

- Rounded/chamfered room corners, bevels, roof housing and keel cross-sections
  remain simplified. Recovery platforms and some ramp routes are adaptations.
- Reference generator mesh origin is elevated above its support; gameplay uses
  our equipment's own ground origin, not a copied source object transform.
- We retain two exterior plasma turrets per base; this is not the reference's
  exact plasma/sentry mix. Materials, trim, equipment models and lighting differ.
- Room connectivity is approximate, not every original doorway/hatch. A complete
  traversal comparison against the source game remains necessary before claiming
  near-90% overall fidelity. The current result is a measured structural pass.
- Source triangulation is never an input to `build-skybreak.py`. The reference
  reader and its local dependencies are optional analysis tools, not build deps.

Use core `fortress_` tests for sampled ramp support/headroom, ceiling collision
and flag access. Python prefab tests catch coplanar floor-box overlaps and validate
placement/ownership. GPU captures cover the principal rooms. Human match play,
bot navigation and complete route traversal are still separate acceptance gates.

## Tribes 1 Broadside — plaintext mission measurements

Reference: the user-owned Starsiege: Tribes disc image, mounted locally, giving
`local-assets/tribes1-disc/Tribes/base/missions/Broadside.MIS`. Unlike the
Tribes 2 DIF, Tribes 1 missions are declarative plaintext (position/rotation
fields per named object), so no binary interior/DTS decoder is needed and the
paired `.dis`/`.vol` geometry is never read.

Reproduce privately:

```sh
python3 scripts/inspect-broadside-tribes1.py \
  --mis local-assets/tribes1-disc/Tribes/base/missions/Broadside.MIS \
  --output local-assets/reference-broadside-tribes1-new
```

Output must be a new directory under ignored `local-assets/`. The parser only
reads `position`/`rotation`/`dataBlock`/`fileName` string fields with a regex;
it never executes mission script. It locates each base's `acommand.N.dis`
tower placement, then expresses every `Generator`/`AmmoStation`/
`InventoryStation`/`CommandStation`/turret/`Flag` object in that tower's
yaw-corrected local frame, so the two bases' layouts can be compared directly
even though their absolute mission-file transforms aren't mirror images.
Source SHA-256: `2eb0b8e6921df60db3527c16fc6334c4df28b5bd0d21d46608b713672fb2745e`.

### Measured local-frame values (both bases agree within noise)

| Feature | Tribes 1 Broadside | Rebuilt asset (v4) |
| --- | --- | --- |
| Base separation (tower to tower) | **297.8 m** | now 298 m (was a 640 m guess) |
| Flag height / offset from tower origin | ~15 m, ~14 m forward | 14 m, matches |
| Command station height | 14 m (same storey as flag) | 14 m, matches |
| Deck-level items (ammo/inventory) | ~0 m | 0 m, matches |
| Mid-level items (ammo/inventory) | ~9 m | closest authored storey 7 m |
| Upper items (inventory/ammo) | ~23.5 m | closest authored storey 25 m |
| Generators | ~32.7 m | 38 m (authored, grounded) |
| Roof turret / sensor apex | ~44–48 m | crown 67 m (authored has extra roof housing) |

The base-separation figure is the one clear, unambiguous correction: it comes
straight from the two `acommand` tower positions and doesn't depend on any
geometry decoding. `maps/skybreak-bastions.json` now places the two Skybreak
bases 298 m apart (was 640 m); `crates/core/src/terrain.rs`'s `ember`/`glacier`
constants and the `fortress_*` test fixtures were updated to match. The
per-storey height table is closer confirmation than a required fix: our 14 m
flag/command level already agreed with both Tribes 1 and the Tribes 2 DIF
before this pass, and the remaining gaps (generator/roof heights) are within
the range of variation already noted between the two source games, not a new
discrepancy — left as-is rather than chasing a third, non-matching number.

As with the DIF study, this is a private numeric diagnostic only. No Tribes 1
geometry, texture or mission text is copied into the map compiler; only the
derived distances above informed the authored asset.

## Silhouette/connectivity investigation (v4 is dimensionally close, shape is not)

The original v4 pass only compared floor-area histograms and the nine chosen
circulation heights; it never actually looked at the DIF's own floor-plan and
profile cuts it was generating. Re-examining
`local-assets/reference-broadside-decoded/floor-sections.png` (14 per-level
top-down cuts) and `vertical-sections.png` (two central profile cuts) shows the
real structure is not a rectangular box tower on a rectangular deck with a flat
keel cap — three real shape mismatches with the authored asset:

1. **Deck outline tapers**, it is not a constant-width rectangle. The central
   Y=0 profile cut shows the classic flared/winged "manta ray" hull silhouette
   at the deck line; our deck is built from stacked axis-aligned rectangles
   (`floating_fortress.py`'s `slab()` calls) with straight sides.
2. **The tower is a stepped/setback silhouette** (wider at ~14–20 m, narrower
   through 25–38 m, narrower again above 45 m, distinct crown above 57 m), with
   two small pylon-like nubs flanking it around z=25–30 m (`vertical-sections.png`,
   right panel) — likely the exterior turret mounts. The authored tower is a
   single constant-radius box shell from 32–57 m with a thin 1 m accent ring,
   not a multi-step taper, and turrets are point-mounted rather than on
   raised pylons.
3. **The keel comes to a point**, not a flat floor cap. `vertical-sections.png`
   (left panel) shows a continuous tapering wedge down to about -60 m with one
   small isolated chamber near y≈-40 m — not the authored asset's straight
   shell down to a flat `slab(-5,5,-5,5,-67)` cap.

Per-level floor plans also show real internal shapes worth matching more
closely than the current abstracted rooms: 14 m is a hollow ring/gallery
around a central room (matches the documented "flag + surrounding gallery"
concept reasonably well), 31 m and 57 m show a ring around what looks like a
lift/elevator shaft (circular gap at center), 38 m has flared arm extensions
plus a central ring, and 44 m shows a row of narrow mullions/columns
(parapet or louvered vent, not currently modeled).

`scripts/inspect-broadside-layout.py` additionally clusters raw wall/ramp
triangles into bounding boxes as a rough cross-check; treat its cluster output
as exploratory only — bounding-box merging can chain unrelated nearby
fragments into a misleadingly large box, so the per-level image cuts above are
the more trustworthy source. Its largest wall cluster (x=[-24,24], y=[-19.5,32],
z=[36,57], area 7473) does at least confirm the tower shaft is a full-width
enclosed box through that height band, not a thin ring.

None of this changes the license/asset position: only dimensions and shapes
inferred from these private renders inform `floating_fortress.py`; no DIF
triangles, materials or textures are ever read by the map compiler.

## Fixes applied from this investigation

Two separate problems were found and fixed, not one:

1. **A real regression, not a fidelity gap.** The base-separation fix above
   moved both Skybreak bases (704/1344 → 875/1173 on the z axis) but missed
   five hardcoded GPU-capture camera positions in `src/scene.rs` that were
   still framed for the old base at z=1344. Every `fortress-*` screenshot
   since that change was pointed at empty air, which looks exactly like
   "walls missing" at a glance. Fixed by shifting those five camera positions
   by the same -171 m the base moved; recaptured and visually confirmed real
   interior geometry (entrance corridor, flag room, armory, generator room)
   in all five.
2. **Silhouette gaps from the profile-cut investigation above**, addressed in
   `scripts/assets/floating_fortress.py`:
   - Added a flared hull skirt below the deck edge (`shell` from radius
     44x52 at the deck down and out to 50x58, then back to the keel taper),
     giving the winged silhouette the Y=0 profile cut shows instead of a
     plain box under the deck.
   - Split the constant-radius 32-57 m tower shell into two tiers: unchanged
     32-45 m, then a new 45-57 m taper down to a 14x16 m top radius, matching
     the real structure's narrower crown. The 57 m storey's floor/wall/ramp
     footprint was scaled in to fit inside the new radius (roof ramp now
     lands at z=18 instead of z=24 — see the matching `fortress_*` test
     fixture in `crates/core/src/terrain.rs`).
   - Added two flanking turret pylons at y=26 (`for side in [-1,1]:
     mesh.box((side*30,25.5,6),(4,1,4),'trim')`) and moved the two plasma
     turrets from deck-tip mounts onto them, matching the small pylon nubs
     visible flanking the tower in the Y=0 profile cut.
   - Tapered the keel fully to a near-point (a 1x1 m tip at y=-68) instead of
     a flat 10x10 m cap, matching the wedge shape in the X=0 profile cut and
     the measured lowest geometry (-67.875 m).

142 workspace library tests, the three `scripts/test-floating-fortress.py`
prefab tests, and all `fortress_*`/`skybreak_flags_*` Rust tests pass after
these changes. GPU captures were regenerated and visually inspected (not just
rebuilt). Remaining known gap: exact per-level room polygons (the ring/lift
shapes at 14/31/57 m, the arm shapes at 7/38 m) are still authored
approximations, not literal reproductions of every wall in the per-level
floor-plan cuts — matching those exactly would mean redoing most rooms from
scratch and was judged lower value than fixing the silhouette and the actual
regression above.

## Gameplay-footage correction (the v5 taper direction was backwards)

The profile-cut investigation above got the crown's taper direction wrong.
The user pointed at a public gameplay showcase,
`youtube.com/watch?v=1Kc9x2Vdhxo` ("Tribes Maps Showcase: Broadside", covering
Starsiege: Tribes 0:00-21:00 and Tribes 2 21:00-28:28), and actual footage is
unambiguous where a DIF profile cut is easy to misread: both games show a
**narrow neck rising from the deck that flares OUT into a wide hooded crown**
near the top, not a tower that keeps narrowing toward the crown. The v5 pass
had built the opposite — narrowing from 24x26 m at 45 m down to 14x16 m at
57 m.

Frames were pulled locally for reference (never committed, never redistributed):
`yt-dlp -f 18` downloaded the public video to a scratch directory outside the
repo, and `ffmpeg -ss <t> -frames:v 1` grabbed stills at several timestamps in
both the Tribes 1 and Tribes 2 segments. Both segments agree: narrow stem,
wide flared crown with a horizontal window/accent band partway up, a small
spire/antenna at the very top, and a comparatively shallow keel wedge below
the deck (our keel, grounded in the T2 DIF's own measured -67.875 m lowest
point, is deeper than the footage suggests — left as is since that number
came from decoded source geometry rather than an eyeballed video frame).

Fixed in `scripts/assets/floating_fortress.py`: the neck now stays a constant
24x26 m through the 38 m generator storey (unchanged, matches the footage's
narrow stem), then flares out to 40x46 m by 52 m (a wide crown, with four
small light accents standing in for the window band around y=48), holds that
width through the 57 m storey, and only then tapers to a small 6x6 m spire
base by 65 m for the mast/sensor. The 57 m storey's floor and turret pylons
did not need to shrink this time — the crown is wide enough to hold them with
room to spare, unlike the too-narrow v5 top. No `fortress_*` test coordinates
needed to change; the roof ramp and room floors were untouched, only the
exterior shell direction around them.

**This flare turned out to be wrong too — see the next section.** A
compressed public video is a weak source for silhouette proportions (lens/FOV
distortion, motion blur, no ground truth for exact widths); it correctly
flagged that v5's crown was too narrow, but the "flares wider at the top"
read was a misinterpretation. Superseded below by the in-engine reference.

## Corrected from the private in-engine reference pack (ground truth, not video)

`docs/broadside-private-reference.md` documents `PeakRunner-Broadside-Reference.app`
and its pack at `local-assets/broadside-reference`: the actual Tribes 2
Classic Broadside_nef geometry, already converted into PeakRunner's own
collision-mesh format (not a DIF needing a third-party reader, not a
video frame). This is by far the highest-confidence source used in this
whole investigation, and it disagreed with the video-based flare.

`scripts/inspect-broadside-inengine.py` reads that pack's `collision.bin`
directly, transforms triangles into a base's local frame using the pack's own
`reference_bases[0]` transform (local axis order is x/y horizontal, z
height — matching the same basis `src/scene.rs` uses to place its
`QA_REFERENCE` capture cameras), keeps only triangles near that base's local
origin (the collision mesh covers the whole map), and renders the same kind
of per-level floor-plan and central-profile cuts as the DIF study. Reproduce:

```sh
local-assets/tools/venv/bin/python scripts/inspect-broadside-inengine.py \
  --output local-assets/reference-broadside-inengine-new
```

The central profile cuts show the tower **narrowing in tiers as it rises**,
not flaring: roughly 30 m half-width from the deck through 14 m, a step down
to about 20 m half-width from 21-40 m (with the two turret-pylon "vase"
shapes visible flanking it around 28-32 m), a trim lip/cornice around 40 m,
a further step down to about 14-16 m half-width from 42 m up through 63 m,
then a small cap and thin mast above that. The keel already matched well: it
tapers to a real point near -68 m, with a small isolated chamber floating
around -40 m.

Corrected in `scripts/assets/floating_fortress.py`, replacing the flare
entirely:

```
shell(mesh,(32,40),(24,26),0,32,...)      # unchanged neck
shell(mesh,(24,26),(24,26),32,45,...)     # unchanged, holds existing 25/31/38/45 m rooms
shell(mesh,(24,26),(15,17),45,52,...)     # NEW: taper down, not out
shell(mesh,(26,28),(26,28),40.5,41.5,...) # NEW: thin cornice lip at the transition
shell(mesh,(15,17),(15,17),52,63,...)     # NEW: narrow tier, holds the 57 m room
shell(mesh,(15,17),(6,6),63,70,...)       # NEW: cap taper to the mast
```

The 45/57 m room floors and the roof ramp did not need to change — the new
tiers still comfortably contain them (more clearance than the old flare gave,
in fact). GPU exterior captures were regenerated and visually compared side
by side with `screenshots/broadside-reference-exterior.png`; the stepped
narrowing silhouette now matches. All 142 workspace tests and the 3 Python
prefab tests still pass, unchanged.

Lesson for future passes on this asset: prefer the private in-engine
reference or the DIF study's own generated section images over gameplay
video for anything about exact proportions or taper direction. Video is fine
for spotting things like "there should be a trim lip here" or general vibe,
but not for which way a wall leans.

## v6 verification pass and the south-face notch

A parallel v6 pass (see `docs/floating-fortress.md`) independently re-examined
the reference and concluded the outer tower envelope holds its ~24x26 m width
all the way to the 57 m terrace, with a separately tapered roof housing above
that — contradicting this document's own three-tiered-taper conclusion from
the section above. Re-checked both at full resolution (not the low-res
contact-sheet thumbnails used earlier in this document): v6 is correct. At
h=42 and h=49 the reference's outer wall-cut sits at essentially the same
-26/+23 m extent as at h=14, not narrower; the earlier "narrows to ~20 m by
21 m" reading was a measurement error from viewing thumbnails too small to
place the gridlines accurately. `scripts/inspect-skybreak-fortress.py` (new)
runs the identical floor-plan/profile-cut analysis as
`inspect-broadside-inengine.py` but against our own compiled pack, so the two
can be diffed directly rather than eyeballed from memory; at h=42/49/21 our
envelope now lands within about 2 m of the reference's, and the 14 m flag
ring's shape and side-well proportions match closely too.

One clear, recurring gap survived that comparison: the reference's own
wall-cuts show a step-back notch on the south (entrance) face at the deck,
14 m and 21 m levels — not present in our asset, which had a flat unbroken
south edge at those heights. Fixed at the deck level only (0 m) in
`floating_fortress.py`: the south deck cap is now three slabs (two full-depth
wing pieces plus a shallower centre piece) instead of one flat slab, giving a
rectangular step-back where the reference has a bevelled one. The 14 m and
21 m notches are not yet replicated — call this the next concrete gap if
continuing this pass. This is purely an outer-edge change, isolated from
every tested ramp/door coordinate; all 143 workspace/prefab tests still pass
unchanged.

## Exact structural measurement, not pictures to eyeball

Everything above this point — including this document's own earlier
self-correction — was ultimately a human (or model) reading pixel positions
off a rendered image, which is exactly how the "outer envelope narrows at
40 m" measurement error happened. `scripts/measure-broadside.py` replaces
that with an actual geometric reconstruction: it unions the reference pack's
collision triangles into connected components using mesh topology (two
triangles merge only if they share a vertex *and* are coplanar — matching
normal and plane offset), which reconstructs each panel exactly as the
original mapper authored it. A wall split by a doorway stays two separate
components with a numeric gap between them; nothing needs to be read off a
picture. Reproduce:

```sh
local-assets/tools/venv/bin/python scripts/measure-broadside.py \
  --output local-assets/broadside-measured-new
```

Output is `panels.json`: for every panel, its kind (wall/floor/ceiling/ramp
by normal), exact x/y/height extent, area and triangle count, sorted by area.
1,211 panels survive a 1 m² noise filter on the first base (13,515 nearby
collision triangles). `scripts/inspect-skybreak-fortress.py` was extended
with the same connected-component logic so our own compiled pack can be
measured identically and diffed panel-for-panel against the reference,
instead of comparing renders by eye.

This immediately found a real, sizeable error the picture-based passes
missed: the reference's keel is a wide sloped hull funnel (radius 32 m at
-8 m tapering to 24 m by -24 m — an exact match, this part was already
right) wrapping a much narrower ~5.5-7 m internal shaft that continues
almost straight down to a near-point around -68 m. Our keel kept widening
well past where the reference's funnel stops, bulging out to a 36 m radius
around -35 m — nothing in the reference matches that width at that depth.
Fixed in `floating_fortress.py` with the exact measured radii:

```
shell(mesh,(32,40),(44,52),-8,0,'trim')       # deck-edge fairing, unchanged
shell(mesh,(24,26),(32,40),-24,-8,'concrete') # hull funnel: matches measured 32->24 m
shell(mesh,(7,7),(24,26),-30,-24,'concrete')  # transition to the internal shaft
shell(mesh,(5.4,5.4),(7,7),-42,-30,'concrete')# shaft, matches measured ~5.5-7 m
shell(mesh,(1,1),(5.4,5.4),-68,-42,'trim')    # taper to the near-point at -68 m
```

Known remaining gap in this same data: the reference's internal shaft is not
round, it's an elongated slot (±7 m in x but extends to about ±32 m in y,
i.e. it runs almost the full length of the keel, not a small shaft under the
flag room). Our corrected version is still a uniform ±7 m taper in both
directions — right radius, wrong shape. The reference data also shows two
small side chambers around x=±5-16 m, height -42 to -45 m, flanking the
shaft, not yet modelled at all. All 143 workspace/prefab tests pass with the
radius fix (it doesn't touch any tested coordinate); GPU exterior capture
regenerated and shows a visibly tighter, more pointed keel silhouette.

Not yet run against interior rooms (25/31/38/45/57 m) or doorway-gap
detection — the panel data supports both (a doorway is just two coplanar
wall components with a horizontal gap between their extents), this is
scoped to what got checked this pass, not a limit of the tool.

## Interior room pass using the same exact measurements

Continuing with the flag room (14 m) and generator storey (38 m), the two
most important interior spaces:

**Flag room doorway.** The reference's measured wall panels at this level
show a single wide gap (about 24 m) in its gallery's front/back walls, where
our asset had three separate narrow gaps (`(-24,-18)`, `(-12,-5)`, `(5,12)`,
`(18,24)` solid, meaning gaps only at `-18..-12`, `-5..5`, `12..18`). Fixed
in `floating_fortress.py` to one wide gap. First attempt used the reference's
exact `-12..12` gap and broke a different, already-passing test: the loft
corridor route samples points at x=+-15 through this same wall plane (a side
connector, unrelated to the front doorway), and `-12..12` doesn't reach that
far, so it walled off a working corridor. Widened to `-18..18` instead —
covers both the reference's doorway *and* the tested corridor at x=+-15.
Corresponding test in `crates/core/src/terrain.rs`
(`fortress_storeys_have_solid_ceilings_and_walkable_upper_rooms`) updated:
checks x=[-10,0,10] are open (was x=[-15,15], which is now itself open and
would no longer distinguish "gap" from "wall") and added x=[-20,20] must
still be solid, so the test can't silently pass with no wall left at all.

**Generator storey (38 m).** Reference floors here cluster into a ~16 m-wide
central spine (x=-8..8) flanked by side rooms around x=-20..-15 / 15..20.
Our authored storey already uses a ~24 m-wide central area (x=-12..12) and
side rooms x=-23..-12 / 12..23 — same shape, proportionally close (within
about 4-5 m), not touched this pass.

All 143 workspace/prefab tests pass with the doorway fix; GPU captures
regenerated and the flag room now visibly opens into a single wide gallery
connection instead of three separated slots.

## Equipment placement and the entryway/hallway, from the mission's own objects

The reference pack's `map.json` also keeps the parsed mission objects (not
just collision geometry) under `objects`: every `StaticShape`/`Turret`/etc.
with its exact `dataBlock`, `position` and `rotation`, straight from the
source mission text (see the `instances` array's `InteriorInstance` entries
for the matching per-base position/rotation to convert those into local
coordinates — they're in the original Torque Z-up mission space, a
*different* space from `reference_bases`' engine-space transform used
everywhere else in this document; don't mix the two).

Checked every `StationInventory`/`GeneratorLarge`/`SentryTurret`/
`SensorLargePulse`/`RepairPack`/`FLAG` object's local position and height for
one base:

- **Inventory stations**: exactly 6 per base, at heights -6, 7 and 25 (two
  each) — our own scheme (`(-13,-6,2)`/`(13,-6,2)` alcove pair,
  `(-9,7,10)`/`(9,7,10)` hall pair, `(-12,25,-13)`/`(12,25,-13)` armory pair)
  already matches this exactly. No change needed; good confirmation the
  entryway and hall equipment counts/heights were already right.
- **Roof sensor**: measured height 66.94 m against our `(0,67,0)` — a 0.06 m
  difference, i.e. already correct.
- **Flag**: measured height 14.72 m against our 15.05 m — already correct
  within margin.
- **Generators**: measured height 41.11 m against our 38 m generator storey —
  a real 3 m difference, *not* fixed this pass. Low confidence it's worth
  chasing: the measured position is the equipment model's own origin, which
  could sit on a pedestal above its room's actual floor rather than
  indicating the room itself should move, and moving the room would touch
  tested ramp/ceiling heights for uncertain benefit.
- **Sentry turret**: measured height 51.05 m against our previous 26 m — a
  real, large (25 m) difference with no similar ambiguity (turrets aren't
  tied to a room floor the way generators are). Fixed: turret pylons and
  equipment moved from y=26 to y=51 in `floating_fortress.py`. The tower's
  radius is still the constant 24x26 m of the main shell at that height, so
  the pylon at x=+-30 stays correctly outside the wall. GPU exterior capture
  now shows the turret mount near the crown instead of the mid-tower neck.

**Entryway/hallway corridor**, checked against the reference's measured wall
panels rather than the objects list: the reference's entrance corridor walls
run x=+-5 (10 m wide) continuously from about y=-33.5 to y=22.5 (56 m long,
i.e. from the deck edge to the equipment hall). Our own entrance corridor
(`wall` calls at x=+-5.5, spanning z=-35 to 22, 57 m) already matches both
the width (within 1 m) and length (within 1 m) closely. No change made here
— this was verification, not a fix.

All 143 tests still pass after the turret move; nothing else in this section
required a code change, only a check.
