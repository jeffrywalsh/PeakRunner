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
- Git history/remotes/stashes retained. See `docs/workspace-layout.md`.

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
Find a Rift preferences are documented in `docs/client-preferences.md`.
They are not in the published `.20260919.4` binaries; never store passwords.

## Resume checkpoint — original checkout restored

- 2026-09-21 collection extension: **Snowblind Clone** and **Desert of Death
  Clone** are installed in the normal native game's wrapped map menu, at
  `local-assets/snowblind-clone/installed` and
  `local-assets/desert-of-death-clone/installed`. Both use the NEW native readers
  and each has an original donut poster; they remain source-derived/private and
  are rejected by public server startup/rotation via `MapId::is_private_clone()`.
  No deployment, commit or public asset packaging. Broadside/Stonehenge installed
  assets were not replaced. See `docs/collection-snowblind-desert.md` for builds,
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
  Normal client offers **Stonehenge Clone** without env overrides; private map
  is rejected by dedicated-server startup and rotation. Nothing deployed.
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
  current client code, NOT the frozen Reference executable. Private clone is
  rejected by dedicated-server startup/rotation. Inventory from the NEW
  `tools/broadside_clone/inventory.py` found 84 extracted mission files; report
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

- Work in `/Users/jeffrywalsh/workspace/PeakRunner/peakrunner`. The original
  checkout is on `map/skybreak-fortress`, based on main `8260f5b`, including
  preferences, rotation and Skybreak. The fortress pass is not deployed.
  Use branches in this directory for future work; do not redirect normal builds
  to the temporary review worktree.
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
- User explicitly requested consolidation of the current development work onto
  main. This does NOT mean the unfinished multi-map system is release-ready.
  Skybreak traversal/balance, full map-transition QA, selected-map admission and
  signed multi-pack launcher packaging remain pending. See
  `docs/skybreak-bastions.md` and `docs/multi-map-system.md`.
- Normal local command: `cargo run --locked --release -p peakrunner --bin peakrunner`.
  Offline choices are Raindance and Skybreak Bastions. Valley remains an internal
  test fixture only. Protocol `maps2` cannot join the public .4 server.
- Live services/downloads remain `0.1.0-raindance.20260919.4`, launcher
  `0.1.0-r1`, feed `2026091907`. Git publication is NOT deployment.
- Original tracked/untracked working state was preserved in LOCAL recovery stash
  `220972590028f1bdcd61f363bef9f8a415a0e1b7` before switching branches.
  It may include private reference screenshots: never publish the stash.
  Do not blindly apply it onto main: most code duplicates already committed work.
  Unique untracked files are restored separately where they do not collide;
  differing older screenshots/documentation remain recoverable from the stash.
  Ignored local assets, credentials, downloads and build cache were left alone.
- The temporary review worktree `/private/tmp/peakrunner-baseline.SZ5oeH`
  is detached at `59524c1`; it is not the active workspace or a durable backup.
- Delete only branches proven ancestors of main. Empty reserved roadmap branches
  count as merged; recreate the relevant branch from current main when work starts.
  Roadmap scope remains in `docs/roadmap.md`.
- Preferences are in main, not the .4 downloads; see `docs/client-preferences.md`.
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
- Client `.20260919.4` is the published visual-only release (armor and light-mode
  menu contrast). Signed feed sequence `2026091907`; launcher remains `0.1.0-r1`.
  The match server was subsequently aligned to `.4` at the user's request with
  an approved restart; `.3` remains protocol-compatible. See
  `docs/visual-release-20260919-4.md` and `deploy/launcher-release.json`.
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
water and private-reference metadata. This work is not deployed or committed.

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
