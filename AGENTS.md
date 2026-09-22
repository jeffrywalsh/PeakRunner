# PeakRunner project guide

Read this before changing code or infrastructure. Active Git repository:
`/Users/jeffrywalsh/workspace/peakrunner`. Rust/build workspace: its `src/`
directory. Run Cargo, packaging, website and Docker commands FROM `src/`.
The old checkout and legacy browser prototype remain at
`/Users/jeffrywalsh/workspace/PeakRunner.bak`. Do not edit/run the backup.

## Three-directory layout — supersedes historical paths below

- `src/`: complete build inputs: Cargo manifests/lockfile, client `src/`,
  `crates/`, `.cargo/`, original `assets/`, map inputs, scripts, tools, deploy,
  site and examples. Keep required non-code build assets here.
- `docs/`: documentation and historical validation notes.
- `research/`: ignored private assets, captures, app bundles and secrets.
  Never stage wholesale or force-add. Only its README is tracked.
- Root retains README, AGENTS, ignore and Git metadata; no other content dirs.
- Local-only compatibility links `src/local-assets`, `src/screenshots`,
  `src/docs` point into research/docs. Public compilation needs none of them;
  Docker excludes them. Never embed imported private map packs.
- `cf-key` moved to `research/secrets/cf-key`; helper default updated.
- Package README inputs now live in `src/assets/package-docs/`; synchronize
  with canonical docs when changing release guidance.
- Older commands assume the native workspace: execute from `src/`. Historical
  absolute PeakRunner/peakrunner paths and app locations are no longer current.
- Git history is on `main` only. Local `main` and `origin/main` match.
  There are no other branches, stashes, or worktrees. See `docs/workspace-layout.md`
  for the directory move; ignore any older note there about a stash or review worktree.
- Linux migration validation passed on dellcon (2026-09-21): release client
  build plus isolated Xvfb/Mesa local-match rendering smoke, visually checked.
  See workspace-layout for image provenance, capture and limitations. This was
  not a deployment; real GPU performance and audio playback remain untested.

## Six-map rotation

Raindance, Skybreak Bastions, Broadside Clone, Stonehenge Clone, Snowblind
Clone, and Desert of Death Clone are the server rotation. Raindance and
Skybreak are original PeakRunner maps. The four clones are reference layouts
brought in so the bases and routes start in the right place. The next work is
to change their geometry and art until the maps are PeakRunner's own. Valley
stays an internal fixture.

There is no private-test switch. A dedicated server loads a reference map when
its pack is installed and rejects that map when the pack is missing.
`PEAKRUNNER_PRIVATE_MAPS_DIR/<map-key>` is where the server reads those packs.
Packaged clients find them in `private-maps/` beside the executable, or in
macOS `Contents/Resources/private-maps/`. Development builds also read
`local-assets/<map-key>/`. Client packaging copies the four packs by default.

The published game `0.1.0-private.20260921.1` was built before this removal.
That server image still checks `PEAKRUNNER_PRIVATE_TEST`, and the running VPS
Compose still sets it. This source change is not deployed. Do not add a
firewall or password. The existing endpoints stay publicly reachable. Preserve
TLS verification and infrastructure hardening.

### Scope and source

- Active checkout: `~/workspace/peakrunner`; all build commands run in `src/`.
  Prior native conversation recovered from thread
  `01a0b61d-e052-7db0-bae8-9e9364058f30`, originally at
  `~/workspace/PeakRunner/peakrunner`. The parent browser project is historical;
  do not build/deploy from `PeakRunner.bak`.
- Published version: `0.1.0-private.20260921.1`. Keep launcher
  `0.1.0-r1` unless verification establishes that it needs a change. Select a
  signed feed sequence greater than all currently published sequences.
- Server-selected CTF rotation, in order: **Raindance, Skybreak Bastions,
  Broadside Clone, Stonehenge Clone, Snowblind Clone, Desert of Death Clone**.
  Valley remains retired/internal. Empty-server reset returns to Raindance.
- For this test the user authorizes deploying the four installed source-derived
  runtime packs alongside clients and server. Keep them in ignored research and
  release storage; do not commit/push imported packs, captures, source archives,
  credentials or signing keys. This is not a general original-assets-only release.
- Original Raindance/Skybreak build inputs remain in `src/assets/maps/`.
  Private inputs via `src/local-assets`: Broadside `broadside-clone/compiled-donut-v1`;
  Stonehenge, Snowblind and Desert of Death each `<map-key>/installed`.
  Copy only `map.json` and its six verified runtime payloads, not editable source,
  mission archives, prior packs, app bundles or the whole research directory.

### Steps in simple terms

1. Save the current server and website settings so we can roll back.
2. Prepare all six maps and make matching game/server builds.
3. Test joining, changing maps, rendering and launcher updates.
4. Put the new server and map packs on the VPS; switch when nobody is playing.
5. Put matching downloads and signed updates on dellcon, then update the website.
6. Check the live server, downloads and launcher from outside the network.
7. Write down exactly what shipped, how to recover, and what still needs playtesting.

