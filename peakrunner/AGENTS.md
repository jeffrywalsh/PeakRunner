# PeakRunner project guide

Read this before changing code or infrastructure. This directory is the active
Rust workspace; the Git root is one directory above it. Commands below run here.
The parent React/Grok scaffold is a legacy prototype, not the native game or the
live peakrunner.net site. Do not replace this project with a web scaffold or apply
the legacy `/workspace:8080` deployment assumptions to native/Docker work.

## Working rules

- Inspect `git status` first. Preserve unrelated/uncommitted work. Do not commit,
  push, merge, delete branches or reset files unless the user requests it.
- Use `rg` for discovery and `apply_patch` for source changes. Read applicable
  `.grok/skills` in the parent when editing UI, gameplay or input. Adapt their
  guidance to Rust/egui rather than introducing React into the client.
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

## Resume checkpoint — 2026-09-19

- Reviewed release baseline `d84133d` is committed and published on GitHub
  `main`. See `docs/release-baseline-review.md` for source-archive equality,
  test results and exclusions. This Git reconciliation did not redeploy services.
- Live game/server remain `0.1.0-raindance.20260919.4`; launcher
  `0.1.0-r1`, feed sequence `2026091907`. Recheck live state before deployment.
- The original checkout at
  `/Users/jeffrywalsh/workspace/PeakRunner` remains on `terrain-materials`
  with its dirty contents preserved. Much of that diff is now committed on main;
  do not blindly commit it again or reset/clean it.
- The clean publication worktree is `/private/tmp/peakrunner-baseline.SZ5oeH`
  (native workspace in its `peakrunner/` subdirectory). Temporary worktrees
  are not durable backups: use `git worktree list` and remote main to recover.
- Find a Rift preferences were isolated on `feature/find-rift-preferences`
  and verified for main: 133 library tests passed, native all-target, Windows
  cross-compile and WebAssembly checks passed. The real macOS browser capture
  restored the saved name/addresses and left the password blank; see
  `screenshots/preferences-browser.png` and `docs/client-preferences.md`.
  No Windows/Linux native runtime test is claimed for this feature.
  Source publication is separate from distribution: preferences are NOT in .4.
- Preferences remember name, directory URL and direct host, never passwords.
  Paths: macOS `~/Library/Application Support/PeakRunner/client.json`;
  Windows `%APPDATA%\PeakRunner\client.json`; Linux
  `$XDG_CONFIG_HOME/PeakRunner/client.json` or `~/.config/PeakRunner/client.json`.
  Use an isolated `PEAKRUNNER_CONFIG_DIR` for QA; automated join sessions must
  not overwrite real preferences.
- Feature/map branch names, order and merge gates are in `docs/roadmap.md`.
  Initialize each from the reviewed main baseline; update from tested main before
  starting later portions. Merge/push only completed, tested work. No branch
  deletion or force pushes. Map names are proposals, all distributed assets original.
- Next: update `feature/multi-map-system` from tested main and implement the
  multi-map foundation before individual maps, then modes, inventory and armor.
  A new client release is still required to deliver preferences to launcher users;
  do not overwrite existing .4 archives or restart servers for this client-only work.

## Gameplay and security contracts

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