**These deployment steps are complete for `0.1.0-private.20260921.1`.**
Do not repeat the deployment simply to complete this checklist. A read-only
follow-up confirmed both pinned images healthy and the six-map rotation configured;
one player was connected, so no further restart or WAN slot test was performed.
Detailed execution and completion evidence:
[six-map release runbook](docs/release-20260921-1.md#detailed-execution-steps).
Human balance/traversal, native Windows runtime and physical Linux GPU/audio
testing remain separate playtest work.

### Implementation and deployment steps

1. Inspect Git status and preserve existing work. This release was later
   committed and pushed as `a4d3f9b`. Check live host health, version/protocol
   and occupancy through the existing
   directory/status endpoints and narrow Docker inspections.
2. Load reference packs from `PEAKRUNNER_PRIVATE_MAPS_DIR/<map-key>`. Fail
   startup when a configured reference pack is missing or invalid. Packaged
   clients locate the same packs in `private-maps/` or in macOS
   `Contents/Resources/private-maps/`. Preserve existing local development paths.
   Do not require `PEAKRUNNER_PRIVATE_TEST`.
3. Include each installed imported pack's identity/fingerprint in gameplay
   compatibility so missing/mismatched test collections are rejected before a
   player slot is granted. `.20260919.4` / `map1` clients are incompatible.
   Do not present combined-collection compatibility as selected-map-only admission.
4. Verify normal workspace library tests, all-target compilation, app boundaries,
   private collection rotation with connected clients, late joins and empty reset.
   Socket tests require local networking. The connected rotation test needs an
   explicit absolute `PEAKRUNNER_PRIVATE_MAPS_DIR`, since Cargo tests run from
   crate directories.
   Render and inspect actual map transitions; keep human balance/traversal and
   platform limitations honest. Do not change approved movement physics.
5. Stage maps with `python3 scripts/stage-private-maps.py ABSOLUTE_NEW_DIRECTORY`.
   It verifies manifest hashes and refuses overwrite. Build matching Mac ARM64,
   Windows x64 and Linux x64 clients. Build Linux/server on dellcon, not the VPS.
   Record exact source archive hashes, build provenance and immutable image IDs.
   Keep private map transfer separate from the source-only Docker build context.
6. Package standalone clients with `scripts/package-client.sh`; the script
   copies the four reference packs, and Mac maps must be copied before bundle
   signing. Stage signed feeds with `scripts/stage-launcher-release.sh`.
   Managed layout is
   `game/private-maps/<map-key>/`, external original Raindance is `map/`, and
   original Skybreak remains embedded. Signing key stays on the admin Mac.
   Launcher r1 rejects zero-byte files. Omit only empty `ambient.f32` files from
   managed payloads: the loader must accept absence only when the manifest's
   SHA-256 proves empty audio. Keep standalone/server packs byte-identical.
   Verify signatures, hashes, fresh install, upgrade, repair/rollback and actual
   managed-game launch. No claim of independent Skybreak pack updating.
7. Stage matching server image and versioned map directory on the VPS. Mount the
   collection read-only at `/opt/peakrunner/private-maps` in the container. Keep
   eight slots, TLS/QUIC UDP 7777, current certificates, limits and non-root user.
   Configure the six-entry rotation in `src/deploy/vps/compose.yaml`.
8. Before cutover, back up current Compose/release/source records and signed
   platform manifests on their hosts. Retain previous images and downloads.
   Recheck occupancy immediately; coordinate any restart with connected players.
   Update the matching server before publishing incompatible clients using
   `sudo -n docker compose --env-file release.conf up -d --no-deps match`.
   Verify health, advancing tick, name, selected map, version and protocol.
9. Put standalone archives and signed blobs on dellcon; verify remote hashes.
   Publish signed platform manifests atomically only after all blobs exist and
   server verification passes. Update website manifest descriptions/checksums
   to describe this six-map test honestly. Build/pin the website image; recreate
   only `site` with the documented dummy-token command. Do not recreate the
   directory/tunnel or touch unrelated dellcon services.
10. Verify apex/www HTTPS, directory details, desktop/mobile download UI, every
    advertised archive hash and signed feed/blob. Run a small encrypted WAN
    client check only when empty. Verify connected rotation and actual rendering;
    leave unverified human/Windows/GPU/audio claims explicitly pending.
11. Record deployed versus staged state, source/image hashes, feed sequence and
    rollback paths in this guide and `docs/release-20260921-1.md`; synchronize
    package release notes. Rollback restores a compatible server/client set;
    launcher recovery needs a newly signed higher sequence, not a downgrade.

### Published checkpoint — 2026-09-21

- **Deployed and published:** game `0.1.0-private.20260921.1`, launcher remains
  `0.1.0-r1`, signed feed sequence `2026092102`. Standalone archive names use
  package revision `r2`. All six maps are configured in the CTF rotation above.
  Restart occurred with zero players after a second occupancy check.
- VPS match image:
  `sha256:d801944ba2641ce9aafc6c4eb991aab131e7cb59b26671571a0d88a24f86b40d`.
  Website image:
  `sha256:27d69f3af6be7598b09918626be5cbf9510e58f5c6d0ebcd70fcecddfa1cf0b2`.
  Directory and tunnel were not recreated. No firewall/DNS/password change.
- Runtime maps are versioned at
  `/opt/peakrunner/private-map-releases/20260921-private1`, with
  `/opt/peakrunner/private-maps` selecting that directory and mounted read-only.
  Exact archives/build records are under
  `/data/peakrunner/releases/20260921-private1/` on dellcon.
- Server/client source archive `source-v3.tar.gz` SHA-256:
  `9f38581e9a63897b7e3930c038f380d3a6deb76aa188c5271d58fae4bdd38050`.
  Separate `private-maps.tar.gz` SHA-256:
  `e89328f4b2834c12d0d8d39052cbf59793b7c00989bda42eeca5a989eea732cb`.
  Saved server image SHA-256:
  `43efb09c5e449fe7d9266c6c0edf5d61bb7fa088fbac897e092d954b01d6f519`.
  Final website archive `site-final-r2.tar.gz` SHA-256:
  `c2bdbe7fc06367a89bc741d981d613710239087b69380710b29bbeea71410906`.
  The deployed binaries were built from that working-tree archive, based on
  `a4f00b5`, before the release was committed. Git records it on `main` at
  `a4d3f9b` ("Record the deployed six-map private test"), pushed to
  `origin/main`. That archive remains the build provenance; the image was not
  built from a clean checkout of `a4d3f9b`. Guide edits after that commit are
  not in the deployed image.
- Linux final clients used the verified base
  `peakrunner-layout-validation:20260921` (ID recorded above), with the saved
  `final-client.Dockerfile` exporting only deliverables. Recipe SHA-256:
  `5407dcece2ecc6bfa4734b11ca62ded6c6cf97eb40389050c2b4c50d357a176d`.
  Do not pass a bare `sha256:...` as Docker FROM/BUILD_BASE: it was interpreted
  as a registry name. The earlier full compiler-image export was slow; prefer
  an artifacts target. `20260921-public1`, source-v2 and feed sequence
  `2026092101` were superseded stages, never published as this release.
- Verification: 152 workspace library tests; all-target and WASM checks
  (existing unused Lobby warning); app boundaries; packaging script parsing.
  The omitted-audio regression rejects missing nonempty audio and corruption.
  Connected local clients rotated through all six maps and back, retained
  identities, accepted a late join and reset after departure. A single GPU
  renderer switched across all six maps and back; captures visually inspected.
- Mac: every map rendered in an isolated real local match. Signed install,
  actual client/update lock, .4 upgrade, rollback, downgrade rejection and
  corrupted-map repair passed. A fresh installation from the published feed
  also launched and held/released its lock.
- Linux: signed install and actual client/update lock passed under Xvfb/Mesa.
  Stonehenge joined/rendered using the installed managed packs (including omitted
  empty audio); capture inspected at
  `research/screenshots/private1-linux-stonehenge.png`. ALSA reported no device:
  audio and real-GPU performance remain untested. Windows was cross-built,
  not newly runtime-tested. Human traversal/balance and extended matches remain
  playtest work; local accelerated rotation is not a full live human match.
- New VPS passed healthy/advancing-tick checks and a two-client, ten-second
  certificate-validated QUIC WAN test: 200 snapshots/client, maximum ack gap
  eight ticks, maximum snapshot gap 81 ms. Isolated image check used ~35 MiB
  idle within the existing 384 MiB/one-CPU limits; no larger capacity claim.
- All three live signed manifests and every blob passed verification; manifests
  return `Cache-Control: no-store`. All six advertised archives matched their
  hashes when downloaded over HTTPS. Apex/www advertise the new version.
  Desktop/mobile host dialogs/downloads passed without overflow or application
  errors. The existing CSP blocks Cloudflare's injected analytics beacon; this
  specific console warning is recorded, and the CSP was not weakened.
- Rollback backups:
  `/opt/peakrunner/backups/20260921-private1/` and
  `/data/peakrunner/public/backups/20260921-private1/` (includes old platform
  manifests). Retain old images, blobs and downloads. Recovery must restore a
  compatible server/client set and use a higher signed feed sequence.
- Detailed provenance/hashes: `src/deploy/launcher-release.json`,
  `src/deploy/client-release-staging.json`, `src/deploy/vps/source.json`,
  `src/deploy/vps/release.conf`, `src/deploy/dellcon/site-source.json` and
  `docs/release-20260921-1.md`. Live state must still be rechecked for future work.

## Working rules

- Inspect `git status` first. Preserve unrelated/uncommitted work. Do not commit,
  push, merge, delete branches or reset files unless the user requests it.
- Use `rg` and `apply_patch`. Legacy `.grok` instructions are archived in the
  backup, not this native project's contract. Keep gameplay/UI in Rust/egui.
- Make focused changes. Leave the playtested movement physics alone unless asked.
- Never claim a build is deployed, a platform is runtime-tested, or configs are
  backed up to GitHub without verifying that specific state.
- Keep this guide and the linked runbooks updated when architecture, commands,
  deployment paths or release procedures change. Live state must still be checked.

## Architecture and ownership

| Component | Location | Responsibility |
| --- | --- | --- |
| Native client | `src/` | Rust, eframe/egui, wgpu, audio, input, prediction and UI |
| Launcher/updater | `crates/launcher/` | Independent native launcher; signed file-level updates, repair and rollback; see `docs/launcher.md` |
| Shared simulation | `crates/core/` | Movement, combat, map data, equipment, authoritative match rules |
| Gameplay protocol | `crates/protocol/` | Messages, compatibility marker, snapshot packets |
| Client networking | `crates/net/` | Client sessions and QUIC transport |
| Match server | `crates/server/` | Authoritative simulation, admission, validation, TLS/QUIC |
| Discovery contract | `crates/discovery/` | Host adverts, status queries, transport helpers |
| Directory app | `crates/directory/` | Lists configured hosts and their live status; no gameplay core |
| Public website | `site/` | Static HTML/CSS/JS, Caddy, download manifest and live host UI |
| Original map kit | `maps/`, `assets/maps/`, `scripts/` | Editable source and compiled original assets |

Preserve app boundaries: directory must not depend on core/protocol/rendering;
server must not depend on client graphics/audio; client must not embed the server
or directory application. Test-only dependencies are intentional. Verify with
`node scripts/check-app-boundaries.mjs`. See `docs/apps.md`.

Next-work branch order and merge gates are in `docs/roadmap.md`. Keep each map
and feature scoped separately; update branches from tested main between portions.
Find a Rift preferences are in the published `.20260921.1` client. See
`docs/client-preferences.md`. Never store passwords.

## Historical build notes

These notes record how the maps and the checkout were built. Current
publication, rotation, and git state are in the six-map section above. `main`
is the only branch, locally and on `origin`. There is no `map/skybreak-fortress`
branch, no recovery stash, and no review worktree. The live game is
`0.1.0-private.20260921.1`, launcher `0.1.0-r1`, feed `2026092102`. Sentences
below that call the clones offline-only, say nothing is deployed, or name
`.20260919.4` as the live download describe the state before that publication.

- 2026-09-21 collection extension: **Snowblind Clone** and **Desert of Death
  Clone** are installed in the normal native game's wrapped map menu, at
  `local-assets/snowblind-clone/installed` and
  `local-assets/desert-of-death-clone/installed`. Both use the NEW native readers
  and each has an original donut poster. They remain reference layouts.
  Current code loads them when the packs are installed. The published server
  image from before that change still has the old startup check.
  Broadside/Stonehenge installed assets were not replaced by this extension.
  See `docs/collection-snowblind-desert.md` for builds,
  validation, omitted source features and exact private-pack paths.
  `stonehenge.py --profile snowblind|desert-of-death` now supports profiles;
  editable exports record profile and fixed poster anchor. Three terrain layers
  are padded with a zero-weight fourth channel. DTS sorted cluster words are
  preserved as integers because unused leaf-plane bits can represent NaN.
  `always-on` equipment circuits explicitly support maps without generators;
  other circuits still require their same-team generator. No artificial generator
  objects are added. Neutral repair prop remains visual-only, not team-assigned.
  QA: 19 Python tests, workspace library tests, all-targets check, app boundaries,
  private spawn/wall tests, GPU exterior/donut/spawn views, real menu-to-match
  capture for each map, CTF pickup/capture and map reset. Both editable rebuilds
  reproduce all six runtime payloads; poster resizing changes six corners only.

- Mission collection parser implemented independently in
  `tools/broadside_clone/mission.py` with four passing tests. Output:
  `local-assets/broadside-clone/missions-v1`: 79 parsed / 5 unsupported of 84.
  Stonehenge parsed 163 objects, seven interior instances across four DIFs,
  two flags/eight spawn regions. Missing direct dependency: Stonehenge_nef.nav.
  See new pipeline README for exact errors/limits. These are parsed mission
  trees, NOT 79 converted playable maps. Stonehenge alone now has a playable
  native import (below). Never reuse the old converter or execute mission scripts.

- Fresh TER v3 codec: `tools/broadside_clone/terrain_file.py`; three synthetic
  tests in `test_terrain_file.py`. Stonehenge terrain decoded to
  `local-assets/stonehenge-clone/terrain-v1` and re-encoded through editable
  JSON/PNG with exact full-file equality (460,306 bytes, 65,536 heights,
  four material layers). Embedded editor scripts remain inert base64 data.
  Only TER version 3 is supported. That codec test alone does not validate
  rendering; separate integration checks now cover Stonehenge below.

- Stonehenge private playable checkpoint (2026-09-21): new independent
  `interior_file.py` (DIF resource44/interior0) and `shape_file.py` (static DTS
  v19–23) feed `tools/broadside_clone/stonehenge.py`. Do not route these through
  old conversion tools or approximate prefab buildings. Installed pack:
  `local-assets/stonehenge-clone/installed` (v6 material-flags correction,
  rebuilt from the previous editable export; prior pack preserved).
  Normal client offers **Stonehenge Clone** when its pack is installed.
  Dedicated servers do the same, with no separate opt-in. The published
  `0.1.0-private.20260921.1` image still has the old startup check.
  Seven building instances, 100 scenery instances, 18 equipment instances;
  88,925 render / 3,603 collision triangles. Source BSP clips 217 terrain cells
  to prevent hills intruding into rooms. Source winding uses triangle strips,
  NOT fans; embedded lightmap UVs are already normalized (no 256/size scaling).
  Editable JSON/PNG rebuild needs no native DIF/DTS/TER/MIS files. Six runtime
  payloads reproduce byte-for-byte; a poster size edit changes exactly six
  render corner positions without changing collision/terrain/textures/audio.
  Run `verify_stonehenge.py local-assets/stonehenge-clone/installed` with the
  private tools venv to repeat. See `docs/stonehenge-private-import.md` for
  commands, actual validation and limitations. Source-derived art/geometry
  remain private/ignored, NOT independently authored public assets.

- Option 1 collection work: normal native client now offers `broadside-clone`
  as a separate MapId when run from the workspace with
  `local-assets/broadside-clone/compiled-donut-v1/map.json` installed. No env
  override or source assets embedded. Raindance/Skybreak unchanged. This is
  current client code, NOT the frozen Reference executable. Dedicated servers
  load it when the pack is installed. Inventory
  from the NEW `tools/broadside_clone/inventory.py` found 84 extracted mission files; report
  at `local-assets/broadside-clone/mission-inventory.json`. Inventory is not
  conversion. Native T1/T2 importers for further maps remain to be implemented
  under the new pipeline; don't invoke old converters without user approval.

- First approved clone scene edit: `tools/broadside_clone/add_donut.py` adds a
  donut picture on Base 1's entrance-hall wall between the ramps. Test app is
  `PeakRunner-Broadside-Clone-Donut.app`; baseline apps unchanged. Two appended
  triangles, one texture layer; original vertices and collision preserved.
  Placement visually checked with GPU diagnostic; packaged renderer is still
  the approved Reference executable. See new pipeline README for reproduction.

- New user-requested `broadside-clone` pipeline is `tools/broadside_clone/`.
  DO NOT reuse old map builders/converters for this task. New codec decodes the
  approved Reference app's payloads to explicit editable JSON/JSONL and PNG mip
  layers and independently recompiles all six binaries with exact hash matches.
  `PeakRunner-Broadside-Clone.app` uses those compiled files and the byte-identical
  approved executable. Three tests and package comparison/signature checks pass;
  visual gameplay confirmation pending. See `tools/broadside_clone/README.md`.
  This is exact reconstruction of converted runtime data, NOT a fresh native
  DIF parser, not independently authored art, and not the old procedural map.

- 2026-09-21: The ONLY user-approved baseline is the complete existing
  `PeakRunner-Broadside-Reference.app`, including its executable. An exact
  `ditto` duplicate is `PeakRunner-Broadside-Working-Copy.app` (ignored/private).
  Recursive file comparison and strict signature verification pass. Do NOT
  substitute `cargo run`, a regenerated pack, or the procedural fortress.
  The Reference binary SHA256 is
  `a4b69054255fca7dfda70d570db755c9454e70a535121df5a26aec108a049a69`;
  it differs from current target/release. Original pack payloads already match
  the previous workshop: binary/render behavior still needs investigation.
  Source revision for that binary is not established. The unchanged copy also
  retains the Reference bundle ID and preferences path; use an explicit app
  path to launch. No visual additions until the exact copy is confirmed.
  This supersedes all experiment-selection instructions below.

- User rejected returning to the procedural fortress/docking prototype. Current
  direction is SMALL ADDITIONS to the approved private Broadside baseline, not
  swapping buildings. `scripts/build-broadside-entrance-lights.py OUTPUT` creates
  a separate private pack with four original decorative markers (96 triangles).
  `local-assets/broadside-entrance-lights-v1` is the current test copy. All old
  vertices, collision and texture mip pixels are verified unchanged. Imported
  assets still make this private-only. See `docs/broadside-workshop.md`.

- Optional original docking experiment: `scripts/build-skybreak.py OUTPUT --docking`
  adds `scripts/assets/docking_bay.py` to both original bases, without changing
  default Skybreak or imported Broadside. Test pack is under local-assets;
  see `docs/floating-fortress.md`. Five asset tests and GPU interior capture pass;
  human traversal and balance remain pending. This is not deployed.

- Latest Broadside direction: **clone the working reference first, then modify**.
  `scripts/build-broadside-workshop.py` creates ignored
  `local-assets/broadside-workshop`, preserving all six reference payloads
  byte-for-byte. Bundle with `sh scripts/bundle-broadside-reference.sh --workshop`;
  play `PeakRunner-Broadside-Workshop.app`. Do not confuse this with original
  Skybreak or the immutable Reference app. See `docs/broadside-workshop.md`.
  Private editable exchange: `scripts/broadside-editable.py` exports one exact
  fortress to OBJ plus a hash-locked sidecar; imports into a new private pack.
  `scripts/test-broadside-editable.py` tests roundtrip/edit/rejection behavior.
  Actual no-edit OBJ roundtrip preserves all six payloads byte-for-byte.
  Revision 1 requires fixed topology/order/groups, retains baseline lightmaps,
  and depends on the source workshop pack. Blender save/export is not yet
  validated; do not claim a complete DCC pipeline or publish these assets.
  Both source-derived apps are private-only; this is not a public asset change.

- Active checkout is `/Users/jeffrywalsh/workspace/peakrunner` on `main`.
  `origin/main` is the same commit. `map/skybreak-fortress` was an ancestor of
  `main` and has been deleted. The 19 September recovery stash was dropped
  after its tracked code was already on `main`; its screenshots remain in
  ignored `research/screenshots/`. The temporary review worktree has been
  removed. Start the next change on a new branch from current `main`.
- Reusable floating-base geometry is in `scripts/assets/floating_fortress.py`;
  placement and terrain remain in the Skybreak compiler/config. See
  `docs/floating-fortress.md` for authoring and regression checks. Do not duplicate
  the building in each future map or modify approved movement to fit geometry.
- Current original asset is v8: separated lower/armory stair lanes, enclosed
  galleries and reusable stairwell floor/walls/ceiling construction in
  `scripts/assets/fortress_rooms.py`. The entrance-to-roof test now samples
  lateral connectors with body sweeps, not just isolated vertical clearance.
  See the v8 section of `docs/broadside-interior-audit.md`. Do not claim all gaps
  removed or 90% fidelity; upper routes and room matching still need work.
- Previous original asset v7 added measured hall boundaries, flag-room central
  partition/side doors and generator gallery walls. Exterior left unchanged
  this pass. `scripts/audit-fortress-interior.py` compares both bases using
  double-sided rays, with provenance hashes and private JSON reports. Run
  `local-assets/tools/venv/bin/python scripts/test-interior-audit.py` for its
  synthetic checks. See `docs/broadside-interior-audit.md` for validated values,
  corrected prior claims and still-large differences. Do not equate passing
  collision tests with fidelity or describe panel bounds as exact room plans.
- Previous original asset v6 corrected outer tower/roof envelope, enclosed
  entry passage and flag-room walls, plus separate original fortress materials
  in `scripts/assets/fortress_materials.py`. Skybreak alone uses those textures;
  Raindance and the private reference app are unchanged. See the v6 section of
  `docs/floating-fortress.md`; it supersedes contradictory v5 silhouette notes.
  A 90% overall match has NOT been established. Normal Skybreak is the rebuilt
  version; `PeakRunner-Broadside-Reference.app` remains the extracted comparison.
- Fortress v4 uses decoded Broadside_nef measurements for the deck, stacked
  levels, flag room and deep keel. See `docs/broadside-reference-study.md` for
  private diagnostic reproduction and the unverified 90% fidelity target.
  Run `scripts/test-floating-fortress.py` and core `fortress_` tests
  after edits. Local Broadside reference archive location and inspection limits
  are recorded in the study; never package source geometry or reference renders.
- Consolidation onto main is commit `a4f00b5`. The six-map release record is
  `a4d3f9b`. Selected-map-only admission and independent Skybreak pack updates
  remain future work. Skybreak traversal and balance still need human playtest.
  See `docs/skybreak-bastions.md` and `docs/multi-map-system.md`.
- Normal local command, from `src/`: `cargo run --locked --release -p peakrunner --bin peakrunner`.
  Offline choices are Raindance, Skybreak Bastions, and the installed private
  clones. Valley remains an internal test fixture only. `.20260919.4` clients
  cannot join the published `maps2` server.
- `main` is the only local and remote branch. Recreate a roadmap branch from
  current `main` when that work starts. Roadmap scope remains in `docs/roadmap.md`.
- Preferences ship in `.20260921.1`. See `docs/client-preferences.md`.
  Use isolated `PEAKRUNNER_CONFIG_DIR` for QA; never save match passwords.

## Gameplay and security contracts

- Multiplayer map rotation and modes are SERVER-selected. Client-local map data
  supports fast rendering and prediction, not authority over map/mode choice.
  Only implemented modes can be configured (CTF today). Future server-delivered
  enthusiast maps/mods are a separate later milestone: no automatic downloads or
  arbitrary server-provided code execution in the current multi-map foundation.
- The server owns identity, teams, movement validity, damage, scores and outcomes.
  Do not accept client-supplied identity, team, frag or damage claims.
- Names use `crates/core/src/names.rs`: 1–24 ASCII alphanumeric/space characters,
  outer spaces trimmed, blank/invalid input rejected. Server checks joining and
  renaming; client checks are only UX. Renames are rate-limited and retain identity.
- Chat: T public, Y team, Enter sends, Escape cancels. Chat captures gameplay input.
  Do not reintroduce a focusable desktop HUD chat button that Space can activate.
- Validate all chat on the server (160 characters/240 UTF-8 bytes, no control/bidi
  overrides, bounded history and rate limits). Render text as text, never markup.
- Team chat must be filtered in `Match::snapshot_for` before serialization, not
  merely hidden by clients. Never send a shared unfiltered snapshot to every peer.
- `GAME_VERSION` labels releases; `game_protocol()` controls compatibility. The
  current `chat2:names1` features and map fingerprint must match client/server.
  Bump compatibility when wire layouts or simulation/map compatibility change.
  This is independent of the underlying encrypted QUIC transport.
- Release `0.1.0-raindance.20260919.2` adds `ping1` to gameplay compatibility. QUIC sessions
  publish server-measured RTT by assigned player ID; local TCP shows unavailable,
  never fabricated ping. Deploy matching clients/server together before publishing.
  World sound cues retain positions and fade by listener distance; UI/hit/flag
  announcement cues are intentionally non-positional. See `docs/hud-audio.md`.
- Published release `.20260919.3` changes turret interception and the camera/
  muzzle FOV curve (`equipment3:fov1` compatibility). Launcher `0.1.0-r1`, signed
  feeds (sequence `2026091906`), standalone clients and the matching server were
  deployed together. See `docs/launcher.md`. Keep walking FOV fixed and capture stings
  team-specific; their regression tests cover snapshot replay and score resets.
- Public gameplay uses certificate-validated QUIC/UDP 7777. Never disable TLS
  verification or route gameplay through Cloudflare Tunnel to fix connectivity.
- Release `.20260919.4` was the previous visual-only public client (armor and
  light-mode menu contrast), feed `2026091907`, launcher `0.1.0-r1`. The live
  game is `0.1.0-private.20260921.1`, feed `2026092102`. `.4` clients cannot
  join it. See `docs/visual-release-20260919-4.md` and `docs/release-20260921-1.md`.
- Original assets only for distribution. Do not package extracted Tribes assets.
  See `docs/original-map-kit.md`, `docs/terrain-art-direction.md`,
  `docs/weapon-damage.md`, `docs/match-comms.md`, and `docs/multiplayer.md`.

## Build and verification

Rust version/targets and dependencies are recorded in Cargo files. Use the lockfile.

```sh
cargo test --workspace --lib
cargo check --workspace --all-targets
node scripts/check-app-boundaries.mjs
cargo check --target wasm32-unknown-unknown -p peakrunner --lib
cargo build --locked --release -p peakrunner --bin peakrunner
```

Socket tests need local TCP/UDP permission. The ignored eight-client load test is:
`cargo test -p peakrunner-server --lib eight_clients_sustain_movement_and_all_weapons -- --ignored`.
GPU capture tests are also explicitly ignored by default; run relevant captures
when changing rendering. Unit tests must not poll real macOS mouse state from
parallel test threads (see `src/mouse.rs`).

Native visual QA uses `examples/launch_smoke.rs`, which must forward BOTH eframe
`logic` and `ui`. Set `QA_CAPTURE_PATH`; optional `PEAKRUNNER_JOIN`, `QA_PAUSE=1`,
`QA_CHAT=team`/`public` exercise a temporary local match. Inspect screenshots,
not just exit codes. Do not kill a user's running game to run a test.

Cross-platform commands, packaging and caveats are in `docs/client-builds.md`.
Use `scripts/package-client.sh`; it refuses overwrite. Mac is Apple Silicon;
Windows is x64 MinGW cross-built; Linux is x64 Debian 12/glibc 2.36+.
`deploy/client-linux.Dockerfile` includes X11/Wayland, Vulkan and audio build/runtime
dependencies. Linux headless QA uses Xvfb + Mesa software Vulkan with
`docker run --init`; without init the Xvfb startup signal can stall at PID 1.
Software rendering does not prove usable frame rates; cross-compiling does not
prove native Windows behavior. Downloads are unsigned/not notarized playtests.

Local UTM compatibility VMs and installer images live on AllOfIt, outside Git.
See `docs/vm-testing.md` for architecture limits, safe VM management and QA.
Preserve the unrelated Windows XP VM. Never commit VM disks or generated accounts.

For the website: `npm --prefix site run check`; `npm --prefix site run dev` uses
`site/serve.mjs`. Production build is `docker build -f site/Dockerfile ... .`.
Verify desktop and mobile in a real browser, inspect screenshots and console,
exercise host details, and check downloads and SHA-256 over public HTTPS.
Store screenshots in `screenshots/`. There is no TanStack build for this site.

## Hosting topology

| Service | Host | Deployment |
| --- | --- | --- |
| Website + directory + tunnel | dellcon, `jeffryw@192.168.1.64` | `/data/peakrunner/public`, Compose `peakrunner-public` |
| Download files | dellcon | `/data/peakrunner/downloads`, read-only site mount |
| Signed update feed | dellcon | `/data/peakrunner/updates/v1`, read-only site mount; private signing key never uploaded |
| Build/source archives | dellcon | `/data/peakrunner/releases/<release>/` |
| Public match | VPS, `peakrunner-admin@198.12.80.145` | `/opt/peakrunner`, Compose `peakrunner-match` |

`peakrunner.net` and `www` → Cloudflare Tunnel → `site:8080`.
`dir.peakrunner.net` → tunnel → `directory:8080`.
Website `/api/*` proxies the directory directly; do not invent a second listing.
`play.peakrunner.net` → DNS-only VPS A record → UDP 7777, NOT the tunnel.
The match display name is Springdale Central; `dellcon-north-spine` is its retained
directory ID, not the display name. Do not rename IDs casually.

Use `deploy/dellcon/README.md`, `deploy/vps/README.md` and
`deploy/vps/OPERATIONS.md` for restore/certificate procedures. Verify current host
state before writes. Do not change unrelated dellcon containers, its shared Caddy,
`/data/docker-compose.yml`, firewall or DNS for a routine release.

## Safe release procedure

1. Run appropriate tests. Check live occupancy and `/servers/<id>` version/protocol.
   Coordinate disruptive restarts if players are present; recheck before cutover.
2. Build on dellcon, not the small VPS. Record immutable image IDs and source
   provenance in `deploy/vps/source.json`, `release.conf`, and
   `deploy/dellcon/site-source.json`. A dirty-tree source archive needs its SHA-256;
   do not claim it is reproduced by a clean Git revision alone.
3. Back up previous compose/release/source records on the target. Keep old images
   and versioned downloads for rollback. Transfer server images with docker
   save/load over SSH; do not overwrite an existing versioned client archive.
4. Update the matching match server before exposing incompatible client downloads.
   VPS: `sudo -n docker compose --env-file release.conf up -d --no-deps match`.
   Verify container health, advancing tick, name, map and protocol. Run a small
   WAN smoke check only when empty; larger tests consume real match slots.
5. Upload archives to dellcon, verify hashes there, then update
   `site/public/release.json` with version, URLs, checksums and honest platform
   limitations. Its description is public release status, not a build promise.
6. Build/pin the site image and copy reviewed configs to `/data/peakrunner/public`.
   For a site-only restart use:
   `PEAKRUNNER_TUNNEL_TOKEN=unused-no-tunnel-recreation docker compose up -d --no-deps --no-build site`.
   This dummy only satisfies Compose parsing; NEVER use it to recreate tunnel.
   Do not rebuild/restart the directory for a client-only protocol change.
7. Verify apex/www HTTPS, `/api/servers`, host details, desktop/mobile download UI,
   and each downloaded archive's hash. Record published vs staged status. Mirror
   non-secret deployment records to the host; local changes are not a GitHub push.

Rollback means restoring previous image pins and matching website manifest/client
links together, then targeted Compose recreation. Existing versioned files should
remain available. Never use broad Docker prune or destructive Git cleanup.

## Private Broadside source reference (2026-09-20)

The user explicitly requested a local reference using actual installed map
assets. `scripts/convert-raindance.py --mission Broadside_nef.mis` imports the
T2 Classic variant into ignored `local-assets/broadside-reference`.
`scripts/bundle-broadside-reference.sh` creates the ignored, private
`PeakRunner-Broadside-Reference.app`, with separate preferences and an in-game
coordinate/structure-distance overlay. This local reference app is an explicit
exception to the no-source-assets packaging rule; public builds are NOT.
Never publish its pack, screenshots or app, or replace public original assets.
See `docs/broadside-private-reference.md` for rebuild steps, the live interior
survey (same structure ray as the reference overlay), and fidelity limits.
Normal map manifests remain compatible through defaults for terrain spacing,
water and private-reference metadata. The Reference app and its pack stay
private. They are not the published six-map client.

## Secrets and operations safety

- VPS SSH uses `peakrunner-admin`, existing authorized keys and explicitly
  user-approved `NOPASSWD: ALL`. Use `sudo -n` for privileged operations; the user
  is not in the Docker group. This is root-equivalent access, not least privilege.
  Password and keyboard-interactive SSH, and direct root SSH, are disabled.
  Do not retry root SSH for routine deployment. Stage uploads in the admin home,
  then use `sudo -n install` into root-owned paths. Before future SSH changes,
  retain a recovery session, schedule rollback, validate `sshd -t`/`visudo -c`,
  verify a new independent connection, then cancel rollback. Do not lock or
  remove the root account; provider-console recovery is separate from SSH.

- Never print/read into chat `cf-key`, tunnel tokens, private TLS keys, password
  env files, complete `docker inspect`, or expanded `docker compose config`.
  Use narrow `--format` inspections and non-secret health/status endpoints.
- Credentials live outside Git. Broad Cloudflare token stays on the admin machine;
  certificate renewal uses a separately scoped token on the VPS. Restore from
  encrypted backups/password manager, not source control.
- `deploy/dellcon/cloudflare.mjs` without flags is read-only; `--apply --sync-config`
  changes tunnel routing. `--start` can reconcile ALL services and obtains the real
  tunnel credential. Do not use it for an ordinary site-only update.
- Preserve read-only mounts/filesystems, dropped capabilities, resource limits,
  log rotation and non-root users. Site Caddy listens on 8080; strip its bundled
  file capability during build so `cap_drop: ALL` does not prevent execution.
- Verify TLS renewals and coordinated restart procedures in the VPS runbook.
  Compose restarts exited containers, not merely unhealthy running processes.
- No promises of cheat-proof play, unlimited capacity or tested OS support.
  Keep eight-player capacity until further measured tests justify a change.
